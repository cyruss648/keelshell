//! Folder organization, recoverable deletion and successful connection history.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState, Connection, Error, ImportReport, SCHEMA_VERSION, Settings, SnapshotRevision, Snippet,
    ValidationError,
    model::{MAX_CONNECTIONS, MAX_DOCUMENT_BYTES, schema, text, validate_connections},
    parse_openssh_config, parse_openssh_config_with_includes,
};

const MAX_FOLDERS: usize = 10_000;
const MAX_FOLDER_DEPTH: usize = 32;

/// Maximum number of persisted, distinct successful connection entries.
pub const MAX_RECENT_CONNECTIONS: usize = 50;

/// A named folder with stable identity, independent of its display path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionFolder {
    /// Folder identity. Folder and connection IDs belong to separate namespaces.
    pub id: Uuid,
    /// Folder label; sibling labels are unique and case-sensitive.
    pub name: String,
    /// Parent folder, or `None` for a root folder.
    pub parent_id: Option<Uuid>,
}

/// A deleted profile retained for explicit restoration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletedConnection {
    /// Original profile, including its identity and local credential reference.
    pub connection: Connection,
    /// Caller-supplied deletion time, in seconds since the Unix epoch.
    pub deleted_at: u64,
}

/// The last successful connection to one active profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentConnection {
    /// An active profile identifier; deleted profiles cannot appear here.
    pub connection_id: Uuid,
    /// Caller-supplied successful connection time, in seconds since the Unix epoch.
    pub connected_at: u64,
}

/// A flattened folder row for a hierarchical selector or sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    /// Stable folder identifier.
    pub id: Uuid,
    /// This folder's label, without its ancestor labels.
    pub name: String,
    /// Parent identity, or `None` for a root folder.
    pub parent_id: Option<Uuid>,
    /// Zero-based nesting depth; root folders have depth zero.
    pub depth: usize,
}

// Presence distinguishes legacy documents from intentionally empty new trees.
// In particular, null must not silently reactivate legacy group membership.
fn present_folders<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<ConnectionFolder>>, D::Error> {
    Vec::<ConnectionFolder>::deserialize(deserializer).map(Some)
}

impl<'de> Deserialize<'de> for AppState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireState {
            schema_version: u32,
            connections: Vec<Connection>,
            #[serde(default, deserialize_with = "present_folders")]
            folders: Option<Vec<ConnectionFolder>>,
            #[serde(default)]
            connection_folders: BTreeMap<Uuid, Uuid>,
            #[serde(default)]
            deleted_connections: Vec<DeletedConnection>,
            #[serde(default)]
            recent_connections: Vec<RecentConnection>,
            snippets: Vec<Snippet>,
            #[serde(default)]
            batch_audits: Vec<crate::BatchAuditRecord>,
            settings: Settings,
            #[serde(default)]
            known_hosts: BTreeMap<String, String>,
            #[serde(default)]
            route_known_hosts: BTreeMap<String, String>,
        }
        let wire = WireState::deserialize(deserializer)?;
        let legacy = wire.folders.is_none();
        let mut state = Self {
            schema_version: wire.schema_version,
            connections: wire.connections,
            folders: wire.folders.unwrap_or_default(),
            connection_folders: wire.connection_folders,
            deleted_connections: wire.deleted_connections,
            recent_connections: wire.recent_connections,
            snippets: wire.snippets,
            batch_audits: wire.batch_audits,
            settings: wire.settings,
            known_hosts: wire.known_hosts,
            route_known_hosts: wire.route_known_hosts,
            snapshot: SnapshotRevision::default(),
        };
        if legacy {
            state
                .migrate_legacy_groups()
                .map_err(serde::de::Error::custom)?;
        }
        Ok(state)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionExport {
    schema_version: u32,
    connections: Vec<Connection>,
    #[serde(default, deserialize_with = "present_folders")]
    folders: Option<Vec<ConnectionFolder>>,
    #[serde(default)]
    connection_folders: BTreeMap<Uuid, Uuid>,
}

impl AppState {
    /// Return a profile's folder, including profiles retained in the recycle bin.
    pub fn folder_id_of(&self, connection_id: Uuid) -> Option<Uuid> {
        self.connection_folders.get(&connection_id).copied()
    }

    /// Return a folder's display path, joining ancestor labels with `/`.
    /// Missing folders, cycles or excessive nesting return `None`.
    pub fn folder_path(&self, folder_id: Uuid) -> Option<String> {
        let index = self
            .folders
            .iter()
            .map(|folder| (folder.id, folder))
            .collect();
        path_in(&index, folder_id).ok()
    }

    /// Return every folder in name-sorted preorder, with roots at depth zero.
    /// Malformed disconnected cycles are omitted instead of causing an unbounded walk.
    pub fn folder_rows(&self) -> Vec<FolderRow> {
        let mut children: BTreeMap<Option<Uuid>, Vec<&ConnectionFolder>> = BTreeMap::new();
        for folder in &self.folders {
            children.entry(folder.parent_id).or_default().push(folder);
        }
        for siblings in children.values_mut() {
            siblings.sort_unstable_by(|left, right| {
                left.name.cmp(&right.name).then(left.id.cmp(&right.id))
            });
        }
        let mut pending: Vec<_> = children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .map(|folder| (*folder, 0))
            .collect();
        let mut visited = HashSet::new();
        let mut rows = Vec::with_capacity(self.folders.len());
        while let Some((folder, depth)) = pending.pop() {
            if depth >= MAX_FOLDER_DEPTH || !visited.insert(folder.id) {
                continue;
            }
            rows.push(FolderRow {
                id: folder.id,
                name: folder.name.clone(),
                parent_id: folder.parent_id,
                depth,
            });
            if let Some(descendants) = children.get(&Some(folder.id)) {
                pending.extend(descendants.iter().rev().map(|child| (*child, depth + 1)));
            }
        }
        rows
    }

    /// Whether a profile belongs to this folder or any descendant folder.
    /// Profiles retained in the recycle bin use their original membership.
    pub fn folder_contains(&self, folder_id: Uuid, connection_id: Uuid) -> bool {
        let mut current = self.folder_id_of(connection_id);
        for _ in 0..MAX_FOLDER_DEPTH {
            let Some(id) = current else {
                return false;
            };
            let Some(folder) = self.folders.iter().find(|folder| folder.id == id) else {
                return false;
            };
            if id == folder_id {
                return true;
            }
            current = folder.parent_id;
        }
        false
    }

    /// Create a folder beneath an existing parent or at the root.
    /// Empty labels, duplicate sibling labels, missing parents and bounds fail atomically.
    ///
    /// ```
    /// use keelshell_core::AppState;
    /// let mut state = AppState::default();
    /// let root = state.create_folder("Production", None)?;
    /// let child = state.create_folder("Database", Some(root))?;
    /// assert_eq!(state.folder_path(child).as_deref(), Some("Production/Database"));
    /// # Ok::<(), keelshell_core::Error>(())
    /// ```
    pub fn create_folder(
        &mut self,
        name: impl Into<String>,
        parent_id: Option<Uuid>,
    ) -> Result<Uuid, Error> {
        self.validate_connection_library()?;
        let folder = ConnectionFolder {
            id: Uuid::new_v4(),
            name: name.into().trim().to_owned(),
            parent_id,
        };
        let id = folder.id;
        let mut folders = self.folders.clone();
        folders.push(folder);
        self.replace_folders(folders)?;
        Ok(id)
    }

    /// Update a folder label and parent as one atomic edit.
    /// Only the final tree is validated, so a valid move-and-rename is not rejected
    /// because the old parent's sibling labels or path length would conflict.
    /// Related active and deleted profile group paths are updated together.
    pub fn update_folder(
        &mut self,
        id: Uuid,
        name: impl Into<String>,
        parent_id: Option<Uuid>,
    ) -> Result<(), Error> {
        self.validate_connection_library()?;
        let mut folders = self.folders.clone();
        let folder = folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or_else(folder_not_found)?;
        folder.name = name.into().trim().to_owned();
        folder.parent_id = parent_id;
        self.replace_folders(folders)
    }

    /// Rename a folder and update legacy group paths for active and deleted profiles.
    /// A duplicate sibling label or an overlong descendant path leaves all data unchanged.
    pub fn rename_folder(&mut self, id: Uuid, name: impl Into<String>) -> Result<(), Error> {
        let parent_id = self
            .folders
            .iter()
            .find(|folder| folder.id == id)
            .ok_or_else(folder_not_found)?
            .parent_id;
        self.update_folder(id, name, parent_id)
    }

    /// Move a folder beneath another folder or to the root.
    /// Self-parenting, descendant-parent cycles and duplicate sibling labels are rejected.
    pub fn move_folder(&mut self, id: Uuid, parent_id: Option<Uuid>) -> Result<(), Error> {
        let name = self
            .folders
            .iter()
            .find(|folder| folder.id == id)
            .ok_or_else(folder_not_found)?
            .name
            .clone();
        self.update_folder(id, name, parent_id)
    }

    /// Remove an empty folder. Child folders and active or deleted memberships block removal.
    /// Restore and move a deleted profile, or explicitly purge it, before deleting its folder.
    pub fn remove_folder(&mut self, id: Uuid) -> Result<ConnectionFolder, Error> {
        self.validate_connection_library()?;
        let index = self
            .folders
            .iter()
            .position(|folder| folder.id == id)
            .ok_or_else(folder_not_found)?;
        if self
            .folders
            .iter()
            .any(|folder| folder.parent_id == Some(id))
            || self
                .connection_folders
                .values()
                .any(|folder_id| *folder_id == id)
        {
            return Err(ValidationError::new(
                "folder",
                "must be empty, including deleted profiles",
            )
            .into());
        }
        Ok(self.folders.remove(index))
    }

    /// Move an active profile to a folder, or to the root with `None`.
    /// Its legacy `group` text is synchronized to the folder path, or cleared at the root.
    pub fn move_connection(&mut self, id: Uuid, folder_id: Option<Uuid>) -> Result<(), Error> {
        self.validate_connection_library()?;
        let group = match folder_id {
            Some(folder) => self.folder_path(folder).ok_or_else(folder_not_found)?,
            None => String::new(),
        };
        let connection = self
            .connections
            .iter_mut()
            .find(|connection| connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        connection.group = group;
        if let Some(folder) = folder_id {
            self.connection_folders.insert(id, folder);
        } else {
            self.connection_folders.remove(&id);
        }
        Ok(())
    }

    /// Move an active profile into the recycle bin without changing its ID or credential reference.
    /// Folder membership is retained; recent-success history is removed. This does not erase a vault entry.
    pub fn soft_delete_connection(
        &mut self,
        id: Uuid,
        deleted_at: u64,
    ) -> Result<Connection, Error> {
        self.validate_connection_library()?;
        self.ensure_no_jump_dependents(id, false)?;
        let index = self
            .connections
            .iter()
            .position(|connection| connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        let connection = self.connections.remove(index);
        self.recent_connections
            .retain(|recent| recent.connection_id != id);
        self.deleted_connections.insert(
            0,
            DeletedConnection {
                connection: connection.clone(),
                deleted_at,
            },
        );
        Ok(connection)
    }

    /// Restore a deleted profile with its original ID, folder and credential reference.
    /// Existing active IDs or complete normalized routes are never overwritten.
    /// Restoration does not claim a new successful connection or recreate recent history.
    pub fn restore_connection(&mut self, id: Uuid) -> Result<Uuid, Error> {
        self.validate_connection_library()?;
        let index = self
            .deleted_connections
            .iter()
            .position(|deleted| deleted.connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        let routes = crate::routes::RouteIndex::new(self)?;
        let original = routes.resolve(id, false)?.identity();
        let mut duplicate = false;
        for connection in &self.connections {
            if connection.id == id || routes.resolve(connection.id, true)?.identity() == original {
                duplicate = true;
                break;
            }
        }
        if duplicate {
            return Err(ValidationError::new(
                "connection.restore",
                "active identity or route already exists",
            )
            .into());
        }
        let mut candidate = self.clone();
        let deleted = candidate.deleted_connections.remove(index);
        candidate.connections.push(deleted.connection);
        candidate.validate_connection_library()?;
        *self = candidate;
        Ok(id)
    }

    /// Permanently discard one deleted profile and its folder membership.
    /// This only removes profile metadata; separately encrypted vault entries are not erased.
    pub fn purge_deleted_connection(&mut self, id: Uuid) -> Result<Connection, Error> {
        self.validate_connection_library()?;
        self.ensure_no_jump_dependents(id, true)?;
        let index = self
            .deleted_connections
            .iter()
            .position(|deleted| deleted.connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        self.connection_folders.remove(&id);
        Ok(self.deleted_connections.remove(index).connection)
    }

    /// Record a confirmed successful SSH connection, in seconds since the Unix epoch.
    /// Call this only after authentication and session setup succeed, never on an attempt.
    /// Entries are unique and newest success calls come first, even if the clock moves backward.
    /// Saving the modified state snapshot is the caller's responsibility.
    pub fn record_successful_connection(
        &mut self,
        id: Uuid,
        connected_at: u64,
    ) -> Result<(), Error> {
        self.validate_connection_library()?;
        if !self
            .connections
            .iter()
            .any(|connection| connection.id == id)
        {
            return Err(Error::ConnectionNotFound);
        }
        self.recent_connections
            .retain(|recent| recent.connection_id != id);
        self.recent_connections.insert(
            0,
            RecentConnection {
                connection_id: id,
                connected_at,
            },
        );
        self.recent_connections.truncate(MAX_RECENT_CONNECTIONS);
        Ok(())
    }

    /// Export active profiles and the complete folder tree, including empty folders.
    /// Recycle-bin entries, recent history, host trust, settings and vault references are excluded.
    /// Authentication method and private-key file paths remain explicit profile metadata;
    /// passwords, key contents and vault secrets are never part of this document.
    pub fn export_connections(&self) -> Result<String, Error> {
        self.validate_connection_library()?;
        let mut connections = self.connections.clone();
        for connection in &mut connections {
            connection.credential_ref = None;
        }
        let active: HashSet<_> = connections.iter().map(|connection| connection.id).collect();
        let output = serde_json::to_string_pretty(&ConnectionExport {
            schema_version: SCHEMA_VERSION,
            connections,
            folders: Some(self.folders.clone()),
            connection_folders: self
                .connection_folders
                .iter()
                .filter(|(id, _)| active.contains(id))
                .map(|(id, folder)| (*id, *folder))
                .collect(),
        })?;
        if output.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(output)
    }

    /// Import profiles and folders atomically, accepting legacy group-only documents.
    /// Matching active or deleted IDs and normalized routes are skipped. New profiles
    /// and folders receive fresh local IDs; equal sibling paths reuse existing folders.
    /// Local vault references are cleared while authentication method and key paths remain.
    /// Re-imports do not overwrite existing profiles or reactivate deleted ones.
    pub fn import_connections(&mut self, input: &str) -> Result<ImportReport, Error> {
        if input.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        let import: ConnectionExport = serde_json::from_str(input)?;
        schema(import.schema_version)?;
        if import.connections.len() > MAX_CONNECTIONS {
            return Err(ValidationError::new("connections", "at most 10000 are supported").into());
        }
        let legacy = import.folders.is_none();
        let mut incoming = AppState {
            connections: import.connections,
            folders: import.folders.unwrap_or_default(),
            connection_folders: import.connection_folders,
            ..AppState::default()
        };
        // Validate every supplied profile before collapsing repeated IDs, so an
        // invalid later entry cannot be hidden by an earlier accepted profile.
        let mut import_ids = HashMap::new();
        let mut unique = Vec::with_capacity(incoming.connections.len());
        let mut skipped = 0;
        for connection in incoming.connections {
            connection.validate()?;
            if let Some(previous) = import_ids.get(&connection.id) {
                if previous != &connection {
                    return Err(ValidationError::new(
                        "connection.id",
                        "repeated imported IDs must have identical definitions",
                    )
                    .into());
                }
                skipped += 1;
            } else {
                import_ids.insert(connection.id, connection.clone());
                unique.push(connection);
            }
        }
        incoming.connections = unique;
        if legacy {
            incoming.migrate_legacy_groups()?;
        }
        incoming.validate_connection_library()?;
        self.validate_connection_library()?;
        let mut candidate = self.clone();
        let mut mapped_folders = HashMap::new();
        for row in incoming.folder_rows() {
            let parent_id = match row.parent_id {
                Some(parent) => Some(*mapped_folders.get(&parent).ok_or_else(folder_not_found)?),
                None => None,
            };
            let local_id = match candidate
                .folders
                .iter()
                .find(|folder| folder.parent_id == parent_id && folder.name == row.name)
            {
                Some(folder) => folder.id,
                None => {
                    let id = Uuid::new_v4();
                    candidate.folders.push(ConnectionFolder {
                        id,
                        name: row.name,
                        parent_id,
                    });
                    id
                }
            };
            mapped_folders.insert(row.id, local_id);
        }
        let imported = crate::routes::import_profiles(&mut candidate, &incoming, &mapped_folders)?;
        skipped += imported.skipped;
        candidate.validate_connection_library()?;
        *self = candidate;
        Ok(ImportReport {
            added: imported.added,
            skipped,
        })
    }

    /// Import exact SSH profiles from an OpenSSH-style document.
    ///
    /// The document is parsed without filesystem or environment access.  Use
    /// [`Self::import_openssh_config_with_includes`] when Include contents have
    /// already been selected and read by a caller-owned worker.  Imported
    /// profiles receive fresh local IDs and never carry vault references.
    pub fn import_openssh_config(&mut self, input: &str) -> Result<ImportReport, Error> {
        Ok(self.import_openssh_config_report(input)?.imported)
    }

    /// Parse and import an OpenSSH document while preserving skipped items for review.
    pub fn import_openssh_config_report(
        &mut self,
        input: &str,
    ) -> Result<crate::OpenSshImportReport, Error> {
        let config = parse_openssh_config(input)?;
        let entries = config.entries.clone();
        let warnings = config.warnings.clone();
        let imported = self.import_parsed_openssh(config)?;
        Ok(crate::OpenSshImportReport {
            imported,
            entries,
            warnings,
        })
    }

    /// Import an OpenSSH-style document with caller-supplied Include contents.
    ///
    /// The map is matched literally by Include path; this method does not read
    /// files or expand paths.  Include resolution should therefore happen in a
    /// bounded background worker with an explicit user-selected file set.
    pub fn import_openssh_config_with_includes(
        &mut self,
        input: &str,
        includes: &BTreeMap<String, String>,
    ) -> Result<ImportReport, Error> {
        Ok(self
            .import_openssh_config_with_includes_report(input, includes)?
            .imported)
    }

    /// Parse and import with caller-selected Include contents and a review report.
    pub fn import_openssh_config_with_includes_report(
        &mut self,
        input: &str,
        includes: &BTreeMap<String, String>,
    ) -> Result<crate::OpenSshImportReport, Error> {
        let config = parse_openssh_config_with_includes(input, includes)?;
        let entries = config.entries.clone();
        let warnings = config.warnings.clone();
        let imported = self.import_parsed_openssh(config)?;
        Ok(crate::OpenSshImportReport {
            imported,
            entries,
            warnings,
        })
    }

    fn import_parsed_openssh(
        &mut self,
        config: crate::OpenSshConfig,
    ) -> Result<ImportReport, Error> {
        let mut incoming = AppState::default();
        let mut aliases = HashMap::with_capacity(config.entries.len());
        for entry in &config.entries {
            let id = entry.connection.id;
            if aliases.insert(entry.alias.clone(), id).is_some() {
                return Err(Error::OpenSshConfig {
                    line: entry.source_line,
                    reason: "duplicate Host aliases are ambiguous",
                });
            }
            incoming.connections.push(entry.connection.clone());
        }
        // Resolve jump aliases only after every profile has been indexed.  A
        // missing alias fails closed instead of silently becoming a direct hop.
        for (entry, connection) in config.entries.iter().zip(incoming.connections.iter_mut()) {
            if let Some(alias) = &entry.proxy_jump {
                connection.jump_host = Some(*aliases.get(alias).ok_or(Error::OpenSshConfig {
                    line: entry.source_line,
                    reason: "ProxyJump references an unknown Host alias",
                })?);
            }
        }
        incoming.validate_connection_library()?;
        self.validate_connection_library()?;
        let mut candidate = self.clone();
        let imported = crate::routes::import_profiles(&mut candidate, &incoming, &HashMap::new())?;
        candidate.validate_connection_library()?;
        *self = candidate;
        Ok(imported)
    }

    pub(crate) fn validate_connection_library(&self) -> Result<(), ValidationError> {
        let paths = validate_folders(&self.folders)?;
        validate_connections(&self.connections)?;
        if self.connections.len() + self.deleted_connections.len() > MAX_CONNECTIONS {
            return Err(ValidationError::new(
                "connections",
                "at most 10000 active and deleted profiles are supported",
            ));
        }
        let active: HashSet<_> = self
            .connections
            .iter()
            .map(|connection| connection.id)
            .collect();
        let mut ids = active.clone();
        for deleted in &self.deleted_connections {
            deleted.connection.validate()?;
            if !ids.insert(deleted.connection.id) {
                return Err(ValidationError::new(
                    "connection.id",
                    "active and deleted IDs must be unique",
                ));
            }
        }
        self.validate_connection_routes()?;
        for (connection_id, folder_id) in &self.connection_folders {
            if !ids.contains(connection_id) || !paths.contains_key(folder_id) {
                return Err(ValidationError::new(
                    "connection_folders",
                    "must reference existing profiles and folders",
                ));
            }
        }
        for connection in self.connections.iter().chain(
            self.deleted_connections
                .iter()
                .map(|entry| &entry.connection),
        ) {
            if let Some(folder_id) = self.connection_folders.get(&connection.id)
                && paths.get(folder_id) != Some(&connection.group)
            {
                return Err(ValidationError::new(
                    "connection.group",
                    "must match the assigned folder path",
                ));
            }
        }
        if self.recent_connections.len() > MAX_RECENT_CONNECTIONS {
            return Err(ValidationError::new(
                "recent_connections",
                "at most 50 are supported",
            ));
        }
        let mut recent_ids = HashSet::new();
        for recent in &self.recent_connections {
            if !active.contains(&recent.connection_id) || !recent_ids.insert(recent.connection_id) {
                return Err(ValidationError::new(
                    "recent_connections",
                    "must contain unique active profile IDs",
                ));
            }
        }
        Ok(())
    }

    fn replace_folders(&mut self, folders: Vec<ConnectionFolder>) -> Result<(), Error> {
        let paths = validate_folders(&folders)?;
        for connection in self.connections.iter_mut().chain(
            self.deleted_connections
                .iter_mut()
                .map(|entry| &mut entry.connection),
        ) {
            if let Some(path) = self
                .connection_folders
                .get(&connection.id)
                .and_then(|id| paths.get(id))
            {
                connection.group.clone_from(path);
            }
        }
        self.folders = folders;
        Ok(())
    }

    fn migrate_legacy_groups(&mut self) -> Result<(), ValidationError> {
        if !self.connection_folders.is_empty()
            || !self.deleted_connections.is_empty()
            || !self.recent_connections.is_empty()
        {
            return Err(ValidationError::new(
                "folders",
                "must be present when library metadata is supplied",
            ));
        }
        validate_connections(&self.connections)?;
        let mut groups: BTreeMap<String, Uuid> = BTreeMap::new();
        for connection in &self.connections {
            if !connection.group.is_empty() {
                // A deterministic representative makes repeated loads of the
                // same legacy bytes stable before the first migrated save.
                groups
                    .entry(connection.group.clone())
                    .and_modify(|id| *id = (*id).min(connection.id))
                    .or_insert(connection.id);
            }
        }
        self.folders = groups
            .iter()
            .map(|(name, id)| ConnectionFolder {
                id: *id,
                name: name.clone(),
                parent_id: None,
            })
            .collect();
        self.connection_folders = self
            .connections
            .iter()
            .filter_map(|connection| {
                groups
                    .get(&connection.group)
                    .map(|folder_id| (connection.id, *folder_id))
            })
            .collect();
        self.validate_connection_library()
    }
}

fn folder_not_found() -> ValidationError {
    ValidationError::new("folder.id", "must reference an existing folder")
}

fn validate_folders(
    folders: &[ConnectionFolder],
) -> Result<BTreeMap<Uuid, String>, ValidationError> {
    if folders.len() > MAX_FOLDERS {
        return Err(ValidationError::new(
            "folders",
            "at most 10000 are supported",
        ));
    }
    let mut index = HashMap::with_capacity(folders.len());
    let mut labels = HashSet::with_capacity(folders.len());
    for folder in folders {
        if folder.id.is_nil() || index.insert(folder.id, folder).is_some() {
            return Err(ValidationError::new(
                "folder.id",
                "must be non-nil and unique",
            ));
        }
        text("folder.name", &folder.name, 120, false)?;
        if !labels.insert((folder.parent_id, &folder.name)) {
            return Err(ValidationError::new(
                "folder.name",
                "must be unique among siblings",
            ));
        }
    }
    folders
        .iter()
        .map(|folder| Ok((folder.id, path_in(&index, folder.id)?)))
        .collect()
}

fn path_in(index: &HashMap<Uuid, &ConnectionFolder>, id: Uuid) -> Result<String, ValidationError> {
    let mut parts = Vec::new();
    let mut current = Some(id);
    let mut visited = HashSet::new();
    while let Some(id) = current {
        if !visited.insert(id) {
            return Err(ValidationError::new(
                "folder.parent_id",
                "must not form a cycle",
            ));
        }
        if parts.len() >= MAX_FOLDER_DEPTH {
            return Err(ValidationError::new(
                "folders",
                "at most 32 nesting levels are supported",
            ));
        }
        let folder = index.get(&id).ok_or_else(folder_not_found)?;
        parts.push(folder.name.as_str());
        current = folder.parent_id;
    }
    parts.reverse();
    let path = parts.join("/");
    text("folder.path", &path, 120, false)?;
    Ok(path)
}
