//! Bounded descriptor I/O and full-content verification for reviewed files.
use super::*;
use sha2::{Digest, Sha256};
use std::io::SeekFrom;
use tokio::io::AsyncSeekExt;

pub(super) fn ensure_snapshot(actual: &Snapshot, expected: &Snapshot) -> Result<()> {
    if actual != expected {
        Err(SessionError::Invalid("resume file changed after review"))
    } else {
        Ok(())
    }
}
pub(super) async fn inspect_paths(
    raw: &RawSftpSession,
    spec: &TransferSpec,
    allow_missing: bool,
) -> Result<(Snapshot, Option<Snapshot>)> {
    let missing_local = allow_missing && spec.direction == TransferDirection::Download;
    let missing_remote = allow_missing && spec.direction == TransferDirection::Upload;
    local_parent_chain(&spec.local, missing_local).await?;
    remote_parent_chain(raw, &spec.remote, missing_remote).await?;
    let local = match tokio::fs::symlink_metadata(&spec.local).await {
        Ok(metadata) => Some(local_snapshot(&metadata)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && missing_local => None,
        Err(error) => return Err(error.into()),
    };
    let remote = match raw.lstat(&spec.remote).await {
        Ok(attrs) => Some(remote_snapshot(attrs.attrs)?),
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == StatusCode::NoSuchFile && missing_remote =>
        {
            None
        }
        Err(error) => return Err(sftp_error(error)),
    };
    let (source, target) = match spec.direction {
        TransferDirection::Upload => (local, remote),
        TransferDirection::Download => (remote, local),
    };
    let source = source.ok_or(SessionError::Invalid("resume source is missing"))?;
    if source.size > MAX_BYTES || target.as_ref().is_some_and(|t| t.size > source.size) {
        return Err(SessionError::Invalid(
            "resume file size exceeds its source or transfer limit",
        ));
    }
    Ok((source, target))
}
async fn local_parent_chain(path: &Path, allow_missing: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(SessionError::Invalid("resume target has no parent"))?;
    if !allow_missing {
        return directory::local_directory_chain(parent).await;
    }
    // Walk complete paths from the root. A Windows canonical path starts with
    // a verbatim drive prefix whose standalone component is not a directory;
    // querying that prefix before its root separator addresses a device path.
    // Root-first checks also reject links before inspecting their descendants.
    for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        match tokio::fs::symlink_metadata(ancestor).await {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(SessionError::Invalid(
                    "resume ancestor is not a real directory",
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
async fn remote_parent_chain(raw: &RawSftpSession, path: &str, allow_missing: bool) -> Result<()> {
    let parent = directory::remote_parent(path)?;
    if !allow_missing {
        return directory::remote_directory_chain(raw, parent).await;
    }
    directory::remote_directory_chain(raw, "/").await?;
    let mut current = String::new();
    for part in parent.split('/').filter(|s| !s.is_empty()) {
        current.push('/');
        current.push_str(part);
        match raw.lstat(&current).await {
            Ok(attrs)
                if attrs
                    .attrs
                    .permissions
                    .is_some_and(|p| p & 0o170000 == 0o040000) => {}
            Ok(_) => {
                return Err(SessionError::Invalid(
                    "resume remote ancestor is not a real directory",
                ));
            }
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile => {}
            Err(e) => return Err(sftp_error(e)),
        }
    }
    Ok(())
}
pub(super) enum OpenFile<'a> {
    Local(tokio::fs::File),
    Remote {
        raw: &'a RawSftpSession,
        handle: String,
    },
}
impl OpenFile<'_> {
    pub(super) async fn snapshot(&self) -> Result<Snapshot> {
        match self {
            Self::Local(file) => local_snapshot(&file.metadata().await?),
            Self::Remote { raw, handle } => {
                remote_snapshot(raw.fstat(handle).await.map_err(sftp_error)?.attrs)
            }
        }
    }
    pub(super) async fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize> {
        match self {
            Self::Local(file) => {
                file.seek(SeekFrom::Start(offset)).await?;
                Ok(file.read(buffer).await?)
            }
            Self::Remote { raw, handle } => {
                match raw.read(handle.as_str(), offset, buffer.len() as u32).await {
                    Ok(data) if !data.data.is_empty() && data.data.len() <= buffer.len() => {
                        buffer[..data.data.len()].copy_from_slice(&data.data);
                        Ok(data.data.len())
                    }
                    Ok(_) => Err(SessionError::Invalid("invalid SFTP read length")),
                    Err(russh_sftp::client::error::Error::Status(status))
                        if status.status_code == StatusCode::Eof =>
                    {
                        Ok(0)
                    }
                    Err(e) => Err(sftp_error(e)),
                }
            }
        }
    }
    pub(super) async fn write_at(&mut self, offset: u64, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Local(file) => {
                file.seek(SeekFrom::Start(offset)).await?;
                file.write_all(bytes).await?;
                file.flush().await?;
                Ok(())
            }
            Self::Remote { raw, handle } => {
                raw.write(handle.as_str(), offset, bytes.to_vec())
                    .await
                    .map_err(sftp_error)?;
                Ok(())
            }
        }
    }
    pub(super) async fn close(self) -> Result<()> {
        if let Self::Remote { raw, handle } = self {
            raw.close(handle).await.map_err(sftp_error)?;
        }
        Ok(())
    }
}
pub(super) async fn open_source<'a>(
    raw: &'a RawSftpSession,
    spec: &TransferSpec,
) -> Result<OpenFile<'a>> {
    match spec.direction {
        TransferDirection::Upload => Ok(OpenFile::Local(tokio::fs::File::open(&spec.local).await?)),
        TransferDirection::Download => open_remote(raw, &spec.remote, OpenFlags::READ).await,
    }
}
pub(super) async fn open_target<'a>(
    raw: &'a RawSftpSession,
    spec: &TransferSpec,
    write: bool,
    missing: bool,
) -> Result<OpenFile<'a>> {
    match spec.direction {
        TransferDirection::Download => Ok(OpenFile::Local(
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(write)
                .create_new(missing)
                .open(&spec.local)
                .await?,
        )),
        TransferDirection::Upload => {
            let flags = OpenFlags::READ
                | if write {
                    OpenFlags::WRITE
                } else {
                    OpenFlags::empty()
                }
                | if missing {
                    OpenFlags::CREATE | OpenFlags::EXCLUDE
                } else {
                    OpenFlags::empty()
                };
            open_remote(raw, &spec.remote, flags).await
        }
    }
}
async fn open_remote<'a>(
    raw: &'a RawSftpSession,
    path: &str,
    flags: OpenFlags,
) -> Result<OpenFile<'a>> {
    let mut attrs = FileAttributes::empty();
    if flags.contains(OpenFlags::CREATE) {
        attrs.permissions = Some(0o600);
    }
    Ok(OpenFile::Remote {
        raw,
        handle: raw
            .open(path, flags, attrs)
            .await
            .map_err(sftp_error)?
            .handle,
    })
}
pub(super) async fn verify_content(
    source: &mut OpenFile<'_>,
    mut target: Option<&mut OpenFile<'_>>,
    size: u64,
    prefix: u64,
) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut source_buffer = vec![0; TRANSFER_CHUNK_SIZE];
    let mut target_buffer = vec![0; TRANSFER_CHUNK_SIZE];
    while offset < size {
        let wanted = (size - offset).min(TRANSFER_CHUNK_SIZE as u64) as usize;
        let count = source.read_at(offset, &mut source_buffer[..wanted]).await?;
        if count == 0 {
            return Err(SessionError::Invalid(
                "resume source ended before its advertised size",
            ));
        }
        if offset < prefix {
            let compare = (prefix - offset).min(count as u64) as usize;
            let mut read = 0;
            let target = target
                .as_mut()
                .ok_or(SessionError::Invalid("resume prefix target is missing"))?;
            while read < compare {
                let n = target
                    .read_at(offset + read as u64, &mut target_buffer[read..compare])
                    .await?;
                if n == 0 {
                    return Err(SessionError::Invalid(
                        "resume target ended before its advertised size",
                    ));
                }
                read += n;
            }
            if source_buffer[..compare] != target_buffer[..compare] {
                return Err(SessionError::Invalid(
                    "existing destination does not match the complete source prefix",
                ));
            }
        }
        hash.update(&source_buffer[..count]);
        offset += count as u64;
    }
    if source.read_at(size, &mut source_buffer[..1]).await? != 0 {
        return Err(SessionError::Invalid(
            "resume source grew during validation",
        ));
    }
    if let Some(target) = target
        && target.read_at(prefix, &mut target_buffer[..1]).await? != 0
    {
        return Err(SessionError::Invalid(
            "resume target grew during validation",
        ));
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn canonical_parent_chain_allows_missing_directories_without_creating_them() -> Result<()>
    {
        let temporary = tempfile::tempdir()?;
        // Windows canonicalize produces a verbatim absolute path. Exercise the
        // real filesystem with it; never strip its prefix to make a test pass.
        let root = tokio::fs::canonicalize(temporary.path()).await?;
        let existing = root.join("existing");
        tokio::fs::create_dir(&existing).await?;
        let destination = existing.join("missing").join("nested").join("file.bin");
        local_parent_chain(&destination, true).await?;
        assert!(!existing.join("missing").exists());
        assert!(local_parent_chain(&destination, false).await.is_err());

        // Single-file destinations and filesystem roots must likewise retain
        // their complete absolute spelling during parent validation.
        local_parent_chain(&existing.join("file.bin"), true).await?;
        local_parent_chain(&root.join("file.bin"), true).await?;
        assert!(!existing.join("file.bin").exists());
        Ok(())
    }

    #[tokio::test]
    async fn missing_parent_validation_rejects_an_existing_regular_file_ancestor() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = tokio::fs::canonicalize(temporary.path()).await?;
        let blocker = root.join("regular-file");
        tokio::fs::write(&blocker, b"preserve this file").await?;
        let destination = blocker.join("missing").join("file.bin");
        assert!(matches!(
            local_parent_chain(&destination, true).await,
            Err(SessionError::Invalid(_))
        ));
        assert_eq!(tokio::fs::read(&blocker).await?, b"preserve this file");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn missing_parent_validation_rejects_a_static_directory_symlink() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = tokio::fs::canonicalize(temporary.path()).await?;
        let actual = root.join("actual");
        tokio::fs::create_dir(&actual).await?;
        let link = root.join("link");
        std::os::unix::fs::symlink(&actual, &link)?;
        assert!(matches!(
            local_parent_chain(&link.join("missing").join("file.bin"), true).await,
            Err(SessionError::Invalid(_))
        ));
        assert!(!actual.join("missing").exists());
        Ok(())
    }
}
