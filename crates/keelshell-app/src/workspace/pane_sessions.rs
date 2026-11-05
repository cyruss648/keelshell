//! Runtime keys assigned to pane leaves; the workspace still owns the sessions.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use keelshell_core::{
    MAX_LAYOUT_DEPTH, MAX_LAYOUT_PANES, MAX_LAYOUT_TABS, MAX_LAYOUT_TABS_PER_PANE, PaneLayout,
    PaneNode, PaneNodeId, SplitAxis, ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

#[cfg(test)]
mod tests;

/// A rejected session mapping or topology change, with no partial mutation.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(super) enum PaneSessionError {
    /// The authoritative session list contains the same key more than once.
    #[error("session keys must be unique")]
    DuplicateSession,
    /// The requested active key is absent from the authoritative session list.
    #[error("active session must be present in the session list")]
    UnknownActive,
    /// An operation requires a session key that is not registered.
    #[error("session is not registered")]
    UnknownSession,
    /// A split requires two distinct keys.
    #[error("split sessions must be distinct")]
    SameSession,
    /// An existing split peer belongs to a different pane.
    #[error("registered split sessions must belong to the same pane")]
    DifferentPanes,
    /// The change would exceed the workspace session limit.
    #[error("session count exceeds the workspace limit")]
    SessionLimit,
    /// The change would exceed a leaf's tab limit.
    #[error("session count exceeds the pane limit")]
    PaneTabLimit,
    /// The requested pane has no leaf tab collection.
    #[error("pane must identify an existing leaf")]
    UnknownPane,
    /// The core topology rejected the target, ratio, identity or geometry.
    #[error(transparent)]
    Layout(#[from] ValidationError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PaneTabs<K> {
    tabs: Vec<K>,
    selected: Option<K>,
}

impl<K: Copy + Eq> PaneTabs<K> {
    fn remove(&mut self, key: K) -> Result<(), PaneSessionError> {
        let index = self
            .tabs
            .iter()
            .position(|tab| *tab == key)
            .ok_or(PaneSessionError::UnknownSession)?;
        self.tabs.remove(index);
        if self.selected == Some(key) {
            self.selected = self.tabs.get(index).or_else(|| self.tabs.last()).copied();
        }
        Ok(())
    }
}

/// Bounded, ephemeral ownership and selection for session keys in a valid layout.
///
/// Each key occurs in exactly one leaf. Nonempty leaves always select one of
/// their keys; only the final leaf may be empty. No entity or transport is owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SessionPanes<K: Copy + Eq + Hash> {
    layout: PaneLayout,
    panes: HashMap<PaneNodeId, PaneTabs<K>>,
}

impl<K: Copy + Eq + Hash> SessionPanes<K> {
    /// Create one empty active leaf with a non-nil UUID.
    pub(super) fn new() -> Self {
        let pane = PaneNodeId(Uuid::new_v4());
        // A v4 UUID is non-nil by construction, so this single-leaf layout
        // needs no fallible constructor or panic-based error handling.
        Self {
            layout: PaneLayout {
                root: pane,
                active: pane,
                nodes: vec![PaneNode::Leaf { id: pane }],
            },
            panes: HashMap::from([(
                pane,
                PaneTabs {
                    tabs: Vec::new(),
                    selected: None,
                },
            )]),
        }
    }

    /// Borrow the topology without exposing mutable layout state.
    pub(super) fn layout(&self) -> &PaneLayout {
        &self.layout
    }

    /// Return the active leaf's stable identity.
    pub(super) fn active_pane(&self) -> PaneNodeId {
        self.layout.active
    }

    /// Return the active leaf's selected key, or `None` for the empty workspace.
    pub(super) fn active_session(&self) -> Option<K> {
        self.selected_in(self.active_pane())
    }

    /// Find the sole leaf owning a registered key.
    pub(super) fn pane_for(&self, key: K) -> Option<PaneNodeId> {
        self.panes
            .iter()
            .find_map(|(id, pane)| pane.tabs.contains(&key).then_some(*id))
    }

    /// Return a leaf's ordered tabs, or an empty slice for an unknown/non-leaf ID.
    pub(super) fn tabs_in(&self, pane: PaneNodeId) -> &[K] {
        self.panes
            .get(&pane)
            .map_or(&[], |pane| pane.tabs.as_slice())
    }

    /// Return a leaf's selected key, or `None` for an empty/unknown/non-leaf ID.
    pub(super) fn selected_in(&self, pane: PaneNodeId) -> Option<K> {
        self.panes.get(&pane).and_then(|pane| pane.selected)
    }

    /// Return one selected key per nonempty leaf in geometric leaf order.
    #[cfg(test)]
    pub(super) fn displayed_sessions(&self) -> Vec<K> {
        // All stored topologies are valid; traversal must use core's geometric
        // order rather than the hash table or the flat node storage order.
        self.layout
            .leaf_ids()
            .into_iter()
            .flatten()
            .filter_map(|pane| self.selected_in(pane))
            .collect()
    }

    /// Reconcile the authoritative keys, retaining surviving owners and tab order.
    ///
    /// Removed selections use the next surviving tab, then the preceding tab.
    /// Empty leaves close before new keys are appended to the resulting active
    /// leaf in input order. `active`, when present, selects its owner last;
    /// `None` preserves the surviving selection or its removal fallback.
    ///
    /// Rejects duplicates, an unknown active key or exceeded limits atomically.
    pub(super) fn reconcile(
        &mut self,
        sessions: &[K],
        active: Option<K>,
    ) -> Result<(), PaneSessionError> {
        if sessions.len() > MAX_LAYOUT_TABS {
            return Err(PaneSessionError::SessionLimit);
        }
        let mut live = HashSet::with_capacity(sessions.len());
        for key in sessions {
            if !live.insert(*key) {
                return Err(PaneSessionError::DuplicateSession);
            }
        }
        if active.is_some_and(|key| !live.contains(&key)) {
            return Err(PaneSessionError::UnknownActive);
        }

        // The bounded copy keeps failed capacity checks from publishing removals,
        // fallback selections or a partly collapsed topology.
        let mut candidate = self.clone();
        for pane in candidate.panes.values_mut() {
            let removed: Vec<_> = pane
                .tabs
                .iter()
                .copied()
                .filter(|key| !live.contains(key))
                .collect();
            for key in removed {
                pane.remove(key)?;
            }
        }
        for pane in candidate.layout.leaf_ids()? {
            if candidate.panes.len() > 1 && candidate.tabs_in(pane).is_empty() {
                candidate.layout.close(pane)?;
                candidate.panes.remove(&pane);
            }
        }
        let added: Vec<_> = sessions
            .iter()
            .copied()
            .filter(|key| candidate.pane_for(*key).is_none())
            .collect();
        let pane = candidate
            .panes
            .get_mut(&candidate.layout.active)
            .ok_or(PaneSessionError::UnknownPane)?;
        if pane.tabs.len() + added.len() > MAX_LAYOUT_TABS_PER_PANE {
            return Err(PaneSessionError::PaneTabLimit);
        }
        pane.tabs.extend(added);
        if pane.selected.is_none() {
            pane.selected = pane.tabs.first().copied();
        }
        if let Some(key) = active
            && !candidate.select(key)
        {
            return Err(PaneSessionError::UnknownActive);
        }
        *self = candidate;
        Ok(())
    }

    /// Select a registered key and its owner; unknown keys leave state unchanged.
    pub(super) fn select(&mut self, key: K) -> bool {
        let Some(owner) = self.pane_for(key) else {
            return false;
        };
        let Some(pane) = self.panes.get_mut(&owner) else {
            return false;
        };
        if self.layout.set_active(owner).is_err() {
            return false;
        }
        pane.selected = Some(key);
        true
    }

    /// Whether one more key fits in the active leaf and the workspace.
    pub(super) fn can_add(&self) -> bool {
        self.session_count() < MAX_LAYOUT_TABS
            && self
                .panes
                .get(&self.active_pane())
                .is_some_and(|pane| pane.tabs.len() < MAX_LAYOUT_TABS_PER_PANE)
    }

    /// Preflight only the primary owner's leaf count and depth for a split.
    ///
    /// Tab limits do not affect this geometric check, even for a full source leaf.
    pub(super) fn can_split(&self, primary: K) -> bool {
        let Some(pane) = self.pane_for(primary) else {
            return false;
        };
        if self.panes.len() >= MAX_LAYOUT_PANES {
            return false;
        }
        let mut pending = vec![(self.layout.root, 0)];
        while let Some((id, depth)) = pending.pop() {
            if id == pane {
                return depth < MAX_LAYOUT_DEPTH;
            }
            if let Some(PaneNode::Split { first, second, .. }) =
                self.layout.nodes.iter().find(|node| node.id() == id)
            {
                pending.push((*second, depth + 1));
                pending.push((*first, depth + 1));
            }
        }
        false
    }

    /// Split at 50%, retaining `primary` as the active key in its original leaf.
    ///
    /// `peer` may be new or already in the same leaf; it alone occupies the new
    /// leaf. A new peer consumes only workspace capacity, not source-leaf space.
    /// Rejects unknown primary, equal keys, a peer in another leaf, or exceeded
    /// limits without changing the original mapping.
    pub(super) fn split(
        &mut self,
        primary: K,
        peer: K,
        axis: SplitAxis,
    ) -> Result<PaneNodeId, PaneSessionError> {
        if primary == peer {
            return Err(PaneSessionError::SameSession);
        }
        let owner = self
            .pane_for(primary)
            .ok_or(PaneSessionError::UnknownSession)?;
        let peer_owner = self.pane_for(peer);
        if peer_owner.is_some_and(|pane| pane != owner) {
            return Err(PaneSessionError::DifferentPanes);
        }
        if peer_owner.is_none() && self.session_count() >= MAX_LAYOUT_TABS {
            return Err(PaneSessionError::SessionLimit);
        }

        let mut candidate = self.clone();
        let new_pane = PaneNodeId(Uuid::new_v4());
        candidate
            .layout
            .split(owner, new_pane, PaneNodeId(Uuid::new_v4()), axis, 5000)?;
        candidate.layout.set_active(owner)?;
        let original = candidate
            .panes
            .get_mut(&owner)
            .ok_or(PaneSessionError::UnknownPane)?;
        if peer_owner.is_some() {
            original.remove(peer)?;
        }
        original.selected = Some(primary);
        candidate.panes.insert(
            new_pane,
            PaneTabs {
                tabs: vec![peer],
                selected: Some(peer),
            },
        );
        *self = candidate;
        Ok(new_pane)
    }

    /// Set a split ratio using core's validation and atomic failure semantics.
    pub(super) fn set_ratio(
        &mut self,
        split_node: PaneNodeId,
        ratio_bps: u16,
    ) -> Result<(), PaneSessionError> {
        self.layout.set_ratio(split_node, ratio_bps)?;
        Ok(())
    }

    /// Replace a retired runtime key in place, preserving geometry and selection.
    ///
    /// Reconnection creates a new transport identity; it must not move the tab
    /// to the currently active pane or inherit the retired transport's grants.
    pub(super) fn replace(&mut self, old: K, new: K) -> Result<(), PaneSessionError> {
        let owner = self.pane_for(old).ok_or(PaneSessionError::UnknownSession)?;
        if old == new {
            return Ok(());
        }
        if self.pane_for(new).is_some() {
            return Err(PaneSessionError::DuplicateSession);
        }
        let pane = self
            .panes
            .get_mut(&owner)
            .ok_or(PaneSessionError::UnknownPane)?;
        let tab = pane
            .tabs
            .iter_mut()
            .find(|key| **key == old)
            .ok_or(PaneSessionError::UnknownSession)?;
        *tab = new;
        if pane.selected == Some(old) {
            pane.selected = Some(new);
        }
        Ok(())
    }

    /// Remove a registered key and return the resulting active selection.
    ///
    /// A removed selection prefers its same-index successor, then its predecessor.
    /// Empty leaves close except for the final leaf. Unknown keys fail atomically.
    pub(super) fn remove(&mut self, key: K) -> Result<Option<K>, PaneSessionError> {
        let owner = self.pane_for(key).ok_or(PaneSessionError::UnknownSession)?;
        let mut candidate = self.clone();
        let pane = candidate
            .panes
            .get_mut(&owner)
            .ok_or(PaneSessionError::UnknownPane)?;
        pane.remove(key)?;
        if pane.tabs.is_empty() && candidate.panes.len() > 1 {
            candidate.layout.close(owner)?;
            candidate.panes.remove(&owner);
        }
        let active = candidate.active_session();
        *self = candidate;
        Ok(active)
    }

    /// Collapse a leaf into its sibling subtree's first leaf without closing keys.
    ///
    /// Moved tabs append in order and the original active key stays active, even
    /// when moved. Rejects the final leaf, unknown targets and destination overflow
    /// without changing topology, tab order or selections.
    pub(super) fn collapse(&mut self, pane: PaneNodeId) -> Result<(), PaneSessionError> {
        if !self.panes.contains_key(&pane) {
            return Err(PaneSessionError::UnknownPane);
        }
        let active = self.active_session();
        let mut candidate = self.clone();
        // Closing a temporarily active target asks core for the adjacent first
        // leaf even when the real active key belongs elsewhere in the layout.
        candidate.layout.set_active(pane)?;
        candidate.layout.close(pane)?;
        let removed = candidate
            .panes
            .remove(&pane)
            .ok_or(PaneSessionError::UnknownPane)?;
        let destination = candidate
            .panes
            .get_mut(&candidate.layout.active)
            .ok_or(PaneSessionError::UnknownPane)?;
        if destination.tabs.len() + removed.tabs.len() > MAX_LAYOUT_TABS_PER_PANE {
            return Err(PaneSessionError::PaneTabLimit);
        }
        destination.tabs.extend(removed.tabs);
        if destination.selected.is_none() {
            destination.selected = destination.tabs.first().copied();
        }
        if let Some(key) = active
            && !candidate.select(key)
        {
            return Err(PaneSessionError::UnknownSession);
        }
        *self = candidate;
        Ok(())
    }

    fn session_count(&self) -> usize {
        self.panes.values().map(|pane| pane.tabs.len()).sum()
    }
}
