#![cfg(test)]
//! Explicit encodings through real GPUI controls and authenticated loopback SFTP.
use super::*;
use keelshell_core::{Settings, TextEncoding, TextLineEnding, Theme, ThemeId};

fn edit(h: &Harness, text: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value(text, window, cx);
        });
    })
    .checked("edit the Unicode draft");
}

fn external(h: &Harness, path: &str, bytes: &[u8]) {
    h.server
        .filesystem
        .replace_external_file(path, bytes)
        .checked("owned external write");
}

fn choose(h: &Harness, index: usize, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        if h.panel.read(cx).editor_encoding_open {
            window.click("file-read-encoding", cx);
            window.render_frame(cx);
        }
        // A real Kit popover keeps the choices outside the small file viewport.
        window.click("file-read-encoding", cx);
        window.render_frame(cx);
        window.click(("file-read-encoding-choice", index), cx);
        window.render_frame(cx);
    })
    .checked("choose the next read encoding through a real Kit control");
}

fn request(h: &Harness, path: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.request_read(selected(path, false), window, cx)
        });
    })
    .checked("explicitly request a remote read");
}

async fn open_encoded(h: &Harness, path: &str, raw: &[u8], index: usize, cx: &mut TestAppContext) {
    h.seed(path, raw);
    choose(h, index, cx);
    request(h, path, cx);
    h.idle(cx).await;
}

fn reviewed(h: &Harness, cx: &TestAppContext) -> (Vec<u8>, Vec<u8>) {
    h.panel.read_with(cx, |panel, _| {
        match &panel.pending.as_ref().checked_option("exact save review").1 {
            Operation::Save {
                reviewed, content, ..
            } => (reviewed.content.clone(), content.clone()),
            _ => panic!("expected an encoded save review"),
        }
    })
}

fn backups(h: &Harness, path: &str) -> Vec<Vec<u8>> {
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect encoded backups");
        let prefix = format!(".{}.keelshell-backup-", path.trim_start_matches('/'));
        let mut result = Vec::new();
        for entry in sftp.list("/").await.checked("list encoded backups") {
            if entry.name.starts_with(&prefix) && !entry.name.contains(".tmp") {
                result.push(
                    sftp.read_regular(&entry.path, 1024 * 1024)
                        .await
                        .checked("read raw backup"),
                );
            }
        }
        sftp.close().await.checked("close backup inspection");
        result.sort();
        result
    })
}

#[gpui_kit::test]
async fn legacy_editor_default_rejects_non_utf8_and_explicit_read_enables_lossless_save(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/explicit-gbk.txt";
    let raw = b"\xd6\xd0\xce\xc4\r\nbase";
    h.seed(path, raw);
    request(&h, path, cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.editing.is_none() && panel.editor_format.is_none());
        assert!(panel.pending.is_none());
        assert!(panel.status.render(cx).contains("UTF-8"));
    });
    choose(&h, 1, cx);
    request(&h, path, cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "中文\r\nbase");
        assert_eq!(
            panel.editor_format.checked_option("GBK format").encoding(),
            TextEncoding::Gbk
        );
        assert!(!panel.has_unsaved_draft(cx));
    });
    edit(&h, "中文\nchanged", cx);
    h.click(cx, "save-remote-file");
    assert_eq!(
        reviewed(&h, cx),
        (raw.to_vec(), b"\xd6\xd0\xce\xc4\r\nchanged".to_vec())
    );
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read(path), b"\xd6\xd0\xce\xc4\r\nchanged");
    assert_eq!(backups(&h, path), vec![raw.to_vec()]);
    h.panel
        .read_with(cx, |panel, cx| assert!(!panel.has_unsaved_draft(cx)));
}

#[gpui_kit::test]
async fn legacy_editor_all_explicit_codecs_keep_exact_review_backup_and_readback(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    // These are independent fixed protocol vectors, not bytes made by the code under test.
    for (index, encoding, body, raw) in [
        (1, TextEncoding::Gbk, "中文", b"\xd6\xd0\xce\xc4".as_slice()),
        (
            2,
            TextEncoding::Gb18030,
            "😀",
            b"\x94\x39\xfc\x36".as_slice(),
        ),
        (
            3,
            TextEncoding::Big5,
            "中文",
            b"\xa4\xa4\xa4\xe5".as_slice(),
        ),
        (
            4,
            TextEncoding::ShiftJis,
            "日本",
            b"\x93\xfa\x96\x7b".as_slice(),
        ),
        (5, TextEncoding::Windows1252, "Café", b"Caf\xe9".as_slice()),
    ] {
        let path = format!("/codec-{index}.txt");
        open_encoded(&h, &path, raw, index, cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), body);
            assert_eq!(
                panel
                    .editor_format
                    .checked_option("explicit format")
                    .encoding(),
                encoding
            );
        });
        edit(&h, &format!("{body}\nnext"), cx);
        let mut expected = raw.to_vec();
        expected.extend_from_slice(b"\nnext");
        h.click(cx, "save-remote-file");
        assert_eq!(reviewed(&h, cx), (raw.to_vec(), expected.clone()));
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(&path), expected);
        assert_eq!(backups(&h, &path), vec![raw.to_vec()]);
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(
                panel
                    .editor_snapshot
                    .as_ref()
                    .checked_option("complete readback")
                    .content,
                expected
            );
            assert_eq!(
                panel
                    .editor_format
                    .checked_option("retained explicit format")
                    .encoding(),
                encoding
            );
            assert!(!panel.has_unsaved_draft(cx));
        });
    }
}

#[gpui_kit::test]
async fn legacy_editor_choice_is_local_and_does_not_reinterpret_or_save_the_open_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/local-choice.txt";
    open_encoded(&h, path, b"\xd6\xd0\xce\xc4\r\n", 1, cx).await;
    edit(&h, "中文\nlocal", cx);
    let requests = h.server.sftp_requests.load(Ordering::Acquire);
    let execs = h.server.exec_requests.load(Ordering::Acquire);
    for index in [0, 2, 3, 4, 5] {
        choose(&h, index, cx);
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), "中文\nlocal");
            assert_eq!(
                panel
                    .editor_format
                    .checked_option("open GBK format")
                    .encoding(),
                TextEncoding::Gbk
            );
            assert_eq!(
                panel
                    .prepare_editor_bytes("中文\nlocal")
                    .checked("preserved save format"),
                b"\xd6\xd0\xce\xc4\r\nlocal"
            );
            assert!(panel.pending.is_none() && panel.operation_id.is_none());
            assert!(panel.has_unsaved_draft(cx));
        });
    }
    assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), requests);
    assert_eq!(h.server.exec_requests.load(Ordering::Acquire), execs);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "save-remote-file");
    assert_eq!(reviewed(&h, cx).1, b"\xd6\xd0\xce\xc4\r\nlocal");
    h.click(cx, "cancel-file-operation");
    assert_eq!(h.read(path), b"\xd6\xd0\xce\xc4\r\n");
}

#[gpui_kit::test]
async fn legacy_editor_dirty_reread_cancel_then_confirm_preserves_and_replaces_exact_format(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/dirty-reread.txt";
    open_encoded(&h, path, b"\xd6\xd0\xce\xc4", 1, cx).await;
    edit(&h, "中文 local", cx);
    choose(&h, 5, cx);
    let before = h.server.sftp_requests.load(Ordering::Acquire);
    request(&h, path, cx);
    h.panel.read_with(cx, |panel, _| {
        assert!(matches!(
            panel.pending,
            Some((_, Operation::ReadEncoded(_, TextEncoding::Windows1252)))
        ));
    });
    assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), before);
    h.click(cx, "cancel-file-operation");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "中文 local");
        assert_eq!(
            panel
                .editor_format
                .checked_option("cancel keeps format")
                .encoding(),
            TextEncoding::Gbk
        );
    });
    external(&h, path, b"Caf\xe9");
    request(&h, path, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "Café");
        assert_eq!(
            panel
                .editor_format
                .checked_option("confirmed new format")
                .encoding(),
            TextEncoding::Windows1252
        );
        assert!(!panel.has_unsaved_draft(cx));
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn legacy_editor_reread_review_rejects_newer_draft_and_changed_document_metadata(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open_encoded(&h, "/review-guard.txt", b"Caf\xe9", 5, cx).await;
    for metadata_change in [false, true] {
        edit(&h, "local", cx);
        request(&h, "/review-guard.txt", cx);
        let before = h.server.sftp_requests.load(Ordering::Acquire);
        if metadata_change {
            cx.update(|cx| {
                h.panel.update(cx, |panel, _| {
                    panel
                        .editor_snapshot
                        .as_mut()
                        .checked_option("existing snapshot")
                        .entry
                        .modified = Some(42);
                })
            });
        } else {
            edit(&h, "newer local", cx);
        }
        h.click(cx, "confirm-file-operation");
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.pending.is_none() && panel.operation_id.is_none());
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                if metadata_change {
                    "local"
                } else {
                    "newer local"
                }
            );
            assert!(panel.has_unsaved_draft(cx));
        });
        assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), before);
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn legacy_editor_unrepresentable_and_best_fit_drafts_fail_before_review_or_write(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for (index, drafts) in [
        (1, vec!["private 😀"]),
        (4, vec!["private ¥", "private ‾"]),
        (5, vec!["private 中文"]),
    ] {
        let path = format!("/lossy-{index}.txt");
        open_encoded(&h, &path, b"base", index, cx).await;
        for draft in drafts {
            edit(&h, draft, cx);
            let requests = h.server.sftp_requests.load(Ordering::Acquire);
            h.click(cx, "save-remote-file");
            h.panel.read_with(cx, |panel, cx| {
                assert!(panel.pending.is_none() && panel.operation_id.is_none());
                assert_eq!(panel.editor.read(cx).value().as_str(), draft);
                assert!(panel.has_unsaved_draft(cx));
                assert!(!panel.status.render(cx).contains("private"));
            });
            assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), requests);
            assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
            assert_eq!(h.read(&path), b"base");
        }
        // An explicit local revert lets the next case open without discarding a dirty draft.
        edit(&h, "base", cx);
    }
}

#[gpui_kit::test]
async fn legacy_editor_empty_and_no_break_saves_keep_encoding_and_crlf_policy(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/legacy-no-break.txt";
    let raw = b"Caf\xe9\r\nbase";
    open_encoded(&h, path, raw, 5, cx).await;
    let mut original = raw.to_vec();
    let mut versions = Vec::new();
    for (draft, expected) in [
        ("", b"".as_slice()),
        ("plain", b"plain".as_slice()),
        ("Café\nlast", b"Caf\xe9\r\nlast".as_slice()),
    ] {
        edit(&h, draft, cx);
        h.click(cx, "save-remote-file");
        assert_eq!(reviewed(&h, cx), (original.clone(), expected.to_vec()));
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(path), expected);
        h.panel.read_with(cx, |panel, cx| {
            let format = panel
                .editor_format
                .checked_option("same document retains format");
            assert_eq!(format.encoding(), TextEncoding::Windows1252);
            assert_eq!(format.line_ending(), TextLineEnding::CrLf);
            assert!(!format.has_bom() && !panel.has_unsaved_draft(cx));
        });
        versions.push(original);
        original = expected.to_vec();
    }
    versions.sort();
    assert_eq!(backups(&h, path), versions);
}

#[gpui_kit::test]
async fn legacy_editor_late_save_keeps_newer_draft_and_the_exact_save_encoding(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/legacy-late-save.txt";
    let raw = b"Caf\xe9\r\nbase";
    open_encoded(&h, path, raw, 5, cx).await;
    edit(&h, "Café\nreviewed", cx);
    h.click(cx, "save-remote-file");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-file-operation", cx);
        wait_for_computed_file_result(h.panel.read(cx));
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("Café\nnewer", window, cx);
        });
    })
    .checked("retain a newer legacy draft before consuming the saved result");
    h.idle(cx).await;
    assert_eq!(h.read(path), b"Caf\xe9\r\nreviewed");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "Café\nnewer");
        assert_eq!(
            panel
                .editor_format
                .checked_option("saved encoding")
                .encoding(),
            TextEncoding::Windows1252
        );
        assert!(panel.has_unsaved_draft(cx));
    });
    h.click(cx, "save-remote-file");
    assert_eq!(
        reviewed(&h, cx),
        (
            b"Caf\xe9\r\nreviewed".to_vec(),
            b"Caf\xe9\r\nnewer".to_vec()
        )
    );
    h.click(cx, "cancel-file-operation");
    assert_eq!(backups(&h, path), vec![raw.to_vec()]);
}

#[gpui_kit::test]
async fn legacy_editor_changed_readback_retains_the_old_baseline_and_raw_backup(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/legacy-readback.txt";
    let raw = b"Caf\xe9\r\nbase";
    open_encoded(&h, path, raw, 5, cx).await;
    edit(&h, "Café\nreviewed", cx);
    h.server
        .filesystem
        .replace_after_atomic_rename(path, b"Caf\xe9\r\nrevieweX")
        .checked("competing final-byte write");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read(path), b"Caf\xe9\r\nrevieweX");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("retained full baseline")
                .content,
            raw
        );
        assert_eq!(panel.editor.read(cx).value().as_str(), "Café\nreviewed");
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none());
        assert!(panel.status.render(cx).contains("读回"));
    });
    assert_eq!(backups(&h, path), vec![raw.to_vec()]);
}

#[gpui_kit::test]
async fn legacy_editor_old_choice_callbacks_cannot_change_a_new_choice_or_pending_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open_encoded(&h, "/legacy-choice-guard.txt", b"Caf\xe9", 5, cx).await;
    let stale = h.panel.read_with(cx, |panel, _| {
        super::super::editor_encoding::ReadBinding::capture(panel)
            .checked_option("live read choice binding")
    });
    choose(&h, 1, cx);
    let before = h.server.sftp_requests.load(Ordering::Acquire);
    cx.update(|cx| {
        h.panel.update(cx, |panel, cx| {
            panel.select_editor_read_encoding(Some(TextEncoding::Big5), &stale, cx);
        })
    });
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.editor_read_encoding, Some(TextEncoding::Gbk))
    });
    edit(&h, "Café local", cx);
    h.click(cx, "save-remote-file");
    let current = h.panel.read_with(cx, |panel, _| {
        super::super::editor_encoding::ReadBinding::capture(panel)
            .checked_option("current choice binding")
    });
    let bytes = reviewed(&h, cx);
    cx.update(|cx| {
        h.panel.update(cx, |panel, cx| {
            panel.select_editor_read_encoding(None, &current, cx);
        })
    });
    assert_eq!(reviewed(&h, cx), bytes);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.editor_read_encoding, Some(TextEncoding::Gbk))
    });
    assert_eq!(h.server.sftp_requests.load(Ordering::Acquire), before);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "cancel-file-operation");
}

#[gpui_kit::test]
async fn legacy_editor_merge_and_diff_use_unicode_and_save_with_the_original_encoding(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/legacy-merge.txt";
    open_encoded(&h, path, b"Caf\xe9\r\nbase\r\n", 5, cx).await;
    edit(&h, "Café\nlocal\n", cx);
    h.click(cx, "toggle-remote-diff");
    h.panel.read_with(cx, |panel, _| {
        let preview = panel
            .diff_preview
            .as_ref()
            .checked_option("Unicode diff preview");
        assert!(preview.contains("Café") && preview.contains("local"));
    });
    external(&h, path, b"Caf\xe9 remote\r\nbase\r\n");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(
            panel
                .merge_review
                .as_ref()
                .checked_option("Unicode merge")
                .plan
                .conflicts()
                .is_empty()
        );
    });
    h.click(cx, "adopt-text-merge");
    h.click(cx, "save-remote-file");
    let expected = b"Caf\xe9 remote\r\nlocal\r\n";
    assert_eq!(reviewed(&h, cx).1, expected);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read(path), expected);
    assert_eq!(
        backups(&h, path),
        vec![b"Caf\xe9 remote\r\nbase\r\n".to_vec()]
    );
}

#[gpui_kit::test]
async fn legacy_editor_incompatible_remote_bytes_require_explicit_reread_even_after_reversion(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let path = "/legacy-reload.txt";
    let raw = b"\xd6\xd0\xce\xc4\nbase";
    open_encoded(&h, path, raw, 1, cx).await;
    edit(&h, "中文\nlocal", cx);
    for changed in [b"\xff".as_slice(), b"\xef\xbb\xbfchanged\nbase".as_slice()] {
        external(&h, path, changed);
        h.click(cx, "read-remote-merge");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.editor_reload_required);
            assert!(panel.merge_review.is_none() && panel.pending.is_none());
            assert_eq!(panel.editor.read(cx).value().as_str(), "中文\nlocal");
        });
        external(&h, path, raw);
        h.click(cx, "save-remote-file");
        h.panel.read_with(cx, |panel, _| {
            assert!(panel.editor_reload_required && panel.pending.is_none());
        });
    }
    request(&h, path, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(!panel.editor_reload_required && !panel.has_unsaved_draft(cx));
        assert_eq!(panel.editor.read(cx).value().as_str(), "中文\nbase");
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn legacy_editor_late_reads_reject_replaced_connection_choice_and_document(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open_encoded(&h, "/late-open.txt", b"Caf\xe9", 5, cx).await;
    h.seed("/late-candidate.txt", b"new");
    for scenario in 0..4 {
        let hold = h
            .server
            .filesystem
            .hold_metadata_path("/late-candidate.txt")
            .checked("hold the new read");
        request(&h, "/late-candidate.txt", cx);
        cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
            h.panel.read(cx).busy && hold.entered() > 0
        })
        .await;
        let replacement = (scenario == 0).then(|| h.server.connect(&h.runtime));
        cx.update(|cx| {
            h.panel.update(cx, |panel, _| {
                match scenario {
                    // Reuse the public session token to exercise actual connection identity.
                    0 => panel.session = replacement.clone(),
                    1 => panel.editor_read_revision = uuid::Uuid::new_v4(),
                    2 => {
                        panel
                            .editor_snapshot
                            .as_mut()
                            .checked_option("old document")
                            .entry
                            .modified = Some(77)
                    }
                    _ => {
                        panel.editing =
                            Some(("/different-document.txt".into(), b"Caf\xe9".to_vec()))
                    }
                }
            })
        });
        hold.release();
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), "Café");
            assert_eq!(
                panel
                    .editor_format
                    .checked_option("old explicit format")
                    .encoding(),
                TextEncoding::Windows1252
            );
            assert_eq!(
                panel
                    .editor_snapshot
                    .as_ref()
                    .checked_option("kept old observation")
                    .content,
                b"Caf\xe9"
            );
            assert!(panel.status.render(cx).contains("已变化"));
        });
        // Restore only the test's intentionally replaced local identity.
        cx.update(|cx| {
            h.panel.update(cx, |panel, _| {
                panel.session = Some(h.session.clone());
                panel.editing = Some(("/late-open.txt".into(), b"Caf\xe9".to_vec()));
            })
        });
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn legacy_editor_encoding_controls_and_reviews_fit_all_themes_languages_and_sizes(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        open_encoded(&h, "/legacy-layout.txt", b"Caf\xe9\r\nbase", 5, cx).await;
        h.click(cx, "refresh-files");
        h.idle(cx).await;
        edit(&h, "Café\nlocal", cx);
        for family in crate::design::registry() {
            for theme in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    choose(&h, 5, cx);
                    cx.update_window(h.window, |_, window, cx| {
                        let settings = Settings {
                            theme_family: ThemeId::new(family.id).checked("registered family"),
                            theme,
                            language,
                            ..Settings::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        i18n::set_language(language, cx);
                        window.render_frame(cx);
                        let pane = window.find("files-layout-scene").bounds();
                        let header = window.find("file-read-encoding").bounds();
                        assert!(header.origin.x >= pane.origin.x && header.right() <= pane.right());
                        window.click("file-read-encoding", cx);
                        window.render_frame(cx);
                        let options = window.find("file-read-encoding-options").bounds();
                        assert!(
                            options.origin.y >= px(0.)
                                && options.bottom() <= window.viewport_size().height,
                            "encoding popover stays within the window: {options:?}"
                        );
                        for index in 0..6usize {
                            let button = window.find(("file-read-encoding-choice", index));
                            assert!(button.visible());
                            assert!(
                                button.bounds().origin.x >= options.origin.x
                                    && button.bounds().right() <= options.right()
                            );
                        }
                        window.click("file-read-encoding", cx);
                        window.render_frame(cx);
                        assert_true_file_row(window);
                    })
                    .checked("compact encoding controls preserve real browsing");
                    h.click(cx, "save-remote-file");
                    assert_eq!(reviewed(&h, cx).1, b"Caf\xe9\r\nlocal");
                    h.panel.read_with(cx, |panel, cx| {
                        assert!(
                            panel
                                .pending
                                .as_ref()
                                .checked_option("localized format review")
                                .0
                                .render(cx)
                                .contains("Windows-1252")
                        );
                    });
                    cx.update_window(h.window, |_, window, cx| {
                        window.render_frame(cx);
                        window.click("expand-file-review", cx);
                        let pane = window.find("files-layout-scene").bounds();
                        let body = window.find("file-confirmation-message").bounds();
                        for id in ["confirm-file-operation", "cancel-file-operation"] {
                            let button = window.find(id);
                            assert!(button.visible());
                            assert!(
                                button.bounds().origin.y >= body.bottom()
                                    && button.bounds().bottom() <= pane.bottom()
                            );
                        }
                    })
                    .checked("complete encoded save review retains fixed actions");
                    h.click(cx, "cancel-file-operation");
                }
            }
        }
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    }
}
