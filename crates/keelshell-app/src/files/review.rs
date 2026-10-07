//! File review and editing surfaces change presentation, never approval authority.
use super::*;
use gpui_kit::component::scroll::{ScrollableElement, ScrollbarAxis};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorSurface {
    Draft,
    Patch,
}

impl FilesPanel {
    pub(super) fn expand_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_none() || self.suspended {
            return;
        }
        self.confirmation_expanded = true;
        self.editor_surface = None;
        self.confirmation_scroll.set_offset(point(px(0.), px(0.)));
        self.review_focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn focus_editor(
        &mut self,
        surface: EditorSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.is_none() || self.pending.is_some() {
            return;
        }
        self.editor_surface = Some(surface);
        if surface == EditorSurface::Patch {
            self.patch_visible = true;
        }
        let input = match surface {
            EditorSurface::Draft => &self.editor,
            EditorSurface::Patch => &self.patch,
        };
        // Focus the same state used by the inline editor, after choosing the
        // visible surface. A directory input is never a fallback focus target.
        input.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn scroll_confirmation(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let offset = self.confirmation_scroll.offset();
        let maximum = self.confirmation_scroll.max_offset();
        let next = match event.keystroke.key.as_str() {
            "up" => point(offset.x, offset.y + px(18.)),
            "down" => point(offset.x, offset.y - px(18.)),
            "left" => point(offset.x + px(36.), offset.y),
            "right" => point(offset.x - px(36.), offset.y),
            "pageup" => point(offset.x, offset.y + px(96.)),
            "pagedown" => point(offset.x, offset.y - px(96.)),
            "home" => point(px(0.), px(0.)),
            "end" => point(offset.x, -maximum.y),
            _ => return,
        };
        self.confirmation_scroll.set_offset(point(
            next.x.clamp(-maximum.x, px(0.)),
            next.y.clamp(-maximum.y, px(0.)),
        ));
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn confirm_button(&self, cx: &Context<Self>) -> Button {
        Button::new("confirm-file-operation")
            .primary()
            .compact()
            .rounded(px(6.))
            .label(if matches!(&self.pending, Some((_, Operation::ApplyDirectorySync(review, _))) if review.plan.is_bounded_mirror()) {
                t(cx, "确认镜像及删除", "Confirm mirror/deletions")
            } else {
                t(cx, "确认", "Confirm")
            })
            .on_click(cx.listener(|view, _, window, cx| view.execute_pending(window, cx)))
    }

    pub(super) fn cancel_review_button(&self, cx: &Context<Self>) -> Button {
        Button::new("cancel-file-operation")
            .ghost()
            .compact()
            .rounded(px(6.))
            .label(t(cx, "取消", "Cancel"))
            .on_click(cx.listener(|view, _, _, cx| {
                view.pending = None;
                view.confirmation_expanded = false;
                cx.notify();
            }))
    }

    pub(super) fn expanded_confirmation(
        &self,
        message: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("file-review-expanded")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .text_xs()
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(IconName::FileText)
                    .child(t(cx, "完整文件审核", "Complete file review"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(t(
                                cx,
                                "方向键 / Page Up、Down 浏览",
                                "Arrow keys / Page Up, Down to inspect",
                            )),
                    ),
            )
            .child(
                div()
                    .id("file-review-keyboard-surface")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .p_2()
                    .track_focus(&self.review_focus)
                    .role(accesskit::Role::Group)
                    .aria_label(t(
                        cx,
                        "完整文件审核正文，可横向和纵向滚动",
                        "Complete file review, horizontal and vertical scrolling",
                    ))
                    .on_key_down(
                        cx.listener(|view, event, _, cx| view.scroll_confirmation(event, cx)),
                    )
                    .child(confirmation_text(
                        cx,
                        &message,
                        &self.confirmation_scroll,
                        true,
                    ))
                    .test_support(),
            )
            .child(
                div()
                    .min_h(px(40.))
                    .flex_shrink_0()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(visual.danger_border))
                    .bg(rgb(visual.danger_surface))
                    .child(
                        Button::new("collapse-file-review")
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(t(cx, "收起审核", "Collapse review"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.confirmation_expanded = false;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(self.confirm_button(cx))
                    .child(self.cancel_review_button(cx)),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn editor_surface(
        &self,
        surface: EditorSurface,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let path = self
            .editing
            .as_ref()
            .map(|(path, _)| path.as_str())
            .unwrap_or("");
        let (input, label, other, other_label) = match surface {
            EditorSurface::Draft => (
                &self.editor,
                t(cx, "编辑草稿", "Edit draft"),
                EditorSurface::Patch,
                t(cx, "输入差异", "Enter patch"),
            ),
            EditorSurface::Patch => (
                &self.patch,
                t(cx, "输入单文件差异", "Enter single-file patch"),
                EditorSurface::Draft,
                t(cx, "查看草稿", "View draft"),
            ),
        };
        div()
            .id("file-editor-expanded")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .text_xs()
            .child(
                div()
                    .min_h(px(32.))
                    .flex_shrink_0()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(IconName::FileText)
                    .child(label)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(path.to_owned()),
                    )
                    .child(
                        Button::new("close-file-editor")
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(t(cx, "返回文件", "Back to files"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.editor_surface = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("file-editor-input")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_2()
                    .track_focus(&input.read(cx).focus_handle(cx))
                    .child(
                        Textarea::new(input)
                            .h(relative(1.))
                            .font_family("monospace")
                            .aria_label(format!("{label} · {path}")),
                    )
                    .test_support(),
            )
            .child(
                div()
                    .min_h(px(40.))
                    .flex_shrink_0()
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .child(
                        Button::new("switch-file-editor")
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(other_label)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.focus_editor(other, window, cx)
                            })),
                    )
                    .when(surface == EditorSurface::Patch, |el| {
                        el.child(
                            Button::new("apply-text-patch")
                                .ghost()
                                .compact()
                                .rounded(px(6.))
                                .disabled(self.suspended || self.busy || self.pending.is_some())
                                .label(t(cx, "解析并应用到草稿", "Parse and apply to draft"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.request_patch(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("save-remote-file")
                            .primary()
                            .compact()
                            .rounded(px(6.))
                            .disabled(self.suspended || self.busy || self.pending.is_some())
                            .icon(IconName::Save)
                            .label(t(cx, "审核并保存", "Review save"))
                            .on_click(cx.listener(|view, _, _, cx| view.request_save(cx))),
                    ),
            )
            .child(
                div()
                    .id("file-editor-status")
                    .h(px(24.))
                    .flex_shrink_0()
                    .px_2()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(visual.muted))
                    .role(accesskit::Role::Label)
                    .aria_label(self.status.render(cx))
                    .child(self.status.render(cx))
                    .test_support(),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn editor_entry_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("file-editor-entry-bar")
            .min_h(px(28.))
            .flex_shrink_0()
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .bg(rgb(visual.canvas))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(visual.muted))
                    .child(
                        self.editing
                            .as_ref()
                            .map(|(path, _)| path.clone())
                            .unwrap_or_default(),
                    ),
            )
            .child(
                Button::new("focus-file-draft")
                    .ghost()
                    .compact()
                    .rounded(px(6.))
                    .disabled(self.pending.is_some())
                    .label(t(cx, "编辑草稿", "Edit draft"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.focus_editor(EditorSurface::Draft, window, cx)
                    })),
            )
            .child(
                Button::new("focus-file-patch")
                    .ghost()
                    .compact()
                    .rounded(px(6.))
                    .disabled(self.suspended || self.pending.is_some())
                    .label(t(cx, "输入差异", "Enter patch"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.focus_editor(EditorSurface::Patch, window, cx)
                    })),
            )
            .test_support()
            .into_any_element()
    }
}

pub(super) fn confirmation_text(
    cx: &App,
    message: &str,
    scroll: &ScrollHandle,
    expanded: bool,
) -> AnyElement {
    let visual = crate::design::palette(cx);
    div()
        .id("file-confirmation-message")
        .flex_1()
        .min_w_0()
        .min_h_0()
        .when(!expanded, |el| el.max_h(px(48.)))
        .overflow_y_scroll()
        .overflow_x_scroll()
        .track_scroll(scroll)
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .font_family("monospace")
        .text_xs()
        .text_color(rgb(visual.text))
        .pb_2()
        .children(message.split('\n').enumerate().map(|(index, line)| {
            let line = SharedString::from(line.to_owned());
            div()
                .id(("file-confirmation-line", index))
                .flex_shrink_0()
                .min_h(px(18.))
                .line_height(px(18.))
                .whitespace_nowrap()
                .role(accesskit::Role::Label)
                .aria_label(line.clone())
                .child(line)
                .test_support()
        }))
        .scrollbar(scroll, ScrollbarAxis::Both)
        .test_support()
        .into_any_element()
}
