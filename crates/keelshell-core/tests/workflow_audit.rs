use keelshell_core::{
    AppState, MAX_WORKFLOW_AUDIT_TOTAL_TASKS, StateStore, WorkflowAuditOutcome as Outcome,
    WorkflowAuditRecord, WorkflowAuditTrigger, WorkflowTaskAudit,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn record(tasks: usize) -> WorkflowAuditRecord {
    let target = Uuid::new_v4();
    WorkflowAuditRecord {
        id: Uuid::new_v4(),
        recorded_at: 1_725_000_000,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: (0..tasks)
            .map(|_| WorkflowTaskAudit {
                id: Uuid::new_v4(),
                target_id: target,
                outcome: Outcome::Succeeded,
            })
            .collect(),
        cancelled: false,
        stopped_after_failure: false,
    }
}

#[test]
fn repeated_receipt_is_idempotent_but_conflicting_receipts_are_atomic() -> TestResult {
    let mut state = AppState::default();
    let first = record(1);
    state.record_workflow_audit(first.clone())?;
    state.record_workflow_audit(first.clone())?;
    assert_eq!(state.workflow_audits, vec![first.clone()]);
    let before = state.clone();
    let mut conflict = first;
    conflict.tasks[0].outcome = Outcome::Unknown;
    assert!(state.record_workflow_audit(conflict).is_err());
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn finite_occurrence_identity_is_not_reused_by_another_run() -> TestResult {
    let mut state = AppState::default();
    let mut first = record(2);
    first.trigger = WorkflowAuditTrigger::Scheduled {
        schedule_id: Uuid::new_v4(),
        occurrence: 31,
        scheduled_at: 1_725_000_060,
    };
    state.record_workflow_audit(first.clone())?;
    let before = state.clone();
    first.id = Uuid::new_v4();
    assert!(state.record_workflow_audit(first).is_err());
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn backward_clock_retains_insertion_order_without_replaying() -> TestResult {
    let mut state = AppState::default();
    let first = record(1);
    let mut second = record(1);
    second.recorded_at = 1;
    state.record_workflow_audit(first.clone())?;
    state.record_workflow_audit(second.clone())?;
    assert_eq!(state.workflow_audits, vec![first, second]);
    for _ in 0..100 {
        state.record_workflow_audit(record(1))?;
    }
    assert_eq!(state.workflow_audits.len(), 100);
    Ok(())
}

#[test]
fn max_workload_is_trimmed_as_whole_runs_with_bounded_disk_and_validation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    for _ in 0..100 {
        let mut worst = record(128);
        let prerequisite = worst.tasks[0].id;
        worst.tasks[0].outcome = Outcome::Failed {
            exit_code: u32::MAX,
        };
        for task in worst.tasks.iter_mut().skip(1) {
            task.outcome = Outcome::DependencyBlocked {
                dependency: prerequisite,
            };
        }
        state.record_workflow_audit(worst)?;
    }
    assert_eq!(state.workflow_audits.len(), 16);
    assert_eq!(
        state
            .workflow_audits
            .iter()
            .map(|entry| entry.tasks.len())
            .sum::<usize>(),
        MAX_WORKFLOW_AUDIT_TOTAL_TASKS
    );
    let bytes = serde_json::to_vec_pretty(&state)?;
    assert!(
        bytes.len() < 640 * 1024,
        "bounded history size {}",
        bytes.len()
    );
    let saved = store.save(&state)?;
    let start = std::time::Instant::now();
    let loaded = StateStore::new(&path).load()?;
    println!(
        "workflow audit 100x128 input -> retained {} runs / {} tasks; state {} bytes; actual restart validation {:?}",
        loaded.workflow_audits.len(),
        MAX_WORKFLOW_AUDIT_TOTAL_TASKS,
        bytes.len(),
        start.elapsed()
    );
    assert_eq!(loaded.workflow_audits, saved.workflow_audits);
    assert!(
        loaded
            .workflow_audits
            .iter()
            .all(|entry| entry.tasks.len() == 128)
    );
    // Directly injecting the original workload cannot bypass the disk budget.
    state.workflow_audits = (0..100).map(|_| record(128)).collect();
    assert!(state.validate().is_err());
    Ok(())
}

#[test]
fn restart_preserves_distinct_outcomes_and_legacy_missing_field_is_empty() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    let legacy = store.save(&state)?;
    assert!(StateStore::new(&path).load()?.workflow_audits.is_empty());
    state = legacy;
    let mut entry = record(7);
    let prerequisite = entry.tasks[0].id;
    for (task, outcome) in entry.tasks.iter_mut().zip([
        Outcome::Succeeded,
        Outcome::Failed { exit_code: 9 },
        Outcome::Rejected,
        Outcome::Unknown,
        Outcome::Cancelled,
        Outcome::DependencyBlocked {
            dependency: prerequisite,
        },
        Outcome::StoppedAfterFailure,
    ]) {
        task.outcome = outcome;
    }
    entry.cancelled = true;
    entry.stopped_after_failure = true;
    state.record_workflow_audit(entry.clone())?;
    store.save(&state)?;
    let loaded = StateStore::new(&path).load()?;
    assert_eq!(loaded.workflow_audits, vec![entry]);
    let wire = serde_json::to_value(&loaded.workflow_audits)?;
    let text = wire.to_string();
    for excluded in [
        "command",
        "stdout",
        "stderr",
        "endpoint",
        "password",
        "parameter",
        "digest",
        "fingerprint",
        "review_token",
    ] {
        assert!(!text.contains(excluded), "forbidden field {excluded}");
    }
    assert!(wire[0].get("options").is_none());
    Ok(())
}

#[test]
fn invalid_wire_rejects_unknown_fields_duplicate_ids_and_impossible_outcomes() -> TestResult {
    let good = record(2);
    let mut value = serde_json::to_value(&good)?;
    value["command"] = serde_json::json!("secret");
    assert!(serde_json::from_value::<WorkflowAuditRecord>(value).is_err());
    let mut bad = good.clone();
    bad.tasks[1].id = bad.tasks[0].id;
    assert!(bad.validate().is_err());
    bad = good.clone();
    bad.tasks[0].outcome = Outcome::Failed { exit_code: 0 };
    assert!(bad.validate().is_err());
    bad = good.clone();
    bad.tasks[0].outcome = Outcome::Cancelled;
    assert!(bad.validate().is_err());
    bad = good.clone();
    bad.tasks[0].outcome = Outcome::DependencyBlocked {
        dependency: bad.tasks[0].id,
    };
    assert!(bad.validate().is_err());
    bad = good;
    bad.trigger = WorkflowAuditTrigger::Scheduled {
        schedule_id: Uuid::new_v4(),
        occurrence: 32,
        scheduled_at: 1,
    };
    assert!(bad.validate().is_err());
    Ok(())
}

#[test]
fn intended_schedule_time_uses_the_existing_signed_calendar_domain() -> TestResult {
    let mut entry = record(1);
    for scheduled_at in [-62_135_596_800, -1, 0, 253_402_300_799] {
        entry.trigger = WorkflowAuditTrigger::Scheduled {
            schedule_id: Uuid::new_v4(),
            occurrence: 0,
            scheduled_at,
        };
        entry.validate()?;
    }
    for scheduled_at in [i64::MIN, i64::MAX] {
        entry.trigger = WorkflowAuditTrigger::Scheduled {
            schedule_id: Uuid::new_v4(),
            occurrence: 0,
            scheduled_at,
        };
        assert!(entry.validate().is_err());
    }
    Ok(())
}
