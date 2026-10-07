//! Explicit text-conflict resolution and patch-to-draft controls.
use super::*;
use keelshell_core::{TextMergeChoice, TextMergePlan};

#[derive(Clone, Copy)]
enum MergeVersion {
    Base,
    Draft,
    Remote,
    Result,
}
pub(super) struct MergeReview {
    snapshot: RegularFileSnapshot,
    pub(super) plan: TextMergePlan,
    draft: String,
    base: String,
    overview: Option<MergeVersion>,
    choices: Vec<Option<TextMergeChoice>>,
    active: usize,
    manual_drafts: Vec<String>,
    pub(super) manual: Entity<TextareaState>,
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
        }) || self.editor.read(cx).value().as_ref() != draft.as_str()
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
        self.merge_review = Some(MergeReview {
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
        });
        self.status = Message::new(
            "已读取当前远端版本；解决冲突并采用合并草稿后，仍需审核全文才能保存。",
            "Current remote version read; resolve conflicts and adopt the draft, then review all contents before saving.",
        );
    }
    pub(super) fn request_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.busy || self.operation_id.is_some() || self.pending.is_some() {
            return;
        }
        if let Some((path, base)) = &self.editing {
            let operation = Operation::ReadMerge {
                path: path.clone(),
                base: base.clone(),
                draft: self.editor.read(cx).value().to_string(),
            };
            // A fresh observation supersedes every choice from the old review;
            // the user's editor draft remains untouched while the worker runs.
            self.merge_review = None;
            self.run(operation, window, cx);
        }
    }
    fn select_conflict(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(review) = &mut self.merge_review {
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
    fn choose_conflict(&mut self, choice: TextMergeChoice, cx: &mut Context<Self>) {
        if self.suspended || self.pending.is_some() {
            return;
        }
        if let Some(review) = &mut self.merge_review
            && review.active < review.choices.len()
        {
            review.choices[review.active] = Some(choice);
            cx.notify();
        }
    }
    fn adopt_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.busy || self.operation_id.is_some() || self.pending.is_some() {
            return;
        }
        let Some(review) = &self.merge_review else {
            return;
        };
        if self.editor.read(cx).value().as_ref() != review.draft.as_str() {
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
                self.editor
                    .update(cx, |input, cx| input.set_value(text, window, cx));
                self.editing = Some((snapshot.entry.path.clone(), snapshot.content.clone()));
                self.editor_snapshot = Some(snapshot);
                self.merge_review = None;
                self.diff_preview = None;
                self.status = Message::new(
                    "已采用合并草稿；远端未写入。请检查全文，再审核保存。",
                    "Merged draft adopted; remote unchanged. Inspect all contents, then review save.",
                );
            }
            Err(error) => {
                self.status =
                    Message::detail("无法采用合并草稿", "Unable to adopt merged draft", error)
            }
        }
        cx.notify();
    }
    pub(super) fn request_save(&mut self, cx: &mut Context<Self>) {
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
        let content = self.editor.read(cx).value().as_bytes().to_vec();
        if self
            .editor
            .read(cx)
            .value()
            .split_inclusive('\n')
            .take(4097)
            .count()
            > 4096
        {
            self.status = FileFailure::TextEdit(keelshell_core::TextEditError::Limit).message();
            cx.notify();
            return;
        }
        if content.len() > 1024 * 1024 || content.contains(&0) {
            self.status = FileFailure::InvalidEditor.message();
            cx.notify();
            return;
        }
        let text = String::from_utf8_lossy(&content);
        let mode = snapshot
            .entry
            .permissions
            .map(|mode| format!("{:04o}", mode & 0o7777))
            .unwrap_or_else(|| "—".into());
        let modified = modified_label(snapshot.entry.modified);
        let details_zh = format!(
            "{}\n{} → {} 字节\n权限：{mode}；修改时间(UTC)：{modified}；末尾 LF：{}\n",
            snapshot.entry.path,
            snapshot.content.len(),
            content.len(),
            if content.ends_with(b"\n") {
                "有"
            } else {
                "无"
            }
        );
        let details_en = format!(
            "{}\n{} → {} bytes\nMode: {mode}; modified (UTC): {modified}; final LF: {}\n",
            snapshot.entry.path,
            snapshot.content.len(),
            content.len(),
            if content.ends_with(b"\n") {
                "yes"
            } else {
                "no"
            }
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
    fn show_merge_version(&mut self, version: Option<MergeVersion>, cx: &mut Context<Self>) {
        if let Some(review) = &mut self.merge_review {
            review.overview = version;
            self.text_review_scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }
    pub(super) fn merge_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let review = self.merge_review.as_ref()?;
        Some(self.render_merge_card(review, cx))
    }
    fn render_merge_card(&self, review: &MergeReview, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
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
        let mut card = div()
            .id("text-merge-card")
            .flex_shrink_0()
            .mx_2()
            .my_1()
            .border_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .flex()
            .flex_col();
        card = card.child(div().px_2().py_1().flex_shrink_0().child(heading));
        card = card.child(self.merge_version_controls(resolved, total, cx));
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
        card.child(self.merge_apply_controls(resolved == total, cx))
            .test_support()
            .into_any_element()
    }
    fn merge_version_controls(
        &self,
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
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.show_merge_version(version, cx)),
                    )
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
                        view.select_conflict(next, window, cx)
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
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.choose_conflict(choice.clone(), cx)),
                    )
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
        let mut card = div().flex().flex_col();
        card = card.child(div().px_2().child(t(
            cx,
            "手工替换当前冲突（不会写入）",
            "Manual replacement of this conflict (no remote write)",
        )));
        card = card.child(
            div()
                .id("merge-manual-draft")
                .h(px(90.))
                .p_1()
                .child(
                    Textarea::new(&review.manual)
                        .h(relative(1.))
                        .font_family("monospace"),
                )
                .test_support(),
        );
        card.child(
            Button::new("merge-use-manual")
                .compact()
                .ghost()
                .disabled(self.suspended)
                .label(t(cx, "采用手工替换", "Choose manual replacement"))
                .on_click(cx.listener(|view, _, _, cx| {
                    if let Some(review) = &view.merge_review {
                        let text = review.manual.read(cx).value().to_string();
                        view.choose_conflict(TextMergeChoice::Manual(text), cx);
                    }
                })),
        )
        .into_any_element()
    }
    fn merge_apply_controls(&self, complete: bool, cx: &mut Context<Self>) -> AnyElement {
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
            .on_click(cx.listener(|view, _, window, cx| view.adopt_merge(window, cx)));
        let close = Button::new("close-text-merge")
            .ghost()
            .compact()
            .label(t(cx, "关闭并保留草稿", "Close; keep draft"))
            .on_click(cx.listener(|view, _, _, cx| {
                view.merge_review = None;
                cx.notify();
            }));
        div()
            .id("text-merge-apply-controls")
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
    pub(super) fn text_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let read = Button::new("read-remote-merge")
            .disabled(self.suspended || self.busy || self.pending.is_some())
            .icon(IconName::FileDiff)
            .ghost()
            .compact()
            .rounded(px(6.))
            .label(t(cx, "读取远端并合并", "Read remote and merge"))
            .on_click(cx.listener(|view, _, window, cx| view.request_merge(window, cx)));
        let patch = Button::new("toggle-text-patch")
            .disabled(self.suspended)
            .ghost()
            .compact()
            .rounded(px(6.))
            .label(t(cx, "应用差异到草稿", "Patch draft"))
            .on_click(cx.listener(|view, _, _, cx| {
                view.patch_visible = !view.patch_visible;
                cx.notify();
            }));
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(read)
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
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
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
