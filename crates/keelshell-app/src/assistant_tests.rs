use super::{AssistantPanel, PreparedRequest, shell_blocks};
use crate::{ai_settings::EphemeralCredentials, i18n::set_language};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_ai::RequestCancellation;
use keelshell_core::{AiAuthentication, AiPreset, AiProfileCatalog, Language, NamedAiProfile};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

fn profile(name: &str) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = name.into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "fixture-model".into();
    profile.authentication = AiAuthentication::None;
    profile
}

fn mount(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<AssistantPanel>) {
    cx.update(gpui_kit::init);
    let one = profile("Profile A");
    let catalog = AiProfileCatalog {
        active_id: Some(one.id),
        profiles: vec![one, profile("Profile B")],
    };
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    let fixture = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(720.), px(900.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
                })
            },
        )
        .unwrap_or_else(|error| panic!("assistant window: {error}"))
    });
    cx.update_window(fixture.0, |_, window, cx| {
        fixture.1.update(cx, |panel, cx| {
            panel.prompt.update(cx, |prompt, cx| {
                prompt.set_value("Explain this output", window, cx)
            });
            panel.set_context(
                "selected output".into(),
                "ops@server.example:22".into(),
                "session-A".into(),
                cx,
            );
        });
    })
    .unwrap_or_else(|error| panic!("assistant fixture: {error}"));
    cx.run_until_parked();
    fixture
}

#[gpui_kit::test]
fn native_question_edit_revokes_approved_payload_and_old_reply(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx);
    let (revision, cancellation) = panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let token = RequestCancellation::new();
        panel.cancellation = Some(token.clone());
        panel.busy = true;
        (panel.request_revision, token)
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("input", panel.read(cx).prompt.entity_id()), cx);
        window.input("x", cx);
    })
    .unwrap_or_else(|error| panic!("edit question: {error}"));
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        assert!(cancellation.is_cancelled());
        assert!(panel.request_revision > revision);
        assert!(panel.prepared.is_none());
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn reconnect_revokes_only_the_retired_context_and_keeps_question(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let revision = panel.request_revision;
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        panel.invalidate_session("different-session", cx);
        assert!(panel.prepared.is_some());
        assert!(!cancellation.is_cancelled());
        panel.invalidate_session("session-A", cx);
        assert!(cancellation.is_cancelled());
        assert_eq!(
            panel.prompt.read(cx).value().as_ref(),
            "Explain this output"
        );
        assert!(panel.context.is_empty());
        assert!(panel.host.is_empty());
        assert!(panel.session_id.is_empty());
        assert!(panel.prepared.is_none());
        panel.finish_request(
            revision,
            ("old-host".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn changing_profile_preserves_host_question_and_context_but_revokes_old_result(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let revision = panel.request_revision;
        let second = panel.profiles.profiles[1].id;
        panel.select_profile(second, cx);
        assert_eq!(panel.profile.as_ref().map(|p| p.id), Some(second));
        assert_eq!(panel.host, "ops@server.example:22");
        assert_eq!(panel.session_id, "session-A");
        assert_eq!(panel.context, "selected output");
        assert_eq!(panel.prompt.read(cx).value(), "Explain this output");
        assert!(panel.prepared.is_none());
        panel.busy = true; // Simulate a new request starting before the old one resolves.
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(
            panel.busy,
            "old response must not clear a newer request's busy state"
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn credential_rotation_and_profile_removal_revoke_preview(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let profile = panel.profile.clone();
        panel.set_profile(profile, Some(Zeroizing::new("temporary-key".into())), cx);
        assert!(panel.prepared.is_none());
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        panel.set_profiles(
            &AiProfileCatalog::default(),
            &EphemeralCredentials::new(),
            cx,
        );
        assert!(panel.profile.is_none());
        assert!(panel.prepared.is_none());
        assert_eq!(panel.session_id, "session-A");
    });
}

#[gpui_kit::test]
fn changing_selected_session_discards_previous_target_and_reply(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let revision = panel.request_revision;
        panel.finish_request(
            revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("```sh\necho reviewed\n```".into()),
            cx,
        );
        assert_eq!(panel.suggestions, ["echo reviewed"]);
        assert_eq!(
            panel
                .response_target
                .as_ref()
                .map(|target| target.1.as_str()),
            Some("session-A")
        );
        panel.set_context(
            "new output".into(),
            "otherhost".into(),
            "session-B".into(),
            cx,
        );
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
        assert_eq!(panel.session_id, "session-B");
    });
}

#[gpui_kit::test]
fn focus_and_language_changes_preserve_the_exact_preview(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx);
    let (revision, payload, selected, status_zh) = panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let payload = panel
            .prepared
            .as_ref()
            .map(PreparedRequest::preview_json)
            .unwrap_or_else(|| panic!("fixture prepares"))
            .to_owned();
        (
            panel.request_revision,
            payload,
            panel.profile.clone(),
            panel.status.render(cx),
        )
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("input", panel.read(cx).prompt.entity_id()), cx);
        set_language(Language::En, cx);
        panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("change locale: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.request_revision, revision);
        assert_eq!(
            panel.prepared.as_ref().map(PreparedRequest::preview_json),
            Some(payload.as_str())
        );
        assert_eq!(panel.profile, selected);
        assert_eq!(panel.prompt.read(cx).value(), "Explain this output");
        assert_eq!(panel.session_id, "session-A");
        assert_ne!(panel.status.render(cx), status_zh);
    });
}

#[gpui_kit::test]
fn unsupported_configuration_and_missing_bearer_key_never_prepare(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut profile = profile("Needs key");
        profile.authentication = AiAuthentication::Bearer { credential: None };
        panel.set_profile(Some(profile.clone()), None, cx);
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        profile.max_output_tokens = Some(100);
        panel.set_profile(Some(profile), Some(Zeroizing::new("temporary".into())), cx);
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
    });
}

#[gpui_kit::test]
async fn approved_send_delivers_delayed_tokio_reply_after_gpui_waits(cx: &mut TestAppContext) {
    let listener =
        TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("loopback bind: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("loopback address: {error}"));
    let (release_reply, reply_gate) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|error| panic!("nonblocking listener: {error}"));
        let deadline = Instant::now() + Duration::from_secs(3);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("bounded loopback accept: {error}"),
            }
        };
        // Some platforms inherit nonblocking mode from the listening socket.
        // Explicit blocking mode makes the bounded read/write timeouts apply.
        stream
            .set_nonblocking(false)
            .unwrap_or_else(|error| panic!("blocking HTTP stream: {error}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("read timeout: {error}"));
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("write timeout: {error}"));
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream
                .read_exact(&mut byte)
                .unwrap_or_else(|error| panic!("read HTTP: {error}"));
            header.push(byte[0]);
            assert!(header.len() <= 4096, "request header bound");
        }
        let header = String::from_utf8_lossy(&header);
        assert!(header.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(!header.to_ascii_lowercase().contains("authorization"));
        let length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or_else(|| panic!("content length"));
        assert!(length <= 64 * 1024, "request body bound");
        let mut body = vec![0; length];
        stream
            .read_exact(&mut body)
            .unwrap_or_else(|error| panic!("read reviewed payload: {error}"));
        reply_gate
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|error| panic!("reply gate: {error}"));
        let response = r#"{"choices":[{"message":{"content":"```sh\necho reviewed\n```"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response)
            .unwrap_or_else(|error| panic!("reply HTTP: {error}"));
        String::from_utf8(body).unwrap_or_else(|error| panic!("UTF-8 payload: {error}"))
    });
    let (window, panel) = mount(cx);
    let preview = panel.update(cx, |panel, cx| {
        let mut profile = profile("Loopback");
        profile.endpoint = format!("http://{address}/v1/chat/completions");
        panel.set_profile(Some(profile), None, cx);
        panel.prepare(cx);
        let preview = panel
            .prepared
            .as_ref()
            .map(PreparedRequest::preview_json)
            .unwrap_or_else(|| panic!("reviewed request"))
            .to_owned();
        panel.send(cx);
        preview
    });
    // The server cannot reply until GPUI has suspended its completion task.
    cx.run_until_parked();
    assert!(panel.read_with(cx, |panel, _| panel.busy));
    release_reply
        .send(())
        .unwrap_or_else(|error| panic!("release reply: {error}"));
    cx.wait_for(window, Duration::from_secs(3), |_, cx| !panel.read(cx).busy)
        .await;
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.response, "```sh\necho reviewed\n```");
        assert_eq!(panel.suggestions, ["echo reviewed"]);
        assert_eq!(
            panel.response_target,
            Some(("ops@server.example:22".into(), "session-A".into()))
        );
    });
    assert_eq!(
        server.join().unwrap_or_else(|_| panic!("loopback server")),
        preview,
        "only the exact approved payload reaches the provider"
    );
}

#[test]
fn only_extracts_closed_explicit_shell_blocks() {
    assert_eq!(
        shell_blocks("```bash\necho hi\n```\n```json\n{}\n```"),
        vec!["echo hi"]
    );
    assert!(shell_blocks("```sh\nrm -rf /").is_empty());
}
#[test]
fn rejects_terminal_control_in_suggestion() {
    assert!(shell_blocks("```bash\necho \u{1b}[2J\n``` ").is_empty());
}

#[gpui_kit::test]
fn locked_reference_blocks_preview_and_key_or_reference_changes_revoke_it(cx: &mut TestAppContext) {
    use keelshell_core::AiSecretRef;
    use uuid::Uuid;
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut selected = panel
            .profile
            .clone()
            .unwrap_or_else(|| panic!("selected profile"));
        selected.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        panel.set_profile(Some(selected.clone()), None, cx);
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        assert!(!panel.busy);
        panel.set_profile(
            Some(selected.clone()),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        selected.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        panel.set_profile(Some(selected.clone()), None, cx);
        assert!(panel.prepared.is_none());
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        panel.set_profile(
            Some(selected.clone()),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        selected.endpoint = "https://other.example/v1/chat/completions".into();
        panel.set_profile(Some(selected), None, cx);
        assert!(panel.prepared.is_none());
    });
}
