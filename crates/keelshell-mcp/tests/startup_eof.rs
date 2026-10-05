#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

use keelshell_mcp::{KeelShellMcpServer, StdioFailure, serve_stream_with_shutdown};
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Output {
    bytes: Mutex<Vec<u8>>,
    flushes: AtomicUsize,
}

struct CaptureWriter(Arc<Output>);

impl AsyncWrite for CaptureWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.0.bytes.lock().unwrap().extend_from_slice(bytes);
        Poll::Ready(Ok(bytes.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.flushes.fetch_add(1, Ordering::Release);
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

async fn eof_after_first_flush(first_frame: &[u8]) -> (Value, Result<(), StdioFailure>) {
    let (mut input, reader) = tokio::io::duplex(8192);
    let closed = CancellationToken::new();
    let output = Arc::new(Output::default());
    let mut serving = Box::pin(serve_stream_with_shutdown(
        KeelShellMcpServer::default(),
        reader,
        CaptureWriter(output.clone()),
        closed.clone(),
    ));
    input.write_all(first_frame).await.unwrap();

    // Pause the SDK consumer once the independently owned writer has really
    // flushed InitializeResult. The client may close input at this point,
    // before a scheduler next polls the startup future.
    tokio::time::timeout(Duration::from_secs(3), async {
        while output.flushes.load(Ordering::Acquire) == 0 {
            assert!(
                serving
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let response: Value = serde_json::from_slice(&output.bytes.lock().unwrap()).unwrap();
    input.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), closed.cancelled())
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), serving)
        .await
        .unwrap();
    (response, result)
}

#[tokio::test]
async fn eof_after_initialize_response_flush_is_success_even_before_sdk_resume() {
    let (response, result) = eof_after_first_flush(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"startup-eof-test\",\"version\":\"1.0\"}}}\n").await;
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
    assert!(
        result.is_ok(),
        "flushed initialization followed by EOF: {result:?}"
    );
}

#[tokio::test]
async fn eof_after_preinitialize_ping_flush_remains_a_startup_failure() {
    let (response, result) =
        eof_after_first_flush(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n").await;
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"], serde_json::json!({}));
    assert!(matches!(result, Err(StdioFailure::Startup)));
}

#[tokio::test]
async fn eof_after_metadata_error_flush_remains_a_startup_failure() {
    let (response, result) = eof_after_first_flush(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n",
    )
    .await;
    assert_eq!(response["id"], 1);
    assert!(response["error"]["code"].is_number());
    assert!(matches!(result, Err(StdioFailure::Startup)));
}
