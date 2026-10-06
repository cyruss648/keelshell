//! Author combination probes: actual parameter controls and real timed SSH admission.
use super::*;
use gpui_kit::component::input::TextareaState;
use keelshell_core::{
    WorkflowScheduleInvalidationReason as Reason, WorkflowScheduleOutcome as Outcome,
    WorkflowScheduleSlotStatus as Slot,
};

const VALUE: &str = "release 中文'v2\nline";
const SOURCE: &str = "printf '%s\\n' {{release}} {{endpoint}}";

fn panel_app(h: &Harness, cx: &App) -> Entity<crate::workflow_commands::WorkflowPanel> {
    h.fixture
        .workspace
        .read(cx)
        .workflow_panel
        .clone()
        .checked_option("current production panel")
}

struct Prepared {
    value: Entity<TextareaState>,
    target: uuid::Uuid,
    review: crate::workflow_commands::WorkflowReview,
    expected: Vec<u8>,
}

fn prepare_parameters(h: &Harness, count: u32, cx: &mut TestAppContext) -> Prepared {
    prepare_parameters_after(h, count, 5, cx)
}

fn prepare_parameters_after(
    h: &Harness,
    count: u32,
    lead_seconds: i64,
    cx: &mut TestAppContext,
) -> Prepared {
    cx.simulate_window_resize(h.fixture.window, size(px(900.), px(580.)));
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.set_reviewed_command(
                SOURCE.into(),
                Some(h.panes[0].terminal.entity_id()),
                window,
                cx,
            );
            view.open_workflow(false, window, cx);
        });
        click_visible(window, ("workflow-target", 0_usize), cx);
    })
    .checked("open production parameter workflow and select authenticated target");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "workflow-sync-parameters", cx);
    })
    .checked("explicit actual parameter union synchronization");
    cx.run_until_parked();
    let (target, endpoint) = panel(h, cx).read_with(cx, |panel, _| {
        let destination = &panel
            .connected_destinations()
            .next()
            .checked_option("captured original target")
            .destination;
        (destination.id, destination.endpoint.clone())
    });
    let value = cx
        .update_window(h.fixture.window, |_, window, cx| {
            click_visible(window, format!("workflow-parameters-{target}-release"), cx);
            let AnyInputState::Textarea(field) = window
                .focused_input(cx)
                .checked_option("actual parameter keyboard focus")
            else {
                panic!("user parameter is the focused multiline input");
            };
            field.update(cx, |input, cx| {
                input.set_selected_range(0..input.value().len(), cx);
                input.replace(VALUE.to_owned(), window, cx);
            });
            click_visible(
                window,
                if count == 1 {
                    "workflow-schedule-once"
                } else {
                    "workflow-schedule-interval"
                },
                cx,
            );
            set_input(window, "workflow-schedule-offset", "+00:00", cx);
            let date =
                format_fixed_offset_datetime(chrono::Utc::now().timestamp() + lead_seconds, 0)
                    .checked("future fixed UTC date");
            set_input(window, "workflow-schedule-start", &date, cx);
            set_input(window, "workflow-schedule-grace", "15", cx);
            if count > 1 {
                set_input(window, "workflow-schedule-period", "60", cx);
                set_input(window, "workflow-schedule-count", &count.to_string(), cx);
            }
            field
        })
        .checked("fill focused actual CJK apostrophe newline parameter and bounded schedule");
    review_plan(h, cx);
    let review = panel(h, cx).read_with(cx, |panel, _| {
        panel
            .reviewed_for_test()
            .checked_option("complete parameter and schedule review")
    });
    // Independent exact shell-literal vector, not a command copied from the review.
    let expected = format!("printf '%s\\n' 'release 中文'\\''v2\nline' '{endpoint}'").into_bytes();
    assert_eq!(review.plan.tasks()[0].command.as_bytes(), expected);
    assert_eq!(
        review.schedule.as_ref().checked_option("schedule").count(),
        count
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    Prepared {
        value,
        target,
        review,
        expected,
    }
}

/// Pair real monotonic elapsed with the original second absolute UTC deadline
/// before confirmation. Later layout work cannot shorten this observation.
fn second_deadline(p: &Prepared) -> (std::time::Instant, Duration) {
    let due = p
        .review
        .schedule
        .as_ref()
        .checked_option("reviewed sequence")
        .scheduled_utc_seconds(1)
        .checked_option("original second occurrence")
        * 1000;
    let observed = std::time::Instant::now();
    let remaining = due - chrono::Utc::now().timestamp_millis();
    assert!(
        remaining >= 60_000,
        "original second deadline before confirmation"
    );
    // The production ledger permits at most two seconds of wall/monotonic drift.
    // Measure the entire authorization interval, including hide/reopen/layout work.
    (
        observed,
        Duration::from_millis(u64::try_from(remaining - 2000).checked("bounded UTC duration")),
    )
}

fn retained(h: &Harness, p: &Prepared, cx: &TestAppContext) {
    panel(h, cx).read_with(cx, |panel, cx| {
        let field = panel
            .parameter_for_test(p.target, "release")
            .checked_option("original parameter still attached to reviewed target");
        assert_eq!(field.entity_id(), p.value.entity_id());
        assert_eq!(field.read(cx).value(), VALUE);
        assert!(panel.reviewed_for_test().as_ref() == Some(&p.review));
    });
}

async fn first_receipt(h: &Harness, cx: &mut TestAppContext) {
    wait_real(h, Duration::from_secs(12), cx, |cx| {
        panel_app(h, cx).read(cx).schedule_status_for_test().first()
            == Some(&Slot::Finished(Outcome::Succeeded))
    })
    .await;
}

fn no_persistence(h: &Harness, cx: &TestAppContext) {
    let stored = h
        .fixture
        .store
        .load()
        .checked("read actual private persistent state");
    assert!(stored.batch_audits.is_empty());
    let persisted = std::fs::read(h.fixture._state_directory.0.join("state.json"))
        .checked("read complete actual persisted state bytes");
    let in_memory = h.fixture.workspace.read_with(cx, |view, _| {
        assert!(view.command_histories.is_empty());
        assert!(view.pending_batch_audits.is_empty());
        assert!(view.state.batch_audits.is_empty());
        serde_json::to_vec(&view.state).checked("serialize exact persistent-state model")
    });
    for bytes in [persisted, in_memory] {
        let text = String::from_utf8(bytes).checked("UTF8 state body");
        for sentinel in [
            "release 中文",
            "printf '%s",
            "fixture stdout",
            "fixture stderr",
        ] {
            assert!(
                !text.contains(sentinel),
                "no value, expanded command or output persisted"
            );
        }
    }
    assert!(
        h.panes.iter().all(|pane| writes(pane).is_empty()),
        "exec never writes PTY"
    );
}

#[gpui_kit::test]
async fn scheduled_parameters_two_real_occurrences_keep_value_review_and_visible_latest_output(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let p = prepare_parameters(&h, 2, cx);
    let original_panel = panel(&h, cx).entity_id();
    let (observed, minimum_elapsed) = second_deadline(&p);
    h.confirm(cx);
    first_receipt(&h, cx).await;
    assert_eq!(h.servers[0].requests(), vec![p.expected.clone()]);
    assert!(panel(&h, cx).read_with(cx, |panel, _| panel.is_running()));
    retained(&h, &p, cx);
    no_persistence(&h, cx);
    // The first complete receipt must remain inspectable while waiting for the next due time.
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("workflow-output").is_some(),
            "successful first receipt output must be reachable between timed occurrences"
        );
        click_visible(window, "workflow-output", cx);
        assert!(window.find("workflow-output").visible());
        window.click("workflow-hide", cx);
    })
    .checked("first receipt actual output and hide control");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!h.fixture.workspace.read(cx).show_workflow);
        assert!(
            window
                .find(("terminal-pane", h.panes[0].terminal.entity_id()))
                .visible()
        );
        window.click("command-workflow", cx);
    })
    .checked("actual retained workflow reopen through workspace action");
    cx.run_until_parked();
    assert_eq!(panel(&h, cx).entity_id(), original_panel);
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                click_visible(window, "workflow-output", cx);
                assert!(window.find("workflow-output").visible());
                let footer = window.find("workflow-footer").bounds();
                assert!(footer.origin.y >= px(0.) && footer.bottom() <= px(580.));
            })
            .checked("minimum bilingual and theme matrix keeps latest output reachable");
            retained(&h, &p, cx);
        }
    }
    completed_real(&h, Duration::from_secs(70), cx).await;
    assert!(
        observed.elapsed() >= minimum_elapsed,
        "real 60-second schedule interval"
    );
    assert_eq!(
        h.servers[0].requests(),
        vec![p.expected.clone(), p.expected.clone()]
    );
    assert!(h.servers[1].requests().is_empty());
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Finished(Outcome::Succeeded),
            Slot::Finished(Outcome::Succeeded)
        ]
    );
    retained(&h, &p, cx);
    no_persistence(&h, cx);
}

#[gpui_kit::test]
async fn scheduled_parameters_silent_value_edit_after_first_receipt_refuses_next_real_due(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let p = prepare_parameters(&h, 2, cx);
    let (observed, minimum_elapsed) = second_deadline(&p);
    h.confirm(cx);
    first_receipt(&h, cx).await;
    retained(&h, &p, cx);
    assert_eq!(h.servers[0].requests(), vec![p.expected.clone()]);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
        // set_value emits no InputEvent; waiting and due-time snapshots must read this field.
        p.value.update(cx, |field, cx| {
            field.set_value("changed 中文'\nno old authority", window, cx)
        });
        assert!(panel_app(&h, cx).read(cx).reviewed_for_test().as_ref() == Some(&p.review));
        assert!(!panel_app(&h, cx).read(cx).review_current(&p.review, cx));
    })
    .checked("silent actual retained Textarea edit after first complete receipt");
    cx.run_until_parked();
    completed_real(&h, Duration::from_secs(2), cx).await;
    assert_eq!(h.servers[0].requests(), vec![p.expected.clone()]);
    assert!(h.servers[1].requests().is_empty());
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Finished(Outcome::Succeeded),
            Slot::Invalidated(Reason::SessionBindingChanged)
        ]
    );
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    cx.update_window(h.fixture.window, |_, window, cx| {
        p.value
            .update(cx, |field, cx| field.set_value(VALUE, window, cx));
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(false, window, cx));
        window.render_frame(cx);
        assert!(
            window.try_find("workflow-confirm").is_none(),
            "restoring a value cannot revive ended authority"
        );
    })
    .checked("restoring original bytes and reopening cannot resurrect schedule");
    // Restoring the value before the original next occurrence cannot revive it.
    // Keep observing real wall time through that original absolute deadline.
    let original_due = p
        .review
        .schedule
        .as_ref()
        .checked_option("original sequence")
        .scheduled_utc_seconds(1)
        .checked_option("original second due")
        * 1000;
    let remaining = original_due + 350 - chrono::Utc::now().timestamp_millis();
    if remaining > 0 {
        real_pause(
            &runtime,
            Duration::from_millis(u64::try_from(remaining).checked("bounded original deadline")),
            cx,
        )
        .await;
    }
    cx.run_until_parked();
    assert!(
        observed.elapsed() >= minimum_elapsed,
        "second original real deadline reached"
    );
    assert_eq!(h.servers[0].requests(), vec![p.expected]);
    no_persistence(&h, cx);
}

async fn changed_binding(cx: &mut TestAppContext, metadata: bool) {
    let h = Harness::new(cx);
    let p = prepare_parameters(&h, 1, cx);
    h.confirm(cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
        h.fixture.workspace.update(cx, |view, _| {
            if metadata {
                view.remote_hosts.insert(
                    h.panes[0].terminal.entity_id(),
                    "changed@metadata.invalid:23".into(),
                );
            } else {
                view.remote_sessions.insert(
                    h.panes[0].terminal.entity_id(),
                    h.servers[1].session.clone(),
                );
            }
            // No manual maintain/tick: the production deadline must re-read current authority.
        });
    })
    .checked("hidden scheduled parameter plan loses metadata or original connection authority");
    cx.run_until_parked();
    completed_real(&h, Duration::from_secs(12), cx).await;
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Invalidated(Reason::SessionBindingChanged)]
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    assert!(
        panel(&h, cx).read_with(cx, |panel, _| panel
            .parameter_for_test(p.target, "release")
            .is_none()),
        "old per-target values withdrawn after connection or metadata invalidation"
    );
    no_persistence(&h, cx);
}

#[gpui_kit::test]
async fn scheduled_parameters_original_connection_replacement_cannot_inherit_authority(
    cx: &mut TestAppContext,
) {
    changed_binding(cx, false).await;
}

#[gpui_kit::test]
async fn scheduled_parameters_same_connection_metadata_change_cannot_inherit_authority(
    cx: &mut TestAppContext,
) {
    changed_binding(cx, true).await;
}

mod sync_combination;
