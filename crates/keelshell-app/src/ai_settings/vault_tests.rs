use super::super::tests::{fixture_profile, mount};
use super::{
    AiAuthentication, AiSecretRef, Completion, Ordering, Uuid, VaultAction, Zeroizing,
    ai_credentials,
};
use gpui_kit::{AppContext, Focusable, TestAppContext, test::TestAppContextExt};
use std::time::Duration;

#[gpui_kit::test]
fn endpoint_edits_clear_bound_key_and_queued_events_cannot_reinsert_it(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_authentication(true, window, cx);
            panel
                .key
                .update(cx, |field, cx| field.set_value("private-key", window, cx));
            panel.sync_editor(cx);
            let id = panel
                .selected
                .unwrap_or_else(|| panic!("required test value"));
            panel.catalog.profiles[0].authentication = AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
            };
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("https://other.example/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(!panel.credentials.contains_key(&id));
            assert!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .is_none()
            );
            panel
                .name
                .update(cx, |field, cx| field.set_value("Renamed", window, cx));
            panel.sync_editor(cx);
            assert!(
                !panel.credentials.contains_key(&id),
                "pending old input must not repopulate the new destination"
            );
            panel.clear_pending_key(window, cx);
            assert!(panel.key.read(cx).value().is_empty());
        })
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, _| assert!(panel.credentials.is_empty()));
}

#[gpui_kit::test]
fn auth_change_clears_key_and_reference_while_name_and_model_keep_them(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_authentication(true, window, cx);
            panel
                .key
                .update(cx, |field, cx| field.set_value("private-key", window, cx));
            panel.sync_editor(cx);
            let reference = Uuid::new_v4();
            panel.catalog.profiles[0].authentication = AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id: reference }),
            };
            panel
                .name
                .update(cx, |field, cx| field.set_value("Renamed", window, cx));
            panel
                .model
                .update(cx, |field, cx| field.set_value("other-model", window, cx));
            panel.sync_editor(cx);
            assert_eq!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                ),
                Some(reference)
            );
            assert_eq!(panel.credentials.len(), 1);
            panel.set_authentication(false, window, cx);
            assert!(panel.credentials.is_empty());
            assert!(panel.key.read(cx).value().is_empty());
            assert!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .is_none()
            );
        })
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
}

#[gpui_kit::test]
async fn explicit_save_and_unlock_run_asynchronously_without_network_or_automatic_apply(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount(cx, fixture_profile());
    let path = panel.read_with(cx, |panel, _| panel.vault_path.clone());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_authentication(true, window, cx);
            panel.key.update(cx, |field, cx| {
                field.set_value("fixture-private-key", window, cx)
            });
            panel.sync_editor(cx);
            panel.begin_vault(VaultAction::Save, window, cx);
            let prompt = panel
                .vault_prompt
                .as_ref()
                .unwrap_or_else(|| panic!("required test value"));
            let revision = panel.revision;
            for field in [&prompt.master, &prompt.confirmation] {
                field.update(cx, |field, cx| {
                    field.set_value("fixture master", window, cx)
                });
            }
            assert_eq!(
                panel.revision, revision,
                "master fields never become metadata edits"
            );
            panel.submit_vault(window, cx);
            assert!(panel.vault_busy());
            assert!(
                panel
                    .vault_prompt
                    .as_ref()
                    .unwrap_or_else(|| panic!("required test value"))
                    .master
                    .read(cx)
                    .value()
                    .is_empty()
            );
            panel.apply(cx);
            assert!(
                !panel.saving,
                "Apply is blocked while a vault write is pending"
            );
        })
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
    cx.run_until_parked();
    cx.wait_for(window, Duration::from_secs(20), |_, cx| {
        !panel.read(cx).vault_busy()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            assert!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .is_some(),
                "save status: {}",
                panel.status.render(cx)
            );
            assert!(panel.operation.is_none());
            assert!(!panel.saving, "encrypted save does not persist the draft");
            panel.unlink_or_lock(false, window, cx);
            assert!(panel.credentials.is_empty());
            assert!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .is_some()
            );
            panel.start_operation(super::super::OperationKind::Test, cx);
            assert!(
                panel.operation.is_none(),
                "locked metadata cannot authorize a request"
            );
            panel.begin_vault(VaultAction::Unlock, window, cx);
            panel
                .vault_prompt
                .as_ref()
                .unwrap_or_else(|| panic!("required test value"))
                .master
                .update(cx, |field, cx| field.set_value("wrong master", window, cx));
            panel.submit_vault(window, cx);
        })
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
    cx.run_until_parked();
    cx.wait_for(window, Duration::from_secs(20), |_, cx| {
        !panel.read(cx).vault_busy()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            assert!(panel.credentials.is_empty());
            assert!(panel.status.render(cx).contains("主密码错误"));
            assert!(panel.key.read(cx).focus_handle(cx).is_focused(window));
            assert!(panel.vault_prompt.is_none());
            panel.begin_vault(VaultAction::Unlock, window, cx);
            let master = &panel
                .vault_prompt
                .as_ref()
                .unwrap_or_else(|| panic!("required test value"))
                .master;
            assert!(master.read(cx).focus_handle(cx).is_focused(window));
            master.update(cx, |field, cx| {
                field.set_value("fixture master", window, cx)
            });
            panel.submit_vault(window, cx);
        });
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
    cx.run_until_parked();
    cx.wait_for(window, Duration::from_secs(20), |_, cx| {
        !panel.read(cx).vault_busy()
    })
    .await;
    panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .credentials
                .get(
                    &panel
                        .selected
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .unwrap_or_else(|| panic!("required test value"))
                .as_str(),
            "fixture-private-key"
        );
        assert!(panel.operation.is_none());
        assert!(!panel.saving);
        assert!(panel.vault_prompt.is_none());
        assert_eq!(panel.key.read(cx).value(), "fixture-private-key");
    });
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read fixture vault: {error}"));
    assert!(!data.contains("fixture-private-key"));
    assert!(!data.contains("fixture master"));
    std::fs::remove_dir_all(path.parent().unwrap_or_else(|| panic!("fixture directory")))
        .unwrap_or_else(|error| panic!("remove fixture: {error}"));
}

#[gpui_kit::test]
fn changed_or_cancelled_draft_ignores_late_unlocked_key(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_authentication(true, window, cx);
            panel.catalog.profiles[0].authentication = AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
            };
            panel.begin_vault(VaultAction::Unlock, window, cx);
            let prompt = panel
                .vault_prompt
                .as_ref()
                .unwrap_or_else(|| panic!("required test value"));
            let id = prompt.id;
            let cancelled = prompt.cancelled.clone();
            let revision = panel.revision;
            let profile = panel
                .profile()
                .unwrap_or_else(|| panic!("required test value"))
                .clone();
            panel.endpoint.update(cx, |field, cx| {
                field.set_value("https://other.example/v1/chat/completions", window, cx)
            });
            panel.sync_editor(cx);
            assert!(cancelled.load(Ordering::Acquire));
            panel.finish_vault(
                id,
                revision,
                &profile,
                Ok(Completion::Unlocked(Zeroizing::new("late-key".into()))),
                window,
                cx,
            );
            assert!(panel.credentials.is_empty());
            assert!(
                ai_credentials::reference(
                    panel
                        .profile()
                        .unwrap_or_else(|| panic!("required test value"))
                )
                .is_none()
            );
            panel.catalog.profiles[0].authentication = AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
            };
            panel.clear_pending_key(window, cx);
            panel.begin_vault(VaultAction::Unlock, window, cx);
            let id = panel
                .vault_prompt
                .as_ref()
                .unwrap_or_else(|| panic!("required test value"))
                .id;
            let revision = panel.revision;
            let profile = panel
                .profile()
                .unwrap_or_else(|| panic!("required test value"))
                .clone();
            panel.close(cx);
            panel.finish_vault(
                id,
                revision,
                &profile,
                Ok(Completion::Saved(Uuid::new_v4())),
                window,
                cx,
            );
            assert_eq!(panel.profile(), Some(&profile));
            assert!(panel.credentials.is_empty());
        })
    })
    .unwrap_or_else(|error| panic!("update test window: {error}"));
}
