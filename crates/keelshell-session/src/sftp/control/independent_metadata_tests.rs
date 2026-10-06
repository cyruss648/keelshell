//! Independent ownership probes for interrupted and lazily constructed futures.
use super::*;
use russh_sftp::client::error::Error as SftpError;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

fn owner(id: u64) -> (TransferContext, mpsc::Receiver<TransferEvent>) {
    let (events, receiver) = mpsc::channel(8);
    (
        TransferContext::new(id, None, events, Arc::new(TransferControl::new())),
        receiver,
    )
}

// Tokio also reinstalls the owning scope when dropping the owned future. This
// cleanup observation must stay with that owner, without leaking to siblings.
struct PendingCleanup;
impl Future for PendingCleanup {
    type Output = TransferExecutionResult<()>;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
impl Drop for PendingCleanup {
    fn drop(&mut self) {
        observed_io();
    }
}

#[tokio::test]
async fn pending_poll_and_owned_drop_restore_the_ambient_scope() -> Result<()> {
    let (active, _events) = owner(1);
    let (sibling, _events) = owner(2);
    let observed = active.confirmed_io.subscribe();
    let sibling_observed = sibling.confirmed_io.subscribe();
    active.mutation_pending.store(true, Ordering::Release);
    let mut running = Box::pin(active.run(Duration::from_secs(1), "idle", PendingCleanup));
    std::future::poll_fn(|cx| {
        assert!(running.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(CURRENT_IO.try_with(|_| ()).is_err());
    remote_io(async { Ok::<_, SftpError>(()) })
        .await
        .map_err(sftp_error)?;
    assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
    drop(running);
    assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
    assert!(CURRENT_IO.try_with(|_| ()).is_err());
    assert!(
        !sibling_observed
            .has_changed()
            .map_err(|_| SessionError::Closed)?
    );
    assert!(active.mutation_pending());
    Ok(())
}

#[tokio::test]
async fn unwinding_a_polled_operation_restores_scope_and_retains_unknown_marker() -> Result<()> {
    let (active, _events) = owner(1);
    let mut observed = active.confirmed_io.subscribe();
    active.mutation_pending.store(true, Ordering::Release);
    let operation = std::future::poll_fn(|_| -> Poll<TransferExecutionResult<()>> {
        observed_io();
        panic!("controlled independent ownership unwind");
    });
    let mut running = Box::pin(active.run(Duration::from_secs(1), "idle", operation));
    let caught = std::future::poll_fn(|cx| {
        Poll::Ready(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || running.as_mut().poll(cx),
        )))
    })
    .await;
    assert!(caught.is_err());
    assert!(CURRENT_IO.try_with(|_| ()).is_err());
    assert!(observed.has_changed().map_err(|_| SessionError::Closed)?);
    observed.borrow_and_update();
    drop(running);
    remote_io(async { Ok::<_, SftpError>(()) })
        .await
        .map_err(sftp_error)?;
    assert!(!observed.has_changed().map_err(|_| SessionError::Closed)?);
    assert!(active.mutation_pending());
    Ok(())
}

#[tokio::test]
async fn constructing_io_inside_an_owner_does_not_capture_that_owner() -> Result<()> {
    let (constructor, _events) = owner(1);
    let (executor, _events) = owner(2);
    let constructor_observed = constructor.confirmed_io.subscribe();
    let executor_observed = executor.confirmed_io.subscribe();
    let deferred = constructor
        .run(Duration::from_secs(1), "constructor idle", async {
            Ok(remote_io(async { Ok::<_, SftpError>(42_u64) }))
        })
        .await
        .map_err(transfer_session_error)?;
    assert!(
        !constructor_observed
            .has_changed()
            .map_err(|_| SessionError::Closed)?
    );
    let value = executor
        .run(Duration::from_secs(1), "executor idle", async {
            deferred.await.map_err(sftp_error).map_err(Into::into)
        })
        .await
        .map_err(transfer_session_error)?;
    assert_eq!(value, 42);
    assert!(
        !constructor_observed
            .has_changed()
            .map_err(|_| SessionError::Closed)?
    );
    assert!(
        executor_observed
            .has_changed()
            .map_err(|_| SessionError::Closed)?
    );
    assert!(CURRENT_IO.try_with(|_| ()).is_err());
    Ok(())
}

#[tokio::test]
async fn pending_nested_owner_and_cancellation_restore_outer_local_io_ownership() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("source");
    tokio::fs::write(&path, b"independent local observation").await?;
    let (outer, _events) = owner(1);
    let (inner, _events) = owner(2);
    let mut outer_observed = outer.confirmed_io.subscribe();
    let inner_observed = inner.confirmed_io.subscribe();
    outer
        .run(Duration::from_secs(1), "outer idle", async {
            let mut running = Box::pin(inner.run(
                Duration::from_secs(1),
                "inner idle",
                inner.remote_mutation(std::future::pending::<std::result::Result<(), SftpError>>()),
            ));
            std::future::poll_fn(|cx| {
                assert!(running.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            assert!(inner.mutation_pending());
            assert!(local_io(tokio::fs::metadata(&path)).await?.is_file());
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
            inner.control.cancel();
            assert!(matches!(
                running.await,
                Err(TransferExecutionError::Cancelled(0))
            ));
            assert!(local_io(tokio::fs::metadata(&path)).await?.is_file());
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
            assert!(inner.mutation_pending());
            Ok(())
        })
        .await
        .map_err(transfer_session_error)?;
    assert!(CURRENT_IO.try_with(|_| ()).is_err());
    assert!(inner.mutation_pending());
    Ok(())
}
