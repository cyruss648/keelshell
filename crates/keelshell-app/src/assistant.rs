//! Explicit-context AI panel. It can propose text, never run a terminal command.
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, IconName,
        button::{Button, ButtonVariants},
        input::{InputEvent, Textarea, TextareaState},
        label::Label,
        scroll::{Scrollbar, ScrollbarMode},
    },
    *,
};
use keelshell_ai::{
    AiError, CommandProposal, ContextDraft, DiagnosticPlan, DiagnosticRisk, LocalAgentClient,
    LocalAgentCredential, LocalAgentError, LocalAskProgress, LocalAskStage, PreparedLocalAsk,
    PreparedRequest, ProviderClient, ProviderConfig, ProviderProtocol, RedactionReport,
    RequestCancellation,
};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiProfileCatalog, KnowledgeDocument, KnowledgeError,
    KnowledgeExclusionReason, KnowledgeSaveBlock, KnowledgeSelection, KnowledgeService,
    KnowledgeSourceKind, MarkdownArtifact, NamedAiProfile, SaveDisposition, SelectedFact,
    SourceBinding,
};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

mod agent;
#[path = "assistant/command_review_target.rs"]
mod command_review_target;

use crate::{
    ai_settings::{
        EphemeralCredentials, local_agent_config, local_agent_error, provider_error, uses_api_key,
    },
    i18n::{Message, t},
};

pub enum AssistantEvent {
    Capture {
        selection_only: bool,
        previous_run: Option<Uuid>,
    },
    OpenSettings,
    AgentStart {
        run_id: Uuid,
        session_id: String,
    },
    AgentPrepareFile {
        run_id: Uuid,
        action_id: Uuid,
        action: keelshell_ai::AgentAction,
    },
    AgentExecute {
        run_id: Uuid,
        action_id: Uuid,
        action: keelshell_ai::AgentAction,
    },
    AgentStop {
        run_id: Uuid,
    },
    AgentProposed {
        run_id: Uuid,
        action_id: Uuid,
        action: keelshell_ai::AgentAction,
    },
    AgentActionRejected {
        run_id: Uuid,
        action_id: Uuid,
    },
    SelectProfile(Uuid),
    Suggestion {
        command: String,
        session_id: String,
    },
}

/// The scope of the terminal snapshot captured by an explicit user action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnowledgeCaptureKind {
    /// Text selected in the terminal emulator.
    Selection,
    /// The currently visible terminal screen.
    VisibleScreen,
}

/// Immutable metadata captured at the same explicit action as the terminal text.
pub(crate) struct KnowledgeCaptureSnapshot {
    pub(crate) text: String,
    pub(crate) host: String,
    pub(crate) session_id: String,
    pub(crate) source_id: Uuid,
    pub(crate) revision: u64,
    pub(crate) captured_at: u64,
    pub(crate) kind: KnowledgeCaptureKind,
}

impl KnowledgeCaptureKind {
    fn title(self) -> &'static str {
        match self {
            Self::Selection => "Explicit terminal selection",
            Self::VisibleScreen => "Visible terminal snapshot",
        }
    }

    fn label(self, cx: &App) -> &'static str {
        match self {
            Self::Selection => t(cx, "明确选择文本", "Explicit selection"),
            Self::VisibleScreen => t(cx, "可见终端快照", "Visible terminal snapshot"),
        }
    }
}

#[derive(Debug, Clone)]
struct KnowledgeCapture {
    source_id: Uuid,
    session_id: String,
    source_label: String,
    revision: u64,
    captured_at: u64,
    kind: KnowledgeCaptureKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnowledgeReviewOutcome {
    Unknown,
    Completed,
    Failed,
}

enum PreparedAssistantRequest {
    Api(PreparedRequest),
    Local(PreparedLocalAsk),
}

struct LocalRequestProgress {
    revision: u64,
    target: (String, String),
    facts: Vec<LocalAskStage>,
    outcome: Option<bool>,
    expanded: bool,
}

struct KnowledgePreview {
    document: KnowledgeDocument,
    markdown: MarkdownArtifact,
    review_outcome: KnowledgeReviewOutcome,
}

impl LocalRequestProgress {
    fn new(revision: u64, target: (String, String)) -> Self {
        Self {
            revision,
            target,
            facts: Vec::with_capacity(8),
            outcome: None,
            expanded: false,
        }
    }

    fn headline(&self) -> Message {
        match self.outcome {
            Some(true) => Message::new(
                "回复已验证，清理已完成",
                "Reply verified; cleanup completed",
            ),
            Some(false) => Message::new(
                "请求未成功，未采用候选回复",
                "Request failed; candidate reply not accepted",
            ),
            None if self.facts.contains(&LocalAskStage::Finalizing) => {
                stage_message(LocalAskStage::Finalizing)
            }
            None => self
                .facts
                .last()
                .copied()
                .map(stage_message)
                .unwrap_or_else(|| Message::new("准备本地请求…", "Preparing local request…")),
        }
    }
}

fn stage_message(stage: LocalAskStage) -> Message {
    match stage {
        LocalAskStage::WorkspaceReady => {
            Message::new("运行工作目录已就绪", "Execution working directory ready")
        }
        LocalAskStage::CheckingCli => {
            Message::new("正在检查版本与能力", "Checking version and capabilities")
        }
        LocalAskStage::CliAdmitted => {
            Message::new("版本与能力已准入", "Version and capabilities admitted")
        }
        LocalAskStage::ProcessStarted => Message::new("Ask 进程已启动", "Ask process started"),
        LocalAskStage::InputDelivered => {
            Message::new("审核输入已写入并关闭", "Reviewed stdin written and closed")
        }
        LocalAskStage::ProtocolStarted => {
            Message::new("协议开始回执已验证", "Protocol start receipt validated")
        }
        LocalAskStage::ProtocolCompleted => Message::new(
            "协议结束回执已验证，仍待最终检查",
            "Protocol end receipt validated; final checks pending",
        ),
        LocalAskStage::Finalizing => Message::new(
            "正在收尾，尚未确认清理完成",
            "Finalizing; cleanup not yet confirmed",
        ),
    }
}

impl PreparedAssistantRequest {
    fn preview_json(&self) -> &str {
        match self {
            Self::Api(request) => request.preview_json(),
            Self::Local(request) => request.preview_json(),
        }
    }

    fn destination(&self) -> &str {
        match self {
            Self::Api(request) => request.provider().endpoint(),
            Self::Local(request) => request.config().inference_endpoint(),
        }
    }

    fn request_options_summary(&self, profile: Option<&NamedAiProfile>, cx: &App) -> String {
        let Self::Api(request) = self else {
            return String::new();
        };
        let options = request.provider().request_options();
        let route = options.proxy_url().unwrap_or(t(
            cx,
            "直连（忽略环境代理）",
            "Direct (ignore environment proxies)",
        ));
        let names = profile
            .map(|p| {
                p.custom_headers
                    .iter()
                    .map(|h| {
                        let source = match &h.value_ref {
                            keelshell_core::AiSecretRef::Ephemeral { .. } => {
                                t(cx, "临时值", "Temporary")
                            }
                            keelshell_core::AiSecretRef::Environment { .. } => {
                                t(cx, "环境引用", "Environment")
                            }
                            keelshell_core::AiSecretRef::SecretStore { .. } => {
                                t(cx, "凭据库引用", "Vault reference")
                            }
                        };
                        format!("{} [{source}]", h.name)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        options.redact_for_review(&format!(
            "{}: {route} · {}: {} · {}: {names}",
            t(cx, "路由", "Route"),
            t(cx, "代理认证", "Proxy authentication"),
            if options.proxy_authenticated() {
                t(cx, "已配置", "Configured")
            } else {
                t(cx, "无", "None")
            },
            t(cx, "自定义请求头", "Custom headers")
        ))
    }

    fn redaction_report(&self) -> RedactionReport {
        match self {
            Self::Api(request) => request.redaction_report(),
            Self::Local(request) => request.redaction_report(),
        }
    }
}

pub struct AssistantPanel {
    profiles: AiProfileCatalog,
    credentials: EphemeralCredentials,
    profile: Option<NamedAiProfile>,
    key: Option<Zeroizing<String>>,
    selecting_profile: bool,
    runtime: Arc<Runtime>,
    cancellation: Option<RequestCancellation>,
    _job: Option<Task<()>>,
    prompt: Entity<TextareaState>,
    content_scroll: ScrollHandle,
    question_anchor: ScrollAnchor,
    preview_scroll: ScrollHandle,
    context: String,
    host: String,
    session_id: String,
    knowledge_capture: Option<KnowledgeCapture>,
    prepared: Option<PreparedAssistantRequest>,
    prepared_key: Option<Zeroizing<String>>,
    response: String,
    status: Message,
    busy: bool,
    preview: bool,
    suggestions: Vec<String>,
    diagnostic_plan: Option<DiagnosticPlan>,
    request_revision: u64,
    response_target: Option<(String, String)>,
    local_progress: Option<LocalRequestProgress>,
    knowledge_preview: Option<KnowledgePreview>,
    agent: agent::AgentPanelState,
    _subscriptions: Vec<Subscription>,
}

impl AssistantPanel {
    /// Open the assistant without collecting context or contacting a provider.
    pub fn new(
        profiles: &AiProfileCatalog,
        credentials: &EphemeralCredentials,
        runtime: Arc<Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let profile = profiles.active().cloned();
        let key = profile
            .as_ref()
            .and_then(|profile| credentials.get(&profile.id))
            .cloned();
        let prompt = cx.new(|cx| {
            TextareaState::new(window, cx).rows(3).placeholder(t(
                cx,
                "描述你想了解或排查的问题",
                "What would you like to understand?",
            ))
        });
        let content_scroll = ScrollHandle::new();
        let question_anchor = ScrollAnchor::for_handle(content_scroll.clone());
        let subscriptions = vec![cx.subscribe(&prompt, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && !view.obsolete_agent_question_change(cx) {
                view.invalidate_request(cx);
            }
        })];
        Self {
            profiles: profiles.clone(),
            credentials: credentials.clone(),
            profile,
            key,
            selecting_profile: false,
            runtime,
            cancellation: None,
            _job: None,
            prompt,
            content_scroll,
            question_anchor,
            preview_scroll: ScrollHandle::new(),
            context: String::new(),
            host: String::new(),
            session_id: String::new(),
            knowledge_capture: None,
            prepared: None,
            prepared_key: None,
            response: String::new(),
            status: Message::new(
                "请主动选择上下文，应用不会自动发送任何内容。",
                "Choose context explicitly. Nothing is sent automatically.",
            ),
            busy: false,
            preview: false,
            suggestions: Vec::new(),
            diagnostic_plan: None,
            request_revision: 0,
            response_target: None,
            local_progress: None,
            knowledge_preview: None,
            agent: agent::AgentPanelState::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Replace persisted choices after a successful settings write, preserving a
    /// still-existing temporary selection instead of forcing the default profile.
    pub fn set_profiles(
        &mut self,
        profiles: &AiProfileCatalog,
        credentials: &EphemeralCredentials,
        cx: &mut Context<Self>,
    ) {
        let selected = self
            .profile
            .as_ref()
            .and_then(|selected| profiles.profiles.iter().find(|p| p.id == selected.id))
            .or_else(|| profiles.active())
            .cloned();
        let key = selected
            .as_ref()
            .and_then(|p| credentials.get(&p.id))
            .cloned();
        if self.credentials != *credentials {
            self.invalidate_request(cx);
        }
        self.profiles = profiles.clone();
        self.credentials = credentials.clone();
        self.set_profile(selected, key, cx);
    }

    /// Select exact metadata plus a temporary credential. Changed metadata or
    /// credentials revoke prepared requests and cancel any old HTTP operation;
    /// the selected host, terminal context and question are never retargeted.
    pub fn set_profile(
        &mut self,
        profile: Option<NamedAiProfile>,
        key: Option<Zeroizing<String>>,
        cx: &mut Context<Self>,
    ) {
        if self.profile != profile || self.key != key {
            self.invalidate_request(cx);
            self.profile = profile;
            self.key = key;
        }
        self.selecting_profile = false;
        cx.notify();
    }

    fn select_profile(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(profile) = self.profiles.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        self.set_profile(Some(profile), self.credentials.get(&id).cloned(), cx);
        cx.emit(AssistantEvent::SelectProfile(id));
    }

    /// Translate only the prompt hint; the approved payload is immutable.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = t(
            cx,
            "描述你想了解或排查的问题",
            "What would you like to understand?",
        );
        self.prompt.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn captured_context_for_test(&self) -> (&str, &str, &str) {
        (&self.context, &self.host, &self.session_id)
    }

    pub fn set_context(
        &mut self,
        text: String,
        host: String,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        let same_session = self
            .knowledge_capture
            .as_ref()
            .is_some_and(|capture| capture.session_id == session_id);
        let (source_id, revision) = self
            .knowledge_capture
            .as_ref()
            .filter(|_| same_session)
            .map_or((Uuid::new_v4(), 1), |capture| {
                (capture.source_id, capture.revision.saturating_add(1))
            });
        self.set_context_snapshot(
            KnowledgeCaptureSnapshot {
                text,
                host,
                session_id,
                source_id,
                revision,
                captured_at: current_unix_timestamp(),
                kind: KnowledgeCaptureKind::Selection,
            },
            cx,
        );
    }

    pub(crate) fn set_context_snapshot(
        &mut self,
        snapshot: KnowledgeCaptureSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_request(cx);
        self.context = snapshot.text;
        self.host = snapshot.host;
        self.session_id = snapshot.session_id.clone();
        self.knowledge_capture = Some(KnowledgeCapture {
            source_id: snapshot.source_id,
            session_id: snapshot.session_id,
            source_label: format!("{} · {}", self.host, self.session_id),
            revision: snapshot.revision,
            captured_at: snapshot.captured_at,
            kind: snapshot.kind,
        });
        self.knowledge_preview = None;
        self.prepared = None;
        self.prepared_key = None;
        self.preview = false;
        self.status = Message::new(
            format!("已从 {} 选择 {} 字节", self.host, self.context.len()),
            format!("Selected {} bytes from {}", self.context.len(), self.host),
        );
        cx.notify();
    }
    /// Revoke context and reviewed output belonging to a retired SSH session.
    pub fn invalidate_session(&mut self, session_id: &str, cx: &mut Context<Self>) {
        if self.session_id != session_id
            && !self
                .response_target
                .as_ref()
                .is_some_and(|(_, id)| id == session_id)
        {
            return;
        }
        self.lose_agent_target(cx);
        self.invalidate_request(cx);
        self.context.clear();
        self.host.clear();
        self.session_id.clear();
        self.knowledge_capture = None;
        self.knowledge_preview = None;
        self.status = Message::new(
            "原 SSH 会话已结束；问题草稿已保留，请重新选择上下文并审核。",
            "The SSH session ended. Your question is preserved; select fresh context and review again.",
        );
        cx.notify();
    }

    fn build_knowledge_preview(&mut self, cx: &mut Context<Self>) {
        self.build_knowledge_preview_for(KnowledgeReviewOutcome::Unknown, cx);
    }

    fn set_knowledge_review_outcome(
        &mut self,
        outcome: KnowledgeReviewOutcome,
        cx: &mut Context<Self>,
    ) {
        self.build_knowledge_preview_for(outcome, cx);
    }

    fn build_knowledge_preview_for(
        &mut self,
        review_outcome: KnowledgeReviewOutcome,
        cx: &mut Context<Self>,
    ) {
        if self.context.trim().is_empty()
            || self.host.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.knowledge_capture.is_none()
        {
            self.knowledge_preview = None;
            self.status = Message::new(
                "请先从当前终端明确选择文本，再生成本地知识预览。",
                "Select text from the current terminal explicitly before building a local knowledge preview.",
            );
            cx.notify();
            return;
        }

        let Some(capture) = self.knowledge_capture.as_ref() else {
            self.knowledge_preview = None;
            self.status = Message::new(
                "缺少终端快照元数据，知识预览已阻止。",
                "Terminal snapshot metadata is missing; the knowledge preview is blocked.",
            );
            cx.notify();
            return;
        };
        if capture.revision == 0 || capture.captured_at == 0 {
            self.knowledge_preview = None;
            self.status = Message::new(
                "终端快照缺少有效的捕获时间或 revision，知识预览已阻止。",
                "The terminal snapshot has no valid capture time or revision; the knowledge preview is blocked.",
            );
            cx.notify();
            return;
        }

        // Terminal snapshots are multiline UI text. The domain boundary accepts
        // a single reviewed fact summary, so normalize whitespace before calling
        // it; this keeps the source explicit while retaining the domain limits.
        let summary = self
            .context
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let source = match SourceBinding::new(
            KnowledgeSourceKind::Session,
            capture.source_id,
            capture.revision,
            &capture.source_label,
        ) {
            Ok(source) => source,
            Err(error) => {
                self.knowledge_preview = None;
                self.status = knowledge_error_message(error);
                cx.notify();
                return;
            }
        };
        let fact_result = match review_outcome {
            KnowledgeReviewOutcome::Unknown => {
                SelectedFact::unknown(source, capture.captured_at, capture.kind.title())
            }
            KnowledgeReviewOutcome::Completed => {
                SelectedFact::completed(source, capture.captured_at, capture.kind.title(), &summary)
            }
            KnowledgeReviewOutcome::Failed => {
                SelectedFact::failed(source, capture.captured_at, capture.kind.title(), &summary)
            }
        };
        let fact = match fact_result {
            Ok(fact) => fact,
            Err(error) => {
                self.knowledge_preview = None;
                self.status = knowledge_error_message(error);
                cx.notify();
                return;
            }
        };
        let selection = match KnowledgeSelection::new(vec![fact]) {
            Ok(selection) => selection,
            Err(error) => {
                self.knowledge_preview = None;
                self.status = knowledge_error_message(error);
                cx.notify();
                return;
            }
        };
        let document = match KnowledgeService::build(&selection) {
            Ok(document) => document,
            Err(error) => {
                self.knowledge_preview = None;
                self.status = knowledge_error_message(error);
                cx.notify();
                return;
            }
        };
        let markdown = match KnowledgeService::render_markdown(&document) {
            Ok(markdown) => markdown,
            Err(error) => {
                self.knowledge_preview = None;
                self.status = knowledge_error_message(error);
                cx.notify();
                return;
            }
        };
        self.status = match document.disposition() {
            SaveDisposition::Persistable => Message::new(
                "本地知识预览已生成；保存和发送仍未接入。",
                "Local knowledge preview generated; saving and sending are not connected.",
            ),
            SaveDisposition::DoNotPersist { .. } => Message::new(
                "结果尚未被明确标记为已完成或已失败；知识条目保存/发送已阻止。",
                "The outcome is not explicitly marked completed or failed; saving/sending this knowledge artifact is blocked.",
            ),
        };
        self.knowledge_preview = Some(KnowledgePreview {
            document,
            markdown,
            review_outcome,
        });
        cx.notify();
    }

    #[cfg(test)]
    fn knowledge_preview_for_test(&self) -> Option<(&KnowledgeDocument, &MarkdownArtifact)> {
        self.knowledge_preview
            .as_ref()
            .map(|preview| (&preview.document, &preview.markdown))
    }

    /// Whether configuration replacement can proceed without cancelling active inference.
    pub(crate) fn config_recovery_idle(&self) -> bool {
        !self.busy && !self.agent_active()
    }

    fn invalidate_request(&mut self, cx: &mut Context<Self>) {
        self.stop_agent(cx);
        self.request_revision = self.request_revision.wrapping_add(1);
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self._job = None;
        self.busy = false;
        self.prepared = None;
        self.prepared_key = None;
        self.preview = false;
        self.preview_scroll.set_offset(point(px(0.), px(0.)));
        self.response.clear();
        self.suggestions.clear();
        self.diagnostic_plan = None;
        self.response_target = None;
        self.local_progress = None;
        self.status = Message::new(
            "请求已修改，请重新预览后再发送；已请求取消旧请求。",
            "Request changed. Preview again before sending; cancellation of the old request was requested.",
        );
        cx.notify();
    }

    fn prepare(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            self.status = Message::new(
                "请先在 AI 设置中添加并选择配置。",
                "Add and select a profile in AI settings first.",
            );
            cx.notify();
            return;
        };
        let validation = match profile.backend {
            AiBackend::Api => profile.validate_current_transport(),
            AiBackend::LocalAgent { .. } => profile.validate_local_agent_transport(),
        };
        if let Err(error) = validation {
            self.status = Message::detail(
                "该配置当前不可用",
                "This configuration is not currently usable",
                error,
            );
            cx.notify();
            return;
        }
        let resolved_key = match profile.backend {
            AiBackend::Api => {
                let mut credentials = self.credentials.clone();
                if let Some(key) = &self.key {
                    credentials.insert(profile.id, key.clone());
                }
                crate::ai_request_options::resolve_authentication(&profile, &credentials)
            }
            AiBackend::LocalAgent { .. } => {
                if matches!(
                    &profile.authentication,
                    AiAuthentication::Bearer {
                        credential: Some(keelshell_core::AiSecretRef::Environment { .. })
                    } | AiAuthentication::Header {
                        credential: Some(keelshell_core::AiSecretRef::Environment { .. }),
                        ..
                    }
                ) {
                    self.credentials
                        .local_environment_key(&profile)
                        .cloned()
                        .map(Some)
                        .ok_or(keelshell_ai::AiError::InvalidApiKey)
                } else {
                    Ok(self.key.clone())
                }
            }
        };
        let resolved_key = match resolved_key {
            Ok(key) => key,
            Err(error) => {
                self.status = ai_error(&error);
                cx.notify();
                return;
            }
        };
        if uses_api_key(&profile.authentication)
            && resolved_key.as_ref().is_none_or(|key| key.is_empty())
        {
            self.status = Message::new(
                "此配置需要 API 密钥，请在 AI 设置中填写或解锁，然后应用。",
                "This profile needs an API key. Enter or unlock it in AI settings, then apply.",
            );
            cx.notify();
            return;
        }
        let protocol = match profile.api_style {
            AiApiStyle::ChatCompletions => ProviderProtocol::ChatCompletions,
            AiApiStyle::Responses => ProviderProtocol::Responses,
            AiApiStyle::AnthropicMessages => ProviderProtocol::AnthropicMessages,
        };
        let Some(question) = self.prepare_agent_question(cx) else {
            return;
        };
        // Every configured transient credential participates in redaction, even
        // when it belongs to a different profile or invocation adapter.
        let secrets: Vec<&str> = self
            .credentials
            .all_secrets()
            .into_iter()
            .chain(resolved_key.as_ref().map(|key| key.as_str()))
            .collect();
        let context = ContextDraft::new(question)
            .with_host_label(self.host.clone())
            .add_selection(
                t(
                    cx,
                    "用户选择的远端终端文本",
                    "Explicitly selected remote terminal text",
                ),
                if self.agent.mode {
                    String::new()
                } else {
                    self.context.clone()
                },
            );
        let result = match profile.backend {
            AiBackend::Api => {
                crate::ai_request_options::resolve_options(&profile, &self.credentials)
                    .and_then(|options| {
                        ProviderConfig::new_with_protocol(
                            &profile.endpoint,
                            &profile.model,
                            protocol,
                        )
                        .and_then(|provider| {
                            provider
                                .with_request_options(options)
                                .with_inference_options(
                                    crate::ai_request_options::inference_options(&profile)?,
                                )
                        })
                    })
                    .and_then(|provider| {
                        context.prepare_with_limits(
                            &provider,
                            &secrets,
                            if self.agent.mode {
                                64 * 1024
                            } else {
                                16 * 1024
                            },
                            profile.max_output_tokens,
                            profile.context_window_tokens,
                        )
                    })
                    .map(PreparedAssistantRequest::Api)
                    .map_err(|error| ai_error(&error))
            }
            AiBackend::LocalAgent { .. } => {
                let config = match local_agent_config(&profile, false) {
                    Ok(config) => config,
                    Err(error) => {
                        self.status = local_agent_error(error);
                        cx.notify();
                        return;
                    }
                };
                if matches!(
                    config.working_directory(),
                    keelshell_ai::LocalAgentWorkingDirectory::Selected(_)
                ) {
                    let byte_budget = if self.agent.mode {
                        64 * 1024
                    } else {
                        16 * 1024
                    };
                    let owned_secrets: Vec<Zeroizing<String>> = secrets
                        .iter()
                        .map(|value| Zeroizing::new((*value).to_owned()))
                        .collect();
                    let expected_profile = profile.clone();
                    let revision = self.request_revision;
                    let target = (self.host.clone(), self.session_id.clone());
                    let cancellation = RequestCancellation::new();
                    self.cancellation = Some(cancellation.clone());
                    self.prepared = None;
                    self.prepared_key = None;
                    self.preview = false;
                    self.busy = true;
                    self.status = Message::new(
                        "后台校验所选工作目录；完成后请审核完整目录与 SSH 片段。",
                        "Validating the selected working directory in the background; then review the full directory and SSH selection.",
                    );
                    let job = crate::runtime_bridge::spawn(
                        &self.runtime,
                        cx.background_executor().clone(),
                        async move {
                            let secrets: Vec<&str> =
                                owned_secrets.iter().map(|value| value.as_str()).collect();
                            config
                                .prepare_checked(context, &secrets, byte_budget, &cancellation)
                                .await
                                .map(PreparedAssistantRequest::Local)
                                .map_err(local_agent_error)
                        },
                    );
                    self._job = Some(cx.spawn(async move |this, cx| {
                        let result = job.await.unwrap_or_else(|_| {
                            Err(local_agent_error(LocalAgentError::DirectoryUnavailable))
                        });
                        let _ = this.update(cx, |panel, cx| {
                            if panel.request_revision != revision
                                || panel.profile.as_ref() != Some(&expected_profile)
                                || (panel.host.clone(), panel.session_id.clone()) != target
                            {
                                return;
                            }
                            panel.busy = false;
                            panel.cancellation = None;
                            panel.accept_prepared(result, resolved_key, cx);
                        });
                    }));
                    cx.notify();
                    return;
                }
                config
                    .prepare(
                        context,
                        &secrets,
                        if self.agent.mode {
                            64 * 1024
                        } else {
                            16 * 1024
                        },
                    )
                    .map(PreparedAssistantRequest::Local)
                    .map_err(local_agent_error)
            }
        };
        self.accept_prepared(result, resolved_key, cx);
    }

    fn accept_prepared(
        &mut self,
        result: Result<PreparedAssistantRequest, Message>,
        resolved_key: Option<Zeroizing<String>>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(request) => {
                let report = request.redaction_report();
                self.status = Message::new(
                    format!(
                        "已脱敏 {} 处 · 已省略 {} 字节 · 请核对下方完整请求",
                        report.total_redactions(),
                        report.truncated_bytes
                    ),
                    format!(
                        "{} redactions · {} bytes omitted · inspect exact request below",
                        report.total_redactions(),
                        report.truncated_bytes
                    ),
                );
                self.prepared = Some(request);
                self.prepared_key = resolved_key;
                self.preview = true;
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(request) = self.prepared.take() else {
            return;
        };
        if !self.begin_agent_round(cx) {
            self.prepared = Some(request);
            return;
        }
        let key = self.prepared_key.take();
        let revision = self.request_revision;
        let response_target = (self.host.clone(), self.session_id.clone());
        let cancellation = RequestCancellation::new();
        self.cancellation = Some(cancellation.clone());
        self.busy = true;
        self.status = Message::new("正在等待模型服务回复…", "Waiting for provider…");
        self.response.clear();
        self.suggestions.clear();
        self.diagnostic_plan = None;
        self.preview = false;
        self.local_progress = None;
        match request {
            PreparedAssistantRequest::Api(request) => {
                let job = crate::runtime_bridge::spawn(
                    &self.runtime,
                    cx.background_executor().clone(),
                    async move {
                        let client = ProviderClient::new_with_options(
                            Duration::from_secs(60),
                            1024 * 1024,
                            request.provider().request_options().clone(),
                        )
                        .map_err(|error| ai_error(&error))?;
                        client
                            .send_approved(
                                request.approve(),
                                key.as_ref().map(|key| key.as_str()),
                                &cancellation,
                            )
                            .await
                            .map(|reply| reply.into_text())
                            .map_err(|error| ai_error(&error))
                    },
                );
                self._job = Some(cx.spawn(async move |this, cx| {
                    let result = job
                        .await
                        .unwrap_or_else(|_| Err(local_agent_error(LocalAgentError::PipeFailed)));
                    let _ = this.update(cx, |panel, cx| {
                        panel.finish_reply(revision, response_target, result, cx)
                    });
                }));
            }
            PreparedAssistantRequest::Local(request) => {
                self.local_progress =
                    Some(LocalRequestProgress::new(revision, response_target.clone()));
                let (progress, receiver) = LocalAskProgress::channel();
                let job = crate::runtime_bridge::spawn_local_ask(
                    &self.runtime,
                    cx.background_executor().clone(),
                    receiver,
                    async move {
                        let credential =
                            LocalAgentCredential::new(key.as_ref().map_or("", |key| key.as_str()))
                                .map_err(local_agent_error)?;
                        LocalAgentClient
                            .ask_with_progress(
                                request.approve(),
                                credential,
                                &cancellation,
                                progress,
                            )
                            .await
                            .map(|reply| reply.text().to_owned())
                            .map_err(local_agent_error)
                    },
                );
                self._job = Some(cx.spawn(async move |this, cx| {
                    loop {
                        match job.next().await {
                            Ok(crate::runtime_bridge::LocalAskEvent::Stage(stage)) => {
                                if this
                                    .update(cx, |panel, cx| {
                                        panel.observe_local_stage(
                                            revision,
                                            &response_target,
                                            stage,
                                            cx,
                                        )
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            event => {
                                let result = match event {
                                    Ok(crate::runtime_bridge::LocalAskEvent::Complete(result)) => {
                                        result
                                    }
                                    _ => Err(local_agent_error(LocalAgentError::PipeFailed)),
                                };
                                let _ = this.update(cx, |panel, cx| {
                                    panel.finish_reply(revision, response_target, result, cx)
                                });
                                break;
                            }
                        }
                    }
                }));
            }
        }
        cx.notify();
    }

    fn observe_local_stage(
        &mut self,
        revision: u64,
        target: &(String, String),
        stage: LocalAskStage,
        cx: &mut Context<Self>,
    ) {
        if !self.busy
            || self.request_revision != revision
            || self.host != target.0
            || self.session_id != target.1
        {
            return;
        }
        let Some(progress) = &mut self.local_progress else {
            return;
        };
        if progress.revision != revision
            || &progress.target != target
            || progress.outcome.is_some()
            || progress.facts.contains(&stage)
        {
            return;
        }
        // Only eight typed facts can exist. Never infer missing predecessors or
        // accept a supplier string as an executable Agent step.
        if progress.facts.len() < LocalAskStage::ALL.len() {
            progress.facts.push(stage);
            cx.notify();
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.invalidate_request(cx);
        self.status = Message::new(
            "已请求取消；后台正在停止并清理，服务端可能已处理发送的内容。",
            "Cancellation requested; background stopping and cleanup are pending. The provider may have processed sent content.",
        );
        cx.notify();
    }

    #[cfg(test)]
    fn finish_request(
        &mut self,
        revision: u64,
        target: (String, String),
        result: Result<String, AiError>,
        cx: &mut Context<Self>,
    ) {
        self.finish_reply(
            revision,
            target,
            result.map_err(|error| ai_error(&error)),
            cx,
        );
    }

    fn finish_reply(
        &mut self,
        revision: u64,
        target: (String, String),
        result: Result<String, Message>,
        cx: &mut Context<Self>,
    ) {
        if self.request_revision != revision || self.host != target.0 || self.session_id != target.1
        {
            return;
        }
        if let Some(progress) = &mut self.local_progress
            && progress.revision == revision
            && progress.target == target
        {
            progress.outcome = Some(result.is_ok());
        }
        self.busy = false;
        self.cancellation = None;
        match result {
            Ok(text) => {
                self.response_target = Some(target);
                // A response may reflect a credential from another configuration.
                // Revision admission precedes redaction, so obsolete results never
                // reach the UI after their owning slots have been replaced/cleared.
                self.response = keelshell_ai::Redactor::new(&self.credentials.all_secrets())
                    .redact(&text)
                    .0;
                if self.agent.mode {
                    self.accept_agent_reply(cx);
                } else {
                    self.suggestions = shell_blocks(&self.response);
                }
                self.diagnostic_plan = None;
                if !self.agent.mode {
                    self.status = Message::new(
                        "已收到回复。命令建议需先审阅，再送入输入框。",
                        "Response received. Suggestions require review before use.",
                    );
                }
            }
            Err(error) => {
                if let Some(run) = &mut self.agent.run {
                    run.fail();
                    let run_id = run.id();
                    self.revoke_agent_backend(run_id, cx);
                    cx.emit(AssistantEvent::AgentStop { run_id });
                }
                self.status = error;
            }
        }
        cx.notify();
    }

    fn suggest(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(reason) = self.review_target_unavailable() {
            self.status = reason;
            cx.notify();
            return;
        }
        let Some(command) = self.suggestions.get(index).cloned() else {
            return;
        };
        let Some((target, session_id)) = self.response_target.clone() else {
            return;
        };
        let proposal = CommandProposal {
            command,
            explanation: t(
                cx,
                "AI 生成的建议；执行前请核对全部参数与目标主机",
                "AI-generated suggestion; review all arguments and target before running",
            )
            .into(),
            risks: vec![
                t(
                    cx,
                    "模型输出不可信，可能存在错误",
                    "Model output is untrusted and may be incorrect",
                )
                .into(),
            ],
            target,
            session_id: session_id.clone(),
        };
        match proposal
            .review(Duration::from_secs(120))
            .and_then(|ticket| ticket.into_suggestion(&proposal, &session_id))
        {
            Ok(command) => cx.emit(AssistantEvent::Suggestion {
                command,
                session_id,
            }),
            Err(error) => self.status = ai_error(&error),
        }
        cx.notify();
    }

    fn build_diagnostic_plan(&mut self, cx: &mut Context<Self>) {
        if self.response.is_empty() {
            return;
        }
        if let Some(reason) = self.review_target_unavailable() {
            self.status = reason;
            cx.notify();
            return;
        }
        let Some((target, session_id)) = self.response_target.clone() else {
            return;
        };
        match DiagnosticPlan::from_response(target, session_id, &self.context, &self.response) {
            Ok(plan) => {
                let count = plan.steps().len();
                self.status = Message::new(
                    format!("已生成 {count} 步诊断计划；每一步仍需单独审核。"),
                    format!("Generated {count}-step diagnostic plan; review each step separately."),
                );
                self.diagnostic_plan = Some(plan);
            }
            Err(error) => self.status = ai_error(&error),
        }
        cx.notify();
    }

    fn suggest_diagnostic_step(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(reason) = self.review_target_unavailable() {
            self.status = reason;
            cx.notify();
            return;
        }
        let Some((target, session_id)) = self.response_target.as_ref() else {
            return;
        };
        let Some(plan) = self.diagnostic_plan.as_ref() else {
            return;
        };
        if plan.target() != target || plan.session_id() != session_id {
            self.status = ai_error(&AiError::DiagnosticPlanMismatch);
            cx.notify();
            return;
        }
        let Ok(review) = plan.review_step(index, Duration::from_secs(120)) else {
            self.status = ai_error(&AiError::InvalidDiagnosticPlan);
            cx.notify();
            return;
        };
        let proposal = match review.into_proposal(plan, session_id) {
            Ok(proposal) => proposal,
            Err(error) => {
                self.status = ai_error(&error);
                cx.notify();
                return;
            }
        };
        match proposal
            .review(Duration::from_secs(120))
            .and_then(|ticket| ticket.into_suggestion(&proposal, session_id))
        {
            Ok(command) => cx.emit(AssistantEvent::Suggestion {
                command,
                session_id: session_id.clone(),
            }),
            Err(error) => self.status = ai_error(&error),
        }
        cx.notify();
    }

    fn knowledge_preview_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(preview) = self.knowledge_preview.as_ref() else {
            return div().into_any_element();
        };
        let disposition = match preview.document.disposition() {
            SaveDisposition::Persistable => Message::new(
                "SaveDisposition：Persistable（仅代表通过本地内容闸门）",
                "SaveDisposition: Persistable (only the local content gate passed)",
            ),
            SaveDisposition::DoNotPersist { reason } => Message::new(
                format!(
                    "SaveDisposition：DoNotPersist · {}",
                    knowledge_save_block_label(reason, cx)
                ),
                format!(
                    "SaveDisposition: DoNotPersist · {}",
                    knowledge_save_block_label(reason, cx)
                ),
            ),
        };
        let disposition_color = if preview.document.disposition().is_persistable() {
            visual.success
        } else {
            visual.danger
        };
        let capture_kind_label = self.knowledge_capture.as_ref().map_or_else(
            || t(cx, "终端快照", "Terminal snapshot"),
            |capture| capture.kind.label(cx),
        );
        let review_label = match preview.review_outcome {
            KnowledgeReviewOutcome::Unknown => t(
                cx,
                "结果状态：未知（仅审核）",
                "Outcome: unknown (review only)",
            ),
            KnowledgeReviewOutcome::Completed => {
                t(cx, "结果状态：已确认完成", "Outcome: confirmed completed")
            }
            KnowledgeReviewOutcome::Failed => {
                t(cx, "结果状态：已确认失败", "Outcome: confirmed failed")
            }
        };
        let mut sources = div()
            .id("knowledge-preview-sources")
            .test_support()
            .flex()
            .flex_col()
            .gap_1();
        for (index, entry) in preview.document.entries().iter().enumerate() {
            sources = sources.child(
                div()
                    .id(("knowledge-entry", index))
                    .test_support()
                    .text_xs()
                    .child(format!(
                        "{} · {} · rev {} · captured {} · {}",
                        entry.title(),
                        entry.source().label(),
                        entry.source().revision(),
                        entry.occurred_at(),
                        capture_kind_label,
                    )),
            );
        }
        for (index, exclusion) in preview.document.exclusions().iter().enumerate() {
            sources = sources.child(
                div()
                    .id(("knowledge-exclusion", index))
                    .test_support()
                    .text_xs()
                    .text_color(rgb(visual.danger))
                    .child(format!(
                        "{} · {} · rev {} · captured {} · {}",
                        knowledge_exclusion_label(exclusion.reason(), cx),
                        exclusion.source().label(),
                        exclusion.source().revision(),
                        exclusion.occurred_at(),
                        capture_kind_label,
                    )),
            );
        }
        let save_label = if preview.document.disposition().is_persistable() {
            Message::new("保存（未接入，OPEN）", "Save (not connected, OPEN)")
        } else {
            Message::new("保存已阻止", "Saving blocked")
        };
        div()
            .id("knowledge-preview")
            .test_support()
            .p_2()
            .rounded(px(6.))
            .bg(rgb(visual.canvas))
            .border_1()
            .border_color(rgb(visual.border))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(cx, "本地知识预览 / 审核", "Local knowledge preview / review")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "来源来自一次明确捕获的终端快照；选择类型、session identity、捕获时间和 revision 均绑定在预览中。不会从 WorkflowAuditRecord 自动导入。",
                        "The source is one explicitly captured terminal snapshot; its capture kind, session identity, capture time and revision stay bound to this preview. WorkflowAuditRecord is never imported automatically.",
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(review_label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "知识条目只有在你明确确认结果状态后才可通过本地内容闸门。",
                        "A knowledge entry passes the local content gate only after you explicitly confirm its outcome.",
                    )),
            )
            .child(
                div()
                    .id("knowledge-preview-disposition")
                    .test_support()
                    .text_xs()
                    .text_color(rgb(disposition_color))
                    .child(disposition.render(cx)),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(cx, "来源、条目与排除项", "Sources, entries and exclusions")),
            )
            .child(sources)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("knowledge-confirm-completed")
                            .ghost()
                            .label(t(cx, "确认已完成", "Confirm completed"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.set_knowledge_review_outcome(
                                    KnowledgeReviewOutcome::Completed,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("knowledge-confirm-failed")
                            .ghost()
                            .label(t(cx, "确认已失败", "Confirm failed"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.set_knowledge_review_outcome(
                                    KnowledgeReviewOutcome::Failed,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(cx, "Markdown 预览", "Markdown preview")),
            )
            .child(
                div()
                    .id("knowledge-preview-markdown")
                    .test_support()
                    .max_h(px(180.))
                    .overflow_y_scroll()
                    .p_2()
                    .bg(rgb(visual.surface))
                    .font_family("monospace")
                    .child(Label::new(preview.markdown.text().to_owned()).text_xs()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "模型发送：未接入；本预览不会自动发送。",
                        "Model sending: not connected; this preview is never sent automatically.",
                    )),
            )
            .child(
                Button::new("knowledge-save-disabled")
                    .ghost()
                    .disabled(true)
                    .label(save_label.render(cx)),
            )
            .into_any_element()
    }
}
impl Drop for AssistantPanel {
    fn drop(&mut self) {
        if let Some((_, cancellation)) = &self.agent.backend {
            cancellation.cancel();
        }
        if let Some(cancellation) = &self.cancellation {
            cancellation.cancel();
        }
    }
}
impl EventEmitter<AssistantEvent> for AssistantPanel {}
impl Render for AssistantPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let profile_choices = if self.selecting_profile {
            let mut choices = div().p_2().bg(rgb(visual.canvas)).flex().flex_col().gap_1();
            for (index, profile) in self.profiles.profiles.iter().enumerate() {
                let id = profile.id;
                choices = choices.child(
                    Button::new(("assistant-profile-choice", index))
                        .ghost()
                        .label(profile.name.clone())
                        .disabled(match profile.backend {
                            AiBackend::Api => profile.validate_current_transport().is_err(),
                            AiBackend::LocalAgent { .. } => {
                                profile.validate_local_agent_transport().is_err()
                            }
                        })
                        .on_click(cx.listener(move |panel, _, _, cx| panel.select_profile(id, cx))),
                );
            }
            Some(choices)
        } else {
            None
        };
        let mut content = div()
            .id("assistant-content")
            .test_support()
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
                cx,
                "AI 助手",
                "AI assistant",
            )))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "解释远端输出、排查问题、准备命令。",
                "Explain remote output, investigate issues, and prepare commands.",
            )))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("assistant-profile")
                            .icon(IconName::ChevronDown)
                            .ghost()
                            .label(
                                self.profile
                                    .as_ref()
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| {
                                        t(cx, "选择 AI 配置", "Select AI profile").to_owned()
                                    }),
                            )
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.selecting_profile = !panel.selecting_profile;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("assistant-open-settings")
                            .icon(IconName::Settings)
                            .ghost()
                            .label(t(cx, "配置", "Settings"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(AssistantEvent::OpenSettings)),
                            ),
                    ),
            )
            .child(
                div().text_xs().text_color(rgb(visual.muted)).child(
                    self.profile
                        .as_ref()
                        .map(|p| p.model.clone())
                        .unwrap_or_else(|| {
                            t(cx, "尚未配置模型服务", "No model provider configured").to_owned()
                        }),
                ),
            )
            .children(profile_choices)
            .child(
                div()
                    .mt_1()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "上下文 · 由你主动选择",
                        "Context · explicitly selected",
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("context-selection")
                            .ghost()
                            .label(t(cx, "所选文本", "Selection"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.invalidate_request(cx);
                                cx.emit(AssistantEvent::Capture {
                                    selection_only: true,
                                    previous_run: panel
                                        .agent
                                        .run
                                        .as_ref()
                                        .map(keelshell_ai::AgentRun::id),
                                })
                            })),
                    )
                    .child(
                        Button::new("context-screen")
                            .ghost()
                            .label(t(cx, "当前屏幕", "Visible screen"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.invalidate_request(cx);
                                cx.emit(AssistantEvent::Capture {
                                    selection_only: false,
                                    previous_run: panel
                                        .agent
                                        .run
                                        .as_ref()
                                        .map(keelshell_ai::AgentRun::id),
                                })
                            })),
                    ),
            )
            .child(div().text_xs().text_color(rgb(visual.muted)).child(
                if self.context.is_empty() {
                    t(
                        cx,
                        "尚未附加远端终端内容",
                        "No remote terminal context attached",
                    )
                    .to_owned()
                } else {
                    Message::new(
                        format!("{} · {} 字节", self.host, self.context.len()),
                        format!("{} · {} bytes", self.host, self.context.len()),
                    )
                    .render(cx)
                },
            ))
            .child(
                Button::new("knowledge-preview-action")
                    .ghost()
                    .disabled(
                        self.context.trim().is_empty()
                            || self.host.trim().is_empty()
                            || self.session_id.trim().is_empty(),
                    )
                    .label(t(cx, "生成本地知识预览", "Build local knowledge preview"))
                    .on_click(cx.listener(|panel, _, _, cx| panel.build_knowledge_preview(cx))),
            )
            .when(self.knowledge_preview.is_some(), |content| {
                content.child(self.knowledge_preview_view(cx))
            })
            .child(self.agent_controls(cx))
            .child(
                div()
                    .id("assistant-question")
                    .test_support()
                    .anchor_scroll(Some(self.question_anchor.clone()))
                    .child(Textarea::new(&self.prompt).readonly(self.agent_active())),
            )
            .child(
                Button::new("prepare-request")
                    .primary()
                    .disabled(self.busy || self.profile.is_none() || self.agent_prepare_disabled())
                    .label(if self.busy {
                        t(cx, "正在请求…", "Request in progress…")
                    } else {
                        t(cx, "预览请求", "Preview request")
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.prepare(cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.accent))
                    .child(self.status.render(cx)),
            );
        if self.preview
            && let Some(request) = self.prepared.as_ref()
        {
            content = content
                .child(div().text_xs().child(
                    Message::detail("发送至", "Destination", request.destination()).render(cx),
                ))
                .child(
                    div()
                        .text_xs()
                        .child(request.request_options_summary(self.profile.as_ref(), cx)),
                )
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(224.))
                        .rounded(px(6.))
                        .bg(rgb(visual.canvas))
                        .border_1()
                        .border_color(rgb(visual.border))
                        .child(
                            div()
                                .id("assistant-request-full-preview")
                                .test_support()
                                .size_full()
                                .flex()
                                .flex_col()
                                .items_start()
                                .overflow_scroll()
                                .track_scroll(&self.preview_scroll)
                                .p_2()
                                .pr_5()
                                .pb_5()
                                .font_family("monospace")
                                .child(
                                    Label::new(request.preview_json().to_owned())
                                        .text_xs()
                                        .whitespace_nowrap()
                                        .flex_shrink_0(),
                                ),
                        )
                        .child(
                            div()
                                .id("assistant-request-preview-scrollbar")
                                .test_support()
                                .aria_label(t(
                                    cx,
                                    "完整请求预览滚动条",
                                    "Complete request preview scrollbars",
                                ))
                                .absolute()
                                .inset_0()
                                .child(
                                    Scrollbar::new(&self.preview_scroll)
                                        .id("assistant-request-preview-scrollbar-control")
                                        .mode(ScrollbarMode::Always),
                                ),
                        ),
                );
        }
        if self.agent.mode {
            content = content.child(self.agent_view(window, cx));
        }
        if !self.response.is_empty() && !self.agent.mode {
            // Provider output stays plain text and never triggers implicit requests.
            content = content
                .child(self.review_target_view("assistant-response-target".into(), cx))
                .child(
                    div()
                        .border_t_1()
                        .border_color(rgb(visual.border))
                        .pt_3()
                        .text_sm()
                        .child(self.response.clone()),
                )
                .child(
                    Button::new("copy-ai-reply")
                        .ghost()
                        .label(t(cx, "复制回复", "Copy response"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(view.response.clone()))
                        })),
                );
            content = content
                .child(self.review_target_view("diagnostic-review-target".into(), cx))
                .child(
                    Button::new("build-diagnostic-plan")
                        .ghost()
                        .disabled(self.review_target_unavailable().is_some())
                        .label(t(cx, "整理为诊断计划", "Build diagnostic plan"))
                        .on_click(cx.listener(|view, _, _, cx| view.build_diagnostic_plan(cx))),
                );
            if let Some(plan) = self.diagnostic_plan.as_ref() {
                let mut plan_view = div()
                    .p_2()
                    .rounded(px(6.))
                    .bg(rgb(visual.canvas))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "诊断计划（逐步审核）",
                        "Diagnostic plan (step-by-step review)",
                    )))
                    .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                        "{} · context {} · response {}",
                        plan.target(),
                        &plan.context_fingerprint()[..12],
                        &plan.response_fingerprint()[..12],
                    )));
                for (index, step) in plan.steps().iter().enumerate() {
                    let risk = match step.risk() {
                        DiagnosticRisk::ReadOnly => t(cx, "只读候选", "Read-only candidate"),
                        DiagnosticRisk::ReviewRequired => t(cx, "需要重点审核", "Review required"),
                    };
                    plan_view = plan_view.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xs().font_family("monospace").child(format!(
                                "{}. {}",
                                index + 1,
                                step.command()
                            )))
                            .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                                "line {} · {}",
                                step.source_line(),
                                risk
                            )))
                            .child(
                                self.review_target_view(
                                    ("diagnostic-step-target", index).into(),
                                    cx,
                                ),
                            )
                            .child(
                                Button::new(("review-diagnostic-step", index))
                                    .ghost()
                                    .disabled(self.review_target_unavailable().is_some())
                                    .label(t(cx, "送入命令审阅区", "Place in command review"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.suggest_diagnostic_step(index, cx)
                                    })),
                            ),
                    );
                }
                content = content.child(plan_view);
            }
            for (index, command) in self.suggestions.iter().enumerate() {
                content = content.child(
                    div()
                        .p_2()
                        .rounded(px(6.))
                        .bg(rgb(visual.canvas))
                        .border_1()
                        .border_color(rgb(visual.border))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_family("monospace")
                                .child(command.clone()),
                        )
                        .child(
                            self.review_target_view(("suggestion-review-target", index).into(), cx),
                        )
                        .child(
                            Button::new(("review-suggestion", index))
                                .ghost()
                                .disabled(self.review_target_unavailable().is_some())
                                .label(t(cx, "送入命令审阅区", "Place in command review"))
                                .on_click(
                                    cx.listener(move |view, _, _, cx| view.suggest(index, cx)),
                                ),
                        ),
                );
            }
        }
        let request_bar = if self.busy || self.local_progress.is_some() || self.agent_active() {
            let mut bar = div()
                .id("assistant-request-bar")
                .test_support()
                .flex_shrink_0()
                .p_3()
                .border_b_1()
                .border_color(rgb(visual.border))
                .bg(rgb(visual.canvas))
                .flex()
                .flex_col()
                .gap_2();
            if let Some(progress) = &self.local_progress {
                let color = match progress.outcome {
                    Some(true) => visual.success,
                    Some(false) => visual.danger,
                    None => visual.accent,
                };
                bar = bar
                    .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                        cx,
                        if self.agent.mode {
                            "本地推理 · 本回合请求进度"
                        } else {
                            "本地 Ask · 请求进度"
                        },
                        if self.agent.mode {
                            "Local inference · Current round progress"
                        } else {
                            "Local Ask · Request progress"
                        },
                    )))
                    .child(
                        div()
                            .id("local-ask-headline")
                            .test_support()
                            .text_xs()
                            .text_color(rgb(color))
                            .child(progress.headline().render(cx)),
                    )
                    .child(
                        Button::new("local-ask-toggle-facts")
                            .ghost()
                            .icon(IconName::ChevronDown)
                            .label(
                                Message::new(
                                    format!("事实记录 ({})", progress.facts.len()),
                                    format!("Observed facts ({})", progress.facts.len()),
                                )
                                .render(cx),
                            )
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if let Some(progress) = &mut panel.local_progress {
                                    progress.expanded = !progress.expanded;
                                }
                                cx.notify();
                            })),
                    );
                if progress.expanded {
                    bar = bar
                        .child(
                            div()
                                .id("local-ask-facts")
                                .test_support()
                                .max_h(px(144.))
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(progress.facts.iter().enumerate().map(
                                    |(index, stage)| {
                                        div()
                                            .id(("local-ask-fact", index))
                                            .test_support()
                                            .text_xs()
                                            .text_color(rgb(visual.muted))
                                            .child(stage_message(*stage).render(cx))
                                    },
                                )),
                        )
                        .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                            cx,
                            "事实可能交错；结束回执不代表请求成功。",
                            "Facts may interleave; an end receipt does not establish success.",
                        )));
                }
            } else {
                bar = bar.child(div().text_xs().text_color(rgb(visual.accent)).child(
                    if self.agent.mode {
                        self.agent_status_headline(cx)
                    } else {
                        t(cx, "正在等待模型服务回复…", "Waiting for provider…").to_owned()
                    },
                ));
            }
            if self.agent_active() {
                bar = bar
                    .child(self.agent_stop_button(cx))
                    .child(self.agent_question_controls(cx));
            }
            if self.busy && !self.agent.mode {
                bar = bar.child(
                    Button::new("assistant-cancel-request")
                        .ghost()
                        .label(t(cx, "取消请求", "Cancel request"))
                        .on_click(cx.listener(|panel, _, _, cx| panel.cancel(cx))),
                );
            }
            Some(bar)
        } else {
            None
        };
        // Confirmation remains outside the scrollable review. A long payload
        // must never move the explicit send action beyond the viewport.
        let confirmation = (self.preview && self.prepared.is_some()).then(|| {
            div()
                .id("assistant-confirmation-footer")
                .test_support()
                .flex_shrink_0()
                .p_3()
                .border_t_1()
                .border_color(rgb(visual.border))
                .child(
                    Button::new("send-approved-request")
                        .primary()
                        .disabled(self.busy)
                        .label(t(cx, "确认发送此请求", "Send this exact request"))
                        .on_click(cx.listener(|view, _, _, cx| view.send(cx))),
                )
        });
        div()
            .h_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .children(request_bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("assistant-scroll")
                            .test_support()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.content_scroll)
                            .child(content.pr(px(18.))),
                    )
                    .child(
                        div()
                            .id("assistant-scrollbar")
                            .test_support()
                            .aria_label(t(
                                cx,
                                "AI 助手正文滚动条",
                                "AI assistant content scrollbar",
                            ))
                            .absolute()
                            .inset_0()
                            .child(
                                Scrollbar::vertical(&self.content_scroll)
                                    .id("assistant-scrollbar-control")
                                    .mode(ScrollbarMode::Always)
                                    .styles(|styles| {
                                        styles
                                            .track(|style| {
                                                style.width(px(12.)).bg(rgb(visual.canvas).into())
                                            })
                                            .thumb(|style| {
                                                style.width(px(7.)).bg(rgb(visual.muted))
                                            })
                                    }),
                            ),
                    ),
            )
            .children(confirmation)
            .children(self.agent_action_footer(cx))
    }
}

fn knowledge_error_message(error: KnowledgeError) -> Message {
    Message::detail(
        "本地知识预览失败，未生成产物",
        "Local knowledge preview failed; no artifact was generated",
        error,
    )
}

fn knowledge_exclusion_label(reason: KnowledgeExclusionReason, cx: &App) -> &'static str {
    match reason {
        KnowledgeExclusionReason::UnknownOutcome => {
            t(cx, "未知结果，已排除", "Unknown outcome, excluded")
        }
        KnowledgeExclusionReason::NotCompleted => {
            t(cx, "未完成结果，已排除", "Incomplete outcome, excluded")
        }
        KnowledgeExclusionReason::SensitiveContent => {
            t(cx, "敏感内容，已排除", "Sensitive content, excluded")
        }
    }
}

fn knowledge_save_block_label(reason: KnowledgeSaveBlock, cx: &App) -> &'static str {
    match reason {
        KnowledgeSaveBlock::SensitiveContent => t(
            cx,
            "敏感内容；保存/发送已阻止",
            "Sensitive content; save/send blocked",
        ),
        KnowledgeSaveBlock::UnknownOrIncompleteOutcome => t(
            cx,
            "未知或未完成结果；保存/发送已阻止",
            "Unknown or incomplete outcome; save/send blocked",
        ),
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn ai_error(error: &AiError) -> Message {
    let zh = match error {
        AiError::InvalidEndpoint => {
            "服务地址必须使用 HTTPS，或本机回环 HTTP；不可包含凭据、查询参数或片段。"
        }
        AiError::InvalidModel => "请填写不含空白或控制字符的模型名称，最多 256 字节。",
        AiError::ContextTooLarge => "上下文或脱敏词列表超过允许大小，请减少所选内容。",
        AiError::EmptyPrompt => "请先填写你想了解的问题。",
        AiError::InvalidBudget => "上下文大小不合要求，问题文本必须完整保留。",
        AiError::InvalidMaxTokens => "输出 Token 上限必须介于 1 和 1000000。",
        AiError::InvalidTokenBudget => {
            "声明的上下文窗口无法容纳完整问题、固定说明和输出预留；请增大窗口或减少输出。"
        }
        AiError::Serialization => "请求格式化失败，请重新准备请求。",
        AiError::InvalidLimits => "请求超时或回复大小限制不合要求。",
        AiError::ClientInitialization => "无法初始化模型服务的网络客户端。",
        AiError::InvalidApiKey => "API 密钥为空或包含不允许的字符。",
        AiError::CredentialInContext => "预览中仍含 API 密钥，请重新脱敏并预览。",
        AiError::Timeout => "请求超时，未自动重试。",
        AiError::Transport => "网络连接或 TLS 验证失败。",
        AiError::HttpStatus(code) => {
            return Message::new(format!("模型服务返回 HTTP {code}"), error.to_string());
        }
        AiError::ResponseTooLarge => "模型回复超过允许大小。",
        AiError::InvalidResponse => "模型服务回复不是有效的 chat-completions JSON。",
        AiError::EmptyReply => "模型服务未返回有效文本。",
        AiError::InvalidProposal => "命令、目标主机或会话信息不完整，无法送入审阅区。",
        AiError::ReviewMismatch => "命令或目标会话已变化，请重新审阅。",
        AiError::ReviewExpired => "命令审阅已过期，请重新审阅。",
        AiError::InvalidDiagnosticPlan => "诊断计划无效，请重新生成。",
        AiError::DiagnosticPlanTooLarge => "诊断计划超过大小或步骤限制，请缩小回复。",
        AiError::NoDiagnosticSteps => "回复中没有完整的 shell 诊断步骤。",
        AiError::DiagnosticPlanMismatch => "诊断计划或会话已变化，请重新生成并审核。",
        _ => return provider_error(error),
    };
    Message::new(zh, error.to_string())
}

fn shell_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut collecting = false;
    let mut current = String::new();
    for line in text.lines() {
        if let Some(language) = line.trim().strip_prefix("```") {
            if collecting {
                if !current.trim().is_empty()
                    && !current
                        .chars()
                        .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    blocks.push(current.trim_end().to_owned());
                }
                collecting = false;
                current.clear();
            } else {
                collecting = matches!(
                    language.trim(),
                    "sh" | "bash" | "zsh" | "shell" | "powershell" | "ps1" | "cmd"
                );
            }
            continue;
        }
        if collecting {
            current.push_str(line);
            current.push('\n');
        }
        if blocks.len() >= 8 {
            break;
        }
    }
    blocks
}
#[cfg(test)]
#[path = "assistant_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "assistant_inference_tests.rs"]
mod inference_tests;
