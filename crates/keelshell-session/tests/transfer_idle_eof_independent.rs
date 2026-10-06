//! Fresh independent EOF ownership and genuine no-reply controls.
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
async fn valid_eof_cannot_hide_a_genuinely_unanswered_readonly_close() -> Result<(), Box<dyn Error>>
{
    for directory in [false, true] {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.mkdir("/independent-close").await?;
        sftp.write("/independent-close/file", b"independent timely EOF")
            .await?;
        let temporary = tempfile::tempdir()?;
        let local = temporary
            .path()
            .join(if directory { "tree" } else { "file" });
        let queue = sftp.clone().transfer_queue();
        let mut job = if directory {
            queue
                .enqueue_directory(
                    sftp.plan_directory_transfer(TransferSpec::download(
                        "/independent-close",
                        &local,
                    ))
                    .await?,
                )
                .await?
        } else {
            queue
                .enqueue(TransferSpec::download("/independent-close/file", &local))
                .await?
        };
        job.pause();
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                match job.recv().await {
                    Some(TransferEvent::Paused { .. }) => break,
                    Some(TransferEvent::Queued { .. } | TransferEvent::Started { .. }) => {}
                    receipt => {
                        return Err::<(), Box<dyn Error>>(
                            format!("readonly-close scenario ended before pause: {receipt:?}")
                                .into(),
                        );
                    }
                }
            }
            Ok::<(), Box<dyn Error>>(())
        })
        .await
        .map_err(|error| format!("readonly-close pre-I/O pause: {error}"))??;
        let hold = server
            .filesystem
            .hold_close("/independent-close/file", false, false)?;
        server.filesystem.start_review_metadata_cadence(350)?;
        server.filesystem.set_transfer_read_delay(750);
        job.resume();
        let began = tokio::time::Instant::now();
        let outcome = tokio::time::timeout(Duration::from_secs(20), completed(&mut job))
            .await
            .map_err(|error| format!("readonly-close terminal receipt: {error}"))??;
        let elapsed = began.elapsed();
        let timeline = server.filesystem.review_metadata_timeline()?;
        let last_eof = timeline
            .iter()
            .filter(|(name, _)| name == "read:returned-valid-eof-status")
            .map(|(_, milliseconds)| *milliseconds)
            .next_back()
            .ok_or("no actual EOF")?;
        let interval_after_eof = elapsed.as_millis().saturating_sub(last_eof);
        let held = hold.entered();
        let pending = hold.pending();
        let expired = hold.expired();
        let quarantine = sftp
            .inspect_remote_mutation_quarantine("/independent-close/file")
            .await;
        hold.release();
        reached(|| hold.pending() == 0).await?;
        let final_timeline = server.filesystem.review_metadata_timeline()?;
        server.filesystem.stop_review_metadata_delay();
        server.filesystem.set_transfer_read_delay(0);
        let output = tokio::fs::read(if directory { local.join("file") } else { local }).await?;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "fresh EOF true stall directory={directory} elapsed={elapsed:?} since_eof_ms={interval_after_eof} outcome={outcome:?} held={held} pending={pending} server_timeline={final_timeline:?}"
        );
        server.stop().await?;
        assert!(
            matches!(outcome, TransferEvent::Failed { .. }),
            "readonly close is not an unknown write"
        );
        assert!(
            (850..1800).contains(&interval_after_eof),
            "one complete 1s idle interval must follow EOF"
        );
        assert_eq!((held, pending, expired), (1, 1, false));
        assert!(matches!(
            quarantine,
            Err(SessionError::Invalid(
                "no matching unknown transfer destination"
            ))
        ));
        assert_eq!(output, b"independent timely EOF");
        assert!(
            final_timeline
                .iter()
                .any(|(name, _)| name == "readonly-close:returned-valid-status")
        );
    }
    Ok(())
}

#[tokio::test]
async fn other_download_owners_eof_cannot_renew_unknown_write_or_writable_close()
-> Result<(), Box<dyn Error>> {
    for close_phase in [false, true] {
        let server = serve().await?;
        let mut configured = options(&server);
        configured.timeout = Duration::from_secs(1);
        let session = SshSession::connect(configured).await.map_err(|error| {
            format!("cross-owner fixture SSH connect close={close_phase}: {error:?}")
        })?;
        let sftp = Arc::new(session.sftp().await.map_err(|error| {
            format!("cross-owner fixture SFTP channel close={close_phase}: {error:?}")
        })?);
        sftp.write("/independent-pending", b"original")
            .await
            .map_err(|error| {
                format!("cross-owner fixture original-file setup close={close_phase}: {error:?}")
            })?;
        sftp.write("/independent-other-empty", b"")
            .await
            .map_err(|error| {
                format!("cross-owner fixture empty-file setup close={close_phase}: {error:?}")
            })?;
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("upload");
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
                    .hold_atomic_upload_after_first("/independent-pending")?,
            )
        };
        let closes = if close_phase {
            Some(
                server
                    .filesystem
                    .hold_close("/.independent-pending.keelshell-", true, true)?,
            )
        } else {
            None
        };
        let queue = sftp.clone().transfer_queue();
        let mut job = queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/independent-pending"))
            .await
            .map_err(|error| {
                format!("cross-owner measured upload enqueue close={close_phase}: {error:?}")
            })?;
        reached(|| {
            writes.as_ref().is_some_and(|h| h.entered() == 1)
                || closes.as_ref().is_some_and(|h| h.entered() == 1)
        })
        .await
        .map_err(|error| {
            format!("cross-owner actual pending mutation close={close_phase}: {error}")
        })?;
        server.filesystem.start_review_metadata_cadence(0)?;
        server.filesystem.set_transfer_read_delay(50);
        let side_queue = sftp.clone().transfer_queue();
        let done = std::sync::atomic::AtomicBool::new(false);
        let owner = async {
            let outcome = completed(&mut job).await;
            done.store(true, std::sync::atomic::Ordering::Release);
            outcome
        };
        let side = async {
            let mut completed_downloads = 0;
            while !done.load(std::sync::atomic::Ordering::Acquire) {
                let destination = temporary
                    .path()
                    .join(format!("other-{completed_downloads}"));
                let mut other = side_queue
                    .enqueue(TransferSpec::download(
                        "/independent-other-empty",
                        &destination,
                    ))
                    .await
                    .map_err(|error| {
                        format!("cross-owner side download enqueue close={close_phase}: {error:?}")
                    })?;
                let outcome = completed(&mut other).await.map_err(|error| {
                    format!("cross-owner side download terminal close={close_phase}: {error}")
                })?;
                assert!(matches!(outcome, TransferEvent::Completed { bytes: 0, .. }));
                assert!(tokio::fs::read(&destination).await?.is_empty());
                completed_downloads += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Ok::<usize, Box<dyn Error>>(completed_downloads)
        };
        let began = tokio::time::Instant::now();
        let (outcome, side_count) =
            tokio::time::timeout(Duration::from_secs(3), async { tokio::join!(owner, side) })
                .await
                .map_err(|error| format!("cross-owner EOF owner/side completion: {error}"))?;
        let elapsed = began.elapsed();
        let outcome = outcome.map_err(|error| {
            format!("cross-owner measured upload terminal close={close_phase}: {error}")
        })?;
        let side_count = side_count?;
        let before = sftp
            .inspect_remote_mutation_quarantine("/independent-pending")
            .await
            .map_err(|error| {
                format!("cross-owner quarantine before late reply close={close_phase}: {error:?}")
            })?;
        let before_ids: Vec<_> = before.entries().iter().map(|e| e.reservation_id).collect();
        let denied = sftp.upload_atomic(&source, "/independent-pending").await;
        if let Some(hold) = writes {
            assert!(!hold.expired());
            hold.release();
        }
        if let Some(hold) = &closes {
            assert!(!hold.expired());
            hold.release();
            reached(|| hold.pending() == 0).await.map_err(|error| {
                format!("cross-owner released CLOSE response close={close_phase}: {error}")
            })?;
        }
        let label = if close_phase {
            "writable-close:returned-valid-status"
        } else {
            "write:returned-valid-status"
        };
        reached(|| {
            server
                .filesystem
                .review_metadata_timeline()
                .is_ok_and(|t| t.iter().any(|(n, _)| n == label))
        })
        .await
        .map_err(|error| {
            format!("cross-owner actual late mutation STATUS close={close_phase}: {error}")
        })?;
        let after = sftp
            .inspect_remote_mutation_quarantine("/independent-pending")
            .await
            .map_err(|error| {
                format!("cross-owner quarantine after late reply close={close_phase}: {error:?}")
            })?;
        let after_ids: Vec<_> = after.entries().iter().map(|e| e.reservation_id).collect();
        let timeline = server.filesystem.review_metadata_timeline()?;
        let eof_count = timeline
            .iter()
            .filter(|(name, _)| name == "read:returned-valid-eof-status")
            .count();
        eprintln!(
            "fresh cross-owner EOF close={close_phase} elapsed={elapsed:?} side_completed={side_count} valid_eof={eof_count} outcome={outcome:?} before_ids={before_ids:?} after_ids={after_ids:?} server_timeline={timeline:?}"
        );
        // The owner/side idle scenario and timeline are complete. Content
        // readback has its own fixed total deadline; it is not an idle probe.
        server.filesystem.stop_review_metadata_delay();
        server.filesystem.set_transfer_read_delay(0);
        let final_bytes = sftp.read("/independent-pending", 1024).await
            .map_err(|error| format!("cross-owner fixture final original-content readback close={close_phase}: {error:?}"))?;
        side_queue.close().await.map_err(|error| {
            format!("cross-owner side queue close close={close_phase}: {error:?}")
        })?;
        queue.close().await.map_err(|error| {
            format!("cross-owner owner queue close close={close_phase}: {error:?}")
        })?;
        sftp.close()
            .await
            .map_err(|error| format!("cross-owner SFTP close close={close_phase}: {error:?}"))?;
        session
            .close()
            .await
            .map_err(|error| format!("cross-owner SSH close close={close_phase}: {error:?}"))?;
        server.stop().await?;
        assert!(side_count >= 5 && eof_count >= side_count);
        assert!(elapsed >= Duration::from_millis(700) && elapsed < Duration::from_millis(1500));
        assert!(
            matches!(outcome, TransferEvent::Uncertain { bytes, .. } if bytes == if close_phase { 12 } else { 32768 })
        );
        assert!(matches!(denied, Err(SessionError::MutationQuarantined)));
        assert_eq!(before_ids.len(), 2);
        assert_eq!(after_ids, before_ids);
        assert_eq!(final_bytes, b"original");
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
    .await
    .map_err(|error| format!("EOF transfer terminal receipt: {error}"))?
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
    let (disconnect, mut shutdown) = tokio::sync::watch::channel(false);
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
                _ = shutdown.changed() => break,
                _ = clients.join_next(), if !clients.is_empty() => {},
            }
        }
        clients.abort_all();
        while clients.join_next().await.is_some() {}
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

impl Server {
    async fn stop(mut self) -> Result<(), Box<dyn Error>> {
        self.disconnect.send_replace(true);
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .map_err(|error| format!("owned EOF listener task did not stop: {error}"))??;
        let connection = tokio::time::timeout(
            // Refusing a closed loopback port can require a TCP retry on some
            // platforms. This bounds only cleanup, never the 1s idle assertion.
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(self.address),
        )
        .await
        .map_err(|error| format!("owned EOF listener refusal probe: {error}"))?;
        assert!(connection.is_err(), "owned listener must be closed");
        Ok(())
    }
}

async fn reached(condition: impl Fn() -> bool) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|error| {
        format!("EOF fixture response gate did not reach its observed state: {error}")
    })?;
    Ok(())
}
