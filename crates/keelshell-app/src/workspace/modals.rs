use super::*;
use crate::proxy_editor::{proxy_endpoint, proxy_username};
use crate::{design::SELECTED, jump_host_picker::endpoint};
use gpui_kit::component::popover::Popover;

#[cfg(test)]
mod tests;

fn framed_modal(
    id: &'static str,
    height: Pixels,
    title: AnyElement,
    body: AnyElement,
    footer: AnyElement,
) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .w(px(640.))
        .h(height)
        .max_w_full()
        .max_h_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .rounded_lg()
        .shadow_lg()
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("{id}-header")))
                .max_h(px(96.))
                .overflow_y_scroll()
                .flex_shrink_0()
                .p_4()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(title),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-body")))
                .test_support()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_y_scroll()
                .p_4()
                .child(body),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-footer")))
                .test_support()
                .flex_shrink_0()
                .min_w_0()
                .p_3()
                .bg(rgb(BG))
                .border_t_1()
                .border_color(rgb(BORDER))
                .child(footer),
        )
        .into_any_element()
}

fn route_steps(steps: &[Connection], current: usize, cx: &App) -> AnyElement {
    div()
        .id("connection-route-steps")
        .test_support()
        .flex()
        .flex_col()
        .gap_2()
        .children(steps.iter().enumerate().map(|(index, connection)| {
            div()
                .id(("connection-route-step", index))
                .test_support()
                .p_2()
                .rounded(px(6.))
                .bg(rgb(if index == current { SELECTED } else { BG }))
                .flex()
                .gap_2()
                .child(
                    div()
                        .w(px(22.))
                        .flex_shrink_0()
                        .text_color(rgb(ACCENT))
                        .child((index + 1).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when_some(connection.proxy.as_ref(), |body, proxy| {
                            body.child(div().text_xs().text_color(rgb(ACCENT)).child(format!(
                                "{} → {}",
                                proxy_endpoint(proxy),
                                t(cx, "SSH", "SSH")
                            )))
                        })
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!(
                                    "{} · {}",
                                    connection.name,
                                    if index + 1 == steps.len() {
                                        t(cx, "目标", "Target")
                                    } else {
                                        t(cx, "跳板", "Jump host")
                                    }
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(rgb(MUTED))
                                .child(endpoint(connection)),
                        ),
                )
        }))
        .into_any_element()
}

impl Workspace {
    fn connection_route_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((steps, current)) = self.route_progress(cx) else {
            return div().into_any_element();
        };
        let Some(connection) = steps.get(current) else {
            return div().into_any_element();
        };
        let title = if steps.len() == 1 {
            if connection.proxy.is_some() {
                t(cx, "经代理连接目标", "Target via proxy")
            } else {
                t(cx, "直连目标", "Direct target")
            }
            .to_owned()
        } else {
            format!(
                "{} {}/{} · {}",
                t(cx, "连接步骤", "Connection step"),
                current + 1,
                steps.len(),
                if current + 1 == steps.len() {
                    t(cx, "目标", "Target")
                } else {
                    t(cx, "跳板", "Jump host")
                }
            )
        };
        div()
            .id("connection-route-status")
            .flex_shrink_0()
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xs().text_color(rgb(ACCENT)).child(title))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(connection.name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(rgb(MUTED))
                    .child(endpoint(connection)),
            )
            .when_some(connection.proxy.as_ref(), |body, proxy| {
                body.child(div().text_xs().text_color(rgb(ACCENT)).child(format!(
                    "{} · {}",
                    t(cx, "网络代理", "Network proxy"),
                    proxy_endpoint(proxy)
                )))
            })
            .child(
                Popover::new("connection-route-popover")
                    .anchor(Anchor::BottomLeft)
                    .trigger(
                        Button::new("connection-route-details")
                            .ghost()
                            .compact()
                            .label(t(cx, "查看完整路线", "Show complete route")),
                    )
                    .content(move |_, window, cx| {
                        div()
                            .id("connection-route-scroll")
                            .test_support()
                            .w(
                                px(480.)
                                    .min((window.viewport_size().width - px(48.)).max(px(100.))),
                            )
                            .max_h(
                                px(320.)
                                    .min((window.viewport_size().height - px(80.)).max(px(100.))),
                            )
                            .overflow_y_scroll()
                            .child(route_steps(&steps, current, cx))
                    }),
            )
            .into_any_element()
    }

    pub(super) fn authentication_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let (height, title, body, footer) = if let Some(prompt) = &self.keyboard_interactive {
            let title_text = if prompt.name.trim().is_empty() {
                t(cx, "键盘交互认证", "Keyboard-interactive authentication")
            } else {
                prompt.name.as_str()
            };
            let body = div()
                .id("keyboard-interactive-content")
                .test_support()
                .flex_shrink_0()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.connection_route_status(cx))
                .when(!prompt.instructions.trim().is_empty(), |body| {
                    body.child(
                        div()
                            .id("keyboard-interactive-instructions")
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(prompt.instructions.clone()),
                    )
                })
                .children(prompt.fields.iter().enumerate().map(|(index, field)| {
                    div()
                        .id(("keyboard-interactive-field", index))
                        .flex_shrink_0()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child(field.prompt.clone()))
                        .child(
                            Input::new(&field.answer)
                                .id(("keyboard-interactive-answer", index))
                                .disabled(false),
                        )
                }));
            let footer = div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("cancel-keyboard-interactive")
                        .ghost()
                        .label(t(cx, "取消连接", "Cancel connection"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.cancel_keyboard_interactive(window, cx)
                        })),
                )
                .child(
                    Button::new("submit-keyboard-interactive")
                        .primary()
                        .label(t(cx, "提交并继续", "Submit and continue"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.submit_keyboard_interactive(window, cx)
                        })),
                );
            (
                px(560.),
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title_text.to_owned())
                    .into_any_element(),
                body.into_any_element(),
                footer.into_any_element(),
            )
        } else if let Some(login) = &self.login {
            use super::vault::LoginMode;
            let locked = login.mode == LoginMode::Unlock;
            let save = login.mode == LoginMode::Save;
            let agent = matches!(login.connection.auth, AuthMethod::Agent);
            let ephemeral = self.route_is_ephemeral();
            let proxy_auth = !save && super::vault::needs_proxy_password(&login.connection);
            let keyboard_interactive = self
                .connect_route
                .as_ref()
                .is_some_and(|route| route.keyboard_interactive);
            let description = if locked {
                t(
                    cx,
                    "此连接已保存凭据。主密码仅用于本次解锁，不会缓存。",
                    "This connection has a saved credential. The master password is used only for this unlock and is not cached.",
                )
            } else if save {
                t(
                    cx,
                    "输入 SSH 密码或私钥口令及凭据库主密码。首次保存会创建凭据库；已有凭据库请使用原主密码。主密码丢失后无法恢复。保存不会自动连接。",
                    "Enter the SSH password or key passphrase and the vault master password. The first save creates a vault; use its existing master password afterward. A lost master password cannot be recovered. Saving does not connect.",
                )
            } else if agent {
                t(
                    cx,
                    "SSH 使用本机 Agent 认证。",
                    "SSH authenticates with the local Agent.",
                )
            } else {
                t(
                    cx,
                    "凭据仅用于本次连接；未加密的私钥无需输入口令。",
                    "The secret is used only for this connection. For an unencrypted key, leave it empty.",
                )
            };
            let submit_label = if save {
                t(cx, "保存凭据", "Save credential")
            } else if locked {
                t(cx, "解锁并连接", "Unlock and connect")
            } else {
                t(cx, "连接", "Connect")
            };
            let mut modes = div().flex().flex_wrap().gap_2();
            if login.mode != LoginMode::Once {
                modes = modes.child(
                    Button::new("one-time-credential")
                        .ghost()
                        .disabled(login.busy || self.saving)
                        .label(t(cx, "仅本次使用", "Use one-time secret"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.login_mode(LoginMode::Once, window, cx)
                        })),
                );
            } else if !agent && !ephemeral {
                modes = modes.child(
                    Button::new("save-credential-mode")
                        .ghost()
                        .disabled(login.busy || self.saving)
                        .label(t(cx, "保存到凭据库…", "Save in vault…"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.login_mode(LoginMode::Save, window, cx)
                        })),
                );
            }
            if !agent && !save && !locked {
                modes = modes.child(
                    Button::new("keyboard-interactive-mode")
                        .ghost()
                        .disabled(login.busy || self.saving)
                        .label(if keyboard_interactive {
                            t(cx, "使用密码/私钥认证", "Use password/key authentication")
                        } else {
                            t(cx, "使用键盘交互 / MFA", "Use keyboard-interactive / MFA")
                        })
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.toggle_keyboard_interactive(window, cx)
                        })),
                );
            }
            if login.connection.credential_ref.is_some() && !ephemeral {
                if !locked {
                    modes = modes.child(
                        Button::new("unlock-credential-mode")
                            .ghost()
                            .disabled(login.busy || self.saving)
                            .label(t(cx, "使用已存凭据", "Use saved credential"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.login_mode(LoginMode::Unlock, window, cx)
                            })),
                    );
                }
                modes = modes.child(
                    Button::new("forget-credential")
                        .ghost()
                        .disabled(login.busy || self.saving)
                        .label(t(cx, "解除凭据关联", "Unlink credential"))
                        .on_click(
                            cx.listener(|view, _, window, cx| view.forget_credential(window, cx)),
                        ),
                );
            }
            let ssh_auth = div()
                .id("ssh-login-fields")
                .flex_shrink_0()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .when(proxy_auth, |body| {
                    body.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "SSH 认证",
                        "SSH authentication",
                    )))
                })
                .child(div().text_sm().text_color(rgb(MUTED)).child(description))
                .when(!locked && !agent, |body| {
                    body.child(
                        Input::new(&login.secret)
                            .id("ssh-password")
                            .disabled(login.busy),
                    )
                })
                .when(locked || save, |body| {
                    body.child(Input::new(&login.master).disabled(login.busy))
                })
                .when(save, |body| {
                    body.child(Input::new(&login.confirmation).disabled(login.busy))
                })
                .child(modes);
            let body = div().id("ssh-login-content").flex_shrink_0().test_support().min_w_0().flex().flex_col().gap_3()
                .child(self.connection_route_status(cx))
                .when(self.route_progress(cx).is_none(), |body| body.child(div().id("ssh-login-endpoint").test_support().text_sm().font_family("monospace").child(endpoint(&login.connection))))
                .child(ssh_auth)
                .when(proxy_auth, |body| {
                    let proxy=login.connection.proxy.as_ref();
                    body.child(div().id("proxy-login-section").flex_shrink_0().test_support().min_w_0().p_3().rounded(px(8.)).bg(rgb(BG)).border_1().border_color(rgb(BORDER)).flex().flex_col().gap_2()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(cx,"代理认证","Proxy authentication")))
                        .child(div().text_xs().text_color(rgb(MUTED)).child(proxy.map(|proxy| format!("{} · {}",proxy_endpoint(proxy),proxy_username(proxy).unwrap_or_default())).unwrap_or_default()))
                        .child(Input::new(&login.proxy_secret).id("proxy-password").disabled(login.busy))
                        .child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,"代理密码仅本次使用。代理认证本身未加密，请使用可信网络。","Proxy password is used for this connection only. Proxy authentication is not encrypted; use a trusted network."))))
                })
                .when_some(login.message.clone(),|body,message|body.child(div().id("credential-status").text_sm().text_color(rgb(MUTED)).child(message.render(cx))))
                .when(login.connection.credential_ref.is_some(),|body|body.child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,"解除关联后将重新询问凭据；加密条目仍保留在本机凭据库。","Unlinking restores the credential prompt; the encrypted entry remains in the local vault."))))
                .when(login.busy,|body|body.child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,"已开始的保存可能继续完成；关闭后不会自动连接。","An admitted save may finish after closing; no connection will start."))));
            let footer = div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("cancel-login")
                        .ghost()
                        .label(if login.busy {
                            t(cx, "关闭", "Close")
                        } else {
                            t(cx, "取消", "Cancel")
                        })
                        .on_click(cx.listener(|view, _, window, cx| view.cancel_login(window, cx))),
                )
                .child(
                    Button::new("submit-login")
                        .primary()
                        .disabled(login.busy || self.saving)
                        .label(submit_label)
                        .on_click(cx.listener(|view, _, window, cx| view.submit_login(window, cx))),
                );
            (
                px(if save || proxy_auth { 560. } else { 480. }),
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "{} · {}",
                        t(cx, "身份认证", "Authenticate"),
                        login.connection.name
                    ))
                    .into_any_element(),
                body.into_any_element(),
                footer.into_any_element(),
            )
        } else if let Some(approval) = &self.host_approval {
            let body = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.connection_route_status(cx))
                .when(self.route_progress(cx).is_none(), |body| {
                    body.child(
                        div()
                            .text_sm()
                            .font_family("monospace")
                            .child(endpoint(&approval.connection)),
                    )
                })
                .child(div().text_sm().text_color(rgb(MUTED)).child(t(
                    cx,
                    "请与可信来源核对服务器指纹后再接受。",
                    "Compare this fingerprint with a trusted source before accepting it.",
                )))
                .child(div().text_sm().font_family("monospace").child(format!(
                    "{}: {}",
                    t(cx, "收到的指纹", "Received"),
                    approval.fingerprint
                )))
                .when_some(approval.previous.as_ref(), |body, previous| {
                    body.child(div().text_sm().font_family("monospace").child(format!(
                        "{}: {previous}",
                        t(cx, "已信任的指纹", "Previously trusted")
                    )))
                });
            let footer = div()
                .flex()
                .flex_wrap()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("reject-host-key")
                        .ghost()
                        .label(t(cx, "取消", "Cancel"))
                        .on_click(
                            cx.listener(|view, _, window, cx| view.reject_host_key(window, cx)),
                        ),
                )
                .child(
                    Button::new("accept-host-key")
                        .primary()
                        .label(t(
                            cx,
                            "信任此指纹并重新连接",
                            "Trust this key and reconnect",
                        ))
                        .disabled(self.saving)
                        .on_click(
                            cx.listener(|view, _, window, cx| view.accept_host_key(window, cx)),
                        ),
                );
            (
                px(480.),
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if approval.previous.is_some() {
                        0xb42318
                    } else {
                        ACCENT
                    }))
                    .child(if approval.previous.is_some() {
                        t(cx, "服务器指纹已变更", "Server identity changed")
                    } else {
                        t(cx, "验证服务器指纹", "Verify server identity")
                    })
                    .into_any_element(),
                body.into_any_element(),
                footer.into_any_element(),
            )
        } else if self.connecting {
            // The card leaves draft input and focus available during a handshake.
            return div()
                .id("ssh-connection-progress")
                .test_support()
                .absolute()
                .right(px(16.))
                .bottom(px(40.))
                .w(px(420.))
                .max_w(relative(0.92))
                .max_h(relative(0.72))
                .min_h_0()
                .overflow_hidden()
                .occlude()
                .p_3()
                .rounded_lg()
                .shadow_lg()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(BORDER))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex_shrink_0()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t(cx, "正在建立 SSH 连接…", "Establishing SSH connection…")),
                )
                .child(
                    div()
                        .id("ssh-connection-progress-body")
                        .test_support()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(self.connection_route_status(cx)),
                )
                .child(
                    div()
                        .max_h(px(48.))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(self.status.render(cx)),
                )
                .child(
                    div().flex_shrink_0().flex().justify_end().child(
                        Button::new("cancel-connect-route")
                            .ghost()
                            .compact()
                            .label(t(cx, "取消连接", "Cancel connection"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_connect_route(window, cx)
                            })),
                    ),
                )
                .into_any_element();
        } else {
            return div().into_any_element();
        };
        div()
            .track_focus(&self.overlay_focus)
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(framed_modal(
                "ssh-authentication",
                height,
                title,
                body,
                footer,
            ))
            .into_any_element()
    }

    pub(super) fn connection_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(form) = self.form.as_ref() else {
            return div().into_any_element();
        };
        let field = |label: &str, state: &Entity<InputState>| {
            div()
                .flex_shrink_0()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(label.to_owned()),
                )
                .child(Input::new(state))
        };
        let body = div()
            .id("connection-editor-content")
            .test_support()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(t(
                        cx,
                        "连接信息保存在本机，密码在连接时输入。",
                        "Keep the endpoint here. Credentials stay outside your profile.",
                    )),
            )
            .child(field(t(cx, "名称", "NAME"), &form.name))
            .child(field(t(cx, "主机", "HOST"), &form.host))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(field(t(cx, "用户名", "USERNAME"), &form.username)),
                    )
                    .child(
                        div()
                            .w(px(100.))
                            .flex_shrink_0()
                            .child(field(t(cx, "端口", "PORT"), &form.port)),
                    ),
            )
            .child(form.jump_picker.clone())
            .child(form.proxy_editor.clone())
            .child(form.reconnect_editor.clone())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(t(cx, "文件夹", "FOLDER")),
                    )
                    .child(
                        Button::new("choose-connection-folder")
                            .ghost()
                            .disabled(self.saving)
                            .label(self.folder_label(form.folder_id, cx))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_destination(DestinationTarget::Draft, window, cx)
                            })),
                    ),
            )
            .child(field(t(cx, "标签", "TAGS"), &form.tags))
            .child(
                Button::new("auth-mode")
                    .ghost()
                    .label(if form.password {
                        t(
                            cx,
                            "认证方式：密码（点击切换为私钥 / SSH Agent）",
                            "Password authentication (click for agent/key)",
                        )
                    } else {
                        t(
                            cx,
                            "认证方式：私钥 / SSH Agent（点击切换为密码）",
                            "Agent / key authentication (click for password)",
                        )
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        if let Some(form) = view.form.as_mut() {
                            form.password = !form.password;
                        }
                        cx.notify();
                    })),
            )
            .when(!form.password, |body| {
                body.child(field(t(cx, "私钥文件", "PRIVATE KEY"), &form.key))
            });
        let footer = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("connection-editor-status")
                    .max_h(px(48.))
                    .overflow_y_scroll()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(self.status.render(cx)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-connection")
                            .ghost()
                            .disabled(self.saving)
                            .label(t(cx, "取消", "Cancel"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.form = None;
                                view.focus_current_surface(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("save-connection")
                            .primary()
                            .disabled(self.saving)
                            .label(t(cx, "保存", "Save connection"))
                            .on_click(
                                cx.listener(|view, _, window, cx| view.save_connection(window, cx)),
                            ),
                    ),
            );
        let title = div()
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .child(if form.id.is_some() {
                t(cx, "编辑 SSH 连接", "Edit SSH connection")
            } else {
                t(cx, "新建 SSH 连接", "New SSH connection")
            });
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(framed_modal(
                "connection-editor",
                px(680.),
                title.into_any_element(),
                body.into_any_element(),
                footer.into_any_element(),
            ))
            .into_any_element()
    }
}
