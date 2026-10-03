use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use directories::ProjectDirs;

use crate::{AppState, Error, SnapshotRevision, model::MAX_DOCUMENT_BYTES};

#[derive(Default)]
enum Observed {
    #[default]
    Unloaded,
    Loaded {
        bytes: Option<Vec<u8>>,
        revision: SnapshotRevision,
    },
}

/// Versioned JSON storage with atomic replacement and lost-update detection.
///
/// Call [`Self::load`] before editing existing state. A corrupt/unsupported file is
/// never replaced with defaults. A missing file returns defaults without creating
/// it. Saves synchronize the temporary file before replacing the destination;
/// Unix also synchronizes the containing directory.
///
/// Unix state/lock files must be owner-only and newly created directories use
/// 0700. Windows uses permissions inherited from the per-user config directory;
/// explicit Windows ACL enforcement is not provided by this crate. The directory
/// must be trusted: cooperative file locks do not defend against a malicious
/// same-user process replacing path components concurrently.
///
/// All methods perform blocking filesystem I/O; invoke them on a worker thread.
pub struct StateStore {
    path: PathBuf,
    observed: Mutex<Observed>,
}

impl StateStore {
    /// Create a store at an explicit path without touching the filesystem.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            observed: Mutex::new(Observed::Unloaded),
        }
    }

    /// Resolve `state.json` in the platform's per-user KeelShell config directory.
    ///
    /// This does not depend on the application's working directory and creates no files.
    pub fn default_path() -> Result<PathBuf, Error> {
        ProjectDirs::from("dev", "KeelShell", "KeelShell")
            .map(|dirs| dirs.config_dir().join("state.json"))
            .ok_or(Error::NoConfigDirectory)
    }

    /// Return the configured state location for diagnostics or an open-folder action.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read and validate state, or return defaults only when the file is absent.
    ///
    /// A successful load records disk bytes and attaches an opaque revision to
    /// the returned snapshot. Repeated loads of unchanged bytes share a revision.
    /// On failure the previous baseline is discarded to prevent later overwrite.
    pub fn load(&self) -> Result<AppState, Error> {
        let mut observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let loaded = (|| {
            if regular_metadata(&self.path)?.is_none() {
                return Ok((AppState::default(), None));
            }
            let _lock = self.lock_file()?;
            let bytes = read_document(&self.path)?;
            let state = match &bytes {
                Some(bytes) => decode(bytes)?,
                None => AppState::default(),
            };
            Ok::<_, Error>((state, bytes))
        })();
        let (mut state, bytes) = match loaded {
            Ok(value) => value,
            Err(error) => {
                *observed = Observed::Unloaded;
                return Err(error);
            }
        };
        let revision = match &*observed {
            Observed::Loaded {
                bytes: previous,
                revision,
            } if previous == &bytes => *revision,
            _ => SnapshotRevision::fresh(),
        };
        state.snapshot = revision;
        *observed = Observed::Loaded { bytes, revision };
        Ok(state)
    }

    /// Validate and atomically save state, refusing to overwrite an unseen change.
    ///
    /// Retain the same store between load/save and replace the edited state with
    /// the returned state after success: `state = store.save(&state)?`. A save
    /// rotates the revision, so all other snapshots from that revision become
    /// stale even when they share this store. On [`Error::Conflict`], reload and
    /// merge in the UI; blindly retrying would discard another process's changes.
    /// On [`Error::Durability`], replacement happened but crash durability is unknown;
    /// reload the committed state before making another edit.
    pub fn save(&self, state: &AppState) -> Result<AppState, Error> {
        state.validate()?;
        let mut bytes = serde_json::to_vec_pretty(state)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        let mut observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let parent = self.parent()?;
        prepare_directory(parent)?;
        let _lock = self.lock_file()?;
        let current = read_document(&self.path)?;
        // Even a caller that skipped load cannot destroy a malformed or newer file.
        if let Some(current) = &current {
            decode(current)?;
        }
        match &*observed {
            Observed::Unloaded if current.is_some() => return Err(Error::NotLoaded),
            Observed::Unloaded if state.snapshot.is_loaded() => return Err(Error::Conflict),
            Observed::Loaded {
                bytes: previous,
                revision,
            } if previous != &current || state.snapshot != *revision => {
                return Err(Error::Conflict);
            }
            _ => {}
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".keelshell-state-")
            .tempfile_in(parent)?;
        set_owner_permissions(temporary.as_file())?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        let mut saved = state.clone();
        saved.snapshot = SnapshotRevision::fresh();
        *observed = Observed::Loaded {
            bytes: Some(bytes),
            revision: saved.snapshot,
        };
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(Error::Durability)?;
        Ok(saved)
    }

    fn parent(&self) -> Result<&Path, Error> {
        if self.path.file_name().is_none() {
            return Err(Error::UnsafePath);
        }
        Ok(self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new(".")))
    }

    fn lock_file(&self) -> Result<File, Error> {
        let parent = self.parent()?;
        check_directory(parent)?;
        let mut name = self
            .path
            .file_name()
            .ok_or(Error::UnsafePath)?
            .to_os_string();
        name.push(".lock");
        let path = parent.join(name);
        regular_metadata(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        check_owner_permissions(&file.metadata()?)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::Busy,
            fs::TryLockError::Error(error) => Error::Io(error),
        })?;
        // Keep this inode permanently: deleting a lock file would allow two owners
        // to lock different inodes under the same name. Closing File releases it.
        Ok(file)
    }
}

fn decode(bytes: &[u8]) -> Result<AppState, Error> {
    #[derive(serde::Deserialize)]
    struct Header {
        schema_version: u32,
    }
    let header: Header = serde_json::from_slice(bytes)?;
    crate::model::schema(header.schema_version)?;
    let state: AppState = serde_json::from_slice(bytes)?;
    state.validate()?;
    Ok(state)
}

fn read_document(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let Some(metadata) = regular_metadata(path)? else {
        return Ok(None);
    };
    check_owner_permissions(&metadata)?;
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(Some(bytes))
}

fn regular_metadata(path: &Path) -> Result<Option<fs::Metadata>, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(metadata)),
        Ok(_) => Err(Error::UnsafePath),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn check_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

fn prepare_directory(path: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    check_directory(path)
}

fn check_owner_permissions(metadata: &fs::Metadata) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Error::InsecurePermissions);
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

fn set_owner_permissions(file: &File) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}
