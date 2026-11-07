//! Real file-pane expansion, keyboard targeting and unchanged approval guards.
use super::*;
use gpui_kit::{Focusable, ScrollDelta};
use keelshell_core::{Language, Settings, Theme, ThemeId};

async fn open_file(h: &Harness, path: &str, bytes: &[u8], cx: &mut TestAppContext) {
    h.seed(path, bytes);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected(path, false)), window, cx)
        })
    })
    .checked("load checked file baseline");
    h.idle(cx).await;
}

#[gpui_kit::test]
async fn remote_editor_selects_filename_language_without_changing_draft(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;

    for (path, expected) in [
        ("/config.rs", "rust"),
        ("/script.py", "python"),
        ("/config.jsonc", "json"),
        ("/settings.toml", "toml"),
        ("/deploy.yml", "yaml"),
        ("/deploy.sh", "bash"),
        ("/deploy.fish", "text"),
        ("/notes.txt", "text"),
    ] {
        let content = format!("content for {path}\n");
        open_file(&h, path, content.as_bytes(), cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).language_name().as_str(), expected);
            assert_eq!(panel.editor.read(cx).value().as_str(), content);
        });
    }
}

fn review_bytes(h: &Harness, cx: &TestAppContext) -> (String, Vec<u8>, Vec<u8>) {
    h.panel.read_with(cx, |panel, _| {
        match &panel
            .pending
            .as_ref()
            .checked_option("pending reviewed file")
            .1
        {
            Operation::Save {
                path,
                reviewed,
                content,
            } => (path.clone(), reviewed.content.clone(), content.clone()),
            _ => panic!("expected exact reviewed save"),
        }
    })
}

fn apply_editor_view_theme(
    family: &'static crate::design::ThemeDefinition,
    appearance: Theme,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) {
    let settings = Settings {
        theme_family: ThemeId::new(family.id).checked("registered editor viewport theme"),
        theme: appearance,
        ..Settings::default()
    };
    crate::design::apply_settings(&settings, Some(window), cx);
}

fn dispatch_review_pointer(
    window: &mut gpui_kit::Window,
    position: gpui_kit::Point<gpui_kit::Pixels>,
    cx: &mut gpui_kit::App,
) {
    for input in [
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseDownEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseUpEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(input, cx);
    }
}

async fn old_painted_save_action_keeps_replacement_read_review(
    action: &'static str,
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open_file(&h, "/z-editing.txt", b"original remote bytes\n", cx).await;
    h.seed("/a-replacement.txt", b"replacement remote bytes\n");
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("unsaved original draft\n", window, cx)
        });
    })
    .checked("prepare the dirty editor for the original save review");
    h.click(cx, "save-remote-file");
    assert_eq!(
        review_bytes(&h, cx),
        (
            "/z-editing.txt".into(),
            b"original remote bytes\n".to_vec(),
            b"unsaved original draft\n".to_vec(),
        )
    );

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let old_action = window.find(action);
        assert!(old_action.visible(), "the original save action is painted");
        let old_position = old_action.bounds().center();
        let replacement_index = h.panel.read_with(cx, |panel, _| {
            crate::files::browser::remote_indices(
                &panel.entries,
                panel.remote_show_hidden,
                panel.remote_sort,
            )
            .iter()
            .position(|index| panel.entries[*index].path == "/a-replacement.txt")
            .checked_option("visible replacement file row")
        });
        let edit = window.find(("open-remote", replacement_index));
        assert!(edit.visible(), "the replacement Edit action is painted");
        let edit_position = edit.bounds().center();

        // The production Edit callback replaces Save with a dirty-editor Read
        // review. Both pointer pairs use this same frame: repainting after Edit
        // would bind a fresh button and hide the stale authorization defect.
        dispatch_review_pointer(window, edit_position, cx);
        h.panel.read_with(cx, |panel, cx| {
            assert!(matches!(
                &panel.pending,
                Some((_, Operation::Read(entry))) if entry.path == "/a-replacement.txt"
            ));
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "unsaved original draft\n"
            );
            assert!(!panel.busy && panel.operation_id.is_none());
        });
        dispatch_review_pointer(window, old_position, cx);
        h.panel.read_with(cx, |panel, cx| {
            assert!(
                matches!(
                    &panel.pending,
                    Some((_, Operation::Read(entry))) if entry.path == "/a-replacement.txt"
                ),
                "an old save-review action cannot approve or cancel the replacement read review"
            );
            assert_eq!(
                panel
                    .editing
                    .as_ref()
                    .checked_option("original file remains open")
                    .0,
                "/z-editing.txt"
            );
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "unsaved original draft\n"
            );
            assert!(!panel.busy && panel.operation_id.is_none());
        });
    })
    .checked("dispatch the original save action after real Edit replaces its review");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(matches!(
            &panel.pending,
            Some((_, Operation::Read(entry))) if entry.path == "/a-replacement.txt"
        ));
        assert_eq!(
            panel.editor.read(cx).value().as_str(),
            "unsaved original draft\n"
        );
        assert!(panel.operation_id.is_none() && panel.transfer_jobs.is_empty());
    });
    assert_eq!(h.read("/z-editing.txt"), b"original remote bytes\n");
    assert_eq!(h.read("/a-replacement.txt"), b"replacement remote bytes\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);

    // A newly painted action still owns the current review and can dismiss it.
    h.click(cx, "cancel-file-operation");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert_eq!(
            panel.editor.read(cx).value().as_str(),
            "unsaved original draft\n"
        );
    });
}

#[gpui_kit::test]
async fn old_painted_save_confirmation_cannot_approve_replacement_read_review(
    cx: &mut TestAppContext,
) {
    old_painted_save_action_keeps_replacement_read_review("confirm-file-operation", cx).await;
}

#[gpui_kit::test]
async fn old_painted_save_cancellation_cannot_cancel_replacement_read_review(
    cx: &mut TestAppContext,
) {
    old_painted_save_action_keeps_replacement_read_review("cancel-file-operation", cx).await;
}

#[gpui_kit::test]
async fn expanded_save_review_retains_complete_rows_and_keyboard_scroll_in_minimum_pane(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        let path = format!("/审核-$(touch unchanged);{}.txt", "x".repeat(512));
        open_file(&h, &path, b"alpha\nbase-middle\nbase-tail\n", cx).await;
        cx.update_window(h.window, |_, window, cx| {
            h.panel.read(cx).editor.clone().update(cx, |input, cx| {
                input.set_value("alpha\ndraft-middle\nremote-tail\n", window, cx)
            })
        })
        .checked("set explicit reviewed draft");
        for family in crate::design::registry() {
            for appearance in [Theme::System, Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(h.window, |_, window, cx| {
                        crate::i18n::set_language(language, cx);
                        apply_editor_view_theme(family, appearance, window, cx);
                    })
                    .checked("set registered review family, language and appearance");
                    h.click(cx, "save-remote-file");
                    let before = review_bytes(&h, cx);
                    let original = h.panel.read_with(cx, |panel, cx| {
                        format!(
                            "{} · {}",
                            panel.host,
                            panel.pending.as_ref().checked_option("review").0.render(cx)
                        )
                    });
                    cx.update_window(h.window, |_, window, cx| {
                    window.render_frame(cx);
                    let compact = window.find("file-confirmation-message").bounds();
                    assert!(
                        compact.size.height <= px(72.),
                        "default review keeps browsing space"
                    );
                    window.click("expand-file-review", cx);
                    let body = window.find("file-confirmation-message").bounds();
                    let pane = window.find("files-layout-scene").bounds();
                    assert!(body.size.height > compact.size.height + px(32.));
                    assert!(body.origin.y >= pane.origin.y && body.bottom() <= pane.bottom());
                    for (index, line) in original.split('\n').enumerate() {
                        assert_eq!(
                            window.find(("file-confirmation-line", index)).label(),
                            Some(line)
                        );
                    }
                    window.press("enter", cx);
                    assert!(
                        h.panel.read(cx).pending.is_some(),
                        "body Enter cannot approve"
                    );
                    window.press("end", cx);
                    for text in ["alpha", "draft-middle", "remote-tail"] {
                        let index = original
                            .split('\n')
                            .position(|line| line == text)
                            .checked_option("full result row");
                        let row = window.find(("file-confirmation-line", index));
                        assert!(
                            row.visible()
                                && row.bounds().origin.y >= body.origin.y
                                && row.bounds().bottom() <= body.bottom(),
                            "three complete short result lines fit together: {text}"
                        );
                    }
                    window.press("right", cx);
                    assert!(h.panel.read(cx).confirmation_scroll.offset().x < px(0.));
                    window.scroll(
                        "file-confirmation-message",
                        ScrollDelta::Pixels(point(px(-100_000.), px(0.))),
                        cx,
                    );
                    assert!(h.panel.read(cx).confirmation_scroll.offset().x < px(-100.));
                    for id in [
                        "confirm-file-operation",
                        "cancel-file-operation",
                        "collapse-file-review",
                    ] {
                        let button = window.find(id);
                        assert!(button.visible() && button.bounds().origin.y >= body.bottom());
                        assert!(
                            button.bounds().right() <= pane.right()
                                && button.bounds().bottom() <= pane.bottom()
                        );
                    }
                    println!("FILE_REVIEW_VIEWPORT_JSON {}", serde_json::json!({
                        "width": width, "height": height, "assistant": true,
                        "family": family.id, "language": format!("{language:?}"),
                        "appearance": format!("{appearance:?}"),
                        "compact_height": f32::from(compact.size.height),
                        "expanded_height": f32::from(body.size.height),
                        "offset_x": f32::from(h.panel.read(cx).confirmation_scroll.offset().x),
                        "offset_y": f32::from(h.panel.read(cx).confirmation_scroll.offset().y),
                        "complete_rows": original.split('\n').count(), "result_rows_visible_together": 3
                    }));
                    window.click("collapse-file-review", cx);
                    assert!(window.try_find("file-review-expanded").is_none());
                    window.click("expand-file-review", cx);
                    assert_eq!(
                        h.panel.read(cx).confirmation_scroll.offset(),
                        point(px(0.), px(0.))
                    );
                })
                .checked("actual compact/expanded layout, keys and wheel");
                    assert_eq!(
                        review_bytes(&h, cx),
                        before,
                        "presentation cannot replace reviewed target or bytes"
                    );
                    h.click(cx, "cancel-file-operation");
                    assert_eq!(h.read(&path), b"alpha\nbase-middle\nbase-tail\n");
                    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
                }
            }
        }
    }
}

#[gpui_kit::test]
async fn file_entry_actions_reveal_and_focus_real_keyboard_targets_without_path_changes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    open_file(&h, "/focus.txt", b"base\n", cx).await;
    let directory = h
        .panel
        .read_with(cx, |panel, cx| panel.path.read(cx).value().to_string());
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let entry = window.find("focus-file-draft");
        assert!(
            entry.visible(),
            "draft entry stays outside offscreen tool cards"
        );
        window.click("focus-file-draft", cx);
        let input = window.find("file-editor-input").bounds();
        let pane = window.find("files-layout-scene").bounds();
        assert!(input.size.height >= px(72.) && input.bottom() <= pane.bottom());
        assert!(
            h.panel
                .read(cx)
                .editor
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.within("file-editor-input").input("local\n", cx);
        assert_eq!(h.panel.read(cx).editor.read(cx).value().as_str(), "local\n");
        assert_eq!(h.panel.read(cx).path.read(cx).value().as_str(), directory);
        window.click("switch-file-editor", cx);
        assert!(
            h.panel
                .read(cx)
                .patch
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.within("file-editor-input").input(
            "--- /focus.txt\n+++ /focus.txt\n@@ -1 +1 @@\n-local\n+patched\n",
            cx,
        );
        assert_eq!(
            h.panel.read(cx).patch.read(cx).value().as_str(),
            "--- /focus.txt\n+++ /focus.txt\n@@ -1 +1 @@\n-local\n+patched\n"
        );
        assert_eq!(h.panel.read(cx).path.read(cx).value().as_str(), directory);
        let control = window.find("apply-text-patch");
        assert!(control.visible() && control.bounds().bottom() <= pane.bottom());
        window.click("apply-text-patch", cx);
    })
    .checked("real scoped keyboard input belongs to draft and patch, not directory");
    h.idle(cx).await;
    assert_eq!(h.read("/focus.txt"), b"base\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "patched\n")
    });
    h.click(cx, "save-remote-file");
    assert_eq!(
        review_bytes(&h, cx),
        (
            "/focus.txt".into(),
            b"base\n".to_vec(),
            b"patched\n".to_vec()
        )
    );
    h.click(cx, "expand-file-review");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/focus.txt"), b"patched\n");
}

#[gpui_kit::test]
async fn expanded_review_preserves_changed_draft_and_suspension_guards(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open_file(&h, "/guard.txt", b"original\n", cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value("reviewed\n", window, cx));
    })
    .checked("set reviewed draft");
    h.click(cx, "save-remote-file");
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value("newer draft\n", window, cx));
        window.click("confirm-file-operation", cx);
    })
    .checked("actual confirmation rejects changed editor while expanded");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer draft\n");
    });
    assert_eq!(h.read("/guard.txt"), b"original\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "save-remote-file");
    h.panel.read_with(cx, |panel, _| {
        assert!(
            !panel.confirmation_expanded,
            "new review always starts compact"
        )
    });
    h.click(cx, "expand-file-review");
    h.panel.update(cx, |panel, cx| panel.suspend(cx));
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none() && !panel.confirmation_expanded);
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer draft\n");
    });
    assert_eq!(h.read("/guard.txt"), b"original\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn expanded_mirror_review_keeps_complete_hash_rows_and_cancel_never_mutates(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let source = h.source("audited-source.txt", b"exact local source");
    h.seed("/target-only.txt", b"keep remote target");
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-remote");
    h.idle(cx).await;
    h.click(cx, "review-directory-sync");
    let before = h.panel.read_with(cx, |panel, cx| {
        let pending = panel.pending.as_ref().checked_option("mirror review");
        assert!(matches!(&pending.1, Operation::ApplyDirectorySync(_, _)));
        pending.0.render(cx).to_string()
    });
    assert!(before.contains("SHA-256"));
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let body = window.find("file-confirmation-message").bounds();
        let pane = window.find("files-layout-scene").bounds();
        let original = format!("{} · {before}", h.panel.read(cx).host);
        for (index, line) in original.split('\n').enumerate() {
            assert_eq!(
                window.find(("file-confirmation-line", index)).label(),
                Some(line)
            );
        }
        window.press("end", cx);
        assert!(h.panel.read(cx).confirmation_scroll.offset().y < px(0.));
        let last = window.find(("file-confirmation-line", original.split('\n').count() - 1));
        assert!(last.visible() && last.bounds().bottom() <= body.bottom());
        for id in ["confirm-file-operation", "cancel-file-operation"] {
            let control = window.find(id);
            assert!(control.visible() && control.bounds().origin.y >= body.bottom());
            assert!(
                control.bounds().right() <= pane.right()
                    && control.bounds().bottom() <= pane.bottom()
            );
        }
    })
    .checked("actual expanded mirror rows, hash and fixed actions");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .pending
                .as_ref()
                .checked_option("same mirror proposal")
                .0
                .render(cx)
                .as_str(),
            before
        );
    });
    h.click(cx, "cancel-file-operation");
    assert_eq!(
        std::fs::read(source).checked("cancelled source readback"),
        b"exact local source"
    );
    assert_eq!(h.read("/target-only.txt"), b"keep remote target");
    h.missing("/audited-source.txt");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.panel.read_with(cx, |panel, _| {
        assert!(
            panel.pending.is_none() && panel.operation_id.is_none() && panel.sync_journal.is_none()
        );
    });
}

fn layout_contains(inner: Bounds<gpui_kit::Pixels>, outer: Bounds<gpui_kit::Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.right() <= outer.right()
        && inner.origin.y >= outer.origin.y
        && inner.bottom() <= outer.bottom()
}

fn scroll_tools_to_end(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    let tools = window.find("file-tools-scroll").bounds();
    // The outer gutter avoids delivering this wheel to the editor or a nested
    // complete-version viewport. No test mutates the production scroll offset.
    let position = point(
        tools.origin.x + px(2.),
        tools.origin.y + tools.size.height / 2.,
    );
    window.dispatch_event(
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        gpui_kit::ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(-100_000.))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn scroll_visible_merge_text_to_end(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    let text = window.find("text-merge-complete-version").bounds();
    let tools = window.find("file-tools-scroll").bounds();
    let top = text.origin.y.max(tools.origin.y);
    let bottom = text.bottom().min(tools.bottom());
    assert!(
        bottom > top,
        "wheel targets the actually visible part of the text viewport"
    );
    let position = point(
        text.origin.x + text.size.width / 2.,
        top + (bottom - top) / 2.,
    );
    for event in [
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(-100_000.))),
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
    }
    window.render_frame(cx);
}

fn fixed_merge_actions(window: &mut gpui_kit::Window) -> Bounds<gpui_kit::Pixels> {
    let pane = window.find("files-layout-scene").bounds();
    let body = window.find("file-tools-scroll").bounds();
    let footer = window.find("text-merge-apply-controls").bounds();
    assert!(layout_contains(body, pane) && layout_contains(footer, pane));
    assert!(
        body.bottom() <= footer.origin.y,
        "review body cannot cover its actions"
    );
    for id in ["adopt-text-merge", "close-text-merge"] {
        let action = window.find(id);
        assert!(action.visible() && layout_contains(action.bounds(), footer));
        assert!(
            action.bounds().size.height >= px(20.),
            "{id} must be a complete hit target"
        );
    }
    footer
}

#[gpui_kit::test]
async fn inline_editor_scroll_extent_includes_wrapped_actions_and_diff(cx: &mut TestAppContext) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        open_file(
            &h,
            "/布局-完整编辑器.txt",
            b"\xef\xbb\xbfalpha\r\nbeta\r\n",
            cx,
        )
        .await;
        for family in crate::design::registry() {
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::En, Language::ZhCn] {
                    cx.update_window(h.window, |_, window, cx| {
                        crate::i18n::set_language(language, cx);
                        apply_editor_view_theme(family, appearance, window, cx);
                    })
                    .checked("select layout family, appearance and language");
                    for with_diff in [false, true] {
                        if with_diff {
                            h.click(cx, "toggle-remote-diff");
                        }
                        cx.update_window(h.window, |_, window, cx| {
                            window.render_frame(cx);
                            let editor = window.find("remote-file-editor").bounds();
                            let actions = window.find("remote-file-editor-actions").bounds();
                            assert!(layout_contains(actions, editor),
                                "wrapped footer must contribute to the editor's scroll extent: {actions:?} in {editor:?}");
                            scroll_tools_to_end(window, cx);
                            for id in ["read-remote-merge", "save-remote-file"] {
                                reveal_file_control(window, cx, id);
                                let target = window.find(id);
                                assert!(target.visible());
                                assert!(layout_contains(target.bounds(), window.find("file-tools-scroll").bounds()));
                                assert!(layout_contains(target.bounds(), window.find("files-layout-scene").bounds()));
                            }
                        })
                        .checked("actual footer bounds and platform wheel reachability");
                        if with_diff {
                            h.click(cx, "toggle-remote-diff");
                        }
                    }
                }
            }
        }
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
        assert_eq!(
            h.read("/布局-完整编辑器.txt"),
            b"\xef\xbb\xbfalpha\r\nbeta\r\n"
        );
    }
}

#[gpui_kit::test]
async fn expanded_merge_versions_and_review_actions_fit_minimum_file_pane(cx: &mut TestAppContext) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        for language in [Language::En, Language::ZhCn] {
            let path = format!("/{}-合并.txt", "完整路径".repeat(24));
            let tail = (0..40)
                .map(|n| format!("未更改行 {n}\r\n"))
                .collect::<String>();
            let base = format!("a\r\nb\r\n{tail}");
            let draft = format!("\u{feff}A\r\nb\r\n{tail}");
            let remote = format!("a\r\nB\r\n{tail}");
            let result = format!("\u{feff}A\r\nB\r\n{tail}");
            open_file(&h, &path, base.as_bytes(), cx).await;
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                h.panel
                    .read(cx)
                    .editor
                    .clone()
                    .update(cx, |input, cx| input.set_value(draft.clone(), window, cx));
            })
            .checked("prepare exact CRLF and leading FEFF draft");
            h.server
                .filesystem
                .replace_external_file(&path, remote.as_bytes())
                .checked("independent remote edit in the controlled test peer");
            h.click(cx, "focus-file-draft");
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let pane = window.find("files-layout-scene").bounds();
                let read = window.find("read-remote-merge");
                assert!(read.visible() && layout_contains(read.bounds(), pane));
                assert!(
                    window.find("file-editor-control").bounds().size.height >= px(72.),
                    "the actual editor control container must retain a usable minimum height"
                );
                window.click("read-remote-merge", cx);
            })
            .checked("expanded editor's fully visible merge entry");
            h.idle(cx).await;
            for (id, text) in [
                ("merge-show-base", &base),
                ("merge-show-draft", &draft),
                ("merge-show-remote", &remote),
                ("merge-show-result", &result),
            ] {
                h.click(cx, id);
                cx.update_window(h.window, |_, window, cx| {
                    window.render_frame(cx);
                    assert!(window.find("file-merge-expanded").visible());
                    let footer = fixed_merge_actions(window);
                    for (index, line) in text.split('\n').enumerate() {
                        assert_eq!(
                            window
                                .find(("text-merge-complete-version", index + 1))
                                .label(),
                            Some(line),
                            "complete version rows retain every byte of display text"
                        );
                    }
                    scroll_visible_merge_text_to_end(window, cx);
                    scroll_tools_to_end(window, cx);
                    let last_index = text
                        .split('\n')
                        .enumerate()
                        .filter(|(_, line)| !line.is_empty())
                        .map(|(index, _)| index + 1)
                        .last()
                        .checked_option("last complete nonempty version row");
                    let last = window.find(("text-merge-complete-version", last_index));
                    assert!(
                        last.visible()
                            && layout_contains(
                                last.bounds(),
                                window.find("file-tools-scroll").bounds()
                            ),
                        "last complete version row must be reachable"
                    );
                    assert_eq!(
                        fixed_merge_actions(window),
                        footer,
                        "scrolling a complete version cannot displace adopt/close"
                    );
                })
                .checked("all real version rows, final row and fixed merge actions");
            }
            let writes = h.server.filesystem.atomic_writes_started();
            h.click(cx, "adopt-text-merge");
            assert_eq!(h.server.filesystem.atomic_writes_started(), writes);
            assert_eq!(
                h.read(&path),
                remote.as_bytes(),
                "adoption only changes the draft"
            );
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.find("file-editor-expanded").visible());
                assert!(
                    h.panel
                        .read(cx)
                        .editor
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
                window.click("save-remote-file", cx);
                window.click("expand-file-review", cx);
                let pane = window.find("files-layout-scene").bounds();
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let action = window.find(id);
                    assert!(action.visible() && layout_contains(action.bounds(), pane));
                }
                window.press("end", cx);
            })
            .checked("adoption returns to the visible draft, then complete save review");
            assert_eq!(
                review_bytes(&h, cx),
                (
                    path.clone(),
                    remote.as_bytes().to_vec(),
                    result.as_bytes().to_vec()
                )
            );
            h.click(cx, "cancel-file-operation");
            assert_eq!(h.server.filesystem.atomic_writes_started(), writes);
            assert_eq!(h.read(&path), remote.as_bytes());
            h.click(cx, "save-remote-file");
            h.click(cx, "expand-file-review");
            h.click(cx, "confirm-file-operation");
            h.idle(cx).await;
            assert_eq!(
                h.read(&path),
                result.as_bytes(),
                "only explicit confirmation writes the full result"
            );
        }
    }
}

#[gpui_kit::test]
async fn inline_merge_card_keeps_review_actions_visible_while_body_scrolls(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let long_conflict = "line-".to_owned() + &"x".repeat(2400);
    let base = format!("base\n{long_conflict}\n");
    let draft = format!("draft\n{long_conflict}\n");
    let remote = format!("remote\n{long_conflict}\n");
    open_file(&h, "/inline-merge.txt", base.as_bytes(), cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value(draft.clone(), window, cx));
    })
    .checked("prepare inline merge draft");
    h.server
        .filesystem
        .replace_external_file("/inline-merge.txt", remote.as_bytes())
        .checked("prepare inline merge remote version");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        scroll_tools_to_end(window, cx);
        let pane = window.find("files-layout-scene").bounds();
        let tools = window.find("file-tools-scroll").bounds();
        let card = window.find("text-merge-card").bounds();
        let body = window.find("text-merge-inline-body").bounds();
        let actions = window.find("text-merge-inline-actions").bounds();
        assert!(layout_contains(card, tools));
        assert!(layout_contains(actions, card));
        assert!(layout_contains(actions, pane));
        assert!(
            body.bottom() <= actions.origin.y,
            "inline review body must end before its fixed action area"
        );
        for id in [
            "merge-show-base",
            "merge-show-draft",
            "merge-show-remote",
            "merge-show-result",
            "merge-show-conflict",
            "adopt-text-merge",
            "close-text-merge",
        ] {
            let action = window.find(id);
            assert!(
                action.visible() && layout_contains(action.bounds(), actions),
                "{id} must stay in the inline fixed action area"
            );
        }

        let before_actions = actions;
        let before_body_offset = h
            .panel
            .read(cx)
            .merge_review
            .as_ref()
            .checked_option("inline review")
            .body_scroll
            .offset();
        let position = body.center();
        for event in [
            gpui_kit::MouseMoveEvent {
                position,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(0.), px(-100_000.))),
                ..Default::default()
            }
            .to_platform_input(),
        ] {
            window.dispatch_event(event, cx);
        }
        window.render_frame(cx);
        assert_eq!(
            window.find("text-merge-inline-actions").bounds(),
            before_actions,
            "body scrolling must not move the inline action area"
        );
        assert!(
            h.panel
                .read(cx)
                .merge_review
                .as_ref()
                .checked_option("inline review after scroll")
                .body_scroll
                .offset()
                .y
                <= before_body_offset.y,
            "the inline body owns the wheel when it has scrollable content"
        );
    })
    .checked("inline merge body and fixed review action layout");
    assert_eq!(h.read("/inline-merge.txt"), remote.as_bytes());
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn expanded_merge_focus_keeps_hidden_editor_and_manual_inputs_isolated(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    open_file(&h, "/focus-merge.txt", b"base\n", cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value("local\n", window, cx));
    })
    .checked("prepare conflicting draft");
    h.server
        .filesystem
        .replace_external_file("/focus-merge.txt", b"remote\n")
        .checked("controlled conflict");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    let manual = h.panel.read_with(cx, |panel, _| {
        panel
            .merge_review
            .as_ref()
            .checked_option("real conflict")
            .manual
            .clone()
    });
    cx.update_window(h.window, |_, window, cx| {
        reveal_file_control(window, cx, "merge-manual-draft");
        manual.update(cx, |input, cx| input.focus(window, cx));
    })
    .checked("focus inline manual draft before expanding");
    h.click(cx, "expand-text-merge");
    let original = manual.read_with(cx, |input, _| input.value().to_string());
    let directory = h
        .panel
        .read_with(cx, |panel, cx| panel.path.read(cx).value().to_string());
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        fixed_merge_actions(window);
        assert!(h.panel.read(cx).review_focus.is_focused(window));
        assert!(!manual.read(cx).focus_handle(cx).is_focused(window));
        assert!(
            !h.panel
                .read(cx)
                .editor
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("expansion explicitly retires the previous input focus");
    cx.simulate_input(h.window, "unfocused-input");
    assert_eq!(
        manual.read_with(cx, |input, _| input.value().to_string()),
        original
    );
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "local\n");
        assert_eq!(panel.path.read(cx).value().as_str(), directory);
    });
    h.click(cx, "close-text-merge");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("file-editor-expanded").visible());
        assert!(
            h.panel
                .read(cx)
                .editor
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("close keeps the draft and restores its visible focus");
    assert_eq!(h.read("/focus-merge.txt"), b"remote\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    let manual = h.panel.read_with(cx, |panel, _| {
        panel
            .merge_review
            .as_ref()
            .checked_option("fresh conflict from the expanded editor")
            .manual
            .clone()
    });
    cx.update_window(h.window, |_, window, cx| {
        reveal_file_control(window, cx, "merge-manual-draft");
        manual.update(cx, |input, cx| input.focus(window, cx));
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
    })
    .checked("explicitly focus the visible manual replacement");
    cx.simulate_input(h.window, "resolved\n");
    h.click(cx, "merge-use-manual");
    h.click(cx, "adopt-text-merge");
    let chosen = manual.read_with(cx, |input, _| input.value().to_string());
    cx.simulate_input(h.window, "visible-draft-input");
    assert_eq!(
        manual.read_with(cx, |input, _| input.value().to_string()),
        chosen,
        "the removed manual field cannot consume draft input"
    );
    h.panel.read_with(cx, |panel, cx| {
        assert!(
            panel
                .editor
                .read(cx)
                .value()
                .contains("visible-draft-input")
        );
        assert_eq!(panel.path.read(cx).value().as_str(), directory);
        assert!(panel.pending.is_none() && panel.merge_review.is_none());
    });
    assert_eq!(h.read("/focus-merge.txt"), b"remote\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn expanded_merge_keyboard_scroll_reaches_body_end_behind_fixed_actions(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    open_file(&h, "/keyboard-merge.txt", b"base\n", cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value("draft\n", window, cx));
    })
    .checked("prepare the bounded merge draft");
    h.server
        .filesystem
        .replace_external_file("/keyboard-merge.txt", b"remote\n")
        .checked("prepare the controlled remote conflict");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.click(cx, "expand-text-merge");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let body = window.find("file-tools-scroll").bounds();
        let footer = fixed_merge_actions(window);
        assert!(footer.origin.y >= body.bottom());
        assert!(h.panel.read(cx).review_focus.is_focused(window));
        assert_eq!(
            h.panel.read(cx).tools_scroll.offset(),
            point(px(0.), px(0.)),
            "expanded merge starts at the complete review root"
        );

        window.press("end", cx);
        window.render_frame(cx);
        let maximum = h.panel.read(cx).tools_scroll.max_offset();
        let offset = h.panel.read(cx).tools_scroll.offset();
        assert!(
            maximum.y > px(0.),
            "fixture must have an outer merge scroll range"
        );
        assert_eq!(
            offset.y, -maximum.y,
            "End reaches the bounded outer merge tail"
        );
        let tail = window.find("merge-use-manual");
        assert!(
            tail.visible()
                && layout_contains(tail.bounds(), body)
                && tail.bounds().bottom() <= body.bottom(),
            "the final body action is visible after keyboard End"
        );
        let manual = window.find("merge-manual-draft");
        assert!(
            manual.visible() && layout_contains(manual.bounds(), body),
            "the manual conflict editor is fully visible after keyboard End"
        );
        let manual_input = window.find("merge-manual-input");
        assert!(
            manual_input.visible() && layout_contains(manual_input.bounds(), body),
            "the manual textarea container is fully visible after keyboard End"
        );
        let footer_after = fixed_merge_actions(window);
        assert_eq!(
            footer_after, footer,
            "outer merge scrolling leaves Adopt and Close outside the body"
        );

        window.press("home", cx);
        assert_eq!(
            h.panel.read(cx).tools_scroll.offset().y,
            px(0.),
            "Home returns to the complete review root"
        );
        window.press("pagedown", cx);
        assert!(h.panel.read(cx).tools_scroll.offset().y < px(0.));
        window.press("pageup", cx);
        assert_eq!(
            h.panel.read(cx).tools_scroll.offset().y,
            px(0.),
            "PageUp follows the same bounded review semantics"
        );
    })
    .checked("keyboard keys scroll the focused outer merge body only");
    assert_eq!(h.read("/keyboard-merge.txt"), b"remote\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
