//! Actual GPUI controls, production workspace dispatch and owned TCP/SSH peers.
use super::*;
use keelshell_core::{Theme, format_fixed_offset_datetime};

mod review_regressions;
mod target_parameters;

fn panel(h: &Harness, cx: &TestAppContext) -> Entity<crate::workflow_commands::WorkflowPanel> {
    h.fixture.workspace.read_with(cx, |view, _| {
        view.workflow_panel
            .clone()
            .checked_option("retained workflow panel")
    })
}
fn set_input(window: &mut Window, id: &'static str, value: &str, cx: &mut App) {
    click_visible(window, id, cx);
    match window
        .focused_input(cx)
        .checked_option("focused schedule field")
    {
        AnyInputState::Input(field) => field.update(cx, |field, cx| {
            field.set_selected_range(0..field.value().len(), cx);
            field.replace(value.to_owned(), window, cx);
        }),
        _ => panic!("actual schedule Input"),
    }
}
async fn real_pause(
    runtime: &tokio::runtime::Runtime,
    duration: Duration,
    cx: &mut TestAppContext,
) {
    let timer = runtime.spawn(async move {
        tokio::time::sleep(duration).await;
    });
    // Never poll a foreign-thread JoinHandle with GPUI's deterministic waker.
    // Poll its completion flag, yielding only through the GPUI test executor;
    // collect it only after it is already finished. This retains real elapsed
    // time without driving UI work on a Tokio worker or blocking a window turn.
    while !timer.is_finished() {
        cx.executor().timer(Duration::from_millis(10)).await;
    }
    runtime
        .block_on(timer)
        .checked("completed owned wall-clock timer");
}
async fn wait_real(
    h: &Harness,
    timeout: Duration,
    cx: &mut TestAppContext,
    ready: impl FnMut(&mut App) -> bool,
) {
    wait_real_observed(h, timeout, cx, ready, |_| String::new()).await;
}

async fn wait_real_observed(
    h: &Harness,
    timeout: Duration,
    cx: &mut TestAppContext,
    mut ready: impl FnMut(&mut App) -> bool,
    mut failure_observation: impl FnMut(&mut App) -> String,
) {
    let deadline = std::time::Instant::now() + timeout;
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    loop {
        let satisfied = cx
            .update_window(h.fixture.window, |_, window, cx| {
                window.render_frame(cx);
                ready(cx)
            })
            .checked("render actual wall-clock schedule");
        if satisfied {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "actual wall-clock schedule deadline {timeout:?}; {}",
            // Assertions evaluate this only on failure. The observer cannot
            // complete work, perform I/O, or renew the original deadline.
            cx.update(|cx| failure_observation(cx)),
        );
        // GPUI's wait_for is measured in deterministic test time. Use an owned
        // Tokio timer as well so a real 60-second occurrence cannot be mistaken
        // for 60 seconds of accelerated test-executor progress.
        real_pause(&runtime, Duration::from_millis(40), cx).await;
        cx.run_until_parked();
    }
}
async fn completed_real(h: &Harness, timeout: Duration, cx: &mut TestAppContext) {
    wait_real(h, timeout, cx, |cx| {
        h.fixture
            .workspace
            .read(cx)
            .workflow_panel
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).is_running())
    })
    .await;
}
fn review_plan(h: &Harness, cx: &mut TestAppContext) {
    // InputEvent subscribers run between UI turns, just as they do between
    // real typing and a later click. Flush edits before preparing the review.
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-review-button", cx);
    })
    .checked("actual later review click");
    cx.run_until_parked();
}
fn schedule(h: &Harness, interval: bool, seconds_ahead: i64, cx: &mut TestAppContext) {
    schedule_source(
        h,
        interval,
        seconds_ahead,
        "printf {{endpoint}}\nprintf 'scheduled 中文'",
        cx,
    );
}
fn schedule_source(
    h: &Harness,
    interval: bool,
    seconds_ahead: i64,
    source: &str,
    cx: &mut TestAppContext,
) {
    h.prepare(source, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-back", cx);
        click_visible(
            window,
            if interval {
                "workflow-schedule-interval"
            } else {
                "workflow-schedule-once"
            },
            cx,
        );
        set_input(window, "workflow-schedule-offset", "+00:00", cx);
        let date = format_fixed_offset_datetime(chrono::Utc::now().timestamp() + seconds_ahead, 0)
            .checked("future date");
        set_input(window, "workflow-schedule-start", &date, cx);
        set_input(window, "workflow-schedule-grace", "15", cx);
        if interval {
            set_input(window, "workflow-schedule-period", "60", cx);
            set_input(window, "workflow-schedule-count", "3", cx);
        }
    })
    .checked("actual typed schedule editor");
    review_plan(h, cx);
    let review = panel(h, cx).read_with(cx, |panel, _| {
        panel.reviewed_for_test().checked_option("scheduled review")
    });
    assert!(review.schedule.is_some());
    assert_eq!(
        review.schedule.as_ref().checked_option("spec").count(),
        if interval { 3 } else { 1 }
    );
    assert!(
        h.servers.iter().all(|server| server.requests().is_empty()),
        "review does not execute"
    );
}

#[gpui_kit::test]
async fn scheduled_workflow_real_due_dispatch_survives_hide_and_reopen(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    schedule(&h, false, 5, cx);
    let original = panel(&h, cx).entity_id();
    let reviewed = panel(&h, cx).read_with(cx, |panel, _| {
        panel.reviewed_for_test().checked_option("review")
    });
    let expected = reviewed.plan.tasks()[0].command.as_bytes().to_vec();
    h.confirm(cx);
    assert!(panel(&h, cx).read_with(cx, |panel, _| panel.is_running()));
    assert!(
        h.servers[0].requests().is_empty(),
        "arming is not immediate execution"
    );
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
        window.render_frame(cx);
    })
    .checked("actual hide click");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!h.fixture.workspace.read(cx).show_workflow);
        assert!(
            window
                .find(("terminal-pane", h.panes[0].terminal.entity_id()))
                .visible()
        );
    })
    .checked("hide armed schedule preserves terminal");
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert_eq!(h.servers[0].requests(), vec![expected]);
    assert!(h.servers[1].requests().is_empty());
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(false, window, cx));
        window.render_frame(cx);
        assert_eq!(
            h.fixture
                .workspace
                .read(cx)
                .workflow_panel
                .as_ref()
                .checked_option("reopened panel")
                .entity_id(),
            original
        );
        click_visible(window, "workflow-schedule-status", cx);
        assert!(window.find("workflow-schedule-status").visible());
        assert!(
            window.try_find("workflow-confirm").is_none(),
            "terminal plan cannot arm again"
        );
    })
    .checked("same completed schedule reopens without replay");
    assert!(
        h.panes.iter().all(|pane| writes(pane).is_empty()),
        "exec never injects into PTY"
    );
}

#[gpui_kit::test]
async fn scheduled_workflow_cancel_withdraws_due_authority(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    schedule(&h, true, 4, cx);
    h.confirm(cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-cancel", cx);
        window.render_frame(cx);
        assert!(window.try_find("workflow-confirm").is_none());
    })
    .checked("actual cancel before first deadline");
    completed_real(&h, Duration::from_secs(10), cx).await;
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    real_pause(&runtime, Duration::from_secs(5), cx).await;
    cx.run_until_parked();
    assert!(
        h.servers.iter().all(|server| server.requests().is_empty()),
        "cancelled schedule never executes or catches up"
    );
}

#[gpui_kit::test]
async fn scheduled_workflow_authenticated_replacement_and_silent_edit_fail_closed(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule(&h, false, 5, cx);
    h.confirm(cx);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let replacement = peer::Server::new(&runtime, 0);
    h.fixture.workspace.update(cx, |view, cx| {
        view.remote_sessions
            .insert(h.panes[0].terminal.entity_id(), replacement.session.clone());
        view.maintain_workflow(cx);
    });
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert!(
        h.servers[0].requests().is_empty() && replacement.requests().is_empty(),
        "same endpoint does not transfer original authorization"
    );

    let edited = Harness::new(cx);
    schedule(&edited, false, 4, cx);
    edited.confirm(cx);
    // Programmatic edits do not emit InputEvent. The final snapshot must still
    // compare the exact reviewed text rather than only its parsed numeric value.
    let edited_panel = panel(&edited, cx);
    cx.update_window(edited.fixture.window, |_, window, cx| {
        edited
            .fixture
            .workspace
            .update(cx, |view, _| view.show_workflow = false);
        edited_panel.update(cx, |panel, cx| {
            panel
                .schedule_grace_for_test()
                .update(cx, |field, cx| field.set_value("015", window, cx));
        });
    })
    .checked("silent equivalent reviewed-grace mutation while hidden");
    completed_real(&edited, Duration::from_secs(10), cx).await;
    assert!(
        edited
            .servers
            .iter()
            .all(|server| server.requests().is_empty())
    );
    assert!(replacement.requests().is_empty());
}

#[gpui_kit::test]
fn scheduled_workflow_controls_review_fit_minimum_all_appearances(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.simulate_window_resize(h.fixture.window, size(px(900.), px(580.)));
    schedule(&h, true, 300, cx);
    let retained = panel(&h, cx);
    let original = retained.read_with(cx, |panel, _| {
        panel.reviewed_for_test().checked_option("review")
    });
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        for language in [Language::ZhCn, Language::En] {
            let before = retained.read_with(cx, |panel, _| {
                panel.reviewed_for_test().checked_option("current review")
            });
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                let footer = window.find("workflow-footer").bounds();
                assert!(contained(footer, window.bounds()));
                for id in ["workflow-hide", "workflow-back", "workflow-confirm"] {
                    let item = window.find(id);
                    assert!(
                        item.visible() && contained(item.bounds(), footer),
                        "{theme:?}/{language:?}/{id}"
                    );
                }
                click_visible(window, "workflow-schedule-review", cx);
                assert!(window.find("workflow-schedule-review").visible());
                assert!(retained.read(cx).review_current(&before, cx));
                window.click("workflow-back", cx);
                for id in [
                    "workflow-schedule-now",
                    "workflow-schedule-once",
                    "workflow-schedule-interval",
                    "workflow-schedule-start",
                    "workflow-schedule-offset",
                    "workflow-schedule-grace",
                    "workflow-schedule-period",
                    "workflow-schedule-count",
                ] {
                    click_visible(window, id, cx);
                    let item = window.find(id);
                    assert!(
                        item.visible() && item.bounds().size.width > px(0.),
                        "{theme:?}/{language:?}/{id}"
                    );
                    // Restore interval after intentionally exercising mode controls.
                    if id == "workflow-schedule-now" || id == "workflow-schedule-once" {
                        click_visible(window, "workflow-schedule-interval", cx);
                    }
                }
                set_input(window, "workflow-schedule-count", "33", cx);
                window.render_frame(cx);
                window.click("workflow-review-button", cx);
                assert!(retained.read(cx).reviewed_for_test().is_none());
                set_input(window, "workflow-schedule-count", "3", cx);
            })
            .checked("minimum window typed scheduling and bilingual semantic themes");
            review_plan(&h, cx);
            let after = retained.read_with(cx, |panel, _| {
                panel.reviewed_for_test().checked_option("restored review")
            });
            let after = after.schedule.as_ref().checked_option("schedule");
            let original = original
                .schedule
                .as_ref()
                .checked_option("original schedule");
            assert_eq!(
                (
                    after.first_utc_seconds(),
                    after.interval_seconds(),
                    after.count(),
                    after.fixed_offset_minutes()
                ),
                (
                    original.first_utc_seconds(),
                    original.interval_seconds(),
                    original.count(),
                    original.fixed_offset_minutes()
                )
            );
        }
    }
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
}

#[gpui_kit::test]
async fn scheduled_workflow_bounded_interval_uses_two_real_clock_occurrences(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule(&h, true, 5, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-back", cx);
        set_input(window, "workflow-schedule-count", "2", cx);
    })
    .checked("edit exactly two sixty-second occurrences");
    review_plan(&h, cx);
    let review = panel(&h, cx).read_with(cx, |panel, _| {
        panel.reviewed_for_test().checked_option("finite review")
    });
    let expected = review.plan.tasks()[0].command.as_bytes().to_vec();
    h.confirm(cx);
    wait_real(&h, Duration::from_secs(10), cx, |_| {
        h.servers[0].requests().len() == 1
    })
    .await;
    assert!(
        panel(&h, cx).read_with(cx, |panel, _| panel.is_running()),
        "first receipt retains future authorized occurrence"
    );
    completed_real(&h, Duration::from_secs(65), cx).await;
    assert_eq!(h.servers[0].requests(), vec![expected.clone(), expected]);
    assert!(h.servers[1].requests().is_empty());
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn scheduled_workflow_confirmed_failure_stops_remaining_occurrences(cx: &mut TestAppContext) {
    use keelshell_core::{
        WorkflowScheduleInvalidationReason as Reason, WorkflowScheduleOutcome as Outcome,
        WorkflowScheduleSlotStatus as Slot,
    };
    let h = Harness::new(cx);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let failing = peer::Server::new(&runtime, 7);
    h.fixture.workspace.update(cx, |view, cx| {
        view.remote_sessions
            .insert(h.panes[0].terminal.entity_id(), failing.session.clone());
        cx.notify();
    });
    schedule_source(&h, true, 4, "fails", cx);
    h.confirm(cx);
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert_eq!(failing.requests(), vec![b"fails".to_vec()]);
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Finished(Outcome::Failed),
            Slot::Invalidated(Reason::PriorRunNotSucceeded),
            Slot::Invalidated(Reason::PriorRunNotSucceeded)
        ]
    );
}

#[gpui_kit::test]
async fn scheduled_workflow_cancel_during_running_retains_uncertain_remote_boundary(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{WorkflowScheduleOutcome as Outcome, WorkflowScheduleSlotStatus as Slot};
    let h = Harness::new(cx);
    schedule_source(&h, true, 4, "hold", cx);
    h.confirm(cx);
    wait_real(&h, Duration::from_secs(10), cx, |_| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-cancel", cx);
    })
    .checked("human cancellation of an admitted schedule occurrence");
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Finished(Outcome::Cancelled),
            Slot::Cancelled,
            Slot::Cancelled
        ]
    );
    // This peer holds the remote operation open. The receipt only proves local
    // cancellation, never remote termination or rollback of an acknowledged exec.
    assert!(h.servers[1].requests().is_empty());
}
