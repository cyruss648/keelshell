//! Author regression coverage for admission before any probe job is created.

use gpui_kit::{AppContext, TestAppContext, test::TestAppContextExt};
use keelshell_ai::AiError;
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiCustomHeader, AiLocalAgent, AiModelReasoning,
    AiModelSampling, AiPreset, AiProxy, AiReasoningCapability, AiReasoningSelection,
    AiSamplingValue, AiSecretRef, NamedAiProfile,
};
use zeroize::Zeroizing;

use super::super::AiSettingsPanel;

const AGENTS: [AiLocalAgent; 2] = [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode];

fn profile(agent: AiLocalAgent) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::Custom);
    profile.name = "Probe admission fixture".into();
    profile.model = "fixture-model".into();
    profile.endpoint = "https://provider.example/v1".into();
    profile.backend = AiBackend::LocalAgent {
        agent,
        executable: std::env::temp_dir()
            .join("unused-probe-admission-fixture")
            .to_string_lossy()
            .into_owned(),
        limits: Default::default(),
    };
    (profile.api_style, profile.authentication) = match agent {
        AiLocalAgent::Codex => (
            AiApiStyle::Responses,
            AiAuthentication::Bearer { credential: None },
        ),
        AiLocalAgent::ClaudeCode => (
            AiApiStyle::AnthropicMessages,
            AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            },
        ),
    };
    profile
}

fn import(
    panel: &mut AiSettingsPanel,
    value: &str,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<AiSettingsPanel>,
) {
    panel.set_local_environment(true, window, cx);
    panel.local_environment.update(cx, |input, cx| {
        input.set_value("PROBE_IMPORT_VARIABLE", window, cx)
    });
    panel.sync_editor(cx);
    panel.read_local_environment_with(|_| Ok(Zeroizing::new(value.into())), window, cx);
}

fn assert_rejected(
    panel: &mut AiSettingsPanel,
    secret: &str,
    cx: &mut gpui_kit::Context<AiSettingsPanel>,
) {
    assert!(panel._job.is_none());
    panel.start_local_probe(cx);
    assert!(panel._job.is_none(), "rejection must precede scheduling");
    assert!(panel.operation.is_none());
    assert!(panel.cancellation.is_none());
    let status = panel.status.render(cx);
    assert!(status.contains("无法检查 CLI") || status.contains("Cannot check CLI"));
    assert!(
        !status.contains(secret),
        "diagnostic must omit secret values"
    );
}

#[gpui_kit::test]
fn probe_admission_rejects_each_selected_editor_field_before_scheduling(cx: &mut TestAppContext) {
    const SECRET: &str = "probe_metadata_value";
    for agent in AGENTS {
        for field in ["name", "endpoint", "model", "executable", "reference"] {
            let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
            cx.update_window(window, |_, window, cx| {
                panel.update(cx, |panel, cx| {
                    import(panel, SECRET, window, cx);
                    let (input, value) = match field {
                        "name" => (panel.name.clone(), SECRET.to_owned()),
                        "endpoint" => (
                            panel.endpoint.clone(),
                            format!("https://provider.example/{SECRET}"),
                        ),
                        "model" => (panel.model.clone(), SECRET.to_owned()),
                        "executable" => (
                            panel.executable.clone(),
                            std::env::temp_dir()
                                .join(SECRET)
                                .to_string_lossy()
                                .into_owned(),
                        ),
                        _ => (panel.local_environment.clone(), SECRET.to_owned()),
                    };
                    input.update(cx, |input, cx| input.set_value(value, window, cx));
                    panel.sync_editor(cx);
                    assert!(
                        panel.catalog.validate().is_ok(),
                        "valid edited metadata: {agent:?}/{field}"
                    );
                    assert!(matches!(
                        crate::ai_request_options::validate_catalog_metadata(
                            &panel.catalog,
                            &panel.credentials
                        ),
                        Err(AiError::CredentialInContext)
                    ));
                    assert_rejected(panel, SECRET, cx);
                });
            })
            .unwrap_or_else(|e| panic!("selected metadata: {e}"));
            cx.run_until_parked();
        }
    }
}

#[gpui_kit::test]
fn probe_admission_checks_hidden_profiles_and_model_fields(cx: &mut TestAppContext) {
    const SECRET: &str = "hidden_metadata_value";
    for agent in AGENTS {
        for field in 0..13 {
            let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
            cx.update_window(window, |_, window, cx| {
                panel.update(cx, |panel, cx| {
                    import(panel, SECRET, window, cx);
                    panel.set_local_environment(false, window, cx);
                    let mut other = crate::ai_settings::tests::fixture_profile();
                    other.name = "Hidden profile".into();
                    match field {
                        0 => other.name = SECRET.into(),
                        1 => other.model = SECRET.into(),
                        2 => {
                            other.endpoint =
                                format!("https://provider.example/{SECRET}/chat/completions")
                        }
                        3 => {
                            other.endpoint =
                                "https://provider.example/%68idden_metadata_value/chat/completions"
                                    .into()
                        }
                        4 => {
                            other.authentication = AiAuthentication::Bearer {
                                credential: Some(AiSecretRef::Environment {
                                    name: SECRET.into(),
                                }),
                            }
                        }
                        5 => other.custom_headers.push(AiCustomHeader {
                            name: SECRET.to_ascii_uppercase(),
                            value_ref: AiSecretRef::Ephemeral {
                                id: uuid::Uuid::new_v4(),
                            },
                        }),
                        6 => other.custom_headers.push(AiCustomHeader {
                            name: "X-Fixture".into(),
                            value_ref: AiSecretRef::Environment {
                                name: SECRET.into(),
                            },
                        }),
                        7 => {
                            other.proxy = AiProxy::Explicit {
                                url: format!("http://{SECRET}.example"),
                                credentials: None,
                            }
                        }
                        8 => {
                            other.proxy = AiProxy::Explicit {
                                url: "http://proxy.example".into(),
                                credentials: Some(AiSecretRef::Environment {
                                    name: SECRET.into(),
                                }),
                            }
                        }
                        9 => {
                            other = profile(agent);
                            other.name = "Hidden local profile".into();
                            if let AiBackend::LocalAgent { executable, .. } = &mut other.backend {
                                *executable = std::env::temp_dir()
                                    .join(SECRET)
                                    .to_string_lossy()
                                    .into_owned();
                            }
                        }
                        10 => {
                            other
                                .reasoning_by_model
                                .insert(SECRET.into(), Default::default());
                        }
                        11 => {
                            other.reasoning_by_model.insert(
                                "unselected-model".into(),
                                AiModelReasoning {
                                    capability: AiReasoningCapability::Effort {
                                        values: vec![SECRET.into()],
                                    },
                                    selection: AiReasoningSelection::Effort(SECRET.into()),
                                },
                            );
                        }
                        _ => {
                            other
                                .sampling_by_model
                                .insert(SECRET.into(), Default::default());
                        }
                    }
                    panel.catalog.profiles.push(other);
                    assert!(
                        panel.catalog.validate().is_ok(),
                        "valid hidden metadata: {agent:?}/{field}"
                    );
                    assert!(matches!(
                        crate::ai_request_options::validate_catalog_metadata(
                            &panel.catalog,
                            &panel.credentials
                        ),
                        Err(AiError::CredentialInContext)
                    ));
                    assert_rejected(panel, SECRET, cx);
                });
            })
            .unwrap_or_else(|e| panic!("hidden metadata: {e}"));
            cx.run_until_parked();
        }
    }
}

#[gpui_kit::test]
fn probe_admission_checks_numeric_secrets_in_inactive_api_sampling(cx: &mut TestAppContext) {
    for agent in AGENTS {
        for (millis, secret) in [(125, "125"), (125, "0.125"), (0, "0.0"), (1000, "1.0")] {
            for temperature in [true, false] {
                let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
                cx.update_window(window, |_, window, cx| {
                    panel.update(cx, |panel, cx| {
                        import(panel, secret, window, cx);
                        panel.set_local_environment(false, window, cx);
                        let mut other = crate::ai_settings::tests::fixture_profile();
                        other.name = "Hidden sampling profile".into();
                        let value = AiSamplingValue::from_millis(millis)
                            .unwrap_or_else(|e| panic!("sampling: {e}"));
                        other.sampling_by_model.insert(
                            "unselected-model".into(),
                            AiModelSampling {
                                declared_supported: true,
                                temperature: temperature.then_some(value),
                                top_p: (!temperature).then_some(value),
                            },
                        );
                        panel.catalog.profiles.push(other);
                        assert!(panel.catalog.validate().is_ok());
                        assert_rejected(panel, secret, cx);
                    });
                })
                .unwrap_or_else(|e| panic!("numeric metadata: {e}"));
                cx.run_until_parked();
            }
        }
    }
}

#[gpui_kit::test]
fn probe_admission_preserves_guards_after_edit_restore_lookup_and_binding_failures(
    cx: &mut TestAppContext,
) {
    const FIRST: &str = "first_retained_probe_value";
    const SECOND: &str = "second_retained_probe_value";
    for agent in AGENTS {
        let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                import(panel, FIRST, window, cx);
                let original = panel
                    .profile()
                    .unwrap_or_else(|| panic!("profile"))
                    .endpoint
                    .clone();
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value("https://changed.example/v1", window, cx)
                });
                panel.sync_editor(cx);
                panel
                    .endpoint
                    .update(cx, |input, cx| input.set_value(original, window, cx));
                panel.sync_editor(cx);
                panel.clear_pending_key(window, cx);
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_none()
                );
                panel.read_local_environment_with(|_| Err(AiError::InvalidApiKey), window, cx);
                assert!(panel.credentials.all_secrets().contains(&FIRST));
                panel
                    .model
                    .update(cx, |input, cx| input.set_value(SECOND, window, cx));
                panel.sync_editor(cx);
                // A value observed by the explicit read remains a guard even
                // when metadata prevents it from ever authorizing delivery.
                panel.read_local_environment_with(
                    |_| Ok(Zeroizing::new(SECOND.into())),
                    window,
                    cx,
                );
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_none()
                );
                assert!(panel.credentials.all_secrets().contains(&SECOND));
                assert_rejected(panel, SECOND, cx);
                panel
                    .model
                    .update(cx, |input, cx| input.set_value("fixture-model", window, cx));
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value(format!("https://provider.example/{FIRST}"), window, cx)
                });
                panel.sync_editor(cx);
                assert_rejected(panel, FIRST, cx);
            });
        })
        .unwrap_or_else(|e| panic!("retained guards: {e}"));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn probe_admission_checks_secrets_in_incomplete_hidden_drafts(cx: &mut TestAppContext) {
    const SECRET: &str = "invalid_draft_guard_value";
    for agent in AGENTS {
        for invalid in ["empty-model", "empty-name", "invalid-reference"] {
            let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
            cx.update_window(window, |_, window, cx| {
                panel.update(cx, |panel, cx| {
                    import(panel, SECRET, window, cx);
                    let mut other = crate::ai_settings::tests::fixture_profile();
                    other.name = "Incomplete hidden profile".into();
                    other.endpoint = format!("https://provider.example/{SECRET}/chat/completions");
                    match invalid {
                        "empty-model" => other.model.clear(),
                        "empty-name" => other.name.clear(),
                        _ => {
                            other.authentication = AiAuthentication::Bearer {
                                credential: Some(AiSecretRef::Environment {
                                    name: format!("9{SECRET}"),
                                }),
                            }
                        }
                    }
                    panel.catalog.profiles.push(other);
                    assert!(panel.catalog.validate().is_err());
                    assert!(matches!(
                        crate::ai_request_options::validate_catalog_secrets(
                            &panel.catalog,
                            &panel.credentials
                        ),
                        Err(AiError::CredentialInContext)
                    ));
                    assert_rejected(panel, SECRET, cx);
                });
            })
            .unwrap_or_else(|e| panic!("incomplete secret metadata: {e}"));
            cx.run_until_parked();
        }
    }
}

#[gpui_kit::test]
fn probe_admission_rejects_secret_set_bounds_without_discarding_drafts(cx: &mut TestAppContext) {
    for agent in AGENTS {
        for bound in ["count", "item", "total"] {
            let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
            cx.update_window(window, |_, _, cx| {
                panel.update(cx, |panel, cx| {
                    let values: Vec<_> = match bound {
                        "count" => (0..4097)
                            .map(|n| Zeroizing::new(format!("bounded-value-{n}")))
                            .collect(),
                        "item" => vec![Zeroizing::new("z".repeat(1024 * 1024 + 1))],
                        _ => (0..9)
                            .map(|n| Zeroizing::new(format!("{n}{}", "z".repeat(1024 * 1024 - 1))))
                            .collect(),
                    };
                    let count = values.len();
                    panel
                        .credentials
                        .retain_request_drafts(uuid::Uuid::new_v4(), values);
                    assert!(matches!(
                        crate::ai_request_options::validate_catalog_metadata(
                            &panel.catalog,
                            &panel.credentials
                        ),
                        Err(AiError::ContextTooLarge)
                    ));
                    assert_rejected(panel, "bounded-value-0", cx);
                    assert_eq!(panel.credentials.all_secrets().len(), count);
                });
            })
            .unwrap_or_else(|e| panic!("bounded draft set: {e}"));
            cx.run_until_parked();
        }
    }
}

#[gpui_kit::test]
fn probe_admission_retains_all_sixteen_imports_after_capacity_failure(cx: &mut TestAppContext) {
    for agent in AGENTS {
        let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                for n in 0..16 {
                    import(panel, &format!("imported-guard-{n}"), window, cx);
                }
                panel.read_local_environment_with(
                    |_| panic!("capacity failure must not look up"),
                    window,
                    cx,
                );
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_none()
                );
                assert_eq!(panel.credentials.all_secrets().len(), 16);
                for n in 0..16 {
                    let secret = format!("imported-guard-{n}");
                    panel
                        .model
                        .update(cx, |input, cx| input.set_value(secret.clone(), window, cx));
                    panel.sync_editor(cx);
                    assert_rejected(panel, &secret, cx);
                }
            });
        })
        .unwrap_or_else(|e| panic!("import capacity: {e}"));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
#[ignore = "Requires an explicitly supplied owned native fixture; run separately from ordinary tests"]
async fn probe_admission_legal_dual_cli_runs_only_owned_native_capability_probes(
    cx: &mut TestAppContext,
) {
    let supplied = std::env::var_os("KEELSHELL_REVIEW_PROBE_FIXTURE")
        .unwrap_or_else(|| panic!("owned fixture required"));
    for (agent, empty_model) in AGENTS
        .into_iter()
        .flat_map(|agent| [false, true].map(|empty| (agent, empty)))
    {
        let directory =
            tempfile::tempdir().unwrap_or_else(|e| panic!("private probe directory: {e}"));
        let label = match agent {
            AiLocalAgent::Codex => "codex",
            AiLocalAgent::ClaudeCode => "claude",
        };
        let executable = directory.path().join(format!(
            "{label}-review-probe-sensitive-agent-fixture{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::copy(&supplied, &executable).unwrap_or_else(|e| panic!("owned fixture copy: {e}"));
        let mut metadata = profile(agent);
        if let AiBackend::LocalAgent {
            executable: value, ..
        } = &mut metadata.backend
        {
            *value = executable.to_string_lossy().into_owned();
        }
        let (window, panel) = crate::ai_settings::tests::mount(cx, metadata);
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                import(panel, "legal-probe-private-value", window, cx);
                if empty_model {
                    panel
                        .model
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    panel.sync_editor(cx);
                    assert!(
                        panel
                            .profile()
                            .unwrap_or_else(|| panic!("profile"))
                            .model
                            .is_empty()
                    );
                }
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().unwrap_or_else(|| panic!("profile")))
                        .is_some()
                );
                panel.start_local_probe(cx);
                assert!(panel._job.is_some());
            });
        })
        .unwrap_or_else(|e| panic!("legal probe schedule: {e}"));
        cx.wait_for(window, std::time::Duration::from_secs(12), |_, cx| {
            crate::ai_settings::tests::request_has_finished(&panel, cx)
        })
        .await;
        let status = panel.read_with(cx, |panel, cx| panel.status.render(cx));
        assert!(
            status.contains("检查通过") || status.contains("capabilities checked"),
            "legal capability status: {status}"
        );
        let raw = std::fs::read_to_string(directory.path().join("review-probe-invocations.jsonl"))
            .unwrap_or_else(|e| panic!("actual invocation record: {e}"));
        let rows: Vec<serde_json::Value> = raw
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("native argv: {e}")))
            .collect();
        assert_eq!(rows.len(), if agent == AiLocalAgent::Codex { 3 } else { 2 });
        assert!(!raw.contains("legal-probe-private-value"));
        assert!(!raw.contains("PROBE_IMPORT_VARIABLE"));
        eprintln!(
            "legal-probe-boundary {}",
            serde_json::json!({"agent":label,"empty_model":empty_model,"invocation_count":rows.len(),"status":status,"records":rows,"boundary":"owned native fixture; no supplier/model/network/GUI; kernel birth census not collected"})
        );
        directory
            .close()
            .unwrap_or_else(|e| panic!("private probe directory removed: {e}"));
    }
}
