//! Desktop editor review through real owned loopback SSH/SFTP packets.
use super::*;

#[tokio::test]
async fn editor_reviewed_utf8_bom_crlf_final_budget_and_original_backup_are_exact()
-> Result<(), Box<dyn Error>> {
    use keelshell_core::{MAX_TEXT_EDIT_BYTES, TextDocumentError, Utf8TextDocument};

    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let raw = b"\xef\xbb\xbfold\r\nlast";
    sftp.write("/format-budget.txt", raw).await?;
    let reviewed = sftp
        .read_regular_snapshot("/format-budget.txt", MAX_TEXT_EDIT_BYTES)
        .await?;
    let document = Utf8TextDocument::decode(&reviewed.content)?;
    assert_eq!(document.body(), "old\r\nlast");
    let draft = format!("{}\n", "x".repeat(MAX_TEXT_EDIT_BYTES - 5));
    let prepared = document.format().prepare_save(&draft)?;
    assert_eq!(prepared.len(), MAX_TEXT_EDIT_BYTES);
    assert!(prepared.starts_with(b"\xef\xbb\xbf"));
    assert!(prepared.ends_with(b"\r\n"));
    let backup = sftp
        .write_editor_reviewed_authorized(&reviewed, &prepared, &|| true)
        .await?;
    assert_eq!(sftp.read_regular(&backup, MAX_TEXT_EDIT_BYTES).await?, raw);
    assert_eq!(
        sftp.read_regular_snapshot("/format-budget.txt", MAX_TEXT_EDIT_BYTES)
            .await?
            .content,
        prepared
    );
    let writes = server.filesystem.atomic_writes_started();
    assert_eq!(
        document.format().prepare_save(&format!("{draft}x")),
        Err(TextDocumentError::Limit)
    );
    assert_eq!(server.filesystem.atomic_writes_started(), writes);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_format_only_raw_changes_revoke_original_authority()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    for (raw, changed) in [
        (b"\xef\xbb\xbfa\n".as_slice(), b"a\n".as_slice()),
        (b"a\nb\n".as_slice(), b"a\r\nb\r\n".as_slice()),
    ] {
        sftp.write("/format-conflict.txt", raw).await?;
        let reviewed = sftp
            .read_regular_snapshot("/format-conflict.txt", 1024)
            .await?;
        server
            .filesystem
            .replace_external_file("/format-conflict.txt", changed)?;
        assert!(
            sftp.write_editor_reviewed_authorized(&reviewed, b"reviewed", &|| true)
                .await
                .is_err()
        );
        assert_eq!(
            sftp.read_regular("/format-conflict.txt", 1024).await?,
            changed
        );
        assert_eq!(server.filesystem.atomic_writes_started(), 0);
    }
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn editor_reviewed_publication_ack_does_not_replace_complete_caller_readback()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    let original = b"\xef\xbb\xbfold\r\nTAIL";
    let reviewed_bytes = b"\xef\xbb\xbfnew\r\nTAIL";
    let raced_bytes = b"\xef\xbb\xbfnew\r\nTAIX";
    sftp.write("/readback-format.txt", original).await?;
    let review = sftp
        .read_regular_snapshot("/readback-format.txt", 1024)
        .await?;
    server
        .filesystem
        .replace_after_atomic_rename("/readback-format.txt", raced_bytes)?;
    let backup = sftp
        .write_editor_reviewed_authorized(&review, reviewed_bytes, &|| true)
        .await?;
    let actual = sftp
        .read_regular_snapshot("/readback-format.txt", 1024)
        .await?;
    assert_eq!(actual.content.len(), reviewed_bytes.len());
    assert_ne!(actual.content, reviewed_bytes);
    assert_eq!(actual.content, raced_bytes);
    assert_eq!(sftp.read_regular(&backup, 1024).await?, original);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

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
