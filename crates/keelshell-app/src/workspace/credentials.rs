//! Vault maintenance owns a modal lease over this workspace's saved references.

use std::collections::{BTreeMap, BTreeSet};

use keelshell_core::{AiAuthentication, AiProxy, AiSecretRef};
use uuid::Uuid;

use super::*;

pub(super) fn credential_references(state: &AppState) -> BTreeSet<Uuid> {
    let ssh = state
        .connections
        .iter()
        .chain(
            state
                .deleted_connections
                .iter()
                .map(|item| &item.connection),
        )
        .filter_map(|connection| connection.credential_ref);
    let ai = state
        .settings
        .ai_profiles
        .profiles
        .iter()
        .filter_map(|profile| match &profile.authentication {
            AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id }),
            }
            | AiAuthentication::Header {
                credential: Some(AiSecretRef::SecretStore { id }),
                ..
            } => Some(*id),
            _ => None,
        });
    let mut references: BTreeSet<_> = ssh.chain(ai).collect();
    for profile in &state.settings.ai_profiles.profiles {
        for header in &profile.custom_headers {
            if let AiSecretRef::SecretStore { id } = header.value_ref {
                references.insert(id);
            }
        }
        if let AiProxy::Explicit {
            credentials: Some(AiSecretRef::SecretStore { id }),
            ..
        } = profile.proxy
        {
            references.insert(id);
        }
    }
    references
}

impl Workspace {
    pub(super) fn can_open_vault(&self) -> bool {
        !self.saving
            && !self.show_batch
            && !self.snippet_modal_open()
            && !self.connecting
            && self.connect_route.is_none()
            && self.pending_recents.is_empty()
            && self.vault_settings.is_none()
            && self.ai_settings.is_none()
            && self.login.is_none()
            && self.host_approval.is_none()
            && self.form.is_none()
            && self.folder_form.is_none()
            && self.destination_prompt.is_none()
    }

    pub(super) fn open_vault_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_open_vault() {
            return;
        }
        let panel = cx.new(|cx| {
            VaultSettings::new(
                self.store.path().with_file_name("vault.json"),
                self.store.path().to_path_buf(),
                self.runtime.clone(),
                credential_references(&self.state),
                self.vault_profile_names(cx),
                window,
                cx,
            )
        });
        self.vault_settings_subscription =
            Some(
                cx.subscribe_in(&panel, window, |view, _, event, window, cx| match event {
                    VaultSettingsEvent::Close { message } => {
                        // The panel only emits Close once its worker has settled.
                        // Pending usage metadata merges after releasing this lease.
                        view.vault_settings = None;
                        view.vault_settings_subscription = None;
                        view.flush_recent_connections(window, cx);
                        view.flush_batch_audits(window, cx);
                        if let Some(message) = message {
                            view.status = message.clone();
                        }
                        view.focus_current_surface(window, cx);
                        cx.notify();
                    }
                }),
            );
        self.vault_settings = Some(panel);
        cx.notify();
    }

    fn vault_profile_names(&self, cx: &App) -> BTreeMap<Uuid, String> {
        self.state
            .connections
            .iter()
            .map(|connection| (connection.id, connection.name.clone()))
            .chain(self.state.deleted_connections.iter().map(|deleted| {
                (
                    deleted.connection.id,
                    format!("{} · {}", deleted.connection.name, t(cx, "回收站", "Trash")),
                )
            }))
            .chain(
                self.state
                    .settings
                    .ai_profiles
                    .profiles
                    .iter()
                    .map(|profile| (profile.id, format!("AI · {}", profile.name))),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelshell_core::{AiCustomHeader, AiPreset, NamedAiProfile};

    #[::core::prelude::v1::test]
    fn maintenance_preserves_active_trashed_and_all_ai_store_references() {
        let mut state = AppState::default();
        let mut active = Connection::new("active", "active.example", "operator");
        let active_ref = Uuid::new_v4();
        active.credential_ref = Some(active_ref);
        let mut removed = Connection::new("removed", "removed.example", "operator");
        let removed_ref = Uuid::new_v4();
        removed.credential_ref = Some(removed_ref);
        let removed_id = removed.id;
        state.connections = vec![active, removed];
        assert!(state.soft_delete_connection(removed_id, 1).is_ok());
        let ai_ref = Uuid::new_v4();
        let mut profile = NamedAiProfile::draft(AiPreset::OpenAi);
        profile.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: Some(AiSecretRef::SecretStore { id: ai_ref }),
        };
        let header_ref = Uuid::new_v4();
        profile.custom_headers.push(AiCustomHeader {
            name: "x-tenant".into(),
            value_ref: AiSecretRef::SecretStore { id: header_ref },
        });
        let proxy_ref = Uuid::new_v4();
        profile.proxy = AiProxy::Explicit {
            url: "https://proxy.example".into(),
            credentials: Some(AiSecretRef::SecretStore { id: proxy_ref }),
        };
        state.settings.ai_profiles.profiles.push(profile);
        assert_eq!(
            credential_references(&state),
            BTreeSet::from([active_ref, removed_ref, ai_ref, header_ref, proxy_ref]),
            "References used by unsupported AI transports must still be protected"
        );
    }
}
