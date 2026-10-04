//! Review controls remain reachable when a short proposal follows a full grant.
use super::*;
use gpui_kit::InputEvent;

#[gpui_kit::test]
async fn first_short_proposal_with_selection_and_directory_has_reachable_review(
    cx: &mut TestAppContext,
) {
    for monitor in [false, true] {
        let h = Harness::new(cx);
        let selection = "KeelShell controlled loopback fixture";
        assert_eq!(selection.len(), 37);

        // The SFTP fixture is the second pane. Render it before selection so
        // first-show terminal resize cannot invalidate the captured snapshot.
        cx.update_window(h.fixture.window, |_, window, cx| {
            h.fixture.workspace.update(cx, |view, cx| {
                view.active = 1;
                cx.notify();
            });
            window.render_frame(cx);
        })
        .checked("render the SFTP fixture pane before selection");
        h.panes[1].terminal.update(cx, |terminal, _| {
            terminal
                .emulator
                .feed(format!("\u{1b}[2J\u{1b}[H{selection}").as_bytes());
            terminal.emulator.start_selection(0, 0);
            terminal.emulator.update_selection(0, 36);
            assert_eq!(terminal.selected_text(), selection);
        });
        let mut tools = vec![
            ToolKind::ListSessions,
            ToolKind::ReadSelection,
            ToolKind::SftpList,
            ToolKind::SftpRead,
            ToolKind::ProposeCommand,
            ToolKind::GetActionStatus,
        ];
        if monitor {
            tools.push(ToolKind::MonitorSnapshot);
        }
        h.grant(1, &tools, "/approved", cx).await;
        let target = h.target(cx).await;
        let metadata = h
            .call("keelshell_list_sessions", json!({}), cx)
            .await
            .checked("discover the captured fragment and validated root");
        assert_eq!(
            metadata["sessions"][0]["granted_roots"],
            json!(["/approved"])
        );
        let fragment: uuid::Uuid =
            serde_json::from_value(metadata["sessions"][0]["selection_ids"][0].clone())
                .checked("read the exact granted fragment UUID");
        let captured = h
            .call(
                "keelshell_read_selection",
                json!({"target":target,"selection_id":fragment}),
                cx,
            )
            .await
            .checked("read the actual captured fragment");
        assert_eq!(captured["text"], selection);
        let action = h.propose(target, "printf 'first proposal\\n'", cx).await;

        for language in [Language::ZhCn, Language::En] {
            for (width, height) in [(1440., 900.), (900., 580.)] {
                cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
                cx.update_window(h.fixture.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    window.render_frame(cx);
                    let viewport = window.find("mcp-scroll").bounds();
                    let footer = window.find("mcp-disable").bounds();
                    for (reset, down) in [
                        (
                            ScrollDelta::Pixels(point(px(0.), px(10000.))),
                            ScrollDelta::Pixels(point(px(0.), px(-120.))),
                        ),
                        (
                            ScrollDelta::Lines(point(0., 1000.)),
                            ScrollDelta::Lines(point(0., -6.)),
                        ),
                    ] {
                        for position in [
                            point(
                                viewport.origin.x + viewport.size.width / 2.,
                                viewport.origin.y + viewport.size.height / 2.,
                            ),
                            point(
                                viewport.origin.x + px(2.),
                                viewport.origin.y + viewport.size.height / 2.,
                            ),
                        ] {
                            // Dispatch platform events at real coordinates;
                            // targeting a scroll element directly could hide
                            // hit-test or event-propagation regressions.
                            window.dispatch_event(
                                gpui_kit::MouseMoveEvent {
                                    position,
                                    ..Default::default()
                                }
                                .to_platform_input(),
                                cx,
                            );
                            window.dispatch_event(
                                gpui_kit::ScrollWheelEvent {
                                    position,
                                    delta: reset,
                                    ..Default::default()
                                }
                                .to_platform_input(),
                                cx,
                            );
                            window.render_frame(cx);
                            let before = window.find(format!("mcp-execute-{action}")).bounds();
                            for _ in 0..8 {
                                window.dispatch_event(
                                    gpui_kit::ScrollWheelEvent {
                                        position,
                                        delta: down,
                                        ..Default::default()
                                    }
                                    .to_platform_input(),
                                    cx,
                                );
                                window.render_frame(cx);
                            }
                            let button = window.find(format!("mcp-execute-{action}"));
                            let after = button.bounds();
                            assert!(button.visible());
                            assert!(
                                after.origin.y < before.origin.y
                                    && after.origin.y >= viewport.origin.y
                                    && after.bottom() <= viewport.bottom(),
                                "{language:?} {width}x{height}, monitor={monitor}, \
                                 pointer={position:?}, delta={down:?}: \
                                 before={before:?}, after={after:?}, viewport={viewport:?}"
                            );
                            assert_eq!(window.find("mcp-disable").bounds(), footer);
                        }
                    }
                })
                .checked("reach the first short proposal with raw wheel events");
            }
        }
        assert!(matches!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_state(action)),
            Some(ActionState::PendingReview)
        ));
        assert!(h.command.requests().is_empty());
    }
}
