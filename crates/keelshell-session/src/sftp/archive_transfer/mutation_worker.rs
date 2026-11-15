//! Ownership of a blocking mutation through its observed result handoff.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::queue;
use crate::{Result, SessionError};

pub(super) struct MutationWorker {
    pub(super) ticket: queue::Reservation,
    pub(super) stop: Arc<AtomicBool>,
}

impl MutationWorker {
    pub(super) fn check(&self) -> Result<()> {
        if self.stop.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        if !self.ticket.is_active() {
            return Err(SessionError::MutationQuarantined);
        }
        Ok(())
    }
}

pub(super) fn submit_mutation<T: Send + 'static>(
    pending: Arc<AtomicBool>,
    worker: MutationWorker,
    operation: impl FnOnce(&MutationWorker) -> Result<T> + Send + 'static,
) -> tokio::task::JoinHandle<Result<T>> {
    submit_at_boundary(pending, worker, operation, || {})
}

fn submit_at_boundary<T: Send + 'static>(
    pending: Arc<AtomicBool>,
    worker: MutationWorker,
    operation: impl FnOnce(&MutationWorker) -> Result<T> + Send + 'static,
    before_return: impl FnOnce() + Send + 'static,
) -> tokio::task::JoinHandle<Result<T>> {
    pending.store(true, Ordering::Release);
    tokio::task::spawn_blocking(move || {
        let result = worker.check().and_then(|()| operation(&worker));
        // Only the async owner may acknowledge this result. Clearing pending
        // here would leave a live, unobserved output on a detached worker while
        // scope completion incorrectly reports an ordinary transport failure.
        before_return();
        result
    })
}

pub(super) async fn observe_mutation<T>(
    pending: &AtomicBool,
    task: &mut tokio::task::JoinHandle<Result<T>>,
) -> Result<T> {
    // A panic or abandoned await must keep the mutation obligation visible.
    let result = task.await.map_err(|_| SessionError::Worker)?;
    pending.store(false, Ordering::Release);
    result
}

#[cfg(test)]
#[path = "mutation_worker/tests.rs"]
mod tests;
