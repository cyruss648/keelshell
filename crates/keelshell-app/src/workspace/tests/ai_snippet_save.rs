//! End-to-end GPUI coverage for the reviewed AI command to local snippet flow.
//!
//! These tests use deterministic in-process SSH panes and a temporary
//! StateStore. They prove event admission and persistence behavior without
//! executing a command or contacting an AI provider.

use super::*;
use crate::{command_suggestions, i18n::set_language};
use gpui_kit::{Entity, ScrollDelta, TestAppContext, test::TestWindowExt};
use keelshell_core::{Language, Snippet, Theme, ThemeId};

const COMMAND: &str = "printf 'reviewed snippet 中文\\n'";

fn reveal_assistant_review(
    fixture: &Fixture,
    panel: &Entity<crate::assistant::AssistantPanel>,
    response: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("context-screen", cx);
    })
    .checked("capture terminal context for AI snippet flow");
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        panel.receive_review_response_for_test(response.to_owned(), cx)
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "assistant-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.render_frame(cx);
    })
    .checked("reveal reviewed AI save action");
}

fn assistant_panel(
    fixture: &Fixture,
    cx: &mut TestAppContext,
) -> Entity<crate::assistant::AssistantPanel> {
    fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.assistant.clone())
}

#[gpui_kit::test]
async fn reviewed_ai_suggestion_opens_locked_draft_and_persists_plain_local_snippet(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Explanation\n```sh\n{COMMAND}\n```"),
        cx,
    );

    cx.update_window(fixture.window, |_, window, cx| {
        assert!(window.find(("save-suggestion", 0_usize)).visible());
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("review AI suggestion before opening snippet draft");
    cx.run_until_parked();

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("snippet-editor").visible());
        assert!(window.find("snippet-command-locked").visible());
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert!(
                !workspace
                    .snippet_editor
                    .as_ref()
                    .unwrap_or_else(|| panic!("AI snippet editor"))
                    .read(cx)
                    .command_is_editable(cx)
            );
        });
        assert_eq!(
            window.find("snippet-editor-save").label(),
            Some("确认保存片段")
        );
        window.click("snippet-name", cx);
        window.input(" · 本地", cx);
        window.click("snippet-description", cx);
        window.input("reviewed by user", cx);
        window.click("snippet-tags", cx);
        window.input("ai, reviewed", cx);
        window.click("snippet-editor-save", cx);
        assert!(fixture.workspace.read(cx).snippet_editor.is_some());
        assert!(!fixture.workspace.read(cx).saving);
        window.click("snippet-editor-save", cx);
    })
    .checked("fill reviewed snippet metadata and confirm save");
    cx.run_until_parked();

    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;

    let saved = fixture.store.load().checked("reload saved AI snippet");
    let snippet = saved
        .snippets
        .iter()
        .find(|snippet| snippet.command == COMMAND)
        .cloned()
        .unwrap_or_else(|| panic!("reviewed command was persisted as a snippet"));
    assert!(snippet.name.contains("本地"));
    assert_eq!(snippet.description, "reviewed by user");
    assert_eq!(snippet.tags, ["ai", "reviewed"]);
    assert!(command_suggestions::snippet_matches(&snippet, "reviewed"));
    assert_eq!(
        command_suggestions::suggest(&Default::default(), &[snippet], "本地")
            .first()
            .map(|suggestion| suggestion.command.as_str()),
        Some(COMMAND)
    );
    assert!(
        writes(&panes[0]).is_empty(),
        "saving a snippet must not run it"
    );
    assert!(writes(&panes[1]).is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("snippet-editor").is_none());
    })
    .checked("close the editor only after successful persistence");
}

#[gpui_kit::test]
fn reviewed_ai_diagnostic_step_uses_the_same_locked_draft_flow(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let _panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Diagnostic\n```bash\n{COMMAND}\n```"),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("build-diagnostic-plan", cx);
        window.render_frame(cx);
        window.scroll(
            "assistant-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find(("save-diagnostic-step", 0_usize)).visible());
        window.click(("save-diagnostic-step", 0_usize), cx);
    })
    .checked("review diagnostic step before saving a snippet");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("snippet-editor").visible());
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert!(
                !workspace
                    .snippet_editor
                    .as_ref()
                    .unwrap_or_else(|| panic!("diagnostic snippet editor"))
                    .read(cx)
                    .command_is_editable(cx)
            );
        });
        window.click("snippet-editor-cancel", cx);
    })
    .checked("cancel diagnostic snippet draft without execution");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.snippet_editor.is_none());
        assert!(
            workspace
                .state
                .snippets
                .iter()
                .all(|snippet| snippet.command != COMMAND)
        );
    });
}

#[gpui_kit::test]
fn target_switch_after_opening_ai_draft_rejects_save_and_drops_candidate(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Explanation\n```sh\n{COMMAND}\n```"),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("open AI snippet draft before target switch");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.active = 1;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("reject save after switching active SSH target");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(
            workspace.snippet_editor.is_none(),
            "editor remains; status={}",
            workspace.status.render(cx)
        );
        assert!(workspace.ai_snippet_review.is_none());
        assert!(workspace.status.render(cx).contains("AI 片段审核已失效"));
    });
    assert!(
        fixture
            .store
            .load()
            .checked("reload rejected state")
            .snippets
            .iter()
            .all(|snippet| snippet.command != COMMAND)
    );
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn closing_ai_target_after_draft_open_keeps_no_save_authority(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Explanation\n```sh\n{COMMAND}\n```"),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("open draft before closing its SSH target");
    cx.run_until_parked();
    let first = panes[0].terminal.entity_id();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            // The production close action is blocked while a modal is open.
            // Model the same retirement boundary directly so the retained
            // draft can prove that its old target no longer grants save
            // authority.
            workspace
                .session_panes
                .remove(first)
                .checked("retire closed target from pane layout");
            workspace.tabs.retain(|tab| tab.entity_id() != first);
            workspace.remote_hosts.remove(&first);
            workspace.remote_sessions.remove(&first);
            workspace.active = 0;
            workspace.assistant.update(cx, |panel, cx| {
                panel.invalidate_session(&format!("{first:?}"), cx)
            });
            cx.notify();
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("reject save after closing original SSH target");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(
            workspace.snippet_editor.is_none(),
            "editor remains; status={}",
            workspace.status.render(cx)
        );
        assert!(
            workspace.status.render(cx).contains("AI 片段审核已失效"),
            "status={}",
            workspace.status.render(cx)
        );
    });
    assert!(
        fixture
            .store
            .load()
            .checked("reload closed-target state")
            .snippets
            .iter()
            .all(|snippet| snippet.command != COMMAND)
    );
    assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn state_store_conflict_keeps_complete_ai_draft_and_error(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let _panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Explanation\n```sh\n{COMMAND}\n```"),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("open draft before conflict");
    cx.run_until_parked();
    let mut external = fixture.store.load().checked("load external conflict state");
    external.snippets.push(Snippet::new("external", "true"));
    fixture
        .store
        .save(&external)
        .checked("write competing StateStore snapshot");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("submit AI snippet against stale revision");
    cx.run_until_parked();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        let panel = workspace
            .snippet_editor
            .as_ref()
            .unwrap_or_else(|| panic!("conflict retains snippet editor"));
        assert!(!panel.read(cx).is_saving());
        assert_eq!(panel.read(cx).command_value_for_test(cx), COMMAND);
        assert!(panel.read(cx).has_error_for_test());
    });
    let disk = fixture.store.load().checked("reload conflict state");
    assert!(
        disk.snippets
            .iter()
            .all(|snippet| snippet.command != COMMAND)
    );
}

#[gpui_kit::test]
fn reviewed_snippet_surface_covers_language_and_theme_tokens(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let _panes = attach_remote_panes(&fixture, cx);
    let panel = assistant_panel(&fixture, cx);
    reveal_assistant_review(
        &fixture,
        &panel,
        &format!("Explanation\n```sh\n{COMMAND}\n```"),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("save-suggestion", 0_usize), cx);
    })
    .checked("open theme matrix draft");
    cx.run_until_parked();
    for language in [Language::ZhCn, Language::En] {
        for family in crate::design::registry() {
            for theme in [Theme::Light, Theme::Dark] {
                cx.update_window(fixture.window, |_, window, cx| {
                    set_language(language, cx);
                    fixture.workspace.update(cx, |workspace, cx| {
                        workspace.state.settings.theme_family =
                            ThemeId::new(family.id).checked("registered AI snippet theme");
                        workspace.state.settings.theme = theme;
                        cx.notify();
                    });
                    let settings = fixture.workspace.read(cx).state.settings.clone();
                    crate::design::apply_settings(&settings, Some(window), cx);
                    window.render_frame(cx);
                    assert!(window.find("snippet-editor-footer").visible());
                    assert!(window.find("snippet-plaintext-notice").visible());
                    assert!(window.find("snippet-command-locked").visible());
                    assert!(window.find("snippet-editor-cancel").visible());
                    assert!(window.find("snippet-editor-save").visible());
                })
                .checked("bilingual themed AI snippet surface");
            }
        }
    }
}
