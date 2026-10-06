//! Independent successful metadata cadence must renew transfer idle.
use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
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
async fn same_session_side_queries_do_not_extend_pending_write_or_close()
-> Result<(), Box<dyn Error>> {
    for close_phase in [false, true] {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.write("/scope-pending", b"original").await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        let content = if close_phase {
            b"review close".to_vec()
        } else {
            vec![0x72; 65536]
        };
        tokio::fs::write(&source, &content).await?;
        let writes = if close_phase {
            None
        } else {
            Some(
                server
                    .filesystem
                    .hold_atomic_upload_after_first("/scope-pending")?,
            )
        };
        let closes = if close_phase {
            Some(
                server
                    .filesystem
                    .hold_close("/.scope-pending.keelshell-", true, true)?,
            )
        } else {
            None
        };
        let queue = sftp.clone().transfer_queue();
        let mut job = queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/scope-pending"))
            .await?;
        reached(|| {
            writes.as_ref().is_some_and(|hold| hold.entered() == 1)
                || closes.as_ref().is_some_and(|hold| hold.entered() == 1)
        })
        .await?;
        server.filesystem.start_review_metadata_cadence(0)?;
        let side_completions = std::sync::atomic::AtomicUsize::new(0);
        let side_queries = async {
            loop {
                // This uses the exact same SftpSession while the queue owner's
                // future is pending. It must run outside that owner's poll scope.
                assert_eq!(sftp.canonicalize("/").await?, "/");
                side_completions.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            #[allow(unreachable_code)]
            Ok::<(), Box<dyn Error>>(())
        };
        tokio::pin!(side_queries);
        let began = tokio::time::Instant::now();
        let outcome = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::select! {
                result = completed(&mut job) => result,
                result = &mut side_queries => {
                    result?;
                    Err::<TransferEvent, Box<dyn Error>>("side queries unexpectedly terminated".into())
                }
            }
        }).await??;
        let elapsed = began.elapsed();
        let queries = side_completions.load(std::sync::atomic::Ordering::Acquire);
        let before = sftp
            .inspect_remote_mutation_quarantine("/scope-pending")
            .await?;
        let before_ids: Vec<_> = before
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect();
        let denied = sftp.upload_atomic(&source, "/scope-pending").await;
        if let Some(hold) = writes {
            hold.release();
        }
        if let Some(hold) = &closes {
            hold.release();
        }
        let late_label = if close_phase {
            "writable-close:returned-valid-status"
        } else {
            "write:returned-valid-status"
        };
        reached(|| {
            server
                .filesystem
                .review_metadata_timeline()
                .is_ok_and(|timeline| timeline.iter().any(|(label, _)| label == late_label))
        })
        .await?;
        let after = sftp
            .inspect_remote_mutation_quarantine("/scope-pending")
            .await?;
        let after_ids: Vec<_> = after
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect();
        let timeline = server.filesystem.review_metadata_timeline()?;
        let final_bytes = sftp.read("/scope-pending", 1024).await;
        server.filesystem.stop_review_metadata_delay();
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "independent poll scope: close={close_phase}, elapsed={elapsed:?}, queries={queries}, outcome={outcome:?}, before_ids={before_ids:?}, after_ids={after_ids:?}, server_timeline={timeline:?}"
        );
        assert!(
            queries >= 5,
            "the same-session side path actually returned multiple metadata replies"
        );
        assert!(elapsed >= Duration::from_millis(700) && elapsed < Duration::from_secs(2));
        assert!(matches!(outcome, TransferEvent::Uncertain { bytes, .. }
            if bytes == if close_phase { content.len() as u64 } else { 32768 }));
        assert!(matches!(denied, Err(SessionError::MutationQuarantined)));
        assert_eq!(before_ids.len(), 2);
        assert_eq!(
            after_ids, before_ids,
            "a server-returned late mutation reply cannot clear already-published unknown identities"
        );
        assert_eq!(final_bytes?, b"original");
    }
    Ok(())
}

#[tokio::test]
async fn timely_absence_replies_cannot_extend_fixed_admission_budget() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = Duration::from_secs(1);
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    tokio::fs::write(&source, b"no admitted mutation").await?;
    server.filesystem.set_review_missing_realpath(true);
    server.filesystem.start_review_metadata_cadence(350)?;
    let began = tokio::time::Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        sftp.upload_atomic(&source, "/missing/ancestor/target"),
    )
    .await?;
    let elapsed = began.elapsed();
    let timeline = server.filesystem.review_metadata_timeline()?;
    let writes = server.filesystem.transfer_writes_started();
    server.filesystem.stop_review_metadata_delay();
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "independent fixed admission: elapsed={elapsed:?}, result={result:?}, writes={writes}, server_timeline={timeline:?}"
    );
    assert!(matches!(
        result,
        Err(SessionError::Timeout("file mutation admission"))
    ));
    assert!(elapsed >= Duration::from_millis(700) && elapsed < Duration::from_secs(2));
    assert!(
        timeline
            .iter()
            .filter(|(label, _)| label == "realpath:returned-valid-no-such-file-status")
            .count()
            >= 2
    );
    assert_eq!(writes, 0);
    Ok(())
}

#[tokio::test]
async fn malformed_name_cannot_admit_or_complete_a_transfer() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = Duration::from_secs(1);
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    tokio::fs::write(&source, b"malformed metadata control").await?;
    server.filesystem.set_review_empty_realpath(true);
    server.filesystem.start_review_metadata_cadence(0)?;
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        sftp.upload_atomic(&source, "/malformed-target"),
    )
    .await?;
    let timeline = server.filesystem.review_metadata_timeline()?;
    let writes = server.filesystem.transfer_writes_started();
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "independent malformed metadata: result={result:?}, writes={writes}, server_timeline={timeline:?}"
    );
    assert!(result.is_err());
    assert!(
        timeline
            .iter()
            .any(|(label, _)| label == "realpath:returned-malformed-empty-name")
    );
    assert_eq!(writes, 0);
    Ok(())
}

async fn completed(
    handle: &mut keelshell_session::sftp::TransferHandle,
) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(8), async {
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

async fn reached(condition: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
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
