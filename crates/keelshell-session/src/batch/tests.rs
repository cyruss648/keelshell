use super::*;
#[test]
fn only_a_complete_zero_exit_is_success() {
    assert!(BatchOutcome::Exited { code: 0 }.is_success());
    for outcome in [
        BatchOutcome::Exited { code: 1 },
        BatchOutcome::Rejected,
        BatchOutcome::Unknown {
            reason: BatchUnknownReason::NoExitStatus,
        },
        BatchOutcome::NotStarted {
            reason: BatchNotStartedReason::Cancelled,
        },
    ] {
        assert!(!outcome.is_success());
    }
}
#[test]
fn empty_batch_is_rejected_before_a_runtime_is_required() {
    assert!(matches!(
        start_batch(Vec::new(), BatchOptions::default()),
        Err(BatchError::InvalidTargets)
    ));
}
