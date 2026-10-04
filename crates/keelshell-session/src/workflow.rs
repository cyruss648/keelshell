//! Reviewed dependency plans executed on captured authenticated SSH sessions.
//!
//! This adapter never edits commands, resolves a new target, reconnects, retries,
//! requests a PTY or launches timers for recurring work. Confirmation must come
//! from an explicit human review; the core receipt proves content consistency,
//! not that a UI performed that review.

mod scheduler;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use keelshell_core::{
    BatchTaskSkipReason, BatchWorkflowError, BatchWorkflowReviewToken, ConfirmedBatchWorkflow,
    MAX_BATCH_WORKFLOW_TARGETS,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    BatchOptions, BatchPolicy, BatchRowReceipt, SshSession, batch::MAX_BATCH_OUTPUT_BYTES,
};

/// One caller-reviewed identity bound to the already authenticated connection.
///
/// The caller must choose the exact session shown in the human review. The
/// adapter owns this value, so later changes to a UI session map cannot redirect
/// an existing workflow. No Debug implementation exposes connection metadata.
pub struct WorkflowBinding {
    /// Identity matching a target_id in the confirmed plan.
    pub id: Uuid,
    /// Captured authenticated session; no endpoint is resolved during execution.
    pub session: SshSession,
}

/// Limits validated for the complete task graph before opening any channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowOptions {
    /// Concurrent admitted tasks, from one through eight; default four.
    pub concurrency: usize,
    /// One through 300 seconds per admitted task, excluding dependency wait.
    /// Existing bounded channel cleanup may follow this deadline.
    pub timeout: Duration,
    /// Combined stdout/stderr payload per task; default 256 KiB. Task count
    /// times this limit cannot exceed the existing 32 MiB batch capture budget.
    pub output_limit: usize,
    /// Whether an unsuccessful task stops unrelated pending branches.
    pub policy: BatchPolicy,
}

impl Default for WorkflowOptions {
    fn default() -> Self {
        Self {
            concurrency: 4,
            timeout: Duration::from_secs(30),
            output_limit: 256 * 1024,
            policy: BatchPolicy::Continue,
        }
    }
}

impl WorkflowOptions {
    fn batch(self) -> BatchOptions {
        BatchOptions {
            concurrency: self.concurrency,
            timeout: self.timeout,
            output_limit: self.output_limit,
            policy: self.policy,
        }
    }
}

/// Terminal task evidence, retaining the transport's exact outcome semantics.
#[derive(Debug, PartialEq, Eq)]
pub enum WorkflowTaskResult {
    /// An admitted transport attempt. The row may still prove NotStarted;
    /// admission is never presented as proof of remote execution.
    Transport {
        /// Bounded stdout/stderr and the observed or uncertain transport result.
        /// Its id is the task identity, not a target/profile identity.
        row: Arc<BatchRowReceipt>,
    },
    /// The task was never admitted to the transport.
    Skipped {
        /// Explicit cancellation/policy or unsuccessful prerequisite.
        reason: BatchTaskSkipReason,
    },
}

/// Exactly one terminal record for a task in the immutable reviewed graph.
#[derive(Debug, PartialEq, Eq)]
pub struct WorkflowTaskReceipt {
    /// Reviewed task identity.
    pub id: Uuid,
    /// Captured target binding identity.
    pub target_id: Uuid,
    /// Transport evidence or proven non-admission reason.
    pub result: WorkflowTaskResult,
}

/// At most one admission and one terminal event per task; no output duplication.
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    /// Locally admitted; OPEN or exec may still be pending.
    Started {
        /// Reviewed task identity.
        id: Uuid,
        /// Reviewed connection binding identity.
        target_id: Uuid,
    },
    /// Shared with the final receipt so bounded output has one allocation.
    Finished {
        /// Exact terminal evidence for the task.
        task: Arc<WorkflowTaskReceipt>,
    },
}

/// Complete task receipt in the confirmed plan's deterministic topological order.
///
/// Task counts must not be stored as target counts in the existing batch audit;
/// several tasks can intentionally share one reviewed target binding.
#[derive(Debug)]
pub struct WorkflowReceipt {
    /// One terminal receipt per validated task, including never-admitted tasks.
    pub tasks: Vec<Arc<WorkflowTaskReceipt>>,
    /// Whether local cancellation was observed while execution was active.
    pub cancelled: bool,
    /// Whether failure policy prevented admission of any pending task.
    pub stopped_after_failure: bool,
    /// Exact core review fingerprint associated with this execution.
    pub fingerprint: BatchWorkflowReviewToken,
    /// Immutable execution limits and policy actually used. The core content
    /// fingerprint does not cover these options; callers must review them too.
    pub options: WorkflowOptions,
}

/// Fixed validation/ownership failures; no commands or peer diagnostics included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowError {
    /// Binding count is outside 1–32 or a binding identity is nil.
    #[error("workflow requires one through 32 non-nil bindings")]
    InvalidBindings,
    /// A caller supplied the same binding identity more than once.
    #[error("duplicate workflow binding: {id}")]
    DuplicateBinding {
        /// Repeated identity.
        id: Uuid,
    },
    /// A reviewed target has no supplied connection.
    #[error("missing workflow binding: {id}")]
    MissingBinding {
        /// Reviewed target identity.
        id: Uuid,
    },
    /// A supplied connection is outside the confirmed plan.
    #[error("unexpected workflow binding: {id}")]
    UnexpectedBinding {
        /// Unreviewed binding identity.
        id: Uuid,
    },
    /// An authenticated connection is already closed before scheduler creation.
    #[error("closed workflow binding: {id}")]
    ClosedBinding {
        /// Unusable captured connection identity.
        id: Uuid,
    },
    /// Concurrency must remain in the existing bounded transport range.
    #[error("workflow concurrency must be one through eight")]
    InvalidConcurrency,
    /// Each admitted task needs a bounded deadline.
    #[error("workflow timeout must be one through 300 seconds")]
    InvalidTimeout,
    /// Per-task output allocation exceeds the graph's aggregate capture budget.
    #[error("workflow output limit exceeds the aggregate budget")]
    InvalidOutputLimit,
    /// Scheduler creation needs an active Tokio runtime.
    #[error("workflow requires an active Tokio runtime")]
    RuntimeUnavailable,
    /// The owned scheduler could not return its final receipt.
    #[error("workflow worker receipt is unavailable")]
    WorkerLost,
    /// A ledger operation failed closed; no additional tasks are admitted.
    #[error("workflow ledger rejected an operation: {0}")]
    Ledger(#[from] BatchWorkflowError),
}

/// Owns every admitted task and its scheduler.
///
/// Events do not require consumption to complete: their queue holds all possible
/// transitions, at most 256. Cancel requests preserve receipts for collection.
/// Dropping aborts owned tasks; existing independent channel guards then own
/// bounded cleanup. Neither operation proves a remote process was terminated.
pub struct WorkflowHandle {
    cancel: watch::Sender<bool>,
    events: mpsc::Receiver<WorkflowEvent>,
    receipt: Option<oneshot::Receiver<Result<WorkflowReceipt, WorkflowError>>>,
    worker: JoinHandle<()>,
}

impl WorkflowHandle {
    /// Prevent pending admission and cancel active transport waits.
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    /// Wait for the next bounded state event, or None after all senders close.
    pub async fn recv(&mut self) -> Option<WorkflowEvent> {
        self.events.recv().await
    }
    /// Poll events without blocking a native UI thread.
    pub fn try_recv(&mut self) -> Result<WorkflowEvent, mpsc::error::TryRecvError> {
        self.events.try_recv()
    }
    /// Whether the owning scheduler has stopped; queued events may remain.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
    /// Collect a ready aggregate without blocking, returning `None` while pending.
    ///
    /// A returned receipt consumes the aggregate exactly once; queued events are
    /// still available. Calling again returns `WorkerLost`, as does `finish`
    /// after this method has collected the receipt. This permits a native UI to
    /// reconcile its event rows with the authoritative complete aggregate.
    pub fn try_finish(&mut self) -> Option<Result<WorkflowReceipt, WorkflowError>> {
        let Some(receiver) = self.receipt.as_mut() else {
            return Some(Err(WorkflowError::WorkerLost));
        };
        match receiver.try_recv() {
            Ok(receipt) => {
                self.receipt = None;
                Some(receipt)
            }
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.receipt = None;
                Some(Err(WorkflowError::WorkerLost))
            }
        }
    }
    /// Collect the complete receipt without consuming events. Cancelling this
    /// future drops the handle and aborts its owned scheduler and task set.
    pub async fn finish(mut self) -> Result<WorkflowReceipt, WorkflowError> {
        self.receipt
            .take()
            .ok_or(WorkflowError::WorkerLost)?
            .await
            .map_err(|_| WorkflowError::WorkerLost)?
    }
}

impl Drop for WorkflowHandle {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        self.worker.abort();
    }
}

/// Validate every captured binding and resource limit, then start reviewed work.
///
/// Commands come only from the consumed immutable core receipt. Each target UUID
/// must have exactly one supplied authenticated session; extra, duplicate,
/// missing and already-closed bindings are rejected before any channel opens.
/// The caller must also display and explicitly review the captured bindings and
/// execution options; neither is authenticated by the core content fingerprint.
/// Several task IDs may intentionally use the same captured session. Existing
/// bounded channel cleanup can close a shared connection after an uncertain OPEN
/// or unsuccessful CLOSE; this never substitutes a different connection.
///
/// ```no_run
/// use keelshell_core::{BatchTaskSpec, BatchWorkflowPlan};
/// use keelshell_session::{SshSession, WorkflowBinding, WorkflowOptions, start_workflow};
/// use uuid::Uuid;
/// # fn run(session: SshSession) -> Result<(), Box<dyn std::error::Error>> {
/// let target = Uuid::from_u128(1);
/// let plan = BatchWorkflowPlan::new(vec![BatchTaskSpec {
///     id: Uuid::from_u128(2), target_id: target,
///     command: "printf 'reviewed check'".into(), dependencies: vec![],
/// }])?;
/// // Display this plan and the exact captured session, then confirm explicitly.
/// let token = plan.review_token();
/// let handle = start_workflow(plan.confirm(token)?,
///     vec![WorkflowBinding { id: target, session }], WorkflowOptions::default())?;
/// // Keep the handle until results are collected; this example cancels instead.
/// handle.cancel();
/// # Ok(())
/// # }
/// ```
pub fn start_workflow(
    confirmed: ConfirmedBatchWorkflow,
    bindings: Vec<WorkflowBinding>,
    options: WorkflowOptions,
) -> Result<WorkflowHandle, WorkflowError> {
    if bindings.is_empty() || bindings.len() > MAX_BATCH_WORKFLOW_TARGETS {
        return Err(WorkflowError::InvalidBindings);
    }
    if !(1..=8).contains(&options.concurrency) {
        return Err(WorkflowError::InvalidConcurrency);
    }
    if options.timeout < Duration::from_secs(1) || options.timeout > Duration::from_secs(300) {
        return Err(WorkflowError::InvalidTimeout);
    }
    let task_count = confirmed.plan().tasks().len();
    if options.output_limit == 0 || options.output_limit > MAX_BATCH_OUTPUT_BYTES / task_count {
        return Err(WorkflowError::InvalidOutputLimit);
    }
    let mut captured = BTreeMap::new();
    for binding in bindings {
        if binding.id.is_nil() {
            return Err(WorkflowError::InvalidBindings);
        }
        if captured.insert(binding.id, binding.session).is_some() {
            return Err(WorkflowError::DuplicateBinding { id: binding.id });
        }
    }
    for task in confirmed.plan().tasks() {
        if !captured.contains_key(&task.target_id) {
            return Err(WorkflowError::MissingBinding { id: task.target_id });
        }
    }
    for (id, session) in &captured {
        if !confirmed
            .plan()
            .tasks()
            .iter()
            .any(|task| task.target_id == *id)
        {
            return Err(WorkflowError::UnexpectedBinding { id: *id });
        }
        if session.is_closed() {
            return Err(WorkflowError::ClosedBinding { id: *id });
        }
    }
    let runtime =
        tokio::runtime::Handle::try_current().map_err(|_| WorkflowError::RuntimeUnavailable)?;
    let (cancel, cancelled) = watch::channel(false);
    let (send, events) = mpsc::channel(task_count * 2);
    let (done, receipt) = oneshot::channel();
    let worker = runtime.spawn(async move {
        let result =
            scheduler::run(confirmed.into_ledger(), captured, options, cancelled, send).await;
        let _ = done.send(result);
    });
    Ok(WorkflowHandle {
        cancel,
        events,
        receipt: Some(receipt),
        worker,
    })
}
