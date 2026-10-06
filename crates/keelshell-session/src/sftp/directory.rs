//! Bounded directory snapshots and exclusive, nontransactional tree transfers.
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use super::*;

#[path = "directory_resume.rs"]
mod resume;
pub use resume::DirectoryResumePlan;

const MAX_DEPTH: usize = 32;
const MAX_ENTRIES: usize = 10_000;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

/// An immutable, read-only scan for an explicitly reviewed directory transfer.
///
/// Plans include empty directories and regular files only. Limits are 32 child
/// levels, 10,000 entries including the root, 4,096 path bytes and 16 GiB. Scans
/// have a fixed 30-second deadline. Execution renews the connection's idle wait
/// after confirmed I/O; read-only revalidation retains the scan limit. Symlinks,
/// special files and names unsafe on Windows are rejected on every platform.
/// The target root must be absent. Cancellation/failure can leave a partial tree;
/// no existing destination is intentionally replaced or automatically deleted.
#[derive(Clone)]
pub struct DirectoryTransferPlan {
    connection: SshSession,
    spec: TransferSpec,
    entries: Vec<Entry>,
    bytes: u64,
}

impl std::fmt::Debug for DirectoryTransferPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectoryTransferPlan")
            .field("spec", &self.spec)
            .field("entries", &self.entries.len())
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    relative: String,
    directory: bool,
    size: u64,
    modified: Option<Modified>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Modified {
    Local(SystemTime),
    Remote(u32),
}

impl DirectoryTransferPlan {
    /// Exact local path scanned for uploads or reviewed as the download target.
    pub fn local_path(&self) -> &Path {
        &self.spec.local
    }
    /// Exact remote path reviewed for this transfer.
    pub fn remote_path(&self) -> &str {
        &self.spec.remote
    }
    /// Transfer direction.
    pub fn direction(&self) -> TransferDirection {
        self.spec.direction
    }
    /// Number of regular files in the scan.
    pub fn files(&self) -> usize {
        self.entries.iter().filter(|entry| !entry.directory).count()
    }
    /// Number of directories, including the source root and empty directories.
    pub fn directories(&self) -> usize {
        self.entries.iter().filter(|entry| entry.directory).count()
    }
    /// Sum of the scanned regular file lengths.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl SftpSession {
    /// Scan a directory without creating output. Pass an upload/download spec
    /// whose source is a directory and whose target is a new directory name.
    /// All existing ancestors must be real directories, not symlinks. The
    /// returned snapshot must be reviewed before calling `enqueue_directory`.
    pub async fn plan_directory_transfer(
        &self,
        spec: TransferSpec,
    ) -> Result<DirectoryTransferPlan> {
        deadline(
            SCAN_TIMEOUT,
            "SFTP directory scan",
            self.scan_directory(spec),
        )
        .await
    }

    async fn scan_directory(&self, spec: TransferSpec) -> Result<DirectoryTransferPlan> {
        validate_remote_root(&spec.remote)?;
        validate_local_root(&spec.local)?;
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let (entries, bytes) = match spec.direction {
            TransferDirection::Upload => {
                local_directory_chain(&spec.local).await?;
                remote_absent_destination(&raw.0, &spec.remote).await?;
                scan_local(&spec.local).await?
            }
            TransferDirection::Download => {
                remote_directory_chain(&raw.0, &spec.remote).await?;
                local_absent_destination(&spec.local).await?;
                scan_remote(&raw.0, &spec.remote).await?
            }
        };
        for entry in &entries {
            validate_lengths(
                &local_child(&spec.local, &entry.relative),
                &remote_child(&spec.remote, &entry.relative),
            )?;
        }
        Ok(DirectoryTransferPlan {
            connection: self._connection.clone(),
            spec,
            entries,
            bytes,
        })
    }

    pub(super) async fn queued_directory(
        &self,
        plan: DirectoryTransferPlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        if !Arc::ptr_eq(&plan.connection.handle, &self._connection.handle) {
            return Err(SessionError::Invalid(
                "directory plan belongs to a different SSH connection",
            )
            .into());
        }

        let current = context
            .validation(
                SCAN_TIMEOUT,
                "directory transfer revalidation",
                self.scan_directory(plan.spec.clone()),
            )
            .await?;
        if current.entries != plan.entries {
            return Err(SessionError::Invalid(
                "directory changed after review; scan and review again",
            )
            .into());
        }
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        for entry in &plan.entries {
            context.checkpoint().await?;
            let local = local_child(&plan.spec.local, &entry.relative);
            let remote = remote_child(&plan.spec.remote, &entry.relative);
            match plan.spec.direction {
                TransferDirection::Upload => {
                    verify_local_entry(&local, entry).await?;
                    let parent = remote_parent(&remote)?;
                    remote_directory_chain(&raw.0, parent).await?;
                    if entry.directory {
                        remote_absent(&raw.0, &remote).await?;
                        let mut attrs = FileAttributes::empty();
                        attrs.permissions = Some(0o700);
                        context.remote_mutation(raw.0.mkdir(&remote, attrs)).await?;
                    } else {
                        self.upload_tree_file(&raw.0, &local, &remote, entry, context)
                            .await?;
                    }
                }
                TransferDirection::Download => {
                    verify_remote_entry(&raw.0, &remote, entry).await?;
                    local_directory_chain(
                        local
                            .parent()
                            .ok_or(SessionError::Invalid("local target has no parent"))?,
                    )
                    .await?;
                    if entry.directory {
                        context
                            .local_mutation(tokio::fs::create_dir(&local))
                            .await?;
                    } else {
                        self.download_tree_file(&raw.0, &remote, &local, entry, context)
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn upload_tree_file(
        &self,
        raw: &RawSftpSession,
        local: &Path,
        remote: &str,
        entry: &Entry,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        let mut source = local_io(tokio::fs::File::open(local)).await?;
        let metadata = local_io(source.metadata()).await?;
        if !metadata.is_file() || local_entry(&metadata, &entry.relative)? != *entry {
            return Err(SessionError::Invalid("local source changed after review").into());
        }
        // Exclusive creation is deliberate: replacing with POSIX rename would
        // overwrite a target introduced by another client after our scan.
        let mut attrs = FileAttributes::empty();
        attrs.permissions = Some(0o600);
        let handle = context
            .remote_mutation(raw.open(
                remote,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
                attrs,
            ))
            .await?
            .handle;
        // Allocate directly on the heap: an inline array becomes part of this
        // future and its enclosing cancellation/deadline/queue futures.
        let mut buffer = vec![0_u8; TRANSFER_CHUNK_SIZE];
        let mut offset = 0_u64;
        loop {
            context.checkpoint().await?;
            let count = source.read(&mut buffer).await?;
            context.confirmed_io();
            if count == 0 {
                break;
            }
            if offset.saturating_add(count as u64) > entry.size {
                return Err(SessionError::Invalid("local source grew after review").into());
            }
            context
                .remote_mutation(raw.write(&handle, offset, buffer[..count].to_vec()))
                .await?;
            offset += count as u64;
            context.progress(count as u64).await?;
        }
        context.remote_mutation(raw.close(handle)).await?;
        if offset != entry.size {
            return Err(SessionError::Invalid("local source shrank after review").into());
        }
        verify_local_entry(local, entry).await?;
        Ok(())
    }

    async fn download_tree_file(
        &self,
        raw: &RawSftpSession,
        remote: &str,
        local: &Path,
        entry: &Entry,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        let handle = remote_io(raw.open(remote, OpenFlags::READ, FileAttributes::empty()))
            .await
            .map_err(sftp_error)?
            .handle;
        let actual = remote_io(raw.fstat(&handle))
            .await
            .map_err(sftp_error)?
            .attrs;
        if remote_entry(&actual, &entry.relative)? != *entry {
            return Err(SessionError::Invalid("remote source changed while opening").into());
        }
        let mut target = context
            .local_mutation(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(local),
            )
            .await?;
        let mut offset = 0_u64;
        loop {
            context.checkpoint().await?;
            match raw.read(&handle, offset, TRANSFER_CHUNK_SIZE as u32).await {
                Ok(data) if !data.data.is_empty() => {
                    context.confirmed_io();
                    let count = data.data.len() as u64;
                    if offset.saturating_add(count) > entry.size {
                        return Err(SessionError::Invalid("remote source grew after review").into());
                    }
                    context
                        .local_mutation(async {
                            target.write_all(&data.data).await?;
                            target.flush().await
                        })
                        .await?;
                    offset += count;
                    context.progress(count).await?;
                }
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::Eof =>
                {
                    // EOF confirms this owner's read even though it adds no bytes.
                    context.confirmed_io();
                    break;
                }
                Ok(_) => {
                    return Err(
                        SessionError::Invalid("remote returned an empty non-EOF read").into(),
                    );
                }
                Err(error) => return Err(sftp_error(error).into()),
            }
        }
        remote_io(raw.close(handle)).await.map_err(sftp_error)?;
        target.flush().await?;
        if offset != entry.size {
            return Err(SessionError::Invalid("remote source shrank after review").into());
        }
        verify_remote_entry(raw, remote, entry).await?;
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        || matches!(
            stem.as_str(),
            "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
        );
    if name.is_empty()
        || name.len() > 255
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || name.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
        || reserved
    {
        return Err(SessionError::Invalid(
            "directory contains an unsafe or nonportable name",
        ));
    }
    Ok(())
}

pub(super) fn validate_remote_root(path: &str) -> Result<()> {
    if !path.starts_with('/') || path == "/" || path.ends_with('/') || path.len() > MAX_PATH_BYTES {
        return Err(SessionError::Invalid(
            "directory transfer requires an absolute named remote directory",
        ));
    }
    for name in path[1..].split('/') {
        validate_name(name)?;
    }
    Ok(())
}
pub(super) fn validate_local_root(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.file_name().is_none() || path.as_os_str().len() > MAX_PATH_BYTES
    {
        return Err(SessionError::Invalid(
            "directory transfer requires an absolute named local directory",
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(name) => validate_name(
                name.to_str()
                    .ok_or(SessionError::Invalid("local path is not UTF-8"))?,
            )?,
            Component::RootDir | Component::Prefix(_) => {}
            _ => {
                return Err(SessionError::Invalid(
                    "local transfer path contains traversal",
                ));
            }
        }
    }
    Ok(())
}
fn validate_lengths(local: &Path, remote: &str) -> Result<()> {
    if local.as_os_str().len() > MAX_PATH_BYTES || remote.len() > MAX_PATH_BYTES {
        Err(SessionError::Invalid("directory path exceeds 4096 bytes"))
    } else {
        Ok(())
    }
}
fn local_child(root: &Path, relative: &str) -> PathBuf {
    if relative.is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    }
}
fn remote_child(root: &str, relative: &str) -> String {
    if relative.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{relative}")
    }
}
pub(super) fn remote_parent(path: &str) -> Result<&str> {
    let (parent, _) = path
        .rsplit_once('/')
        .ok_or(SessionError::Invalid("remote path has no parent"))?;
    Ok(if parent.is_empty() { "/" } else { parent })
}

pub(super) async fn local_directory_chain(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let metadata = local_io(tokio::fs::symlink_metadata(ancestor)).await?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(SessionError::Invalid(
                "local directory ancestor is a symlink or is not a directory",
            ));
        }
    }
    Ok(())
}
async fn local_absent_destination(path: &Path) -> Result<()> {
    local_directory_chain(
        path.parent()
            .ok_or(SessionError::Invalid("local target has no parent"))?,
    )
    .await?;
    match local_io(tokio::fs::symlink_metadata(path)).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(SessionError::Invalid(
            "local destination already exists; choose a new directory",
        )),
    }
}
pub(super) async fn remote_directory_chain(raw: &RawSftpSession, path: &str) -> Result<()> {
    let mut current = path;
    loop {
        let attrs = remote_io(raw.lstat(current))
            .await
            .map_err(sftp_error)?
            .attrs;
        if !attrs.file_type().is_dir() {
            return Err(SessionError::Invalid(
                "remote directory ancestor is a symlink or is not a directory",
            ));
        }
        if current == "/" {
            break;
        }
        current = remote_parent(current)?;
    }
    Ok(())
}
async fn remote_absent(raw: &RawSftpSession, path: &str) -> Result<()> {
    match remote_io(raw.lstat(path)).await {
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == StatusCode::NoSuchFile =>
        {
            Ok(())
        }
        Err(error) => Err(sftp_error(error)),
        Ok(_) => Err(SessionError::Invalid(
            "remote destination already exists; choose a new directory",
        )),
    }
}
async fn remote_absent_destination(raw: &RawSftpSession, path: &str) -> Result<()> {
    remote_directory_chain(raw, remote_parent(path)?).await?;
    remote_absent(raw, path).await
}

fn local_entry(metadata: &std::fs::Metadata, relative: &str) -> Result<Entry> {
    if !metadata.is_dir() && !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(SessionError::Invalid(
            "local tree contains a symlink or special file",
        ));
    }
    Ok(Entry {
        relative: relative.into(),
        directory: metadata.is_dir(),
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified: if metadata.is_file() {
            metadata.modified().ok().map(Modified::Local)
        } else {
            None
        },
    })
}
fn remote_entry(attrs: &FileAttributes, relative: &str) -> Result<Entry> {
    let kind = attrs.file_type();
    if !kind.is_dir() && !kind.is_file() {
        return Err(SessionError::Invalid(
            "remote tree contains a symlink, special file, or missing type",
        ));
    }
    Ok(Entry {
        relative: relative.into(),
        directory: kind.is_dir(),
        size: if kind.is_file() {
            attrs
                .size
                .ok_or(SessionError::Invalid("remote file length is missing"))?
        } else {
            0
        },
        modified: if kind.is_file() {
            attrs.mtime.map(Modified::Remote)
        } else {
            None
        },
    })
}
fn push_entry(entries: &mut Vec<Entry>, bytes: &mut u64, entry: Entry) -> Result<()> {
    if entries.len() >= MAX_ENTRIES {
        return Err(SessionError::EntryLimit(MAX_ENTRIES));
    }
    if entry
        .relative
        .split('/')
        .filter(|name| !name.is_empty())
        .count()
        > MAX_DEPTH
    {
        return Err(SessionError::Invalid("directory exceeds 32 child levels"));
    }
    *bytes = bytes
        .checked_add(entry.size)
        .filter(|bytes| *bytes <= MAX_BYTES)
        .ok_or(SessionError::Invalid("directory exceeds 16 GiB"))?;
    entries.push(entry);
    Ok(())
}

async fn scan_local(root: &Path) -> Result<(Vec<Entry>, u64)> {
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut pending = vec![String::new()];
    while let Some(relative) = pending.pop() {
        let path = local_child(root, &relative);
        validate_lengths(&path, &relative)?;
        local_directory_chain(
            path.parent()
                .ok_or(SessionError::Invalid("local source has no parent"))?,
        )
        .await?;
        let metadata = local_io(tokio::fs::symlink_metadata(&path)).await?;
        let entry = local_entry(&metadata, &relative)?;
        let directory = entry.directory;
        push_entry(&mut entries, &mut bytes, entry)?;
        if directory {
            let mut directory = tokio::fs::read_dir(path).await?;
            let mut names = HashSet::new();
            while let Some(child) = directory.next_entry().await? {
                let name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| SessionError::Invalid("local filename is not UTF-8"))?;
                validate_name(&name)?;
                if !names.insert(name.to_lowercase()) {
                    return Err(SessionError::Invalid("directory has case-colliding names"));
                }
                if pending.len() + entries.len() >= MAX_ENTRIES {
                    return Err(SessionError::EntryLimit(MAX_ENTRIES));
                }
                pending.push(if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                });
            }
        }
    }
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok((entries, bytes))
}

async fn scan_remote(raw: &RawSftpSession, root: &str) -> Result<(Vec<Entry>, u64)> {
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut pending = vec![String::new()];
    while let Some(relative) = pending.pop() {
        let path = remote_child(root, &relative);
        validate_lengths(Path::new(&relative), &path)?;
        remote_directory_chain(raw, remote_parent(&path)?).await?;
        let attrs = remote_io(raw.lstat(&path)).await.map_err(sftp_error)?.attrs;
        let entry = remote_entry(&attrs, &relative)?;
        let directory = entry.directory;
        push_entry(&mut entries, &mut bytes, entry)?;
        if directory {
            let handle = remote_io(raw.opendir(&path))
                .await
                .map_err(sftp_error)?
                .handle;
            let mut names = HashSet::new();
            let mut empty_packets = 0;
            loop {
                match remote_io(raw.readdir(&handle)).await {
                    Ok(packet) => {
                        if packet
                            .files
                            .iter()
                            .all(|file| matches!(file.filename.as_str(), "." | ".."))
                        {
                            empty_packets += 1;
                            if empty_packets > 16 {
                                return Err(SessionError::Invalid(
                                    "remote listing made no progress",
                                ));
                            }
                        } else {
                            empty_packets = 0;
                        }
                        for child in packet.files {
                            let name = child.filename;
                            if matches!(name.as_str(), "." | "..") {
                                continue;
                            }
                            validate_name(&name)?;
                            if !names.insert(name.to_lowercase()) {
                                return Err(SessionError::Invalid(
                                    "remote directory has duplicate or case-colliding names",
                                ));
                            }
                            if pending.len() + entries.len() >= MAX_ENTRIES {
                                return Err(SessionError::EntryLimit(MAX_ENTRIES));
                            }
                            pending.push(if relative.is_empty() {
                                name
                            } else {
                                format!("{relative}/{name}")
                            });
                        }
                    }
                    Err(russh_sftp::client::error::Error::Status(status))
                        if status.status_code == StatusCode::Eof =>
                    {
                        break;
                    }
                    Err(error) => return Err(sftp_error(error)),
                }
            }
            remote_io(raw.close(handle)).await.map_err(sftp_error)?;
        }
    }
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok((entries, bytes))
}
async fn verify_local_entry(path: &Path, expected: &Entry) -> Result<()> {
    local_directory_chain(
        path.parent()
            .ok_or(SessionError::Invalid("local source has no parent"))?,
    )
    .await?;
    let metadata = local_io(tokio::fs::symlink_metadata(path)).await?;
    if local_entry(&metadata, &expected.relative)? != *expected {
        return Err(SessionError::Invalid("local source changed after review"));
    }
    Ok(())
}
async fn verify_remote_entry(raw: &RawSftpSession, path: &str, expected: &Entry) -> Result<()> {
    remote_directory_chain(raw, remote_parent(path)?).await?;
    let attrs = remote_io(raw.lstat(path)).await.map_err(sftp_error)?.attrs;
    if remote_entry(&attrs, &expected.relative)? != *expected {
        return Err(SessionError::Invalid("remote source changed after review"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_execution_futures_do_not_embed_transfer_buffers() {
        // Infer the future without constructing or polling a transport. Large
        // inline buffers multiply debug stack use through async wrappers.
        fn file_size<F>(
            _: impl FnOnce(
                &'static SftpSession,
                &'static RawSftpSession,
                &'static Entry,
                &'static TransferContext,
            ) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        fn directory_size<F>(
            _: impl FnOnce(&'static SftpSession, DirectoryTransferPlan, &'static TransferContext) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        let upload = file_size(|session, raw, entry, context| {
            session.upload_tree_file(raw, Path::new("source"), "/target", entry, context)
        });
        let download = file_size(|session, raw, entry, context| {
            session.download_tree_file(raw, "/source", Path::new("target"), entry, context)
        });
        let directory =
            directory_size(|session, plan, context| session.queued_directory(plan, context));
        eprintln!(
            "directory future sizes: upload={upload}, download={download}, execution={directory}"
        );
        // Leave room for platform-specific I/O state, but not an inline chunk.
        for (name, bytes, limit) in [
            ("upload", upload, 16 * 1024),
            ("download", download, 16 * 1024),
            ("execution", directory, 32 * 1024),
        ] {
            assert!(bytes < limit, "{name} future uses {bytes} bytes");
        }
    }

    #[test]
    fn portable_names_reject_traversal_aliases_and_windows_devices() {
        for name in [
            "",
            ".",
            "..",
            "../outside",
            "a/b",
            "a\\b",
            "a:stream",
            "a.",
            "a ",
            "CON",
            "nul.txt",
            "NUL .txt",
            "CONIN$",
            "COM1.log",
            "LPT9",
            "COM¹",
            "bad\nname",
        ] {
            assert!(validate_name(name).is_err(), "accepted {name:?}");
        }
        for name in [".config", "空目录", "com10.txt", "readme.md"] {
            assert!(validate_name(name).is_ok());
        }
    }
    #[test]
    fn roots_reject_relative_remote_traversal_and_root_copies() {
        for path in ["/", "relative", "/a/../b", "/a//b", "/a/", "/a/./b"] {
            assert!(validate_remote_root(path).is_err());
        }
        assert!(validate_remote_root("/home/user/data").is_ok());
    }
    #[test]
    fn entry_budget_checks_size_depth_and_count() {
        let file = Entry {
            relative: "file".into(),
            directory: false,
            size: MAX_BYTES + 1,
            modified: None,
        };
        assert!(push_entry(&mut Vec::new(), &mut 0, file.clone()).is_err());
        assert!(
            push_entry(
                &mut Vec::new(),
                &mut 0,
                Entry {
                    relative: vec!["a"; MAX_DEPTH + 1].join("/"),
                    size: 0,
                    ..file.clone()
                }
            )
            .is_err()
        );
        assert!(
            push_entry(
                &mut vec![file.clone(); MAX_ENTRIES],
                &mut 0,
                Entry { size: 0, ..file }
            )
            .is_err()
        );
    }
}
