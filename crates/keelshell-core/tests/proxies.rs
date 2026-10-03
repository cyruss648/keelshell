use std::fs;

use keelshell_core::{
    AppState, AuthMethod, Connection, ConnectionProxy, Error, ProxyAuthentication, ProxyKind,
    RouteEndpoint, RouteIdentity, StateStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn proxy() -> ConnectionProxy {
    let mut proxy = ConnectionProxy::new(ProxyKind::Socks5, "Proxy.EXAMPLE.test.", 1080);
    proxy.auth = ProxyAuthentication::UsernamePassword {
        username: "代理 account".into(),
    };
    proxy
}

fn chain() -> AppState {
    let mut state = AppState::default();
    let mut parent = None;
    for index in 0..3 {
        let mut connection =
            Connection::new(format!("Node {index}"), format!("node-{index}.test"), "ops");
        connection.auth = AuthMethod::Password;
        connection.credential_ref = Some(Uuid::new_v4());
        connection.jump_host = parent;
        parent = Some(connection.id);
        state.connections.push(connection);
    }
    state
}

fn pin() -> String {
    format!("SHA256:{}", "A".repeat(43))
}

fn unchanged(actual: &AppState, expected: &AppState) {
    assert_eq!(actual, expected);
    assert_eq!(actual.snapshot, expected.snapshot);
}

#[test]
fn missing_proxy_fields_preserve_legacy_profile_and_identity_bytes() -> TestResult {
    let profile = Connection::new("Legacy", "Node-0.TEST.", "ops");
    let encoded = serde_json::to_string(&profile)?;
    assert!(!encoded.contains("proxy"));
    let decoded: Connection = serde_json::from_str(&encoded)?;
    assert!(decoded.proxy.is_none());
    let state = AppState {
        connections: vec![decoded],
        ..AppState::default()
    };
    let identity = state.connection_route(profile.id)?.identity();
    let legacy = r#"{"version":1,"endpoints":[{"host":"node-0.test","port":22,"username":"ops"}]}"#;
    assert_eq!(serde_json::to_string(&identity)?, legacy);
    let restored: RouteIdentity = serde_json::from_str(legacy)?;
    restored.validate()?;
    assert_eq!(restored, identity);
    Ok(())
}

#[test]
fn proxy_authentication_rejects_inline_secrets_unknown_fields_and_implicit_modes() -> TestResult {
    let value = serde_json::to_value(proxy())?;
    assert_eq!(value["kind"], "socks5");
    assert_eq!(value["auth"]["type"], "username_password");
    for invalid_auth in [
        json!({"type":"none","password":"not-accepted"}),
        json!({"type":"none","username":"ignored-user"}),
        json!({"type":"username_password","username":"ops","password":"not-accepted"}),
        json!({"type":"username_password","username":"ops","credential_ref":Uuid::new_v4()}),
        json!({"type":"username_password"}),
        json!({"type":"agent"}),
        Value::Null,
    ] {
        let mut malformed = value.clone();
        malformed["auth"] = invalid_auth;
        assert!(serde_json::from_value::<ConnectionProxy>(malformed).is_err());
    }
    for field in ["password", "secret", "credential_ref", "command", "url"] {
        let mut malformed = value.clone();
        malformed[field] = "not-accepted".into();
        assert!(serde_json::from_value::<ConnectionProxy>(malformed).is_err());
    }
    for field in ["kind", "host", "port", "auth"] {
        let mut malformed = value.clone();
        malformed.as_object_mut().ok_or("object")?.remove(field);
        assert!(serde_json::from_value::<ConnectionProxy>(malformed).is_err());
    }
    let mut unknown = value;
    unknown["kind"] = "https_connect".into();
    assert!(serde_json::from_value::<ConnectionProxy>(unknown).is_err());
    for auth in [ProxyAuthentication::None, proxy().auth] {
        assert_eq!(
            serde_json::from_str::<ProxyAuthentication>(&serde_json::to_string(&auth)?)?,
            auth
        );
    }
    Ok(())
}

#[test]
fn proxy_validation_rejects_unsafe_endpoints_without_echoing_input() -> TestResult {
    for host in [
        "",
        " ",
        "host\r\nInjected",
        "http://proxy.test",
        "user@proxy.test",
        "-option",
        "[::1]",
        "proxy.test:1080",
        "proxy/path",
        ".",
        "host?query",
        "host#fragment",
        "代理.test",
        "host!name",
        "host%zone",
    ] {
        let mut value = proxy();
        value.host = host.into();
        assert!(value.validate().is_err(), "accepted {host:?}");
    }
    let mut value = proxy();
    value.port = 0;
    assert_eq!(
        value.validate().err().ok_or("invalid port accepted")?.field,
        "connection.proxy.port"
    );
    value.port = 1080;
    value.host = "never-echo-this\n".into();
    assert!(
        !value
            .validate()
            .err()
            .ok_or("invalid host accepted")?
            .to_string()
            .contains("never-echo-this")
    );
    Ok(())
}

#[test]
fn proxy_wire_addresses_share_ascii_bounds_without_changing_legacy_hosts() -> TestResult {
    for host in [
        "127.0.0.1",
        "2001:db8::1",
        "::ffff:192.0.2.1",
        "Proxy_1.EXAMPLE.test.",
        "xn--fiqs8s.example",
    ] {
        let mut profile = Connection::new("Node", host, "ops");
        let mut upstream = proxy();
        upstream.host = host.into();
        profile.proxy = Some(upstream);
        profile.validate()?;
        RouteEndpoint::from_connection(&profile)?;
    }
    // A final dot occupies one input byte even though identities remove it.
    let boundary = format!("{}.", "a".repeat(252));
    let mut profile = Connection::new("Node", &boundary, "ops");
    let mut upstream = proxy();
    upstream.host = boundary.clone();
    profile.proxy = Some(upstream);
    profile.validate()?;
    profile.proxy.as_mut().ok_or("proxy")?.host.push('.');
    assert_eq!(
        profile.validate().err().ok_or("long proxy accepted")?.field,
        "connection.proxy.host"
    );
    profile.proxy.as_mut().ok_or("proxy")?.host = boundary;
    profile.host.push('.');
    assert_eq!(
        profile
            .validate()
            .err()
            .ok_or("long target accepted")?
            .field,
        "connection.host"
    );
    for host in [
        "代理.test",
        "host!name",
        "a%b",
        "-option",
        "...",
        "fe80::1%lo0",
    ] {
        profile.host = host.into();
        assert_eq!(
            profile
                .validate()
                .err()
                .ok_or("invalid target accepted")?
                .field,
            "connection.host"
        );
    }
    for host in ["代理.test", "host!name"] {
        let legacy = Connection::new("Legacy", host, "ops");
        legacy.validate()?;
        RouteEndpoint::from_connection(&legacy)?;
    }
    Ok(())
}

#[test]
fn proxy_usernames_are_bounded_by_utf8_bytes_and_http_colon_rules() -> TestResult {
    let mut value = proxy();
    let boundary = format!("{}123456", "中".repeat(83));
    assert_eq!(boundary.len(), 255);
    value.auth = ProxyAuthentication::UsernamePassword {
        username: boundary.clone(),
    };
    value.validate()?;
    for username in [
        format!("{boundary}x"),
        "中".repeat(86),
        String::new(),
        " outer".into(),
        "outer ".into(),
        "a\tb".into(),
        "a\0b".into(),
        "a\u{7f}b".into(),
    ] {
        value.auth = ProxyAuthentication::UsernamePassword { username };
        assert!(value.validate().is_err());
    }
    value.auth = ProxyAuthentication::UsernamePassword {
        username: "ops:team".into(),
    };
    value.validate()?;
    value.kind = ProxyKind::HttpConnect;
    assert!(value.validate().is_err());
    value.auth = ProxyAuthentication::UsernamePassword {
        username: "Ops Team 用户".into(),
    };
    value.validate()?;
    assert_eq!(
        serde_json::to_value(&value)?["auth"]["username"],
        "Ops Team 用户"
    );
    Ok(())
}

#[test]
fn route_prefix_versions_change_only_when_a_proxy_enters_the_prefix() -> TestResult {
    let mut state = chain();
    state.connections[1].proxy = Some(proxy());
    let route = state.connection_route(state.connections[2].id)?;
    for (hop, version) in [(0, 1), (1, 2), (2, 2)] {
        let identity = route.identity_prefix(hop).ok_or("prefix")?;
        identity.validate()?;
        assert_eq!(serde_json::to_value(identity)?["version"], version);
    }
    assert_eq!(
        route.hops()[1].proxy.as_ref().ok_or("proxy")?.host,
        "Proxy.EXAMPLE.test."
    );
    assert_eq!(
        route.identity().endpoints()[1]
            .proxy
            .as_ref()
            .ok_or("proxy")?
            .host,
        "proxy.example.test"
    );
    assert_eq!(route.identity().endpoints().len(), 3);
    let before = route.clone();
    state.connections[1].proxy = None;
    assert_eq!(route, before);
    assert_ne!(
        state.connection_route(state.connections[2].id)?.identity(),
        route.identity()
    );
    Ok(())
}

#[test]
fn normalized_proxy_endpoints_keep_account_case_protocol_and_order() -> TestResult {
    let mut value = Connection::new("Node", "TARGET.test.", "ops");
    value.proxy = Some(proxy());
    let original = RouteEndpoint::from_connection(&value)?;
    let mut canonical = value.clone();
    canonical.proxy.as_mut().ok_or("proxy")?.host = "proxy.example.test".into();
    assert_eq!(RouteEndpoint::from_connection(&canonical)?, original);
    for variant in 0..4 {
        let mut changed = canonical.clone();
        let proxy = changed.proxy.as_mut().ok_or("proxy")?;
        match variant {
            0 => proxy.kind = ProxyKind::HttpConnect,
            1 => proxy.port = 3128,
            2 => proxy.auth = ProxyAuthentication::None,
            _ => {
                proxy.auth = ProxyAuthentication::UsernamePassword {
                    username: "Other Account".into(),
                }
            }
        }
        assert_ne!(RouteEndpoint::from_connection(&changed)?, original);
    }
    canonical.proxy.as_mut().ok_or("proxy")?.host = "0:0:0:0:0:0:0:1".into();
    let ipv6 = RouteEndpoint::from_connection(&canonical)?;
    canonical.proxy.as_mut().ok_or("proxy")?.host = "::1".into();
    assert_eq!(RouteEndpoint::from_connection(&canonical)?, ipv6);
    Ok(())
}

#[test]
fn one_hop_proxy_trust_never_falls_back_to_legacy_direct_pins() -> TestResult {
    let mut state = chain();
    state.connections.truncate(1);
    let id = state.connections[0].id;
    state.trust_host_key("node-0.test", 22, &pin())?;
    let direct = state
        .connection_route(id)?
        .host_key_scope(0)
        .ok_or("scope")?;
    assert_eq!(state.host_key_for_scope(&direct), Some(pin().as_str()));
    state.connections[0].proxy = Some(proxy());
    let proxied = state
        .connection_route(id)?
        .host_key_scope(0)
        .ok_or("scope")?;
    assert!(state.host_key_for_scope(&proxied).is_none());
    state.trust_host_key_for_scope(&proxied, &pin())?;
    state.validate()?;
    assert_eq!(state.known_hosts.len(), 1);
    assert_eq!(state.route_known_hosts.len(), 1);
    assert_eq!(state.host_key_for_scope(&proxied), Some(pin().as_str()));
    state.connections[0].proxy.as_mut().ok_or("proxy")?.host = "another-proxy.test".into();
    let another = state
        .connection_route(id)?
        .host_key_scope(0)
        .ok_or("scope")?;
    assert!(state.host_key_for_scope(&another).is_none());
    assert_eq!(state.host_key_for_scope(&proxied), Some(pin().as_str()));
    Ok(())
}

#[test]
fn upstream_proxy_changes_isolate_every_downstream_pin_but_keep_direct_first_hop() -> TestResult {
    let mut state = chain();
    state.connections[1].proxy = Some(proxy());
    let target = state.connections[2].id;
    let old = state.connection_route(target)?;
    for hop in 0..3 {
        state.trust_host_key_for_scope(&old.host_key_scope(hop).ok_or("scope")?, &pin())?;
    }
    state.connections[1].proxy.as_mut().ok_or("proxy")?.auth = ProxyAuthentication::None;
    let new = state.connection_route(target)?;
    assert_eq!(
        state.host_key_for_scope(&new.host_key_scope(0).ok_or("scope")?),
        Some(pin().as_str())
    );
    for hop in 1..3 {
        assert!(
            state
                .host_key_for_scope(&new.host_key_scope(hop).ok_or("scope")?)
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn identity_validation_rejects_version_downgrades_noncanonical_proxy_and_extra_secrets()
-> TestResult {
    let mut state = chain();
    state.connections[0].proxy = Some(proxy());
    let good = serde_json::to_value(state.connection_route(state.connections[2].id)?.identity())?;
    for variant in 0..4 {
        let mut bad = good.clone();
        match variant {
            0 => bad["version"] = 1.into(),
            1 => bad["version"] = 3.into(),
            2 => bad["endpoints"][0]["proxy"]["host"] = "UPPERCASE.test.".into(),
            _ => bad["endpoints"][0]["proxy"]["port"] = 0.into(),
        }
        assert!(
            serde_json::from_value::<RouteIdentity>(bad.clone())?
                .validate()
                .is_err()
        );
        let mut candidate = state.clone();
        candidate
            .route_known_hosts
            .insert(serde_json::to_string(&bad)?, pin());
        assert!(candidate.validate().is_err());
    }
    let mut downgraded = good.clone();
    downgraded["endpoints"][0]
        .as_object_mut()
        .ok_or("endpoint")?
        .remove("proxy");
    assert!(
        serde_json::from_value::<RouteIdentity>(downgraded)?
            .validate()
            .is_err()
    );
    let mut secret = good;
    secret["endpoints"][0]["proxy"]["auth"]["password"] = "reject-this".into();
    assert!(serde_json::from_value::<RouteIdentity>(secret).is_err());
    Ok(())
}

#[test]
fn changed_proxy_clears_current_and_active_or_deleted_descendant_credentials_and_recents()
-> TestResult {
    for variant in 0..6 {
        let mut state = chain();
        state.connections[1].proxy = Some(proxy());
        let changed = state.connections[1].id;
        let deleted = state.connections[2].id;
        state.soft_delete_connection(deleted, 1)?;
        let mut dependent = Connection::new("Other target", "other-target.test", "ops");
        dependent.jump_host = Some(changed);
        dependent.credential_ref = Some(Uuid::new_v4());
        state.connections.push(dependent);
        let mut unrelated = Connection::new("Independent", "independent.test", "ops");
        unrelated.credential_ref = Some(Uuid::new_v4());
        state.connections.push(unrelated);
        for id in state.connections.iter().map(|c| c.id).collect::<Vec<_>>() {
            state.record_successful_connection(id, 10)?;
        }
        let before = state.clone();
        let mut edited = state.connections[1].clone();
        let proxy = edited.proxy.as_mut().ok_or("proxy")?;
        match variant {
            0 => proxy.kind = ProxyKind::HttpConnect,
            1 => proxy.host = "new-proxy.test".into(),
            2 => proxy.port = 8080,
            3 => proxy.auth = ProxyAuthentication::None,
            4 => {
                proxy.auth = ProxyAuthentication::UsernamePassword {
                    username: "different".into(),
                }
            }
            _ => edited.proxy = None,
        }
        state.update_connection(edited)?;
        for index in [0, 3] {
            assert_eq!(
                state.connections[index].credential_ref,
                before.connections[index].credential_ref
            );
        }
        for index in [1, 2] {
            assert!(state.connections[index].credential_ref.is_none());
        }
        assert!(
            state.deleted_connections[0]
                .connection
                .credential_ref
                .is_none()
        );
        assert_eq!(state.recent_connections.len(), 2);
        assert!(state.recent_connections.iter().all(|r| {
            [state.connections[0].id, state.connections[3].id].contains(&r.connection_id)
        }));
    }
    Ok(())
}

#[test]
fn canonical_proxy_spelling_and_display_edits_keep_credentials_and_recent_history() -> TestResult {
    let mut state = chain();
    state.connections[1].proxy = Some(proxy());
    state.record_successful_connection(state.connections[2].id, 10)?;
    let before = state.clone();
    let mut edited = state.connections[1].clone();
    edited.proxy.as_mut().ok_or("proxy")?.host = "proxy.example.test".into();
    edited.name = "Renamed".into();
    edited.tags = vec!["new label".into()];
    state.update_connection(edited)?;
    for (actual, original) in state.connections.iter().zip(&before.connections) {
        assert_eq!(actual.credential_ref, original.credential_ref);
    }
    assert_eq!(state.recent_connections, before.recent_connections);
    Ok(())
}

#[test]
fn invalid_proxy_updates_fail_atomically_and_cannot_hide_invalid_jump_graphs() -> TestResult {
    let mut state = chain();
    let before = state.clone();
    let mut edited = state.connections[1].clone();
    edited.proxy = Some(proxy());
    edited.proxy.as_mut().ok_or("proxy")?.port = 0;
    assert!(state.update_connection(edited.clone()).is_err());
    unchanged(&state, &before);
    edited.proxy.as_mut().ok_or("proxy")?.port = 1080;
    edited.jump_host = Some(state.connections[2].id);
    assert!(state.update_connection(edited).is_err());
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn copy_preserves_proxy_metadata_but_never_inherits_ssh_credential_reference() -> TestResult {
    let mut state = chain();
    state.connections[1].proxy = Some(proxy());
    let original = state.connections[1].clone();
    let copy_id = state.duplicate_connection(original.id)?;
    let copy = state
        .connections
        .iter()
        .find(|p| p.id == copy_id)
        .ok_or("copy")?;
    assert_ne!(copy.id, original.id);
    assert_eq!(copy.proxy, original.proxy);
    assert_eq!(copy.jump_host, original.jump_host);
    assert!(copy.credential_ref.is_none());
    Ok(())
}

#[test]
fn proxy_export_import_round_trip_remaps_routes_without_credentials_or_host_trust() -> TestResult {
    let mut source = chain();
    source.connections[0].proxy = Some(proxy());
    source.connections[2].proxy = Some(ConnectionProxy::new(
        ProxyKind::HttpConnect,
        "second-proxy.test",
        3128,
    ));
    let identity = source
        .connection_route(source.connections[2].id)?
        .identity();
    let scope = source
        .connection_route(source.connections[2].id)?
        .host_key_scope(2)
        .ok_or("scope")?;
    source.trust_host_key_for_scope(&scope, &pin())?;
    let exported = source.export_connections()?;
    assert!(!exported.contains("credential_ref"));
    assert!(!exported.contains("known_hosts"));
    assert!(!exported.contains("SHA256:"));
    let mut target = AppState::default();
    assert_eq!(target.import_connections(&exported)?.added, 3);
    let last = target
        .connections
        .iter()
        .find(|p| p.host == "node-2.test")
        .ok_or("target")?;
    assert_ne!(last.id, source.connections[2].id);
    assert_eq!(target.connection_route(last.id)?.identity(), identity);
    assert!(
        target
            .connections
            .iter()
            .all(|p| p.credential_ref.is_none())
    );
    assert!(target.route_known_hosts.is_empty());
    assert_eq!(target.import_connections(&exported)?.skipped, 3);
    Ok(())
}

#[test]
fn importing_different_proxies_cannot_merge_routes_or_rebind_conflicting_jump_ids() -> TestResult {
    let source = chain();
    let mut target = AppState::default();
    assert_eq!(
        target
            .import_connections(&source.export_connections()?)?
            .added,
        3
    );
    let mut other = source.clone();
    other.connections[0].proxy = Some(proxy());
    assert_eq!(
        target
            .import_connections(&other.export_connections()?)?
            .added,
        3
    );
    assert_eq!(target.connections.len(), 6);
    let mut collision = chain();
    collision.connections[0].id = target.connections[0].id;
    collision.connections[0].proxy = Some(proxy());
    collision.connections[1].jump_host = Some(collision.connections[0].id);
    collision.connections[1].host = "new-dependent.test".into();
    let before = target.clone();
    assert!(
        target
            .import_connections(&collision.export_connections()?)
            .is_err()
    );
    unchanged(&target, &before);
    Ok(())
}

#[test]
fn malformed_late_proxy_import_rejects_the_whole_document() -> TestResult {
    let source = chain();
    let mut document: Value = serde_json::from_str(&source.export_connections()?)?;
    document["connections"][2]["proxy"] = json!({"kind":"socks5","host":"proxy.test","port":1080,"auth":{"type":"none","password":"forbidden"}});
    let mut state = AppState::default();
    let before = state.clone();
    assert!(
        state
            .import_connections(&serde_json::to_string(&document)?)
            .is_err()
    );
    unchanged(&state, &before);
    document["connections"][2]["proxy"]["auth"] = json!({"type":"none"});
    document["connections"][2]["proxy"]["port"] = 0.into();
    assert!(
        state
            .import_connections(&serde_json::to_string(&document)?)
            .is_err()
    );
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn restore_distinguishes_proxy_routes_and_deletion_still_protects_dependencies() -> TestResult {
    let mut state = chain();
    state.connections[0].proxy = Some(proxy());
    let target = state.connections[2].id;
    let before = state.clone();
    assert!(
        state
            .soft_delete_connection(state.connections[0].id, 1)
            .is_err()
    );
    unchanged(&state, &before);
    let deleted = state.soft_delete_connection(target, 1)?;
    let mut other = deleted.duplicate("Different route")?;
    other.proxy = Some(ConnectionProxy::new(
        ProxyKind::HttpConnect,
        "target-proxy.test",
        8080,
    ));
    state.connections.push(other);
    state.restore_connection(target)?;
    assert!(
        state
            .connections
            .iter()
            .any(|p| p.id == target && p.credential_ref == deleted.credential_ref)
    );
    state.soft_delete_connection(target, 2)?;
    state.connections.push(deleted.duplicate("Same route")?);
    let before = state.clone();
    assert!(state.restore_connection(target).is_err());
    unchanged(&state, &before);
    Ok(())
}

#[test]
fn proxy_routes_and_pins_persist_without_bypassing_revision_conflicts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = chain();
    state.connections[0].proxy = Some(proxy());
    let scope = state
        .connection_route(state.connections[2].id)?
        .host_key_scope(2)
        .ok_or("scope")?;
    state.trust_host_key_for_scope(&scope, &pin())?;
    store.save(&state)?;
    let second = StateStore::new(store.path());
    let stale = second.load()?;
    assert_eq!(stale, state);
    let mut edited = store.load()?;
    let mut profile = edited.connections[0].clone();
    profile.proxy.as_mut().ok_or("proxy")?.port = 3128;
    edited.update_connection(profile)?;
    store.save(&edited)?;
    let bytes = fs::read(store.path())?;
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    assert_eq!(fs::read(store.path())?, bytes);
    assert_eq!(StateStore::new(store.path()).load()?, edited);
    Ok(())
}
