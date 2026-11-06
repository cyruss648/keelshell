//! UTF-8 envelopes through real GPUI actions and owned loopback SFTP.
use super::*;
use keelshell_core::{
    MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES, Settings, TextLineEnding, Theme, ThemeId,
};
use keelshell_session::sftp::RegularFileSnapshot;

async fn open_raw(h: &Harness, path: &str, bytes: &[u8], cx: &mut TestAppContext) {
    h.seed(path, bytes);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected(path, false)), window, cx)
        })
    })
    .checked("open complete raw editor observation");
    h.idle(cx).await;
}

fn reviewed_bytes(h: &Harness, cx: &TestAppContext) -> (Vec<u8>, Vec<u8>) {
    h.panel.read_with(cx, |panel, _| {
        match &panel.pending.as_ref().checked_option("save review").1 {
            Operation::Save {
                reviewed, content, ..
            } => (reviewed.content.clone(), content.clone()),
            _ => panic!("expected a save review"),
        }
    })
}

fn remote_snapshot(h: &Harness, path: &str) -> RegularFileSnapshot {
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect editor metadata");
        let snapshot = sftp
            .read_regular_snapshot(path, MAX_TEXT_EDIT_BYTES)
            .await
            .checked("read complete editor snapshot");
        sftp.close().await.checked("close editor inspection");
        snapshot
    })
}

fn assert_original_backup(h: &Harness, path: &str, original: &[u8]) {
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect original backup");
        let prefix = format!(".{}.keelshell-backup-", path.trim_start_matches('/'));
        let entries = sftp.list("/").await.checked("list actual backup");
        let backups = entries
            .iter()
            .filter(|entry| entry.name.starts_with(&prefix) && !entry.name.contains(".tmp"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1, "one original backup per confirmed save");
        assert_eq!(
            sftp.read_regular(&backups[0].path, MAX_TEXT_EDIT_BYTES)
                .await
                .checked("read every original backup byte"),
            original
        );
        sftp.close().await.checked("close backup inspection");
    });
}

fn assert_backup_versions(h: &Harness, path: &str, expected: &[Vec<u8>]) {
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect backup versions");
        let prefix = format!(".{}.keelshell-backup-", path.trim_start_matches('/'));
        let entries = sftp.list("/").await.checked("list backup versions");
        let mut actual = Vec::new();
        for entry in entries
            .iter()
            .filter(|entry| entry.name.starts_with(&prefix) && !entry.name.contains(".tmp"))
        {
            actual.push(
                sftp.read_regular(&entry.path, MAX_TEXT_EDIT_BYTES)
                    .await
                    .checked("read complete backup version"),
            );
        }
        actual.sort();
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(actual, expected);
        sftp.close().await.checked("close backup versions");
    });
}

#[gpui_kit::test]
async fn independent_editor_format_r1_late_save_rebases_new_bom_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for leading in [1, 2] {
        let path = format!("/late-new-bom-{leading}.txt");
        let raw = b"base\n";
        let reviewed = format!("{}reviewed\u{feff}\n", "\u{feff}".repeat(leading));
        let newer = format!("{reviewed}new\n");
        open(&h, &path, raw, &reviewed, cx).await;
        h.click(cx, "save-remote-file");
        assert_eq!(
            reviewed_bytes(&h, cx),
            (raw.to_vec(), reviewed.as_bytes().to_vec())
        );
        cx.update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("confirm-file-operation", cx);
            wait_for_computed_file_result(h.panel.read(cx));
            h.panel
                .read(cx)
                .editor
                .clone()
                .update(cx, |input, cx| input.set_value(newer.clone(), window, cx));
        })
        .checked("keep new FEFF draft before adopting successful save");
        h.idle(cx).await;
        assert_eq!(h.read(&path), reviewed.as_bytes());
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), newer);
            assert!(panel.has_unsaved_draft(cx));
            assert_eq!(
                panel
                    .prepare_editor_bytes(&panel.editor.read(cx).value())
                    .checked("prepare retained leading FEFF once"),
                newer.as_bytes()
            );
        });
        h.click(cx, "save-remote-file");
        assert_eq!(
            reviewed_bytes(&h, cx),
            (reviewed.as_bytes().to_vec(), newer.as_bytes().to_vec())
        );
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(&path), newer.as_bytes());
        h.panel.read_with(cx, |panel, cx| {
            assert!(!panel.has_unsaved_draft(cx));
            assert_eq!(
                panel
                    .prepare_editor_bytes(&panel.editor.read(cx).value())
                    .checked("saved canonical body preserves all intended FEFF"),
                newer.as_bytes()
            );
        });
        assert_backup_versions(&h, &path, &[raw.to_vec(), reviewed.as_bytes().to_vec()]);
    }
}

#[gpui_kit::test]
async fn independent_editor_format_r1_merge_keeps_new_leading_feff(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for leading in [1, 2] {
        let path = format!("/merge-new-feff-{leading}.txt");
        let raw = b"a\nb\n";
        let remote = b"a\nB\n";
        let draft = format!("{}A\nb\n", "\u{feff}".repeat(leading));
        let resolved = format!("{}A\nB\n", "\u{feff}".repeat(leading));
        open(&h, &path, raw, &draft, cx).await;
        let writes_before_merge = h.server.filesystem.atomic_writes_started();
        external(&h, &path, remote);
        h.click(cx, "read-remote-merge");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, _| {
            let review = panel.merge_review.as_ref().checked_option("new FEFF merge");
            assert!(review.plan.conflicts().is_empty());
            assert_eq!(
                review.plan.resolve(&[]).checked("exact resolved merge"),
                resolved
            );
        });
        h.click(cx, "adopt-text-merge");
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(
                panel
                    .prepare_editor_bytes(&panel.editor.read(cx).value())
                    .checked("adopted merge keeps all selected bytes"),
                resolved.as_bytes()
            );
            assert!(panel.has_unsaved_draft(cx));
        });
        assert_eq!(h.read(&path), remote);
        assert_eq!(
            h.server.filesystem.atomic_writes_started(),
            writes_before_merge
        );
        h.click(cx, "save-remote-file");
        assert_eq!(
            reviewed_bytes(&h, cx),
            (remote.to_vec(), resolved.as_bytes().to_vec())
        );
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(&path), resolved.as_bytes());
        assert_original_backup(&h, &path, remote);
    }
}

#[gpui_kit::test]
async fn independent_editor_format_r1_crlf_policy_survives_zero_break_save(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for bom in [false, true] {
        for empty in [false, true] {
            for late in [false, true] {
                let path = format!("/crlf-zero-break-{bom}-{empty}-{late}.txt");
                let prefix = if bom { "\u{feff}" } else { "" };
                let raw = format!("{prefix}a\r\nb").into_bytes();
                let intermediate = if empty { "" } else { "ab" };
                let first_saved = format!("{prefix}{intermediate}").into_bytes();
                let next_draft = "a\nb\nlast";
                let second_saved = format!("{prefix}a\r\nb\r\nlast").into_bytes();
                open(&h, &path, &raw, intermediate, cx).await;
                h.click(cx, "save-remote-file");
                assert_eq!(reviewed_bytes(&h, cx), (raw.clone(), first_saved.clone()));
                if late {
                    cx.update_window(h.window, |_, window, cx| {
                        window.render_frame(cx);
                        window.click("confirm-file-operation", cx);
                        wait_for_computed_file_result(h.panel.read(cx));
                        h.panel
                            .read(cx)
                            .editor
                            .clone()
                            .update(cx, |input, cx| input.set_value(next_draft, window, cx));
                    })
                    .checked("retain CRLF policy with a newer LF draft");
                } else {
                    h.click(cx, "confirm-file-operation");
                }
                h.idle(cx).await;
                assert_eq!(h.read(&path), first_saved);
                h.panel.read_with(cx, |panel, cx| {
                    assert_eq!(
                        panel
                            .editor_format
                            .checked_option("open insertion policy")
                            .line_ending(),
                        TextLineEnding::CrLf
                    );
                    assert_eq!(panel.has_unsaved_draft(cx), late);
                    assert_eq!(
                        panel.editor.read(cx).value().as_str(),
                        if late { next_draft } else { intermediate }
                    );
                });
                if !late {
                    edit(&h, next_draft, cx);
                }
                h.click(cx, "save-remote-file");
                assert_eq!(
                    reviewed_bytes(&h, cx),
                    (first_saved.clone(), second_saved.clone())
                );
                h.click(cx, "confirm-file-operation");
                h.idle(cx).await;
                assert_eq!(h.read(&path), second_saved);
                h.panel.read_with(cx, |panel, cx| {
                    assert_eq!(panel.editor.read(cx).value().as_str(), "a\r\nb\r\nlast");
                    assert!(!panel.has_unsaved_draft(cx));
                });
                assert_backup_versions(&h, &path, &[raw, first_saved]);
            }
        }
    }
}

#[gpui_kit::test]
async fn editor_format_open_separates_one_signature_without_false_dirty(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for (raw, body, bom, ending) in [
        (b"".as_slice(), "", false, TextLineEnding::None),
        (b"\xef\xbb\xbf", "", true, TextLineEnding::None),
        (
            "\u{feff}中文\u{feff}\r\nlast".as_bytes(),
            "中文\u{feff}\r\nlast",
            true,
            TextLineEnding::CrLf,
        ),
        (
            b"a\r\nb\nlast",
            "a\r\nb\nlast",
            false,
            TextLineEnding::Mixed,
        ),
        (b"a\rb\r", "a\rb\r", false, TextLineEnding::Cr),
    ] {
        open_raw(&h, "/format-open.txt", raw, cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), body);
            assert_eq!(panel.editing.as_ref().checked_option("raw baseline").1, raw);
            let format = panel.editor_format.checked_option("observed format");
            assert_eq!(format.has_bom(), bom);
            assert_eq!(format.line_ending(), ending);
            assert!(!panel.has_unsaved_draft(cx));
            assert!(panel.pending.is_none());
            assert!(panel.status.render(cx).contains("UTF-8"));
        });
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn editor_format_actual_enter_cancel_and_save_use_exact_bom_crlf_bytes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let raw = b"\xef\xbb\xbfa\r\nb";
    let expected = b"\xef\xbb\xbfa\r\n\r\nb";
    open_raw(&h, "/enter.txt", raw, cx).await;
    let original = remote_snapshot(&h, "/enter.txt");
    h.click(cx, "focus-file-draft");
    cx.update_window(h.window, |_, window, cx| {
        window.activate_window();
        window.press("right", cx);
        window.press("enter", cx);
        assert_eq!(
            h.panel.read(cx).editor.read(cx).value().as_str(),
            "a\n\r\nb"
        );
    })
    .checked("actual editor Enter inserts LF into loaded CRLF");
    h.click(cx, "save-remote-file");
    assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
    h.panel.read_with(cx, |panel, cx| {
        let review = panel
            .pending
            .as_ref()
            .checked_option("format review")
            .0
            .render(cx);
        assert!(
            review.contains("UTF-8；BOM：有；换行：CRLF")
                || review.contains("UTF-8; BOM: yes; line endings: CRLF")
        );
        assert!(review.contains("CRLF"));
        assert!(review.contains("7 → 9"));
    });
    h.click(cx, "cancel-file-operation");
    let cancelled = remote_snapshot(&h, "/enter.txt");
    assert_eq!(cancelled.content, original.content);
    assert_eq!(cancelled.entry.size, original.entry.size);
    assert_eq!(cancelled.entry.permissions, original.entry.permissions);
    assert_eq!(cancelled.entry.modified, original.entry.modified);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);

    h.click(cx, "save-remote-file");
    assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/enter.txt"), expected);
    assert_original_backup(&h, "/enter.txt", raw);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("full readback")
                .content,
            expected
        );
        assert_eq!(panel.editor.read(cx).value().as_str(), "a\r\n\r\nb");
        assert!(!panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none());
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 2);
}

#[gpui_kit::test]
async fn editor_format_uniform_lf_draft_is_byte_identical_to_crlf_baseline(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(
        &h,
        "/same-crlf.txt",
        b"\xef\xbb\xbfa\r\nb\r\n",
        "a\nb\n",
        cx,
    )
    .await;
    cx.update(|cx| {
        h.panel
            .update(cx, |panel, cx| panel.toggle_diff_preview(cx));
    });
    h.panel.read_with(cx, |panel, cx| {
        assert!(!panel.has_unsaved_draft(cx));
        assert!(
            panel
                .diff_preview
                .as_ref()
                .checked_option("prepared diff")
                .contains("byte-identical")
        );
    });
    assert_eq!(h.read("/same-crlf.txt"), b"\xef\xbb\xbfa\r\nb\r\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn editor_format_mixed_and_standalone_cr_remain_verbatim_through_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for (path, raw, body, expected) in [
        (
            "/mixed.txt",
            b"\xef\xbb\xbfa\r\nb\nlast".as_slice(),
            "a\r\nnew\nb\nlast",
            b"\xef\xbb\xbfa\r\nnew\nb\nlast".as_slice(),
        ),
        (
            "/standalone-cr.txt",
            b"a\rb\r".as_slice(),
            "a\rb\rnew\n",
            b"a\rb\rnew\n".as_slice(),
        ),
    ] {
        open(&h, path, raw, body, cx).await;
        h.click(cx, "save-remote-file");
        assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(path), expected);
        assert_original_backup(&h, path, raw);
        h.panel
            .read_with(cx, |panel, cx| assert!(!panel.has_unsaved_draft(cx)));
    }
}

#[gpui_kit::test]
async fn editor_format_final_bom_expansion_line_and_nul_limits_refuse_before_write(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let raw = b"\xef\xbb\xbfa\r\n";
    open_raw(&h, "/limits.txt", raw, cx).await;
    let expanded = format!("{}\n", "x".repeat(MAX_TEXT_EDIT_BYTES - 4));
    for draft in [
        "x".repeat(MAX_TEXT_EDIT_BYTES - 2),
        expanded,
        "a\n".repeat(MAX_TEXT_EDIT_LINES + 1),
        "private\0text".into(),
    ] {
        edit(&h, &draft, cx);
        h.click(cx, "save-remote-file");
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.pending.is_none());
            assert!(!panel.busy && panel.operation_id.is_none());
            assert!(panel.has_unsaved_draft(cx));
            let status = panel.status.render(cx);
            assert!(status.contains("UTF-8"));
            assert!(
                !status.contains("private"),
                "errors do not echo document data"
            );
            assert_eq!(panel.editor.read(cx).value().as_str(), draft);
        });
        assert_eq!(h.read("/limits.txt"), raw);
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    }
}

#[gpui_kit::test]
async fn editor_format_remote_signature_or_eol_change_requires_explicit_reopen(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let raw = b"a\nb\n";
    for (path, changed) in [
        ("/remote-bom.txt", b"\xef\xbb\xbfa\nb\n".as_slice()),
        ("/remote-eol.txt", b"a\r\nb\r\n".as_slice()),
    ] {
        open(&h, path, raw, "local\nb\n", cx).await;
        external(&h, path, changed);
        h.click(cx, "save-remote-file");
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.pending.is_none() && panel.merge_review.is_none());
            assert!(panel.editor_reload_required);
            assert_eq!(panel.editor.read(cx).value().as_str(), "local\nb\n");
            assert_eq!(panel.editing.as_ref().checked_option("old baseline").1, raw);
            assert!(panel.status.render(cx).contains("BOM"));
            assert!(panel.has_unsaved_draft(cx));
        });
        assert_eq!(h.read(path), changed);
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
        external(&h, path, raw);
        h.click(cx, "save-remote-file");
        h.click(cx, "read-remote-merge");
        h.panel.read_with(cx, |panel, _| {
            assert!(panel.pending.is_none() && panel.merge_review.is_none());
            assert!(!panel.busy && panel.operation_id.is_none());
            assert!(
                panel.editor_reload_required,
                "remote reversion cannot clear the reopen gate"
            );
        });
        external(&h, path, changed);

        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel.request_read(selected(path, false), window, cx)
            });
        })
        .checked("explicit reopen still requests draft discard consent");
        h.panel.read_with(cx, |panel, _| {
            assert!(
                matches!(&panel.pending, Some((_, Operation::Read(entry))) if entry.path == path)
            );
        });
        h.click(cx, "cancel-file-operation");
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), "local\nb\n");
        });
        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel.request_read(selected(path, false), window, cx)
            });
        })
        .checked("new explicit reopen review");
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(
                panel
                    .editor_snapshot
                    .as_ref()
                    .checked_option("new baseline")
                    .content,
                changed
            );
            assert!(!panel.has_unsaved_draft(cx));
            assert!(panel.pending.is_none() && panel.merge_review.is_none());
            assert!(!panel.editor_reload_required);
        });
        edit(&h, "reviewed anew\nb\n", cx);
        h.click(cx, "save-remote-file");
        let (baseline, prepared) = reviewed_bytes(&h, cx);
        assert_eq!(baseline, changed);
        assert_eq!(
            prepared,
            if path == "/remote-bom.txt" {
                b"\xef\xbb\xbfreviewed anew\nb\n".as_slice()
            } else {
                b"reviewed anew\r\nb\r\n".as_slice()
            }
        );
        h.click(cx, "cancel-file-operation");
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    }
}

#[gpui_kit::test]
async fn editor_format_ordinary_merge_retains_exactly_one_original_bom(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let remote = b"\xef\xbb\xbfa\r\nB\r\nc";
    open(
        &h,
        "/bom-merge.txt",
        b"\xef\xbb\xbfa\r\nb\r\nc",
        "A\nb\nc",
        cx,
    )
    .await;
    external(&h, "/bom-merge.txt", remote);
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.merge_review.is_some());
        assert!(panel.pending.is_none());
    });
    h.click(cx, "adopt-text-merge");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "A\r\nB\r\nc");
        assert!(panel.editor_format.checked_option("merge format").has_bom());
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "save-remote-file");
    let expected = b"\xef\xbb\xbfA\r\nB\r\nc";
    assert_eq!(reviewed_bytes(&h, cx), (remote.to_vec(), expected.to_vec()));
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/bom-merge.txt"), expected);
    assert_original_backup(&h, "/bom-merge.txt", remote);
}

#[gpui_kit::test]
async fn editor_format_stale_review_refuses_changed_draft_and_keeps_newer_edits(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let raw = b"\xef\xbb\xbfbase\r\n";
    open(&h, "/stale-format.txt", raw, "reviewed\n", cx).await;
    h.click(cx, "save-remote-file");
    edit(&h, "changed after review\n", cx);
    h.click(cx, "confirm-file-operation");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert_eq!(
            panel.editor.read(cx).value().as_str(),
            "changed after review\n"
        );
    });
    assert_eq!(h.read("/stale-format.txt"), raw);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);

    edit(&h, "reviewed\n", cx);
    h.click(cx, "save-remote-file");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-file-operation", cx);
        wait_for_computed_file_result(h.panel.read(cx));
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("newer unsaved\n", window, cx)
        });
    })
    .checked("queue successful readback before applying a newer local edit");
    h.idle(cx).await;
    let saved = b"\xef\xbb\xbfreviewed\r\n";
    assert_eq!(h.read("/stale-format.txt"), saved);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer unsaved\n");
        assert!(panel.has_unsaved_draft(cx));
        assert_eq!(
            panel.editing.as_ref().checked_option("saved baseline").1,
            saved
        );
        assert!(panel.pending.is_none());
    });
    h.click(cx, "save-remote-file");
    assert_eq!(
        reviewed_bytes(&h, cx),
        (saved.to_vec(), b"\xef\xbb\xbfnewer unsaved\r\n".to_vec())
    );
    h.click(cx, "cancel-file-operation");
    assert_original_backup(&h, "/stale-format.txt", raw);
}

#[gpui_kit::test]
async fn editor_format_changed_last_readback_byte_never_adopts_a_saved_baseline(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let raw = b"\xef\xbb\xbfbase\r\n";
    let expected = b"\xef\xbb\xbfreviewed\r\nTAIL";
    let changed = b"\xef\xbb\xbfreviewed\r\nTAIX";
    open(&h, "/readback-format.txt", raw, "reviewed\nTAIL", cx).await;
    h.server
        .filesystem
        .replace_after_atomic_rename("/readback-format.txt", changed)
        .checked("simulate a same-size competing writer after acknowledged rename");
    h.click(cx, "save-remote-file");
    assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/readback-format.txt"), changed);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("retained original")
                .content,
            raw
        );
        assert_eq!(
            panel.editing.as_ref().checked_option("old raw baseline").1,
            raw
        );
        assert_eq!(panel.editor.read(cx).value().as_str(), "reviewed\nTAIL");
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none() && panel.merge_review.is_none());
        assert!(panel.status.render(cx).contains("读回"));
    });
    assert_original_backup(&h, "/readback-format.txt", raw);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 2);
}

#[gpui_kit::test]
async fn editor_format_full_review_and_fixed_actions_fit_all_families_and_languages(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        let raw = b"\xef\xbb\xbfalpha\r\nbase";
        let expected = b"\xef\xbb\xbfalpha\r\nreviewed";
        open(&h, "/format-layout.txt", raw, "alpha\nreviewed", cx).await;
        for family in ["frost", "graphite", "lavender"] {
            for theme in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(h.window, |_, window, cx| {
                        let settings = Settings {
                            theme_family: ThemeId::new(family).checked("registered theme"),
                            theme,
                            language,
                            ..Settings::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        i18n::set_language(language, cx);
                    })
                    .checked("apply real family appearance and language");
                    h.click(cx, "save-remote-file");
                    assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
                    let review = h.panel.read_with(cx, |panel, cx| {
                        format!(
                            "{} · {}",
                            panel.host,
                            panel
                                .pending
                                .as_ref()
                                .checked_option("full format review")
                                .0
                                .render(cx)
                        )
                    });
                    assert!(review.contains(match language {
                        Language::ZhCn => "UTF-8；BOM：有；换行：CRLF",
                        Language::En => "UTF-8; BOM: yes; line endings: CRLF",
                    }));
                    assert!(review.contains("CRLF"));
                    assert!(review.contains("14 → 18"));
                    cx.update_window(h.window, |_, window, cx| {
                        window.render_frame(cx);
                        let compact = window.find("file-confirmation-message").bounds();
                        assert!(compact.size.height <= px(72.));
                        window.click("expand-file-review", cx);
                        let viewport = window.find("file-confirmation-message").bounds();
                        let pane = window.find("files-layout-scene").bounds();
                        assert!(
                            viewport.origin.y >= pane.origin.y
                                && viewport.bottom() <= pane.bottom()
                        );
                        for (index, line) in review.split('\n').enumerate() {
                            assert_eq!(
                                window.find(("file-confirmation-line", index)).label(),
                                Some(line)
                            );
                        }
                        for id in ["confirm-file-operation", "cancel-file-operation"] {
                            let action = window.find(id);
                            assert!(action.visible());
                            assert!(action.bounds().origin.y >= viewport.bottom());
                            assert!(
                                action.bounds().right() <= pane.right()
                                    && action.bounds().bottom() <= pane.bottom()
                            );
                        }
                        window.press("enter", cx);
                        assert!(
                            h.panel.read(cx).pending.is_some(),
                            "review text Enter cannot send"
                        );
                    })
                    .checked("complete format review and reachable fixed actions");
                    assert_eq!(reviewed_bytes(&h, cx), (raw.to_vec(), expected.to_vec()));
                    h.click(cx, "cancel-file-operation");
                    assert_eq!(h.read("/format-layout.txt"), raw);
                    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
                }
            }
        }
    }
}
