//! In-memory protocol peer shared by production FilesPanel behavior tests.
use keelshell_session::{SshAuth, SshOptions, SshSession};
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
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

use crate::files::sftp_test_filesystem as filesystem;

#[path = "test_server_identity_tests.rs"]
mod identity_tests;

#[path = "archive_tools_fixture.rs"]
pub(super) mod archive_tools_fixture;

#[path = "server_archive_fixture.rs"]
pub(super) mod server_archive_fixture;

#[path = "pause_handoff_fixture.rs"]
pub(crate) mod pause_handoff_fixture;

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
    archive_tools: Arc<archive_tools_fixture::Fixture>,
    archive_validation: Arc<archive_tools_fixture::Fixture>,
    exec_requests: Arc<AtomicUsize>,
    server_archives: Arc<server_archive_fixture::Fixture>,
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

    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.exec_requests.fetch_add(1, Ordering::AcqRel);
        if let Some(recipe) = self.server_archives.lookup(command) {
            let Some(channel) = self.channels.remove(&id) else {
                return Err(russh::Error::Disconnect);
            };
            session.channel_success(id)?;
            let fixture = self.server_archives.clone();
            let filesystem = self.filesystem.clone();
            self.jobs.spawn(async move {
                fixture.serve(channel, recipe, filesystem).await;
            });
            return Ok(());
        }
        let fixture = if command == keelshell_session::archive_tools::probe_command().as_bytes() {
            &self.archive_tools
        } else if command == keelshell_session::archive_validation::validation_command().as_bytes()
        {
            &self.archive_validation
        } else {
            session.channel_failure(id)?;
            session.close(id)?;
            return Ok(());
        };
        let Some(mut channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        let (response, hold) = fixture.reply();
        session.channel_success(id)?;
        self.jobs.spawn(async move {
            if let Some(mut hold) = hold {
                tokio::select! {
                    _ = &mut hold => {}
                    _ = tokio::time::sleep(Duration::from_secs(7)) => {
                        let _ = channel.close().await;
                        return;
                    }
                    _ = async {
                        // Exec and other request notifications may already
                        // be queued. Only closure retires the held reply.
                        while let Some(message) = channel.wait().await {
                            if matches!(message, russh::ChannelMsg::Close) {
                                break;
                            }
                        }
                    } => {
                        let _ = channel.close().await;
                        return;
                    }
                }
            }
            let _ = channel.data(&response[..]).await;
            let _ = channel.exit_status(0).await;
            let _ = channel.eof().await;
            let _ = channel.close().await;
        });
        Ok(())
    }
}

pub(crate) struct Server {
    pub(crate) filesystem: filesystem::Filesystem,
    pub(crate) active: Arc<AtomicUsize>,
    pub(super) archive_tools: Arc<archive_tools_fixture::Fixture>,
    pub(super) archive_validation: Arc<archive_tools_fixture::Fixture>,
    pub(super) exec_requests: Arc<AtomicUsize>,
    #[cfg(unix)]
    pub(super) server_archives: Arc<server_archive_fixture::Fixture>,
    address: std::net::SocketAddr,
    fingerprint: String,
    task: JoinHandle<()>,
    listener_handoff: Option<oneshot::Sender<oneshot::Sender<TcpListener>>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub(crate) fn new(runtime: &tokio::runtime::Runtime) -> Self {
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("bind file test server");
        Self::from_listener(runtime, listener)
    }
    fn from_listener(runtime: &tokio::runtime::Runtime, listener: TcpListener) -> Self {
        let address = listener.local_addr().checked("file server address");
        // Independent filesystems need distinct host identities when loopback
        // ports are reused. Reconnects to this Server keep the same config/key.
        let mut seed = [0_u8; 32];
        for part in seed.as_chunks_mut::<16>().0 {
            part.copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        }
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let filesystem = filesystem::Filesystem::default();
        let active = Arc::new(AtomicUsize::new(0));
        let shared_fs = filesystem.clone();
        let shared_active = active.clone();
        let archive_tools = Arc::new(archive_tools_fixture::Fixture::default());
        let shared_archive_tools = archive_tools.clone();
        let archive_validation = Arc::new(archive_tools_fixture::Fixture::validation());
        let shared_archive_validation = archive_validation.clone();
        let exec_requests = Arc::new(AtomicUsize::new(0));
        let shared_exec_requests = exec_requests.clone();
        let server_archives = Arc::new(server_archive_fixture::Fixture::default());
        let shared_server_archives = server_archives.clone();
        let (listener_handoff, mut handoff_request) =
            oneshot::channel::<oneshot::Sender<TcpListener>>();
        let task = runtime.spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    request = &mut handoff_request => {
                        // Transfer the bound socket without a rebind race. Existing
                        // SSH sessions keep their filesystem and are still joined.
                        if let Ok(return_listener) = request {
                            let _ = return_listener.send(listener);
                        }
                        break;
                    }
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break };
                        let config = config.clone();
                        let peer = Peer {
                            channels: HashMap::new(),
                            filesystem: shared_fs.clone(),
                            active: shared_active.clone(),
                            jobs: JoinSet::new(),
                            archive_tools: shared_archive_tools.clone(),
                            archive_validation: shared_archive_validation.clone(),
                            exec_requests: shared_exec_requests.clone(),
                            server_archives: shared_server_archives.clone(),
                        };
                        clients.spawn(async move {
                            if let Ok(client) = server::run_stream(config, socket, peer).await {
                                let _ = client.await;
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
            // Dropping a russh RunningSession detaches its protocol task, so the
            // endpoint-reuse regression closes its clients before joining here.
            while clients.join_next().await.is_some() {}
        });
        Self {
            filesystem,
            active,
            archive_tools,
            archive_validation,
            exec_requests,
            #[cfg(unix)]
            server_archives,
            address,
            fingerprint,
            task,
            listener_handoff: Some(listener_handoff),
        }
    }
    pub(crate) fn connect(&self, runtime: &tokio::runtime::Runtime) -> SshSession {
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
