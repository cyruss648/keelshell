//! SFTP operations on the authenticated SSH transport, without shell commands.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, Packet, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::ssh::{ChannelStreamOwner, OwnedRawSftpSession, SshSession, deadline};
use crate::{Result, SessionError};

mod control;
mod directory;
mod file_resume;
use control::{TransferContext, TransferControl};
pub use directory::{DirectoryResumePlan, DirectoryTransferPlan};
pub use file_resume::FileResumePlan;

/// A remote directory entry with portable metadata.
#[derive(Debug, Clone)]
pub struct RemoteEntry {
    /// Base file name, as supplied by the server.
    pub name: String,
    /// Full remote path.
    pub path: String,
    /// Byte size if the server supplies it.
    pub size: Option<u64>,
    /// Whether the remote entry is a directory.
    pub is_directory: bool,
    /// Whether the remote entry is a symbolic link.
    pub is_symlink: bool,
    /// POSIX mode bits when supplied by the server.
    pub permissions: Option<u32>,
    /// Modification timestamp in seconds since Unix epoch, when available.
    pub modified: Option<u32>,
}

/// Direction of a queued remote file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// Copy one local regular file to the remote destination.
    Upload,
    /// Copy one remote regular file to a new local destination.
    Download,
}

/// One file transfer submitted to [`TransferQueue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSpec {
    /// Whether bytes flow to or from the remote host.
    pub direction: TransferDirection,
    /// Local source for uploads or local destination for downloads.
    pub local: PathBuf,
    /// Remote source for downloads or remote destination for uploads.
    pub remote: String,
}

impl TransferSpec {
    /// Build an upload request. The destination is replaced as bytes arrive;
    /// an interrupted upload can therefore leave a partial remote file.
    pub fn upload(local: impl Into<PathBuf>, remote: impl Into<String>) -> Self {
        Self {
            direction: TransferDirection::Upload,
            local: local.into(),
            remote: remote.into(),
        }
    }

    /// Build a download request. The local destination must not already exist;
    /// an interrupted download leaves a visible partial file for inspection.
    pub fn download(remote: impl Into<String>, local: impl Into<PathBuf>) -> Self {
        Self {
            direction: TransferDirection::Download,
            local: local.into(),
            remote: remote.into(),
        }
    }
}

/// A bounded event emitted for one queued transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferEvent {
    /// The queue accepted the request. The worker may still be busy with an
    /// earlier request when this event is observed.
    Queued {
        /// Stable identifier for this queue entry.
        id: u64,
    },
    /// The worker started preparing the transfer and knows the source size when available.
    Started {
        /// Stable identifier for this queue entry.
        id: u64,
        /// Total bytes when the server or local filesystem supplied a size.
        total: Option<u64>,
    },
    /// A chunk was acknowledged or a resume prefix was verified.
    Progress {
        /// Stable identifier for this queue entry.
        id: u64,
        /// Acknowledged destination bytes, including verified resume prefixes.
        transferred: u64,
        /// Total bytes when known.
        total: Option<u64>,
    },
    /// The worker reached a safe point with no pending write. Paused time does
    /// not consume the active transfer deadline; the job keeps its FIFO slot.
    Paused {
        /// Stable queue identifier.
        id: u64,
        /// Acknowledged destination bytes, including a verified resume prefix.
        transferred: u64,
        /// Full source byte size when known.
        total: Option<u64>,
    },
    /// The worker acknowledged a resume request before continuing I/O.
    Resumed {
        /// Stable queue identifier.
        id: u64,
        /// Acknowledged destination bytes, including a verified resume prefix.
        transferred: u64,
        /// Full source byte size when known.
        total: Option<u64>,
    },
    /// The transfer completed and the destination handle was closed.
    Completed {
        /// Stable identifier for this queue entry.
        id: u64,
        /// Bytes successfully transferred.
        bytes: u64,
    },
    /// Cancellation was requested before the next chunk. Remote or local
    /// partial output may remain and should be inspected before retrying.
    Cancelled {
        /// Stable identifier for this queue entry.
        id: u64,
        /// Bytes transferred before cancellation was observed.
        bytes: u64,
    },
    /// The transfer failed. The string is safe for UI display and excludes
    /// authentication material.
    Failed {
        /// Stable identifier for this queue entry.
        id: u64,
        /// Sanitized transport or filesystem error.
        error: String,
    },
}

/// A handle for observing and cancelling one queue entry.
pub struct TransferHandle {
    id: u64,
    events: mpsc::Receiver<TransferEvent>,
    terminal: Option<oneshot::Receiver<TransferEvent>>,
    control: Arc<TransferControl>,
}

impl TransferHandle {
    /// Stable identifier used by every event for this transfer.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Request cancellation, including while paused or awaiting an I/O reply.
    /// The terminal event counts acknowledged bytes and verified resume prefixes;
    /// a pending remote write may still finish after local cancellation.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Request a pause. Wait for `TransferEvent::Paused` before treating it as
    /// acknowledged; already submitted remote writes may still complete first.
    pub fn pause(&self) {
        self.control.pause(true);
    }

    /// Request continuation. A queued job keeps its FIFO position while paused.
    pub fn resume(&self) {
        self.control.pause(false);
    }

    /// Wait for the next queue event. `None` means the queue was dropped before
    /// it could report a terminal event.
    pub async fn recv(&mut self) -> Option<TransferEvent> {
        if let Some(event) = self.events.recv().await {
            return Some(event);
        }
        self.terminal.take()?.await.ok()
    }
}

impl Drop for TransferHandle {
    fn drop(&mut self) {
        self.control.cancel();
    }
}

/// A single-worker FIFO queue for bounded-memory SFTP uploads and downloads.
///
/// The queue owns no UI state and can be polled from a GPUI task or another
/// async consumer. Each queued transfer receives progress and exactly one
/// terminal event (`Completed`, `Cancelled` or `Failed`) while the queue lives.
/// Create it inside an active Tokio runtime and keep the queue alive until its
/// handles have reached a terminal event.
pub struct TransferQueue {
    commands: mpsc::Sender<TransferCommand>,
    next_id: AtomicU64,
    worker: Option<JoinHandle<()>>,
}

enum TransferJob {
    File(TransferSpec),
    Directory(DirectoryTransferPlan),
    Resume(FileResumePlan),
    DirectoryResume(DirectoryResumePlan),
}

struct TransferCommand {
    id: u64,
    spec: TransferJob,
    events: mpsc::Sender<TransferEvent>,
    terminal: oneshot::Sender<TransferEvent>,
    control: Arc<TransferControl>,
}

/// One SFTP subsystem. Dropping a pending operation cancels the local future;
/// a timed-out remote mutation has an unknown outcome and must be checked before retry.
pub struct SftpSession {
    inner: russh_sftp::client::SftpSession,
    timeout: Duration,
    _connection: SshSession,
    channel_owner: ChannelStreamOwner,
    cleanups: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl SftpSession {
    pub(crate) async fn from_stream<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        stream: S,
        timeout: Duration,
        connection: SshSession,
        channel_owner: ChannelStreamOwner,
    ) -> Result<Self> {
        let inner = russh_sftp::client::SftpSession::new(stream)
            .await
            .map_err(sftp_error)?;
        inner.set_timeout(timeout.as_secs().max(1));
        Ok(Self {
            inner,
            timeout,
            _connection: connection,
            channel_owner,
            cleanups: Arc::default(),
        })
    }

    /// List at most 10,000 remote entries, excluding `.` and `..`.
    pub async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        self.list_limited(path, 10_000).await
    }

    /// Read directory packets with a strict entry bound. A dedicated subsystem
    /// is closed by its scope guard on success, failure, timeout or cancellation,
    /// including cancellation before the server returns an opendir handle.
    pub async fn list_limited(&self, path: &str, max_entries: usize) -> Result<Vec<RemoteEntry>> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP list", async {
            let raw = self._connection.sftp_raw().await?;
            let guard = DirectoryChannel(raw);
            let handle = guard.0.opendir(path).await.map_err(sftp_error)?.handle;
            let mut entries = Vec::new();
            let result = loop {
                match guard.0.readdir(&handle).await {
                    Ok(packet) => {
                        if entries.len().saturating_add(packet.files.len()) > max_entries {
                            break Err(SessionError::EntryLimit(max_entries));
                        }
                        for file in packet.files {
                            if matches!(file.filename.as_str(), "." | "..") {
                                continue;
                            }
                            let attrs = file.attrs;
                            entries.push(RemoteEntry {
                                path: format!("{}/{}", path.trim_end_matches('/'), file.filename),
                                name: file.filename,
                                size: attrs.size,
                                is_directory: attrs.file_type().is_dir(),
                                is_symlink: attrs.file_type().is_symlink(),
                                permissions: attrs.permissions,
                                modified: attrs.mtime,
                            });
                        }
                    }
                    Err(russh_sftp::client::error::Error::Status(status))
                        if status.status_code == russh_sftp::protocol::StatusCode::Eof =>
                    {
                        break Ok(entries);
                    }
                    Err(error) => break Err(sftp_error(error)),
                }
            };
            let closed = guard.0.close(handle).await.map_err(sftp_error);
            let entries = result?;
            closed?;
            Ok(entries)
        })
        .await
    }

    /// Resolve a path using the remote filesystem's canonicalization semantics.
    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP canonicalize", async {
            self.inner.canonicalize(path).await.map_err(sftp_error)
        })
        .await
    }

    /// Read at most `max_bytes`; exceeding the limit is an error rather than a
    /// silently truncated file. Use `download` for large files.
    pub async fn read(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP read", async {
            let mut file = self.inner.open(path).await.map_err(sftp_error)?;
            let mut content = Vec::new();
            let result = (&mut file)
                .take((max_bytes as u64).saturating_add(1))
                .read_to_end(&mut content)
                .await;
            let close = file.close().await;
            result?;
            close?;
            if content.len() > max_bytes {
                return Err(SessionError::OutputLimit(max_bytes));
            }
            Ok(content)
        })
        .await
    }

    /// Low-level create/truncate write. Failure or cancellation can leave a
    /// partial destination; interactive editors should use `write_atomic`.
    pub async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP write", async {
            let mut file = self.inner.create(path).await.map_err(sftp_error)?;
            let result = file.write_all(data).await;
            let close = file.close().await;
            result?;
            close?;
            Ok(())
        })
        .await
    }

    /// Stream an upload with bounded memory. The remote destination is replaced;
    /// an interrupted upload may leave a partial file for an explicit retry.
    pub async fn upload(&self, local: &Path, remote: &str) -> Result<u64> {
        valid_path(remote)?;
        deadline(self.timeout, "SFTP upload", async {
            let mut source = tokio::fs::File::open(local).await?;
            let mut target = self.inner.create(remote).await.map_err(sftp_error)?;
            let result = tokio::io::copy(&mut source, &mut target).await;
            let close = target.close().await;
            let count = result?;
            close?;
            Ok(count)
        })
        .await
    }

    /// Replace a regular file atomically using negotiated OpenSSH POSIX rename.
    /// The old destination remains intact until all temporary bytes are written
    /// and its handle is closed. An uncertain rename acknowledgement means the
    /// target is either the old file or the complete new file. Symbolic links
    /// are refused; existing rwx permissions are copied, but ownership, ACLs,
    /// special mode bits and other metadata are not preserved. New files use
    /// mode 0600. This guarantees atomic visibility, not crash durability.
    pub async fn write_atomic(&self, path: &str, data: &[u8]) -> Result<()> {
        valid_atomic_path(path)?;
        deadline(self.timeout, "SFTP atomic write", async {
            self.replace_from_reader(path, &mut &data[..]).await?;
            Ok(())
        })
        .await
    }

    /// Stream a local file through a same-directory temporary file, then
    /// atomically replace the remote regular file. Uses the same capability,
    /// metadata and uncertain-acknowledgement policy as `write_atomic`.
    pub async fn upload_atomic(&self, local: &Path, remote: &str) -> Result<u64> {
        valid_atomic_path(remote)?;
        deadline(self.timeout, "SFTP atomic upload", async {
            let mut source = tokio::fs::File::open(local).await?;
            self.replace_from_reader(remote, &mut source).await
        })
        .await
    }

    async fn replace_from_reader<R: AsyncRead + Unpin>(
        &self,
        path: &str,
        source: &mut R,
    ) -> Result<u64> {
        let (raw, version) = self._connection.sftp_raw_with_version().await?;
        if version
            .extensions
            .get("posix-rename@openssh.com")
            .map(String::as_str)
            != Some("1")
        {
            return Err(SessionError::Unsupported(
                "server must advertise posix-rename@openssh.com version 1",
            ));
        }
        let mut attrs = FileAttributes::empty();
        attrs.permissions = Some(0o600);
        match raw.lstat(path).await {
            Ok(existing) => {
                if !existing.attrs.file_type().is_file() {
                    return Err(SessionError::Invalid(
                        "atomic replacement requires a confirmed regular file",
                    ));
                }
                if let Some(mode) = existing.attrs.permissions {
                    attrs.permissions = Some(mode & 0o777);
                }
            }
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile => {}
            Err(error) => return Err(sftp_error(error)),
        }
        let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
        let temporary = format!("{parent}/.{name}.keelshell-{}.tmp", uuid::Uuid::new_v4());
        let mut guard = AtomicTemporary {
            raw: Arc::new(raw),
            temporary: Some(temporary.clone()),
            handle: None,
            cleanups: self.cleanups.clone(),
            connection: self._connection.clone(),
            runtime: tokio::runtime::Handle::current(),
        };
        let opened = guard
            .raw
            .open(
                temporary,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
                attrs,
            )
            .await;
        let handle = match opened {
            Ok(opened) => opened.handle,
            Err(error) => {
                // A definite failure did not create our file. In particular,
                // O_EXCL collisions must never remove somebody else's entry.
                if matches!(&error, russh_sftp::client::error::Error::Status(_)) {
                    guard.temporary = None;
                }
                return Err(sftp_error(error));
            }
        };
        guard.handle = Some(handle.clone());
        // Keep chunk storage out of this future and its caller's debug stack.
        let mut buffer = vec![0_u8; 32 * 1024];
        let mut offset = 0_u64;
        loop {
            let count = source.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            guard
                .raw
                .write(&handle, offset, buffer[..count].to_vec())
                .await
                .map_err(sftp_error)?;
            offset = offset
                .checked_add(count as u64)
                .ok_or(SessionError::Invalid("upload exceeds u64 size"))?;
        }
        guard.raw.close(&handle).await.map_err(sftp_error)?;
        guard.handle = None;
        let mut payload = Vec::new();
        for value in [
            guard.temporary.as_deref().ok_or(SessionError::Worker)?,
            path,
        ] {
            let len = u32::try_from(value.len())
                .map_err(|_| SessionError::Invalid("remote path is too long"))?;
            payload.extend_from_slice(&len.to_be_bytes());
            payload.extend_from_slice(value.as_bytes());
        }
        match guard
            .raw
            .extended("posix-rename@openssh.com", payload)
            .await
            .map_err(sftp_error)?
        {
            Packet::Status(status) if status.status_code == StatusCode::Ok => {}
            Packet::Status(status) => {
                return Err(sftp_error(format!(
                    "atomic rename: {:?}: {}",
                    status.status_code, status.error_message
                )));
            }
            _ => return Err(sftp_error("unexpected atomic rename response")),
        }
        guard.temporary = None;
        Ok(offset)
    }

    /// Stream into a new local file, refusing to overwrite an existing path.
    /// An interrupted operation leaves the partial file visibly present.
    pub async fn download(&self, remote: &str, local: &Path) -> Result<u64> {
        valid_path(remote)?;
        deadline(self.timeout, "SFTP download", async {
            let mut source = self.inner.open(remote).await.map_err(sftp_error)?;
            let mut target = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(local)
                .await?;
            let result = tokio::io::copy(&mut source, &mut target).await;
            let close = source.close().await;
            let count = result?;
            close?;
            target.flush().await?;
            Ok(count)
        })
        .await
    }

    /// Create an empty remote directory.
    pub async fn mkdir(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP mkdir", async {
            self.inner.create_dir(path).await.map_err(sftp_error)
        })
        .await
    }
    /// Rename a remote entry according to the server's SFTP semantics.
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        valid_path(from)?;
        valid_path(to)?;
        deadline(self.timeout, "SFTP rename", async {
            self.inner.rename(from, to).await.map_err(sftp_error)
        })
        .await
    }
    /// Remove one file or symbolic link; this never recursively removes a tree.
    pub async fn remove(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP remove", async {
            self.inner.remove_file(path).await.map_err(sftp_error)
        })
        .await
    }
    /// Remove an empty remote directory.
    pub async fn rmdir(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP rmdir", async {
            self.inner.remove_dir(path).await.map_err(sftp_error)
        })
        .await
    }

    /// Change only POSIX mode bits for one reviewed remote entry.
    ///
    /// The entry is a review snapshot, not an authority by itself. Before the
    /// mutation this method re-reads every parent with `lstat`, rejects any
    /// symbolic link or special-file target, and compares the current type and
    /// mode with the snapshot. The request carries no owner, group, size or
    /// timestamp fields. A second `lstat` verifies the acknowledged result.
    /// SFTP v3 cannot provide a compare-and-swap or no-follow mutation, so a
    /// concurrent server-side rename remains outside this guarantee.
    pub async fn set_permissions_reviewed(
        &self,
        reviewed: &RemoteEntry,
        mode: u32,
    ) -> Result<RemoteEntry> {
        valid_reviewed_entry(reviewed)?;
        if mode > 0o7777 {
            return Err(SessionError::Invalid(
                "POSIX mode must be between 0000 and 7777",
            ));
        }
        deadline(self.timeout, "SFTP set permissions", async {
            for parent in reviewed_parent_paths(&reviewed.path)? {
                let attrs = self
                    .inner
                    .symlink_metadata(&parent)
                    .await
                    .map_err(sftp_error)?;
                if attrs.file_type().is_symlink() || !attrs.file_type().is_dir() {
                    return Err(SessionError::Invalid(
                        "permission target has an unsafe parent directory",
                    ));
                }
            }
            let before = self
                .inner
                .symlink_metadata(&reviewed.path)
                .await
                .map_err(sftp_error)?;
            if before.file_type().is_symlink()
                || (!before.file_type().is_file() && !before.file_type().is_dir())
                || before.file_type().is_dir() != reviewed.is_directory
                || before.permissions.map(|value| value & 0o7777)
                    != reviewed.permissions.map(|value| value & 0o7777)
            {
                return Err(SessionError::UnverifiedMutation(
                    "reviewed remote type or mode changed",
                ));
            }
            let mut attributes = FileAttributes::empty();
            attributes.permissions = Some(mode);
            self.inner
                .set_metadata(&reviewed.path, attributes)
                .await
                .map_err(sftp_error)?;
            let after = self
                .inner
                .symlink_metadata(&reviewed.path)
                .await
                .map_err(|_| SessionError::UnverifiedMutation("remote mode readback failed"))?;
            if after.file_type().is_symlink()
                || (!after.file_type().is_file() && !after.file_type().is_dir())
                || after.file_type().is_dir() != reviewed.is_directory
                || after.permissions.map(|value| value & 0o7777) != Some(mode)
            {
                return Err(SessionError::UnverifiedMutation(
                    "remote mode readback did not match the requested mode",
                ));
            }
            Ok(remote_entry_from_attributes(reviewed, after))
        })
        .await
    }

    /// Stop this subsystem's relay and request its channel close. Ordinary
    /// cleanup preserves SSH; a stalled protocol close can force shared shutdown.
    /// Completion is not a remote acknowledgement or a rollback of file writes.
    pub async fn close(&self) -> Result<()> {
        // Do not wait for a raw writer to consume its queued close sentinel.
        self.channel_owner.cancel();
        let pending = self
            .cleanups
            .lock()
            .map(|mut tasks| std::mem::take(&mut *tasks))
            .unwrap_or_default();
        for task in pending {
            let _ = task.await;
        }
        deadline(
            self.timeout.min(Duration::from_secs(2)),
            "SFTP close",
            async { self.inner.close().await.map_err(sftp_error) },
        )
        .await
    }
}

fn valid_reviewed_entry(entry: &RemoteEntry) -> Result<()> {
    valid_path(&entry.path)?;
    if entry.path == "/"
        || !entry.path.starts_with('/')
        || entry.path.len() > 4096
        || entry.path.chars().any(char::is_control)
    {
        return Err(SessionError::Invalid(
            "reviewed remote path must be an absolute bounded path",
        ));
    }
    if entry.is_symlink || entry.permissions.is_none() {
        return Err(SessionError::UnverifiedMutation(
            "reviewed entry is a symbolic link or has no mode",
        ));
    }
    if entry
        .path
        .split('/')
        .any(|component| matches!(component, "." | ".."))
    {
        return Err(SessionError::Invalid(
            "reviewed remote path contains a traversal component",
        ));
    }
    Ok(())
}

fn reviewed_parent_paths(path: &str) -> Result<Vec<String>> {
    let components: Vec<&str> = path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect();
    if components.iter().any(|component| component.contains('\0')) {
        return Err(SessionError::Invalid("remote path contains NUL"));
    }
    Ok((1..components.len())
        .map(|count| format!("/{}", components[..count].join("/")))
        .collect())
}

fn remote_entry_from_attributes(reviewed: &RemoteEntry, attrs: FileAttributes) -> RemoteEntry {
    RemoteEntry {
        name: reviewed.name.clone(),
        path: reviewed.path.clone(),
        size: attrs.size,
        is_directory: attrs.file_type().is_dir(),
        is_symlink: attrs.file_type().is_symlink(),
        permissions: attrs.permissions,
        modified: attrs.mtime,
    }
}

impl SftpSession {
    /// Start a FIFO transfer queue using this authenticated SFTP subsystem.
    ///
    /// The queue keeps the session alive through an [`Arc`]. Its worker is
    /// single threaded by design so progress ordering is deterministic and a
    /// server with a small request window is not flooded by UI actions.
    pub fn transfer_queue(self: Arc<Self>) -> TransferQueue {
        TransferQueue::new(self)
    }

    async fn queued_upload(
        &self,
        local: &Path,
        remote: &str,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        let mut source = tokio::fs::File::open(local)
            .await
            .map_err(SessionError::from)?;
        if !source
            .metadata()
            .await
            .map_err(SessionError::from)?
            .is_file()
        {
            return Err(SessionError::Invalid(
                "file upload requires a regular file; use directory upload",
            )
            .into());
        }
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let handle = raw
            .0
            .open(
                remote,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                FileAttributes::empty(),
            )
            .await
            .map_err(sftp_error)?
            .handle;
        let mut buffer = vec![0; TRANSFER_CHUNK_SIZE];
        let mut offset = 0;
        loop {
            context.checkpoint().await?;
            let count = source.read(&mut buffer).await.map_err(SessionError::from)?;
            if count == 0 {
                break;
            }
            raw.0
                .write(&handle, offset, buffer[..count].to_vec())
                .await
                .map_err(sftp_error)?;
            offset += count as u64;
            context.progress(count as u64).await?;
        }
        raw.0.close(handle).await.map_err(sftp_error)?;
        Ok(())
    }

    async fn queued_download(
        &self,
        remote: &str,
        local: &Path,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let handle = raw
            .0
            .open(remote, OpenFlags::READ, FileAttributes::empty())
            .await
            .map_err(sftp_error)?
            .handle;
        let mut target = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(local)
            .await
            .map_err(SessionError::from)?;
        let mut offset = 0;
        loop {
            context.checkpoint().await?;
            let data = match raw
                .0
                .read(&handle, offset, TRANSFER_CHUNK_SIZE as u32)
                .await
            {
                Ok(data) if !data.data.is_empty() && data.data.len() <= TRANSFER_CHUNK_SIZE => {
                    data.data
                }
                Ok(_) => return Err(SessionError::Invalid("invalid SFTP read length").into()),
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::Eof =>
                {
                    break;
                }
                Err(error) => return Err(sftp_error(error).into()),
            };
            target.write_all(&data).await.map_err(SessionError::from)?;
            target.flush().await.map_err(SessionError::from)?;
            offset += data.len() as u64;
            context.progress(data.len() as u64).await?;
        }
        raw.0.close(handle).await.map_err(sftp_error)?;
        Ok(())
    }
}

const TRANSFER_CHUNK_SIZE: usize = 64 * 1024;
type TransferExecutionResult<T = u64> = std::result::Result<T, TransferExecutionError>;
#[derive(Debug)]
enum TransferExecutionError {
    Cancelled(u64),
    Error(SessionError),
}
impl From<SessionError> for TransferExecutionError {
    fn from(error: SessionError) -> Self {
        Self::Error(error)
    }
}
impl From<std::io::Error> for TransferExecutionError {
    fn from(error: std::io::Error) -> Self {
        Self::Error(error.into())
    }
}

impl TransferQueue {
    /// Create a FIFO worker inside the active Tokio runtime. Paused entries
    /// retain their place; progress is coalesced when consumers are slow.
    pub fn new(sftp: Arc<SftpSession>) -> Self {
        let (commands, mut receiver) = mpsc::channel::<TransferCommand>(32);
        let worker = tokio::spawn(async move {
            while let Some(TransferCommand {
                id,
                spec,
                events,
                terminal,
                control,
            }) = receiver.recv().await
            {
                if *control.cancelled.borrow() {
                    let _ = terminal.send(TransferEvent::Cancelled { id, bytes: 0 });
                    continue;
                }
                let prepare = async {
                    match &spec {
                        TransferJob::Directory(plan) => Some(plan.bytes()),
                        TransferJob::DirectoryResume(plan) => Some(plan.bytes()),
                        TransferJob::Resume(plan) => Some(plan.bytes()),
                        TransferJob::File(spec) => match spec.direction {
                            TransferDirection::Upload => {
                                tokio::fs::metadata(&spec.local).await.ok().map(|m| m.len())
                            }
                            TransferDirection::Download => sftp
                                .inner
                                .metadata(&spec.remote)
                                .await
                                .ok()
                                .and_then(|m| m.size),
                        },
                    }
                };
                let total = tokio::select! {
                    biased;
                    _ = control.cancelled() => {
                        let _ = terminal.send(TransferEvent::Cancelled {id, bytes:0});
                        continue;
                    }
                    result = tokio::time::timeout(sftp.timeout, prepare) => match result {
                        Ok(total) => total,
                        Err(_) => { let _ = terminal.send(TransferEvent::Failed {id,error:SessionError::Timeout("SFTP transfer preparation").to_string()}); continue; }
                    }
                };
                let context = TransferContext::new(id, total, events.clone(), control);
                let budget = match spec {
                    TransferJob::Directory(_)
                    | TransferJob::DirectoryResume(_)
                    | TransferJob::Resume(_) => Duration::from_secs(15 * 60),
                    _ => sftp.timeout,
                };
                let result = context
                    .run(budget, "SFTP transfer", async {
                        context.event(TransferEvent::Started { id, total }).await?;
                        context.checkpoint().await?;
                        match spec {
                            TransferJob::Directory(plan) => {
                                sftp.queued_directory(plan, &context).await
                            }
                            TransferJob::DirectoryResume(plan) => {
                                sftp.queued_directory_resume(&plan, &context).await
                            }
                            TransferJob::Resume(plan) => {
                                sftp.execute_file_resume(&plan, &context).await
                            }
                            TransferJob::File(spec) => match spec.direction {
                                TransferDirection::Upload => {
                                    sftp.queued_upload(&spec.local, &spec.remote, &context)
                                        .await
                                }
                                TransferDirection::Download => {
                                    sftp.queued_download(&spec.remote, &spec.local, &context)
                                        .await
                                }
                            },
                        }
                    })
                    .await;
                let outcome = match result {
                    Ok(()) => TransferEvent::Completed {
                        id,
                        bytes: context.bytes(),
                    },
                    Err(TransferExecutionError::Cancelled(bytes)) => {
                        TransferEvent::Cancelled { id, bytes }
                    }
                    Err(TransferExecutionError::Error(error)) => TransferEvent::Failed {
                        id,
                        error: error.to_string(),
                    },
                };
                let _ = terminal.send(outcome);
            }
        });
        Self {
            commands,
            next_id: AtomicU64::new(1),
            worker: Some(worker),
        }
    }
    /// Enqueue an exclusive new-directory transfer after review.
    pub async fn enqueue_directory(&self, plan: DirectoryTransferPlan) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::Directory(plan)).await
    }
    /// Enqueue a reviewed continuation. Source and destination are fully
    /// revalidated before writing; interrupted output remains visible.
    pub async fn enqueue_resume(&self, plan: FileResumePlan) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::Resume(plan)).await
    }
    /// Enqueue a reviewed partial directory after full validation.
    pub async fn enqueue_directory_resume(
        &self,
        plan: DirectoryResumePlan,
    ) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::DirectoryResume(plan)).await
    }
    /// Enqueue a regular upload or exclusive local download.
    pub async fn enqueue(&self, spec: TransferSpec) -> Result<TransferHandle> {
        valid_path(&spec.remote)?;
        if spec.local.as_os_str().is_empty() {
            return Err(SessionError::Invalid("local transfer path is empty"));
        }
        self.enqueue_job(TransferJob::File(spec)).await
    }
    async fn enqueue_job(&self, spec: TransferJob) -> Result<TransferHandle> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let control = Arc::new(TransferControl::new());
        let (events, receiver) = mpsc::channel(32);
        let (terminal, terminal_receiver) = oneshot::channel();
        let _ = events.try_send(TransferEvent::Queued { id });
        self.commands
            .send(TransferCommand {
                id,
                spec,
                events,
                terminal,
                control: control.clone(),
            })
            .await
            .map_err(|_| SessionError::Worker)?;
        Ok(TransferHandle {
            id,
            events: receiver,
            terminal: Some(terminal_receiver),
            control,
        })
    }
}
impl Drop for TransferQueue {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

fn valid_atomic_path(path: &str) -> Result<()> {
    valid_path(path)?;
    if matches!(path.rsplit('/').next(), Some("" | "." | "..") | None) {
        return Err(SessionError::Invalid(
            "atomic destination must have a file name",
        ));
    }
    Ok(())
}

/// Owns the known temporary pathname before OPEN is sent, including the case
/// where cancellation occurs before an OPEN acknowledgement returns its handle.
struct AtomicTemporary {
    raw: Arc<OwnedRawSftpSession>,
    temporary: Option<String>,
    handle: Option<String>,
    cleanups: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    connection: SshSession,
    runtime: tokio::runtime::Handle,
}
impl Drop for AtomicTemporary {
    fn drop(&mut self) {
        let Some(path) = self.temporary.take() else {
            let _ = self.raw.close_session();
            return;
        };
        let raw = self.raw.clone();
        let handle = self.handle.take();
        let connection = self.connection.clone();
        let task = self.runtime.spawn(async move {
            let _connection = connection;
            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                if let Some(handle) = handle {
                    let _ = raw.close(handle).await;
                }
                let _ = raw.remove(path).await;
            })
            .await;
            let _ = raw.close_session();
        });
        if let Ok(mut tasks) = self.cleanups.lock() {
            tasks.retain(|task| !task.is_finished());
            tasks.push(task);
        }
    }
}

fn valid_path(path: &str) -> Result<()> {
    if path.is_empty() || path.contains('\0') {
        Err(SessionError::Invalid(
            "remote path is empty or contains NUL",
        ))
    } else {
        Ok(())
    }
}

fn sftp_error(error: impl std::fmt::Display) -> SessionError {
    SessionError::Sftp(error.to_string())
}

struct DirectoryChannel(OwnedRawSftpSession);
impl Drop for DirectoryChannel {
    fn drop(&mut self) {
        let _ = self.0.close_session();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_future_keeps_large_buffers_on_the_heap() {
        fn future_size<F>(
            _: impl FnOnce(&'static SftpSession, &'static FileResumePlan, &'static TransferContext) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        let size = future_size(|session, plan, context| session.execute_file_resume(plan, context));
        eprintln!("file resume future: {size} bytes");
        assert!(size < 32 * 1024, "file resume future uses {size} bytes");
    }

    #[test]
    fn file_transfer_futures_do_not_embed_transfer_buffers() {
        // Type inference measures unpolled futures without a real transport.
        fn future_size<F>(
            _: impl FnOnce(&'static SftpSession, &'static TransferContext) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        let sizes = [
            (
                "queued upload",
                future_size(|session, context| {
                    session.queued_upload(Path::new("source"), "/target", context)
                }),
            ),
            (
                "queued download",
                future_size(|session, context| {
                    session.queued_download("/source", Path::new("target"), context)
                }),
            ),
            (
                "atomic upload",
                future_size(|session, _| session.upload_atomic(Path::new("source"), "/target")),
            ),
            (
                "atomic write",
                future_size(|session, _| session.write_atomic("/target", b"contents")),
            ),
        ];
        for (name, bytes) in sizes {
            eprintln!("{name} future: {bytes} bytes");
            assert!(bytes < 16 * 1024, "{name} future uses {bytes} bytes");
        }
    }
}
