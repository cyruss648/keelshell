//! Assert painted connection colors against real lifecycle and restored-layout states.
use super::*;
use crate::{i18n::Message, terminal::TransportState};
use gpui_kit::{App, ElementId, SharedString, Window, rgb};
use keelshell_core::ThemeId;
use keelshell_session::{ConnectionEnd, ShellEnd};
use tokio::sync::watch;

fn settle(window: &mut Window, cx: &mut App) {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
}

fn assert_indicator_color(
    window: &mut Window,
    id: impl Into<ElementId>,
    expected: u32,
    scene: &str,
) {
    let indicator = window.find(id);
    assert!(
        indicator.visible(),
        "{scene}: connection indicator is visible"
    );
    let bounds = indicator.bounds().scale(window.scale_factor());
    let quad = window
        .painted_quads()
        .into_iter()
        .find(|quad| quad.bounds == bounds)
        .unwrap_or_else(|| panic!("{scene}: no painted connection indicator at {bounds:?}"));
    assert_eq!(
        quad.background,
        gpui_kit::Background::from(rgb(expected)),
        "{scene}: painted connection state"
    );
}

#[gpui_kit::test]
fn connection_indicators_follow_lifecycle_and_active_tab_in_all_themes(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let (source, pending) = watch::channel(TransportState::Starting);
    let (_ready_source, ready) = watch::channel(TransportState::Ready);
    panes[0]
        .terminal
        .update(cx, |terminal, _| terminal.attach_lifecycle(pending));
    panes[1]
        .terminal
        .update(cx, |terminal, _| terminal.attach_lifecycle(ready));

    for (state, message, connected) in [
        (
            TransportState::Starting,
            Message::new("正在连接…", "Starting…"),
            false,
        ),
        (
            TransportState::Ready,
            Message::new("已连接", "Connected"),
            true,
        ),
        (
            TransportState::Ended(ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost)),
            Message::new("SSH 连接已断开", "SSH connection lost"),
            false,
        ),
    ] {
        source.send_replace(state);
        panes[0].terminal.update(cx, |terminal, cx| {
            terminal.status = message;
            cx.notify();
        });
        cx.update_window(fixture.window, |_, window, cx| {
            for family in crate::design::registry() {
                for appearance in [Theme::Light, Theme::Dark] {
                    for language in [Language::ZhCn, Language::En] {
                        fixture.workspace.update(cx, |view, cx| {
                            view.state.settings.theme_family =
                                ThemeId::new(family.id).checked("registered theme identity");
                            view.state.settings.theme = appearance;
                            crate::design::apply_settings(&view.state.settings, Some(window), cx);
                            i18n::set_language(language, cx);
                        });
                        settle(window, cx);
                        let visual = crate::design::palette(cx);
                        let scene = format!(
                            "{}/{appearance:?}/{language:?}/{}",
                            family.id,
                            panes[0].terminal.read(cx).status.render(cx)
                        );
                        let expected = if connected {
                            visual.success
                        } else {
                            visual.warning
                        };
                        assert_eq!(panes[0].terminal.read(cx).is_open(), connected, "{scene}");
                        assert_indicator_color(
                            window,
                            ("session-tab-status", 0usize),
                            expected,
                            &scene,
                        );
                        assert_indicator_color(
                            window,
                            ("session-tab-status", 1usize),
                            visual.success,
                            &scene,
                        );
                        for (index, expected) in [(1usize, visual.success), (0usize, expected)] {
                            window.click(("session-tab", index), cx);
                            settle(window, cx);
                            assert_eq!(fixture.workspace.read(cx).active, index, "{scene}");
                            assert_indicator_color(
                                window,
                                "workspace-connection-status-indicator",
                                expected,
                                &scene,
                            );
                        }
                    }
                }
            }
        })
        .checked("render actual lifecycle and active-tab connection colors");
    }
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn restored_and_empty_panes_never_paint_connected_indicators(cx: &mut TestAppContext) {
    let fixture = mount_state_sized(cx, super::startup_restore::scene(false), 1600., 1000.);
    cx.update_window(fixture.window, |_, window, cx| {
        for family in crate::design::registry() {
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    fixture.workspace.update(cx, |view, cx| {
                        view.state.settings.theme_family =
                            ThemeId::new(family.id).checked("registered theme identity");
                        view.state.settings.theme = appearance;
                        crate::design::apply_settings(&view.state.settings, Some(window), cx);
                        i18n::set_language(language, cx);
                    });
                    let visual = crate::design::palette(cx);
                    let scene = format!("{}/{appearance:?}/{language:?}", family.id);
                    for (index, expected) in [
                        (0usize, visual.warning),
                        (1, visual.warning),
                        (2, visual.danger),
                    ] {
                        fixture.workspace.update(cx, |view, cx| {
                            view.select_session_tab(index, window, cx);
                        });
                        settle(window, cx);
                        let view = fixture.workspace.read(cx);
                        let terminal = &view.tabs[index];
                        assert!(
                            view.restored_tabs.contains_key(&terminal.entity_id()),
                            "{scene}"
                        );
                        assert!(
                            !terminal.read(cx).is_open(),
                            "{scene}: restored tab stays inert"
                        );
                        assert!(terminal.read(cx).end_reason().is_none(), "{scene}");
                        assert_indicator_color(
                            window,
                            ("session-tab-status", index),
                            expected,
                            &scene,
                        );
                        assert_indicator_color(
                            window,
                            "workspace-connection-status-indicator",
                            expected,
                            &scene,
                        );
                    }
                    let choose: SharedString =
                        format!("select-empty-pane-{}", uuid::Uuid::from_u128(203)).into();
                    window.click(choose, cx);
                    settle(window, cx);
                    let view = fixture.workspace.read(cx);
                    assert!(!view.tabs.is_empty(), "{scene}: sibling tabs remain");
                    assert!(
                        view.tabs.get(view.active).is_none(),
                        "{scene}: empty active pane"
                    );
                    assert_indicator_color(
                        window,
                        "workspace-connection-status-indicator",
                        visual.muted,
                        &scene,
                    );
                    assert!(!view.connecting && view.connect_route.is_none(), "{scene}");
                }
            }
        }
    })
    .checked("render disconnected, missing and empty restored-pane connection colors");
}
