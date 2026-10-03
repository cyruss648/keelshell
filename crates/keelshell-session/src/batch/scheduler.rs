use super::{
    BatchEvent, BatchNotStartedReason, BatchOptions, BatchOutcome, BatchPolicy, BatchReceipt,
    BatchRowReceipt, BatchTarget, BatchUnknownReason, cancelled,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::{Id, JoinSet},
};

pub(super) async fn run(
    targets: Vec<BatchTarget>,
    options: BatchOptions,
    mut cancel: watch::Receiver<bool>,
    events: mpsc::Sender<BatchEvent>,
    done: oneshot::Sender<BatchReceipt>,
) {
    let ids: Vec<_> = targets.iter().map(|target| target.id).collect();
    let mut queued: VecDeque<_> = targets.into_iter().enumerate().collect();
    let mut rows: Vec<Option<Arc<BatchRowReceipt>>> = vec![None; ids.len()];
    let mut running = JoinSet::new();
    let mut task_rows: HashMap<Id, usize> = HashMap::new();
    let failed = Arc::new(AtomicBool::new(false));
    let mut was_cancelled = false;
    let mut stopped_after_failure = false;
    loop {
        // Drain ready results before admitting replacements. The failure flag
        // is also set by a row before channel cleanup, not only after its join.
        while let Some(result) = running.try_join_next_with_id() {
            record(result, &mut task_rows, &ids, &mut rows, &events, &failed);
        }
        was_cancelled |= *cancel.borrow();
        let stopped =
            options.policy == BatchPolicy::StopAfterFailure && failed.load(Ordering::Acquire);
        if was_cancelled || stopped {
            stopped_after_failure |= !was_cancelled && stopped && !queued.is_empty();
            let reason = if was_cancelled {
                BatchNotStartedReason::Cancelled
            } else {
                BatchNotStartedReason::StoppedAfterFailure
            };
            for (index, target) in queued.drain(..) {
                let row = Arc::new(BatchRowReceipt {
                    id: target.id,
                    outcome: BatchOutcome::NotStarted { reason },
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
                rows[index] = Some(row.clone());
                let _ = events.try_send(BatchEvent::Finished { row });
            }
        }
        while running.len() < options.concurrency && !queued.is_empty() {
            if *cancel.borrow()
                || (options.policy == BatchPolicy::StopAfterFailure
                    && failed.load(Ordering::Acquire))
            {
                break;
            }
            let Some((index, target)) = queued.pop_front() else {
                break;
            };
            let _ = events.try_send(BatchEvent::Started { id: target.id });
            let cancelled = cancel.clone();
            let failed = failed.clone();
            let task = running.spawn(async move {
                target
                    .session
                    .batch_exec(target.id, target.command, options, cancelled, failed)
                    .await
            });
            task_rows.insert(task.id(), index);
        }
        if running.is_empty() {
            if queued.is_empty() {
                break;
            }
            continue;
        }
        tokio::select! {
            biased;
            _=cancelled(&mut cancel), if !was_cancelled=>{was_cancelled=true;},
            Some(result)=running.join_next_with_id()=>record(result,&mut task_rows,&ids,&mut rows,&events,&failed),
        }
    }
    // JoinSet remains owned until every admitted task returned. Dropping this
    // scheduler instead aborts the set, transferring channel cleanup to guards.
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            row.unwrap_or_else(|| {
                Arc::new(BatchRowReceipt {
                    id: ids[index],
                    outcome: BatchOutcome::Unknown {
                        reason: BatchUnknownReason::WorkerFailed,
                    },
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            })
        })
        .collect();
    let _ = done.send(BatchReceipt {
        rows,
        cancelled: was_cancelled,
        stopped_after_failure,
    });
}
fn record(
    result: Result<(Id, BatchRowReceipt), tokio::task::JoinError>,
    tasks: &mut HashMap<Id, usize>,
    ids: &[uuid::Uuid],
    rows: &mut [Option<Arc<BatchRowReceipt>>],
    events: &mpsc::Sender<BatchEvent>,
    failed: &AtomicBool,
) {
    let (task, row) = match result {
        Ok((task, row)) => (task, Some(row)),
        Err(error) => (error.id(), None),
    };
    let Some(index) = tasks.remove(&task) else {
        return;
    };
    let row = row.unwrap_or_else(|| BatchRowReceipt {
        id: ids[index],
        outcome: BatchOutcome::Unknown {
            reason: BatchUnknownReason::WorkerFailed,
        },
        stdout: Vec::new(),
        stderr: Vec::new(),
    });
    if !row.outcome.is_success() {
        failed.store(true, Ordering::Release);
    }
    let row = Arc::new(row);
    rows[index] = Some(row.clone());
    let _ = events.try_send(BatchEvent::Finished { row });
}
