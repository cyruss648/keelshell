#![cfg(test)]
//! Real window dispatch with isolated transports; not native keyboard acceptance.
use super::*;
use gpui_kit::{
    Context, ElementInputHandler, InputHandler, IntoElement, ParentElement, Render, Styled,
    component::input::{
        Editor, EditorState, IndentInline, Input, OutdentInline, Textarea, TextareaState,
    },
    div,
};

fn focus_command(fixture: &Fixture, text: &str, window: &mut Window, cx: &mut App) {
    let command = fixture.workspace.read(cx).command.clone();
    window.render_frame(cx);
    window.click(("input", command.entity_id()), cx);
    command.update(cx, |input, cx| {
        input.set_value(text, window, cx);
        input.set_selected_range(0..text.len(), cx);
    });
    window.render_frame(cx);
    assert!(command.read(cx).focus_handle(cx).is_focused(window));
}

fn assert_command_tab(fixture: &Fixture, key: &str, window: &mut Window, cx: &mut App) {
    let command = fixture.workspace.read(cx).command.clone();
    let before = command.read(cx).value();
    let focus = command.read(cx).focus_handle(cx);
    // Derive the ordinary rendered tab destination, without invoking any
    // composer handler. The actual assertion below dispatches a keyboard event.
    match key {
        "tab" => window.focus_next(cx),
        "shift-tab" => window.focus_prev(cx),
        _ => unreachable!("only the two tab directions are tested"),
    }
    let expected = window.focused(cx);
    assert_ne!(
        expected,
        Some(focus.clone()),
        "fixture needs another tab stop"
    );
    focus.focus(window, cx);
    window.render_frame(cx);

    window.press(key, cx);
    window.render_frame(cx);
    assert_eq!(
        window.focused(cx),
        expected,
        "{key} must use ordinary tab order"
    );
    assert_eq!(command.read(cx).value().as_bytes(), before.as_bytes());
}

#[gpui_kit::test]
fn command_tab_leaves_empty_composer_without_editing_or_sending(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        focus_command(&fixture, "", window, cx);
        assert_command_tab(&fixture, "tab", window, cx);
    })
    .checked("dispatch forward tab at the empty composer");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_shift_tab_leaves_empty_composer_without_editing_or_sending(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        focus_command(&fixture, "", window, cx);
        assert_command_tab(&fixture, "shift-tab", window, cx);
    })
    .checked("dispatch reverse tab at the empty composer");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_tabs_preserve_indented_multiline_bytes_across_selections(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    let draft = "\tprintf '中文'\n    echo second\n";
    cx.update_window(fixture.window, |_, window, cx| {
        for key in ["tab", "shift-tab"] {
            for selection in [0..0, draft.len()..draft.len(), 0..draft.len()] {
                focus_command(&fixture, draft, window, cx);
                let command = fixture.workspace.read(cx).command.clone();
                command.update(cx, |input, cx| input.set_selected_range(selection, cx));
                assert_command_tab(&fixture, key, window, cx);
            }
        }
    })
    .checked("both tab directions preserve selected and collapsed multiline drafts");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_tabs_during_marked_text_keep_the_composer_owner(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        for key in ["tab", "shift-tab"] {
            focus_command(&fixture, "", window, cx);
            let command = fixture.workspace.read(cx).command.clone();
            let mut handler = ElementInputHandler::new(
                window.find(("input", command.entity_id())).bounds(),
                command.clone(),
            );
            handler.replace_and_mark_text_in_range(Some(0..0), "    候选", Some(4..6), window, cx);
            window.render_frame(cx);
            assert!(handler.marked_text_range(window, cx).is_some());
            window.press(key, cx);
            window.render_frame(cx);
            // Composition retains the existing input action path. This test
            // does not impose a new editing policy on Kit's marked text.
            assert!(command.read(cx).focus_handle(cx).is_focused(window));
        }
    })
    .checked("marked input does not enter composer focus traversal");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_capture_does_not_route_actions_from_the_run_button(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        focus_command(&fixture, "echo unsent", window, cx);
        window.press("tab", cx);
        window.render_frame(cx);
        assert_eq!(window.find("run-command").focused(), Some(true));
        let focus = window.focused(cx).checked_option("run button focus");
        focus.dispatch_action(&IndentInline, window, cx);
        assert_eq!(window.focused(cx), Some(focus.clone()));
        focus.dispatch_action(&OutdentInline, window, cx);
        assert_eq!(window.focused(cx), Some(focus));
        assert_eq!(
            fixture.workspace.read(cx).command.read(cx).value(),
            "echo unsent"
        );
    })
    .checked("composer capture requires actual textarea focus even within the same bar");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_reverse_tab_action_respects_a_modal_before_repaint(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        focus_command(&fixture, "echo unsent", window, cx);
        fixture.workspace.update(cx, |view, cx| {
            view.show_appearance = true;
            cx.notify();
        });
        // Dispatch against the previously painted composer, before the modal
        // unmounts it. An unindented outdent has no edit in the original input.
        let focus = fixture.workspace.read(cx).command.read(cx).focus_handle(cx);
        focus.dispatch_action(&OutdentInline, window, cx);
        let command = &fixture.workspace.read(cx).command;
        assert!(command.read(cx).focus_handle(cx).is_focused(window));
        assert_eq!(command.read(cx).value(), "echo unsent");
    })
    .checked("live modal state prevents a stale composer capture from moving focus");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
fn command_tabs_leave_mounted_modal_focus_and_draft_intact(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        focus_command(&fixture, "\tunsent\n    draft", window, cx);
        window.click("theme-settings", cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(fixture.workspace.read(cx).active_modal().is_some());
        assert!(window.try_find("command-input-bar").is_none());
        for key in ["tab", "shift-tab"] {
            for _ in 0..4 {
                window.press(key, cx);
                window.render_frame(cx);
                assert!(
                    fixture
                        .workspace
                        .read(cx)
                        .modal_focus
                        .contains_focused(window, cx),
                    "{key} escaped the active modal"
                );
                assert_eq!(
                    fixture.workspace.read(cx).command.read(cx).value(),
                    "\tunsent\n    draft"
                );
            }
        }
    })
    .checked("mounted modal retains both tab directions");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

struct OtherInputs {
    workspace: Entity<Workspace>,
    input: Entity<InputState>,
    textarea: Entity<TextareaState>,
    editor: Entity<EditorState>,
}

impl Render for OtherInputs {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().child(self.workspace.clone()))
            .child(div().h(px(32.)).child(Input::new(&self.input)))
            .child(div().h(px(48.)).child(Textarea::new(&self.textarea)))
            .child(div().h(px(64.)).child(Editor::new(&self.editor)))
    }
}

#[gpui_kit::test]
fn command_capture_leaves_other_input_editor_and_terminal_scopes_unchanged(
    cx: &mut TestAppContext,
) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        let controls = cx.new(|cx| OtherInputs {
            workspace: fixture.workspace.clone(),
            input: cx.new(|cx| InputState::new(window, cx)),
            textarea: cx.new(|cx| TextareaState::new(window, cx).rows(2)),
            // Match the remote draft editor's editing configuration, without
            // opening an SSH session or granting any remote write capability.
            editor: cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("text")
                    .searchable(false)
                    .folding(false)
                    .auto_close(false)
                    .smart_indent(false)
            }),
        });
        window.replace_root(cx, |window, cx| {
            gpui_kit::base::Root::new(controls.clone(), window, cx)
        });
        window.render_frame(cx);
        let (input, textarea, editor) = {
            let controls = controls.read(cx);
            (
                controls.input.clone(),
                controls.textarea.clone(),
                controls.editor.clone(),
            )
        };
        input.update(cx, |input, cx| input.set_value("other input", window, cx));
        window.render_frame(cx);
        window.click(("input", input.entity_id()), cx);
        window.press("tab", cx);
        window.render_frame(cx);
        assert!(textarea.read(cx).focus_handle(cx).is_focused(window));
        input.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
        window.focus_prev(cx);
        let previous = window.focused(cx);
        assert_ne!(previous, Some(input.read(cx).focus_handle(cx)));
        input.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
        window.press("shift-tab", cx);
        window.render_frame(cx);
        assert_eq!(window.focused(cx), previous);
        assert_eq!(input.read(cx).value(), "other input");

        for key in ["tab", "shift-tab"] {
            let expected = if key == "tab" {
                "    first\n    second"
            } else {
                "first\n    second"
            };
            textarea.update(cx, |input, cx| {
                input.set_value("  first\n    second", window, cx);
                input.set_selected_range(0..0, cx);
            });
            textarea.read(cx).focus_handle(cx).focus(window, cx);
            window.render_frame(cx);
            window.press(key, cx);
            window.render_frame(cx);
            assert!(textarea.read(cx).focus_handle(cx).is_focused(window));
            assert_eq!(textarea.read(cx).value(), expected);

            editor.update(cx, |input, cx| {
                input.set_value("  first\n    second", window, cx);
                input.set_selected_range(0..0, cx);
            });
            editor.read(cx).focus_handle(cx).focus(window, cx);
            window.render_frame(cx);
            window.press(key, cx);
            window.render_frame(cx);
            assert!(editor.read(cx).focus_handle(cx).is_focused(window));
            assert_eq!(editor.read(cx).value(), expected);
        }
        // Dispatch the exact captured actions while the terminal owns focus.
        // Raw terminal Tab encoding is outside this navigation regression.
        let terminal_focus = panes[0].terminal.read(cx).focus_handle(cx);
        terminal_focus.focus(window, cx);
        window.render_frame(cx);
        terminal_focus.dispatch_action(&IndentInline, window, cx);
        terminal_focus.dispatch_action(&OutdentInline, window, cx);
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(
            fixture
                .workspace
                .read(cx)
                .command
                .read(cx)
                .value()
                .is_empty()
        );
    })
    .checked("unrelated inputs keep Kit indentation and terminal action ownership");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
