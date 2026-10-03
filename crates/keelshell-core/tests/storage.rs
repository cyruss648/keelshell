use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use keelshell_core::{AppState, AuthMethod, Connection, Error, StateStore};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

#[test]
fn absent_state_does_not_create_directories_until_saved() -> TestResult {
    let temp = tempfile::tempdir()?;
    let parent = temp.path().join("new/config");
    let store = StateStore::new(parent.join("state.json"));
    let state = store.load()?;
    assert!(!parent.exists());
    assert!(state.connections.is_empty());
    store.save(&state)?;
    assert!(store.path().is_file());
    Ok(())
}

#[test]
fn profiles_and_preferences_round_trip_through_real_disk() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut state = store.load()?;
    let mut connection = Connection::new("中国节点", "127.0.0.1", "tester");
    connection.group = "Lab".into();
    connection.auth = AuthMethod::PrivateKey {
        path: "keys/test-id".into(),
    };
    connection.tags = vec!["实验".into()];
    state.connections.push(connection);
    state.settings.font_size = 17.0;
    store.save(&state)?;
    let reopened = StateStore::new(store.path());
    assert_eq!(reopened.load()?, state);
    assert!(!fs::read_to_string(store.path())?.contains("password"));
    Ok(())
}

#[test]
fn second_store_cannot_overwrite_changes_after_its_load() -> TestResult {
    let temp = tempfile::tempdir()?;
    let first = StateStore::new(temp.path().join("state.json"));
    first.save(&AppState::default())?;
    let second = StateStore::new(first.path());
    let stale = second.load()?;
    let mut edited = first.load()?;
    edited
        .connections
        .push(Connection::new("Saved first", "first.test", "user"));
    first.save(&edited)?;
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    assert_eq!(StateStore::new(first.path()).load()?, edited);
    Ok(())
}

#[test]
fn external_change_after_absent_load_is_not_overwritten() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let stale = store.load()?;
    let other = StateStore::new(store.path());
    let mut state = AppState::default();
    state.settings.font_size = 18.0;
    other.save(&state)?;
    assert!(matches!(store.save(&stale), Err(Error::Conflict)));
    Ok(())
}

#[test]
fn corrupt_document_is_never_replaced_with_defaults() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    let corrupt = b"{partial-json";
    write_private(&path, corrupt)?;
    let store = StateStore::new(&path);
    assert!(matches!(store.load(), Err(Error::Json(_))));
    assert!(matches!(
        store.save(&AppState::default()),
        Err(Error::Json(_))
    ));
    assert_eq!(fs::read(path)?, corrupt);
    Ok(())
}

#[test]
fn old_and_future_schemas_are_preserved_without_implicit_migration() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    for version in [0, 99] {
        let document = format!("{{\"schema_version\":{version},\"future_field\":true}}");
        write_private(&path, document.as_bytes())?;
        let store = StateStore::new(&path);
        assert!(
            matches!(store.load(), Err(Error::UnsupportedSchema { found, .. }) if found == version)
        );
        assert!(matches!(
            store.save(&AppState::default()),
            Err(Error::UnsupportedSchema { .. })
        ));
        assert_eq!(fs::read_to_string(&path)?, document);
    }
    Ok(())
}

#[test]
fn existing_valid_file_requires_successful_load_before_save() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    StateStore::new(&path).save(&AppState::default())?;
    assert!(matches!(
        StateStore::new(path).save(&AppState::default()),
        Err(Error::NotLoaded)
    ));
    Ok(())
}

#[test]
fn invalid_edit_does_not_modify_last_valid_file() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut state = store.load()?;
    store.save(&state)?;
    let original = fs::read(store.path())?;
    state.settings.font_size = f32::NAN;
    assert!(matches!(store.save(&state), Err(Error::Validation(_))));
    assert_eq!(fs::read(store.path())?, original);
    Ok(())
}

#[test]
fn operating_system_lock_excludes_another_store_without_waiting() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    let store = StateStore::new(&path);
    let state = store.load()?;
    let state = store.save(&state)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("state.json.lock"))?;
    lock.try_lock()?;
    assert!(matches!(store.save(&state), Err(Error::Busy)));
    drop(lock);
    store.save(&state)?;
    Ok(())
}

#[test]
fn oversized_file_is_rejected_before_json_decoding() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    write_private(&path, b"{}")?;
    OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(4 * 1024 * 1024 + 1)?;
    assert!(matches!(StateStore::new(path).load(), Err(Error::TooLarge)));
    Ok(())
}

#[cfg(unix)]
#[test]
fn state_is_owner_only_and_insecure_existing_permissions_are_rejected() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("private/state.json"));
    store.save(&AppState::default())?;
    assert_eq!(
        fs::metadata(store.path())?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(temp.path().join("private"))?
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644))?;
    assert!(matches!(store.load(), Err(Error::InsecurePermissions)));
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_state_cannot_read_or_replace_its_target() -> TestResult {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let target = temp.path().join("outside.json");
    write_private(&target, b"do not change")?;
    let path = temp.path().join("state.json");
    symlink(&target, &path)?;
    let store = StateStore::new(path);
    assert!(matches!(store.load(), Err(Error::UnsafePath)));
    assert!(matches!(
        store.save(&AppState::default()),
        Err(Error::UnsafePath)
    ));
    assert_eq!(fs::read(target)?, b"do not change");
    Ok(())
}

#[test]
fn same_store_rejects_a_second_edited_snapshot_after_first_save() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut first = store.load()?;
    let mut second = store.load()?;
    first.settings.font_size = 18.0;
    second.settings.scrollback_lines = 2000;
    let saved = store.save(&first)?;
    assert!(matches!(store.save(&second), Err(Error::Conflict)));
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    Ok(())
}

#[test]
fn returned_snapshot_can_be_saved_again_but_previous_clone_cannot() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let original = store.load()?;
    let mut current = store.save(&original)?;
    current.settings.font_size = 20.0;
    let latest = store.save(&current)?;
    assert!(matches!(store.save(&original), Err(Error::Conflict)));
    assert!(matches!(store.save(&current), Err(Error::Conflict)));
    assert_eq!(store.load()?, latest);
    Ok(())
}

#[test]
fn concurrent_writers_on_one_store_cannot_both_commit_one_revision() -> TestResult {
    use std::sync::{Arc, Barrier};

    let temp = tempfile::tempdir()?;
    let store = Arc::new(StateStore::new(temp.path().join("state.json")));
    let original = store.load()?;
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for font_size in [18.0, 20.0] {
        let store = store.clone();
        let barrier = barrier.clone();
        let mut edited = original.clone();
        edited.settings.font_size = font_size;
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.save(&edited)
        }));
    }
    let mut successes = 0;
    let mut conflicts = 0;
    for worker in workers {
        match worker.join().map_err(|_| "state writer panicked")? {
            Ok(_) => successes += 1,
            Err(Error::Conflict) => conflicts += 1,
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!((successes, conflicts), (1, 1));
    Ok(())
}

#[test]
fn serialized_snapshots_cannot_recreate_a_store_revision() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let original = store.load()?;
    let saved = store.save(&original)?;
    let json = serde_json::to_string(&saved)?;
    assert!(!json.contains("snapshot"));
    let decoded: AppState = serde_json::from_str(&json)?;
    assert!(matches!(store.save(&decoded), Err(Error::Conflict)));
    Ok(())
}

#[test]
fn explicitly_trusted_host_pins_round_trip_through_disk() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut state = store.load()?;
    let fingerprint = format!("SHA256:{}", "A".repeat(43));
    state.trust_host_key("Example.TEST.", 22, &fingerprint)?;
    let saved = store.save(&state)?;
    assert_eq!(
        saved.host_key("example.test", 22),
        Some(fingerprint.as_str())
    );
    let loaded = StateStore::new(store.path()).load()?;
    assert_eq!(
        loaded.host_key("EXAMPLE.test", 22),
        Some(fingerprint.as_str())
    );
    assert!(loaded.host_key("example.test", 2222).is_none());
    Ok(())
}

#[test]
fn schema_one_documents_without_host_pins_remain_loadable() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    let mut document = serde_json::to_value(AppState::default())?;
    document
        .as_object_mut()
        .ok_or("expected state object")?
        .remove("known_hosts");
    write_private(&path, &serde_json::to_vec(&document)?)?;
    assert!(StateStore::new(path).load()?.known_hosts.is_empty());
    Ok(())
}
