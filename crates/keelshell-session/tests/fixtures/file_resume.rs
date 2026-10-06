//! Real TCP SFTP continuation and acknowledged control regression coverage.
use super::*;
use keelshell_session::sftp::TransferHandle;

async fn next(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(8), handle.recv())
        .await?
        .ok_or_else(|| "missing transfer event".into())
}
async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    loop {
        let event = next(handle).await?;
        if matches!(
            event,
            TransferEvent::Completed { .. }
                | TransferEvent::Cancelled { .. }
                | TransferEvent::Failed { .. }
        ) {
            return Ok(event);
        }
    }
}
async fn paused(handle: &mut TransferHandle) -> Result<u64, Box<dyn Error>> {
    loop {
        match next(handle).await? {
            TransferEvent::Paused { transferred, .. } => return Ok(transferred),
            TransferEvent::Completed { .. }
            | TransferEvent::Failed { .. }
            | TransferEvent::Cancelled { .. } => {
                return Err("transfer ended before pause acknowledgement".into());
            }
            _ => {}
        }
    }
}
async fn pause_after_progress(handle: &mut TransferHandle) -> Result<u64, Box<dyn Error>> {
    loop {
        match next(handle).await? {
            TransferEvent::Progress { transferred, .. } if transferred > 0 => {
                handle.pause();
                break;
            }
            TransferEvent::Completed { .. } | TransferEvent::Failed { .. } => {
                return Err("transfer ended before progress".into());
            }
            _ => {}
        }
    }
    paused(handle).await
}
fn bytes() -> Vec<u8> {
    (0..524_311).map(|i| (i % 251) as u8).collect()
}
fn temp() -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn Error>> {
    let guard = tempfile::tempdir()?;
    let path = guard.path().canonicalize()?;
    Ok((guard, path))
}

#[tokio::test]
async fn file_resume_roundtrip_reconnect_and_full_equal_length_verification()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let target = local.join("target");
    let content = bytes();
    tokio::fs::write(&source, &content).await?;
    sftp.write("/partial", &content[..70_013]).await?;
    let old_plan = sftp
        .plan_file_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    session.close().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let mut wrong_connection = queue.enqueue_resume(old_plan).await?;
    assert!(matches!(
        terminal(&mut wrong_connection).await?,
        TransferEvent::Failed { .. }
    ));
    let plan = sftp
        .plan_file_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    assert_eq!(plan.existing_bytes(), 70_013);
    let mut upload = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut upload).await?,
        TransferEvent::Completed { bytes: 524_311, .. }
    ));
    tokio::fs::write(&target, &content[..31_337]).await?;
    let plan = sftp
        .plan_file_resume(TransferSpec::download("/partial", &target))
        .await?;
    let mut download = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut download).await?,
        TransferEvent::Completed { bytes: 524_311, .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, content);
    let plan = sftp
        .plan_file_resume(TransferSpec::download("/partial", &target))
        .await?;
    assert_eq!(plan.existing_bytes(), plan.bytes());
    let mut complete = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut complete).await?,
        TransferEvent::Completed { bytes: 524_311, .. }
    ));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_resume_prefix_size_and_nonregular_files_never_change_destination()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let (_guard, local) = temp()?;
    let source = local.join("source");
    tokio::fs::write(&source, b"correct source").await?;
    for (path, value) in [
        ("/wrong", &b"WRONG"[..]),
        ("/equal", &b"invalid source"[..]),
        ("/long", &b"correct source extra"[..]),
    ] {
        sftp.write(path, value).await?;
        assert!(
            sftp.plan_file_resume(TransferSpec::upload(&source, path))
                .await
                .is_err()
        );
        assert_eq!(sftp.read(path, 64).await?, value);
    }
    server.filesystem.insert_symlink("/link")?;
    assert!(
        sftp.plan_file_resume(TransferSpec::upload(&source, "/link"))
            .await
            .is_err()
    );
    sftp.mkdir("/directory").await?;
    assert!(
        sftp.plan_file_resume(TransferSpec::upload(&source, "/directory"))
            .await
            .is_err()
    );
    sftp.write("/source", b"correct source").await?;
    let destination = local.join("destination");
    tokio::fs::write(&destination, b"BAD").await?;
    assert!(
        sftp.plan_file_resume(TransferSpec::download("/source", &destination))
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read(&destination).await?, b"BAD");
    #[cfg(unix)]
    {
        let link = local.join("link");
        std::os::unix::fs::symlink(&source, &link)?;
        assert!(
            sftp.plan_file_resume(TransferSpec::download("/source", &link))
                .await
                .is_err()
        );
    }
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_source_and_destination_changes_fail_before_any_write()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let target = local.join("target");
    sftp.write("/source", b"prefix original").await?;
    tokio::fs::write(&target, b"prefix").await?;
    let plan = sftp
        .plan_file_resume(TransferSpec::download("/source", &target))
        .await?;
    sftp.write("/source", b"prefix CHANGED!").await?;
    let mut changed = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut changed).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, b"prefix");
    let plan = sftp
        .plan_file_resume(TransferSpec::download("/source", &target))
        .await?;
    tokio::fs::write(&target, b"prefix C").await?;
    let mut changed = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut changed).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, b"prefix C");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn acknowledged_pause_has_no_writes_and_does_not_consume_active_timeout()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut config = options(&server);
    config.timeout = Duration::from_millis(900);
    let session = SshSession::connect(config).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let content = bytes();
    tokio::fs::write(&source, &content).await?;
    server.filesystem.set_transfer_write_delay(20);
    let mut transfer = queue
        .enqueue(TransferSpec::upload(&source, "/paused"))
        .await?;
    let acknowledged = pause_after_progress(&mut transfer).await?;
    assert!(acknowledged > 0 && acknowledged < content.len() as u64);
    let at_pause = sftp.read("/paused", 1_000_000).await?;
    assert_eq!(at_pause.len() as u64, acknowledged);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(sftp.read("/paused", 1_000_000).await?, at_pause);
    transfer.resume();
    let mut saw_resume = false;
    loop {
        match next(&mut transfer).await? {
            TransferEvent::Resumed { transferred, .. } => {
                assert_eq!(transferred, acknowledged);
                saw_resume = true;
            }
            TransferEvent::Completed { bytes, .. } => {
                assert_eq!(bytes, content.len() as u64);
                break;
            }
            TransferEvent::Failed { error, .. } => return Err(error.into()),
            _ => {}
        }
    }
    assert!(saw_resume);
    assert_eq!(sftp.read("/paused", 1_000_000).await?, content);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn cancelling_or_dropping_paused_handles_releases_fifo_without_more_writes()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let source = local.join("source");
    tokio::fs::write(&source, bytes()).await?;
    server.filesystem.set_transfer_write_delay(20);
    let mut cancelled = queue
        .enqueue(TransferSpec::upload(&source, "/cancel"))
        .await?;
    let acknowledged = pause_after_progress(&mut cancelled).await?;
    let mut dropped = queue
        .enqueue(TransferSpec::upload(&source, "/drop"))
        .await?;
    dropped.pause();
    cancelled.cancel();
    assert_eq!(
        terminal(&mut cancelled).await?,
        TransferEvent::Cancelled {
            id: cancelled.id(),
            bytes: acknowledged
        }
    );
    assert_eq!(paused(&mut dropped).await?, 0);
    drop(dropped);
    let mut next_job = queue
        .enqueue(TransferSpec::upload(&source, "/next"))
        .await?;
    assert!(matches!(
        terminal(&mut next_job).await?,
        TransferEvent::Completed { .. }
    ));
    assert_eq!(
        sftp.read("/cancel", 1_000_000).await?.len() as u64,
        acknowledged
    );
    assert!(sftp.read("/drop", 1_000_000).await.is_err());
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn resumed_file_revalidates_source_and_target_after_acknowledged_pause()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let content = bytes();
    tokio::fs::write(&source, &content).await?;
    server.filesystem.set_transfer_write_delay(20);
    for change_source in [true, false] {
        tokio::fs::write(&source, &content).await?;
        sftp.write("/resume", &content[..31_337]).await?;
        let plan = sftp
            .plan_file_resume(TransferSpec::upload(&source, "/resume"))
            .await?;
        let mut handle = queue.enqueue_resume(plan).await?;
        pause_after_progress(&mut handle).await?;
        if change_source {
            let mut altered = content.clone();
            altered[400_000] ^= 1;
            tokio::fs::write(&source, altered).await?;
        } else {
            assert!(matches!(
                sftp.write("/resume", b"other process content").await,
                Err(SessionError::MutationBusy)
            ));
            server
                .filesystem
                .replace_external_file("/resume", b"other process content")?;
        }
        let unchanged = sftp.read("/resume", 1_000_000).await?;
        handle.resume();
        assert!(matches!(
            terminal(&mut handle).await?,
            TransferEvent::Failed { .. }
        ));
        assert_eq!(sftp.read("/resume", 1_000_000).await?, unchanged);
    }
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn resume_accepts_short_sftp_reads_without_skipping_prefix_bytes()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let target = local.join("target");
    let content = bytes();
    tokio::fs::write(&source, &content).await?;
    sftp.write("/partial", &content[..17_031]).await?;
    server.filesystem.set_transfer_read_limit(7_001);
    let plan = sftp
        .plan_file_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    let mut upload = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut upload).await?,
        TransferEvent::Completed { bytes: 524_311, .. }
    ));
    tokio::fs::write(&target, &content[..16_399]).await?;
    let plan = sftp
        .plan_file_resume(TransferSpec::download("/partial", &target))
        .await?;
    let mut download = queue.enqueue_resume(plan).await?;
    assert!(matches!(
        terminal(&mut download).await?,
        TransferEvent::Completed { bytes: 524_311, .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, content);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn dropping_consumer_during_pending_write_releases_queue_and_preserves_ssh()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let next_source = local.join("next");
    tokio::fs::write(&source, bytes()).await?;
    tokio::fs::write(&next_source, b"next job").await?;
    server.filesystem.set_transfer_write_delay(150);
    let abandoned = queue
        .enqueue(TransferSpec::upload(&source, "/abandoned"))
        .await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.filesystem.transfer_writes_started() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    drop(abandoned);
    let mut next_job = queue
        .enqueue(TransferSpec::upload(&next_source, "/next-job"))
        .await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), terminal(&mut next_job)).await??,
        TransferEvent::Completed { bytes: 8, .. }
    ));
    assert_eq!(sftp.read("/next-job", 64).await?, b"next job");
    // An already submitted remote WRITE may finish, but no second chunk is sent.
    assert!(sftp.read("/abandoned", 1_000_000).await?.len() <= 64 * 1024);
    assert_eq!(
        session.exec("echo survives").await?.stdout,
        b"echo survives"
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn malformed_non_eof_or_oversized_reads_reject_review_without_changing_output()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temp()?;
    let source = local.join("source");
    let target = local.join("target");
    let content = bytes();
    tokio::fs::write(&source, &content).await?;
    tokio::fs::write(&target, &content[..731]).await?;
    sftp.write("/source", &content).await?;
    sftp.write("/target", &content[..791]).await?;
    for mode in [1, 2] {
        server.filesystem.set_invalid_transfer_read(mode);
        assert!(
            sftp.plan_file_resume(TransferSpec::upload(&source, "/target"))
                .await
                .is_err()
        );
        assert!(
            sftp.plan_file_resume(TransferSpec::download("/source", &target))
                .await
                .is_err()
        );
        server.filesystem.set_invalid_transfer_read(0);
        assert_eq!(sftp.read("/target", 1024).await?, content[..791]);
        assert_eq!(tokio::fs::read(&target).await?, content[..731]);
    }
    session.close().await?;
    Ok(())
}
