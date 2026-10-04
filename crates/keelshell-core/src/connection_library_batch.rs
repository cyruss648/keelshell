//! Atomic metadata changes over an explicit set of saved connection identities.

use std::collections::HashSet;

use thiserror::Error;
use uuid::Uuid;

use crate::{AppState, Connection, DeletedConnection};

/// One operation applied uniformly to an explicitly reviewed profile selection.
/// These operations only edit metadata; they never open or close a transport or
/// erase a credential store entry or host trust record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionLibraryAction {
    /// Move active profiles to the given folder, or the unfiled root.
    Move(Option<Uuid>),
    /// Add distinct tags to each active profile, retaining its existing tags.
    AddTags(Vec<String>),
    /// Remove matching tags from each active profile.
    RemoveTags(Vec<String>),
    /// Explicitly replace all tags on each active profile; an empty list clears tags.
    ReplaceTags(Vec<String>),
    /// Set a uniform favorite state rather than toggling mixed values.
    Favorite(bool),
    /// Retain active profiles in trash with the supplied Unix timestamp.
    Trash(u64),
    /// Restore deleted profiles together, allowing a selected deleted jump chain.
    Restore,
    /// Permanently remove selected deleted profiles and their folder memberships.
    Purge,
}

/// A failed library transaction. Every failure leaves the input state unchanged.
#[derive(Debug, Error)]
pub enum ConnectionLibraryBatchError {
    /// A transaction must name at least one profile explicitly.
    #[error("select at least one profile")]
    EmptySelection,
    /// Repeated IDs are rejected instead of silently applying an action twice.
    #[error("profile {0} was selected more than once")]
    DuplicateSelection(Uuid),
    /// An ID is missing from the active or deleted collection required by the action.
    #[error("selected profile {0} is unavailable for this operation")]
    SelectionUnavailable(Uuid),
    /// A retained profile references a selected jump host. Select the dependent
    /// as well, or change its route before removing its jump host.
    #[error("profile {dependent} still references selected jump host {jump_host}")]
    JumpDependent {
        /// Selected jump host identity.
        jump_host: Uuid,
        /// Retained dependent identity.
        dependent: Uuid,
    },
    /// Restoration would recreate an already active normalized route.
    #[error("restored profile {0} would duplicate an active route")]
    DuplicateRoute(Uuid),
    /// Existing or final metadata violates the application's domain invariants.
    #[error(transparent)]
    InvalidState(#[from] crate::Error),
}

impl AppState {
    /// Apply one metadata operation to an explicit profile selection atomically.
    ///
    /// The complete final state is validated before replacement. Jump chains may
    /// be trashed, restored or purged together regardless of selection order;
    /// retained dependents block removal. Trash removes recent-success history,
    /// while purge also removes folder membership. Credential references and
    /// both host-trust maps are preserved by organization/trash/restore; purge
    /// removes only the discarded profile's reference, retaining vault and trust.
    /// Persist the resulting snapshot once through [`crate::StateStore`] on a worker.
    ///
    /// # Errors
    /// Returns a typed error for empty, duplicate or unavailable selections,
    /// retained jump dependents, duplicate restoration routes, invalid tags or
    /// other invalid state. Failure preserves all data and the snapshot revision.
    pub fn apply_connection_library_batch(
        &mut self,
        ids: &[Uuid],
        action: &ConnectionLibraryAction,
    ) -> Result<Vec<Connection>, ConnectionLibraryBatchError> {
        self.validate()?;
        if ids.is_empty() {
            return Err(ConnectionLibraryBatchError::EmptySelection);
        }
        let mut selected = HashSet::with_capacity(ids.len());
        let deleted = matches!(
            action,
            ConnectionLibraryAction::Restore | ConnectionLibraryAction::Purge
        );
        let mut affected = Vec::with_capacity(ids.len());
        for id in ids {
            if !selected.insert(*id) {
                return Err(ConnectionLibraryBatchError::DuplicateSelection(*id));
            }
            let profile = if deleted {
                self.deleted_connections
                    .iter()
                    .map(|entry| &entry.connection)
                    .find(|entry| entry.id == *id)
            } else {
                self.connections.iter().find(|entry| entry.id == *id)
            }
            .ok_or(ConnectionLibraryBatchError::SelectionUnavailable(*id))?;
            affected.push(profile.clone());
        }
        if matches!(
            action,
            ConnectionLibraryAction::Trash(_) | ConnectionLibraryAction::Purge
        ) {
            for profile in self.connections.iter().chain(
                self.deleted_connections
                    .iter()
                    .filter(|_| deleted)
                    .map(|entry| &entry.connection),
            ) {
                if !selected.contains(&profile.id)
                    && let Some(jump_host) = profile.jump_host.filter(|id| selected.contains(id))
                {
                    return Err(ConnectionLibraryBatchError::JumpDependent {
                        jump_host,
                        dependent: profile.id,
                    });
                }
            }
        }
        let mut candidate = self.clone();
        match action {
            ConnectionLibraryAction::Move(folder) => {
                let group = match folder {
                    Some(folder) => candidate.folder_path(*folder).ok_or_else(|| {
                        crate::Error::from(crate::ValidationError::new("folder", "was not found"))
                    })?,
                    None => String::new(),
                };
                for item in candidate
                    .connections
                    .iter_mut()
                    .filter(|item| selected.contains(&item.id))
                {
                    item.group.clone_from(&group);
                    if let Some(folder) = folder {
                        candidate.connection_folders.insert(item.id, *folder);
                    } else {
                        candidate.connection_folders.remove(&item.id);
                    }
                }
            }
            ConnectionLibraryAction::AddTags(tags)
            | ConnectionLibraryAction::RemoveTags(tags)
            | ConnectionLibraryAction::ReplaceTags(tags) => {
                // Validate supplied tags even when removal would otherwise ignore them.
                let mut probe = affected
                    .first()
                    .ok_or(ConnectionLibraryBatchError::EmptySelection)?
                    .clone();
                probe.tags = tags.clone();
                probe.validate().map_err(crate::Error::from)?;
                for profile in candidate
                    .connections
                    .iter_mut()
                    .filter(|entry| selected.contains(&entry.id))
                {
                    match action {
                        ConnectionLibraryAction::AddTags(_) => {
                            for tag in tags {
                                if !profile.tags.contains(tag) {
                                    profile.tags.push(tag.clone());
                                }
                            }
                        }
                        ConnectionLibraryAction::RemoveTags(_) => {
                            profile.tags.retain(|tag| !tags.contains(tag))
                        }
                        _ => profile.tags = tags.clone(),
                    }
                }
            }
            ConnectionLibraryAction::Favorite(favorite) => {
                for profile in candidate
                    .connections
                    .iter_mut()
                    .filter(|entry| selected.contains(&entry.id))
                {
                    profile.favorite = *favorite;
                }
            }
            ConnectionLibraryAction::Trash(deleted_at) => {
                candidate
                    .connections
                    .retain(|profile| !selected.contains(&profile.id));
                candidate
                    .recent_connections
                    .retain(|recent| !selected.contains(&recent.connection_id));
                candidate.deleted_connections.splice(
                    0..0,
                    affected
                        .iter()
                        .cloned()
                        .map(|connection| DeletedConnection {
                            connection,
                            deleted_at: *deleted_at,
                        }),
                );
            }
            ConnectionLibraryAction::Restore => {
                candidate
                    .deleted_connections
                    .retain(|entry| !selected.contains(&entry.connection.id));
                candidate.connections.extend(affected.iter().cloned());
                candidate.validate()?;
                let routes =
                    crate::routes::RouteIndex::new(&candidate).map_err(crate::Error::from)?;
                let mut identities = HashSet::new();
                // Existing duplicate routes are valid library data. Only newly
                // restored identities must be distinct from active and selected routes.
                for profile in &self.connections {
                    identities.insert(
                        routes
                            .resolve(profile.id, true)
                            .map_err(crate::Error::from)?
                            .identity()
                            .key()
                            .map_err(crate::Error::from)?,
                    );
                }
                for profile in &affected {
                    if !identities.insert(
                        routes
                            .resolve(profile.id, true)
                            .map_err(crate::Error::from)?
                            .identity()
                            .key()
                            .map_err(crate::Error::from)?,
                    ) {
                        return Err(ConnectionLibraryBatchError::DuplicateRoute(profile.id));
                    }
                }
            }
            ConnectionLibraryAction::Purge => {
                candidate
                    .deleted_connections
                    .retain(|entry| !selected.contains(&entry.connection.id));
                candidate
                    .connection_folders
                    .retain(|id, _| !selected.contains(id));
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(affected)
    }
}
