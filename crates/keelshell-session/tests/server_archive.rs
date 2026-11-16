//! Reviewed server archives through actual localhost SSH and SFTP.
//!
//! Normal fixture cases execute the generated Python helper. Fault responses
//! exercise conservative transport outcomes and do not establish OpenSSH/native
//! acceptance. OpenSSH coverage is owned by the parent's disposable runner.

#![cfg(unix)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use keelshell_core::ArchiveFormat;
use keelshell_session::{
    SessionError,
    sftp::{ServerArchivePlan, SftpSession, TransferSpec},
};

#[path = "support/server_archive_fixture.rs"]
mod fixture;
use fixture::{Fault, Fixture, TestResult, alive, entered};

async fn plan(
    fixture: &Fixture,
    sftp: &SftpSession,
    format: ArchiveFormat,
) -> TestResult<ServerArchivePlan> {
    let request = fixture.request(format)?;
    let plan = sftp
        .plan_server_archive(request, &AtomicBool::new(false))
        .await?;
    fixture.approve_pack(&plan)?;
    Ok(plan)
}

async fn assert_quarantine(sftp: &SftpSession, plan: &ServerArchivePlan) -> TestResult {
    let entries = sftp
        .inspect_remote_mutation_quarantine(plan.request().destination())
        .await?;
    assert!(
        entries
            .entries()
            .iter()
            .any(|entry| entry.destination == plan.request().destination())
    );
    let entries = sftp
        .inspect_remote_mutation_quarantine(plan.staging_path())
        .await?;
    assert!(
        entries
            .entries()
            .iter()
            .any(|entry| entry.destination == plan.staging_path())
    );
    Ok(())
}

async fn assert_no_quarantine(sftp: &SftpSession, plan: &ServerArchivePlan) -> TestResult {
    assert!(matches!(
        sftp.inspect_remote_mutation_quarantine(plan.request().destination())
            .await,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    assert!(matches!(
        sftp.inspect_remote_mutation_quarantine(plan.staging_path())
            .await,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    Ok(())
}

async fn success(format: ArchiveFormat) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, format).await?;
    assert_eq!(plan.manifest().members().len(), 4);
    assert!(
        sftp.inspect_entry(plan.request().destination())
            .await?
            .is_none()
    );
    assert!(sftp.inspect_entry(plan.staging_path()).await?.is_none());
    assert_eq!(
        plan.command(),
        plan.request().packing_command(plan.staging_path())?
    );
    assert_eq!(plan.input(), plan.manifest().packing_input());
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    let stage = plan.staging_path().to_owned();
    let receipt = sftp
        .execute_server_archive(plan.clone(), &AtomicBool::new(false))
        .await?;
    let bytes = sftp.read_regular(&receipt.remote, 64 * 1024 * 1024).await?;
    plan.manifest().verify_archive(&bytes)?;
    assert_eq!(receipt.member_count, 4);
    assert_eq!(receipt.archive_bytes, bytes.len() as u64);
    assert_eq!(receipt.expanded_bytes, plan.manifest().expanded_bytes());
    assert_eq!(receipt.remote, plan.request().destination());
    assert_eq!(
        *fixture
            .controls
            .stdin
            .lock()
            .map_err(|_| "poisoned fixture")?,
        plan.input()
    );
    assert!(fixture.controls.eof_count.load(Ordering::Acquire) >= 2);
    assert!(sftp.inspect_entry(&stage).await?.is_none());
    assert_no_quarantine(&sftp, &plan).await?;
    // A cloned review is not a second mutation authority.
    assert!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await
            .is_err()
    );
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 1);
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_zip_executes_the_real_helper_and_verifies_every_member() -> TestResult {
    success(ArchiveFormat::Zip).await
}

#[tokio::test]
async fn server_archive_tar_gz_executes_the_real_helper_and_verifies_every_member() -> TestResult {
    success(ArchiveFormat::TarGz).await
}

#[tokio::test]
async fn server_archive_changed_source_is_dispatched_failure_and_quarantines_both_writes()
-> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    tokio::fs::write(fixture.path("binary.bin")?, [1, 254, 129, 11]).await?;
    assert!(matches!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await,
        Err(SessionError::MutationUncertain)
    ));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 1);
    assert!(
        sftp.inspect_entry(plan.request().destination())
            .await?
            .is_none()
    );
    assert_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_existing_destination_prevents_even_inspection() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let request = fixture.request(ArchiveFormat::Zip)?;
    tokio::fs::write(request.destination(), b"existing owner").await?;
    assert!(
        sftp.plan_server_archive(request, &AtomicBool::new(false))
            .await
            .is_err()
    );
    assert_eq!(fixture.controls.inspect_commands.load(Ordering::Acquire), 0);
    assert_eq!(
        tokio::fs::read(fixture.path("packed.zip")?).await?,
        b"existing owner"
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

async fn existing_after_review(stage: bool) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    let target = if stage {
        plan.staging_path()
    } else {
        plan.request().destination()
    };
    tokio::fs::write(target, b"preserve external owner").await?;
    assert!(matches!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await,
        Err(SessionError::Invalid(_))
    ));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert_eq!(tokio::fs::read(target).await?, b"preserve external owner");
    assert_no_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_late_destination_is_preserved_without_unknown_hold() -> TestResult {
    existing_after_review(false).await
}

#[tokio::test]
async fn server_archive_preexisting_exact_stage_is_preserved_without_unknown_hold() -> TestResult {
    existing_after_review(true).await
}

#[tokio::test]
async fn server_archive_replacement_connection_to_same_endpoint_cannot_use_review() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let (replacement, other) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    assert!(!ssh.same_connection(&replacement));
    assert!(matches!(
        other
            .execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await,
        Err(SessionError::Invalid(_))
    ));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert_no_quarantine(&other, &plan).await?;
    // Original connection remains eligible after the rejected replacement.
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    other.close().await?;
    replacement.close().await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_before_planning_or_dispatch_creates_no_unknown_hold() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let request = fixture.request(ArchiveFormat::Zip)?;
    let opens = fixture.controls.opens.load(Ordering::Acquire);
    assert!(matches!(
        sftp.plan_server_archive(request, &AtomicBool::new(true))
            .await,
        Err(SessionError::Closed)
    ));
    assert_eq!(fixture.controls.opens.load(Ordering::Acquire), opens);
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    assert!(matches!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(true))
            .await,
        Err(SessionError::Closed)
    ));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert_no_quarantine(&sftp, &plan).await?;
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_during_late_exec_open_never_dispatches_and_can_review_again()
-> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    // Execution first opens its read-only SFTP observer, then the EXEC channel.
    fixture.controls.open_gate.arm();
    fixture.controls.open_gate_at.store(
        fixture.controls.opens.load(Ordering::Acquire) + 2,
        Ordering::Release,
    );
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let owner = sftp.clone();
    let current = plan.clone();
    let execution = tokio::spawn(async move { owner.execute_server_archive(current, &flag).await });
    entered(|| fixture.controls.open_gate.entered() != 0).await?;
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), execution).await??,
        Err(SessionError::Closed)
    ));
    fixture.controls.open_gate.release();
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert_no_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_during_inspection_open_cannot_execute_a_late_inspection()
-> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let request = fixture.request(ArchiveFormat::Zip)?;
    fixture.controls.open_gate.arm();
    fixture.controls.open_gate_at.store(
        fixture.controls.opens.load(Ordering::Acquire) + 2,
        Ordering::Release,
    );
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let owner = sftp.clone();
    let planning = tokio::spawn(async move { owner.plan_server_archive(request, &flag).await });
    entered(|| fixture.controls.open_gate.entered() != 0).await?;
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), planning).await??,
        Err(SessionError::Closed)
    ));
    fixture.controls.open_gate.release();
    alive(&ssh).await?;
    assert_eq!(fixture.controls.inspect_commands.load(Ordering::Acquire), 0);
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

async fn cancellation_after_dispatch(drop_future: bool, during_input: bool) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    if during_input {
        for index in 0..256 {
            tokio::fs::write(
                fixture.path(&format!("tree/part-{index:04}.txt"))?,
                b"content",
            )
            .await?;
        }
    }
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    if during_input {
        assert!(plan.input().len() > 64 * 1024);
        fixture.controls.input_gate.arm();
    } else {
        fixture.controls.dispatch_gate.arm();
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let owner = sftp.clone();
    let current = plan.clone();
    let execution = tokio::spawn(async move { owner.execute_server_archive(current, &flag).await });
    entered(|| {
        if during_input {
            fixture.controls.input_gate.entered() != 0
        } else {
            fixture.controls.dispatch_gate.entered() != 0
        }
    })
    .await?;
    if during_input {
        assert!(fixture.controls.input_bytes.load(Ordering::Acquire) < plan.input().len());
    }
    if drop_future {
        execution.abort();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), execution)
                .await?
                .is_err()
        );
    } else {
        cancelled.store(true, Ordering::Release);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), execution).await??,
            Err(SessionError::MutationUncertain)
        ));
    }
    fixture.controls.input_gate.release();
    fixture.controls.dispatch_gate.release();
    assert_quarantine(&sftp, &plan).await?;
    assert!(matches!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await,
        Err(SessionError::MutationQuarantined)
    ));
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_while_exec_ack_is_pending_retains_both_quarantines() -> TestResult {
    cancellation_after_dispatch(false, false).await
}

#[tokio::test]
async fn server_archive_cancel_during_stdin_retains_both_quarantines() -> TestResult {
    cancellation_after_dispatch(false, true).await
}

#[tokio::test]
async fn server_archive_dropped_dispatched_future_retains_quarantine_and_unrelated_ssh()
-> TestResult {
    cancellation_after_dispatch(true, false).await
}

async fn rejected_result(fault: Fault) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    fixture.pack_fault(fault)?;
    assert!(
        matches!(
            sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
                .await,
            Err(SessionError::MutationUncertain)
        ),
        "fault {fault:?} must remain unknown after dispatch"
    );
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 1);
    assert_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

macro_rules! rejected_results {
    ($($name:ident => $fault:ident),+ $(,)?) => {$(
        #[tokio::test]
        async fn $name() -> TestResult { rejected_result(Fault::$fault).await }
    )+};
}

rejected_results! {
    server_archive_exec_rejection_is_conservatively_unknown => Reject,
    server_archive_malformed_receipt_retains_quarantine => Malformed,
    server_archive_overbudget_stdout_retains_quarantine => Overflow,
    server_archive_combined_stdout_stderr_budget_retains_quarantine => CombinedOverflow,
    server_archive_missing_receipt_retains_quarantine => NoReceipt,
    server_archive_missing_exit_retains_quarantine => NoExit,
    server_archive_missing_eof_retains_quarantine => NoEof,
    server_archive_nonzero_exit_after_publication_retains_quarantine => NonzeroExit,
    server_archive_wrong_receipt_size_retains_quarantine => WrongSize,
    server_archive_wrong_receipt_digest_retains_quarantine => WrongDigest,
    server_archive_wrong_published_bytes_retain_quarantine => WrongBytes,
    server_archive_valid_receipt_for_different_members_retains_quarantine => DifferentArchive,
    server_archive_symlink_publication_retains_quarantine => SymlinkDestination,
    server_archive_retained_stage_after_success_retains_quarantine => RetainedStage,
    server_archive_failed_sftp_readback_retains_quarantine => FailRead,
    server_archive_failed_read_handle_close_retains_quarantine => FailReadClose,
    server_archive_partial_input_and_early_close_retain_quarantine => PartialInput,
}

#[tokio::test]
async fn server_archive_missing_exec_ack_uses_the_single_deadline_and_retains_quarantine()
-> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect_timeout(Duration::from_secs(1)).await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    fixture.pack_fault(Fault::NoAck)?;
    let started = std::time::Instant::now();
    assert!(matches!(
        tokio::time::timeout(
            Duration::from_secs(3),
            sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
        )
        .await?,
        Err(SessionError::MutationUncertain)
    ));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

async fn rejected_inspection(fault: Fault) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let request = fixture.request(ArchiveFormat::Zip)?;
    fixture.inspect_fault(fault)?;
    assert!(
        sftp.plan_server_archive(request.clone(), &AtomicBool::new(false))
            .await
            .is_err()
    );
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert!(matches!(
        sftp.inspect_remote_mutation_quarantine(request.destination())
            .await,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    fixture.inspect_fault(Fault::None)?;
    let plan = sftp
        .plan_server_archive(request, &AtomicBool::new(false))
        .await?;
    fixture.approve_pack(&plan)?;
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_malformed_inspection_never_creates_mutation_authority() -> TestResult {
    rejected_inspection(Fault::Malformed).await
}

#[tokio::test]
async fn server_archive_overbudget_inspection_never_creates_mutation_authority() -> TestResult {
    rejected_inspection(Fault::CombinedOverflow).await
}

async fn busy(stage: bool, source: bool) -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    let target = if source {
        plan.request().sources()[1].as_str()
    } else if stage {
        plan.staging_path()
    } else {
        plan.request().destination()
    };
    let spec = TransferSpec::upload(fixture.root()?.join("tree"), target);
    let scope = sftp.reserve_directory_sync(&spec, &|| true).await?;
    assert!(matches!(
        sftp.execute_server_archive(plan.clone(), &AtomicBool::new(false))
            .await,
        Err(SessionError::MutationBusy)
    ));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    assert_no_quarantine(&sftp, &plan).await?;
    drop(scope);
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_busy_source_tree_rejects_before_dispatch() -> TestResult {
    busy(false, true).await
}

#[tokio::test]
async fn server_archive_busy_final_tree_rejects_before_dispatch() -> TestResult {
    busy(false, false).await
}

#[tokio::test]
async fn server_archive_busy_staging_tree_rejects_before_dispatch() -> TestResult {
    busy(true, false).await
}

#[tokio::test]
async fn server_archive_readback_keeps_source_destination_and_stage_admission() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    fixture.controls.read_gate.arm();
    let owner = sftp.clone();
    let current = plan.clone();
    let execution = tokio::spawn(async move {
        owner
            .execute_server_archive(current, &AtomicBool::new(false))
            .await
    });
    entered(|| fixture.controls.read_gate.entered() != 0).await?;
    for target in [
        plan.request().sources()[1].as_str(),
        plan.request().destination(),
        plan.staging_path(),
    ] {
        let spec = TransferSpec::upload(fixture.root()?.join("tree"), target);
        assert!(matches!(
            sftp.reserve_directory_sync(&spec, &|| true).await,
            Err(SessionError::MutationBusy)
        ));
    }
    fixture.controls.read_gate.release();
    tokio::time::timeout(Duration::from_secs(5), execution).await???;
    assert_no_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_during_final_readback_does_not_clear_pending() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    fixture.controls.read_gate.arm();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let owner = sftp.clone();
    let current = plan.clone();
    let execution = tokio::spawn(async move { owner.execute_server_archive(current, &flag).await });
    entered(|| fixture.controls.read_gate.entered() != 0).await?;
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), execution).await??,
        Err(SessionError::MutationUncertain)
    ));
    fixture.controls.read_gate.release();
    assert_quarantine(&sftp, &plan).await?;
    alive(&ssh).await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_archive_cancel_during_admission_releases_without_unknown_write() -> TestResult {
    let fixture = Fixture::start().await?;
    let (ssh, sftp) = fixture.connect().await?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip).await?;
    fixture.controls.canonical_gate.arm();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let owner = sftp.clone();
    let current = plan.clone();
    let execution = tokio::spawn(async move { owner.execute_server_archive(current, &flag).await });
    entered(|| fixture.controls.canonical_gate.entered() != 0).await?;
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), execution).await??,
        Err(SessionError::Closed)
    ));
    fixture.controls.canonical_gate.release();
    assert_no_quarantine(&sftp, &plan).await?;
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 0);
    sftp.execute_server_archive(plan, &AtomicBool::new(false))
        .await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn server_archive_dispatch_and_readback_share_one_fixed_execution_deadline() -> TestResult {
    let fixture = Fixture::start()
        .await
        .map_err(|error| format!("fixed deadline fixture phase=start: {error}"))?;
    let (ssh, sftp) = fixture
        .connect_timeout(Duration::from_secs(1))
        .await
        .map_err(|error| format!("fixed deadline fixture phase=connect: {error}"))?;
    let plan = plan(&fixture, &sftp, ArchiveFormat::Zip)
        .await
        .map_err(|error| format!("fixed deadline fixture phase=plan: {error}"))?;
    fixture.controls.dispatch_gate.arm();
    fixture.controls.read_gate.arm();
    let owner = sftp.clone();
    let current = plan.clone();
    // Setup uses real time. Only the execution deadline is controlled here;
    // the real helper and transport must still reach both barriers.
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let execution = tokio::spawn(async move {
        owner
            .execute_server_archive(current, &AtomicBool::new(false))
            .await
    });
    fixture
        .wait_frozen(
            "dispatch barrier",
            Duration::from_secs(5),
            fixture.controls.dispatch_gate.wait_for_entry(),
        )
        .await?;
    assert_eq!(started.elapsed(), Duration::ZERO);
    tokio::time::advance(Duration::from_millis(650)).await;
    fixture.controls.dispatch_gate.release();
    fixture
        .wait_frozen(
            "readback barrier",
            Duration::from_secs(5),
            fixture.controls.read_gate.wait_for_entry(),
        )
        .await?;
    assert_eq!(started.elapsed(), Duration::from_millis(650));
    assert_eq!(fixture.controls.pack_commands.load(Ordering::Acquire), 1);
    assert!(!execution.is_finished());
    // Cross the original 1s deadline, allowing one millisecond for timer rounding.
    tokio::time::advance(Duration::from_millis(351)).await;
    assert!(matches!(
        fixture
            .wait_frozen(
                "execution deadline",
                Duration::from_secs(2),
                tokio::time::timeout(Duration::from_secs(2), execution),
            )
            .await???,
        Err(SessionError::MutationUncertain)
    ));
    assert!(started.elapsed() < Duration::from_millis(1400));
    tokio::time::resume();
    fixture.controls.read_gate.release();
    assert_quarantine(&sftp, &plan)
        .await
        .map_err(|error| format!("fixed deadline fixture phase=quarantine: {error}"))?;
    alive(&ssh)
        .await
        .map_err(|error| format!("fixed deadline fixture phase=alive: {error}"))?;
    sftp.close()
        .await
        .map_err(|error| format!("fixed deadline fixture phase=sftp-close: {error}"))?;
    ssh.close()
        .await
        .map_err(|error| format!("fixed deadline fixture phase=ssh-close: {error}"))?;
    Ok(())
}
