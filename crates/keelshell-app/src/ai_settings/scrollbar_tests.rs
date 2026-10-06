//! Pointer-driven access to production settings controls; no pixel/native claim.
use super::{AiSettingsEvent, AiSettingsPanel};
use crate::{
    ai_settings::tests::{fixture_profile, mount_sized},
    design,
    i18n::{language, set_language},
};
use gpui_kit::{App, AppContext, Entity, TestAppContext, Window, point, px, test::TestWindowExt};
use keelshell_core::{
    AiBackend, AiLocalAgent, AiModelSampling, AiProxy, AiSamplingValue, Language, Theme,
};
use std::sync::{Arc, Mutex};

// Read the live thumb's approximate grab position, then use actual mouse down,
// move and up events. No test writes the handle offset or calls scroll_to_item.
fn drag_form_bar(
    window: &mut Window,
    panel: &Entity<AiSettingsPanel>,
    fraction: f32,
    cx: &mut App,
) {
    window.render_frame(cx);
    let viewport = window.find("ai-profile-form-scroll").bounds();
    let handle = &panel.read(cx).form_scroll;
    let distance = f32::from(handle.max_offset().y);
    assert!(distance > 0., "production form overflows");
    let height = f32::from(viewport.size.height);
    let thumb = (height * height / (height + distance)).max(48.);
    let travel = (height - 8. - thumb).max(0.);
    let current = (-f32::from(handle.offset().y) / distance).clamp(0., 1.);
    let x = viewport.right() - px(7.);
    let from = point(x, viewport.top() + px(14. + current * travel));
    let y = if fraction <= 0. {
        viewport.top() + px(2.)
    } else if fraction >= 1. {
        viewport.bottom() - px(2.)
    } else {
        viewport.top() + px(14. + fraction * travel)
    };
    window.drag(from, point(x, y), cx);
    window.render_frame(cx);
}

fn drag_to_input(
    window: &mut Window,
    panel: &Entity<AiSettingsPanel>,
    id: &'static str,
    cx: &mut App,
) {
    window.render_frame(cx);
    let viewport = window.find("ai-profile-form-scroll").bounds();
    let target = window.find(id).bounds();
    let handle = &panel.read(cx).form_scroll;
    let distance = f32::from(handle.max_offset().y);
    let desired = f32::from(target.top() - viewport.top() - handle.offset().y - px(40.));
    drag_form_bar(window, panel, (desired / distance).clamp(0., 1.), cx);
    let target = window.find(id);
    let viewport = window.find("ai-profile-form-scroll").bounds();
    assert!(target.visible(), "{id} becomes visible after thumb drag");
    assert!(target.bounds().top() >= viewport.top());
    assert!(target.bounds().bottom() <= viewport.bottom());
}

fn replace_input(window: &mut Window, id: &'static str, text: &str, cx: &mut App) {
    window.click(id, cx);
    window.press(
        if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        },
        cx,
    );
    window.press("backspace", cx);
    window.input(text, cx);
    assert_eq!(window.find(id).value(), Some(text));
}

#[gpui_kit::test]
fn settings_scrollbar_pointer_reaches_api_parameters_and_footer_keeps_drafts(
    cx: &mut TestAppContext,
) {
    for (locale, theme) in [
        (Language::ZhCn, Theme::System),
        (Language::En, Theme::Dark),
        (Language::ZhCn, Theme::Light),
    ] {
        let mut profile = fixture_profile();
        profile.model = "scroll-selected-model".into();
        profile.sampling_by_model.insert(
            profile.model.clone(),
            AiModelSampling {
                declared_supported: true,
                temperature: Some(
                    AiSamplingValue::parse("0.125")
                        .unwrap_or_else(|error| panic!("sampling fixture: {error}")),
                ),
                top_p: None,
            },
        );
        profile.proxy = AiProxy::Explicit {
            url: "http://proxy.example".into(),
            credentials: None,
        };
        let id = profile.id;
        let (handle, panel) = mount_sized(cx, profile, 900., 580.);
        let applied = Arc::new(Mutex::new(None));
        let saved = applied.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&panel, move |_, event, _| {
                if let AiSettingsEvent::Apply { catalog, .. } = event {
                    *saved
                        .lock()
                        .unwrap_or_else(|error| panic!("apply snapshot: {error}")) =
                        Some(catalog.clone());
                }
            })
        });
        cx.update_window(handle, |_, window, cx| {
            set_language(locale, cx);
            design::apply(theme, Some(window), cx);
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            window.render_frame(cx);
            let apply = window.find("ai-settings-apply").bounds();
            let theme_mode = cx.global::<gpui_kit::component::Theme>().mode;
            assert!(window.find("ai-profile-form-scrollbar").visible());
            assert_eq!(panel.read(cx).form_scroll.offset().y, px(0.));
            assert!(!window.find("ai-inference-temperature").visible());
            let persistent = panel.read(cx).form_scroll.clone();
            drag_form_bar(window, &panel, 1., cx);
            assert!(panel.read(cx).form_scroll.offset().y < px(0.));
            assert_eq!(
                panel.read(cx).form_scroll.offset().y,
                -panel.read(cx).form_scroll.max_offset().y
            );
            assert!(
                window.find("ai-proxy-url").visible(),
                "last API input reachable"
            );
            replace_input(window, "ai-proxy-url", "http://proxy.example:8080", cx);
            drag_to_input(window, &panel, "ai-inference-temperature", cx);
            replace_input(window, "ai-inference-temperature", "0.25", cx);
            let offset = panel.read(cx).form_scroll.offset();
            window.render_frame(cx);
            assert_eq!(
                panel.read(cx).form_scroll.offset(),
                offset,
                "rerender keeps scroll position"
            );
            assert_eq!(
                persistent.offset(),
                panel.read(cx).form_scroll.offset(),
                "same persistent handle survives edits"
            );
            assert_eq!(language(cx), locale);
            assert_eq!(cx.global::<gpui_kit::component::Theme>().mode, theme_mode);
            assert_eq!(panel.read(cx).selected, Some(id));
            assert_eq!(
                panel.read(cx).model.read(cx).value(),
                "scroll-selected-model"
            );
            assert_eq!(panel.read(cx).inference_inputs[1].read(cx).value(), "0.25");
            let now = window.find("ai-settings-apply");
            assert!(now.visible());
            assert_eq!(
                now.bounds(),
                apply,
                "footer stays fixed during pointer scroll"
            );
            window.click("ai-settings-apply", cx);
        })
        .unwrap_or_else(|error| panic!("production API scrollbar: {error}"));
        cx.run_until_parked();
        let catalog = applied
            .lock()
            .unwrap_or_else(|error| panic!("applied catalog: {error}"))
            .clone()
            .unwrap_or_else(|| panic!("actual fixed Apply emitted"));
        let profile = catalog
            .active()
            .unwrap_or_else(|| panic!("selected saved profile"));
        assert_eq!(profile.id, id);
        assert_eq!(profile.model, "scroll-selected-model");
        assert_eq!(
            profile.sampling_by_model[&profile.model]
                .temperature
                .map(AiSamplingValue::millis),
            Some(250)
        );
        assert_eq!(
            profile.proxy,
            AiProxy::Explicit {
                url: "http://proxy.example:8080".into(),
                credentials: None
            }
        );
    }
}

#[gpui_kit::test]
fn settings_scrollbar_pointer_and_wheel_share_cli_form_and_fixed_apply(cx: &mut TestAppContext) {
    let mut profile = fixture_profile();
    profile.backend = AiBackend::LocalAgent {
        agent: AiLocalAgent::Codex,
        executable: std::env::temp_dir()
            .join("keelshell-owned-unlaunched-agent")
            .to_string_lossy()
            .into_owned(),
        limits: Default::default(),
    };
    let id = profile.id;
    let (handle, panel) = mount_sized(cx, profile, 900., 580.);
    let applied = Arc::new(Mutex::new(None));
    let saved = applied.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if let AiSettingsEvent::Apply { catalog, .. } = event {
                *saved
                    .lock()
                    .unwrap_or_else(|error| panic!("CLI snapshot: {error}")) =
                    Some(catalog.clone());
            }
        })
    });
    cx.update_window(handle, |_, window, cx| {
        set_language(Language::En, cx);
        design::apply(Theme::Dark, Some(window), cx);
        panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        window.render_frame(cx);
        let apply = window.find("ai-settings-apply").bounds();
        let theme_mode = cx.global::<gpui_kit::component::Theme>().mode;
        let persistent = panel.read(cx).form_scroll.clone();
        drag_form_bar(window, &panel, 1., cx);
        let bottom = panel.read(cx).form_scroll.offset().y;
        assert_eq!(bottom, -panel.read(cx).form_scroll.max_offset().y);
        assert!(window.find("ai-local-output").visible());
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., 3.)),
            cx,
        );
        assert!(
            panel.read(cx).form_scroll.offset().y > bottom,
            "wheel uses same container"
        );
        drag_form_bar(window, &panel, 1., cx);
        replace_input(window, "ai-local-output", "4096", cx);
        window.render_frame(cx);
        assert_eq!(persistent.offset(), panel.read(cx).form_scroll.offset());
        assert_eq!(panel.read(cx).selected, Some(id));
        assert_eq!(language(cx), Language::En);
        assert_eq!(cx.global::<gpui_kit::component::Theme>().mode, theme_mode);
        assert_eq!(panel.read(cx).model.read(cx).value(), "fixture-model");
        assert!(panel.read(cx).operation.is_none(), "no CLI request starts");
        assert_eq!(window.find("ai-settings-apply").bounds(), apply);
        window.click("ai-settings-apply", cx);
    })
    .unwrap_or_else(|error| panic!("production CLI scrollbar: {error}"));
    cx.run_until_parked();
    let catalog = applied
        .lock()
        .unwrap_or_else(|error| panic!("CLI applied: {error}"))
        .clone()
        .unwrap_or_else(|| panic!("CLI actual Apply emitted"));
    let profile = catalog
        .active()
        .unwrap_or_else(|| panic!("saved CLI profile"));
    let AiBackend::LocalAgent { limits, .. } = &profile.backend else {
        panic!("CLI preserved")
    };
    assert_eq!(limits.output_kib(), 4096);
    assert_eq!(profile.id, id);
}

#[gpui_kit::test]
fn settings_validation_feedback_keeps_minimum_window_actions_reachable(cx: &mut TestAppContext) {
    for locale in [Language::En, Language::ZhCn] {
        for theme in [Theme::System, Theme::Dark, Theme::Light] {
            let mut profile = fixture_profile();
            profile.sampling_by_model.insert(
                profile.model.clone(),
                AiModelSampling {
                    declared_supported: true,
                    ..Default::default()
                },
            );
            let (handle, panel) = mount_sized(cx, profile, 900., 580.);
            let applied = Arc::new(Mutex::new(Vec::new()));
            let saved = applied.clone();
            let _subscription = cx.update(|cx| {
                cx.subscribe(&panel, move |_, event, _| {
                    if let AiSettingsEvent::Apply { catalog, .. } = event {
                        saved
                            .lock()
                            .unwrap_or_else(|error| panic!("feedback Apply capture: {error}"))
                            .push(catalog.clone());
                    }
                })
            });
            cx.update_window(handle, |_, window, cx| {
                set_language(locale, cx);
                design::apply(theme, Some(window), cx);
                panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
                drag_to_input(window, &panel, "ai-inference-temperature", cx);
                replace_input(window, "ai-inference-temperature", "invalid", cx);
                window.click("ai-settings-apply", cx);
                window.render_frame(cx);
                assert_eq!(
                    panel.read(cx).status.render(cx),
                    AiSettingsPanel::inference_error().render(cx),
                    "full validation feedback is retained"
                );
                for id in ["ai-settings-cancel", "ai-settings-apply"] {
                    let button = window.find(id);
                    assert!(button.visible(), "{locale:?}/{theme:?}: {id} visible");
                    let bounds = button.bounds();
                    assert!(bounds.left() >= px(0.) && bounds.right() <= px(900.));
                    assert!(bounds.top() >= px(0.) && bounds.bottom() <= px(580.));
                }
            })
            .unwrap_or_else(|error| panic!("invalid minimum-window feedback: {error}"));
            cx.run_until_parked();
            assert!(
                applied
                    .lock()
                    .unwrap_or_else(|error| panic!("blocked feedback events: {error}"))
                    .is_empty(),
                "invalid draft never emits Apply"
            );
            cx.update_window(handle, |_, window, cx| {
                drag_to_input(window, &panel, "ai-inference-temperature", cx);
                replace_input(window, "ai-inference-temperature", "0", cx);
                window.click("ai-settings-apply", cx);
            })
            .unwrap_or_else(|error| panic!("corrected minimum-window draft: {error}"));
            cx.run_until_parked();
            let applied = applied
                .lock()
                .unwrap_or_else(|error| panic!("corrected feedback events: {error}"));
            assert_eq!(applied.len(), 1, "corrected draft emits one actual Apply");
            let profile = applied[0]
                .active()
                .unwrap_or_else(|| panic!("corrected selected profile"));
            assert_eq!(
                profile.sampling_by_model[&profile.model]
                    .temperature
                    .map(AiSamplingValue::millis),
                Some(0),
                "explicit zero is preserved"
            );
        }
    }
}
