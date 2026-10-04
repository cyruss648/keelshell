use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use rmcp::ServiceExt;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::sync::CancellationToken;

use crate::KeelShellMcpServer;

mod channel;

/// Maximum bytes in one newline-delimited request, including its newline.
/// Exceeding this bound closes the transport instead of accumulating input.
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;

/// Maximum input frames without successfully flushed output frames. Counting
/// notifications and abandoned requests conservatively bounds SDK dispatch and
/// pending output too. They retain a slot until this connection ends.
/// The fixed service emits no unsolicited progress/subscription messages;
/// adding those would require correlating admission to request IDs instead.
pub const MAX_PENDING_FRAMES: usize = 32;

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
    /// EOF/error was observed but complete SDK cleanup exceeded two seconds.
    #[error("MCP shutdown exceeded its deadline")]
    Shutdown,
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
    serve_stream_with_shutdown(server, reader, writer, CancellationToken::new()).await
}

/// Serve an authenticated desktop stream with externally owned cancellation.
/// Cancellation reaches tool futures and explicitly stops the SDK service;
/// cleanup is awaited for at most two seconds. The caller must authenticate
/// transport peers and keep authority in the desktop process.
pub async fn serve_stream_with_shutdown<R, W>(
    server: KeelShellMcpServer,
    reader: R,
    writer: W,
    closed: CancellationToken,
) -> Result<(), StdioFailure>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let budget = Arc::new(FrameBudget::default());
    let server = server.with_connection_lifecycle(closed.clone());
    let (transport, mut io_tasks) = channel::MessageTransport::new(
        BoundedReader {
            inner: reader,
            line_bytes: 0,
            failed: false,
            // The reader actor classifies EOF/failure before publishing it to
            // the shared connection token. A child token prevents startup from
            // mistaking that I/O completion for external revocation.
            closed: closed.child_token(),
            budget: budget.clone(),
        },
        BoundedWriter {
            inner: writer,
            budget: budget.clone(),
            pending_flush: 0,
            closing: Box::pin(closed.clone().cancelled_owned()),
        },
        closed.clone(),
    );
    let startup = tokio::select! {
        biased;
        _ = closed.cancelled() => None,
        result = tokio::time::timeout(Duration::from_secs(10), server.serve(transport)) => {
            Some(result)
        }
    };
    let service = match startup {
        Some(Ok(Ok(service))) => service,
        _ => {
            let result = if startup.is_none() && !io_tasks.input_ended() && !io_tasks.failed() {
                Ok(())
            } else {
                Err(StdioFailure::Startup)
            };
            io_tasks.stop();
            tokio::time::timeout(Duration::from_secs(2), io_tasks.join())
                .await
                .map_err(|_| StdioFailure::Shutdown)??;
            return result;
        }
    };
    let sdk_cancel = service.cancellation_token();
    let mut waiting = Box::pin(service.waiting());
    let completed = tokio::select! {
        result = &mut waiting => Some(result),
        _ = closed.cancelled() => None,
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    io_tasks.stop();
    if completed.is_none() {
        sdk_cancel.cancel();
    }
    tokio::time::timeout_at(deadline, async {
        let sdk_result = match completed {
            Some(result) => result,
            None => waiting.await,
        };
        let io_result = io_tasks.join().await;
        sdk_result.map_err(|_| StdioFailure::Runtime)?;
        io_result
    })
    .await
    .map_err(|_| StdioFailure::Shutdown)??;
    if budget.exhausted.load(Ordering::Acquire) || io_tasks.failed() {
        return Err(StdioFailure::Runtime);
    }
    Ok(())
}

#[derive(Default)]
struct FrameBudget {
    pending: AtomicUsize,
    exhausted: AtomicBool,
}

impl FrameBudget {
    fn admit(&self) -> bool {
        if self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                (pending < MAX_PENDING_FRAMES).then_some(pending + 1)
            })
            .is_err()
        {
            self.exhausted.store(true, Ordering::Release);
            false
        } else {
            true
        }
    }

    fn flushed(&self, frames: usize) {
        let _ = self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                Some(pending.saturating_sub(frames))
            });
    }
}

struct BoundedWriter<W> {
    inner: W,
    budget: Arc<FrameBudget>,
    pending_flush: usize,
    closing: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl<W: AsyncWrite + Unpin> BoundedWriter<W> {
    fn check_closing(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if self.closing.as_mut().poll(cx).is_ready() {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "MCP input closed",
            ))
        } else {
            Ok(())
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for BoundedWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.check_closing(cx) {
            return Poll::Ready(Err(error));
        }
        match Pin::new(&mut self.inner).poll_write(cx, bytes) {
            Poll::Ready(Ok(count)) => {
                self.pending_flush += bytes[..count].iter().filter(|byte| **byte == b'\n').count();
                Poll::Ready(Ok(count))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check_closing(cx) {
            return Poll::Ready(Err(error));
        }
        match Pin::new(&mut self.inner).poll_flush(cx) {
            Poll::Ready(Ok(())) => {
                self.budget.flushed(self.pending_flush);
                self.pending_flush = 0;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check_closing(cx) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct BoundedReader<R> {
    inner: R,
    line_bytes: usize,
    failed: bool,
    closed: CancellationToken,
    budget: Arc<FrameBudget>,
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
                        if !self.budget.admit() {
                            self.failed = true;
                            self.closed.cancel();
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "MCP input frame budget exceeded",
                            )));
                        }
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

    #[test]
    fn frame_budget_does_not_expand_when_backend_completion_precedes_output_flush() {
        let budget = FrameBudget::default();
        for _ in 0..MAX_PENDING_FRAMES {
            assert!(budget.admit());
        }
        assert!(!budget.admit());
        budget.flushed(1);
        assert!(budget.admit());
        assert!(!budget.admit());
    }

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
            budget: Arc::new(FrameBudget::default()),
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
            budget: Arc::new(FrameBudget::default()),
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
