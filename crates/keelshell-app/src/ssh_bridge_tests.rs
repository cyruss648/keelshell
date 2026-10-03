//! Production bridge exercised with real SSH and bounded UI byte queues.
use super::{TransportState, run};
use crate::terminal::TerminalCommand;
use keelshell_session::{SessionEvent, ShellEnd, SshAuth, SshOptions, SshSession};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    error::Error,
    net::Shutdown,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum Script {
    Quiet,
    CloseOnInput,
    OutputBacklog,
}
struct Peer {
    script: Script,
    channels: Vec<Channel<server::Msg>>,
    received: Arc<AtomicUsize>,
    ended: bool,
}
impl server::Handler for Peer {
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
        self.channels.push(channel);
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
        if matches!(self.script, Script::OutputBacklog) {
            for _ in 0..96 {
                session.data(id, vec![b'x'; 128])?;
            }
        }
        Ok(())
    }
    fn adjust_window(&mut self, _: ChannelId, _: u32) -> u32 {
        1
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.received.fetch_add(data.len(), Ordering::Release);
        if matches!(self.script, Script::CloseOnInput) && !self.ended {
            self.ended = true;
            session.data(id, b"tail after blocked input".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.close(id)?;
        }
        Ok(())
    }
}
struct Fixture {
    options: SshOptions,
    socket: Arc<Mutex<Option<std::net::TcpStream>>>,
    received: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(socket) = self.socket.lock()
            && let Some(socket) = &*socket
        {
            let _ = socket.shutdown(Shutdown::Both);
        }
        self.task.abort();
    }
}
async fn serve(script: Script) -> Result<Fixture, Box<dyn Error>> {
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[67; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        window_size: 1024,
        maximum_packet_size: 512,
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let socket = Arc::new(Mutex::new(None));
    let control = socket.clone();
    let received = Arc::new(AtomicUsize::new(0));
    let counter = received.clone();
    let task = tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(stream) = stream.into_std() else {
            return;
        };
        if let Ok(duplicate) = stream.try_clone()
            && let Ok(mut control) = control.lock()
        {
            *control = Some(duplicate);
        }
        let Ok(stream) = tokio::net::TcpStream::from_std(stream) else {
            return;
        };
        if let Ok(running) = server::run_stream(
            config,
            stream,
            Peer {
                script,
                channels: Vec::new(),
                received: counter,
                ended: false,
            },
        )
        .await
        {
            let _ = running.await;
        }
    });
    Ok(Fixture {
        options: SshOptions {
            host: address.ip().to_string(),
            port: address.port(),
            username: "fixture".into(),
            auth: SshAuth::Password(Zeroizing::new("fixture".into())),
            proxy: None,
            expected_host_key: Some(fingerprint),
            timeout: Duration::from_secs(2),
        },
        socket,
        received,
        task,
    })
}
async fn connect(fixture: &Fixture) -> Result<SshSession, Box<dyn Error>> {
    Ok(SshSession::connect(SshOptions {
        host: fixture.options.host.clone(),
        port: fixture.options.port,
        username: "fixture".into(),
        auth: SshAuth::Password(Zeroizing::new("fixture".into())),
        proxy: None,
        expected_host_key: fixture.options.expected_host_key.clone(),
        timeout: Duration::from_secs(2),
    })
    .await?)
}
async fn ready(state: &mut watch::Receiver<TransportState>) -> Result<(), Box<dyn Error>> {
    loop {
        if matches!(*state.borrow_and_update(), TransportState::Ready) {
            return Ok(());
        }
        state.changed().await?;
    }
}

#[tokio::test]
async fn quiet_shell_becomes_ready_before_data_and_cancels_cleanly() -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(6), async {
        let fixture = serve(Script::Quiet).await?;
        let session = connect(&fixture).await?;
        let (_commands, receiver) = mpsc::sync_channel(1);
        let (output, incoming) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let (lifecycle, mut state) = watch::channel(TransportState::Starting);
        let (worker, driver) = tokio::join!(
            run(
                session.clone(),
                receiver,
                output,
                cancelled.clone(),
                lifecycle
            ),
            async {
                ready(&mut state).await?;
                assert!(incoming.try_recv().is_err());
                cancelled.store(true, Ordering::Release);
                Ok::<_, Box<dyn Error>>(())
            }
        );
        driver?;
        worker?;
        assert!(matches!(
            *state.borrow(),
            TransportState::Ended(ShellEnd::Cancelled)
        ));
        session.close().await?;
        Ok::<_, Box<dyn Error>>(())
    })
    .await?
}

#[tokio::test]
async fn blocked_input_preserves_tail_and_exit_when_write_fails() -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(6), async {
        let fixture = serve(Script::CloseOnInput).await?;
        let session = connect(&fixture).await?;
        let (commands, receiver) = mpsc::sync_channel(1);
        let (output, incoming) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let (lifecycle, mut state) = watch::channel(TransportState::Starting);
        let (worker, driver) = tokio::join!(
            run(session.clone(), receiver, output, cancelled, lifecycle),
            async {
                ready(&mut state).await?;
                commands.send(TerminalCommand::Write(vec![b'a'; 8 * 1024 * 1024]))?;
                let mut bytes = Vec::new();
                loop {
                    match incoming.try_recv() {
                        Ok(SessionEvent::Data(data)) => bytes.extend(data),
                        Ok(SessionEvent::Exited { code, success }) => {
                            assert_eq!(code, 0);
                            assert!(success);
                            break;
                        }
                        Ok(SessionEvent::Error(error)) => return Err(error.into()),
                        Err(mpsc::TryRecvError::Disconnected) => {
                            return Err("bridge lost tail".into());
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(Duration::from_millis(2)).await
                        }
                    }
                }
                assert_eq!(bytes, b"tail after blocked input");
                Ok::<_, Box<dyn Error>>(())
            }
        );
        driver?;
        worker?;
        assert!(matches!(
            *state.borrow(),
            TransportState::Ended(ShellEnd::Exited { code: 0 })
        ));
        assert!(fixture.received.load(Ordering::Acquire) <= 1024);
        session.close().await?;
        Ok::<_, Box<dyn Error>>(())
    })
    .await?
}

#[tokio::test]
async fn full_ui_output_queue_does_not_block_cancel_or_owned_cleanup() -> Result<(), Box<dyn Error>>
{
    tokio::time::timeout(Duration::from_secs(6), async {
        let fixture = serve(Script::OutputBacklog).await?;
        let session = connect(&fixture).await?;
        let (_commands, receiver) = mpsc::sync_channel(1);
        let (output, incoming) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let (lifecycle, mut state) = watch::channel(TransportState::Starting);
        let (worker, driver) = tokio::join!(
            run(
                session.clone(),
                receiver,
                output,
                cancelled.clone(),
                lifecycle
            ),
            async {
                ready(&mut state).await?;
                tokio::time::sleep(Duration::from_millis(30)).await;
                cancelled.store(true, Ordering::Release);
                Ok::<_, Box<dyn Error>>(())
            }
        );
        driver?;
        worker?;
        assert!(incoming.try_recv().is_ok());
        assert!(matches!(
            *state.borrow(),
            TransportState::Ended(ShellEnd::Cancelled)
        ));
        session.close().await?;
        Ok::<_, Box<dyn Error>>(())
    })
    .await?
}
