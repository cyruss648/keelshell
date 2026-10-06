//! Explicit two-client connection metadata synchronization over an encrypted directory.
//!
//! This transport expects a user-selected shared or mounted directory with cooperative cross-device locks and atomic replacement.
//! It does not implement cloud accounts or transfer that directory itself. Existing
//! clients remember the authenticated generation/digest and every record version;
//! a fresh client has no external rollback anchor. All operations block and belong
//! on a worker. No operation connects to SSH or reads credentials/key files.

use crate::{
    AppState, AuthMethod, Connection, ConnectionProxy, Error, ReconnectPolicy, RouteIdentity,
    StateStore, ValidationError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
use zeroize::Zeroizing;

const FILE: &str = "keelshell-profiles.ksync";
const MAX_RECORDS: usize = 2000;
const MAX_DEVICES: usize = 64;

/// Portable, allow-listed SSH metadata. Authentication, private-key paths, vault
/// references, host trust, history and AI settings are structurally unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncProfile {
    /// Stable profile identity shared by participating clients.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Legacy group label; local folder placement remains device-local.
    pub group: String,
    /// Remote hostname or IP literal.
    pub host: String,
    /// Remote SSH port.
    pub port: u16,
    /// Remote account name, never its authentication secret.
    pub username: String,
    /// Search labels.
    pub tags: Vec<String>,
    /// Favorite preference.
    pub favorite: bool,
    /// Saved jump profile identity, with no runtime route or trust.
    pub jump_host: Option<Uuid>,
    /// Proxy endpoint/account metadata, never its password.
    pub proxy: Option<ConnectionProxy>,
    /// Bounded reconnect preference; applying it cannot start a connection.
    pub reconnect: ReconnectPolicy,
}
impl SyncProfile {
    fn from_connection(c: &Connection) -> Self {
        Self {
            id: c.id,
            name: c.name.clone(),
            group: c.group.clone(),
            host: c.host.clone(),
            port: c.port,
            username: c.username.clone(),
            tags: c.tags.clone(),
            favorite: c.favorite,
            jump_host: c.jump_host,
            proxy: c.proxy.clone(),
            reconnect: c.reconnect,
        }
    }
    fn connection(&self, prior: Option<&Connection>) -> Connection {
        let mut c = Connection::new(&self.name, &self.host, &self.username);
        c.id = self.id;
        c.group = self.group.clone();
        c.port = self.port;
        c.tags = self.tags.clone();
        c.favorite = self.favorite;
        c.jump_host = self.jump_host;
        c.proxy = self.proxy.clone();
        c.reconnect = self.reconnect;
        // Local credentials remain bound to the exact endpoint/authentication route.
        // New/changed destinations prompt with agent defaults, never inherit a key.
        if let Some(prior) = prior
            && prior.host == c.host
            && prior.port == c.port
            && prior.username == c.username
            && prior.jump_host == c.jump_host
            && prior.proxy == c.proxy
        {
            c.auth = prior.auth.clone();
            c.credential_ref = prior.credential_ref;
        } else {
            c.auth = AuthMethod::Agent;
        }
        c
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: BTreeMap<Uuid, u64>,
    profile: Option<SyncProfile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    channel: Uuid,
    generation: u64,
    records: BTreeMap<Uuid, Record>,
}
impl Snapshot {
    fn validate(&self) -> Result<(), ProfileSyncError> {
        if self.schema != 1 || self.channel.is_nil() || self.records.len() > MAX_RECORDS {
            return Err(ProfileSyncError::Invalid);
        }
        for (id, r) in &self.records {
            if id.is_nil()
                || r.version.is_empty()
                || r.version.len() > MAX_DEVICES
                || r.version.iter().any(|(d, v)| d.is_nil() || *v == 0)
                || r.profile.as_ref().is_some_and(|p| p.id != *id)
            {
                return Err(ProfileSyncError::Invalid);
            }
            if let Some(p) = &r.profile {
                p.connection(None)
                    .validate()
                    .map_err(|_| ProfileSyncError::Invalid)?;
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    expected: Option<String>,
    bytes: Vec<u8>,
    snapshot: Snapshot,
}
/// Non-secret, device-local synchronization state, atomically saved with profiles.
///
/// Baselines and encrypted pending publication bytes survive interruption. Neither
/// passphrases nor derived encryption keys are serialized. Disabling retains this
/// rollback anchor; resetting it requires a deliberate new channel/device setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSyncLocal {
    directory: PathBuf,
    device: Uuid,
    enabled: bool,
    seen_digest: Option<String>,
    baseline: Snapshot,
    pending: Option<Pending>,
    #[serde(default)]
    local_folder_profiles: BTreeSet<Uuid>,
}
impl ProfileSyncLocal {
    /// User-selected transport directory, without filesystem or cloud access.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Whether this client explicitly enabled synchronization.
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    /// Whether an approved publication needs explicit recovery.
    pub fn publication_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Generation of the last locally acknowledged shared snapshot.
    pub fn generation(&self) -> u64 {
        self.baseline.generation
    }
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        let invalid = || {
            ValidationError::new(
                "profile_sync",
                "invalid or oversized synchronization ledger",
            )
        };
        if self.device.is_nil()
            || !self.directory.is_absolute()
            || self.directory.to_str().is_none_or(|s| s.len() > 4096)
            || self.seen_digest.as_ref().is_some_and(|s| !valid_digest(s))
        {
            return Err(invalid());
        }
        if self.local_folder_profiles.len() > crate::model::MAX_CONNECTIONS
            || self.local_folder_profiles.iter().any(Uuid::is_nil)
        {
            return Err(invalid());
        }
        self.baseline.validate().map_err(|_| invalid())?;
        if let Some(p) = &self.pending {
            if p.bytes.len() > crate::model::MAX_DOCUMENT_BYTES
                || p.expected.as_ref().is_some_and(|s| !valid_digest(s))
                || p.snapshot.channel != self.baseline.channel
                || p.snapshot.generation
                    != self
                        .baseline
                        .generation
                        .checked_add(1)
                        .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            p.snapshot.validate().map_err(|_| invalid())?;
        }
        Ok(())
    }
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Explicit resolution of one reviewed difference, including deletion conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSyncChoice {
    /// Keep the reviewed portable value and publish its resolved version.
    Local,
    /// Accept the shared value, including a tombstone.
    Remote,
}
/// One full-profile difference requiring an explicit selection.
#[derive(Debug, Clone)]
pub struct ProfileSyncRow {
    /// Stable identity; selections must cover exactly the displayed row set.
    pub id: Uuid,
    /// This client's portable metadata, or a deletion. A folder-managed group
    /// projects the acknowledged shared label; see `local_placement` separately.
    pub local: Option<SyncProfile>,
    /// Shared allow-listed metadata, or a persistent tombstone.
    pub remote: Option<SyncProfile>,
    /// Both sides changed independently from the last approved baseline.
    pub conflict: bool,
    /// Device-local folder placement retained by either resolution. An empty
    /// string denotes the root; this is distinct from the shared legacy label.
    pub local_placement: Option<String>,
}
/// A complete route change derived from the exact selected profile combination.
/// It contains no credential values or private-key paths.
#[derive(Debug, Clone)]
pub struct ProfileSyncRouteChange {
    /// Existing local profile affected, including unchanged downstream metadata.
    pub id: Uuid,
    /// Complete network route before the approved metadata changes.
    pub before: RouteIdentity,
    /// Complete network route after the approved metadata changes.
    pub after: RouteIdentity,
    /// Whether this approval removes a local auth method or credential reference.
    /// Credential-store entries themselves are retained.
    pub resets_authentication: bool,
}
/// Pure validated effects of all selected differences, before save admission.
#[derive(Debug, Clone)]
pub struct ProfileSyncPreview {
    /// Existing profiles whose complete routes or local authentication change.
    pub route_changes: Vec<ProfileSyncRouteChange>,
}
/// Authenticated, immutable review bound to exact local state and peer bytes.
/// Contains no password/derived key and cannot publish until all rows are selected.
pub struct ProfileSyncReview {
    source: AppState,
    directory: PathBuf,
    peer_bytes: Option<Vec<u8>>,
    peer: Snapshot,
    device: Uuid,
    rows: Vec<ProfileSyncRow>,
}
impl ProfileSyncReview {
    /// Complete difference list; every row requires an explicit resolution.
    pub fn rows(&self) -> &[ProfileSyncRow] {
        &self.rows
    }
    /// Validated local snapshot bound to this review, for refreshing a stale UI.
    pub fn local_state(&self) -> &AppState {
        &self.source
    }
    /// Validate every choice and expose complete resulting route effects before
    /// approval. This performs no I/O, credential access, saving or publication.
    pub fn preview(
        &self,
        choices: &BTreeMap<Uuid, ProfileSyncChoice>,
    ) -> Result<ProfileSyncPreview, ProfileSyncError> {
        let (mut candidate, _) = resolve_choices(self, choices, 0)?;
        let route_changes = reconcile_authentication(&self.source, &mut candidate)?;
        candidate.validate()?;
        Ok(ProfileSyncPreview { route_changes })
    }
    /// Whether approval creates a new shared encrypted channel.
    pub fn creates_channel(&self) -> bool {
        self.peer_bytes.is_none()
    }
    /// Authenticated channel identity for confirming devices joined the same space.
    pub fn channel(&self) -> Uuid {
        self.peer.channel
    }
    /// Shared snapshot generation read for this review.
    pub fn generation(&self) -> u64 {
        self.peer.generation
    }
}
/// Actual local commit and publication state. A pending result is recoverable and
/// must be displayed honestly; local changes may already have been saved.
pub struct ProfileSyncOutcome {
    /// Authoritative saved local state, retaining the store's optimistic token.
    pub state: AppState,
    /// True only after peer replacement and final local acknowledgment succeed.
    pub published: bool,
}
/// Typed synchronization failure with no user metadata/password in its messages.
#[derive(Debug, thiserror::Error)]
pub enum ProfileSyncError {
    /// Disk/crypto/state operation failed; do not echo source paths into AI context.
    #[error("synchronization storage or authentication failed")]
    Storage(#[from] Error),
    /// Filesystem operation failed before a local synchronization commit.
    #[error("synchronization I/O failed")]
    Io(#[from] std::io::Error),
    /// Invalid schema, limits, identities, metadata or envelope.
    #[error("invalid synchronization snapshot")]
    Invalid,
    /// Current local/peer content changed after review.
    #[error("synchronization review is stale; inspect again")]
    Stale,
    /// Shared version went backwards or changed at an acknowledged generation.
    #[error("synchronization rollback or record replay refused")]
    Replay,
    /// Directory/channel differs from this client's established rollback anchor.
    #[error("synchronization channel or directory differs")]
    Channel,
    /// Every difference requires exactly one explicit choice.
    #[error("review every synchronization difference before applying")]
    Incomplete,
    /// A previous approved publication must be resumed or retained first.
    #[error("approved synchronization publication is pending")]
    Pending,
    /// Cancel was requested before save admission.
    #[error("synchronization cancelled before saving")]
    Cancelled,
    /// A cooperating publisher owns the transport lock.
    #[error("synchronization transport is busy")]
    Busy,
}

/// Blocking synchronization orchestrator; never access it on a UI/render thread.
pub struct ProfileSyncService {
    store: Arc<StateStore>,
}
impl ProfileSyncService {
    /// Bind operations to the same state store used by the workspace.
    pub fn new(store: Arc<StateStore>) -> Self {
        Self { store }
    }
    /// Authenticate and pull a shared snapshot without saving or publishing.
    /// Missing transport files propose a new channel, never create one implicitly.
    pub fn inspect(
        &self,
        directory: PathBuf,
        password: Zeroizing<String>,
        cancel: &AtomicBool,
    ) -> Result<ProfileSyncReview, ProfileSyncError> {
        check_cancel(cancel)?;
        if password.is_empty() || password.len() > 4096 {
            return Err(Error::VaultInvalidPassphrase.into());
        }
        check_directory(&directory)?;
        let source = self.store.load()?;
        if source
            .profile_sync
            .as_ref()
            .is_some_and(|s| s.pending.is_some())
        {
            return Err(ProfileSyncError::Pending);
        }
        let bytes = read_peer(&directory)?;
        let peer = if let Some(bytes) = &bytes {
            decode_peer(bytes, &password)?
        } else {
            Snapshot {
                schema: 1,
                channel: Uuid::new_v4(),
                generation: 0,
                records: BTreeMap::new(),
            }
        };
        drop(password);
        check_cancel(cancel)?;
        let device = source
            .profile_sync
            .as_ref()
            .map_or_else(Uuid::new_v4, |s| s.device);
        if let Some(local) = &source.profile_sync {
            if local.directory != directory || local.baseline.channel != peer.channel {
                return Err(if bytes.is_none() {
                    ProfileSyncError::Replay
                } else {
                    ProfileSyncError::Channel
                });
            }
            check_replay(local, &peer, bytes.as_deref())?;
        }
        let baseline = source.profile_sync.as_ref().map(|s| &s.baseline.records);
        let active: BTreeMap<_, _> = source
            .connections
            .iter()
            .map(|c| (c.id, wire_profile(&source, c)))
            .collect();
        let ids: BTreeSet<_> = active
            .keys()
            .chain(peer.records.keys())
            .chain(baseline.into_iter().flat_map(|b| b.keys()))
            .copied()
            .collect();
        let mut rows = Vec::new();
        for id in ids {
            let local = active.get(&id).cloned();
            let remote = peer.records.get(&id).and_then(|r| r.profile.clone());
            let base = baseline.and_then(|b| b.get(&id));
            let local_changed = local.as_ref() != base.and_then(|r| r.profile.as_ref());
            let remote_changed = peer.records.get(&id) != base;
            if local_changed || remote_changed {
                rows.push(ProfileSyncRow {
                    id,
                    conflict: local_changed && remote_changed && local != remote,
                    local_placement: local_placement(&source, id),
                    local,
                    remote,
                });
            }
        }
        Ok(ProfileSyncReview {
            source,
            directory,
            peer_bytes: bytes,
            peer,
            device,
            rows,
        })
    }
    /// Apply exactly the reviewed choices. Before admission, recheck both sources
    /// and cancellation. A local approved pending journal precedes publication so
    /// interruption cannot silently lose a committed resolution or duplicate it.
    pub fn apply(
        &self,
        review: ProfileSyncReview,
        choices: BTreeMap<Uuid, ProfileSyncChoice>,
        password: Zeroizing<String>,
        cancel: &AtomicBool,
    ) -> Result<ProfileSyncOutcome, ProfileSyncError> {
        check_cancel(cancel)?;
        let _lock = peer_lock(&review.directory)?;
        if read_peer(&review.directory)? != review.peer_bytes {
            return Err(ProfileSyncError::Stale);
        }
        // Authenticate again using the password supplied for this operation. A
        // create review authenticates its fresh encrypted candidate below.
        if let Some(bytes) = &review.peer_bytes {
            decode_peer(bytes, &password)?;
        }
        let current = self.store.load()?;
        if current != review.source || current.snapshot != review.source.snapshot {
            return Err(ProfileSyncError::Stale);
        }
        let received_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let (mut candidate, next) = resolve_choices(&review, &choices, received_at)?;
        reconcile_authentication(&current, &mut candidate)?;
        let plaintext =
            Zeroizing::new(serde_json::to_vec(&next).map_err(|_| ProfileSyncError::Invalid)?);
        let encrypted = crate::vault::seal_sync_payload(&plaintext, &password)?;
        drop(password);
        drop(plaintext);
        // Pending stores ciphertext and non-secret receipts only. No key survives.
        let baseline = review.peer.clone();
        candidate.profile_sync = Some(ProfileSyncLocal {
            directory: review.directory.clone(),
            device: review.device,
            enabled: true,
            seen_digest: review.peer_bytes.as_deref().map(digest),
            baseline,
            local_folder_profiles: folder_profiles(&current, &candidate),
            pending: Some(Pending {
                expected: review.peer_bytes.as_deref().map(digest),
                bytes: encrypted,
                snapshot: next,
            }),
        });
        candidate.validate()?;
        check_cancel(cancel)?;
        let saved = match self.store.save(&candidate) {
            Ok(saved) => saved,
            Err(Error::Durability(_)) => {
                // Replacement was admitted, but directory durability is unknown.
                // Return the reloaded pending journal and require explicit resume.
                return Ok(ProfileSyncOutcome {
                    state: self.store.load()?,
                    published: false,
                });
            }
            Err(error) => return Err(error.into()),
        };
        // Cancellation after local admission cannot undo that saved decision. Do
        // not publish it until the user explicitly resumes the pending operation.
        if cancel.load(Ordering::Acquire) {
            return Ok(ProfileSyncOutcome {
                state: saved,
                published: false,
            });
        }
        self.finish_locked(saved, cancel)
    }
    /// Explicitly resume a previously approved publication. The passphrase proves
    /// the saved encrypted candidate; shared CAS still refuses unrelated changes.
    pub fn resume(
        &self,
        password: Zeroizing<String>,
        cancel: &AtomicBool,
    ) -> Result<ProfileSyncOutcome, ProfileSyncError> {
        check_cancel(cancel)?;
        let state = self.store.load()?;
        let local = state
            .profile_sync
            .as_ref()
            .ok_or(ProfileSyncError::Pending)?;
        let pending = local.pending.as_ref().ok_or(ProfileSyncError::Pending)?;
        let decoded = decode_peer(&pending.bytes, &password)?;
        drop(password);
        if decoded != pending.snapshot {
            return Err(ProfileSyncError::Invalid);
        }
        let _lock = peer_lock(&local.directory)?;
        check_cancel(cancel)?;
        self.finish_locked(state, cancel)
    }
    /// Disable further synchronization while retaining baselines/replay protection.
    /// Existing profiles and approved pending publication remain unchanged.
    pub fn disable(&self, cancel: &AtomicBool) -> Result<AppState, ProfileSyncError> {
        check_cancel(cancel)?;
        let mut state = self.store.load()?;
        if let Some(local) = &mut state.profile_sync {
            local.enabled = false;
        }
        check_cancel(cancel)?;
        Ok(self.store.save(&state)?)
    }
    /// Explicitly forget this device's channel pairing while keeping every local
    /// profile and secret. This destroys the local rollback anchor and requires
    /// a separate UI confirmation; it never deletes/reset the encrypted peer.
    pub fn forget(&self, cancel: &AtomicBool) -> Result<AppState, ProfileSyncError> {
        check_cancel(cancel)?;
        let mut state = self.store.load()?;
        if state
            .profile_sync
            .as_ref()
            .is_some_and(|s| s.pending.is_some())
        {
            return Err(ProfileSyncError::Pending);
        }
        state.profile_sync = None;
        check_cancel(cancel)?;
        Ok(self.store.save(&state)?)
    }

    /// Discard only the unpublished receipt while retaining approved local edits
    /// and the observed rollback anchor. A published candidate must be resumed.
    pub fn discard_pending(&self, cancel: &AtomicBool) -> Result<AppState, ProfileSyncError> {
        check_cancel(cancel)?;
        let mut state = self.store.load()?;
        let local = state
            .profile_sync
            .as_ref()
            .ok_or(ProfileSyncError::Pending)?;
        let pending = local.pending.as_ref().ok_or(ProfileSyncError::Pending)?;
        let _lock = peer_lock(&local.directory)?;
        if read_peer(&local.directory)?.as_deref() == Some(pending.bytes.as_slice()) {
            return Err(ProfileSyncError::Pending);
        }
        state
            .profile_sync
            .as_mut()
            .ok_or(ProfileSyncError::Pending)?
            .pending = None;
        check_cancel(cancel)?;
        Ok(self.store.save(&state)?)
    }

    fn finish_locked(
        &self,
        mut state: AppState,
        cancel: &AtomicBool,
    ) -> Result<ProfileSyncOutcome, ProfileSyncError> {
        let Some(local) = state.profile_sync.as_ref() else {
            return Err(ProfileSyncError::Pending);
        };
        let Some(pending) = local.pending.as_ref() else {
            return Err(ProfileSyncError::Pending);
        };
        let current = match read_peer(&local.directory) {
            Ok(current) => current,
            Err(_) => {
                return Ok(ProfileSyncOutcome {
                    state,
                    published: false,
                });
            }
        };
        let fresh = match self.store.load() {
            Ok(fresh) => fresh,
            Err(_) => {
                return Ok(ProfileSyncOutcome {
                    state,
                    published: false,
                });
            }
        };
        if fresh != state || fresh.snapshot != state.snapshot {
            return Ok(ProfileSyncOutcome {
                state: fresh,
                published: false,
            });
        }
        let already = current.as_deref().is_some_and(|b| b == pending.bytes);
        if !already && current.as_deref().map(digest) != pending.expected {
            return Ok(ProfileSyncOutcome {
                state,
                published: false,
            });
        }
        if cancel.load(Ordering::Acquire) {
            return Ok(ProfileSyncOutcome {
                state,
                published: false,
            });
        }
        if !already && write_peer(&local.directory, &pending.bytes).is_err() {
            return Ok(ProfileSyncOutcome {
                state,
                published: false,
            });
        }
        let mut acknowledged = state.clone();
        let local = acknowledged
            .profile_sync
            .as_mut()
            .ok_or(ProfileSyncError::Pending)?;
        let pending = local.pending.take().ok_or(ProfileSyncError::Pending)?;
        local.seen_digest = Some(digest(&pending.bytes));
        local.baseline = pending.snapshot;
        if cancel.load(Ordering::Acquire) {
            return Ok(ProfileSyncOutcome {
                state,
                published: false,
            });
        }
        match self.store.save(&acknowledged) {
            Ok(saved) => Ok(ProfileSyncOutcome {
                state: saved,
                published: true,
            }),
            Err(_) => {
                state = self.store.load().unwrap_or(state);
                Ok(ProfileSyncOutcome {
                    state,
                    published: false,
                })
            }
        }
    }
}
fn wire_profile(state: &AppState, connection: &Connection) -> SyncProfile {
    let mut profile = SyncProfile::from_connection(connection);
    if local_placement(state, connection.id).is_some() {
        // Local tree paths must neither overwrite a shared legacy label nor
        // manufacture an edit after moving a previously managed profile to root.
        profile.group = state
            .profile_sync
            .as_ref()
            .and_then(|local| local.baseline.records.get(&connection.id))
            .and_then(|record| record.profile.as_ref())
            .map(|baseline| baseline.group.clone())
            .unwrap_or_default();
    }
    profile
}
fn local_placement(state: &AppState, id: Uuid) -> Option<String> {
    if let Some(folder) = state.folder_id_of(id) {
        return state.folder_path(folder);
    }
    state
        .profile_sync
        .as_ref()
        .filter(|local| local.local_folder_profiles.contains(&id))
        .map(|_| String::new())
}
fn folder_profiles(source: &AppState, candidate: &AppState) -> BTreeSet<Uuid> {
    let existing: BTreeSet<_> = candidate
        .connections
        .iter()
        .map(|profile| profile.id)
        .chain(
            candidate
                .deleted_connections
                .iter()
                .map(|d| d.connection.id),
        )
        .collect();
    source
        .profile_sync
        .as_ref()
        .into_iter()
        .flat_map(|local| &local.local_folder_profiles)
        .chain(source.connection_folders.keys())
        .filter(|id| existing.contains(id))
        .copied()
        .collect()
}
fn resolve_choices(
    review: &ProfileSyncReview,
    choices: &BTreeMap<Uuid, ProfileSyncChoice>,
    received_at: u64,
) -> Result<(AppState, Snapshot), ProfileSyncError> {
    if choices.len() != review.rows.len()
        || review.rows.iter().any(|row| !choices.contains_key(&row.id))
    {
        return Err(ProfileSyncError::Incomplete);
    }
    let current = &review.source;
    let mut next = review.peer.clone();
    next.generation = next
        .generation
        .checked_add(1)
        .ok_or(ProfileSyncError::Invalid)?;
    let mut candidate = current.clone();
    for row in &review.rows {
        let selected = match choices.get(&row.id) {
            Some(ProfileSyncChoice::Local) => &row.local,
            Some(ProfileSyncChoice::Remote) => &row.remote,
            None => return Err(ProfileSyncError::Incomplete),
        };
        if selected != &row.remote {
            let mut version = next
                .records
                .get(&row.id)
                .map(|record| record.version.clone())
                .unwrap_or_default();
            if let Some(base) = current
                .profile_sync
                .as_ref()
                .and_then(|local| local.baseline.records.get(&row.id))
            {
                for (device, value) in &base.version {
                    let count = version.entry(*device).or_default();
                    *count = (*count).max(*value);
                }
            }
            let counter = version.entry(review.device).or_default();
            *counter = counter.checked_add(1).ok_or(ProfileSyncError::Invalid)?;
            next.records.insert(
                row.id,
                Record {
                    version,
                    profile: selected.clone(),
                },
            );
        }
        apply_profile(&mut candidate, row.id, selected.as_ref(), received_at);
    }
    next.validate()?;
    Ok((candidate, next))
}
fn reconcile_authentication(
    source: &AppState,
    candidate: &mut AppState,
) -> Result<Vec<ProfileSyncRouteChange>, ProfileSyncError> {
    let before = crate::routes::RouteIndex::new(source).map_err(Error::from)?;
    let after = crate::routes::RouteIndex::new(candidate).map_err(Error::from)?;
    let mut resets = BTreeSet::new();
    let mut changes = Vec::new();
    for profile in &candidate.connections {
        let next = after.resolve(profile.id, true).map_err(Error::from)?;
        if !before.active.contains(&profile.id) {
            resets.insert(profile.id);
            continue;
        }
        let prior = before.resolve(profile.id, true).map_err(Error::from)?;
        let original = before
            .profiles
            .get(&profile.id)
            .ok_or(ProfileSyncError::Invalid)?;
        let route_changed = prior.identity() != next.identity()
            || !prior
                .hops()
                .iter()
                .map(|hop| hop.id)
                .eq(next.hops().iter().map(|hop| hop.id));
        let auth_changed =
            original.auth != profile.auth || original.credential_ref != profile.credential_ref;
        if route_changed || auth_changed {
            resets.insert(profile.id);
            changes.push(ProfileSyncRouteChange {
                id: profile.id,
                before: prior.identity(),
                after: next.identity(),
                resets_authentication: original.auth != AuthMethod::Agent
                    || original.credential_ref.is_some(),
            });
        }
    }
    // Resolve all routes against the final chosen graph before mutating any auth
    // metadata. A parent edit can affect children absent from the difference rows.
    for profile in &mut candidate.connections {
        if resets.contains(&profile.id) {
            profile.auth = AuthMethod::Agent;
            profile.credential_ref = None;
        }
    }
    Ok(changes)
}
fn apply_profile(state: &mut AppState, id: Uuid, value: Option<&SyncProfile>, received_at: u64) {
    let prior = state.connections.iter().find(|c| c.id == id).cloned();
    let retained = prior.as_ref().or_else(|| {
        state
            .deleted_connections
            .iter()
            .find(|deleted| deleted.connection.id == id)
            .map(|deleted| &deleted.connection)
    });
    let known_local = retained.is_some();
    let local_group = state
        .folder_id_of(id)
        .and_then(|folder| state.folder_path(folder));
    let index = state.connections.iter().position(|c| c.id == id);
    match value {
        Some(profile) => {
            let mut connection = profile.connection(retained);
            if let Some(group) = local_group {
                connection.group = group;
            }
            if let Some(index) = index {
                state.connections[index] = connection;
            } else {
                state.connections.push(connection);
            }
            if !known_local {
                state.connection_folders.remove(&id);
            }
            state
                .deleted_connections
                .retain(|deleted| deleted.connection.id != id);
        }
        None => {
            state.connections.retain(|connection| connection.id != id);
            if let Some(connection) = prior
                && !state
                    .deleted_connections
                    .iter()
                    .any(|d| d.connection.id == id)
            {
                state.deleted_connections.push(crate::DeletedConnection {
                    connection,
                    deleted_at: received_at,
                });
            }
            state
                .recent_connections
                .retain(|recent| recent.connection_id != id);
        }
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), ProfileSyncError> {
    if cancel.load(Ordering::Acquire) {
        Err(ProfileSyncError::Cancelled)
    } else {
        Ok(())
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn decode_peer(bytes: &[u8], password: &str) -> Result<Snapshot, ProfileSyncError> {
    let plaintext = crate::vault::open_sync_payload(bytes, password)?;
    let snapshot: Snapshot =
        serde_json::from_slice(&plaintext).map_err(|_| ProfileSyncError::Invalid)?;
    snapshot.validate()?;
    Ok(snapshot)
}
fn check_replay(
    local: &ProfileSyncLocal,
    peer: &Snapshot,
    bytes: Option<&[u8]>,
) -> Result<(), ProfileSyncError> {
    if peer.generation < local.baseline.generation
        || (peer.generation == local.baseline.generation && bytes.map(digest) != local.seen_digest)
    {
        return Err(ProfileSyncError::Replay);
    }
    for (id, base) in &local.baseline.records {
        let Some(new) = peer.records.get(id) else {
            return Err(ProfileSyncError::Replay);
        };
        if base
            .version
            .iter()
            .any(|(d, v)| new.version.get(d).is_none_or(|n| n < v))
            || (base.version == new.version && base.profile != new.profile)
        {
            return Err(ProfileSyncError::Replay);
        }
    }
    Ok(())
}
fn check_directory(path: &Path) -> Result<(), ProfileSyncError> {
    if !path.is_absolute()
        || path.to_str().is_none_or(|s| s.len() > 4096)
        || !fs::symlink_metadata(path)?.file_type().is_dir()
    {
        return Err(ProfileSyncError::Invalid);
    }
    Ok(())
}
fn read_peer(directory: &Path) -> Result<Option<Vec<u8>>, ProfileSyncError> {
    check_directory(directory)?;
    let path = directory.join(FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !metadata.file_type().is_file() || metadata.len() > crate::model::MAX_DOCUMENT_BYTES as u64 {
        return Err(ProfileSyncError::Invalid);
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(crate::model::MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > crate::model::MAX_DOCUMENT_BYTES {
        return Err(ProfileSyncError::Invalid);
    }
    Ok(Some(bytes))
}
fn peer_lock(directory: &Path) -> Result<File, ProfileSyncError> {
    check_directory(directory)?;
    let path = directory.join("keelshell-profiles.ksync.lock");
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && !metadata.file_type().is_file()
    {
        return Err(ProfileSyncError::Invalid);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(path)?;
    lock.try_lock().map_err(|e| match e {
        fs::TryLockError::WouldBlock => ProfileSyncError::Busy,
        fs::TryLockError::Error(e) => ProfileSyncError::Io(e),
    })?;
    Ok(lock)
}
fn write_peer(directory: &Path, bytes: &[u8]) -> Result<(), ProfileSyncError> {
    let mut temp = tempfile::Builder::new()
        .prefix(".keelshell-sync-")
        .tempfile_in(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(directory.join(FILE))
        .map_err(|e| ProfileSyncError::Io(e.error))?;
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Checked<T> {
        fn checked(self) -> T;
    }
    impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
        #[track_caller]
        fn checked(self) -> T {
            match self {
                Ok(value) => value,
                Err(error) => panic!("sync fixture failed: {error:?}"),
            }
        }
    }
    impl<T> Checked<T> for Option<T> {
        #[track_caller]
        fn checked(self) -> T {
            match self {
                Some(value) => value,
                None => panic!("sync fixture missing expected value"),
            }
        }
    }

    const PASSWORD: &str = "pending-publication-fixture";
    fn pending() -> (tempfile::TempDir, Arc<StateStore>, PathBuf, Vec<u8>) {
        let t = tempfile::tempdir().checked();
        let directory = t.path().join("shared");
        fs::create_dir(&directory).checked();
        let store = Arc::new(StateStore::new(t.path().join("local/state.json")));
        let mut state = store.load().checked();
        let mut connection = Connection::new("Approved", "fixture.invalid", "fixture");
        connection.id = Uuid::from_u128(801);
        state.connections.push(connection.clone());
        let device = Uuid::from_u128(802);
        let channel = Uuid::from_u128(803);
        let baseline = Snapshot {
            schema: 1,
            channel,
            generation: 0,
            records: BTreeMap::new(),
        };
        let snapshot = Snapshot {
            generation: 1,
            records: BTreeMap::from([(
                connection.id,
                Record {
                    version: BTreeMap::from([(device, 1)]),
                    profile: Some(SyncProfile::from_connection(&connection)),
                },
            )]),
            ..baseline.clone()
        };
        let plaintext = Zeroizing::new(serde_json::to_vec(&snapshot).checked());
        let bytes = crate::vault::seal_sync_payload(&plaintext, PASSWORD).checked();
        state.profile_sync = Some(ProfileSyncLocal {
            directory: directory.clone(),
            device,
            enabled: true,
            local_folder_profiles: BTreeSet::new(),
            seen_digest: None,
            baseline,
            pending: Some(Pending {
                expected: None,
                bytes: bytes.clone(),
                snapshot,
            }),
        });
        store.save(&state).checked();
        (t, store, directory, bytes)
    }
    #[test]
    fn explicit_resume_recovers_local_commit_and_is_idempotent_for_already_published_bytes() {
        let (_t, store, dir, bytes) = pending();
        let service = ProfileSyncService::new(store.clone());
        // Represents interruption after peer atomic replacement but before local acknowledgment.
        write_peer(&dir, &bytes).checked();
        assert!(matches!(
            service.discard_pending(&AtomicBool::new(false)),
            Err(ProfileSyncError::Pending)
        ));
        let result = service
            .resume(Zeroizing::new(PASSWORD.into()), &AtomicBool::new(false))
            .checked();
        assert!(result.published);
        assert!(!result.state.profile_sync.checked().publication_pending());
        assert_eq!(read_peer(&dir).checked().checked(), bytes);
    }
    #[test]
    fn explicit_resume_wrong_password_and_cancel_keep_approved_pending_receipt() {
        let (_t, store, dir, _bytes) = pending();
        let service = ProfileSyncService::new(store.clone());
        let before = store.load().checked();
        assert!(
            service
                .resume(Zeroizing::new("wrong".into()), &AtomicBool::new(false))
                .is_err()
        );
        assert!(matches!(
            service.resume(Zeroizing::new(PASSWORD.into()), &AtomicBool::new(true)),
            Err(ProfileSyncError::Cancelled)
        ));
        assert_eq!(store.load().checked(), before);
        assert!(read_peer(&dir).checked().is_none());
    }
    #[test]
    fn changed_peer_leaves_pending_until_explicit_discard_preserves_local_approval() {
        let (_t, store, dir, _bytes) = pending();
        let service = ProfileSyncService::new(store.clone());
        write_peer(&dir, b"different encrypted peer bytes").checked();
        let result = service
            .resume(Zeroizing::new(PASSWORD.into()), &AtomicBool::new(false))
            .checked();
        assert!(!result.published);
        assert!(result.state.profile_sync.checked().publication_pending());
        let state = service.discard_pending(&AtomicBool::new(false)).checked();
        assert_eq!(state.connections[0].name, "Approved");
        assert!(!state.profile_sync.checked().publication_pending());
        assert_eq!(
            read_peer(&dir).checked().checked(),
            b"different encrypted peer bytes"
        );
    }
    #[test]
    fn forgetting_pending_is_refused_and_completed_pairing_can_be_forgotten_without_peer_deletion()
    {
        let (_t, store, dir, bytes) = pending();
        let service = ProfileSyncService::new(store.clone());
        assert!(matches!(
            service.forget(&AtomicBool::new(false)),
            Err(ProfileSyncError::Pending)
        ));
        let before = store.load().checked();
        assert!(before.profile_sync.checked().publication_pending());
        let outcome = service
            .resume(Zeroizing::new(PASSWORD.into()), &AtomicBool::new(false))
            .checked();
        assert!(outcome.published);
        let state = service.forget(&AtomicBool::new(false)).checked();
        assert!(state.profile_sync.is_none());
        assert_eq!(state.connections[0].name, "Approved");
        assert_eq!(read_peer(&dir).checked().checked(), bytes);
    }
    #[test]
    fn higher_generation_cannot_omit_tombstones_or_replay_record_versions() {
        let (_t, store, _dir, _bytes) = pending();
        let state = store.load().checked();
        let mut local = state.profile_sync.checked();
        let pending = local.pending.take().checked();
        local.baseline = pending.snapshot;
        local.seen_digest = Some(digest(&pending.bytes));
        let mut peer = local.baseline.clone();
        peer.generation += 1;
        peer.records.clear();
        assert!(matches!(
            check_replay(&local, &peer, Some(b"new snapshot")),
            Err(ProfileSyncError::Replay)
        ));
        peer = local.baseline.clone();
        peer.generation += 1;
        peer.records.values_mut().next().checked().profile = None;
        assert!(matches!(
            check_replay(&local, &peer, Some(b"new snapshot")),
            Err(ProfileSyncError::Replay)
        ));
    }
}
