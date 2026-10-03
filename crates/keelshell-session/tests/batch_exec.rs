//! Real TCP/SSH scheduler tests; fixture command text is never executed by a shell.
#[path = "fixtures/batch_server.rs"]
mod fixture;
use fixture::{Opening, Server, TestResult, bounded, until};
use keelshell_session::{
    BatchError, BatchEvent, BatchNotStartedReason as NotStarted, BatchOptions,
    BatchOutcome as Outcome, BatchPolicy, BatchTarget, BatchUnknownReason as Unknown, SshSession,
    start_batch,
};
use std::{sync::atomic::Ordering, time::Duration};
use uuid::Uuid;
fn target(session: &SshSession, command: impl Into<String>) -> BatchTarget {
    BatchTarget {
        id: Uuid::new_v4(),
        session: session.clone(),
        command: command.into(),
    }
}
fn options() -> BatchOptions {
    BatchOptions {
        timeout: Duration::from_secs(4),
        ..Default::default()
    }
}

#[tokio::test]
async fn entire_batch_is_validated_before_any_channel_opens() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        for limit in [0, 33] {
            let targets = (0..limit).map(|_| target(&ssh, "safe")).collect();
            assert!(matches!(
                start_batch(targets, options()),
                Err(BatchError::InvalidTargets)
            ));
        }
        for concurrency in [0, 9] {
            assert!(matches!(
                start_batch(
                    vec![target(&ssh, "safe")],
                    BatchOptions {
                        concurrency,
                        ..options()
                    }
                ),
                Err(BatchError::InvalidConcurrency)
            ));
        }
        for timeout in [Duration::ZERO, Duration::from_secs(301)] {
            assert!(matches!(
                start_batch(
                    vec![target(&ssh, "safe")],
                    BatchOptions {
                        timeout,
                        ..options()
                    }
                ),
                Err(BatchError::InvalidTimeout)
            ));
        }
        for output_limit in [0, 32 * 1024 * 1024 + 1] {
            assert!(matches!(
                start_batch(
                    vec![target(&ssh, "safe")],
                    BatchOptions {
                        output_limit,
                        ..options()
                    }
                ),
                Err(BatchError::InvalidOutputLimit)
            ));
        }
        assert!(matches!(
            start_batch(
                vec![target(&ssh, "safe"), target(&ssh, "safe")],
                BatchOptions {
                    output_limit: 32 * 1024 * 1024,
                    ..options()
                }
            ),
            Err(BatchError::InvalidOutputLimit)
        ));
        for command in [
            String::new(),
            "\n \t".into(),
            "nul\0value".into(),
            "x".repeat(65537),
        ] {
            assert!(matches!(
                start_batch(vec![target(&ssh, "safe"), target(&ssh, command)], options()),
                Err(BatchError::InvalidCommand)
            ));
        }
        let first = target(&ssh, "safe");
        let mut second = target(&ssh, "safe");
        second.id = first.id;
        assert!(matches!(
            start_batch(vec![first, second], options()),
            Err(BatchError::DuplicateTarget)
        ));
        tokio::task::yield_now().await;
        assert_eq!(server.observed.opens.load(Ordering::Acquire), 0);
        assert_eq!(server.observed.command_count()?, 0);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn concurrency_is_bounded_and_receipts_preserve_input_order_and_exact_commands() -> TestResult
{
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let first = server.connect().await?;
        let second = server.connect().await?;
        let commands: Vec<_> = (0..8)
            .map(|index| format!("hold-{index}\n  exact whitespace  \n"))
            .collect();
        let targets: Vec<_> = commands
            .iter()
            .enumerate()
            .map(|(i, c)| target(if i % 2 == 0 { &first } else { &second }, c))
            .collect();
        let ids: Vec<_> = targets.iter().map(|t| t.id).collect();
        let handle = start_batch(
            targets,
            BatchOptions {
                concurrency: 3,
                ..options()
            },
        )?;
        until(|| server.observed.command_count().is_ok_and(|n| n == 3)).await?;
        assert_eq!(server.observed.peak.load(Ordering::Acquire), 3);
        for (offset, command) in commands.iter().enumerate() {
            until(|| server.observed.command_count().is_ok_and(|n| n > offset)).await?;
            server.observed.release(command)?;
        }
        let receipt = handle.finish().await?;
        assert_eq!(receipt.rows.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
        for (row, command) in receipt.rows.iter().zip(commands) {
            assert_eq!(row.outcome, Outcome::Exited { code: 0 });
            assert_eq!(row.stdout, command.as_bytes());
            assert_eq!(row.stderr, b"fixture stderr");
        }
        assert!(server.observed.peak.load(Ordering::Acquire) <= 3);
        assert!(!receipt.cancelled);
        assert!(!receipt.stopped_after_failure);
        first.close().await?;
        second.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn stop_after_failure_skips_queued_rows_but_collects_inflight_success() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let mut targets = vec![target(&ssh, "hold-fail"), target(&ssh, "hold-success")];
        targets.extend((0..6).map(|_| target(&ssh, "never")));
        let mut handle = start_batch(
            targets,
            BatchOptions {
                concurrency: 2,
                policy: BatchPolicy::StopAfterFailure,
                ..options()
            },
        )?;
        until(|| server.observed.command_count().is_ok_and(|n| n == 2)).await?;
        server.observed.release("hold-fail")?;
        loop {
            match handle.recv().await {
                Some(BatchEvent::Finished { row })
                    if row.outcome == (Outcome::Exited { code: 7 }) =>
                {
                    break;
                }
                Some(_) => {}
                None => return Err("batch closed before failure receipt".into()),
            }
        }
        assert_eq!(server.observed.command_count()?, 2);
        server.observed.release("hold-success")?;
        let receipt = handle.finish().await?;
        assert_eq!(receipt.rows[0].outcome, Outcome::Exited { code: 7 });
        assert_eq!(receipt.rows[1].outcome, Outcome::Exited { code: 0 });
        assert!(receipt.rows[2..].iter().all(|r| r.outcome
            == Outcome::NotStarted {
                reason: NotStarted::StoppedAfterFailure
            }));
        assert_eq!(server.observed.command_count()?, 2);
        assert!(receipt.stopped_after_failure);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancelling_before_scheduler_poll_never_opens_or_executes() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let handle = start_batch((0..32).map(|_| target(&ssh, "never")).collect(), options())?;
        handle.cancel();
        let receipt = handle.finish().await?;
        assert_eq!(receipt.rows.len(), 32);
        assert!(receipt.cancelled);
        assert!(receipt.rows.iter().all(|r| r.outcome
            == Outcome::NotStarted {
                reason: NotStarted::Cancelled
            }));
        assert_eq!(server.observed.opens.load(Ordering::Acquire), 0);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancelling_after_exec_keeps_partial_output_and_does_not_start_queued_targets() -> TestResult
{
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let handle = start_batch(
            vec![target(&ssh, "flood-stall"), target(&ssh, "never")],
            BatchOptions {
                concurrency: 1,
                output_limit: 4 * 1024 * 1024,
                ..options()
            },
        )?;
        until(|| server.observed.windows.load(Ordering::Acquire) > 0).await?;
        handle.cancel();
        let receipt = handle.finish().await?;
        assert_eq!(
            receipt.rows[0].outcome,
            Outcome::Unknown {
                reason: Unknown::Cancelled
            }
        );
        assert!(!receipt.rows[0].stdout.is_empty());
        assert_eq!(receipt.rows[0].stderr, b"partial stderr");
        assert_eq!(
            receipt.rows[1].outcome,
            Outcome::NotStarted {
                reason: NotStarted::Cancelled
            }
        );
        assert_eq!(server.observed.command_count()?, 1);
        until(|| server.observed.closes.load(Ordering::Acquire) >= 1).await?;
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn unread_events_cannot_block_any_terminal_receipt_or_cancellation() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let mut handle = start_batch(
            (0..32).map(|_| target(&ssh, "ok")).collect(),
            BatchOptions {
                concurrency: 8,
                ..options()
            },
        )?;
        until(|| handle.is_finished()).await?;
        let mut starts = 0;
        let mut rows = Vec::new();
        while let Ok(event) = handle.try_recv() {
            match event {
                BatchEvent::Started { .. } => starts += 1,
                BatchEvent::Finished { row } => rows.push(row),
            }
        }
        assert_eq!(starts, 32);
        assert_eq!(rows.len(), 32);
        let receipt = handle.finish().await?;
        assert_eq!(receipt.rows.len(), 32);
        assert!(receipt.rows.iter().all(|r| r.outcome.is_success()));
        assert!(rows.iter().all(|row| {
            receipt
                .rows
                .iter()
                .any(|final_row| std::sync::Arc::ptr_eq(row, final_row))
        }));
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn rejection_exit_failure_missing_status_output_limit_and_disconnect_are_distinct()
-> TestResult {
    bounded(async {
        for (command, expected) in [
            ("reject", Outcome::Rejected),
            ("fail", Outcome::Exited { code: 7 }),
            (
                "close",
                Outcome::Unknown {
                    reason: Unknown::NoExitStatus,
                },
            ),
            (
                "limit",
                Outcome::Unknown {
                    reason: Unknown::OutputLimit,
                },
            ),
            (
                "drop",
                Outcome::Unknown {
                    reason: Unknown::ConnectionLost,
                },
            ),
        ] {
            let server = Server::start(Opening::Normal).await?;
            let ssh = server.connect().await?;
            let receipt = start_batch(
                vec![target(&ssh, command)],
                BatchOptions {
                    output_limit: if command == "limit" { 7 } else { 1024 },
                    ..options()
                },
            )?
            .finish()
            .await?;
            assert_eq!(receipt.rows[0].outcome, expected, "{command}");
            if command == "limit" {
                assert_eq!(receipt.rows[0].stdout, b"1234");
                assert_eq!(receipt.rows[0].stderr, b"abc");
            }
            if command != "drop" {
                assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
                ssh.close().await?;
            }
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn timeout_after_exec_is_unknown_with_bounded_partial_output() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let started = tokio::time::Instant::now();
        let receipt = start_batch(
            vec![target(&ssh, "flood-stall")],
            BatchOptions {
                timeout: Duration::from_secs(1),
                output_limit: 4 * 1024 * 1024,
                ..options()
            },
        )?
        .finish()
        .await?;
        assert_eq!(
            receipt.rows[0].outcome,
            Outcome::Unknown {
                reason: Unknown::Timeout
            }
        );
        assert!(!receipt.rows[0].stdout.is_empty());
        assert_eq!(receipt.rows[0].stderr, b"partial stderr");
        assert!(started.elapsed() < Duration::from_secs(4));
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn late_open_after_cancellation_is_closed_without_sending_exec() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Delay(Duration::from_millis(200))).await?;
        let ssh = server.connect().await?;
        let handle = start_batch(vec![target(&ssh, "never")], options())?;
        until(|| server.observed.opens.load(Ordering::Acquire) == 1).await?;
        handle.cancel();
        let receipt = handle.finish().await?;
        assert_eq!(
            receipt.rows[0].outcome,
            Outcome::NotStarted {
                reason: NotStarted::Cancelled
            }
        );
        until(|| server.observed.closes.load(Ordering::Acquire) == 1).await?;
        assert_eq!(server.observed.command_count()?, 0);
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn dropping_the_handle_aborts_owned_jobs_and_preserves_unrelated_channels() -> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let handle = start_batch(
            vec![target(&ssh, "hold-drop"), target(&ssh, "never")],
            BatchOptions {
                concurrency: 1,
                ..options()
            },
        )?;
        until(|| server.observed.command_count().is_ok_and(|n| n == 1)).await?;
        drop(handle);
        until(|| server.observed.closes.load(Ordering::Acquire) == 1).await?;
        until(|| server.observed.active.load(Ordering::Acquire) == 0).await?;
        assert_eq!(server.observed.command_count()?, 1);
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn refused_and_timed_out_opens_are_proven_not_started() -> TestResult {
    bounded(async {
        for (opening, reason) in [
            (Opening::Reject, NotStarted::ChannelRejected),
            (
                Opening::Delay(Duration::from_millis(1500)),
                NotStarted::Timeout,
            ),
        ] {
            let server = Server::start(opening).await?;
            let ssh = server.connect().await?;
            let receipt = start_batch(
                vec![target(&ssh, "never")],
                BatchOptions {
                    timeout: Duration::from_secs(1),
                    ..options()
                },
            )?
            .finish()
            .await?;
            assert_eq!(receipt.rows[0].outcome, Outcome::NotStarted { reason });
            assert_eq!(server.observed.command_count()?, 0);
            if reason == NotStarted::Timeout {
                until(|| ssh.is_closed()).await?;
            } else {
                ssh.close().await?;
            }
        }
        Ok(())
    })
    .await
}

#[test]
fn batch_scheduler_and_capture_work_on_a_two_mebibyte_thread_stack() -> TestResult {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| -> TestResult {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(bounded(async {
                let server = Server::start(Opening::Normal).await?;
                let ssh = server.connect().await?;
                let handle = start_batch(vec![target(&ssh, "ok")], options())?;
                let receipt = handle.finish().await?;
                assert!(receipt.rows[0].outcome.is_success());
                ssh.close().await?;
                Ok(())
            }))
        })?
        .join()
        .map_err(|_| "small-stack worker panicked")?
}

#[tokio::test]
async fn contradictory_rejection_or_missing_close_cannot_be_reported_as_success() -> TestResult {
    bounded(async {
        for (command, reason) in [
            ("status-then-reject", Unknown::Protocol),
            ("data-then-reject", Unknown::Protocol),
            ("status-then-drop", Unknown::ConnectionLost),
        ] {
            let server = Server::start(Opening::Normal).await?;
            let ssh = server.connect().await?;
            let receipt = start_batch(vec![target(&ssh, command)], options())?
                .finish()
                .await?;
            assert_eq!(receipt.rows[0].outcome, Outcome::Unknown { reason });
            if command == "data-then-reject" {
                assert_eq!(receipt.rows[0].stdout, b"observed");
            }
            if command != "status-then-drop" {
                ssh.close().await?;
            }
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn stop_policy_flag_distinguishes_actual_skips_from_cancellation_or_last_row_failure()
-> TestResult {
    bounded(async {
        let server = Server::start(Opening::Normal).await?;
        let ssh = server.connect().await?;
        let policy = BatchOptions {
            policy: BatchPolicy::StopAfterFailure,
            ..options()
        };
        let receipt = start_batch(vec![target(&ssh, "fail")], policy)?
            .finish()
            .await?;
        assert!(!receipt.stopped_after_failure);
        assert!(!receipt.cancelled);
        let handle = start_batch(
            vec![target(&ssh, "hold-cancel"), target(&ssh, "never")],
            BatchOptions {
                concurrency: 1,
                ..policy
            },
        )?;
        until(|| server.observed.command_count().is_ok_and(|n| n == 2)).await?;
        handle.cancel();
        let receipt = handle.finish().await?;
        assert!(receipt.cancelled);
        assert!(!receipt.stopped_after_failure);
        assert_eq!(
            receipt.rows[1].outcome,
            Outcome::NotStarted {
                reason: NotStarted::Cancelled
            }
        );
        ssh.close().await?;
        Ok(())
    })
    .await
}
