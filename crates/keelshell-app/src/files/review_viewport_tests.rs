//! Real file-pane expansion, keyboard targeting and unchanged approval guards.
use super::*;
use gpui_kit::{Focusable, ScrollDelta};
use keelshell_core::{Language, Theme};

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
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                cx.update_window(h.window, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                })
                .checked("set real review language and theme");
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
                        "language": format!("{language:?}"), "theme": format!("{theme:?}"),
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
