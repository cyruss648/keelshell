//! Shared mutation admission exercised through actual owned TCP/SFTP handlers.
use super::*;
use keelshell_session::sftp::{RegularFileSnapshot, SftpSession, TransferHandle};
use std::future::Future;
use std::path::Path;
use std::sync::atomic::AtomicBool;

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

async fn all_public_mutators_are_isolated(
    sftp: &SftpSession,
    baseline: &RegularFileSnapshot,
    parent: &keelshell_session::sftp::RemoteEntry,
    source: &Path,
    download: &Path,
    quarantined: bool,
) -> Result<(), Box<dyn Error>> {
    let path = baseline.entry.path.as_str();
    isolation(sftp.write(path, b"bypass"), quarantined, "write").await?;
    isolation(sftp.upload(source, path), quarantined, "upload").await?;
    isolation(
        sftp.write_atomic(path, b"bypass"),
        quarantined,
        "write_atomic",
    )
    .await?;
    isolation(
        sftp.upload_atomic(source, path),
        quarantined,
        "upload_atomic",
    )
    .await?;
    isolation(
        sftp.write_regular_reviewed(baseline, b"bypass"),
        quarantined,
        "write_regular_reviewed",
    )
    .await?;
    let authorized = || true;
    isolation(
        sftp.write_regular_reviewed_authorized(baseline, b"bypass", &authorized),
        quarantined,
        "authorized reviewed writer",
    )
    .await?;
    isolation(
        sftp.download(path, download),
        quarantined,
        "download source",
    )
    .await?;
    isolation(sftp.mkdir(path), quarantined, "mkdir").await?;
    isolation(sftp.rename(path, "/moved"), quarantined, "rename source").await?;
    isolation(
        sftp.rename("/candidate", path),
        quarantined,
        "rename destination",
    )
    .await?;
    isolation(sftp.remove(path), quarantined, "remove").await?;
    isolation(sftp.rmdir(path), quarantined, "rmdir").await?;
    isolation(
        sftp.set_permissions_reviewed(&baseline.entry, 0o600),
        quarantined,
        "file permissions",
    )
    .await?;
    isolation(sftp.rmdir(&parent.path), quarantined, "parent rmdir").await?;
    isolation(
        sftp.set_permissions_reviewed(parent, 0o755),
        quarantined,
        "parent permissions",
    )
    .await?;
    assert!(!download.exists(), "blocked download created local output");
    assert_eq!(sftp.canonicalize(path).await?, path);
    assert_eq!(sftp.read(path, 64 * 1024).await?, baseline.content);
    let observed = sftp.read_regular_snapshot(path, 64 * 1024).await?;
    assert_eq!(observed.content, baseline.content);
    assert_eq!(observed.entry.permissions, baseline.entry.permissions);
    assert_eq!(observed.entry.modified, baseline.entry.modified);
    assert_eq!(sftp.read("/candidate", 100).await?, b"candidate stays");
    assert!(sftp.inspect_entry("/moved").await?.is_none());
    assert!(
        sftp.list(&parent.path)
            .await?
            .iter()
            .any(|entry| entry.path == path)
    );
    Ok(())
}

#[tokio::test]
async fn every_public_mutator_obeys_active_and_unknown_queue_isolation_until_exact_consent()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.mkdir("/protected").await?;
    sftp.write("/protected/file", b"reviewed original").await?;
    sftp.write("/candidate", b"candidate stays").await?;
    let baseline = sftp
        .read_regular_snapshot("/protected/file", 64 * 1024)
        .await?;
    let parent = sftp
        .inspect_entry("/protected")
        .await?
        .ok_or("missing fixture parent")?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    let download = local.path().join("blocked-download");
    tokio::fs::write(&source, vec![0x51; 256 * 1024]).await?;
    let spec = TransferSpec::upload(&source, "/protected/file");
    let queue = sftp.clone().transfer_queue();
    queue.set_parallelism(2)?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/protected/file")?;
    let mut running = queue.enqueue_atomic_upload(spec.clone()).await?;
    wait_for(|| hold.entered() == 1).await?;
    let before = server.filesystem.transfer_writes_started();
    all_public_mutators_are_isolated(&sftp, &baseline, &parent, &source, &download, false).await?;
    let sync_spec = TransferSpec::upload(local.path(), "/protected");
    let authorized = || true;
    isolation(
        sftp.reserve_directory_sync(&sync_spec, &authorized),
        false,
        "directory sync root against active child",
    )
    .await?;
    assert_eq!(server.filesystem.transfer_writes_started(), before);
    assert_eq!(hold.entered(), 1);
    sftp.write_atomic("/spare", b"independent active write")
        .await?;
    assert_eq!(sftp.read("/spare", 100).await?, b"independent active write");
    running.cancel();
    assert!(matches!(
        terminal(&mut running).await?,
        TransferEvent::Uncertain { bytes: 32768, .. }
    ));
    let before = server.filesystem.transfer_writes_started();
    all_public_mutators_are_isolated(&sftp, &baseline, &parent, &source, &download, true).await?;
    isolation(
        sftp.reserve_directory_sync(&sync_spec, &authorized),
        true,
        "directory sync root against unknown child",
    )
    .await?;
    assert_eq!(server.filesystem.transfer_writes_started(), before);
    sftp.write_atomic("/spare", b"independent unknown write")
        .await?;
    assert_eq!(
        sftp.read("/spare", 100).await?,
        b"independent unknown write"
    );
    assert!(!hold.expired());
    hold.release();
    queue.close().await?;
    sftp.close().await?;
    let fresh = Arc::new(session.sftp().await?);
    let review = fresh.inspect_transfer_quarantine(&spec).await?;
    assert!(
        review
            .entries()
            .iter()
            .any(|entry| entry.destination == "/protected/file" && entry.reservation_id > 0)
    );
    assert!(
        fresh
            .acknowledge_transfer_quarantine(&review, &AtomicBool::new(true))
            .await
            .is_err()
    );
    isolation(
        fresh.write_regular_reviewed(&baseline, b"approved after consent"),
        true,
        "revoked risk consent",
    )
    .await?;
    fresh
        .acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
        .await?;
    assert!(
        fresh
            .acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
            .await
            .is_err(),
        "exact consent is single-use"
    );
    fresh
        .write_regular_reviewed(&baseline, b"approved after consent")
        .await?;
    assert_eq!(
        fresh.read_regular("/protected/file", 64 * 1024).await?,
        b"approved after consent"
    );
    fresh.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn dropping_direct_reviewed_write_at_actual_pending_write_quarantines_other_connections()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/direct", b"old direct target").await?;
    let baseline = sftp.read_regular_snapshot("/direct", 64 * 1024).await?;
    let replacement = vec![0x62; 64 * 1024];
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/direct")?;
    let mut writer = Box::pin(sftp.write_regular_reviewed(&baseline, &replacement));
    tokio::select! {
        result = &mut writer => panic!("writer finished before its pending WRITE: {result:?}"),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    assert_eq!(
        sftp.read_regular("/direct", 64 * 1024).await?,
        baseline.content
    );
    assert_eq!(
        server.filesystem.transfer_writes_started(),
        3,
        "seed plus first and held second WRITE reached the server"
    );
    drop(writer);
    let other_session = SshSession::connect(options(&server)).await?;
    let other = other_session.sftp().await?;
    isolation(
        other.write_atomic("/direct", b"bypass"),
        true,
        "cross-connection direct unknown",
    )
    .await?;
    other
        .write_atomic("/direct-independent", b"allowed")
        .await?;
    assert_eq!(
        other.read_regular("/direct", 64 * 1024).await?,
        baseline.content
    );
    let local = tempfile::tempdir()?;
    let spec = TransferSpec::upload(local.path().join("review-source"), "/direct");
    let pending_review = other.inspect_transfer_quarantine(&spec).await?;
    assert!(
        pending_review
            .entries()
            .iter()
            .any(|entry| entry.destination == "/direct")
    );
    assert!(!hold.expired());
    hold.release();
    sftp.close().await?;
    // Owned staging cleanup may change an observed temporary pathname. Consent
    // must use fresh exact observations after that cleanup completion boundary.
    let review = other.inspect_transfer_quarantine(&spec).await?;
    other
        .acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
        .await?;
    other
        .write_regular_reviewed(&baseline, b"explicitly accepted followup")
        .await?;
    assert_eq!(
        other.read_regular("/direct", 64 * 1024).await?,
        b"explicitly accepted followup"
    );
    other.close().await?;
    other_session.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn direct_reviewed_raw_status_rejection_preserves_target_and_needs_no_risk_consent()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/rejected-direct", b"preserved direct target")
        .await?;
    let baseline = sftp
        .read_regular_snapshot("/rejected-direct", 64 * 1024)
        .await?;
    server.filesystem.set_atomic_write_failure(true);
    let bytes = vec![0x35; 64 * 1024];
    assert!(
        matches!(
            sftp.write_regular_reviewed(&baseline, &bytes).await,
            Err(SessionError::Sftp(_))
        ),
        "an acknowledged STATUS failure is a definite transport rejection"
    );
    assert_eq!(server.filesystem.atomic_writes_started(), 2);
    assert_eq!(
        sftp.read_regular("/rejected-direct", 64 * 1024).await?,
        baseline.content
    );
    sftp.close().await?;
    let fresh = session.sftp().await?;
    let root = tempfile::tempdir()?;
    let spec = TransferSpec::upload(root.path().join("source"), "/rejected-direct");
    assert!(fresh.inspect_transfer_quarantine(&spec).await.is_err());
    server.filesystem.set_atomic_write_failure(false);
    fresh.write_regular_reviewed(&baseline, &bytes).await?;
    assert_eq!(
        fresh.read_regular("/rejected-direct", 64 * 1024).await?,
        bytes
    );
    fresh.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn authorization_revoked_during_actual_metadata_preflight_creates_no_write_or_quarantine()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/authorized", b"reviewed authority target")
        .await?;
    let baseline = sftp.read_regular_snapshot("/authorized", 64 * 1024).await?;
    let held = server.filesystem.hold_metadata_path("/authorized")?;
    let revoked = AtomicBool::new(false);
    let authorized = || !revoked.load(Ordering::Acquire);
    let before = server.filesystem.transfer_writes_started();
    let mut writer = Box::pin(sftp.write_regular_reviewed_authorized(
        &baseline,
        b"must stay unsent",
        &authorized,
    ));
    tokio::select! {
        result = &mut writer => panic!("writer finished before metadata barrier: {result:?}"),
        result = wait_for(|| held.entered() == 1) => result?,
    }
    revoked.store(true, Ordering::Release);
    assert!(!held.expired());
    held.release();
    assert!(matches!(writer.await, Err(SessionError::Closed)));
    assert_eq!(server.filesystem.transfer_writes_started(), before);
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    assert_eq!(
        sftp.read_regular("/authorized", 64 * 1024).await?,
        baseline.content
    );
    let local = tempfile::tempdir()?;
    let spec = TransferSpec::upload(local.path().join("source"), "/authorized");
    assert!(sftp.inspect_transfer_quarantine(&spec).await.is_err());
    revoked.store(false, Ordering::Release);
    sftp.write_regular_reviewed_authorized(&baseline, b"fresh authorized write", &authorized)
        .await?;
    assert_eq!(
        sftp.read_regular("/authorized", 64 * 1024).await?,
        b"fresh authorized write"
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn authorized_writer_revoked_and_dropped_during_pending_write_retains_unknown_isolation()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/revoked-pending", b"original pending target")
        .await?;
    let baseline = sftp
        .read_regular_snapshot("/revoked-pending", 64 * 1024)
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/revoked-pending")?;
    let revoked = AtomicBool::new(false);
    let authorized = || !revoked.load(Ordering::Acquire);
    let replacement = vec![0x72; 64 * 1024];
    let mut writer =
        Box::pin(sftp.write_regular_reviewed_authorized(&baseline, &replacement, &authorized));
    tokio::select! {
        result = &mut writer => panic!("writer finished before pending WRITE: {result:?}"),
        result = wait_for(|| hold.entered() == 1) => result?,
    }
    revoked.store(true, Ordering::Release);
    // This mirrors the caller's lease-revoked select dropping the transport
    // future. Local cancellation cannot establish the held remote reply.
    drop(writer);
    isolation(
        sftp.write_regular_reviewed(&baseline, b"bypass"),
        true,
        "revoked pending WRITE",
    )
    .await?;
    assert_eq!(
        sftp.read_regular("/revoked-pending", 64 * 1024).await?,
        baseline.content
    );
    let local = tempfile::tempdir()?;
    let spec = TransferSpec::upload(local.path().join("source"), "/revoked-pending");
    assert!(
        sftp.inspect_transfer_quarantine(&spec)
            .await?
            .entries()
            .iter()
            .any(|entry| entry.destination == "/revoked-pending")
    );
    assert!(!hold.expired());
    hold.release();
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_sync_scope_owns_both_roots_and_local_publication_blocks_queued_download()
-> Result<(), Box<dyn Error>> {
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

#[tokio::test]
async fn rejected_write_with_unacknowledged_owned_cleanup_is_unknown_not_failed()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/cleanup-original", b"original").await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    tokio::fs::write(&source, vec![0x79; 128 * 1024]).await?;
    let queue = sftp.clone().transfer_queue();
    let held_write = server
        .filesystem
        .hold_atomic_upload_after_first("/cleanup-original")?;
    let mut transfer = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/cleanup-original"))
        .await?;
    wait_for(|| held_write.entered() == 1).await?;
    let temporary = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path.contains(".cleanup-original.keelshell-"))
        .ok_or("missing owned temporary")?
        .path;
    let cleanup = server.filesystem.hold_remove_path(&temporary)?;
    server.filesystem.set_atomic_write_failure(true);
    held_write.release();
    wait_for(|| cleanup.entered() == 1).await?;
    isolation(
        sftp.write_atomic("/cleanup-original", b"bypass"),
        false,
        "owned cleanup retains active final",
    )
    .await?;
    sftp.write_atomic("/cleanup-spare", b"independent").await?;
    assert!(matches!(
        terminal(&mut transfer).await?,
        TransferEvent::Uncertain { bytes: 32768, .. }
    ));
    assert!(!cleanup.expired());
    isolation(
        sftp.write_atomic("/cleanup-original", b"bypass"),
        true,
        "cleanup unknown final",
    )
    .await?;
    let review = sftp
        .inspect_remote_mutation_quarantine("/cleanup-original")
        .await?;
    assert_eq!(review.entries().len(), 2);
    assert!(
        review
            .entries()
            .iter()
            .any(|entry| entry.destination == temporary)
    );
    assert_eq!(sftp.read("/cleanup-original", 100).await?, b"original");
    cleanup.release();
    queue.close().await?;
    sftp.close().await?;
    let fresh = session.sftp().await?;
    isolation(
        fresh.write_atomic("/cleanup-original", b"late cleanup bypass"),
        true,
        "late cleanup cannot clear IDs",
    )
    .await?;
    assert_eq!(
        fresh
            .inspect_remote_mutation_quarantine("/cleanup-original")
            .await?
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>(),
        review
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>()
    );
    fresh.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn closing_sftp_owner_cancels_its_queue_without_replay_or_false_remote_rollback()
-> Result<(), Box<dyn Error>> {
    for pending_write in [false, true] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.write("/owner-close", b"original").await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        tokio::fs::write(&source, vec![0x7a; 128 * 1024]).await?;
        let spec = TransferSpec::upload(&source, "/owner-close");
        let queue = sftp.clone().transfer_queue();
        let held = server
            .filesystem
            .hold_atomic_upload_after_first("/owner-close")?;
        let mut transfer = queue.enqueue_atomic_upload(spec.clone()).await?;
        if pending_write {
            wait_for(|| held.entered() == 1).await?;
        } else {
            transfer.pause();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if matches!(
                        transfer.recv().await,
                        Some(TransferEvent::Paused { transferred: 0, .. })
                    ) {
                        break;
                    }
                }
            })
            .await?;
        }
        sftp.close().await?;
        assert!(matches!(
            queue.enqueue_atomic_upload(spec).await,
            Err(SessionError::Closed)
        ));
        if pending_write {
            assert!(matches!(
                terminal(&mut transfer).await?,
                TransferEvent::Uncertain { bytes: 32768, .. }
            ));
        } else {
            assert!(matches!(
                terminal(&mut transfer).await?,
                TransferEvent::Cancelled { bytes: 0, .. }
            ));
        }
        assert!(!held.expired());
        held.release();
        queue.close().await?;
        let fresh = session.sftp().await?;
        assert_eq!(fresh.read("/owner-close", 100).await?, b"original");
        if pending_write {
            isolation(
                fresh.write_atomic("/owner-close", b"no rollback claim"),
                true,
                "closed owner unknown",
            )
            .await?;
        } else {
            fresh
                .write_atomic("/owner-close", b"fresh separately admitted")
                .await?;
        }
        fresh.write_atomic("/owner-spare", b"usable").await?;
        assert!(!session.is_closed());
        fresh.close().await?;
        session.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn unexpected_rename_packet_retains_unknown_even_after_observed_publication()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/wrong-reply", b"original").await?;
    server.filesystem.set_unexpected_atomic_reply(true);
    assert!(matches!(
        sftp.write_atomic("/wrong-reply", b"published but no STATUS")
            .await,
        Err(SessionError::MutationUncertain)
    ));
    assert_eq!(
        sftp.read("/wrong-reply", 100).await?,
        b"published but no STATUS"
    );
    server.filesystem.set_unexpected_atomic_reply(false);
    isolation(
        sftp.write_atomic("/wrong-reply", b"automatic follow-up"),
        true,
        "unexpected rename reply",
    )
    .await?;
    let review = sftp
        .inspect_remote_mutation_quarantine("/wrong-reply")
        .await?;
    assert_eq!(review.entries().len(), 2);
    sftp.write_atomic("/wrong-reply-spare", b"independent")
        .await?;
    sftp.acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
        .await?;
    sftp.write_atomic("/wrong-reply", b"new separately reviewed write")
        .await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
