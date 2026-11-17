//! Controlled SSH frames test routing/authority; captures are a separate opt-in.
//! Neither fixture replay nor a Windows runner is native desktop acceptance.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
    time::Duration,
};

use base64::Engine as _;
use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    monitor::{
        CapabilityReason, CapabilityState, CpuSample, LinuxMonitor, MonitorError, MonitorPlatform,
        MonitorRuntime, ProcessInfo, SocketInfo,
    },
};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use serde_json::json;
use tokio::{
    sync::{Semaphore, watch},
    task::JoinSet,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const CANCEL_BUDGET: Duration = Duration::from_secs(1);
const IO_BUDGET: Duration = Duration::from_secs(2);
const PLAIN_EXEC: &str = "fixture ordinary exec";
const PLAIN_STDOUT: &[u8] = b"ordinary stdout\0payload\r\n";
const PLAIN_STDERR: &[u8] = b"ordinary stderr\r\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Posix = 0,
    Probe = 1,
    Resources = 2,
    Processes = 3,
    Sockets = 4,
    PlainExec = 5,
}

struct HeldReply {
    channel: Mutex<Option<russh::ChannelId>>,
    started: Semaphore,
    release: Semaphore,
    finished: Semaphore,
}

impl HeldReply {
    fn new() -> Self {
        Self {
            channel: Mutex::new(None),
            started: Semaphore::new(0),
            release: Semaphore::new(0),
            finished: Semaphore::new(0),
        }
    }

    async fn wait_sent<F: Future>(&self, pending: Pin<&mut F>) -> TestResult<russh::ChannelId> {
        tokio::select! {
            _ = pending => Err("reader completed before its held exec was observed".into()),
            permit = tokio::time::timeout(IO_BUDGET, self.started.acquire()) => {
                permit??.forget();
                self.channel.lock().map_err(|_| "held channel lock")?
                    .ok_or_else(|| "missing held channel".into())
            }
        }
    }

    async fn release_and_wait(&self) -> TestResult {
        self.release.add_permits(1);
        tokio::time::timeout(IO_BUDGET, self.finished.acquire())
            .await??
            .forget();
        Ok(())
    }
}

struct Control {
    counts: [AtomicUsize; 6],
    commands: Mutex<Vec<String>>,
    unconfirmed: AtomicBool,
    missing_processes: AtomicBool,
    denied_resources: AtomicBool,
    opened: AtomicUsize,
    hold_open_at: AtomicUsize,
    open_started: Semaphore,
    open_release: Semaphore,
    open_channel: Mutex<Option<russh::ChannelId>>,
    held_replies: Mutex<Vec<(Operation, Arc<HeldReply>)>>,
    closed: watch::Sender<HashSet<russh::ChannelId>>,
    jobs: Mutex<JoinSet<TestResult>>,
    socket: Mutex<Option<std::net::TcpStream>>,
    captures: Option<[Vec<u8>; 4]>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            counts: std::array::from_fn(|_| AtomicUsize::new(0)),
            commands: Mutex::new(Vec::new()),
            unconfirmed: AtomicBool::new(false),
            missing_processes: AtomicBool::new(false),
            denied_resources: AtomicBool::new(false),
            opened: AtomicUsize::new(0),
            hold_open_at: AtomicUsize::new(0),
            open_started: Semaphore::new(0),
            open_release: Semaphore::new(0),
            open_channel: Mutex::new(None),
            held_replies: Mutex::new(Vec::new()),
            closed: watch::channel(HashSet::new()).0,
            jobs: Mutex::new(JoinSet::new()),
            socket: Mutex::new(None),
            captures: None,
        }
    }
}

impl Control {
    fn count(&self, operation: Operation) -> usize {
        self.counts[operation as usize].load(Ordering::Acquire)
    }

    fn hold_reply(&self, operation: Operation) -> TestResult<Arc<HeldReply>> {
        let held = Arc::new(HeldReply::new());
        self.held_replies
            .lock()
            .map_err(|_| "held replies lock")?
            .push((operation, held.clone()));
        Ok(held)
    }

    fn spawn(&self, job: impl Future<Output = TestResult> + Send + 'static) -> TestResult {
        self.jobs
            .lock()
            .map_err(|_| "fixture jobs lock")?
            .spawn(job);
        Ok(())
    }

    async fn wait_open<F: Future>(&self, pending: Pin<&mut F>) -> TestResult<russh::ChannelId> {
        tokio::select! {
            _ = pending => Err("reader completed before its held open was observed".into()),
            permit = tokio::time::timeout(IO_BUDGET, self.open_started.acquire()) => {
                permit??.forget();
                self.open_channel.lock().map_err(|_| "open channel lock")?
                    .ok_or_else(|| "missing held open channel".into())
            }
        }
    }

    async fn wait_closed(&self, channel: russh::ChannelId) -> TestResult {
        let mut closed = self.closed.subscribe();
        tokio::time::timeout(CANCEL_BUDGET, closed.wait_for(|ids| ids.contains(&channel)))
            .await??;
        Ok(())
    }

    fn stop(&self) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.abort_all();
        }
        if let Ok(socket) = self.socket.lock()
            && let Some(socket) = socket.as_ref()
        {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}

fn frame(operation: Operation, failure: Option<&str>) -> Vec<u8> {
    let (name, data) = match operation {
        Operation::Probe => ("probe", json!({"os":"Win32NT"})),
        Operation::Resources => (
            "resources",
            json!({
                "boot_time":"1700000000","uptime_seconds":100.0,"cpu_percent":25.0,
                "memory_total_bytes":"1048576","memory_free_bytes":"262144",
                "network_count":1,"networks":[
                    {"name":"Ethernet 测试","received_bytes":"1234","transmitted_bytes":"5678"}
                ],
                "capacity_status":"ok","filesystem_count":1,"filesystems":[
                    {"source":"C:\\","mount":"C:\\","total_bytes":"1000",
                     "total_free_bytes":"400","available_bytes":"300"}
                ]
            }),
        ),
        Operation::Processes => (
            "processes",
            json!({"count":2,"processes":[
                {"pid":"0","name":"Idle","working_set_bytes":null},
                {"pid":"42","name":"测试 worker","working_set_bytes":"4096"}
            ]}),
        ),
        Operation::Sockets => (
            "sockets",
            json!({"count":2,"sockets":[
                {"protocol":"tcp","address":"127.0.0.1","port":"2222","pid":"42"},
                {"protocol":"udp","address":"::1","port":"5353","pid":"42"}
            ]}),
        ),
        Operation::Posix | Operation::PlainExec => return Vec::new(),
    };
    let mut encoded = serde_json::to_vec(&json!({
        "version":1,"operation":name,"platform":"Windows","runtime":"WindowsPowerShell",
        "runtime_version":"5.1.26100.123","units":"bytes seconds percent",
        "status":failure.unwrap_or("ok"),
        "data":if failure.is_some() {serde_json::Value::Null} else {data}
    }))
    .unwrap_or_else(|error| panic!("fixture JSON: {error}"));
    encoded.extend_from_slice(b"\r\n");
    encoded
}

fn operation(command: &str) -> TestResult<Operation> {
    if command == PLAIN_EXEC {
        return Ok(Operation::PlainExec);
    }
    if command.contains("@@KS:monitor-platform:1@@") {
        return Ok(Operation::Posix);
    }
    let encoded = command
        .strip_prefix("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ")
        .ok_or("unexpected non-production command prefix")?;
    if command.len() >= 8000 {
        return Err("command exceeds fixed default-shell budget".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    if bytes.len() % 2 != 0 {
        return Err("incomplete UTF-16LE command".into());
    }
    let utf16: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect();
    let script = String::from_utf16(&utf16)?;
    if script.contains("Win32_OperatingSystem") {
        Ok(Operation::Resources)
    } else if script.contains("Get-NetTCPConnection") {
        Ok(Operation::Sockets)
    } else if script.contains("Get-Process") {
        Ok(Operation::Processes)
    } else {
        Ok(Operation::Probe)
    }
}

struct Peer {
    state: Arc<Control>,
    channels: HashMap<russh::ChannelId, russh::Channel<server::Msg>>,
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
        channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let id = channel.id();
        self.channels.insert(id, channel);
        let ordinal = self.state.opened.fetch_add(1, Ordering::AcqRel) + 1;
        if self.state.hold_open_at.load(Ordering::Acquire) == ordinal {
            *self
                .state
                .open_channel
                .lock()
                .map_err(|_| russh::Error::Disconnect)? = Some(id);
            self.state.open_started.add_permits(1);
            let state = self.state.clone();
            self.state
                .spawn(async move {
                    tokio::time::timeout(Duration::from_secs(3), state.open_release.acquire())
                        .await??
                        .forget();
                    reply.accept().await;
                    Ok(())
                })
                .map_err(|_| russh::Error::Disconnect)?;
        } else {
            reply.accept().await;
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        id: russh::ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.state.closed.send_modify(|closed| {
            closed.insert(id);
        });
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: russh::ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let command = std::str::from_utf8(data).map_err(|_| russh::Error::Disconnect)?;
        let operation = operation(command).map_err(|_| russh::Error::Disconnect)?;
        self.state
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(command.to_owned());
        self.state.counts[operation as usize].fetch_add(1, Ordering::Release);
        session.channel_success(id)?;
        let missing = matches!(operation, Operation::Processes)
            && self.state.missing_processes.load(Ordering::Acquire);
        let denied = matches!(operation, Operation::Resources)
            && self.state.denied_resources.load(Ordering::Acquire);
        let failed_probe =
            matches!(operation, Operation::Probe) && self.state.unconfirmed.load(Ordering::Acquire);
        let body = if matches!(operation, Operation::PlainExec) {
            PLAIN_STDOUT.to_vec()
        } else if matches!(operation, Operation::Posix) {
            Vec::new()
        } else if failed_probe {
            b"untrusted shell diagnostic\r\n".to_vec()
        } else if let Some(captures) = &self.state.captures {
            captures[operation as usize - 1].clone()
        } else {
            frame(
                operation,
                if missing {
                    Some("command_missing")
                } else if denied {
                    Some("permission_denied")
                } else {
                    None
                },
            )
        };
        let status = if matches!(operation, Operation::PlainExec) {
            23
        } else if matches!(operation, Operation::Posix) || missing || denied || failed_probe {
            1
        } else {
            0
        };
        let held = {
            let mut replies = self
                .state
                .held_replies
                .lock()
                .map_err(|_| russh::Error::Disconnect)?;
            replies
                .iter()
                .position(|(candidate, _)| *candidate == operation)
                .map(|index| replies.remove(index).1)
        };
        if let Some(held) = held {
            *held.channel.lock().map_err(|_| russh::Error::Disconnect)? = Some(id);
            held.started.add_permits(1);
            let handle = session.handle();
            self.state
                .spawn(async move {
                    // The handler must remain free to receive this channel's CLOSE.
                    tokio::time::timeout(Duration::from_secs(10), held.release.acquire())
                        .await??
                        .forget();
                    for chunk in body.chunks(4096) {
                        handle
                            .data(id, chunk.to_vec())
                            .await
                            .map_err(|_| "delayed stdout")?;
                    }
                    if operation == Operation::PlainExec {
                        handle
                            .extended_data(id, 1, PLAIN_STDERR.to_vec())
                            .await
                            .map_err(|_| "delayed stderr")?;
                    }
                    handle
                        .exit_status_request(id, status)
                        .await
                        .map_err(|_| "delayed status")?;
                    handle.eof(id).await.map_err(|_| "delayed EOF")?;
                    handle.close(id).await.map_err(|_| "delayed CLOSE")?;
                    held.finished.add_permits(1);
                    Ok(())
                })
                .map_err(|_| russh::Error::Disconnect)?;
            return Ok(());
        }
        for chunk in body.chunks(4096) {
            session.data(id, chunk.to_vec())?;
        }
        if operation == Operation::PlainExec {
            session.extended_data(id, 1, PLAIN_STDERR.to_vec())?;
        }
        session.exit_status_request(id, status)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}

struct Fixture {
    session: SshSession,
    state: Arc<Control>,
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<TestResult>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.stop();
        self.task.abort();
    }
}

impl Fixture {
    async fn new(control: Control) -> TestResult<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x71; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let state = Arc::new(control);
        let peer = Peer {
            state: state.clone(),
            channels: HashMap::new(),
        };
        let server_state = state.clone();
        let task = tokio::spawn(async move {
            let (socket, _) =
                tokio::time::timeout(Duration::from_secs(5), listener.accept()).await??;
            let socket = socket.into_std()?;
            *server_state
                .socket
                .lock()
                .map_err(|_| "server socket lock")? = Some(socket.try_clone()?);
            let socket = tokio::net::TcpStream::from_std(socket)?;
            server::run_stream(config, socket, peer).await?.await?;
            TestResult::Ok(())
        });
        let mut options = SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(3);
        let session = match SshSession::connect(options).await {
            Ok(session) => session,
            Err(error) => {
                state.stop();
                task.abort();
                let _ = task.await;
                return Err(error.into());
            }
        };
        Ok(Self {
            session,
            state,
            address,
            task,
        })
    }

    async fn close(mut self) -> TestResult {
        let mut jobs = {
            let mut registered = self.state.jobs.lock().map_err(|_| "fixture jobs lock")?;
            std::mem::take(&mut *registered)
        };
        jobs.abort_all();
        let workers = tokio::time::timeout(IO_BUDGET, async {
            while let Some(result) = jobs.join_next().await {
                match result {
                    Ok(result) => result?,
                    Err(error) if error.is_cancelled() => {}
                    Err(error) => return Err(error.into()),
                }
            }
            TestResult::Ok(())
        })
        .await;
        let disconnected = self.session.close().await;
        let server = tokio::time::timeout(IO_BUDGET, &mut self.task).await;
        if server.is_err() {
            self.state.stop();
            // Closing the duplicate socket also stops russh's internal session.
            let _ = tokio::time::timeout(IO_BUDGET, &mut self.task).await?;
        }
        workers??;
        disconnected?;
        server???;
        assert!(
            tokio::net::TcpStream::connect(self.address).await.is_err(),
            "owned listener survived"
        );
        Ok(())
    }

    async fn run(case: impl AsyncFnOnce(&Self) -> TestResult) -> TestResult {
        let fixture = Self::new(Control::default()).await?;
        let result = {
            let mut pending = Box::pin(case(&fixture));
            tokio::time::timeout(
                Duration::from_secs(15),
                std::future::poll_fn(|cx| {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pending.as_mut().poll(cx)
                    })) {
                        Ok(Poll::Pending) => Poll::Pending,
                        Ok(Poll::Ready(result)) => Poll::Ready(Ok(result)),
                        Err(panic) => Poll::Ready(Err(panic)),
                    }
                }),
            )
            .await
        };
        let cleanup = fixture.close().await;
        match result? {
            Ok(result) => result?,
            Err(panic) => std::panic::resume_unwind(panic),
        }
        cleanup
    }
}

async fn collect(monitor: &LinuxMonitor, operation: Operation) -> Result<(), MonitorError> {
    match operation {
        Operation::Resources => monitor.snapshot().await.map(|_| ()),
        Operation::Processes => monitor.processes().await.map(|_| ()),
        Operation::Sockets => monitor.listening_sockets().await.map(|_| ()),
        _ => unreachable!("only read collectors enter this helper"),
    }
}

async fn revoke(monitor: &LinuxMonitor, reset: bool) {
    if reset {
        monitor.reset_capabilities().await;
    } else {
        monitor.invalidate_pending_capabilities();
    }
}

async fn cancelled<T>(pending: impl Future<Output = Result<T, MonitorError>>) -> TestResult {
    match tokio::time::timeout(CANCEL_BUDGET, pending).await? {
        Err(MonitorError::Cancelled) => Ok(()),
        Err(error) => Err(format!("expected Cancelled before SSH timeout, got {error:?}").into()),
        Ok(_) => Err("revoked reader completed successfully".into()),
    }
}

async fn poll_pending<F: Future>(mut pending: Pin<&mut F>) {
    std::future::poll_fn(|cx| {
        assert!(
            pending.as_mut().poll(cx).is_pending(),
            "reader must still be pending"
        );
        Poll::Ready(())
    })
    .await;
}

async fn ordinary_exec(session: &SshSession) -> TestResult {
    let output = tokio::time::timeout(IO_BUDGET, session.exec(PLAIN_EXEC)).await??;
    assert_eq!(output.stdout, PLAIN_STDOUT);
    assert_eq!(output.stderr, PLAIN_STDERR);
    assert_eq!(output.exit_status, Some(23));
    Ok(())
}

async fn sent_collector_case(operation: Operation, reset: bool) -> TestResult {
    Fixture::run(async move |fixture| {
        let monitor = LinuxMonitor::new(fixture.session.clone());
        for collector in [
            Operation::Resources,
            Operation::Processes,
            Operation::Sockets,
        ] {
            collect(&monitor, collector).await?;
        }
        let before = monitor.capabilities().await;
        let report = monitor.capability_report().await;
        let held = fixture.state.hold_reply(operation)?;
        let mut pending = Box::pin(collect(&monitor, operation));
        let channel = held.wait_sent(pending.as_mut()).await?;
        assert_eq!(
            fixture.state.count(operation),
            2,
            "{operation:?} exec was sent"
        );

        revoke(&monitor, reset).await;
        cancelled(pending.as_mut()).await?;
        fixture.state.wait_closed(channel).await?;
        assert_eq!(held.release.available_permits(), 0);
        assert_eq!(held.finished.available_permits(), 0, "peer has not replied");
        assert!(!monitor.accepts_capability_report(&report));
        let expected = if reset { Default::default() } else { before };
        assert_eq!(monitor.capabilities().await, expected);

        held.release_and_wait().await?;
        ordinary_exec(&fixture.session).await?;
        assert_eq!(
            monitor.capabilities().await,
            expected,
            "late reply cannot refresh cache"
        );
        collect(&monitor, operation).await?;
        let after = monitor.capabilities().await;
        let observation = match operation {
            Operation::Resources => after.resources,
            Operation::Processes => after.processes,
            Operation::Sockets => after.sockets,
            _ => unreachable!("collector case"),
        };
        assert_eq!(observation.state, CapabilityState::Available);
        assert!(observation.checked_at.is_some());
        assert_eq!(after.environment, before.environment);
        assert_eq!(fixture.state.count(operation), 3);
        assert_eq!(
            fixture.state.count(Operation::Posix),
            if reset { 2 } else { 1 }
        );
        assert_eq!(
            fixture.state.count(Operation::Probe),
            if reset { 2 } else { 1 }
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn invalidate_sent_resources_returns_cancelled_and_preserves_cache() -> TestResult {
    sent_collector_case(Operation::Resources, false).await
}

#[tokio::test]
async fn reset_sent_resources_returns_cancelled_and_clears_cache() -> TestResult {
    sent_collector_case(Operation::Resources, true).await
}

#[tokio::test]
async fn invalidate_sent_processes_returns_cancelled_and_preserves_cache() -> TestResult {
    sent_collector_case(Operation::Processes, false).await
}

#[tokio::test]
async fn reset_sent_processes_returns_cancelled_and_clears_cache() -> TestResult {
    sent_collector_case(Operation::Processes, true).await
}

#[tokio::test]
async fn invalidate_sent_sockets_returns_cancelled_and_preserves_cache() -> TestResult {
    sent_collector_case(Operation::Sockets, false).await
}

#[tokio::test]
async fn reset_sent_sockets_returns_cancelled_and_clears_cache() -> TestResult {
    sent_collector_case(Operation::Sockets, true).await
}

async fn identity_and_gate_case(reset: bool) -> TestResult {
    for operation in [Operation::Posix, Operation::Probe] {
        Fixture::run(async move |fixture| {
            let monitor = LinuxMonitor::new(fixture.session.clone());
            let held = fixture.state.hold_reply(operation)?;
            let mut leader = Box::pin(collect(&monitor, Operation::Resources));
            let channel = held.wait_sent(leader.as_mut()).await?;
            let mut waiters = [
                Box::pin(collect(&monitor, Operation::Resources)),
                Box::pin(collect(&monitor, Operation::Processes)),
                Box::pin(collect(&monitor, Operation::Sockets)),
            ];
            for waiter in &mut waiters {
                poll_pending(waiter.as_mut()).await;
            }
            let report = monitor.capability_report().await;
            revoke(&monitor, reset).await;

            // Keep the leader unpolled, retaining probe_gate: each waiter must
            // notice invalidation without waiting for that mutex to be released.
            for waiter in &mut waiters {
                cancelled(waiter.as_mut()).await?;
            }
            assert_eq!(held.finished.available_permits(), 0);
            cancelled(leader.as_mut()).await?;
            fixture.state.wait_closed(channel).await?;
            assert!(!monitor.accepts_capability_report(&report));
            assert_eq!(monitor.capabilities().await, Default::default());
            for collector in [
                Operation::Resources,
                Operation::Processes,
                Operation::Sockets,
            ] {
                assert_eq!(
                    fixture.state.count(collector),
                    0,
                    "no stale collector dispatch"
                );
            }
            assert_eq!(fixture.state.count(Operation::Posix), 1);
            assert_eq!(
                fixture.state.count(Operation::Probe),
                usize::from(operation == Operation::Probe)
            );

            held.release_and_wait().await?;
            ordinary_exec(&fixture.session).await?;
            assert_eq!(monitor.capabilities().await, Default::default());
            assert_eq!(monitor.processes().await?.len(), 2);
            assert_eq!(fixture.state.count(Operation::Posix), 2);
            assert_eq!(
                fixture.state.count(Operation::Probe),
                1 + usize::from(operation == Operation::Probe)
            );
            assert_eq!(fixture.state.count(Operation::Resources), 0);
            assert_eq!(fixture.state.count(Operation::Processes), 1);
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn invalidate_identity_and_probe_gate_waiters_return_cancelled() -> TestResult {
    identity_and_gate_case(false).await
}

#[tokio::test]
async fn reset_identity_and_probe_gate_waiters_return_cancelled() -> TestResult {
    identity_and_gate_case(true).await
}

#[tokio::test]
async fn cancelled_generation_leaves_new_generation_and_plain_exec_channels_alive() -> TestResult {
    for reset in [false, true] {
        Fixture::run(async move |fixture| {
            let monitor = LinuxMonitor::new(fixture.session.clone());
            monitor.snapshot().await?;
            let before = monitor.capabilities().await;
            let old = fixture.state.hold_reply(Operation::Resources)?;
            let mut old_reader = Box::pin(monitor.snapshot());
            let old_channel = old.wait_sent(old_reader.as_mut()).await?;

            let sibling_monitor = LinuxMonitor::new(fixture.session.clone());
            let sibling = fixture.state.hold_reply(Operation::Processes)?;
            let mut sibling_reader = Box::pin(sibling_monitor.processes());
            let sibling_channel = sibling.wait_sent(sibling_reader.as_mut()).await?;
            let ordinary = fixture.state.hold_reply(Operation::PlainExec)?;
            let mut exec = Box::pin(fixture.session.exec(PLAIN_EXEC));
            let exec_channel = ordinary.wait_sent(exec.as_mut()).await?;

            revoke(&monitor, reset).await;
            let new = fixture.state.hold_reply(Operation::Resources)?;
            let mut new_reader = Box::pin(monitor.snapshot());
            let new_channel = new.wait_sent(new_reader.as_mut()).await?;
            assert_ne!(old_channel, new_channel);
            // Admit the new generation before polling the old cancellation so
            // old cleanup runs while all three unaffected channels are live.
            cancelled(old_reader.as_mut()).await?;
            fixture.state.wait_closed(old_channel).await?;
            for channel in [new_channel, sibling_channel, exec_channel] {
                assert!(!fixture.state.closed.borrow().contains(&channel));
            }
            old.release_and_wait().await?;
            ordinary_exec(&fixture.session).await?;
            poll_pending(new_reader.as_mut()).await;
            poll_pending(sibling_reader.as_mut()).await;
            poll_pending(exec.as_mut()).await;
            assert_eq!(
                monitor.capabilities().await.resources,
                if reset {
                    Default::default()
                } else {
                    before.resources
                }
            );

            new.release_and_wait().await?;
            let sample = tokio::time::timeout(IO_BUDGET, new_reader.as_mut()).await??;
            assert_eq!(sample.cpu, CpuSample::ReportedPercent(25.));
            let current = monitor.capabilities().await;
            assert_eq!(current.resources.state, CapabilityState::Available);
            sibling.release_and_wait().await?;
            let processes = tokio::time::timeout(IO_BUDGET, sibling_reader.as_mut()).await??;
            assert_eq!(processes.iter().map(|p| p.pid).collect::<Vec<_>>(), [0, 42]);
            ordinary.release_and_wait().await?;
            let output = tokio::time::timeout(IO_BUDGET, exec.as_mut()).await??;
            assert_eq!(output.stdout, PLAIN_STDOUT);
            assert_eq!(output.stderr, PLAIN_STDERR);
            assert_eq!(output.exit_status, Some(23));
            assert_eq!(monitor.capabilities().await, current);
            ordinary_exec(&fixture.session).await?;
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn windows_socket_first_establishes_identity_and_all_read_only_fields_stay_honest()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let sockets = monitor.listening_sockets().await?;
        assert_eq!(sockets.len(), 2);
        assert_eq!(sockets[0].state, "LISTEN");
        assert_eq!(sockets[1].state, "BOUND");
        let environment = monitor
            .capabilities()
            .await
            .environment
            .ok_or("missing Windows identity")?;
        assert_eq!(environment.platform, MonitorPlatform::Windows);
        assert_eq!(
            environment.runtime,
            MonitorRuntime::WindowsPowerShell {
                version: [5, 1, 26100, 123]
            }
        );
        let sample = monitor.snapshot().await?;
        assert_eq!(sample.cpu, CpuSample::ReportedPercent(25.));
        assert_eq!(sample.load_average, None);
        assert_eq!(sample.memory.free_bytes, Some(262144));
        assert_eq!(sample.memory.used_bytes(), None);
        assert_eq!(sample.cpu.cumulative(), None);
        assert!(sample.rates_since(&sample).networks.is_empty());
        assert_eq!(sample.filesystems[0].used_bytes, 600);
        assert_eq!(sample.filesystems[0].available_bytes, 300);
        let processes = monitor.processes().await?;
        assert_eq!(processes.iter().map(|p| p.pid).collect::<Vec<_>>(), [0, 42]);
        assert_eq!(processes[1].working_set_bytes, Some(4096));
        assert!(
            processes
                .iter()
                .all(|p| p.user.is_none() && p.cpu_percent.is_none() && p.memory_percent.is_none())
        );
        assert_eq!(fixture.state.count(Operation::Posix), 1);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_clones_share_independent_negative_cache_and_explicit_reset_redetects() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            missing_processes: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let clone = monitor.clone();
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::CommandMissing,
                ..
            })
        ));
        let before = monitor.capabilities().await;
        fixture
            .state
            .missing_processes
            .store(false, Ordering::Release);
        assert!(clone.processes().await.is_err());
        assert_eq!(fixture.state.count(Operation::Processes), 1);
        assert!(
            clone.snapshot().await.is_ok(),
            "process failure must not suppress resources"
        );
        assert_eq!(monitor.capabilities().await.processes, before.processes);
        monitor.invalidate_pending_capabilities();
        assert!(monitor.snapshot().await.is_ok());
        assert_eq!(
            fixture.state.count(Operation::Probe),
            1,
            "pause preserves completed identity"
        );
        clone.reset_capabilities().await;
        assert_eq!(monitor.capabilities().await.environment, None);
        assert_eq!(clone.processes().await?.len(), 2);
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        assert_eq!(fixture.state.count(Operation::Processes), 2);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_resource_permission_cache_preserves_age_until_explicit_recheck() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            denied_resources: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::PermissionDenied,
                ..
            })
        ));
        let before = monitor.capabilities().await;
        let report = monitor.capability_report().await;
        fixture
            .state
            .denied_resources
            .store(false, Ordering::Release);
        for _ in 0..2 {
            assert!(matches!(
                monitor.clone().snapshot().await,
                Err(MonitorError::CapabilityUnavailable {
                    reason: CapabilityReason::PermissionDenied,
                    ..
                })
            ));
        }
        assert_eq!(monitor.capabilities().await, before);
        assert!(monitor.accepts_capability_report(&report));
        assert_eq!(fixture.state.count(Operation::Resources), 1);
        assert_eq!(monitor.processes().await?.len(), 2);
        monitor.reset_capabilities().await;
        assert!(!monitor.accepts_capability_report(&report));
        assert!(monitor.snapshot().await.is_ok());
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        assert_eq!(fixture.state.count(Operation::Resources), 2);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_pending_identity_cannot_start_a_collector_after_reset() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        Fixture::run(async |fixture| {
            let monitor = LinuxMonitor::new(fixture.session.clone());
            let held = fixture.state.hold_reply(Operation::Probe)?;
            let pending_monitor = monitor.clone();
            let mut pending = Box::pin(async move { pending_monitor.snapshot().await });
            let channel = held.wait_sent(pending.as_mut()).await?;
            monitor.reset_capabilities().await;
            cancelled(pending.as_mut()).await?;
            fixture.state.wait_closed(channel).await?;
            held.release_and_wait().await?;
            assert_eq!(fixture.state.count(Operation::Resources), 0);
            assert_eq!(monitor.capabilities().await.environment, None);
            assert_eq!(
                monitor.capabilities().await.resources.state,
                CapabilityState::Unknown
            );
            assert_eq!(monitor.processes().await?.len(), 2);
            assert_eq!(fixture.state.count(Operation::Probe), 2);
            Ok(())
        })
        .await
    })
    .await?
}

#[tokio::test]
async fn revoked_identity_channel_admission_never_dispatches_posix_or_windows_probe() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(20), async {
        for ordinal in [1, 2] {
            for reset in [false, true] {
                Fixture::run(async move |fixture| {
                    fixture.state.hold_open_at.store(ordinal, Ordering::Release);
                    let monitor = LinuxMonitor::new(fixture.session.clone());
                    let reader = monitor.clone();
                    let mut pending = Box::pin(async move { reader.snapshot().await });
                    let channel = fixture.state.wait_open(pending.as_mut()).await?;
                    revoke(&monitor, reset).await;
                    cancelled(pending.as_mut()).await?;
                    assert!(!fixture.state.closed.borrow().contains(&channel));
                    fixture.state.open_release.add_permits(1);
                    fixture.state.wait_closed(channel).await?;
                    assert_eq!(fixture.state.count(Operation::Posix), ordinal - 1);
                    assert_eq!(fixture.state.count(Operation::Probe), 0);
                    assert_eq!(fixture.state.count(Operation::Resources), 0);
                    assert_eq!(monitor.capabilities().await.environment, None);
                    assert_eq!(
                        monitor.capabilities().await.resources.state,
                        CapabilityState::Unknown
                    );
                    ordinary_exec(&fixture.session).await?;
                    Ok(())
                })
                .await?;
            }
        }
        TestResult::Ok(())
    })
    .await?
}

#[tokio::test]
async fn revoked_cached_collector_channel_admission_never_dispatches_any_read_operation()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(25), async {
        for operation in [
            Operation::Resources,
            Operation::Processes,
            Operation::Sockets,
        ] {
            for reset in [false, true] {
                Fixture::run(async move |fixture| {
                    let monitor = LinuxMonitor::new(fixture.session.clone());
                    monitor.snapshot().await?;
                    let before = monitor.capabilities().await;
                    let commands = fixture
                        .state
                        .commands
                        .lock()
                        .map_err(|_| "commands lock")?
                        .len();
                    let report = monitor.capability_report().await;
                    fixture.state.hold_open_at.store(
                        fixture.state.opened.load(Ordering::Acquire) + 1,
                        Ordering::Release,
                    );
                    let reader = monitor.clone();
                    let mut pending = Box::pin(async move {
                        match operation {
                            Operation::Resources => reader.snapshot().await.map(|_| ()),
                            Operation::Processes => reader.processes().await.map(|_| ()),
                            Operation::Sockets => reader.listening_sockets().await.map(|_| ()),
                            _ => unreachable!("only read collectors enter this fixture"),
                        }
                    });
                    let channel = fixture.state.wait_open(pending.as_mut()).await?;
                    revoke(&monitor, reset).await;
                    cancelled(pending.as_mut()).await?;
                    assert!(!fixture.state.closed.borrow().contains(&channel));
                    fixture.state.open_release.add_permits(1);
                    fixture.state.wait_closed(channel).await?;
                    assert_eq!(
                        fixture
                            .state
                            .commands
                            .lock()
                            .map_err(|_| "commands lock")?
                            .len(),
                        commands,
                        "{operation:?} must not send exec after revoked admission"
                    );
                    assert!(!monitor.accepts_capability_report(&report));
                    assert_eq!(
                        monitor.capabilities().await,
                        if reset { Default::default() } else { before }
                    );
                    ordinary_exec(&fixture.session).await?;
                    Ok(())
                })
                .await?;
            }
        }
        TestResult::Ok(())
    })
    .await?
}

#[tokio::test]
async fn simultaneous_readers_deduplicate_identity_probes_on_one_connection() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let clone = monitor.clone();
        let (resources, processes, sockets) = tokio::join!(
            monitor.snapshot(),
            clone.processes(),
            monitor.listening_sockets(),
        );
        assert!(resources.is_ok() && processes.is_ok() && sockets.is_ok());
        assert_eq!(fixture.state.count(Operation::Posix), 1);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn two_failed_probes_leave_environment_unknown_and_do_not_cache_a_missing_command()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            unconfirmed: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::EnvironmentUnconfirmed)
        ));
        let observed = monitor.capabilities().await;
        assert_eq!(observed.environment, None);
        assert_eq!(observed.resources.state, CapabilityState::Unknown);
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::EnvironmentUnconfirmed)
        ));
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        fixture.state.unconfirmed.store(false, Ordering::Release);
        assert!(
            monitor.snapshot().await.is_ok(),
            "unknown identity may be re-observed"
        );
        assert_eq!(fixture.state.count(Operation::Probe), 3);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_reviewed_linux_actions_fail_before_reading_or_signalling_processes() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let socket = SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*".into(),
            process: Some("PID 42".into()),
        };
        assert!(matches!(
            monitor.probe_tcp(&socket).await,
            Err(MonitorError::Unsupported(_))
        ));
        let process = ProcessInfo {
            pid: 42,
            user: None,
            cpu_percent: None,
            memory_percent: None,
            working_set_bytes: Some(4096),
            command: "test".into(),
        };
        assert!(matches!(
            monitor.inspect_process(&process).await,
            Err(MonitorError::Unsupported(_))
        ));
        assert_eq!(fixture.state.count(Operation::Processes), 0);
        assert_eq!(fixture.state.count(Operation::Sockets), 0);
        assert_eq!(fixture.state.count(Operation::Resources), 0);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires genuine Windows API frames from the isolated Windows monitor lab"]
async fn captured_windows_api_frames_are_accepted_through_authenticated_monitor_routing()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(20), async {
        let directory = PathBuf::from(
            std::env::var_os("KEELSHELL_WINDOWS_MONITOR_CAPTURE_DIR")
                .ok_or("capture directory required")?,
        );
        let mut captures: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::new());
        for (slot, filename) in captures.iter_mut().zip([
            "probe.stdout",
            "resources.stdout",
            "processes.stdout",
            "sockets.stdout",
        ]) {
            *slot = std::fs::read(directory.join(filename))?;
            if slot.len() > 2 * 1024 * 1024 {
                return Err("capture exceeds the production output bound".into());
            }
        }
        let fixture = Fixture::new(Control {
            captures: Some(captures),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let sample = monitor.snapshot().await?;
        assert_eq!(sample.platform, MonitorPlatform::Windows);
        assert!(sample.memory.total_bytes > 0);
        assert!(sample.memory.available_bytes.is_none() && sample.memory.used_bytes().is_none());
        assert_eq!(sample.load_average, None);
        assert!(sample.cpu.cumulative().is_none());
        let _processes = monitor.processes().await?;
        let _sockets = monitor.listening_sockets().await?;
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}
