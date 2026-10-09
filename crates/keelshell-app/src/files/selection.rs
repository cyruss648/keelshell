//! Selection owns paths, never painted row indices or filesystem authority.

use std::collections::BTreeSet;

pub(super) const MAX_SELECTION: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct Selection<T> {
    paths: BTreeSet<T>,
    anchor: Option<T>,
}

impl<T> Default for Selection<T> {
    fn default() -> Self {
        Self {
            paths: BTreeSet::new(),
            anchor: None,
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
    pub(super) fn clear(&mut self) {
        self.paths.clear();
        self.anchor = None;
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
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
