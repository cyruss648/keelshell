#![allow(clippy::unwrap_used, clippy::expect_used)]

use keelshell_mcp::*;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, Lines, ReadHalf,
        WriteHalf,
    },
    net::{TcpListener, TcpStream},
    process::Command,
    sync::Notify,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn target() -> SessionIdentity {
    SessionIdentity {
        connection_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
        route_revision: Uuid::from_u128(3),
    }
}
fn grant() -> SessionGrant {
    SessionGrant::new(target(), [ToolKind::ListSessions], vec![], []).unwrap()
}
#[derive(Default)]
struct FixtureBackend {
    pending: bool,
    entered: Notify,
    dropped: Arc<AtomicBool>,
}
struct DropMarker(Arc<AtomicBool>);
impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
impl DesktopBackend for FixtureBackend {
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async move {
            request.authorization.check()?;
            let _owned = DropMarker(self.dropped.clone());
            if self.pending {
                self.entered.notify_one();
                return std::future::pending().await;
            }
            match request.operation {
                Operation::ListSessions => Ok(BackendReply::Sessions {
                    sessions: vec![SessionMetadata {
                        target: target(),
                        display_name: "IPC-owned fixture".into(),
                        selection_ids: vec![],
                        granted_roots: vec![],
                    }],
                }),
                _ => Err(McpFailure::Forbidden),
            }
        })
    }
}
async fn host(backend: Arc<FixtureBackend>) -> (DesktopIpcHost, PolicyController) {
    let policy = PolicyController::default();
    policy
        .replace(AccessPolicy::enabled(vec![grant()]).unwrap())
        .unwrap();
    let server =
        KeelShellMcpServer::new(backend, policy.clone(), 2, Duration::from_secs(5)).unwrap();
    (DesktopIpcHost::bind(server).await.unwrap(), policy)
}
fn fields(host: &DesktopIpcHost) -> (String, String) {
    let config: Value = serde_json::from_str(&host.launch_environment()).unwrap();
    (
        config["env"][MCP_ADDRESS_ENV].as_str().unwrap().into(),
        config["env"][MCP_SECRET_ENV].as_str().unwrap().into(),
    )
}
struct Client {
    reader: Lines<BufReader<ReadHalf<AuthenticatedIpcStream>>>,
    writer: WriteHalf<AuthenticatedIpcStream>,
}
impl Client {
    async fn connect(host: &DesktopIpcHost) -> Self {
        let (address, secret) = fields(host);
        let stream = DesktopIpcClient::new(&address, &secret)
            .unwrap()
            .connect()
            .await
            .unwrap();
        let (reader, writer) = tokio::io::split(stream);
        let mut client = Self {
            reader: BufReader::new(reader).lines(),
            writer,
        };
        client.send(initialize()).await;
        assert_eq!(
            client.read().await["result"]["protocolVersion"],
            "2025-06-18"
        );
        client
            .send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        client
    }
    async fn send(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await.unwrap();
        self.writer.flush().await.unwrap();
    }
    async fn read(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(3), self.reader.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
}
fn initialize() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"owned-ipc-test","version":"1.0"}}})
}
fn list_sessions(id: usize) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})
}

#[tokio::test]
async fn desktop_authority_remains_live_and_is_never_copied_into_the_adapter() {
    let (host, policy) = host(Arc::new(FixtureBackend::default())).await;
    let mut client = Client::connect(&host).await;
    client.send(list_sessions(2)).await;
    assert_eq!(
        client.read().await["result"]["structuredContent"]["sessions"][0]["target"],
        json!(target())
    );
    policy.disable().unwrap();
    client.send(list_sessions(3)).await;
    assert_eq!(
        client.read().await["result"]["structuredContent"]["error"]["code"],
        "DISABLED"
    );
    host.close().await.unwrap();
}

#[tokio::test]
async fn host_drop_closes_current_listener_and_drops_owned_inflight_backend() {
    let backend = Arc::new(FixtureBackend {
        pending: true,
        ..FixtureBackend::default()
    });
    let (host, _policy) = host(backend.clone()).await;
    let (address, _) = fields(&host);
    let mut client = Client::connect(&host).await;
    client.send(list_sessions(2)).await;
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    drop(host);
    tokio::time::timeout(Duration::from_secs(3), async {
        while !backend.dropped.load(Ordering::SeqCst) || TcpStream::connect(&address).await.is_ok()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), client.reader.next_line())
            .await
            .unwrap()
            .map_or(true, |value| value.is_none())
    );
}

#[tokio::test]
async fn retiring_old_host_does_not_disable_a_replacement_desktop_policy() {
    let backend = Arc::new(FixtureBackend::default());
    let (old_host, policy) = host(backend.clone()).await;
    let (old_address, old_secret) = fields(&old_host);
    policy
        .replace(AccessPolicy::enabled(vec![grant()]).unwrap())
        .unwrap();
    let replacement = DesktopIpcHost::bind(
        KeelShellMcpServer::new(backend, policy.clone(), 2, Duration::from_secs(2)).unwrap(),
    )
    .await
    .unwrap();
    old_host.close().await.unwrap();
    assert!(
        DesktopIpcClient::new(&old_address, &old_secret)
            .unwrap()
            .connect()
            .await
            .is_err()
    );
    let mut client = Client::connect(&replacement).await;
    client.send(list_sessions(2)).await;
    assert_eq!(
        client.read().await["result"]["structuredContent"]["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    replacement.close().await.unwrap();
}

#[tokio::test]
async fn eight_incomplete_authentications_are_bounded_and_release_their_slots() {
    let (host, _) = host(Arc::new(FixtureBackend::default())).await;
    let (address, secret) = fields(&host);
    let mut idle = Vec::new();
    for _ in 0..MAX_IPC_CONNECTIONS {
        idle.push(TcpStream::connect(&address).await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let client = DesktopIpcClient::new(&address, &secret).unwrap();
    assert_eq!(
        client.connect().await.unwrap_err(),
        IpcFailure::Authentication
    );
    tokio::time::sleep(Duration::from_millis(2100)).await;
    for mut stream in idle {
        let mut bytes = [0; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), stream.read(&mut bytes))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
    assert!(client.connect().await.is_ok());
    host.close().await.unwrap();
}

#[test]
fn copied_fields_reject_partial_remote_and_invalid_capability_formats() {
    let secret = "a".repeat(64);
    for address in [
        "localhost:1",
        "127.0.0.2:1",
        "0.0.0.0:1",
        "[::1]:1",
        "127.0.0.1:0",
        "127.0.0.1:65536",
        "https://127.0.0.1:1",
    ] {
        assert_eq!(
            DesktopIpcClient::new(address, &secret).unwrap_err(),
            IpcFailure::InvalidConfiguration
        );
    }
    for secret in ["", "bad", &"a".repeat(63), &"g".repeat(64)] {
        assert_eq!(
            DesktopIpcClient::new("127.0.0.1:1234", secret).unwrap_err(),
            IpcFailure::InvalidConfiguration
        );
    }
    let client = DesktopIpcClient::new("127.0.0.1:1234", &"a".repeat(64)).unwrap();
    assert!(!format!("{client:?}").contains(&secret));
}

const CHILD_PIPE_LIMIT: usize = 32 * 1024;
const CHILD_OUTPUT_DEADLINE: Duration = Duration::from_secs(2);
const CHILD_CLEANUP_DEADLINE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapturePhase {
    Spawn,
    Wait,
    StdoutRead,
    StderrRead,
    StdoutQuota,
    StderrQuota,
    Deadline,
    Cancelled,
}

#[derive(Debug)]
enum CleanupIssue {
    Kill,
    Wait,
    Deadline,
}

struct PipeCapture {
    bytes: Vec<u8>,
    observed_bytes: usize,
    eof: bool,
}

impl Default for PipeCapture {
    fn default() -> Self {
        Self {
            bytes: Vec::with_capacity(CHILD_PIPE_LIMIT),
            observed_bytes: 0,
            eof: false,
        }
    }
}

// Failed assertions must not dump captured protocol bodies into a test log.
impl std::fmt::Debug for PipeCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PipeCapture")
            .field("observed_bytes", &self.observed_bytes)
            .field("retained_bytes", &self.bytes.len())
            .field("eof", &self.eof)
            .finish()
    }
}

#[derive(Debug)]
struct OwnedOutput {
    pid: Option<u32>,
    status: std::process::ExitStatus,
    stdout: PipeCapture,
    stderr: PipeCapture,
}

struct CaptureFailure {
    iteration: usize,
    pid: Option<u32>,
    phase: CapturePhase,
    status: Option<std::process::ExitStatus>,
    stdout: PipeCapture,
    stderr: PipeCapture,
    kill_requested: bool,
    reaped: bool,
    cleanup_issue: Option<CleanupIssue>,
}

impl std::fmt::Debug for CaptureFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CaptureFailure")
            .field("iteration", &self.iteration)
            .field("pid", &self.pid)
            .field("phase", &self.phase)
            .field("status", &self.status)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("kill_requested", &self.kill_requested)
            .field("reaped", &self.reaped)
            .field("cleanup_issue", &self.cleanup_issue)
            .finish()
    }
}

async fn capture_pipe<R: AsyncRead + Unpin>(
    mut reader: R,
    capture: &mut PipeCapture,
    quota: CapturePhase,
    read_error: CapturePhase,
) -> Result<(), CapturePhase> {
    let mut chunk = [0_u8; 4096];
    loop {
        // Read at most one extra byte to distinguish an exact-limit EOF from
        // overflow, while retaining no more than the fixed pipe quota.
        let remaining = CHILD_PIPE_LIMIT - capture.bytes.len();
        let read_size = chunk.len().min(remaining + 1);
        let count = reader
            .read(&mut chunk[..read_size])
            .await
            .map_err(|_| read_error)?;
        if count == 0 {
            capture.eof = true;
            return Ok(());
        }
        capture.observed_bytes += count;
        capture
            .bytes
            .extend_from_slice(&chunk[..count.min(remaining)]);
        if count > remaining {
            return Err(quota);
        }
    }
}

async fn capture_owned_child(
    mut command: Command,
    iteration: usize,
    cancelled: CancellationToken,
) -> Result<OwnedOutput, CaptureFailure> {
    command
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // This is the original total spawn/output budget, per environment pair.
    // Cleanup has a separate budget and can never turn a deadline into success.
    let deadline = tokio::time::Instant::now() + CHILD_OUTPUT_DEADLINE;
    let mut child = command.spawn().map_err(|_| CaptureFailure {
        iteration,
        pid: None,
        phase: CapturePhase::Spawn,
        status: None,
        stdout: PipeCapture::default(),
        stderr: PipeCapture::default(),
        kill_requested: false,
        reaped: false,
        cleanup_issue: None,
    })?;
    let pid = child.id();
    let stdout_reader = child.stdout.take().unwrap();
    let stderr_reader = child.stderr.take().unwrap();
    let mut stdout = PipeCapture::default();
    let mut stderr = PipeCapture::default();
    let mut status = None;
    // The three futures remain owned here; there are no detached reader tasks.
    // Cancelling this select drops the read handles before explicit child reap.
    let result = tokio::select! {
        biased;
        _ = cancelled.cancelled() => Err(CapturePhase::Cancelled),
        output = tokio::time::timeout_at(deadline, async {
            tokio::try_join!(
                async {
                    status = Some(child.wait().await.map_err(|_| CapturePhase::Wait)?);
                    Ok(())
                },
                capture_pipe(stdout_reader, &mut stdout, CapturePhase::StdoutQuota, CapturePhase::StdoutRead),
                capture_pipe(stderr_reader, &mut stderr, CapturePhase::StderrQuota, CapturePhase::StderrRead),
            ).map(|_| ())
        }) => output.unwrap_or(Err(CapturePhase::Deadline)),
    };
    // A ready future can win against an already elapsed Tokio timer; require
    // real completion within the unchanged total deadline before success.
    let result = if result.is_ok() && tokio::time::Instant::now() >= deadline {
        Err(CapturePhase::Deadline)
    } else {
        result
    };
    if let Err(phase) = result {
        let mut kill_requested = false;
        let mut cleanup_issue = None;
        if status.is_none() {
            kill_requested = true;
            if child.start_kill().is_err() {
                cleanup_issue = Some(CleanupIssue::Kill);
            }
            match tokio::time::timeout(CHILD_CLEANUP_DEADLINE, child.wait()).await {
                Ok(Ok(exit)) => status = Some(exit),
                Ok(Err(_)) => cleanup_issue = Some(CleanupIssue::Wait),
                Err(_) => cleanup_issue = Some(CleanupIssue::Deadline),
            }
        }
        return Err(CaptureFailure {
            iteration,
            pid,
            phase,
            reaped: status.is_some(),
            status,
            stdout,
            stderr,
            kill_requested,
            cleanup_issue,
        });
    }
    Ok(OwnedOutput {
        pid,
        status: status.unwrap(),
        stdout,
        stderr,
    })
}

#[tokio::test]
async fn binary_rejects_partial_or_invalid_environment_without_stdout_fallback() {
    for (iteration, (address, secret)) in [
        (Some("127.0.0.1:1"), None),
        (None, Some("abcd")),
        (Some("0.0.0.0:1"), Some("abcd")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut command = Command::new(env!("CARGO_BIN_EXE_keelshell-mcp"));
        command
            .kill_on_drop(true)
            .env_remove(MCP_ADDRESS_ENV)
            .env_remove(MCP_SECRET_ENV);
        if let Some(address) = address {
            command.env(MCP_ADDRESS_ENV, address);
        }
        if let Some(secret) = secret {
            command.env(MCP_SECRET_ENV, secret);
        }
        let result = capture_owned_child(command, iteration, CancellationToken::new())
            .await
            .unwrap_or_else(|failure| panic!("{failure:?}"));
        assert!(
            !result.status.success(),
            "environment iteration {iteration}: unexpected success"
        );
        assert!(
            result.stdout.bytes.is_empty(),
            "environment iteration {iteration}: unexpected stdout {:?}",
            result.stdout
        );
        assert!(
            std::str::from_utf8(&result.stderr.bytes)
                .ok()
                .map(str::trim)
                == Some("invalid MCP desktop launch configuration"),
            "environment iteration {iteration}: static rejection mismatch {:?}",
            result.stderr
        );
    }
}

#[tokio::test]
async fn transparent_stdio_binary_serves_the_desktop_owned_protocol_and_exits_on_eof() {
    let (host, _) = host(Arc::new(FixtureBackend::default())).await;
    let (address, secret) = fields(&host);
    let mut child = Command::new(env!("CARGO_BIN_EXE_keelshell-mcp"))
        .env(MCP_ADDRESS_ENV, address)
        .env(MCP_SECRET_ENV, secret)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    async fn send(stdin: &mut tokio::process::ChildStdin, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        stdin.write_all(&bytes).await.unwrap();
        stdin.flush().await.unwrap();
    }
    async fn read(stdout: &mut Lines<BufReader<tokio::process::ChildStdout>>) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(3), stdout.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    send(&mut stdin, initialize()).await;
    assert_eq!(
        read(&mut stdout).await["result"]["protocolVersion"],
        "2025-06-18"
    );
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    send(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await;
    assert_eq!(
        read(&mut stdout).await["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    send(&mut stdin, list_sessions(3)).await;
    assert_eq!(
        read(&mut stdout).await["result"]["structuredContent"]["sessions"][0]["display_name"],
        "IPC-owned fixture"
    );
    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .unwrap()
        .unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    assert!(status.success(), "static adapter diagnostic: {stderr}");
    host.close().await.unwrap();
}

#[tokio::test]
async fn fake_listener_times_out_without_accepting_context_from_a_stdio_input() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client =
        DesktopIpcClient::new(&listener.local_addr().unwrap().to_string(), &"a".repeat(64))
            .unwrap();
    let accepted = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let mut hello = [0; 40];
        stream.read_exact(&mut hello).await.unwrap();
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).await.unwrap();
        rest
    });
    let result = client
        .bridge(&b"private context must never arrive"[..], tokio::io::sink())
        .await;
    assert_eq!(result.unwrap_err(), IpcFailure::AuthenticationDeadline);
    assert!(accepted.await.unwrap().is_empty());
}

#[tokio::test]
async fn host_close_interrupts_authenticated_connection_before_sdk_initialize() {
    let (host, _) = host(Arc::new(FixtureBackend::default())).await;
    let (address, secret) = fields(&host);
    let mut stream = DesktopIpcClient::new(&address, &secret)
        .unwrap()
        .connect()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), host.close())
        .await
        .unwrap()
        .unwrap();
    let mut byte = [0];
    assert!(
        tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
            .await
            .unwrap()
            .is_err()
    );
}

#[tokio::test]
async fn unread_stdio_output_and_input_eof_cannot_keep_the_adapter_or_host_alive() {
    let (host, _) = host(Arc::new(FixtureBackend::default())).await;
    let (address, secret) = fields(&host);
    let mut child = Command::new(env!("CARGO_BIN_EXE_keelshell-mcp"))
        .env(MCP_ADDRESS_ENV, address)
        .env(MCP_SECRET_ENV, secret)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = serde_json::to_vec(&initialize()).unwrap();
    input.push(b'\n');
    input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n");
    for id in 2..128 {
        let mut request =
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{}}))
                .unwrap();
        request.push(b'\n');
        input.extend(request);
    }
    let mut stdin = child.stdin.take().unwrap();
    // An early server close is valid; only the shutdown bound is under test.
    let _ = tokio::time::timeout(Duration::from_secs(2), stdin.write_all(&input)).await;
    drop(stdin);
    tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), host.close())
        .await
        .unwrap()
        .unwrap();
}

const OUTPUT_FIXTURE_MODE: &str = "KEELSHELL_MCP_OUTPUT_TEST_FIXTURE";
const OUTPUT_FIXTURE_READY: &str = "KEELSHELL_MCP_OUTPUT_TEST_READY";

fn output_fixture(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("owned_output_fixture")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(OUTPUT_FIXTURE_MODE, mode)
        .env_remove(OUTPUT_FIXTURE_READY);
    command
}

// The test executable itself is the portable, self-owned child fixture. Normal
// test runs do nothing here; only explicitly launched fixture children emit.
#[test]
fn owned_output_fixture() {
    use std::io::Write;
    let Ok(mode) = std::env::var(OUTPUT_FIXTURE_MODE) else {
        return;
    };
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    match mode.as_str() {
        "clean" => {
            stdout.write_all(b"fixture-complete-stdout\n").unwrap();
            stderr.write_all(b"fixture-complete-stderr\n").unwrap();
            stdout.flush().unwrap();
            stderr.flush().unwrap();
            return;
        }
        "stderr-limit-eof" => {
            stderr.write_all(&vec![b'x'; CHILD_PIPE_LIMIT]).unwrap();
            stderr.flush().unwrap();
            return;
        }
        "partial" => {
            stdout.write_all(b"fixture-partial-stdout").unwrap();
            stderr.write_all(b"fixture-partial-stderr").unwrap();
        }
        "stdout-quota" => stdout
            .write_all(&vec![b'x'; CHILD_PIPE_LIMIT + 4096])
            .unwrap(),
        "stderr-quota" => stderr
            .write_all(&vec![b'x'; CHILD_PIPE_LIMIT + 4096])
            .unwrap(),
        _ => panic!("unknown owned output fixture mode"),
    }
    stdout.flush().unwrap();
    stderr.flush().unwrap();
    if let Some(path) = std::env::var_os(OUTPUT_FIXTURE_READY) {
        std::fs::write(path, std::process::id().to_string()).unwrap();
    }
    // Finite fallback if the parent guard fails; successful tests kill/reap
    // these deliberately idle children within their original output budget.
    std::thread::sleep(Duration::from_secs(10));
    panic!("owned output fixture exceeded its expected parent cleanup");
}

#[tokio::test]
async fn owned_output_capture_waits_for_exit_and_both_eofs() {
    let result = capture_owned_child(output_fixture("clean"), 100, CancellationToken::new())
        .await
        .unwrap();
    assert!(result.pid.is_some());
    assert!(result.status.success());
    assert!(result.stdout.eof && result.stderr.eof);
    assert!(
        result
            .stdout
            .bytes
            .windows(b"fixture-complete-stdout\n".len())
            .any(|part| part == b"fixture-complete-stdout\n")
    );
    assert_eq!(result.stderr.bytes, b"fixture-complete-stderr\n");
}

#[tokio::test]
async fn owned_output_capture_preserves_partial_timeout_and_reaps() {
    let failure = capture_owned_child(output_fixture("partial"), 101, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(failure.phase, CapturePhase::Deadline);
    assert!(failure.pid.is_some() && failure.reaped && failure.kill_requested);
    assert!(failure.cleanup_issue.is_none(), "{failure:?}");
    assert!(!failure.stdout.eof && !failure.stderr.eof);
    assert!(
        failure
            .stdout
            .bytes
            .windows(22)
            .any(|part| part == b"fixture-partial-stdout")
    );
    assert_eq!(failure.stderr.bytes, b"fixture-partial-stderr");
    assert!(!format!("{failure:?}").contains("fixture-partial"));
}

#[tokio::test]
async fn owned_output_capture_bounds_each_pipe_and_reaps() {
    for (iteration, mode, expected) in [
        (102, "stdout-quota", CapturePhase::StdoutQuota),
        (103, "stderr-quota", CapturePhase::StderrQuota),
    ] {
        let failure =
            capture_owned_child(output_fixture(mode), iteration, CancellationToken::new())
                .await
                .unwrap_err();
        assert_eq!(failure.phase, expected);
        assert!(failure.pid.is_some() && failure.reaped, "{failure:?}");
        assert!(failure.cleanup_issue.is_none(), "{failure:?}");
        let pipe = if mode == "stdout-quota" {
            &failure.stdout
        } else {
            &failure.stderr
        };
        assert_eq!(pipe.observed_bytes, CHILD_PIPE_LIMIT + 1);
        assert_eq!(pipe.bytes.len(), CHILD_PIPE_LIMIT);
        assert!(!pipe.eof);
    }
}

#[tokio::test]
async fn owned_output_capture_explicit_cancel_reaps_a_ready_child() {
    let temporary = std::env::temp_dir().join(format!("keelshell-owned-output-{}", Uuid::new_v4()));
    std::fs::create_dir(&temporary).unwrap();
    let ready = temporary.join("ready-pid");
    let mut command = output_fixture("partial");
    command.env(OUTPUT_FIXTURE_READY, &ready);
    let cancelled = CancellationToken::new();
    let capture = capture_owned_child(command, 104, cancelled.clone());
    let cancel_after_ready = async {
        let observed = tokio::time::timeout(CHILD_OUTPUT_DEADLINE, async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&ready)
                    && let Ok(pid) = text.parse::<u32>()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .ok();
        // Always complete the capture/cleanup future before asserting readiness.
        cancelled.cancel();
        observed
    };
    let (result, ready_pid) = tokio::join!(capture, cancel_after_ready);
    std::fs::remove_dir_all(&temporary).unwrap();
    let failure = result.unwrap_err();
    assert!(
        ready_pid.is_some(),
        "fixture did not become ready: {failure:?}"
    );
    assert_eq!(failure.phase, CapturePhase::Cancelled);
    assert_eq!(failure.pid, ready_pid);
    assert!(failure.reaped && failure.kill_requested, "{failure:?}");
    assert!(failure.cleanup_issue.is_none(), "{failure:?}");
}

#[tokio::test]
async fn owned_output_capture_accepts_exact_quota_followed_by_eof() {
    let result = capture_owned_child(
        output_fixture("stderr-limit-eof"),
        105,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(result.status.success());
    assert!(result.stdout.eof && result.stderr.eof);
    assert_eq!(result.stderr.observed_bytes, CHILD_PIPE_LIMIT);
    assert_eq!(result.stderr.bytes.len(), CHILD_PIPE_LIMIT);
}
