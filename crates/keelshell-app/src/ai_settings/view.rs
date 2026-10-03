use gpui_kit::{
    component::{
        Disableable, IconName, Selectable,
        button::{Button, ButtonVariants},
        input::Input,
    },
    *,
};
use keelshell_core::{AiAuthentication, AiPreset};

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
    div()
        .text_xs()
        .text_color(rgb(0x66758b))
        .child(t(cx, zh, en))
}

impl Render for AiSettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .bg(rgb(0xf6f8fb))
            .border_r_1()
            .border_color(rgb(0xdce3ec))
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
        let mut form = div()
            .id("ai-profile-form-scroll")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
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
                .child(label(
                    cx,
                    "支持 Chat Completions；其他 API 风格尚未接入。",
                    "Chat Completions is supported; other API styles are not available yet.",
                ))
                .child(label(cx, "完整请求地址", "Full request endpoint"))
                .child(Input::new(&self.endpoint).id("ai-profile-endpoint"))
                .child(label(
                    cx,
                    "仅 HTTPS 或本机回环 HTTP；不自动探测地址。",
                    "HTTPS or loopback HTTP only; no automatic endpoint probing.",
                ));
            let bearer = matches!(profile.authentication, AiAuthentication::Bearer { .. });
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
                        Button::new("ai-auth-none")
                            .label(t(cx, "无认证", "No authentication"))
                            .selected(matches!(profile.authentication, AiAuthentication::None))
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.set_authentication(false, window, cx)
                            })),
                    ),
            );
            if bearer {
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
                    .bg(rgb(0xf6f8fb));
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
            form = form.child(div().mt_2().p_2().bg(rgb(0xf6f8fb)).rounded(px(6.))
                    .child(label(cx,"高级选项暂不可用：自定义请求头、代理、Token 限制与推理参数。","Advanced options unavailable: custom headers, proxy, token limits and reasoning controls.")))
                .child(div().flex().gap_2().mt_2()
                    .child(Button::new("ai-profile-default").ghost().label(if self.catalog.active_id == Some(profile.id) { t(cx,"★ 默认配置","★ Default configuration") } else { t(cx,"设为默认","Set as default") })
                        .on_click(cx.listener(|panel, _, _, cx|panel.make_default(cx))))
                    .child(Button::new("ai-profile-delete").icon(IconName::Delete).ghost().label(t(cx,"删除此配置","Delete configuration"))
                        .on_click(cx.listener(|panel, _, window, cx|panel.remove(window,cx)))));
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
            .bg(rgb(0xffffff))
            .text_color(rgb(0x1d2939))
            .child(div().flex_1().min_h_0().flex().child(list).child(form))
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(0xdce3ec))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(rgb(0x2878e3))
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
