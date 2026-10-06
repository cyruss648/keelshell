//! Production review surfaces backed by separately authenticated loopback exec peers.
use super::batch_peer as peer;
use super::*;
use crate::command_suggestions::SuggestionSource;
use gpui_kit::{
    App, ScrollDelta, Window,
    component::{WindowExt, input::AnyInputState},
    point, px,
};
use keelshell_core::{BatchAuditRecord, BatchAuditSummary, Snippet};

fn seed(fixture: &Fixture, cx: &mut TestAppContext) -> Snippet {
    let mut snippet = Snippet::new("参数化检查", "printf '%s\\n' {{path}}");
    snippet.parameterized = true;
    fixture.workspace.update(cx, |view, cx| {
        view.state.snippets = vec![snippet.clone()];
        view.state = fixture
            .store
            .save(&view.state)
            .checked("persist opt-in template");
        view.snippet_sources = Arc::new(view.state.snippets.clone());
        view.command_sources_revision = view.command_sources_revision.wrapping_add(1);
        cx.notify();
    });
    snippet
}
fn replace(window: &mut Window, text: &str, cx: &mut App) {
    match window
        .focused_input(cx)
        .unwrap_or_else(|| panic!("focused input"))
    {
        AnyInputState::Input(input) => input.update(cx, |input, cx| {
            input.set_selected_range(0..input.value().len(), cx);
            input.replace(text.to_owned(), window, cx);
        }),
        AnyInputState::Textarea(input) => input.update(cx, |input, cx| {
            input.set_selected_range(0..input.value().len(), cx);
            input.replace(text.to_owned(), window, cx);
        }),
        _ => panic!("unexpected focused input"),
    }
}
fn open_parameters(fixture: &Fixture, snippet: &Snippet, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let ticket = view
                .candidate(
                    snippet.command.clone(),
                    snippet.name.clone(),
                    SuggestionSource::Snippet(snippet.id),
                    cx,
                )
                .unwrap_or_else(|| panic!("live snippet candidate"));
            view.insert_candidate(ticket, window, cx);
        });
        window.render_frame(cx);
        window.click(("snippet-parameter-input", 0_usize), cx);
        window.input("中文'$(unsafe)", cx);
    })
    .checked("open actual parameter review and enter literal value");
    cx.run_until_parked();
}

fn pending_audit(command: &str) -> BatchAuditRecord {
    BatchAuditRecord::new(
        command,
        1_725_000_000,
        vec![],
        BatchAuditSummary {
            target_count: 1,
            succeeded: 1,
            failed: 0,
            unknown: 0,
            not_started: 0,
            cancelled: false,
            stopped_after_failure: false,
        },
    )
    .checked("create pending batch audit")
}

#[gpui_kit::test]
fn closing_snippet_modal_flushes_pending_batch_audits(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let audit = pending_audit("printf 'snippet-close'");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.pending_batch_audits.push(audit.clone());
            workspace.open_snippet_editor(None, window, cx);
            workspace.close_snippet_modal(window, cx);
            assert!(workspace.saving);
        });
    })
    .checked("close snippet modal and flush pending batch audit");
    cx.run_until_parked();
    let persisted = fixture.store.load().checked("reload snippet-close audit");
    assert_eq!(persisted.batch_audits, vec![audit]);
}

#[gpui_kit::test]
fn closing_vault_modal_flushes_pending_batch_audits(cx: &mut TestAppContext) {
    use crate::vault_settings::VaultSettingsEvent;

    let fixture = mount(cx, Vec::new());
    let audit = pending_audit("printf 'vault-close'");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.pending_batch_audits.push(audit.clone());
            workspace.open_vault_settings(window, cx);
            let panel = workspace
                .vault_settings
                .clone()
                .checked_option("vault panel is open");
            panel.update(cx, |_, cx| {
                cx.emit(VaultSettingsEvent::Close { message: None })
            });
        });
    })
    .checked("close vault modal and flush pending batch audit");
    cx.run_until_parked();
    let persisted = fixture.store.load().checked("reload vault-close audit");
    assert_eq!(persisted.batch_audits, vec![audit]);
}

#[gpui_kit::test]
fn parameter_review_never_executes_and_expanded_values_skip_history(cx: &mut TestAppContext) {
    let f = mount(cx, Vec::new());
    let panes = attach_remote_panes(&f, cx);
    let snippet = seed(&f, cx);
    open_parameters(&f, &snippet, cx);
    assert!(writes(&panes[0]).is_empty());
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-parameters-insert", cx);
    })
    .checked("explicit parameter insertion");
    cx.run_until_parked();
    f.workspace.read_with(cx, |view, cx| {
        assert!(view.snippet_parameters.is_none());
        assert!(!view.command_record_history);
        assert_eq!(
            view.command.read(cx).value().as_str(),
            "printf '%s\\n' '中文'\\''$(unsafe)'"
        );
    });
    assert!(writes(&panes[0]).is_empty());
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("run-command", cx);
    })
    .checked("manually run expanded command");
    cx.run_until_parked();
    assert_eq!(
        writes(&panes[0]),
        b"printf '%s\\n' '\xe4\xb8\xad\xe6\x96\x87'\\''$(unsafe)'\r"
    );
    assert!(writes(&panes[1]).is_empty());
    f.workspace.read_with(cx, |view, _| {
        assert!(
            view.command_histories
                .values()
                .all(|h| h.newest_first().next().is_none())
        )
    });
    let disk = f.store.load().checked("reload persisted source");
    assert_eq!(disk.snippets, vec![snippet]);
}

#[gpui_kit::test]
fn parameter_authorization_rejects_changed_source_input_or_target(cx: &mut TestAppContext) {
    for change in ["source", "input", "target"] {
        let f = mount(cx, Vec::new());
        let panes = attach_remote_panes(&f, cx);
        let snippet = seed(&f, cx);
        open_parameters(&f, &snippet, cx);
        cx.update_window(f.window, |_, window, cx| {
            f.workspace.update(cx, |view, cx| match change {
                "source" => view.state.snippets[0].parameterized = false,
                "input" => view.set_reviewed_command(
                    "new draft".into(),
                    Some(panes[0].terminal.entity_id()),
                    window,
                    cx,
                ),
                _ => view.active = 1,
            });
            window.render_frame(cx);
            window.click("snippet-parameters-insert", cx);
        })
        .checked("invalidate source, draft or entity before accepting");
        cx.run_until_parked();
        f.workspace.read_with(cx, |view, cx| {
            assert!(view.snippet_parameters.is_some());
            assert!(!view.command.read(cx).value().contains("unsafe"));
        });
        assert!(writes(&panes[0]).is_empty());
        assert!(writes(&panes[1]).is_empty());
    }
}

struct Harness {
    fixture: Fixture,
    panes: Vec<RemotePane>,
    servers: Vec<peer::Server>,
}
impl Harness {
    fn new(cx: &mut TestAppContext, codes: [u32; 2]) -> Self {
        let fixture = mount(cx, Vec::new());
        let panes = attach_remote_panes(&fixture, cx);
        let runtime = fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        let servers = codes
            .into_iter()
            .map(|code| peer::Server::new(&runtime, code))
            .collect::<Vec<_>>();
        fixture.workspace.update(cx, |view, cx| {
            for (pane, server) in panes.iter().zip(&servers) {
                view.remote_sessions
                    .insert(pane.terminal.entity_id(), server.session.clone());
            }
            cx.notify();
        });
        Self {
            fixture,
            panes,
            servers,
        }
    }
    fn prepare(&self, text: &str, concurrency: &str, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            self.fixture.workspace.update(cx, |view, cx| {
                view.set_reviewed_command(
                    text.into(),
                    Some(self.panes[0].terminal.entity_id()),
                    window,
                    cx,
                );
                view.open_batch_commands(false, window, cx);
            });
            window.render_frame(cx);
            window.click(("batch-select", 0_usize), cx);
            window.click(("batch-select", 1_usize), cx);
            window.click("batch-concurrency", cx);
            replace(window, concurrency, cx);
        })
        .checked("select two live SSH sessions and set execution options");
        cx.run_until_parked();
        cx.update_window(self.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("batch-review-button", cx);
        })
        .checked("prepare exact review after input change events settle");
        cx.run_until_parked();
    }
    fn confirm(&self, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            if window.try_find("batch-confirm").is_some() {
                window.click("batch-confirm", cx);
            }
        })
        .checked("explicit batch confirmation when the review is still valid");
        cx.run_until_parked();
    }
    async fn complete(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.fixture.window, Duration::from_secs(8), |_, cx| {
            self.fixture
                .workspace
                .read(cx)
                .batch_panel
                .as_ref()
                .is_some_and(|p| !p.read(cx).is_running())
        })
        .await;
    }
}

#[gpui_kit::test]
async fn batch_template_review_binds_distinct_target_commands_and_separates_terminal_history(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, [0, 7]);
    let text = "printf '%s' {{endpoint}}";
    h.prepare(text, "2", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("batch-reviewed-target-commands").visible());
        assert!(
            window
                .find(("batch-reviewed-target-command", 0_usize))
                .visible()
        );
        assert!(
            window
                .try_find(("batch-reviewed-target-command", 1_usize))
                .is_some()
        );
        let list = window.find("batch-reviewed-target-commands").bounds();
        let last = window
            .find(("batch-reviewed-target-command", 1_usize))
            .bounds();
        window.scroll(
            "batch-reviewed-target-commands",
            ScrollDelta::Pixels(point(px(0.), list.origin.y + px(8.) - last.bottom())),
            cx,
        );
        window.render_frame(cx);
        assert!(
            window
                .find(("batch-reviewed-target-command", 1_usize))
                .visible()
        );
    })
    .checked("inspect per-target rendered batch commands before confirmation");
    assert!(h.servers.iter().all(|s| s.requests().is_empty()));
    h.confirm(cx);
    h.complete(cx).await;
    cx.run_until_parked();
    assert_eq!(
        h.servers[0].requests(),
        vec![b"printf '%s' 'fixture-0@example.invalid:22'".to_vec()]
    );
    assert_eq!(
        h.servers[1].requests(),
        vec![b"printf '%s' 'fixture-1@example.invalid:22'".to_vec()]
    );
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    h.fixture
        .workspace
        .read_with(cx, |view, _| assert!(view.command_histories.is_empty()));
    let audit = h
        .fixture
        .store
        .load()
        .checked("reload persisted batch audit")
        .batch_audits;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].target_count, 2);
    assert_eq!(audit[0].succeeded, 1);
    assert_eq!(audit[0].failed, 1);
    let encoded = serde_json::to_string(&audit[0]).checked("serialize persisted batch audit");
    assert!(!encoded.contains(text));
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("batch-confirm").is_none());
        assert!(window.find("batch-output").visible());
        window.click("batch-copy-output", cx);
    })
    .checked("inspect batch output without terminal injection");
    let clip = cx.update(|cx| {
        cx.read_from_clipboard()
            .unwrap_or_else(|| panic!("copied output"))
            .text()
            .unwrap_or_else(|| panic!("text clipboard"))
    });
    assert!(clip.contains("fixture stdout"));
    assert!(!clip.contains('\u{1b}'));
}

#[gpui_kit::test]
async fn batch_literal_review_preserves_exact_bytes_and_separates_terminal_history(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, [0, 7]);
    let text = "printf '中文'\nprintf 'two'";
    h.prepare(text, "2", cx);
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.confirm(cx);
    h.complete(cx).await;
    cx.run_until_parked();
    for server in &h.servers {
        assert_eq!(server.requests(), vec![text.as_bytes().to_vec()]);
    }
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    h.fixture
        .workspace
        .read_with(cx, |view, _| assert!(view.command_histories.is_empty()));
}

#[gpui_kit::test]
async fn batch_stop_after_failure_does_not_start_remaining_host(cx: &mut TestAppContext) {
    let h = Harness::new(cx, [9, 0]);
    h.prepare("reviewed command", "1", cx);
    h.confirm(cx);
    h.complete(cx).await;
    assert_eq!(h.servers[0].requests(), vec![b"reviewed command".to_vec()]);
    assert!(h.servers[1].requests().is_empty());
}

#[gpui_kit::test]
async fn batch_cancellation_keeps_receipts_and_returning_to_workspace_does_not_cancel(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, [0, 0]);
    h.prepare("hold", "1", cx);
    h.confirm(cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-hide", cx);
    })
    .checked("keep owned task while returning to workspace");
    cx.run_until_parked();
    h.fixture.workspace.read_with(cx, |view, cx| {
        assert!(!view.show_batch);
        assert!(
            view.batch_panel
                .as_ref()
                .unwrap_or_else(|| panic!("retained panel"))
                .read(cx)
                .is_running()
        );
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("command-batch", cx);
        window.render_frame(cx);
        window.click("batch-cancel", cx);
    })
    .checked("reopen and cancel batch");
    h.complete(cx).await;
    assert!(h.servers[1].requests().is_empty());
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    assert_eq!(
        runtime
            .block_on(h.servers[0].session.exec("still usable"))
            .checked("cancel preserves SSH")
            .exit_status,
        Some(0)
    );
}

#[gpui_kit::test]
fn batch_review_rejects_a_closed_or_replaced_terminal(cx: &mut TestAppContext) {
    let h = Harness::new(cx, [0, 0]);
    h.prepare("never send", "1", cx);
    h.fixture.workspace.update(cx, |view, _| {
        view.tabs.remove(0);
    });
    h.confirm(cx);
    assert!(h.servers.iter().all(|s| s.requests().is_empty()));
}

#[gpui_kit::test]
async fn batch_same_endpoint_replacement_connection_cannot_inherit_review(cx: &mut TestAppContext) {
    let h = Harness::new(cx, [0, 0]);
    h.prepare("reviewed replacement control", "1", cx);
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find("batch-confirm").visible(),
            "original review must be confirmable before replacement"
        );
    })
    .checked("original exact review is present before changing connection identity");
    h.fixture.workspace.update(cx, |view, _| {
        // Keep the terminal entity, endpoint and route untouched, but substitute
        // a second separately authenticated SSH connection under that entity.
        view.remote_sessions.insert(
            h.panes[0].terminal.entity_id(),
            h.servers[1].session.clone(),
        );
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        if window.try_find("batch-confirm").is_some() {
            window.click("batch-confirm", cx);
        }
    })
    .checked("confirm only if the review is still available after replacement");
    cx.run_until_parked();
    h.complete(cx).await;
    let counts = h
        .servers
        .iter()
        .map(|server| server.requests().len())
        .collect::<Vec<_>>();
    eprintln!("separately authenticated replacement control request counts: {counts:?}");
    assert!(
        h.servers.iter().all(|server| server.requests().is_empty()),
        "a replacement authenticated connection received the old review"
    );
}

#[gpui_kit::test]
async fn batch_custom_parameters_use_actual_target_fields_and_never_persist_value_derivatives(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, [0, 0]);
    let before = h.fixture.store.load().checked("original persisted state");
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.set_reviewed_command(
                "printf '%s' {{path}}".into(),
                Some(h.panes[0].terminal.entity_id()),
                window,
                cx,
            );
            view.open_batch_commands(false, window, cx);
        });
        window.render_frame(cx);
        window.click(("batch-select", 0_usize), cx);
        window.click(("batch-select", 1_usize), cx);
        window.click("batch-sync-parameters", cx);
        window.render_frame(cx);
        window.click("batch-parameters-0-path", cx);
        replace(window, "first'$(no)", cx);
        let last = window.find("batch-parameters-1-path").bounds();
        let body = window.find("batch-body").bounds();
        window.scroll(
            "batch-body",
            ScrollDelta::Pixels(point(px(0.), body.origin.y + px(8.) - last.bottom())),
            cx,
        );
        window.render_frame(cx);
        window.click("batch-parameters-1-path", cx);
        replace(window, "second 中文", cx);
    })
    .checked("actual sync button and independent per-target value inputs");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-review-button", cx);
    })
    .checked("review after real input change events settle");
    cx.run_until_parked();
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.confirm(cx);
    h.complete(cx).await;
    cx.run_until_parked();
    assert_eq!(
        h.servers[0].requests(),
        vec![b"printf '%s' 'first'\\''$(no)'".to_vec()]
    );
    assert_eq!(
        h.servers[1].requests(),
        vec!["printf '%s' 'second 中文'".as_bytes().to_vec()]
    );
    h.fixture.workspace.read_with(cx, |view, _| {
        assert!(view.pending_batch_audits.is_empty());
        assert!(view.command_histories.is_empty());
    });
    let after = h
        .fixture
        .store
        .load()
        .checked("persisted parameter-free state");
    assert_eq!(after.batch_audits, before.batch_audits);
    let encoded = serde_json::to_string(&after).checked("persisted metadata contents");
    assert!(!encoded.contains("first") && !encoded.contains("second 中文"));
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn closing_selected_tab_cancels_hidden_batch_without_rebinding(cx: &mut TestAppContext) {
    let h = Harness::new(cx, [0, 0]);
    h.prepare("hold", "1", cx);
    h.confirm(cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-hide", cx);
    })
    .checked("hide running batch before closing its session");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.close_tab(&crate::workspace::CloseTab, window, cx);
            assert_eq!(view.tabs.len(), 1);
        });
    })
    .checked("closing the captured entity cancels its hidden batch");
    h.complete(cx).await;
    assert!(h.servers[1].requests().is_empty());
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn batch_continue_policy_admits_next_host_after_failure(cx: &mut TestAppContext) {
    let h = Harness::new(cx, [7, 0]);
    h.prepare("continue explicitly", "1", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-back", cx);
        window.render_frame(cx);
        window.click("batch-failure-policy", cx);
        window.click("batch-review-button", cx);
    })
    .checked("explicitly select continue and review the new policy");
    h.confirm(cx);
    h.complete(cx).await;
    for server in &h.servers {
        assert_eq!(server.requests(), vec![b"continue explicitly".to_vec()]);
    }
}

#[gpui_kit::test]
fn batch_review_keeps_fixed_actions_visible_and_blocks_other_modal_shortcuts(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, [0, 0]);
    h.prepare("review only", "1", cx);
    for (width, height) in [(900., 580.), (1440., 900.)] {
        cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
        cx.run_until_parked();
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                h.fixture.workspace.update(cx, |view, cx| {
                    let assistant = view.show_assistant;
                    view.open_connections(&crate::workspace::OpenConnections, window, cx);
                    view.open_ai_settings(window, cx);
                    view.open_vault_settings(window, cx);
                    view.split_remote(window, cx);
                    view.toggle_assistant(&crate::workspace::ToggleAssistant, window, cx);
                    assert!(view.show_batch);
                    assert!(!view.show_connections);
                    assert!(view.ai_settings.is_none() && view.vault_settings.is_none());
                    assert_eq!(view.show_assistant, assistant);
                    assert_eq!(view.tabs.len(), 2);
                });
                window.render_frame(cx);
                for id in ["batch-footer", "batch-confirm", "batch-hide", "batch-back"] {
                    let bounds = window.find(id).bounds();
                    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                    assert!(bounds.origin.x >= px(0.) && bounds.origin.y >= px(0.));
                    assert!(
                        bounds.right() <= px(width) && bounds.bottom() <= px(height),
                        "{id} {bounds:?}"
                    );
                }
                assert!(window.find("batch-reviewed-command").visible());
            })
            .checked("review retains action geometry and modal ownership across resize and locale");
        }
    }
    assert!(h.servers.iter().all(|s| s.requests().is_empty()));
}

#[gpui_kit::test]
fn parameter_delete_undo_retains_history_choice_and_original_target(cx: &mut TestAppContext) {
    let f = mount(cx, Vec::new());
    let panes = attach_remote_panes(&f, cx);
    let snippet = seed(&f, cx);
    open_parameters(&f, &snippet, cx);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-parameters-insert", cx);
    })
    .checked("accept parameter preview");
    cx.run_until_parked();
    let original = f
        .workspace
        .read_with(cx, |view, cx| view.command.read(cx).value().to_string());
    cx.update_window(f.window, |_, window, cx| {
        let input = f.workspace.read(cx).command.entity_id();
        window.render_frame(cx);
        window.click(("input", input), cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.press("backspace", cx);
    })
    .checked("delete parameter text using actual editing action");
    cx.run_until_parked();
    f.workspace.read_with(cx, |view, cx| {
        assert!(view.command.read(cx).value().is_empty());
        assert!(!view.command_record_history);
        assert_eq!(view.command_target, Some(panes[0].terminal.entity_id()));
    });
    cx.update_window(f.window, |_, window, cx| {
        window.click(("session-tab", 1_usize), cx);
        let input = f.workspace.read(cx).command.entity_id();
        window.render_frame(cx);
        window.click(("input", input), cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            },
            cx,
        );
    })
    .checked("restore draft after switching tabs");
    cx.run_until_parked();
    f.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.command.read(cx).value().as_str(), original);
        assert!(!view.command_record_history);
        assert_eq!(view.command_target, Some(panes[0].terminal.entity_id()));
    });
    cx.update_window(f.window, |_, window, cx| {
        f.workspace
            .update(cx, |view, cx| view.run_command(window, cx));
        window.render_frame(cx);
        window.click("new-command-draft", cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            },
            cx,
        );
    })
    .checked("wrong target cannot run, new draft cannot restore old sensitive undo text");
    cx.run_until_parked();
    f.workspace.read_with(cx, |view, cx| {
        assert!(view.command.read(cx).value().is_empty());
        assert!(view.command_record_history);
        assert_eq!(view.command_target, None);
    });
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
