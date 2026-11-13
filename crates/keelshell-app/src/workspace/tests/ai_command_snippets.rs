//! GPUI regressions for saving reviewed AI commands as reusable snippets.

use super::*;
use crate::snippet_editor::SnippetEditor;
use gpui_kit::{App, Entity, ScrollDelta, Window, point};
use keelshell_core::Snippet;

const ORIGINAL_COMMAND: &str = "printf 'reviewed source 中文\\n'\n\tprintf 'done'";
const EDITED_COMMAND: &str = "printf 'edited after review'";

fn prepare_ai_review(fixture: &Fixture, pane: &RemotePane, cx: &mut TestAppContext) {
    let session_id = format!("{:?}", pane.terminal.entity_id());
    let target = fixture.workspace.read_with(cx, |workspace, _| {
        workspace
            .remote_hosts
            .get(&pane.terminal.entity_id())
            .cloned()
            .checked_option("AI review target")
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = true;
            workspace.assistant.update(cx, |assistant, cx| {
                assistant.set_context("selected output".into(), target, session_id, cx);
                assistant.receive_review_response_for_test(
                    format!("Model explanation\n```sh\n{ORIGINAL_COMMAND}\n```"),
                    cx,
                );
            });
            cx.notify();
        });
        window.render_frame(cx);
    })
    .checked("prepare reviewed AI command in the production assistant");
}

fn reveal_ai_save_button(window: &mut Window, cx: &mut App) {
    for _ in 0..40 {
        window.render_frame(cx);
        let button = window.find(("save-suggestion", 0_usize));
        if button.visible() {
            return;
        }
        window.scroll(
            "assistant-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
    }
    panic!("AI save-as-snippet button did not become visible");
}

fn reveal_diagnostic_save_button(window: &mut Window, cx: &mut App) {
    for _ in 0..40 {
        window.render_frame(cx);
        let button = window.find(("save-diagnostic-step", 0_usize));
        if button.visible() {
            return;
        }
        window.scroll(
            "assistant-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
    }
    panic!("diagnostic save-as-snippet button did not become visible");
}

fn reveal_diagnostic_plan_button(window: &mut Window, cx: &mut App) {
    for _ in 0..40 {
        window.render_frame(cx);
        if let Some(button) = window.try_find("build-diagnostic-plan")
            && button.visible()
        {
            return;
        }
        window.scroll(
            "assistant-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
    }
    panic!("diagnostic plan button did not become visible");
}

fn open_ai_snippet_editor(
    fixture: &Fixture,
    pane: &RemotePane,
    cx: &mut TestAppContext,
) -> Entity<SnippetEditor> {
    prepare_ai_review(fixture, pane, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        reveal_ai_save_button(window, cx);
        let button = window.find(("save-suggestion", 0_usize));
        assert!(button.visible());
        assert_eq!(button.label(), Some("保存为命令片段"));
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("click AI save-as-snippet action");
    cx.run_until_parked();
    let editor = fixture.workspace.read_with(cx, |workspace, _| {
        workspace
            .snippet_editor
            .clone()
            .checked_option("AI save action opens the snippet editor")
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let source = window.find("snippet-ai-review-target");
        assert!(source.visible());
        let label = source.label().unwrap_or_default();
        let session_id = format!("{:?}", pane.terminal.entity_id());
        assert!(
            label.contains("fixture-0@example.invalid:22") && label.contains(&session_id),
            "original AI target/session missing from editor: {label}"
        );
        assert_eq!(
            editor
                .read(cx)
                .draft(cx)
                .checked("original AI command draft")
                .command,
            ORIGINAL_COMMAND
        );
    })
    .checked("verify original AI target and command in the editor");
    editor
}

#[gpui_kit::test]
fn ai_save_button_opens_bound_editor_and_editing_command_requires_review_again(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let editor = open_ai_snippet_editor(&fixture, &panes[0], cx);

    cx.update_window(fixture.window, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_command_text(EDITED_COMMAND, window, cx);
        });
        window.render_frame(cx);
        let state_view = window.find("snippet-ai-review-state");
        let state = state_view.label().unwrap_or_default();
        assert!(
            state.contains("重新审核"),
            "edited AI command must show the re-review warning: {state}"
        );
        assert!(state.contains("原审核"));
        assert!(
            fixture.workspace.read(cx).snippet_editor.is_some(),
            "editing the AI draft must retain the editor"
        );
    })
    .checked("show re-review warning after editing the AI command");
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn stale_ai_session_blocks_save_retains_draft_and_does_not_change_state_store(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let before = fixture
        .store
        .load()
        .checked("load state before stale AI save")
        .snippets;
    let editor = open_ai_snippet_editor(&fixture, &panes[0], cx);
    let mut expected = editor.read_with(cx, |editor, cx| {
        editor.draft(cx).checked("AI draft before stale change")
    });

    cx.update_window(fixture.window, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_command_text(EDITED_COMMAND, window, cx);
            expected.command = EDITED_COMMAND.into();
        });
        fixture.workspace.update(cx, |workspace, cx| {
            // The modal remains open while the active terminal changes. The
            // source carried by the editor must still bind the save to pane 0.
            workspace.active = 1;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("submit AI snippet after changing the active session");
    cx.run_until_parked();

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(fixture.workspace.read(cx).snippet_editor.is_some());
        assert_eq!(
            editor.read(cx).draft(cx).checked("retained stale AI draft"),
            expected
        );
        let error_view = window.find("snippet-editor-error");
        let error = error_view.label().unwrap_or_default();
        assert!(
            error.contains("目标已变化")
                || error.contains("target changed")
                || error.contains("会话已结束")
                || error.contains("session ended"),
            "stale target feedback is unclear: {error}"
        );
    })
    .checked("retain editor and report stale AI session");
    assert_eq!(
        fixture
            .store
            .load()
            .checked("load state after stale AI save")
            .snippets,
        before
    );
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn state_reload_blocks_ai_snippet_save_but_restores_retry_and_cancel(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let before = fixture
        .store
        .load()
        .checked("load state before blocked AI save")
        .snippets;
    let editor = open_ai_snippet_editor(&fixture, &panes[0], cx);

    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.state_reloading = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("submit AI snippet while configuration reload is active");
    cx.run_until_parked();

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(fixture.workspace.read(cx).snippet_editor.is_some());
        assert!(!editor.read(cx).is_saving());
        let error = window.find("snippet-editor-error");
        assert!(error.visible());
        let error_text = error.label().unwrap_or_default();
        assert!(
            error_text.contains("重新加载") || error_text.contains("reload"),
            "reload gate error is unclear: {error_text}"
        );
        window.click("snippet-editor-save", cx);
    })
    .checked("retry blocked AI snippet save after the first failure");
    cx.run_until_parked();

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!editor.read(cx).is_saving());
        assert!(window.find("snippet-editor-error").visible());
        window.click("snippet-editor-cancel", cx);
    })
    .checked("cancel the retained AI snippet draft");
    cx.run_until_parked();

    assert!(
        fixture
            .workspace
            .read_with(cx, |workspace, _| workspace.snippet_editor.is_none()),
        "cancel must close the recovered editor"
    );
    assert_eq!(
        fixture
            .store
            .load()
            .checked("load state after blocked AI saves")
            .snippets,
        before
    );
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn duplicate_ai_snippet_id_reports_failure_without_persisting(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let existing = Snippet::new("existing", "printf existing");
    let mut state = fixture.store.load().checked("load duplicate fixture state");
    state.snippets.push(existing.clone());
    let saved = save_and_adopt_fixture_state(&fixture, state, "seed duplicate snippet", cx);
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.snippet_sources = Arc::new(saved.snippets.clone());
            workspace.command_sources_revision = workspace.command_sources_revision.wrapping_add(1);
            cx.notify();
        });
    })
    .checked("adopt duplicate snippet source");
    let panes = attach_remote_panes(&fixture, cx);
    let editor = open_ai_snippet_editor(&fixture, &panes[0], cx);
    cx.update_window(fixture.window, |_, window, cx| {
        editor.update(cx, |editor, _| editor.set_id_for_test(existing.id));
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("submit duplicate AI snippet ID");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let error_view = window.find("snippet-editor-error");
        let error = error_view.label().unwrap_or_default();
        assert!(
            error.contains("片段未保存") && error.contains("unique"),
            "duplicate ID feedback is unclear: {error}"
        );
    })
    .checked("report duplicate AI snippet ID");
    assert_eq!(
        fixture
            .store
            .load()
            .checked("load state after duplicate AI save")
            .snippets,
        saved.snippets
    );
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn diagnostic_save_button_opens_bound_snippet_editor(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    prepare_ai_review(&fixture, &panes[0], cx);

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        reveal_diagnostic_plan_button(window, cx);
        window.click("build-diagnostic-plan", cx);
        window.render_frame(cx);
        reveal_diagnostic_save_button(window, cx);
        let button = window.find(("save-diagnostic-step", 0_usize));
        assert!(button.visible());
        assert_eq!(button.label(), Some("保存为命令片段"));
        window.click(("save-diagnostic-step", 0_usize), cx);
    })
    .checked("click diagnostic save-as-snippet action");
    cx.run_until_parked();

    let editor = fixture.workspace.read_with(cx, |workspace, _| {
        workspace
            .snippet_editor
            .clone()
            .checked_option("diagnostic save action opens the snippet editor")
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            editor
                .read(cx)
                .draft(cx)
                .checked("diagnostic snippet draft")
                .command,
            ORIGINAL_COMMAND
        );
        let source = window.find("snippet-ai-review-target");
        assert!(source.visible());
        let label = source.label().unwrap_or_default();
        assert!(
            label.contains("fixture-0@example.invalid:22"),
            "diagnostic target missing from snippet editor: {label}"
        );
    })
    .checked("verify diagnostic snippet editor binding");
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}
