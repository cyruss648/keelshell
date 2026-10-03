use super::{JumpHostPicker, PAGE_SIZE, endpoint};
use crate::{
    design::{ACCENT, BORDER, CANVAS, MUTED, SELECTED, SURFACE, TEXT},
    i18n::t,
};
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        input::Input,
    },
    prelude::FluentBuilder,
    *,
};

impl Render for JumpHostPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chosen = self
            .selected
            .and_then(|id| {
                self.state
                    .connections
                    .iter()
                    .find(|connection| connection.id == id)
            })
            .cloned();
        let chosen_route = self.selected.map(|id| self.route(id));
        let mut picker = div()
            .id("jump-host-picker")
            .flex_shrink_0()
            .test_support()
            .track_focus(&self.focus)
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded(px(8.))
            .bg(rgb(CANVAS))
            .border_1()
            .border_color(rgb(BORDER))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xs().text_color(rgb(MUTED)).child(t(
                                cx,
                                "连接方式",
                                "Connection route",
                            )))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(match &chosen {
                                        Some(connection) => format!(
                                            "{} · {}",
                                            t(cx, "经跳板连接", "Via jump host"),
                                            connection.name
                                        ),
                                        None if self.selected.is_some() => t(
                                            cx,
                                            "已选跳板不可用",
                                            "Selected jump host unavailable",
                                        )
                                        .to_owned(),
                                        None => t(cx, "不使用跳板", "No jump host").to_owned(),
                                    }),
                            )
                            .when_some(chosen.as_ref(), |el, connection| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .font_family("monospace")
                                        .text_color(rgb(MUTED))
                                        .child(endpoint(connection)),
                                )
                            }),
                    )
                    .child(
                        Button::new("choose-jump-host")
                            .ghost()
                            .compact()
                            .label(if self.expanded {
                                t(cx, "收起", "Collapse")
                            } else {
                                t(cx, "更改", "Change")
                            })
                            .on_click(
                                cx.listener(|picker, _, window, cx| picker.toggle(window, cx)),
                            ),
                    ),
            );
        if let Some(route) = &chosen_route {
            if let Some(error) = &route.error {
                picker = picker.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xb42318))
                        .child(error.render(cx)),
                );
            } else {
                let names = route
                    .hops
                    .iter()
                    .map(|hop| match &hop.proxy {
                        Some(proxy) => format!(
                            "{} → {}",
                            crate::proxy_editor::proxy_endpoint(proxy),
                            hop.name
                        ),
                        None => hop.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" → ");
                picker = picker.child(
                    div()
                        .id("jump-route-preview")
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(format!(
                            "{names} → {}",
                            t(cx, "当前表单目标", "target in this form")
                        )),
                );
            }
        }
        if !self.expanded {
            return picker;
        }
        let query = self.search.read(cx).value();
        let matches = self
            .state
            .connections
            .iter()
            .filter(|connection| connection.matches(query.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let pages = matches.len().div_ceil(PAGE_SIZE).max(1);
        self.page = self.page.min(pages - 1);
        picker = picker
            .child(Input::new(&self.search).id("jump-host-search"))
            .child(
                Button::new("jump-host-direct")
                    .ghost()
                    .label(t(cx, "不使用 SSH 跳板", "No SSH jump host"))
                    .on_click(cx.listener(|picker, _, window, cx| picker.choose(None, window, cx))),
            );
        let mut rows = div()
            .id("jump-host-options")
            .test_support()
            .max_h(px(216.))
            .overflow_y_scroll();
        for connection in matches.iter().skip(self.page * PAGE_SIZE).take(PAGE_SIZE) {
            let route = self.route(connection.id);
            let id = connection.id;
            let selected = self.selected == Some(id);
            let disabled = route.error.is_some();
            rows = rows.child(
                div()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(rgb(if selected { SELECTED } else { SURFACE }))
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
                            .when_some(connection.proxy.as_ref(), |el, proxy| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MUTED))
                                        .child(crate::proxy_editor::proxy_endpoint(proxy)),
                                )
                            })
                            .when_some(
                                self.state
                                    .folder_id_of(connection.id)
                                    .and_then(|folder| self.state.folder_path(folder)),
                                |el, path| {
                                    el.child(div().text_xs().text_color(rgb(MUTED)).child(path))
                                },
                            )
                            .when(route.hops.len() > 1, |el| {
                                el.child(div().text_xs().text_color(rgb(MUTED)).child(format!(
                                    "{} {}",
                                    route.hops.len(),
                                    t(cx, "站已存路线", "saved route steps")
                                )))
                            })
                            .when_some(route.error, |el, error| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xb42318))
                                        .child(error.render(cx)),
                                )
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("jump-host-select-{id}")))
                            .ghost()
                            .compact()
                            .disabled(disabled)
                            .label(if selected {
                                t(cx, "已选择", "Selected")
                            } else {
                                t(cx, "选择", "Choose")
                            })
                            .on_click(cx.listener(move |picker, _, window, cx| {
                                picker.choose(Some(id), window, cx)
                            })),
                    ),
            );
        }
        if matches.is_empty() {
            rows = rows.child(div().p_3().text_sm().text_color(rgb(MUTED)).child(t(
                cx,
                "没有匹配的已存连接。",
                "No saved connections match this search.",
            )));
        }
        picker.child(rows).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(rgb(ACCENT))
                        .child(format!(
                            "{} / {} · {} {}",
                            self.page + 1,
                            pages,
                            matches.len(),
                            t(cx, "项", "matches")
                        )),
                )
                .child(
                    Button::new("jump-host-previous")
                        .ghost()
                        .compact()
                        .disabled(self.page == 0)
                        .label(t(cx, "上一页", "Previous"))
                        .on_click(cx.listener(|picker, _, _, cx| {
                            picker.page = picker.page.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("jump-host-next")
                        .ghost()
                        .compact()
                        .disabled(self.page + 1 >= pages)
                        .label(t(cx, "下一页", "Next"))
                        .on_click(cx.listener(move |picker, _, _, cx| {
                            picker.page = (picker.page + 1).min(pages - 1);
                            cx.notify();
                        })),
                ),
        )
    }
}
