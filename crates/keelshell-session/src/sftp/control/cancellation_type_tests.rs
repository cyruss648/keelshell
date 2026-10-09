//! Cancel during the current operation poll, after the outer listener was polled.
//! These controls exercise the actual atomic writer's safe points and queue
//! terminal classifier; no timing sleep or detached process chooses the race.
use super::*;

fn context() -> (TransferContext, mpsc::Receiver<TransferEvent>) {
    let (events, receiver) = mpsc::channel(16);
    let context = TransferContext::new(41, None, events, Arc::new(TransferControl::new()));
    context.transferred.store(32_768, Ordering::Release);
    (context, receiver)
}

#[tokio::test]
async fn atomic_checkpoint_cancelled_in_current_poll_keeps_cancelled_terminal() -> Result<()> {
    let (context, _receiver) = context();
    let mut operation_polled = false;
    let result = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                // biased run has already polled its false cancellation branch.
                // Sending here makes the shared writer observe it in this poll.
                operation_polled = true;
                context.control.cancel();
                writer_checkpoint(Some(&context)).await
            },
        )
        .await;
    assert!(operation_polled);
    assert!(matches!(
        &result,
        Err(TransferExecutionError::Cancelled(32_768))
    ));
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), true, result),
        TransferEvent::Cancelled {
            id: 41,
            bytes: 32_768
        }
    ));
    Ok(())
}

#[tokio::test]
async fn atomic_progress_cancelled_in_current_poll_keeps_acknowledged_bytes() -> Result<()> {
    let (context, _receiver) = context();
    let mut operation_polled = false;
    let result = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                operation_polled = true;
                context.control.cancel();
                writer_progress(Some(&context), 7).await
            },
        )
        .await;
    assert!(operation_polled);
    assert!(matches!(
        &result,
        Err(TransferExecutionError::Cancelled(32_775))
    ));
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), true, result),
        TransferEvent::Cancelled {
            id: 41,
            bytes: 32_775
        }
    ));
    Ok(())
}

#[tokio::test]
async fn legacy_session_error_mapping_loses_cancelled_identity_in_current_poll() -> Result<()> {
    let (context, _receiver) = context();
    let mut operation_polled = false;
    let result: TransferExecutionResult<()> = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                operation_polled = true;
                context.control.cancel();
                // The retained public Result mapper is deliberately lossy. This
                // demonstrates why it cannot be used inside the queued writer.
                writer_checkpoint(Some(&context))
                    .await
                    .map_err(transfer_session_error)
                    .map_err(Into::into)
            },
        )
        .await;
    assert!(operation_polled);
    assert!(matches!(
        &result,
        Err(TransferExecutionError::Error(SessionError::Closed))
    ));
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), true, result),
        TransferEvent::Failed { id: 41, .. }
    ));
    Ok(())
}

#[tokio::test]
async fn real_closed_in_current_poll_stays_failed_despite_cancellation_flag() -> Result<()> {
    let (context, _receiver) = context();
    let result: TransferExecutionResult<()> = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                context.control.cancel();
                Err(SessionError::Closed.into())
            },
        )
        .await;
    assert!(matches!(
        &result,
        Err(TransferExecutionError::Error(SessionError::Closed))
    ));
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), true, result),
        TransferEvent::Failed { id: 41, .. }
    ));
    Ok(())
}

#[tokio::test]
async fn real_io_error_stays_failed_despite_cancellation_flag() -> Result<()> {
    let (context, _receiver) = context();
    let result: TransferExecutionResult<()> = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                context.control.cancel();
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into())
            },
        )
        .await;
    assert!(
        matches!(&result, Err(TransferExecutionError::Error(SessionError::Io(error)))
        if error.kind() == std::io::ErrorKind::PermissionDenied)
    );
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), true, result),
        TransferEvent::Failed { id: 41, .. }
    ));
    Ok(())
}

#[tokio::test]
async fn cancelled_safe_point_cannot_clear_unknown_destination_result() -> Result<()> {
    let (context, _receiver) = context();
    let result = context
        .run(
            Duration::from_secs(1),
            "typed cancellation control",
            async {
                context.control.cancel();
                writer_checkpoint(Some(&context)).await
            },
        )
        .await;
    assert!(matches!(
        &result,
        Err(TransferExecutionError::Cancelled(32_768))
    ));
    assert!(matches!(
        queue::terminal_transfer_event(41, context.bytes(), false, result),
        TransferEvent::Uncertain {
            id: 41,
            bytes: 32_768,
            ..
        }
    ));
    Ok(())
}
