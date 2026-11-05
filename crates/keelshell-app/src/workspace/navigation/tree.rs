//! Stable connection-tree identities and process-local browsing preferences.
use gpui_kit::{App, FocusHandle, ScrollHandle};
use keelshell_core::AppState;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::workspace) enum NodeId {
    Unfiled,
    Folder(Uuid),
    Connection(Uuid),
}

impl NodeId {
    pub(in crate::workspace) fn element_id(self) -> String {
        match self {
            Self::Unfiled => "navigation-unfiled".into(),
            Self::Folder(id) => format!("navigation-folder-{id}"),
            Self::Connection(id) => format!("navigation-connection-{id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::workspace) struct Entry {
    pub(in crate::workspace) id: NodeId,
    pub(in crate::workspace) parent: Option<NodeId>,
    pub(in crate::workspace) depth: usize,
    pub(in crate::workspace) name: String,
    pub(in crate::workspace) has_children: bool,
    pub(in crate::workspace) expanded: bool,
}

pub(in crate::workspace) struct NavigationTree {
    pub(in crate::workspace) focus: FocusHandle,
    pub(in crate::workspace) scroll: ScrollHandle,
    pub(in crate::workspace) cursor: Option<NodeId>,
    pub(in crate::workspace) visible: Vec<Entry>,
    // Activation belongs to a released key and the same visible identity. A held
    // Enter must not focus a shell early and feed its repeat events to that shell.
    pub(in crate::workspace) armed: Option<(String, NodeId)>,
    // Ownership outlives focus and proposal invalidation. A mouse click may move
    // focus while these keys are still physically held; only KeyUp releases them.
    pub(in crate::workspace) held: BTreeSet<String>,
    collapsed: BTreeSet<NodeId>,
    filtered_collapsed: BTreeSet<NodeId>,
    query: String,
}

impl NavigationTree {
    pub(in crate::workspace) fn new(cx: &App) -> Self {
        Self {
            focus: cx.focus_handle().tab_index(0).tab_stop(true),
            scroll: ScrollHandle::new(),
            cursor: None,
            visible: Vec::new(),
            armed: None,
            held: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            filtered_collapsed: BTreeSet::new(),
            query: String::new(),
        }
    }

    /// Reconcile by UUID, never by a row index that could now name another host.
    pub(in crate::workspace) fn synchronize(&mut self, state: &AppState, query: &str) {
        if self.query != query {
            self.query = query.into();
            self.filtered_collapsed.clear();
            self.armed = None;
        }
        let folders = state.folder_rows();
        self.collapsed.retain(|node| match node {
            NodeId::Unfiled => true,
            NodeId::Folder(id) => folders.iter().any(|row| row.id == *id),
            NodeId::Connection(_) => false,
        });
        let all = entries(state, query);
        let collapsed = if query.trim().is_empty() {
            &self.collapsed
        } else {
            &self.filtered_collapsed
        };
        let mut visible: Vec<Entry> = Vec::new();
        for mut entry in all.iter().cloned() {
            if entry
                .parent
                .is_some_and(|parent| !visible.iter().any(|row| row.id == parent && row.expanded))
            {
                continue;
            }
            entry.expanded = entry.has_children && !collapsed.contains(&entry.id);
            visible.push(entry);
        }
        if self.visible != visible {
            self.armed = None;
        }
        let previous_cursor = self.cursor;
        let mut cursor = previous_cursor;
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if visible.iter().any(|entry| entry.id == id) {
                break;
            }
            if !visited.insert(id) {
                cursor = None;
                break;
            }
            cursor = all
                .iter()
                .find(|entry| entry.id == id)
                .or_else(|| self.visible.iter().find(|entry| entry.id == id))
                .and_then(|entry| entry.parent);
        }
        self.cursor = cursor.or_else(|| visible.first().map(|entry| entry.id));
        self.visible = visible;
        if self.cursor != previous_cursor
            && let Some(id) = self.cursor
        {
            self.select(id);
        }
    }

    pub(in crate::workspace) fn toggle(&mut self, id: NodeId) {
        if !self
            .visible
            .iter()
            .any(|entry| entry.id == id && entry.has_children)
        {
            return;
        }
        let collapsed = if self.query.trim().is_empty() {
            &mut self.collapsed
        } else {
            &mut self.filtered_collapsed
        };
        if !collapsed.remove(&id) {
            collapsed.insert(id);
        }
        self.cursor = Some(id);
        self.armed = None;
    }

    pub(in crate::workspace) fn select(&mut self, id: NodeId) {
        if let Some(index) = self.visible.iter().position(|entry| entry.id == id) {
            self.cursor = Some(id);
            self.armed = None;
            self.scroll.scroll_to_item(index);
        }
    }
}

fn entries(state: &AppState, query: &str) -> Vec<Entry> {
    let mut profiles: Vec<_> = state
        .connections
        .iter()
        .filter(|p| p.matches(query))
        .collect();
    profiles
        .sort_unstable_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    let mut entries = Vec::new();
    let unfiled: Vec<_> = profiles
        .iter()
        .filter(|p| state.folder_id_of(p.id).is_none())
        .collect();
    if !unfiled.is_empty() {
        entries.push(Entry {
            id: NodeId::Unfiled,
            parent: None,
            depth: 0,
            name: String::new(),
            has_children: true,
            expanded: true,
        });
        for profile in unfiled {
            entries.push(Entry {
                id: NodeId::Connection(profile.id),
                parent: Some(NodeId::Unfiled),
                depth: 1,
                name: profile.name.clone(),
                has_children: false,
                expanded: false,
            });
        }
    }
    let folders = state.folder_rows();
    for folder in &folders {
        if !query.trim().is_empty()
            && !profiles
                .iter()
                .any(|p| state.folder_contains(folder.id, p.id))
        {
            continue;
        }
        entries.push(Entry {
            id: NodeId::Folder(folder.id),
            parent: folder.parent_id.map(NodeId::Folder),
            depth: folder.depth,
            name: folder.name.clone(),
            has_children: folders
                .iter()
                .any(|child| child.parent_id == Some(folder.id))
                || profiles
                    .iter()
                    .any(|p| state.folder_id_of(p.id) == Some(folder.id)),
            expanded: true,
        });
        for profile in profiles
            .iter()
            .filter(|p| state.folder_id_of(p.id) == Some(folder.id))
        {
            entries.push(Entry {
                id: NodeId::Connection(profile.id),
                parent: Some(NodeId::Folder(folder.id)),
                depth: folder.depth + 1,
                name: profile.name.clone(),
                has_children: false,
                expanded: false,
            });
        }
    }
    entries
}
