//! Production GPUI controls submit multiple reviewed jobs on one real SSH peer.
use super::*;
use crate::files::{FileFailure, worker};
use keelshell_session::sftp::{TransferEvent, TransferSpec};

#[gpui_kit::test]
async fn reviewed_queue_overlaps_jobs_and_targets_pause_cancel_and_concurrency_controls(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x69; 256 * 1024];
    let first_source = h.source("parallel-one.bin", &bytes);
    let second_source = h.source("parallel-two.bin", &bytes);
    let hold = h
        .server
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("hold both independent uploads");
    h.local_input(cx, &first_source);
    h.click(cx, "upload-file");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.transfer_jobs.is_empty())
    );
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    let first_id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.local_input(cx, &second_source);
    h.click(cx, "upload-file");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 1, "review alone cannot enqueue or write");
        assert!(matches!(&panel.pending, Some((_, Operation::Upload(_, target))) if target == "/parallel-two.bin"));
    });
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        hold.entered() == 2
            && h.panel
                .read(cx)
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Running)
    })
    .await;
    let second_id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[1].id);
    h.click(cx, "transfer-parallelism-1");
    assert_eq!(
        h.panel.read_with(cx, |panel, _| panel
            .queue_parallelism
            .load(Ordering::Acquire)),
        1
    );
    h.click(cx, &format!("cancel-transfer-{second_id}"));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).transfer_jobs[1].status.phase == TransferPhase::Uncertain
    })
    .await;
    h.click(cx, &format!("pause-transfer-{first_id}"));
    assert!(!hold.expired());
    hold.release();
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).transfer_jobs[0].status.phase == TransferPhase::Paused
    })
    .await;
    h.click(cx, &format!("select-transfer-{first_id}"));
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(h.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.resize(size(px(480.), px(440.)));
            window.bounds_changed(cx);
            window.render_frame(cx);
            reveal_file_control(window, cx, "file-transfer-card");
            assert!(window.find("resume-file-transfer").visible());
            assert!(window.find("cancel-active-file-operation").visible());
            assert_eq!(
                window.find("resume-file-transfer").label(),
                Some(if language == Language::ZhCn {
                    "继续"
                } else {
                    "Continue"
                })
            );
        })
        .checked("actual bilingual paused actions remain reachable");
    }
    let third_source = h.source("waiting-third.bin", b"not admitted yet");
    h.local_input(cx, &third_source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|s| s.phase == TransferPhase::Queued)
    })
    .await;
    let third_id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[2].id);
    h.click(cx, &format!("cancel-transfer-{third_id}"));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).transfer_jobs[2].status.phase == TransferPhase::Cancelled
    })
    .await;
    h.click(cx, &format!("select-transfer-{first_id}"));
    h.click(cx, "resume-file-transfer");
    h.idle(cx).await;
    assert_eq!(h.read("/parallel-one.bin"), bytes);
    h.missing("/parallel-two.bin");
    h.missing("/waiting-third.bin");
    assert!(!h.session.is_closed());
}

#[gpui_kit::test]
async fn suspension_cancels_all_jobs_and_revokes_a_new_pending_review(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let first = h.source("retire-one.bin", &vec![0x42; 128 * 1024]);
    let second = h.source("retire-two.bin", &vec![0x53; 128 * 1024]);
    let hold = h
        .server
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("hold owned old-session mutations");
    for source in [&first, &second] {
        h.local_input(cx, source);
        h.click(cx, "upload-file");
        h.click(cx, "confirm-file-operation");
    }
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 2)
        .await;
    h.local_input(cx, &first);
    h.click(cx, "upload-file");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            let old_token = panel.session_token;
            assert!(panel.pending.is_some());
            panel.suspend(cx);
            assert_ne!(panel.session_token, old_token);
            assert!(panel.pending.is_none());
            panel.execute_pending(window, cx);
            assert_eq!(panel.transfer_jobs.len(), 2);
            assert!(panel.session.is_none());
        });
    })
    .checked("retire exact session and unapproved third submission");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(
            panel
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Uncertain)
        );
        assert!(!panel.can_offer_recovery());
        assert!(panel.recovery.is_none());
    });
    hold.release();
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
    })
    .await;
    h.missing("/retire-one.bin");
    h.missing("/retire-two.bin");
}

#[gpui_kit::test]
async fn isolation_inspection_requires_full_bilingual_risk_review_and_preserves_unknown_history(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x73; 256 * 1024];
    let source = h.source("consent.bin", &bytes);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/consent.bin")
        .checked("hold consent candidate mutation");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    let original = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.click(cx, &format!("cancel-transfer-{original}"));
    h.phase(cx, TransferPhase::Uncertain).await;
    h.idle(cx).await;
    hold.release();
    h.click(cx, "inspect-transfer-isolation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(matches!(&panel.pending, Some((_, Operation::AcknowledgeQuarantine(review))) if review.entries().len() == 2 && review.entries()[0].destination == "/consent.bin" && review.entries()[0].reservation_id > 0));
        assert_eq!(panel.transfer_jobs[0].status.phase, TransferPhase::Uncertain);
    });
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(h.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.resize(size(px(480.), px(440.)));
            window.bounds_changed(cx);
            h.panel
                .update(cx, |panel, cx| panel.refresh_locale(window, cx));
            window.render_frame(cx);
            let panel = h.panel.read(cx);
            let (message, _) = panel
                .pending
                .as_ref()
                .checked_option("risk review remains pending");
            let text = message.render(cx);
            assert!(text.contains("/consent.bin") && text.contains('#'));
            assert!(text.contains(if language == Language::ZhCn {
                "无法证明"
            } else {
                "cannot prove"
            }));
            assert!(window.find("confirm-file-operation").visible());
            assert!(window.find("cancel-file-operation").visible());
        })
        .checked("complete risk and reservation IDs with fixed bilingual actions");
    }
    h.click(cx, "cancel-file-operation");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs[1].status.phase, TransferPhase::Failed)
    });
    h.missing("/consent.bin");
    h.click(cx, &format!("select-transfer-{original}"));
    h.click(cx, "inspect-transfer-isolation");
    h.idle(cx).await;
    h.server
        .filesystem
        .replace_external_file("/consent.bin", b"changed after inspection")
        .checked("explicit external filesystem change invalidates risk observations");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/consent.bin"), b"changed after inspection");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Uncertain
        )
    });
    h.click(cx, "inspect-transfer-isolation");
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Uncertain
        );
        assert!(panel.pending.is_none());
    });
    h.click(cx, "upload-file");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs.len(),
            2,
            "acknowledgement cannot enqueue or replay"
        )
    });
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/consent.bin"), bytes);
}

#[gpui_kit::test]
async fn suspension_during_isolation_inspection_discards_late_review_and_cannot_release_target(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("revoked-consent.bin", &vec![0x33; 128 * 1024]);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/revoked-consent.bin")
        .checked("own delayed old mutation");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    let id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.click(cx, &format!("cancel-transfer-{id}"));
    h.phase(cx, TransferPhase::Uncertain).await;
    h.idle(cx).await;
    hold.release();
    let inspection = h
        .server
        .filesystem
        .hold_canonical_path("/")
        .checked("own read-only inspection hold");
    h.click(cx, "inspect-transfer-isolation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        inspection.entered() == 1
    })
    .await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            panel.execute_pending(window, cx);
            assert!(panel.pending.is_none());
        });
    })
    .checked("revoke inspection authority");
    assert!(!inspection.expired());
    inspection.release();
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Uncertain
        );
    });
    h.runtime.block_on(async {
        let sftp = Arc::new(h.session.sftp().await.checked("inspect old quarantine"));
        let queue = sftp.clone().transfer_queue();
        let mut transfer = queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/revoked-consent.bin"))
            .await
            .checked("explicit new request on old connection");
        loop {
            match tokio::time::timeout(Duration::from_secs(5), transfer.recv())
                .await
                .checked("bounded quarantine result")
            {
                Some(TransferEvent::Failed { .. }) => break,
                Some(TransferEvent::Queued { .. }) => {}
                other => panic!("revoked inspection cannot clear isolation: {other:?}"),
            }
        }
        queue.close().await.checked("close rejected queue");
        sftp.close().await.checked("close read-only check");
    });
    h.missing("/revoked-consent.bin");
}

#[gpui_kit::test]
async fn cancellation_during_quarantine_confirmation_cannot_clear_reviewed_reservation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("confirm-revoked.bin", &vec![0x4d; 128 * 1024]);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/confirm-revoked.bin")
        .checked("own unresolved mutation");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    let id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.click(cx, &format!("cancel-transfer-{id}"));
    h.phase(cx, TransferPhase::Uncertain).await;
    h.idle(cx).await;
    hold.release();
    h.click(cx, "inspect-transfer-isolation");
    h.idle(cx).await;
    let confirmation = h
        .server
        .filesystem
        .hold_metadata_path("/confirm-revoked.bin")
        .checked("hold read-only ACK recheck");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        confirmation.entered() == 1
    })
    .await;
    h.click(cx, "cancel-active-file-operation");
    assert!(!confirmation.expired());
    confirmation.release();
    h.idle(cx).await;
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("inspect unchanged reservation");
        let review = sftp
            .inspect_transfer_quarantine(&TransferSpec::upload(&source, "/confirm-revoked.bin"))
            .await
            .checked("revoked confirmation retains quarantine");
        assert_eq!(review.entries()[0].destination, "/confirm-revoked.bin");
        sftp.close().await.checked("close isolated inspection");
    });
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Uncertain
        )
    });
    h.missing("/confirm-revoked.bin");
}

#[gpui_kit::test]
async fn lost_pause_control_waits_for_unknown_terminal_instead_of_claiming_cancellation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("lost-control.bin", &vec![0x2f; 128 * 1024]);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/lost-control.bin")
        .checked("own pending mutation");
    h.runtime.block_on(async {
        let sftp = Arc::new(h.session.sftp().await.checked("lost-control SFTP"));
        let queue = sftp.clone().transfer_queue();
        let mut transfer = queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/lost-control.bin"))
            .await
            .checked("submit controlled upload");
        tokio::time::timeout(Duration::from_secs(5), async {
            while hold.entered() != 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .checked("held mutation reached");
        let (pause, receiver) = tokio::sync::watch::channel(false);
        drop(pause);
        let (progress, _messages) = std::sync::mpsc::sync_channel(16);
        let result = worker::observe_transfer(
            &mut transfer,
            keelshell_session::sftp::TransferDirection::Upload,
            None,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            receiver,
            &progress,
        )
        .await;
        assert!(
            matches!(result, Err(FileFailure::OutcomeUncertain(_))),
            "control loss must preserve the real unknown terminal"
        );
        assert!(!hold.expired());
        hold.release();
        queue.close().await.checked("close cancelled queue");
        sftp.close().await.checked("close owned base channel");
    });
    h.missing("/lost-control.bin");
}
