//! Existing-file review safeguards through real isolated SSH/SFTP packets.
use super::*;

#[tokio::test]
async fn reviewed_file_replaces_exact_bytes_preserves_mode_and_cleans_staging()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/reviewed.txt", "before中文\n".as_bytes())
        .await?;
    let baseline = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    let replacement = "after中文\nno final newline";
    sftp.write_regular_reviewed(&baseline, replacement.as_bytes())
        .await?;
    assert_eq!(
        sftp.read_regular("/reviewed.txt", 64 * 1024).await?,
        replacement.as_bytes()
    );
    assert_eq!(
        sftp.inspect_entry("/reviewed.txt")
            .await?
            .ok_or_else(|| std::io::Error::other("reviewed fixture file disappeared"))?
            .permissions,
        baseline.entry.permissions
    );
    assert_eq!(sftp.list("/").await?.len(), 1);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_file_refuses_stale_content_and_metadata_before_staging()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/reviewed.txt", b"before").await?;
    let baseline = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    sftp.write("/reviewed.txt", b"change").await?;
    assert!(
        sftp.write_regular_reviewed(&baseline, b"replacement")
            .await
            .is_err()
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    assert_eq!(
        sftp.read_regular("/reviewed.txt", 64 * 1024).await?,
        b"change"
    );
    server
        .filesystem
        .set_file_mtime("/reviewed.txt", 1710000000)
        .map_err(std::io::Error::other)?;
    let timed = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    assert_eq!(timed.entry.modified, Some(1710000000));
    server
        .filesystem
        .set_file_mtime("/reviewed.txt", 1710000001)
        .map_err(std::io::Error::other)?;
    assert!(
        sftp.write_regular_reviewed(&timed, b"replacement")
            .await
            .is_err()
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    let baseline = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    sftp.set_permissions_reviewed(&baseline.entry, 0o640)
        .await?;
    assert!(
        sftp.write_regular_reviewed(&baseline, b"replacement")
            .await
            .is_err()
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_file_requires_existing_regular_complete_bounded_snapshot_and_atomic_capability()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    assert!(
        sftp.read_regular_snapshot("/missing", 64 * 1024)
            .await
            .is_err()
    );
    sftp.mkdir("/directory").await?;
    assert!(
        sftp.read_regular_snapshot("/directory", 64 * 1024)
            .await
            .is_err()
    );
    server
        .filesystem
        .insert_symlink("/link")
        .map_err(std::io::Error::other)?;
    assert!(
        sftp.read_regular_snapshot("/link", 64 * 1024)
            .await
            .is_err()
    );
    sftp.write("/reviewed.txt", b"before").await?;
    let baseline = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    assert!(
        sftp.write_regular_reviewed(&baseline, &vec![b'x'; 64 * 1024 + 1])
            .await
            .is_err()
    );
    server.filesystem.set_atomic_unsupported(true);
    assert!(
        sftp.write_regular_reviewed(&baseline, b"replacement")
            .await
            .is_err()
    );
    assert_eq!(
        sftp.read_regular("/reviewed.txt", 64 * 1024).await?,
        b"before"
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_file_checks_content_again_after_staging_and_preserves_concurrent_writer()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/reviewed.txt", b"before").await?;
    let baseline = sftp
        .read_regular_snapshot("/reviewed.txt", 64 * 1024)
        .await?;
    let gate = server
        .filesystem
        .hold_atomic_writes_after_first()
        .map_err(std::io::Error::other)?;
    let replacement = vec![b'x'; 64 * 1024];
    let writer = sftp.write_regular_reviewed(&baseline, &replacement);
    let concurrent = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while gate.entered() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let other = session.sftp().await?;
        other.write("/reviewed.txt", b"concurrent").await?;
        other.close().await?;
        gate.release();
        Ok::<(), Box<dyn Error>>(())
    };
    let (written, concurrent) = tokio::join!(writer, concurrent);
    concurrent?;
    assert!(written.is_err());
    assert_eq!(
        sftp.read_regular("/reviewed.txt", 64 * 1024).await?,
        b"concurrent"
    );
    sftp.close().await?;
    // The temporary guard owns asynchronous cleanup, so observe it boundedly.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let verification = session.sftp().await?;
            let entries = verification.list("/").await?;
            verification.close().await?;
            if entries.len() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok::<(), keelshell_session::SessionError>(())
    })
    .await??;
    session.close().await?;
    Ok(())
}
