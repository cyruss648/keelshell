//! Real GPUI review actions and exact bytes on owned loopback SSH/SFTP.
use super::*;

async fn open(h: &Harness, path: &str, base: &[u8], draft: &str, cx: &mut TestAppContext) {
    h.seed(path, base);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected(path, false)), window, cx)
        })
    })
    .checked("read checked baseline");
    h.idle(cx).await;
    edit(h, draft, cx);
}
fn edit(h: &Harness, text: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |input, cx| input.set_value(text, window, cx))
    })
    .checked("edit local draft");
}
fn external(h: &Harness, path: &str, text: &[u8]) {
    h.server
        .filesystem
        .replace_external_file(path, text)
        .checked("owned external file change");
}

#[gpui_kit::test]
async fn independent_remote_changes_merge_to_reviewed_draft_and_full_readback(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/merge.txt", b"a\r\nb\r\nc", "A\r\nb\r\nc", cx).await;
    external(&h, "/merge.txt", b"a\r\nB\r\nc");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.merge_review.is_some());
        assert_eq!(panel.editor.read(cx).value().as_str(), "A\r\nb\r\nc");
        assert!(panel.pending.is_none());
    });
    h.click(cx, "adopt-text-merge");
    assert_eq!(h.read("/merge.txt"), b"a\r\nB\r\nc");
    h.click(cx, "save-remote-file");
    h.panel.read_with(cx, |panel, cx| {
        let review = &panel.pending.as_ref().checked_option("full review").0;
        assert!(review.render(cx).contains("A\r\nB\r\nc"));
        assert!(review.render(cx).contains("最终全文"));
    });
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/merge.txt"), b"A\r\nB\r\nc");
    h.panel.read_with(cx, |panel, cx| {
        assert!(!panel.has_unsaved_draft(cx));
        assert_eq!(
            panel
                .editor_snapshot
                .as_ref()
                .checked_option("readback baseline")
                .content,
            b"A\r\nB\r\nc"
        );
    });
}

#[gpui_kit::test]
async fn stale_save_creates_conflict_review_and_every_conflict_requires_choice(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/conflict.txt", b"a\nb\nc\nd\n", "A\nb\nC\nd\n", cx).await;
    external(&h, "/conflict.txt", b"REMOTE_A\nb\nREMOTE_C\nd\n");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .merge_review
                .as_ref()
                .checked_option("conflict review")
                .plan
                .conflicts()
                .len(),
            2
        );
        assert_eq!(panel.editor.read(cx).value().as_str(), "A\nb\nC\nd\n");
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "merge-use-draft");
    h.click(cx, "merge-next-conflict");
    h.click(cx, "adopt-text-merge");
    h.panel
        .read_with(cx, |panel, _| assert!(panel.merge_review.is_some()));
    h.click(cx, "merge-use-remote");
    h.click(cx, "adopt-text-merge");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.editor.read(cx).value().as_str(),
            "A\nb\nREMOTE_C\nd\n"
        )
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "save-remote-file");
    h.click(cx, "cancel-file-operation");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none());
    });
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/conflict.txt"), b"A\nb\nREMOTE_C\nd\n");
}

#[gpui_kit::test]
async fn manual_merge_and_patch_remain_drafts_until_full_review(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/手工.txt", b"base\n", "local\n", cx).await;
    external(&h, "/手工.txt", b"remote\n");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        let manual = h
            .panel
            .read(cx)
            .merge_review
            .as_ref()
            .checked_option("manual review")
            .manual
            .clone();
        manual.update(cx, |input, cx| input.set_value("手工\r\nlast", window, cx));
    })
    .checked("explicit manual replacement");
    h.click(cx, "merge-use-manual");
    h.click(cx, "adopt-text-merge");
    h.click(cx, "toggle-text-patch");
    cx.update_window(h.window,|_,window,cx|h.panel.read(cx).patch.clone().update(cx,|input,cx|input.set_value("--- /手工.txt\n+++ /手工.txt\n@@ -1,2 +1,2 @@\n 手工\r\n-last\n\\ No newline at end of file\n+patched\n\\ No newline at end of file\n",window,cx))).checked("paste exact single-file patch");
    h.click(cx, "apply-text-patch");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), "手工\r\npatched")
    });
    assert_eq!(h.read("/手工.txt"), b"remote\n");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/手工.txt"), "手工\r\npatched".as_bytes());
}

#[gpui_kit::test]
async fn changes_during_merge_read_patch_parse_review_and_suspend_preserve_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/safe.txt", b"base\n", "local\n", cx).await;
    external(&h, "/safe.txt", b"remote\n");
    let hold = h
        .server
        .filesystem
        .hold_metadata_path("/safe.txt")
        .checked("owned exact checked read");
    h.click(cx, "read-remote-merge");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() > 0)
        .await;
    edit(&h, "newer draft\n", cx);
    hold.release();
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.merge_review.is_none());
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer draft\n");
    });
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.click(cx, "merge-use-draft");
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx))
    })
    .checked("revoke captured SSH connection");
    h.click(cx, "adopt-text-merge");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer draft\n");
        assert!(panel.suspended);
    });
    assert_eq!(h.read("/safe.txt"), b"remote\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn changed_remote_version_after_merge_adoption_restarts_review_and_link_fails_closed(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/again.txt", b"a\nb\n", "A\nb\n", cx).await;
    external(&h, "/again.txt", b"a\nB\n");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.click(cx, "adopt-text-merge");
    h.click(cx, "save-remote-file");
    external(&h, "/again.txt", b"third\nB\n");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.merge_review.is_some());
        assert_eq!(panel.editor.read(cx).value().as_str(), "A\nB\n");
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "close-text-merge");
    h.server
        .filesystem
        .insert_symlink("/again.txt")
        .checked("replace leaf type with symlink");
    h.click(cx, "save-remote-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.merge_review.is_none());
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.status.render(cx).contains("regular"));
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn patch_rejection_and_bilingual_full_review_preserve_state(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/patch.txt", b"base\n", "draft\n", cx).await;
    h.click(cx, "toggle-text-patch");
    for patch in [
        "--- /other\n+++ /other\n@@ -1 +1 @@\n-draft\n+bad\n",
        "--- /patch.txt\n+++ /patch.txt\n@@ -1 +1 @@\n-mismatch\n+bad\n",
    ] {
        cx.update_window(h.window, |_, window, cx| {
            h.panel
                .read(cx)
                .patch
                .clone()
                .update(cx, |input, cx| input.set_value(patch, window, cx))
        })
        .checked("paste rejected patch");
        h.click(cx, "apply-text-patch");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert_eq!(panel.editor.read(cx).value().as_str(), "draft\n")
        });
    }
    h.click(cx, "save-remote-file");
    cx.update(|cx| i18n::set_language(Language::En, cx));
    h.panel.read_with(cx, |panel, cx| {
        assert!(
            panel
                .pending
                .as_ref()
                .checked_option("English full review")
                .0
                .render(cx)
                .contains("Complete result")
        )
    });
    edit(&h, "changed after review\n", cx);
    h.click(cx, "confirm-file-operation");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert!(panel.status.render(cx).contains("Editor changed"));
    });
    assert_eq!(h.read("/patch.txt"), b"base\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn conflict_navigation_retains_manual_drafts_and_invalidates_edited_manual_choice(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(
        &h,
        "/manual-navigation.txt",
        b"a\nb\nc\n",
        "LOCAL_A\nb\nLOCAL_C\n",
        cx,
    )
    .await;
    external(&h, "/manual-navigation.txt", b"REMOTE_A\nb\nREMOTE_C\n");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    let set_manual = |text: &str, cx: &mut TestAppContext| {
        cx.update_window(h.window, |_, window, cx| {
            h.panel
                .read(cx)
                .merge_review
                .as_ref()
                .checked_option("manual navigation review")
                .manual
                .clone()
                .update(cx, |input, cx| input.set_value(text, window, cx))
        })
        .checked("retain manual replacement buffer")
    };
    set_manual("chosen manual\n", cx);
    h.click(cx, "merge-use-manual");
    set_manual("changed manual\n", cx);
    h.click(cx, "merge-next-conflict");
    h.click(cx, "merge-use-remote");
    h.click(cx, "merge-previous-conflict");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .merge_review
                .as_ref()
                .checked_option("retained manual review")
                .manual
                .read(cx)
                .value()
                .as_str(),
            "changed manual\n"
        )
    });
    h.click(cx, "adopt-text-merge");
    h.panel
        .read_with(cx, |panel, _| assert!(panel.merge_review.is_some()));
    h.click(cx, "merge-use-manual");
    h.click(cx, "merge-show-result");
    h.click(cx, "adopt-text-merge");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.editor.read(cx).value().as_str(),
            "changed manual\nb\nREMOTE_C\n"
        )
    });
    assert_eq!(h.read("/manual-navigation.txt"), b"REMOTE_A\nb\nREMOTE_C\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn complete_merge_versions_keep_literal_rows_and_two_axis_scroll_without_writes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let long = "中文-$(literal);".repeat(300);
    let tail = "tail\n".repeat(40);
    let base = format!("base\n{long}\n{tail}");
    let draft = format!("draft\n{long}\n{tail}");
    let remote = format!("remote\n{long}\n{tail}");
    open(&h, "/complete.txt", base.as_bytes(), &draft, cx).await;
    external(&h, "/complete.txt", remote.as_bytes());
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| i18n::set_language(language, cx));
        for (button, text) in [
            ("merge-show-base", &base),
            ("merge-show-draft", &draft),
            ("merge-show-remote", &remote),
        ] {
            h.click(cx, button);
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let viewport = window.find("text-merge-complete-version").bounds();
                let summary_row = window.find(("text-merge-complete-version", 0usize));
                let summary = summary_row
                    .label()
                    .checked_option("localized complete version metadata");
                assert!(summary.contains(match language {
                    Language::ZhCn => "字节 · 末尾 LF: 有",
                    Language::En => "bytes · final LF: yes",
                }));
                for (index, line) in text.split('\n').enumerate() {
                    assert_eq!(
                        window
                            .find(("text-merge-complete-version", index + 1))
                            .label(),
                        Some(line)
                    );
                }
                let maximum = h.panel.read(cx).text_review_scroll.max_offset();
                assert!(maximum.x > px(100.) && maximum.y > px(100.));
                let outer_before = h.panel.read(cx).tools_scroll.offset();
                let outer=window.find("file-tools-scroll").bounds();
                let top=viewport.origin.y.max(outer.origin.y);let bottom=viewport.bottom().min(outer.bottom());
                assert!(bottom-top>px(10.),"version text viewport has a visible intersection {viewport:?} in {outer:?}");
                let position=point(viewport.origin.x+px(12.),top+(bottom-top)/2.);
                window.dispatch_event(
                    gpui_kit::MouseMoveEvent {
                        position,
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                for delta in [point(px(0.),px(-10000.)),point(px(-10000.),px(0.))] {
                    window.dispatch_event(gpui_kit::MouseMoveEvent {position,..Default::default()}.to_platform_input(),cx);
                    window.dispatch_event(
                        gpui_kit::ScrollWheelEvent {
                            position,
                            delta: gpui_kit::ScrollDelta::Pixels(delta),
                            touch_phase:gpui_kit::TouchPhase::Started,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    window.render_frame(cx);
                }
                let offset = h.panel.read(cx).text_review_scroll.offset();
                assert_eq!(h.panel.read(cx).tools_scroll.offset(),outer_before,"inner text gestures cannot move the outer tools");
                assert!(
                    offset.x < px(-100.) && offset.y < px(-100.),
                    "actual two-axis wheel {offset:?} in {viewport:?}, maximum {maximum:?}, outer {outer:?}, outer offset {:?}",h.panel.read(cx).tools_scroll.offset()
                );
            })
            .checked("literal full-version rows and actual two-axis wheel");
        }
    }
    h.click(cx, "close-text-merge");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value().as_str(), draft)
    });
    assert_eq!(h.read("/complete.txt"), remote.as_bytes());
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn logical_diff_does_not_claim_byte_equality_for_line_ending_changes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/line-endings.txt", b"a\r\nb\r\n", "a\nb\n", cx).await;
    cx.update(|cx| {
        h.panel
            .update(cx, |panel, cx| panel.toggle_diff_preview(cx));
    });
    h.panel.read_with(cx, |panel, cx| {
        let preview = panel.diff_preview.as_ref().checked_option("logical diff");
        assert!(preview.contains("字节仍不同"));
        assert!(preview.contains("bytes still differ"));
        assert!(panel.status.render(cx).contains("字节仍不同"));
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none());
    });
    assert_eq!(h.read("/line-endings.txt"), b"a\r\nb\r\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn rereading_remote_revokes_old_merge_without_adopting_a_different_baseline(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/reread.txt", b"base A\n", "draft L\n", cx).await;
    external(&h, "/reread.txt", b"remote B\n");
    h.click(cx, "read-remote-merge");
    h.idle(cx).await;
    h.click(cx, "merge-use-draft");
    external(&h, "/reread.txt", b"remote C\n");
    let hold = h
        .server
        .filesystem
        .hold_metadata_path("/reread.txt")
        .checked("hold current remote observation after capturing base A");
    h.click(cx, "read-remote-merge");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).busy && hold.entered() > 0
    })
    .await;
    let old_adoption_exists = cx
        .update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            // Exercise a stale visible action if the old review was not revoked.
            window.try_find("adopt-text-merge").is_some()
        })
        .checked("attempt old adoption while a new observation is in flight");
    if old_adoption_exists {
        h.click(cx, "adopt-text-merge");
    }
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.editing.as_ref().checked_option("captured baseline").1,
            b"base A\n"
        );
        assert_eq!(panel.editor.read(cx).value().as_str(), "draft L\n");
        assert!(
            panel.merge_review.is_none(),
            "a new read revokes the old review"
        );
    });
    hold.release();
    h.idle(cx).await;
    h.click(cx, "merge-show-base");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("text-merge-complete-version", 1usize)).label(),
            Some("base A")
        );
    })
    .checked("complete baseline matches the base used to calculate this plan");
    assert_eq!(h.read("/reread.txt"), b"remote C\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn changed_baseline_rejects_a_late_merge_plan_even_when_path_and_draft_match(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    open(&h, "/late-base.txt", b"base A\n", "draft L\n", cx).await;
    external(&h, "/late-base.txt", b"remote C\n");
    let hold = h
        .server
        .filesystem
        .hold_metadata_path("/late-base.txt")
        .checked("hold a read with captured base A");
    h.click(cx, "read-remote-merge");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).busy && hold.entered() > 0
    })
    .await;
    // Model a future editor-baseline replacement independently of the UI's
    // busy guard: the asynchronous result must bind all of its input identities.
    cx.update(|cx| {
        h.panel.update(cx, |panel, _| {
            panel.editing = Some(("/late-base.txt".into(), b"replacement B\n".to_vec()));
        })
    });
    hold.release();
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.merge_review.is_none());
        assert_eq!(panel.editor.read(cx).value().as_str(), "draft L\n");
        assert_eq!(
            panel
                .editing
                .as_ref()
                .checked_option("replacement baseline")
                .1,
            b"replacement B\n"
        );
        assert!(panel.status.render(cx).contains("基线已变化"));
        assert!(panel.pending.is_none());
    });
    assert_eq!(h.read("/late-base.txt"), b"remote C\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
