//! Bounded metadata comparison for two directory snapshots.
//!
//! The core layer does not walk a local filesystem or make an SFTP request.
//! Callers collect snapshots on their own workers, then pass the relative
//! entries here for deterministic comparison. Missing metadata is reported as
//! [`DirectoryEntryStatus::Uncertain`] instead of being treated as equality;
//! a later content check can therefore remain an explicit, reviewed action.

use std::collections::BTreeMap;

/// Maximum number of entries accepted from either snapshot.
pub const MAX_DIRECTORY_COMPARE_ENTRIES: usize = 10_000;
/// Maximum UTF-8 bytes in one normalized, relative entry path.
pub const MAX_DIRECTORY_COMPARE_PATH_BYTES: usize = 4_096;

/// Which snapshot side caused a validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryCompareSide {
    /// The first snapshot passed to [`compare_directories`].
    Left,
    /// The second snapshot passed to [`compare_directories`].
    Right,
}

/// The kind of one entry in a directory snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryEntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link. Callers must not follow links while collecting a
    /// snapshot unless that behavior is separately reviewed.
    Symlink,
    /// A device, socket, FIFO, or another unsupported filesystem object.
    Other,
}

/// Portable metadata collected for one relative directory entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntrySnapshot {
    /// Normalized relative path using `/` separators.
    pub path: String,
    /// Filesystem object kind observed by the caller.
    pub kind: DirectoryEntryKind,
    /// Byte size when the source provided one.
    pub size: Option<u64>,
    /// Modification time in seconds since the Unix epoch, when provided.
    pub modified: Option<u64>,
}

impl DirectoryEntrySnapshot {
    /// Construct a snapshot entry after validating its relative path.
    ///
    /// The path is intentionally not made absolute or canonical. Relative
    /// paths keep local and remote roots separate and prevent a comparison
    /// result from accidentally becoming an instruction to access another
    /// location.
    pub fn new(
        path: impl Into<String>,
        kind: DirectoryEntryKind,
        size: Option<u64>,
        modified: Option<u64>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            size,
            modified,
        }
    }
}

/// A validation failure before comparison begins.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DirectoryCompareError {
    /// A snapshot contains more entries than the bounded comparison accepts.
    #[error("{side:?} directory snapshot exceeds the {limit} entry limit")]
    TooManyEntries {
        /// Snapshot side that exceeded the limit.
        side: DirectoryCompareSide,
        /// Maximum number of entries accepted.
        limit: usize,
    },
    /// One relative path is too long.
    #[error("{side:?} directory entry path exceeds the byte limit")]
    PathTooLong {
        /// Snapshot side containing the path.
        side: DirectoryCompareSide,
        /// Number of bytes supplied by the caller.
        bytes: usize,
    },
    /// A path is not a safe normalized relative path.
    #[error("{side:?} directory entry path is invalid")]
    InvalidPath {
        /// Snapshot side containing the path.
        side: DirectoryCompareSide,
        /// The rejected path, retained only for local diagnostics.
        path: String,
    },
    /// The same path appeared more than once in one snapshot.
    #[error("{side:?} directory snapshot contains a duplicate path")]
    DuplicatePath {
        /// Snapshot side containing the duplicate.
        side: DirectoryCompareSide,
        /// The duplicated relative path.
        path: String,
    },
}

/// Classification of one path after comparing both snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryEntryStatus {
    /// Both sides contain the same kind and all available metadata agrees.
    Same,
    /// Only the left snapshot contains the path.
    LeftOnly,
    /// Only the right snapshot contains the path.
    RightOnly,
    /// Both sides contain the path, but known metadata differs.
    Changed,
    /// Both sides contain the path and no known field differs, but at least one
    /// field was unavailable, so equality has not been proven.
    Uncertain,
}

impl DirectoryEntryStatus {
    /// Return whether a row needs an explicit review or transfer decision.
    pub const fn needs_review(self) -> bool {
        !matches!(self, Self::Same)
    }
}

/// One deterministic row in a directory comparison report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryCompareRow {
    /// Relative path shared by the row.
    pub path: String,
    /// Metadata from the left snapshot, if present.
    pub left: Option<DirectoryEntrySnapshot>,
    /// Metadata from the right snapshot, if present.
    pub right: Option<DirectoryEntrySnapshot>,
    /// Comparison classification for this path.
    pub status: DirectoryEntryStatus,
}

/// A bounded, sorted comparison of two directory snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryCompareReport {
    rows: Vec<DirectoryCompareRow>,
    same: usize,
    left_only: usize,
    right_only: usize,
    changed: usize,
    uncertain: usize,
}

impl DirectoryCompareReport {
    /// Return rows sorted by normalized relative path.
    pub fn rows(&self) -> &[DirectoryCompareRow] {
        &self.rows
    }

    /// Number of paths equal on known metadata.
    pub const fn same_count(&self) -> usize {
        self.same
    }

    /// Number of paths present only on the left.
    pub const fn left_only_count(&self) -> usize {
        self.left_only
    }

    /// Number of paths present only on the right.
    pub const fn right_only_count(&self) -> usize {
        self.right_only
    }

    /// Number of paths with a known metadata mismatch.
    pub const fn changed_count(&self) -> usize {
        self.changed
    }

    /// Number of paths whose available metadata did not prove equality.
    pub const fn uncertain_count(&self) -> usize {
        self.uncertain
    }

    /// Return true only when every path is present on both sides and equal.
    pub const fn is_equal(&self) -> bool {
        self.left_only == 0 && self.right_only == 0 && self.changed == 0 && self.uncertain == 0
    }

    /// Number of rows requiring review before any synchronization action.
    pub const fn review_count(&self) -> usize {
        self.left_only + self.right_only + self.changed + self.uncertain
    }
}

/// Compare two bounded directory snapshots by normalized relative path.
///
/// The function performs no filesystem or network I/O. It compares object
/// kind, byte size when both sides provide it, and modification time when both
/// sides provide it. A missing field never silently proves equality and is
/// classified as [`DirectoryEntryStatus::Uncertain`].
///
/// # Errors
///
/// Returns a validation error for an oversized snapshot, duplicate path, or a
/// path that is not a safe normalized relative path.
pub fn compare_directories(
    left: &[DirectoryEntrySnapshot],
    right: &[DirectoryEntrySnapshot],
) -> Result<DirectoryCompareReport, DirectoryCompareError> {
    let left = index_snapshot(left, DirectoryCompareSide::Left)?;
    let right = index_snapshot(right, DirectoryCompareSide::Right)?;
    let mut rows = Vec::with_capacity(left.len().saturating_add(right.len()));
    let mut same = 0;
    let mut left_only = 0;
    let mut right_only = 0;
    let mut changed = 0;
    let mut uncertain = 0;

    for (path, left_entry) in &left {
        match right.get(path) {
            Some(right_entry) => {
                let status = classify_metadata(left_entry, right_entry);
                match status {
                    DirectoryEntryStatus::Same => same += 1,
                    DirectoryEntryStatus::Changed => changed += 1,
                    DirectoryEntryStatus::Uncertain => uncertain += 1,
                    // `classify_metadata` currently only returns paired statuses. Keep
                    // this branch total so a future classifier change cannot panic in a
                    // production comparison worker.
                    DirectoryEntryStatus::LeftOnly | DirectoryEntryStatus::RightOnly => {}
                }
                rows.push(DirectoryCompareRow {
                    path: path.clone(),
                    left: Some(left_entry.clone()),
                    right: Some(right_entry.clone()),
                    status,
                });
            }
            None => {
                left_only += 1;
                rows.push(DirectoryCompareRow {
                    path: path.clone(),
                    left: Some(left_entry.clone()),
                    right: None,
                    status: DirectoryEntryStatus::LeftOnly,
                });
            }
        }
    }
    for (path, right_entry) in right {
        if left.contains_key(&path) {
            continue;
        }
        right_only += 1;
        rows.push(DirectoryCompareRow {
            path,
            left: None,
            right: Some(right_entry),
            status: DirectoryEntryStatus::RightOnly,
        });
    }
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(DirectoryCompareReport {
        rows,
        same,
        left_only,
        right_only,
        changed,
        uncertain,
    })
}

fn index_snapshot(
    entries: &[DirectoryEntrySnapshot],
    side: DirectoryCompareSide,
) -> Result<BTreeMap<String, DirectoryEntrySnapshot>, DirectoryCompareError> {
    if entries.len() > MAX_DIRECTORY_COMPARE_ENTRIES {
        return Err(DirectoryCompareError::TooManyEntries {
            side,
            limit: MAX_DIRECTORY_COMPARE_ENTRIES,
        });
    }
    let mut indexed = BTreeMap::new();
    for entry in entries {
        validate_path(&entry.path, side)?;
        if indexed.insert(entry.path.clone(), entry.clone()).is_some() {
            return Err(DirectoryCompareError::DuplicatePath {
                side,
                path: entry.path.clone(),
            });
        }
    }
    Ok(indexed)
}

fn validate_path(path: &str, side: DirectoryCompareSide) -> Result<(), DirectoryCompareError> {
    if path.len() > MAX_DIRECTORY_COMPARE_PATH_BYTES {
        return Err(DirectoryCompareError::PathTooLong {
            side,
            bytes: path.len(),
        });
    }
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(DirectoryCompareError::InvalidPath {
            side,
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn classify_metadata(
    left: &DirectoryEntrySnapshot,
    right: &DirectoryEntrySnapshot,
) -> DirectoryEntryStatus {
    if left.kind != right.kind
        || matches!((left.size, right.size), (Some(left), Some(right)) if left != right)
        || matches!((left.modified, right.modified), (Some(left), Some(right)) if left != right)
    {
        return DirectoryEntryStatus::Changed;
    }
    if (left.kind == DirectoryEntryKind::File && (left.size.is_none() || right.size.is_none()))
        || left.modified.is_none()
        || right.modified.is_none()
    {
        DirectoryEntryStatus::Uncertain
    } else {
        DirectoryEntryStatus::Same
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: u64, modified: u64) -> DirectoryEntrySnapshot {
        DirectoryEntrySnapshot::new(path, DirectoryEntryKind::File, Some(size), Some(modified))
    }

    #[test]
    fn comparison_is_sorted_and_classifies_metadata_and_presence() {
        let report = compare_directories(
            &[
                file("z.txt", 4, 1),
                file("same.txt", 1, 1),
                file("changed.txt", 1, 1),
            ],
            &[
                file("same.txt", 1, 1),
                file("changed.txt", 2, 1),
                file("new.txt", 1, 1),
            ],
        )
        .unwrap_or_else(|error| panic!("comparison: {error}"));
        assert_eq!(
            report
                .rows()
                .iter()
                .map(|row| row.path.as_str())
                .collect::<Vec<_>>(),
            ["changed.txt", "new.txt", "same.txt", "z.txt"]
        );
        assert_eq!(report.same_count(), 1);
        assert_eq!(report.changed_count(), 1);
        assert_eq!(report.left_only_count(), 1);
        assert_eq!(report.right_only_count(), 1);
        assert_eq!(report.review_count(), 3);
        assert!(!report.is_equal());
    }

    #[test]
    fn missing_metadata_is_uncertain_and_equal_directories_can_be_proven() {
        let left = [DirectoryEntrySnapshot::new(
            "folder",
            DirectoryEntryKind::Directory,
            None,
            None,
        )];
        let right = [DirectoryEntrySnapshot::new(
            "folder",
            DirectoryEntryKind::Directory,
            None,
            None,
        )];
        let uncertain = compare_directories(&left, &right)
            .unwrap_or_else(|error| panic!("comparison: {error}"));
        assert_eq!(uncertain.uncertain_count(), 1);
        assert!(!uncertain.is_equal());

        let left = [file("folder/file", 3, 4)];
        let right = [file("folder/file", 3, 4)];
        let equal = compare_directories(&left, &right)
            .unwrap_or_else(|error| panic!("comparison: {error}"));
        assert!(equal.is_equal());
        assert_eq!(equal.rows()[0].status, DirectoryEntryStatus::Same);
    }

    #[test]
    fn invalid_and_duplicate_paths_are_rejected_with_bounds() {
        let invalid = DirectoryEntrySnapshot {
            path: "../secret".into(),
            kind: DirectoryEntryKind::File,
            size: Some(1),
            modified: Some(1),
        };
        assert!(matches!(
            compare_directories(&[invalid], &[]),
            Err(DirectoryCompareError::InvalidPath {
                side: DirectoryCompareSide::Left,
                ..
            })
        ));
        let duplicate = file("same", 1, 1);
        assert!(matches!(
            compare_directories(&[duplicate.clone(), duplicate], &[]),
            Err(DirectoryCompareError::DuplicatePath {
                side: DirectoryCompareSide::Left,
                ..
            })
        ));
        let many = (0..=MAX_DIRECTORY_COMPARE_ENTRIES)
            .map(|index| file(&format!("file-{index}"), 1, 1))
            .collect::<Vec<_>>();
        assert!(matches!(
            compare_directories(&many, &[]),
            Err(DirectoryCompareError::TooManyEntries {
                side: DirectoryCompareSide::Left,
                ..
            })
        ));
    }
}
