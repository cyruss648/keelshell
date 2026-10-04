use gpui_kit::{
    component::{
        Disableable, IconName, Selectable,
        button::{Button, ButtonVariants},
        input::Input,
    },
    *,
};
use keelshell_core::{AiApiStyle, AiAuthentication, AiBackend, AiLocalAgent, AiPreset};

use super::{AiSettingsPanel, OperationKind};
use crate::i18n::t;

fn preset_label(preset: AiPreset, cx: &App) -> &'static str {
    match preset {
        AiPreset::Claude => "Claude",
        AiPreset::OpenAi => "OpenAI",
        AiPreset::Gemini => "Gemini",
        AiPreset::DeepSeek => "DeepSeek",
        AiPreset::Qwen => "Qwen",
        AiPreset::MiniMax => "MiniMax",
        AiPreset::Ollama => "Ollama",
        AiPreset::OpenAiCompatible => t(cx, "OpenAI 兼容", "OpenAI Compatible"),
        AiPreset::AnthropicCompatible => t(cx, "Anthropic 兼容", "Anthropic Compatible"),
        AiPreset::Custom => t(cx, "自定义", "Custom"),
    }
}

fn label(cx: &App, zh: &'static str, en: &'static str) -> Div {
    let visual = crate::design::palette(cx);
    div()
        .text_xs()
        .text_color(rgb(visual.muted))
        .child(t(cx, zh, en))
}

impl Render for AiSettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        self.clear_pending_key(window, cx);
        let mut list = div()
            .id("ai-profile-list")
            .w(px(215.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(visual.canvas))
            .border_r_1()
            .border_color(rgb(visual.border))
            .child(label(cx, "AI 配置", "AI configurations"))
            .child(
                Button::new("ai-profile-create")
                    .icon(IconName::Plus)
                    .primary()
                    .label(t(cx, "新建配置", "New configuration"))
                    .on_click(cx.listener(|panel, _, window, cx| panel.create(window, cx))),
            );
        let mut rows = div()
            .id("ai-profile-list-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (index, profile) in self.catalog.profiles.iter().enumerate() {
            let id = profile.id;
            let name = if profile.name.trim().is_empty() {
                t(cx, "未命名配置", "Unnamed configuration").to_owned()
            } else {
                profile.name.clone()
            };
            let text = if self.catalog.active_id == Some(id) {
                format!("★ {name}")
            } else {
                name
            };
            let mut button = Button::new(("ai-profile", index))
                .label(text)
                .on_click(cx.listener(move |panel, _, window, cx| panel.select(id, window, cx)));
            button = if self.selected == Some(id) {
                button.primary()
            } else {
                button.ghost()
            };
            rows = rows.child(button);
        }
        list = list.child(rows).child(label(
            cx,
            "★ 为默认配置；助手内可临时切换。",
            "★ marks the default; the assistant can select another profile.",
        ));
        let profile = self.profile().cloned();
        // Keep the form at its natural content height inside a scroll viewport.
        // A constrained flex column would shrink model-result hit areas as
        // advanced fields are added, leaving partially visible buttons whose
        // centers are covered by later inputs.
        let mut form = div()
            .id("ai-profile-form-content")
            .test_support()
            .w_full()
            .flex_shrink_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_2();
        if let Some(profile) = profile {
            form = form
                .child(label(cx, "配置名称", "Configuration name"))
                .child(Input::new(&self.name).id("ai-profile-name"))
                .child(label(
                    cx,
                    "调用方式 · 切换会清除密钥和请求选项",
                    "Invocation · switching clears keys and request options",
                ));
            let mut backends = div().flex().flex_wrap().gap_2();
            for (id, agent, text) in [
                ("ai-backend-api", None, t(cx, "模型 API", "Model API")),
                ("ai-backend-codex", Some(AiLocalAgent::Codex), "Codex CLI"),
                (
                    "ai-backend-claude",
                    Some(AiLocalAgent::ClaudeCode),
                    "Claude Code",
                ),
            ] {
                let selected = match profile.backend {
                    AiBackend::Api => agent.is_none(),
                    AiBackend::LocalAgent { agent: current, .. } => agent == Some(current),
                };
                backends = backends.child(Button::new(id).label(text).selected(selected).on_click(
                    cx.listener(move |panel, _, window, cx| panel.set_backend(agent, window, cx)),
                ));
            }
            form = form.child(backends);
            if profile.backend == AiBackend::Api {
                form = form.child(label(
                    cx,
                    "服务商预设 · 更换会清除临时密钥",
                    "Provider preset · changing clears the temporary key",
                ));
                let mut presets = div().flex().flex_wrap().gap_1();
                for (index, preset) in AiPreset::ALL.into_iter().enumerate() {
                    let supported = preset.api_style().supports_current_transport();
                    let mut button = Button::new(("ai-preset", index))
                        .label(if preset == AiPreset::Custom {
                            t(cx, "自定义", "Custom")
                        } else {
                            preset_label(preset, cx)
                        })
                        .disabled(!supported)
                        .on_click(cx.listener(move |panel, _, window, cx| {
                            panel.use_preset(preset, window, cx)
                        }));
                    button = if profile.preset == preset {
                        button.primary()
                    } else {
                        button.ghost()
                    };
                    presets = presets.child(button);
                }
                form = form
                    .child(presets)
                    .child(label(cx, "请求协议", "Request protocol"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("ai-api-style-chat")
                                    .label("Chat Completions")
                                    .selected(profile.api_style == AiApiStyle::ChatCompletions)
                                    .on_click(cx.listener(|panel, _, window, cx| {
                                        panel.set_api_style(AiApiStyle::ChatCompletions, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("ai-api-style-responses")
                                    .label("Responses")
                                    .selected(profile.api_style == AiApiStyle::Responses)
                                    .on_click(cx.listener(|panel, _, window, cx| {
                                        panel.set_api_style(AiApiStyle::Responses, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("ai-api-style-anthropic")
                                    .label("Anthropic Messages")
                                    .selected(profile.api_style == AiApiStyle::AnthropicMessages)
                                    .on_click(cx.listener(|panel, _, window, cx| {
                                        panel.set_api_style(
                                            AiApiStyle::AnthropicMessages,
                                            window,
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .child(label(
                        cx,
                        if profile.api_style == AiApiStyle::AnthropicMessages {
                            "Anthropic Messages 使用 /messages 地址和 x-api-key。"
                        } else {
                            "Responses 使用 /responses 地址；请求仍需人工审阅。"
                        },
                        if profile.api_style == AiApiStyle::AnthropicMessages {
                            "Anthropic Messages uses /messages and x-api-key."
                        } else {
                            "Responses uses a /responses endpoint; requests still require review."
                        },
                    ))
                    .child(label(cx, "完整请求地址", "Full request endpoint"))
                    .child(Input::new(&self.endpoint).id("ai-profile-endpoint"))
                    .child(label(
                        cx,
                        "仅 HTTPS 或本机回环 HTTP；不自动探测地址。",
                        "HTTPS or loopback HTTP only; no automatic endpoint probing.",
                    ));
                let bearer = matches!(profile.authentication, AiAuthentication::Bearer { .. });
                let header = matches!(
                    &profile.authentication,
                    AiAuthentication::Header { name, .. }
                        if name.eq_ignore_ascii_case("x-api-key")
                );
                form = form.child(label(cx, "认证方式", "Authentication")).child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("ai-auth-bearer")
                                .label(t(cx, "Bearer 密钥", "Bearer API key"))
                                .selected(bearer)
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    panel.set_authentication(true, window, cx)
                                })),
                        )
                        .child(
                            Button::new("ai-auth-header")
                                .label(t(cx, "x-api-key", "x-api-key"))
                                .selected(header)
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    panel.set_header_authentication(window, cx)
                                })),
                        )
                        .child(
                            Button::new("ai-auth-none")
                                .label(t(cx, "无认证", "No authentication"))
                                .selected(matches!(profile.authentication, AiAuthentication::None))
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    panel.set_authentication(false, window, cx)
                                })),
                        ),
                );
                if bearer || header {
                    form = form
                        .child(Input::new(&self.key).id("ai-profile-key"))
                        .child(self.vault_controls(cx));
                }
                form = form.child(label(cx,"模型 ID","Model ID"))
                .child(Input::new(&self.model).id("ai-profile-model"))
                .child(div().flex().gap_2()
                    .child(Button::new("ai-models-discover").icon(IconName::Search).ghost().label(t(cx,"发现模型","Discover models"))
                        .disabled(self.operation.is_some() || self.vault_prompt.is_some()).on_click(cx.listener(|panel, _, _, cx| panel.start_operation(OperationKind::Models,cx))))
                    .child(Button::new("ai-profile-test").icon(IconName::Play).ghost().label(t(cx,"测试连接","Test connection"))
                        .disabled(self.operation.is_some() || self.vault_prompt.is_some()).on_click(cx.listener(|panel, _, _, cx| panel.start_operation(OperationKind::Test,cx))))
                    .child(Button::new("ai-operation-cancel").icon(IconName::Close).ghost().label(t(cx,"取消请求","Cancel request"))
                        .disabled(self.operation.is_none()).on_click(cx.listener(|panel, _, _, cx| panel.cancel_operation(true,cx)))))
                .child(label(cx,"测试会向当前地址发送固定的连接检查提示，可能产生服务费用；不发送终端上下文。","The test sends a fixed connectivity prompt to this endpoint and may incur provider usage; terminal context is excluded."));
                if !self.models.is_empty() {
                    let mut models = div()
                        .id("ai-discovered-models")
                        .max_h(px(135.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .p_2()
                        .bg(rgb(visual.canvas));
                    for (index, model) in self.models.iter().enumerate() {
                        let model = model.clone();
                        models = models.child(
                            Button::new(("ai-discovered-model", index))
                                .ghost()
                                .label(model.clone())
                                .on_click(cx.listener(move |panel, _, window, cx| {
                                    panel.model.update(cx, |field, cx| {
                                        field.set_value(model.clone(), window, cx)
                                    });
                                    panel.sync_editor(cx);
                                })),
                        );
                    }
                    form = form.child(models);
                }
                form = form.child(label(cx, "上下文窗口（Token）", "Declared context window (tokens)"))
                .child(Input::new(&self.context_tokens).id("ai-profile-context-tokens").aria_label(t(cx, "上下文窗口（Token）", "Declared context window (tokens)")))
                .child(label(cx, "输出上限（Token）", "Output token limit"))
                .child(Input::new(&self.output_tokens).id("ai-profile-output-tokens").aria_label(t(cx, "输出上限（Token）", "Output token limit")))
                .child(label(cx, "上下文窗口按保守字节预算限制输入，并预留输出；不是实际 Token 计数。填写窗口但不填写输出时使用 4096。", "The context window limits input using a conservative byte budget and reserves output; it is not a measured token count. A window with no output limit uses 4096."))
                .child(label(cx, "输出字段：Chat Completions 使用 max_completion_tokens，Responses 使用 max_output_tokens，Messages 使用 max_tokens；兼容服务需支持对应字段。", "Output field: max_completion_tokens for Chat Completions, max_output_tokens for Responses, max_tokens for Messages. Compatible servers must support the selected field."))
                .child(div().mt_2().p_2().bg(rgb(visual.canvas)).rounded(px(6.))
                    .child(label(cx,"高级选项暂不可用：自定义请求头、代理与推理参数。","Advanced options unavailable: custom headers, proxy and reasoning controls.")))
                ;
            } else {
                form = form
                    .child(label(cx, "CLI 可执行文件", "CLI executable"))
                    .child(Input::new(&self.executable).id("ai-profile-executable").aria_label(t(cx, "CLI 可执行文件绝对路径", "Absolute CLI executable path")))
                    .child(label(cx, "已核对 Codex 0.160.0 / Claude Code 2.1.285；新版本需重新核对兼容性。", "Checked: Codex 0.160.0 / Claude Code 2.1.285. New versions require compatibility review."))
                    .child(label(cx, "推理服务基础地址", "Inference service base URL"))
                    .child(Input::new(&self.endpoint).id("ai-profile-endpoint"))
                    .child(label(cx, "仅 HTTPS 或本机回环 HTTP；地址将在发送前审核中展示。", "HTTPS or loopback HTTP only; the URL appears in the send review."))
                    .child(label(cx, "显式 API 密钥", "Explicit API key"))
                    .child(Input::new(&self.key).id("ai-profile-key"))
                    .child(self.vault_controls(cx))
                    .child(label(cx, "CLI 在隔离临时目录中仅运行问答；不复用现有订阅登录、项目、工具或用户配置。仍会调用上述推理服务。", "CLI runs Ask in an isolated temporary directory. Existing subscription logins, projects, tools and user settings are not reused. The inference service above is still contacted."))
                    .child(label(cx, "模型 ID", "Model ID"))
                    .child(Input::new(&self.model).id("ai-profile-model"))
                    .child(div().flex().flex_wrap().gap_2()
                        .child(Button::new("ai-local-probe").icon(IconName::Search).ghost().label(t(cx, "检查 CLI", "Check CLI"))
                            .disabled(self.operation.is_some() || self.vault_prompt.is_some()).on_click(cx.listener(|panel, _, _, cx| panel.start_operation(OperationKind::LocalProbe, cx))))
                        .child(Button::new("ai-operation-cancel").icon(IconName::Close).ghost().label(t(cx, "取消检查", "Cancel check"))
                            .disabled(self.operation.is_none()).on_click(cx.listener(|panel, _, _, cx| panel.cancel_operation(true, cx)))))
                    .child(label(cx, "检查只读取 CLI 版本、帮助和能力，不发送模型请求；问题在助手中审核后发送。", "Checking reads only CLI version, help and capabilities. Questions are sent after review in the assistant."));
            }
            form = form.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .mt_2()
                    .child(
                        Button::new("ai-profile-default")
                            .ghost()
                            .label(if self.catalog.active_id == Some(profile.id) {
                                t(cx, "★ 默认配置", "★ Default configuration")
                            } else {
                                t(cx, "设为默认", "Set as default")
                            })
                            .on_click(cx.listener(|panel, _, _, cx| panel.make_default(cx))),
                    )
                    .child(
                        Button::new("ai-profile-delete")
                            .icon(IconName::Delete)
                            .ghost()
                            .label(t(cx, "删除此配置", "Delete configuration"))
                            .on_click(cx.listener(|panel, _, window, cx| panel.remove(window, cx))),
                    ),
            );
        } else {
            form = form.child(div().p_4().child(t(
                cx,
                "点击“新建配置”添加模型服务。",
                "Click New configuration to add a model provider.",
            )));
        }
        div()
            .id("ai-settings-panel")
            .track_focus(&self.focus)
            .key_context("AiSettings")
            .h_full()
            .w_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .child(
                div().flex_1().min_h_0().flex().child(list).child(
                    div()
                        .id("ai-profile-form-scroll")
                        .test_support()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(form),
                ),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(rgb(visual.accent))
                            .child(self.status.render(cx)),
                    )
                    .child(
                        Button::new("ai-settings-cancel")
                            .ghost()
                            .label(t(cx, "取消", "Cancel"))
                            .disabled(self.saving)
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    )
                    .child(
                        Button::new("ai-settings-apply")
                            .icon(IconName::Check)
                            .primary()
                            .label(if self.saving {
                                t(cx, "正在保存…", "Saving…")
                            } else {
                                t(cx, "应用", "Apply")
                            })
                            .disabled(self.saving || self.vault_busy())
                            .on_click(cx.listener(|panel, _, _, cx| panel.apply(cx))),
                    ),
            )
    }
}
