#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use keelshell_core::{
    AppState, Connection, ConnectionLibraryAction, ConnectionLibraryBatchError, Error, StateStore,
    TunnelRule, TunnelRuleAction, TunnelRuleKind, TunnelRuleReviewError,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn enabled_rule(connection_id: uuid::Uuid) -> Result<TunnelRule, Box<dyn std::error::Error>> {
    let mut rule = TunnelRule::new(
        "Database",
        connection_id,
        TunnelRuleKind::Local,
        "127.0.0.1",
        15432,
        Some("db.internal".into()),
        Some(5432),
    )?;
    rule.enabled = true;
    Ok(rule)
}

#[test]
fn named_rules_round_trip_through_state_store_without_runtime_side_effects() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    let connection = Connection::new("Database host", "host.test", "operator");
    let connection_id = connection.id;
    state.connections.push(connection);
    let rule = enabled_rule(connection_id)?;
    let rule_id = state.add_tunnel_rule(rule.clone())?;
    let saved = store.save(&state)?;

    let reopened = StateStore::new(store.path());
    let loaded = reopened.load()?;
    assert_eq!(loaded.tunnel_rules, vec![rule]);
    assert_eq!(loaded.tunnel_rules[0].id, rule_id);
    assert!(loaded.tunnel_rules[0].enabled);
    assert!(fs::read_to_string(store.path())?.contains("tunnel_rules"));
    assert_eq!(loaded, saved);
    Ok(())
}

#[test]
fn legacy_state_without_tunnel_rules_migrates_to_empty_without_rewriting_on_load() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let state = store.load()?;
    let state = store.save(&state)?;
    let mut document = serde_json::to_value(&state)?;
    document
        .as_object_mut()
        .ok_or("state must be an object")?
        .remove("tunnel_rules");
    let legacy = serde_json::to_vec_pretty(&document)?;
    fs::write(&path, &legacy)?;

    let loaded = StateStore::new(&path).load()?;
    assert!(loaded.tunnel_rules.is_empty());
    assert_eq!(fs::read(&path)?, legacy);
    Ok(())
}

#[test]
fn rule_binding_and_review_are_explicit_and_do_not_start_a_listener() -> TestResult {
    let mut state = AppState::default();
    let connection = Connection::new("Host", "host.test", "operator");
    state.connections.push(connection.clone());
    let rule = enabled_rule(connection.id)?;
    let id = state.add_tunnel_rule(rule)?;

    let plan = state.plan_tunnel_rule_action(id, TunnelRuleAction::Start)?;
    let token = plan.review_token();
    assert_eq!(plan.clone().confirm(&state, token)?.plan().rule().id, id);
    let other_token = state
        .plan_tunnel_rule_action(id, TunnelRuleAction::Stop)?
        .review_token();
    assert_eq!(
        plan.confirm(&state, other_token)
            .expect_err("a different token must fail"),
        TunnelRuleReviewError::TokenMismatch
    );
    Ok(())
}

#[test]
fn deleting_a_connection_with_a_named_rule_is_rejected_atomically() -> TestResult {
    let connection = Connection::new("Host", "host.test", "operator");
    let connection_id = connection.id;
    let mut state = AppState::default();
    state.connections.push(connection.clone());
    state.add_tunnel_rule(enabled_rule(connection_id)?)?;

    let error = state
        .soft_delete_connection(connection_id, 1)
        .expect_err("a referenced connection must remain active");
    assert!(matches!(error, Error::ConnectionReferencedByTunnelRule));
    assert!(
        state
            .connections
            .iter()
            .any(|item| item.id == connection_id)
    );
    assert!(state.deleted_connections.is_empty());

    let error = state
        .remove_connection(connection_id)
        .expect_err("hard deletion must use the same reference guard");
    assert!(matches!(error, Error::ConnectionReferencedByTunnelRule));
    assert!(state.validate().is_ok());
    Ok(())
}

#[test]
fn purging_a_deleted_connection_with_a_named_rule_is_rejected() -> TestResult {
    let connection = Connection::new("Host", "host.test", "operator");
    let connection_id = connection.id;
    let mut state = AppState::default();
    state
        .deleted_connections
        .push(keelshell_core::DeletedConnection {
            connection,
            deleted_at: 1,
        });
    state.tunnel_rules.push(enabled_rule(connection_id)?);

    let error = state
        .purge_deleted_connection(connection_id)
        .expect_err("purge must preserve referenced metadata");
    assert!(matches!(error, Error::ConnectionReferencedByTunnelRule));
    assert_eq!(state.deleted_connections.len(), 1);
    Ok(())
}

#[test]
fn stale_review_plan_is_rejected_after_connection_update() -> TestResult {
    let connection = Connection::new("Host", "host.test", "operator");
    let connection_id = connection.id;
    let mut state = AppState::default();
    state.connections.push(connection.clone());
    let rule = enabled_rule(connection_id)?;
    let rule_id = state.add_tunnel_rule(rule)?;
    let plan = state.plan_tunnel_rule_action(rule_id, TunnelRuleAction::Start)?;
    let token = plan.review_token();

    let mut changed = connection;
    changed.host = "changed.test".into();
    state.update_connection(changed)?;

    assert_eq!(
        plan.confirm(&state, token),
        Err(TunnelRuleReviewError::StateChanged)
    );
    Ok(())
}

#[test]
fn batch_trash_rejects_any_referenced_connection_without_partial_deletion() -> TestResult {
    let referenced = Connection::new("Referenced", "referenced.test", "operator");
    let unrelated = Connection::new("Unrelated", "unrelated.test", "operator");
    let referenced_id = referenced.id;
    let unrelated_id = unrelated.id;
    let mut state = AppState::default();
    state.connections.extend([referenced, unrelated]);
    state.add_tunnel_rule(enabled_rule(referenced_id)?)?;
    let before = state.clone();

    let error = state
        .apply_connection_library_batch(
            &[referenced_id, unrelated_id],
            &ConnectionLibraryAction::Trash(1),
        )
        .expect_err("a referenced profile must block the complete batch");
    assert!(matches!(
        error,
        ConnectionLibraryBatchError::InvalidState(Error::ConnectionReferencedByTunnelRule)
    ));
    assert_eq!(state, before);
    assert!(state.validate().is_ok());
    Ok(())
}

#[test]
fn changing_or_removing_a_rule_invalidates_its_old_review_plan() -> TestResult {
    let connection = Connection::new("Host", "host.test", "operator");
    let connection_id = connection.id;
    let mut state = AppState::default();
    state.connections.push(connection);
    let rule = enabled_rule(connection_id)?;
    let rule_id = state.add_tunnel_rule(rule.clone())?;

    let changed_plan = state.plan_tunnel_rule_action(rule_id, TunnelRuleAction::Start)?;
    let changed_token = changed_plan.review_token();
    let mut changed_rule = rule;
    changed_rule.target_host = Some("other.test".into());
    state.update_tunnel_rule(changed_rule)?;
    assert_eq!(
        changed_plan.confirm(&state, changed_token),
        Err(TunnelRuleReviewError::StateChanged)
    );

    let removed_plan = state.plan_tunnel_rule_action(rule_id, TunnelRuleAction::Start)?;
    let removed_token = removed_plan.review_token();
    state.remove_tunnel_rule(rule_id)?;
    assert_eq!(
        removed_plan.confirm(&state, removed_token),
        Err(TunnelRuleReviewError::StateChanged)
    );
    Ok(())
}

#[test]
fn saving_again_rotates_state_revision_and_invalidates_old_review_plan() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = store.load()?;
    let connection = Connection::new("Host", "host.test", "operator");
    let connection_id = connection.id;
    state.connections.push(connection);
    let mut rule = enabled_rule(connection_id)?;
    rule.enabled = true;
    state.add_tunnel_rule(rule.clone())?;
    state = store.save(&state)?;

    let plan = state.plan_tunnel_rule_action(rule.id, TunnelRuleAction::Start)?;
    let token = plan.review_token();
    state = store.save(&state)?;

    assert_eq!(
        plan.confirm(&state, token),
        Err(TunnelRuleReviewError::StateChanged)
    );
    Ok(())
}

#[test]
fn disabled_rule_cannot_be_planned_for_start_and_missing_binding_is_rejected() -> TestResult {
    let mut state = AppState::default();
    let connection = Connection::new("Host", "host.test", "operator");
    let rule = TunnelRule::new(
        "Host tunnel",
        connection.id,
        TunnelRuleKind::Dynamic,
        "127.0.0.1",
        0,
        None,
        None,
    )?;
    state.connections.push(connection);
    let id = state.add_tunnel_rule(rule)?;
    assert!(matches!(
        state.plan_tunnel_rule_action(id, TunnelRuleAction::Start),
        Err(Error::Validation(error)) if error.field == "tunnel_rule.enabled"
    ));

    let mut invalid = state.tunnel_rules[0].clone();
    invalid.connection_id = uuid::Uuid::new_v4();
    assert!(
        matches!(state.update_tunnel_rule(invalid), Err(Error::Validation(error)) if error.field == "tunnel_rule.connection_id")
    );
    Ok(())
}
