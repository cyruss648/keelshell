//! Source-bound pre-spawn credential counterexamples, without any CLI process.

use keelshell_ai::{
    ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentError,
    LocalAgentKind, RequestCancellation,
};

#[tokio::test]
async fn independent_actual_send_rejects_credential_in_each_audit_field_before_spawn() {
    const SECRET: &str = "review_binding_value";
    let parent = tempfile::tempdir().unwrap_or_else(|e| panic!("owned parent: {e}"));
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        for field in 0..6 {
            let executable = parent.path().join(if field == 0 {
                SECRET
            } else {
                "never-created-native-cli"
            });
            let scratch = if field == 1 {
                parent.path().join(SECRET)
            } else {
                parent.path().to_owned()
            };
            let model = if field == 2 { SECRET } else { "fixture-model" };
            let endpoint = match field {
                3 => format!("https://provider.example/{SECRET}"),
                5 => "https://provider.example/%72eview_binding_value".into(),
                _ => "https://provider.example/v1".into(),
            };
            let reference = if field == 4 {
                SECRET
            } else {
                "REVIEW_ONLY_REFERENCE"
            };
            let config = LocalAgentConfig::new(kind, executable, scratch, model)
                .and_then(|c| c.with_inference_endpoint(&endpoint))
                .and_then(|c| c.with_credential_environment_reference(Some(reference)))
                .unwrap_or_else(|e| panic!("admitted metadata {kind:?}/{field}: {e}"));
            assert!(
                config
                    .prepare(
                        ContextDraft::new("Explain the selected output"),
                        &[SECRET],
                        8192
                    )
                    .is_err(),
                "known at prepare: {kind:?}/{field}"
            );
            let prepared = config
                .prepare(ContextDraft::new("Explain the selected output"), &[], 8192)
                .unwrap_or_else(|e| panic!("no prior secret: {e}"));
            let actual = LocalAgentClient
                .ask(
                    prepared.approve(),
                    LocalAgentCredential::new(SECRET).unwrap_or_else(|e| panic!("credential: {e}")),
                    &RequestCancellation::new(),
                )
                .await;
            assert_eq!(
                actual.err(),
                Some(LocalAgentError::CredentialInContext),
                "send gate precedes nonexistent executable: {kind:?}/{field}"
            );
        }
    }
    assert!(
        parent
            .path()
            .read_dir()
            .unwrap_or_else(|e| panic!("parent listing: {e}"))
            .next()
            .is_none(),
        "no scratch was created"
    );
    parent
        .close()
        .unwrap_or_else(|e| panic!("parent removed: {e}"));
}
