use super::SnippetEditor;
use crate::{
    design::{ACCENT, BORDER, CANVAS, MUTED, SURFACE, TEXT},
    i18n::t,
};
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
    },
    prelude::FluentBuilder,
    *,
};

fn label(text: &'static str) -> Div {
    div().text_xs().text_color(rgb(MUTED)).child(text)
}

impl Render for SnippetEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_template_feedback(cx);
        div()
            .id("snippet-editor")
            .test_support()
            .track_focus(&self.focus)
            .key_context("SnippetEditor")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(SURFACE))
            .text_color(rgb(TEXT))
            .text_sm()
            .child(
                div()
                    .flex_shrink_0()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(if self.editing {
                        t(cx, "编辑命令片段", "Edit command snippet")
                    } else {
                        t(cx, "新建命令片段", "New command snippet")
                    }))
                    .child(label(t(
                        cx,
                        "保存常用命令，使用时先填入命令栏审核。",
                        "Save reusable commands and review them in the command bar before running.",
                    ))),
            )
            .child(
                div()
                    .id("snippet-editor-form")
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(t(cx, "名称", "Name")))
                            .child(Input::new(&self.name).id("snippet-name").disabled(self.saving)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(t(cx, "说明 · 可选", "Description · optional")))
                            .child(Input::new(&self.description).id("snippet-description").disabled(self.saving)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(t(cx, "标签 · 逗号分隔", "Tags · comma-separated")))
                            .child(Input::new(&self.tags).id("snippet-tags").disabled(self.saving)),
                    )
                    .child(
                        div().flex_shrink_0().flex().flex_col().gap_2()
                            .child(div().flex().items_center().gap_2()
                                .child(div().flex_1().min_w_0().child(label(t(cx,"变量参数 · 显式启用", "Parameters · opt in"))))
                                .child(Button::new("snippet-parameters-toggle").ghost().label(if self.parameterized {t(cx,"已启用", "Enabled")}else{t(cx,"未启用", "Disabled")}).disabled(self.saving).on_click(cx.listener(|editor,_,_,cx|editor.toggle_parameters(cx)))))
                            .when(self.parameterized,|body|body
                                .child(label(t(cx,"使用 {{name}}；填写值时自动作为字面参数引用，不要给变量加引号。", "Use {{name}}. Values are quoted as literal arguments; do not quote placeholders.")))
                                .child(div().text_xs().font_family("monospace").child("cat {{path}} · tar --file={{archive}}"))
                                .when_some(self.template_feedback.as_ref(),|body,feedback| {
                                    if let Some(error)=&feedback.error {
                                        body.child(div().id("snippet-template-feedback").test_support().text_xs().text_color(rgb(0xb42318)).child(error.render(cx)))
                                    } else {
                                        body.child(div().id("snippet-template-feedback").test_support().max_h(px(72.)).overflow_y_scroll().text_xs().text_color(rgb(MUTED)).child(
                                            if feedback.variables.is_empty(){t(cx,"未检测到变量，可保留为无参数模板。", "No variables detected; this can remain a template without parameters.").to_owned()}
                                            else{format!("{}: {}",t(cx,"使用时填写", "Fill when using"),feedback.variables.join(", "))}
                                        ))
                                    }
                                }))
                            .when(!self.parameterized,|body|body.child(label(t(cx,"按原文保存和填入，{{name}} 不会自动转换。", "Saved and inserted verbatim; {{name}} is not interpreted.")))),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(t(cx, "命令 · 支持多行", "Command · multiple lines supported")))
                            .child(
                                div()
                                    .id("snippet-command-container")
                                    .test_support()
                                    .h(px(176.))
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(
                                        Textarea::new(&self.command)
                                            .accessibility_id("snippet-command")
                                            .aria_label(t(cx, "命令内容", "Command text"))
                                            .h(relative(1.))
                                            .font_family("monospace")
                                            .disabled(self.saving),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("snippet-editor-footer")
                    .test_support()
                    .flex_shrink_0()
                    .min_w_0()
                    .p_3()
                    .bg(rgb(CANVAS))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("snippet-plaintext-notice")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(t(
                                cx,
                                "片段以明文保存在本机。请勿填写密码、API 密钥或其他机密。",
                                "Snippets are saved as plain text on this device. Do not include passwords, API keys or other secrets.",
                            )),
                    )
                    .when_some(self.error.as_ref(), |footer, error| {
                        footer.child(
                            div()
                                .id("snippet-editor-error")
                                .test_support()
                                .max_h(px(64.))
                                .overflow_y_scroll()
                                .text_xs()
                                .text_color(rgb(0xb42318))
                                .child(error.render(cx)),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(rgb(ACCENT))
                                    .child(if self.saving { t(cx, "正在保存…", "Saving…") } else { "" }),
                            )
                            .child(
                                Button::new("snippet-editor-cancel")
                                    .ghost()
                                    .label(t(cx, "取消", "Cancel"))
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|editor, _, _, cx| editor.cancel(cx))),
                            )
                            .child(
                                Button::new("snippet-editor-save")
                                    .primary()
                                    .label(t(cx, "保存片段", "Save snippet"))
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|editor, _, window, cx| editor.save(window, cx))),
                            ),
                    ),
            )
    }
}
