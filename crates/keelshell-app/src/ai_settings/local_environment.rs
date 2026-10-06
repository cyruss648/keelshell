//! Explicit local-key import reuses the catalog's secret-reference metadata.

use gpui_kit::{
    component::{Disableable, Selectable, button::Button, input::Input},
    *,
};
use keelshell_ai::AiError;
use keelshell_core::{AiAuthentication, AiBackend, AiSecretRef, NamedAiProfile};
use zeroize::Zeroizing;

use super::AiSettingsPanel;
use crate::i18n::{Message, t};

pub(super) fn reference_name(profile: Option<&NamedAiProfile>) -> Option<&str> {
    let profile = profile?;
    if profile.backend == AiBackend::Api {
        return None;
    }
    match &profile.authentication {
        AiAuthentication::Bearer {
            credential: Some(AiSecretRef::Environment { name }),
        }
        | AiAuthentication::Header {
            credential: Some(AiSecretRef::Environment { name }),
            ..
        } => Some(name),
        _ => None,
    }
}

fn set_reference(profile: &mut NamedAiProfile, value: Option<AiSecretRef>) {
    match &mut profile.authentication {
        AiAuthentication::Bearer { credential } | AiAuthentication::Header { credential, .. } => {
            *credential = value
        }
        AiAuthentication::None => {}
    }
}

impl AiSettingsPanel {
    pub(super) fn load_local_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = reference_name(self.profile())
            .map(str::to_owned)
            .or_else(|| {
                self.selected
                    .and_then(|id| self.local_environment_drafts.get(&id).cloned())
            })
            .unwrap_or_default();
        self.local_environment
            .update(cx, |field, cx| field.set_value(name, window, cx));
    }

    pub(super) fn sync_local_environment(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.profile() else {
            return;
        };
        if profile.backend == AiBackend::Api {
            return;
        }
        let id = profile.id;
        let value = self.local_environment.read(cx).value().to_string();
        let changed = reference_name(Some(profile)).is_some_and(|name| name != value);
        self.local_environment_drafts.insert(id, value.clone());
        if changed {
            if let Some(profile) = self
                .catalog
                .profiles
                .iter_mut()
                .find(|profile| profile.id == id)
            {
                // Keep invalid text intact; core validation blocks Apply/probe.
                set_reference(profile, Some(AiSecretRef::Environment { name: value }));
            }
            self.credentials.remove(&id);
            self.clear_key_pending = true;
            self.editor_values.key.clear();
            self.changed(true, cx);
        }
    }

    pub(super) fn set_local_environment(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_editor(cx);
        let Some(profile) = self.profile() else {
            return;
        };
        if profile.backend == AiBackend::Api || reference_name(Some(profile)).is_some() == enabled {
            return;
        }
        let id = profile.id;
        let name = self
            .local_environment_drafts
            .get(&id)
            .cloned()
            .unwrap_or_default();
        if let Some(profile) = self
            .catalog
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
        {
            set_reference(
                profile,
                enabled.then_some(AiSecretRef::Environment { name }),
            );
        }
        self.credentials.remove(&id);
        self.changed(true, cx);
        self.load_editor(window, cx);
    }

    pub(super) fn read_local_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.read_local_environment_with(crate::ai_request_options::environment, window, cx);
    }

    fn read_local_environment_with(
        &mut self,
        mut lookup: impl FnMut(&str) -> Result<Zeroizing<String>, AiError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile().cloned() else {
            return;
        };
        let Some(name) = reference_name(Some(&profile)) else {
            return;
        };
        // Re-reading revokes the old delivery value even when the new lookup
        // fails. Previously observed values remain disclosure guards only.
        self.credentials.remove(&profile.id);
        self.key
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.editor_values.key.clear();
        // The explicit clear is already complete. A queued source/name Change
        // must not clear the newly imported value after this handler returns.
        self.clear_key_pending = false;
        self.changed(true, cx);
        let result = (|| {
            profile
                .validate_local_agent_transport()
                .map_err(|_| AiError::InvalidRequestOptions)?;
            if !self.credentials.local_environment_capacity(profile.id) {
                return Err(AiError::ContextTooLarge);
            }
            let value = lookup(name)?;
            if value.is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
                return Err(AiError::InvalidApiKey);
            }
            self.credentials
                .retain_local_environment(profile.id, value.clone());
            crate::ai_request_options::validate_catalog_metadata(&self.catalog, &self.credentials)?;
            self.credentials
                .bind_local_environment(&profile, value.clone());
            self.key.update(cx, |field, cx| {
                field.set_value(value.to_string(), window, cx)
            });
            self.editor_values.key = value;
            Ok(())
        })();
        self.status = if result.is_ok() {
            Message::new(
                "已显式读取到临时密钥；值不显示、不保存。应用后仍需预览并确认发送；环境改变不会自动刷新。",
                "Explicitly loaded a temporary key; its value is hidden and not persisted. Apply, then review and confirm sending. Environment changes do not refresh it automatically.",
            )
        } else {
            Message::new(
                "未读取密钥：请检查变量名、应用进程环境、值的有效范围及配置中的已知秘密。旧发送值已撤销。",
                "Key was not loaded. Check the variable name, the application process environment, value bounds and known secrets in configuration. The previous delivery value was revoked.",
            )
        };
        cx.notify();
    }

    pub(super) fn local_credential_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let environment = reference_name(self.profile()).is_some();
        let mut view = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t(cx, "API 密钥来源", "API key source"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("ai-local-key-explicit")
                            .label(t(cx, "输入 / 加密凭据库", "Enter / encrypted vault"))
                            .selected(!environment)
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.set_local_environment(false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("ai-local-key-environment")
                            .label(t(cx, "环境变量引用", "Environment reference"))
                            .selected(environment)
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.set_local_environment(true, window, cx)
                            })),
                    ),
            );
        if environment {
            let loaded = self
                .profile()
                .is_some_and(|profile| self.credentials.local_environment_key(profile).is_some());
            view = view.child(t(
                cx,
                if loaded {
                    "临时密钥已加载"
                } else {
                    "尚未加载临时密钥"
                },
                if loaded {
                    "Temporary key loaded"
                } else {
                    "Temporary key not loaded"
                },
            ));
            view = view.child(Input::new(&self.local_environment).id("ai-local-key-environment-name")
                    .aria_label(t(cx, "API 密钥环境变量名", "API key environment variable name")))
                .child(Button::new("ai-local-key-environment-read").label(t(cx, "读取到临时密钥", "Load temporary key"))
                    .disabled(self.saving || self.vault_prompt.is_some())
                    .on_click(cx.listener(|panel, _, window, cx| panel.read_local_environment(window, cx))))
                .child(div().min_w_0().whitespace_normal().child(t(cx, "只保存变量名，值只在点击读取时从应用进程环境获取；启动、检查 CLI 和发送均不自动读取。不会转交其他环境项。修改来源、变量名、地址或执行文件后需重新读取。", "Only the variable name is saved. Clicking Load reads the application process environment. Startup, CLI checks and sending never read it automatically. No other environment entries are forwarded. Reload after changing the source, name, endpoint or executable.")));
        } else {
            view = view
                .child(Input::new(&self.key).id("ai-profile-key"))
                .child(self.vault_controls(cx));
        }
        view.into_any_element()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod independent_tests;

#[cfg(test)]
mod probe_admission_tests;

#[cfg(test)]
mod reviewer_v2_tests;
