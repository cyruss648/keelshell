//! Desktop editor review through real owned loopback SSH/SFTP packets.
use super::*;

#[tokio::test]
async fn editor_reviewed_supports_one_mib_without_expanding_mcp_budget()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let old = vec![b'a'; 1024 * 1024];
    let new = vec![b'b'; 1024 * 1024];
    sftp.write("/editor.txt", &old).await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    assert!(matches!(
        sftp.write_regular_reviewed(&review, &new).await,
        Err(SessionError::OutputLimit(65536))
    ));
    let backup = sftp
        .write_editor_reviewed_authorized(&review, &new, &|| true)
        .await?;
    assert_eq!(sftp.read_regular(&backup, 1024 * 1024).await?, old);
    assert_eq!(
        sftp.read_regular_snapshot("/editor.txt", 1024 * 1024)
            .await?
            .content,
        new
    );
    assert_eq!(sftp.list("/").await?.len(), 2);
    let current = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    assert!(matches!(
        sftp.write_editor_reviewed_authorized(&current, &vec![b'x'; 1024 * 1024 + 1], &|| true)
            .await,
        Err(SessionError::OutputLimit(1048576))
    ));
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_rechecks_external_content_after_staging() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/editor.txt", b"base").await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/editor.txt")?;
    let replacement = vec![b'n'; 128 * 1024];
    let writer = sftp.write_editor_reviewed_authorized(&review, &replacement, &|| true);
    let external = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while hold.entered() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        server
            .filesystem
            .replace_external_file("/editor.txt", b"changed externally")?;
        hold.release();
        Ok::<(), Box<dyn Error>>(())
    };
    let (result, external) = tokio::join!(writer, external);
    external?;
    assert!(result.is_err());
    assert_eq!(
        sftp.read_regular("/editor.txt", 1024 * 1024).await?,
        b"changed externally"
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_revocation_refuses_publication_after_confirmed_write()
-> Result<(), Box<dyn Error>> {
    use std::sync::atomic::AtomicBool;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/editor.txt", b"base").await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/editor.txt")?;
    let replacement = vec![b'n'; 128 * 1024];
    let allowed = AtomicBool::new(true);
    let authorize = || allowed.load(Ordering::Acquire);
    let writer = sftp.write_editor_reviewed_authorized(&review, &replacement, &authorize);
    let revoke = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while hold.entered() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        allowed.store(false, Ordering::Release);
        hold.release();
        Ok::<(), Box<dyn Error>>(())
    };
    let (result, revoke) = tokio::join!(writer, revoke);
    revoke?;
    assert!(result.is_err());
    assert_eq!(
        sftp.read_regular("/editor.txt", 1024 * 1024).await?,
        b"base"
    );
    assert!(matches!(
        sftp.inspect_remote_mutation_quarantine("/editor.txt").await,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_type_metadata_and_denied_authority_fail_before_staging()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/editor.txt", b"base").await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    assert!(
        sftp.write_editor_reviewed_authorized(&review, b"replacement", &|| false)
            .await
            .is_err()
    );
    server
        .filesystem
        .set_file_mtime("/editor.txt", 1710000001)?;
    assert!(
        sftp.write_editor_reviewed_authorized(&review, b"replacement", &|| true)
            .await
            .is_err()
    );
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    server.filesystem.insert_symlink("/editor.txt")?;
    assert!(
        sftp.write_editor_reviewed_authorized(&review, b"replacement", &|| true)
            .await
            .is_err()
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_dropped_unacknowledged_writable_close_keeps_target_quarantine()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/editor.txt", b"base").await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    let hold =
        server
            .filesystem
            .hold_close_excluding("/.editor.txt.keelshell-", true, true, "backup")?;
    {
        let writer = sftp.write_editor_reviewed_authorized(&review, b"new draft", &|| true);
        tokio::pin!(writer);
        tokio::select! {
            result=&mut writer=>return Err(std::io::Error::other(format!("owned CLOSE never blocked: {result:?}")).into()),
            observed=tokio::time::timeout(Duration::from_secs(5),async {while hold.entered()==0 {tokio::task::yield_now().await;}})=>{observed?;}
        }
    }
    assert_eq!(
        sftp.read_regular("/editor.txt", 1024 * 1024).await?,
        b"base"
    );
    assert!(
        !sftp
            .inspect_remote_mutation_quarantine("/editor.txt")
            .await?
            .entries()
            .is_empty()
    );
    assert!(matches!(
        sftp.write("/editor.txt", b"bypass").await,
        Err(SessionError::MutationQuarantined)
    ));
    hold.release();
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_backup_survives_unknown_target_publication() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/editor.txt", b"original").await?;
    let review = sftp
        .read_regular_snapshot("/editor.txt", 1024 * 1024)
        .await?;
    let hold =
        server
            .filesystem
            .hold_close_excluding("/.editor.txt.keelshell-", true, true, "backup")?;
    let writer = sftp.write_editor_reviewed_authorized(&review, b"replacement", &|| true);
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(std::io::Error::other(format!("target publication was not held: {result:?}")).into()),
        observed = tokio::time::timeout(Duration::from_secs(5), async {
            while hold.entered() == 0 {
                tokio::task::yield_now().await;
            }
        }) => observed?,
    }
    let entries = sftp.list("/").await?;
    let backup = entries
        .iter()
        .find(|entry| {
            entry.name.starts_with(".editor.txt.keelshell-backup-") && !entry.name.contains(".tmp")
        })
        .ok_or("reviewed original was not backed up before publication")?;
    assert_eq!(sftp.read_regular(&backup.path, 1024).await?, b"original");
    assert_eq!(sftp.read_regular("/editor.txt", 1024).await?, b"original");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut writer)
            .await
            .is_err(),
        "target publication remained blocked"
    );
    hold.release();
    writer.await?;
    assert_eq!(
        sftp.read_regular("/editor.txt", 1024).await?,
        b"replacement"
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
