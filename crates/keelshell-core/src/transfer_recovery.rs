//! Durable metadata for explicit post-restart transfer recovery.
//!
//! The journal contains paths, direction, transfer kind, a bounded progress
//! checkpoint and an outcome classification. It deliberately has no SSH
//! session identity, credential reference, review token, secret or worker
//! handle. Loading it is observational; callers must create a new session and
//! a new review before any transfer can start.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, ValidationError};

/// Schema version for the separate transfer-recovery journal.
pub const TRANSFER_RECOVERY_SCHEMA_VERSION: u32 = 1;
/// Maximum number of unfinished or unresolved transfer records retained.
pub const MAX_TRANSFER_RECOVERY_RECORDS: usize = 32;
/// Maximum serialized recovery document size.
pub const MAX_TRANSFER_RECOVERY_DOCUMENT_BYTES: usize = 256 * 1024;
/// Maximum UTF-8 path length accepted for each persisted path.
pub const MAX_TRANSFER_RECOVERY_PATH_BYTES: usize = 4096;

/// Direction of a persisted transfer descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferRecoveryDirection {
    /// Bytes were intended to flow from the local path to the remote path.
    Upload,
    /// Bytes were intended to flow from the remote path to the local path.
    Download,
}

/// Last known class of a transfer when the process stopped or the result was
/// retained for explicit inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferRecoveryState {
    /// The last durable checkpoint was before a terminal result.
    Unfinished,
    /// The transport could not prove whether a mutation completed.
    OutcomeUnknown,
    /// The transfer ended with a known failure.
    Failed,
    /// The user or transport cancelled the transfer; partial output may remain.
    Cancelled,
}

/// Safe, bounded metadata that can survive an application restart.
///
/// This type is intentionally independent of the transport crate. In
/// particular, it cannot contain an authenticated session, an SFTP handle, a
/// review token or a worker owner. The paths are displayed for inspection and
/// are converted into a fresh transport request only by the application after
/// a current-session read-only plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferRecoveryRecord {
    /// Stable identifier for this recovery entry.
    pub id: Uuid,
    /// Saved connection profile identity when the transfer originated from one.
    ///
    /// Quick-connect and legacy records may omit this binding. The application
    /// must then show the current SSH endpoint and require the user to confirm
    /// that it is the original target before continuing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<Uuid>,
    /// Direction of the original transfer.
    pub direction: TransferRecoveryDirection,
    /// Local source or destination path.
    pub local_path: PathBuf,
    /// Remote source or destination path.
    pub remote_path: String,
    /// Whether the descriptor represents a directory transfer.
    pub directory: bool,
    /// Last known safe outcome class.
    pub state: TransferRecoveryState,
    /// Bytes in the last durable application checkpoint.
    pub transferred_bytes: u64,
    /// Total source bytes when a checkpoint knew the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    /// Bytes already verified when the transfer was a continuation.
    pub reviewed_existing_bytes: u64,
    /// Unix seconds when the record was first created.
    pub created_at: u64,
    /// Unix seconds of the last record update.
    pub updated_at: u64,
}

impl TransferRecoveryRecord {
    /// Construct and validate one durable recovery descriptor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        direction: TransferRecoveryDirection,
        local_path: impl Into<PathBuf>,
        remote_path: impl Into<String>,
        directory: bool,
        state: TransferRecoveryState,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
        reviewed_existing_bytes: u64,
        created_at: u64,
        updated_at: u64,
    ) -> Result<Self, ValidationError> {
        Self::new_with_connection_id(
            id,
            None,
            direction,
            local_path,
            remote_path,
            directory,
            state,
            transferred_bytes,
            total_bytes,
            reviewed_existing_bytes,
            created_at,
            updated_at,
        )
    }

    /// Construct and validate a descriptor bound to a saved connection.
    ///
    /// The connection ID is a local profile identity, not an authentication
    /// credential or a host-key proof. A current session must still complete
    /// its normal SSH and host-key checks before a continuation is reviewed.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_connection_id(
        id: Uuid,
        connection_id: Option<Uuid>,
        direction: TransferRecoveryDirection,
        local_path: impl Into<PathBuf>,
        remote_path: impl Into<String>,
        directory: bool,
        state: TransferRecoveryState,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
        reviewed_existing_bytes: u64,
        created_at: u64,
        updated_at: u64,
    ) -> Result<Self, ValidationError> {
        let record = Self {
            id,
            connection_id,
            direction,
            local_path: local_path.into(),
            remote_path: remote_path.into(),
            directory,
            state,
            transferred_bytes,
            total_bytes,
            reviewed_existing_bytes,
            created_at,
            updated_at,
        };
        record.validate()?;
        Ok(record)
    }

    /// Validate paths, bounds and monotonic record metadata without I/O.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new(
                "transfer_recovery.id",
                "must not be nil",
            ));
        }
        if self
            .connection_id
            .is_some_and(|connection_id| connection_id.is_nil())
        {
            return Err(ValidationError::new(
                "transfer_recovery.connection_id",
                "must not be nil",
            ));
        }
        let Some(local) = self.local_path.to_str() else {
            return Err(ValidationError::new(
                "transfer_recovery.local_path",
                "must be valid Unicode",
            ));
        };
        validate_path_text("transfer_recovery.local_path", local, false)?;
        if self
            .local_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ValidationError::new(
                "transfer_recovery.local_path",
                "must not contain parent traversal",
            ));
        }
        validate_path_text("transfer_recovery.remote_path", &self.remote_path, false)?;
        if !self.remote_path.starts_with('/')
            || self
                .remote_path
                .split('/')
                .any(|part| part == ".." || part == ".")
        {
            return Err(ValidationError::new(
                "transfer_recovery.remote_path",
                "must be an absolute path without traversal",
            ));
        }
        if let Some(total) = self.total_bytes {
            if self.transferred_bytes > total {
                return Err(ValidationError::new(
                    "transfer_recovery.transferred_bytes",
                    "must not exceed the total byte count",
                ));
            }
            if self.reviewed_existing_bytes > total {
                return Err(ValidationError::new(
                    "transfer_recovery.reviewed_existing_bytes",
                    "must not exceed the total byte count",
                ));
            }
        }
        if self.created_at > self.updated_at {
            return Err(ValidationError::new(
                "transfer_recovery.updated_at",
                "must not precede created_at",
            ));
        }
        Ok(())
    }
}

/// Separate atomic storage for transfer-recovery metadata.
///
/// Every public method performs blocking filesystem I/O and must run off the
/// GPUI event/render thread. The store uses an owner-only JSON file, a sibling
/// journal lock and per-record claim locks. `upsert` and `remove` read the
/// latest file while holding both relevant locks, so multiple KeelShell
/// windows do not overwrite or concurrently recover the same record.
pub struct TransferRecoveryStore {
    path: Option<PathBuf>,
    synchronization: Mutex<()>,
}

/// An ephemeral exclusive claim for one persisted recovery record.
///
/// The claim is held by an OS file lock rather than written into the recovery
/// JSON. It therefore coordinates windows and processes without turning the
/// journal into a source of runtime authority. Dropping the claim releases it.
pub struct TransferRecoveryClaim {
    path: Option<PathBuf>,
    id: Uuid,
    _lock: Option<File>,
}

impl TransferRecoveryClaim {
    /// Return the record identifier protected by this ephemeral claim.
    pub fn id(&self) -> Uuid {
        self.id
    }
}

impl TransferRecoveryStore {
    /// Create a durable store at an explicit path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
            synchronization: Mutex::new(()),
        }
    }

    /// Create a store that intentionally does not persist anything.
    ///
    /// The application uses this only for isolated component fixtures that do
    /// not supply an application data directory. Production workspaces always
    /// use [`Self::from_state_path`].
    pub fn disabled() -> Self {
        Self {
            path: None,
            synchronization: Mutex::new(()),
        }
    }

    /// Resolve the journal beside a normal `state.json` path.
    pub fn from_state_path(state_path: impl AsRef<Path>) -> Self {
        Self::new(state_path.as_ref().with_file_name("transfer-recovery.json"))
    }

    /// Resolve the journal in the platform's per-user KeelShell directory.
    pub fn default_path() -> Result<PathBuf, Error> {
        ProjectDirs::from("dev", "KeelShell", "KeelShell")
            .map(|dirs| dirs.config_dir().join("transfer-recovery.json"))
            .ok_or(Error::NoConfigDirectory)
    }

    /// Return the configured path, or `None` for a disabled fixture store.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Load all records, preserving a malformed file as an error.
    pub fn load(&self) -> Result<Vec<TransferRecoveryRecord>, Error> {
        let Some(path) = self.path.as_deref() else {
            return Ok(Vec::new());
        };
        let _guard = self.synchronization.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file(path)?;
        read_records(path)
    }

    /// Claim one existing record for a single recovery review or transfer.
    ///
    /// The claim is exclusive across cooperating processes and remains held
    /// until the returned value is dropped. It does not persist a session,
    /// credential, review token or transfer worker.
    pub fn claim(&self, id: Uuid) -> Result<TransferRecoveryClaim, Error> {
        let Some(path) = self.path.as_deref() else {
            return Ok(TransferRecoveryClaim {
                path: None,
                id,
                _lock: None,
            });
        };
        let _guard = self.synchronization.lock().map_err(|_| Error::Poisoned)?;
        let lock = self.open_claim_lock(path, id)?;
        let records = read_records(path)?;
        if !records.iter().any(|record| record.id == id) {
            return Err(Error::TransferRecoveryNotFound);
        }
        Ok(TransferRecoveryClaim {
            path: Some(path.to_path_buf()),
            id,
            _lock: Some(lock),
        })
    }

    /// Atomically add or replace one record.
    pub fn upsert(&self, record: TransferRecoveryRecord) -> Result<(), Error> {
        record.validate()?;
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let _guard = self.synchronization.lock().map_err(|_| Error::Poisoned)?;
        let _claim_lock = self.open_claim_lock(path, record.id)?;
        let _lock = self.lock_file(path)?;
        upsert_record(path, record)
    }

    /// Atomically update a record while holding its exclusive claim.
    ///
    /// Recovery workers use this method after a record has been inspected and
    /// approved. The claim prevents another process from starting or
    /// discarding the same recovery while these checkpoints are written.
    pub fn upsert_with_claim(
        &self,
        claim: &TransferRecoveryClaim,
        record: TransferRecoveryRecord,
    ) -> Result<(), Error> {
        if claim.path != self.path || claim.id.is_nil() || claim.id != record.id {
            return Err(Error::TransferRecoveryClaimMismatch);
        }
        record.validate()?;
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let _guard = self.synchronization.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file(path)?;
        upsert_record(path, record)
    }

    /// Remove one record after an explicit successful recovery or discard.
    ///
    /// Removing a missing record is idempotent because a crash may have
    /// completed a cleanup in another process before this call.
    pub fn remove(&self, id: Uuid) -> Result<(), Error> {
        match self.claim(id) {
            Ok(claim) => self.remove_with_claim(&claim),
            Err(Error::TransferRecoveryNotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Remove a record while holding its exclusive claim.
    ///
    /// This is used by the worker that owns a successful reviewed recovery.
    /// Other windows cannot discard or replace the same record while the
    /// claim remains alive.
    pub fn remove_with_claim(&self, claim: &TransferRecoveryClaim) -> Result<(), Error> {
        if claim.path != self.path || claim.id.is_nil() {
            return Err(Error::TransferRecoveryClaimMismatch);
        }
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let _guard = self.synchronization.lock().map_err(|_| Error::Poisoned)?;
        let _lock = self.lock_file(path)?;
        let mut records = read_records(path)?;
        let original_len = records.len();
        records.retain(|record| record.id != claim.id);
        if records.len() == original_len {
            return Ok(());
        }
        write_records(path, &records)
    }

    fn open_claim_lock(&self, path: &Path, id: Uuid) -> Result<File, Error> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(Error::UnsafePath)?;
        prepare_directory(parent)?;
        let file_name = path.file_name().ok_or(Error::UnsafePath)?;
        let mut claim_name = file_name.to_os_string();
        claim_name.push(format!(".claim-{id}.lock"));
        let claim_path = parent.join(claim_name);
        regular_metadata(&claim_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(claim_path)?;
        check_owner_permissions(&file.metadata()?)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::Busy,
            fs::TryLockError::Error(error) => Error::Io(error),
        })?;
        Ok(file)
    }

    fn lock_file(&self, path: &Path) -> Result<File, Error> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let parent = parent.ok_or(Error::UnsafePath)?;
        prepare_directory(parent)?;
        let file_name = path.file_name().ok_or(Error::UnsafePath)?;
        let mut lock_name = file_name.to_os_string();
        lock_name.push(".lock");
        let lock_path = parent.join(lock_name);
        regular_metadata(&lock_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(lock_path)?;
        check_owner_permissions(&file.metadata()?)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::Busy,
            fs::TryLockError::Error(error) => Error::Io(error),
        })?;
        Ok(file)
    }
}

fn upsert_record(path: &Path, mut record: TransferRecoveryRecord) -> Result<(), Error> {
    let mut records = read_records(path)?;
    if let Some(index) = records.iter().position(|item| item.id == record.id) {
        // The first durable write owns the creation timestamp. A later
        // terminal result must not make a long-lived record look new.
        record.created_at = records[index].created_at;
        record.validate()?;
        records[index] = record;
    } else {
        if records.len() >= MAX_TRANSFER_RECOVERY_RECORDS {
            return Err(Error::TransferRecoveryLimit);
        }
        records.push(record);
    }
    validate_records(&records)?;
    write_records(path, &records)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferRecoveryDocument {
    schema_version: u32,
    records: Vec<TransferRecoveryRecord>,
}

fn read_records(path: &Path) -> Result<Vec<TransferRecoveryRecord>, Error> {
    let Some(bytes) = read_bounded(path)? else {
        return Ok(Vec::new());
    };
    let document: TransferRecoveryDocument = serde_json::from_slice(&bytes)?;
    if document.schema_version != TRANSFER_RECOVERY_SCHEMA_VERSION {
        return Err(Error::UnsupportedTransferRecoverySchema {
            found: document.schema_version,
            expected: TRANSFER_RECOVERY_SCHEMA_VERSION,
        });
    }
    validate_records(&document.records)?;
    Ok(document.records)
}

fn validate_records(records: &[TransferRecoveryRecord]) -> Result<(), Error> {
    if records.len() > MAX_TRANSFER_RECOVERY_RECORDS {
        return Err(Error::TransferRecoveryLimit);
    }
    let mut ids = HashSet::new();
    for record in records {
        record.validate()?;
        if !ids.insert(record.id) {
            return Err(ValidationError::new("transfer_recovery.id", "must be unique").into());
        }
    }
    Ok(())
}

fn write_records(path: &Path, records: &[TransferRecoveryRecord]) -> Result<(), Error> {
    let document = TransferRecoveryDocument {
        schema_version: TRANSFER_RECOVERY_SCHEMA_VERSION,
        records: records.to_vec(),
    };
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_TRANSFER_RECOVERY_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = parent.ok_or(Error::UnsafePath)?;
    prepare_directory(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".keelshell-transfer-recovery-")
        .tempfile_in(parent)?;
    set_owner_permissions(temporary.as_file())?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| Error::Io(error.error))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(Error::Durability)?;
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let Some(metadata) = regular_metadata(path)? else {
        return Ok(None);
    };
    check_owner_permissions(&metadata)?;
    if metadata.len() > MAX_TRANSFER_RECOVERY_DOCUMENT_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_TRANSFER_RECOVERY_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TRANSFER_RECOVERY_DOCUMENT_BYTES {
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

fn prepare_directory(path: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

fn validate_path_text(
    field: &'static str,
    value: &str,
    allow_empty: bool,
) -> Result<(), ValidationError> {
    if (!allow_empty && value.is_empty())
        || value.len() > MAX_TRANSFER_RECOVERY_PATH_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ValidationError::new(
            field,
            "must be nonempty, bounded and free of control characters",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use tempfile::TempDir;

    fn record(id: Uuid, state: TransferRecoveryState) -> TransferRecoveryRecord {
        TransferRecoveryRecord::new(
            id,
            TransferRecoveryDirection::Upload,
            "/tmp/source.txt",
            "/srv/partial.txt",
            false,
            state,
            4,
            Some(8),
            0,
            10,
            10,
        )
        .expect("valid recovery record")
    }

    #[test]
    fn record_rejects_progress_above_total_and_nil_connection_identity() {
        let id = Uuid::new_v4();
        assert!(
            TransferRecoveryRecord::new_with_connection_id(
                id,
                Some(Uuid::nil()),
                TransferRecoveryDirection::Upload,
                "/tmp/source.txt",
                "/srv/partial.txt",
                false,
                TransferRecoveryState::Unfinished,
                4,
                Some(8),
                0,
                10,
                10,
            )
            .is_err()
        );
        assert!(
            TransferRecoveryRecord::new(
                id,
                TransferRecoveryDirection::Upload,
                "/tmp/source.txt",
                "/srv/partial.txt",
                false,
                TransferRecoveryState::Unfinished,
                9,
                Some(8),
                0,
                10,
                10,
            )
            .is_err()
        );
    }

    fn write_private(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).expect("write recovery document");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("set private recovery permissions");
        }
    }

    #[test]
    fn record_rejects_secrets_in_path_shape_and_parent_traversal() {
        let mut value = record(Uuid::new_v4(), TransferRecoveryState::Unfinished);
        value.remote_path = "/srv/\nsecret".into();
        assert!(value.validate().is_err());
        value.remote_path = "/srv/../secret".into();
        assert!(value.validate().is_err());
        value.remote_path = "/srv/secret".into();
        value.local_path = PathBuf::from("/tmp/../secret");
        assert!(value.validate().is_err());
    }

    #[test]
    fn store_round_trips_records_without_runtime_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let store = TransferRecoveryStore::new(directory.path().join("transfer-recovery.json"));
        let id = Uuid::new_v4();
        store
            .upsert(record(id, TransferRecoveryState::OutcomeUnknown))
            .expect("persist recovery record");
        let loaded = TransferRecoveryStore::new(directory.path().join("transfer-recovery.json"))
            .load()
            .expect("reload recovery record");
        assert_eq!(
            loaded,
            vec![record(id, TransferRecoveryState::OutcomeUnknown)]
        );
        let bytes = std::fs::read(directory.path().join("transfer-recovery.json"))
            .expect("read recovery document");
        let text = String::from_utf8(bytes).expect("recovery JSON is UTF-8");
        for forbidden in [
            "session_token",
            "review_token",
            "credential",
            "secret",
            "worker",
        ] {
            assert!(
                !text.contains(forbidden),
                "{forbidden} leaked into metadata"
            );
        }
    }

    #[test]
    fn claim_is_exclusive_and_remove_requires_the_claim() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("transfer-recovery.json");
        let first = TransferRecoveryStore::new(&path);
        let second = TransferRecoveryStore::new(&path);
        let id = Uuid::new_v4();
        first
            .upsert(record(id, TransferRecoveryState::Unfinished))
            .expect("persist recovery record");

        let claim = first.claim(id).expect("claim recovery record");
        assert!(matches!(second.claim(id), Err(Error::Busy)));
        assert!(matches!(
            second.upsert(record(id, TransferRecoveryState::Failed)),
            Err(Error::Busy)
        ));
        assert!(matches!(second.remove(id), Err(Error::Busy)));
        first
            .remove_with_claim(&claim)
            .expect("remove with owned claim");
        drop(claim);
        assert!(matches!(
            second.claim(id),
            Err(Error::TransferRecoveryNotFound)
        ));
    }

    #[test]
    fn claimed_upsert_rejects_a_different_record_id_or_journal() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("transfer-recovery.json");
        let first = TransferRecoveryStore::new(&path);
        let second = TransferRecoveryStore::new(directory.path().join("other.json"));
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        first
            .upsert(record(first_id, TransferRecoveryState::Unfinished))
            .expect("persist first record");
        first
            .upsert(record(second_id, TransferRecoveryState::Failed))
            .expect("persist second record");

        let claim = first.claim(first_id).expect("claim first record");
        assert!(matches!(
            first.upsert_with_claim(&claim, record(second_id, TransferRecoveryState::Failed)),
            Err(Error::TransferRecoveryClaimMismatch)
        ));
        assert!(matches!(
            second.upsert_with_claim(&claim, record(first_id, TransferRecoveryState::Failed)),
            Err(Error::TransferRecoveryClaimMismatch)
        ));
    }

    #[test]
    fn upsert_and_remove_preserve_other_records_across_store_instances() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("transfer-recovery.json");
        let first = TransferRecoveryStore::new(&path);
        let second = TransferRecoveryStore::new(&path);
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        first
            .upsert(record(first_id, TransferRecoveryState::Unfinished))
            .expect("persist first record");
        second
            .upsert(record(second_id, TransferRecoveryState::Failed))
            .expect("persist second record");
        first.remove(first_id).expect("remove first record");
        assert_eq!(
            second.load().expect("load remaining record"),
            vec![record(second_id, TransferRecoveryState::Failed)]
        );
        second.remove(Uuid::new_v4()).expect("idempotent removal");
    }

    #[test]
    fn malformed_or_unknown_documents_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("transfer-recovery.json");
        write_private(&path, br#"{"schema_version":99,"records":[]}"#);
        let error = TransferRecoveryStore::new(&path)
            .load()
            .expect_err("unsupported schema must fail");
        assert!(matches!(
            error,
            Error::UnsupportedTransferRecoverySchema { found: 99, .. }
        ));
        write_private(&path, br#"{"schema_version":1,"records":[],"extra":true}"#);
        assert!(matches!(
            TransferRecoveryStore::new(path).load(),
            Err(Error::Json(_))
        ));
    }
}
