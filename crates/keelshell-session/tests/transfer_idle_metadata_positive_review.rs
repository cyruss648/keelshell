//! Independent successful metadata cadence must renew transfer idle.
use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    sftp::{TransferEvent, TransferSpec},
};
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
#[path = "fixtures/idle_metadata_matrix_review.rs"]
mod sftp_fixture;

#[tokio::test]
async fn timely_readonly_metadata_completes_directory_and_resume_downloads()
-> Result<(), Box<dyn Error>> {
    // Keep the existing-target resume variants sequential in their own binary:
    // their conservative local alias claims intentionally exclude other writers.
    for mode in 0..3 {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await?;
        let sftp = Arc::new(session.sftp().await?);
        let expected = b"independent readonly metadata";
        sftp.mkdir("/review-source").await?;
        sftp.write("/review-source/file", expected).await?;
        let local = tempfile::tempdir()?;
        let destination = local.path().join("destination");
        let queue = sftp.clone().transfer_queue();
        let mut job = match mode {
            0 => {
                let plan = sftp
                    .plan_directory_transfer(TransferSpec::download("/review-source", &destination))
                    .await?;
                queue.enqueue_directory(plan).await?
            }
            1 => {
                tokio::fs::write(&destination, &expected[..5]).await?;
                let plan = sftp
                    .plan_file_resume(TransferSpec::download("/review-source/file", &destination))
                    .await?;
                queue.enqueue_resume(plan).await?
            }
            _ => {
                tokio::fs::create_dir(&destination).await?;
                tokio::fs::write(destination.join("file"), &expected[..5]).await?;
                let plan = sftp
                    .plan_directory_resume(TransferSpec::download("/review-source", &destination))
                    .await?;
                queue.enqueue_directory_resume(plan).await?
            }
        };
        job.pause();
        tokio::time::timeout(Duration::from_secs(4), async {
            while let Some(event) = job.recv().await {
                match event {
                    TransferEvent::Paused { .. } => return Ok(()),
                    TransferEvent::Failed { .. }
                    | TransferEvent::Uncertain { .. }
                    | TransferEvent::Completed { .. }
                    | TransferEvent::Cancelled { .. } => {
                        return Err::<(), Box<dyn Error>>(
                            format!("transfer terminated before acknowledged pause: {event:?}")
                                .into(),
                        );
                    }
                    _ => {}
                }
            }
            Err::<(), Box<dyn Error>>("transfer stream ended before pause".into())
        })
        .await??;
        server.filesystem.start_review_metadata_cadence(350)?;
        let began = tokio::time::Instant::now();
        job.resume();
        let outcome = completed(&mut job).await?;
        let elapsed = began.elapsed();
        let timeline = server.filesystem.review_metadata_timeline()?;
        server.filesystem.stop_review_metadata_delay();
        let path = if mode == 1 {
            destination
        } else {
            destination.join("file")
        };
        let actual = tokio::fs::read(path).await;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "independent readonly metadata: mode={mode}, elapsed={elapsed:?}, outcome={outcome:?}, server_timeline={timeline:?}"
        );
        assert!(elapsed > Duration::from_secs(1));
        assert!(
            matches!(outcome, TransferEvent::Completed { bytes, .. } if bytes == expected.len() as u64)
        );
        assert_eq!(actual?, expected);
        for label in [
            "readonly-open:returned-valid-handle",
            "fstat:returned-valid-attrs",
            "readonly-close:returned-valid-status",
        ] {
            assert!(
                timeline.iter().any(|(value, _)| value == label),
                "actual {label} must be present for mode {mode}"
            );
        }
        if mode != 1 {
            for label in [
                "opendir:returned-valid-handle",
                "readdir:returned-valid-name",
                "readdir:returned-valid-eof-status",
            ] {
                assert!(
                    timeline.iter().any(|(value, _)| value == label),
                    "actual {label} must be present for tree mode {mode}"
                );
            }
        }
    }
    Ok(())
}

async fn completed(
    handle: &mut keelshell_session::sftp::TransferHandle,
) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(40), async {
        while let Some(event) = handle.recv().await {
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Uncertain { .. }
            ) {
                return Ok(event);
            }
        }
        Err::<_, Box<dyn Error>>("transfer stream ended".into())
    })
    .await?
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
