//! Original-connection reviewed transfer behavior through owned TCP SFTP.
use super::*;
use keelshell_session::sftp::TransferHandle;

async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let event = handle
                .recv()
                .await
                .ok_or("reviewed transfer lost its terminal event")?;
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Uncertain { .. }
            ) {
                return Ok::<_, Box<dyn Error>>(event);
            }
        }
    })
    .await?
}

#[tokio::test]
async fn reviewed_file_roundtrip_creates_only_after_admission_and_preserves_full_bytes()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let guard = tempfile::tempdir()?;
    let root = guard.path().canonicalize()?;
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let bytes: Vec<_> = (0..131_113).map(|index| (index % 251) as u8).collect();
    tokio::fs::write(&source, &bytes).await?;
    let upload = sftp
        .plan_file_transfer(TransferSpec::upload(&source, "/reviewed.bin"))
        .await?;
    assert_eq!(upload.bytes(), bytes.len() as u64);
    assert!(!upload.replaces_existing());
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    assert!(sftp.inspect_entry("/reviewed.bin").await?.is_none());
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_reviewed_file(upload).await?;
    assert!(
        matches!(terminal(&mut transfer).await?, TransferEvent::Completed { bytes: count, .. } if count == bytes.len() as u64)
    );
    assert_eq!(
        sftp.read_regular("/reviewed.bin", bytes.len()).await?,
        bytes
    );
    let download = sftp
        .plan_file_transfer(TransferSpec::download("/reviewed.bin", &target))
        .await?;
    assert!(!target.exists());
    let mut transfer = queue.enqueue_reviewed_file(download).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Completed { .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, bytes);
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_file_refuses_changed_local_source_or_remote_overwrite_before_staging()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let guard = tempfile::tempdir()?;
    let source = guard.path().canonicalize()?.join("source.bin");
    tokio::fs::write(&source, b"reviewed").await?;
    let upload = sftp
        .plan_file_transfer(TransferSpec::upload(&source, "/target.bin"))
        .await?;
    tokio::fs::write(&source, b"changed length").await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_reviewed_file(upload).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    assert!(sftp.inspect_entry("/target.bin").await?.is_none());
    sftp.write("/target.bin", b"old").await?;
    let upload = sftp
        .plan_file_transfer(TransferSpec::upload(&source, "/target.bin"))
        .await?;
    assert!(upload.replaces_existing());
    sftp.write("/target.bin", b"new target length").await?;
    let mut transfer = queue.enqueue_reviewed_file(upload).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    assert_eq!(
        sftp.read_regular("/target.bin", 100).await?,
        b"new target length"
    );
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_download_refuses_a_late_local_destination_without_truncating_it()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/source.bin", b"remote bytes").await?;
    let guard = tempfile::tempdir()?;
    let target = guard.path().canonicalize()?.join("download.bin");
    let plan = sftp
        .plan_file_transfer(TransferSpec::download("/source.bin", &target))
        .await?;
    tokio::fs::write(&target, b"do not truncate").await?;
    let queue = sftp.clone().transfer_queue();
    let mut transfer = queue.enqueue_reviewed_file(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, b"do not truncate");
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_file_is_bound_to_the_original_authenticated_connection()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let guard = tempfile::tempdir()?;
    let source = guard.path().canonicalize()?.join("source.bin");
    tokio::fs::write(&source, b"original source").await?;
    let plan = sftp
        .plan_file_transfer(TransferSpec::upload(&source, "/bound.bin"))
        .await?;
    let replacement = SshSession::connect(options(&server)).await?;
    let other = Arc::new(replacement.sftp().await?);
    let queue = other.clone().transfer_queue();
    let mut transfer = queue.enqueue_reviewed_file(plan).await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    queue.close().await?;
    other.close().await?;
    replacement.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn reviewed_file_rejects_links_and_traversal_without_source_reads_or_writes()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let guard = tempfile::tempdir()?;
    let root = guard.path().canonicalize()?;
    let source = root.join("source");
    tokio::fs::write(&source, b"ordinary").await?;
    let link = root.join("link");
    std::os::unix::fs::symlink(&source, &link)?;
    assert!(
        sftp.plan_file_transfer(TransferSpec::upload(&link, "/link.bin"))
            .await
            .is_err()
    );
    assert!(
        sftp.plan_file_transfer(TransferSpec::upload(&source, "/../escape"))
            .await
            .is_err()
    );
    let parent_link = root.join("parent-link");
    std::os::unix::fs::symlink(&root, &parent_link)?;
    assert!(
        sftp.plan_file_transfer(TransferSpec::upload(
            parent_link.join("source"),
            "/link.bin"
        ))
        .await
        .is_err()
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_upload_rechecks_source_identity_and_destination_after_confirmed_staging()
-> Result<(), Box<dyn Error>> {
    for replace_source in [true, false] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        let guard = tempfile::tempdir()?;
        let root = guard.path().canonicalize()?;
        let source = root.join("source.bin");
        tokio::fs::write(&source, vec![b's'; 128 * 1024]).await?;
        sftp.write("/reviewed.bin", b"original destination").await?;
        let plan = sftp
            .plan_file_transfer(TransferSpec::upload(&source, "/reviewed.bin"))
            .await?;
        let held = server
            .filesystem
            .hold_atomic_upload_after_first("/reviewed.bin")?;
        let queue = sftp.clone().transfer_queue();
        let mut transfer = queue.enqueue_reviewed_file(plan).await?;
        tokio::time::timeout(Duration::from_secs(5), async {
            while held.entered() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert!(server.filesystem.atomic_writes_started() > 0);
        if replace_source {
            // Keep the reviewed descriptor alive while replacing its name.
            // Equal length still fails the named source identity observation.
            tokio::fs::rename(&source, root.join("original-source.bin")).await?;
            tokio::fs::write(&source, vec![b'x'; 128 * 1024]).await?;
        } else {
            server
                .filesystem
                .replace_external_file("/reviewed.bin", b"external destination change")?;
        }
        assert!(!held.expired());
        held.release();
        assert!(matches!(
            terminal(&mut transfer).await?,
            TransferEvent::Failed { .. }
        ));
        let expected: &[u8] = if replace_source {
            b"original destination"
        } else {
            b"external destination change"
        };
        assert_eq!(sftp.read_regular("/reviewed.bin", 1024).await?, expected);
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
    }
    Ok(())
}
