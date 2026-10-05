//! Test the real Apply subscription and StateStore, including a direct event bypass.

use super::{Checked, mount};
use crate::{ai_request_options::EphemeralCredentials, ai_settings::AiSettingsEvent};
use gpui_kit::{AppContext, TestAppContext, test::TestAppContextExt};
use keelshell_core::{
    AiAuthentication, AiCustomHeader, AiPreset, AiProfileCatalog, AiProxy, AiSecretRef,
    NamedAiProfile,
};
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

fn fixture_profile() -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "Owned persistence profile".into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "ordinary-model".into();
    profile.authentication = AiAuthentication::None;
    profile
}

#[gpui_kit::test]
async fn metadata_apply_event_cannot_write_known_secrets_and_safe_snapshot_persists(
    cx: &mut TestAppContext,
) {
    const SECRET: &str = "owned-persistence-secret";
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.open_ai_settings(window, cx));
    })
    .checked("open real AI settings subscription");
    let panel = fixture
        .workspace
        .read_with(cx, |view, _| view.ai_settings.clone())
        .unwrap_or_else(|| panic!("settings panel"));
    let before = std::fs::read(fixture.store.path()).checked("read original owned state");
    let mut credentials = EphemeralCredentials::new();
    credentials.retain_request_drafts(Uuid::new_v4(), vec![Zeroizing::new(SECRET.into())]);
    for field in 0..4 {
        let mut profile = fixture_profile();
        match field {
            0 => profile.custom_headers.push(AiCustomHeader {
                name: SECRET.to_ascii_uppercase(),
                value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
            }),
            1 => profile.model = SECRET.into(),
            2 => profile.endpoint = format!("https://provider.example/{SECRET}/chat/completions"),
            _ => {
                profile.proxy = AiProxy::Explicit {
                    url: format!("http://{SECRET}.example"),
                    credentials: None,
                }
            }
        }
        let catalog = AiProfileCatalog {
            active_id: Some(profile.id),
            profiles: vec![profile],
        };
        assert!(catalog.validate().is_ok());
        // A consumer must guard the immutable event too, even if a future
        // producer accidentally omits the panel's validation.
        panel.update(cx, |_, cx| {
            cx.emit(AiSettingsEvent::Apply {
                catalog,
                credentials: credentials.clone(),
                revision: field,
            })
        });
        cx.run_until_parked();
        fixture.workspace.read_with(cx, |view, cx| {
            assert!(!view.saving);
            assert!(view.state.settings.ai_profiles.profiles.is_empty());
            assert!(!view.status.render(cx).contains(SECRET));
        });
        assert_eq!(
            std::fs::read(fixture.store.path()).checked("guarded state bytes"),
            before
        );
    }
    let mut profile = fixture_profile();
    profile.custom_headers.push(AiCustomHeader {
        name: "X-Approved-Safe".into(),
        value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
    });
    let catalog = AiProfileCatalog {
        active_id: Some(profile.id),
        profiles: vec![profile],
    };
    let saved_catalog = catalog.clone();
    panel.update(cx, |_, cx| {
        cx.emit(AiSettingsEvent::Apply {
            catalog,
            credentials,
            revision: 0,
        })
    });
    cx.run_until_parked();
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let saved = fixture
        .store
        .load()
        .checked("load actual saved safe metadata");
    assert_eq!(saved.settings.ai_profiles, saved_catalog);
    let bytes = std::fs::read(fixture.store.path()).checked("safe state bytes");
    assert!(
        !String::from_utf8(bytes)
            .checked("safe state JSON")
            .contains(SECRET)
    );
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.ai_credentials.all_secrets().contains(&SECRET))
    });
}
