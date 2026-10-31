use super::{AssistantPanel, KnowledgeReviewOutcome};
use crate::ai_settings::EphemeralCredentials;
use gpui_kit::{AppContext, TestAppContext, WindowOptions, test::TestWindowExt};
use keelshell_ai::RequestCancellation;
use keelshell_core::{AiApiStyle, AiProfileCatalog, KnowledgeOutcome, NamedAiProfile};
use keelshell_core::{
    AiAuthentication, AiModelReasoning, AiPreset, AiReasoningCapability, AiReasoningSelection,
};
use std::sync::Arc;
use uuid::Uuid;
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

#[gpui_kit::test]
fn explicit_terminal_context_builds_reviewable_knowledge_preview_and_keeps_source_binding(
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
    let catalog = AiProfileCatalog::default();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
            })
        })
        .unwrap_or_else(|error| panic!("open knowledge preview window: {error}"))
    });
    let source_id = cx
        .update_window(window, |_, _, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_context(
                    "systemctl status keelshell\nservice is healthy".into(),
                    "fixture-host".into(),
                    "session-a".into(),
                    cx,
                );
                panel.build_knowledge_preview(cx);
                let (document, markdown) = panel
                    .knowledge_preview_for_test()
                    .unwrap_or_else(|| panic!("explicit selection should produce a preview"));
                assert!(!document.disposition().is_persistable());
                assert!(document.entries().is_empty());
                assert!(markdown.text().contains("unknown"));
                assert_eq!(document.exclusions().len(), 1);
                document.exclusions()[0].source().id()
            })
        })
        .unwrap_or_else(|error| panic!("build unknown knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-confirm-completed", cx);
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("confirm knowledge outcome: {error}"));

    cx.update_window(window, |_, window, cx| {
        let review_id = panel.update(cx, |panel, _| {
            let (document, markdown) = panel
                .knowledge_preview_for_test()
                .unwrap_or_else(|| panic!("explicit outcome review should rebuild the preview"));
            assert!(document.disposition().is_persistable());
            assert_eq!(document.entries().len(), 1);
            assert!(markdown.text().contains("service is healthy"));
            let review_id = panel
                .knowledge_review_id_for_test()
                .unwrap_or_else(|| panic!("review token should be present"));
            assert!(panel.knowledge_save_candidate(review_id).is_some());
            review_id
        });
        window.render_frame(cx);
        assert!(window.find("knowledge-save").visible());

        panel.update(cx, |panel, cx| {
            panel.set_context(
                "service is healthy again".into(),
                "fixture-host".into(),
                "session-a".into(),
                cx,
            );
            panel.build_knowledge_preview(cx);
            let (document, _) = panel
                .knowledge_preview_for_test()
                .unwrap_or_else(|| panic!("same session should remain reviewable"));
            assert_eq!(document.exclusions()[0].source().id(), source_id);
            assert!(panel.knowledge_save_candidate(review_id).is_none());
        });
    })
    .unwrap_or_else(|error| panic!("inspect confirmed knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-confirm-failed", cx);
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("confirm failed knowledge outcome: {error}"));

    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, _| {
            let (document, _) = panel
                .knowledge_preview_for_test()
                .unwrap_or_else(|| panic!("failed outcome should rebuild the preview"));
            assert_eq!(document.entries()[0].outcome(), KnowledgeOutcome::Failed);
            let review_id = panel
                .knowledge_review_id_for_test()
                .unwrap_or_else(|| panic!("failed review token should be present"));
            assert!(panel.knowledge_save_candidate(review_id).is_some());
        });
    })
    .unwrap_or_else(|error| panic!("inspect failed knowledge preview: {error}"));

    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_context(
                "api key=fixture-secret".into(),
                "fixture-host".into(),
                "session-a".into(),
                cx,
            );
            panel.build_knowledge_preview(cx);
        });
    })
    .unwrap_or_else(|error| panic!("build sensitive knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-confirm-completed", cx);
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("confirm sensitive knowledge outcome: {error}"));

    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, _| {
            let (document, markdown) = panel.knowledge_preview_for_test().unwrap_or_else(|| {
                panic!("sensitive content should still yield a blocked preview")
            });
            assert!(!document.disposition().is_persistable());
            assert!(markdown.text().contains("sensitive_content"));
            assert!(!markdown.text().contains("fixture-secret"));
        });
    })
    .unwrap_or_else(|error| panic!("inspect sensitive knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("knowledge-preview-action").visible());
        assert!(window.find("knowledge-save-disabled").visible());
    })
    .unwrap_or_else(|error| panic!("knowledge preview interaction: {error}"));
}

#[gpui_kit::test]
fn unknown_knowledge_write_result_disables_automatic_replay(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    let catalog = AiProfileCatalog::default();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
            })
        })
        .unwrap_or_else(|error| panic!("open knowledge save window: {error}"))
    });

    let review_id = cx
        .update_window(window, |_, _, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_context(
                    "service is healthy".into(),
                    "fixture-host".into(),
                    "session-a".into(),
                    cx,
                );
                panel.build_knowledge_preview(cx);
                panel.set_knowledge_review_outcome(KnowledgeReviewOutcome::Completed, cx);
                panel
                    .knowledge_review_id_for_test()
                    .unwrap_or_else(|| panic!("review token should be present"))
            })
        })
        .unwrap_or_else(|error| panic!("build reviewable knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("knowledge-save").visible());
        panel.update(cx, |panel, cx| {
            panel.mark_knowledge_save_pending(review_id, cx);
            panel.mark_knowledge_save_failed(review_id, true, cx);
            assert!(!panel.knowledge_save_ready_for_test());
        });
        window.render_frame(cx);
        assert!(window.find("knowledge-save-unknown").visible());
        assert!(window.find("knowledge-reload").visible());
        assert!(window.try_find("knowledge-save").is_none());
        panel.update(cx, |panel, cx| {
            assert!(panel.mark_knowledge_reload_pending(review_id, cx));
            assert!(panel.mark_knowledge_reload_failed(
                review_id,
                &keelshell_core::Error::TooLarge,
                cx,
            ));
        });
        window.render_frame(cx);
        assert!(window.find("knowledge-reload").visible());
        assert!(window.try_find("knowledge-save").is_none());
    })
    .unwrap_or_else(|error| panic!("retain unknown save result without replay: {error}"));
}

#[gpui_kit::test]
fn stale_knowledge_save_completion_does_not_mark_current_preview_saved(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    let catalog = AiProfileCatalog::default();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
            })
        })
        .unwrap_or_else(|error| panic!("open stale knowledge window: {error}"))
    });
    let review_id = cx
        .update_window(window, |_, _, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_context(
                    "systemctl status keelshell\nservice is healthy".into(),
                    "fixture-host".into(),
                    "session-a".into(),
                    cx,
                );
                panel.build_knowledge_preview(cx);
                panel.set_knowledge_review_outcome(KnowledgeReviewOutcome::Completed, cx);
                panel
                    .knowledge_review_id_for_test()
                    .unwrap_or_else(|| panic!("knowledge review id"))
            })
        })
        .unwrap_or_else(|error| panic!("build stale knowledge preview: {error}"));

    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.mark_knowledge_save_pending(review_id, cx);
            assert!(!panel.mark_knowledge_saved(Uuid::new_v4(), cx));
            assert!(!panel.knowledge_save_ready_for_test());
        });
        window.render_frame(cx);
        assert!(window.find("knowledge-save-pending").visible());
        assert!(window.try_find("knowledge-save-saved").is_none());
    })
    .unwrap_or_else(|error| panic!("inspect stale knowledge completion: {error}"));
}
