//! Real encrypted SSH channels with controlled discovery replies; no shell or tool is executed.

use keelshell_session::{
    ConnectionState, SessionError, SshAuth, SshOptions, SshSession,
    archive_tools::{
        ArchiveTool, ArchiveToolAvailability, ArchiveToolObservation, ArchiveToolReport,
        probe_command,
    },
};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    collections::HashMap,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
const FRAME: &[u8] = b"KEELSHELL_ARCHIVE_TOOLS_V1\0";
const HEALTH_COMMAND: &str = "archive-tool-fixture-health";

fn body(status: &str, resolution: impl Fn(ArchiveTool) -> String) -> Vec<u8> {
    let mut bytes = FRAME.to_vec();
    for tool in ArchiveTool::ALL {
        let path = resolution(tool);
        for field in [tool.name(), status, path.as_str()] {
            bytes.extend_from_slice(field.as_bytes());
            bytes.push(0);
        }
    }
    bytes
}

fn found_report() -> ArchiveToolReport {
    ArchiveToolReport {
        tools: ArchiveTool::ALL
            .into_iter()
            .map(|tool| ArchiveToolObservation {
                tool,
                availability: ArchiveToolAvailability::Found,
                path: Some(format!("/fixture/归档 tools/{}", tool.name())),
            })
            .collect(),
    }
}

#[derive(Clone)]
struct Response {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_status: Option<u32>,
    chunk_size: usize,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            stdout: body("found", |tool| {
                format!("/fixture/归档 tools/{}", tool.name())
            }),
            stderr: Vec::new(),
            exit_status: Some(0),
            chunk_size: 4096,
        }
    }
}

#[derive(Clone)]
enum Mode {
    Reply(Response),
    HoldOpen,
    HoldAck,
    HoldResult,
    HoldClose,
    Reject,
}

#[derive(Default)]
struct Observed {
    opened: AtomicUsize,
    closed: AtomicUsize,
    probes: AtomicUsize,
    late_replies: AtomicUsize,
    subsystems: AtomicUsize,
    unexpected: AtomicUsize,
    commands: Mutex<Vec<Vec<u8>>>,
}

struct Peer {
    mode: Mode,
    observed: Arc<Observed>,
    gate: Arc<Semaphore>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        let index = self.observed.opened.fetch_add(1, Ordering::AcqRel);
        if matches!(self.mode, Mode::HoldOpen) && index == 0 {
            let gate = self.gate.clone();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(15), gate.acquire()).await
                {
                    permit.forget();
                    reply.accept().await;
                }
            });
        } else {
            reply.accept().await;
        }
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if data == HEALTH_COMMAND.as_bytes() {
            session.channel_success(id)?;
            session.data(id, b"alive".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.close(id)?;
            self.channels.remove(&id);
            return Ok(());
        }
        if data != probe_command().as_bytes() {
            self.observed.unexpected.fetch_add(1, Ordering::AcqRel);
            session.channel_failure(id)?;
            return Ok(());
        }
        self.observed
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(data.to_vec());
        self.observed.probes.fetch_add(1, Ordering::AcqRel);
        if matches!(self.mode, Mode::Reject) {
            session.channel_failure(id)?;
            return Ok(());
        }
        if matches!(self.mode, Mode::HoldAck) {
            return Ok(());
        }
        session.channel_success(id)?;
        if matches!(self.mode, Mode::HoldResult) {
            let gate = self.gate.clone();
            let observed = self.observed.clone();
            let handle = session.handle();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(15), gate.acquire()).await
                {
                    permit.forget();
                    observed.late_replies.fetch_add(1, Ordering::AcqRel);
                    let response = Response::default();
                    let _ = handle.data(id, response.stdout).await;
                    let _ = handle.exit_status_request(id, 0).await;
                    let _ = handle.close(id).await;
                }
            });
            return Ok(());
        }
        let response = match &self.mode {
            Mode::Reply(response) => response.clone(),
            _ => Response::default(),
        };
        for chunk in response.stdout.chunks(response.chunk_size) {
            session.data(id, chunk.to_vec())?;
        }
        for chunk in response.stderr.chunks(response.chunk_size) {
            session.extended_data(id, 1, chunk.to_vec())?;
        }
        if let Some(status) = response.exit_status {
            session.exit_status_request(id, status)?;
        }
        if !matches!(self.mode, Mode::HoldClose) {
            session.eof(id)?;
            session.close(id)?;
            self.channels.remove(&id);
        }
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        _: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed.subsystems.fetch_add(1, Ordering::AcqRel);
        session.channel_failure(id)?;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.observed.closed.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

struct Fixture {
    task: JoinHandle<()>,
    session: SshSession,
    observed: Arc<Observed>,
    gate: Arc<Semaphore>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Fixture {
    async fn new(mode: Mode) -> TestResult<Self> {
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x43; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let observed = Arc::new(Observed::default());
        let gate = Arc::new(Semaphore::new(0));
        let peer = Peer {
            mode,
            observed: observed.clone(),
            gate: gate.clone(),
            channels: HashMap::new(),
            jobs: JoinSet::new(),
        };
        let task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, stream, peer).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = port;
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(3);
        let session = match SshSession::connect(options).await {
            Ok(session) => session,
            Err(error) => {
                task.abort();
                let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
                return Err(error.into());
            }
        };
        Ok(Self {
            task,
            session,
            observed,
            gate,
        })
    }

    async fn healthy(&self) -> TestResult {
        assert_eq!(self.session.connection_state(), ConnectionState::Connected);
        let output =
            tokio::time::timeout(Duration::from_secs(3), self.session.exec(HEALTH_COMMAND))
                .await??;
        assert_eq!(output.stdout, b"alive");
        assert_eq!(output.exit_status, Some(0));
        assert_eq!(self.observed.subsystems.load(Ordering::Acquire), 0);
        assert_eq!(self.observed.unexpected.load(Ordering::Acquire), 0);
        Ok(())
    }

    async fn finish(mut self) -> TestResult {
        self.gate.add_permits(8);
        if self.session.connection_state() == ConnectionState::Connected {
            self.session.close().await?;
        }
        let joined = tokio::time::timeout(Duration::from_secs(3), &mut self.task).await?;
        joined?;
        Ok(())
    }
}

async fn wait_count(counter: &AtomicUsize, minimum: usize) -> TestResult {
    tokio::time::timeout(Duration::from_secs(3), async {
        while counter.load(Ordering::Acquire) < minimum {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

fn assert_cancelled(result: keelshell_session::Result<ArchiveToolReport>) {
    assert!(
        matches!(result, Err(SessionError::Closed)),
        "cancelled discovery must not adopt a report: {result:?}"
    );
}

#[tokio::test]
async fn reviewed_preview_is_sent_verbatim_once_without_sftp_or_tool_execution() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response {
        chunk_size: 1,
        ..Response::default()
    }))
    .await?;
    assert_eq!(fixture.observed.opened.load(Ordering::Acquire), 0);
    let same_connection = fixture.session.clone();
    let report = fixture
        .session
        .inspect_archive_tools(&AtomicBool::new(false))
        .await?;
    assert_eq!(report, found_report());
    assert!(fixture.session.same_connection(&same_connection));
    assert_eq!(
        *fixture
            .observed
            .commands
            .lock()
            .map_err(|_| "command lock")?,
        vec![probe_command().into_bytes()]
    );
    assert_eq!(fixture.observed.opened.load(Ordering::Acquire), 1);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn missing_tools_are_successful_observations_instead_of_environment_errors() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response {
        stdout: body("missing", |_| String::new()),
        ..Response::default()
    }))
    .await?;
    let report = fixture
        .session
        .inspect_archive_tools(&AtomicBool::new(false))
        .await?;
    assert_eq!(
        report.tools,
        ArchiveTool::ALL
            .into_iter()
            .map(|tool| ArchiveToolObservation {
                tool,
                availability: ArchiveToolAvailability::Missing,
                path: None,
            })
            .collect::<Vec<_>>()
    );
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn shell_and_relative_resolutions_cannot_be_promoted_to_found_over_ssh() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response {
        stdout: body("unverified", |tool| match tool {
            ArchiveTool::Tar => "./bin/tar".into(),
            ArchiveTool::Gzip => "alias gzip='other gzip'".into(),
            ArchiveTool::Python3 => String::new(),
            tool => tool.name().to_owned(),
        }),
        ..Response::default()
    }))
    .await?;
    let report = fixture
        .session
        .inspect_archive_tools(&AtomicBool::new(false))
        .await?;
    assert!(
        report
            .tools
            .iter()
            .all(|tool| tool.availability == ArchiveToolAvailability::Unverified)
    );
    assert_eq!(report.tools[0].path.as_deref(), Some("./bin/tar"));
    assert_eq!(report.tools[4].path, None);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn each_explicit_request_uses_a_fresh_channel_and_no_cached_authority() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response::default())).await?;
    let cancelled = AtomicBool::new(false);
    for _ in 0..2 {
        assert_eq!(
            fixture.session.inspect_archive_tools(&cancelled).await?,
            found_report()
        );
    }
    assert_eq!(fixture.observed.probes.load(Ordering::Acquire), 2);
    assert_eq!(fixture.observed.opened.load(Ordering::Acquire), 2);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn cancellation_before_dispatch_opens_no_channel_and_preserves_connection() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response::default())).await?;
    assert_cancelled(
        fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(true))
            .await,
    );
    assert_eq!(fixture.observed.opened.load(Ordering::Acquire), 0);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn cancelled_late_open_is_reaped_without_sending_exec() -> TestResult {
    let fixture = Fixture::new(Mode::HoldOpen).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.inspect_archive_tools(&control).await });
    wait_count(&fixture.observed.opened, 1).await?;
    cancelled.store(true, Ordering::Release);
    assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    fixture.gate.add_permits(1);
    wait_count(&fixture.observed.closed, 1).await?;
    assert_eq!(fixture.observed.probes.load(Ordering::Acquire), 0);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn dropping_a_pending_probe_reaps_a_late_open_without_exec() -> TestResult {
    let fixture = Fixture::new(Mode::HoldOpen).await?;
    let session = fixture.session.clone();
    let job =
        tokio::spawn(async move { session.inspect_archive_tools(&AtomicBool::new(false)).await });
    wait_count(&fixture.observed.opened, 1).await?;
    job.abort();
    let joined = tokio::time::timeout(Duration::from_secs(2), job).await?;
    assert!(joined.as_ref().is_err_and(|error| error.is_cancelled()));
    fixture.gate.add_permits(1);
    wait_count(&fixture.observed.closed, 1).await?;
    assert_eq!(fixture.observed.probes.load(Ordering::Acquire), 0);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn cancellation_discards_a_late_valid_report_and_reaps_the_channel() -> TestResult {
    let fixture = Fixture::new(Mode::HoldResult).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.inspect_archive_tools(&control).await });
    wait_count(&fixture.observed.probes, 1).await?;
    cancelled.store(true, Ordering::Release);
    assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    wait_count(&fixture.observed.closed, 1).await?;
    fixture.gate.add_permits(1);
    wait_count(&fixture.observed.late_replies, 1).await?;
    assert_eq!(fixture.observed.probes.load(Ordering::Acquire), 1);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn cancellation_while_awaiting_exec_ack_closes_only_the_owned_channel() -> TestResult {
    let fixture = Fixture::new(Mode::HoldAck).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.inspect_archive_tools(&control).await });
    wait_count(&fixture.observed.probes, 1).await?;
    cancelled.store(true, Ordering::Release);
    assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    wait_count(&fixture.observed.closed, 1).await?;
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn held_exec_ack_expires_under_the_overall_ten_second_deadline() -> TestResult {
    let fixture = Fixture::new(Mode::HoldAck).await?;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false)),
    )
    .await?;
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "{result:?}"
    );
    wait_count(&fixture.observed.closed, 1).await?;
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn a_complete_zero_exit_frame_on_a_held_open_channel_cannot_be_adopted() -> TestResult {
    let fixture = Fixture::new(Mode::HoldClose).await?;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false)),
    )
    .await?;
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "{result:?}"
    );
    wait_count(&fixture.observed.closed, 1).await?;
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn unconfirmed_channel_open_expires_without_exec_or_a_successful_report() -> TestResult {
    let fixture = Fixture::new(Mode::HoldOpen).await?;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false)),
    )
    .await?;
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "{result:?}"
    );
    assert_eq!(fixture.observed.probes.load(Ordering::Acquire), 0);
    // Exceptional unconfirmed-open cleanup may stop the shared transport.
    fixture.gate.add_permits(1);
    fixture.finish().await
}

#[tokio::test]
async fn unsupported_shell_and_rejected_exec_are_typed_and_preserve_connection() -> TestResult {
    for mode in [
        Mode::Reply(Response {
            exit_status: Some(65),
            ..Response::default()
        }),
        Mode::Reject,
    ] {
        let fixture = Fixture::new(mode).await?;
        let result = fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false))
            .await;
        assert!(
            matches!(result, Err(SessionError::Unsupported(_))),
            "{result:?}"
        );
        fixture.healthy().await?;
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn malformed_stderr_and_absent_exit_cannot_become_tool_observations() -> TestResult {
    for (response, unsupported) in [
        (
            Response {
                stdout: b"login banner\n".to_vec(),
                ..Response::default()
            },
            false,
        ),
        (
            Response {
                stderr: b"unsafe peer diagnostics".to_vec(),
                ..Response::default()
            },
            false,
        ),
        (
            Response {
                exit_status: None,
                ..Response::default()
            },
            true,
        ),
    ] {
        let fixture = Fixture::new(Mode::Reply(response)).await?;
        let result = fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false))
            .await;
        assert!(
            matches!(
                (&result, unsupported),
                (Err(SessionError::Unsupported(_)), true) | (Err(SessionError::Invalid(_)), false)
            ),
            "{result:?}"
        );
        fixture.healthy().await?;
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn combined_stdout_stderr_flood_is_bounded_and_does_not_kill_the_connection() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response {
        stdout: vec![b'x'; 24 * 1024],
        stderr: vec![b'e'; 9 * 1024],
        ..Response::default()
    }))
    .await?;
    let result = fixture
        .session
        .inspect_archive_tools(&AtomicBool::new(false))
        .await;
    assert!(
        matches!(result, Err(SessionError::OutputLimit(limit)) if limit <= 64 * 1024),
        "{result:?}"
    );
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn an_already_closing_connection_cannot_open_a_discovery_channel() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response::default())).await?;
    fixture.session.close().await?;
    assert_cancelled(
        fixture
            .session
            .inspect_archive_tools(&AtomicBool::new(false))
            .await,
    );
    assert_eq!(fixture.observed.opened.load(Ordering::Acquire), 0);
    fixture.finish().await
}
