//! A key started in navigation must not become input for a newly focused SSH.
use super::{keyboard::*, *};

fn bind_profiles(fixture: &Fixture, panes: &[RemotePane], profiles: &[Connection], cx: &mut App) {
    fixture.workspace.update(cx, |view, cx| {
        for (pane, profile) in panes.iter().zip(profiles) {
            view.remote_connection_ids
                .insert(pane.terminal.entity_id(), profile.id);
        }
        cx.notify();
    });
}

#[gpui_kit::test]
fn held_navigation_key_cannot_write_after_pointer_switch_and_fresh_input_recovers(
    cx: &mut TestAppContext,
) {
    for key in ["enter", "space", "down", "left"] {
        let profiles = vec![saved_profile("Alpha"), saved_profile("Beta")];
        let (fixture, panes) = navigation_fixture(cx, profiles.clone());
        cx.update_window(fixture.window, |_, window, cx| {
            bind_profiles(&fixture, &panes, &profiles, cx);
            window.render_frame(cx);
            window.click("navigation-search", cx);
            press(window, cx, "down");
            press(window, cx, "down");
            key_down(window, cx, key, false);
            window.render_frame(cx);
            window.click(row_id(profiles[1].id), cx);
            window.render_frame(cx);
            assert_eq!(fixture.workspace.read(cx).active, 1);
            key_down(window, cx, key, true);
            key_up(window, cx, key);
            assert!(panes.iter().all(|pane| writes(pane).is_empty()), "{key}");
            press(window, cx, "enter");
            assert_eq!(writes(&panes[1]), b"\r", "new press must reach SSH");
            assert!(writes(&panes[0]).is_empty());
        })
        .checked("pointer focus transfer retains the old key owner");
    }
}

#[gpui_kit::test]
fn overlapping_activation_keys_cancel_the_chord_until_both_are_released(cx: &mut TestAppContext) {
    for (first, second) in [("enter", "space"), ("space", "enter")] {
        for release_first in [false, true] {
            let profiles = vec![saved_profile("Alpha"), saved_profile("Beta")];
            let (fixture, panes) = navigation_fixture(cx, profiles.clone());
            cx.update_window(fixture.window, |_, window, cx| {
                bind_profiles(&fixture, &panes, &profiles, cx);
                window.render_frame(cx);
                window.click("navigation-search", cx);
                press(window, cx, "down");
                press(window, cx, "end");
                key_down(window, cx, first, false);
                key_down(window, cx, second, false);
                let (released, held) = if release_first {
                    (first, second)
                } else {
                    (second, first)
                };
                key_up(window, cx, released);
                window.render_frame(cx);
                assert_eq!(fixture.workspace.read(cx).active, 0);
                assert_eq!(window.find("navigation-rows").focused(), Some(true));
                key_down(window, cx, held, true);
                key_up(window, cx, held);
                assert_eq!(fixture.workspace.read(cx).active, 0);
                assert!(panes.iter().all(|pane| writes(pane).is_empty()));
                press(window, cx, "enter");
                assert_eq!(fixture.workspace.read(cx).active, 1);
                assert!(panes.iter().all(|pane| writes(pane).is_empty()));
            })
            .checked("overlapping activation cannot hand held input to SSH");
        }
    }
}

#[gpui_kit::test]
fn navigation_key_owner_survives_filter_hide_and_modal_invalidation(cx: &mut TestAppContext) {
    for transition in ["filter", "hidden", "modal"] {
        let profiles = vec![saved_profile("Alpha"), saved_profile("Beta")];
        let (fixture, panes) = navigation_fixture(cx, profiles.clone());
        cx.update_window(fixture.window, |_, window, cx| {
            bind_profiles(&fixture, &panes, &profiles, cx);
            window.render_frame(cx);
            window.click("navigation-search", cx);
            press(window, cx, "down");
            press(window, cx, "end");
            key_down(window, cx, "enter", false);
            fixture.workspace.update(cx, |view, cx| {
                match transition {
                    "filter" => view.search.update(cx, |input, cx| {
                        input.set_value("Alpha", window, cx);
                    }),
                    "hidden" => view.show_navigation = false,
                    "modal" => view.open_appearance(window, cx),
                    _ => unreachable!(),
                }
                cx.notify();
            });
            window.render_frame(cx);
            window.render_frame(cx);
            key_down(window, cx, "enter", true);
            fixture.workspace.update(cx, |view, cx| {
                if transition == "modal" {
                    assert!(view.show_appearance, "old key must not close a dialog");
                    view.close_active_modal(window, cx);
                }
                cx.notify();
            });
            window.render_frame(cx);
            window.render_frame(cx);
            window.click(("session-tab", 1usize), cx);
            window.render_frame(cx);
            key_down(window, cx, "enter", true);
            key_up(window, cx, "enter");
            assert!(
                panes.iter().all(|pane| writes(pane).is_empty()),
                "{transition}"
            );
            press(window, cx, "enter");
            assert_eq!(writes(&panes[1]), b"\r");
        })
        .checked("invalidating a proposal does not release its held key");
    }
}
