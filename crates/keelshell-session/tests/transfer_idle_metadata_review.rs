//! Independent successful metadata cadence must renew transfer idle.
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

// Preserve the actual packet filesystem; this private review adds only delay
// and monotonic observations to successful non-mutating metadata responses.
#[allow(dead_code)]
#[path = "fixtures/idle_review_metadata.rs"]
mod sftp_fixture;

#[tokio::test]
async fn successful_metadata_cadence_renews_direct_atomic_upload_idle() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = Duration::from_secs(1);
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    sftp.write_atomic("/metadata-cadence", b"original").await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    let expected = b"independent timely metadata";
    tokio::fs::write(&source, expected).await?;
    server.filesystem.start_review_metadata_cadence(350)?;
    let began = tokio::time::Instant::now();
    let outcome = tokio::time::timeout(
        Duration::from_secs(8),
        sftp.upload_atomic(&source, "/metadata-cadence"),
    )
    .await?;
    let elapsed = began.elapsed();
    let timeline = server.filesystem.review_metadata_timeline()?;
    server.filesystem.stop_review_metadata_delay();
    let actual = sftp.read("/metadata-cadence", 1024).await;
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "independent metadata cadence: elapsed={elapsed:?}, actual_outcome={outcome:?}, server_success_timeline={timeline:?}, final_bytes={actual:?}"
    );
    assert!(
        timeline
            .iter()
            .filter(|(label, _)| label.contains(":returned-valid-"))
            .count()
            >= 3
    );
    assert_eq!(
        outcome?,
        expected.len() as u64,
        "each actual successful metadata reply is timely and must renew idle"
    );
    assert_eq!(actual?, expected);
    Ok(())
}

#[tokio::test]
async fn undelayed_atomic_publication_control_has_complete_bytes() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = Duration::from_secs(1);
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    sftp.write_atomic("/metadata-cadence", b"original").await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    let expected = b"independent timely metadata";
    tokio::fs::write(&source, expected).await?;
    server.filesystem.start_review_metadata_cadence(0)?;
    let began = tokio::time::Instant::now();
    let outcome = tokio::time::timeout(
        Duration::from_secs(8),
        sftp.upload_atomic(&source, "/metadata-cadence"),
    )
    .await?;
    let elapsed = began.elapsed();
    let timeline = server.filesystem.review_metadata_timeline()?;
    server.filesystem.stop_review_metadata_delay();
    let actual = sftp.read("/metadata-cadence", 1024).await;
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "independent no-delay metadata control: elapsed={elapsed:?}, actual_outcome={outcome:?}, server_success_timeline={timeline:?}, final_bytes={actual:?}"
    );
    assert!(
        timeline
            .iter()
            .filter(|(label, _)| label.contains(":returned-valid-"))
            .count()
            >= 3
    );
    assert_eq!(
        outcome?,
        expected.len() as u64,
        "each actual successful metadata reply is timely and must renew idle"
    );
    assert_eq!(actual?, expected);
    Ok(())
}

#[derive(Default)]
struct Fixture {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    filesystem: sftp_fixture::Filesystem,
}
impl server::Handler for Fixture {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "ephemeral-test-password" {
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
}

struct Server {
    filesystem: sftp_fixture::Filesystem,
    address: SocketAddr,
    fingerprint: String,
    task: JoinHandle<()>,
    disconnect: tokio::sync::watch::Sender<bool>,
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
    let shared_fs = filesystem.clone();
    let (disconnect, shutdown) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((socket, _)) = accepted else { break; };
                    let config = config.clone();
                    let fixture = Fixture {
                        filesystem: shared_fs.clone(), channels: HashMap::new(),
                    };
                    let mut shutdown = shutdown.clone();
                    clients.spawn(async move {
                        if let Ok(mut session) = server::run_stream(config, socket, fixture).await {
                            let handle = session.handle();
                            tokio::select! {
                                _ = &mut session => {},
                                _ = shutdown.changed() => {
                                    let _ = handle.disconnect(russh::Disconnect::ByApplication,
                                        "fixture stop".into(), "en".into()).await;
                                },
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
        address,
        fingerprint,
        task,
        disconnect,
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
