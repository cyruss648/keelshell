//! Process-wide resource exclusion. A fresh SSH identity cannot erase unknown I/O.
use super::*;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sftp) enum Target {
    Local(PathBuf),
    Remote(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteScope {
    host: String,
    port: u16,
    fingerprint: Option<String>,
}

pub(crate) struct TransferReservations {
    scope: RemoteScope,
    identity: Arc<()>,
}
/// A compound operation may claim both rename endpoints or a sync source/tree.
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::sftp) struct ResourceClaims {
    pub(in crate::sftp) local: Vec<Claim>,
    pub(in crate::sftp) remote: Vec<Claim>,
}
impl ResourceClaims {
    fn writes(&self) -> usize {
        self.local
            .iter()
            .chain(&self.remote)
            .filter(|c| c.write)
            .count()
    }
    fn conflicts(&self, other: &Self, remote: bool) -> bool {
        self.local
            .iter()
            .any(|a| other.local.iter().any(|b| overlap(a, b)))
            || (remote
                && self
                    .remote
                    .iter()
                    .any(|a| other.remote.iter().any(|b| overlap(a, b))))
    }
}
impl From<&Claims> for ResourceClaims {
    fn from(claims: &Claims) -> Self {
        Self {
            local: vec![claims.local.clone()],
            remote: vec![claims.remote.clone()],
        }
    }
}
#[derive(Default)]
struct Registry {
    next: u64,
    active: Vec<Active>,
    uncertain: Vec<Unknown>,
    exhausted: bool,
}
struct Active {
    id: u64,
    owner: Arc<()>,
    scope: RemoteScope,
    claims: ResourceClaims,
    write_ids: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Unknown {
    id: u64,
    group: u64,
    scope: Option<RemoteScope>,
    claim: Claim,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
pub(in crate::sftp) enum Admission {
    Busy,
    Quarantined,
    Ready(Reservation),
}
/// Clones retain active claims through owned temporary cleanup. Unknown
/// publication is immediate, before the caller observes its terminal result.
#[derive(Clone)]
pub(in crate::sftp) struct Reservation {
    state: Arc<ReservationState>,
}
/// Owns one exclusive temporary until publication or cleanup acknowledgement.
/// Aborting cleanup quarantines the full parent action, never unlocking it.
pub(in crate::sftp) struct TemporaryReservation {
    owner: Reservation,
    id: u64,
    claim: Claim,
    completed: bool,
}
impl TemporaryReservation {
    pub(in crate::sftp) fn finish(mut self, known: bool) {
        self.completed = true;
        if !known {
            self.owner.state.retire(false);
            return;
        }
        if let Ok(mut state) = registry().lock()
            && let Some(active) = state
                .active
                .iter_mut()
                .find(|a| a.id == self.owner.state.id)
            && let Some(index) = active.write_ids.iter().position(|id| *id == self.id)
        {
            active.write_ids.remove(index);
            active.claims.remote.retain(|c| c != &self.claim);
        }
        // A late cleanup cannot remove any already-published Unknown record.
    }
}
impl Drop for TemporaryReservation {
    fn drop(&mut self) {
        if !self.completed {
            self.owner.state.retire(false);
        }
        self.owner.state.cleanups.fetch_sub(1, Ordering::AcqRel);
        self.owner.state.cleanup_changed.notify_waiters();
    }
}
struct ReservationState {
    id: u64,
    reported: std::sync::atomic::AtomicBool,
    retired: std::sync::atomic::AtomicBool,
    cleanups: std::sync::atomic::AtomicUsize,
    cleanup_changed: tokio::sync::Notify,
}
fn cost(state: &Registry) -> usize {
    let groups: HashSet<_> = state.uncertain.iter().map(|u| u.group).collect();
    groups.len() + state.active.len()
}
impl TransferReservations {
    pub(crate) fn new(options: &crate::SshOptions) -> Self {
        Self {
            // Principals/routes conservatively share one verified filesystem.
            scope: RemoteScope {
                host: options.host.to_lowercase().trim_end_matches('.').to_owned(),
                port: options.port,
                fingerprint: options.expected_host_key.clone(),
            },
            identity: Arc::new(()),
        }
    }
    pub(super) fn admit(owner: &Arc<Self>, claims: &Claims) -> Admission {
        Self::admit_resources(owner, &ResourceClaims::from(claims))
    }
    pub(in crate::sftp) fn admit_resources(
        owner: &Arc<Self>,
        claims: &ResourceClaims,
    ) -> Admission {
        let Ok(mut state) = registry().lock() else {
            return Admission::Quarantined;
        };
        if state.exhausted
            || state.uncertain.iter().any(|unknown| match &unknown.scope {
                None => claims.local.iter().any(|c| overlap(&unknown.claim, c)),
                Some(scope) => {
                    scope == &owner.scope
                        && claims.remote.iter().any(|c| overlap(&unknown.claim, c))
                }
            })
        {
            return Admission::Quarantined;
        }
        let weight = claims.writes().max(1);
        if weight > 2 {
            return Admission::Quarantined;
        }
        if cost(&state) >= MAX_QUEUED_TRANSFERS {
            return if state.active.is_empty() {
                Admission::Quarantined
            } else {
                Admission::Busy
            };
        }
        if state
            .active
            .iter()
            .filter(|a| Arc::ptr_eq(&a.owner, &owner.identity))
            .count()
            >= MAX_PARALLEL_TRANSFERS
            || state
                .active
                .iter()
                .any(|a| a.claims.conflicts(claims, a.scope == owner.scope))
        {
            return Admission::Busy;
        }
        let Some(end) = state.next.checked_add(weight as u64) else {
            state.exhausted = true;
            return Admission::Quarantined;
        };
        let id = state.next + 1;
        let write_ids = (id..=end).collect();
        state.next = end;
        state.active.push(Active {
            id,
            owner: owner.identity.clone(),
            scope: owner.scope.clone(),
            claims: claims.clone(),
            write_ids,
        });
        Admission::Ready(Reservation {
            state: Arc::new(ReservationState {
                id,
                reported: std::sync::atomic::AtomicBool::new(false),
                retired: std::sync::atomic::AtomicBool::new(false),
                cleanups: std::sync::atomic::AtomicUsize::new(0),
                cleanup_changed: tokio::sync::Notify::new(),
            }),
        })
    }
}
impl Reservation {
    pub(in crate::sftp) fn is_active(&self) -> bool {
        !self.state.retired.load(Ordering::Acquire)
    }
    pub(in crate::sftp) async fn cleanup_known(&self) -> bool {
        let completed = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let changed = self.state.cleanup_changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if self.state.cleanups.load(Ordering::Acquire) == 0 {
                    break;
                }
                changed.await;
            }
        })
        .await;
        if completed.is_err() {
            self.state.retire(false);
        }
        !self.state.retired.load(Ordering::Acquire)
    }
    pub(in crate::sftp) fn finish(self, known: bool) {
        self.state.reported.store(true, Ordering::Release);
        if !known {
            self.state.retire(false);
        }
    }
    /// Extend this owner before exclusive temporary CREATE. It consumes no
    /// extra connection worker slot, but reserves an additional bounded path.
    pub(in crate::sftp) fn add_temporary(&self, claim: Claim) -> Result<TemporaryReservation> {
        let mut state = registry().lock().map_err(|_| SessionError::Worker)?;
        if self.state.retired.load(Ordering::Acquire) {
            return Err(SessionError::MutationQuarantined);
        }
        let index = state
            .active
            .iter()
            .position(|a| a.id == self.state.id)
            .ok_or(SessionError::Closed)?;
        let scope = state.active[index].scope.clone();
        if state.exhausted
            || state
                .uncertain
                .iter()
                .any(|u| u.scope.as_ref() == Some(&scope) && overlap(&u.claim, &claim))
        {
            return Err(SessionError::MutationQuarantined);
        }
        if state.active[index].claims.writes() >= 2
            || state.active.iter().any(|a| {
                a.id != self.state.id
                    && a.scope == scope
                    && a.claims.remote.iter().any(|c| overlap(c, &claim))
            })
        {
            return Err(SessionError::MutationBusy);
        }
        let Some(id) = state.next.checked_add(1) else {
            state.exhausted = true;
            return Err(SessionError::MutationQuarantined);
        };
        state.next = id;
        state.active[index].claims.remote.push(claim.clone());
        state.active[index].write_ids.push(id);
        self.state.cleanups.fetch_add(1, Ordering::AcqRel);
        Ok(TemporaryReservation {
            owner: self.clone(),
            id,
            claim,
            completed: false,
        })
    }
}
impl ReservationState {
    fn retire(&self, known: bool) {
        if self.retired.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut state) = registry().lock()
            && let Some(index) = state.active.iter().position(|a| a.id == self.id)
        {
            let active = state.active.remove(index);
            if !known {
                let writes = active
                    .claims
                    .local
                    .into_iter()
                    .filter(|c| c.write)
                    .map(|c| (None, c))
                    .chain(
                        active
                            .claims
                            .remote
                            .into_iter()
                            .filter(|c| c.write)
                            .map(|c| (Some(active.scope.clone()), c)),
                    );
                for (id, (scope, claim)) in active.write_ids.into_iter().zip(writes) {
                    state.uncertain.push(Unknown {
                        id,
                        group: active.id,
                        scope,
                        claim,
                    });
                }
            }
        }
    }
}
impl Drop for ReservationState {
    fn drop(&mut self) {
        self.retire(self.reported.load(Ordering::Acquire));
    }
}

/// One inspected destination whose previous mutation has no confirmed outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferQuarantineEntry {
    /// Process-unique reservation ID; approval releases only this exact record.
    pub reservation_id: u64,
    /// Complete canonical target; displayed exactly, without case folding.
    pub destination: String,
    /// `true` for a destination on this computer, `false` for the remote host.
    pub local: bool,
    /// Whether the destination existed at inspection time.
    pub exists: bool,
    /// Whether inspection found a directory.
    pub directory: bool,
    /// Whether inspection found a symbolic link; it is never followed here.
    pub symlink: bool,
    /// Observed byte size; absent for a missing target or unavailable metadata.
    pub bytes: Option<u64>,
    /// Observed modification time in seconds since the Unix epoch, when supplied.
    pub modified: Option<u64>,
}
/// Read-only observations and exact quarantine IDs awaiting explicit risk consent.
///
/// Inspection does not prove a late request has stopped. Acknowledgement only
/// removes the reviewed application reservations; the previous job stays unknown.
/// This token is bound to the authenticated connection that made the inspection.
#[derive(Clone)]
pub struct TransferQuarantineReview {
    identity: Arc<()>,
    unknown: Vec<Unknown>,
    entries: Vec<TransferQuarantineEntry>,
    scope_records: Vec<u64>,
}
impl TransferQuarantineReview {
    /// Full destination observations that must be shown before risk consent.
    pub fn entries(&self) -> &[TransferQuarantineEntry] {
        &self.entries
    }
}
fn scope_records(state: &Registry, scope: &RemoteScope) -> Vec<u64> {
    state
        .uncertain
        .iter()
        .filter(|unknown| unknown.scope.as_ref().is_none_or(|target| target == scope))
        .map(|unknown| unknown.id)
        .collect()
}
impl SftpSession {
    /// Inspect quarantines conflicting with a reviewed transfer destination.
    /// No file is changed and no interrupted job is replayed. Remote identity is
    /// the configured host/port and verified host key; aliases are not unified.
    pub async fn inspect_transfer_quarantine(
        &self,
        spec: &TransferSpec,
    ) -> Result<TransferQuarantineReview> {
        deadline(self.timeout, "transfer quarantine inspection", async {
            if self.is_closed() {
                return Err(SessionError::Closed);
            }
            let claims = claims(
                self,
                &if spec.direction == TransferDirection::Upload {
                    TransferJob::AtomicUpload(spec.clone())
                } else {
                    TransferJob::File(spec.clone())
                },
            )
            .await?;
            self.inspect_quarantine_resources(&ResourceClaims::from(&claims))
                .await
        })
        .await
    }
    /// Inspect every unresolved mutation record associated with this remote
    /// pathname, including both rename endpoints and owned temporary paths.
    /// This read-only operation neither authorizes nor resumes a writer.
    pub async fn inspect_remote_mutation_quarantine(
        &self,
        path: &str,
    ) -> Result<TransferQuarantineReview> {
        deadline(
            self.timeout,
            "remote mutation quarantine inspection",
            async {
                let claims = ResourceClaims {
                    local: vec![],
                    remote: vec![remote_claim(self, path, true, true).await?],
                };
                self.inspect_quarantine_resources(&claims).await
            },
        )
        .await
    }
    /// Inspect unresolved records associated with a local destination pathname.
    /// Local reservations are shared across all application SSH connections.
    pub async fn inspect_local_mutation_quarantine(
        &self,
        path: &Path,
    ) -> Result<TransferQuarantineReview> {
        deadline(
            self.timeout,
            "local mutation quarantine inspection",
            async {
                let claims = ResourceClaims {
                    local: vec![local_claim(path, true).await?],
                    remote: vec![],
                };
                self.inspect_quarantine_resources(&claims).await
            },
        )
        .await
    }
    async fn inspect_quarantine_resources(
        &self,
        claims: &ResourceClaims,
    ) -> Result<TransferQuarantineReview> {
        if self.is_closed() {
            return Err(SessionError::Closed);
        }
        let owner = &self._connection.transfer_reservations;
        let (unknown, scope_records): (Vec<_>, _) = {
            let state = registry().lock().map_err(|_| SessionError::Worker)?;
            if state.exhausted {
                return Err(SessionError::Invalid(
                    "transfer reservation registry is unavailable",
                ));
            }
            (
                {
                    let groups: Vec<_> = state
                        .uncertain
                        .iter()
                        .filter(|unknown| match &unknown.scope {
                            None => claims
                                .local
                                .iter()
                                .any(|c| c.write && overlap(&unknown.claim, c)),
                            Some(scope) => {
                                scope == &owner.scope
                                    && claims
                                        .remote
                                        .iter()
                                        .any(|c| c.write && overlap(&unknown.claim, c))
                            }
                        })
                        .map(|u| u.group)
                        .collect();
                    state
                        .uncertain
                        .iter()
                        .filter(|u| groups.contains(&u.group))
                        .cloned()
                        .collect()
                },
                scope_records(&state, &owner.scope),
            )
        };
        if unknown.is_empty() {
            return Err(SessionError::Invalid(
                "no matching unknown transfer destination",
            ));
        }
        let mut entries = Vec::with_capacity(unknown.len());
        for item in &unknown {
            entries.push(inspect(self, item).await?);
        }
        if self.is_closed() {
            return Err(SessionError::Closed);
        }
        Ok(TransferQuarantineReview {
            identity: owner.identity.clone(),
            unknown,
            entries,
            scope_records,
        })
    }
    /// After explicit user consent to unresolved late-I/O risk, release exactly
    /// the inspected reservations. Metadata is rechecked, connection identity
    /// must match, and any changed quarantine revision rejects the review. This writes
    /// no file and cannot establish success, rollback, or remote termination.
    /// `revoked` must be set when the requesting UI loses authority or cancels;
    /// it is checked again immediately before changing the reservations.
    pub async fn acknowledge_transfer_quarantine(
        &self,
        review: &TransferQuarantineReview,
        revoked: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        deadline(self.timeout, "transfer quarantine acknowledgement", async {
            let owner = &self._connection.transfer_reservations;
            if self.is_closed() || revoked.load(Ordering::Acquire) {
                return Err(SessionError::Closed);
            }
            if !Arc::ptr_eq(&owner.identity, &review.identity) {
                return Err(SessionError::Invalid(
                    "quarantine review belongs to another SSH connection",
                ));
            }
            for (unknown, before) in review.unknown.iter().zip(&review.entries) {
                if &inspect(self, unknown).await? != before {
                    return Err(SessionError::Invalid(
                        "quarantined destination changed after inspection; inspect again",
                    ));
                }
            }
            let mut state = registry().lock().map_err(|_| SessionError::Worker)?;
            if self.is_closed() || revoked.load(Ordering::Acquire) {
                return Err(SessionError::Closed);
            }
            if state.exhausted
                || scope_records(&state, &owner.scope) != review.scope_records
                || !review
                    .unknown
                    .iter()
                    .all(|item| state.uncertain.contains(item))
            {
                return Err(SessionError::Invalid(
                    "quarantine review is stale; inspect again",
                ));
            }
            state
                .uncertain
                .retain(|item| !review.unknown.contains(item));
            Ok(())
        })
        .await
    }
}
async fn inspect(sftp: &SftpSession, unknown: &Unknown) -> Result<TransferQuarantineEntry> {
    let target = &unknown.claim.target;
    let mut entry = TransferQuarantineEntry {
        reservation_id: unknown.id,
        destination: match target {
            Target::Local(path) => path.display().to_string(),
            Target::Remote(path) => path.clone(),
        },
        local: matches!(target, Target::Local(_)),
        exists: false,
        directory: false,
        symlink: false,
        bytes: None,
        modified: None,
    };
    match target {
        Target::Local(path) => match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => {
                entry.exists = true;
                entry.directory = metadata.is_dir();
                entry.symlink = metadata.is_symlink();
                entry.bytes = Some(metadata.len());
                entry.modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|time| time.as_secs());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        },
        Target::Remote(path) => match sftp.inner.symlink_metadata(path).await {
            Ok(metadata) => {
                entry.exists = true;
                entry.directory = metadata.is_dir();
                entry.symlink = metadata.is_symlink();
                entry.bytes = metadata.size;
                entry.modified = metadata.mtime.map(u64::from);
            }
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile => {}
            Err(error) => return Err(sftp_error(error)),
        },
    }
    Ok(entry)
}
