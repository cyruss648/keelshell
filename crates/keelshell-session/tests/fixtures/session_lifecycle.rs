//! Typed lifecycle against actual TCP/SSH packets, including unread byte queues.
use keelshell_session::{
    ConnectionEnd, ConnectionState, SessionEvent, ShellEnd, SshAuth, SshOptions, SshSession,
    SshShell,
};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use std::{
    error::Error,
    future::Future,
    net::Shutdown,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum Mode {
    Quiet,
    Exit(u32),
    Signal,
    Close,
    Eof,
    Backlog,
}
struct Handler {
    inner: super::Fixture,
    mode: Mode,
    received: Arc<AtomicUsize>,
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.inner.auth_password(user, password).await
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.inner
            .channel_open_session(channel, reply, session)
            .await
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
        match self.mode {
            Mode::Quiet => {}
            Mode::Exit(code) => {
                session.data(id, b"final bytes\r\n".to_vec())?;
                session.exit_status_request(id, code)?;
                session.close(id)?;
            }
            Mode::Signal => {
                session.exit_signal_request(
                    id,
                    russh::Sig::TERM,
                    false,
                    "untrusted peer diagnostic",
                    "en",
                )?;
                session.close(id)?;
            }
            Mode::Close => session.close(id)?,
            Mode::Eof => session.eof(id)?,
            Mode::Backlog => {
                for _ in 0..96 {
                    session.data(id, vec![b'x'; 128])?;
                }
                session.exit_status_request(id, 7)?;
                session.close(id)?;
            }
        }
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.received.fetch_add(data.len(), Ordering::Release);
        if matches!(self.mode, Mode::Quiet) {
            session.data(id, data.to_vec())?;
        }
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.inner.exec_request(id, data, session).await
    }
}
struct Server {
    options: SshOptions,
    sockets: Arc<Mutex<Vec<std::net::TcpStream>>>,
    received: Arc<AtomicUsize>,
    disconnect: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Server {
    fn cut(&self) {
        if let Ok(sockets) = self.sockets.lock() {
            for socket in sockets.iter() {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.cut();
        self.task.abort();
    }
}
async fn serve(mode: Mode) -> Result<Server, Box<dyn Error>> {
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(1),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let sockets = Arc::new(Mutex::new(Vec::new()));
    let controls = sockets.clone();
    let received = Arc::new(AtomicUsize::new(0));
    let counts = received.clone();
    let (disconnect, mut closing) = watch::channel(false);
    let task = tokio::spawn(async move {
        let Ok((socket, _)) = listener.accept().await else {
            return;
        };
        let Ok(socket) = socket.into_std() else {
            return;
        };
        if let Ok(duplicate) = socket.try_clone()
            && let Ok(mut sockets) = controls.lock()
        {
            sockets.push(duplicate);
        }
        let Ok(socket) = tokio::net::TcpStream::from_std(socket) else {
            return;
        };
        let handler = Handler {
            inner: super::Fixture::default(),
            mode,
            received: counts,
        };
        if let Ok(mut running) = server::run_stream(config, socket, handler).await {
            tokio::select! {
                _=&mut running=>{},
                _=closing.changed()=> {let _=running.handle().disconnect(russh::Disconnect::ByApplication,"peer text must not enter typed state".into(),"en".into()).await; let _=running.await;},
            }
        }
    });
    Ok(Server {
        options: SshOptions {
            host: address.ip().to_string(),
            port: address.port(),
            username: "fixture".into(),
            auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
            proxy: None,
            expected_host_key: Some(fingerprint),
            timeout: Duration::from_secs(2),
        },
        sockets,
        received,
        disconnect,
        task,
    })
}
async fn connect(server: &Server) -> Result<SshSession, Box<dyn Error>> {
    let options = SshOptions {
        host: server.options.host.clone(),
        port: server.options.port,
        username: server.options.username.clone(),
        auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
        proxy: None,
        expected_host_key: server.options.expected_host_key.clone(),
        timeout: server.options.timeout,
    };
    Ok(SshSession::connect(options).await?)
}
async fn ended(shell: &SshShell) -> Result<ShellEnd, Box<dyn Error>> {
    let mut state = shell.subscribe_completion();
    loop {
        if let Some(reason) = state.borrow_and_update().clone() {
            return Ok(reason);
        }
        state.changed().await?;
    }
}
async fn bounded(
    future: impl Future<Output = Result<(), Box<dyn Error>>>,
) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(8), future).await?
}

#[tokio::test]
async fn exit_status_preserves_tail_bytes_and_keeps_parent_usable() -> Result<(), Box<dyn Error>> {
    bounded(async {
        for code in [0, 17] {
            let server = serve(Mode::Exit(code)).await?;
            let session = connect(&server).await?;
            let mut shell = session.start_shell(24, 80).await?;
            assert_eq!(ended(&shell).await?, ShellEnd::Exited { code });
            let mut bytes = Vec::new();
            loop {
                match shell.recv().await {
                    Some(SessionEvent::Data(data)) => bytes.extend(data),
                    Some(SessionEvent::Exited {
                        code: actual,
                        success,
                    }) => {
                        assert_eq!(actual, code);
                        assert_eq!(success, code == 0);
                        break;
                    }
                    other => return Err(format!("unexpected event {other:?}").into()),
                }
            }
            assert_eq!(bytes, b"final bytes\r\n");
            assert!(!ended(&shell).await?.is_reconnectable());
            shell.close().await?;
            assert_eq!(session.connection_state(), ConnectionState::Connected);
            assert_eq!(session.exec("echo alive").await?.stdout, b"echo alive");
            session.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn signal_and_statusless_close_are_not_transport_loss() -> Result<(), Box<dyn Error>> {
    bounded(async {
        for (mode, expected) in [
            (
                Mode::Signal,
                ShellEnd::Signalled {
                    signal: "TERM".into(),
                },
            ),
            (Mode::Close, ShellEnd::ChannelClosed),
        ] {
            let server = serve(mode).await?;
            let session = connect(&server).await?;
            let shell = session.start_shell(24, 80).await?;
            assert_eq!(ended(&shell).await?, expected);
            assert!(!expected.is_reconnectable());
            assert_eq!(session.connection_state(), ConnectionState::Connected);
            shell.close().await?;
            session.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn eof_is_only_half_close_and_quiet_shell_is_ready() -> Result<(), Box<dyn Error>> {
    bounded(async {
        let server = serve(Mode::Eof).await?;
        let session = connect(&server).await?;
        let shell = session.start_shell(24, 80).await?;
        assert!(
            tokio::time::timeout(Duration::from_millis(40), ended(&shell))
                .await
                .is_err()
        );
        shell.write(b"still writable").await?;
        while server.received.load(Ordering::Acquire) == 0 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_eq!(server.received.load(Ordering::Acquire), 14);
        assert_eq!(shell.completion(), None);
        shell.close().await?;
        assert_eq!(ended(&shell).await?, ShellEnd::Cancelled);
        session.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn tcp_loss_is_typed_before_consuming_legacy_output() -> Result<(), Box<dyn Error>> {
    bounded(async {
        let server = serve(Mode::Quiet).await?;
        let session = connect(&server).await?;
        let shell = session.start_shell(24, 80).await?;
        server.cut();
        assert_eq!(
            ended(&shell).await?,
            ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost)
        );
        assert!(ended(&shell).await?.is_reconnectable());
        assert_eq!(
            session.connection_state(),
            ConnectionState::Closed(ConnectionEnd::TransportLost)
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn explicit_server_disconnect_has_no_untrusted_description() -> Result<(), Box<dyn Error>> {
    bounded(async {
        let server = serve(Mode::Quiet).await?;
        let session = connect(&server).await?;
        let shell = session.start_shell(24, 80).await?;
        server.disconnect.send_replace(true);
        let reason = ended(&shell).await?;
        assert_eq!(
            reason,
            ShellEnd::ConnectionClosed(ConnectionEnd::RemoteDisconnected {
                code: russh::Disconnect::ByApplication as u32
            })
        );
        assert!(!reason.is_reconnectable());
        assert!(!format!("{reason:?}").contains("peer text"));
        Ok(())
    })
    .await
}

#[tokio::test]
async fn local_shell_close_does_not_close_shared_transport_or_sibling() -> Result<(), Box<dyn Error>>
{
    bounded(async {
        let server = serve(Mode::Quiet).await?;
        let session = connect(&server).await?;
        let first = session.start_shell(24, 80).await?;
        let mut second = session.start_shell(24, 80).await?;
        first.close().await?;
        assert_eq!(ended(&first).await?, ShellEnd::Cancelled);
        assert_eq!(session.connection_state(), ConnectionState::Connected);
        second.write(b"sibling").await?;
        assert!(matches!(second.recv().await,Some(SessionEvent::Data(data)) if data==b"sibling"));
        session.close().await?;
        assert_eq!(
            ended(&second).await?,
            ShellEnd::ConnectionClosed(ConnectionEnd::LocalClosed)
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn unread_output_does_not_hide_known_exit_and_cancellation_is_bounded()
-> Result<(), Box<dyn Error>> {
    bounded(async {
        let server = serve(Mode::Backlog).await?;
        let session = connect(&server).await?;
        let shell = session.start_shell(24, 80).await?;
        assert_eq!(ended(&shell).await?, ShellEnd::Exited { code: 7 });
        server.cut();
        assert_eq!(ended(&shell).await?, ShellEnd::Exited { code: 7 });
        let _ = tokio::time::timeout(Duration::from_secs(1), shell.close()).await?;
        assert_eq!(shell.completion(), Some(ShellEnd::Exited { code: 7 }));
        Ok(())
    })
    .await
}
