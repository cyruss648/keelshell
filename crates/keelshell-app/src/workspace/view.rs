//! Remote workspace composition: compact tabs, host monitor, terminal, command bar, files.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::{
    assets::IconName,
    base::FocusTrapElement,
    component::{Icon, Selectable},
};

impl Workspace {
    fn responsive_assistant_width(viewport_width: Pixels) -> Pixels {
        px(360.).min(viewport_width * 0.34).max(px(240.))
    }

    fn responsive_navigation_width(&self, viewport_width: Pixels) -> Pixels {
        if self.navigation_panel == NavigationPanel::Monitor || viewport_width >= px(1120.) {
            px(260.)
        } else {
            px(236.)
        }
    }

    fn session_strip(
        &self,
        tabs: AnyElement,
        compact_toolbar: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let components = crate::design::components(cx);
        let mut session_content = div()
            .id("session-content-group")
            .test_support()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .child(tabs)
            .child(
                div()
                    .id("session-count")
                    .test_support()
                    .aria_label(if self.tabs.is_empty() {
                        t(cx, "快速连接", "Quick connect").to_owned()
                    } else {
                        format!("{} {}", self.tabs.len(), t(cx, "个会话", "sessions"))
                    })
                    .flex_shrink_0()
                    .px_2()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .bg(rgb(visual.surface_active))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded(px(components.control_radius))
                    .child(if self.tabs.is_empty() {
                        t(cx, "快速连接", "Quick connect").to_owned()
                    } else {
                        format!("{} {}", self.tabs.len(), t(cx, "个会话", "sessions"))
                    }),
            );
        if !compact_toolbar {
            session_content = session_content
                .px_1()
                .bg(rgb(visual.surface))
                .border_1()
                .border_color(rgb(visual.border))
                .rounded(px(components.control_radius));
        }

        let new_window = Button::new("new-workspace-window")
            .ghost()
            .compact()
            .icon(IconName::Plus)
            .accessibility_label(t(cx, "新建工作区窗口", "New workspace window"))
            .localized_tooltip("新建工作区窗口", "New workspace window")
            .when(!compact_toolbar, |button| {
                button.label(t(cx, "新建窗口", "New window"))
            })
            .disabled(!self.app_windows.read(cx).can_open() || !self.configuration_current(cx))
            .on_click(cx.listener(|view, _, window, cx| {
                view.new_workspace_window(&NewWorkspaceWindow, window, cx)
            }));
        let previous_session = Button::new("previous-session")
            .ghost()
            .compact()
            .icon(IconName::ChevronLeft)
            .accessibility_label(t(cx, "上一个 SSH 会话", "Previous SSH session"))
            .localized_tooltip("上一个 SSH 会话", "Previous SSH session")
            .disabled(self.active == 0)
            .on_click(cx.listener(|view, _, window, cx| {
                view.select_session_tab(view.active.saturating_sub(1), window, cx);
            }))
            .map(|button| self.retain_modal_button("previous-session", button, cx));
        let next_session = Button::new("next-session")
            .ghost()
            .compact()
            .icon(IconName::ChevronRight)
            .accessibility_label(t(cx, "下一个 SSH 会话", "Next SSH session"))
            .localized_tooltip("下一个 SSH 会话", "Next SSH session")
            .disabled(self.active + 1 >= self.tabs.len())
            .on_click(cx.listener(|view, _, window, cx| {
                view.select_session_tab(view.active + 1, window, cx);
            }))
            .map(|button| self.retain_modal_button("next-session", button, cx));
        let new_session = Button::new("new-session")
            .icon(IconName::Plus)
            .ghost()
            .compact()
            .accessibility_label(t(cx, "新建 SSH 会话", "New SSH session"))
            .localized_tooltip("新建 SSH 会话", "New SSH session")
            .on_click(cx.listener(|view, _, window, cx| {
                view.open_connections(&OpenConnections, window, cx)
            }))
            .map(|button| self.retain_modal_button("new-session", button, cx));
        let mut session_actions = div()
            .id("session-actions-group")
            .test_support()
            .flex_shrink_0()
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .child(new_window)
            .child(
                div()
                    .id("session-action-divider")
                    .test_support()
                    .h(px(20.))
                    .w(px(1.))
                    .flex_shrink_0()
                    .bg(rgb(visual.border)),
            );
        if !compact_toolbar {
            session_actions = session_actions
                .px_1()
                .bg(rgb(visual.surface))
                .border_1()
                .border_color(rgb(visual.border))
                .rounded(px(components.control_radius));
        }
        if self.tabs.len() > 1 {
            session_actions = session_actions.child(previous_session).child(next_session);
        }
        session_actions = session_actions.child(new_session);
        div()
            .id("session-strip")
            .test_support()
            .aria_label(t(cx, "SSH 会话导航", "SSH session navigation"))
            .aria_description(t(
                cx,
                "使用左右按钮或 Command/Control 加 Alt 加左右方向键切换会话。",
                "Use the arrow buttons or Command/Control plus Alt plus Left/Right to switch sessions.",
            ))
            .h(px(chrome.session_strip_height))
            .flex_shrink_0()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(rgb(visual.chrome))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(session_content)
            .child(session_actions)
            .into_any_element()
    }

    pub(super) fn select_session_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal().is_some() || self.connecting {
            return;
        }
        let Some(tab) = self.tabs.get(index).cloned() else {
            return;
        };
        self.cancel_remote_completion(cx);
        self.active = index;
        self.sync_session_panes();
        self.session_panes.select(tab.entity_id());
        self.refresh_protocol_selection(cx);
        self.session_tab_scroll.scroll_to_item(index);
        if self.restored_tabs.contains_key(&tab.entity_id()) {
            self.surface_focus.focus(window, cx);
        } else {
            tab.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    pub(super) fn previous_session(
        &mut self,
        _: &PreviousSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs.len() > 1 {
            self.select_session_tab(self.active.saturating_sub(1), window, cx);
        }
    }

    pub(super) fn next_session(
        &mut self,
        _: &NextSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs.len() > 1 {
            self.select_session_tab(
                self.active.saturating_add(1).min(self.tabs.len() - 1),
                window,
                cx,
            );
        }
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

    pub(super) fn quick_connect_surface(&self, cx: &mut Context<Self>) -> AnyElement {
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
                    .rounded(px(crate::design::components(cx).panel_radius))
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
            .bg(rgba(visual.scrim))
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
                    .rounded(px(crate::design::components(cx).panel_radius))
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
    pub(super) fn monitor_column(&self, cx: &mut Context<Self>) -> AnyElement {
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
        self.observe_shared_configuration_if_needed(window, cx);
        if self.close_review.is_some() || self.closing.is_some() || self.cleanup_unknown.is_some() {
            self.synchronize_modal_focus(window, cx);
            return self.isolated_modal_surface(modal_scope::ModalKind::WindowClose, window, cx);
        }
        self.maintain_mcp(cx);
        self.maintain_remote_completion(window, cx);
        self.maintain_command_workflows(cx);
        self.refresh_command_suggestions(window, cx);
        self.synchronize_modal_focus(window, cx);
        if let Some(kind) = self.active_modal()
            && kind != modal_scope::ModalKind::WindowSwitcher
        {
            return self.isolated_modal_surface(kind, window, cx);
        }
        self.render_workspace(window, cx)
    }
}

impl Workspace {
    // Finish each child behind a call boundary so debug builder temporaries
    // do not accumulate in the workspace renderer's stack frame.
    #[inline(never)]
    fn session_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let mut tabs = div()
            .id("session-tabs")
            .test_support()
            .aria_label(t(cx, "SSH 会话标签", "SSH session tabs"))
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
                    .text_color(rgb(visual.muted))
                    .child(t(cx, "快速连接", "Quick connect")),
            );
        }
        for (index, terminal) in self.tabs.iter().enumerate() {
            let terminal_id = terminal.entity_id();
            tabs = tabs.child(
                div()
                    .id(("session-tab", index))
                    .test_support()
                    .h(px(chrome.session_tab_height))
                    .w(px(184.))
                    .flex_shrink_0()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .aria_label(format!(
                        "{}: {}",
                        t(cx, "SSH 会话", "SSH session"),
                        self.session_title(terminal_id, cx)
                    ))
                    .bg(rgb(if self.active == index {
                        visual.surface_active
                    } else {
                        visual.canvas
                    }))
                    .border_1()
                    .border_color(rgb(if self.active == index {
                        visual.accent
                    } else {
                        visual.border
                    }))
                    .when(self.active == index, |tab| {
                        tab.border_b_2().border_color(rgb(visual.accent))
                    })
                    .hover(|style| style.bg(rgb(visual.surface_hover)))
                    .active(|style| style.bg(rgb(visual.surface_active)))
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
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_ellipsis()
                                    .font_weight(if self.active == index {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(self.session_title(terminal_id, cx)),
                            )
                            .when_some(self.restored_status(terminal_id, cx), |title, status| {
                                title.child(
                                    div()
                                        .text_ellipsis()
                                        .text_xs()
                                        .text_color(rgb(visual.muted))
                                        .child(status),
                                )
                            }),
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
                                self.session_title(terminal_id, cx),
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
        tabs.into_any_element()
    }

    #[inline(never)]
    fn workspace_tool_tabs(
        &self,
        active_id: Option<EntityId>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let mut bottom_tabs = div()
            .id("workspace-tool-tabs")
            .test_support()
            .h(px(chrome.tool_tabs_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_t_1()
            .border_b_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.chrome));
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
                    .selected(self.visible_panel == Some(kind))
                    .toggled(self.visible_panel == Some(kind))
                    .disabled(
                        kind != ToolPanel::Commands
                            && active_id.is_none_or(|id| self.restored_tabs.contains_key(&id)),
                    )
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
        bottom_tabs.into_any_element()
    }

    #[inline(never)]
    fn workspace_session_actions(
        &self,
        show_navigation: bool,
        compact_toolbar: bool,
        active_id: Option<EntityId>,
        cx: &mut Context<Self>,
    ) -> [AnyElement; 9] {
        let visual = crate::design::palette(cx);
        let generation = self.pane_generation;
        let can_split = !self.connecting
            && self.tabs.len() < keelshell_core::MAX_LAYOUT_TABS
            && active_id.is_some_and(|id| {
                self.session_panes.can_split(id)
                    && self.remote_sessions.contains_key(&id)
                    && self
                        .tabs
                        .iter()
                        .any(|tab| tab.entity_id() == id && tab.read(cx).is_open())
            });
        let active_pane = self.session_panes.active_pane();
        [
            Button::new("toggle-navigation")
                .ghost()
                .compact()
                .icon(IconName::Columns2)
                .selected(show_navigation)
                .toggled(show_navigation)
                .disabled(self.tabs.is_empty())
                .accessibility_label(t(cx, "连接与主机侧栏", "Connections and host sidebar"))
                .localized_tooltip(
                    "显示或收起连接与主机信息",
                    "Show or hide connections and host information",
                )
                .on_click(cx.listener(|view, _, window, cx| {
                    if view.tabs.is_empty() {
                        return;
                    }
                    let visible = view.navigation_visible(window);
                    view.show_navigation = !visible;
                    if view.show_navigation && window.viewport_size().width < px(1280.) {
                        view.show_assistant = false;
                    }
                    cx.notify();
                }))
                .map(|button| self.retain_modal_button("toggle-navigation", button, cx))
                .into_any_element(),
            Button::new("connection-manager")
                .ghost()
                .compact()
                .icon(IconName::FolderOpen)
                .accessibility_label(t(cx, "连接", "Connections"))
                .localized_tooltip("连接", "Connections")
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "连接", "Connections"))
                })
                .on_click(cx.listener(|view, _, window, cx| {
                    view.open_connections(&OpenConnections, window, cx)
                }))
                .map(|button| self.retain_modal_button("connection-manager", button, cx))
                .into_any_element(),
            {
                let context = active_id
                    .and_then(|id| self.remote_hosts.get(&id))
                    .cloned()
                    .unwrap_or_else(|| t(cx, "选择一个连接", "Select a connection").to_owned());
                div()
                    .id("workspace-session-context")
                    .test_support()
                    .flex_1()
                    .flex_shrink(1.)
                    .min_w_0()
                    .when(compact_toolbar, |context| context.min_w(px(112.)))
                    .px_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(
                        div()
                            .id("workspace-session-context-label")
                            .test_support()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(visual.muted))
                            .child(t(cx, "远程工作区", "Remote workspace")),
                    )
                    .child(
                        div()
                            .id("workspace-session-context-value")
                            .test_support()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(visual.text))
                            .child(context),
                    )
                    .when(!compact_toolbar, |context| {
                        context
                            .bg(rgb(visual.surface))
                            .border_1()
                            .border_color(rgb(visual.border))
                            .rounded(px(crate::design::components(cx).control_radius))
                    })
                    .into_any_element()
            },
            Button::new("split-session")
                .icon(IconName::Columns2)
                .ghost()
                .compact()
                .accessibility_label(t(
                    cx,
                    "左右分屏当前 SSH 会话",
                    "Split current SSH session left and right",
                ))
                .localized_tooltip(
                    "左右分屏当前 SSH 会话",
                    "Split current SSH session left and right",
                )
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "左右分屏", "Split left/right"))
                })
                .disabled(!can_split)
                .on_click(cx.listener(move |view, _, window, cx| {
                    if view.pane_generation == generation
                        && view.tabs.get(view.active).map(Entity::entity_id) == active_id
                    {
                        view.split_remote(keelshell_core::SplitAxis::Horizontal, window, cx);
                    }
                }))
                .map(|button| self.retain_modal_button("split-session", button, cx))
                .into_any_element(),
            Button::new("split-session-vertical")
                .ghost()
                .compact()
                .icon(IconName::Rows2)
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "上下分屏", "Split top/bottom"))
                })
                .accessibility_label(t(
                    cx,
                    "上下分屏当前 SSH 会话",
                    "Split current SSH session top and bottom",
                ))
                .localized_tooltip(
                    "上下分屏当前 SSH 会话",
                    "Split current SSH session top and bottom",
                )
                .disabled(!can_split)
                .on_click(cx.listener(move |view, _, window, cx| {
                    if view.pane_generation == generation
                        && view.tabs.get(view.active).map(Entity::entity_id) == active_id
                    {
                        view.split_remote(keelshell_core::SplitAxis::Vertical, window, cx);
                    }
                }))
                .map(|button| self.retain_modal_button("split-session-vertical", button, cx))
                .into_any_element(),
            Button::new("collapse-pane")
                .ghost()
                .compact()
                .icon(IconName::PanelsTopLeft)
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "合并", "Merge"))
                })
                .accessibility_label(t(
                    cx,
                    "合并活动面板并保留会话",
                    "Merge active pane and keep its sessions",
                ))
                .localized_tooltip(
                    "合并活动面板并保留会话",
                    "Merge active pane and keep its sessions",
                )
                .disabled(self.session_panes.layout().nodes.len() == 1)
                .on_click(cx.listener(move |view, _, window, cx| {
                    if view.pane_generation == generation
                        && view.session_panes.active_pane() == active_pane
                    {
                        view.collapse_active_pane(window, cx);
                    }
                }))
                .map(|button| self.retain_modal_button("collapse-pane", button, cx))
                .into_any_element(),
            Button::new("zoom-pane")
                .ghost()
                .compact()
                .icon(IconName::Maximize2)
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "聚焦", "Focus"))
                })
                .accessibility_label(t(cx, "仅显示活动面板", "Show only the active pane"))
                .localized_tooltip("切换活动面板聚焦显示", "Toggle focus on the active pane")
                .selected(self.pane_zoomed)
                .toggled(self.pane_zoomed)
                .disabled(self.session_panes.layout().nodes.len() == 1)
                .on_click(cx.listener(|view, _, _, cx| {
                    if view.active_modal().is_none() && !view.connecting {
                        view.pane_zoomed = !view.pane_zoomed;
                        cx.notify();
                    }
                }))
                .map(|button| self.retain_modal_button("zoom-pane", button, cx))
                .into_any_element(),
            Button::new("save-pane-layout")
                .ghost()
                .compact()
                .icon(IconName::Save)
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "保存布局", "Save layout"))
                })
                .accessibility_label(t(cx, "保存当前窗口布局", "Save current window layout"))
                .localized_tooltip("保存当前窗口布局", "Save current window layout")
                .disabled(self.saving || self.state_reloading || self.connecting)
                .on_click(cx.listener(|view, _, window, cx| view.save_pane_layout(window, cx)))
                .map(|button| self.retain_modal_button("save-pane-layout", button, cx))
                .into_any_element(),
            Button::new("toggle-assistant")
                .icon(IconName::Sparkles)
                .ghost()
                .compact()
                .accessibility_label(t(cx, "AI 助手", "AI assistant"))
                .localized_tooltip("AI 助手", "AI assistant")
                .when(compact_toolbar, |button| button.label(t(cx, "AI", "AI")))
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "AI 助手", "AI assistant"))
                })
                .on_click(cx.listener(|view, _, window, cx| {
                    view.toggle_assistant(&ToggleAssistant, window, cx)
                }))
                .map(|button| self.retain_modal_button("toggle-assistant", button, cx))
                .into_any_element(),
        ]
    }

    fn workspace_brand(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("workspace-brand")
            .test_support()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .pr_3()
            .border_r_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .size(px(24.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .bg(rgb(visual.primary))
                    .text_color(rgb(visual.primary_text))
                    .child(Icon::new(IconName::Server).size_3()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("KeelShell"),
                    )
                    .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                        cx,
                        "远程工作台",
                        "Remote workbench",
                    ))),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn workspace_settings_actions(
        &self,
        compact_toolbar: bool,
        cx: &mut Context<Self>,
    ) -> [AnyElement; 5] {
        [
            Button::new("vault-settings")
                .icon(IconName::Lock)
                .ghost()
                .compact()
                .accessibility_label(t(cx, "凭据库", "Credential vault"))
                .localized_tooltip("凭据库", "Credential vault")
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "凭据库", "Vault"))
                })
                .disabled(!self.can_open_vault())
                .on_click(cx.listener(|view, _, window, cx| {
                    view.open_vault_settings(window, cx)
                })).map(|button| self.retain_modal_button("vault-settings", button, cx))
            .into_any_element(),
            Button::new("about-updates")
                .icon(IconName::Package)
                .ghost()
                .compact()
                .accessibility_label(t(cx, "关于/更新", "About / updates"))
                .localized_tooltip("关于/更新", "About / updates")
                .when(!compact_toolbar, |button| button.label(self.update_service.read(cx).action_label(cx)))
                .on_click(cx.listener(|view, _, window, cx| {
                    view.open_updates(window, cx)
                })).map(|button| self.retain_modal_button("about-updates", button, cx))
            .into_any_element(),
            Button::new("configuration-recovery")
                .icon(IconName::RefreshCw)
                .ghost()
                .compact()
                .disabled(self.state_reloading)
                .accessibility_label(t(cx, "配置备份与恢复", "Configuration backup and recovery"))
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "配置恢复", "Recovery"))
                })
                .localized_tooltip("备份和恢复本机配置；恢复前先关闭 SSH 标签并停止 AI、批量、定时与 MCP 操作", "Back up and restore local metadata; close SSH tabs and stop AI, batch, scheduled and MCP work before recovery")
                .on_click(cx.listener(|view, _, window, cx| {
                    view.open_configuration_recovery(window, cx)
                })).map(|button| self.retain_modal_button("configuration-recovery", button, cx))
            .into_any_element(),
            Button::new("theme-settings")
                .ghost()
                .compact()
                .icon(IconName::Settings)
                .when(!compact_toolbar, |button| {
                    button.label(t(cx, "主题", "Themes"))
                })
                .accessibility_label(t(cx, "主题与外观", "Themes and appearance"))
                .disabled(self.saving || self.state_reloading)
                .localized_tooltip("选择霜白、曜石、雾紫及明暗模式", "Choose a theme family and appearance")
                .on_click(cx.listener(|view, _, window, cx| view.open_appearance(window, cx)))
                .map(|button| self.retain_modal_button("theme-settings", button, cx))
            .into_any_element(),
            Button::new("language")
                .ghost()
                .compact()
                .label(t(cx, "中/EN", "EN/中"))
                .accessibility_label(t(cx, "切换语言：中文或英文", "Switch language: Chinese or English"))
                .localized_tooltip("切换中文或英文", "Switch between Chinese and English")
                .disabled(self.saving || self.state_reloading)
                .on_click(
                    cx.listener(|view, _, window, cx| view.switch_language(window, cx)),
                ).map(|button| self.retain_modal_button("language", button, cx))
            .into_any_element(),
        ]
    }

    #[inline(never)]
    fn workspace_toolbar(
        &self,
        show_navigation: bool,
        compact_toolbar: bool,
        show_brand: bool,
        active_id: Option<EntityId>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let components = crate::design::components(cx);
        let [
            navigation_toggle,
            connection_manager,
            workspace_context,
            split_horizontal,
            split_vertical,
            collapse_pane,
            zoom_pane,
            save_layout,
            assistant,
        ] = self.workspace_session_actions(show_navigation, compact_toolbar, active_id, cx);
        self.refresh_window_summary(cx);
        let window_summaries = self.app_windows.read(cx).summaries();
        let window_count = window_summaries.len();
        let window_label = if i18n::language(cx) == Language::ZhCn {
            format!(
                "{}，{}",
                t(cx, "工作区窗口", "Workspace windows"),
                window_count
            )
        } else {
            format!(
                "{} {}",
                t(cx, "工作区窗口", "Workspace windows"),
                window_count
            )
        };
        let window_switcher = Button::new("workspace-window-switcher")
            .ghost()
            .compact()
            .label(format!("{} {}", t(cx, "工作区", "Workspace"), window_count))
            .accessibility_label(window_label)
            .selected(self.show_window_switcher)
            .toggled(self.show_window_switcher)
            .on_click(cx.listener(|view, _, window, cx| view.toggle_window_switcher(window, cx)))
            .map(|button| self.retain_modal_button("workspace-window-switcher", button, cx));
        let toolbar_group = |id: &'static str| {
            let group = div()
                .id(id)
                .test_support()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1();
            if compact_toolbar {
                group
            } else {
                group
                    .px_1()
                    .bg(rgb(visual.surface))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded(px(components.control_radius))
            }
        };
        let toolbar_separator = |id: &'static str| {
            let separator = div()
                .id(id)
                .test_support()
                .h(px(20.))
                .w(px(1.))
                .flex_shrink_0()
                .bg(rgb(visual.border));
            if compact_toolbar {
                separator
            } else {
                separator.mx_1()
            }
        };
        let navigation_group = toolbar_group("workspace-navigation-group")
            .child(navigation_toggle)
            .child(connection_manager);
        let context_group = workspace_context;
        let pane_group = toolbar_group("workspace-pane-actions")
            .child(split_horizontal)
            .child(split_vertical)
            .child(collapse_pane)
            .child(zoom_pane)
            .child(save_layout);
        let assistant_group = toolbar_group("workspace-assistant-action").child(assistant);
        let settings_group = toolbar_group("workspace-settings-actions")
            .children(self.workspace_settings_actions(compact_toolbar, cx));
        div()
            .id("workspace-toolbar")
            .test_support()
            .h(px(chrome.toolbar_height))
            .bg(rgb(visual.toolbar))
            .px_2()
            .gap_2()
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(visual.border))
            .when(show_brand, |toolbar| {
                toolbar.child(self.workspace_brand(cx))
            })
            .child(window_switcher)
            .child(navigation_group)
            .child(context_group)
            .child(toolbar_separator("workspace-pane-divider"))
            .child(pane_group)
            .child(toolbar_separator("workspace-assistant-divider"))
            .child(assistant_group)
            .child(toolbar_separator("workspace-settings-divider"))
            .child(settings_group)
            .into_any_element()
    }

    #[inline(never)]
    fn command_input_bar(
        &self,
        active_id: Option<EntityId>,
        reviewed_id: Option<EntityId>,
        reviewed_target: String,
        stale_command: bool,
        compact_command_tools: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let target_color = if reviewed_id != active_id {
            visual.warning
        } else {
            visual.accent
        };
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
                    .id("command-target-badge")
                    .test_support()
                    .max_w(px(220.))
                    .min_w(px(112.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(1.))
                    .px_2()
                    .py_1()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .bg(rgb(visual.chrome))
                    .border_1()
                    .border_color(rgb(if reviewed_id != active_id {
                        visual.warning
                    } else {
                        visual.border
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(rgb(visual.muted))
                            .child(Icon::new(IconName::Server).size_3())
                            .child(t(cx, "审核目标", "Review target")),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_xs()
                            .text_ellipsis()
                            .text_color(rgb(target_color))
                            .child(reviewed_target),
                    ),
            )
            .capture_action(cx.listener(|view, _: &CompleteRemoteCommand, window, cx| {
                if view.command.read(cx).focus_handle(cx).is_focused(window)
                    && !view.command.update(cx, |input, cx| {
                        input.marked_text_range(window, cx).is_some()
                    })
                {
                    view.request_remote_completion(false, window, cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            // Bound input actions run before raw key callbacks in GPUI.
            .capture_action(cx.listener(
                |view, _: &gpui_kit::component::input::MoveUp, window, cx| {
                    if !view.suggestion_action("up", window, cx) {
                        cx.propagate();
                    }
                },
            ))
            .capture_action(cx.listener(
                |view, _: &gpui_kit::component::input::MoveDown, window, cx| {
                    if !view.suggestion_action("down", window, cx) {
                        cx.propagate();
                    }
                },
            ))
            .capture_action(cx.listener(
                |view, _: &gpui_kit::component::input::Escape, window, cx| {
                    if !view.suggestion_action("escape", window, cx) {
                        cx.propagate();
                    }
                },
            ))
            .capture_action(cx.listener(
                |view, action: &gpui_kit::component::input::Enter, window, cx| {
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
                    .h(px(chrome.command_input_height))
                    .child(Textarea::new(&self.command).h_full()),
            )
            .when(stale_command, |bar| {
                bar.child(
                    Button::new("review-reconnected-command")
                        .ghost()
                        .compact()
                        .label(t(cx, "用于当前会话", "Use in current session"))
                        .on_click(
                            cx.listener(|view, _, window, cx| {
                                view.review_stale_command(window, cx)
                            }),
                        )
                        .map(|button| {
                            self.retain_modal_button("review-reconnected-command", button, cx)
                        }),
                )
            })
            .child(
                Button::new("run-command")
                    .primary()
                    .compact()
                    .label(t(cx, "执行", "Run"))
                    .disabled(
                        reviewed_id != active_id
                            || self
                                .tabs
                                .get(self.active)
                                .is_none_or(|tab| !tab.read(cx).is_open()),
                    )
                    .on_click(cx.listener(|view, _, window, cx| view.run_command(window, cx)))
                    .map(|button| self.retain_modal_button("run-command", button, cx)),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn command_actions(&self, compact_command_tools: bool, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("command-actions")
            .test_support()
            .flex_shrink_0()
            .min_w_0()
            .px_2()
            .py_1()
            .border_t_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .when(compact_command_tools, |row| row.py(px(2.)).gap_1())
            .child(
                Button::new("command-history-policy")
                    .ghost()
                    .compact()
                    .selected(self.command_record_history)
                    .toggled(self.command_record_history)
                    .label(if self.command_record_history {
                        t(cx, "记录历史", "History on")
                    } else {
                        t(cx, "不记历史", "History off")
                    })
                    .accessibility_label(if self.command_record_history {
                        t(cx, "本次命令：记录历史", "This command: keep history")
                    } else {
                        t(cx, "本次命令：不记录历史", "This command: skip history")
                    })
                    .localized_tooltip(
                        "仅控制当前命令执行后的历史记录",
                        "Controls history recording for the current command only",
                    )
                    .disabled(self.command_surface_blocked())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.command_record_history = !view.command_record_history;
                        cx.notify();
                    }))
                    .map(|button| self.retain_modal_button("command-history-policy", button, cx)),
            )
            .child(
                Button::new("new-command-draft")
                    .ghost()
                    .compact()
                    .label(t(cx, "新命令", "New command"))
                    .localized_tooltip(
                        "清空当前草稿，重新选择目标与历史设置",
                        "Clear this draft and reset its target and history choice",
                    )
                    .disabled(self.command_surface_blocked())
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.command_surface_blocked() {
                            return;
                        }
                        view.set_reviewed_command(String::new(), None, window, cx);
                        view.command.read(cx).focus_handle(cx).focus(window, cx);
                        cx.notify();
                    }))
                    .map(|button| self.retain_modal_button("new-command-draft", button, cx)),
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
                            t(cx, "批量任务 · 运行中", "Batch · running")
                        } else {
                            t(cx, "批量任务", "Batch tasks")
                        },
                    )
                    .disabled(self.command_surface_blocked())
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_batch_commands(false, window, cx)
                    }))
                    .map(|button| self.retain_modal_button("command-batch", button, cx)),
            )
            .child(
                Button::new("command-workflow")
                    .ghost()
                    .compact()
                    .label(
                        if self
                            .workflow_panel
                            .as_ref()
                            .is_some_and(|panel| panel.read(cx).is_running())
                        {
                            t(cx, "工作流 · 运行中", "Workflow · running")
                        } else {
                            t(cx, "依赖工作流", "Dependency workflow")
                        },
                    )
                    .disabled(self.command_surface_blocked() && self.workflow_panel.is_none())
                    .on_click(
                        cx.listener(|view, _, window, cx| view.open_workflow(false, window, cx)),
                    )
                    .map(|button| self.retain_modal_button("command-workflow", button, cx)),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn workspace_status_bar(&self, terminal_status: String, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        div()
            .h(px(chrome.status_bar_height))
            .px_3()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .bg(rgb(visual.chrome))
            .border_t_1()
            .border_color(rgb(visual.border))
            .text_xs()
            .text_color(rgb(visual.muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .child(terminal_status),
            )
            .child(
                div()
                    .min_w_0()
                    .text_ellipsis()
                    .child(self.status.render(cx)),
            )
            .child(
                Button::new("mcp-settings")
                    .ghost()
                    .compact()
                    .label(self.mcp_toolbar_label())
                    .localized_tooltip(
                        "对外 MCP 授权与命令审阅",
                        "External MCP grants and command review",
                    )
                    .on_click(cx.listener(|view, _, window, cx| view.open_mcp(window, cx)))
                    .map(|button| self.retain_modal_button("mcp-settings", button, cx)),
            )
            .into_any_element()
    }

    // Keep the large workspace builder off the modal rendering stack. An early
    // return in one monolithic debug frame still reserves all its temporaries.
    #[inline(never)]
    fn render_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.sync_session_panes();
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let viewport = window.viewport_size();
        let assistant_width = Self::responsive_assistant_width(viewport.width);
        let navigation_width = self.responsive_navigation_width(viewport.width);
        let navigation_title = match self.navigation_panel {
            NavigationPanel::Connections => t(cx, "连接导航", "Connection navigation"),
            NavigationPanel::Monitor => t(cx, "主机监控", "Host monitoring"),
        };
        let navigation_icon = match self.navigation_panel {
            NavigationPanel::Connections => IconName::FolderOpen,
            NavigationPanel::Monitor => IconName::Activity,
        };
        // At compact widths keep the terminal usable while the assistant is open.
        let show_navigation = self.navigation_visible(window);
        if !show_navigation {
            self.navigation_tree.armed = None;
        }
        if !show_navigation && self.navigation_focus.contains_focused(window, cx) {
            // A resize can unmount the rail while its retained search is focused.
            // Use the inert workspace, never a terminal that would receive typing.
            self.surface_focus.focus(window, cx);
        }
        let compact_command_tools = viewport.height < px(700.);
        // At short heights share the new session-strip budget with the
        // scrollable tool area, retaining usable file controls and multiline input.
        let tool_height = if self.visible_panel == Some(ToolPanel::Files) {
            crate::files::FilesPanel::workbench_height(viewport.height)
        } else {
            px(340.).min(viewport.height * 0.40)
                - if compact_command_tools {
                    px(32.)
                } else {
                    px(0.)
                }
        };
        let active_id = self.tabs.get(self.active).map(Entity::entity_id);
        // Keep the brand at the wide breakpoint, but use compact action labels
        // at the breakpoint itself so every group remains inside the toolbar.
        let compact_toolbar = viewport.width <= px(1280.);
        let show_brand = viewport.width >= px(1280.);
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
                show_navigation,
                self.show_assistant,
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
                        && view.show_assistant == expected.6
                        && view.navigation_visible(window) == expected.5
                    {
                        view.session_tab_scroll.scroll_to_item(view.active);
                        cx.notify();
                    }
                });
            }
        }
        let tabs = self.session_tabs(cx);
        let session_strip = self.session_strip(tabs, compact_toolbar, cx);
        let body = if self.tabs.is_empty() && self.state.workspace_layout.is_none() {
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
            self.native_pane_canvas(window, cx)
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
            .map(|tab| {
                self.restored_status(tab.entity_id(), cx)
                    .map(str::to_owned)
                    .unwrap_or_else(|| tab.read(cx).status.render(cx))
            })
            .unwrap_or_else(|| t(cx, "无活动 SSH 会话", "No active SSH session").into());
        let bottom_tabs = self.workspace_tool_tabs(active_id, cx);
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
            .on_action(cx.listener(Self::open_connections))
            .on_action(cx.listener(Self::open_command_palette))
            .on_action(cx.listener(Self::new_workspace_window))
            .on_action(cx.listener(Self::close_workspace_window))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_assistant))
            .child(self.workspace_toolbar(
                show_navigation,
                compact_toolbar,
                show_brand,
                active_id,
                cx,
            ))
            .when(self.show_window_switcher, |surface| {
                surface.child(self.workspace_window_switcher(window, cx))
            })
            .when(self.configuration_invalidated, |surface| {
                surface.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .child(t(
                            cx,
                            "共享配置已变化；草稿和现有 SSH 保留。",
                            "Shared configuration changed; drafts and live SSH are retained.",
                        ))
                        .child(
                            Button::new("reload-window-configuration")
                                .small()
                                .label(t(cx, "重新加载配置", "Reload configuration"))
                                .disabled(self.saving || self.state_reloading)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.reload_window_configuration(window, cx)
                                })),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(show_navigation, |el| {
                        el.child(
                            div()
                                .id("monitor-column")
                                .test_support()
                                .track_focus(&self.navigation_focus)
                                .w(navigation_width)
                                .h_full()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(rgb(visual.border))
                                .child(
                                    div()
                                        .id("workspace-navigation-rail")
                                        .test_support()
                                        .role(accesskit::Role::Group)
                                        .aria_label(t(cx, "工作区导航", "Workspace navigation"))
                                        .size_full()
                                        .min_w_0()
                                        .min_h_0()
                                        .flex()
                                        .flex_col()
                                        .bg(rgb(visual.sidebar))
                                        .child(
                                            div()
                                                .id("workspace-navigation-header")
                                                .test_support()
                                                .aria_label(navigation_title)
                                                .h(px(chrome.pane_header_height))
                                                .flex_shrink_0()
                                                .px_3()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .bg(rgb(visual.chrome))
                                                .border_b_1()
                                                .border_color(rgb(visual.border))
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(
                                                    div()
                                                        .text_color(rgb(visual.accent))
                                                        .child(Icon::new(navigation_icon).size_3()),
                                                )
                                                .child(navigation_title),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .min_h_0()
                                                .child(self.navigation_column(window, cx)),
                                        ),
                                ),
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
                            .child(session_strip)
                            .child(div().flex_1().min_h_0().child(body))
                            .when(active_id.is_some(), |el| {
                                el.child(self.command_input_bar(
                                    active_id,
                                    reviewed_id,
                                    reviewed_target,
                                    stale_command,
                                    compact_command_tools,
                                    cx,
                                ))
                                .child(self.remote_completion_controls(true, cx))
                                .child(self.command_actions(compact_command_tools, cx))
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
            .child(self.workspace_status_bar(terminal_status, cx))
            .when(self.connecting, |surface| {
                surface.child(self.authentication_modal(cx))
            })
            .capture_key_down(cx.listener(Self::capture_navigation_key_down))
            .capture_key_up(cx.listener(Self::capture_navigation_key_up))
            .into_any_element();
        self.protect_surface_input(surface, cx)
    }

    pub(super) fn workspace_window_switcher(
        &self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let summaries = self.app_windows.read(cx).summaries();
        let popover = div()
            .id("workspace-window-switcher-popover")
            .test_support()
            .absolute()
            .top(px(chrome.toolbar_height))
            .right(px(12.))
            .w(px(360.))
            .max_w(relative(0.92))
            .max_h(relative(0.72))
            .min_h_0()
            .occlude()
            .p_2()
            .rounded(px(crate::design::components(cx).panel_radius))
            .shadow_lg()
            .bg(rgb(visual.surface))
            .border_1()
            .border_color(rgb(visual.border))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "工作区窗口",
                        "Workspace windows",
                    )))
                    .child(
                        Button::new("close-workspace-window-switcher")
                            .ghost()
                            .compact()
                            .label("×")
                            .accessibility_label(t(cx, "关闭窗口列表", "Close window list"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.close_window_switcher(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("workspace-window-list")
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_y_scroll()
                    .children(summaries.iter().map(|summary| {
                        let is_current = summary.id == self.window_id;
                        let session_label = if summary.session_count == 0 {
                            t(cx, "0 个会话", "0 sessions").to_owned()
                        } else {
                            format!("{} {}", summary.session_count, t(cx, "个会话", "sessions"))
                        };
                        let visible_label = format!(
                            "{} · {} · {}",
                            summary.sequence, session_label, summary.context
                        );
                        let accessible_label = if i18n::language(cx) == Language::ZhCn {
                            format!(
                                "{} {}：{}，{}",
                                t(cx, "窗口", "Window"),
                                summary.sequence,
                                session_label,
                                summary.context
                            )
                        } else {
                            format!(
                                "{} {}: {}, {}",
                                t(cx, "窗口", "Window"),
                                summary.sequence,
                                session_label,
                                summary.context
                            )
                        };
                        Button::new(("workspace-window", summary.sequence))
                            .ghost()
                            .w_full()
                            .justify_start()
                            .selected(is_current)
                            .toggled(is_current)
                            .label(visible_label)
                            .accessibility_label(accessible_label)
                            .on_click(cx.listener({
                                let id = summary.id;
                                move |view, _, window, cx| {
                                    view.activate_workspace_window(id, window, cx)
                                }
                            }))
                    })),
            )
            .child(
                Button::new("workspace-new-window")
                    .primary()
                    .w_full()
                    .label(t(cx, "新建窗口", "New window"))
                    .disabled(
                        !self.app_windows.read(cx).can_open() || !self.configuration_current(cx),
                    )
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.close_window_switcher(window, cx);
                        view.new_workspace_window(&NewWorkspaceWindow, window, cx);
                    })),
            )
            .into_any_element();
        div()
            .id("workspace-window-switcher-overlay")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .role(accesskit::Role::Dialog)
            .accessibility_id("keelshell.workspace-window-switcher")
            .aria_label(t(cx, "工作区窗口", "Workspace windows"))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && !event.keystroke.modifiers.modified()
                    && view.close_active_modal(window, cx)
                {
                    cx.stop_propagation();
                }
            }))
            .child(popover)
            .focus_trap("workspace-window-switcher-trap", &self.modal_focus)
            .into_any_element()
    }
}
