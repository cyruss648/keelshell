//! Real TCP/SSH reviewed-DAG tests; fixture commands select protocol scenarios.
#[path = "fixtures/batch_server.rs"]
mod fixture;

use std::{sync::atomic::Ordering, time::Duration};

use fixture::{Opening, Server, TestResult, bounded, until};
use keelshell_core::{
    BatchTaskSkipReason, BatchTaskSpec, BatchWorkflowPlan, ConfirmedBatchWorkflow,
};
use keelshell_session::{
    BatchNotStartedReason, BatchOutcome, BatchPolicy, BatchUnknownReason, SshSession,
    WorkflowBinding, WorkflowError, WorkflowEvent, WorkflowOptions, WorkflowTaskReceipt,
    WorkflowTaskResult, start_workflow,
};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn task(value: u128, target: u128, command: &str, dependencies: &[u128]) -> BatchTaskSpec {
    BatchTaskSpec {
        id: id(value),
        target_id: id(target),
        command: command.into(),
        dependencies: dependencies.iter().copied().map(id).collect(),
    }
}

fn confirmed(tasks: Vec<BatchTaskSpec>) -> TestResult<ConfirmedBatchWorkflow> {
    let plan = BatchWorkflowPlan::new(tasks)?;
    let token = plan.review_token();
    Ok(plan.confirm(token)?)
}

fn binding(value: u128, session: &SshSession) -> WorkflowBinding {
    WorkflowBinding {
        id: id(value),
        session: session.clone(),
    }
}

fn options() -> WorkflowOptions {
    WorkflowOptions {
        timeout: Duration::from_secs(4),
        ..Default::default()
    }
}

fn outcome(task: &WorkflowTaskReceipt) -> TestResult<BatchOutcome> {
    match &task.result {
        WorkflowTaskResult::Transport { row } => Ok(row.outcome),
        WorkflowTaskResult::Skipped { .. } => Err("expected transport receipt".into()),
    }
}

#[tokio::test]
async fn all_bindings_and_graph_output_limits_are_checked_before_any_channel() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let one = || confirmed(vec![task(1, 101, "safe", &[])]);
        for concurrency in [0, 9] {
            assert!(matches!(
                start_workflow(
                    one()?,
                    vec![binding(101, &ssh)],
                    WorkflowOptions {
                        concurrency,
                        ..options()
                    }
                ),
                Err(WorkflowError::InvalidConcurrency)
            ));
        }
        for timeout in [Duration::ZERO, Duration::from_secs(301)] {
            assert!(matches!(
                start_workflow(
                    one()?,
                    vec![binding(101, &ssh)],
                    WorkflowOptions {
                        timeout,
                        ..options()
                    }
                ),
                Err(WorkflowError::InvalidTimeout)
            ));
        }
        for output_limit in [0, 32 * 1024 * 1024 + 1] {
            assert!(matches!(
                start_workflow(
                    one()?,
                    vec![binding(101, &ssh)],
                    WorkflowOptions {
                        output_limit,
                        ..options()
                    }
                ),
                Err(WorkflowError::InvalidOutputLimit)
            ));
        }
        let maximum = confirmed(
            (1..=128)
                .map(|value| task(value, 101, "safe", &[]))
                .collect(),
        )?;
        assert!(matches!(
            start_workflow(
                maximum,
                vec![binding(101, &ssh)],
                WorkflowOptions {
                    output_limit: 256 * 1024 + 1,
                    ..options()
                }
            ),
            Err(WorkflowError::InvalidOutputLimit)
        ));
        for bindings in [
            Vec::new(),
            (101..=133).map(|value| binding(value, &ssh)).collect(),
        ] {
            assert!(matches!(
                start_workflow(one()?, bindings, options()),
                Err(WorkflowError::InvalidBindings)
            ));
        }
        assert!(matches!(
            start_workflow(one()?, vec![binding(0, &ssh)], options()),
            Err(WorkflowError::InvalidBindings)
        ));
        assert!(matches!(
            start_workflow(one()?, vec![binding(101, &ssh), binding(101, &ssh)], options()),
            Err(WorkflowError::DuplicateBinding { id: repeated }) if repeated == id(101)
        ));
        assert!(matches!(
            start_workflow(one()?, vec![binding(102, &ssh)], options()),
            Err(WorkflowError::MissingBinding { id: missing }) if missing == id(101)
        ));
        assert!(matches!(
            start_workflow(one()?, vec![binding(101, &ssh), binding(102, &ssh)], options()),
            Err(WorkflowError::UnexpectedBinding { id: extra }) if extra == id(102)
        ));
        ssh.close().await?;
        until(|| ssh.is_closed()).await?;
        assert!(matches!(
            start_workflow(one()?, vec![binding(101, &ssh)], options()),
            Err(WorkflowError::ClosedBinding { id: closed }) if closed == id(101)
        ));
        assert_eq!(server.observed.opens.load(Ordering::Acquire), 0);
        assert_eq!(server.observed.command_count()?, 0);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn exact_reviewed_commands_wait_for_every_prerequisite_on_captured_targets() -> TestResult {
    bounded(async {
        let first = Server::start(Opening::Normal).await?;
        let second = Server::start(Opening::Normal).await?;
        let first_ssh = first.connect().await?;
        let second_ssh = second.connect().await?;
        let command = "hold-first\n  exact whitespace  \n";
        let plan = confirmed(vec![
            task(3, 101, "joined\n  exact final  \n", &[2, 1]),
            task(2, 102, "hold-second", &[]),
            task(1, 101, command, &[]),
        ])?;
        let fingerprint = plan.plan().review_token();
        let handle = start_workflow(
            plan,
            vec![binding(101, &first_ssh), binding(102, &second_ssh)],
            WorkflowOptions {
                concurrency: 2,
                ..options()
            },
        )?;
        until(|| {
            first.observed.command_count().is_ok_and(|count| count == 1)
                && second
                    .observed
                    .command_count()
                    .is_ok_and(|count| count == 1)
        })
        .await?;
        first.observed.release(command)?;
        until(|| first.observed.active.load(Ordering::Acquire) == 0).await?;
        assert_eq!(first.observed.command_count()?, 1);
        second.observed.release("hold-second")?;
        let receipt = handle.finish().await?;
        assert_eq!(receipt.fingerprint, fingerprint);
        assert_eq!(receipt.options.concurrency, 2);
        assert_eq!(
            receipt.tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![id(1), id(2), id(3)]
        );
        assert!(
            receipt
                .tasks
                .iter()
                .all(|task| outcome(task).is_ok_and(BatchOutcome::is_success))
        );
        assert_eq!(
            *first.observed.commands.lock().map_err(|_| "fixture lock")?,
            vec![
                command.as_bytes().to_vec(),
                b"joined\n  exact final  \n".to_vec()
            ]
        );
        assert_eq!(receipt.tasks[1].target_id, id(102));
        first_ssh.close().await?;
        second_ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn failures_rejections_and_unknown_results_skip_descendants_but_continue_independent_work()
-> TestResult {
    bounded(async {
        for (command, expected) in [
            ("fail", BatchOutcome::Exited { code: 7 }),
            ("reject", BatchOutcome::Rejected),
            (
                "close",
                BatchOutcome::Unknown {
                    reason: BatchUnknownReason::NoExitStatus,
                },
            ),
        ] {
            let server = Server::start(Opening::Normal).await?;
            let ssh = server.connect().await?;
            let plan = confirmed(vec![
                task(1, 101, command, &[]),
                task(2, 101, "never", &[1]),
                task(3, 101, "independent", &[]),
                task(4, 101, "never-transitive", &[2]),
            ])?;
            let receipt = start_workflow(
                plan,
                vec![binding(101, &ssh)],
                WorkflowOptions {
                    concurrency: 1,
                    ..options()
                },
            )?
            .finish()
            .await?;
            assert_eq!(outcome(&receipt.tasks[0])?, expected);
            assert_eq!(
                receipt.tasks[1].result,
                WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::DependencyNotSucceeded { dependency: id(1) }
                }
            );
            assert_eq!(
                outcome(&receipt.tasks[2])?,
                BatchOutcome::Exited { code: 0 }
            );
            assert_eq!(
                receipt.tasks[3].result,
                WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::DependencyNotSucceeded { dependency: id(2) }
                }
            );
            assert_eq!(server.observed.command_count()?, 2);
            assert!(!receipt.stopped_after_failure);
            ssh.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn stop_policy_skips_unrelated_pending_tasks_and_collects_inflight_results() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let plan = confirmed(vec![
            task(1, 101, "hold-fail", &[]),
            task(2, 101, "hold-success", &[]),
            task(3, 101, "never", &[]),
            task(4, 101, "never-dependent", &[1]),
        ])?;
        let mut handle = start_workflow(
            plan,
            vec![binding(101, &ssh)],
            WorkflowOptions {
                concurrency: 2,
                policy: BatchPolicy::StopAfterFailure,
                ..options()
            },
        )?;
        until(|| {
            server
                .observed
                .command_count()
                .is_ok_and(|count| count == 2)
        })
        .await?;
        server.observed.release("hold-fail")?;
        loop {
            match handle.recv().await {
                Some(WorkflowEvent::Finished { task }) if task.id == id(3) => break,
                Some(_) => {}
                None => return Err("missing policy skip event".into()),
            }
        }
        assert_eq!(server.observed.command_count()?, 2);
        server.observed.release("hold-success")?;
        let receipt = handle.finish().await?;
        assert_eq!(
            outcome(&receipt.tasks[0])?,
            BatchOutcome::Exited { code: 7 }
        );
        assert_eq!(
            outcome(&receipt.tasks[1])?,
            BatchOutcome::Exited { code: 0 }
        );
        assert_eq!(
            receipt.tasks[2].result,
            WorkflowTaskResult::Skipped {
                reason: BatchTaskSkipReason::StoppedAfterFailure
            }
        );
        assert_eq!(
            receipt.tasks[3].result,
            WorkflowTaskResult::Skipped {
                reason: BatchTaskSkipReason::DependencyNotSucceeded { dependency: id(1) }
            }
        );
        assert!(receipt.stopped_after_failure);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancellation_before_first_poll_skips_every_task_without_a_channel() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let plan = confirmed(
            (1..=128)
                .map(|value| task(value, 101, "never", &[]))
                .collect(),
        )?;
        let handle = start_workflow(plan, vec![binding(101, &ssh)], options())?;
        handle.cancel();
        let receipt = handle.finish().await?;
        assert_eq!(receipt.tasks.len(), 128);
        assert!(receipt.cancelled);
        assert!(receipt.tasks.iter().all(|task| task.result
            == WorkflowTaskResult::Skipped {
                reason: BatchTaskSkipReason::Cancelled
            }));
        assert_eq!(server.observed.opens.load(Ordering::Acquire), 0);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancelling_active_work_keeps_partial_output_and_preserves_the_connection() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let plan = confirmed(vec![
            task(1, 101, "flood-stall", &[]),
            task(2, 101, "never", &[1]),
            task(3, 101, "never-independent", &[]),
        ])?;
        let handle = start_workflow(
            plan,
            vec![binding(101, &ssh)],
            WorkflowOptions {
                concurrency: 1,
                output_limit: 4 * 1024 * 1024,
                ..options()
            },
        )?;
        until(|| server.observed.windows.load(Ordering::Acquire) > 0).await?;
        let started = tokio::time::Instant::now();
        handle.cancel();
        let receipt = handle.finish().await?;
        assert!(started.elapsed() < Duration::from_secs(4));
        assert_eq!(
            outcome(&receipt.tasks[0])?,
            BatchOutcome::Unknown {
                reason: BatchUnknownReason::Cancelled
            }
        );
        let WorkflowTaskResult::Transport { row } = &receipt.tasks[0].result else {
            return Err("missing active row".into());
        };
        assert!(!row.stdout.is_empty());
        assert_eq!(row.stderr, b"partial stderr");
        assert!(receipt.tasks[1..].iter().all(|task| task.result
            == WorkflowTaskResult::Skipped {
                reason: BatchTaskSkipReason::Cancelled
            }));
        assert_eq!(server.observed.command_count()?, 1);
        until(|| server.observed.active.load(Ordering::Acquire) == 0).await?;
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn dropping_the_handle_aborts_owned_channels_and_never_admits_descendants() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let plan = confirmed(vec![
            task(1, 101, "hold-drop", &[]),
            task(2, 101, "never", &[1]),
        ])?;
        let handle = start_workflow(plan, vec![binding(101, &ssh)], options())?;
        until(|| {
            server
                .observed
                .command_count()
                .is_ok_and(|count| count == 1)
        })
        .await?;
        drop(handle);
        until(|| {
            server.observed.closes.load(Ordering::Acquire) == 1
                && server.observed.active.load(Ordering::Acquire) == 0
        })
        .await?;
        assert_eq!(server.observed.command_count()?, 1);
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn replacing_the_callers_binding_does_not_redirect_queued_commands() -> TestResult {
    bounded(async {
        let original = Server::start(Opening::Normal).await?;
        let replacement = Server::start(Opening::Normal).await?;
        let original_ssh = original.connect().await?;
        let mut selected = original_ssh.clone();
        let plan = confirmed(vec![
            task(1, 101, "hold-original", &[]),
            task(2, 101, "after-original", &[1]),
        ])?;
        let handle = start_workflow(plan, vec![binding(101, &selected)], options())?;
        until(|| {
            original
                .observed
                .command_count()
                .is_ok_and(|count| count == 1)
        })
        .await?;
        selected = replacement.connect().await?;
        original.observed.release("hold-original")?;
        let receipt = handle.finish().await?;
        assert!(
            receipt
                .tasks
                .iter()
                .all(|task| outcome(task).is_ok_and(BatchOutcome::is_success))
        );
        assert_eq!(original.observed.command_count()?, 2);
        assert_eq!(replacement.observed.opens.load(Ordering::Acquire), 0);
        original_ssh.close().await?;
        selected.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn timeout_and_output_limit_remain_unknown_and_block_downstream() -> TestResult {
    bounded(async {
        for (command, expected, output_limit) in [
            ("flood-stall", BatchUnknownReason::Timeout, 4 * 1024 * 1024),
            ("limit", BatchUnknownReason::OutputLimit, 7),
        ] {
            let server = Server::start(Opening::Normal).await?;
            let ssh = server.connect().await?;
            let plan = confirmed(vec![
                task(1, 101, command, &[]),
                task(2, 101, "never", &[1]),
            ])?;
            let receipt = start_workflow(
                plan,
                vec![binding(101, &ssh)],
                WorkflowOptions {
                    timeout: Duration::from_secs(1),
                    output_limit,
                    ..options()
                },
            )?
            .finish()
            .await?;
            assert_eq!(
                outcome(&receipt.tasks[0])?,
                BatchOutcome::Unknown { reason: expected }
            );
            assert_eq!(
                receipt.tasks[1].result,
                WorkflowTaskResult::Skipped {
                    reason: BatchTaskSkipReason::DependencyNotSucceeded { dependency: id(1) }
                }
            );
            assert_eq!(server.observed.command_count()?, 1);
            ssh.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn rejected_and_late_cancelled_opens_are_not_started_and_never_release_dependencies()
-> TestResult {
    bounded(async {
        for opening in [Opening::Reject, Opening::Delay(Duration::from_millis(200))] {
            let server = Server::start(opening).await?;
            let ssh = server.connect().await?;
            let plan = confirmed(vec![
                task(1, 101, "never", &[]),
                task(2, 101, "never-after", &[1]),
            ])?;
            let handle = start_workflow(plan, vec![binding(101, &ssh)], options())?;
            if matches!(opening, Opening::Delay(_)) {
                until(|| server.observed.opens.load(Ordering::Acquire) == 1).await?;
                handle.cancel();
            }
            let receipt = handle.finish().await?;
            assert_eq!(
                outcome(&receipt.tasks[0])?,
                BatchOutcome::NotStarted {
                    reason: if matches!(opening, Opening::Reject) {
                        BatchNotStartedReason::ChannelRejected
                    } else {
                        BatchNotStartedReason::Cancelled
                    }
                }
            );
            assert!(matches!(
                receipt.tasks[1].result,
                WorkflowTaskResult::Skipped { .. }
            ));
            assert_eq!(server.observed.command_count()?, 0);
            assert_eq!(server.observed.opens.load(Ordering::Acquire), 1);
            if matches!(opening, Opening::Delay(_)) {
                until(|| server.observed.closes.load(Ordering::Acquire) == 1).await?;
            }
            ssh.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn maximum_graph_has_bounded_concurrency_and_unread_events_share_terminal_receipts()
-> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let tasks = (1..=128).map(|value| task(value, 101, "ok", &[])).collect();
        let mut handle = start_workflow(
            confirmed(tasks)?,
            vec![binding(101, &ssh)],
            WorkflowOptions {
                concurrency: 8,
                ..options()
            },
        )?;
        until(|| handle.is_finished()).await?;
        let mut starts = 0;
        let mut events = Vec::new();
        while let Ok(event) = handle.try_recv() {
            match event {
                WorkflowEvent::Started {
                    id: task,
                    target_id,
                } => {
                    assert!(!task.is_nil());
                    assert_eq!(target_id, id(101));
                    starts += 1;
                }
                WorkflowEvent::Finished { task } => events.push(task),
            }
        }
        let receipt = handle.finish().await?;
        assert_eq!(starts, 128);
        assert_eq!(events.len(), 128);
        assert_eq!(receipt.tasks.len(), 128);
        assert!(
            receipt
                .tasks
                .iter()
                .all(|task| outcome(task).is_ok_and(BatchOutcome::is_success))
        );
        assert!(events.iter().all(|event| {
            receipt
                .tasks
                .iter()
                .any(|task| std::sync::Arc::ptr_eq(event, task))
        }));
        assert!(server.observed.peak.load(Ordering::Acquire) <= 8);
        assert_eq!(server.observed.command_count()?, 128);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn nonblocking_aggregate_collection_keeps_queued_events_and_compares_actual_connections()
-> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let replacement = server.connect().await?;
        assert!(ssh.same_connection(&ssh.clone()));
        assert!(!ssh.same_connection(&replacement));
        let mut handle = start_workflow(
            confirmed(vec![task(1, 101, "hold-aggregate", &[])])?,
            vec![binding(101, &ssh)],
            options(),
        )?;
        assert!(handle.try_finish().is_none());
        until(|| {
            server
                .observed
                .command_count()
                .is_ok_and(|count| count == 1)
        })
        .await?;
        server.observed.release("hold-aggregate")?;
        let receipt = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(result) = handle.try_finish() {
                    break result;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await??;
        assert_eq!(
            outcome(&receipt.tasks[0])?,
            BatchOutcome::Exited { code: 0 }
        );
        let mut started = 0;
        let mut finished = 0;
        while let Ok(event) = handle.try_recv() {
            match event {
                WorkflowEvent::Started { .. } => started += 1,
                WorkflowEvent::Finished { task } => {
                    finished += 1;
                    assert!(std::sync::Arc::ptr_eq(&task, &receipt.tasks[0]));
                }
            }
        }
        assert_eq!((started, finished), (1, 1));
        assert!(matches!(
            handle.try_finish(),
            Some(Err(WorkflowError::WorkerLost))
        ));
        assert!(!ssh.is_closed() && !replacement.is_closed());
        ssh.close().await?;
        replacement.close().await?;
        Ok(())
    })
    .await
}
