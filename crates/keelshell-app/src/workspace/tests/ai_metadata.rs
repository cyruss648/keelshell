//! Test the real Apply subscription and StateStore, including a direct event bypass.

use super::{Checked, mount, mount_sized};
use crate::{ai_request_options::EphemeralCredentials, ai_settings::AiSettingsEvent};
use gpui_kit::{
    AppContext, TestAppContext,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{
    AiAuthentication, AiCustomHeader, AiPreset, AiProfileCatalog, AiProxy, AiReasoningSelection,
    AiSecretRef, NamedAiProfile,
};
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

fn fixture_profile() -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "Owned persistence profile".into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "ordinary-model".into();
    profile.authentication = AiAuthentication::None;
    profile
}

#[gpui_kit::test]
async fn metadata_apply_event_cannot_write_known_secrets_and_safe_snapshot_persists(
    cx: &mut TestAppContext,
) {
    const SECRET: &str = "owned-persistence-secret";
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.open_ai_settings(window, cx));
    })
    .checked("open real AI settings subscription");
    let panel = fixture
        .workspace
        .read_with(cx, |view, _| view.ai_settings.clone())
        .unwrap_or_else(|| panic!("settings panel"));
    let before = std::fs::read(fixture.store.path()).checked("read original owned state");
    let mut credentials = EphemeralCredentials::new();
    credentials.retain_request_drafts(Uuid::new_v4(), vec![Zeroizing::new(SECRET.into())]);
    for field in 0..4 {
        let mut profile = fixture_profile();
        match field {
            0 => profile.custom_headers.push(AiCustomHeader {
                name: SECRET.to_ascii_uppercase(),
                value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
            }),
            1 => profile.model = SECRET.into(),
            2 => profile.endpoint = format!("https://provider.example/{SECRET}/chat/completions"),
            _ => {
                profile.proxy = AiProxy::Explicit {
                    url: format!("http://{SECRET}.example"),
                    credentials: None,
                }
            }
        }
        let catalog = AiProfileCatalog {
            active_id: Some(profile.id),
            profiles: vec![profile],
        };
        assert!(catalog.validate().is_ok());
        // A consumer must guard the immutable event too, even if a future
        // producer accidentally omits the panel's validation.
        panel.update(cx, |_, cx| {
            cx.emit(AiSettingsEvent::Apply {
                catalog,
                credentials: credentials.clone(),
                revision: field,
            })
        });
        cx.run_until_parked();
        fixture.workspace.read_with(cx, |view, cx| {
            assert!(!view.saving);
            assert!(view.state.settings.ai_profiles.profiles.is_empty());
            assert!(!view.status.render(cx).contains(SECRET));
        });
        assert_eq!(
            std::fs::read(fixture.store.path()).checked("guarded state bytes"),
            before
        );
    }
    let mut profile = fixture_profile();
    profile.custom_headers.push(AiCustomHeader {
        name: "X-Approved-Safe".into(),
        value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
    });
    let catalog = AiProfileCatalog {
        active_id: Some(profile.id),
        profiles: vec![profile],
    };
    let saved_catalog = catalog.clone();
    panel.update(cx, |_, cx| {
        cx.emit(AiSettingsEvent::Apply {
            catalog,
            credentials,
            revision: 0,
        })
    });
    cx.run_until_parked();
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let saved = fixture
        .store
        .load()
        .checked("load actual saved safe metadata");
    assert_eq!(saved.settings.ai_profiles, saved_catalog);
    let bytes = std::fs::read(fixture.store.path()).checked("safe state bytes");
    assert!(
        !String::from_utf8(bytes)
            .checked("safe state JSON")
            .contains(SECRET)
    );
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.ai_credentials.all_secrets().contains(&SECRET))
    });
}

// The production Workspace constrains its modal even in a tall fixture. Scroll
// the actual form until the full target is inside its viewport before clicking.
fn reveal_inference_control(
    window: &mut gpui_kit::Window,
    id: &'static str,
    cx: &mut gpui_kit::App,
) {
    for _ in 0..100 {
        window.render_frame(cx);
        let control = window.find(id);
        let viewport = window.find("ai-profile-form-scroll").bounds();
        if control.visible()
            && control.bounds().top() >= viewport.top()
            && control.bounds().bottom() <= viewport.bottom()
        {
            return;
        }
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., -3.)),
            cx,
        );
    }
    panic!("inference control {id} did not enter the real form viewport");
}

#[gpui_kit::test]
async fn inference_numeric_actual_apply_and_persist_reject_inactive_known_secrets(
    cx: &mut TestAppContext,
) {
    use keelshell_ai::{AiError, ContextDraft, ProviderConfig, ProviderProtocol, RequestOptions};
    use keelshell_core::{
        AiApiStyle, AiMessagesInference, AiMessagesThinking, AiModelReasoning, AiModelSampling,
        AiReasoningCapability, AiReasoningSelection, AiSamplingValue,
    };
    // Cover both representations of each sampling control, explicit zero's
    // JSON spelling, and a legal Messages budget. Each owner is inactive.
    for (field, text, secret) in [
        ("ai-inference-temperature", "0.125", "125"),
        ("ai-inference-temperature", "0.125", "0.125"),
        ("ai-inference-temperature", "0", "0.0"),
        ("ai-inference-top-p", "0.125", "125"),
        ("ai-inference-top-p", "0.125", "0.125"),
        ("ai-inference-budget", "2048", "2048"),
    ] {
        let fixture = mount_sized(cx, Vec::new(), 1280., 2800.);
        let before = std::fs::read(fixture.store.path()).checked("original numeric state bytes");
        let mut active = fixture_profile();
        if field == "ai-inference-budget" {
            active.api_style = AiApiStyle::AnthropicMessages;
            active.max_output_tokens = Some(8192);
            active.reasoning_by_model.insert(
                active.model.clone(),
                AiModelReasoning {
                    capability: AiReasoningCapability::Messages {
                        efforts: vec![],
                        adaptive: false,
                        disabled: false,
                        manual_budget: true,
                    },
                    selection: AiReasoningSelection::Messages(AiMessagesInference {
                        effort: None,
                        thinking: AiMessagesThinking::LegacyBudget(1024),
                    }),
                },
            );
        } else {
            active.sampling_by_model.insert(
                active.model.clone(),
                AiModelSampling {
                    declared_supported: true,
                    temperature: None,
                    top_p: None,
                },
            );
        }
        let mut inactive = fixture_profile();
        inactive.name = "Inactive numeric credential owner".into();
        let mut credentials = EphemeralCredentials::new();
        credentials.insert(inactive.id, Zeroizing::new(secret.into()));
        let mut catalog = AiProfileCatalog {
            active_id: Some(active.id),
            profiles: vec![active, inactive],
        };
        assert!(catalog.validate().is_ok());
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.state.settings.ai_profiles = catalog.clone();
                view.ai_credentials = credentials.clone();
                view.open_ai_settings(window, cx);
            });
        })
        .checked("open production numeric settings subscription");
        cx.run_until_parked();
        let panel = fixture
            .workspace
            .read_with(cx, |view, _| view.ai_settings.clone())
            .unwrap_or_else(|| panic!("numeric settings panel"));
        let emitted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = emitted.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&panel, move |_, event, _| {
                if matches!(event, AiSettingsEvent::Apply { .. }) {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            })
        });
        cx.update_window(fixture.window, |_, window, cx| {
            reveal_inference_control(window, field, cx);
            window.click(field, cx);
            if field == "ai-inference-budget" {
                window.press(
                    if cfg!(target_os = "macos") {
                        "cmd-a"
                    } else {
                        "ctrl-a"
                    },
                    cx,
                );
                window.press("backspace", cx);
            }
            window.input(text, cx);
            assert_eq!(
                window.find(field).value(),
                Some(text),
                "actual edited input"
            );
            window.click("ai-settings-apply", cx);
        })
        .checked("edit native input through UI and click real Apply");
        cx.run_until_parked();
        assert_eq!(
            emitted.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "producer refuses {field}/{secret}"
        );
        fixture
            .workspace
            .read_with(cx, |view, _| assert!(!view.saving));
        assert_eq!(
            std::fs::read(fixture.store.path()).checked("producer guarded bytes"),
            before
        );

        // Also deliver a valid typed snapshot directly: production persistence
        // must reject it even if a future producer misses its own admission.
        let active = &mut catalog.profiles[0];
        if field == "ai-inference-budget" {
            active
                .reasoning_by_model
                .get_mut(&active.model)
                .unwrap_or_else(|| panic!("selected manual budget"))
                .selection = AiReasoningSelection::Messages(AiMessagesInference {
                effort: None,
                thinking: AiMessagesThinking::LegacyBudget(2048),
            });
        } else {
            let sampling = active
                .sampling_by_model
                .get_mut(&active.model)
                .unwrap_or_else(|| panic!("selected sampling"));
            let value = Some(AiSamplingValue::parse(text).checked("exact selected sampling"));
            if field == "ai-inference-temperature" {
                sampling.temperature = value;
            } else {
                sampling.top_p = value;
            }
        }
        assert!(
            catalog.validate().is_ok(),
            "legal candidate for consumer admission"
        );
        let protocol = if field == "ai-inference-budget" {
            ProviderProtocol::AnthropicMessages
        } else {
            ProviderProtocol::ChatCompletions
        };
        let active = &catalog.profiles[0];
        let provider = ProviderConfig::new_with_protocol(&active.endpoint, &active.model, protocol)
            .checked("numeric provider")
            .with_request_options(
                RequestOptions::default()
                    .with_context_secrets(&credentials.all_secrets())
                    .checked("inactive known secrets"),
            )
            .with_inference_options(
                crate::ai_request_options::inference_options(active)
                    .checked("production inference mapper"),
            )
            .checked("typed inference provider");
        assert!(matches!(
            ContextDraft::new("ordinary question").prepare(&provider, &[], 8192),
            Err(AiError::CredentialInContext)
        ));
        panel.update(cx, |_, cx| {
            cx.emit(AiSettingsEvent::Apply {
                catalog,
                credentials,
                revision: 0,
            })
        });
        cx.run_until_parked();
        fixture
            .workspace
            .read_with(cx, |view, _| assert!(!view.saving));
        assert_eq!(
            std::fs::read(fixture.store.path()).checked("consumer guarded bytes"),
            before
        );
        assert!(
            fixture
                .store
                .load()
                .checked("actual refused StateStore")
                .settings
                .ai_profiles
                .profiles
                .is_empty()
        );
    }
}

#[gpui_kit::test]
async fn inference_numeric_actual_apply_persists_zero_omission_and_combined_messages(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{
        AiApiStyle, AiMessagesEffort, AiMessagesInference, AiMessagesThinking, AiModelReasoning,
        AiReasoningCapability,
    };
    for kind in 0..3 {
        let fixture = mount_sized(cx, Vec::new(), 1280., 2800.);
        let mut active = fixture_profile();
        if kind == 2 {
            active.api_style = AiApiStyle::AnthropicMessages;
            active.max_output_tokens = Some(8192);
            active.reasoning_by_model.insert(
                active.model.clone(),
                AiModelReasoning {
                    capability: AiReasoningCapability::Messages {
                        efforts: vec![AiMessagesEffort::Medium],
                        adaptive: true,
                        disabled: true,
                        manual_budget: true,
                    },
                    selection: AiReasoningSelection::Messages(AiMessagesInference {
                        effort: Some(AiMessagesEffort::Medium),
                        thinking: AiMessagesThinking::LegacyBudget(2048),
                    }),
                },
            );
        }
        let mut inactive = fixture_profile();
        inactive.name = "Inactive safe credential owner".into();
        let mut credentials = EphemeralCredentials::new();
        credentials.insert(inactive.id, Zeroizing::new("unrelated-owned-secret".into()));
        let catalog = AiProfileCatalog {
            active_id: Some(active.id),
            profiles: vec![active, inactive],
        };
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.state.settings.ai_profiles = catalog.clone();
                view.ai_credentials = credentials;
                view.open_ai_settings(window, cx);
            })
        })
        .checked("open safe production settings");
        cx.run_until_parked();
        cx.update_window(fixture.window, |_, window, cx| {
            if kind == 0 {
                reveal_inference_control(window, "ai-sampling-support", cx);
                window.click("ai-sampling-support", cx);
                reveal_inference_control(window, "ai-inference-temperature", cx);
                window.click("ai-inference-temperature", cx);
                window.input("0", cx);
            }
            window.click("ai-settings-apply", cx);
        })
        .checked("real safe Apply");
        cx.run_until_parked();
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            !fixture.workspace.read(cx).saving
        })
        .await;
        let saved = fixture
            .store
            .load()
            .checked("load actual safe inference state");
        let active = saved
            .settings
            .ai_profiles
            .active()
            .unwrap_or_else(|| panic!("actual saved profile"));
        if kind == 0 {
            assert_eq!(
                active.sampling_by_model[&active.model]
                    .temperature
                    .map(keelshell_core::AiSamplingValue::millis),
                Some(0)
            );
        } else {
            assert_eq!(saved.settings.ai_profiles, catalog);
        }
        let options =
            crate::ai_request_options::inference_options(active).checked("safe persisted mapper");
        let protocol = if kind == 2 {
            keelshell_ai::ProviderProtocol::AnthropicMessages
        } else {
            keelshell_ai::ProviderProtocol::ChatCompletions
        };
        let provider = keelshell_ai::ProviderConfig::new_with_protocol(
            &active.endpoint,
            &active.model,
            protocol,
        )
        .checked("safe provider")
        .with_request_options(
            keelshell_ai::RequestOptions::default()
                .with_context_secrets(&["unrelated-owned-secret"])
                .checked("safe known secret"),
        )
        .with_inference_options(options)
        .checked("safe selected options");
        let draft = keelshell_ai::ContextDraft::new("ordinary question")
            .prepare(&provider, &[], 8192)
            .checked("safe complete reviewed payload");
        let body: serde_json::Value =
            serde_json::from_str(draft.preview_json()).checked("safe exact body");
        if kind == 0 {
            assert_eq!(body["temperature"], 0.0);
        }
        if kind == 1 {
            assert!(body.get("temperature").is_none());
        }
        if kind == 2 {
            assert_eq!(body["output_config"]["effort"], "medium");
            assert_eq!(body["thinking"]["budget_tokens"], 2048);
        }
    }
}
