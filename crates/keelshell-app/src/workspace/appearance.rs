//! Registry-driven native theme selection, using the existing serialized save path.

use super::*;
use crate::design::{self, ThemeDefinition};
use gpui_kit::base::Selectable;
use modal_scope::ModalKind;

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
                    .child(
                        div()
                            .p_5()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t(cx, "主题与外观", "Themes and appearance")),
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
                            .min_h_0()
                            .overflow_y_scroll()
                            .px_5()
                            .pb_5()
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                div().flex().flex_wrap().gap_3().children(
                                    design::registry()
                                        .iter()
                                        .map(|theme| self.theme_card(theme, cx)),
                                ),
                            )
                            .child(
                                div()
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
                                    .children([
                                        (keelshell_core::Theme::System, "theme-system", "跟随系统", "System"),
                                        (keelshell_core::Theme::Light, "theme-light", "浅色", "Light"),
                                        (keelshell_core::Theme::Dark, "theme-dark", "深色", "Dark"),
                                    ].into_iter().map(|(mode, id, zh, en)| {
                                        Button::new(id)
                                            .label(t(cx, zh, en))
                                            .selected(self.state.settings.theme == mode)
                                            .disabled(busy)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.select_theme(mode, window, cx)
                                            }))
                                    })),
                            )
                            .when(
                                !design::registry().iter().any(|theme| theme.id == self.state.settings.theme_family.as_str()),
                                |body| body.child(
                                    div().text_sm().text_color(rgb(p.warning)).child(t(
                                        cx,
                                        "已保存的主题暂不可用，当前显示霜白。选择新主题后才会覆盖原偏好。",
                                        "The saved family is unavailable. Frost is shown until you choose a replacement.",
                                    )),
                                ),
                            ),
                    )
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

    fn theme_card(&self, theme: &'static ThemeDefinition, cx: &mut Context<Self>) -> AnyElement {
        let p = theme.palette(design::resolve(
            theme.preferred_appearance,
            WindowAppearance::Light,
        ));
        let active = self.state.settings.theme_family.as_str() == theme.id;
        let visual = design::palette(cx);
        div()
            .flex_1()
            .min_w(px(220.))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .h(px(124.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(theme.components.panel_radius))
                    .border_2()
                    .border_color(rgb(if active { visual.accent } else { visual.border }))
                    .bg(rgb(p.canvas))
                    .child(
                        div()
                            .h(px(24.))
                            .px_2()
                            .bg(rgb(p.chrome))
                            .text_color(rgb(p.muted))
                            .text_xs()
                            .flex()
                            .items_center()
                            .child(t(cx, "配色预览", "Color preview")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .w(px(46.))
                                    .p_2()
                                    .bg(rgb(p.sidebar))
                                    .child(div().h(px(5.)).bg(rgb(p.accent)).rounded(px(2.))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .m_2()
                                    .p_2()
                                    .rounded(px(theme.components.control_radius))
                                    .bg(rgb(theme.terminal.background))
                                    .text_color(rgb(theme.terminal.foreground))
                                    .font_family("monospace")
                                    .text_xs()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child("Aa 012345")
                                    .child(
                                        div().text_color(rgb(theme.terminal.cursor)).child("> _"),
                                    ),
                            ),
                    ),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                cx,
                theme.name_zh,
                theme.name_en,
            )))
            .child(
                div()
                    .h(px(36.))
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(t(cx, theme.description_zh, theme.description_en)),
            )
            .child(
                Button::new(SharedString::from(format!("theme-family-{}", theme.id)))
                    .w_full()
                    .label(
                        if theme.preferred_appearance == keelshell_core::Theme::Dark {
                            t(cx, "应用 · 深色", "Apply · Dark")
                        } else {
                            t(cx, "应用 · 浅色", "Apply · Light")
                        },
                    )
                    .accessibility_label(format!(
                        "{} · {}",
                        t(cx, theme.name_zh, theme.name_en),
                        t(cx, "应用推荐外观", "Apply recommended appearance"),
                    ))
                    .selected(active)
                    .disabled(self.saving || self.state_reloading)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_theme_family(theme, window, cx)
                    })),
            )
            .into_any_element()
    }
}
