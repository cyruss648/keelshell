//! Independent rz/sz peers on the production SSH shell's actual raw PTY.
//!
//! Run with `scripts/openssh_interop.py --zmodem`. The runner supplies disposable
//! pinned SSH credentials; callers supply existing rz/sz executable paths. No
//! peer implementation is bundled, installed or treated as application code.
//! Optional `KEELSHELL_ZMODEM_RZ_ARGS`/`SZ_ARGS` JSON arrays select a peer's
//! literal argv; omitted arrays retain the original lrzsz invocation.

use std::{collections::VecDeque, error::Error, path::PathBuf, time::Duration};

use keelshell_core::zmodem::HeaderFormat;
use keelshell_session::{
    SessionEvent, ShellEnd, SshAuth, SshOptions, SshSession,
    zmodem::{
        ZmodemControl as Control, ZmodemController, ZmodemDirection, ZmodemFailure,
        ZmodemUiEvent as Ui,
    },
};
use russh::keys::{HashAlg, PublicKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn peer_arguments(encoded: Option<&str>) -> TestResult<Vec<String>> {
    let Some(encoded) = encoded else {
        return Ok(vec!["-b".into(), "-vv".into()]);
    };
    if encoded.len() > 16_384 {
        return Err("peer argument JSON exceeds its byte budget".into());
    }
    let arguments: Vec<String> = serde_json::from_str(encoded)?;
    if arguments.len() > 32
        || arguments.iter().any(|argument| {
            argument.len() > 2048 || argument.bytes().any(|byte| byte < 32 || byte == 127)
        })
        || arguments.iter().map(String::len).sum::<usize>() > 8192
    {
        return Err("peer arguments exceed their count, byte or control-character budget".into());
    }
    Ok(arguments)
}

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
    DownloadBatch(PathBuf),
    CancelAfterSaved(PathBuf),
    Preserve(PathBuf),
    Cancel,
}

struct Report {
    events: Vec<Ui>,
    end: ShellEnd,
    terminal: Vec<u8>,
    incoming_format: Option<HeaderFormat>,
}

#[derive(Debug, Default)]
struct FirstDataHeader {
    prefix: Vec<u8>,
    format: Option<HeaderFormat>,
}

// Independent CRC vectors also covered by the core golden-wire tests.
const DATA16: &[u8] = b"*\x18A\x18J\x18@\x18@\x18@\x18@F\xae";
const DATA32: &[u8] = b"*\x18C\x18J\x18@\x18@\x18@\x18@\xbc\xef\x18\xd2\x18\xcc";

impl FirstDataHeader {
    fn observe(&mut self, bytes: &[u8]) {
        if self.format.is_some() {
            return;
        }
        self.prefix
            .extend(bytes.iter().take(96 - self.prefix.len()));
        // Armed only after the single file's review. Require the first header
        // at offset zero, including its CRC; never search through file payload.
        let candidate = self
            .prefix
            .iter()
            .position(|byte| !matches!(byte, 0x11 | 0x13 | 0x91 | 0x93 | b'\r' | b'\n'))
            .map_or(&[][..], |index| &self.prefix[index..]);
        let pads = candidate.iter().take_while(|byte| **byte == b'*').count();
        if !(1..=2).contains(&pads) {
            return;
        }
        for (golden, format) in [
            (DATA16, HeaderFormat::Binary16),
            (DATA32, HeaderFormat::Binary32),
        ] {
            if candidate[pads..].starts_with(&golden[1..]) {
                self.format = Some(format);
                break;
            }
        }
    }
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
        self.run_peer(receiving, flags, names, intent, false).await
    }

    async fn run_peer(
        &self,
        receiving: bool,
        flags: &[&str],
        names: &[&str],
        intent: Intent,
        interactive: bool,
    ) -> TestResult<Report> {
        let peer = std::env::var(if receiving {
            "KEELSHELL_ZMODEM_RZ"
        } else {
            "KEELSHELL_ZMODEM_SZ"
        })?;
        if !peer.starts_with('/') || peer.chars().any(char::is_control) {
            return Err("configure an absolute, control-free rz/sz peer path".into());
        }
        let argument_variable = if receiving {
            "KEELSHELL_ZMODEM_RZ_ARGS"
        } else {
            "KEELSHELL_ZMODEM_SZ_ARGS"
        };
        let encoded = match std::env::var(argument_variable) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(error.into()),
        };
        let args = peer_arguments(encoded.as_deref())?;
        // A peer may use a positional rz/sz subcommand; shared test options
        // must precede that subcommand rather than become file arguments.
        let args = flags
            .iter()
            .copied()
            .chain(args.iter().map(String::as_str))
            .chain(names.iter().copied())
            .map(quote)
            .collect::<Vec<_>>()
            .join(" ");
        let command = format!(
            "cd {} && stty raw -echo -onlcr && {}{} {args} 2>{}{}\n",
            quote(&self.remote),
            if interactive { "" } else { "exec " },
            quote(&peer),
            quote(&self.path("peer.log")),
            if interactive {
                "; printf '\\n__KS_PEER_RETURN__\\n'"
            } else {
                ""
            },
        );
        let mut shell = self.session.start_shell(30, 100).await?;
        shell.write(command.as_bytes()).await?;
        let mut controller = ZmodemController::new(self.session.clone(), "owned loopback OpenSSH");
        let mut events = Vec::new();
        let mut terminal = Vec::new();
        let mut ended = false;
        let mut preserve_rejected = false;
        let mut saved_count = 0;
        let mut peer_headers = VecDeque::new();
        let mut incoming_header: Option<FirstDataHeader> = None;
        while !ended {
            let event = tokio::select! {
                event = shell.recv() => Some(event.ok_or("peer closed before protocol result")?),
                _ = tokio::time::sleep(Duration::from_millis(20)) => None,
            };
            if let Some(SessionEvent::Data(bytes)) = &event {
                if let Some(header) = &mut incoming_header {
                    header.observe(bytes);
                }
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
            let mut effects = match event {
                Some(SessionEvent::Data(bytes)) => controller.feed(&self.session, &bytes).await,
                Some(SessionEvent::Exited { code, success }) => {
                    controller.transport_exited(&self.session, code, success)
                }
                Some(other) => {
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
                None => controller.tick(),
            };
            loop {
                // Flush actual SSH output before accepting the corresponding
                // event. Merely producing OO is not a successful write.
                let token = controller.protocol_output_token();
                let has_writes = !effects.writes.is_empty();
                for bytes in effects.writes.drain(..) {
                    shell.writer().write_confirmed(&bytes).await?;
                }
                if has_writes && let Some(token) = token {
                    controller.protocol_output_flushed(token);
                }
                terminal.append(&mut effects.terminal);
                if terminal.len() > 32 * 1024 {
                    return Err("fixture terminal output exceeded its bounded capture".into());
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
                            Ui::DownloadOffered {
                                id, offer, name, ..
                            },
                            Intent::DownloadBatch(directory) | Intent::CancelAfterSaved(directory),
                        ) => {
                            if matches!(intent, Intent::CancelAfterSaved(_)) && saved_count > 0 {
                                Some(Control::Cancel { transfer: *id })
                            } else {
                                Some(Control::PrepareReceive {
                                    transfer: *id,
                                    offer: *offer,
                                    destination: directory.join(name),
                                })
                            }
                        }
                        (Ui::FileSaved { .. }, _) => {
                            saved_count += 1;
                            None
                        }
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
                            if matches!(intent, Intent::Download(_)) && incoming_header.is_none() {
                                incoming_header = Some(FirstDataHeader::default());
                            }
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
                        (
                            Ui::Finished { .. }
                            | Ui::EndedWithoutFarewell { .. }
                            | Ui::Cancelled { .. },
                            _,
                        ) => {
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
        if interactive {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !terminal
                    .windows(b"__KS_PEER_RETURN__".len())
                    .any(|part| part == b"__KS_PEER_RETURN__")
                {
                    match shell.recv().await {
                        Some(SessionEvent::Data(bytes)) => {
                            terminal.extend(controller.feed(&self.session, &bytes).await.terminal);
                            if terminal.len() > 32 * 1024 {
                                return Err(
                                    "fixture terminal output exceeded its bounded capture".into()
                                );
                            }
                        }
                        other => {
                            return Err(format!(
                                "interactive shell ended before its return marker: {other:?}"
                            )
                            .into());
                        }
                    }
                }
                shell
                    .write(b"printf '__KS_AFTER_ZMODEM__\\n'; exit 0\n")
                    .await?;
                while let Some(event) = shell.recv().await {
                    match event {
                        SessionEvent::Data(bytes) => {
                            terminal.extend(bytes);
                            if terminal.len() > 32 * 1024 {
                                return Err(
                                    "fixture terminal output exceeded its bounded capture".into()
                                );
                            }
                        }
                        SessionEvent::Exited {
                            code: 0,
                            success: true,
                        } => break,
                        other => {
                            return Err(format!(
                                "interactive shell did not finish cleanly: {other:?}"
                            )
                            .into());
                        }
                    }
                }
                Ok::<_, Box<dyn Error + Send + Sync>>(())
            })
            .await??;
            assert!(
                terminal
                    .windows(b"__KS_AFTER_ZMODEM__".len())
                    .any(|part| part == b"__KS_AFTER_ZMODEM__")
            );
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
        let incoming_format = incoming_header.and_then(|header| header.format);
        eprintln!(
            "peer receipt: first_data_header={incoming_format:?}; strict_OO={}; missing_OO={}; saved={saved_count}; shell_end={end:?}",
            events
                .iter()
                .filter(|event| matches!(event, Ui::Finished { .. }))
                .count(),
            events
                .iter()
                .filter(|event| matches!(event, Ui::EndedWithoutFarewell { .. }))
                .count(),
        );
        Ok(Report {
            events,
            end,
            terminal,
            incoming_format,
        })
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
    // A peer may finish an empty file with ZEOF without a ZDATA frame.
    if !bytes.is_empty() {
        let expected = if flags.contains(&"-o") {
            HeaderFormat::Binary16
        } else {
            HeaderFormat::Binary32
        };
        assert_eq!(report.incoming_format, Some(expected));
    }
    assert_eq!(std::fs::read(&destination)?, bytes);
    completed(&report, 1);
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
async fn real_sz_returns_to_the_same_shell_after_a_bounded_receive_farewell() -> TestResult {
    tokio::time::timeout(Duration::from_secs(45), async {
        let fixture = Fixture::new().await?;
        let name = "interactive-源.bin";
        let bytes = binary();
        fixture.sftp.write(&fixture.path(name), &bytes).await?;
        let destination = fixture.local.join(name);
        let report = fixture.run_peer(
            false,
            &[],
            &[name],
            Intent::Download(destination.clone()),
            true,
        ).await?;
        assert_eq!(std::fs::read(destination)?, bytes);
        assert_eq!(report.end, ShellEnd::Exited { code: 0 });
        let strict = report.events.iter().filter(|event| matches!(event, Ui::Finished { .. })).count();
        let missing = report.events.iter().filter(|event| matches!(event, Ui::EndedWithoutFarewell { .. })).count();
        assert_eq!(strict + missing, 1, "exactly one explicitly classified receive outcome");
        assert!(!report.events.iter().any(|event| matches!(event, Ui::Failed { .. } | Ui::Cancelled { .. })));
        assert_eq!(report.events.iter().filter(|event| matches!(event, Ui::FileSaved { .. })).count(), 1);
        assert!(report.terminal.windows(b"__KS_PEER_RETURN__".len()).any(|part| part == b"__KS_PEER_RETURN__"));
        assert!(report.terminal.windows(b"__KS_AFTER_ZMODEM__".len()).any(|part| part == b"__KS_AFTER_ZMODEM__"));
        eprintln!("interactive receive outcome: strict_OO={strict}; missing_optional_OO={missing}; same_shell_continued=true");
        fixture.close(&[name]).await
    }).await?
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

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_download_batch_requires_a_separate_review_and_save_for_each_file() -> TestResult {
    tokio::time::timeout(Duration::from_secs(60), async {
        let fixture = Fixture::new().await?;
        let names = ["binary.bin", "逐项审核.txt", "empty.bin"];
        let contents = [binary(), "独立文件审核\r\n".as_bytes().to_vec(), Vec::new()];
        for (name, content) in names.iter().zip(&contents) {
            fixture.sftp.write(&fixture.path(name), content).await?;
        }
        let directory = fixture.local.join("downloads");
        std::fs::create_dir(&directory)?;
        let report = fixture
            .run(false, &[], &names, Intent::DownloadBatch(directory.clone()))
            .await?;
        let offers: Vec<_> = report
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event, Ui::DownloadOffered { .. }))
            .collect();
        let reviews: Vec<_> = report
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event, Ui::Review { .. }))
            .collect();
        let saved: Vec<_> = report
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event, Ui::FileSaved { .. }))
            .collect();
        assert_eq!(offers.len(), names.len());
        assert_eq!(reviews.len(), names.len());
        assert_eq!(saved.len(), names.len());
        let mut offer_ids = std::collections::HashSet::new();
        let mut review_ids = std::collections::HashSet::new();
        for (index, (name, content)) in names.iter().zip(&contents).enumerate() {
            let (
                Ui::DownloadOffered {
                    id,
                    offer,
                    name: offered_name,
                    size,
                },
                Ui::Review {
                    transfer,
                    review,
                    direction,
                    files,
                    destination,
                },
                Ui::FileSaved {
                    id: saved_transfer,
                    path,
                    bytes,
                    sha256,
                },
            ) = (offers[index].1, reviews[index].1, saved[index].1)
            else {
                unreachable!("vectors contain only the filtered event types");
            };
            let expected_path = directory.join(name);
            assert!(offer_ids.insert(*offer), "each file needs a distinct offer");
            assert!(
                review_ids.insert(*review),
                "each file needs a one-use review"
            );
            assert_eq!(id, transfer);
            assert_eq!(id, saved_transfer);
            assert_eq!(offered_name, name);
            assert_eq!(*size, content.len() as u64);
            assert_eq!(*direction, ZmodemDirection::Download);
            assert_eq!(destination.as_ref(), Some(&expected_path));
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, expected_path);
            assert_eq!(files[0].name, *name);
            assert_eq!(files[0].size, content.len() as u64);
            assert_eq!(files[0].sha256, None);
            assert_eq!(path, &expected_path);
            assert_eq!(*bytes, content.len() as u64);
            let expected_sha256: String = Sha256::digest(content)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(sha256, &expected_sha256);
            assert!(offers[index].0 < reviews[index].0);
            assert!(reviews[index].0 < saved[index].0);
            if index > 0 {
                assert_eq!(
                    id,
                    match offers[0].1 {
                        Ui::DownloadOffered { id, .. } => id,
                        _ => unreachable!("only offers were selected"),
                    }
                );
                assert!(saved[index - 1].0 < offers[index].0);
            }
            assert_eq!(std::fs::read(expected_path)?, *content);
        }
        completed(&report, names.len());
        fixture.close(&names).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH runner and independent rz/sz"]
async fn real_sz_cancel_after_one_saved_file_does_not_accept_the_next_offer() -> TestResult {
    tokio::time::timeout(Duration::from_secs(40), async {
        let fixture = Fixture::new().await?;
        let names = ["first.bin", "未审核.bin"];
        let contents = [b"verified first file".as_slice(), b"unreviewed second file"];
        for (name, content) in names.iter().zip(contents) {
            fixture.sftp.write(&fixture.path(name), content).await?;
        }
        let directory = fixture.local.join("downloads");
        std::fs::create_dir(&directory)?;
        let report = fixture
            .run(
                false,
                &[],
                &names,
                Intent::CancelAfterSaved(directory.clone()),
            )
            .await?;
        assert!(report.events.iter().any(|event| matches!(
            event,
            Ui::Cancelled {
                uncertain: false,
                ..
            }
        )));
        assert_eq!(
            report
                .events
                .iter()
                .filter(|event| matches!(event, Ui::FileSaved { .. }))
                .count(),
            1
        );
        assert_eq!(
            report
                .events
                .iter()
                .filter(|event| matches!(event, Ui::Review { .. }))
                .count(),
            1
        );
        assert_eq!(
            report
                .events
                .iter()
                .filter(|event| matches!(event, Ui::DownloadOffered { .. }))
                .count(),
            2
        );
        assert!(!report.events.iter().any(|event| matches!(
            event,
            Ui::Finished { .. } | Ui::EndedWithoutFarewell { .. } | Ui::Failed { .. }
        )));
        assert_eq!(std::fs::read(directory.join(names[0]))?, contents[0]);
        assert_eq!(
            std::fs::read_dir(&directory)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?,
            [std::ffi::OsString::from(names[0])]
        );
        for (name, content) in names.iter().zip(contents) {
            assert_eq!(
                fixture.sftp.read_regular(&fixture.path(name), 1024).await?,
                content
            );
        }
        fixture.close(&names).await
    })
    .await?
}

#[test]
fn first_data_header_survives_every_split_and_ignores_opposite_payload_markers() {
    for (golden, format, opposite_payload) in [
        (DATA16, HeaderFormat::Binary16, b"*\x18C".as_slice()),
        (DATA32, HeaderFormat::Binary32, b"*\x18A".as_slice()),
    ] {
        for pads in [1, 2] {
            let mut wire = vec![0x11, b'\r', b'\n'];
            wire.extend(std::iter::repeat_n(b'*', pads));
            wire.extend_from_slice(&golden[1..]);
            wire.extend_from_slice(opposite_payload);
            for split in 0..=wire.len() {
                let mut observer = FirstDataHeader::default();
                observer.observe(&wire[..split]);
                observer.observe(&wire[split..]);
                observer.observe(opposite_payload);
                assert_eq!(observer.format, Some(format), "{format:?}/{split}");
                assert!(observer.prefix.len() <= 96);
            }
        }
    }
}

#[test]
fn first_data_header_rejects_bad_crc_payload_noise_and_over_budget_prefixes() {
    let mut corrupt = DATA16.to_vec();
    corrupt[DATA16.len() - 1] ^= 1;
    corrupt.extend_from_slice(DATA32);
    for wire in [
        corrupt,
        [b"*\x18C".as_slice(), DATA16].concat(),
        [b"noise".as_slice(), DATA32].concat(),
        [vec![0x11; 97], DATA16.to_vec()].concat(),
    ] {
        let mut observer = FirstDataHeader::default();
        for byte in wire {
            observer.observe(&[byte]);
        }
        assert_eq!(observer.format, None);
        assert!(observer.prefix.len() <= 96);
    }
}

#[test]
fn peer_arguments_default_to_the_original_lrzsz_options() -> TestResult {
    assert_eq!(peer_arguments(None)?, ["-b", "-vv"]);
    Ok(())
}

#[test]
fn peer_arguments_keep_literal_unicode_empty_strings_and_shell_metacharacters() -> TestResult {
    assert_eq!(
        peer_arguments(Some(
            r#"["-raw","sz","","逐项 ' $HOME $(echo literal); *"]"#
        ))?,
        ["-raw", "sz", "", "逐项 ' $HOME $(echo literal); *"]
    );
    Ok(())
}

#[test]
fn peer_arguments_require_a_json_string_array() {
    for encoded in ["", "null", "{}", "\"sz\"", "[1]", "[true]", "[[]]"] {
        assert!(peer_arguments(Some(encoded)).is_err(), "{encoded}");
    }
}

#[test]
fn peer_arguments_reject_all_ascii_controls() -> TestResult {
    for byte in (0_u8..32).chain(std::iter::once(127)) {
        let encoded = serde_json::to_string(&[format!("before{}after", char::from(byte))])?;
        assert!(peer_arguments(Some(&encoded)).is_err(), "control {byte}");
    }
    Ok(())
}

#[test]
fn peer_arguments_bound_the_json_and_argument_count() -> TestResult {
    let boundary = format!("{}[]", " ".repeat(16_382));
    assert!(peer_arguments(Some(&boundary))?.is_empty());
    assert!(peer_arguments(Some(&(boundary + " "))).is_err());
    let boundary = serde_json::to_string(&vec![""; 32])?;
    assert_eq!(peer_arguments(Some(&boundary))?.len(), 32);
    let over = serde_json::to_string(&vec![""; 33])?;
    assert!(peer_arguments(Some(&over)).is_err());
    Ok(())
}

#[test]
fn peer_arguments_enforce_utf8_bytes_per_argument_and_in_total() -> TestResult {
    let value = "界".repeat(682) + "ab";
    assert_eq!(value.len(), 2048);
    let boundary = serde_json::to_string(&vec![value.clone(); 4])?;
    assert_eq!(
        peer_arguments(Some(&boundary))?
            .iter()
            .map(String::len)
            .sum::<usize>(),
        8192
    );
    let over_one = serde_json::to_string(&[value.clone() + "c"])?;
    assert!(peer_arguments(Some(&over_one)).is_err());
    let mut total_over = vec![value; 4];
    total_over.push("c".into());
    let total_over = serde_json::to_string(&total_over)?;
    assert!(peer_arguments(Some(&total_over)).is_err());
    Ok(())
}
