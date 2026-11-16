//! Selection owns paths, never painted row indices or filesystem authority.

use std::collections::BTreeSet;

pub(super) const MAX_SELECTION: usize = 32;

/// A destination in the current complete visible order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CursorMove {
    Previous,
    Next,
    First,
    Last,
}

#[derive(Clone, Debug)]
pub(super) struct Selection<T> {
    paths: BTreeSet<T>,
    anchor: Option<T>,
    cursor: Option<T>,
}

impl<T> Default for Selection<T> {
    fn default() -> Self {
        Self {
            paths: BTreeSet::new(),
            anchor: None,
            cursor: None,
        }
    }
}

impl<T: Ord + Clone> Selection<T> {
    pub(super) fn contains(&self, path: &T) -> bool {
        self.paths.contains(path)
    }
    pub(super) fn len(&self) -> usize {
        self.paths.len()
    }
    pub(super) fn paths(&self) -> impl Iterator<Item = &T> {
        self.paths.iter()
    }
    /// The last accepted cursor path, even if a toggle deselected it.
    pub(super) fn cursor(&self) -> Option<&T> {
        self.cursor.as_ref()
    }
    pub(super) fn clear(&mut self) {
        self.paths.clear();
        self.anchor = None;
        self.cursor = None;
    }
    pub(super) fn single(&self) -> Option<&T> {
        (self.paths.len() == 1)
            .then(|| self.paths.first())
            .flatten()
    }
    pub(super) fn retain_visible(&mut self, visible: &[T]) {
        self.paths.retain(|path| visible.contains(path));
        if self
            .anchor
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            self.anchor = None;
        }
        if self
            .cursor
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            self.cursor = None;
        }
    }
    /// Apply a click to the current complete visible order. A stale row cannot
    /// restore a hidden item; oversized ranges leave the previous selection intact.
    pub(super) fn click(&mut self, path: &T, visible: &[T], toggle: bool, range: bool) -> bool {
        let Some(end) = visible.iter().position(|item| item == path) else {
            return false;
        };
        let mut next = if toggle {
            self.paths.clone()
        } else {
            BTreeSet::new()
        };
        if range {
            let start = self
                .anchor
                .as_ref()
                .and_then(|anchor| visible.iter().position(|item| item == anchor))
                .unwrap_or(end);
            next.extend(visible[start.min(end)..=start.max(end)].iter().cloned());
        } else {
            if !next.remove(path) {
                next.insert(path.clone());
            }
        }
        if next.len() > MAX_SELECTION {
            return false;
        }
        self.paths = next;
        if !range {
            self.anchor = Some(path.clone());
        }
        self.cursor = Some(path.clone());
        true
    }
    /// Move in visible order, clamping at the first and last rows.
    ///
    /// Extending replaces the selection with the range from the existing anchor;
    /// a missing or hidden anchor falls back to the destination, as for clicks.
    /// Returns true for an accepted destination, including a clamped boundary.
    /// Empty lists and oversized ranges leave all state unchanged and return false.
    pub(super) fn move_cursor(
        &mut self,
        movement: CursorMove,
        visible: &[T],
        extend: bool,
    ) -> bool {
        let Some(last) = visible.len().checked_sub(1) else {
            return false;
        };
        let current = self
            .cursor
            .as_ref()
            .and_then(|cursor| visible.iter().position(|path| path == cursor));
        let destination = match movement {
            CursorMove::Previous => current.map_or(last, |index| index.saturating_sub(1)),
            CursorMove::Next => current.map_or(0, |index| (index + 1).min(last)),
            CursorMove::First => 0,
            CursorMove::Last => last,
        };
        self.click(&visible[destination], visible, false, extend)
    }
    /// Select every visible path within the budget, replacing hidden selections.
    ///
    /// The first row becomes the anchor. A visible cursor is preserved; otherwise
    /// it moves to the first row. An empty list clears all state successfully.
    /// Oversized lists leave all state unchanged and return false.
    pub(super) fn select_all(&mut self, visible: &[T]) -> bool {
        if visible.len() > MAX_SELECTION {
            return false;
        }
        let cursor = self
            .cursor
            .as_ref()
            .filter(|path| visible.contains(path))
            .or_else(|| visible.first())
            .cloned();
        self.paths = visible.iter().cloned().collect();
        self.anchor = visible.first().cloned();
        self.cursor = cursor;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot<T: Ord + Clone>(selection: &Selection<T>) -> (Vec<T>, Option<T>, Option<T>) {
        (
            selection.paths().cloned().collect(),
            selection.anchor.clone(),
            selection.cursor().cloned(),
        )
    }

    #[test]
    fn ranges_follow_current_order_and_toggle_preserves_other_paths() {
        let mut selection = Selection::default();
        let visible = vec!["c", "a", "b", "d"];
        assert!(selection.click(&"a", &visible, false, false));
        assert!(selection.click(&"d", &visible, false, true));
        assert_eq!(
            selection.paths().copied().collect::<Vec<_>>(),
            vec!["a", "b", "d"]
        );
        assert!(selection.click(&"b", &visible, true, false));
        assert_eq!(
            selection.paths().copied().collect::<Vec<_>>(),
            vec!["a", "d"]
        );
    }
    #[test]
    fn hidden_and_stale_rows_do_not_restore_selection() {
        let mut selection = Selection::default();
        selection.click(&".hidden", &[".hidden", "visible"], false, false);
        selection.retain_visible(&["visible"]);
        assert!(!selection.click(&".hidden", &["visible"], true, false));
        assert_eq!(selection.len(), 0);
        assert!(!selection.contains(&".hidden"));
    }
    #[test]
    fn over_budget_ranges_preserve_the_original_selection() {
        let mut selection = Selection::default();
        let visible: Vec<_> = (0..=MAX_SELECTION).collect();
        selection.click(&0, &visible, false, false);
        assert!(!selection.click(&MAX_SELECTION, &visible, false, true));
        assert_eq!(selection.single(), Some(&0));
    }

    #[test]
    fn default_and_clear_have_no_selection_anchor_or_cursor() {
        let mut selection = Selection::default();
        assert_eq!(snapshot(&selection), (vec![], None, None));
        assert!(selection.click(&"a", &["a", "b"], false, false));
        assert!(selection.click(&"b", &["a", "b"], false, true));
        selection.clear();
        assert_eq!(snapshot(&selection), (vec![], None, None));
    }

    #[test]
    fn successful_clicks_update_cursor_and_only_non_ranges_replace_anchor() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b"];
        assert!(selection.click(&"z", &visible, false, false));
        assert_eq!(snapshot(&selection), (vec!["z"], Some("z"), Some("z")));
        assert!(selection.click(&"m", &visible, false, true));
        assert_eq!(
            snapshot(&selection),
            (vec!["a", "m", "z"], Some("z"), Some("m"))
        );
        assert!(selection.click(&"m", &visible, true, false));
        assert_eq!(snapshot(&selection), (vec!["a", "z"], Some("m"), Some("m")));
        assert!(selection.click(&"b", &visible, true, true));
        assert_eq!(
            snapshot(&selection),
            (vec!["a", "b", "m", "z"], Some("m"), Some("b"))
        );
    }

    #[test]
    fn toggling_off_the_only_path_keeps_cursor_for_the_next_keyboard_move() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "b"];
        assert!(selection.click(&"a", &visible, false, false));
        assert!(selection.click(&"a", &visible, true, false));
        assert_eq!(snapshot(&selection), (vec![], Some("a"), Some("a")));
        assert!(selection.move_cursor(CursorMove::Next, &visible, false));
        assert_eq!(snapshot(&selection), (vec!["b"], Some("b"), Some("b")));
    }

    #[test]
    fn stale_clicks_preserve_paths_anchor_and_cursor_for_every_modifier() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m"];
        assert!(selection.click(&"z", &visible, false, false));
        assert!(selection.click(&"m", &visible, false, true));
        let before = snapshot(&selection);
        for toggle in [false, true] {
            for range in [false, true] {
                assert!(!selection.click(&"hidden", &visible, toggle, range));
                assert_eq!(snapshot(&selection), before);
            }
        }
    }

    #[test]
    fn initial_keyboard_moves_start_at_the_requested_visible_edge() {
        let visible = ["z", "a", "m", "b"];
        for (movement, destination) in [
            (CursorMove::Previous, "b"),
            (CursorMove::Next, "z"),
            (CursorMove::First, "z"),
            (CursorMove::Last, "b"),
        ] {
            for extend in [false, true] {
                let mut selection = Selection::default();
                assert!(selection.move_cursor(movement, &visible, extend));
                assert_eq!(
                    snapshot(&selection),
                    (
                        vec![destination],
                        (!extend).then_some(destination),
                        Some(destination)
                    )
                );
            }
        }
    }

    #[test]
    fn adjacent_keyboard_moves_follow_display_order_in_both_directions() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b"];
        assert!(selection.click(&"a", &visible, false, false));
        for (movement, destination) in [
            (CursorMove::Next, "m"),
            (CursorMove::Next, "b"),
            (CursorMove::Previous, "m"),
            (CursorMove::Previous, "a"),
            (CursorMove::Previous, "z"),
        ] {
            assert!(selection.move_cursor(movement, &visible, false));
            assert_eq!(
                snapshot(&selection),
                (vec![destination], Some(destination), Some(destination))
            );
        }
    }

    #[test]
    fn first_last_and_adjacent_moves_clamp_without_wrapping() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b"];
        assert!(selection.click(&"a", &visible, false, false));
        for (movement, destination) in [
            (CursorMove::First, "z"),
            (CursorMove::Previous, "z"),
            (CursorMove::Last, "b"),
            (CursorMove::Next, "b"),
        ] {
            assert!(selection.move_cursor(movement, &visible, false));
            assert_eq!(
                snapshot(&selection),
                (vec![destination], Some(destination), Some(destination))
            );
        }
    }

    #[test]
    fn non_extending_move_replaces_multiple_paths_and_the_old_anchor() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b"];
        assert!(selection.click(&"z", &visible, false, false));
        assert!(selection.click(&"m", &visible, false, true));
        assert_eq!(selection.len(), 3);
        assert!(selection.move_cursor(CursorMove::Previous, &visible, false));
        assert_eq!(snapshot(&selection), (vec!["a"], Some("a"), Some("a")));
    }

    #[test]
    fn shift_moves_expand_shrink_and_reverse_around_the_original_anchor() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b", "y"];
        assert!(selection.click(&"m", &visible, false, false));
        for (movement, destination, paths) in [
            (CursorMove::Next, "b", vec!["b", "m"]),
            (CursorMove::Next, "y", vec!["b", "m", "y"]),
            (CursorMove::Previous, "b", vec!["b", "m"]),
            (CursorMove::Previous, "m", vec!["m"]),
            (CursorMove::Previous, "a", vec!["a", "m"]),
            (CursorMove::Previous, "z", vec!["a", "m", "z"]),
            (CursorMove::Next, "a", vec!["a", "m"]),
            (CursorMove::Next, "m", vec!["m"]),
            (CursorMove::Next, "b", vec!["b", "m"]),
        ] {
            assert!(selection.move_cursor(movement, &visible, true));
            assert_eq!(snapshot(&selection), (paths, Some("m"), Some(destination)));
        }
    }

    #[test]
    fn shift_first_and_last_replace_the_range_and_keep_the_anchor_at_boundaries() {
        let mut selection = Selection::default();
        let visible = ["z", "a", "m", "b", "y"];
        assert!(selection.click(&"m", &visible, false, false));
        for (movement, destination, paths) in [
            (CursorMove::First, "z", vec!["a", "m", "z"]),
            (CursorMove::Previous, "z", vec!["a", "m", "z"]),
            (CursorMove::Last, "y", vec!["b", "m", "y"]),
            (CursorMove::Next, "y", vec!["b", "m", "y"]),
            (CursorMove::First, "z", vec!["a", "m", "z"]),
        ] {
            assert!(selection.move_cursor(movement, &visible, true));
            assert_eq!(snapshot(&selection), (paths, Some("m"), Some(destination)));
        }
    }

    #[test]
    fn sorting_preserves_cursor_identity_and_moves_in_the_new_order() {
        let mut selection = Selection::default();
        assert!(selection.click(&"b", &["a", "b", "c", "d"], false, false));
        let before = snapshot(&selection);
        let sorted = ["d", "b", "a", "c"];
        selection.retain_visible(&sorted);
        assert_eq!(snapshot(&selection), before);
        assert!(selection.move_cursor(CursorMove::Next, &sorted, false));
        assert_eq!(snapshot(&selection), (vec!["a"], Some("a"), Some("a")));
        assert!(selection.move_cursor(CursorMove::Previous, &sorted, false));
        assert_eq!(snapshot(&selection), (vec!["b"], Some("b"), Some("b")));
    }

    #[test]
    fn shift_ranges_resolve_anchor_and_cursor_again_after_sorting() {
        let mut selection = Selection::default();
        let original = ["a", "b", "c", "d"];
        assert!(selection.click(&"a", &original, false, false));
        assert!(selection.click(&"c", &original, false, true));
        let sorted = ["d", "a", "c", "b"];
        selection.retain_visible(&sorted);
        assert!(selection.move_cursor(CursorMove::Next, &sorted, true));
        assert_eq!(
            snapshot(&selection),
            (vec!["a", "b", "c"], Some("a"), Some("b"))
        );
        assert!(selection.move_cursor(CursorMove::Previous, &sorted, true));
        assert_eq!(snapshot(&selection), (vec!["a", "c"], Some("a"), Some("c")));
    }

    #[test]
    fn filtering_out_cursor_preserves_the_visible_anchor() {
        let mut selection = Selection::default();
        let original = ["a", "b", "c"];
        assert!(selection.click(&"a", &original, false, false));
        assert!(selection.click(&"c", &original, false, true));
        let filtered = ["b", "a"];
        selection.retain_visible(&filtered);
        assert_eq!(snapshot(&selection), (vec!["a", "b"], Some("a"), None));
        assert!(selection.move_cursor(CursorMove::Previous, &filtered, true));
        assert_eq!(snapshot(&selection), (vec!["a"], Some("a"), Some("a")));
    }

    #[test]
    fn filtering_out_anchor_preserves_cursor_and_range_click_fallback() {
        let mut selection = Selection::default();
        let original = ["a", "b", "c"];
        assert!(selection.click(&"a", &original, false, false));
        assert!(selection.click(&"c", &original, false, true));
        let filtered = ["c", "b"];
        selection.retain_visible(&filtered);
        assert_eq!(snapshot(&selection), (vec!["b", "c"], None, Some("c")));
        assert!(selection.move_cursor(CursorMove::Next, &filtered, true));
        assert_eq!(snapshot(&selection), (vec!["b"], None, Some("b")));
    }

    #[test]
    fn retaining_no_visible_paths_clears_selection_anchor_and_cursor() {
        let mut selection = Selection::default();
        assert!(selection.click(&"a", &["a", "b"], false, false));
        assert!(selection.click(&"b", &["a", "b"], false, true));
        selection.retain_visible(&[]);
        assert_eq!(snapshot(&selection), (vec![], None, None));
    }

    #[test]
    fn stale_cursor_is_treated_as_absent_without_using_selection_order() {
        let visible = ["z", "a", "b"];
        for (movement, destination) in [
            (CursorMove::Previous, "b"),
            (CursorMove::Next, "z"),
            (CursorMove::First, "z"),
            (CursorMove::Last, "b"),
        ] {
            for extend in [false, true] {
                let mut selection = Selection::default();
                assert!(selection.click(&"hidden", &["hidden"], false, false));
                assert!(selection.move_cursor(movement, &visible, extend));
                assert_eq!(
                    snapshot(&selection),
                    (
                        vec![destination],
                        Some(if extend { "hidden" } else { destination }),
                        Some(destination)
                    )
                );
            }
        }
    }

    #[test]
    fn empty_keyboard_moves_preserve_both_empty_and_historical_state() {
        let mut historical = Selection::default();
        assert!(historical.click(&"a", &["a", "b"], false, false));
        assert!(historical.click(&"b", &["a", "b"], false, true));
        for mut selection in [Selection::default(), historical] {
            let before = snapshot(&selection);
            for movement in [
                CursorMove::Previous,
                CursorMove::Next,
                CursorMove::First,
                CursorMove::Last,
            ] {
                for extend in [false, true] {
                    assert!(!selection.move_cursor(movement, &[], extend));
                    assert_eq!(snapshot(&selection), before);
                }
            }
        }
    }

    #[test]
    fn select_all_empty_succeeds_and_clears_paths_anchor_and_cursor() {
        let mut selection = Selection::default();
        assert!(selection.click(&"a", &["a", "b"], false, false));
        assert!(selection.click(&"b", &["a", "b"], false, true));
        assert!(selection.select_all(&[]));
        assert_eq!(snapshot(&selection), (vec![], None, None));
        assert!(selection.select_all(&[]));
        assert_eq!(snapshot(&selection), (vec![], None, None));
    }

    #[test]
    fn select_all_replaces_hidden_paths_and_uses_the_first_displayed_path() {
        let mut selection = Selection::default();
        assert!(selection.click(&"hidden", &["hidden"], false, false));
        assert!(selection.select_all(&["z", "a", "b"]));
        assert_eq!(
            snapshot(&selection),
            (vec!["a", "b", "z"], Some("z"), Some("z"))
        );
    }

    #[test]
    fn select_all_keeps_a_visible_cursor_and_reanchors_at_the_display_start() {
        let mut selection = Selection::default();
        let visible = ["z", "b", "a"];
        assert!(selection.click(&"b", &visible, false, false));
        assert!(selection.click(&"b", &visible, true, false));
        assert!(selection.select_all(&visible));
        assert_eq!(
            snapshot(&selection),
            (vec!["a", "b", "z"], Some("z"), Some("b"))
        );
        assert!(selection.move_cursor(CursorMove::First, &visible, true));
        assert_eq!(snapshot(&selection), (vec!["z"], Some("z"), Some("z")));
    }

    #[test]
    fn select_all_accepts_32_and_rejects_33_without_truncation_or_state_changes() {
        let mut selection = Selection::default();
        let visible: Vec<_> = (0..=MAX_SELECTION).rev().collect();
        assert!(selection.click(&25, &visible, false, false));
        assert!(selection.select_all(&visible[..MAX_SELECTION]));
        assert_eq!(
            snapshot(&selection),
            ((1..=MAX_SELECTION).collect(), Some(MAX_SELECTION), Some(25))
        );
        let before = snapshot(&selection);
        assert!(!selection.select_all(&visible));
        assert_eq!(snapshot(&selection), before);
    }

    #[test]
    fn oversized_toggle_click_preserves_paths_anchor_and_cursor() {
        let mut selection = Selection::default();
        let visible: Vec<_> = (0..=MAX_SELECTION).collect();
        assert!(selection.click(&7, &visible, false, false));
        assert!(selection.select_all(&visible[..MAX_SELECTION]));
        let before = snapshot(&selection);
        assert!(!selection.click(&MAX_SELECTION, &visible, true, false));
        assert_eq!(snapshot(&selection), before);
    }

    #[test]
    fn click_ranges_accept_32_and_reject_33_for_both_toggle_modes_atomically() {
        let mut selection = Selection::default();
        let visible: Vec<_> = (0..=MAX_SELECTION).collect();
        assert!(selection.click(&0, &visible, false, false));
        assert!(selection.click(&(MAX_SELECTION - 1), &visible, false, true));
        assert_eq!(
            snapshot(&selection),
            (
                (0..MAX_SELECTION).collect(),
                Some(0),
                Some(MAX_SELECTION - 1)
            )
        );
        let before = snapshot(&selection);
        for toggle in [false, true] {
            assert!(!selection.click(&MAX_SELECTION, &visible, toggle, true));
            assert_eq!(snapshot(&selection), before);
        }
    }

    #[test]
    fn keyboard_ranges_at_32_reject_the_33rd_row_in_both_directions_atomically() {
        let visible: Vec<_> = (0..=MAX_SELECTION).collect();
        for from_end in [false, true] {
            let mut selection = Selection::default();
            let (anchor, limited, accepted, adjacent, edge) = if from_end {
                (
                    MAX_SELECTION,
                    &visible[1..],
                    CursorMove::First,
                    CursorMove::Previous,
                    CursorMove::First,
                )
            } else {
                (
                    0,
                    &visible[..MAX_SELECTION],
                    CursorMove::Last,
                    CursorMove::Next,
                    CursorMove::Last,
                )
            };
            assert!(selection.click(&anchor, &visible, false, false));
            assert!(selection.move_cursor(accepted, limited, true));
            let cursor = if from_end { 1 } else { MAX_SELECTION - 1 };
            assert_eq!(
                snapshot(&selection),
                (limited.to_vec(), Some(anchor), Some(cursor))
            );
            let before = snapshot(&selection);
            for movement in [adjacent, edge] {
                assert!(!selection.move_cursor(movement, &visible, true));
                assert_eq!(snapshot(&selection), before);
            }
        }
    }

    #[test]
    fn single_visible_row_accepts_all_moves_without_wrapping_or_losing_anchor() {
        let mut selection = Selection::default();
        for movement in [
            CursorMove::Previous,
            CursorMove::Next,
            CursorMove::First,
            CursorMove::Last,
        ] {
            for extend in [false, true] {
                assert!(selection.move_cursor(movement, &["only"], extend));
                assert_eq!(
                    snapshot(&selection),
                    (vec!["only"], Some("only"), Some("only"))
                );
            }
        }
    }

    #[test]
    fn unicode_strings_keep_distinct_paths_and_follow_the_given_order() {
        let visible = [
            "远程/🦀.txt",
            "远程/é.txt",
            "远程/e\u{301}.txt",
            "远程/�.txt",
        ]
        .map(String::from);
        let mut selection = Selection::default();
        assert!(selection.click(&visible[1], &visible, false, false));
        assert!(selection.move_cursor(CursorMove::Next, &visible, true));
        assert_eq!(selection.len(), 2);
        assert!(selection.contains(&visible[1]));
        assert!(selection.contains(&visible[2]));
        assert_eq!(selection.anchor.as_ref(), Some(&visible[1]));
        assert_eq!(selection.cursor(), Some(&visible[2]));
        assert!(selection.select_all(&visible));
        assert_eq!(selection.len(), visible.len());
        assert!(visible.iter().all(|path| selection.contains(path)));
        assert_eq!(selection.anchor.as_ref(), Some(&visible[0]));
        assert_eq!(selection.cursor(), Some(&visible[2]));
    }

    fn assert_native_paths_remain_distinct_in_display_order(visible: &[std::path::PathBuf]) {
        let mut selection = Selection::default();
        assert!(selection.click(&visible[0], visible, false, false));
        assert!(selection.move_cursor(CursorMove::Next, visible, true));
        assert_eq!(selection.cursor(), Some(&visible[1]));
        assert!(selection.move_cursor(CursorMove::Next, visible, true));
        assert_eq!(selection.cursor(), Some(&visible[2]));
        assert_eq!(selection.anchor.as_ref(), Some(&visible[0]));
        assert_eq!(selection.len(), 3);
        assert!(visible[..3].iter().all(|path| selection.contains(path)));
        assert!(selection.select_all(visible));
        assert_eq!(selection.len(), visible.len());
        assert!(visible.iter().all(|path| selection.contains(path)));
        assert_eq!(selection.anchor.as_ref(), Some(&visible[0]));
        assert_eq!(selection.cursor(), Some(&visible[2]));
    }

    #[test]
    fn unicode_native_paths_preserve_composed_and_decomposed_names() {
        let visible = [
            "目录/🦀.txt",
            "目录/é.txt",
            "目录/e\u{301}.txt",
            "目录/�.txt",
        ]
        .map(std::path::PathBuf::from);
        assert_native_paths_remain_distinct_in_display_order(&visible);
    }

    #[cfg(unix)]
    #[test]
    fn unix_native_paths_preserve_invalid_utf8_without_lossy_collisions() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        let visible = [
            PathBuf::from(OsString::from_vec(b"directory/\xff".to_vec())),
            PathBuf::from("directory/�"),
            PathBuf::from(OsString::from_vec(b"directory/\xfe".to_vec())),
        ];
        assert_native_paths_remain_distinct_in_display_order(&visible);
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_paths_preserve_unpaired_surrogates_without_lossy_collisions() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let visible = [
            PathBuf::from(OsString::from_wide(&[0xd801])),
            PathBuf::from("�"),
            PathBuf::from(OsString::from_wide(&[0xd800])),
        ];
        assert_native_paths_remain_distinct_in_display_order(&visible);
    }
}
