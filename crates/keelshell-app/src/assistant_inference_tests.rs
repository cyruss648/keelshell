use super::AssistantPanel;
use crate::ai_settings::EphemeralCredentials;
use gpui_kit::{AppContext, TestAppContext, WindowOptions};
use keelshell_ai::RequestCancellation;
use keelshell_core::{AiApiStyle, AiProfileCatalog, NamedAiProfile};
use keelshell_core::{
    AiAuthentication, AiModelReasoning, AiPreset, AiReasoningCapability, AiReasoningSelection,
};
use std::sync::Arc;
use zeroize::Zeroizing;

#[gpui_kit::test]
fn long_protocol_reviews_keep_confirmation_visible_and_pointer_scroll_reaches_end(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{Bounds, WindowBounds, point, px, size, test::TestWindowExt};
    use keelshell_core::{Language, Theme};
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    for style in [
        AiApiStyle::ChatCompletions,
        AiApiStyle::Responses,
        AiApiStyle::AnthropicMessages,
    ] {
        let profile = fixture(style);
        let catalog = AiProfileCatalog {
            active_id: Some(profile.id),
            profiles: vec![profile],
        };
        let (handle, panel) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(380.), px(580.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        AssistantPanel::new(
                            &catalog,
                            &EphemeralCredentials::new(),
                            runtime.clone(),
                            window,
                            cx,
                        )
                    })
                },
            )
            .unwrap_or_else(|error| panic!("review window: {error}"))
        });
        cx.update_window(handle, |_, window, cx| {
            for locale in [Language::ZhCn, Language::En] {
                for theme in [Theme::System, Theme::Light, Theme::Dark] {
                    crate::i18n::set_language(locale, cx);
                    crate::design::apply(theme, Some(window), cx);
                    panel.update(cx, |panel, cx| {
                        panel.refresh_locale(window, cx);
                        panel.prompt.update(cx, |input, cx| {
                            input.set_value("Review this selected output", window, cx)
                        });
                        panel.set_context(
                            "isolated selected output\\n".repeat(300),
                            "fixture host".into(),
                            "fixture session".into(),
                            cx,
                        );
                        panel.prepare(cx);
                    });
                    window.render_frame(cx);
                    let before = window.find("send-approved-request").bounds();
                    let viewport = window.find("assistant-scroll").bounds();
                    let footer = window.find("assistant-confirmation-footer").bounds();
                    assert!(window.find("send-approved-request").visible());
                    assert!(before.top() >= footer.top());
                    assert!(before.bottom() <= px(580.));
                    assert!(before.left() >= px(0.) && before.right() <= px(380.));
                    assert!(footer.top() >= viewport.bottom());
                    assert!(window.find("assistant-scrollbar").visible());
                    // Drag the production thumb, rather than directly changing
                    // the scroll offset or using a test-only reveal helper.
                    let scroll = panel.read(cx).content_scroll.clone();
                    assert!(scroll.max_offset().y > px(0.));
                    let height = f32::from(viewport.size.height);
                    let distance = f32::from(scroll.max_offset().y);
                    let thumb = (height * height / (height + distance)).max(48.);
                    let travel = (height - 8. - thumb).max(0.);
                    let current = (-f32::from(scroll.offset().y) / distance).clamp(0., 1.);
                    let x = viewport.right() - px(7.);
                    window.drag(
                        point(x, viewport.top() + px(14. + current * travel)),
                        point(x, viewport.bottom() - px(2.)),
                        cx,
                    );
                    window.render_frame(cx);
                    assert_eq!(scroll.offset().y, -scroll.max_offset().y);
                    assert_eq!(window.find("send-approved-request").bounds(), before);
                    assert!(!panel.read(cx).busy);
                    assert!(panel.read(cx)._job.is_none());
                    panel.update(cx, |panel, cx| {
                        panel.set_context(
                            "Changed after review".into(),
                            "different host".into(),
                            "different session".into(),
                            cx,
                        );
                    });
                    window.render_frame(cx);
                    assert!(panel.read(cx).prepared.is_none());
                    assert!(!panel.read(cx).preview);
                    assert!(window.try_find("send-approved-request").is_none());
                    assert!(!panel.read(cx).busy);
                }
            }
        })
        .unwrap_or_else(|error| panic!("long protocol review: {error}"));
    }
}

fn fixture(style: AiApiStyle) -> NamedAiProfile {
    let mut p = NamedAiProfile::draft(AiPreset::Custom);
    p.name = "Inference assistant".into();
    p.api_style = style;
    p.endpoint = "https://provider.example/v1/request".into();
    p.model = "fixture-model".into();
    p.authentication = AiAuthentication::None;
    p.reasoning_by_model.insert(
        p.model.clone(),
        AiModelReasoning {
            capability: AiReasoningCapability::Effort {
                values: vec!["high".into()],
            },
            selection: AiReasoningSelection::Effort("high".into()),
        },
    );
    if style == AiApiStyle::AnthropicMessages {
        p.reasoning_by_model.insert(
            p.model.clone(),
            AiModelReasoning {
                capability: AiReasoningCapability::Messages {
                    efforts: vec![keelshell_core::AiMessagesEffort::High],
                    adaptive: true,
                    disabled: false,
                    manual_budget: false,
                },
                selection: AiReasoningSelection::Messages(keelshell_core::AiMessagesInference {
                    effort: Some(keelshell_core::AiMessagesEffort::High),
                    thinking: keelshell_core::AiMessagesThinking::Adaptive,
                }),
            },
        );
    }
    p
}

#[gpui_kit::test]
fn inference_review_contains_exact_protocol_field_and_profile_edit_revokes_it(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    for style in [
        AiApiStyle::ChatCompletions,
        AiApiStyle::Responses,
        AiApiStyle::AnthropicMessages,
    ] {
        let p = fixture(style);
        let catalog = AiProfileCatalog {
            active_id: Some(p.id),
            profiles: vec![p.clone()],
        };
        let (window, panel) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    AssistantPanel::new(
                        &catalog,
                        &EphemeralCredentials::new(),
                        runtime.clone(),
                        window,
                        cx,
                    )
                })
            })
            .unwrap_or_else(|error| panic!("open inference window: {error}"))
        });
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.prompt.update(cx, |input, cx| {
                    input.set_value("Explain selected output", window, cx)
                });
                panel.set_context(
                    "fixture output".into(),
                    "fixture host".into(),
                    "session-a".into(),
                    cx,
                );
                panel.prepare(cx);
                let prepared = panel
                    .prepared
                    .as_ref()
                    .unwrap_or_else(|| panic!("reviewed inference payload"));
                let payload: serde_json::Value = serde_json::from_str(prepared.preview_json())
                    .unwrap_or_else(|error| panic!("reviewed JSON: {error}"));
                let expected = match style {
                    AiApiStyle::ChatCompletions => &payload["reasoning_effort"],
                    AiApiStyle::Responses => &payload["reasoning"]["effort"],
                    AiApiStyle::AnthropicMessages => &payload["output_config"]["effort"],
                };
                assert_eq!(expected, "high");
                if style == AiApiStyle::AnthropicMessages {
                    assert_eq!(payload["thinking"]["type"], "adaptive");
                }
                assert!(!panel.busy);
                assert!(panel.cancellation.is_none());
                let cancellation = RequestCancellation::new();
                panel.cancellation = Some(cancellation.clone());
                let before = panel.request_revision;
                let mut changed = p.clone();
                changed.reasoning_by_model.clear();
                panel.set_profile(Some(changed), None, cx);
                assert!(cancellation.is_cancelled());
                assert!(panel.prepared.is_none());
                assert!(panel.request_revision > before);
                panel.send(cx);
                assert!(!panel.busy);
            });
        })
        .unwrap_or_else(|error| panic!("update inference window: {error}"));
    }
}

#[gpui_kit::test]
fn inactive_configured_secret_in_effort_metadata_refuses_preview(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    let p = fixture(AiApiStyle::AnthropicMessages);
    let mut inactive = fixture(AiApiStyle::Responses);
    inactive.name = "Inactive credential owner".into();
    let catalog = AiProfileCatalog {
        active_id: Some(p.id),
        profiles: vec![p, inactive.clone()],
    };
    let mut credentials = EphemeralCredentials::new();
    credentials.insert(inactive.id, Zeroizing::new("high".into()));
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AssistantPanel::new(&catalog, &credentials, runtime, window, cx))
        })
        .unwrap_or_else(|error| panic!("open inference window: {error}"))
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.prompt.update(cx, |input, cx| {
                input.set_value("ordinary question", window, cx)
            });
            panel.set_context(
                "ordinary selected text".into(),
                "host".into(),
                "session".into(),
                cx,
            );
            panel.prepare(cx);
            assert!(panel.prepared.is_none());
            assert!(!panel.busy);
            assert!(!panel.status.render(cx).contains("high"));
        });
    })
    .unwrap_or_else(|error| panic!("update inference window: {error}"));
}
