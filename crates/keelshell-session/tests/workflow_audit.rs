//! Pure typed receipt projections; no transport, process or network is started.
use keelshell_core::{
    BatchTaskSkipReason, BatchTaskSpec, BatchWorkflowPlan, WorkflowAuditOutcome as Outcome,
    WorkflowAuditTrigger,
};
use keelshell_session::{
    BatchOutcome, BatchRowReceipt, BatchUnknownReason, WorkflowOptions, WorkflowReceipt,
    WorkflowTaskReceipt, WorkflowTaskResult,
};
use std::sync::Arc;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> Result<(BatchWorkflowPlan, WorkflowReceipt), keelshell_core::BatchWorkflowError> {
    let target = Uuid::new_v4();
    let prerequisite = Uuid::from_u128(1);
    let tasks = (0..7)
        .map(|index| BatchTaskSpec {
            id: Uuid::from_u128(index + 1),
            target_id: target,
            command: "SECRET-COMMAND-$TOKEN".into(),
            dependencies: if index == 5 {
                vec![prerequisite]
            } else {
                vec![]
            },
        })
        .collect();
    let plan = BatchWorkflowPlan::new(tasks)?;
    let receipts = plan
        .tasks()
        .iter()
        .map(|task| {
            let result = match task.id.as_u128() {
                5 => WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::Cancelled,
                },
                6 => WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::DependencyNotSucceeded {
                        dependency: prerequisite,
                    },
                },
                7 => WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::StoppedAfterFailure,
                },
                value => WorkflowTaskResult::Transport {
                    row: Arc::new(BatchRowReceipt {
                        id: task.id,
                        outcome: match value {
                            1 => BatchOutcome::Exited { code: 9 },
                            2 => BatchOutcome::Exited { code: 0 },
                            3 => BatchOutcome::Rejected,
                            _ => BatchOutcome::Unknown {
                                reason: BatchUnknownReason::Cancelled,
                            },
                        },
                        stdout: b"SECRET-STDOUT".to_vec(),
                        stderr: b"SECRET-STDERR".to_vec(),
                    }),
                },
            };
            Arc::new(WorkflowTaskReceipt {
                id: task.id,
                target_id: task.target_id,
                result,
            })
        })
        .collect();
    let receipt = WorkflowReceipt {
        tasks: receipts,
        cancelled: true,
        stopped_after_failure: true,
        fingerprint: plan.review_token(),
        options: WorkflowOptions::default(),
    };
    Ok((plan, receipt))
}

fn copy_result(result: &WorkflowTaskResult) -> WorkflowTaskResult {
    match result {
        WorkflowTaskResult::Transport { row } => WorkflowTaskResult::Transport { row: row.clone() },
        WorkflowTaskResult::Skipped { reason } => WorkflowTaskResult::Skipped { reason: *reason },
    }
}
fn copy_task(task: &WorkflowTaskReceipt) -> WorkflowTaskReceipt {
    WorkflowTaskReceipt {
        id: task.id,
        target_id: task.target_id,
        result: copy_result(&task.result),
    }
}

fn copy_receipt(receipt: &WorkflowReceipt) -> WorkflowReceipt {
    WorkflowReceipt {
        tasks: receipt.tasks.clone(),
        cancelled: receipt.cancelled,
        stopped_after_failure: receipt.stopped_after_failure,
        fingerprint: receipt.fingerprint,
        options: receipt.options,
    }
}

#[test]
fn exact_receipt_projects_nonsecret_statuses_and_does_not_copy_outputs() -> TestResult {
    let (plan, receipt) = fixture()?;
    let audit = receipt.audit_record(
        &plan,
        receipt.options,
        Uuid::new_v4(),
        1,
        WorkflowAuditTrigger::Manual,
    )?;
    assert_eq!(audit.tasks.len(), plan.tasks().len());
    assert!(
        audit
            .tasks
            .iter()
            .any(|task| matches!(task.outcome, Outcome::Failed { exit_code: 9 }))
    );
    assert!(
        audit
            .tasks
            .iter()
            .any(|task| matches!(task.outcome, Outcome::DependencyBlocked { .. }))
    );
    for expected in [
        Outcome::Succeeded,
        Outcome::Failed { exit_code: 9 },
        Outcome::Rejected,
        Outcome::Unknown,
        Outcome::Cancelled,
        Outcome::DependencyBlocked {
            dependency: Uuid::from_u128(1),
        },
        Outcome::StoppedAfterFailure,
    ] {
        assert!(
            audit.tasks.iter().any(|task| task.outcome == expected),
            "missing projected state {expected:?}"
        );
    }
    let text = format!("{audit:?}");
    for forbidden in [
        "SECRET",
        "command",
        "stdout",
        "stderr",
        "digest",
        "fingerprint",
        "options",
    ] {
        assert!(!text.contains(forbidden));
    }
    Ok(())
}

#[test]
fn partial_wrong_target_wrong_options_wrong_inner_row_and_false_dependency_are_rejected()
-> TestResult {
    let (plan, receipt) = fixture()?;
    let audit = |receipt: &WorkflowReceipt| {
        receipt.audit_record(
            &plan,
            WorkflowOptions::default(),
            Uuid::new_v4(),
            1,
            WorkflowAuditTrigger::Manual,
        )
    };
    let mut bad = copy_receipt(&receipt);
    bad.tasks.pop();
    assert!(audit(&bad).is_err());
    bad = copy_receipt(&receipt);
    let mut first = copy_task(&bad.tasks[0]);
    first.target_id = Uuid::new_v4();
    bad.tasks[0] = Arc::new(first);
    assert!(audit(&bad).is_err());
    bad = copy_receipt(&receipt);
    bad.options.concurrency = 1;
    assert!(audit(&bad).is_err());
    bad = copy_receipt(&receipt);
    let mut first = copy_task(&bad.tasks[0]);
    first.result = WorkflowTaskResult::Transport {
        row: Arc::new(BatchRowReceipt {
            id: Uuid::new_v4(),
            outcome: BatchOutcome::Exited { code: 0 },
            stdout: vec![],
            stderr: vec![],
        }),
    };
    bad.tasks[0] = Arc::new(first);
    assert!(audit(&bad).is_err());
    bad = receipt;
    let mut first = copy_task(&bad.tasks[0]);
    first.result = WorkflowTaskResult::Skipped {
        reason: BatchTaskSkipReason::DependencyNotSucceeded {
            dependency: Uuid::new_v4(),
        },
    };
    bad.tasks[0] = Arc::new(first);
    assert!(audit(&bad).is_err());
    Ok(())
}
