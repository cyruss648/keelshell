use super::*;

#[gpui_kit::test]
fn ordinary_ask_command_stays_reviewable_and_requires_a_manual_click(cx: &mut TestAppContext) {
    let (handle, panel) = mount(cx);
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = observed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event: &AssistantEvent, _| {
            if let AssistantEvent::Suggestion {
                command,
                session_id,
            } = event
            {
                captured
                    .lock()
                    .unwrap_or_else(|_| panic!("suggestion lock"))
                    .push((command.clone(), session_id.clone()));
            }
        })
    });
    panel.update(cx, |panel, cx| {
        panel.finish_reply(
            panel.request_revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("```sh\nprintf 'ask-progress-review-canary'\n```".into()),
            cx,
        );
        assert_eq!(panel.suggestions, ["printf 'ask-progress-review-canary'"]);
    });
    assert!(
        observed
            .lock()
            .unwrap_or_else(|_| panic!("suggestion lock"))
            .is_empty()
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "assistant-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find(("review-suggestion", 0_usize)).visible());
        window.click(("review-suggestion", 0_usize), cx);
    })
    .unwrap_or_else(|error| panic!("manual command review: {error}"));
    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(|_| panic!("suggestion lock")),
        [(
            "printf 'ask-progress-review-canary'".into(),
            "session-A".into()
        )]
    );
}

#[gpui_kit::test]
fn embedded_configured_secret_stays_hidden_in_reply_and_suggestions(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let selected = panel.profile.clone().unwrap_or_else(|| panic!("profile"));
        let catalog = panel.profiles.clone();
        let mut credentials = EphemeralCredentials::new();
        credentials.insert(selected.id, Zeroizing::new("sk-fixture-only-value".into()));
        panel.set_profiles(&catalog, &credentials, cx);
        panel.finish_reply(
            panel.request_revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("```sh\nprintf 'prefixsk-fixture-only-valuesuffix'\n```".into()),
            cx,
        );
        assert_eq!(
            panel.response,
            "```sh\nprintf 'prefix[REDACTED]suffix'\n```"
        );
        assert_eq!(panel.suggestions, ["printf 'prefix[REDACTED]suffix'"]);
    });
}
