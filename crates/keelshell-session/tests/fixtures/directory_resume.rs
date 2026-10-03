//! Resume complete partial trees through real loopback SFTP packets.

use super::*;
use keelshell_session::sftp::TransferHandle;
use std::path::PathBuf;

async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = handle.recv().await {
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Failed { .. }
            ) {
                return Ok(event);
            }
        }
        Err("resume event stream closed without a terminal event".into())
    })
    .await?
}

fn temporary() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
    let guard = tempfile::tempdir()?;
    let path = guard.path().canonicalize()?;
    Ok((guard, path))
}

fn payload() -> Vec<u8> {
    (0..196_613).map(|index| (index % 251) as u8).collect()
}

async fn pause_after_progress(
    handle: &mut TransferHandle,
    minimum: u64,
) -> Result<u64, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = handle.recv().await {
            if matches!(event, TransferEvent::Progress { transferred, .. } if transferred >= minimum)
            {
                handle.pause();
                break;
            }
            if matches!(event, TransferEvent::Completed { .. } | TransferEvent::Failed { .. }) {
                return Err("transfer finished before the requested pause point".into());
            }
        }
        while let Some(event) = handle.recv().await {
            if let TransferEvent::Paused { transferred, .. } = event {
                return Ok(transferred);
            }
            if matches!(event, TransferEvent::Completed { .. } | TransferEvent::Failed { .. }) {
                return Err("transfer finished before pause acknowledgement".into());
            }
        }
        Err::<u64, Box<dyn Error>>("pause event stream closed".into())
    })
    .await?
}

#[tokio::test]
async fn upload_resume_preserves_verified_files_and_finishes_missing_tree_entries()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir_all(source.join("nested/empty")).await?;
    let content = payload();
    tokio::fs::write(source.join("nested/payload.bin"), &content).await?;
    tokio::fs::write(source.join("complete"), b"already complete").await?;
    tokio::fs::write(source.join("missing"), b"new bytes").await?;
    tokio::fs::write(source.join("zero"), []).await?;
    sftp.mkdir("/partial").await?;
    sftp.mkdir("/partial/nested").await?;
    sftp.write("/partial/nested/payload.bin", &content[..65_539])
        .await?;
    sftp.write("/partial/complete", b"already complete").await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    assert_eq!(plan.files(), 4);
    assert_eq!(plan.directories(), 3);
    assert_eq!(plan.bytes(), content.len() as u64 + 25);
    assert_eq!(plan.existing_bytes(), 65_539 + 16);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    assert_eq!(
        terminal(&mut transfer).await?,
        TransferEvent::Completed {
            id: transfer.id(),
            bytes: content.len() as u64 + 25,
        }
    );
    assert_eq!(
        sftp.read("/partial/nested/payload.bin", 256_000).await?,
        content
    );
    assert_eq!(
        sftp.read("/partial/complete", 64).await?,
        b"already complete"
    );
    assert_eq!(sftp.read("/partial/missing", 64).await?, b"new bytes");
    assert!(sftp.read("/partial/zero", 64).await?.is_empty());
    assert!(sftp.list("/partial/nested/empty").await?.is_empty());
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn download_resume_verifies_partial_bytes_and_creates_missing_empty_directories()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let target = local.join("partial");
    tokio::fs::create_dir(&target).await?;
    let content = payload();
    sftp.mkdir("/source").await?;
    sftp.mkdir("/source/nested").await?;
    sftp.mkdir("/source/nested/empty").await?;
    sftp.write("/source/data", &content).await?;
    sftp.write("/source/nested/zero", b"").await?;
    tokio::fs::write(target.join("data"), &content[..93_701]).await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::download("/source", &target))
        .await?;
    assert_eq!(plan.existing_bytes(), 93_701);
    assert_eq!(plan.bytes(), content.len() as u64);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    assert_eq!(tokio::fs::read(target.join("data")).await?, content);
    assert!(target.join("nested/empty").is_dir());
    assert!(
        tokio::fs::read(target.join("nested/zero"))
            .await?
            .is_empty()
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_resume_rejects_wrong_prefix_longer_targets_and_unplanned_entries()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    tokio::fs::write(source.join("data"), b"expected contents").await?;
    for (remote, content) in [
        ("/mismatch", &b"wrong"[..]),
        ("/longer", &b"expected contents and extra"[..]),
        ("/same-length", &b"different content"[..]),
    ] {
        sftp.mkdir(remote).await?;
        sftp.write(&format!("{remote}/data"), content).await?;
        assert!(
            sftp.plan_directory_resume(TransferSpec::upload(&source, remote))
                .await
                .is_err()
        );
        assert_eq!(sftp.read(&format!("{remote}/data"), 64).await?, content);
    }
    sftp.mkdir("/extra").await?;
    sftp.write("/extra/unplanned", b"preserve me").await?;
    assert!(
        sftp.plan_directory_resume(TransferSpec::upload(&source, "/extra"))
            .await
            .is_err()
    );
    assert_eq!(sftp.read("/extra/unplanned", 64).await?, b"preserve me");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn resumed_tree_revalidates_all_files_before_creating_the_first_missing_entry()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    tokio::fs::write(source.join("aa-missing"), b"must not be created").await?;
    tokio::fs::write(source.join("zz-partial"), b"good prefix and rest").await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/zz-partial", b"good prefix").await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    sftp.write("/partial/zz-partial", b"evil prefix").await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    let listing = sftp.list("/partial").await?;
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "zz-partial");
    assert_eq!(sftp.read("/partial/zz-partial", 64).await?, b"evil prefix");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn same_length_remote_source_change_after_review_leaves_local_tree_untouched()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let target = local.join("partial");
    tokio::fs::create_dir(&target).await?;
    sftp.mkdir("/source").await?;
    sftp.write("/source/aa-missing", b"new file").await?;
    sftp.write("/source/zz-partial", b"prefix old tail").await?;
    tokio::fs::write(target.join("zz-partial"), b"prefix ").await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::download("/source", &target))
        .await?;
    // The fixture deliberately supplies no mtime. Equal size must not turn a
    // changed source suffix into an implicitly re-approved transfer.
    sftp.write("/source/zz-partial", b"prefix new tail").await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert!(!target.join("aa-missing").exists());
    assert_eq!(
        tokio::fs::read(target.join("zz-partial")).await?,
        b"prefix "
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_resume_plans_allow_new_subsystems_but_never_a_new_ssh_connection()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    tokio::fs::write(source.join("data"), b"content").await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/data", b"con").await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    let other = SshSession::connect(options(&server)).await?;
    let other_sftp = Arc::new(other.sftp().await?);
    let other_queue = other_sftp.transfer_queue();
    let mut invalid = other_queue.enqueue_directory_resume(plan.clone()).await?;
    assert!(matches!(
        terminal(&mut invalid).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(sftp.read("/partial/data", 64).await?, b"con");
    sftp.close().await?;
    let reopened = Arc::new(session.sftp().await?);
    let queue = reopened.clone().transfer_queue();
    let mut valid = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut valid).await?,
        TransferEvent::Completed { bytes: 7, .. }
    ));
    assert_eq!(reopened.read("/partial/data", 64).await?, b"content");
    other.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn paused_directory_resume_can_cancel_release_fifo_and_be_explicitly_replanned()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    let content = payload();
    tokio::fs::write(source.join("data"), &content).await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/data", &content[..13]).await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    server.filesystem.set_transfer_write_delay(80);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    let mut next = queue
        .enqueue(TransferSpec::upload(source.join("data"), "/next"))
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = transfer.recv().await {
            if matches!(event, TransferEvent::Progress { .. }) {
                transfer.pause();
                return Ok::<_, Box<dyn Error>>(());
            }
        }
        Err("directory resume produced no progress".into())
    })
    .await??;
    let paused_at = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = transfer.recv().await {
            match event {
                TransferEvent::Paused { transferred, .. } => return Ok(transferred),
                TransferEvent::Completed { .. } | TransferEvent::Failed { .. } => {
                    return Err("directory resume terminated before pause acknowledgement".into());
                }
                _ => {}
            }
        }
        Err::<u64, Box<dyn Error>>("pause event stream closed".into())
    })
    .await??;
    assert!(paused_at >= 13 && paused_at < content.len() as u64);
    let partial = sftp.read("/partial/data", 256_000).await?;
    assert_eq!(partial.len() as u64, paused_at);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(sftp.read("/partial/data", 256_000).await?, partial);
    assert!(
        !sftp
            .list("/")
            .await?
            .iter()
            .any(|entry| entry.name == "next")
    );
    transfer.cancel();
    // A later Resume request cannot override irreversible cancellation.
    transfer.resume();
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Cancelled { bytes, .. } if bytes == paused_at
    ));
    server.filesystem.set_transfer_write_delay(0);
    assert!(matches!(
        terminal(&mut next).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    assert_eq!(plan.existing_bytes(), paused_at);
    let mut resumed = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut resumed).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    assert_eq!(sftp.read("/partial/data", 256_000).await?, content);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_resume_progress_accounts_each_file_prefix_when_that_file_is_processed()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    let content = payload();
    tokio::fs::write(source.join("aa-new"), b"first").await?;
    tokio::fs::write(source.join("zz-partial"), &content).await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/zz-partial", &content[..100_000])
        .await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    assert_eq!(plan.existing_bytes(), 100_000);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    let mut progress = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = transfer.recv().await {
            match event {
                TransferEvent::Progress { transferred, .. } => progress.push(transferred),
                TransferEvent::Completed { bytes, .. } => {
                    assert_eq!(bytes, content.len() as u64 + 5);
                    return Ok::<_, Box<dyn Error>>(());
                }
                TransferEvent::Cancelled { .. } | TransferEvent::Failed { .. } => {
                    return Err("directory accounting fixture failed".into());
                }
                _ => {}
            }
        }
        Err("directory accounting event stream closed".into())
    })
    .await??;
    let first_file = progress.iter().position(|bytes| *bytes == 5);
    let later_prefix = progress.iter().position(|bytes| *bytes == 100_005);
    assert!(matches!((first_file, later_prefix), (Some(first), Some(later)) if first < later));
    assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
    // Before zz-partial is processed, subtracting the full reviewed 100000-byte
    // prefix total would erase aa-new's five freshly acknowledged bytes.
    assert_eq!(sftp.read("/partial/aa-new", 64).await?, b"first");
    assert_eq!(sftp.read("/partial/zz-partial", 256_000).await?, content);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn changing_the_current_prefix_while_paused_is_rejected_before_more_bytes_are_written()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    let content = payload();
    tokio::fs::write(source.join("data"), &content).await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/data", &content[..13]).await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    server.filesystem.set_transfer_write_delay(80);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    let paused_at = pause_after_progress(&mut transfer, 13).await?;
    assert!(paused_at < content.len() as u64);
    let mut changed = sftp.read("/partial/data", 256_000).await?;
    changed[0] ^= 0xff;
    sftp.write("/partial/data", &changed).await?;
    transfer.resume();
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(sftp.read("/partial/data", 256_000).await?, changed);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn final_tree_verification_detects_mutation_of_an_earlier_completed_file()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    let content = payload();
    tokio::fs::write(source.join("aa-complete"), b"first").await?;
    tokio::fs::write(source.join("zz-partial"), &content).await?;
    sftp.mkdir("/partial").await?;
    sftp.write("/partial/zz-partial", &content[..13]).await?;
    let plan = sftp
        .plan_directory_resume(TransferSpec::upload(&source, "/partial"))
        .await?;
    server.filesystem.set_transfer_write_delay(80);
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory_resume(plan).await?;
    let paused_at = pause_after_progress(&mut transfer, 18).await?;
    assert!(paused_at < content.len() as u64 + 5);
    assert_eq!(sftp.read("/partial/aa-complete", 64).await?, b"first");
    // Remote fixture metadata has no mtime; the length also remains identical.
    // Only the final complete-tree content recheck can detect this mutation.
    sftp.write("/partial/aa-complete", b"other").await?;
    server.filesystem.set_transfer_write_delay(0);
    transfer.resume();
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(sftp.read("/partial/aa-complete", 64).await?, b"other");
    assert_eq!(sftp.read("/partial/zz-partial", 256_000).await?, content);
    session.close().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn directory_resume_refuses_static_symlink_targets_and_source_children()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_guard, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    tokio::fs::write(source.join("data"), b"content").await?;
    sftp.mkdir("/partial").await?;
    server.filesystem.insert_symlink("/partial/data")?;
    assert!(
        sftp.plan_directory_resume(TransferSpec::upload(&source, "/partial"))
            .await
            .is_err()
    );
    std::os::unix::fs::symlink(source.join("data"), source.join("alias"))?;
    sftp.mkdir("/empty").await?;
    assert!(
        sftp.plan_directory_resume(TransferSpec::upload(&source, "/empty"))
            .await
            .is_err()
    );
    assert!(sftp.list("/empty").await?.is_empty());
    session.close().await?;
    Ok(())
}
