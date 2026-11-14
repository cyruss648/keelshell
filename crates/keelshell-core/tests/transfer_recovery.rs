#![allow(clippy::expect_used, clippy::unwrap_used)]

use keelshell_core::{
    Error, TransferRecoveryDirection, TransferRecoveryRecord, TransferRecoveryState,
    TransferRecoveryStore,
};
use tempfile::TempDir;
use uuid::Uuid;

fn write_private(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("write recovery document");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .expect("set private recovery permissions");
    }
}

fn record(id: Uuid, state: TransferRecoveryState) -> TransferRecoveryRecord {
    TransferRecoveryRecord::new(
        id,
        TransferRecoveryDirection::Download,
        "/tmp/partial.bin",
        "/srv/source.bin",
        false,
        state,
        12,
        Some(24),
        12,
        100,
        100,
    )
    .expect("valid transfer recovery record")
}

#[test]
fn recovery_document_is_restart_visible_but_contains_no_resume_authority() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("transfer-recovery.json");
    let id = Uuid::new_v4();
    TransferRecoveryStore::new(&path)
        .upsert(record(id, TransferRecoveryState::Unfinished))
        .expect("write unfinished metadata");

    let reopened = TransferRecoveryStore::new(&path)
        .load()
        .expect("load metadata after restart");
    assert_eq!(
        reopened,
        vec![record(id, TransferRecoveryState::Unfinished)]
    );
    let json = std::fs::read_to_string(path).expect("read recovery JSON");
    for forbidden in [
        "session_token",
        "session_identity",
        "review_token",
        "review_id",
        "credential",
        "secret",
        "worker",
    ] {
        assert!(
            !json.contains(forbidden),
            "{forbidden} leaked into metadata"
        );
    }
}

#[test]
fn saved_connection_binding_round_trips_and_legacy_records_remain_unbound() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("transfer-recovery.json");
    let connection_id = Uuid::new_v4();
    let id = Uuid::new_v4();
    let bound = TransferRecoveryRecord::new_with_connection_id(
        id,
        Some(connection_id),
        TransferRecoveryDirection::Upload,
        "/tmp/partial.bin",
        "/srv/source.bin",
        false,
        TransferRecoveryState::Failed,
        12,
        Some(24),
        12,
        100,
        100,
    )
    .expect("valid bound record");
    let store = TransferRecoveryStore::new(&path);
    store.upsert(bound.clone()).expect("write bound metadata");
    assert_eq!(store.load().expect("load bound metadata"), vec![bound]);

    write_private(&path, br#"{"schema_version":1,"records":[{"id":"00000000-0000-0000-0000-000000000001","direction":"download","local_path":"/tmp/partial.bin","remote_path":"/srv/source.bin","directory":false,"state":"failed","transferred_bytes":12,"total_bytes":24,"reviewed_existing_bytes":12,"created_at":100,"updated_at":100}]}"#);
    let legacy = TransferRecoveryStore::new(&path)
        .load()
        .expect("load legacy unbound metadata");
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].connection_id, None);
}

#[test]
fn recovery_store_rejects_invalid_schema_and_duplicate_records() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("transfer-recovery.json");
    write_private(&path, br#"{"schema_version":7,"records":[]}"#);
    assert!(matches!(
        TransferRecoveryStore::new(&path).load(),
        Err(Error::UnsupportedTransferRecoverySchema { found: 7, .. })
    ));

    let store = TransferRecoveryStore::new(&path);
    store
        .upsert(record(Uuid::new_v4(), TransferRecoveryState::Failed))
        .expect_err("the malformed document cannot be overwritten");
}
