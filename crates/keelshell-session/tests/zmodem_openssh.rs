//! Independent rz/sz peers on the production SSH shell's actual raw PTY.
//!
//! Run with `scripts/openssh_interop.py --zmodem`. The runner supplies disposable
//! pinned SSH credentials; callers supply existing rz/sz executable paths. No
//! peer implementation is bundled, installed or treated as application code.

use std::{collections::VecDeque, error::Error, path::PathBuf, time::Duration};

use keelshell_session::{
    SessionEvent, ShellEnd, SshAuth, SshOptions, SshSession,
    zmodem::{ZmodemControl as Control, ZmodemController, ZmodemFailure, ZmodemUiEvent as Ui},
};
use russh::keys::{HashAlg, PublicKey};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

struct Fixture {
    session: SshSession,
    sftp: keelshell_session::sftp::SftpSession,
    remote: String,
    _local: tempfile::TempDir,
    local: PathBuf,
}

enum Intent {
    Upload(Vec<PathBuf>),
    Download(PathBuf),
    Preserve(PathBuf),
    Cancel,
}

struct Report {
    events: Vec<Ui>,
    end: ShellEnd,
}

impl Fixture {
    async fn new() -> TestResult<Self> {
        let mut options = SshOptions::new("127.0.0.1", std::env::var("KEELSHELL_OPENSSH_USER")?);
        options.port = std::env::var("KEELSHELL_OPENSSH_PORT")?.parse()?;
        let host = std::fs::read_to_string(std::env::var("KEELSHELL_OPENSSH_HOST_KEY")?)?;
        options.expected_host_key = Some(
            PublicKey::from_openssh(&host)?
                .fingerprint(HashAlg::Sha256)
                .to_string(),
        );
        options.auth = SshAuth::PrivateKey {
            path: PathBuf::from(std::env::var("KEELSHELL_OPENSSH_CLIENT_KEY")?),
            passphrase: None,
        };
        options.timeout = Duration::from_secs(10);
        let session = SshSession::connect(options).await?;
        let sftp = session.sftp().await?;
        let root = std::env::var("KEELSHELL_OPENSSH_ROOT")?;
        if !root.starts_with('/') || root == "/" || root.split('/').any(|part| part == "..") {
            return Err("an absolute disposable remote root is required".into());
        }
        let remote = format!("{}/zmodem-{}", root.trim_end_matches('/'), Uuid::new_v4());
        sftp.mkdir(&remote).await?;
        let local = tempfile::tempdir()?;
        let path = local.path().canonicalize()?;
        Ok(Self {
            session,
            sftp,
            remote,
            _local: local,
            local: path,
        })
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.remote)
    }

    async fn run(
        &self,
        receiving: bool,
        flags: &[&str],
        names: &[&str],
        intent: Intent,
    ) -> TestResult<Report> {
        let peer = std::env::var(if receiving {
            "KEELSHELL_ZMODEM_RZ"
        } else {
            "KEELSHELL_ZMODEM_SZ"
        })?;
        if !peer.starts_with('/') || peer.chars().any(char::is_control) {
            return Err("configure an absolute, control-free rz/sz peer path".into());
        }
        let mut args = vec!["-b", "-vv"];
        args.extend_from_slice(flags);
        let args = args
            .into_iter()
            .chain(names.iter().copied())
            .map(quote)
            .collect::<Vec<_>>()
            .join(" ");
        let command = format!(
            "cd {} && stty raw -echo -onlcr && exec {} {args} 2>{}\n",
            quote(&self.remote),
            quote(&peer),
            quote(&self.path("peer.log")),
        );
        let mut shell = self.session.start_shell(30, 100).await?;
        shell.write(command.as_bytes()).await?;
        let mut controller = ZmodemController::new(self.session.clone(), "owned loopback OpenSSH");
        let mut events = Vec::new();
        let mut ended = false;
        let mut preserve_rejected = false;
        let mut peer_headers = VecDeque::new();
        while !ended {
            let Some(event) = shell.recv().await else {
                return Err("peer closed before protocol result".into());
            };
            if let SessionEvent::Data(bytes) = &event {
                peer_headers.push_back(
                    bytes
                        .iter()
                        .take(96)
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                );
                if peer_headers.len() > 6 {
                    peer_headers.pop_front();
                }
            }
            let mut effects = controller
                .feed(
                    &self.session,
                    match &event {
                        SessionEvent::Data(bytes) => bytes,
                        other => {
                            let log = self
                                .sftp
                                .read_regular(&self.path("peer.log"), 64 * 1024)
                                .await
                                .unwrap_or_default();
                            return Err(format!(
                                "unexpected SSH terminal event: {other:?}; controller: {:?}; latest incoming headers: {peer_headers:?}; prior events: {:?}; peer diagnostics: {}",
                                controller.state(),
                                events.iter().rev().take(5).collect::<Vec<_>>(),
                                String::from_utf8_lossy(&log),
                            ).into());
                        }
                    },
                )
                .await;
            loop {
                // Flush actual SSH output before accepting the corresponding
                // event. Merely producing OO is not a successful write.
                for bytes in effects.writes.drain(..) {
                    shell.write(&bytes).await?;
                }
                let mut pending: VecDeque<_> = effects.events.drain(..).collect();
                if pending.is_empty() {
                    break;
                }
                effects = Default::default();
                while let Some(event) = pending.pop_front() {
                    let action = match (&event, &intent) {
                        (Ui::UploadRequested { id }, Intent::Upload(sources)) => {
                            Some(Control::PrepareUpload {
                                transfer: *id,
                                sources: sources.clone(),
                            })
                        }
                        (
                            Ui::DownloadOffered { id, offer, .. },
                            Intent::Download(destination) | Intent::Preserve(destination),
                        ) => Some(Control::PrepareReceive {
                            transfer: *id,
                            offer: *offer,
                            destination: destination.clone(),
                        }),
                        (
                            Ui::UploadRequested { id } | Ui::DownloadOffered { id, .. },
                            Intent::Cancel,
                        ) => Some(Control::Cancel { transfer: *id }),
                        (
                            Ui::Review {
                                transfer,
                                review,
                                files,
                                ..
                            },
                            _,
                        ) => {
                            assert!(
                                !files.is_empty(),
                                "real review must describe the selected file"
                            );
                            Some(Control::Confirm {
                                transfer: *transfer,
                                review: *review,
                            })
                        }
                        (
                            Ui::ActionRejected {
                                id,
                                reason: ZmodemFailure::DestinationExists,
                            },
                            Intent::Preserve(_),
                        ) => {
                            preserve_rejected = true;
                            Some(Control::Cancel { transfer: *id })
                        }
                        (Ui::Finished { .. } | Ui::Cancelled { .. }, _) => {
                            ended = true;
                            None
                        }
                        (Ui::Failed { .. } | Ui::ActionRejected { .. }, _) => {
                            let log = self
                                .sftp
                                .read_regular(&self.path("peer.log"), 64 * 1024)
                                .await
                                .unwrap_or_default();
                            return Err(format!("real peer transfer rejected: {event:?}; latest incoming headers: {peer_headers:?}; prior events: {:?}; peer diagnostics: {}", events.iter().rev().take(5).collect::<Vec<_>>(), String::from_utf8_lossy(&log)).into());
                        }
                        _ => None,
                    };
                    events.push(event);
                    if let Some(action) = action {
                        let more = controller.control(&self.session, action).await;
                        effects.writes.extend(more.writes);
                        effects.events.extend(more.events);
                    }
                }
            }
        }
        if matches!(intent, Intent::Preserve(_)) {
            assert!(preserve_rejected);
        }
        // The peer replaces its shell, so its independently observed exit
        // establishes whether it accepted the final protocol trailer.
        let peer_cancelled = events
            .iter()
            .any(|event| matches!(event, Ui::Cancelled { .. }));
        let end = tokio::time::timeout(
            Duration::from_secs(if peer_cancelled { 2 } else { 5 }),
            async {
                loop {
                    if let Some(end) = shell.completion() {
                        break end;
                    }
                    let _ = shell.recv().await;
                }
            },
        )
        .await;
        // CAN has no durable peer-exit acknowledgement. Close this owned
        // fixture channel after the bounded observation without claiming one.
        shell.close().await?;
        let end = match end {
            Ok(end) => end,
            Err(_) if peer_cancelled => ShellEnd::Cancelled,
            Err(error) => return Err(error.into()),
        };
        Ok(Report { events, end })
    }

    async fn close(self, names: &[&str]) -> TestResult {
        for name in names {
            if self.sftp.inspect_entry(&self.path(name)).await?.is_some() {
                self.sftp.remove(&self.path(name)).await?;
            }
        }
        self.sftp.remove(&self.path("peer.log")).await?;
        self.sftp.rmdir(&self.remote).await?;
        self.sftp.close().await?;
        self.session.close().await?;
        Ok(())
    }
}

fn completed(report: &Report, saved: usize) {
    assert_eq!(report.end, ShellEnd::Exited { code: 0 });
    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, Ui::Finished { .. }))
    );
    assert_eq!(
        report
            .events
            .iter()
            .filter(|event| matches!(event, Ui::FileSaved { .. }))
            .count(),
        saved
    );
}

fn binary() -> Vec<u8> {
    (0..262_156).map(|index| (index % 256) as u8).collect()
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_rz_receives_reviewed_binary_and_unicode_batch() -> TestResult {
    tokio::time::timeout(Duration::from_secs(45), async {
        let fixture = Fixture::new().await?;
        let names = ["binary.bin", "源文件.bin"];
        let bytes = binary();
        for name in names {
            std::fs::write(fixture.local.join(name), &bytes)?;
        }
        let report = fixture
            .run(
                true,
                &[],
                &[],
                Intent::Upload(names.iter().map(|name| fixture.local.join(name)).collect()),
            )
            .await?;
        completed(&report, 0);
        for name in names {
            assert_eq!(
                fixture
                    .sftp
                    .read_regular(&fixture.path(name), 1_000_000)
                    .await?,
                bytes
            );
        }
        fixture.close(&names).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_rz_receives_a_reviewed_empty_file() -> TestResult {
    tokio::time::timeout(Duration::from_secs(25), async {
        let fixture = Fixture::new().await?;
        let source = fixture.local.join("empty.bin");
        std::fs::write(&source, [])?;
        completed(
            &fixture
                .run(true, &[], &[], Intent::Upload(vec![source]))
                .await?,
            0,
        );
        assert!(
            fixture
                .sftp
                .read_regular(&fixture.path("empty.bin"), 1)
                .await?
                .is_empty()
        );
        fixture.close(&["empty.bin"]).await
    })
    .await?
}

async fn receive(name: &str, bytes: &[u8], flags: &[&str]) -> TestResult {
    let fixture = Fixture::new().await?;
    fixture.sftp.write(&fixture.path(name), bytes).await?;
    let destination = fixture.local.join(name);
    let report = fixture
        .run(false, flags, &[name], Intent::Download(destination.clone()))
        .await?;
    completed(&report, 1);
    assert_eq!(std::fs::read(&destination)?, bytes);
    fixture.close(&[name]).await
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_sends_binary_unicode_content_over_crc32() -> TestResult {
    tokio::time::timeout(
        Duration::from_secs(45),
        receive("源文件.bin", &binary(), &[]),
    )
    .await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_sends_binary_content_over_crc16() -> TestResult {
    tokio::time::timeout(
        Duration::from_secs(45),
        receive("binary.bin", &binary(), &["-o"]),
    )
    .await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_sends_an_individually_reviewed_empty_file() -> TestResult {
    tokio::time::timeout(Duration::from_secs(25), receive("empty.bin", &[], &[])).await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_existing_local_destination_is_preserved_and_peer_cancelled() -> TestResult {
    tokio::time::timeout(Duration::from_secs(25), async {
        let fixture = Fixture::new().await?;
        fixture
            .sftp
            .write(&fixture.path("source.bin"), b"remote replacement")
            .await?;
        let destination = fixture.local.join("existing.bin");
        std::fs::write(&destination, b"preserve existing")?;
        let report = fixture
            .run(
                false,
                &[],
                &["source.bin"],
                Intent::Preserve(destination.clone()),
            )
            .await?;
        assert!(report.events.iter().any(|event| matches!(
            event,
            Ui::Cancelled {
                uncertain: false,
                ..
            }
        )));
        assert!(
            !report
                .events
                .iter()
                .any(|event| matches!(event, Ui::FileSaved { .. } | Ui::Finished { .. }))
        );
        assert_eq!(std::fs::read(destination)?, b"preserve existing");
        fixture.close(&["source.bin"]).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_rz_cancel_before_source_review_does_not_create_files() -> TestResult {
    tokio::time::timeout(Duration::from_secs(25), async {
        let fixture = Fixture::new().await?;
        let report = fixture.run(true, &[], &[], Intent::Cancel).await?;
        assert!(report.events.iter().any(|event| matches!(
            event,
            Ui::Cancelled {
                uncertain: false,
                ..
            }
        )));
        assert_eq!(
            fixture
                .sftp
                .list(&fixture.remote)
                .await?
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["peer.log"]
        );
        fixture.close(&[]).await
    })
    .await?
}
