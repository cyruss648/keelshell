//! Test-only, fixed-size observations of real sync work; never a completion signal.

use super::{ProfileSyncError, Report};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ResultCategory {
    Review,
    Published,
    State,
    Cancelled,
    Stale,
    Busy,
    Pending,
    Storage,
    Io,
    Invalid,
    Replay,
    Channel,
    Incomplete,
}

impl ResultCategory {
    pub(super) fn from_result(result: &Result<Report, ProfileSyncError>) -> Self {
        match result {
            Ok(Report::Review(_)) => Self::Review,
            Ok(Report::State(_, true)) => Self::Published,
            Ok(Report::State(_, false)) => Self::State,
            Err(ProfileSyncError::Cancelled) => Self::Cancelled,
            Err(ProfileSyncError::Stale) => Self::Stale,
            Err(ProfileSyncError::Busy) => Self::Busy,
            Err(ProfileSyncError::Pending) => Self::Pending,
            Err(ProfileSyncError::Storage(_)) => Self::Storage,
            Err(ProfileSyncError::Io(_)) => Self::Io,
            Err(ProfileSyncError::Invalid) => Self::Invalid,
            Err(ProfileSyncError::Replay) => Self::Replay,
            Err(ProfileSyncError::Channel) => Self::Channel,
            Err(ProfileSyncError::Incomplete) => Self::Incomplete,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) operation: u64,
    pub(super) submitted: Duration,
    pub(super) age: Duration,
    pub(super) blocking_started: Option<Duration>,
    pub(super) service_returned: Option<Duration>,
    pub(super) foreground_completed: Option<Duration>,
    pub(super) service_result: Option<ResultCategory>,
    pub(super) foreground_result: Option<ResultCategory>,
}

#[derive(Default)]
struct Progress {
    blocking_started: Option<Duration>,
    service_returned: Option<(Duration, ResultCategory)>,
    foreground_completed: Option<(Duration, ResultCategory)>,
}

pub(super) struct OperationObservation {
    operation: u64,
    submitted: Instant,
    progress: Mutex<Progress>,
}

impl OperationObservation {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            // This bounded test-run label is diagnostic only. Arc identity owns
            // attribution, so the numeric counter is never an authority token.
            operation: NEXT_OPERATION.fetch_add(1, Ordering::Relaxed),
            submitted: Instant::now(),
            progress: Mutex::new(Progress::default()),
        })
    }

    pub(super) fn blocking_started(&self) {
        let mut progress = self.progress.lock().unwrap_or_else(|p| p.into_inner());
        progress
            .blocking_started
            .get_or_insert_with(|| self.submitted.elapsed());
    }

    pub(super) fn service_returned(&self, category: ResultCategory) {
        let mut progress = self.progress.lock().unwrap_or_else(|p| p.into_inner());
        progress
            .service_returned
            .get_or_insert_with(|| (self.submitted.elapsed(), category));
    }

    pub(super) fn foreground_completed(
        self: &Arc<Self>,
        current: Option<&Arc<Self>>,
        category: ResultCategory,
    ) {
        // Guard only attribution. The caller always executes normal completion,
        // even if a detached/older observation no longer belongs to the panel.
        if current.is_some_and(|current| Arc::ptr_eq(current, self)) {
            let mut progress = self.progress.lock().unwrap_or_else(|p| p.into_inner());
            progress
                .foreground_completed
                .get_or_insert_with(|| (self.submitted.elapsed(), category));
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let progress = self.progress.lock().unwrap_or_else(|p| p.into_inner());
        Snapshot {
            operation: self.operation,
            submitted: Duration::ZERO,
            age: self.submitted.elapsed(),
            blocking_started: progress.blocking_started,
            service_returned: progress.service_returned.map(|(time, _)| time),
            foreground_completed: progress.foreground_completed.map(|(time, _)| time),
            service_result: progress.service_returned.map(|(_, category)| category),
            foreground_result: progress.foreground_completed.map(|(_, category)| category),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread};

    #[test]
    fn held_worker_separates_queued_running_and_returned_before_foreground() {
        let observation = OperationObservation::new();
        let worker = Arc::clone(&observation);
        let (start_tx, start_rx) = mpsc::sync_channel(1);
        let (return_tx, return_rx) = mpsc::sync_channel(1);
        let (phase_tx, phase_rx) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            start_rx.recv_timeout(Duration::from_secs(2)).checked();
            worker.blocking_started();
            phase_tx.send(()).checked();
            return_rx.recv_timeout(Duration::from_secs(2)).checked();
            worker.service_returned(ResultCategory::Review);
            phase_tx.send(()).checked();
        });
        let queued = observation.snapshot();
        assert!(queued.blocking_started.is_none());
        assert!(queued.service_returned.is_none());
        assert!(queued.foreground_completed.is_none());
        start_tx.send(()).checked();
        phase_rx.recv_timeout(Duration::from_secs(2)).checked();
        let running = observation.snapshot();
        assert!(running.blocking_started.is_some());
        assert!(running.service_returned.is_none());
        assert!(running.foreground_completed.is_none());
        return_tx.send(()).checked();
        phase_rx.recv_timeout(Duration::from_secs(2)).checked();
        let returned = observation.snapshot();
        assert!(returned.service_returned.is_some());
        assert_eq!(returned.service_result, Some(ResultCategory::Review));
        assert!(returned.foreground_completed.is_none());
        observation.foreground_completed(Some(&observation), ResultCategory::Review);
        let completed = observation.snapshot();
        assert!(completed.foreground_completed.is_some());
        assert_eq!(completed.foreground_result, Some(ResultCategory::Review));
        assert!(completed.submitted <= completed.blocking_started.checked());
        assert!(completed.blocking_started <= completed.service_returned);
        assert!(completed.service_returned <= completed.foreground_completed);
        assert!(completed.foreground_completed.checked() <= completed.age);
        thread.join().checked();
    }

    #[test]
    fn detached_old_job_and_late_completion_cannot_update_current_operation() {
        let old = OperationObservation::new();
        old.blocking_started();
        let current = OperationObservation::new();
        old.service_returned(ResultCategory::Cancelled);
        old.foreground_completed(None, ResultCategory::Cancelled);
        old.foreground_completed(Some(&current), ResultCategory::Cancelled);
        let new_snapshot = current.snapshot();
        assert_ne!(old.snapshot().operation, new_snapshot.operation);
        assert!(old.snapshot().foreground_completed.is_none());
        assert!(new_snapshot.blocking_started.is_none());
        assert!(new_snapshot.service_returned.is_none());
        assert!(new_snapshot.foreground_completed.is_none());
        assert!(new_snapshot.service_result.is_none());
        current.blocking_started();
        current.service_returned(ResultCategory::Published);
        current.foreground_completed(Some(&current), ResultCategory::Published);
        assert_eq!(
            current.snapshot().foreground_result,
            Some(ResultCategory::Published)
        );
        assert_eq!(
            old.snapshot().service_result,
            Some(ResultCategory::Cancelled)
        );
    }

    trait Checked<T> {
        fn checked(self) -> T;
    }
    impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
        #[track_caller]
        fn checked(self) -> T {
            match self {
                Ok(value) => value,
                Err(error) => panic!("bounded observation fixture: {error:?}"),
            }
        }
    }
    impl<T> Checked<T> for Option<T> {
        #[track_caller]
        fn checked(self) -> T {
            match self {
                Some(value) => value,
                None => panic!("missing observation fixture stage"),
            }
        }
    }
}
