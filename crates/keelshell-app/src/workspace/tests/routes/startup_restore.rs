//! Construct and render startup state from disk, then exercise explicit real SSH routes.

use super::*;
use crate::command_suggestions::SuggestionSource;
use crate::workspace::ToolPanel;
use gpui_kit::EntityId;
use keelshell_core::{
    LayoutPane, LayoutTab, LayoutWindow, MAX_LAYOUT_TABS, MAX_LAYOUT_TABS_PER_PANE, PaneLayout,
    PaneNodeId, ReconnectPolicy, SplitAxis, WORKSPACE_LAYOUT_VERSION, WorkspaceLayout,
};

fn saved_layout(connection: uuid::Uuid) -> WorkspaceLayout {
    let tab = |connection_id| LayoutTab {
        id: uuid::Uuid::new_v4(),
        connection_id,
    };
    let unused_pane = PaneNodeId(uuid::Uuid::new_v4());
    let unused_tab = tab(Some(connection));
    let unused = LayoutWindow {
        id: uuid::Uuid::new_v4(),
        layout: PaneLayout::single(unused_pane).checked("unused saved window"),
        panes: vec![LayoutPane {
            pane: unused_pane,
            active_tab: Some(unused_tab.id),
            tabs: vec![unused_tab],
        }],
    };
    let left = PaneNodeId(uuid::Uuid::new_v4());
    let right = PaneNodeId(uuid::Uuid::new_v4());
    let mut layout = PaneLayout::single(left).checked("saved first pane");
    layout
        .split(
            left,
            right,
            PaneNodeId(uuid::Uuid::new_v4()),
            SplitAxis::Horizontal,
            6200,
        )
        .checked("saved split geometry");
    let left_tabs = vec![tab(None), tab(Some(connection))];
    let right_tabs = vec![tab(Some(uuid::Uuid::new_v4())), tab(Some(connection))];
    let selected = LayoutWindow {
        id: uuid::Uuid::new_v4(),
        layout,
        panes: vec![
            LayoutPane {
                pane: left,
                active_tab: Some(left_tabs[1].id),
                tabs: left_tabs,
            },
            LayoutPane {
                pane: right,
                active_tab: Some(right_tabs[1].id),
                tabs: right_tabs,
            },
        ],
    };
    let snapshot = WorkspaceLayout {
        version: WORKSPACE_LAYOUT_VERSION,
        active_window: selected.id,
        windows: vec![unused, selected],
    };
    snapshot.validate().checked("valid startup metadata");
    snapshot
}

fn reopen_from_disk(fixture: &mut Fixture, snapshot: &WorkspaceLayout, cx: &mut TestAppContext) {
    let runtime = fixture.workspace.read_with(cx, |view, _| {
        assert!(view.tabs.is_empty());
        assert!(view.remote_sessions.is_empty());
        assert!(view.connect_route.is_none());
        view.runtime.clone()
    });
    let mut state = fixture.store.load().checked("load isolated route metadata");
    state.workspace_layout = Some(snapshot.clone());
    for connection in &mut state.connections {
        connection.reconnect = ReconnectPolicy::Automatic {
            max_attempts: 2,
            initial_delay_seconds: 1,
            max_delay_seconds: 1,
        };
    }
    fixture
        .store
        .save(&state)
        .checked("persist startup metadata");
    let loaded = fixture
        .store
        .load()
        .checked("load persisted startup metadata");
    let before = std::fs::read(fixture.store.path())
        .checked("capture metadata before Workspace construction");
    let store = fixture.store.clone();
    // The previous root stays empty. The new Workspace is the displayed root,
    // and the original fixture retains the directory and runtime ownership.
    let (window, workspace) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1600.), px(1000.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::new(store, loaded, None, runtime, window, cx)),
        )
        .checked("mount fresh production Workspace from the saved file")
    });
    fixture.window = window;
    fixture.workspace = workspace;
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
    })
    .checked("display the fresh Workspace as the real GPUI root");
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read metadata after Workspace construction"),
        before
    );
}

fn tab_ids(fixture: &Fixture, cx: &mut TestAppContext) -> Vec<EntityId> {
    fixture.workspace.read_with(cx, |view, _| {
        view.tabs.iter().map(Entity::entity_id).collect()
    })
}

fn assert_saved_positions(
    fixture: &Fixture,
    saved: &LayoutWindow,
    ids: &[EntityId],
    cx: &mut TestAppContext,
) {
    fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.layout_window_id, saved.id);
        assert_eq!(view.session_panes.layout(), &saved.layout);
        assert_eq!(
            view.tabs.iter().map(Entity::entity_id).collect::<Vec<_>>(),
            ids
        );
        let mut offset = 0;
        for pane in &saved.panes {
            let expected = &ids[offset..offset + pane.tabs.len()];
            assert_eq!(view.session_panes.tabs_in(pane.pane), expected);
            for (tab, id) in pane.tabs.iter().zip(expected) {
                assert_eq!(view.pane_tab_ids[id], tab.id);
                assert_eq!(view.session_panes.pane_for(*id), Some(pane.pane));
            }
            let selected = pane
                .tabs
                .iter()
                .position(|tab| Some(tab.id) == pane.active_tab)
                .map(|index| expected[index]);
            assert_eq!(view.session_panes.selected_in(pane.pane), selected);
            offset += pane.tabs.len();
        }
        assert_eq!(offset, ids.len());
        assert_eq!(
            view.tabs.get(view.active).map(Entity::entity_id),
            view.session_panes.active_session()
        );
    });
}

fn assert_no_command_bytes(route: &RouteFixture) {
    for peer in [&route.gateway_state, &route.target_state] {
        assert!(
            peer.received
                .lock()
                .checked("inspect real SSH shell input")
                .is_empty()
        );
    }
}

fn assert_no_connections(route: &RouteFixture) {
    for peer in [&route.gateway_state, &route.target_state] {
        assert_eq!(peer.authenticated.load(Ordering::Acquire), 0);
        assert_eq!(peer.opened.load(Ordering::Acquire), 0);
        assert_eq!(peer.alive.load(Ordering::Acquire), 0);
    }
    assert_no_command_bytes(route);
}

fn click_connect(fixture: &Fixture, id: EntityId, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(("restored-tab", id)).visible());
        window.click(("connect-restored-tab", id), cx);
    })
    .checked("explicitly connect the displayed restored placeholder");
}

async fn authenticate_pinned(fixture: &Fixture, route: &RouteFixture, cx: &mut TestAppContext) {
    for id in [route.gateway.id, route.target.id] {
        login_for(fixture, id, cx).await;
        fixture.workspace.read_with(cx, |view, cx| {
            assert!(
                view.login
                    .as_ref()
                    .checked_option("fresh per-hop login")
                    .secret
                    .read(cx)
                    .value()
                    .is_empty()
            );
        });
        submit_secret(fixture, id, cx);
    }
}

async fn ready_replacement(
    fixture: &Fixture,
    previous: EntityId,
    stable: uuid::Uuid,
    cx: &mut TestAppContext,
) -> EntityId {
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let view = fixture.workspace.read(cx);
        !view.saving
            && view.connect_route.is_none()
            && view.tabs.iter().any(|terminal| {
                let id = terminal.entity_id();
                id != previous
                    && view.pane_tab_ids.get(&id) == Some(&stable)
                    && terminal.read(cx).is_open()
                    && terminal.read(cx).visible_text().contains("TARGET READY")
            })
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        view.tabs
            .iter()
            .map(Entity::entity_id)
            .find(|id| view.pane_tab_ids.get(id) == Some(&stable))
            .checked_option("authenticated replacement at the saved slot")
    })
}

async fn close_sessions(fixture: &Fixture, route: &RouteFixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let sessions: Vec<_> = view.remote_sessions.keys().copied().collect();
            for id in sessions {
                view.close_session_tab(id, window, cx);
            }
        });
    })
    .checked("retire only this fixture's authenticated sessions");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn startup_renders_only_the_saved_active_window_without_ssh_or_runtime_authority(
    cx: &mut TestAppContext,
) {
    let (mut fixture, route) = mount_route(cx, false, false);
    let snapshot = saved_layout(route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let before = std::fs::read(fixture.store.path()).checked("capture startup metadata bytes");
    let ids = tab_ids(&fixture, cx);
    assert_eq!(ids.len(), 4);
    assert_saved_positions(&fixture, &snapshot.windows[1], &ids, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(window.find(("restored-tab", ids[1])).visible());
        assert!(window.find(("restored-tab", ids[3])).visible());
        fixture.workspace.update(cx, |view, cx| {
            assert_eq!(view.restored_tabs.len(), 4);
            assert_eq!(view.session_panes.active_session(), Some(ids[3]));
            for terminal in &view.tabs {
                assert!(!terminal.read(cx).is_open());
                assert!(terminal.read(cx).visible_text().trim().is_empty());
            }
            assert!(view.remote_sessions.is_empty());
            assert!(view.remote_hosts.is_empty());
            assert!(view.remote_connection_ids.is_empty());
            assert!(view.reconnect_bindings.is_empty());
            assert!(view.panels.is_empty());
            assert!(view.archived_panels.is_empty());
            assert!(view.tunnel_owners.is_empty());
            assert!(view.command_histories.is_empty());
            assert!(view.command.read(cx).value().is_empty());
            assert!(view.command_target.is_none());
            assert!(view.suggestion_request.is_none());
            assert!(!view.suggestion_job_running);
            assert!(!view.remote_completion.busy());
            assert!(view.remote_completion.choices.is_empty());
            assert!(view.batch_panel.is_none());
            assert!(view.workflow_panel.is_none());
            assert!(view.agent.is_none());
            assert!(view.knowledge_source_ids.is_empty());
            assert!(view.knowledge_source_revisions.is_empty());
            assert!(!view.mcp_test_enabled());
            assert!(view.mcp_test_client().is_err());
            assert!(view.mcp_test_workers_finished());
            assert!(view.mcp.config_recovery_idle());
            assert_eq!(
                view.assistant.read(cx).captured_context_for_test(),
                ("", "", "")
            );
            assert!(
                view.candidate(
                    "must-not-be-reviewed".into(),
                    "inert placeholder".into(),
                    SuggestionSource::History,
                    cx,
                )
                .is_none()
            );
            view.request_restored_connect(ids[0], window, cx);
            view.request_restored_connect(ids[2], window, cx);
            assert!(view.connect_route.is_none());
        });
        for (index, label) in [(0, "未保存连接占位"), (2, "连接缺失或在回收站")] {
            fixture
                .workspace
                .update(cx, |view, cx| view.select_session_tab(index, window, cx));
            window.render_frame(cx);
            assert_eq!(
                window.find(("restored-tab-state", ids[index])).label(),
                Some(label)
            );
            assert!(
                window
                    .try_find(("connect-restored-tab", ids[index]))
                    .is_none()
            );
        }
    })
    .checked("render inert, unbound and missing references without executable targets");
    // Let the normal reconnect poll cross the saved policy's first retry delay.
    cx.background_executor
        .timer(Duration::from_millis(1200))
        .await;
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |view, _| {
        assert!(!view.connecting);
        assert!(view.connect_route.is_none());
        assert!(view.login.is_none());
        assert!(view.host_approval.is_none());
        assert!(view.reconnect_bindings.is_empty());
        assert!(view.state.recent_connections.is_empty());
        assert_eq!(view.state.workspace_layout.as_ref(), Some(&snapshot));
    });
    assert_no_connections(&route);
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read metadata after startup and selection"),
        before
    );
}

#[gpui_kit::test]
async fn restored_connect_requires_fresh_per_hop_trust_and_replaces_only_its_saved_slot(
    cx: &mut TestAppContext,
) {
    let (mut fixture, route) = mount_route(cx, false, false);
    let snapshot = saved_layout(route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let mut ids = tab_ids(&fixture, cx);
    let old = ids[1];
    assert_no_connections(&route);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.set_reviewed_command(
                "never-reuse-placeholder-target".into(),
                Some(old),
                window,
                cx,
            );
        });
    })
    .checked("retain an unsent draft tied to the inert runtime entity");
    click_connect(&fixture, old, cx);
    fixture.workspace.read_with(cx, |view, _| {
        let target = view
            .connect_route
            .as_ref()
            .and_then(|route| route.restored)
            .checked_option("restored route ownership");
        assert_eq!(target.entity, old);
        assert_eq!(target.layout_tab, snapshot.windows[1].panes[0].tabs[1].id);
        assert_eq!(target.connection, route.target.id);
        assert!(view.reconnect_bindings.is_empty());
    });
    for (index, profile) in [&route.gateway, &route.target].into_iter().enumerate() {
        login_for(&fixture, profile.id, cx).await;
        fixture.workspace.read_with(cx, |view, cx| {
            assert!(
                view.login
                    .as_ref()
                    .checked_option("new login")
                    .secret
                    .read(cx)
                    .value()
                    .is_empty()
            );
        });
        submit_secret(&fixture, profile.id, cx);
        cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
            fixture.workspace.read(cx).host_approval.is_some()
        })
        .await;
        fixture.workspace.read_with(cx, |view, _| {
            let approval = view
                .host_approval
                .as_ref()
                .checked_option("unknown host review");
            assert_eq!(approval.connection.id, profile.id);
            assert_eq!(approval.fingerprint, route.pins[index]);
            assert!(approval.previous.is_none());
            assert_eq!(view.tabs[1].entity_id(), old);
            assert!(view.remote_sessions.is_empty());
        });
        assert_eq!(
            [&route.gateway_state, &route.target_state][index]
                .authenticated
                .load(Ordering::Acquire),
            0
        );
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("accept-host-key", cx);
        })
        .checked("approve the current hop's displayed identity");
        login_for(&fixture, profile.id, cx).await;
        fixture.workspace.read_with(cx, |view, cx| {
            assert!(
                view.login
                    .as_ref()
                    .checked_option("login after trust")
                    .secret
                    .read(cx)
                    .value()
                    .is_empty()
            );
        });
        submit_secret(&fixture, profile.id, cx);
    }
    let new = ready_replacement(&fixture, old, snapshot.windows[1].panes[0].tabs[1].id, cx).await;
    ids[1] = new;
    assert_saved_positions(&fixture, &snapshot.windows[1], &ids, cx);
    fixture.workspace.read_with(cx, |view, cx| {
        assert_ne!(new, old);
        assert_eq!(view.restored_tabs.len(), 3);
        assert!(!view.restored_tabs.contains_key(&old));
        assert!(!view.pane_tab_ids.contains_key(&old));
        assert!(!view.remote_sessions.contains_key(&old));
        assert!(!view.reconnect_bindings.contains_key(&old));
        assert_eq!(view.reconnect_bindings.len(), 1);
        assert_eq!(view.remote_connection_ids[&new], route.target.id);
        assert!(view.command_histories[&new].newest_first().next().is_none());
        assert!(!view.command_histories.contains_key(&old));
        assert!(view.archived_panels.is_empty());
        assert!(view.agent.is_none());
        assert!(!view.mcp_test_enabled());
        assert_eq!(view.command_target, Some(old));
        assert_eq!(
            view.command.read(cx).value(),
            "never-reuse-placeholder-target"
        );
        assert_eq!(view.session_panes.active_session(), Some(ids[3]));
        assert_eq!(
            view.assistant.read(cx).captured_context_for_test(),
            ("", "", "")
        );
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.select_session_tab(1, window, cx);
            view.run_command(window, cx);
            assert_eq!(view.command_target, Some(old));
            assert_eq!(
                view.command.read(cx).value(),
                "never-reuse-placeholder-target"
            );
            assert!(view.command_histories[&new].newest_first().next().is_none());
        });
    })
    .checked("the new runtime entity cannot execute the retired placeholder's draft");
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.gateway_state.opened.load(Ordering::Acquire), 2);
    assert_no_command_bytes(&route);
    assert_eq!(
        fixture
            .store
            .load()
            .checked("read persisted layout after authentication")
            .workspace_layout,
        Some(snapshot)
    );
    close_sessions(&fixture, &route, cx).await;
    assert_no_command_bytes(&route);
}

#[gpui_kit::test]
async fn restored_connect_resolves_the_profile_saved_after_startup_at_action_time(
    cx: &mut TestAppContext,
) {
    let (mut fixture, previous_route) = mount_route(cx, false, true);
    let snapshot = saved_layout(previous_route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let ids = tab_ids(&fixture, cx);
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let mut current = route_fixture(&runtime, false);
    current.target.id = previous_route.target.id;
    current.target.name = "changed current route".into();
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let mut state = view.state.clone();
            state.connections.push(current.gateway.clone());
            state
                .update_connection(current.target.clone())
                .checked("change saved target route");
            let resolved = state
                .connection_route(current.target.id)
                .checked("resolve current route");
            for (index, pin) in current.pins.iter().enumerate() {
                state
                    .trust_host_key_for_scope(
                        &resolved
                            .host_key_scope(index)
                            .checked_option("current scope"),
                        pin,
                    )
                    .checked("pin the current fixture route");
            }
            view.state = fixture
                .store
                .save(&state)
                .checked("persist current profile before Connect");
            cx.notify();
        });
    })
    .checked("change the profile without replacing the restored placeholder");
    assert_eq!(
        fixture
            .store
            .load()
            .checked("read the changed profile before connecting")
            .connections
            .iter()
            .find(|profile| profile.id == current.target.id),
        Some(&current.target)
    );
    assert_no_connections(&previous_route);
    assert_no_connections(&current);
    click_connect(&fixture, ids[1], cx);
    fixture.workspace.read_with(cx, |view, cx| {
        let (hops, index) = view
            .route_progress(cx)
            .checked_option("current saved route");
        assert_eq!(index, 0);
        assert_eq!(hops, vec![current.gateway.clone(), current.target.clone()]);
        assert_eq!(view.tabs[1].entity_id(), ids[1]);
    });
    authenticate_pinned(&fixture, &current, cx).await;
    let new = ready_replacement(
        &fixture,
        ids[1],
        snapshot.windows[1].panes[0].tabs[1].id,
        cx,
    )
    .await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.tabs[1].entity_id(), new);
        assert_eq!(view.tabs[1].read(cx).title, current.target.name);
        assert_eq!(view.remote_connection_ids[&new], current.target.id);
    });
    assert_no_connections(&previous_route);
    assert_eq!(
        current.gateway_state.authenticated.load(Ordering::Acquire),
        1
    );
    assert_eq!(
        current.target_state.authenticated.load(Ordering::Acquire),
        1
    );
    assert_no_command_bytes(&current);
    close_sessions(&fixture, &current, cx).await;
    assert_no_command_bytes(&current);
}

async fn reject_changed_profile_during_authentication(trash: bool, cx: &mut TestAppContext) {
    let (mut fixture, route) = mount_route(cx, true, true);
    let snapshot = saved_layout(route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let ids = tab_ids(&fixture, cx);
    click_connect(&fixture, ids[1], cx);
    authenticate_pinned(&fixture, &route, cx).await;
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.target_state.authenticated.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.connecting);
            let mut state = view.state.clone();
            if trash {
                state
                    .soft_delete_connection(route.target.id, 1)
                    .checked("trash authenticating profile");
            } else {
                let mut gateway = route.gateway.clone();
                gateway.username = "changed-during-authentication".into();
                state
                    .update_connection(gateway)
                    .checked("change authenticating route");
            }
            view.state = fixture
                .store
                .save(&state)
                .checked("save changed profile while reply is gated");
            cx.notify();
        });
    })
    .checked("invalidate the route while the real server holds its authentication reply");
    route
        .target_state
        .gate
        .as_ref()
        .checked_option("target authentication gate")
        .notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let view = fixture.workspace.read(cx);
        !view.connecting
            && view.connect_route.is_none()
            && route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
    assert_saved_positions(&fixture, &snapshot.windows[1], &ids, cx);
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.restored_tabs.len(), ids.len());
        assert!(
            view.tabs
                .iter()
                .all(|terminal| !terminal.read(cx).is_open())
        );
        assert!(view.remote_sessions.is_empty());
        assert!(view.reconnect_bindings.is_empty());
        assert!(view.command_histories.is_empty());
        assert!(view.panels.is_empty());
        assert!(view.login.is_none());
        assert!(view.host_approval.is_none());
        assert!(view.state.recent_connections.is_empty());
        assert_eq!(view.state.workspace_layout.as_ref(), Some(&snapshot));
    });
    if trash {
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("connect-restored-tab", ids[1])).is_none());
            fixture.workspace.update(cx, |view, cx| {
                view.request_restored_connect(ids[1], window, cx);
                assert!(view.connect_route.is_none());
            });
        })
        .checked("a stale Connect action cannot revive a trashed profile");
    }
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 1);
    assert_no_command_bytes(&route);
    let saved = fixture
        .store
        .load()
        .checked("read metadata after rejected completion");
    assert!(saved.recent_connections.is_empty());
    assert_eq!(saved.workspace_layout, Some(snapshot));
}

#[gpui_kit::test]
async fn restored_authentication_discards_late_success_when_profile_is_trashed(
    cx: &mut TestAppContext,
) {
    reject_changed_profile_during_authentication(true, cx).await;
}

#[gpui_kit::test]
async fn restored_authentication_discards_late_success_when_upstream_profile_changes(
    cx: &mut TestAppContext,
) {
    reject_changed_profile_during_authentication(false, cx).await;
}

#[gpui_kit::test]
async fn restored_connect_preserves_another_live_session_tools_and_reviewed_command_target(
    cx: &mut TestAppContext,
) {
    let (mut fixture, route) = mount_route(cx, false, true);
    let snapshot = saved_layout(route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let mut ids = tab_ids(&fixture, cx);
    click_connect(&fixture, ids[1], cx);
    authenticate_pinned(&fixture, &route, cx).await;
    ids[1] = ready_replacement(
        &fixture,
        ids[1],
        snapshot.windows[1].panes[0].tabs[1].id,
        cx,
    )
    .await;
    let live = ids[1];
    let monitor = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.select_session_tab(1, window, cx);
                view.toggle_panel(ToolPanel::Commands, window, cx);
                view.set_reviewed_command(
                    "keep-other-session-review".into(),
                    Some(live),
                    window,
                    cx,
                );
                view.assistant.update(cx, |assistant, cx| {
                    assistant.set_context(
                        "live-context".into(),
                        "fixture".into(),
                        format!("{live:?}"),
                        cx,
                    )
                });
                view.panels[&live]
                    .monitor
                    .as_ref()
                    .checked_option("live monitor")
                    .entity_id()
            })
        })
        .checked("retain the live session's tools, AI context and unsent review");
    let old = ids[3];
    click_connect(&fixture, old, cx);
    authenticate_pinned(&fixture, &route, cx).await;
    ids[3] = ready_replacement(&fixture, old, snapshot.windows[1].panes[1].tabs[1].id, cx).await;
    let mut expected = snapshot.windows[1].clone();
    expected
        .layout
        .set_active(expected.panes[0].pane)
        .checked("expected active live pane");
    assert_saved_positions(&fixture, &expected, &ids, cx);
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.remote_sessions.len(), 2);
        assert_eq!(view.session_panes.active_session(), Some(live));
        assert!(view.visible_panel == Some(ToolPanel::Commands));
        assert_eq!(
            view.panels[&live]
                .monitor
                .as_ref()
                .checked_option("retained monitor")
                .entity_id(),
            monitor
        );
        assert_eq!(view.command_target, Some(live));
        assert_eq!(view.command.read(cx).value(), "keep-other-session-review");
        assert_eq!(
            view.assistant.read(cx).captured_context_for_test(),
            ("live-context", "fixture", format!("{live:?}").as_str())
        );
        assert!(
            view.command_histories
                .values()
                .all(|history| history.newest_first().next().is_none())
        );
        assert!(!view.mcp_test_enabled());
        assert!(view.agent.is_none());
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.select_session_tab(3, window, cx);
            view.run_command(window, cx);
            assert_eq!(view.command_target, Some(live));
            assert_eq!(view.command.read(cx).value(), "keep-other-session-review");
            assert!(
                view.command_histories[&ids[3]]
                    .newest_first()
                    .next()
                    .is_none()
            );
        });
    })
    .checked("the second restored session cannot consume the first session's command review");
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 2);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 2);
    assert_no_command_bytes(&route);
    close_sessions(&fixture, &route, cx).await;
    assert_no_command_bytes(&route);
}

#[gpui_kit::test]
async fn restored_connect_replaces_a_slot_at_both_pane_and_workspace_capacity(
    cx: &mut TestAppContext,
) {
    let (mut fixture, route) = mount_route(cx, false, true);
    let mut snapshot = saved_layout(route.target.id);
    snapshot.windows.remove(0);
    let saved = &mut snapshot.windows[0];
    for primary in [saved.panes[0].pane, saved.panes[1].pane] {
        let new = PaneNodeId(uuid::Uuid::new_v4());
        saved
            .layout
            .split(
                primary,
                new,
                PaneNodeId(uuid::Uuid::new_v4()),
                SplitAxis::Vertical,
                5000,
            )
            .checked("create four capacity-bounded saved leaves");
        saved.panes.push(LayoutPane {
            pane: new,
            tabs: Vec::new(),
            active_tab: None,
        });
    }
    for pane in &mut saved.panes {
        pane.tabs = (0..MAX_LAYOUT_TABS_PER_PANE)
            .map(|_| LayoutTab {
                id: uuid::Uuid::new_v4(),
                connection_id: Some(route.target.id),
            })
            .collect();
        pane.active_tab = Some(pane.tabs[17].id);
    }
    saved
        .layout
        .set_active(saved.panes[0].pane)
        .checked("select the full first leaf");
    snapshot.validate().checked("full workspace snapshot");
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let mut ids = tab_ids(&fixture, cx);
    assert_eq!(ids.len(), MAX_LAYOUT_TABS);
    let old = ids[17];
    click_connect(&fixture, old, cx);
    authenticate_pinned(&fixture, &route, cx).await;
    ids[17] = ready_replacement(&fixture, old, snapshot.windows[0].panes[0].tabs[17].id, cx).await;
    assert_saved_positions(&fixture, &snapshot.windows[0], &ids, cx);
    fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.tabs.len(), MAX_LAYOUT_TABS);
        assert_eq!(view.restored_tabs.len(), MAX_LAYOUT_TABS - 1);
        assert_eq!(view.remote_sessions.len(), 1);
    });
    assert_no_command_bytes(&route);
    close_sessions(&fixture, &route, cx).await;
}

#[gpui_kit::test]
async fn cancelled_restored_authentication_cannot_recreate_a_closed_placeholder(
    cx: &mut TestAppContext,
) {
    let (mut fixture, route) = mount_route(cx, true, true);
    let snapshot = saved_layout(route.target.id);
    reopen_from_disk(&mut fixture, &snapshot, cx);
    let ids = tab_ids(&fixture, cx);
    let old = ids[1];
    let stable = snapshot.windows[1].panes[0].tabs[1].id;
    click_connect(&fixture, old, cx);
    authenticate_pinned(&fixture, &route, cx).await;
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.target_state.authenticated.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-connect-route", cx);
        fixture.workspace.update(cx, |view, cx| {
            view.close_session_tab(old, window, cx);
            assert!(!view.restored_tabs.contains_key(&old));
            assert!(!view.pane_tab_ids.contains_key(&old));
        });
    })
    .checked("cancel exactly this attempt and close its now-inert tab");
    route
        .target_state
        .gate
        .as_ref()
        .checked_option("held authentication reply")
        .notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.tabs.len(), ids.len() - 1);
        assert!(view.tabs.iter().all(|tab| !tab.read(cx).is_open()));
        assert!(!view.pane_tab_ids.values().any(|id| *id == stable));
        assert_eq!(view.session_panes.active_session(), Some(ids[3]));
        assert!(view.remote_sessions.is_empty());
        assert!(view.reconnect_bindings.is_empty());
        assert!(view.connect_route.is_none());
        assert!(view.state.recent_connections.is_empty());
    });
    assert_no_command_bytes(&route);
}
