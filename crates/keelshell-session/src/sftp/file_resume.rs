//! Read-only continuation review and verified, nontruncating execution.
use super::*;

mod io;
use io::*;
use std::time::SystemTime;

const MAX_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const REVIEW_TIMEOUT: Duration = Duration::from_secs(30);

/// A read-only review of a regular source and its exact existing target prefix.
///
/// The complete source is SHA-256 fingerprinted and every existing destination
/// byte is compared. Execution repeats this validation before writing. Plans
/// bind the authenticated SSH connection; reconnecting requires a fresh plan.
/// Paths must be absolute, portable names with no symlink ancestors. A transfer
/// can still fail after writing if another process mutates files concurrently:
/// portable SFTP v3 does not offer atomic version checks or `NOFOLLOW` opens.
#[derive(Clone)]
pub struct FileResumePlan {
    connection: SshSession,
    spec: TransferSpec,
    source: Snapshot,
    target: Option<Snapshot>,
    digest: [u8; 32],
    allow_missing: bool,
}
impl std::fmt::Debug for FileResumePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileResumePlan")
            .field("spec", &self.spec)
            .field("bytes", &self.bytes())
            .field("existing_bytes", &self.existing_bytes())
            .finish_non_exhaustive()
    }
}
impl FileResumePlan {
    /// Reviewed local source or destination path.
    pub fn local_path(&self) -> &Path {
        &self.spec.local
    }
    /// Reviewed remote source or destination path.
    pub fn remote_path(&self) -> &str {
        &self.spec.remote
    }
    /// Direction of this transfer.
    pub fn direction(&self) -> TransferDirection {
        self.spec.direction
    }
    /// Complete source size, including the already present prefix.
    pub fn bytes(&self) -> u64 {
        self.source.size
    }
    /// Existing, byte-for-byte verified destination prefix length.
    pub fn existing_bytes(&self) -> u64 {
        self.target.as_ref().map_or(0, |s| s.size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    size: u64,
    modified: Option<SystemTime>,
    remote_modified: Option<u32>,
    mode: Option<u32>,
    identity: Option<(u64, u64)>,
}
fn local_snapshot(metadata: &std::fs::Metadata) -> Result<Snapshot> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(SessionError::Invalid("resume requires a regular file"));
    }
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        Some((metadata.dev(), metadata.ino()))
    };
    #[cfg(not(unix))]
    let identity = None;
    Ok(Snapshot {
        size: metadata.len(),
        modified: metadata.modified().ok(),
        remote_modified: None,
        mode: None,
        identity,
    })
}
fn remote_snapshot(attrs: FileAttributes) -> Result<Snapshot> {
    if attrs.permissions.is_none_or(|p| p & 0o170000 != 0o100000) {
        return Err(SessionError::Invalid(
            "remote resume requires proven regular file metadata",
        ));
    }
    Ok(Snapshot {
        size: attrs
            .size
            .ok_or(SessionError::Invalid("remote file size is missing"))?,
        modified: None,
        remote_modified: attrs.mtime,
        mode: attrs.permissions,
        identity: None,
    })
}

impl SftpSession {
    /// Review an existing partial file without modifying it. All existing
    /// destination bytes must equal the source prefix; equal-length files are
    /// fully compared too. Review is bounded to 16 GiB and 30 seconds.
    pub async fn plan_file_resume(&self, spec: TransferSpec) -> Result<FileResumePlan> {
        deadline(
            REVIEW_TIMEOUT,
            "SFTP resume review",
            self.scan_file_resume(spec, false),
        )
        .await
    }
    pub(super) async fn plan_file_resume_allow_missing(
        &self,
        spec: TransferSpec,
    ) -> Result<FileResumePlan> {
        deadline(
            REVIEW_TIMEOUT,
            "SFTP resume review",
            self.scan_file_resume(spec, true),
        )
        .await
    }
    async fn scan_file_resume(
        &self,
        spec: TransferSpec,
        allow_missing: bool,
    ) -> Result<FileResumePlan> {
        directory::validate_local_root(&spec.local)?;
        directory::validate_remote_root(&spec.remote)?;
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let (source_snapshot, target_snapshot) =
            inspect_paths(&raw.0, &spec, allow_missing).await?;
        let mut source = open_source(&raw.0, &spec).await?;
        ensure_snapshot(&source.snapshot().await?, &source_snapshot)?;
        let mut target = if target_snapshot.is_some() {
            Some(open_target(&raw.0, &spec, false, false).await?)
        } else {
            None
        };
        if let (Some(file), Some(snapshot)) = (&mut target, &target_snapshot) {
            ensure_snapshot(&file.snapshot().await?, snapshot)?;
        }
        let digest = verify_content(
            &mut source,
            target.as_mut(),
            source_snapshot.size,
            target_snapshot.as_ref().map_or(0, |s| s.size),
        )
        .await?;
        ensure_snapshot(&source.snapshot().await?, &source_snapshot)?;
        if let (Some(file), Some(snapshot)) = (&mut target, &target_snapshot) {
            ensure_snapshot(&file.snapshot().await?, snapshot)?;
        }
        let current = inspect_paths(&raw.0, &spec, allow_missing).await?;
        if current != (source_snapshot.clone(), target_snapshot.clone()) {
            return Err(SessionError::Invalid("resume path changed during review"));
        }
        source.close(None).await?;
        if let Some(target) = target {
            target.close(None).await?;
        }
        Ok(FileResumePlan {
            connection: self._connection.clone(),
            spec,
            source: source_snapshot,
            target: target_snapshot,
            digest,
            allow_missing,
        })
    }
    pub(super) async fn validate_file_resume(&self, plan: &FileResumePlan) -> Result<()> {
        if !Arc::ptr_eq(&plan.connection.handle, &self._connection.handle) {
            return Err(SessionError::Invalid(
                "resume plan belongs to a different SSH connection",
            ));
        }
        let current = deadline(
            REVIEW_TIMEOUT,
            "SFTP resume validation",
            self.scan_file_resume(plan.spec.clone(), plan.allow_missing),
        )
        .await?;
        if current.source != plan.source
            || current.target != plan.target
            || current.digest != plan.digest
        {
            return Err(SessionError::Invalid(
                "resume source or destination changed after review",
            ));
        }
        Ok(())
    }
    pub(super) async fn validate_completed_file_resume(&self, plan: &FileResumePlan) -> Result<()> {
        let current = self.scan_file_resume(plan.spec.clone(), false).await?;
        if current.source != plan.source
            || current.digest != plan.digest
            || current.existing_bytes() != plan.bytes()
        {
            return Err(SessionError::Invalid(
                "completed resume file changed during directory transfer",
            ));
        }
        Ok(())
    }
    pub(super) async fn execute_file_resume(
        &self,
        plan: &FileResumePlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        context.checkpoint().await?;
        context
            .validation(
                REVIEW_TIMEOUT,
                "resume transfer revalidation",
                self.validate_file_resume(plan),
            )
            .await?;
        context.checkpoint().await?;
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let mut source = open_source(&raw.0, &plan.spec).await?;
        ensure_snapshot(&source.snapshot().await?, &plan.source)?;
        // Validate through the exact opened destination handle before any write.
        // A missing destination is created only after the full source recheck.
        let mut target = if plan.target.is_some() {
            Some(open_target(&raw.0, &plan.spec, true, false).await?)
        } else {
            None
        };
        if let (Some(file), Some(snapshot)) = (&mut target, &plan.target) {
            ensure_snapshot(&file.snapshot().await?, snapshot)?;
        }
        let digest = context
            .validation(
                REVIEW_TIMEOUT,
                "resume descriptor validation",
                verify_content(
                    &mut source,
                    target.as_mut(),
                    plan.bytes(),
                    plan.existing_bytes(),
                ),
            )
            .await?;
        if digest != plan.digest {
            return Err(SessionError::Invalid("resume source changed before writing").into());
        }
        let current = inspect_paths(&raw.0, &plan.spec, plan.allow_missing).await?;
        if current != (plan.source.clone(), plan.target.clone()) {
            return Err(SessionError::Invalid("resume destination changed before writing").into());
        }
        if target.is_none() {
            target = Some(io::create_transfer_target(&raw.0, &plan.spec, context).await?);
        }
        let mut target = target.ok_or(SessionError::Invalid("resume target is unavailable"))?;
        let mut generation = context.resume_generation();
        context.progress(plan.existing_bytes()).await?;
        let mut offset = plan.existing_bytes();
        let mut buffer = vec![0; TRANSFER_CHUNK_SIZE];
        while offset < plan.bytes() {
            context.checkpoint().await?;
            if generation != context.resume_generation() {
                let named = context
                    .validation(
                        REVIEW_TIMEOUT,
                        "paused resume revalidation",
                        self.scan_file_resume(plan.spec.clone(), false),
                    )
                    .await?;
                ensure_snapshot(&named.source, &plan.source)?;
                if named.digest != plan.digest
                    || named.existing_bytes() != offset
                    || named.target.as_ref() != Some(&target.snapshot().await?)
                {
                    return Err(SessionError::Invalid("resume files changed while paused").into());
                }
                // Rebind descriptors to the verified names. A remote rename can
                // leave an old handle valid without exposing an inode identifier.
                source.close(None).await?;
                target.close(Some(context)).await?;
                source = open_source(&raw.0, &plan.spec).await?;
                target = open_target(&raw.0, &plan.spec, true, false).await?;
                ensure_snapshot(&source.snapshot().await?, &plan.source)?;
                if named.target.as_ref() != Some(&target.snapshot().await?) {
                    return Err(SessionError::Invalid(
                        "resume destination changed while reopening",
                    )
                    .into());
                }
                let digest = context
                    .validation(
                        REVIEW_TIMEOUT,
                        "reopened resume validation",
                        verify_content(&mut source, Some(&mut target), plan.bytes(), offset),
                    )
                    .await?;
                if digest != plan.digest {
                    return Err(SessionError::Invalid("resume source changed while paused").into());
                }
                generation = context.resume_generation();
            }
            let count = source
                .read_at(
                    offset,
                    &mut buffer[..(plan.bytes() - offset).min(TRANSFER_CHUNK_SIZE as u64) as usize],
                )
                .await?;
            if count == 0 {
                return Err(SessionError::Invalid("resume source shrank while copying").into());
            }
            context.confirmed_io();
            target.write_at(offset, &buffer[..count], context).await?;
            offset += count as u64;
            context.progress(count as u64).await?;
        }
        context.checkpoint().await?;
        ensure_snapshot(&source.snapshot().await?, &plan.source)?;
        // Detect concurrent same-size content changes before claiming completion.
        let final_digest = context
            .validation(
                REVIEW_TIMEOUT,
                "completed resume validation",
                verify_content(&mut source, Some(&mut target), plan.bytes(), plan.bytes()),
            )
            .await?;
        if final_digest != plan.digest {
            return Err(SessionError::Invalid("resume source changed during transfer").into());
        }
        let final_paths = inspect_paths(&raw.0, &plan.spec, false).await?;
        ensure_snapshot(&final_paths.0, &plan.source)?;
        if final_paths
            .1
            .as_ref()
            .is_none_or(|s| s.size != plan.bytes())
            || final_paths.1.as_ref() != Some(&target.snapshot().await?)
        {
            return Err(SessionError::Invalid("resume destination changed during transfer").into());
        }
        source.close(None).await?;
        target.close(Some(context)).await?;
        // Reopen the named destination too: a rename while paused can replace
        // the path while the old SFTP handle still addresses the original file.
        context.checkpoint().await?;
        context
            .validation(
                REVIEW_TIMEOUT,
                "completed resume named-file validation",
                self.validate_completed_file_resume(plan),
            )
            .await?;
        Ok(())
    }
}
