//! Controlled SSH protocol fixture. Commands select deterministic scenarios;
//! no shell interprets or executes test input.
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::{
    Channel, ChannelId, ChannelOpenFailure,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use std::{
    collections::HashMap,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;
pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy)]
pub enum Opening {
    Normal,
    Reject,
    Delay(Duration),
}
#[derive(Default)]
pub struct Observed {
    pub opens: AtomicUsize,
    pub closes: AtomicUsize,
    pub active: AtomicUsize,
    pub peak: AtomicUsize,
    pub windows: AtomicUsize,
    pub commands: Mutex<Vec<Vec<u8>>>,
    gates: Mutex<HashMap<Vec<u8>, Arc<Semaphore>>>,
}
impl Observed {
    pub fn command_count(&self) -> TestResult<usize> {
        Ok(self.commands.lock().map_err(|_| "fixture lock")?.len())
    }
    pub fn release(&self, command: &str) -> TestResult {
        self.gates
            .lock()
            .map_err(|_| "fixture lock")?
            .get(command.as_bytes())
            .ok_or("missing gate")?
            .add_permits(1);
        Ok(())
    }
}
struct Active(Arc<Observed>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Handler {
    opening: Opening,
    observed: Arc<Observed>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: HashMap<ChannelId, JoinHandle<()>>,
}
impl Drop for Handler {
    fn drop(&mut self) {
        for job in self.jobs.values() {
            job.abort();
        }
    }
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "ephemeral" {
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
        let index = self.observed.opens.fetch_add(1, Ordering::AcqRel);
        if matches!(self.opening, Opening::Reject) {
            reply
                .reject(ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        }
        let id = channel.id();
        self.channels.insert(id, channel);
        if let Opening::Delay(delay) = self.opening
            && index == 0
        {
            self.jobs.insert(
                id,
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    reply.accept().await;
                }),
            );
        } else {
            reply.accept().await;
        }
        Ok(())
    }
    async fn window_adjusted(
        &mut self,
        _: ChannelId,
        _: u32,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed.windows.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(command.to_vec());
        if command == b"reject" {
            session.channel_failure(id)?;
            session.close(id)?;
            return Ok(());
        }
        if command == b"status-then-reject" {
            session.exit_status_request(id, 0)?;
            session.channel_failure(id)?;
            session.close(id)?;
            return Ok(());
        }
        if command == b"data-then-reject" {
            session.data(id, b"observed".to_vec())?;
            session.channel_failure(id)?;
            session.close(id)?;
            return Ok(());
        }
        session.channel_success(id)?;
        let gate = if command.starts_with(b"hold") {
            let gate = Arc::new(Semaphore::new(0));
            self.observed
                .gates
                .lock()
                .map_err(|_| russh::Error::Disconnect)?
                .insert(command.to_vec(), gate.clone());
            Some(gate)
        } else {
            None
        };
        let observed = self.observed.clone();
        let handle = session.handle();
        let command = command.to_vec();
        let active = observed.active.fetch_add(1, Ordering::AcqRel) + 1;
        observed.peak.fetch_max(active, Ordering::AcqRel);
        self.jobs.insert(
            id,
            tokio::spawn(async move {
                let _active = Active(observed);
                if let Some(gate) = gate {
                    let Ok(permit) = gate.acquire().await else {
                        return;
                    };
                    permit.forget();
                }
                if command == b"flood-stall" {
                    if handle
                        .extended_data(id, 1, b"partial stderr".to_vec())
                        .await
                        .is_err()
                    {
                        return;
                    }
                    for _ in 0..768 {
                        if handle.data(id, vec![b'x'; 4096]).await.is_err() {
                            return;
                        }
                    }
                    std::future::pending::<()>().await;
                }
                if command == b"limit" {
                    let _ = handle.data(id, b"1234".to_vec()).await;
                    let _ = handle.extended_data(id, 1, b"abcdefgh".to_vec()).await;
                    std::future::pending::<()>().await;
                }
                if command == b"status-then-drop" {
                    let _ = handle.exit_status_request(id, 0).await;
                }
                if command == b"drop" || command == b"status-then-drop" {
                    let _ = handle
                        .disconnect(
                            russh::Disconnect::ByApplication,
                            "fixture disconnect".into(),
                            "en".into(),
                        )
                        .await;
                    return;
                }
                let _ = handle.data(id, command.clone()).await;
                let _ = handle
                    .extended_data(id, 1, b"fixture stderr".to_vec())
                    .await;
                if command != b"close" {
                    let _ = handle
                        .exit_status_request(
                            id,
                            if command.starts_with(b"hold-fail") || command == b"fail" {
                                7
                            } else {
                                0
                            },
                        )
                        .await;
                }
                let _ = handle.close(id).await;
            }),
        );
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed.closes.fetch_add(1, Ordering::AcqRel);
        self.channels.remove(&id);
        if let Some(job) = self.jobs.remove(&id) {
            job.abort();
        }
        Ok(())
    }
}
pub struct Server {
    pub observed: Arc<Observed>,
    port: u16,
    fingerprint: String,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(opening: Opening) -> TestResult<Self> {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let observed = Arc::new(Observed::default());
        let state = observed.clone();
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            loop {
                tokio::select! {result=listener.accept()=>{let Ok((socket,_))=result else{break;};let handler=Handler{opening,observed:state.clone(),channels:HashMap::new(),jobs:HashMap::new()};let config=config.clone();jobs.spawn(async move{if let Ok(running)=server::run_stream(config,socket,handler).await{let _=running.await;}});},_=jobs.join_next(),if !jobs.is_empty()=>{}}
            }
        });
        Ok(Self {
            observed,
            port,
            fingerprint,
            task,
        })
    }
    pub async fn connect(&self) -> TestResult<SshSession> {
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = self.port;
        options.expected_host_key = Some(self.fingerprint.clone());
        options.auth = SshAuth::Password(Zeroizing::new("ephemeral".into()));
        options.timeout = Duration::from_secs(2);
        Ok(SshSession::connect(options).await?)
    }
}
pub async fn until(mut predicate: impl FnMut() -> bool) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}
pub async fn bounded(future: impl std::future::Future<Output = TestResult>) -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), future).await?
}
