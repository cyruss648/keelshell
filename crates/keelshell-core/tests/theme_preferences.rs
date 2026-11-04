use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use keelshell_core::{
    AiProfileCatalog, AppState, Error, Language, Settings, StateStore, Theme, ThemeId,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
fn legacy_missing_family_defaults_to_frost_without_changing_appearance_or_disk() -> TestResult {
    for (theme, appearance) in [
        (Theme::System, None),
        (Theme::System, Some("system")),
        (Theme::Light, Some("light")),
        (Theme::Dark, Some("dark")),
    ] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        let mut expected = AppState::default();
        expected.settings.language = Language::En;
        expected.settings.font_size = 19.0;
        expected.settings.scrollback_lines = 4_000;
        expected.settings.theme = theme;
        let mut document = serde_json::to_value(&expected)?;
        let settings = document["settings"]
            .as_object_mut()
            .ok_or("settings object missing")?;
        settings.remove("theme_family");
        match appearance {
            Some(value) => {
                settings.insert("theme".into(), json!(value));
            }
            None => {
                settings.remove("theme");
            }
        }
        write_private(&path, &document)?;
        let original = fs::read(&path)?;

        let store = StateStore::new(&path);
        let loaded = store.load()?;
        assert_eq!(loaded.settings.theme_family.as_str(), "frost");
        assert_eq!(loaded.settings.theme, theme);
        assert_eq!(loaded, expected);
        assert_eq!(fs::read(&path)?, original, "loading must not rewrite state");

        let saved = store.save(&loaded)?;
        let written: Value = serde_json::from_slice(&fs::read(&path)?)?;
        assert_eq!(written, serde_json::to_value(&expected)?);
        assert_eq!(StateStore::new(&path).load()?, saved);
    }
    Ok(())
}

#[test]
fn all_theme_families_round_trip_through_state_store_independently_of_appearance() -> TestResult {
    for family in ["frost", "graphite", "lavender"] {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            let directory = tempfile::tempdir()?;
            let store = StateStore::new(directory.path().join("state.json"));
            let mut state = store.load()?;
            state.settings.language = Language::En;
            state.settings.font_size = 21.0;
            state.settings.scrollback_lines = 7_000;
            state.settings.theme = theme;
            let mut expected = serde_json::to_value(&state)?;
            expected["settings"]["theme_family"] = json!(family);

            state.settings.theme_family = ThemeId::new(family)?;
            let saved = store.save(&state)?;
            let written: Value = serde_json::from_slice(&fs::read(store.path())?)?;
            assert_eq!(written, expected, "only the family preference may change");
            let reopened = StateStore::new(store.path()).load()?;
            assert_eq!(reopened.settings.theme_family.as_str(), family);
            assert_eq!(reopened.settings.theme, theme);
            assert_eq!(reopened, saved);
        }
    }
    Ok(())
}

#[test]
fn unknown_valid_family_survives_reload_and_unrelated_preference_save() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let mut document = serde_json::to_value(AppState::default())?;
    document["settings"]["theme_family"] = json!("future-family-v2");
    document["settings"]["theme"] = json!("light");
    write_private(&path, &document)?;
    let original = fs::read(&path)?;

    let store = StateStore::new(&path);
    let loaded = store.load()?;
    assert_eq!(loaded.settings.theme_family.as_str(), "future-family-v2");
    assert_eq!(loaded.settings.theme, Theme::Light);
    assert_eq!(fs::read(&path)?, original);
    let saved = store.save(&loaded)?;

    let reopened_store = StateStore::new(&path);
    let mut reopened = reopened_store.load()?;
    assert_eq!(reopened, saved);
    reopened.settings.theme = Theme::Dark;
    reopened.settings.font_size = 23.0;
    let saved = reopened_store.save(&reopened)?;
    let final_state = StateStore::new(&path).load()?;
    assert_eq!(final_state, saved);
    assert_eq!(
        final_state.settings.theme_family.as_str(),
        "future-family-v2"
    );
    assert_eq!(final_state.settings.theme, Theme::Dark);
    document["settings"]["theme"] = json!("dark");
    document["settings"]["font_size"] = json!(23.0);
    let written: Value = serde_json::from_slice(&fs::read(&path)?)?;
    assert_eq!(written, document);
    Ok(())
}

#[test]
fn invalid_or_null_family_is_rejected_without_overwriting_state() -> TestResult {
    for family in [
        json!(""),
        json!("Frost"),
        json!("1frost"),
        json!("-frost"),
        json!("frost/other"),
        json!("frost\n"),
        json!("fröst"),
        json!("a".repeat(65)),
        json!(null),
        json!(true),
        json!(1),
        json!(["frost"]),
        json!({"id": "frost"}),
    ] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        let mut document = serde_json::to_value(AppState::default())?;
        document["settings"]["theme_family"] = family;
        write_private(&path, &document)?;
        let original = fs::read(&path)?;
        let store = StateStore::new(&path);

        assert!(matches!(store.load(), Err(Error::Json(_))));
        assert!(matches!(
            store.save(&AppState::default()),
            Err(Error::Json(_))
        ));
        assert_eq!(fs::read(&path)?, original);
    }
    Ok(())
}

#[test]
fn theme_family_preserves_custom_legacy_ai_migration_and_explicit_catalog_rules() -> TestResult {
    for family in [None, Some("lavender")] {
        let mut document = json!({
            "font_size": 19.0,
            "scrollback_lines": 4_000,
            "theme": "dark",
            "ai": {
                "enabled": true,
                "base_url": "https://example.test/v1",
                "model": "legacy-model"
            }
        });
        if let Some(family) = family {
            document["theme_family"] = json!(family);
        }
        let migrated: Settings = serde_json::from_value(document.clone())?;
        assert_eq!(migrated.theme_family.as_str(), family.unwrap_or("frost"));
        assert_eq!(migrated.theme, Theme::Dark);
        let active = migrated
            .ai_profiles
            .active()
            .ok_or("legacy AI profile did not migrate")?;
        assert_eq!(active.model, "legacy-model");
        assert_eq!(active.endpoint, "https://example.test/v1/chat/completions");

        document["ai_profiles"] = serde_json::to_value(AiProfileCatalog::default())?;
        let explicit: Settings = serde_json::from_value(document.clone())?;
        assert_eq!(explicit.theme_family, migrated.theme_family);
        assert_eq!(explicit.theme, migrated.theme);
        assert_eq!(explicit.ai_profiles, AiProfileCatalog::default());

        document["ai_profiles"] = Value::Null;
        assert!(serde_json::from_value::<Settings>(document).is_err());
    }
    Ok(())
}
