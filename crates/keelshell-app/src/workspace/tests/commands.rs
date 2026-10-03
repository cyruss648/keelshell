//! Production command-library, draft persistence and target-binding regressions.

use super::*;
use gpui_kit::{
    App, ElementInputHandler, InputHandler, SharedString, Window,
    component::{WindowExt, input::AnyInputState},
};
use keelshell_core::Snippet;

const BODY: &str = "  printf 'needle 中文\\n'\n\tprintf 'reviewed'\n";

fn snippet() -> Snippet {
    let mut snippet = Snippet::new("needle 检查", BODY);
    snippet.description = "审核完整路径后使用".into();
    snippet.tags = vec!["诊断".into(), "disk,space".into(), "quote\"tag".into()];
    snippet
}

fn seed(fixture: &Fixture, snippets: Vec<Snippet>, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let mut state = workspace.state.clone();
            state.snippets = snippets;
            workspace.state = fixture.store.save(&state).checked("seed snippet fixture");
            workspace.snippet_sources = Arc::new(workspace.state.snippets.clone());
            workspace.command_sources_revision = workspace.command_sources_revision.wrapping_add(1);
            cx.notify();
        });
    })
    .checked("replace only test fixture snippets");
}

fn current_editor(fixture: &Fixture, cx: &App) -> Entity<crate::snippet_editor::SnippetEditor> {
    fixture
        .workspace
        .read(cx)
        .snippet_editor
        .clone()
        .checked_option("snippet editor is open")
}

fn replace_focused_text(window: &mut Window, value: &str, cx: &mut App) {
    // Resolve the input through actual focus after clicking it, rather than
    // exposing the modal's private fields to workspace tests.
    match window
        .focused_input(cx)
        .checked_option("clicked editable input owns focus")
    {
        AnyInputState::Input(field) => {
            field.update(cx, |field, cx| field.set_value(value, window, cx))
        }
        AnyInputState::Textarea(field) => {
            field.update(cx, |field, cx| field.set_value(value, window, cx))
        }
        _ => panic!("unexpected input type in command editor"),
    }
}

async fn wait_saved(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
}

async fn prepare_suggestion(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        let input = fixture.workspace.read(cx).command.entity_id();
        window.render_frame(cx);
        window.click(("input", input), cx);
        window.input("needle", cx);
    })
    .checked("type query into actual command field");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).command_candidates(cx).is_empty()
    })
    .await;
}

#[gpui_kit::test]
async fn offline_library_creates_blank_draft_and_persists_edits_with_stable_identity(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    seed(&fixture, Vec::new(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("commands", cx);
        window.click("new-snippet", cx);
    })
    .checked("open snippet editor");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(fixture.workspace.read(cx).tabs.is_empty());
        assert!(fixture.workspace.read(cx).remote_sessions.is_empty());
        assert_eq!(window.find("snippet-name").value(), Some(""));
        window.click("snippet-name", cx);
        window.input("离线片段", cx);
        window.click("snippet-description", cx);
        window.input("人工审核后使用", cx);
        window.click("snippet-tags", cx);
        window.input("诊断, \"disk,space\", \"quote\"\"tag\"", cx);
        fixture.workspace.update(cx, |workspace, cx| {
            let editor = workspace
                .snippet_editor
                .clone()
                .checked_option("snippet editor command");
            editor.update(cx, |editor, cx| editor.set_command_text(BODY, window, cx));
        });
        window.click("snippet-editor-save", cx);
    })
    .checked("create snippet entirely without SSH");
    wait_saved(&fixture, cx).await;
    let saved = fixture
        .store
        .load()
        .checked("load created snippet")
        .snippets[0]
        .clone();
    assert_eq!(saved.name, "离线片段");
    assert_eq!(saved.command, BODY);
    assert_eq!(saved.tags, snippet().tags);
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(fixture.workspace.read(cx).snippet_editor.is_none());
        window.render_frame(cx);
        window.click(SharedString::from(format!("edit-snippet-{}", saved.id)), cx);
        window.click("snippet-name", cx);
        replace_focused_text(window, "改名后仍是同一片段", cx);
        window.click("snippet-editor-save", cx);
    })
    .checked("edit only the saved snippet name");
    wait_saved(&fixture, cx).await;
    let edited = fixture.store.load().checked("load edited snippet").snippets[0].clone();
    assert_eq!(edited.id, saved.id);
    assert_eq!(edited.name, "改名后仍是同一片段");
    assert_eq!(edited.command, saved.command);
    assert_eq!(edited.description, saved.description);
    assert_eq!(edited.tags, saved.tags);
}

#[gpui_kit::test]
async fn deleting_a_snippet_requires_confirmation_and_keeps_previously_inserted_text(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let saved = snippet();
    seed(&fixture, vec![saved.clone()], cx);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("commands", cx);
        window.click(
            SharedString::from(format!("insert-snippet-{}", saved.id)),
            cx,
        );
        window.click(
            SharedString::from(format!("delete-snippet-{}", saved.id)),
            cx,
        );
        assert!(fixture.workspace.read(cx).snippet_delete.is_some());
        assert!(!fixture.workspace.read(cx).saving);
    })
    .checked("request deletion after explicitly inserting the snippet");
    assert_eq!(
        fixture
            .store
            .load()
            .checked("before deletion confirmation")
            .snippets,
        vec![saved.clone()]
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("cancel-delete-snippet", cx);
    })
    .checked("cancel first deletion request");
    cx.run_until_parked();
    assert_eq!(
        fixture
            .store
            .load()
            .checked("after cancelled deletion")
            .snippets,
        vec![saved.clone()]
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(
            SharedString::from(format!("delete-snippet-{}", saved.id)),
            cx,
        );
        window.click("confirm-delete-snippet", cx);
    })
    .checked("confirm second deletion request");
    wait_saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("persisted deletion")
            .snippets
            .is_empty()
    );
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.command.read(cx).value().as_str(), BODY);
        assert_eq!(
            workspace.command_target,
            Some(panes[0].terminal.entity_id())
        );
    });
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn asynchronous_suggestion_inserts_exact_multiline_text_and_run_keeps_its_target(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    seed(&fixture, vec![snippet()], cx);
    let panes = attach_remote_panes(&fixture, cx);
    prepare_suggestion(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("command-suggestion", 0_usize), cx);
    })
    .checked("click the asynchronously prepared suggestion");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.command.read(cx).value().as_str(), BODY);
        assert_eq!(
            workspace.command_target,
            Some(panes[0].terminal.entity_id())
        );
    });
    assert!(writes(&panes[0]).is_empty(), "insertion must not execute");
    assert!(writes(&panes[1]).is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("session-tab", 1_usize), cx);
        window.click("run-command", cx);
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.run_command(window, cx));
    })
    .checked("mismatched active tab cannot send the reviewed draft");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("session-tab", 0_usize), cx);
        window.click("run-command", cx);
    })
    .checked("manually execute on the original reviewed tab");
    cx.run_until_parked();
    assert_eq!(writes(&panes[0]), format!("{BODY}\r").as_bytes());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn stale_suggestion_tickets_reject_input_tab_source_and_closed_terminal_changes(
    cx: &mut TestAppContext,
) {
    for change in ["input", "tab", "source", "closed"] {
        let fixture = mount(cx, Vec::new());
        seed(&fixture, vec![snippet()], cx);
        let panes = attach_remote_panes(&fixture, cx);
        prepare_suggestion(&fixture, cx).await;
        let ticket = fixture.workspace.read_with(cx, |workspace, cx| {
            workspace.command_candidates(cx).remove(0)
        });
        if change == "closed" {
            panes[0]
                ._output
                .send(SessionEvent::Exited {
                    code: 0,
                    success: true,
                })
                .checked("close fixture remote process");
            cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
                !panes[0].terminal.read(cx).is_open()
            })
            .await;
        }
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                match change {
                    "input" => {
                        // Returning to the same text does not revive an older
                        // reviewed ticket: its input revision still changed.
                        let target = workspace.command_target;
                        workspace.set_reviewed_command("edited".into(), target, window, cx);
                        workspace.set_reviewed_command("needle".into(), target, window, cx);
                    }
                    "tab" => workspace.active = 1,
                    "source" => workspace.state.snippets.clear(),
                    _ => {}
                }
                workspace.insert_candidate(ticket, window, cx);
                assert_eq!(
                    workspace.command.read(cx).value().as_str(),
                    "needle",
                    "stale {change} ticket changed the draft"
                );
                assert!(workspace.status.render(cx).contains("已变化"));
            });
        })
        .checked("refuse stale candidate at the production admission boundary");
        cx.run_until_parked();
        assert!(writes(&panes[0]).is_empty(), "{change} ticket sent bytes");
        assert!(
            writes(&panes[1]).is_empty(),
            "{change} ticket sent bytes to another host"
        );
    }
}

#[gpui_kit::test]
async fn command_history_and_unsaved_drafts_never_enter_persisted_settings(
    cx: &mut TestAppContext,
) {
    const HISTORY: &str = "printf private-history-sentinel-3917";
    const DRAFT: &str = "printf unsaved-draft-sentinel-8442";
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.set_reviewed_command(
                HISTORY.into(),
                Some(panes[0].terminal.entity_id()),
                window,
                cx,
            );
            workspace.run_command(window, cx);
            workspace.set_reviewed_command(
                DRAFT.into(),
                Some(panes[0].terminal.entity_id()),
                window,
                cx,
            );
            workspace.switch_language(window, cx);
        });
    })
    .checked("run a command then persist only a language change");
    wait_saved(&fixture, cx).await;
    let json = std::fs::read_to_string(fixture.store.path()).checked("inspect test state bytes");
    assert!(!json.contains(HISTORY));
    assert!(!json.contains(DRAFT));
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.command.read(cx).value().as_str(), DRAFT);
        assert_eq!(
            workspace.command_histories[&panes[0].terminal.entity_id()]
                .newest_first()
                .collect::<Vec<_>>(),
            vec![HISTORY]
        );
    });
    assert_eq!(writes(&panes[0]), format!("{HISTORY}\r").as_bytes());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn snippet_modal_captures_typing_and_save_conflict_retains_the_complete_draft(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let saved = snippet();
    seed(&fixture, vec![saved.clone()], cx);
    let panes = attach_remote_panes(&fixture, cx);
    let external_store = StateStore::new(fixture.store.path());
    let mut external = external_store.load().checked("external state snapshot");
    external.settings.font_size = 19.;
    external_store
        .save(&external)
        .checked("publish conflicting external setting");
    let draft = cx
        .update_window(fixture.window, |_, window, cx| {
            window.click("commands", cx);
            window.click(SharedString::from(format!("edit-snippet-{}", saved.id)), cx);
            window.input("编辑内容只进入草稿", cx);
            let editor = current_editor(&fixture, cx);
            let draft = editor.read(cx).draft(cx).checked("edited modal snapshot");
            assert!(draft.name.contains("编辑内容只进入草稿"));
            fixture
                .workspace
                .update(cx, |workspace, cx| workspace.run_command(window, cx));
            window.click("snippet-editor-save", cx);
            draft
        })
        .checked("type into modal and submit stale disk snapshot");
    wait_saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        let editor = current_editor(&fixture, cx);
        assert!(!editor.read(cx).is_saving());
        assert_eq!(
            editor.read(cx).draft(cx).checked("conflict retains draft"),
            draft
        );
        window.render_frame(cx);
        assert!(window.find("snippet-editor-error").visible());
        window.click("snippet-name", cx);
        window.input("仍可编辑", cx);
        assert!(
            editor
                .read(cx)
                .draft(cx)
                .checked("editable after conflict")
                .name
                .contains("仍可编辑")
        );
        window.click("snippet-editor-cancel", cx);
    })
    .checked("failed save stays reviewable and can be cancelled");
    cx.run_until_parked();
    let disk = external_store
        .load()
        .checked("conflict preserves external changes");
    assert_eq!(disk.settings.font_size, 19.);
    assert_eq!(disk.snippets, vec![saved]);
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
fn bilingual_multiline_command_bar_stays_bounded_and_never_sends_enter_implicitly(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.set_reviewed_command(
                BODY.repeat(30),
                Some(panes[0].terminal.entity_id()),
                window,
                cx,
            )
        });
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            let field = window.find(("input", fixture.workspace.read(cx).command.entity_id()));
            let run = window.find("run-command");
            assert!(field.visible());
            assert!(field.bounds().size.height <= px(64.));
            assert!(field.bounds().right() <= run.bounds().origin.x);
            assert!(run.visible());
            assert!(run.bounds().right() <= window.bounds().right());
            assert_eq!(
                run.label(),
                Some(if language == Language::ZhCn {
                    "执行"
                } else {
                    "Run"
                })
            );
        }
        let input = fixture.workspace.read(cx).command.entity_id();
        window.click(("input", input), cx);
        window.press("enter", cx);
        assert!(
            !fixture
                .workspace
                .read(cx)
                .command
                .read(cx)
                .value()
                .is_empty()
        );
    })
    .checked("bounded actual multiline input in both languages");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

fn eight_snippets() -> Vec<Snippet> {
    (0..8)
        .map(|index| {
            Snippet::new(
                format!("needle {index}"),
                format!("printf 'candidate-{index}'\n\tprintf 'reviewed 中文-{index}'\n"),
            )
        })
        .collect()
}

#[gpui_kit::test]
async fn keyboard_selection_scrolls_the_eighth_suggestion_into_view_before_insertion(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let snippets = eight_snippets();
    let eighth = snippets[7].command.clone();
    seed(&fixture, snippets, cx);
    let panes = attach_remote_panes(&fixture, cx);
    prepare_suggestion(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(fixture.workspace.read(cx).command_candidates(cx).len(), 8);
        assert_eq!(fixture.workspace.read(cx).suggestion_selected, 0);
        let input = fixture.workspace.read(cx).command.clone();
        let mut handler =
            ElementInputHandler::new(window.find(("input", input.entity_id())).bounds(), input);
        assert_eq!(
            handler.marked_text_range(window, cx),
            None,
            "ordinary query unexpectedly has IME preedit"
        );
        // Exercise wrap in both directions as well as sequential movement.
        window.press("up", cx);
        assert_eq!(fixture.workspace.read(cx).suggestion_selected, 7);
        window.press("down", cx);
        assert_eq!(fixture.workspace.read(cx).suggestion_selected, 0);
        for _ in 0..7 {
            window.press("down", cx);
        }
        assert_eq!(fixture.workspace.read(cx).suggestion_selected, 7);
        window.render_frame(cx);
        let viewport = window.find("command-suggestions").bounds();
        let row = window.find(("command-suggestion", 7_usize));
        assert!(row.visible());
        assert!(
            row.bounds().origin.y >= viewport.origin.y,
            "selected row {:?} escaped viewport {viewport:?}",
            row.bounds()
        );
        assert!(
            row.bounds().bottom() <= viewport.bottom(),
            "selected row {:?} escaped viewport {viewport:?}",
            row.bounds()
        );
        assert!(viewport.size.height <= px(176.));
        window.press("enter", cx);
        assert_eq!(
            fixture.workspace.read(cx).command.read(cx).value().as_str(),
            eighth
        );
        assert_eq!(
            fixture.workspace.read(cx).command_target,
            Some(panes[0].terminal.entity_id())
        );
    })
    .checked("keyboard navigation keeps the selected row visible");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn marked_ime_text_keeps_enter_arrows_and_escape_out_of_suggestion_actions(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let snippets = eight_snippets();
    seed(&fixture, snippets.clone(), cx);
    let panes = attach_remote_panes(&fixture, cx);
    prepare_suggestion(&fixture, cx).await;
    for key in ["enter", "down", "up", "escape"] {
        cx.update_window(fixture.window, |_, window, cx| {
            let input = fixture.workspace.read(cx).command.clone();
            input.update(cx, |input, cx| input.set_value("needle", window, cx));
            input.read(cx).focus_handle(cx).focus(window, cx);
            let mut handler =
                ElementInputHandler::new(window.find(("input", input.entity_id())).bounds(), input);
            handler.replace_and_mark_text_in_range(Some(0..6), "needle", Some(0..6), window, cx);
            assert!(handler.marked_text_range(window, cx).is_some());
        })
        .checked("establish an actual marked composition in the command input");
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            fixture.workspace.read(cx).command_candidates(cx).len() == 8
        })
        .await;
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.suggestion_selected = 3;
                cx.notify();
            });
            window.render_frame(cx);
            let input = fixture.workspace.read(cx).command.clone();
            let mut handler =
                ElementInputHandler::new(window.find(("input", input.entity_id())).bounds(), input);
            assert!(
                handler.marked_text_range(window, cx).is_some(),
                "preedit missing before {key}"
            );
            let dismissed = fixture.workspace.read(cx).suggestion_dismissed.clone();
            window.press(key, cx);
            let workspace = fixture.workspace.read(cx);
            assert_eq!(
                workspace.suggestion_selected, 3,
                "{key} moved a suggestion during composition"
            );
            assert_eq!(
                workspace.suggestion_dismissed, dismissed,
                "{key} dismissed suggestions during composition"
            );
            assert!(
                !snippets
                    .iter()
                    .any(|snippet| snippet.command == workspace.command.read(cx).value().as_str()),
                "{key} accepted a suggestion during composition"
            );
        })
        .checked("dispatch keyboard input while the platform input handler owns a preedit");
    }
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn escape_during_an_actual_pending_match_suppresses_its_late_results_until_new_input(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    seed(&fixture, eight_snippets(), cx);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace
                .command
                .update(cx, |input, cx| input.set_value("needle", window, cx));
            workspace
                .command
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            // Start the real production background task. The test dispatcher
            // cannot deliver its foreground result until this callback returns.
            workspace.refresh_command_suggestions(window, cx);
            assert!(workspace.suggestion_job_running);
            assert!(workspace.suggestion_request.is_some());
            assert!(workspace.suggestion_cache.is_empty());
        });
        window.press("escape", cx);
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.suggestion_job_running);
        assert_eq!(
            workspace.suggestion_dismissed,
            Some((workspace.command_revision, "needle".to_owned()))
        );
        assert!(workspace.command_candidates(cx).is_empty());
    })
    .checked("dismiss before the real matching worker can deliver results");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).suggestion_job_running
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(
            workspace.suggestion_cache.len(),
            8,
            "matching worker must actually have produced its result"
        );
        assert!(workspace.command_candidates(cx).is_empty());
        assert!(window.try_find("command-suggestions").is_none());
        assert_eq!(workspace.command.read(cx).value().as_str(), "needle");
        window.input(" ", cx);
    })
    .checked("late results stay hidden and a new user edit starts a new request");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        fixture.workspace.read(cx).command_candidates(cx).len() == 8
    })
    .await;
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}
