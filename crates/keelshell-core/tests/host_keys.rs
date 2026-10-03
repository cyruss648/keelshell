use keelshell_core::AppState;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fingerprint() -> String {
    format!("SHA256:{}", "A".repeat(43))
}

#[test]
fn dns_and_ipv6_addresses_use_canonical_host_port_keys() -> TestResult {
    let mut state = AppState::default();
    let pin = fingerprint();
    state.trust_host_key("Example.COM.", 22, &pin)?;
    state.trust_host_key("0:0:0:0:0:0:0:1", 2222, &pin)?;
    assert!(state.known_hosts.contains_key("[example.com]:22"));
    assert!(state.known_hosts.contains_key("[::1]:2222"));
    assert_eq!(state.host_key("::1", 2222), Some(pin.as_str()));
    assert!(state.host_key("::1", 22).is_none());
    Ok(())
}

#[test]
fn invalid_fingerprints_and_destinations_do_not_modify_trust() {
    let mut state = AppState::default();
    for pin in [
        "SHA256:short",
        "MD5:001122",
        "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB",
    ] {
        assert!(state.trust_host_key("example.test", 22, pin).is_err());
    }
    for (host, port) in [
        ("https://example.test", 22),
        ("[::1]", 22),
        ("host", 0),
        (" ", 22),
    ] {
        assert!(state.trust_host_key(host, port, &fingerprint()).is_err());
    }
    assert!(state.known_hosts.is_empty());
}

#[test]
fn direct_edits_to_the_trust_map_are_validated_before_persistence() {
    let mut state = AppState::default();
    state
        .known_hosts
        .insert("[EXAMPLE.test]:22".into(), fingerprint());
    assert!(state.validate().is_err());
    state.known_hosts.clear();
    state
        .known_hosts
        .insert("[example.test]:22".into(), "SHA256:short".into());
    assert!(state.validate().is_err());
}

#[test]
fn connection_exports_and_imports_do_not_transfer_host_trust() -> TestResult {
    let mut source = AppState::default();
    source.trust_host_key("source.test", 22, &fingerprint())?;
    let export = source.export_connections()?;
    assert!(!export.contains("known_hosts"));
    assert!(!export.contains("SHA256:"));
    let mut target = AppState::default();
    target.trust_host_key("local.test", 2222, &fingerprint())?;
    let before = target.known_hosts.clone();
    target.import_connections(&export)?;
    assert_eq!(target.known_hosts, before);
    assert!(target.host_key("source.test", 22).is_none());
    Ok(())
}
