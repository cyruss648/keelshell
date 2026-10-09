//! SFTP operations on the authenticated SSH transport, without shell commands.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, Packet, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::ssh::{ChannelStreamOwner, OwnedRawSftpSession, SshSession, deadline};
use crate::{Result, SessionError};

mod control;
mod directory;
mod file_resume;
mod file_transfer;
pub use file_transfer::{FileTransferPlan, MAX_REVIEWED_FILE_BYTES};
mod inspection;
mod mutation;
pub use mutation::FileMutationScope;
mod queue;
use control::{TransferContext, TransferControl, local_io, remote_io};

/// Observe a completed SFTP setup reply if an owning transfer is being polled.
/// Setup/admission deadlines remain separate and this does not resolve writes.
pub(crate) fn observed_transfer_io() {
    control::observed_io();
}
pub use directory::{DirectoryResumePlan, DirectoryTransferPlan};
pub use file_resume::FileResumePlan;
pub(crate) use queue::TransferReservations;
pub use queue::{
    MAX_PARALLEL_TRANSFERS, MAX_QUEUED_TRANSFERS, TransferQuarantineEntry,
    TransferQuarantineReview, TransferQueue,
};

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

/// Complete checked regular-file observation, retained only by its caller.
#[derive(Debug, Clone)]
pub struct RegularFileSnapshot {
    /// Exact complete file bytes; no truncation or text normalization.
    pub content: Vec<u8>,
    /// Path, size, explicit mode/type and timestamp observed during the read.
    pub entry: RemoteEntry,
}

/// Maximum directory depth accepted by [`SftpSession::snapshot_tree_limited`].
pub const MAX_SNAPSHOT_DEPTH: usize = 32;

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
    /// not consume the active transfer deadline; the job keeps its slot and locks.
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
    /// A mutating request was still awaiting acknowledgement when the local
    /// operation ended. Its destination is quarantined in this application;
    /// inspect it and explicitly review its unresolved risk before release.
    /// Reconnecting does not prove that an earlier mutation has stopped.
    Uncertain {
        /// Stable queue identifier.
        id: u64,
        /// Only previously acknowledged bytes, never the pending write. A full
        /// byte count does not prove writable CLOSE or publication completed.
        bytes: u64,
        /// Sanitized reason and the required next step.
        error: String,
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

    /// Request continuation. A paused job keeps its slot and path reservations.
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

/// One SFTP subsystem. Dropping a pending operation cancels the local future;
/// a timed-out remote mutation has an unknown outcome and must be checked before retry.
pub struct SftpSession {
    inner: russh_sftp::client::SftpSession,
    timeout: Duration,
    _connection: SshSession,
    channel_owner: ChannelStreamOwner,
    cleanups: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    stopped: tokio::sync::watch::Sender<bool>,
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
            stopped: tokio::sync::watch::channel(false).0,
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
            let handle = remote_io(guard.0.opendir(path))
                .await
                .map_err(sftp_error)?
                .handle;
            let mut entries = Vec::new();
            let result = loop {
                match remote_io(guard.0.readdir(&handle)).await {
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
            let closed = remote_io(guard.0.close(handle)).await.map_err(sftp_error);
            let entries = result?;
            closed?;
            Ok(entries)
        })
        .await
    }

    /// Collect a bounded recursive snapshot without following symbolic links.
    ///
    /// The returned entries retain absolute remote paths and are sorted by
    /// path for deterministic review. `max_depth` counts child directories
    /// below `path`; zero lists only the immediate children. A depth boundary
    /// is rejected if a directory would have to be omitted, rather than
    /// returning a report that looks complete while silently missing entries.
    /// This method only reads SFTP metadata and does not hash or mutate files.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` for zero limits or a depth above
    /// [`MAX_SNAPSHOT_DEPTH`], `EntryLimit` when the tree is larger than the
    /// requested bound, or the underlying SFTP error.
    pub async fn snapshot_tree_limited(
        &self,
        path: &str,
        max_entries: usize,
        max_depth: usize,
    ) -> Result<Vec<RemoteEntry>> {
        valid_path(path)?;
        if max_entries == 0 || max_depth > MAX_SNAPSHOT_DEPTH {
            return Err(SessionError::Invalid("invalid remote snapshot limits"));
        }
        let root = if path == "/" {
            "/".to_owned()
        } else {
            path.trim_end_matches('/').to_owned()
        };
        let mut pending = VecDeque::from([(root, 0usize)]);
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        while let Some((directory, depth)) = pending.pop_front() {
            if entries.len() >= max_entries {
                return Err(SessionError::EntryLimit(max_entries));
            }
            let remaining = max_entries.saturating_sub(entries.len());
            let page = match self.list_limited(&directory, remaining).await {
                Ok(page) => page,
                Err(SessionError::EntryLimit(_)) => {
                    return Err(SessionError::EntryLimit(max_entries));
                }
                Err(error) => return Err(error),
            };
            for entry in page {
                if !seen.insert(entry.path.clone()) {
                    return Err(SessionError::Invalid(
                        "remote snapshot contains duplicate paths",
                    ));
                }
                if entries.len() == max_entries {
                    return Err(SessionError::EntryLimit(max_entries));
                }
                if entry.is_directory && !entry.is_symlink {
                    if depth == max_depth {
                        return Err(SessionError::Invalid(
                            "remote snapshot exceeds the requested depth",
                        ));
                    }
                    pending.push_back((entry.path.clone(), depth + 1));
                }
                entries.push(entry);
            }
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    /// Resolve a path using the remote filesystem's canonicalization semantics.
    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP canonicalize", async {
            remote_io(self.inner.canonicalize(path))
                .await
                .map_err(sftp_error)
        })
        .await
    }

    /// Read at most `max_bytes`; exceeding the limit is an error rather than a
    /// silently truncated file. Use `download` for large files.
    pub async fn read(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>> {
        valid_path(path)?;
        deadline(self.timeout, "SFTP read", async {
            let mut file = remote_io(self.inner.open(path)).await.map_err(sftp_error)?;
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
        let scope = self.reserve_remote(&[path], false, &|| true).await?;
        let result = deadline(self.timeout, "SFTP write", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            let handle = scope
                .remote(raw.0.open(
                    path,
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                    FileAttributes::empty(),
                ))
                .await?
                .handle;
            let mut chunks = data.chunks(32 * 1024).enumerate();
            while let Some((index, first)) = chunks.next() {
                let second = chunks.next();
                scope
                    .remote(async {
                        // Observe both bounded requests before classifying a
                        // rejection: one STATUS cannot hide a missing peer reply.
                        let (first, second) = tokio::join!(
                            raw.0
                                .write(&handle, (index * 32 * 1024) as u64, first.to_vec()),
                            async {
                                match second {
                                    Some((index, bytes)) => raw
                                        .0
                                        .write(&handle, (index * 32 * 1024) as u64, bytes.to_vec())
                                        .await
                                        .map(|_| ()),
                                    None => Ok(()),
                                }
                            }
                        );
                        match (first.map(|_| ()), second) {
                            (Err(error), _)
                                if !matches!(
                                    &error,
                                    russh_sftp::client::error::Error::Status(_)
                                ) =>
                            {
                                Err(error)
                            }
                            (_, Err(error))
                                if !matches!(
                                    &error,
                                    russh_sftp::client::error::Error::Status(_)
                                ) =>
                            {
                                Err(error)
                            }
                            (Err(error), _) | (_, Err(error)) => Err(error),
                            _ => Ok(()),
                        }
                    })
                    .await?;
            }
            scope.remote(raw.0.close(handle)).await?;
            Ok(())
        })
        .await;
        scope.complete(result).await
    }

    /// Stream an in-place upload with bounded memory and shared exclusion.
    /// Failure may leave partial output; pending mutation replies retain quarantine.
    /// Confirmed I/O renews the connection's idle timeout, so a progressing
    /// transfer may take longer than one timeout interval in total.
    pub async fn upload(&self, local: &Path, remote: &str) -> Result<u64> {
        valid_path(remote)?;
        let scope = self
            .reserve_transfer(&TransferSpec::upload(local, remote), false)
            .await?;
        let context = scope.context()?;
        let result = context
            .run(self.timeout, "SFTP upload idle wait", async {
                scope.revalidate().await?;
                tokio::select! {
                    biased;
                    _=self.cancelled()=>Err(TransferExecutionError::from(SessionError::Closed)),
                    result=self.queued_upload(local,remote,&context)=>result,
                }?;
                Ok(context.bytes())
            })
            .await
            .map_err(transfer_session_error);
        scope.complete(result).await
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
        let scope = self.reserve_remote(&[path], true, &|| true).await?;
        let result = deadline(
            self.timeout,
            "SFTP atomic write",
            scope.write_remote_atomic(path, data),
        )
        .await;
        scope.complete(result).await
    }

    /// Stream a local file through an exclusive temporary, then atomically publish.
    /// Shared source/destination exclusion also applies to direct API callers.
    /// Each confirmed I/O completion renews the idle timeout; publication still
    /// requires acknowledged writable CLOSE and POSIX rename.
    pub async fn upload_atomic(&self, local: &Path, remote: &str) -> Result<u64> {
        valid_atomic_path(remote)?;
        let scope = self
            .reserve_transfer(&TransferSpec::upload(local, remote), true)
            .await?;
        let context = scope.context()?;
        let result = context.run(self.timeout, "SFTP atomic upload idle wait", async {
            scope.revalidate().await?;
            let mut source = local_io(tokio::fs::File::open(local)).await?;
            tokio::select! {
                biased;
                _=self.cancelled()=>Err(TransferExecutionError::from(SessionError::Closed)),
                result=self.replace_from_reader_checked(remote, &mut source, None, Some(&context), Some(&scope), None)=>result,
            }
        })
        .await.map_err(transfer_session_error);
        scope.complete(result).await
    }

    /// Replace an existing reviewed regular file of at most 64 KiB.
    /// Complete content, size, mode, timestamp and canonical path are rechecked
    /// before staging and immediately before POSIX rename. Existing temporary
    /// ownership and no-fallback guarantees are identical to `write_atomic`.
    /// SFTP v3 cannot atomically compare the target with the review: a malicious
    /// concurrent server-side rename after the final check remains possible.
    pub async fn write_regular_reviewed(
        &self,
        reviewed: &RegularFileSnapshot,
        data: &[u8],
    ) -> Result<()> {
        self.write_regular_reviewed_authorized(reviewed, data, &|| true)
            .await
    }

    /// Replace a reviewed file while checking current authority before each write.
    /// Ordinary proposal approval cannot release a conflicting unknown result.
    /// Dropping this future during a request retains shared target quarantine.
    pub async fn write_regular_reviewed_authorized(
        &self,
        reviewed: &RegularFileSnapshot,
        data: &[u8],
        authorized: &(dyn Fn() -> bool + Sync),
    ) -> Result<()> {
        self.write_regular_reviewed_bounded(reviewed, data, 64 * 1024, authorized)
            .await
    }

    /// Replace an explicitly reviewed desktop text file of at most 1 MiB.
    /// Exact content and metadata are rechecked before staging and publication.
    /// Authorization is checked before each mutation; unknown results retain the
    /// existing shared target quarantine. This does not expand MCP's 64 KiB limit.
    /// SFTP supplies observations rather than an atomic compare-and-swap lock.
    pub async fn write_editor_reviewed_authorized(
        &self,
        reviewed: &RegularFileSnapshot,
        data: &[u8],
        authorized: &(dyn Fn() -> bool + Sync),
    ) -> Result<()> {
        if std::str::from_utf8(data).is_err()
            || data.contains(&0)
            || std::str::from_utf8(&reviewed.content).is_err()
            || reviewed.content.contains(&0)
        {
            return Err(SessionError::Invalid(
                "editor replacement requires UTF-8 text without NUL",
            ));
        }
        self.write_regular_reviewed_bounded(reviewed, data, 1024 * 1024, authorized)
            .await
    }

    async fn write_regular_reviewed_bounded(
        &self,
        reviewed: &RegularFileSnapshot,
        data: &[u8],
        limit: usize,
        authorized: &(dyn Fn() -> bool + Sync),
    ) -> Result<()> {
        inspection::inspection_path(&reviewed.entry.path)?;
        if reviewed.content.len() > limit || data.len() > limit {
            return Err(SessionError::OutputLimit(limit));
        }
        let scope = self
            .reserve_remote(&[&reviewed.entry.path], true, authorized)
            .await?;
        let result = deadline(self.timeout, "SFTP reviewed replacement", async {
            self.replace_from_reader_checked(
                &reviewed.entry.path,
                &mut &data[..],
                Some(reviewed),
                None,
                Some(&scope),
                None,
            )
            .await
            .map_err(transfer_session_error)?;
            Ok(())
        })
        .await;
        scope.complete(result).await
    }

    async fn replace_from_reader_checked<R: AsyncRead + Unpin>(
        &self,
        path: &str,
        source: &mut R,
        reviewed: Option<&RegularFileSnapshot>,
        transfer: Option<&TransferContext>,
        mutation: Option<&FileMutationScope<'_>>,
        file_review: Option<&FileTransferPlan>,
    ) -> TransferExecutionResult<u64> {
        if let Some(reviewed) = reviewed {
            // The checked read owns a nested protocol future. Keep it out of
            // the shared writer frame, including ordinary non-review uploads.
            Box::pin(self.verify_regular_snapshot(reviewed)).await?;
        }
        let (raw, version) = self._connection.sftp_raw_with_version().await?;
        if version
            .extensions
            .get("posix-rename@openssh.com")
            .map(String::as_str)
            != Some("1")
        {
            return Err(SessionError::Unsupported(
                "server must advertise posix-rename@openssh.com version 1",
            )
            .into());
        }
        let mut attrs = FileAttributes::empty();
        attrs.permissions = Some(0o600);
        match remote_io(raw.lstat(path)).await {
            Ok(existing) => {
                if !existing.attrs.file_type().is_file() {
                    return Err(SessionError::Invalid(
                        "atomic replacement requires a confirmed regular file",
                    )
                    .into());
                }
                if let Some(mode) = existing.attrs.permissions {
                    attrs.permissions = Some(mode & 0o777);
                }
            }
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile => {}
            Err(error) => return Err(sftp_error(error).into()),
        }
        let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
        let temporary = format!("{parent}/.{name}.keelshell-{}.tmp", uuid::Uuid::new_v4());
        let ticket = match (transfer, mutation) {
            (Some(context), _) => context.reservation.clone().ok_or(SessionError::Worker)?,
            (_, Some(scope)) => scope.ticket()?,
            _ => return Err(SessionError::Worker.into()),
        };
        let temporary_owner =
            ticket.add_temporary(queue::remote_claim(self, &temporary, true, true).await?)?;
        let mut guard = AtomicTemporary {
            reservation: Some(temporary_owner),
            raw: Arc::new(raw),
            temporary: Some(temporary.clone()),
            handle: None,
            cleanups: self.cleanups.clone(),
            connection: self._connection.clone(),
            runtime: tokio::runtime::Handle::current(),
        };
        let opening = guard.raw.open(
            temporary,
            OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
            attrs,
        );
        let opened = tracked_remote(transfer, mutation, opening).await;
        let definitely_rejected = match (transfer, mutation) {
            (Some(context), _) => !context.mutation_pending(),
            (_, Some(scope)) => !scope.pending.load(Ordering::Acquire),
            _ => false,
        };
        let handle = match opened {
            Ok(opened) => opened.handle,
            Err(error) => {
                // A definite failure did not create our file. In particular,
                // O_EXCL collisions must never remove somebody else's entry.
                if definitely_rejected
                    || transfer.is_some_and(|transfer| !transfer.mutation_pending())
                {
                    guard.temporary = None;
                }
                return Err(error.into());
            }
        };
        guard.handle = Some(handle.clone());
        // Keep chunk storage out of this future and its caller's debug stack.
        let mut buffer = vec![0_u8; 32 * 1024];
        let mut offset = 0_u64;
        loop {
            writer_checkpoint(transfer).await?;
            let count = source.read(&mut buffer).await?;
            if let Some(transfer) = transfer {
                transfer.confirmed_io();
            }
            if count == 0 {
                break;
            }
            let writing = guard.raw.write(&handle, offset, buffer[..count].to_vec());
            tracked_remote(transfer, mutation, writing).await?;
            offset = offset
                .checked_add(count as u64)
                .ok_or(SessionError::Invalid("upload exceeds u64 size"))?;
            writer_progress(transfer, count as u64).await?;
        }
        // A sent CLOSE invalidates the descriptor even before its reply. Do
        // not let cleanup retry it if cancellation drops the pending reply.
        tracked_remote(transfer, mutation, async {
            // Clear only when the operation is polled after authority checks.
            // Pre-I/O cancellation still lets owned cleanup close this handle.
            guard.handle = None;
            guard.raw.close(&handle).await
        })
        .await?;
        if let Some(reviewed) = reviewed {
            Box::pin(self.verify_regular_snapshot(reviewed)).await?;
        }
        writer_checkpoint(transfer).await?;
        if let Some(plan) = file_review {
            // The nested metadata checks own protocol futures. Keep that state
            // off the shared writer frame, even when this call has no review.
            Box::pin(self.validate_file_transfer(plan)).await?;
        }
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
        let publication = async {
            match guard
                .raw
                .extended("posix-rename@openssh.com", payload)
                .await?
            {
                Packet::Status(status) => Ok(status),
                _ => Err(russh_sftp::client::error::Error::UnexpectedBehavior(
                    "atomic rename returned no STATUS acknowledgement".into(),
                )),
            }
        };
        let published = tracked_remote(transfer, mutation, publication).await?;
        match published.status_code {
            StatusCode::Ok => {}
            _ => {
                return Err(sftp_error(format!(
                    "atomic rename: {:?}: {}",
                    published.status_code, published.error_message
                ))
                .into());
            }
        }
        guard.temporary = None;
        Ok(offset)
    }

    /// Stream into a new local file, refusing to overwrite an existing path.
    /// An interrupted operation leaves the partial file visibly present.
    /// Completed reads and local writes renew the idle timeout; an unanswered
    /// request or incomplete local mutation remains bounded by that timeout.
    pub async fn download(&self, remote: &str, local: &Path) -> Result<u64> {
        valid_path(remote)?;
        let scope = self
            .reserve_transfer(&TransferSpec::download(remote, local), false)
            .await?;
        let context = scope.context()?;
        let result = context
            .run(self.timeout, "SFTP download idle wait", async {
                scope.revalidate().await?;
                tokio::select! {
                    biased;
                    _=self.cancelled()=>Err(TransferExecutionError::from(SessionError::Closed)),
                    result=self.queued_download(remote,local,&context)=>result,
                }?;
                Ok(context.bytes())
            })
            .await
            .map_err(transfer_session_error);
        scope.complete(result).await
    }

    /// Create an empty remote directory under shared application exclusion.
    pub async fn mkdir(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        let scope = self.reserve_remote(&[path], true, &|| true).await?;
        let result = deadline(self.timeout, "SFTP mkdir", scope.mkdir_remote(path)).await;
        scope.complete(result).await
    }
    /// Rename an entry, reserving both canonical pathname endpoints and subtrees.
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        valid_path(from)?;
        valid_path(to)?;
        let scope = self.reserve_remote(&[from, to], true, &|| true).await?;
        let result = deadline(self.timeout, "SFTP rename", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            scope.remote(raw.0.rename(from, to)).await?;
            Ok(())
        })
        .await;
        scope.complete(result).await
    }
    /// Remove one file/link; never recursively remove a tree. Shared quarantine
    /// and active target admission apply even after separate action approval.
    pub async fn remove(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        let scope = self.reserve_remote(&[path], true, &|| true).await?;
        let result = deadline(self.timeout, "SFTP remove", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            scope.remote(raw.0.remove(path)).await?;
            Ok(())
        })
        .await;
        scope.complete(result).await
    }
    /// Remove an empty directory while reserving its entire subtree.
    pub async fn rmdir(&self, path: &str) -> Result<()> {
        valid_path(path)?;
        let scope = self.reserve_remote(&[path], true, &|| true).await?;
        let result = deadline(self.timeout, "SFTP rmdir", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            scope.remote(raw.0.rmdir(path)).await?;
            Ok(())
        })
        .await;
        scope.complete(result).await
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
        let scope = self
            .reserve_remote(&[&reviewed.path], false, &|| true)
            .await?;
        let result = deadline(self.timeout, "SFTP set permissions", async {
            for parent in reviewed_parent_paths(&reviewed.path)? {
                let attrs = remote_io(self.inner.symlink_metadata(&parent))
                    .await
                    .map_err(sftp_error)?;
                if attrs.file_type().is_symlink() || !attrs.file_type().is_dir() {
                    return Err(SessionError::Invalid(
                        "permission target has an unsafe parent directory",
                    ));
                }
            }
            let before = remote_io(self.inner.symlink_metadata(&reviewed.path))
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
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            scope
                .remote(raw.0.setstat(&reviewed.path, attributes))
                .await?;
            let after = remote_io(self.inner.symlink_metadata(&reviewed.path))
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
        .await;
        scope.complete(result).await
    }

    /// Stop this subsystem's relay and request its channel close. Ordinary
    /// cleanup preserves SSH; a stalled protocol close can force shared shutdown.
    /// Completion is not a remote acknowledgement or a rollback of file writes.
    pub async fn close(&self) -> Result<()> {
        self.stopped.send_replace(true);
        // Queue the sentinel while the relay is still alive. Cancelling the
        // relay first races the upstream writer's receiver and can turn an
        // otherwise successful cleanup into `SendError: channel closed`.
        let close_result = deadline(
            self.timeout.min(Duration::from_secs(2)),
            "SFTP close",
            async {
                match self.inner.close().await {
                    Ok(()) => Ok(()),
                    Err(error) if sftp_close_is_already_closed(&error) => Ok(()),
                    Err(error) => Err(sftp_error(error)),
                }
            },
        )
        .await;

        // Do not wait for a raw writer to consume the queued close sentinel.
        self.channel_owner.cancel();
        let pending = self
            .cleanups
            .lock()
            .map(|mut tasks| std::mem::take(&mut *tasks))
            .unwrap_or_default();
        for task in pending {
            let _ = task.await;
        }
        close_result
    }
}

// russh-sftp 3.0 wraps a closed writer receiver in `UnexpectedBehavior` rather
// than exposing a dedicated channel-closed variant. Keep this compatibility
// check in one place so a future upstream variant can be matched structurally.
fn sftp_close_is_already_closed(error: &russh_sftp::client::error::Error) -> bool {
    matches!(
        error,
        russh_sftp::client::error::Error::UnexpectedBehavior(message)
            if message == "SendError: channel closed"
    )
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
    fn is_closed(&self) -> bool {
        *self.stopped.borrow() || self._connection.is_closed()
    }
    async fn cancelled(&self) {
        let mut stopped = self.stopped.subscribe();
        while !*stopped.borrow_and_update() {
            if stopped.changed().await.is_err() {
                break;
            }
        }
    }

    /// Start a transfer queue on this exact authenticated connection.
    ///
    /// The default is one worker for compatibility. Use
    /// [`TransferQueue::set_parallelism`] to admit up to four independent jobs;
    /// overlapping source/destination paths remain serialized.
    pub fn transfer_queue(self: Arc<Self>) -> TransferQueue {
        TransferQueue::new(self)
    }

    async fn queued_atomic_upload(
        &self,
        local: &Path,
        remote: &str,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        let mut source = local_io(tokio::fs::File::open(local)).await?;
        if !local_io(source.metadata()).await?.is_file() {
            return Err(SessionError::Invalid("file upload requires a regular file").into());
        }
        self.replace_from_reader_checked(remote, &mut source, None, Some(context), None, None)
            .await?;
        Ok(())
    }

    async fn queued_upload(
        &self,
        local: &Path,
        remote: &str,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        let mut source = local_io(tokio::fs::File::open(local))
            .await
            .map_err(SessionError::from)?;
        if !local_io(source.metadata())
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
        let handle = context
            .remote_mutation(raw.0.open(
                remote,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                FileAttributes::empty(),
            ))
            .await?
            .handle;
        let mut buffer = vec![0; TRANSFER_CHUNK_SIZE];
        let mut offset = 0;
        loop {
            context.checkpoint().await?;
            let count = source.read(&mut buffer).await.map_err(SessionError::from)?;
            context.confirmed_io();
            if count == 0 {
                break;
            }
            context
                .remote_mutation(raw.0.write(&handle, offset, buffer[..count].to_vec()))
                .await?;
            offset += count as u64;
            context.progress(count as u64).await?;
        }
        context.remote_mutation(raw.0.close(handle)).await?;
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
        let handle = remote_io(raw.0.open(remote, OpenFlags::READ, FileAttributes::empty()))
            .await
            .map_err(sftp_error)?
            .handle;
        let mut target = context
            .local_mutation(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(local),
            )
            .await?;
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
                    // EOF confirms this owner's read even though it adds no bytes.
                    context.confirmed_io();
                    break;
                }
                Err(error) => return Err(sftp_error(error).into()),
            };
            context.confirmed_io();
            context
                .local_mutation(async {
                    target.write_all(&data).await?;
                    target.flush().await
                })
                .await?;
            offset += data.len() as u64;
            context.progress(data.len() as u64).await?;
        }
        remote_io(raw.0.close(handle)).await.map_err(sftp_error)?;
        Ok(())
    }
}

const TRANSFER_CHUNK_SIZE: usize = 64 * 1024;
// Queue jobs retain typed cancellation through the shared atomic writer.
// Mapping it to SessionError::Closed belongs only at public Result boundaries:
// cancellation can arrive after an outer select polled its first branch.
async fn writer_checkpoint(transfer: Option<&TransferContext>) -> TransferExecutionResult<()> {
    match transfer {
        Some(transfer) => transfer.checkpoint().await,
        None => Ok(()),
    }
}

async fn writer_progress(
    transfer: Option<&TransferContext>,
    count: u64,
) -> TransferExecutionResult<()> {
    match transfer {
        Some(transfer) => transfer.progress(count).await,
        None => Ok(()),
    }
}

type TransferExecutionResult<T = u64> = std::result::Result<T, TransferExecutionError>;
#[derive(Debug)]
enum TransferExecutionError {
    Cancelled(u64),
    Error(SessionError),
}
fn transfer_session_error(error: TransferExecutionError) -> SessionError {
    match error {
        TransferExecutionError::Cancelled(_) => SessionError::Closed,
        TransferExecutionError::Error(error) => error,
    }
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

async fn tracked_remote<T>(
    transfer: Option<&TransferContext>,
    mutation: Option<&FileMutationScope<'_>>,
    operation: impl std::future::Future<
        Output = std::result::Result<T, russh_sftp::client::error::Error>,
    >,
) -> Result<T> {
    match (transfer, mutation) {
        (Some(context), Some(scope)) => {
            let result = scope.remote(operation).await;
            if result.is_ok() {
                context.confirmed_io();
            }
            result
        }
        (Some(context), None) => context
            .remote_mutation(operation)
            .await
            .map_err(transfer_session_error),
        (_, Some(scope)) => scope.remote(operation).await,
        _ => Err(SessionError::Worker),
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
    reservation: Option<queue::TemporaryReservation>,
    raw: Arc<OwnedRawSftpSession>,
    temporary: Option<String>,
    handle: Option<String>,
    cleanups: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    connection: SshSession,
    runtime: tokio::runtime::Handle,
}
impl Drop for AtomicTemporary {
    fn drop(&mut self) {
        let ticket = self.reservation.take();
        let Some(path) = self.temporary.take() else {
            let _ = self.raw.close_session();
            if let Some(ticket) = ticket {
                ticket.finish(true);
            }
            return;
        };
        let raw = self.raw.clone();
        let handle = self.handle.take();
        let connection = self.connection.clone();
        let task = self.runtime.spawn(async move {
            let _connection = connection;
            let cleaned = tokio::time::timeout(Duration::from_secs(2), async {
                if let Some(handle) = handle {
                    raw.close(handle).await?;
                }
                raw.remove(path).await
            })
            .await;
            let known = matches!(
                &cleaned,
                Ok(Ok(_)) | Ok(Err(russh_sftp::client::error::Error::Status(_)))
            );
            if let Some(ticket) = ticket {
                ticket.finish(known);
            }
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
    fn closed_writer_send_error_is_idempotent_but_other_errors_are_not() {
        assert!(sftp_close_is_already_closed(
            &russh_sftp::client::error::Error::UnexpectedBehavior(
                "SendError: channel closed".into()
            )
        ));
        assert!(!sftp_close_is_already_closed(
            &russh_sftp::client::error::Error::UnexpectedBehavior("SendError: other".into())
        ));
        assert!(!sftp_close_is_already_closed(
            &russh_sftp::client::error::Error::Timeout
        ));
    }

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
