//! Reviewable, side-effect-free plans for synchronizing directory snapshots.
//!
//! This module deliberately stops at a plan and a user-confirmation token.
//! Applying a plan requires a transport-specific worker to re-read the source,
//! revalidate the token and obtain an explicit UI confirmation. No function in
//! this module writes a local or remote filesystem.

use crate::directory_compare::{
    DirectoryCompareReport, DirectoryContentHash, DirectoryEntryKind, DirectoryEntrySnapshot,
    DirectoryEntryStatus,
};
use sha2::{Digest, Sha256};

/// Direction in which copy operations in a plan move bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySyncDirection {
    /// Copy from the left snapshot into the right snapshot.
    LeftToRight,
    /// Copy from the right snapshot into the left snapshot.
    RightToLeft,
}

/// Whether destination-only entries may be deleted by a future executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySyncDeletePolicy {
    /// Preserve entries that exist only on the destination side.
    PreserveDestination,
    /// Include explicit delete operations for destination-only entries.
    IncludeDeletes,
}

/// One operation that a reviewed executor may perform after revalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectorySyncOperation {
    /// Copy one source entry to the destination path.
    Copy {
        /// Normalized relative path to copy.
        path: String,
        /// Source object kind captured during review.
        source_kind: DirectoryEntryKind,
        /// Optional digest observed at planning time for source revalidation.
        expected_source_hash: Option<DirectoryContentHash>,
        /// Optional byte size observed at planning time for source revalidation.
        expected_source_size: Option<u64>,
    },
    /// Delete one destination-only path after an additional explicit review.
    Delete {
        /// Normalized relative path to delete.
        path: String,
    },
}

/// A deterministic, immutable synchronization plan awaiting user review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySyncPlan {
    direction: DirectorySyncDirection,
    delete_policy: DirectorySyncDeletePolicy,
    operations: Vec<DirectorySyncOperation>,
    review_token: [u8; 32],
}

impl DirectorySyncPlan {
    /// Return the selected copy direction.
    pub const fn direction(&self) -> DirectorySyncDirection {
        self.direction
    }

    /// Return the deletion policy captured by this plan.
    pub const fn delete_policy(&self) -> DirectorySyncDeletePolicy {
        self.delete_policy
    }

    /// Return operations in stable path order.
    pub fn operations(&self) -> &[DirectorySyncOperation] {
        &self.operations
    }

    /// Number of operations awaiting review.
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Return the opaque token a UI can display as a short review fingerprint.
    ///
    /// The token is not a credential and does not authorize execution by
    /// itself. An executor must still re-read both roots and obtain a fresh
    /// confirmation from the user.
    pub fn review_token(&self) -> DirectorySyncReviewToken {
        DirectorySyncReviewToken(self.review_token)
    }

    /// Consume the plan after the user has explicitly confirmed its token.
    ///
    /// The returned value is only an execution receipt. It contains no write
    /// capability; a transport worker must enforce source revalidation,
    /// destination checks and its own bounded mutation policy.
    pub fn confirm(
        self,
        confirmation: DirectorySyncReviewToken,
    ) -> Result<ConfirmedDirectorySync, DirectorySyncConfirmError> {
        if confirmation.0 != self.review_token {
            return Err(DirectorySyncConfirmError::TokenMismatch);
        }
        Ok(ConfirmedDirectorySync { plan: self })
    }
}

/// Opaque fingerprint copied from a plan after its rows were reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectorySyncReviewToken([u8; 32]);

/// A plan whose review token has been explicitly acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedDirectorySync {
    plan: DirectorySyncPlan,
}

impl ConfirmedDirectorySync {
    /// Borrow the confirmed plan for a transport-specific executor.
    pub fn plan(&self) -> &DirectorySyncPlan {
        &self.plan
    }

    /// Consume the receipt and return its plan to a transport adapter.
    pub fn into_plan(self) -> DirectorySyncPlan {
        self.plan
    }
}

/// Failure while producing a bounded synchronization plan.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DirectorySyncPlanError {
    /// An uncertain comparison row was not explicitly excluded.
    #[error("directory sync cannot plan an uncertain row without explicit exclusion: {path}")]
    UncertainRow {
        /// Relative path requiring a content or metadata review.
        path: String,
    },
    /// A malformed comparison row did not include the selected source side.
    #[error("directory sync row has no source entry: {path}")]
    MissingSource {
        /// Relative path of the malformed row.
        path: String,
    },
}

/// Failure while acknowledging a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DirectorySyncConfirmError {
    /// The acknowledgement did not match the exact reviewed plan.
    #[error("directory sync review token does not match the plan")]
    TokenMismatch,
}

/// Build a deterministic, side-effect-free synchronization plan.
///
/// Changed and one-sided rows become copy operations in the requested
/// direction. Destination-only rows are preserved or represented as explicit
/// deletes according to `delete_policy`. `Uncertain` rows are rejected: the
/// caller must first collect bounded content hashes or explicitly omit the
/// row in a later policy layer. No operation is executed here.
pub fn plan_directory_sync(
    report: &DirectoryCompareReport,
    direction: DirectorySyncDirection,
    delete_policy: DirectorySyncDeletePolicy,
) -> Result<DirectorySyncPlan, DirectorySyncPlanError> {
    let mut operations = Vec::new();
    for row in report.rows() {
        let operation = match (direction, row.status) {
            (_, DirectoryEntryStatus::Same) => None,
            (_, DirectoryEntryStatus::Uncertain) => {
                return Err(DirectorySyncPlanError::UncertainRow {
                    path: row.path.clone(),
                });
            }
            (DirectorySyncDirection::LeftToRight, DirectoryEntryStatus::LeftOnly)
            | (DirectorySyncDirection::LeftToRight, DirectoryEntryStatus::Changed) => {
                Some(copy_operation(row.left.as_ref(), &row.path)?)
            }
            (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::RightOnly)
            | (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::Changed) => {
                Some(copy_operation(row.right.as_ref(), &row.path)?)
            }
            (DirectorySyncDirection::LeftToRight, DirectoryEntryStatus::RightOnly)
            | (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::LeftOnly) => {
                match delete_policy {
                    DirectorySyncDeletePolicy::PreserveDestination => None,
                    DirectorySyncDeletePolicy::IncludeDeletes => {
                        Some(DirectorySyncOperation::Delete {
                            path: row.path.clone(),
                        })
                    }
                }
            }
        };
        if let Some(operation) = operation {
            operations.push(operation);
        }
    }
    let review_token = plan_digest(direction, delete_policy, &operations);
    Ok(DirectorySyncPlan {
        direction,
        delete_policy,
        operations,
        review_token,
    })
}

fn copy_operation(
    entry: Option<&DirectoryEntrySnapshot>,
    path: &str,
) -> Result<DirectorySyncOperation, DirectorySyncPlanError> {
    let Some(entry) = entry else {
        return Err(DirectorySyncPlanError::MissingSource {
            path: path.to_owned(),
        });
    };
    Ok(DirectorySyncOperation::Copy {
        path: entry.path.clone(),
        source_kind: entry.kind,
        expected_source_hash: entry.content_hash,
        expected_source_size: entry.size,
    })
}

fn plan_digest(
    direction: DirectorySyncDirection,
    delete_policy: DirectorySyncDeletePolicy,
    operations: &[DirectorySyncOperation],
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.push(match direction {
        DirectorySyncDirection::LeftToRight => 0,
        DirectorySyncDirection::RightToLeft => 1,
    });
    bytes.push(match delete_policy {
        DirectorySyncDeletePolicy::PreserveDestination => 0,
        DirectorySyncDeletePolicy::IncludeDeletes => 1,
    });
    for operation in operations {
        match operation {
            DirectorySyncOperation::Copy {
                path,
                source_kind,
                expected_source_hash,
                expected_source_size,
            } => {
                bytes.push(0);
                bytes.extend_from_slice(path.as_bytes());
                bytes.push(0);
                bytes.push(match source_kind {
                    DirectoryEntryKind::File => 0,
                    DirectoryEntryKind::Directory => 1,
                    DirectoryEntryKind::Symlink => 2,
                    DirectoryEntryKind::Other => 3,
                });
                bytes.extend_from_slice(&expected_source_size.unwrap_or_default().to_be_bytes());
                bytes.extend_from_slice(&expected_source_hash.unwrap_or_default());
            }
            DirectorySyncOperation::Delete { path } => {
                bytes.push(1);
                bytes.extend_from_slice(path.as_bytes());
            }
        }
        bytes.push(0xff);
    }
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory_compare::{
        DirectoryEntryKind, DirectoryEntrySnapshot, compare_directories, hash_directory_content,
    };

    #[test]
    fn plan_is_reviewable_and_does_not_execute_writes() {
        let hash =
            hash_directory_content(b"source").unwrap_or_else(|error| panic!("hash: {error}"));
        let left =
            [
                DirectoryEntrySnapshot::new("changed", DirectoryEntryKind::File, Some(6), Some(1))
                    .with_content_hash(hash),
            ];
        let right = [
            DirectoryEntrySnapshot::new("changed", DirectoryEntryKind::File, Some(5), Some(1)),
            DirectoryEntrySnapshot::new("remote-only", DirectoryEntryKind::File, Some(1), Some(1)),
        ];
        let report =
            compare_directories(&left, &right).unwrap_or_else(|error| panic!("report: {error}"));
        let plan = plan_directory_sync(
            &report,
            DirectorySyncDirection::LeftToRight,
            DirectorySyncDeletePolicy::PreserveDestination,
        )
        .unwrap_or_else(|error| panic!("plan: {error}"));
        assert_eq!(plan.operation_count(), 1);
        assert!(matches!(
            plan.operations()[0],
            DirectorySyncOperation::Copy { .. }
        ));
        let confirmed = plan
            .clone()
            .confirm(plan.review_token())
            .unwrap_or_else(|error| panic!("confirm: {error}"));
        assert_eq!(confirmed.plan().operations(), plan.operations());
    }

    #[test]
    fn uncertain_rows_are_rejected_until_content_is_available() {
        let left =
            [
                DirectoryEntrySnapshot::new("file", DirectoryEntryKind::File, Some(1), Some(1))
                    .with_content_hash([1; 32]),
            ];
        let right = [DirectoryEntrySnapshot::new(
            "file",
            DirectoryEntryKind::File,
            Some(1),
            Some(1),
        )];
        let report =
            compare_directories(&left, &right).unwrap_or_else(|error| panic!("report: {error}"));
        assert!(matches!(
            plan_directory_sync(
                &report,
                DirectorySyncDirection::LeftToRight,
                DirectorySyncDeletePolicy::PreserveDestination
            ),
            Err(DirectorySyncPlanError::UncertainRow { .. })
        ));
    }

    #[test]
    fn delete_policy_controls_destination_only_rows() {
        let left = [DirectoryEntrySnapshot::new(
            "local",
            DirectoryEntryKind::File,
            Some(1),
            Some(1),
        )];
        let right = [DirectoryEntrySnapshot::new(
            "remote",
            DirectoryEntryKind::File,
            Some(1),
            Some(1),
        )];
        let report =
            compare_directories(&left, &right).unwrap_or_else(|error| panic!("report: {error}"));
        let plan = plan_directory_sync(
            &report,
            DirectorySyncDirection::LeftToRight,
            DirectorySyncDeletePolicy::IncludeDeletes,
        )
        .unwrap_or_else(|error| panic!("plan: {error}"));
        assert!(plan.operations().iter().any(|operation| matches!(operation, DirectorySyncOperation::Delete { path } if path == "remote")));
    }

    #[test]
    fn mismatched_review_token_is_rejected() {
        let report =
            compare_directories(&[], &[]).unwrap_or_else(|error| panic!("report: {error}"));
        let plan = plan_directory_sync(
            &report,
            DirectorySyncDirection::LeftToRight,
            DirectorySyncDeletePolicy::PreserveDestination,
        )
        .unwrap_or_else(|error| panic!("plan: {error}"));
        let token = DirectorySyncReviewToken([0; 32]);
        assert_eq!(
            plan.confirm(token),
            Err(DirectorySyncConfirmError::TokenMismatch)
        );
    }
}
