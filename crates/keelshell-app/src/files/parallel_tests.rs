//! Production GPUI controls submit multiple reviewed jobs on one real SSH peer.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::files::{FileFailure, worker};
use crate::i18n;
use gpui_kit::{Bounds, WindowBounds, WindowOptions, point, px, size, test::TestWindowExt};
use keelshell_core::{
    Language, Settings, ThemeId, TransferRecoveryDirection, TransferRecoveryRecord,
    TransferRecoveryState, TransferRecoveryStore,
};
use keelshell_session::sftp::{TransferEvent, TransferSpec};
use std::sync::Arc;
use std::time::Duration;

fn mount_recovery_harness(
    cx: &mut TestAppContext,
    store: Arc<TransferRecoveryStore>,
    connection_id: Option<uuid::Uuid>,
) -> Harness {
    Harness::new_with(cx, {
        let store = store.clone();
        move |cx, session, runtime| {
            cx.update(|cx| {
                gpui_kit::init(cx);
                i18n::set_language(Language::ZhCn, cx);
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.), px(0.)),
                            size(px(1100.), px(440.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            FilesPanel::new_with_recovery_store_and_connection(
                                session,
                                "fixture SSH".into(),
                                connection_id,
                                runtime,
                                store,
                                window,
                                cx,
                            )
                        })
                    },
                )
                .checked("mount persisted recovery fixture")
            })
        }
    })
}

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
async fn persisted_recovery_card_is_reachable_in_all_registered_families_and_discard_is_explicit(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::TempDir::new().expect("recovery fixture directory");
    let source = directory.path().join("partial-source.bin");
    std::fs::write(&source, b"partial bytes").expect("recovery fixture source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    let connection_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new_with_connection_id(
                recovery_id,
                Some(connection_id),
                TransferRecoveryDirection::Upload,
                &source,
                "/restart.bin",
                false,
                TransferRecoveryState::Failed,
                6,
                Some(12),
                6,
                100,
                100,
            )
            .expect("valid persisted recovery fixture"),
        )
        .expect("persist recovery fixture");

    let h = Harness::new_with(cx, {
        let store = store.clone();
        move |cx, session, runtime| {
            cx.update(|cx| {
                gpui_kit::init(cx);
                i18n::set_language(Language::ZhCn, cx);
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.), px(0.)),
                            size(px(1100.), px(440.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            FilesPanel::new_with_recovery_store_and_connection(
                                session,
                                "fixture SSH".into(),
                                Some(connection_id),
                                runtime,
                                store,
                                window,
                                cx,
                            )
                        })
                    },
                )
                .checked("mount persisted recovery card")
            })
        }
    });
    h.idle(cx).await;
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;

    for family in crate::design::registry() {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.window, |_, window, cx| {
                let settings = Settings {
                    theme_family: ThemeId::new(family.id).expect("registered recovery card theme"),
                    theme: keelshell_core::Theme::Light,
                    ..Settings::default()
                };
                crate::design::apply_settings(&settings, Some(window), cx);
                i18n::set_language(language, cx);
                if window.try_find("file-transfer-queue-details").is_none() {
                    window.click("toggle-file-transfer-queue", cx);
                }
                window.render_frame(cx);
                let inspect = format!("inspect-persisted-transfer-{recovery_id}");
                let discard = format!("discard-persisted-transfer-{recovery_id}");
                assert!(window.find(inspect.clone()).visible());
                assert!(window.find(discard).visible());
                assert!(
                    window
                        .find("persisted-transfer-recovery-current-session")
                        .visible()
                );
            })
            .checked("render persisted recovery card across themes and languages");
        }
    }

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("discard-persisted-transfer-{recovery_id}"), cx);
    })
    .checked("explicitly discard persisted recovery record");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).recovery_records.is_empty()
    })
    .await;
    assert!(
        store
            .load()
            .expect("load after explicit discard")
            .is_empty()
    );
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.transfer_jobs.is_empty());
        assert!(panel.pending.is_none());
    });
}

#[gpui_kit::test]
async fn unbound_persisted_recovery_requires_host_confirmation_before_read_only_inspection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::TempDir::new().expect("recovery fixture directory");
    let source = directory.path().join("legacy-source.bin");
    std::fs::write(&source, b"legacy partial bytes").expect("write recovery fixture source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new(
                recovery_id,
                TransferRecoveryDirection::Upload,
                &source,
                "/legacy-restart.bin",
                false,
                TransferRecoveryState::Failed,
                6,
                Some(18),
                6,
                100,
                100,
            )
            .expect("valid unbound recovery fixture"),
        )
        .expect("persist unbound recovery fixture");

    let h = Harness::new_with(cx, {
        let store = store.clone();
        move |cx, session, runtime| {
            cx.update(|cx| {
                gpui_kit::init(cx);
                i18n::set_language(Language::ZhCn, cx);
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.), px(0.)),
                            size(px(1100.), px(440.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            FilesPanel::new_with_recovery_store_and_connection(
                                session,
                                "legacy fixture SSH".into(),
                                None,
                                runtime,
                                store,
                                window,
                                cx,
                            )
                        })
                    },
                )
                .checked("mount unbound recovery fixture")
            })
        }
    });
    h.idle(cx).await;
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        if window.try_find("file-transfer-queue-details").is_none() {
            window.click("toggle-file-transfer-queue", cx);
        }
        window.render_frame(cx);
        assert!(
            window
                .find(format!("inspect-persisted-transfer-{recovery_id}"))
                .visible()
        );
        window.click(format!("inspect-persisted-transfer-{recovery_id}"), cx);
    })
    .checked("request unbound persisted recovery inspection");

    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel
            .read(cx)
            .pending
            .as_ref()
            .is_some_and(|(_, operation)| {
                matches!(
                    operation,
                    Operation::PlanPersistedRecovery { id, .. } if *id == recovery_id
                )
            })
            && h.panel.read(cx).operation_id.is_none()
    })
    .await;
    h.panel.read_with(cx, |panel, cx| {
        let (message, operation) = panel.pending.as_ref().expect("host confirmation review");
        assert!(matches!(
            operation,
            Operation::PlanPersistedRecovery { id, .. } if *id == recovery_id
        ));
        let (zh, _) = message.translations();
        assert!(zh.contains("没有保存连接身份"));
        assert!(panel.recovery_claim.is_some());
        assert!(panel.operation_id.is_none());
        let _ = cx;
    });

    h.click(cx, "cancel-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).recovery_claim.is_none()
    })
    .await;
    let claim = store
        .claim(recovery_id)
        .expect("cancelling host confirmation releases the claim");
    drop(claim);
}

#[gpui_kit::test]
async fn persisted_recovery_plan_progress_is_durable_when_review_is_cancelled(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::TempDir::new().expect("recovery fixture directory");
    let source = directory
        .path()
        .canonicalize()
        .expect("canonical recovery fixture directory")
        .join("plan-source.bin");
    std::fs::write(&source, b"0123456789abcdef").expect("write recovery fixture source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    let connection_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new_with_connection_id(
                recovery_id,
                Some(connection_id),
                TransferRecoveryDirection::Upload,
                &source,
                "/plan-restart.bin",
                false,
                TransferRecoveryState::Failed,
                2,
                Some(16),
                2,
                100,
                100,
            )
            .expect("valid persisted recovery fixture"),
        )
        .expect("persist recovery fixture");

    let h = mount_recovery_harness(cx, store.clone(), Some(connection_id));
    h.idle(cx).await;
    h.seed("/plan-restart.bin", b"012345");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        if window.try_find("file-transfer-queue-details").is_none() {
            window.click("toggle-file-transfer-queue", cx);
        }
        window.render_frame(cx);
    })
    .checked("open persisted recovery queue");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let inspect = format!("inspect-persisted-transfer-{recovery_id}");
        assert!(window.find(inspect.clone()).visible());
        window.click(inspect, cx);
    })
    .checked("inspect persisted recovery after queue render");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel
            .read(cx)
            .pending
            .as_ref()
            .is_some_and(|(_, operation)| {
                matches!(
                    operation,
                    Operation::ResumePersistedFile(id, plan)
                        if *id == recovery_id && plan.existing_bytes() == 6
                )
            })
    })
    .await;

    let updated = store
        .load()
        .expect("load progress after read-only plan")
        .into_iter()
        .find(|record| record.id == recovery_id)
        .expect("planned recovery record");
    assert_eq!(updated.reviewed_existing_bytes, 6);
    assert_eq!(updated.total_bytes, Some(16));
    assert_eq!(updated.transferred_bytes, 6);

    h.click(cx, "cancel-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).recovery_claim.is_none()
    })
    .await;
    let retained = store
        .load()
        .expect("load progress after cancelled review")
        .into_iter()
        .find(|record| record.id == recovery_id)
        .expect("retained recovery record");
    assert_eq!(retained.reviewed_existing_bytes, 6);
    assert_eq!(retained.transferred_bytes, 6);
}

#[gpui_kit::test]
async fn persisted_recovery_confirmation_failure_releases_the_claim(cx: &mut TestAppContext) {
    let directory = tempfile::TempDir::new().expect("recovery fixture directory");
    let source = directory
        .path()
        .canonicalize()
        .expect("canonical recovery fixture directory")
        .join("stale-source.bin");
    std::fs::write(&source, b"0123456789abcdef").expect("write recovery fixture source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    let connection_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new_with_connection_id(
                recovery_id,
                Some(connection_id),
                TransferRecoveryDirection::Upload,
                &source,
                "/stale-restart.bin",
                false,
                TransferRecoveryState::Failed,
                2,
                Some(16),
                2,
                100,
                100,
            )
            .expect("valid persisted recovery fixture"),
        )
        .expect("persist recovery fixture");

    let h = mount_recovery_harness(cx, store.clone(), Some(connection_id));
    h.idle(cx).await;
    h.seed("/stale-restart.bin", b"012345");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;

    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        if window.try_find("file-transfer-queue-details").is_none() {
            window.click("toggle-file-transfer-queue", cx);
        }
        window.render_frame(cx);
    })
    .checked("open persisted recovery queue for failed confirmation");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let inspect = format!("inspect-persisted-transfer-{recovery_id}");
        assert!(window.find(inspect.clone()).visible());
        window.click(inspect, cx);
    })
    .checked("inspect persisted recovery before failed confirmation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).pending.as_ref().is_some_and(|(_, operation)| {
            matches!(operation, Operation::ResumePersistedFile(id, _) if *id == recovery_id)
        })
    })
    .await;

    h.panel.update(cx, |panel, _| {
        panel.recovery_records.clear();
    });
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).pending.is_none() && h.panel.read(cx).operation_id.is_none()
    })
    .await;

    let claim = store
        .claim(recovery_id)
        .expect("confirmation preflight failure releases the claim");
    drop(claim);
}

#[gpui_kit::test]
async fn persistent_queued_upload_removes_its_record_after_success(cx: &mut TestAppContext) {
    let directory = tempfile::TempDir::new().expect("persistent queue fixture directory");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let h = mount_recovery_harness(cx, store.clone(), Some(uuid::Uuid::new_v4()));
    h.idle(cx).await;

    let bytes = vec![0x4a; 96 * 1024];
    let source = h.source("persistent-success.bin", &bytes);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");

    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        h.panel.read(cx).transfer_jobs.first().is_some_and(|job| {
            job.status.phase == TransferPhase::Completed
                && store.load().is_ok_and(|records| records.is_empty())
        })
    })
    .await;

    assert_eq!(h.read("/persistent-success.bin"), bytes);
}

#[gpui_kit::test]
async fn persistent_queue_initialization_failure_retains_a_failed_record(cx: &mut TestAppContext) {
    let directory = tempfile::TempDir::new().expect("persistent queue failure fixture directory");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let h = mount_recovery_harness(cx, store.clone(), Some(uuid::Uuid::new_v4()));
    h.idle(cx).await;

    let source = h.source("persistent-queue-failure.bin", b"queue failure");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.runtime
        .block_on(h.session.close())
        .checked("close session before queue initialization");
    h.click(cx, "confirm-file-operation");

    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        h.panel.read(cx).transfer_jobs.first().is_some_and(|job| {
            job.status.phase == TransferPhase::Failed
                && store.load().is_ok_and(|records| {
                    records
                        .iter()
                        .any(|record| record.state == TransferRecoveryState::Failed)
                })
        })
    })
    .await;
}

#[gpui_kit::test]
async fn persistent_cancel_retains_an_uncertain_or_cancelled_record(cx: &mut TestAppContext) {
    let directory = tempfile::TempDir::new().expect("persistent cancellation fixture directory");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let h = mount_recovery_harness(cx, store.clone(), Some(uuid::Uuid::new_v4()));
    h.idle(cx).await;

    let bytes = vec![0x6b; 256 * 1024];
    let source = h.source("persistent-cancel.bin", &bytes);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/persistent-cancel.bin")
        .checked("hold persistent cancellation upload");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        hold.entered() == 1
            && h.panel.read(cx).transfer_jobs.first().is_some_and(|job| {
                matches!(
                    job.status.phase,
                    TransferPhase::Running | TransferPhase::Pausing
                )
            })
    })
    .await;

    let job_id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.click(cx, &format!("cancel-transfer-{job_id}"));
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        h.panel.read(cx).transfer_jobs.first().is_some_and(|job| {
            matches!(
                job.status.phase,
                TransferPhase::Uncertain | TransferPhase::Cancelled
            )
        }) && store.load().is_ok_and(|records| {
            records.iter().any(|record| {
                matches!(
                    record.state,
                    TransferRecoveryState::OutcomeUnknown | TransferRecoveryState::Cancelled
                )
            })
        })
    })
    .await;
    hold.release();
}

#[gpui_kit::test]
async fn persistent_concurrent_terminals_leave_a_current_empty_recovery_list(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::TempDir::new().expect("persistent concurrent fixture directory");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let h = mount_recovery_harness(cx, store.clone(), Some(uuid::Uuid::new_v4()));
    h.idle(cx).await;

    let first_bytes = vec![0x71; 160 * 1024];
    let second_bytes = vec![0x72; 160 * 1024];
    let first = h.source("persistent-concurrent-one.bin", &first_bytes);
    let second = h.source("persistent-concurrent-two.bin", &second_bytes);
    let hold = h
        .server
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("hold persistent concurrent uploads");
    for source in [&first, &second] {
        h.local_input(cx, source);
        h.click(cx, "upload-file");
        h.click(cx, "confirm-file-operation");
    }
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        hold.entered() == 2
            && h.panel.read(cx).transfer_jobs.len() == 2
            && h.panel.read(cx).transfer_jobs.iter().all(|job| {
                matches!(
                    job.status.phase,
                    TransferPhase::Running | TransferPhase::Queued
                )
            })
    })
    .await;

    hold.release();
    cx.wait_for(h.window, Duration::from_secs(15), |_, cx| {
        let panel = h.panel.read(cx);
        panel.transfer_jobs.len() == 2
            && panel
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Completed)
            && !panel.recovery_loading
            && panel.recovery_records.is_empty()
            && store.load().is_ok_and(|records| records.is_empty())
    })
    .await;
    assert_eq!(h.read("/persistent-concurrent-one.bin"), first_bytes);
    assert_eq!(h.read("/persistent-concurrent-two.bin"), second_bytes);
}

#[gpui_kit::test]
async fn stale_recovery_load_cannot_restore_a_record_after_discard_mutation(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::TempDir::new().expect("discard race fixture directory");
    let source = directory.path().join("discard-race-source.bin");
    std::fs::write(&source, b"discard race").expect("write discard race source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    let connection_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new_with_connection_id(
                recovery_id,
                Some(connection_id),
                TransferRecoveryDirection::Upload,
                &source,
                "/discard-race.bin",
                false,
                TransferRecoveryState::Failed,
                4,
                Some(12),
                4,
                100,
                100,
            )
            .expect("valid discard race fixture"),
        )
        .expect("persist discard race fixture");

    let h = mount_recovery_harness(cx, store.clone(), Some(connection_id));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;
    let (session, load_generation, mutation_generation, stale_records) =
        h.panel.read_with(cx, |panel, _| {
            (
                panel.session_token,
                panel.recovery_load_generation,
                panel.recovery_mutation_generation,
                panel.recovery_records.clone(),
            )
        });
    assert_eq!(stale_records.len(), 1);

    store.remove(recovery_id).expect("discard recovery fixture");
    h.panel.update(cx, |panel, cx| {
        panel.apply_discard_result(recovery_id, session, Ok(()), cx);
        panel.apply_recovery_load(
            session,
            load_generation,
            mutation_generation,
            Ok(stale_records),
            cx,
        );
    });
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.recovery_records.is_empty());
        assert!(!panel.recovery_loading);
    });
    assert!(store.load().expect("load after discard race").is_empty());
}

#[gpui_kit::test]
async fn stale_recovery_load_cannot_revert_persisted_plan_progress(cx: &mut TestAppContext) {
    let directory = tempfile::TempDir::new().expect("plan progress race fixture directory");
    let source = directory.path().join("plan-progress-race-source.bin");
    std::fs::write(&source, b"plan progress race").expect("write plan progress race source");
    let store = Arc::new(TransferRecoveryStore::new(
        directory.path().join("transfer-recovery.json"),
    ));
    let recovery_id = uuid::Uuid::new_v4();
    let connection_id = uuid::Uuid::new_v4();
    store
        .upsert(
            TransferRecoveryRecord::new_with_connection_id(
                recovery_id,
                Some(connection_id),
                TransferRecoveryDirection::Upload,
                &source,
                "/plan-progress-race.bin",
                false,
                TransferRecoveryState::Failed,
                2,
                Some(16),
                2,
                100,
                100,
            )
            .expect("valid plan progress race fixture"),
        )
        .expect("persist plan progress race fixture");

    let h = mount_recovery_harness(cx, store.clone(), Some(connection_id));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        !h.panel.read(cx).recovery_loading
    })
    .await;
    let (session, load_generation, mutation_generation, stale_records) =
        h.panel.read_with(cx, |panel, _| {
            (
                panel.session_token,
                panel.recovery_load_generation,
                panel.recovery_mutation_generation,
                panel.recovery_records.clone(),
            )
        });
    let mut persisted = stale_records[0].clone();
    persisted.reviewed_existing_bytes = 6;
    persisted.transferred_bytes = 6;
    persisted.total_bytes = Some(16);
    persisted.updated_at = 101;
    store
        .upsert(persisted)
        .expect("persist updated plan progress");

    h.panel.update(cx, |panel, cx| {
        panel
            .apply_persisted_recovery_progress(recovery_id, 6, 16)
            .expect("apply updated plan progress");
        panel.apply_recovery_load(
            session,
            load_generation,
            mutation_generation,
            Ok(stale_records),
            cx,
        );
    });
    h.panel.read_with(cx, |panel, _| {
        let record = panel
            .recovery_records
            .iter()
            .find(|record| record.id == recovery_id)
            .expect("updated recovery record");
        assert_eq!(record.reviewed_existing_bytes, 6);
        assert_eq!(record.transferred_bytes, 6);
        assert!(!panel.recovery_loading);
    });
    let persisted = store
        .load()
        .expect("load after plan progress race")
        .into_iter()
        .find(|record| record.id == recovery_id)
        .expect("persisted updated recovery record");
    assert_eq!(persisted.reviewed_existing_bytes, 6);
    assert_eq!(persisted.transferred_bytes, 6);
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
            None,
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

fn queue_entry_contains(inner: Bounds<gpui_kit::Pixels>, outer: Bounds<gpui_kit::Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.right() <= outer.right()
        && inner.origin.y >= outer.origin.y
        && inner.bottom() <= outer.bottom()
}

fn queue_entry_click(h: &Harness, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let button = window.find("toggle-file-transfer-queue");
        assert!(
            button.visible(),
            "the queue entry must be painted without tool scrolling"
        );
        window.click("toggle-file-transfer-queue", cx);
    })
    .checked("click the fixed production queue entry");
}

#[gpui_kit::test]
async fn queue_entry_completed_results_are_visible_and_return_preserves_browser_selection_and_drafts(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    for index in 0..6 {
        let bytes = format!("complete owned transfer {index}");
        let name = format!("queue-entry-{index}.txt");
        let source = h.source(&name, bytes.as_bytes());
        h.local_input(cx, &source);
        h.click(cx, "upload-file");
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.read(&format!("/{name}")), bytes.as_bytes());
    }
    h.seed("/a-selected.txt", b"selection only");
    let selected_local = h.source("a-selected.txt", b"local selection only");
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.browse_local(h.local.0.clone(), window, cx)
        });
    })
    .checked("explicitly load local browser for selection preservation");
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
    cx.update_window(h.window, |_, window, cx| {
        let local_index = h.panel.read_with(cx, |panel, _| {
            let listing = panel
                .local_browser
                .listing
                .as_ref()
                .checked_option("complete explicit local listing");
            // Local row IDs retain the unsorted native index. The first painted
            // row must be resolved from its original path, not assumed index 0.
            let index = listing
                .entries
                .iter()
                .position(|entry| entry.path == selected_local)
                .checked_option("the original local selection target");
            assert_eq!(
                crate::files::browser::local_indices(
                    &listing.entries,
                    panel.local_browser.show_hidden,
                    panel.local_browser.sort,
                )
                .first()
                .copied(),
                Some(index),
                "the intended target must be the first painted local entry"
            );
            index
        });
        window.render_frame(cx);
        let local_row = window.find(("local-entry", local_index));
        assert!(
            local_row.visible()
                && queue_entry_contains(local_row.bounds(), window.find("local-files").bounds()),
            "the actual first local data row must fit before the queue is opened"
        );
        window.click(("select-local-entry", local_index), cx);
        window.click(("select-remote-entry", 0_usize), cx);
        h.panel.update(cx, |panel, cx| {
            assert!(panel.local_browser.selection.contains(&selected_local));
            assert!(
                panel
                    .remote_selection
                    .contains(&"/a-selected.txt".to_owned())
            );
            panel.path.update(cx, |input, cx| {
                input.set_value("/unsubmitted remote draft", window, cx)
            });
            panel.local.update(cx, |input, cx| {
                input.set_value("unsubmitted local draft", window, cx)
            });
            panel
                .name
                .update(cx, |input, cx| input.set_value("未执行.txt", window, cx));
            panel
                .mode
                .update(cx, |input, cx| input.set_value("0600", window, cx));
        });
    })
    .checked("select real entries and retain unsubmitted drafts");
    let before = h.panel.read_with(cx, |panel, cx| {
        (
            panel
                .local_browser
                .selection
                .paths()
                .cloned()
                .collect::<Vec<_>>(),
            panel.remote_selection.paths().cloned().collect::<Vec<_>>(),
            panel.path.read(cx).value().to_string(),
            panel.local.read(cx).value().to_string(),
            panel.name.read(cx).value().to_string(),
            panel.mode.read(cx).value().to_string(),
        )
    });
    let first_id = h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 6);
        assert!(
            panel
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Completed)
        );
        panel.transfer_jobs[0].id
    });
    for (width, height) in [(900., 580.), (1440., 900.)] {
        for language in [Language::ZhCn, Language::En] {
            for theme in [
                keelshell_core::Theme::System,
                keelshell_core::Theme::Light,
                keelshell_core::Theme::Dark,
            ] {
                cx.update_window(h.window, |_, window, cx| {
                    window.resize(size(px(width), px(height)));
                    window.bounds_changed(cx);
                    i18n::set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                    window.render_frame(cx);
                    let area = window.find("files-layout-scene").bounds();
                    let summary = window.find("file-transfer-summary");
                    assert!(summary.visible() && queue_entry_contains(summary.bounds(), area));
                    assert!(queue_entry_contains(
                        window.find("toggle-file-transfer-queue").bounds(),
                        summary.bounds()
                    ));
                    let completed = window.find("file-transfer-summary-completed");
                    assert_eq!(
                        completed.label(),
                        Some(if language == Language::ZhCn {
                            "完成 6"
                        } else {
                            "Done 6"
                        })
                    );
                    for id in ["active", "completed", "failed", "cancelled", "uncertain"] {
                        let id =
                            gpui_kit::SharedString::from(format!("file-transfer-summary-{id}"));
                        let viewport = window.find("file-transfer-summary-counts").bounds();
                        let count = window.find(id.clone()).bounds();
                        window.scroll(
                            "file-transfer-summary-counts",
                            gpui_kit::ScrollDelta::Pixels(point(
                                viewport.center().x - count.center().x,
                                px(0.),
                            )),
                            cx,
                        );
                        let count = window.find(id);
                        assert!(
                            count.visible() && queue_entry_contains(count.bounds(), viewport),
                            "every real status count remains reachable in the compact strip"
                        );
                    }
                    assert_true_file_row(window);
                })
                .checked("default summary stays painted in both languages and all themes");
                queue_entry_click(&h, cx);
                cx.update_window(h.window, |_, window, cx| {
                    window.render_frame(cx);
                    assert!(h.panel.read(cx).queue_details_visible);
                    assert!(window.try_find("file-browsing-area").is_none());
                    let details = window.find("file-transfer-queue-details");
                    let row = window.find(gpui_kit::SharedString::from(format!("transfer-job-{first_id}")));
                    assert!(row.visible() && queue_entry_contains(row.bounds(), details.bounds()),
                        "queue entry must reveal an actual retained outcome without outer tool scrolling");
                    assert!(window.try_find("inspect-transfer-isolation").is_some());
                    assert!(window.try_find("transfer-parallelism-2").is_some());
                }).checked("entry opens actual queue results and retains existing controls");
                queue_entry_click(&h, cx);
                cx.update_window(h.window, |_, window, cx| {
                    window.render_frame(cx);
                    assert_true_file_row(window);
                })
                .checked("return restores the real browser view");
                assert_eq!(
                    h.panel.read_with(cx, |panel, cx| (
                        panel
                            .local_browser
                            .selection
                            .paths()
                            .cloned()
                            .collect::<Vec<_>>(),
                        panel.remote_selection.paths().cloned().collect::<Vec<_>>(),
                        panel.path.read(cx).value().to_string(),
                        panel.local.read(cx).value().to_string(),
                        panel.name.read(cx).value().to_string(),
                        panel.mode.read(cx).value().to_string(),
                    )),
                    before
                );
                assert_eq!(h.server.filesystem.atomic_writes_started(), 6);
            }
        }
    }
}

#[gpui_kit::test]
async fn queue_entry_keeps_pending_review_and_fixed_actions_without_dispatch_or_approval(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let bytes = b"only explicit confirmation can publish";
    let source = h.source("queue-review.txt", bytes);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    for language in [Language::ZhCn, Language::En] {
        for theme in [
            keelshell_core::Theme::System,
            keelshell_core::Theme::Light,
            keelshell_core::Theme::Dark,
        ] {
            cx.update_window(h.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
            })
            .checked("paint original unexecuted review");
            queue_entry_click(&h, cx);
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let area = window.find("files-layout-scene").bounds();
                let review = window.find("file-confirmation-message");
                assert!(review.visible() && queue_entry_contains(review.bounds(), area));
                for id in [
                    "expand-file-review",
                    "confirm-file-operation",
                    "cancel-file-operation",
                ] {
                    let button = window.find(id);
                    assert!(button.visible() && queue_entry_contains(button.bounds(), area));
                }
                h.panel.read_with(cx, |panel, _| {
                    assert!(
                        matches!(&panel.pending, Some((_, Operation::Upload(local, target)))
                        if local == &source && target == "/queue-review.txt")
                    );
                    assert!(panel.transfer_jobs.is_empty());
                });
            })
            .checked("queue surface retains the exact pending review and all fixed actions");
            assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
            queue_entry_click(&h, cx);
        }
    }
    queue_entry_click(&h, cx);
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("file-review-expanded").is_some());
        assert!(window.try_find("file-transfer-queue-details").is_none());
        assert!(window.find("confirm-file-operation").visible());
        assert!(window.find("cancel-file-operation").visible());
    })
    .checked("full review takes precedence over queue presentation");
    h.click(cx, "cancel-file-operation");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.missing("/queue-review.txt");
    queue_entry_click(&h, cx);
    h.click(cx, "upload-file");
    queue_entry_click(&h, cx);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/queue-review.txt"), bytes);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
}

#[gpui_kit::test]
async fn queue_entry_old_painted_owner_cannot_switch_a_replacement_session_surface(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find("toggle-file-transfer-queue").bounds().center();
        h.panel
            .update(cx, |panel, _| panel.session_token = uuid::Uuid::new_v4());
        for input in [
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
        assert!(
            !h.panel.read(cx).queue_details_visible,
            "an old frame cannot operate the new owner"
        );
        assert!(h.panel.read(cx).transfer_jobs.is_empty());
    })
    .checked("dispatch the old painted queue entry after its owner changed");
    queue_entry_click(&h, cx);
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.queue_details_visible)
    );
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
