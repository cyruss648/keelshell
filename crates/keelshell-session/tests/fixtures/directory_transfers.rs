//! Real loopback SSH/SFTP acceptance for bounded directory jobs.
use super::*;
use keelshell_session::sftp::{DirectoryTransferPlan, TransferHandle};
use std::path::{Path, PathBuf};

async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(8), async {
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
        Err("transfer stopped without a terminal event".into())
    })
    .await
    .map_err(|error| format!("directory transfer terminal receipt: {error}"))?
}

fn temporary() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    // macOS commonly exposes TMPDIR through a /var symlink. Product transfer
    // paths reject static symlink ancestors, so fixture paths use their real root.
    let path = directory.path().canonicalize()?;
    Ok((directory, path))
}

async fn uploaded(
    sftp: &Arc<keelshell_session::sftp::SftpSession>,
    root: &Path,
    remote: &str,
) -> Result<DirectoryTransferPlan, Box<dyn Error>> {
    tokio::fs::create_dir_all(root.join("nested/empty")).await?;
    tokio::fs::write(root.join("nested/payload.bin"), vec![0x9d; 196_613]).await?;
    tokio::fs::write(root.join("零字节.txt"), []).await?;
    Ok(sftp
        .plan_directory_transfer(TransferSpec::upload(root, remote))
        .await?)
}

#[tokio::test]
async fn directory_roundtrip_preserves_nested_bytes_and_empty_directories()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let source = local.join("source");
    let plan = uploaded(&sftp, &source, "/copied").await?;
    assert_eq!(
        (plan.files(), plan.directories(), plan.bytes()),
        (2, 3, 196_613)
    );
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    let destination = local.join("downloaded");
    let plan = sftp
        .plan_directory_transfer(TransferSpec::download("/copied", &destination))
        .await?;
    let mut download = queue.enqueue_directory(plan).await?;
    assert!(matches!(
        terminal(&mut download).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    assert_eq!(
        tokio::fs::read(destination.join("nested/payload.bin")).await?,
        tokio::fs::read(source.join("nested/payload.bin")).await?
    );
    assert!(destination.join("nested/empty").is_dir());
    assert_eq!(
        tokio::fs::metadata(destination.join("零字节.txt"))
            .await?
            .len(),
        0
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_rechecks_source_and_refuses_existing_targets() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let source = local.join("source");
    let plan = uploaded(&sftp, &source, "/copied").await?;
    tokio::fs::write(source.join("new-file"), b"new").await?;
    let queue = sftp.clone().transfer_queue();
    let mut changed = queue.enqueue_directory(plan).await?;
    assert!(
        matches!(terminal(&mut changed).await?, TransferEvent::Failed { error, .. } if error.contains("changed after review"))
    );
    assert!(
        sftp.list("/").await?.is_empty(),
        "pre-write source change must not create output"
    );
    let plan = sftp
        .plan_directory_transfer(TransferSpec::upload(&source, "/copied"))
        .await?;
    sftp.mkdir("/copied").await?;
    sftp.write("/copied/preserve", b"existing").await?;
    let mut duplicate = queue.enqueue_directory(plan).await?;
    assert!(
        matches!(terminal(&mut duplicate).await?, TransferEvent::Failed { error, .. } if error.contains("destination already exists"))
    );
    assert_eq!(sftp.read("/copied/preserve", 64).await?, b"existing");
    let destination = local.join("destination");
    let plan = sftp
        .plan_directory_transfer(TransferSpec::download("/copied", &destination))
        .await?;
    tokio::fs::create_dir(&destination).await?;
    tokio::fs::write(destination.join("preserve"), b"local existing").await?;
    let mut duplicate = queue.enqueue_directory(plan).await?;
    assert!(matches!(
        terminal(&mut duplicate).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(
        tokio::fs::read(destination.join("preserve")).await?,
        b"local existing"
    );
    session.close().await?;
    Ok(())
}

#[test]
fn directory_cancellation_preserves_partial_tree_and_next_fifo_job_runs()
-> Result<(), Box<dyn Error>> {
    // Match the small default Windows test stack on every platform. The runtime
    // is current-thread so the real queue worker is also polled on this stack.
    let thread = std::thread::Builder::new()
        .name("directory-transfer-2mib".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(30), cancellation_and_next_fifo_job())
                    .await
                    .map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())
            })
        })?;
    thread
        .join()
        .map_err(|_| "directory transfer thread panicked")?
        .map_err(Into::into)
}

async fn cancellation_and_next_fifo_job() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let source = local.join("source");
    let plan = uploaded(&sftp, &source, "/partial").await?;
    let next = sftp
        .plan_directory_transfer(TransferSpec::upload(&source, "/next"))
        .await?;
    let hold = server
        .filesystem
        .hold_transfer_writes_after_first("/partial/nested/payload.bin")?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    let mut next = queue.enqueue_directory(next).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = transfer.recv().await {
            if matches!(event, TransferEvent::Progress { .. }) {
                return;
            }
        }
        panic!("missing directory progress");
    })
    .await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while hold.entered() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|error| format!("directory cancellation pause at held second WRITE: {error}"))?;
    assert!(!hold.expired());
    // Keep the second WRITE unanswered until pause is requested. This prevents
    // completion before a delayed event consumer can request the checkpoint.
    // Releasing it lets the worker acknowledge the WRITE before emitting Paused.
    transfer.pause();
    hold.release();
    let paused = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match transfer.recv().await {
                Some(TransferEvent::Paused { transferred, .. }) => return Ok(transferred),
                Some(TransferEvent::Progress { .. }) => {}
                receipt => {
                    return Err::<_, Box<dyn Error>>(
                        format!(
                            "directory cancellation ended before acknowledged pause: {receipt:?}"
                        )
                        .into(),
                    );
                }
            }
        }
    })
    .await
    .map_err(|error| format!("directory cancellation acknowledged pause: {error}"))??;
    assert!(paused > 0 && paused < 196_613);
    transfer.cancel();
    let cancelled = terminal(&mut transfer).await?;
    assert!(
        matches!(cancelled, TransferEvent::Cancelled { bytes, .. } if bytes == paused),
        "{cancelled:?}"
    );
    assert!(matches!(
        terminal(&mut next).await?,
        TransferEvent::Completed { bytes: 196_613, .. }
    ));
    assert!(
        sftp.list("/")
            .await?
            .iter()
            .any(|entry| entry.name == "partial")
    );
    assert_eq!(
        sftp.read("/next/nested/payload.bin", 256_000).await?.len(),
        196_613
    );
    let partial = sftp.read("/partial/nested/payload.bin", 256_000).await?;
    assert_eq!(partial, vec![0x9d; paused as usize]);
    eprintln!("known directory cancellation acknowledged_pause={paused} outcome={cancelled:?}");
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[test]
fn directory_cancellation_with_unanswered_write_quarantines_conflicts_and_runs_safe_fifo()
-> Result<(), Box<dyn Error>> {
    let thread = std::thread::Builder::new()
        .name("directory-unknown-write-2mib".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(30),
                    Box::pin(unknown_cancellation_and_next_fifo_job()),
                )
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
            })
        })?;
    thread
        .join()
        .map_err(|_| "directory unknown-write thread panicked")?
        .map_err(Into::into)
}

async fn unknown_cancellation_and_next_fifo_job() -> Result<(), Box<dyn Error>> {
    let mut server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let source = local.join("source");
    let plan = uploaded(&sftp, &source, "/uncertain-directory").await?;
    let next = sftp
        .plan_directory_transfer(TransferSpec::upload(&source, "/safe-directory"))
        .await?;
    let hold = server
        .filesystem
        .hold_transfer_writes_after_first("/uncertain-directory/nested/payload.bin")?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while hold.entered() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|error| format!("directory cancellation actual unanswered WRITE: {error}"))?;
    assert!(!hold.expired());
    let acknowledged = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match transfer.recv().await {
                Some(TransferEvent::Progress { transferred, .. }) if transferred > 0 => {
                    return Ok(transferred);
                }
                Some(
                    TransferEvent::Queued { .. }
                    | TransferEvent::Started { .. }
                    | TransferEvent::Progress { .. },
                ) => {}
                receipt => return Err::<_, Box<dyn Error>>(
                    format!(
                        "unanswered directory WRITE ended before acknowledged progress: {receipt:?}"
                    )
                    .into(),
                ),
            }
        }
    })
    .await
    .map_err(|error| format!("unanswered directory WRITE acknowledged progress: {error}"))??;
    assert!(acknowledged > 0 && acknowledged < 196_613);
    assert_eq!(server.filesystem.prepared_successful_write_statuses(), 1);
    assert_eq!(
        sftp.read("/uncertain-directory/nested/payload.bin", 256_000)
            .await?,
        vec![0x9d; acknowledged as usize]
    );
    let mut conflict = queue
        .enqueue(TransferSpec::upload(
            source.join("nested/payload.bin"),
            "/uncertain-directory/conflict.bin",
        ))
        .await?;
    let mut next = queue.enqueue_directory(next).await?;
    transfer.cancel();
    let uncertain = terminal(&mut transfer).await?;
    assert!(
        matches!(uncertain, TransferEvent::Uncertain { bytes, .. } if bytes == acknowledged),
        "{uncertain:?}"
    );
    let conflict = terminal(&mut conflict).await?;
    assert!(
        matches!(conflict, TransferEvent::Failed { ref error, .. } if error.contains("unknown transfer result")),
        "{conflict:?}"
    );
    let safe = terminal(&mut next).await?;
    assert!(
        matches!(safe, TransferEvent::Completed { bytes: 196_613, .. }),
        "{safe:?}"
    );
    assert_eq!(
        sftp.read("/safe-directory/nested/payload.bin", 256_000)
            .await?,
        vec![0x9d; 196_613]
    );
    assert!(
        sftp.list("/uncertain-directory")
            .await?
            .iter()
            .all(|entry| entry.name != "conflict.bin")
    );
    let before = sftp
        .inspect_remote_mutation_quarantine("/uncertain-directory")
        .await?;
    let before_ids: Vec<_> = before
        .entries()
        .iter()
        .map(|entry| entry.reservation_id)
        .collect();
    assert!(!before_ids.is_empty());
    assert!(matches!(
        sftp.write("/uncertain-directory/denied.bin", b"must not be sent")
            .await,
        Err(SessionError::MutationQuarantined)
    ));
    let writes_before_release = server.filesystem.prepared_successful_write_statuses();
    assert!(!hold.expired());
    hold.release();
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.filesystem.prepared_successful_write_statuses() != writes_before_release + 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<(), Box<dyn Error>>(())
    })
    .await
    .map_err(|error| {
        format!("directory cancellation late handler-prepared success WRITE STATUS: {error}")
    })??;
    let after = sftp
        .inspect_remote_mutation_quarantine("/uncertain-directory")
        .await?;
    let after_ids: Vec<_> = after
        .entries()
        .iter()
        .map(|entry| entry.reservation_id)
        .collect();
    assert_eq!(
        after_ids, before_ids,
        "preparing a late success STATUS cannot retire unknown writes"
    );
    assert!(matches!(
        sftp.write("/uncertain-directory/denied.bin", b"still forbidden")
            .await,
        Err(SessionError::MutationQuarantined)
    ));
    eprintln!(
        "unknown directory cancellation acknowledged={acknowledged} outcome={uncertain:?} conflict={conflict:?} safe={safe:?} before_ids={before_ids:?} after_ids={after_ids:?} writes_before_release={writes_before_release} writes_after_release={}",
        server.filesystem.prepared_successful_write_statuses()
    );
    // The fixture owns this scope and explicitly acknowledges its late-I/O
    // risk after observing the late server mutation and retained quarantine.
    // The fixture counter does not establish transport delivery of its STATUS.
    sftp.acknowledge_transfer_quarantine(&after, &std::sync::atomic::AtomicBool::new(false))
        .await?;
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    // The shared fixture's disconnect signal stops clients. Its listener task
    // is separately owned and keeps accepting until its handle is aborted.
    server.task.abort();
    let stopped = tokio::time::timeout(Duration::from_secs(3), &mut server.task)
        .await
        .map_err(|error| format!("directory cancellation owned listener shutdown: {error}"))?;
    assert!(matches!(stopped, Err(error) if error.is_cancelled()));
    let refused = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(server.address))
        .await
        .map_err(|error| format!("directory cancellation owned listener refusal: {error}"))?;
    assert!(refused.is_err(), "owned directory listener must be closed");
    Ok(())
}

#[tokio::test]
async fn directory_rejects_malicious_remote_names_and_symlinks_without_output()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let (_temporary, local) = temporary()?;
    sftp.mkdir("/source").await?;
    for name in [
        "../escape",
        "nested/escape",
        "a\\escape",
        "file:ads",
        "CON.txt",
        "tail.",
    ] {
        server.filesystem.set_injected_name(Some(name))?;
        assert!(
            sftp.plan_directory_transfer(TransferSpec::download("/source", local.join("target")))
                .await
                .is_err(),
            "accepted {name:?}"
        );
        assert!(!local.join("target").exists());
    }
    server.filesystem.set_injected_name(None)?;
    server.filesystem.insert_symlink("/source/link")?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::download("/source", local.join("target")))
            .await
            .is_err()
    );
    assert!(!local.join("target").exists());
    session.close().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn directory_rejects_local_symlink_roots_children_and_target_ancestors()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let (_temporary, local) = temporary()?;
    let source = local.join("source");
    tokio::fs::create_dir(&source).await?;
    std::os::unix::fs::symlink(&source, local.join("alias"))?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::upload(local.join("alias"), "/copied"))
            .await
            .is_err()
    );
    sftp.mkdir("/source").await?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::download(
            "/source",
            local.join("alias/target")
        ))
        .await
        .is_err()
    );
    std::os::unix::fs::symlink(local.join("missing"), source.join("link"))?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::upload(source, "/copied"))
            .await
            .is_err()
    );
    assert!(
        sftp.list("/")
            .await?
            .iter()
            .all(|entry| entry.name != "copied")
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_plan_cannot_move_to_another_authenticated_connection()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let plan = uploaded(&sftp, &local.join("source"), "/copied").await?;
    let other = SshSession::connect(options(&server)).await?;
    let other_sftp = Arc::new(other.sftp().await?);
    let queue = other_sftp.transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    assert!(
        matches!(terminal(&mut transfer).await?, TransferEvent::Failed { error, .. } if error.contains("different SSH connection"))
    );
    assert!(sftp.list("/").await?.is_empty());
    other.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_empty_root_and_cancelled_pending_job_do_not_create_unreviewed_output()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    let source = local.join("empty");
    tokio::fs::create_dir(&source).await?;
    let plan = sftp
        .plan_directory_transfer(TransferSpec::upload(&source, "/empty"))
        .await?;
    assert_eq!((plan.files(), plan.directories(), plan.bytes()), (0, 1, 0));
    let cancelled = sftp
        .plan_directory_transfer(TransferSpec::upload(&source, "/cancelled"))
        .await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    let mut cancelled = queue.enqueue_directory(cancelled).await?;
    cancelled.cancel();
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Completed { bytes: 0, .. }
    ));
    assert!(matches!(
        terminal(&mut cancelled).await?,
        TransferEvent::Cancelled { bytes: 0, .. }
    ));
    assert!(
        sftp.list("/")
            .await?
            .iter()
            .all(|entry| entry.name != "cancelled")
    );
    let mut wrong_kind = queue
        .enqueue(TransferSpec::upload(&source, "/do-not-create"))
        .await?;
    assert!(matches!(
        terminal(&mut wrong_kind).await?,
        TransferEvent::Failed { .. }
    ));
    assert!(
        sftp.list("/")
            .await?
            .iter()
            .all(|entry| entry.name != "do-not-create")
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_download_detects_changed_remote_snapshot_and_case_collisions()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let (_temporary, local) = temporary()?;
    sftp.mkdir("/source").await?;
    sftp.write("/source/readme", b"initial").await?;
    let target = local.join("target");
    let plan = sftp
        .plan_directory_transfer(TransferSpec::download("/source", &target))
        .await?;
    sftp.write("/source/readme", b"changed length").await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_directory(plan).await?;
    assert!(
        matches!(terminal(&mut transfer).await?, TransferEvent::Failed { error, .. } if error.contains("changed after review"))
    );
    assert!(!target.exists());
    sftp.write("/source/README", b"alias").await?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::download("/source", &target))
            .await
            .is_err()
    );
    assert!(!target.exists());
    server.filesystem.insert_symlink("/alias")?;
    assert!(
        sftp.plan_directory_transfer(TransferSpec::upload(&local, "/alias/target"))
            .await
            .is_err()
    );
    session.close().await?;
    Ok(())
}
