//! Actual metadata replies renew only an active owning transfer's idle wait.
use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    sftp::{TransferEvent, TransferHandle, TransferSpec},
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

// Slow conservative filesystem claims are isolated from unrelated TCP tests.
// Unused packet gates are retained so the fixture protocol remains shared.
#[allow(dead_code)]
#[path = "fixtures/sftp.rs"]
mod sftp_fixture;
use sftp_fixture::MetadataKind;

// These scenarios deliberately exercise conservative whole-filesystem claims.
// Serialize them within this binary without weakening production exclusion.
static SCENARIOS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[path = "fixtures/directory_sync_isolation.rs"]
mod directory_sync_isolation;
const IDLE: Duration = Duration::from_millis(500);
const CADENCE_MS: usize = 150;

async fn connected(server: &Server) -> Result<SshSession, Box<dyn Error>> {
    let mut configured = options(server);
    configured.timeout = IDLE;
    Ok(SshSession::connect(configured).await?)
}
async fn started(job: &mut TransferHandle) -> Result<(), Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match job.recv().await {
                Some(TransferEvent::Started { .. }) => return Ok(()),
                Some(TransferEvent::Queued { .. }) => {}
                other => {
                    return Err::<(), Box<dyn Error>>(
                        format!("expected active transfer: {other:?}").into(),
                    );
                }
            }
        }
    })
    .await?
}
async fn terminal(job: &mut TransferHandle) -> Result<TransferEvent, Box<dyn Error>> {
    tokio::time::timeout(Duration::from_secs(25), async {
        while let Some(event) = job.recv().await {
            if matches!(
                event,
                TransferEvent::Completed { .. }
                    | TransferEvent::Failed { .. }
                    | TransferEvent::Cancelled { .. }
                    | TransferEvent::Uncertain { .. }
            ) {
                return Ok(event);
            }
        }
        Err::<_, Box<dyn Error>>("transfer ended without terminal event".into())
    })
    .await?
}
fn completed(outcome: &TransferEvent, bytes: usize) {
    assert!(
        matches!(outcome, TransferEvent::Completed { bytes: actual, .. } if *actual == bytes as u64),
        "{outcome:?}"
    );
}
fn observed(server: &Server, kinds: &[MetadataKind]) -> Result<(), Box<dyn Error>> {
    let timeline = server.filesystem.metadata_cadence.timeline()?;
    eprintln!("actual completed packet metadata timeline: {timeline:?}");
    for kind in kinds {
        assert!(
            timeline
                .iter()
                .any(|reply| reply.kind == *kind && reply.completion),
            "missing {kind:?} reply"
        );
    }
    assert!(
        timeline
            .iter()
            .filter(|reply| reply.completion)
            .map(|reply| reply.elapsed_ms)
            .max()
            .unwrap_or(0)
            > IDLE.as_millis()
    );
    Ok(())
}

#[tokio::test]
async fn queued_download_handles_timely_metadata_and_closes_the_source()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = connected(&server).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write("/download-metadata", b"complete source").await?;
    let local = tempfile::tempdir()?;
    let destination = local.path().join("output");
    let queue = sftp.clone().transfer_queue();
    let mut job = queue
        .enqueue(TransferSpec::download("/download-metadata", &destination))
        .await?;
    started(&mut job).await?;
    server.filesystem.metadata_cadence.start(CADENCE_MS)?;
    let outcome = terminal(&mut job).await?;
    server.filesystem.metadata_cadence.stop();
    let content = tokio::fs::read(&destination).await?;
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    eprintln!("metadata queued download: {outcome:?}");
    completed(&outcome, b"complete source".len());
    assert_eq!(content, b"complete source");
    // This single-file path has no FSTAT and may finish within one interval.
    let timeline = server.filesystem.metadata_cadence.timeline()?;
    assert!(
        timeline
            .iter()
            .any(|reply| reply.kind == MetadataKind::ReadOpen && reply.completion)
    );
    assert!(
        timeline
            .iter()
            .any(|reply| reply.kind == MetadataKind::ReadClose && reply.completion)
    );
    Ok(())
}

#[tokio::test]
async fn tree_download_metadata_cadence_outlives_idle_with_complete_files()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = connected(&server).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.mkdir("/tree-metadata").await?;
    for name in ["one", "two"] {
        sftp.write(&format!("/tree-metadata/{name}"), name.as_bytes())
            .await?;
    }
    let local = tempfile::tempdir()?;
    let destination = local.path().join("tree");
    let plan = sftp
        .plan_directory_transfer(TransferSpec::download("/tree-metadata", &destination))
        .await?;
    let queue = sftp.clone().transfer_queue();
    let mut job = queue.enqueue_directory(plan).await?;
    started(&mut job).await?;
    server.filesystem.metadata_cadence.start(CADENCE_MS)?;
    let began = tokio::time::Instant::now();
    let outcome = terminal(&mut job).await?;
    server.filesystem.metadata_cadence.stop();
    let elapsed = began.elapsed();
    let one = tokio::fs::read(destination.join("one")).await?;
    let two = tokio::fs::read(destination.join("two")).await?;
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    eprintln!("metadata tree download: elapsed={elapsed:?}, {outcome:?}");
    completed(&outcome, 6);
    assert_eq!(one, b"one");
    assert_eq!(two, b"two");
    assert!(elapsed > IDLE);
    observed(
        &server,
        &[
            MetadataKind::Lstat,
            MetadataKind::Fstat,
            MetadataKind::ReadOpen,
            MetadataKind::ReadClose,
            MetadataKind::OpenDir,
            MetadataKind::ReadDir,
        ],
    )?;
    Ok(())
}

#[tokio::test]
async fn file_and_directory_resume_metadata_cadence_revalidates_and_publishes_complete_bytes()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    for directory in [false, true] {
        let server = serve().await?;
        let session = connected(&server).await?;
        let sftp = Arc::new(session.sftp().await?);
        sftp.mkdir("/resume-metadata").await?;
        sftp.write("/resume-metadata/file", b"complete ").await?;
        let local = tempfile::tempdir()?;
        let tree = local.path().join("tree");
        tokio::fs::create_dir(&tree).await?;
        let source = tree.join("file");
        tokio::fs::write(&source, b"complete resumed content").await?;
        let queue = sftp.clone().transfer_queue();
        let mut job = if directory {
            let plan = sftp
                .plan_directory_resume(TransferSpec::upload(&tree, "/resume-metadata"))
                .await?;
            queue.enqueue_directory_resume(plan).await?
        } else {
            let plan = sftp
                .plan_file_resume(TransferSpec::upload(&source, "/resume-metadata/file"))
                .await?;
            queue.enqueue_resume(plan).await?
        };
        started(&mut job).await?;
        server.filesystem.metadata_cadence.start(CADENCE_MS)?;
        let began = tokio::time::Instant::now();
        let outcome = terminal(&mut job).await?;
        server.filesystem.metadata_cadence.stop();
        let elapsed = began.elapsed();
        let actual = sftp.read("/resume-metadata/file", 1024).await?;
        queue.close().await?;
        sftp.close().await?;
        session.close().await?;
        eprintln!("metadata resume: directory={directory}, elapsed={elapsed:?}, {outcome:?}");
        completed(&outcome, b"complete resumed content".len());
        assert_eq!(actual, b"complete resumed content");
        assert!(elapsed > IDLE);
        observed(
            &server,
            &[
                MetadataKind::Lstat,
                MetadataKind::Fstat,
                MetadataKind::ReadOpen,
                MetadataKind::ReadClose,
            ],
        )?;
    }
    Ok(())
}

#[tokio::test]
async fn timely_metadata_does_not_extend_fixed_pre_admission_deadline() -> Result<(), Box<dyn Error>>
{
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = connected(&server).await?;
    let sftp = session.sftp().await?;
    sftp.write_atomic("/admission-metadata", b"original")
        .await?;
    let local = tempfile::tempdir()?;
    let source = local.path().join("source");
    tokio::fs::write(&source, b"replacement").await?;
    server.filesystem.metadata_cadence.start(CADENCE_MS)?;
    let began = tokio::time::Instant::now();
    let outcome = tokio::time::timeout(
        Duration::from_secs(3),
        sftp.upload(&source, "/admission-metadata"),
    )
    .await?;
    let elapsed = began.elapsed();
    server.filesystem.metadata_cadence.stop();
    let actual = sftp.read("/admission-metadata", 1024).await?;
    let quarantines = sftp
        .inspect_transfer_quarantine(&TransferSpec::upload(&source, "/admission-metadata"))
        .await;
    let timeline = server.filesystem.metadata_cadence.timeline()?;
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "metadata fixed admission: elapsed={elapsed:?}, outcome={outcome:?}, timeline={timeline:?}"
    );
    assert!(matches!(outcome, Err(SessionError::Timeout(_))));
    assert!(elapsed < Duration::from_secs(2));
    assert_eq!(actual, b"original");
    assert!(matches!(
        quarantines,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    assert!(timeline.iter().filter(|reply| reply.completion).count() >= 2);
    Ok(())
}

#[tokio::test]
async fn held_readonly_close_is_bounded_despite_another_owners_confirmed_activity()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = connected(&server).await?;
    let sftp = Arc::new(session.sftp().await?);
    sftp.write_atomic("/held-readonly", b"complete download")
        .await?;
    let local = tempfile::tempdir()?;
    let destination = local.path().join("output");
    let peer_source = local.path().join("peer");
    tokio::fs::write(&peer_source, b"peer progress").await?;
    let hold = server
        .filesystem
        .hold_close("/held-readonly", false, false)?;
    let queue = sftp.clone().transfer_queue();
    let mut job = queue
        .enqueue(TransferSpec::download("/held-readonly", &destination))
        .await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while hold.entered() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    let began = tokio::time::Instant::now();
    let peer = async {
        let mut acknowledgements = 0;
        for _ in 0..10 {
            sftp.upload_atomic(&peer_source, "/peer-metadata").await?;
            acknowledgements += 1;
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        Ok::<_, Box<dyn Error>>(acknowledgements)
    };
    let (outcome, peer_result) = tokio::join!(
        async {
            let outcome = terminal(&mut job).await;
            (outcome, began.elapsed())
        },
        peer
    );
    let (outcome, terminal_elapsed) = outcome;
    let elapsed = began.elapsed();
    let outcome = outcome?;
    let peer_acknowledgements = peer_result?;
    let expired = hold.expired();
    hold.release();
    let quarantines = sftp
        .inspect_transfer_quarantine(&TransferSpec::download("/held-readonly", &destination))
        .await;
    let actual = tokio::fs::read(&destination).await?;
    let peer_actual = sftp.read("/peer-metadata", 1024).await?;
    queue.close().await?;
    sftp.close().await?;
    session.close().await?;
    eprintln!(
        "held readonly metadata: terminal_elapsed={terminal_elapsed:?}, total_elapsed={elapsed:?}, outcome={outcome:?}, peer_acks={peer_acknowledgements}"
    );
    assert!(matches!(outcome, TransferEvent::Failed { .. }));
    assert!(!expired);
    assert!(terminal_elapsed < Duration::from_millis(900));
    assert!(elapsed < Duration::from_secs(3));
    assert_eq!(peer_acknowledgements, 10);
    assert_eq!(actual, b"complete download");
    assert_eq!(peer_actual, b"peer progress");
    assert!(matches!(
        quarantines,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
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
