//! Independent retention and untrusted nested wire boundaries.
use keelshell_core::{
    AppState, WorkflowAuditOutcome, WorkflowAuditRecord, WorkflowAuditTrigger, WorkflowTaskAudit,
};
use uuid::Uuid;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn record(count: usize) -> WorkflowAuditRecord {
    WorkflowAuditRecord {
        id: Uuid::new_v4(),
        recorded_at: 1,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: (0..count)
            .map(|_| WorkflowTaskAudit {
                id: Uuid::new_v4(),
                target_id: Uuid::from_u128(1),
                outcome: WorkflowAuditOutcome::Succeeded,
            })
            .collect(),
        cancelled: false,
        stopped_after_failure: false,
    }
}
#[test]
fn independent_audit_whole_run_fifo_exact_budget_and_failed_append_are_atomic() -> TestResult {
    let mut state = AppState::default();
    for _ in 0..15 {
        state.record_workflow_audit(record(128))?;
    }
    let head = record(127);
    state.record_workflow_audit(head.clone())?;
    let last = record(1);
    state.record_workflow_audit(last.clone())?;
    assert_eq!(
        state
            .workflow_audits
            .iter()
            .map(|r| r.tasks.len())
            .sum::<usize>(),
        2048
    );
    let original_first = state.workflow_audits[0].id;
    let fresh = record(1);
    state.record_workflow_audit(fresh.clone())?;
    assert!(!state.workflow_audits.iter().any(|r| r.id == original_first));
    assert_eq!(
        state
            .workflow_audits
            .iter()
            .map(|r| r.tasks.len())
            .sum::<usize>(),
        1921
    );
    assert_eq!(state.workflow_audits.last(), Some(&fresh));
    assert!(state.workflow_audits.contains(&head) && state.workflow_audits.contains(&last));
    let before = state.clone();
    let mut invalid = record(33);
    for (index, t) in invalid.tasks.iter_mut().enumerate() {
        t.target_id = Uuid::from_u128(index as u128 + 1);
    }
    assert!(state.record_workflow_audit(invalid).is_err());
    assert_eq!(state, before);
    Ok(())
}
#[test]
fn independent_audit_untrusted_nested_extra_fields_cannot_reintroduce_execution_context()
-> TestResult {
    let good = serde_json::to_value(record(1))?;
    for key in [
        "command",
        "stdout",
        "params",
        "address",
        "path",
        "fingerprint",
    ] {
        for location in ["task", "outcome", "trigger"] {
            let mut value = good.clone();
            let slot = match location {
                "task" => &mut value["tasks"][0],
                "outcome" => &mut value["tasks"][0]["outcome"],
                _ => &mut value["trigger"],
            };
            slot[key] = serde_json::json!("sensitive marker");
            assert!(
                serde_json::from_value::<WorkflowAuditRecord>(value).is_err(),
                "{location}/{key}"
            );
        }
    }
    Ok(())
}

#[test]
fn independent_audit_all_unit_and_data_variants_reject_untrusted_extra_keys() -> TestResult {
    use keelshell_core::WorkflowAuditNotStarted;
    let mut record = record(2);
    record.cancelled = true;
    record.stopped_after_failure = true;
    let prerequisite = record.tasks[1].id;
    let mut accepted = Vec::new();
    let mut checked = 0;
    for outcome in [
        WorkflowAuditOutcome::Succeeded,
        WorkflowAuditOutcome::Failed { exit_code: 7 },
        WorkflowAuditOutcome::Rejected,
        WorkflowAuditOutcome::Unknown,
        WorkflowAuditOutcome::Cancelled,
        WorkflowAuditOutcome::DependencyBlocked {
            dependency: prerequisite,
        },
        WorkflowAuditOutcome::StoppedAfterFailure,
        WorkflowAuditOutcome::NotStarted {
            reason: WorkflowAuditNotStarted::Deadline,
        },
    ] {
        record.tasks[0].outcome = outcome;
        assert_eq!(
            serde_json::from_value::<WorkflowAuditRecord>(serde_json::to_value(&record)?)?,
            record
        );
        for key in [
            "command",
            "stdout",
            "params",
            "address",
            "path",
            "fingerprint",
        ] {
            let mut wire = serde_json::to_value(&record)?;
            wire["tasks"][0]["outcome"][key] = serde_json::json!("private-marker");
            checked += 1;
            if serde_json::from_value::<WorkflowAuditRecord>(wire).is_ok() {
                accepted.push(format!("{outcome:?}/{key}"));
            }
        }
    }
    for trigger in [
        WorkflowAuditTrigger::Manual,
        WorkflowAuditTrigger::Scheduled {
            schedule_id: Uuid::new_v4(),
            occurrence: 0,
            scheduled_at: 0,
        },
    ] {
        record.trigger = trigger;
        assert_eq!(
            serde_json::from_value::<WorkflowAuditRecord>(serde_json::to_value(&record)?)?,
            record
        );
        for key in [
            "command",
            "stdout",
            "params",
            "address",
            "path",
            "fingerprint",
        ] {
            let mut wire = serde_json::to_value(&record)?;
            wire["trigger"][key] = serde_json::json!("private-marker");
            checked += 1;
            if serde_json::from_value::<WorkflowAuditRecord>(wire).is_ok() {
                accepted.push(format!("{trigger:?}/{key}"));
            }
        }
    }
    println!(
        "strict nested variant controls checked={checked}; accepted extra-field cases={accepted:?}"
    );
    assert!(
        accepted.is_empty(),
        "strict wire accepted extra keys: {accepted:?}"
    );
    Ok(())
}
