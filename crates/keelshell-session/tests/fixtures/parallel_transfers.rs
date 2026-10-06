//! Real TCP/SFTP overlap and conflict tests, using owned response barriers.
use super::*;
use keelshell_session::sftp::{MAX_QUEUED_TRANSFERS, TransferHandle, TransferQueue};

async fn next(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(7), handle.recv())
        .await?
        .ok_or_else(|| "transfer stream ended".into())
}
async fn terminal(handle: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    loop {
        let event = next(handle).await?;
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
}
async fn wait_for(predicate: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn distinct_existing_atomic_uploads_really_overlap_before_either_publishes()
-> Result<(), Box<dyn Error>> {
    overlapping_existing_atomic_uploads().await
}

async fn overlapping_existing_atomic_uploads() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/one", b"old-one").await?;
    sftp.write("/two", b"old-two").await?;
    let queue = sftp.clone().transfer_queue();
    queue.set_parallelism(2)?;
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let content = vec![0x91; 256 * 1024];
    tokio::fs::write(&source, &content).await?;
    let hold = server.filesystem.hold_atomic_writes_after_first()?;
    let mut one = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/one"))
        .await?;
    let mut two = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/two"))
        .await?;
    // Both distinct server handlers reach the second WRITE without releasing
    // either. A sequential implementation cannot pass this barrier.
    wait_for(|| hold.entered() == 2).await?;
    assert!(!hold.expired());
    assert_eq!(sftp.read("/one", 100).await?, b"old-one");
    assert_eq!(sftp.read("/two", 100).await?, b"old-two");
    queue.set_parallelism(1)?;
    assert_eq!(queue.parallelism(), 1);
    assert!(queue.set_parallelism(0).is_err());
    assert!(queue.set_parallelism(5).is_err());
    hold.release();
    assert!(matches!(
        terminal(&mut one).await?,
        TransferEvent::Completed { bytes: 262144, .. }
    ));
    assert!(matches!(
        terminal(&mut two).await?,
        TransferEvent::Completed { bytes: 262144, .. }
    ));
    assert_eq!(sftp.read("/one", content.len()).await?, content);
    assert_eq!(sftp.read("/two", content.len()).await?, content);
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn same_atomic_destination_waits_and_queued_cancel_never_writes() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    queue.set_parallelism(4)?;
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    tokio::fs::write(&source, vec![0x34; 256 * 1024]).await?;
    let hold = server.filesystem.hold_atomic_writes_after_first()?;
    let mut first = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/same"))
        .await?;
    wait_for(|| hold.entered() == 1).await?;
    let mut collision = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/same"))
        .await?;
    assert!(matches!(
        next(&mut collision).await?,
        TransferEvent::Queued { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(120), collision.recv())
            .await
            .is_err()
    );
    assert_eq!(hold.entered(), 1);
    collision.cancel();
    assert!(matches!(
        terminal(&mut collision).await?,
        TransferEvent::Cancelled { bytes: 0, .. }
    ));
    first.cancel();
    assert!(matches!(
        terminal(&mut first).await?,
        TransferEvent::Uncertain { bytes: 32768, .. }
    ));
    hold.release();
    queue.close().await?;
    assert!(
        sftp.read("/same", 100).await.is_err(),
        "cancelled atomic upload cannot publish a partial target"
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_root_blocks_child_destination_but_allows_independent_job()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    queue.set_parallelism(3)?;
    let root = tempfile::tempdir()?;
    let tree = root.path().join("tree");
    tokio::fs::create_dir(&tree).await?;
    let source = tree.join("file");
    tokio::fs::write(&source, vec![0x12; 256 * 1024]).await?;
    let plan = sftp
        .plan_directory_transfer(TransferSpec::upload(&tree, "/tree"))
        .await?;
    let hold = server
        .filesystem
        .hold_transfer_writes_after_first("/tree/file")?;
    let mut parent = queue.enqueue_directory(plan).await?;
    wait_for(|| hold.entered() == 1).await?;
    let mut child = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/tree/another"))
        .await?;
    assert!(matches!(
        next(&mut child).await?,
        TransferEvent::Queued { .. }
    ));
    let mut independent = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/independent"))
        .await?;
    assert!(matches!(
        terminal(&mut independent).await?,
        TransferEvent::Completed { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(120), child.recv())
            .await
            .is_err()
    );
    assert!(!hold.expired());
    hold.release();
    assert!(matches!(
        terminal(&mut parent).await?,
        TransferEvent::Completed { .. }
    ));
    assert!(matches!(
        terminal(&mut child).await?,
        TransferEvent::Completed { .. }
    ));
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn paused_job_retains_slot_and_lock_while_another_job_completes() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    queue.set_parallelism(2)?;
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    tokio::fs::write(&source, vec![0x71; 256 * 1024]).await?;
    let mut paused = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/paused"))
        .await?;
    paused.pause();
    loop {
        if matches!(next(&mut paused).await?, TransferEvent::Paused { .. }) {
            break;
        }
    }
    let mut independent = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/other"))
        .await?;
    let mut conflict = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/paused"))
        .await?;
    assert!(matches!(
        next(&mut conflict).await?,
        TransferEvent::Queued { .. }
    ));
    assert!(matches!(
        terminal(&mut independent).await?,
        TransferEvent::Completed { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(120), conflict.recv())
            .await
            .is_err()
    );
    paused.resume();
    assert!(matches!(
        terminal(&mut paused).await?,
        TransferEvent::Completed { .. }
    ));
    assert!(matches!(
        terminal(&mut conflict).await?,
        TransferEvent::Completed { .. }
    ));
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn admission_is_bounded_and_retired_queue_cancels_without_replay()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = TransferQueue::new(sftp.clone());
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    tokio::fs::write(&source, vec![0x31; 128 * 1024]).await?;
    let mut first = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/zero"))
        .await?;
    first.pause();
    loop {
        if matches!(next(&mut first).await?, TransferEvent::Paused { .. }) {
            break;
        }
    }
    let mut pending = Vec::new();
    for index in 1..MAX_QUEUED_TRANSFERS {
        pending.push(
            queue
                .enqueue_atomic_upload(TransferSpec::upload(&source, format!("/pending-{index}")))
                .await?,
        );
    }
    assert!(
        queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/too-many"))
            .await
            .is_err()
    );
    queue.cancel_all();
    assert!(
        queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/retired"))
            .await
            .is_err()
    );
    assert!(matches!(
        terminal(&mut first).await?,
        TransferEvent::Cancelled { bytes: 0, .. }
    ));
    for handle in &mut pending {
        assert!(matches!(
            terminal(handle).await?,
            TransferEvent::Cancelled { bytes: 0, .. }
        ));
    }
    queue.close().await?;
    assert!(sftp.read("/retired", 100).await.is_err());
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn unknown_atomic_write_stays_isolated_across_connections_until_explicit_inspected_consent()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/isolated", b"old target").await?;
    let source_root = tempfile::tempdir()?;
    let source = source_root.path().join("source");
    let content = vec![0x51; 256 * 1024];
    tokio::fs::write(&source, &content).await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/isolated")?;
    let first_queue = sftp.clone().transfer_queue();
    let mut first = first_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/isolated"))
        .await?;
    wait_for(|| hold.entered() == 1).await?;
    first.cancel();
    assert!(matches!(
        terminal(&mut first).await?,
        TransferEvent::Uncertain { bytes: 32768, .. }
    ));
    assert_eq!(sftp.read("/isolated", 100).await?, b"old target");
    let other_sftp = Arc::new(session.sftp().await?);
    let other_queue = other_sftp.clone().transfer_queue();
    other_queue.set_parallelism(2)?;
    let mut conflicting = other_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/isolated"))
        .await?;
    assert!(matches!(
        terminal(&mut conflicting).await?,
        TransferEvent::Failed { .. }
    ));
    let mut distinct = other_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/distinct"))
        .await?;
    assert!(matches!(
        terminal(&mut distinct).await?,
        TransferEvent::Completed { .. }
    ));
    assert_eq!(other_sftp.read("/distinct", content.len()).await?, content);
    assert_eq!(hold.entered(), 1);
    assert!(!hold.expired());
    hold.release();
    first_queue.close().await?;
    other_queue.close().await?;
    other_sftp.close().await?;
    sftp.close().await?;
    // Reconnection changes approval identity, but cannot establish late-I/O completion.
    let fresh = SshSession::connect(options(&server)).await?;
    let fresh_sftp = Arc::new(fresh.sftp().await?);
    let fresh_queue = fresh_sftp.clone().transfer_queue();
    let spec = TransferSpec::upload(&source, "/isolated");
    let mut still_isolated = fresh_queue.enqueue_atomic_upload(spec.clone()).await?;
    assert!(matches!(
        terminal(&mut still_isolated).await?,
        TransferEvent::Failed { .. }
    ));
    let review = fresh_sftp.inspect_transfer_quarantine(&spec).await?;
    assert_eq!(
        review.entries().len(),
        2,
        "final target plus the owned temporary are reviewed together"
    );
    assert_eq!(review.entries()[0].destination, "/isolated");
    assert_eq!(review.entries()[0].bytes, Some(10));
    let unrelated_session = SshSession::connect(options(&server)).await?;
    let unrelated_sftp = unrelated_session.sftp().await?;
    assert!(
        unrelated_sftp
            .acknowledge_transfer_quarantine(&review, &std::sync::atomic::AtomicBool::new(false))
            .await
            .is_err()
    );
    // An observed change invalidates this exact review. No broad clear occurs.
    assert!(matches!(
        fresh_sftp.write("/isolated", b"application bypass").await,
        Err(SessionError::MutationQuarantined)
    ));
    server
        .filesystem
        .replace_external_file("/isolated", b"changed target")?;
    assert!(
        fresh_sftp
            .acknowledge_transfer_quarantine(&review, &std::sync::atomic::AtomicBool::new(false))
            .await
            .is_err()
    );
    let prior = fresh_sftp.inspect_transfer_quarantine(&spec).await?;
    let next_hold = server
        .filesystem
        .hold_atomic_upload_after_first("/separate-unknown")?;
    let mut next_unknown = fresh_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/separate-unknown"))
        .await?;
    wait_for(|| next_hold.entered() == 1).await?;
    next_unknown.cancel();
    assert!(matches!(
        terminal(&mut next_unknown).await?,
        TransferEvent::Uncertain { .. }
    ));
    assert!(
        fresh_sftp
            .acknowledge_transfer_quarantine(&prior, &std::sync::atomic::AtomicBool::new(false))
            .await
            .is_err(),
        "a new quarantine invalidates consent"
    );
    next_hold.release();
    let reviewed = fresh_sftp.inspect_transfer_quarantine(&spec).await?;
    assert!(reviewed.entries()[0].reservation_id > 0);
    assert!(
        fresh_sftp
            .acknowledge_transfer_quarantine(&reviewed, &std::sync::atomic::AtomicBool::new(true))
            .await
            .is_err()
    );
    let metadata = server.filesystem.hold_metadata_path("/isolated")?;
    let revoke = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let checking_sftp = fresh_sftp.clone();
    let checking_review = reviewed.clone();
    let checking_revoke = revoke.clone();
    let pending_consent = tokio::spawn(async move {
        checking_sftp
            .acknowledge_transfer_quarantine(&checking_review, &checking_revoke)
            .await
    });
    wait_for(|| metadata.entered() == 1).await?;
    revoke.store(true, std::sync::atomic::Ordering::Release);
    assert!(!metadata.expired());
    metadata.release();
    assert!(
        pending_consent.await?.is_err(),
        "revocation during read-only confirmation recheck is binding"
    );
    fresh_sftp
        .acknowledge_transfer_quarantine(&reviewed, &std::sync::atomic::AtomicBool::new(false))
        .await?;
    let mut separate_still_blocked = fresh_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/separate-unknown"))
        .await?;
    assert!(
        matches!(
            terminal(&mut separate_still_blocked).await?,
            TransferEvent::Failed { .. }
        ),
        "only exact reviewed IDs are cleared"
    );
    assert!(
        fresh_sftp
            .acknowledge_transfer_quarantine(&reviewed, &std::sync::atomic::AtomicBool::new(false))
            .await
            .is_err()
    );
    unrelated_sftp.close().await?;
    unrelated_session.close().await?;
    let mut reviewed_again = fresh_queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/isolated"))
        .await?;
    assert!(matches!(
        terminal(&mut reviewed_again).await?,
        TransferEvent::Completed { .. }
    ));
    assert_eq!(fresh_sftp.read("/isolated", content.len()).await?, content);
    fresh_queue.close().await?;
    fresh_sftp.close().await?;
    fresh.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn distinct_downloads_overlap_real_read_handlers_and_local_paths_lock_across_connections()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let first_session = SshSession::connect(options(&server)).await?;
    let second_session = SshSession::connect(options(&server)).await?;
    let first_sftp = Arc::new(first_session.sftp().await?);
    let second_sftp = Arc::new(second_session.sftp().await?);
    let bytes = vec![0x2b; 256 * 1024];
    first_sftp.write("/download-source", &bytes).await?;
    let local = tempfile::tempdir()?;
    let first_path = local.path().join("one");
    let second_path = local.path().join("two");
    let first_queue = first_sftp.clone().transfer_queue();
    let second_queue = second_sftp.clone().transfer_queue();
    second_queue.set_parallelism(2)?;
    let hold = server.filesystem.hold_transfer_reads_after_first()?;
    let mut first = first_queue
        .enqueue(TransferSpec::download("/download-source", &first_path))
        .await?;
    let mut second = second_queue
        .enqueue(TransferSpec::download("/download-source", &second_path))
        .await?;
    wait_for(|| hold.entered() == 2).await?;
    assert!(!hold.expired());
    assert_eq!(tokio::fs::metadata(&first_path).await?.len(), 65536);
    assert_eq!(tokio::fs::metadata(&second_path).await?.len(), 65536);
    let mut collision = second_queue
        .enqueue(TransferSpec::download("/download-source", &first_path))
        .await?;
    assert!(matches!(
        next(&mut collision).await?,
        TransferEvent::Queued { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), collision.recv())
            .await
            .is_err()
    );
    collision.cancel();
    assert!(matches!(
        terminal(&mut collision).await?,
        TransferEvent::Cancelled { bytes: 0, .. }
    ));
    // Cancelling a pending READ has no pending destination mutation; the partial
    // local file is honest, not a fabricated rollback or unknown late write.
    first.cancel();
    assert!(matches!(
        terminal(&mut first).await?,
        TransferEvent::Cancelled { bytes: 65536, .. }
    ));
    hold.release();
    assert!(matches!(
        terminal(&mut second).await?,
        TransferEvent::Completed { bytes: 262144, .. }
    ));
    assert_eq!(tokio::fs::read(&second_path).await?, bytes);
    first_queue.close().await?;
    second_queue.close().await?;
    first_sftp.close().await?;
    second_sftp.close().await?;
    first_session.close().await?;
    second_session.close().await?;
    Ok(())
}

#[tokio::test]
async fn before_io_pause_and_cancellation_release_local_destination_for_a_new_connection()
-> Result<(), Box<dyn Error>> {
    let first_server = serve().await?;
    let second_server = serve().await?;
    let first_session = SshSession::connect(options(&first_server)).await?;
    let second_session = SshSession::connect(options(&second_server)).await?;
    let first_sftp = Arc::new(first_session.sftp().await?);
    let second_sftp = Arc::new(second_session.sftp().await?);
    first_sftp.write("/source", b"first").await?;
    second_sftp.write("/source", b"second").await?;
    let root = tempfile::tempdir()?;
    let target = root.path().join("result");
    let first_queue = first_sftp.clone().transfer_queue();
    let second_queue = second_sftp.clone().transfer_queue();
    second_queue.set_parallelism(2)?;
    let mut first = first_queue
        .enqueue(TransferSpec::download("/source", &target))
        .await?;
    first.pause();
    loop {
        match next(&mut first).await? {
            TransferEvent::Paused { transferred: 0, .. } => break,
            TransferEvent::Failed { error, .. } | TransferEvent::Uncertain { error, .. } => {
                return Err(error.into());
            }
            TransferEvent::Completed { .. } | TransferEvent::Cancelled { .. } => {
                return Err("pre-I/O pause missed".into());
            }
            _ => {}
        }
    }
    assert!(!target.exists());
    let mut waiting = second_queue
        .enqueue(TransferSpec::download("/source", &target))
        .await?;
    assert!(matches!(
        next(&mut waiting).await?,
        TransferEvent::Queued { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), waiting.recv())
            .await
            .is_err()
    );
    let distinct_path = root.path().join("independent");
    let mut distinct = second_queue
        .enqueue(TransferSpec::download("/source", &distinct_path))
        .await?;
    assert!(matches!(
        terminal(&mut distinct).await?,
        TransferEvent::Completed { bytes: 6, .. }
    ));
    first.cancel();
    assert!(matches!(
        terminal(&mut first).await?,
        TransferEvent::Cancelled { bytes: 0, .. }
    ));
    assert!(matches!(
        terminal(&mut waiting).await?,
        TransferEvent::Completed { bytes: 6, .. }
    ));
    assert_eq!(tokio::fs::read(target).await?, b"second");
    first_queue.close().await?;
    second_queue.close().await?;
    first_sftp.close().await?;
    second_sftp.close().await?;
    first_session.close().await?;
    second_session.close().await?;
    Ok(())
}

#[test]
fn actual_parallel_atomic_transfers_run_on_a_two_mib_stack() -> Result<(), Box<dyn Error>> {
    let owned = std::thread::Builder::new()
        .name("parallel-sftp-2mib".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| -> Result<(), String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(30),
                    overlapping_existing_atomic_uploads(),
                )
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
            })
        })?;
    owned
        .join()
        .map_err(|_| "parallel transfer small-stack thread panicked")?
        .map_err(Into::into)
}

#[tokio::test]
async fn acknowledged_atomic_write_rejection_releases_destination_without_quarantine()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/rejected", b"preserved").await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    let bytes = vec![0x35; 128 * 1024];
    tokio::fs::write(&source, &bytes).await?;
    let queue = sftp.clone().transfer_queue();
    server.filesystem.set_atomic_write_failure(true);
    let mut rejected = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/rejected"))
        .await?;
    assert!(matches!(
        terminal(&mut rejected).await?,
        TransferEvent::Failed { .. }
    ));
    assert_eq!(sftp.read("/rejected", 100).await?, b"preserved");
    assert!(
        sftp.inspect_transfer_quarantine(&TransferSpec::upload(&source, "/rejected"))
            .await
            .is_err()
    );
    server.filesystem.set_atomic_write_failure(false);
    let mut reviewed = queue
        .enqueue_atomic_upload(TransferSpec::upload(&source, "/rejected"))
        .await?;
    assert!(matches!(
        terminal(&mut reviewed).await?,
        TransferEvent::Completed { bytes: 131072, .. }
    ));
    assert_eq!(sftp.read("/rejected", bytes.len()).await?, bytes);
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn acknowledged_missing_resume_create_rejection_is_failed_and_allows_new_review()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    tokio::fs::create_dir(&source).await?;
    tokio::fs::write(source.join("create-rejected"), b"reviewed bytes").await?;
    sftp.mkdir("/resume-root").await?;
    let spec = TransferSpec::upload(&source, "/resume-root");
    let plan = sftp.plan_directory_resume(spec.clone()).await?;
    let queue = sftp.clone().transfer_queue();
    server.filesystem.set_resume_create_rejection(true);
    let mut denied = queue.enqueue_directory_resume(plan).await?;
    assert!(matches!(
        terminal(&mut denied).await?,
        TransferEvent::Failed { .. }
    ));
    assert!(sftp.list("/resume-root").await?.is_empty());
    assert!(sftp.inspect_transfer_quarantine(&spec).await.is_err());
    server.filesystem.set_resume_create_rejection(false);
    let fresh_plan = sftp.plan_directory_resume(spec).await?;
    let mut accepted = queue.enqueue_directory_resume(fresh_plan).await?;
    assert!(matches!(
        terminal(&mut accepted).await?,
        TransferEvent::Completed { bytes: 14, .. }
    ));
    assert_eq!(
        sftp.read("/resume-root/create-rejected", 100).await?,
        b"reviewed bytes"
    );
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
