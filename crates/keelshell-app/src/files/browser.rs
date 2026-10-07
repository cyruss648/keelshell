//! Display ordering and filtering never grant filesystem or SFTP authority.

use super::local_catalog::{LocalEntry, LocalEntryKind};
use keelshell_session::sftp::RemoteEntry;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub(super) fn local_destination(folder: &Path, name: &str) -> Option<PathBuf> {
    // Match the application's existing portable tree-name restrictions rather
    // than letting a server name become a local drive, device or path alias.
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
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
    if name.is_empty()
        || name.len() > 255
        || matches!(name, "." | "..")
        || device
        || name.ends_with(['.', ' '])
        || name.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
    {
        None
    } else {
        Some(folder.join(name))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SortColumn {
    #[default]
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BrowserSort {
    pub(super) column: SortColumn,
    pub(super) descending: bool,
}

impl BrowserSort {
    pub(super) fn select(&mut self, column: SortColumn) {
        if self.column == column {
            self.descending = !self.descending;
        } else {
            self.column = column;
            self.descending = false;
        }
    }

    pub(super) fn indicator(self, column: SortColumn) -> &'static str {
        if self.column != column {
            ""
        } else if self.descending {
            " ↓"
        } else {
            " ↑"
        }
    }
}

fn optional_order(left: Option<u64>, right: Option<u64>, descending: bool) -> Ordering {
    // Unknown metadata stays last in either direction; it is never invented as zero.
    match (left, right) {
        (Some(left), Some(right)) => {
            let order = left.cmp(&right);
            if descending { order.reverse() } else { order }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(super) fn remote_hidden(entry: &RemoteEntry) -> bool {
    entry.name.starts_with('.')
}

pub(super) fn remote_indices(
    entries: &[RemoteEntry],
    show_hidden: bool,
    sort: BrowserSort,
) -> Vec<usize> {
    let mut indices: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| show_hidden || !remote_hidden(entry))
        .map(|(index, _)| index)
        .collect();
    indices.sort_by(|&left, &right| {
        let left = &entries[left];
        let right = &entries[right];
        let directories = right.is_directory.cmp(&left.is_directory);
        let values = match sort.column {
            SortColumn::Name => {
                let order = left.name.cmp(&right.name);
                if sort.descending {
                    order.reverse()
                } else {
                    order
                }
            }
            SortColumn::Size => optional_order(left.size, right.size, sort.descending),
            SortColumn::Modified => optional_order(
                left.modified.map(u64::from),
                right.modified.map(u64::from),
                sort.descending,
            ),
        };
        directories
            .then(values)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });
    indices
}

pub(super) fn local_indices(
    entries: &[LocalEntry],
    show_hidden: bool,
    sort: BrowserSort,
) -> Vec<usize> {
    let mut indices: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| show_hidden || !entry.hidden)
        .map(|(index, _)| index)
        .collect();
    indices.sort_by(|&left, &right| {
        let left = &entries[left];
        let right = &entries[right];
        let directories = (right.kind == LocalEntryKind::Directory)
            .cmp(&(left.kind == LocalEntryKind::Directory));
        let values = match sort.column {
            SortColumn::Name => {
                let order = left.name.cmp(&right.name);
                if sort.descending {
                    order.reverse()
                } else {
                    order
                }
            }
            SortColumn::Size => optional_order(left.size, right.size, sort.descending),
            SortColumn::Modified => optional_order(left.modified, right.modified, sort.descending),
        };
        directories
            .then(values)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str, directory: bool, size: Option<u64>) -> RemoteEntry {
        RemoteEntry {
            name: name.into(),
            path: format!("/root/{name}"),
            size,
            is_directory: directory,
            is_symlink: false,
            permissions: None,
            modified: None,
        }
    }

    #[test]
    fn remote_sort_keeps_native_indices_and_unknowns_last_in_both_directions() {
        let entries = vec![
            remote("b", false, None),
            remote(".hidden", false, Some(1)),
            remote("z-folder", true, None),
            remote("a", false, Some(10)),
            remote("c", false, Some(2)),
        ];
        for (descending, expected) in [(false, vec![2, 4, 3, 0]), (true, vec![2, 3, 4, 0])] {
            assert_eq!(
                remote_indices(
                    &entries,
                    false,
                    BrowserSort {
                        column: SortColumn::Size,
                        descending
                    }
                ),
                expected
            );
        }
        assert_eq!(
            remote_indices(&entries, true, BrowserSort::default()),
            vec![2, 1, 3, 0, 4]
        );
        assert_eq!(entries[0].name, "b");
    }

    #[test]
    fn server_names_cannot_become_local_traversal_devices_or_aliases() {
        let folder = Path::new("owned-destination");
        for name in [
            "",
            ".",
            "..",
            "../escape",
            "/absolute",
            "x\\child",
            "C:drive",
            "NUL",
            "CON.log",
            "COM1",
            "LPT².txt",
            "trailing.",
            "trailing ",
            "newline\n",
            "wild?card",
        ] {
            assert!(local_destination(folder, name).is_none(), "{name:?}");
        }
        assert_eq!(
            local_destination(folder, "报告.txt"),
            Some(folder.join("报告.txt"))
        );
    }

    #[test]
    fn selecting_a_new_column_resets_direction_and_repeated_selection_toggles() {
        let mut sort = BrowserSort::default();
        sort.select(SortColumn::Name);
        assert!(sort.descending);
        sort.select(SortColumn::Modified);
        assert!(!sort.descending);
        assert_eq!(sort.indicator(SortColumn::Name), "");
        assert_eq!(sort.indicator(SortColumn::Modified), " ↑");
    }

    #[test]
    fn local_links_remain_links_and_hidden_filter_does_not_mutate_the_snapshot() {
        let entries = vec![
            LocalEntry {
                name: ".link".into(),
                path: "/owned/.link".into(),
                kind: LocalEntryKind::Symlink,
                size: None,
                modified: None,
                hidden: true,
            },
            LocalEntry {
                name: "folder".into(),
                path: "/owned/folder".into(),
                kind: LocalEntryKind::Directory,
                size: None,
                modified: None,
                hidden: false,
            },
            LocalEntry {
                name: "file".into(),
                path: "/owned/file".into(),
                kind: LocalEntryKind::File,
                size: Some(1),
                modified: Some(2),
                hidden: false,
            },
        ];
        assert_eq!(
            local_indices(&entries, false, BrowserSort::default()),
            vec![1, 2]
        );
        assert_eq!(
            local_indices(&entries, true, BrowserSort::default()),
            vec![1, 0, 2]
        );
        assert_eq!(entries[0].kind, LocalEntryKind::Symlink);
    }
}
