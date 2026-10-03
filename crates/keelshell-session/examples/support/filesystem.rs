//! Small real-filesystem SFTP fixture, restricted to its temporary root.
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub struct Filesystem {
    root: Arc<PathBuf>,
    files: HashMap<String, fs::File>,
    directories: HashMap<String, Option<Vec<File>>>,
    io_delay: Duration,
}
impl Filesystem {
    pub fn new(root: Arc<PathBuf>) -> Self {
        Self {
            root,
            files: HashMap::new(),
            directories: HashMap::new(),
            io_delay: Duration::ZERO,
        }
    }
    /// Delay file requests so native UI acceptance can observe in-flight work.
    pub fn with_io_delay(mut self, delay: Duration) -> Self {
        self.io_delay = delay;
        self
    }
    fn path(&self, value: &str) -> Result<PathBuf, StatusCode> {
        if value.contains('\0') {
            return Err(StatusCode::BadMessage);
        }
        let mut path = self.root.as_ref().clone();
        for component in Path::new(value).components() {
            match component {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => {
                    path.push(name);
                    if fs::symlink_metadata(&path)
                        .is_ok_and(|metadata| metadata.file_type().is_symlink())
                    {
                        return Err(StatusCode::PermissionDenied);
                    }
                }
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(StatusCode::PermissionDenied);
                }
            }
        }
        Ok(path)
    }
}
fn io(error: std::io::Error) -> StatusCode {
    match error.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}
fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: "en".into(),
    }
}
fn metadata(path: &Path) -> Result<FileAttributes, StatusCode> {
    Ok(FileAttributes::from(&fs::metadata(path).map_err(io)?))
}

impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        let mut version = Version::new();
        if cfg!(unix) {
            version
                .extensions
                .insert("posix-rename@openssh.com".into(), "1".into());
        }
        Ok(version)
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        if self.files.len() >= 32 {
            return Err(StatusCode::Failure);
        }
        let path = self.path(&filename)?;
        let mut options = fs::OpenOptions::new();
        options
            .read(flags.contains(OpenFlags::READ))
            .write(flags.contains(OpenFlags::WRITE))
            .append(flags.contains(OpenFlags::APPEND))
            .create(flags.contains(OpenFlags::CREATE))
            .create_new(flags.contains(OpenFlags::EXCLUDE))
            .truncate(flags.contains(OpenFlags::TRUNCATE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(attrs.permissions.unwrap_or(0o600) & 0o777);
        }
        #[cfg(not(unix))]
        let _ = attrs;
        let file = options.open(path).map_err(io)?;
        let handle = uuid::Uuid::new_v4().to_string();
        self.files.insert(handle.clone(), file);
        Ok(Handle { id, handle })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        if self.files.remove(&handle).is_none() && self.directories.remove(&handle).is_none() {
            return Err(StatusCode::Failure);
        }
        Ok(ok(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, StatusCode> {
        if !self.io_delay.is_zero() {
            tokio::time::sleep(self.io_delay).await;
        }
        let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        let mut data = vec![0; length.min(64 * 1024) as usize];
        let count = file.read(&mut data).map_err(io)?;
        if count == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(count);
        Ok(Data { id, data })
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        if offset.saturating_add(data.len() as u64) > 16 * 1024 * 1024 {
            return Err(StatusCode::Failure);
        }
        if !self.io_delay.is_zero() {
            tokio::time::sleep(self.io_delay).await;
        }
        let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        file.write_all(&data).map_err(io)?;
        Ok(ok(id))
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        Ok(Attrs {
            id,
            attrs: metadata(&self.path(&path)?)?,
        })
    }
    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.lstat(id, path).await
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, StatusCode> {
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(
                &self
                    .files
                    .get(&handle)
                    .ok_or(StatusCode::Failure)?
                    .metadata()
                    .map_err(io)?,
            ),
        })
    }
    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusCode> {
        if self.directories.len() >= 32 {
            return Err(StatusCode::Failure);
        }
        let files = fs::read_dir(self.path(&path)?)
            .map_err(io)?
            .take(1000)
            .map(|entry| {
                let entry = entry.map_err(io)?;
                if entry.file_type().map_err(io)?.is_symlink() {
                    return Err(StatusCode::PermissionDenied);
                }
                Ok(File::new(
                    entry.file_name().to_string_lossy(),
                    FileAttributes::from(&entry.metadata().map_err(io)?),
                ))
            })
            .collect::<Result<Vec<_>, StatusCode>>()?;
        let handle = uuid::Uuid::new_v4().to_string();
        self.directories.insert(handle.clone(), Some(files));
        Ok(Handle { id, handle })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusCode> {
        let files = self
            .directories
            .get_mut(&handle)
            .ok_or(StatusCode::Failure)?
            .take()
            .ok_or(StatusCode::Eof)?;
        if files.is_empty() {
            return Err(StatusCode::Eof);
        }
        Ok(Name { id, files })
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        let resolved = self.path(&path)?;
        let relative = resolved
            .strip_prefix(self.root.as_ref())
            .map_err(|_| StatusCode::PermissionDenied)?;
        Ok(Name {
            id,
            files: vec![File::new(
                format!("/{}", relative.to_string_lossy().replace('\\', "/")),
                metadata(&resolved)?,
            )],
        })
    }
    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _: FileAttributes,
    ) -> Result<Status, StatusCode> {
        fs::create_dir(self.path(&path)?).map_err(io)?;
        Ok(ok(id))
    }
    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        fs::remove_dir(self.path(&path)?).map_err(io)?;
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        fs::remove_file(self.path(&path)?).map_err(io)?;
        Ok(ok(id))
    }
    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, StatusCode> {
        fs::rename(self.path(&old)?, self.path(&new)?).map_err(io)?;
        Ok(ok(id))
    }
    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, StatusCode> {
        if request != "posix-rename@openssh.com" || !cfg!(unix) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh_sftp::server::Handler;

    #[test]
    fn parent_components_cannot_escape_temporary_root() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let filesystem = Filesystem::new(Arc::new(root.path().to_path_buf()));
        assert!(filesystem.path("/../outside").is_err());
        assert!(filesystem.path("uploads/../../outside").is_err());
        assert_eq!(
            filesystem
                .path("/welcome.txt")
                .map_err(|e| format!("{e:?}"))?,
            root.path().join("welcome.txt")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlinks_cannot_escape_temporary_root() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), root.path().join("link"))?;
        let filesystem = Filesystem::new(Arc::new(root.path().to_path_buf()));
        assert!(filesystem.path("/link/outside.txt").is_err());
        Ok(())
    }

    #[tokio::test]
    async fn exclusive_staging_replaces_real_file_after_close()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("target.txt"), b"original")?;
        let mut filesystem = Filesystem::new(Arc::new(root.path().to_path_buf()));
        let opened = filesystem
            .open(
                1,
                "/staging.tmp".into(),
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
                FileAttributes::empty(),
            )
            .await
            .map_err(|e| format!("{e:?}"))?;
        filesystem
            .write(
                2,
                opened.handle.clone(),
                0,
                b"complete replacement".to_vec(),
            )
            .await
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(fs::read(root.path().join("target.txt"))?, b"original");
        assert!(
            filesystem
                .open(
                    3,
                    "/staging.tmp".into(),
                    OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
                    FileAttributes::empty()
                )
                .await
                .is_err()
        );
        filesystem
            .close(4, opened.handle)
            .await
            .map_err(|e| format!("{e:?}"))?;
        filesystem
            .rename(5, "/staging.tmp".into(), "/target.txt".into())
            .await
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            fs::read(root.path().join("target.txt"))?,
            b"complete replacement"
        );
        Ok(())
    }
}
