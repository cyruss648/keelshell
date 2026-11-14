use super::SnippetEditor;
use crate::i18n::t;
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
    },
    prelude::FluentBuilder,
    *,
};

fn label(cx: &App, text: &'static str) -> Div {
    let visual = crate::design::palette(cx);
    div().text_xs().text_color(rgb(visual.muted)).child(text)
}

impl Render for SnippetEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
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
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .text_sm()
            .child(
                div()
                    .flex_shrink_0()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(if self.editing {
                        t(cx, "编辑命令片段", "Edit command snippet")
                    } else {
                        t(cx, "新建命令片段", "New command snippet")
                    }))
                    .child(label(cx, t(
                        cx,
                        if self.ai_reviewed {
                            "AI 命令已审核；请填写片段信息并再次确认保存。"
                        } else {
                            "保存常用命令，使用时先填入命令栏审核。"
                        },
                        if self.ai_reviewed {
                            "The AI command was reviewed; complete the snippet details and confirm saving."
                        } else {
                            "Save reusable commands and review them in the command bar before running."
                        },
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
                            .child(label(cx, t(cx, "名称", "Name")))
                            .child(
                                Input::new(&self.name)
                                    .id("snippet-name")
                                    .accessibility_id("snippet-name")
                                    .aria_label(t(cx, "片段名称", "Snippet name"))
                                    .disabled(self.saving),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(cx, t(cx, "说明 · 可选", "Description · optional")))
                            .child(
                                Input::new(&self.description)
                                    .id("snippet-description")
                                    .accessibility_id("snippet-description")
                                    .aria_label(t(cx, "片段说明", "Snippet description"))
                                    .disabled(self.saving),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(cx, t(cx, "标签 · 逗号分隔", "Tags · comma-separated")))
                            .child(
                                Input::new(&self.tags)
                                    .id("snippet-tags")
                                    .accessibility_id("snippet-tags")
                                    .aria_label(t(cx, "片段标签", "Snippet tags"))
                                    .disabled(self.saving),
                            ),
                    )
                    .child(
                        div().flex_shrink_0().flex().flex_col().gap_2()
                            .child(div().flex().items_center().gap_2()
                                .child(div().flex_1().min_w_0().child(label(cx, t(cx,"变量参数 · 显式启用", "Parameters · opt in"))))
                                .child(Button::new("snippet-parameters-toggle").ghost().label(if self.parameterized {t(cx,"已启用", "Enabled")}else{t(cx,"未启用", "Disabled")}).disabled(self.saving).on_click(cx.listener(|editor,_,_,cx|editor.toggle_parameters(cx)))))
                            .when(self.parameterized,|body|body
                                .child(label(cx, t(cx,"使用 {{name}}；填写值时自动作为字面参数引用，不要给变量加引号。", "Use {{name}}. Values are quoted as literal arguments; do not quote placeholders.")))
                                .child(div().text_xs().font_family("monospace").child("cat {{path}} · tar --file={{archive}}"))
                                .when_some(self.template_feedback.as_ref(),|body,feedback| {
                                    if let Some(error)=&feedback.error {
                                        body.child(div().id("snippet-template-feedback").test_support().text_xs().text_color(rgb(visual.danger)).child(error.render(cx)))
                                    } else {
                                        body.child(div().id("snippet-template-feedback").test_support().max_h(px(72.)).overflow_y_scroll().text_xs().text_color(rgb(visual.muted)).child(
                                            if feedback.variables.is_empty(){t(cx,"未检测到变量，可保留为无参数模板。", "No variables detected; this can remain a template without parameters.").to_owned()}
                                            else{format!("{}: {}",t(cx,"使用时填写", "Fill when using"),feedback.variables.join(", "))}
                                        ))
                                    }
                                }))
                            .when(!self.parameterized,|body|body.child(label(cx, t(cx,"按原文保存和填入，{{name}} 不会自动转换。", "Saved and inserted verbatim; {{name}} is not interpreted.")))),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(label(
                                        cx,
                                        t(cx, "命令 · 支持多行", "Command · multiple lines supported"),
                                    ))
                                    .when(self.command_locked, |row| {
                                        row.child(
                                            div()
                                                .id("snippet-command-locked")
                                                .test_support()
                                                .text_xs()
                                                .text_color(rgb(visual.accent))
                                                .child(t(
                                                    cx,
                                                    "已审核正文 · 只读",
                                                    "Reviewed body · read-only",
                                                )),
                                        )
                                    }),
                            )
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
                                            .disabled(self.saving || self.command_locked),
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
                    .bg(rgb(visual.canvas))
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("snippet-plaintext-notice")
                            .test_support()
                            .text_xs()
                            .text_color(rgb(visual.muted))
                            .child(t(
                                cx,
                                "片段以明文保存在本机。请勿填写密码、API 密钥或其他机密。",
                                "Snippets are saved as plain text on this device. Do not include passwords, API keys or other secrets.",
                            )),
                    )
                    .when(self.ai_reviewed, |footer| {
                        footer.child(
                            div()
                                .id("snippet-ai-review-notice")
                                .test_support()
                                .text_xs()
                                .text_color(rgb(visual.muted))
                                .child(t(
                                    cx,
                                    "要修改命令，请取消当前保存流程并重新审核；不会自动执行命令。",
                                    "To change the command, cancel this save flow and review it again. Nothing runs automatically.",
                                )),
                        )
                    })
                    .when_some(self.error.as_ref(), |footer, error| {
                        footer.child(
                            div()
                                .id("snippet-editor-error")
                                .test_support()
                                .max_h(px(64.))
                                .overflow_y_scroll()
                                .text_xs()
                                .text_color(rgb(visual.danger))
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
                                    .text_color(rgb(visual.accent))
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
                                    .label(if self.ai_reviewed && !self.save_confirmed {
                                        t(cx, "确认保存片段", "Confirm save snippet")
                                    } else {
                                        t(cx, "保存片段", "Save snippet")
                                    })
                                    .disabled(self.saving)
                                    .on_click(cx.listener(|editor, _, window, cx| editor.save(window, cx))),
                            )
                            .when(self.ai_reviewed, |actions| {
                                actions.child(
                                    Button::new("snippet-editor-re-review")
                                        .ghost()
                                        .label(t(
                                            cx,
                                            "取消并重新审核命令",
                                            "Cancel and re-review command",
                                        ))
                                        .disabled(self.saving)
                                        .on_click(cx.listener(|editor, _, _, cx| {
                                            editor.re_review(cx)
                                        })),
                                )
                            }),
                    ),
            )
    }
}
