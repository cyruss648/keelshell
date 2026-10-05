//! Cross-platform encrypted credential vault, separate from application state.
//!
//! Profiles keep opaque references. Each secret and the complete ordered entry
//! manifest are authenticated, including an empty vault. Unlock authenticates the
//! manifest before exposing a mutable vault. The master passphrase is never stored.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{Error, SnapshotRevision, model::MAX_DOCUMENT_BYTES};

const VAULT_SCHEMA_VERSION: u32 = 2;
const KEY_BYTES: usize = 32;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const ARGON_MEMORY_KIB: u32 = 64 * 1024;
const ARGON_ITERATIONS: u32 = 3;
const ARGON_LANES: u32 = 1;
const MAX_ENTRIES: usize = 10_000;
const MAX_SECRET_BYTES: usize = 1024 * 1024;
const MAX_PASSPHRASE_BYTES: usize = 4096;
const MANIFEST_DOMAIN: &[u8] = b"keelshell-vault-manifest-v2\0";
const ENTRY_DOMAIN: &[u8] = b"keelshell-vault-entry-v2\0";

/// The credential role determines which authentication field may consume it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    /// A password used by password authentication.
    Password,
    /// A passphrase used to unlock a private key file.
    PrivateKeyPassphrase,
    /// An API key owned by a named AI profile.
    AiApiKey,
    /// Header or proxy secret with a purpose-bound encrypted payload.
    AiRequestSecret,
}

impl CredentialKind {
    fn code(self) -> u8 {
        match self {
            Self::Password => 1,
            Self::PrivateKeyPassphrase => 2,
            Self::AiApiKey => 3,
            Self::AiRequestSecret => 4,
        }
    }
}

/// Authenticated entry identity and binding, without exposing its secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialMetadata {
    /// The opaque reference stored by the credential's owner.
    pub reference: Uuid,
    /// The connection or named AI profile that owns this credential.
    pub owner_id: Uuid,
    /// The only authentication role that may consume this credential.
    pub kind: CredentialKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultEnvelope {
    manifest: VaultManifest,
    nonce: String,
    authentication: String,
}

#[derive(Serialize)]
struct EncodedEnvelope<'a> {
    manifest: &'a VaultManifest,
    nonce: &'a str,
    authentication: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultManifest {
    schema_version: u32,
    kdf: KdfDocument,
    entries: Vec<EncryptedEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KdfDocument {
    algorithm: String,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    salt: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncryptedEntry {
    id: Uuid,
    connection_id: Uuid,
    kind: CredentialKind,
    nonce: String,
    ciphertext: String,
}

/// An authenticated, unlocked in-memory credential vault.
///
/// Only encrypted entries are retained. The derived key and temporary plaintext
/// allocations are zeroized on drop; [`Self::get`] returns a zeroizing string.
/// Callers must likewise protect their borrowed passphrase and secret inputs.
/// `Debug` reports only an entry count. Unlocking an unauthenticated schema 1
/// document is deliberately unsupported; it cannot prove even an empty vault's key.
pub struct CredentialVault {
    key: Zeroizing<[u8; KEY_BYTES]>,
    manifest: VaultManifest,
    snapshot: SnapshotRevision,
}

impl CredentialVault {
    /// Create an empty vault with a fresh random salt.
    pub fn create(passphrase: &str) -> Result<Self, Error> {
        validate_passphrase(passphrase)?;
        let mut salt = [0_u8; SALT_BYTES];
        fill_random(&mut salt)?;
        let kdf = KdfDocument {
            algorithm: "argon2id".to_owned(),
            memory_kib: ARGON_MEMORY_KIB,
            iterations: ARGON_ITERATIONS,
            lanes: ARGON_LANES,
            salt: BASE64.encode(salt),
        };
        let key = derive_key(passphrase, &salt, &kdf)?;
        Ok(Self {
            key,
            manifest: VaultManifest {
                schema_version: VAULT_SCHEMA_VERSION,
                kdf,
                entries: Vec::new(),
            },
            snapshot: SnapshotRevision::default(),
        })
    }

    /// Authenticate the entire serialized vault before returning an unlocked value.
    ///
    /// Incorrect passwords and any changes to the ordered manifest fail here,
    /// including entry insertion, deletion, replacement and reordering. JSON
    /// formatting and object field order are immaterial. KDF parameters are checked
    /// before allocating work memory; arbitrary KDF costs are never accepted.
    pub fn unlock(bytes: &[u8], passphrase: &str) -> Result<Self, Error> {
        validate_passphrase(passphrase)?;
        let envelope = decode_envelope(bytes)?;
        let salt = decode_fixed::<SALT_BYTES>(&envelope.manifest.kdf.salt)?;
        let key = derive_key(passphrase, &salt, &envelope.manifest.kdf)?;
        let nonce = decode_fixed::<NONCE_BYTES>(&envelope.nonce)?;
        let authentication = decode_fixed::<TAG_BYTES>(&envelope.authentication)?;
        decrypt(
            &key,
            &nonce,
            &manifest_aad(&envelope.manifest)?,
            &authentication,
        )?;
        Ok(Self {
            key,
            manifest: envelope.manifest,
            snapshot: SnapshotRevision::default(),
        })
    }

    /// Return whether this vault contains no credential entries.
    pub fn is_empty(&self) -> bool {
        self.manifest.entries.is_empty()
    }

    /// Return the number of authenticated encrypted entries.
    pub fn len(&self) -> usize {
        self.manifest.entries.len()
    }

    /// Iterate over authenticated entry bindings without decrypting credentials.
    ///
    /// Entries follow manifest order. Loaded metadata is available only after
    /// [`Self::unlock`] has authenticated the complete manifest.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = CredentialMetadata> + '_ {
        self.manifest
            .entries
            .iter()
            .map(|entry| CredentialMetadata {
                reference: entry.id,
                owner_id: entry.connection_id,
                kind: entry.kind,
            })
    }

    /// Re-encrypt every credential under a fresh salt, key and entry nonce.
    ///
    /// References, owner bindings, roles and manifest order are preserved. All
    /// work is staged: validation, decryption or randomness failure leaves this
    /// vault unchanged. The original save snapshot is retained so a subsequent
    /// [`VaultStore::save`] can replace only the authenticated file loaded earlier.
    /// This method changes memory only; callers must save to persist the rotation.
    pub fn rotate_passphrase(&mut self, new_passphrase: &str) -> Result<(), Error> {
        let mut rotated = Self::create(new_passphrase)?;
        rotated.manifest.entries.reserve(self.len());
        for entry in &self.manifest.entries {
            let secret = decrypt_secret(&self.key, entry)?;
            let nonce = random_nonce()?;
            let ciphertext = encrypt(
                &rotated.key,
                &nonce,
                &entry_aad(entry.id, entry.connection_id, entry.kind),
                secret.as_bytes(),
            )?;
            rotated.manifest.entries.push(EncryptedEntry {
                id: entry.id,
                connection_id: entry.connection_id,
                kind: entry.kind,
                nonce: BASE64.encode(nonce),
                ciphertext: BASE64.encode(ciphertext.as_slice()),
            });
        }
        check_document_size(&rotated.manifest)?;
        rotated.snapshot = self.snapshot;
        *self = rotated;
        Ok(())
    }

    /// Add or replace a credential, keeping its profile and role binding immutable.
    ///
    /// Reusing `reference` may update its secret, but cannot retarget an existing
    /// entry to another profile or authentication role. Plaintext is encrypted
    /// before this method returns and is not retained in the vault. An entry
    /// exceeding the aggregate document budget fails without changing the vault.
    pub fn set(
        &mut self,
        reference: Uuid,
        connection_id: Uuid,
        kind: CredentialKind,
        secret: &str,
    ) -> Result<(), Error> {
        if reference.is_nil() || connection_id.is_nil() {
            return Err(Error::VaultInvalidEntry);
        }
        validate_secret(secret)?;
        let index = self
            .manifest
            .entries
            .iter()
            .position(|entry| entry.id == reference);
        if let Some(index) = index {
            let existing = &self.manifest.entries[index];
            if existing.connection_id != connection_id || existing.kind != kind {
                return Err(Error::VaultEntryMismatch);
            }
        } else if self.len() >= MAX_ENTRIES {
            return Err(Error::VaultLimit);
        }
        let nonce = random_nonce()?;
        let ciphertext = encrypt(
            &self.key,
            &nonce,
            &entry_aad(reference, connection_id, kind),
            secret.as_bytes(),
        )?;
        let entry = EncryptedEntry {
            id: reference,
            connection_id,
            kind,
            nonce: BASE64.encode(nonce),
            ciphertext: BASE64.encode(ciphertext.as_slice()),
        };
        let previous = match index {
            Some(index) => Some(std::mem::replace(&mut self.manifest.entries[index], entry)),
            None => {
                self.manifest.entries.push(entry);
                None
            }
        };
        // Count encoded bytes without allocating a document. Roll back both
        // insertions and replacements before returning an aggregate-limit error.
        if let Err(error) = check_document_size(&self.manifest) {
            if let (Some(index), Some(previous)) = (index, previous) {
                self.manifest.entries[index] = previous;
            } else {
                self.manifest.entries.pop();
            }
            return Err(error);
        }
        Ok(())
    }

    /// Return a decrypted credential after checking its profile and role.
    pub fn get(
        &self,
        reference: Uuid,
        connection_id: Uuid,
        kind: CredentialKind,
    ) -> Result<Zeroizing<String>, Error> {
        let entry = self
            .manifest
            .entries
            .iter()
            .find(|entry| entry.id == reference)
            .ok_or(Error::VaultEntryNotFound)?;
        if entry.connection_id != connection_id || entry.kind != kind {
            return Err(Error::VaultEntryMismatch);
        }
        decrypt_secret(&self.key, entry)
    }

    /// Remove an entry. Returns `true` when an entry was removed.
    pub fn remove(&mut self, reference: Uuid) -> bool {
        let before = self.len();
        self.manifest.entries.retain(|entry| entry.id != reference);
        before != self.len()
    }

    /// Serialize a freshly authenticated manifest containing encrypted entries.
    ///
    /// A fresh nonce authenticates even an empty manifest. The output never
    /// includes plaintext; its size is bounded before it can be persisted.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        check_document_size(&self.manifest)?;
        let nonce = random_nonce()?;
        let authentication = encrypt(&self.key, &nonce, &manifest_aad(&self.manifest)?, &[])?;
        let mut bytes = serde_json::to_vec_pretty(&EncodedEnvelope {
            manifest: &self.manifest,
            nonce: &BASE64.encode(nonce),
            authentication: &BASE64.encode(authentication.as_slice()),
        })?;
        bytes.push(b'\n');
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(bytes)
    }
}

impl std::fmt::Debug for CredentialVault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialVault")
            .field("entries", &self.len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
enum Observed {
    #[default]
    Unloaded,
    Loaded {
        bytes: Option<Vec<u8>>,
        revision: SnapshotRevision,
    },
}

/// An atomically replaced vault with cooperative locks and lost-update detection.
///
/// Keep this store between load and save. Existing files require a successful
/// authenticated load; every save compares its bytes and the vault snapshot under
/// the file lock. Separate loaded snapshots cannot silently overwrite each other.
///
/// Blocking methods belong on a worker thread. Unix files are owner-only; Windows
/// inherits its per-user directory ACL. The parent directory must be trusted:
/// cooperative locks do not defend against a malicious same-user process changing
/// path components concurrently. No OS keychain or anti-rollback service is used.
/// Restoring a complete old, authentic file cannot be detected after restarting.
pub struct VaultStore {
    path: PathBuf,
    observed: Mutex<Observed>,
}

impl VaultStore {
    /// Construct a store at an explicit path without touching the filesystem.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            observed: Mutex::new(Observed::Unloaded),
        }
    }

    /// Resolve `vault.json` in the per-user KeelShell configuration directory.
    pub fn default_path() -> Result<PathBuf, Error> {
        ProjectDirs::from("dev", "KeelShell", "KeelShell")
            .map(|dirs| dirs.config_dir().join("vault.json"))
            .ok_or(Error::NoConfigDirectory)
    }

    /// Return the configured vault path for diagnostics or an open-folder action.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Authenticate a vault, or create it only in memory when the file is absent.
    ///
    /// A failed load discards this store's previous save baseline, so it cannot
    /// later overwrite a file that failed authentication or path validation.
    pub fn load(&self, passphrase: &str) -> Result<CredentialVault, Error> {
        self.load_with_creation(passphrase, true)
    }

    /// Authenticate an existing vault without creating an empty replacement.
    ///
    /// A missing file, including one removed before the locked read, returns
    /// [`Error::Io`] with [`std::io::ErrorKind::NotFound`]. Every failed load
    /// discards this store's previous save baseline. An initially missing path
    /// does not cause its parent directory or a lock file to be created.
    pub fn load_existing(&self, passphrase: &str) -> Result<CredentialVault, Error> {
        self.load_with_creation(passphrase, false)
    }

    fn load_with_creation(
        &self,
        passphrase: &str,
        create_if_missing: bool,
    ) -> Result<CredentialVault, Error> {
        let mut observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let loaded = (|| {
            self.parent()?;
            let bytes = if regular_metadata(&self.path)?.is_some() {
                let _lock = self.lock_file()?;
                read_document(&self.path)?
            } else {
                None
            };
            let vault = match &bytes {
                Some(bytes) => CredentialVault::unlock(bytes, passphrase)?,
                None if create_if_missing => CredentialVault::create(passphrase)?,
                None => {
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "credential vault does not exist",
                    )));
                }
            };
            Ok::<_, Error>((vault, bytes))
        })();
        let (mut vault, bytes) = match loaded {
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
        vault.snapshot = revision;
        *observed = Observed::Loaded { bytes, revision };
        Ok(vault)
    }

    /// Atomically save this store's current snapshot, then advance its revision.
    ///
    /// An existing file cannot be replaced without loading it successfully first.
    /// On [`Error::VaultConflict`], explicitly reload and reconcile the changes.
    /// The passed vault receives its new snapshot after replacement. A directory
    /// sync failure reports [`Error::Durability`]; replacement already happened.
    pub fn save(&self, vault: &mut CredentialVault) -> Result<(), Error> {
        let bytes = vault.to_bytes()?;
        let mut observed = self.observed.lock().map_err(|_| Error::Poisoned)?;
        let parent = self.parent()?;
        prepare_directory(parent)?;
        let _lock = self.lock_file()?;
        let current = read_document(&self.path)?;
        if let Some(current) = &current {
            decode_envelope(current)?;
        }
        match &*observed {
            Observed::Unloaded if current.is_some() => return Err(Error::VaultNotLoaded),
            Observed::Unloaded if vault.snapshot.is_loaded() => return Err(Error::VaultConflict),
            Observed::Loaded {
                bytes: previous,
                revision,
            } if previous != &current || vault.snapshot != *revision => {
                return Err(Error::VaultConflict);
            }
            _ => {}
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".keelshell-vault-")
            .tempfile_in(parent)?;
        set_owner_permissions(temporary.as_file())?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| Error::Io(error.error))?;
        vault.snapshot = SnapshotRevision::fresh();
        *observed = Observed::Loaded {
            bytes: Some(bytes),
            revision: vault.snapshot,
        };
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(Error::Durability)?;
        Ok(())
    }

    fn parent(&self) -> Result<&Path, Error> {
        if self.path.file_name().is_none() {
            return Err(Error::UnsafePath);
        }
        Ok(self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")))
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
        // Keep this inode permanently; unlinking it permits two independent locks.
        Ok(file)
    }
}

fn derive_key(
    passphrase: &str,
    salt: &[u8; SALT_BYTES],
    kdf: &KdfDocument,
) -> Result<Zeroizing<[u8; KEY_BYTES]>, Error> {
    validate_kdf(kdf)?;
    let params = Params::new(kdf.memory_kib, kdf.iterations, kdf.lanes, Some(KEY_BYTES))
        .map_err(|_| Error::VaultCrypto)?;
    let mut memory = Zeroizing::new(vec![Block::default(); params.block_count()]);
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
    argon
        .hash_password_into_with_memory(
            passphrase.as_bytes(),
            salt,
            key.as_mut(),
            memory.as_mut_slice(),
        )
        .map_err(|_| Error::VaultCrypto)?;
    Ok(key)
}

fn decrypt_secret(
    key: &[u8; KEY_BYTES],
    entry: &EncryptedEntry,
) -> Result<Zeroizing<String>, Error> {
    let nonce = decode_fixed::<NONCE_BYTES>(&entry.nonce)?;
    let ciphertext = decode_bytes(&entry.ciphertext)?;
    let plaintext = decrypt(
        key,
        &nonce,
        &entry_aad(entry.id, entry.connection_id, entry.kind),
        &ciphertext,
    )?;
    // Validate by borrowing: FromUtf8Error would own an unprotected plaintext
    // Vec on its error path. Both allocations here have zeroizing owners.
    let secret = std::str::from_utf8(&plaintext).map_err(|_| Error::VaultCorrupt)?;
    validate_secret(secret).map_err(|_| Error::VaultCorrupt)?;
    Ok(Zeroizing::new(secret.to_owned()))
}

fn check_document_size(manifest: &VaultManifest) -> Result<(), Error> {
    struct Counter {
        written: usize,
        exceeded: bool,
    }
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_DOCUMENT_BYTES - 1 - self.written {
                self.exceeded = true;
                return Err(std::io::Error::other("vault document limit reached"));
            }
            self.written += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter {
        written: 0,
        exceeded: false,
    };
    // Base64 nonces and tags have fixed ASCII lengths and never need JSON escaping.
    // Count exactly the same pretty envelope that to_bytes will produce, plus LF.
    let result = serde_json::to_writer_pretty(
        &mut counter,
        &EncodedEnvelope {
            manifest,
            nonce: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            authentication: "AAAAAAAAAAAAAAAAAAAAAA==",
        },
    );
    if counter.exceeded {
        return Err(Error::TooLarge);
    }
    result.map_err(Error::Json)
}

fn manifest_aad(manifest: &VaultManifest) -> Result<Vec<u8>, Error> {
    let mut aad = MANIFEST_DOMAIN.to_vec();
    serde_json::to_writer(&mut aad, manifest)?;
    if aad.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(aad)
}

fn entry_aad(reference: Uuid, connection_id: Uuid, kind: CredentialKind) -> Vec<u8> {
    let mut aad = Vec::with_capacity(ENTRY_DOMAIN.len() + 33);
    aad.extend_from_slice(ENTRY_DOMAIN);
    aad.extend_from_slice(reference.as_bytes());
    aad.extend_from_slice(connection_id.as_bytes());
    aad.push(kind.code());
    aad
}

fn encrypt(
    key: &[u8; KEY_BYTES],
    nonce: &[u8; NONCE_BYTES],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::VaultCrypto)?;
    let nonce = XNonce::try_from(nonce.as_slice()).map_err(|_| Error::VaultCrypto)?;
    // Allocate final capacity before copying plaintext, avoiding reallocation of
    // a secret buffer when the tag is appended. Errors also drop a zeroizing owner.
    let mut buffer = Zeroizing::new(Vec::with_capacity(plaintext.len() + TAG_BYTES));
    buffer.extend_from_slice(plaintext);
    cipher
        .encrypt_in_place(&nonce, aad, &mut *buffer)
        .map_err(|_| Error::VaultCrypto)?;
    Ok(buffer)
}

fn decrypt(
    key: &[u8; KEY_BYTES],
    nonce: &[u8; NONCE_BYTES],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::VaultCrypto)?;
    let nonce = XNonce::try_from(nonce.as_slice()).map_err(|_| Error::VaultCrypto)?;
    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    cipher
        .decrypt_in_place(&nonce, aad, &mut *buffer)
        .map_err(|_| Error::VaultUnlockFailed)?;
    Ok(buffer)
}

fn random_nonce() -> Result<[u8; NONCE_BYTES], Error> {
    let mut nonce = [0_u8; NONCE_BYTES];
    fill_random(&mut nonce)?;
    Ok(nonce)
}

fn fill_random(bytes: &mut [u8]) -> Result<(), Error> {
    use rand::{TryRngCore, rngs::OsRng};
    OsRng
        .try_fill_bytes(bytes)
        .map_err(|_| Error::VaultRandomness)
}

fn validate_passphrase(passphrase: &str) -> Result<(), Error> {
    if passphrase.is_empty() || passphrase.len() > MAX_PASSPHRASE_BYTES {
        return Err(Error::VaultInvalidPassphrase);
    }
    Ok(())
}

fn validate_secret(secret: &str) -> Result<(), Error> {
    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES || secret.contains('\0') {
        return Err(Error::VaultInvalidSecret);
    }
    Ok(())
}

fn validate_kdf(kdf: &KdfDocument) -> Result<(), Error> {
    if kdf.algorithm != "argon2id"
        || kdf.memory_kib != ARGON_MEMORY_KIB
        || kdf.iterations != ARGON_ITERATIONS
        || kdf.lanes != ARGON_LANES
    {
        return Err(Error::VaultUnsupportedKdf);
    }
    Ok(())
}

fn decode_envelope(bytes: &[u8]) -> Result<VaultEnvelope, Error> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    let envelope: VaultEnvelope = serde_json::from_slice(bytes).map_err(|_| Error::VaultCorrupt)?;
    let manifest = &envelope.manifest;
    if manifest.schema_version != VAULT_SCHEMA_VERSION || manifest.entries.len() > MAX_ENTRIES {
        return Err(Error::VaultCorrupt);
    }
    validate_kdf(&manifest.kdf)?;
    decode_fixed::<SALT_BYTES>(&manifest.kdf.salt)?;
    decode_fixed::<NONCE_BYTES>(&envelope.nonce)?;
    decode_fixed::<TAG_BYTES>(&envelope.authentication)?;
    let mut ids = BTreeSet::new();
    for entry in &manifest.entries {
        if entry.id.is_nil() || entry.connection_id.is_nil() || !ids.insert(entry.id) {
            return Err(Error::VaultCorrupt);
        }
        decode_fixed::<NONCE_BYTES>(&entry.nonce)?;
        let ciphertext = decode_bytes(&entry.ciphertext)?;
        if ciphertext.len() <= TAG_BYTES || ciphertext.len() > MAX_SECRET_BYTES + TAG_BYTES {
            return Err(Error::VaultCorrupt);
        }
    }
    Ok(envelope)
}

fn decode_fixed<const N: usize>(input: &str) -> Result<[u8; N], Error> {
    let bytes = decode_bytes(input)?;
    bytes.try_into().map_err(|_| Error::VaultCorrupt)
}

fn decode_bytes(input: &str) -> Result<Vec<u8>, Error> {
    BASE64.decode(input).map_err(|_| Error::VaultCorrupt)
}

fn read_document(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let Some(metadata) = regular_metadata(path)? else {
        return Ok(None);
    };
    check_owner_permissions(&metadata)?;
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_DOCUMENT_BYTES as u64 + 1)
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
        Err(error) => Err(Error::Io(error)),
    }
}

fn check_directory(path: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(path)?.file_type().is_dir() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_role_codes_preserve_schema2_entry_authentication() {
        assert_eq!(CredentialKind::Password.code(), 1);
        assert_eq!(CredentialKind::PrivateKeyPassphrase.code(), 2);
        assert_eq!(CredentialKind::AiApiKey.code(), 3);
        assert_eq!(CredentialKind::AiRequestSecret.code(), 4);
    }

    #[test]
    fn failed_later_entry_decryption_cannot_partially_rotate_vault() -> Result<(), Error> {
        let mut vault = CredentialVault::create("original passphrase")?;
        let owner = Uuid::new_v4();
        let first = Uuid::new_v4();
        vault.set(first, owner, CredentialKind::Password, "first-secret")?;
        vault.set(
            Uuid::new_v4(),
            owner,
            CredentialKind::AiApiKey,
            "second-secret",
        )?;
        let mut ciphertext = decode_bytes(&vault.manifest.entries[1].ciphertext)?;
        ciphertext[0] ^= 1;
        vault.manifest.entries[1].ciphertext = BASE64.encode(ciphertext);
        vault.snapshot = SnapshotRevision::fresh();
        let snapshot = vault.snapshot;
        let key = Zeroizing::new(*vault.key);
        let manifest = serde_json::to_vec(&vault.manifest)?;

        assert!(matches!(
            vault.rotate_passphrase("replacement passphrase"),
            Err(Error::VaultUnlockFailed)
        ));
        assert_eq!(vault.snapshot, snapshot);
        assert_eq!(*vault.key, *key);
        assert_eq!(serde_json::to_vec(&vault.manifest)?, manifest);
        assert_eq!(
            vault.get(first, owner, CredentialKind::Password)?.as_str(),
            "first-secret"
        );
        Ok(())
    }
}
