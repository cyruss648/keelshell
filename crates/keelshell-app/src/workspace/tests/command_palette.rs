use super::*;
use crate::workspace::OpenCommandPalette;
use gpui_kit::Pixels;

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

fn overlaps(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x < outer.right()
        && inner.right() > outer.origin.x
        && inner.origin.y < outer.bottom()
        && inner.bottom() > outer.origin.y
}

fn open_palette(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.open_command_palette(&OpenCommandPalette, window, cx);
            assert!(workspace.show_command_palette);
        });
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
    })
    .checked("prepare command palette action");
}

fn search_palette(fixture: &Fixture, query: &str, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace
                .command_palette_query
                .update(cx, |input, cx| input.set_value(query, window, cx));
        });
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
    })
    .checked("search command palette action");
}

#[gpui_kit::test]
fn compact_english_quick_connect_scroll_keeps_connect_action_reachable(cx: &mut TestAppContext) {
    for height in [580., 900.] {
        let fixture = mount_sized(
            cx,
            vec![Connection::new("审核主机", "192.0.2.10", "operator")],
            480.,
            height,
        );
        cx.update_window(fixture.window, |_, window, cx| {
            i18n::set_language(Language::En, cx);
            window.render_frame(cx);
            let surface = window.find("quick-connect-surface").bounds();
            assert!(
                contained(surface, window.bounds()),
                "480x{height}: quick-connect surface escaped compact window: {surface:?}"
            );
            let library = window.find("connection-table-scroll");
            assert!(
                library.visible()
                    && library.bounds().size.height >= px(48.)
                    && contained(library.bounds(), window.bounds()),
                "480x{height}: quick-connect surface consumed the connection library viewport: surface={surface:?}, library={:?}, connection-library={:?}, command-column={:?}, session-strip={:?}, tool-tabs={:?}, window={:?}",
                library.bounds(),
                window.find("connection-library").bounds(),
                window.find("command-column").bounds(),
                window.find("session-strip").bounds(),
                window.find("workspace-tool-tabs").bounds(),
                window.bounds()
            );
            let rows = window.find("connection-rows");
            assert!(
                rows.visible()
                    && rows.role() == Some(gpui_kit::Role::List)
                    && contained(rows.bounds(), library.bounds()),
                "480x{height}: connection rows are not reachable inside the library viewport: rows={:?}, library={:?}",
                rows.bounds(),
                library.bounds()
            );
            let first_row = window.find(("connection-row", 0_usize));
            assert!(
                first_row.visible() && overlaps(first_row.bounds(), rows.bounds()),
                "480x{height}: first saved connection has no visible list intersection: row={:?}, rows={:?}",
                first_row.bounds(),
                rows.bounds()
            );
            window.scroll(
                "connection-rows",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-60.))),
                cx,
            );
            window.render_frame(cx);
            let saved_connect = window.find(("connect", 0_usize));
            assert!(
                saved_connect.visible() && contained(saved_connect.bounds(), rows.bounds()),
                "480x{height}: saved connection Connect action is not reachable after list scroll: action={:?}, rows={:?}",
                saved_connect.bounds(),
                rows.bounds()
            );
            let before_connect = window.find("quick-connect").bounds();
            window.scroll(
                "quick-connect-surface",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-100.))),
                cx,
            );
            window.render_frame(cx);
            assert_eq!(
                window.find("quick-connect-surface").bounds(),
                surface,
                "480x{height}: quick-connect wheel changed the compact surface bounds"
            );
            assert_eq!(
                window.find("connection-table-scroll").bounds(),
                library.bounds(),
                "480x{height}: quick-connect wheel changed the sibling connection library bounds"
            );
            let connect = window.find("quick-connect");
            assert!(
                connect.visible() && contained(connect.bounds(), surface),
                "480x{height}: Connect action is not reachable after scrolling compact quick-connect: before={before_connect:?}, after={:?} in {surface:?}",
                connect.bounds()
            );
            assert_eq!(connect.label(), Some("Connect"));
            i18n::set_language(Language::ZhCn, cx);
        })
        .checked("compact English quick-connect scroll reaches Connect");
    }
}

#[gpui_kit::test]
fn command_palette_is_bilingual_and_restores_focus_on_escape(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    open_palette(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let modal = window
            .try_find("command-palette-modal")
            .unwrap_or_else(|| panic!("command palette root is absent"));
        assert!(modal.visible());
        assert!(window.find("command-palette-input").visible());
        assert!(
            window
                .find("command-palette-item-open-connections")
                .label()
                .checked_option("first command palette row")
                .contains("打开连接管理器")
        );

        i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        assert!(
            window
                .find("command-palette-item-open-connections")
                .label()
                .checked_option("English command palette row")
                .contains("Open connection manager")
        );

        window.press("escape", cx);
        window.render_frame(cx);
        assert!(!fixture.workspace.read(cx).show_command_palette);
        assert!(
            fixture
                .workspace
                .read(cx)
                .surface_focus
                .contains_focused(window, cx)
        );
        assert!(window.try_find("command-palette-modal").is_none());
        i18n::set_language(Language::ZhCn, cx);
    })
    .checked("render bilingual command palette and restore after escape");
}

#[gpui_kit::test]
fn command_palette_enter_reuses_connection_manager_review_path(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    open_palette(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.press("enter", cx);
        window.render_frame(cx);
        assert!(!fixture.workspace.read(cx).show_command_palette);
        assert!(fixture.workspace.read(cx).show_connections);
        assert!(window.find("connection-manager-dialog").visible());
    })
    .checked("dispatch palette action through existing connection manager path");
}

#[gpui_kit::test]
fn command_palette_quick_connect_opens_the_one_time_connection_surface(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    open_palette(&fixture, cx);
    search_palette(&fixture, "快速连接", cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.press("enter", cx);
        window.render_frame(cx);
        assert!(!fixture.workspace.read(cx).show_command_palette);
        assert!(fixture.workspace.read(cx).show_quick_connect);
        assert!(window.find("quick-connect-surface").visible());
        assert!(window.find("quick-host").visible());
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(!fixture.workspace.read(cx).show_quick_connect);
    })
    .checked("route quick connect to the one-time connection modal");
}

#[gpui_kit::test]
fn command_palette_host_monitor_selects_the_monitor_navigation_panel(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let _panes = attach_remote_panes(&fixture, cx);
    open_palette(&fixture, cx);
    search_palette(&fixture, "主机监控", cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.press("enter", cx);
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.show_navigation);
        assert_eq!(
            workspace.navigation_panel,
            crate::workspace::navigation::NavigationPanel::Monitor
        );
        assert!(window.try_find("navigation-rows").is_none());
        assert!(window.find("navigation-monitor").visible());
    })
    .checked("route host monitoring to the monitor navigation panel");
}

#[gpui_kit::test]
fn command_palette_session_actions_follow_first_and_last_tab_boundaries(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let _panes = attach_remote_panes(&fixture, cx);
    open_palette(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(
            window
                .try_find("command-palette-item-next-session")
                .is_some()
        );
        assert!(
            window
                .try_find("command-palette-item-previous-session")
                .is_none()
        );
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.active = 1;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(
            window
                .try_find("command-palette-item-previous-session")
                .is_some()
        );
        assert!(
            window
                .try_find("command-palette-item-next-session")
                .is_none()
        );
    })
    .checked("hide unavailable session navigation actions at tab boundaries");
}

#[gpui_kit::test]
fn command_palette_opens_from_the_platform_shortcut(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.surface_focus.focus(window, cx);
        });
        let shortcut = if cfg!(target_os = "macos") {
            "cmd-k"
        } else {
            "ctrl-k"
        };
        window.press(shortcut, cx);
        assert!(fixture.workspace.read(cx).show_command_palette);
        assert!(window.find("command-palette-input").visible());
    })
    .checked("dispatch the platform command palette shortcut");
}

#[gpui_kit::test]
fn command_palette_card_and_results_fit_the_narrow_modal_surface(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    open_palette(&fixture, cx);
    cx.update_window(fixture.window, |_, window, _cx| {
        let surface = window.find("workspace-modal-trap").bounds();
        let card = window.find("command-palette-card");
        let results = window.find("command-palette-results");
        let input = window.find("command-palette-input");

        assert!(card.visible());
        assert!(card.bounds().left() >= surface.left());
        assert!(card.bounds().top() >= surface.top());
        assert!(card.bounds().right() <= surface.right());
        assert!(card.bounds().bottom() <= surface.bottom());
        assert!(results.visible());
        assert!(results.bounds().size.height > px(0.));
        assert!(input.visible());
    })
    .checked("keep command palette content inside the 900 by 580 modal");
}

#[gpui_kit::test]
fn command_palette_scrolls_keyboard_selection_into_view_when_results_overflow(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let _panes = attach_remote_panes(&fixture, cx);
    open_palette(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for _ in 0..11 {
            window.press("down", cx);
        }
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);

        let results = window.find("command-palette-results").bounds();
        let selected = window.find("command-palette-item-vault-settings").bounds();
        assert_eq!(fixture.workspace.read(cx).command_palette_selected, 11);
        assert!(
            selected.top() >= results.top(),
            "selected command row should be visible above the results viewport"
        );
        assert!(
            selected.bottom() <= results.bottom(),
            "selected command row should be visible below the results viewport"
        );
    })
    .checked("keep the keyboard-selected command visible in an overflowing result list");
}
