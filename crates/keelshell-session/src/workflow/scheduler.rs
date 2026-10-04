use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use keelshell_core::{BatchTaskOutcome, BatchTaskSkipReason, BatchTaskStatus, BatchWorkflowLedger};
use tokio::{
    sync::{mpsc, watch},
    task::{Id, JoinSet},
};
use uuid::Uuid;

use super::{
    WorkflowError, WorkflowEvent, WorkflowOptions, WorkflowReceipt, WorkflowTaskReceipt,
    WorkflowTaskResult,
};
use crate::{
    BatchOutcome, BatchPolicy, BatchRowReceipt, BatchUnknownReason, SshSession, batch::cancelled,
};

pub(super) async fn run(
    mut ledger: BatchWorkflowLedger,
    bindings: BTreeMap<Uuid, SshSession>,
    options: WorkflowOptions,
    mut cancel: watch::Receiver<bool>,
    events: mpsc::Sender<WorkflowEvent>,
) -> Result<WorkflowReceipt, WorkflowError> {
    let identities: Vec<_> = ledger
        .plan()
        .tasks()
        .iter()
        .map(|task| (task.id, task.target_id))
        .collect();
    let mut rows = vec![None; identities.len()];
    let mut running = JoinSet::new();
    let mut task_rows = HashMap::new();
    let failed = Arc::new(AtomicBool::new(false));
    let mut was_cancelled = false;
    let mut stopped_after_failure = false;
    loop {
        // A row sets failed before bounded channel cleanup. Drain complete
        // results before new admissions, and consult that early flag as well.
        while let Some(result) = running.try_join_next_with_id() {
            // Cancellation already observed by a finishing row must cancel
            // pending tasks before failure propagation can label their skips.
            was_cancelled |= *cancel.borrow();
            if was_cancelled {
                ledger.cancel_pending();
            }
            record(
                result,
                &mut task_rows,
                &identities,
                &mut ledger,
                &mut rows,
                &events,
                &failed,
            )?;
        }
        was_cancelled |= *cancel.borrow();
        let stopped =
            options.policy == BatchPolicy::StopAfterFailure && failed.load(Ordering::Acquire);
        if was_cancelled {
            ledger.cancel_pending();
        } else if stopped {
            for (id, _) in &identities {
                if matches!(
                    ledger.status(*id)?,
                    BatchTaskStatus::Ready | BatchTaskStatus::Blocked { .. }
                ) {
                    stopped_after_failure = true;
                    ledger.skip(*id, BatchTaskSkipReason::StoppedAfterFailure)?;
                }
            }
        }
        collect_skipped(&identities, &ledger, &mut rows, &events)?;
        if !was_cancelled && !stopped {
            let ready = ledger.ready_tasks();
            for id in ready {
                if running.len() >= options.concurrency
                    || *cancel.borrow()
                    || (options.policy == BatchPolicy::StopAfterFailure
                        && failed.load(Ordering::Acquire))
                {
                    break;
                }
                let index = identities
                    .iter()
                    .position(|(task, _)| *task == id)
                    .ok_or(WorkflowError::WorkerLost)?;
                let task = &ledger.plan().tasks()[index];
                let session = bindings
                    .get(&task.target_id)
                    .ok_or(WorkflowError::MissingBinding { id: task.target_id })?
                    .clone();
                let target_id = task.target_id;
                // Exact reviewed text is the sole source of the exec bytes.
                let command = task.command.clone();
                ledger.admit(id)?;
                let _ = events.try_send(WorkflowEvent::Started { id, target_id });
                let cancelled = cancel.clone();
                let failed = failed.clone();
                let task = running.spawn(async move {
                    session
                        .batch_exec(id, command, options.batch(), cancelled, failed)
                        .await
                });
                task_rows.insert(task.id(), index);
            }
        }
        if running.is_empty() {
            if ledger.is_finished() {
                break;
            }
            // A validated DAG always has a ready task unless cancellation or
            // early failure just stopped admissions; process that at the top.
            if *cancel.borrow()
                || (options.policy == BatchPolicy::StopAfterFailure
                    && failed.load(Ordering::Acquire))
            {
                continue;
            }
            return Err(WorkflowError::WorkerLost);
        }
        tokio::select! {
            biased;
            _ = cancelled(&mut cancel), if !was_cancelled => was_cancelled = true,
            Some(result) = running.join_next_with_id() => {
                was_cancelled |= *cancel.borrow();
                if was_cancelled {
                    ledger.cancel_pending();
                }
                record(result, &mut task_rows, &identities, &mut ledger,
                    &mut rows, &events, &failed)?;
            },
        }
    }
    // Every admitted worker returned. A cancelled OPEN can still have an
    // independently owned, deadline-bounded late-confirmation cleanup task.
    // Dropping this scheduler instead aborts its JoinSet and transfers cleanup
    // to the same existing channel owners, without a termination claim.
    let tasks = rows
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(WorkflowError::WorkerLost)?;
    Ok(WorkflowReceipt {
        tasks,
        cancelled: was_cancelled,
        stopped_after_failure,
        fingerprint: ledger.plan().review_token(),
        options,
    })
}

fn record(
    result: Result<(Id, BatchRowReceipt), tokio::task::JoinError>,
    workers: &mut HashMap<Id, usize>,
    identities: &[(Uuid, Uuid)],
    ledger: &mut BatchWorkflowLedger,
    rows: &mut [Option<Arc<WorkflowTaskReceipt>>],
    events: &mpsc::Sender<WorkflowEvent>,
    failed: &AtomicBool,
) -> Result<(), WorkflowError> {
    let (worker, row) = match result {
        Ok((worker, row)) => (worker, Some(row)),
        Err(error) => (error.id(), None),
    };
    let index = workers.remove(&worker).ok_or(WorkflowError::WorkerLost)?;
    let (id, target_id) = identities[index];
    let row = row.unwrap_or_else(|| BatchRowReceipt {
        id,
        outcome: BatchOutcome::Unknown {
            reason: BatchUnknownReason::WorkerFailed,
        },
        stdout: Vec::new(),
        stderr: Vec::new(),
    });
    if row.id != id || rows[index].is_some() {
        return Err(WorkflowError::WorkerLost);
    }
    let outcome = match row.outcome {
        BatchOutcome::Exited { code: 0 } => BatchTaskOutcome::Success,
        BatchOutcome::Unknown { .. } => BatchTaskOutcome::Unknown,
        _ => BatchTaskOutcome::Failed,
    };
    if outcome != BatchTaskOutcome::Success {
        failed.store(true, Ordering::Release);
    }
    ledger.finish(id, outcome)?;
    let task = Arc::new(WorkflowTaskReceipt {
        id,
        target_id,
        result: WorkflowTaskResult::Transport { row: Arc::new(row) },
    });
    rows[index] = Some(task.clone());
    let _ = events.try_send(WorkflowEvent::Finished { task });
    Ok(())
}

fn collect_skipped(
    identities: &[(Uuid, Uuid)],
    ledger: &BatchWorkflowLedger,
    rows: &mut [Option<Arc<WorkflowTaskReceipt>>],
    events: &mpsc::Sender<WorkflowEvent>,
) -> Result<(), WorkflowError> {
    for (index, (id, target_id)) in identities.iter().enumerate() {
        if rows[index].is_none()
            && let BatchTaskStatus::Skipped(reason) = ledger.status(*id)?
        {
            let task = Arc::new(WorkflowTaskReceipt {
                id: *id,
                target_id: *target_id,
                result: WorkflowTaskResult::Skipped { reason },
            });
            rows[index] = Some(task.clone());
            let _ = events.try_send(WorkflowEvent::Finished { task });
        }
    }
    Ok(())
}
