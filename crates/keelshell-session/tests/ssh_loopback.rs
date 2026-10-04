//! Protocol tests against an ephemeral loopback russh server.

use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    sftp::{TransferEvent, TransferSpec},
};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

#[path = "fixtures/sftp.rs"]
mod sftp_fixture;

#[path = "fixtures/socks.rs"]
mod socks_tests;

#[path = "fixtures/directory_resume.rs"]
mod directory_resume;
#[path = "fixtures/directory_transfers.rs"]
mod directory_transfers;
#[path = "fixtures/file_resume.rs"]
mod file_resume;

#[path = "fixtures/session_lifecycle.rs"]
mod session_lifecycle;

#[path = "fixtures/session_ownership.rs"]
mod session_ownership;

#[path = "fixtures/jump_server.rs"]
mod jump_fixture;

#[path = "fixtures/jump_transport.rs"]
mod jump_transport;

#[path = "fixtures/upstream_proxy.rs"]
mod upstream_proxy;

#[derive(Default)]
struct Fixture {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    filesystem: sftp_fixture::Filesystem,
    forward_ack_gate: Arc<sftp_fixture::ResponseGate>,
    auth_delay: Arc<AtomicU32>,
    password_authentications: Arc<AtomicU32>,
    bound_port: Arc<AtomicU32>,
    forward_listeners: Arc<AtomicU32>,
    forwards: HashMap<u32, JoinHandle<()>>,
    direct_requests: Arc<Mutex<Vec<(String, u32)>>>,
    direct_delay: Arc<AtomicU32>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in self.forwards.values() {
            task.abort();
        }
    }
}

// The lease moves into the listener future before spawn, so even an abort
// before its first poll records release of the actual owned listener.
struct ForwardListenerLease(Arc<AtomicU32>);
impl Drop for ForwardListenerLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl server::Handler for Fixture {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        let delay = self.auth_delay.load(Ordering::Acquire);
        if delay != 0 {
            tokio::time::sleep(Duration::from_millis(u64::from(delay))).await;
        }
        self.password_authentications.fetch_add(1, Ordering::AcqRel);
        Ok(
            if user == "fixture" && password == "ephemeral-test-password" {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }
    async fn auth_publickey(
        &mut self,
        user: &str,
        _: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let Some(channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        session.channel_success(id)?;
        tokio::spawn(russh_sftp::server::run(
            channel.into_stream(),
            self.filesystem.clone(),
        ));
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
        let Some(mut channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        tokio::spawn(async move {
            // Consume the actual channel queue. Echoing only in Handler::data
            // leaves the channel's bounded queue unread and stalls large tests.
            while let Some(message) = channel.wait().await {
                match message {
                    russh::ChannelMsg::Data { data } => {
                        if channel.data(&data[..]).await.is_err() {
                            break;
                        }
                    }
                    russh::ChannelMsg::Eof | russh::ChannelMsg::Close => {
                        let _ = channel.close().await;
                        break;
                    }
                    _ => {}
                }
            }
        });
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.channels.contains_key(&id) {
            session.data(id, data.to_vec())?;
        }
        Ok(())
    }
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        let listener = TcpListener::bind((address, *port as u16)).await?;
        *port = u32::from(listener.local_addr()?.port());
        let port = *port;
        self.bound_port.store(port, Ordering::Release);
        self.forward_listeners.fetch_add(1, Ordering::AcqRel);
        let listener_lease = ForwardListenerLease(self.forward_listeners.clone());
        let address = address.to_owned();
        let handle = session.handle();
        self.forwards.insert(port, tokio::spawn(async move {
            let _listener_lease = listener_lease;
            let mut streams = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut socket, peer)) = accepted else { break; };
                        let Ok(channel) = handle.channel_open_forwarded_tcpip(&address, port, peer.ip().to_string(), u32::from(peer.port())).await else { break; };
                        streams.spawn(async move { let mut stream = channel.into_stream(); let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await; });
                    },
                    _ = streams.join_next(), if !streams.is_empty() => {},
                }
            }
        }));
        if self.forward_ack_gate.is_armed() {
            self.forward_ack_gate
                .hold()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
        }
        Ok(true)
    }
    async fn cancel_tcpip_forward(
        &mut self,
        _: &str,
        port: u32,
        _: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        if let Some(task) = self.forwards.remove(&port) {
            task.abort();
            let _ = task.await;
        }
        Ok(true)
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, data.to_vec())?;
        session.extended_data(channel, 1, b"fixture-stderr".to_vec())?;
        session.exit_status_request(channel, 7)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
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
        if let Ok(mut requests) = self.direct_requests.lock() {
            requests.push((host.to_owned(), port));
        }
        let delay = self.direct_delay.load(Ordering::Acquire);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(u64::from(delay))).await;
        }
        if host == "blocked.invalid" {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        }
        // This domain is deliberately resolvable only by the SSH test server.
        let host = if host == "remote-only.invalid" {
            "127.0.0.1"
        } else {
            host
        };
        let Ok(mut socket) = TcpStream::connect((host, port as u16)).await else {
            reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
        });
        Ok(())
    }
}

struct Server {
    filesystem: sftp_fixture::Filesystem,
    forward_ack_gate: Arc<sftp_fixture::ResponseGate>,
    auth_delay: Arc<AtomicU32>,
    password_authentications: Arc<AtomicU32>,
    bound_port: Arc<AtomicU32>,
    forward_listeners: Arc<AtomicU32>,
    address: SocketAddr,
    fingerprint: String,
    task: JoinHandle<()>,
    disconnect: tokio::sync::watch::Sender<bool>,
    direct_requests: Arc<Mutex<Vec<(String, u32)>>>,
    direct_delay: Arc<AtomicU32>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.disconnect.send_replace(true);
        self.task.abort();
    }
}

async fn serve() -> Result<Server, Box<dyn Error>> {
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
    let filesystem = sftp_fixture::Filesystem::default();
    let forward_ack_gate = Arc::new(sftp_fixture::ResponseGate::default());
    let auth_delay = Arc::new(AtomicU32::new(0));
    let password_authentications = Arc::new(AtomicU32::new(0));
    let bound_port = Arc::new(AtomicU32::new(0));
    let forward_listeners = Arc::new(AtomicU32::new(0));
    let direct_requests = Arc::new(Mutex::new(Vec::new()));
    let direct_delay = Arc::new(AtomicU32::new(0));
    let shared_requests = direct_requests.clone();
    let shared_direct_delay = direct_delay.clone();
    let shared_fs = filesystem.clone();
    let shared_forward_gate = forward_ack_gate.clone();
    let shared_auth_delay = auth_delay.clone();
    let shared_authentications = password_authentications.clone();
    let shared_port = bound_port.clone();
    let shared_listeners = forward_listeners.clone();
    let (disconnect, shutdown) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((socket, _)) = accepted else { break; };
                    let config = config.clone();
                    let fixture = Fixture { filesystem: shared_fs.clone(), forward_ack_gate: shared_forward_gate.clone(), auth_delay: shared_auth_delay.clone(), password_authentications: shared_authentications.clone(), bound_port: shared_port.clone(), forward_listeners: shared_listeners.clone(), channels: HashMap::new(), forwards: HashMap::new(), direct_requests: shared_requests.clone(), direct_delay: shared_direct_delay.clone() };
                    let mut shutdown = shutdown.clone();
                    clients.spawn(async move {
                        if let Ok(mut session) = server::run_stream(config, socket, fixture).await {
                            let handle = session.handle();
                            tokio::select! {
                                _ = &mut session => {},
                                _ = shutdown.changed() => { let _ = handle.disconnect(russh::Disconnect::ByApplication, "fixture stop".into(), "en".into()).await; },
                            }
                        }
                    });
                },
                _ = clients.join_next(), if !clients.is_empty() => {},
            }
        }
    });
    Ok(Server {
        filesystem,
        forward_ack_gate,
        auth_delay,
        password_authentications,
        bound_port,
        forward_listeners,
        address,
        fingerprint,
        task,
        disconnect,
        direct_requests,
        direct_delay,
    })
}

fn options(server: &Server) -> SshOptions {
    SshOptions {
        host: server.address.ip().to_string(),
        port: server.address.port(),
        username: "fixture".into(),
        proxy: None,
        expected_host_key: Some(server.fingerprint.clone()),
        auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
        timeout: Duration::from_secs(3),
    }
}

async fn connect_after_delayed_authentication(
    server: &Server,
) -> Result<SshSession, Box<dyn Error>> {
    // Timeout tests must reach the initialized operation. A setup deliberately
    // slower than the old 100 ms budget proves they no longer test handshaking.
    let delay = Duration::from_millis(150);
    server.auth_delay.store(150, Ordering::Release);
    let started = std::time::Instant::now();
    let session = SshSession::connect(options(server)).await?;
    assert!(started.elapsed() >= delay);
    assert_eq!(server.password_authentications.load(Ordering::Acquire), 1);
    eprintln!(
        "fixture authentication completed after {:?}; setup and operation budget {:?}",
        started.elapsed(),
        options(server).timeout,
    );
    Ok(session)
}

async fn wait_for_fixture(
    condition: impl Fn() -> bool,
    failure: &'static str,
) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| failure.into())
}

#[tokio::test]
async fn unknown_host_key_is_returned_before_authentication() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut opts = options(&server);
    opts.expected_host_key = None;
    match SshSession::connect(opts).await {
        Err(SessionError::UnknownHostKey { fingerprint }) => {
            assert_eq!(fingerprint, server.fingerprint)
        }
        _ => return Err("unknown host key was not rejected".into()),
    }
    Ok(())
}

#[tokio::test]
async fn changed_host_key_is_rejected_even_with_valid_credentials() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut opts = options(&server);
    opts.expected_host_key = Some("SHA256:previous-identity".into());
    assert!(matches!(
        SshSession::connect(opts).await,
        Err(SessionError::ChangedHostKey { .. })
    ));
    Ok(())
}

#[tokio::test]
async fn password_auth_exec_stderr_and_exit_status_use_real_channels() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let output = session.exec("literal; command payload").await?;
    assert_eq!(output.stdout, b"literal; command payload");
    assert_eq!(output.stderr, b"fixture-stderr");
    assert_eq!(output.exit_status, Some(7));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn rejected_password_and_output_limit_are_explicit_errors() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut opts = options(&server);
    opts.auth = SshAuth::Password(Zeroizing::new("wrong".into()));
    assert!(matches!(
        SshSession::connect(opts).await,
        Err(SessionError::Authentication)
    ));
    let session = SshSession::connect(options(&server)).await?;
    assert!(matches!(
        session.exec_limited("larger than limit", 4).await,
        Err(SessionError::OutputLimit(4))
    ));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn local_forward_transports_bytes_and_closes_listener() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let echo = TcpListener::bind("127.0.0.1:0").await?;
    let echo_address = echo.local_addr()?;
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await?;
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).await?;
        stream.write_all(&bytes).await?;
        Ok::<_, std::io::Error>(())
    });
    let forward = session
        .forward_local(
            "127.0.0.1:0".parse()?,
            "127.0.0.1".into(),
            echo_address.port(),
        )
        .await?;
    let address = forward.local_addr();
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(b"PING").await?;
    let mut bytes = [0; 4];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut bytes)).await??;
    assert_eq!(&bytes, b"PING");
    forward.close().await;
    tokio::task::yield_now().await;
    assert!(TcpStream::connect(address).await.is_err());
    tokio::time::timeout(Duration::from_secs(3), echo_task).await???;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn interactive_shell_transports_bytes_and_resize() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let mut shell = session.start_shell(24, 80).await?;
    shell.resize(30, 100).await?;
    shell.write("中文终端\r".as_bytes()).await?;
    let event = tokio::time::timeout(Duration::from_secs(3), shell.recv()).await?;
    assert_eq!(
        event,
        Some(keelshell_session::SessionEvent::Data(
            "中文终端\r".as_bytes().to_vec()
        ))
    );
    shell.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn interactive_shell_drains_output_during_saturated_input() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let mut shell = session.start_shell(24, 80).await?;
    let writer = shell.writer();
    let content = vec![0x5a; 8 * 1024 * 1024];
    let sending = writer.write(&content);
    let receiving = async {
        let mut count = 0;
        while count < content.len() {
            match shell.recv().await {
                Some(keelshell_session::SessionEvent::Data(bytes)) => {
                    assert!(bytes.iter().all(|&byte| byte == 0x5a));
                    count += bytes.len();
                }
                other => return Err(format!("unexpected event during echo: {other:?}")),
            }
        }
        Ok::<_, String>(count)
    };
    let (sent, received) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(sending, receiving)
    })
    .await?;
    sent?;
    assert_eq!(received?, content.len());
    shell.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_packets_cover_crud_limits_and_streaming_transfer() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/fixture").await?;
    sftp.write("/fixture/data.bin", b"binary\0payload").await?;
    assert_eq!(
        sftp.read("/fixture/data.bin", 1024).await?,
        b"binary\0payload"
    );
    assert!(matches!(
        sftp.read("/fixture/data.bin", 2).await,
        Err(SessionError::OutputLimit(2))
    ));
    let entries = sftp.list("/fixture").await?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "data.bin");
    sftp.rename("/fixture/data.bin", "/fixture/renamed.bin")
        .await?;
    let directory = tempfile::tempdir()?;
    let local = directory.path().join("download.bin");
    assert_eq!(sftp.download("/fixture/renamed.bin", &local).await?, 14);
    assert_eq!(tokio::fs::read(&local).await?, b"binary\0payload");
    sftp.upload(&local, "/fixture/upload.bin").await?;
    assert_eq!(
        sftp.read("/fixture/upload.bin", 1024).await?,
        b"binary\0payload"
    );
    assert!(sftp.download("/fixture/renamed.bin", &local).await.is_err());
    sftp.remove("/fixture/renamed.bin").await?;
    sftp.remove("/fixture/upload.bin").await?;
    sftp.rmdir("/fixture").await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_tree_snapshot_is_sorted_bounded_and_depth_explicit() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/tree").await?;
    sftp.mkdir("/tree/nested").await?;
    sftp.write("/tree/z.txt", b"z").await?;
    sftp.write("/tree/nested/a.txt", b"a").await?;

    let snapshot = sftp.snapshot_tree_limited("/tree", 8, 2).await?;
    let paths = snapshot
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["/tree/nested", "/tree/nested/a.txt", "/tree/z.txt"]);
    assert!(snapshot.iter().all(|entry| !entry.is_symlink));
    assert!(matches!(
        sftp.snapshot_tree_limited("/tree", 8, 0).await,
        Err(SessionError::Invalid(
            "remote snapshot exceeds the requested depth"
        ))
    ));
    assert!(matches!(
        sftp.snapshot_tree_limited("/tree", 2, 2).await,
        Err(SessionError::EntryLimit(2))
    ));
    assert!(matches!(
        sftp.snapshot_tree_limited("/tree", 0, 2).await,
        Err(SessionError::Invalid("invalid remote snapshot limits"))
    ));
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_transfer_queue_reports_progress_and_cancels_pending_work()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = Arc::new(session.sftp().await?);
    let queue = sftp.clone().transfer_queue();
    let directory = tempfile::tempdir()?;
    let local = directory.path().join("queue-upload.bin");
    let content = vec![0x5a; 128 * 1024];
    tokio::fs::write(&local, &content).await?;

    // The fixture deliberately delays the second remote write so the second
    // queue entry is guaranteed to remain pending when it is cancelled.
    server.filesystem.set_atomic_write_stall(true);
    let mut upload = queue
        .enqueue(TransferSpec::upload(&local, "/queue.keelshell-stall"))
        .await?;
    assert_eq!(
        upload.recv().await,
        Some(TransferEvent::Queued { id: upload.id() })
    );
    assert_eq!(
        upload.recv().await,
        Some(TransferEvent::Started {
            id: upload.id(),
            total: Some(content.len() as u64),
        })
    );

    let mut cancelled = queue
        .enqueue(TransferSpec::upload(&local, "/queue-cancel.bin"))
        .await?;
    cancelled.cancel();
    let mut progress_events = 0;
    let upload_terminal = loop {
        match upload.recv().await {
            Some(TransferEvent::Progress { .. }) => progress_events += 1,
            Some(event @ TransferEvent::Completed { .. }) => break event,
            Some(other) => return Err(format!("unexpected upload event: {other:?}").into()),
            None => return Err("upload event stream closed".into()),
        }
    };
    assert!(progress_events >= 2, "expected chunk progress events");
    assert_eq!(
        upload_terminal,
        TransferEvent::Completed {
            id: upload.id(),
            bytes: content.len() as u64,
        }
    );
    let mut cancelled_events = Vec::new();
    while let Some(event) = cancelled.recv().await {
        let terminal = matches!(event, TransferEvent::Cancelled { .. });
        cancelled_events.push(event);
        if terminal {
            break;
        }
    }
    assert_eq!(
        cancelled_events,
        vec![
            TransferEvent::Queued { id: cancelled.id() },
            TransferEvent::Cancelled {
                id: cancelled.id(),
                bytes: 0,
            },
        ]
    );

    // Downloads report the remote stat size and use create_new for the local
    // destination, keeping an interrupted file visible for an explicit retry.
    sftp.write("/queue-download.bin", &content).await?;
    let downloaded = directory.path().join("queue-download.bin");
    let mut download = queue
        .enqueue(TransferSpec::download("/queue-download.bin", &downloaded))
        .await?;
    let mut saw_progress = false;
    let download_terminal = loop {
        match download.recv().await {
            Some(TransferEvent::Progress { .. }) => saw_progress = true,
            Some(event @ TransferEvent::Completed { .. }) => break event,
            Some(TransferEvent::Queued { .. } | TransferEvent::Started { .. }) => {}
            Some(other) => return Err(format!("unexpected download event: {other:?}").into()),
            None => return Err("download event stream closed".into()),
        }
    };
    assert!(saw_progress);
    assert_eq!(
        download_terminal,
        TransferEvent::Completed {
            id: download.id(),
            bytes: content.len() as u64,
        }
    );
    assert_eq!(tokio::fs::read(&downloaded).await?, content);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_atomic_replace_and_upload_publish_complete_content() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/atomic.bin", b"original").await?;
    let content = vec![b'x'; 96 * 1024];
    sftp.write_atomic("/atomic.bin", &content).await?;
    assert_eq!(sftp.read("/atomic.bin", content.len()).await?, content);
    let directory = tempfile::tempdir()?;
    let local = directory.path().join("new.bin");
    tokio::fs::write(&local, b"uploaded\0complete").await?;
    assert_eq!(sftp.upload_atomic(&local, "/atomic.bin").await?, 17);
    assert_eq!(sftp.read("/atomic.bin", 64).await?, b"uploaded\0complete");
    let entries = sftp.list("/").await?;
    assert_eq!(
        entries.len(),
        1,
        "successful replace must not leave staging files"
    );
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_atomic_capability_and_failed_write_preserve_original() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/atomic.bin", b"original").await?;
    server.filesystem.set_atomic_unsupported(true);
    assert!(matches!(
        sftp.write_atomic("/atomic.bin", b"new").await,
        Err(SessionError::Unsupported(_))
    ));
    assert_eq!(sftp.read("/atomic.bin", 64).await?, b"original");
    assert_eq!(
        sftp.list("/").await?.len(),
        1,
        "unsupported capability must not create temporary files"
    );
    server.filesystem.set_atomic_unsupported(false);
    server.filesystem.set_atomic_write_failure(true);
    assert!(
        sftp.write_atomic("/atomic.bin", &vec![1; 96 * 1024])
            .await
            .is_err()
    );
    assert!(
        server.filesystem.atomic_writes_started() >= 2,
        "must fail after a real temporary-file prefix was written"
    );
    sftp.close().await?;
    let verification = session.sftp().await?;
    assert_eq!(verification.read("/atomic.bin", 64).await?, b"original");
    assert_eq!(
        verification.list("/").await?.len(),
        1,
        "cleanup barrier must remove temporary files"
    );
    verification.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_atomic_cancellation_preserves_original_and_cleans_staging()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/atomic.bin", b"original").await?;
    server.filesystem.set_atomic_write_stall(true);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            sftp.write_atomic("/atomic.bin", &vec![1; 96 * 1024])
        )
        .await
        .is_err()
    );
    assert!(server.filesystem.atomic_writes_started() >= 2);
    sftp.close().await?;
    let verification = session.sftp().await?;
    assert_eq!(verification.read("/atomic.bin", 64).await?, b"original");
    assert_eq!(verification.list("/").await?.len(), 1);
    verification.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_atomic_transport_disconnect_preserves_original() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/atomic.bin", b"original").await?;
    server.filesystem.set_atomic_write_stall(true);
    let disconnect = session.clone();
    let interrupted = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        disconnect.close().await
    };
    let content = vec![1; 96 * 1024];
    let (written, closed) = tokio::join!(sftp.write_atomic("/atomic.bin", &content), interrupted);
    closed?;
    assert!(written.is_err());
    assert!(server.filesystem.atomic_writes_started() >= 2);
    let _ = sftp.close().await;
    let reconnected = SshSession::connect(options(&server)).await?;
    let verification = reconnected.sftp().await?;
    assert_eq!(verification.read("/atomic.bin", 64).await?, b"original");
    verification.close().await?;
    reconnected.close().await?;
    Ok(())
}

#[tokio::test]
async fn remote_forward_transports_bytes_and_is_cancelled() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let echo = TcpListener::bind("127.0.0.1:0").await?;
    let echo_address = echo.local_addr()?;
    let echo_task = tokio::spawn(async move {
        let (mut stream, _) = echo.accept().await?;
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).await?;
        stream.write_all(&bytes).await?;
        Ok::<_, std::io::Error>(())
    });
    let forward = session
        .forward_remote("127.0.0.1".into(), 0, echo_address)
        .await?;
    let address = format!("127.0.0.1:{}", forward.remote_port());
    let mut stream = TcpStream::connect(&address).await?;
    stream.write_all(b"PONG").await?;
    let mut bytes = [0; 4];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut bytes)).await??;
    assert_eq!(&bytes, b"PONG");
    forward.close().await?;
    assert!(TcpStream::connect(&address).await.is_err());
    tokio::time::timeout(Duration::from_secs(3), echo_task).await???;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_limit_failure_and_timeout_release_remote_handles() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let session = connect_after_delayed_authentication(&server).await?;
    let sftp = session.sftp().await?;
    sftp.write("/data", b"x").await?;
    assert!(matches!(
        sftp.list_limited("/", 0).await,
        Err(SessionError::EntryLimit(0))
    ));
    wait_for_fixture(
        || server.filesystem.active_directory_handles() == 0,
        "directory handle leaked after entry limit",
    )
    .await?;
    assert!(matches!(
        sftp.list("/error").await,
        Err(SessionError::Sftp(_))
    ));
    wait_for_fixture(
        || server.filesystem.active_directory_handles() == 0,
        "directory handle leaked after server error",
    )
    .await?;
    server.filesystem.stall_directory_reads();
    let started = std::time::Instant::now();
    let (listed, observed) = tokio::join!(sftp.list("/stall"), async {
        wait_for_fixture(
            || server.filesystem.directory_reads_stalled() == 1,
            "list never reached the gated SFTP READDIR",
        )
        .await?;
        assert_eq!(server.filesystem.active_directory_handles(), 1);
        eprintln!("fixture SFTP READDIR entered with one allocated directory handle");
        Ok::<_, Box<dyn Error>>(())
    });
    // Release only after the operation has returned: a load-sensitive fixed
    // delay could acknowledge the request before its timeout ever occurs.
    server.filesystem.release_directory_reads();
    observed?;
    assert!(matches!(listed, Err(SessionError::Timeout("SFTP list"))));
    wait_for_fixture(
        || server.filesystem.active_directory_handles() == 0,
        "directory handle leaked after cancellation",
    )
    .await?;
    eprintln!(
        "fixture SFTP list timed out after {:?}; directory handles cleaned",
        started.elapsed(),
    );
    // A list timeout closes its dedicated subsystem, not the shared SSH link.
    assert_eq!(sftp.list("/").await?.len(), 1);
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn set_permissions_changes_only_the_remote_mode_bits() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/mode.txt", b"content").await?;
    let before = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path == "/mode.txt")
        .ok_or("mode fixture missing")?;
    assert_eq!(before.size, Some(7));
    assert_eq!(before.permissions.map(|mode| mode & 0o7777), Some(0o644));

    let updated = sftp.set_permissions_reviewed(&before, 0o640).await?;
    assert_eq!(updated.permissions.map(|mode| mode & 0o7777), Some(0o640));
    let after = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path == "/mode.txt")
        .ok_or("mode fixture missing after update")?;
    assert_eq!(after.size, Some(7));
    assert_eq!(after.permissions.map(|mode| mode & 0o7777), Some(0o640));
    assert!(matches!(
        sftp.set_permissions_reviewed(&after, 0o10_000).await,
        Err(SessionError::Invalid(_))
    ));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_permissions_reject_stale_targets_links_and_special_parents()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.write("/stale.txt", b"content").await?;
    let stale = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path == "/stale.txt")
        .ok_or("stale fixture missing")?;
    sftp.set_permissions_reviewed(&stale, 0o640).await?;
    assert!(matches!(
        sftp.set_permissions_reviewed(&stale, 0o600).await,
        Err(SessionError::UnverifiedMutation(_))
    ));

    server.filesystem.insert_symlink("/link")?;
    let link = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path == "/link")
        .ok_or("link fixture missing")?;
    assert!(link.is_symlink);
    assert!(matches!(
        sftp.set_permissions_reviewed(&link, 0o600).await,
        Err(SessionError::UnverifiedMutation(_))
    ));

    server.filesystem.insert_symlink("/parent")?;
    let nested = keelshell_session::sftp::RemoteEntry {
        name: "child".into(),
        path: "/parent/child".into(),
        size: Some(1),
        is_directory: false,
        is_symlink: false,
        permissions: Some(0o100644),
        modified: None,
    };
    assert!(matches!(
        sftp.set_permissions_reviewed(&nested, 0o600).await,
        Err(SessionError::Invalid(_))
    ));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn reviewed_permissions_support_a_directory_and_return_fresh_metadata()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/folder").await?;
    let folder = sftp
        .list("/")
        .await?
        .into_iter()
        .find(|entry| entry.path == "/folder")
        .ok_or("directory fixture missing")?;
    assert!(folder.is_directory);
    let updated = sftp.set_permissions_reviewed(&folder, 0o750).await?;
    assert!(updated.is_directory);
    assert_eq!(updated.permissions.map(|mode| mode & 0o7777), Some(0o750));
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn unknown_remote_forward_allocation_disconnects_and_releases_listener()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = connect_after_delayed_authentication(&server).await?;
    server.forward_ack_gate.arm();
    let started = std::time::Instant::now();
    let (forwarded, observed) = tokio::join!(
        session.forward_remote("127.0.0.1".into(), 0, "127.0.0.1:9".parse()?),
        async {
            wait_for_fixture(
                || server.forward_ack_gate.entered() == 1,
                "forward never reached the gated allocation acknowledgement",
            )
            .await?;
            let port = server.bound_port.load(Ordering::Acquire);
            assert_ne!(port, 0, "fixture bound before withholding acknowledgement");
            assert_eq!(server.forward_listeners.load(Ordering::Acquire), 1);
            // Binding a second listener verifies allocation without accepting
            // a probe stream that could itself terminate the fixture listener.
            let probe = tokio::time::timeout(
                Duration::from_secs(1),
                TcpListener::bind(("127.0.0.1", port as u16)),
            )
            .await?;
            assert!(matches!(probe, Err(error) if error.kind() == std::io::ErrorKind::AddrInUse));
            eprintln!("fixture remote forward entered with one bound allocated listener");
            Ok::<_, Box<dyn Error>>(())
        }
    );
    server.forward_ack_gate.release();
    observed?;
    assert!(matches!(
        forwarded,
        Err(SessionError::Timeout("remote forward"))
    ));
    let port = server.bound_port.load(Ordering::Acquire);
    wait_for_fixture(
        || session.is_closed() && server.forward_listeners.load(Ordering::Acquire) == 0,
        "uncertain allocation did not close SSH and release its owned listener",
    )
    .await?;
    // Rebinding proves OS port release without depending on how quickly a
    // closed-port connection reports refusal on each platform.
    let released_address = SocketAddr::from(([127, 0, 0, 1], port as u16));
    let rebound =
        tokio::time::timeout(Duration::from_secs(1), TcpListener::bind(released_address)).await??;
    assert_eq!(rebound.local_addr()?, released_address);
    drop(rebound);
    assert!(session.exec("must no longer run").await.is_err());
    eprintln!(
        "fixture remote forward timed out after {:?}; exact endpoint rebound and SSH closed",
        started.elapsed(),
    );
    Ok(())
}

#[tokio::test]
async fn private_key_authentication_signs_with_an_ephemeral_file() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("id_ed25519");
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    key.write_openssh_file(&path, russh::keys::ssh_key::LineEnding::LF)?;
    let mut opts = options(&server);
    opts.auth = SshAuth::PrivateKey {
        path,
        passphrase: None,
    };
    let session = SshSession::connect(opts).await?;
    assert_eq!(
        session.exec("key-authenticated").await?.stdout,
        b"key-authenticated"
    );
    session.close().await?;
    Ok(())
}
