//! Production progress reducer, foreground bridge and minimum-window layout.
use super::super::{LocalRequestProgress, stage_message};
use super::*;
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, TestSupportExt,
    Window, div,
};
use keelshell_ai::{LocalAskProgress, LocalAskStage};

fn begin(panel: &mut AssistantPanel) -> (u64, (String, String)) {
    let revision = panel.request_revision;
    let target = (panel.host.clone(), panel.session_id.clone());
    panel.busy = true;
    panel.cancellation = Some(RequestCancellation::new());
    panel.local_progress = Some(LocalRequestProgress::new(revision, target.clone()));
    (revision, target)
}

#[gpui_kit::test]
fn local_progress_admits_exact_owner_once_and_finalizing_does_not_regress(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let (revision, target) = begin(panel);
        panel.observe_local_stage(
            revision,
            &(target.0.clone(), "other-session".into()),
            LocalAskStage::CliAdmitted,
            cx,
        );
        panel.observe_local_stage(
            revision.wrapping_add(1),
            &target,
            LocalAskStage::CliAdmitted,
            cx,
        );
        assert!(
            panel
                .local_progress
                .as_ref()
                .is_some_and(|p| p.facts.is_empty())
        );
        panel.finish_reply(
            revision,
            ("other-host".into(), target.1.clone()),
            Ok("wrong response".into()),
            cx,
        );
        assert!(panel.busy && panel.response.is_empty());
        for stage in [
            LocalAskStage::InputDelivered,
            LocalAskStage::InputDelivered,
            LocalAskStage::Finalizing,
            LocalAskStage::ProtocolCompleted,
        ] {
            panel.observe_local_stage(revision, &target, stage, cx);
        }
        let progress = panel
            .local_progress
            .as_ref()
            .unwrap_or_else(|| panic!("progress"));
        assert_eq!(
            progress.facts,
            [
                LocalAskStage::InputDelivered,
                LocalAskStage::Finalizing,
                LocalAskStage::ProtocolCompleted
            ]
        );
        assert_eq!(
            progress.headline(),
            stage_message(LocalAskStage::Finalizing)
        );
        assert!(panel.suggestions.is_empty());
        panel.finish_reply(
            revision,
            target.clone(),
            Ok("```sh\necho reviewed\n```".into()),
            cx,
        );
        assert!(!panel.busy);
        assert_eq!(panel.suggestions, ["echo reviewed"]);
        assert!(
            panel
                .local_progress
                .as_ref()
                .is_some_and(|p| p.outcome == Some(true))
        );
        panel.observe_local_stage(revision, &target, LocalAskStage::ProcessStarted, cx);
        assert_eq!(
            panel.local_progress.as_ref().map(|p| p.facts.len()),
            Some(3)
        );
    });
}

#[gpui_kit::test]
fn local_progress_clears_on_cancel_and_session_change_and_rejects_old_events(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let (old_revision, old_target) = begin(panel);
        let cancellation = panel
            .cancellation
            .clone()
            .unwrap_or_else(|| panic!("token"));
        panel.observe_local_stage(old_revision, &old_target, LocalAskStage::ProcessStarted, cx);
        panel.cancel(cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.local_progress.is_none());
        assert!(panel.status.render(cx).contains("已请求取消"));
        panel.set_context(
            "new context".into(),
            "new-host".into(),
            "session-B".into(),
            cx,
        );
        let (new_revision, new_target) = begin(panel);
        panel.observe_local_stage(new_revision, &new_target, LocalAskStage::WorkspaceReady, cx);
        panel.observe_local_stage(old_revision, &old_target, LocalAskStage::Finalizing, cx);
        panel.finish_reply(old_revision, old_target, Ok("old answer".into()), cx);
        assert!(panel.busy && panel.response.is_empty());
        assert_eq!(
            panel.local_progress.as_ref().map(|p| p.facts.clone()),
            Some(vec![LocalAskStage::WorkspaceReady])
        );
        panel.invalidate_session("session-B", cx);
        assert!(panel.local_progress.is_none());
        assert!(!panel.busy);
    });
}

#[gpui_kit::test]
async fn local_progress_bridge_delivers_real_failure_on_foreground_and_stale_completion_is_rejected(
    cx: &mut TestAppContext,
) {
    use crate::runtime_bridge::{LocalAskEvent, spawn_local_ask};
    use keelshell_ai::{
        ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentKind,
    };
    let (handle, panel) = mount(cx);
    let scratch = tempfile::tempdir().unwrap_or_else(|e| panic!("owned scratch: {e}"));
    // libtest is deliberately not a supported CLI. The owned --version child
    // really exits unsuccessfully; no installed supplier or model is invoked.
    let executable = std::env::current_exe().unwrap_or_else(|e| panic!("test exe: {e}"));
    let config = LocalAgentConfig::new(
        LocalAgentKind::Codex,
        executable,
        scratch.path(),
        "fixture-model",
    )
    .unwrap_or_else(|e| panic!("config: {e}"));
    let review = config
        .prepare(ContextDraft::new("fixed canary"), &[], 8192)
        .unwrap_or_else(|e| panic!("review: {e}"));
    let (revision, target, runtime) = panel.update(cx, |panel, _| {
        let (revision, target) = begin(panel);
        (revision, target, panel.runtime.clone())
    });
    let (progress, receiver) = LocalAskProgress::channel();
    let signal = RequestCancellation::new();
    let job = spawn_local_ask(
        &runtime,
        cx.background_executor.clone(),
        receiver,
        async move {
            let credential = LocalAgentCredential::new("owned-test-token")
                .unwrap_or_else(|e| panic!("credential: {e}"));
            LocalAgentClient
                .ask_with_progress(review.approve(), credential, &signal, progress)
                .await
                .map(|reply| reply.text().to_owned())
                .map_err(crate::ai_settings::local_agent_error)
        },
    );
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_copy = done.clone();
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed_copy = observed.clone();
    let panel_copy = panel.clone();
    let task = cx.spawn(async move |mut cx| {
        loop {
            match job
                .next()
                .await
                .unwrap_or_else(|_| panic!("bridge disconnected"))
            {
                LocalAskEvent::Stage(stage) => {
                    observed_copy
                        .lock()
                        .unwrap_or_else(|_| panic!("facts lock"))
                        .push(stage);
                    panel_copy.update(&mut cx, |panel, cx| {
                        panel.observe_local_stage(revision, &target, stage, cx);
                        if stage == LocalAskStage::WorkspaceReady {
                            panel.set_context(
                                "fresh context".into(),
                                "new-host".into(),
                                "session-B".into(),
                                cx,
                            );
                            let (revision, target) = begin(panel);
                            panel.observe_local_stage(
                                revision,
                                &target,
                                LocalAskStage::InputDelivered,
                                cx,
                            );
                        }
                    });
                }
                LocalAskEvent::Complete(result) => {
                    assert!(result.is_err());
                    panel_copy.update(&mut cx, |panel, cx| {
                        panel.finish_reply(revision, target, result, cx)
                    });
                    done_copy.store(true, std::sync::atomic::Ordering::Release);
                    break;
                }
            }
        }
    });
    cx.wait_for(handle, Duration::from_secs(5), |_, _| {
        done.load(std::sync::atomic::Ordering::Acquire)
    })
    .await;
    task.await;
    assert_eq!(
        *observed.lock().unwrap_or_else(|_| panic!("facts lock")),
        [
            LocalAskStage::WorkspaceReady,
            LocalAskStage::CheckingCli,
            LocalAskStage::Finalizing
        ]
    );
    panel.read_with(cx, |panel, _| {
        assert!(panel.busy && panel.response.is_empty());
        assert_eq!(panel.session_id, "session-B");
        assert_eq!(
            panel.local_progress.as_ref().map(|p| p.facts.clone()),
            Some(vec![LocalAskStage::InputDelivered])
        );
    });
    assert_eq!(
        std::fs::read_dir(scratch.path())
            .unwrap_or_else(|e| panic!("scratch: {e}"))
            .count(),
        0
    );
}

struct ProgressScene(Entity<AssistantPanel>);

#[gpui_kit::test]
fn local_progress_question_profile_and_panel_destruction_revoke_ownership(cx: &mut TestAppContext) {
    let (handle, panel) = mount(cx);
    let mut token = RequestCancellation::new();
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_profile(
                Some(local_profile(keelshell_core::AiLocalAgent::Codex)),
                Some(Zeroizing::new("fixed-token".into())),
                cx,
            );
            let (revision, target) = begin(panel);
            token = panel
                .cancellation
                .clone()
                .unwrap_or_else(|| panic!("question token"));
            panel.observe_local_stage(revision, &target, LocalAskStage::ProcessStarted, cx);
        });
        window.render_frame(cx);
        window.click(("input", panel.read(cx).prompt.entity_id()), cx);
        window.input("x", cx);
    })
    .unwrap_or_else(|e| panic!("real question edit: {e}"));
    cx.run_until_parked();
    assert!(token.is_cancelled());
    panel.update(cx, |panel, cx| {
        assert!(panel.local_progress.is_none());
        let (revision, target) = begin(panel);
        let token = panel
            .cancellation
            .clone()
            .unwrap_or_else(|| panic!("profile token"));
        panel.observe_local_stage(revision, &target, LocalAskStage::ProcessStarted, cx);
        let mut selected = panel.profile.clone().unwrap_or_else(|| panic!("profile"));
        selected.model = "different-model".into();
        panel.set_profile(
            Some(selected),
            Some(Zeroizing::new("fixed-token".into())),
            cx,
        );
        assert!(token.is_cancelled());
        assert!(panel.local_progress.is_none());
        let (revision, target) = begin(panel);
        panel.observe_local_stage(revision, &target, LocalAskStage::InputDelivered, cx);
    });
    // A rendered window root and cached elements can retain owners. Exercise
    // actual last-owner destruction with a real, unmounted panel, and verify
    // the weak entity is gone before making a cleanup-request claim.
    let (catalog, credentials, runtime) = panel.read_with(cx, |panel, _| {
        (
            panel.profiles.clone(),
            panel.credentials.clone(),
            panel.runtime.clone(),
        )
    });
    let detached = cx
        .update_window(handle, |_, window, cx| {
            cx.new(|cx| AssistantPanel::new(&catalog, &credentials, runtime, window, cx))
        })
        .unwrap_or_else(|e| panic!("unmounted panel: {e}"));
    let token = detached.update(cx, |panel, cx| {
        let (revision, target) = begin(panel);
        panel.observe_local_stage(revision, &target, LocalAskStage::InputDelivered, cx);
        panel
            .cancellation
            .clone()
            .unwrap_or_else(|| panic!("drop token"))
    });
    let weak = detached.downgrade();
    drop(detached);
    cx.update(|_| {});
    cx.run_until_parked();
    assert!(
        weak.upgrade().is_none(),
        "last owner was not actually released"
    );
    assert!(
        token.is_cancelled(),
        "destroying the last panel owner must request cleanup"
    );
}

impl Render for ProgressScene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().justify_end().child(
            div()
                .id("progress-panel-area")
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
fn local_progress_minimum_window_keeps_cancel_history_and_command_review_reachable(
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
    let profile = local_profile(keelshell_core::AiLocalAgent::Codex);
    let catalog = AiProfileCatalog {
        active_id: Some(profile.id),
        profiles: vec![profile],
    };
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
                cx.new(|_| ProgressScene(panel))
            },
        )
        .unwrap_or_else(|e| panic!("progress window: {e}"))
    });
    let panel = scene.read_with(cx, |scene, _| scene.0.clone());
    panel.update(cx, |panel, cx| {
        panel.set_context(
            "fixed context".into(),
            "fixture-host".into(),
            "fixture-session".into(),
            cx,
        );
        let (revision, target) = begin(panel);
        for stage in LocalAskStage::ALL {
            panel.observe_local_stage(revision, &target, stage, cx);
        }
    });
    for language in [Language::ZhCn, Language::En] {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for expanded in [false, true] {
                cx.update_window(handle, |_, window, cx| {
                    set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                    panel.update(cx, |panel, cx| { if let Some(p) = &mut panel.local_progress { p.expanded = expanded; } cx.notify(); });
                    window.render_frame(cx);
                    let area = window.find("progress-panel-area").bounds();
                    let cancel = window.find("assistant-cancel-request");
                    assert!(cancel.visible() && contains(cancel.bounds(), area) && contains(cancel.bounds(), window.bounds()), "cancel inaccessible: {language:?}/{theme:?}/{expanded}");
                    assert!(window.find("assistant-scroll").bounds().size.height > px(100.));
                    window.scroll("assistant-scroll", gpui_kit::ScrollDelta::Lines(point(0., -1000.)), cx);
                    window.render_frame(cx);
                    assert_eq!(window.find("assistant-cancel-request").bounds(), cancel.bounds(), "body scroll moved fixed cancellation");
                    if expanded {
                        assert!(window.find("local-ask-facts").bounds().size.height <= px(144.));
                        window.scroll("local-ask-facts", gpui_kit::ScrollDelta::Lines(point(0., -1000.)), cx);
                        window.render_frame(cx);
                        assert!(window.find(("local-ask-fact", 7_usize)).visible());
                    }
                    eprintln!("local-progress 900x580 320px {language:?}/{theme:?}/expanded={expanded}: cancel={:?}; body={:?}", window.find("assistant-cancel-request").bounds(), window.find("assistant-scroll").bounds());
                }).unwrap_or_else(|e| panic!("progress layout: {e}"));
            }
        }
    }
    cx.update_window(handle, |_, window, cx| {
        window.click("assistant-cancel-request", cx);
        window.render_frame(cx);
        assert!(window.try_find("assistant-request-bar").is_none());
    })
    .unwrap_or_else(|e| panic!("actual cancel: {e}"));
    let observed = Arc::new(std::sync::Mutex::new(None));
    let captured = observed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event: &AssistantEvent, _| {
            if let AssistantEvent::Suggestion {
                command,
                session_id,
            } = event
            {
                *captured
                    .lock()
                    .unwrap_or_else(|_| panic!("suggestion lock")) =
                    Some((command.clone(), session_id.clone()));
            }
        })
    });
    for language in [Language::ZhCn, Language::En] {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            cx.update_window(handle, |_, window, cx| {
                set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                panel.update(cx, |panel, cx| {
                    panel.invalidate_request(cx);
                    let (revision, target) = begin(panel);
                    panel.observe_local_stage(revision, &target, LocalAskStage::Finalizing, cx);
                    panel.finish_reply(
                        revision,
                        target,
                        Ok(format!(
                            "{}\n```sh\necho reviewed\n```",
                            "fixed explanation\n".repeat(80)
                        )),
                        cx,
                    );
                });
                window.render_frame(cx);
                window.scroll(
                    "assistant-scroll",
                    gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                    cx,
                );
                window.render_frame(cx);
                let review = window.find(("review-suggestion", 0_usize));
                assert!(
                    review.visible()
                        && contains(review.bounds(), window.find("assistant-scroll").bounds())
                );
                window.click(("review-suggestion", 0_usize), cx);
            })
            .unwrap_or_else(|e| panic!("post-result review layout: {e}"));
        }
    }
    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(|_| panic!("suggestion lock")),
        Some(("echo reviewed".into(), "fixture-session".into()))
    );
}
