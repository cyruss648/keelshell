#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use keelshell_mcp::*;
use serde_json::{Value, json};
use tokio::{
    io::{
        AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, DuplexStream, Lines, ReadHalf,
        WriteHalf,
    },
    sync::Notify,
    task::JoinHandle,
};
use uuid::Uuid;

fn target() -> SessionIdentity {
    SessionIdentity {
        connection_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
        route_revision: Uuid::from_u128(3),
    }
}

struct DropMarker(Arc<AtomicBool>);
impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct ControlledBackend {
    slow: bool,
    entered: Notify,
    dropped: Arc<AtomicBool>,
}
impl DesktopBackend for ControlledBackend {
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async move {
            request.authorization.check()?;
            let _owned = DropMarker(self.dropped.clone());
            if self.slow {
                self.entered.notify_one();
                return std::future::pending().await;
            }
            match request.operation {
                Operation::ListSessions => Ok(BackendReply::Sessions {
                    sessions: vec![SessionMetadata {
                        target: target(),
                        display_name: "Protocol fixture".into(),
                        selection_ids: Vec::new(),
                        granted_roots: Vec::new(),
                    }],
                }),
                Operation::ProposeCommand { target, .. } => {
                    let proposal = request.proposal.unwrap();
                    Ok(BackendReply::PendingCommand {
                        target,
                        action_id: proposal.id,
                        digest: proposal.digest,
                    })
                }
                _ => Err(McpFailure::NotConnected),
            }
        })
    }
}

struct Client {
    reader: Lines<BufReader<ReadHalf<DuplexStream>>>,
    writer: WriteHalf<DuplexStream>,
    server: JoinHandle<Result<(), StdioFailure>>,
    backpressure_polls: Arc<AtomicUsize>,
}

struct PressureWriter {
    inner: WriteHalf<DuplexStream>,
    polls: Arc<AtomicUsize>,
}
impl AsyncWrite for PressureWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        if result.is_pending() {
            self.polls.fetch_add(1, Ordering::SeqCst);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if result.is_pending() {
            self.polls.fetch_add(1, Ordering::SeqCst);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl Client {
    async fn connect(backend: Arc<dyn DesktopBackend>) -> (Self, PolicyController) {
        Self::connect_with_capacity(backend, 256 * 1024).await
    }

    async fn connect_with_capacity(
        backend: Arc<dyn DesktopBackend>,
        capacity: usize,
    ) -> (Self, PolicyController) {
        let policy = PolicyController::default();
        let grant = SessionGrant::new(
            target(),
            [ToolKind::ListSessions, ToolKind::ProposeCommand],
            vec![],
            [],
        )
        .unwrap();
        policy
            .replace(AccessPolicy::enabled(vec![grant]).unwrap())
            .unwrap();
        let server =
            KeelShellMcpServer::new(backend, policy.clone(), 2, Duration::from_secs(2)).unwrap();
        let (client, server_io) = tokio::io::duplex(capacity);
        let (read, write) = tokio::io::split(server_io);
        let backpressure_polls = Arc::new(AtomicUsize::new(0));
        let server = tokio::spawn(serve_stream(
            server,
            read,
            PressureWriter {
                inner: write,
                polls: backpressure_polls.clone(),
            },
        ));
        let (read, writer) = tokio::io::split(client);
        let mut client = Self {
            reader: BufReader::new(read).lines(),
            writer,
            server,
            backpressure_polls,
        };
        client.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"raw-stream-test","version":"1.0"}}})).await;
        assert_eq!(
            client.read().await["result"]["protocolVersion"],
            "2025-06-18"
        );
        client
            .send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        (client, policy)
    }
    async fn send(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await.unwrap();
    }
    async fn read(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(3), self.reader.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    async fn finish(mut self) {
        self.writer.shutdown().await.unwrap();
        drop(self.writer);
        tokio::time::timeout(Duration::from_secs(3), self.server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn protocol_returns_granted_read_and_review_proposal_then_honors_revocation() {
    let (mut client, policy) = Client::connect(Arc::new(ControlledBackend::default())).await;
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    let response = client.read().await;
    assert_eq!(
        response["result"]["structuredContent"]["sessions"][0]["target"],
        json!(target())
    );
    client.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"keelshell_propose_command","arguments":{"target":target(),"command":"printf reviewed"}}})).await;
    let response = client.read().await;
    assert_eq!(
        response["result"]["structuredContent"]["kind"],
        "pending_command"
    );
    assert_eq!(
        response["result"]["structuredContent"]["digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    policy.disable().unwrap();
    client.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    assert_eq!(
        client.read().await["result"]["structuredContent"]["error"]["code"],
        "DISABLED"
    );
    client.finish().await;
}

#[tokio::test]
async fn protocol_cancel_notification_drops_owned_pending_backend_work() {
    let backend = Arc::new(ControlledBackend {
        slow: true,
        ..ControlledBackend::default()
    });
    let (mut client, _policy) = Client::connect(backend.clone()).await;
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    client.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"stop"}})).await;
    // The official SDK discards a response to a client-abandoned request.
    // Prove owned work is released and the next request remains usable.
    tokio::time::timeout(Duration::from_secs(1), async {
        while !backend.dropped.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    client
        .send(json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}))
        .await;
    let response = client.read().await;
    assert_eq!(response["id"], 3);
    assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 8);
    assert!(backend.dropped.load(Ordering::SeqCst));
    client.finish().await;
}

#[tokio::test]
async fn protocol_revocation_drops_inflight_read_and_returns_no_context() {
    let backend = Arc::new(ControlledBackend {
        slow: true,
        ..ControlledBackend::default()
    });
    let (mut client, policy) = Client::connect(backend.clone()).await;
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    policy.disable().unwrap();
    let response = client.read().await;
    assert_eq!(
        response["result"]["structuredContent"]["error"]["code"],
        "REVOKED"
    );
    assert!(
        response["result"]["structuredContent"]
            .get("sessions")
            .is_none()
    );
    assert!(backend.dropped.load(Ordering::SeqCst));
    client.finish().await;
}

#[tokio::test]
async fn closing_input_releases_owned_backend_before_service_returns() {
    let backend = Arc::new(ControlledBackend {
        slow: true,
        ..ControlledBackend::default()
    });
    let (mut client, _policy) = Client::connect(backend.clone()).await;
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), client.finish())
        .await
        .unwrap();
    assert!(backend.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn eof_interrupts_proven_output_backpressure_and_finishes_sdk_cleanup() {
    let (mut client, _policy) =
        Client::connect_with_capacity(Arc::new(ControlledBackend::default()), 512).await;
    let baseline = client.backpressure_polls.load(Ordering::SeqCst);
    client
        .send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    tokio::time::timeout(Duration::from_secs(1), async {
        while client.backpressure_polls.load(Ordering::SeqCst) == baseline {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), client.finish())
        .await
        .unwrap();
}
