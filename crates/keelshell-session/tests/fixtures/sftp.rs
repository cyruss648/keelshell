//! Deterministic filesystem fixture behind real SFTP packets.
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Scenario controls/timeline are used by the separate metadata TCP binary.
// Other targets still use the same packet handler with cadence disabled.
#[allow(dead_code)]
#[path = "metadata_cadence.rs"]
mod metadata_cadence;
use metadata_cadence::MetadataCadence;
pub use metadata_cadence::MetadataKind;

/// Holds a fixture response until the test has observed its operation timeout.
/// The fallback deadline also bounds cleanup if the test fails before release.
#[derive(Default)]
pub struct ResponseGate {
    armed: AtomicBool,
    entered: AtomicUsize,
    release: tokio::sync::Notify,
}

impl ResponseGate {
    pub fn arm(&self) {
        self.armed.store(true, Ordering::Release);
    }
    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }
    pub fn entered(&self) -> usize {
        self.entered.load(Ordering::Acquire)
    }
    pub fn release(&self) {
        // notify_one retains a permit if the handler has not yet been polled.
        self.release.notify_one();
    }
    pub async fn hold(&self) -> Result<(), ()> {
        self.entered.fetch_add(1, Ordering::AcqRel);
        tokio::time::timeout(Duration::from_secs(10), self.release.notified())
            .await
            .map_err(|_| ())
    }
}

/// Holds nonzero-offset writes for one target of a fresh reviewed upload.
/// Watch state preserves release-before-poll and separates successive holds.
struct TransferWriteGate {
    state: tokio::sync::watch::Sender<Option<Arc<TransferWriteState>>>,
}
struct TransferWriteState {
    target: String,
    prefix: bool,
    entered: AtomicUsize,
    expired: AtomicBool,
}
impl Default for TransferWriteGate {
    fn default() -> Self {
        Self {
            state: tokio::sync::watch::channel(None).0,
        }
    }
}
impl TransferWriteGate {
    async fn hold(&self, handle: &str, offset: u64, deadline: Duration) -> Result<(), ()> {
        let mut state = self.state.subscribe();
        let owned = match state.borrow_and_update().as_ref() {
            Some(owned)
                if offset > 0
                    && (owned.target == handle
                        || (owned.prefix && handle.starts_with(&owned.target))
                        || (owned.target.is_empty() && handle.contains(".keelshell-"))) =>
            {
                owned.clone()
            }
            _ => return Ok(()),
        };
        owned.entered.fetch_add(1, Ordering::AcqRel);
        // Each hold owns its counters and identity. An old handler cannot
        // attribute a delayed entry/expiry to a newly armed hold for this path.
        // No filesystem mutex is held across this wait.
        let result = tokio::time::timeout(deadline, async {
            loop {
                if !state
                    .borrow_and_update()
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &owned))
                {
                    return Ok(());
                }
                state.changed().await.map_err(|_| ())?;
            }
        })
        .await
        .map_err(|_| ());
        match result {
            Ok(result) => result,
            Err(()) => {
                owned.expired.store(true, Ordering::Release);
                Err(())
            }
        }
    }
}

/// Releases a fixture WRITE even when its test fails or unwinds.
/// Dropping a cloned Filesystem does not release another owner's active hold.
pub struct TransferWriteHold {
    gate: Arc<TransferWriteGate>,
    owned: Arc<TransferWriteState>,
}
impl TransferWriteHold {
    pub fn entered(&self) -> usize {
        self.owned.entered.load(Ordering::Acquire)
    }
    pub fn expired(&self) -> bool {
        self.owned.expired.load(Ordering::Acquire)
    }
    pub fn release(self) {
        // The consuming operation has the same cleanup behavior as Drop.
    }
}
impl Drop for TransferWriteHold {
    fn drop(&mut self) {
        self.gate.state.send_if_modified(|state| {
            if state
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.owned))
            {
                *state = None;
                true
            } else {
                false
            }
        });
    }
}

#[derive(Default)]
struct CanonicalPathGate {
    state: tokio::sync::watch::Sender<Option<Arc<CanonicalPathState>>>,
}
struct CanonicalPathState {
    path: String,
    entered: AtomicUsize,
    expired: AtomicBool,
}
impl CanonicalPathGate {
    async fn hold(&self, path: &str, deadline: Duration) -> Result<(), ()> {
        let mut state = self.state.subscribe();
        let owned = match state.borrow_and_update().as_ref() {
            Some(owned) if owned.path == path => owned.clone(),
            _ => return Ok(()),
        };
        owned.entered.fetch_add(1, Ordering::AcqRel);
        let result = tokio::time::timeout(deadline, async {
            loop {
                if !state
                    .borrow_and_update()
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &owned))
                {
                    return Ok(());
                }
                state.changed().await.map_err(|_| ())?;
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(_) => {
                owned.expired.store(true, Ordering::Release);
                Err(())
            }
        }
    }
}

/// Holds only REALPATH replies for an exact fixture path until this owner drops.
/// The fallback deadline bounds cleanup after a failed or unwinding test.
pub struct CanonicalPathHold {
    gate: Arc<CanonicalPathGate>,
    owned: Arc<CanonicalPathState>,
}
impl CanonicalPathHold {
    pub fn entered(&self) -> usize {
        self.owned.entered.load(Ordering::Acquire)
    }
    pub fn expired(&self) -> bool {
        self.owned.expired.load(Ordering::Acquire)
    }
    pub fn release(self) {}
}
impl Drop for CanonicalPathHold {
    fn drop(&mut self) {
        self.gate.state.send_if_modified(|state| {
            if state
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.owned))
            {
                *state = None;
                true
            } else {
                false
            }
        });
    }
}

/// CLOSE has a separate mode-aware barrier: validation/source READ handles
/// cannot accidentally satisfy a writable-target test's synchronization point.
#[derive(Default)]
struct CloseGate {
    state: tokio::sync::watch::Sender<Option<Arc<CloseState>>>,
}
struct CloseState {
    path: String,
    prefix: bool,
    writable: bool,
    entered: AtomicUsize,
    expired: AtomicBool,
    pending: AtomicUsize,
}
struct CloseLease(Arc<CloseState>);
impl Drop for CloseLease {
    fn drop(&mut self) {
        self.0.pending.fetch_sub(1, Ordering::AcqRel);
    }
}
impl CloseGate {
    async fn hold(&self, path: &str, writable: bool) -> Result<(), ()> {
        let mut state = self.state.subscribe();
        let owned = match state.borrow_and_update().as_ref() {
            Some(owned)
                if owned.writable == writable
                    && (owned.path == path || (owned.prefix && path.starts_with(&owned.path))) =>
            {
                owned.clone()
            }
            _ => return Ok(()),
        };
        owned.pending.fetch_add(1, Ordering::AcqRel);
        let _lease = CloseLease(owned.clone());
        owned.entered.fetch_add(1, Ordering::AcqRel);
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if !state
                    .borrow_and_update()
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &owned))
                {
                    return Ok(());
                }
                state.changed().await.map_err(|_| ())?;
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(_) => {
                owned.expired.store(true, Ordering::Release);
                Err(())
            }
        }
    }
}
/// Owns an exact writable/readonly CLOSE reply barrier and its actual in-flight
/// handler count. Drop releases only this generation; fallback is never proof.
pub struct CloseHold {
    gate: Arc<CloseGate>,
    owned: Arc<CloseState>,
}
impl CloseHold {
    pub fn entered(&self) -> usize {
        self.owned.entered.load(Ordering::Acquire)
    }
    pub fn pending(&self) -> usize {
        self.owned.pending.load(Ordering::Acquire)
    }
    pub fn expired(&self) -> bool {
        self.owned.expired.load(Ordering::Acquire)
    }
    pub fn release(&self) {
        self.gate.state.send_if_modified(|state| {
            if state
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.owned))
            {
                *state = None;
                true
            } else {
                false
            }
        });
    }
}
impl Drop for CloseHold {
    fn drop(&mut self) {
        self.release();
    }
}

#[derive(Default)]
pub struct Filesystem {
    state: std::sync::Arc<std::sync::Mutex<FileState>>,
    open_handles: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    opened: HashSet<String>,
    unsupported_atomic: Arc<AtomicBool>,
    unexpected_atomic_reply: Arc<AtomicBool>,
    fail_atomic_write: Arc<AtomicBool>,
    reject_resume_create: Arc<AtomicBool>,
    stall_atomic_write: Arc<AtomicBool>,
    atomic_writes: Arc<AtomicUsize>,
    transfer_write_delay: Arc<AtomicUsize>,
    transfer_read_delay: Arc<AtomicUsize>,
    pub metadata_cadence: Arc<MetadataCadence>,
    transfer_write_gate: Arc<TransferWriteGate>,
    transfer_read_gate: Arc<TransferWriteGate>,
    transfer_read_limit: Arc<AtomicUsize>,
    transfer_writes: Arc<AtomicUsize>,
    prepared_successful_write_statuses: Arc<AtomicUsize>,
    invalid_transfer_read: Arc<AtomicUsize>,
    injected_name: Arc<std::sync::Mutex<Option<String>>>,
    directory_read_gate: Arc<ResponseGate>,
    canonical_path_gate: Arc<CanonicalPathGate>,
    metadata_path_gate: Arc<CanonicalPathGate>,
    remove_path_gate: Arc<CanonicalPathGate>,
    close_gate: Arc<CloseGate>,
    writable_handles: HashSet<String>,
    writable_closes: Arc<AtomicUsize>,
    reject_writable_close: Arc<AtomicBool>,
}

impl Clone for Filesystem {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            open_handles: self.open_handles.clone(),
            opened: HashSet::new(),
            unsupported_atomic: self.unsupported_atomic.clone(),
            unexpected_atomic_reply: self.unexpected_atomic_reply.clone(),
            fail_atomic_write: self.fail_atomic_write.clone(),
            reject_resume_create: self.reject_resume_create.clone(),
            stall_atomic_write: self.stall_atomic_write.clone(),
            atomic_writes: self.atomic_writes.clone(),
            transfer_write_delay: self.transfer_write_delay.clone(),
            transfer_read_delay: self.transfer_read_delay.clone(),
            metadata_cadence: self.metadata_cadence.clone(),
            transfer_write_gate: self.transfer_write_gate.clone(),
            transfer_read_gate: self.transfer_read_gate.clone(),
            transfer_read_limit: self.transfer_read_limit.clone(),
            transfer_writes: self.transfer_writes.clone(),
            prepared_successful_write_statuses: self.prepared_successful_write_statuses.clone(),
            invalid_transfer_read: self.invalid_transfer_read.clone(),
            injected_name: self.injected_name.clone(),
            directory_read_gate: self.directory_read_gate.clone(),
            canonical_path_gate: self.canonical_path_gate.clone(),
            metadata_path_gate: self.metadata_path_gate.clone(),
            remove_path_gate: self.remove_path_gate.clone(),
            close_gate: self.close_gate.clone(),
            writable_handles: HashSet::new(),
            writable_closes: self.writable_closes.clone(),
            reject_writable_close: self.reject_writable_close.clone(),
        }
    }
}
impl Filesystem {
    /// Simulate a filesystem change made outside application admission. Tests
    /// must use this explicitly; production SFTP writers now share isolation.
    pub fn replace_external_file(&self, path: &str, bytes: &[u8]) -> Result<(), &'static str> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "fixture filesystem poisoned")?;
        if state.directories.contains(path) {
            return Err("external fixture target is a directory");
        }
        state.files.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }
    /// Observe a production root validation on one exact path without delaying
    /// unrelated SFTP operations or releasing another test owner's hold.
    pub fn hold_canonical_path(&self, path: &str) -> Result<CanonicalPathHold, &'static str> {
        Self::hold_exact_path(&self.canonical_path_gate, path)
    }
    /// Own an exact LSTAT hold for observing revocation during risk confirmation.
    pub fn hold_metadata_path(&self, path: &str) -> Result<CanonicalPathHold, &'static str> {
        Self::hold_exact_path(&self.metadata_path_gate, path)
    }
    /// Hold an owned temporary's REMOVE reply before actual fixture deletion.
    pub fn hold_remove_path(&self, path: &str) -> Result<CanonicalPathHold, &'static str> {
        Self::hold_exact_path(&self.remove_path_gate, path)
    }
    fn hold_exact_path(
        gate: &Arc<CanonicalPathGate>,
        path: &str,
    ) -> Result<CanonicalPathHold, &'static str> {
        let owned = Arc::new(CanonicalPathState {
            path: path.to_owned(),
            entered: AtomicUsize::new(0),
            expired: AtomicBool::new(false),
        });
        let mut armed = false;
        gate.state.send_if_modified(|state| {
            if state.is_some() {
                return false;
            }
            *state = Some(owned.clone());
            armed = true;
            true
        });
        if !armed {
            return Err("fixture already has an owned exact path hold");
        }
        Ok(CanonicalPathHold {
            gate: gate.clone(),
            owned,
        })
    }
    pub fn hold_close(
        &self,
        path: &str,
        prefix: bool,
        writable: bool,
    ) -> Result<CloseHold, &'static str> {
        let owned = Arc::new(CloseState {
            path: path.to_owned(),
            prefix,
            writable,
            entered: AtomicUsize::new(0),
            expired: AtomicBool::new(false),
            pending: AtomicUsize::new(0),
        });
        let mut armed = false;
        self.close_gate.state.send_if_modified(|state| {
            if state.is_some() {
                return false;
            }
            *state = Some(owned.clone());
            armed = true;
            true
        });
        if !armed {
            return Err("fixture already has an owned CLOSE hold");
        }
        Ok(CloseHold {
            gate: self.close_gate.clone(),
            owned,
        })
    }
    pub fn writable_closes_started(&self) -> usize {
        self.writable_closes.load(Ordering::Acquire)
    }
    pub fn reject_writable_closes(&self, reject: bool) {
        self.reject_writable_close.store(reject, Ordering::Release);
    }
    pub fn stall_directory_reads(&self) {
        self.directory_read_gate.arm();
    }
    pub fn directory_reads_stalled(&self) -> usize {
        self.directory_read_gate.entered()
    }
    pub fn release_directory_reads(&self) {
        self.directory_read_gate.release();
    }
    /// Use only for a fresh upload whose first WRITE starts at offset zero.
    /// Nonzero-offset WRITE requests for this exact handle wait before mutation
    /// and ACK; this is not a resume-transfer acknowledgement observer.
    pub fn hold_transfer_writes_after_first(
        &self,
        target: &str,
    ) -> Result<TransferWriteHold, &'static str> {
        self.hold_transfer_target(target.to_owned(), false)
    }
    /// Hold one upload's generated same-directory atomic temporary by prefix.
    pub fn hold_atomic_upload_after_first(
        &self,
        target: &str,
    ) -> Result<TransferWriteHold, &'static str> {
        let (parent, name) = target
            .rsplit_once('/')
            .ok_or("atomic fixture requires absolute destination")?;
        self.hold_transfer_target(format!("{parent}/.{name}.keelshell-"), true)
    }
    fn hold_transfer_target(
        &self,
        target: String,
        prefix: bool,
    ) -> Result<TransferWriteHold, &'static str> {
        let owned = Arc::new(TransferWriteState {
            target,
            prefix,
            entered: AtomicUsize::new(0),
            expired: AtomicBool::new(false),
        });
        let mut armed = false;
        self.transfer_write_gate.state.send_if_modified(|state| {
            if state.is_some() {
                return false;
            }
            *state = Some(owned.clone());
            armed = true;
            true
        });
        if !armed {
            return Err("fixture already has an owned WRITE hold");
        }
        Ok(TransferWriteHold {
            gate: self.transfer_write_gate.clone(),
            owned,
        })
    }
    /// Hold all nonzero-offset READ requests on independently owned handlers.
    pub fn hold_transfer_reads_after_first(&self) -> Result<TransferWriteHold, &'static str> {
        let owned = Arc::new(TransferWriteState {
            target: String::new(),
            prefix: true,
            entered: AtomicUsize::new(0),
            expired: AtomicBool::new(false),
        });
        let mut armed = false;
        self.transfer_read_gate.state.send_if_modified(|state| {
            if state.is_some() {
                return false;
            }
            *state = Some(owned.clone());
            armed = true;
            true
        });
        if !armed {
            return Err("fixture already has an owned READ hold");
        }
        Ok(TransferWriteHold {
            gate: self.transfer_read_gate.clone(),
            owned,
        })
    }
    pub fn set_invalid_transfer_read(&self, mode: usize) {
        self.invalid_transfer_read.store(mode, Ordering::Release);
    }
    /// Own a WRITE hold for a generated atomic temporary after its first chunk.
    pub fn hold_atomic_writes_after_first(&self) -> Result<TransferWriteHold, &'static str> {
        self.hold_transfer_target(String::new(), false)
    }
    pub fn transfer_writes_started(&self) -> usize {
        self.transfer_writes.load(Ordering::Acquire)
    }
    /// Successful WRITE statuses prepared by the handler, before transport delivery.
    /// This counter is separate from metadata idle observations and proves no ACK receipt.
    pub fn prepared_successful_write_statuses(&self) -> usize {
        self.prepared_successful_write_statuses
            .load(Ordering::Acquire)
    }
    pub fn set_transfer_read_limit(&self, bytes: usize) {
        self.transfer_read_limit.store(bytes, Ordering::Release);
    }
    pub fn set_transfer_write_delay(&self, milliseconds: usize) {
        self.transfer_write_delay
            .store(milliseconds, Ordering::Release);
    }
    // Used by the separate slow-transfer integration target, not every fixture.
    #[allow(dead_code)]
    pub fn set_transfer_read_delay(&self, milliseconds: usize) {
        self.transfer_read_delay
            .store(milliseconds, Ordering::Release);
    }
    pub fn set_injected_name(&self, name: Option<&str>) -> Result<(), &'static str> {
        *self
            .injected_name
            .lock()
            .map_err(|_| "fixture name mutex poisoned")? = name.map(str::to_owned);
        Ok(())
    }
    pub fn insert_symlink(&self, path: &str) -> Result<(), &'static str> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "fixture state mutex poisoned")?;
        state.files.insert(path.into(), b"target".to_vec());
        state.modes.insert(path.into(), 0o120777);
        Ok(())
    }
    pub fn active_directory_handles(&self) -> usize {
        self.open_handles.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn set_file_mtime(&self, path: &str, value: u32) -> Result<(), &'static str> {
        let mut state = self.state.lock().map_err(|_| "fixture state poisoned")?;
        if !state.files.contains_key(path) {
            return Err("fixture file missing");
        }
        state.mtimes.insert(path.to_owned(), value);
        Ok(())
    }
    pub fn set_atomic_unsupported(&self, value: bool) {
        self.unsupported_atomic.store(value, Ordering::Release);
    }
    pub fn set_unexpected_atomic_reply(&self, value: bool) {
        self.unexpected_atomic_reply.store(value, Ordering::Release);
    }
    pub fn set_resume_create_rejection(&self, value: bool) {
        self.reject_resume_create.store(value, Ordering::Release);
    }
    pub fn set_atomic_write_failure(&self, value: bool) {
        self.fail_atomic_write.store(value, Ordering::Release);
    }
    pub fn set_atomic_write_stall(&self, value: bool) {
        self.stall_atomic_write.store(value, Ordering::Release);
    }
    pub fn atomic_writes_started(&self) -> usize {
        self.atomic_writes.load(Ordering::Acquire)
    }
}
impl Drop for Filesystem {
    fn drop(&mut self) {
        self.open_handles
            .fetch_sub(self.opened.len(), std::sync::atomic::Ordering::AcqRel);
    }
}

#[derive(Default)]
struct FileState {
    files: HashMap<String, Vec<u8>>,
    directories: HashSet<String>,
    read_directories: HashSet<String>,
    modes: HashMap<String, u32>,
    mtimes: HashMap<String, u32>,
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: "en".into(),
    }
}

impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        let mut version = Version::new();
        if !self.unsupported_atomic.load(Ordering::Acquire) {
            version
                .extensions
                .insert("posix-rename@openssh.com".into(), "1".into());
        }
        Ok(version)
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.metadata_cadence.wait(MetadataKind::Lstat).await?;
        self.metadata_path_gate
            .hold(&path, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        let state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if path == "/" || state.directories.contains(&path) {
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o40000 | state.modes.get(&path).copied().unwrap_or(0o700));
            self.metadata_cadence.record(MetadataKind::Lstat, true)?;
            return Ok(Attrs { id, attrs });
        }
        let Some(file) = state.files.get(&path) else {
            self.metadata_cadence.record(MetadataKind::Lstat, true)?;
            return Err(StatusCode::NoSuchFile);
        };
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(file.len() as u64);
        attrs.permissions = Some(0o100000 | state.modes.get(&path).copied().unwrap_or(0o644));
        attrs.mtime = state.mtimes.get(&path).copied();
        attrs.atime = attrs.mtime;
        self.metadata_cadence.record(MetadataKind::Lstat, true)?;
        Ok(Attrs { id, attrs })
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        self.metadata_cadence.wait(MetadataKind::Fstat).await?;
        let reply = self.lstat(id, handle).await;
        self.metadata_cadence.record(MetadataKind::Fstat, true)?;
        reply
    }
    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, StatusCode> {
        if request != "posix-rename@openssh.com" || self.unsupported_atomic.load(Ordering::Acquire)
        {
            return Err(StatusCode::OpUnsupported);
        }
        fn string(input: &mut &[u8]) -> Result<String, StatusCode> {
            let length = u32::from_be_bytes(
                input
                    .get(..4)
                    .ok_or(StatusCode::BadMessage)?
                    .try_into()
                    .map_err(|_| StatusCode::BadMessage)?,
            ) as usize;
            *input = &input[4..];
            let text = std::str::from_utf8(input.get(..length).ok_or(StatusCode::BadMessage)?)
                .map_err(|_| StatusCode::BadMessage)?
                .to_owned();
            *input = &input[length..];
            Ok(text)
        }
        let mut input = data.as_slice();
        let from = string(&mut input)?;
        let to = string(&mut input)?;
        if !input.is_empty() {
            return Err(StatusCode::BadMessage);
        }
        let status = self.rename(id, from, to).await?;
        if self.unexpected_atomic_reply.load(Ordering::Acquire) {
            Ok(Packet::Data(russh_sftp::protocol::Data {
                id,
                data: b"not a rename STATUS".to_vec(),
            }))
        } else {
            Ok(Packet::Status(status))
        }
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let readonly =
            !flags.intersects(OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE);
        if readonly {
            self.metadata_cadence.wait(MetadataKind::ReadOpen).await?;
        }
        if flags.contains(OpenFlags::CREATE)
            && filename.ends_with("/create-rejected")
            && self.reject_resume_create.load(Ordering::Acquire)
        {
            return Err(StatusCode::PermissionDenied);
        }
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if flags.contains(OpenFlags::EXCLUDE) && state.files.contains_key(&filename) {
            return Err(StatusCode::Failure);
        }
        if flags.contains(OpenFlags::CREATE) {
            state.files.entry(filename.clone()).or_default();
            state
                .modes
                .insert(filename.clone(), attrs.permissions.unwrap_or(0o644));
        }
        let file = state
            .files
            .get_mut(&filename)
            .ok_or(StatusCode::NoSuchFile)?;
        if flags.contains(OpenFlags::TRUNCATE) {
            file.clear();
        }
        if flags.contains(OpenFlags::WRITE) {
            self.writable_handles.insert(filename.clone());
        }
        if readonly {
            self.metadata_cadence.record(MetadataKind::ReadOpen, true)?;
        }
        Ok(Handle {
            id,
            handle: filename,
        })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        // Sending CLOSE invalidates this handle even if its STATUS is delayed.
        let writable = self.writable_handles.remove(&handle);
        if !writable {
            self.metadata_cadence.wait(MetadataKind::ReadClose).await?;
        }
        if writable {
            self.writable_closes.fetch_add(1, Ordering::AcqRel);
        }
        self.close_gate
            .hold(&handle, writable)
            .await
            .map_err(|_| StatusCode::Failure)?;
        if writable && self.reject_writable_close.load(Ordering::Acquire) {
            return Err(StatusCode::PermissionDenied);
        }
        if self.opened.remove(&handle) {
            self.open_handles
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        }
        if !writable {
            self.metadata_cadence
                .record(MetadataKind::ReadClose, true)?;
        }
        Ok(ok(id))
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        bytes: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        self.transfer_writes.fetch_add(1, Ordering::AcqRel);
        self.transfer_write_gate
            .hold(&handle, offset, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        let delay = self.transfer_write_delay.load(Ordering::Acquire);
        if delay > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
        }
        if handle.contains(".keelshell-") {
            self.atomic_writes.fetch_add(1, Ordering::AcqRel);
            if offset > 0 && self.fail_atomic_write.load(Ordering::Acquire) {
                return Err(StatusCode::Failure);
            }
            if offset > 0 && self.stall_atomic_write.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        let file = state.files.get_mut(&handle).ok_or(StatusCode::NoSuchFile)?;
        let end = offset.checked_add(bytes.len()).ok_or(StatusCode::Failure)?;
        if end > 1024 * 1024 {
            return Err(StatusCode::Failure);
        }
        file.resize(file.len().max(end), 0);
        file[offset..end].copy_from_slice(&bytes);
        self.prepared_successful_write_statuses
            .fetch_add(1, Ordering::Release);
        Ok(ok(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
        let delay = self.transfer_read_delay.load(Ordering::Acquire);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        }
        self.transfer_read_gate
            .hold(&handle, offset, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        let state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        let file = state.files.get(&handle).ok_or(StatusCode::NoSuchFile)?;
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        if offset >= file.len() {
            return Err(StatusCode::Eof);
        }
        let limit = self.transfer_read_limit.load(Ordering::Acquire);
        let length = if limit == 0 {
            length as usize
        } else {
            (length as usize).min(limit)
        };
        let mut data = file[offset..file.len().min(offset + length)].to_vec();
        match self.invalid_transfer_read.load(Ordering::Acquire) {
            1 => data.clear(),
            2 => data.resize(length.saturating_add(1), 0),
            _ => {}
        }
        Ok(Data { id, data })
    }
    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _: FileAttributes,
    ) -> Result<Status, StatusCode> {
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if state.files.contains_key(&path) || !state.directories.insert(path) {
            return Err(StatusCode::Failure);
        }
        Ok(ok(id))
    }
    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        self.remove_path_gate
            .hold(&path, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        let prefix = format!("{}/", path.trim_end_matches('/'));
        if state.files.keys().any(|child| child.starts_with(&prefix))
            || state
                .directories
                .iter()
                .any(|child| child.starts_with(&prefix))
        {
            return Err(StatusCode::Failure);
        }
        if !state.directories.remove(&path) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        self.remove_path_gate
            .hold(&path, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        state.files.remove(&path).ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, StatusCode> {
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        let data = state.files.remove(&old).ok_or(StatusCode::NoSuchFile)?;
        if let Some(mode) = state.modes.remove(&old) {
            state.modes.insert(new.clone(), mode);
        }
        state.files.insert(new, data);
        Ok(ok(id))
    }
    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, StatusCode> {
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if path == "/" || state.directories.contains(&path) {
            if let Some(mode) = attrs.permissions {
                state.modes.insert(path, mode & 0o7777);
            }
            return Ok(ok(id));
        }
        if !state.files.contains_key(&path) {
            return Err(StatusCode::NoSuchFile);
        }
        if let Some(mode) = attrs.permissions {
            state.modes.insert(path, mode & 0o7777);
        }
        Ok(ok(id))
    }
    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusCode> {
        self.metadata_cadence.wait(MetadataKind::OpenDir).await?;
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        state.read_directories.remove(&path);
        if self.opened.insert(path.clone()) {
            self.open_handles
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        self.metadata_cadence.record(MetadataKind::OpenDir, true)?;
        Ok(Handle { id, handle: path })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusCode> {
        self.metadata_cadence.wait(MetadataKind::ReadDir).await?;
        if handle == "/stall" {
            if self.directory_read_gate.is_armed() {
                self.directory_read_gate
                    .hold()
                    .await
                    .map_err(|_| StatusCode::Failure)?;
            } else {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        if handle == "/error" {
            return Err(StatusCode::PermissionDenied);
        }
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if !state.read_directories.insert(handle.clone()) {
            self.metadata_cadence.record(MetadataKind::ReadDir, true)?;
            return Err(StatusCode::Eof);
        }
        let prefix = format!("{}/", handle.trim_end_matches('/'));
        let mut files: Vec<File> = state
            .files
            .iter()
            .filter_map(|(path, data)| {
                let name = path.strip_prefix(&prefix)?;
                if name.contains('/') {
                    return None;
                }
                let mut attrs = FileAttributes::empty();
                attrs.size = Some(data.len() as u64);
                attrs.permissions =
                    Some(0o100000 | state.modes.get(path).copied().unwrap_or(0o644));
                Some(File::new(name, attrs))
            })
            .collect();
        files.extend(state.directories.iter().filter_map(|path| {
            let name = path.strip_prefix(&prefix)?;
            if name.contains('/') {
                return None;
            }
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o40000 | state.modes.get(path).copied().unwrap_or(0o700));
            Some(File::new(name, attrs))
        }));
        if let Some(name) = self
            .injected_name
            .lock()
            .map_err(|_| StatusCode::Failure)?
            .as_ref()
        {
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o100600);
            attrs.size = Some(1);
            files.push(File::new(name, attrs));
        }
        self.metadata_cadence.record(MetadataKind::ReadDir, true)?;
        Ok(Name { id, files })
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        self.metadata_cadence.wait(MetadataKind::Realpath).await?;
        self.canonical_path_gate
            .hold(&path, Duration::from_secs(10))
            .await
            .map_err(|_| StatusCode::Failure)?;
        self.metadata_cadence.record(MetadataKind::Realpath, true)?;
        Ok(Name {
            id,
            files: vec![File::dummy(if path == "." { "/".into() } else { path })],
        })
    }
}

#[cfg(test)]
mod canonical_path_gate_tests {
    use super::*;
    use std::{future::Future, task::Poll};

    async fn poll_held_once(future: &mut std::pin::Pin<Box<impl Future<Output = Result<(), ()>>>>) {
        std::future::poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn exact_path_hold_survives_clones_and_releases_on_owner_drop()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        let held = filesystem.hold_canonical_path("/approved/missing")?;
        let clone = filesystem.clone();
        assert!(clone.hold_canonical_path("/another").is_err());
        assert!(
            clone
                .canonical_path_gate
                .hold("/approved", Duration::from_secs(1))
                .await
                .is_ok()
        );
        let gate = clone.canonical_path_gate.clone();
        let mut pending = Box::pin(gate.hold("/approved/missing", Duration::from_secs(1)));
        poll_held_once(&mut pending).await;
        drop(clone);
        poll_held_once(&mut pending).await;
        assert_eq!(held.entered(), 1);
        assert!(!held.expired());
        drop(held);
        assert!(pending.await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn release_before_poll_and_old_handler_do_not_consume_new_hold()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        filesystem.hold_canonical_path("/same")?.release();
        assert!(
            filesystem
                .canonical_path_gate
                .hold("/same", Duration::from_secs(1))
                .await
                .is_ok()
        );
        let old = filesystem.hold_canonical_path("/same")?;
        let mut pending = Box::pin(
            filesystem
                .canonical_path_gate
                .hold("/same", Duration::from_secs(1)),
        );
        poll_held_once(&mut pending).await;
        old.release();
        let current = filesystem.hold_canonical_path("/same")?;
        assert!(pending.await.is_ok());
        assert_eq!(current.entered(), 0);
        let mut next = Box::pin(
            filesystem
                .canonical_path_gate
                .hold("/same", Duration::from_secs(1)),
        );
        poll_held_once(&mut next).await;
        assert_eq!(current.entered(), 1);
        assert!(!current.expired());
        current.release();
        assert!(next.await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn expired_reply_stays_owned_until_drop_and_cannot_rearm_silently()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        let held = filesystem.hold_canonical_path("/expired")?;
        assert!(
            filesystem
                .canonical_path_gate
                .hold("/expired", Duration::from_millis(1))
                .await
                .is_err()
        );
        assert_eq!(held.entered(), 1);
        assert!(held.expired());
        assert!(filesystem.hold_canonical_path("/expired").is_err());
        held.release();
        assert!(filesystem.hold_canonical_path("/expired").is_ok());
        Ok(())
    }
}

#[cfg(test)]
mod transfer_write_gate_tests {
    use super::*;
    use std::{future::Future, task::Poll};

    async fn poll_held_once(future: &mut std::pin::Pin<Box<impl Future<Output = Result<(), ()>>>>) {
        std::future::poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn first_write_unrelated_targets_and_release_before_poll_are_never_held()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        let hold = filesystem.hold_transfer_writes_after_first("/selected.bin")?;
        assert!(
            filesystem
                .hold_transfer_writes_after_first("/other.bin")
                .is_err()
        );
        let gate = filesystem.transfer_write_gate.clone();
        assert_eq!(
            gate.hold("/selected.bin", 0, Duration::from_millis(50))
                .await,
            Ok(())
        );
        assert_eq!(
            gate.hold("/unrelated.bin", 65536, Duration::from_millis(50))
                .await,
            Ok(())
        );
        assert_eq!(hold.entered(), 0);
        drop(filesystem.clone());
        assert!(
            gate.state.borrow().is_some(),
            "a subsystem clone cannot release the owned hold"
        );
        let pending = gate.hold("/selected.bin", 65536, Duration::from_millis(50));
        hold.release();
        assert_eq!(
            pending.await,
            Ok(()),
            "release before the handler's first poll is retained"
        );
        Ok(())
    }

    #[tokio::test]
    async fn guard_unwind_releases_an_entered_write_and_rearming_does_not_retrap_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        let hold = filesystem.hold_transfer_writes_after_first("/selected.bin")?;
        let gate = filesystem.transfer_write_gate.clone();
        let mut pending = Box::pin(gate.hold("/selected.bin", 65536, Duration::from_secs(1)));
        poll_held_once(&mut pending).await;
        assert_eq!(hold.entered(), 1);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owned_hold = hold;
            panic!("controlled fixture failure exercises owned release");
        }));
        assert!(unwind.is_err());
        let next = filesystem.hold_transfer_writes_after_first("/selected.bin")?;
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), pending).await,
            Ok(Ok(()))
        );
        assert!(
            gate.state.borrow().is_some(),
            "the new generation remains owned"
        );
        let mut next_pending = Box::pin(gate.hold("/selected.bin", 65536, Duration::from_secs(1)));
        poll_held_once(&mut next_pending).await;
        assert_eq!(next.entered(), 1);
        drop(next);
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), next_pending).await,
            Ok(Ok(()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_release_expires_as_failure_instead_of_admitting_a_write()
    -> Result<(), Box<dyn std::error::Error>> {
        let filesystem = Filesystem::default();
        let hold = filesystem.hold_transfer_writes_after_first("/selected.bin")?;
        assert_eq!(
            filesystem
                .transfer_write_gate
                .hold("/selected.bin", 65536, Duration::from_millis(20))
                .await,
            Err(())
        );
        assert_eq!(hold.entered(), 1);
        assert!(hold.expired());
        assert!(filesystem.transfer_write_gate.state.borrow().is_some());
        drop(hold);
        assert!(filesystem.transfer_write_gate.state.borrow().is_none());
        Ok(())
    }
}
