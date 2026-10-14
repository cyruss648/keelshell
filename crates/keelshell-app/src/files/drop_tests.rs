//! Platform file-drop events use the production read-only review and SFTP queue.
use super::*;
use gpui_kit::{ExternalPaths, FileDropEvent};

fn drop_paths(h: &Harness, paths: Vec<PathBuf>, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let table = window.find("remote-files-table");
        assert!(table.visible());
        let position = table.bounds().center();
        window.dispatch_event(
            FileDropEvent::Entered {
                position,
                paths: ExternalPaths(paths.into_iter().collect()),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        window.dispatch_event(FileDropEvent::Pending { position }.to_platform_input(), cx);
        window.dispatch_event(FileDropEvent::Submit { position }.to_platform_input(), cx);
        window.dispatch_event(FileDropEvent::Ended.to_platform_input(), cx);
    })
    .checked("dispatch actual platform file-drop events to the remote table");
}

fn review(h: &Harness, cx: &TestAppContext) -> (uuid::Uuid, usize) {
    h.panel.read_with(cx, |panel, _| match &panel.pending {
        Some((_, Operation::TransferBatch(plan))) => (plan.id, plan.ready_count()),
        _ => panic!("a drop must create an explicit batch review"),
    })
}

#[gpui_kit::test]
async fn drop_upload_reviews_files_and_folders_before_cancel_or_exact_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let file = h.source("拖放.txt", b"exact dropped file bytes\n");
    let folder = h.local.0.join("drop-folder");
    std::fs::create_dir(&folder).checked("create owned dropped directory");
    std::fs::write(folder.join("child.txt"), b"nested dropped bytes")
        .checked("seed the dropped directory");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.local_browser.listing.is_none())
    );
    drop_paths(&h, vec![file.clone(), folder.clone()], cx);
    h.idle(cx).await;
    assert_eq!(review(&h, cx).1, 2);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.transfer_jobs.is_empty())
    );
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| i18n::set_language(language, cx));
        h.panel.read_with(cx, |panel, cx| {
            let text = panel
                .pending
                .as_ref()
                .checked_option("drop review")
                .0
                .render(cx);
            assert!(text.contains(&file.display().to_string()));
            assert!(text.contains(&folder.display().to_string()));
            assert!(text.contains("/拖放.txt"));
            assert!(text.contains("/drop-folder"));
            assert!(text.contains(match language {
                Language::ZhCn => "拖入的本地路径（逐项列出）",
                Language::En => "Dropped local paths (listed individually)",
            }));
        });
    }
    h.click(cx, "cancel-file-operation");
    h.missing("/拖放.txt");
    h.missing("/drop-folder");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    drop_paths(&h, vec![file, folder], cx);
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 2);
        assert!(
            panel
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Completed)
        );
        assert!(panel.local_browser.listing.is_none());
    });
    assert_eq!(h.read("/拖放.txt"), b"exact dropped file bytes\n");
    assert_eq!(h.read("/drop-folder/child.txt"), b"nested dropped bytes");
}

#[gpui_kit::test]
async fn drop_upload_reports_each_missing_nonportable_and_duplicate_destination(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let first = h.source("same.txt", b"first selected exact source");
    let other = h.local.0.join("other");
    std::fs::create_dir(&other).checked("create second dropped source parent");
    let duplicate = other.join("same.txt");
    std::fs::write(&duplicate, b"rejected same-name source").checked("seed duplicate basename");
    let missing = h.local.0.join("missing.txt");
    let unsafe_name = h.source("trailing.", b"nonportable destination");
    drop_paths(&h, vec![first, duplicate, missing, unsafe_name], cx);
    h.idle(cx).await;
    assert_eq!(review(&h, cx).1, 1);
    h.panel.read_with(cx, |panel, _| {
        let message = &panel.pending.as_ref().checked_option("all drop outcomes").0;
        assert!(message.translations().0.contains("3 个拒绝项"));
        assert!(message.translations().1.contains("3 rejected entries"));
        for name in ["same.txt", "missing.txt", "trailing."] {
            assert!(message.translations().0.contains(name));
            assert!(message.translations().1.contains(name));
        }
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        h.panel.read_with(cx, |panel, _| panel.transfer_jobs.len()),
        1
    );
    assert_eq!(h.read("/same.txt"), b"first selected exact source");
    h.missing("/missing.txt");
    h.missing("/trailing.");
}

#[gpui_kit::test]
async fn drop_upload_limit_and_pending_review_preserve_the_original_intent(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let file = h.source("bounded.txt", b"unapproved source");
    drop_paths(&h, vec![file.clone(); 33], cx);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert!(!panel.busy);
        assert!(panel.operation_id.is_none());
        assert!(panel.status.translations().0.contains("1–32"));
        assert!(panel.status.translations().1.contains("1–32"));
    });
    drop_paths(&h, vec![file.clone()], cx);
    h.idle(cx).await;
    let original = review(&h, cx);
    drop_paths(&h, vec![h.source("replacement.txt", b"not reviewed")], cx);
    assert_eq!(review(&h, cx), original);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "cancel-file-operation");
    // Entering and then leaving a window is not a drop and grants no review.
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find("remote-files-table").bounds().center();
        window.dispatch_event(
            FileDropEvent::Entered {
                position,
                paths: ExternalPaths([file].into_iter().collect()),
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(FileDropEvent::Exited.to_platform_input(), cx);
    })
    .checked("cancel native drag before dropping");
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()
        && panel.transfer_jobs.is_empty()));
    h.missing("/bounded.txt");
    h.missing("/replacement.txt");
}

#[gpui_kit::test]
async fn drop_upload_navigation_and_suspension_revoke_unexecuted_sources(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let file = h.source("revoked.txt", b"never transferred");
    drop_paths(&h, vec![file.clone()], cx);
    h.idle(cx).await;
    let previous = h.panel.read_with(cx, |panel, _| {
        panel
            .pending
            .as_ref()
            .checked_option("review to revoke")
            .1
            .clone()
    });
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.run(previous, window, cx));
    })
    .checked("replay withdrawn exact source plan after navigation");
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()
        && panel.transfer_jobs.is_empty()));
    drop_paths(&h, vec![file.clone()], cx);
    h.idle(cx).await;
    assert_eq!(review(&h, cx).1, 1);
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
    })
    .checked("suspend the original SSH session capability");
    drop_paths(&h, vec![file], cx);
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()
        && panel.transfer_jobs.is_empty()));
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.missing("/revoked.txt");
}

#[gpui_kit::test]
async fn drop_upload_during_active_transfer_does_not_prepare_another_target(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let first = h.source("active.bin", &vec![0x46; 256 * 1024]);
    let blocked = h.source("blocked.txt", b"not admitted while busy");
    let hold = h
        .server
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("hold owned upload");
    drop_paths(&h, vec![first], cx);
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        hold.entered() == 1 && h.panel.read(cx).busy
    })
    .await;
    drop_paths(&h, vec![blocked], cx);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert_eq!(panel.transfer_jobs.len(), 1);
        assert!(panel.operation_id.is_none());
    });
    assert!(!hold.expired());
    hold.release();
    h.idle(cx).await;
    assert_eq!(h.read("/active.bin"), vec![0x46; 256 * 1024]);
    h.missing("/blocked.txt");
}

#[cfg(unix)]
#[gpui_kit::test]
async fn drop_upload_does_not_follow_a_symbolic_link_source(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("ordinary.txt", b"original local bytes");
    let link = h.local.0.join("linked.txt");
    std::os::unix::fs::symlink(&source, &link).checked("create owned dropped symbolic link");
    drop_paths(&h, vec![source, link], cx);
    h.idle(cx).await;
    assert_eq!(review(&h, cx).1, 1);
    h.panel.read_with(cx, |panel, _| {
        let text = panel
            .pending
            .as_ref()
            .checked_option("explicit link refusal")
            .0
            .translations();
        assert!(text.0.contains("linked.txt"));
        assert!(text.1.contains("1 rejected entries"));
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/ordinary.txt"), b"original local bytes");
    h.missing("/linked.txt");
}
