//! Saved knowledge is an explicit Ask attachment, never live SSH authority.

use super::*;

#[cfg(test)]
mod tests;

/// Typed, non-content failures at the local selection-to-request boundary.
#[derive(Debug, thiserror::Error)]
pub(super) enum KnowledgeContextError {
    #[error(transparent)]
    Selection(#[from] keelshell_core::SavedContextError),
    #[error("saved knowledge is being edited or reloaded; select it again")]
    LibraryUnavailable,
    #[error("the knowledge selection changed; preview the request again")]
    StaleReview,
    #[error("complete knowledge does not fit the request; reduce selected context")]
    RequestBudget,
    #[error("saved knowledge attachments are available in Ask mode")]
    AskOnly,
}

#[derive(Clone, Copy)]
pub(super) struct KnowledgeContextReview {
    revision: Uuid,
    pub(super) count: usize,
}

pub(super) fn context_error_message(error: KnowledgeContextError) -> Message {
    let zh = match &error {
        KnowledgeContextError::Selection(keelshell_core::SavedContextError::TooManyEntries) => {
            "最多选择 8 条本地知识。"
        }
        KnowledgeContextError::Selection(keelshell_core::SavedContextError::TooLarge) => {
            "所选知识及来源信息超过 32 KiB，请减少条目。"
        }
        KnowledgeContextError::Selection(_) | KnowledgeContextError::StaleReview => {
            "知识选择已失效；请重新选择并预览请求。"
        }
        KnowledgeContextError::LibraryUnavailable => {
            "知识正在编辑或重新加载；请完成操作后重新选择。"
        }
        KnowledgeContextError::RequestBudget => {
            "完整知识无法放入请求预算；请减少知识、终端内容或问题后重新预览。"
        }
        KnowledgeContextError::AskOnly => "已选知识仅用于 Ask；请切回 Ask 或清空知识选择。",
    };
    Message::new(zh, error.to_string())
}

impl AssistantPanel {
    pub(super) fn compose_knowledge_context(
        &self,
        cx: &App,
    ) -> Result<(KnowledgeContextReview, Vec<String>), KnowledgeContextError> {
        let library = self.knowledge_library.read(cx);
        let count = library.context_selection().entries().len();
        if self.agent.mode && count > 0 {
            return Err(KnowledgeContextError::AskOnly);
        }
        Ok((
            KnowledgeContextReview {
                revision: library.context_revision(),
                count,
            },
            library.compose_context(cx)?,
        ))
    }

    pub(super) fn knowledge_request_is_current(&self, cx: &App) -> bool {
        let library = self.knowledge_library.read(cx);
        self.request_knowledge.is_some_and(|review| {
            review.revision == library.context_revision() && library.validate_context(cx).is_ok()
        })
    }

    pub(super) fn knowledge_context_changed(&mut self, cx: &mut Context<Self>) {
        // Events can queue behind a newer explicit preview. Check current state,
        // not an old event's revision, before revoking that newer request.
        if self.request_knowledge.is_some() && !self.knowledge_request_is_current(cx) {
            self.invalidate_request(cx);
            self.status = context_error_message(KnowledgeContextError::StaleReview);
        }
        cx.notify();
    }

    pub(super) fn send_reviewed(&mut self, review_id: Uuid, cx: &mut Context<Self>) {
        if self.prepared_review_id == Some(review_id) {
            self.send(cx);
        }
    }

    pub(super) fn clear_knowledge_context(&mut self, cx: &mut Context<Self>) {
        self.knowledge_library
            .update(cx, |library, cx| library.clear_context(cx));
        self.knowledge_context_changed(cx);
    }

    fn remove_knowledge_context(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.knowledge_library
            .update(cx, |library, cx| library.remove_context(id, cx));
        self.knowledge_context_changed(cx);
    }

    pub(super) fn knowledge_context_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let selection = self.knowledge_library.read(cx).context_selection().clone();
        let mut view = div()
            .id("assistant-knowledge-context")
            .test_support()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("assistant-choose-knowledge")
                            .ghost()
                            .compact()
                            .label(t(cx, "选择本地知识", "Choose saved knowledge"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.show_knowledge_library = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        div().text_color(rgb(visual.muted)).child(
                            Message::new(
                                format!(
                                    "{} / 8 条，{} 字节",
                                    selection.entries().len(),
                                    selection.bytes()
                                ),
                                format!(
                                    "{} / 8 entries, {} bytes",
                                    selection.entries().len(),
                                    selection.bytes()
                                ),
                            )
                            .render(cx),
                        ),
                    ),
            );
        for entry in selection.entries() {
            let id = entry.id();
            let title: String = entry.entry().title().chars().take(72).collect();
            let source: String = entry.entry().source().label().chars().take(72).collect();
            let occurred_at = i64::try_from(entry.entry().occurred_at())
                .ok()
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                .map(|time| time.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| entry.entry().occurred_at().to_string());
            let outcome = match entry.entry().outcome() {
                keelshell_core::KnowledgeOutcome::Completed => {
                    t(cx, "原结果：完成", "Original: completed")
                }
                keelshell_core::KnowledgeOutcome::Failed => {
                    t(cx, "原结果：失败", "Original: failed")
                }
            };
            view = view.child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_color(rgb(visual.muted)).child(source))
                    .child(
                        div()
                            .text_color(rgb(visual.muted))
                            .child(format!("{occurred_at}; {outcome}")),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "assistant-remove-knowledge-{id}"
                        )))
                        .ghost()
                        .compact()
                        .label(t(cx, "移除", "Remove"))
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            panel.remove_knowledge_context(id, cx)
                        })),
                    ),
            );
        }
        if !selection.entries().is_empty() {
            view = view
                .child(div().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "历史资料仅供 Ask 参考，不代表当前主机状态。完整来源和正文将在请求中审核。",
                    "Historical Ask references do not establish current host state. Review full sources and text in the request.",
                )))
                .child(
                    Button::new("assistant-clear-knowledge")
                        .ghost()
                        .compact()
                        .label(t(cx, "清空知识选择", "Clear knowledge selection"))
                        .on_click(cx.listener(|panel, _, _, cx| panel.clear_knowledge_context(cx))),
                );
        }
        view.into_any_element()
    }
}
