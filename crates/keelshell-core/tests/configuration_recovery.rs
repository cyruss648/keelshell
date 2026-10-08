//! Real isolated files exercise metadata recovery without any network or credentials.
use keelshell_core::{
    AppState, ConfigBackupStatus, ConfigSourceStatus, Connection, Error, MAX_CONFIG_BACKUPS,
    MAX_CONFIG_ORIGINALS, StateStore,
};
use std::{fs, io::Write, path::Path};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn private_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

fn seeded(directory: &Path) -> Result<StateStore, Error> {
    let store = StateStore::new(directory.join("state.json"));
    let mut state = store.load()?;
    state
        .connections
        .push(Connection::new("original", "original.test", "user"));
    store.save(&state)?;
    Ok(store)
}

#[test]
fn first_save_has_no_previous_snapshot_and_listing_creates_no_directory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let absent = StateStore::new(directory.path().join("not-created/state.json"));
    assert!(absent.config_backups()?.is_empty());
    assert!(!directory.path().join("not-created").exists());
    let store = seeded(directory.path())?;
    assert!(store.config_backups()?.is_empty());
    Ok(())
}

#[test]
fn saves_rotate_eight_valid_snapshots_and_keep_current_metadata() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let mut state = store.load()?;
    for index in 0..20 {
        state.connections[0].name = format!("revision-{index}");
        state = store.save(&state)?;
    }
    let backups = store.config_backups()?;
    assert_eq!(backups.len(), MAX_CONFIG_BACKUPS);
    assert!(
        backups
            .iter()
            .all(|entry| entry.status == ConfigBackupStatus::Available)
    );
    let retained = backups
        .iter()
        .map(|entry| {
            let bytes = fs::read(
                directory
                    .path()
                    .join(format!("state.json.backups/{}.json", entry.id.uuid())),
            )?;
            let document: serde_json::Value = serde_json::from_slice(&bytes)?;
            Ok(document["state"]["connections"][0]["name"]
                .as_str()
                .ok_or("name")?
                .to_owned())
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    assert_eq!(
        retained,
        (11..=18)
            .rev()
            .map(|index| format!("revision-{index}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(store.load()?.connections[0].name, "revision-19");
    Ok(())
}

#[test]
fn truncated_original_is_preserved_exactly_before_explicit_restore() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let corrupt = b"{\"schema_version\":1,\"connections\":[";
    private_write(store.path(), corrupt)?;
    assert!(store.load().is_err());
    let preview = store.preview_config_backup(id)?;
    assert_eq!(preview.current_status(), ConfigSourceStatus::Corrupt);
    assert_eq!(preview.current_summary(), None);
    assert_eq!(preview.summary().connections, 1);
    assert_eq!(
        fs::read(store.path())?,
        corrupt,
        "preview has no write authority"
    );
    let restored = store.restore_config_backup(&preview)?;
    assert_eq!(restored.connections[0].name, "original");
    let originals = fs::read_dir(directory.path().join("state.json.originals"))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(originals.len(), 1);
    assert_eq!(fs::read(originals[0].path())?, corrupt);
    assert_eq!(store.load()?, restored);
    Ok(())
}

#[test]
fn corrupted_load_never_triggers_automatic_restore_or_new_backup() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    store.create_config_backup()?;
    private_write(store.path(), b"broken")?;
    let before = store.config_backups()?;
    assert!(store.load().is_err());
    assert!(store.save(&AppState::default()).is_err());
    assert!(store.create_config_backup().is_err());
    assert_eq!(fs::read(store.path())?, b"broken");
    assert_eq!(store.config_backups()?, before);
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn missing_current_file_can_be_restored_without_fabricated_original() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    fs::remove_file(store.path())?;
    let preview = store.preview_config_backup(id)?;
    assert_eq!(preview.current_status(), ConfigSourceStatus::Missing);
    assert_eq!(preview.current_summary(), None);
    assert_eq!(store.restore_config_backup(&preview)?.connections.len(), 1);
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn valid_current_file_is_preserved_and_restore_issues_a_new_revision() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let mut newer = store.load()?;
    newer.connections[0].name = "newer".into();
    newer
        .connections
        .push(Connection::new("second", "second.test", "user"));
    let newer = store.save(&newer)?;
    let original = fs::read(store.path())?;
    let preview = store.preview_config_backup(id)?;
    assert_eq!(preview.current_status(), ConfigSourceStatus::Valid);
    assert_eq!(
        preview
            .current_summary()
            .ok_or("current counts missing")?
            .connections,
        2
    );
    assert_eq!(preview.summary().connections, 1);
    let restored = store.restore_config_backup(&preview)?;
    assert_ne!(restored.snapshot, newer.snapshot);
    assert!(matches!(store.save(&newer), Err(Error::Conflict)));
    let originals = fs::read_dir(directory.path().join("state.json.originals"))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(fs::read(originals[0].path())?, original);
    Ok(())
}

#[test]
fn same_bytes_saved_after_review_still_invalidate_the_revision() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let preview = store.preview_config_backup(id)?;
    let same = store.load()?;
    store.save(&same)?;
    assert!(matches!(
        store.restore_config_backup(&preview),
        Err(Error::ConfigRecoveryConflict)
    ));
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn empty_valid_current_has_real_zero_counts_instead_of_unavailable_counts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let mut empty = store.load()?;
    empty.connections.clear();
    store.save(&empty)?;
    let before = fs::read(store.path())?;
    let preview = store.preview_config_backup(id)?;
    let current = preview
        .current_summary()
        .ok_or("valid current counts unavailable")?;
    assert_eq!(current.connections, 0);
    assert_eq!(preview.summary().connections, 1);
    assert_eq!(fs::read(store.path())?, before);
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn another_store_cannot_reuse_an_opaque_review_for_the_same_path() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let preview = store.preview_config_backup(id)?;
    let other = StateStore::new(store.path());
    assert!(matches!(
        other.restore_config_backup(&preview),
        Err(Error::ConfigRecoveryConflict)
    ));
    Ok(())
}

#[test]
fn missing_to_existing_race_refuses_recovery_without_overwrite() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    fs::remove_file(store.path())?;
    let preview = store.preview_config_backup(id)?;
    private_write(store.path(), b"external-new-bytes")?;
    assert!(matches!(
        store.restore_config_backup(&preview),
        Err(Error::ConfigRecoveryConflict)
    ));
    assert_eq!(fs::read(store.path())?, b"external-new-bytes");
    Ok(())
}

#[test]
fn corrupt_to_changed_corrupt_race_refuses_recovery_without_preservation_side_effect() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    private_write(store.path(), b"first-corrupt")?;
    let preview = store.preview_config_backup(id)?;
    private_write(store.path(), b"second-corrupt")?;
    assert!(matches!(
        store.restore_config_backup(&preview),
        Err(Error::ConfigRecoveryConflict)
    ));
    assert_eq!(fs::read(store.path())?, b"second-corrupt");
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn changing_selected_backup_after_review_is_rejected() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let preview = store.preview_config_backup(id)?;
    private_write(
        &directory
            .path()
            .join(format!("state.json.backups/{}.json", id.uuid())),
        b"broken",
    )?;
    assert!(matches!(
        store.restore_config_backup(&preview),
        Err(Error::ConfigRecoveryConflict)
    ));
    assert_eq!(store.load()?.connections[0].name, "original");
    Ok(())
}

#[test]
fn future_backup_schema_remains_listed_and_cannot_be_restored() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    private_write(
        &directory
            .path()
            .join(format!("state.json.backups/{}.json", id.uuid())),
        b"{\"backup_format_version\":1,\"sequence\":1,\"state\":{\"schema_version\":999}}",
    )?;
    assert_eq!(
        store.config_backups()?[0].status,
        ConfigBackupStatus::UnsupportedSchema(999)
    );
    assert!(matches!(
        store.preview_config_backup(id),
        Err(Error::UnsupportedSchema { found: 999, .. })
    ));
    Ok(())
}

#[test]
fn future_current_schema_requires_explicit_review_and_preserves_original() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let future = b"{\"schema_version\":999,\"future_data\":[1,2,3]}";
    private_write(store.path(), future)?;
    let preview = store.preview_config_backup(id)?;
    assert_eq!(
        preview.current_status(),
        ConfigSourceStatus::UnsupportedSchema(999)
    );
    assert_eq!(preview.current_summary(), None);
    store.restore_config_backup(&preview)?;
    let original = fs::read_dir(directory.path().join("state.json.originals"))?
        .next()
        .ok_or("original missing")??;
    assert_eq!(fs::read(original.path())?, future);
    Ok(())
}

#[test]
fn legacy_schema_one_backup_keeps_existing_default_field_migration() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let mut old = serde_json::to_value(store.load()?)?;
    old.as_object_mut().ok_or("object")?.remove("folders");
    old.as_object_mut()
        .ok_or("object")?
        .remove("connection_folders");
    let document = serde_json::json!({"backup_format_version":1,"sequence":1,"state":old});
    private_write(
        &directory
            .path()
            .join(format!("state.json.backups/{}.json", id.uuid())),
        &serde_json::to_vec(&document)?,
    )?;
    let preview = store.preview_config_backup(id)?;
    assert_eq!(store.restore_config_backup(&preview)?.connections.len(), 1);
    Ok(())
}

#[test]
fn preserved_originals_are_bounded_and_never_rotated_away() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    for index in 0..MAX_CONFIG_ORIGINALS {
        private_write(store.path(), format!("corrupt-{index}").as_bytes())?;
        let preview = store.preview_config_backup(id)?;
        store.restore_config_backup(&preview)?;
    }
    private_write(store.path(), b"ninth-corrupt-original")?;
    let preview = store.preview_config_backup(id)?;
    assert!(matches!(
        store.restore_config_backup(&preview),
        Err(Error::ConfigOriginalLimit)
    ));
    assert_eq!(fs::read(store.path())?, b"ninth-corrupt-original");
    assert_eq!(
        fs::read_dir(directory.path().join("state.json.originals"))?.count(),
        MAX_CONFIG_ORIGINALS
    );
    Ok(())
}

#[test]
fn vault_is_never_opened_copied_or_replaced_by_metadata_recovery() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let vault = directory.path().join("credentials.vault");
    let sentinel = b"opaque authenticated vault; wrong password is not supplied to this API";
    private_write(&vault, sentinel)?;
    let id = store.create_config_backup()?;
    private_write(store.path(), b"broken")?;
    store.restore_config_backup(&store.preview_config_backup(id)?)?;
    assert_eq!(fs::read(vault)?, sentinel);
    for entry in fs::read_dir(directory.path().join("state.json.backups"))? {
        assert!(
            !fs::read(entry?.path())?
                .windows(sentinel.len())
                .any(|part| part == sentinel)
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn backup_and_original_files_remain_owner_only() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    private_write(store.path(), b"corrupt")?;
    store.restore_config_backup(&store.preview_config_backup(id)?)?;
    for name in ["state.json.backups", "state.json.originals"] {
        assert_eq!(
            fs::metadata(directory.path().join(name))?
                .permissions()
                .mode()
                & 0o077,
            0
        );
        for entry in fs::read_dir(directory.path().join(name))? {
            assert_eq!(entry?.metadata()?.permissions().mode() & 0o077, 0);
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_backup_is_rejected_without_touching_its_target() -> TestResult {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let path = directory
        .path()
        .join(format!("state.json.backups/{}.json", id.uuid()));
    fs::remove_file(&path)?;
    let target = directory.path().join("outside");
    private_write(&target, b"unrelated")?;
    symlink(&target, &path)?;
    assert!(matches!(store.config_backups(), Err(Error::UnsafePath)));
    assert_eq!(fs::read(target)?, b"unrelated");
    Ok(())
}

#[test]
fn unexpected_history_file_prevents_save_and_keeps_valid_state() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    store.create_config_backup()?;
    private_write(
        &directory.path().join("state.json.backups/unrelated.json"),
        b"unrelated",
    )?;
    let original = fs::read(store.path())?;
    let mut state = store.load()?;
    state.connections[0].name = "should-not-save".into();
    assert!(matches!(store.save(&state), Err(Error::UnsafePath)));
    assert_eq!(fs::read(store.path())?, original);
    Ok(())
}

#[test]
fn wrong_vault_password_remains_rejected_and_metadata_recovery_keeps_credential_binding()
-> TestResult {
    use keelshell_core::{CredentialKind, CredentialVault, VaultStore};
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let reference = uuid::Uuid::new_v4();
    let mut state = store.load()?;
    state.connections[0].credential_ref = Some(reference);
    let owner = state.connections[0].id;
    store.save(&state)?;
    let id = store.create_config_backup()?;
    let master = "isolated-configuration-recovery-master";
    let secret = "isolated-configuration-recovery-password";
    let mut vault = CredentialVault::create(master)?;
    vault.set(reference, owner, CredentialKind::Password, secret)?;
    let encrypted = vault.to_bytes()?;
    let vault_path = directory.path().join("vault.json");
    private_write(&vault_path, &encrypted)?;
    assert!(matches!(
        VaultStore::new(&vault_path).load_existing("wrong isolated master"),
        Err(Error::VaultUnlockFailed)
    ));
    private_write(store.path(), b"metadata-only-corruption")?;
    let restored = store.restore_config_backup(&store.preview_config_backup(id)?)?;
    assert_eq!(restored.connections[0].credential_ref, Some(reference));
    assert_eq!(fs::read(&vault_path)?, encrypted);
    let reopened = VaultStore::new(&vault_path).load_existing(master)?;
    assert_eq!(
        reopened
            .get(reference, owner, CredentialKind::Password)?
            .as_str(),
        secret
    );
    for entry in fs::read_dir(directory.path().join("state.json.backups"))? {
        let bytes = fs::read(entry?.path())?;
        assert!(
            !bytes
                .windows(master.len())
                .any(|part| part == master.as_bytes())
        );
        assert!(
            !bytes
                .windows(secret.len())
                .any(|part| part == secret.as_bytes())
        );
    }
    Ok(())
}

#[test]
fn oversized_current_file_refuses_bounded_recovery_without_overwrite() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let bytes = vec![b'x'; 4 * 1024 * 1024 + 1];
    private_write(store.path(), &bytes)?;
    assert!(matches!(
        store.preview_config_backup(id),
        Err(Error::TooLarge)
    ));
    assert_eq!(fs::metadata(store.path())?.len(), bytes.len() as u64);
    assert!(!directory.path().join("state.json.originals").exists());
    Ok(())
}

#[test]
fn ninth_history_entry_fails_closed_without_pruning_any_backup() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    for _ in 0..MAX_CONFIG_BACKUPS {
        store.create_config_backup()?;
    }
    let path = directory
        .path()
        .join(format!("state.json.backups/{}.json", uuid::Uuid::new_v4()));
    private_write(&path, b"unexpected ninth backup")?;
    let before = fs::read(store.path())?;
    assert!(matches!(
        store.config_backups(),
        Err(Error::ConfigBackupLimit)
    ));
    assert!(matches!(
        store.create_config_backup(),
        Err(Error::ConfigBackupLimit)
    ));
    assert_eq!(fs::read(store.path())?, before);
    assert_eq!(
        fs::read_dir(directory.path().join("state.json.backups"))?.count(),
        9
    );
    Ok(())
}

#[test]
fn backup_sequence_overflow_refuses_save_before_primary_replacement() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = seeded(directory.path())?;
    let id = store.create_config_backup()?;
    let path = directory
        .path()
        .join(format!("state.json.backups/{}.json", id.uuid()));
    let mut document: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    document["sequence"] = serde_json::json!(u64::MAX);
    private_write(&path, &serde_json::to_vec(&document)?)?;
    let before = fs::read(store.path())?;
    let mut state = store.load()?;
    state.connections[0].name = "not saved".into();
    assert!(matches!(store.save(&state), Err(Error::ConfigBackupLimit)));
    assert_eq!(fs::read(store.path())?, before);
    Ok(())
}
