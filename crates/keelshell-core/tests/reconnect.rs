use std::{fs, path::PathBuf, time::Duration};

use keelshell_core::{
    AppState, AuthMethod, Connection, ConnectionProxy, Error, ProxyAuthentication, ProxyKind,
    ReconnectPolicy, StateStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn automatic() -> ReconnectPolicy {
    ReconnectPolicy::Automatic {
        max_attempts: 3,
        initial_delay_seconds: 2,
        max_delay_seconds: 30,
    }
}

fn chain() -> AppState {
    let mut state = AppState::default();
    let mut parent = None;
    for index in 0..3 {
        let mut profile =
            Connection::new(format!("Hop {index}"), format!("hop-{index}.test"), "ops");
        profile.auth = AuthMethod::Password;
        profile.credential_ref = Some(Uuid::new_v4());
        profile.jump_host = parent;
        parent = Some(profile.id);
        state.connections.push(profile);
    }
    state
}

fn pin() -> String {
    format!("SHA256:{}", "A".repeat(43))
}

#[test]
fn legacy_profiles_default_to_manual_and_keep_identical_json() -> TestResult {
    let profile = Connection::new("Legacy", "HOST.test.", "ops");
    assert_eq!(profile.reconnect, ReconnectPolicy::Manual);
    let old = format!(
        r#"{{"id":"{}","name":"Legacy","group":"","host":"HOST.test.","port":22,"username":"ops","auth":{{"type":"agent"}},"tags":[],"favorite":false}}"#,
        profile.id
    );
    assert_eq!(serde_json::to_string(&profile)?, old);
    let decoded: Connection = serde_json::from_str(&old)?;
    assert_eq!(decoded, profile);
    decoded.validate()?;
    Ok(())
}

#[test]
fn explicit_modes_round_trip_and_reject_unknown_fields_including_manual() -> TestResult {
    for policy in [ReconnectPolicy::Manual, automatic()] {
        let encoded = serde_json::to_value(policy)?;
        assert_eq!(
            serde_json::from_value::<ReconnectPolicy>(encoded.clone())?,
            policy
        );
        for field in [
            "password",
            "secret",
            "credential_ref",
            "attempt",
            "enabled",
            "version",
        ] {
            let mut malformed = encoded.clone();
            malformed[field] = "not-accepted".into();
            assert!(serde_json::from_value::<ReconnectPolicy>(malformed).is_err());
        }
    }
    for malformed in [
        json!({"type":"manual", "max_attempts":3}),
        json!({"type":"automatic", "max_attempts":3, "initial_delay_seconds":2}),
        json!({"type":"automatic", "initial_delay_seconds":2, "max_delay_seconds":30}),
        json!({"type":"automatic", "max_attempts":3, "max_delay_seconds":30}),
        json!({"type":"always"}),
        json!({"enabled":true}),
        Value::Null,
        json!(true),
    ] {
        assert!(serde_json::from_value::<ReconnectPolicy>(malformed).is_err());
    }
    for malformed in [
        r#"{"type":"manual","type":"automatic"}"#,
        r#"{"type":"automatic","max_attempts":3,"max_attempts":4,"initial_delay_seconds":2,"max_delay_seconds":30}"#,
    ] {
        assert!(serde_json::from_str::<ReconnectPolicy>(malformed).is_err());
    }
    Ok(())
}

#[test]
fn numeric_wire_types_cannot_wrap_or_coerce_into_valid_policies() -> TestResult {
    let good = serde_json::to_value(automatic())?;
    for (field, invalid) in [
        ("max_attempts", json!(-1)),
        ("max_attempts", json!(256)),
        ("max_attempts", json!(1.5)),
        ("max_attempts", json!("3")),
        ("initial_delay_seconds", json!(65_536)),
        ("max_delay_seconds", json!(-1)),
    ] {
        let mut malformed = good.clone();
        malformed[field] = invalid;
        assert!(serde_json::from_value::<ReconnectPolicy>(malformed).is_err());
    }
    Ok(())
}

#[test]
fn validation_rejects_unbounded_zero_and_reversed_policies_without_scheduling() -> TestResult {
    for (attempts, initial, cap, field) in [
        (0, 2, 30, "max_attempts"),
        (11, 2, 30, "max_attempts"),
        (3, 0, 30, "initial_delay_seconds"),
        (3, 61, 120, "initial_delay_seconds"),
        (3, 2, 0, "max_delay_seconds"),
        (3, 30, 29, "max_delay_seconds"),
        (3, 2, 301, "max_delay_seconds"),
        (u8::MAX, u16::MAX, u16::MAX, "max_attempts"),
    ] {
        let policy = ReconnectPolicy::Automatic {
            max_attempts: attempts,
            initial_delay_seconds: initial,
            max_delay_seconds: cap,
        };
        let error = policy.validate().err().ok_or("invalid policy accepted")?;
        assert_eq!(error.field, format!("connection.reconnect.{field}"));
        assert_eq!(policy.retry_delay(1), None);
        assert_eq!(policy.retry_delay(usize::MAX), None);
    }
    Ok(())
}

#[test]
fn automatic_backoff_is_one_based_capped_and_exhausted_after_the_whole_route_budget() -> TestResult
{
    let policy = ReconnectPolicy::Automatic {
        max_attempts: 10,
        initial_delay_seconds: 2,
        max_delay_seconds: 30,
    };
    policy.validate()?;
    let delays = (1..=10)
        .map(|attempt| policy.retry_delay(attempt).map(|delay| delay.as_secs()))
        .collect::<Vec<_>>();
    assert_eq!(delays, [2, 4, 8, 16, 30, 30, 30, 30, 30, 30].map(Some));
    for attempt in [0, 11, usize::MAX] {
        assert_eq!(policy.retry_delay(attempt), None);
    }
    for attempt in [0, 1, usize::MAX] {
        assert_eq!(ReconnectPolicy::Manual.retry_delay(attempt), None);
    }
    for (attempts, initial, cap) in [(1, 1, 1), (10, 60, 300)] {
        let boundary = ReconnectPolicy::Automatic {
            max_attempts: attempts,
            initial_delay_seconds: initial,
            max_delay_seconds: cap,
        };
        boundary.validate()?;
        assert_eq!(
            boundary.retry_delay(1),
            Some(Duration::from_secs(u64::from(initial)))
        );
        assert!(boundary.retry_delay(usize::from(attempts)).is_some());
    }
    Ok(())
}

#[test]
fn policy_edits_preserve_route_bytes_pins_credentials_and_recent_success() -> TestResult {
    for proxied in [false, true] {
        let mut state = chain();
        if proxied {
            state.connections[0].proxy =
                Some(ConnectionProxy::new(ProxyKind::Socks5, "proxy.test", 1080));
        }
        let ids: Vec<_> = state.connections.iter().map(|profile| profile.id).collect();
        for id in &ids {
            state.record_successful_connection(*id, 1)?;
        }
        let route = state.connection_route(ids[2])?;
        for hop in 0..3 {
            state.trust_host_key_for_scope(&route.host_key_scope(hop).ok_or("scope")?, &pin())?;
        }
        state.soft_delete_connection(ids[2], 2)?;
        let before = state.clone();
        let active_route = state.connection_route(ids[1])?;
        let identity_json = serde_json::to_string(&active_route.identity())?;
        let mut edited = state.connections[0].clone();
        edited.reconnect = automatic();
        state.update_connection(edited)?;
        assert_eq!(state.known_hosts, before.known_hosts);
        assert_eq!(state.route_known_hosts, before.route_known_hosts);
        assert_eq!(state.recent_connections, before.recent_connections);
        assert_eq!(state.deleted_connections, before.deleted_connections);
        for (actual, previous) in state.connections.iter().zip(&before.connections) {
            assert_eq!(actual.credential_ref, previous.credential_ref);
        }
        assert_eq!(
            serde_json::to_string(&state.connection_route(ids[1])?.identity())?,
            identity_json
        );
        assert!(active_route.same_reconnect_target(&state.connection_route(ids[1])?));
        state.restore_connection(ids[2])?;
        assert!(route.same_reconnect_target(&state.connection_route(ids[2])?));
    }
    Ok(())
}

#[test]
fn route_guard_accepts_metadata_new_credential_references_and_canonical_spelling() -> TestResult {
    let mut state = chain();
    state.connections[0].proxy = Some(ConnectionProxy::new(
        ProxyKind::HttpConnect,
        "PROXY.TEST.",
        8080,
    ));
    let target = state.connections[2].id;
    let before = state.connection_route(target)?;
    for profile in &mut state.connections {
        profile.name = format!("Renamed {}", profile.name);
        profile.group = "Display only".into();
        profile.tags.push("新标签".into());
        profile.favorite = true;
        profile.credential_ref = Some(Uuid::new_v4());
        profile.reconnect = automatic();
        profile.host = format!("{}.", profile.host.to_uppercase());
    }
    state.connections[0].proxy.as_mut().ok_or("proxy")?.host = "proxy.test".into();
    state.connections.reverse();
    let after = state.connection_route(target)?;
    assert_ne!(before, after);
    assert!(before.same_reconnect_target(&after));
    assert!(after.same_reconnect_target(&before));
    Ok(())
}

#[test]
fn route_guard_rejects_changed_network_account_authentication_or_proxy_on_any_hop() -> TestResult {
    let state = chain();
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    for index in 0..3 {
        for field in ["host", "port", "username", "auth", "proxy"] {
            let mut changed = state.clone();
            let profile = &mut changed.connections[index];
            match field {
                "host" => profile.host = "changed.test".into(),
                "port" => profile.port = 2222,
                "username" => profile.username = "different-account".into(),
                "auth" => profile.auth = AuthMethod::Agent,
                "proxy" => {
                    profile.proxy =
                        Some(ConnectionProxy::new(ProxyKind::Socks5, "proxy.test", 1080))
                }
                _ => unreachable!(),
            }
            let candidate = changed.connection_route(target)?;
            assert!(
                !original.same_reconnect_target(&candidate),
                "accepted hop {index} {field}"
            );
            assert!(!candidate.same_reconnect_target(&original));
        }
    }
    Ok(())
}

#[test]
fn route_guard_rejects_proxy_protocol_address_port_and_exact_auth_username_changes() -> TestResult {
    let mut state = chain();
    let mut proxy = ConnectionProxy::new(ProxyKind::Socks5, "proxy.test", 1080);
    proxy.auth = ProxyAuthentication::UsernamePassword {
        username: "ProxyUser".into(),
    };
    state.connections[1].proxy = Some(proxy);
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    for field in ["kind", "host", "port", "username", "auth"] {
        let mut changed = state.clone();
        let proxy = changed.connections[1].proxy.as_mut().ok_or("proxy")?;
        match field {
            "kind" => proxy.kind = ProxyKind::HttpConnect,
            "host" => proxy.host = "other-proxy.test".into(),
            "port" => proxy.port = 8080,
            "username" => {
                proxy.auth = ProxyAuthentication::UsernamePassword {
                    username: "proxyuser".into(),
                }
            }
            "auth" => proxy.auth = ProxyAuthentication::None,
            _ => unreachable!(),
        }
        assert!(
            !original.same_reconnect_target(&changed.connection_route(target)?),
            "accepted {field}"
        );
    }
    Ok(())
}

#[test]
fn route_guard_rejects_private_key_path_change_even_when_trust_identity_matches() -> TestResult {
    let mut state = chain();
    state.connections[0].auth = AuthMethod::PrivateKey {
        path: PathBuf::from("first-key"),
    };
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    state.connections[0].auth = AuthMethod::PrivateKey {
        path: PathBuf::from("second-key"),
    };
    let changed = state.connection_route(target)?;
    assert_eq!(original.identity(), changed.identity());
    assert!(!original.same_reconnect_target(&changed));
    Ok(())
}

#[test]
fn route_guard_rejects_new_profile_ids_and_rebound_equivalent_jump_profiles() -> TestResult {
    let state = chain();
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    for index in 0..3 {
        let mut changed = state.clone();
        let replacement = Uuid::new_v4();
        changed.connections[index].id = replacement;
        if index + 1 < changed.connections.len() {
            changed.connections[index + 1].jump_host = Some(replacement);
        }
        let current_target = changed.connections[2].id;
        let candidate = changed.connection_route(current_target)?;
        assert_eq!(original.identity(), candidate.identity());
        assert!(!original.same_reconnect_target(&candidate));
    }
    Ok(())
}

#[test]
fn route_guard_rejects_changed_hop_order_and_depth() -> TestResult {
    let mut state = chain();
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    state.connections[0].jump_host = Some(state.connections[1].id);
    state.connections[1].jump_host = None;
    state.connections[2].jump_host = Some(state.connections[0].id);
    assert!(!original.same_reconnect_target(&state.connection_route(target)?));
    state.connections[2].jump_host = None;
    assert!(!original.same_reconnect_target(&state.connection_route(target)?));
    Ok(())
}

#[test]
fn deleted_profiles_cannot_be_resolved_but_explicit_restore_retains_policy_and_identity()
-> TestResult {
    let mut state = chain();
    state.connections[2].reconnect = automatic();
    let target = state.connections[2].id;
    let original = state.connection_route(target)?;
    state.soft_delete_connection(target, 1)?;
    assert!(matches!(
        state.connection_route(target),
        Err(Error::ConnectionNotFound)
    ));
    assert_eq!(
        state.deleted_connections[0].connection.reconnect,
        automatic()
    );
    state.restore_connection(target)?;
    let restored = state.connection_route(target)?;
    assert!(original.same_reconnect_target(&restored));
    assert_eq!(
        restored.hops().last().ok_or("target")?.reconnect,
        automatic()
    );
    assert!(state.recent_connections.is_empty());
    Ok(())
}

#[test]
fn duplicate_and_import_preserve_policy_but_do_not_inherit_credentials_or_activity() -> TestResult {
    let mut source = chain();
    for profile in &mut source.connections {
        profile.reconnect = automatic();
    }
    let original = &source.connections[2];
    let duplicate = original.duplicate("Copy")?;
    assert_eq!(duplicate.reconnect, automatic());
    assert!(duplicate.credential_ref.is_none());
    assert_ne!(duplicate.id, original.id);
    let mut destination = AppState::default();
    let report = destination.import_connections(&source.export_connections()?)?;
    assert_eq!(report.added, 3);
    assert_eq!(report.skipped, 0);
    assert!(
        destination
            .connections
            .iter()
            .all(|profile| profile.reconnect == automatic() && profile.credential_ref.is_none())
    );
    assert!(destination.recent_connections.is_empty());
    assert!(destination.known_hosts.is_empty());
    assert!(destination.route_known_hosts.is_empty());
    let target = destination
        .connections
        .iter()
        .find(|profile| profile.name == original.name)
        .ok_or("imported target")?;
    assert_eq!(
        source.connection_route(original.id)?.identity(),
        destination.connection_route(target.id)?.identity()
    );
    assert!(
        !source
            .connection_route(original.id)?
            .same_reconnect_target(&destination.connection_route(target.id)?)
    );
    Ok(())
}

#[test]
fn skipped_import_does_not_override_an_existing_manual_policy() -> TestResult {
    let mut state = chain();
    let mut source = state.clone();
    for profile in &mut source.connections {
        profile.reconnect = automatic();
    }
    let before = state.clone();
    let report = state.import_connections(&source.export_connections()?)?;
    assert_eq!(report.added, 0);
    assert_eq!(report.skipped, 3);
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn invalid_import_and_update_leave_the_entire_state_unchanged() -> TestResult {
    let mut state = chain();
    let before = state.clone();
    let invalid = ReconnectPolicy::Automatic {
        max_attempts: 0,
        initial_delay_seconds: 2,
        max_delay_seconds: 30,
    };
    let mut edited = state.connections[0].clone();
    edited.reconnect = invalid;
    assert!(state.update_connection(edited).is_err());
    assert_eq!(state, before);
    let mut document: Value = serde_json::from_str(&state.export_connections()?)?;
    document["connections"][2]["reconnect"] = serde_json::to_value(invalid)?;
    assert!(
        state
            .import_connections(&serde_json::to_string(&document)?)
            .is_err()
    );
    assert_eq!(state, before);
    assert_eq!(state.snapshot, before.snapshot);
    Ok(())
}

#[test]
fn policy_round_trips_real_storage_and_rejects_invalid_and_stale_writes() -> TestResult {
    let temp = tempfile::tempdir()?;
    let first = StateStore::new(temp.path().join("state.json"));
    let second = StateStore::new(first.path());
    let mut state = chain();
    state.connections[2].reconnect = automatic();
    state = first.save(&state)?;
    assert_eq!(StateStore::new(first.path()).load()?, state);
    let mut stale = second.load()?;
    let mut edited = state.connections[0].clone();
    edited.reconnect = automatic();
    state.update_connection(edited)?;
    state = first.save(&state)?;
    let disk = fs::read(first.path())?;
    let mut stale_edit = stale.connections[0].clone();
    stale_edit.name = "Stale edit".into();
    stale.update_connection(stale_edit)?;
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    let mut invalid = state.clone();
    invalid.connections[0].reconnect = ReconnectPolicy::Automatic {
        max_attempts: 3,
        initial_delay_seconds: 0,
        max_delay_seconds: 30,
    };
    assert!(first.save(&invalid).is_err());
    assert_eq!(fs::read(first.path())?, disk);
    assert_eq!(StateStore::new(first.path()).load()?, state);
    let json: Value = serde_json::from_slice(&disk)?;
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["connections"][2]["reconnect"]["type"], "automatic");
    Ok(())
}
