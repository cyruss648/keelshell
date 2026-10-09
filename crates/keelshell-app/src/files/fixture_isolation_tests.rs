//! Independent fixtures still share the application's conservative local owner.
use super::*;

struct Contender {
    queued: std::sync::mpsc::Receiver<()>,
    admitted: std::sync::mpsc::Receiver<Arc<fixture_group::FixtureGroup>>,
    thread: std::thread::JoinHandle<()>,
}

impl Contender {
    fn start() -> Self {
        use std::{future::Future, task::Poll};
        let (queued, queued_receive) = std::sync::mpsc::channel();
        let (admitted, admitted_receive) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let runtime = Harness::runtime();
            runtime.block_on(async {
                let mut wait = Box::pin(fixture_group::FixtureGroup::wait());
                std::future::poll_fn(|cx| {
                    assert!(wait.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                queued
                    .send(())
                    .checked("report actual pending lifetime contender");
                admitted
                    .send(wait.await)
                    .checked("report actual released lifetime admission");
            });
        });
        Self {
            queued: queued_receive,
            admitted: admitted_receive,
            thread,
        }
    }

    fn pending(&self) {
        self.queued
            .recv_timeout(Duration::from_secs(5))
            .checked("bounded real-clock pending setup admission");
        assert!(matches!(
            self.admitted.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    fn finish(self) -> Arc<fixture_group::FixtureGroup> {
        // Old UI entities/actions have ended. This is the next fixture's setup,
        // using a real deadline, with no UI callback needed to release the owner.
        let deadline = Instant::now() + Duration::from_secs(12);
        let group = self
            .admitted
            .recv_timeout(Duration::from_secs(12))
            .checked("bounded real-clock next fixture setup");
        while !self.thread.is_finished() {
            assert!(
                Instant::now() < deadline,
                "setup admission thread did not finish"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        self.thread
            .join()
            .checked("join owned lifetime admission contender");
        group
    }
}

#[gpui_kit::test]
async fn retained_window_keeps_admission_after_normal_harness_drop(cx: &mut TestAppContext) {
    // Preserve the fresh non-author witness's real continuation, pause and
    // normal Harness drop. Only the next group's expected admission changes.
    let download = Harness::new(cx);
    download.idle(cx).await;
    let bytes = vec![0x72; 768 * 1024];
    download.seed("/lifetime-source.bin", &bytes);
    let destination = download.source("lifetime-partial.bin", &bytes[..4096]);
    download.local_input(cx, &destination);
    cx.update_window(download.window, |_, window, cx| {
        download.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/lifetime-source.bin", false))
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("download-file", cx);
    })
    .checked("review retained-window existing-local continuation");
    download.idle(cx).await;
    download.server.filesystem.set_transfer_read_delay(75);
    download.click(cx, "confirm-file-operation");
    cx.wait_for(download.window, Duration::from_secs(8), |_, cx| {
        download
            .panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|state| state.phase == TransferPhase::Running && state.transferred > 4096)
    })
    .await;
    download.click(cx, "pause-file-transfer");
    download.phase(cx, TransferPhase::Paused).await;
    let old_window = download.window;
    let old_panel = download.panel.clone();
    let weak_panel = old_panel.downgrade();
    let weak_group = Arc::downgrade(&download.mutation_group);
    drop(download);
    let contender = Contender::start();
    contender.pending();
    old_panel.update(cx, |panel, cx| panel.cancel_active(cx));
    cx.wait_for(old_window, Duration::from_secs(12), |_, cx| {
        !old_panel.read(cx).busy
    })
    .await;
    // Even a terminal worker cannot retire an actual retained file window.
    assert!(matches!(
        contender.admitted.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    cx.update_window(old_window, |_, window, _| window.remove_window())
        .checked("close owned retained window");
    drop(old_panel);
    let before_flush = (weak_panel.upgrade().is_some(), weak_group.strong_count());
    // Entity drops are deferred until the next normal App update flushes them.
    cx.update(|_| {});
    let after_flush = (weak_panel.upgrade().is_some(), weak_group.strong_count());
    eprintln!(
        "LOCAL_FIXTURE_PANEL_LIFETIME {}",
        serde_json::json!({"case":"retained_window", "before_flush_panel_upgrades":before_flush.0, "before_flush_group_owners":before_flush.1, "after_flush_panel_upgrades":after_flush.0, "after_flush_group_owners":after_flush.1})
    );
    assert!(!after_flush.0);
    cx.run_until_parked();
    let group = contender.finish();
    let sync = Harness::in_group(cx, mount, Harness::runtime(), group);
    sync.idle(cx).await;
    sync.source("lifetime-changed.txt", b"new!");
    sync.seed("/lifetime-changed.txt", b"old!");
    sync.local_input(cx, &sync.local.0);
    sync.click(cx, "compare-directories");
    sync.idle(cx).await;
    sync.click(cx, "plan-sync-to-remote");
    sync.idle(cx).await;
    sync.click(cx, "review-directory-sync");
    assert_eq!(sync.read("/lifetime-changed.txt"), b"old!");
    sync.click(cx, "confirm-file-operation");
    sync.idle(cx).await;
    assert_eq!(sync.server.filesystem.atomic_writes_started(), 1);
    assert_eq!(sync.read("/lifetime-changed.txt"), b"new!");
}

#[gpui_kit::test]
async fn removed_window_keeps_admission_until_real_worker_cleanup_finishes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("worker-changed.txt", b"new!");
    h.seed("/worker-changed.txt", b"old!");
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-sync-to-remote");
    h.idle(cx).await;
    let hold = h
        .server
        .filesystem
        .hold_close("/.worker-changed.txt.keelshell-", true, true)
        .checked("hold the actual staged writer CLOSE reply");
    h.click(cx, "review-directory-sync");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.pending() > 0)
        .await;
    let stop = h.panel.read_with(cx, |panel, _| {
        panel
            .operation_stop
            .as_ref()
            .checked_option("actual sync transport stop")
            .clone()
    });
    let completion = crate::terminal::transport_completion_for_test(&stop)
        .checked_option("actual sync worker completion");
    assert!(
        completion
            .lock()
            .checked("read actual unfinished worker")
            .is_none()
    );
    let weak_group = Arc::downgrade(&h.mutation_group);
    let Harness {
        window,
        panel,
        server,
        runtime,
        session,
        local,
        mutation_group,
    } = h;
    let weak_panel = panel.downgrade();
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("remove the real working file window");
    drop(panel);
    drop(mutation_group);
    let before_flush = (weak_panel.upgrade().is_some(), weak_group.strong_count());
    cx.update(|_| {});
    let after_flush = (weak_panel.upgrade().is_some(), weak_group.strong_count());
    eprintln!(
        "LOCAL_FIXTURE_PANEL_LIFETIME {}",
        serde_json::json!({"case":"removed_worker_window", "before_flush_panel_upgrades":before_flush.0, "before_flush_group_owners":before_flush.1, "after_flush_panel_upgrades":after_flush.0, "after_flush_group_owners":after_flush.1, "close_pending":hold.pending(), "worker_finished":completion.lock().checked("observe actual worker after flush").is_some()})
    );
    assert!(!after_flush.0);
    cx.run_until_parked();
    assert!(hold.pending() > 0 && !hold.expired());
    assert!(
        completion
            .lock()
            .checked("worker still owns actual CLOSE cleanup")
            .is_none()
    );
    let contender = Contender::start();
    contender.pending();
    assert!(hold.pending() > 0 && !hold.expired());
    assert!(
        completion
            .lock()
            .checked("pending contender cannot finish the worker")
            .is_none()
    );
    hold.release();
    let deadline = Instant::now() + Duration::from_secs(12);
    while completion
        .lock()
        .checked("observe real worker termination")
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "removed panel's real worker did not terminate"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let group = contender.finish();
    drop(group);
    assert!(!hold.expired());
    // These are exact original peer/runtime owners, kept alive for the CLOSE;
    // neither Server::Drop abort nor runtime teardown served as the drain.
    drop((server, session, local, runtime));
}

#[gpui_kit::test]
async fn held_existing_local_resume_excludes_other_fixture_sync_without_mutation(
    cx: &mut TestAppContext,
) {
    let sync = Harness::new(cx);
    sync.idle(cx).await;
    sync.source("changed.txt", b"new!");
    sync.source("local-only.txt", b"local");
    sync.seed("/changed.txt", b"old!");
    sync.seed("/remote-only.txt", b"keep");
    sync.local_input(cx, &sync.local.0);
    sync.click(cx, "compare-directories");
    sync.idle(cx).await;
    sync.click(cx, "plan-sync-to-remote");
    sync.idle(cx).await;

    let download = Harness::new_in(cx, &sync);
    download.idle(cx).await;
    assert_ne!(sync.local.0, download.local.0);
    let bytes = vec![0x6b; 144 * 1024];
    download.seed("/source.bin", &bytes);
    let destination = download.source("partial-download.bin", &bytes[..4096]);
    download.local_input(cx, &destination);
    cx.update_window(download.window, |_, window, cx| {
        download.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/source.bin", false));
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("download-file", cx);
    })
    .checked("review an existing local file in an independent SSH fixture");
    download.idle(cx).await;
    assert_eq!(
        std::fs::read(&destination).checked("review retained the original prefix"),
        bytes[..4096]
    );

    // Arm only after the read-only review. Execution has already admitted its
    // existing local writer when source revalidation reaches this real READ.
    let hold = download
        .server
        .filesystem
        .hold_transfer_reads_after_first()
        .checked("own the resumed download's real nonzero READ reply");
    download.click(cx, "confirm-file-operation");
    cx.wait_for(download.window, Duration::from_secs(5), |_, cx| {
        hold.entered() > 0
            && download
                .panel
                .read(cx)
                .transfer
                .as_ref()
                .is_some_and(|state| state.phase == TransferPhase::Running)
    })
    .await;

    sync.click(cx, "review-directory-sync");
    sync.click(cx, "confirm-file-operation");
    sync.idle(cx).await;
    sync.panel.read_with(cx, |panel, cx| {
        assert!(
            panel
                .status
                .render(cx)
                .contains("file target is owned by an active application mutation")
        );
        assert!(!panel.busy && panel.operation_id.is_none() && panel.pending.is_none());
    });
    assert_eq!(sync.read("/changed.txt"), b"old!");
    sync.missing("/local-only.txt");
    assert_eq!(sync.read("/remote-only.txt"), b"keep");
    assert_eq!(sync.server.filesystem.atomic_writes_started(), 0);
    assert_eq!(
        std::fs::read(&destination).checked("held validation has not appended"),
        bytes[..4096]
    );
    assert!(!hold.expired());
    hold.release();

    download.idle(cx).await;
    assert_eq!(
        std::fs::read(&destination).checked("completed exact local continuation"),
        bytes
    );
    download.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|state| state.phase),
            Some(TransferPhase::Completed)
        );
    });

    // The rejected review never retries itself. A new plan and a distinct
    // human confirmation remain required after the competing writer ends.
    sync.click(cx, "compare-directories");
    sync.idle(cx).await;
    sync.click(cx, "plan-sync-to-remote");
    sync.idle(cx).await;
    sync.click(cx, "review-directory-sync");
    assert_eq!(sync.read("/changed.txt"), b"old!");
    sync.click(cx, "confirm-file-operation");
    sync.idle(cx).await;
    assert_eq!(sync.read("/changed.txt"), b"new!");
    assert_eq!(sync.read("/local-only.txt"), b"local");
    assert_eq!(sync.read("/remote-only.txt"), b"keep");
    assert_eq!(sync.server.filesystem.atomic_writes_started(), 2);
}
