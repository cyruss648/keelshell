//! Local knowledge browsing and explicit, snapshot-bound mutation review.
//! The workspace owns every disk operation; this panel never contacts a model.

use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
    },
    *,
};
use keelshell_core::{
    AppState, KnowledgeError, KnowledgeOutcome, MAX_KNOWLEDGE_SEARCH_QUERY_BYTES,
    MAX_KNOWLEDGE_TEXT_BYTES, SavedKnowledgeContext, SavedKnowledgeEntry, SnapshotRevision,
};
use uuid::Uuid;

use crate::i18n::{Message, t};

mod batch;
#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod tests;

pub(crate) enum LibraryEvent {
    Apply(Uuid),
    Reload(Uuid),
    Close,
    ContextChanged,
}

#[derive(Clone)]
pub(crate) struct MutationReview {
    pub(crate) id: Uuid,
    pub(crate) snapshot: SnapshotRevision,
    pub(crate) expected: Vec<SavedKnowledgeEntry>,
    pub(crate) replacement: Option<SavedKnowledgeEntry>,
    batch_revision: Option<Uuid>,
    draft_title: SharedString,
    draft_summary: SharedString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Saving(Uuid),
    NeedsReload(Uuid),
    Reloading(Uuid),
}

pub(crate) struct KnowledgeLibrary {
    entries: Vec<SavedKnowledgeEntry>,
    snapshot: SnapshotRevision,
    query: Entity<InputState>,
    scroll: ScrollHandle,
    title: Entity<InputState>,
    summary: Entity<TextareaState>,
    selected: Option<SavedKnowledgeEntry>,
    // Management selection never grants Ask attachment or edit authority.
    management: Vec<SavedKnowledgeEntry>,
    management_revision: Uuid,
    review: Option<MutationReview>,
    phase: Phase,
    show_source_details: bool,
    message: Message,
    context: SavedKnowledgeContext,
    context_revision: Uuid,
    _subscriptions: Vec<Subscription>,
}

impl KnowledgeLibrary {
    #[cfg(test)]
    pub(crate) fn set_draft_for_test(
        &mut self,
        title: &str,
        summary: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.title
            .update(cx, |input, cx| input.set_value(title, window, cx));
        self.summary
            .update(cx, |input, cx| input.set_value(summary, window, cx));
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        let title = cx.new(|cx| InputState::new(window, cx));
        let summary = cx.new(|cx| TextareaState::new(window, cx).rows(8));
        let subscriptions = vec![
            cx.subscribe(&query, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&title, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.invalidate_review(cx);
                }
            }),
            cx.subscribe(&summary, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.invalidate_review(cx);
                }
            }),
        ];
        Self {
            entries: Vec::new(),
            snapshot: SnapshotRevision::default(),
            query,
            scroll: ScrollHandle::default(),
            title,
            summary,
            selected: None,
            management: Vec::new(),
            management_revision: Uuid::new_v4(),
            review: None,
            phase: Phase::Ready,
            show_source_details: false,
            message: Message::new(
                "查看和整理明确保存的本地知识。",
                "Browse and organize explicitly saved local knowledge.",
            ),
            context: SavedKnowledgeContext::default(),
            context_revision: Uuid::new_v4(),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn set_snapshot(&mut self, state: &AppState, cx: &mut Context<Self>) {
        if self.snapshot == state.snapshot && self.entries == state.knowledge_entries {
            return;
        }
        self.snapshot = state.snapshot;
        self.entries.clone_from(&state.knowledge_entries);
        // Any new authoritative snapshot retires painted management controls,
        // even when the selected entries themselves are unchanged.
        self.management_revision = Uuid::new_v4();
        self.retain_management();
        if self.context.retain_current(&self.entries) {
            self.context_changed(cx);
        }
        if self.phase == Phase::Ready {
            self.review = None;
            if self
                .selected
                .as_ref()
                .is_some_and(|selected| !self.entries.contains(selected))
            {
                self.selected = None;
            }
            self.message = Message::new(
                "本地知识已刷新；修改需要重新审核。",
                "Local knowledge refreshed. Review changes again before saving.",
            );
        }
        cx.notify();
    }

    fn invalidate_review(&mut self, cx: &mut Context<Self>) {
        if let Some(selected) = &self.selected
            && !self.saved_draft_matches(selected, cx)
            && self.context.remove(selected.id())
        {
            self.context_changed(cx);
        }
        if self.phase == Phase::Ready
            && self
                .review
                .as_ref()
                .is_some_and(|review| !self.draft_matches(review, cx))
        {
            self.review = None;
        }
        cx.notify();
    }

    fn saved_draft_matches(&self, entry: &SavedKnowledgeEntry, cx: &App) -> bool {
        self.title.read(cx).value().as_ref() == entry.entry().title()
            && self.summary.read(cx).value().as_ref() == entry.entry().summary()
    }

    pub(super) fn context_revision(&self) -> Uuid {
        self.context_revision
    }

    pub(super) fn context_selection(&self) -> &SavedKnowledgeContext {
        &self.context
    }

    pub(super) fn compose_context(
        &self,
        cx: &App,
    ) -> Result<Vec<String>, super::knowledge_context::KnowledgeContextError> {
        self.validate_context(cx)?;
        self.context.compose(&self.entries).map_err(Into::into)
    }

    pub(super) fn validate_context(
        &self,
        cx: &App,
    ) -> Result<(), super::knowledge_context::KnowledgeContextError> {
        use super::knowledge_context::KnowledgeContextError;
        if !self.context.entries().is_empty()
            && (self.phase != Phase::Ready
                || self.selected.as_ref().is_some_and(|entry| {
                    self.context.entries().contains(entry) && !self.saved_draft_matches(entry, cx)
                }))
        {
            return Err(KnowledgeContextError::LibraryUnavailable);
        }
        self.context
            .validate_current(&self.entries)
            .map_err(Into::into)
    }

    fn context_changed(&mut self, cx: &mut Context<Self>) {
        self.context_revision = Uuid::new_v4();
        cx.emit(LibraryEvent::ContextChanged);
        cx.notify();
    }

    pub(super) fn remove_context(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.context.remove(id) {
            self.context_changed(cx);
        }
    }

    pub(super) fn clear_context(&mut self, cx: &mut Context<Self>) {
        if self.context.clear() {
            self.context_changed(cx);
        }
    }

    pub(super) fn toggle_context(
        &mut self,
        expected: &SavedKnowledgeEntry,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready || !self.entries.contains(expected) {
            return;
        }
        if self.context.entries().contains(expected) {
            self.remove_context(expected.id(), cx);
            return;
        }
        // An editor draft is never substituted for the saved version behind the
        // user's back. Save it first or return to the list to select saved text.
        if self.selected.as_ref() == Some(expected) && !self.saved_draft_matches(expected, cx) {
            return;
        }
        match self.context.select(expected, &self.entries) {
            Ok(true) => {
                self.context_changed(cx);
                self.message = Message::new(
                    "已加入 Ask 草稿。返回助手后预览完整请求，再确认发送。",
                    "Added to the Ask draft. Return to the assistant, preview the full request, then confirm sending.",
                );
            }
            Ok(false) => {}
            Err(error) => {
                self.message = super::knowledge_context::context_error_message(error.into());
                cx.notify();
            }
        }
    }

    fn context_button(
        &self,
        entry: &SavedKnowledgeEntry,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let expected = entry.clone();
        let selected = self.context.entries().contains(entry);
        let revision = self.context_revision;
        Button::new(SharedString::from(format!(
            "knowledge-context-toggle-{}",
            entry.id()
        )))
        .ghost()
        .compact()
        .toggled(selected)
        .disabled(!ready)
        .label(if selected {
            t(cx, "移出 Ask", "Remove from Ask")
        } else {
            t(cx, "加入 Ask", "Add to Ask")
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            if view.context_revision == revision {
                view.toggle_context(&expected, cx);
            }
        }))
    }

    fn select(
        &mut self,
        expected: &SavedKnowledgeEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready || !self.entries.contains(expected) {
            return;
        }
        self.review = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.selected = Some(expected.clone());
        self.show_source_details = false;
        self.title.update(cx, |input, cx| {
            input.set_value(expected.entry().title(), window, cx)
        });
        self.summary.update(cx, |input, cx| {
            input.set_value(expected.entry().summary(), window, cx)
        });
        self.message = Message::new(
            "编辑标题或正文后预览；来源和原结果保持绑定。",
            "Edit the title or text, then review. The source and original outcome stay bound.",
        );
        cx.notify();
    }

    fn prepare(&mut self, expected: &SavedKnowledgeEntry, delete: bool, cx: &mut Context<Self>) {
        if self.phase != Phase::Ready
            || self.selected.as_ref() != Some(expected)
            || !self.entries.contains(expected)
        {
            return;
        }
        let replacement = if delete {
            None
        } else {
            match self.edited_entry(expected, cx) {
                Ok(entry) => Some(entry),
                Err(error) => {
                    self.review = None;
                    self.message = Message::detail(
                        "修改未通过本地内容检查",
                        "The changes did not pass the local content check",
                        error,
                    );
                    cx.notify();
                    return;
                }
            }
        };
        self.review = Some(MutationReview {
            id: Uuid::new_v4(),
            snapshot: self.snapshot,
            expected: vec![expected.clone()],
            replacement,
            batch_revision: None,
            draft_title: self.title.read(cx).value(),
            draft_summary: self.summary.read(cx).value(),
        });
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.message = Message::new(
            "请审核完整内容，再确认本次本地修改。",
            "Review the full content, then confirm this local change.",
        );
        cx.notify();
    }

    fn edited_entry(
        &self,
        expected: &SavedKnowledgeEntry,
        cx: &App,
    ) -> Result<SavedKnowledgeEntry, KnowledgeError> {
        let raw = self.summary.read(cx).value();
        if raw.len() > MAX_KNOWLEDGE_TEXT_BYTES {
            return Err(KnowledgeError::TextTooLarge);
        }
        if raw
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(KnowledgeError::EmptyText);
        }
        // As with terminal captures, the editable multiline draft becomes one
        // reviewed summary. Check the raw budget before reducing whitespace.
        let summary = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        expected.with_reviewed_content(self.title.read(cx).value().as_ref(), &summary)
    }

    fn draft_matches(&self, review: &MutationReview, cx: &App) -> bool {
        review.batch_revision.is_some()
            || (self.title.read(cx).value() == review.draft_title
                && self.summary.read(cx).value() == review.draft_summary)
    }

    pub(crate) fn candidate(&self, id: Uuid, cx: &App) -> Option<MutationReview> {
        let review = self.review.as_ref()?;
        (self.phase == Phase::Ready
            && review.id == id
            && review.snapshot == self.snapshot
            && self.draft_matches(review, cx)
            && self.review_selection_matches(review)
            && review
                .expected
                .iter()
                .all(|entry| self.entry_is_current(entry)))
        .then(|| review.clone())
    }

    fn confirm(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.candidate(id, cx).is_some() {
            cx.emit(LibraryEvent::Apply(id));
        }
    }

    fn cancel_review(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.phase == Phase::Ready && self.review.as_ref().is_some_and(|r| r.id == id) {
            self.review = None;
            self.scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }

    fn disable_editor(&self, disabled: bool, cx: &mut Context<Self>) {
        self.title
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        self.summary
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
    }

    pub(crate) fn begin_save(&mut self, id: Uuid, cx: &mut Context<Self>) -> bool {
        if self.candidate(id, cx).is_none() {
            return false;
        }
        self.phase = Phase::Saving(id);
        if let Some(review) = &self.review {
            for entry in &review.expected {
                self.context.remove(entry.id());
            }
        }
        self.context_changed(cx);
        self.disable_editor(true, cx);
        self.message = Message::new("正在保存本地修改…", "Saving local changes…");
        cx.notify();
        true
    }

    pub(crate) fn finish_save(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.phase != Phase::Saving(id) {
            return;
        }
        self.phase = Phase::Ready;
        self.review = None;
        self.selected = None;
        self.disable_editor(false, cx);
        self.message = Message::new("本地知识修改已保存。", "Local knowledge changes saved.");
        cx.notify();
    }

    pub(crate) fn save_failed(
        &mut self,
        id: Uuid,
        error: &keelshell_core::Error,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Saving(id) {
            return;
        }
        self.require_reload(error, cx);
    }

    pub(crate) fn candidate_failed(
        &mut self,
        id: Uuid,
        error: &keelshell_core::Error,
        cx: &mut Context<Self>,
    ) {
        if self.phase == Phase::Ready && self.review.as_ref().is_some_and(|r| r.id == id) {
            self.require_reload(error, cx);
        }
    }

    fn require_reload(&mut self, error: &keelshell_core::Error, cx: &mut Context<Self>) {
        self.review = None;
        // Even an ordinary I/O error can race an external writer. A fresh load
        // is required before another review, never an automatic retry.
        self.phase = Phase::NeedsReload(Uuid::new_v4());
        self.disable_editor(true, cx);
        self.context.clear();
        self.context_changed(cx);
        self.message = Message::detail(
            "修改未确认保存；请重新加载并重新审核",
            "The change was not confirmed saved. Reload and review again",
            error,
        );
        cx.notify();
    }

    pub(crate) fn reject_review(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Ready {
            self.review = None;
            self.message = Message::new(
                "审核已过期，请重新选择条目并审核。",
                "The review is stale. Select the entry and review it again.",
            );
            cx.notify();
        }
    }

    pub(crate) fn begin_reload(&mut self, token: Uuid, cx: &mut Context<Self>) -> bool {
        if self.phase != Phase::NeedsReload(token) {
            return false;
        }
        self.phase = Phase::Reloading(token);
        self.context.clear();
        self.context_changed(cx);
        self.message = Message::new("正在重新加载本机配置…", "Reloading local configuration…");
        cx.notify();
        true
    }

    pub(crate) fn finish_reload(
        &mut self,
        token: Uuid,
        state: &AppState,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.phase != Phase::Reloading(token) {
            return false;
        }
        self.snapshot = state.snapshot;
        self.entries.clone_from(&state.knowledge_entries);
        self.context.clear();
        self.context_changed(cx);
        self.phase = Phase::Ready;
        self.selected = None;
        self.review = None;
        self.management.clear();
        self.management_revision = Uuid::new_v4();
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.disable_editor(false, cx);
        self.message = Message::new(
            "配置已重新加载，旧审核已清除。请重新选择条目。",
            "Configuration reloaded and the old review cleared. Select the entry again.",
        );
        cx.notify();
        true
    }

    pub(crate) fn reload_failed(
        &mut self,
        token: Uuid,
        error: &keelshell_core::Error,
        cx: &mut Context<Self>,
    ) {
        if self.phase == Phase::Reloading(token) {
            self.phase = Phase::NeedsReload(Uuid::new_v4());
            self.message = Message::detail(
                "重新加载失败，修改仍被阻止",
                "Reload failed; changes remain blocked",
                error,
            );
            cx.notify();
        }
    }

    fn matching_entries(&self, cx: &App) -> Option<Vec<SavedKnowledgeEntry>> {
        let query = self.query.read(cx).value();
        if query.len() > MAX_KNOWLEDGE_SEARCH_QUERY_BYTES || query.chars().any(char::is_control) {
            return None;
        }
        let query = query.trim().to_lowercase();
        Some(
            self.entries
                .iter()
                .filter(|entry| {
                    query.is_empty()
                        || [
                            entry.entry().title(),
                            entry.entry().summary(),
                            entry.entry().source().label(),
                        ]
                        .iter()
                        .any(|text| text.to_lowercase().contains(&query))
                })
                .cloned()
                .collect(),
        )
    }

    fn source_view(&self, entry: &SavedKnowledgeEntry, cx: &mut Context<Self>) -> AnyElement {
        let fact = entry.entry();
        let visual = crate::design::palette(cx);
        let captured_at = i64::try_from(fact.occurred_at())
            .ok()
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| fact.occurred_at().to_string());
        let mut source = div()
            .text_xs()
            .text_color(rgb(visual.muted))
            .flex()
            .flex_col()
            .gap_1()
            .child(fact.source().label().to_owned())
            .child(match fact.outcome() {
                KnowledgeOutcome::Completed => t(cx, "已确认完成", "Confirmed completed"),
                KnowledgeOutcome::Failed => t(cx, "已确认失败", "Confirmed failed"),
            })
            .child(captured_at)
            .child(
                Button::new("knowledge-library-source-details")
                    .ghost()
                    .label(if self.show_source_details {
                        t(cx, "收起来源详情", "Hide source details")
                    } else {
                        t(cx, "查看来源详情", "View source details")
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.show_source_details = !view.show_source_details;
                        cx.notify();
                    })),
            );
        if self.show_source_details {
            source = source
                .child(format!(
                    "{}: {}",
                    t(cx, "会话", "Session"),
                    entry.session_id()
                ))
                .child(format!(
                    "{}: {}",
                    t(cx, "来源", "Source"),
                    fact.source().id()
                ))
                .child(format!(
                    "{}: {}",
                    t(cx, "来源版本", "Source revision"),
                    fact.source().revision()
                ))
                .child(match entry.capture_type() {
                    keelshell_core::KnowledgeCaptureType::Selection => {
                        t(cx, "选中文本", "Selected text")
                    }
                    keelshell_core::KnowledgeCaptureType::VisibleScreen => {
                        t(cx, "可见终端", "Visible screen")
                    }
                });
        }
        source.into_any_element()
    }

    fn entry_text(entry: &SavedKnowledgeEntry) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(entry.entry().title().to_owned()),
            )
            .child(div().child(entry.entry().summary().to_owned()))
            .into_any_element()
    }
}

impl EventEmitter<LibraryEvent> for KnowledgeLibrary {}

impl Render for KnowledgeLibrary {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let ready = self.phase == Phase::Ready;
        let mut body = div().flex().flex_col().gap_3();
        let mut footer = div().flex().flex_wrap().gap_2();
        if let Some(review) = self.review.as_ref().filter(|r| r.batch_revision.is_some()) {
            let (review_body, review_footer) = self.batch_review_view(review, ready, cx);
            body = review_body;
            footer = review_footer;
        } else if let Some(review) = self.review.as_ref() {
            let id = review.id;
            let Some(expected) = review.expected.first() else {
                return div().into_any_element();
            };
            body = body
                .child(self.source_view(expected, cx))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                    cx,
                    "当前保存的内容",
                    "Currently saved content",
                )))
                .child(Self::entry_text(expected));
            if let Some(replacement) = &review.replacement {
                body = body
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "确认后保存的内容",
                        "Content after confirmation",
                    )))
                    .child(Self::entry_text(replacement));
            } else {
                body = body.child(div().text_color(rgb(visual.danger)).child(t(
                    cx,
                    "确认后将从当前本地知识库删除此条目。配置备份可能保留历史副本。",
                    "Confirmation removes this entry from the current local library. Configuration backups may retain historical copies.",
                )));
            }
            footer = footer
                .child(
                    Button::new("knowledge-library-confirm")
                        .primary()
                        .disabled(!ready)
                        .label(if review.replacement.is_some() {
                            t(cx, "确认保存修改", "Confirm changes")
                        } else {
                            t(cx, "确认删除条目", "Confirm deletion")
                        })
                        .on_click(cx.listener(move |view, _, _, cx| view.confirm(id, cx))),
                )
                .child(
                    Button::new("knowledge-library-cancel")
                        .ghost()
                        .disabled(!ready)
                        .label(t(cx, "返回编辑", "Back to editing"))
                        .on_click(cx.listener(move |view, _, _, cx| view.cancel_review(id, cx))),
                );
        } else if let Some(selected) = self.selected.clone() {
            let delete_target = selected.clone();
            body = body
                .child(self.source_view(&selected, cx))
                .child(div().child(t(cx, "标题", "Title")))
                .child(
                    Input::new(&self.title)
                        .id("knowledge-library-title")
                        .aria_label(t(cx, "知识标题", "Knowledge title"))
                        .disabled(!ready),
                )
                .child(div().child(t(
                    cx,
                    "摘要（换行会合并为空格）",
                    "Summary (line breaks become spaces)",
                )))
                .child(
                    div().id("knowledge-library-summary").child(
                        Textarea::new(&self.summary)
                            .aria_label(t(cx, "知识摘要", "Knowledge summary"))
                            .disabled(!ready),
                    ),
                );
            footer = footer
                .child(self.context_button(
                    &selected,
                    ready && self.saved_draft_matches(&selected, cx),
                    cx,
                ))
                .child(
                    Button::new("knowledge-library-review-edit")
                        .primary()
                        .disabled(!ready)
                        .label(t(cx, "预览修改", "Review changes"))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.prepare(&selected, false, cx)),
                        ),
                )
                .child(
                    Button::new("knowledge-library-review-delete")
                        .ghost()
                        .disabled(!ready)
                        .label(t(cx, "审核删除", "Review deletion"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.prepare(&delete_target, true, cx)
                        })),
                )
                .child(
                    Button::new("knowledge-library-back")
                        .ghost()
                        .disabled(!ready)
                        .label(t(cx, "返回列表", "Back to list"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            if view.phase == Phase::Ready {
                                view.selected = None;
                                cx.notify();
                            }
                        })),
                );
        } else {
            body = body
                .child(div().text_xs().child(t(
                    cx,
                    "搜索标题、正文或来源",
                    "Search title, text or source",
                )))
                .child(
                    Input::new(&self.query)
                        .id("knowledge-library-search")
                        .aria_label(t(cx, "搜索本地知识", "Search local knowledge")),
                );
            match self.matching_entries(cx) {
                None => {
                    body = body.child(t(
                        cx,
                        "搜索词最多 512 字节且不能包含控制字符。",
                        "Search accepts at most 512 bytes and no control characters.",
                    ));
                }
                Some(entries) => {
                    body = body.child(format!("{} / {}", entries.len(), self.entries.len()));
                    if entries.is_empty() {
                        body = body.child(t(
                            cx,
                            "没有匹配的知识条目。",
                            "No matching knowledge entries.",
                        ));
                    }
                    for entry in entries {
                        let title: String = entry.entry().title().chars().take(48).collect();
                        let context_button = self.context_button(&entry, ready, cx);
                        let management_checkbox = self.management_checkbox(&entry, ready, cx);
                        body = body.child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_start()
                                .gap_2()
                                .child(management_checkbox)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "knowledge-entry-{}",
                                                entry.id()
                                            )))
                                            .ghost()
                                            .w_full()
                                            .disabled(!ready)
                                            .label(title)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.select(&entry, window, cx)
                                            })),
                                        )
                                        .child(context_button),
                                ),
                        );
                    }
                }
            }
        }
        if !self.context.entries().is_empty() {
            let revision = self.context_revision;
            footer = footer.child(
                Button::new("knowledge-context-clear-library")
                    .ghost()
                    .label(t(cx, "清空 Ask 知识", "Clear Ask knowledge"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.context_revision == revision {
                            view.clear_context(cx);
                        }
                    })),
            );
        }
        if let Phase::NeedsReload(token) = self.phase {
            footer = footer.child(
                Button::new("knowledge-library-reload")
                    .primary()
                    .label(t(cx, "重新加载并重新审核", "Reload and review again"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.phase == Phase::NeedsReload(token) {
                            cx.emit(LibraryEvent::Reload(token));
                        }
                    })),
            );
        }
        div()
            .id("knowledge-library")
            .test_support()
            .h_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .text_sm()
            .text_color(rgb(visual.text))
            .bg(rgb(visual.surface))
            .child(
                div()
                    .p_3()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "本地知识库",
                        "Local knowledge library",
                    )))
                    .child(
                        div().text_xs().text_color(rgb(visual.muted)).child(
                            Message::new(
                                format!(
                                    "Ask 已选 {} / 8；历史资料不会恢复 SSH。",
                                    self.context.entries().len()
                                ),
                                format!(
                                    "Ask: {} / 8 selected. Historical records do not restore SSH.",
                                    self.context.entries().len()
                                ),
                            )
                            .render(cx),
                        ),
                    )
                    .child(
                        Button::new("knowledge-library-close")
                            .ghost()
                            .label(t(cx, "返回 AI 助手", "Back to AI assistant"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(LibraryEvent::Close))),
                    ),
            )
            .children(
                self.review
                    .is_none()
                    .then(|| self.management_selection_view(ready, cx)),
            )
            .child(
                div()
                    .id("knowledge-library-scroll")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll)
                    .p_3()
                    .child(body),
            )
            .child(
                div()
                    .id("knowledge-library-footer")
                    .test_support()
                    .flex_shrink_0()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_xs().child(self.message.render(cx)))
                    .child(footer),
            )
            .into_any_element()
    }
}
