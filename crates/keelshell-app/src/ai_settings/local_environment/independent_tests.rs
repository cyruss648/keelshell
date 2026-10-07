use std::sync::{Arc, Mutex};

use gpui_kit::{
    AppContext, TestAppContext,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_ai::{AiError, RequestCancellation};
use keelshell_core::{
    AiAuthentication, AiBackend, AiLocalAgent, AiProfileCatalog, AppState, StateStore,
};
use zeroize::Zeroizing;

use crate::{ai_credentials::VaultAction, ai_settings::AiSettingsEvent};

fn fixture_profile_for_agent(agent: AiLocalAgent) -> keelshell_core::NamedAiProfile {
    let mut profile = keelshell_core::NamedAiProfile::draft(keelshell_core::AiPreset::Custom);
    profile.name = "Independent local fixture".into();
    profile.model = "fixture-model".into();
    profile.backend = AiBackend::LocalAgent {
        working_directory: Default::default(),
        agent,
        executable: std::env::temp_dir()
            .join("unused-review-native-cli")
            .to_string_lossy()
            .into_owned(),
        limits: Default::default(),
    };
    match agent {
        AiLocalAgent::Codex => {
            profile.api_style = keelshell_core::AiApiStyle::Responses;
            profile.endpoint = "https://provider.example/v1".into();
            profile.authentication = AiAuthentication::Bearer { credential: None };
        }
        AiLocalAgent::ClaudeCode => {
            profile.api_style = keelshell_core::AiApiStyle::AnthropicMessages;
            profile.endpoint = "https://provider.example".into();
            profile.authentication = AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            };
        }
    }
    profile
}

fn load(
    panel: &mut crate::ai_settings::AiSettingsPanel,
    value: &str,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<crate::ai_settings::AiSettingsPanel>,
) {
    panel.set_local_environment(true, window, cx);
    panel
        .local_environment
        .update(cx, |input, cx| input.set_value("KEY_VARIABLE", window, cx));
    panel.sync_editor(cx);
    panel.read_local_environment_with(|_| Ok(Zeroizing::new(value.into())), window, cx);
}

#[gpui_kit::test]
fn independent_destination_edit_and_revert_never_reactivates_import(cx: &mut TestAppContext) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        for field in ["endpoint", "executable", "reference"] {
            let (window, panel) =
                crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(agent));
            cx.update_window(window, |_, window, cx| {
                panel.update(cx, |panel, cx| {
                    load(panel, "protected-import-value", window, cx);
                    let original = panel.profile().unwrap_or_else(|| panic!("profile")).clone();
                    let before = RequestCancellation::new();
                    panel.cancellation = Some(before.clone());
                    let (input, old, new) = match field {
                        "endpoint" => (
                            panel.endpoint.clone(),
                            original.endpoint.clone(),
                            "https://other.example/v1".to_owned(),
                        ),
                        "executable" => (
                            panel.executable.clone(),
                            super::super::executable_value(&original),
                            std::env::temp_dir()
                                .join("other-native-cli")
                                .to_string_lossy()
                                .into_owned(),
                        ),
                        _ => (
                            panel.local_environment.clone(),
                            "KEY_VARIABLE".into(),
                            "OTHER_VARIABLE".into(),
                        ),
                    };
                    input.update(cx, |input, cx| input.set_value(new, window, cx));
                    panel.sync_editor(cx);
                    assert!(before.is_cancelled());
                    assert!(
                        panel
                            .credentials
                            .local_environment_key(
                                panel.profile().unwrap_or_else(|| panic!("edited"))
                            )
                            .is_none()
                    );
                    input.update(cx, |input, cx| input.set_value(old, window, cx));
                    panel.sync_editor(cx);
                    panel.clear_pending_key(window, cx);
                    assert!(
                        panel.credentials.local_environment_key(&original).is_none(),
                        "restore is not an import: {agent:?} / {field}"
                    );
                    assert!(panel.key.read(cx).value().is_empty());
                    assert!(
                        panel
                            .credentials
                            .all_secrets()
                            .contains(&"protected-import-value")
                    );
                    panel.read_local_environment_with(
                        |_| Err(AiError::InvalidRequestOptions),
                        window,
                        cx,
                    );
                    assert!(panel.credentials.local_environment_key(&original).is_none());
                })
            })
            .unwrap_or_else(|e| panic!("edit / revert: {e}"));
            cx.run_until_parked();
        }
    }
}

#[gpui_kit::test]
fn independent_environment_vault_source_switch_requires_new_explicit_import(
    cx: &mut TestAppContext,
) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let (window, panel) =
            crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(agent));
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                load(panel, "vault-separation-value", window, cx);
                let id = panel.selected.unwrap_or_else(|| panic!("id"));
                for action in [VaultAction::Save, VaultAction::Unlock] {
                    panel.begin_vault(action, window, cx);
                    assert!(panel.vault_prompt.is_none());
                    assert!(
                        panel
                            .credentials
                            .local_environment_key(
                                panel.profile().unwrap_or_else(|| panic!("profile"))
                            )
                            .is_some()
                    );
                }
                panel.set_local_environment(false, window, cx);
                assert!(panel.credentials.get(&id).is_none());
                assert!(panel.key.read(cx).value().is_empty());
                panel.key.update(cx, |input, cx| {
                    input.set_value("manual-key-value", window, cx)
                });
                panel.sync_editor(cx);
                assert_eq!(
                    panel.credentials.get(&id).map(|v| v.as_str()),
                    Some("manual-key-value")
                );
                panel.set_local_environment(true, window, cx);
                assert!(panel.credentials.get(&id).is_none());
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_none()
                );
                assert!(
                    panel
                        .credentials
                        .all_secrets()
                        .contains(&"vault-separation-value")
                );
            })
        })
        .unwrap_or_else(|e| panic!("source exclusivity: {e}"));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn independent_actual_apply_storage_reload_never_resolves_reference_or_restores_key(
    cx: &mut TestAppContext,
) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let directory = tempfile::tempdir().unwrap_or_else(|e| panic!("private store: {e}"));
        let path = directory.path().join("state.json");
        let (window, panel) =
            crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(agent));
        let emitted = Arc::new(Mutex::new(None));
        let capture = emitted.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&panel, move |_, event, _| {
                if let AiSettingsEvent::Apply {
                    catalog,
                    credentials,
                    ..
                } = event
                {
                    *capture.lock().unwrap_or_else(|e| panic!("capture: {e}")) =
                        Some((catalog.clone(), credentials.clone()));
                }
            })
        });
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                load(panel, "apply-only-secret-value", window, cx)
            });
        })
        .unwrap_or_else(|e| panic!("load: {e}"));
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("ai-settings-apply").visible());
            window.click("ai-settings-apply", cx);
        })
        .unwrap_or_else(|e| panic!("actual Apply click: {e}"));
        cx.run_until_parked();
        let (catalog, credentials) = emitted
            .lock()
            .unwrap_or_else(|e| panic!("capture: {e}"))
            .clone()
            .unwrap_or_else(|| panic!("actual Apply event"));
        assert!(
            credentials
                .local_environment_key(catalog.active().unwrap_or_else(|| panic!("active")))
                .is_some()
        );
        let mut state = AppState::default();
        state.settings.ai_profiles = catalog.clone();
        StateStore::new(&path)
            .save(&state)
            .unwrap_or_else(|e| panic!("disk save: {e}"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("saved bytes: {e}"));
        let text = String::from_utf8(bytes.clone()).unwrap_or_else(|e| panic!("JSON: {e}"));
        assert!(text.contains("KEY_VARIABLE"));
        assert!(!text.contains("apply-only-secret-value"));
        let loaded = StateStore::new(&path)
            .load()
            .unwrap_or_else(|e| panic!("reload: {e}"));
        assert_eq!(loaded.settings.ai_profiles, catalog);
        assert_eq!(
            std::fs::read(&path).unwrap_or_else(|e| panic!("unchanged: {e}")),
            bytes
        );
        let reopened = crate::ai_request_options::EphemeralCredentials::new();
        assert!(
            reopened
                .local_environment_key(
                    loaded
                        .settings
                        .ai_profiles
                        .active()
                        .unwrap_or_else(|| panic!("active"))
                )
                .is_none()
        );
        directory
            .close()
            .unwrap_or_else(|e| panic!("private delete: {e}"));
    }
}

#[gpui_kit::test]
fn independent_invalid_reference_never_looks_up_and_preserves_existing_guard(
    cx: &mut TestAppContext,
) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let (window, panel) =
            crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(agent));
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                load(panel, "first-private-value", window, cx);
                for invalid in ["", "KEY=value", "9KEY", "éKEY"] {
                    panel
                        .local_environment
                        .update(cx, |input, cx| input.set_value(invalid, window, cx));
                    panel.sync_editor(cx);
                    panel.read_local_environment_with(
                        |_| panic!("invalid reference must not look up"),
                        window,
                        cx,
                    );
                    assert_eq!(panel.local_environment.read(cx).value(), invalid);
                    assert!(
                        panel
                            .credentials
                            .local_environment_key(
                                panel.profile().unwrap_or_else(|| panic!("profile"))
                            )
                            .is_none()
                    );
                    assert!(
                        panel
                            .credentials
                            .all_secrets()
                            .contains(&"first-private-value")
                    );
                    panel.apply(cx);
                    assert!(!panel.saving);
                }
            })
        })
        .unwrap_or_else(|e| panic!("invalid reference: {e}"));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn independent_observed_import_rejects_active_and_inactive_metadata_after_unbind(
    cx: &mut TestAppContext,
) {
    let (window, panel) =
        crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(AiLocalAgent::Codex));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            load(panel, "0.125", window, cx);
            panel.set_local_environment(false, window, cx);
            assert!(panel.credentials.all_secrets().contains(&"0.125"));
            let mut inactive = crate::ai_settings::tests::fixture_profile();
            inactive.name = "Other API".into();
            inactive.sampling_by_model.insert(
                inactive.model.clone(),
                keelshell_core::AiModelSampling {
                    declared_supported: true,
                    temperature: Some(
                        keelshell_core::AiSamplingValue::from_millis(125)
                            .unwrap_or_else(|e| panic!("sample: {e}")),
                    ),
                    top_p: None,
                },
            );
            panel.catalog.profiles.push(inactive);
            assert!(
                crate::ai_request_options::validate_catalog_metadata(
                    &panel.catalog,
                    &panel.credentials
                )
                .is_err()
            );
            panel.apply(cx);
            assert!(!panel.saving);
            panel.catalog.profiles.pop();
            panel
                .model
                .update(cx, |input, cx| input.set_value("model-0.125", window, cx));
            panel.sync_editor(cx);
            panel.apply(cx);
            assert!(!panel.saving);
            assert!(panel.credentials.all_secrets().contains(&"0.125"));
        })
    })
    .unwrap_or_else(|e| panic!("inactive metadata: {e}"));
}

#[test]
fn independent_purpose_binding_rejects_all_receiver_identity_changes() {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let mut original = fixture_profile_for_agent(agent);
        match &mut original.authentication {
            AiAuthentication::Bearer { credential }
            | AiAuthentication::Header { credential, .. } => {
                *credential = Some(keelshell_core::AiSecretRef::Environment {
                    name: "KEY_VARIABLE".into(),
                })
            }
            AiAuthentication::None => panic!("fixed auth"),
        }
        let mut credentials = crate::ai_request_options::EphemeralCredentials::new();
        credentials
            .retain_local_environment(original.id, Zeroizing::new("unique-import-value".into()));
        credentials.bind_local_environment(&original, Zeroizing::new("unique-import-value".into()));
        for variant in 0..6 {
            let mut changed = original.clone();
            match variant {
                0 => changed.id = uuid::Uuid::new_v4(),
                1 => changed.endpoint.push_str("/other"),
                2 => {
                    if let AiBackend::LocalAgent { executable, .. } = &mut changed.backend {
                        executable.push_str("-other");
                    }
                }
                3 => {
                    if let AiBackend::LocalAgent { agent, .. } = &mut changed.backend {
                        *agent = match agent {
                            AiLocalAgent::Codex => AiLocalAgent::ClaudeCode,
                            AiLocalAgent::ClaudeCode => AiLocalAgent::Codex,
                        };
                    }
                }
                4 => changed.api_style = keelshell_core::AiApiStyle::ChatCompletions,
                _ => match &mut changed.authentication {
                    AiAuthentication::Bearer { credential }
                    | AiAuthentication::Header { credential, .. } => {
                        *credential = Some(keelshell_core::AiSecretRef::Environment {
                            name: "OTHER_VARIABLE".into(),
                        })
                    }
                    AiAuthentication::None => panic!("fixed auth"),
                },
            }
            assert!(
                credentials.local_environment_key(&changed).is_none(),
                "{agent:?} / receiver variant {variant}"
            );
        }
        original.name = "renamed".into();
        original.model = "other-model".into();
        assert_eq!(
            credentials
                .local_environment_key(&original)
                .map(|v| v.as_str()),
            Some("unique-import-value")
        );
        let mut catalog = AiProfileCatalog {
            active_id: Some(original.id),
            profiles: vec![original],
        };
        catalog.profiles[0].name = "contains-unique-import-value".into();
        credentials.remove(&catalog.profiles[0].id);
        assert!(
            crate::ai_request_options::validate_catalog_metadata(&catalog, &credentials).is_err()
        );
    }
}

#[gpui_kit::test]
fn independent_single_line_reference_normalization_is_exact_before_explicit_lookup(
    cx: &mut TestAppContext,
) {
    let (window, panel) =
        crate::ai_settings::tests::mount(cx, fixture_profile_for_agent(AiLocalAgent::Codex));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_local_environment(true, window, cx);
            panel
                .local_environment
                .update(cx, |input, cx| input.set_value("_KEY\n", window, cx));
            assert_eq!(panel.local_environment.read(cx).value(), "_KEY");
            panel.sync_editor(cx);
            assert!(
                panel
                    .credentials
                    .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                    .is_none()
            );
            panel.read_local_environment_with(
                |name| {
                    assert_eq!(name, "_KEY");
                    Ok(Zeroizing::new("normalized-source-value".into()))
                },
                window,
                cx,
            );
            assert_eq!(super::reference_name(panel.profile()), Some("_KEY"));
            assert!(
                panel
                    .credentials
                    .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                    .is_some()
            );
        })
    })
    .unwrap_or_else(|e| panic!("single line reference normalization: {e}"));
    cx.run_until_parked();
}

#[gpui_kit::test]
async fn independent_cli_probe_must_reject_observed_secret_metadata_before_any_job(
    cx: &mut TestAppContext,
) {
    const SECRET: &str = "probe_binding_value";
    let mut violations = Vec::new();
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let directory = tempfile::tempdir().unwrap_or_else(|e| panic!("private probe tree: {e}"));
        let supplied = std::env::var_os("KEELSHELL_REVIEW_PROBE_FIXTURE");
        let label = match agent {
            AiLocalAgent::Codex => "codex",
            AiLocalAgent::ClaudeCode => "claude",
        };
        let initial = directory.path().join(format!(
            "{label}-review-probe-sensitive-agent-fixture{}",
            std::env::consts::EXE_SUFFIX
        ));
        if let Some(ref supplied) = supplied {
            std::fs::copy(supplied, &initial).unwrap_or_else(|e| panic!("owned fixture copy: {e}"));
        }
        let mut metadata = fixture_profile_for_agent(agent);
        if let AiBackend::LocalAgent { executable, .. } = &mut metadata.backend {
            *executable = initial.to_string_lossy().into_owned();
        }
        let (window, panel) = crate::ai_settings::tests::mount(cx, metadata);
        let mut scheduled = false;
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                load(panel, SECRET, window, cx);
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_some()
                );
                if agent == AiLocalAgent::Codex {
                    panel.endpoint.update(cx, |input, cx| {
                        input.set_value(format!("https://provider.example/{SECRET}"), window, cx)
                    });
                } else {
                    let modified = directory.path().join(format!(
                        "claude-review-probe-sensitive-agent-fixture-{SECRET}{}",
                        std::env::consts::EXE_SUFFIX
                    ));
                    if supplied.is_some() {
                        std::fs::copy(&initial, &modified)
                            .unwrap_or_else(|e| panic!("second owned fixture copy: {e}"));
                    }
                    panel.executable.update(cx, |input, cx| {
                        input.set_value(modified.to_string_lossy().into_owned(), window, cx)
                    });
                }
                panel.sync_editor(cx);
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_none()
                );
                assert!(panel.credentials.all_secrets().contains(&SECRET));
                assert!(
                    crate::ai_request_options::validate_catalog_metadata(
                        &panel.catalog,
                        &panel.credentials
                    )
                    .is_err()
                );
                panel.apply(cx);
                assert!(!panel.saving);
                panel.start_local_probe(cx);
                scheduled = panel._job.is_some();
            })
        })
        .unwrap_or_else(|e| panic!("probe boundary: {e}"));
        cx.wait_for(window, std::time::Duration::from_secs(12), |_, cx| {
            crate::ai_settings::tests::request_has_finished(&panel, cx)
        })
        .await;
        let status = panel.read_with(cx, |panel, cx| panel.status.render(cx));
        let recorder = directory.path().join("review-probe-invocations.jsonl");
        let raw = std::fs::read_to_string(&recorder).unwrap_or_default();
        let rows: Vec<serde_json::Value> = raw
            .lines()
            .map(|line| {
                serde_json::from_str(line).unwrap_or_else(|e| panic!("owned argv record: {e}"))
            })
            .collect();
        let secret_in_argv = rows.iter().any(|row| {
            row["argv"].as_array().is_some_and(|argv| {
                argv.iter()
                    .any(|arg| arg.as_str().is_some_and(|text| text.contains(SECRET)))
            })
        });
        eprintln!(
            "probe-boundary {}",
            serde_json::json!({"agent":label,"job_scheduled":scheduled,"actual_fixture":supplied.is_some(),"invocation_count":rows.len(),"secret_in_argv":secret_in_argv,"status":status,"records":rows,"boundary":"self-hosted compiled fixture, no supplier/model/network; kernel birth census not collected"})
        );
        directory
            .close()
            .unwrap_or_else(|e| panic!("private probe tree removed: {e}"));
        violations.push((agent, scheduled, secret_in_argv));
    }
    assert!(
        violations
            .iter()
            .all(|(_, scheduled, secret_in_argv)| !*scheduled && !*secret_in_argv),
        "known-secret probe must be rejected before scheduling: {violations:?}"
    );
}
