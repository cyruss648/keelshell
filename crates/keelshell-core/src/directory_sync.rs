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

/// Maximum nested directory depth in a complete reviewed mirror.
pub const MAX_DIRECTORY_MIRROR_DEPTH: usize = 32;
/// Maximum file bytes observed across both sides of one reviewed mirror.
pub const MAX_DIRECTORY_MIRROR_CONTENT_BYTES: u64 = 256 * 1024 * 1024;

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
        /// Destination kind observed at planning time, when the path existed.
        expected_destination_kind: Option<DirectoryEntryKind>,
        /// Destination size observed at planning time, when available.
        expected_destination_size: Option<u64>,
        /// Destination digest observed at planning time, when available.
        expected_destination_hash: Option<DirectoryContentHash>,
    },
    /// Delete one destination-only path after an additional explicit review.
    Delete {
        /// Normalized relative path to delete.
        path: String,
        /// Destination kind observed at planning time.
        expected_destination_kind: DirectoryEntryKind,
        /// Destination size observed at planning time, when available.
        expected_destination_size: Option<u64>,
        /// Destination digest observed at planning time, when available.
        expected_destination_hash: Option<DirectoryContentHash>,
    },
}

/// A deterministic, immutable synchronization plan awaiting user review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySyncPlan {
    direction: DirectorySyncDirection,
    delete_policy: DirectorySyncDeletePolicy,
    operations: Vec<DirectorySyncOperation>,
    review_token: [u8; 32],
    bounded_mirror: bool,
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

    /// Whether this plan passed the complete bounded recursive mirror policy.
    /// Generic delete intentions do not satisfy this execution precondition.
    pub const fn is_bounded_mirror(&self) -> bool {
        self.bounded_mirror
    }

    /// Return deterministic execution order: copy/create parents before children,
    /// followed by deletion children before their directory parents. Generic
    /// synchronization intentions retain their stable path order.
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

    /// Return the complete lowercase hexadecimal review fingerprint.
    /// This identifies reviewed intent; it is not execution authority.
    pub fn review_fingerprint(&self) -> String {
        self.review_token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
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
    /// Mirror snapshots contain an unsupported, ambiguous or non-portable row.
    #[error("bounded mirror refuses unsafe snapshot row: {path}")]
    UnsafeMirrorRow {
        /// Relative path requiring a new safe complete snapshot.
        path: String,
    },
    /// Mirror files require complete bounded content observations on both sides.
    #[error("bounded mirror requires file size and SHA-256: {path}")]
    IncompleteMirrorContent {
        /// Relative file path whose complete content has not been captured.
        path: String,
    },
    /// The complete mirror exceeds a fixed depth or combined content budget.
    #[error("bounded mirror exceeds its depth/content budget: {path}")]
    MirrorBudget {
        /// Relative path at which the bounded policy could no longer be met.
        path: String,
    },
    /// A malformed comparison row did not include the selected source side.
    #[error("directory sync row has no source entry: {path}")]
    MissingSource {
        /// Relative path of the malformed row.
        path: String,
    },
    /// A malformed comparison row did not include the selected destination side.
    #[error("directory sync row has no destination entry: {path}")]
    MissingDestination {
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
            | (DirectorySyncDirection::LeftToRight, DirectoryEntryStatus::Changed) => Some(
                copy_operation(row.left.as_ref(), row.right.as_ref(), &row.path)?,
            ),
            (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::RightOnly)
            | (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::Changed) => Some(
                copy_operation(row.right.as_ref(), row.left.as_ref(), &row.path)?,
            ),
            (DirectorySyncDirection::LeftToRight, DirectoryEntryStatus::RightOnly) => {
                match delete_policy {
                    DirectorySyncDeletePolicy::PreserveDestination => None,
                    DirectorySyncDeletePolicy::IncludeDeletes => {
                        Some(delete_operation(row.right.as_ref(), &row.path)?)
                    }
                }
            }
            (DirectorySyncDirection::RightToLeft, DirectoryEntryStatus::LeftOnly) => {
                match delete_policy {
                    DirectorySyncDeletePolicy::PreserveDestination => None,
                    DirectorySyncDeletePolicy::IncludeDeletes => {
                        Some(delete_operation(row.left.as_ref(), &row.path)?)
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
        bounded_mirror: false,
    })
}

/// Build a complete bounded mirror plan for regular files and directory trees.
///
/// Both snapshots must be complete, portable and structurally coherent. Every
/// file requires a complete SHA-256 and size; links, special objects, type/name
/// conflicts and exceeded depth/content budgets reject the whole plan. Every
/// destination-only subtree is expanded into individually reviewed rows, ordered
/// children before parents; no implicit or recursive deletion primitive is granted.
/// This has no filesystem effects. An executor still needs explicit confirmation,
/// captured roots/session authority, tree reservations and fresh per-item checks.
pub fn plan_directory_mirror(
    report: &DirectoryCompareReport,
    direction: DirectorySyncDirection,
) -> Result<DirectorySyncPlan, DirectorySyncPlanError> {
    if let Some(conflict) = directory_mirror_conflicts(report, direction)
        .into_iter()
        .next()
    {
        return Err(match conflict.reason {
            DirectoryMirrorConflictReason::IncompleteContent => {
                DirectorySyncPlanError::IncompleteMirrorContent {
                    path: conflict.path,
                }
            }
            DirectoryMirrorConflictReason::BudgetExceeded => DirectorySyncPlanError::MirrorBudget {
                path: conflict.path,
            },
            _ => DirectorySyncPlanError::UnsafeMirrorRow {
                path: conflict.path,
            },
        });
    }
    let mut plan =
        plan_directory_sync(report, direction, DirectorySyncDeletePolicy::IncludeDeletes)?;
    plan.operations.sort_by(|left, right| match (left, right) {
        (
            DirectorySyncOperation::Copy { path: a, .. },
            DirectorySyncOperation::Copy { path: b, .. },
        ) => a.cmp(b),
        (DirectorySyncOperation::Copy { .. }, DirectorySyncOperation::Delete { .. }) => {
            std::cmp::Ordering::Less
        }
        (DirectorySyncOperation::Delete { .. }, DirectorySyncOperation::Copy { .. }) => {
            std::cmp::Ordering::Greater
        }
        (
            DirectorySyncOperation::Delete { path: a, .. },
            DirectorySyncOperation::Delete { path: b, .. },
        ) => b
            .split('/')
            .count()
            .cmp(&a.split('/').count())
            .then_with(|| a.cmp(b)),
    });
    plan.review_token = plan_digest(direction, plan.delete_policy, &plan.operations);
    plan.bounded_mirror = true;
    let mut hash = Sha256::new();
    hash.update(b"keelshell/bounded-mirror/v2\0");
    hash.update(plan.review_token);
    plan.review_token = hash.finalize().into();
    Ok(plan)
}

/// Fixed reason why a complete observed snapshot cannot form a bounded mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirectoryMirrorConflictReason {
    /// A link, special object or inconsistent directory metadata was observed.
    UnsupportedObject,
    /// The relative name is unsafe on one supported platform.
    NonPortablePath,
    /// The sides disagree on the object's kind.
    TypeConflict,
    /// Distinct paths compare equal without case sensitivity.
    CaseConflict,
    /// A descendant's directory ancestor is missing or has another type.
    IncompleteHierarchy,
    /// A regular file lacks complete bounded size/content observations.
    IncompleteContent,
    /// A depth or combined complete-content budget was exceeded.
    BudgetExceeded,
}
/// One read-only conflict; it never grants deletion or copy authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryMirrorConflict {
    /// Normalized relative path of the observed problem.
    pub path: String,
    /// Fixed policy reason for refusing the whole mirror plan.
    pub reason: DirectoryMirrorConflictReason,
}
/// Inspect every comparison row and return all bounded policy conflicts.
/// A metadata-only caller may collect content for `IncompleteContent` rows,
/// but must refuse all other conflicts and rerun this on complete snapshots.
pub fn directory_mirror_conflicts(
    report: &DirectoryCompareReport,
    _direction: DirectorySyncDirection,
) -> Vec<DirectoryMirrorConflict> {
    use DirectoryMirrorConflictReason as Reason;
    use std::collections::{BTreeMap, BTreeSet};
    let mut conflicts = BTreeSet::new();
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut content_bytes = 0u64;
    for row in report.rows() {
        if !portable_mirror_path(&row.path) {
            conflicts.insert((row.path.clone(), Reason::NonPortablePath));
        }
        if matches!((&row.left,&row.right),(Some(a),Some(b)) if a.kind!=b.kind) {
            conflicts.insert((row.path.clone(), Reason::TypeConflict));
        }
        if let Some(old) = names.insert(row.path.to_lowercase(), &row.path)
            && old != &row.path
        {
            conflicts.insert((old.clone(), Reason::CaseConflict));
            conflicts.insert((row.path.clone(), Reason::CaseConflict));
        }
        for (entry, map) in [(&row.left, &mut left), (&row.right, &mut right)] {
            if let Some(entry) = entry {
                let directory_depth = entry
                    .path
                    .split('/')
                    .count()
                    .saturating_sub(usize::from(entry.kind != DirectoryEntryKind::Directory));
                if directory_depth > MAX_DIRECTORY_MIRROR_DEPTH {
                    conflicts.insert((row.path.clone(), Reason::BudgetExceeded));
                }
                if entry.kind == DirectoryEntryKind::File {
                    content_bytes = content_bytes.saturating_add(entry.size.unwrap_or_default());
                    if content_bytes > MAX_DIRECTORY_MIRROR_CONTENT_BYTES {
                        conflicts.insert((row.path.clone(), Reason::BudgetExceeded));
                    }
                }
                match entry.kind {
                    DirectoryEntryKind::File
                        if entry
                            .size
                            .is_some_and(|n| n <= crate::MAX_DIRECTORY_HASH_BYTES as u64)
                            && entry.content_hash.is_some() => {}
                    DirectoryEntryKind::File => {
                        conflicts.insert((row.path.clone(), Reason::IncompleteContent));
                    }
                    DirectoryEntryKind::Directory
                        if entry.size.is_none() && entry.content_hash.is_none() => {}
                    _ => {
                        conflicts.insert((row.path.clone(), Reason::UnsupportedObject));
                    }
                }
                map.insert(row.path.as_str(), entry);
            }
        }
    }
    for map in [&left, &right] {
        for path in map.keys() {
            let mut child = *path;
            while let Some((parent, _)) = child.rsplit_once('/') {
                if !map
                    .get(parent)
                    .is_some_and(|entry| entry.kind == DirectoryEntryKind::Directory)
                {
                    conflicts.insert(((*path).to_owned(), Reason::IncompleteHierarchy));
                }
                child = parent;
            }
        }
    }
    conflicts
        .into_iter()
        .map(|(path, reason)| DirectoryMirrorConflict { path, reason })
        .collect()
}

fn portable_mirror_path(path: &str) -> bool {
    path.split('/').all(|name| {
        let stem = name
            .split('.')
            .next()
            .unwrap_or(name)
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        let device = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
        ) || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(
                stem.get(3..),
                Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
            ));
        !name.is_empty()
            && name.len() <= 255
            && !matches!(name, "." | "..")
            && !device
            && !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
    })
}

fn copy_operation(
    source: Option<&DirectoryEntrySnapshot>,
    destination: Option<&DirectoryEntrySnapshot>,
    path: &str,
) -> Result<DirectorySyncOperation, DirectorySyncPlanError> {
    let Some(source) = source else {
        return Err(DirectorySyncPlanError::MissingSource {
            path: path.to_owned(),
        });
    };
    Ok(DirectorySyncOperation::Copy {
        path: source.path.clone(),
        source_kind: source.kind,
        expected_source_hash: source.content_hash,
        expected_source_size: source.size,
        expected_destination_kind: destination.map(|entry| entry.kind),
        expected_destination_size: destination.and_then(|entry| entry.size),
        expected_destination_hash: destination.and_then(|entry| entry.content_hash),
    })
}

fn delete_operation(
    destination: Option<&DirectoryEntrySnapshot>,
    path: &str,
) -> Result<DirectorySyncOperation, DirectorySyncPlanError> {
    let Some(destination) = destination else {
        return Err(DirectorySyncPlanError::MissingDestination {
            path: path.to_owned(),
        });
    };
    Ok(DirectorySyncOperation::Delete {
        path: destination.path.clone(),
        expected_destination_kind: destination.kind,
        expected_destination_size: destination.size,
        expected_destination_hash: destination.content_hash,
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
                expected_destination_kind,
                expected_destination_size,
                expected_destination_hash,
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
                bytes.push(match expected_destination_kind {
                    Some(DirectoryEntryKind::File) => 0,
                    Some(DirectoryEntryKind::Directory) => 1,
                    Some(DirectoryEntryKind::Symlink) => 2,
                    Some(DirectoryEntryKind::Other) => 3,
                    None => 0xff,
                });
                bytes.extend_from_slice(
                    &expected_destination_size.unwrap_or_default().to_be_bytes(),
                );
                bytes.extend_from_slice(&expected_destination_hash.unwrap_or_default());
            }
            DirectorySyncOperation::Delete {
                path,
                expected_destination_kind,
                expected_destination_size,
                expected_destination_hash,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(path.as_bytes());
                bytes.push(match expected_destination_kind {
                    DirectoryEntryKind::File => 0,
                    DirectoryEntryKind::Directory => 1,
                    DirectoryEntryKind::Symlink => 2,
                    DirectoryEntryKind::Other => 3,
                });
                bytes.extend_from_slice(
                    &expected_destination_size.unwrap_or_default().to_be_bytes(),
                );
                bytes.extend_from_slice(&expected_destination_hash.unwrap_or_default());
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
        assert!(plan.operations().iter().any(|operation| matches!(operation, DirectorySyncOperation::Delete { path, expected_destination_kind: DirectoryEntryKind::File, .. } if path == "remote")));
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
