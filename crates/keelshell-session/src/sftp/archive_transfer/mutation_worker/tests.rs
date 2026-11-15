#![cfg(test)]

use std::path::PathBuf;
use std::time::Duration;

use super::*;
use crate::sftp::{ArchiveTransferRequest, queue};
use crate::{SshAuth, SshOptions, SshSession};
use keelshell_core::ArchiveFormat;

#[path = "../../../../tests/support/zmodem_fixture.rs"]
mod fixture;

const WAIT: Duration = Duration::from_secs(5);

async fn abandoned_handoff(publish: bool) -> fixture::TestResult {
    let fixture = fixture::Fixture::start().await?;
    fixture.assert_no_authority_used();
    let sftp = fixture.session.sftp().await?;
    let temporary = tempfile::tempdir()?;
    let destination = temporary.path().join("published");
    let staging = temporary.path().join("staging");
    let request = ArchiveTransferRequest::DownloadExtract {
        remote: "/archive.zip".into(),
        local: destination.clone(),
        format: ArchiveFormat::Zip,
    };
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    let scope = sftp
        .reserve_archive_transfer(&request, Some(&staging), &allowed)
        .await?;
    let worker = MutationWorker {
        ticket: scope.ticket()?,
        stop: Arc::new(AtomicBool::new(false)),
    };
    let stop = worker.stop.clone();
    let worker_staging = staging.clone();
    let worker_destination = destination.clone();
    let (arrived, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let mut task = submit_at_boundary(
        scope.pending.clone(),
        worker,
        move |worker| {
            let guard = super::super::UnverifiedStage {
                ticket: Some(worker.ticket.clone()),
            };
            std::fs::create_dir(&worker_staging)?;
            std::fs::write(worker_staging.join("evidence.bin"), b"real pending output")?;
            let path = if publish {
                std::fs::rename(&worker_staging, &worker_destination)?;
                worker_destination
            } else {
                worker_staging
            };
            Ok((guard, path))
        },
        move || {
            let _ = arrived.send(());
            assert!(
                released.recv_timeout(WAIT).is_ok(),
                "bounded handoff barrier"
            );
        },
    );
    tokio::time::timeout(WAIT, ready).await??;
    authorized.store(false, Ordering::Release);
    stop.store(true, Ordering::Release);
    let authority = super::super::Authority {
        sftp: &sftp,
        authorized: &allowed,
    };
    let cancelled = authority
        .wait(observe_mutation(&scope.pending, &mut task))
        .await;
    assert!(matches!(cancelled, Err(SessionError::Closed)));
    let classified = scope.complete::<()>(Err(SessionError::Closed)).await;
    let is_uncertain = matches!(classified, Err(SessionError::MutationUncertain));
    let output: PathBuf = if publish {
        destination.clone()
    } else {
        staging.clone()
    };
    assert_eq!(
        std::fs::read(output.join("evidence.bin"))?,
        b"real pending output"
    );
    let mut claims = queue::ResourceClaims::default();
    for path in [&destination, &staging] {
        let mut claim = queue::local_claim(path, true).await?;
        claim.whole_side = false;
        claims.local.push(claim);
    }
    let other = Arc::new(queue::TransferReservations::new(&SshOptions::new(
        format!("handoff-{}.invalid", uuid::Uuid::new_v4()),
        "different-owner",
    )));
    assert!(matches!(
        queue::TransferReservations::admit_resources(&other, &claims),
        queue::Admission::Busy | queue::Admission::Quarantined
    ));
    release.send(())?;
    let result = tokio::time::timeout(WAIT, &mut task).await???;
    drop(result);
    assert!(matches!(
        queue::TransferReservations::admit_resources(&other, &claims),
        queue::Admission::Quarantined
    ));
    assert!(matches!(
        scope.complete::<()>(Ok(())).await,
        Err(SessionError::MutationUncertain)
    ));
    drop(scope);
    sftp.close().await?;
    fixture.finish().await?;
    assert!(
        is_uncertain,
        "unobserved result must preserve the UI unknown hold"
    );
    Ok(())
}

#[tokio::test]
async fn cancelled_stage_handoff_reports_uncertain_before_worker_result_is_returned()
-> fixture::TestResult {
    abandoned_handoff(false).await
}

#[tokio::test]
async fn cancelled_publication_handoff_reports_uncertain_before_worker_result_is_returned()
-> fixture::TestResult {
    abandoned_handoff(true).await
}

#[tokio::test]
async fn observed_mutation_result_acknowledges_a_stopped_worker() -> fixture::TestResult {
    let fixture = fixture::Fixture::start().await?;
    fixture.assert_no_authority_used();
    let sftp = fixture.session.sftp().await?;
    let temporary = tempfile::tempdir()?;
    let scope = sftp
        .reserve_directory_sync(
            &crate::sftp::TransferSpec::download("/", temporary.path()),
            &|| true,
        )
        .await?;
    let worker = MutationWorker {
        ticket: scope.ticket()?,
        stop: Arc::new(AtomicBool::new(false)),
    };
    let mut task = submit_mutation(scope.pending.clone(), worker, |_| Ok(17u32));
    let observed = tokio::time::timeout(WAIT, observe_mutation(&scope.pending, &mut task)).await?;
    assert_eq!(scope.complete(observed).await?, 17);
    assert!(!scope.pending.load(Ordering::Acquire));
    drop(scope);
    sftp.close().await?;
    fixture.finish().await
}
