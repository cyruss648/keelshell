//! Local categories have separate drafts and reviews; they grant no context authority.

use std::collections::BTreeMap;

use super::*;

#[derive(Clone, PartialEq, Eq, Default)]
pub(super) enum CategoryFilter {
    #[default]
    All,
    Uncategorized,
    Named(String),
}

impl CategoryFilter {
    pub(super) fn matches(&self, entry: &SavedKnowledgeEntry) -> bool {
        match self {
            Self::All => true,
            Self::Uncategorized => entry.category().is_none(),
            Self::Named(name) => entry.category() == Some(name.as_str()),
        }
    }
}

impl KnowledgeLibrary {
    #[cfg(test)]
    pub(crate) fn set_category_draft_for_test(
        &mut self,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.category_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            cx.emit(InputEvent::Change);
        });
    }

    pub(super) fn category_counts(&self) -> (usize, BTreeMap<&str, usize>) {
        let mut uncategorized = 0;
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            if let Some(name) = entry.category() {
                *counts.entry(name).or_default() += 1;
            } else {
                uncategorized += 1;
            }
        }
        (uncategorized, counts)
    }

    pub(super) fn retain_category_filter(&mut self) {
        if let CategoryFilter::Named(name) = &self.category_filter
            && !self
                .entries
                .iter()
                .any(|entry| entry.category() == Some(name.as_str()))
        {
            self.category_filter = CategoryFilter::All;
        }
    }

    pub(super) fn set_category_filter(&mut self, filter: CategoryFilter, cx: &mut Context<Self>) {
        self.category_filter = filter;
        self.retain_category_filter();
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    pub(super) fn category_label(entry: &SavedKnowledgeEntry, cx: &App) -> String {
        format!(
            "{}: {}",
            t(cx, "分类", "Category"),
            entry
                .category()
                .unwrap_or_else(|| t(cx, "未分类", "Uncategorized")),
        )
    }

    pub(super) fn category_draft_matches(&self, entry: &SavedKnowledgeEntry, cx: &App) -> bool {
        self.category_input.read(cx).value().as_ref() == entry.category().unwrap_or_default()
    }

    pub(super) fn category_draft_changed(&mut self, cx: &mut Context<Self>) {
        // Rotate on every edit, including A -> B -> A. A previously painted
        // confirm must not regain authority after the input returns to A.
        self.category_revision = Uuid::new_v4();
        if self.phase == Phase::Ready {
            self.review = None;
        }
        if let Some(target) = &self.category_target
            && !self.category_draft_matches(target, cx)
            && self.context.remove(target.id())
        {
            self.context_changed(cx);
        }
        cx.notify();
    }

    pub(super) fn open_category(
        &mut self,
        expected: &SavedKnowledgeEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready || !self.entry_is_current(expected) {
            return;
        }
        self.review = None;
        self.category_target = Some(expected.clone());
        self.category_revision = Uuid::new_v4();
        self.category_input.update(cx, |input, cx| {
            input.set_value(expected.category().unwrap_or_default(), window, cx)
        });
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.message = Message::new(
            "分类只整理本地知识；不会加入模型请求或恢复历史 SSH 权限。",
            "Categories organize local knowledge only. They are not model context or historical SSH permissions.",
        );
        cx.notify();
    }

    pub(super) fn prepare_category(
        &mut self,
        expected: &SavedKnowledgeEntry,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready
            || self.category_target.as_ref() != Some(expected)
            || !self.entry_is_current(expected)
        {
            return;
        }
        let draft = self.category_input.read(cx).value();
        let replacement = match expected.with_reviewed_category(Some(draft.as_ref())) {
            Ok(replacement) => replacement,
            Err(_) => {
                self.review = None;
                // A category can contain personal data. Do not echo its input
                // or a parser payload through status messages or diagnostics.
                self.message = Message::new(
                    "分类最多 128 字节，不能含控制字符或敏感凭据标记。",
                    "Categories accept at most 128 bytes, without control characters or sensitive credential markers.",
                );
                cx.notify();
                return;
            }
        };
        if replacement == *expected {
            self.review = None;
            self.message = Message::new(
                "分类未变化，无需保存。",
                "The category is unchanged. No save is needed.",
            );
            cx.notify();
            return;
        }
        self.review = Some(MutationReview {
            id: Uuid::new_v4(),
            snapshot: self.snapshot,
            expected: vec![expected.clone()],
            replacement: Some(replacement),
            batch_revision: None,
            category_revision: Some(self.category_revision),
            draft_category: draft,
            draft_title: SharedString::default(),
            draft_summary: SharedString::default(),
        });
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.message = Message::new(
            "请审核完整条目与分类变化；确认后仅保存分类，取消不会写入。",
            "Review the complete record and category change. Confirm saves only the category; cancel writes nothing.",
        );
        cx.notify();
    }

    pub(super) fn category_button(
        &self,
        entry: &SavedKnowledgeEntry,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let expected = entry.clone();
        let label = Self::category_label(entry, cx);
        let short = match entry.category() {
            Some(name) => {
                let mut short: String = name.chars().take(6).collect();
                if name.chars().count() > 6 {
                    short.push('…');
                }
                short
            }
            None => t(cx, "未分类", "Uncategorized").to_owned(),
        };
        Button::new(SharedString::from(format!(
            "knowledge-category-edit-{}",
            entry.id()
        )))
        .ghost()
        .compact()
        .disabled(!ready)
        .label(short)
        .accessibility_label(label.clone())
        .tooltip(label)
        .on_click(cx.listener(move |view, _, window, cx| view.open_category(&expected, window, cx)))
    }

    pub(super) fn category_filter_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let (uncategorized, counts) = self.category_counts();
        let mut filters = div()
            .id("knowledge-category-filters")
            .test_support()
            .role(Role::Group)
            .aria_label(t(cx, "分类筛选", "Category filters"))
            .h(px(112.))
            .min_h_0()
            .flex_shrink_0()
            .min_w_0()
            .mx_3()
            .overflow_y_scroll()
            .track_scroll(&self.category_scroll)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Button::new("knowledge-category-all")
                    .ghost()
                    .compact()
                    .flex_shrink_0()
                    .w_full()
                    .toggled(self.category_filter == CategoryFilter::All)
                    .label(format!("{} ({})", t(cx, "全部", "All"), self.entries.len()))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.set_category_filter(CategoryFilter::All, cx)
                    })),
            )
            .child(
                Button::new("knowledge-category-uncategorized")
                    .ghost()
                    .compact()
                    .flex_shrink_0()
                    .w_full()
                    .toggled(self.category_filter == CategoryFilter::Uncategorized)
                    .label(format!(
                        "{} ({uncategorized})",
                        t(cx, "未分类", "Uncategorized")
                    ))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.set_category_filter(CategoryFilter::Uncategorized, cx)
                    })),
            );
        for (index, (name, count)) in counts.into_iter().enumerate() {
            let filter = CategoryFilter::Named(name.to_owned());
            let selected = self.category_filter == filter;
            let label = format!("{name} ({count})");
            // Wrap complete names in bounded fragments, including long ASCII
            // names without spaces. Equal prefixes remain visibly distinguishable.
            let characters: Vec<_> = name.chars().collect();
            let mut text = div()
                .min_w_0()
                .w_full()
                .flex()
                .flex_wrap()
                .whitespace_normal();
            for chunk in characters.chunks(12) {
                text = text.child(
                    div()
                        .flex_shrink_0()
                        .child(chunk.iter().collect::<String>()),
                );
            }
            text = text.child(div().flex_shrink_0().child(format!(" ({count})")));
            filters = filters.child(
                Button::new(SharedString::from(format!(
                    "knowledge-category-filter-{index}"
                )))
                .ghost()
                .compact()
                .tab_stop(true)
                .flex_shrink_0()
                .w_full()
                .h_auto()
                .min_h(px(28.))
                .toggled(selected)
                .accessibility_label(label.clone())
                .tooltip(label)
                .child(text)
                .on_click(
                    cx.listener(move |view, _, _, cx| view.set_category_filter(filter.clone(), cx)),
                ),
            );
        }
        filters.into_any_element()
    }

    pub(super) fn category_editor_view(
        &self,
        target: &SavedKnowledgeEntry,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let expected = target.clone();
        let body = div().min_w_0().flex().flex_col().gap_2()
            .child(div().child(t(cx, "分类（留空即未分类）", "Category (empty means uncategorized)")))
            .child(Input::new(&self.category_input)
                .id("knowledge-category-input")
                .aria_label(t(cx, "本地知识分类", "Local knowledge category"))
                .disabled(!ready))
            .child(div().text_xs().child(t(
                cx,
                "最多 128 个 UTF-8 字节；去除首尾空白，区分大小写。仅保存分类，不提交标题或正文草稿。",
                "Up to 128 UTF-8 bytes; trimmed and case-sensitive. Saves only the category, leaving title and text drafts separate.",
            )))
            .child(self.management_entry_view(target, cx));
        let footer = div()
            .min_w_0()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("knowledge-category-review")
                    .primary()
                    .compact()
                    .disabled(!ready)
                    .label(t(cx, "审核分类", "Review category"))
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.prepare_category(&expected, cx)),
                    ),
            )
            .child(
                Button::new("knowledge-category-back")
                    .ghost()
                    .compact()
                    .disabled(!ready)
                    .label(t(cx, "取消分类修改", "Cancel category change"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        if view.phase == Phase::Ready {
                            view.category_target = None;
                            view.category_revision = Uuid::new_v4();
                            view.review = None;
                            view.scroll.set_offset(point(px(0.), px(0.)));
                            cx.notify();
                        }
                    })),
            );
        (body, footer)
    }

    pub(super) fn category_review_view(
        &self,
        review: &MutationReview,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        let mut body = div().min_w_0().flex().flex_col().gap_2().child(
            div().font_weight(FontWeight::SEMIBOLD).child(t(
                cx,
                "当前完整条目",
                "Current complete record",
            )),
        );
        if let Some(expected) = review.expected.first() {
            body = body.child(self.complete_entry_view(expected, "knowledge-category-before", cx));
        }
        if let Some(replacement) = &review.replacement {
            body = body
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                    cx,
                    "确认后完整条目",
                    "Complete record after confirmation",
                )))
                .child(self.complete_entry_view(replacement, "knowledge-category-after", cx));
        }
        let id = review.id;
        let footer = div()
            .min_w_0()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("knowledge-library-confirm")
                    .primary()
                    .compact()
                    .disabled(!ready)
                    .label(t(cx, "确认分类修改", "Confirm category"))
                    .on_click(cx.listener(move |view, _, _, cx| view.confirm(id, cx))),
            )
            .child(
                Button::new("knowledge-library-cancel")
                    .ghost()
                    .compact()
                    .disabled(!ready)
                    .label(t(cx, "返回分类编辑", "Back to category"))
                    .on_click(cx.listener(move |view, _, _, cx| view.cancel_review(id, cx))),
            );
        (body, footer)
    }
}
