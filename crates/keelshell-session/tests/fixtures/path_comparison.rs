#![cfg(test)]
//! Actual encrypted connections, recursive route identity and hostile sample peers.

use super::{jump_fixture, upstream_proxy::proxy_fixture};
use keelshell_session::{
    ProxyKind, SshAuth, SshOptions, SshSession,
    path_comparison::{
        PathComparisonError, PathComparisonPlan, PathComparisonStatus, PathProbeFailure,
        PathProbeKind, PathSampleOutcome, PathSide,
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
    net::{Shutdown, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(3);
const PAYLOAD: usize = 262144;
const HEALTH: &[u8] = b"path-fixture-health";

#[derive(Clone, Copy, Debug)]
enum Fault {
    Wrong,
    Truncated,
    Extra,
    Stderr,
    NoAck,
    NoEof,
    NoExit,
    BadExit,
    DuplicateExit,
    AfterEof,
    Flood,
    NoClose,
    Reject,
}

#[derive(Clone, Copy)]
enum Mode {
    Valid,
    Fault(Fault, PathProbeKind),
    HoldOpen,
    HoldAck,
    HoldReply,
}

#[derive(Default)]
struct Observed {
    opens: AtomicUsize,
    execs: AtomicUsize,
    closes: AtomicUsize,
    input_eof: AtomicUsize,
    late: AtomicUsize,
    commands: Mutex<Vec<(usize, PathProbeKind, Vec<u8>)>>,
}

struct ProbeChannel {
    _channel: Channel<server::Msg>,
    kind: Option<PathProbeKind>,
    input_bytes: usize,
    valid_input: bool,
}

struct Peer {
    connection: usize,
    mode: Mode,
    observed: Arc<Observed>,
    gate: Arc<Semaphore>,
    channels: HashMap<ChannelId, ProbeChannel>,
    jobs: JoinSet<()>,
}

impl Peer {
    fn fault(&self, kind: PathProbeKind) -> Option<Fault> {
        match self.mode {
            Mode::Fault(fault, selected) if selected == kind => Some(fault),
            _ => None,
        }
    }
}

fn expected(kind: PathProbeKind) -> Vec<u8> {
    match kind {
        PathProbeKind::Exec => b"KEELSHELL_PATH_EXEC_V1\n".to_vec(),
        PathProbeKind::Upload => b"KEELSHELL_PATH_UPLOAD_262144_V1\n".to_vec(),
        PathProbeKind::Download => vec![b'K'; PAYLOAD],
    }
}

fn reply(
    session: &mut server::Session,
    id: ChannelId,
    kind: PathProbeKind,
    fault: Option<Fault>,
) -> Result<(), russh::Error> {
    let mut output = expected(kind);
    match fault {
        Some(Fault::Wrong) => output[0] = b'!',
        Some(Fault::Truncated) => {
            output.pop();
        }
        Some(Fault::Extra) => output.push(b'!'),
        _ => {}
    }
    for chunk in output.chunks(1024) {
        session.data(id, chunk.to_vec())?;
    }
    if matches!(fault, Some(Fault::Stderr)) {
        session.extended_data(id, 1, b"UNTRUSTED_PEER_TEXT".to_vec())?;
    }
    if !matches!(fault, Some(Fault::NoEof)) {
        session.eof(id)?;
    }
    if matches!(fault, Some(Fault::AfterEof)) {
        session.data(id, b"!".to_vec())?;
    }
    if !matches!(fault, Some(Fault::NoExit)) {
        session.exit_status_request(id, u32::from(matches!(fault, Some(Fault::BadExit))))?;
    }
    if matches!(fault, Some(Fault::DuplicateExit)) {
        session.exit_status_request(id, 0)?;
    }
    if !matches!(fault, Some(Fault::NoClose)) {
        session.close(id)?;
    }
    Ok(())
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        _: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if password == "test-only" {
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
        self.channels.insert(
            channel.id(),
            ProbeChannel {
                _channel: channel,
                kind: None,
                input_bytes: 0,
                valid_input: true,
            },
        );
        let index = self.observed.opens.fetch_add(1, Ordering::AcqRel);
        if matches!(self.mode, Mode::HoldOpen) && index == 0 {
            let gate = self.gate.clone();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(12), gate.acquire()).await
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
        if command == HEALTH {
            session.channel_success(id)?;
            session.data(id, b"alive".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.eof(id)?;
            session.close(id)?;
            self.channels.remove(&id);
            return Ok(());
        }
        let text = std::str::from_utf8(command).map_err(|_| russh::Error::Disconnect)?;
        let kind = if text.contains("# KEELSHELL_PATH_PROBE exec\n") {
            PathProbeKind::Exec
        } else if text.contains("# KEELSHELL_PATH_PROBE upload\n") {
            PathProbeKind::Upload
        } else if text.contains("# KEELSHELL_PATH_PROBE download\n") {
            PathProbeKind::Download
        } else {
            return Err(russh::Error::Disconnect);
        };
        self.channels
            .get_mut(&id)
            .ok_or(russh::Error::Disconnect)?
            .kind = Some(kind);
        self.observed
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push((self.connection, kind, command.to_vec()));
        self.observed.execs.fetch_add(1, Ordering::Release);
        let fault = self.fault(kind);
        if matches!(fault, Some(Fault::Reject)) {
            session.channel_failure(id)?;
        } else if matches!(self.mode, Mode::HoldAck) {
            // The peer intentionally retains the channel without acknowledging.
        } else if matches!(fault, Some(Fault::NoAck)) {
            reply(session, id, kind, fault)?;
            self.channels.remove(&id);
        } else {
            session.channel_success(id)?;
        }
        Ok(())
    }

    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let flood = matches!(self.mode, Mode::Fault(Fault::Flood, PathProbeKind::Upload));
        if let Some(channel) = self.channels.get_mut(&id) {
            if flood && channel.input_bytes == 0 {
                // More than the client receive window, emitted before accepting
                // all stdin. Serial write-then-read implementations deadlock.
                for _ in 0..768 {
                    session.data(id, vec![b'!'; 4096])?;
                }
            }
            channel.input_bytes = channel.input_bytes.saturating_add(data.len());
            channel.valid_input &= data.iter().all(|byte| *byte == b'K')
                && channel.input_bytes <= PAYLOAD
                && channel.kind == Some(PathProbeKind::Upload);
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        id: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let Some(channel) = self.channels.get(&id) else {
            return Ok(());
        };
        let Some(kind) = channel.kind else {
            return Ok(());
        };
        self.observed.input_eof.fetch_add(1, Ordering::Release);
        if !channel.valid_input
            || channel.input_bytes
                != if kind == PathProbeKind::Upload {
                    PAYLOAD
                } else {
                    0
                }
        {
            session.exit_status_request(id, 65)?;
            session.eof(id)?;
            session.close(id)?;
            return Ok(());
        }
        if matches!(self.mode, Mode::HoldReply) {
            let gate = self.gate.clone();
            let observed = self.observed.clone();
            let handle = session.handle();
            self.jobs.spawn(async move {
                if let Ok(Ok(permit)) =
                    tokio::time::timeout(Duration::from_secs(12), gate.acquire()).await
                {
                    permit.forget();
                    observed.late.fetch_add(1, Ordering::Release);
                    let _ = handle.data(id, expected(kind)).await;
                    let _ = handle.eof(id).await;
                    let _ = handle.exit_status_request(id, 0).await;
                    let _ = handle.close(id).await;
                }
            });
            return Ok(());
        }
        let fault = self.fault(kind);
        reply(session, id, kind, fault)?;
        if !matches!(fault, Some(Fault::NoClose)) {
            self.channels.remove(&id);
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.observed.closes.fetch_add(1, Ordering::Release);
        Ok(())
    }
}

struct Node {
    address: SocketAddr,
    fingerprint: String,
    observed: Arc<Observed>,
    gate: Arc<Semaphore>,
    task: JoinHandle<()>,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct SocketOwner(std::net::TcpStream);
impl Drop for SocketOwner {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

impl Node {
    async fn start(mode: Mode, seed: u8) -> TestResult<Self> {
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let observed = Arc::new(Observed::default());
        let gate = Arc::new(Semaphore::new(0));
        let config = Arc::new(server::Config {
            keys: vec![key],
            window_size: 16 * 1024,
            maximum_packet_size: 4096,
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let shared = observed.clone();
        let permits = gate.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            let mut connection = 0;
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break; };
                        connection += 1;
                        let peer = Peer {
                            connection,
                            mode,
                            observed: shared.clone(),
                            gate: permits.clone(),
                            channels: HashMap::new(),
                            jobs: JoinSet::new(),
                        };
                        let config = config.clone();
                        clients.spawn(async move {
                            let Ok(socket) = socket.into_std() else { return; };
                            let Ok(duplicate) = socket.try_clone() else { return; };
                            let _owner = SocketOwner(duplicate);
                            let Ok(socket) = TcpStream::from_std(socket) else { return; };
                            let _ = socket.set_nodelay(true);
                            if let Ok(running) = server::run_stream(config, socket, peer).await {
                                let _ = running.await;
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
        });
        Ok(Self {
            address,
            fingerprint,
            observed,
            gate,
            task,
        })
    }

    fn options(&self) -> SshOptions {
        let mut options = SshOptions::new(self.address.ip().to_string(), "fixture");
        options.port = self.address.port();
        options.expected_host_key = Some(self.fingerprint.clone());
        options.auth = SshAuth::Password(Zeroizing::new("test-only".into()));
        options.timeout = TIMEOUT;
        options
    }
}

struct Pair {
    node: Node,
    _jump_server: jump_fixture::Server,
    jump: SshSession,
    first: SshSession,
    second: SshSession,
    sibling: SshSession,
    plan: PathComparisonPlan,
}

impl Pair {
    async fn new(mode: Mode) -> TestResult<Self> {
        Self::new_path(mode, false).await
    }

    async fn new_path(mode: Mode, relay_first: bool) -> TestResult<Self> {
        let node = Node::start(mode, 0x61).await?;
        let jump_server = jump_fixture::Server::start().await?;
        let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
        let first = SshSession::connect(node.options()).await?;
        let second = SshSession::connect_through(&jump, node.options()).await?;
        let sibling = SshSession::connect_through(&jump, node.options()).await?;
        let plan = if relay_first {
            PathComparisonPlan::new(second.clone(), first.clone())?
        } else {
            PathComparisonPlan::new(first.clone(), second.clone())?
        };
        assert_eq!(
            plan.connection_info(PathSide::A).jump().is_some(),
            relay_first
        );
        Ok(Self {
            node,
            _jump_server: jump_server,
            jump,
            first,
            second,
            sibling,
            plan,
        })
    }

    async fn healthy(&self) -> TestResult {
        for session in [&self.first, &self.second, &self.sibling] {
            let output =
                tokio::time::timeout(TIMEOUT, session.exec("path-fixture-health")).await??;
            assert_eq!(output.stdout, b"alive");
            assert_eq!(output.exit_status, Some(0));
            assert!(!session.is_closed());
        }
        assert_eq!(
            self.jump.exec("parent lives").await?.stdout,
            b"parent lives"
        );
        Ok(())
    }

    async fn finish(mut self) -> TestResult {
        self.node.gate.add_permits(8);
        self.first.close().await?;
        self.second.close().await?;
        self.sibling.close().await?;
        self.jump.close().await?;
        self.node.task.abort();
        let result = tokio::time::timeout(TIMEOUT, &mut self.node.task).await?;
        assert!(result.is_err_and(|error| error.is_cancelled()));
        Ok(())
    }
}

async fn wait_count(counter: &AtomicUsize, minimum: usize) -> TestResult {
    jump_fixture::wait_until(|| counter.load(Ordering::Acquire) >= minimum).await
}

#[tokio::test]
async fn comparison_seals_actual_recursive_hops_and_each_explicit_proxy_without_credentials()
-> TestResult {
    let root_server = jump_fixture::Server::start().await?;
    let middle_server = jump_fixture::Server::start().await?;
    let target = Node::start(Mode::Valid, 0x62).await?;
    let root_proxy = proxy_fixture::ProxyServer::start(
        ProxyKind::HttpConnect,
        root_server.address,
        true,
        proxy_fixture::Mode::Tunnel,
    )
    .await?;
    let middle_proxy = proxy_fixture::ProxyServer::start(
        ProxyKind::Socks5,
        middle_server.address,
        true,
        proxy_fixture::Mode::Tunnel,
    )
    .await?;
    let mut root_options = root_server.options(TIMEOUT);
    root_options.proxy = Some(root_proxy.options(ProxyKind::HttpConnect, true));
    let root = SshSession::connect(root_options).await?;
    let mut middle_options = middle_server.options(TIMEOUT);
    middle_options.proxy = Some(middle_proxy.options(ProxyKind::Socks5, true));
    let middle = SshSession::connect_through(&root, middle_options).await?;
    let child = SshSession::connect_through(&middle, target.options()).await?;
    let direct = SshSession::connect(target.options()).await?;
    let info = child.connection_info();
    assert_eq!(info.host(), "127.0.0.1");
    assert_eq!(info.port(), target.address.port());
    assert_eq!(info.username(), "fixture");
    assert_eq!(info.verified_host_key(), target.fingerprint);
    assert!(info.proxy().is_none());
    let actual_middle = info.jump().ok_or("missing actual middle")?;
    assert_eq!(actual_middle, middle.connection_info());
    assert_eq!(
        actual_middle.proxy().map(|proxy| proxy.kind()),
        Some(ProxyKind::Socks5)
    );
    let actual_root = actual_middle.jump().ok_or("missing actual root")?;
    assert_eq!(actual_root, root.connection_info());
    assert!(actual_root.jump().is_none());
    assert_eq!(
        actual_root
            .proxy()
            .map(|proxy| (proxy.kind(), proxy.host(), proxy.port())),
        Some((
            ProxyKind::HttpConnect,
            "127.0.0.1",
            root_proxy.address.port()
        ))
    );
    let plan = PathComparisonPlan::new(direct.clone(), child.clone())?;
    assert!(plan.matches_sessions(&direct.clone(), &child.clone()));
    assert!(!plan.matches_sessions(&child, &direct));
    let review = plan.review_text();
    for pin in [
        &root_server.fingerprint,
        &middle_server.fingerprint,
        &target.fingerprint,
    ] {
        assert!(review.contains(pin));
    }
    for text in [review, format!("{info:?}")] {
        for secret in ["代理-user", "pa:ss-秘密", "test-only"] {
            assert!(!text.contains(secret));
        }
    }
    child.close().await?;
    direct.close().await?;
    middle.close().await?;
    root.close().await?;
    root_proxy.assert_healthy();
    middle_proxy.assert_healthy();
    Ok(())
}

#[tokio::test]
async fn comparison_eligibility_rejects_clones_same_route_and_every_endpoint_dimension()
-> TestResult {
    let pair = Pair::new(Mode::Valid).await?;
    assert!(matches!(
        PathComparisonPlan::new(pair.first.clone(), pair.first.clone()),
        Err(PathComparisonError::SameConnection)
    ));
    let same_route = SshSession::connect(pair.node.options()).await?;
    assert!(matches!(
        PathComparisonPlan::new(pair.first.clone(), same_route.clone()),
        Err(PathComparisonError::SameRoute)
    ));
    let mut changed_host = pair.node.options();
    changed_host.host = "localhost".into();
    let host = SshSession::connect_through(&pair.jump, changed_host).await?;
    let mut changed_user = pair.node.options();
    changed_user.username = "another-fixture-user".into();
    let user = SshSession::connect_through(&pair.jump, changed_user).await?;
    let other_port = Node::start(Mode::Valid, 0x61).await?;
    let port = SshSession::connect_through(&pair.jump, other_port.options()).await?;
    let other_key = Node::start(Mode::Valid, 0x63).await?;
    let proxy = proxy_fixture::ProxyServer::start(
        ProxyKind::HttpConnect,
        other_key.address,
        false,
        proxy_fixture::Mode::Tunnel,
    )
    .await?;
    let mut changed_key = pair.node.options();
    changed_key.expected_host_key = Some(other_key.fingerprint.clone());
    changed_key.proxy = Some(proxy.options(ProxyKind::HttpConnect, false));
    let key = SshSession::connect_through(&pair.jump, changed_key).await?;
    assert_eq!(
        key.connection_info().host(),
        pair.first.connection_info().host()
    );
    assert_eq!(
        key.connection_info().port(),
        pair.first.connection_info().port()
    );
    for changed in [&host, &user, &port, &key] {
        assert!(matches!(
            PathComparisonPlan::new(pair.first.clone(), changed.clone()),
            Err(PathComparisonError::DifferentEndpoint)
        ));
        changed.close().await?;
    }
    // An explicit proxy alone differentiates routes even without an SSH jump.
    let proxy = proxy_fixture::ProxyServer::start(
        ProxyKind::Socks5,
        pair.node.address,
        true,
        proxy_fixture::Mode::Tunnel,
    )
    .await?;
    let mut proxied_options = pair.node.options();
    proxied_options.proxy = Some(proxy.options(ProxyKind::Socks5, true));
    let proxied = SshSession::connect(proxied_options).await?;
    assert!(proxied.connection_info().jump().is_none());
    assert!(PathComparisonPlan::new(pair.first.clone(), proxied.clone()).is_ok());
    proxied.close().await?;
    assert!(matches!(
        PathComparisonPlan::new(pair.first.clone(), proxied),
        Err(PathComparisonError::ConnectionClosed)
    ));
    same_route.close().await?;
    pair.healthy().await?;
    pair.finish().await
}

#[tokio::test]
async fn comparison_strict_samples_follow_alternating_rounds_and_reviewed_commands() -> TestResult {
    let pair = Pair::new(Mode::Valid).await?;
    let review = pair.plan.review_text();
    let report = tokio::time::timeout(
        Duration::from_secs(25),
        pair.plan.run(&AtomicBool::new(false)),
    )
    .await?;
    assert_eq!(report.status, PathComparisonStatus::Complete, "{report:?}");
    assert!(!report.cleanup_unconfirmed);
    assert_eq!(report.samples.len(), 18);
    assert_eq!(
        report.samples.iter().map(|row| row.bytes).sum::<usize>(),
        3 * 1024 * 1024
    );
    let mut expected_order = Vec::new();
    for round in 1..=3 {
        let sides = if round == 2 {
            [PathSide::B, PathSide::A]
        } else {
            [PathSide::A, PathSide::B]
        };
        for kind in [
            PathProbeKind::Exec,
            PathProbeKind::Upload,
            PathProbeKind::Download,
        ] {
            for side in sides {
                expected_order.push((round, side, kind));
            }
        }
    }
    assert_eq!(
        report
            .samples
            .iter()
            .map(|row| (row.round, row.path, row.kind))
            .collect::<Vec<_>>(),
        expected_order
    );
    let commands = pair
        .node
        .observed
        .commands
        .lock()
        .map_err(|_| "commands lock")?
        .clone();
    assert_eq!(commands.len(), 18);
    for ((_, side, kind), (connection, observed_kind, command)) in
        expected_order.iter().zip(commands)
    {
        assert_eq!(connection, if *side == PathSide::A { 1 } else { 2 });
        assert_eq!(*kind, observed_kind);
        assert!(review.contains(std::str::from_utf8(&command)?));
    }
    for side in [PathSide::A, PathSide::B] {
        for kind in [
            PathProbeKind::Exec,
            PathProbeKind::Upload,
            PathProbeKind::Download,
        ] {
            assert!(report.summary(side, kind).is_some());
        }
    }
    pair.healthy().await?;
    pair.finish().await
}

#[tokio::test]
async fn comparison_malformed_ack_output_eof_exit_and_close_never_become_success() -> TestResult {
    for fault in [
        Fault::Wrong,
        Fault::Truncated,
        Fault::Extra,
        Fault::Stderr,
        Fault::NoAck,
        Fault::NoEof,
        Fault::NoExit,
        Fault::BadExit,
        Fault::DuplicateExit,
        Fault::AfterEof,
        Fault::Reject,
    ] {
        let pair = Pair::new(Mode::Fault(fault, PathProbeKind::Exec)).await?;
        let report = tokio::time::timeout(
            Duration::from_secs(12),
            pair.plan.run(&AtomicBool::new(false)),
        )
        .await?;
        assert_eq!(
            report.status,
            PathComparisonStatus::Incomplete,
            "{fault:?}: {report:?}"
        );
        assert_eq!(report.samples.len(), 1, "{fault:?}: {report:?}");
        assert!(matches!(
            report.samples[0].outcome,
            PathSampleOutcome::Failed(_)
        ));
        assert_eq!(report.summary(PathSide::A, PathProbeKind::Exec), None);
        assert!(!format!("{report:?}").contains("UNTRUSTED_PEER_TEXT"));
        pair.healthy().await?;
        pair.finish().await?;
    }
    for kind in [PathProbeKind::Upload, PathProbeKind::Download] {
        let pair = Pair::new(Mode::Fault(Fault::Wrong, kind)).await?;
        let report = pair.plan.run(&AtomicBool::new(false)).await;
        assert_eq!(report.status, PathComparisonStatus::Incomplete);
        assert_eq!(report.samples.last().map(|row| row.kind), Some(kind));
        assert!(matches!(
            report.samples.last().map(|row| &row.outcome),
            Some(PathSampleOutcome::Failed(PathProbeFailure::InvalidStdout))
        ));
        pair.finish().await?;
    }
    let pair = Pair::new(Mode::Fault(Fault::Flood, PathProbeKind::Upload)).await?;
    let report = tokio::time::timeout(
        Duration::from_secs(12),
        pair.plan.run(&AtomicBool::new(false)),
    )
    .await?;
    assert_eq!(report.status, PathComparisonStatus::Incomplete);
    assert_eq!(report.samples.len(), 3, "{report:?}");
    assert!(matches!(
        report.samples.last().map(|row| &row.outcome),
        Some(PathSampleOutcome::Failed(PathProbeFailure::OutputLimit))
    ));
    assert!(!report.cleanup_unconfirmed, "{report:?}");
    pair.healthy().await?;
    pair.finish().await?;
    Ok(())
}

#[tokio::test]
async fn comparison_precancelled_run_never_opens_channels() -> TestResult {
    let pair = Pair::new(Mode::Valid).await?;
    let report = pair.plan.run(&AtomicBool::new(true)).await;
    assert_eq!(report.status, PathComparisonStatus::Cancelled);
    assert!(report.samples.is_empty());
    assert!(!report.cleanup_unconfirmed);
    assert_eq!(pair.node.observed.opens.load(Ordering::Acquire), 0);
    pair.healthy().await?;
    pair.finish().await
}

#[tokio::test]
async fn comparison_cancelled_or_dropped_late_open_never_executes_and_preserves_jump_and_sibling()
-> TestResult {
    cancelled_or_dropped_late_open(false).await
}

#[tokio::test]
async fn comparison_relay_child_cancelled_or_dropped_late_open_preserves_shared_parent_and_sibling()
-> TestResult {
    cancelled_or_dropped_late_open(true).await
}

async fn cancelled_or_dropped_late_open(relay_first: bool) -> TestResult {
    for drop_future in [false, true] {
        let pair = Pair::new_path(Mode::HoldOpen, relay_first).await?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let plan = pair.plan.clone();
        let job = tokio::spawn(async move { plan.run(&flag).await });
        wait_count(&pair.node.observed.opens, 1).await?;
        if drop_future {
            job.abort();
            assert!(job.await.is_err_and(|error| error.is_cancelled()));
            pair.node.gate.add_permits(1);
        } else {
            cancelled.store(true, Ordering::Release);
            pair.node.gate.add_permits(1);
            let report = tokio::time::timeout(TIMEOUT, job).await??;
            assert_eq!(report.status, PathComparisonStatus::Cancelled);
            assert!(!report.cleanup_unconfirmed, "{report:?}");
        }
        wait_count(&pair.node.observed.closes, 1).await?;
        assert_eq!(pair.node.observed.execs.load(Ordering::Acquire), 0);
        pair.healthy().await?;
        pair.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn comparison_cancelled_dispatched_probe_keeps_unknown_and_rejects_late_valid_result()
-> TestResult {
    cancelled_dispatched_probe(false).await
}

#[tokio::test]
async fn comparison_relay_child_dispatched_cancel_retains_unknown_and_preserves_shared_sibling()
-> TestResult {
    cancelled_dispatched_probe(true).await
}

async fn cancelled_dispatched_probe(relay_first: bool) -> TestResult {
    for mode in [Mode::HoldAck, Mode::HoldReply] {
        let pair = Pair::new_path(mode, relay_first).await?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let plan = pair.plan.clone();
        let job = tokio::spawn(async move { plan.run(&flag).await });
        wait_count(&pair.node.observed.execs, 1).await?;
        if matches!(mode, Mode::HoldReply) {
            wait_count(&pair.node.observed.input_eof, 1).await?;
        }
        cancelled.store(true, Ordering::Release);
        let report = tokio::time::timeout(TIMEOUT, job).await??;
        assert_eq!(report.status, PathComparisonStatus::Cancelled);
        assert!(report.cleanup_unconfirmed, "{report:?}");
        assert_eq!(report.samples.len(), 1);
        assert_eq!(
            report.samples[0].outcome,
            PathSampleOutcome::Failed(PathProbeFailure::Cancelled)
        );
        wait_count(&pair.node.observed.closes, 1).await?;
        pair.node.gate.add_permits(1);
        if matches!(mode, Mode::HoldReply) {
            wait_count(&pair.node.observed.late, 1).await?;
        }
        assert_eq!(report.summary(PathSide::A, PathProbeKind::Exec), None);
        pair.healthy().await?;
        pair.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn comparison_missing_close_hits_sample_deadline_and_retains_unconfirmed_cleanup()
-> TestResult {
    missing_close_deadline(false).await
}

#[tokio::test]
async fn comparison_relay_child_missing_close_deadline_retains_unknown_and_shared_sibling()
-> TestResult {
    missing_close_deadline(true).await
}

async fn missing_close_deadline(relay_first: bool) -> TestResult {
    let pair = Pair::new_path(
        Mode::Fault(Fault::NoClose, PathProbeKind::Exec),
        relay_first,
    )
    .await?;
    let report = tokio::time::timeout(
        Duration::from_secs(12),
        pair.plan.run(&AtomicBool::new(false)),
    )
    .await?;
    assert_eq!(report.status, PathComparisonStatus::Incomplete);
    assert_eq!(report.samples.len(), 1);
    assert_eq!(
        report.samples[0].outcome,
        PathSampleOutcome::Failed(PathProbeFailure::SampleDeadline)
    );
    assert!(report.cleanup_unconfirmed);
    wait_count(&pair.node.observed.closes, 1).await?;
    pair.healthy().await?;
    pair.finish().await
}
