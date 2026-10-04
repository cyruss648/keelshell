use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    *,
};
use keelshell_core::{AiAuthentication, AiSecretRef, Error, NamedAiProfile};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::AiSettingsPanel;
use crate::{
    ai_credentials::{self, Completion, VaultAction, uses_api_key_authentication},
    i18n::{Message, t},
};

pub(super) struct VaultPrompt {
    id: Uuid,
    action: VaultAction,
    master: Entity<InputState>,
    confirmation: Entity<InputState>,
    busy: bool,
    cancelled: Arc<AtomicBool>,
}

impl Drop for VaultPrompt {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl AiSettingsPanel {
    pub(super) fn vault_busy(&self) -> bool {
        self.vault_prompt.as_ref().is_some_and(|prompt| prompt.busy)
    }

    pub(super) fn cancel_vault(&mut self) {
        self.vault_prompt = None;
    }

    pub(super) fn begin_vault(
        &mut self,
        action: VaultAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile() else {
            return;
        };
        if !uses_api_key_authentication(&profile.authentication) {
            return;
        }
        if let Err(error) = super::validate_selected_transport(profile) {
            self.status =
                Message::detail("请先补全配置", "Complete this configuration first", error);
            cx.notify();
            return;
        }
        if action == VaultAction::Save
            && self
                .credentials
                .get(&profile.id)
                .is_none_or(|key| key.is_empty())
        {
            self.status = Message::new(
                "请先填写要保存的 API 密钥。",
                "Enter the API key to save first.",
            );
            cx.notify();
            return;
        }
        if action == VaultAction::Unlock && ai_credentials::reference(profile).is_none() {
            return;
        }
        self.cancel_operation(false, cx);
        let master = cx.new(|cx| InputState::new(window, cx).masked(true));
        let confirmation = cx.new(|cx| InputState::new(window, cx).masked(true));
        master.read(cx).focus_handle(cx).focus(window, cx);
        self.vault_prompt = Some(VaultPrompt {
            id: Uuid::new_v4(),
            action,
            master,
            confirmation,
            busy: false,
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        self.status = Message::empty();
        cx.notify();
    }

    fn dismiss_vault(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_vault();
        self.key.read(cx).focus_handle(cx).focus(window, cx);
        self.status = Message::new(
            "已取消凭据操作；已开始写入的密文可能保留，配置未应用。",
            "Credential operation cancelled; an admitted encrypted write may remain. Configuration was not applied.",
        );
        cx.notify();
    }

    pub(super) fn unlink_or_lock(
        &mut self,
        unlink: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            self.credentials.remove(&profile.id);
            if unlink && uses_api_key_authentication(&profile.authentication) {
                let header = matches!(&profile.authentication, AiAuthentication::Header { .. });
                profile.authentication = if header {
                    AiAuthentication::Header {
                        name: "x-api-key".into(),
                        credential: None,
                    }
                } else {
                    AiAuthentication::Bearer { credential: None }
                };
            }
            self.changed(false, cx);
            self.load_editor(window, cx);
            self.status = if unlink {
                Message::new(
                    "已解除草稿关联；应用后对助手生效。加密条目保留，可在凭据维护中清理。",
                    "Draft unlinked; apply to update the assistant. Ciphertext remains for vault maintenance.",
                )
            } else {
                Message::new(
                    "已清除草稿中的临时密钥；应用后对助手生效。",
                    "Draft key cleared; apply to update the assistant.",
                )
            };
            self.key.read(cx).focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    fn submit_vault(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile().cloned() else {
            return;
        };
        let Some(prompt) = &mut self.vault_prompt else {
            return;
        };
        let master = Zeroizing::new(prompt.master.read(cx).value().to_string());
        if master.is_empty() || master.len() > 4096 {
            self.status = Message::new(
                "请输入主密码（最多 4096 字节）。",
                "Enter a master password (at most 4096 bytes).",
            );
            cx.notify();
            return;
        }
        if prompt.action == VaultAction::Save {
            let confirmation = Zeroizing::new(prompt.confirmation.read(cx).value().to_string());
            if *master != *confirmation {
                self.status = Message::new(
                    "两次输入的主密码不一致。",
                    "The master passwords do not match.",
                );
                cx.notify();
                return;
            }
        }
        for field in [&prompt.master, &prompt.confirmation] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        prompt.busy = true;
        let prompt_id = prompt.id;
        let action = prompt.action;
        let cancelled = prompt.cancelled.clone();
        let revision = self.revision;
        let key = self
            .credentials
            .get(&profile.id)
            .cloned()
            .unwrap_or_default();
        let path = self.vault_path.clone();
        let operation_profile = profile.clone();
        self.status = Message::new("正在处理加密凭据…", "Processing encrypted credential…");
        // Tokio's blocking pool cannot stall a GPUI foreground task. Closing
        // the draft cancels admission/result delivery, not an in-progress KDF.
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                tokio::task::spawn_blocking(move || {
                    ai_credentials::operate(
                        path,
                        &operation_profile,
                        action,
                        master,
                        key,
                        &cancelled,
                    )
                })
                .await
                .unwrap_or(Err(Error::VaultCrypto))
            },
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = job.await.unwrap_or(Err(Error::VaultCrypto));
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.finish_vault(prompt_id, revision, &profile, result, window, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_vault(
        &mut self,
        id: Uuid,
        revision: u64,
        profile: &NamedAiProfile,
        result: Result<Completion, Error>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .vault_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.id == id && !prompt.cancelled.load(Ordering::Acquire))
        {
            return;
        }
        if revision != self.revision || self.profile() != Some(profile) {
            self.cancel_vault();
            self.status = Message::new(
                "配置已修改，已忽略旧凭据结果。",
                "Configuration changed; the previous credential result was ignored.",
            );
            self.key.read(cx).focus_handle(cx).focus(window, cx);
            cx.notify();
            return;
        }
        self.cancel_vault();
        match result {
            Ok(Completion::Saved(reference)) => {
                if let Some(profile) = self
                    .catalog
                    .profiles
                    .iter_mut()
                    .find(|p| p.id == profile.id)
                {
                    let header = matches!(&profile.authentication, AiAuthentication::Header { .. });
                    profile.authentication = if header {
                        AiAuthentication::Header {
                            name: "x-api-key".into(),
                            credential: Some(AiSecretRef::SecretStore { id: reference }),
                        }
                    } else {
                        AiAuthentication::Bearer {
                            credential: Some(AiSecretRef::SecretStore { id: reference }),
                        }
                    };
                }
                self.changed(false, cx);
                self.status = Message::new(
                    "密钥已加密保存；点击应用保存关联。取消将仅留下可清理的密文。",
                    "Key encrypted. Apply to save its reference; cancelling leaves only cleanable ciphertext.",
                );
            }
            Ok(Completion::Unlocked(key)) => {
                self.credentials.insert(profile.id, key);
                self.changed(false, cx);
                self.load_editor(window, cx);
                self.status = Message::new(
                    "已为本次运行解锁；点击应用后助手可用。未发起网络请求。",
                    "Unlocked for this process. Apply to use in the assistant. No network request was made.",
                );
            }
            Ok(Completion::Cancelled) => {}
            Err(error) => self.status = vault_error(&error),
        }
        self.key.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(super) fn vault_controls(&self, cx: &mut Context<Self>) -> Div {
        let visual = crate::design::palette(cx);
        let reference = self.profile().and_then(ai_credentials::reference);
        let has_key = self
            .selected
            .and_then(|id| self.credentials.get(&id))
            .is_some_and(|key| !key.is_empty());
        let mut content = div().flex().flex_col().gap_2().p_2().rounded(px(6.)).bg(rgb(visual.canvas))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(if reference.is_some() {
                if has_key { t(cx, "已关联加密密钥 · 本次运行已解锁", "Encrypted key linked · unlocked in this process") }
                else { t(cx, "已关联加密密钥 · 需要主密码解锁", "Encrypted key linked · master password required") }
            } else { t(cx, "密钥默认仅驻留内存；仅点击加密保存才写入凭据库。", "Keys stay in memory unless you explicitly save them encrypted.") }))
            .child(div().flex().flex_wrap().gap_2()
                .child(Button::new("ai-key-save").ghost().label(t(cx, "加密保存密钥", "Save key encrypted")).disabled(self.saving || self.vault_busy() || !has_key).on_click(cx.listener(|panel, _, window, cx| panel.begin_vault(VaultAction::Save, window, cx))))
                .child(Button::new("ai-key-unlock").ghost().label(t(cx, "解锁密钥", "Unlock key")).disabled(self.saving || self.vault_busy() || reference.is_none()).on_click(cx.listener(|panel, _, window, cx| panel.begin_vault(VaultAction::Unlock, window, cx))))
                .child(Button::new("ai-key-lock").ghost().label(t(cx, "清除临时密钥", "Clear temporary key")).disabled(self.saving || self.vault_busy() || !has_key).on_click(cx.listener(|panel, _, window, cx| panel.unlink_or_lock(false, window, cx))))
                .child(Button::new("ai-key-unlink").ghost().label(t(cx, "解除关联", "Unlink key")).disabled(self.saving || self.vault_busy() || reference.is_none()).on_click(cx.listener(|panel, _, window, cx| panel.unlink_or_lock(true, window, cx)))))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(cx, "更改地址、调用方式、CLI 路径或认证会清除草稿密钥与关联。清除与解除关联需应用后对助手生效。", "Changing endpoint, invocation, CLI path or authentication clears the draft key and reference. Apply clearing/unlinking to update the assistant.")));
        if let Some(prompt) = &self.vault_prompt {
            content = content
                .child(
                    div()
                        .text_xs()
                        .child(t(cx, "凭据库主密码", "Vault master password")),
                )
                .child(
                    Input::new(&prompt.master)
                        .id("ai-vault-master")
                        .disabled(prompt.busy),
                );
            if prompt.action == VaultAction::Save {
                content = content
                    .child(div().text_xs().child(t(
                        cx,
                        "再次输入主密码（新库会使用此密码）",
                        "Repeat master password (used for a new vault)",
                    )))
                    .child(
                        Input::new(&prompt.confirmation)
                            .id("ai-vault-confirmation")
                            .disabled(prompt.busy),
                    );
            }
            content = content.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("ai-vault-submit")
                            .primary()
                            .label(t(cx, "确认", "Confirm"))
                            .disabled(prompt.busy)
                            .on_click(
                                cx.listener(|panel, _, window, cx| panel.submit_vault(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("ai-vault-cancel")
                            .ghost()
                            .label(t(cx, "取消凭据操作", "Cancel credential operation"))
                            .on_click(
                                cx.listener(|panel, _, window, cx| panel.dismiss_vault(window, cx)),
                            ),
                    ),
            );
        }
        content
    }
}

fn vault_error(error: &Error) -> Message {
    match error {
        Error::VaultUnlockFailed => Message::new(
            "无法解锁：主密码错误或凭据库损坏。",
            "Unlock failed: incorrect master password or damaged vault.",
        ),
        Error::VaultEntryMismatch => Message::new(
            "密钥不属于当前模型服务，请解除关联后重新填写。",
            "The key belongs to a different provider configuration. Unlink it and enter a new key.",
        ),
        Error::VaultEntryNotFound => Message::new(
            "找不到加密密钥，请解除关联后重新填写。",
            "Encrypted key not found. Unlink it and enter a new key.",
        ),
        Error::VaultConflict | Error::Conflict | Error::Busy => Message::new(
            "凭据库已变更或正在使用，请重试。",
            "The vault changed or is busy. Retry the operation.",
        ),
        _ => Message::new(
            "凭据操作失败；请检查凭据库权限和配置。未发送请求。",
            "Credential operation failed. Check vault permissions and configuration. No request was sent.",
        ),
    }
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
