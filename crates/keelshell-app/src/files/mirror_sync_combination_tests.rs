//! Independent combinations of real SFTP work and production sync modal events.
use super::*;
use crate::workspace::tests::mirror_sync_combination::{Scene, mount_scene};

fn combination(cx: &mut TestAppContext) -> (Harness, Scene) {
    let mut scene = None;
    let h = Harness::new_with(cx, |cx, session, runtime| {
        let mounted = mount_scene(cx, session, runtime);
        let pair = (mounted.window, mounted.files.clone());
        scene = Some(mounted);
        pair
    });
    (
        h,
        scene.checked_option("retained production workspace scene"),
    )
}

#[gpui_kit::test]
async fn profile_sync_disable_keeps_original_queue_owner_and_finishes_exact_upload(
    cx: &mut TestAppContext,
) {
    let (h, scene) = combination(cx);
    h.idle(cx).await;
    let bytes = vec![0x63; 256 * 1024];
    let source = h.source("captured-owner.bin", &bytes);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/captured-owner.bin")
        .checked("real second WRITE held after first ACK");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        hold.entered() == 1
            && h.panel
                .read(cx)
                .transfer_jobs
                .first()
                .is_some_and(|job| job.status.phase == TransferPhase::Running)
    })
    .await;
    let (token, job_id, queue) = h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 1);
        assert_eq!(panel.transfer_jobs[0].status.phase, TransferPhase::Running);
        (
            panel.session_token,
            panel.transfer_jobs[0].id,
            panel
                .transfer_queue
                .get()
                .checked_option("initialized owned queue")
                .clone(),
        )
    });
    scene.open_sync(cx);
    scene.external_saved_host_then_disable(cx);
    scene.disabled(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(!panel.suspended);
        assert_eq!(panel.session_token, token);
        assert_eq!(panel.transfer_jobs[0].id, job_id);
        assert!(Arc::ptr_eq(
            panel
                .transfer_queue
                .get()
                .checked_option("same initialized queue"),
            &queue
        ));
        assert!(
            panel
                .session
                .as_ref()
                .checked_option("same live SSH")
                .same_connection(&h.session)
        );
    });
    assert!(!hold.expired());
    scene.close_sync(cx);
    h.panel.read_with(cx, |panel, _| {
        assert!(Arc::ptr_eq(
            panel
                .transfer_queue
                .get()
                .checked_option("same active owner after Close"),
            &queue
        ))
    });
    hold.release();
    h.idle(cx).await;
    assert_eq!(h.read("/captured-owner.bin"), bytes);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs[0].id, job_id);
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Completed
        );
        // Production closes and clears a queue when its last job finishes.
        // This retirement must preserve the exact completed job record.
        assert!(panel.transfer_queue.get().is_none());
    });
    scene.inspect_history_without_replay(cx);
    h.runtime
        .block_on(h.session.close())
        .checked("close controlled SSH");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
    })
    .await;
    eprintln!(
        "actual combination: Disable/Changed and Close kept queue Arc/token/id; original authenticated upload exact=262144; Unknown history PTY replay=0"
    );
}

#[gpui_kit::test]
async fn profile_sync_close_and_history_cannot_restore_retired_mirror_approval(
    cx: &mut TestAppContext,
) {
    let (h, scene) = combination(cx);
    h.idle(cx).await;
    h.seed(
        "/reviewed-delete",
        b"preserve the captured authenticated target",
    );
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-remote");
    h.idle(cx).await;
    h.click(cx, "review-directory-sync");
    let token = h.panel.read_with(cx, |panel, _| {
        assert!(matches!(
            &panel.pending,
            Some((_, Operation::ApplyDirectorySync(_, _)))
        ));
        panel.session_token
    });
    scene.open_sync(cx);
    scene.retire_files(cx).await;
    scene.external_saved_host_then_disable(cx);
    scene.disabled(cx).await;
    scene.close_sync(cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            assert!(panel.suspended && panel.session.is_none() && panel.pending.is_none());
            assert_ne!(panel.session_token, token);
            panel.execute_pending(window, cx);
            assert!(panel.sync_journal.is_none() && panel.transfer_jobs.is_empty());
        });
        window.render_frame(cx);
        assert!(window.try_find("confirm-file-operation").is_none());
    })
    .checked("late stale confirmation handler cannot regain authority");
    scene.inspect_history_without_replay(cx);
    assert_eq!(
        h.read("/reviewed-delete"),
        b"preserve the captured authenticated target"
    );
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.runtime
        .block_on(h.session.close())
        .checked("close controlled SSH");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
    })
    .await;
    eprintln!(
        "actual combination: sync modal + production suspend_panels + Disable/Close + Unknown viewer; retired mirror stays unapproved; original target exact; atomic writes=0; PTY replay=0"
    );
}
