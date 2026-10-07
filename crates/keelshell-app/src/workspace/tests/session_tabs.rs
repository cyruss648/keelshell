//! Actual GPUI bounds and pointer routing; these are not native desktop screenshots.
use super::*;
use crate::workspace::CloseTab;
use gpui_kit::{MouseButton, Window};

fn render_and_reveal(window: &mut Window, cx: &mut gpui_kit::App) {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
}

fn assert_active_close_visible(fixture: &Fixture, window: &Window, cx: &gpui_kit::App) {
    let active = fixture.workspace.read(cx).active;
    let viewport = window.find("session-tabs").bounds();
    let close = window.find(("close-tab", active));
    let bounds = close.bounds();
    assert!(close.visible());
    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
    assert!(
        bounds.left() >= viewport.left(),
        "close is left-clipped: {bounds:?} / {viewport:?}"
    );
    assert!(
        bounds.right() <= viewport.right(),
        "close is right-clipped: {bounds:?} / {viewport:?}"
    );
    assert!(bounds.top() >= viewport.top() && bounds.bottom() <= viewport.bottom());
    let title = window.find(("session-tab-title", active)).bounds();
    assert!(title.size.width > px(0.));
    assert!(
        title.right() <= bounds.left(),
        "title overlaps close: {title:?} / {bounds:?}"
    );
}

#[gpui_kit::test]
fn long_session_titles_keep_close_visible_in_both_languages_and_window_sizes(
    cx: &mut TestAppContext,
) {
    for width in [900., 1100., 1279., 1280., 1440.] {
        let fixture = mount_sized(cx, Vec::new(), width, 580.);
        let panes = attach_remote_panes(&fixture, cx);
        cx.update_window(fixture.window, |_, window, cx| {
            for (index, pane) in panes.iter().enumerate() {
                pane.terminal.update(cx, |terminal, cx| {
                    terminal.title = format!(
                        "超长远程会话-{index}-production-readonly@host.example.invalid:22222"
                    );
                    cx.notify();
                });
            }
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                for assistant in [false, true] {
                    for active in 0..panes.len() {
                        fixture.workspace.update(cx, |workspace, cx| {
                            workspace.show_assistant = assistant;
                            workspace.active = active;
                            cx.notify();
                        });
                        render_and_reveal(window, cx);
                        assert_active_close_visible(&fixture, window, cx);
                    }
                }
            }
        })
        .checked("measure real bilingual long-title close controls");
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
fn navigation_reveals_and_closes_the_selected_session_without_sending_drafts(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes: Vec<_> = (0..3)
        .flat_map(|_| attach_remote_panes(&fixture, cx))
        .collect();
    let identities: Vec<_> = panes.iter().map(|pane| pane.terminal.entity_id()).collect();
    cx.update_window(fixture.window, |_, window, cx| {
        i18n::set_language(Language::En, cx);
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.tabs = panes.iter().map(|pane| pane.terminal.clone()).collect();
            workspace.active = 0;
            workspace.command.update(cx, |input, cx| {
                input.set_value("printf '尚未发送'", window, cx);
            });
            cx.notify();
        });
        render_and_reveal(window, cx);
        assert_active_close_visible(&fixture, window, cx);
        window.click("previous-session", cx);
        assert_eq!(fixture.workspace.read(cx).active, 0);
        for expected in 1..panes.len() {
            window.click("next-session", cx);
            render_and_reveal(window, cx);
            assert_eq!(fixture.workspace.read(cx).active, expected);
            assert_active_close_visible(&fixture, window, cx);
        }
        window.click("next-session", cx);
        assert_eq!(fixture.workspace.read(cx).active, panes.len() - 1);
        window.click(("close-tab", panes.len() - 1), cx);
        render_and_reveal(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.tabs.len(), panes.len() - 1);
        assert_eq!(workspace.active, panes.len() - 2);
        assert_eq!(
            workspace
                .tabs
                .iter()
                .map(Entity::entity_id)
                .collect::<Vec<_>>(),
            identities[..identities.len() - 1]
        );
        assert_eq!(workspace.command.read(cx).value(), "printf '尚未发送'");
        assert_active_close_visible(&fixture, window, cx);
        window.click("previous-session", cx);
        render_and_reveal(window, cx);
        assert_eq!(fixture.workspace.read(cx).active, panes.len() - 3);
        assert_active_close_visible(&fixture, window, cx);
    })
    .checked("navigate and close real session controls at minimum width");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn painted_close_resolves_its_session_after_another_tab_is_removed(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 580.);
    let panes: Vec<_> = (0..2)
        .flat_map(|_| attach_remote_panes(&fixture, cx))
        .collect();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.tabs = panes[..3]
                .iter()
                .map(|pane| pane.terminal.clone())
                .collect();
            workspace.active = 2;
            cx.notify();
        });
        render_and_reveal(window, cx);
        let position = window.find(("close-tab", 2_usize)).bounds().center();
        // Keep the painted listeners: the table can change in a foreground
        // turn before the platform delivers the pointer activation.
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.active = 1;
            workspace.close_tab(&CloseTab, window, cx);
        });
        for input in [
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
            window.dispatch_event(input, cx);
        }
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.tabs.len(), 1);
        assert_eq!(workspace.tabs[0].entity_id(), panes[0].terminal.entity_id());
        // A second activation of the same painted control has no live target.
        for input in [
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
            window.dispatch_event(input, cx);
        }
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.tabs.len(), 1);
        assert_eq!(workspace.tabs[0].entity_id(), panes[0].terminal.entity_id());
    })
    .checked("activate the previously painted close without repainting stale indices");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn same_active_tab_survives_language_resize_and_manual_scroll(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 580.);
    let panes: Vec<_> = (0..3)
        .flat_map(|_| attach_remote_panes(&fixture, cx))
        .collect();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.tabs = panes.iter().map(|pane| pane.terminal.clone()).collect();
            workspace.active = 2;
            cx.notify();
        });
        for width in [1440., 900., 1440., 900.] {
            window.resize(size(px(width), px(580.)));
            window.bounds_changed(cx);
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                render_and_reveal(window, cx);
                assert_eq!(fixture.workspace.read(cx).active, 2);
                assert_active_close_visible(&fixture, window, cx);
            }
        }
        let initial = fixture.workspace.read(cx).session_tab_scroll.offset();
        let move_right = window.find("session-tabs").bounds().right()
            - window.find(("close-tab", 2_usize)).bounds().right()
            + px(20.);
        fixture
            .workspace
            .read(cx)
            .session_tab_scroll
            .set_offset(point(initial.x + move_right, initial.y));
        // Ordinary repaint must preserve manual exploration, even when it
        // partially clips the currently active tab.
        window.render_frame(cx);
        let manual = fixture.workspace.read(cx).session_tab_scroll.offset();
        assert_eq!(manual.x, initial.x + move_right);
        assert!(
            window.find(("close-tab", 2_usize)).bounds().right()
                > window.find("session-tabs").bounds().right()
        );
        render_and_reveal(window, cx);
        assert_eq!(
            fixture.workspace.read(cx).session_tab_scroll.offset(),
            manual
        );
        window.click(("session-tab", 2_usize), cx);
        render_and_reveal(window, cx);
        assert_active_close_visible(&fixture, window, cx);
        assert_eq!(fixture.workspace.read(cx).active, 2);
    })
    .checked("keep the same active tab visible after language, layout and explicit reselection");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
