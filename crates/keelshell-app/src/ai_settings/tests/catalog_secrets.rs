use super::*;
use crate::ai_request_options::{RequestSecret, SecretPurpose};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use keelshell_core::AiProxy;
use zeroize::Zeroizing;

#[gpui_kit::test]
async fn manual_inactive_basic_model_and_encoded_destination_cannot_start_http(
    cx: &mut TestAppContext,
) {
    let bare = STANDARD.encode("catalog-inactive-user:catalog-inactive-pass");
    let listener =
        TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("owned listener: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("listener mode: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned address: {error}"));
    for (style, suffix) in [
        (AiApiStyle::ChatCompletions, "chat/completions"),
        (AiApiStyle::Responses, "responses"),
        (AiApiStyle::AnthropicMessages, "messages"),
    ] {
        let mut profile = fixture_profile();
        profile.api_style = style;
        profile.endpoint = format!("http://{address}/v1/{suffix}");
        let (handle, panel) = mount_sized(cx, profile, 900., 580.);
        panel.update(cx, |panel, _| {
            let mut inactive = fixture_profile();
            let reference = AiSecretRef::Ephemeral {
                id: uuid::Uuid::new_v4(),
            };
            inactive.proxy = AiProxy::Explicit {
                url: "http://127.0.0.1:9".into(),
                credentials: Some(reference.clone()),
            };
            panel.catalog.profiles.push(inactive.clone());
            panel.credentials.insert_request(
                &inactive,
                SecretPurpose::Proxy,
                reference,
                RequestSecret::Proxy {
                    username: Zeroizing::new("catalog-inactive-user".into()),
                    password: Zeroizing::new("catalog-inactive-pass".into()),
                },
            );
            assert!(panel.credentials.all_secrets().contains(&bare.as_str()));
        });
        cx.update_window(handle, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel
                    .model
                    .update(cx, |field, cx| field.set_value(bare.clone(), window, cx));
                panel.sync_editor(cx);
                panel.start_operation(OperationKind::Test, cx);
            });
        })
        .unwrap_or_else(|error| panic!("production model input: {error}"));
        cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
            request_has_finished(&panel, cx)
        })
        .await;
        panel.read_with(cx, |panel, cx| {
            assert!(panel.models.is_empty());
            assert!(!panel.status.render(cx).contains(&bare));
        });
        let encoded: String = bare.bytes().map(|byte| format!("%{byte:02X}")).collect();
        cx.update_window(handle, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.model.update(cx, |field, cx| {
                    field.set_value("ordinary-model", window, cx)
                });
                panel.endpoint.update(cx, |field, cx| {
                    field.set_value(
                        format!("http://{address}/{encoded}/v1/{suffix}"),
                        window,
                        cx,
                    )
                });
                panel.sync_editor(cx);
                panel.start_operation(OperationKind::Models, cx);
            });
        })
        .unwrap_or_else(|error| panic!("production endpoint input: {error}"));
        cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
            request_has_finished(&panel, cx)
        })
        .await;
        panel.read_with(cx, |panel, cx| {
            assert!(panel.models.is_empty());
            assert!(!panel.status.render(cx).contains(&bare));
        });
    }
    assert!(
        matches!(listener.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock),
        "all protocol production InputState operations must make zero HTTP requests"
    );
}

#[gpui_kit::test]
fn inactive_secret_change_prunes_catalog_and_rejects_previous_operation_callback(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx, fixture_profile());
    let bare = STANDARD.encode("catalog-new-user:catalog-new-pass");
    let cancellation = RequestCancellation::new();
    panel.update(cx, |panel, cx| {
        panel.models = vec!["ordinary-model".into(), bare.clone()];
        panel.cancellation = Some(cancellation.clone());
        panel.operation = Some(OperationKind::Models);
        let previous = panel.operation_revision;
        let mut inactive = fixture_profile();
        let reference = AiSecretRef::Ephemeral {
            id: uuid::Uuid::new_v4(),
        };
        inactive.proxy = AiProxy::Explicit {
            url: "http://127.0.0.1:9".into(),
            credentials: Some(reference.clone()),
        };
        panel.catalog.profiles.push(inactive.clone());
        panel.credentials.insert_request(
            &inactive,
            SecretPurpose::Proxy,
            reference,
            RequestSecret::Proxy {
                username: Zeroizing::new("catalog-new-user".into()),
                password: Zeroizing::new("catalog-new-pass".into()),
            },
        );
        panel.changed(false, cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.operation_revision > previous);
        assert!(panel.operation.is_none());
        assert_eq!(panel.models, ["ordinary-model"]);
        let status = panel.status.render(cx);
        panel.finish_operation(previous, Err(AiError::Transport), cx);
        assert_eq!(panel.status.render(cx), status);
        assert_eq!(panel.models, ["ordinary-model"]);
    });
}
