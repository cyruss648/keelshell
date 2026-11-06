//! Production startup, native layout and inert actions; no desktop acceptance claim.

use super::*;
use gpui_kit::{App, EntityId, SharedString, Window};
use keelshell_core::{
    AuthMethod, DeletedConnection, LayoutPane, LayoutTab, LayoutTabAvailability, LayoutWindow,
    PaneLayout, PaneNodeId, SplitAxis, WORKSPACE_LAYOUT_VERSION, WorkspaceLayout,
};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn pane(value: u128) -> PaneNodeId {
    PaneNodeId(id(value))
}

pub(super) fn scene(active_empty: bool) -> AppState {
    let mut profile = Connection::new("Current saved profile", "restore.invalid", "reader");
    profile.id = id(1);
    profile.auth = AuthMethod::Password;
    let mut layout = PaneLayout::single(pane(201)).checked("initial saved leaf");
    layout
        .split(pane(201), pane(202), pane(204), SplitAxis::Horizontal, 4000)
        .checked("saved left/right geometry");
    layout
        .split(pane(202), pane(203), pane(205), SplitAxis::Vertical, 6500)
        .checked("saved nested geometry");
    layout
        .set_active(if active_empty { pane(203) } else { pane(201) })
        .checked("saved active leaf");
    let mut state = AppState {
        connections: vec![profile],
        workspace_layout: Some(WorkspaceLayout {
            version: WORKSPACE_LAYOUT_VERSION,
            windows: vec![
                LayoutWindow {
                    id: id(100),
                    layout: PaneLayout::single(pane(101)).checked("unselected saved window"),
                    panes: vec![LayoutPane {
                        pane: pane(101),
                        tabs: vec![LayoutTab {
                            id: id(102),
                            connection_id: Some(id(1)),
                        }],
                        active_tab: Some(id(102)),
                    }],
                },
                LayoutWindow {
                    id: id(200),
                    layout,
                    panes: vec![
                        LayoutPane {
                            pane: pane(201),
                            tabs: vec![
                                LayoutTab {
                                    id: id(301),
                                    connection_id: Some(id(1)),
                                },
                                LayoutTab {
                                    id: id(302),
                                    connection_id: None,
                                },
                            ],
                            active_tab: Some(id(302)),
                        },
                        LayoutPane {
                            pane: pane(202),
                            tabs: vec![LayoutTab {
                                id: id(303),
                                connection_id: Some(id(2)),
                            }],
                            active_tab: Some(id(303)),
                        },
                        LayoutPane {
                            pane: pane(203),
                            tabs: Vec::new(),
                            active_tab: None,
                        },
                    ],
                },
            ],
            active_window: id(200),
        }),
        ..AppState::default()
    };
    state.settings.updates.frequency = keelshell_core::UpdateCheckFrequency::Disabled;
    state
}

fn key(workspace: &Workspace, stable: u128) -> EntityId {
    workspace
        .pane_tab_ids
        .iter()
        .find_map(|(entity, saved)| (*saved == id(stable)).then_some(*entity))
        .checked_option("restored stable tab UUID")
}

fn settle(window: &mut Window, cx: &mut App) {
    for _ in 0..3 {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
    }
    window.render_frame(cx);
}

fn assert_inert(workspace: &Workspace, cx: &App) {
    assert!(workspace.tabs.iter().all(|tab| !tab.read(cx).is_open()));
    assert!(workspace.remote_sessions.is_empty());
    assert!(workspace.remote_hosts.is_empty());
    assert!(workspace.remote_connection_ids.is_empty());
    assert!(workspace.panels.is_empty());
    assert!(workspace.archived_panels.is_empty());
    assert!(workspace.reconnect_bindings.is_empty());
    assert!(workspace.terminal_observers.is_empty());
    assert!(workspace.terminal_focus.is_empty());
    assert!(workspace.tunnel_subscriptions.is_empty());
    assert!(workspace.tunnel_owners.is_empty());
    assert!(workspace.command_histories.is_empty());
    assert!(workspace.command_target.is_none());
    assert!(workspace.command.read(cx).value().is_empty());
    assert!(workspace.agent.is_none());
    assert!(workspace.batch_panel.is_none());
    assert!(workspace.workflow_panel.is_none());
    assert!(!workspace.mcp_test_enabled());
    assert!(workspace.mcp_test_client().is_err());
    assert_eq!(
        workspace.assistant.read(cx).captured_context_for_test(),
        ("", "", "")
    );
    assert!(!workspace.connecting);
    assert!(workspace.connect_route.is_none());
    assert!(workspace.login.is_none());
    assert!(workspace.host_approval.is_none());
}

#[gpui_kit::test]
fn startup_restores_persisted_active_window_geometry_tab_identity_order_and_selection(
    cx: &mut TestAppContext,
) {
    let state = scene(false);
    let expected = state
        .workspace_layout
        .clone()
        .checked_option("saved layout");
    let fixture = mount_state_sized(cx, state, 1600., 1000.);
    let before = std::fs::read(fixture.store.path()).checked("read state before rendering startup");
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.layout_window_id, id(200));
            assert_eq!(
                workspace.session_panes.layout(),
                &expected.windows[1].layout
            );
            assert_eq!(workspace.tabs.len(), 3);
            for saved in &expected.windows[1].panes {
                let restored = workspace.session_panes.tabs_in(saved.pane);
                assert_eq!(
                    restored
                        .iter()
                        .map(|key| workspace.pane_tab_ids[key])
                        .collect::<Vec<_>>(),
                    saved.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>()
                );
                assert_eq!(
                    workspace
                        .session_panes
                        .selected_in(saved.pane)
                        .map(|key| workspace.pane_tab_ids[&key]),
                    saved.active_tab
                );
            }
            assert_eq!(
                workspace.tabs[workspace.active].entity_id(),
                key(workspace, 302)
            );
            assert_inert(workspace, cx);
        });
        let canvas = window.find("pane-canvas").bounds();
        for stable in [302, 303] {
            let target = key(fixture.workspace.read(cx), stable);
            let placeholder = window.find(("restored-tab", target));
            assert!(placeholder.visible());
            let bounds = placeholder.bounds();
            assert!(bounds.size.width > px(100.) && bounds.size.height > px(60.));
            assert!(bounds.left() >= canvas.left() && bounds.right() <= canvas.right());
        }
        assert!(
            window
                .find(SharedString::from(format!("empty-pane-{}", id(203))))
                .visible()
        );
    })
    .checked("render actual restored workspace and assert native geometry");
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read state after startup render"),
        before,
        "startup restore must not rewrite the persisted layout"
    );
}

#[gpui_kit::test]
fn startup_empty_active_leaf_does_not_select_or_focus_a_sibling_and_all_empty_topology_survives(
    cx: &mut TestAppContext,
) {
    for all_empty in [false, true] {
        let mut state = scene(true);
        if all_empty {
            for saved in &mut state
                .workspace_layout
                .as_mut()
                .checked_option("saved layout")
                .windows[1]
                .panes
            {
                saved.tabs.clear();
                saved.active_tab = None;
            }
        }
        let fixture = mount_state_sized(cx, state, 1600., 1000.);
        cx.update_window(fixture.window, |_, window, cx| {
            settle(window, cx);
            fixture.workspace.update(cx, |workspace, cx| {
                assert!(workspace.sync_session_panes());
                assert_eq!(workspace.session_panes.active_pane(), pane(203));
                assert_eq!(workspace.session_panes.active_session(), None);
                assert!(workspace.tabs.get(workspace.active).is_none());
                assert_eq!(
                    workspace
                        .session_panes
                        .layout()
                        .leaf_ids()
                        .checked("leaves")
                        .len(),
                    3
                );
                workspace.focus_current_surface(window, cx);
                assert!(workspace.surface_focus.is_focused(window));
                assert!(
                    workspace
                        .tabs
                        .iter()
                        .all(|tab| !tab.read(cx).focus_handle(cx).is_focused(window))
                );
                assert_inert(workspace, cx);
            });
            settle(window, cx);
            assert!(window.find("pane-canvas").visible());
            let choose: SharedString = format!("select-empty-pane-{}", id(203)).into();
            assert_eq!(window.find(choose).checked(), Some(true));
        })
        .checked("retain a real empty selected leaf across production reconciliation");
    }
}

#[gpui_kit::test]
fn restored_states_and_actions_are_bilingual_in_all_three_semantic_themes(cx: &mut TestAppContext) {
    let fixture = mount_state_sized(cx, scene(false), 1600., 1000.);
    cx.update_window(fixture.window, |_, window, cx| {
        for language in [Language::ZhCn, Language::En] {
            for family in crate::design::registry() {
                for mode in [Theme::Light, Theme::Dark] {
                    fixture.workspace.update(cx, |workspace, cx| {
                        workspace.state.settings.theme_family =
                            keelshell_core::ThemeId::new(family.id).checked("registered family");
                        workspace.state.settings.theme = mode;
                        crate::design::apply_settings(&workspace.state.settings, Some(window), cx);
                        i18n::set_language(language, cx);
                    });
                    for stable in [301, 302, 303] {
                        fixture.workspace.update(cx, |workspace, cx| {
                            let target = key(workspace, stable);
                            let index = workspace
                                .tabs
                                .iter()
                                .position(|tab| tab.entity_id() == target)
                                .checked_option("restored tab position");
                            workspace.select_session_tab(index, window, cx);
                        });
                        settle(window, cx);
                        let workspace = fixture.workspace.read(cx);
                        let target = key(workspace, stable);
                        let available = stable == 301;
                        let state = workspace
                            .restored_status(target, cx)
                            .checked_option("state label");
                        assert_eq!(
                            window.find(("restored-tab-state", target)).label(),
                            Some(state)
                        );
                        assert!(window.find(("manage-restored-tab", target)).visible());
                        assert!(window.find(("close-restored-tab", target)).visible());
                        assert_eq!(
                            window.try_find(("connect-restored-tab", target)).is_some(),
                            available
                        );
                        if available {
                            let expected = format!(
                                "{}: Current saved profile",
                                if language == Language::ZhCn {
                                    "连接当前保存配置"
                                } else {
                                    "Connect using current saved profile"
                                }
                            );
                            assert_eq!(
                                window.find(("connect-restored-tab", target)).label(),
                                Some(expected.as_str())
                            );
                        }
                        assert_inert(workspace, cx);
                    }
                }
            }
        }
    })
    .checked("exercise production placeholders and localized actions in the theme matrix");
}

#[gpui_kit::test]
fn restored_connect_uses_current_profile_and_deleted_or_trashed_references_stay_inert(
    cx: &mut TestAppContext,
) {
    let fixture = mount_state_sized(cx, scene(false), 1440., 900.);
    cx.update_window(fixture.window, |_, window, cx| {
        let target = key(fixture.workspace.read(cx), 301);
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.state.connections[0].host = "updated.restore.invalid".into();
            workspace.state.connections[0].username = "current-user".into();
            workspace.request_restored_connect(target, window, cx);
            let login = workspace
                .login
                .as_ref()
                .checked_option("current profile login");
            assert_eq!(login.connection.host, "updated.restore.invalid");
            assert_eq!(login.connection.username, "current-user");
            assert_eq!(workspace.restored_tabs[&target].id, id(301));
            assert!(workspace.remote_sessions.is_empty());
            assert!(workspace.reconnect_bindings.is_empty());
            workspace.cancel_connect_route(window, cx);
            workspace.show_connections = false;
            let removed = workspace.state.connections.remove(0);
            assert_eq!(
                workspace.restored_tabs[&target].availability(&workspace.state),
                LayoutTabAvailability::Missing
            );
            workspace.request_restored_connect(target, window, cx);
            assert_inert(workspace, cx);
            workspace.state.deleted_connections.push(DeletedConnection {
                connection: removed,
                deleted_at: 1,
            });
            workspace.request_restored_connect(target, window, cx);
            assert_inert(workspace, cx);
        });
        settle(window, cx);
    })
    .checked("resolve the saved UUID at activation time and fail closed for removed profiles");
}

#[gpui_kit::test]
async fn restored_close_and_save_keep_other_window_and_remaining_references_without_connecting(
    cx: &mut TestAppContext,
) {
    let fixture = mount_state_sized(cx, scene(false), 1600., 1000.);
    let original = fixture
        .store
        .load()
        .checked("read original layout")
        .workspace_layout
        .checked_option("original layout");
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        let target = key(fixture.workspace.read(cx), 302);
        window.click(("close-restored-tab", target), cx);
        settle(window, cx);
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.tabs.len(), 2);
            assert!(!workspace.restored_tabs.contains_key(&target));
            assert_eq!(workspace.pane_tab_ids[&key(workspace, 301)], id(301));
            assert_eq!(
                workspace.restored_tabs[&key(workspace, 303)].connection_id,
                Some(id(2))
            );
            assert_inert(workspace, cx);
        });
        window.click("save-pane-layout", cx);
    })
    .checked("close a restored placeholder and explicitly save through production actions");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let saved = fixture
        .store
        .load()
        .checked("reload saved restored layout")
        .workspace_layout
        .checked_option("saved layout");
    assert_eq!(saved.windows.len(), 2);
    assert_eq!(saved.windows[0], original.windows[0]);
    assert_eq!(saved.active_window, id(200));
    assert_eq!(
        saved.windows[1].panes[0].tabs,
        vec![LayoutTab {
            id: id(301),
            connection_id: Some(id(1))
        }]
    );
    assert_eq!(saved.windows[1].panes[0].active_tab, Some(id(301)));
    assert_eq!(
        saved.windows[1].panes[1].tabs,
        original.windows[1].panes[1].tabs
    );
    assert_eq!(saved.windows[1].panes[2], original.windows[1].panes[2]);
    fixture.workspace.read_with(cx, assert_inert);
}

#[gpui_kit::test]
fn restored_compact_view_preserves_geometry_and_selects_tabs_without_ssh_focus(
    cx: &mut TestAppContext,
) {
    let fixture = mount_state_sized(cx, scene(false), 360., 580.);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, _| workspace.show_navigation = false);
        settle(window, cx);
        let before = fixture.workspace.read(cx).session_panes.layout().clone();
        assert_eq!(window.viewport_size().width, px(360.));
        assert!(fixture.workspace.read(cx).compact_panes());
        assert!(window.find("compact-pane-notice").visible());
        window.click("next-session", cx);
        settle(window, cx);
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(
                workspace.session_panes.active_session(),
                Some(key(workspace, 303))
            );
            assert_eq!(workspace.session_panes.layout().nodes, before.nodes);
            assert!(workspace.surface_focus.is_focused(window));
            assert_inert(workspace, cx);
        });
        let missing = key(fixture.workspace.read(cx), 303);
        assert!(window.find(("restored-tab", missing)).visible());
    })
    .checked("use the real compact canvas and session strip with restored state");
}

#[gpui_kit::test]
async fn restored_layout_save_conflict_preserves_placeholder_identity_and_external_profile_change(
    cx: &mut TestAppContext,
) {
    let fixture = mount_state_sized(cx, scene(false), 1440., 900.);
    let before = fixture.workspace.read_with(cx, |workspace, _| {
        (
            workspace.session_panes.clone(),
            workspace.pane_tab_ids.clone(),
            workspace.restored_tabs.clone(),
        )
    });
    let external = StateStore::new(fixture.store.path());
    let mut changed = external.load().checked("read competing configuration");
    changed.connections[0].host = "external.restore.invalid".into();
    external
        .save(&changed)
        .checked("save competing profile revision");
    let bytes = std::fs::read(fixture.store.path()).checked("capture competing state");
    cx.update_window(fixture.window, |_, window, cx| {
        settle(window, cx);
        window.click("save-pane-layout", cx);
    })
    .checked("save the restored layout from a stale state revision");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read state after conflicting layout save"),
        bytes
    );
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.session_panes, before.0);
        assert_eq!(workspace.pane_tab_ids, before.1);
        assert_eq!(workspace.restored_tabs, before.2);
        assert_inert(workspace, cx);
    });
}
