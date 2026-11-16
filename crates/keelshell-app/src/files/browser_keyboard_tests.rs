#![cfg(test)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Real GPUI key routing, native local paths, and controlled SSH/SFTP boundaries.
use super::browser_keyboard::BrowserSide;
use super::editor_keyboard_tests::Harness;
use super::test_server::Checked;
use super::*;
use ::core::prelude::v1::test;
use gpui_kit::{
    InputEvent as _,
    test::{TestAppContextExt, TestWindowExt},
};

fn entry(name: &str, directory: bool) -> RemoteEntry {
    RemoteEntry {
        name: name.into(),
        path: format!("/{name}"),
        size: Some(8),
        is_directory: directory,
        is_symlink: false,
        permissions: Some(0o100644),
        modified: Some(42),
    }
}

fn install_remote(h: &Harness, entries: Vec<RemoteEntry>, cx: &mut TestAppContext) {
    h.in_window(cx, |window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.directory = Some("/".into());
            panel.entries = entries;
            panel.remote_sort = browser::BrowserSort::default();
            panel.remote_show_hidden = false;
            panel.remote_selection.clear();
            panel.selected = None;
            panel.editor_surface = None;
            cx.notify();
        });
        window.render_frame(cx);
        h.panel
            .read(cx)
            .browser_focus(BrowserSide::Remote)
            .clone()
            .focus(window, cx);
        window.render_frame(cx);
    });
}

fn selected(h: &Harness, cx: &TestAppContext) -> Vec<String> {
    h.panel.read_with(cx, |panel, _| {
        panel.remote_selection.paths().cloned().collect()
    })
}

fn cursor(h: &Harness, cx: &TestAppContext) -> Option<String> {
    h.panel
        .read_with(cx, |panel, _| panel.remote_selection.cursor().cloned())
}

fn raw_key(window: &mut Window, cx: &mut App, key: &'static str) {
    window.dispatch_event(
        KeyDownEvent {
            keystroke: Keystroke::parse(key).checked("browser key"),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
}

fn select_all_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    }
}

async fn browse_local(h: &Harness, path: PathBuf, cx: &mut TestAppContext) {
    h.in_window(cx, |window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.browse_local(path, window, cx));
    });
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
    h.in_window(cx, |window, cx| {
        window.render_frame(cx);
        h.panel
            .read(cx)
            .browser_focus(BrowserSide::Local)
            .clone()
            .focus(window, cx);
        window.render_frame(cx);
    });
}

#[gpui_kit::test]
async fn browser_keyboard_display_order_ranges_toggle_and_clear(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(
        &h,
        vec![
            entry("z.txt", false),
            entry(".hidden", false),
            entry("folder", true),
            entry("a.txt", false),
            entry("m.txt", false),
        ],
        cx,
    );
    h.press(cx, "down");
    assert_eq!(cursor(&h, cx).as_deref(), Some("/folder"));
    h.press(cx, "down");
    h.press(cx, "shift-down");
    assert_eq!(selected(&h, cx), vec!["/a.txt", "/m.txt"]);
    h.press(cx, "shift-up");
    assert_eq!(selected(&h, cx), vec!["/a.txt"]);
    h.press(cx, "shift-up");
    assert_eq!(selected(&h, cx), vec!["/a.txt", "/folder"]);
    h.press(cx, "end");
    assert_eq!(selected(&h, cx), vec!["/z.txt"]);
    h.press(cx, "space");
    assert!(selected(&h, cx).is_empty());
    assert_eq!(cursor(&h, cx).as_deref(), Some("/z.txt"));
    h.press(cx, "up");
    assert_eq!(selected(&h, cx), vec!["/m.txt"]);
    h.press(cx, select_all_key());
    assert_eq!(
        selected(&h, cx),
        vec!["/a.txt", "/folder", "/m.txt", "/z.txt"]
    );
    h.press(cx, "escape");
    assert!(selected(&h, cx).is_empty());
    assert!(cursor(&h, cx).is_none());
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browser_keyboard_both_lists_are_reachable_by_tab_and_shift_tab(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(&h, vec![entry("a.txt", false), entry("b.txt", false)], cx);
    let local = tempfile::tempdir().checked("Tab navigation local fixture");
    browse_local(&h, local.path().to_owned(), cx).await;
    for key in ["tab", "shift-tab"] {
        h.in_window(cx, |window, cx| {
            h.scene
                .read(cx)
                .other
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            window.render_frame(cx);
            let mut reached = [false, false];
            for _ in 0..128 {
                window.press(key, cx);
                window.render_frame(cx);
                for (index, side) in [BrowserSide::Local, BrowserSide::Remote]
                    .into_iter()
                    .enumerate()
                {
                    reached[index] |= h.panel.read(cx).browser_focus(side).is_focused(window);
                }
                if reached == [true, true] {
                    break;
                }
            }
            assert_eq!(reached, [true, true], "both file lists must be {key} stops");
        });
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browser_keyboard_selection_budget_keeps_original_and_scrolls_to_cursor(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(
        &h,
        (0..33)
            .map(|i| entry(&format!("{i:02}.txt"), false))
            .collect(),
        cx,
    );
    h.press(cx, "down");
    h.press(cx, select_all_key());
    assert_eq!(selected(&h, cx), vec!["/00.txt"]);
    assert_eq!(cursor(&h, cx).as_deref(), Some("/00.txt"));
    h.press(cx, "shift-end");
    assert_eq!(selected(&h, cx), vec!["/00.txt"]);
    assert_eq!(cursor(&h, cx).as_deref(), Some("/00.txt"));
    h.in_window(cx, |window, _| {
        assert!(
            window
                .find("file-operation-status")
                .label()
                .unwrap()
                .contains("已保留原选区")
        );
    });
    h.press(cx, "end");
    h.in_window(cx, |window, cx| {
        let panel = h.panel.read(cx);
        let scroll = panel.browser_scroll_handle(BrowserSide::Remote);
        assert!(scroll.offset().y < px(0.));
        let row = window.find(("remote-entry", 32_usize)).bounds();
        let viewport = window.find("remote-file-rows").bounds();
        assert_eq!(viewport, scroll.bounds());
        assert!(row.top() >= viewport.top());
        assert!(
            row.bottom() <= viewport.bottom(),
            "cursor row {row:?}; viewport {viewport:?}; scroll bounds {:?}, offset {:?}, max {:?}, item {:?}",
            scroll.bounds(),
            scroll.offset(),
            scroll.max_offset(),
            scroll.bounds_for_item(32),
        );
    });
    install_remote(
        &h,
        (0..32)
            .map(|i| entry(&format!("{i:02}.txt"), false))
            .collect(),
        cx,
    );
    h.press(cx, select_all_key());
    assert_eq!(selected(&h, cx).len(), 32);
}

#[gpui_kit::test]
async fn browser_keyboard_page_moves_and_initial_shift_use_a_stable_anchor(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(
        &h,
        (0..30)
            .map(|i| entry(&format!("{i:02}.txt"), false))
            .collect(),
        cx,
    );
    h.press(cx, "shift-down");
    h.press(cx, "shift-down");
    assert_eq!(selected(&h, cx), vec!["/00.txt", "/01.txt"]);
    h.press(cx, "home");
    let page = h
        .in_window(cx, |_, cx| {
            (h.panel
                .read(cx)
                .browser_scroll_handle(BrowserSide::Remote)
                .bounds()
                .size
                .height
                / px(28.))
            .floor() as usize
        })
        .clamp(1, 29);
    h.press(cx, "shift-pagedown");
    assert_eq!(selected(&h, cx).len(), page + 1);
    assert_eq!(cursor(&h, cx), Some(format!("/{page:02}.txt")));
    h.press(cx, "shift-pageup");
    assert_eq!(selected(&h, cx), vec!["/00.txt"]);
    h.press(cx, "end");
    h.press(cx, "pagedown");
    assert_eq!(selected(&h, cx), vec!["/29.txt"]);
}

#[gpui_kit::test]
async fn browser_keyboard_enter_preserves_dirty_draft_until_discard_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.open(cx, "/browser-original.txt", b"original", "unsaved draft")
        .await;
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("seed other browser file");
        sftp.write("/browser-other.txt", b"other")
            .await
            .checked("write other fixture");
        sftp.close().await.checked("close seed");
    });
    install_remote(&h, vec![entry("browser-other.txt", false)], cx);
    h.press(cx, "down");
    h.press(cx, "enter");
    h.in_window(cx, |window, cx| {
        let panel = h.panel.read(cx);
        assert!(panel.pending.is_some());
        assert!(
            panel
                .editing
                .as_ref()
                .unwrap()
                .0
                .ends_with("browser-original.txt")
        );
        assert_eq!(panel.editor.read(cx).value().as_ref(), "unsaved draft");
        assert!(panel.operation_id.is_none());
        window.press("enter", cx);
        window.render_frame(cx);
        assert!(h.panel.read(cx).pending.is_some());
        window.click("cancel-file-operation", cx);
        window.render_frame(cx);
        assert_eq!(
            h.panel.read(cx).editor.read(cx).value().as_ref(),
            "unsaved draft"
        );
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browser_keyboard_delete_repeats_only_review_and_cancel_keeps_remote_bytes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("seed delete fixture");
        sftp.write("/browser-delete.txt", b"keep complete bytes")
            .await
            .checked("seed");
        sftp.close().await.checked("close seed");
    });
    install_remote(&h, vec![entry("browser-delete.txt", false)], cx);
    h.press(cx, "down");
    h.in_window(cx, |window, cx| {
        raw_key(window, cx, "delete");
        let id = h.panel.read(cx).pending_review_id;
        raw_key(window, cx, "delete");
        raw_key(window, cx, "enter");
        window.render_frame(cx);
        assert_eq!(h.panel.read(cx).pending_review_id, id);
        assert!(h.panel.read(cx).confirmation_expanded);
        assert!(h.panel.read(cx).review_focus.is_focused(window));
        assert!(matches!(
            &h.panel.read(cx).pending.as_ref().unwrap().1,
            Operation::Delete(entry) if entry.path == "/browser-delete.txt"
        ));
        for key in ["delete", "enter", "enter"] {
            window.press(key, cx);
            window.render_frame(cx);
            assert!(h.panel.read(cx).pending.is_some());
            assert!(h.panel.read(cx).operation_id.is_none());
        }
        window.click("cancel-file-operation", cx);
        window.render_frame(cx);
    });
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("read cancellation");
        assert_eq!(
            sftp.read("/browser-delete.txt", 1024)
                .await
                .checked("read full file"),
            b"keep complete bytes"
        );
        sftp.close().await.checked("close readback");
    });
}

#[gpui_kit::test]
async fn browser_keyboard_rename_reveals_prefilled_name_without_proposing_mutation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(&h, vec![entry("中文.txt", false)], cx);
    h.press(cx, "down");
    h.press(cx, "f2");
    h.in_window(cx, |window, cx| {
        let panel = h.panel.read(cx);
        assert_eq!(panel.tool_section, Some(action_view::ToolSection::File));
        assert_eq!(panel.name.read(cx).value().as_ref(), "中文.txt");
        assert!(panel.name.read(cx).focus_handle(cx).is_focused(window));
        assert!(window.find("file-name-draft").visible());
        assert!(panel.pending.is_none());
        assert!(panel.operation_id.is_none());
        window.press("backspace", cx);
        window.render_frame(cx);
        assert_eq!(h.panel.read(cx).directory.as_deref(), Some("/"));
        assert!(h.panel.read(cx).operation_id.is_none());
    });
}

#[gpui_kit::test]
async fn browser_keyboard_rename_and_delete_execute_only_after_separate_confirm(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("seed rename fixture");
        sftp.write("/rename-before.txt", b"original")
            .await
            .checked("write seed");
        sftp.close().await.checked("close seed");
    });
    install_remote(&h, vec![entry("rename-before.txt", false)], cx);
    h.press(cx, "down");
    h.press(cx, "f2");
    h.in_window(cx, |window, cx| {
        window.press(select_all_key(), cx);
        window.input("rename-after.txt", cx);
    });
    // Input subscriptions flush at the end of an App update, as they do
    // between native input events. Observe review in the next update.
    h.press(cx, "enter");
    h.in_window(cx, |window, cx| {
        window.render_frame(cx);
        let panel = h.panel.read(cx);
        assert!(panel.confirmation_expanded);
        assert!(matches!(
            &panel.pending.as_ref().unwrap().1,
            Operation::Rename(source, destination)
                if source == "/rename-before.txt" && destination == "/rename-after.txt"
        ));
        assert!(panel.operation_id.is_none());
        window.press("enter", cx);
        window.render_frame(cx);
        assert!(h.panel.read(cx).pending.is_some());
        assert!(h.panel.read(cx).operation_id.is_none());
    });
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("read before confirmation");
        assert_eq!(
            sftp.read("/rename-before.txt", 1024)
                .await
                .checked("original remains"),
            b"original"
        );
        assert!(
            sftp.list("/")
                .await
                .checked("list pending rename")
                .iter()
                .all(|e| e.name != "rename-after.txt")
        );
        sftp.close().await.checked("close observation");
    });
    h.in_window(cx, |window, cx| window.click("confirm-file-operation", cx));
    h.idle(cx).await;
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("read confirmed rename");
        assert_eq!(
            sftp.read("/rename-after.txt", 1024)
                .await
                .checked("complete renamed bytes"),
            b"original"
        );
        assert!(
            sftp.list("/")
                .await
                .checked("list renamed")
                .iter()
                .all(|e| e.name != "rename-before.txt")
        );
        sftp.close().await.checked("close rename observation");
    });
    install_remote(&h, vec![entry("rename-after.txt", false)], cx);
    h.press(cx, "down");
    h.press(cx, "delete");
    h.in_window(cx, |window, cx| window.click("confirm-file-operation", cx));
    h.idle(cx).await;
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("read confirmed delete");
        assert!(
            sftp.list("/")
                .await
                .checked("list deleted")
                .iter()
                .all(|e| e.name != "rename-after.txt")
        );
        sftp.close().await.checked("close delete observation");
    });
}

#[gpui_kit::test]
async fn browser_keyboard_rename_intent_is_withdrawn_by_selection_or_path_edit(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for edit_path in [false, true] {
        install_remote(&h, vec![entry("a.txt", false), entry("b.txt", false)], cx);
        h.press(cx, "down");
        h.press(cx, "f2");
        h.in_window(cx, |window, cx| {
            if edit_path {
                h.panel
                    .read(cx)
                    .path
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
                window.render_frame(cx);
                window.press(select_all_key(), cx);
                window.input("/different", cx);
            } else {
                window.click(("remote-entry", 1_usize), cx);
            }
        });
        h.in_window(cx, |window, cx| {
            assert!(h.panel.read(cx).browser_rename_intent.is_none());
            h.panel
                .read(cx)
                .name
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            window.render_frame(cx);
        });
        h.press(cx, "enter");
        h.in_window(cx, |window, cx| {
            window.render_frame(cx);
            assert!(h.panel.read(cx).browser_rename_intent.is_none());
            assert!(h.panel.read(cx).pending.is_none());
            assert!(h.panel.read(cx).operation_id.is_none());
        });
    }
}

#[gpui_kit::test]
async fn browser_keyboard_local_open_use_parent_and_non_utf8_stay_native(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let scratch = tempfile::tempdir().checked("local keyboard fixture");
    let folder = scratch.path().join("folder");
    std::fs::create_dir(&folder).checked("create explicit folder");
    let file = scratch.path().join("中文.txt");
    std::fs::write(&file, b"local source").checked("create explicit source");
    browse_local(&h, scratch.path().to_owned(), cx).await;
    h.press(cx, "down");
    h.press(cx, "enter");
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
    h.in_window(cx, |window, cx| {
        assert_eq!(
            h.panel
                .read(cx)
                .local_browser
                .listing
                .as_ref()
                .unwrap()
                .directory,
            folder
        );
        window.render_frame(cx);
    });
    h.press(cx, "backspace");
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
    h.press(cx, "end");
    h.press(cx, "enter");
    h.in_window(cx, |_, cx| {
        let panel = h.panel.read(cx);
        assert_eq!(
            panel.local.read(cx).value().as_ref(),
            file.to_str().unwrap()
        );
        assert!(panel.pending.is_none());
        assert!(panel.operation_id.is_none());
    });
    #[cfg(unix)]
    {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let native = scratch
            .path()
            .join(OsString::from_vec(b"\xff-native.txt".to_vec()));
        // APFS rejects illegal UTF-8 names. Inject the native listing value to
        // test this UI boundary without claiming filesystem creation succeeded.
        h.in_window(cx, |window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel.local_browser.listing.as_mut().unwrap().entries.push(
                    local_catalog::LocalEntry {
                        name: native.file_name().unwrap().to_owned(),
                        path: native.clone(),
                        kind: local_catalog::LocalEntryKind::File,
                        size: Some(13),
                        modified: None,
                        hidden: false,
                    },
                );
                panel.local_browser.selection.clear();
                panel.local_browser.selected = None;
                cx.notify();
            });
            window.render_frame(cx);
            h.panel
                .read(cx)
                .browser_focus(BrowserSide::Local)
                .clone()
                .focus(window, cx);
            window.render_frame(cx);
        });
        h.press(cx, "end");
        assert_eq!(
            h.panel
                .read_with(cx, |p, _| p.local_browser.selected.clone()),
            Some(native)
        );
        h.press(cx, "enter");
        h.in_window(cx, |_, cx| {
            assert_eq!(
                h.panel.read(cx).local.read(cx).value().as_ref(),
                file.to_str().unwrap()
            );
        });
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browser_keyboard_other_inputs_buttons_and_hidden_tables_keep_their_keys(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(&h, vec![entry("a.txt", false), entry("b.txt", false)], cx);
    h.press(cx, "down");
    let before = selected(&h, cx);
    h.in_window(cx, |window, cx| {
        let other = h.scene.read(cx).other.clone();
        other.update(cx, |input, cx| input.set_value("other text", window, cx));
        other.read(cx).focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
    });
    h.press(cx, select_all_key());
    h.press(cx, "backspace");
    assert_eq!(selected(&h, cx), before);
    assert_eq!(
        h.scene
            .read_with(cx, |scene, cx| scene.other.read(cx).value().to_string()),
        ""
    );
    h.in_window(cx, |window, cx| {
        window.click(("select-remote-entry", 1_usize), cx);
        window.render_frame(cx);
        assert!(
            h.panel
                .read(cx)
                .browser_focus(BrowserSide::Remote)
                .is_focused(window)
        );
    });
    h.in_window(cx, |window, cx| {
        h.scene.update(cx, |scene, cx| {
            scene.show_files = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.press("delete", cx);
        window.press("enter", cx);
        window.render_frame(cx);
        assert!(h.panel.read(cx).pending.is_none());
        assert!(h.panel.read(cx).operation_id.is_none());
    });
}

#[gpui_kit::test]
async fn browser_keyboard_old_paint_rejects_changed_identity_listing_and_order(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let mutations: [fn(&mut FilesPanel); 7] = [
        |p| p.session_token = uuid::Uuid::new_v4(),
        |p| p.connection_id = Some(uuid::Uuid::new_v4()),
        |p| p.directory = Some("/changed".into()),
        |p| p.entries[0].permissions = Some(0o100600),
        |p| p.entries[0].path = "/different.txt".into(),
        |p| p.remote_sort.descending = true,
        |p| p.remote_show_hidden = true,
    ];
    for mutate in mutations {
        install_remote(&h, vec![entry("a.txt", false), entry("b.txt", false)], cx);
        h.press(cx, "down");
        h.in_window(cx, |window, cx| {
            // Establish keyboard modality and paint before changing the entity
            // without a notification, so dispatch uses the old listing listener.
            raw_key(window, cx, "left");
            window.render_frame(cx);
            h.panel.update(cx, |panel, _| mutate(panel));
            raw_key(window, cx, "delete");
            assert!(h.panel.read(cx).pending.is_none());
            assert!(h.panel.read(cx).operation_id.is_none());
            assert_eq!(h.panel.read(cx).selected.as_ref().unwrap().path, "/a.txt");
            window.render_frame(cx);
        });
    }
}

#[gpui_kit::test]
async fn browser_keyboard_busy_selection_stays_local_and_review_blocks_new_actions(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(&h, vec![entry("a.txt", false), entry("b.txt", false)], cx);
    h.in_window(cx, |_, cx| h.panel.update(cx, |p, _| p.busy = true));
    h.press(cx, "down");
    h.press(cx, "f2");
    h.press(cx, "delete");
    assert_eq!(selected(&h, cx), vec!["/a.txt"]);
    h.in_window(cx, |window, cx| {
        assert!(h.panel.read(cx).pending.is_none());
        assert!(
            h.panel
                .read(cx)
                .browser_focus(BrowserSide::Remote)
                .is_focused(window)
        );
        h.panel.update(cx, |p, cx| {
            p.busy = false;
            cx.notify();
        });
        window.render_frame(cx);
    });
    h.press(cx, "delete");
    let review = h.panel.read_with(cx, |p, _| p.pending_review_id);
    for key in ["down", select_all_key(), "f2", "delete", "enter"] {
        h.press(cx, key);
        h.in_window(cx, |_, cx| {
            assert_eq!(h.panel.read(cx).pending_review_id, review);
            assert!(h.panel.read(cx).pending.is_some());
            assert!(h.panel.read(cx).operation_id.is_none());
        });
    }
}

#[gpui_kit::test]
async fn browser_keyboard_three_themes_bilingual_roles_and_status_text(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    install_remote(&h, vec![entry("中文.txt", false)], cx);
    for family in crate::design::registry() {
        for (language, list_label, status) in [
            (
                keelshell_core::Language::ZhCn,
                "远程文件列表",
                "已保存并完整读回；原始备份已保留",
            ),
            (
                keelshell_core::Language::En,
                "Remote file list",
                "Saved and fully read back; original backup retained",
            ),
        ] {
            h.in_window(cx, |window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply_settings(
                    &keelshell_core::Settings {
                        theme_family: keelshell_core::ThemeId::new(family.id).unwrap(),
                        theme: keelshell_core::Theme::Dark,
                        ..Default::default()
                    },
                    Some(window),
                    cx,
                );
                h.panel.update(cx, |p, cx| {
                    p.status = Message::new(
                        "已保存并完整读回；原始备份已保留",
                        "Saved and fully read back; original backup retained",
                    );
                    cx.notify();
                });
                window.render_frame(cx);
                assert_eq!(window.find("remote-files").label(), Some(list_label));
                assert_eq!(window.find("file-operation-status").label(), Some(status));
                assert!(
                    window
                        .find("file-location-status")
                        .label()
                        .unwrap()
                        .ends_with('/')
                );
                assert_eq!(crate::design::current_definition(cx).id, family.id);
            });
            h.press(cx, "home");
            assert_eq!(selected(&h, cx), vec!["/中文.txt"]);
        }
    }
}
