//! Remote workspace composition: compact tabs, host monitor, terminal, command bar, files.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::assets::IconName;
use gpui_kit::component::Selectable;

impl Workspace {
    fn select_session_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index).cloned() else {
            return;
        };
        self.cancel_remote_completion(cx);
        self.active = index;
        self.session_tab_scroll.scroll_to_item(index);
        tab.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn close_session_tab(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        // A painted control can survive a foreground list change. Resolve its
        // original entity, so an old index cannot close a replacement session.
        let Some(index) = self.tabs.iter().position(|tab| tab.entity_id() == id) else {
            return;
        };
        self.active = index;
        self.close_tab(&CloseTab, window, cx);
    }

    fn empty_workspace_body(&self, width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(300.))
                    .flex_shrink_0()
                    .child(self.quick_connect_surface(cx)),
            )
            .child(
                div()
                    .id("connection-library")
                    .flex_1()
                    .min_h_0()
                    .child(self.connection_table(width, cx)),
            )
            .into_any_element()
    }

    fn quick_connect_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let field = |label: &'static str, id: &'static str, state: &Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w_0()
                .child(div().text_xs().text_color(rgb(visual.muted)).child(label))
                .child(Input::new(state).id(id).aria_label(label))
        };
        let auth_label = if self.quick_password {
            t(
                cx,
                "认证方式：密码（连接时询问）",
                "Authentication: password (ask on connect)",
            )
        } else if self.quick_connect.key.read(cx).value().trim().is_empty() {
            t(cx, "认证方式：SSH Agent", "Authentication: SSH agent")
        } else {
            t(cx, "认证方式：私钥文件", "Authentication: private key")
        };
        div()
            .id("quick-connect-surface")
            .test_support()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(visual.canvas))
            .p_4()
            .child(
                div()
                    .w(px(680.))
                    .max_w(relative(0.96))
                    .bg(rgb(visual.surface))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded_lg()
                    .shadow_lg()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t(cx, "快速连接", "Quick connect")),
                            )
                            .child(
                                div().text_sm().text_color(rgb(visual.muted)).child(t(
                                    cx,
                                    "输入 SSH 端点即可开始一次性远程会话。此处不会保存连接配置。",
                                    "Enter an SSH endpoint to start a one-time remote session. This draft is not saved.",
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(div().flex_1().min_w_0().child(field(
                                t(cx, "主机或 IP 地址", "Host or IP address"),
                                "quick-host",
                                &self.quick_connect.host,
                            )))
                            .child(div().w(px(100.)).flex_shrink_0().child(field(
                                t(cx, "端口", "Port"),
                                "quick-port",
                                &self.quick_connect.port,
                            )))
                            .child(div().flex_1().min_w_0().child(field(
                                t(cx, "SSH 用户名", "SSH username"),
                                "quick-username",
                                &self.quick_connect.username,
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_3()
                            .child(
                                Button::new("quick-auth-mode")
                                    .ghost()
                                    .label(auth_label)
                                    .localized_tooltip("点击切换 Agent/私钥与密码认证；密码仅在本次连接中使用", "Click to switch between agent/key and password authentication; the password is used only for this connection")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.quick_password = !view.quick_password;
                                        cx.notify();
                                    })).map(|button| self.retain_modal_button("quick-auth-mode", button, cx)),
                            )
                            .when(!self.quick_password, |row| {
                                row.child(div().flex_1().min_w_0().child(field(
                                    t(
                                        cx,
                                        "私钥路径（可选）",
                                        "Private key path (optional)",
                                    ),
                                    "quick-key",
                                    &self.quick_connect.key,
                                )))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("quick-open-manager")
                                    .ghost()
                                    .label(t(cx, "打开连接管理器", "Open connection manager"))
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.open_connections(&OpenConnections, window, cx)
                                    })).map(|button| self.retain_modal_button("quick-open-manager", button, cx)),
                            )
                            .child(
                                Button::new("save-quick-profile")
                                    .ghost()
                                    .label(t(cx, "保存为连接…", "Save as connection…"))
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save_quick_as_profile(window, cx)
                                    })).map(|button| self.retain_modal_button("save-quick-profile", button, cx)),
                            )
                            .child(
                                Button::new("quick-connect")
                                    .primary()
                                    .label(t(cx, "连接", "Connect"))
                                    .disabled(self.connecting || self.saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.connect_quick(window, cx)
                                    })).map(|button| self.retain_modal_button("quick-connect", button, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn connection_manager(&self, width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        if !self.show_connections {
            return div().into_any_element();
        }
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000055))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("connection-manager-dialog")
                    .test_support()
                    .w(px(1280.))
                    .h(px(560.))
                    .max_w_full()
                    .max_h_full()
                    .min_w_0()
                    .min_h_0()
                    .bg(rgb(visual.surface))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded_lg()
                    .overflow_hidden()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(34.))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t(cx, "连接管理器", "Connection manager"))
                            .child(
                                Button::new("close-manager")
                                    .ghost()
                                    .compact()
                                    .label("×")
                                    .accessibility_label(t(
                                        cx,
                                        "关闭连接管理器",
                                        "Close connection manager",
                                    ))
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.show_connections = false;
                                        view.focus_current_surface(window, cx);
                                        cx.notify();
                                    }))
                                    .map(|button| {
                                        self.retain_modal_button("close-manager", button, cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(self.connection_table(width.min(px(1280.)), cx)),
                    ),
            )
            .into_any_element()
    }
    fn monitor_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        if let Some(monitor) = self
            .tabs
            .get(self.active)
            .and_then(|tab| self.selected_panels(tab.entity_id()))
            .and_then(|panels| panels.monitor.clone())
        {
            return div().size_full().child(monitor).into_any_element();
        }
        div()
            .size_full()
            .bg(rgb(visual.surface))
            .text_sm()
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(t(cx, "主机信息", "Host information")),
            )
            .child(div().p_3().text_color(rgb(visual.muted)).child(t(
                cx,
                "连接 SSH 主机后显示系统、CPU、内存、进程、网络和磁盘信息。",
                "Connect an SSH host to view system, CPU, memory, processes, network and disks.",
            )))
            .into_any_element()
    }
}
impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        self.maintain_mcp(cx);
        self.maintain_remote_completion(window, cx);
        self.maintain_command_workflows(cx);
        self.refresh_command_suggestions(window, cx);
        self.synchronize_modal_focus(window, cx);
        if let Some(kind) = self.active_modal() {
            return self.isolated_modal_surface(kind, window, cx);
        }
        let viewport = window.viewport_size();
        let assistant_width = px(380.).min(viewport.width * 0.42);
        // At compact widths keep the terminal usable while the assistant is open.
        let show_monitor = !self.tabs.is_empty()
            && viewport.width >= px(900.)
            && (!self.show_assistant || viewport.width >= px(1280.));
        let tool_height = px(340.).min(viewport.height * 0.40);
        // Keep the existing tool budget: secondary completion controls use one
        // row in short windows so Files and command review cannot crush the terminal.
        let compact_command_tools = viewport.height < px(700.);
        let active_id = self.tabs.get(self.active).map(Entity::entity_id);
        let compact_toolbar = viewport.width < px(1280.);
        // Only reveal after selection or a layout change: continuous renders must
        // not undo a user's manual horizontal scroll through other sessions.
        let update_action = self.update_service.read(cx).action_label(cx);
        let reveal = active_id.map(|id| {
            (
                id,
                viewport.width,
                self.tabs.len(),
                i18n::language(cx),
                update_action,
            )
        });
        if self.session_tab_reveal != reveal {
            self.session_tab_reveal = reveal;
            if let Some(expected) = reveal {
                self.session_tab_scroll.scroll_to_item(self.active);
                // Div applies a scroll request before updating its measured
                // viewport. Reapply once after layout, against the new bounds.
                cx.on_next_frame(window, move |view, window, cx| {
                    if view.session_tab_reveal == Some(expected)
                        && view.tabs.get(view.active).map(Entity::entity_id) == Some(expected.0)
                        && window.viewport_size().width == expected.1
                        && view.tabs.len() == expected.2
                        && i18n::language(cx) == expected.3
                        && view.update_service.read(cx).action_label(cx) == expected.4
                    {
                        view.session_tab_scroll.scroll_to_item(view.active);
                        cx.notify();
                    }
                });
            }
        }
        let mut tabs = div()
            .id("session-tabs")
            .test_support()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&self.session_tab_scroll);
        if self.tabs.is_empty() {
            tabs = tabs.child(
                div()
                    .h_full()
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(rgb(visual.surface))
                    .child(t(cx, "快速连接", "Quick connect")),
            );
        }
        for (index, terminal) in self.tabs.iter().enumerate() {
            let terminal_id = terminal.entity_id();
            tabs = tabs.child(
                div()
                    .id(("session-tab", index))
                    .test_support()
                    .h(px(34.))
                    .w(px(184.))
                    .flex_shrink_0()
                    .rounded_md()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .bg(rgb(if self.active == index {
                        visual.selected
                    } else {
                        visual.canvas
                    }))
                    .border_r_1()
                    .border_color(rgb(visual.border))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if let Some(index) = view
                            .tabs
                            .iter()
                            .position(|tab| tab.entity_id() == terminal_id)
                        {
                            view.select_session_tab(index, window, cx);
                        }
                    }))
                    .child(
                        div()
                            .id(("session-tab-title", index))
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(terminal.read(cx).title.clone()),
                    )
                    .when(terminal.read(cx).end_reason().is_some(), |tab| {
                        tab.child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(rgb(visual.muted))
                                .child(t(cx, "已断开", "Offline")),
                        )
                    })
                    .child(
                        Button::new(("close-tab", index))
                            .ghost()
                            .compact()
                            .flex_shrink_0()
                            .label("×")
                            .accessibility_label(format!(
                                "{}: {}",
                                t(cx, "关闭 SSH 会话", "Close SSH session"),
                                terminal.read(cx).title,
                            ))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                cx.stop_propagation();
                                view.close_session_tab(terminal_id, window, cx);
                            }))
                            .map(|button| {
                                self.retain_modal_button(("close-tab", index), button, cx)
                            }),
                    ),
            );
        }
        let body = if self.tabs.is_empty() {
            self.empty_workspace_body(
                viewport.width
                    - if self.show_assistant {
                        assistant_width
                    } else {
                        px(0.)
                    },
                cx,
            )
        } else {
            let mut area = div().size_full().flex().gap(px(1.)).bg(rgb(visual.border));
            for pane in self.displayed_terminals() {
                let id = pane.entity_id();
                area = area.child(
                    div()
                        .id(("terminal-pane", id))
                        .test_support()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .p_2()
                        .bg(rgb(0x0c121b))
                        .border_t_2()
                        .border_color(rgb(if Some(id) == active_id {
                            visual.accent
                        } else {
                            0x0c121b
                        }))
                        .child(self.reconnect_banner(id, cx))
                        .child(div().flex_1().min_h_0().child(pane)),
                );
            }
            area.into_any_element()
        };
        let reviewed_id = self.command_target.or(active_id);
        let stale_command = self
            .command_target
            .is_some_and(|target| !self.tabs.iter().any(|tab| tab.entity_id() == target));
        let reviewed_target = reviewed_id
            .and_then(|id| self.remote_hosts.get(&id))
            .cloned()
            .unwrap_or_else(|| {
                if stale_command {
                    t(
                        cx,
                        "上次会话 · 待审核",
                        "Previous session · review required",
                    )
                    .into()
                } else {
                    t(cx, "未连接", "Disconnected").into()
                }
            });
        let terminal_status = self
            .tabs
            .get(self.active)
            .map(|tab| tab.read(cx).status.render(cx))
            .unwrap_or_else(|| t(cx, "无活动 SSH 会话", "No active SSH session").into());
        let mut bottom_tabs = div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_t_1()
            .border_b_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas));
        for (id, kind, zh, en) in [
            ("files", ToolPanel::Files, "文件", "Files"),
            ("commands", ToolPanel::Commands, "命令", "Commands"),
            ("tunnels", ToolPanel::Tunnels, "隧道", "Tunnels"),
        ] {
            bottom_tabs = bottom_tabs.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .label(t(cx, zh, en))
                    .disabled(active_id.is_none() && kind != ToolPanel::Commands)
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.toggle_panel(kind, window, cx)),
                    )
                    .map(|button| self.retain_modal_button(id, button, cx)),
            );
        }
        if let Some(id) = active_id.filter(|id| self.archived_panels.contains_key(id)) {
            bottom_tabs = bottom_tabs
                .child(div().flex_1())
                .child(
                    Button::new("show-current-session")
                        .ghost()
                        .compact()
                        .label(t(cx, "当前会话", "Current session"))
                        .disabled(!self.show_archived.contains(&id))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.show_archived.remove(&id);
                            cx.notify();
                        }))
                        .map(|button| self.retain_modal_button("show-current-session", button, cx)),
                )
                .child(
                    Button::new("show-previous-session")
                        .ghost()
                        .compact()
                        .label(t(cx, "上次会话快照", "Previous snapshot"))
                        .disabled(self.show_archived.contains(&id))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.show_archived.insert(id);
                            cx.notify();
                        }))
                        .map(|button| {
                            self.retain_modal_button("show-previous-session", button, cx)
                        }),
                );
        }
        let panels = active_id.and_then(|id| self.selected_panels(id));
        let bottom = match self.visible_panel {
            Some(ToolPanel::Files) => panels
                .and_then(|p| p.files.clone())
                .map(IntoElement::into_any_element),
            Some(ToolPanel::Tunnels) => panels
                .and_then(|p| p.tunnels.clone())
                .map(IntoElement::into_any_element),
            Some(ToolPanel::Commands) => Some(self.command_library(cx)),
            None => None,
        };
        // Candidate review temporarily uses the bottom tool space. Keeping the
        // selected tool intact restores its draft and scroll position on dismissal.
        let bottom = if self.remote_completion.visible() {
            None
        } else {
            bottom
        };
        let surface = div()
            .id("workspace")
            .track_focus(&self.surface_focus)
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.text))
            .text_sm()
            .font_family(".SystemUIFont")
            .on_action(cx.listener(Self::open_connections))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_assistant))
            .child(
                div()
                    .h(px(42.))
                    .px_2()
                    .gap_1()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(
                        Button::new("connection-manager")
                            .ghost()
                            .compact()
                            .icon(IconName::FolderOpen)
                            .accessibility_label(t(cx, "连接", "Connections"))
                            .localized_tooltip("连接", "Connections")
                            .when(!compact_toolbar, |button| button.label(t(cx, "连接", "Connections")))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx)
                            })).map(|button| self.retain_modal_button("connection-manager", button, cx)),
                    )
                    .child(tabs)
                    .when(self.tabs.len() > 1, |toolbar| {
                        toolbar.children([
                            Button::new("previous-session")
                                .ghost()
                                .compact()
                                .icon(IconName::ChevronLeft)
                                .accessibility_label(t(cx, "上一个 SSH 会话", "Previous SSH session"))
                                .localized_tooltip("上一个 SSH 会话", "Previous SSH session")
                                .disabled(self.active == 0)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.select_session_tab(view.active.saturating_sub(1), window, cx);
                                }))
                                .map(|button| self.retain_modal_button("previous-session", button, cx)),
                            Button::new("next-session")
                                .ghost()
                                .compact()
                                .icon(IconName::ChevronRight)
                                .accessibility_label(t(cx, "下一个 SSH 会话", "Next SSH session"))
                                .localized_tooltip("下一个 SSH 会话", "Next SSH session")
                                .disabled(self.active + 1 >= self.tabs.len())
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.select_session_tab(view.active + 1, window, cx);
                                }))
                                .map(|button| self.retain_modal_button("next-session", button, cx)),
                        ])
                    })
                    .child(
                        Button::new("new-session")
                            .icon(IconName::Plus)
                            .ghost()
                            .compact()
                            .accessibility_label(t(cx, "新建 SSH 会话", "New SSH session"))
                            .localized_tooltip("新建 SSH 会话", "New SSH session")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx)
                            })).map(|button| self.retain_modal_button("new-session", button, cx)),
                    )
                    .child(
                        Button::new("split-session")
                            .icon(IconName::Columns2)
                            .ghost()
                            .compact()
                            .accessibility_label(t(cx, "分屏", "Split"))
                            .localized_tooltip("分屏", "Split")
                            .when(!compact_toolbar, |button| button.label(t(cx, "分屏", "Split")))
                            .disabled(active_id.is_none())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.split_remote(window, cx)),
                            ).map(|button| self.retain_modal_button("split-session", button, cx)),
                    )
                    .child(
                        Button::new("toggle-assistant")
                            .icon(IconName::Sparkles)
                            .ghost()
                            .compact()
                            .accessibility_label(t(cx, "AI 助手", "AI assistant"))
                            .localized_tooltip("AI 助手", "AI assistant")
                            .when(!compact_toolbar, |button| button.label(t(cx, "AI 助手", "AI assistant")))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.toggle_assistant(&ToggleAssistant, window, cx)
                            })).map(|button| self.retain_modal_button("toggle-assistant", button, cx)),
                    )
                    .child(
                        Button::new("vault-settings")
                            .ghost()
                            .compact()
                            .label(t(cx, "凭据库", "Vault"))
                            .disabled(!self.can_open_vault())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_vault_settings(window, cx)
                            })).map(|button| self.retain_modal_button("vault-settings", button, cx)),
                    )
                    .child(
                        Button::new("about-updates")
                            .icon(IconName::Package)
                            .ghost()
                            .compact()
                            .accessibility_label(t(cx, "关于/更新", "About / updates"))
                            .localized_tooltip("关于/更新", "About / updates")
                            .when(!compact_toolbar, |button| button.label(self.update_service.read(cx).action_label(cx)))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_updates(window, cx)
                            })).map(|button| self.retain_modal_button("about-updates", button, cx)),
                    )
                    .child(
                        Button::new("configuration-recovery")
                            .ghost()
                            .compact()
                            .accessibility_label(t(cx, "配置备份与恢复", "Configuration backup and recovery"))
                            .label(t(cx, "配置恢复", "Recovery"))
                            .localized_tooltip("备份和恢复本机配置；恢复前先关闭 SSH 标签并停止 AI、批量、定时与 MCP 操作", "Back up and restore local metadata; close SSH tabs and stop AI, batch, scheduled and MCP work before recovery")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_configuration_recovery(window, cx)
                            })).map(|button| self.retain_modal_button("configuration-recovery", button, cx)),
                    )
                    .child(
                        div().id("appearance-selector").test_support().flex().gap_1()
                            .children([
                                (keelshell_core::Theme::System, "theme-system", t(cx, "跟随系统", "System")),
                                (keelshell_core::Theme::Light, "theme-light", t(cx, "浅色", "Light")),
                                (keelshell_core::Theme::Dark, "theme-dark", t(cx, "深色", "Dark")),
                            ].into_iter().map(|(theme, id, label)| {
                                Button::new(id).ghost().compact().label(label)
                                    .selected(self.state.settings.theme == theme)
                                    .disabled(self.saving || self.vault_settings.is_some() || self.snippet_modal_open())
                                    .localized_tooltip("设置应用外观；跟随系统会自动响应系统变化", "Select appearance; System follows platform changes")
                                    .on_click(cx.listener(move |view, _, window, cx| view.select_theme(theme, window, cx))).map(|button| self.retain_modal_button(id, button, cx))
                            }))
                    )
                    .child(
                        Button::new("language")
                            .ghost()
                            .compact()
                            .label(t(cx, "中文 / EN", "EN / 中文"))
                            .disabled(self.saving)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.switch_language(window, cx)),
                            ).map(|button| self.retain_modal_button("language", button, cx)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(show_monitor, |el| {
                        el.child(
                            div()
                                .id("monitor-column")
                                .test_support()
                                .w(px(260.))
                                .h_full()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(rgb(visual.border))
                                .child(self.monitor_column(cx)),
                        )
                    })
                    .child(
                        div()
                            .id("command-column")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().flex_1().min_h_0().child(body))
                            .when(active_id.is_some(), |el| {
                                el.child(
                                    div()
                                        .flex_shrink_0()
                                        .px_2()
                                        .py_1()
                                        .when(compact_command_tools, |bar| bar.py_0())
                                        .bg(rgb(visual.surface))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .max_w(px(220.))
                                                .text_xs()
                                                .text_ellipsis()
                                                .text_color(rgb(if reviewed_id != active_id {
                                                    visual.warning
                                                } else {
                                                    visual.muted
                                                }))
                                                .child(reviewed_target),
                                        )
                                        .capture_action(cx.listener(
                                            |view, _: &CompleteRemoteCommand, window, cx| {
                                                if view
                                                    .command
                                                    .read(cx)
                                                    .focus_handle(cx)
                                                    .is_focused(window)
                                                    && !view.command.update(cx, |input, cx| {
                                                        input
                                                            .marked_text_range(window, cx)
                                                            .is_some()
                                                    })
                                                {
                                                    view.request_remote_completion(
                                                        false, window, cx,
                                                    );
                                                    cx.stop_propagation();
                                                } else {
                                                    cx.propagate();
                                                }
                                            },
                                        ))
                                        // Bound input actions run before raw key callbacks in GPUI.
                                        .capture_action(cx.listener(
                                            |view,
                                             _: &gpui_kit::component::input::MoveUp,
                                             window,
                                             cx| {
                                                if !view.suggestion_action("up", window, cx) {
                                                    cx.propagate();
                                                }
                                            },
                                        ))
                                        .capture_action(cx.listener(
                                            |view,
                                             _: &gpui_kit::component::input::MoveDown,
                                             window,
                                             cx| {
                                                if !view.suggestion_action("down", window, cx) {
                                                    cx.propagate();
                                                }
                                            },
                                        ))
                                        .capture_action(cx.listener(
                                            |view,
                                             _: &gpui_kit::component::input::Escape,
                                             window,
                                             cx| {
                                                if !view.suggestion_action("escape", window, cx) {
                                                    cx.propagate();
                                                }
                                            },
                                        ))
                                        .capture_action(cx.listener(
                                            |view,
                                             action: &gpui_kit::component::input::Enter,
                                             window,
                                             cx| {
                                                if action.secondary
                                                    || action.shift
                                                    || !view.suggestion_action("enter", window, cx)
                                                {
                                                    cx.propagate();
                                                }
                                            },
                                        ))
                                        .child(
                                            div()
                                                .id("command-input-container")
                                                .test_support()
                                                .flex_1()
                                                .min_w_0()
                                                .h(px(64.))
                                                .child(Textarea::new(&self.command).h_full()),
                                        )
                                        .when(stale_command, |bar| {
                                            bar.child(
                                                Button::new("review-reconnected-command")
                                                    .ghost()
                                                    .compact()
                                                    .label(t(
                                                        cx,
                                                        "用于当前会话",
                                                        "Use in current session",
                                                    ))
                                                    .on_click(cx.listener(
                                                        |view, _, window, cx| {
                                                            view.review_stale_command(window, cx)
                                                        },
                                                    )).map(|button| self.retain_modal_button("review-reconnected-command", button, cx)),
                                            )
                                        })
                                        .child(
                                            Button::new("run-command")
                                                .compact()
                                                .label(t(cx, "执行", "Run"))
                                                .disabled(
                                                    reviewed_id != active_id
                                                        || self.tabs.get(self.active).is_none_or(
                                                            |tab| !tab.read(cx).is_open(),
                                                        ),
                                                )
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    view.run_command(window, cx)
                                                })).map(|button| self.retain_modal_button("run-command", button, cx)),
                                        ),
                                )
                                .child(self.remote_completion_controls(compact_command_tools, cx))
                                .child(
                                    div()
                                        .id("command-actions")
                                        .test_support()
                                        .flex_shrink_0()
                                        .min_w_0()
                                        .px_2()
                                        .py_1()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_2()
                                        .when(compact_command_tools, |row| row.py(px(2.)).gap_1())
                                        .child(
                                            Button::new("command-history-policy")
                                                .ghost()
                                                .compact()
                                                .label(if self.command_record_history {
                                                    t(
                                                        cx,
                                                        "本次命令：记录历史",
                                                        "This command: keep history",
                                                    )
                                                } else {
                                                    t(
                                                        cx,
                                                        "本次命令：不记录历史",
                                                        "This command: skip history",
                                                    )
                                                })
                                                .disabled(self.command_surface_blocked())
                                                .on_click(cx.listener(|view, _, _, cx| {
                                                    view.command_record_history =
                                                        !view.command_record_history;
                                                    cx.notify();
                                                })).map(|button| self.retain_modal_button("command-history-policy", button, cx)),
                                        )
                                        .child(
                                            Button::new("new-command-draft")
                                                .ghost().compact()
                                                .label(t(cx, "新命令", "New command"))
                                                .localized_tooltip("清空当前草稿，重新选择目标与历史设置", "Clear this draft and reset its target and history choice")
                                                .disabled(self.command_surface_blocked())
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    if view.command_surface_blocked() { return; }
                                                    view.set_reviewed_command(String::new(), None, window, cx);
                                                    view.command.read(cx).focus_handle(cx).focus(window, cx);
                                                    cx.notify();
                                                })).map(|button| self.retain_modal_button("new-command-draft", button, cx)),
                                        )
                                        .child(
                                            Button::new("command-batch")
                                                .ghost()
                                                .compact()
                                                .label(
                                                    if self
                                                        .batch_panel
                                                        .as_ref()
                                                        .is_some_and(|p| p.read(cx).is_running())
                                                    {
                                                        t(
                                                            cx,
                                                            "批量任务 · 运行中",
                                                            "Batch · running",
                                                        )
                                                    } else {
                                                        t(cx, "批量任务", "Batch tasks")
                                                    },
                                                )
                                                .disabled(self.command_surface_blocked())
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    view.open_batch_commands(false, window, cx)
                                                })).map(|button| self.retain_modal_button("command-batch", button, cx)),
                                        )
                                        .child(Button::new("command-workflow").ghost().compact()
                                            .label(if self.workflow_panel.as_ref().is_some_and(|panel|panel.read(cx).is_running()) {
                                                t(cx,"工作流 · 运行中","Workflow · running")
                                            } else {t(cx,"依赖工作流","Dependency workflow")})
                                            .disabled(self.command_surface_blocked())
                                            .on_click(cx.listener(|view,_,window,cx|view.open_workflow(false,window,cx))).map(|button| self.retain_modal_button("command-workflow", button, cx))),
                                )
                                .child(self.remote_completion_list(cx))
                                .child(self.suggestion_list(window, cx))
                            })
                            .child(bottom_tabs)
                            .when_some(bottom, |el, panel| {
                                el.child(div().h(tool_height).flex_shrink_0().child(panel))
                            }),
                    )
                    .when(self.show_assistant, |el| {
                        el.child(
                            div()
                                .id("assistant-column")
                                .test_support()
                                .w(assistant_width)
                                .min_w_0()
                                .overflow_hidden()
                                .flex_shrink_0()
                                .h_full()
                                .bg(rgb(visual.surface))
                                .border_l_1()
                                .border_color(rgb(visual.border))
                                .child(self.assistant.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(div().flex_1().min_w_0().text_ellipsis().child(terminal_status))
                    .child(div().min_w_0().text_ellipsis().child(self.status.render(cx)))
                    .child(
                        Button::new("mcp-settings").ghost().compact().label(self.mcp_toolbar_label())
                            .localized_tooltip("对外 MCP 授权与命令审阅", "External MCP grants and command review")
                            .on_click(cx.listener(|view, _, window, cx| view.open_mcp(window, cx))).map(|button| self.retain_modal_button("mcp-settings", button, cx)),
                    ),
            )
            .when(self.connecting, |surface| surface.child(self.authentication_modal(cx)))
            .capture_key_down(cx.listener(|view, _, window, cx| view.capture_surface_input(window, cx)))
            .into_any_element();
        self.protect_surface_input(surface, cx)
    }
}
