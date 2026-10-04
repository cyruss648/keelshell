//! Opt-in interoperability checks against an independently managed OpenSSH
//! server. Tests use only a loopback endpoint, pinned identity, ephemeral key,
//! and a caller-supplied disposable remote directory. No account is created.

use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};

use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    sftp::{SftpSession, TransferEvent, TransferHandle, TransferSpec},
};
use russh::keys::{HashAlg, PublicKey};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

struct Fixture {
    ssh: SshSession,
    sftp: Arc<SftpSession>,
    directory: String,
}

impl Fixture {
    fn options() -> TestResult<SshOptions> {
        let username = std::env::var("KEELSHELL_OPENSSH_USER")?;
        let mut options = SshOptions::new("127.0.0.1", username);
        options.port = std::env::var("KEELSHELL_OPENSSH_PORT")?.parse()?;
        let public = std::fs::read_to_string(std::env::var("KEELSHELL_OPENSSH_HOST_KEY")?)?;
        options.expected_host_key = Some(
            PublicKey::from_openssh(&public)?
                .fingerprint(HashAlg::Sha256)
                .to_string(),
        );
        options.auth = SshAuth::PrivateKey {
            path: PathBuf::from(std::env::var("KEELSHELL_OPENSSH_CLIENT_KEY")?),
            passphrase: None,
        };
        options.timeout = Duration::from_secs(30);
        Ok(options)
    }

    async fn connect() -> TestResult<Self> {
        let base = std::env::var("KEELSHELL_OPENSSH_ROOT")?;
        if !base.starts_with('/') || base == "/" || base.split('/').any(|part| part == "..") {
            return Err("configure an absolute disposable remote directory".into());
        }
        let directory = format!(
            "{}/keelshell-interop-{}",
            base.trim_end_matches('/'),
            uuid::Uuid::new_v4()
        );
        let ssh = SshSession::connect(Self::options()?)
            .await
            .map_err(|error| format!("OpenSSH authentication: {error}"))?;
        let sftp = Arc::new(
            ssh.sftp()
                .await
                .map_err(|error| format!("OpenSSH subsystem: {error}"))?,
        );
        sftp.mkdir(&directory)
            .await
            .map_err(|error| format!("OpenSSH isolated directory: {error}"))?;
        Ok(Self {
            ssh,
            sftp,
            directory,
        })
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.directory)
    }

    async fn reconnect(&mut self) -> TestResult {
        self.sftp.close().await?;
        self.ssh.close().await?;
        self.ssh = SshSession::connect(Self::options()?).await?;
        self.sftp = Arc::new(self.ssh.sftp().await?);
        Ok(())
    }

    async fn close(self, files: &[&str], directories: &[&str]) -> TestResult {
        for name in files {
            self.sftp.remove(&self.path(name)).await?;
        }
        for name in directories {
            self.sftp.rmdir(&self.path(name)).await?;
        }
        self.sftp.rmdir(&self.directory).await?;
        self.sftp.close().await?;
        self.ssh.close().await?;
        Ok(())
    }
}

async fn completed(handle: &mut TransferHandle) -> TestResult<u64> {
    loop {
        match tokio::time::timeout(Duration::from_secs(90), handle.recv()).await? {
            Some(TransferEvent::Completed { bytes, .. }) => return Ok(bytes),
            Some(TransferEvent::Failed { error, .. }) => return Err(error.into()),
            Some(TransferEvent::Cancelled { .. }) => return Err("unexpected cancellation".into()),
            Some(_) => {}
            None => return Err("transfer ended without a terminal event".into()),
        }
    }
}

async fn failed(handle: &mut TransferHandle) -> TestResult {
    loop {
        match tokio::time::timeout(Duration::from_secs(30), handle.recv()).await? {
            Some(TransferEvent::Failed { .. }) => return Ok(()),
            Some(TransferEvent::Completed { .. } | TransferEvent::Cancelled { .. }) => {
                return Err("conflicting destination did not fail".into());
            }
            Some(_) => {}
            None => return Err("transfer ended without failure".into()),
        }
    }
}

/// Request a pause only after new bytes (not just a verified resume prefix)
/// have been acknowledged, and fail if the transfer wins the race to completion.
async fn pause_after_progress(handle: &mut TransferHandle, existing: u64) -> TestResult<u64> {
    let mut requested = false;
    loop {
        match tokio::time::timeout(Duration::from_secs(30), handle.recv()).await? {
            Some(TransferEvent::Progress { transferred, .. })
                if !requested && transferred > existing =>
            {
                handle.pause();
                requested = true;
            }
            Some(TransferEvent::Paused { transferred, .. }) => return Ok(transferred),
            Some(
                TransferEvent::Completed { .. }
                | TransferEvent::Failed { .. }
                | TransferEvent::Cancelled { .. },
            )
            | None => {
                return Err("transfer finished before acknowledged pause".into());
            }
            _ => {}
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_checked_content_reads_inspect_regular_files_and_refuse_truncation() -> TestResult {
    tokio::time::timeout(Duration::from_secs(60), async {
        let fixture = Fixture::connect().await?;
        fixture
            .sftp
            .write(&fixture.path("checked.bin"), b"checked-content")
            .await?;
        fixture.sftp.write(&fixture.path("empty.bin"), b"").await?;
        let inspected = fixture
            .sftp
            .inspect_entry(&fixture.path("checked.bin"))
            .await?
            .ok_or("checked file is missing")?;
        assert!(!inspected.is_directory && !inspected.is_symlink);
        assert_eq!(inspected.size, Some(15));
        assert_eq!(
            fixture
                .sftp
                .read_regular(&fixture.path("checked.bin"), 15)
                .await?,
            b"checked-content"
        );
        assert!(
            fixture
                .sftp
                .read_regular(&fixture.path("checked.bin"), 14)
                .await
                .is_err()
        );
        assert_eq!(
            fixture
                .sftp
                .read_regular(&fixture.path("empty.bin"), 0)
                .await?,
            b""
        );
        assert!(
            fixture
                .sftp
                .read_regular(&fixture.directory, 100)
                .await
                .is_err()
        );
        assert!(
            fixture
                .sftp
                .inspect_entry(&fixture.path("missing.bin"))
                .await?
                .is_none()
        );
        fixture.close(&["checked.bin", "empty.bin"], &[]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_file_resume_revalidates_content_and_works_on_a_new_connection() -> TestResult {
    tokio::time::timeout(Duration::from_secs(120), async {
        let mut fixture = Fixture::connect().await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source.bin");
        let bytes: Vec<u8> = (0..512 * 1024).map(|index| (index % 251) as u8).collect();
        tokio::fs::write(&source, &bytes).await?;
        fixture
            .sftp
            .write(&fixture.path("upload.bin"), &bytes[..137_003])
            .await?;

        // A fresh connection must inspect the partial file itself; no retained
        // in-memory offset or prior transfer handle is available to it.
        fixture.reconnect().await?;
        let fresh = fixture.sftp.clone();
        let plan = fresh
            .plan_file_resume(TransferSpec::upload(&source, fixture.path("upload.bin")))
            .await?;
        assert_eq!(plan.existing_bytes(), 137_003);
        let queue = fresh.clone().transfer_queue();
        let mut upload = queue.enqueue_resume(plan).await?;
        assert_eq!(completed(&mut upload).await?, bytes.len() as u64);
        assert_eq!(
            fresh.read(&fixture.path("upload.bin"), bytes.len()).await?,
            bytes
        );

        let destination = local.path().join("download.bin");
        tokio::fs::write(&destination, &bytes[..99_001]).await?;
        let plan = fresh
            .plan_file_resume(TransferSpec::download(
                fixture.path("upload.bin"),
                &destination,
            ))
            .await?;
        assert_eq!(plan.existing_bytes(), 99_001);
        let mut download = queue.enqueue_resume(plan).await?;
        assert_eq!(completed(&mut download).await?, bytes.len() as u64);
        assert_eq!(tokio::fs::read(&destination).await?, bytes);

        fixture
            .sftp
            .write(&fixture.path("conflict.bin"), &bytes[..100])
            .await?;
        let stale = fresh
            .plan_file_resume(TransferSpec::upload(&source, fixture.path("conflict.bin")))
            .await?;
        let replacement = b"changed after review";
        fixture
            .sftp
            .write(&fixture.path("conflict.bin"), replacement)
            .await?;
        let mut conflict = queue.enqueue_resume(stale).await?;
        failed(&mut conflict).await?;
        assert_eq!(
            fresh.read(&fixture.path("conflict.bin"), 100).await?,
            replacement
        );
        drop(queue);
        fixture.close(&["upload.bin", "conflict.bin"], &[]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_directory_resume_fills_partial_trees_in_both_directions() -> TestResult {
    tokio::time::timeout(Duration::from_secs(120), async {
        let fixture = Fixture::connect().await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        tokio::fs::create_dir_all(source.join("empty")).await?;
        tokio::fs::create_dir_all(source.join("nested")).await?;
        let bytes: Vec<u8> = (0..100_003).map(|index| (index % 239) as u8).collect();
        tokio::fs::write(source.join("nested/data.bin"), &bytes).await?;
        tokio::fs::write(source.join("done.txt"), b"complete").await?;
        fixture.sftp.mkdir(&fixture.path("tree")).await?;
        fixture.sftp.mkdir(&fixture.path("tree/nested")).await?;
        fixture
            .sftp
            .write(&fixture.path("tree/nested/data.bin"), &bytes[..17_031])
            .await?;
        fixture
            .sftp
            .write(&fixture.path("tree/done.txt"), b"complete")
            .await?;

        let queue = fixture.sftp.clone().transfer_queue();
        let plan = fixture
            .sftp
            .plan_directory_resume(TransferSpec::upload(&source, fixture.path("tree")))
            .await?;
        assert_eq!(plan.existing_bytes(), 17_039);
        let mut upload = queue.enqueue_directory_resume(plan).await?;
        assert_eq!(completed(&mut upload).await?, bytes.len() as u64 + 8);
        assert_eq!(
            fixture
                .sftp
                .read(&fixture.path("tree/nested/data.bin"), bytes.len())
                .await?,
            bytes
        );
        assert!(
            fixture
                .sftp
                .list(&fixture.path("tree/empty"))
                .await?
                .is_empty()
        );

        let destination = local.path().join("download");
        tokio::fs::create_dir_all(destination.join("nested")).await?;
        tokio::fs::write(destination.join("nested/data.bin"), &bytes[..901]).await?;
        let plan = fixture
            .sftp
            .plan_directory_resume(TransferSpec::download(fixture.path("tree"), &destination))
            .await?;
        let mut download = queue.enqueue_directory_resume(plan).await?;
        assert_eq!(completed(&mut download).await?, bytes.len() as u64 + 8);
        assert_eq!(
            tokio::fs::read(destination.join("nested/data.bin")).await?,
            bytes
        );
        assert_eq!(
            tokio::fs::read(destination.join("done.txt")).await?,
            b"complete"
        );
        assert!(destination.join("empty").is_dir());
        drop(queue);
        fixture
            .close(
                &["tree/nested/data.bin", "tree/done.txt"],
                &["tree/empty", "tree/nested", "tree"],
            )
            .await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_acknowledged_pause_stops_writes_and_cancel_preserves_the_session() -> TestResult {
    tokio::time::timeout(Duration::from_secs(120), async {
        let fixture = Fixture::connect().await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("large.bin");
        let bytes = vec![0x57; 8 * 1024 * 1024];
        tokio::fs::write(&source, &bytes).await?;
        let queue = fixture.sftp.clone().transfer_queue();
        let mut upload = queue
            .enqueue(TransferSpec::upload(&source, fixture.path("partial.bin")))
            .await?;
        let confirmed = pause_after_progress(&mut upload, 0).await?;
        let before = fixture
            .sftp
            .read(&fixture.path("partial.bin"), bytes.len())
            .await?;
        assert_eq!(before.len() as u64, confirmed);
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            fixture
                .sftp
                .read(&fixture.path("partial.bin"), bytes.len())
                .await?,
            before
        );
        upload.cancel();
        loop {
            match upload.recv().await {
                Some(TransferEvent::Cancelled { bytes, .. }) => {
                    assert_eq!(bytes, confirmed);
                    break;
                }
                Some(TransferEvent::Completed { .. } | TransferEvent::Failed { .. }) | None => {
                    return Err("paused cancellation did not finish".into());
                }
                _ => {}
            }
        }
        let plan = fixture
            .sftp
            .plan_file_resume(TransferSpec::upload(&source, fixture.path("partial.bin")))
            .await?;
        let mut resumed = queue.enqueue_resume(plan).await?;
        assert_eq!(completed(&mut resumed).await?, bytes.len() as u64);
        assert_eq!(
            fixture
                .sftp
                .read(&fixture.path("partial.bin"), bytes.len())
                .await?,
            bytes
        );
        let echo = fixture
            .ssh
            .exec("printf keelshell-openssh-interoperability")
            .await?;
        assert_eq!(echo.exit_status, Some(0));
        assert_eq!(echo.stdout, b"keelshell-openssh-interoperability");
        drop(queue);
        fixture.close(&["partial.bin"], &[]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_resume_rejects_a_replaced_inode_without_writing_either_file() -> TestResult {
    tokio::time::timeout(Duration::from_secs(120), async {
        let fixture = Fixture::connect().await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source.bin");
        let bytes = vec![0x57; 8 * 1024 * 1024];
        tokio::fs::write(&source, &bytes).await?;
        let existing = 137_003;
        fixture
            .sftp
            .write(&fixture.path("partial.bin"), &bytes[..existing])
            .await?;
        let plan = fixture
            .sftp
            .plan_file_resume(TransferSpec::upload(&source, fixture.path("partial.bin")))
            .await?;
        let queue = fixture.sftp.clone().transfer_queue();
        let mut resumed = queue.enqueue_resume(plan).await?;
        let confirmed = pause_after_progress(&mut resumed, existing as u64).await?;
        assert!(confirmed > existing as u64 && confirmed < bytes.len() as u64);

        // This opt-in fixture runs OpenSSH on this same host. Keep the original
        // inode open while atomically substituting another file at its name.
        // Equal length, mode and mtime deliberately defeat metadata-only checks.
        let target = PathBuf::from(fixture.path("partial.bin"));
        let replacement = PathBuf::from(fixture.path("replacement.bin"));
        let (old_file, poison) = tokio::task::spawn_blocking(move || -> TestResult<_> {
            use std::io::Write;
            let old_file = std::fs::File::open(&target)?;
            let original = old_file.metadata()?;
            assert_eq!(original.len(), confirmed);
            let poison = vec![0xa7; usize::try_from(confirmed)?];
            let mut replacement_file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&replacement)?;
            replacement_file.write_all(&poison)?;
            replacement_file.set_permissions(original.permissions())?;
            replacement_file.set_times(
                std::fs::FileTimes::new()
                    .set_accessed(original.accessed()?)
                    .set_modified(original.modified()?),
            )?;
            let replacement_metadata = replacement_file.metadata()?;
            assert_eq!(replacement_metadata.len(), original.len());
            assert_eq!(replacement_metadata.modified()?, original.modified()?);
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(replacement_metadata.mode(), original.mode());
                assert_ne!(replacement_metadata.ino(), original.ino());
            }
            drop(replacement_file);
            std::fs::rename(&replacement, &target)?;
            Ok((old_file, poison))
        })
        .await??;

        resumed.resume();
        failed(&mut resumed).await?;
        assert_eq!(
            fixture
                .sftp
                .read(&fixture.path("partial.bin"), bytes.len())
                .await?,
            poison
        );
        assert_eq!(tokio::fs::read(fixture.path("partial.bin")).await?, poison);
        // A failed continuation must not append to the unlinked original inode.
        assert_eq!(old_file.metadata()?.len(), confirmed);
        drop(old_file);
        drop(queue);
        fixture.close(&["partial.bin"], &[]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_remote_completion_is_literal_read_only_and_uses_canonical_paths() -> TestResult {
    use keelshell_session::{CompletionKind, CompletionQuery};
    tokio::time::timeout(Duration::from_secs(45), async {
        let fixture = Fixture::connect().await?;
        let quoted = "a'\"中文 file";
        let metacharacters = "$(touch completion-must-not-exist)";
        fixture.sftp.mkdir(&fixture.path("nested")).await?;
        fixture
            .sftp
            .write(&fixture.path(quoted), b"unchanged")
            .await?;
        fixture
            .sftp
            .write(&fixture.path(metacharacters), b"literal only")
            .await?;
        #[cfg(unix)]
        std::os::unix::fs::symlink("nested", fixture.path("linked directory"))?;

        let base = fixture.ssh.completion_base().await?;
        assert!(base.starts_with('/'));
        let result = fixture
            .ssh
            .complete_remote(CompletionQuery::Paths {
                directory: format!("{}/nested/..", fixture.directory),
                prefix: String::new(),
                directories_only: false,
            })
            .await?;
        // macOS resolves /var to /private/var; bind the actual server REALPATH.
        let canonical = fixture.sftp.canonicalize(&fixture.directory).await?;
        assert_eq!(
            result.resolved_directory.as_deref(),
            Some(canonical.as_str())
        );
        assert!(
            result
                .candidates
                .iter()
                .any(|row| row.name == quoted && row.path == format!("{canonical}/{quoted}"))
        );
        assert!(
            result
                .candidates
                .iter()
                .any(|row| row.name == metacharacters)
        );
        #[cfg(unix)]
        assert!(
            result
                .candidates
                .iter()
                .any(|row| row.name == "linked directory"
                    && row.is_symlink
                    && row.kind == CompletionKind::Directory)
        );
        let directories = fixture
            .ssh
            .complete_remote(CompletionQuery::Paths {
                directory: fixture.directory.clone(),
                prefix: String::new(),
                directories_only: true,
            })
            .await?;
        assert!(
            directories
                .candidates
                .iter()
                .all(|row| row.kind == CompletionKind::Directory)
        );
        assert!(
            directories
                .candidates
                .iter()
                .any(|row| row.name == "nested")
        );
        let commands = fixture
            .ssh
            .complete_remote(CompletionQuery::Commands {
                prefix: "sh".into(),
            })
            .await?;
        assert!(
            commands
                .candidates
                .iter()
                .any(|row| row.name == "sh" && row.kind == CompletionKind::Executable)
        );
        assert!(
            commands
                .candidates
                .iter()
                .all(|row| row.path.starts_with('/') && row.name.starts_with("sh"))
        );
        let hostile = fixture
            .ssh
            .complete_remote(CompletionQuery::Commands {
                prefix: "$(touch completion-must-not-exist)".into(),
            })
            .await?;
        assert!(hostile.candidates.is_empty());
        assert_eq!(
            fixture.sftp.read(&fixture.path(quoted), 32).await?,
            b"unchanged"
        );
        assert_eq!(
            fixture.sftp.read(&fixture.path(metacharacters), 32).await?,
            b"literal only"
        );
        assert!(!tokio::fs::try_exists(fixture.path("completion-must-not-exist")).await?);
        assert_eq!(
            fixture.ssh.exec("printf survives").await?.stdout,
            b"survives"
        );
        #[cfg(unix)]
        fixture
            .sftp
            .remove(&fixture.path("linked directory"))
            .await?;
        fixture.close(&[quoted, metacharacters], &["nested"]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires a disposable OpenSSH loopback server and KEELSHELL_OPENSSH_* settings"]
async fn openssh_batch_exec_preserves_results_policy_and_unknown_timeout() -> TestResult {
    use keelshell_session::{
        BatchNotStartedReason, BatchOptions, BatchOutcome, BatchPolicy, BatchTarget,
        BatchUnknownReason, start_batch,
    };
    tokio::time::timeout(Duration::from_secs(30), async {
        let fixture = Fixture::connect().await?;
        let independent = SshSession::connect(Fixture::options()?).await?;
        let target = |session: &SshSession, command: &str| BatchTarget {
            id: uuid::Uuid::new_v4(),
            session: session.clone(),
            command: command.into(),
        };
        let rows = vec![
            target(
                &fixture.ssh,
                "printf 'first\\n'; printf 'warning' >&2; exit 0",
            ),
            target(&independent, "printf second; exit 7"),
        ];
        let ids: Vec<_> = rows.iter().map(|row| row.id).collect();
        let receipt = start_batch(
            rows,
            BatchOptions {
                concurrency: 2,
                timeout: Duration::from_secs(5),
                ..Default::default()
            },
        )?
        .finish()
        .await?;
        assert_eq!(
            receipt.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            ids
        );
        assert_eq!(receipt.rows[0].outcome, BatchOutcome::Exited { code: 0 });
        assert_eq!(receipt.rows[0].stdout, b"first\n");
        assert_eq!(receipt.rows[0].stderr, b"warning");
        assert_eq!(receipt.rows[1].outcome, BatchOutcome::Exited { code: 7 });
        assert_eq!(receipt.rows[1].stdout, b"second");
        let receipt = start_batch(
            vec![
                target(&fixture.ssh, "exit 9"),
                target(&independent, "printf queued"),
            ],
            BatchOptions {
                concurrency: 1,
                policy: BatchPolicy::StopAfterFailure,
                ..Default::default()
            },
        )?
        .finish()
        .await?;
        assert_eq!(receipt.rows[0].outcome, BatchOutcome::Exited { code: 9 });
        assert_eq!(
            receipt.rows[1].outcome,
            BatchOutcome::NotStarted {
                reason: BatchNotStartedReason::StoppedAfterFailure
            }
        );
        assert!(receipt.stopped_after_failure);
        let receipt = start_batch(
            vec![target(&fixture.ssh, "printf started; sleep 2; printf late")],
            BatchOptions {
                timeout: Duration::from_secs(1),
                ..Default::default()
            },
        )?
        .finish()
        .await?;
        assert_eq!(
            receipt.rows[0].outcome,
            BatchOutcome::Unknown {
                reason: BatchUnknownReason::Timeout
            }
        );
        assert_eq!(receipt.rows[0].stdout, b"started");
        assert_eq!(
            fixture.ssh.exec("printf survives").await?.stdout,
            b"survives"
        );
        assert_eq!(
            independent.exec("printf independent").await?.stdout,
            b"independent"
        );
        independent.close().await?;
        fixture.close(&[], &[]).await
    })
    .await?
}
