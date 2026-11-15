//! Archive reviews through real encrypted TCP and controlled SFTP packets.
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use keelshell_core::ArchiveMember;
use keelshell_core::{ArchiveBundle, ArchiveFormat};
use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    sftp::{ArchiveTransferRequest, SftpSession},
};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use russh_sftp::protocol::{
    Attrs, Data, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use zeroize::Zeroizing;

// Reuse the established file, byte-stream and bounded packet-hold implementation.
// This adapter changes standard RENAME to refuse an existing destination.
#[allow(dead_code)]
#[path = "fixtures/sftp.rs"]
mod sftp_fixture;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Default)]
struct Controls {
    writes: AtomicUsize,
    standard_renames: AtomicUsize,
    replacement_renames: AtomicUsize,
    before_rename: sftp_fixture::ResponseGate,
    after_rename: sftp_fixture::ResponseGate,
}

struct ArchiveFilesystem {
    inner: sftp_fixture::Filesystem,
    controls: Arc<Controls>,
}

impl russh_sftp::server::Handler for ArchiveFilesystem {
    type Error = StatusCode;

    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        extensions: HashMap<String, String>,
    ) -> Result<Version, StatusCode> {
        self.inner.init(version, extensions).await
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        self.inner.realpath(id, path).await
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.inner.lstat(id, path).await
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        self.inner.fstat(id, handle).await
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        if flags.intersects(OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE) {
            self.controls.writes.fetch_add(1, Ordering::AcqRel);
            if filename.contains(".keelshell-archive-")
                && (!flags.contains(OpenFlags::EXCLUDE)
                    || flags.contains(OpenFlags::TRUNCATE)
                    || attrs.permissions != Some(0o600))
            {
                return Err(StatusCode::PermissionDenied);
            }
        }
        self.inner.open(id, filename, flags, attrs).await
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        self.inner.close(id, handle).await
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
        self.inner.read(id, handle, offset, length).await
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        bytes: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        self.inner.write(id, handle, offset, bytes).await
    }
    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, StatusCode> {
        self.controls
            .standard_renames
            .fetch_add(1, Ordering::AcqRel);
        if self.controls.before_rename.is_armed() {
            self.controls
                .before_rename
                .hold()
                .await
                .map_err(|_| StatusCode::Failure)?;
        }
        match self.inner.lstat(id, new.clone()).await {
            Ok(_) => return Err(StatusCode::Failure),
            Err(StatusCode::NoSuchFile) => {}
            Err(error) => return Err(error),
        }
        let result = self.inner.rename(id, old, new).await?;
        if self.controls.after_rename.is_armed() {
            self.controls
                .after_rename
                .hold()
                .await
                .map_err(|_| StatusCode::Failure)?;
        }
        Ok(result)
    }
    async fn extended(&mut self, _: u32, _: String, _: Vec<u8>) -> Result<Packet, StatusCode> {
        self.controls
            .replacement_renames
            .fetch_add(1, Ordering::AcqRel);
        Err(StatusCode::OpUnsupported)
    }
}

struct SshHandler {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    filesystem: sftp_fixture::Filesystem,
    controls: Arc<Controls>,
}

impl server::Handler for SshHandler {
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
        let channel = self.channels.remove(&id).ok_or(russh::Error::Disconnect)?;
        session.channel_success(id)?;
        tokio::spawn(russh_sftp::server::run(
            channel.into_stream(),
            ArchiveFilesystem {
                inner: self.filesystem.clone(),
                controls: self.controls.clone(),
            },
        ));
        Ok(())
    }
}

struct Server {
    address: SocketAddr,
    fingerprint: String,
    filesystem: sftp_fixture::Filesystem,
    controls: Arc<Controls>,
    task: JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.controls.before_rename.release();
        self.controls.after_rename.release();
        self.task.abort();
    }
}

impl Server {
    async fn start() -> TestResult<Self> {
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
        let shared = filesystem.clone();
        let controls = Arc::new(Controls::default());
        let shared_controls = controls.clone();
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break };
                        let config = config.clone();
                        let handler = SshHandler {
                            channels: HashMap::new(),
                            filesystem: shared.clone(),
                            controls: shared_controls.clone(),
                        };
                        clients.spawn(async move {
                            if let Ok(session) = server::run_stream(config, socket, handler).await {
                                let _ = session.await;
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
        });
        Ok(Self {
            address,
            fingerprint,
            filesystem,
            controls,
            task,
        })
    }

    async fn connect(&self) -> TestResult<(SshSession, Arc<SftpSession>)> {
        self.connect_timeout(Duration::from_secs(8)).await
    }

    async fn connect_timeout(
        &self,
        timeout: Duration,
    ) -> TestResult<(SshSession, Arc<SftpSession>)> {
        let options = SshOptions {
            host: self.address.ip().to_string(),
            port: self.address.port(),
            username: "fixture".into(),
            proxy: None,
            expected_host_key: Some(self.fingerprint.clone()),
            auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
            timeout,
        };
        let ssh = SshSession::connect(options).await?;
        let sftp = Arc::new(ssh.sftp().await?);
        Ok((ssh, sftp))
    }
}

fn upload(sources: Vec<PathBuf>, remote: &str, format: ArchiveFormat) -> ArchiveTransferRequest {
    ArchiveTransferRequest::PackUpload {
        sources,
        remote: remote.into(),
        format,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn extract(remote: &str, local: PathBuf, format: ArchiveFormat) -> ArchiveTransferRequest {
    ArchiveTransferRequest::DownloadExtract {
        remote: remote.into(),
        local,
        format,
    }
}

fn binary(length: usize) -> Vec<u8> {
    let mut state = 0x7c1c_0001u32;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

async fn entered(mut condition: impl FnMut() -> bool) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
async fn reviewed_archive_binary_multiselect_roundtrip_uses_private_exclusive_staging_and_standard_rename()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let bytes = binary(150_113);
    let source = root.join("binary.dat");
    tokio::fs::write(&source, &bytes).await?;
    let folder = root.join("中文");
    tokio::fs::create_dir(&folder).await?;
    tokio::fs::create_dir(folder.join("empty")).await?;
    tokio::fs::write(folder.join("zero"), []).await?;
    for (format, name) in [
        (ArchiveFormat::Zip, "bundle.zip"),
        (ArchiveFormat::TarGz, "bundle.tar.gz"),
    ] {
        let remote = format!("/{name}");
        let request = upload(vec![folder.clone(), source.clone()], &remote, format);
        let plan = sftp
            .plan_archive_transfer(request.clone(), &|| true)
            .await?;
        assert_eq!(plan.request(), &request);
        assert_eq!(plan.members().len(), 4);
        assert_eq!(plan.expanded_bytes(), bytes.len() as u64);
        assert_eq!(
            server.controls.writes.load(Ordering::Acquire),
            if format == ArchiveFormat::Zip { 0 } else { 1 }
        );
        let receipt = sftp
            .execute_archive_transfer(plan.clone(), &|| true)
            .await?;
        assert_eq!(receipt.member_count, plan.members().len());
        assert_eq!(receipt.archive_bytes, plan.archive_bytes());
        let encoded = sftp.read_regular(&remote, 1024 * 1024).await?;
        assert_eq!(
            ArchiveBundle::decode(format, &encoded)?.members(),
            plan.members()
        );
        let destination = root.join(name);
        let download = sftp
            .plan_archive_transfer(extract(&remote, destination.clone(), format), &|| true)
            .await?;
        assert!(!destination.exists());
        let receipt = sftp.execute_archive_transfer(download, &|| true).await?;
        assert_eq!(receipt.expanded_bytes, bytes.len() as u64);
        assert_eq!(
            tokio::fs::read(destination.join("binary.dat")).await?,
            bytes
        );
        assert!(destination.join("中文/empty").is_dir());
        assert_eq!(
            tokio::fs::metadata(destination.join("中文/zero"))
                .await?
                .len(),
            0
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                tokio::fs::metadata(&destination)
                    .await?
                    .permissions()
                    .mode()
                    & 0o7777,
                0o700
            );
            assert_eq!(
                tokio::fs::metadata(destination.join("binary.dat"))
                    .await?
                    .permissions()
                    .mode()
                    & 0o7777,
                0o600
            );
        }
    }
    assert_eq!(server.controls.standard_renames.load(Ordering::Acquire), 2);
    assert_eq!(
        server.controls.replacement_renames.load(Ordering::Acquire),
        0
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_rechecks_full_source_digest_even_when_length_identity_and_mtime_are_restored()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("file.bin");
    tokio::fs::write(&source, b"reviewed").await?;
    let modified = tokio::fs::metadata(&source).await?.modified()?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source.clone()], "/changed.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    tokio::fs::write(&source, b"mutated!").await?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&source)?
        .set_modified(modified)?;
    assert!(sftp.execute_archive_transfer(plan, &|| true).await.is_err());
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    assert!(sftp.inspect_entry("/changed.zip").await?.is_none());
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_new_sources_and_late_remote_destinations_require_a_fresh_review_before_any_write()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().canonicalize()?.join("selected");
    tokio::fs::create_dir(&directory).await?;
    tokio::fs::write(directory.join("reviewed"), b"one").await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(
                vec![directory.clone()],
                "/new-member.zip",
                ArchiveFormat::Zip,
            ),
            &|| true,
        )
        .await?;
    tokio::fs::write(directory.join("not-reviewed"), b"two").await?;
    assert!(sftp.execute_archive_transfer(plan, &|| true).await.is_err());
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![directory], "/late.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    server
        .filesystem
        .replace_external_file("/late.zip", b"external owner")?;
    assert!(sftp.execute_archive_transfer(plan, &|| true).await.is_err());
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    assert_eq!(
        sftp.read_regular("/late.zip", 100).await?,
        b"external owner"
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_review_is_bound_to_the_exact_ssh_connection_and_current_authority() -> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let (replacement, other) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("file");
    tokio::fs::write(&source, b"reviewed").await?;
    let request = upload(vec![source], "/connection.zip", ArchiveFormat::Zip);
    assert!(matches!(
        sftp.plan_archive_transfer(request.clone(), &|| false).await,
        Err(SessionError::Closed)
    ));
    let plan = sftp.plan_archive_transfer(request, &|| true).await?;
    assert!(
        other
            .execute_archive_transfer(plan.clone(), &|| true)
            .await
            .is_err()
    );
    assert!(matches!(
        sftp.execute_archive_transfer(plan, &|| false).await,
        Err(SessionError::Closed)
    ));
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    other.close().await?;
    replacement.close().await?;
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn standard_sftp_rename_refuses_a_destination_created_at_the_publication_boundary()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("file");
    tokio::fs::write(&source, b"reviewed").await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source], "/race.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    server.controls.before_rename.arm();
    let owner = sftp.clone();
    let execution =
        tokio::spawn(async move { owner.execute_archive_transfer(plan, &|| true).await });
    entered(|| server.controls.before_rename.entered() > 0).await?;
    server
        .filesystem
        .replace_external_file("/race.zip", b"keep external destination")?;
    server.controls.before_rename.release();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), execution)
            .await??
            .is_err()
    );
    assert_eq!(
        sftp.read_regular("/race.zip", 100).await?,
        b"keep external destination"
    );
    assert_eq!(
        server.controls.replacement_renames.load(Ordering::Acquire),
        0
    );
    assert!(
        !sftp
            .inspect_remote_mutation_quarantine("/race.zip")
            .await?
            .entries()
            .is_empty()
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_revocation_during_pending_write_retains_quarantine_and_never_publishes()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("large");
    tokio::fs::write(&source, binary(180_001)).await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source], "/cancel.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/cancel.zip")?;
    let allowed = Arc::new(AtomicBool::new(true));
    let current = allowed.clone();
    let owner = sftp.clone();
    let execution = tokio::spawn(async move {
        owner
            .execute_archive_transfer(plan, &|| current.load(Ordering::Acquire))
            .await
    });
    entered(|| hold.entered() > 0).await?;
    allowed.store(false, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(5), execution).await??;
    assert!(matches!(result, Err(SessionError::MutationUncertain)));
    assert!(!hold.expired());
    hold.release();
    assert!(sftp.inspect_entry("/cancel.zip").await?.is_none());
    assert_eq!(server.controls.standard_renames.load(Ordering::Acquire), 0);
    let quarantine = sftp
        .inspect_remote_mutation_quarantine("/cancel.zip")
        .await?;
    assert!(
        quarantine
            .entries()
            .iter()
            .any(|entry| entry.destination == "/cancel.zip")
    );
    assert!(
        quarantine
            .entries()
            .iter()
            .any(|entry| entry.destination.contains(".cancel.zip.keelshell-archive-"))
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_overall_deadline_preserves_a_pending_write_as_unknown() -> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect_timeout(Duration::from_secs(1)).await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("large");
    tokio::fs::write(&source, binary(180_001)).await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source], "/deadline.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/deadline.zip")?;
    let owner = sftp.clone();
    let started = std::time::Instant::now();
    let execution =
        tokio::spawn(async move { owner.execute_archive_transfer(plan, &|| true).await });
    entered(|| hold.entered() > 0).await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(4), execution).await??,
        Err(SessionError::MutationUncertain)
    ));
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!hold.expired());
    hold.release();
    assert!(sftp.inspect_entry("/deadline.zip").await?.is_none());
    assert!(
        !sftp
            .inspect_remote_mutation_quarantine("/deadline.zip")
            .await?
            .entries()
            .is_empty()
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn archive_rechecks_sources_after_staging_and_before_standard_rename() -> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("large");
    tokio::fs::write(&source, binary(180_001)).await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source.clone()], "/staged.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    let hold = server
        .filesystem
        .hold_atomic_upload_after_first("/staged.zip")?;
    let owner = sftp.clone();
    let execution =
        tokio::spawn(async move { owner.execute_archive_transfer(plan, &|| true).await });
    entered(|| hold.entered() > 0).await?;
    tokio::fs::write(&source, vec![b'x'; 180_001]).await?;
    assert!(!hold.expired());
    hold.release();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), execution)
            .await??
            .is_err()
    );
    assert_eq!(server.controls.standard_renames.load(Ordering::Acquire), 0);
    assert!(sftp.inspect_entry("/staged.zip").await?.is_none());
    assert!(
        !sftp
            .inspect_remote_mutation_quarantine("/staged.zip")
            .await?
            .entries()
            .is_empty()
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[tokio::test]
async fn lost_standard_rename_acknowledgement_is_unknown_even_when_remote_bytes_were_published()
-> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().canonicalize()?.join("file");
    tokio::fs::write(&source, b"reviewed").await?;
    let plan = sftp
        .plan_archive_transfer(
            upload(vec![source], "/unknown.zip", ArchiveFormat::Zip),
            &|| true,
        )
        .await?;
    server.controls.after_rename.arm();
    let allowed = Arc::new(AtomicBool::new(true));
    let current = allowed.clone();
    let owner = sftp.clone();
    let execution = tokio::spawn(async move {
        owner
            .execute_archive_transfer(plan, &|| current.load(Ordering::Acquire))
            .await
    });
    entered(|| server.controls.after_rename.entered() > 0).await?;
    allowed.store(false, Ordering::Release);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), execution).await??,
        Err(SessionError::MutationUncertain)
    ));
    server.controls.after_rename.release();
    let encoded = sftp.read_regular("/unknown.zip", 1024 * 1024).await?;
    assert_eq!(
        ArchiveBundle::decode(ArchiveFormat::Zip, &encoded)?.members()[0].bytes(),
        b"reviewed"
    );
    assert!(
        sftp.inspect_remote_mutation_quarantine("/unknown.zip")
            .await?
            .entries()
            .iter()
            .any(|entry| entry.destination == "/unknown.zip")
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
async fn extraction_rechecks_all_remote_bytes_and_preserves_a_late_local_destination() -> TestResult
{
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let bytes = ArchiveBundle::new(vec![ArchiveMember::new_file("safe", b"reviewed".to_vec())?])?
        .encode(ArchiveFormat::TarGz)?;
    sftp.write("/source.tar.gz", &bytes).await?;
    let destination = root.join("extract");
    let request = extract("/source.tar.gz", destination.clone(), ArchiveFormat::TarGz);
    let plan = sftp
        .plan_archive_transfer(request.clone(), &|| true)
        .await?;
    let mut changed = bytes.clone();
    let index = changed.len() / 2;
    changed[index] ^= 0x40;
    server
        .filesystem
        .replace_external_file("/source.tar.gz", &changed)?;
    assert!(sftp.execute_archive_transfer(plan, &|| true).await.is_err());
    assert!(!destination.exists());
    assert!(
        tokio::fs::read_dir(&root)
            .await?
            .next_entry()
            .await?
            .is_none()
    );
    server
        .filesystem
        .replace_external_file("/source.tar.gz", &bytes)?;
    let plan = sftp.plan_archive_transfer(request, &|| true).await?;
    tokio::fs::create_dir(&destination).await?;
    tokio::fs::write(destination.join("keep"), b"external local owner").await?;
    assert!(sftp.execute_archive_transfer(plan, &|| true).await.is_err());
    assert_eq!(
        tokio::fs::read(destination.join("keep")).await?,
        b"external local owner"
    );
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
async fn corrupt_and_traversing_archives_never_reach_review_or_create_local_output() -> TestResult {
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let mut traversal = ArchiveBundle::new(vec![ArchiveMember::new_file(
        "safe.txt",
        b"bytes".to_vec(),
    )?])?
    .encode(ArchiveFormat::Zip)?;
    let mut replacements = 0;
    for offset in 0..traversal.len().saturating_sub(7) {
        if &traversal[offset..offset + 8] == b"safe.txt" {
            traversal[offset..offset + 8].copy_from_slice(b"../x.txt");
            replacements += 1;
        }
    }
    assert_eq!(replacements, 2);
    for bytes in [traversal, b"PK incomplete archive".to_vec()] {
        server
            .filesystem
            .replace_external_file("/unsafe.zip", &bytes)?;
        assert!(
            sftp.plan_archive_transfer(
                extract("/unsafe.zip", root.join("output"), ArchiveFormat::Zip),
                &|| true
            )
            .await
            .is_err()
        );
    }
    assert!(
        tokio::fs::read_dir(&root)
            .await?
            .next_entry()
            .await?
            .is_none()
    );
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn archive_rejects_source_symlinks_hard_links_parent_links_roots_and_traversal() -> TestResult
{
    let server = Server::start().await?;
    let (ssh, sftp) = server.connect().await?;
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let source = root.join("source");
    tokio::fs::write(&source, b"source").await?;
    let link = root.join("link");
    std::os::unix::fs::symlink(&source, &link)?;
    let parent_link = root.join("parent-link");
    std::os::unix::fs::symlink(&root, &parent_link)?;
    for source in [
        link,
        parent_link.join("source"),
        root.join("child/../source"),
        PathBuf::from("/"),
    ] {
        assert!(
            sftp.plan_archive_transfer(
                upload(vec![source], "/unsafe.zip", ArchiveFormat::Zip),
                &|| true
            )
            .await
            .is_err()
        );
    }
    tokio::fs::hard_link(&source, root.join("alias")).await?;
    assert!(
        sftp.plan_archive_transfer(
            upload(vec![source], "/unsafe.zip", ArchiveFormat::Zip),
            &|| true
        )
        .await
        .is_err()
    );
    assert_eq!(server.controls.writes.load(Ordering::Acquire), 0);
    sftp.close().await?;
    ssh.close().await?;
    Ok(())
}
