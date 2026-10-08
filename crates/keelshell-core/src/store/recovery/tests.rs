use super::*;

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}

impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, operation: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{operation}: {error:?}"),
        }
    }
}

impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self, operation: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{operation}"),
        }
    }
}

#[test]
fn post_commit_sync_failure_rolls_back_corrupt_original_and_keeps_preserved_copy() {
    let directory = tempfile::tempdir().checked("isolated directory");
    let store = StateStore::new(directory.path().join("state.json"));
    store
        .save(&AppState::default())
        .checked("seed valid metadata");
    let id = store.create_config_backup().checked("backup");
    let corrupt = b"incomplete-json";
    fs::write(store.path(), corrupt).checked("existing owner-only file");
    let preview = store
        .preview_config_backup(id)
        .checked("review damaged state");
    let result = store.restore_with(&preview, |_| {
        Err(std::io::Error::other("controlled sync failure"))
    });
    assert!(matches!(result, Err(Error::ConfigRecoveryRolledBack(_))));
    assert_eq!(fs::read(store.path()).checked("rolled-back bytes"), corrupt);
    let original = fs::read_dir(directory.path().join("state.json.originals"))
        .checked("originals")
        .next()
        .checked("one original")
        .checked("entry");
    assert_eq!(
        fs::read(original.path()).checked("preserved bytes"),
        corrupt
    );
    assert!(matches!(store.load(), Err(Error::Json(_))));
}

#[test]
fn post_commit_sync_failure_rolls_back_missing_current_file_to_absence() {
    let directory = tempfile::tempdir().checked("isolated directory");
    let store = StateStore::new(directory.path().join("state.json"));
    store
        .save(&AppState::default())
        .checked("seed valid metadata");
    let id = store.create_config_backup().checked("backup");
    fs::remove_file(store.path()).checked("remove current state");
    let preview = store
        .preview_config_backup(id)
        .checked("review absent state");
    let result = store.restore_with(&preview, |_| {
        Err(std::io::Error::other("controlled sync failure"))
    });
    assert!(matches!(result, Err(Error::ConfigRecoveryRolledBack(_))));
    assert!(!store.path().exists());
    assert!(!directory.path().join("state.json.originals").exists());
}

#[test]
fn failed_rollback_retains_original_and_discards_loaded_baseline() {
    let directory = tempfile::tempdir().checked("isolated directory");
    let store = StateStore::new(directory.path().join("state.json"));
    store
        .save(&AppState::default())
        .checked("seed valid metadata");
    let id = store.create_config_backup().checked("backup");
    let original = fs::read(store.path()).checked("original bytes");
    let preview = store
        .preview_config_backup(id)
        .checked("review valid state");
    let result = store.restore_with(&preview, |parent| {
        fs::remove_file(parent.join("state.json"))?;
        fs::create_dir(parent.join("state.json"))?;
        Err(std::io::Error::other("controlled destination obstruction"))
    });
    assert!(matches!(result, Err(Error::ConfigRecoveryRequired)));
    let saved = fs::read_dir(directory.path().join("state.json.originals"))
        .checked("originals")
        .next()
        .checked("one original")
        .checked("entry");
    assert_eq!(fs::read(saved.path()).checked("preserved copy"), original);
    assert!(matches!(
        *store.observed.lock().checked("baseline"),
        Observed::Unloaded
    ));
}
