//! Delayed real SSH session opens and SFTP initialization ownership.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use keelshell_session::{SessionError, SshAuth, SshOptions, SshSession};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use tokio::{net::TcpListener, task::JoinHandle};
use zeroize::Zeroizing;

#[derive(Default)]
struct Observed {
    next_delay: AtomicUsize,
    init_delay: AtomicUsize,
    init_started: AtomicBool,
    init_finished: AtomicBool,
    opened: Mutex<Vec<ChannelId>>,
    closed: Mutex<HashSet<ChannelId>>,
    disconnected: AtomicBool,
    stall_next_sftp: AtomicBool,
    stalled_write: AtomicBool,
    stalled_bytes: AtomicUsize,
}

struct Delayed {
    inner: super::Fixture,
    observed: Arc<Observed>,
    tasks: Vec<JoinHandle<()>>,
    unconfirmed: Vec<server::ChannelOpenHandle>,
    stalled_channels: HashMap<ChannelId, JoinHandle<()>>,
    stalled_packets: HashMap<ChannelId, PacketMonitor>,
}
#[derive(Default)]
struct PacketMonitor {
    header: Vec<u8>,
    remaining: usize,
}
impl PacketMonitor {
    fn observe(&mut self, mut bytes: &[u8], observed: &Observed) {
        // Observe real WRITE framing even when SSH's exhausted window prevents
        // the second bounded request from reaching the decoder in full.
        while !bytes.is_empty() {
            if self.remaining != 0 {
                let consumed = self.remaining.min(bytes.len());
                self.remaining -= consumed;
                bytes = &bytes[consumed..];
            } else {
                let consumed = (5 - self.header.len()).min(bytes.len());
                self.header.extend_from_slice(&bytes[..consumed]);
                bytes = &bytes[consumed..];
                if self.header.len() == 5 {
                    let length = u32::from_be_bytes([
                        self.header[0],
                        self.header[1],
                        self.header[2],
                        self.header[3],
                    ]) as usize;
                    if self.header[4] == 6 {
                        observed.stalled_write.store(true, Ordering::Release);
                    }
                    self.remaining = length.saturating_sub(1);
                    self.header.clear();
                }
            }
        }
    }
}
impl Drop for Delayed {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        for task in self.stalled_channels.values() {
            task.abort();
        }
    }
}
impl server::Handler for Delayed {
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
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Ok(mut opened) = self.observed.opened.lock() {
            opened.push(channel.id());
        }
        self.inner.channels.insert(channel.id(), channel);
        match self.observed.next_delay.swap(0, Ordering::AcqRel) {
            0 => reply.accept().await,
            usize::MAX => self.unconfirmed.push(reply),
            milliseconds => self.tasks.push(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(milliseconds as u64)).await;
                reply.accept().await;
            })),
        }
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Ok(mut closed) = self.observed.closed.lock() {
            closed.insert(id);
        }
        self.inner.channels.remove(&id);
        if let Some(task) = self.stalled_channels.remove(&id) {
            task.abort();
        }
        Ok(())
    }
    fn adjust_window(&mut self, id: ChannelId, current: u32) -> u32 {
        // A target of one exhausts the remaining admitted bytes without another
        // adjustment (the protocol's refill threshold becomes zero). Other SSH
        // messages still run, unlike blocking the entire server handler.
        if self.stalled_channels.contains_key(&id) {
            1
        } else {
            current
        }
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.stalled_channels.contains_key(&id) {
            self.observed
                .stalled_bytes
                .fetch_add(data.len(), Ordering::Release);
            self.stalled_packets
                .entry(id)
                .or_default()
                .observe(data, &self.observed);
        }
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
        self.inner.shell_request(id, session).await
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.inner.exec_request(id, data, session).await
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" && self.observed.stall_next_sftp.swap(false, Ordering::AcqRel) {
            let channel = self
                .inner
                .channels
                .remove(&id)
                .ok_or(russh::Error::Disconnect)?;
            session.channel_success(id)?;
            let observed = self.observed.clone();
            self.stalled_channels.insert(
                id,
                tokio::spawn(async move {
                    let _ = russh_sftp::server::run(channel.into_stream(), StalledWrites(observed))
                        .await;
                }),
            );
            return Ok(());
        }
        let delay = self.observed.init_delay.swap(0, Ordering::AcqRel);
        if delay == 0 {
            return self.inner.subsystem_request(id, name, session).await;
        }
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let channel = self
            .inner
            .channels
            .remove(&id)
            .ok_or(russh::Error::Disconnect)?;
        session.channel_success(id)?;
        let observed = self.observed.clone();
        self.tasks.push(tokio::spawn(async move {
            let _ =
                russh_sftp::server::run(channel.into_stream(), DelayedVersion { delay, observed })
                    .await;
        }));
        Ok(())
    }
}

struct DelayedVersion {
    delay: usize,
    observed: Arc<Observed>,
}
impl russh_sftp::server::Handler for DelayedVersion {
    type Error = russh_sftp::protocol::StatusCode;
    fn unimplemented(&self) -> Self::Error {
        Self::Error::OpUnsupported
    }
    async fn init(
        &mut self,
        _: u32,
        _: HashMap<String, String>,
    ) -> Result<russh_sftp::protocol::Version, Self::Error> {
        self.observed.init_started.store(true, Ordering::Release);
        tokio::time::sleep(Duration::from_millis(self.delay as u64)).await;
        self.observed.init_finished.store(true, Ordering::Release);
        Ok(russh_sftp::protocol::Version::new())
    }
}

struct StalledWrites(Arc<Observed>);
impl russh_sftp::server::Handler for StalledWrites {
    type Error = russh_sftp::protocol::StatusCode;
    fn unimplemented(&self) -> Self::Error {
        Self::Error::OpUnsupported
    }
    async fn init(
        &mut self,
        _: u32,
        _: HashMap<String, String>,
    ) -> Result<russh_sftp::protocol::Version, Self::Error> {
        Ok(russh_sftp::protocol::Version::new())
    }
    async fn open(
        &mut self,
        id: u32,
        _: String,
        _: russh_sftp::protocol::OpenFlags,
        _: russh_sftp::protocol::FileAttributes,
    ) -> Result<russh_sftp::protocol::Handle, Self::Error> {
        Ok(russh_sftp::protocol::Handle {
            id,
            handle: "blocked".into(),
        })
    }
    async fn write(
        &mut self,
        _: u32,
        _: String,
        _: u64,
        _: Vec<u8>,
    ) -> Result<russh_sftp::protocol::Status, Self::Error> {
        self.0.stalled_write.store(true, Ordering::Release);
        std::future::pending().await
    }
}

struct Server {
    address: SocketAddr,
    fingerprint: String,
    observed: Arc<Observed>,
    stop: tokio::sync::watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.task.abort();
    }
}
impl Server {
    fn options(&self, timeout: Duration) -> SshOptions {
        SshOptions {
            host: self.address.ip().to_string(),
            port: self.address.port(),
            username: "fixture".into(),
            proxy: None,
            expected_host_key: Some(self.fingerprint.clone()),
            auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
            timeout,
        }
    }
    fn opened(&self) -> Vec<ChannelId> {
        self.observed
            .opened
            .lock()
            .unwrap_or_else(|_| panic!("opened mutex"))
            .clone()
    }
    fn was_closed(&self, id: ChannelId) -> bool {
        self.observed
            .closed
            .lock()
            .unwrap_or_else(|_| panic!("closed mutex"))
            .contains(&id)
    }
}

async fn serve() -> Result<Server, Box<dyn Error>> {
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        window_size: 64 * 1024,
        auth_rejection_time: Duration::from_millis(1),
        auth_rejection_time_initial: Some(Duration::from_millis(1)),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let observed = Arc::new(Observed::default());
    let shared = observed.clone();
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        let Ok((socket, _)) = listener.accept().await else {
            return;
        };
        let handler = Delayed {
            inner: super::Fixture::default(),
            observed: shared.clone(),
            tasks: Vec::new(),
            unconfirmed: Vec::new(),
            stalled_channels: HashMap::new(),
            stalled_packets: HashMap::new(),
        };
        if let Ok(mut running) = server::run_stream(config, socket, handler).await {
            let handle = running.handle();
            tokio::select! {
                _ = &mut running => {},
                _ = stopped.changed() => { let _ = handle.disconnect(russh::Disconnect::ByApplication, "fixture closed".into(), "en".into()).await; },
            }
        }
        shared.disconnected.store(true, Ordering::Release);
    });
    Ok(Server {
        address,
        fingerprint,
        observed,
        stop,
        task,
    })
}

async fn wait_until(mut predicate: impl FnMut() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn cancelling_each_session_open_closes_late_confirmation_and_preserves_ssh()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(server.options(Duration::from_secs(3))).await?;
    let sftp = Arc::new(session.sftp().await?);
    for operation in 0..4 {
        server.observed.next_delay.store(350, Ordering::Release);
        let count = server.opened().len();
        let owned = session.clone();
        let existing = sftp.clone();
        let task = tokio::spawn(async move {
            match operation {
                0 => {
                    owned.start_shell(24, 80).await?;
                }
                1 => {
                    owned.exec("cancel before confirmation").await?;
                }
                2 => {
                    owned.sftp().await?;
                }
                _ => {
                    existing.list_limited("/", 10).await?;
                }
            }
            Ok::<_, SessionError>(())
        });
        wait_until(|| server.opened().len() > count).await?;
        let channel = server.opened()[count];
        task.abort();
        assert!(task.await.is_err());
        wait_until(|| server.was_closed(channel)).await?;
        assert!(!session.is_closed());
        assert_eq!(session.exec("still usable").await?.stdout, b"still usable");
        assert!(sftp.list_limited("/", 10).await?.is_empty());
    }
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn unconfirmed_cancelled_open_releases_shared_transport_within_budget()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(server.options(Duration::from_millis(450))).await?;
    server
        .observed
        .next_delay
        .store(usize::MAX, Ordering::Release);
    let owned = session.clone();
    let started = tokio::time::Instant::now();
    let task = tokio::spawn(async move { owned.sftp().await });
    wait_until(|| !server.opened().is_empty()).await?;
    task.abort();
    assert!(task.await.is_err());
    wait_until(|| session.is_closed() && server.observed.disconnected.load(Ordering::Acquire))
        .await?;
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(server.opened().len(), 1);
    assert!(session.exec("must not reopen").await.is_err());
    Ok(())
}

#[tokio::test]
async fn sftp_initialization_cancellation_closes_stream_before_late_version()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(server.options(Duration::from_secs(3))).await?;
    server.observed.init_delay.store(700, Ordering::Release);
    let owned = session.clone();
    let task = tokio::spawn(async move { owned.sftp().await });
    wait_until(|| server.observed.init_started.load(Ordering::Acquire)).await?;
    let channel = server.opened()[0];
    task.abort();
    assert!(task.await.is_err());
    wait_until(|| server.was_closed(channel)).await?;
    assert!(
        !server.observed.init_finished.load(Ordering::Acquire),
        "channel cleanup must not wait for SFTP version"
    );
    wait_until(|| server.observed.init_finished.load(Ordering::Acquire)).await?;
    assert!(!session.is_closed());
    assert_eq!(
        session.exec("after cancelled init").await?.stdout,
        b"after cancelled init"
    );
    let sftp = session.sftp().await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn owned_sftp_stream_moves_large_payloads_and_closes_without_ssh_disconnect()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(server.options(Duration::from_secs(5))).await?;
    let sftp = session.sftp().await?;
    let channel = server.opened()[0];
    // The fixture caps each file at 1 MiB. Repeated 768 KiB files exercise
    // sustained data in both directions beyond the relay's 64 KiB capacity.
    let bytes: Vec<_> = (0..768 * 1024).map(|index| (index % 251) as u8).collect();
    for index in 0..4 {
        let path = format!("/large-{index}.bin");
        sftp.write(&path, &bytes).await?;
        assert_eq!(sftp.read(&path, bytes.len()).await?, bytes);
    }
    sftp.close().await?;
    wait_until(|| server.was_closed(channel)).await?;
    assert!(!session.is_closed());
    assert_eq!(
        session.exec("after stream close").await?.stdout,
        b"after stream close"
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn sftp_close_after_shared_transport_shutdown_is_idempotent() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(server.options(Duration::from_secs(5))).await?;
    let sftp = session.sftp().await?;

    // The shared SSH shutdown closes the relay before the SFTP owner gets its
    // cleanup turn. `SftpSession::close` must treat that already-closed writer
    // as a completed local cleanup and must not mask the original shutdown.
    session.close().await?;
    sftp.close().await?;
    Ok(())
}

#[tokio::test]
async fn high_level_close_or_drop_cancels_a_zero_window_sftp_writer() -> Result<(), Box<dyn Error>>
{
    for explicit_close in [true, false] {
        let server = serve().await?;
        let session = SshSession::connect(server.options(Duration::from_secs(10))).await?;
        let sftp = Arc::new(session.sftp().await?);
        // Admission uses the normal subsystem for canonical metadata. The
        // direct writer's independent subsystem stalls actual bounded pipelined
        // WRITEs until its 64 KiB SSH window is exhausted.
        server
            .observed
            .stall_next_sftp
            .store(true, Ordering::Release);
        let writer = sftp.clone();
        let writing =
            tokio::spawn(
                async move { writer.write("/blocked", &vec![0x42; 4 * 1024 * 1024]).await },
            );
        wait_until(|| {
            server.observed.stalled_write.load(Ordering::Acquire)
                && server.observed.stalled_bytes.load(Ordering::Acquire) >= 64 * 1024
        })
        .await.inspect_err(|_| {
            eprintln!("zero-window entry close={explicit_close} write_header={} bytes={} opened={:?} finished={}", server.observed.stalled_write.load(Ordering::Acquire), server.observed.stalled_bytes.load(Ordering::Acquire), server.opened(), writing.is_finished());
        })?;
        let mut previous = 0;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                tokio::time::sleep(Duration::from_millis(30)).await;
                let now = server.observed.stalled_bytes.load(Ordering::Acquire);
                if now == previous {
                    break;
                }
                previous = now;
            }
        })
        .await?;
        let channel = *server
            .opened()
            .last()
            .ok_or("missing raw writer subsystem")?;
        assert!(
            !writing.is_finished(),
            "bounded pipelined raw writer must remain blocked"
        );
        assert!(!server.was_closed(channel));
        if explicit_close {
            tokio::time::timeout(Duration::from_secs(1), sftp.close()).await??;
            // The live session and write task still hold their owners here.
            // Explicit close must work without relying on their subsequent Drop.
            wait_until(|| server.was_closed(channel)).await?;
        }
        drop(sftp);
        writing.abort();
        let _ = writing.await;
        wait_until(|| server.was_closed(channel)).await?;
        assert!(
            !session.is_closed(),
            "normal SFTP release must preserve SSH"
        );
        assert_eq!(
            session.exec("after zero window cleanup").await?.stdout,
            b"after zero window cleanup"
        );
        session.close().await?;
    }
    Ok(())
}
