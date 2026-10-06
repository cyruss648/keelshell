//! Ephemeral task editor, complete human review and captured SSH dependency run.
use std::{collections::HashSet, sync::Arc, time::Duration};

use gpui_kit::{
    component::input::{InputEvent, InputState, TextareaState},
    *,
};
use keelshell_core::{BatchParameterizedTemplate, BatchTaskSpec, BatchWorkflowPlan};
use keelshell_session::{
    BatchPolicy, SshSession, WorkflowBinding, WorkflowEvent, WorkflowHandle, WorkflowOptions,
    WorkflowReceipt, WorkflowTaskReceipt, WorkflowTaskResult,
};
use uuid::Uuid;

use crate::{
    batch_commands::Destination,
    i18n::{Message, t},
};

mod schedule;
#[cfg(test)]
mod tests;
mod view;

/// Owned authenticated instance; display metadata alone is never execution authority.
pub(crate) struct ConnectedDestination {
    pub destination: Destination,
    pub session: SshSession,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct WorkflowReview {
    token: Uuid,
    revision: u64,
    pub plan: BatchWorkflowPlan,
    pub destinations: Vec<Destination>,
    pub sources: Vec<(Uuid, String)>,
    pub labels: Vec<(Uuid, String)>,
    pub options: WorkflowOptions,
    pub schedule: Option<keelshell_core::WorkflowScheduleSpec>,
    schedule_inputs: schedule::ScheduleReviewInputs,
}

pub(crate) enum WorkflowPanelEvent {
    Start(Box<WorkflowReview>),
    Hide,
    New,
    RefreshTargets,
}

struct TargetRow {
    connected: ConnectedDestination,
    available: bool,
    parameters: Vec<crate::target_parameters::ParameterField>,
}
struct TaskDraft {
    id: Uuid,
    number: usize,
    name: Entity<InputState>,
    command: Entity<TextareaState>,
    target: Option<Uuid>,
    dependencies: Vec<Uuid>,
    _subscriptions: Vec<Subscription>,
}
struct TaskProgress {
    id: Uuid,
    admitted: bool,
    receipt: Option<Arc<WorkflowTaskReceipt>>,
}

pub(crate) struct WorkflowPanel {
    targets: Vec<TargetRow>,
    tasks: Vec<TaskDraft>,
    next_number: usize,
    selected: Option<Uuid>,
    concurrency: Entity<InputState>,
    timeout: Entity<InputState>,
    stop_after_failure: bool,
    revision: u64,
    review: Option<WorkflowReview>,
    starting: bool,
    handle: Option<WorkflowHandle>,
    run_id: Option<Uuid>,
    progress: Vec<TaskProgress>,
    complete: bool,
    cancelling: bool,
    detail: Option<Uuid>,
    detail_text: SharedString,
    message: Option<Message>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    _poll: Option<Task<()>>,
    schedule_draft: schedule::ScheduleDraft,
    scheduled: Option<schedule::ScheduledRun>,
    _schedule_poll: Option<Task<()>>,
}

fn input(value: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut field = InputState::new(window, cx);
        field.set_value(value, window, cx);
        field
    })
}

impl WorkflowPanel {
    pub(crate) fn new(
        destinations: Vec<ConnectedDestination>,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let concurrency = input("2", window, cx);
        let timeout = input("30", window, cx);
        let subscriptions = [&concurrency, &timeout]
            .into_iter()
            .map(|field| {
                cx.subscribe(field, |panel, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        panel.changed(cx);
                    }
                })
            })
            .collect();
        let schedule_draft = schedule::ScheduleDraft::new(window, cx);
        let mut panel = Self {
            targets: destinations
                .into_iter()
                .map(|connected| TargetRow {
                    connected,
                    available: true,
                    parameters: Vec::new(),
                })
                .collect(),
            tasks: Vec::new(),
            next_number: 1,
            selected: None,
            concurrency,
            timeout,
            stop_after_failure: true,
            revision: 0,
            review: None,
            starting: false,
            handle: None,
            run_id: None,
            progress: Vec::new(),
            complete: false,
            cancelling: false,
            detail: None,
            detail_text: "".into(),
            message: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
            _poll: None,
            schedule_draft,
            scheduled: None,
            _schedule_poll: None,
        };
        panel.add_task(text, window, cx);
        panel
    }

    pub(crate) fn is_running(&self) -> bool {
        self.starting || self.handle.is_some() || self.schedule_active()
    }
    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }
    fn editable(&self) -> bool {
        self.review.is_none() && !self.is_running() && !self.complete
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        for row in &mut self.targets {
            for field in &mut row.parameters {
                if !field.value.read(cx).value().is_empty() {
                    field.allow_empty = false;
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
        if !self.is_running() && !self.complete {
            let expired = self.review.take().is_some();
            self.set_disabled(false, cx);
            if expired {
                self.message = Some(Message::new(
                    "内容已变化，请重新审核。",
                    "Content changed; review again.",
                ));
            } else {
                self.message = None;
            }
        }
        cx.notify();
    }

    fn add_task(&mut self, command: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || self.tasks.len() >= 128 {
            return;
        }
        let id = Uuid::new_v4();
        let name = input("", window, cx);
        let command = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx).rows(5);
            input.set_value(command, window, cx);
            input
        });
        let subscriptions = vec![
            cx.subscribe(&name, |panel, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    panel.changed(cx);
                }
            }),
            cx.subscribe(&command, |panel, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    panel.changed(cx);
                }
            }),
        ];
        self.tasks.push(TaskDraft {
            id,
            number: self.next_number,
            name,
            command,
            target: None,
            dependencies: Vec::new(),
            _subscriptions: subscriptions,
        });
        self.next_number = self.next_number.saturating_add(1);
        self.selected = Some(id);
        self.changed(cx);
    }

    fn remove_task(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        self.tasks.retain(|task| task.id != id);
        let mut removed_edges = false;
        for task in &mut self.tasks {
            let before = task.dependencies.len();
            task.dependencies.retain(|dependency| *dependency != id);
            removed_edges |= before != task.dependencies.len();
        }
        if self.selected == Some(id) {
            self.selected = self.tasks.first().map(|task| task.id);
        }
        self.changed(cx);
        if removed_edges {
            self.message = Some(Message::new(
                "已删除任务及其依赖边；执行前请核对完整依赖计划。",
                "Task and its dependency edges removed; review the complete dependency plan before execution.",
            ));
        }
    }

    fn label(&self, id: Uuid, cx: &App) -> String {
        self.tasks
            .iter()
            .find(|task| task.id == id)
            .map(|task| {
                let name = task.name.read(cx).value();
                if name.trim().is_empty() {
                    format!("{} {}", t(cx, "任务", "Task"), task.number)
                } else {
                    format!(
                        "#{} · {}",
                        task.number,
                        crate::command_text::visible_command(name.as_str())
                    )
                }
            })
            .unwrap_or_else(|| id.to_string())
    }

    fn toggle_parameter_empty(&mut self, target: Uuid, index: usize, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if let Some(row) = self
            .targets
            .iter_mut()
            .find(|row| row.connected.destination.id == target)
            && let Some(field) = row.parameters.get_mut(index)
            && field.value.read(cx).value().is_empty()
        {
            field.allow_empty = !field.allow_empty;
            self.changed(cx);
        }
    }

    fn sync_parameters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let mut required = std::collections::BTreeMap::<Uuid, Vec<String>>::new();
        for task in &self.tasks {
            let Some(target) = task.target else {
                continue;
            };
            let names =
                match crate::target_parameters::names(task.command.read(cx).value().as_str()) {
                    Ok(names) => names,
                    Err(error) => {
                        self.message = Some(error);
                        cx.notify();
                        return;
                    }
                };
            let all = required.entry(target).or_default();
            for name in names {
                if !all.contains(&name) {
                    all.push(name);
                }
            }
            if all.len() > 32 {
                self.message = Some(Message::new(
                    "每个目标最多 32 个用户参数。",
                    "At most 32 user parameters per target.",
                ));
                cx.notify();
                return;
            }
        }
        for row in &mut self.targets {
            if !row.available {
                continue;
            }
            let names = required
                .get(&row.connected.destination.id)
                .map_or(&[][..], Vec::as_slice);
            crate::target_parameters::synchronize(&mut row.parameters, names, window, cx);
            for field in &mut row.parameters {
                if field.subscription.is_none() {
                    field.subscription = Some(cx.subscribe(
                        &field.value,
                        |panel, _, event: &InputEvent, cx| {
                            if matches!(event, InputEvent::Change) {
                                panel.changed(cx);
                            }
                        },
                    ));
                }
            }
        }
        self.changed(cx);
    }

    fn snapshot(&self, token: Uuid, cx: &App) -> Result<WorkflowReview, Message> {
        let concurrency = self
            .concurrency
            .read(cx)
            .value()
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=8).contains(n));
        let timeout = self
            .timeout
            .read(cx)
            .value()
            .parse::<u64>()
            .ok()
            .filter(|n| (1..=300).contains(n));
        let (Some(concurrency), Some(timeout)) = (concurrency, timeout) else {
            return Err(Message::new(
                "并发必须为 1–8，单任务超时必须为 1–300 秒。",
                "Concurrency must be 1–8; per-task timeout must be 1–300 seconds.",
            ));
        };
        let mut sources = Vec::new();
        let mut labels = Vec::new();
        let mut specs = Vec::new();
        let mut source_bytes = 0_usize;
        let mut target_names = std::collections::BTreeMap::<Uuid, Vec<String>>::new();
        for task in &self.tasks {
            if let Some(target) = task.target {
                let names =
                    crate::target_parameters::names(task.command.read(cx).value().as_str())?;
                let all = target_names.entry(target).or_default();
                for name in names {
                    if !all.contains(&name) {
                        all.push(name);
                    }
                }
            }
        }
        let mut mappings = std::collections::BTreeMap::new();
        let mut value_bytes = 0_usize;
        for (id, names) in &target_names {
            if names.len() > 32 {
                return Err(Message::new(
                    "每个目标最多 32 个用户参数。",
                    "At most 32 user parameters per target.",
                ));
            }
            if let Some(row) = self
                .targets
                .iter()
                .find(|row| row.connected.destination.id == *id)
            {
                let values = crate::target_parameters::values(&row.parameters, cx)?;
                values
                    .validate_names(names)
                    .map_err(|error| crate::target_parameters::error_message(&error))?;
                value_bytes = value_bytes.saturating_add(values.byte_len());
                if value_bytes > 1024 * 1024 {
                    return Err(Message::new(
                        "目标参数总量超过 1 MiB。",
                        "Target parameter values exceed 1 MiB.",
                    ));
                }
                mappings.insert(*id, values);
            }
        }
        for task in &self.tasks {
            let name = task.name.read(cx).value().to_string();
            if name.len() > 256 || name.chars().any(char::is_control) {
                return Err(Message::new(
                    "任务名称最多 256 字节，不含控制字符。",
                    "Task names allow up to 256 bytes without controls.",
                ));
            }
            let Some(target) = self.targets.iter().find(|row| {
                Some(row.connected.destination.id) == task.target
                    && row.available
                    && !row.connected.session.is_closed()
            }) else {
                return Err(Message::new(
                    format!(
                        "{}：请选择仍在线的已认证 SSH 会话。",
                        self.label(task.id, cx)
                    ),
                    format!(
                        "{}: select an authenticated SSH session that is still connected.",
                        self.label(task.id, cx)
                    ),
                ));
            };
            let source = task.command.read(cx).value().to_string();
            source_bytes = source_bytes.saturating_add(source.len());
            if source_bytes > 1024 * 1024 {
                return Err(Message::new(
                    "命令草稿总量不能超过 1 MiB。",
                    "Total command drafts cannot exceed 1 MiB.",
                ));
            }
            let template = BatchParameterizedTemplate::compile(&source)
                .map_err(|error| crate::target_parameters::error_message(&error))?;
            let command = if let Some(template) = template {
                let values = mappings
                    .get(&target.connected.destination.id)
                    .ok_or_else(|| {
                        Message::new("缺少目标参数映射。", "Target parameter mapping is missing.")
                    })?;
                let task_values = values
                    .for_task(template.parameters())
                    .map_err(|error| crate::target_parameters::error_message(&error))?;
                template
                    .render(&target.connected.destination.template_context, &task_values)
                    .map_err(|error| crate::target_parameters::error_message(&error))?
            } else {
                source.clone()
            };
            specs.push(BatchTaskSpec {
                id: task.id,
                target_id: target.connected.destination.id,
                command,
                dependencies: task.dependencies.clone(),
            });
            sources.push((task.id, source));
            labels.push((task.id, name));
        }
        let plan = BatchWorkflowPlan::new(specs).map_err(|error| {
            Message::detail(
                "依赖计划无效（任务、命令、依赖或循环）",
                "Invalid dependency plan (tasks, commands, dependencies or cycle)",
                error,
            )
        })?;
        let targets: HashSet<_> = plan.tasks().iter().map(|task| task.target_id).collect();
        let destinations = self
            .targets
            .iter()
            .filter(|row| targets.contains(&row.connected.destination.id))
            .map(|row| row.connected.destination.clone())
            .collect();
        Ok(WorkflowReview {
            token,
            revision: self.revision,
            plan: plan.clone(),
            destinations,
            sources,
            labels,
            schedule: self.schedule_snapshot(token, &plan, cx)?,
            schedule_inputs: self.schedule_draft.review_inputs(cx),
            options: WorkflowOptions {
                concurrency,
                timeout: Duration::from_secs(timeout),
                output_limit: 256 * 1024,
                policy: if self.stop_after_failure {
                    BatchPolicy::StopAfterFailure
                } else {
                    BatchPolicy::Continue
                },
            },
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
    #[cfg(test)]
    pub(crate) fn reviewed_for_test(&self) -> Option<WorkflowReview> {
        self.review.clone()
    }

    #[cfg(test)]
    pub(crate) fn parameter_for_test(
        &self,
        target: Uuid,
        name: &str,
    ) -> Option<Entity<TextareaState>> {
        self.targets
            .iter()
            .find(|row| row.connected.destination.id == target)?
            .parameters
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.clone())
    }

    pub(crate) fn review_current(&self, review: &WorkflowReview, cx: &App) -> bool {
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
        cx.emit(WorkflowPanelEvent::Start(Box::new(review)));
        cx.notify();
    }
    pub(crate) fn fail_start(&mut self, cx: &mut Context<Self>) {
        self.stop_schedule(
            keelshell_core::WorkflowScheduleInvalidationReason::SessionBindingChanged,
            cx,
        );
        self.starting = false;
        self.review = None;
        self.set_disabled(false, cx);
        self.message = Some(Message::new(
            "审核已失效：任务、参数、定时配置、执行选项或 SSH 会话变化。请重新选择并审核。",
            "Review expired: task, parameter, schedule, execution options or SSH session changed. Select and review again.",
        ));
        cx.notify();
    }
    pub(crate) fn connected_destinations(&self) -> impl Iterator<Item = &ConnectedDestination> {
        self.targets.iter().map(|row| &row.connected)
    }
    pub(crate) fn begin(
        &mut self,
        review: &WorkflowReview,
        runtime: &tokio::runtime::Runtime,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.starting || !self.review_current(review, cx) {
            self.fail_start(cx);
            return;
        }
        if review.schedule.is_some() && !self.admit_schedule(review, window, cx) {
            return;
        }
        let result = review.plan.clone().confirm(review.plan.review_token());
        let result = result
            .map_err(|error| Message::detail("计划确认失败", "Plan confirmation failed", error))
            .and_then(|confirmed| {
                let bindings = self
                    .targets
                    .iter()
                    .filter(|row| {
                        review
                            .destinations
                            .iter()
                            .any(|target| target.id == row.connected.destination.id)
                    })
                    .map(|row| WorkflowBinding {
                        id: row.connected.destination.id,
                        session: row.connected.session.clone(),
                    })
                    .collect();
                let _runtime = runtime.enter();
                keelshell_session::start_workflow(confirmed, bindings, review.options).map_err(
                    |error| Message::detail("工作流未启动", "Workflow did not start", error),
                )
            });
        self.starting = false;
        match result {
            Ok(handle) => {
                self.progress = review
                    .plan
                    .tasks()
                    .iter()
                    .map(|task| TaskProgress {
                        id: task.id,
                        admitted: false,
                        receipt: None,
                    })
                    .collect();
                self.handle = Some(handle);
                let run_id = Uuid::new_v4();
                self.run_id = Some(run_id);
                self.message = None;
                self._poll = Some(cx.spawn_in(window, async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(32))
                            .await;
                        if !this
                            .update_in(cx, |panel, _, cx| panel.poll(run_id, cx))
                            .unwrap_or(false)
                        {
                            break;
                        }
                    }
                }));
            }
            Err(error) => {
                self.complete =
                    self.finish_schedule(keelshell_core::WorkflowScheduleOutcome::Failed);
                self.stop_schedule(
                    keelshell_core::WorkflowScheduleInvalidationReason::SessionBindingChanged,
                    cx,
                );
                self.review = None;
                self.set_disabled(false, cx);
                self.message = Some(error);
            }
        }
        cx.notify();
    }

    fn poll(&mut self, run_id: Uuid, cx: &mut Context<Self>) -> bool {
        if self.run_id != Some(run_id) {
            return false;
        }
        let Some(handle) = self.handle.as_mut() else {
            return false;
        };
        let mut changed = false;
        while let Ok(event) = handle.try_recv() {
            match event {
                WorkflowEvent::Started { id, .. } => {
                    if let Some(progress) = self.progress.iter_mut().find(|row| row.id == id) {
                        progress.admitted = true;
                    }
                }
                WorkflowEvent::Finished { task } => {
                    if let Some(progress) = self.progress.iter_mut().find(|row| row.id == task.id) {
                        progress.receipt = Some(task);
                    }
                }
            }
            changed = true;
        }
        if let Some(result) = handle.try_finish() {
            // Only a matching complete receipt can distinguish known failure from
            // uncertain execution. Either stops repetition; neither is retried.
            let scheduled_outcome = match result.as_ref() {
                Ok(receipt) if self.receipt_matches(receipt) => {
                    if receipt.cancelled || self.cancelling {
                        keelshell_core::WorkflowScheduleOutcome::Cancelled
                    } else if receipt.tasks.iter().any(|task| matches!(
                        &task.result, WorkflowTaskResult::Transport { row }
                            if matches!(row.outcome, keelshell_session::BatchOutcome::Unknown { .. })
                    )) {
                        keelshell_core::WorkflowScheduleOutcome::Unknown
                    } else if receipt.tasks.iter().all(|task| matches!(
                        &task.result, WorkflowTaskResult::Transport { row } if row.outcome.is_success()
                    )) {
                        keelshell_core::WorkflowScheduleOutcome::Succeeded
                    } else {
                        keelshell_core::WorkflowScheduleOutcome::Failed
                    }
                }
                _ => keelshell_core::WorkflowScheduleOutcome::Unknown,
            };
            match result {
                Ok(receipt) if self.receipt_matches(&receipt) => {
                    self.cancelling |= receipt.cancelled;
                    for task in receipt.tasks {
                        if let Some(progress) =
                            self.progress.iter_mut().find(|row| row.id == task.id)
                        {
                            progress.receipt = Some(task);
                        }
                    }
                }
                _ => {
                    self.message = Some(Message::new(
                        "完整执行回执缺失或不匹配；缺失结果未知，不会自动重试。",
                        "Complete execution receipt is missing or mismatched; missing outcomes are unknown and will not be retried automatically.",
                    ))
                }
            }
            self.handle = None;
            self.complete = self.finish_schedule(scheduled_outcome);
            changed = true;
        }
        if changed {
            if self.detail.is_none() {
                self.detail = self
                    .progress
                    .iter()
                    .find(|row| row.receipt.is_some())
                    .map(|row| row.id);
            }
            self.refresh_detail(cx);
            cx.notify();
        }
        self.handle.is_some()
    }
    fn receipt_matches(&self, receipt: &WorkflowReceipt) -> bool {
        self.review.as_ref().is_some_and(|review| {
            receipt.fingerprint == review.plan.review_token()
                && receipt.options == review.options
                && receipt.tasks.len() == review.plan.tasks().len()
                && receipt
                    .tasks
                    .iter()
                    .zip(review.plan.tasks())
                    .all(|(receipt, task)| {
                        receipt.id == task.id && receipt.target_id == task.target_id
                    })
        })
    }

    /// Never replace captured connections. Changed instances become unavailable.
    pub(crate) fn update_available(
        &mut self,
        current: &[ConnectedDestination],
        cx: &mut Context<Self>,
    ) {
        let used: HashSet<_> = self.tasks.iter().filter_map(|task| task.target).collect();
        let mut lost = false;
        let mut changed = false;
        for row in &mut self.targets {
            if row.available
                && !current.iter().any(|live| {
                    live.destination.entity == row.connected.destination.entity
                        && live.destination.name == row.connected.destination.name
                        && live.destination.profile_id == row.connected.destination.profile_id
                        && live.destination.endpoint == row.connected.destination.endpoint
                        && live.destination.route == row.connected.destination.route
                        && live.destination.template_context
                            == row.connected.destination.template_context
                        && live.session.same_connection(&row.connected.session)
                        && !live.session.is_closed()
                })
            {
                row.available = false;
                row.parameters.clear();
                changed = true;
                lost |= used.contains(&row.connected.destination.id);
            }
        }
        if lost {
            // Invalidation marks a waiting schedule complete. Capture whether
            // this was still an unclaimed review before changing that state,
            // so a lost target cannot leave an actionable-looking old review.
            let waiting_review = self.handle.is_none() && !self.complete;
            self.stop_schedule(
                keelshell_core::WorkflowScheduleInvalidationReason::SessionBindingChanged,
                cx,
            );
            if let Some(handle) = &self.handle {
                handle.cancel();
                self.cancelling = true;
            } else if waiting_review {
                self.starting = false;
                self.review = None;
                self.set_disabled(false, cx);
                self.revision = self.revision.wrapping_add(1);
            }
            self.message = Some(Message::new(
                "所选 SSH 会话或目标资料变化。等待任务停止放行；已开始任务按回执判断，取消不证明远端停止。",
                "A selected SSH session or target metadata changed. Pending admission stops; started tasks retain their receipts. Cancellation does not prove remote termination.",
            ));
        }
        if changed {
            cx.notify();
        }
    }
    pub(crate) fn refresh_targets(
        &mut self,
        current: Vec<ConnectedDestination>,
        cx: &mut Context<Self>,
    ) {
        self.update_available(&current, cx);
        if !self.editable() {
            return;
        }
        for connected in current {
            if !self.targets.iter().any(|row| {
                row.available
                    && row.connected.destination.entity == connected.destination.entity
                    && row.connected.session.same_connection(&connected.session)
            }) {
                self.targets.push(TargetRow {
                    connected,
                    available: true,
                    parameters: Vec::new(),
                });
            }
        }
        // Retain unavailable selected bindings for explicit re-selection, but do
        // not accumulate stale unselected identities after repeated refreshes.
        let used: HashSet<_> = self.tasks.iter().filter_map(|task| task.target).collect();
        self.targets
            .retain(|row| row.available || used.contains(&row.connected.destination.id));
        self.changed(cx);
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.cancel_schedule(cx);
        if let Some(handle) = &self.handle {
            handle.cancel();
            self.cancelling = true;
            cx.notify();
        }
    }
    fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        for row in &self.targets {
            for field in &row.parameters {
                field
                    .value
                    .update(cx, |input, cx| input.set_disabled(disabled, cx));
            }
        }
        for task in &self.tasks {
            task.name
                .update(cx, |field, cx| field.set_disabled(disabled, cx));
            task.command
                .update(cx, |field, cx| field.set_disabled(disabled, cx));
        }
        self.schedule_draft.set_disabled(disabled, cx);
        for field in [&self.concurrency, &self.timeout] {
            field.update(cx, |field, cx| field.set_disabled(disabled, cx));
        }
    }
    fn back(&mut self, cx: &mut Context<Self>) {
        if self.is_running() || self.complete {
            return;
        }
        self.review = None;
        self.set_disabled(false, cx);
        self.message = None;
        cx.notify();
    }
    pub(crate) fn refresh_locale(&mut self, cx: &mut Context<Self>) {
        self.refresh_detail(cx);
        cx.notify();
    }
    fn refresh_detail(&mut self, cx: &App) {
        self.detail_text = self
            .detail
            .and_then(|id| self.progress.iter().find(|row| row.id == id))
            .and_then(|row| row.receipt.as_ref())
            .map(|receipt| match &receipt.result {
                WorkflowTaskResult::Transport { row } => format!(
                    "stdout\n{}\n\nstderr\n{}",
                    crate::command_text::visible_command(&String::from_utf8_lossy(&row.stdout)),
                    crate::command_text::visible_command(&String::from_utf8_lossy(&row.stderr))
                ),
                WorkflowTaskResult::Skipped { .. } => t(
                    cx,
                    "任务未放行，没有远端输出。",
                    "Task was not admitted; no remote output.",
                )
                .into(),
            })
            .unwrap_or_default()
            .into();
    }
}
impl EventEmitter<WorkflowPanelEvent> for WorkflowPanel {}
