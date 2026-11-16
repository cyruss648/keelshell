//! Backup verification through public editor APIs and real TCP/SSH/SFTP packets.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use keelshell_core::{MAX_TEXT_EDIT_BYTES, TextDocument, TextEncoding};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};
use zeroize::Zeroizing;

use super::{RegularFileSnapshot, SftpSession};
use crate::{SessionError, SshAuth, SshOptions, SshSession};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const TARGET: &str = "/editor.txt";
const BACKUP_PREFIX: &str = "/.editor.txt.keelshell-backup-";
const TARGET_MODE: u32 = 0o100640;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    CorruptTail,
    Truncate,
    Missing,
    Symlink,
    Directory,
    MissingType,
    MissingSize,
    WrongMode,
    Oversized,
    ReadFailure,
    EarlyEof,
    OpenedMetadataChange,
    ReadMetadataChange,
    LeafMetadataChange,
    CloseFailure,
    RevokeAfterPublication,
    RevokeOnClose,
    ChangeTargetOnClose,
    UnknownPublication,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HoldPoint {
    None,
    Read,
    ReadClose,
}

struct Gate {
    entered: AtomicUsize,
    pending: AtomicUsize,
    released: tokio::sync::watch::Sender<bool>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            entered: AtomicUsize::new(0),
            pending: AtomicUsize::new(0),
            released: tokio::sync::watch::channel(false).0,
        }
    }
}

struct GateLease<'a>(&'a AtomicUsize);

impl Drop for GateLease<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Gate {
    async fn hold(&self) -> Result<(), StatusCode> {
        let mut released = self.released.subscribe();
        self.pending.fetch_add(1, Ordering::AcqRel);
        let _lease = GateLease(&self.pending);
        self.entered.fetch_add(1, Ordering::AcqRel);
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if *released.borrow_and_update() {
                    return Ok(());
                }
                released.changed().await.map_err(|_| StatusCode::Failure)?;
            }
        })
        .await
        .map_err(|_| StatusCode::Failure)?
    }

    fn release(&self) {
        self.released.send_replace(true);
    }
}

#[derive(Clone)]
struct StoredFile {
    content: Vec<u8>,
    permissions: Option<u32>,
    modified: u32,
    has_size: bool,
}

impl StoredFile {
    fn attrs(&self) -> FileAttributes {
        let mut attrs = FileAttributes::empty();
        attrs.size = self.has_size.then_some(self.content.len() as u64);
        attrs.permissions = self.permissions;
        attrs.mtime = Some(self.modified);
        attrs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    BackupPublished,
    BackupOpened,
    BackupFstat,
    BackupLeafStat,
    BackupRead { offset: u64, content: Vec<u8> },
    BackupEof,
    BackupClosed,
    TargetStaged,
    TargetPublished,
    Removed(String),
}

#[derive(Default)]
struct State {
    files: HashMap<String, StoredFile>,
    backup: Option<String>,
    events: Vec<Event>,
}

struct Shared {
    state: Mutex<State>,
    fault: Fault,
    hold: HoldPoint,
    gate: Gate,
    allowed: AtomicBool,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("isolated fixture state poisoned")
    }
}

struct OpenedFile {
    path: String,
    writable: bool,
    fstats: usize,
}

struct Filesystem {
    shared: Arc<Shared>,
    handles: HashMap<String, OpenedFile>,
}

fn is_backup(path: &str) -> bool {
    path.starts_with(BACKUP_PREFIX) && path.ends_with(".txt")
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: "en".into(),
    }
}

fn packet_string(input: &mut &[u8]) -> Result<String, StatusCode> {
    let size = u32::from_be_bytes(
        input
            .get(..4)
            .ok_or(StatusCode::BadMessage)?
            .try_into()
            .map_err(|_| StatusCode::BadMessage)?,
    ) as usize;
    *input = &input[4..];
    let value = std::str::from_utf8(input.get(..size).ok_or(StatusCode::BadMessage)?)
        .map_err(|_| StatusCode::BadMessage)?
        .to_owned();
    *input = &input[size..];
    Ok(value)
}

impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;

    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        let mut version = Version::new();
        version
            .extensions
            .insert("posix-rename@openssh.com".into(), "1".into());
        Ok(version)
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        Ok(Name {
            id,
            files: vec![File::dummy(if path == "." { "/".into() } else { path })],
        })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        if path == "/" {
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o40700);
            return Ok(Attrs { id, attrs });
        }
        let mut state = self.shared.state();
        let attrs = state
            .files
            .get(&path)
            .ok_or(StatusCode::NoSuchFile)?
            .attrs();
        if is_backup(&path) {
            state.events.push(Event::BackupLeafStat);
        }
        Ok(Attrs { id, attrs })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.lstat(id, path).await
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let writable = flags.contains(OpenFlags::WRITE);
        {
            let mut state = self.shared.state();
            if flags.contains(OpenFlags::EXCLUDE) && state.files.contains_key(&filename) {
                return Err(StatusCode::Failure);
            }
            if flags.contains(OpenFlags::CREATE) {
                state.files.insert(
                    filename.clone(),
                    StoredFile {
                        content: Vec::new(),
                        permissions: attrs.permissions.map(|mode| 0o100000 | mode),
                        modified: 23,
                        has_size: true,
                    },
                );
            }
            let file = state
                .files
                .get_mut(&filename)
                .ok_or(StatusCode::NoSuchFile)?;
            if flags.contains(OpenFlags::TRUNCATE) {
                file.content.clear();
            }
            if is_backup(&filename) && !writable {
                state.events.push(Event::BackupOpened);
            }
            if writable && !filename.contains(".keelshell-backup-") {
                state.events.push(Event::TargetStaged);
            }
        }
        let handle = id.to_string();
        self.handles.insert(
            handle.clone(),
            OpenedFile {
                path: filename,
                writable,
                fstats: 0,
            },
        );
        Ok(Handle { id, handle })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        let opened = self.handles.get_mut(&handle).ok_or(StatusCode::Failure)?;
        opened.fstats += 1;
        let mut state = self.shared.state();
        let file = state
            .files
            .get_mut(&opened.path)
            .ok_or(StatusCode::NoSuchFile)?;
        let backup = is_backup(&opened.path);
        if backup && self.shared.fault == Fault::OpenedMetadataChange && opened.fstats == 1 {
            file.modified += 1;
        }
        let attrs = file.attrs();
        if backup && self.shared.fault == Fault::LeafMetadataChange && opened.fstats == 2 {
            file.modified += 1;
        }
        if backup {
            state.events.push(Event::BackupFstat);
        }
        Ok(Attrs { id, attrs })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        content: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        let opened = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        if !opened.writable {
            return Err(StatusCode::PermissionDenied);
        }
        let mut state = self.shared.state();
        let file = state
            .files
            .get_mut(&opened.path)
            .ok_or(StatusCode::NoSuchFile)?;
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        let end = offset
            .checked_add(content.len())
            .ok_or(StatusCode::Failure)?;
        if end > MAX_TEXT_EDIT_BYTES {
            return Err(StatusCode::Failure);
        }
        file.content.resize(file.content.len().max(end), 0);
        file.content[offset..end].copy_from_slice(&content);
        Ok(ok(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
        let path = self
            .handles
            .get(&handle)
            .ok_or(StatusCode::Failure)?
            .path
            .clone();
        let backup = is_backup(&path);
        if backup && offset == 0 && self.shared.hold == HoldPoint::Read {
            self.shared.gate.hold().await?;
        }
        if backup && self.shared.fault == Fault::ReadFailure {
            return Err(StatusCode::PermissionDenied);
        }
        let mut state = self.shared.state();
        let file = state.files.get_mut(&path).ok_or(StatusCode::NoSuchFile)?;
        let start = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        if start >= file.content.len()
            || (backup && self.shared.fault == Fault::EarlyEof && offset > 0)
        {
            if backup && self.shared.fault == Fault::ReadMetadataChange {
                file.modified += 1;
            }
            if backup {
                state.events.push(Event::BackupEof);
            }
            return Err(StatusCode::Eof);
        }
        let end = file.content.len().min(start + length as usize);
        let data = file.content[start..end].to_vec();
        if backup {
            state.events.push(Event::BackupRead {
                offset,
                content: data.clone(),
            });
        }
        Ok(Data { id, data })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        let opened = self.handles.remove(&handle).ok_or(StatusCode::Failure)?;
        if is_backup(&opened.path) && !opened.writable {
            if self.shared.hold == HoldPoint::ReadClose {
                self.shared.gate.hold().await?;
            }
            if self.shared.fault == Fault::CloseFailure {
                return Err(StatusCode::PermissionDenied);
            }
            let mut state = self.shared.state();
            state.events.push(Event::BackupClosed);
            if self.shared.fault == Fault::ChangeTargetOnClose {
                state
                    .files
                    .get_mut(TARGET)
                    .ok_or(StatusCode::NoSuchFile)?
                    .content = b"external".to_vec();
            }
            if self.shared.fault == Fault::RevokeOnClose {
                self.shared.allowed.store(false, Ordering::Release);
            }
        }
        Ok(ok(id))
    }

    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, StatusCode> {
        if request != "posix-rename@openssh.com" {
            return Err(StatusCode::OpUnsupported);
        }
        let mut input = data.as_slice();
        let from = packet_string(&mut input)?;
        let to = packet_string(&mut input)?;
        if !input.is_empty() {
            return Err(StatusCode::BadMessage);
        }
        let mut state = self.shared.state();
        let mut file = state.files.remove(&from).ok_or(StatusCode::NoSuchFile)?;
        if is_backup(&to) {
            state.backup = Some(to.clone());
            state.events.push(Event::BackupPublished);
            // Faults take effect at publication and are observed only through
            // later SFTP requests; successful rename alone cannot detect them.
            match self.shared.fault {
                Fault::CorruptTail => {
                    *file.content.last_mut().ok_or(StatusCode::Failure)? ^= 1;
                }
                Fault::Truncate => {
                    file.content.pop();
                }
                Fault::Symlink => file.permissions = Some(0o120777),
                Fault::Directory => file.permissions = Some(0o40700),
                Fault::MissingType => file.permissions = Some(0o600),
                Fault::MissingSize => file.has_size = false,
                Fault::WrongMode => file.permissions = Some(0o100644),
                Fault::Oversized => file.content.push(b'x'),
                Fault::RevokeAfterPublication => {
                    self.shared.allowed.store(false, Ordering::Release);
                }
                _ => {}
            }
            if self.shared.fault != Fault::Missing {
                state.files.insert(to, file);
            }
            if self.shared.fault == Fault::UnknownPublication {
                return Ok(Packet::Data(Data {
                    id,
                    data: b"no STATUS acknowledgement".to_vec(),
                }));
            }
        } else {
            if to == TARGET {
                state.events.push(Event::TargetPublished);
            }
            state.files.insert(to, file);
        }
        Ok(Packet::Status(ok(id)))
    }

    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        let mut state = self.shared.state();
        state.events.push(Event::Removed(path.clone()));
        state.files.remove(&path).ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
}

struct Peer {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    shared: Arc<Shared>,
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
        let filesystem = Filesystem {
            shared: self.shared.clone(),
            handles: HashMap::new(),
        };
        self.jobs.spawn(async move {
            russh_sftp::server::run(channel.into_stream(), filesystem).await;
        });
        Ok(())
    }
}

struct Fixture {
    shared: Arc<Shared>,
    session: SshSession,
    sftp: SftpSession,
    server: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.shared.gate.release();
        if let Some(server) = self.server.get_mut().take() {
            server.abort();
        }
    }
}

impl Fixture {
    async fn start(original: &[u8], fault: Fault, hold: HoldPoint) -> TestResult<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                files: HashMap::from([(
                    TARGET.into(),
                    StoredFile {
                        content: original.to_vec(),
                        permissions: Some(TARGET_MODE),
                        modified: 17,
                        has_size: true,
                    },
                )]),
                ..State::default()
            }),
            fault,
            hold,
            gate: Gate::default(),
            allowed: AtomicBool::new(true),
        });
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
        let peer = Peer {
            channels: HashMap::new(),
            shared: shared.clone(),
            jobs: JoinSet::new(),
        };
        let mut server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, stream, peer).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("ephemeral-test-password".into()));
        options.timeout = Duration::from_secs(5);
        let connected = tokio::time::timeout(Duration::from_secs(8), async {
            let session = SshSession::connect(options).await?;
            let sftp = session.sftp().await?;
            Ok::<_, SessionError>((session, sftp))
        })
        .await;
        let (session, sftp) = match connected {
            Ok(Ok(connected)) => connected,
            failed => {
                server.abort();
                let _ = (&mut server).await;
                return Err(match failed {
                    Ok(Err(error)) => error.into(),
                    Err(error) => error.into(),
                    Ok(Ok(_)) => unreachable!("successful setup is handled above"),
                });
            }
        };
        Ok(Self {
            shared,
            session,
            sftp,
            server: tokio::sync::Mutex::new(Some(server)),
        })
    }

    async fn review(&self) -> TestResult<RegularFileSnapshot> {
        Ok(self
            .sftp
            .read_regular_snapshot(TARGET, MAX_TEXT_EDIT_BYTES)
            .await?)
    }

    fn authorized(&self) -> bool {
        self.shared.allowed.load(Ordering::Acquire)
    }

    fn backup(&self) -> String {
        self.shared
            .state()
            .backup
            .clone()
            .expect("the controlled peer published a backup")
    }

    fn assert_target_untouched(&self, reviewed: &RegularFileSnapshot) {
        let state = self.shared.state();
        let target = state.files.get(TARGET).expect("original target retained");
        assert_eq!(target.content, reviewed.content);
        assert_eq!(target.attrs().size, reviewed.entry.size);
        assert_eq!(target.permissions, reviewed.entry.permissions);
        assert_eq!(Some(target.modified), reviewed.entry.modified);
        assert!(!state.events.contains(&Event::TargetStaged));
        assert!(!state.events.contains(&Event::TargetPublished));
        assert!(
            !state
                .events
                .iter()
                .any(|event| matches!(event, Event::Removed(path) if is_backup(path))),
            "verification never removes a published backup"
        );
    }

    fn assert_complete_read_before_target(&self, original: &[u8], replacement: &[u8]) {
        let state = self.shared.state();
        let position = |event: Event| {
            state
                .events
                .iter()
                .position(|observed| observed == &event)
                .expect("required SFTP event")
        };
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| **event == Event::BackupPublished)
                .count(),
            1
        );
        assert!(position(Event::BackupPublished) < position(Event::BackupOpened));
        assert!(position(Event::BackupOpened) < position(Event::BackupEof));
        assert!(position(Event::BackupEof) < position(Event::BackupClosed));
        assert!(position(Event::BackupClosed) < position(Event::TargetStaged));
        assert!(position(Event::TargetStaged) < position(Event::TargetPublished));
        let mut readback = Vec::new();
        for event in &state.events {
            if let Event::BackupRead { offset, content } = event {
                assert_eq!(*offset, readback.len() as u64);
                readback.extend_from_slice(content);
            }
        }
        assert_eq!(readback, original);
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| **event == Event::BackupFstat)
                .count(),
            2
        );
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| **event == Event::BackupLeafStat)
                .count(),
            2
        );
        let backup = state
            .files
            .get(state.backup.as_ref().expect("published backup path"))
            .expect("published backup retained");
        assert_eq!(backup.content, original);
        assert_eq!(backup.permissions, Some(0o100600));
        assert_eq!(backup.attrs().size, Some(original.len() as u64));
        let target = state.files.get(TARGET).expect("replacement target");
        assert_eq!(target.content, replacement);
        assert_eq!(target.permissions, Some(TARGET_MODE));
        assert_eq!(state.files.len(), 2, "no temporary or extra backup remains");
    }

    async fn assert_no_quarantine(&self, path: &str) -> TestResult {
        assert!(matches!(
            self.sftp.inspect_remote_mutation_quarantine(path).await,
            Err(SessionError::Invalid(
                "no matching unknown transfer destination"
            ))
        ));
        Ok(())
    }

    async fn finish(&self) -> TestResult {
        self.shared.gate.release();
        self.sftp.close().await?;
        self.session.close().await?;
        if let Some(mut server) = self.server.lock().await.take() {
            match tokio::time::timeout(Duration::from_secs(5), &mut server).await {
                Ok(joined) => joined?,
                Err(error) => {
                    server.abort();
                    let _ = server.await;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }
}

async fn reached(condition: impl Fn() -> bool) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}

fn assert_unverified(result: crate::Result<String>) {
    let error = result.expect_err("an unverified backup cannot authorize target replacement");
    assert!(
        matches!(error, SessionError::EditorBackupUnverified),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "backup could not be fully verified; this target replacement was not started"
    );
    assert!(!error.is_retryable());
}

async fn rejects_acknowledged_backup(fault: Fault, original: &[u8]) -> TestResult {
    let fixture = Fixture::start(original, fault, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    assert_unverified(
        fixture
            .sftp
            .write_editor_reviewed_authorized(&reviewed, b"new", &|| fixture.authorized())
            .await,
    );
    fixture.assert_target_untouched(&reviewed);
    let backup = fixture.backup();
    {
        let state = fixture.shared.state();
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| **event == Event::BackupPublished)
                .count(),
            1,
            "verification failure must not retry backup publication"
        );
        if fault == Fault::Missing {
            assert!(!state.files.contains_key(&backup));
        } else {
            assert!(
                state.files.contains_key(&backup),
                "failed backup remains for inspection"
            );
            let file = &state.files[&backup];
            match fault {
                Fault::CorruptTail => {
                    assert_eq!(file.content.len(), original.len());
                    assert_eq!(
                        &file.content[..original.len() - 1],
                        &original[..original.len() - 1]
                    );
                    assert_ne!(file.content.last(), original.last());
                }
                Fault::Truncate => assert_eq!(file.content, &original[..original.len() - 1]),
                Fault::Oversized => {
                    assert_eq!(&file.content[..original.len()], original);
                    assert_eq!(file.content.len(), original.len() + 1);
                }
                _ => assert_eq!(file.content, original),
            }
        }
        assert_eq!(
            state.files.len(),
            if fault == Fault::Missing { 1 } else { 2 }
        );
    }
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_empty_source_requires_checked_eof_and_close_before_target() -> TestResult {
    let fixture = Fixture::start(b"", Fault::None, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    let backup = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, b"new", &|| true)
        .await?;
    assert_eq!(backup, fixture.backup());
    fixture.assert_complete_read_before_target(b"", b"new");
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_unicode_multichunk_readback_precedes_target_staging() -> TestResult {
    let original = "中文🙂".repeat(10_000).into_bytes();
    let fixture = Fixture::start(&original, Fault::None, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    let replacement = "新内容".as_bytes();
    let backup = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, replacement, &|| true)
        .await?;
    assert_eq!(backup, fixture.backup());
    fixture.assert_complete_read_before_target(&original, replacement);
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_utf16_keeps_bom_and_raw_bytes_in_checked_readback() -> TestResult {
    let original = b"\xff\xfe\x2d\x4e\x87\x65\r\0\n\0";
    let replacement = b"\xff\xfe\xb0\x65\r\0\n\0";
    let fixture = Fixture::start(original, Fault::None, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    let backup = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, replacement, &|| true)
        .await?;
    assert_eq!(backup, fixture.backup());
    fixture.assert_complete_read_before_target(original, replacement);
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_explicit_legacy_formats_read_back_original_raw_bytes() -> TestResult {
    for (encoding, original, replacement) in [
        (
            TextEncoding::Gbk,
            b"\xd6\xd0\r\nold".as_slice(),
            b"\xc4\xe3\r\nnew".as_slice(),
        ),
        (
            TextEncoding::Gb18030,
            b"\x90\x30\x81\x30\r\nold",
            b"\x90\x30\x81\x30\r\nnew",
        ),
        (TextEncoding::Big5, b"\xa4\xa4\r\nold", b"\xa7\x41\r\nnew"),
        (
            TextEncoding::ShiftJis,
            b"\x93\xfa\r\nold",
            b"\x93\x8c\r\nnew",
        ),
        (
            TextEncoding::Windows1252,
            b"caf\xe9\r\nold",
            b"na\xefve\r\nnew",
        ),
    ] {
        let fixture = Fixture::start(original, Fault::None, HoldPoint::None).await?;
        let reviewed = fixture.review().await?;
        let format = TextDocument::decode_with_encoding(original, encoding)?.format();
        let backup = fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, replacement, format, &|| true)
            .await?;
        assert_eq!(backup, fixture.backup());
        fixture.assert_complete_read_before_target(original, replacement);
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn editor_backup_explicit_legacy_corruption_blocks_target() -> TestResult {
    let original = b"\xd6\xd0\r\nprivate original";
    let fixture = Fixture::start(original, Fault::CorruptTail, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    let format = TextDocument::decode_with_encoding(original, TextEncoding::Gbk)?.format();
    assert_unverified(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                b"\xc4\xe3\r\nnew",
                format,
                &|| true,
            )
            .await,
    );
    fixture.assert_target_untouched(&reviewed);
    let backup = fixture.backup();
    assert_ne!(fixture.shared.state().files[&backup].content, original);
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_acknowledged_tail_corruption_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::CorruptTail, &vec![b'x'; 96 * 1024 + 1]).await
}

#[tokio::test]
async fn editor_backup_acknowledged_truncation_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::Truncate, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_missing_leaf_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::Missing, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_symlink_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::Symlink, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_directory_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::Directory, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_missing_type_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::MissingType, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_missing_size_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::MissingSize, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_wrong_mode_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::WrongMode, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_oversized_content_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::Oversized, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_read_failure_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::ReadFailure, b"private original").await
}

#[tokio::test]
async fn editor_backup_acknowledged_early_eof_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::EarlyEof, &vec![b'x'; 64 * 1024 + 1]).await
}

#[tokio::test]
async fn editor_backup_opened_handle_metadata_change_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::OpenedMetadataChange, b"private original").await
}

#[tokio::test]
async fn editor_backup_metadata_change_during_read_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::ReadMetadataChange, b"private original").await
}

#[tokio::test]
async fn editor_backup_leaf_metadata_change_after_handle_check_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::LeafMetadataChange, b"private original").await
}

#[tokio::test]
async fn editor_backup_readonly_close_failure_blocks_target() -> TestResult {
    rejects_acknowledged_backup(Fault::CloseFailure, b"private original").await
}

#[tokio::test]
async fn editor_backup_readback_keeps_shared_mutation_reservation() -> TestResult {
    let fixture = Fixture::start(b"original", Fault::None, HoldPoint::Read).await?;
    let reviewed = fixture.review().await?;
    let writer = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, b"new", &|| true);
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("backup READ was not held: {result:?}").into()),
        observed = reached(|| fixture.shared.gate.entered.load(Ordering::Acquire) > 0) => observed?,
    }
    fixture.assert_target_untouched(&reviewed);
    let backup = fixture.backup();
    let sibling = fixture.session.sftp().await?;
    assert!(matches!(
        sibling.write_atomic(&backup, b"bypass").await,
        Err(SessionError::MutationBusy)
    ));
    fixture.shared.gate.release();
    assert_eq!(writer.await?, backup);
    fixture.assert_complete_read_before_target(&reviewed.content, b"new");
    sibling.close().await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_revoked_after_publication_keeps_closed_error_and_original() -> TestResult {
    let fixture =
        Fixture::start(b"original", Fault::RevokeAfterPublication, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    assert!(matches!(
        fixture
            .sftp
            .write_editor_reviewed_authorized(&reviewed, b"new", &|| fixture.authorized())
            .await,
        Err(SessionError::Closed)
    ));
    fixture.assert_target_untouched(&reviewed);
    assert!(!fixture.shared.state().events.contains(&Event::BackupOpened));
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&fixture.backup()).await?;
    fixture.finish().await
}

async fn revoked_while_held(fault: Fault, hold: HoldPoint) -> TestResult {
    let fixture = Fixture::start(b"original", fault, hold).await?;
    let reviewed = fixture.review().await?;
    let authorize = || fixture.authorized();
    let writer = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, b"new", &authorize);
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("backup readback was not held: {result:?}").into()),
        observed = reached(|| fixture.shared.gate.entered.load(Ordering::Acquire) > 0) => observed?,
    }
    fixture.assert_target_untouched(&reviewed);
    fixture.shared.allowed.store(false, Ordering::Release);
    fixture.shared.gate.release();
    assert!(matches!(writer.await, Err(SessionError::Closed)));
    fixture.assert_target_untouched(&reviewed);
    let backup = fixture.backup();
    assert_eq!(
        fixture.shared.state().files[&backup].content,
        reviewed.content
    );
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_revoked_during_read_blocks_target() -> TestResult {
    revoked_while_held(Fault::None, HoldPoint::Read).await
}

#[tokio::test]
async fn editor_backup_revoked_during_readonly_close_blocks_target() -> TestResult {
    revoked_while_held(Fault::None, HoldPoint::ReadClose).await
}

#[tokio::test]
async fn editor_backup_revoked_during_failed_read_keeps_closed_error() -> TestResult {
    revoked_while_held(Fault::ReadFailure, HoldPoint::Read).await
}

#[tokio::test]
async fn editor_backup_revoked_on_final_close_ack_blocks_target() -> TestResult {
    let fixture = Fixture::start(b"original", Fault::RevokeOnClose, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    assert!(matches!(
        fixture
            .sftp
            .write_editor_reviewed_authorized(&reviewed, b"new", &|| fixture.authorized())
            .await,
        Err(SessionError::Closed)
    ));
    fixture.assert_target_untouched(&reviewed);
    assert!(fixture.shared.state().events.contains(&Event::BackupClosed));
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&fixture.backup()).await?;
    fixture.finish().await
}

async fn readback_times_out(revoke: bool) -> TestResult {
    let mut fixture = Fixture::start(b"original", Fault::None, HoldPoint::Read).await?;
    let reviewed = fixture.review().await?;
    // Keep normal setup/review budgets; only the editor operation is shortened.
    fixture.sftp.timeout = Duration::from_secs(2);
    let authorize = || fixture.authorized();
    let writer = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, b"new", &authorize);
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("backup READ was not held: {result:?}").into()),
        observed = reached(|| fixture.shared.gate.entered.load(Ordering::Acquire) > 0) => observed?,
    }
    if revoke {
        fixture.shared.allowed.store(false, Ordering::Release);
        assert!(matches!(writer.await, Err(SessionError::Closed)));
    } else {
        assert_unverified(writer.await);
    }
    fixture.shared.gate.release();
    reached(|| fixture.shared.gate.pending.load(Ordering::Acquire) == 0).await?;
    fixture.assert_target_untouched(&reviewed);
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&fixture.backup()).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_readback_timeout_is_unverified_without_target_quarantine() -> TestResult {
    readback_times_out(false).await
}

#[tokio::test]
async fn editor_backup_revoked_during_readback_timeout_keeps_closed_error() -> TestResult {
    readback_times_out(true).await
}

#[tokio::test]
async fn editor_backup_unknown_publication_keeps_shared_quarantine_and_original() -> TestResult {
    let fixture = Fixture::start(b"original", Fault::UnknownPublication, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    assert!(matches!(
        fixture
            .sftp
            .write_editor_reviewed_authorized(&reviewed, b"new", &|| true)
            .await,
        Err(SessionError::MutationUncertain)
    ));
    fixture.assert_target_untouched(&reviewed);
    let backup = fixture.backup();
    assert_eq!(
        fixture.shared.state().files[&backup].content,
        reviewed.content
    );
    assert!(!fixture.shared.state().events.contains(&Event::BackupOpened));
    assert!(
        !fixture
            .sftp
            .inspect_remote_mutation_quarantine(&backup)
            .await?
            .entries()
            .is_empty()
    );
    let sibling = fixture.session.sftp().await?;
    assert!(matches!(
        sibling.write_atomic(&backup, b"bypass").await,
        Err(SessionError::MutationQuarantined)
    ));
    fixture.assert_no_quarantine(TARGET).await?;
    sibling.close().await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_backup_readback_does_not_skip_original_target_revalidation() -> TestResult {
    let fixture = Fixture::start(b"original", Fault::ChangeTargetOnClose, HoldPoint::None).await?;
    let reviewed = fixture.review().await?;
    let result = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, b"new", &|| true)
        .await;
    assert!(
        matches!(
            result,
            Err(SessionError::Invalid(
                "reviewed file content or metadata changed"
            ))
        ),
        "an equal-size competing write must fail the original byte review: {result:?}",
    );
    let backup = fixture.backup();
    {
        let state = fixture.shared.state();
        assert_eq!(state.files[TARGET].content, b"external");
        assert_eq!(state.files[&backup].content, reviewed.content);
        assert!(state.events.contains(&Event::BackupClosed));
        assert!(!state.events.contains(&Event::TargetStaged));
        assert!(!state.events.contains(&Event::TargetPublished));
    }
    fixture.assert_no_quarantine(TARGET).await?;
    fixture.assert_no_quarantine(&backup).await?;
    fixture.finish().await
}
