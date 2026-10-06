//! Transfers may outlive one idle interval when every request is acknowledged.
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    sftp::{TransferEvent, TransferSpec},
};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

// A separate integration process keeps long full-content checks from occupying
// the process-wide conservative local alias claims used by unrelated fixtures.
// Reuse the real packet filesystem and its gates; other gates are unused here.
#[allow(dead_code)]
#[path = "fixtures/sftp.rs"]
mod sftp_fixture;

const IDLE: Duration = Duration::from_secs(1);
const WRITE_DELAY_MS: usize = 90;
const CONTENT_BYTES: usize = 1024 * 1024;

#[tokio::test]
async fn acknowledged_queue_upload_outlives_one_idle_interval() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = IDLE;
    let session = SshSession::connect(configured).await?;
    let sftp = Arc::new(session.sftp().await?);
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let content = vec![0x69; CONTENT_BYTES];
    tokio::fs::write(&source, &content).await?;
    server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
    let queue = sftp.clone().transfer_queue();
    let began = tokio::time::Instant::now();
    let mut handle = queue
        .enqueue(TransferSpec::upload(&source, "/slow-queue"))
        .await?;
    let outcome = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            match handle.recv().await {
                Some(
                    event @ (TransferEvent::Completed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Uncertain { .. }),
                ) => break Ok(event),
                Some(_) => {}
                None => break Err("transfer stream ended"),
            }
        }
    })
    .await??;
    let elapsed = began.elapsed();
    let requests = server.filesystem.transfer_writes_started();
    let remote = sftp.read("/slow-queue", CONTENT_BYTES).await;
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    eprintln!("slow queue: elapsed={elapsed:?}, requests={requests}, outcome={outcome:?}");
    assert!(requests > 2, "real acknowledged WRITE progression occurred");
    assert!(elapsed > IDLE);
    assert!(matches!(
        outcome,
        TransferEvent::Completed { bytes, .. } if bytes == CONTENT_BYTES as u64
    ));
    assert_eq!(remote?, content);
    Ok(())
}

#[tokio::test]
async fn acknowledged_direct_upload_outlives_one_idle_interval() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = IDLE;
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let content = vec![0x79; CONTENT_BYTES];
    tokio::fs::write(&source, &content).await?;
    server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
    let began = tokio::time::Instant::now();
    let outcome =
        tokio::time::timeout(Duration::from_secs(8), sftp.upload(&source, "/slow-direct")).await?;
    let elapsed = began.elapsed();
    let requests = server.filesystem.transfer_writes_started();
    let remote = sftp.read("/slow-direct", CONTENT_BYTES).await;
    sftp.close().await?;
    session.close().await?;
    eprintln!("slow direct: elapsed={elapsed:?}, requests={requests}, outcome={outcome:?}");
    assert!(requests > 2, "real acknowledged WRITE progression occurred");
    assert!(elapsed >= IDLE);
    assert_eq!(outcome?, CONTENT_BYTES as u64);
    assert_eq!(remote?, content);
    Ok(())
}

async fn completed(
    handle: &mut keelshell_session::sftp::TransferHandle,
) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(20), async {
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

async fn idle_session(server: &Server) -> Result<SshSession, Box<dyn Error>> {
    let mut configured = options(server);
    configured.timeout = IDLE;
    Ok(SshSession::connect(configured).await?)
}

#[tokio::test]
async fn direct_and_queued_atomic_uploads_progress_past_idle_and_publish_complete_bytes()
-> Result<(), Box<dyn Error>> {
    for direct in [false, true] {
        let server = serve().await?;
        let session = idle_session(&server).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.write("/slow-atomic", b"original").await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        let content = vec![0x19; CONTENT_BYTES];
        tokio::fs::write(&source, &content).await?;
        server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
        let queue = sftp.clone().transfer_queue();
        let began = tokio::time::Instant::now();
        let outcome = if direct {
            tokio::time::timeout(
                Duration::from_secs(10),
                sftp.upload_atomic(&source, "/slow-atomic"),
            )
            .await?
            .map(|bytes| TransferEvent::Completed { id: 0, bytes })
        } else {
            let mut job = queue
                .enqueue_atomic_upload(TransferSpec::upload(&source, "/slow-atomic"))
                .await?;
            Ok(completed(&mut job).await?)
        };
        let elapsed = began.elapsed();
        let remote = sftp.read("/slow-atomic", CONTENT_BYTES).await;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!("slow atomic: direct={direct}, elapsed={elapsed:?}, outcome={outcome:?}");
        assert!(elapsed > IDLE);
        assert!(
            matches!(outcome?, TransferEvent::Completed { bytes, .. } if bytes == CONTENT_BYTES as u64)
        );
        assert_eq!(remote?, content);
    }
    Ok(())
}

#[tokio::test]
async fn directory_and_continuation_uploads_use_the_same_progress_idle_clock()
-> Result<(), Box<dyn Error>> {
    for mode in 0..3 {
        let server = serve().await?;
        let session = idle_session(&server).await?;
        let sftp = Arc::new(session.sftp().await?);
        let local = tempfile::tempdir()?;
        let tree = local.path().join("tree");
        tokio::fs::create_dir(&tree).await?;
        let source = tree.join("file");
        let content = vec![0x59; CONTENT_BYTES];
        tokio::fs::write(&source, &content).await?;
        if mode > 0 {
            sftp.mkdir("/slow-tree").await?;
            sftp.write("/slow-tree/file", &content[..65536]).await?;
        }
        let queue = sftp.clone().transfer_queue();
        let mut job = match mode {
            0 => {
                let plan = sftp
                    .plan_directory_transfer(TransferSpec::upload(&tree, "/slow-tree"))
                    .await?;
                server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
                queue.enqueue_directory(plan).await?
            }
            1 => {
                let plan = sftp
                    .plan_file_resume(TransferSpec::upload(&source, "/slow-tree/file"))
                    .await?;
                server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
                queue.enqueue_resume(plan).await?
            }
            _ => {
                let plan = sftp
                    .plan_directory_resume(TransferSpec::upload(&tree, "/slow-tree"))
                    .await?;
                server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
                queue.enqueue_directory_resume(plan).await?
            }
        };
        let began = tokio::time::Instant::now();
        let outcome = completed(&mut job).await?;
        let elapsed = began.elapsed();
        let remote = sftp.read("/slow-tree/file", CONTENT_BYTES).await;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!("slow continuation: mode={mode}, elapsed={elapsed:?}, outcome={outcome:?}");
        assert!(elapsed > IDLE);
        assert!(
            matches!(outcome, TransferEvent::Completed { bytes, .. } if bytes == CONTENT_BYTES as u64)
        );
        assert_eq!(remote?, content);
    }
    Ok(())
}

#[tokio::test]
async fn slow_acknowledged_download_and_full_resume_validation_outlive_idle()
-> Result<(), Box<dyn Error>> {
    for mode in 0..4 {
        let server = serve().await?;
        let session = idle_session(&server).await?;
        let sftp = Arc::new(session.sftp().await?);
        let content = vec![0x49; CONTENT_BYTES];
        sftp.mkdir("/slow-source").await?;
        sftp.write("/slow-source/file", &content).await?;
        let local = tempfile::tempdir()?;
        let target = local.path().join("file");
        if mode == 2 {
            tokio::fs::write(&target, &content[..65536]).await?;
        }
        let queue = sftp.clone().transfer_queue();
        let spec = TransferSpec::download("/slow-source/file", &target);
        let resume = if mode == 2 {
            Some(sftp.plan_file_resume(spec.clone()).await?)
        } else {
            None
        };
        let tree_target = local.path().join("tree");
        let directory = if mode == 3 {
            Some(
                sftp.plan_directory_transfer(TransferSpec::download("/slow-source", &tree_target))
                    .await?,
            )
        } else {
            None
        };
        server.filesystem.set_transfer_read_delay(WRITE_DELAY_MS);
        let began = tokio::time::Instant::now();
        let outcome = match mode {
            0 => tokio::time::timeout(
                Duration::from_secs(20),
                sftp.download("/slow-source/file", &target),
            )
            .await?
            .map(|bytes| TransferEvent::Completed { id: 0, bytes }),
            1 => {
                let mut job = queue.enqueue(spec).await?;
                Ok(completed(&mut job).await?)
            }
            2 => {
                let mut job = queue
                    .enqueue_resume(resume.ok_or("resume plan missing")?)
                    .await?;
                Ok(completed(&mut job).await?)
            }
            _ => {
                let mut job = queue
                    .enqueue_directory(directory.ok_or("directory plan missing")?)
                    .await?;
                Ok(completed(&mut job).await?)
            }
        };
        let elapsed = began.elapsed();
        let actual_path = if mode == 3 {
            tree_target.join("file")
        } else {
            target
        };
        let actual = tokio::fs::read(actual_path).await;
        server.filesystem.set_transfer_read_delay(0);
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!("slow download: mode={mode}, elapsed={elapsed:?}, outcome={outcome:?}");
        assert!(elapsed > IDLE);
        assert!(
            matches!(outcome?, TransferEvent::Completed { bytes, .. } if bytes == CONTENT_BYTES as u64)
        );
        assert_eq!(actual?, content);
    }
    Ok(())
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

#[tokio::test]
async fn pending_write_and_close_timeout_independently_of_another_jobs_acknowledgments()
-> Result<(), Box<dyn Error>> {
    for close_phase in [false, true] {
        let server = serve().await?;
        let session = idle_session(&server).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.write("/idle-blocked", b"original").await?;
        let local = tempfile::tempdir()?;
        let source = local.path().join("source");
        let content = vec![0x39; CONTENT_BYTES];
        tokio::fs::write(&source, &content).await?;
        let short = local.path().join("short");
        tokio::fs::write(&short, b"fully acknowledged bytes").await?;
        let writes = if !close_phase {
            Some(
                server
                    .filesystem
                    .hold_atomic_upload_after_first("/idle-blocked")?,
            )
        } else {
            None
        };
        let closes = if close_phase {
            Some(
                server
                    .filesystem
                    .hold_close("/.idle-blocked.keelshell-", true, true)?,
            )
        } else {
            None
        };
        server.filesystem.set_transfer_write_delay(WRITE_DELAY_MS);
        let queue = sftp.clone().transfer_queue();
        queue.set_parallelism(2)?;
        let mut blocked = queue
            .enqueue_atomic_upload(TransferSpec::upload(
                if close_phase { &short } else { &source },
                "/idle-blocked",
            ))
            .await?;
        reached(|| {
            writes.as_ref().is_some_and(|h| h.entered() == 1)
                || closes.as_ref().is_some_and(|h| h.entered() == 1)
        })
        .await?;
        let began = tokio::time::Instant::now();
        let mut peer = queue
            .enqueue_atomic_upload(TransferSpec::upload(&source, "/idle-peer"))
            .await?;
        let outcome = completed(&mut blocked).await?;
        let elapsed = began.elapsed();
        let other_session = idle_session(&server).await?;
        let other = other_session.sftp().await?;
        let denied = other.upload_atomic(&short, "/idle-blocked").await;
        let before = other
            .inspect_remote_mutation_quarantine("/idle-blocked")
            .await?;
        let before_ids: Vec<_> = before.entries().iter().map(|e| e.reservation_id).collect();
        let peer_outcome = completed(&mut peer).await?;
        let peer_bytes = other.read("/idle-peer", CONTENT_BYTES).await;
        if let Some(hold) = writes {
            hold.release();
        }
        if let Some(hold) = &closes {
            hold.release();
        }
        if let Some(hold) = &closes {
            reached(|| hold.pending() == 0).await?;
        }
        let after = other
            .inspect_remote_mutation_quarantine("/idle-blocked")
            .await?;
        let after_ids: Vec<_> = after.entries().iter().map(|e| e.reservation_id).collect();
        queue.close().await?;
        other.close().await?;
        other_session.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!(
            "pending mutation: close={close_phase}, elapsed={elapsed:?}, outcome={outcome:?}, peer={peer_outcome:?}"
        );
        assert!(elapsed < Duration::from_secs(3));
        assert!(
            matches!(outcome, TransferEvent::Uncertain { bytes, .. } if bytes == if close_phase { 24 } else { 32768 })
        );
        assert!(matches!(denied, Err(SessionError::MutationQuarantined)));
        assert_eq!(before_ids.len(), 2);
        assert_eq!(
            after_ids, before_ids,
            "late ACK does not silently clear quarantine"
        );
        assert!(
            matches!(peer_outcome, TransferEvent::Completed { bytes, .. } if bytes == CONTENT_BYTES as u64)
        );
        assert_eq!(peer_bytes?, content);
    }
    Ok(())
}

#[tokio::test]
async fn an_unanswered_read_has_a_bounded_known_partial_download() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = idle_session(&server).await?;
    let sftp = Arc::new(session.sftp().await?);
    let content = vec![0x29; CONTENT_BYTES];
    sftp.write("/idle-read", &content).await?;
    let local = tempfile::tempdir()?;
    let target = local.path().join("partial");
    let hold = server.filesystem.hold_transfer_reads_after_first()?;
    let queue = sftp.clone().transfer_queue();
    let spec = TransferSpec::download("/idle-read", &target);
    let mut job = queue.enqueue(spec.clone()).await?;
    reached(|| hold.entered() == 1).await?;
    let began = tokio::time::Instant::now();
    let outcome = completed(&mut job).await?;
    let elapsed = began.elapsed();
    let partial = tokio::fs::read(&target).await?;
    let quarantine = sftp.inspect_transfer_quarantine(&spec).await;
    hold.release();
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "unanswered READ: elapsed={elapsed:?}, outcome={outcome:?}, partial={}",
        partial.len()
    );
    assert!(elapsed < Duration::from_secs(3));
    assert!(matches!(outcome, TransferEvent::Failed { .. }));
    assert_eq!(partial, content[..65536]);
    assert!(
        quarantine.is_err(),
        "cancelled READ cannot leave a late destination write"
    );
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
