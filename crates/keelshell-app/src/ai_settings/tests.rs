use super::{AiSettingsPanel, EphemeralCredentials, OperationKind};
use crate::i18n::{Message, set_language};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Focusable, TestAppContext, WindowBounds,
    WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_ai::{AiError, RequestCancellation};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiPreset, AiProfileCatalog, Language, NamedAiProfile,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) fn fixture_profile() -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "运维模型".into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "fixture-model".into();
    profile.authentication = AiAuthentication::None;
    profile
}

#[gpui_kit::test]
fn token_edits_cancel_requests_preserve_invalid_drafts_and_pending_save_revision(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount(cx, fixture_profile());
    let cancellation = RequestCancellation::new();
    panel.update(cx, |panel, _| {
        panel.cancellation = Some(cancellation.clone());
        panel.operation = Some(OperationKind::Models);
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel
                .context_tokens
                .update(cx, |field, cx| field.set_value("32768", window, cx));
            panel
                .output_tokens
                .update(cx, |field, cx| field.set_value("8192", window, cx));
            panel.sync_editor(cx);
            assert!(cancellation.is_cancelled());
            assert_eq!(
                panel.profile().map(|p| p.max_output_tokens),
                Some(Some(8192))
            );
            assert_eq!(
                panel.profile().map(|p| p.context_window_tokens),
                Some(Some(32768))
            );
            panel.apply(cx);
            let saved_revision = panel.revision;
            panel
                .output_tokens
                .update(cx, |field, cx| field.set_value("invalid", window, cx));
            panel.sync_editor(cx);
            panel.mark_saved(saved_revision, cx);
            assert!(!panel.saving);
            assert!(panel.revision > saved_revision);
            panel.load_editor(window, cx);
            assert_eq!(panel.output_tokens.read(cx).value(), "invalid");
            let original = panel.selected.unwrap_or_else(|| panic!("original profile"));
            let second = fixture_profile();
            let second_id = second.id;
            panel.catalog.profiles.push(second);
            panel.select(second_id, window, cx);
            panel.select(original, window, cx);
            assert_eq!(panel.output_tokens.read(cx).value(), "invalid");
            set_language(Language::En, cx);
            panel.refresh_locale(window, cx);
            assert_eq!(panel.output_tokens.read(cx).value(), "invalid");
            panel.apply(cx);
            assert!(!panel.saving);
            panel.start_operation(OperationKind::Test, cx);
            assert!(panel.operation.is_none());
        });
    })
    .unwrap_or_else(|error| panic!("token editor: {error}"));
}

#[gpui_kit::test]
fn native_token_input_event_updates_metadata_and_cancels_inflight_operation(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount(cx, fixture_profile());
    let cancellation = RequestCancellation::new();
    panel.update(cx, |panel, _| {
        panel.cancellation = Some(cancellation.clone());
        panel.operation = Some(OperationKind::Test);
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        panel
            .read(cx)
            .output_tokens
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        window.input("512", cx);
    })
    .unwrap_or_else(|error| panic!("native token edit: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.profile().map(|p| p.max_output_tokens),
            Some(Some(512))
        );
        assert!(cancellation.is_cancelled());
        assert!(panel.operation.is_none());
    });
}

pub(super) fn mount(
    cx: &mut TestAppContext,
    profile: NamedAiProfile,
) -> (AnyWindowHandle, Entity<AiSettingsPanel>) {
    cx.update(gpui_kit::init);
    let catalog = AiProfileCatalog {
        active_id: Some(profile.id),
        profiles: vec![profile],
    };
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("runtime: {error}")),
    );
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1000.), px(900.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    AiSettingsPanel::new(
                        &catalog,
                        &EphemeralCredentials::new(),
                        runtime,
                        std::env::temp_dir()
                            .join(format!("keelshell-ai-settings-{}", uuid::Uuid::new_v4()))
                            .join("vault.json"),
                        window,
                        cx,
                    )
                })
            },
        )
        .unwrap_or_else(|error| panic!("settings window: {error}"))
    })
}

fn configure_http_stream(stream: &TcpStream) {
    // Some platforms inherit nonblocking mode on accept. SO_RCVTIMEO cannot make a
    // nonblocking socket wait for request bytes that have not arrived yet.
    stream
        .set_nonblocking(false)
        .unwrap_or_else(|error| panic!("blocking HTTP stream: {error}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap_or_else(|error| panic!("read timeout: {error}"));
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap_or_else(|error| panic!("write timeout: {error}"));
}

#[test]
fn accepted_http_fixture_socket_waits_for_its_read_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("loopback listener: {error}"));
    let client = TcpStream::connect(
        listener
            .local_addr()
            .unwrap_or_else(|error| panic!("loopback address: {error}")),
    )
    .unwrap_or_else(|error| panic!("loopback connect: {error}"));
    let (mut stream, _) = listener
        .accept()
        .unwrap_or_else(|error| panic!("loopback accept: {error}"));
    // Force the inherited nonblocking flag so every OS guards this
    // fixture contract. The connected client deliberately sends no bytes.
    stream
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("simulate inherited flag: {error}"));
    configure_http_stream(&stream);
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap_or_else(|error| panic!("short test deadline: {error}"));
    let started = Instant::now();
    let Err(error) = stream.read(&mut [0]) else {
        panic!("idle peer must time out without producing bytes");
    };
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    assert!(
        started.elapsed() >= Duration::from_millis(50),
        "read returned before its deadline; the fixture socket is nonblocking"
    );
    drop(client);
}

#[gpui_kit::test]
fn named_drafts_survive_selection_default_change_and_delete(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        let first = panel
            .read(cx)
            .selected
            .unwrap_or_else(|| panic!("initial profile"));
        panel.update(cx, |panel, cx| {
            panel.create(window, cx);
            panel
                .name
                .update(cx, |field, cx| field.set_value("备用模型", window, cx));
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("http://127.0.0.1:9901/v1/chat/completions", window, cx)
            });
            panel
                .model
                .update(cx, |field, cx| field.set_value("backup-model", window, cx));
            panel.set_authentication(false, window, cx);
            panel.make_default(cx);
            let second = panel.selected.unwrap_or_else(|| panic!("second profile"));
            assert_eq!(panel.catalog.active_id, Some(second));
            panel.select(first, window, cx);
            assert_eq!(panel.model.read(cx).value(), "fixture-model");
            panel.select(second, window, cx);
            assert_eq!(panel.name.read(cx).value(), "备用模型");
            assert_eq!(panel.model.read(cx).value(), "backup-model");
            panel.remove(window, cx);
            assert_eq!(panel.catalog.profiles.len(), 1);
            assert_eq!(panel.catalog.active_id, None);
            assert_eq!(panel.selected, Some(first));
        });
    })
    .unwrap_or_else(|error| panic!("edit drafts: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.catalog.profiles[0].name, "运维模型");
        assert_eq!(panel.catalog.profiles[0].model, "fixture-model");
    });
}

#[gpui_kit::test]
fn pending_save_preserves_new_native_edits_and_failure_keeps_draft(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    let revision = panel.update(cx, |panel, cx| {
        panel.apply(cx);
        assert!(panel.saving);
        panel.revision
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("ai-profile-name", cx);
        window.input(" revised", cx);
    })
    .unwrap_or_else(|error| panic!("edit during save: {error}"));
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        panel.mark_saved(revision, cx);
        assert!(!panel.saving);
        assert!(panel.revision > revision);
        assert!(panel.name.read(cx).value().contains("revised"));
        assert!(panel.status.render(cx).contains("新修改"));
        panel.set_saving(true, cx);
        panel.report_failure(Message::new("磁盘写入失败", "Disk write failed"), cx);
        assert!(!panel.saving);
        assert!(panel.name.read(cx).value().contains("revised"));
        assert_eq!(panel.status.render(cx), "磁盘写入失败");
    });
}

#[gpui_kit::test]
fn preset_changes_clear_credentials_and_locale_keeps_values(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.key.update(cx, |key, cx| {
                key.set_value("fixture-private-key", window, cx)
            });
            panel.sync_editor(cx);
            assert_eq!(panel.credentials.len(), 1);
            panel.use_preset(AiPreset::Ollama, window, cx);
            assert!(panel.credentials.is_empty());
            assert!(panel.key.read(cx).value().is_empty());
            assert_eq!(
                panel.profile().map(|p| &p.authentication),
                Some(&AiAuthentication::None)
            );
            let before = panel.read_values(cx);
            let revision = panel.revision;
            set_language(Language::En, cx);
            panel.refresh_locale(window, cx);
            assert!(panel.read_values(cx) == before);
            assert_eq!(panel.revision, revision);
        });
    })
    .unwrap_or_else(|error| panic!("presets and locale: {error}"));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn protocol_switch_updates_only_supported_suffix_and_clears_key(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.key.update(cx, |key, cx| {
                key.set_value("fixture-private-key", window, cx)
            });
            panel.sync_editor(cx);
            panel.set_api_style(AiApiStyle::Responses, window, cx);
            assert_eq!(
                panel.profile().map(|profile| profile.api_style),
                Some(AiApiStyle::Responses)
            );
            assert_eq!(
                panel.profile().map(|profile| profile.endpoint.as_str()),
                Some("https://provider.example/v1/responses")
            );
            assert!(panel.credentials.is_empty());
            assert!(panel.key.read(cx).value().is_empty());
            panel.set_api_style(AiApiStyle::ChatCompletions, window, cx);
            assert_eq!(
                panel.profile().map(|profile| profile.endpoint.as_str()),
                Some("https://provider.example/v1/chat/completions")
            );
            panel.set_api_style(AiApiStyle::AnthropicMessages, window, cx);
            assert_eq!(
                panel.profile().map(|profile| profile.endpoint.as_str()),
                Some("https://provider.example/v1/messages")
            );
            assert_eq!(
                panel.profile().map(|profile| &profile.authentication),
                Some(&AiAuthentication::Header {
                    name: "x-api-key".into(),
                    credential: None,
                })
            );
            panel.set_api_style(AiApiStyle::ChatCompletions, window, cx);
            assert!(matches!(
                panel.profile().map(|profile| &profile.authentication),
                Some(AiAuthentication::Bearer { credential: None })
            ));
        });
    })
    .unwrap_or_else(|error| panic!("switch protocol: {error}"));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn changing_form_cancels_operation_and_ignores_late_failure(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let cancellation = RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.operation = Some(OperationKind::Test);
            let old = panel.operation_revision;
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("https://other.example/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.operation.is_none());
            let status = panel.status.clone();
            panel.finish_operation(old, Err(AiError::HttpStatus(401)), cx);
            assert_eq!(panel.status, status);
        });
    })
    .unwrap_or_else(|error| panic!("cancel operation: {error}"));
}

#[gpui_kit::test]
fn invalid_profile_is_not_applied_and_bearer_without_key_is_not_requested(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_authentication(true, window, cx);
            panel.start_operation(OperationKind::Test, cx);
            assert!(panel.operation.is_none());
            assert!(panel.cancellation.is_none());
            panel
                .name
                .update(cx, |field, cx| field.set_value("", window, cx));
            panel.apply(cx);
            assert!(!panel.saving);
        })
    })
    .unwrap_or_else(|error| panic!("invalid profile: {error}"));
}

#[gpui_kit::test]
async fn manual_discovery_uses_real_loopback_http_and_selection_updates_only_model(
    cx: &mut TestAppContext,
) {
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
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("bounded loopback accept: {error}"),
            }
        };
        configure_http_stream(&stream);
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream
                .read_exact(&mut byte)
                .unwrap_or_else(|error| panic!("read HTTP: {error}"));
            bytes.push(byte[0]);
            assert!(bytes.len() <= 4096, "request header bound");
        }
        let request = String::from_utf8_lossy(&bytes);
        assert!(request.starts_with("GET /v1/models HTTP/1.1"));
        assert!(!request.to_ascii_lowercase().contains("authorization"));
        reply_gate
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|error| panic!("reply gate: {error}"));
        let body = r#"{"data":[{"id":"model-B"},{"id":"model-A"}]}"#;
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap_or_else(|error|panic!("reply HTTP: {error}"));
    });
    let mut profile = fixture_profile();
    profile.endpoint = format!("http://{address}/v1/chat/completions");
    let (window, panel) = mount(cx, profile);
    panel.update(cx, |panel, cx| {
        panel.start_operation(OperationKind::Models, cx)
    });
    // Force GPUI to suspend before the real Tokio worker can complete. Without
    // this gate, a fast loopback reply can hide an illegal foreign-thread wake.
    cx.run_until_parked();
    assert!(panel.read_with(cx, |panel, _| panel.operation.is_some()));
    release_reply
        .send(())
        .unwrap_or_else(|error| panic!("release reply: {error}"));
    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        panel.read(cx).operation.is_none()
    })
    .await;
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.models, ["model-A", "model-B"]);
        assert_eq!(
            panel.model.read(cx).value(),
            "fixture-model",
            "discovery does not choose a model automatically"
        );
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(("ai-discovered-model", 0_usize))
                .bounds()
                .size
                .height
                >= px(28.),
            "adding advanced fields must not collapse discovered-model hit areas"
        );
        window.click(("ai-discovered-model", 0_usize), cx);
    })
    .unwrap_or_else(|error| panic!("choose discovered model: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.model.read(cx).value(), "model-A")
    });
    server
        .join()
        .unwrap_or_else(|_| panic!("loopback server failed"));
}

#[gpui_kit::test]
fn error_categories_are_localized_without_exposing_provider_details(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx, fixture_profile());
    panel.update(cx, |panel, cx| {
        panel.finish_operation(panel.operation_revision, Err(AiError::HttpStatus(401)), cx);
        assert_eq!(panel.status.render(cx), "认证失败（未自动重试）");
        set_language(Language::En, cx);
        assert_eq!(
            panel.status.render(cx),
            "Authentication (not retried automatically)"
        );
        panel.finish_operation(panel.operation_revision, Err(AiError::Timeout), cx);
        assert_eq!(
            panel.status.render(cx),
            "Timeout (not retried automatically)"
        );
    });
}

#[gpui_kit::test]
fn opening_and_emptying_settings_keep_keyboard_focus_inside_the_modal(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            panel
                .read(cx)
                .name
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        panel.update(cx, |panel, cx| panel.remove(window, cx));
        window.render_frame(cx);
        assert!(panel.read(cx).focus.is_focused(window));
        panel.update(cx, |panel, cx| panel.create(window, cx));
        window.render_frame(cx);
        assert!(
            panel
                .read(cx)
                .name
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .unwrap_or_else(|error| panic!("modal keyboard focus: {error}"));
}
