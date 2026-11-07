//! Explicit text-conflict resolution and patch-to-draft controls.
use super::*;
use keelshell_core::{TextMergeChoice, TextMergePlan};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum MergeVersion {
    Base,
    Draft,
    Remote,
    Result,
}
pub(super) struct MergeReview {
    /// Binds every rendered merge action to the observation that produced it.
    ///
    /// GPUI can retain an old frame until the next paint. A UUID lets those
    /// listeners fail closed if a newer remote observation has already
    /// replaced this review.
    pub(super) review_id: Uuid,
    snapshot: RegularFileSnapshot,
    pub(super) plan: TextMergePlan,
    draft: String,
    base: String,
    overview: Option<MergeVersion>,
    pub(super) choices: Vec<Option<TextMergeChoice>>,
    pub(super) active: usize,
    manual_drafts: Vec<String>,
    pub(super) manual: Entity<TextareaState>,
    /// Inline review cards need their own body viewport; the expanded surface
    /// continues to use `tools_scroll` for the complete review.
    pub(super) body_scroll: ScrollHandle,
}
impl FilesPanel {
    pub(super) fn receive_merge(
        &mut self,
        snapshot: RegularFileSnapshot,
        plan: TextMergePlan,
        base: Vec<u8>,
        draft: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.as_ref().is_none_or(|(path, current_base)| {
            path != &snapshot.entry.path || current_base != &base
        }) || !self.draft_matches_bytes(&self.editor.read(cx).value(), draft.as_bytes())
        {
            self.status = Message::new(
                "读取期间草稿、文件或基线已变化，保留当前草稿；请重新读取并合并。",
                "Draft, selected file or baseline changed during the read; preserved it. Read and merge again.",
            );
            return;
        }
        let manual = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx);
            input.set_value(
                plan.conflicts()
                    .first()
                    .map(|conflict| conflict.draft.clone())
                    .unwrap_or_default(),
                window,
                cx,
            );
            input
        });
        self.text_review_scroll.set_offset(point(px(0.), px(0.)));
        // Display exactly the base used by this worker's plan. A mutable editor
        // baseline must never substitute a different version into that review.
        let base = String::from_utf8_lossy(&base).into_owned();
        let expanded = self.editor_surface.take().is_some();
        self.merge_review = Some(MergeReview {
            review_id: Uuid::new_v4(),
            snapshot,
            base,
            overview: None,
            choices: vec![None; plan.conflicts().len()],
            manual_drafts: plan
                .conflicts()
                .iter()
                .map(|conflict| conflict.draft.clone())
                .collect(),
            plan,
            draft,
            active: 0,
            manual,
            body_scroll: ScrollHandle::new(),
        });
        if expanded {
            // The old editor has just been unmounted. Its retained input must
            // lose focus before the merge review can receive keyboard events.
            self.focus_editor(review::EditorSurface::Merge, window, cx);
        }
        self.status = Message::new(
            "已读取当前远端版本；解决冲突并采用合并草稿后，仍需审核全文才能保存。",
            "Current remote version read; resolve conflicts and adopt the draft, then review all contents before saving.",
        );
    }
    pub(super) fn request_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.busy || self.operation_id.is_some() || self.pending.is_some() {
            return;
        }
        if self.editor_reload_required {
            self.status = FileFailure::EditorFormatChanged.message();
            cx.notify();
            return;
        }
        if let Some((path, base)) = &self.editing {
            let draft = match self.prepare_editor_bytes(&self.editor.read(cx).value()) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(_) => {
                        self.status = FileFailure::InvalidEditor.message();
                        cx.notify();
                        return;
                    }
                },
                Err(error) => {
                    self.status = error.message();
                    cx.notify();
                    return;
                }
            };
            let operation = Operation::ReadMerge {
                path: path.clone(),
                base: base.clone(),
                draft,
            };
            // A fresh observation supersedes every choice from the old review;
            // the user's editor draft remains untouched while the worker runs.
            self.merge_review = None;
            self.run(operation, window, cx);
        }
    }
    fn select_conflict(
        &mut self,
        review_id: Uuid,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(review) = self
            .merge_review
            .as_mut()
            .filter(|review| review.review_id == review_id)
        {
            if index >= review.plan.conflicts().len() {
                return;
            }
            let current = review.manual.read(cx).value().to_string();
            if matches!(&review.choices[review.active],Some(TextMergeChoice::Manual(text)) if text != &current)
            {
                review.choices[review.active] = None;
            }
            review.manual_drafts[review.active] = current;
            review.active = index;
            review.overview = None;
            let text = review.manual_drafts[index].clone();
            review
                .manual
                .update(cx, |input, cx| input.set_value(text, window, cx));
            self.text_review_scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }
    fn choose_conflict(
        &mut self,
        review_id: Uuid,
        choice: TextMergeChoice,
        cx: &mut Context<Self>,
    ) {
        if self.suspended || self.pending.is_some() {
            return;
        }
        if let Some(review) = self
            .merge_review
            .as_mut()
            .filter(|review| review.review_id == review_id)
            && review.active < review.choices.len()
        {
            review.choices[review.active] = Some(choice);
            cx.notify();
        }
    }
    fn adopt_merge(&mut self, review_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.busy || self.operation_id.is_some() || self.pending.is_some() {
            return;
        }
        let Some(review) = self
            .merge_review
            .as_ref()
            .filter(|review| review.review_id == review_id)
        else {
            return;
        };
        if !self.draft_matches_bytes(&self.editor.read(cx).value(), review.draft.as_bytes()) {
            self.status = Message::new(
                "冲突审核后草稿已变化，请重新读取并合并；当前草稿已保留。",
                "Draft changed after conflict review. Read and merge again; current draft retained.",
            );
            cx.notify();
            return;
        }
        if let Some(Some(TextMergeChoice::Manual(text))) = review.choices.get(review.active)
            && review.manual.read(cx).value().as_ref() != text.as_str()
        {
            self.status = Message::new(
                "手工替换内容已变化，请重新点击采用手工替换。",
                "Manual replacement changed; choose that replacement again.",
            );
            cx.notify();
            return;
        }
        match review.plan.resolve(&review.choices) {
            Ok(text) => {
                let snapshot = review.snapshot.clone();
                let document = match Utf8TextDocument::decode(text.as_bytes()) {
                    Ok(document) => document,
                    Err(error) => {
                        self.status = FileFailure::TextDocument(error).message();
                        cx.notify();
                        return;
                    }
                };
                let original = match Utf8TextDocument::decode(&snapshot.content) {
                    Ok(document) => document,
                    Err(error) => {
                        self.status = FileFailure::TextDocument(error).message();
                        cx.notify();
                        return;
                    }
                };
                let format = original
                    .format()
                    .retain_insertion_policy(self.editor_format.unwrap_or(original.format()));
                // Remove a signature only when the owning remote envelope has one.
                // A new leading FEFF on a no-BOM baseline remains draft data.
                let body = if format.has_bom() {
                    document.body().to_owned()
                } else {
                    text
                };
                self.editor
                    .update(cx, |input, cx| input.set_value(body, window, cx));
                self.editing = Some((snapshot.entry.path.clone(), snapshot.content.clone()));
                self.editor_format = Some(format);
                self.editor_snapshot = Some(snapshot);
                self.merge_review = None;
                self.diff_preview = None;
                self.status = Message::new(
                    "已采用合并草稿；远端未写入。请检查全文，再审核保存。",
                    "Merged draft adopted; remote unchanged. Inspect all contents, then review save.",
                );
                if self.editor_surface == Some(review::EditorSurface::Merge) {
                    self.focus_editor(review::EditorSurface::Draft, window, cx);
                }
            }
            Err(error) => {
                self.status =
                    Message::detail("无法采用合并草稿", "Unable to adopt merged draft", error)
            }
        }
        cx.notify();
    }
    pub(super) fn request_save(&mut self, cx: &mut Context<Self>) {
        if self.editor_reload_required {
            self.status = FileFailure::EditorFormatChanged.message();
            cx.notify();
            return;
        }
        if self.merge_review.is_some() {
            self.status = Message::new(
                "请先解决并采用合并草稿，或关闭合并审核保留当前草稿。",
                "Resolve and adopt the merge, or close its review to keep the current draft.",
            );
            cx.notify();
            return;
        }
        let Some(snapshot) = self.editor_snapshot.clone() else {
            self.status = Message::new(
                "缺少已校验的文件基线，请重新打开文件；当前草稿已保留。",
                "No checked file baseline. Open the file again; current draft retained.",
            );
            cx.notify();
            return;
        };
        if self
            .editing
            .as_ref()
            .is_none_or(|(path, base)| path != &snapshot.entry.path || base != &snapshot.content)
        {
            self.status = Message::new(
                "编辑目标或基线已变化，请重新读取并审核；当前草稿保留。",
                "Editor target or baseline changed. Read and review again; draft retained.",
            );
            cx.notify();
            return;
        }
        let content = match self.prepare_editor_bytes(&self.editor.read(cx).value()) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status = error.message();
                cx.notify();
                return;
            }
        };
        let document = match Utf8TextDocument::decode(&content) {
            Ok(document) => document,
            Err(error) => {
                self.status = FileFailure::TextDocument(error).message();
                cx.notify();
                return;
            }
        };
        let description = text_format_description(document.format());
        let text = String::from_utf8_lossy(&content);
        let mode = snapshot
            .entry
            .permissions
            .map(|mode| format!("{:04o}", mode & 0o7777))
            .unwrap_or_else(|| "—".into());
        let modified = modified_label(snapshot.entry.modified);
        let details_zh = format!(
            "{}\n{} → {} 字节\n{}；权限：{mode}；修改时间(UTC)：{modified}；末尾 LF：{}；末尾 CR：{}\n",
            snapshot.entry.path,
            snapshot.content.len(),
            content.len(),
            description.0,
            if content.ends_with(b"\n") {
                "有"
            } else {
                "无"
            },
            if content.ends_with(b"\r") {
                "有"
            } else {
                "无"
            },
        );
        let details_en = format!(
            "{}\n{} → {} bytes\n{}; mode: {mode}; modified (UTC): {modified}; final LF: {}; final CR: {}\n",
            snapshot.entry.path,
            snapshot.content.len(),
            content.len(),
            description.1,
            if content.ends_with(b"\n") {
                "yes"
            } else {
                "no"
            },
            if content.ends_with(b"\r") {
                "yes"
            } else {
                "no"
            },
        );
        self.confirm(Message::new(format!("审核最终全文并保存？\n{details_zh}先复核原文件内容与元数据，发布前再复核；原子替换后完整读回。仅保留普通 rwx 权限，不保留所有者、ACL 或特殊模式。SFTP 不提供远端比较交换锁；不自动重试。\n── 最终全文 ──\n{text}"),format!("Review the complete result and save?\n{details_en}Recheck original content/metadata before staging and publication; read back all published bytes. Only ordinary rwx permissions are preserved, excluding ownership, ACLs and special modes. SFTP has no remote compare-and-swap lock; no automatic retry.\n── Complete result ──\n{text}")),Operation::Save{path:snapshot.entry.path.clone(),reviewed:snapshot,content},cx);
    }
    pub(super) fn request_patch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.suspended {
            return;
        }
        if let Some((path, _)) = &self.editing {
            self.run(
                Operation::ApplyPatchToDraft {
                    path: path.clone(),
                    draft: self.editor.read(cx).value().to_string(),
                    patch: self.patch.read(cx).value().to_string(),
                },
                window,
                cx,
            );
        }
    }
    fn show_merge_version(
        &mut self,
        review_id: Uuid,
        version: Option<MergeVersion>,
        cx: &mut Context<Self>,
    ) {
        if let Some(review) = self
            .merge_review
            .as_mut()
            .filter(|review| review.review_id == review_id)
        {
            review.overview = version;
            self.text_review_scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }
    fn close_merge_review(&mut self, review_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .merge_review
            .as_ref()
            .is_none_or(|review| review.review_id != review_id)
        {
            return;
        }
        self.merge_review = None;
        if self.editor_surface == Some(review::EditorSurface::Merge) {
            self.focus_editor(review::EditorSurface::Draft, window, cx);
        }
        cx.notify();
    }
    pub(super) fn merge_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let review = self.merge_review.as_ref()?;
        Some(self.render_merge_card(review, cx))
    }
    fn render_merge_card(&self, review: &MergeReview, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::scroll::ScrollableElement;
        let visual = crate::design::palette(cx);
        let review_id = review.review_id;
        div()
            .id("text-merge-card")
            .flex_shrink_0()
            .min_h_0()
            .h_full()
            .mx_2()
            .my_1()
            .border_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .flex()
            .flex_col()
            .child(
                div()
                    .id("text-merge-inline-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&review.body_scroll)
                    .vertical_scrollbar(&review.body_scroll)
                    .on_scroll(|_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(move |view, event, _, cx| {
                        if let Some(review) = view
                            .merge_review
                            .as_ref()
                            .filter(|review| review.review_id == review_id)
                        {
                            Self::scroll_bounded(&review.body_scroll, event, cx);
                        }
                    }))
                    .child(self.merge_review_body_without_versions(review, cx))
                    .test_support(),
            )
            .child(self.merge_inline_action_bar(review, cx))
            .test_support()
            .into_any_element()
    }

    fn merge_review_body_without_versions(
        &self,
        review: &MergeReview,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.merge_review_body_inner(review, false, cx)
    }
    pub(super) fn merge_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::scroll::ScrollableElement;
        let Some(review) = &self.merge_review else {
            return self.editor_surface(review::EditorSurface::Draft, cx);
        };
        let visual = crate::design::palette(cx);
        div()
            .id("file-merge-expanded")
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
                    .id("file-tools-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_focus(&self.review_focus)
                    .track_scroll(&self.tools_scroll)
                    .vertical_scrollbar(&self.tools_scroll)
                    .on_key_down(cx.listener(|view, event, _, cx| {
                        Self::scroll_bounded(&view.tools_scroll, event, cx);
                    }))
                    .child(self.merge_review_body_inner(review, true, cx))
                    .test_support(),
            )
            // The complete versions and conflict inputs scroll independently
            // of these actions, even at the minimum file-pane height.
            .child(self.merge_apply_controls(
                review.review_id,
                review.choices.iter().all(Option::is_some),
                cx,
            ))
            .child(self.editor_status(cx))
            .test_support()
            .into_any_element()
    }
    fn merge_review_body_inner(
        &self,
        review: &MergeReview,
        include_version_controls: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let total = review.choices.len();
        let resolved = review
            .choices
            .iter()
            .filter(|choice| choice.is_some())
            .count();
        let heading = format!(
            "{} · {resolved}/{total} · {}",
            t(cx, "三方合并审核", "Three-way merge review"),
            review.snapshot.entry.path
        );
        let mut card = div().min_w_0().flex_shrink_0().flex().flex_col();
        card = card.child(
            div()
                .px_2()
                .py_1()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1()
                .child(div().flex_1().min_w_0().text_ellipsis().child(heading))
                .when(
                    self.editor_surface != Some(review::EditorSurface::Merge),
                    |el| {
                        let review_id = review.review_id;
                        el.child(
                            Button::new("expand-text-merge")
                                .ghost()
                                .compact()
                                .label(t(cx, "展开合并审核", "Expand merge review"))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if view
                                        .merge_review
                                        .as_ref()
                                        .is_some_and(|review| review.review_id == review_id)
                                    {
                                        view.focus_editor(review::EditorSurface::Merge, window, cx);
                                    }
                                })),
                        )
                    },
                ),
        );
        if include_version_controls {
            card = card.child(self.merge_version_controls(review.review_id, resolved, total, cx));
        }
        if let Some(version) = review.overview {
            let (label, text) = match version {
                MergeVersion::Base => (
                    t(cx, "打开时完整基线", "Complete baseline at open"),
                    review.base.clone(),
                ),
                MergeVersion::Draft => (
                    t(cx, "读取时完整草稿", "Complete draft at read"),
                    review.draft.clone(),
                ),
                MergeVersion::Remote => (
                    t(cx, "已校验当前远端全文", "Checked current remote contents"),
                    String::from_utf8_lossy(&review.snapshot.content).into_owned(),
                ),
                MergeVersion::Result => (
                    t(
                        cx,
                        "已解决的合并全文（尚未写入）",
                        "Resolved complete merge (remote unchanged)",
                    ),
                    review
                        .plan
                        .resolve(&review.choices)
                        .unwrap_or_else(|error| error.to_string()),
                ),
            };
            let final_lf = if text.ends_with('\n') {
                t(cx, "有", "yes")
            } else {
                t(cx, "无", "no")
            };
            let text = format!(
                "{label} · {} {} · {}: {final_lf}\n{text}",
                text.len(),
                t(cx, "字节", "bytes"),
                t(cx, "末尾 LF", "final LF"),
            );
            card = card.child(text_scroll(
                "text-merge-complete-version",
                &text,
                &self.text_review_scroll,
                160.,
                cx,
            ));
        }
        if let Some(conflict) = review.plan.conflicts().get(review.active) {
            if review.overview.is_none() {
                let region = if conflict.base_lines.is_empty() {
                    format!(
                        "{} {}",
                        t(cx, "插入位置", "Insertion position"),
                        conflict.base_lines.start
                    )
                } else {
                    format!(
                        "{} {}–{}",
                        t(cx, "基线行", "Base lines"),
                        conflict.base_lines.start + 1,
                        conflict.base_lines.end
                    )
                };
                let text = format!(
                    "{} {} / {total} · {region}\n\n── {} ──\n{}\n── {} ──\n{}\n── {} ──\n{}",
                    t(cx, "冲突", "Conflict"),
                    review.active + 1,
                    t(cx, "打开时基线", "Baseline at open"),
                    conflict.base,
                    t(cx, "本地草稿", "Local draft"),
                    conflict.draft,
                    t(cx, "当前远端", "Current remote"),
                    conflict.remote
                );
                card = card.child(text_scroll(
                    "text-merge-conflict-details",
                    &text,
                    &self.text_review_scroll,
                    160.,
                    cx,
                ));
            }
            card = card.child(self.merge_conflict_controls(review, cx));
            card = card.child(self.merge_manual_card(review, cx));
        } else {
            card=card.child(div().p_2().child(t(cx,"无重叠冲突，已组合两侧独立更改；仍需采用草稿并审核全文。","No overlapping conflicts; independent edits combined. Adopt the draft and review all contents.")));
        }
        card.into_any_element()
    }
    fn merge_version_controls(
        &self,
        review_id: Uuid,
        resolved: usize,
        total: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let choices = [
            (
                "merge-show-base",
                t(cx, "完整基线", "Complete base"),
                Some(MergeVersion::Base),
            ),
            (
                "merge-show-draft",
                t(cx, "完整草稿", "Complete draft"),
                Some(MergeVersion::Draft),
            ),
            (
                "merge-show-remote",
                t(cx, "完整远端", "Complete remote"),
                Some(MergeVersion::Remote),
            ),
            (
                "merge-show-result",
                t(cx, "合并全文", "Complete merge"),
                Some(MergeVersion::Result),
            ),
            (
                "merge-show-conflict",
                t(cx, "当前冲突", "Current conflict"),
                None,
            ),
        ];
        let buttons = choices
            .into_iter()
            .map(|(id, label, version)| {
                Button::new(id)
                    .ghost()
                    .compact()
                    .disabled(
                        (id == "merge-show-result" && resolved != total)
                            || (id == "merge-show-conflict" && total == 0),
                    )
                    .label(label)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.show_merge_version(review_id, version, cx)
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .px_2()
            .py_1()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(buttons)
            .into_any_element()
    }
    fn merge_conflict_controls(&self, review: &MergeReview, cx: &mut Context<Self>) -> AnyElement {
        let review_id = review.review_id;
        let index = review.active;
        let total = review.choices.len();
        let navigation = [
            (
                "merge-previous-conflict",
                t(cx, "上一项", "Previous"),
                index.saturating_sub(1),
                index == 0,
            ),
            (
                "merge-next-conflict",
                t(cx, "下一项", "Next"),
                index + 1,
                index + 1 >= total,
            ),
        ];
        let mut buttons = navigation
            .into_iter()
            .map(|(id, label, next, disabled)| {
                Button::new(id)
                    .compact()
                    .ghost()
                    .disabled(disabled)
                    .label(label)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_conflict(review_id, next, window, cx)
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        for (id, label, choice) in [
            (
                "merge-use-draft",
                t(cx, "保留草稿", "Keep draft"),
                TextMergeChoice::Draft,
            ),
            (
                "merge-use-remote",
                t(cx, "保留远端", "Keep remote"),
                TextMergeChoice::Remote,
            ),
        ] {
            buttons.push(
                Button::new(id)
                    .compact()
                    .ghost()
                    .disabled(self.suspended)
                    .label(label)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.choose_conflict(review_id, choice.clone(), cx)
                    }))
                    .into_any_element(),
            );
        }
        let status = match review.choices[index].as_ref() {
            Some(TextMergeChoice::Draft) => t(cx, "已选草稿", "Draft selected"),
            Some(TextMergeChoice::Remote) => t(cx, "已选远端", "Remote selected"),
            Some(TextMergeChoice::Manual(_)) => t(cx, "已选手工内容", "Manual selected"),
            None => t(cx, "尚未解决", "Unresolved"),
        };
        let visual = crate::design::palette(cx);
        div()
            .px_2()
            .py_1()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(buttons)
            .child(div().text_color(rgb(visual.accent)).child(status))
            .into_any_element()
    }
    fn merge_manual_card(&self, review: &MergeReview, cx: &mut Context<Self>) -> AnyElement {
        let review_id = review.review_id;
        let mut card = div().flex().flex_col();
        card = card.child(div().px_2().child(t(
            cx,
            "手工替换当前冲突（不会写入）",
            "Manual replacement of this conflict (no remote write)",
        )));
        card = card.child(
            div()
                .id("merge-manual-draft")
                // Keep the conflict editor fully reachable in the smallest
                // supported file-pane viewport. The expanded merge surface
                // reserves space for its fixed actions and status row, so a
                // taller field cannot be brought wholly into view in the
                // 900×580 window's compact file workspace.
                .h(px(72.))
                .p_1()
                .child(
                    div()
                        .id("merge-manual-input")
                        .h(relative(1.))
                        .min_h_0()
                        .min_w_0()
                        .child(
                            Textarea::new(&review.manual)
                                .h(relative(1.))
                                .font_family("monospace"),
                        )
                        .test_support(),
                )
                .test_support(),
        );
        card.child(
            Button::new("merge-use-manual")
                .compact()
                .ghost()
                .disabled(self.suspended)
                .label(t(cx, "采用手工替换", "Choose manual replacement"))
                .on_click(cx.listener(move |view, _, _, cx| {
                    if let Some(review) = view
                        .merge_review
                        .as_ref()
                        .filter(|review| review.review_id == review_id)
                    {
                        let text = review.manual.read(cx).value().to_string();
                        view.choose_conflict(review_id, TextMergeChoice::Manual(text), cx);
                    }
                })),
        )
        .into_any_element()
    }
    fn merge_apply_controls(
        &self,
        review_id: Uuid,
        complete: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let adopt = Button::new("adopt-text-merge")
            .primary()
            .compact()
            .disabled(
                self.suspended
                    || self.busy
                    || self.operation_id.is_some()
                    || self.pending.is_some()
                    || !complete,
            )
            .label(t(cx, "采用合并草稿", "Adopt merged draft"))
            .on_click(
                cx.listener(move |view, _, window, cx| view.adopt_merge(review_id, window, cx)),
            );
        let close = Button::new("close-text-merge")
            .ghost()
            .compact()
            .label(t(cx, "关闭并保留草稿", "Close; keep draft"))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.close_merge_review(review_id, window, cx)
            }));
        div()
            .id("text-merge-apply-controls")
            .min_h(px(40.))
            .px_2()
            .py_1()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(adopt)
            .child(close)
            .test_support()
            .into_any_element()
    }

    fn merge_inline_action_bar(&self, review: &MergeReview, cx: &mut Context<Self>) -> AnyElement {
        let complete = review.choices.iter().all(Option::is_some);
        div()
            .id("text-merge-inline-actions")
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(crate::design::palette(cx).border))
            .bg(rgb(crate::design::palette(cx).canvas))
            .child(
                self.merge_version_controls(
                    review.review_id,
                    review
                        .choices
                        .iter()
                        .filter(|choice| choice.is_some())
                        .count(),
                    review.choices.len(),
                    cx,
                ),
            )
            .child(self.merge_apply_controls(review.review_id, complete, cx))
            .test_support()
            .into_any_element()
    }
    pub(super) fn patch_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.patch_visible || self.editing.is_none() {
            return None;
        }
        Some(self.render_patch_card(cx))
    }
    fn render_patch_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut card = div()
            .id("text-patch-card")
            .flex_shrink_0()
            .mx_2()
            .my_1()
            .border_1()
            .border_color(rgb(visual.border));
        card=card.child(div().p_2().child(t(cx,"粘贴单文件 unified diff；两行头必须是当前完整远端路径。精确匹配草稿，只修改草稿。","Paste a single-file unified diff; both headers must use the complete current remote path. Exact draft matching; draft changes only.")));
        card = card.child(
            div()
                .id("text-patch-input")
                .h(px(100.))
                .p_1()
                .child(
                    Textarea::new(&self.patch)
                        .h(relative(1.))
                        .font_family("monospace"),
                )
                .test_support(),
        );
        card.child(
            Button::new("apply-text-patch")
                .ghost()
                .compact()
                .disabled(self.suspended || self.busy || self.pending.is_some())
                .label(t(cx, "解析并应用到草稿", "Parse and apply to draft"))
                .on_click(cx.listener(|view, _, window, cx| view.request_patch(window, cx))),
        )
        .test_support()
        .into_any_element()
    }
    pub(super) fn read_merge_button(&self, cx: &Context<Self>) -> Button {
        Button::new("read-remote-merge")
            .disabled(self.suspended || self.busy || self.pending.is_some())
            .icon(IconName::FileDiff)
            .ghost()
            .compact()
            .rounded(px(6.))
            .label(t(cx, "读取远端并合并", "Read remote and merge"))
            .on_click(cx.listener(|view, _, window, cx| view.request_merge(window, cx)))
    }
    pub(super) fn text_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let patch = Button::new("toggle-text-patch")
            .disabled(self.suspended)
            .ghost()
            .compact()
            .rounded(px(6.))
            .label(t(cx, "应用差异到草稿", "Patch draft"))
            .on_click(cx.listener(|view, _, window, cx| {
                view.focus_editor(review::EditorSurface::Patch, window, cx);
            }));
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(self.read_merge_button(cx))
            .child(patch)
            .into_any_element()
    }
}

/// Original text rows retain their intrinsic widths; actions stay outside this
/// viewport so both long paths and complete unwrapped file lines are inspectable.
pub(super) fn text_scroll(
    id: &'static str,
    text: &str,
    scroll: &ScrollHandle,
    height: f32,
    cx: &App,
) -> impl IntoElement {
    use gpui_kit::component::scroll::{ScrollableElement, ScrollbarAxis};
    let visual = crate::design::palette(cx);
    div()
        .id(id)
        // Keep a real outer gutter so the containing tools remain scrollable
        // when this independently scrolling viewport fills their visible area.
        .mx_1()
        .h(px(height))
        .min_h_0()
        .overflow_x_scroll()
        .overflow_y_scroll()
        .track_scroll(scroll)
        // Inner text gestures must not also move the outer tools viewport.
        // GPUI applies the built-in scroll listener before this bubble guard.
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .font_family("monospace")
        .text_xs()
        .text_color(rgb(visual.text))
        .bg(rgb(visual.canvas))
        .children(text.split('\n').enumerate().map(|(index, line)| {
            div()
                .id((id, index))
                .flex_shrink_0()
                .min_h(px(18.))
                .line_height(px(18.))
                .whitespace_nowrap()
                .role(accesskit::Role::Label)
                .aria_label(line.to_owned())
                .child(line.to_owned())
                .test_support()
        }))
        .scrollbar(scroll, ScrollbarAxis::Both)
        .test_support()
}
