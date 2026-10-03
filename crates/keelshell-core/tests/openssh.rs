use std::collections::BTreeMap;
use std::fmt::Display;

use keelshell_core::{
    AppState, AuthMethod, parse_openssh_config, parse_openssh_config_with_includes,
};

fn require<T, E: Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error}"),
    }
}

#[test]
fn imports_exact_hosts_and_reports_unsafe_blocks() {
    let config = require(
        parse_openssh_config(
            "Host build\n  HostName build.example\n  User ci\n  Port 2200\n  IdentityFile ~/.ssh/id_ed25519\n  ProxyCommand ssh relay %h %p\nHost *\n  User default\nHost *.wild\n  HostName ignored.example\n",
        ),
        "safe subset should parse",
    );
    assert_eq!(config.entries.len(), 1);
    let entry = &config.entries[0];
    assert_eq!(entry.alias, "build");
    assert_eq!(entry.connection.host, "build.example");
    assert_eq!(entry.connection.port, 2200);
    assert_eq!(entry.connection.username, "ci");
    assert!(matches!(
        entry.connection.auth,
        AuthMethod::PrivateKey { .. }
    ));
    assert!(
        config
            .warnings
            .iter()
            .any(|warning| warning.reason.contains("semantic"))
    );
    assert!(
        config
            .warnings
            .iter()
            .any(|warning| warning.reason.contains("wildcard"))
    );
}

#[test]
fn missing_include_is_a_review_warning_and_never_reads_disk() {
    let config = require(
        parse_openssh_config(
            "Include ~/.ssh/conf.d/*\nHost app\n User deploy\n HostName app.example\n",
        ),
        "missing include is skipped",
    );
    assert_eq!(config.entries.len(), 1);
    assert!(
        config
            .warnings
            .iter()
            .any(|warning| warning.reason.contains("Include"))
    );
}

#[test]
fn supplied_include_is_literal_and_proxy_jump_is_resolved_on_import() {
    let mut includes = BTreeMap::new();
    includes.insert(
        "conf.d/relay".to_owned(),
        "Host relay\n HostName relay.example\n User jump\n".to_owned(),
    );
    let config = require(
        parse_openssh_config_with_includes(
            "Include conf.d/relay\nHost app\n HostName app.example\n User deploy\n ProxyJump relay\n",
            &includes,
        ),
        "literal include should parse",
    );
    assert_eq!(config.entries.len(), 2);
    let mut state = AppState::default();
    let report = require(state.import_openssh_config_with_includes_report(
        "Include conf.d/relay\nHost app\n HostName app.example\n User deploy\n ProxyJump relay\n",
        &includes,
    ), "import should resolve jump alias");
    assert_eq!(report.imported.added, 2);
    assert!(report.warnings.is_empty());
    let Some(app) = state.connections.iter().find(|item| item.name == "app") else {
        panic!("imported app profile");
    };
    assert!(app.jump_host.is_some());
}
