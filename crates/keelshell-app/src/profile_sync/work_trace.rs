//! Bounded test-only timings. No profile, path, password, result, or transport payload is retained.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Queued,
    AsyncEntered,
    BlockingEntered,
    CoreReturned,
    BlockingReturned,
    BlockingJoined,
    ForegroundReceived,
    CallbackEntered,
}

pub(super) struct WorkTrace {
    operation: &'static str,
    began: Instant,
    offsets: [AtomicU64; 8],
}

impl WorkTrace {
    pub(super) fn new(operation: &'static str) -> Self {
        let trace = Self {
            operation,
            began: Instant::now(),
            offsets: std::array::from_fn(|_| AtomicU64::new(0)),
        };
        trace.mark(Phase::Queued);
        trace
    }

    pub(super) fn mark(&self, phase: Phase) {
        let micros = u64::try_from(self.began.elapsed().as_micros())
            .unwrap_or(u64::MAX - 1)
            .saturating_add(1);
        // Each stage is recorded once. Zero means that stage has not run; one
        // preserves a real zero-microsecond observation without inventing timing.
        let _ = self.offsets[phase as usize].compare_exchange(
            0,
            micros,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            operation: self.operation,
            elapsed_us: self.began.elapsed().as_micros(),
            offsets_us: std::array::from_fn(|index| {
                self.offsets[index].load(Ordering::Acquire).checked_sub(1)
            }),
        }
    }
}

#[derive(Debug)]
pub(super) struct Snapshot {
    pub(super) operation: &'static str,
    pub(super) elapsed_us: u128,
    /// Ordered as queued, async-entered, blocking-entered, core-returned,
    /// blocking-returned, blocking-joined, foreground-received, callback-entered.
    pub(super) offsets_us: [Option<u64>; 8],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unobserved_phases_remain_absent_and_repeated_marks_do_not_move_first_time() {
        let trace = WorkTrace::new("disable");
        let original = trace.snapshot();
        assert_eq!(original.operation, "disable");
        assert!(original.offsets_us[0].is_some());
        assert!(original.offsets_us[1..].iter().all(Option::is_none));
        trace.mark(Phase::AsyncEntered);
        let first = trace.snapshot().offsets_us[1];
        trace.mark(Phase::AsyncEntered);
        assert_eq!(trace.snapshot().offsets_us[1], first);
        assert!(trace.snapshot().elapsed_us >= original.elapsed_us);
    }

    #[test]
    fn worker_phases_are_visible_after_join_without_a_ui_callback() {
        let trace = std::sync::Arc::new(WorkTrace::new("apply"));
        let worker = trace.clone();
        let thread = std::thread::spawn(move || {
            for phase in [
                Phase::AsyncEntered,
                Phase::BlockingEntered,
                Phase::CoreReturned,
                Phase::BlockingReturned,
                Phase::BlockingJoined,
            ] {
                worker.mark(phase);
            }
        });
        assert!(thread.join().is_ok());
        let snapshot = trace.snapshot();
        assert!(snapshot.offsets_us[..6].iter().all(Option::is_some));
        assert!(snapshot.offsets_us[6..].iter().all(Option::is_none));
        for pair in snapshot.offsets_us[..6].windows(2) {
            assert!(pair[0] <= pair[1], "actual worker order: {snapshot:?}");
        }
        trace.mark(Phase::ForegroundReceived);
        trace.mark(Phase::CallbackEntered);
        assert!(trace.snapshot().offsets_us.iter().all(Option::is_some));
    }
}
