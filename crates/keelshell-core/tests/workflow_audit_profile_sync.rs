//! Persisted workflow results remain device-local during reviewed profile sync.

use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};

use keelshell_core::{
    AppState, Connection, ProfileSyncChoice, ProfileSyncService, StateStore, WorkflowAuditOutcome,
    WorkflowAuditRecord, WorkflowAuditTrigger, WorkflowTaskAudit,
};
use uuid::Uuid;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn history(id: u128) -> WorkflowAuditRecord {
    WorkflowAuditRecord {
        id: Uuid::from_u128(id),
        recorded_at: 1_725_000_000,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: vec![WorkflowTaskAudit {
            id: Uuid::from_u128(id + 1),
            target_id: Uuid::from_u128(700),
            outcome: WorkflowAuditOutcome::Succeeded,
        }],
        cancelled: false,
        stopped_after_failure: false,
    }
}

#[test]
fn reviewed_sync_preserves_both_local_histories_and_legacy_state_loading() -> TestResult {
    let directory = tempfile::tempdir()?;
    let shared = directory.path().join("shared");
    std::fs::create_dir(&shared)?;
    let a = Arc::new(StateStore::new(directory.path().join("a/state.json")));
    let b = Arc::new(StateStore::new(directory.path().join("b/state.json")));
    let first = history(800);
    let second = history(900);
    let mut initial_a = a.load()?;
    let mut connection = Connection::new("Fixture", "fixture.invalid", "fixture");
    connection.id = Uuid::from_u128(700);
    initial_a.connections.push(connection);
    initial_a.record_workflow_audit(first.clone())?;
    a.save(&initial_a)?;
    let mut initial_b = b.load()?;
    initial_b.record_workflow_audit(second.clone())?;
    b.save(&initial_b)?;

    for (store, choice) in [
        (&a, ProfileSyncChoice::Local),
        (&b, ProfileSyncChoice::Remote),
    ] {
        let service = ProfileSyncService::new(store.clone());
        let review = service.inspect(
            shared.clone(),
            Zeroizing::new("isolated-combination-password".into()),
            &AtomicBool::new(false),
        )?;
        let rows = serde_json::to_value(
            review
                .rows()
                .iter()
                .map(|row| (&row.local, &row.remote))
                .collect::<Vec<_>>(),
        )?;
        let rows_text = serde_json::to_string(&rows)?;
        assert!(!rows_text.contains("workflow_audits"));
        assert!(!rows_text.contains(&first.id.to_string()));
        assert!(!rows_text.contains(&second.id.to_string()));
        let choices: BTreeMap<_, _> = review.rows().iter().map(|row| (row.id, choice)).collect();
        assert!(
            service
                .apply(
                    review,
                    choices,
                    Zeroizing::new("isolated-combination-password".into()),
                    &AtomicBool::new(false),
                )?
                .published
        );
    }

    let final_a = a.load()?;
    let final_b = b.load()?;
    assert_eq!(final_a.workflow_audits, vec![first]);
    assert_eq!(final_b.workflow_audits, vec![second]);
    assert!(
        final_a
            .profile_sync
            .as_ref()
            .is_some_and(|sync| sync.enabled())
    );
    assert!(
        final_b
            .profile_sync
            .as_ref()
            .is_some_and(|sync| sync.enabled())
    );
    assert_eq!(final_b.connections[0].host, "fixture.invalid");
    assert_eq!(StateStore::new(b.path()).load()?, final_b);

    let mut legacy = serde_json::to_value(&final_b)?;
    legacy
        .as_object_mut()
        .ok_or("state is not an object")?
        .remove("workflow_audits");
    let legacy: AppState = serde_json::from_value(legacy)?;
    assert!(legacy.workflow_audits.is_empty());
    assert_eq!(legacy.profile_sync, final_b.profile_sync);

    let mut hostile = serde_json::to_value(&final_b)?;
    hostile["workflow_audits"][0]["tasks"][0]["outcome"]["command"] =
        serde_json::json!("unexpected isolated context");
    assert!(serde_json::from_value::<AppState>(hostile).is_err());
    assert_eq!(b.load()?, final_b);
    Ok(())
}
