use std::{
    future::Future,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rmcp::{
    RoleServer,
    model::{ClientJsonRpcMessage, ErrorData, ServerJsonRpcMessage},
    transport::{
        Transport,
        async_rw::{JsonRpcMessageCodec, JsonRpcMessageCodecError},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, Encoder},
    sync::CancellationToken,
};

use super::{MAX_PENDING_FRAMES, MAX_REQUEST_BYTES, StdioFailure};

struct OutputJob {
    message: ServerJsonRpcMessage,
    flushed: Option<oneshot::Sender<io::Result<()>>>,
}

/// SDK receive cancellation drops only a queue wait. Parsing and protocol-error
/// output belong to owned I/O tasks, which stop only with this connection.
pub(super) struct MessageTransport {
    incoming: mpsc::Receiver<ClientJsonRpcMessage>,
    outgoing: mpsc::Sender<OutputJob>,
    closed: CancellationToken,
}

pub(super) struct IoTasks {
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    closed: CancellationToken,
    input_ended: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}

impl MessageTransport {
    pub(super) fn new<R, W>(reader: R, writer: W, closed: CancellationToken) -> (Self, IoTasks)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (incoming_tx, incoming) = mpsc::channel(MAX_PENDING_FRAMES);
        let (outgoing, outgoing_rx) = mpsc::channel(MAX_PENDING_FRAMES);
        let input_ended = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let reading = tokio::spawn(read_messages(
            reader,
            incoming_tx,
            outgoing.clone(),
            closed.clone(),
            input_ended.clone(),
            failed.clone(),
        ));
        let writing = tokio::spawn(write_messages(
            writer,
            outgoing_rx,
            closed.clone(),
            failed.clone(),
        ));
        (
            Self {
                incoming,
                outgoing,
                closed: closed.clone(),
            },
            IoTasks {
                reader: Some(reading),
                writer: Some(writing),
                closed,
                input_ended,
                failed,
            },
        )
    }
}

impl Transport<RoleServer> for MessageTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        message: ServerJsonRpcMessage,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        let outgoing = self.outgoing.clone();
        let closed = self.closed.clone();
        async move {
            let (flushed, completion) = oneshot::channel();
            let job = OutputJob {
                message,
                flushed: Some(flushed),
            };
            tokio::select! {
                biased;
                _ = closed.cancelled() => Err(closed_error()),
                result = async {
                    outgoing.send(job).await.map_err(|_| closed_error())?;
                    completion.await.map_err(|_| closed_error())?
                } => result,
            }
        }
    }

    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        self.incoming.recv().await
    }

    async fn close(&mut self) -> io::Result<()> {
        // The authenticated outer transport owns its final EOF frame. Dropping
        // this writer must not add a second underlying shutdown operation.
        self.closed.cancel();
        Ok(())
    }
}

impl Drop for MessageTransport {
    fn drop(&mut self) {
        self.closed.cancel();
    }
}

impl IoTasks {
    pub(super) fn input_ended(&self) -> bool {
        self.input_ended.load(Ordering::Acquire)
    }

    pub(super) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    pub(super) fn stop(&self) {
        self.closed.cancel();
        // Connection shutdown already permits truncating an in-flight frame.
        // Abort before joining so pending I/O cannot consume a second deadline.
        for task in [&self.reader, &self.writer].into_iter().flatten() {
            task.abort();
        }
    }

    pub(super) async fn join(&mut self) -> Result<(), StdioFailure> {
        for task in [&mut self.reader, &mut self.writer].into_iter().flatten() {
            match task.await {
                Ok(()) => {}
                Err(error) if error.is_cancelled() => {}
                Err(_) => return Err(StdioFailure::Runtime),
            }
        }
        self.reader = None;
        self.writer = None;
        Ok(())
    }
}

impl Drop for IoTasks {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn read_messages<R: AsyncRead + Send + Unpin>(
    reader: R,
    incoming: mpsc::Sender<ClientJsonRpcMessage>,
    outgoing: mpsc::Sender<OutputJob>,
    closed: CancellationToken,
    input_ended: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let mut codec =
        JsonRpcMessageCodec::<ClientJsonRpcMessage>::new_with_max_length(MAX_REQUEST_BYTES - 1);
    loop {
        let read = tokio::select! {
            biased;
            _ = closed.cancelled() => return,
            result = reader.read_until(b'\n', &mut line) => result,
        };
        match read {
            Ok(0) => break,
            Ok(_) if !line.ends_with(b"\n") => break,
            Ok(_) => {}
            Err(_) => {
                failed.store(true, Ordering::Release);
                break;
            }
        }
        let mut bytes = BytesMut::from(line.as_slice());
        line.clear();
        match codec.decode(&mut bytes) {
            Ok(Some(message)) => {
                // Never wait behind SDK dispatch or output to notice input EOF.
                if incoming.try_send(message).is_err() {
                    failed.store(true, Ordering::Release);
                    break;
                }
            }
            Ok(None) => {}
            Err(JsonRpcMessageCodecError::Serde(error)) => match error.classify() {
                serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {}
                serde_json::error::Category::Data | serde_json::error::Category::Io => {
                    let job = OutputJob {
                        message: ServerJsonRpcMessage::error(
                            ErrorData::invalid_request("Invalid request", None),
                            None,
                        ),
                        flushed: None,
                    };
                    if outgoing.try_send(job).is_err() {
                        failed.store(true, Ordering::Release);
                        break;
                    }
                }
            },
            Err(_) => {
                failed.store(true, Ordering::Release);
                break;
            }
        }
    }
    input_ended.store(true, Ordering::Release);
    closed.cancel();
}

async fn write_messages<W: AsyncWrite + Send + Unpin>(
    mut writer: W,
    mut outgoing: mpsc::Receiver<OutputJob>,
    closed: CancellationToken,
    failed: Arc<AtomicBool>,
) {
    let mut codec = JsonRpcMessageCodec::<ServerJsonRpcMessage>::default();
    while let Some(job) = tokio::select! {
        biased;
        _ = closed.cancelled() => None,
        result = outgoing.recv() => result,
    } {
        let mut bytes = BytesMut::new();
        let result = tokio::select! {
            biased;
            _ = closed.cancelled() => Err(closed_error()),
            result = async {
                codec.encode(job.message, &mut bytes).map_err(io::Error::from)?;
                writer.write_all(&bytes).await?;
                writer.flush().await
            } => result,
        };
        if let Some(flushed) = job.flushed {
            let _ = flushed.send(result.as_ref().map(|_| ()).map_err(|_| closed_error()));
        }
        if result.is_err() {
            if !closed.is_cancelled() {
                failed.store(true, Ordering::Release);
                closed.cancel();
            }
            return;
        }
    }
}

fn closed_error() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "MCP transport closed")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::{
        pin::Pin,
        sync::{Mutex, atomic::AtomicUsize},
        task::{Context, Poll, Waker},
        time::Duration,
    };

    use rmcp::model::NumberOrString;
    use tokio::{
        io::{DuplexStream, ReadBuf},
        sync::Notify,
    };

    use super::*;
    use crate::transport::{BoundedReader, BoundedWriter, FrameBudget};

    #[derive(Default)]
    struct Observations {
        frames: AtomicUsize,
        pending_after: AtomicUsize,
        changed: Notify,
        dropped: AtomicBool,
    }

    struct ObservedReader {
        inner: DuplexStream,
        observations: Arc<Observations>,
    }

    impl AsyncRead for ObservedReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let before = buf.filled().len();
            let result = Pin::new(&mut self.inner).poll_read(cx, buf);
            if let Poll::Ready(Ok(())) = &result {
                let frames = buf.filled()[before..]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count();
                self.observations.frames.fetch_add(frames, Ordering::AcqRel);
            }
            if result.is_pending() {
                self.observations.pending_after.store(
                    self.observations.frames.load(Ordering::Acquire),
                    Ordering::Release,
                );
                self.observations.changed.notify_one();
            }
            result
        }
    }

    impl Drop for ObservedReader {
        fn drop(&mut self) {
            self.observations.dropped.store(true, Ordering::Release);
            self.observations.changed.notify_one();
        }
    }

    #[derive(Default)]
    struct WriteGate {
        blocked: AtomicBool,
        stop_after_prefix: AtomicBool,
        waker: Mutex<Option<Waker>>,
        bytes: Mutex<Vec<u8>>,
        entered: Notify,
        flushes: AtomicUsize,
        changed: Notify,
        shutdown_calls: AtomicUsize,
        dropped: AtomicBool,
    }

    impl WriteGate {
        fn release(&self) {
            self.blocked.store(false, Ordering::Release);
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake();
            }
        }
    }

    struct ControlledWriter(Arc<WriteGate>);

    impl AsyncWrite for ControlledWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.0.blocked.load(Ordering::Acquire) {
                *self.0.waker.lock().unwrap() = Some(cx.waker().clone());
                self.0.entered.notify_one();
                return Poll::Pending;
            }
            let partial = self.0.stop_after_prefix.swap(false, Ordering::AcqRel);
            let count = if partial {
                bytes.len().min(7)
            } else {
                bytes.len()
            };
            self.0
                .bytes
                .lock()
                .unwrap()
                .extend_from_slice(&bytes[..count]);
            if partial {
                self.0.blocked.store(true, Ordering::Release);
            }
            Poll::Ready(Ok(count))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            self.0.flushes.fetch_add(1, Ordering::AcqRel);
            self.0.changed.notify_one();
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            self.0.shutdown_calls.fetch_add(1, Ordering::AcqRel);
            Poll::Ready(Ok(()))
        }
    }

    impl Drop for ControlledWriter {
        fn drop(&mut self) {
            self.0.dropped.store(true, Ordering::Release);
            self.0.changed.notify_one();
        }
    }

    struct Fixture {
        input: DuplexStream,
        transport: MessageTransport,
        tasks: IoTasks,
        read: Arc<Observations>,
        write: Arc<WriteGate>,
        budget: Arc<FrameBudget>,
        closed: CancellationToken,
    }

    impl Fixture {
        fn new(blocked: bool, partial: bool) -> Self {
            let (input, reader) = tokio::io::duplex(8192);
            let closed = CancellationToken::new();
            let budget = Arc::new(FrameBudget::default());
            let read = Arc::new(Observations::default());
            let write = Arc::new(WriteGate::default());
            write.blocked.store(blocked, Ordering::Release);
            write.stop_after_prefix.store(partial, Ordering::Release);
            let (transport, tasks) = MessageTransport::new(
                BoundedReader {
                    inner: ObservedReader {
                        inner: reader,
                        observations: read.clone(),
                    },
                    line_bytes: 0,
                    failed: false,
                    closed: closed.child_token(),
                    budget: budget.clone(),
                },
                BoundedWriter {
                    inner: ControlledWriter(write.clone()),
                    budget: budget.clone(),
                    pending_flush: 0,
                    closing: Box::pin(closed.clone().cancelled_owned()),
                },
                closed.clone(),
            );
            Self {
                input,
                transport,
                tasks,
                read,
                write,
                budget,
                closed,
            }
        }

        async fn read_waiting_after(&self, frames: usize) {
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let changed = self.read.changed.notified();
                    if self.read.pending_after.load(Ordering::Acquire) >= frames {
                        break;
                    }
                    changed.await;
                }
            })
            .await
            .unwrap();
        }

        async fn flushed(&self, count: usize) {
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let changed = self.write.changed.notified();
                    if self.write.flushes.load(Ordering::Acquire) >= count {
                        break;
                    }
                    changed.await;
                }
            })
            .await
            .unwrap();
        }

        fn cancel_receive(&mut self) {
            let mut receive = Box::pin(self.transport.receive());
            let mut cx = Context::from_waker(Waker::noop());
            assert!(receive.as_mut().poll(&mut cx).is_pending());
            // The losing SDK select branch drops this queue wait while the
            // independently owned error output remains queued or in flight.
        }

        async fn subsequent_ping(&mut self) {
            self.input
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"ping\"}\n")
                .await
                .unwrap();
            let message = tokio::time::timeout(Duration::from_secs(3), self.transport.receive())
                .await
                .unwrap()
                .unwrap();
            let ClientJsonRpcMessage::Request(request) = message else {
                panic!("expected request")
            };
            assert_eq!(request.id, NumberOrString::Number(5));
        }

        async fn stop(&mut self) {
            self.tasks.stop();
            tokio::time::timeout(Duration::from_secs(2), self.tasks.join())
                .await
                .unwrap()
                .unwrap();
            assert!(self.read.dropped.load(Ordering::Acquire));
            assert!(self.write.dropped.load(Ordering::Acquire));
            assert_eq!(self.write.shutdown_calls.load(Ordering::Acquire), 0);
        }
    }

    const MALFORMED: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":9}\n";

    #[tokio::test]
    async fn cancelling_sdk_receive_cannot_drop_an_error_queued_behind_a_previous_send() {
        let mut fixture = Fixture::new(true, false);
        fixture
            .input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n")
            .await
            .unwrap();
        assert!(fixture.transport.receive().await.is_some());
        let old_send = tokio::spawn(fixture.transport.send(ServerJsonRpcMessage::error(
            ErrorData::invalid_params("Invalid params", None),
            Some(NumberOrString::Number(3)),
        )));
        tokio::time::timeout(Duration::from_secs(3), fixture.write.entered.notified())
            .await
            .unwrap();
        fixture.input.write_all(MALFORMED).await.unwrap();
        fixture.read_waiting_after(2).await;
        fixture.cancel_receive();
        fixture.cancel_receive();
        assert_eq!(fixture.budget.pending.load(Ordering::Acquire), 2);
        assert!(!old_send.is_finished(), "send must wait for real flush");
        fixture.write.release();
        tokio::time::timeout(Duration::from_secs(3), old_send)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        fixture.flushed(2).await;
        let bytes = fixture.write.bytes.lock().unwrap().clone();
        let replies: Vec<serde_json::Value> = bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0]["id"], 3);
        assert_eq!(replies[1]["error"]["code"], -32600);
        assert!(replies[1]["id"].is_null());
        assert_eq!(fixture.budget.pending.load(Ordering::Acquire), 0);
        fixture.subsequent_ping().await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn cancelling_sdk_receive_during_partial_error_write_never_restarts_the_frame() {
        let mut fixture = Fixture::new(false, true);
        fixture.input.write_all(MALFORMED).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), fixture.write.entered.notified())
            .await
            .unwrap();
        fixture.read_waiting_after(1).await;
        fixture.cancel_receive();
        assert_eq!(fixture.write.bytes.lock().unwrap().len(), 7);
        assert_eq!(fixture.budget.pending.load(Ordering::Acquire), 1);
        fixture.write.release();
        fixture.flushed(1).await;
        let bytes = fixture.write.bytes.lock().unwrap().clone();
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
        let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reply["error"]["code"], -32600);
        assert_eq!(fixture.budget.pending.load(Ordering::Acquire), 0);
        fixture.subsequent_ping().await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn eof_still_reaches_shutdown_while_malformed_error_output_is_blocked() {
        let mut fixture = Fixture::new(false, true);
        fixture.input.write_all(MALFORMED).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), fixture.write.entered.notified())
            .await
            .unwrap();
        fixture.input.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), fixture.closed.cancelled())
            .await
            .unwrap();
        assert!(fixture.tasks.input_ended());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn full_inbound_queue_does_not_block_input_eof_or_release_unflushed_budget() {
        let mut fixture = Fixture::new(false, false);
        let mut frames = Vec::new();
        for id in 1..=MAX_PENDING_FRAMES {
            frames.extend_from_slice(
                format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"ping\"}}\n").as_bytes(),
            );
        }
        fixture.input.write_all(&frames).await.unwrap();
        fixture.read_waiting_after(MAX_PENDING_FRAMES).await;
        assert_eq!(fixture.transport.incoming.len(), MAX_PENDING_FRAMES);
        assert_eq!(
            fixture.budget.pending.load(Ordering::Acquire),
            MAX_PENDING_FRAMES
        );
        fixture.input.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), fixture.closed.cancelled())
            .await
            .unwrap();
        assert!(!fixture.tasks.failed());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn dropping_io_owner_aborts_both_tasks_without_detaching_them() {
        let fixture = Fixture::new(true, false);
        let Fixture {
            transport,
            tasks,
            read,
            write,
            ..
        } = fixture;
        drop(tasks);
        drop(transport);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !read.dropped.load(Ordering::Acquire) || !write.dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(write.shutdown_calls.load(Ordering::Acquire), 0);
    }
}
