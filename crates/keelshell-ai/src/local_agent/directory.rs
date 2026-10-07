//! Directory authority is acquired off the UI thread and retained through cleanup.

use std::{
    fmt,
    fs::{File, Metadata},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use sha2::{Digest, Sha256};

use super::{LocalAgentError, LocalAgentKind};
use crate::RequestCancellation;

const CHECK_DEADLINE: Duration = Duration::from_secs(5);
const MAX_COMPONENTS: usize = 128;
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// A local Ask working directory, independent of home, credentials and tools.
#[derive(Clone, PartialEq, Eq, Default)]
pub enum LocalAgentWorkingDirectory {
    /// Create a new owned empty directory; existing profiles preserve this mode.
    #[default]
    Isolated,
    /// A user-authorized native absolute path requiring background validation.
    Selected(PathBuf),
}

impl fmt::Debug for LocalAgentWorkingDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Isolated => f.write_str("Isolated"),
            Self::Selected(_) => f.write_str("Selected(<private path>)"),
        }
    }
}

impl LocalAgentWorkingDirectory {
    /// Validate the explicit directory on a bounded worker without starting a CLI.
    ///
    /// The returned handles authorize only this reviewed filesystem identity.
    /// Codex metadata syntax is checked within the selected directory; no file
    /// contents are approved as model context or returned to the caller.
    pub async fn validate_directory(
        &self,
        kind: LocalAgentKind,
        cancellation: &RequestCancellation,
    ) -> Result<Option<ValidatedLocalAgentDirectory>, LocalAgentError> {
        match self {
            Self::Isolated => {
                cancelled(cancellation)?;
                Ok(None)
            }
            Self::Selected(path) => {
                ValidatedLocalAgentDirectory::acquire(path.clone(), kind, cancellation)
                    .await
                    .map(Some)
            }
        }
    }

    pub(super) fn validate_metadata(&self) -> Result<(), LocalAgentError> {
        let Self::Selected(path) = self else {
            return Ok(());
        };
        let text = path.to_str().ok_or(LocalAgentError::DirectoryInvalid)?;
        if !path.is_absolute()
            || text.len() > 4096
            || text.chars().any(char::is_control)
            || path.components().count() > MAX_COMPONENTS
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(LocalAgentError::DirectoryInvalid);
        }
        #[cfg(windows)]
        if !matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), std::path::Prefix::Disk(_)))
        {
            // Remote/device namespaces can block outside our local directory contract.
            return Err(LocalAgentError::DirectoryInvalid);
        }
        Ok(())
    }
}

struct DirectoryNode {
    path: PathBuf,
    // Retained on Windows to deny directory replacement; Unix also uses it for
    // the child-only fchdir path.
    _file: File,
    metadata: Metadata,
}

struct DirectoryGuard {
    selected: PathBuf,
    canonical: PathBuf,
    nodes: Vec<DirectoryNode>,
    codex_metadata: Option<MetadataReview>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MetadataSnapshot {
    Absent {},
    DirectoryOnly {
        folder_identity: [u64; 2],
    },
    Config {
        folder_identity: [u64; 2],
        file_identity: [u64; 2],
        digest: [u8; 32],
    },
}

struct MetadataReview {
    snapshot: MetadataSnapshot,
    // On Windows these handles also deny deletion/writes; Unix checks identity
    // and the complete bounded content again rather than relying on timestamps.
    _held_files: Vec<File>,
}

/// A private directory-handle authority for one reviewed Ask request.
///
/// This proves filesystem validation, not CLI compatibility. The runtime still
/// admits the exact CLI and effective policy on every invocation. Dropping this
/// value closes the owned handles; no selected directory is created or modified.
#[derive(Clone)]
pub struct ValidatedLocalAgentDirectory(Arc<DirectoryGuard>);

impl fmt::Debug for ValidatedLocalAgentDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ValidatedLocalAgentDirectory(<private path and handles>)")
    }
}

impl ValidatedLocalAgentDirectory {
    /// Exact user-selected path; display in full without logging it.
    pub fn selected_path(&self) -> &Path {
        &self.0.selected
    }

    /// Resolved real directory path; display in full beside the selected path.
    pub fn canonical_path(&self) -> &Path {
        &self.0.canonical
    }

    pub(super) async fn acquire(
        path: PathBuf,
        kind: LocalAgentKind,
        cancellation: &RequestCancellation,
    ) -> Result<Self, LocalAgentError> {
        LocalAgentWorkingDirectory::Selected(path.clone()).validate_metadata()?;
        let cancellation = cancellation.clone();
        let worker_cancel = cancellation.clone();
        let guard = bounded_check(cancellation, move || {
            let guard = DirectoryGuard::open(path, kind, &worker_cancel)?;
            guard.recheck(&worker_cancel)?;
            Ok(Self(Arc::new(guard)))
        })
        .await?;
        Ok(guard)
    }

    pub(super) async fn recheck(
        &self,
        cancellation: &RequestCancellation,
    ) -> Result<(), LocalAgentError> {
        let guard = self.0.clone();
        let worker_cancel = cancellation.clone();
        bounded_check(cancellation.clone(), move || guard.recheck(&worker_cancel)).await
    }

    #[cfg(unix)]
    pub(super) fn snapshot(&self) -> DirectorySnapshot {
        self.0.snapshot()
    }

    #[cfg(unix)]
    pub(super) fn recheck_child(
        &self,
        cancellation: &RequestCancellation,
    ) -> Result<(), LocalAgentError> {
        self.0.recheck(cancellation)
    }

    #[cfg(unix)]
    pub(super) fn enter_child(&self) -> Result<(), LocalAgentError> {
        let leaf = self
            .0
            .nodes
            .last()
            .ok_or(LocalAgentError::DirectoryUnavailable)?;
        // Called only by the single-thread internal launcher after exec, before
        // any runtime/GPUI initialization. Never changes the desktop parent's cwd.
        nix::unistd::fchdir(&leaf._file).map_err(|_| LocalAgentError::DirectoryUnavailable)
    }

    #[cfg(windows)]
    pub(super) fn child_path(&self) -> PathBuf {
        self.0.canonical.clone()
    }

    #[cfg(unix)]
    pub(super) fn reopen_child(
        snapshot: &DirectorySnapshot,
        kind: LocalAgentKind,
    ) -> Result<Self, LocalAgentError> {
        let cancellation = RequestCancellation::new();
        LocalAgentWorkingDirectory::Selected(snapshot.selected.clone()).validate_metadata()?;
        let guard = DirectoryGuard::open(snapshot.selected.clone(), kind, &cancellation)?;
        guard.recheck(&cancellation)?;
        if guard.snapshot() != *snapshot {
            return Err(LocalAgentError::DirectoryChanged);
        }
        Ok(Self(Arc::new(guard)))
    }
}

#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectorySnapshot {
    selected: PathBuf,
    canonical: PathBuf,
    nodes: Vec<[u64; 2]>,
    codex_metadata: Option<MetadataSnapshot>,
}

impl DirectoryGuard {
    #[cfg(unix)]
    fn snapshot(&self) -> DirectorySnapshot {
        DirectorySnapshot {
            selected: self.selected.clone(),
            canonical: self.canonical.clone(),
            nodes: self
                .nodes
                .iter()
                .map(|node| metadata_identity(&node.metadata))
                .collect(),
            codex_metadata: self
                .codex_metadata
                .as_ref()
                .map(|review| review.snapshot.clone()),
        }
    }
}

pub(super) async fn bounded_check<T: Send + 'static>(
    cancellation: RequestCancellation,
    operation: impl FnOnce() -> Result<T, LocalAgentError> + Send + 'static,
) -> Result<T, LocalAgentError> {
    cancelled(&cancellation)?;
    let job = tokio::task::spawn_blocking(operation);
    tokio::select! {
        result = job => {
            cancelled(&cancellation)?;
            result.map_err(|_| LocalAgentError::DirectoryUnavailable)?
        }
        _ = tokio::time::sleep(CHECK_DEADLINE) => {
            // No late worker result may retain review authority. OS filesystem
            // calls cannot be interrupted; cancel checks stop the next step.
            cancellation.cancel();
            Err(LocalAgentError::DirectoryValidationTimedOut)
        },
        _ = observe_cancel(&cancellation) => Err(LocalAgentError::Cancelled),
    }
}

async fn observe_cancel(cancellation: &RequestCancellation) {
    while !cancellation.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn cancelled(cancellation: &RequestCancellation) -> Result<(), LocalAgentError> {
    if cancellation.is_cancelled() {
        Err(LocalAgentError::Cancelled)
    } else {
        Ok(())
    }
}

impl DirectoryGuard {
    fn open(
        path: PathBuf,
        kind: LocalAgentKind,
        cancellation: &RequestCancellation,
    ) -> Result<Self, LocalAgentError> {
        let mut nodes = Vec::new();
        let mut prefix = PathBuf::new();
        for part in path.components() {
            cancelled(cancellation)?;
            prefix.push(part.as_os_str());
            if matches!(part, Component::Prefix(_)) {
                continue;
            }
            let file = open_directory(&prefix, nodes.last(), part)?;
            let metadata = file.metadata().map_err(directory_error)?;
            reject_link(&metadata)?;
            if !metadata.is_dir() {
                return Err(LocalAgentError::DirectoryNotDirectory);
            }
            nodes.push(DirectoryNode {
                path: prefix.clone(),
                _file: file,
                metadata,
            });
        }
        let canonical = std::fs::canonicalize(&path).map_err(directory_error)?;
        let leaf = nodes.last().ok_or(LocalAgentError::DirectoryInvalid)?;
        if !same_identity(
            &leaf.metadata,
            &std::fs::metadata(&canonical).map_err(directory_error)?,
        ) {
            return Err(LocalAgentError::DirectoryChanged);
        }
        let codex_metadata = if kind == LocalAgentKind::Codex {
            Some(read_codex_metadata(leaf, cancellation)?)
        } else {
            None
        };
        Ok(Self {
            selected: path,
            canonical,
            nodes,
            codex_metadata,
        })
    }

    fn recheck(&self, cancellation: &RequestCancellation) -> Result<(), LocalAgentError> {
        for node in &self.nodes {
            cancelled(cancellation)?;
            let actual = std::fs::symlink_metadata(&node.path)
                .map_err(|_| LocalAgentError::DirectoryChanged)?;
            reject_link(&actual)?;
            if !actual.is_dir() || !same_identity(&node.metadata, &actual) {
                return Err(LocalAgentError::DirectoryChanged);
            }
        }
        if std::fs::canonicalize(&self.selected).map_err(|_| LocalAgentError::DirectoryChanged)?
            != self.canonical
        {
            return Err(LocalAgentError::DirectoryChanged);
        }
        if let Some(previous) = &self.codex_metadata {
            let leaf = self.nodes.last().ok_or(LocalAgentError::DirectoryChanged)?;
            let current = read_codex_metadata(leaf, cancellation)?;
            if previous.snapshot != current.snapshot {
                return Err(LocalAgentError::DirectoryMetadataChanged);
            }
        }
        cancelled(cancellation)
    }
}

fn reject_link(metadata: &Metadata) -> Result<(), LocalAgentError> {
    if metadata.file_type().is_symlink() {
        return Err(LocalAgentError::DirectorySymlink);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(LocalAgentError::DirectorySymlink);
        }
    }
    Ok(())
}

fn directory_error(error: std::io::Error) -> LocalAgentError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return LocalAgentError::DirectoryMissing;
    }
    if error.kind() == std::io::ErrorKind::NotADirectory {
        return LocalAgentError::DirectoryNotDirectory;
    }
    LocalAgentError::DirectoryUnavailable
}

#[cfg(unix)]
fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
fn metadata_identity(metadata: &Metadata) -> [u64; 2] {
    use std::os::unix::fs::MetadataExt;
    [metadata.dev(), metadata.ino()]
}

#[cfg(windows)]
fn metadata_identity(metadata: &Metadata) -> [u64; 2] {
    use std::os::windows::fs::MetadataExt;
    // Path replacement is prevented by live handles denying DELETE sharing.
    [
        metadata.creation_time(),
        u64::from(metadata.file_attributes()),
    ]
}

#[cfg(not(any(unix, windows)))]
fn metadata_identity(_metadata: &Metadata) -> [u64; 2] {
    [0, 0]
}

#[cfg(windows)]
fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // Replacement is prevented by retained handles, not by this timestamp.
    left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}

#[cfg(not(any(unix, windows)))]
fn same_identity(_left: &Metadata, _right: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn open_directory(
    path: &Path,
    parent: Option<&DirectoryNode>,
    part: Component<'_>,
) -> Result<File, LocalAgentError> {
    use nix::{
        fcntl::{OFlag, open, openat},
        sys::stat::Mode,
    };
    let flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let result = if let (Some(parent), Component::Normal(name)) = (parent, part) {
        openat(&parent._file, Path::new(name), flags, Mode::empty())
    } else {
        open(path, flags, Mode::empty())
    };
    result.map(File::from).map_err(|error| match error {
        nix::errno::Errno::ELOOP => LocalAgentError::DirectorySymlink,
        nix::errno::Errno::ENOENT => LocalAgentError::DirectoryMissing,
        nix::errno::Errno::ENOTDIR => {
            if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
                LocalAgentError::DirectorySymlink
            } else {
                LocalAgentError::DirectoryNotDirectory
            }
        }
        _ => LocalAgentError::DirectoryUnavailable,
    })
}

#[cfg(windows)]
fn open_directory(
    path: &Path,
    _parent: Option<&DirectoryNode>,
    _part: Component<'_>,
) -> Result<File, LocalAgentError> {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE, no DELETE.
        .custom_flags(0x0200_0000 | 0x0020_0000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT.
        .open(path)
        .map_err(directory_error)
}

#[cfg(not(any(unix, windows)))]
fn open_directory(
    _path: &Path,
    _parent: Option<&DirectoryNode>,
    _part: Component<'_>,
) -> Result<File, LocalAgentError> {
    Err(LocalAgentError::DirectoryUnsupported)
}

fn read_codex_metadata(
    leaf: &DirectoryNode,
    cancellation: &RequestCancellation,
) -> Result<MetadataReview, LocalAgentError> {
    cancelled(cancellation)?;
    let folder = leaf.path.join(".codex");
    let metadata = match std::fs::symlink_metadata(&folder) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MetadataReview {
                snapshot: MetadataSnapshot::Absent {},
                _held_files: Vec::new(),
            });
        }
        Err(_) => return Err(LocalAgentError::DirectoryUnavailable),
    };
    reject_link(&metadata)?;
    if !metadata.is_dir() {
        return Err(LocalAgentError::DirectoryMetadataInvalid);
    }
    let folder_file = open_directory(
        &folder,
        Some(leaf),
        Component::Normal(std::ffi::OsStr::new(".codex")),
    )?;
    let folder_actual = folder_file.metadata().map_err(directory_error)?;
    if !same_identity(&metadata, &folder_actual) {
        return Err(LocalAgentError::DirectoryMetadataChanged);
    }
    let config = folder.join("config.toml");
    let file_meta = match std::fs::symlink_metadata(&config) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MetadataReview {
                snapshot: MetadataSnapshot::DirectoryOnly {
                    folder_identity: metadata_identity(&folder_actual),
                },
                _held_files: vec![folder_file],
            });
        }
        Err(_) => return Err(LocalAgentError::DirectoryUnavailable),
    };
    reject_link(&file_meta)?;
    if !file_meta.is_file() || file_meta.len() > MAX_CONFIG_BYTES {
        return Err(LocalAgentError::DirectoryMetadataInvalid);
    }
    #[cfg(unix)]
    let mut file = {
        use nix::{
            fcntl::{OFlag, openat},
            sys::stat::Mode,
        };
        File::from(
            openat(
                &folder_file,
                "config.toml",
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?,
        )
    };
    #[cfg(windows)]
    let mut file = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1)
            .custom_flags(0x0020_0000)
            .open(&config)
            .map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?
    };
    #[cfg(not(any(unix, windows)))]
    return Err(LocalAgentError::DirectoryUnsupported);
    #[cfg(any(unix, windows))]
    {
        let actual = file
            .metadata()
            .map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?;
        reject_link(&actual)?;
        if !actual.is_file() || !same_identity(&file_meta, &actual) {
            return Err(LocalAgentError::DirectoryMetadataInvalid);
        }
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        (&mut file)
            .take(MAX_CONFIG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?;
        cancelled(cancellation)?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err(LocalAgentError::DirectoryMetadataInvalid);
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?;
        let _: toml::Table =
            toml::from_str(text).map_err(|_| LocalAgentError::DirectoryMetadataInvalid)?;
        // Only the digest survives; metadata contents never enter the preview/stdin.
        Ok(MetadataReview {
            snapshot: MetadataSnapshot::Config {
                folder_identity: metadata_identity(&folder_actual),
                file_identity: metadata_identity(&actual),
                digest: Sha256::digest(&bytes).into(),
            },
            _held_files: vec![folder_file, file],
        })
    }
}

#[cfg(test)]
mod tests;
