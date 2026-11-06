//! Local controlled SSH peers exercise real connection isolation, without an
//! external host or a desktop/native acceptance claim.
use super::*;
use crate::workspace::window_lifecycle::CloseIntent;

async fn connected(
    fixture: &Fixture,
    route: &RouteFixture,
    cx: &mut TestAppContext,
) -> Entity<TerminalView> {
    start_route(fixture, route, cx);
    for id in [route.gateway.id, route.target.id] {
        login_for(fixture, id, cx).await;
        submit_secret(fixture, id, cx);
    }
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let view = fixture.workspace.read(cx);
        !view.saving
            && !view.connecting
            && view.route_jobs == 0
            && view.tabs.first().is_some_and(|tab| {
                tab.read(cx).is_open() && tab.read(cx).visible_text().contains("TARGET READY")
            })
            && view.panels.values().all(|panels| {
                panels
                    .files
                    .as_ref()
                    .is_none_or(|panel| !panel.read(cx).window_close_busy())
                    && panels
                        .monitor
                        .as_ref()
                        .is_none_or(|panel| !panel.read(cx).window_close_busy(cx))
            })
    })
    .await;
    fixture
        .workspace
        .read_with(cx, |view, _| view.tabs[0].clone())
}

#[gpui_kit::test]
async fn multiwindow_close_rejects_format_only_drift_then_reaps_a_and_keeps_b_ssh_live(
    cx: &mut TestAppContext,
) {
    let (first, route) = mount_route(cx, false, true);
    let a_terminal = connected(&first, &route, cx).await;
    let second = super::super::windows::open_empty(&first, cx);
    let b_terminal = connected(&second, &route, cx).await;
    let (a_session, b_session) = cx.update(|cx| {
        (
            first.workspace.read(cx).remote_sessions[&a_terminal.entity_id()].clone(),
            second.workspace.read(cx).remote_sessions[&b_terminal.entity_id()].clone(),
        )
    });
    assert!(!a_session.same_connection(&b_session));
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            let files = view.panels[&a_terminal.entity_id()]
                .files
                .clone()
                .unwrap_or_else(|| panic!("real SSH file panel missing"));
            files.update(cx, |panel, cx| {
                panel.seed_draft_for_test("/fixture.txt", "original\n", "draft\n", window, cx)
            });
            view.request_window_close(CloseIntent::Window, window, cx);
            let ticket = view
                .close_review
                .as_ref()
                .map(|review| review.id)
                .unwrap_or_else(|| panic!("close review blocked: {}", view.status.render(cx)));
            let text = files.read(cx).draft_snapshot(cx);
            files.update(cx, |panel, _| {
                panel.set_window_close_format_for_test(b"\xef\xbb\xbforiginal\r\n")
            });
            assert_eq!(files.read(cx).draft_snapshot(cx), text);
            view.confirm_window_close(ticket, window, cx);
            assert!(view.close_review.is_none() && view.closing.is_none());
            assert!(!a_session.is_closed());
            view.request_window_close(CloseIntent::Window, window, cx);
            let ticket = view
                .close_review
                .as_ref()
                .map(|review| review.id)
                .unwrap_or_else(|| panic!("fresh close review missing"));
            view.confirm_window_close(ticket, window, cx);
            assert!(!a_terminal.read(cx).is_open());
        });
    })
    .checked("reject changed format with identical text, then approve fresh window close");
    cx.wait_for(second.window, Duration::from_secs(10), |_, cx| {
        second.workspace.read(cx).app_windows.read(cx).windows.len() == 1
    })
    .await;
    assert!(a_session.is_closed());
    assert!(!b_session.is_closed());
    assert!(!crate::terminal::application_is_quitting_for_test());
    cx.update(|cx| {
        b_terminal
            .update(cx, |terminal, _| {
                terminal.try_write(b"surviving-window-b\r".to_vec())
            })
            .checked("send real SSH input on surviving B");
    });
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        b_terminal
            .read(cx)
            .visible_text()
            .contains("surviving-window-b")
    })
    .await;
    let runtime = second
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let close = runtime.spawn(async move { b_session.close().await });
    let result =
        runtime.block_on(async { tokio::time::timeout(Duration::from_secs(3), close).await });
    assert!(matches!(result, Ok(Ok(Ok(())))));
    let owners = second.workspace.read_with(cx, |view, _| {
        view.worker_owners.iter().copied().collect::<Vec<_>>()
    });
    crate::terminal::stop_window_workers(
        &owners,
        std::time::Instant::now() + Duration::from_secs(3),
    )
    .checked("actually join remaining controlled SSH workers");
}
