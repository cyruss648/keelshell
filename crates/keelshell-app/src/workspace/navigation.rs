//! Saved connection navigation beside live sessions, independent of manager filters.
use super::*;
use crate::jump_host_picker::endpoint;
use gpui_kit::{
    assets::IconName,
    component::{Icon, Selectable, tooltip::Tooltip},
};
mod keyboard;
pub(super) mod tree;
use tree::{Entry, NodeId};

/// The left rail's content; switching panels retains the existing entities.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigationPanel {
    #[default]
    Connections,
    Monitor,
}

impl Workspace {
    pub(super) fn navigation_visible(&self, window: &Window) -> bool {
        let width = window.viewport_size().width;
        self.show_navigation
            && !self.tabs.is_empty()
            && width >= px(900.)
            && (!self.show_assistant || width >= px(1280.))
    }

    /// Render the navigation rail inside the width allocated by the workspace.
    pub(super) fn navigation_column(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut selector = div()
            .flex()
            .flex_shrink_0()
            .gap_1()
            .p_2()
            .bg(rgb(visual.chrome))
            .border_b_1()
            .border_color(rgb(visual.border));
        for (id, panel, label) in [
            (
                "navigation-connections",
                NavigationPanel::Connections,
                t(cx, "连接", "Connections"),
            ),
            (
                "navigation-monitor",
                NavigationPanel::Monitor,
                t(cx, "主机信息", "Host info"),
            ),
        ] {
            selector = selector.child(
                Button::new(id)
                    .ghost()
                    .small()
                    .flex_1()
                    .min_w_0()
                    .label(label)
                    .accessibility_label(label)
                    .selected(self.navigation_panel == panel)
                    .toggled(self.navigation_panel == panel)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.active_modal().is_none() {
                            view.navigation_tree.armed = None;
                            if view.navigation_tree.focus.is_focused(window) {
                                view.surface_focus.focus(window, cx);
                            }
                            view.navigation_panel = panel;
                            cx.notify();
                        }
                    }))
                    .map(|button| self.retain_modal_button(id, button, cx)),
            );
        }
        let content = match self.navigation_panel {
            NavigationPanel::Connections => self.navigation_connections(window, cx),
            NavigationPanel::Monitor => self.monitor_column(cx),
        };
        div()
            .id("navigation-column")
            .test_support()
            .role(Role::Group)
            .aria_label(t(cx, "连接与主机监控", "Connections and host monitoring"))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.sidebar))
            .text_color(rgb(visual.text))
            .text_sm()
            .child(selector)
            .child(div().flex_1().min_w_0().min_h_0().child(content))
            .into_any_element()
    }

    fn navigation_connections(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let query = self.search.read(cx).value();
        self.navigation_tree.synchronize(&self.state, &query);
        if !self.navigation_tree.focus.is_focused(window) {
            self.navigation_tree.armed = None;
        }
        let mut rows = div()
            .id("navigation-rows")
            .test_support()
            .role(Role::Tree)
            .aria_label(t(cx, "已保存连接", "Saved connections"))
            .aria_description(t(cx, "方向键浏览，Enter 打开，左右键折叠或展开文件夹。", "Use arrow keys to browse, Enter to open, and Left or Right to collapse or expand folders."))
            .track_focus(&self.navigation_tree.focus)
            .tab_index(0)
            .track_scroll(&self.navigation_tree.scroll)
            .on_key_down(cx.listener(Self::navigation_key_down))
            .on_key_up(cx.listener(Self::navigation_key_up))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .px_2()
            .pb_2()
            .flex()
            .flex_col()
            .gap_1();
        for entry in &self.navigation_tree.visible {
            rows = rows.child(self.navigation_tree_row(entry, window, cx));
        }
        if self.navigation_tree.visible.is_empty() {
            let message = if self.state.connections.is_empty() {
                t(
                    cx,
                    "尚未保存 SSH 连接。点击“新建连接”添加。",
                    "No saved SSH connections. Choose Add connection to create one.",
                )
            } else {
                t(
                    cx,
                    "没有匹配的连接。请修改或清空搜索。",
                    "No matching connections. Edit or clear the search.",
                )
            };
            rows = rows.child(
                div()
                    .id("navigation-empty")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(message)
                    .p_2()
                    .text_color(rgb(visual.muted))
                    .child(message),
            );
        }
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_1()
                            .child(
                                div()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t(cx, "已保存连接", "Saved connections")),
                            )
                            .child(
                                Button::new("navigation-add-connection")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .icon(IconName::Plus)
                                    .accessibility_label(t(cx, "新建连接", "Add connection"))
                                    .tooltip(t(cx, "新建连接", "Add connection"))
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        if view.active_modal().is_none() {
                                            view.open_form(window, cx);
                                        }
                                    }))
                                    .map(|button| {
                                        self.retain_modal_button(
                                            "navigation-add-connection",
                                            button,
                                            cx,
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .capture_key_down(cx.listener(
                                |view, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.key == "down"
                                        && event.keystroke.modifiers == Modifiers::default()
                                        && view.active_modal().is_none()
                                        && view.search.read(cx).focus_handle(cx).is_focused(window)
                                    {
                                        view.navigation_tree.synchronize(
                                            &view.state,
                                            &view.search.read(cx).value(),
                                        );
                                        if let Some(id) = view.navigation_tree.cursor {
                                            view.navigation_tree.select(id);
                                            view.navigation_tree.focus.focus(window, cx);
                                            cx.notify();
                                        }
                                        view.navigation_tree.held.insert("down".into());
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                            .child(
                                Input::new(&self.search)
                                    .id("navigation-search")
                                    .small()
                                    .aria_label(t(
                                        cx,
                                        "搜索已保存连接",
                                        "Search saved connections",
                                    )),
                            ),
                    ),
            )
            .child(rows)
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .p_2()
                    .child(
                        Button::new("navigation-manage-connections")
                            .ghost()
                            .small()
                            .w_full()
                            .label(t(cx, "管理连接", "Manage connections"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_connections(&OpenConnections, window, cx);
                            }))
                            .map(|button| {
                                self.retain_modal_button(
                                    "navigation-manage-connections",
                                    button,
                                    cx,
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn navigation_tree_row(
        &self,
        entry: &Entry,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let id = entry.id;
        let cursor = self.navigation_tree.cursor == Some(id);
        let siblings: Vec<_> = self
            .navigation_tree
            .visible
            .iter()
            .filter(|row| row.parent == entry.parent)
            .collect();
        let position = siblings
            .iter()
            .position(|row| row.id == id)
            .unwrap_or_default();
        let base = gpui_kit::base::Button::new(SharedString::from(id.element_id()))
            .focusable(false)
            .justify_start()
            .role(Role::TreeItem)
            .aria_level(entry.depth + 1)
            .aria_position_in_set(position + 1)
            .aria_size_of_set(siblings.len())
            .aria_selected(cursor)
            .when(cursor, |row| row.aria_active_descendant())
            .flex_shrink_0()
            .min_w_0()
            .w_full()
            .border_1()
            .border_color(rgb(visual.sidebar))
            .rounded(px(crate::design::components(cx).control_radius))
            .pl(px(8. + entry.depth.min(4) as f32 * 10.))
            .pr_2()
            .py_1()
            .line_height(relative(1.3))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(visual.surface)))
            .active(|style| style.bg(rgb(visual.selected)))
            .when(cursor, |row| {
                row.bg(rgb(visual.selected))
                    .when(self.navigation_tree.focus.is_focused(window), |row| {
                        row.border_color(rgb(visual.accent))
                    })
            })
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(cx.listener(move |view, _, window, cx| {
                view.activate_navigation_node(id, window, cx);
            }));
        let NodeId::Connection(profile_id) = id else {
            let name = if id == NodeId::Unfiled {
                t(cx, "未归档", "Unfiled").to_owned()
            } else {
                entry.name.clone()
            };
            let path = match id {
                NodeId::Folder(folder) => self.folder_label(Some(folder), cx),
                _ => name.clone(),
            };
            let label = format!("{}: {path}", t(cx, "文件夹", "Folder"));
            return base
                .h(px(32.))
                .aria_label(label.clone())
                .when(entry.has_children, |row| row.aria_expanded(entry.expanded))
                .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Icon::new(if entry.expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .size_3()
                    .when(!entry.has_children, |icon| icon.invisible()),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_ellipsis()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name),
                )
                .into_any_element();
        };
        let Some(connection) = self
            .state
            .connections
            .iter()
            .find(|profile| profile.id == profile_id)
        else {
            return div().into_any_element();
        };
        let live = self.navigation_live_tab(profile_id, cx);
        let selected = live == Some(self.active);
        let state = if selected {
            t(cx, "当前已连接会话", "Current connected session")
        } else if live.is_some() {
            t(cx, "已连接", "Connected")
        } else {
            t(cx, "已保存", "Saved")
        };
        let action = if live.is_some() {
            t(cx, "切换到会话", "Switch to session")
        } else {
            t(cx, "连接", "Connect")
        };
        let endpoint = endpoint(connection);
        let label = format!("{action}: {}\n{endpoint}\n{state}", connection.name);
        base.h(px(66.))
            .aria_label(label.clone())
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
            .disabled(
                live.is_none() && (self.connecting || self.saving || self.connect_route.is_some()),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_ellipsis()
                            .child(connection.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(visual.muted))
                            .text_ellipsis()
                            .child(endpoint),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if live.is_some() {
                                visual.success
                            } else {
                                visual.muted
                            }))
                            .text_ellipsis()
                            .child(state),
                    ),
            )
            .into_any_element()
    }

    fn navigation_live_tab(&self, profile: uuid::Uuid, cx: &App) -> Option<usize> {
        let matches = |tab: &Entity<TerminalView>| {
            if self.remote_connection_ids.get(&tab.entity_id()) != Some(&profile) {
                return false;
            }
            if self
                .reconnect_bindings
                .get(&tab.entity_id())
                .is_some_and(|binding| !self.binding_current(binding))
            {
                // Editing a saved endpoint or jump route does not move an
                // already connected shell. Never label that old owner as this
                // destination or reuse it through the updated profile's row.
                return false;
            }
            let terminal = tab.read(cx);
            terminal.is_open()
                && terminal
                    .subscribe_lifecycle()
                    .is_none_or(|source| source.has_changed().is_ok())
                && self
                    .remote_sessions
                    .get(&tab.entity_id())
                    .is_none_or(|session| !session.is_closed())
        };
        // Split panes may share a saved UUID. Keep the selected live pane when
        // possible, then use tab order rather than nondeterministic map order.
        if self.tabs.get(self.active).is_some_and(matches) {
            Some(self.active)
        } else {
            self.tabs.iter().position(matches)
        }
    }

    fn activate_navigation_connection(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal().is_some() {
            return;
        }
        // A painted row can outlive an edit, deletion or tab replacement.
        // Retain only the UUID and resolve both owners at activation time.
        let Some(connection) = self
            .state
            .connections
            .iter()
            .find(|profile| profile.id == id)
        else {
            self.status = Message::new(
                "连接已移除，请重新选择。",
                "Connection removed. Select another saved connection.",
            );
            cx.notify();
            return;
        };
        if let Some(index) = self.navigation_live_tab(id, cx) {
            self.select_session_tab(index, window, cx);
        } else {
            self.request_connect(connection.clone(), window, cx);
        }
    }
}
