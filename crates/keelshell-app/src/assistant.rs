//! Explicit-context AI panel. It can propose text, never run a terminal command.
use std::{sync::Arc, time::Duration};

use gpui_kit::{
    component::{
        Disableable, IconName,
        button::{Button, ButtonVariants},
        input::{InputEvent, Textarea, TextareaState},
    },
    *,
};
use keelshell_ai::{
    AiError, CommandProposal, ContextDraft, DiagnosticPlan, DiagnosticRisk, PreparedRequest,
    ProviderClient, ProviderConfig, ProviderProtocol, RequestCancellation,
};
use keelshell_core::{AiApiStyle, AiProfileCatalog, NamedAiProfile};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    ai_settings::{EphemeralCredentials, provider_error, uses_api_key},
    i18n::{Message, t},
};

pub enum AssistantEvent {
    Capture { selection_only: bool },
    OpenSettings,
    SelectProfile(Uuid),
    Suggestion { command: String, session_id: String },
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
    context: String,
    host: String,
    session_id: String,
    prepared: Option<PreparedRequest>,
    response: String,
    status: Message,
    busy: bool,
    preview: bool,
    suggestions: Vec<String>,
    diagnostic_plan: Option<DiagnosticPlan>,
    request_revision: u64,
    response_target: Option<(String, String)>,
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
        let subscriptions = vec![cx.subscribe(&prompt, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
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
            context: String::new(),
            host: String::new(),
            session_id: String::new(),
            prepared: None,
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
        self.invalidate_request(cx);
        self.context = text;
        self.host = host;
        self.session_id = session_id;
        self.prepared = None;
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
        self.invalidate_request(cx);
        self.context.clear();
        self.host.clear();
        self.session_id.clear();
        self.status = Message::new(
            "原 SSH 会话已结束；问题草稿已保留，请重新选择上下文并审核。",
            "The SSH session ended. Your question is preserved; select fresh context and review again.",
        );
        cx.notify();
    }

    fn invalidate_request(&mut self, cx: &mut Context<Self>) {
        self.request_revision = self.request_revision.wrapping_add(1);
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self._job = None;
        self.busy = false;
        self.prepared = None;
        self.preview = false;
        self.response.clear();
        self.suggestions.clear();
        self.diagnostic_plan = None;
        self.response_target = None;
        self.status = Message::new(
            "请求已修改，请重新预览后再发送；旧请求已取消。",
            "Request changed. Preview again before sending; the old request was cancelled.",
        );
        cx.notify();
    }

    fn prepare(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(profile) = self.profile.as_ref() else {
            self.status = Message::new(
                "请先在 AI 设置中添加并选择配置。",
                "Add and select a profile in AI settings first.",
            );
            cx.notify();
            return;
        };
        if let Err(error) = profile.validate_current_transport() {
            self.status = Message::detail(
                "该配置当前不可用",
                "This configuration is not currently usable",
                error,
            );
            cx.notify();
            return;
        }
        if uses_api_key(&profile.authentication)
            && self.key.as_ref().is_none_or(|key| key.is_empty())
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
        let result = ProviderConfig::new_with_protocol(&profile.endpoint, &profile.model, protocol)
            .and_then(|provider| {
                // All explicitly configured temporary credentials are redacted,
                // including one belonging to another saved profile.
                let secrets: Vec<&str> = self
                    .credentials
                    .values()
                    .map(|key| key.as_str())
                    .chain(self.key.as_ref().map(|key| key.as_str()))
                    .collect();
                ContextDraft::new(self.prompt.read(cx).value().to_string())
                    .with_host_label(self.host.clone())
                    .add_selection(
                        t(
                            cx,
                            "用户选择的远端终端文本",
                            "Explicitly selected remote terminal text",
                        ),
                        self.context.clone(),
                    )
                    .prepare_with_limits(
                        &provider,
                        &secrets,
                        16 * 1024,
                        profile.max_output_tokens,
                        profile.context_window_tokens,
                    )
            });
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
                self.preview = true;
            }
            Err(error) => self.status = ai_error(&error),
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
        let key = if self
            .profile
            .as_ref()
            .is_some_and(|p| uses_api_key(&p.authentication))
        {
            self.key.clone()
        } else {
            None
        };
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
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                let client = ProviderClient::new(Duration::from_secs(60), 1024 * 1024)?;
                client
                    .send_approved(
                        request.approve(),
                        key.as_ref().map(|key| key.as_str()),
                        &cancellation,
                    )
                    .await
                    .map(|reply| reply.into_text())
            },
        );
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(AiError::Transport));
            let _ = this.update(cx, |panel, cx| {
                panel.finish_request(revision, response_target, result, cx)
            });
        }));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.invalidate_request(cx);
        self.status = Message::new(
            "已取消本地请求；服务端可能已处理发送的内容。",
            "Local request cancelled; the provider may already have processed sent content.",
        );
        cx.notify();
    }

    fn finish_request(
        &mut self,
        revision: u64,
        target: (String, String),
        result: Result<String, AiError>,
        cx: &mut Context<Self>,
    ) {
        if self.request_revision != revision {
            return;
        }
        self.busy = false;
        self.cancellation = None;
        match result {
            Ok(text) => {
                self.response_target = Some(target);
                self.response = text;
                self.suggestions = shell_blocks(&self.response);
                self.diagnostic_plan = None;
                self.status = Message::new(
                    "已收到回复。命令建议需先审阅，再送入输入框。",
                    "Response received. Suggestions require review before use.",
                );
            }
            Err(error) => self.status = ai_error(&error),
        }
        cx.notify();
    }

    fn suggest(&mut self, index: usize, cx: &mut Context<Self>) {
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
        match DiagnosticPlan::from_response(
            self.host.clone(),
            self.session_id.clone(),
            &self.context,
            &self.response,
        ) {
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
        let Some(plan) = self.diagnostic_plan.as_ref() else {
            return;
        };
        let Ok(review) = plan.review_step(index, Duration::from_secs(120)) else {
            self.status = ai_error(&AiError::InvalidDiagnosticPlan);
            cx.notify();
            return;
        };
        let proposal = match review.into_proposal(plan, &self.session_id) {
            Ok(proposal) => proposal,
            Err(error) => {
                self.status = ai_error(&error);
                cx.notify();
                return;
            }
        };
        match proposal
            .review(Duration::from_secs(120))
            .and_then(|ticket| ticket.into_suggestion(&proposal, &self.session_id))
        {
            Ok(command) => cx.emit(AssistantEvent::Suggestion {
                command,
                session_id: self.session_id.clone(),
            }),
            Err(error) => self.status = ai_error(&error),
        }
        cx.notify();
    }
}
impl Drop for AssistantPanel {
    fn drop(&mut self) {
        if let Some(cancellation) = &self.cancellation {
            cancellation.cancel();
        }
    }
}
impl EventEmitter<AssistantEvent> for AssistantPanel {}
impl Render for AssistantPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let profile_choices = if self.selecting_profile {
            let mut choices = div().p_2().bg(rgb(visual.canvas)).flex().flex_col().gap_1();
            for (index, profile) in self.profiles.profiles.iter().enumerate() {
                let id = profile.id;
                choices = choices.child(
                    Button::new(("assistant-profile-choice", index))
                        .ghost()
                        .label(profile.name.clone())
                        .disabled(profile.validate_current_transport().is_err())
                        .on_click(cx.listener(move |panel, _, _, cx| panel.select_profile(id, cx))),
                );
            }
            Some(choices)
        } else {
            None
        };
        let mut content = div()
            .id("assistant-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
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
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(AssistantEvent::Capture {
                                    selection_only: true,
                                })
                            })),
                    )
                    .child(
                        Button::new("context-screen")
                            .ghost()
                            .label(t(cx, "当前屏幕", "Visible screen"))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(AssistantEvent::Capture {
                                    selection_only: false,
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
            .child(Textarea::new(&self.prompt))
            .child(
                Button::new("prepare-request")
                    .primary()
                    .disabled(self.busy || self.profile.is_none())
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
        if self.busy {
            content = content.child(
                Button::new("assistant-cancel-request")
                    .ghost()
                    .label(t(cx, "取消请求", "Cancel request"))
                    .on_click(cx.listener(|panel, _, _, cx| panel.cancel(cx))),
            );
        }
        if self.preview
            && let Some(request) = self.prepared.as_ref()
        {
            content = content
                .child(
                    div().text_xs().child(
                        Message::detail("发送至", "Destination", request.provider().endpoint())
                            .render(cx),
                    ),
                )
                .child(
                    div()
                        .p_2()
                        .rounded(px(6.))
                        .bg(rgb(visual.canvas))
                        .border_1()
                        .border_color(rgb(visual.border))
                        .text_xs()
                        .font_family("monospace")
                        .child(request.preview_json().to_owned()),
                )
                .child(
                    Button::new("send-approved-request")
                        .primary()
                        .label(t(cx, "确认发送此请求", "Send this exact request"))
                        .on_click(cx.listener(|view, _, _, cx| view.send(cx))),
                );
        }
        if !self.response.is_empty() {
            // Provider output stays plain text and never triggers implicit requests.
            content = content
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
            content = content.child(
                Button::new("build-diagnostic-plan")
                    .ghost()
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
                                Button::new(("review-diagnostic-step", index))
                                    .ghost()
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
                            Button::new(("review-suggestion", index))
                                .ghost()
                                .label(t(cx, "送入命令审阅区", "Place in command review"))
                                .on_click(
                                    cx.listener(move |view, _, _, cx| view.suggest(index, cx)),
                                ),
                        ),
                );
            }
        }
        div()
            .h_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .child(content)
    }
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
