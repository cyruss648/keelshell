use keelshell_core::{AppState, AuthMethod, Connection, Error, Settings, Snippet};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn fresh_state_has_no_hosts_and_only_valid_templates() -> TestResult {
    let state = AppState::default();
    state.validate()?;
    assert!(state.connections.is_empty());
    assert!(!state.snippets.is_empty());
    assert!(!state.settings.ai.enabled);
    Ok(())
}

#[test]
fn search_combines_name_group_host_user_and_tags() {
    let mut prod = Connection::new("数据库 主节点", "db.example.test", "admin");
    prod.group = "Production".into();
    prod.tags = vec!["Postgres".into()];
    let state = AppState {
        connections: vec![prod, Connection::new("Dev", "localhost", "dev")],
        ..AppState::default()
    };
    assert_eq!(
        state
            .search_connections("production 数据库 POSTGRES admin")
            .len(),
        1
    );
    assert!(state.search_connections("production dev").is_empty());
    assert_eq!(state.search_connections("").len(), 2);
    assert_eq!(state.groups(), vec!["Production"]);
}

#[test]
fn clone_uses_new_identity_and_preserves_authentication_reference() -> TestResult {
    let mut connection = Connection::new("Host", "example.test", "user");
    connection.auth = AuthMethod::PrivateKey {
        path: "keys/identity".into(),
    };
    connection.credential_ref = Some(uuid::Uuid::new_v4());
    connection.favorite = true;
    let original_id = connection.id;
    let mut state = AppState {
        connections: vec![connection],
        ..AppState::default()
    };
    let copied_id = state.duplicate_connection(original_id)?;
    assert_ne!(original_id, copied_id);
    assert_eq!(state.connections[1].name, "Host (copy)");
    assert_eq!(state.connections[0].auth, state.connections[1].auth);
    assert!(state.connections[1].credential_ref.is_none());
    assert!(state.connections[1].favorite);
    Ok(())
}

#[test]
fn import_is_idempotent_by_normalized_endpoint_and_preserves_existing_values() -> TestResult {
    let existing = Connection::new("Keep this name", "DB.example.test.", "admin");
    let mut imported = Connection::new("Different name", "db.example.test", "admin");
    imported.port = existing.port;
    let source = AppState {
        connections: vec![imported, Connection::new("New", "new.example.test", "user")],
        ..AppState::default()
    };
    let export = source.export_connections()?;
    let mut target = AppState {
        connections: vec![existing],
        ..AppState::default()
    };
    let report = target.import_connections(&export)?;
    assert_eq!((report.added, report.skipped), (1, 1));
    assert_eq!(target.connections[0].name, "Keep this name");
    assert_ne!(target.connections[1].id, source.connections[1].id);
    assert_eq!(target.import_connections(&export)?.added, 0);
    Ok(())
}

#[test]
fn import_validation_is_atomic_when_later_profile_is_invalid() -> TestResult {
    let source = AppState {
        connections: vec![
            Connection::new("Good", "good.test", "user"),
            Connection::new("Bad", "bad.test", "user"),
        ],
        ..AppState::default()
    };
    let mut document: serde_json::Value = serde_json::from_str(&source.export_connections()?)?;
    document["connections"][1]["port"] = 0.into();
    let mut target = AppState::default();
    let original = target.clone();
    assert!(matches!(
        target.import_connections(&serde_json::to_string(&document)?),
        Err(Error::Validation(_))
    ));
    assert_eq!(target, original);
    Ok(())
}

#[test]
fn repeated_ids_inside_an_import_are_deduplicated() -> TestResult {
    let source = AppState {
        connections: vec![Connection::new("Host", "host.test", "user")],
        ..AppState::default()
    };
    let mut document: serde_json::Value = serde_json::from_str(&source.export_connections()?)?;
    let connection = document["connections"][0].clone();
    document["connections"] = serde_json::json!([connection, connection]);
    let mut target = AppState::default();
    let report = target.import_connections(&serde_json::to_string(&document)?)?;
    assert_eq!((report.added, report.skipped), (1, 1));
    target.validate()?;
    Ok(())
}

#[test]
fn export_excludes_snippets_and_ai_settings() -> TestResult {
    let mut state = AppState::default();
    state.settings.ai.model = "private-model-name".into();
    state.snippets.push(Snippet::new(
        "Private command",
        "echo private-command-marker",
    ));
    state
        .connections
        .push(Connection::new("Host", "host.test", "user"));
    let export = state.export_connections()?;
    assert!(!export.contains("private-model-name"));
    assert!(!export.contains("private-command-marker"));
    assert!(!export.contains("settings"));
    Ok(())
}

#[test]
fn persistent_authentication_rejects_embedded_password_fields() -> TestResult {
    let mut value = serde_json::to_value(Connection::new("Host", "host.test", "user"))?;
    value["auth"] = serde_json::json!({"type":"password", "password":"never-store-this"});
    assert!(serde_json::from_value::<Connection>(value).is_err());
    Ok(())
}

#[test]
fn connection_validation_rejects_commands_urls_controls_and_zero_port() {
    for host in [
        "-oProxyCommand=bad",
        "ssh://host",
        "http:example.test",
        "user@host",
        "host\nother",
        "host/path",
        "[::1]",
    ] {
        assert!(
            Connection::new("Test", host, "user").validate().is_err(),
            "accepted {host:?}"
        );
    }
    for host in ["localhost", "example.test", "127.0.0.1", "2001:db8::1"] {
        assert!(
            Connection::new("Test", host, "user").validate().is_ok(),
            "rejected {host:?}"
        );
    }
    let mut zero_port = Connection::new("Test", "localhost", "user");
    zero_port.port = 0;
    assert!(zero_port.validate().is_err());
}

#[test]
fn settings_refuse_nonfinite_sizes_unbounded_scrollback_and_url_credentials() {
    let mut settings = Settings {
        font_size: f32::NAN,
        ..Settings::default()
    };
    assert!(settings.validate().is_err());
    settings.font_size = 14.0;
    settings.scrollback_lines = usize::MAX;
    assert!(settings.validate().is_err());
    settings.scrollback_lines = 1000;
    for endpoint in [
        "https://token@example.test/v1",
        "https://example.test/v1?key=secret",
        "file:///tmp/model",
    ] {
        settings.ai.base_url = endpoint.into();
        assert!(settings.validate().is_err());
    }
}

#[test]
fn snippets_allow_multiline_commands_but_reject_escape_sequences() {
    assert!(
        Snippet::new("Multiline", "pwd\nprintf '\\n'\n")
            .validate()
            .is_ok()
    );
    assert!(Snippet::new("Escape", "echo ok\x1b[2J").validate().is_err());
}

#[test]
fn duplicate_connection_ids_are_rejected() {
    let connection = Connection::new("Host", "host.test", "user");
    let state = AppState {
        connections: vec![connection.clone(), connection],
        ..AppState::default()
    };
    assert!(matches!(state.validate(), Err(Error::Validation(_))));
}

#[test]
fn removing_connection_returns_metadata_and_favorite_toggle_is_persistable() -> TestResult {
    let mut first = Connection::new("First", "first.example.test", "operator");
    first.group = "Production".into();
    let second = Connection::new("Second", "second.example.test", "operator");
    let first_id = first.id;
    let second_id = second.id;
    let mut state = AppState {
        connections: vec![first, second],
        ..AppState::default()
    };

    assert!(state.toggle_connection_favorite(first_id)?);
    assert!(state.connections[0].favorite);
    assert!(!state.toggle_connection_favorite(first_id)?);

    let removed = state.remove_connection(first_id)?;
    assert_eq!(removed.name, "First");
    assert_eq!(state.connections.len(), 1);
    assert_eq!(state.connections[0].id, second_id);
    assert!(matches!(
        state.remove_connection(first_id),
        Err(Error::ConnectionNotFound)
    ));
    Ok(())
}
