//! Actual localhost SSH, real temporary files and the generated Python helper.
//! Fault modes are transport/verification tests, not server acceptance evidence.

use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Seek, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use keelshell_core::server_archive::ServerArchiveRequest;
use keelshell_core::{ArchiveBundle, ArchiveFormat, ArchiveMember};
use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    sftp::{ServerArchivePlan, SftpSession},
};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, ChannelMsg, server};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};
use zeroize::Zeroizing;

pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    Reject,
    NoAck,
    Malformed,
    Overflow,
    CombinedOverflow,
    NoReceipt,
    NoExit,
    NoEof,
    NonzeroExit,
    WrongSize,
    WrongDigest,
    WrongBytes,
    DifferentArchive,
    SymlinkDestination,
    RetainedStage,
    FailRead,
    FailReadClose,
    PartialInput,
}

/// A bounded, explicitly armed barrier; release-before-entry retains a permit.
#[derive(Default)]
pub struct Gate {
    armed: AtomicBool,
    entered: AtomicUsize,
    released: tokio::sync::Notify,
}

impl Gate {
    pub fn arm(&self) {
        self.armed.store(true, Ordering::Release);
    }

    pub fn entered(&self) -> usize {
        self.entered.load(Ordering::Acquire)
    }

    pub fn release(&self) {
        self.armed.store(false, Ordering::Release);
        self.released.notify_one();
    }

    async fn hold(&self) -> TestResult {
        if self.armed.load(Ordering::Acquire) {
            self.entered.fetch_add(1, Ordering::AcqRel);
            tokio::time::timeout(Duration::from_secs(8), self.released.notified()).await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
enum ApprovedCommand {
    Inspect,
    Pack {
        destination: String,
        staging: String,
        format: ArchiveFormat,
    },
}

#[derive(Default)]
pub struct Controls {
    approved: Mutex<HashMap<String, ApprovedCommand>>,
    pub inspect_fault: Mutex<Fault>,
    pub pack_fault: Mutex<Fault>,
    pub inspect_commands: AtomicUsize,
    pub pack_commands: AtomicUsize,
    pub opens: AtomicUsize,
    pub closes: AtomicUsize,
    pub input_bytes: AtomicUsize,
    pub eof_count: AtomicUsize,
    pub stdin: Mutex<Vec<u8>>,
    pub commands: Mutex<Vec<String>>,
    pub open_gate_at: AtomicUsize,
    pub open_gate: Gate,
    pub canonical_gate: Gate,
    pub dispatch_gate: Gate,
    pub input_gate: Gate,
    pub output_gate: Gate,
    pub read_gate: Gate,
    pub close_gate: Gate,
    stop: AtomicBool,
}

pub struct Fixture {
    address: SocketAddr,
    fingerprint: String,
    root: Arc<tempfile::TempDir>,
    pub controls: Arc<Controls>,
    task: JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.controls.stop.store(true, Ordering::Release);
        for gate in [
            &self.controls.open_gate,
            &self.controls.canonical_gate,
            &self.controls.dispatch_gate,
            &self.controls.input_gate,
            &self.controls.output_gate,
            &self.controls.read_gate,
            &self.controls.close_gate,
        ] {
            gate.release();
        }
        self.task.abort();
    }
}

impl Fixture {
    pub async fn start() -> TestResult<Self> {
        let root = Arc::new(tempfile::tempdir()?);
        let canonical = root.path().canonicalize()?;
        tokio::fs::create_dir(canonical.join("tree")).await?;
        tokio::fs::create_dir(canonical.join("tree/empty")).await?;
        tokio::fs::write(
            canonical.join("tree/text.txt"),
            b"complete reviewed bytes\n",
        )
        .await?;
        tokio::fs::write(canonical.join("binary.bin"), [0, 255, 128, 10]).await?;
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            window_size: 4096,
            maximum_packet_size: 1024,
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let controls = Arc::new(Controls::default());
        let shared = controls.clone();
        let owned_root = root.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break };
                        let handler = SshHandler {
                            channels: HashMap::new(),
                            controls: shared.clone(),
                            root: owned_root.clone(),
                            tasks: JoinSet::new(),
                        };
                        let config = config.clone();
                        clients.spawn(async move {
                            if let Ok(client) = server::run_stream(config, stream, handler).await {
                                let _ = client.await;
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
            root,
            controls,
            task,
        })
    }

    pub fn root(&self) -> TestResult<PathBuf> {
        Ok(self.root.path().canonicalize()?)
    }

    pub fn path(&self, name: &str) -> TestResult<String> {
        self.root()?
            .join(name)
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| "non-UTF8 fixture".into())
    }

    pub fn request(&self, format: ArchiveFormat) -> TestResult<ServerArchiveRequest> {
        let name = match format {
            ArchiveFormat::Zip => "packed.zip",
            ArchiveFormat::TarGz => "packed.tar.gz",
        };
        let request = ServerArchiveRequest::new(
            vec![self.path("binary.bin")?, self.path("tree")?],
            self.path(name)?,
            format,
        )?;
        self.approve_inspection(&request)?;
        Ok(request)
    }

    pub fn approve_inspection(&self, request: &ServerArchiveRequest) -> TestResult {
        self.controls
            .approved
            .lock()
            .map_err(|_| "poisoned fixture")?
            .insert(request.inspection_command(), ApprovedCommand::Inspect);
        Ok(())
    }

    pub fn approve_pack(&self, plan: &ServerArchivePlan) -> TestResult {
        self.controls
            .approved
            .lock()
            .map_err(|_| "poisoned fixture")?
            .insert(
                plan.command().to_owned(),
                ApprovedCommand::Pack {
                    destination: plan.request().destination().to_owned(),
                    staging: plan.staging_path().to_owned(),
                    format: plan.request().format(),
                },
            );
        Ok(())
    }

    pub async fn connect(&self) -> TestResult<(SshSession, Arc<SftpSession>)> {
        self.connect_timeout(Duration::from_secs(6)).await
    }

    pub async fn connect_timeout(
        &self,
        timeout: Duration,
    ) -> TestResult<(SshSession, Arc<SftpSession>)> {
        let ssh = SshSession::connect(SshOptions {
            host: "127.0.0.1".into(),
            port: self.address.port(),
            username: "fixture".into(),
            proxy: None,
            expected_host_key: Some(self.fingerprint.clone()),
            auth: SshAuth::Password(Zeroizing::new("ephemeral-test-password".into())),
            timeout,
        })
        .await?;
        let sftp = Arc::new(ssh.sftp().await?);
        Ok((ssh, sftp))
    }

    pub fn pack_fault(&self, fault: Fault) -> TestResult {
        *self
            .controls
            .pack_fault
            .lock()
            .map_err(|_| "poisoned fixture")? = fault;
        Ok(())
    }

    pub fn inspect_fault(&self, fault: Fault) -> TestResult {
        *self
            .controls
            .inspect_fault
            .lock()
            .map_err(|_| "poisoned fixture")? = fault;
        Ok(())
    }
}

struct SshHandler {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    controls: Arc<Controls>,
    root: Arc<tempfile::TempDir>,
    tasks: JoinSet<()>,
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
        let number = self.controls.opens.fetch_add(1, Ordering::AcqRel) + 1;
        if self.controls.open_gate_at.load(Ordering::Acquire) == number {
            self.controls
                .open_gate
                .hold()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
        }
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.controls.closes.fetch_add(1, Ordering::AcqRel);
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
        let filesystem = Filesystem {
            root: self.root.clone(),
            controls: self.controls.clone(),
            handles: HashMap::new(),
            next: 0,
        };
        self.tasks.spawn(async move {
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
        let command = String::from_utf8(command.to_vec()).map_err(|_| russh::Error::Disconnect)?;
        let mut channel = self.channels.remove(&id).ok_or(russh::Error::Disconnect)?;
        if command == "keelshell-fixture-ping" {
            session.channel_success(id)?;
            self.tasks.spawn(async move {
                let _ = channel.data(&b"alive"[..]).await;
                let _ = channel.exit_status(0).await;
                let _ = channel.eof().await;
                let _ = channel.close().await;
            });
            return Ok(());
        }
        let approved = self
            .controls
            .approved
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .get(&command)
            .cloned();
        let Some(approved) = approved else {
            session.channel_failure(id)?;
            return Ok(());
        };
        self.controls
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(command.clone());
        let fault = match &approved {
            ApprovedCommand::Inspect => {
                self.controls
                    .inspect_commands
                    .fetch_add(1, Ordering::AcqRel);
                *self
                    .controls
                    .inspect_fault
                    .lock()
                    .map_err(|_| russh::Error::Disconnect)?
            }
            ApprovedCommand::Pack { .. } => {
                self.controls.pack_commands.fetch_add(1, Ordering::AcqRel);
                self.controls
                    .dispatch_gate
                    .hold()
                    .await
                    .map_err(|_| russh::Error::Disconnect)?;
                *self
                    .controls
                    .pack_fault
                    .lock()
                    .map_err(|_| russh::Error::Disconnect)?
            }
        };
        if fault == Fault::Reject {
            session.channel_failure(id)?;
            return Ok(());
        }
        if fault != Fault::NoAck {
            session.channel_success(id)?;
        }
        let controls = self.controls.clone();
        let root = self.root.clone();
        self.tasks.spawn(async move {
            let operation = async {
                let mut input = Vec::new();
                let mut got_eof = false;
                while let Some(message) = channel.wait().await {
                    match message {
                        ChannelMsg::Data { data } => {
                            if input.len().saturating_add(data.len()) > 1024 * 1024 {
                                return Err("fixture input exceeded budget".into());
                            }
                            input.extend_from_slice(&data);
                            controls.input_bytes.fetch_add(data.len(), Ordering::AcqRel);
                            if matches!(&approved, ApprovedCommand::Pack { .. }) {
                                controls.input_gate.hold().await?;
                            }
                            if fault == Fault::PartialInput {
                                channel.close().await?;
                                return Ok::<_, Box<dyn Error + Send + Sync>>(());
                            }
                        }
                        ChannelMsg::Eof => {
                            got_eof = true;
                            controls.eof_count.fetch_add(1, Ordering::AcqRel);
                            break;
                        }
                        ChannelMsg::Close => return Ok(()),
                        _ => {}
                    }
                }
                if !got_eof {
                    return Ok(());
                }
                if matches!(&approved, ApprovedCommand::Pack { .. }) {
                    *controls.stdin.lock().map_err(|_| "poisoned fixture")? = input.clone();
                }
                let mut output = match fault {
                    Fault::Malformed => ProcessOutput {
                        stdout: b"{bad json".to_vec(),
                        stderr: vec![],
                        exit: 0,
                    },
                    Fault::Overflow => ProcessOutput {
                        stdout: vec![b'x'; 1024 * 1024 + 1],
                        stderr: vec![],
                        exit: 0,
                    },
                    Fault::CombinedOverflow => ProcessOutput {
                        stdout: vec![b' '; 600_000],
                        stderr: vec![b'x'; 600_000],
                        exit: 0,
                    },
                    _ => {
                        let controls = controls.clone();
                        let keep_root = root.clone();
                        tokio::task::spawn_blocking(move || {
                            let _keep_root = keep_root;
                            run_helper(&command, input, &controls.stop)
                        })
                        .await??
                    }
                };
                alter_result(&approved, fault, &mut output)?;
                controls.output_gate.hold().await?;
                channel.data(&output.stdout[..]).await?;
                if !output.stderr.is_empty() {
                    channel.extended_data(1, &output.stderr[..]).await?;
                }
                if fault != Fault::NoExit {
                    channel.exit_status(output.exit).await?;
                }
                if fault != Fault::NoEof {
                    channel.eof().await?;
                }
                channel.close().await?;
                Ok(())
            };
            let _ = tokio::time::timeout(Duration::from_secs(10), operation).await;
        });
        Ok(())
    }
}

struct ProcessOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit: u32,
}

fn run_helper(command: &str, input: Vec<u8>, stop: &AtomicBool) -> TestResult<ProcessOutput> {
    // Only exact generated commands registered above can reach the shell.
    // Anonymous files avoid pipe deadlocks and unbounded reader-thread joins.
    let mut stdin = tempfile::tempfile()?;
    stdin.write_all(&input)?;
    stdin.rewind()?;
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let child = Command::new("/bin/sh")
        .args(["-c", &format!("exec {command}")])
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()?;
    let mut child = HelperChild(child);
    let until = Instant::now() + Duration::from_secs(7);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if stop.load(Ordering::Acquire)
            || Instant::now() >= until
            || stdout
                .metadata()?
                .len()
                .saturating_add(stderr.metadata()?.len())
                > 1024 * 1024
        {
            let _ = child.0.kill();
            let reaped_until = Instant::now() + Duration::from_secs(1);
            while child.0.try_wait()?.is_none() {
                if Instant::now() >= reaped_until {
                    return Err("fixture helper did not reap after kill".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            return Err("fixture Python helper was cancelled or exceeded 7 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    stdout.rewind()?;
    stderr.rewind()?;
    Ok(ProcessOutput {
        stdout: bounded_read(stdout)?,
        stderr: bounded_read(stderr)?,
        exit: status.code().unwrap_or(255) as u32,
    })
}

struct HelperChild(Child);

impl Drop for HelperChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.try_wait();
    }
}

fn bounded_read(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn alter_result(
    approved: &ApprovedCommand,
    fault: Fault,
    output: &mut ProcessOutput,
) -> TestResult {
    let ApprovedCommand::Pack {
        destination,
        staging,
        format,
    } = approved
    else {
        return Ok(());
    };
    match fault {
        Fault::NoReceipt => output.stdout.clear(),
        Fault::NonzeroExit => output.exit = 17,
        Fault::WrongSize | Fault::WrongDigest | Fault::DifferentArchive => {
            let mut receipt: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            match fault {
                Fault::WrongSize => receipt["archive_bytes"] = 1.into(),
                Fault::WrongDigest => receipt["sha256"] = "0".repeat(64).into(),
                Fault::DifferentArchive => {
                    let bundle = ArchiveBundle::new(vec![ArchiveMember::new_file(
                        "unreviewed.txt",
                        b"not the selected members".to_vec(),
                    )?])?;
                    let bytes = bundle.encode(*format)?;
                    std::fs::write(destination, &bytes)?;
                    receipt["archive_bytes"] = (bytes.len() as u64).into();
                    receipt["sha256"] = Sha256::digest(&bytes)
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                        .into();
                }
                _ => {}
            }
            output.stdout = serde_json::to_vec(&receipt)?;
        }
        Fault::WrongBytes => {
            let mut bytes = std::fs::read(destination)?;
            if let Some(byte) = bytes.first_mut() {
                *byte ^= 1;
            }
            std::fs::write(destination, bytes)?;
        }
        Fault::SymlinkDestination => {
            let target = format!("{destination}.fixture-original");
            std::fs::rename(destination, &target)?;
            std::os::unix::fs::symlink(&target, destination)?;
        }
        Fault::RetainedStage => std::fs::create_dir(staging)?,
        _ => {}
    }
    Ok(())
}

struct Filesystem {
    root: Arc<tempfile::TempDir>,
    controls: Arc<Controls>,
    handles: HashMap<String, (tokio::fs::File, String)>,
    next: u64,
}

impl Filesystem {
    fn checked_path(&self, path: &str, ancestors: bool) -> Result<PathBuf, StatusCode> {
        let root = self.root.path().canonicalize().map_err(status)?;
        let path = PathBuf::from(path);
        if !path.is_absolute()
            || path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || (!path.starts_with(&root) && !(ancestors && root.starts_with(&path)))
        {
            return Err(StatusCode::PermissionDenied);
        }
        Ok(path)
    }

    fn fault(&self) -> Result<Fault, StatusCode> {
        self.controls
            .pack_fault
            .lock()
            .map(|value| *value)
            .map_err(|_| StatusCode::Failure)
    }
}

fn status(error: std::io::Error) -> StatusCode {
    if error.kind() == std::io::ErrorKind::NotFound {
        StatusCode::NoSuchFile
    } else {
        StatusCode::Failure
    }
}

fn attrs(metadata: &std::fs::Metadata) -> FileAttributes {
    use std::os::unix::fs::MetadataExt;
    let mut attrs = FileAttributes::from(metadata);
    // The upstream conversion labels all nondirectories regular. LSTAT must
    // preserve the real symlink/special type for transport rejection tests.
    attrs.permissions = Some(metadata.mode());
    attrs
}

fn success(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}

impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;

    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        self.controls
            .canonical_gate
            .hold()
            .await
            .map_err(|_| StatusCode::Failure)?;
        let path = self.checked_path(&path, true)?;
        let canonical = tokio::fs::canonicalize(path).await.map_err(status)?;
        Ok(Name {
            id,
            files: vec![File::dummy(canonical.to_string_lossy().into_owned())],
        })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        let path = self.checked_path(&path, true)?;
        let metadata = tokio::fs::symlink_metadata(path).await.map_err(status)?;
        Ok(Attrs {
            id,
            attrs: attrs(&metadata),
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        let path = self.checked_path(&path, true)?;
        let metadata = tokio::fs::metadata(path).await.map_err(status)?;
        Ok(Attrs {
            id,
            attrs: attrs(&metadata),
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        let (file, _) = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        Ok(Attrs {
            id,
            attrs: attrs(&file.metadata().await.map_err(status)?),
        })
    }

    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let checked = self.checked_path(&path, false)?;
        let mut options = tokio::fs::OpenOptions::new();
        options.read(flags.contains(OpenFlags::READ));
        options.write(flags.contains(OpenFlags::WRITE));
        options.create(flags.contains(OpenFlags::CREATE) && !flags.contains(OpenFlags::EXCLUDE));
        options.create_new(flags.contains(OpenFlags::CREATE) && flags.contains(OpenFlags::EXCLUDE));
        options.truncate(flags.contains(OpenFlags::TRUNCATE));
        let file = options.open(checked).await.map_err(status)?;
        self.next += 1;
        let handle = self.next.to_string();
        self.handles.insert(handle.clone(), (file, path));
        Ok(Handle { id, handle })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        self.controls
            .read_gate
            .hold()
            .await
            .map_err(|_| StatusCode::Failure)?;
        if self.fault()? == Fault::FailRead {
            return Err(StatusCode::Failure);
        }
        let (file, _) = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(status)?;
        let mut data = vec![0; length.min(64 * 1024) as usize];
        let count = file.read(&mut data).await.map_err(status)?;
        if count == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(count);
        Ok(Data { id, data })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        self.controls
            .close_gate
            .hold()
            .await
            .map_err(|_| StatusCode::Failure)?;
        self.handles.remove(&handle).ok_or(StatusCode::Failure)?;
        if self.fault()? == Fault::FailReadClose {
            return Err(StatusCode::Failure);
        }
        Ok(success(id))
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        let (file, _) = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(status)?;
        file.write_all(&data).await.map_err(status)?;
        Ok(success(id))
    }
}

pub async fn entered(mut condition: impl FnMut() -> bool) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await?;
    Ok(())
}

pub async fn alive(ssh: &SshSession) -> TestResult {
    let output =
        tokio::time::timeout(Duration::from_secs(5), ssh.exec("keelshell-fixture-ping")).await??;
    assert_eq!(output.stdout, b"alive");
    assert_eq!(output.exit_status, Some(0));
    Ok(())
}
