#![allow(clippy::expect_used)]

use gpui_kit::{AppContext, TestAppContext};
use keelshell_ai::{AiError, RequestCancellation};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiLocalAgent, AiPreset, Language, NamedAiProfile,
};
use zeroize::Zeroizing;

fn profile(agent: AiLocalAgent) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::Custom);
    profile.name = "Isolated local".into();
    profile.model = "fixture-model".into();
    profile.backend = AiBackend::LocalAgent {
        working_directory: Default::default(),
        agent,
        executable: std::env::temp_dir()
            .join("unused-cli")
            .to_string_lossy()
            .into(),
        limits: Default::default(),
    };
    match agent {
        AiLocalAgent::Codex => {
            profile.api_style = AiApiStyle::Responses;
            profile.endpoint = "https://provider.example/v1".into();
            profile.authentication = AiAuthentication::Bearer { credential: None };
        }
        AiLocalAgent::ClaudeCode => {
            profile.api_style = AiApiStyle::AnthropicMessages;
            profile.endpoint = "https://provider.example".into();
            profile.authentication = AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            };
        }
    }
    profile
}

#[gpui_kit::test]
fn explicit_import_is_hidden_bound_and_revokes_on_edit_or_failed_reread(cx: &mut TestAppContext) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let (window, panel) = crate::ai_settings::tests::mount(cx, profile(agent));
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_local_environment(true, window, cx);
                panel.local_environment.update(cx, |input, cx| {
                    input.set_value("KEELSHELL_TEST_KEY", window, cx)
                });
                panel.sync_editor(cx);
                let id = panel.selected.expect("profile");
                let cancellation = RequestCancellation::new();
                panel.cancellation = Some(cancellation.clone());
                panel.read_local_environment_with(
                    |name| {
                        assert_eq!(name, "KEELSHELL_TEST_KEY");
                        Ok(Zeroizing::new("opaque-loaded-key".into()))
                    },
                    window,
                    cx,
                );
                assert!(cancellation.is_cancelled());
                let profile = panel.profile().expect("metadata");
                assert_eq!(
                    panel
                        .credentials
                        .local_environment_key(profile)
                        .expect("bound")
                        .as_str(),
                    "opaque-loaded-key"
                );
                panel
                    .model
                    .update(cx, |input, cx| input.set_value("another-model", window, cx));
                panel.sync_editor(cx);
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().expect("edited model"))
                        .is_some()
                );
                let profile = panel.profile().expect("metadata after edit");
                let metadata = serde_json::to_string(&panel.catalog).expect("JSON");
                assert!(metadata.contains("KEELSHELL_TEST_KEY"));
                assert!(!metadata.contains("opaque-loaded-key"));
                let config =
                    crate::ai_settings::local_agent_config(profile, false).expect("config");
                let review = config
                    .prepare(
                        keelshell_ai::ContextDraft::new("Explain a remote error"),
                        &panel.credentials.all_secrets(),
                        8192,
                    )
                    .expect("review");
                assert!(review.preview_json().contains("KEELSHELL_TEST_KEY"));
                assert!(!review.preview_json().contains("opaque-loaded-key"));
                assert!(!review.preview_stdin().contains("KEELSHELL_TEST_KEY"));
                let mut wrong = profile.clone();
                wrong.endpoint = "https://other.example/v1".into();
                assert!(panel.credentials.local_environment_key(&wrong).is_none());
                panel.read_local_environment_with(
                    |_| Err(AiError::InvalidRequestOptions),
                    window,
                    cx,
                );
                assert!(!panel.credentials.contains_key(&id));
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().expect("profile"))
                        .is_none()
                );
                assert!(
                    panel
                        .credentials
                        .all_secrets()
                        .contains(&"opaque-loaded-key")
                );
                panel.read_local_environment_with(
                    |_| Ok(Zeroizing::new("second-loaded-key".into())),
                    window,
                    cx,
                );
                panel
                    .local_environment
                    .update(cx, |input, cx| input.set_value("CHANGED_KEY", window, cx));
                panel.sync_editor(cx);
                assert!(!panel.credentials.contains_key(&id));
                panel.clear_pending_key(window, cx);
                cx.notify();
            })
        })
        .expect("window");
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn invalid_reference_drafts_survive_selection_locale_and_block_apply_without_lookup(
    cx: &mut TestAppContext,
) {
    let (window, panel) = crate::ai_settings::tests::mount(cx, profile(AiLocalAgent::Codex));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let first = panel.selected.expect("first");
            panel.set_local_environment(true, window, cx);
            panel
                .local_environment
                .update(cx, |input, cx| input.set_value("KEY=value", window, cx));
            panel.sync_editor(cx);
            panel.read_local_environment_with(
                |_| panic!("invalid name must not be read"),
                window,
                cx,
            );
            let mut second = profile(AiLocalAgent::ClaudeCode);
            second.name = "Second local".into();
            let second_id = second.id;
            panel.catalog.profiles.push(second);
            panel.select(second_id, window, cx);
            panel.select(first, window, cx);
            assert_eq!(panel.local_environment.read(cx).value(), "KEY=value");
            crate::i18n::set_language(Language::En, cx);
            panel.refresh_locale(window, cx);
            assert_eq!(panel.local_environment.read(cx).value(), "KEY=value");
            panel.apply(cx);
            assert!(!panel.saving);
            panel.set_local_environment(false, window, cx);
            panel.set_local_environment(true, window, cx);
            assert_eq!(panel.local_environment.read(cx).value(), "KEY=value");
        })
    })
    .expect("window");
}

#[gpui_kit::test]
fn newly_observed_secret_blocks_all_metadata_and_is_retained_without_authorizing_delivery(
    cx: &mut TestAppContext,
) {
    let (window, panel) = crate::ai_settings::tests::mount(cx, profile(AiLocalAgent::Codex));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_local_environment(true, window, cx);
            panel
                .local_environment
                .update(cx, |input, cx| input.set_value("KEY_VARIABLE", window, cx));
            panel.sync_editor(cx);
            let mut inactive = profile(AiLocalAgent::ClaudeCode);
            inactive.name = "hidden-secret-literal".into();
            panel.catalog.profiles.push(inactive);
            panel.read_local_environment_with(
                |_| Ok(Zeroizing::new("hidden-secret-literal".into())),
                window,
                cx,
            );
            assert!(
                panel
                    .credentials
                    .get(&panel.selected.expect("id"))
                    .is_none()
            );
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"hidden-secret-literal")
            );
            panel.apply(cx);
            assert!(!panel.saving);
            let first = panel.selected.expect("first");
            panel.set_backend(None, window, cx);
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"hidden-secret-literal")
            );
            panel.remove(window, cx);
            assert!(
                !panel
                    .credentials
                    .all_secrets()
                    .contains(&"hidden-secret-literal")
            );
            assert!(
                panel
                    .catalog
                    .profiles
                    .iter()
                    .all(|profile| profile.id != first)
            );
        })
    })
    .expect("window");
}

fn reveal(
    window: &mut gpui_kit::Window,
    panel: &gpui_kit::Entity<crate::ai_settings::AiSettingsPanel>,
    id: &'static str,
    cx: &mut gpui_kit::App,
) {
    use gpui_kit::{point, px, test::TestWindowExt};
    window.render_frame(cx);
    let viewport = window.find("ai-profile-form-scroll").bounds();
    let target = window.find(id).bounds();
    let handle = &panel.read(cx).form_scroll;
    let distance = f32::from(handle.max_offset().y);
    let height = f32::from(viewport.size.height);
    let thumb = (height * height / (height + distance)).max(48.);
    let travel = (height - 8. - thumb).max(0.);
    let current = (-f32::from(handle.offset().y) / distance).clamp(0., 1.);
    let fraction = (f32::from(target.top() - viewport.top() - handle.offset().y - px(40.))
        / distance)
        .clamp(0., 1.);
    let x = viewport.right() - px(7.);
    window.drag(
        point(x, viewport.top() + px(14. + current * travel)),
        point(x, viewport.top() + px(14. + fraction * travel)),
        cx,
    );
    window.render_frame(cx);
    let actual = window.find(id);
    assert!(actual.visible(), "{id} reached by actual thumb drag");
    assert!(actual.bounds().top() >= viewport.top());
    assert!(actual.bounds().bottom() <= viewport.bottom());
}

#[gpui_kit::test]
fn local_environment_pointer_editor_is_reachable_in_small_language_theme_matrix(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestWindowExt;
    use keelshell_core::Theme;
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                let (window, panel) =
                    crate::ai_settings::tests::mount_sized(cx, profile(agent), 900., 580.);
                cx.update(|cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply(theme, None, cx);
                });
                cx.update_window(window, |_, window, cx| {
                    reveal(window, &panel, "ai-local-key-environment", cx);
                    window.click("ai-local-key-environment", cx);
                })
                .expect("source click");
                cx.run_until_parked();
                cx.update_window(window, |_, window, cx| {
                    reveal(window, &panel, "ai-local-key-environment-name", cx);
                    window.click("ai-local-key-environment-name", cx);
                    window.input("KEY=value", cx);
                })
                .expect("name input");
                cx.run_until_parked();
                panel.read_with(cx, |panel, cx| {
                    assert_eq!(panel.local_environment.read(cx).value(), "KEY=value");
                });
                cx.update_window(window, |_, window, cx| {
                    reveal(window, &panel, "ai-local-key-environment-read", cx);
                    window.click("ai-local-key-environment-read", cx);
                })
                .expect("read click");
                cx.run_until_parked();
                panel.read_with(cx, |panel, cx| {
                    assert!(
                        !panel
                            .credentials
                            .contains_key(&panel.selected.expect("selected"))
                    );
                    assert!(!panel.status.render(cx).is_empty());
                });
                cx.update_window(window, |_, window, cx| {
                    window.render_frame(cx);
                    let apply = window.find("ai-settings-apply");
                    assert!(apply.visible());
                    assert!(apply.bounds().bottom() <= gpui_kit::px(580.));
                    window.click("ai-settings-apply", cx);
                })
                .expect("apply click");
                cx.run_until_parked();
                panel.read_with(cx, |panel, _| {
                    assert!(!panel.saving);
                });
            }
        }
    }
}

#[gpui_kit::test]
fn queued_input_changes_cannot_clear_new_import_and_value_limits_fail_closed(
    cx: &mut TestAppContext,
) {
    let (window, panel) = crate::ai_settings::tests::mount(cx, profile(AiLocalAgent::Codex));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_local_environment(true, window, cx);
            panel
                .local_environment
                .update(cx, |input, cx| input.set_value("KEY_VARIABLE", window, cx));
            panel.sync_editor(cx);
            assert!(panel.clear_key_pending);
            panel.read_local_environment_with(
                |_| Ok(Zeroizing::new("new-loaded-value".into())),
                window,
                cx,
            );
            assert!(!panel.clear_key_pending);
        })
    })
    .expect("import window");
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .credentials
                .local_environment_key(panel.profile().expect("profile"))
                .expect("survives queued changes")
                .as_str(),
            "new-loaded-value"
        );
        assert_eq!(panel.key.read(cx).value(), "new-loaded-value");
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            for value in [String::new(), "contains\ncontrol".into(), "x".repeat(8193)] {
                panel.read_local_environment_with(
                    |_| Ok(Zeroizing::new(value.clone())),
                    window,
                    cx,
                );
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().expect("profile"))
                        .is_none()
                );
            }
            for index in 0..15 {
                panel.read_local_environment_with(
                    |_| Ok(Zeroizing::new(format!("bounded-import-value-{index}"))),
                    window,
                    cx,
                );
                assert!(
                    panel
                        .credentials
                        .local_environment_key(panel.profile().expect("profile"))
                        .is_some()
                );
            }
            panel.read_local_environment_with(
                |_| panic!("capacity must reject before another lookup"),
                window,
                cx,
            );
            assert!(
                panel
                    .credentials
                    .local_environment_key(panel.profile().expect("profile"))
                    .is_none()
            );
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"new-loaded-value")
            );
        })
    })
    .expect("limits window");
}
