use super::*;
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        input::Textarea,
    },
    prelude::FluentBuilder,
};

impl Render for SnippetParameters {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let mut fields = div().flex().flex_col().gap_3().flex_shrink_0();
        for (index, parameter) in self.parameters.iter().enumerate() {
            let empty = parameter.allow_empty;
            fields = fields.child(
                div()
                    .id(("snippet-parameter-row", index))
                    .test_support()
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
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .font_family("monospace")
                                    .child(parameter.name.clone()),
                            )
                            .child(
                                Button::new(("snippet-parameter-empty", index))
                                    .ghost()
                                    .compact()
                                    .label(if empty {
                                        t(cx, "✓ 使用空值", "✓ Empty value")
                                    } else {
                                        t(cx, "使用空值", "Use empty")
                                    })
                                    .disabled(
                                        self.saving || !parameter.value.read(cx).value().is_empty(),
                                    )
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.toggle_empty(index, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id(("snippet-parameter-input", index))
                            .test_support()
                            .h(px(64.))
                            .min_w_0()
                            .overflow_hidden()
                            .child(
                                Textarea::new(&parameter.value)
                                    .h_full()
                                    .font_family("monospace")
                                    .disabled(self.saving),
                            ),
                    ),
            );
        }
        div().id("snippet-parameters").test_support().track_focus(&self.focus).key_context("SnippetParameters")
            .size_full().min_w_0().min_h_0().flex().flex_col().overflow_hidden().bg(rgb(visual.surface)).text_color(rgb(visual.text)).text_sm()
            .child(div().flex_shrink_0().p_4().border_b_1().border_color(rgb(visual.border)).flex().flex_col().gap_1()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,"填写片段参数", "Fill snippet parameters")))
                .child(div().min_w_0().text_ellipsis().child(self.snippet.name.clone()))
                .child(div().id("snippet-parameters-target").test_support().text_xs().text_color(rgb(visual.accent)).min_w_0().text_ellipsis().child(self.target.clone())))
            .child(div().id("snippet-parameters-body").test_support().flex_1().min_w_0().min_h_0().overflow_y_scroll().p_4().flex().flex_col().gap_4()
                .child(div().flex_shrink_0().text_xs().text_color(rgb(visual.muted)).child(t(cx,"参数仅本次使用，不保存为片段；填入后仍需审核执行。", "Values are used only this time and are not saved in the snippet. Review again before running.")))
                .child(fields)
                .when(self.parameters.is_empty(),|body|body.child(div().flex_shrink_0().text_xs().text_color(rgb(visual.muted)).child(t(cx,"此模板没有需要填写的变量，请审核下方命令。", "This template has no parameters. Review the command below."))))
                .child(div().flex_shrink_0().flex().flex_col().gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,"完整命令预览", "Full command preview")))
                    .child(div().id("snippet-parameters-preview").test_support().h(px(168.)).min_w_0().overflow_hidden()
                        .child(Textarea::new(&self.preview).readonly(true).h_full().font_family("monospace")))))
            .child(div().id("snippet-parameters-footer").test_support().flex_shrink_0().p_3().bg(rgb(visual.canvas)).border_t_1().border_color(rgb(visual.border)).flex().flex_col().gap_2()
                .when_some(self.error.as_ref().or(self.validation.as_ref()),|footer,error|footer.child(div().id("snippet-parameters-error").test_support().max_h(px(64.)).overflow_y_scroll().text_xs().text_color(rgb(visual.danger)).child(error.render(cx))))
                .child(div().flex().items_center().justify_end().gap_2()
                    .child(div().flex_1().min_w_0().text_xs().text_color(rgb(visual.muted)).child(if self.saving{t(cx,"正在审核…", "Reviewing…")}else{t(cx,"仅填入，不执行", "Insert only; does not run")}))
                    .child(Button::new("snippet-parameters-cancel").ghost().label(t(cx,"取消", "Cancel")).disabled(self.saving).on_click(cx.listener(|view,_,_,cx|view.cancel(cx))))
                    .child(Button::new("snippet-parameters-insert").primary().label(t(cx,"填入命令栏", "Insert command")).disabled(self.saving||self.rendered.is_none()).on_click(cx.listener(|view,_,window,cx|view.submit(window,cx))))))
    }
}
