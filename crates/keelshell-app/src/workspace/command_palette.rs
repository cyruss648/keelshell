//! Workspace integration for the keyboard-first command palette.

use super::*;
use crate::command_palette::{
    self as palette_model, CommandPaletteAction, CommandPaletteContext, CommandPaletteLanguage,
};
use gpui_kit::{assets::IconName, component::Icon};

impl Workspace {
    pub(super) fn open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(kind) = self.active_modal() {
            if kind == modal_scope::ModalKind::CommandPalette {
                self.close_command_palette(window, cx);
            }
            return;
        }
        self.show_command_palette = true;
        self.command_palette_selected = 0;
        self.command_palette_query
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.command_palette_query
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        cx.notify();
    }

    pub(super) fn close_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_command_palette = false;
        self.command_palette_selected = 0;
        self.focus_current_surface(window, cx);
    }

    fn command_palette_context(&self) -> CommandPaletteContext {
        CommandPaletteContext {
            has_session: !self.tabs.is_empty() || self.state.workspace_layout.is_some(),
            assistant_open: self.show_assistant,
            can_previous_session: self.tabs.len() > 1 && self.active > 0,
            can_next_session: self.tabs.len() > 1 && self.active + 1 < self.tabs.len(),
        }
    }

    fn command_palette_language(&self, cx: &App) -> CommandPaletteLanguage {
        match i18n::language(cx) {
            keelshell_core::Language::ZhCn => CommandPaletteLanguage::ZhCn,
            keelshell_core::Language::En => CommandPaletteLanguage::En,
        }
    }

    fn command_palette_matches(&self, cx: &App) -> Vec<palette_model::CommandPaletteMatch> {
        let query = self.command_palette_query.read(cx).value();
        palette_model::search(&query, self.command_palette_context())
    }

    pub(super) fn command_palette_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let matches = self.command_palette_matches(cx);
        match event.keystroke.key.as_str() {
            "down" if !matches.is_empty() => {
                self.command_palette_selected = (self.command_palette_selected + 1) % matches.len();
                cx.stop_propagation();
                cx.notify();
            }
            "up" if !matches.is_empty() => {
                self.command_palette_selected = self
                    .command_palette_selected
                    .checked_sub(1)
                    .unwrap_or(matches.len() - 1);
                cx.stop_propagation();
                cx.notify();
            }
            "enter" => {
                if let Some(matched) = matches.get(self.command_palette_selected) {
                    self.run_command_palette_action(matched.item.action, window, cx);
                    cx.stop_propagation();
                }
            }
            _ => {}
        }
    }

    fn run_command_palette_action(
        &mut self,
        action: CommandPaletteAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_command_palette = false;
        self.command_palette_selected = 0;
        match action {
            CommandPaletteAction::OpenConnectionManager => {
                self.open_connections(&OpenConnections, window, cx);
            }
            CommandPaletteAction::QuickConnect => self.open_quick_connect(window, cx),
            CommandPaletteAction::PreviousSession => {
                self.previous_session(&PreviousSession, window, cx)
            }
            CommandPaletteAction::NextSession => self.next_session(&NextSession, window, cx),
            CommandPaletteAction::ToggleAssistant => {
                self.toggle_assistant(&ToggleAssistant, window, cx)
            }
            CommandPaletteAction::OpenFilePanel => {
                if self.visible_panel != Some(ToolPanel::Files) {
                    self.toggle_panel(ToolPanel::Files, window, cx);
                } else {
                    self.focus_current_surface(window, cx);
                }
            }
            CommandPaletteAction::OpenHostMonitor => {
                self.show_navigation = true;
                self.navigation_panel = NavigationPanel::Monitor;
                if window.viewport_size().width < px(1280.) {
                    self.show_assistant = false;
                }
                self.focus_current_surface(window, cx);
                cx.notify();
            }
            CommandPaletteAction::OpenTunnels => {
                if self.visible_panel != Some(ToolPanel::Tunnels) {
                    self.toggle_panel(ToolPanel::Tunnels, window, cx);
                } else {
                    self.focus_current_surface(window, cx);
                }
            }
            CommandPaletteAction::OpenCommandSnippets => {
                if self.visible_panel != Some(ToolPanel::Commands) {
                    self.toggle_panel(ToolPanel::Commands, window, cx);
                } else {
                    self.focus_current_surface(window, cx);
                }
            }
            CommandPaletteAction::OpenBatchCommands => {
                self.open_batch_commands(false, window, cx);
            }
            CommandPaletteAction::OpenDependencyWorkflow => self.open_workflow(false, window, cx),
            CommandPaletteAction::ToggleAppearance => self.open_appearance(window, cx),
            CommandPaletteAction::OpenCredentialSettings => self.open_vault_settings(window, cx),
            CommandPaletteAction::OpenAiSettings => self.open_ai_settings(window, cx),
            CommandPaletteAction::OpenUpdates => self.open_updates(window, cx),
        }
        cx.notify();
    }

    fn open_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_quick_connect = true;
        self.quick_connect
            .host
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        cx.notify();
    }

    fn command_palette_icon(action: CommandPaletteAction) -> IconName {
        match action {
            CommandPaletteAction::OpenConnectionManager => IconName::FolderOpen,
            CommandPaletteAction::QuickConnect => IconName::Plus,
            CommandPaletteAction::PreviousSession => IconName::ChevronLeft,
            CommandPaletteAction::NextSession => IconName::ChevronRight,
            CommandPaletteAction::ToggleAssistant => IconName::Sparkles,
            CommandPaletteAction::OpenFilePanel => IconName::Folder,
            CommandPaletteAction::OpenHostMonitor => IconName::Activity,
            CommandPaletteAction::OpenTunnels => IconName::Network,
            CommandPaletteAction::OpenCommandSnippets => IconName::Bookmark,
            CommandPaletteAction::OpenBatchCommands => IconName::Server,
            CommandPaletteAction::OpenDependencyWorkflow => IconName::Package,
            CommandPaletteAction::ToggleAppearance => IconName::Settings,
            CommandPaletteAction::OpenCredentialSettings => IconName::Lock,
            CommandPaletteAction::OpenAiSettings => IconName::Sparkles,
            CommandPaletteAction::OpenUpdates => IconName::Info,
        }
    }

    pub(super) fn command_palette_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let context = self.command_palette_context();
        let language = self.command_palette_language(cx);
        let matches = self.command_palette_matches(cx);
        let selected = self
            .command_palette_selected
            .min(matches.len().saturating_sub(1));
        let mut rows = div()
            .id("command-palette-results")
            .test_support()
            .role(accesskit::Role::List)
            .aria_label(t(cx, "可用工作区动作", "Available workspace actions"))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (index, matched) in matches.iter().enumerate() {
            let item = matched.item;
            let action = item.action;
            let shortcut = item.shortcut.map(|hint| {
                if cfg!(target_os = "macos") {
                    hint.macos
                } else {
                    hint.other
                }
            });
            let active = index == selected;
            let mut row = div()
                .id(format!("command-palette-item-{}", item.action.route_key()))
                .test_support()
                .role(accesskit::Role::ListItem)
                .aria_label(item.localized_title(language, context))
                .aria_selected(active)
                .when(active, |row| row.aria_active_descendant())
                .aria_position_in_set(index + 1)
                .aria_size_of_set(matches.len())
                .cursor_pointer()
                .flex_shrink_0()
                .min_h(px(58.))
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .when(active, |row| row.bg(rgb(visual.selected)))
                .when(item.is_active(context), |row| {
                    row.border_l_2().border_color(rgb(visual.accent))
                })
                .when(!active, |row| {
                    row.hover(|style| style.bg(rgb(visual.surface_hover)))
                })
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.run_command_palette_action(action, window, cx)
                }))
                .child(
                    div()
                        .w(px(28.))
                        .h(px(28.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(crate::design::components(cx).control_radius))
                        .bg(rgb(if active {
                            visual.accent
                        } else {
                            visual.surface_active
                        }))
                        .text_color(rgb(if active {
                            visual.primary_text
                        } else {
                            visual.muted
                        }))
                        .child(Icon::new(Self::command_palette_icon(action)).size_4()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .text_ellipsis()
                                .font_weight(if active {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(item.localized_title(language, context)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(visual.muted))
                                .text_ellipsis()
                                .child(item.localized_description(language)),
                        ),
                );
            if let Some(shortcut) = shortcut {
                row = row.child(
                    div()
                        .flex_shrink_0()
                        .px_2()
                        .py_1()
                        .rounded(px(crate::design::components(cx).control_radius))
                        .bg(rgb(visual.surface_active))
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(shortcut),
                );
            }
            rows = rows.child(row);
        }
        let card = div()
            .id("command-palette-card")
            .w(px(720.))
            .h(px(560.))
            .max_w_full()
            .max_h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(crate::design::components(cx).panel_radius))
            .border_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
            .shadow_lg()
            .child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
                                cx,
                                "快速操作中心",
                                "Command palette",
                            )))
                            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                                cx,
                                "↑↓ 选择 · Enter 执行 · Esc 关闭",
                                "↑↓ select · Enter run · Esc close",
                            ))),
                    )
                    .child(
                        Input::new(&self.command_palette_query)
                            .id("command-palette-input")
                            .aria_label(t(cx, "搜索工作区动作", "Search workspace actions")),
                    ),
            )
            .child(rows)
            .child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(format!(
                        "{} {}",
                        matches.len(),
                        t(cx, "项可用动作", "available actions")
                    )),
            )
            .test_support()
            .into_any_element();
        div()
            .id("command-palette-modal")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(visual.overlay))
            .capture_key_down(cx.listener(Self::command_palette_key_down))
            .child(card)
            .test_support()
            .into_any_element()
    }
}
