use thiserror::Error;

/// A domain validation failure, deliberately excluding user-provided values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid {field}: {reason}")]
pub struct ValidationError {
    /// Stable field name suitable for attaching a UI error to an input.
    pub field: &'static str,
    /// Human-readable constraint without the potentially sensitive input value.
    pub reason: &'static str,
}

impl ValidationError {
    pub(crate) const fn new(field: &'static str, reason: &'static str) -> Self {
        Self { field, reason }
    }
}

/// A recoverable persistence, import or domain error.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O operation failed before a successful atomic state replacement.
    #[error("state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// Input JSON could not be decoded or output JSON could not be encoded.
    #[error("invalid state document: {0}")]
    Json(#[from] serde_json::Error),
    /// A model failed its domain constraints.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// This program cannot safely interpret the document's schema.
    #[error("unsupported state schema {found}; expected {expected}")]
    UnsupportedSchema {
        /// Version found in the document.
        found: u32,
        /// Version understood by this program.
        expected: u32,
    },
    /// A bounded input was too large to process.
    #[error("state document exceeds the 4 MiB limit")]
    TooLarge,
    /// The snapshot revision is stale or disk changed; reload and merge.
    #[error("state snapshot is stale or disk changed; reload and merge before saving")]
    Conflict,
    /// An existing file cannot be replaced without successfully loading it first.
    #[error("load the existing state successfully before saving")]
    NotLoaded,
    /// Another cooperating process is currently reading or saving this state.
    #[error("state is busy in another process; retry the operation")]
    Busy,
    /// A symlink, non-regular file, or unsuitable parent was encountered.
    #[error("state location must use a regular file and directory, not a symlink")]
    UnsafePath,
    /// A Unix state or lock file grants access to other users.
    #[error("state file permissions must be owner-only (0600)")]
    InsecurePermissions,
    /// The operating system did not supply a per-user configuration directory.
    #[error("no per-user configuration directory is available")]
    NoConfigDirectory,
    /// An earlier panic poisoned this store's synchronization state.
    #[error("state store synchronization failed; reopen the store")]
    Poisoned,
    /// The requested identifier does not exist.
    #[error("connection was not found")]
    ConnectionNotFound,
    /// Atomic replacement succeeded but syncing its directory failed.
    #[error("state was replaced, but directory durability could not be confirmed: {0}")]
    Durability(std::io::Error),
    /// The chosen configuration backup disappeared or is not owned by this store.
    #[error("configuration backup was not found")]
    ConfigBackupNotFound,
    /// The local backup wrapper version or sequence is invalid.
    #[error("configuration backup format is unsupported or invalid")]
    ConfigBackupFormat,
    /// A backup changed after review or the current state/revision changed.
    #[error("configuration recovery review is stale; inspect again")]
    ConfigRecoveryConflict,
    /// The bounded backup directory contains too many or unrecognized entries.
    #[error(
        "configuration backup directory exceeds its bounded limit or contains unexpected entries"
    )]
    ConfigBackupLimit,
    /// Preserved originals are never automatically pruned; archive them explicitly first.
    #[error(
        "preserved configuration originals reached the eight-file limit; archive them before recovery"
    )]
    ConfigOriginalLimit,
    /// Replacement failed after commit, but the prior state (including absence) was restored.
    #[error("configuration recovery failed; the original state was restored: {0}")]
    ConfigRecoveryRolledBack(std::io::Error),
    /// A post-commit failure could not confirm rollback; any existing original remains preserved.
    #[error(
        "configuration recovery needs manual repair; an original copy was retained if the reviewed file existed"
    )]
    ConfigRecoveryRequired,
    /// The vault passphrase is empty or exceeds the bounded input limit.
    #[error("vault passphrase is invalid")]
    VaultInvalidPassphrase,
    /// A vault secret is empty, contains NUL or exceeds the bounded input limit.
    #[error("vault secret is invalid")]
    VaultInvalidSecret,
    /// The encrypted vault document is malformed or fails validation.
    #[error("encrypted vault document is invalid")]
    VaultCorrupt,
    /// The vault uses a KDF configuration this build does not understand.
    #[error("encrypted vault KDF is unsupported")]
    VaultUnsupportedKdf,
    /// The supplied passphrase cannot authenticate the complete encrypted vault.
    #[error("vault unlock failed")]
    VaultUnlockFailed,
    /// An OpenSSH-style configuration could not be imported safely.
    #[error("invalid SSH configuration at line {line}: {reason}")]
    OpenSshConfig {
        /// One-based source line, or zero when the error is not tied to a line.
        line: usize,
        /// Stable reason that is safe to display without echoing configuration values.
        reason: &'static str,
    },
    /// The loaded vault is stale, belongs to another store, or disk changed.
    #[error("vault snapshot is stale or disk changed; reload before saving")]
    VaultConflict,
    /// An existing vault must be authenticated by this store before replacement.
    #[error("unlock the existing vault successfully before saving")]
    VaultNotLoaded,
    /// A cryptographic primitive failed before authentication could complete.
    #[error("vault cryptographic operation failed")]
    VaultCrypto,
    /// The operating system random source could not produce vault nonces.
    #[error("vault randomness source failed")]
    VaultRandomness,
    /// A vault reference or profile identity was invalid.
    #[error("vault entry is invalid")]
    VaultInvalidEntry,
    /// The encrypted vault has reached its bounded entry limit.
    #[error("encrypted vault has reached its entry limit")]
    VaultLimit,
    /// The requested vault reference does not exist.
    #[error("vault entry was not found")]
    VaultEntryNotFound,
    /// A vault reference belongs to a different profile or credential role.
    #[error("vault entry does not match this profile or credential role")]
    VaultEntryMismatch,
}
