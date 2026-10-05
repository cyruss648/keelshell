#[gpui_kit::test]
fn independent_request_options_inactive_proxy_basic_is_redacted_before_any_send(
    cx: &mut TestAppContext,
) {
    use crate::ai_request_options::{RequestSecret, SecretPurpose};
    use keelshell_core::{AiLocalAgent, AiProxy, AiSecretRef};
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut inactive = profile("Inactive synthetic proxy");
        let reference = AiSecretRef::Ephemeral { id: uuid::Uuid::new_v4() };
        inactive.proxy = AiProxy::Explicit {
            url: "http://127.0.0.1:9".into(),
            credentials: Some(reference.clone()),
        };
        let mut credentials = EphemeralCredentials::new();
        credentials.insert_request(
            &inactive,
            SecretPurpose::Proxy,
            reference,
            RequestSecret::Proxy {
                username: Zeroizing::new("synthetic-user".into()),
                password: Zeroizing::new("synthetic-pass".into()),
            },
        );
        // This is the exact HTTP Basic derived representation of the two synthetic values.
        let basic = "c3ludGhldGljLXVzZXI6c3ludGhldGljLXBhc3M=";
        let mut observations = Vec::new();
        for index in 0..5 {
            let mut active = match index {
                3 => local_profile(AiLocalAgent::Codex),
                4 => local_profile(AiLocalAgent::ClaudeCode),
                _ => profile("Active API"),
            };
            if index == 1 {
                active.api_style = AiApiStyle::Responses;
                active.endpoint = "https://provider.example/v1/responses".into();
            } else if index == 2 {
                active.api_style = AiApiStyle::AnthropicMessages;
                active.endpoint = "https://provider.example/v1/messages".into();
            }
            let mut active_credentials = credentials.clone();
            if index >= 3 {
                active_credentials.insert(active.id, Zeroizing::new("fixture-key".into()));
            }
            let catalog = AiProfileCatalog {
                active_id: Some(active.id),
                profiles: vec![active, inactive.clone()],
            };
            panel.set_profiles(&catalog, &active_credentials, cx);
            panel.set_context(format!("selected output {basic}"), "host".into(), "session-A".into(), cx);
            panel.prepare(cx);
            let prepared = panel.prepared.as_ref().unwrap_or_else(|| panic!("expected reviewed request for scenario {index}"));
            let leaked = prepared.preview_json().contains(basic);
            observations.push((index, leaked));
            assert!(!panel.busy);
            assert!(panel._job.is_none(), "preparation must not send or start a CLI");
        }
        eprintln!("independent inactive Basic observations (scenario, present in preview): {observations:?}");
        assert!(observations.iter().all(|(_, leaked)| !leaked), "known inactive proxy Basic must not enter any approved request context");
    });
}
