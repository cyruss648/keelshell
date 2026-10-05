use super::{AiSettingsPanel, RequestSecret, SecretPurpose, Source, Uuid};
use crate::ai_settings::tests::{fixture_profile, mount_sized, request_has_finished};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Context, ScrollDelta, TestAppContext, Window, point};
use keelshell_ai::{AiError, ContextDraft, ProviderConfig, RequestCancellation};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const FIRST: &str = "retained-inactive-first-secret";
const SECOND: &str = "retained-inactive-second-secret";

#[gpui_kit::test]
fn incomplete_and_oversize_proxy_raw_drafts_stay_known_and_basic_tracks_current_pair(
    cx: &mut TestAppContext,
) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            let editor = panel
                .request_editors
                .get_mut(&id)
                .unwrap_or_else(|| panic!("editor"));
            editor.proxy = true;
            editor.proxy_auth = true;
            editor.proxy_url.update(cx, |field, cx| {
                field.set_value("http://127.0.0.1:9", window, cx)
            });
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            panel.request_editors[&id].username.update(cx, |field, cx| {
                field.set_value("retained-proxy-user", window, cx)
            });
            panel.sync_editor(cx);
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"retained-proxy-user")
            );
            assert!(
                crate::ai_request_options::resolve_options(
                    panel.profile().unwrap_or_else(|| panic!("profile")),
                    &panel.credentials
                )
                .is_err()
            );
            let long_password = "p".repeat(256);
            panel.request_editors[&id]
                .password
                .update(cx, |field, cx| field.set_value(&long_password, window, cx));
            panel.sync_editor(cx);
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&long_password.as_str())
            );
            assert!(!panel.request_draft_valid(id));
            let active = fixture_profile();
            let options = crate::ai_request_options::resolve_options(&active, &panel.credentials)
                .unwrap_or_else(|error| panic!("other direct profile: {error}"));
            assert!(options.proxy_url().is_none());
            assert!(
                !options
                    .redact_for_review(&long_password)
                    .contains(&long_password)
            );
            panel.request_editors[&id].password.update(cx, |field, cx| {
                field.set_value("retained-proxy-password", window, cx)
            });
            panel.sync_editor(cx);
            let basic = STANDARD.encode("retained-proxy-user:retained-proxy-password");
            assert!(
                !panel
                    .credentials
                    .all_secrets()
                    .contains(&long_password.as_str())
            );
            assert!(panel.credentials.all_secrets().contains(&basic.as_str()));
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&format!("Basic {basic}").as_str())
            );
            assert!(panel.request_draft_valid(id));
            panel.clear_request_secret(&SecretPurpose::Proxy, window, cx);
            assert!(panel.credentials.is_empty());
            assert!(
                panel.request_editors[&id]
                    .username
                    .read(cx)
                    .value()
                    .is_empty()
            );
            assert!(
                panel.request_editors[&id]
                    .password
                    .read(cx)
                    .value()
                    .is_empty()
            );
        })
    })
    .unwrap_or_else(|error| panic!("proxy draft known-value lifecycle: {error}"));
}

fn put_value(
    panel: &mut AiSettingsPanel,
    index: usize,
    value: &str,
    window: &mut Window,
    cx: &mut Context<AiSettingsPanel>,
) {
    let id = panel.selected.unwrap_or_else(|| panic!("selected profile"));
    panel.request_editors[&id].headers[index]
        .value
        .update(cx, |field, cx| field.set_value(value, window, cx));
    panel.sync_editor(cx);
}

fn duplicate_headers(
    panel: &mut AiSettingsPanel,
    window: &mut Window,
    cx: &mut Context<AiSettingsPanel>,
) {
    for (index, value) in [FIRST, SECOND].into_iter().enumerate() {
        panel.add_request_header(window, cx);
        let id = panel.selected.unwrap_or_else(|| panic!("selected profile"));
        panel.request_editors[&id].headers[index]
            .name
            .update(cx, |field, cx| {
                field.set_value(
                    if index == 0 {
                        "x-duplicate"
                    } else {
                        "X-DUPLICATE"
                    },
                    window,
                    cx,
                )
            });
        panel.sync_editor(cx);
        panel.clear_pending_request_fields(window, cx);
        put_value(panel, index, value, window, cx);
    }
}

#[gpui_kit::test]
fn invalid_blank_duplicate_and_reference_drafts_guard_context_without_delivery(
    cx: &mut TestAppContext,
) {
    for case in 0..4 {
        let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
        cx.update_window(handle, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.add_request_header(window, cx);
                let id = panel.selected.unwrap_or_else(|| panic!("selected profile"));
                if case != 0 {
                    panel.request_editors[&id].headers[0]
                        .name
                        .update(cx, |field, cx| {
                            field.set_value(
                                if case == 1 {
                                    "Content-Length"
                                } else {
                                    "x-duplicate"
                                },
                                window,
                                cx,
                            )
                        });
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                }
                put_value(panel, 0, FIRST, window, cx);
                if case == 2 {
                    panel.add_request_header(window, cx);
                    panel.request_editors[&id].headers[1]
                        .name
                        .update(cx, |field, cx| field.set_value("x-duplicate", window, cx));
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                    put_value(panel, 1, SECOND, window, cx);
                }
                if case == 3 {
                    panel.add_request_header(window, cx);
                    panel
                        .request_editors
                        .get_mut(&id)
                        .unwrap_or_else(|| panic!("editor"))
                        .headers[1]
                        .source = Source::Vault;
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                    panel.request_editors[&id].headers[1]
                        .reference
                        .update(cx, |field, cx| {
                            field.set_value("invalid-vault-id", window, cx)
                        });
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                }
                assert!(!panel.request_draft_valid(id), "invalid draft case {case}");
                assert_eq!(
                    panel.request_editors[&id].headers[0].value.read(cx).value(),
                    FIRST
                );
                assert!(
                    panel.request_editors[&id].headers[0]
                        .value
                        .read(cx)
                        .presentation()
                        .is_masked()
                );
                assert!(panel.credentials.all_secrets().contains(&FIRST));
                let active = fixture_profile();
                let options =
                    crate::ai_request_options::resolve_options(&active, &panel.credentials)
                        .unwrap_or_else(|error| panic!("active options: {error}"));
                assert!(options.header_names().next().is_none());
                let provider = ProviderConfig::new(&active.endpoint, &active.model)
                    .unwrap_or_else(|error| panic!("provider: {error}"))
                    .with_request_options(options);
                let review = ContextDraft::new(format!("Explain {FIRST} and {SECOND}"))
                    .prepare(&provider, &[], 4096)
                    .unwrap_or_else(|error| panic!("review: {error}"));
                assert!(!review.preview_json().contains(FIRST));
                if case == 2 {
                    assert!(!review.preview_json().contains(SECOND));
                }
                let metadata = serde_json::to_string(&panel.catalog)
                    .unwrap_or_else(|error| panic!("metadata: {error}"));
                assert!(!metadata.contains(FIRST));
            })
        })
        .unwrap_or_else(|error| panic!("invalid draft case {case}: {error}"));
    }
}

#[gpui_kit::test]
fn oversize_retained_input_stays_masked_and_other_profile_fails_closed(cx: &mut TestAppContext) {
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    let oversized = "s".repeat(1024 * 1024 + 1);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            put_value(panel, 0, &oversized, window, cx);
            let inactive = panel.selected.unwrap_or_else(|| panic!("inactive profile"));
            let active = fixture_profile();
            let active_id = active.id;
            panel.catalog.profiles.push(active);
            panel.select(active_id, window, cx);
            panel.start_operation(super::super::OperationKind::Models, cx);
            assert!(panel.operation.is_none());
            assert!(panel._job.is_none());
            assert!(matches!(
                crate::ai_request_options::resolve_options(
                    panel.profile().unwrap_or_else(|| panic!("active profile")),
                    &panel.credentials
                ),
                Err(AiError::ContextTooLarge)
            ));
            panel.select(inactive, window, cx);
            assert_eq!(
                panel.request_editors[&inactive].headers[0]
                    .value
                    .read(cx)
                    .value()
                    .len(),
                oversized.len()
            );
            assert_eq!(
                panel.request_editors[&inactive].headers[0]
                    .value
                    .read(cx)
                    .value()
                    .as_ref(),
                oversized.as_str()
            );
            assert!(
                panel.request_editors[&inactive].headers[0]
                    .value
                    .read(cx)
                    .presentation()
                    .is_masked()
            );
        })
    })
    .unwrap_or_else(|error| panic!("oversize retained input: {error}"));
}

#[gpui_kit::test]
fn duplicate_draft_edits_prune_models_cancel_and_discard_late_callback(cx: &mut TestAppContext) {
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            duplicate_headers(panel, window, cx);
            panel.models = vec!["ordinary-model".into(), "newly-known-draft".into()];
            let cancellation = RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.operation = Some(super::super::OperationKind::Models);
            let old_revision = panel.operation_revision;
            put_value(panel, 0, "newly-known-draft", window, cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.operation.is_none());
            assert_eq!(panel.models, ["ordinary-model"]);
            assert!(!panel.credentials.all_secrets().contains(&FIRST));
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"newly-known-draft")
            );
            let status = panel.status.render(cx);
            panel.finish_operation(old_revision, Err(AiError::Transport), cx);
            assert_eq!(panel.models, ["ordinary-model"]);
            assert_eq!(panel.status.render(cx), status);
            panel.cancel_operation(true, cx);
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"newly-known-draft")
            );
        })
    })
    .unwrap_or_else(|error| panic!("draft lifecycle: {error}"));
}

#[gpui_kit::test]
fn removing_clearing_renaming_and_destination_changes_release_retained_values(
    cx: &mut TestAppContext,
) {
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| duplicate_headers(panel, window, cx));
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.click(("ai-header-remove", 1_usize), cx);
        panel.read_with(cx, |panel, _| {
            assert!(panel.credentials.all_secrets().contains(&FIRST));
            assert!(!panel.credentials.all_secrets().contains(&SECOND));
        });
        panel.update(cx, |panel, cx| {
            panel.clear_request_secret(&SecretPurpose::Header("x-duplicate".into()), window, cx);
            assert!(panel.credentials.is_empty());
            put_value(panel, 0, FIRST, window, cx);
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            panel.request_editors[&id].headers[0]
                .name
                .update(cx, |field, cx| field.set_value("x-renamed", window, cx));
            panel.sync_editor(cx);
            assert!(panel.credentials.is_empty());
            panel.clear_pending_request_fields(window, cx);
            put_value(panel, 0, FIRST, window, cx);
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("https://other.invalid/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(panel.credentials.is_empty());
            panel
                .model
                .update(cx, |field, cx| field.set_value("changed-model", window, cx));
            panel.sync_editor(cx);
            assert!(panel.credentials.is_empty());
            panel.clear_pending_request_fields(window, cx);
            put_value(panel, 0, FIRST, window, cx);
            panel.remove(window, cx);
            assert!(panel.credentials.is_empty());
            assert!(panel.request_editors.is_empty());
        });
    })
    .unwrap_or_else(|error| panic!("draft release: {error}"));
}

#[gpui_kit::test]
fn vault_backfill_replaces_draft_guard_and_preserves_same_destination_delivery(
    cx: &mut TestAppContext,
) {
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            panel.request_editors[&id].headers[0]
                .name
                .update(cx, |field, cx| field.set_value("x-project", window, cx));
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            put_value(panel, 0, FIRST, window, cx);
            panel.apply_request_vault_result(
                &SecretPurpose::Header("x-project".into()),
                Some(Uuid::new_v4()),
                Some(RequestSecret::Header(Zeroizing::new(SECOND.into()))),
                window,
                cx,
            );
            assert!(!panel.credentials.all_secrets().contains(&FIRST));
            assert!(panel.credentials.all_secrets().contains(&SECOND));
            let profile = panel.profile().unwrap_or_else(|| panic!("profile")).clone();
            let options = crate::ai_request_options::resolve_options(&profile, &panel.credentials)
                .unwrap_or_else(|error| panic!("same target delivery: {error}"));
            assert_eq!(options.header_names().collect::<Vec<_>>(), ["x-project"]);
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("https://other.invalid/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(panel.credentials.is_empty());
            let changed = panel.profile().unwrap_or_else(|| panic!("profile"));
            assert!(
                crate::ai_request_options::resolve_options(changed, &panel.credentials).is_err()
            );
            panel.name.update(cx, |field, cx| {
                field.set_value("Queued unrelated edit", window, cx)
            });
            panel.sync_editor(cx);
            assert!(panel.credentials.is_empty());
        })
    })
    .unwrap_or_else(|error| panic!("vault draft binding: {error}"));
}

#[gpui_kit::test]
async fn retained_duplicate_draft_rejects_catalog_and_manual_model_before_next_http(
    cx: &mut TestAppContext,
) {
    let listener =
        TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("owned listener: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("listener mode: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned address: {error}"));
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("bounded owned accept: {error}"),
            }
        };
        stream
            .set_nonblocking(false)
            .unwrap_or_else(|error| panic!("accepted stream: {error}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("read limit: {error}"));
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("write limit: {error}"));
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream
                .read_exact(&mut byte)
                .unwrap_or_else(|error| panic!("owned GET: {error}"));
            bytes.push(byte[0]);
            assert!(bytes.len() <= 16384);
        }
        let request = String::from_utf8(bytes).unwrap_or_else(|error| panic!("GET UTF8: {error}"));
        assert!(request.starts_with("GET /v1/models HTTP/1.1\r\n"));
        assert!(!request.contains(FIRST));
        assert!(!request.contains(SECOND));
        let reply = serde_json::json!({"data":[{"id": FIRST}]}).to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap_or_else(|error| panic!("owned response: {error}"));
        listener
    });
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            duplicate_headers(panel, window, cx);
            let mut active = fixture_profile();
            active.endpoint = format!("http://{address}/v1/chat/completions");
            let active_id = active.id;
            panel.catalog.profiles.push(active);
            panel.select(active_id, window, cx);
            panel.start_operation(super::super::OperationKind::Models, cx);
        })
    })
    .unwrap_or_else(|error| panic!("start discovery: {error}"));
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
        request_has_finished(&panel, cx)
    })
    .await;
    panel.read_with(cx, |panel, cx| {
        assert!(panel.models.is_empty());
        assert!(!panel.status.render(cx).contains(FIRST));
    });
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel
                .model
                .update(cx, |field, cx| field.set_value(FIRST, window, cx));
            panel.sync_editor(cx);
            panel.start_operation(super::super::OperationKind::Test, cx);
        })
    })
    .unwrap_or_else(|error| panic!("manual model input: {error}"));
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
        request_has_finished(&panel, cx)
    })
    .await;
    let listener = server
        .join()
        .unwrap_or_else(|_| panic!("owned server joined"));
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "one GET and no subsequent HTTP connection"
    );
    panel.read_with(cx, |panel, cx| {
        assert!(!panel.status.render(cx).contains(FIRST))
    });
}

// Preserve the independent reviewer regression byte for byte.
include!("private_header_name_gpui.rs");

#[gpui_kit::test]
fn known_metadata_blocks_apply_events_and_valid_replacement_saves_without_secret_text(
    cx: &mut TestAppContext,
) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let events = Arc::new(AtomicUsize::new(0));
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    panel.update(cx, |panel_state, cx| {
        let events = events.clone();
        panel_state
            ._subscriptions
            .push(cx.subscribe(&panel, move |_, _, event, _| {
                if let crate::ai_settings::AiSettingsEvent::Apply {
                    catalog,
                    credentials,
                    ..
                } = event
                {
                    assert!(credentials.all_secrets().contains(&FIRST));
                    let metadata = serde_json::to_string(catalog)
                        .unwrap_or_else(|error| panic!("fixture metadata: {error}"));
                    assert!(!metadata.contains(FIRST));
                    events.fetch_add(1, Ordering::SeqCst);
                }
            }));
    });
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            let owner = panel.selected.unwrap_or_else(|| panic!("owner"));
            panel.request_editors[&owner].headers[0]
                .name
                .update(cx, |field, cx| field.set_value("x-owned", window, cx));
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            put_value(panel, 0, FIRST, window, cx);
            let mut active = fixture_profile();
            active.name = "Independent active profile".into();
            active.custom_headers.push(keelshell_core::AiCustomHeader {
                name: FIRST.to_ascii_uppercase(),
                value_ref: keelshell_core::AiSecretRef::Ephemeral { id: Uuid::new_v4() },
            });
            let active_id = active.id;
            panel.catalog.profiles.push(active);
            panel.select(active_id, window, cx);
            put_value(panel, 0, SECOND, window, cx);
            assert!(panel.request_draft_valid(owner) && panel.request_draft_valid(active_id));
            panel.apply(cx);
            assert!(!panel.saving);
            assert!(!panel.status.render(cx).contains(FIRST));
            assert!(!panel.status.render(cx).contains(SECOND));
            assert_eq!(events.load(Ordering::SeqCst), 0);
            assert_eq!(
                panel.request_editors[&owner].headers[0]
                    .value
                    .read(cx)
                    .value(),
                FIRST
            );
            panel.request_editors[&active_id].headers[0]
                .name
                .update(cx, |field, cx| {
                    field.set_value("X-Approved-Safe", window, cx)
                });
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            put_value(panel, 0, SECOND, window, cx);
            panel.apply(cx);
            assert!(panel.saving);
            let revision = panel.revision;
            // A new masked value arriving during save stays known and cannot be
            // erased by the acknowledgement for the earlier safe snapshot.
            put_value(panel, 0, "newer-pending-secret", window, cx);
            panel.mark_saved(revision, cx);
            assert!(
                panel
                    .credentials
                    .all_secrets()
                    .contains(&"newer-pending-secret")
            );
            assert_eq!(
                panel.request_editors[&active_id].headers[0]
                    .value
                    .read(cx)
                    .value(),
                "newer-pending-secret"
            );
            assert!(!panel.saving);
        });
    })
    .unwrap_or_else(|error| panic!("known metadata apply: {error}"));
    cx.run_until_parked();
    assert_eq!(events.load(Ordering::SeqCst), 1);
}
