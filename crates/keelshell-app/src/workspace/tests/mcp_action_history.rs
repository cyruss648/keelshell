//! History cleanup and explicit acknowledgements keep uncertain actions distinct.
use super::*;
use crate::workspace::modal_scope::ModalKind;
use gpui_kit::ElementId;

async fn held_unknown(h: &Harness, cx: &mut TestAppContext) -> uuid::Uuid {
    h.grant(
        0,
        &[
            ToolKind::ListSessions,
            ToolKind::ProposeCommand,
            ToolKind::GetActionStatus,
        ],
        "",
        cx,
    )
    .await;
    let target = h.target(cx).await;
    let before = h.command.requests().len();
    let id = h.propose(target, "hold", cx).await;
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        h.command.requests().len() > before
    })
    .await;
    h.grant(
        0,
        &[
            ToolKind::ListSessions,
            ToolKind::ProposeCommand,
            ToolKind::GetActionStatus,
        ],
        "",
        cx,
    )
    .await;
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::OutcomeUnknown)
    ));
    id
}

fn click_in_history(h: &Harness, id: impl Into<ElementId>, cx: &mut TestAppContext) {
    let id = id.into();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find(id.clone()).bounds();
        let viewport = window.find("mcp-scroll").bounds();
        window.scroll(
            "mcp-scroll",
            ScrollDelta::Pixels(point(
                px(0.),
                viewport.origin.y + viewport.size.height / 2.
                    - bounds.origin.y
                    - bounds.size.height / 2.,
            )),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find(id.clone()).visible());
        window.click(id, cx);
        window.render_frame(cx);
    })
    .checked("click the reachable history control");
}

#[gpui_kit::test]
async fn mcp_history_clear_preserves_unknown_pending_and_window_close_hold(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let unknown = held_unknown(&h, cx).await;
    let target = h.target(cx).await;
    let pending = h.propose(target, "must remain pending", cx).await;
    let rejected = h.propose(target, "must not execute", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.review_mcp_action(rejected, false, cx);
        // A direct/stale confirmation without an open review has no effect.
        view.acknowledge_mcp_unknown(unknown, cx);
        assert_eq!(view.mcp_test_unknown_acknowledged(unknown), Some(false));
    });
    click_in_history(&h, "mcp-clear-actions", cx);
    h.fixture.workspace.read_with(cx, |view, _| {
        assert!(view.mcp_test_state(rejected).is_none());
        assert!(matches!(
            view.mcp_test_state(pending),
            Some(ActionState::PendingReview)
        ));
        assert!(matches!(
            view.mcp_test_state(unknown),
            Some(ActionState::OutcomeUnknown)
        ));
        assert_eq!(view.mcp_test_unknown_acknowledged(unknown), Some(false));
    });
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_disable());
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read(cx).mcp_test_workers_finished()
    })
    .await;
    click_in_history(&h, "mcp-clear-actions", cx);
    h.fixture.workspace.read_with(cx, |view, _| {
        assert!(view.mcp_test_state(pending).is_none());
        assert!(matches!(
            view.mcp_test_state(unknown),
            Some(ActionState::OutcomeUnknown)
        ));
        assert!(!view.mcp.window_close_idle());
    });
    assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
}

#[gpui_kit::test]
async fn mcp_unknown_review_cancel_and_confirm_preserve_original_state_without_replay(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let id = held_unknown(&h, cx).await;
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_disable());
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read(cx).mcp_test_workers_finished()
    })
    .await;
    click_in_history(&h, format!("mcp-unknown-review-{id}"), cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        assert_eq!(
            h.fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::McpUnknownReview)
        );
        assert!(window.try_find("mcp-disable").is_none());
        window.press("escape", cx);
        window.render_frame(cx);
        assert_eq!(
            h.fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::Mcp)
        );
    })
    .checked("escape leaves the unknown acknowledgement unconsumed");
    click_in_history(&h, format!("mcp-unknown-review-{id}"), cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-unknown-review-cancel", cx);
        window.render_frame(cx);
    })
    .checked("cancel keeps the close hold");
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.mcp_test_unknown_acknowledged(id), Some(false));
        assert!(!view.mcp.window_close_idle());
    });
    click_in_history(&h, format!("mcp-unknown-review-{id}"), cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        assert!(window.find("mcp-unknown-review-identity").label().is_some());
        window.click("mcp-unknown-review-confirm", cx);
        window.render_frame(cx);
        assert_eq!(
            h.fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::Mcp)
        );
        assert_eq!(window.find("mcp-clear-actions").disabled(), Some(true));
    })
    .checked("explicitly acknowledge the exact unknown outcome");
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.mcp_test_unknown_acknowledged(id), Some(true));
        assert!(matches!(
            view.mcp_test_state(id),
            Some(ActionState::OutcomeUnknown)
        ));
        assert!(view.mcp.window_close_idle());
        assert_eq!(view.mcp_test_output(id), Some(""));
    });
    assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
}

#[gpui_kit::test]
async fn mcp_unknown_review_rejects_a_confirmation_for_another_action(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let first = held_unknown(&h, cx).await;
    let second = held_unknown(&h, cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.open_mcp_unknown_review(first, cx);
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
        window.render_frame(cx);
    })
    .checked("dismiss the first review");
    h.fixture.workspace.update(cx, |view, cx| {
        view.open_mcp_unknown_review(second, cx);
        view.acknowledge_mcp_unknown(first, cx);
        assert_eq!(view.mcp.unknown_reviewing, Some(second));
        assert_eq!(view.mcp_test_unknown_acknowledged(first), Some(false));
        assert_eq!(view.mcp_test_unknown_acknowledged(second), Some(false));
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("mcp-unknown-review-confirm", cx);
        window.render_frame(cx);
    })
    .checked("acknowledge only the currently reviewed action");
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.mcp_test_unknown_acknowledged(first), Some(false));
        assert_eq!(view.mcp_test_unknown_acknowledged(second), Some(true));
        assert!(matches!(
            view.mcp_test_state(first),
            Some(ActionState::OutcomeUnknown)
        ));
        assert!(matches!(
            view.mcp_test_state(second),
            Some(ActionState::OutcomeUnknown)
        ));
    });
    assert_eq!(
        h.command.requests(),
        vec![b"hold".to_vec(), b"hold".to_vec()]
    );
}
