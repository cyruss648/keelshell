use std::fs;

use keelshell_core::{BatchAuditRecord, BatchAuditSummary, StateStore, command_sha256};
use tempfile::tempdir;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn batch_audit_roundtrips_through_real_state_store_without_command_text() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    let target = Uuid::new_v4();
    let command = "printf 'sensitive command text'";
    let record = BatchAuditRecord::new(
        command,
        1_725_000_000,
        vec![target],
        BatchAuditSummary {
            target_count: 2,
            succeeded: 1,
            failed: 0,
            unknown: 1,
            not_started: 0,
            cancelled: true,
            stopped_after_failure: false,
        },
    )?;
    let digest = command_sha256(command);
    state.record_batch_audit(record.clone())?;
    let saved = store.save(&state)?;
    assert_eq!(saved.batch_audits, vec![record]);

    let reopened = StateStore::new(&path);
    let loaded = reopened.load()?;
    assert_eq!(loaded.batch_audits, saved.batch_audits);
    assert_eq!(loaded.batch_audits[0].command_sha256, digest);
    let bytes = fs::read(&path)?;
    let document = String::from_utf8(bytes)?;
    assert!(!document.contains("sensitive command text"));
    assert!(!document.contains("stdout"));
    assert!(!document.contains("node.example"));
    Ok(())
}

#[test]
fn older_state_without_batch_audits_loads_as_empty() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    // Empty audit collections are omitted from the wire representation, so a
    // state saved by an older build exercises the same absent-field path.
    let state = store.load()?;
    store.save(&state)?;
    let loaded = StateStore::new(&path).load()?;
    assert!(loaded.batch_audits.is_empty());
    Ok(())
}
