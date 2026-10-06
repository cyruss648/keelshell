use std::{fs, fs::OpenOptions, io::Write, path::Path};

use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiCustomHeader, AiLocalAgent, AiModelReasoning,
    AiPreset, AiProfileCatalog, AiProxy, AiReasoningCapability, AiReasoningSelection, AiSecretRef,
    AppState, Error, NamedAiProfile, Settings, StateStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn missing_backend_preserves_api_intent_without_rewriting_metadata() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let mut state = AppState::default();
    let profile = profile("Existing API");
    let id = profile.id;
    state.settings.ai_profiles.upsert(profile)?;
    state.settings.ai_profiles.activate(id)?;
    let mut wire = serde_json::to_value(state)?;
    wire["settings"]["ai_profiles"]["profiles"][0]
        .as_object_mut()
        .ok_or("profile object missing")?
        .remove("backend");
    write_private(&path, &wire)?;
    let before = fs::read(&path)?;
    let loaded = StateStore::new(&path).load()?;
    let profile = loaded
        .settings
        .ai_profiles
        .active()
        .ok_or("active profile missing")?;
    assert_eq!(profile.backend, AiBackend::Api);
    profile.validate_current_transport()?;
    assert_eq!(fs::read(path)?, before);
    Ok(())
}

#[test]
fn local_agent_metadata_roundtrips_and_never_falls_back_to_http() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    let mut profile = profile("Local Ask");
    profile.backend = AiBackend::LocalAgent {
        agent: AiLocalAgent::Codex,
        executable: directory
            .path()
            .join("codex")
            .to_string_lossy()
            .into_owned(),
        limits: Default::default(),
    };
    profile.api_style = AiApiStyle::Responses;
    profile.endpoint = "https://example.test/v1".to_owned();
    profile.validate_local_agent_transport()?;
    assert!(profile.validate_current_transport().is_err());
    assert!(profile.legacy_projection(true).is_err());
    let id = profile.id;
    state.settings.ai_profiles.upsert(profile.clone())?;
    state.settings.ai_profiles.activate(id)?;
    let saved = store.save(&state)?;
    let loaded = StateStore::new(store.path()).load()?;
    assert_eq!(saved, loaded);
    assert_eq!(loaded.settings.ai_profiles.active(), Some(&profile));
    assert!(
        !directory.path().join("codex").exists(),
        "metadata operations must not start or create an executable"
    );
    Ok(())
}

#[test]
fn saved_local_budgets_and_old_metadata_keep_exact_intent_without_load_rewrite() -> TestResult {
    let directory = tempfile::tempdir()?;
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let path = directory.path().join(format!("state-{agent:?}.json"));
        let mut state = AppState::default();
        let mut selected = profile("Budgeted Ask");
        selected.backend = AiBackend::LocalAgent {
            agent,
            executable: "/opt/fixture-cli".into(),
            limits: keelshell_core::AiLocalAgentLimits::new(27, 3, 19)?,
        };
        match agent {
            AiLocalAgent::Codex => selected.api_style = AiApiStyle::Responses,
            AiLocalAgent::ClaudeCode => {
                selected.api_style = AiApiStyle::AnthropicMessages;
                selected.authentication = AiAuthentication::Header {
                    name: "x-api-key".into(),
                    credential: None,
                };
            }
        }
        selected.validate_local_agent_transport()?;
        let id = selected.id;
        state.settings.ai_profiles.upsert(selected.clone())?;
        state.settings.ai_profiles.activate(id)?;
        StateStore::new(&path).save(&state)?;
        assert_eq!(
            StateStore::new(&path).load()?.settings.ai_profiles.active(),
            Some(&selected)
        );
        let mut old = serde_json::to_value(state)?;
        old["settings"]["ai_profiles"]["profiles"][0]["backend"]
            .as_object_mut()
            .ok_or("backend missing")?
            .remove("limits");
        write_private(&path, &old)?;
        let before = fs::read(&path)?;
        let loaded = StateStore::new(&path).load()?;
        let AiBackend::LocalAgent { limits, .. } = &loaded
            .settings
            .ai_profiles
            .active()
            .ok_or("active missing")?
            .backend
        else {
            return Err("local backend lost".into());
        };
        assert_eq!(*limits, keelshell_core::AiLocalAgentLimits::default());
        assert_eq!(fs::read(&path)?, before);
        old["settings"]["ai_profiles"]["profiles"][0]["backend"]["limits"] =
            json!({"answer_kib":2,"output_kib":1});
        write_private(&path, &old)?;
        let invalid = fs::read(&path)?;
        assert!(StateStore::new(&path).load().is_err());
        assert_eq!(
            fs::read(&path)?,
            invalid,
            "invalid metadata is not rewritten"
        );
    }
    Ok(())
}

#[test]
fn local_agent_request_rejects_api_options_or_wrong_authentication() -> TestResult {
    let mut profile = profile("Claude Ask");
    profile.backend = AiBackend::LocalAgent {
        agent: AiLocalAgent::ClaudeCode,
        executable: "/opt/keelshell-fixture/claude".to_owned(),
        limits: Default::default(),
    };
    profile.api_style = AiApiStyle::AnthropicMessages;
    profile.authentication = AiAuthentication::Header {
        name: "x-api-key".into(),
        credential: None,
    };
    profile.validate_local_agent_transport()?;
    profile.max_output_tokens = Some(1024);
    assert!(profile.validate_local_agent_transport().is_err());
    profile.max_output_tokens = None;
    profile.authentication = AiAuthentication::None;
    assert!(profile.validate_local_agent_transport().is_err());
    Ok(())
}

fn profile(name: &str) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = name.into();
    profile.endpoint = "https://example.test/v1/chat/completions".into();
    profile.model = "chosen-model".into();
    profile
}

fn write_private(path: &Path, value: &Value) -> TestResult {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&serde_json::to_vec(value)?)?;
    Ok(())
}

fn legacy_document(
    endpoint: &str,
    enabled: bool,
    model: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut value = serde_json::to_value(AppState::default())?;
    value["settings"]
        .as_object_mut()
        .ok_or("Settings must serialize as an object")?
        .remove("ai_profiles");
    value["settings"]["ai"] = json!({"base_url":endpoint,"enabled":enabled,"model":model});
    Ok(value)
}

#[test]
fn legacy_schema_one_migrates_on_read_and_persists_stable_named_identity() -> TestResult {
    for endpoint in [
        "https://example.test/v1",
        "https://example.test/v1/chat/completions",
        "https://example.test/v1/chat/completions/",
    ] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        write_private(&path, &legacy_document(endpoint, true, "chosen-model")?)?;
        let original = fs::read(&path)?;
        let store = StateStore::new(&path);
        let state = store.load()?;
        assert_eq!(
            fs::read(&path)?,
            original,
            "loading must not rewrite the file"
        );
        let active = state
            .settings
            .ai_profiles
            .active()
            .ok_or("missing migrated profile")?;
        assert_eq!(active.endpoint, "https://example.test/v1/chat/completions");
        assert_eq!(active.model, "chosen-model");
        assert_eq!(active.name, "默认配置");
        let id = active.id;
        let saved = store.save(&state)?;
        let reopened = StateStore::new(&path).load()?;
        assert_eq!(reopened, saved);
        assert_eq!(reopened.settings.ai_profiles.active_id, Some(id));
        assert!(fs::read_to_string(&path)?.contains("ai_profiles"));
    }
    Ok(())
}

#[test]
fn explicit_empty_or_disabled_catalog_never_resurrects_legacy_selection() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let mut document = legacy_document("https://example.test/v1", true, "legacy-model")?;
    document["settings"]["ai_profiles"] = serde_json::to_value(AiProfileCatalog::default())?;
    write_private(&path, &document)?;
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    assert!(state.settings.ai_profiles.profiles.is_empty());
    let named = profile("Replacement");
    let id = named.id;
    state.settings.ai_profiles.upsert(named)?;
    // A saved but inactive configuration must stay inactive despite legacy.enabled.
    state = store.save(&state)?;
    assert!(
        StateStore::new(&path)
            .load()?
            .settings
            .ai_profiles
            .active_id
            .is_none()
    );
    state.settings.ai_profiles.activate(id)?;
    state = store.save(&state)?;
    assert!(state.settings.ai_profiles.remove(id).is_some());
    store.save(&state)?;
    let reopened = StateStore::new(path).load()?;
    assert!(reopened.settings.ai_profiles.profiles.is_empty());
    assert!(reopened.settings.ai_profiles.active_id.is_none());
    Ok(())
}

#[test]
fn legacy_disabled_and_unselected_models_preserve_user_intent() -> TestResult {
    let unselected: AppState =
        serde_json::from_value(legacy_document("http://localhost:11434/v1", false, "")?)?;
    assert!(unselected.settings.ai_profiles.profiles.is_empty());
    let disabled: AppState = serde_json::from_value(legacy_document(
        "https://example.test/v1",
        false,
        "chosen-model",
    )?)?;
    assert_eq!(disabled.settings.ai_profiles.profiles.len(), 1);
    assert!(disabled.settings.ai_profiles.active_id.is_none());
    // An enabled legacy entry with no model is invalid, never silently invented.
    assert!(
        serde_json::from_value::<AppState>(legacy_document("https://example.test/v1", true, "",)?)
            .is_err()
    );
    Ok(())
}

#[test]
fn named_configuration_metadata_and_references_survive_real_disk_roundtrip() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    let mut named = profile("自定义供应商");
    named.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
    };
    named.custom_headers = vec![AiCustomHeader {
        name: "X-Tenant".into(),
        value_ref: AiSecretRef::Environment {
            name: "KEELSHELL_TENANT".into(),
        },
    }];
    named.proxy = AiProxy::Explicit {
        url: "socks5h://localhost:1080".into(),
        credentials: None,
    };
    named.context_window_tokens = Some(32_768);
    named.max_output_tokens = Some(4096);
    named.reasoning_by_model.insert(
        named.model.clone(),
        AiModelReasoning {
            capability: AiReasoningCapability::TokenBudget {
                min: 1024,
                max: 4096,
            },
            selection: AiReasoningSelection::Budget(2048),
        },
    );
    assert!(named.validate_current_transport().is_err());
    state.settings.ai_profiles.upsert(named)?;
    let saved = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    assert!(!saved.export_connections()?.contains("KEELSHELL_TENANT"));
    Ok(())
}

#[test]
fn invalid_catalog_edit_cannot_replace_a_valid_state_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.save(&store.load()?)?;
    let original = fs::read(store.path())?;
    state.settings.ai_profiles.active_id = Some(Uuid::new_v4());
    assert!(matches!(store.save(&state), Err(Error::Validation(_))));
    assert_eq!(fs::read(store.path())?, original);
    Ok(())
}

#[test]
fn null_catalog_unknown_fields_and_failed_migration_preserve_original_bytes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let mut null = legacy_document("https://example.test/v1", true, "chosen-model")?;
    null["settings"]["ai_profiles"] = Value::Null;
    let mut unknown = serde_json::to_value(AppState::default())?;
    unknown["settings"]["api_key"] = json!("do-not-store");
    let insecure = legacy_document("http://remote.example.test/v1", true, "chosen-model")?;
    for document in [null, unknown, insecure] {
        write_private(&path, &document)?;
        let original = fs::read(&path)?;
        let store = StateStore::new(&path);
        assert!(store.load().is_err());
        assert!(store.save(&AppState::default()).is_err());
        assert_eq!(fs::read(&path)?, original);
    }
    Ok(())
}

#[test]
fn unit_metadata_variants_refuse_inline_secret_fields() -> TestResult {
    for value in [
        json!({"kind":"none","api_key":"never-store"}),
        json!({"kind":"bearer","credential":null,"api_key":"never-store"}),
    ] {
        assert!(serde_json::from_value::<AiAuthentication>(value).is_err());
    }
    assert!(
        serde_json::from_value::<AiProxy>(json!({
            "kind":"direct","password":"never-store"
        }))
        .is_err()
    );
    for kind in ["unknown", "unsupported", "thinking_toggle"] {
        assert!(
            serde_json::from_value::<AiReasoningCapability>(json!({
                "kind":kind,"api_key":"never-store"
            }))
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<AiReasoningSelection>(json!({
            "kind":"provider_default","api_key":"never-store"
        }))
        .is_err()
    );
    // Valid empty variants still round-trip; strict decoding must not break defaults.
    let default = Settings::default();
    assert_eq!(
        serde_json::from_value::<Settings>(serde_json::to_value(&default)?)?,
        default
    );
    Ok(())
}

#[test]
fn bearer_references_admit_supported_sources_but_reject_invalid_reference_identity() -> TestResult {
    let mut named = profile("Vault provider");
    named.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
    };
    named.validate_current_transport()?;
    named.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::Environment {
            name: "UNRESOLVED_KEY".into(),
        }),
    };
    named.validate_current_transport()?;
    named.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::Ephemeral { id: Uuid::new_v4() }),
    };
    named.validate_current_transport()?;
    for reference in [
        AiSecretRef::Ephemeral { id: Uuid::nil() },
        AiSecretRef::SecretStore { id: Uuid::nil() },
        AiSecretRef::Environment {
            name: "INVALID=NAME".into(),
        },
    ] {
        named.authentication = AiAuthentication::Bearer {
            credential: Some(reference),
        };
        assert!(named.validate_current_transport().is_err());
    }
    // Metadata admission never supplies a value: explicit resolution belongs to the API caller.
    Ok(())
}

#[test]
fn local_key_environment_references_roundtrip_without_resolution_or_load_rewrite() -> TestResult {
    let directory = tempfile::tempdir()?;
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let path = directory
            .path()
            .join(format!("local-reference-{agent:?}.json"));
        let mut named = profile("Referenced local key");
        named.backend = AiBackend::LocalAgent {
            agent,
            executable: directory
                .path()
                .join("unused-native-cli")
                .to_string_lossy()
                .into(),
            limits: Default::default(),
        };
        let reference = AiSecretRef::Environment {
            name: "KEELSHELL_IMPORT_ONLY_KEY".into(),
        };
        match agent {
            AiLocalAgent::Codex => {
                named.api_style = AiApiStyle::Responses;
                named.authentication = AiAuthentication::Bearer {
                    credential: Some(reference.clone()),
                };
            }
            AiLocalAgent::ClaudeCode => {
                named.api_style = AiApiStyle::AnthropicMessages;
                named.authentication = AiAuthentication::Header {
                    name: "x-api-key".into(),
                    credential: Some(reference.clone()),
                };
            }
        }
        named.validate_local_agent_transport()?;
        let mut state = AppState::default();
        state.settings.ai_profiles.upsert(named.clone())?;
        StateStore::new(&path).save(&state)?;
        let before = fs::read(&path)?;
        let loaded = StateStore::new(&path).load()?;
        assert_eq!(loaded.settings.ai_profiles.profiles[0], named);
        assert_eq!(fs::read(&path)?, before);
        assert!(String::from_utf8(before)?.contains("KEELSHELL_IMPORT_ONLY_KEY"));
        assert!(directory.path().read_dir()?.all(|entry| {
            !entry
                .map(|entry| entry.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .contains("unused-native-cli")
        }));
        let mut wire = serde_json::to_value(state)?;
        wire["settings"]["ai_profiles"]["profiles"][0]["authentication"]["credential"] =
            json!({"source":"environment","name":"KEY=value"});
        write_private(&path, &wire)?;
        let invalid = fs::read(&path)?;
        assert!(StateStore::new(&path).load().is_err());
        assert_eq!(fs::read(&path)?, invalid);
    }
    Ok(())
}
