use std::fs;

use keelshell_core::{
    AppState, AuthMethod, Connection, ConnectionRoute, Error, MAX_JUMP_HOSTS, RouteIdentity,
    StateStore,
};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn chain(jumps: usize) -> AppState {
    let mut state = AppState::default();
    let mut parent = None;
    for index in 0..=jumps {
        let mut connection = Connection::new(
            format!("Node {index}"),
            format!("node-{index}.example.test"),
            "operator",
        );
        connection.jump_host = parent;
        parent = Some(connection.id);
        state.connections.push(connection);
    }
    state
}

fn pin() -> String {
    format!("SHA256:{}", "A".repeat(43))
}

fn unchanged(state: &AppState, before: &AppState) {
    assert_eq!(state, before);
    assert_eq!(state.snapshot, before.snapshot);
}

#[test]
fn route_snapshot_connects_outermost_first_and_remains_owned() -> TestResult {
    let mut state = chain(MAX_JUMP_HOSTS);
    let target = state.connections[MAX_JUMP_HOSTS].id;
    let ordered = state.connections.clone();
    state.connections.reverse();
    state.validate()?;
    let route = state.connection_route(target)?;
    assert_eq!(route.hops(), ordered);
    assert_eq!(route.identity().endpoints().len(), MAX_JUMP_HOSTS + 1);
    assert_eq!(
        route
            .identity_prefix(0)
            .ok_or("first prefix")?
            .endpoints()
            .len(),
        1
    );
    assert!(route.identity_prefix(MAX_JUMP_HOSTS + 1).is_none());
    assert!(route.identity_prefix(usize::MAX).is_none());
    assert!(route.host_key_scope(usize::MAX).is_none());
    state.connections[0].name = "Changed later".into();
    assert_eq!(route.hops(), ordered);
    Ok(())
}

#[test]
fn direct_profiles_and_old_documents_keep_single_endpoint_identity() -> TestResult {
    let mut connection = Connection::new("Direct", "EXAMPLE.test.", "Operator");
    let json = serde_json::to_string(&connection)?;
    assert!(!json.contains("jump_host"));
    let loaded: Connection = serde_json::from_str(&json)?;
    assert!(loaded.jump_host.is_none());
    connection = loaded;
    let id = connection.id;
    let state = AppState {
        connections: vec![connection],
        ..AppState::default()
    };
    let json = serde_json::to_string(&state)?;
    assert!(!json.contains("route_known_hosts"));
    let state: AppState = serde_json::from_str(&json)?;
    state.validate()?;
    let identity = state.connection_route(id)?.identity();
    assert_eq!(identity.endpoints()[0].host, "example.test");
    assert_eq!(identity.endpoints()[0].username, "Operator");
    assert_eq!(identity.endpoints().len(), 1);
    assert_eq!(
        serde_json::from_str::<RouteIdentity>(&serde_json::to_string(&identity)?)?,
        identity
    );
    Ok(())
}

#[test]
fn direct_ephemeral_route_does_not_require_a_saved_profile() -> TestResult {
    let connection = Connection::new("One time", "quick.example.test", "operator");
    let id = connection.id;
    let route = ConnectionRoute::direct(connection.clone())?;
    assert_eq!(route.hops(), &[connection]);
    assert_eq!(route.identity().endpoints().len(), 1);
    assert_eq!(route.identity().endpoints()[0].host, "quick.example.test");
    assert!(route.host_key_scope(0).is_some());
    assert!(AppState::default().connection_route(id).is_err());
    Ok(())
}

#[test]
fn direct_ephemeral_route_rejects_saved_route_metadata() {
    let mut connection = Connection::new("One time", "quick.example.test", "operator");
    connection.jump_host = Some(Uuid::new_v4());
    assert!(ConnectionRoute::direct(connection).is_err());

    let mut connection = Connection::new("One time", "quick.example.test", "operator");
    connection.credential_ref = Some(Uuid::new_v4());
    assert!(ConnectionRoute::direct(connection).is_err());
}

#[test]
fn graph_rejects_missing_nil_self_cycles_duplicate_ids_and_excessive_depth() {
    for malformed in ["missing", "nil", "self", "cycle", "duplicate"] {
        let mut state = chain(2);
        let target = state.connections[2].id;
        match malformed {
            "missing" => state.connections[0].jump_host = Some(Uuid::new_v4()),
            "nil" => state.connections[0].jump_host = Some(Uuid::nil()),
            "self" => state.connections[0].jump_host = Some(state.connections[0].id),
            "cycle" => state.connections[0].jump_host = Some(target),
            "duplicate" => state.connections.push(state.connections[0].clone()),
            _ => unreachable!(),
        }
        assert!(state.validate().is_err(), "accepted {malformed}");
        assert!(
            state.connection_route(target).is_err(),
            "resolved {malformed}"
        );
    }
    let state = chain(MAX_JUMP_HOSTS + 1);
    assert!(state.validate().is_err());
    assert!(
        state
            .connection_route(state.connections[MAX_JUMP_HOSTS + 1].id)
            .is_err()
    );
}

#[test]
fn invalid_route_update_is_atomic_and_unknown_update_never_inserts() -> TestResult {
    let mut state = chain(2);
    let before = state.clone();
    let mut cyclic = state.connections[0].clone();
    cyclic.jump_host = Some(state.connections[2].id);
    assert!(state.update_connection(cyclic).is_err());
    unchanged(&state, &before);
    let missing = Connection::new("Missing", "missing.example.test", "operator");
    assert!(matches!(
        state.update_connection(missing),
        Err(Error::ConnectionNotFound)
    ));
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn recycle_requires_dependents_first_and_restore_requires_jumps_first() -> TestResult {
    let mut state = chain(2);
    let ids: Vec<_> = state.connections.iter().map(|profile| profile.id).collect();
    let before = state.clone();
    assert!(state.soft_delete_connection(ids[0], 1).is_err());
    assert!(state.remove_connection(ids[1]).is_err());
    unchanged(&state, &before);
    for id in ids.iter().rev() {
        state.soft_delete_connection(*id, 1)?;
    }
    state.validate()?;
    assert!(matches!(
        state.connection_route(ids[2]),
        Err(Error::ConnectionNotFound)
    ));
    let before = state.clone();
    assert!(state.restore_connection(ids[2]).is_err());
    assert!(state.purge_deleted_connection(ids[0]).is_err());
    unchanged(&state, &before);
    for id in &ids {
        assert_eq!(state.restore_connection(*id)?, *id);
    }
    assert_eq!(state.connection_route(ids[2])?.hops().len(), 3);
    Ok(())
}

#[test]
fn trash_references_block_permanent_removal_without_blocking_safe_recycling() -> TestResult {
    let mut state = chain(1);
    let gateway = state.connections[0].id;
    let target = state.connections[1].id;
    state.soft_delete_connection(target, 1)?;
    let before = state.clone();
    assert!(state.remove_connection(gateway).is_err());
    unchanged(&state, &before);
    state.soft_delete_connection(gateway, 2)?;
    state.purge_deleted_connection(target)?;
    state.purge_deleted_connection(gateway)?;
    assert!(state.connections.is_empty());
    assert!(state.deleted_connections.is_empty());
    Ok(())
}

#[test]
fn active_routes_cannot_load_through_trash_even_if_ids_exist() -> TestResult {
    let mut state = chain(1);
    let gateway = state.connections.remove(0);
    let target = state.connections[0].id;
    state
        .deleted_connections
        .push(keelshell_core::DeletedConnection {
            connection: gateway,
            deleted_at: 1,
        });
    assert!(state.validate().is_err());
    assert!(state.connection_route(target).is_err());
    Ok(())
}

#[test]
fn duplicate_retains_jump_identity_but_never_copies_the_targets_credential() -> TestResult {
    let mut state = chain(1);
    state.connections[1].credential_ref = Some(Uuid::new_v4());
    let original = state.connections[1].clone();
    let copied = state.duplicate_connection(original.id)?;
    let duplicate = state
        .connections
        .iter()
        .find(|profile| profile.id == copied)
        .ok_or("copy missing")?;
    assert_eq!(duplicate.jump_host, original.jump_host);
    assert_eq!(duplicate.auth, original.auth);
    assert!(duplicate.credential_ref.is_none());
    assert_ne!(duplicate.id, original.id);
    state.validate()?;
    Ok(())
}

fn credential_chain() -> TestResultState {
    let mut state = chain(2);
    for profile in &mut state.connections {
        profile.auth = AuthMethod::Password;
        profile.credential_ref = Some(Uuid::new_v4());
    }
    for id in state
        .connections
        .iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>()
    {
        state.record_successful_connection(id, 1)?;
    }
    Ok(state)
}
type TestResultState = Result<AppState, Box<dyn std::error::Error>>;

#[test]
fn changing_jump_destination_clears_active_and_trash_descendant_credentials_only() -> TestResult {
    let mut state = credential_chain()?;
    let trash_id = state.connections[2].id;
    state.soft_delete_connection(trash_id, 2)?;
    let mut unrelated = Connection::new("Unrelated", "elsewhere.example.test", "operator");
    unrelated.credential_ref = Some(Uuid::new_v4());
    state.connections.push(unrelated.clone());
    state.record_successful_connection(unrelated.id, 2)?;
    let mut edited = state.connections[0].clone();
    edited.host = "new-gateway.example.test".into();
    state.update_connection(edited)?;
    assert!(
        state.connections[..2]
            .iter()
            .all(|profile| profile.credential_ref.is_none())
    );
    assert!(
        state.deleted_connections[0]
            .connection
            .credential_ref
            .is_none()
    );
    assert_eq!(state.connections[2], unrelated);
    assert_eq!(state.recent_connections.len(), 1);
    assert_eq!(state.recent_connections[0].connection_id, unrelated.id);
    Ok(())
}

#[test]
fn changing_auth_invalidates_itself_and_downstream_but_keeps_upstream() -> TestResult {
    let mut state = credential_chain()?;
    let gateway = state.connections[0].clone();
    let mut edited = state.connections[1].clone();
    edited.auth = AuthMethod::PrivateKey {
        path: "keys/other-key".into(),
    };
    state.update_connection(edited)?;
    assert_eq!(state.connections[0], gateway);
    assert!(
        state.connections[1..]
            .iter()
            .all(|profile| profile.credential_ref.is_none())
    );
    assert_eq!(state.recent_connections.len(), 1);
    assert_eq!(state.recent_connections[0].connection_id, gateway.id);
    Ok(())
}

#[test]
fn renaming_tagging_and_normalized_dns_spelling_preserve_route_credentials_and_recent() -> TestResult
{
    let mut state = credential_chain()?;
    let before = state.clone();
    let original_identity = state.connection_route(state.connections[2].id)?.identity();
    let mut edited = state.connections[0].clone();
    edited.name = "Renamed gateway".into();
    edited.tags = vec!["changed label".into()];
    edited.favorite = true;
    edited.host = edited.host.to_uppercase() + ".";
    state.update_connection(edited)?;
    assert_eq!(
        state.connection_route(state.connections[2].id)?.identity(),
        original_identity
    );
    assert_eq!(
        state
            .connections
            .iter()
            .map(|profile| profile.credential_ref)
            .collect::<Vec<_>>(),
        before
            .connections
            .iter()
            .map(|profile| profile.credential_ref)
            .collect::<Vec<_>>()
    );
    assert_eq!(state.recent_connections, before.recent_connections);
    Ok(())
}

#[test]
fn routed_host_trust_isolated_from_direct_and_other_gateway_namespaces() -> TestResult {
    let mut state = chain(1);
    state.connections[1].host = "10.0.0.8".into();
    let target = state.connections[1].id;
    let mut another_gateway = state.connections[0].duplicate("Other gateway")?;
    another_gateway.host = "other-gateway.example.test".into();
    let mut another_target = state.connections[1].duplicate("Other target")?;
    another_target.jump_host = Some(another_gateway.id);
    let other_target = another_target.id;
    state.connections.extend([another_gateway, another_target]);
    state.trust_host_key("10.0.0.8", 22, &pin())?;
    let route = state.connection_route(target)?;
    let first = route.host_key_scope(0).ok_or("first hop")?;
    let scope = route.host_key_scope(1).ok_or("target hop")?;
    assert!(state.host_key_for_scope(&scope).is_none());
    state.trust_host_key_for_scope(&first, &pin())?;
    assert_eq!(
        state.host_key("node-0.example.test", 22),
        Some(pin().as_str())
    );
    state.trust_host_key_for_scope(&scope, &pin())?;
    assert_eq!(state.host_key_for_scope(&scope), Some(pin().as_str()));
    assert!(
        state
            .host_key_for_scope(
                &state
                    .connection_route(other_target)?
                    .host_key_scope(1)
                    .ok_or("other hop")?
            )
            .is_none()
    );
    let mut gateway = state.connections[0].clone();
    gateway.username = "other-account".into();
    state.update_connection(gateway)?;
    let new_scope = state
        .connection_route(target)?
        .host_key_scope(1)
        .ok_or("changed hop")?;
    assert_ne!(new_scope, scope);
    assert!(state.host_key_for_scope(&new_scope).is_none());
    assert_eq!(state.host_key_for_scope(&scope), Some(pin().as_str()));
    Ok(())
}

#[test]
fn normalized_ipv6_routes_ignore_uuids_and_display_labels_but_keep_order_and_accounts() -> TestResult
{
    let mut state = chain(1);
    state.connections[0].host = "0:0:0:0:0:0:0:1".into();
    let target = state.connections[1].id;
    let original = state.connection_route(target)?.identity();
    assert_eq!(original.endpoints()[0].host, "::1");
    let mut clone = state.clone();
    clone.connections[0].id = Uuid::new_v4();
    clone.connections[0].name = "Different label".into();
    clone.connections[0].host = "::1".into();
    clone.connections[1].jump_host = Some(clone.connections[0].id);
    assert_eq!(clone.connection_route(target)?.identity(), original);
    clone.connections[0].username = "Operator".into();
    assert_ne!(clone.connection_route(target)?.identity(), original);
    Ok(())
}

#[test]
fn malformed_or_noncanonical_route_pins_fail_before_storage() -> TestResult {
    let state = chain(1);
    let identity = state.connection_route(state.connections[1].id)?.identity();
    let valid: serde_json::Value = serde_json::to_value(identity)?;
    let mut invalid = Vec::new();
    for version in [0, 2] {
        let mut value = valid.clone();
        value["version"] = version.into();
        invalid.push(serde_json::to_string(&value)?);
    }
    for count in [0, 1, MAX_JUMP_HOSTS + 2] {
        let mut value = valid.clone();
        value["endpoints"] = serde_json::Value::Array(vec![valid["endpoints"][0].clone(); count]);
        invalid.push(serde_json::to_string(&value)?);
    }
    let mut uppercase = valid.clone();
    uppercase["endpoints"][0]["host"] = "GATEWAY.EXAMPLE.TEST".into();
    invalid.push(serde_json::to_string(&uppercase)?);
    let mut unknown = valid;
    unknown["extra"] = true.into();
    invalid.push(serde_json::to_string(&unknown)?);
    invalid.push(serde_json::to_string_pretty(
        &state.connection_route(state.connections[1].id)?.identity(),
    )?);
    for key in invalid {
        let mut edited = state.clone();
        edited.route_known_hosts.insert(key, pin());
        assert!(edited.validate().is_err());
    }
    let mut edited = state;
    let scope = edited
        .connection_route(edited.connections[1].id)?
        .host_key_scope(1)
        .ok_or("scope")?;
    let before = edited.clone();
    assert!(
        edited
            .trust_host_key_for_scope(&scope, "SHA256:short")
            .is_err()
    );
    unchanged(&edited, &before);
    Ok(())
}

#[test]
fn total_pin_budget_is_shared_by_legacy_and_routed_scopes() -> TestResult {
    let mut state = chain(1);
    for index in 0..10_000 {
        state
            .known_hosts
            .insert(format!("[host-{index}.test]:22"), pin());
    }
    let scope = state
        .connection_route(state.connections[1].id)?
        .host_key_scope(1)
        .ok_or("scope")?;
    let before = state.clone();
    assert!(state.trust_host_key_for_scope(&scope, &pin()).is_err());
    unchanged(&state, &before);
    state.known_hosts.remove("[host-0.test]:22");
    state.trust_host_key_for_scope(&scope, &pin())?;
    assert!(state.trust_host_key("extra.test", 22, &pin()).is_err());
    state.trust_host_key_for_scope(&scope, &pin())?;
    state.validate()?;
    Ok(())
}

#[test]
fn export_import_remaps_unordered_chain_and_omits_all_local_secrets_and_trust() -> TestResult {
    let mut source = credential_chain()?;
    let target = source.connections[2].id;
    let route = source.connection_route(target)?;
    for index in 0..3 {
        source.trust_host_key_for_scope(&route.host_key_scope(index).ok_or("scope")?, &pin())?;
    }
    source.connections.reverse();
    let export = source.export_connections()?;
    for forbidden in [
        "credential_ref",
        "known_hosts",
        "route_known_hosts",
        "SHA256:",
        "recent_connections",
    ] {
        assert!(!export.contains(forbidden));
    }
    let mut target_state = AppState::default();
    let report = target_state.import_connections(&export)?;
    assert_eq!((report.added, report.skipped), (3, 0));
    assert!(
        target_state
            .connections
            .iter()
            .all(|profile| profile.credential_ref.is_none())
    );
    assert!(
        target_state
            .connections
            .iter()
            .all(|profile| source.connections.iter().all(|old| old.id != profile.id))
    );
    let imported = target_state
        .connections
        .iter()
        .find(|profile| profile.name == "Node 2")
        .ok_or("target missing")?;
    assert_eq!(
        target_state.connection_route(imported.id)?.identity(),
        route.identity()
    );
    assert_eq!(
        target_state
            .connections
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>(),
        ["Node 2", "Node 1", "Node 0"]
    );
    let before = target_state.clone();
    assert_eq!(target_state.import_connections(&export)?.added, 0);
    unchanged(&target_state, &before);
    Ok(())
}

#[test]
fn identical_private_addresses_behind_distinct_gateways_are_not_deduplicated() -> TestResult {
    let mut first = chain(1);
    first.connections[1].host = "10.0.0.8".into();
    let mut second = chain(1);
    second.connections[0].host = "different-gateway.test".into();
    second.connections[1].host = "10.0.0.8".into();
    let report = first.import_connections(&second.export_connections()?)?;
    assert_eq!((report.added, report.skipped), (2, 0));
    assert_eq!(
        first
            .connections
            .iter()
            .filter(|profile| profile.host == "10.0.0.8")
            .count(),
        2
    );
    Ok(())
}

#[test]
fn existing_unambiguous_compatible_gateway_is_reused_without_changing_its_credentials() -> TestResult
{
    let mut local = chain(0);
    local.connections[0].credential_ref = Some(Uuid::new_v4());
    let gateway = local.connections[0].clone();
    let source = chain(1);
    let report = local.import_connections(&source.export_connections()?)?;
    assert_eq!((report.added, report.skipped), (1, 1));
    assert_eq!(local.connections[0], gateway);
    assert_eq!(local.connections[1].jump_host, Some(gateway.id));
    Ok(())
}

#[test]
fn import_refuses_to_guess_external_uuid_references_even_when_local_profile_exists() -> TestResult {
    let mut local = chain(0);
    let gateway = local.connections[0].id;
    let mut source = Connection::new("Dependent", "private.test", "operator");
    source.jump_host = Some(gateway);
    let input = serde_json::json!({"schema_version":1,"connections":[source]}).to_string();
    let before = local.clone();
    assert!(local.import_connections(&input).is_err());
    unchanged(&local, &before);
    Ok(())
}

#[test]
fn imported_dependencies_reject_id_collisions_trash_auth_conflicts_and_ambiguous_aliases()
-> TestResult {
    for conflict in ["id", "trash", "auth", "duplicate"] {
        let mut source = chain(1);
        let mut local = chain(0);
        match conflict {
            "id" => {
                source.connections[0].id = local.connections[0].id;
                source.connections[1].jump_host = Some(local.connections[0].id);
                local.connections[0].host = "different-destination.test".into();
            }
            "trash" => {
                local.soft_delete_connection(local.connections[0].id, 1)?;
            }
            "auth" => local.connections[0].auth = AuthMethod::Password,
            "duplicate" => {
                local.duplicate_connection(local.connections[0].id)?;
            }
            _ => unreachable!(),
        }
        let before = local.clone();
        assert!(
            local
                .import_connections(&source.export_connections()?)
                .is_err(),
            "accepted {conflict}"
        );
        unchanged(&local, &before);
    }
    Ok(())
}

#[test]
fn conflicting_repeated_import_id_cannot_select_an_arbitrary_jump_definition() -> TestResult {
    let source = chain(1);
    let mut document: serde_json::Value = serde_json::from_str(&source.export_connections()?)?;
    let mut conflicting = document["connections"][0].clone();
    conflicting["host"] = "different.test".into();
    document["connections"]
        .as_array_mut()
        .ok_or("connections array")?
        .push(conflicting);
    let mut local = AppState::default();
    let before = local.clone();
    assert!(local.import_connections(&document.to_string()).is_err());
    unchanged(&local, &before);
    Ok(())
}

#[test]
fn restore_same_private_endpoint_is_allowed_when_its_complete_route_differs() -> TestResult {
    let mut state = chain(1);
    let target = state.connections[1].id;
    let mut direct = state.connections[1].duplicate("Direct address")?;
    direct.jump_host = None;
    state.connections.push(direct);
    state.soft_delete_connection(target, 1)?;
    state.restore_connection(target)?;
    assert_eq!(state.connections.len(), 3);
    Ok(())
}

#[test]
fn route_and_scoped_pins_round_trip_real_storage_and_stale_edits_cannot_overwrite() -> TestResult {
    let temp = tempfile::tempdir()?;
    let first = StateStore::new(temp.path().join("state.json"));
    let second = StateStore::new(first.path());
    let mut state = credential_chain()?;
    let route = state.connection_route(state.connections[2].id)?;
    state.trust_host_key_for_scope(&route.host_key_scope(2).ok_or("scope")?, &pin())?;
    let saved = first.save(&state)?;
    assert_eq!(StateStore::new(first.path()).load()?, saved);
    let mut stale = second.load()?;
    let mut updated = saved;
    let mut jump = updated.connections[0].clone();
    jump.host = "new.example.test".into();
    updated.update_connection(jump)?;
    let updated = first.save(&updated)?;
    let disk = fs::read(first.path())?;
    let mut target = stale.connections[2].clone();
    target.name = "Stale rename".into();
    stale.update_connection(target)?;
    let draft = stale.clone();
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    assert_eq!(fs::read(first.path())?, disk);
    assert_eq!(StateStore::new(first.path()).load()?, updated);
    unchanged(&stale, &draft);
    Ok(())
}

#[test]
fn changing_the_jump_link_invalidates_only_the_changed_subtree() -> TestResult {
    let mut state = credential_chain()?;
    let gateway = state.connections[0].clone();
    let other = Connection::new("Other route", "other.example.test", "operator");
    let mut edited = state.connections[1].clone();
    edited.jump_host = Some(other.id);
    state.connections.push(other.clone());
    state.update_connection(edited)?;
    assert_eq!(state.connections[0], gateway);
    assert_eq!(state.connections[3], other);
    assert!(
        state.connections[1..3]
            .iter()
            .all(|profile| profile.credential_ref.is_none())
    );
    assert_eq!(state.recent_connections.len(), 1);
    assert_eq!(state.recent_connections[0].connection_id, gateway.id);
    assert_eq!(
        state.connection_route(state.connections[2].id)?.hops()[0].id,
        other.id
    );
    Ok(())
}

#[test]
fn imported_equivalent_gateway_aliases_remap_to_one_new_local_identity() -> TestResult {
    let mut source = chain(1);
    let gateway = source.connections[0].clone();
    let alias = gateway.duplicate("Gateway alias")?;
    source.connections[1].jump_host = Some(alias.id);
    source.connections.push(alias);
    let export = source.export_connections()?;
    let mut local = AppState::default();
    let report = local.import_connections(&export)?;
    assert_eq!((report.added, report.skipped), (2, 1));
    assert_eq!(
        local.connections[1].jump_host,
        Some(local.connections[0].id)
    );
    assert_ne!(local.connections[0].id, gateway.id);
    local.validate()?;
    Ok(())
}
