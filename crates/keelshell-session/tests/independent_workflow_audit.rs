//! Independent real TCP-SSH receipt projection and no-execution persistence retry.
// Shared fixture defines scenarios used by other integration binaries.
#[allow(dead_code)]
#[path = "fixtures/batch_server.rs"]
mod fixture;
use fixture::{Opening, Server, TestResult, bounded, until};
use keelshell_core::{
    BatchTaskSpec, BatchWorkflowPlan, StateStore, WorkflowAuditOutcome, WorkflowAuditTrigger,
};
use keelshell_session::{WorkflowBinding, WorkflowOptions, start_workflow};
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn independent_audit_actual_failed_prerequisite_and_success_are_exact_and_retry_has_zero_wire()
-> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let target = Uuid::new_v4();
        let first = Uuid::from_u128(1);
        let blocked = Uuid::from_u128(2);
        let independent = Uuid::from_u128(3);
        let secret = "private-审计'\ncommand-no-disk-82";
        let plan = BatchWorkflowPlan::new(vec![
            BatchTaskSpec { id: first, target_id: target, command: "fail".into(), dependencies: vec![] },
            BatchTaskSpec { id: blocked, target_id: target, command: "never".into(), dependencies: vec![first] },
            BatchTaskSpec { id: independent, target_id: target, command: secret.into(), dependencies: vec![] },
        ])?;
        let options = WorkflowOptions { concurrency: 1, timeout: Duration::from_secs(4), ..Default::default() };
        let receipt = start_workflow(
            plan.clone().confirm(plan.review_token())?,
            vec![WorkflowBinding { id: target, session: ssh.clone() }], options,
        )?.finish().await?;
        let record = receipt.audit_record(&plan, options, Uuid::new_v4(), 1_725_000_000, WorkflowAuditTrigger::Manual)?;
        assert_eq!(record.tasks.iter().map(|task| task.id).collect::<Vec<_>>(), plan.tasks().iter().map(|task| task.id).collect::<Vec<_>>());
        assert_eq!(record.tasks.iter().find(|t| t.id == first).ok_or("failed task")?.outcome, WorkflowAuditOutcome::Failed { exit_code: 7 });
        assert_eq!(record.tasks.iter().find(|t| t.id == blocked).ok_or("blocked task")?.outcome, WorkflowAuditOutcome::DependencyBlocked { dependency: first });
        assert_eq!(record.tasks.iter().find(|t| t.id == independent).ok_or("independent task")?.outcome, WorkflowAuditOutcome::Succeeded);
        let actual_commands = server.observed.commands.lock().map_err(|_| "fixture lock")?.clone();
        // The two roots are independent; the reviewed graph does not impose
        // ordering between them. Require each exact command once and no extra.
        let mut unordered = actual_commands.clone(); unordered.sort();
        let mut expected = vec![b"fail".to_vec(), secret.as_bytes().to_vec()]; expected.sort();
        assert_eq!(unordered, expected);
        let mut wrong_options = options; wrong_options.output_limit -= 1;
        assert!(receipt.audit_record(&plan, wrong_options, Uuid::new_v4(), 1_725_000_000, WorkflowAuditTrigger::Manual).is_err());
        let other = BatchWorkflowPlan::new(vec![BatchTaskSpec { id: first, target_id: target, command: "different-review".into(), dependencies: vec![] }])?;
        assert!(receipt.audit_record(&other, options, Uuid::new_v4(), 1_725_000_000, WorkflowAuditTrigger::Manual).is_err());
        let dir = tempfile::tempdir()?; let path = dir.path().join("state.json");
        let store = StateStore::new(&path); let mut state = store.load()?;
        state.record_workflow_audit(record.clone())?; let saved = store.save(&state)?;
        let external = StateStore::new(&path); let mut changed = external.load()?;
        changed.settings.language = keelshell_core::Language::En; external.save(&changed)?;
        assert!(store.save(&saved).is_err());
        let bytes = std::fs::read(&path)?; let text = std::str::from_utf8(&bytes)?;
        for excluded in [secret, "command-no-disk-82", "fixture stderr", "different-review"] { assert!(!text.contains(excluded)); }
        assert_eq!(StateStore::new(&path).load()?.workflow_audits, vec![record]);
        assert_eq!(server.observed.commands.lock().map_err(|_| "fixture lock")?.as_slice(), actual_commands.as_slice());
        ssh.close().await?; until(|| ssh.is_closed()).await?;
        println!("independent audit actual SSH: failed/blocked/succeeded exact; two exec requests; zero retry requests; durable bytes exclude raw output/command");
        Ok(())
    }).await
}
