//! Bounded metadata history and explicit recovery under the state-store lock.
use super::*;
use std::time::UNIX_EPOCH;
use uuid::Uuid;

/// Maximum retained snapshots: eight documents of at most 4 MiB plus 256 bytes each.
pub const MAX_CONFIG_BACKUPS: usize = 8;
/// Maximum preserved originals; these are never automatically deleted.
pub const MAX_CONFIG_ORIGINALS: usize = 8;

const MAX_BACKUP_BYTES: usize = MAX_DOCUMENT_BYTES + 256;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupDocument {
    backup_format_version: u32,
    sequence: u64,
    state: AppState,
}

/// Opaque identifier for a backup in this store's private history directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigBackupId(Uuid);

impl ConfigBackupId {
    /// Return the non-secret identifier for display, never a caller-supplied path.
    pub fn uuid(self) -> Uuid {
        self.0
    }
}

/// Validation result for a bounded metadata snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigBackupStatus {
    /// Fully validated by this build's schema and domain rules.
    Available,
    /// JSON or domain constraints were invalid; this file cannot be restored.
    Invalid,
    /// This build cannot interpret the named schema; no downgrade is attempted.
    UnsupportedSchema(u32),
}

/// Non-secret list entry; listing does not activate or restore any state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigBackup {
    /// Store-owned identifier.
    pub id: ConfigBackupId,
    /// Monotonic local sequence used for rotation; absent for invalid wrappers.
    pub sequence: Option<u64>,
    /// File modification time in Unix milliseconds, absent when unavailable.
    pub modified_unix_millis: Option<u128>,
    /// Exact bounded file size.
    pub bytes: u64,
    /// Whether this build can safely preview and restore the snapshot.
    pub status: ConfigBackupStatus,
}

/// Current state classification at the moment of recovery review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSourceStatus {
    /// No state file exists; recovery would create it.
    Missing,
    /// The current metadata is valid and will be preserved before replacement.
    Valid,
    /// The current bytes cannot be decoded and will be preserved exactly.
    Corrupt,
    /// The current schema is unsupported; exact original bytes will be preserved.
    UnsupportedSchema(u32),
}

/// Counts for a review without exposing names, endpoints, command text or secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigRecoverySummary {
    /// Number of active connection profiles in the reviewed configuration.
    pub connections: usize,
    /// Number of soft-deleted profiles in the reviewed configuration.
    pub deleted_connections: usize,
    /// Number of connection folders in the reviewed configuration.
    pub folders: usize,
    /// Number of saved command templates in the reviewed configuration.
    pub snippets: usize,
    /// Number of AI configurations (credential references only).
    pub ai_profiles: usize,
    /// Number of saved trust entries; these do not authenticate a new session.
    pub trusted_hosts: usize,
}

/// Opaque review binding the store, current bytes/revision and selected backup.
///
/// It grants no transport, command, transfer or external-agent authority. Pass it
/// to [`StateStore::restore_config_backup`] only after explicit user approval.
/// It implements no Debug output that could disclose original or backup contents.
#[derive(Clone)]
pub struct ConfigRecoveryPreview {
    store_identity: Uuid,
    observed_revision: Option<SnapshotRevision>,
    current: Option<Vec<u8>>,
    backup: Vec<u8>,
    id: ConfigBackupId,
    current_status: ConfigSourceStatus,
    current_summary: Option<ConfigRecoverySummary>,
    summary: ConfigRecoverySummary,
}

impl ConfigRecoveryPreview {
    /// Return the exact selected backup identity.
    pub fn backup_id(&self) -> ConfigBackupId {
        self.id
    }
    /// Return the reviewed current-state classification.
    pub fn current_status(&self) -> ConfigSourceStatus {
        self.current_status
    }
    /// Return counts from the exact current document bound to this review.
    ///
    /// Missing, corrupt and unsupported documents return `None`; no default
    /// state is substituted for an unreadable original.
    pub fn current_summary(&self) -> Option<ConfigRecoverySummary> {
        self.current_summary
    }
    /// Return metadata counts from the validated replacement.
    pub fn summary(&self) -> ConfigRecoverySummary {
        self.summary
    }
}

impl StateStore {
    /// List at most eight owner-only metadata backups without creating snapshots.
    ///
    /// All I/O is blocking. Invalid or unsupported documents remain visible but
    /// cannot be previewed. Unsafe files, unrecognized names and excess entries
    /// fail closed instead of being pruned. Vaults and OS credentials are excluded.
    /// An existing configuration directory can gain the persistent cooperative
    /// lock file; listing never replaces configuration or creates history entries.
    pub fn config_backups(&self) -> Result<Vec<ConfigBackup>, Error> {
        let _observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        match fs::symlink_metadata(self.parent()?) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
            Ok(metadata) if !metadata.file_type().is_dir() => return Err(Error::UnsafePath),
            Ok(_) => {}
        }
        let _lock = self.lock_file()?;
        let directory = self.history_path("backups")?;
        let entries = history_entries(
            &directory,
            MAX_CONFIG_BACKUPS,
            MAX_BACKUP_BYTES,
            Error::ConfigBackupLimit,
        )?;
        let mut backups: Vec<ConfigBackup> = entries
            .into_iter()
            .map(|(id, path, metadata)| {
                let bytes =
                    read_bounded(&path, MAX_BACKUP_BYTES)?.ok_or(Error::ConfigBackupNotFound)?;
                let (sequence, status) = match decode_backup(&bytes) {
                    Ok(document) => (Some(document.sequence), ConfigBackupStatus::Available),
                    Err(Error::UnsupportedSchema { found, .. }) => {
                        (None, ConfigBackupStatus::UnsupportedSchema(found))
                    }
                    Err(_) => (None, ConfigBackupStatus::Invalid),
                };
                Ok(ConfigBackup {
                    id,
                    sequence,
                    modified_unix_millis: metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis()),
                    bytes: metadata.len(),
                    status,
                })
            })
            .collect::<Result<_, Error>>()?;
        backups.sort_by_key(|backup| std::cmp::Reverse(backup.sequence));
        Ok(backups)
    }

    /// Explicitly snapshot the valid current metadata, with the same eight-file rotation.
    ///
    /// A missing or corrupt state is never converted to defaults or backed up as
    /// valid. The credential vault and OS store are not opened or copied.
    pub fn create_config_backup(&self) -> Result<ConfigBackupId, Error> {
        let _observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file()?;
        let bytes = read_document(&self.path)?.ok_or(Error::NotLoaded)?;
        self.rotate_backup(&bytes)
    }

    /// Validate a selected backup and bind a review to exact current disk bytes.
    ///
    /// This also works when [`Self::load`] rejected a damaged or future-schema
    /// document. It never replaces metadata or creates snapshots; acquiring the
    /// cooperative lock may create its persistent lock file. Files over 4 MiB, symlinks and unsafe
    /// permissions require manual repair and are never accepted for recovery.
    pub fn preview_config_backup(
        &self,
        id: ConfigBackupId,
    ) -> Result<ConfigRecoveryPreview, Error> {
        let observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file()?;
        let current = read_document(&self.path)?;
        let backup = self.selected_backup(id)?;
        let state = decode_backup(&backup)?.state;
        let (current_status, current_summary) = source_review(current.as_deref());
        Ok(ConfigRecoveryPreview {
            store_identity: self.identity,
            observed_revision: observed_revision(&observed),
            current,
            backup,
            id,
            current_status,
            current_summary,
            summary: configuration_summary(&state),
        })
    }

    /// Restore an explicitly approved review, preserving exact originals first.
    ///
    /// Both current bytes and the process-local revision must still match. The
    /// selected backup is re-read and validated under the same cooperative lock.
    /// Existing originals are stored owner-only in `state.json.originals`; up to
    /// eight are preserved and never pruned automatically. A full originals
    /// directory refuses recovery before replacement. Missing state has no original.
    ///
    /// Atomic replacement failures leave current bytes untouched. A directory-sync
    /// failure after replacement attempts atomic rollback; [`Error::ConfigRecoveryRequired`]
    /// retains any existing original and discards the store baseline for manual repair.
    /// A success issues a fresh revision; callers must use the returned snapshot.
    /// This API activates no SSH/AI/MCP operations and never replays work.
    pub fn restore_config_backup(
        &self,
        preview: &ConfigRecoveryPreview,
    ) -> Result<AppState, Error> {
        self.restore_with(preview, sync_parent)
    }

    fn restore_with(
        &self,
        preview: &ConfigRecoveryPreview,
        synchronize: impl Fn(&Path) -> std::io::Result<()>,
    ) -> Result<AppState, Error> {
        let mut observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file()?;
        if self.identity != preview.store_identity
            || observed_revision(&observed) != preview.observed_revision
            || read_document(&self.path)? != preview.current
            || self.selected_backup(preview.id)? != preview.backup
        {
            return Err(Error::ConfigRecoveryConflict);
        }
        let mut state = decode_backup(&preview.backup)?.state;
        let mut replacement = serde_json::to_vec(&state)?;
        replacement.push(b'\n');
        if replacement.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        if let Some(original) = &preview.current {
            self.preserve_original(original)?;
        }
        atomic_replace(self.parent()?, &self.path, &replacement)?;
        if let Err(error) = synchronize(self.parent()?) {
            // Once replacement happened, an in-memory baseline cannot certify
            // the disk's identity until rollback or a new load actually succeeds.
            *observed = Observed::Unloaded;
            let rollback = match &preview.current {
                Some(original) => atomic_replace(self.parent()?, &self.path, original),
                None => fs::remove_file(&self.path).map_err(Error::Io),
            }
            .and_then(|()| sync_parent(self.parent()?).map_err(Error::Io));
            return match rollback {
                Ok(()) => Err(Error::ConfigRecoveryRolledBack(error)),
                Err(_) => Err(Error::ConfigRecoveryRequired),
            };
        }
        state.snapshot = SnapshotRevision::fresh();
        *observed = Observed::Loaded {
            bytes: Some(replacement),
            revision: state.snapshot,
        };
        Ok(state)
    }

    fn selected_backup(&self, id: ConfigBackupId) -> Result<Vec<u8>, Error> {
        let directory = self.history_path("backups")?;
        let entries = history_entries(
            &directory,
            MAX_CONFIG_BACKUPS,
            MAX_BACKUP_BYTES,
            Error::ConfigBackupLimit,
        )?;
        let (_, path, _) = entries
            .into_iter()
            .find(|(candidate, _, _)| *candidate == id)
            .ok_or(Error::ConfigBackupNotFound)?;
        read_bounded(&path, MAX_BACKUP_BYTES)?.ok_or(Error::ConfigBackupNotFound)
    }

    pub(super) fn rotate_backup(&self, bytes: &[u8]) -> Result<ConfigBackupId, Error> {
        let state = decode(bytes)?;
        let directory = self.history_path("backups")?;
        prepare_directory(&directory)?;
        check_owner_permissions(&fs::symlink_metadata(&directory)?)?;
        let entries = history_entries(
            &directory,
            MAX_CONFIG_BACKUPS,
            MAX_BACKUP_BYTES,
            Error::ConfigBackupLimit,
        )?;
        let mut ordered = entries
            .into_iter()
            .map(|(id, path, _)| {
                let bytes =
                    read_bounded(&path, MAX_BACKUP_BYTES)?.ok_or(Error::ConfigBackupNotFound)?;
                let document = decode_backup(&bytes)?;
                Ok((document.sequence, id, path))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        ordered.sort_by_key(|(sequence, id, _)| (*sequence, id.0));
        if ordered.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::ConfigBackupFormat);
        }
        let sequence = ordered
            .last()
            .map_or(Some(1), |entry| entry.0.checked_add(1))
            .ok_or(Error::ConfigBackupLimit)?;
        let document = serde_json::to_vec(&BackupDocument {
            backup_format_version: 1,
            sequence,
            state,
        })?;
        if document.len() > MAX_BACKUP_BYTES {
            return Err(Error::TooLarge);
        }
        if ordered.len() == MAX_CONFIG_BACKUPS {
            let (_, id, path) = &ordered[0];
            // Reuse an owned slot with atomic replacement. There is never a ninth
            // persistent file; a review still binds the slot's exact prior bytes.
            atomic_replace(&directory, path, &document)?;
            sync_parent(&directory)?;
            sync_parent(self.parent()?)?;
            return Ok(*id);
        }
        let id = ConfigBackupId(Uuid::new_v4());
        let path = directory.join(format!("{}.json", id.0));
        write_exclusive(&directory, &path, &document)?;
        sync_parent(self.parent()?)?;
        Ok(id)
    }

    fn preserve_original(&self, bytes: &[u8]) -> Result<(), Error> {
        let directory = self.history_path("originals")?;
        prepare_directory(&directory)?;
        check_owner_permissions(&fs::symlink_metadata(&directory)?)?;
        let entries = history_entries(
            &directory,
            MAX_CONFIG_ORIGINALS,
            MAX_DOCUMENT_BYTES,
            Error::ConfigOriginalLimit,
        )?;
        if entries.len() == MAX_CONFIG_ORIGINALS {
            return Err(Error::ConfigOriginalLimit);
        }
        write_exclusive(
            &directory,
            &directory.join(format!("{}.json", Uuid::new_v4())),
            bytes,
        )?;
        // The original directory may be newly created; its parent's entry must
        // also be durable before the current configuration can be replaced.
        sync_parent(self.parent()?).map_err(Error::Io)
    }

    fn history_path(&self, suffix: &str) -> Result<PathBuf, Error> {
        let mut name = self
            .path
            .file_name()
            .ok_or(Error::UnsafePath)?
            .to_os_string();
        name.push(".");
        name.push(suffix);
        Ok(self.parent()?.join(name))
    }
}

fn observed_revision(observed: &Observed) -> Option<SnapshotRevision> {
    match observed {
        Observed::Unloaded => None,
        Observed::Loaded { revision, .. } => Some(*revision),
    }
}

fn configuration_summary(state: &AppState) -> ConfigRecoverySummary {
    ConfigRecoverySummary {
        connections: state.connections.len(),
        deleted_connections: state.deleted_connections.len(),
        folders: state.folders.len(),
        snippets: state.snippets.len(),
        ai_profiles: state.settings.ai_profiles.profiles.len(),
        trusted_hosts: state.known_hosts.len() + state.route_known_hosts.len(),
    }
}

fn source_review(bytes: Option<&[u8]>) -> (ConfigSourceStatus, Option<ConfigRecoverySummary>) {
    match bytes {
        None => (ConfigSourceStatus::Missing, None),
        Some(bytes) => match decode(bytes) {
            Ok(state) => (
                ConfigSourceStatus::Valid,
                Some(configuration_summary(&state)),
            ),
            Err(Error::UnsupportedSchema { found, .. }) => {
                (ConfigSourceStatus::UnsupportedSchema(found), None)
            }
            Err(_) => (ConfigSourceStatus::Corrupt, None),
        },
    }
}

type HistoryEntry = (ConfigBackupId, PathBuf, fs::Metadata);

fn history_entries(
    directory: &Path,
    limit: usize,
    byte_limit: usize,
    limit_error: Error,
) -> Result<Vec<HistoryEntry>, Error> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
        Ok(metadata) if !metadata.file_type().is_dir() => return Err(Error::UnsafePath),
        Ok(metadata) => check_owner_permissions(&metadata)?,
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        if entries.len() == limit {
            return Err(limit_error);
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::UnsafePath)?;
        let id = name
            .strip_suffix(".json")
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|id| format!("{id}.json") == name)
            .ok_or(Error::UnsafePath)?;
        let path = entry.path();
        let metadata = regular_metadata(&path)?.ok_or(Error::ConfigBackupNotFound)?;
        check_owner_permissions(&metadata)?;
        if metadata.len() > byte_limit as u64 {
            return Err(Error::TooLarge);
        }
        entries.push((ConfigBackupId(id), path, metadata));
    }
    Ok(entries)
}

fn decode_backup(bytes: &[u8]) -> Result<BackupDocument, Error> {
    #[derive(serde::Deserialize)]
    struct Header {
        backup_format_version: u32,
        sequence: u64,
        state: StateHeader,
    }
    #[derive(serde::Deserialize)]
    struct StateHeader {
        schema_version: u32,
    }
    let header: Header = serde_json::from_slice(bytes)?;
    if header.backup_format_version != 1 || header.sequence == 0 {
        return Err(Error::ConfigBackupFormat);
    }
    crate::model::schema(header.state.schema_version)?;
    let document: BackupDocument = serde_json::from_slice(bytes)?;
    document.state.validate()?;
    Ok(document)
}

fn write_exclusive(directory: &Path, path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = tempfile::Builder::new()
        .prefix(".keelshell-history-")
        .tempfile_in(directory)?;
    set_owner_permissions(file.as_file())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)
        .map_err(|error| Error::Io(error.error))?;
    sync_parent(directory)?;
    Ok(())
}

fn atomic_replace(parent: &Path, path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = tempfile::Builder::new()
        .prefix(".keelshell-recovery-")
        .tempfile_in(parent)?;
    set_owner_permissions(file.as_file())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn sync_parent(parent: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = parent;
    Ok(())
}

#[cfg(test)]
mod tests;
