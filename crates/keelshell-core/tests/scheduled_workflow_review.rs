//! Independent public-API counterexamples for finite schedule authorization.
use std::time::Duration;

use keelshell_core::{
    BatchTaskSpec, BatchWorkflowPlan, ScheduleClockSample, WorkflowScheduleBinding,
    WorkflowScheduleError as Error, WorkflowScheduleInvalidationReason as Reason,
    WorkflowScheduleLedger, WorkflowScheduleOutcome as Outcome, WorkflowScheduleSlotStatus as Slot,
    WorkflowScheduleSpec, WorkflowScheduleStatus as Status, format_fixed_offset_datetime,
    parse_fixed_offset_datetime,
};
use uuid::Uuid;

fn checked<T, E: std::fmt::Debug>(value: Result<T, E>) -> T {
    value.unwrap_or_else(|error| panic!("valid independent fixture: {error:?}"))
}

fn sample(wall_millis: i64, elapsed_millis: u64) -> ScheduleClockSample {
    ScheduleClockSample {
        utc_millis: wall_millis,
        monotonic: Duration::from_millis(elapsed_millis),
    }
}

fn spec(first: i64, grace: u32, interval: u64, count: u32) -> WorkflowScheduleSpec {
    let plan = checked(BatchWorkflowPlan::new(vec![BatchTaskSpec {
        id: Uuid::from_u128(1),
        target_id: Uuid::from_u128(2),
        command: "printf 'reviewed'".into(),
        dependencies: vec![],
    }]));
    checked(WorkflowScheduleSpec::interval(
        Uuid::new_v4(),
        checked(WorkflowScheduleBinding::new(
            Uuid::from_u128(3),
            7,
            plan.review_token(),
        )),
        first,
        0,
        grace,
        interval,
        count,
    ))
}

#[test]
fn independent_schedule_all_thirty_two_authorized_slots_run_once_and_exhaust() {
    let mut ledger = checked(WorkflowScheduleLedger::new(
        spec(10, 1, 60, 32),
        sample(0, 0),
    ));
    for index in 0..32 {
        let millis = (10 + i64::from(index) * 60) * 1_000;
        let token = checked(ledger.tick(sample(millis, millis as u64)))
            .unwrap_or_else(|| panic!("authorized slot {index}"));
        assert_eq!(token.slot_index(), index);
        checked(ledger.claim(&token, sample(millis + 1_000, (millis + 1_000) as u64)));
        checked(ledger.finish(&token, Outcome::Succeeded));
        assert_eq!(
            checked(ledger.tick(sample(millis + 1_000, (millis + 1_000) as u64))),
            None
        );
    }
    assert_eq!(ledger.status(), Status::Complete);
    assert!(
        ledger
            .slots()
            .iter()
            .all(|slot| slot.status() == Slot::Finished(Outcome::Succeeded))
    );
    assert_eq!(checked(ledger.tick(sample(604_800_000, 604_800_000))), None);
}

#[test]
fn independent_schedule_expired_ticket_cannot_complete_a_later_running_slot() {
    let mut ledger = checked(WorkflowScheduleLedger::new(
        spec(10, 1, 60, 3),
        sample(0, 0),
    ));
    let old = checked(ledger.tick(sample(10_000, 10_000)))
        .unwrap_or_else(|| panic!("first offered ticket"));
    let next = checked(ledger.tick(sample(70_000, 70_000)))
        .unwrap_or_else(|| panic!("second offered ticket"));
    checked(ledger.claim(&next, sample(70_000, 70_000)));
    assert_eq!(
        ledger.claim(&old, sample(70_000, 70_000)),
        Err(Error::InvalidTransition)
    );
    assert_eq!(
        ledger.finish(&old, Outcome::Succeeded),
        Err(Error::InvalidTransition)
    );
    assert_eq!(ledger.slots()[0].status(), Slot::Missed);
    assert_eq!(ledger.slots()[1].status(), Slot::Running);
    checked(ledger.finish(&next, Outcome::Succeeded));
    assert_eq!(ledger.slots()[2].status(), Slot::Pending);
}

#[test]
fn independent_schedule_claim_time_clock_regression_permanently_withdraws_authority() {
    for (claim, reason) in [
        (sample(9_999, 10_001), Reason::WallClockMovedBackward),
        (sample(10_001, 9_999), Reason::MonotonicMovedBackward),
        (sample(12_001, 10_000), Reason::ClockDrift),
    ] {
        let mut ledger = checked(WorkflowScheduleLedger::new(
            spec(10, 60, 60, 2),
            sample(0, 0),
        ));
        let token =
            checked(ledger.tick(sample(10_000, 10_000))).unwrap_or_else(|| panic!("due ticket"));
        assert_eq!(
            ledger.claim(&token, claim),
            Err(Error::ClockInvalidated { reason })
        );
        assert_eq!(ledger.status(), Status::Invalidated(reason));
        assert_eq!(checked(ledger.tick(sample(70_000, 70_000))), None);
        assert_eq!(
            ledger.claim(&token, sample(70_000, 70_000)),
            Err(Error::NoLongerActive)
        );
    }
}

#[test]
fn independent_schedule_failed_or_unknown_run_cannot_release_an_already_busy_slot() {
    for outcome in [Outcome::Failed, Outcome::Unknown] {
        let mut ledger = checked(WorkflowScheduleLedger::new(
            spec(10, 60, 60, 3),
            sample(0, 0),
        ));
        let token =
            checked(ledger.tick(sample(10_000, 10_000))).unwrap_or_else(|| panic!("first ticket"));
        checked(ledger.claim(&token, sample(10_000, 10_000)));
        assert_eq!(checked(ledger.tick(sample(70_000, 70_000))), None);
        assert_eq!(ledger.slots()[1].status(), Slot::SkippedBusy);
        checked(ledger.finish(&token, outcome));
        assert_eq!(ledger.slots()[1].status(), Slot::SkippedBusy);
        assert_eq!(
            ledger.slots()[2].status(),
            Slot::Invalidated(Reason::PriorRunNotSucceeded)
        );
        assert_eq!(checked(ledger.tick(sample(130_000, 130_000))), None);
    }
}

#[test]
fn independent_schedule_seven_day_authorization_boundary_includes_grace_and_milliseconds() {
    let exact = spec(604_740, 60, 60, 1);
    assert!(WorkflowScheduleLedger::new(exact.clone(), sample(0, 0)).is_ok());
    assert!(matches!(
        WorkflowScheduleLedger::new(exact, sample(-1, 0)),
        Err(Error::SpanLimit)
    ));
    assert!(matches!(
        WorkflowScheduleLedger::new(spec(604_741, 60, 60, 1), sample(0, 0)),
        Err(Error::SpanLimit)
    ));
}

#[test]
fn independent_schedule_fixed_calendar_agrees_with_external_gregorian_epoch_vectors() {
    // Expected epochs were obtained independently with Python's datetime and
    // exact timedelta arithmetic, including minimum/maximum UTC calendar years.
    for (text, offset, epoch) in [
        ("0001-01-01 14:00:00", 840, -62_135_596_800),
        ("1900-03-01 00:00:00", 0, -2_203_891_200),
        ("1969-12-31 23:59:59", 0, -1),
        ("2000-02-29 12:34:56", -330, 951_847_496),
        ("2024-02-29 23:59:59", 765, 1_709_205_299),
        ("9999-12-31 09:59:59", -840, 253_402_300_799),
    ] {
        assert_eq!(checked(parse_fixed_offset_datetime(text, offset)), epoch);
        assert_eq!(checked(format_fixed_offset_datetime(epoch, offset)), text);
    }
}
