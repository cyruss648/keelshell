//! Inert workspace metadata, separate from authenticated sessions and execution.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, PaneLayout, PaneNodeId, ValidationError};

/// Supported version of the optional workspace layout document.
pub const WORKSPACE_LAYOUT_VERSION: u32 = 1;
/// Maximum saved windows in one layout.
pub const MAX_LAYOUT_WINDOWS: usize = 8;
/// Maximum saved tabs in a single leaf pane.
pub const MAX_LAYOUT_TABS_PER_PANE: usize = 32;
/// Maximum saved tabs across all panes and windows.
pub const MAX_LAYOUT_TABS: usize = 128;

/// A saved, inactive tab reference. No endpoint, credential, command or output is stored.
///
/// A missing profile remains a placeholder. A one-time connection is represented
/// by `connection_id: None`; its endpoint and authentication are not persisted.
/// Repeated references to one profile are allowed, but tab IDs must be distinct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutTab {
    /// Stable identity, unique across every window of the saved workspace.
    pub id: Uuid,
    /// Saved profile reference, resolved again only when an explicit action needs it.
    pub connection_id: Option<Uuid>,
}

/// Whether an inactive tab still has a matching profile in the current library.
///
/// These states never imply authentication, trust, a live session or permission
/// to connect. They do not carry a transport or executable request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutTabAvailability {
    /// The current active connection library contains the referenced UUID.
    Available,
    /// The profile was removed, is in the trash, or is not in this library.
    Missing,
    /// No persistent profile was saved for this tab.
    Unbound,
}

impl LayoutTab {
    /// Resolve display availability against active profiles without changing state.
    ///
    /// Call [`WorkspaceLayout::validate`] before consuming untrusted layout data.
    /// A restored tab always starts inactive, including `Available` tabs.
    pub fn availability(&self, state: &AppState) -> LayoutTabAvailability {
        match self.connection_id {
            None => LayoutTabAvailability::Unbound,
            Some(id) if state.connections.iter().any(|profile| profile.id == id) => {
                LayoutTabAvailability::Available
            }
            Some(_) => LayoutTabAvailability::Missing,
        }
    }
}

/// Ordered inactive tabs belonging to exactly one leaf of a window's topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutPane {
    /// Leaf ID in this window's [`PaneLayout`]; split-node IDs are not allowed.
    pub pane: PaneNodeId,
    /// Ordered saved tabs, bounded by [`MAX_LAYOUT_TABS_PER_PANE`].
    pub tabs: Vec<LayoutTab>,
    /// Selected tab, or `None` exactly when the pane is empty.
    pub active_tab: Option<Uuid>,
}

/// One saved window, independent of native window handles and monitor geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutWindow {
    /// Stable window ID, unique within this workspace.
    pub id: Uuid,
    /// Bounded pane topology; node IDs are scoped to this window.
    pub layout: PaneLayout,
    /// Exactly one tab collection for every leaf pane, including empty panes.
    pub panes: Vec<LayoutPane>,
}

/// Validated layout metadata that can be saved through [`crate::StateStore`].
///
/// Loading this data does not restore a session or issue any action. Native
/// rendering, window creation and explicit reconnection are separate UI work.
/// There is intentionally no field for drafts, commands, output, transfer
/// queues, approved reviews, credentials, runtime session IDs or live tunnels.
///
/// Public fields allow callers to assemble a complete candidate; call
/// [`Self::validate`] before using it. StateStore validates before writing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLayout {
    /// Independent format version; unknown versions are rejected.
    pub version: u32,
    /// Saved windows in presentation order, between one and eight.
    pub windows: Vec<LayoutWindow>,
    /// Selected window ID, which must occur in `windows`.
    pub active_window: Uuid,
}

impl WorkspaceLayout {
    /// Validate topology, selection, bounds and all identity references.
    ///
    /// Unknown profile IDs are permitted so deleting a profile cannot corrupt
    /// the entire configuration; [`LayoutTab::availability`] reports them as
    /// missing. A UUID never authorizes connecting to its current endpoint.
    ///
    /// Returns a fixed, non-content-bearing [`ValidationError`] on failure.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != WORKSPACE_LAYOUT_VERSION {
            return Err(ValidationError::new(
                "workspace_layout.version",
                "unsupported workspace layout version",
            ));
        }
        if self.windows.is_empty() || self.windows.len() > MAX_LAYOUT_WINDOWS {
            return Err(ValidationError::new(
                "workspace_layout.windows",
                "must contain between 1 and 8 windows",
            ));
        }
        let mut windows = HashSet::new();
        let mut tabs = HashSet::new();
        let mut tab_count = 0;
        for window in &self.windows {
            if window.id.is_nil() || !windows.insert(window.id) {
                return Err(ValidationError::new(
                    "workspace_layout.window.id",
                    "must be non-nil and unique",
                ));
            }
            window.validate(&mut tabs, &mut tab_count)?;
        }
        if !windows.contains(&self.active_window) {
            return Err(ValidationError::new(
                "workspace_layout.active_window",
                "must identify a saved window",
            ));
        }
        Ok(())
    }
}

impl LayoutWindow {
    fn validate(
        &self,
        tab_ids: &mut HashSet<Uuid>,
        tab_count: &mut usize,
    ) -> Result<(), ValidationError> {
        // leaf_ids validates the graph before traversing it. The flat graph's
        // limits also bound this collection independently of serialized depth.
        let leaves = self.layout.leaf_ids()?;
        if self.panes.len() != leaves.len() {
            return Err(ValidationError::new(
                "workspace_layout.panes",
                "must cover every leaf exactly once",
            ));
        }
        let mut remaining: HashSet<_> = leaves.into_iter().collect();
        for pane in &self.panes {
            if !remaining.remove(&pane.pane) {
                return Err(ValidationError::new(
                    "workspace_layout.pane",
                    "must identify a distinct leaf",
                ));
            }
            pane.validate(tab_ids, tab_count)?;
        }
        Ok(())
    }
}

impl LayoutPane {
    fn validate(
        &self,
        tab_ids: &mut HashSet<Uuid>,
        tab_count: &mut usize,
    ) -> Result<(), ValidationError> {
        if self.tabs.len() > MAX_LAYOUT_TABS_PER_PANE {
            return Err(ValidationError::new(
                "workspace_layout.pane.tabs",
                "at most 32 tabs per pane are supported",
            ));
        }
        *tab_count += self.tabs.len();
        if *tab_count > MAX_LAYOUT_TABS {
            return Err(ValidationError::new(
                "workspace_layout.tabs",
                "at most 128 tabs per workspace are supported",
            ));
        }
        for tab in &self.tabs {
            if tab.id.is_nil() || !tab_ids.insert(tab.id) {
                return Err(ValidationError::new(
                    "workspace_layout.tab.id",
                    "must be non-nil and globally unique",
                ));
            }
            if tab.connection_id.is_some_and(|id| id.is_nil()) {
                return Err(ValidationError::new(
                    "workspace_layout.tab.connection_id",
                    "must be non-nil when present",
                ));
            }
        }
        match self.active_tab {
            None if self.tabs.is_empty() => Ok(()),
            Some(id) if self.tabs.iter().any(|tab| tab.id == id) => Ok(()),
            _ => Err(ValidationError::new(
                "workspace_layout.pane.active_tab",
                "must select a tab, or be absent exactly when the pane is empty",
            )),
        }
    }
}
