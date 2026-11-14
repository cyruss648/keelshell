//! Crash-recovery records for named tunnel workers.
//!
//! A [`TunnelRuntimeSnapshot`] is a durable observation, not a resumable
//! runtime object. It contains enough non-sensitive metadata for a user to
//! decide whether to inspect and explicitly start a new worker after a restart.
//! It never contains a socket, SSH session, worker handle, command, credential,
//! private-key path, password, proxy password, or traffic payload.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AppState, AuthMethod, Error, RouteIdentity, SnapshotRevision, TunnelRule, ValidationError,
};

/// Current format for a named tunnel runtime snapshot.
pub const TUNNEL_RUNTIME_SNAPSHOT_VERSION: u32 = 1;
/// Maximum number of crash-recovery candidates retained in one state file.
pub const MAX_TUNNEL_RUNTIME_SNAPSHOTS: usize = 256;
const MAX_ASSIGNED_LISTENER_BYTES: usize = 512;

/// The authentication category used by each saved route hop.
///
/// Only the category is retained. Passwords, private-key contents, credential
/// references and private-key paths are deliberately excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelAuthKind {
    /// Authentication is obtained from an SSH agent.
    Agent,
    /// A password is requested or resolved at connection time.
    Password,
    /// A private key is read at connection time; its path is not retained.
    PrivateKey,
}

impl TunnelAuthKind {
    fn from_auth(auth: &AuthMethod) -> Self {
        match auth {
            AuthMethod::Agent => Self::Agent,
            AuthMethod::Password => Self::Password,
            AuthMethod::PrivateKey { .. } => Self::PrivateKey,
        }
    }
}

/// Non-sensitive identity of the saved connection route used by a worker.
///
/// The ordered profile IDs detect a changed jump-host graph. `route` contains
/// normalized endpoint and proxy metadata without credentials, while
/// `auth_kinds` detects a changed authentication target without storing any
/// authentication material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelConnectionBinding {
    /// The named connection selected for the tunnel.
    pub connection_id: Uuid,
    /// Ordered profile IDs from the first jump host through the target.
    pub profile_ids: Vec<Uuid>,
    /// Canonical route endpoints and non-secret proxy metadata.
    pub route: RouteIdentity,
    /// Authentication category for each ordered route hop.
    pub auth_kinds: Vec<TunnelAuthKind>,
    /// Opaque fingerprints for the authentication target of each route hop.
    ///
    /// The fingerprint covers only non-secret target metadata, such as a vault
    /// credential ID or private-key path, and is used to fail closed when that
    /// target changes. The original metadata is never persisted.
    pub auth_target_fingerprints: Vec<String>,
}

impl TunnelConnectionBinding {
    /// Resolve a fresh non-secret binding from the current application state.
    ///
    /// This performs metadata validation only. It does not connect, read a
    /// credential, or access a private key.
    pub fn from_state(state: &AppState, connection_id: Uuid) -> Result<Self, Error> {
        let route = state.connection_route(connection_id)?;
        let profile_ids = route.hops().iter().map(|profile| profile.id).collect();
        let auth_kinds = route
            .hops()
            .iter()
            .map(|profile| TunnelAuthKind::from_auth(&profile.auth))
            .collect();
        let auth_target_fingerprints = route.hops().iter().map(auth_target_fingerprint).collect();
        Ok(Self {
            connection_id,
            profile_ids,
            route: route.identity(),
            auth_kinds,
            auth_target_fingerprints,
        })
    }

    /// Validate the bounded, non-sensitive binding shape.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.connection_id.is_nil() {
            return Err(ValidationError::new(
                "tunnel_runtime.connection_id",
                "must not be nil",
            ));
        }
        if self.profile_ids.is_empty()
            || self.profile_ids.len() > crate::MAX_JUMP_HOSTS + 1
            || self.profile_ids.iter().any(Uuid::is_nil)
        {
            return Err(ValidationError::new(
                "tunnel_runtime.profile_ids",
                "must contain a bounded ordered route without nil IDs",
            ));
        }
        let mut unique = HashSet::new();
        if self.profile_ids.iter().any(|id| !unique.insert(id)) {
            return Err(ValidationError::new(
                "tunnel_runtime.profile_ids",
                "must not contain duplicate profile IDs",
            ));
        }
        self.route.validate()?;
        if self.auth_kinds.len() != self.profile_ids.len() {
            return Err(ValidationError::new(
                "tunnel_runtime.auth_kinds",
                "must have one authentication category per route hop",
            ));
        }
        if self.auth_target_fingerprints.len() != self.profile_ids.len()
            || self
                .auth_target_fingerprints
                .iter()
                .any(|fingerprint| !is_sha256_hex(fingerprint))
        {
            return Err(ValidationError::new(
                "tunnel_runtime.auth_target_fingerprints",
                "must have one lowercase SHA-256 fingerprint per route hop",
            ));
        }
        Ok(())
    }
}

fn auth_target_fingerprint(profile: &crate::Connection) -> String {
    let mut digest = Sha256::new();
    digest.update(b"keelshell-tunnel-auth-target-v1\0");
    match &profile.auth {
        AuthMethod::Agent => {
            digest.update(b"agent");
        }
        AuthMethod::Password => {
            digest.update(b"password");
            if let Some(credential_ref) = profile.credential_ref {
                digest.update(b"\0vault\0");
                digest.update(credential_ref.as_bytes());
            } else {
                digest.update(b"\0ephemeral");
            }
        }
        AuthMethod::PrivateKey { path } => {
            digest.update(b"private_key\0");
            digest.update(path.to_string_lossy().as_bytes());
        }
    }
    digest_hex(&digest.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn digest_hex(digest: &[u8]) -> String {
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        encoded.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    encoded
}

/// Why a persisted runtime record is not currently eligible for explicit
/// recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TunnelRecoveryError {
    /// The candidate ID is not in the current state.
    #[error("named tunnel recovery candidate was not found")]
    SnapshotNotFound,
    /// The candidate format is newer than this build understands.
    #[error("named tunnel recovery candidate uses an unsupported snapshot version")]
    UnsupportedSnapshotVersion,
    /// The candidate's persisted shape is invalid.
    #[error("named tunnel recovery candidate is invalid")]
    InvalidSnapshot,
    /// The saved rule was removed.
    #[error("the named tunnel rule no longer exists")]
    RuleMissing,
    /// The saved rule exists but its content changed.
    #[error("the named tunnel rule changed since the last observation")]
    RuleChanged,
    /// The saved connection was removed.
    #[error("the saved connection no longer exists")]
    ConnectionMissing,
    /// The ordered route, endpoint or jump-host graph changed.
    #[error("the saved connection route changed since the last observation")]
    ConnectionRouteChanged,
    /// One or more route-hop authentication categories changed.
    #[error("the saved connection authentication target changed")]
    AuthenticationTargetChanged,
    /// The current application state cannot be admitted for recovery.
    #[error("the current application state is invalid; reload it before recovery")]
    StateInvalid,
    /// Cleanup of the observed worker did not have a confirmed result.
    #[error(
        "named tunnel cleanup is unconfirmed; resolve or discard the candidate before recovery"
    )]
    CleanupUnconfirmed,
    /// The user supplied a token for another review.
    #[error("named tunnel recovery review token does not match")]
    TokenMismatch,
    /// The state or candidate changed after review.
    #[error("named tunnel recovery review is stale; inspect and review again")]
    StateChanged,
}

/// A candidate status that can be displayed without implying that a listener
/// is currently reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelRuntimeStatus {
    /// The last observation was a live listener; the process may have crashed.
    PossibleRunning,
    /// Cleanup did not produce a confirmed result; ownership remains unknown.
    CleanupUnconfirmed,
}

/// A durable, non-sensitive observation of one named tunnel worker.
///
/// The record is intentionally not a restart instruction. It is retained after
/// a crash or unknown cleanup and is removed only after confirmed cleanup or an
/// explicit user discard. Loading it never reconnects or starts a worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelRuntimeSnapshot {
    /// Stable identity for this observed worker instance.
    pub id: Uuid,
    /// Snapshot wire format. Unknown nonzero versions are preserved but fail
    /// closed during recovery planning.
    pub snapshot_version: u32,
    /// Exact non-secret rule content observed when the worker started.
    pub rule: TunnelRule,
    /// Exact non-secret connection route and authentication categories.
    pub connection: TunnelConnectionBinding,
    /// Actual listener endpoint reported by the transport worker.
    pub assigned_listener: String,
    /// Unix seconds when the worker first reported a listener.
    pub started_at: u64,
    /// Why this record is being shown as a recovery candidate.
    pub status: TunnelRuntimeStatus,
}

impl TunnelRuntimeSnapshot {
    /// Construct a live-listener candidate using the current snapshot format.
    pub fn new(
        rule: TunnelRule,
        connection: TunnelConnectionBinding,
        assigned_listener: impl Into<String>,
        started_at: u64,
    ) -> Result<Self, ValidationError> {
        let snapshot = Self {
            id: Uuid::new_v4(),
            snapshot_version: TUNNEL_RUNTIME_SNAPSHOT_VERSION,
            rule,
            connection,
            assigned_listener: assigned_listener.into(),
            started_at,
            status: TunnelRuntimeStatus::PossibleRunning,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Return whether this build can interpret the snapshot fields.
    pub const fn is_supported_version(&self) -> bool {
        self.snapshot_version == TUNNEL_RUNTIME_SNAPSHOT_VERSION
    }

    /// Validate bounded data and the non-sensitive record envelope.
    ///
    /// Unknown nonzero versions remain valid for storage so a newer client can
    /// be preserved across a downgrade. Recovery planning still rejects them.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new("tunnel_runtime.id", "must not be nil"));
        }
        if self.snapshot_version == 0 {
            return Err(ValidationError::new(
                "tunnel_runtime.snapshot_version",
                "must be nonzero",
            ));
        }
        self.rule.validate()?;
        self.connection.validate()?;
        if self.rule.connection_id != self.connection.connection_id {
            return Err(ValidationError::new(
                "tunnel_runtime.connection_id",
                "must match the rule and connection binding",
            ));
        }
        validate_assigned_listener(&self.assigned_listener)?;
        if self.started_at == 0 {
            return Err(ValidationError::new(
                "tunnel_runtime.started_at",
                "must be a nonzero Unix timestamp",
            ));
        }
        Ok(())
    }
}

fn validate_assigned_listener(value: &str) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > MAX_ASSIGNED_LISTENER_BYTES
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '@' | '?' | '#')
        })
    {
        return Err(ValidationError::new(
            "tunnel_runtime.assigned_listener",
            "must be a bounded endpoint receipt without credentials or control characters",
        ));
    }
    Ok(())
}

/// A state-bound, side-effect-free review of one crash-recovery candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTunnelRecoveryPlan {
    snapshot: TunnelRuntimeSnapshot,
    state_revision: SnapshotRevision,
    review_token: TunnelRecoveryReviewToken,
}

impl ValidatedTunnelRecoveryPlan {
    /// Return the exact candidate shown to the user.
    pub const fn snapshot(&self) -> &TunnelRuntimeSnapshot {
        &self.snapshot
    }

    /// Return the opaque token that must be supplied by the Confirm action.
    pub const fn review_token(&self) -> TunnelRecoveryReviewToken {
        self.review_token
    }

    /// Confirm the exact candidate against a fresh current state.
    ///
    /// Confirmation only authorizes a new explicit start. It does not carry or
    /// recreate the old listener, SSH session, socket, worker, or traffic.
    pub fn confirm(
        self,
        state: &AppState,
        token: TunnelRecoveryReviewToken,
    ) -> Result<ConfirmedTunnelRecovery, TunnelRecoveryError> {
        if token != self.review_token {
            return Err(TunnelRecoveryError::TokenMismatch);
        }
        if state.snapshot != self.state_revision {
            return Err(TunnelRecoveryError::StateChanged);
        }
        state.assess_tunnel_runtime_snapshot(self.snapshot.id)?;
        let current = state
            .tunnel_runtime_snapshots
            .iter()
            .find(|candidate| candidate.id == self.snapshot.id)
            .ok_or(TunnelRecoveryError::SnapshotNotFound)?;
        if current != &self.snapshot {
            return Err(TunnelRecoveryError::StateChanged);
        }
        Ok(ConfirmedTunnelRecovery { plan: self })
    }
}

/// Opaque token for one explicit recovery review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunnelRecoveryReviewToken(Uuid);

impl TunnelRecoveryReviewToken {
    /// Return a non-secret hexadecimal receipt suitable for an audit label.
    pub fn hex(self) -> String {
        self.0.simple().to_string()
    }
}

/// A confirmed recovery review with no execution capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedTunnelRecovery {
    plan: ValidatedTunnelRecoveryPlan,
}

impl ConfirmedTunnelRecovery {
    /// Borrow the state-bound recovery plan.
    pub const fn plan(&self) -> &ValidatedTunnelRecoveryPlan {
        &self.plan
    }

    /// Consume the confirmation receipt for a transport-specific fresh start.
    pub fn into_plan(self) -> ValidatedTunnelRecoveryPlan {
        self.plan
    }
}

impl AppState {
    /// Validate all persisted runtime candidates without requiring their
    /// historical rules or connections to still exist.
    pub fn validate_tunnel_runtime_snapshots(&self) -> Result<(), ValidationError> {
        if self.tunnel_runtime_snapshots.len() > MAX_TUNNEL_RUNTIME_SNAPSHOTS {
            return Err(ValidationError::new(
                "tunnel_runtime_snapshots",
                "at most 256 recovery candidates are supported",
            ));
        }
        let mut ids = HashSet::new();
        for snapshot in &self.tunnel_runtime_snapshots {
            snapshot.validate()?;
            if !ids.insert(snapshot.id) {
                return Err(ValidationError::new("tunnel_runtime.id", "must be unique"));
            }
        }
        Ok(())
    }

    /// Add or replace a candidate atomically in memory.
    ///
    /// Call [`crate::StateStore::save`] from a background executor to make the
    /// observation durable. Replacing a candidate never starts or stops a
    /// listener.
    pub fn upsert_tunnel_runtime_snapshot(
        &mut self,
        snapshot: TunnelRuntimeSnapshot,
    ) -> Result<(), Error> {
        snapshot.validate()?;
        let mut candidate = self.clone();
        if let Some(existing) = candidate
            .tunnel_runtime_snapshots
            .iter_mut()
            .find(|existing| existing.id == snapshot.id)
        {
            *existing = snapshot;
        } else {
            if candidate.tunnel_runtime_snapshots.len() >= MAX_TUNNEL_RUNTIME_SNAPSHOTS {
                return Err(ValidationError::new(
                    "tunnel_runtime_snapshots",
                    "at most 256 recovery candidates are supported",
                )
                .into());
            }
            candidate.tunnel_runtime_snapshots.push(snapshot);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Remove a candidate after confirmed cleanup or an explicit user discard.
    pub fn remove_tunnel_runtime_snapshot(&mut self, id: Uuid) -> Result<(), Error> {
        let mut candidate = self.clone();
        let index = candidate
            .tunnel_runtime_snapshots
            .iter()
            .position(|snapshot| snapshot.id == id)
            .ok_or(Error::TunnelRuntimeSnapshotNotFound)?;
        candidate.tunnel_runtime_snapshots.remove(index);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Check whether a candidate is safe to present for explicit review.
    ///
    /// A successful result only admits the review screen. It never implies that
    /// the old listener is reachable and never authorizes automatic startup.
    pub fn assess_tunnel_runtime_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> Result<(), TunnelRecoveryError> {
        self.validate()
            .map_err(|_| TunnelRecoveryError::StateInvalid)?;
        let snapshot = self
            .tunnel_runtime_snapshots
            .iter()
            .find(|snapshot| snapshot.id == snapshot_id)
            .ok_or(TunnelRecoveryError::SnapshotNotFound)?;
        snapshot
            .validate()
            .map_err(|_| TunnelRecoveryError::InvalidSnapshot)?;
        if !snapshot.is_supported_version() {
            return Err(TunnelRecoveryError::UnsupportedSnapshotVersion);
        }
        if snapshot.status == TunnelRuntimeStatus::CleanupUnconfirmed {
            return Err(TunnelRecoveryError::CleanupUnconfirmed);
        }
        if !self
            .connections
            .iter()
            .any(|connection| connection.id == snapshot.connection.connection_id)
        {
            return Err(TunnelRecoveryError::ConnectionMissing);
        }
        let current_rule = self
            .tunnel_rules
            .iter()
            .find(|rule| rule.id == snapshot.rule.id)
            .ok_or(TunnelRecoveryError::RuleMissing)?;
        if current_rule != &snapshot.rule {
            return Err(TunnelRecoveryError::RuleChanged);
        }
        let current_binding = Self::binding_for_recovery(self, snapshot.connection.connection_id)?;
        if current_binding.profile_ids != snapshot.connection.profile_ids
            || current_binding.route != snapshot.connection.route
        {
            return Err(TunnelRecoveryError::ConnectionRouteChanged);
        }
        if current_binding.auth_kinds != snapshot.connection.auth_kinds {
            return Err(TunnelRecoveryError::AuthenticationTargetChanged);
        }
        if current_binding.auth_target_fingerprints != snapshot.connection.auth_target_fingerprints
        {
            return Err(TunnelRecoveryError::AuthenticationTargetChanged);
        }
        Ok(())
    }

    /// Build a complete review plan for a current, exact recovery candidate.
    pub fn plan_tunnel_runtime_recovery(
        &self,
        snapshot_id: Uuid,
    ) -> Result<ValidatedTunnelRecoveryPlan, TunnelRecoveryError> {
        self.assess_tunnel_runtime_snapshot(snapshot_id)?;
        let snapshot = self
            .tunnel_runtime_snapshots
            .iter()
            .find(|snapshot| snapshot.id == snapshot_id)
            .cloned()
            .ok_or(TunnelRecoveryError::SnapshotNotFound)?;
        Ok(ValidatedTunnelRecoveryPlan {
            snapshot,
            state_revision: self.snapshot,
            review_token: TunnelRecoveryReviewToken(Uuid::new_v4()),
        })
    }

    fn binding_for_recovery(
        state: &AppState,
        connection_id: Uuid,
    ) -> Result<TunnelConnectionBinding, TunnelRecoveryError> {
        TunnelConnectionBinding::from_state(state, connection_id)
            .map_err(|_| TunnelRecoveryError::ConnectionRouteChanged)
    }
}
