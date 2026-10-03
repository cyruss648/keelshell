//! Transfer requests and acknowledged safe points, independent of UI state.
use super::*;
use tokio::sync::watch;

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
    id: u64,
    total: Option<u64>,
    events: mpsc::Sender<TransferEvent>,
    control: Arc<TransferControl>,
    transferred: AtomicU64,
    paused: watch::Sender<bool>,
    resume_generation: AtomicU64,
}
impl TransferContext {
    pub(super) fn new(
        id: u64,
        total: Option<u64>,
        events: mpsc::Sender<TransferEvent>,
        control: Arc<TransferControl>,
    ) -> Self {
        Self {
            id,
            total,
            events,
            control,
            transferred: AtomicU64::new(0),
            paused: watch::channel(false).0,
            resume_generation: AtomicU64::new(0),
        }
    }
    pub(super) fn resume_generation(&self) -> u64 {
        self.resume_generation.load(Ordering::Acquire)
    }
    pub(super) fn bytes(&self) -> u64 {
        self.transferred.load(Ordering::Acquire)
    }
    pub(super) async fn event(&self, event: TransferEvent) -> TransferExecutionResult<()> {
        tokio::select! {
            biased;
            _ = self.control.cancelled() => Err(TransferExecutionError::Cancelled(self.bytes())),
            result = self.events.send(event) => result.map_err(|_| SessionError::Closed.into()),
        }
    }
    /// Only call between fully acknowledged requests, never while a WRITE is pending.
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
    pub(super) async fn run<F>(
        &self,
        budget: Duration,
        label: &'static str,
        operation: F,
    ) -> TransferExecutionResult<()>
    where
        F: std::future::Future<Output = TransferExecutionResult<()>>,
    {
        tokio::pin!(operation);
        let mut paused = self.paused.subscribe();
        let mut remaining = budget;
        loop {
            let is_paused = *paused.borrow_and_update();
            let started = tokio::time::Instant::now();
            tokio::select! {
                biased;
                _ = self.control.cancelled() => return Err(TransferExecutionError::Cancelled(self.bytes())),
                result = &mut operation => return result,
                result = paused.changed() => { if result.is_err() { return Err(SessionError::Closed.into()); } }
                _ = tokio::time::sleep(remaining), if !is_paused => return Err(SessionError::Timeout(label).into()),
            }
            if !is_paused {
                remaining = remaining.saturating_sub(started.elapsed());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
