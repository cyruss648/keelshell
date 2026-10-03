//! Remote workspace composition: compact tabs, host monitor, terminal, command bar, files.
use super::*;
use crate::design::{SELECTED, TEXT};
use gpui_kit::assets::IconName;

impl Workspace {
    fn connection_manager(&self, cx: &mut Context<Self>) -> AnyElement {
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
                    .w(px(1280.))
                    .h(px(560.))
                    .max_w_full()
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(BORDER))
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
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.show_connections = false;
                                        view.focus_current_surface(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(div().flex_1().min_h_0().child(self.connection_table(cx))),
            )
            .into_any_element()
    }
    fn monitor_column(&self, cx: &mut Context<Self>) -> AnyElement {
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
            .bg(rgb(PANEL))
            .text_sm()
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(t(cx, "主机信息", "Host information")),
            )
            .child(div().p_3().text_color(rgb(MUTED)).child(t(
                cx,
                "连接 SSH 主机后显示系统、CPU、内存、进程、网络和磁盘信息。",
                "Connect an SSH host to view system, CPU, memory, processes, network and disks.",
            )))
            .into_any_element()
    }
}
impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.maintain_remote_completion(window, cx);
        self.maintain_command_workflows(cx);
        self.refresh_command_suggestions(window, cx);
        let viewport = window.viewport_size();
        // At compact widths keep the terminal usable while the assistant is open.
        let show_monitor = !self.tabs.is_empty()
            && viewport.width >= px(900.)
            && (!self.show_assistant || viewport.width >= px(1280.));
        let tool_height = px(340.).min(viewport.height * 0.40);
        let active_id = self.tabs.get(self.active).map(Entity::entity_id);
        let mut tabs = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .overflow_hidden();
        if self.tabs.is_empty() {
            tabs = tabs.child(
                div()
                    .h_full()
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(rgb(PANEL))
                    .child(t(cx, "快速连接", "Quick connect")),
            );
        }
        for (index, terminal) in self.tabs.iter().enumerate() {
            tabs = tabs.child(
                div()
                    .id(("session-tab", index))
                    .test_support()
                    .h(px(34.))
                    .rounded_md()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .bg(rgb(if self.active == index { SELECTED } else { BG }))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.cancel_remote_completion(cx);
                        view.active = index;
                        if let Some(tab) = view.tabs.get(index) {
                            tab.read(cx).focus_handle(cx).focus(window, cx);
                        }
                        cx.notify();
                    }))
                    .child(terminal.read(cx).title.clone())
                    .when(terminal.read(cx).end_reason().is_some(), |tab| {
                        tab.child(div().text_xs().text_color(rgb(MUTED)).child(t(
                            cx,
                            "已断开",
                            "Offline",
                        )))
                    })
                    .child(
                        Button::new(("close-tab", index))
                            .ghost()
                            .compact()
                            .label("×")
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.active = index;
                                view.close_tab(&CloseTab, window, cx);
                            })),
                    ),
            );
        }
        let body = if self.tabs.is_empty() {
            self.connection_table(cx)
        } else {
            let mut area = div().size_full().flex().gap(px(1.)).bg(rgb(BORDER));
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
                            ACCENT
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
            .border_color(rgb(BORDER))
            .bg(rgb(BG));
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
                    ),
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
                        })),
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
                        })),
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
        div()
            .id("workspace")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
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
                    .border_color(rgb(BORDER))
                    .child(
                        Button::new("connection-manager")
                            .ghost()
                            .compact()
                            .icon(IconName::FolderOpen)
                            .label(t(cx, "连接", "Connections"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx)
                            })),
                    )
                    .child(tabs)
                    .child(
                        Button::new("new-session")
                            .icon(IconName::Plus)
                            .ghost()
                            .compact()
                            .tooltip(t(cx, "新建 SSH 会话", "New SSH session"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx)
                            })),
                    )
                    .child(
                        Button::new("split-session")
                            .icon(IconName::Columns2)
                            .ghost()
                            .compact()
                            .label(t(cx, "分屏", "Split"))
                            .disabled(active_id.is_none())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.split_remote(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("toggle-assistant")
                            .icon(IconName::Sparkles)
                            .ghost()
                            .compact()
                            .label(t(cx, "AI 助手", "AI assistant"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.toggle_assistant(&ToggleAssistant, window, cx)
                            })),
                    )
                    .child(
                        Button::new("vault-settings")
                            .ghost()
                            .compact()
                            .label(t(cx, "凭据库", "Vault"))
                            .disabled(!self.can_open_vault())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_vault_settings(window, cx)
                            })),
                    )
                    .child(
                        Button::new("about-updates")
                            .icon(IconName::Package)
                            .ghost()
                            .compact()
                            .label(t(cx, "关于/更新", "About / updates"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_updates(window, cx)
                            })),
                    )
                    .child(
                        Button::new("language")
                            .ghost()
                            .compact()
                            .label(t(cx, "中文 / EN", "EN / 中文"))
                            .disabled(self.saving)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.switch_language(window, cx)),
                            ),
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
                                .w(px(260.))
                                .h_full()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(rgb(BORDER))
                                .child(self.monitor_column(cx)),
                        )
                    })
                    .child(
                        div()
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
                                        .bg(rgb(PANEL))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .max_w(px(220.))
                                                .text_xs()
                                                .text_ellipsis()
                                                .text_color(rgb(if reviewed_id != active_id {
                                                    0xb14c2c
                                                } else {
                                                    MUTED
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
                                                    )),
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
                                                })),
                                        ),
                                )
                                .child(self.remote_completion_controls(cx))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .justify_between()
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
                                                })),
                                        )
                                        .child(
                                            Button::new("new-command-draft")
                                                .ghost().compact()
                                                .label(t(cx, "新命令", "New command"))
                                                .tooltip(t(cx, "清空当前草稿，重新选择目标与历史设置", "Clear this draft and reset its target and history choice"))
                                                .disabled(self.command_surface_blocked())
                                                .on_click(cx.listener(|view, _, window, cx| {
                                                    if view.command_surface_blocked() { return; }
                                                    view.set_reviewed_command(String::new(), None, window, cx);
                                                    view.command.read(cx).focus_handle(cx).focus(window, cx);
                                                    cx.notify();
                                                })),
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
                                                })),
                                        ),
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
                                .w(px(380.))
                                .flex_shrink_0()
                                .h_full()
                                .bg(rgb(PANEL))
                                .border_l_1()
                                .border_color(rgb(BORDER))
                                .child(self.assistant.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(24.))
                    .px_3()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(terminal_status)
                    .child(self.status.render(cx)),
            )
            .child(self.connection_manager(cx))
            .child(self.connection_form(cx))
            .child(self.library_modal(cx))
            .child(self.authentication_modal(cx))
            .child(self.snippet_modal(cx))
            .child(self.archive_confirmation(cx))
            .child(self.batch_modal(cx))
            .when_some(self.ai_settings.clone(), |el, panel| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(rgba(0x17243a66))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(1080.))
                                .max_w_full()
                                .h(px(700.))
                                .max_h_full()
                                .bg(rgb(crate::design::SURFACE))
                                .rounded_lg()
                                .shadow_lg()
                                .overflow_hidden()
                                .child(panel),
                        ),
                )
            })
            .when_some(self.vault_settings.clone(), |el, panel| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(rgba(0x17243a66))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(900.))
                                .max_w_full()
                                .h(px(650.))
                                .max_h_full()
                                .bg(rgb(crate::design::SURFACE))
                                .rounded_lg()
                                .shadow_lg()
                                .overflow_hidden()
                                .child(panel),
                        ),
                )
            })
            .when_some(self.update_panel.clone(), |el, panel| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(rgba(0x17243a66))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(760.))
                                .max_w_full()
                                .h(px(600.))
                                .max_h_full()
                                .bg(rgb(crate::design::SURFACE))
                                .rounded_lg()
                                .shadow_lg()
                                .overflow_hidden()
                                .child(panel),
                        ),
                )
            })
    }
}
