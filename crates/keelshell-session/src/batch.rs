//! Bounded independent SSH exec jobs on already authenticated connections.
//! Commands are immutable reviewed text. Jobs never reconnect, retry, request a
//! PTY, replay terminal input, or infer the interactive shell's environment.

mod scheduler;
#[cfg(test)]
mod tests;

use crate::SshSession;
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use uuid::Uuid;

/// Maximum number of distinct target identities in one batch.
pub const MAX_BATCH_TARGETS: usize = 32;
/// Maximum aggregate captured stdout/stderr payload across all rows.
/// Protocol buffers, command text and small scheduler metadata are additional.
pub const MAX_BATCH_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
/// Maximum command size per target in UTF-8 bytes.
pub const MAX_BATCH_COMMAND_BYTES: usize = 64 * 1024;

/// One immutable command and an already authenticated, explicitly chosen session.
/// Deliberately has no Debug implementation, avoiding incidental command logging.
pub struct BatchTarget {
    /// Caller-owned identity; must be unique within the batch.
    pub id: Uuid,
    /// Captured connection. Later UI selection changes cannot redirect this job.
    pub session: SshSession,
    /// Exact command bytes, including whitespace and newlines; never trimmed.
    pub command: String,
}

/// Whether an observed failed or uncertain row prevents further admissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BatchPolicy {
    /// Continue starting queued rows regardless of earlier outcomes.
    #[default]
    Continue,
    /// Stop queued rows after the first nonzero exit, rejection or uncertain
    /// outcome. Already admitted rows continue collecting their own results.
    StopAfterFailure,
}

/// Per-batch limits, checked completely before any channel is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchOptions {
    /// Concurrent admitted rows, from one through eight; default four.
    pub concurrency: usize,
    /// One through 300 seconds per admitted row, covering OPEN, exec and output.
    /// Queued time is excluded. Existing bounded channel cleanup may follow it.
    pub timeout: Duration,
    /// Combined stdout/stderr bytes per row, default one MiB. Target count times
    /// this limit must not exceed [`MAX_BATCH_OUTPUT_BYTES`].
    pub output_limit: usize,
    /// Failure admission policy.
    pub policy: BatchPolicy,
}
impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            concurrency: 4,
            timeout: Duration::from_secs(30),
            output_limit: 1024 * 1024,
            policy: BatchPolicy::Continue,
        }
    }
}

/// Proven reason why this row did not submit an exec request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchNotStartedReason {
    /// Cancelled before exec submission, including queued rows.
    Cancelled,
    /// A prior row triggered the stop-after-failure admission policy.
    StoppedAfterFailure,
    /// Deadline expired before exec submission.
    Timeout,
    /// The server explicitly rejected channel opening.
    ChannelRejected,
    /// The connection could not open a usable channel.
    ConnectionLost,
    /// The scheduler failed before admitting this row.
    WorkerFailed,
}

/// A request may have reached the peer, but its complete outcome is unconfirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchUnknownReason {
    /// Local cancellation does not prove termination of a remote process.
    Cancelled,
    /// The per-row deadline expired after exec may have been submitted.
    Timeout,
    /// The channel or connection stopped without a complete result.
    ConnectionLost,
    /// The channel closed without an exit-status response.
    NoExitStatus,
    /// Captured stdout/stderr reached the configured bound and more data arrived.
    OutputLimit,
    /// The peer reported signal termination rather than an exit code.
    RemoteSignal,
    /// Contradictory or unsupported request/response ordering.
    Protocol,
    /// An admitted worker stopped unexpectedly.
    WorkerFailed,
}

/// Final execution classification. Only `Exited { code: 0 }` means success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchOutcome {
    /// The peer sent this exit status and its output channel then ended.
    Exited {
        /// Zero is success; any other value is a confirmed command failure.
        code: u32,
    },
    /// The peer explicitly rejected the exec request before acknowledging it.
    Rejected,
    /// No exec request was submitted by this row.
    NotStarted {
        /// Admission, opening or cancellation reason.
        reason: BatchNotStartedReason,
    },
    /// Exec may have been submitted; cancellation is not remote rollback.
    Unknown {
        /// Fixed classification without peer-provided diagnostic text.
        reason: BatchUnknownReason,
    },
}
impl BatchOutcome {
    /// Whether a complete zero exit status was observed.
    pub fn is_success(self) -> bool {
        self == Self::Exited { code: 0 }
    }
}

/// A completed row, preserving bounded bytes collected before its terminal state.
#[derive(Debug, PartialEq, Eq)]
pub struct BatchRowReceipt {
    /// Original immutable target identity.
    pub id: Uuid,
    /// Confirmed or uncertain outcome.
    pub outcome: BatchOutcome,
    /// Raw stdout bytes. UI consumers must escape controls and handle invalid UTF-8.
    pub stdout: Vec<u8>,
    /// Raw stderr bytes; shares the same per-row limit with stdout.
    pub stderr: Vec<u8>,
}

/// At most one Started and one Finished event per row, without output duplication.
#[derive(Debug, Clone)]
pub enum BatchEvent {
    /// The scheduler admitted this row into a concurrency slot; OPEN may still
    /// be pending, so this alone does not prove remote command execution.
    Started {
        /// Target identity.
        id: Uuid,
    },
    /// Final row receipt, shared with the final aggregate receipt.
    Finished {
        /// Immutable bounded output and outcome.
        row: Arc<BatchRowReceipt>,
    },
}

/// Complete batch receipt in the original input order.
#[derive(Debug)]
pub struct BatchReceipt {
    /// Exactly one terminal receipt for each validated target.
    pub rows: Vec<Arc<BatchRowReceipt>>,
    /// Whether user cancellation was observed while the batch was active.
    pub cancelled: bool,
    /// Whether the configured policy stopped further admissions after a failure.
    pub stopped_after_failure: bool,
}

/// Fixed validation and ownership failures; does not include command or peer text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BatchError {
    /// A batch requires from one through 32 targets.
    #[error("batch requires one through 32 targets")]
    InvalidTargets,
    /// Multiple targets had the same caller identity.
    #[error("batch target identities must be unique")]
    DuplicateTarget,
    /// Concurrency must be in the supported range.
    #[error("batch concurrency must be one through eight")]
    InvalidConcurrency,
    /// Per-row timeout must be between one and 300 seconds.
    #[error("batch timeout must be one through 300 seconds")]
    InvalidTimeout,
    /// Output allocation would exceed the aggregate capture bound.
    #[error("batch output limit exceeds the aggregate budget")]
    InvalidOutputLimit,
    /// Empty, NUL-containing or oversized command.
    #[error("batch command is empty, contains NUL or exceeds 64 KiB")]
    InvalidCommand,
    /// Starting requires an active Tokio runtime.
    #[error("batch requires an active Tokio runtime")]
    RuntimeUnavailable,
    /// The owning worker could not return its aggregate receipt.
    #[error("batch worker receipt is unavailable")]
    WorkerLost,
}

/// Owns the scheduler and all admitted jobs. Events never impose backpressure:
/// the queue can hold every possible state transition (at most 64 events).
///
/// [`Self::cancel`] preserves receipts for collection. Dropping the handle aborts
/// its scheduler and admitted tasks; their independent channel guards then own
/// bounded cleanup. Neither operation proves that a remote process was killed.
pub struct BatchHandle {
    cancel: watch::Sender<bool>,
    events: mpsc::Receiver<BatchEvent>,
    receipt: Option<oneshot::Receiver<BatchReceipt>>,
    worker: JoinHandle<()>,
}
impl BatchHandle {
    /// Stop queued admissions and request cancellation of every admitted row.
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    /// Receive the next bounded state event, or None when all senders have closed.
    pub async fn recv(&mut self) -> Option<BatchEvent> {
        self.events.recv().await
    }
    /// Poll events without blocking a native UI thread.
    pub fn try_recv(&mut self) -> Result<BatchEvent, mpsc::error::TryRecvError> {
        self.events.try_recv()
    }
    /// Whether the owned scheduler has stopped. Drain remaining events before
    /// discarding the handle if individual row receipts are needed.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    /// Wait for the complete receipt without requiring events to be consumed.
    /// Cancelling this future drops the handle and aborts its owned worker.
    pub async fn finish(mut self) -> Result<BatchReceipt, BatchError> {
        self.receipt
            .take()
            .ok_or(BatchError::WorkerLost)?
            .await
            .map_err(|_| BatchError::WorkerLost)
    }
}
impl Drop for BatchHandle {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        self.worker.abort();
    }
}

/// Validate an entire immutable request, then start an owned bounded scheduler.
/// Duplicate IDs are rejected rather than silently executing a target twice.
/// Multiple IDs may intentionally refer to clones of the same authenticated SSH
/// session; cleanup normally closes only each job's independent channel. An
/// unconfirmed OPEN or failed bounded CLOSE can require shutting down that shared
/// connection under the existing transport ownership contract.
pub fn start_batch(
    targets: Vec<BatchTarget>,
    options: BatchOptions,
) -> Result<BatchHandle, BatchError> {
    validate(&targets, options)?;
    let runtime =
        tokio::runtime::Handle::try_current().map_err(|_| BatchError::RuntimeUnavailable)?;
    let (cancel, cancelled) = watch::channel(false);
    let (send, events) = mpsc::channel(MAX_BATCH_TARGETS * 2);
    let (done, receipt) = oneshot::channel();
    let worker = runtime.spawn(scheduler::run(targets, options, cancelled, send, done));
    Ok(BatchHandle {
        cancel,
        events,
        receipt: Some(receipt),
        worker,
    })
}
fn validate(targets: &[BatchTarget], options: BatchOptions) -> Result<(), BatchError> {
    if targets.is_empty() || targets.len() > MAX_BATCH_TARGETS {
        return Err(BatchError::InvalidTargets);
    }
    if !(1..=8).contains(&options.concurrency) {
        return Err(BatchError::InvalidConcurrency);
    }
    if options.timeout < Duration::from_secs(1) || options.timeout > Duration::from_secs(300) {
        return Err(BatchError::InvalidTimeout);
    }
    if options.output_limit == 0 || options.output_limit > MAX_BATCH_OUTPUT_BYTES / targets.len() {
        return Err(BatchError::InvalidOutputLimit);
    }
    let mut ids = HashSet::with_capacity(targets.len());
    for target in targets {
        if !ids.insert(target.id) {
            return Err(BatchError::DuplicateTarget);
        }
        if target.command.trim().is_empty()
            || target.command.len() > MAX_BATCH_COMMAND_BYTES
            || target.command.contains('\0')
        {
            return Err(BatchError::InvalidCommand);
        }
    }
    Ok(())
}

pub(crate) async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
