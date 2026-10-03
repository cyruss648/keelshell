//! The maintenance modal owns the state lane until its background work settles.

use super::*;
use keelshell_core::{AiAuthentication, AiPreset, AiProfileCatalog, AiSecretRef, NamedAiProfile};
use zeroize::Zeroizing;

#[gpui_kit::test]
fn vault_modal_isolates_configuration_and_restores_connection_focus(cx: &mut TestAppContext) {
    let connection = Connection::new("fixture", "fixture.example", "operator");
    let fixture = mount(cx, vec![connection]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.open_connections(&super::super::OpenConnections, window, cx);
            workspace.open_vault_settings(window, cx);
            assert!(workspace.vault_settings.is_some());
            let mut candidate = workspace.state.clone();
            candidate.connections.clear();
            workspace.persist(candidate, super::super::AfterSave::None, window, cx);
            assert!(!workspace.saving);
            assert_eq!(workspace.state.connections.len(), 1);
            workspace.open_ai_settings(window, cx);
            assert!(workspace.ai_settings.is_none());
            workspace.open_connections(&super::super::OpenConnections, window, cx);
            assert!(
                !workspace
                    .search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            workspace.close_tab(&super::super::CloseTab, window, cx);
        });
    })
    .checked("open maintenance and prevent stale reference changes");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.vault_settings.is_none());
        assert!(workspace.show_connections);
        assert!(
            workspace
                .search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("restore underlying manager keyboard focus");
}

#[gpui_kit::test]
fn vault_admission_waits_for_saving_connection_and_other_drafts(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.saving = true;
            workspace.open_vault_settings(window, cx);
            assert!(workspace.vault_settings.is_none());
            workspace.saving = false;
            workspace.connecting = true;
            workspace.open_vault_settings(window, cx);
            assert!(workspace.vault_settings.is_none());
            workspace.connecting = false;
            let profile = Connection::new("fixture", "fixture.example", "operator");
            let state = keelshell_core::AppState {
                connections: vec![profile.clone()],
                ..Default::default()
            };
            let route = state.connection_route(profile.id).checked("fixture route");
            workspace.pending_recents.push((profile, route, 1));
            workspace.open_vault_settings(window, cx);
            assert!(workspace.vault_settings.is_none());
            workspace.pending_recents.clear();
            workspace.open_ai_settings(window, cx);
            workspace.open_vault_settings(window, cx);
            assert!(workspace.vault_settings.is_none());
        });
    })
    .checked("refuse opening over in-flight reference changes");
}

#[gpui_kit::test]
async fn ai_key_draft_cancel_preserves_workspace_and_apply_unlinks_it(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "fixture provider".into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "fixture-model".into();
    profile.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::SecretStore {
            id: uuid::Uuid::new_v4(),
        }),
    };
    let id = profile.id;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let mut state = workspace.state.clone();
            state.settings.ai_profiles = AiProfileCatalog {
                active_id: Some(id),
                profiles: vec![profile.clone()],
            };
            workspace.state = fixture.store.save(&state).checked("seed AI metadata");
            workspace
                .ai_credentials
                .insert(id, Zeroizing::new("fixture-api-key".into()));
            workspace.assistant.update(cx, |assistant, cx| {
                assistant.set_profiles(
                    &workspace.state.settings.ai_profiles,
                    &workspace.ai_credentials,
                    cx,
                );
            });
            workspace.open_ai_settings(window, cx);
        });
        window.render_frame(cx);
        window.click("ai-key-lock", cx);
        window.render_frame(cx);
        window.click("ai-settings-cancel", cx);
    })
    .checked("clear only the draft key and cancel settings");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            assert!(workspace.ai_settings.is_none());
            assert_eq!(
                workspace.ai_credentials.get(&id).map(|key| key.as_str()),
                Some("fixture-api-key")
            );
            assert_eq!(workspace.state.settings.ai_profiles.profiles[0], profile);
            workspace.open_ai_settings(window, cx);
        });
        window.render_frame(cx);
        window.click("ai-key-unlink", cx);
        window.render_frame(cx);
        window.click("ai-settings-apply", cx);
    })
    .checked("apply unlink through real settings buttons");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        !workspace.saving && workspace.ai_settings.is_none()
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(!workspace.ai_credentials.contains_key(&id));
        assert_eq!(
            workspace.state.settings.ai_profiles.profiles[0].authentication,
            AiAuthentication::Bearer { credential: None }
        );
    });
    assert_eq!(
        fixture
            .store
            .load()
            .checked("reload applied AI metadata")
            .settings
            .ai_profiles
            .profiles[0]
            .authentication,
        AiAuthentication::Bearer { credential: None }
    );
}

#[gpui_kit::test]
fn vault_background_close_keeps_its_final_outcome_visible(cx: &mut TestAppContext) {
    use crate::{i18n::Message, vault_settings::VaultSettingsEvent};
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.open_vault_settings(window, cx);
            let panel = workspace
                .vault_settings
                .clone()
                .unwrap_or_else(|| panic!("maintenance panel missing"));
            panel.update(cx, |_, cx| {
                cx.emit(VaultSettingsEvent::Close {
                    message: Some(Message::new(
                        "主密码已更新，请使用新主密码。",
                        "Master password changed; use the new password.",
                    )),
                });
            });
        });
    })
    .checked("deliver the admitted operation outcome at deferred close");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.vault_settings.is_none());
        assert_eq!(
            workspace.status.render(cx),
            "主密码已更新，请使用新主密码。"
        );
    });
}
