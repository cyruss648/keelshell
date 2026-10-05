//! Tokio runs network futures; only GPUI's executor wakes foreground tasks.

use std::{future::Future, sync::mpsc, time::Duration};

use gpui_kit::BackgroundExecutor;
use keelshell_ai::{LocalAskProgressReceiver, LocalAskStage};
use tokio::runtime::Runtime;

/// Deliver one completion without registering a GPUI waker on a Tokio task.
/// Dropping the receiver stops polling; the caller owns request cancellation.
pub(crate) fn spawn<T: Send + 'static>(
    runtime: &Runtime,
    executor: BackgroundExecutor,
    operation: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = Result<T, mpsc::RecvError>> + 'static {
    let (sender, receiver) = mpsc::sync_channel(1);
    runtime.spawn(async move {
        // There is exactly one result and one slot. A closed receiver means
        // the UI cancelled its wait; never block a Tokio worker on delivery.
        let _ = sender.try_send(operation.await);
    });
    async move {
        loop {
            match receiver.try_recv() {
                Ok(result) => return Ok(result),
                Err(mpsc::TryRecvError::Disconnected) => return Err(mpsc::RecvError),
                Err(mpsc::TryRecvError::Empty) => {
                    // A foreign worker must not wake GPUI's deterministic test
                    // scheduler. Keep wakeups within its own executor instead.
                    executor.timer(Duration::from_millis(16)).await;
                }
            }
        }
    }
}

pub(crate) enum LocalAskEvent<T> {
    Stage(LocalAskStage),
    Complete(T),
}

/// Foreground polling owns both receivers. Only its GPUI timer registers a waker;
/// neither lifecycle observations nor the Tokio result can wake the UI directly.
pub(crate) struct LocalAskBridge<T> {
    executor: BackgroundExecutor,
    progress: LocalAskProgressReceiver,
    result: mpsc::Receiver<T>,
}

pub(crate) fn spawn_local_ask<T: Send + 'static>(
    runtime: &Runtime,
    executor: BackgroundExecutor,
    progress: LocalAskProgressReceiver,
    operation: impl Future<Output = T> + Send + 'static,
) -> LocalAskBridge<T> {
    let (sender, result) = mpsc::sync_channel(1);
    runtime.spawn(async move {
        let _ = sender.try_send(operation.await);
    });
    LocalAskBridge {
        executor,
        progress,
        result,
    }
}

impl<T> LocalAskBridge<T> {
    pub(crate) async fn next(&self) -> Result<LocalAskEvent<T>, mpsc::RecvError> {
        loop {
            // At most eight once-only observations exist. Drain them before the
            // completion so a fast child does not erase its observed history.
            if let Ok(stage) = self.progress.try_recv() {
                return Ok(LocalAskEvent::Stage(stage));
            }
            match self.result.try_recv() {
                Ok(result) => return Ok(LocalAskEvent::Complete(result)),
                Err(mpsc::TryRecvError::Disconnected) => return Err(mpsc::RecvError),
                Err(mpsc::TryRecvError::Empty) => {
                    self.executor.timer(Duration::from_millis(16)).await;
                }
            }
        }
    }
}
