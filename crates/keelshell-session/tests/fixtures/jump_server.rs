//! Observable SSH relay without blocking the server while a channel is pending.
use keelshell_session::{SshAuth, SshOptions};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::{Shutdown, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use zeroize::Zeroizing;

#[derive(Default)]
pub(super) struct Observed {
    pub auth_started: AtomicUsize,
    pub auth_delay: AtomicUsize,
    pub next_open_delay: AtomicUsize,
    pub never_confirm_session: AtomicUsize,
    pub drop_connections: AtomicUsize,
    pub disconnected: AtomicUsize,
    pub requests: Mutex<Vec<(String, u32)>>,
    opened: Mutex<Vec<ChannelId>>,
    closed: Mutex<HashSet<ChannelId>>,
}
impl Observed {
    pub fn opens(&self) -> Vec<ChannelId> {
        self.opened
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
    pub fn was_closed(&self, id: ChannelId) -> bool {
        self.closed.lock().is_ok_and(|value| value.contains(&id))
    }
}

struct Fixture {
    inner: super::Fixture,
    observed: Arc<Observed>,
    relays: HashMap<ChannelId, JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in self.relays.values() {
            task.abort();
        }
        self.observed.disconnected.fetch_add(1, Ordering::Release);
    }
}
impl server::Handler for Fixture {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.observed.auth_started.fetch_add(1, Ordering::Release);
        let delay = self.observed.auth_delay.load(Ordering::Acquire);
        if delay != 0 {
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        }
        self.inner.auth_password(user, password).await
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self
            .observed
            .never_confirm_session
            .swap(0, Ordering::AcqRel)
            != 0
        {
            let id = channel.id();
            self.relays.insert(
                id,
                tokio::spawn(async move {
                    let _pending = (channel, reply);
                    std::future::pending::<()>().await;
                }),
            );
            return Ok(());
        }
        self.inner
            .channel_open_session(channel, reply, session)
            .await
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.inner.exec_request(id, data, session).await
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
        if let Some(task) = self.relays.remove(&id) {
            task.abort();
        }
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
        let id = channel.id();
        if let Ok(mut opened) = self.observed.opened.lock() {
            opened.push(id);
        }
        if let Ok(mut requests) = self.observed.requests.lock() {
            requests.push((host.into(), port));
        }
        let delay = self.observed.next_open_delay.swap(0, Ordering::AcqRel);
        let host = host.to_owned();
        self.relays.insert(
            id,
            tokio::spawn(async move {
                if delay == usize::MAX {
                    // Own both values until the parent disconnects; never send a reply.
                    let _pending = (channel, reply);
                    std::future::pending::<()>().await;
                    return;
                }
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay as u64)).await;
                }
                if host == "blocked.invalid" {
                    reply
                        .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                        .await;
                    return;
                }
                let resolved = if host == "remote-only.invalid" {
                    "127.0.0.1"
                } else {
                    &host
                };
                let Ok(mut socket) = TcpStream::connect((resolved, port as u16)).await else {
                    reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
                    return;
                };
                reply.accept().await;
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
            }),
        );
        Ok(())
    }
}

pub(super) struct Server {
    pub address: SocketAddr,
    pub fingerprint: String,
    pub observed: Arc<Observed>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.task.abort();
    }
}
impl Server {
    pub fn options(&self, timeout: Duration) -> SshOptions {
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
    pub async fn start() -> Result<Self, Box<dyn Error>> {
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
        let observed = Arc::new(Observed::default());
        let shared = observed.clone();
        let (stop, mut stopped) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    biased;
                    _ = stopped.changed() => break,
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break; };
                        if shared.drop_connections.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| count.checked_sub(1)).is_ok() {
                            drop(socket);
                            continue;
                        }
                        let fixture = Fixture { inner: Default::default(), observed: shared.clone(), relays: HashMap::new() };
                        let config = config.clone();
                        clients.spawn(async move {
                            let Ok(socket) = socket.into_std() else { return; };
                            let Ok(duplicate) = socket.try_clone() else { return; };
                            let _owner = SocketOwner(duplicate);
                            let Ok(socket) = TcpStream::from_std(socket) else { return; };
                            if let Ok(session) = server::run_stream(config, socket, fixture).await { let _ = session.await; }
                        });
                    },
                    _ = clients.join_next(), if !clients.is_empty() => {},
                }
            }
        });
        Ok(Self {
            address,
            fingerprint,
            observed,
            stop,
            task,
        })
    }
}
struct SocketOwner(std::net::TcpStream);
impl Drop for SocketOwner {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

pub(super) async fn wait_until(mut condition: impl FnMut() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}
