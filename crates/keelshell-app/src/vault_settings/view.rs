use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::Input,
};
use keelshell_core::CredentialKind;

fn label(cx: &App, text: impl Into<SharedString>) -> Div {
    let visual = crate::design::palette(cx);
    div()
        .text_xs()
        .text_color(rgb(visual.muted))
        .child(text.into())
}

fn kind_label(kind: CredentialKind, cx: &App) -> &'static str {
    match kind {
        CredentialKind::Password => t(cx, "SSH 密码", "SSH password"),
        CredentialKind::PrivateKeyPassphrase => t(cx, "私钥口令", "Private key passphrase"),
        CredentialKind::AiApiKey => t(cx, "AI API 密钥", "AI API key"),
    }
}

impl Render for VaultSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let busy = self.is_busy();
        let mut list = div()
            .id("vault-entry-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if !self.inspected {
            list=list.child(div().p_4().text_color(rgb(visual.muted)).child(t(cx,"验证主密码后显示经过认证的条目元数据。这里不会显示密码、口令或 API 密钥。", "Authenticate to view verified entry metadata. Passwords, passphrases and API keys are never displayed here.")));
        } else if self.entries.is_empty() {
            list = list.child(div().p_4().text_color(rgb(visual.muted)).child(t(
                cx,
                "凭据库中没有条目。",
                "The vault has no entries.",
            )));
        }
        for (index, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.page * 50)
            .take(50)
        {
            let reference = entry.reference;
            let linked = self.in_use.contains(&reference);
            let owner = self
                .profile_names
                .get(&entry.owner_id)
                .cloned()
                .unwrap_or_else(|| {
                    t(
                        cx,
                        "未保存或已移除的配置",
                        "Unsaved or removed configuration",
                    )
                    .to_owned()
                });
            list = list.child(
                div()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(owner))
                                    .child(label(
                                        cx,
                                        if linked {
                                            t(
                                                cx,
                                                "当前已保存配置已关联",
                                                "Referenced by saved configuration",
                                            )
                                        } else {
                                            t(
                                                cx,
                                                "当前已保存配置未关联",
                                                "Unlinked in saved configuration",
                                            )
                                        },
                                    )),
                            )
                            .child(label(cx, kind_label(entry.kind, cx)))
                            .child(label(
                                cx,
                                format!("{} {}", t(cx, "引用", "Reference"), reference),
                            ))
                            .child(label(
                                cx,
                                format!("{} {}", t(cx, "配置 ID", "Profile ID"), entry.owner_id),
                            )),
                    )
                    .child(
                        Button::new(("vault-delete-entry", index))
                            .icon(IconName::Trash)
                            .ghost()
                            .compact()
                            .label(t(cx, "删除", "Delete"))
                            .disabled(busy || linked || self.deletion.is_some())
                            .on_click(cx.listener(move |panel, _, window, cx| {
                                panel.request_delete(reference, window, cx)
                            })),
                    ),
            );
        }
        let pages = self.entries.len().div_ceil(50).max(1);
        let mut actions=div().id("vault-maintenance-actions").w(px(320.)).flex_shrink_0().min_h_0().overflow_y_scroll().flex().flex_col().gap_3().p_4().bg(rgb(visual.canvas)).border_l_1().border_color(rgb(visual.border))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,"验证与维护","Authentication and maintenance")))
            .child(label(cx, t(cx,"当前主密码 · 每次操作重新输入","Current master password · enter for each operation")))
            .child(Input::new(&self.master).id("vault-master").disabled(busy))
            .child(Button::new("vault-inspect").icon(IconName::RefreshCw).primary().label(t(cx,"解锁检查","Unlock and inspect")).disabled(busy).on_click(cx.listener(|panel,_,window,cx|panel.submit(Action::Inspect,window,cx))))
            .child(label(cx, t(cx,"解锁检查只保留条目列表，不缓存主密码或解密密钥。", "Inspection retains only the entry list, never the master password or decryption key.")));
        if let Some(reference) = self.deletion {
            actions = actions.child(
                div()
                    .p_3()
                    .rounded(px(8.))
                    .bg(rgb(visual.surface))
                    .border_1()
                    .border_color(rgb(visual.danger_border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(visual.danger))
                            .child(t(
                                cx,
                                "确认删除未关联条目",
                                "Confirm unlinked entry deletion",
                            )),
                    )
                    .child(label(cx, reference.to_string()))
                    .child(label(
                        cx,
                        t(
                            cx,
                            "删除不可撤销。请在上方重新输入当前主密码。",
                            "Deletion cannot be undone. Enter the current master password above.",
                        ),
                    ))
                    .child(
                        Button::new("vault-confirm-delete")
                            .label(t(cx, "确认删除此条目", "Confirm deletion"))
                            .disabled(busy)
                            .on_click(cx.listener(move |panel, _, window, cx| {
                                panel.submit(Action::Delete(reference), window, cx)
                            })),
                    )
                    .child(
                        Button::new("vault-cancel-delete")
                            .ghost()
                            .label(t(cx, "取消删除", "Cancel deletion"))
                            .disabled(busy)
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.deletion = None;
                                panel.clear_inputs(window, cx);
                                panel.focus(window, cx);
                                cx.notify();
                            })),
                    ),
            );
        } else {
            actions = actions.child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "更改主密码",
                        "Change master password",
                    )))
                    .child(label(cx, t(
                        cx,
                        "保留已保存的凭据；后续操作使用新主密码。",
                        "Keep saved credentials; use the new master password for subsequent operations.",
                    )))
                    .child(
                        Input::new(&self.replacement)
                            .id("vault-new-master")
                            .disabled(busy || !self.inspected),
                    )
                    .child(
                        Input::new(&self.confirmation)
                            .id("vault-confirm-master")
                            .disabled(busy || !self.inspected),
                    )
                    .child(
                        Button::new("vault-rotate")
                            .label(t(cx, "更改主密码", "Change master password"))
                            .disabled(busy || !self.inspected)
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.submit(Action::Rotate, window, cx)
                            })),
                    ),
            );
        }
        if busy {
            actions=actions.child(label(cx, t(cx,"取消或关闭只阻止尚未开始的保存；已开始的写入可能完成，页面会等待后台结束。", "Cancel or close prevents a save that has not started. An admitted write may finish; this page waits for the worker.")))
                .child(Button::new("vault-cancel-operation").ghost().label(t(cx,"请求取消操作","Request cancellation")).disabled(self.close_after_work).on_click(cx.listener(|panel,_,_,cx|panel.cancel(cx))));
        }
        div().id("vault-settings-panel").key_context("VaultSettings").w_full().h_full().min_h_0().flex().flex_col().bg(rgb(visual.surface)).text_color(rgb(visual.text)).text_sm()
            .child(div().px_4().py_3().border_b_1().border_color(rgb(visual.border)).flex().items_center().gap_3()
                .child(div().flex_1().flex().flex_col().gap_1().child(div().font_weight(FontWeight::SEMIBOLD).text_lg().child(t(cx,"凭据库维护","Credential vault"))).child(label(cx, t(cx,"检查条目 · 清理未关联凭据 · 更改主密码", "Inspect entries · remove unlinked credentials · change master password"))))
                .child(Button::new("vault-lock").ghost().label(t(cx,"锁定","Lock")).disabled(busy || !self.inspected).on_click(cx.listener(|panel,_,window,cx|panel.lock(window,cx)))))
            .child(div().flex_1().min_h_0().flex()
                .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().child(list)
                    .child(div().p_2().border_t_1().border_color(rgb(visual.border)).flex().items_center().gap_2()
                        .child(Button::new("vault-previous-page").ghost().icon(IconName::ChevronLeft).disabled(self.page==0 || busy).on_click(cx.listener(|panel,_,_,cx|{panel.page=panel.page.saturating_sub(1);cx.notify();})))
                        .child(label(cx, format!("{} / {} · {} {}",self.page+1,pages,self.entries.len(),t(cx,"条目","entries"))))
                        .child(Button::new("vault-next-page").ghost().icon(IconName::ChevronRight).disabled(self.page+1>=pages || busy).on_click(cx.listener(|panel,_,_,cx|{panel.page+=1;cx.notify();})))))
                .child(actions))
            .child(div().p_3().border_t_1().border_color(rgb(visual.border)).flex().items_center().gap_3()
                .child(div().flex_1().text_xs().text_color(rgb(visual.accent)).child(self.status.render(cx)))
                .child(Button::new("vault-close").ghost().label(if self.close_after_work {t(cx,"正在等待后台结束…","Waiting for background work…")} else {t(cx,"关闭","Close")}).disabled(self.close_after_work).on_click(cx.listener(|panel,_,window,cx|panel.close(window,cx)))))
    }
}
