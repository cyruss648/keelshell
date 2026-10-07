//! The request owner aborts its Tokio task; foreground polling never owns I/O.

use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use gpui_kit::BackgroundExecutor;
use tokio::{runtime::Runtime, task::AbortHandle};

pub(super) struct Worker {
    abort: AbortHandle,
    pub(super) cancelled: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.abort.abort();
    }
}

pub(super) fn spawn<T: Send + 'static>(
    runtime: &Runtime,
    executor: BackgroundExecutor,
    cancelled: Arc<AtomicBool>,
    operation: impl Future<Output = T> + Send + 'static,
) -> (
    Worker,
    impl Future<Output = Result<T, mpsc::RecvError>> + 'static,
) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = runtime.spawn(async move {
        // Failed delivery drops any staged payload and its cleanup guard.
        let _ = sender.try_send(operation.await);
    });
    let worker = Worker {
        abort: handle.abort_handle(),
        cancelled,
    };
    let completion = async move {
        loop {
            match receiver.try_recv() {
                Ok(result) => return Ok(result),
                Err(mpsc::TryRecvError::Disconnected) => return Err(mpsc::RecvError),
                Err(mpsc::TryRecvError::Empty) => executor.timer(Duration::from_millis(16)).await,
            }
        }
    };
    (worker, completion)
}
