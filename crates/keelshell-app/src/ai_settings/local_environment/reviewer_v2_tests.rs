//! Independent checks for disclosure guards and unfinished capability drafts.

use gpui_kit::{AppContext, TestAppContext, test::TestAppContextExt};
use keelshell_ai::AiError;
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiLocalAgent, AiPreset, NamedAiProfile,
};
use zeroize::Zeroizing;

fn probe_profile(agent: AiLocalAgent, executable: String) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::Custom);
    profile.name = "Independent probe admission".into();
    profile.model.clear();
    profile.endpoint = "https://provider.example/v1".into();
    profile.backend = AiBackend::LocalAgent {
        working_directory: Default::default(),
        agent,
        executable,
        limits: Default::default(),
    };
    (profile.api_style, profile.authentication) = match agent {
        AiLocalAgent::Codex => (
            AiApiStyle::Responses,
            AiAuthentication::Bearer { credential: None },
        ),
        AiLocalAgent::ClaudeCode => (
            AiApiStyle::AnthropicMessages,
            AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            },
        ),
    };
    profile
}

#[gpui_kit::test]
fn reviewer_v2_retained_non_environment_secrets_reject_hidden_unfinished_drafts(
    cx: &mut TestAppContext,
) {
    const SECRET: &str = "reviewer_saved_disclosure_value";
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        for source in ["authentication", "request_draft"] {
            for placement in ["name", "encoded_endpoint"] {
                let executable = std::env::temp_dir()
                    .join("never-invoked-reviewer-probe")
                    .to_string_lossy()
                    .into_owned();
                let (window, panel) =
                    crate::ai_settings::tests::mount(cx, probe_profile(agent, executable));
                cx.update_window(window, |_, _, cx| panel.update(cx, |panel, cx| {
                    let owner = uuid::Uuid::new_v4();
                    if source == "authentication" {
                        panel.credentials.insert(owner, Zeroizing::new(SECRET.into()));
                    } else {
                        panel.credentials.retain_request_drafts(owner, vec![Zeroizing::new(SECRET.into())]);
                    }
                    let mut hidden = crate::ai_settings::tests::fixture_profile();
                    hidden.model.clear();
                    match placement {
                        "name" => hidden.name = SECRET.into(),
                        _ => hidden.endpoint = "https://provider.example/%72eviewer_saved_disclosure_value/chat/completions".into(),
                    }
                    panel.catalog.profiles.push(hidden);
                    assert!(panel.catalog.validate().is_err(), "unfinished drafts remain structurally invalid");
                    assert!(matches!(crate::ai_request_options::validate_catalog_secrets(&panel.catalog, &panel.credentials), Err(AiError::CredentialInContext)), "disclosure traversal must reach invalid drafts");
                    panel.apply(cx);
                    assert!(!panel.saving, "Apply still rejects incomplete metadata");
                    panel.start_local_probe(cx);
                    assert!(panel._job.is_none() && panel.operation.is_none() && panel.cancellation.is_none(), "known-secret admission precedes any job");
                    let message = panel.status.render(cx);
                    assert!(message.contains("无法检查 CLI") || message.contains("Cannot check CLI"));
                    assert!(!message.contains(SECRET));
                })).unwrap_or_else(|error| panic!("hidden draft admission: {error}"));
                cx.run_until_parked();
            }
        }
    }
}

#[gpui_kit::test]
fn reviewer_v2_overbound_retained_request_secret_set_stops_probe_before_scheduling(
    cx: &mut TestAppContext,
) {
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let executable = std::env::temp_dir()
            .join("never-invoked-reviewer-bound-probe")
            .to_string_lossy()
            .into_owned();
        let (window, panel) =
            crate::ai_settings::tests::mount(cx, probe_profile(agent, executable));
        cx.update_window(window, |_, _, cx| {
            panel.update(cx, |panel, cx| {
                panel.credentials.retain_request_drafts(
                    uuid::Uuid::new_v4(),
                    (0..4097)
                        .map(|i| Zeroizing::new(format!("independent-bound-{i}")))
                        .collect(),
                );
                assert!(matches!(
                    crate::ai_request_options::validate_catalog_secrets(
                        &panel.catalog,
                        &panel.credentials
                    ),
                    Err(AiError::ContextTooLarge)
                ));
                panel.start_local_probe(cx);
                assert!(
                    panel._job.is_none()
                        && panel.operation.is_none()
                        && panel.cancellation.is_none()
                );
                let status = panel.status.render(cx);
                assert!(!status.contains("independent-bound-"));
            })
        })
        .unwrap_or_else(|error| panic!("overbound retained request set: {error}"));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
#[ignore = "requires explicitly supplied owned native mock-child, never a supplier executable"]
async fn reviewer_v2_empty_model_and_hidden_incomplete_draft_allow_owned_capability_probe(
    cx: &mut TestAppContext,
) {
    let supplied = std::env::var_os("KEELSHELL_REVIEW_PROBE_FIXTURE")
        .unwrap_or_else(|| panic!("owned fixture required"));
    for agent in [AiLocalAgent::Codex, AiLocalAgent::ClaudeCode] {
        let directory =
            tempfile::tempdir().unwrap_or_else(|error| panic!("owned probe directory: {error}"));
        let label = match agent {
            AiLocalAgent::Codex => "codex",
            AiLocalAgent::ClaudeCode => "claude",
        };
        let executable = directory.path().join(format!(
            "{label}-review-probe-sensitive-agent-fixture{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::copy(&supplied, &executable)
            .unwrap_or_else(|error| panic!("owned executable copy: {error}"));
        let (window, panel) = crate::ai_settings::tests::mount(
            cx,
            probe_profile(agent, executable.to_string_lossy().into_owned()),
        );
        cx.update_window(window, |_, _, cx| {
            panel.update(cx, |panel, cx| {
                let mut hidden = crate::ai_settings::tests::fixture_profile();
                hidden.name.clear();
                hidden.model.clear();
                panel.catalog.profiles.push(hidden);
                panel.credentials.retain_request_drafts(
                    uuid::Uuid::new_v4(),
                    vec![Zeroizing::new("retained-private-reviewer-value".into())],
                );
                assert!(panel.catalog.validate().is_err());
                assert!(
                    crate::ai_request_options::validate_catalog_secrets(
                        &panel.catalog,
                        &panel.credentials
                    )
                    .is_ok()
                );
                panel.apply(cx);
                assert!(!panel.saving);
                panel.start_local_probe(cx);
                assert!(panel._job.is_some());
            })
        })
        .unwrap_or_else(|error| panic!("unfinished capability draft: {error}"));
        cx.wait_for(window, std::time::Duration::from_secs(12), |_, cx| {
            crate::ai_settings::tests::request_has_finished(&panel, cx)
        })
        .await;
        let status = panel.read_with(cx, |panel, cx| panel.status.render(cx));
        assert!(
            status.contains("检查通过") || status.contains("capabilities checked"),
            "{status}"
        );
        let raw = std::fs::read_to_string(directory.path().join("review-probe-invocations.jsonl"))
            .unwrap_or_else(|error| panic!("actual owned argv: {error}"));
        let rows: Vec<serde_json::Value> = raw
            .lines()
            .map(|line| {
                serde_json::from_str(line).unwrap_or_else(|error| panic!("argv JSON: {error}"))
            })
            .collect();
        assert_eq!(rows.len(), if agent == AiLocalAgent::Codex { 3 } else { 2 });
        assert!(!raw.contains("retained-private-reviewer-value"));
        eprintln!(
            "reviewer-v2-legal-probe {}",
            serde_json::json!({"agent":label,"empty_model":true,"hidden_incomplete_draft":true,"invocation_count":rows.len(),"status":status,"records":rows,"boundary":"owned native mock-child only; no supplier/model/network/GUI; kernel birth census not collected"})
        );
        directory
            .close()
            .unwrap_or_else(|error| panic!("owned directory removal: {error}"));
    }
}
