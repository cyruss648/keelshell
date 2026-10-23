#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use keelshell_core::{
    AppState, Connection, Error, StateStore, TunnelRule, TunnelRuleAction, TunnelRuleKind,
    TunnelRuleReviewError,
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
    assert_eq!(plan.clone().confirm(token)?.plan().rule().id, id);
    let other_token = state
        .plan_tunnel_rule_action(id, TunnelRuleAction::Stop)?
        .review_token();
    assert_eq!(
        plan.confirm(other_token)
            .expect_err("a different token must fail"),
        TunnelRuleReviewError::TokenMismatch
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
