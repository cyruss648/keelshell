use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use rmcp::ServiceExt;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::sync::CancellationToken;

use crate::KeelShellMcpServer;

/// Maximum bytes in one newline-delimited request, including its newline.
/// Exceeding this bound closes the transport instead of accumulating input.
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;

/// Static transport diagnostics. Private protocol bodies and I/O details are
/// never interpolated into stderr or public errors.
#[derive(Debug, Error)]
pub enum StdioFailure {
    /// MCP negotiation failed, input closed, or startup exceeded ten seconds.
    #[error("MCP startup failed or exceeded its deadline")]
    Startup,
    /// The negotiated stdio service did not shut down normally.
    #[error("MCP transport stopped unexpectedly")]
    Runtime,
}

/// Serve MCP on stdin/stdout. Stdout contains SDK JSON-RPC only. The standalone
/// executable has no desktop authority, even if a client asks for one.
pub async fn serve_stdio(server: KeelShellMcpServer) -> Result<(), StdioFailure> {
    serve_stream(server, tokio::io::stdin(), tokio::io::stdout()).await
}

/// Serve a newline-framed byte stream with the same input and startup bounds as
/// stdio. An IPC integrator must separately authenticate peers and bind current
/// desktop policy; a stream alone is not authenticated authority.
pub async fn serve_stream<R, W>(
    server: KeelShellMcpServer,
    reader: R,
    writer: W,
) -> Result<(), StdioFailure>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let closed = CancellationToken::new();
    let server = server.with_connection_lifecycle(closed.clone());
    let transport = (
        BoundedReader {
            inner: reader,
            line_bytes: 0,
            failed: false,
            closed,
        },
        writer,
    );
    let service = tokio::time::timeout(Duration::from_secs(10), server.serve(transport))
        .await
        .map_err(|_| StdioFailure::Startup)?
        .map_err(|_| StdioFailure::Startup)?;
    service.waiting().await.map_err(|_| StdioFailure::Runtime)?;
    Ok(())
}

struct BoundedReader<R> {
    inner: R,
    line_bytes: usize,
    failed: bool,
    closed: CancellationToken,
}

impl<R> Drop for BoundedReader<R> {
    fn drop(&mut self) {
        self.closed.cancel();
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.failed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP input bound exceeded",
            )));
        }
        // AsyncRead must leave the caller's filled length unchanged on error.
        // Inspect a bounded scratch chunk before publishing any bytes.
        let mut scratch = [0_u8; 8192];
        let available = buf.remaining().min(scratch.len());
        if available == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut read = ReadBuf::new(&mut scratch[..available]);
        match Pin::new(&mut self.inner).poll_read(cx, &mut read) {
            Poll::Ready(Ok(())) => {
                if read.filled().is_empty() {
                    self.closed.cancel();
                }
                for byte in read.filled() {
                    self.line_bytes += 1;
                    if self.line_bytes > MAX_REQUEST_BYTES {
                        self.failed = true;
                        self.closed.cancel();
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "MCP input bound exceeded",
                        )));
                    }
                    if *byte == b'\n' {
                        self.line_bytes = 0;
                    }
                }
                buf.put_slice(read.filled());
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => {
                self.closed.cancel();
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn request_bound_includes_newline_and_resets_between_messages() {
        let mut first = vec![b'x'; MAX_REQUEST_BYTES - 1];
        first.push(b'\n');
        first.extend_from_slice(b"next\n");
        let mut reader = BoundedReader {
            inner: std::io::Cursor::new(first.clone()),
            line_bytes: 0,
            failed: false,
            closed: CancellationToken::new(),
        };
        let mut result = Vec::new();
        reader.read_to_end(&mut result).await.unwrap();
        assert_eq!(result, first);
    }

    #[tokio::test]
    async fn overlong_line_fails_closed_and_never_recovers_with_more_input() {
        let mut input = vec![b'x'; MAX_REQUEST_BYTES];
        input.extend_from_slice(b"\nnext\n");
        let mut reader = BoundedReader {
            inner: std::io::Cursor::new(input),
            line_bytes: 0,
            failed: false,
            closed: CancellationToken::new(),
        };
        let mut result = Vec::new();
        assert_eq!(
            reader.read_to_end(&mut result).await.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            reader.read_to_end(&mut result).await.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(result.len() <= MAX_REQUEST_BYTES);
    }
}
