use gpui_kit::{AppContext, TestAppContext, test::TestWindowExt};
use keelshell_ai::RequestCancellation;
use keelshell_core::{AiApiStyle, AiLocalAgent, AiModelSampling, AiSamplingValue, Language};

use super::AiReasoningSelection;
use crate::{
    ai_settings::{
        AiSettingsEvent, OperationKind,
        tests::{fixture_profile, mount_sized},
    },
    i18n::set_language,
};

#[gpui_kit::test]
fn inference_real_controls_declare_support_and_keep_explicit_zero(cx: &mut TestAppContext) {
    let (window, panel) = mount_sized(cx, fixture_profile(), 1200., 2800.);
    let applied = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result = applied.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if let AiSettingsEvent::Apply { catalog, .. } = event {
                *result
                    .lock()
                    .unwrap_or_else(|error| panic!("apply capture mutex: {error}")) =
                    Some(catalog.clone());
            }
        })
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.click("ai-sampling-support", cx);
        panel.update(cx, |p, cx| {
            p.inference_inputs[1].update(cx, |input, cx| input.set_value("0", window, cx));
            p.sync_editor(cx);
            let sampling = p
                .profile()
                .and_then(|p| p.sampling_by_model.get(&p.model))
                .unwrap_or_else(|| panic!("sampling"));
            assert!(sampling.declared_supported);
            assert_eq!(sampling.temperature.map(AiSamplingValue::millis), Some(0));
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.click(("ai-reasoning-effort", 0_usize), cx);
        panel.update(cx, |p, cx| {
            let profile = p.profile().unwrap_or_else(|| panic!("selected profile"));
            assert_eq!(
                profile.reasoning_by_model[&profile.model].selection,
                AiReasoningSelection::Effort("none".into())
            );
            assert!(profile.validate_current_transport().is_ok());
            p.apply(cx);
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
    cx.run_until_parked();
    let saved = applied
        .lock()
        .unwrap_or_else(|error| panic!("apply capture mutex: {error}"))
        .clone()
        .unwrap_or_else(|| panic!("Apply catalog event"));
    let profile = saved
        .active()
        .unwrap_or_else(|| panic!("saved active profile"));
    assert_eq!(
        profile.sampling_by_model[&profile.model]
            .temperature
            .map(AiSamplingValue::millis),
        Some(0)
    );
    assert_eq!(
        profile.reasoning_by_model[&profile.model].selection,
        AiReasoningSelection::Effort("none".into())
    );
}

#[gpui_kit::test]
fn inference_invalid_raw_draft_survives_models_profiles_locale_and_refuses_apply(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount_sized(cx, fixture_profile(), 1200., 2800.);
    let emitted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = emitted.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if matches!(event, AiSettingsEvent::Apply { .. }) {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
    });
    let cancellation = RequestCancellation::new();
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |p, cx| {
            p.cancellation = Some(cancellation.clone());
            p.operation = Some(OperationKind::Models);
            p.toggle_sampling_support(window, cx);
            p.inference_inputs[1].update(cx, |input, cx| input.set_value("NaN", window, cx));
            p.sync_editor(cx);
            assert!(cancellation.is_cancelled());
            assert!(!p.inference_draft_valid(
                p.selected.unwrap_or_else(|| panic!("selected profile id"))
            ));
            let first = p.selected.unwrap_or_else(|| panic!("selected profile id"));
            let second = fixture_profile();
            let second_id = second.id;
            p.catalog.profiles.push(second);
            p.select(second_id, window, cx);
            p.select(first, window, cx);
            assert_eq!(p.inference_inputs[1].read(cx).value(), "NaN");
            set_language(Language::En, cx);
            p.refresh_locale(window, cx);
            assert_eq!(p.inference_inputs[1].read(cx).value(), "NaN");
            p.choose_reasoning(AiReasoningSelection::ProviderDefault, window, cx);
            assert_eq!(p.inference_inputs[1].read(cx).value(), "NaN");
            p.apply(cx);
            assert!(!p.saving);
            p.start_operation(OperationKind::Test, cx);
            assert!(p.operation.is_none());
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
    cx.run_until_parked();
    assert_eq!(emitted.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[gpui_kit::test]
fn inference_model_switch_and_destination_changes_keep_declarations_scoped(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount_sized(cx, fixture_profile(), 1000., 1000.);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |p, cx| {
            p.toggle_sampling_support(window, cx);
            p.inference_inputs[1].update(cx, |input, cx| input.set_value("0.125", window, cx));
            p.sync_editor(cx);
            p.model
                .update(cx, |input, cx| input.set_value("second-model", window, cx));
            p.sync_editor(cx);
            p.load_inference_editor(window, cx);
            assert!(p.inference_inputs[1].read(cx).value().is_empty());
            p.model
                .update(cx, |input, cx| input.set_value("fixture-model", window, cx));
            p.sync_editor(cx);
            p.load_inference_editor(window, cx);
            assert_eq!(p.inference_inputs[1].read(cx).value(), "0.125");
            p.endpoint.update(cx, |input, cx| {
                input.set_value("https://second.example/v1/chat/completions", window, cx)
            });
            p.sync_editor(cx);
            p.load_inference_editor(window, cx);
            assert!(
                p.profile()
                    .unwrap_or_else(|| panic!("selected profile"))
                    .sampling_by_model
                    .is_empty()
            );
            assert!(p.inference_inputs[1].read(cx).value().is_empty());
            p.choose_reasoning(AiReasoningSelection::Effort("low".into()), window, cx);
            p.set_api_style(AiApiStyle::Responses, window, cx);
            assert!(
                p.profile()
                    .unwrap_or_else(|| panic!("selected profile"))
                    .reasoning_by_model
                    .is_empty()
            );
            p.toggle_sampling_support(window, cx);
            p.set_backend(Some(AiLocalAgent::Codex), window, cx);
            assert!(
                p.profile()
                    .unwrap_or_else(|| panic!("selected profile"))
                    .sampling_by_model
                    .is_empty()
            );
            assert!(p.inference_drafts.is_empty());
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
}

#[gpui_kit::test]
fn inactive_sampling_metadata_known_secret_cannot_be_saved(cx: &mut TestAppContext) {
    let (window, panel) = mount_sized(cx, fixture_profile(), 1000., 1000.);
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received = count.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if matches!(event, AiSettingsEvent::Apply { .. }) {
                received.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
    });
    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |p, cx| {
            let mut inactive = fixture_profile();
            inactive.name = "inactive fixture".into();
            inactive
                .sampling_by_model
                .insert("known-inactive-secret".into(), AiModelSampling::default());
            p.credentials.insert(
                inactive.id,
                zeroize::Zeroizing::new("known-inactive-secret".into()),
            );
            p.catalog.profiles.push(inactive);
            p.apply(cx);
            assert!(!p.saving);
            assert!(!p.status.render(cx).contains("known-inactive-secret"));
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
    cx.run_until_parked();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[gpui_kit::test]
fn inference_minimum_bilingual_form_scroll_keeps_fixed_apply_controls(cx: &mut TestAppContext) {
    for language in [Language::ZhCn, Language::En] {
        let (window, _) = mount_sized(cx, fixture_profile(), 720., 580.);
        cx.update(|cx| set_language(language, cx));
        cx.run_until_parked();
        cx.update_window(window, |_, window, _| {
            let viewport = window.find("ai-profile-form-scroll").bounds();
            let content = window.find("ai-profile-form-content").bounds();
            assert!(content.size.height > viewport.size.height);
            let apply = window.find("ai-settings-apply").bounds();
            assert!(apply.origin.y >= viewport.origin.y + viewport.size.height);
            assert!(apply.origin.y + apply.size.height <= gpui_kit::px(580.));
        })
        .unwrap_or_else(|error| panic!("update inference window: {error}"));
    }
}

#[gpui_kit::test]
fn messages_independent_controls_persist_reload_and_clear_on_destination_change(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{
        AiMessagesEffort, AiMessagesInference, AiMessagesThinking, NamedAiProfile,
    };
    let mut profile = fixture_profile();
    profile.api_style = AiApiStyle::AnthropicMessages;
    let (window, panel) = mount_sized(cx, profile, 1200., 3200.);
    let applied = std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured = applied.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if let AiSettingsEvent::Apply { catalog, .. } = event {
                *captured
                    .lock()
                    .unwrap_or_else(|e| panic!("Apply mutex: {e}")) = Some(catalog.clone());
            }
        })
    });
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.click(("ai-reasoning-effort", 1_usize), cx)
    })
    .unwrap_or_else(|e| panic!("effort control: {e}"));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.click("ai-thinking-adaptive", cx);
        panel.update(cx, |p, cx| {
            let profile = p.profile().unwrap_or_else(|| panic!("selected profile"));
            assert_eq!(
                AiMessagesInference::from_selection(
                    &profile.reasoning_by_model[&profile.model].selection
                )
                .unwrap_or_else(|e| panic!("composed selection: {e}")),
                AiMessagesInference {
                    effort: Some(AiMessagesEffort::Medium),
                    thinking: AiMessagesThinking::Adaptive,
                }
            );
            p.apply(cx);
        });
    })
    .unwrap_or_else(|e| panic!("thinking control: {e}"));
    cx.run_until_parked();
    let saved = applied
        .lock()
        .unwrap_or_else(|e| panic!("Apply mutex: {e}"))
        .clone()
        .unwrap_or_else(|| panic!("Apply event"));
    let profile = saved.active().unwrap_or_else(|| panic!("saved profile"));
    let stored = serde_json::to_value(profile).unwrap_or_else(|e| panic!("serialize profile: {e}"));
    let loaded: NamedAiProfile =
        serde_json::from_value(stored).unwrap_or_else(|e| panic!("reload profile: {e}"));
    let (window, panel) = mount_sized(cx, loaded, 1200., 3200.);
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| w.click("ai-reasoning-default", cx))
        .unwrap_or_else(|e| panic!("default effort: {e}"));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        panel.update(cx, |p, _| {
            let profile = p.profile().unwrap_or_else(|| panic!("reloaded profile"));
            let options = AiMessagesInference::from_selection(
                &profile.reasoning_by_model[&profile.model].selection,
            )
            .unwrap_or_else(|e| panic!("independent omission: {e}"));
            assert_eq!(options.effort, None);
            assert_eq!(options.thinking, AiMessagesThinking::Adaptive);
        });
        w.click("ai-thinking-default", cx);
    })
    .unwrap_or_else(|e| panic!("default thinking: {e}"));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        panel.update(cx, |p, _| {
            let profile = p.profile().unwrap_or_else(|| panic!("default profile"));
            assert_eq!(
                profile.reasoning_by_model[&profile.model].selection,
                AiReasoningSelection::ProviderDefault
            );
        });
        w.click("ai-thinking-budget", cx);
    })
    .unwrap_or_else(|e| panic!("legacy mode: {e}"));
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        panel.update(cx, |p, cx| {
            p.inference_inputs[0].update(cx, |input, cx| input.set_value("NaN", w, cx));
            p.sync_editor(cx);
            p.choose_reasoning(AiReasoningSelection::Effort("low".into()), w, cx);
            assert_eq!(p.inference_inputs[0].read(cx).value(), "NaN");
            let id = p.selected.unwrap_or_else(|| panic!("selected id"));
            assert!(!p.inference_draft_valid(id));
            p.apply(cx);
            assert!(!p.saving);
            p.model
                .update(cx, |input, cx| input.set_value("another-model", w, cx));
            p.sync_editor(cx);
            p.load_inference_editor(w, cx);
            assert!(p.inference_inputs[0].read(cx).value().is_empty());
            p.model
                .update(cx, |input, cx| input.set_value("fixture-model", w, cx));
            p.sync_editor(cx);
            p.load_inference_editor(w, cx);
            assert_eq!(p.inference_inputs[0].read(cx).value(), "NaN");
            p.endpoint.update(cx, |input, cx| {
                input.set_value("https://changed.example/v1/messages", w, cx)
            });
            p.sync_editor(cx);
            p.load_inference_editor(w, cx);
            assert!(
                p.profile()
                    .unwrap_or_else(|| panic!("changed profile"))
                    .reasoning_by_model
                    .is_empty()
            );
            assert!(p.inference_drafts.is_empty());
            assert!(p.inference_inputs[0].read(cx).value().is_empty());
        });
    })
    .unwrap_or_else(|e| panic!("composed draft boundary: {e}"));
}

#[gpui_kit::test]
fn inactive_known_secret_in_composed_thinking_metadata_cannot_be_saved(cx: &mut TestAppContext) {
    use keelshell_core::{AiMessagesEffort, AiMessagesInference, AiMessagesThinking};
    let mut profile = fixture_profile();
    profile.api_style = AiApiStyle::AnthropicMessages;
    profile.reasoning_by_model.insert(
        profile.model.clone(),
        keelshell_core::AiModelReasoning {
            capability: keelshell_core::AiReasoningCapability::Messages {
                efforts: vec![AiMessagesEffort::Medium],
                adaptive: true,
                disabled: false,
                manual_budget: false,
            },
            selection: AiReasoningSelection::Messages(AiMessagesInference {
                effort: Some(AiMessagesEffort::Medium),
                thinking: AiMessagesThinking::Adaptive,
            }),
        },
    );
    let (window, panel) = mount_sized(cx, profile, 1000., 1000.);
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received = count.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if matches!(event, AiSettingsEvent::Apply { .. }) {
                received.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
    });
    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |p, cx| {
            let mut inactive = fixture_profile();
            inactive.name = "Inactive mode credential".into();
            p.credentials
                .insert(inactive.id, zeroize::Zeroizing::new("adaptive".into()));
            p.catalog.profiles.push(inactive);
            assert!(p.catalog.validate().is_ok());
            assert!(
                p.profile()
                    .is_some_and(|profile| profile.validate_current_transport().is_ok())
            );
            p.apply(cx);
            assert!(!p.saving);
            assert!(!p.status.render(cx).contains("adaptive"));
        })
    })
    .unwrap_or_else(|e| panic!("mode metadata boundary: {e}"));
    cx.run_until_parked();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
}
