//! Transfer requests and acknowledged safe points, independent of UI state.
use super::*;
use tokio::sync::watch;

tokio::task_local! {
    // Bound only while this owner's operation future is polled. Sibling
    // futures, spawned cleanup and other transfers do not inherit it.
    static CURRENT_IO: watch::Sender<()>;
}

/// Report an actual completed reply within the current transfer poll scope.
/// This never clears a pending destination mutation or changes authority.
pub(super) fn observed_io() {
    let _ = CURRENT_IO.try_with(|sender| sender.send_replace(()));
}

/// Observe a read-only/descriptor protocol response without mapping its error.
/// A matched STATUS (including a missing path) is an actual reply; transport
/// timeouts and malformed replies are not. Callers retain their own semantics.
pub(super) async fn remote_io<T>(
    operation: impl std::future::Future<
        Output = std::result::Result<T, russh_sftp::client::error::Error>,
    >,
) -> std::result::Result<T, russh_sftp::client::error::Error> {
    let result = operation.await;
    if result.is_ok() || matches!(&result, Err(russh_sftp::client::error::Error::Status(_))) {
        observed_io();
    }
    result
}

/// Observe actual local metadata/source completion, including a missing path.
/// Pending writes still belong to `local_mutation`; this helper cannot resolve
/// an uncertain mutation merely because another read completed.
pub(super) async fn local_io<T>(
    operation: impl std::future::Future<Output = std::io::Result<T>>,
) -> std::io::Result<T> {
    let result = operation.await;
    if result.is_ok()
        || matches!(&result, Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        observed_io();
    }
    result
}

pub(super) struct TransferControl {
    pub(super) cancelled: watch::Sender<bool>,
    requested_pause: watch::Sender<bool>,
}
impl TransferControl {
    pub(super) fn new() -> Self {
        Self {
            cancelled: watch::channel(false).0,
            requested_pause: watch::channel(false).0,
        }
    }
    pub(super) fn cancel(&self) {
        self.cancelled.send_replace(true);
    }
    pub(super) fn pause(&self, paused: bool) {
        self.requested_pause.send_replace(paused);
    }
    pub(super) async fn cancelled(&self) {
        let mut receiver = self.cancelled.subscribe();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

pub(super) struct TransferContext {
    pub(super) reservation: Option<queue::Reservation>,
    id: u64,
    total: Option<u64>,
    events: mpsc::Sender<TransferEvent>,
    control: Arc<TransferControl>,
    transferred: AtomicU64,
    paused: watch::Sender<bool>,
    resume_generation: AtomicU64,
    confirmed_io: watch::Sender<()>,
    validating: watch::Sender<bool>,
    pub(super) mutation_pending: Arc<std::sync::atomic::AtomicBool>,
}

struct ValidationPhase<'a>(&'a watch::Sender<bool>);
impl Drop for ValidationPhase<'_> {
    fn drop(&mut self) {
        self.0.send_replace(false);
    }
}

impl TransferContext {
    pub(super) fn new(
        id: u64,
        total: Option<u64>,
        events: mpsc::Sender<TransferEvent>,
        control: Arc<TransferControl>,
    ) -> Self {
        Self {
            reservation: None,
            id,
            total,
            events,
            control,
            transferred: AtomicU64::new(0),
            paused: watch::channel(false).0,
            resume_generation: AtomicU64::new(0),
            confirmed_io: watch::channel(()).0,
            validating: watch::channel(false).0,
            mutation_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub(super) fn resume_generation(&self) -> u64 {
        self.resume_generation.load(Ordering::Acquire)
    }
    pub(super) fn bytes(&self) -> u64 {
        self.transferred.load(Ordering::Acquire)
    }
    pub(super) fn mutation_pending(&self) -> bool {
        self.mutation_pending.load(Ordering::Acquire)
    }
    /// Only completed protocol replies or actual local I/O renew the idle wait.
    /// A UI event, safe-point poll, or another transfer's activity cannot do so.
    pub(super) fn confirmed_io(&self) {
        self.confirmed_io.send_replace(());
    }
    /// Full read-only scans keep their existing fixed deadline. They cannot
    /// renew it by producing replies, and cannot contain destination mutations.
    pub(super) async fn validation<T>(
        &self,
        budget: Duration,
        label: &'static str,
        operation: impl std::future::Future<Output = Result<T>>,
    ) -> TransferExecutionResult<T> {
        self.validating.send_replace(true);
        let _phase = ValidationPhase(&self.validating);
        let result = deadline(budget, label, operation).await;
        if result.is_ok() {
            self.confirmed_io();
        }
        result.map_err(Into::into)
    }
    /// Keep the marker set when cancellation drops an in-flight mutation. Only
    /// a protocol success or explicit STATUS rejection proves its reply arrived.
    pub(super) async fn remote_mutation<T>(
        &self,
        operation: impl std::future::Future<
            Output = std::result::Result<T, russh_sftp::client::error::Error>,
        >,
    ) -> TransferExecutionResult<T> {
        self.mutation_pending.store(true, Ordering::Release);
        let result = operation.await;
        if result.is_ok() || matches!(&result, Err(russh_sftp::client::error::Error::Status(_))) {
            self.mutation_pending.store(false, Ordering::Release);
        }
        if result.is_ok() {
            self.confirmed_io();
        }
        result.map_err(sftp_error).map_err(Into::into)
    }
    /// File creation/write/flush futures may outlive their cancelled caller.
    /// Returning from the whole operation is the local completion boundary.
    pub(super) async fn local_mutation<T>(
        &self,
        operation: impl std::future::Future<Output = std::io::Result<T>>,
    ) -> TransferExecutionResult<T> {
        self.mutation_pending.store(true, Ordering::Release);
        let result = operation.await;
        self.mutation_pending.store(false, Ordering::Release);
        if result.is_ok() {
            self.confirmed_io();
        }
        result.map_err(Into::into)
    }
    pub(super) async fn event(&self, event: TransferEvent) -> TransferExecutionResult<()> {
        tokio::select! {
            biased;
            _ = self.control.cancelled() => Err(TransferExecutionError::Cancelled(self.bytes())),
            result = self.events.send(event) => result.map_err(|_| SessionError::Closed.into()),
        }
    }
    /// Only call between fully acknowledged destination requests, including
    /// writable CLOSE; acknowledged bytes alone do not establish that boundary.
    pub(super) async fn checkpoint(&self) -> TransferExecutionResult<()> {
        if *self.control.cancelled.borrow() {
            return Err(TransferExecutionError::Cancelled(self.bytes()));
        }
        let mut desired = self.control.requested_pause.subscribe();
        if !*desired.borrow_and_update() {
            return Ok(());
        }
        self.paused.send_replace(true);
        self.event(TransferEvent::Paused {
            id: self.id,
            transferred: self.bytes(),
            total: self.total,
        })
        .await?;
        loop {
            if !*desired.borrow_and_update() {
                break;
            }
            tokio::select! {
                biased;
                _ = self.control.cancelled() => return Err(TransferExecutionError::Cancelled(self.bytes())),
                result = desired.changed() => { if result.is_err() { return Err(SessionError::Closed.into()); } }
            }
        }
        self.resume_generation.fetch_add(1, Ordering::AcqRel);
        self.paused.send_replace(false);
        self.event(TransferEvent::Resumed {
            id: self.id,
            transferred: self.bytes(),
            total: self.total,
        })
        .await
    }
    pub(super) async fn progress(&self, delta: u64) -> TransferExecutionResult<()> {
        let next = self
            .bytes()
            .checked_add(delta)
            .ok_or(SessionError::Invalid("transfer exceeds u64 size"))?;
        self.transferred.store(next, Ordering::Release);
        // Leave room for control acknowledgements; slow consumers can skip
        // intermediate progress. Terminal results use a separate one-shot lane.
        if self.events.capacity() > 4 {
            let _ = self.events.try_send(TransferEvent::Progress {
                id: self.id,
                transferred: next,
                total: self.total,
            });
        }
        self.checkpoint().await
    }
    pub(super) async fn run<F, T>(
        &self,
        budget: Duration,
        label: &'static str,
        operation: F,
    ) -> TransferExecutionResult<T>
    where
        F: std::future::Future<Output = TransferExecutionResult<T>>,
    {
        let operation = CURRENT_IO.scope(self.confirmed_io.clone(), operation);
        tokio::pin!(operation);
        let mut paused = self.paused.subscribe();
        let mut confirmed = self.confirmed_io.subscribe();
        let mut validating = self.validating.subscribe();
        let mut remaining = budget;
        loop {
            let user_paused = *paused.borrow_and_update();
            let read_only_validation = *validating.borrow_and_update();
            let idle_suspended = user_paused || read_only_validation;
            let started = tokio::time::Instant::now();
            tokio::select! {
                biased;
                _ = self.control.cancelled() => return Err(TransferExecutionError::Cancelled(self.bytes())),
                result = &mut operation => return result,
                result = paused.changed() => { if result.is_err() { return Err(SessionError::Closed.into()); } }
                result = validating.changed() => { if result.is_err() { return Err(SessionError::Closed.into()); } }
                result = confirmed.changed() => {
                    if result.is_err() { return Err(SessionError::Closed.into()); }
                    remaining = budget;
                    continue;
                }
                _ = tokio::time::sleep(remaining), if !idle_suspended => return Err(SessionError::Timeout(label).into()),
            }
            if !idle_suspended {
                remaining = remaining.saturating_sub(started.elapsed());
            }
        }
    }
}

#[cfg(test)]
#[path = "control/metadata_tests.rs"]
mod metadata_tests;

#[cfg(test)]
#[path = "control/independent_metadata_tests.rs"]
mod independent_metadata_tests;

#[cfg(test)]
#[path = "control/cancellation_type_tests.rs"]
mod cancellation_type_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> (TransferContext, mpsc::Receiver<TransferEvent>) {
        let (events, receiver) = mpsc::channel(64);
        (
            TransferContext::new(1, None, events, Arc::new(TransferControl::new())),
            receiver,
        )
    }

    #[tokio::test]
    async fn confirmed_local_io_renews_idle_without_a_total_transfer_limit() -> Result<()> {
        tokio::time::timeout(Duration::from_secs(2), async {
            let (context, _receiver) = context();
            let began = tokio::time::Instant::now();
            context
                .run(Duration::from_millis(100), "idle", async {
                    for _ in 0..8 {
                        context
                            .local_mutation(async {
                                tokio::time::sleep(Duration::from_millis(25)).await;
                                Ok(())
                            })
                            .await?;
                    }
                    Ok(())
                })
                .await
                .map_err(transfer_session_error)?;
            assert!(began.elapsed() >= Duration::from_millis(200));
            Ok(())
        })
        .await
        .map_err(|_| SessionError::Timeout("test hard bound"))?
    }

    #[tokio::test]
    async fn safe_points_and_presentation_events_cannot_renew_idle() -> Result<()> {
        tokio::time::timeout(Duration::from_secs(2), async {
            let (context, _receiver) = context();
            let result = context
                .run::<_, ()>(Duration::from_millis(80), "idle", async {
                    loop {
                        context.checkpoint().await?;
                        context
                            .event(TransferEvent::Progress {
                                id: 1,
                                transferred: 1,
                                total: None,
                            })
                            .await?;
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await;
            assert!(matches!(
                result,
                Err(TransferExecutionError::Error(SessionError::Timeout("idle")))
            ));
            assert!(!context.mutation_pending());
            Ok(())
        })
        .await
        .map_err(|_| SessionError::Timeout("test hard bound"))?
    }

    #[tokio::test]
    async fn another_owners_confirmations_cannot_extend_a_pending_mutation() -> Result<()> {
        tokio::time::timeout(Duration::from_secs(2), async {
        let (context, _receiver) = context();
        let (other, _other_receiver) = self::context();
        let work = async {
            loop {
                other.confirmed_io();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::pin!(work);
        let result = tokio::select! {
            result = context.run(Duration::from_millis(80), "idle", context.remote_mutation(std::future::pending::<std::result::Result<(), russh_sftp::client::error::Error>>())) => result,
            () = &mut work => return Err(SessionError::Worker),
        };
        assert!(matches!(
            result,
            Err(TransferExecutionError::Error(SessionError::Timeout("idle")))
        ));
        assert!(context.mutation_pending());
        assert!(!other.mutation_pending());
        Ok(())
    }).await.map_err(|_| SessionError::Timeout("test hard bound"))?
    }

    #[tokio::test]
    async fn a_fixed_read_only_validation_can_outlive_idle_but_cannot_renew_itself() -> Result<()> {
        tokio::time::timeout(Duration::from_secs(2), async {
            let (context, _receiver) = context();
            context
                .run(Duration::from_millis(50), "idle", async {
                    context
                        .validation(Duration::from_millis(250), "validation", async {
                            tokio::time::sleep(Duration::from_millis(130)).await;
                            Ok(())
                        })
                        .await?;
                    context
                        .local_mutation(async {
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Ok(())
                        })
                        .await
                })
                .await
                .map_err(transfer_session_error)?;
            let began = tokio::time::Instant::now();
            let result = context
                .run(Duration::from_millis(20), "idle", async {
                    context
                        .validation::<()>(Duration::from_millis(80), "validation", async {
                            loop {
                                context.confirmed_io();
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        })
                        .await
                })
                .await;
            assert!(matches!(
                result,
                Err(TransferExecutionError::Error(SessionError::Timeout(
                    "validation"
                )))
            ));
            assert!(began.elapsed() < Duration::from_millis(300));
            assert!(!*context.validating.borrow());
            assert!(!context.mutation_pending());
            Ok(())
        })
        .await
        .map_err(|_| SessionError::Timeout("test hard bound"))?
    }

    #[tokio::test]
    async fn acknowledged_pause_excludes_wall_time_and_cancellation_still_interrupts() -> Result<()>
    {
        tokio::time::timeout(Duration::from_secs(2), async {
            let (context, mut receiver) = context();
            let controller = async {
                loop {
                    if matches!(receiver.recv().await, Some(TransferEvent::Paused { .. })) {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(180)).await;
                context.control.pause(false);
            };
            let operation = async {
                context.local_mutation(async { Ok(()) }).await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
                context.control.pause(true);
                context.checkpoint().await?;
                context
                    .local_mutation(async {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(())
                    })
                    .await
            };
            let (result, ()) = tokio::join!(
                context.run(Duration::from_millis(80), "idle", operation),
                controller
            );
            result.map_err(transfer_session_error)?;
            assert_eq!(context.resume_generation(), 1);
            let cancel = async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                context.control.cancel();
            };
            let (result, ()) = tokio::join!(
                context.run(
                    Duration::from_millis(80),
                    "idle",
                    context.validation(
                        Duration::from_secs(1),
                        "validation",
                        std::future::pending::<Result<()>>()
                    )
                ),
                cancel
            );
            assert!(matches!(result, Err(TransferExecutionError::Cancelled(0))));
            assert!(!*context.validating.borrow());
            Ok(())
        })
        .await
        .map_err(|_| SessionError::Timeout("test hard bound"))?
    }

    #[tokio::test]
    async fn cancellation_interrupts_a_full_control_event_queue()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (sender, _receiver) = mpsc::channel(2);
        sender.try_send(TransferEvent::Queued { id: 1 })?;
        sender.try_send(TransferEvent::Started {
            id: 1,
            total: Some(10),
        })?;
        let control = Arc::new(TransferControl::new());
        control.pause(true);
        let context = Arc::new(TransferContext::new(1, Some(10), sender, control.clone()));
        let worker = tokio::spawn(async move {
            context
                .run(
                    Duration::from_millis(100),
                    "test transfer",
                    context.checkpoint(),
                )
                .await
        });
        tokio::task::yield_now().await;
        control.cancel();
        let result = tokio::time::timeout(Duration::from_millis(250), worker).await??;
        assert!(matches!(result, Err(TransferExecutionError::Cancelled(0))));
        Ok(())
    }
}
