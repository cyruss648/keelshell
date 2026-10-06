//! Actual native MCP file approval cannot release another transfer's risk lock.
use super::*;
use keelshell_session::sftp::{TransferEvent, TransferSpec};
use std::sync::atomic::AtomicBool;

fn approve(h: &Harness, id: uuid::Uuid, cx: &mut TestAppContext) {
    open_review(h, id, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-file-review-approve", cx)
    })
    .checked("actual human MCP file approval");
}
#[gpui_kit::test]
async fn mcp_native_file_approval_obeys_active_and_unknown_transfer_isolation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let session = h
        .fixture
        .workspace
        .read_with(cx, |view, _| {
            view.remote_sessions
                .get(&h.panes[1].terminal.entity_id())
                .cloned()
        })
        .checked_option("same concrete authenticated file session");
    let root = tempfile::tempdir().checked("isolated transfer source");
    let source = root.path().join("source");
    std::fs::write(&source, vec![0x58; 128 * 1024]).checked("write isolated source");
    let sftp = Arc::new(
        h.runtime
            .block_on(session.sftp())
            .checked("owned queue SFTP"),
    );
    let queue = h.runtime.block_on(async { sftp.clone().transfer_queue() });
    queue
        .set_parallelism(2)
        .checked("spare actual worker capacity");
    let hold = h
        .files
        .filesystem
        .hold_atomic_upload_after_first("/approved/中文.txt")
        .checked("hold real queued WRITE");
    let spec = TransferSpec::upload(&source, "/approved/中文.txt");
    let mut job = h
        .runtime
        .block_on(queue.enqueue_atomic_upload(spec.clone()))
        .checked("explicit reviewed fixture transfer");
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        hold.entered() == 1
    })
    .await;
    let before = h.files.filesystem.transfer_writes_started();
    let active = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "must remain unsent",
        cx,
    )
    .await;
    approve(&h, active, cx);
    wait_state(&h, active, ActionState::Failed, cx).await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    assert_eq!(h.files.filesystem.transfer_writes_started(), before);
    let independent = propose_file(
        &h,
        target,
        "/approved/large",
        &"x".repeat(1024),
        "independent MCP write",
        cx,
    )
    .await;
    approve(&h, independent, cx);
    wait_state(&h, independent, ActionState::Succeeded, cx).await;
    assert_eq!(bytes(&h, "/approved/large", cx), b"independent MCP write");
    job.cancel();
    let terminal = h.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(7), async {
            while let Some(event) = job.recv().await {
                if matches!(
                    event,
                    TransferEvent::Completed { .. }
                        | TransferEvent::Cancelled { .. }
                        | TransferEvent::Failed { .. }
                        | TransferEvent::Uncertain { .. }
                ) {
                    return event;
                }
            }
            panic!("owned transfer ended without terminal result");
        })
        .await
        .checked("bounded actual transfer terminal")
    });
    assert!(matches!(
        terminal,
        TransferEvent::Uncertain { bytes: 32768, .. }
    ));
    let before = h.files.filesystem.transfer_writes_started();
    let unknown = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "ordinary approval cannot consent to risk",
        cx,
    )
    .await;
    approve(&h, unknown, cx);
    wait_state(&h, unknown, ActionState::Failed, cx).await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    assert_eq!(h.files.filesystem.transfer_writes_started(), before);
    assert!(!hold.expired());
    hold.release();
    h.runtime.block_on(async {
        queue.close().await.checked("drain original queue");
        sftp.close()
            .await
            .checked("drain exact owned staging cleanup");
    });
    h.runtime.block_on(async {
        let inspector = session
            .sftp()
            .await
            .checked("readonly exact risk inspection");
        let review = inspector
            .inspect_transfer_quarantine(&spec)
            .await
            .checked("full associated quarantine records");
        assert_eq!(review.entries().len(), 2);
        assert!(
            review
                .entries()
                .iter()
                .any(|entry| entry.destination == "/approved/中文.txt")
        );
        inspector
            .acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
            .await
            .checked("separate explicit risk acknowledgement");
        inspector
            .close()
            .await
            .checked("close inspection without replay");
    });
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    let fresh = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "fresh reviewed replacement",
        cx,
    )
    .await;
    assert_eq!(
        bytes(&h, "/approved/中文.txt", cx),
        "受控中文\n".as_bytes(),
        "new proposal alone does not write"
    );
    approve(&h, fresh, cx);
    wait_state(&h, fresh, ActionState::Succeeded, cx).await;
    assert_eq!(
        bytes(&h, "/approved/中文.txt", cx),
        b"fresh reviewed replacement"
    );
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(active)),
        Some(ActionState::Failed)
    ));
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(unknown)),
        Some(ActionState::Failed)
    ));
}
