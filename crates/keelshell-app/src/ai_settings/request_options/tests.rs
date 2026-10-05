use super::{AiSecretRef, AiSettingsPanel, SecretPurpose, Uuid};
use crate::ai_settings::tests::{fixture_profile, mount_sized};
use gpui_kit::{
    AppContext, Context, Focusable, TestAppContext, Window, point, px, test::TestWindowExt,
};
use keelshell_ai::RequestCancellation;

fn edit_header(
    panel: &mut AiSettingsPanel,
    index: usize,
    name: &str,
    value: &str,
    window: &mut Window,
    cx: &mut Context<AiSettingsPanel>,
) {
    let id = panel.selected.unwrap_or_else(|| panic!("profile"));
    let editor = panel
        .request_editors
        .get(&id)
        .unwrap_or_else(|| panic!("editor"));
    editor.headers[index]
        .name
        .update(cx, |f, cx| f.set_value(name, window, cx));
    panel.sync_editor(cx);
    panel.clear_pending_request_fields(window, cx);
    let editor = panel
        .request_editors
        .get(&id)
        .unwrap_or_else(|| panic!("editor"));
    editor.headers[index]
        .value
        .update(cx, |f, cx| f.set_value(value, window, cx));
    panel.sync_editor(cx);
}

#[gpui_kit::test]
fn header_rename_isolated_and_endpoint_queued_changes_cannot_restore_secret(
    cx: &mut TestAppContext,
) {
    let (handle, entity) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        entity.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            panel.add_request_header(window, cx);
            edit_header(panel, 0, "x-one", "synthetic-one", window, cx);
            edit_header(panel, 1, "x-two", "synthetic-two", window, cx);
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            assert_eq!(panel.credentials.all_secrets().len(), 2);
            panel.request_editors[&id].headers[0]
                .name
                .update(cx, |f, cx| f.set_value("x-renamed", window, cx));
            panel.sync_editor(cx);
            assert!(!panel.credentials.all_secrets().contains(&"synthetic-one"));
            assert!(panel.credentials.all_secrets().contains(&"synthetic-two"));
            // A second queued change before UI clearing must retain only unaffected values.
            panel
                .name
                .update(cx, |f, cx| f.set_value("Renamed profile", window, cx));
            panel.sync_editor(cx);
            assert!(panel.credentials.all_secrets().contains(&"synthetic-two"));
            panel.clear_pending_request_fields(window, cx);
            panel.endpoint.update(cx, |f, cx| {
                f.set_value("https://other.invalid/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(panel.credentials.all_secrets().is_empty());
            panel
                .model
                .update(cx, |f, cx| f.set_value("other-model", window, cx));
            panel.sync_editor(cx);
            assert!(panel.credentials.all_secrets().is_empty());
            panel.clear_pending_request_fields(window, cx);
            assert!(
                panel.request_editors[&id].headers.iter().all(|h| h
                    .value
                    .read(cx)
                    .value()
                    .is_empty())
            );
        })
    })
    .unwrap_or_else(|error| panic!("edit request headers: {error}"));
}

#[gpui_kit::test]
fn invalid_header_and_proxy_drafts_survive_profile_locale_save_ack_and_block_requests(
    cx: &mut TestAppContext,
) {
    let (handle, entity) = mount_sized(cx, fixture_profile(), 900., 580.);
    let cancellation = RequestCancellation::new();
    cx.update_window(handle, |_, window, cx| {
        entity.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            edit_header(panel, 0, "x-project", "synthetic-secret", window, cx);
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            panel.apply(cx);
            let saved = panel.revision;
            panel.cancellation = Some(cancellation.clone());
            panel.operation = Some(super::super::OperationKind::Test);
            panel.request_editors[&id].headers[0]
                .name
                .update(cx, |f, cx| f.set_value("Content-Length", window, cx));
            panel.sync_editor(cx);
            assert!(cancellation.is_cancelled());
            panel.mark_saved(saved, cx);
            assert!(!panel.saving);
            let second = fixture_profile();
            let second_id = second.id;
            panel.catalog.profiles.push(second);
            panel.select(second_id, window, cx);
            panel.select(id, window, cx);
            crate::i18n::set_language(keelshell_core::Language::En, cx);
            panel.refresh_locale(window, cx);
            for theme in [keelshell_core::Theme::Dark, keelshell_core::Theme::Light] {
                crate::design::apply(theme, Some(window), cx);
                panel.refresh_locale(window, cx);
                assert_eq!(
                    panel.request_editors[&id].headers[0].name.read(cx).value(),
                    "Content-Length"
                );
                assert!(!panel.request_draft_valid(id));
            }
            assert_eq!(
                panel.request_editors[&id].headers[0].name.read(cx).value(),
                "Content-Length"
            );
            panel.apply(cx);
            assert!(!panel.saving);
            panel.start_operation(super::super::OperationKind::Models, cx);
            assert!(panel.operation.is_none());
            edit_header(panel, 0, "x-project", "new-secret", window, cx);
            let editor = panel
                .request_editors
                .get_mut(&id)
                .unwrap_or_else(|| panic!("editor"));
            editor.proxy = true;
            editor.proxy_url.update(cx, |f, cx| {
                f.set_value("http://user:password@localhost:8888", window, cx)
            });
            panel.sync_editor(cx);
            assert!(!panel.request_draft_valid(id));
            panel.apply(cx);
            assert!(!panel.saving);
            panel.select(second_id, window, cx);
            panel.select(id, window, cx);
            assert_eq!(
                panel.request_editors[&id].proxy_url.read(cx).value(),
                "http://user:password@localhost:8888"
            );
        })
    })
    .unwrap_or_else(|error| panic!("preserve invalid request drafts: {error}"));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let footer = window.find("ai-settings-apply").bounds();
        assert!(footer.bottom() <= px(580.));
        assert!(footer.size.height >= px(28.));
    })
    .unwrap_or_else(|error| panic!("render footer after edits: {error}"));
}

#[gpui_kit::test]
fn editing_unlocked_value_detaches_store_reference_and_proxy_route_clears_only_proxy(
    cx: &mut TestAppContext,
) {
    let (handle, entity) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| entity.update(cx, |panel, cx| {
        panel.add_request_header(window, cx); edit_header(panel, 0, "x-project", "synthetic-secret", window, cx);
        let purpose = SecretPurpose::Header("x-project".into()); let store = Uuid::new_v4();
        panel.apply_request_vault_result(&purpose, Some(store), None, window, cx);
        let id = panel.selected.unwrap_or_else(|| panic!("profile"));
        assert!(matches!(&panel.profile().unwrap_or_else(|| panic!("profile")).custom_headers[0].value_ref, AiSecretRef::SecretStore { id } if *id == store));
        panel.request_editors[&id].headers[0].value.update(cx, |f, cx| f.set_value("replacement-secret", window, cx)); panel.sync_editor(cx); panel.clear_pending_request_fields(window, cx);
        assert!(matches!(panel.profile().unwrap_or_else(|| panic!("profile")).custom_headers[0].value_ref, AiSecretRef::Ephemeral { .. }));
        assert!(panel.credentials.all_secrets().contains(&"replacement-secret"));
        let editor = panel.request_editors.get_mut(&id).unwrap_or_else(|| panic!("editor")); editor.proxy = true; editor.proxy_auth = true; editor.proxy_url.update(cx, |f, cx| f.set_value("http://localhost:8888", window, cx)); panel.sync_editor(cx); panel.clear_pending_request_fields(window, cx);
        panel.request_editors[&id].username.update(cx, |f, cx| f.set_value("synthetic-user", window, cx)); panel.request_editors[&id].password.update(cx, |f, cx| f.set_value("synthetic-password", window, cx)); panel.sync_editor(cx);
        assert!(panel.credentials.all_secrets().contains(&"synthetic-password"));
        panel.request_editors[&id].proxy_url.update(cx, |f, cx| f.set_value("socks5://localhost:1080", window, cx)); panel.sync_editor(cx);
        assert!(!panel.credentials.all_secrets().contains(&"synthetic-password")); assert!(panel.credentials.all_secrets().contains(&"replacement-secret"));
        panel.clear_pending_request_fields(window, cx);
    })).unwrap_or_else(|error| panic!("request reference changes: {error}"));
}

#[gpui_kit::test]
fn request_options_environment_and_vault_references_detach_on_destination_or_purpose_change(
    cx: &mut TestAppContext,
) {
    let mut profile = fixture_profile();
    profile.custom_headers.push(keelshell_core::AiCustomHeader {
        name: "x-project".into(),
        value_ref: AiSecretRef::Environment {
            name: "FIXTURE_HEADER".into(),
        },
    });
    profile.proxy = keelshell_core::AiProxy::Explicit {
        url: "http://localhost:8888".into(),
        credentials: Some(AiSecretRef::Environment {
            name: "FIXTURE_PROXY".into(),
        }),
    };
    let (handle, panel) = mount_sized(cx, profile, 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let id = panel.selected.unwrap_or_else(|| panic!("profile"));
            panel.request_editors[&id].proxy_url.update(cx, |f, cx| {
                f.set_value("socks5://localhost:1080", window, cx)
            });
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            assert!(matches!(
                &panel.profile().unwrap_or_else(|| panic!("profile")).proxy,
                keelshell_core::AiProxy::Explicit {
                    credentials: Some(AiSecretRef::Ephemeral { .. }),
                    ..
                }
            ));
            assert!(matches!(
                &panel
                    .profile()
                    .unwrap_or_else(|| panic!("profile"))
                    .custom_headers[0]
                    .value_ref,
                AiSecretRef::Environment { .. }
            ));
            panel.endpoint.update(cx, |f, cx| {
                f.set_value("https://other.invalid/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            assert!(matches!(
                &panel
                    .profile()
                    .unwrap_or_else(|| panic!("profile"))
                    .custom_headers[0]
                    .value_ref,
                AiSecretRef::Ephemeral { .. }
            ));
            panel.apply_request_vault_result(
                &SecretPurpose::Header("x-project".into()),
                Some(Uuid::new_v4()),
                None,
                window,
                cx,
            );
            panel.request_editors[&id].headers[0]
                .name
                .update(cx, |f, cx| f.set_value("x-other", window, cx));
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            assert!(matches!(
                &panel
                    .profile()
                    .unwrap_or_else(|| panic!("profile"))
                    .custom_headers[0]
                    .value_ref,
                AiSecretRef::Ephemeral { .. }
            ));
            assert!(panel.credentials.all_secrets().is_empty());
        })
    })
    .unwrap_or_else(|error| panic!("detach old destination references: {error}"));
}

#[gpui_kit::test]
fn request_options_vault_prompts_work_with_no_api_auth_and_fixed_footer(cx: &mut TestAppContext) {
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.add_request_header(window, cx);
            edit_header(panel, 0, "x-project", "synthetic-header", window, cx);
        });
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        assert!(window.find("ai-request-save-header-x-project").visible());
        window.click("ai-request-save-header-x-project", cx);
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        for id in [
            "ai-vault-master",
            "ai-vault-confirmation",
            "ai-vault-submit",
            "ai-vault-cancel",
            "ai-settings-apply",
        ] {
            assert!(window.find(id).visible(), "header vault control {id}");
        }
        window.click("ai-vault-master", cx);
        window.input("synthetic-master", cx);
        window.click("ai-vault-cancel", cx);
        assert!(panel.read(cx).vault_prompt.is_none());
        let profile_id = panel.read(cx).selected.unwrap_or_else(|| panic!("profile"));
        assert!(
            panel.read(cx).request_editors[&profile_id].headers[0]
                .value
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        panel.update(cx, |panel, cx| {
            let editor = panel
                .request_editors
                .get_mut(&profile_id)
                .unwrap_or_else(|| panic!("editor"));
            editor.proxy = true;
            editor.proxy_auth = true;
            editor.proxy_url.update(cx, |input, cx| {
                input.set_value("http://localhost:8888", window, cx)
            });
            panel.sync_editor(cx);
            panel.clear_pending_request_fields(window, cx);
            panel.request_editors[&profile_id]
                .username
                .update(cx, |input, cx| {
                    input.set_value("synthetic-user", window, cx)
                });
            panel.request_editors[&profile_id]
                .password
                .update(cx, |input, cx| {
                    input.set_value("synthetic-password", window, cx)
                });
            panel.sync_editor(cx);
        });
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.click("ai-request-save-proxy", cx);
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        for id in [
            "ai-vault-master",
            "ai-vault-confirmation",
            "ai-vault-submit",
            "ai-vault-cancel",
            "ai-settings-apply",
        ] {
            assert!(window.find(id).visible(), "proxy vault control {id}");
        }
        window.click("ai-vault-cancel", cx);
        assert!(panel.read(cx).vault_prompt.is_none());
        assert!(
            panel.read(cx).request_editors[&profile_id]
                .password
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(window.find("ai-settings-apply").bounds().bottom() <= px(580.));
    })
    .unwrap_or_else(|error| panic!("request vault UI: {error}"));
}
