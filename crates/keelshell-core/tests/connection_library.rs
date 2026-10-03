use std::{fs, io::Write};

use keelshell_core::{
    AppState, AuthMethod, Connection, ConnectionFolder, Error, MAX_RECENT_CONNECTIONS,
    RecentConnection, StateStore,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn add_profile(state: &mut AppState, name: &str, host: &str) -> Uuid {
    let connection = Connection::new(name, host, "operator");
    let id = connection.id;
    state.connections.push(connection);
    id
}

fn legacy_document(state: &AppState) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = serde_json::to_value(state)?;
    if let Some(object) = value.as_object_mut() {
        for field in [
            "folders",
            "connection_folders",
            "deleted_connections",
            "recent_connections",
        ] {
            object.remove(field);
        }
    }
    Ok(value)
}

#[test]
fn legacy_groups_migrate_stably_without_changing_profile_identity_or_authentication() -> TestResult
{
    let mut first = Connection::new("生产节点", "first.example.test", "operator");
    first.group = "生产/关键服务".into();
    first.auth = AuthMethod::PrivateKey {
        path: "keys/production".into(),
    };
    first.credential_ref = Some(Uuid::new_v4());
    first.favorite = true;
    first.tags = vec!["核心".into()];
    let mut second = Connection::new("备用", "second.example.test", "operator");
    second.group.clone_from(&first.group);
    let root = Connection::new("Root", "root.example.test", "operator");
    let original = AppState {
        connections: vec![first.clone(), second.clone(), root.clone()],
        ..AppState::default()
    };
    let value = legacy_document(&original)?;
    let migrated: AppState = serde_json::from_value(value.clone())?;
    let repeated: AppState = serde_json::from_value(value)?;
    migrated.validate()?;
    assert_eq!(migrated, repeated);
    assert_eq!(migrated.connections, original.connections);
    assert_eq!(migrated.folders.len(), 1);
    assert_eq!(migrated.folders[0].name, "生产/关键服务");
    assert_eq!(
        migrated.folder_id_of(first.id),
        migrated.folder_id_of(second.id)
    );
    assert!(migrated.folder_id_of(root.id).is_none());
    assert!(migrated.deleted_connections.is_empty());
    assert!(migrated.recent_connections.is_empty());
    Ok(())
}

#[test]
fn explicit_empty_tree_never_reactivates_legacy_group_labels() -> TestResult {
    let mut state = AppState::default();
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.connections[0].group = "Historical label".into();
    let loaded: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert!(loaded.folders.is_empty());
    assert!(loaded.folder_id_of(id).is_none());
    let mut value = serde_json::to_value(&state)?;
    value["folders"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AppState>(value).is_err());
    Ok(())
}

#[test]
fn migration_and_trash_restore_survive_state_store_reload() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut source = AppState::default();
    let id = add_profile(&mut source, "Persistent", "persistent.example.test");
    source.connections[0].group = "Legacy".into();
    source.connections[0].credential_ref = Some(Uuid::new_v4());
    let mut file = tempfile::NamedTempFile::new_in(temp.path())?;
    file.write_all(&serde_json::to_vec(&legacy_document(&source)?)?)?;
    let path = file.into_temp_path();
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    let folder_id = state.folder_id_of(id);
    state.record_successful_connection(id, 100)?;
    let removed = state.soft_delete_connection(id, 110)?;
    let saved = store.save(&state)?;
    assert!(fs::read_to_string(&path)?.contains("deleted_connections"));
    let reopened = StateStore::new(&path);
    let mut restored = reopened.load()?;
    assert_eq!(restored, saved);
    assert_eq!(restored.deleted_connections[0].deleted_at, 110);
    assert_eq!(restored.folder_id_of(id), folder_id);
    assert!(restored.recent_connections.is_empty());
    assert_eq!(restored.restore_connection(id)?, id);
    assert_eq!(restored.connections[0], removed);
    let restored = reopened.save(&restored)?;
    assert_eq!(StateStore::new(&path).load()?, restored);
    Ok(())
}

#[test]
fn folder_tree_preorder_and_descendant_membership_use_identity() -> TestResult {
    let mut state = AppState::default();
    let z = state.create_folder("Z", None)?;
    let a = state.create_folder("A", None)?;
    let child = state.create_folder("Same label", Some(a))?;
    let other = state.create_folder("Same label", Some(z))?;
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.move_connection(id, Some(child))?;
    assert_eq!(
        state
            .folder_rows()
            .iter()
            .map(|row| (row.id, row.depth))
            .collect::<Vec<_>>(),
        vec![(a, 0), (child, 1), (z, 0), (other, 1)]
    );
    assert!(state.folder_contains(a, id));
    assert!(state.folder_contains(child, id));
    assert!(!state.folder_contains(other, id));
    assert_eq!(state.connections[0].group, "A/Same label");
    state.move_connection(id, None)?;
    assert!(state.connections[0].group.is_empty());
    assert!(state.folder_id_of(id).is_none());
    Ok(())
}

#[test]
fn moving_a_folder_rejects_self_and_descendant_cycles_atomically() -> TestResult {
    let mut state = AppState::default();
    let root = state.create_folder("Root", None)?;
    let child = state.create_folder("Child", Some(root))?;
    let grandchild = state.create_folder("Grandchild", Some(child))?;
    for parent in [
        Some(root),
        Some(child),
        Some(grandchild),
        Some(Uuid::new_v4()),
    ] {
        let before = state.clone();
        assert!(state.move_folder(root, parent).is_err());
        assert_eq!(state, before);
    }
    Ok(())
}

#[test]
fn folder_names_depth_and_descendant_path_limits_are_atomic() -> TestResult {
    let mut state = AppState::default();
    let first = state.create_folder("First", None)?;
    let second = state.create_folder("Second", None)?;
    let before = state.clone();
    for name in [
        "First".to_owned(),
        "".to_owned(),
        "bad\nname".to_owned(),
        "x".repeat(121),
    ] {
        assert!(state.rename_folder(second, name).is_err());
        assert_eq!(state, before);
    }
    let child = state.create_folder("child", Some(first))?;
    let before = state.clone();
    assert!(state.rename_folder(first, "x".repeat(120)).is_err());
    assert_eq!(state, before);
    assert_eq!(state.folder_path(child).as_deref(), Some("First/child"));
    let mut nested = AppState::default();
    let mut parent = None;
    for _ in 0..32 {
        parent = Some(nested.create_folder("x", parent)?);
    }
    let before = nested.clone();
    assert!(nested.create_folder("x", parent).is_err());
    assert_eq!(nested, before);
    Ok(())
}

#[test]
fn renaming_and_moving_folder_updates_active_and_deleted_legacy_paths() -> TestResult {
    let mut state = AppState::default();
    let root = state.create_folder("Root", None)?;
    let child = state.create_folder("Child", Some(root))?;
    let destination = state.create_folder("Destination", None)?;
    let active = add_profile(&mut state, "Active", "active.example.test");
    let deleted = add_profile(&mut state, "Deleted", "deleted.example.test");
    state.move_connection(active, Some(child))?;
    state.move_connection(deleted, Some(child))?;
    state.soft_delete_connection(deleted, 1)?;
    state.rename_folder(root, "Renamed")?;
    assert_eq!(
        state.deleted_connections[0].connection.group,
        "Renamed/Child"
    );
    state.move_folder(child, Some(destination))?;
    assert_eq!(state.connections[0].group, "Destination/Child");
    assert_eq!(
        state.deleted_connections[0].connection.group,
        "Destination/Child"
    );
    assert_eq!(state.folder_id_of(active), Some(child));
    assert_eq!(state.folder_id_of(deleted), Some(child));
    state.validate()?;
    Ok(())
}

#[test]
fn folder_deletion_requires_no_children_active_profiles_or_trash_memberships() -> TestResult {
    let mut state = AppState::default();
    let root = state.create_folder("Root", None)?;
    let child = state.create_folder("Child", Some(root))?;
    assert!(state.remove_folder(root).is_err());
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.move_connection(id, Some(child))?;
    assert!(state.remove_folder(child).is_err());
    state.soft_delete_connection(id, 10)?;
    let before = state.clone();
    assert!(state.remove_folder(child).is_err());
    assert_eq!(state, before);
    state.purge_deleted_connection(id)?;
    assert!(state.folder_id_of(id).is_none());
    assert_eq!(state.remove_folder(child)?.id, child);
    assert_eq!(state.remove_folder(root)?.id, root);
    state.validate()?;
    Ok(())
}

#[test]
fn restore_retains_original_metadata_without_manufacturing_a_success() -> TestResult {
    let mut state = AppState::default();
    let folder = state.create_folder("Folder", None)?;
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.connections[0].credential_ref = Some(Uuid::new_v4());
    state.connections[0].favorite = true;
    state.move_connection(id, Some(folder))?;
    let original = state.connections[0].clone();
    state.record_successful_connection(id, 10)?;
    state.soft_delete_connection(id, 20)?;
    assert!(state.connections.is_empty());
    assert!(matches!(
        state.record_successful_connection(id, 30),
        Err(Error::ConnectionNotFound)
    ));
    state.restore_connection(id)?;
    assert_eq!(state.connections[0], original);
    assert_eq!(state.folder_id_of(id), Some(folder));
    assert!(state.recent_connections.is_empty());
    assert!(state.deleted_connections.is_empty());
    Ok(())
}

#[test]
fn restore_rejects_an_existing_id_without_changing_either_profile() -> TestResult {
    let mut state = AppState::default();
    let id = add_profile(&mut state, "Original", "original.example.test");
    let mut impostor = state.soft_delete_connection(id, 10)?;
    impostor.host = "different.example.test".into();
    impostor.name = "Existing".into();
    state.connections.push(impostor);
    let before = state.clone();
    assert!(matches!(
        state.restore_connection(id),
        Err(Error::Validation(_))
    ));
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn restore_rejects_equivalent_dns_and_ipv6_endpoints_without_overwriting() -> TestResult {
    for (original, existing) in [
        ("HOST.example.test.", "host.example.test"),
        ("2001:0db8::1", "2001:db8:0:0:0:0:0:1"),
    ] {
        let mut state = AppState::default();
        let id = add_profile(&mut state, "Original", original);
        state.soft_delete_connection(id, 10)?;
        add_profile(&mut state, "Existing", existing);
        let before = state.clone();
        assert!(state.restore_connection(id).is_err());
        assert_eq!(state, before);
        state.validate()?;
    }
    Ok(())
}

#[test]
fn recent_successes_are_bounded_unique_and_ordered_by_observed_success() -> TestResult {
    let mut state = AppState::default();
    let ids: Vec<_> = (0..MAX_RECENT_CONNECTIONS + 3)
        .map(|index| {
            add_profile(
                &mut state,
                &format!("Host {index}"),
                &format!("host-{index}.example.test"),
            )
        })
        .collect();
    assert!(state.recent_connections.is_empty());
    for (index, id) in ids.iter().enumerate() {
        state.record_successful_connection(*id, index as u64 + 100)?;
    }
    assert_eq!(state.recent_connections.len(), MAX_RECENT_CONNECTIONS);
    assert_eq!(
        state.recent_connections[0].connection_id,
        ids[ids.len() - 1]
    );
    assert!(
        !state
            .recent_connections
            .iter()
            .any(|recent| recent.connection_id == ids[0])
    );
    state.record_successful_connection(ids[4], 1)?;
    assert_eq!(
        state.recent_connections[0],
        RecentConnection {
            connection_id: ids[4],
            connected_at: 1
        }
    );
    assert_eq!(
        state
            .recent_connections
            .iter()
            .filter(|recent| recent.connection_id == ids[4])
            .count(),
        1
    );
    let before = state.clone();
    assert!(
        state
            .record_successful_connection(Uuid::new_v4(), 500)
            .is_err()
    );
    assert_eq!(state, before);
    state.soft_delete_connection(ids[4], 600)?;
    assert!(
        !state
            .recent_connections
            .iter()
            .any(|recent| recent.connection_id == ids[4])
    );
    Ok(())
}

#[test]
fn duplication_keeps_folder_but_clears_local_vault_reference_and_recent_history() -> TestResult {
    let mut state = AppState::default();
    let folder = state.create_folder("Folder", None)?;
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.move_connection(id, Some(folder))?;
    state.connections[0].credential_ref = Some(Uuid::new_v4());
    state.record_successful_connection(id, 10)?;
    let copy = state.duplicate_connection(id)?;
    assert_eq!(state.folder_id_of(copy), Some(folder));
    assert!(state.connections[1].credential_ref.is_none());
    assert_eq!(state.connections[1].group, "Folder");
    assert_eq!(state.recent_connections.len(), 1);
    state.validate()?;
    Ok(())
}

#[test]
fn export_round_trips_nested_and_empty_folders_with_fresh_local_ids() -> TestResult {
    let mut source = AppState::default();
    let root = source.create_folder("Root", None)?;
    let child = source.create_folder("Child", Some(root))?;
    source.create_folder("Empty", Some(root))?;
    let id = add_profile(&mut source, "Host", "host.example.test");
    source.move_connection(id, Some(child))?;
    source.connections[0].auth = AuthMethod::PrivateKey {
        path: "keys/identity".into(),
    };
    source.connections[0].credential_ref = Some(Uuid::new_v4());
    source.record_successful_connection(id, 10)?;
    let removed = add_profile(&mut source, "Deleted only", "deleted-only.example.test");
    source.move_connection(removed, Some(child))?;
    source.soft_delete_connection(removed, 20)?;
    let export = source.export_connections()?;
    for excluded in [
        "credential_ref",
        "deleted_connections",
        "deleted-only.example.test",
        "recent_connections",
        "connected_at",
        "known_hosts",
    ] {
        assert!(!export.contains(excluded), "export includes {excluded}");
    }
    assert!(export.contains("keys/identity"));
    let mut target = AppState::default();
    let target_root = target.create_folder("Root", None)?;
    let report = target.import_connections(&export)?;
    assert_eq!((report.added, report.skipped), (1, 0));
    assert_eq!(target.folders.len(), 3);
    assert_eq!(target.folder_rows()[0].id, target_root);
    assert!(target.folders.iter().all(|folder| {
        !source
            .folders
            .iter()
            .any(|original| original.id == folder.id)
    }));
    assert_ne!(target.connections[0].id, id);
    assert_eq!(target.connections[0].auth, source.connections[0].auth);
    assert!(target.connections[0].credential_ref.is_none());
    assert_eq!(
        target
            .folder_path(
                target
                    .folder_id_of(target.connections[0].id)
                    .ok_or("missing membership")?
            )
            .as_deref(),
        Some("Root/Child")
    );
    assert!(target.deleted_connections.is_empty());
    assert!(target.recent_connections.is_empty());
    let before = target.clone();
    assert_eq!(target.import_connections(&export)?.added, 0);
    assert_eq!(target, before);
    Ok(())
}

#[test]
fn legacy_connection_exports_are_accepted_and_vault_references_removed() -> TestResult {
    let mut connection = Connection::new("Legacy", "legacy.example.test", "operator");
    connection.group = "Legacy group".into();
    connection.credential_ref = Some(Uuid::new_v4());
    let document = serde_json::json!({"schema_version": 1, "connections": [connection]});
    let mut state = AppState::default();
    state.import_connections(&serde_json::to_string(&document)?)?;
    assert_eq!(state.folders[0].name, "Legacy group");
    assert_eq!(
        state.folder_id_of(state.connections[0].id),
        Some(state.folders[0].id)
    );
    assert!(state.connections[0].credential_ref.is_none());
    state.validate()?;
    Ok(())
}

#[test]
fn import_never_reactivates_a_deleted_endpoint_or_changes_its_credentials() -> TestResult {
    let mut source = AppState::default();
    let id = add_profile(&mut source, "Original", "host.example.test");
    source.connections[0].credential_ref = Some(Uuid::new_v4());
    let export = source.export_connections()?;
    source.soft_delete_connection(id, 1)?;
    let before = source.clone();
    let report = source.import_connections(&export)?;
    assert_eq!((report.added, report.skipped), (0, 1));
    assert_eq!(source, before);
    Ok(())
}

#[test]
fn malformed_tree_and_membership_imports_leave_state_unchanged() -> TestResult {
    let mut source = AppState::default();
    let root = source.create_folder("Root", None)?;
    let child = source.create_folder("Child", Some(root))?;
    let id = add_profile(&mut source, "Host", "host.example.test");
    source.move_connection(id, Some(child))?;
    let valid: serde_json::Value = serde_json::from_str(&source.export_connections()?)?;
    let mut target = AppState::default();
    let before = target.clone();
    let mut cyclic = valid.clone();
    cyclic["folders"][0]["parent_id"] = serde_json::to_value(child)?;
    assert!(
        target
            .import_connections(&serde_json::to_string(&cyclic)?)
            .is_err()
    );
    assert_eq!(target, before);
    let mut dangling = valid.clone();
    dangling["connection_folders"][id.to_string()] = serde_json::to_value(Uuid::new_v4())?;
    assert!(
        target
            .import_connections(&serde_json::to_string(&dangling)?)
            .is_err()
    );
    assert_eq!(target, before);
    let mut stale_path = valid;
    stale_path["connections"][0]["group"] = "Different".into();
    assert!(
        target
            .import_connections(&serde_json::to_string(&stale_path)?)
            .is_err()
    );
    assert_eq!(target, before);
    Ok(())
}

#[test]
fn validation_rejects_dangling_history_memberships_duplicate_folders_and_cycles() -> TestResult {
    let mut state = AppState::default();
    let id = add_profile(&mut state, "Host", "host.example.test");
    let folder = state.create_folder("Folder", None)?;
    let valid = state.clone();
    state.connection_folders.insert(id, Uuid::new_v4());
    assert!(state.validate().is_err());
    state = valid.clone();
    state.recent_connections.push(RecentConnection {
        connection_id: Uuid::new_v4(),
        connected_at: 1,
    });
    assert!(state.validate().is_err());
    state = valid.clone();
    state.folders.push(ConnectionFolder {
        id: folder,
        name: "Duplicate".into(),
        parent_id: None,
    });
    assert!(state.validate().is_err());
    state = valid;
    state.folders[0].parent_id = Some(folder);
    assert!(state.validate().is_err());
    assert!(state.folder_path(folder).is_none());
    assert!(state.folder_rows().is_empty());
    Ok(())
}

#[test]
fn permanently_removing_active_metadata_cleans_folder_and_recent_links() -> TestResult {
    let mut state = AppState::default();
    let id = add_profile(&mut state, "Host", "host.example.test");
    let folder = state.create_folder("Folder", None)?;
    state.move_connection(id, Some(folder))?;
    state.record_successful_connection(id, 1)?;
    state.remove_connection(id)?;
    assert!(state.folder_id_of(id).is_none());
    assert!(state.recent_connections.is_empty());
    state.remove_folder(folder)?;
    state.validate()?;
    Ok(())
}

#[test]
fn folder_only_import_changes_the_candidate_even_when_no_profile_is_added() -> TestResult {
    let mut source = AppState::default();
    let root = source.create_folder("Empty root", None)?;
    source.create_folder("Empty child", Some(root))?;
    let mut target = AppState::default();
    let before = target.clone();
    let report = target.import_connections(&source.export_connections()?)?;
    assert_eq!((report.added, report.skipped), (0, 0));
    assert_ne!(target, before);
    assert!(target.connections.is_empty());
    assert_eq!(target.folders.len(), 2);
    let imported = target.clone();
    target.import_connections(&source.export_connections()?)?;
    assert_eq!(target, imported);
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let saved = store.save(&target)?;
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    Ok(())
}

#[test]
fn updating_folder_uses_final_destination_for_sibling_uniqueness() -> TestResult {
    let mut state = AppState::default();
    let a = state.create_folder("A", None)?;
    let b = state.create_folder("B", None)?;
    let x = state.create_folder("X", Some(a))?;
    state.create_folder("Y", Some(a))?;
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.move_connection(id, Some(x))?;
    state.soft_delete_connection(id, 10)?;
    assert!(state.rename_folder(x, "Y").is_err());
    state.update_folder(x, "Y", Some(b))?;
    assert_eq!(state.folder_path(x).as_deref(), Some("B/Y"));
    assert_eq!(state.deleted_connections[0].connection.group, "B/Y");
    assert_eq!(state.folder_id_of(id), Some(x));
    state.validate()?;
    Ok(())
}

#[test]
fn updating_folder_checks_final_path_length_and_keeps_failed_edits_atomic() -> TestResult {
    let mut state = AppState::default();
    let long_parent = state.create_folder("A".repeat(60), None)?;
    let short_parent = state.create_folder("B", None)?;
    let child = state.create_folder("X".repeat(50), Some(long_parent))?;
    let id = add_profile(&mut state, "Host", "host.example.test");
    state.move_connection(id, Some(child))?;
    assert!(state.rename_folder(child, "Y".repeat(70)).is_err());
    state.update_folder(child, "Y".repeat(70), Some(short_parent))?;
    assert_eq!(state.connections[0].group, format!("B/{}", "Y".repeat(70)));
    let before = state.clone();
    assert!(
        state
            .update_folder(child, "Z".repeat(70), Some(long_parent))
            .is_err()
    );
    assert_eq!(state, before);
    assert!(
        state
            .update_folder(short_parent, "Invalid", Some(child))
            .is_err()
    );
    assert_eq!(state, before);
    state.validate()?;
    Ok(())
}
