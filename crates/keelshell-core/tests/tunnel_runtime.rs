#![allow(clippy::expect_used, clippy::unwrap_used)]

use keelshell_core::{
    AppState, AuthMethod, Connection, Error, StateStore, TunnelConnectionBinding,
    TunnelRecoveryError, TunnelRule, TunnelRuleKind, TunnelRuntimeSnapshot, TunnelRuntimeStatus,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> (AppState, uuid::Uuid, uuid::Uuid) {
    let connection = Connection::new("Recovery host", "host.test", "operator");
    let connection_id = connection.id;
    let mut state = AppState::default();
    state.connections.push(connection);
    let mut rule = TunnelRule::new(
        "Database",
        connection_id,
        TunnelRuleKind::Local,
        "127.0.0.1",
        0,
        Some("db.internal".into()),
        Some(5432),
    )
    .expect("valid rule");
    rule.enabled = true;
    let rule_id = state.add_tunnel_rule(rule).expect("rule added");
    (state, connection_id, rule_id)
}

fn candidate(
    state: &AppState,
    connection_id: uuid::Uuid,
    rule_id: uuid::Uuid,
) -> TunnelRuntimeSnapshot {
    let rule = state
        .tunnel_rules
        .iter()
        .find(|rule| rule.id == rule_id)
        .cloned()
        .expect("rule");
    TunnelRuntimeSnapshot::new(
        rule,
        TunnelConnectionBinding::from_state(state, connection_id).expect("binding"),
        "127.0.0.1:40123",
        1_700_000_000,
    )
    .expect("candidate")
}

#[test]
fn runtime_snapshot_round_trip_contains_only_non_sensitive_recovery_metadata() -> TestResult {
    let (mut state, connection_id, rule_id) = fixture();
    let snapshot = candidate(&state, connection_id, rule_id);
    state.upsert_tunnel_runtime_snapshot(snapshot.clone())?;

    let document = serde_json::to_string(&snapshot)?;
    assert!(!document.contains("password"));
    assert!(!document.contains("private_key"));
    assert!(!document.contains("credential_ref"));
    assert!(!document.contains("worker"));
    assert!(!document.contains("socket"));
    assert!(document.contains("assigned_listener"));
    assert!(document.contains("route"));

    let decoded: TunnelRuntimeSnapshot = serde_json::from_str(&document)?;
    assert_eq!(decoded, snapshot);
    assert_eq!(state.tunnel_runtime_snapshots, vec![snapshot]);
    Ok(())
}

#[test]
fn runtime_snapshot_persists_and_reload_keeps_the_candidate_without_starting_anything() -> TestResult
{
    let (mut state, connection_id, rule_id) = fixture();
    let snapshot = candidate(&state, connection_id, rule_id);
    state.upsert_tunnel_runtime_snapshot(snapshot.clone())?;

    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let saved = store.save(&state)?;
    let reopened = StateStore::new(store.path());
    let loaded = reopened.load()?;

    assert_eq!(loaded.tunnel_runtime_snapshots, vec![snapshot]);
    assert_eq!(loaded, saved);
    assert_eq!(
        loaded.tunnel_runtime_snapshots[0].status,
        TunnelRuntimeStatus::PossibleRunning
    );
    Ok(())
}

#[test]
fn recovery_plan_requires_exact_rule_route_and_authentication_binding() -> TestResult {
    let (mut state, connection_id, rule_id) = fixture();
    let snapshot = candidate(&state, connection_id, rule_id);
    let snapshot_id = snapshot.id;
    state.upsert_tunnel_runtime_snapshot(snapshot)?;
    assert!(state.plan_tunnel_runtime_recovery(snapshot_id).is_ok());

    let mut changed_rule = state.tunnel_rules[0].clone();
    changed_rule.target_port = Some(6432);
    state.update_tunnel_rule(changed_rule)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::RuleChanged)
    );

    let mut restored_rule = state.tunnel_rules[0].clone();
    restored_rule.target_port = Some(5432);
    state.update_tunnel_rule(restored_rule)?;
    let mut changed_connection = state.connections[0].clone();
    changed_connection.host = "other.test".into();
    state.update_connection(changed_connection)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::ConnectionRouteChanged)
    );

    let mut restored_connection = state.connections[0].clone();
    restored_connection.host = "host.test".into();
    restored_connection.auth = AuthMethod::Password;
    state.update_connection(restored_connection)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::AuthenticationTargetChanged)
    );
    Ok(())
}

#[test]
fn recovery_rejects_changed_auth_target_without_persisting_the_target() -> TestResult {
    let (mut state, connection_id, rule_id) = fixture();
    let mut private_key_connection = state.connections[0].clone();
    private_key_connection.auth = AuthMethod::PrivateKey {
        path: "/Users/operator/.ssh/old-key".into(),
    };
    state.update_connection(private_key_connection)?;
    let snapshot = candidate(&state, connection_id, rule_id);
    let snapshot_id = snapshot.id;
    let document = serde_json::to_string(&snapshot)?;
    assert!(!document.contains("/Users/operator/.ssh/old-key"));
    assert!(document.contains("auth_target_fingerprints"));
    state.upsert_tunnel_runtime_snapshot(snapshot)?;

    let mut changed_key = state.connections[0].clone();
    changed_key.auth = AuthMethod::PrivateKey {
        path: "/Users/operator/.ssh/new-key".into(),
    };
    state.update_connection(changed_key)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::AuthenticationTargetChanged)
    );

    let mut password_connection = state.connections[0].clone();
    password_connection.auth = AuthMethod::Password;
    password_connection.credential_ref = Some(uuid::Uuid::new_v4());
    state.update_connection(password_connection)?;
    let password_snapshot = TunnelRuntimeSnapshot::new(
        state.tunnel_rules[0].clone(),
        TunnelConnectionBinding::from_state(&state, connection_id)?,
        "127.0.0.1:40125",
        1_700_000_002,
    )?;
    let password_snapshot_id = password_snapshot.id;
    state.upsert_tunnel_runtime_snapshot(password_snapshot)?;
    let mut rotated_credential = state.connections[0].clone();
    rotated_credential.credential_ref = Some(uuid::Uuid::new_v4());
    state.update_connection(rotated_credential)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(password_snapshot_id),
        Err(TunnelRecoveryError::AuthenticationTargetChanged)
    );
    Ok(())
}

#[test]
fn missing_rule_and_unsupported_version_keep_candidates_fail_closed() -> TestResult {
    let (mut state, connection_id, rule_id) = fixture();
    let mut snapshot = candidate(&state, connection_id, rule_id);
    let snapshot_id = snapshot.id;
    state.upsert_tunnel_runtime_snapshot(snapshot.clone())?;
    state.remove_tunnel_rule(rule_id)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::RuleMissing)
    );

    let (mut state, connection_id, rule_id) = fixture();
    snapshot = candidate(&state, connection_id, rule_id);
    snapshot.snapshot_version = 99;
    let snapshot_id = snapshot.id;
    state.upsert_tunnel_runtime_snapshot(snapshot)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::UnsupportedSnapshotVersion)
    );
    let json = serde_json::to_value(&state)?;
    let decoded: AppState = serde_json::from_value(json)?;
    assert_eq!(decoded.tunnel_runtime_snapshots.len(), 1);

    let (mut state, connection_id, rule_id) = fixture();
    let mut snapshot = candidate(&state, connection_id, rule_id);
    snapshot.status = TunnelRuntimeStatus::CleanupUnconfirmed;
    let snapshot_id = snapshot.id;
    state.upsert_tunnel_runtime_snapshot(snapshot)?;
    assert_eq!(
        state.assess_tunnel_runtime_snapshot(snapshot_id),
        Err(TunnelRecoveryError::CleanupUnconfirmed)
    );
    Ok(())
}

#[test]
fn recovery_confirmation_is_token_bound_and_state_stale() -> TestResult {
    let (mut state, connection_id, rule_id) = fixture();
    let snapshot = candidate(&state, connection_id, rule_id);
    let snapshot_id = snapshot.id;
    state.upsert_tunnel_runtime_snapshot(snapshot)?;
    let plan = state.plan_tunnel_runtime_recovery(snapshot_id)?;
    let other_token = {
        let other_plan = state.plan_tunnel_runtime_recovery(snapshot_id)?;
        other_plan.review_token()
    };
    assert_eq!(
        plan.clone()
            .confirm(&state, other_token)
            .expect_err("wrong token"),
        TunnelRecoveryError::TokenMismatch
    );
    assert!(plan.clone().confirm(&state, plan.review_token()).is_ok());

    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let state = store.save(&state)?;
    let plan = state.plan_tunnel_runtime_recovery(snapshot_id)?;
    let mut changed = state.clone();
    changed.settings.scrollback_lines += 1;
    let changed = store.save(&changed)?;
    let token = plan.review_token();
    assert_eq!(
        plan.confirm(&changed, token).expect_err("changed revision"),
        TunnelRecoveryError::StateChanged
    );
    Ok(())
}

#[test]
fn confirmed_cleanup_or_explicit_discard_removes_only_the_selected_candidate() -> TestResult {
    let (mut state, connection_id, first_rule_id) = fixture();
    let first = candidate(&state, connection_id, first_rule_id);
    let first_id = first.id;
    let mut second_rule = state.tunnel_rules[0].clone();
    second_rule.id = uuid::Uuid::new_v4();
    second_rule.name = "Second".into();
    state.add_tunnel_rule(second_rule.clone())?;
    let second = TunnelRuntimeSnapshot::new(
        second_rule,
        TunnelConnectionBinding::from_state(&state, connection_id)?,
        "127.0.0.1:40124",
        1_700_000_001,
    )?;
    let second_id = second.id;
    state.upsert_tunnel_runtime_snapshot(first)?;
    state.upsert_tunnel_runtime_snapshot(second)?;

    state.remove_tunnel_runtime_snapshot(first_id)?;
    assert_eq!(state.tunnel_runtime_snapshots.len(), 1);
    assert_eq!(state.tunnel_runtime_snapshots[0].id, second_id);
    assert!(matches!(
        state.remove_tunnel_runtime_snapshot(first_id),
        Err(Error::TunnelRuntimeSnapshotNotFound)
    ));

    state.tunnel_runtime_snapshots[0].status = TunnelRuntimeStatus::CleanupUnconfirmed;
    state.validate()?;
    assert_eq!(state.tunnel_runtime_snapshots.len(), 1);
    Ok(())
}
