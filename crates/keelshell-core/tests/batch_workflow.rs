//! Public reviewed-DAG workflows, with local rendering and synthetic receipts.
use keelshell_core::{
    BatchCommandTemplate, BatchTargetContext, BatchTaskOutcome, BatchTaskSkipReason, BatchTaskSpec,
    BatchTaskStatus, BatchWorkflowError, BatchWorkflowPlan,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn task(id: u128, target_id: u128, command: &str, dependencies: &[u128]) -> BatchTaskSpec {
    BatchTaskSpec {
        id: Uuid::from_u128(id),
        target_id: Uuid::from_u128(target_id),
        command: command.to_owned(),
        dependencies: dependencies.iter().map(|id| Uuid::from_u128(*id)).collect(),
    }
}

#[test]
fn rendered_target_commands_survive_review_and_only_success_opens_the_join() -> TestResult {
    let template = BatchCommandTemplate::compile("printf '%s\\n' {{name}} {{host}}")?
        .ok_or("expected template")?;
    let context = BatchTargetContext {
        name: "ops 'A'".into(),
        host: "node-a.example".into(),
        ..Default::default()
    };
    let command = template.render(&context)?;
    let plan = BatchWorkflowPlan::new(vec![
        task(3, 101, "printf 'join complete'", &[2, 1]),
        task(2, 102, "printf 'parallel check'", &[]),
        task(1, 101, &command, &[]),
    ])?;
    assert_eq!(plan.tasks()[0].command, command);
    let token = plan.review_token();
    let receipt = plan.confirm(token)?;
    assert_eq!(receipt.plan().tasks()[0].target_id, Uuid::from_u128(101));
    let mut ledger = receipt.into_ledger();
    assert_eq!(
        ledger.ready_tasks(),
        [Uuid::from_u128(1), Uuid::from_u128(2)]
    );
    ledger.admit(Uuid::from_u128(1))?;
    ledger.admit(Uuid::from_u128(2))?;
    ledger.finish(Uuid::from_u128(2), BatchTaskOutcome::Success)?;
    assert_eq!(
        ledger.status(Uuid::from_u128(3))?,
        BatchTaskStatus::Blocked {
            waiting_for: vec![Uuid::from_u128(1)]
        }
    );
    assert_eq!(
        ledger.admit(Uuid::from_u128(3)),
        Err(BatchWorkflowError::InvalidTransition {
            id: Uuid::from_u128(3)
        })
    );
    ledger.finish(Uuid::from_u128(1), BatchTaskOutcome::Success)?;
    assert_eq!(ledger.ready_tasks(), [Uuid::from_u128(3)]);
    assert_eq!(ledger.plan().tasks()[0].command, command);
    ledger.admit(Uuid::from_u128(3))?;
    ledger.finish(Uuid::from_u128(3), BatchTaskOutcome::Success)?;
    assert!(ledger.is_finished());
    Ok(())
}

#[test]
fn unknown_receipt_skips_dependents_but_keeps_an_independent_running_task() -> TestResult {
    let plan = BatchWorkflowPlan::new(vec![
        task(1, 101, "check A", &[]),
        task(2, 101, "after A", &[1]),
        task(3, 102, "independent B", &[]),
        task(4, 102, "after dependent", &[2]),
    ])?;
    let token = plan.review_token();
    let mut ledger = plan.confirm(token)?.into_ledger();
    ledger.admit(Uuid::from_u128(1))?;
    ledger.admit(Uuid::from_u128(3))?;
    ledger.finish(Uuid::from_u128(1), BatchTaskOutcome::Unknown)?;
    assert_eq!(
        ledger.status(Uuid::from_u128(2))?,
        BatchTaskStatus::Skipped(BatchTaskSkipReason::DependencyNotSucceeded {
            dependency: Uuid::from_u128(1)
        })
    );
    assert_eq!(
        ledger.status(Uuid::from_u128(4))?,
        BatchTaskStatus::Skipped(BatchTaskSkipReason::DependencyNotSucceeded {
            dependency: Uuid::from_u128(2)
        })
    );
    assert_eq!(ledger.status(Uuid::from_u128(3))?, BatchTaskStatus::Running);
    ledger.cancel_pending();
    assert_eq!(ledger.status(Uuid::from_u128(3))?, BatchTaskStatus::Running);
    ledger.finish(Uuid::from_u128(3), BatchTaskOutcome::Failed)?;
    assert!(ledger.is_finished());
    Ok(())
}

#[test]
fn target_or_command_changes_need_new_review_even_with_identical_task_ids() -> TestResult {
    let original = BatchWorkflowPlan::new(vec![task(1, 101, "echo reviewed", &[])])?;
    let old_token = original.review_token();
    for changed in [
        task(1, 102, "echo reviewed", &[]),
        task(1, 101, "echo reviewed\n", &[]),
    ] {
        let replacement = BatchWorkflowPlan::new(vec![changed])?;
        assert!(matches!(
            replacement.confirm(old_token),
            Err(BatchWorkflowError::ReviewMismatch)
        ));
    }
    let mut ledger = original.confirm(old_token)?.into_ledger();
    assert!(matches!(
        ledger.skip(
            Uuid::from_u128(1),
            BatchTaskSkipReason::DependencyNotSucceeded {
                dependency: Uuid::from_u128(9)
            }
        ),
        Err(BatchWorkflowError::InvalidTransition { .. })
    ));
    assert_eq!(ledger.status(Uuid::from_u128(1))?, BatchTaskStatus::Ready);
    Ok(())
}
