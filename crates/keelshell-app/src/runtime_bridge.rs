//! Tokio runs network futures; only GPUI's executor wakes foreground tasks.

use std::{future::Future, sync::mpsc, time::Duration};

use gpui_kit::BackgroundExecutor;
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
