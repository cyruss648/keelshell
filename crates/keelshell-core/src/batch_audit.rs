//! Bounded, non-secret history for reviewed SSH batch executions.
//!
//! The audit ledger deliberately stores no command text, output, endpoint or
//! credential. A command digest can correlate a later review without storing
//! the command plaintext; it is not a secret and low-entropy commands may be
//! guessable by an observer who already knows candidate commands.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{AppState, Error, ValidationError};

/// Maximum number of retained batch audit records in the application state.
pub const MAX_BATCH_AUDITS: usize = 500;
/// Maximum number of targets in one reviewed batch, matching the scheduler.
pub const MAX_BATCH_AUDIT_TARGETS: usize = 32;
/// Maximum command size accepted when deriving an audit digest.
const MAX_BATCH_COMMAND_BYTES: usize = 64 * 1024;

/// Outcome and policy summary captured when a reviewed batch reaches a terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchAuditSummary {
    /// Number of targets selected for the reviewed run.
    pub target_count: usize,
    /// Targets that returned exit code zero.
    pub succeeded: usize,
    /// Targets with a confirmed non-zero exit or explicit rejection.
    pub failed: usize,
    /// Targets whose remote outcome could not be confirmed.
    pub unknown: usize,
    /// Targets that were never admitted to an exec channel.
    pub not_started: usize,
    /// Whether cancellation was observed while this run was active.
    pub cancelled: bool,
    /// Whether the stop-after-failure policy prevented queued admissions.
    pub stopped_after_failure: bool,
}

/// Non-secret result summary for one explicitly reviewed batch execution.
///
/// `target_ids` contains saved profile identities only when the caller has
/// them. Ephemeral sessions may leave that vector empty while retaining the
/// aggregate `target_count`; no endpoint or display label is persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchAuditRecord {
    /// Stable identity for this history entry.
    pub id: Uuid,
    /// Caller-supplied Unix timestamp in seconds when execution finished.
    pub recorded_at: u64,
    /// SHA-256 digest of the reviewed command, encoded as lowercase hex.
    pub command_sha256: String,
    /// Saved profile IDs represented by this run, if available.
    pub target_ids: Vec<Uuid>,
    /// Number of targets selected for the reviewed run.
    pub target_count: usize,
    /// Targets that returned exit code zero.
    pub succeeded: usize,
    /// Targets with a confirmed non-zero exit or explicit rejection.
    pub failed: usize,
    /// Targets whose remote outcome could not be confirmed.
    pub unknown: usize,
    /// Targets that were never admitted to an exec channel.
    pub not_started: usize,
    /// Whether cancellation was observed while this run was active.
    pub cancelled: bool,
    /// Whether the stop-after-failure policy prevented queued admissions.
    pub stopped_after_failure: bool,
}

impl BatchAuditRecord {
    /// Build an audit record from transient reviewed command text and counts.
    ///
    /// The command is hashed immediately and is never copied into the returned
    /// value. `target_ids` may be a subset when some targets were ephemeral.
    /// Counts must cover every selected target exactly once.
    pub fn new(
        command: &str,
        recorded_at: u64,
        target_ids: Vec<Uuid>,
        summary: BatchAuditSummary,
    ) -> Result<Self, ValidationError> {
        validate_command(command)?;
        Self::from_digest(
            Sha256::digest(command.as_bytes()).into(),
            recorded_at,
            target_ids,
            summary,
        )
    }

    /// Build an audit record from a digest captured before the command leaves
    /// the transient execution workflow.
    ///
    /// This is useful to callers that already hash the reviewed command and
    /// want to avoid retaining another plaintext copy while saving metadata.
    pub fn from_digest(
        digest: [u8; 32],
        recorded_at: u64,
        target_ids: Vec<Uuid>,
        summary: BatchAuditSummary,
    ) -> Result<Self, ValidationError> {
        let record = Self {
            id: Uuid::new_v4(),
            recorded_at,
            command_sha256: digest_hex(&digest),
            target_ids,
            target_count: summary.target_count,
            succeeded: summary.succeeded,
            failed: summary.failed,
            unknown: summary.unknown,
            not_started: summary.not_started,
            cancelled: summary.cancelled,
            stopped_after_failure: summary.stopped_after_failure,
        };
        record.validate()?;
        Ok(record)
    }

    /// Validate the bounded, non-secret wire representation.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new("batch_audit.id", "must not be nil"));
        }
        if self.recorded_at == 0 {
            return Err(ValidationError::new(
                "batch_audit.recorded_at",
                "must be a nonzero Unix timestamp",
            ));
        }
        if self.command_sha256.len() != 64
            || !self
                .command_sha256
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(ValidationError::new(
                "batch_audit.command_sha256",
                "must be 64 lowercase hexadecimal characters",
            ));
        }
        if !(1..=MAX_BATCH_AUDIT_TARGETS).contains(&self.target_count) {
            return Err(ValidationError::new(
                "batch_audit.target_count",
                "must be between 1 and 32",
            ));
        }
        if self.target_ids.len() > self.target_count {
            return Err(ValidationError::new(
                "batch_audit.target_ids",
                "cannot exceed target_count",
            ));
        }
        let mut ids = HashSet::with_capacity(self.target_ids.len());
        if self
            .target_ids
            .iter()
            .any(|id| id.is_nil() || !ids.insert(*id))
        {
            return Err(ValidationError::new(
                "batch_audit.target_ids",
                "must contain unique non-nil identities",
            ));
        }
        if self.succeeded > self.target_count
            || self.failed > self.target_count
            || self.unknown > self.target_count
            || self.not_started > self.target_count
            || self
                .succeeded
                .checked_add(self.failed)
                .and_then(|count| count.checked_add(self.unknown))
                .and_then(|count| count.checked_add(self.not_started))
                != Some(self.target_count)
        {
            return Err(ValidationError::new(
                "batch_audit.outcomes",
                "outcome counts must cover target_count exactly once",
            ));
        }
        Ok(())
    }
}

/// Return the lowercase SHA-256 digest used by [`BatchAuditRecord`].
pub fn command_sha256(command: &str) -> String {
    let digest: [u8; 32] = Sha256::digest(command.as_bytes()).into();
    digest_hex(&digest)
}

fn digest_hex(digest: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        encoded.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    encoded
}

impl AppState {
    /// Append one validated audit record and retain only the newest 500 entries.
    ///
    /// The candidate state is validated before assignment, so an invalid or
    /// duplicate record cannot partially mutate the caller's snapshot. Persist
    /// the resulting state through [`crate::StateStore`] after this edit.
    pub fn record_batch_audit(&mut self, record: BatchAuditRecord) -> Result<(), Error> {
        let mut candidate = self.clone();
        if candidate
            .batch_audits
            .iter()
            .any(|entry| entry.id == record.id)
        {
            return Err(ValidationError::new("batch_audit.id", "must be unique").into());
        }
        candidate.batch_audits.push(record);
        if candidate.batch_audits.len() > MAX_BATCH_AUDITS {
            let excess = candidate.batch_audits.len() - MAX_BATCH_AUDITS;
            candidate.batch_audits.drain(..excess);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

pub(crate) fn validate_batch_audits(records: &[BatchAuditRecord]) -> Result<(), ValidationError> {
    if records.len() > MAX_BATCH_AUDITS {
        return Err(ValidationError::new(
            "batch_audits",
            "at most 500 records are supported",
        ));
    }
    let mut ids = HashSet::with_capacity(records.len());
    for record in records {
        record.validate()?;
        if !ids.insert(record.id) {
            return Err(ValidationError::new("batch_audit.id", "must be unique"));
        }
    }
    Ok(())
}

fn validate_command(command: &str) -> Result<(), ValidationError> {
    if command.trim().is_empty()
        || command.len() > MAX_BATCH_COMMAND_BYTES
        || command
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(ValidationError::new(
            "batch_audit.command",
            "must contain bounded text without terminal control characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error:?}"),
        }
    }

    fn error_field<T: std::fmt::Debug, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Ok(value) => panic!("unexpected success: {value:?}"),
            Err(error) => error,
        }
    }

    fn record(id: Uuid, timestamp: u64) -> BatchAuditRecord {
        BatchAuditRecord {
            id,
            recorded_at: timestamp,
            command_sha256: command_sha256("printf 'reviewed'"),
            target_ids: vec![Uuid::new_v4()],
            target_count: 1,
            succeeded: 1,
            failed: 0,
            unknown: 0,
            not_started: 0,
            cancelled: false,
            stopped_after_failure: false,
        }
    }

    #[test]
    fn constructor_hashes_without_retaining_command_text() {
        let record = valid(BatchAuditRecord::new(
            "printf 'secret-value'",
            1,
            vec![Uuid::new_v4()],
            BatchAuditSummary {
                target_count: 1,
                succeeded: 1,
                failed: 0,
                unknown: 0,
                not_started: 0,
                cancelled: false,
                stopped_after_failure: false,
            },
        ));
        assert_eq!(
            record.command_sha256,
            command_sha256("printf 'secret-value'")
        );
        let encoded = valid(serde_json::to_string(&record));
        assert!(!encoded.contains("secret-value"));
        assert!(record.validate().is_ok());
    }

    #[test]
    fn invalid_outcome_counts_and_duplicate_targets_fail_closed() {
        let mut record = record(Uuid::new_v4(), 1);
        record.failed = 1;
        assert_eq!(error_field(record.validate()).field, "batch_audit.outcomes");
        record.failed = 0;
        record.target_ids.push(record.target_ids[0]);
        assert_eq!(
            error_field(record.validate()).field,
            "batch_audit.target_ids"
        );
    }

    #[test]
    fn record_batch_audit_is_atomic_and_fifo_bounded() {
        let mut state = AppState::default();
        let first_id = Uuid::new_v4();
        let first = record(first_id, 1);
        valid(state.record_batch_audit(first));
        let before = state.clone();
        let mut duplicate = record(first_id, 2);
        duplicate.target_ids.clear();
        assert!(state.record_batch_audit(duplicate).is_err());
        assert_eq!(state, before);

        for timestamp in 2..=(MAX_BATCH_AUDITS as u64 + 1) {
            valid(state.record_batch_audit(record(Uuid::new_v4(), timestamp)));
        }
        assert_eq!(state.batch_audits.len(), MAX_BATCH_AUDITS);
        assert_eq!(state.batch_audits[0].recorded_at, 2);
        assert_eq!(
            valid(state.batch_audits.last().ok_or("missing last audit")).recorded_at,
            501
        );
    }
}
