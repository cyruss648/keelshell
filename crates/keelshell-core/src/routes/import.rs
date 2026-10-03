//! Resolve all imported identities before adding any profile or jump reference.

use std::collections::HashMap;

use uuid::Uuid;

use super::{RouteIndex, route_error};
use crate::{AppState, Connection, Error, ImportReport};

pub(crate) fn import_profiles(
    candidate: &mut AppState,
    incoming: &AppState,
    mapped_folders: &HashMap<Uuid, Uuid>,
) -> Result<ImportReport, Error> {
    let existing = RouteIndex::new(candidate)?;
    let source = RouteIndex::new(incoming)?;
    let mut existing_routes: HashMap<String, Vec<Uuid>> = HashMap::new();
    for id in existing.profiles.keys() {
        existing_routes
            .entry(existing.resolve(*id, false)?.identity.key()?)
            .or_default()
            .push(*id);
    }
    let mut mapped: HashMap<Uuid, Option<Uuid>> = HashMap::new();
    let mut pending: Vec<(Uuid, Connection)> = Vec::new();
    let mut new_routes: HashMap<String, (Uuid, Uuid)> = HashMap::new();
    let mut skipped = 0;
    for connection in &incoming.connections {
        let route = source.resolve(connection.id, true)?;
        let key = route.identity.key()?;
        // An ID collision can be skipped, but never authorizes rebinding an
        // imported jump edge to a different local endpoint or authentication.
        let reused = if existing.profiles.contains_key(&connection.id) {
            Some(vec![connection.id])
        } else {
            existing_routes.get(&key).cloned()
        };
        if let Some(matches) = reused {
            let local = if let [id] = matches.as_slice() {
                let local_route = existing.resolve(*id, false)?;
                (existing.active.contains(id)
                    && local_route.identity == route.identity
                    && local_route
                        .hops
                        .iter()
                        .map(|hop| &hop.auth)
                        .eq(route.hops.iter().map(|hop| &hop.auth)))
                .then_some(*id)
            } else {
                None
            };
            mapped.insert(connection.id, local);
            skipped += 1;
            continue;
        }
        if let Some((original, local)) = new_routes.get(&key) {
            let first = source.resolve(*original, true)?;
            let compatible = first
                .hops
                .iter()
                .map(|hop| &hop.auth)
                .eq(route.hops.iter().map(|hop| &hop.auth));
            mapped.insert(connection.id, compatible.then_some(*local));
            skipped += 1;
            continue;
        }
        let mut inserted = connection.clone();
        inserted.id = Uuid::new_v4();
        inserted.credential_ref = None;
        mapped.insert(connection.id, Some(inserted.id));
        new_routes.insert(key, (connection.id, inserted.id));
        pending.push((connection.id, inserted));
    }
    let added = pending.len();
    for (original, mut connection) in pending {
        connection.jump_host = match connection.jump_host {
            Some(jump) => Some(mapped.get(&jump).copied().flatten().ok_or_else(|| {
                route_error(
                    "imported jump host is unavailable or ambiguous; resolve its conflict first",
                )
            })?),
            None => None,
        };
        if let Some(folder) = incoming.connection_folders.get(&original) {
            let folder = mapped_folders
                .get(folder)
                .ok_or_else(|| route_error("imported folder mapping is missing"))?;
            candidate.connection_folders.insert(connection.id, *folder);
        }
        candidate.connections.push(connection);
    }
    Ok(ImportReport { added, skipped })
}
