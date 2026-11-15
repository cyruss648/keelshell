#![cfg(test)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Production GPUI key dispatch with real controlled SSH/SFTP and terminal input.
use super::*;
use super::{
    editor_keyboard::{FIND_KEY, SAVE_KEY},
    test_server::{Checked, Server},
};
use crate::terminal::{TerminalCommand, TerminalView};
use ::core::prelude::v1::test;
use gpui_kit::{
    InputEvent as _,
    test::{TestAppContextExt, TestWindowExt},
};

struct KeyboardScene {
    panel: Entity<FilesPanel>,
    terminal: Entity<TerminalView>,
    other: Entity<InputState>,
    show_files: bool,
}

impl Render for KeyboardScene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().h(px(32.)).child(Input::new(&self.other)))
            .child(div().h(px(72.)).child(self.terminal.clone()))
            .when(self.show_files, |el| {
                el.child(div().flex_1().min_h_0().child(self.panel.clone()))
            })
    }
}

struct Harness {
    window: AnyWindowHandle,
    scene: Entity<KeyboardScene>,
    panel: Entity<FilesPanel>,
    server: Server,
    session: SshSession,
    runtime: Arc<tokio::runtime::Runtime>,
    commands: mpsc::Receiver<TerminalCommand>,
    _output: mpsc::SyncSender<keelshell_session::SessionEvent>,
    _group: Arc<fixture_group::FixtureGroup>,
}

impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("keyboard fixture runtime"),
        );
        let group = fixture_group::FixtureGroup::acquire_for_context(cx, &runtime);
        let server = Server::new(&runtime);
        let session = server.connect(&runtime);
        let (output, incoming) = mpsc::sync_channel(64);
        let (outgoing, commands) = mpsc::sync_channel(64);
        let (window, scene) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(keelshell_core::Language::ZhCn, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(1100.), px(580.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| KeyboardScene {
                        panel: cx.new(|cx| {
                            FilesPanel::new(
                                session.clone(),
                                "keyboard fixture SSH".into(),
                                runtime.clone(),
                                window,
                                cx,
                            )
                        }),
                        terminal: cx.new(|cx| {
                            TerminalView::from_transport(
                                "keyboard fixture terminal".into(),
                                14.,
                                1000,
                                incoming,
                                outgoing,
                                Arc::new(AtomicBool::new(false)),
                                cx,
                            )
                        }),
                        other: cx.new(|cx| InputState::new(window, cx)),
                        show_files: true,
                    })
                },
            )
            .checked("mount actual editor and terminal")
        });
        let panel = scene.read_with(cx, |scene, _| scene.panel.clone());
        Self {
            window,
            scene,
            panel,
            server,
            session,
            runtime,
            commands,
            _output: output,
            _group: group,
        }
    }

    fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        run: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window, |_, window, cx| run(window, cx))
            .checked("keyboard fixture window")
    }

    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(12), |_, cx| {
            !self.panel.read(cx).busy
        })
        .await;
    }

    async fn open(&self, cx: &mut TestAppContext, path: &str, original: &[u8], draft: &str) {
        self.idle(cx).await;
        self.runtime.block_on(async {
            let sftp = self.session.sftp().await.checked("seed keyboard file");
            sftp.write(path, original).await.checked("seed original");
            sftp.close().await.checked("close seed");
        });
        self.in_window(cx, |window, cx| {
            self.panel.update(cx, |panel, cx| {
                panel.request_read(
                    RemoteEntry {
                        name: path.trim_start_matches('/').into(),
                        path: path.into(),
                        size: None,
                        is_directory: false,
                        is_symlink: false,
                        permissions: None,
                        modified: None,
                    },
                    window,
                    cx,
                );
            });
        });
        self.idle(cx).await;
        self.in_window(cx, |window, cx| {
            self.panel.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_value(draft, window, cx);
            });
            window.render_frame(cx);
        });
    }

    fn focus_draft(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |window, cx| {
            window.render_frame(cx);
            self.panel
                .read(cx)
                .editor
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            window.render_frame(cx);
        });
    }

    fn press(&self, cx: &mut TestAppContext, key: &str) {
        self.in_window(cx, |window, cx| {
            window.press(key, cx);
            window.render_frame(cx);
            window.render_frame(cx);
        });
    }

    fn review(&self, cx: &TestAppContext) -> (uuid::Uuid, Vec<u8>, Vec<u8>) {
        self.panel.read_with(cx, |panel, _| {
            match &panel.pending.as_ref().expect("save proposal").1 {
                Operation::Save {
                    reviewed, content, ..
                } => (
                    panel.pending_review_id,
                    reviewed.content.clone(),
                    content.clone(),
                ),
                _ => panic!("shortcut must propose a save"),
            }
        })
    }

    fn assert_original(&self, path: &str, original: &[u8]) {
        assert_eq!(self.server.filesystem.atomic_writes_started(), 0);
        self.runtime.block_on(async {
            let sftp = self.session.sftp().await.checked("read untouched target");
            assert_eq!(
                sftp.read(path, 1024 * 1024).await.checked("read original"),
                original
            );
            sftp.close().await.checked("close observation");
        });
    }
}

fn key_down(window: &mut Window, cx: &mut App, key: &'static str, held: bool) {
    window.dispatch_event(
        KeyDownEvent {
            keystroke: Keystroke::parse(key).checked("editor shortcut"),
            is_held: held,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
}

#[gpui_kit::test]
async fn editor_keyboard_save_opens_complete_review_and_repeats_never_confirm(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let path = "/keyboard-save.txt";
    let original = b"\xef\xbb\xbfbase\r\n";
    h.open(cx, path, original, "reviewed\ntext\n").await;
    h.focus_draft(cx);
    h.in_window(cx, |window, cx| {
        // Deliver auto-repeat before a new paint as well as after expansion.
        key_down(window, cx, SAVE_KEY, false);
        let review = h.panel.read(cx).pending_review_id;
        for _ in 0..3 {
            key_down(window, cx, SAVE_KEY, true);
            assert_eq!(h.panel.read(cx).pending_review_id, review);
        }
        window.render_frame(cx);
        assert!(window.find("file-review-expanded").visible());
        assert!(h.panel.read(cx).review_focus.is_focused(window));
        assert!(window.try_find("file-editor-expanded").is_none());
        for key in [SAVE_KEY, SAVE_KEY, FIND_KEY, "enter"] {
            window.press(key, cx);
            window.render_frame(cx);
            assert_eq!(h.panel.read(cx).pending_review_id, review);
            assert!(h.panel.read(cx).confirmation_expanded);
            assert!(h.panel.read(cx).operation_id.is_none());
        }
        assert_eq!(window.find("confirm-file-operation").label(), Some("确认"));
    });
    let (_, before, after) = h.review(cx);
    assert_eq!(before, original);
    assert_eq!(after, b"\xef\xbb\xbfreviewed\r\ntext\r\n");
    h.assert_original(path, original);
    h.in_window(cx, |window, cx| {
        window.click("cancel-file-operation", cx);
        assert!(h.panel.read(cx).pending.is_none());
        assert_eq!(h.panel.read(cx).editor.read(cx).value(), "reviewed\ntext\n");
    });
    h.assert_original(path, original);
}

#[gpui_kit::test]
async fn editor_keyboard_find_reveals_and_focuses_the_single_literal_search(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-find.txt", b"needle needle", "needle needle")
        .await;
    h.focus_draft(cx);
    h.press(cx, FIND_KEY);
    h.in_window(cx, |window, cx| {
        let panel = h.panel.read(cx);
        assert_eq!(panel.editor_surface, Some(review::EditorSurface::Draft));
        assert!(window.find("file-editor-expanded").visible());
        let search = window.find("editor-find-input");
        let editor = window.find("file-editor-expanded").bounds();
        assert!(
            search.visible()
                && search.bounds().top() >= editor.top()
                && search.bounds().bottom() <= editor.bottom()
        );
        assert!(
            panel
                .editor_find
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("needle", cx);
        window.press(FIND_KEY, cx);
        window.render_frame(cx);
        assert_eq!(h.panel.read(cx).editor_find.read(cx).value(), "needle");
        window.click("find-editor-text", cx);
        assert_eq!(h.panel.read(cx).editor_match, Some(0));
        assert_eq!(h.panel.read(cx).editor.read(cx).value(), "needle needle");
        assert!(h.panel.read(cx).pending.is_none());
    });
    h.assert_original("/keyboard-find.txt", b"needle needle");
}

#[gpui_kit::test]
async fn editor_keyboard_save_from_find_replace_and_toolbar_reviews_only_the_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-subregion.txt", b"base", "draft to review")
        .await;
    h.focus_draft(cx);
    h.press(cx, FIND_KEY);
    for replacement in [false, true] {
        h.in_window(cx, |window, cx| {
            let input = if replacement {
                h.panel.read(cx).editor_replace.clone()
            } else {
                h.panel.read(cx).editor_find.clone()
            };
            input.update(cx, |input, cx| input.set_value("not the draft", window, cx));
            input.read(cx).focus_handle(cx).focus(window, cx);
            window.render_frame(cx);
            window.press(SAVE_KEY, cx);
            window.render_frame(cx);
            assert!(window.find("file-review-expanded").visible());
        });
        let (_, _, content) = h.review(cx);
        assert_eq!(content, b"draft to review");
        h.in_window(cx, |window, cx| {
            window.click("cancel-file-operation", cx);
            window.render_frame(cx);
            window.click("focus-file-draft", cx);
            window.render_frame(cx);
        });
    }
    h.in_window(cx, |window, cx| {
        // Editor Tab inserts indentation; focus an actual search-row button.
        window.click("find-editor-text", cx);
        window.render_frame(cx);
        assert!(
            h.panel
                .read(cx)
                .editor_keyboard_focus
                .contains_focused(window, cx)
        );
        window.press(SAVE_KEY, cx);
        window.render_frame(cx);
        assert!(window.find("file-review-expanded").visible());
    });
    assert_eq!(h.review(cx).2, b"draft to review");
    h.assert_original("/keyboard-subregion.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_does_not_steal_terminal_or_other_input_shortcuts(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-scope.txt", b"base", "unsaved draft")
        .await;
    h.in_window(cx, |window, cx| {
        for input in [
            h.scene.read(cx).other.clone(),
            h.panel.read(cx).path.clone(),
        ] {
            input.read(cx).focus_handle(cx).focus(window, cx);
            window.render_frame(cx);
            window.press(SAVE_KEY, cx);
            window.press(FIND_KEY, cx);
            window.render_frame(cx);
            assert!(input.read(cx).focus_handle(cx).is_focused(window));
            assert!(h.panel.read(cx).editor_surface.is_none());
            assert!(h.panel.read(cx).pending.is_none());
        }
        let terminal = h.scene.read(cx).terminal.clone();
        terminal.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
        // These are literal flow-control/search bytes on all three platforms.
        window.press("ctrl-s", cx);
        window.press("ctrl-f", cx);
        window.render_frame(cx);
        assert!(terminal.read(cx).focus_handle(cx).is_focused(window));
        assert!(h.panel.read(cx).pending.is_none());
        assert!(h.panel.read(cx).editor_surface.is_none());
    });
    let writes: Vec<_> = h
        .commands
        .try_iter()
        .filter_map(|command| match command {
            TerminalCommand::Write(bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(writes, b"\x13\x06");
    h.assert_original("/keyboard-scope.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_rejects_busy_hidden_missing_session_and_pending_frames(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-boundaries.txt", b"base", "preserved draft")
        .await;
    for boundary in 0..5 {
        h.focus_draft(cx);
        h.in_window(cx, |window, cx| {
            h.panel.update(cx, |panel, cx| match boundary {
                0 => panel.busy = true,
                1 => panel.operation_id = Some(uuid::Uuid::new_v4()),
                2 => panel.queue_details_visible = true,
                3 => panel.session = None,
                4 => panel.confirm(
                    Message::new("独立审核", "Separate review"),
                    Operation::Mkdir("/separate-review".into()),
                    cx,
                ),
                _ => unreachable!(),
            });
            let pending = h.panel.read(cx).pending_review_id;
            // Keep the old painted editor listeners to exercise live guards.
            for key in [SAVE_KEY, FIND_KEY] {
                key_down(window, cx, key, false);
                key_down(window, cx, key, true);
            }
            let panel = h.panel.read(cx);
            assert!(panel.editor_surface.is_none());
            assert!(!panel.confirmation_expanded);
            assert_eq!(panel.pending_review_id, pending);
            assert_eq!(panel.editor.read(cx).value(), "preserved draft");
            if boundary == 4 {
                assert!(matches!(
                    &panel.pending,
                    Some((_, Operation::Mkdir(path))) if path == "/separate-review"
                ));
            } else {
                assert!(panel.pending.is_none());
            }
            h.panel.update(cx, |panel, cx| {
                panel.busy = false;
                panel.operation_id = None;
                panel.queue_details_visible = false;
                panel.session = Some(h.session.clone());
                panel.pending = None;
                cx.notify();
            });
        });
    }
    h.assert_original("/keyboard-boundaries.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_old_paint_cannot_follow_a_changed_connection_or_document(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-identity.txt", b"base", "preserved draft")
        .await;
    for boundary in 0..3 {
        h.focus_draft(cx);
        h.in_window(cx, |window, cx| {
            // GPUI redraws on a change from pointer to keyboard modality before
            // dispatching a key. Establish keyboard modality before retaining
            // the old frame whose captured identity this regression exercises.
            key_down(window, cx, "left", false);
            window.render_frame(cx);
            let (session, connection, editing) = h.panel.update(cx, |panel, _| {
                let previous = (
                    panel.session_token,
                    panel.connection_id,
                    panel.editing.clone(),
                );
                match boundary {
                    0 => panel.session_token = uuid::Uuid::new_v4(),
                    1 => panel.connection_id = Some(uuid::Uuid::new_v4()),
                    2 => panel.editing.as_mut().expect("open document").0 = "/other.txt".into(),
                    _ => unreachable!(),
                }
                previous
            });
            key_down(window, cx, SAVE_KEY, false);
            key_down(window, cx, FIND_KEY, false);
            assert!(h.panel.read(cx).pending.is_none(), "boundary {boundary}");
            assert!(h.panel.read(cx).editor_surface.is_none());
            h.panel.update(cx, |panel, cx| {
                panel.session_token = session;
                panel.connection_id = connection;
                panel.editing = editing;
                cx.notify();
            });
        });
    }
    h.assert_original("/keyboard-identity.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_suspension_and_unmounted_editor_preserve_drafts(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-suspended.txt", b"base", "preserved draft")
        .await;
    h.focus_draft(cx);
    h.in_window(cx, |window, cx| {
        h.scene.update(cx, |scene, cx| {
            scene.show_files = false;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("remote-file-editor").is_none());
        for key in [SAVE_KEY, FIND_KEY] {
            window.press(key, cx);
        }
        assert_eq!(h.panel.read(cx).editor.read(cx).value(), "preserved draft");
        assert!(h.panel.read(cx).pending.is_none());
        h.scene.update(cx, |scene, cx| {
            scene.show_files = true;
            cx.notify();
        });
    });
    h.focus_draft(cx);
    h.in_window(cx, |window, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
        for key in [SAVE_KEY, FIND_KEY] {
            key_down(window, cx, key, false);
        }
        window.render_frame(cx);
        assert!(h.panel.read(cx).pending.is_none());
        assert!(h.panel.read(cx).editor_surface.is_none());
        assert_eq!(h.panel.read(cx).editor.read(cx).value(), "preserved draft");
    });
    h.assert_original("/keyboard-suspended.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_patch_surface_does_not_review_an_unapplied_patch(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-patch.txt", b"base", "preserved draft")
        .await;
    h.in_window(cx, |window, cx| {
        window.render_frame(cx);
        window.click("focus-file-patch", cx);
        window.render_frame(cx);
        let patch = h.panel.read(cx).patch.clone();
        patch.update(cx, |input, cx| {
            input.set_value("unapplied patch", window, cx)
        });
        assert!(patch.read(cx).focus_handle(cx).is_focused(window));
        for key in [SAVE_KEY, FIND_KEY] {
            window.press(key, cx);
            window.render_frame(cx);
        }
        assert_eq!(
            h.panel.read(cx).editor_surface,
            Some(review::EditorSurface::Patch)
        );
        assert!(patch.read(cx).focus_handle(cx).is_focused(window));
        assert!(h.panel.read(cx).pending.is_none());
        assert_eq!(h.panel.read(cx).editor.read(cx).value(), "preserved draft");
    });
    h.assert_original("/keyboard-patch.txt", b"base");
}

#[gpui_kit::test]
async fn editor_keyboard_find_and_save_keep_bilingual_controls_across_three_themes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/keyboard-themes.txt", b"base", "preserved draft")
        .await;
    for family in crate::design::registry() {
        for (language, find, save) in [
            (keelshell_core::Language::ZhCn, "查找", "审核并保存"),
            (keelshell_core::Language::En, "Find", "Review save"),
        ] {
            h.in_window(cx, |window, cx| {
                crate::i18n::set_language(language, cx);
                let settings = keelshell_core::Settings {
                    theme_family: keelshell_core::ThemeId::new(family.id).expect("theme"),
                    theme: keelshell_core::Theme::Dark,
                    ..Default::default()
                };
                crate::design::apply_settings(&settings, Some(window), cx);
                h.panel.update(cx, |panel, cx| {
                    panel.editor_surface = None;
                    cx.notify();
                });
            });
            h.focus_draft(cx);
            h.press(cx, FIND_KEY);
            h.in_window(cx, |window, cx| {
                assert_eq!(window.find("focus-file-search").label(), Some(find));
                assert_eq!(window.find("save-remote-file").label(), Some(save));
                assert!(
                    h.panel
                        .read(cx)
                        .editor_find
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
                assert_eq!(crate::design::current_definition(cx).id, family.id);
                window.press(SAVE_KEY, cx);
                window.render_frame(cx);
                assert!(window.find("file-review-expanded").visible());
                window.click("cancel-file-operation", cx);
                window.render_frame(cx);
            });
        }
    }
    h.assert_original("/keyboard-themes.txt", b"base");
}
