//! Real GPUI controls and captured-target admission; no provider request is sent.
use super::super::LocalRequestProgress;
use super::*;
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, TestSupportExt,
    Window, div,
};

type Observed = Arc<std::sync::Mutex<Vec<(String, String)>>>;

fn observe(
    panel: &Entity<AssistantPanel>,
    cx: &mut TestAppContext,
) -> (Observed, gpui_kit::Subscription) {
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let copied = observed.clone();
    let subscription = cx.update(|cx| {
        cx.subscribe(panel, move |_, event: &AssistantEvent, _| {
            if let AssistantEvent::Suggestion {
                command,
                session_id,
            } = event
            {
                copied
                    .lock()
                    .unwrap_or_else(|_| panic!("events lock"))
                    .push((command.clone(), session_id.clone()));
            }
        })
    });
    (observed, subscription)
}

fn reveal_review(window: &mut Window, cx: &mut gpui_kit::App) {
    window.render_frame(cx);
    window.scroll(
        "assistant-scroll",
        gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
        cx,
    );
    window.render_frame(cx);
}

fn assert_inert_review_control(
    id: impl Into<gpui_kit::ElementId>,
    panel: &Entity<AssistantPanel>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    // Kit 0.7 does not expose aria-disabled on Button. Observe real GPUI pointer/key
    // dispatch instead: disabled controls cannot focus or run their callback.
    let before = panel.read(cx).status.render(cx);
    window.blur(cx);
    window.click(id, cx);
    assert!(window.focused(cx).is_none());
    window.press("enter", cx);
    window.press("space", cx);
    assert_eq!(panel.read(cx).status.render(cx), before);
}

#[gpui_kit::test]
fn unavailable_target_disables_controls_with_localized_reason_and_defensive_guard(
    cx: &mut TestAppContext,
) {
    let (handle, panel) = mount(cx);
    let (observed, _subscription) = observe(&panel, cx);
    for language in [Language::ZhCn, Language::En] {
        for (host, session, missing) in [
            ("", "", true),
            (" \t", "\n", true),
            ("fixture-host", "", false),
            ("", "session-A", false),
        ] {
            cx.update_window(handle, |_, window, cx| {
                set_language(language, cx);
                panel.update(cx, |panel, cx| {
                    panel.set_context(String::new(), host.into(), session.into(), cx);
                    panel.receive_review_response_for_test(
                        "```sh\nprintf 'ordinary-command'\n```".into(),
                        cx,
                    );
                });
                reveal_review(window, cx);
                let review = window.find(("review-suggestion", 0_usize));
                assert!(review.visible());
                let target = window.find(("suggestion-review-target", 0_usize));
                let label = target
                    .label()
                    .unwrap_or_else(|| panic!("accessible reason"));
                let expected = match (language, missing) {
                    (Language::ZhCn, true) => "未绑定 SSH 主机和会话",
                    (Language::En, true) => "has no SSH host or session",
                    (Language::ZhCn, false) => "不完整或无效",
                    (Language::En, false) => "incomplete or invalid",
                };
                assert!(target.visible() && label.contains(expected), "{label}");
                assert_inert_review_control(("review-suggestion", 0_usize), &panel, window, cx);
                assert_inert_review_control("build-diagnostic-plan", &panel, window, cx);
                assert!(
                    observed
                        .lock()
                        .unwrap_or_else(|_| panic!("events lock"))
                        .is_empty()
                );
                panel.update(cx, |panel, cx| {
                    panel.suggest(0, cx);
                    panel.build_diagnostic_plan(cx);
                    assert!(panel.diagnostic_plan.is_none());
                    assert!(panel.status.render(cx).contains(expected));
                });
            })
            .unwrap_or_else(|e| panic!("unavailable controls: {e}"));
        }
    }
    assert!(
        observed
            .lock()
            .unwrap_or_else(|_| panic!("events lock"))
            .is_empty()
    );
}

#[gpui_kit::test]
fn exact_captured_command_and_diagnostic_step_emit_only_after_manual_click(
    cx: &mut TestAppContext,
) {
    const COMMAND: &str = "  printf 'review 中文\\n'\n\tprintf 'done'";
    let (handle, panel) = mount(cx);
    let (observed, _subscription) = observe(&panel, cx);
    panel.update(cx, |panel, cx| {
        panel.receive_review_response_for_test(format!("```sh\n{COMMAND}\n```"), cx)
    });
    assert!(
        observed
            .lock()
            .unwrap_or_else(|_| panic!("events lock"))
            .is_empty()
    );
    cx.update_window(handle, |_, window, cx| {
        set_language(Language::En, cx);
        reveal_review(window, cx);
        let target = window.find(("suggestion-review-target", 0_usize));
        let label = target
            .label()
            .unwrap_or_else(|| panic!("captured target label"));
        assert!(label.contains("ops@server.example:22") && label.contains("session-A"));
        window.click(("review-suggestion", 0_usize), cx);
    })
    .unwrap_or_else(|e| panic!("manual exact proposal: {e}"));
    // Entity events are delivered after the window update has completed.
    cx.run_until_parked();
    assert_eq!(
        observed
            .lock()
            .unwrap_or_else(|_| panic!("events lock"))
            .as_slice(),
        [(COMMAND.into(), "session-A".into())]
    );
    cx.update_window(handle, |_, window, cx| {
        window.click("build-diagnostic-plan", cx);
        reveal_review(window, cx);
        window.click(("review-diagnostic-step", 0_usize), cx);
    })
    .unwrap_or_else(|e| panic!("manual exact proposals: {e}"));
    cx.run_until_parked();
    assert_eq!(
        *observed.lock().unwrap_or_else(|_| panic!("events lock")),
        [
            (COMMAND.into(), "session-A".into()),
            (COMMAND.into(), "session-A".into())
        ]
    );
}

#[gpui_kit::test]
fn stale_response_target_and_plan_cannot_rebind_to_changed_panel_session(cx: &mut TestAppContext) {
    let (handle, panel) = mount(cx);
    let (observed, _subscription) = observe(&panel, cx);
    panel.update(cx, |panel, cx| {
        panel.receive_review_response_for_test("```sh\nuptime\n```".into(), cx);
        panel.build_diagnostic_plan(cx);
        assert!(panel.diagnostic_plan.is_some());
        // Defensive stale-frame case: the old response survives a field change.
        // Normal explicit capture revokes it instead (checked below).
        panel.session_id = "session-B".into();
        panel.suggest(0, cx);
        panel.suggest_diagnostic_step(0, cx);
        panel.build_diagnostic_plan(cx);
        assert_eq!(
            panel.response_target.as_ref().map(|t| t.1.as_str()),
            Some("session-A")
        );
        assert_eq!(
            panel.diagnostic_plan.as_ref().map(|p| p.session_id()),
            Some("session-A")
        );
    });
    cx.update_window(handle, |_, window, cx| {
        set_language(Language::En, cx);
        reveal_review(window, cx);
        assert_inert_review_control(("review-suggestion", 0_usize), &panel, window, cx);
        assert_inert_review_control(("review-diagnostic-step", 0_usize), &panel, window, cx);
        let label = window
            .find(("suggestion-review-target", 0_usize))
            .label()
            .unwrap_or_default()
            .to_owned();
        assert!(
            label.contains("session-A")
                && !label.contains("session-B")
                && label.contains("captured target has changed")
        );
        panel.update(cx, |panel, cx| {
            let old_revision = panel.request_revision;
            panel.set_context(
                "new context".into(),
                "new-host".into(),
                "session-B".into(),
                cx,
            );
            panel.finish_reply(
                old_revision,
                ("ops@server.example:22".into(), "session-A".into()),
                Ok("```sh\necho stale\n```".into()),
                cx,
            );
            assert!(
                panel.response.is_empty()
                    && panel.response_target.is_none()
                    && panel.diagnostic_plan.is_none()
            );
        });
    })
    .unwrap_or_else(|e| panic!("stale response target: {e}"));
    assert!(
        observed
            .lock()
            .unwrap_or_else(|_| panic!("events lock"))
            .is_empty()
    );
}

struct ReviewScene(Entity<AssistantPanel>);
impl Render for ReviewScene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().justify_end().child(
            div()
                .id("review-panel-area")
                .test_support()
                .w(px(320.))
                .h_full()
                .min_w_0()
                .overflow_hidden()
                .child(self.0.clone()),
        )
    }
}

fn contains(inner: Bounds<gpui_kit::Pixels>, outer: Bounds<gpui_kit::Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
fn long_reply_keeps_target_reason_review_and_progress_reachable_in_minimum_layout(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|e| panic!("runtime: {e}")),
    );
    let catalog = AiProfileCatalog::default();
    let (handle, scene) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let panel = cx.new(|cx| {
                    AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
                });
                cx.new(|_| ReviewScene(panel))
            },
        )
        .unwrap_or_else(|e| panic!("minimum review window: {e}"))
    });
    let panel = scene.read_with(cx, |scene, _| scene.0.clone());
    let (observed, _subscription) = observe(&panel, cx);
    for language in [Language::ZhCn, Language::En] {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for available in [false, true] {
                for expanded in [false, true] {
                    let before = observed
                        .lock()
                        .unwrap_or_else(|_| panic!("events lock"))
                        .len();
                    cx.update_window(handle, |_, window, cx| {
                        set_language(language, cx);
                        crate::design::apply(theme, Some(window), cx);
                        panel.update(cx, |panel, cx| {
                            let target = if available { ("fixture-host".to_owned(), "fixture-session".to_owned()) } else { (String::new(), String::new()) };
                            panel.set_context("fixed context".into(), target.0.clone(), target.1.clone(), cx);
                            let mut progress = LocalRequestProgress::new(panel.request_revision, target.clone());
                            progress.facts = keelshell_ai::LocalAskStage::ALL.to_vec();
                            progress.expanded = expanded;
                            panel.local_progress = Some(progress);
                            panel.receive_review_response_for_test(format!("{}\n```sh\nuptime\n```", "long explanation 中文\n".repeat(100)), cx);
                        });
                        reveal_review(window, cx);
                        let scroll = window.find("assistant-scroll").bounds();
                        let target = window.find(("suggestion-review-target", 0_usize));
                        let review = window.find(("review-suggestion", 0_usize));
                        assert!(target.visible() && review.visible() && contains(target.bounds(), scroll) && contains(review.bounds(), scroll), "review/target clipped {language:?}/{theme:?}/{available}/{expanded}: target={:?}, review={:?}, scroll={scroll:?}", target.bounds(), review.bounds());
                        if available {
                            window.click(("review-suggestion", 0_usize), cx);
                        } else {
                            assert_inert_review_control(("review-suggestion", 0_usize), &panel, window, cx);
                        }
                        assert!(window.find("assistant-request-bar").visible());
                        assert!(window.find("local-ask-headline").visible());
                        assert!(contains(review.bounds(), window.find("review-panel-area").bounds()));
                    }).unwrap_or_else(|e| panic!("minimum target layout: {e}"));
                    cx.run_until_parked();
                    let events = observed.lock().unwrap_or_else(|_| panic!("events lock"));
                    assert_eq!(events.len(), before + usize::from(available));
                    if available {
                        assert_eq!(
                            events.get(before),
                            Some(&("uptime".into(), "fixture-session".into()))
                        );
                    }
                }
            }
        }
    }
}
