//! Actual TCP CLOSE acknowledgements, cancellation and descriptor ownership.
use super::*;
use keelshell_session::sftp::{SftpSession, TransferHandle, TransferQueue};

const CONTENT: &[u8] = b"new bytes fully WRITE-acknowledged";

async fn wait_for(predicate: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}
async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(7), async {
        while let Some(event) = handle.recv().await {
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Uncertain { .. }
            ) {
                return Ok(event);
            }
        }
        Err::<_, Box<dyn Error>>("missing terminal receipt".into())
    })
    .await?
}
async fn paused(handle: &mut TransferHandle) -> Result<u64, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = handle.recv().await {
            match event {
                TransferEvent::Paused { transferred, .. } => return Ok(transferred),
                TransferEvent::Completed { .. }
                | TransferEvent::Cancelled { .. }
                | TransferEvent::Failed { .. }
                | TransferEvent::Uncertain { .. } => {
                    return Err("ended before the acknowledged pause".into());
                }
                _ => {}
            }
        }
        Err::<_, Box<dyn Error>>("missing pause receipt".into())
    })
    .await?
}
fn pending(hold: &sftp_fixture::CloseHold) {
    assert_eq!(
        hold.entered(),
        1,
        "only the owned CLOSE may satisfy this barrier"
    );
    assert_eq!(
        hold.pending(),
        1,
        "actual server CLOSE handler still owns its reply"
    );
    assert!(
        !hold.expired(),
        "fallback cannot establish pending CLOSE evidence"
    );
}
async fn conflicting(sftp: &SftpSession, path: &str, unknown: bool) -> Result<(), Box<dyn Error>> {
    let result =
        tokio::time::timeout(Duration::from_secs(3), sftp.write_atomic(path, b"bypass")).await?;
    assert!(
        matches!(
            (&result, unknown),
            (Err(SessionError::MutationBusy), false)
                | (Err(SessionError::MutationQuarantined), true)
        ),
        "conflicting writer: {result:?}"
    );
    Ok(())
}
async fn stop(
    server: &mut Server,
    sftp: &SftpSession,
    session: &SshSession,
    queue: TransferQueue,
) -> Result<(), Box<dyn Error>> {
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    server.task.abort();
    let _ = (&mut server.task).await;
    Ok(())
}
fn temporary() -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn Error>> {
    let guard = tempfile::tempdir()?;
    let path = guard.path().canonicalize()?;
    Ok((guard, path))
}

#[tokio::test]
async fn writable_close_drop_and_queue_cancel_retain_unknown_across_connections()
-> Result<(), Box<dyn Error>> {
    for mode in 0..3 {
        let queued = mode == 1;
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let (_guard, local) = temporary()?;
        let source = local.join("source");
        tokio::fs::write(&source, CONTENT).await?;
        sftp.write("/close-target", b"old").await?;
        let queue = sftp.clone().transfer_queue();
        queue.set_parallelism(4)?;
        let hold = server.filesystem.hold_close("/close-target", false, true)?;
        let mut event = None;
        if queued {
            let mut handle = queue
                .enqueue(TransferSpec::upload(&source, "/close-target"))
                .await?;
            wait_for(|| hold.entered() == 1).await?;
            conflicting(&sftp, "/close-target", false).await?;
            handle.cancel();
            event = Some(terminal(&mut handle).await?);
        } else {
            let mut writing = Box::pin(async {
                if mode == 2 {
                    sftp.upload(&source, "/close-target").await.map(|_| ())
                } else {
                    sftp.write("/close-target", CONTENT).await
                }
            });
            tokio::select! {
                result = &mut writing => return Err(format!("writer ended before held CLOSE: {result:?}").into()),
                result = wait_for(|| hold.entered() == 1) => result?,
            }
            conflicting(&sftp, "/close-target", false).await?;
            drop(writing);
        }
        pending(&hold);
        conflicting(&sftp, "/close-target", true).await?;
        let review = sftp
            .inspect_remote_mutation_quarantine("/close-target")
            .await?;
        assert_eq!(review.entries().len(), 1);
        assert_eq!(review.entries()[0].destination, "/close-target");
        assert_eq!(sftp.read("/close-target", 100).await?, CONTENT);
        let other_session = SshSession::connect(options(&server)).await?;
        let other = other_session.sftp().await?;
        conflicting(&other, "/close-target", true).await?;
        // Legacy in-place writes already own the conservative remote inode
        // side. CLOSE must retain that same claim, without shrinking it.
        conflicting(&other, "/independent", true).await?;
        pending(&hold);
        let ids: Vec<_> = review
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect();
        hold.release();
        wait_for(|| hold.pending() == 0).await?;
        let late = sftp
            .inspect_remote_mutation_quarantine("/close-target")
            .await?;
        assert_eq!(
            late.entries()
                .iter()
                .map(|entry| entry.reservation_id)
                .collect::<Vec<_>>(),
            ids
        );
        other.close().await?;
        other_session.close().await?;
        stop(&mut server, &sftp, &session, queue).await?;
        if let Some(event) = event {
            assert!(
                matches!(event, TransferEvent::Uncertain { bytes, .. } if bytes == CONTENT.len() as u64),
                "{event:?}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn directory_upload_and_continuation_writable_child_close_quarantine_the_tree()
-> Result<(), Box<dyn Error>> {
    for resume in [false, true] {
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let (_guard, local) = temporary()?;
        let source = local.join("tree");
        tokio::fs::create_dir(&source).await?;
        tokio::fs::write(source.join("child"), CONTENT).await?;
        let queue = sftp.clone().transfer_queue();
        queue.set_parallelism(2)?;
        if resume {
            sftp.mkdir("/close-tree").await?;
            sftp.write("/close-tree/child", &CONTENT[..4]).await?;
        }
        let hold = server
            .filesystem
            .hold_close("/close-tree/child", false, true)?;
        let mut handle = if resume {
            let plan = sftp
                .plan_directory_resume(TransferSpec::upload(&source, "/close-tree"))
                .await?;
            queue.enqueue_directory_resume(plan).await?
        } else {
            let plan = sftp
                .plan_directory_transfer(TransferSpec::upload(&source, "/close-tree"))
                .await?;
            queue.enqueue_directory(plan).await?
        };
        wait_for(|| hold.entered() == 1).await?;
        conflicting(&sftp, "/close-tree/child", false).await?;
        handle.cancel();
        let event = terminal(&mut handle).await?;
        pending(&hold);
        conflicting(&sftp, "/close-tree/child", true).await?;
        conflicting(&sftp, "/close-tree/new-child", true).await?;
        assert_eq!(sftp.read("/close-tree/child", 100).await?, CONTENT);
        let review = sftp
            .inspect_remote_mutation_quarantine("/close-tree/child")
            .await?;
        assert_eq!(review.entries().len(), 1);
        assert_eq!(review.entries()[0].destination, "/close-tree");
        if resume {
            conflicting(&sftp, "/independent", true).await?;
        } else {
            sftp.write_atomic("/independent", b"allowed").await?;
        }
        pending(&hold);
        hold.release();
        stop(&mut server, &sftp, &session, queue).await?;
        assert!(
            matches!(event, TransferEvent::Uncertain { bytes, .. } if bytes == CONTENT.len() as u64),
            "{event:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn resume_writable_target_final_and_reopen_close_wait_for_status()
-> Result<(), Box<dyn Error>> {
    for reopen in [false, true] {
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let (_guard, local) = temporary()?;
        let source = local.join("source");
        let content = if reopen {
            vec![0x61; 262_151]
        } else {
            CONTENT.to_vec()
        };
        tokio::fs::write(&source, &content).await?;
        sftp.write("/resume-close", &content[..4]).await?;
        let plan = sftp
            .plan_file_resume(TransferSpec::upload(&source, "/resume-close"))
            .await?;
        let queue = sftp.clone().transfer_queue();
        let close = server.filesystem.hold_close("/resume-close", false, true)?;
        let write = if reopen {
            Some(
                server
                    .filesystem
                    .hold_transfer_writes_after_first("/resume-close")?,
            )
        } else {
            None
        };
        let mut handle = queue.enqueue_resume(plan).await?;
        let expected = if let Some(write) = write {
            wait_for(|| write.entered() == 1).await?;
            handle.pause();
            write.release();
            let bytes = paused(&mut handle).await?;
            assert!(bytes < content.len() as u64);
            assert_eq!(close.entered(), 0);
            handle.resume();
            bytes
        } else {
            content.len() as u64
        };
        wait_for(|| close.entered() == 1).await?;
        handle.cancel();
        let event = terminal(&mut handle).await?;
        pending(&close);
        conflicting(&sftp, "/resume-close", true).await?;
        assert_eq!(
            sftp.inspect_remote_mutation_quarantine("/resume-close")
                .await?
                .entries()
                .len(),
            1
        );
        // Existing in-place continuation conservatively reserves this side.
        conflicting(&sftp, "/independent", true).await?;
        pending(&close);
        close.release();
        stop(&mut server, &sftp, &session, queue).await?;
        assert!(
            matches!(event, TransferEvent::Uncertain { bytes, .. } if bytes == expected),
            "{event:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn atomic_temporary_close_cancel_preserves_final_and_both_unknown_ids()
-> Result<(), Box<dyn Error>> {
    for queued in [false, true] {
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let (_guard, local) = temporary()?;
        let source = local.join("source");
        tokio::fs::write(&source, CONTENT).await?;
        sftp.write("/atomic-close", b"original survives").await?;
        let queue = sftp.clone().transfer_queue();
        let count = server.filesystem.writable_closes_started();
        let hold = server
            .filesystem
            .hold_close("/.atomic-close.keelshell-", true, true)?;
        let mut event = None;
        if queued {
            let mut handle = queue
                .enqueue_atomic_upload(TransferSpec::upload(&source, "/atomic-close"))
                .await?;
            wait_for(|| hold.entered() == 1).await?;
            handle.cancel();
            event = Some(terminal(&mut handle).await?);
        } else {
            let mut writing = Box::pin(sftp.write_atomic("/atomic-close", CONTENT));
            tokio::select! {
                result = &mut writing => return Err(format!("atomic writer ended before CLOSE: {result:?}").into()),
                result = wait_for(|| hold.entered() == 1) => result?,
            }
            drop(writing);
        }
        pending(&hold);
        assert_eq!(
            server.filesystem.writable_closes_started(),
            count + 1,
            "cleanup must not retry an invalidated handle"
        );
        conflicting(&sftp, "/atomic-close", true).await?;
        let review = sftp
            .inspect_remote_mutation_quarantine("/atomic-close")
            .await?;
        assert_eq!(review.entries().len(), 2);
        assert!(
            review
                .entries()
                .iter()
                .any(|entry| entry.destination == "/atomic-close")
        );
        assert!(
            review
                .entries()
                .iter()
                .any(|entry| entry.destination.starts_with("/.atomic-close.keelshell-"))
        );
        assert_eq!(sftp.read("/atomic-close", 100).await?, b"original survives");
        sftp.write_atomic("/independent", b"allowed").await?;
        pending(&hold);
        let ids: Vec<_> = review
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect();
        hold.release();
        wait_for(|| hold.pending() == 0).await?;
        let late = sftp
            .inspect_remote_mutation_quarantine("/atomic-close")
            .await?;
        assert_eq!(
            late.entries()
                .iter()
                .map(|entry| entry.reservation_id)
                .collect::<Vec<_>>(),
            ids
        );
        stop(&mut server, &sftp, &session, queue).await?;
        if let Some(event) = event {
            assert!(
                matches!(event, TransferEvent::Uncertain { bytes, .. } if bytes == CONTENT.len() as u64),
                "{event:?}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn rejected_write_cleanup_close_without_status_remains_unknown() -> Result<(), Box<dyn Error>>
{
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    sftp.write("/cleanup-close", b"original survives").await?;
    server.filesystem.set_atomic_write_failure(true);
    let hold = server
        .filesystem
        .hold_close("/.cleanup-close.keelshell-", true, true)?;
    let content = vec![0x77; 65_537];
    let mut writing = Box::pin(sftp.write_atomic("/cleanup-close", &content));
    tokio::select! {
        result = &mut writing => return Err(format!("cleanup ended before CLOSE barrier: {result:?}").into()),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    conflicting(&sftp, "/cleanup-close", false).await?;
    let result = tokio::time::timeout(Duration::from_secs(5), &mut writing).await?;
    drop(writing);
    assert!(
        matches!(result, Err(SessionError::MutationUncertain)),
        "{result:?}"
    );
    pending(&hold);
    conflicting(&sftp, "/cleanup-close", true).await?;
    let review = sftp
        .inspect_remote_mutation_quarantine("/cleanup-close")
        .await?;
    assert_eq!(review.entries().len(), 2);
    assert_eq!(
        sftp.read("/cleanup-close", 100).await?,
        b"original survives"
    );
    server.filesystem.set_atomic_write_failure(false);
    sftp.write_atomic("/independent", b"allowed").await?;
    pending(&hold);
    hold.release();
    let late = sftp
        .inspect_remote_mutation_quarantine("/cleanup-close")
        .await?;
    assert_eq!(
        late.entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>(),
        review
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>()
    );
    stop(&mut server, &sftp, &session, queue).await?;
    Ok(())
}

#[tokio::test]
async fn explicit_writable_close_status_refusal_is_known_and_new_review_may_write()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::write(&source, CONTENT).await?;
    sftp.write("/status-resume", &CONTENT[..4]).await?;
    let plan = sftp
        .plan_file_resume(TransferSpec::upload(&source, "/status-resume"))
        .await?;
    server.filesystem.reject_writable_closes(true);
    assert!(matches!(
        sftp.write("/status-direct", CONTENT).await,
        Err(SessionError::Sftp(_))
    ));
    assert!(matches!(
        sftp.write_atomic("/status-atomic", CONTENT).await,
        Err(SessionError::Sftp(_))
    ));
    let mut upload = queue
        .enqueue(TransferSpec::upload(&source, "/status-queue"))
        .await?;
    assert!(matches!(
        terminal(&mut upload).await?,
        TransferEvent::Failed { .. }
    ));
    let mut resume = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut resume).await?,
        TransferEvent::Failed { .. }
    ));
    server.filesystem.reject_writable_closes(false);
    for path in [
        "/status-direct",
        "/status-atomic",
        "/status-queue",
        "/status-resume",
    ] {
        assert!(sftp.inspect_remote_mutation_quarantine(path).await.is_err());
        sftp.write_atomic(path, b"fresh manual request").await?;
        assert_eq!(sftp.read(path, 100).await?, b"fresh manual request");
    }
    stop(&mut server, &sftp, &session, queue).await?;
    Ok(())
}

#[tokio::test]
async fn acknowledged_pause_cancel_before_close_sends_no_close_and_no_unknown()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::write(&source, vec![0x72; 262_151]).await?;
    let close = server.filesystem.hold_close("/pre-close", false, true)?;
    let write = server
        .filesystem
        .hold_transfer_writes_after_first("/pre-close")?;
    let mut handle = queue
        .enqueue(TransferSpec::upload(&source, "/pre-close"))
        .await?;
    wait_for(|| write.entered() == 1).await?;
    handle.pause();
    write.release();
    let bytes = paused(&mut handle).await?;
    assert_eq!(close.entered(), 0);
    assert_eq!(server.filesystem.writable_closes_started(), 0);
    handle.cancel();
    let event = terminal(&mut handle).await?;
    assert!(matches!(event, TransferEvent::Cancelled { bytes: observed, .. } if observed == bytes));
    assert!(
        sftp.inspect_remote_mutation_quarantine("/pre-close")
            .await
            .is_err()
    );
    assert_eq!(close.entered(), 0);
    close.release();
    sftp.write_atomic("/pre-close", b"fresh manual request")
        .await?;
    stop(&mut server, &sftp, &session, queue).await?;
    Ok(())
}

#[tokio::test]
async fn readonly_source_and_scan_close_drop_do_not_quarantine_remote_writes()
-> Result<(), Box<dyn Error>> {
    for scan in [false, true] {
        let mut server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let queue = sftp.clone().transfer_queue();
        let (_guard, local) = temporary()?;
        let target = local.join("target");
        sftp.write("/readonly-close", CONTENT).await?;
        if scan {
            tokio::fs::write(&target, &CONTENT[..4]).await?;
        }
        let hold = server
            .filesystem
            .hold_close("/readonly-close", false, false)?;
        let event = if scan {
            let mut planning =
                Box::pin(sftp.plan_file_resume(TransferSpec::download("/readonly-close", &target)));
            tokio::select! {
                result = &mut planning => return Err(format!("scan ended before CLOSE barrier: {result:?}").into()),
                result = wait_for(|| hold.entered() == 1) => result?,
            }
            drop(planning);
            None
        } else {
            let mut handle = queue
                .enqueue(TransferSpec::download("/readonly-close", &target))
                .await?;
            wait_for(|| hold.entered() == 1).await?;
            handle.cancel();
            Some(terminal(&mut handle).await?)
        };
        pending(&hold);
        assert!(
            sftp.inspect_remote_mutation_quarantine("/readonly-close")
                .await
                .is_err()
        );
        assert!(
            sftp.inspect_local_mutation_quarantine(&target)
                .await
                .is_err()
        );
        sftp.write_atomic("/readonly-close", b"allowed after read-only cancellation")
            .await?;
        pending(&hold);
        hold.release();
        stop(&mut server, &sftp, &session, queue).await?;
        if let Some(event) = event {
            assert!(
                matches!(event, TransferEvent::Cancelled { bytes, .. } if bytes == CONTENT.len() as u64),
                "{event:?}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn acknowledged_atomic_close_releases_only_after_status_and_distinct_target_overlaps()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    sftp.write("/acknowledged-close", b"original").await?;
    let hold = server
        .filesystem
        .hold_close("/.acknowledged-close.keelshell-", true, true)?;
    let mut writing = Box::pin(sftp.write_atomic("/acknowledged-close", CONTENT));
    tokio::select! {
        result = &mut writing => return Err(format!("writer ended before CLOSE: {result:?}").into()),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    conflicting(&sftp, "/acknowledged-close", false).await?;
    sftp.write_atomic("/independent", b"overlaps pending CLOSE")
        .await?;
    pending(&hold);
    assert_eq!(sftp.read("/acknowledged-close", 100).await?, b"original");
    hold.release();
    tokio::time::timeout(Duration::from_secs(5), &mut writing).await??;
    drop(writing);
    wait_for(|| hold.pending() == 0).await?;
    assert_eq!(sftp.read("/acknowledged-close", 100).await?, CONTENT);
    assert!(
        sftp.inspect_remote_mutation_quarantine("/acknowledged-close")
            .await
            .is_err()
    );
    sftp.write_atomic("/acknowledged-close", b"fresh manual request")
        .await?;
    stop(&mut server, &sftp, &session, queue).await?;
    Ok(())
}

#[tokio::test]
async fn revoked_reviewed_writer_dropped_during_close_retains_exact_final_and_temp_ids()
-> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    sftp.write("/authorized-close", b"original").await?;
    let baseline = sftp.read_regular_snapshot("/authorized-close", 100).await?;
    let authorized = std::sync::atomic::AtomicBool::new(true);
    let authority = || authorized.load(Ordering::Acquire);
    let hold = server
        .filesystem
        .hold_close("/.authorized-close.keelshell-", true, true)?;
    let mut writing =
        Box::pin(sftp.write_regular_reviewed_authorized(&baseline, CONTENT, &authority));
    tokio::select! {
        result = &mut writing => return Err(format!("authorized writer ended before CLOSE: {result:?}").into()),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    authorized.store(false, Ordering::Release);
    // The caller observes revocation and retires its pending future; dropping
    // cannot turn WRITE progress into proof of this writable CLOSE completion.
    drop(writing);
    pending(&hold);
    conflicting(&sftp, "/authorized-close", true).await?;
    let review = sftp
        .inspect_remote_mutation_quarantine("/authorized-close")
        .await?;
    assert_eq!(review.entries().len(), 2);
    let fresh_authority = || true;
    assert!(matches!(
        sftp.write_regular_reviewed_authorized(&baseline, b"ordinary approval", &fresh_authority)
            .await,
        Err(SessionError::MutationQuarantined)
    ));
    assert_eq!(sftp.read("/authorized-close", 100).await?, b"original");
    sftp.write_atomic("/independent", b"allowed").await?;
    pending(&hold);
    hold.release();
    wait_for(|| hold.pending() == 0).await?;
    let late = sftp
        .inspect_remote_mutation_quarantine("/authorized-close")
        .await?;
    assert_eq!(
        late.entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>(),
        review
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>()
    );
    stop(&mut server, &sftp, &session, queue).await?;
    Ok(())
}
