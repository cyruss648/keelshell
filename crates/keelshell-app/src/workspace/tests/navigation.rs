//! Production GPUI pointer routing with isolated transports; not desktop acceptance.
use super::*;
use crate::terminal::TransportState;
use crate::workspace::{library::LibraryFilter, navigation::NavigationPanel};
use gpui_kit::{App, MouseButton, Pixels, Point, SharedString, Window};

mod input_lifetime;
mod keyboard;
mod selection_recovery;

fn row_id(id: uuid::Uuid) -> SharedString {
    format!("navigation-connection-{id}").into()
}

fn folder_id(id: uuid::Uuid) -> SharedString {
    format!("navigation-folder-{id}").into()
}

fn saved_profile(name: &str) -> Connection {
    let mut profile = password_profile();
    profile.name = name.into();
    profile
}

fn navigation_fixture(
    cx: &mut TestAppContext,
    profiles: Vec<Connection>,
) -> (Fixture, Vec<RemotePane>) {
    let fixture = mount_sized(cx, profiles, 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.navigation_panel = NavigationPanel::Connections;
            cx.notify();
        });
    })
    .checked("select connection navigation with existing fixture transports");
    (fixture, panes)
}

fn click_painted_position(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    // Do not render between foreground mutation and activation: the listeners
    // must still belong to the old painted row.
    for event in [
        gpui_kit::MouseDownEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
    }
}

#[gpui_kit::test]
fn navigation_resolves_reordered_live_tabs_and_preserves_drafts_across_panels(
    cx: &mut TestAppContext,
) {
    let first = saved_profile("主机 A");
    let second = saved_profile("主机 B");
    let second_id = second.id;
    let (fixture, panes) = navigation_fixture(cx, vec![first.clone(), second.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            for (pane, profile) in panes.iter().zip([&first, &second]) {
                workspace
                    .remote_connection_ids
                    .insert(pane.terminal.entity_id(), profile.id);
            }
            workspace.library_filter = LibraryFilter::Trash;
            workspace.command.update(cx, |input, cx| {
                input.set_value("printf '未发送草稿'", window, cx);
            });
            workspace.quick_connect.host.update(cx, |input, cx| {
                input.set_value("unsaved.example.invalid", window, cx);
            });
            cx.notify();
        });
        window.render_frame(cx);
        let position = window.find(row_id(second_id)).bounds().center();
        fixture.workspace.update(cx, |workspace, _| {
            workspace.tabs.swap(0, 1);
            workspace.active = 1;
        });
        click_painted_position(position, window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.active, 0);
        assert_eq!(
            workspace.tabs[workspace.active].entity_id(),
            panes[1].terminal.entity_id()
        );
        assert!(
            panes[1]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(workspace.login.is_none() && workspace.connect_route.is_none());
        let command_entity = workspace.command.entity_id();
        let assistant_entity = workspace.assistant.entity_id();
        let search_entity = workspace.search.entity_id();

        for (language, selected) in [
            (Language::ZhCn, "当前已连接会话"),
            (Language::En, "Current connected session"),
        ] {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            let row = window.find(row_id(second_id));
            assert!(row.visible());
            let label = row
                .label()
                .checked_option("complete navigation AX identity");
            assert!(label.contains("主机 B"));
            assert!(label.contains("fixture-user@127.0.0.1:22"));
            assert!(label.contains(selected));
            let rail = window.find("navigation-column").bounds();
            assert!(row.bounds().left() >= rail.left());
            assert!(row.bounds().right() <= rail.right());
            window.click("navigation-monitor", cx);
            window.render_frame(cx);
            assert_eq!(
                fixture.workspace.read(cx).navigation_panel,
                NavigationPanel::Monitor
            );
            assert!(window.try_find("navigation-rows").is_none());
            window.click("navigation-connections", cx);
            window.render_frame(cx);
            window.click(row_id(second_id), cx);
            let workspace = fixture.workspace.read(cx);
            assert_eq!(workspace.active, 0);
            assert_eq!(workspace.library_filter, LibraryFilter::Trash);
            assert_eq!(workspace.command.entity_id(), command_entity);
            assert_eq!(workspace.assistant.entity_id(), assistant_entity);
            assert_eq!(workspace.search.entity_id(), search_entity);
            assert_eq!(workspace.command.read(cx).value(), "printf '未发送草稿'");
            assert_eq!(
                workspace.quick_connect.host.read(cx).value(),
                "unsaved.example.invalid"
            );
        }
    })
    .checked("activate current UUID owners and switch bilingual navigation panels");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    assert_eq!(
        fixture
            .store
            .load()
            .checked("read unchanged fixture profiles")
            .connections,
        vec![first, second]
    );
}

#[gpui_kit::test]
fn navigation_search_keeps_folder_ancestors_and_manager_filter_then_opens_real_forms(
    cx: &mut TestAppContext,
) {
    let parent_profile = saved_profile("Alpha");
    let nested_profile = saved_profile("Beta");
    let loose_profile = saved_profile("Loose");
    let (fixture, panes) = navigation_fixture(
        cx,
        vec![
            parent_profile.clone(),
            nested_profile.clone(),
            loose_profile.clone(),
        ],
    );
    cx.update_window(fixture.window, |_, window, cx| {
        let (root, child) = fixture.workspace.update(cx, |workspace, cx| {
            let root = workspace
                .state
                .create_folder("Production", None)
                .checked("create parent folder");
            let child = workspace
                .state
                .create_folder("Database", Some(root))
                .checked("create child folder");
            workspace
                .state
                .move_connection(parent_profile.id, Some(root))
                .checked("file parent profile");
            workspace
                .state
                .move_connection(nested_profile.id, Some(child))
                .checked("file nested profile");
            workspace.library_filter = LibraryFilter::Trash;
            cx.notify();
            (root, child)
        });
        window.render_frame(cx);
        window.click("navigation-search", cx);
        window.input("Beta", cx);
        window.render_frame(cx);
        assert!(window.find(folder_id(root)).visible());
        assert!(window.find(folder_id(child)).visible());
        assert!(window.find(row_id(nested_profile.id)).visible());
        assert!(window.try_find(row_id(parent_profile.id)).is_none());
        assert!(window.try_find(row_id(loose_profile.id)).is_none());
        assert_eq!(
            window.find(folder_id(child)).label(),
            Some("文件夹: Production/Database")
        );
        assert!(
            window.find(folder_id(child)).bounds().left()
                <= window.find(row_id(nested_profile.id)).bounds().left()
        );
        let search_entity = fixture.workspace.read(cx).search.entity_id();
        window.click("navigation-manage-connections", cx);
        window.render_frame(cx);
        assert!(window.find("connection-manager-dialog").visible());
        assert!(window.try_find("navigation-column").is_none());
        assert_eq!(fixture.workspace.read(cx).search.entity_id(), search_entity);
        assert_eq!(
            fixture.workspace.read(cx).library_filter,
            LibraryFilter::Trash
        );
        window.click("close-manager", cx);
        window.render_frame(cx);
        assert_eq!(window.find("navigation-search").value(), Some("Beta"));
        window.click("navigation-add-connection", cx);
        window.render_frame(cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .form
                .as_ref()
                .checked_option("real connection editor")
                .id
                .is_none()
        );
        assert!(window.try_find("navigation-column").is_none());
        window.click("cancel-connection", cx);
        window.render_frame(cx);
        window.click("navigation-search", cx);
        window.input(" no-match", cx);
        window.render_frame(cx);
        assert!(window.find("navigation-empty").visible());
        assert!(window.try_find(row_id(nested_profile.id)).is_none());
        assert!(window.find("navigation-add-connection").visible());
        assert!(window.find("navigation-manage-connections").visible());
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("search through the tree and reuse the isolated manager and editor");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn painted_navigation_row_cannot_connect_a_deleted_uuid_or_its_replacement(
    cx: &mut TestAppContext,
) {
    let original = saved_profile("Same visible name");
    let replacement = saved_profile("Same visible name");
    let (fixture, panes) = navigation_fixture(cx, vec![original.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find(row_id(original.id)).bounds().center();
        fixture.workspace.update(cx, |workspace, _| {
            workspace.state.connections = vec![replacement.clone()];
            // Even an existing tab with the removed UUID cannot resurrect it.
            workspace
                .remote_connection_ids
                .insert(panes[1].terminal.entity_id(), original.id);
        });
        click_painted_position(position, window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.active, 0);
        assert_eq!(workspace.tabs.len(), 2);
        assert!(workspace.login.is_none());
        assert!(workspace.connect_route.is_none());
        assert!(!workspace.connecting);
        assert_eq!(workspace.state.connections, vec![replacement.clone()]);
        assert_eq!(workspace.status.render(cx), "连接已移除，请重新选择。");
    })
    .checked("reject a painted row whose UUID is no longer saved");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn painted_navigation_row_rechecks_save_guard_and_uses_the_current_saved_destination(
    cx: &mut TestAppContext,
) {
    let original = saved_profile("Before edit");
    let mut current = original.clone();
    current.name = "After edit".into();
    current.host = "192.0.2.42".into();
    current.port = 2222;
    current.username = "current-user".into();
    let (fixture, panes) = navigation_fixture(cx, vec![original.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find(row_id(original.id)).bounds().center();
        fixture.workspace.update(cx, |workspace, _| {
            workspace.state.connections[0] = current.clone();
            workspace.saving = true;
        });
        click_painted_position(position, window, cx);
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.login.is_none() && workspace.connect_route.is_none());
        fixture.workspace.update(cx, |workspace, _| {
            workspace.saving = false;
        });
        click_painted_position(position, window, cx);
        let workspace = fixture.workspace.read(cx);
        let login = workspace
            .login
            .as_ref()
            .checked_option("normal password guard");
        assert_eq!(login.connection, current);
        assert!(!workspace.connecting);
        assert_eq!(workspace.tabs.len(), 2);
        let (hops, _) = workspace
            .route_progress(cx)
            .checked_option("current saved route");
        assert_eq!(hops.last(), Some(&current));
    })
    .checked("activate a stale painted row through current persistence and authentication guards");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn painted_connected_row_rechecks_transport_end_before_reusing_a_tab(cx: &mut TestAppContext) {
    for explicit_end in [false, true] {
        let profile = saved_profile("Previously connected");
        let (fixture, panes) = navigation_fixture(cx, vec![profile.clone()]);
        let (lifecycle, source) = tokio::sync::watch::channel(TransportState::Ready);
        cx.update_window(fixture.window, |_, window, cx| {
            panes[0].terminal.update(cx, |terminal, _| {
                terminal.attach_lifecycle(source);
            });
            fixture.workspace.update(cx, |workspace, cx| {
                workspace
                    .remote_connection_ids
                    .insert(panes[0].terminal.entity_id(), profile.id);
                cx.notify();
            });
            window.render_frame(cx);
            let row = window.find(row_id(profile.id));
            assert!(
                row.label()
                    .checked_option("connected row label")
                    .contains("已连接")
            );
            let position = row.bounds().center();
            if explicit_end {
                lifecycle.send_replace(TransportState::Ended(
                    keelshell_session::ShellEnd::ChannelClosed,
                ));
            }
            drop(lifecycle);
            click_painted_position(position, window, cx);
            let workspace = fixture.workspace.read(cx);
            assert_eq!(
                workspace
                    .login
                    .as_ref()
                    .checked_option("closed transport requires normal connection review")
                    .connection
                    .id,
                profile.id
            );
            assert!(workspace.connect_route.is_some());
            assert_eq!(workspace.tabs.len(), 2);
            assert!(!workspace.connecting);
        })
        .checked("observe typed closure or a dropped lifecycle producer before the next poll");
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
fn painted_connected_row_cannot_reuse_a_session_for_an_edited_route(cx: &mut TestAppContext) {
    let target = saved_profile("Current saved target");
    let gateway = saved_profile("Gateway");
    for edit_gateway in [false, true] {
        let mut target = target.clone();
        if edit_gateway {
            target.jump_host = Some(gateway.id);
        }
        let (fixture, panes) = navigation_fixture(cx, vec![target.clone(), gateway.clone()]);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, _| {
                let route = workspace
                    .state
                    .connection_route(target.id)
                    .checked("snapshot original destination and gateway");
                let tab = panes[1].terminal.entity_id();
                workspace.remote_connection_ids.insert(tab, target.id);
                workspace.bind_remote_tab(tab, route);
            });
            window.render_frame(cx);
            let position = window.find(row_id(target.id)).bounds().center();
            fixture.workspace.update(cx, |workspace, _| {
                let id = if edit_gateway { gateway.id } else { target.id };
                workspace
                    .state
                    .connections
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .checked_option("current saved hop")
                    .host = "192.0.2.43".into();
            });
            click_painted_position(position, window, cx);
            let workspace = fixture.workspace.read(cx);
            assert_eq!(workspace.active, 0, "old shell is not the updated route");
            assert_eq!(workspace.tabs.len(), 2);
            let (hops, _) = workspace
                .route_progress(cx)
                .checked_option("updated route entered normal authentication");
            let updated = if edit_gateway {
                hops.first()
            } else {
                hops.last()
            }
            .checked_option("changed hop");
            assert_eq!(updated.host, "192.0.2.43");
            assert!(workspace.login.is_some());
            assert!(!workspace.connecting);
        })
        .checked("recheck destination and gateway before reusing a connected UUID");
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
fn navigation_scroll_reaches_saved_rows_while_add_and_manage_remain_visible(
    cx: &mut TestAppContext,
) {
    let profiles: Vec<_> = (0..18)
        .map(|index| saved_profile(&format!("Host {index:02} with a long saved name")))
        .collect();
    let target = profiles.last().checked_option("last saved row").id;
    let (fixture, panes) = navigation_fixture(cx, profiles);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        for _ in 0..48 {
            let viewport = window.find("navigation-rows").bounds();
            let row = window.find(row_id(target));
            if row.visible()
                && row.bounds().top() >= viewport.top()
                && row.bounds().bottom() <= viewport.bottom()
            {
                break;
            }
            window.scroll("navigation-rows", ScrollDelta::Lines(point(0., -3.)), cx);
            window.render_frame(cx);
        }
        let viewport = window.find("navigation-rows").bounds();
        let row = window.find(row_id(target));
        assert!(row.visible());
        assert!(row.bounds().top() >= viewport.top() && row.bounds().bottom() <= viewport.bottom());
        for id in ["navigation-add-connection", "navigation-manage-connections"] {
            let action = window.find(id);
            assert!(action.visible());
            assert!(action.bounds().bottom() <= window.bounds().bottom());
        }
        window.click(row_id(target), cx);
        assert_eq!(
            fixture
                .workspace
                .read(cx)
                .login
                .as_ref()
                .checked_option("connect the scrolled saved row")
                .connection
                .id,
            target
        );
    })
    .checked("scroll the real rail and activate its final saved profile");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
