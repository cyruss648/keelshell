//! Independent CLOSE boundary probes; separate from the frozen author tests.
use super::*;
use std::sync::atomic::AtomicBool;

const CONTENT: &[u8] = b"new bytes fully WRITE-acknowledged";

async fn held(predicate: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

async fn terminal(
    job: &mut keelshell_session::sftp::TransferHandle,
) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(7), async {
        while let Some(event) = job.recv().await {
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Uncertain { .. }
            ) {
                return Ok(event);
            }
        }
        Err::<_, Box<dyn Error>>("missing terminal outcome".into())
    })
    .await?
}

async fn stop(
    server: &mut Server,
    session: &SshSession,
    sftp: &keelshell_session::sftp::SftpSession,
    queue: keelshell_session::sftp::TransferQueue,
) -> Result<(), Box<dyn Error>> {
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    server.task.abort();
    let _ = (&mut server.task).await;
    Ok(())
}

#[tokio::test]
async fn reviewer_close_timeout_without_cancellation_keeps_destination_unknown()
-> Result<(), Box<dyn Error>> {
    for mode in 0..3 {
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.write("/timeout-close", b"original").await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        tokio::fs::write(&source, CONTENT).await?;
        let queue = sftp.clone().transfer_queue();
        let close = server.filesystem.hold_close(
            if mode == 2 {
                "/.timeout-close.keelshell-"
            } else {
                "/timeout-close"
            },
            mode == 2,
            true,
        )?;
        if mode == 1 {
            let mut job = queue
                .enqueue(TransferSpec::upload(&source, "/timeout-close"))
                .await?;
            held(|| close.entered() == 1).await?;
            let event = terminal(&mut job).await?;
            assert!(matches!(event, TransferEvent::Uncertain { bytes: 34, .. }));
        } else {
            let mut future = Box::pin(async {
                if mode == 2 {
                    sftp.write_atomic("/timeout-close", CONTENT).await
                } else {
                    sftp.write("/timeout-close", CONTENT).await
                }
            });
            tokio::select! {
                result = &mut future => return Err(format!("completed before actual CLOSE: {result:?}").into()),
                result = held(|| close.entered() == 1) => result?,
            }
            let result = tokio::time::timeout(Duration::from_secs(7), future).await?;
            assert!(matches!(result, Err(SessionError::MutationUncertain)));
        }
        assert_eq!(close.pending(), 1);
        assert!(!close.expired());
        let other_session = SshSession::connect(options(&server)).await?;
        let other = other_session.sftp().await?;
        assert!(matches!(
            other.write_atomic("/timeout-close", b"conflict").await,
            Err(SessionError::MutationQuarantined)
        ));
        let review = other
            .inspect_remote_mutation_quarantine("/timeout-close")
            .await?;
        let ids: Vec<_> = review.entries().iter().map(|e| e.reservation_id).collect();
        assert_eq!(ids.len(), if mode == 2 { 2 } else { 1 });
        assert_eq!(
            other.read("/timeout-close", 100).await?,
            if mode == 2 { b"original" } else { CONTENT }
        );
        if mode == 2 {
            other.write_atomic("/separate", b"still useful").await?;
        }
        close.release();
        held(|| close.pending() == 0).await?;
        let fresh = other
            .inspect_remote_mutation_quarantine("/timeout-close")
            .await?;
        assert_eq!(
            fresh
                .entries()
                .iter()
                .map(|e| e.reservation_id)
                .collect::<Vec<_>>(),
            ids
        );
        eprintln!(
            "independent CLOSE timeout: mode={mode}, actual pending=1, same target quarantined, IDs={} retained after late reply",
            ids.len()
        );
        other.close().await?;
        other_session.close().await?;
        stop(&mut server, &session, &sftp, queue).await?;
    }
    Ok(())
}

#[tokio::test]
async fn reviewer_revoked_before_close_first_poll_still_closes_owned_temp_and_retains_unknown()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    sftp.write("/revoke-before-close", b"original").await?;
    let baseline = sftp
        .read_regular_snapshot("/revoke-before-close", 100)
        .await?;
    let authorized = AtomicBool::new(true);
    let authority = || authorized.load(Ordering::Acquire);
    let writes = server
        .filesystem
        .hold_atomic_upload_after_first("/revoke-before-close")?;
    let close = server
        .filesystem
        .hold_close("/.revoke-before-close.keelshell-", true, true)?;
    let before = server.filesystem.writable_closes_started();
    let content = vec![0x37; 65536];
    let mut writing =
        Box::pin(sftp.write_regular_reviewed_authorized(&baseline, &content, &authority));
    tokio::select! {
        result = &mut writing => return Err(format!("writer ended before second WRITE: {result:?}").into()),
        result = held(|| writes.entered() == 1) => result?,
    }
    assert_eq!(close.entered(), 0);
    authorized.store(false, Ordering::Release);
    writes.release();
    let result = tokio::time::timeout(Duration::from_secs(7), writing).await?;
    assert!(matches!(result, Err(SessionError::MutationUncertain)));
    assert_eq!(
        close.entered(),
        1,
        "preflight rejection must leave handle for owned cleanup CLOSE"
    );
    assert_eq!(close.pending(), 1);
    assert!(!close.expired());
    assert_eq!(server.filesystem.writable_closes_started(), before + 1);
    assert!(matches!(
        sftp.write_atomic("/revoke-before-close", b"conflict").await,
        Err(SessionError::MutationQuarantined)
    ));
    let review = sftp
        .inspect_remote_mutation_quarantine("/revoke-before-close")
        .await?;
    assert_eq!(review.entries().len(), 2);
    assert_eq!(sftp.read("/revoke-before-close", 100).await?, b"original");
    sftp.write_atomic("/independent-revoke", b"permitted")
        .await?;
    let ids: Vec<_> = review.entries().iter().map(|e| e.reservation_id).collect();
    close.release();
    held(|| close.pending() == 0).await?;
    let fresh = sftp
        .inspect_remote_mutation_quarantine("/revoke-before-close")
        .await?;
    assert_eq!(
        fresh
            .entries()
            .iter()
            .map(|e| e.reservation_id)
            .collect::<Vec<_>>(),
        ids
    );
    eprintln!(
        "independent pre-CLOSE revocation: cleanup attempted exactly one writable CLOSE; two exact IDs retained; original final preserved; unrelated target permitted"
    );
    stop(&mut server, &session, &sftp, queue).await?;
    Ok(())
}

#[tokio::test]
async fn reviewer_status_refusal_upload_and_directory_close_are_known_failures()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let source = root.join("source");
    tokio::fs::write(&source, CONTENT).await?;
    let tree = root.join("tree");
    tokio::fs::create_dir(&tree).await?;
    tokio::fs::write(tree.join("child"), CONTENT).await?;
    let plan = sftp
        .plan_directory_transfer(TransferSpec::upload(&tree, "/refused-tree"))
        .await?;
    sftp.mkdir("/refused-resume").await?;
    sftp.write("/refused-resume/child", &CONTENT[..4]).await?;
    let resume = sftp
        .plan_directory_resume(TransferSpec::upload(&tree, "/refused-resume"))
        .await?;
    let before = server.filesystem.writable_closes_started();
    server.filesystem.reject_writable_closes(true);
    assert!(matches!(
        sftp.upload(&source, "/refused-upload").await,
        Err(SessionError::Sftp(_))
    ));
    assert!(matches!(
        sftp.upload_atomic(&source, "/refused-atomic").await,
        Err(SessionError::Sftp(_))
    ));
    let mut directory = queue.enqueue_directory(plan).await?;
    assert!(matches!(
        terminal(&mut directory).await?,
        TransferEvent::Failed { .. }
    ));
    let mut continuation = queue.enqueue_directory_resume(resume).await?;
    assert!(matches!(
        terminal(&mut continuation).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(server.filesystem.writable_closes_started(), before + 4);
    server.filesystem.reject_writable_closes(false);
    for path in [
        "/refused-upload",
        "/refused-atomic",
        "/refused-tree/child",
        "/refused-resume/child",
    ] {
        assert!(sftp.inspect_remote_mutation_quarantine(path).await.is_err());
        sftp.write_atomic(path, b"fresh reviewed request").await?;
        assert_eq!(sftp.read(path, 100).await?, b"fresh reviewed request");
    }
    eprintln!(
        "independent explicit CLOSE refusal: direct upload/atomic upload/directory upload/directory continuation Failed; four actual writable CLOSEs; no Unknown; fresh manual writes succeeded"
    );
    stop(&mut server, &session, &sftp, queue).await?;
    Ok(())
}
