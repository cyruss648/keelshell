//! Credential maintenance without retaining a master password or unlocked vault.

#[cfg(test)]
mod tests;
mod view;
mod worker;

use crate::i18n::{Message, t};
use gpui_kit::{component::input::InputState, *};
use keelshell_core::{CredentialMetadata, Error};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::runtime::Runtime;
use uuid::Uuid;
use worker::{Action, Failure, Report};
use zeroize::Zeroizing;

/// Closing is emitted only after background work releases its admission window.
pub enum VaultSettingsEvent {
    Close { message: Option<Message> },
}

/// An exclusive maintenance modal. The workspace must freeze credential-reference
/// mutations while it is open, and wait for in-flight state saves before opening.
/// Each operation independently authenticates; only non-secret metadata is kept.
pub struct VaultSettings {
    path: PathBuf,
    state_path: PathBuf,
    runtime: Arc<Runtime>,
    workspace_references: BTreeSet<Uuid>,
    in_use: BTreeSet<Uuid>,
    profile_names: BTreeMap<Uuid, String>,
    entries: Vec<CredentialMetadata>,
    inspected: bool,
    page: usize,
    master: Entity<InputState>,
    replacement: Entity<InputState>,
    confirmation: Entity<InputState>,
    deletion: Option<Uuid>,
    cancellation: Option<Arc<AtomicBool>>,
    close_after_work: bool,
    status: Message,
    _job: Option<Task<()>>,
}

impl EventEmitter<VaultSettingsEvent> for VaultSettings {}

fn secret_field(window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).masked(true))
}

impl VaultSettings {
    pub fn new(
        path: PathBuf,
        state_path: PathBuf,
        runtime: Arc<Runtime>,
        in_use: BTreeSet<Uuid>,
        profile_names: BTreeMap<Uuid, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self {
            path,
            state_path,
            runtime,
            workspace_references: in_use.clone(),
            in_use,
            profile_names,
            entries: Vec::new(),
            inspected: false,
            page: 0,
            master: secret_field(window, cx),
            replacement: secret_field(window, cx),
            confirmation: secret_field(window, cx),
            deletion: None,
            cancellation: None,
            close_after_work: false,
            status: Message::new(
                "输入当前主密码以验证并检查凭据库。每次操作都需重新验证。",
                "Enter the current master password to inspect the vault. Every operation authenticates again.",
            ),
            _job: None,
        };
        panel.refresh_locale(window, cx);
        panel.focus(window, cx);
        panel
    }

    pub fn is_busy(&self) -> bool {
        self.cancellation.is_some()
    }

    /// Call before allowing a fresh destructive confirmation. An update invalidates
    /// any pending confirmation and requests cancellation before save admission.
    pub fn update_references(&mut self, in_use: BTreeSet<Uuid>, cx: &mut Context<Self>) {
        if self.workspace_references != in_use {
            self.workspace_references = in_use.clone();
            self.in_use = in_use;
            self.deletion = None;
            if let Some(cancelled) = &self.cancellation {
                cancelled.store(true, Ordering::Release);
            }
            self.status = Message::new(
                "配置引用已变化，请重新检查凭据库。",
                "Configuration references changed. Inspect the vault again.",
            );
            self.inspected = false;
            self.entries.clear();
            cx.notify();
        }
    }

    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (input, zh, en) in [
            (
                &self.master,
                "当前主密码（仅本次操作）",
                "Current master password (this operation only)",
            ),
            (&self.replacement, "新主密码", "New master password"),
            (
                &self.confirmation,
                "再次输入新主密码",
                "Repeat the new master password",
            ),
        ] {
            input.update(cx, |input, cx| {
                input.set_placeholder(t(cx, zh, en), window, cx)
            });
        }
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.master.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn clear_inputs(&self, window: &mut Window, cx: &mut App) {
        for input in [&self.master, &self.replacement, &self.confirmation] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_inputs(window, cx);
        self.deletion = None;
        if self.is_busy() {
            self.close_after_work = true;
            self.cancel(cx);
        } else {
            self.entries.clear();
            self.inspected = false;
            cx.emit(VaultSettingsEvent::Close { message: None });
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancelled) = &self.cancellation {
            cancelled.store(true, Ordering::Release);
            self.status = Message::new(
                "已请求取消；等待后台结束。已经开始的保存可能完成。",
                "Cancellation requested; waiting for background work. A save already started may complete.",
            );
            cx.notify();
        }
    }

    fn lock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.clear_inputs(window, cx);
        self.entries.clear();
        self.inspected = false;
        self.deletion = None;
        self.status = Message::new(
            "已锁定并隐藏条目；主密码不会保留。",
            "Locked and entries hidden; the master password is not retained.",
        );
        self.focus(window, cx);
        cx.notify();
    }

    fn request_delete(&mut self, reference: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy()
            || !self.inspected
            || self.in_use.contains(&reference)
            || !self
                .entries
                .iter()
                .any(|entry| entry.reference == reference)
        {
            return;
        }
        self.clear_inputs(window, cx);
        self.deletion = Some(reference);
        self.status = Message::new(
            "此条目在当前已保存配置中未关联。删除不可撤销，请再次输入主密码并确认。",
            "This entry is unlinked in the currently saved configuration. Deletion is irreversible; enter the master password again and confirm.",
        );
        self.focus(window, cx);
        cx.notify();
    }

    fn submit(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        if let Action::Delete(reference) = action
            && (self.deletion != Some(reference)
                || self.in_use.contains(&reference)
                || !self.inspected)
        {
            return;
        }
        if matches!(action, Action::Rotate) && !self.inspected {
            return;
        }
        let master = Zeroizing::new(self.master.read(cx).value().to_string());
        let replacement = Zeroizing::new(self.replacement.read(cx).value().to_string());
        let confirmation = Zeroizing::new(self.confirmation.read(cx).value().to_string());
        self.clear_inputs(window, cx);
        if master.is_empty() || master.len() > 4096 {
            self.status = Message::new(
                "请输入有效的当前主密码。",
                "Enter a valid current master password.",
            );
            self.focus(window, cx);
            cx.notify();
            return;
        }
        if matches!(action, Action::Rotate)
            && (replacement.is_empty() || replacement.len() > 4096 || *replacement != *confirmation)
        {
            self.status = Message::new(
                "新主密码不能为空，且两次输入必须一致（最多 4096 字节）。",
                "The new master password must be nonempty and match its confirmation (at most 4096 bytes).",
            );
            self.replacement.read(cx).focus_handle(cx).focus(window, cx);
            cx.notify();
            return;
        }
        drop(confirmation);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancellation = Some(cancelled.clone());
        self.deletion = None;
        self.status = Message::new(
            "正在后台验证和处理…",
            "Authenticating and processing in the background…",
        );
        let path = self.path.clone();
        let state_path = self.state_path.clone();
        let in_use = self.workspace_references.clone();
        let task = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                tokio::task::spawn_blocking(move || {
                    worker::operate(
                        path,
                        state_path,
                        in_use,
                        action,
                        master,
                        replacement,
                        &cancelled,
                    )
                })
                .await
            },
        );
        self._job = Some(cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(Ok(result)) => result,
                _ => Err(Failure::Worker),
            };
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.complete(result, action, window, cx)
            });
        }));
        cx.notify();
    }

    fn complete(
        &mut self,
        result: Result<Report, Failure>,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancellation = None;
        match result {
            Ok(Report::Cancelled) => {
                self.status = Message::new("已取消；未开始保存。", "Cancelled before saving.");
            }
            Ok(Report::Ready { entries, in_use }) => {
                self.entries = entries;
                self.page = self.page.min(self.entries.len().saturating_sub(1) / 50);
                self.in_use = in_use;
                self.in_use
                    .extend(self.workspace_references.iter().copied());
                self.inspected = true;
                self.status = match action {
                    Action::Inspect => Message::new(
                        format!(
                            "验证成功，共 {} 条凭据；列表不显示秘密。",
                            self.entries.len()
                        ),
                        format!(
                            "Verified {} credentials; secrets are never displayed.",
                            self.entries.len()
                        ),
                    ),
                    Action::Delete(_) => {
                        Message::new("未关联凭据已删除。", "Unlinked credential deleted.")
                    }
                    Action::Rotate => Message::new(
                        "主密码已更改；条目引用保持不变。后续操作请使用新主密码。",
                        "Master password changed; entry references are unchanged. Use the new master password for subsequent operations.",
                    ),
                };
            }
            Err(error) => {
                self.inspected = false;
                self.entries.clear();
                self.deletion = None;
                self.status = failure_message(error);
            }
        }
        if self.close_after_work {
            self.close_after_work = false;
            self.clear_inputs(window, cx);
            self.entries.clear();
            self.inspected = false;
            self.deletion = None;
            cx.emit(VaultSettingsEvent::Close {
                message: Some(self.status.clone()),
            });
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }
}

impl Drop for VaultSettings {
    fn drop(&mut self) {
        if let Some(cancelled) = &self.cancellation {
            cancelled.store(true, Ordering::Release);
        }
    }
}

fn failure_message(error: Failure) -> Message {
    match error {
        Failure::Missing => Message::new(
            "尚无凭据库；请先在 SSH 或 AI 配置中显式保存凭据。",
            "No vault exists yet. Explicitly save a credential in SSH or AI settings first.",
        ),
        Failure::Linked => Message::new(
            "该条目已被当前已保存配置关联，不能删除。请重新检查列表。",
            "The entry is referenced by the currently saved configuration and cannot be deleted. Inspect again.",
        ),
        Failure::Worker => Message::new(
            "凭据维护后台任务失败，请重新检查。",
            "The maintenance worker failed. Inspect again.",
        ),
        Failure::Core(Error::VaultUnlockFailed) => Message::new(
            "无法验证凭据库：主密码错误或文件已被修改。",
            "Vault authentication failed: incorrect master password or modified file.",
        ),
        Failure::Core(Error::VaultConflict) => Message::new(
            "凭据库已被其他操作修改；未覆盖该文件。请重新检查并重试。",
            "Another operation changed the vault; it was not overwritten. Inspect again before retrying.",
        ),
        Failure::Core(Error::Durability(_)) => Message::new(
            "文件已替换，但无法确认磁盘同步；请重新检查，新主密码可能已生效。",
            "The file was replaced but disk sync could not be confirmed. Inspect again; the new password may already apply.",
        ),
        Failure::Core(error) => Message::detail(
            "凭据维护失败（输入已清空）",
            "Credential maintenance failed (inputs cleared)",
            error,
        ),
    }
}
