//! Registry-driven native theme selection, using the existing serialized save path.

use super::*;
use crate::design::{self, Palette, ThemeDefinition};
use gpui_kit::base::Selectable;
use keelshell_core::Theme;
use modal_scope::ModalKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThemeCardState {
    current_family: bool,
    recommended_preset_applied: bool,
}

impl ThemeCardState {
    fn for_theme(
        current_family_id: &str,
        current_appearance: Theme,
        theme: &ThemeDefinition,
    ) -> Self {
        let current_family = current_family_id == theme.id;
        Self {
            current_family,
            recommended_preset_applied: current_family
                && current_appearance == theme.preferred_appearance,
        }
    }
}

fn appearance_label(cx: &App, appearance: Theme) -> &'static str {
    match appearance {
        Theme::System => t(cx, "跟随系统", "System"),
        Theme::Light => t(cx, "浅色", "Light"),
        Theme::Dark => t(cx, "深色", "Dark"),
    }
}

impl Workspace {
    pub(super) fn open_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving
            || self.state_reloading
            || self
                .active_modal()
                .is_some_and(|kind| !matches!(kind, ModalKind::Manager | ModalKind::Appearance))
        {
            return;
        }
        self.show_appearance = true;
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    fn select_theme_family(
        &mut self,
        theme: &'static ThemeDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal() != Some(ModalKind::Appearance) || self.saving || self.state_reloading
        {
            return;
        }
        let Ok(id) = keelshell_core::ThemeId::new(theme.id) else {
            // An invalid built-in must never reach persistence. Registry tests
            // catch this during development; production fails without a panic.
            self.status = Message::new("主题定义无效", "Invalid theme definition");
            cx.notify();
            return;
        };
        if self.state.settings.theme_family == id
            && self.state.settings.theme == theme.preferred_appearance
        {
            return;
        }
        let mut candidate = self.state.clone();
        candidate.settings.theme_family = id;
        candidate.settings.theme = theme.preferred_appearance;
        // Applying after successful persistence keeps the preview and saved
        // preference consistent on Conflict, I/O failure or durability unknown.
        self.persist(candidate, AfterSave::Theme, window, cx);
    }

    pub(super) fn appearance_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = design::palette(cx);
        let busy = self.saving || self.state_reloading;
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .id("theme-preferences")
                    .test_support()
                    .w(px(860.))
                    .max_w_full()
                    .max_h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(design::components(cx).panel_radius))
                    .border_1()
                    .border_color(rgb(p.border))
                    .bg(rgb(p.surface))
                    .shadow_lg()
                    .child(
                        div()
                            .p_5()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(p.accent)),
                                    )
                                    .child(
                                        div().text_xl().font_weight(FontWeight::SEMIBOLD).child(t(
                                            cx,
                                            "主题与外观",
                                            "Themes and appearance",
                                        )),
                                    )
                                    .child(
                                        div()
                                            .ml_auto()
                                            .px_2()
                                            .py_1()
                                            .rounded_full()
                                            .bg(rgb(p.selected))
                                            .text_xs()
                                            .text_color(rgb(p.accent))
                                            .child(format!(
                                                "{} {}",
                                                design::registry().len(),
                                                t(cx, "套主题", "themes")
                                            )),
                                    ),
                            )
                            .child(div().text_sm().text_color(rgb(p.muted)).child(t(
                                cx,
                                "选择一套配色，再按习惯调整明暗模式。",
                                "Choose a color family, then adjust its appearance.",
                            ))),
                    )
                    .child(
                        div()
                            .id("theme-catalog-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px_5()
                            .pb_5()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
                                            cx,
                                            "主题家族",
                                            "Theme families",
                                        )),
                                    )
                                    .child(div().flex_1().h(px(1.)).bg(rgb(p.border)))
                                    .child(div().text_xs().text_color(rgb(p.muted)).child(t(
                                        cx,
                                        "选择后立即保存",
                                        "Saved when selected",
                                    ))),
                            )
                            .child(
                                div().flex().flex_wrap().gap_3().children(
                                    design::registry()
                                        .iter()
                                        .map(|theme| self.theme_card(theme, cx)),
                                ),
                            ),
                    )
                    .child(self.appearance_mode_section(busy, p, cx))
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_5()
                            .py_3()
                            .border_t_1()
                            .border_color(rgb(p.border))
                            .bg(rgb(p.chrome))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("theme-save-status")
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(rgb(p.muted))
                                    .child(self.status.render(cx)),
                            )
                            .child(
                                Button::new("theme-done")
                                    .label(t(cx, "完成", "Done"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.close_active_modal(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn appearance_mode_section(
        &self,
        busy: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("theme-appearance-section")
            .test_support()
            .flex_shrink_0()
            .mx_5()
            .mb_4()
            .p_3()
            .rounded(px(design::components(cx).panel_radius))
            .border_1()
            .border_color(rgb(p.border))
            .bg(rgb(p.canvas))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(t(cx, "明暗模式", "Appearance"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child(t(
                                cx,
                                "三套主题均支持浅色、深色和跟随系统。",
                                "Every family supports light, dark and system modes.",
                            )),
                    ),
            )
            .children(
                [
                    (
                        keelshell_core::Theme::System,
                        "theme-system",
                        "跟随系统",
                        "System",
                    ),
                    (keelshell_core::Theme::Light, "theme-light", "浅色", "Light"),
                    (keelshell_core::Theme::Dark, "theme-dark", "深色", "Dark"),
                ]
                .into_iter()
                .map(|(mode, id, zh, en)| {
                    Button::new(id)
                        .label(t(cx, zh, en))
                        .selected(self.state.settings.theme == mode)
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.select_theme(mode, window, cx)
                        }))
                }),
            )
            .when(
                !design::registry()
                    .iter()
                    .any(|theme| theme.id == self.state.settings.theme_family.as_str()),
                |body| {
                    body.child(
                        div().text_sm().text_color(rgb(p.warning)).child(t(
                            cx,
                            "已保存的主题暂不可用，当前显示霜白。选择新主题后才会覆盖原偏好。",
                            "The saved family is unavailable. Frost is shown until you choose a replacement.",
                        )),
                    )
                },
            )
            .into_any_element()
    }

    fn theme_card(&self, theme: &'static ThemeDefinition, cx: &mut Context<Self>) -> AnyElement {
        let p = theme.palette(design::resolve(
            theme.preferred_appearance,
            WindowAppearance::Light,
        ));
        let state = ThemeCardState::for_theme(
            self.state.settings.theme_family.as_str(),
            self.state.settings.theme,
            theme,
        );
        let visual = design::palette(cx);
        let recommended = appearance_label(cx, theme.preferred_appearance);
        let current = appearance_label(cx, self.state.settings.theme);
        let action_label = if state.recommended_preset_applied {
            format!("{} · {recommended}", t(cx, "已应用", "Applied"))
        } else if state.current_family {
            format!("{} · {recommended}", t(cx, "应用推荐", "Use recommended"))
        } else {
            format!("{} · {recommended}", t(cx, "应用", "Apply"))
        };
        let accessibility_label = if state.current_family {
            format!(
                "{} · {}：{current} · {}：{recommended} · {action_label}",
                t(cx, theme.name_zh, theme.name_en),
                t(cx, "当前明暗", "Current appearance"),
                t(cx, "推荐外观", "Recommended appearance"),
            )
        } else {
            format!(
                "{} · {}：{recommended} · {action_label}",
                t(cx, theme.name_zh, theme.name_en),
                t(cx, "推荐外观", "Recommended appearance"),
            )
        };
        div()
            .id(format!("theme-card-{}", theme.id))
            .test_support()
            .flex_1()
            .min_w(px(220.))
            .p_2()
            .rounded(px(theme.components.panel_radius + 2.))
            .border_1()
            .border_color(rgb(if state.current_family {
                visual.accent
            } else {
                visual.border
            }))
            .bg(rgb(if state.current_family {
                visual.selected
            } else {
                visual.surface
            }))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id(format!("theme-preview-{}", theme.id))
                    .test_support()
                    .h(px(136.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(theme.components.panel_radius))
                    .border_1()
                    .border_color(rgb(p.border))
                    .child(
                        div()
                            .h(px(28.))
                            .px_2()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .bg(rgb(p.chrome))
                            .border_b_1()
                            .border_color(rgb(p.border))
                            .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(rgb(p.accent)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(rgb(p.text))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(t(cx, "工作台预览", "Workbench preview")),
                            )
                            .child(
                                div()
                                    .px_1()
                                    .rounded_full()
                                    .bg(rgb(p.surface_active))
                                    .text_color(rgb(p.accent))
                                    .text_xs()
                                    .child(t(cx, "就绪", "Ready")),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .id(format!("theme-preview-sidebar-{}", theme.id))
                                    .test_support()
                                    .w(px(48.))
                                    .flex_shrink_0()
                                    .p_2()
                                    .bg(rgb(p.sidebar))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().h(px(5.)).w(px(24.)).rounded_full().bg(rgb(p.accent)),
                                    )
                                    .child(
                                        div().h(px(5.)).w(px(30.)).rounded_full().bg(rgb(p.border)),
                                    )
                                    .child(
                                        div().h(px(5.)).w(px(19.)).rounded_full().bg(rgb(p.border)),
                                    ),
                            )
                            .child(
                                div()
                                    .id(format!("theme-preview-workspace-{}", theme.id))
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .p_2()
                                    .bg(rgb(p.canvas))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .h(px(5.))
                                                    .w(px(30.))
                                                    .rounded_full()
                                                    .bg(rgb(p.accent)),
                                            )
                                            .child(
                                                div()
                                                    .h(px(5.))
                                                    .flex_1()
                                                    .rounded_full()
                                                    .bg(rgb(p.border)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id(format!("theme-preview-terminal-{}", theme.id))
                                            .test_support()
                                            .flex_1()
                                            .m_2()
                                            .p_2()
                                            .rounded(px(theme.components.control_radius))
                                            .bg(rgb(theme.terminal.background))
                                            .text_color(rgb(theme.terminal.foreground))
                                            .font_family(theme.typography.code_font_family)
                                            .font_weight(theme.typography.code_font_weight)
                                            .text_xs()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child("Aa 012345")
                                            .child(
                                                div()
                                                    .text_color(rgb(theme.terminal.cursor))
                                                    .child("> _"),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(20.))
                            .flex_shrink_0()
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .bg(rgb(p.chrome))
                            .border_t_1()
                            .border_color(rgb(p.border))
                            .child(div().h(px(4.)).w(px(44.)).rounded_full().bg(rgb(p.border)))
                            .child(
                                div()
                                    .h(px(4.))
                                    .flex_1()
                                    .rounded_full()
                                    .bg(rgb(p.surface_active)),
                            ),
                    ),
            )
            .child(
                div()
                    .id(format!("theme-card-heading-{}", theme.id))
                    .test_support()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(p.accent)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(t(cx, theme.name_zh, theme.name_en)),
                    )
                    .when(state.current_family, |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded(px(theme.components.control_radius))
                                .bg(rgb(visual.selected))
                                .text_color(rgb(visual.accent))
                                .text_xs()
                                .child(t(cx, "当前族", "Current family")),
                        )
                    }),
            )
            .child(
                div()
                    .id(format!("theme-card-details-{}", theme.id))
                    .test_support()
                    .min_h(px(36.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(cx, theme.description_zh, theme.description_en))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(5.)).h(px(5.)).rounded_full().bg(rgb(p.accent)))
                            .child(format!(
                                "{}：{recommended}",
                                t(cx, "推荐外观", "Recommended appearance")
                            )),
                    )
                    .when(state.current_family, |body| {
                        body.child(format!(
                            "{}：{current}",
                            t(cx, "当前明暗", "Current appearance")
                        ))
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("theme-family-{}", theme.id)))
                    .w_full()
                    .label(action_label)
                    .accessibility_label(accessibility_label)
                    .selected(state.recommended_preset_applied)
                    .disabled(self.saving || self.state_reloading)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_theme_family(theme, window, cx)
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeCardState;
    use crate::design::registry;
    use keelshell_core::Theme;

    #[test]
    fn theme_card_state_separates_family_from_recommended_preset() {
        let graphite = registry()
            .iter()
            .find(|theme| theme.id == "graphite")
            .unwrap_or_else(|| panic!("graphite is a registered theme"));

        let family_with_custom_appearance =
            ThemeCardState::for_theme(graphite.id, Theme::Light, graphite);
        assert!(family_with_custom_appearance.current_family);
        assert!(!family_with_custom_appearance.recommended_preset_applied);

        let applied_preset = ThemeCardState::for_theme(graphite.id, Theme::Dark, graphite);
        assert!(applied_preset.current_family);
        assert!(applied_preset.recommended_preset_applied);

        let different_family = ThemeCardState::for_theme("frost", Theme::Dark, graphite);
        assert!(!different_family.current_family);
        assert!(!different_family.recommended_preset_applied);
    }
}
