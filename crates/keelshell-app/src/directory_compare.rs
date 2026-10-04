//! Blocking local directory snapshot collection for the comparison worker.
//!
//! Filesystem traversal belongs on a worker, never in GPUI rendering or event
//! callbacks. The result contains only relative paths and portable metadata so
//! the caller can pass it to keelshell_core::compare_directories alongside a
//! remote SFTP snapshot.

use keelshell_core::{DirectoryEntryKind, DirectoryEntrySnapshot};
use std::{collections::VecDeque, fs, path::Path, time::UNIX_EPOCH};

/// Maximum depth accepted by the local snapshot adapter.
#[allow(dead_code)]
pub(crate) const MAX_LOCAL_SNAPSHOT_DEPTH: usize = 32;

/// A bounded local snapshot traversal failure.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub(crate) enum LocalSnapshotError {
    /// The selected root is not an existing directory.
    #[error("local snapshot root is not a directory")]
    RootNotDirectory,
    /// A filesystem operation failed.
    #[error("local snapshot filesystem error: {0}")]
    Io(#[from] std::io::Error),
    /// The local tree exceeds the configured entry bound.
    #[error("local snapshot exceeds the {0} entry limit")]
    EntryLimit(usize),
    /// A directory would be omitted at the configured depth boundary.
    #[error("local snapshot exceeds the configured depth")]
    DepthLimit,
    /// A filename cannot be represented in the portable UTF-8 snapshot.
    #[error("local snapshot contains a non-UTF-8 filename")]
    NonUtf8Path,
    /// A caller supplied an unsupported bound.
    #[error("invalid local snapshot limits")]
    InvalidLimits,
}

/// Collect a bounded, relative local directory snapshot.
///
/// The function uses symlink_metadata and never follows symbolic links. It
/// performs blocking filesystem I/O and must run in a worker or background
/// task. Relative paths use slash separators and are suitable for
/// keelshell_core::compare_directories.
#[allow(dead_code)]
pub(crate) fn snapshot_local_directory(
    root: &Path,
    max_entries: usize,
    max_depth: usize,
) -> Result<Vec<DirectoryEntrySnapshot>, LocalSnapshotError> {
    if max_entries == 0 || max_depth > MAX_LOCAL_SNAPSHOT_DEPTH {
        return Err(LocalSnapshotError::InvalidLimits);
    }
    let root_metadata = fs::symlink_metadata(root)?;
    if !root_metadata.is_dir() {
        return Err(LocalSnapshotError::RootNotDirectory);
    }

    let mut pending = VecDeque::from([(root.to_owned(), String::new(), 0usize)]);
    let mut snapshot = Vec::new();
    while let Some((directory, prefix, depth)) = pending.pop_front() {
        for child in fs::read_dir(directory)? {
            let child = child?;
            let path = child.path();
            let name = child.file_name();
            let name = name.to_str().ok_or(LocalSnapshotError::NonUtf8Path)?;
            let relative = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            let metadata = fs::symlink_metadata(&path)?;
            let file_type = metadata.file_type();
            let kind = if file_type.is_symlink() {
                DirectoryEntryKind::Symlink
            } else if file_type.is_dir() {
                DirectoryEntryKind::Directory
            } else if file_type.is_file() {
                DirectoryEntryKind::File
            } else {
                DirectoryEntryKind::Other
            };
            if snapshot.len() == max_entries {
                return Err(LocalSnapshotError::EntryLimit(max_entries));
            }
            snapshot.push(DirectoryEntrySnapshot::new(
                relative.clone(),
                kind,
                file_type.is_file().then_some(metadata.len()),
                metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_secs()),
            ));
            if file_type.is_dir() {
                if depth == max_depth {
                    return Err(LocalSnapshotError::DepthLimit);
                }
                pending.push_back((path, relative, depth + 1));
            }
        }
    }
    snapshot.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_relative_sorted_and_respects_depth_and_entry_bounds() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("temp root: {error}"));
        fs::create_dir(root.path().join("nested"))
            .unwrap_or_else(|error| panic!("nested: {error}"));
        fs::write(root.path().join("z.txt"), b"z").unwrap_or_else(|error| panic!("z: {error}"));
        fs::write(root.path().join("nested").join("a.txt"), b"a")
            .unwrap_or_else(|error| panic!("a: {error}"));
        let snapshot = snapshot_local_directory(root.path(), 8, 2)
            .unwrap_or_else(|error| panic!("snapshot: {error}"));
        assert_eq!(
            snapshot
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["nested", "nested/a.txt", "z.txt"]
        );
        assert!(matches!(
            snapshot_local_directory(root.path(), 8, 0),
            Err(LocalSnapshotError::DepthLimit)
        ));
        assert!(matches!(
            snapshot_local_directory(root.path(), 2, 2),
            Err(LocalSnapshotError::EntryLimit(2))
        ));
    }

    #[test]
    fn snapshot_rejects_a_file_root_and_invalid_limits() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("temp root: {error}"));
        let file = root.path().join("file");
        fs::write(&file, b"file").unwrap_or_else(|error| panic!("file: {error}"));
        assert!(matches!(
            snapshot_local_directory(&file, 8, 2),
            Err(LocalSnapshotError::RootNotDirectory)
        ));
        assert!(matches!(
            snapshot_local_directory(root.path(), 0, 2),
            Err(LocalSnapshotError::InvalidLimits)
        ));
    }
}
