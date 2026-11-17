//! Local-forward lifecycle regressions using real loopback SSH and TCP peers.

use std::error::Error;
use std::future::{Future, poll_fn};
use std::io;
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, server};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::timeout;
use zeroize::Zeroizing;

use super::{LocalForward, bind_local_forward, local_forward_shutdown};
use crate::{ConnectionEnd, ConnectionState, SessionError, SshAuth, SshOptions, SshSession};

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;

const LIMIT: Duration = Duration::from_secs(5);

struct Peer {
    opened: watch::Sender<usize>,
    open_gate: Arc<Semaphore>,
    relays: JoinSet<()>,
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _: &str, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.opened.send_modify(|count| *count += 1);
        let _permit = self
            .open_gate
            .acquire()
            .await
            .map_err(|_| russh::Error::Disconnect)?;
        let port = u16::try_from(port).map_err(|_| russh::Error::Disconnect)?;
        let mut socket = TcpStream::connect((host, port)).await?;
        reply.accept().await;
        self.relays.spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
        });
        Ok(())
    }
}

struct TestServer {
    address: SocketAddr,
    fingerprint: String,
    sockets: Arc<Mutex<Vec<std::net::TcpStream>>>,
    disconnect: watch::Sender<bool>,
    opened: watch::Sender<usize>,
    open_gate: Arc<Semaphore>,
    task: JoinHandle<io::Result<()>>,
}

impl TestServer {
    async fn start() -> TestResult<Self> {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let sockets = Arc::new(Mutex::new(Vec::new()));
        let controls = sockets.clone();
        let (disconnect, shutdown) = watch::channel(false);
        let (opened, _) = watch::channel(0);
        let observations = opened.clone();
        let open_gate = Arc::new(Semaphore::new(1));
        let gate = open_gate.clone();
        let task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
            let mut peers = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (socket, _) = accepted?;
                        let socket = socket.into_std()?;
                        controls
                            .lock()
                            .map_err(|_| io::Error::other("fixture socket lock poisoned"))?
                            .push(socket.try_clone()?);
                        let socket = TcpStream::from_std(socket)?;
                        let config = config.clone();
                        let mut shutdown = shutdown.clone();
                        let peer = Peer {
                            opened: observations.clone(),
                            open_gate: gate.clone(),
                            relays: JoinSet::new(),
                        };
                        peers.spawn(async move {
                            let mut running = server::run_stream(config, socket, peer).await?;
                            let handle = running.handle();
                            tokio::select! {
                                result = &mut running => result?,
                                _ = shutdown.changed() => {
                                    handle.disconnect(russh::Disconnect::ByApplication, String::new(), "en".into()).await?;
                                    running.await?;
                                },
                            }
                            Ok::<_, russh::Error>(())
                        });
                    },
                    _ = peers.join_next(), if !peers.is_empty() => {},
                }
            }
        });
        Ok(Self {
            address,
            fingerprint,
            sockets,
            disconnect,
            opened,
            open_gate,
            task,
        })
    }

    async fn connect(&self) -> TestResult<SshSession> {
        Ok(timeout(
            LIMIT,
            SshSession::connect(SshOptions {
                host: self.address.ip().to_string(),
                port: self.address.port(),
                username: "fixture".into(),
                expected_host_key: Some(self.fingerprint.clone()),
                auth: SshAuth::Password(Zeroizing::new("ephemeral-fixture".into())),
                proxy: None,
                // Cancellation must release pending opens before this deadline.
                timeout: Duration::from_secs(30),
            }),
        )
        .await??)
    }

    fn cut(&self, connection: usize) -> TestResult {
        let sockets = self.sockets.lock().map_err(|_| "fixture socket lock")?;
        sockets
            .get(connection)
            .ok_or("fixture connection missing")?
            .shutdown(Shutdown::Both)?;
        Ok(())
    }

    fn connection_count(&self) -> TestResult<usize> {
        Ok(self
            .sockets
            .lock()
            .map_err(|_| "fixture socket lock")?
            .len())
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Ok(sockets) = self.sockets.lock() {
            for socket in sockets.iter() {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
        self.task.abort();
    }
}

async fn begin_close(session: &SshSession) -> crate::Result<()> {
    let closing = session.close();
    tokio::pin!(closing);
    // A single poll publishes Closing without scheduling the protocol worker.
    // This deliberately tests the window in which is_closed is still false.
    poll_fn(|cx| {
        Poll::Ready(match closing.as_mut().poll(cx) {
            Poll::Ready(result) => result,
            Poll::Pending => Ok(()),
        })
    })
    .await
}

async fn ended(session: &SshSession) -> TestResult {
    let mut state = session.subscribe_state();
    timeout(LIMIT, async {
        loop {
            if matches!(*state.borrow_and_update(), ConnectionState::Closed(_)) {
                return Ok::<_, watch::error::RecvError>(());
            }
            state.changed().await?;
        }
    })
    .await??;
    Ok(())
}

async fn released(forward: &mut LocalForward) -> TestResult {
    // Awaiting the task has no cancellation side effect. Keep the owner alive.
    timeout(LIMIT, &mut forward.task).await??;
    assert!(forward.is_closed());
    let _rebound = TcpListener::bind(forward.local_addr()).await?;
    Ok(())
}

async fn closed(socket: &mut TcpStream) -> TestResult {
    let mut byte = [0];
    match timeout(LIMIT, socket.read(&mut byte)).await? {
        Ok(0) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::NotConnected
            ) =>
        {
            Ok(())
        }
        other => Err(format!("owned TCP stream was not closed: {other:?}").into()),
    }
}

async fn exchange(client: &mut TcpStream, target: &mut TcpStream, bytes: &[u8]) -> TestResult {
    timeout(LIMIT, async {
        client.write_all(bytes).await?;
        let mut received = vec![0; bytes.len()];
        target.read_exact(&mut received).await?;
        assert_eq!(received.as_slice(), bytes);
        target.write_all(&received).await?;
        client.read_exact(&mut received).await?;
        assert_eq!(received.as_slice(), bytes);
        Ok::<_, io::Error>(())
    })
    .await??;
    Ok(())
}

async fn connected_forward(
    session: &SshSession,
) -> TestResult<(LocalForward, TcpStream, TcpStream)> {
    let target = TcpListener::bind("127.0.0.1:0").await?;
    let forward = session
        .forward_local(
            "127.0.0.1:0".parse()?,
            "127.0.0.1".into(),
            target.local_addr()?.port(),
        )
        .await?;
    let mut client = timeout(LIMIT, TcpStream::connect(forward.local_addr())).await??;
    let (mut remote, _) = timeout(LIMIT, target.accept()).await??;
    exchange(&mut client, &mut remote, b"established\0relay").await?;
    Ok((forward, client, remote))
}

#[tokio::test]
async fn local_forward_rejects_closing_before_binding_occupied_address() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    begin_close(&session).await?;
    assert_eq!(session.connection_state(), ConnectionState::Closing);
    assert!(!session.is_closed());
    assert!(matches!(
        session
            .forward_local(occupied.local_addr()?, "127.0.0.1".into(), 22)
            .await,
        Err(SessionError::Closed)
    ));
    Ok(())
}

#[tokio::test]
async fn local_forward_rejects_closed_before_binding_occupied_address() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    server.cut(0)?;
    ended(&session).await?;
    assert!(matches!(
        session
            .forward_local(occupied.local_addr()?, "127.0.0.1".into(), 22)
            .await,
        Err(SessionError::Closed)
    ));
    Ok(())
}

#[tokio::test]
async fn local_forward_live_bind_failure_preserves_session() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let occupied = TcpListener::bind("127.0.0.1:0").await?;
    assert!(matches!(
        session
            .forward_local(occupied.local_addr()?, "127.0.0.1".into(), 22)
            .await,
        Err(SessionError::Io(error)) if error.kind() == io::ErrorKind::AddrInUse
    ));
    let (forward, mut client, mut target) = connected_forward(&session).await?;
    exchange(&mut client, &mut target, b"after bind failure").await?;
    forward.close().await;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_rechecks_state_when_bind_completes() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let mut bound_address = None;
    let result = bind_local_forward(&session, async {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        bound_address = Some(listener.local_addr()?);
        assert!(begin_close(&session).await.is_ok());
        Ok(listener)
    })
    .await;
    assert!(matches!(result, Err(SessionError::Closed)));
    let _rebound = TcpListener::bind(bound_address.ok_or("bind was not polled")?).await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_cancels_pending_bind_when_session_starts_closing() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (bound, address) = oneshot::channel();
    let binding = bind_local_forward(&session, async {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        assert!(bound.send(listener.local_addr()?).is_ok());
        std::future::pending::<()>().await;
        Ok(listener)
    });
    let closing = async {
        let address = address.await?;
        session.close().await?;
        Ok::<_, Box<dyn Error>>(address)
    };
    let (result, address) = timeout(LIMIT, async { tokio::join!(binding, closing) }).await?;
    assert!(matches!(result, Err(SessionError::Closed)));
    let _rebound = TcpListener::bind(address?).await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_closing_releases_idle_listener_with_owner_alive() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let mut forward = session
        .forward_local("127.0.0.1:0".parse()?, "127.0.0.1".into(), 22)
        .await?;
    begin_close(&session).await?;
    released(&mut forward).await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_remote_disconnect_releases_idle_listener() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let mut forward = session
        .forward_local("127.0.0.1:0".parse()?, "127.0.0.1".into(), 22)
        .await?;
    server.disconnect.send_replace(true);
    ended(&session).await?;
    assert_eq!(
        session.connection_state(),
        ConnectionState::Closed(ConnectionEnd::RemoteDisconnected {
            code: russh::Disconnect::ByApplication as u32,
        })
    );
    released(&mut forward).await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_tcp_loss_releases_listener_and_established_streams() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (mut forward, mut client, mut target) = connected_forward(&session).await?;
    server.cut(0)?;
    ended(&session).await?;
    assert_eq!(
        session.connection_state(),
        ConnectionState::Closed(ConnectionEnd::TransportLost)
    );
    released(&mut forward).await?;
    closed(&mut client).await?;
    closed(&mut target).await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_closing_releases_pending_channel_open_before_its_deadline() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let target = TcpListener::bind("127.0.0.1:0").await?;
    let mut forward = session
        .forward_local(
            "127.0.0.1:0".parse()?,
            "127.0.0.1".into(),
            target.local_addr()?.port(),
        )
        .await?;
    let gate = server.open_gate.acquire().await?;
    let mut opened = server.opened.subscribe();
    let mut client = timeout(LIMIT, TcpStream::connect(forward.local_addr())).await??;
    timeout(LIMIT, async {
        while *opened.borrow_and_update() == 0 {
            opened.changed().await?;
        }
        Ok::<_, watch::error::RecvError>(())
    })
    .await??;
    begin_close(&session).await?;
    released(&mut forward).await?;
    closed(&mut client).await?;
    drop(gate);
    Ok(())
}

#[tokio::test]
async fn local_forward_close_preserves_sibling_forward_on_shared_session() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (first, mut first_client, mut first_target) = connected_forward(&session).await?;
    let (second, mut second_client, mut second_target) = connected_forward(&session).await?;
    let first_address = first.local_addr();
    timeout(LIMIT, first.close()).await?;
    closed(&mut first_client).await?;
    closed(&mut first_target).await?;
    let _rebound = TcpListener::bind(first_address).await?;
    assert_eq!(session.connection_state(), ConnectionState::Connected);
    assert!(!session.is_closed());
    assert!(!second.is_closed());
    exchange(&mut second_client, &mut second_target, b"sibling survives").await?;
    second.close().await;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_ssh_shutdown_preserves_an_independent_session() -> TestResult {
    let server = TestServer::start().await?;
    let first_session = server.connect().await?;
    let second_session = server.connect().await?;
    assert!(!first_session.same_connection(&second_session));
    let (mut first, mut first_client, mut first_target) = connected_forward(&first_session).await?;
    let (second, mut second_client, mut second_target) = connected_forward(&second_session).await?;
    first_session.close().await?;
    released(&mut first).await?;
    closed(&mut first_client).await?;
    closed(&mut first_target).await?;
    assert_eq!(
        second_session.connection_state(),
        ConnectionState::Connected
    );
    assert!(!second_session.is_closed());
    assert!(!second.is_closed());
    exchange(
        &mut second_client,
        &mut second_target,
        b"independent session",
    )
    .await?;
    second.close().await;
    second_session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_new_connection_never_revives_original_listener() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let mut original = session
        .forward_local("127.0.0.1:0".parse()?, "127.0.0.1".into(), 22)
        .await?;
    server.cut(0)?;
    released(&mut original).await?;
    assert_eq!(server.connection_count()?, 1);
    let _rebound = TcpListener::bind(original.local_addr()).await?;
    let replacement = server.connect().await?;
    assert!(!session.same_connection(&replacement));
    let (forward, mut client, mut target) = connected_forward(&replacement).await?;
    exchange(&mut client, &mut target, b"fresh connection only").await?;
    assert!(original.is_closed());
    assert_eq!(server.connection_count()?, 2);
    forward.close().await;
    replacement.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_shutdown_observes_already_seen_terminal_states() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    for terminal in [
        ConnectionState::Closing,
        ConnectionState::Closed(ConnectionEnd::TransportLost),
    ] {
        let (sender, mut state) = watch::channel(terminal);
        assert_eq!(*state.borrow_and_update(), terminal);
        assert!(!session.is_closed());
        timeout(LIMIT, local_forward_shutdown(&session, &mut state)).await?;
        drop(sender);
    }
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_shutdown_exits_when_watch_sender_disappears() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (sender, mut state) = watch::channel(ConnectionState::Connected);
    drop(sender);
    timeout(LIMIT, local_forward_shutdown(&session, &mut state)).await?;
    assert_eq!(session.connection_state(), ConnectionState::Connected);
    assert!(!session.is_closed());
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_shutdown_keeps_waiting_on_connected_updates() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (sender, mut state) = watch::channel(ConnectionState::Connected);
    {
        let stopped = local_forward_shutdown(&session, &mut state);
        tokio::pin!(stopped);
        for _ in 0..3 {
            sender.send_replace(ConnectionState::Connected);
            poll_fn(|cx| {
                assert!(stopped.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
        }
    }
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_shutdown_checks_transport_when_watch_state_is_stale() -> TestResult {
    let server = TestServer::start().await?;
    let session = server.connect().await?;
    let (sender, mut stale_state) = watch::channel(ConnectionState::Connected);
    let stopped = local_forward_shutdown(&session, &mut stale_state);
    tokio::pin!(stopped);
    // Poll once while both signals say Connected, then stop only the real TCP
    // transport. This receiver deliberately never gets a lifecycle notification.
    poll_fn(|cx| {
        assert!(stopped.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    server.cut(0)?;
    timeout(LIMIT, &mut stopped).await?;
    assert_eq!(*sender.borrow(), ConnectionState::Connected);
    Ok(())
}
