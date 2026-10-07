use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use keelshell_core::{AppState, Error, StateStore, UpdateCheckFrequency, UpdatePreferences};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LAST_SUPPORTED_UNIX_SECONDS: u64 = 253_402_300_799;
const CHECK_METADATA_FIELD: &str = "settings.updates.last_successful_check";

fn write_private(path: &Path, value: &Value) -> TestResult {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&serde_json::to_vec(value)?)?;
    Ok(())
}

#[test]
fn legacy_settings_without_updates_migrate_without_rewriting_original_bytes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let mut expected = AppState::default();
    expected.settings.font_size = 19.0;
    expected.settings.scrollback_lines = 4_000;
    let mut document = serde_json::to_value(&expected)?;
    document["settings"]
        .as_object_mut()
        .ok_or("settings object missing")?
        .remove("updates");
    write_private(&path, &document)?;
    let original = fs::read(&path)?;

    let store = StateStore::new(&path);
    let loaded = store.load()?;
    assert_eq!(
        loaded.settings.updates.frequency,
        UpdateCheckFrequency::Daily
    );
    assert!(!loaded.settings.updates.auto_download);
    assert_eq!(loaded.settings.updates.last_successful_check, None);
    assert_eq!(loaded, expected);
    assert_eq!(
        fs::read(&path)?,
        original,
        "loading must not rewrite old state"
    );

    let saved = store.save(&loaded)?;
    let written: Value = serde_json::from_slice(&fs::read(&path)?)?;
    assert_eq!(written, serde_json::to_value(&expected)?);
    assert_eq!(StateStore::new(&path).load()?, saved);
    Ok(())
}

#[test]
fn every_saved_update_policy_and_successful_check_round_trip_through_real_disk() -> TestResult {
    for frequency in [
        UpdateCheckFrequency::Disabled,
        UpdateCheckFrequency::Daily,
        UpdateCheckFrequency::Weekly,
    ] {
        for auto_download in [false, true] {
            let directory = tempfile::tempdir()?;
            let store = StateStore::new(directory.path().join("state.json"));
            let mut state = store.load()?;
            let preferences = UpdatePreferences {
                frequency,
                auto_download,
                last_successful_check: Some(1_700_000_000),
            };
            state.settings.updates = preferences;
            state.settings.font_size = 18.0;
            let saved = store.save(&state)?;

            let document: Value = serde_json::from_slice(&fs::read(store.path())?)?;
            assert_eq!(
                document["settings"]["updates"],
                serde_json::to_value(preferences)?
            );
            assert!(
                document["settings"]["updates"]
                    .get("auto_install")
                    .is_none()
            );
            let reopened = StateStore::new(store.path()).load()?;
            assert_eq!(reopened.settings.updates, preferences);
            assert_eq!(reopened, saved);
        }
    }
    Ok(())
}

#[test]
fn successful_check_metadata_save_preserves_policy_and_unrelated_settings() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    state.settings.updates = UpdatePreferences {
        frequency: UpdateCheckFrequency::Weekly,
        auto_download: true,
        last_successful_check: None,
    };
    state.settings.font_size = 21.0;
    state.settings.scrollback_lines = 7_000;
    let mut state = store.save(&state)?;
    let before: Value = serde_json::from_slice(&fs::read(store.path())?)?;
    assert!(
        before["settings"]["updates"]
            .get("last_successful_check")
            .is_none()
    );

    state.settings.updates.last_successful_check = Some(1_700_086_400);
    let saved = store.save(&state)?;
    let reopened = StateStore::new(store.path()).load()?;
    assert_eq!(reopened, saved);
    let mut expected = before;
    expected["settings"]["updates"]["last_successful_check"] = json!(1_700_086_400_u64);
    let after: Value = serde_json::from_slice(&fs::read(store.path())?)?;
    assert_eq!(
        after, expected,
        "check metadata must not replace other settings"
    );
    Ok(())
}

fn assert_invalid_document_preserved(
    label: &str,
    preferences: Value,
    validation_error: bool,
) -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let stale = store.load()?;
    let mut document = serde_json::to_value(AppState::default())?;
    document["settings"]["updates"] = preferences;
    write_private(&path, &document)?;
    let original = fs::read(&path)?;

    // A failed load must neither silently migrate present data nor allow a
    // previously loaded snapshot or defaults to overwrite the rejected bytes.
    for result in [
        store.load(),
        store.save(&stale),
        store.save(&AppState::default()),
    ] {
        if validation_error {
            assert!(
                matches!(result, Err(Error::Validation(error))
                    if error.field == CHECK_METADATA_FIELD),
                "{label} must fail domain validation"
            );
        } else {
            assert!(
                matches!(result, Err(Error::Json(_))),
                "{label} must fail decoding"
            );
        }
        assert_eq!(
            fs::read(&path)?,
            original,
            "{label} must retain original bytes"
        );
    }
    Ok(())
}

#[test]
fn present_null_unknown_fields_and_invalid_types_cannot_be_migrated_or_overwritten() -> TestResult {
    for (label, preferences) in [
        ("null preferences", Value::Null),
        ("string preferences", json!("daily")),
        ("array preferences", json!([])),
        ("missing frequency", json!({"auto_download": false})),
        ("missing auto_download", json!({"frequency": "daily"})),
        (
            "unknown preference field",
            json!({"frequency": "daily", "auto_download": false, "auto_install": true}),
        ),
        (
            "unknown frequency",
            json!({"frequency": "hourly", "auto_download": false}),
        ),
        (
            "numeric frequency",
            json!({"frequency": 1, "auto_download": false}),
        ),
        (
            "string download flag",
            json!({"frequency": "daily", "auto_download": "yes"}),
        ),
        (
            "null download flag",
            json!({"frequency": "daily", "auto_download": null}),
        ),
        (
            "negative check timestamp",
            json!({"frequency": "daily", "auto_download": false, "last_successful_check": -1}),
        ),
        (
            "string check timestamp",
            json!({"frequency": "daily", "auto_download": false, "last_successful_check": "1"}),
        ),
    ] {
        assert_invalid_document_preserved(label, preferences, false)?;
    }
    Ok(())
}

#[test]
fn out_of_range_check_timestamps_preserve_the_rejected_document() -> TestResult {
    for seconds in [LAST_SUPPORTED_UNIX_SECONDS + 1, u64::MAX] {
        assert_invalid_document_preserved(
            "out-of-range check timestamp",
            json!({
                "frequency": "weekly",
                "auto_download": true,
                "last_successful_check": seconds,
            }),
            true,
        )?;
    }
    Ok(())
}

#[test]
fn timestamp_boundaries_round_trip_and_invalid_edits_preserve_last_valid_state() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    for seconds in [0, LAST_SUPPORTED_UNIX_SECONDS] {
        state.settings.updates.last_successful_check = Some(seconds);
        state = store.save(&state)?;
        assert_eq!(StateStore::new(store.path()).load()?, state);
    }
    let original = fs::read(store.path())?;
    let mut invalid = state.clone();
    invalid.settings.updates.last_successful_check = Some(LAST_SUPPORTED_UNIX_SECONDS + 1);
    assert!(matches!(store.save(&invalid), Err(Error::Validation(error))
            if error.field == CHECK_METADATA_FIELD));
    assert_eq!(fs::read(store.path())?, original);
    assert_eq!(StateStore::new(store.path()).load()?, state);
    Ok(())
}

#[test]
fn stale_successful_check_cannot_overwrite_another_instances_new_policy() -> TestResult {
    for (frequency, auto_download) in [
        (UpdateCheckFrequency::Disabled, false),
        (UpdateCheckFrequency::Weekly, true),
    ] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        let checking_instance = StateStore::new(&path);
        let mut initial = checking_instance.load()?;
        initial.settings.updates.last_successful_check = Some(1_700_000_000);
        let mut stale_check = checking_instance.save(&initial)?;

        let editing_instance = StateStore::new(&path);
        let mut edited = editing_instance.load()?;
        edited.settings.updates.frequency = frequency;
        edited.settings.updates.auto_download = auto_download;
        edited.settings.font_size = 23.0;
        let winner = editing_instance.save(&edited)?;
        let original = fs::read(&path)?;

        stale_check.settings.updates.last_successful_check = Some(1_700_086_400);
        let retained_draft = stale_check.clone();
        assert!(matches!(
            checking_instance.save(&stale_check),
            Err(Error::Conflict)
        ));
        assert_eq!(fs::read(&path)?, original);
        assert_eq!(StateStore::new(&path).load()?, winner);
        assert_eq!(stale_check, retained_draft);
        assert_eq!(stale_check.snapshot, retained_draft.snapshot);
    }
    Ok(())
}
