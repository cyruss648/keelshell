//! Bounded local observations and descriptor-relative no-replace publication.
//!
//! Private staging is retained on an unverified result. The caller owns the
//! shared mutation reservation and exposes retained paths through its registry.

use std::collections::BTreeSet;
use std::fs::File;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs::Metadata;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use keelshell_core::zmodem::MAX_FILE_BYTES;
use keelshell_core::zmodem::{MAX_FILES, MAX_TOTAL_BYTES, validate_basename};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use sha2::{Digest, Sha256};

use super::{ZmodemFailure, ZmodemFileReview};

type Result<T> = std::result::Result<T, ZmodemFailure>;

/// Complete observed content stays private to the controller until confirmation.
pub(super) struct UploadPlan {
    sources: Vec<Source>,
}

struct Source {
    view: ZmodemFileReview,
    bytes: Vec<u8>,
    stamp: FileStamp,
    parent: Parents,
    // Retention prevents inode reuse from turning a new file into an old review.
    _descriptor: File,
}

/// The reviewed local namespace and unpredictable, as-yet-uncreated staging.
#[derive(Clone)]
pub(super) struct Destination {
    pub(super) path: PathBuf,
    pub(super) staging: PathBuf,
    parent: Parents,
}

#[derive(Clone)]
struct Parents {
    path: PathBuf,
    chain: Vec<DirectoryStamp>,
    _descriptor: Arc<File>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectoryStamp {
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    owner: (u32, u32),
    #[cfg(unix)]
    mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    bytes: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    owner: (u32, u32),
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    changed: (i64, i64),
}

/// Successful full readback, not merely an acknowledged rename.
pub(super) struct Saved {
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn io(_: std::io::Error) -> ZmodemFailure {
    ZmodemFailure::LocalIo
}

fn cancelled(stop: &AtomicBool) -> Result<()> {
    if stop.load(Ordering::Acquire) {
        Err(ZmodemFailure::Stale)
    } else {
        Ok(())
    }
}

fn alias(name: &str) -> String {
    keelshell_platform::filename_alias(name)
}

fn path(path: &Path) -> Result<&str> {
    let text = path.to_str().ok_or(ZmodemFailure::InvalidPath)?;
    if !path.is_absolute()
        || text.len() > 4096
        || text.chars().any(|character| {
            character.is_control()
                || matches!(character as u32, 0x200b..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069)
        })
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        || path.file_name().is_none()
    {
        return Err(ZmodemFailure::InvalidPath);
    }
    Ok(text)
}

fn basename(path: &Path) -> Result<&str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ZmodemFailure::InvalidPath)?;
    validate_basename(name).map_err(|_| ZmodemFailure::InvalidPath)?;
    Ok(name)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl DirectoryStamp {
    fn inspect(metadata: &Metadata) -> Result<Self> {
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ZmodemFailure::InvalidPath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                identity: (metadata.dev(), metadata.ino()),
                owner: (metadata.uid(), metadata.gid()),
                // Write activity changes directory timestamps, not its identity.
                mode: metadata.mode(),
            })
        }
        #[cfg(not(unix))]
        {
            // Stable std does not expose Windows hard-link identity/count.
            // Never substitute size/timestamps for a reviewed file identity.
            Err(ZmodemFailure::Unsupported)
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl FileStamp {
    fn inspect(metadata: &Metadata) -> Result<Self> {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ZmodemFailure::InvalidPath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(ZmodemFailure::InvalidPath);
            }
            Ok(Self {
                bytes: metadata.len(),
                modified: metadata.modified().ok(),
                created: metadata.created().ok(),
                identity: (metadata.dev(), metadata.ino()),
                owner: (metadata.uid(), metadata.gid()),
                mode: metadata.mode(),
                changed: (metadata.ctime(), metadata.ctime_nsec()),
            })
        }
        #[cfg(not(unix))]
        {
            Err(ZmodemFailure::Unsupported)
        }
    }
}

/// Open each ancestor separately. A path string checked with `lstat` followed by
/// `open` is insufficient: a substituted intermediate link could escape it.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn parents(path: &Path) -> Result<(Parents, File)> {
    use rustix::fs::{Mode, OFlags, open, openat};

    let parent = path.parent().ok_or(ZmodemFailure::InvalidPath)?;
    let mut descriptor: File = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(io)?
    .into();
    let mut chain = vec![DirectoryStamp::inspect(
        &descriptor.metadata().map_err(io)?,
    )?];
    for component in parent.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                descriptor = openat(
                    &descriptor,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(std::io::Error::from)
                .map_err(|_| ZmodemFailure::InvalidPath)?
                .into();
                chain.push(DirectoryStamp::inspect(
                    &descriptor.metadata().map_err(io)?,
                )?);
            }
            _ => return Err(ZmodemFailure::InvalidPath),
        }
    }
    let retained = Arc::new(descriptor.try_clone().map_err(io)?);
    Ok((
        Parents {
            path: parent.to_owned(),
            chain,
            _descriptor: retained,
        },
        descriptor,
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn parents(_: &Path) -> Result<(Parents, File)> {
    Err(ZmodemFailure::Unsupported)
}

fn revalidate_parents(path: &Path, expected: &Parents) -> Result<File> {
    let (current, descriptor) = parents(path)?;
    if current.path != expected.path || current.chain != expected.chain {
        return Err(ZmodemFailure::SourceChanged);
    }
    Ok(descriptor)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_source(path: &Path, bound: u64, stop: &AtomicBool) -> Result<(Source, File)> {
    use rustix::fs::{Mode, OFlags, openat};

    cancelled(stop)?;
    self::path(path)?;
    let name = basename(path)?.to_owned();
    let (parent, descriptor) = parents(path)?;
    let mut file: File = openat(
        &descriptor,
        path.file_name().ok_or(ZmodemFailure::InvalidPath)?,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(|_| ZmodemFailure::InvalidPath)?
    .into();
    let stamp = FileStamp::inspect(&file.metadata().map_err(io)?)?;
    if stamp.bytes > MAX_FILE_BYTES || stamp.bytes > bound {
        return Err(ZmodemFailure::Limit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(stamp.bytes as usize)
        .map_err(|_| ZmodemFailure::Limit)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancelled(stop)?;
        let count = file.read(&mut buffer).map_err(io)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > stamp.bytes {
            return Err(ZmodemFailure::SourceChanged);
        }
        digest.update(&buffer[..count]);
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != stamp.bytes
        || FileStamp::inspect(&file.metadata().map_err(io)?)? != stamp
    {
        return Err(ZmodemFailure::SourceChanged);
    }
    let descriptor = revalidate_parents(path, &parent)?;
    let named: File = openat(
        &descriptor,
        path.file_name().ok_or(ZmodemFailure::InvalidPath)?,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(|_| ZmodemFailure::SourceChanged)?
    .into();
    if FileStamp::inspect(&named.metadata().map_err(io)?)? != stamp {
        return Err(ZmodemFailure::SourceChanged);
    }
    cancelled(stop)?;
    Ok((
        Source {
            view: ZmodemFileReview {
                path: path.to_owned(),
                name,
                size: stamp.bytes,
                sha256: Some(
                    digest
                        .finalize()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                ),
            },
            bytes,
            stamp,
            parent,
            _descriptor: file,
        },
        named,
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_source(_: &Path, _: u64, _: &AtomicBool) -> Result<(Source, File)> {
    Err(ZmodemFailure::Unsupported)
}

impl UploadPlan {
    pub(super) fn prepare(paths: Vec<PathBuf>, stop: &AtomicBool) -> Result<Self> {
        if paths.is_empty() || paths.len() > MAX_FILES {
            return Err(ZmodemFailure::Limit);
        }
        let mut sources = Vec::with_capacity(paths.len());
        let mut names = BTreeSet::new();
        #[cfg(unix)]
        let mut identities = BTreeSet::new();
        let mut total = 0u64;
        for path in paths {
            let (source, _) = read_source(&path, MAX_TOTAL_BYTES.saturating_sub(total), stop)?;
            if !names.insert(alias(&source.view.name)) {
                return Err(ZmodemFailure::InvalidPath);
            }
            #[cfg(unix)]
            if !identities.insert(source.stamp.identity) {
                return Err(ZmodemFailure::InvalidPath);
            }
            total = total
                .checked_add(source.stamp.bytes)
                .filter(|bytes| *bytes <= MAX_TOTAL_BYTES)
                .ok_or(ZmodemFailure::Limit)?;
            sources.push(source);
        }
        Ok(Self { sources })
    }

    pub(super) fn review(&self) -> Vec<ZmodemFileReview> {
        self.sources
            .iter()
            .map(|source| source.view.clone())
            .collect()
    }

    pub(super) fn confirm(
        self,
        stop: &AtomicBool,
    ) -> Result<Vec<keelshell_core::zmodem::SendFile>> {
        for source in &self.sources {
            let (current, _) =
                read_source(&source.view.path, source.stamp.bytes, stop).map_err(|reason| {
                    match reason {
                        ZmodemFailure::Stale => reason,
                        _ => ZmodemFailure::SourceChanged,
                    }
                })?;
            if current.stamp != source.stamp
                || current.parent.chain != source.parent.chain
                || current.view != source.view
                || current.bytes != source.bytes
            {
                return Err(ZmodemFailure::SourceChanged);
            }
        }
        // Bytes enter the pure sender only after every source was revalidated.
        self.sources
            .into_iter()
            .map(|source| {
                keelshell_core::zmodem::SendFile::new(source.view.name, source.bytes)
                    .map_err(|_| ZmodemFailure::InvalidPath)
            })
            .collect()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn absent(parent: &File, path: &Path) -> Result<()> {
    use rustix::fs::{AtFlags, statat};
    match statat(
        parent,
        path.file_name().ok_or(ZmodemFailure::InvalidPath)?,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(ZmodemFailure::DestinationExists),
        Err(_) => Err(ZmodemFailure::LocalIo),
    }?;
    // A case-sensitive filesystem must not admit an alias that a portable or
    // case-insensitive peer/user would consider the same local destination.
    let requested = alias(basename(path)?);
    let directory = rustix::fs::Dir::read_from(parent)
        .map_err(std::io::Error::from)
        .map_err(io)?;
    for (count, entry) in directory.enumerate() {
        if count >= 16_384 {
            return Err(ZmodemFailure::Limit);
        }
        let entry = entry.map_err(std::io::Error::from).map_err(io)?;
        if let Ok(name) = entry.file_name().to_str()
            && alias(name) == requested
        {
            return Err(ZmodemFailure::DestinationExists);
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn absent(_: &File, _: &Path) -> Result<()> {
    Err(ZmodemFailure::Unsupported)
}

impl Destination {
    pub(super) fn prepare(path: PathBuf, stop: &AtomicBool) -> Result<Self> {
        cancelled(stop)?;
        self::path(&path)?;
        basename(&path)?;
        let (parent, descriptor) = parents(&path)?;
        absent(&descriptor, &path)?;
        let staging = parent
            .path
            .join(format!(".keelshell-zmodem-{}.part", uuid::Uuid::new_v4()));
        self::path(&staging)?;
        absent(&descriptor, &staging)?;
        cancelled(stop)?;
        Ok(Self {
            path,
            staging,
            parent,
        })
    }

    pub(super) fn revalidate(&self, stop: &AtomicBool) -> Result<File> {
        cancelled(stop)?;
        let parent = revalidate_parents(&self.path, &self.parent)?;
        absent(&parent, &self.path)?;
        absent(&parent, &self.staging)?;
        Ok(parent)
    }

    /// Returns only after descriptor identity, private access and complete bytes
    /// were verified at the exact no-replace destination. On any post-create
    /// error `mutated` remains set: the owner retains both paths in quarantine.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn publish(
        self,
        bytes: Vec<u8>,
        stop: &AtomicBool,
        mutated: &AtomicBool,
    ) -> Result<Saved> {
        use rustix::fs::{Mode, OFlags, RenameFlags, fchmod, openat, renameat_with};

        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(ZmodemFailure::Limit);
        }
        let parent = self.revalidate(stop)?;
        let stage_name = self.staging.file_name().ok_or(ZmodemFailure::InvalidPath)?;
        let destination_name = self.path.file_name().ok_or(ZmodemFailure::InvalidPath)?;
        cancelled(stop)?;
        // Mark before exclusive CREATE; a panic or abandoned worker cannot
        // release the destination merely because no rename result was observed.
        mutated.store(true, Ordering::Release);
        let mut staging: File = openat(
            &parent,
            stage_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)
        .map_err(io)?
        .into();
        // chmod is not an ACL check. Clear and verify access via the platform's
        // fd-bound helper before any received content reaches the filesystem.
        fchmod(&staging, Mode::RUSR | Mode::WUSR)
            .map_err(std::io::Error::from)
            .map_err(io)?;
        private_acl(&staging, true)?;
        let before = FileStamp::inspect(&staging.metadata().map_err(io)?)?;
        #[cfg(unix)]
        if before.mode & 0o7777 != 0o600 || before.bytes != 0 {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        for chunk in bytes.chunks(64 * 1024) {
            cancelled(stop)?;
            staging.write_all(chunk).map_err(io)?;
        }
        staging.sync_all().map_err(io)?;
        private_acl(&staging, false)?;
        let stage = FileStamp::inspect(&staging.metadata().map_err(io)?)?;
        if stage.bytes != bytes.len() as u64 {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        verify_content(&mut staging, &bytes, stop)?;
        // Reopen the exact ancestor chain immediately before publication, while
        // the held directory descriptor prevents a rename from redirecting it.
        let current_parent = revalidate_parents(&self.path, &self.parent)?;
        absent(&current_parent, &self.path)?;
        let named: File = openat(
            &parent,
            stage_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .map_err(io)?
        .into();
        if FileStamp::inspect(&named.metadata().map_err(io)?)? != stage {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        private_acl(&named, false)?;
        cancelled(stop)?;
        renameat_with(
            &parent,
            stage_name,
            &parent,
            destination_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ZmodemFailure::DestinationExists
            } else {
                ZmodemFailure::UnverifiedPublication
            }
        })?;
        parent.sync_all().map_err(io)?;
        let published_parent = revalidate_parents(&self.path, &self.parent)?;
        let mut published: File = openat(
            &published_parent,
            destination_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .map_err(io)?
        .into();
        let actual = FileStamp::inspect(&published.metadata().map_err(io)?)?;
        // rename may change ctime. The held descriptor, identity and complete
        // bytes, rather than the pre-rename ctime, identify the result.
        if actual.identity != stage.identity
            || actual.owner != stage.owner
            || actual.mode & 0o7777 != 0o600
            || actual.bytes != bytes.len() as u64
        {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        private_acl(&published, false)?;
        verify_content(&mut published, &bytes, stop)?;
        let after = FileStamp::inspect(&published.metadata().map_err(io)?)?;
        if after != actual {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        revalidate_parents(&self.path, &self.parent)?;
        let named: File = openat(
            &published_parent,
            destination_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .map_err(io)?
        .into();
        if FileStamp::inspect(&named.metadata().map_err(io)?)? != actual
            || !matches!(absent(&published_parent, &self.staging), Ok(()))
        {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        private_acl(&named, false)?;
        cancelled(stop)?;
        Ok(Saved {
            path: self.path,
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(super) fn publish(self, _: Vec<u8>, _: &AtomicBool, _: &AtomicBool) -> Result<Saved> {
        Err(ZmodemFailure::Unsupported)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_content(file: &mut File, bytes: &[u8], stop: &AtomicBool) -> Result<()> {
    file.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut offset = 0usize;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancelled(stop)?;
        let count = file.read(&mut buffer).map_err(io)?;
        if count == 0 {
            break;
        }
        let end = offset
            .checked_add(count)
            .filter(|end| *end <= bytes.len())
            .ok_or(ZmodemFailure::UnverifiedPublication)?;
        if bytes[offset..end] != buffer[..count] {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        offset = end;
    }
    if offset != bytes.len() {
        return Err(ZmodemFailure::UnverifiedPublication);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn private_acl(file: &File, clear: bool) -> Result<()> {
    // Reject unknown ACL namespaces instead of treating mode 0600 as proof.
    let mut names = [0u8; 64 * 1024];
    let length = rustix::fs::flistxattr(file, &mut names[..])
        .map_err(std::io::Error::from)
        .map_err(io)?;
    for name in names[..length].split(|byte| *byte == 0) {
        let name = std::str::from_utf8(name).map_err(|_| ZmodemFailure::Unsupported)?;
        if !name.to_ascii_lowercase().contains("acl") {
            continue;
        }
        if !matches!(name, "system.posix_acl_access" | "system.posix_acl_default") || !clear {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        rustix::fs::fremovexattr(file, name)
            .map_err(std::io::Error::from)
            .map_err(io)?;
    }
    if clear {
        private_acl(file, false)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn private_acl(file: &File, clear: bool) -> Result<()> {
    crate::sftp::zmodem_private_file_acl(file, clear)
        .map_err(|_| ZmodemFailure::UnverifiedPublication)
}

#[cfg(test)]
mod tests;
