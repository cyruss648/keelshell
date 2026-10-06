//! Reviewed directory trees share the real process-wide local writer exclusion.
//! Keep this immediate-admission scenario with the existing serialized filesystem
//! scenarios: unrelated in-place writers in another temporary directory still
//! own the entire local side to protect hard-link aliases.
use super::*;
use std::future::Future;

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
        Err::<_, Box<dyn Error>>("transfer ended without a terminal receipt".into())
    })
    .await?
}

async fn isolation<T>(
    operation: impl Future<Output = Result<T, SessionError>>,
    quarantined: bool,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    // Admission rejection is immediate. Waiting for the held WRITE would mask
    // a bypass, and its eventual fixture fallback must never satisfy this test.
    let result = tokio::time::timeout(Duration::from_secs(3), operation).await?;
    match result {
        Err(SessionError::MutationBusy) if !quarantined => Ok(()),
        Err(SessionError::MutationQuarantined) if quarantined => Ok(()),
        Err(error) => panic!("{name}: expected shared isolation, got {error:?}"),
        Ok(_) => panic!("{name}: conflicting operation was admitted"),
    }
}

#[tokio::test]
async fn directory_sync_scope_owns_both_roots_and_local_publication_blocks_queued_download()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.mkdir("/sync-root").await?;
    sftp.write("/sync-root/source", b"queued content").await?;
    let local = tempfile::tempdir()?;
    let target = local.path().join("download");
    let spec = TransferSpec::download("/sync-root", local.path());
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    let local_published = local.path().join("published");
    scope.local_operation(|| std::fs::write(&local_published, b"scope-owned local publication"))?;
    assert_eq!(
        tokio::fs::read(&local_published).await?,
        b"scope-owned local publication"
    );
    let queue = sftp.clone().transfer_queue();
    let mut waiting = queue
        .enqueue(TransferSpec::download("/sync-root/source", &target))
        .await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), waiting.recv())
            .await?
            .ok_or("missing queued receipt")?,
        TransferEvent::Queued { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), waiting.recv())
            .await
            .is_err(),
        "queued download must not enter while its local subtree is reserved"
    );
    assert!(!target.exists());
    isolation(
        sftp.mkdir("/sync-root/foreign"),
        false,
        "sync remote source root",
    )
    .await?;
    assert!(sftp.inspect_entry("/sync-root/foreign").await?.is_none());
    assert_eq!(
        sftp.read("/sync-root/source", 100).await?,
        b"queued content"
    );
    drop(scope);
    assert!(matches!(
        terminal(&mut waiting).await?,
        TransferEvent::Completed { bytes: 14, .. }
    ));
    assert_eq!(tokio::fs::read(&target).await?, b"queued content");
    let upload_spec = TransferSpec::upload(local.path(), "/");
    let upload_scope = sftp
        .reserve_directory_sync(&upload_spec, &permitted)
        .await?;
    upload_scope.mkdir_remote("/sync-root/owned").await?;
    upload_scope
        .write_remote_atomic("/sync-root/owned/file", b"nested operation shares owner")
        .await?;
    assert_eq!(
        sftp.read("/sync-root/owned/file", 100).await?,
        b"nested operation shares owner"
    );
    // One reviewed synchronization can publish more children than the global
    // action budget: acknowledged temporaries retire within the same owner.
    for index in 0..40 {
        upload_scope
            .write_remote_atomic(&format!("/sync-root/owned/child-{index}"), b"child")
            .await?;
    }
    assert_eq!(sftp.list("/sync-root/owned").await?.len(), 41);
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/sync-root/owned/blocked")?;
    let replacement = vec![0x78; 64 * 1024];
    let mut child =
        Box::pin(upload_scope.write_remote_atomic("/sync-root/owned/blocked", &replacement));
    tokio::select! {
        result = &mut child => panic!("child finished before held WRITE: {result:?}"),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    assert!(
        matches!(
            upload_scope
                .mkdir_remote("/sync-root/owned/concurrent")
                .await,
            Err(SessionError::MutationBusy)
        ),
        "one scope must not interleave two child writers"
    );
    assert!(
        sftp.inspect_entry("/sync-root/owned/concurrent")
            .await?
            .is_none()
    );
    drop(child);
    assert!(!hold.expired());
    hold.release();
    drop(upload_scope);
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
