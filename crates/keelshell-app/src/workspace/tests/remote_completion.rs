//! Real GPUI controls backed by a pinned loopback SSH/SFTP peer.
use super::*;
use gpui_kit::{ElementInputHandler, InputHandler};
use std::sync::atomic::Ordering;
#[path = "remote_completion_peer.rs"]
mod peer;

struct Harness {
    fixture: Fixture,
    panes: Vec<RemotePane>,
    server: peer::Server,
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let fixture = mount(cx, Vec::new());
        let panes = attach_remote_panes(&fixture, cx);
        let runtime = fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        let server = peer::Server::new(&runtime);
        let session = server.connect(&runtime);
        runtime.block_on(async {
            let sftp = session.sftp().await.checked("seed completion files");
            sftp.mkdir("/docs").await.checked("seed docs directory");
            sftp.mkdir("/stall").await.checked("seed delayed directory");
            for name in ["report 中文'$(safe).txt", "report-second", "late-file"] {
                sftp.write(&format!("/docs/{name}"), b"fixture")
                    .await
                    .checked("seed literal filename");
                sftp.write(&format!("/stall/{name}"), b"fixture")
                    .await
                    .checked("seed delayed filename");
            }
            sftp.close().await.checked("close seed SFTP");
        });
        fixture.workspace.update(cx, |view, cx| {
            for pane in &panes {
                view.remote_sessions
                    .insert(pane.terminal.entity_id(), session.clone());
            }
            cx.notify();
        });
        Self {
            fixture,
            panes,
            server,
        }
    }
    fn draft(&self, text: &str, caret: usize, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            let command = self.fixture.workspace.read(cx).command.clone();
            window.render_frame(cx);
            window.click(("input", command.entity_id()), cx);
            command.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.replace(text.to_owned(), window, cx);
                input.set_selected_range(caret..caret, cx);
            });
        })
        .checked("type multi-line draft through actual Textarea editing API");
        cx.run_until_parked();
    }
    fn directory(&self, text: &str, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            let input = self
                .fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .clone();
            window.render_frame(cx);
            window.click(("input", input.entity_id()), cx);
            input.update(cx, |input, cx| {
                input.set_selected_range(0..input.value().len(), cx);
                input.replace(text.to_owned(), window, cx);
            });
        })
        .checked("edit explicit completion directory");
        cx.run_until_parked();
    }
    fn query(&self, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("remote-complete", cx);
        })
        .checked("explicit remote query button");
    }
    async fn results(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.fixture.window, Duration::from_secs(6), |_, cx| {
            !self
                .fixture
                .workspace
                .read(cx)
                .remote_completion
                .choices
                .is_empty()
        })
        .await;
    }
    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.fixture.window, Duration::from_secs(6), |_, cx| {
            !self.fixture.workspace.read(cx).remote_completion.busy()
        })
        .await;
    }
    fn text(&self, cx: &mut TestAppContext) -> String {
        self.fixture
            .workspace
            .read_with(cx, |view, cx| view.command.read(cx).value().to_string())
    }
    fn quiet(&self) {
        for pane in &self.panes {
            assert!(writes(pane).is_empty());
        }
    }
}

#[gpui_kit::test]
async fn explicit_remote_path_inserts_only_word_preserves_multiline_and_undo(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    let original = "printf '😀中文'\ncat rep && printf end";
    let caret = original.find("rep").checked_option("word position") + 3;
    h.draft(original, caret, cx);
    let before = h.server.opened.load(Ordering::Acquire);
    cx.run_until_parked();
    assert_eq!(
        h.server.opened.load(Ordering::Acquire),
        before,
        "typing never queries remote"
    );
    h.query(cx);
    h.results(cx).await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("remote-completion-choice", 0_usize), cx);
    })
    .checked("choose first literal remote filename");
    cx.run_until_parked();
    let expected = "printf '😀中文'\ncat '/docs/report 中文'\\''$(safe).txt' && printf end";
    assert_eq!(h.text(cx), expected);
    h.quiet();
    cx.update_window(h.fixture.window, |_, window, cx| {
        let command = h.fixture.workspace.read(cx).command.clone();
        assert_eq!(
            command.read(cx).cursor(),
            expected.find(" &&").checked_option("suffix")
        );
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            },
            cx,
        );
    })
    .checked("undo one atomic replacement");
    cx.run_until_parked();
    assert_eq!(h.text(cx), original);
    h.fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(
            view.command.read(cx).selected_range(),
            caret..caret,
            "Undo restores the original caret, not an invented selection"
        );
    });
    h.quiet();
}

#[gpui_kit::test]
async fn cursor_only_move_and_directory_edit_revoke_already_rendered_tickets(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    let choice = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.press("left", cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(choice.clone(), window, cx)
        });
    })
    .checked("cursor movement rejects stale click before observer delivery");
    cx.run_until_parked();
    assert_eq!(h.text(cx), "cat rep");
    assert!(
        !h.fixture
            .workspace
            .read_with(cx, |view, _| view.remote_completion.visible())
    );
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    let choice = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    h.directory("/stall", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(choice, window, cx)
        })
    })
    .checked("directory changes revoke ticket");
    assert_eq!(h.text(cx), "cat rep");
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    let old = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    cx.update_window(h.fixture.window,|_,window,cx|{
        h.fixture.workspace.update(cx,|view,cx|{
            view.set_reviewed_command("cat rep".into(),Some(h.panes[0].terminal.entity_id()),window,cx);
            view.command.update(cx,|input,cx|input.set_selected_range(7..7,cx));
            view.insert_remote_completion(old,window,cx);
        });
    }).checked("programmatic insertion revokes even when full text and caret are restored in the same turn");
    assert_eq!(h.text(cx), "cat rep");
    cx.run_until_parked();
    h.query(cx);
    h.results(cx).await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.press("shift-enter", cx);
    })
    .checked("Shift Enter remains a newline with candidates visible");
    cx.run_until_parked();
    assert_eq!(h.text(cx), "cat rep\n");
    assert!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.remote_completion.choices.is_empty())
    );
    h.quiet();
}

#[gpui_kit::test]
async fn pending_cancel_input_and_tab_switch_discard_network_results(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.directory("/stall", cx);
    h.draft("cat rep", 7, cx);
    for action in ["escape", "typing", "tab"] {
        h.draft("cat rep", 7, cx);
        h.query(cx);
        cx.wait_for(h.fixture.window, Duration::from_secs(3), |_, _| {
            h.server.filesystem.active_directory_handles() > 0
        })
        .await;
        cx.update_window(h.fixture.window, |_, window, cx| match action {
            "escape" => window.press("escape", cx),
            "typing" => window.input("x", cx),
            _ => window.click(("session-tab", 1_usize), cx),
        })
        .checked("cancel while actual remote READDIR is delayed");
        h.idle(cx).await;
        cx.wait_for(h.fixture.window, Duration::from_secs(4), |_, _| {
            h.server.active.load(Ordering::Acquire) == 0
        })
        .await;
        cx.update_window(h.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                h.fixture
                    .workspace
                    .read(cx)
                    .remote_completion
                    .choices
                    .is_empty()
            );
            assert!(window.try_find("remote-completion-panel").is_none());
        })
        .checked("late result stays dismissed");
    }
    h.quiet();
}

#[gpui_kit::test]
async fn sftp_base_is_explicit_per_session_and_never_reads_terminal_output(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.draft("cat rep", 7, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("completion-read-base", cx);
    })
    .checked("explicit SFTP base request");
    h.idle(cx).await;
    h.fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(
            view.remote_completion.directory.read(cx).value().as_str(),
            "/"
        )
    });
    h.directory("/docs", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click(("session-tab", 1_usize), cx);
        window.render_frame(cx);
        assert!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .is_empty()
        );
        h.panes[1].terminal.update(cx, |terminal, cx| {
            terminal.emulator.feed(b"pwd\r\n/docs\r\n$ ");
            cx.notify();
        });
        window.render_frame(cx);
        assert!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .is_empty()
        );
        window.click(("session-tab", 0_usize), cx);
        window.render_frame(cx);
        assert_eq!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .as_str(),
            "/docs"
        );
    })
    .checked("independent directory state is not shell-output inference");
    h.quiet();
}

#[gpui_kit::test]
async fn selection_and_ime_block_remote_shortcuts_without_terminal_writes(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    let count = h.server.opened.load(Ordering::Acquire);
    cx.update_window(h.fixture.window, |_, window, cx| {
        let input = h.fixture.workspace.read(cx).command.clone();
        input.update(cx, |input, cx| input.set_selected_range(4..7, cx));
        window.click("remote-complete", cx);
    })
    .checked("selected text is not implicitly replaced");
    cx.run_until_parked();
    assert_eq!(h.server.opened.load(Ordering::Acquire), count);
    cx.update_window(h.fixture.window, |_, window, cx| {
        let input = h.fixture.workspace.read(cx).command.clone();
        window.render_frame(cx);
        let mut handler =
            ElementInputHandler::new(window.find(("input", input.entity_id())).bounds(), input);
        handler.replace_and_mark_text_in_range(Some(4..7), "rep", Some(0..3), window, cx);
        assert!(handler.marked_text_range(window, cx).is_some());
        window.press("ctrl-space", cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.request_remote_completion(false, window, cx)
        });
    })
    .checked("actual marked input blocks shortcut and defensive request path");
    cx.run_until_parked();
    assert_eq!(h.server.opened.load(Ordering::Acquire), count);
    h.quiet();
}

#[gpui_kit::test]
async fn keyboard_scroll_and_enter_preserve_language_and_narrow_controls(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let session = h.fixture.workspace.read_with(cx, |view, _| {
        view.remote_sessions[&h.panes[0].terminal.entity_id()].clone()
    });
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("seed keyboard candidates");
        for index in 0..12 {
            sftp.write(&format!("/docs/entry-{index:02}"), b"x")
                .await
                .checked("seed keyboard filename");
        }
        sftp.close().await.checked("close seed session");
    });
    h.directory("/docs", cx);
    h.draft("cat entry", 9, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        super::super::bind_keys(cx);
        window.press("ctrl-space", cx);
    })
    .checked("real explicit keyboard query");
    h.results(cx).await;
    for width in [960., 520.] {
        for language in [Language::ZhCn, Language::En] {
            cx.simulate_window_resize(h.fixture.window, size(px(width), px(760.)));
            cx.run_until_parked();
            cx.update_window(h.fixture.window, |_, window, cx| {
                assert_eq!(window.viewport_size(), size(px(width), px(760.)));
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let workspace = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
                for id in [
                    "remote-completion-controls",
                    "remote-complete",
                    "completion-read-base",
                    "completion-directory-field",
                ] {
                    let item = window.find(id).bounds();
                    assert!(
                        item.origin.x >= workspace.origin.x && item.right() <= workspace.right(),
                        "{language:?} {id}: {item:?} outside {workspace:?}"
                    );
                    assert!(item.size.width > px(8.));
                }
                assert_eq!(
                    h.fixture
                        .workspace
                        .read(cx)
                        .remote_completion
                        .directory
                        .read(cx)
                        .value()
                        .as_str(),
                    "/docs"
                );
                assert_eq!(
                    h.fixture
                        .workspace
                        .read(cx)
                        .command
                        .read(cx)
                        .value()
                        .as_str(),
                    "cat entry"
                );
            })
            .checked("bilingual narrow query controls retain full draft and directory");
        }
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        for _ in 0..11 {
            window.press("down", cx);
            window.render_frame(cx);
        }
        let row = window.find(("remote-completion-choice", 11_usize)).bounds();
        let list = window.find("remote-completion-list").bounds();
        assert!(
            row.origin.y >= list.origin.y && row.bottom() <= list.bottom(),
            "last keyboard candidate must be visible"
        );
        window.press("enter", cx);
    })
    .checked("scroll to last item and accept by Enter without running");
    cx.run_until_parked();
    assert_eq!(h.text(cx), "cat '/docs/entry-11'");
    h.quiet();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("run-command", cx);
    })
    .checked("only explicit Run sends accepted draft");
    cx.run_until_parked();
    assert_eq!(writes(&h.panes[0]), b"cat '/docs/entry-11'\r");
    assert!(writes(&h.panes[1]).is_empty());
}

#[gpui_kit::test]
async fn opening_modal_and_closing_target_revoke_tickets_and_keep_drafts(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    let old = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("connection-manager", cx);
        window.render_frame(cx);
        assert!(h.fixture.workspace.read(cx).show_connections);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(old.clone(), window, cx)
        });
        assert!(window.try_find("remote-completion-panel").is_none());
        window.click("close-manager", cx);
        window.render_frame(cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(old.clone(), window, cx)
        });
    })
    .checked("modal invalidation remains sticky after it closes");
    assert_eq!(h.text(cx), "cat rep");
    h.query(cx);
    h.results(cx).await;
    let old = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click(("close-tab", 0_usize), cx);
        window.render_frame(cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(old.clone(), window, cx)
        });
        assert_eq!(h.fixture.workspace.read(cx).tabs.len(), 1);
    })
    .checked("closed target cannot insert into remaining session");
    assert_eq!(h.text(cx), "cat rep");
    h.quiet();
}

#[gpui_kit::test]
async fn failed_read_query_shows_localized_error_and_keeps_explicit_directory(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.directory("/error", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.idle(cx).await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let view = h.fixture.workspace.read(cx);
        assert!(view.remote_completion.choices.is_empty());
        assert_eq!(
            view.remote_completion
                .message
                .as_ref()
                .checked_option("permission diagnostic")
                .render(cx),
            "当前账户无权读取补全目录。"
        );
        assert_eq!(
            view.remote_completion.directory.read(cx).value().as_str(),
            "/error"
        );
        i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        assert_eq!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .message
                .as_ref()
                .checked_option("English diagnostic")
                .render(cx),
            "This account cannot read the completion directory."
        );
    })
    .checked("fixed remote diagnostic changes language without losing input");
    assert_eq!(h.text(cx), "cat rep");
    h.quiet();
}

#[gpui_kit::test]
async fn command_probe_never_receives_draft_and_canonical_alias_inserts_resolved_path(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let session = h.fixture.workspace.read_with(cx, |view, _| {
        view.remote_sessions[&h.panes[0].terminal.entity_id()].clone()
    });
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("seed PATH command");
        sftp.write("/docs/cmd-list", b"fixture executable")
            .await
            .checked("seed executable fixture");
        sftp.close().await.checked("close seed SFTP");
    });
    let text = "printf 'private-draft-sentinel'\ncmd-l --flag";
    let caret = text.find(" --flag").checked_option("command word end");
    h.draft(text, caret, cx);
    h.query(cx);
    h.results(cx).await;
    {
        let probes = h
            .server
            .probes
            .lock()
            .checked("inspect received fixed command");
        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0],
            br#"command printf 'KEELSHELL_COMPLETION_V1\000%s\000' "$PATH""#
        );
        assert!(!String::from_utf8_lossy(&probes[0]).contains("private-draft-sentinel"));
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .checked("accept executable basename");
    cx.run_until_parked();
    assert_eq!(
        h.text(cx),
        "printf 'private-draft-sentinel'\n'cmd-list' --flag"
    );
    h.quiet();
    h.directory("/alias", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("remote-completion-choice", 0_usize), cx);
    })
    .checked("canonical remote parent determines absolute insertion");
    cx.run_until_parked();
    assert_eq!(h.text(cx), "cat '/docs/report 中文'\\''$(safe).txt'");
    h.quiet();
}

#[gpui_kit::test]
async fn completion_review_reserves_terminal_space_and_restores_selected_tool(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("commands", cx);
    })
    .checked("open existing bottom tool before querying");
    h.query(cx);
    h.results(cx).await;
    for (width, height) in [(900., 580.), (1440., 900.)] {
        for language in [Language::ZhCn, Language::En] {
            cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
            cx.run_until_parked();
            cx.update_window(h.fixture.window, |_, window, cx| {
                assert_eq!(window.viewport_size(), size(px(width),px(height)));
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let terminal = window
                    .find(("terminal-pane", h.panes[0].terminal.entity_id()))
                    .bounds();
                let input = window.find("command-input-container").bounds();
                let run = window.find("run-command").bounds();
                let panel = window.find("remote-completion-panel").bounds();
                assert!(
                    terminal.size.height >= px(80.),
                    "{width}x{height} {language:?}: terminal collapsed {terminal:?}"
                );
                assert!(
                    input.size.height >= px(60.) && input.size.width >= px(120.),
                    "usable input {input:?}"
                );
                assert!(
                    run.bottom() <= px(height) && panel.bottom() <= px(height),
                    "{width}x{height} {language:?} actual {:?}: terminal {terminal:?}; input {input:?}; run {run:?}; panel {panel:?}", window.viewport_size()
                );
                assert!(
                    h.fixture.workspace.read(cx).visible_panel
                        == Some(super::super::ToolPanel::Commands)
                );
                assert!(
                    window.try_find("new-snippet").is_none(),
                    "tool body folds without changing its selection"
                );
            })
            .checked("native-size terminal/input/candidate geometry");
        }
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("commands", cx);
        window.render_frame(cx);
        assert!(window.try_find("new-snippet").is_some());
        assert!(window.try_find("remote-completion-panel").is_none());
        assert_eq!(
            h.fixture
                .workspace
                .read(cx)
                .command
                .read(cx)
                .value()
                .as_str(),
            "cat rep"
        );
    })
    .checked("explicit tool selection dismisses candidates and restores original tool draft");
    h.quiet();
}

#[gpui_kit::test]
async fn file_directory_button_uses_only_loaded_live_panel_of_current_session(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("files", cx);
    })
    .checked("open production file panel on same SSH transport");
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, cx| {
        h.fixture
            .workspace
            .read(cx)
            .panels
            .get(&h.panes[0].terminal.entity_id())
            .and_then(|panels| panels.files.as_ref())
            .and_then(|files| files.read(cx).completion_directory())
            == Some("/")
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("completion-use-files", cx);
        assert_eq!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .as_str(),
            "/"
        );
        let files = h.fixture.workspace.read(cx).panels[&h.panes[0].terminal.entity_id()]
            .files
            .clone()
            .checked_option("live FilesPanel");
        files.update(cx, |files, cx| files.suspend(cx));
        assert!(files.read(cx).completion_directory().is_none());
        window.click(("session-tab", 1_usize), cx);
        window.render_frame(cx);
        window.click("completion-use-files", cx);
        assert!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .is_empty()
        );
    })
    .checked("no cross-tab or suspended-file snapshot directory reuse");
    h.quiet();
}

#[gpui_kit::test]
async fn terminal_end_and_same_slot_new_entity_reject_old_candidate(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.directory("/docs", cx);
    h.draft("cat rep", 7, cx);
    h.query(cx);
    h.results(cx).await;
    let old = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.remote_completion.choices[0].clone());
    h.panes[0]
        ._output
        .send(SessionEvent::Exited {
            code: 0,
            success: true,
        })
        .checked("send actual terminal lifecycle event");
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, cx| {
        !h.panes[0].terminal.read(cx).is_open()
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(old.clone(), window, cx)
        });
        assert!(view_choices_empty(&h.fixture, cx));
    })
    .checked("ended terminal revokes existing candidate");
    let replacement = attach_remote_panes(&h.fixture, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("review-reconnected-command", cx);
        h.fixture.workspace.update(cx, |view, cx| {
            view.insert_remote_completion(old.clone(), window, cx)
        });
        assert!(
            h.fixture
                .workspace
                .read(cx)
                .remote_completion
                .directory
                .read(cx)
                .value()
                .is_empty()
        );
    })
    .checked("same tab slot with fresh EntityId cannot inherit completion capability");
    assert_eq!(h.text(cx), "cat rep");
    h.quiet();
    for pane in &replacement {
        assert!(writes(pane).is_empty());
    }
}

fn view_choices_empty(fixture: &Fixture, cx: &gpui_kit::App) -> bool {
    fixture
        .workspace
        .read(cx)
        .remote_completion
        .choices
        .is_empty()
}
