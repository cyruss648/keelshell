//! Management selection is transient and independent of Ask and the editor.

use super::*;

impl super::super::AssistantPanel {
    pub(crate) fn revoke_stale_knowledge_request(&mut self, cx: &mut Context<Self>) {
        self.knowledge_context_changed(cx);
    }
}

impl KnowledgeLibrary {
    pub(super) fn entry_is_current(&self, expected: &SavedKnowledgeEntry) -> bool {
        let mut matching = self
            .entries
            .iter()
            .filter(|entry| entry.id() == expected.id());
        matching.next() == Some(expected) && matching.next().is_none()
    }

    pub(super) fn retain_management(&mut self) {
        let before = self.management.len();
        let entries = &self.entries;
        self.management.retain(|expected| {
            let mut matching = entries.iter().filter(|entry| entry.id() == expected.id());
            matching.next() == Some(expected) && matching.next().is_none()
        });
        if before != self.management.len() {
            self.management_revision = Uuid::new_v4();
        }
    }

    pub(super) fn review_selection_matches(&self, review: &MutationReview) -> bool {
        match review.batch_revision {
            Some(revision) => {
                review.replacement.is_none()
                    && revision == self.management_revision
                    && review.expected == self.management
                    && !review.expected.is_empty()
                    && review.expected.len() <= AppState::MAX_KNOWLEDGE_DELETE_ENTRIES
                    && review.expected.iter().enumerate().all(|(index, entry)| {
                        !review.expected[..index]
                            .iter()
                            .any(|old| old.id() == entry.id())
                    })
            }
            None => review.expected.len() == 1 && self.selected.as_ref() == review.expected.first(),
        }
    }

    pub(super) fn toggle_management(
        &mut self,
        expected: &SavedKnowledgeEntry,
        revision: Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready
            || self.management_revision != revision
            || !self.entry_is_current(expected)
        {
            return;
        }
        if let Some(index) = self
            .management
            .iter()
            .position(|entry| entry.id() == expected.id())
        {
            self.management.remove(index);
        } else if self.management.len() < AppState::MAX_KNOWLEDGE_DELETE_ENTRIES {
            self.management.push(expected.clone());
        } else {
            self.message = Message::new(
                "每批最多选择 32 条知识；可先移除一些选择。",
                "Select at most 32 entries per batch. Remove some selections first.",
            );
            cx.notify();
            return;
        }
        // Rotate even on an A -> B -> A selection change. An old painted action
        // cannot regain authority just because the final set looks the same.
        self.management_revision = Uuid::new_v4();
        self.review = None;
        cx.notify();
    }

    pub(super) fn clear_management(&mut self, revision: Uuid, cx: &mut Context<Self>) {
        if self.phase != Phase::Ready || self.management_revision != revision {
            return;
        }
        self.management.clear();
        self.management_revision = Uuid::new_v4();
        self.review = None;
        cx.notify();
    }

    pub(super) fn prepare_batch_delete(&mut self, revision: Uuid, cx: &mut Context<Self>) {
        if self.phase != Phase::Ready || self.management_revision != revision {
            return;
        }
        let review = MutationReview {
            id: Uuid::new_v4(),
            snapshot: self.snapshot,
            expected: self.management.clone(),
            replacement: None,
            batch_revision: Some(revision),
            draft_title: SharedString::default(),
            draft_summary: SharedString::default(),
        };
        if !self.review_selection_matches(&review)
            || !review
                .expected
                .iter()
                .all(|entry| self.entry_is_current(entry))
        {
            self.review = None;
            self.message = Message::new(
                "请选择 1–32 条当前知识，再重新审核。",
                "Select 1–32 current entries, then review again.",
            );
        } else {
            self.review = Some(review);
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.message = Message::new(
                "请逐项核对 UUID、完整内容和来源，再确认本批删除。",
                "Check every UUID, full text and source before confirming this batch.",
            );
        }
        cx.notify();
    }

    pub(super) fn management_checkbox(
        &self,
        entry: &SavedKnowledgeEntry,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> Checkbox {
        let expected = entry.clone();
        let selected = self.management.contains(entry);
        let revision = self.management_revision;
        Checkbox::new(SharedString::from(format!(
            "knowledge-management-toggle-{}",
            entry.id()
        )))
        .small()
        .mt_2()
        .checked(selected)
        .tab_stop(true)
        .disabled(
            !ready
                || (!selected && self.management.len() >= AppState::MAX_KNOWLEDGE_DELETE_ENTRIES),
        )
        .accessibility_label(format!(
            "{}: {} · {}",
            t(cx, "管理选择（待删除）", "Deletion selection"),
            entry.entry().title(),
            entry.id()
        ))
        .tooltip(t(
            cx,
            "选择待删除；不改变 Ask 附件",
            "Select for deletion; Ask attachments stay separate",
        ))
        .on_change(cx.listener(move |view, _, _, cx| {
            cx.stop_propagation();
            view.toggle_management(&expected, revision, cx);
        }))
    }

    pub(super) fn management_selection_view(
        &self,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let revision = self.management_revision;
        let matching = self.matching_entries(cx).unwrap_or_default();
        let hidden = self
            .management
            .iter()
            .filter(|entry| !matching.contains(entry))
            .count();
        div()
            .id("knowledge-management-selection")
            .test_support()
            .flex_shrink_0()
            .min_w_0()
            .px_3()
            .pb_2()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(
                div().text_xs().child(
                    Message::new(
                        format!("管理 {} / 32（隐藏 {hidden}）", self.management.len()),
                        format!("Delete: {} / 32 ({hidden} hidden)", self.management.len()),
                    )
                    .render(cx),
                ),
            )
            .child(
                Button::new("knowledge-management-clear")
                    .ghost()
                    .compact()
                    .disabled(!ready || self.management.is_empty())
                    .label(t(cx, "清除", "Clear"))
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.clear_management(revision, cx)),
                    ),
            )
            .child(self.management_review_button(ready, cx))
            .into_any_element()
    }

    pub(super) fn management_review_button(&self, ready: bool, cx: &mut Context<Self>) -> Button {
        let revision = self.management_revision;
        Button::new("knowledge-management-review")
            .danger()
            .compact()
            .disabled(!ready || self.management.is_empty())
            .label(
                Message::new(
                    format!("审核删除 {} 条", self.management.len()),
                    format!("Review deletion ({})", self.management.len()),
                )
                .render(cx),
            )
            .on_click(cx.listener(move |view, _, _, cx| view.prepare_batch_delete(revision, cx)))
    }

    pub(super) fn management_entry_view(
        &self,
        entry: &SavedKnowledgeEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fact = entry.entry();
        let outcome = match fact.outcome() {
            KnowledgeOutcome::Completed => t(cx, "原结果：完成", "Original outcome: completed"),
            KnowledgeOutcome::Failed => t(cx, "原结果：失败", "Original outcome: failed"),
        };
        let capture = match entry.capture_type() {
            keelshell_core::KnowledgeCaptureType::Selection => t(cx, "选中文本", "Selected text"),
            keelshell_core::KnowledgeCaptureType::VisibleScreen => {
                t(cx, "可见终端", "Visible screen")
            }
        };
        let visual = crate::design::palette(cx);
        div()
            .id(SharedString::from(format!(
                "knowledge-management-full-{}",
                entry.id()
            )))
            .test_support()
            .role(Role::Group)
            .aria_label(format!(
                "UUID: {} · {} · {} · {outcome}",
                entry.id(),
                fact.title(),
                fact.source().label()
            ))
            .aria_description(fact.summary().to_owned())
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .pb_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .whitespace_normal()
            .child(format!("UUID: {}", entry.id()))
            .child(Self::entry_text(entry))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                "{}: {}",
                t(cx, "来源（会话）", "Source (session)"),
                fact.source().label()
            )))
            .child(div().text_xs().child(format!(
                "{}: {} · {}: {}",
                t(cx, "来源 UUID", "Source UUID"),
                fact.source().id(),
                t(cx, "版本", "Revision"),
                fact.source().revision()
            )))
            .child(div().text_xs().child(format!(
                "{}: {} · {capture}",
                t(cx, "原会话", "Original session"),
                entry.session_id()
            )))
            .child(div().text_xs().child(format!(
                "{}: {} · {outcome}",
                t(cx, "Unix 时间（秒）", "Unix time (seconds)"),
                fact.occurred_at()
            )))
            .into_any_element()
    }

    pub(super) fn batch_review_view(
        &self,
        review: &MutationReview,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let visual = crate::design::palette(cx);
        let mut body = div().min_w_0().flex().flex_col().gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(
                Message::new(
                    format!("审核本批删除：共 {} 条", review.expected.len()),
                    format!("Review batch deletion: {} entries", review.expected.len()),
                ).render(cx)
            ))
            .child(div().text_xs().text_color(rgb(visual.danger)).child(t(
                cx,
                "确认后整批从当前本地库删除；取消不会写入。配置备份可能保留历史副本，不保证安全擦除。",
                "Confirmation removes this entire batch from the current local library. Cancel writes nothing. Backups may retain historical copies; this is not secure erasure.",
            )));
        for entry in &review.expected {
            body = body.child(self.management_entry_view(entry, cx));
        }
        let id = review.id;
        let footer = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("knowledge-library-confirm")
                    .danger()
                    .compact()
                    .disabled(!ready)
                    .label(
                        Message::new(
                            format!("确认删除 {} 条", review.expected.len()),
                            format!("Delete {} entries", review.expected.len()),
                        )
                        .render(cx),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| view.confirm(id, cx))),
            )
            .child(
                Button::new("knowledge-library-cancel")
                    .ghost()
                    .compact()
                    .disabled(!ready)
                    .label(t(cx, "取消删除", "Cancel deletion"))
                    .on_click(cx.listener(move |view, _, _, cx| view.cancel_review(id, cx))),
            );
        (body, footer)
    }
}
