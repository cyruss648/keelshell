//! Read-only file review with a retained local source descriptor.

use super::*;
use std::fs::Metadata;
use std::io::SeekFrom;
use std::time::SystemTime;
use tokio::io::AsyncSeekExt;

/// Maximum regular-file source size accepted by a reviewed transfer.
pub const MAX_REVIEWED_FILE_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalStamp {
    bytes: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(windows)]
    attributes: u32,
}

impl LocalStamp {
    fn inspect(metadata: &Metadata) -> Result<Self> {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(SessionError::Invalid(
                "reviewed transfer requires a regular local file",
            ));
        }
        #[cfg(windows)]
        let attributes = {
            use std::os::windows::fs::MetadataExt;
            let attributes = metadata.file_attributes();
            if attributes & 0x400 != 0 {
                return Err(SessionError::Invalid(
                    "reviewed transfer rejects local reparse points",
                ));
            }
            attributes
        };
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            (metadata.dev(), metadata.ino())
        };
        Ok(Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            identity,
            #[cfg(windows)]
            attributes,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteStamp {
    bytes: u64,
    mode: u32,
    modified: Option<u32>,
}

impl RemoteStamp {
    fn inspect(entry: &RemoteEntry) -> Result<Self> {
        let mode = entry
            .permissions
            .filter(|mode| mode & 0o170000 == 0o100000)
            .ok_or(SessionError::Invalid(
                "reviewed transfer requires explicit remote regular-file type",
            ))?;
        if entry.is_directory || entry.is_symlink {
            return Err(SessionError::Invalid(
                "reviewed transfer rejects remote links and special files",
            ));
        }
        Ok(Self {
            bytes: entry.size.ok_or(SessionError::Invalid(
                "reviewed transfer requires a source size",
            ))?,
            mode,
            modified: entry.modified,
        })
    }
    fn attributes(attributes: FileAttributes) -> Result<Self> {
        let mode = attributes
            .permissions
            .filter(|mode| mode & 0o170000 == 0o100000)
            .ok_or(SessionError::Invalid(
                "opened reviewed source is not a confirmed regular file",
            ))?;
        Ok(Self {
            bytes: attributes.size.ok_or(SessionError::Invalid(
                "opened reviewed source size is unknown",
            ))?,
            mode,
            modified: attributes.mtime,
        })
    }
}

/// An immutable read-only regular-file transfer review on one SSH connection.
///
/// Uploads retain the exact opened local source descriptor through approval and
/// queueing. Execution checks the named metadata and the retained descriptor
/// before any destination mutation. Downloads check the opened remote handle
/// before creating the local target; local targets must remain absent. Portable
/// SFTP metadata is an observation, not a lock or an atomic no-follow/version
/// guarantee against other processes. Source contents are not copied or hashed
/// during this metadata-only preparation.
#[derive(Clone)]
pub struct FileTransferPlan {
    connection: SshSession,
    spec: TransferSpec,
    local: Option<LocalStamp>,
    remote: Option<RemoteStamp>,
    source: Option<Arc<tokio::sync::Mutex<tokio::fs::File>>>,
    metadata_descriptor: Option<Arc<tokio::fs::File>>,
    bytes: u64,
}

impl std::fmt::Debug for FileTransferPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileTransferPlan")
            .field("spec", &self.spec)
            .field("bytes", &self.bytes)
            .field("replaces_existing", &self.replaces_existing())
            .finish_non_exhaustive()
    }
}

impl FileTransferPlan {
    /// Exact reviewed local source or destination.
    pub fn local_path(&self) -> &Path {
        &self.spec.local
    }
    /// Exact reviewed remote source or destination.
    pub fn remote_path(&self) -> &str {
        &self.spec.remote
    }
    /// Reviewed transfer direction.
    pub fn direction(&self) -> TransferDirection {
        self.spec.direction
    }
    /// Reviewed source length; never an unknown value replaced by zero.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Whether the upload review observed an existing regular destination.
    pub fn replaces_existing(&self) -> bool {
        self.direction() == TransferDirection::Upload && self.remote.is_some()
    }
}

async fn local_parent(path: &Path) -> Result<()> {
    directory::validate_local_root(path)?;
    // Walk root first: reject a link/reparse point before inspecting descendants.
    let parent = path.parent().ok_or(SessionError::Invalid(
        "reviewed transfer needs a local parent",
    ))?;
    for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        // A Windows drive/verbatim prefix alone is not a rooted directory.
        if !ancestor.has_root() {
            continue;
        }
        let metadata = local_io(tokio::fs::symlink_metadata(ancestor)).await?;
        #[cfg(windows)]
        let reparse = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let reparse = false;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || reparse {
            return Err(SessionError::Invalid(
                "reviewed transfer ancestor is a link or not a directory",
            ));
        }
    }
    Ok(())
}

async fn inspect_local(path: &Path) -> Result<Option<LocalStamp>> {
    local_parent(path).await?;
    match local_io(tokio::fs::symlink_metadata(path)).await {
        Ok(metadata) => Ok(Some(LocalStamp::inspect(&metadata)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

impl SftpSession {
    /// Prepare one regular-file transfer using bounded metadata-only I/O.
    /// No destination is created or written. Upload targets may be absent or
    /// regular files; downloads only admit absent local targets. All paths must
    /// be absolute, portable and free of links or parent traversal.
    pub async fn plan_file_transfer(&self, spec: TransferSpec) -> Result<FileTransferPlan> {
        deadline(
            self.timeout,
            "SFTP file transfer review",
            self.inspect_file_transfer(spec),
        )
        .await
    }

    async fn inspect_file_transfer(&self, spec: TransferSpec) -> Result<FileTransferPlan> {
        directory::validate_remote_root(&spec.remote)?;
        let local = inspect_local(&spec.local).await?;
        let remote = self
            .inspect_entry(&spec.remote)
            .await?
            .as_ref()
            .map(RemoteStamp::inspect)
            .transpose()?;
        let (bytes, source, metadata_descriptor) = match spec.direction {
            TransferDirection::Upload => {
                let stamp = local
                    .as_ref()
                    .ok_or(SessionError::Invalid("reviewed upload source is missing"))?;
                let source = local_io(tokio::fs::File::open(&spec.local)).await?;
                if LocalStamp::inspect(&local_io(source.metadata()).await?)? != *stamp
                    || inspect_local(&spec.local).await? != local
                {
                    return Err(SessionError::Invalid(
                        "local upload source changed during review",
                    ));
                }
                let metadata_descriptor = Arc::new(local_io(source.try_clone()).await?);
                (
                    stamp.bytes,
                    Some(Arc::new(tokio::sync::Mutex::new(source))),
                    Some(metadata_descriptor),
                )
            }
            TransferDirection::Download => {
                if local.is_some() {
                    return Err(SessionError::Invalid(
                        "reviewed download target already exists",
                    ));
                }
                (
                    remote
                        .as_ref()
                        .ok_or(SessionError::Invalid("reviewed download source is missing"))?
                        .bytes,
                    None,
                    None,
                )
            }
        };
        if bytes > MAX_REVIEWED_FILE_BYTES {
            return Err(SessionError::Invalid("reviewed file exceeds 16 GiB"));
        }
        Ok(FileTransferPlan {
            connection: self._connection.clone(),
            spec,
            local,
            remote,
            source,
            metadata_descriptor,
            bytes,
        })
    }

    pub(super) async fn validate_file_transfer(&self, plan: &FileTransferPlan) -> Result<()> {
        if !Arc::ptr_eq(&plan.connection.handle, &self._connection.handle) {
            return Err(SessionError::Invalid(
                "file transfer plan belongs to another SSH connection",
            ));
        }
        let local = inspect_local(&plan.spec.local).await?;
        let remote = self
            .inspect_entry(&plan.spec.remote)
            .await?
            .as_ref()
            .map(RemoteStamp::inspect)
            .transpose()?;
        if local != plan.local || remote != plan.remote {
            return Err(SessionError::Invalid(
                "file source or destination changed after review",
            ));
        }
        if let Some(descriptor) = &plan.metadata_descriptor
            && Some(LocalStamp::inspect(
                &local_io(descriptor.metadata()).await?,
            )?) != plan.local
        {
            return Err(SessionError::Invalid("reviewed source descriptor changed"));
        }
        Ok(())
    }

    pub(super) async fn queued_reviewed_file(
        &self,
        plan: FileTransferPlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        context
            .validation(
                self.timeout,
                "reviewed file revalidation",
                self.validate_file_transfer(&plan),
            )
            .await?;
        context.checkpoint().await?;
        match plan.direction() {
            TransferDirection::Upload => {
                let mut source = plan
                    .source
                    .as_ref()
                    .ok_or(SessionError::Worker)?
                    .lock()
                    .await;
                let stamp = plan.local.as_ref().ok_or(SessionError::Worker)?;
                if LocalStamp::inspect(&local_io(source.metadata()).await?)? != *stamp {
                    return Err(
                        SessionError::Invalid("opened upload source changed after review").into(),
                    );
                }
                source.seek(SeekFrom::Start(0)).await?;
                // Bound streaming to the reviewed length. The atomic publisher
                // asynchronously checks the retained descriptor after EOF.
                let mut reader = ReviewedReader {
                    file: &mut source,
                    remaining: plan.bytes,
                };
                self.replace_from_reader_checked(
                    &plan.spec.remote,
                    &mut reader,
                    None,
                    Some(context),
                    None,
                    Some(&plan),
                )
                .await?;
                Ok(())
            }
            TransferDirection::Download => self.download_reviewed_file(&plan, context).await,
        }
    }

    async fn download_reviewed_file(
        &self,
        plan: &FileTransferPlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let handle = remote_io(raw.0.open(
            &plan.spec.remote,
            OpenFlags::READ,
            FileAttributes::empty(),
        ))
        .await
        .map_err(sftp_error)?
        .handle;
        let result: TransferExecutionResult<()> = async {
            let stamp = RemoteStamp::attributes(
                remote_io(raw.0.fstat(&handle))
                    .await
                    .map_err(sftp_error)?
                    .attrs,
            )?;
            if Some(&stamp) != plan.remote.as_ref() {
                return Err(
                    SessionError::Invalid("opened download source changed after review").into(),
                );
            }
            // The opened handle has been checked before create_new. A target
            // appearing since approval is refused without truncation.
            local_parent(&plan.spec.local).await?;
            let mut target = context
                .local_mutation(
                    tokio::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&plan.spec.local),
                )
                .await?;
            let mut offset = 0_u64;
            loop {
                context.checkpoint().await?;
                let length = plan
                    .bytes
                    .saturating_sub(offset)
                    .saturating_add(1)
                    .min(TRANSFER_CHUNK_SIZE as u64) as u32;
                let data = match raw.0.read(&handle, offset, length).await {
                    Ok(packet)
                        if !packet.data.is_empty() && packet.data.len() <= length as usize =>
                    {
                        packet.data
                    }
                    Ok(_) => {
                        return Err(
                            SessionError::Invalid("invalid reviewed SFTP read length").into()
                        );
                    }
                    Err(russh_sftp::client::error::Error::Status(status))
                        if status.status_code == StatusCode::Eof =>
                    {
                        break;
                    }
                    Err(error) => return Err(sftp_error(error).into()),
                };
                context.confirmed_io();
                if offset.saturating_add(data.len() as u64) > plan.bytes {
                    return Err(
                        SessionError::Invalid("reviewed source grew during download").into(),
                    );
                }
                context
                    .local_mutation(async {
                        target.write_all(&data).await?;
                        target.flush().await
                    })
                    .await?;
                offset += data.len() as u64;
                context.progress(data.len() as u64).await?;
            }
            context.confirmed_io();
            if offset != plan.bytes
                || RemoteStamp::attributes(
                    remote_io(raw.0.fstat(&handle))
                        .await
                        .map_err(sftp_error)?
                        .attrs,
                )? != stamp
            {
                return Err(SessionError::Invalid(
                    "reviewed download source changed while reading",
                )
                .into());
            }
            Ok(())
        }
        .await;
        let closed = remote_io(raw.0.close(handle)).await.map_err(sftp_error);
        result?;
        closed?;
        Ok(())
    }
}

/// Read exactly the reviewed number of bytes; the publisher separately checks
/// source metadata after EOF and before publishing the temporary file.
struct ReviewedReader<'a> {
    file: &'a mut tokio::fs::File,
    remaining: u64,
}

impl AsyncRead for ReviewedReader<'_> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Metadata validation is async and happens in the publisher after EOF.
        // This adapter prevents a growing source from streaming past the bound.
        let before = buffer.filled().len();
        let result = std::pin::Pin::new(&mut *self.file).poll_read(cx, buffer);
        if let std::task::Poll::Ready(Ok(())) = &result {
            let added = (buffer.filled().len() - before) as u64;
            if added > self.remaining || (added == 0 && self.remaining != 0) {
                return std::task::Poll::Ready(Err(std::io::Error::other(
                    "reviewed upload size changed",
                )));
            }
            self.remaining -= added;
        }
        result
    }
}
