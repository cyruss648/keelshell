//! Rendered workspace regions and navigation transitions, not native GUI evidence.
use super::*;
use crate::workspace::navigation::NavigationPanel;
use gpui_kit::{App, Pixels, SharedString, Window};
use keelshell_core::{Theme, ThemeId};
use keelshell_session::{ConnectionEnd, ShellEnd};
use tokio::sync::watch;

use crate::{i18n::Message, terminal::TransportState};

fn settle(window: &mut Window, cx: &mut App) {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
}

fn inside(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
fn workbench_regions_fit_all_registered_themes_without_changing_drafts(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&fixture, cx);
    let identity = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.command.update(cx, |input, cx| {
                    input.set_value("printf '未发送'\n# second line", window, cx);
                });
                (
                    view.command.entity_id(),
                    view.command_revision,
                    view.command_target,
                    view.assistant.entity_id(),
                    view.remote_completion.directory.entity_id(),
                )
            })
        })
        .checked("prepare retained command and assistant identities");
    for (width, height) in [(900., 580.), (1280., 840.), (1440., 900.)] {
        cx.simulate_window_resize(fixture.window, size(px(width), px(height)));
        cx.run_until_parked();
        cx.update_window(fixture.window, |_, window, cx| {
            for family in crate::design::registry() {
                for appearance in [Theme::Light, Theme::Dark] {
                    for language in [Language::ZhCn, Language::En] {
                        i18n::set_language(language, cx);
                        fixture.workspace.update(cx, |view, cx| {
                            view.state.settings.theme_family =
                                ThemeId::new(family.id).checked("registered theme identity");
                            view.state.settings.theme = appearance;
                            crate::design::apply_settings(&view.state.settings, Some(window), cx);
                        });
                        settle(window, cx);
                        let scene =
                            format!("{width}x{height}/{}/{appearance:?}/{language:?}", family.id);
                        let toolbar = window.find("workspace-toolbar").bounds();
                        let strip = window.find("session-strip").bounds();
                        let column = window.find("command-column").bounds();
                        assert!(inside(toolbar, window.bounds()), "{scene}: toolbar");
                        assert!(inside(strip, column), "{scene}: session strip");
                        assert!(strip.top() >= toolbar.bottom(), "{scene}: separate chrome");
                        match window.try_find("workspace-brand") {
                            Some(brand) => {
                                assert!(width >= 1280., "{scene}: brand is too wide");
                                assert!(inside(brand.bounds(), toolbar), "{scene}: brand");
                            }
                            None => {
                                assert!(width < 1280., "{scene}: wide toolbar lost brand");
                            }
                        }
                        let terminal = window
                            .find(("terminal-pane", panes[0].terminal.entity_id()))
                            .bounds();
                        assert!(terminal.top() >= strip.bottom(), "{scene}: terminal");
                        assert!(terminal.size.width >= px(200.), "{scene}: terminal width");
                        assert!(terminal.size.height >= px(80.), "{scene}: terminal height");
                        for id in [
                            "toggle-navigation",
                            "navigation-connections",
                            "navigation-monitor",
                            "new-session",
                            "command-input-container",
                            "command-target-badge",
                            "remote-completion-controls",
                            "command-actions",
                            "workspace-tool-tabs",
                        ] {
                            let item = window.find(id);
                            assert!(item.visible(), "{scene}: {id} hidden");
                            assert!(inside(item.bounds(), window.bounds()), "{scene}: {id}");
                        }
                        let view = fixture.workspace.read(cx);
                        assert_eq!(
                            (
                                view.command.entity_id(),
                                view.command_revision,
                                view.command_target,
                                view.assistant.entity_id(),
                                view.remote_completion.directory.entity_id(),
                            ),
                            identity,
                            "{scene}: presentation must retain review and input identities"
                        );
                        assert_eq!(
                            view.command.read(cx).value(),
                            "printf '未发送'\n# second line"
                        );
                    }
                }
            }
        })
        .checked("measure actual bilingual regions across all theme palettes");
    }
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn toolbar_groups_keep_action_order_and_accessible_names_at_workbench_widths(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for (width, language) in [
            (1440., Language::ZhCn),
            (1280., Language::En),
            (900., Language::En),
        ] {
            window.resize(size(px(width), px(580.)));
            window.bounds_changed(cx);
            i18n::set_language(language, cx);
            settle(window, cx);

            let toolbar = window.find("workspace-toolbar").bounds();
            let mut previous_right = toolbar.left();
            for id in [
                "workspace-navigation-group",
                "workspace-session-context",
                "workspace-pane-divider",
                "workspace-pane-actions",
                "workspace-assistant-divider",
                "workspace-assistant-action",
                "workspace-settings-divider",
                "workspace-settings-actions",
            ] {
                let element = window.find(id);
                let bounds = element.bounds();
                assert!(element.visible(), "{language:?}/{width}: {id} hidden");
                assert!(
                    inside(bounds, toolbar),
                    "{language:?}/{width}: {id} escapes toolbar"
                );
                assert!(
                    bounds.left() >= previous_right,
                    "{language:?}/{width}: {id} overlaps the previous toolbar group"
                );
                previous_right = bounds.right();
            }

            for id in [
                "toggle-navigation",
                "connection-manager",
                "split-session",
                "split-session-vertical",
                "collapse-pane",
                "zoom-pane",
                "save-pane-layout",
                "toggle-assistant",
                "vault-settings",
                "about-updates",
                "configuration-recovery",
                "theme-settings",
                "language",
            ] {
                let element = window.find(id);
                assert!(element.visible(), "{language:?}/{width}: {id} hidden");
                assert!(
                    element.bounds().right() <= toolbar.right(),
                    "{language:?}/{width}: {id} escapes toolbar"
                );
            }

            let context = window.find("workspace-session-context").bounds();
            let context_label = window.find("workspace-session-context-label").bounds();
            let context_value = window.find("workspace-session-context-value").bounds();
            assert!(
                context.size.width >= px(if width <= 1280. { 112. } else { 72. }),
                "{language:?}/{width}: context width collapsed"
            );
            assert!(
                context_label.size.width >= px(72.),
                "{language:?}/{width}: context label collapsed"
            );
            assert!(
                context_value.size.width >= px(72.),
                "{language:?}/{width}: context value collapsed"
            );
            assert!(
                context_label.size.height <= px(20.),
                "{language:?}/{width}: context label wrapped vertically"
            );
            assert!(
                context_value.size.height <= px(20.),
                "{language:?}/{width}: context value wrapped vertically"
            );

            let labels = [
                (
                    "toggle-navigation",
                    crate::i18n::t(cx, "连接与主机侧栏", "Connections and host sidebar"),
                ),
                (
                    "connection-manager",
                    crate::i18n::t(cx, "连接", "Connections"),
                ),
                (
                    "split-session",
                    crate::i18n::t(
                        cx,
                        "左右分屏当前 SSH 会话",
                        "Split current SSH session left and right",
                    ),
                ),
                (
                    "toggle-assistant",
                    crate::i18n::t(cx, "AI 助手", "AI assistant"),
                ),
            ];
            for (id, expected) in labels {
                assert_eq!(
                    window.find(id).label(),
                    Some(expected),
                    "{language:?}/{width}: {id} AX label"
                );
            }
        }
    })
    .checked("keep the grouped toolbar ordered and accessible at wide and compact widths");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn compact_toolbar_keeps_critical_actions_discoverable_and_terminal_usable(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for family in crate::design::registry() {
            let family_id = family.id;
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    i18n::set_language(language, cx);
                    fixture.workspace.update(cx, |view, cx| {
                        view.show_assistant = false;
                        view.state.settings.theme_family =
                            ThemeId::new(family.id).checked("registered theme identity");
                        view.state.settings.theme = appearance;
                        crate::design::apply_settings(&view.state.settings, Some(window), cx);
                    });
                    settle(window, cx);

                    let toolbar = window.find("workspace-toolbar").bounds();
                    let strip = window.find("session-strip").bounds();
                    for (id, expected) in [
                        (
                            "toggle-assistant",
                            crate::i18n::t(cx, "AI 助手", "AI assistant"),
                        ),
                        (
                            "vault-settings",
                            crate::i18n::t(cx, "凭据库", "Credential vault"),
                        ),
                        (
                            "configuration-recovery",
                            crate::i18n::t(
                                cx,
                                "配置备份与恢复",
                                "Configuration backup and recovery",
                            ),
                        ),
                    ] {
                        let action = window.find(id);
                        assert!(
                            action.visible(),
                            "{family_id}/{appearance:?}/{language:?}: {id} hidden"
                        );
                        assert!(
                            inside(action.bounds(), toolbar),
                            "{family_id}/{appearance:?}/{language:?}: {id} escapes toolbar"
                        );
                        assert!(
                            action.bounds().size.width >= px(20.)
                                && action.bounds().size.height >= px(20.),
                            "{family_id}/{appearance:?}/{language:?}: {id} collapsed {:?}",
                            action.bounds()
                        );
                        assert_eq!(
                            action.label(),
                            Some(expected),
                            "{family_id}/{appearance:?}/{language:?}: {id} AX label"
                        );
                    }

                    let new_window = window.find("new-workspace-window");
                    assert!(
                        new_window.visible() && inside(new_window.bounds(), strip),
                        "{family_id}/{appearance:?}/{language:?}: new window discovery"
                    );
                    assert!(
                        new_window.bounds().size.width >= px(20.)
                            && new_window.bounds().size.height >= px(20.),
                        "{family_id}/{appearance:?}/{language:?}: new window collapsed {:?}",
                        new_window.bounds()
                    );
                    assert_eq!(
                        new_window.label(),
                        Some(crate::i18n::t(cx, "新建工作区窗口", "New workspace window")),
                        "{family_id}/{appearance:?}/{language:?}: new window AX label"
                    );

                    window.click("toggle-assistant", cx);
                    settle(window, cx);
                    let assistant = window.find("assistant-column").bounds();
                    let terminal = window
                        .find(("terminal-pane", panes[0].terminal.entity_id()))
                        .bounds();
                    assert!(
                        assistant.size.width >= px(240.) && assistant.size.width <= px(360.),
                        "{family_id}/{appearance:?}/{language:?}: assistant width {:?}",
                        assistant
                    );
                    assert!(
                        terminal.size.width >= px(200.) && terminal.size.height >= px(80.),
                        "{family_id}/{appearance:?}/{language:?}: terminal budget {:?}",
                        terminal
                    );
                    assert!(
                        window.find("toggle-assistant").visible(),
                        "{family_id}/{appearance:?}/{language:?}: AI entry hidden while open"
                    );
                    assert_eq!(
                        window.find("toggle-assistant").label(),
                        Some(crate::i18n::t(cx, "AI 助手", "AI assistant")),
                        "{family_id}/{appearance:?}/{language:?}: open AI AX label"
                    );
                    window.click("toggle-assistant", cx);
                    settle(window, cx);
                }
            }
        }
    })
    .checked("keep compact workbench actions discoverable across themes and languages");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn navigation_headers_expose_panel_semantics_and_token_geometry(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for family in crate::design::registry() {
            let family_id = family.id;
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    i18n::set_language(language, cx);
                    fixture.workspace.update(cx, |view, cx| {
                        view.show_assistant = false;
                        view.navigation_panel = NavigationPanel::Connections;
                        view.state.settings.theme_family =
                            ThemeId::new(family.id).checked("registered theme identity");
                        view.state.settings.theme = appearance;
                        crate::design::apply_settings(&view.state.settings, Some(window), cx);
                    });
                    settle(window, cx);

                    let chrome = crate::design::chrome(cx);
                    let rail = window.find("workspace-navigation-rail");
                    let rail_bounds = rail.bounds();
                    assert_eq!(
                        rail.role(),
                        Some(gpui_kit::accesskit::Role::Group),
                        "{family_id}/{appearance:?}/{language:?}: navigation rail role"
                    );
                    assert_eq!(
                        rail.label(),
                        Some(crate::i18n::t(cx, "工作区导航", "Workspace navigation")),
                        "{family_id}/{appearance:?}/{language:?}: navigation rail AX label"
                    );
                    let column = window.find("monitor-column").bounds();
                    assert_eq!(
                        column.size.width,
                        px(236.),
                        "{family_id}/{appearance:?}/{language:?}: compact connection rail width"
                    );
                    let header = window.find("workspace-navigation-header");
                    assert_eq!(
                        header.label(),
                        Some(crate::i18n::t(cx, "连接导航", "Connection navigation")),
                        "{family_id}/{appearance:?}/{language:?}: connection header AX label"
                    );
                    assert_eq!(
                        header.bounds().size.height,
                        px(chrome.pane_header_height),
                        "{family_id}/{appearance:?}/{language:?}: connection header token height"
                    );
                    assert!(
                        inside(header.bounds(), rail_bounds),
                        "{family_id}/{appearance:?}/{language:?}: connection header escapes rail"
                    );

                    window.click("navigation-monitor", cx);
                    settle(window, cx);
                    let monitor_column = window.find("monitor-column").bounds();
                    let monitor_header = window.find("workspace-navigation-header");
                    assert_eq!(
                        monitor_column.size.width,
                        px(260.),
                        "{family_id}/{appearance:?}/{language:?}: monitor rail width"
                    );
                    assert_eq!(
                        monitor_header.label(),
                        Some(crate::i18n::t(cx, "主机监控", "Host monitoring")),
                        "{family_id}/{appearance:?}/{language:?}: monitor header AX label"
                    );
                    assert_eq!(
                        monitor_header.bounds().size.height,
                        px(chrome.pane_header_height),
                        "{family_id}/{appearance:?}/{language:?}: monitor header token height"
                    );
                    assert!(
                        inside(
                            monitor_header.bounds(),
                            window.find("workspace-navigation-rail").bounds()
                        ),
                        "{family_id}/{appearance:?}/{language:?}: monitor header escapes rail"
                    );
                }
            }
        }
    })
    .checked("keep connection and host-monitor navigation semantics stable across themes");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn every_terminal_pane_keeps_a_bilingual_connection_identity_bar(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for (language, expected_protocol) in [
            (Language::ZhCn, "SSH / UTF-8"),
            (Language::En, "SSH / UTF-8"),
        ] {
            i18n::set_language(language, cx);
            let settings = fixture.workspace.read(cx).state.settings.clone();
            crate::design::apply_settings(&settings, Some(window), cx);
            settle(window, cx);
            let pane = fixture.workspace.read(cx).session_panes.active_pane();
            let identity = window.find(SharedString::from(format!("pane-identity-{}", pane.0)));
            let endpoint = window.find(SharedString::from(format!(
                "pane-identity-endpoint-{}",
                pane.0
            )));
            let protocol = window.find(SharedString::from(format!(
                "pane-identity-protocol-{}",
                pane.0
            )));
            let status = window.find(SharedString::from(format!(
                "pane-identity-status-{}",
                pane.0
            )));
            assert!(identity.visible(), "{language:?}: identity bar");
            assert!(inside(
                identity.bounds(),
                window
                    .find(("terminal-pane", panes[0].terminal.entity_id()))
                    .bounds()
            ));
            assert_eq!(
                endpoint.label(),
                Some("fixture-0@example.invalid:22"),
                "{language:?}: endpoint remains real and stable"
            );
            assert_eq!(
                protocol.label(),
                Some(expected_protocol),
                "{language:?}: protocol and status"
            );
            assert_eq!(
                status.label(),
                Some(if language == Language::ZhCn {
                    "已连接"
                } else {
                    "Connected"
                })
            );
        }
    })
    .checked("render the real connection identity bar in both supported languages");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn connection_identity_bar_tracks_starting_and_transport_loss_in_both_languages(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let (_, lifecycle_rx) = watch::channel(TransportState::Starting);
    panes[0].terminal.update(cx, |terminal, _| {
        terminal.attach_lifecycle(lifecycle_rx);
        terminal.status = Message::new("正在连接…", "Starting…");
    });

    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        let pane = fixture.workspace.read(cx).session_panes.active_pane();
        for (language, expected) in [(Language::ZhCn, "正在连接…"), (Language::En, "Starting…")]
        {
            i18n::set_language(language, cx);
            settle(window, cx);
            let identity = window.find(SharedString::from(format!("pane-identity-{}", pane.0)));
            let status = window.find(SharedString::from(format!(
                "pane-identity-status-{}",
                pane.0
            )));
            assert_eq!(
                status.label(),
                Some(expected),
                "{language:?}: starting status"
            );
            let label = identity
                .label()
                .unwrap_or_else(|| panic!("{language:?}: identity AX label"));
            assert!(label.contains("fixture-0@example.invalid:22"));
            assert!(label.contains(expected));
        }
    })
    .checked("render the connecting identity in both supported languages");

    let (_, ended_rx) = watch::channel(TransportState::Ended(ShellEnd::ConnectionClosed(
        ConnectionEnd::TransportLost,
    )));
    panes[0].terminal.update(cx, |terminal, _| {
        terminal.attach_lifecycle(ended_rx);
        terminal.status = Message::new("SSH 连接已断开", "SSH connection lost");
    });
    cx.update_window(fixture.window, |_, window, cx| {
        let pane = fixture.workspace.read(cx).session_panes.active_pane();
        for (language, expected) in [
            (Language::ZhCn, "SSH 连接已断开"),
            (Language::En, "SSH connection lost"),
        ] {
            i18n::set_language(language, cx);
            settle(window, cx);
            let identity = window.find(SharedString::from(format!("pane-identity-{}", pane.0)));
            let status = window.find(SharedString::from(format!(
                "pane-identity-status-{}",
                pane.0
            )));
            assert_eq!(status.label(), Some(expected), "{language:?}: loss status");
            let label = identity
                .label()
                .unwrap_or_else(|| panic!("{language:?}: loss identity AX label"));
            assert!(label.contains("fixture-0@example.invalid:22"));
            assert!(label.contains(expected));
        }
    })
    .checked("render the typed transport loss in both supported languages");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn sidebar_and_assistant_transitions_reveal_the_active_tab_without_sending_text(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 580.);
    let panes: Vec<_> = (0..4)
        .flat_map(|_| attach_remote_panes(&fixture, cx))
        .collect();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.tabs = panes.iter().map(|pane| pane.terminal.clone()).collect();
            view.active = panes.len() - 1;
            view.command.update(cx, |input, cx| {
                input.set_value("printf 'retained navigation draft'", window, cx);
            });
            cx.notify();
        });
        settle(window, cx);
        let command_id = fixture.workspace.read(cx).command.entity_id();
        for button in [
            "toggle-navigation",
            "toggle-navigation",
            "toggle-assistant",
            "toggle-navigation",
            "toggle-navigation",
        ] {
            window.click(button, cx);
            settle(window, cx);
            let viewport = window.find("session-tabs").bounds();
            let close = window.find(("close-tab", panes.len() - 1));
            assert!(close.visible() && inside(close.bounds(), viewport));
            let view = fixture.workspace.read(cx);
            assert_eq!(view.active, panes.len() - 1);
            assert_eq!(view.tabs.len(), panes.len());
            assert_eq!(view.command.entity_id(), command_id);
            assert_eq!(
                view.command.read(cx).value(),
                "printf 'retained navigation draft'"
            );
        }
    })
    .checked("real buttons resize the session strip and retain its active close control");
    cx.simulate_window_resize(fixture.window, size(px(900.), px(580.)));
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        assert!(fixture.workspace.read(cx).show_assistant);
        assert!(window.try_find("monitor-column").is_none());
        window.click("toggle-navigation", cx);
        settle(window, cx);
        let view = fixture.workspace.read(cx);
        assert!(!view.show_assistant && view.show_navigation);
        assert!(window.find("navigation-connections").visible());
        assert!(window.try_find("assistant-column").is_none());
        assert!(inside(
            window.find(("close-tab", panes.len() - 1)).bounds(),
            window.find("session-tabs").bounds()
        ));
        assert_eq!(
            view.command.read(cx).value(),
            "printf 'retained navigation draft'"
        );
    })
    .checked("explicit sidebar access in a compact window preserves assistant and command state");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn resized_navigation_does_not_leave_a_hidden_search_or_terminal_focused(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 840.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_assistant = true;
            cx.notify();
        });
        settle(window, cx);
        window.click("navigation-search", cx);
        window.input("retained query", cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .navigation_focus
                .contains_focused(window, cx)
        );
    })
    .checked("focus the real search while both side panels are visible");
    cx.simulate_window_resize(fixture.window, size(px(900.), px(580.)));
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        let view = fixture.workspace.read(cx);
        assert!(view.surface_focus.is_focused(window));
        assert!(window.try_find("navigation-search").is_none());
        assert_eq!(view.search.read(cx).value(), "retained query");
        window.press("enter", cx);
        window.press("x", cx);
        assert_eq!(
            fixture.workspace.read(cx).search.read(cx).value(),
            "retained query"
        );
    })
    .checked("move focus to the inert workspace when resizing unmounts the search");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn modal_return_to_a_navigation_hidden_during_resize_never_focuses_ssh(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 840.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update(super::super::bind_keys);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_assistant = true;
            cx.notify();
        });
        settle(window, cx);
        window.click("navigation-search", cx);
        window.input("query retained behind modal", cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-t"
            } else {
                "ctrl-shift-t"
            },
            cx,
        );
        settle(window, cx);
        assert!(window.find("connection-manager-dialog").visible());
        window.resize(size(px(900.), px(580.)));
        window.bounds_changed(cx);
        settle(window, cx);
        window.click("close-manager", cx);
        settle(window, cx);
        assert!(window.try_find("navigation-search").is_none());
        assert!(
            fixture.workspace.read(cx).surface_focus.is_focused(window),
            "return from a modal to hidden navigation must use the inert workspace"
        );
        window.press("x", cx);
        window.press("enter", cx);
        assert_eq!(
            fixture.workspace.read(cx).search.read(cx).value(),
            "query retained behind modal"
        );
    })
    .checked("real modal keyboard entry, resize and return keep SSH out of the focus fallback");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn workspace_selected_controls_expose_accessible_state(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        assert_eq!(window.find("toggle-navigation").checked(), Some(true));
        for id in ["files", "commands", "tunnels"] {
            assert_eq!(window.find(id).checked(), Some(false));
        }
        window.click("commands", cx);
        settle(window, cx);
        assert_eq!(window.find("commands").checked(), Some(true));
        assert_eq!(window.find("files").checked(), Some(false));
        assert_eq!(window.find("tunnels").checked(), Some(false));
        window.click("toggle-assistant", cx);
        settle(window, cx);
        assert_eq!(window.find("toggle-navigation").checked(), Some(false));
        window.click("toggle-navigation", cx);
        settle(window, cx);
        assert_eq!(window.find("toggle-navigation").checked(), Some(true));
        assert!(!fixture.workspace.read(cx).show_assistant);
        window.click("commands", cx);
        settle(window, cx);
        assert_eq!(window.find("commands").checked(), Some(false));
    })
    .checked("selected tools and actual sidebar visibility reach the accessibility tree");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
