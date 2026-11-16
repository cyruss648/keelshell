//! Backup verification through actual reviewed GPUI actions and SFTP packets.
use super::{CheckedOption, Harness, selected};
use crate::{files::Operation, i18n};
use gpui_kit::{
    AppContext, ScrollDelta, TestAppContext, point, px,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{Language, Settings, TextDocument, Theme, ThemeId};
use std::{sync::atomic::Ordering, time::Duration};

use super::super::test_server::Checked;

async fn open(h: &Harness, path: &str, raw: &[u8], draft: &str, cx: &mut TestAppContext) {
    h.seed(path, raw);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected(path, false)), window, cx);
        });
    })
    .checked("open a complete editor baseline");
    h.idle(cx).await;
    edit(h, draft, cx);
}

fn edit(h: &Harness, text: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_value(text, window, cx);
        });
    })
    .checked("edit the current draft");
}

fn backup(h: &Harness, name: &str) -> Vec<u8> {
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("inspect the retained backup");
        let prefix = format!(".{name}.keelshell-backup-");
        let entries = sftp.list("/").await.checked("list retained editor backups");
        let entries = entries
            .iter()
            .filter(|entry| entry.name.starts_with(&prefix) && entry.name.ends_with(".txt"))
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let bytes = sftp
            .read_regular(&entries[0].path, keelshell_core::MAX_TEXT_EDIT_BYTES)
            .await
            .checked("read all retained backup bytes");
        sftp.close().await.checked("close backup inspection");
        bytes
    })
}

fn assert_draft_retained(h: &Harness, raw: &[u8], draft: &str, cx: &TestAppContext) {
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), draft);
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("retained baseline")
                .content,
            raw,
        );
        assert_eq!(
            panel.editing.as_ref().checked_option("retained editing").1,
            raw
        );
        assert!(panel.pending.is_none());
        assert!(panel.has_unsaved_draft(cx));
    });
}

#[gpui_kit::test]
async fn editor_backup_failed_readback_preserves_target_draft_and_bilingual_status(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/backup-read-error.txt";
    let raw = b"original remote bytes";
    let draft = "reviewed replacement";
    open(&h, path, raw, draft, cx).await;
    let held = h
        .server
        .filesystem
        .hold_close("/..backup-read-error.txt.keelshell-backup-", true, true)
        .checked("hold the backup's writable CLOSE before its publication");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| held.entered() == 1)
        .await;
    assert_eq!(held.entered(), 1);
    h.server.filesystem.set_invalid_transfer_read(1);
    held.release();
    h.idle(cx).await;
    h.server.filesystem.set_invalid_transfer_read(0);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
    assert_eq!(h.read(path), raw);
    assert_eq!(backup(&h, "backup-read-error.txt"), raw);
    assert_draft_retained(&h, raw, draft, cx);

    let requests = h.server.sftp_requests.load(Ordering::Acquire);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.hover("file-operation-status", cx);
    })
    .checked("hover the actual compact status");
    cx.run_until_parked();
    cx.dispatcher.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &Settings {
                            theme_family: ThemeId::new(family.id).checked("registered theme"),
                            theme,
                            ..Default::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    let expected = h.panel.read(cx).status.render(cx);
                    let status = window.find("file-operation-status");
                    let label = status.label().checked_option("status accessibility label");
                    assert_eq!(label, expected);
                    let detail = window.find("file-operation-status-detail");
                    assert!(detail.visible());
                    assert_eq!(detail.label(), Some(expected.as_str()));
                    assert!(expected.contains(match language {
                        Language::ZhCn => "本次目标替换尚未发起",
                        Language::En => "this target replacement was not started",
                    }));
                    assert!(!expected.contains("publication acknowledged"));
                    assert!(!expected.contains("original remote bytes"));
                    assert!(detail.bounds().right() <= window.bounds().right());
                    assert!(detail.bounds().bottom() <= window.bounds().bottom());
                })
                .checked("retain complete live feedback across themes and languages");
            }
        }
    }
    assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), requests);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
    assert_draft_retained(&h, raw, draft, cx);
}

#[gpui_kit::test]
async fn editor_backup_readback_close_precedes_publication_and_keeps_newer_utf16_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/backup-close.txt";
    let mut raw = vec![0xfe, 0xff];
    for unit in "原始\r\n内容".encode_utf16() {
        raw.extend_from_slice(&unit.to_be_bytes());
    }
    let draft = "已审核\n内容";
    let expected = TextDocument::decode(&raw)
        .checked("decode original UTF-16")
        .format()
        .prepare_save(draft)
        .checked("prepare reviewed UTF-16 bytes");
    open(&h, path, &raw, draft, cx).await;
    let held = h
        .server
        .filesystem
        .hold_close("/.backup-close.txt.keelshell-backup-", true, false)
        .checked("hold the actual backup readback CLOSE");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| held.entered() == 1)
        .await;
    assert_eq!(held.entered(), 1);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
    assert_eq!(h.read(path), raw);
    assert!(h.panel.read_with(cx, |panel, _| panel.busy));
    let newer = "后续修改尚未保存";
    edit(&h, newer, cx);
    held.release();
    h.idle(cx).await;
    assert_eq!(h.server.filesystem.atomic_writes_started(), 2);
    assert_eq!(h.read(path), expected);
    assert_eq!(backup(&h, "backup-close.txt"), raw);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), newer);
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("saved baseline")
                .content,
            expected,
        );
        assert!(panel.has_unsaved_draft(cx));
        let (zh, en) = panel.status.translations();
        assert!(zh.contains("后续编辑尚未保存") && zh.contains("已完整核验"));
        assert!(en.contains("newer edits remain unsaved") && en.contains("Fully verified"));
    });
}

#[gpui_kit::test]
async fn editor_backup_cancelled_readback_does_not_start_target_replacement(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/backup-cancel.txt";
    let raw = b"before cancellation";
    let draft = "keep this unsaved draft";
    open(&h, path, raw, draft, cx).await;
    let held = h
        .server
        .filesystem
        .hold_close("/.backup-cancel.txt.keelshell-backup-", true, false)
        .checked("hold completed backup bytes before readback CLOSE ACK");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| held.entered() == 1)
        .await;
    assert_eq!(held.entered(), 1);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
    h.click(cx, "cancel-active-file-operation");
    held.release();
    h.idle(cx).await;
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
    assert_eq!(h.read(path), raw);
    assert_eq!(backup(&h, "backup-cancel.txt"), raw);
    assert_draft_retained(&h, raw, draft, cx);
    h.panel.read_with(cx, |panel, _| {
        let (zh, en) = panel.status.translations();
        assert!(zh.contains("取消") && en.contains("cancel"));
        assert!(!en.contains("Fully verified original backup"));
    });
}

#[gpui_kit::test]
async fn editor_backup_verified_but_target_readback_failed_keeps_publication_unknown_message(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/backup-target-readback.txt";
    let raw = b"original baseline";
    let draft = "reviewed new text";
    open(&h, path, raw, draft, cx).await;
    h.server
        .filesystem
        .replace_after_atomic_rename(path, b"independent competing writer")
        .checked("replace only the published target before its acknowledgement");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.server.filesystem.atomic_writes_started(), 2);
    assert_eq!(h.read(path), b"independent competing writer");
    assert_eq!(backup(&h, "backup-target-readback.txt"), raw);
    assert_draft_retained(&h, raw, draft, cx);
    h.panel.read_with(cx, |panel, _| {
        let (zh, en) = panel.status.translations();
        assert!(zh.contains("已收到原子发布确认") && en.contains("publication acknowledged"));
        assert!(!zh.contains("本次目标替换尚未发起"));
        assert!(!en.contains("this target replacement was not started"));
    });
}

#[gpui_kit::test]
async fn editor_status_detail_long_message_accepts_hover_and_wheel(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let body = (0..80)
        .map(|index| format!("Diagnostic line {index:02}: retained original backup and draft"))
        .collect::<Vec<_>>()
        .join("\n");
    let zh = format!("完整诊断开头\n{body}\n完整诊断末尾");
    let en = format!("Full diagnostic start\n{body}\nFull diagnostic end");
    let requests = h.server.sftp_requests.load(Ordering::Acquire);
    cx.update_window(h.window, |_, window, cx| {
        i18n::set_language(Language::En, cx);
        h.panel.update(cx, |panel, cx| {
            panel.status = i18n::Message::new(zh, en);
            cx.notify();
        });
        window.render_frame(cx);
        let status = window.find("file-operation-status").bounds();
        assert!(status.size.height <= px(crate::design::chrome(cx).status_bar_height));
        assert!(status.bottom() <= window.bounds().bottom());
        window.hover("file-operation-status", cx);
    })
    .checked("show a status whose rendered content exceeds the detail viewport");
    cx.run_until_parked();
    cx.dispatcher.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let initial_y = cx
        .update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            let detail = window.find("file-operation-status-detail").bounds();
            let content = window.find("file-operation-status-detail-content").bounds();
            assert!(content.size.height > detail.size.height + px(400.));
            assert!(content.bottom() > detail.bottom());
            assert!(
                !window
                    .find("file-operation-status")
                    .bounds()
                    .contains(&detail.center()),
                "moving into the popup must leave the status trigger",
            );
            window.hover("file-operation-status-detail", cx);
            content.origin.y
        })
        .checked("move the actual pointer from the status line into its popup");
    cx.run_until_parked();
    cx.dispatcher.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("file-operation-status-detail").visible());
        window.scroll(
            "file-operation-status-detail",
            ScrollDelta::Pixels(point(px(0.), px(-200.))),
            cx,
        );
        let moved = window.find("file-operation-status-detail-content").bounds();
        assert!(moved.origin.y < initial_y - px(100.));
        window.scroll(
            "file-operation-status-detail",
            ScrollDelta::Pixels(point(px(0.), px(-20_000.))),
            cx,
        );
        let detail = window.find("file-operation-status-detail").bounds();
        let end = window.find("file-operation-status-detail-content").bounds();
        assert!(end.bottom() <= detail.bottom() + px(1.));
        assert!(end.bottom() > detail.origin.y);
        assert!(
            window
                .find("file-operation-status-detail")
                .label()
                .checked_option("complete English diagnostic label")
                .ends_with("Full diagnostic end"),
        );
        i18n::set_language(Language::ZhCn, cx);
        window.render_frame(cx);
        assert!(
            window
                .find("file-operation-status-detail")
                .label()
                .checked_option("complete current Chinese diagnostic label")
                .ends_with("完整诊断末尾"),
        );
        window.scroll(
            "file-operation-status-detail",
            ScrollDelta::Pixels(point(px(0.), px(20_000.))),
            cx,
        );
        let detail = window.find("file-operation-status-detail").bounds();
        let start = window.find("file-operation-status-detail-content").bounds();
        assert!((start.origin.y - detail.origin.y).abs() <= px(1.));
    })
    .checked("keep the popup open, scroll to its actual tail and return to the start");
    assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), requests);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
