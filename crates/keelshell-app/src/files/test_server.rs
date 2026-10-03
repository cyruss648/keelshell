//! In-memory protocol peer shared by production FilesPanel behavior tests.
use super::super::SshSession;
use keelshell_session::{SshAuth, SshOptions};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

use crate::files::sftp_test_filesystem as filesystem;

pub(super) trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}

struct LiveSubsystem(Arc<AtomicUsize>);
impl Drop for LiveSubsystem {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Peer {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    filesystem: filesystem::Filesystem,
    active: Arc<AtomicUsize>,
    jobs: JoinSet<()>,
}
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "fixture" {
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
        self.active.fetch_add(1, Ordering::AcqRel);
        let guard = LiveSubsystem(self.active.clone());
        let filesystem = self.filesystem.clone();
        self.jobs.spawn(async move {
            let _guard = guard;
            russh_sftp::server::run(channel.into_stream(), filesystem).await;
        });
        Ok(())
    }
}

pub(super) struct Server {
    pub(super) filesystem: filesystem::Filesystem,
    pub(super) active: Arc<AtomicUsize>,
    address: std::net::SocketAddr,
    fingerprint: String,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub(super) fn new(runtime: &tokio::runtime::Runtime) -> Self {
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("bind file test server");
        let address = listener.local_addr().checked("file server address");
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x59; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let filesystem = filesystem::Filesystem::default();
        let active = Arc::new(AtomicUsize::new(0));
        let shared_fs = filesystem.clone();
        let shared_active = active.clone();
        let task=runtime.spawn(async move {
            let mut clients=JoinSet::new();
            loop {tokio::select! {
                accepted=listener.accept()=>{
                    let Ok((socket,_))=accepted else {break};
                    let config=config.clone();
                    let peer=Peer {channels:HashMap::new(),filesystem:shared_fs.clone(),active:shared_active.clone(),jobs:JoinSet::new()};
                    clients.spawn(async move {if let Ok(client)=server::run_stream(config,socket,peer).await {let _=client.await;}});
                }
                _=clients.join_next(),if !clients.is_empty()=>{}
            }}
        });
        Self {
            filesystem,
            active,
            address,
            fingerprint,
            task,
        }
    }
    pub(super) fn connect(&self, runtime: &tokio::runtime::Runtime) -> SshSession {
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = self.address.port();
        options.timeout = Duration::from_secs(5);
        options.expected_host_key = Some(self.fingerprint.clone());
        options.auth = SshAuth::Password(Zeroizing::new("fixture".into()));
        runtime
            .block_on(SshSession::connect(options))
            .checked("connect real file test SSH")
    }
}
