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
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::Notify,
};
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

#[tokio::test]
async fn binary_rejects_partial_or_invalid_environment_without_stdout_fallback() {
    for (address, secret) in [
        (Some("127.0.0.1:1"), None),
        (None, Some("abcd")),
        (Some("0.0.0.0:1"), Some("abcd")),
    ] {
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
        let result = tokio::time::timeout(Duration::from_secs(2), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().trim(),
            "invalid MCP desktop launch configuration"
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
        7
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
