//! Request-option controls share the parent scrolling form and fixed footer.

use super::*;
use crate::{ai_credentials::VaultAction, i18n::t};
use gpui_kit::component::{
    Disableable, Selectable,
    button::{Button, ButtonVariants},
    input::Input,
};

impl AiSettingsPanel {
    pub(in crate::ai_settings) fn request_options_view(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut view = div()
            .id("ai-request-options")
            .flex()
            .flex_col()
            .gap_2()
            .child(super::super::view::label(
                cx,
                "自定义请求头 · 值只在本次运行保留",
                "Custom headers · values stay in this process",
            ));
        let Some(id) = self.selected else {
            return view;
        };
        let Some(editor) = self.request_editors.get(&id) else {
            return view;
        };
        for (index, h) in editor.headers.iter().enumerate() {
            let visual = crate::design::palette(cx);
            let mut row = div()
                .id(("ai-header-row", index))
                .p_2()
                .rounded(px(6.))
                .bg(rgb(visual.canvas))
                .border_1()
                .border_color(rgb(visual.border))
                .flex()
                .flex_col()
                .gap_1()
                .child(super::super::view::label(cx, "请求头名称", "Header name"))
                .child(
                    Input::new(&h.name)
                        .id(("ai-header-name", index))
                        .aria_label(t(cx, "请求头名称", "Header name")),
                );
            let mut choices = div().flex().flex_wrap().gap_1();
            for (kind, zh, en) in [
                (Source::Temporary, "临时值", "Temporary"),
                (Source::Environment, "环境引用", "Environment"),
                (Source::Vault, "凭据库引用", "Vault reference"),
            ] {
                choices = choices.child(
                    Button::new(("ai-header-source", index * 3 + kind as usize))
                        .ghost()
                        .label(t(cx, zh, en))
                        .selected(h.source == kind)
                        .on_click(cx.listener(move |panel, _, window, cx| {
                            if let Some(e) = panel.request_editors.get_mut(&id)
                                && let Some(h) = e.headers.get_mut(index)
                            {
                                h.source = kind;
                            }
                            panel.sync_editor(cx);
                            panel.clear_pending_request_fields(window, cx);
                        })),
                );
            }
            choices = choices.child(
                Button::new(("ai-header-remove", index))
                    .ghost()
                    .label(t(cx, "删除", "Remove"))
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        if let Some(e) = panel.request_editors.get_mut(&id)
                            && index < e.headers.len()
                        {
                            let removed = e.headers.remove(index);
                            removed
                                .value
                                .update(cx, |f, cx| f.set_value("", window, cx));
                        }
                        panel.sync_editor(cx);
                    })),
            );
            row = row.child(choices);
            if h.source == Source::Environment {
                row = row.child(super::super::view::label(
                    cx,
                    "环境变量名称（只保存名称）",
                    "Environment variable name (name only)",
                ));
                row = row.child(
                    Input::new(&h.reference)
                        .id(("ai-header-reference", index))
                        .aria_label(t(cx, "环境变量名称", "Environment variable name")),
                );
            } else {
                row = row.child(super::super::view::label(
                    cx,
                    "秘密值（临时输入或已解锁值）",
                    "Secret value (temporary or unlocked)",
                ));
                row = row.child(
                    Input::new(&h.value)
                        .id(("ai-header-value", index))
                        .aria_label(t(cx, "请求头秘密值", "Header secret value")),
                );
                if h.source == Source::Vault {
                    row = row.child(
                        Input::new(&h.reference)
                            .id(("ai-header-vault-id", index))
                            .aria_label(t(cx, "凭据库 UUID", "Vault UUID")),
                    );
                }
                let purpose = SecretPurpose::Header(h.name.read(cx).value().to_ascii_lowercase());
                row = row.child(self.request_vault_controls(purpose.clone(), cx));
                if self.vault_prompt_matches(Some(&purpose)) {
                    row = row.child(self.vault_prompt_controls(cx));
                }
            }
            view = view.child(row);
        }
        view = view.child(
            Button::new("ai-header-add")
                .ghost()
                .label(t(cx, "添加请求头", "Add header"))
                .disabled(editor.headers.len() >= 32)
                .on_click(cx.listener(|panel, _, window, cx| panel.add_request_header(window, cx))),
        );
        let mut route = div().flex().flex_wrap().gap_1();
        for (explicit, zh, en) in [
            (
                false,
                "直连（忽略环境代理）",
                "Direct (ignore environment proxies)",
            ),
            (true, "显式代理", "Explicit proxy"),
        ] {
            route = route.child(
                Button::new(if explicit {
                    "ai-proxy-explicit"
                } else {
                    "ai-proxy-direct"
                })
                .label(t(cx, zh, en))
                .selected(editor.proxy == explicit)
                .on_click(cx.listener(move |panel, _, window, cx| {
                    if let Some(e) = panel.request_editors.get_mut(&id) {
                        e.proxy = explicit;
                    }
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                })),
            );
        }
        view = view.child(route);
        if editor.proxy {
            view = view.child(super::super::view::label(cx, "代理地址：http(s)://、socks5:// 或 socks5h://；不接受 URL 密码、路径和查询", "Proxy origin: http(s)://, socks5:// or socks5h://; no URL credentials, paths or queries")).child(Input::new(&editor.proxy_url).id("ai-proxy-url").aria_label(t(cx, "代理地址", "Proxy URL")));
            view = view.child(
                Button::new("ai-proxy-auth-toggle")
                    .ghost()
                    .label(t(cx, "使用代理认证", "Use proxy authentication"))
                    .selected(editor.proxy_auth)
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        if let Some(e) = panel.request_editors.get_mut(&id) {
                            e.proxy_auth = !e.proxy_auth;
                        }
                        panel.sync_editor(cx);
                        panel.clear_pending_request_fields(window, cx);
                    })),
            );
            if editor.proxy_auth {
                let mut choices = div().flex().flex_wrap().gap_1();
                for (kind, zh, en) in [
                    (Source::Temporary, "临时凭据", "Temporary"),
                    (Source::Environment, "环境 JSON 引用", "Environment JSON"),
                    (Source::Vault, "凭据库引用", "Vault reference"),
                ] {
                    choices = choices.child(
                        Button::new(("ai-proxy-source", kind as usize))
                            .ghost()
                            .label(t(cx, zh, en))
                            .selected(editor.proxy_source == kind)
                            .on_click(cx.listener(move |panel, _, window, cx| {
                                if let Some(e) = panel.request_editors.get_mut(&id) {
                                    e.proxy_source = kind;
                                }
                                panel.sync_editor(cx);
                                panel.clear_pending_request_fields(window, cx);
                            })),
                    );
                }
                view = view.child(choices);
                if editor.proxy_source == Source::Environment {
                    view = view.child(super::super::view::label(
                        cx,
                        "环境变量名（值为 username/password JSON）",
                        "Environment variable name (username/password JSON value)",
                    ));
                    view = view.child(
                        Input::new(&editor.proxy_reference)
                            .id("ai-proxy-env")
                            .aria_label(t(
                                cx,
                                "含 username/password JSON 的环境变量名",
                                "Environment name containing username/password JSON",
                            )),
                    );
                } else {
                    view = view
                        .child(super::super::view::label(
                            cx,
                            "代理用户名",
                            "Proxy username",
                        ))
                        .child(
                            Input::new(&editor.username)
                                .id("ai-proxy-username")
                                .aria_label(t(cx, "代理用户名", "Proxy username")),
                        )
                        .child(super::super::view::label(cx, "代理密码", "Proxy password"))
                        .child(
                            Input::new(&editor.password)
                                .id("ai-proxy-password")
                                .aria_label(t(cx, "代理密码", "Proxy password")),
                        );
                    if editor.proxy_source == Source::Vault {
                        view = view.child(
                            Input::new(&editor.proxy_reference)
                                .id("ai-proxy-vault-id")
                                .aria_label(t(cx, "代理凭据 UUID", "Proxy credential UUID")),
                        );
                    }
                    view = view.child(self.request_vault_controls(SecretPurpose::Proxy, cx));
                    if self.vault_prompt_matches(Some(&SecretPurpose::Proxy)) {
                        view = view.child(self.vault_prompt_controls(cx));
                    }
                }
            }
        }
        view.child(super::super::view::label(cx, "缺失引用或无效值会阻止请求；代理失败不会改为直连。推理与采样选项按当前模型单独配置。", "Missing references or invalid values prevent requests; proxy failure never falls back to direct. Inference and sampling options are configured separately for the current model."))
    }

    fn request_vault_controls(&self, purpose: SecretPurpose, cx: &mut Context<Self>) -> Div {
        let identity = match &purpose {
            SecretPurpose::Header(name) => format!("header-{name}"),
            SecretPurpose::Proxy => "proxy".into(),
        };
        let save = purpose.clone();
        let unlock = purpose.clone();
        let lock = purpose;
        div()
            .flex()
            .gap_1()
            .child(
                Button::new(SharedString::from(format!("ai-request-save-{identity}")))
                    .ghost()
                    .label(t(cx, "加密保存", "Encrypt and save"))
                    .disabled(self.saving || self.vault_prompt.is_some())
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        panel.begin_request_vault(save.clone(), VaultAction::Save, window, cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("ai-request-unlock-{identity}")))
                    .ghost()
                    .label(t(cx, "解锁", "Unlock"))
                    .disabled(
                        self.saving
                            || self.vault_prompt.is_some()
                            || self
                                .profile()
                                .and_then(|p| reference_for(p, &unlock))
                                .is_none_or(|r| !matches!(r, AiSecretRef::SecretStore { .. })),
                    )
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        panel.begin_request_vault(unlock.clone(), VaultAction::Unlock, window, cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("ai-request-lock-{identity}")))
                    .ghost()
                    .label(t(cx, "清除临时值", "Clear process value"))
                    .on_click(cx.listener(move |panel, _, window, cx| {
                        panel.clear_request_secret(&lock, window, cx);
                    })),
            )
    }
}
