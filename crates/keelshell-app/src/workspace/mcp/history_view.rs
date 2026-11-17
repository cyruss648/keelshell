use super::*;

impl Workspace {
    pub(in crate::workspace) fn mcp_unknown_review_modal(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(action) = self.mcp.unknown_reviewing.and_then(|id| {
            self.mcp.actions.iter().find(|action| {
                action.proposal.id() == id && matches!(action.state, ActionState::OutcomeUnknown)
            })
        }) else {
            return div().into_any_element();
        };
        let id = action.proposal.id();
        let target = action.proposal.target();
        let visual = crate::design::palette(cx);
        let identity = format!(
            "connection: {}\nsession: {}\nroute: {}\naction: {}\ndigest: {}",
            target.connection_id,
            target.session_id,
            target.route_revision,
            id,
            action.proposal.digest()
        );
        let content = match &action.proposal {
            ReviewedProposal::Command(proposal) => review::visible_command(&proposal.command),
            ReviewedProposal::File { proposal, diff, .. } => format!(
                "{}\nSHA-256: {}\nSHA-256: {}\n\n{}",
                review::visible_command(&proposal.path),
                proposal.expected_sha256,
                proposal.replacement_sha256,
                diff
            ),
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(visual.overlay))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("mcp-unknown-review-dialog")
                    .test_support()
                    .track_focus(&self.overlay_focus)
                    .w(px(850.))
                    .h(px(660.))
                    .max_w(relative(0.96))
                    .max_h(relative(0.94))
                    .min_w_0()
                    .min_h_0()
                    .bg(rgb(visual.surface))
                    .text_color(rgb(visual.text))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded_lg()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .p_3()
                            .flex_shrink_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(cx, "核对未知远端结果", "Review unknown remote outcome")),
                    )
                    .child(
                        div()
                            .id("mcp-unknown-review-scroll")
                            .test_support()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().child(review::visible_command(&action.label)))
                            .child(div().child(review::visible_command(&action.route)))
                            .child(div().text_sm().child(t(cx,
                                "请通过独立的 SSH 或文件检查核对远端状态。确认仅记录你已核对，并解除此提案的窗口关闭限制；不会重试、回滚或将提案改成成功。原始未知记录在本次运行中保留。",
                                "Check the remote state through an independent SSH or file inspection. Confirmation records your acknowledgement and releases this proposal's window-close hold. It does not retry, roll back or mark the proposal successful. The unknown record remains for this run.")))
                            .child(
                                div()
                                    .id("mcp-unknown-review-identity")
                                    .test_support()
                                    .role(accesskit::Role::Label)
                                    .aria_label(identity.clone())
                                    .text_xs()
                                    .overflow_x_scroll()
                                    .child(identity),
                            )
                            .child(
                                div()
                                    .id("mcp-unknown-review-content")
                                    .test_support()
                                    .role(accesskit::Role::Label)
                                    .aria_label(content.clone())
                                    .flex_shrink_0()
                                    .font_family("monospace")
                                    .text_sm()
                                    .whitespace_normal()
                                    .overflow_x_scroll()
                                    .bg(rgb(visual.canvas))
                                    .p_2()
                                    .child(content),
                            ),
                    )
                    .child(
                        div()
                            .id("mcp-unknown-review-footer")
                            .test_support()
                            .p_3()
                            .flex_shrink_0()
                            .border_t_1()
                            .border_color(rgb(visual.border))
                            .flex()
                            .flex_wrap()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("mcp-unknown-review-cancel")
                                    .ghost()
                                    .label(t(cx, "保留关闭限制", "Keep close hold"))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.mcp.unknown_reviewing = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("mcp-unknown-review-confirm")
                                    .primary()
                                    .label(t(cx, "我已独立核对远端结果", "I have checked the remote outcome"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.acknowledge_mcp_unknown(id, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn mcp_unknown_action_controls(
        &self,
        id: uuid::Uuid,
        acknowledged: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().text_color(rgb(visual.muted)).child(if acknowledged {
                t(cx, "你已确认独立核对；原始状态仍为结果未知。", "You acknowledged an independent check. The recorded outcome remains unknown.")
            } else {
                t(cx, "远端可能已有影响。请先独立核对；清理记录不会解除关闭限制。", "Remote effects may already exist. Check independently; clearing history keeps the close hold.")
            }))
            .when(!acknowledged, |body| body.child(
                Button::new(format!("mcp-unknown-review-{id}"))
                    .ghost()
                    .label(t(cx, "核对未知结果", "Review unknown outcome"))
                    .on_click(cx.listener(move |view, _, _, cx| view.open_mcp_unknown_review(id, cx)))
            ))
            .into_any_element()
    }
}
