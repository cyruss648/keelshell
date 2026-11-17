//! Raw wheel and pointer input reach production review controls after a full grant.
use super::*;
use crate::workspace::modal_scope::ModalKind;
use gpui_kit::{App, InputEvent, MouseButton, Pixels, Point, Window};

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
                                gpui_kit::ScrollEvent {
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
                                    gpui_kit::ScrollEvent {
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

const FILE_PATH: &str = "/approved/中文.txt";
const FILE_BASELINE: &str = "受控中文\n";

async fn full_grant_with_pending_files(
    record_count: usize,
    cx: &mut TestAppContext,
) -> (Harness, Vec<uuid::Uuid>) {
    let h = Harness::new(cx);
    let selection = "KeelShell controlled loopback fixture";
    // First-show terminal resize clears selection; capture only after the
    // actual SFTP pane has received its production layout.
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.active = 1;
            cx.notify();
        });
        window.render_frame(cx);
    })
    .checked("render the file proposal's actual SSH pane");
    h.panes[1].terminal.update(cx, |terminal, _| {
        terminal
            .emulator
            .feed(format!("\u{1b}[2J\u{1b}[H{selection}").as_bytes());
        terminal.emulator.start_selection(0, 0);
        terminal.emulator.update_selection(0, 36);
        assert_eq!(terminal.selected_text(), selection);
    });
    h.grant(
        1,
        &[
            ToolKind::ListSessions,
            ToolKind::ReadSelection,
            ToolKind::SftpList,
            ToolKind::SftpRead,
            ToolKind::MonitorSnapshot,
            ToolKind::ProposeCommand,
            ToolKind::ProposeFileChange,
            ToolKind::GetActionStatus,
        ],
        "/approved",
        cx,
    )
    .await;
    let target = h.target(cx).await;
    let metadata = h
        .call("keelshell_list_sessions", json!({}), cx)
        .await
        .checked("discover the complete grant used by the scrolling regression");
    assert_eq!(
        metadata["sessions"][0]["granted_roots"],
        json!(["/approved"])
    );
    let fragment: uuid::Uuid =
        serde_json::from_value(metadata["sessions"][0]["selection_ids"][0].clone())
            .checked("discover the explicitly captured fragment");
    let captured = h
        .call(
            "keelshell_read_selection",
            json!({"target":target,"selection_id":fragment}),
            cx,
        )
        .await
        .checked("read the full grant's real selected fragment");
    assert_eq!(captured["text"], selection);

    let mut actions = Vec::with_capacity(record_count);
    for index in 0..record_count {
        let reply = h
            .call(
                "keelshell_propose_file_change",
                json!({
                    "target":target,
                    "path":FILE_PATH,
                    "expected_sha256":keelshell_mcp::content_sha256(FILE_BASELINE),
                    "replacement":format!("scroll regression proposal {index:02}\n")
                        .repeat(if record_count == 1 { 120 } else { 1 }),
                }),
                cx,
            )
            .await
            .checked("prepare a real pending file proposal over isolated SFTP");
        assert_eq!(reply["kind"], "pending_file_change");
        let id: uuid::Uuid =
            serde_json::from_value(reply["action_id"].clone()).checked("pending file identity");
        assert!(!actions.contains(&id));
        h.fixture.workspace.read_with(cx, |view, _| {
            assert!(matches!(
                view.mcp_test_state(id),
                Some(ActionState::PendingReview)
            ));
        });
        actions.push(id);
    }
    if record_count == 32 {
        assert_eq!(
            h.call(
                "keelshell_propose_file_change",
                json!({
                    "target":target,
                    "path":FILE_PATH,
                    "expected_sha256":keelshell_mcp::content_sha256(FILE_BASELINE),
                    "replacement":"must not create record 33\n",
                }),
                cx,
            )
            .await,
            Err(McpFailure::Busy),
            "the rendered 32-record case must reach the real admission limit"
        );
    }
    (h, actions)
}

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

fn raw_wheel(position: Point<Pixels>, delta: ScrollDelta, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        gpui_kit::ScrollEvent {
            position,
            delta,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn raw_click(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let position = point(
        bounds.origin.x + bounds.size.width / 2.,
        bounds.origin.y + bounds.size.height / 2.,
    );
    for event in [
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
    }
    window.render_frame(cx);
}

fn raw_drag(from: Point<Pixels>, to: Point<Pixels>, window: &mut Window, cx: &mut App) {
    for event in [
        gpui_kit::MouseMoveEvent {
            position: from,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseDownEvent {
            position: from,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseMoveEvent {
            position: to,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseUpEvent {
            position: to,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
        window.render_frame(cx);
    }
}

struct ReviewScrollScenario<'a> {
    delta: ScrollDelta,
    gutter: bool,
    description: &'a str,
    long_diff: bool,
}

fn wheel_to_file_review(
    h: &Harness,
    action: uuid::Uuid,
    scenario: ReviewScrollScenario<'_>,
    window: &mut Window,
    cx: &mut App,
) {
    let ReviewScrollScenario {
        delta,
        gutter,
        description: scene,
        long_diff,
    } = scenario;
    let viewport = window.find("mcp-scroll").bounds();
    assert!(window.find("mcp-scrollbar").visible(), "{scene}");
    let dialog = window.find("mcp-dialog").bounds();
    let header = window.find("mcp-close").bounds();
    let footer = ["mcp-disable", "mcp-copy-launch", "mcp-grant-session"]
        .map(|id| (id, window.find(id).bounds()));
    for (id, bounds) in &footer {
        assert!(
            window.find(*id).visible()
                && contained(*bounds, dialog)
                && contained(*bounds, window.bounds())
                && bounds.origin.y >= viewport.bottom(),
            "{scene}: fixed footer {id} is outside the dialog: {bounds:?}"
        );
    }
    let fixed_chrome = |window: &mut Window| {
        assert_eq!(window.find("mcp-scroll").bounds(), viewport, "{scene}");
        assert_eq!(window.find("mcp-close").bounds(), header, "{scene}");
        for (id, bounds) in &footer {
            assert_eq!(
                window.find(*id).bounds(),
                *bounds,
                "{scene}: wheel moved fixed footer {id}"
            );
        }
    };
    let position = point(
        if gutter {
            viewport.origin.x + px(2.)
        } else {
            viewport.origin.x + viewport.size.width / 2.
        },
        viewport.origin.y + viewport.size.height / 2.,
    );
    // Use the native hit-test route, including an interior pointer position;
    // a directly targeted scroll helper can miss a clipped flex viewport.
    raw_wheel(
        position,
        ScrollDelta::Pixels(point(px(0.), px(100000.))),
        window,
        cx,
    );
    fixed_chrome(window);
    let button_id = format!("mcp-execute-{action}");
    let before = window.find(button_id.clone()).bounds();
    let body_before = window.find("mcp-clear-actions").bounds();
    assert!(
        before.bottom() > viewport.bottom(),
        "{scene}: fixture must begin with the review action clipped: \
         button={before:?}, viewport={viewport:?}"
    );

    raw_wheel(position, delta, window, cx);
    fixed_chrome(window);
    let after_first = window.find(button_id.clone()).bounds();
    let body_after = window.find("mcp-clear-actions").bounds();
    assert!(
        after_first.origin.y < before.origin.y && body_after.origin.y < body_before.origin.y,
        "{scene}: wheel did not move production body content: \
         button={before:?} -> {after_first:?}, body={body_before:?} -> {body_after:?}"
    );
    assert_eq!(
        after_first.size, before.size,
        "{scene}: wheel resized a card"
    );
    assert_eq!(body_after.size, body_before.size, "{scene}");

    // Small wheel steps must reach an enabled control; a bounded search also
    // catches middle cards being skipped while traversing a saturated history.
    for _ in 0..192 {
        if contained(window.find(button_id.clone()).bounds(), viewport) {
            break;
        }
        raw_wheel(position, delta, window, cx);
        fixed_chrome(window);
    }
    let button = window.find(button_id.clone());
    let reachable = button.bounds();
    assert!(
        button.visible() && contained(reachable, viewport) && contained(reachable, window.bounds()),
        "{scene}: review action remains unreachable after bounded wheel input: \
         {reachable:?} in {viewport:?}"
    );
    assert_ne!(button.disabled(), Some(true), "{scene}");
    assert_eq!(
        h.fixture.workspace.read(cx).active_modal(),
        Some(ModalKind::Mcp),
        "{scene}"
    );
    raw_click(reachable, window, cx);
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(
            view.active_modal(),
            Some(ModalKind::McpFileReview),
            "{scene}"
        );
        assert_eq!(
            view.mcp.reviewing,
            Some(action),
            "{scene}: wrong file opened"
        );
        assert!(matches!(
            view.mcp_test_state(action),
            Some(ActionState::PendingReview)
        ));
    });
    assert!(window.try_find("mcp-disable").is_none(), "{scene}");
    assert_eq!(
        window.find("mcp-file-review-path").label(),
        Some(FILE_PATH),
        "{scene}"
    );
    let review_footer = window.find("mcp-file-review-footer").bounds();
    assert!(
        window.find("mcp-file-review-scrollbar").visible(),
        "{scene}"
    );
    let diff = window.find("mcp-file-review-diff").bounds();
    let review_viewport = window.find("mcp-file-review-scroll").bounds();
    if long_diff {
        assert!(
            diff.size.height > review_viewport.size.height,
            "{scene}: the complete 120-line diff must overflow the review viewport"
        );
    }
    if diff.bottom() > review_viewport.bottom() {
        let bar = window.find("mcp-file-review-scrollbar").bounds();
        let before = window.find("mcp-file-review-diff").bounds();
        raw_drag(
            point(bar.right() - px(6.), bar.origin.y + px(6.)),
            point(bar.right() - px(6.), bar.bottom() - px(6.)),
            window,
            cx,
        );
        assert!(
            window.find("mcp-file-review-diff").bounds().origin.y < before.origin.y,
            "{scene}: dragging the visible review scrollbar must move the complete diff"
        );
        let end = window.find("mcp-file-review-diff").bounds().bottom();
        assert!(
            end > review_viewport.origin.y && end <= review_viewport.bottom(),
            "{scene}: dragging to the end must reveal the complete diff's final line: \
             end={end:?}, viewport={review_viewport:?}"
        );
        assert_eq!(
            window.find("mcp-file-review-footer").bounds(),
            review_footer,
            "{scene}: dragging the diff must keep its review actions fixed"
        );
    }
    let close = window.find("mcp-file-review-close");
    assert!(
        close.visible()
            && contained(close.bounds(), review_footer)
            && contained(close.bounds(), window.bounds()),
        "{scene}: keep-pending action is not reachable"
    );
    raw_click(close.bounds(), window, cx);
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.active_modal(), Some(ModalKind::Mcp), "{scene}");
        assert_eq!(view.mcp.reviewing, None, "{scene}");
    });
}

async fn file_proposal_scroll_matrix(record_count: usize, cx: &mut TestAppContext) {
    let (h, actions) = full_grant_with_pending_files(record_count, cx).await;
    let indices = match record_count {
        1 => vec![0],
        6 => vec![0, 5],
        32 => vec![15, 31],
        _ => panic!("unsupported scroll regression fixture"),
    };
    for family in crate::design::registry() {
        for language in [Language::ZhCn, Language::En] {
            for (width, height) in [(900., 580.), (1440., 900.)] {
                cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
                cx.update_window(h.fixture.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    let mut settings = h.fixture.workspace.read(cx).state.settings.clone();
                    settings.theme = family.preferred_appearance;
                    settings.theme_family =
                        keelshell_core::ThemeId::new(family.id).checked("registered MCP theme");
                    settings.language = language;
                    crate::design::apply_settings(&settings, Some(window), cx);
                    h.fixture.workspace.update(cx, |view, cx| {
                        view.state.settings = settings;
                        cx.notify();
                    });
                    window.render_frame(cx);
                    raw_wheel(
                        window.find("mcp-scroll").bounds().center(),
                        ScrollDelta::Pixels(point(px(0.), px(100000.))),
                        window,
                        cx,
                    );
                    let bar = window.find("mcp-scrollbar").bounds();
                    let footer = window.find("mcp-disable").bounds();
                    let body = window.find("mcp-clear-actions").bounds();
                    raw_drag(
                        point(bar.right() - px(6.), bar.origin.y + px(6.)),
                        point(bar.right() - px(6.), bar.bottom() - px(6.)),
                        window,
                        cx,
                    );
                    assert!(
                        window.find("mcp-clear-actions").bounds().origin.y < body.origin.y,
                        "{} {language:?} {width}x{height}: dragging the MCP scrollbar \
                         must move production content",
                        family.id
                    );
                    assert_eq!(window.find("mcp-disable").bounds(), footer);
                    for (delta, gutter) in [
                        (ScrollDelta::Pixels(point(px(0.), px(-120.))), false),
                        (ScrollDelta::Lines(point(0., -6.)), true),
                    ] {
                        for &index in &indices {
                            let scene = format!(
                                "{} {language:?} {width}x{height}, {record_count} records, \
                                 index={index}, delta={delta:?}, gutter={gutter}",
                                family.id
                            );
                            wheel_to_file_review(
                                &h,
                                actions[index],
                                ReviewScrollScenario {
                                    delta,
                                    gutter,
                                    description: &scene,
                                    long_diff: record_count == 1,
                                },
                                window,
                                cx,
                            );
                        }
                    }
                })
                .checked("reach actual pending file reviews by raw wheel and pointer input");
            }
        }
    }
    h.fixture.workspace.read_with(cx, |view, _| {
        for id in actions {
            assert!(matches!(
                view.mcp_test_state(id),
                Some(ActionState::PendingReview)
            ));
        }
    });
    assert!(h.command.requests().is_empty());
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes,
        "opening and closing a reachable review cannot approve a file mutation"
    );
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn first_pending_file_with_full_grant_scrolls_to_clickable_review(cx: &mut TestAppContext) {
    file_proposal_scroll_matrix(1, cx).await;
}

#[gpui_kit::test]
async fn multiple_pending_files_scroll_to_first_and_last_clickable_reviews(
    cx: &mut TestAppContext,
) {
    file_proposal_scroll_matrix(6, cx).await;
}

#[gpui_kit::test]
async fn saturated_pending_files_scroll_to_middle_and_last_clickable_reviews(
    cx: &mut TestAppContext,
) {
    file_proposal_scroll_matrix(32, cx).await;
}
