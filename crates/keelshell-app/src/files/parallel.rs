//! Multiple reviewed jobs share one queue and the original authenticated session.
use super::*;
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

impl FilesPanel {
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
                session_token,
                spec,
                directory,
            },
        });
        self.select_job(id);
        self.pending = None;
        self.busy = true;
        self.status = Message::new(
            "已加入传输队列；可继续审核加入其它任务。",
            "Added to the transfer queue; review and add more jobs while it runs.",
        );
        let queue_cell = self.transfer_queue.clone();
        let parallelism = self.queue_parallelism.clone();
        let runtime = self.runtime.clone();
        let worker_stop = stop.clone();
        let (sender, receiver) = mpsc::sync_channel(16);
        if let Err(error) = crate::terminal::spawn_transport_worker(
            "keelshell-queued-sftp",
            stop,
            move || {
                let result = runtime.block_on(async {
                if worker_stop.load(Ordering::Acquire) { return Err(FileFailure::CancelledBeforeStart); }
                let queue = tokio::select! {
                    biased;
                    _ = cancellation(&worker_stop) => return Err(FileFailure::CancelledBeforeStart),
                    result = queue_cell.get_or_try_init(|| async {
                        let sftp = Arc::new(session.sftp().await?);
                        let queue = Arc::new(sftp.transfer_queue());
                        queue.set_parallelism(parallelism.load(Ordering::Acquire))?;
                        Ok::<_, SessionError>(queue)
                    }) => result.map_err(FileFailure::from)?.clone(),
                };
                queue.set_parallelism(parallelism.load(Ordering::Acquire))?;
                if worker_stop.load(Ordering::Acquire) { return Err(FileFailure::CancelledBeforeStart); }
                queued_operation(&queue, operation, worker_stop, pause_receiver, &sender).await
            });
                let completion = result.as_ref().map(|_| ()).map_err(ToString::to_string);
                let _ = sender.send(WorkerMessage::Result(Box::new(result)));
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
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let message = match receiver.try_recv() {
                    Ok(message) => Some(message),
                    Err(mpsc::TryRecvError::Disconnected) => Some(WorkerMessage::Result(Box::new(
                        Err(FileFailure::WorkerStopped),
                    ))),
                    Err(mpsc::TryRecvError::Empty) => None,
                };
                let terminal = matches!(message, Some(WorkerMessage::Result(_)));
                if let Some(message) = message {
                    if this
                        .update_in(cx, |view, _, cx| {
                            let Some(job) = view
                                .transfer_jobs
                                .iter_mut()
                                .find(|job| job.id == id && job.session_token == session_token)
                            else {
                                return;
                            };
                            match message {
                                WorkerMessage::Transfer(event) => job.status.update(event),
                                WorkerMessage::Progress(message) => job.result = Some(message),
                                WorkerMessage::Result(result) => {
                                    if let Some(phase) = terminal_transfer_phase(true, &result) {
                                        job.status.phase = phase;
                                    }
                                    job.result = Some(match *result {
                                        Ok(Outcome::Done(message)) => message,
                                        Ok(_) => Message::new("传输已结束", "Transfer ended"),
                                        Err(error) => error.message(),
                                    });
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
        div().id("parallel-transfer-queue").test_support().flex().flex_col().flex_shrink_0().gap_1().p_2().border_t_1().border_color(rgb(visual.border)).child(header).child(div().text_color(rgb(visual.muted)).child(t(cx, "暂停保留槽位和路径锁；调低并发只影响后续任务。未知写入结果隔离目标，重连不会解除；可检查目标，再显式审核解除隔离的风险。不会重放。", "Paused jobs keep slots and path locks; lower concurrency applies to later jobs. Unknown writes isolate destinations across reconnects; inspect the target and explicitly review the risk of releasing isolation. Jobs never replay."))).when(!self.transfer_jobs.is_empty(), |view| view.child(list)).into_any_element()
    }
}

async fn queued_operation(
    queue: &TransferQueue,
    operation: Operation,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> Result<Outcome, FileFailure> {
    let (direction, existing) = match &operation {
        Operation::Upload(..) => (TransferDirection::Upload, None),
        Operation::Download(..) => (TransferDirection::Download, None),
        Operation::TransferDirectory(plan) => (plan.direction(), None),
        Operation::ResumeFile(plan) => (plan.direction(), Some((plan.existing_bytes(), false))),
        Operation::ResumeDirectory(plan) => (plan.direction(), Some((plan.existing_bytes(), true))),
        _ => return Err(FileFailure::WorkerStopped),
    };
    let mut transfer = tokio::select! {
        biased;
        _ = cancellation(&stop) => return Err(FileFailure::CancelledBeforeStart),
        result = async { match operation {
            Operation::Upload(local, remote) => queue.enqueue_atomic_upload(TransferSpec::upload(local, remote)).await,
            Operation::Download(remote, local) => queue.enqueue(TransferSpec::download(remote, local)).await,
            Operation::TransferDirectory(plan) => queue.enqueue_directory(plan).await,
            Operation::ResumeFile(plan) => queue.enqueue_resume(plan).await,
            Operation::ResumeDirectory(plan) => queue.enqueue_directory_resume(plan).await,
            _ => Err(SessionError::Worker),
        }} => result.map_err(FileFailure::from)?,
    };
    worker::observe_transfer(&mut transfer, direction, existing, stop, pause, progress).await
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
