//! Native review for the raw PTY's explicitly leased file-transfer protocol.
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use keelshell_session::zmodem::{ZmodemControl, ZmodemDirection, ZmodemFailure, ZmodemUiEvent};
use uuid::Uuid;

use super::*;
use gpui_kit::base::{Disableable, StyledExt};

const MAX_VERIFIED_SAVES: usize = 32;
const MAX_PENDING_RESUMES: usize = 256;

/// The bridge publishes ownership before an event can wait in a full UI queue.
/// Acknowledging an older uncertainty cannot clear a newer, unread result.
#[derive(Default)]
pub(crate) struct Safety {
    pub(crate) active: AtomicBool,
    pub(crate) uncertain_epoch: AtomicU64,
    publication_running: Arc<AtomicBool>,
    acknowledged_epoch: AtomicU64,
    events: Mutex<VecDeque<ZmodemUiEvent>>,
}

impl Safety {
    pub(crate) fn new(publication_running: Arc<AtomicBool>) -> Self {
        Self {
            publication_running,
            ..Self::default()
        }
    }

    pub(crate) fn close_busy(&self) -> bool {
        self.active.load(Ordering::Acquire)
            || self.publication_running.load(Ordering::Acquire)
            || self.uncertain_epoch.load(Ordering::Acquire)
                != self.acknowledged_epoch.load(Ordering::Acquire)
    }

    pub(crate) fn publish(&self, event: ZmodemUiEvent) -> Result<(), String> {
        // Keep the close hold even if the bounded notification queue cannot
        // retain this result. An unread uncertainty is never acknowledged.
        if matches!(
            event,
            ZmodemUiEvent::Failed {
                uncertain: true,
                ..
            } | ZmodemUiEvent::Cancelled {
                uncertain: true,
                ..
            }
        ) {
            self.uncertain_epoch.fetch_add(1, Ordering::AcqRel);
        }
        let mut events = self
            .events
            .lock()
            .map_err(|_| "ZMODEM_UI_STATE_UNAVAILABLE")?;
        if let ZmodemUiEvent::Progress { id, .. } = &event
            && matches!(events.back(), Some(ZmodemUiEvent::Progress { id: previous, .. }) if previous == id)
        {
            events.pop_back();
        }
        if events.len() >= 256 {
            return Err("ZMODEM_UI_STATE_LIMIT".into());
        }
        events.push_back(event);
        Ok(())
    }

    pub(crate) fn take_events(&self) -> Result<Vec<ZmodemUiEvent>, String> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| "ZMODEM_UI_STATE_UNAVAILABLE")?;
        let count = events.len().min(64);
        Ok(events.drain(..count).collect())
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) safety: Arc<Safety>,
    current: Option<Transfer>,
    seen_uncertain_epoch: u64,
    unresolved: BTreeMap<Uuid, Message>,
    acknowledge_review: Option<(u64, Vec<Uuid>)>,
    pending_resumes: VecDeque<Uuid>,
    verified_saves: VecDeque<VerifiedSave>,
}

#[derive(Debug, PartialEq, Eq)]
struct VerifiedSave {
    id: Uuid,
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

impl VerifiedSave {
    fn summary(&self) -> Message {
        Message::new(
            format!(
                "已验证保存：{}\n{} 字节\nSHA-256: {}",
                self.path.display(),
                self.bytes,
                self.sha256
            ),
            format!(
                "Verified saved file: {}\n{} bytes\nSHA-256: {}",
                self.path.display(),
                self.bytes,
                self.sha256
            ),
        )
    }
}

struct Transfer {
    id: Uuid,
    choice: Choice,
    stage: Stage,
    summary: Message,
    feedback: Message,
    picker: Option<Uuid>,
}

enum Choice {
    Upload,
    Download { offer: Uuid, name: String },
}

enum Stage {
    Upload,
    Download { offer: Uuid, name: String },
    Review { review: Uuid },
    Running,
    Cancelling,
    Ended,
}

impl TerminalView {
    pub(crate) fn attach_zmodem_safety(&mut self, safety: Arc<Safety>) {
        self.zmodem.safety = safety;
    }

    pub(crate) fn zmodem_close_busy(&self) -> bool {
        self.zmodem.safety.close_busy()
            || !self.zmodem.unresolved.is_empty()
            || self
                .zmodem
                .current
                .as_ref()
                .is_some_and(|transfer| !matches!(transfer.stage, Stage::Ended))
    }

    pub(super) fn poll_zmodem(&mut self, cx: &mut Context<Self>) {
        match self.zmodem.safety.take_events() {
            Ok(events) => {
                for event in events {
                    self.zmodem_event(event, cx);
                }
            }
            Err(error) => self.input_error = Some(error),
        }
    }

    pub(super) fn zmodem_input_blocked(&self) -> bool {
        self.zmodem.safety.active.load(Ordering::Acquire)
            || self
                .zmodem
                .current
                .as_ref()
                .is_some_and(|transfer| !matches!(transfer.stage, Stage::Ended))
    }

    pub(super) fn retry_zmodem_resume(&mut self) {
        while let Some(id) = self.zmodem.pending_resumes.pop_front() {
            match self.outgoing.try_send(TerminalCommand::ResumeInput(id)) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => {
                    self.zmodem.pending_resumes.push_front(id);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.zmodem.pending_resumes.clear();
                    break;
                }
            }
        }
    }

    pub(super) fn zmodem_event(&mut self, event: ZmodemUiEvent, cx: &mut Context<Self>) {
        let endpoint = self.transport_target.as_deref().unwrap_or("");
        match event {
            ZmodemUiEvent::UploadRequested { id } => {
                self.zmodem.current = Some(Transfer {
                    id,
                    choice: Choice::Upload,
                    stage: Stage::Upload,
                    summary: Message::new(
                        format!(
                            "当前 SSH 目标：{endpoint}\n远端接收程序请求 Zmodem 上传。选择本地文件后，先审核完整文件清单；选择本身不会发送文件。"
                        ),
                        format!(
                            "Current SSH target: {endpoint}\nThe remote receiver requests a Zmodem upload. Choose local files, then review the complete list. Selecting files does not send them."
                        ),
                    ),
                    feedback: Message::empty(),
                    picker: None,
                });
            }
            ZmodemUiEvent::DownloadOffered {
                id,
                offer,
                name,
                size,
            } => {
                self.zmodem.current = Some(Transfer {
                    id,
                    choice: Choice::Download {
                        offer,
                        name: name.clone(),
                    },
                    stage: Stage::Download {
                        offer,
                        name: name.clone(),
                    },
                    summary: Message::new(
                        format!(
                            "当前 SSH 目标：{endpoint}\n远端文件：{name}\n声明字节数：{size}\n\n选择新的本地文件名，然后审核并确认。当前尚未接受文件内容；已有文件不会被覆盖。后续文件需要分别审核。"
                        ),
                        format!(
                            "Current SSH target: {endpoint}\nRemote file: {name}\nDeclared bytes: {size}\n\nChoose a new local filename, then review and confirm. No file content has been accepted. Existing files are preserved. Each subsequent file requires its own review."
                        ),
                    ),
                    feedback: Message::empty(),
                    picker: None,
                });
            }
            ZmodemUiEvent::Review {
                transfer,
                review,
                direction,
                files,
                destination,
            } => {
                let Some(current) = self.zmodem.current.as_mut().filter(|current| {
                    current.id == transfer
                        && !matches!(current.stage, Stage::Ended | Stage::Cancelling)
                }) else {
                    return;
                };
                let mut zh = format!("当前 SSH 目标：{endpoint}\n");
                let mut en = format!("Current SSH target: {endpoint}\n");
                for file in files {
                    zh.push_str(&format!(
                        "\n文件：{}\n本地路径：{}\n字节数：{}\n",
                        file.name,
                        file.path.display(),
                        file.size
                    ));
                    en.push_str(&format!(
                        "\nFile: {}\nLocal path: {}\nBytes: {}\n",
                        file.name,
                        file.path.display(),
                        file.size
                    ));
                    if let Some(digest) = file.sha256 {
                        zh.push_str(&format!("SHA-256: {digest}\n"));
                        en.push_str(&format!("SHA-256: {digest}\n"));
                    }
                }
                match direction {
                    ZmodemDirection::Upload => {
                        zh.push_str("\n确认后重新核验本地源并发送这些文件。远端 rz 程序决定保存目录和覆盖策略；本工具无法从该协议证明远端保存位置、未覆盖旧文件或保存后内容。取消不表示远端已删除接收的数据。");
                        en.push_str("\nConfirmation revalidates local sources and sends these files. The remote rz program controls its directory and overwrite policy. This protocol cannot verify its save location, preservation of existing remote files or saved contents. Cancellation does not prove remote data was removed.");
                    }
                    ZmodemDirection::Download => {
                        if let Some(path) = destination {
                            zh.push_str(&format!("\n新的本地目标：{}\n", path.display()));
                            en.push_str(&format!("\nNew local target: {}\n", path.display()));
                        }
                        zh.push_str("\n确认只接受本次文件。完整接收、按新目标发布并读回验证后才显示保存成功；已有目标保留。异常结果需要核对，不自动重试。");
                        en.push_str("\nConfirmation accepts only this file. A saved result requires complete reception, publication to the new target and verified readback. Existing targets are preserved. Inspect uncertain outcomes; there is no automatic retry.");
                    }
                }
                current.summary = Message::new(zh, en);
                current.stage = Stage::Review { review };
                current.feedback = Message::empty();
                current.picker = None;
            }
            ZmodemUiEvent::Progress { id, bytes, total } => {
                if let Some(current) = self.zmodem.current.as_mut().filter(|current| {
                    current.id == id && !matches!(current.stage, Stage::Ended | Stage::Cancelling)
                }) {
                    current.stage = Stage::Running;
                    current.feedback = Message::new(
                        format!("协议进度：{bytes} / {total} 字节"),
                        format!("Protocol progress: {bytes} / {total} bytes"),
                    );
                }
            }
            ZmodemUiEvent::FileSaved {
                id,
                path,
                bytes,
                sha256,
            } => {
                let saved = VerifiedSave {
                    id,
                    path,
                    bytes,
                    sha256,
                };
                // A verified publication is independent of the current offer
                // and its transient feedback, including late notifications.
                if !self.zmodem.verified_saves.contains(&saved) {
                    if self.zmodem.verified_saves.len() == MAX_VERIFIED_SAVES {
                        self.zmodem.verified_saves.pop_front();
                    }
                    self.zmodem.verified_saves.push_back(saved);
                }
            }
            ZmodemUiEvent::Finished { id } => {
                self.end_zmodem(id, Message::new(
                    "Zmodem 协议已完成。上传结果仍需在远端检查；下载以已验证保存的回执为准。",
                    "Zmodem protocol completed. Inspect uploaded files remotely; downloads are confirmed by their verified save receipts.",
                ), false);
            }
            ZmodemUiEvent::EndedWithoutFarewell { id } => {
                self.end_zmodem(id, Message::new(
                    "远端未发送最终 OO，Zmodem 已结束等待。此结果不表示完整握手成功；下载仅以各文件的已验证保存回执为准。",
                    "The peer did not send the final OO; Zmodem stopped waiting. The full handshake was not confirmed. Only each file's verified save receipt confirms its download.",
                ), false);
            }
            ZmodemUiEvent::Cancelled { id, uncertain } => {
                self.end_zmodem(id, Message::new(
                    "Zmodem 已取消。取消不能证明远端文件已删除或本地发布已回滚。",
                    "Zmodem cancelled. Cancellation does not prove remote removal or local publication rollback.",
                ), uncertain);
            }
            ZmodemUiEvent::Failed {
                id,
                reason,
                uncertain,
            } => {
                self.end_zmodem(id, zmodem_failure(reason), uncertain);
            }
            ZmodemUiEvent::InputRejected { id } => {
                if let Some(current) = self.zmodem.current.as_mut().filter(|current| {
                    current.id == id && !matches!(current.stage, Stage::Ended | Stage::Cancelling)
                }) {
                    current.feedback = Message::new(
                        "Zmodem 占用终端期间，普通输入未发送，也不会在传输结束后重放。",
                        "Ordinary input was not sent while Zmodem owns this terminal and will not be replayed afterwards.",
                    );
                }
            }
            ZmodemUiEvent::ActionRejected { id, reason } => {
                if let Some(current) = self.zmodem.current.as_mut().filter(|current| {
                    current.id == id && !matches!(current.stage, Stage::Ended | Stage::Cancelling)
                }) {
                    current.stage = match &current.choice {
                        Choice::Upload => Stage::Upload,
                        Choice::Download { offer, name } => Stage::Download {
                            offer: *offer,
                            name: name.clone(),
                        },
                    };
                    current.summary = Message::new(
                        format!(
                            "当前 SSH 目标：{endpoint}\n上次操作未获授权，请重新选择文件或新的本地目标并审核。"
                        ),
                        format!(
                            "Current SSH target: {endpoint}\nThe previous action was not authorized. Choose files or a new local target and review again."
                        ),
                    );
                    current.feedback = zmodem_failure(reason);
                    current.picker = None;
                }
            }
        }
        if self.zmodem_input_blocked() {
            self.pending.clear_writes();
            self.composition.clear();
        }
        self.retry_zmodem_resume();
        cx.notify();
    }

    fn end_zmodem(&mut self, id: Uuid, feedback: Message, uncertain: bool) {
        if uncertain {
            self.zmodem.seen_uncertain_epoch = self.zmodem.seen_uncertain_epoch.saturating_add(1);
        }
        if let Some(current) = self
            .zmodem
            .current
            .as_mut()
            .filter(|current| current.id == id)
        {
            if !matches!(current.stage, Stage::Ended) {
                current.stage = Stage::Ended;
                current.picker = None;
                current.feedback = feedback.clone();
            }
            if uncertain {
                let (zh, en) = current.summary.translations();
                let (zh_result, en_result) = feedback.translations();
                self.zmodem.unresolved.insert(
                    id,
                    Message::new(
                        format!("{zh}\n\n{zh_result}"),
                        format!("{en}\n\n{en_result}"),
                    ),
                );
                self.zmodem.acknowledge_review = None;
            }
        } else if uncertain {
            // An older result still protects close even when a newer offer is
            // already visible. Do not replace that new offer with an old one.
            self.zmodem.unresolved.insert(id, feedback);
            self.zmodem.acknowledge_review = None;
        }
        // Receive leases precede the first file offer, so there may be no
        // card (or only a previous card). Only the bridge can release its
        // latch, after checking this exact ID and draining protocol writes.
        if !self.zmodem.pending_resumes.contains(&id) {
            if self.zmodem.pending_resumes.len() < MAX_PENDING_RESUMES {
                self.zmodem.pending_resumes.push_back(id);
            } else {
                // Preserve queued handoffs and the bridge's input fence if
                // notification traffic exceeds the bounded handoff budget.
                self.input_error = Some("ZMODEM_INPUT_HANDOFF_LIMIT".into());
            }
        }
    }

    fn zmodem_control(&mut self, control: ZmodemControl, cx: &mut Context<Self>) {
        let Some(current) = self.zmodem.current.as_mut() else {
            return;
        };
        if matches!(current.stage, Stage::Ended | Stage::Cancelling) {
            return;
        }
        let current_action = match &control {
            ZmodemControl::PrepareUpload { transfer, .. } => {
                *transfer == current.id && matches!(current.stage, Stage::Upload)
            }
            ZmodemControl::PrepareReceive {
                transfer, offer, ..
            } => {
                *transfer == current.id
                    && matches!(current.stage, Stage::Download { offer: current_offer, .. } if current_offer == *offer)
            }
            ZmodemControl::Confirm { transfer, review } => {
                *transfer == current.id
                    && matches!(current.stage, Stage::Review { review: current_review } if current_review == *review)
            }
            ZmodemControl::Cancel { transfer } => *transfer == current.id,
        };
        if !current_action {
            return;
        }
        let cancelling = matches!(control, ZmodemControl::Cancel { .. });
        match self
            .outgoing
            .try_send(TerminalCommand::Zmodem(Box::new(control)))
        {
            Ok(()) => {
                current.stage = if cancelling {
                    Stage::Cancelling
                } else {
                    Stage::Running
                };
                current.picker = None;
                current.feedback = Message::new("正在处理本次操作…", "Processing this operation…");
            }
            Err(mpsc::TrySendError::Full(_)) => {
                current.feedback = Message::new(
                    "终端操作队列已满，本次操作未提交；请稍后重新点击。",
                    "The terminal operation queue is full. This action was not submitted; try again.",
                );
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                current.feedback = Message::new(
                    "终端连接已关闭，本次操作未提交。",
                    "The terminal connection is closed. This action was not submitted.",
                );
            }
        }
        cx.notify();
    }

    fn choose_zmodem_sources(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.zmodem.current.as_mut().filter(|current| {
            current.id == id && matches!(current.stage, Stage::Upload) && current.picker.is_none()
        }) else {
            return;
        };
        let picker = Uuid::new_v4();
        current.picker = Some(picker);
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(cx, "选择 Zmodem 上传文件", "Choose files for Zmodem upload").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = choice.await;
            let _ = this.update_in(cx, |view, _, cx| {
                let Some(current) = view.zmodem.current.as_mut().filter(|current| {
                    current.id == id
                        && current.picker == Some(picker)
                        && matches!(current.stage, Stage::Upload)
                }) else {
                    return;
                };
                current.picker = None;
                if let Ok(Ok(Some(sources))) = result {
                    view.zmodem_control(
                        ZmodemControl::PrepareUpload {
                            transfer: id,
                            sources,
                        },
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose_zmodem_destination(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self
            .zmodem
            .current
            .as_mut()
            .filter(|current| current.id == id && current.picker.is_none())
        else {
            return;
        };
        let Stage::Download { offer, name } = &current.stage else {
            return;
        };
        let offer = *offer;
        let name = name.clone();
        let picker = Uuid::new_v4();
        current.picker = Some(picker);
        let directory = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" }));
        let choice = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let result = choice.await;
            let _ = this.update_in(cx, |view, _, cx| {
                let Some(current) = view.zmodem.current.as_mut().filter(|current| {
                    current.id == id && current.picker == Some(picker)
                        && matches!(current.stage, Stage::Download { offer: current_offer, .. } if current_offer == offer)
                }) else { return; };
                current.picker = None;
                if let Ok(Ok(Some(destination))) = result {
                    view.zmodem_control(ZmodemControl::PrepareReceive { transfer: id, offer, destination }, cx);
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn acknowledge_zmodem_outcomes(&mut self, cx: &mut Context<Self>) {
        let epoch = self.zmodem.seen_uncertain_epoch;
        let ids: Vec<_> = self.zmodem.unresolved.keys().copied().collect();
        if self.zmodem_input_blocked()
            || self
                .zmodem
                .safety
                .publication_running
                .load(Ordering::Acquire)
            || self.zmodem.safety.uncertain_epoch.load(Ordering::Acquire) != epoch
        {
            self.zmodem.acknowledge_review = None;
            return;
        }
        if self.zmodem.acknowledge_review.as_ref() == Some(&(epoch, ids.clone())) {
            // This acknowledges only the displayed uncertainty for window
            // closing; it performs no filesystem cleanup or reservation reset.
            self.zmodem
                .safety
                .acknowledged_epoch
                .store(epoch, Ordering::Release);
            self.zmodem.unresolved.clear();
            self.zmodem.acknowledge_review = None;
        } else {
            self.zmodem.acknowledge_review = Some((epoch, ids));
        }
        cx.notify();
    }

    pub(super) fn zmodem_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.zmodem.current.is_none() && self.zmodem.unresolved.is_empty() {
            return None;
        }
        let visual = crate::design::palette(cx);
        let mut body = div()
            .id("zmodem-review-body")
            .test_support()
            .max_h(px(190.))
            .overflow_y_scroll()
            .text_xs()
            .flex()
            .flex_col()
            .gap_2();
        let mut actions = div()
            .id("zmodem-review-actions")
            .test_support()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2();
        let local_supported = cfg!(any(target_os = "linux", target_os = "macos"));
        if !local_supported {
            body = body.child(div().whitespace_normal().child(t(cx,
                "当前平台的 Zmodem 本地文件安全后端尚未完成，可以取消本次协议请求。",
                "This platform's safe local Zmodem file backend is pending. Cancel this protocol request.",
            )));
        }
        if let Some(current) = &self.zmodem.current {
            let id = current.id;
            body = body
                .child(div().whitespace_normal().child(current.summary.render(cx)))
                .child(
                    div()
                        .id("zmodem-feedback")
                        .aria_label(current.feedback.render(cx))
                        .child(current.feedback.render(cx)),
                );
            let picker_pending = current.picker.is_some();
            actions = match &current.stage {
                Stage::Upload => actions.child(
                    Button::new("zmodem-choose-upload")
                        .small()
                        .primary()
                        .label(t(cx, "选择上传文件", "Choose upload files"))
                        .disabled(picker_pending || !local_supported)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.choose_zmodem_sources(id, window, cx)
                        })),
                ),
                Stage::Download { .. } => actions.child(
                    Button::new("zmodem-choose-destination")
                        .small()
                        .primary()
                        .label(t(cx, "选择新本地文件", "Choose new local file"))
                        .disabled(picker_pending || !local_supported)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.choose_zmodem_destination(id, window, cx)
                        })),
                ),
                Stage::Review { review } => {
                    let review = *review;
                    actions.child(
                        Button::new("zmodem-confirm")
                            .small()
                            .primary()
                            .label(t(cx, "确认本次传输", "Confirm this transfer"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.zmodem_control(
                                    ZmodemControl::Confirm {
                                        transfer: id,
                                        review,
                                    },
                                    cx,
                                )
                            })),
                    )
                }
                Stage::Running | Stage::Cancelling => actions,
                Stage::Ended => actions.child(
                    Button::new("zmodem-dismiss")
                        .small()
                        .ghost()
                        .label(t(cx, "收起回执", "Hide receipt"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.zmodem.current.as_ref().is_some_and(|current| {
                                current.id == id && matches!(current.stage, Stage::Ended)
                            }) {
                                view.zmodem.current = None;
                            }
                            cx.notify();
                        })),
                ),
            };
            if !matches!(current.stage, Stage::Ended) {
                actions = actions.child(
                    Button::new("zmodem-cancel")
                        .small()
                        .ghost()
                        .label(t(cx, "取消 Zmodem", "Cancel Zmodem"))
                        .disabled(matches!(current.stage, Stage::Cancelling))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.zmodem_control(ZmodemControl::Cancel { transfer: id }, cx)
                        })),
                );
            }
        }
        if !self.zmodem.verified_saves.is_empty() {
            body = body.child(div().font_semibold().child(t(
                cx,
                "已验证保存的文件（保留最近 32 条）",
                "Verified saved files (latest 32 retained)",
            )));
            for (index, saved) in self.zmodem.verified_saves.iter().enumerate() {
                let summary = saved.summary().render(cx);
                body = body.child(
                    div()
                        .id(("zmodem-verified-save", index))
                        .test_support()
                        .aria_label(summary.clone())
                        .flex_shrink_0()
                        .whitespace_normal()
                        .child(summary),
                );
            }
        }
        for summary in self.zmodem.unresolved.values() {
            body = body.child(
                div()
                    .text_color(rgb(visual.warning))
                    .whitespace_normal()
                    .child(summary.render(cx)),
            );
        }
        if !self.zmodem.unresolved.is_empty() {
            let reviewing = self.zmodem.acknowledge_review.is_some();
            if reviewing {
                body = body.child(t(
                    cx,
                    "请检查以上每个原始目标和临时文件。再次确认仅允许关闭窗口，不代表传输成功、回滚、清理或解除文件隔离。",
                    "Inspect every original destination and staging file above. Confirming again permits window closing only; it does not establish success, rollback, cleanup or removal of file quarantine.",
                ));
            }
            actions = actions.child(
                Button::new("zmodem-acknowledge-unknown")
                    .small()
                    .ghost()
                    .label(if reviewing {
                        t(
                            cx,
                            "已核对，确认未知结果",
                            "Inspected: acknowledge unknown outcomes",
                        )
                    } else {
                        t(cx, "核对未知结果", "Review unknown outcomes")
                    })
                    .disabled(self.zmodem_input_blocked())
                    .on_click(cx.listener(|view, _, _, cx| view.acknowledge_zmodem_outcomes(cx))),
            );
        }
        Some(
            div()
                .id("zmodem-transfer")
                .test_support()
                .occlude()
                .capture_any_mouse_down(
                    cx.listener(|view, _, _, cx| view.publish_input_selection(cx)),
                )
                .track_focus(&self.zmodem_focus)
                .absolute()
                .bottom_2()
                .left_2()
                .right_2()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(rgb(visual.border))
                .bg(rgb(visual.surface))
                .text_color(rgb(visual.text))
                .flex()
                .flex_col()
                .gap_2()
                .child(div().font_semibold().text_sm().child("Zmodem"))
                .child(body)
                .child(actions)
                .into_any_element(),
        )
    }
}

fn zmodem_failure(reason: ZmodemFailure) -> Message {
    let (zh, en) = match reason {
        ZmodemFailure::Stale => (
            "传输、审核或连接已变化，请重新准备。",
            "The transfer, review or connection changed. Prepare again.",
        ),
        ZmodemFailure::InvalidPath => (
            "文件或目标路径不安全，操作已拒绝。",
            "The file or destination path is unsafe. The action was rejected.",
        ),
        ZmodemFailure::SourceChanged => (
            "本地源已变化，旧审核已失效。",
            "The local source changed. The old review is invalid.",
        ),
        ZmodemFailure::Limit => (
            "Zmodem 的文件、大小或协议预算已达到上限。",
            "A Zmodem file, size or protocol budget was exceeded.",
        ),
        ZmodemFailure::Protocol => (
            "对端协议无效或请求了不允许的操作。",
            "The peer sent an invalid or disallowed protocol sequence.",
        ),
        ZmodemFailure::PeerCancelled => (
            "远端取消了 Zmodem；请检查已接收的数据。",
            "The peer cancelled Zmodem. Inspect any received data.",
        ),
        ZmodemFailure::Timeout => (
            "Zmodem 等待超时，传输结果需要检查。",
            "Zmodem timed out. Inspect the transfer outcome.",
        ),
        ZmodemFailure::DestinationBusy => (
            "另一个任务正在使用本地目标，请稍后重新审核。",
            "Another task owns the local destination. Review again later.",
        ),
        ZmodemFailure::DestinationQuarantined => (
            "本地目标被未知操作隔离，请先在文件面板检查。",
            "An unknown operation quarantines this local destination. Inspect it in the Files panel.",
        ),
        ZmodemFailure::DestinationExists => (
            "本地目标已经存在，原文件已保留。",
            "The local destination already exists. It was preserved.",
        ),
        ZmodemFailure::Unsupported => (
            "当前平台尚无已核验的安全发布后端。",
            "This platform lacks a verified safe publication backend.",
        ),
        ZmodemFailure::LocalIo => (
            "本地文件操作失败，请检查路径和权限。",
            "A local file operation failed. Check paths and permissions.",
        ),
        ZmodemFailure::UnverifiedPublication => (
            "本地发布结果未能核验，请检查原目标和临时文件。",
            "Local publication could not be verified. Inspect the original target and staging file.",
        ),
        ZmodemFailure::ConnectionClosed => (
            "原 SSH 连接已关闭，旧传输不能用于新连接。",
            "The original SSH connection closed. This transfer cannot be reused by a new connection.",
        ),
    };
    Message::new(zh, en)
}

#[cfg(test)]
#[path = "terminal_zmodem_tests.rs"]
mod tests;
