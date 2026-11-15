//! Multiple reviewed jobs share one queue and the original authenticated session.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::component::{checkbox::Checkbox, scroll::ScrollableElement};
use keelshell_session::sftp::{MAX_QUEUED_TRANSFERS, TransferQueue};
use std::collections::VecDeque;

type RecoveryDiscardOutcome = (uuid::Uuid, Result<(), String>);

pub(super) struct QueuedTransfer {
    pub(super) id: uuid::Uuid,
    session_token: uuid::Uuid,
    pub(super) status: TransferStatus,
    result: Option<Message>,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Sender<bool>,
    recovery: RecoveryCandidate,
}

fn active(phase: TransferPhase) -> bool {
    !matches!(
        phase,
        TransferPhase::Completed
            | TransferPhase::Cancelled
            | TransferPhase::Failed
            | TransferPhase::Uncertain
    )
}

#[derive(Default, Debug, PartialEq, Eq)]
struct QueueSummary {
    active: usize,
    completed: usize,
    failed: usize,
    cancelled: usize,
    uncertain: usize,
}

impl QueueSummary {
    fn for_session(jobs: &[QueuedTransfer], session_token: uuid::Uuid) -> Self {
        let mut summary = Self::default();
        for job in jobs.iter().filter(|job| job.session_token == session_token) {
            match job.status.phase {
                TransferPhase::Completed => summary.completed += 1,
                TransferPhase::Failed => summary.failed += 1,
                TransferPhase::Cancelled => summary.cancelled += 1,
                TransferPhase::Uncertain => summary.uncertain += 1,
                _ => summary.active += 1,
            }
        }
        summary
    }
}

impl FilesPanel {
    fn set_queue_details_visible(
        &mut self,
        expected_session: uuid::Uuid,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        // A painted entry belongs to this panel's original SSH owner. Switching
        // surfaces changes neither browser drafts nor the pending approval.
        if self.session_token != expected_session {
            return;
        }
        if !visible && self.recovery_review_session.is_some() {
            self.cancel_recovery_review_queue(cx);
        }
        self.queue_details_visible = visible;
        cx.notify();
    }

    pub(super) fn queue_entry_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let summary = QueueSummary::for_session(&self.transfer_jobs, self.session_token);
        let mut counts = div()
            .id("file-transfer-summary-counts")
            .test_support()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(rgb(visual.muted))
                    .child(t(cx, "当前", "Current")),
            );
        for (id, label, count, color) in [
            (
                "active",
                t(cx, "活动", "Active"),
                summary.active,
                visual.accent,
            ),
            (
                "completed",
                t(cx, "完成", "Done"),
                summary.completed,
                visual.success,
            ),
            (
                "failed",
                t(cx, "失败", "Failed"),
                summary.failed,
                visual.danger,
            ),
            (
                "cancelled",
                t(cx, "取消", "Cancelled"),
                summary.cancelled,
                visual.muted,
            ),
            (
                "uncertain",
                t(cx, "未知", "Unknown"),
                summary.uncertain,
                visual.warning,
            ),
        ] {
            let text = SharedString::from(format!("{label} {count}"));
            counts = counts.child(
                div()
                    .id(SharedString::from(format!("file-transfer-summary-{id}")))
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_color(rgb(color))
                    .role(accesskit::Role::Label)
                    .aria_label(text.clone())
                    .child(text)
                    .test_support(),
            );
        }
        let expected_session = self.session_token;
        let show_details = !self.queue_details_visible;
        div()
            .id("file-transfer-summary")
            .h(px(30.))
            .flex_shrink_0()
            .min_w_0()
            .px_1()
            .flex()
            .items_center()
            .gap_2()
            .border_t_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .child(self.tool_section_button(action_view::ToolSection::File, cx))
            .child(self.tool_section_button(action_view::ToolSection::Transfer, cx))
            .child(self.batch_controls(cx))
            .child(
                Button::new("toggle-file-transfer-queue")
                    .flex_shrink_0()
                    .xsmall()
                    .compact()
                    .ghost()
                    .selected(self.queue_details_visible)
                    .toggled(self.queue_details_visible)
                    .label(if self.queue_details_visible {
                        t(cx, "返回文件", "Back to files")
                    } else {
                        t(cx, "传输队列", "Transfer queue")
                    })
                    .accessibility_label(t(
                        cx,
                        "切换传输队列详情；数量为当前 SSH 会话",
                        "Toggle transfer queue details; counts belong to the current SSH session",
                    ))
                    .localized_tooltip(
                        "数量仅属于当前 SSH 会话；详情保留本面板历史记录",
                        "Counts belong to the current SSH session; details retain this panel's history",
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.set_queue_details_visible(expected_session, show_details, cx);
                    })),
            )
            .child(counts)
            .test_support()
            .into_any_element()
    }

    pub(super) fn queue_details_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("file-transfer-queue-details")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.queue_details_scroll)
            .vertical_scrollbar(&self.queue_details_scroll)
            .child(self.parallel_queue_card(cx))
            .test_support()
            .into_any_element()
    }

    pub(super) fn batch_queue_has_capacity(&self, count: usize) -> bool {
        count <= MAX_QUEUED_TRANSFERS
            && self
                .transfer_jobs
                .iter()
                .filter(|job| active(job.status.phase))
                .count()
                + count
                <= MAX_QUEUED_TRANSFERS
    }
    pub(super) fn has_active_transfers(&self) -> bool {
        self.transfer_jobs
            .iter()
            .any(|job| active(job.status.phase))
    }
    pub(super) fn cancel_transfers(&mut self) {
        for job in &mut self.transfer_jobs {
            if active(job.status.phase) {
                job.stop.store(true, Ordering::Release);
                job.status.phase = TransferPhase::Cancelling;
            }
        }
        if let Some(queue) = self.transfer_queue.get() {
            queue.cancel_all();
        }
        self.transfer_queue = Arc::new(tokio::sync::OnceCell::new());
    }
    fn select_job(&mut self, id: uuid::Uuid) {
        let Some(job) = self.transfer_jobs.iter().find(|job| job.id == id) else {
            return;
        };
        self.selected_transfer = Some(id);
        self.transfer = Some(job.status.clone());
        if self.operation_id.is_none() {
            self.operation_stop = active(job.status.phase).then(|| job.stop.clone());
            self.transfer_pause = active(job.status.phase).then(|| job.pause.clone());
            self.recovery = (!self.suspended
                && job.session_token == self.session_token
                && job.status.phase == TransferPhase::Failed)
                .then(|| RecoveryCandidate {
                    persisted_id: job.recovery.persisted_id,
                    session_token: job.recovery.session_token,
                    spec: job.recovery.spec.clone(),
                    directory: job.recovery.directory,
                });
        }
    }
    pub(super) fn request_job_pause(
        &mut self,
        id: uuid::Uuid,
        paused: bool,
        cx: &mut Context<Self>,
    ) {
        if self.suspended {
            return;
        }
        let Some(job) = self
            .transfer_jobs
            .iter_mut()
            .find(|job| job.id == id && job.session_token == self.session_token)
        else {
            return;
        };
        if job.status.phase
            != if paused {
                TransferPhase::Running
            } else {
                TransferPhase::Paused
            }
        {
            return;
        }
        job.pause.send_replace(paused);
        job.status.phase = if paused {
            TransferPhase::Pausing
        } else {
            TransferPhase::Resuming
        };
        self.select_job(id);
        cx.notify();
    }
    pub(super) fn cancel_job(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        let Some(job) = self.transfer_jobs.iter_mut().find(|job| job.id == id) else {
            return;
        };
        if !active(job.status.phase) {
            return;
        }
        job.stop.store(true, Ordering::Release);
        job.status.phase = TransferPhase::Cancelling;
        self.select_job(id);
        self.status = Message::new(
            "正在取消，等待该任务的传输回执…",
            "Cancelling; waiting for this job's transport result…",
        );
        cx.notify();
    }
    pub(super) fn run_queued_transfer(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let persisted_id = operation.persisted_recovery_id();
        let recovery_claim = if let Some(id) = persisted_id {
            let claim = self.recovery_claim.take();
            if self.active_recovery_id != Some(id) {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "恢复审核已失效，请重新检查该传输记录。",
                    "The recovery review is stale; inspect the transfer record again.",
                );
                return;
            }
            let Some(claim) = claim else {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "无法确认恢复记录的独占审核权，已拒绝继续。",
                    "The recovery record is not exclusively claimed; continuation was refused.",
                );
                return;
            };
            if claim.id() != id {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "恢复记录审核权与目标不一致，已拒绝继续。",
                    "The recovery claim does not match the reviewed target; continuation was refused.",
                );
                return;
            }
            Some(claim)
        } else {
            None
        };
        let Some(session) = self.session.clone().filter(|_| !self.suspended) else {
            if persisted_id.is_some() {
                self.revoke_recovery_review();
            }
            return;
        };
        let Some(status) = operation.transfer_status() else {
            return;
        };
        let Some((spec, directory)) = operation.recovery_spec() else {
            return;
        };
        let recovery_id = persisted_id.unwrap_or_else(uuid::Uuid::new_v4);
        let persisted_progress = super::persisted_recovery_progress(&operation);
        let recovery_record = if let Some(id) = persisted_id {
            let Some(record) = self.recovery_records.iter().find(|record| record.id == id) else {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "重启后的传输记录已消失，请重新加载文件面板后再检查。",
                    "The post-restart transfer record is gone; reload the file panel before inspecting it again.",
                );
                cx.notify();
                return;
            };
            let direction = match spec.direction {
                TransferDirection::Upload => TransferRecoveryDirection::Upload,
                TransferDirection::Download => TransferRecoveryDirection::Download,
            };
            if record.direction != direction
                || record.local_path != spec.local
                || record.remote_path != spec.remote
                || record.directory != directory
            {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "恢复记录与当前审核目标不一致，已拒绝继续。",
                    "The recovery record does not match the reviewed target; continuation was refused.",
                );
                cx.notify();
                return;
            }
            if !recovery_connection_matches(record, self.connection_id) {
                self.revoke_recovery_review();
                self.status = Message::new(
                    "恢复记录绑定了另一条保存连接，已拒绝继续。",
                    "The recovery record belongs to another saved connection; continuation was refused.",
                );
                cx.notify();
                return;
            }
            let mut record = record.clone();
            if let Some((existing_bytes, total_bytes)) = persisted_progress
                && let Err(error) = super::update_persisted_recovery_progress(
                    &mut record,
                    existing_bytes,
                    total_bytes,
                )
            {
                self.revoke_recovery_review();
                self.status = Message::new(
                    format!("恢复计划进度无效：{error}"),
                    format!("The recovery plan progress is invalid: {error}"),
                );
                cx.notify();
                return;
            }
            record
        } else {
            match recovery_record_for(
                recovery_id,
                self.connection_id,
                &spec,
                directory,
                status.reviewed_existing,
            ) {
                Ok(record) => record,
                Err(error) => {
                    self.status = Message::new(
                        format!("无法创建传输恢复记录：{error}"),
                        format!("Unable to create transfer recovery metadata: {error}"),
                    );
                    cx.notify();
                    return;
                }
            }
        };
        if self
            .transfer_jobs
            .iter()
            .filter(|job| active(job.status.phase))
            .count()
            >= MAX_QUEUED_TRANSFERS
        {
            if let Some(recovery_id) = persisted_id {
                self.requeue_failed_recovery_review_item(recovery_id);
                self.recovery_claim = None;
                self.active_recovery_id = None;
            }
            self.status = Message::new(
                "传输队列已满（最多 32 个未结束任务），请等待或取消后再加入。",
                "Transfer queue is full (32 unfinished jobs); wait or cancel before adding another.",
            );
            cx.notify();
            return;
        }
        // Keep at most 32 completed records in addition to the bounded active
        // queue. Removing a record never drops an active handle or replays it.
        while self
            .transfer_jobs
            .iter()
            .filter(|job| !active(job.status.phase))
            .count()
            >= MAX_QUEUED_TRANSFERS
        {
            if let Some(index) = self
                .transfer_jobs
                .iter()
                .position(|job| !active(job.status.phase))
            {
                self.transfer_jobs.remove(index);
            }
        }
        let id = uuid::Uuid::new_v4();
        let session_token = self.session_token;
        let stop = Arc::new(AtomicBool::new(false));
        let (pause, pause_receiver) = tokio::sync::watch::channel(false);
        self.transfer_jobs.push(QueuedTransfer {
            id,
            session_token,
            status,
            result: None,
            stop: stop.clone(),
            pause,
            recovery: RecoveryCandidate {
                persisted_id: Some(recovery_id),
                session_token,
                spec,
                directory,
            },
        });
        self.select_job(id);
        self.pending = None;
        self.busy = true;
        self.active_recovery_id = Some(recovery_id);
        if self.recovery_review_active == Some(recovery_id) {
            self.recovery_review_active = None;
            if self.recovery_review_queue.is_empty() {
                self.recovery_review_session = None;
            }
        }
        self.status = Message::new(
            "已加入传输队列；可继续审核加入其它任务。",
            "Added to the transfer queue; review and add more jobs while it runs.",
        );
        let queue_cell = self.transfer_queue.clone();
        let parallelism = self.queue_parallelism.clone();
        let runtime = self.runtime.clone();
        let recovery_store = self.recovery_store.clone();
        let recovery_record_for_worker = recovery_record.clone();
        let recovery_claim_for_worker = recovery_claim;
        let worker_stop = stop.clone();
        let (sender, receiver) = mpsc::sync_channel(16);
        #[cfg(test)]
        let fixture_group = self.fixture_group.clone();
        if let Err(error) = crate::terminal::spawn_owned_transport_worker(
            cx.entity_id(),
            "keelshell-queued-sftp",
            stop,
            move || {
                let (result, recovery_warning) = runtime.block_on(async {
                    if worker_stop.load(Ordering::Acquire) {
                        return (Err(FileFailure::CancelledBeforeStart), None);
                    }
                    let mut recovery_writer = TransferRecoveryWriter::new(
                        recovery_store,
                        recovery_record_for_worker,
                        recovery_claim_for_worker,
                    );
                    if let Err(error) = recovery_writer.start() {
                        return (Err(error), None);
                    }
                    let queue = tokio::select! {
                        biased;
                        _ = cancellation(&worker_stop) => {
                            let result = Err(FileFailure::CancelledBeforeStart);
                            recovery_writer.finish(&result, 0, None);
                            let warning = recovery_writer
                                .take_error()
                                .map(super::recovery_finish_warning);
                            return (result, warning);
                        }
                        result = queue_cell.get_or_try_init(|| async {
                            let sftp = Arc::new(session.sftp().await?);
                            let queue = Arc::new(sftp.transfer_queue());
                            queue.set_parallelism(parallelism.load(Ordering::Acquire))?;
                            Ok::<_, SessionError>(queue)
                        }) => match result {
                            Ok(queue) => queue.clone(),
                            Err(error) => {
                                let result = Err(FileFailure::from(error));
                                recovery_writer.finish(&result, 0, None);
                                let warning = recovery_writer
                                    .take_error()
                                    .map(super::recovery_finish_warning);
                                return (result, warning);
                            }
                        },
                    };
                    #[cfg(test)]
                    if let Some(group) = &fixture_group {
                        group.retain_queue(&runtime, &queue);
                    }
                    if let Err(error) = queue.set_parallelism(parallelism.load(Ordering::Acquire)) {
                        let result = Err(FileFailure::from(error));
                        recovery_writer.finish(&result, 0, None);
                        let warning = recovery_writer
                            .take_error()
                            .map(super::recovery_finish_warning);
                        return (result, warning);
                    }
                    if worker_stop.load(Ordering::Acquire) {
                        let result = Err(FileFailure::CancelledBeforeStart);
                        recovery_writer.finish(&result, 0, None);
                        let warning = recovery_writer
                            .take_error()
                            .map(super::recovery_finish_warning);
                        return (result, warning);
                    }
                    let result = queued_operation(
                        &queue,
                        operation,
                        worker_stop,
                        pause_receiver,
                        &sender,
                        Some(&mut recovery_writer),
                    )
                    .await;
                    recovery_writer.finish(&result, 0, None);
                    let warning = recovery_writer
                        .take_error()
                        .map(super::recovery_finish_warning);
                    (result, warning)
                });
                let completion = result.as_ref().map(|_| ()).map_err(ToString::to_string);
                let _ = sender.send(WorkerMessage::Result {
                    result: Box::new(result),
                    recovery_warning,
                });
                #[cfg(test)]
                {
                    // Release the captured cell before the final group owner;
                    // cleanup then unwraps and joins the actual queue scheduler.
                    drop(queue_cell);
                    drop(fixture_group);
                }
                completion
            },
        ) {
            if let Some(job) = self.transfer_jobs.iter_mut().find(|job| job.id == id) {
                job.status.phase = TransferPhase::Failed;
                job.result = Some(Message::detail(
                    "无法启动传输工作线程",
                    "Unable to start transfer worker",
                    error,
                ));
            }
            self.busy = self.has_active_transfers();
            self.select_job(id);
            if persisted_id.is_some() {
                self.requeue_failed_recovery_review_item(recovery_id);
                self.recovery_claim = None;
                self.active_recovery_id = None;
            }
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let message = match receiver.try_recv() {
                    Ok(message) => Some(message),
                    Err(mpsc::TryRecvError::Disconnected) => Some(WorkerMessage::Result {
                        result: Box::new(Err(FileFailure::WorkerStopped)),
                        recovery_warning: None,
                    }),
                    Err(mpsc::TryRecvError::Empty) => None,
                };
                let terminal = matches!(message, Some(WorkerMessage::Result { .. }));
                if let Some(message) = message {
                    if this
                        .update_in(cx, |view, window, cx| {
                            let Some(job) = view
                                .transfer_jobs
                                .iter_mut()
                                .find(|job| job.id == id && job.session_token == session_token)
                            else {
                                return;
                            };
                            let succeeded = match &message {
                                WorkerMessage::Result { result, .. } => result.is_ok(),
                                _ => false,
                            };
                            match message {
                                WorkerMessage::Transfer(event) => job.status.update(event),
                                WorkerMessage::Progress(message) => job.result = Some(message),
                                WorkerMessage::Result {
                                    result,
                                    recovery_warning,
                                } => {
                                    if let Some(phase) = terminal_transfer_phase(true, &result) {
                                        job.status.phase = phase;
                                    }
                                    let message = match *result {
                                        Ok(Outcome::Done(message)) => message,
                                        Ok(_) => Message::new("传输已结束", "Transfer ended"),
                                        Err(error) => error.message(),
                                    };
                                    job.result = Some(append_recovery_warning(
                                        message,
                                        recovery_warning.as_ref(),
                                    ));
                                }
                            }
                            // A retired panel retains honest results, but late
                            // events cannot revive a proposal or target a new host.
                            if view.selected_transfer == Some(id) {
                                view.select_job(id);
                            }
                            view.busy = view.operation_id.is_some() || view.has_active_transfers();
                            if terminal && !view.has_active_transfers() {
                                if let Some(queue) = view.transfer_queue.get() {
                                    queue.cancel_all();
                                }
                                view.transfer_queue = Arc::new(tokio::sync::OnceCell::new());
                            }
                            if terminal
                                && view.selected_transfer == Some(id)
                                && view.operation_id.is_none()
                                && !view.suspended
                                && let Some(message) = view
                                    .transfer_jobs
                                    .iter()
                                    .find(|job| job.id == id)
                                    .and_then(|job| job.result.as_ref())
                            {
                                view.status = message.clone();
                            }
                            if terminal {
                                if view.active_recovery_id == Some(recovery_id) {
                                    if succeeded {
                                        view.recovery = None;
                                    }
                                    view.active_recovery_id = None;
                                }
                                // Every queued transfer owns a durable record,
                                // including jobs that were added while another
                                // transfer was active. Refreshing on every
                                // terminal result keeps those records visible
                                // even when the single selected recovery ID
                                // has since moved to another job.
                                view.load_recovery_records(window, cx);
                                view.prune_transfer_history();
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                    if terminal {
                        return;
                    }
                }
                executor.timer(Duration::from_millis(16)).await;
                if this.update_in(cx, |_, _, _| ()).is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn inspect_selected_quarantine(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.pending.is_some() || self.operation_id.is_some() {
            return;
        }
        let Some(job) = self
            .transfer_jobs
            .iter()
            .find(|job| Some(job.id) == self.selected_transfer)
        else {
            return;
        };
        if !matches!(
            job.status.phase,
            TransferPhase::Uncertain | TransferPhase::Failed
        ) {
            return;
        }
        let spec = job.recovery.spec.clone();
        self.run(Operation::InspectQuarantine(spec), window, cx);
    }
    fn file_isolation_target(&self) -> Option<IsolationTarget> {
        self.isolation_target
            .clone()
            .or_else(|| {
                self.editing
                    .as_ref()
                    .map(|(path, _)| IsolationTarget::Remote(path.clone()))
            })
            .or_else(|| {
                self.selected
                    .as_ref()
                    .map(|entry| IsolationTarget::Remote(entry.path.clone()))
            })
    }
    fn inspect_file_quarantine(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.pending.is_some() || self.operation_id.is_some() {
            return;
        }
        if let Some(target) = self.file_isolation_target() {
            self.run(Operation::InspectFileQuarantine(target), window, cx);
        }
    }

    pub(super) fn clear_recovery_review_queue(&mut self) {
        self.recovery_selection.clear();
        self.recovery_review_queue.clear();
        self.recovery_review_session = None;
        self.recovery_review_active = None;
        self.recovery_review_revision = self.recovery_review_revision.wrapping_add(1);
    }

    fn cancel_recovery_review_queue(&mut self, cx: &mut Context<Self>) {
        if self
            .pending
            .as_ref()
            .and_then(|(_, operation)| operation.persisted_recovery_id())
            .is_some()
        {
            self.pending = None;
            self.confirmation_expanded = false;
        }
        self.revoke_recovery_review();
        self.status = Message::new(
            "已清空恢复待审核队列；未审核记录保留，未执行传输。",
            "Cleared the recovery review queue; unreviewed records were retained and no transfer ran.",
        );
        cx.notify();
    }

    pub(super) fn revoke_recovery_review(&mut self) {
        self.clear_recovery_review_queue();
        self.recovery_claim = None;
        self.active_recovery_id = None;
    }

    fn requeue_failed_recovery_review_item(&mut self, id: uuid::Uuid) {
        if self.recovery_review_active == Some(id) {
            if self.recovery_review_session == Some(self.session_token)
                && !self.recovery_review_queue.contains(&id)
            {
                self.recovery_review_queue.push_front(id);
            }
            self.recovery_review_active = None;
        }
    }

    pub(super) fn recovery_review_queue_is_current(&self) -> bool {
        self.recovery_review_session == Some(self.session_token)
            && self
                .recovery_review_queue
                .iter()
                .all(|id| self.recovery_records.iter().any(|record| record.id == *id))
    }

    fn queue_selected_persisted_recoveries(&mut self, cx: &mut Context<Self>) {
        if self.recovery_review_session.is_some() && !self.recovery_review_queue_is_current() {
            self.revoke_recovery_review();
            cx.notify();
            return;
        }
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
            || !self.recovery_review_queue.is_empty()
        {
            return;
        }
        let selected_count = self.recovery_selection.len();
        if selected_count == 0 {
            self.status = Message::new(
                "请先选择至少一条恢复记录。",
                "Select at least one recovery record first.",
            );
            cx.notify();
            return;
        }
        let queue: Vec<_> = self
            .recovery_records
            .iter()
            .filter(|record| self.recovery_selection.contains(&record.id))
            .map(|record| record.id)
            .collect();
        if queue.len() != selected_count {
            self.revoke_recovery_review();
            self.status = Message::new(
                "恢复列表已变化，已清空选择和待审核队列；请重新选择。",
                "The recovery list changed; selection and the review queue were cleared. Select again.",
            );
            cx.notify();
            return;
        }
        self.recovery_review_queue = VecDeque::from(queue);
        self.recovery_review_session = Some(self.session_token);
        self.recovery_selection.clear();
        self.status = Message::new(
            format!(
                "已建立 {} 条有序待审核队列；不会自动连接或执行。",
                selected_count
            ),
            format!(
                "Queued {selected_count} records in stable order; nothing will connect or execute automatically."
            ),
        );
        cx.notify();
    }

    fn selected_recovery_ids_in_order(&self) -> Vec<uuid::Uuid> {
        self.recovery_records
            .iter()
            .filter(|record| self.recovery_selection.contains(&record.id))
            .map(|record| record.id)
            .collect()
    }

    fn toggle_recovery_selection(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
            || !self.recovery_review_queue.is_empty()
        {
            return;
        }
        let visible: Vec<_> = self
            .recovery_records
            .iter()
            .map(|record| record.id)
            .collect();
        if !self.recovery_selection.click(&id, &visible, true, false) {
            self.status = Message::new(
                "最多同时整理 32 条恢复记录。",
                "Select at most 32 recovery records at once.",
            );
        }
        cx.notify();
    }

    fn review_next_persisted_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
        {
            return;
        }
        if !self.recovery_review_queue_is_current() {
            self.revoke_recovery_review();
            self.status = Message::new(
                "待审核队列已失效；请重新选择恢复记录。",
                "The pending review queue is stale; select the recovery records again.",
            );
            cx.notify();
            return;
        }
        let Some(id) = self.recovery_review_queue.front().copied() else {
            self.status = Message::new(
                "待审核队列已完成；没有自动执行任何记录。",
                "The review queue is complete; no record was executed automatically.",
            );
            cx.notify();
            return;
        };
        self.request_persisted_recovery_inner(id, true, window, cx);
    }

    pub(super) fn request_persisted_recovery(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_persisted_recovery_inner(id, false, window, cx);
    }

    fn request_persisted_recovery_inner(
        &mut self,
        id: uuid::Uuid,
        queued: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || self.recovery_writes.contains(&id)
            || !self.recovery_claim_requests.is_empty()
        {
            return;
        }
        if !queued {
            self.revoke_recovery_review();
        }
        if queued
            && (self.recovery_review_session != Some(self.session_token)
                || self.recovery_review_queue.front().copied() != Some(id))
        {
            self.revoke_recovery_review();
            self.status = Message::new(
                "待审核队列已失效，已拒绝旧入口；请重新选择。",
                "The review queue is stale; the old entry was refused. Select again.",
            );
            cx.notify();
            return;
        }
        let Some(record) = self.recovery_records.iter().find(|record| record.id == id) else {
            if queued {
                self.revoke_recovery_review();
            }
            return;
        };
        if !recovery_connection_matches(record, self.connection_id) {
            if queued {
                self.revoke_recovery_review();
            }
            self.status = Message::new(
                "恢复记录绑定了另一条保存连接；请在原连接标签中检查。",
                "This recovery record belongs to another saved connection; inspect it from the original connection tab.",
            );
            cx.notify();
            return;
        }
        let spec = super::transfer_spec_from_recovery(record);
        let directory = record.directory;
        let connection_bound = record.connection_id.is_some();
        let expected_record = record.clone();
        self.recovery_claim_requests.insert(id);
        let store = self.recovery_store.clone();
        let runtime = self.runtime.clone();
        let executor = cx.background_executor().clone();
        let expected_session = self.session_token;
        let expected_revision = self.recovery_review_revision;
        let task = crate::runtime_bridge::spawn(&runtime, executor, async move {
            tokio::task::spawn_blocking(move || {
                let claim = store.claim(id).map_err(|error| error.to_string())?;
                let records = store.load().map_err(|error| error.to_string())?;
                if !records.contains(&expected_record) {
                    return Err(
                        "the recovery record changed; reload and inspect it again".to_owned()
                    );
                }
                Ok(claim)
            })
            .await
            .map_err(|error| format!("transfer recovery claim worker stopped: {error}"))?
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |view, window, cx| {
                if view.session_token != expected_session {
                    return;
                }
                view.recovery_claim_requests.remove(&id);
                if view.suspended || view.recovery_review_revision != expected_revision {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(claim)) => {
                        if queued
                            && (!view.recovery_review_queue_is_current()
                                || view.recovery_review_queue.front().copied() != Some(id))
                        {
                            view.revoke_recovery_review();
                            view.status = Message::new(
                                "待审核队列在占用记录期间已变化；未采用这条 claim。",
                                "The review queue changed while claiming the record; the claim was not adopted.",
                            );
                            return;
                        }
                        if queued {
                            view.recovery_review_queue.pop_front();
                            view.recovery_review_active = Some(id);
                        }
                        view.active_recovery_id = Some(id);
                        let operation = Operation::PlanPersistedRecovery {
                            id,
                            spec,
                            directory,
                        };
                        view.recovery_claim = Some(claim);
                        if connection_bound {
                            view.run(operation, window, cx);
                        } else {
                            view.status = Message::new(
                                "请先确认当前 SSH 主机，再开始只读检查。",
                                "Confirm the current SSH host before starting the read-only inspection.",
                            );
                            view.replace_pending_review(
                                super::persisted_recovery_host_confirmation(&view.host),
                                operation,
                            );
                        }
                    }
                    Ok(Err(error)) => {
                        if queued {
                            view.revoke_recovery_review();
                        }
                        view.status = Message::new(
                            format!("无法占用传输恢复记录：{error}"),
                            format!("Unable to claim the transfer recovery record: {error}"),
                        );
                    }
                    Err(error) => {
                        if queued {
                            view.revoke_recovery_review();
                        }
                        view.status = Message::new(
                            format!("无法占用传输恢复记录：{error}"),
                            format!("Unable to claim the transfer recovery record: {error}"),
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn batch_discard_review_message(&self, ids: &[uuid::Uuid]) -> Message {
        let mut zh = format!(
            "即将逐条占用并丢弃 {} 条本地传输恢复记录。\n这一步只删除本地恢复日志，不连接 SSH、不执行远端检查、不重放传输。每条记录都必须独立获得 claim；Busy、记录消失或其它错误会保留未处理记录并报告。请明确确认：\n",
            ids.len()
        );
        let mut en = format!(
            "This will claim and discard {} local transfer recovery records one by one.\nThis only removes local recovery metadata; it does not connect SSH, inspect a remote target, or replay a transfer. Each record needs its own claim; Busy, missing-record and other errors retain the unprocessed record and are reported. Confirm explicitly:\n",
            ids.len()
        );
        for (index, id) in ids.iter().enumerate() {
            let Some(record) = self.recovery_records.iter().find(|record| record.id == *id) else {
                continue;
            };
            let (direction_zh, direction_en, source, destination) = match record.direction {
                TransferRecoveryDirection::Upload => (
                    "上传",
                    "Upload",
                    record.local_path.display().to_string(),
                    record.remote_path.clone(),
                ),
                TransferRecoveryDirection::Download => (
                    "下载",
                    "Download",
                    record.remote_path.clone(),
                    record.local_path.display().to_string(),
                ),
            };
            zh.push_str(&format!(
                "{}. {direction_zh} · {source} → {destination}\n",
                index + 1
            ));
            en.push_str(&format!(
                "{}. {direction_en} · {source} → {destination}\n",
                index + 1
            ));
        }
        Message::new(zh, en)
    }

    fn request_batch_discard(&mut self, cx: &mut Context<Self>) {
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
            || !self.recovery_review_queue.is_empty()
        {
            return;
        }
        let selected_count = self.recovery_selection.len();
        if selected_count == 0 {
            self.status = Message::new(
                "请先选择要丢弃的恢复记录。",
                "Select recovery records to discard first.",
            );
            cx.notify();
            return;
        }
        let ids = self.selected_recovery_ids_in_order();
        if ids.len() != selected_count {
            self.revoke_recovery_review();
            self.status = Message::new(
                "恢复列表已变化，已清空选择；请重新选择后再丢弃。",
                "The recovery list changed; selection was cleared. Select again before discarding.",
            );
            cx.notify();
            return;
        }
        let candidates: Vec<_> = ids
            .iter()
            .filter_map(|id| {
                self.recovery_records
                    .iter()
                    .find(|record| record.id == *id)
                    .map(|record| RecoveryDiscardCandidate {
                        id: *id,
                        record: record.clone(),
                    })
            })
            .collect();
        if candidates.len() != selected_count {
            self.revoke_recovery_review();
            self.status = Message::new(
                "恢复列表已变化，已清空选择；请重新选择后再丢弃。",
                "The recovery list changed; selection was cleared. Select again before discarding.",
            );
            cx.notify();
            return;
        }
        self.replace_pending_review(
            self.batch_discard_review_message(&ids),
            Operation::DiscardPersistedRecoveryBatch(candidates),
        );
        cx.notify();
    }

    pub(super) fn start_batch_discard(
        &mut self,
        candidates: Vec<RecoveryDiscardCandidate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<_> = candidates.iter().map(|candidate| candidate.id).collect();
        if self.suspended
            || self.recovery_loading
            || self.operation_id.is_some()
            || candidates.is_empty()
            || candidates.iter().any(|candidate| {
                !self
                    .recovery_records
                    .iter()
                    .any(|record| record.id == candidate.id)
            })
        {
            self.status = Message::new(
                "恢复列表已变化，批量丢弃已拒绝；原记录保留。",
                "The recovery list changed; batch discard was refused and records were retained.",
            );
            cx.notify();
            return;
        }
        self.recovery_writes.extend(ids.iter().copied());
        self.recovery_selection.clear();
        let store = self.recovery_store.clone();
        let runtime = self.runtime.clone();
        let executor = cx.background_executor().clone();
        let expected_session = self.session_token;
        let worker_candidates = candidates.clone();
        let task = crate::runtime_bridge::spawn(&runtime, executor, async move {
            tokio::task::spawn_blocking(move || {
                worker_candidates
                    .into_iter()
                    .map(|candidate| {
                        let id = candidate.id;
                        let result = match store.claim(id) {
                            Ok(claim) => match store.load() {
                                Ok(records) if records.contains(&candidate.record) => store
                                    .remove_with_claim(&claim)
                                    .map_err(|error| error.to_string()),
                                Ok(_) => {
                                    Err("the record changed after confirmation; it was retained"
                                        .to_owned())
                                }
                                Err(error) => Err(error.to_string()),
                            },
                            Err(error) => Err(error.to_string()),
                        };
                        (id, result)
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .map_err(|error| format!("transfer recovery discard worker stopped: {error}"))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(error.to_string()),
            };
            let _ = this.update_in(cx, |view, window, cx| {
                view.apply_batch_discard_result(ids, expected_session, result, window, cx);
            });
        })
        .detach();
        self.status = Message::new(
            "正在逐条处理恢复记录；未处理项会保留。",
            "Discarding recovery records one by one; unprocessed items are retained.",
        );
        cx.notify();
    }

    pub(super) fn apply_batch_discard_result(
        &mut self,
        ids: Vec<uuid::Uuid>,
        expected_session: uuid::Uuid,
        result: Result<Vec<RecoveryDiscardOutcome>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.session_token != expected_session {
            return;
        }
        for id in &ids {
            self.recovery_writes.remove(id);
        }
        let outcomes = result.unwrap_or_else(|error| {
            ids.iter()
                .copied()
                .map(|id| (id, Err(error.clone())))
                .collect()
        });
        let successful: Vec<_> = outcomes
            .iter()
            .filter_map(|(id, result)| result.is_ok().then_some(*id))
            .collect();
        let failed: Vec<_> = outcomes
            .iter()
            .filter_map(|(id, result)| result.as_ref().err().map(|error| (*id, error.clone())))
            .collect();
        self.advance_recovery_mutation_generation();
        self.recovery_records
            .retain(|record| !successful.contains(&record.id));
        // The worker may have removed some records before an overall error or
        // an unknown return. Reload the authoritative store in every outcome
        // so a stale UI snapshot cannot hide or resurrect retained records.
        self.load_recovery_records(window, cx);
        self.recovery_selection.clear();
        let visible: Vec<_> = self
            .recovery_records
            .iter()
            .map(|record| record.id)
            .collect();
        for (id, _) in &failed {
            let _ = self.recovery_selection.click(id, &visible, true, false);
        }
        let mut zh = format!(
            "批量丢弃完成：确认删除 {} 条，另有 {} 条未确认删除；保留仍存在的记录并重新读取实际列表。",
            successful.len(),
            failed.len()
        );
        let mut en = format!(
            "Batch discard finished: {} confirmed discarded, {} not confirmed; remaining records are retained and the actual list is reloaded.",
            successful.len(),
            failed.len()
        );
        for (id, error) in failed {
            let path = self
                .recovery_records
                .iter()
                .find(|record| record.id == id)
                .map(|record| record.remote_path.as_str())
                .unwrap_or("record");
            zh.push_str(&format!("\n未确认删除：{path}（{error}）"));
            en.push_str(&format!("\nNot confirmed discarded: {path} ({error})"));
        }
        self.status = Message::new(zh, en);
        cx.notify();
    }

    fn discard_persisted_recovery(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended
            || self.pending.is_some()
            || self.operation_id.is_some()
            || self.recovery_writes.contains(&id)
            || self.recovery_claim_requests.contains(&id)
        {
            return;
        }
        if !self.recovery_records.iter().any(|record| record.id == id) {
            return;
        }
        self.recovery_writes.insert(id);
        let store = self.recovery_store.clone();
        let runtime = self.runtime.clone();
        let executor = cx.background_executor().clone();
        let expected_session = self.session_token;
        let task = crate::runtime_bridge::spawn(&runtime, executor, async move {
            tokio::task::spawn_blocking(move || store.remove(id))
                .await
                .map_err(|error| format!("transfer recovery discard worker stopped: {error}"))?
                .map_err(|error| error.to_string())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(_) => Err("transfer recovery discard worker did not return".to_owned()),
            };
            let _ = this.update_in(cx, |view, window, cx| {
                view.apply_discard_result(id, expected_session, result, window, cx);
            });
        })
        .detach();
    }

    pub(super) fn apply_discard_result(
        &mut self,
        id: uuid::Uuid,
        expected_session: uuid::Uuid,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recovery_writes.remove(&id);
        if self.session_token != expected_session {
            return;
        }
        match result {
            Ok(()) => {
                self.advance_recovery_mutation_generation();
                self.recovery_records.retain(|record| record.id != id);
                if self.active_recovery_id == Some(id) {
                    self.active_recovery_id = None;
                    self.recovery = None;
                }
                // Discard changes the durable set, so refresh the complete
                // snapshot after invalidating any older load. This also
                // reconciles records completed by another worker while the
                // discarded row was in flight.
                self.load_recovery_records(window, cx);
                self.status = Message::new(
                    "已丢弃这条传输恢复记录；未连接、未检查、未重放。",
                    "Discarded the transfer recovery record; nothing connected, inspected or replayed.",
                );
            }
            Err(error) => {
                self.recovery_load_error = Some(error.clone());
                self.status = Message::new(
                    format!("无法丢弃传输恢复记录：{error}"),
                    format!("Unable to discard transfer recovery metadata: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn persisted_recovery_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut card = div()
            .id("persisted-transfer-recovery")
            .test_support()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_shrink_0()
                            .child(t(
                                cx,
                                "重启后传输记录",
                                "Post-restart transfer records",
                            )),
                    )
                    .child(
                        div()
                            .id("persisted-transfer-recovery-current-session")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(format!(
                                "{}: {}",
                                t(cx, "当前 SSH 会话", "Current SSH session"),
                                self.host
                            )),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "记录只保存路径、方向和进度；必须在当前 SSH 会话中重新检查、审核并确认，应用不会自动连接或重放。",
                        "Records contain paths, direction and progress only. The current SSH session must inspect, review and confirm again; the app never connects or replays automatically.",
                    )),
            );
        if self.recovery_loading {
            return card
                .child(
                    div()
                        .id("persisted-transfer-recovery-loading")
                        .text_color(rgb(visual.muted))
                        .child(t(cx, "正在读取恢复记录…", "Loading recovery records…")),
                )
                .into_any_element();
        }
        if let Some(error) = &self.recovery_load_error {
            return card
                .child(
                    div()
                        .id("persisted-transfer-recovery-error")
                        .text_color(rgb(visual.danger))
                        .child(format!(
                            "{}: {error}",
                            t(cx, "恢复记录读取失败", "Recovery record load failed")
                        )),
                )
                .into_any_element();
        }
        if self.recovery_records.is_empty() {
            return card
                .child(
                    div()
                        .id("persisted-transfer-recovery-empty")
                        .text_color(rgb(visual.muted))
                        .child(t(cx, "没有待处理记录", "No pending records")),
                )
                .into_any_element();
        }
        let expected_session = self.session_token;
        let selected_count = self.recovery_selection.len();
        let queued_count = self.recovery_review_queue.len();
        let selection_disabled = self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
            || !self.recovery_review_queue.is_empty();
        let queue_button_disabled = selection_disabled || selected_count == 0;
        let review_next_disabled = self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || !self.recovery_writes.is_empty()
            || !self.recovery_claim_requests.is_empty()
            || self.recovery_review_active.is_some()
            || self.recovery_review_queue.is_empty();
        let discard_button_disabled = selection_disabled || selected_count == 0;
        let selected_label =
            SharedString::from(format!("{}: {selected_count}", t(cx, "已选择", "Selected")));
        let mut controls = div()
            .id("persisted-transfer-recovery-batch-controls")
            .test_support()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id("persisted-transfer-recovery-selection-count")
                    .test_support()
                    .text_color(rgb(visual.accent))
                    .role(accesskit::Role::Label)
                    .aria_label(selected_label.clone())
                    .child(selected_label),
            )
            .child(
                Button::new("queue-selected-persisted-transfers")
                    .compact()
                    .ghost()
                    .disabled(queue_button_disabled)
                    .label(t(cx, "建立审核队列", "Build review queue"))
                    .accessibility_label(t(
                        cx,
                        "按当前列表顺序建立恢复记录审核队列",
                        "Build an ordered recovery review queue from the current list",
                    ))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.session_token != expected_session {
                            view.revoke_recovery_review();
                            return;
                        }
                        view.queue_selected_persisted_recoveries(cx);
                    })),
            )
            .child(
                Button::new("review-next-persisted-transfer")
                    .compact()
                    .primary()
                    .disabled(review_next_disabled)
                    .label(t(cx, "逐条进入审核", "Review one by one"))
                    .accessibility_label(t(
                        cx,
                        "进入下一条恢复记录的独立检查和审核",
                        "Enter the next recovery record's independent inspection and review",
                    ))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.session_token != expected_session {
                            view.revoke_recovery_review();
                            return;
                        }
                        view.review_next_persisted_recovery(window, cx);
                    })),
            )
            .child(
                Button::new("discard-selected-persisted-transfers")
                    .compact()
                    .ghost()
                    .disabled(discard_button_disabled)
                    .label(t(cx, "批量丢弃", "Discard selected"))
                    .accessibility_label(t(
                        cx,
                        "逐条占用并丢弃选中的恢复记录，需要再次确认",
                        "Claim and discard selected recovery records one by one; confirmation required",
                    ))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.session_token != expected_session {
                            view.revoke_recovery_review();
                            return;
                        }
                        view.request_batch_discard(cx);
                    })),
            );
        if queued_count > 0 {
            let queued_label =
                SharedString::from(format!("{}: {queued_count}", t(cx, "待审核", "Queued")));
            controls = controls.child(
                div()
                    .id("persisted-transfer-recovery-queue-count")
                    .test_support()
                    .text_color(rgb(visual.warning))
                    .role(accesskit::Role::Label)
                    .aria_label(queued_label.clone())
                    .child(queued_label),
            );
        }
        if self.recovery_review_session.is_some() {
            controls = controls.child(
                Button::new("cancel-persisted-transfer-review-queue")
                    .compact()
                    .ghost()
                    .label(t(cx, "取消待审核", "Cancel review queue"))
                    .accessibility_label(t(
                        cx,
                        "清空待审核队列并保留未审核的恢复记录",
                        "Clear the review queue and retain unreviewed recovery records",
                    ))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.session_token == expected_session {
                            view.cancel_recovery_review_queue(cx);
                        }
                    })),
            );
        }
        card = card.child(controls);
        for record in &self.recovery_records {
            let id = record.id;
            let spec = super::transfer_spec_from_recovery(record);
            let direction = match record.direction {
                TransferRecoveryDirection::Upload => t(cx, "上传", "Upload"),
                TransferRecoveryDirection::Download => t(cx, "下载", "Download"),
            };
            let state = match record.state {
                TransferRecoveryState::Unfinished => t(cx, "未完成", "Unfinished"),
                TransferRecoveryState::OutcomeUnknown => t(cx, "结果未知", "Outcome unknown"),
                TransferRecoveryState::Failed => t(cx, "失败", "Failed"),
                TransferRecoveryState::Cancelled => t(cx, "已取消", "Cancelled"),
            };
            let binding = if record.connection_id.is_some() {
                t(cx, "已绑定保存连接", "Bound to saved connection")
            } else {
                t(
                    cx,
                    "未绑定连接身份，请确认当前 SSH 主机",
                    "No connection binding; verify the current SSH host",
                )
            };
            let summary = format!(
                "{direction} · {state} · {binding} · {} → {} · {} B",
                spec.local.display(),
                spec.remote,
                record.transferred_bytes
            );
            let inspect_disabled = self.suspended
                || self.pending.is_some()
                || self.operation_id.is_some()
                || self.recovery_writes.contains(&id)
                || self.recovery_claim_requests.contains(&id);
            let discard_disabled = self.suspended
                || self.pending.is_some()
                || self.operation_id.is_some()
                || self.recovery_writes.contains(&id)
                || self.recovery_claim_requests.contains(&id)
                || self.recovery_review_session.is_some();
            let selected = self.recovery_selection.contains(&id);
            card = card.child(
                div()
                    .id(SharedString::from(format!(
                        "persisted-transfer-recovery-row-{id}"
                    )))
                    .test_support()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        Checkbox::new(SharedString::from(format!(
                            "select-persisted-transfer-{id}"
                        )))
                        .small()
                        .checked(selected)
                        .disabled(selection_disabled)
                        .accessibility_label(format!(
                            "{} {}",
                            t(cx, "选择恢复记录", "Select recovery record"),
                            summary
                        ))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            if view.session_token != expected_session {
                                view.revoke_recovery_review();
                                return;
                            }
                            view.toggle_recovery_selection(id, cx);
                        })),
                    )
                    .child(div().flex_1().min_w_0().text_ellipsis().child(summary))
                    .child(
                        Button::new(SharedString::from(format!(
                            "inspect-persisted-transfer-{id}"
                        )))
                        .primary()
                        .compact()
                        .disabled(inspect_disabled)
                        .label(t(cx, "检查", "Inspect"))
                        .accessibility_label(t(
                            cx,
                            "检查并重新审核传输记录",
                            "Inspect and re-review transfer record",
                        ))
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                view.request_persisted_recovery(id, window, cx)
                            },
                        )),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "discard-persisted-transfer-{id}"
                        )))
                        .ghost()
                        .compact()
                        .disabled(discard_disabled)
                        .label(t(cx, "丢弃", "Discard"))
                        .accessibility_label(t(
                            cx,
                            "丢弃传输恢复记录",
                            "Discard transfer recovery record",
                        ))
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                view.discard_persisted_recovery(id, window, cx)
                            },
                        )),
                    ),
            );
        }
        card.into_any_element()
    }

    fn persisted_recovery_surface_visible(&self) -> bool {
        self.recovery_loading
            || self.recovery_load_error.is_some()
            || !self.recovery_records.is_empty()
    }

    fn prune_transfer_history(&mut self) {
        while self
            .transfer_jobs
            .iter()
            .filter(|job| !active(job.status.phase))
            .count()
            > MAX_QUEUED_TRANSFERS
        {
            let Some(index) = self.transfer_jobs.iter().position(|job| {
                !active(job.status.phase) && Some(job.id) != self.selected_transfer
            }) else {
                break;
            };
            self.transfer_jobs.remove(index);
        }
    }
    pub(super) fn parallel_queue_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let limit = self.queue_parallelism.load(Ordering::Acquire);
        let active_count = self
            .transfer_jobs
            .iter()
            .filter(|job| active(job.status.phase))
            .count();
        let mut header = div().flex().flex_wrap().items_center().gap_1().child(
            div().font_weight(FontWeight::SEMIBOLD).child(t(
                cx,
                "传输队列 · 并发",
                "Transfer queue · Concurrency",
            )),
        );
        for count in 1..=4 {
            header = header.child(
                Button::new(SharedString::from(format!("transfer-parallelism-{count}")))
                    .compact()
                    .ghost()
                    .label(count.to_string())
                    .disabled(self.suspended)
                    .when(limit == count, |button| button.primary())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.suspended {
                            return;
                        }
                        view.queue_parallelism.store(count, Ordering::Release);
                        if let Some(queue) = view.transfer_queue.get()
                            && let Err(error) = queue.set_parallelism(count)
                        {
                            view.status =
                                Message::detail("并发设置失败", "Unable to set concurrency", error);
                        }
                        cx.notify();
                    })),
            );
        }
        header = header.child(
            div()
                .text_color(rgb(visual.muted))
                .child(format!("{active_count}/32")),
        );
        header = header.child(
            Button::new("inspect-transfer-isolation")
                .compact()
                .ghost()
                .label(t(cx, "检查隔离目标", "Inspect isolated target"))
                .disabled(
                    self.suspended
                        || self.operation_id.is_some()
                        || self.pending.is_some()
                        || !self.transfer_jobs.iter().any(|job| {
                            Some(job.id) == self.selected_transfer
                                && matches!(
                                    job.status.phase,
                                    TransferPhase::Uncertain | TransferPhase::Failed
                                )
                        }),
                )
                .on_click(
                    cx.listener(|view, _, window, cx| view.inspect_selected_quarantine(window, cx)),
                ),
        );
        header = header.child(
            Button::new("inspect-file-isolation")
                .compact()
                .ghost()
                .label(t(cx, "检查文件隔离", "Inspect file isolation"))
                .disabled(
                    self.suspended
                        || self.operation_id.is_some()
                        || self.pending.is_some()
                        || self.file_isolation_target().is_none(),
                )
                .on_click(
                    cx.listener(|view, _, window, cx| view.inspect_file_quarantine(window, cx)),
                ),
        );
        let mut list = div()
            .id("transfer-queue-list")
            .test_support()
            .max_h(px(148.))
            .overflow_y_scroll()
            .track_scroll(&self.transfer_list_scroll)
            .vertical_scrollbar(&self.transfer_list_scroll)
            .flex()
            .flex_col()
            .gap_1();
        for job in &self.transfer_jobs {
            let id = job.id;
            let selected = self.selected_transfer == Some(id);
            let mut row = div()
                .id(SharedString::from(format!("transfer-job-{id}")))
                .test_support()
                .flex()
                .flex_wrap()
                .gap_1()
                .items_center()
                .border_b_1()
                .border_color(rgb(visual.border))
                .child(
                    Button::new(SharedString::from(format!("select-transfer-{id}")))
                        .max_w(relative(0.6))
                        .ghost()
                        .compact()
                        .when(selected, |b| b.primary())
                        .label(job.status.destination.clone())
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.select_job(id);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_color(rgb(visual.accent))
                        .child(job.status.phase.label(cx)),
                )
                .child(
                    div()
                        .text_color(rgb(visual.muted))
                        .child(job.status.bytes_label(cx)),
                );
            if active(job.status.phase) {
                if matches!(
                    job.status.phase,
                    TransferPhase::Running | TransferPhase::Paused
                ) {
                    let pause = job.status.phase == TransferPhase::Running;
                    row = row.child(
                        Button::new(SharedString::from(format!("pause-transfer-{id}")))
                            .ghost()
                            .compact()
                            .disabled(self.suspended)
                            .label(if pause {
                                t(cx, "暂停", "Pause")
                            } else {
                                t(cx, "继续", "Continue")
                            })
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.request_job_pause(id, pause, cx)
                            })),
                    );
                }
                row = row.child(
                    Button::new(SharedString::from(format!("cancel-transfer-{id}")))
                        .ghost()
                        .compact()
                        .disabled(job.status.phase == TransferPhase::Cancelling)
                        .label(t(cx, "取消", "Cancel"))
                        .on_click(cx.listener(move |view, _, _, cx| view.cancel_job(id, cx))),
                );
            }
            if let Some(message) = &job.result
                && !active(job.status.phase)
            {
                row = row.child(
                    div()
                        .w_full()
                        .text_color(rgb(visual.muted))
                        .child(message.render(cx)),
                );
            }
            list = list.child(row);
        }
        let explanation = div().text_color(rgb(visual.muted)).child(t(
            cx,
            "暂停保留槽位和路径锁；调低并发只影响后续任务。未知写入结果隔离目标，重连不会解除；可检查目标，再显式审核解除隔离的风险。不会重放。",
            "Paused jobs keep slots and path locks; lower concurrency applies to later jobs. Unknown writes isolate destinations across reconnects; inspect the target and explicitly review the risk of releasing isolation. Jobs never replay.",
        ));
        let card = div()
            .id("parallel-transfer-queue")
            .test_support()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap_1()
            .p_2()
            .border_t_1()
            .border_color(rgb(visual.border));
        let recovery_visible = self.persisted_recovery_surface_visible();
        // The dedicated queue starts with actual outcomes. The full concurrency
        // controls and isolation explanation remain reachable below the rows.
        let card = if self.queue_details_visible {
            card.when(recovery_visible, |view| {
                view.child(self.persisted_recovery_card(cx))
            })
            .when(!self.transfer_jobs.is_empty(), |view| view.child(list))
            .child(header)
            .child(explanation)
        } else {
            card.child(header)
                .child(explanation)
                .when(recovery_visible, |view| {
                    view.child(self.persisted_recovery_card(cx))
                })
                .when(!self.transfer_jobs.is_empty(), |view| view.child(list))
        };
        card.into_any_element()
    }
}

async fn queued_operation(
    queue: &TransferQueue,
    operation: Operation,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
    recovery: Option<&mut TransferRecoveryWriter>,
) -> Result<Outcome, FileFailure> {
    let (direction, existing) = match &operation {
        Operation::TransferFile(plan) => (plan.direction(), None),
        Operation::Upload(..) => (TransferDirection::Upload, None),
        Operation::Download(..) => (TransferDirection::Download, None),
        Operation::TransferDirectory(plan) => (plan.direction(), None),
        Operation::ResumeFile(plan) => (plan.direction(), Some((plan.existing_bytes(), false))),
        Operation::ResumeDirectory(plan) => (plan.direction(), Some((plan.existing_bytes(), true))),
        Operation::ResumePersistedFile(_, plan) => {
            (plan.direction(), Some((plan.existing_bytes(), false)))
        }
        Operation::ResumePersistedDirectory(_, plan) => {
            (plan.direction(), Some((plan.existing_bytes(), true)))
        }
        _ => return Err(FileFailure::WorkerStopped),
    };
    let mut transfer = tokio::select! {
        biased;
        _ = cancellation(&stop) => return Err(FileFailure::CancelledBeforeStart),
        result = async { match operation {
            Operation::TransferFile(plan) => queue.enqueue_reviewed_file(plan).await,
            Operation::Upload(local, remote) => queue.enqueue_atomic_upload(TransferSpec::upload(local, remote)).await,
            Operation::Download(remote, local) => queue.enqueue(TransferSpec::download(remote, local)).await,
            Operation::TransferDirectory(plan) => queue.enqueue_directory(plan).await,
            Operation::ResumeFile(plan) => queue.enqueue_resume(plan).await,
            Operation::ResumeDirectory(plan) => queue.enqueue_directory_resume(plan).await,
            Operation::ResumePersistedFile(_, plan) => queue.enqueue_resume(plan).await,
            Operation::ResumePersistedDirectory(_, plan) => {
                queue.enqueue_directory_resume(plan).await
            }
            _ => Err(SessionError::Worker),
        }} => result.map_err(FileFailure::from)?,
    };
    worker::observe_transfer(
        &mut transfer,
        direction,
        existing,
        stop,
        pause,
        progress,
        recovery,
    )
    .await
}

/// The complete observed targets and unresolved risk stay in the existing
/// scrollable approval surface with fixed confirm/cancel buttons.
pub(super) fn quarantine_review_message(
    review: &keelshell_session::sftp::TransferQuarantineReview,
) -> Message {
    let mut zh = String::from(
        "解除以下目标的应用隔离？此操作不写文件、不重试。只读检查无法证明迟到的写入或重命名已停止；确认表示接受后续写入与旧请求冲突的风险。原任务仍为结果未知，新的传输需要再次审核。\n",
    );
    let mut en = String::from(
        "Release application isolation for these targets? This changes no file and retries nothing. Read-only inspection cannot prove late writes or renames have stopped; confirmation accepts the risk that later writes conflict with the old request. Previous jobs stay unknown; any new transfer needs separate review.\n",
    );
    for entry in review.entries() {
        let side_zh = if entry.local { "本地" } else { "远端" };
        let side_en = if entry.local { "Local" } else { "Remote" };
        let kind_zh = if !entry.exists {
            "不存在"
        } else if entry.symlink {
            "符号链接"
        } else if entry.directory {
            "目录"
        } else {
            "文件"
        };
        let kind_en = if !entry.exists {
            "missing"
        } else if entry.symlink {
            "symlink"
        } else if entry.directory {
            "directory"
        } else {
            "file"
        };
        let bytes = entry.bytes.map_or("—".to_owned(), |n| n.to_string());
        let modified = entry.modified.map_or("—".to_owned(), |n| n.to_string());
        zh.push_str(&format!(
            "\n隔离记录 #{} · {side_zh}：{}\n只读观察：{kind_zh}；{bytes} 字节；修改时间（Unix 秒）{modified}\n",
            entry.reservation_id, entry.destination
        ));
        en.push_str(&format!("\nIsolation record #{} · {side_en}: {}\nRead-only observation: {kind_en}; {bytes} bytes; modification time (Unix seconds) {modified}\n", entry.reservation_id, entry.destination));
    }
    Message::new(zh, en)
}

#[cfg(test)]
mod queue_summary_tests {
    use super::*;

    fn job(session_token: uuid::Uuid, phase: TransferPhase) -> QueuedTransfer {
        let spec = TransferSpec::upload(PathBuf::from("not-opened-by-summary"), "/not-written");
        let mut status = TransferStatus::new(spec.clone(), false, false);
        status.phase = phase;
        let (pause, _) = tokio::sync::watch::channel(false);
        QueuedTransfer {
            id: uuid::Uuid::new_v4(),
            session_token,
            status,
            result: None,
            stop: Arc::new(AtomicBool::new(false)),
            pause,
            recovery: RecoveryCandidate {
                persisted_id: None,
                session_token,
                spec,
                directory: false,
            },
        }
    }

    #[core::prelude::v1::test]
    fn queue_entry_counts_all_current_owner_phases_without_merging_terminal_outcomes() {
        let owner = uuid::Uuid::new_v4();
        let old_owner = uuid::Uuid::new_v4();
        let phases = [
            TransferPhase::Preparing,
            TransferPhase::Queued,
            TransferPhase::Running,
            TransferPhase::Pausing,
            TransferPhase::Paused,
            TransferPhase::Resuming,
            TransferPhase::Cancelling,
            TransferPhase::Completed,
            TransferPhase::Failed,
            TransferPhase::Cancelled,
            TransferPhase::Uncertain,
        ];
        let jobs: Vec<_> = phases
            .iter()
            .copied()
            .flat_map(|phase| [job(owner, phase), job(old_owner, phase)])
            .collect();
        assert_eq!(
            QueueSummary::for_session(&jobs, owner),
            QueueSummary {
                active: 7,
                completed: 1,
                failed: 1,
                cancelled: 1,
                uncertain: 1,
            }
        );
        assert_eq!(
            QueueSummary::for_session(&jobs, uuid::Uuid::new_v4()),
            QueueSummary::default()
        );
        assert!(
            jobs.iter().all(|job| !job.stop.load(Ordering::Acquire)),
            "summarizing a queue cannot cancel or operate it"
        );
    }
}
