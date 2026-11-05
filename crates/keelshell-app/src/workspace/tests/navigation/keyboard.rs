//! Dispatch production keyboard events; no direct calls to navigation handlers.
use super::*;
use gpui_kit::{KeyDownEvent, KeyUpEvent, Keystroke};

pub(super) fn press(window: &mut Window, cx: &mut App, key: &str) {
    window.press(key, cx);
    window.render_frame(cx);
    window.render_frame(cx);
}

pub(super) fn key_down(window: &mut Window, cx: &mut App, key: &'static str, held: bool) {
    window.dispatch_event(
        KeyDownEvent {
            keystroke: Keystroke::parse(key).checked("test keystroke"),
            is_held: held,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
}

pub(super) fn key_up(window: &mut Window, cx: &mut App, key: &'static str) {
    window.dispatch_event(
        KeyUpEvent {
            keystroke: Keystroke::parse(key).checked("test keystroke"),
        }
        .to_platform_input(),
        cx,
    );
}

#[gpui_kit::test]
fn keyboard_tree_navigates_parents_children_and_empty_folders_without_connecting(
    cx: &mut TestAppContext,
) {
    let alpha = saved_profile("Alpha");
    let beta = saved_profile("Beta");
    let (fixture, panes) = navigation_fixture(cx, vec![alpha.clone(), beta.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        let (root, child, empty) = fixture.workspace.update(cx, |view, cx| {
            let root = view.state.create_folder("Production", None).checked("root");
            let child = view
                .state
                .create_folder("Database", Some(root))
                .checked("child");
            let empty = view
                .state
                .create_folder("Empty", Some(root))
                .checked("empty");
            view.state
                .move_connection(alpha.id, Some(root))
                .checked("file alpha");
            view.state
                .move_connection(beta.id, Some(child))
                .checked("file beta");
            cx.notify();
            (root, child, empty)
        });
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        assert_eq!(window.find("navigation-rows").focused(), Some(true));
        assert_eq!(window.find(folder_id(root)).selected(), Some(true));
        assert_eq!(window.find(folder_id(root)).expanded(), Some(true));
        press(window, cx, "right");
        assert_eq!(window.find(row_id(alpha.id)).selected(), Some(true));
        press(window, cx, "down");
        assert_eq!(window.find(folder_id(child)).selected(), Some(true));
        press(window, cx, "right");
        assert_eq!(window.find(row_id(beta.id)).selected(), Some(true));
        press(window, cx, "left");
        assert_eq!(window.find(folder_id(child)).selected(), Some(true));
        press(window, cx, "left");
        assert_eq!(window.find(folder_id(child)).expanded(), Some(false));
        assert!(window.try_find(row_id(beta.id)).is_none());
        press(window, cx, "left");
        assert_eq!(window.find(folder_id(root)).selected(), Some(true));
        press(window, cx, "left");
        assert!(window.try_find(folder_id(child)).is_none());
        press(window, cx, "enter");
        assert_eq!(window.find(folder_id(root)).expanded(), Some(true));
        assert_eq!(window.find(folder_id(child)).expanded(), Some(false));
        press(window, cx, "end");
        assert_eq!(window.find(folder_id(empty)).selected(), Some(true));
        assert_eq!(window.find(folder_id(empty)).expanded(), None);
        press(window, cx, "right");
        press(window, cx, "enter");
        assert_eq!(window.find(folder_id(empty)).selected(), Some(true));
        press(window, cx, "home");
        press(window, cx, "up");
        assert_eq!(window.find(folder_id(root)).selected(), Some(true));
        assert!(fixture.workspace.read(cx).login.is_none());
        assert!(fixture.workspace.read(cx).connect_route.is_none());
    })
    .checked("navigate a real hierarchical workbench");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn keyboard_connection_activation_waits_for_release_and_resolves_reordered_live_session(
    cx: &mut TestAppContext,
) {
    let first = saved_profile("Alpha");
    let second = saved_profile("Beta");
    let (fixture, panes) = navigation_fixture(cx, vec![first.clone(), second.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            for (pane, profile) in panes.iter().zip([&first, &second]) {
                view.remote_connection_ids
                    .insert(pane.terminal.entity_id(), profile.id);
            }
            view.command.update(cx, |input, cx| {
                input.set_value("untouched command", window, cx)
            });
            cx.notify();
        });
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        press(window, cx, "end");
        assert_eq!(window.find(row_id(second.id)).selected(), Some(true));
        key_down(window, cx, "enter", false);
        key_down(window, cx, "enter", true);
        assert_eq!(fixture.workspace.read(cx).active, 0);
        assert!(
            fixture
                .workspace
                .read(cx)
                .navigation_tree
                .focus
                .is_focused(window)
        );
        fixture.workspace.update(cx, |view, _| {
            view.tabs.swap(0, 1);
            view.active = 1;
        });
        key_up(window, cx, "enter");
        let view = fixture.workspace.read(cx);
        assert_eq!(
            view.tabs[view.active].entity_id(),
            panes[1].terminal.entity_id()
        );
        assert!(
            panes[1]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert_eq!(view.command.read(cx).value(), "untouched command");
    })
    .checked("keyboard connects only on the matching release");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn keyboard_release_cannot_activate_a_removed_or_filtered_row(cx: &mut TestAppContext) {
    for filter in [false, true] {
        let first = saved_profile("Alpha");
        let second = saved_profile("Beta");
        let (fixture, panes) = navigation_fixture(cx, vec![first, second.clone()]);
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("navigation-search", cx);
            press(window, cx, "down");
            press(window, cx, "end");
            key_down(window, cx, "enter", false);
            // No render before release: the listener still belongs to the old
            // frame, but it must not activate a replacement row at that index.
            fixture.workspace.update(cx, |view, cx| {
                if filter {
                    view.search
                        .update(cx, |input, cx| input.set_value("Alpha", window, cx));
                } else {
                    view.state
                        .connections
                        .retain(|profile| profile.id != second.id);
                }
            });
            key_up(window, cx, "enter");
            let view = fixture.workspace.read(cx);
            assert!(view.login.is_none() && view.connect_route.is_none());
            assert_eq!(view.active, 0);
        })
        .checked("stale keyboard release is rejected");
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
fn search_temporarily_reveals_collapsed_ancestors_and_restores_browsing_preferences(
    cx: &mut TestAppContext,
) {
    let profile = saved_profile("Database");
    let (fixture, panes) = navigation_fixture(cx, vec![profile.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        let root = fixture.workspace.update(cx, |view, cx| {
            let root = view.state.create_folder("Production", None).checked("root");
            view.state
                .move_connection(profile.id, Some(root))
                .checked("file profile");
            cx.notify();
            root
        });
        window.render_frame(cx);
        window.click(folder_id(root), cx);
        window.render_frame(cx);
        assert_eq!(window.find(folder_id(root)).expanded(), Some(false));
        window.click("navigation-search", cx);
        window.input("Database", cx);
        window.render_frame(cx);
        assert_eq!(window.find(folder_id(root)).expanded(), Some(true));
        assert!(window.find(row_id(profile.id)).visible());
        press(window, cx, "down");
        press(window, cx, "left");
        assert_eq!(window.find(folder_id(root)).expanded(), Some(false));
        window.click("navigation-search", cx);
        press(
            window,
            cx,
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
        );
        press(window, cx, "backspace");
        assert_eq!(window.find(folder_id(root)).expanded(), Some(false));
        assert!(window.try_find(row_id(profile.id)).is_none());
        press(window, cx, "down");
        fixture.workspace.update(cx, |view, cx| {
            view.command.update(cx, |input, cx| {
                input.set_value("unsent theme draft", window, cx);
            });
        });
        for family in crate::design::registry() {
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    fixture.workspace.update(cx, |view, cx| {
                        view.state.settings.theme_family =
                            keelshell_core::ThemeId::new(family.id).checked("theme");
                        view.state.settings.theme = appearance;
                        crate::design::apply_settings(&view.state.settings, Some(window), cx);
                    });
                    i18n::set_language(language, cx);
                    assert_eq!(crate::design::current_definition(cx).id, family.id);
                    assert_eq!(
                        crate::design::palette(cx),
                        family.palette(crate::design::resolve(appearance, window.appearance()))
                    );
                    window.render_frame(cx);
                    assert_eq!(window.find(folder_id(root)).expanded(), Some(false));
                    assert_eq!(window.find(folder_id(root)).selected(), Some(true));
                    assert_eq!(window.find("navigation-rows").focused(), Some(true));
                    let view = fixture.workspace.read(cx);
                    assert_eq!(view.command.read(cx).value(), "unsent theme draft");
                    assert!(view.search.read(cx).value().is_empty());
                }
            }
        }
    })
    .checked("search and appearance retain folder preferences");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn keyboard_end_reveals_last_row_and_tab_leaves_the_tree(cx: &mut TestAppContext) {
    let profiles: Vec<_> = (0..30)
        .map(|i| saved_profile(&format!("Host {i:02}")))
        .collect();
    let last = profiles.last().checked_option("last row").id;
    let (fixture, panes) = navigation_fixture(cx, profiles);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        press(window, cx, "end");
        let viewport = window.find("navigation-rows").bounds();
        let row = window.find(row_id(last));
        assert_eq!(row.selected(), Some(true));
        assert!(row.bounds().top() >= viewport.top() && row.bounds().bottom() <= viewport.bottom());
        press(window, cx, "tab");
        assert_eq!(
            window.find("navigation-manage-connections").focused(),
            Some(true)
        );
        press(window, cx, "shift-tab");
        assert_eq!(window.find("navigation-rows").focused(), Some(true));
        assert_eq!(window.find(row_id(last)).selected(), Some(true));
        press(window, cx, "home");
        assert_eq!(window.find("navigation-unfiled").selected(), Some(true));
        assert!(window.find("navigation-unfiled").visible());
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("keyboard scrolling and single tree tab stop");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn hidden_tree_disarms_release_and_keeps_typing_away_from_ssh(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![saved_profile("Alpha")]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        press(window, cx, "end");
        key_down(window, cx, "enter", false);
        fixture.workspace.update(cx, |view, cx| {
            view.show_assistant = true;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("navigation-rows").is_none());
        assert!(fixture.workspace.read(cx).surface_focus.is_focused(window));
        key_up(window, cx, "enter");
        window.input("not a shell command", cx);
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.navigation_tree.armed.is_none());
            view.show_assistant = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        key_up(window, cx, "enter");
        assert!(fixture.workspace.read(cx).login.is_none());
        assert_eq!(
            fixture.workspace.read(cx).navigation_tree.cursor,
            fixture
                .workspace
                .read(cx)
                .navigation_tree
                .visible
                .last()
                .map(|row| row.id)
        );
    })
    .checked("unmount cancels keyboard activation");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn keyboard_can_focus_empty_tree_without_opening_a_connection(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "tab");
        assert_eq!(window.find("navigation-rows").focused(), Some(true));
        for key in [
            "home", "end", "down", "up", "left", "right", "enter", "space",
        ] {
            press(window, cx, key);
        }
        assert!(window.find("navigation-empty").visible());
        assert_eq!(fixture.workspace.read(cx).navigation_tree.cursor, None);
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("empty keyboard navigation is inert");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn held_search_down_is_not_transferred_to_a_clicked_terminal(cx: &mut TestAppContext) {
    for profiles in [vec![], vec![saved_profile("Alpha")]] {
        let (fixture, panes) = navigation_fixture(cx, profiles);
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("navigation-search", cx);
            key_down(window, cx, "down", false);
            window.render_frame(cx);
            window.click(("session-tab", 1usize), cx);
            window.render_frame(cx);
            key_down(window, cx, "down", true);
            key_up(window, cx, "down");
            assert!(panes.iter().all(|pane| writes(pane).is_empty()));
            press(window, cx, "enter");
            assert_eq!(writes(&panes[1]), b"\r");
        })
        .checked("search entry key retains its owner until release");
    }
}

#[gpui_kit::test]
fn key_guard_is_window_scoped_and_recovers_a_missing_release(cx: &mut TestAppContext) {
    let (owner, owner_panes) = navigation_fixture(cx, vec![saved_profile("Alpha")]);
    let (other, other_panes) = navigation_fixture(cx, vec![]);
    cx.update_window(owner.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "down");
        press(window, cx, "end");
        key_down(window, cx, "enter", false);
        window.click(("session-tab", 1usize), cx);
        window.render_frame(cx);
    })
    .checked("start a held key only in the first window");
    cx.update_window(other.window, |_, window, cx| {
        press(window, cx, "enter");
        assert_eq!(writes(&other_panes[0]), b"\r");
    })
    .checked("another window keeps receiving its own input");
    cx.update_window(owner.window, |_, window, cx| {
        // Simulate an OS focus change that loses the old KeyUp. The first fresh
        // down clears the old owner without dispatching an action on its behalf.
        key_down(window, cx, "enter", false);
        key_up(window, cx, "enter");
        assert!(owner_panes.iter().all(|pane| writes(pane).is_empty()));
        press(window, cx, "enter");
        assert_eq!(writes(&owner_panes[1]), b"\r");
    })
    .checked("missed release cannot swallow every future key");
}
