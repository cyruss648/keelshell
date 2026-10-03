//! Independent, named AI configuration editor. Only metadata is persisted;
//! encrypted references are optional and unlocking remains an explicit action.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use keelshell_ai::{
    AiError, AiErrorCategory, ConnectivityReport, ModelCatalog, ProviderClient, ProviderConfig,
    ProviderEndpoint, RequestCancellation,
};
use keelshell_core::{AiAuthentication, AiPreset, AiProfileCatalog, NamedAiProfile};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::i18n::{Message, t};

#[cfg(test)]
mod tests;
mod vault;
mod view;
use vault::VaultPrompt;

/// Temporary keys indexed by profile identity. Never serialize or debug this map.
pub type EphemeralCredentials = BTreeMap<Uuid, Zeroizing<String>>;

/// Persistence is owned by the workspace. Apply carries an immutable metadata
/// and credential snapshot; the panel closes only after persistence succeeds.
pub enum AiSettingsEvent {
    Apply {
        catalog: AiProfileCatalog,
        credentials: EphemeralCredentials,
        revision: u64,
    },
    Close,
}

#[derive(Clone, PartialEq, Eq)]
struct EditorValues {
    name: String,
    endpoint: String,
    model: String,
    key: Zeroizing<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperationKind {
    Models,
    Test,
}

enum OperationResult {
    Models(ModelCatalog),
    Test(ConnectivityReport),
}

/// DBX-style editor with a named configuration list and an independent form.
/// Editing, selecting or opening this panel never makes a network request.
pub struct AiSettingsPanel {
    catalog: AiProfileCatalog,
    credentials: EphemeralCredentials,
    selected: Option<Uuid>,
    focus: FocusHandle,
    name: Entity<InputState>,
    endpoint: Entity<InputState>,
    model: Entity<InputState>,
    key: Entity<InputState>,
    editor_values: EditorValues,
    clear_key_pending: bool,
    vault_path: PathBuf,
    vault_prompt: Option<VaultPrompt>,
    models: Vec<String>,
    status: Message,
    revision: u64,
    saving: bool,
    operation: Option<OperationKind>,
    operation_revision: u64,
    cancellation: Option<RequestCancellation>,
    runtime: Arc<Runtime>,
    _job: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

fn field(value: &str, placeholder: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut field = InputState::new(window, cx).placeholder(placeholder.to_owned());
        field.set_value(value.to_owned(), window, cx);
        field
    })
}

impl AiSettingsPanel {
    /// Copy the current settings into an isolated draft, including temporary keys.
    pub fn new(
        catalog: &AiProfileCatalog,
        credentials: &EphemeralCredentials,
        runtime: Arc<Runtime>,
        vault_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = catalog
            .active_id
            .or_else(|| catalog.profiles.first().map(|p| p.id));
        let profile = selected.and_then(|id| catalog.profiles.iter().find(|p| p.id == id));
        let values = EditorValues {
            name: profile.map_or_else(String::new, |p| p.name.clone()),
            endpoint: profile.map_or_else(String::new, |p| p.endpoint.clone()),
            model: profile.map_or_else(String::new, |p| p.model.clone()),
            key: selected
                .and_then(|id| credentials.get(&id))
                .cloned()
                .unwrap_or_default(),
        };
        let name = field(
            &values.name,
            t(
                cx,
                "例如：日常运维助手",
                "For example: Operations assistant",
            ),
            window,
            cx,
        );
        let endpoint = field(
            &values.endpoint,
            t(
                cx,
                "完整的 chat/completions 地址",
                "Full chat/completions URL",
            ),
            window,
            cx,
        );
        let model = field(
            &values.model,
            t(
                cx,
                "手动输入模型 ID 或从发现结果选择",
                "Enter a model ID or select a discovered model",
            ),
            window,
            cx,
        );
        let key = cx.new(|cx| {
            let mut key = InputState::new(window, cx).masked(true).placeholder(t(
                cx,
                "默认仅本次运行有效；可显式加密保存",
                "Temporary by default; optionally save encrypted",
            ));
            key.set_value(values.key.to_string(), window, cx);
            key
        });
        // A modal must take keyboard focus as well as occlude pointer input.
        // An empty catalog has no rendered input, so focus the panel itself.
        let focus = cx.focus_handle();
        if selected.is_some() {
            name.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            focus.focus(window, cx);
        }
        let subscriptions = [&name, &endpoint, &model, &key]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(field, window, |panel, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        panel.sync_editor(cx);
                        panel.clear_pending_key(window, cx);
                    }
                })
            })
            .collect();
        Self {
            catalog: catalog.clone(),
            credentials: credentials.clone(),
            selected,
            focus,
            name,
            endpoint,
            model,
            key,
            editor_values: values,
            clear_key_pending: false,
            vault_path,
            vault_prompt: None,
            models: Vec::new(),
            status: Message::new(
                "先选择或新建配置。发现模型与测试仅在你点击后联网。",
                "Select or create a profile. Discovery and tests run only when clicked.",
            ),
            revision: 0,
            saving: false,
            operation: None,
            operation_revision: 0,
            cancellation: None,
            runtime,
            _job: None,
            _subscriptions: subscriptions,
        }
    }

    /// Translate hints without modifying draft values or operation identity.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (field, zh, en) in [
            (
                &self.name,
                "例如：日常运维助手",
                "For example: Operations assistant",
            ),
            (
                &self.endpoint,
                "完整的 chat/completions 地址",
                "Full chat/completions URL",
            ),
            (
                &self.model,
                "手动输入模型 ID 或从发现结果选择",
                "Enter a model ID or select a discovered model",
            ),
            (
                &self.key,
                "默认仅本次运行有效；可显式加密保存",
                "Temporary by default; optionally save encrypted",
            ),
        ] {
            let hint = t(cx, zh, en);
            field.update(cx, |field, cx| field.set_placeholder(hint, window, cx));
        }
        cx.notify();
    }

    /// Mark a metadata persistence transaction as pending without locking edits.
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        cx.notify();
    }

    /// Acknowledge exactly the revision emitted in Apply. Edits made while the
    /// disk write was pending remain visible and are never silently overwritten.
    pub fn mark_saved(&mut self, revision: u64, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        self.saving = false;
        if self.revision == revision {
            cx.emit(AiSettingsEvent::Close);
        } else {
            self.status = Message::new(
                "先前的配置快照已保存；保存期间的新修改仍保留，请再次应用。",
                "The earlier snapshot was saved. New edits remain here; apply again to save them.",
            );
        }
        cx.notify();
    }

    /// Keep all editor values after a failed workspace save.
    pub fn report_failure(&mut self, message: Message, cx: &mut Context<Self>) {
        self.saving = false;
        self.status = message;
        cx.notify();
    }

    fn profile(&self) -> Option<&NamedAiProfile> {
        self.selected
            .and_then(|id| self.catalog.profiles.iter().find(|p| p.id == id))
    }

    fn read_values(&self, cx: &App) -> EditorValues {
        EditorValues {
            name: self.name.read(cx).value().to_string(),
            endpoint: self.endpoint.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
            key: Zeroizing::new(self.key.read(cx).value().to_string()),
        }
    }

    fn sync_editor(&mut self, cx: &mut Context<Self>) {
        let mut values = self.read_values(cx);
        if self.clear_key_pending {
            values.key.clear();
        }
        if values == self.editor_values {
            return;
        }
        let endpoint_changed = values.endpoint != self.editor_values.endpoint;
        let key_changed = values.key != self.editor_values.key;
        if endpoint_changed {
            // Reject the old field value even before its queued UI clear runs.
            // A later name/model edit must not restore the previous destination's key.
            values.key.clear();
            self.clear_key_pending = true;
        }
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            profile.name.clone_from(&values.name);
            profile.endpoint.clone_from(&values.endpoint);
            profile.model.clone_from(&values.model);
            if (endpoint_changed || key_changed)
                && matches!(profile.authentication, AiAuthentication::Bearer { .. })
            {
                profile.authentication = AiAuthentication::Bearer { credential: None };
            }
            if values.key.is_empty() {
                self.credentials.remove(&profile.id);
            } else {
                self.credentials.insert(profile.id, values.key.clone());
            }
        }
        self.editor_values = values;
        self.changed(endpoint_changed || key_changed, cx);
    }

    fn clear_pending_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clear_key_pending {
            self.key.update(cx, |key, cx| key.set_value("", window, cx));
            self.clear_key_pending = false;
        }
    }

    fn changed(&mut self, clear_models: bool, cx: &mut Context<Self>) {
        self.cancel_vault();
        self.revision = self.revision.wrapping_add(1);
        self.cancel_operation(false, cx);
        if clear_models {
            self.models.clear();
        }
        self.status = Message::new("配置有未保存的修改。", "Configuration has unsaved changes.");
        cx.notify();
    }

    fn load_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let values = EditorValues {
            name: self.profile().map_or_else(String::new, |p| p.name.clone()),
            endpoint: self
                .profile()
                .map_or_else(String::new, |p| p.endpoint.clone()),
            model: self.profile().map_or_else(String::new, |p| p.model.clone()),
            key: self
                .selected
                .and_then(|id| self.credentials.get(&id))
                .cloned()
                .unwrap_or_default(),
        };
        for (field, value) in [
            (&self.name, values.name.as_str()),
            (&self.endpoint, &values.endpoint),
            (&self.model, &values.model),
            (&self.key, &values.key),
        ] {
            field.update(cx, |field, cx| {
                field.set_value(value.to_owned(), window, cx)
            });
        }
        // Queued programmatic Change events see this exact signature and cannot
        // turn a locale refresh/selection into edits to another profile.
        self.editor_values = values;
        self.clear_key_pending = false;
    }

    fn select(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.selected == Some(id) {
            return;
        }
        self.cancel_operation(false, cx);
        self.models.clear();
        self.cancel_vault();
        self.selected = Some(id);
        self.load_editor(window, cx);
        self.status = Message::empty();
        cx.notify();
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.catalog.profiles.len() >= 64 {
            self.status = Message::new("最多创建 64 个配置。", "At most 64 profiles are allowed.");
            cx.notify();
            return;
        }
        let draft = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
        self.selected = Some(draft.id);
        self.catalog.profiles.push(draft);
        self.changed(true, cx);
        self.load_editor(window, cx);
        self.name.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            self.catalog.remove(id);
            self.credentials.remove(&id);
            self.selected = self.catalog.profiles.first().map(|p| p.id);
            self.changed(true, cx);
            self.load_editor(window, cx);
            if self.selected.is_none() {
                self.focus.focus(window, cx);
            }
        }
    }

    fn use_preset(&mut self, preset: AiPreset, window: &mut Window, cx: &mut Context<Self>) {
        if !preset.api_style().supports_current_transport() {
            return;
        }
        self.sync_editor(cx);
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            profile.preset = preset;
            profile.api_style = preset.api_style();
            profile.endpoint = preset.endpoint().into();
            profile.authentication = if preset == AiPreset::Ollama {
                AiAuthentication::None
            } else {
                AiAuthentication::Bearer { credential: None }
            };
            // A credential for one destination is never carried to another preset.
            self.credentials.remove(&profile.id);
            self.changed(true, cx);
            self.load_editor(window, cx);
        }
    }

    fn set_authentication(&mut self, bearer: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            if matches!(profile.authentication, AiAuthentication::Bearer { .. }) == bearer
                && matches!(
                    profile.authentication,
                    AiAuthentication::None | AiAuthentication::Bearer { .. }
                )
            {
                return;
            }
            self.credentials.remove(&profile.id);
            profile.authentication = if bearer {
                AiAuthentication::Bearer { credential: None }
            } else {
                AiAuthentication::None
            };
            self.changed(true, cx);
            self.load_editor(window, cx);
        }
    }

    fn make_default(&mut self, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        let Some(profile) = self.profile() else {
            return;
        };
        if let Err(error) = profile.validate_current_transport() {
            self.status = Message::detail("无法设为默认配置", "Cannot set default profile", error);
        } else {
            self.catalog.active_id = self.selected;
            self.changed(false, cx);
        }
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        if let Err(error) = self.catalog.validate() {
            self.status = Message::detail(
                "配置尚不完整或名称重复",
                "Invalid or duplicate profile",
                error,
            );
            cx.notify();
            return;
        }
        self.cancel_operation(false, cx);
        self.cancel_vault();
        self.saving = true;
        self.status = Message::new("正在保存配置…", "Saving configurations…");
        cx.emit(AiSettingsEvent::Apply {
            catalog: self.catalog.clone(),
            credentials: self.credentials.clone(),
            revision: self.revision,
        });
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.cancel_operation(false, cx);
        self.cancel_vault();
        cx.emit(AiSettingsEvent::Close);
    }

    fn cancel_operation(&mut self, show_status: bool, cx: &mut Context<Self>) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.operation = None;
        self.operation_revision = self.operation_revision.wrapping_add(1);
        self._job = None;
        if show_status {
            self.status = Message::new(
                "已取消本地请求；服务端可能已处理已发送的测试。",
                "Local request cancelled; the provider may already have processed a sent test.",
            );
        }
        cx.notify();
    }

    fn start_operation(&mut self, kind: OperationKind, cx: &mut Context<Self>) {
        if self.vault_prompt.is_some() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile().cloned() else {
            return;
        };
        // Discovery can work before a name/model is entered. Validate a temporary
        // metadata copy without claiming that placeholder values are usable.
        let mut validation = profile.clone();
        if validation.name.trim().is_empty() {
            validation.name = "Discovery validation".into();
        }
        if kind == OperationKind::Models && validation.model.is_empty() {
            validation.model = "discovery-only".into();
        }
        if let Err(error) = validation.validate_current_transport() {
            self.status = Message::detail(
                "当前配置无法请求",
                "Configuration cannot be requested",
                error,
            );
            cx.notify();
            return;
        }
        let key = match &profile.authentication {
            AiAuthentication::None => None,
            AiAuthentication::Bearer { .. } => self
                .credentials
                .get(&profile.id)
                .filter(|key| !key.is_empty())
                .cloned(),
            _ => None,
        };
        if matches!(profile.authentication, AiAuthentication::Bearer { .. }) && key.is_none() {
            self.status = Message::new(
                "请填写临时 API 密钥或解锁已保存的密钥，或明确选择无认证。",
                "Enter a temporary API key or unlock the saved key, or explicitly select no authentication.",
            );
            cx.notify();
            return;
        }
        self.cancel_operation(false, cx);
        let cancellation = RequestCancellation::new();
        self.cancellation = Some(cancellation.clone());
        self.operation = Some(kind);
        let operation_revision = self.operation_revision;
        self.status = match kind {
            OperationKind::Models => Message::new("正在发现模型…", "Discovering models…"),
            OperationKind::Test => {
                Message::new("正在发送固定连接测试…", "Sending a fixed connection test…")
            }
        };
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                let client = ProviderClient::new(Duration::from_secs(30), 1024 * 1024)?;
                let api_key = key.as_ref().map(|key| key.as_str());
                match kind {
                    OperationKind::Models => client
                        .discover_models(
                            &ProviderEndpoint::new(&profile.endpoint)?,
                            api_key,
                            &cancellation,
                        )
                        .await
                        .map(OperationResult::Models),
                    OperationKind::Test => client
                        .test_connection(
                            &ProviderConfig::new(&profile.endpoint, &profile.model)?,
                            api_key,
                            &cancellation,
                        )
                        .await
                        .map(OperationResult::Test),
                }
            },
        );
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(AiError::Transport));
            let _ = this.update(cx, |panel, cx| {
                panel.finish_operation(operation_revision, result, cx)
            });
        }));
        cx.notify();
    }

    fn finish_operation(
        &mut self,
        revision: u64,
        result: Result<OperationResult, AiError>,
        cx: &mut Context<Self>,
    ) {
        if revision != self.operation_revision {
            return;
        }
        self.operation = None;
        self.cancellation = None;
        match result {
            Ok(OperationResult::Models(catalog)) => {
                self.models = catalog.models().to_vec();
                self.status = Message::new(
                    format!(
                        "发现 {} 个模型，可点击选择；也可继续手动输入。",
                        self.models.len()
                    ),
                    format!(
                        "Discovered {} models. Select one or keep entering a model manually.",
                        self.models.len()
                    ),
                );
            }
            Ok(OperationResult::Test(report)) => {
                let model = report.actual_model();
                self.status = Message::new(
                    format!(
                        "连接测试成功 · {} ms · 服务端模型：{}",
                        report.elapsed().as_millis(),
                        model.unwrap_or("未返回")
                    ),
                    format!(
                        "Connection test succeeded · {} ms · server model: {}",
                        report.elapsed().as_millis(),
                        model.unwrap_or("not reported")
                    ),
                );
            }
            Err(error) => self.status = provider_error(&error),
        }
        cx.notify();
    }
}

impl Drop for AiSettingsPanel {
    fn drop(&mut self) {
        self.cancel_vault();
        if let Some(cancellation) = &self.cancellation {
            cancellation.cancel();
        }
    }
}
impl EventEmitter<AiSettingsEvent> for AiSettingsPanel {}

/// Bilingual, non-sensitive category labels; never include a service error body.
pub(crate) fn provider_error(error: &AiError) -> Message {
    let (zh, en) = match error.category() {
        AiErrorCategory::Configuration => ("配置错误", "Configuration"),
        AiErrorCategory::Authentication => ("认证失败", "Authentication"),
        AiErrorCategory::PermissionDenied => ("权限不足", "Permission denied"),
        AiErrorCategory::UnsupportedEndpoint => ("接口不支持", "Unsupported endpoint"),
        AiErrorCategory::RateLimited => ("速率或额度限制", "Rate or quota limit"),
        AiErrorCategory::ProviderUnavailable => ("模型服务不可用", "Provider unavailable"),
        AiErrorCategory::RedirectRejected => ("已拒绝重定向", "Redirect rejected"),
        AiErrorCategory::RequestRejected => ("服务拒绝请求", "Request rejected"),
        AiErrorCategory::Cancelled => ("已取消", "Cancelled"),
        AiErrorCategory::Timeout => ("请求超时", "Timeout"),
        AiErrorCategory::Transport => ("网络或 TLS 错误", "Network or TLS"),
        AiErrorCategory::ResponseTooLarge => ("回复超过大小限制", "Response too large"),
        AiErrorCategory::InvalidResponse => ("服务回复格式错误", "Invalid response"),
        AiErrorCategory::Review => ("审阅已失效", "Review invalid"),
        _ => ("AI 请求失败", "AI request failed"),
    };
    Message::new(
        format!("{zh}（未自动重试）"),
        format!("{en} (not retried automatically)"),
    )
}
