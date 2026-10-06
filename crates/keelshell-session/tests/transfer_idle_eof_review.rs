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
async fn timely_read_eof_then_timely_readonly_close_complete_downloads()
-> Result<(), Box<dyn Error>> {
    let mut observed = Vec::new();
    for directory in [false, true] {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.mkdir("/eof-source").await?;
        let expected = b"independent timely EOF";
        sftp.write("/eof-source/file", expected).await?;
        let temporary = tempfile::tempdir()?;
        let local = temporary
            .path()
            .join(if directory { "tree" } else { "file" });
        let queue = sftp.clone().transfer_queue();
        let mut job = if directory {
            let plan = sftp
                .plan_directory_transfer(TransferSpec::download("/eof-source", &local))
                .await?;
            queue.enqueue_directory(plan).await?
        } else {
            queue
                .enqueue(TransferSpec::download("/eof-source/file", &local))
                .await?
        };
        job.pause();
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if matches!(job.recv().await, Some(TransferEvent::Paused { .. })) {
                    return Ok::<_, Box<dyn Error>>(());
                }
            }
        })
        .await??;
        // Keep the application's one-second idle budget. Each genuine READ
        // response is delayed 750ms and the readonly CLOSE response 350ms.
        // Enable delays only after admission and the acknowledged safe pause.
        server.filesystem.start_review_metadata_cadence(350)?;
        server.filesystem.set_transfer_read_delay(750);
        job.resume();
        let began = tokio::time::Instant::now();
        let outcome = tokio::time::timeout(Duration::from_secs(20), completed(&mut job)).await??;
        let elapsed = began.elapsed();
        // If a failure dropped the pending CLOSE, retain its late server reply
        // separately. This is not proof that the client consumed that reply.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let timeline = server.filesystem.review_metadata_timeline()?;
        server.filesystem.stop_review_metadata_delay();
        server.filesystem.set_transfer_read_delay(0);
        let output = tokio::fs::read(if directory { local.join("file") } else { local }).await?;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "independent EOF boundary: directory={directory}, elapsed={elapsed:?}, outcome={outcome:?}, output_bytes={}, server_timeline={timeline:?}",
            output.len()
        );
        observed.push((directory, outcome, output, timeline));
    }
    // Both distinct production paths run and clean up before either assertion.
    for (directory, outcome, output, timeline) in observed {
        assert!(
            timeline
                .iter()
                .any(|(label, _)| label == "read:returned-valid-eof-status")
        );
        assert!(
            timeline
                .iter()
                .any(|(label, _)| label == "readonly-close:begin")
        );
        assert_eq!(output, b"independent timely EOF");
        assert!(
            matches!(outcome, TransferEvent::Completed { bytes: 22, .. }),
            "timely EOF and readonly CLOSE must not falsely time out; directory={directory}: {outcome:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn empty_read_eof_then_timely_readonly_close_complete_downloads() -> Result<(), Box<dyn Error>>
{
    let mut observed = Vec::new();
    for directory in [false, true] {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.mkdir("/eof-source").await?;
        let expected = b"";
        sftp.write("/eof-source/file", expected).await?;
        let temporary = tempfile::tempdir()?;
        let local = temporary
            .path()
            .join(if directory { "tree" } else { "file" });
        let queue = sftp.clone().transfer_queue();
        let mut job = if directory {
            let plan = sftp
                .plan_directory_transfer(TransferSpec::download("/eof-source", &local))
                .await?;
            queue.enqueue_directory(plan).await?
        } else {
            queue
                .enqueue(TransferSpec::download("/eof-source/file", &local))
                .await?
        };
        job.pause();
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if matches!(job.recv().await, Some(TransferEvent::Paused { .. })) {
                    return Ok::<_, Box<dyn Error>>(());
                }
            }
        })
        .await??;
        // Keep the application's one-second idle budget. Each genuine READ
        // response is delayed 750ms and the readonly CLOSE response 350ms.
        // Enable delays only after admission and the acknowledged safe pause.
        server.filesystem.start_review_metadata_cadence(350)?;
        server.filesystem.set_transfer_read_delay(750);
        job.resume();
        let began = tokio::time::Instant::now();
        let outcome = tokio::time::timeout(Duration::from_secs(20), completed(&mut job)).await??;
        let elapsed = began.elapsed();
        // If a failure dropped the pending CLOSE, retain its late server reply
        // separately. This is not proof that the client consumed that reply.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let timeline = server.filesystem.review_metadata_timeline()?;
        server.filesystem.stop_review_metadata_delay();
        server.filesystem.set_transfer_read_delay(0);
        let output = tokio::fs::read(if directory { local.join("file") } else { local }).await?;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "empty EOF boundary: directory={directory}, elapsed={elapsed:?}, outcome={outcome:?}, output_bytes={}, server_timeline={timeline:?}",
            output.len()
        );
        observed.push((directory, outcome, output, timeline));
    }
    // Both distinct production paths run and clean up before either assertion.
    for (directory, outcome, output, timeline) in observed {
        assert!(
            timeline
                .iter()
                .any(|(label, _)| label == "read:returned-valid-eof-status")
        );
        assert!(
            timeline
                .iter()
                .any(|(label, _)| label == "readonly-close:begin")
        );
        assert!(output.is_empty());
        assert!(
            matches!(outcome, TransferEvent::Completed { bytes: 0, .. }),
            "timely EOF and readonly CLOSE must not falsely time out; directory={directory}: {outcome:?}"
        );
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
