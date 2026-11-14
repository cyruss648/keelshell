//! Multiple reviewed jobs share one queue and the original authenticated session.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::component::scroll::ScrollableElement;
use keelshell_session::sftp::{MAX_QUEUED_TRANSFERS, TransferQueue};

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
        let Some(session) = self.session.clone().filter(|_| !self.suspended) else {
            return;
        };
        let Some(status) = operation.transfer_status() else {
            return;
        };
        let Some((spec, directory)) = operation.recovery_spec() else {
            return;
        };
        let persisted_id = operation.persisted_recovery_id();
        let recovery_id = persisted_id.unwrap_or_else(uuid::Uuid::new_v4);
        let persisted_progress = super::persisted_recovery_progress(&operation);
        let recovery_record = if let Some(id) = persisted_id {
            let Some(record) = self.recovery_records.iter().find(|record| record.id == id) else {
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
                self.status = Message::new(
                    "恢复记录与当前审核目标不一致，已拒绝继续。",
                    "The recovery record does not match the reviewed target; continuation was refused.",
                );
                cx.notify();
                return;
            }
            if !recovery_connection_matches(record, self.connection_id) {
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
            self.status = Message::new(
                "传输队列已满（最多 32 个未结束任务），请等待或取消后再加入。",
                "Transfer queue is full (32 unfinished jobs); wait or cancel before adding another.",
            );
            cx.notify();
            return;
        }
        let recovery_claim = if let Some(id) = persisted_id {
            if self.active_recovery_id != Some(id) {
                self.status = Message::new(
                    "恢复审核已失效，请重新检查该传输记录。",
                    "The recovery review is stale; inspect the transfer record again.",
                );
                cx.notify();
                return;
            }
            let Some(claim) = self.recovery_claim.take() else {
                self.status = Message::new(
                    "无法确认恢复记录的独占审核权，已拒绝继续。",
                    "The recovery record is not exclusively claimed; continuation was refused.",
                );
                cx.notify();
                return;
            };
            if claim.id() != id {
                self.status = Message::new(
                    "恢复记录审核权与目标不一致，已拒绝继续。",
                    "The recovery claim does not match the reviewed target; continuation was refused.",
                );
                cx.notify();
                return;
            }
            Some(claim)
        } else {
            None
        };
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
                persisted_id,
                session_token,
                spec,
                directory,
            },
        });
        self.select_job(id);
        self.pending = None;
        self.busy = true;
        self.active_recovery_id = Some(recovery_id);
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
                                    view.load_recovery_records(window, cx);
                                }
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

    fn request_persisted_recovery(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended
            || self.recovery_loading
            || self.pending.is_some()
            || self.operation_id.is_some()
            || self.recovery_writes.contains(&id)
            || self.recovery_claim_requests.contains(&id)
        {
            return;
        }
        let Some(record) = self.recovery_records.iter().find(|record| record.id == id) else {
            return;
        };
        if !recovery_connection_matches(record, self.connection_id) {
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
        self.recovery_claim_requests.insert(id);
        let store = self.recovery_store.clone();
        let runtime = self.runtime.clone();
        let executor = cx.background_executor().clone();
        let expected_session = self.session_token;
        let task = crate::runtime_bridge::spawn(&runtime, executor, async move {
            tokio::task::spawn_blocking(move || store.claim(id))
                .await
                .map_err(|error| format!("transfer recovery claim worker stopped: {error}"))?
                .map_err(|error| error.to_string())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.recovery_claim_requests.remove(&id);
                if view.session_token != expected_session || view.suspended {
                    return;
                }
                match result {
                    Ok(Ok(claim)) => {
                        view.active_recovery_id = Some(id);
                        view.recovery_claim = Some(claim);
                        let operation = Operation::PlanPersistedRecovery {
                            id,
                            spec,
                            directory,
                        };
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
                        view.status = Message::new(
                            format!("无法占用传输恢复记录：{error}"),
                            format!("Unable to claim the transfer recovery record: {error}"),
                        );
                    }
                    Err(error) => {
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
            let result = task.await;
            let _ = this.update_in(cx, |view, _, cx| {
                view.recovery_writes.remove(&id);
                if view.session_token != expected_session {
                    return;
                }
                match result {
                    Ok(Ok(())) => {
                        view.recovery_records.retain(|record| record.id != id);
                        if view.active_recovery_id == Some(id) {
                            view.active_recovery_id = None;
                            view.recovery = None;
                        }
                        view.status = Message::new(
                            "已丢弃这条传输恢复记录；未连接、未检查、未重放。",
                            "Discarded the transfer recovery record; nothing connected, inspected or replayed.",
                        );
                    }
                    Ok(Err(error)) => {
                        view.recovery_load_error = Some(error.to_string());
                        view.status = Message::new(
                            format!("无法丢弃传输恢复记录：{error}"),
                            format!("Unable to discard transfer recovery metadata: {error}"),
                        );
                    }
                    Err(error) => {
                        view.recovery_load_error = Some(error.to_string());
                        view.status = Message::new(
                            format!("无法丢弃传输恢复记录：{error}"),
                            format!("Unable to discard transfer recovery metadata: {error}"),
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
                || self.recovery_claim_requests.contains(&id);
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
