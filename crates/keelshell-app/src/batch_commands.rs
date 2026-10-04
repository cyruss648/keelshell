//! Explicit multi-host review and an owned, in-memory batch receipt.
use std::{collections::HashSet, sync::Arc, time::Duration};

use gpui_kit::{
    component::input::{InputState, TextareaState},
    *,
};
use keelshell_core::{BatchCommandTemplate, BatchTargetContext};
use keelshell_session::{
    BatchEvent, BatchHandle, BatchOptions, BatchPolicy, BatchRowReceipt, BatchTarget,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Non-sensitive metadata emitted after a reviewed batch reaches a terminal
/// state. The command text, endpoints and command output are deliberately not
/// included; the workspace may persist this as an audit trail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BatchAuditDraft {
    pub command_digest: [u8; 32],
    pub profile_ids: Vec<Uuid>,
    pub target_count: usize,
    pub succeeded: u32,
    pub failed: u32,
    pub unknown: u32,
    pub not_started: u32,
    pub cancelled: bool,
    pub stopped_after_failure: bool,
}

use crate::i18n::{Message, t};

#[cfg(test)]
mod tests;
mod view;

/// Metadata captured from an already authenticated terminal, never from output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub id: Uuid,
    /// Saved profile identity when the session came from the connection library.
    /// One-time sessions intentionally keep this empty.
    pub profile_id: Option<Uuid>,
    pub entity: EntityId,
    pub name: String,
    pub endpoint: String,
    pub route: String,
    /// Immutable metadata used only for local per-target template rendering.
    pub template_context: BatchTargetContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Review {
    pub token: Uuid,
    pub targets: Vec<Uuid>,
    /// Exact source text shown during review and used for the audit digest.
    pub command: String,
    /// One immutable rendered command per selected target, in row order.
    pub commands: Vec<(Uuid, String)>,
    pub concurrency: usize,
    pub timeout_seconds: u64,
    pub stop_after_failure: bool,
}

pub(crate) enum BatchPanelEvent {
    Start(Review),
    Completed(BatchAuditDraft),
    Hide,
    New,
}

struct Row {
    destination: Destination,
    selected: bool,
    available: bool,
    started: bool,
    receipt: Option<Arc<BatchRowReceipt>>,
}

pub(crate) struct BatchPanel {
    rows: Vec<Row>,
    command: Entity<TextareaState>,
    concurrency: Entity<InputState>,
    timeout: Entity<InputState>,
    stop_after_failure: bool,
    review: Option<Review>,
    starting: bool,
    handle: Option<BatchHandle>,
    complete: bool,
    cancelling: bool,
    audit_emitted: bool,
    detail: Option<Uuid>,
    detail_text: SharedString,
    message: Option<Message>,
    focus: FocusHandle,
    _poll: Option<Task<()>>,
}

fn input(value: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut field = InputState::new(window, cx);
        field.set_value(value, window, cx);
        field
    })
}

impl BatchPanel {
    pub(crate) fn new(
        destinations: Vec<Destination>,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            rows: destinations
                .into_iter()
                .map(|destination| Row {
                    destination,
                    selected: false,
                    available: true,
                    started: false,
                    receipt: None,
                })
                .collect(),
            command: cx.new(|cx| {
                let mut field = TextareaState::new(window, cx).rows(6);
                field.set_value(text, window, cx);
                field
            }),
            concurrency: input("2", window, cx),
            timeout: input("30", window, cx),
            stop_after_failure: true,
            review: None,
            starting: false,
            handle: None,
            complete: false,
            cancelling: false,
            audit_emitted: false,
            detail: None,
            detail_text: "".into(),
            message: None,
            focus: cx.focus_handle(),
            _poll: None,
        }
    }

    pub(crate) fn destinations(&self) -> impl Iterator<Item = &Destination> {
        self.rows.iter().map(|row| &row.destination)
    }

    pub(crate) fn is_running(&self) -> bool {
        self.starting || self.handle.is_some()
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    fn editable(&self) -> bool {
        self.review.is_none() && !self.is_running() && !self.complete
    }

    fn snapshot(&self, token: Uuid, cx: &App) -> Result<Review, Message> {
        let command = self.command.read(cx).value().to_string();
        let targets: Vec<_> = self
            .rows
            .iter()
            .filter(|row| row.selected)
            .map(|row| row.destination.id)
            .collect();
        if targets.is_empty()
            || targets.len() > 32
            || self.rows.iter().any(|r| r.selected && !r.available)
        {
            return Err(Message::new(
                "请选择 1–32 个仍在线的 SSH 会话。",
                "Select 1–32 SSH sessions that are still connected.",
            ));
        }
        if command.trim().is_empty()
            || command.len() > 65_536
            || command
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(Message::new(
                "请输入最多 64 KiB 的命令，不含终端控制字符。",
                "Enter a command up to 64 KiB without terminal control characters.",
            ));
        }
        let concurrency = self
            .concurrency
            .read(cx)
            .value()
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=8).contains(n));
        let timeout_seconds = self
            .timeout
            .read(cx)
            .value()
            .parse::<u64>()
            .ok()
            .filter(|n| (1..=300).contains(n));
        let (Some(concurrency), Some(timeout_seconds)) = (concurrency, timeout_seconds) else {
            return Err(Message::new(
                "并发数必须为 1–8，单主机超时必须为 1–300 秒。",
                "Concurrency must be 1–8; per-host timeout must be 1–300 seconds.",
            ));
        };
        let commands = render_target_commands(&command, &self.rows)?;
        Ok(Review {
            token,
            targets,
            command,
            commands,
            concurrency,
            timeout_seconds,
            stop_after_failure: self.stop_after_failure,
        })
    }

    fn prepare(&mut self, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        match self.snapshot(Uuid::new_v4(), cx) {
            Ok(review) => {
                self.review = Some(review);
                self.set_disabled(true, cx);
                self.message = None;
            }
            Err(error) => self.message = Some(error),
        }
        cx.notify();
    }

    pub(crate) fn review_current(&self, review: &Review, cx: &App) -> bool {
        self.review.as_ref() == Some(review)
            && self
                .snapshot(review.token, cx)
                .is_ok_and(|snapshot| snapshot == *review)
            && self.handle.is_none()
            && !self.complete
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.review.clone() else {
            return;
        };
        if self.starting || !self.review_current(&review, cx) {
            return;
        }
        self.starting = true;
        cx.emit(BatchPanelEvent::Start(review));
        cx.notify();
    }

    pub(crate) fn fail_start(&mut self, error: Message, cx: &mut Context<Self>) {
        self.starting = false;
        self.message = Some(error);
        cx.notify();
    }

    pub(crate) fn begin(
        &mut self,
        review: &Review,
        targets: Vec<BatchTarget>,
        runtime: &tokio::runtime::Runtime,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.starting || !self.review_current(review, cx) {
            return;
        }
        let options = BatchOptions {
            concurrency: review.concurrency,
            timeout: Duration::from_secs(review.timeout_seconds),
            output_limit: 1024 * 1024,
            policy: if review.stop_after_failure {
                BatchPolicy::StopAfterFailure
            } else {
                BatchPolicy::Continue
            },
        };
        let result = {
            let _runtime = runtime.enter();
            keelshell_session::start_batch(targets, options)
        };
        self.starting = false;
        match result {
            Ok(handle) => {
                self.handle = Some(handle);
                self.message = None;
                self._poll = Some(cx.spawn_in(window, async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(32))
                            .await;
                        if !this
                            .update_in(cx, |panel, _, cx| panel.poll(cx))
                            .unwrap_or(false)
                        {
                            break;
                        }
                    }
                }));
            }
            Err(error) => {
                self.message = Some(Message::detail(
                    "批量任务未启动",
                    "Batch did not start",
                    error,
                ))
            }
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(handle) = &mut self.handle else {
            return false;
        };
        // Observe completion before draining: a worker finishing between an empty
        // read and this observation must not make its last receipts disappear.
        let finished = handle.is_finished();
        let mut changed = false;
        while let Ok(event) = handle.try_recv() {
            match event {
                BatchEvent::Started { id } => {
                    if let Some(row) = self.rows.iter_mut().find(|r| r.destination.id == id) {
                        row.started = true;
                    }
                }
                BatchEvent::Finished { row: receipt } => {
                    if let Some(row) = self
                        .rows
                        .iter_mut()
                        .find(|r| r.destination.id == receipt.id)
                    {
                        row.receipt = Some(receipt);
                    }
                }
            }
            changed = true;
        }
        if finished {
            self.complete = true;
            self.handle = None;
            if self.rows.iter().any(|r| r.selected && r.receipt.is_none()) {
                self.message = Some(Message::new(
                    "执行器未返回完整回执，缺失结果未知；不要自动重试。",
                    "Worker returned no complete receipt; missing outcomes are unknown. Do not retry automatically.",
                ));
            }
            if !self.audit_emitted {
                self.audit_emitted = true;
                cx.emit(BatchPanelEvent::Completed(self.audit_draft()));
            }
            changed = true;
        }
        if changed {
            if self.detail.is_none() {
                self.detail = self
                    .rows
                    .iter()
                    .find(|r| r.selected && r.receipt.is_some())
                    .map(|r| r.destination.id);
            }
            self.refresh_detail(cx);
            cx.notify();
        }
        self.handle.is_some()
    }

    pub(crate) fn update_available(&mut self, live: &HashSet<EntityId>, cx: &mut Context<Self>) {
        let mut lost = false;
        for row in &mut self.rows {
            if row.available && !live.contains(&row.destination.entity) {
                row.available = false;
                lost |= row.selected;
            }
        }
        if lost {
            if let Some(handle) = &self.handle {
                handle.cancel();
                self.cancelling = true;
            }
            self.message = Some(Message::new(
                "所选会话已结束；尚未开始的任务停止，已开始的任务按回执判断结果。",
                "A selected session ended. Pending work stops; started work is reported by its receipt.",
            ));
            cx.notify();
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = &self.handle {
            handle.cancel();
            self.cancelling = true;
            cx.notify();
        }
    }

    fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.command
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        for field in [&self.concurrency, &self.timeout] {
            field.update(cx, |input, cx| input.set_disabled(disabled, cx));
        }
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.is_running() || self.complete {
            return;
        }
        self.review = None;
        self.message = None;
        self.set_disabled(false, cx);
        cx.notify();
    }

    pub(crate) fn refresh_locale(&mut self, cx: &mut Context<Self>) {
        self.refresh_detail(cx);
        cx.notify();
    }

    fn refresh_detail(&mut self, cx: &App) {
        self.detail_text = self
            .detail
            .and_then(|id| self.rows.iter().find(|r| r.destination.id == id))
            .and_then(|row| row.receipt.as_ref())
            .map(|row| {
                format!(
                    "stdout\n{}\n\nstderr\n{}",
                    output_preview(&row.stdout, cx),
                    output_preview(&row.stderr, cx)
                )
            })
            .unwrap_or_default()
            .into();
    }

    fn audit_draft(&self) -> BatchAuditDraft {
        let mut succeeded = 0;
        let mut failed = 0;
        let mut unknown = 0;
        let mut not_started = 0;
        let mut profile_ids = Vec::new();
        for row in self.rows.iter().filter(|row| row.selected) {
            if let Some(id) = row.destination.profile_id {
                profile_ids.push(id);
            }
            match row.receipt.as_ref().map(|receipt| receipt.outcome) {
                Some(keelshell_session::BatchOutcome::Exited { code: 0 }) => succeeded += 1,
                Some(keelshell_session::BatchOutcome::Exited { .. })
                | Some(keelshell_session::BatchOutcome::Rejected) => failed += 1,
                Some(keelshell_session::BatchOutcome::Unknown { .. }) | None => unknown += 1,
                Some(keelshell_session::BatchOutcome::NotStarted { .. }) => not_started += 1,
            }
        }
        profile_ids.sort_unstable();
        profile_ids.dedup();
        let digest_input = self
            .review
            .as_ref()
            .map(|review| {
                let mut input = review.command.clone();
                for (id, command) in &review.commands {
                    input.push('\n');
                    input.push_str(&id.to_string());
                    input.push('=');
                    input.push_str(command);
                }
                input
            })
            .unwrap_or_default();
        BatchAuditDraft {
            command_digest: Sha256::digest(digest_input.as_bytes()).into(),
            profile_ids,
            target_count: self.rows.iter().filter(|row| row.selected).count(),
            succeeded,
            failed,
            unknown,
            not_started,
            cancelled: self.cancelling,
            stopped_after_failure: self.rows.iter().any(|row| {
                row.receipt.as_ref().is_some_and(|receipt| {
                    matches!(
                        receipt.outcome,
                        keelshell_session::BatchOutcome::NotStarted {
                            reason: keelshell_session::BatchNotStartedReason::StoppedAfterFailure
                        }
                    )
                })
            }),
        }
    }
}

impl EventEmitter<BatchPanelEvent> for BatchPanel {}

/// Render one immutable command for each selected target during the review step.
/// No session, network, retry, or execution operation occurs here.
fn render_target_commands(command: &str, rows: &[Row]) -> Result<Vec<(Uuid, String)>, Message> {
    let template = BatchCommandTemplate::compile(command).map_err(|error| {
        Message::new(
            format!("批量模板无效：{error}"),
            format!("Invalid batch template: {error}"),
        )
    })?;
    rows.iter()
        .filter(|row| row.selected)
        .map(|row| {
            let rendered = template
                .as_ref()
                .map(|template| template.render(&row.destination.template_context))
                .transpose()
                .map_err(|error| {
                    Message::new(
                        format!("目标 {} 的模板无法展开：{error}", row.destination.name),
                        format!(
                            "Could not render the template for {}: {error}",
                            row.destination.name
                        ),
                    )
                })?
                .unwrap_or_else(|| command.to_owned());
            Ok((row.destination.id, rendered))
        })
        .collect()
}

fn output_preview(bytes: &[u8], cx: &App) -> String {
    const LIMIT: usize = 64 * 1024;
    let truncated = bytes.len() > LIMIT;
    let mut text = String::new();
    for c in String::from_utf8_lossy(&bytes[..bytes.len().min(LIMIT)]).chars() {
        if (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            text.extend(c.escape_default());
        } else {
            text.push(c);
        }
    }
    if truncated {
        text.push_str(t(
            cx,
            "\n[预览仅显示前 64 KiB]",
            "\n[Preview shows the first 64 KiB]",
        ));
    }
    text
}
