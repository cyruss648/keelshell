//! Restore one saved window as native, inactive views, without runtime authority.

use super::*;
use gpui_kit::component::Selectable;
use keelshell_core::{LayoutTab, LayoutTabAvailability};

/// An explicit connection action is tied to the inert owner, never a tab index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RestoredTarget {
    pub(super) entity: EntityId,
    pub(super) layout_tab: uuid::Uuid,
    pub(super) connection: uuid::Uuid,
}

impl Workspace {
    /// Called only by construction, before any runtime or reviewed target exists.
    pub(super) fn restore_startup_layout(&mut self, cx: &mut Context<Self>) {
        // Reloading configuration must never replace live sessions or reviews.
        if !self.tabs.is_empty() || !self.remote_sessions.is_empty() {
            return;
        }
        let Some(snapshot) = &self.state.workspace_layout else {
            return;
        };
        if let Err(error) = snapshot.validate() {
            self.status = Message::detail(
                "无法恢复布局，保存数据已保留",
                "Cannot restore layout; saved data was preserved",
                error,
            );
            return;
        }
        let Some(saved) = snapshot
            .windows
            .iter()
            .find(|saved| saved.id == snapshot.active_window)
        else {
            return;
        };
        let mut tabs = Vec::new();
        let mut stable_ids = HashMap::new();
        let mut restored = HashMap::new();
        let panes = pane_sessions::SessionPanes::restore(saved, |tab| {
            let title = restored_title(tab, &self.state, cx);
            let terminal =
                cx.new(|cx| TerminalView::inactive(title, self.state.settings.font_size, cx));
            let id = terminal.entity_id();
            stable_ids.insert(id, tab.id);
            restored.insert(id, tab.clone());
            tabs.push(terminal);
            id
        });
        match panes {
            Ok(panes) => {
                self.layout_window_id = saved.id;
                self.active = panes
                    .active_session()
                    .and_then(|active| tabs.iter().position(|tab| tab.entity_id() == active))
                    // An empty saved active leaf has no global strip selection.
                    .unwrap_or(tabs.len());
                self.tabs = tabs;
                self.pane_tab_ids = stable_ids;
                self.restored_tabs = restored;
                self.session_panes = panes;
                self.refresh_protocol_selection(cx);
                self.status = Message::new(
                    "已恢复保存的活动窗口布局，所有标签均未连接。",
                    "Restored the saved active window layout. All tabs are disconnected.",
                );
            }
            Err(error) => {
                self.status = Message::detail(
                    "无法恢复布局，保存数据已保留",
                    "Cannot restore layout; saved data was preserved",
                    error,
                );
            }
        }
    }

    pub(super) fn session_title(&self, id: EntityId, cx: &App) -> String {
        if let Some(tab) = self.restored_tabs.get(&id) {
            restored_title(tab, &self.state, cx)
        } else {
            self.tabs
                .iter()
                .find(|tab| tab.entity_id() == id)
                .map(|tab| tab.read(cx).title.clone())
                .unwrap_or_default()
        }
    }

    pub(super) fn restored_target_current(&self, target: RestoredTarget) -> bool {
        self.restored_tabs.get(&target.entity).is_some_and(|tab| {
            tab.id == target.layout_tab
                && tab.connection_id == Some(target.connection)
                && tab.availability(&self.state) == LayoutTabAvailability::Available
        }) && self.pane_tab_ids.get(&target.entity) == Some(&target.layout_tab)
            && self.session_panes.pane_for(target.entity).is_some()
            && self.tabs.iter().any(|tab| tab.entity_id() == target.entity)
            && !self.remote_sessions.contains_key(&target.entity)
    }

    pub(super) fn request_restored_connect(
        &mut self,
        id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal().is_some()
            || self.connecting
            || self.saving
            || self.state_reloading
            || self.connect_route.is_some()
        {
            return;
        }
        let Some(tab) = self.restored_tabs.get(&id) else {
            return;
        };
        let Some(connection_id) = tab.connection_id else {
            return;
        };
        let target = RestoredTarget {
            entity: id,
            layout_tab: tab.id,
            connection: connection_id,
        };
        if !self.restored_target_current(target) {
            self.status = Message::new(
                "连接已移除，请在连接管理器中检查；此标签保持未连接。",
                "Connection removed. Check the connection manager; this tab remains disconnected.",
            );
            cx.notify();
            return;
        }
        let Some(connection) = self
            .state
            .connections
            .iter()
            .find(|profile| profile.id == connection_id)
            .cloned()
        else {
            return;
        };
        // Reuse ordinary current-profile authentication, per-hop trust checks,
        // cancellation and revision guards. This is not a reconnect ticket.
        if self.begin_connect_route(&connection, cx) {
            if let Some(route) = &mut self.connect_route {
                route.restored = Some(target);
            }
            self.prepare_route_hop(window, cx);
        }
    }

    pub(super) fn restored_status(&self, id: EntityId, cx: &App) -> Option<&'static str> {
        self.restored_tabs
            .get(&id)
            .map(|tab| restored_state(tab.availability(&self.state), cx))
    }

    #[inline(never)]
    pub(super) fn render_restored_tab(&self, id: EntityId, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.restored_tabs.get(&id) else {
            return Empty.into_any_element();
        };
        let visual = crate::design::palette(cx);
        let availability = tab.availability(&self.state);
        let state = restored_state(availability, cx);
        let hint = match availability {
            LayoutTabAvailability::Available => t(
                cx,
                "连接会使用当前配置，重新认证并核对服务器身份。",
                "Connect uses the current profile, fresh authentication and server identity checks.",
            ),
            LayoutTabAvailability::Unbound => t(
                cx,
                "上次的一次性连接未保存。请从连接管理器新建或选择连接。",
                "The one-time connection was not saved. Create or choose a connection in the manager.",
            ),
            LayoutTabAvailability::Missing => t(
                cx,
                "连接已删除或在回收站。请在连接管理器检查；此标签保留原引用。",
                "Check deleted connections in the manager. This tab keeps its original reference.",
            ),
        };
        div()
            .id(("restored-tab", id))
            .test_support()
            .role(Role::Group)
            .aria_label(format!("{}: {state}", self.session_title(id, cx)))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .p_3()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.text))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.session_title(id, cx)),
            )
            .child(
                div()
                    .id(("restored-tab-state", id))
                    .test_support()
                    .role(Role::Label)
                    .aria_label(state)
                    .text_sm()
                    .text_color(rgb(visual.muted))
                    .child(state),
            )
            .child(div().text_sm().text_color(rgb(visual.muted)).child(hint))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .when(
                        availability == LayoutTabAvailability::Available,
                        |actions| {
                            actions.child(
                                Button::new(("connect-restored-tab", id))
                                    .primary()
                                    .small()
                                    .label(t(cx, "连接", "Connect"))
                                    .accessibility_label(format!(
                                        "{}: {}",
                                        t(
                                            cx,
                                            "连接当前保存配置",
                                            "Connect using current saved profile"
                                        ),
                                        self.session_title(id, cx)
                                    ))
                                    .disabled(
                                        self.connecting
                                            || self.saving
                                            || self.state_reloading
                                            || self.connect_route.is_some(),
                                    )
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.request_restored_connect(id, window, cx);
                                    }))
                                    .map(|button| {
                                        self.retain_modal_button(
                                            ("connect-restored-tab", id),
                                            button,
                                            cx,
                                        )
                                    }),
                            )
                        },
                    )
                    .child(
                        Button::new(("manage-restored-tab", id))
                            .ghost()
                            .small()
                            .label(t(cx, "管理连接", "Manage connections"))
                            .accessibility_label(t(cx, "打开连接管理器", "Open connection manager"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx);
                            }))
                            .map(|button| {
                                self.retain_modal_button(("manage-restored-tab", id), button, cx)
                            }),
                    )
                    .child(
                        Button::new(("close-restored-tab", id))
                            .ghost()
                            .small()
                            .label(t(cx, "关闭标签", "Close tab"))
                            .accessibility_label(t(cx, "关闭未连接标签", "Close disconnected tab"))
                            .disabled(self.connecting)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.close_session_tab(id, window, cx);
                            }))
                            .map(|button| {
                                self.retain_modal_button(("close-restored-tab", id), button, cx)
                            }),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    pub(super) fn render_empty_restored_pane(
        &self,
        pane: keelshell_core::PaneNodeId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let label = t(cx, "空面板，未连接", "Empty pane, disconnected");
        div()
            .id(SharedString::from(format!("empty-pane-{}", pane.0)))
            .test_support()
            .role(Role::Group)
            .aria_label(label)
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.text))
            .border_t_2()
            .border_color(rgb(if self.session_panes.active_pane() == pane {
                visual.accent
            } else {
                visual.border
            }))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().text_color(rgb(visual.muted)).child(label))
            .child(
                Button::new(SharedString::from(format!("select-empty-pane-{}", pane.0)))
                    .ghost()
                    .small()
                    .label(t(cx, "选择此面板", "Select this pane"))
                    .accessibility_label(t(cx, "选择空面板", "Select empty pane"))
                    .selected(self.session_panes.active_pane() == pane)
                    .toggled(self.session_panes.active_pane() == pane)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.active_modal().is_none()
                            && !view.connecting
                            && view.session_panes.tabs_in(pane).is_empty()
                            && view.session_panes.select_pane(pane)
                        {
                            view.active = view.tabs.len();
                            view.refresh_protocol_selection(cx);
                            view.surface_focus.focus(window, cx);
                            cx.notify();
                        }
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("open-empty-pane-{}", pane.0)))
                    .ghost()
                    .small()
                    .label(t(cx, "管理连接", "Manage connections"))
                    .accessibility_label(t(
                        cx,
                        "为此空面板选择连接",
                        "Choose connection for this empty pane",
                    ))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.active_modal().is_none()
                            && !view.connecting
                            && view.session_panes.tabs_in(pane).is_empty()
                            && view.session_panes.select_pane(pane)
                        {
                            view.active = view.tabs.len();
                            view.refresh_protocol_selection(cx);
                            view.open_connections(&OpenConnections, window, cx);
                        }
                    })),
            )
            .into_any_element()
    }
}

fn restored_title(tab: &LayoutTab, state: &AppState, cx: &App) -> String {
    match tab.availability(state) {
        LayoutTabAvailability::Available => state
            .connections
            .iter()
            .find(|profile| Some(profile.id) == tab.connection_id)
            .map(|profile| profile.name.clone())
            .unwrap_or_default(),
        LayoutTabAvailability::Unbound => t(cx, "未保存连接占位", "Unsaved connection").into(),
        LayoutTabAvailability::Missing => t(cx, "连接缺失", "Missing connection").into(),
    }
}

pub(super) fn restored_state(availability: LayoutTabAvailability, cx: &App) -> &'static str {
    match availability {
        LayoutTabAvailability::Available => t(cx, "可用，未连接", "Available, disconnected"),
        LayoutTabAvailability::Unbound => t(cx, "未保存连接占位", "Unsaved connection placeholder"),
        LayoutTabAvailability::Missing => {
            t(cx, "连接缺失或在回收站", "Connection missing or in trash")
        }
    }
}
