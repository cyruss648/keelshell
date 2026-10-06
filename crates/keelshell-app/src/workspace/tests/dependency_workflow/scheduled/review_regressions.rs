//! Non-author regressions through production workspace approval and SSH dispatch.
use super::*;
use keelshell_core::{
    WorkflowScheduleInvalidationReason as Reason, WorkflowScheduleOutcome as Outcome,
    WorkflowScheduleSlotStatus as Slot,
};

#[gpui_kit::test]
async fn independent_schedule_delayed_human_approval_never_arms_a_past_first_occurrence(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule(&h, false, 2, cx);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    real_pause(&runtime, Duration::from_secs(3), cx).await;
    h.confirm(cx);
    assert!(!panel(&h, cx).read_with(cx, |panel, _| panel.is_running()));
    assert!(
        panel(&h, cx)
            .read_with(cx, |panel, _| panel.schedule_status_for_test())
            .is_empty()
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
}

#[gpui_kit::test]
async fn independent_schedule_unknown_timeout_stops_all_future_occurrences(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule_source(&h, true, 5, "hold", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-back", cx);
        set_input(window, "workflow-timeout", "1", cx);
    })
    .checked("explicit bounded task timeout before review");
    review_plan(&h, cx);
    h.confirm(cx);
    completed_real(&h, Duration::from_secs(12), cx).await;
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Finished(Outcome::Unknown),
            Slot::Invalidated(Reason::PriorRunNotSucceeded),
            Slot::Invalidated(Reason::PriorRunNotSucceeded),
        ]
    );
    assert!(h.servers[1].requests().is_empty());
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn independent_schedule_hidden_endpoint_metadata_change_with_same_connection_stops_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule(&h, false, 4, cx);
    h.confirm(cx);
    h.fixture.workspace.update(cx, |view, cx| {
        view.show_workflow = false;
        view.remote_hosts.insert(
            h.panes[0].terminal.entity_id(),
            "changed@metadata.invalid:23".into(),
        );
        view.maintain_workflow(cx);
    });
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Invalidated(Reason::SessionBindingChanged),]
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
}

#[gpui_kit::test]
async fn independent_schedule_silent_count_change_while_armed_cannot_extend_reviewed_sequence(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    schedule(&h, true, 4, cx);
    let count = cx
        .update_window(h.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("workflow-back", cx);
            set_input(window, "workflow-schedule-count", "3", cx);
            match window
                .focused_input(cx)
                .checked_option("focused reviewed count")
            {
                AnyInputState::Input(field) => field,
                _ => panic!("schedule count is an InputState"),
            }
        })
        .checked("retain exact count input entity before review");
    review_plan(&h, cx);
    h.confirm(cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, _| view.show_workflow = false);
        // set_value emits no InputEvent; the complete snapshot must catch this.
        count.update(cx, |field, cx| field.set_value("4", window, cx));
    })
    .checked("silent extension of an armed hidden schedule");
    completed_real(&h, Duration::from_secs(10), cx).await;
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![
            Slot::Invalidated(Reason::SessionBindingChanged),
            Slot::Invalidated(Reason::SessionBindingChanged),
            Slot::Invalidated(Reason::SessionBindingChanged),
        ]
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
}
