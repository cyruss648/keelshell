//! Real encrypted, ephemeral SSH identities. No shell command is executed.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use super::{SshAuth, SshOptions, SshSession};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use russh_sftp::protocol::{Attrs, File, FileAttributes, Name, Status, StatusCode, Version};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};
use zeroize::Zeroizing;

pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Default)]
pub struct Observed {
    pub channels: AtomicUsize,
    pub subsystems: AtomicUsize,
    pub commands: AtomicUsize,
}

struct Peer {
    observed: Arc<Observed>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "ephemeral-zmodem-fixture" {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed.channels.fetch_add(1, Ordering::AcqRel);
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
        self.observed.subsystems.fetch_add(1, Ordering::AcqRel);
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let Some(channel) = self.channels.remove(&id) else {
            session.channel_failure(id)?;
            return Ok(());
        };
        session.channel_success(id)?;
        self.jobs.spawn(async move {
            russh_sftp::server::run(channel.into_stream(), Namespace).await;
        });
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        _: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed.commands.fetch_add(1, Ordering::AcqRel);
        session.channel_failure(id)?;
        Ok(())
    }
}

/// Only the namespace observations needed by shared admission. It has no writes.
struct Namespace;
impl russh_sftp::server::Handler for Namespace {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        _: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version {
            version,
            extensions: HashMap::new(),
        })
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File {
                filename: path,
                longname: String::new(),
                attrs: directory(),
            }],
        })
    }

    async fn lstat(&mut self, id: u32, _: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: directory(),
        })
    }

    async fn stat(&mut self, id: u32, _: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: directory(),
        })
    }

    async fn close(&mut self, id: u32, _: String) -> Result<Status, Self::Error> {
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

fn directory() -> FileAttributes {
    let mut attrs = FileAttributes::empty();
    attrs.permissions = Some(0o040755);
    attrs
}

pub struct Fixture {
    pub session: SshSession,
    pub observed: Arc<Observed>,
    task: Option<JoinHandle<()>>,
}

impl Fixture {
    pub async fn start() -> TestResult<Self> {
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
        let peer = Peer {
            observed: observed.clone(),
            channels: HashMap::new(),
            jobs: JoinSet::new(),
        };
        let task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, stream, peer).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("ephemeral-zmodem-fixture".into()));
        options.timeout = Duration::from_secs(3);
        let session = match tokio::time::timeout(
            Duration::from_secs(5),
            SshSession::connect(options),
        )
        .await
        {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => {
                task.abort();
                let _ = task.await;
                return Err(error.into());
            }
            Err(error) => {
                task.abort();
                let _ = task.await;
                return Err(error.into());
            }
        };
        Ok(Self {
            session,
            observed,
            task: Some(task),
        })
    }

    pub fn assert_no_authority_used(&self) {
        assert_eq!(self.observed.channels.load(Ordering::Acquire), 0);
        assert_eq!(self.observed.subsystems.load(Ordering::Acquire), 0);
        assert_eq!(self.observed.commands.load(Ordering::Acquire), 0);
    }

    pub async fn finish(mut self) -> TestResult {
        self.session.close().await?;
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(joined) => joined?,
                Err(error) => {
                    task.abort();
                    let _ = task.await;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
