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
pub struct Filesystem {
    state: std::sync::Arc<std::sync::Mutex<FileState>>,
    open_handles: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    opened: HashSet<String>,
    unsupported_atomic: Arc<AtomicBool>,
    fail_atomic_write: Arc<AtomicBool>,
    stall_atomic_write: Arc<AtomicBool>,
    atomic_writes: Arc<AtomicUsize>,
    transfer_write_delay: Arc<AtomicUsize>,
    transfer_write_gate: Arc<TransferWriteGate>,
    transfer_read_limit: Arc<AtomicUsize>,
    transfer_writes: Arc<AtomicUsize>,
    invalid_transfer_read: Arc<AtomicUsize>,
    injected_name: Arc<std::sync::Mutex<Option<String>>>,
    directory_read_gate: Arc<ResponseGate>,
}

impl Clone for Filesystem {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            open_handles: self.open_handles.clone(),
            opened: HashSet::new(),
            unsupported_atomic: self.unsupported_atomic.clone(),
            fail_atomic_write: self.fail_atomic_write.clone(),
            stall_atomic_write: self.stall_atomic_write.clone(),
            atomic_writes: self.atomic_writes.clone(),
            transfer_write_delay: self.transfer_write_delay.clone(),
            transfer_write_gate: self.transfer_write_gate.clone(),
            transfer_read_limit: self.transfer_read_limit.clone(),
            transfer_writes: self.transfer_writes.clone(),
            invalid_transfer_read: self.invalid_transfer_read.clone(),
            injected_name: self.injected_name.clone(),
            directory_read_gate: self.directory_read_gate.clone(),
        }
    }
}
impl Filesystem {
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
        let owned = Arc::new(TransferWriteState {
            target: target.to_owned(),
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
    pub fn set_invalid_transfer_read(&self, mode: usize) {
        self.invalid_transfer_read.store(mode, Ordering::Release);
    }
    /// Own a WRITE hold for a generated atomic temporary after its first chunk.
    pub fn hold_atomic_writes_after_first(&self) -> Result<TransferWriteHold, &'static str> {
        let owned = Arc::new(TransferWriteState {
            target: String::new(),
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
    pub fn transfer_writes_started(&self) -> usize {
        self.transfer_writes.load(Ordering::Acquire)
    }
    pub fn set_transfer_read_limit(&self, bytes: usize) {
        self.transfer_read_limit.store(bytes, Ordering::Release);
    }
    pub fn set_transfer_write_delay(&self, milliseconds: usize) {
        self.transfer_write_delay
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
        let state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        if path == "/" || state.directories.contains(&path) {
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o40000 | state.modes.get(&path).copied().unwrap_or(0o700));
            return Ok(Attrs { id, attrs });
        }
        let file = state.files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(file.len() as u64);
        attrs.permissions = Some(0o100000 | state.modes.get(&path).copied().unwrap_or(0o644));
        attrs.mtime = state.mtimes.get(&path).copied();
        attrs.atime = attrs.mtime;
        Ok(Attrs { id, attrs })
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        self.lstat(id, handle).await
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
        Ok(Packet::Status(self.rename(id, from, to).await?))
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
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
        Ok(Handle {
            id,
            handle: filename,
        })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        if self.opened.remove(&handle) {
            self.open_handles
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
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
        Ok(ok(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
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
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        state.directories.remove(&path);
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
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
        let mut state = self.state.lock().map_err(|_| StatusCode::Failure)?;
        state.read_directories.remove(&path);
        if self.opened.insert(path.clone()) {
            self.open_handles
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        Ok(Handle { id, handle: path })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusCode> {
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
        Ok(Name { id, files })
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        Ok(Name {
            id,
            files: vec![File::dummy(if path == "." { "/".into() } else { path })],
        })
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
