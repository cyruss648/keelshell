use keelshell_core::{
    AppState, Connection, ConnectionLibraryAction as Action,
    ConnectionLibraryBatchError as BatchError, Error, StateStore,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn chain() -> AppState {
    let mut jump = Connection::new("Gateway", "gateway.example.test", "operator");
    jump.credential_ref = Some(Uuid::new_v4());
    jump.tags = vec!["existing".into()];
    let mut target = Connection::new("Target", "target.example.test", "operator");
    target.jump_host = Some(jump.id);
    AppState {
        connections: vec![jump, target],
        ..Default::default()
    }
}

fn unchanged(state: &AppState, before: &AppState) {
    assert_eq!(state, before);
    assert_eq!(state.snapshot, before.snapshot);
}

#[test]
fn organization_preserves_routes_credentials_trust_and_unselected_profiles() -> TestResult {
    let mut state = chain();
    let ids: Vec<_> = state.connections.iter().map(|profile| profile.id).collect();
    let folder = state.create_folder("Operations", None)?;
    let other = Connection::new("Unselected", "other.example.test", "operator");
    state.connections.push(other.clone());
    let route = state.connection_route(ids[1])?;
    for index in 0..route.hops().len() {
        state.trust_host_key_for_scope(
            &route.host_key_scope(index).ok_or("scope")?,
            &format!("SHA256:{}", "A".repeat(43)),
        )?;
    }
    let original = state.clone();
    state.apply_connection_library_batch(&ids, &Action::Move(Some(folder)))?;
    state.apply_connection_library_batch(&ids, &Action::AddTags(vec!["organized".into()]))?;
    state.apply_connection_library_batch(&ids, &Action::Favorite(true))?;
    assert!(state.connections[..2].iter().all(|profile| profile.favorite
        && profile.group == "Operations"
        && profile.tags.contains(&"organized".into())));
    assert_eq!(
        state.connections[0].credential_ref,
        original.connections[0].credential_ref
    );
    assert_eq!(state.connection_route(ids[1])?.identity(), route.identity());
    assert_eq!(state.known_hosts, original.known_hosts);
    assert_eq!(state.route_known_hosts, original.route_known_hosts);
    assert_eq!(state.connections[2], other);
    state.apply_connection_library_batch(&ids, &Action::RemoveTags(vec!["organized".into()]))?;
    assert_eq!(state.connections[0].tags, vec!["existing"]);
    state.apply_connection_library_batch(&ids, &Action::ReplaceTags(Vec::new()))?;
    assert!(
        state.connections[..2]
            .iter()
            .all(|profile| profile.tags.is_empty())
    );
    Ok(())
}

#[test]
fn invalid_later_tag_result_rolls_back_every_profile_and_revision() -> TestResult {
    let mut state = chain();
    state.connections[1].tags = (0..32).map(|index| format!("tag-{index}")).collect();
    let ids: Vec<_> = state.connections.iter().map(|profile| profile.id).collect();
    let before = state.clone();
    assert!(
        state
            .apply_connection_library_batch(&ids, &Action::AddTags(vec!["overflow".into()]))
            .is_err()
    );
    unchanged(&state, &before);
    assert!(
        state
            .apply_connection_library_batch(&ids, &Action::Move(Some(Uuid::new_v4())))
            .is_err()
    );
    unchanged(&state, &before);
    assert!(
        state
            .apply_connection_library_batch(&ids, &Action::RemoveTags(vec![" bad ".into()]))
            .is_err()
    );
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn exact_selection_rejects_empty_duplicate_missing_and_wrong_collection() {
    let mut state = chain();
    let id = state.connections[0].id;
    let before = state.clone();
    assert!(matches!(
        state.apply_connection_library_batch(&[], &Action::Favorite(true)),
        Err(BatchError::EmptySelection)
    ));
    assert!(
        matches!(state.apply_connection_library_batch(&[id,id],&Action::Favorite(true)),Err(BatchError::DuplicateSelection(found)) if found==id)
    );
    assert!(matches!(
        state.apply_connection_library_batch(&[id, Uuid::new_v4()], &Action::Favorite(true)),
        Err(BatchError::SelectionUnavailable(_))
    ));
    assert!(
        matches!(state.apply_connection_library_batch(&[id],&Action::Purge),Err(BatchError::SelectionUnavailable(found)) if found==id)
    );
    unchanged(&state, &before);
}

#[test]
fn jump_chain_trash_restore_and_purge_are_atomic_and_order_independent() -> TestResult {
    let mut state = chain();
    let ids: Vec<_> = state
        .connections
        .iter()
        .rev()
        .map(|profile| profile.id)
        .collect();
    let original = state.clone();
    for id in &ids {
        state.record_successful_connection(*id, 10)?;
    }
    state.apply_connection_library_batch(&ids, &Action::Trash(20))?;
    assert!(state.connections.is_empty());
    assert!(state.recent_connections.is_empty());
    assert!(
        state
            .deleted_connections
            .iter()
            .all(|entry| entry.deleted_at == 20)
    );
    state.apply_connection_library_batch(&ids, &Action::Restore)?;
    assert!(state.deleted_connections.is_empty());
    for profile in &original.connections {
        assert!(state.connections.contains(profile));
    }
    assert!(state.recent_connections.is_empty());
    state.apply_connection_library_batch(&ids, &Action::Trash(30))?;
    state.apply_connection_library_batch(&ids, &Action::Purge)?;
    assert!(state.deleted_connections.is_empty());
    state.validate()?;
    Ok(())
}

#[test]
fn unselected_active_or_deleted_dependents_block_removal_without_partial_changes() -> TestResult {
    let mut state = chain();
    let jump = state.connections[0].id;
    let target = state.connections[1].id;
    let original = state.clone();
    assert!(
        matches!(state.apply_connection_library_batch(&[jump],&Action::Trash(20)),Err(BatchError::JumpDependent { jump_host,dependent }) if jump_host==jump && dependent==target)
    );
    unchanged(&state, &original);
    state.apply_connection_library_batch(&[target], &Action::Trash(20))?;
    state.apply_connection_library_batch(&[jump], &Action::Trash(21))?;
    let trashed = state.clone();
    assert!(
        matches!(state.apply_connection_library_batch(&[jump],&Action::Purge),Err(BatchError::JumpDependent { dependent,.. }) if dependent==target)
    );
    unchanged(&state, &trashed);
    state.apply_connection_library_batch(&[jump, target], &Action::Purge)?;
    Ok(())
}

#[test]
fn restoration_rejects_duplicate_routes_and_deleted_unselected_jumps() -> TestResult {
    let mut state = chain();
    let jump = state.connections[0].id;
    let target = state.connections[1].id;
    state.apply_connection_library_batch(&[jump, target], &Action::Trash(20))?;
    let before = state.clone();
    assert!(
        state
            .apply_connection_library_batch(&[target], &Action::Restore)
            .is_err()
    );
    unchanged(&state, &before);
    state.connections.push(Connection::new(
        "Other gateway",
        "gateway.example.test",
        "operator",
    ));
    let before = state.clone();
    assert!(matches!(
        state.apply_connection_library_batch(&[jump, target], &Action::Restore),
        Err(BatchError::DuplicateRoute(_))
    ));
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn purge_removes_membership_but_retains_vault_reference_owner_and_shared_trust() -> TestResult {
    let mut state = chain();
    let ids: Vec<_> = state.connections.iter().map(|profile| profile.id).collect();
    let folder = state.create_folder("Retained folder", None)?;
    let route = state.connection_route(ids[1])?;
    state.trust_host_key_for_scope(
        &route.host_key_scope(1).ok_or("scope")?,
        &format!("SHA256:{}", "A".repeat(43)),
    )?;
    let reference = state.connections[0].credential_ref;
    let mut other = Connection::new("Other", "other.example.test", "operator");
    other.credential_ref = reference;
    state.connections.push(other.clone());
    state.apply_connection_library_batch(&ids, &Action::Move(Some(folder)))?;
    let trust = state.route_known_hosts.clone();
    state.apply_connection_library_batch(&ids, &Action::Trash(10))?;
    state.apply_connection_library_batch(&ids, &Action::Purge)?;
    assert_eq!(state.connections, vec![other]);
    assert!(state.connection_folders.is_empty());
    assert_eq!(state.folders.len(), 1);
    assert_eq!(state.route_known_hosts, trust);
    state.remove_folder(folder)?;
    Ok(())
}

#[test]
fn one_store_save_advances_revision_once_and_rejects_external_changes() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut original = store.load()?;
    original.connections = chain().connections;
    let original = store.save(&original)?;
    let ids: Vec<_> = original
        .connections
        .iter()
        .map(|profile| profile.id)
        .collect();
    let mut candidate = original.clone();
    candidate.apply_connection_library_batch(&ids, &Action::Favorite(true))?;
    assert_eq!(candidate.snapshot, original.snapshot);
    let saved = store.save(&candidate)?;
    assert_ne!(saved.snapshot, original.snapshot);
    assert!(matches!(store.save(&candidate), Err(Error::Conflict)));
    let external = StateStore::new(store.path());
    let mut external_state = external.load()?;
    external_state.connections[0].name = "External change".into();
    external.save(&external_state)?;
    let bytes = std::fs::read(store.path())?;
    let mut next = saved;
    next.apply_connection_library_batch(&ids, &Action::Trash(10))?;
    assert!(matches!(store.save(&next), Err(Error::Conflict)));
    assert_eq!(std::fs::read(store.path())?, bytes);
    Ok(())
}

#[test]
fn unrelated_existing_duplicate_routes_do_not_block_restoration() -> TestResult {
    let mut state = chain();
    let target = state.connections[1].id;
    state.apply_connection_library_batch(&[target], &Action::Trash(10))?;
    state.connections.push(Connection::new(
        "Existing duplicate",
        "gateway.example.test",
        "operator",
    ));
    state.apply_connection_library_batch(&[target], &Action::Restore)?;
    assert!(state.connection_route(target).is_ok());
    Ok(())
}
