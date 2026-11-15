#![cfg(test)]
//! Encrypted SSH cancellation/protocol regressions; the peer never runs a real shell.

use keelshell_session::{
    ConnectionState, SessionError, SshAuth, SshOptions, SshSession,
    archive_tools::ArchiveTool,
    archive_validation::{ArchiveValidationReport, ToolCheckStatus, validation_command},
};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use serde_json::json;
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
const HEALTH: &str = "archive-validation-fixture-health";

fn wire() -> TestResult<Vec<u8>> {
    Ok(serde_json::to_vec(&json!({
        "protocol": "keelshell.archive-validation.v1",
        "tools": ArchiveTool::ALL.map(|tool| json!({
            "tool": tool.name(),
            "path": format!("/fixture/归档 tools/{}", tool.name()),
            "version": format!("{} observed version 1", tool.name()),
            "version_status": "passed",
            "stream_check": if matches!(tool, ArchiveTool::Tar | ArchiveTool::Gzip | ArchiveTool::Python3) {
                "passed"
            } else { "not_tested" },
        })),
    }))?)
}

#[derive(Clone)]
struct Response {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit: Option<u32>,
    chunks: usize,
}

impl Response {
    fn valid() -> TestResult<Self> {
        Ok(Self {
            stdout: wire()?,
            stderr: Vec::new(),
            exit: Some(0),
            chunks: 4096,
        })
    }
}

#[derive(Clone)]
enum Mode {
    Reply(Response),
    HoldOpen,
    HoldAck,
    HoldReply,
    HoldClose,
    Reject,
}

#[derive(Default)]
struct Observed {
    opens: AtomicUsize,
    execs: AtomicUsize,
    closes: AtomicUsize,
    late: AtomicUsize,
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
        let index = self.observed.opens.fetch_add(1, Ordering::AcqRel);
        if matches!(self.mode, Mode::HoldOpen) && index == 0 {
            let gate = self.gate.clone();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(14), gate.acquire()).await
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
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if command == HEALTH.as_bytes() {
            session.channel_success(id)?;
            session.data(id, b"alive".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.close(id)?;
            self.channels.remove(&id);
            return Ok(());
        }
        if command != validation_command().as_bytes() {
            self.observed.unexpected.fetch_add(1, Ordering::AcqRel);
            session.channel_failure(id)?;
            return Ok(());
        }
        self.observed
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(command.to_vec());
        self.observed.execs.fetch_add(1, Ordering::AcqRel);
        if matches!(self.mode, Mode::Reject) {
            session.channel_failure(id)?;
            return Ok(());
        }
        if matches!(self.mode, Mode::HoldAck) {
            return Ok(());
        }
        session.channel_success(id)?;
        if matches!(self.mode, Mode::HoldReply) {
            let handle = session.handle();
            let gate = self.gate.clone();
            let observed = self.observed.clone();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(14), gate.acquire()).await
                {
                    permit.forget();
                    observed.late.fetch_add(1, Ordering::AcqRel);
                    if let Ok(data) = wire() {
                        let _ = handle.data(id, data).await;
                    }
                    let _ = handle.exit_status_request(id, 0).await;
                    let _ = handle.close(id).await;
                }
            });
            return Ok(());
        }
        let response = match &self.mode {
            Mode::Reply(response) => response.clone(),
            _ => Response::valid().map_err(|_| russh::Error::Disconnect)?,
        };
        for chunk in response.stdout.chunks(response.chunks) {
            session.data(id, chunk.to_vec())?;
        }
        for chunk in response.stderr.chunks(response.chunks) {
            session.extended_data(id, 1, chunk.to_vec())?;
        }
        if let Some(status) = response.exit {
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
        self.observed.unexpected.fetch_add(1, Ordering::AcqRel);
        session.channel_failure(id)?;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.observed.closes.fetch_add(1, Ordering::AcqRel);
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
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x56; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let observed = Arc::new(Observed::default());
        let gate = Arc::new(Semaphore::new(0));
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            ..Default::default()
        });
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
        let out = tokio::time::timeout(Duration::from_secs(3), self.session.exec(HEALTH)).await??;
        assert_eq!((out.stdout, out.exit_status), (b"alive".to_vec(), Some(0)));
        assert_eq!(self.observed.unexpected.load(Ordering::Acquire), 0);
        Ok(())
    }

    async fn finish(mut self) -> TestResult {
        self.gate.add_permits(8);
        if self.session.connection_state() == ConnectionState::Connected {
            self.session.close().await?;
        }
        tokio::time::timeout(Duration::from_secs(3), &mut self.task).await??;
        Ok(())
    }
}

async fn wait_count(counter: &AtomicUsize, value: usize) -> TestResult {
    tokio::time::timeout(Duration::from_secs(3), async {
        while counter.load(Ordering::Acquire) < value {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

fn assert_cancelled(result: keelshell_session::Result<ArchiveValidationReport>) {
    assert!(matches!(result, Err(SessionError::Closed)), "{result:?}");
}

#[tokio::test]
async fn complete_reviewed_helper_is_sent_verbatim_on_only_the_selected_connection() -> TestResult {
    let selected = Fixture::new(Mode::Reply(Response {
        chunks: 1,
        ..Response::valid()?
    }))
    .await?;
    let other = Fixture::new(Mode::Reply(Response::valid()?)).await?;
    let cloned = selected.session.clone();
    let report = selected
        .session
        .validate_archive_tools(&AtomicBool::new(false))
        .await?;
    assert!(selected.session.same_connection(&cloned));
    assert!(!selected.session.same_connection(&other.session));
    assert_eq!(report.tools.len(), 7);
    for (row, tool) in report.tools.iter().zip(ArchiveTool::ALL) {
        assert_eq!(row.tool, tool);
        assert_eq!(row.version_status, ToolCheckStatus::Passed);
        assert_eq!(
            row.stream_check,
            if matches!(
                tool,
                ArchiveTool::Tar | ArchiveTool::Gzip | ArchiveTool::Python3
            ) {
                ToolCheckStatus::Passed
            } else {
                ToolCheckStatus::NotTested
            }
        );
    }
    assert_eq!(
        *selected
            .observed
            .commands
            .lock()
            .map_err(|_| "command lock")?,
        vec![validation_command().into_bytes()]
    );
    assert_eq!(other.observed.opens.load(Ordering::Acquire), 0);
    selected.healthy().await?;
    other.healthy().await?;
    selected.finish().await?;
    other.finish().await
}

#[tokio::test]
async fn each_validation_request_uses_a_fresh_channel_and_no_cached_report() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response::valid()?)).await?;
    for _ in 0..2 {
        fixture
            .session
            .validate_archive_tools(&AtomicBool::new(false))
            .await?;
    }
    assert_eq!(fixture.observed.execs.load(Ordering::Acquire), 2);
    assert_eq!(fixture.observed.opens.load(Ordering::Acquire), 2);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn pre_cancelled_or_closing_validation_never_opens_a_channel() -> TestResult {
    let fixture = Fixture::new(Mode::Reply(Response::valid()?)).await?;
    assert_cancelled(
        fixture
            .session
            .validate_archive_tools(&AtomicBool::new(true))
            .await,
    );
    assert_eq!(fixture.observed.opens.load(Ordering::Acquire), 0);
    fixture.healthy().await?;
    let opens = fixture.observed.opens.load(Ordering::Acquire);
    fixture.session.close().await?;
    assert_cancelled(
        fixture
            .session
            .validate_archive_tools(&AtomicBool::new(false))
            .await,
    );
    assert_eq!(fixture.observed.opens.load(Ordering::Acquire), opens);
    fixture.finish().await
}

async fn retire_pending_open(abort: bool) -> TestResult {
    let fixture = Fixture::new(Mode::HoldOpen).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.validate_archive_tools(&control).await });
    wait_count(&fixture.observed.opens, 1).await?;
    if abort {
        job.abort();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), job)
                .await?
                .is_err_and(|e| e.is_cancelled())
        );
    } else {
        cancelled.store(true, Ordering::Release);
        assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    }
    fixture.gate.add_permits(1);
    wait_count(&fixture.observed.closes, 1).await?;
    assert_eq!(fixture.observed.execs.load(Ordering::Acquire), 0);
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn validation_cancellation_reaps_a_late_open_without_exec() -> TestResult {
    retire_pending_open(false).await
}

#[tokio::test]
async fn dropped_validation_future_reaps_a_late_open_without_exec() -> TestResult {
    retire_pending_open(true).await
}

#[tokio::test]
async fn cancelled_validation_discards_a_late_valid_reply_without_claiming_remote_stop()
-> TestResult {
    let fixture = Fixture::new(Mode::HoldReply).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.validate_archive_tools(&control).await });
    wait_count(&fixture.observed.execs, 1).await?;
    cancelled.store(true, Ordering::Release);
    assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    wait_count(&fixture.observed.closes, 1).await?;
    // This peer can still attempt a remote reply after local cancellation and CLOSE.
    fixture.gate.add_permits(1);
    wait_count(&fixture.observed.late, 1).await?;
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn validation_cancellation_while_waiting_for_exec_ack_preserves_the_connection() -> TestResult
{
    let fixture = Fixture::new(Mode::HoldAck).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let control = cancelled.clone();
    let session = fixture.session.clone();
    let job = tokio::spawn(async move { session.validate_archive_tools(&control).await });
    wait_count(&fixture.observed.execs, 1).await?;
    cancelled.store(true, Ordering::Release);
    assert_cancelled(tokio::time::timeout(Duration::from_secs(2), job).await??);
    wait_count(&fixture.observed.closes, 1).await?;
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn validation_deadline_includes_exec_ack_and_channel_close() -> TestResult {
    // These controlled peers share elapsed wall time, not an SSH connection or result.
    let ack = Fixture::new(Mode::HoldAck).await?;
    let close = Fixture::new(Mode::HoldClose).await?;
    let cancel_ack = AtomicBool::new(false);
    let cancel_close = AtomicBool::new(false);
    let (left, right) = tokio::join!(
        tokio::time::timeout(
            Duration::from_secs(12),
            ack.session.validate_archive_tools(&cancel_ack)
        ),
        tokio::time::timeout(
            Duration::from_secs(12),
            close.session.validate_archive_tools(&cancel_close)
        ),
    );
    assert!(matches!(left?, Err(SessionError::Timeout(_))));
    assert!(matches!(right?, Err(SessionError::Timeout(_))));
    wait_count(&ack.observed.closes, 1).await?;
    wait_count(&close.observed.closes, 1).await?;
    ack.healthy().await?;
    close.healthy().await?;
    ack.finish().await?;
    close.finish().await
}

#[tokio::test]
async fn validation_unconfirmed_open_times_out_without_exec_or_a_successful_report() -> TestResult {
    let fixture = Fixture::new(Mode::HoldOpen).await?;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        fixture
            .session
            .validate_archive_tools(&AtomicBool::new(false)),
    )
    .await?;
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "{result:?}"
    );
    assert_eq!(fixture.observed.execs.load(Ordering::Acquire), 0);
    // Exceptional unconfirmed-open cleanup can stop the shared transport.
    fixture.gate.add_permits(1);
    fixture.finish().await
}

#[tokio::test]
async fn malformed_validation_stderr_missing_exit_and_helper_failure_fail_closed() -> TestResult {
    for (response, unsupported) in [
        (
            Response {
                stdout: b"banner\n".to_vec(),
                ..Response::valid()?
            },
            false,
        ),
        (
            Response {
                stderr: b"untrusted error".to_vec(),
                ..Response::valid()?
            },
            false,
        ),
        (
            Response {
                exit: None,
                ..Response::valid()?
            },
            true,
        ),
        (
            Response {
                exit: Some(65),
                ..Response::valid()?
            },
            true,
        ),
    ] {
        let fixture = Fixture::new(Mode::Reply(response)).await?;
        let result = fixture
            .session
            .validate_archive_tools(&AtomicBool::new(false))
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
    let fixture = Fixture::new(Mode::Reject).await?;
    assert!(matches!(
        fixture
            .session
            .validate_archive_tools(&AtomicBool::new(false))
            .await,
        Err(SessionError::Unsupported(_))
    ));
    fixture.healthy().await?;
    fixture.finish().await
}

#[tokio::test]
async fn validation_combined_output_flood_is_bounded_without_closing_the_connection() -> TestResult
{
    let fixture = Fixture::new(Mode::Reply(Response {
        stdout: vec![b'x'; 24 * 1024],
        stderr: vec![b'e'; 9 * 1024],
        ..Response::valid()?
    }))
    .await?;
    let result = fixture
        .session
        .validate_archive_tools(&AtomicBool::new(false))
        .await;
    assert!(
        matches!(result, Err(SessionError::OutputLimit(limit)) if limit <= 32768),
        "{result:?}"
    );
    fixture.healthy().await?;
    fixture.finish().await
}
