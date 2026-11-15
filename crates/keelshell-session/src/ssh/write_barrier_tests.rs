//! Real SSH write barriers under socket backpressure, missing replies and rekey.
#![cfg(test)]

use std::{
    collections::HashMap,
    error::Error,
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use keelshell_core::zmodem::{FrameType, Header, HeaderFormat, encode_header};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{Notify, watch},
    task::JoinHandle,
    time::timeout,
};
use zeroize::Zeroizing;

use super::{SshAuth, SshOptions, SshSession, SshShell, transport::TransportControl};
use crate::{
    SessionError,
    zmodem::{ZmodemController, ZmodemState, ZmodemUiEvent},
};

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;

const IO_BUDGET: Duration = Duration::from_secs(5);
const WRITE_BUDGET: Duration = Duration::from_secs(8);
const NO_RECEIPT: Duration = Duration::from_millis(80);
const BEYOND_FAREWELL_GRACE: Duration = Duration::from_millis(3_100);

#[derive(Default)]
struct GateState {
    held: bool,
    blocked: bool,
    waker: Option<Waker>,
}

#[derive(Default)]
struct WriteGate {
    state: Mutex<GateState>,
    observed: Notify,
}

impl WriteGate {
    fn state(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(|_| panic!("write gate mutex poisoned"))
    }

    fn hold(&self) {
        let mut state = self.state();
        assert!(!state.held, "write gate was already held");
        state.held = true;
        state.blocked = false;
    }

    fn poll_blocked(&self, cx: &Context<'_>) -> bool {
        let mut state = self.state();
        if !state.held {
            return false;
        }
        state.blocked = true;
        // Register under the same lock used by release: release cannot miss a
        // writer between checking the gate and returning Poll::Pending.
        state.waker = Some(cx.waker().clone());
        drop(state);
        self.observed.notify_one();
        true
    }

    fn release(&self) {
        let waker = {
            let mut state = self.state();
            state.held = false;
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    async fn wait_blocked(&self) -> TestResult {
        timeout(IO_BUDGET, async {
            loop {
                let notified = self.observed.notified();
                if self.state().blocked {
                    return;
                }
                // notify_one retains a permit even before this future's first
                // poll; the sticky blocked flag also survives a cancelled wait.
                notified.await;
            }
        })
        .await?;
        Ok(())
    }
}

struct GatedStream {
    stream: TcpStream,
    gate: Arc<WriteGate>,
}

impl AsyncRead for GatedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}

impl AsyncWrite for GatedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !bytes.is_empty() && self.gate.poll_blocked(cx) {
            return Poll::Pending;
        }
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

struct Handler {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    received: watch::Sender<Vec<u8>>,
}

impl server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_password(&mut self, _: &str, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        // These tests send only a few small frames, below the channel queue's
        // capacity. Retain the channel without spawning another consumer task.
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        id: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }

    async fn data(
        &mut self,
        _: ChannelId,
        bytes: &[u8],
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.received
            .send_modify(|received| received.extend_from_slice(bytes));
        Ok(())
    }
}

struct Peer {
    client_gate: Arc<WriteGate>,
    server_gate: Arc<WriteGate>,
    client_control: Arc<TransportControl>,
    server_control: Arc<TransportControl>,
    received: watch::Receiver<Vec<u8>>,
    task: JoinHandle<Result<(), russh::Error>>,
}

impl Peer {
    fn stop(&self) {
        // Shutdown works even while russh's write owner is blocked. Release its
        // registered waker as well, so the background session can observe EOF.
        let _ = self.client_control.abort();
        let _ = self.server_control.abort();
        self.client_gate.release();
        self.server_gate.release();
    }

    async fn finish(&mut self) -> TestResult {
        self.stop();
        // A protocol/I/O error is expected after deliberate socket shutdown.
        // Joining the outer task also awaits russh's actual session task.
        let _ = timeout(IO_BUDGET, &mut self.task).await??;
        Ok(())
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.stop();
        self.task.abort();
    }
}

struct Fixture {
    peer: Peer,
    session: SshSession,
    shell: SshShell,
}

impl Fixture {
    async fn new() -> TestResult<Self> {
        timeout(IO_BUDGET, Self::connect()).await?
    }

    async fn connect() -> TestResult<Self> {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let client_stream = TcpStream::connect(address).await?;
        let (server_stream, _) = listener.accept().await?;
        drop(listener);
        client_stream.set_nodelay(true)?;
        server_stream.set_nodelay(true)?;
        let (client_stream, client_control) = TransportControl::attach(client_stream)?;
        let (server_stream, server_control) = TransportControl::attach(server_stream)?;
        let client_gate = Arc::new(WriteGate::default());
        let server_gate = Arc::new(WriteGate::default());
        let (received_tx, received) = watch::channel(Vec::new());
        let stream = GatedStream {
            stream: server_stream,
            gate: server_gate.clone(),
        };
        let task = tokio::spawn(async move {
            server::run_stream(
                config,
                stream,
                Handler {
                    channels: HashMap::new(),
                    received: received_tx,
                },
            )
            .await?
            .await
        });
        // Construct cleanup ownership before the first cancellable handshake.
        let peer = Peer {
            client_gate,
            server_gate,
            client_control,
            server_control,
            received,
            task,
        };
        let options = SshOptions {
            host: address.ip().to_string(),
            port: address.port(),
            username: "write-barrier-fixture".into(),
            expected_host_key: Some(fingerprint),
            auth: SshAuth::Password(Zeroizing::new("ephemeral-fixture-only".into())),
            proxy: None,
            timeout: WRITE_BUDGET,
        };
        let session = SshSession::authenticate_stream(
            &options,
            GatedStream {
                stream: client_stream,
                gate: peer.client_gate.clone(),
            },
            peer.client_control.clone(),
            None,
        )
        .await?;
        let shell = session.start_shell(24, 80).await?;
        // Drain setup traffic with a real roundtrip. russh 0.65.1 rejects this
        // unknown request with REQUEST_FAILURE; that denial is a peer receipt.
        let reply = session
            .handle
            .send_global_request("keepalive@openssh.com", &[], true)
            .await;
        assert!(
            matches!(reply, Err(russh::Error::RequestDenied)),
            "fixture must generate a real SSH request denial: {reply:?}"
        );
        Ok(Self {
            peer,
            session,
            shell,
        })
    }
}

async fn wait_for_blocked_write<F>(gate: &WriteGate, confirmation: Pin<&mut F>) -> TestResult
where
    F: Future<Output = crate::Result<()>>,
{
    tokio::select! {
        biased;
        result = confirmation => {
            Err(format!("write confirmation returned before the gate blocked: {result:?}").into())
        }
        result = gate.wait_blocked() => result,
    }
}

#[tokio::test]
async fn write_confirmed_keeps_farewell_active_past_three_seconds_of_transport_backpressure()
-> TestResult {
    let mut fixture = Fixture::new().await?;
    let writer = fixture.shell.writer();
    let mut controller = ZmodemController::new(fixture.session.clone(), "barrier fixture");
    let invitation = encode_header(Header::new(FrameType::RequestInit, 0, HeaderFormat::Hex));
    let initial = timeout(IO_BUDGET, controller.feed(&fixture.session, &invitation)).await?;
    let lease = controller.lease_id().ok_or("missing receive lease")?;
    let finish = encode_header(Header::new(FrameType::Finish, 0, HeaderFormat::Hex));
    let final_output = timeout(IO_BUDGET, controller.feed(&fixture.session, &finish)).await?;
    assert!(!initial.writes.is_empty());
    assert!(!final_output.writes.is_empty());
    assert!(final_output.events.is_empty());
    let token = controller
        .protocol_output_token()
        .ok_or("missing final output token")?;
    let initial_bytes = initial.writes.concat();
    let final_bytes = final_output.writes.concat();
    let mut expected = initial_bytes.clone();
    expected.extend_from_slice(&final_bytes);

    fixture.peer.client_gate.hold();
    // Ordinary write succeeds at queue admission with the socket still gated.
    timeout(IO_BUDGET, writer.write(&initial_bytes)).await??;
    fixture.peer.client_gate.wait_blocked().await?;
    assert!(fixture.peer.received.borrow().is_empty());

    let confirmation = writer.write_confirmed(&final_bytes);
    tokio::pin!(confirmation);
    let held_at = Instant::now();
    while held_at.elapsed() < BEYOND_FAREWELL_GRACE {
        let remaining = BEYOND_FAREWELL_GRACE.saturating_sub(held_at.elapsed());
        if let Ok(result) = timeout(remaining.min(NO_RECEIPT), &mut confirmation).await {
            panic!("transport produced a receipt while its socket was blocked: {result:?}");
        }
        let tick = controller.tick();
        assert!(
            tick.events.is_empty(),
            "unconfirmed farewell ended: {tick:?}"
        );
        assert!(tick.writes.is_empty());
        assert!(controller.is_active());
        assert_eq!(controller.lease_id(), Some(lease));
        assert_eq!(controller.protocol_output_token(), Some(token));
        assert!(fixture.peer.received.borrow().is_empty());
    }
    assert!(held_at.elapsed() > Duration::from_secs(3));

    fixture.peer.client_gate.release();
    timeout(IO_BUDGET, &mut confirmation).await??;
    assert_eq!(
        fixture.peer.received.borrow().as_slice(),
        expected.as_slice(),
        "peer must receive all initial and final protocol bytes in order"
    );
    controller.protocol_output_flushed(token);
    let ended = controller.transport_exited(&fixture.session, 0, true);
    assert_eq!(
        ended.events,
        vec![ZmodemUiEvent::EndedWithoutFarewell { id: lease }]
    );
    assert_eq!(controller.state(), ZmodemState::EndedWithoutFarewell);
    assert!(!controller.is_active());
    fixture.peer.finish().await
}

#[tokio::test]
async fn write_confirmed_rejects_disconnect_after_data_without_a_peer_reply() -> TestResult {
    let mut fixture = Fixture::new().await?;
    let writer = fixture.shell.writer();
    fixture.peer.server_gate.hold();
    let confirmation = writer.write_confirmed(b"received before disconnect");
    tokio::pin!(confirmation);
    wait_for_blocked_write(&fixture.peer.server_gate, confirmation.as_mut()).await?;
    assert_eq!(
        fixture.peer.received.borrow().as_slice(),
        b"received before disconnect"
    );
    assert!(timeout(NO_RECEIPT, &mut confirmation).await.is_err());

    // The response packet reached the server's real AsyncWrite, but cannot
    // reach the client. EOF drops the reply sender in the client protocol task.
    fixture.peer.server_control.abort()?;
    fixture.peer.server_gate.release();
    let result = timeout(IO_BUDGET, &mut confirmation).await?;
    assert!(
        result.is_err(),
        "a dropped response must be an error, including send_ping's trap: {result:?}"
    );
    fixture.peer.finish().await
}

#[tokio::test]
async fn write_confirmed_times_out_when_data_arrives_but_the_peer_reply_is_blocked() -> TestResult {
    let mut fixture = Fixture::new().await?;
    let mut writer = fixture.shell.writer();
    // Keep authentication's generous budget; shorten only this writer's real
    // production deadline after the initialized shell has been acknowledged.
    writer.timeout = Duration::from_millis(250);
    fixture.peer.server_gate.hold();
    let confirmation = writer.write_confirmed(b"received without a receipt");
    tokio::pin!(confirmation);
    wait_for_blocked_write(&fixture.peer.server_gate, confirmation.as_mut()).await?;
    assert_eq!(
        fixture.peer.received.borrow().as_slice(),
        b"received without a receipt"
    );
    assert!(timeout(NO_RECEIPT, &mut confirmation).await.is_err());
    let result = timeout(IO_BUDGET, &mut confirmation).await?;
    assert!(
        matches!(result, Err(SessionError::Timeout(_))),
        "data admission and arrival cannot replace the missing roundtrip: {result:?}"
    );
    fixture.peer.finish().await
}

#[tokio::test]
async fn write_confirmed_waits_for_a_preceding_rekey_before_receiving_a_reply() -> TestResult {
    let mut fixture = Fixture::new().await?;
    let writer = fixture.shell.writer();
    fixture.peer.client_gate.hold();
    timeout(IO_BUDGET, fixture.session.handle.rekey_soon()).await??;
    // Observe an actual blocked KEX write before admitting the channel data.
    fixture.peer.client_gate.wait_blocked().await?;
    let confirmation = writer.write_confirmed(b"ordered after rekey");
    tokio::pin!(confirmation);
    assert!(timeout(NO_RECEIPT, &mut confirmation).await.is_err());
    assert!(fixture.peer.received.borrow().is_empty());

    fixture.peer.client_gate.release();
    timeout(IO_BUDGET, &mut confirmation).await??;
    assert_eq!(
        fixture.peer.received.borrow().as_slice(),
        b"ordered after rekey"
    );
    fixture.peer.finish().await
}
