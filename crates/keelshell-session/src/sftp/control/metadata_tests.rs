//! Poll-scoped I/O observation must not become shared session activity.
use super::*;
use russh_sftp::{client::error::Error as SftpError, protocol::Status};

fn context(id: u64) -> (TransferContext, mpsc::Receiver<TransferEvent>) {
    let (events, receiver) = mpsc::channel(16);
    (
        TransferContext::new(id, None, events, Arc::new(TransferControl::new())),
        receiver,
    )
}

fn missing() -> SftpError {
    SftpError::Status(Status {
        id: 7,
        status_code: StatusCode::NoSuchFile,
        error_message: "missing test path".into(),
        language_tag: "en".into(),
    })
}

async fn reply() -> std::result::Result<(), SftpError> {
    // The real protocol is covered by the metadata TCP integration tests.
    // Here the typed completion boundary isolates task-local ownership.
    Ok(())
}

#[tokio::test]
async fn matched_reply_and_absence_renew_without_resolving_pending_mutation() -> Result<()> {
    let (context, _events) = context(1);
    let mut observed = context.confirmed_io.subscribe();
    context.mutation_pending.store(true, Ordering::Release);
    context
        .run(Duration::from_secs(1), "idle", async {
            remote_io(reply()).await.map_err(sftp_error)?;
            assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
            observed.borrow_and_update();
            assert!(matches!(remote_io(async { Err::<(), _>(missing()) }).await,
            Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile));
            assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
            observed.borrow_and_update();
            for error in [
                SftpError::Timeout,
                SftpError::UnexpectedPacket,
                SftpError::UnexpectedBehavior("malformed response".into()),
                SftpError::IO("closed".into()),
            ] {
                assert!(remote_io(async { Err::<(), _>(error) }).await.is_err());
                assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
            }
            assert!(context.mutation_pending());
            Ok(())
        })
        .await
        .map_err(transfer_session_error)?;
    assert!(context.mutation_pending());
    Ok(())
}

#[tokio::test]
async fn real_local_metadata_and_missing_path_renew_only_inside_owner_scope() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let file = temporary.path().join("source");
    tokio::fs::write(&file, b"content").await?;
    let (context, _events) = context(1);
    let mut observed = context.confirmed_io.subscribe();
    assert!(local_io(tokio::fs::metadata(&file)).await?.is_file());
    assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
    context
        .run(Duration::from_secs(1), "idle", async {
            assert_eq!(local_io(tokio::fs::metadata(&file)).await?.len(), 7);
            assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
            observed.borrow_and_update();
            assert!(
                matches!(local_io(tokio::fs::metadata(temporary.path().join("missing"))).await,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound)
            );
            assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
            observed.borrow_and_update();
            let failure = local_io(async {
                Err::<(), _>(std::io::Error::from(std::io::ErrorKind::TimedOut))
            })
            .await;
            assert!(failure.is_err());
            assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
            Ok(())
        })
        .await
        .map_err(transfer_session_error)?;
    Ok(())
}

async fn cadence() -> TransferExecutionResult<()> {
    for _ in 0..16 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        remote_io(reply()).await.map_err(sftp_error)?;
    }
    Ok(())
}

#[tokio::test]
async fn joined_owners_restore_poll_scope_and_cannot_renew_a_held_mutation() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let (held, _events) = context(1);
        let (timely, _events) = context(2);
        let began = tokio::time::Instant::now();
        let ((held_result, held_elapsed), timely_result) = tokio::join!(
            async {
                let result = held
                    .run(
                        Duration::from_millis(80),
                        "held idle",
                        held.remote_mutation(std::future::pending::<
                            std::result::Result<(), SftpError>,
                        >()),
                    )
                    .await;
                (result, began.elapsed())
            },
            timely.run(Duration::from_millis(80), "timely idle", cadence()),
        );
        assert!(matches!(
            held_result,
            Err(TransferExecutionError::Error(SessionError::Timeout(
                "held idle"
            )))
        ));
        timely_result.map_err(transfer_session_error)?;
        assert!(held_elapsed < Duration::from_millis(200));
        assert!(began.elapsed() >= Duration::from_millis(320));
        assert!(held.mutation_pending());
        assert!(!timely.mutation_pending());
        Ok(())
    })
    .await
    .map_err(|_| SessionError::Timeout("test hard bound"))?
}

#[tokio::test]
async fn selected_owners_restore_scope_between_sibling_future_polls() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let (held, _events) = context(1);
        let (timely, _events) = context(2);
        let mut held_observed = held.confirmed_io.subscribe();
        let timely_observed = timely.confirmed_io.subscribe();
        let result = tokio::select! {
            result = held.run::<_, ()>(Duration::from_millis(80), "held idle", std::future::pending()) => result,
            result = timely.run(Duration::from_millis(80), "timely idle", cadence()) => {
                result.map_err(transfer_session_error)?;
                return Err(SessionError::Worker);
            }
        };
        assert!(matches!(result, Err(TransferExecutionError::Error(SessionError::Timeout("held idle")))));
        assert!(!held_observed.has_changed().map_err(|_| SessionError::Closed)?);
        held_observed.borrow_and_update();
        assert!(timely_observed.has_changed().map_err(|_| SessionError::Closed)?);
        Ok(())
    }).await.map_err(|_| SessionError::Timeout("test hard bound"))?
}

#[tokio::test]
async fn nested_owner_overrides_then_restores_outer_owner() -> Result<()> {
    let (outer, _events) = context(1);
    let (inner, _events) = context(2);
    let mut outer_observed = outer.confirmed_io.subscribe();
    let mut inner_observed = inner.confirmed_io.subscribe();
    outer
        .run(Duration::from_secs(1), "outer idle", async {
            remote_io(reply()).await.map_err(sftp_error)?;
            assert!(
                outer_observed
                    .has_changed()
                    .map_err(|_| SessionError::Closed)?
            );
            outer_observed.borrow_and_update();
            assert!(
                !inner_observed
                    .has_changed()
                    .map_err(|_| SessionError::Closed)?
            );
            inner
                .run(Duration::from_secs(1), "inner idle", async {
                    tokio::task::yield_now().await;
                    remote_io(reply()).await.map_err(sftp_error)?;
                    assert!(
                        inner_observed
                            .has_changed()
                            .map_err(|_| SessionError::Closed)?
                    );
                    inner_observed.borrow_and_update();
                    assert!(
                        !outer_observed
                            .has_changed()
                            .map_err(|_| SessionError::Closed)?
                    );
                    Ok(())
                })
                .await?;
            tokio::task::yield_now().await;
            remote_io(reply()).await.map_err(sftp_error)?;
            assert!(
                outer_observed
                    .has_changed()
                    .map_err(|_| SessionError::Closed)?
            );
            assert!(
                !inner_observed
                    .has_changed()
                    .map_err(|_| SessionError::Closed)?
            );
            Ok(())
        })
        .await
        .map_err(transfer_session_error)?;
    Ok(())
}

#[tokio::test]
async fn spawned_local_metadata_cannot_inherit_or_renew_parent_scope() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let temporary = tempfile::tempdir()?;
        let file = temporary.path().join("source");
        tokio::fs::write(&file, b"content").await?;
        let (parent, _events) = context(1);
        let observed = parent.confirmed_io.subscribe();
        let completions = Arc::new(AtomicU64::new(0));
        let child_completions = completions.clone();
        let mut child = None;
        let began = tokio::time::Instant::now();
        let result = parent
            .run(Duration::from_millis(80), "parent idle", async {
                child = Some(tokio::spawn(async move {
                    for _ in 0..20 {
                        local_io(tokio::fs::metadata(&file)).await?;
                        child_completions.fetch_add(1, Ordering::AcqRel);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Ok::<(), std::io::Error>(())
                }));
                parent
                    .remote_mutation(std::future::pending::<std::result::Result<(), SftpError>>())
                    .await
            })
            .await;
        assert!(matches!(
            result,
            Err(TransferExecutionError::Error(SessionError::Timeout(
                "parent idle"
            )))
        ));
        assert!(began.elapsed() < Duration::from_millis(250));
        assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
        assert!(parent.mutation_pending());
        child
            .ok_or(SessionError::Worker)?
            .await
            .map_err(|_| SessionError::Worker)??;
        assert_eq!(completions.load(Ordering::Acquire), 20);
        Ok(())
    })
    .await
    .map_err(|_| SessionError::Timeout("test hard bound"))?
}

#[tokio::test]
async fn typed_metadata_replies_cannot_extend_fixed_validation_deadline() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let (context, _events) = context(1);
        let began = tokio::time::Instant::now();
        let result = context
            .run(Duration::from_millis(20), "idle", async {
                context
                    .validation::<()>(Duration::from_millis(80), "fixed validation", async {
                        loop {
                            remote_io(reply()).await.map_err(sftp_error)?;
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
            })
            .await;
        assert!(matches!(
            result,
            Err(TransferExecutionError::Error(SessionError::Timeout(
                "fixed validation"
            )))
        ));
        assert!(began.elapsed() < Duration::from_millis(250));
        assert!(!context.mutation_pending());
        assert!(!*context.validating.borrow());
        Ok(())
    })
    .await
    .map_err(|_| SessionError::Timeout("test hard bound"))?
}
