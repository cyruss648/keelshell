//! Independent, named AI configuration editor. Only metadata is persisted;
//! encrypted references are optional and unlocking remains an explicit action.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use keelshell_ai::{
    AiError, AiErrorCategory, ConnectivityReport, LocalAgentClient, LocalAgentConfig,
    LocalAgentError, LocalAgentKind, LocalAgentLimits, LocalAgentProbe, ModelCatalog,
    ProviderClient, ProviderConfig, ProviderEndpoint, ProviderProtocol, RequestCancellation,
};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiLocalAgent, AiLocalAgentLimits, AiPreset,
    AiProfileCatalog, NamedAiProfile,
};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::i18n::{Message, t};

mod inference;
mod request_options;
#[cfg(test)]
mod scrollbar_tests;
#[cfg(test)]
pub(crate) mod tests;
mod vault;
mod view;
use vault::VaultPrompt;

/// Temporary keys indexed by profile identity. Never serialize or debug this map.
pub use crate::ai_request_options::EphemeralCredentials;

/// Whether the selected profile uses one transient API-key value.
///
/// The wire adapter chooses the header scheme from the immutable protocol. The
/// settings and assistant layers only need to know whether a value must be
/// resolved; they must not infer a bearer header from this predicate.
pub(crate) fn uses_api_key(authentication: &AiAuthentication) -> bool {
    match authentication {
        AiAuthentication::Bearer { .. } => true,
        AiAuthentication::Header { name, .. } => name.eq_ignore_ascii_case("x-api-key"),
        AiAuthentication::None => false,
    }
}

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
struct LocalLimitDraft {
    timeout_seconds: String,
    answer_kib: String,
    output_kib: String,
}

impl LocalLimitDraft {
    fn for_profile(profile: Option<&NamedAiProfile>) -> Self {
        let limits = match profile.map(|profile| &profile.backend) {
            Some(AiBackend::LocalAgent { limits, .. }) => *limits,
            _ => AiLocalAgentLimits::default(),
        };
        Self {
            timeout_seconds: limits.timeout_seconds().to_string(),
            answer_kib: limits.answer_kib().to_string(),
            output_kib: limits.output_kib().to_string(),
        }
    }

    fn parse(&self) -> Result<AiLocalAgentLimits, ()> {
        fn whole(value: &str) -> Result<u16, ()> {
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(());
            }
            value.parse().map_err(|_| ())
        }
        AiLocalAgentLimits::new(
            whole(&self.timeout_seconds)?,
            whole(&self.answer_kib)?,
            whole(&self.output_kib)?,
        )
        .map_err(|_| ())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct EditorValues {
    name: String,
    executable: String,
    endpoint: String,
    model: String,
    key: Zeroizing<String>,
    context_tokens: String,
    output_tokens: String,
    local_limits: LocalLimitDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationKind {
    Models,
    Test,
    LocalProbe,
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
    form_scroll: ScrollHandle,
    name: Entity<InputState>,
    executable: Entity<InputState>,
    endpoint: Entity<InputState>,
    model: Entity<InputState>,
    key: Entity<InputState>,
    context_tokens: Entity<InputState>,
    output_tokens: Entity<InputState>,
    token_drafts: BTreeMap<Uuid, (String, String)>,
    inference_inputs: [Entity<InputState>; 3],
    inference_drafts: BTreeMap<(Uuid, String), inference::InferenceDraft>,
    inference_active: Option<(Uuid, String)>,
    inference_values: inference::InferenceDraft,
    request_editors: BTreeMap<Uuid, request_options::RequestEditor>,
    local_timeout: Entity<InputState>,
    local_answer: Entity<InputState>,
    local_output: Entity<InputState>,
    local_limit_drafts: BTreeMap<Uuid, LocalLimitDraft>,
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
            local_limits: LocalLimitDraft::for_profile(profile),
            name: profile.map_or_else(String::new, |p| p.name.clone()),
            executable: profile.map_or_else(String::new, executable_value),
            endpoint: profile.map_or_else(String::new, |p| p.endpoint.clone()),
            model: profile.map_or_else(String::new, |p| p.model.clone()),
            key: selected
                .and_then(|id| credentials.get(&id))
                .cloned()
                .unwrap_or_default(),
            context_tokens: profile
                .and_then(|p| p.context_window_tokens)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            output_tokens: profile
                .and_then(|p| p.max_output_tokens)
                .map(|v| v.to_string())
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
                "模型 API 完整请求地址 / CLI 服务基础地址",
                "Full API request URL / CLI service base URL",
            ),
            window,
            cx,
        );
        let executable = field(
            &values.executable,
            t(
                cx,
                "本地 CLI 可执行文件的绝对路径",
                "Absolute path to the native CLI executable",
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
        let context_tokens = field(
            &values.context_tokens,
            t(
                cx,
                "可选；按模型文档填写",
                "Optional; use the model's documented window",
            ),
            window,
            cx,
        );
        let output_tokens = field(
            &values.output_tokens,
            t(cx, "可选；1–1000000", "Optional; 1–1000000"),
            window,
            cx,
        );
        let inference_inputs = inference::inputs(window, cx);
        let local_timeout = field(
            &values.local_limits.timeout_seconds,
            t(cx, "必填；1–300 秒", "Required; 1–300 seconds"),
            window,
            cx,
        );
        let local_answer = field(
            &values.local_limits.answer_kib,
            t(cx, "必填；1–1024 KiB", "Required; 1–1024 KiB"),
            window,
            cx,
        );
        let local_output = field(
            &values.local_limits.output_kib,
            t(
                cx,
                "必填；1–8192 KiB，至少覆盖回答",
                "Required; 1–8192 KiB, at least the answer budget",
            ),
            window,
            cx,
        );
        // A modal must take keyboard focus as well as occlude pointer input.
        // An empty catalog has no rendered input, so focus the panel itself.
        let focus = cx.focus_handle();
        if selected.is_some() {
            name.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            focus.focus(window, cx);
        }
        let subscriptions = [
            &name,
            &executable,
            &endpoint,
            &model,
            &key,
            &context_tokens,
            &output_tokens,
            &inference_inputs[0],
            &inference_inputs[1],
            &inference_inputs[2],
            &local_timeout,
            &local_answer,
            &local_output,
        ]
        .into_iter()
        .map(|field| {
            cx.subscribe_in(field, window, |panel, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    panel.sync_editor(cx);
                    panel.clear_pending_key(window, cx);
                    panel.clear_pending_request_fields(window, cx);
                }
            })
        })
        .collect();
        let mut panel = Self {
            catalog: catalog.clone(),
            credentials: credentials.clone(),
            selected,
            focus,
            form_scroll: ScrollHandle::new(),
            name,
            executable,
            endpoint,
            model,
            key,
            context_tokens,
            output_tokens,
            token_drafts: BTreeMap::new(),
            inference_inputs,
            inference_drafts: BTreeMap::new(),
            inference_active: None,
            inference_values: inference::InferenceDraft(Default::default()),
            request_editors: BTreeMap::new(),
            local_timeout,
            local_answer,
            local_output,
            local_limit_drafts: BTreeMap::new(),
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
        };
        panel.load_inference_editor(window, cx);
        panel.load_request_editor(window, cx);
        panel
    }

    /// Translate hints without modifying draft values or operation identity.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (field, zh, en) in [
            (
                &self.local_timeout,
                "必填；1–300 秒",
                "Required; 1–300 seconds",
            ),
            (
                &self.local_answer,
                "必填；1–1024 KiB",
                "Required; 1–1024 KiB",
            ),
            (
                &self.local_output,
                "必填；1–8192 KiB，至少覆盖回答",
                "Required; 1–8192 KiB, at least the answer budget",
            ),
            (
                &self.executable,
                "本地 CLI 可执行文件的绝对路径",
                "Absolute path to the native CLI executable",
            ),
            (
                &self.name,
                "例如：日常运维助手",
                "For example: Operations assistant",
            ),
            (
                &self.endpoint,
                "模型 API 完整请求地址 / CLI 服务基础地址",
                "Full API request URL / CLI service base URL",
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
            (
                &self.context_tokens,
                "可选；按模型文档填写",
                "Optional; use the model's documented window",
            ),
            (
                &self.output_tokens,
                "可选；1–1000000",
                "Optional; 1–1000000",
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
            local_limits: LocalLimitDraft {
                timeout_seconds: self.local_timeout.read(cx).value().to_string(),
                answer_kib: self.local_answer.read(cx).value().to_string(),
                output_kib: self.local_output.read(cx).value().to_string(),
            },
            name: self.name.read(cx).value().to_string(),
            executable: self.executable.read(cx).value().to_string(),
            endpoint: self.endpoint.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
            key: Zeroizing::new(self.key.read(cx).value().to_string()),
            context_tokens: self.context_tokens.read(cx).value().to_string(),
            output_tokens: self.output_tokens.read(cx).value().to_string(),
        }
    }

    fn sync_editor(&mut self, cx: &mut Context<Self>) {
        let mut values = self.read_values(cx);
        if self.clear_key_pending {
            values.key.clear();
        }
        let endpoint_changed = values.endpoint != self.editor_values.endpoint
            || values.executable != self.editor_values.executable;
        let inference_changed = self.sync_inference_editor(cx);
        let request_changed = self.sync_request_editor(endpoint_changed, cx) || inference_changed;
        if values == self.editor_values && !request_changed {
            return;
        }
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
            if endpoint_changed {
                profile.reasoning_by_model.clear();
                profile.sampling_by_model.clear();
                self.inference_drafts.retain(|(id, _), _| *id != profile.id);
                self.inference_active = None;
            }
            profile.name.clone_from(&values.name);
            profile.endpoint.clone_from(&values.endpoint);
            profile.model.clone_from(&values.model);
            if let AiBackend::LocalAgent {
                executable, limits, ..
            } = &mut profile.backend
            {
                executable.clone_from(&values.executable);
                self.local_limit_drafts
                    .insert(profile.id, values.local_limits.clone());
                // Invalid text remains an editor draft; every launch/save checks
                // that draft before it can consume the last valid typed value.
                if let Ok(valid) = values.local_limits.parse() {
                    *limits = valid;
                }
            }
            self.token_drafts.insert(
                profile.id,
                (values.context_tokens.clone(), values.output_tokens.clone()),
            );
            if let (Ok(context), Ok(output)) = (
                parse_token_input(&values.context_tokens, 16 * 1024 * 1024),
                parse_token_input(&values.output_tokens, 1_000_000),
            ) {
                profile.context_window_tokens = context;
                profile.max_output_tokens = output;
            }
            if endpoint_changed || key_changed {
                match &mut profile.authentication {
                    AiAuthentication::Bearer { credential }
                    | AiAuthentication::Header {
                        name: _,
                        credential,
                    } => *credential = None,
                    AiAuthentication::None => {}
                }
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
        } else {
            // A credential may become known after the previous catalog arrived.
            // Invalidate literal secret IDs as well as the in-flight operation.
            let known = self.credentials.all_secrets();
            self.models.retain(|model| {
                !known
                    .iter()
                    .any(|secret| !secret.is_empty() && model.contains(secret))
            });
        }
        self.status = Message::new("配置有未保存的修改。", "Configuration has unsaved changes.");
        cx.notify();
    }

    fn load_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let token_draft = self.selected.and_then(|id| self.token_drafts.get(&id));
        let values = EditorValues {
            local_limits: self
                .selected
                .and_then(|id| self.local_limit_drafts.get(&id))
                .cloned()
                .unwrap_or_else(|| LocalLimitDraft::for_profile(self.profile())),
            name: self.profile().map_or_else(String::new, |p| p.name.clone()),
            executable: self.profile().map_or_else(String::new, executable_value),
            endpoint: self
                .profile()
                .map_or_else(String::new, |p| p.endpoint.clone()),
            model: self.profile().map_or_else(String::new, |p| p.model.clone()),
            key: self
                .selected
                .and_then(|id| self.credentials.get(&id))
                .cloned()
                .unwrap_or_default(),
            context_tokens: token_draft.map(|v| v.0.clone()).unwrap_or_else(|| {
                self.profile()
                    .and_then(|p| p.context_window_tokens)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }),
            output_tokens: token_draft.map(|v| v.1.clone()).unwrap_or_else(|| {
                self.profile()
                    .and_then(|p| p.max_output_tokens)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }),
        };
        for (field, value) in [
            (&self.name, values.name.as_str()),
            (&self.executable, &values.executable),
            (&self.endpoint, &values.endpoint),
            (&self.model, &values.model),
            (&self.key, &values.key),
            (&self.context_tokens, &values.context_tokens),
            (&self.output_tokens, &values.output_tokens),
            (&self.local_timeout, &values.local_limits.timeout_seconds),
            (&self.local_answer, &values.local_limits.answer_kib),
            (&self.local_output, &values.local_limits.output_kib),
        ] {
            field.update(cx, |field, cx| {
                field.set_value(value.to_owned(), window, cx)
            });
        }
        // Queued programmatic Change events see this exact signature and cannot
        // turn a locale refresh/selection into edits to another profile.
        self.editor_values = values;
        self.clear_key_pending = false;
        self.load_inference_editor(window, cx);
        self.load_request_editor(window, cx);
        self.clear_pending_request_fields(window, cx);
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
            self.credentials.clear_requests(id);
            self.request_editors.remove(&id);
            self.credentials.remove(&id);
            self.local_limit_drafts.remove(&id);
            self.token_drafts.remove(&id);
            self.inference_drafts.retain(|(owner, _), _| *owner != id);
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
        if self.profile().is_some_and(|p| p.backend != AiBackend::Api) {
            return;
        }
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            profile.reasoning_by_model.clear();
            profile.sampling_by_model.clear();
            self.inference_drafts
                .retain(|(owner, _), _| *owner != profile.id);
            self.inference_active = None;
            profile.preset = preset;
            profile.api_style = preset.api_style();
            profile.endpoint = preset.endpoint().into();
            profile.authentication = match preset.api_style() {
                AiApiStyle::AnthropicMessages => AiAuthentication::Header {
                    name: "x-api-key".into(),
                    credential: None,
                },
                _ if preset == AiPreset::Ollama => AiAuthentication::None,
                _ => AiAuthentication::Bearer { credential: None },
            };
            let id = profile.id;
            for header in &mut profile.custom_headers {
                header.value_ref = keelshell_core::AiSecretRef::Ephemeral { id: Uuid::new_v4() };
            }
            if let keelshell_core::AiProxy::Explicit {
                credentials: Some(reference),
                ..
            } = &mut profile.proxy
            {
                *reference = keelshell_core::AiSecretRef::Ephemeral { id: Uuid::new_v4() };
            }
            self.credentials.clear_requests(id);
            self.request_editors.remove(&id);
            // A credential for one destination is never carried to another preset.
            self.credentials.remove(&profile.id);
            self.changed(true, cx);
            self.load_editor(window, cx);
        }
    }

    fn set_api_style(&mut self, style: AiApiStyle, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.profile().is_some_and(|p| p.backend != AiBackend::Api) {
            return;
        }
        let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        else {
            return;
        };
        if profile.api_style == style {
            return;
        }
        let old_endpoint = profile.endpoint.clone();
        profile.reasoning_by_model.clear();
        profile.sampling_by_model.clear();
        self.inference_drafts
            .retain(|(owner, _), _| *owner != profile.id);
        self.inference_active = None;
        profile.api_style = style;
        profile.endpoint = match style {
            AiApiStyle::ChatCompletions => replace_protocol_suffixes(
                &old_endpoint,
                &["/responses", "/messages"],
                "/chat/completions",
            ),
            AiApiStyle::Responses => replace_protocol_suffixes(
                &old_endpoint,
                &["/chat/completions", "/messages"],
                "/responses",
            ),
            AiApiStyle::AnthropicMessages => replace_protocol_suffixes(
                &old_endpoint,
                &["/chat/completions", "/responses"],
                "/messages",
            ),
        };
        // The authentication scheme belongs to the protocol. Even a custom
        // endpoint whose suffix cannot be rewritten must not retain a bearer
        // credential when switching to Anthropic, or vice versa.
        profile.authentication = match style {
            AiApiStyle::AnthropicMessages => AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            },
            _ => AiAuthentication::Bearer { credential: None },
        };
        self.credentials.remove(&profile.id);
        self.sync_request_editor(true, cx);
        self.changed(true, cx);
        self.load_editor(window, cx);
    }

    fn set_authentication(&mut self, bearer: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.profile().is_some_and(|p| p.backend != AiBackend::Api) {
            return;
        }
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

    fn set_header_authentication(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.profile().is_some_and(|p| p.backend != AiBackend::Api) {
            return;
        }
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            if matches!(
                &profile.authentication,
                AiAuthentication::Header { name, .. }
                    if name.eq_ignore_ascii_case("x-api-key")
            ) {
                return;
            }
            self.credentials.remove(&profile.id);
            profile.authentication = AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
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
        if let Err(error) = validate_selected_transport(profile) {
            self.status = Message::detail("无法设为默认配置", "Cannot set default profile", error);
        } else {
            self.catalog.active_id = self.selected;
            self.changed(false, cx);
        }
        cx.notify();
    }

    fn set_backend(
        &mut self,
        agent: Option<AiLocalAgent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_editor(cx);
        let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        else {
            return;
        };
        let current = match profile.backend {
            AiBackend::Api => None,
            AiBackend::LocalAgent { agent, .. } => Some(agent),
        };
        if current == agent {
            return;
        }
        // Destination, executable and authentication form one approval boundary.
        // Switching adapters cannot carry keys or unsupported HTTP controls.
        self.credentials.remove(&profile.id);
        self.token_drafts.remove(&profile.id);
        self.local_limit_drafts.remove(&profile.id);
        profile.max_output_tokens = None;
        profile.context_window_tokens = None;
        profile.reasoning_by_model.clear();
        profile.sampling_by_model.clear();
        self.inference_drafts
            .retain(|(owner, _), _| *owner != profile.id);
        self.inference_active = None;
        profile.custom_headers.clear();
        profile.proxy = keelshell_core::AiProxy::Direct;
        self.credentials.clear_requests(profile.id);
        self.request_editors.remove(&profile.id);
        profile.model.clear();
        if let Some(agent) = agent {
            profile.backend = AiBackend::LocalAgent {
                agent,
                executable: String::new(),
                limits: AiLocalAgentLimits::default(),
            };
            profile.preset = AiPreset::Custom;
            match agent {
                AiLocalAgent::Codex => {
                    profile.api_style = AiApiStyle::Responses;
                    profile.endpoint = "https://api.openai.com/v1".into();
                    profile.authentication = AiAuthentication::Bearer { credential: None };
                }
                AiLocalAgent::ClaudeCode => {
                    profile.api_style = AiApiStyle::AnthropicMessages;
                    profile.endpoint = "https://api.anthropic.com".into();
                    profile.authentication = AiAuthentication::Header {
                        name: "x-api-key".into(),
                        credential: None,
                    };
                }
            }
        } else {
            profile.backend = AiBackend::Api;
            profile.preset = AiPreset::OpenAiCompatible;
            profile.api_style = AiApiStyle::ChatCompletions;
            profile.endpoint = AiPreset::OpenAiCompatible.endpoint().into();
            profile.authentication = AiAuthentication::Bearer { credential: None };
        }
        self.changed(true, cx);
        self.load_editor(window, cx);
    }

    fn start_local_probe(&mut self, cx: &mut Context<Self>) {
        if self.vault_prompt.is_some() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile() else {
            return;
        };
        if !self.local_limit_draft_valid(profile.id) {
            self.status = local_limit_draft_error();
            cx.notify();
            return;
        }
        let config = match local_agent_config(profile, true) {
            Ok(config) => config,
            Err(error) => {
                self.status = local_agent_error(error);
                cx.notify();
                return;
            }
        };
        self.cancel_operation(false, cx);
        let cancellation = RequestCancellation::new();
        self.cancellation = Some(cancellation.clone());
        self.operation = Some(OperationKind::LocalProbe);
        let revision = self.operation_revision;
        self.status = Message::new(
            "正在检查 CLI 版本与能力；不发送模型请求…",
            "Checking CLI version and capabilities; no inference request…",
        );
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move { LocalAgentClient.probe(&config, &cancellation).await },
        );
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(LocalAgentError::PipeFailed));
            let _ = this.update(cx, |panel, cx| {
                panel.finish_local_probe(revision, result, cx)
            });
        }));
        cx.notify();
    }

    fn finish_local_probe(
        &mut self,
        revision: u64,
        result: Result<LocalAgentProbe, LocalAgentError>,
        cx: &mut Context<Self>,
    ) {
        if revision != self.operation_revision {
            return;
        }
        self.operation = None;
        self.cancellation = None;
        self.status = match result {
            Ok(probe) => Message::new(
                format!(
                    "{} {} 检查通过；发送时将再次核对，不代表模型连接已测试。",
                    probe.kind().label(),
                    probe.version()
                ),
                format!(
                    "{} {} capabilities checked; admission repeats on send. Inference connectivity was not tested.",
                    probe.kind().label(),
                    probe.version()
                ),
            ),
            Err(error) => local_agent_error(error),
        };
        cx.notify();
    }

    fn local_limit_draft_valid(&self, id: Uuid) -> bool {
        self.catalog
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .is_some_and(|profile| {
                profile.backend == AiBackend::Api
                    || self
                        .local_limit_drafts
                        .get(&id)
                        .is_none_or(|draft| draft.parse().is_ok())
            })
    }

    fn token_draft_valid(&self, id: Uuid) -> bool {
        if let Some((context, output)) = self.token_drafts.get(&id) {
            return parse_token_input(context, 16 * 1024 * 1024).is_ok()
                && parse_token_input(output, 1_000_000).is_ok();
        }
        self.catalog
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .is_some_and(|profile| {
                profile
                    .max_output_tokens
                    .is_none_or(|value| value > 0 && value <= 1_000_000)
                    && profile
                        .context_window_tokens
                        .is_none_or(|value| value > 0 && value <= 16 * 1024 * 1024)
            })
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.vault_busy() {
            return;
        }
        self.sync_editor(cx);
        if self
            .catalog
            .profiles
            .iter()
            .any(|profile| !self.token_draft_valid(profile.id))
        {
            self.status = Message::new(
                "Token 限制必须为空或范围内的正整数，请检查各配置。",
                "Token limits must be empty or positive integers within range. Check each profile.",
            );
            cx.notify();
            return;
        }
        if self
            .catalog
            .profiles
            .iter()
            .any(|profile| !self.inference_draft_valid(profile.id))
        {
            self.status = Self::inference_error();
            cx.notify();
            return;
        }
        if self
            .catalog
            .profiles
            .iter()
            .any(|profile| !self.local_limit_draft_valid(profile.id))
        {
            self.status = local_limit_draft_error();
            cx.notify();
            return;
        }
        if self
            .catalog
            .profiles
            .iter()
            .any(|p| !self.request_draft_valid(p.id))
        {
            self.status = Message::new(
                "请先修正请求头或代理引用。",
                "Correct custom headers or proxy references first.",
            );
            cx.notify();
            return;
        }
        if let Err(error) = self.catalog.validate() {
            self.status = Message::detail(
                "配置尚不完整或名称重复",
                "Invalid or duplicate profile",
                error,
            );
            cx.notify();
            return;
        }
        if crate::ai_request_options::validate_catalog_metadata(&self.catalog, &self.credentials)
            .is_err()
        {
            self.status = Message::new(
                "配置文字包含已知秘密或秘密集合超限，请修正后再保存。",
                "Configuration text contains a known secret or the secret set exceeds its limit. Correct it before saving.",
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
        if kind == OperationKind::LocalProbe {
            self.start_local_probe(cx);
            return;
        }
        if self.vault_prompt.is_some() {
            return;
        }
        self.sync_editor(cx);
        let Some(profile) = self.profile().cloned() else {
            return;
        };
        if !self.request_draft_valid(profile.id) {
            self.status = Message::new(
                "请先修正请求头或代理引用。",
                "Correct custom headers or proxy references first.",
            );
            cx.notify();
            return;
        }
        if !self.inference_draft_valid(profile.id) {
            self.status = Self::inference_error();
            cx.notify();
            return;
        }
        if !self.token_draft_valid(profile.id) {
            self.status = Message::new(
                "请先修正 Token 限制。",
                "Correct the token limits before requesting.",
            );
            cx.notify();
            return;
        }
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
        let key =
            match crate::ai_request_options::resolve_authentication(&profile, &self.credentials) {
                Ok(key) => key,
                Err(error) => {
                    self.status = provider_error(&error);
                    cx.notify();
                    return;
                }
            };
        let options = match crate::ai_request_options::resolve_options(&profile, &self.credentials)
        {
            Ok(options) => options,
            Err(error) => {
                self.status = provider_error(&error);
                cx.notify();
                return;
            }
        };
        if uses_api_key(&profile.authentication) && key.is_none() {
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
            OperationKind::LocalProbe => Message::empty(),
        };
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                let client = ProviderClient::new_with_options(
                    Duration::from_secs(30),
                    1024 * 1024,
                    options.clone(),
                )?;
                let api_key = key.as_ref().map(|key| key.as_str());
                let protocol = match profile.api_style {
                    AiApiStyle::ChatCompletions => ProviderProtocol::ChatCompletions,
                    AiApiStyle::Responses => ProviderProtocol::Responses,
                    AiApiStyle::AnthropicMessages => ProviderProtocol::AnthropicMessages,
                };
                match kind {
                    OperationKind::Models => client
                        .discover_models(
                            &ProviderEndpoint::new_with_protocol(&profile.endpoint, protocol)?,
                            api_key,
                            &cancellation,
                        )
                        .await
                        .map(OperationResult::Models),
                    OperationKind::Test => client
                        .test_connection_with_limits(
                            &ProviderConfig::new_with_protocol(
                                &profile.endpoint,
                                &profile.model,
                                protocol,
                            )?
                            .with_request_options(options)
                            .with_inference_options(
                                crate::ai_request_options::inference_options(&profile)?,
                            )?,
                            api_key,
                            &cancellation,
                            profile.max_output_tokens,
                            profile.context_window_tokens,
                        )
                        .await
                        .map(OperationResult::Test),
                    OperationKind::LocalProbe => Err(AiError::InvalidEndpoint),
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

fn executable_value(profile: &NamedAiProfile) -> String {
    match &profile.backend {
        AiBackend::Api => String::new(),
        AiBackend::LocalAgent { executable, .. } => executable.clone(),
    }
}

fn validate_selected_transport(
    profile: &NamedAiProfile,
) -> Result<(), keelshell_core::ValidationError> {
    match profile.backend {
        AiBackend::Api => profile.validate_current_transport(),
        AiBackend::LocalAgent { .. } => profile.validate_local_agent_transport(),
    }
}

/// Construct immutable CLI metadata without filesystem lookup or process I/O.
/// Probe placeholders are limited to capability detection and cannot become an
/// approved inference request or silently change stored model/name values.
pub(crate) fn local_agent_config(
    profile: &NamedAiProfile,
    probing: bool,
) -> Result<LocalAgentConfig, LocalAgentError> {
    let mut metadata = profile.clone();
    if probing {
        if metadata.name.trim().is_empty() {
            metadata.name = "CLI capability probe".into();
        }
        if metadata.model.is_empty() {
            metadata.model = "capability-probe-only".into();
        }
    }
    metadata
        .validate_local_agent_transport()
        .map_err(|_| LocalAgentError::InvalidConfiguration)?;
    let AiBackend::LocalAgent {
        agent,
        executable,
        limits,
    } = &metadata.backend
    else {
        return Err(LocalAgentError::InvalidConfiguration);
    };
    let kind = match agent {
        AiLocalAgent::Codex => LocalAgentKind::Codex,
        AiLocalAgent::ClaudeCode => LocalAgentKind::ClaudeCode,
    };
    LocalAgentConfig::new(
        kind,
        PathBuf::from(executable),
        std::env::temp_dir(),
        metadata.model,
    )?
    .with_inference_endpoint(&metadata.endpoint)
    .and_then(|config| {
        LocalAgentLimits::for_ask(
            Duration::from_secs(u64::from(limits.timeout_seconds())),
            usize::from(limits.answer_kib()) * 1024,
            usize::from(limits.output_kib()) * 1024,
        )
        .map(|limits| config.with_limits(limits))
    })
}

fn local_limit_draft_error() -> Message {
    Message::new(
        "本地限制需为整数：时限 1–300 秒、回答 1–1024 KiB、累计输出 1–8192 KiB，且输出至少覆盖回答。请检查各配置。",
        "Local limits require whole numbers: 1–300 seconds, 1–1024 KiB answer, 1–8192 KiB combined output, with output at least the answer budget. Check each profile.",
    )
}

/// Translate typed local failures without displaying paths, stderr or context.
pub(crate) fn local_agent_error(error: LocalAgentError) -> Message {
    if let LocalAgentError::Context(error) = &error {
        return provider_error(error);
    }
    let (zh, en) = match error {
        LocalAgentError::InvalidConfiguration | LocalAgentError::InvalidLimits => (
            "CLI 配置无效；请检查绝对路径、模型和受支持的选项。",
            "Invalid CLI configuration; check the absolute path, model and supported options.",
        ),
        LocalAgentError::InvalidEndpoint => (
            "推理服务地址无效；仅支持无凭据的 HTTPS 或回环 HTTP。",
            "Invalid inference URL; use HTTPS or loopback HTTP without URL credentials.",
        ),
        LocalAgentError::UnsupportedExecutable => (
            "请选择原生 CLI 可执行文件，不能使用 .cmd 或 .bat 启动器。",
            "Select a native CLI executable, not a .cmd or .bat launcher.",
        ),
        LocalAgentError::SpawnFailed => (
            "无法启动 CLI；请检查路径和执行权限。",
            "Cannot start CLI; check its path and executable permissions.",
        ),
        LocalAgentError::UnsupportedVersion | LocalAgentError::IsolationUnsupported => (
            "该 CLI 版本或能力尚未通过兼容核对，请使用已验证版本。",
            "CLI version or capabilities are outside the checked compatibility set. Use a verified version.",
        ),
        LocalAgentError::MissingCredential => (
            "请填写或解锁显式 API 密钥；本切片不复用 CLI 订阅登录。",
            "Enter or unlock an explicit API key; CLI subscription logins are not reused by this adapter.",
        ),
        LocalAgentError::CredentialInContext => (
            "密钥格式无效或仍出现在审核上下文中；请重新脱敏。",
            "Credential is invalid or still appears in reviewed context. Redact and review again.",
        ),
        LocalAgentError::ReviewExpired => (
            "CLI 审核已过期，请重新审阅后发送。",
            "CLI review expired. Prepare and review again.",
        ),
        LocalAgentError::Timeout => (
            "CLI 本地超时；推理服务可能已开始处理。",
            "CLI timed out locally; inference may already have begun.",
        ),
        LocalAgentError::Cancelled => (
            "CLI 本地调用已取消；推理服务可能已开始处理。",
            "Local CLI invocation cancelled; inference may already have begun.",
        ),
        LocalAgentError::CleanupFailed | LocalAgentError::ScratchFailed => (
            "CLI 进程或临时数据清理未能确认。",
            "CLI process or temporary-data cleanup could not be confirmed.",
        ),
        LocalAgentError::OutputTooLarge => {
            ("CLI 输出超过有界限制。", "CLI output exceeded its bound.")
        }
        LocalAgentError::UnexpectedOperation => (
            "CLI 尝试了问答范围之外的工具或 Hook，已拒绝。",
            "CLI attempted a tool or hook outside Ask scope and was rejected.",
        ),
        LocalAgentError::InvalidProtocol | LocalAgentError::EmptyReply => (
            "CLI 未返回完整的受支持文本回复。",
            "CLI did not return a complete supported textual reply.",
        ),
        LocalAgentError::InferenceFailed | LocalAgentError::ProcessFailed => (
            "CLI 或推理服务失败；未自动重试。",
            "CLI or inference failed; no automatic retry.",
        ),
        _ => (
            "CLI 通信失败；未自动重试。",
            "CLI communication failed; no automatic retry.",
        ),
    };
    Message::new(zh, en)
}

fn parse_token_input(value: &str, maximum: u32) -> Result<Option<u32>, ()> {
    if value.is_empty() {
        return Ok(None);
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0 && *value <= maximum)
        .map(Some)
        .ok_or(())
}

fn replace_protocol_suffixes(endpoint: &str, old: &[&str], new: &str) -> String {
    old.iter()
        .find_map(|suffix| endpoint.strip_suffix(suffix))
        .map_or_else(|| endpoint.to_owned(), |prefix| format!("{prefix}{new}"))
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
