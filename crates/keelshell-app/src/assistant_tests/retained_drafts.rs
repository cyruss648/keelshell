use super::*;
use crate::ai_settings::tests::{mount_sized, request_draft_credentials};

#[gpui_kit::test]
fn actual_duplicate_settings_inputs_guard_api_and_local_reviews_and_reply_lifecycle(
    cx: &mut TestAppContext,
) {
    const FIRST: &str = "actual-retained-first-secret";
    const SECOND: &str = "actual-retained-second-secret";
    let (settings_handle, settings) = mount_sized(cx, profile("Inactive draft owner"), 900., 580.);
    for (index, value) in [FIRST, SECOND].into_iter().enumerate() {
        cx.update_window(settings_handle, |_, window, cx| {
            window.render_frame(cx);
            reveal_request_option(window, "ai-header-add".into(), cx);
            window.click("ai-header-add", cx);
        })
        .unwrap_or_else(|error| panic!("add actual header: {error}"));
        input_request_option(
            settings_handle,
            ("ai-header-name", index).into(),
            "x-duplicate",
            cx,
        );
        input_request_option(
            settings_handle,
            ("ai-header-value", index).into(),
            value,
            cx,
        );
    }
    let (draft_owner, credentials) =
        settings.read_with(cx, |_, cx| request_draft_credentials(&settings, cx));
    assert!(credentials.all_secrets().contains(&FIRST));
    assert!(credentials.all_secrets().contains(&SECOND));
    let (handle, assistant) = mount(cx);
    for style in [
        AiApiStyle::ChatCompletions,
        AiApiStyle::Responses,
        AiApiStyle::AnthropicMessages,
    ] {
        let mut active = profile("Active API");
        active.api_style = style;
        active.endpoint = format!(
            "https://provider.example/v1/{}",
            match style {
                AiApiStyle::ChatCompletions => "chat/completions",
                AiApiStyle::Responses => "responses",
                AiApiStyle::AnthropicMessages => "messages",
            }
        );
        let catalog = AiProfileCatalog {
            active_id: Some(active.id),
            profiles: vec![active.clone()],
        };
        cx.update_window(handle, |_, window, cx| {
            assistant.update(cx, |panel, cx| {
                panel.set_profiles(&catalog, &credentials, cx);
                panel.prompt.update(cx, |field, cx| {
                    field.set_value(format!("Explain {FIRST} {SECOND}"), window, cx)
                });
                panel.prepare(cx);
                let request = panel
                    .prepared
                    .as_ref()
                    .unwrap_or_else(|| panic!("API review"));
                assert!(!request.preview_json().contains(FIRST));
                assert!(!request.preview_json().contains(SECOND));
                let revision = panel.request_revision;
                panel.finish_reply(
                    revision,
                    ("host".into(), "session-A".into()),
                    Ok(format!(
                        "answer {FIRST} {SECOND}\n```sh\nprintf '{FIRST}'\n```"
                    )),
                    cx,
                );
                assert!(!panel.response.contains(FIRST));
                assert!(!panel.response.contains(SECOND));
                assert!(
                    panel
                        .suggestions
                        .iter()
                        .all(|command| !command.contains(FIRST))
                );
                active.model = FIRST.into();
                panel.set_profile(Some(active.clone()), None, cx);
                panel.prepare(cx);
                assert!(panel.prepared.is_none());
                assert!(panel._job.is_none());
            })
        })
        .unwrap_or_else(|error| panic!("API draft guard: {error}"));
    }
    for agent in [
        keelshell_core::AiLocalAgent::Codex,
        keelshell_core::AiLocalAgent::ClaudeCode,
    ] {
        let active = local_profile(agent);
        let mut local_credentials = credentials.clone();
        local_credentials.insert(active.id, Zeroizing::new("fixture-key".into()));
        let catalog = AiProfileCatalog {
            active_id: Some(active.id),
            profiles: vec![active],
        };
        cx.update_window(handle, |_, window, cx| {
            assistant.update(cx, |panel, cx| {
                panel.set_profiles(&catalog, &local_credentials, cx);
                panel.prompt.update(cx, |field, cx| {
                    field.set_value(format!("Explain {FIRST} {SECOND}"), window, cx)
                });
                panel.prepare(cx);
                let request = panel
                    .prepared
                    .as_ref()
                    .unwrap_or_else(|| panic!("local review"));
                assert!(!request.preview_json().contains(FIRST));
                assert!(!request.preview_json().contains(SECOND));
                assert!(panel._job.is_none());
            })
        })
        .unwrap_or_else(|error| panic!("local draft review: {error}"));
    }
    assistant.update(cx, |panel, cx| {
        let catalog = panel.profiles.clone();
        let old_revision = panel.request_revision;
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        // A snapshot replacement must compare the retained pool itself, even
        // when there is no valid delivery slot for a duplicate-name header.
        let mut cleared = panel.credentials.clone();
        cleared.clear_requests(draft_owner);
        panel.set_profiles(&catalog, &cleared, cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.prepared.is_none());
        assert!(panel.response.is_empty());
        panel.finish_reply(
            old_revision,
            ("host".into(), "session-A".into()),
            Ok(format!("late {FIRST}")),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
    });
}
