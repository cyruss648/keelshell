//! Checked, bounded reads for content review without shell commands.
use super::*;

impl SftpSession {
    /// Inspect an absolute path after checking its existing parent chain.
    ///
    /// Returns `None` only for a missing leaf. Parents must be actual directories
    /// with explicit type bits; links and ambiguous metadata fail closed.
    /// SFTP v3 checks are observations, not an atomic no-follow guarantee against
    /// a concurrent server-side rename.
    pub async fn inspect_entry(&self, path: &str) -> Result<Option<RemoteEntry>> {
        inspection_path(path)?;
        deadline(self.timeout, "SFTP checked inspection", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            directory::remote_directory_chain(
                &raw.0,
                if path == "/" {
                    "/"
                } else {
                    directory::remote_parent(path)?
                },
            )
            .await?;
            match raw.0.lstat(path).await {
                Ok(packet) => Ok(Some(entry(path, packet.attrs))),
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::NoSuchFile =>
                {
                    Ok(None)
                }
                Err(error) => Err(sftp_error(error)),
            }
        })
        .await
    }

    /// Read a confirmed regular file under a strict byte bound.
    ///
    /// Parent and leaf type checks precede opening; the exact opened handle is
    /// checked before and after reading, and the named leaf is checked again.
    /// A size or metadata change, missing type, link, or oversized file is an
    /// error. The result is never a truncated success. SFTP v3 cannot exclude
    /// every concurrent writer; this supplies a review observation, not a lock.
    pub async fn read_regular(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>> {
        inspection_path(path)?;
        deadline(self.timeout, "SFTP checked regular-file read", async {
            let raw = DirectoryChannel(self._connection.sftp_raw().await?);
            directory::remote_directory_chain(&raw.0, directory::remote_parent(path)?).await?;
            let before = raw.0.lstat(path).await.map_err(sftp_error)?.attrs;
            regular(&before, max_bytes)?;
            let handle = raw
                .0
                .open(path, OpenFlags::READ, FileAttributes::empty())
                .await
                .map_err(sftp_error)?
                .handle;
            let result = async {
                let opened = raw.0.fstat(&handle).await.map_err(sftp_error)?.attrs;
                same_file(&before, &opened)?;
                let mut bytes = Vec::new();
                loop {
                    let remaining = max_bytes.saturating_sub(bytes.len()).saturating_add(1);
                    let length = remaining.min(32 * 1024) as u32;
                    match raw.0.read(&handle, bytes.len() as u64, length).await {
                        Ok(packet)
                            if !packet.data.is_empty() && packet.data.len() <= length as usize =>
                        {
                            bytes.extend_from_slice(&packet.data);
                            if bytes.len() > max_bytes {
                                return Err(SessionError::OutputLimit(max_bytes));
                            }
                        }
                        Ok(_) => {
                            return Err(SessionError::Invalid("invalid checked SFTP read length"));
                        }
                        Err(russh_sftp::client::error::Error::Status(status))
                            if status.status_code == StatusCode::Eof =>
                        {
                            break;
                        }
                        Err(error) => return Err(sftp_error(error)),
                    }
                }
                same_file(
                    &before,
                    &raw.0.fstat(&handle).await.map_err(sftp_error)?.attrs,
                )?;
                same_file(&before, &raw.0.lstat(path).await.map_err(sftp_error)?.attrs)?;
                if before.size != Some(bytes.len() as u64) {
                    return Err(SessionError::Invalid("checked SFTP file size changed"));
                }
                Ok(bytes)
            }
            .await;
            let closed = raw.0.close(&handle).await.map_err(sftp_error);
            let bytes = result?;
            closed?;
            Ok(bytes)
        })
        .await
    }
}

fn inspection_path(path: &str) -> Result<()> {
    valid_path(path)?;
    if path != "/"
        && (!path.starts_with('/')
            || path.len() > 4096
            || path.contains('\\')
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || matches!(part, "." | "..")))
    {
        return Err(SessionError::Invalid(
            "checked SFTP path must be normalized and absolute",
        ));
    }
    Ok(())
}

fn regular(attrs: &FileAttributes, max_bytes: usize) -> Result<()> {
    if !attrs
        .permissions
        .is_some_and(|mode| mode & 0o170000 == 0o100000)
    {
        return Err(SessionError::Invalid(
            "checked SFTP read requires an explicit regular file",
        ));
    }
    let size = attrs
        .size
        .ok_or(SessionError::Invalid("checked SFTP file has no size"))?;
    if size > max_bytes as u64 {
        return Err(SessionError::OutputLimit(max_bytes));
    }
    Ok(())
}

fn same_file(before: &FileAttributes, after: &FileAttributes) -> Result<()> {
    if before.size != after.size
        || before.permissions != after.permissions
        || before.mtime != after.mtime
    {
        return Err(SessionError::Invalid(
            "checked SFTP file changed while reading",
        ));
    }
    Ok(())
}

fn entry(path: &str, attrs: FileAttributes) -> RemoteEntry {
    RemoteEntry {
        name: path.rsplit('/').next().unwrap_or(path).to_owned(),
        path: path.to_owned(),
        size: attrs.size,
        is_directory: attrs.file_type().is_dir(),
        is_symlink: attrs.file_type().is_symlink(),
        permissions: attrs.permissions,
        modified: attrs.mtime,
    }
}

#[cfg(test)]
mod tests {
    use super::{FileAttributes, SessionError, inspection_path, regular, same_file};

    #[test]
    fn checked_paths_require_normalized_absolute_components() {
        for path in ["relative", "/a/../b", "/a//b", "/a\\b", "/a\nb", "/a/"] {
            assert!(inspection_path(path).is_err(), "{path:?}");
        }
        assert!(inspection_path("/").is_ok());
        assert!(inspection_path("/中文/file").is_ok());
    }

    #[test]
    fn checked_regular_files_require_type_size_and_unchanged_metadata() {
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(4);
        assert!(regular(&attrs, 4).is_err());
        attrs.permissions = Some(0o120777);
        assert!(regular(&attrs, 4).is_err());
        attrs.permissions = Some(0o100600);
        assert!(regular(&attrs, 4).is_ok());
        assert!(matches!(
            regular(&attrs, 3),
            Err(SessionError::OutputLimit(3))
        ));
        let mut changed = attrs.clone();
        changed.size = Some(5);
        assert!(same_file(&attrs, &changed).is_err());
    }
}
