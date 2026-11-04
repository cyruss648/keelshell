//! Remote browser layout and explicit file-operation controls.
use super::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement;

fn directory_compare_card(comparison: &DirectoryComparison, cx: &App) -> impl IntoElement {
    let visual = crate::design::palette(cx);
    let report = &comparison.report;
    let summary = match crate::i18n::language(cx) {
        keelshell_core::Language::ZhCn => format!(
            "{} 一致 · {} 已变化 · {} 仅本地 · {} 仅远端 · {} 待确认",
            report.same_count(),
            report.changed_count(),
            report.left_only_count(),
            report.right_only_count(),
            report.uncertain_count(),
        ),
        keelshell_core::Language::En => format!(
            "{} same · {} changed · {} local only · {} remote only · {} uncertain",
            report.same_count(),
            report.changed_count(),
            report.left_only_count(),
            report.right_only_count(),
            report.uncertain_count(),
        ),
    };
    let mut rows = div().flex().flex_col().gap_1();
    for row in report.rows().iter().take(100) {
        rows = rows.child(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .w(px(92.))
                        .flex_shrink_0()
                        .child(compare_status_label(row.status, cx)),
                )
                .child(div().min_w_0().text_ellipsis().child(row.path.clone())),
        );
    }
    if report.rows().len() > 100 {
        rows = rows.child(div().text_color(rgb(visual.muted)).child(
            match crate::i18n::language(cx) {
                keelshell_core::Language::ZhCn => {
                    format!("仅显示前 100 / {} 项", report.rows().len())
                }
                keelshell_core::Language::En => {
                    format!("Showing the first 100 of {} entries", report.rows().len())
                }
            },
        ));
    }
    let mut card = div()
        .id("directory-comparison-card")
        .flex_shrink_0()
        .mx_2()
        .my_1()
        .p_2()
        .max_h(px(140.))
        .overflow_y_scroll()
        .border_1()
        .border_color(rgb(visual.border))
        .bg(rgb(visual.canvas))
        .child(
            div()
                .flex()
                .gap_2()
                .child(IconName::FileDiff)
                .child(t(
                    cx,
                    "目录比较（只读）",
                    "Directory comparison (read-only)",
                ))
                .child(div().flex_1().text_color(rgb(visual.muted)).child(summary)),
        )
        .child(div().text_color(rgb(visual.muted)).child(format!(
            "{} ↔ {}",
            comparison.local.display(),
            comparison.remote
        )))
        .child(rows)
        .test_support();
    if let Some(plan) = &comparison.sync_plan {
        let direction = if plan.direction() == DirectorySyncDirection::LeftToRight {
            t(cx, "本地 → 远端", "Local → remote")
        } else {
            t(cx, "远端 → 本地", "Remote → local")
        };
        card = card.child(div().text_color(rgb(visual.accent)).child(
            match crate::i18n::language(cx) {
                keelshell_core::Language::ZhCn => format!(
                    "内容已校验 · {direction} · {} 项 · {}",
                    plan.operation_count(),
                    if plan.is_bounded_mirror() {
                        "包含明确删除；仍需完整审核"
                    } else {
                        "保留目标独有项"
                    }
                ),
                keelshell_core::Language::En => format!(
                    "Content verified · {direction} · {} operations · {}",
                    plan.operation_count(),
                    if plan.is_bounded_mirror() {
                        "explicit deletions included; review required"
                    } else {
                        "preserve destination-only entries"
                    }
                ),
            },
        ));
    }
    card
}

fn compare_status_label(status: DirectoryEntryStatus, cx: &App) -> SharedString {
    match status {
        DirectoryEntryStatus::Same => t(cx, "一致", "Same").into(),
        DirectoryEntryStatus::Changed => t(cx, "已变化", "Changed").into(),
        DirectoryEntryStatus::LeftOnly => t(cx, "仅本地", "Local only").into(),
        DirectoryEntryStatus::RightOnly => t(cx, "仅远端", "Remote only").into(),
        DirectoryEntryStatus::Uncertain => t(cx, "待确认", "Uncertain").into(),
    }
}

impl FilesPanel {
    // Keep remote list construction in a separate frame. The debug GPUI
    // builders otherwise accumulate over a megabyte of temporary stack state
    // before calling either local browser surface on a normal test thread.
    fn remote_browser_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        // Both nested scrollers must keep their own axis; GPUI otherwise
        // remaps one wheel delta onto the rows and the columns simultaneously.
        let mut list = div()
            .id("remote-files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .flex()
            .flex_col()
            .test_support();
        for (index, source_index) in
            browser::remote_indices(&self.entries, self.remote_show_hidden, self.remote_sort)
                .into_iter()
                .enumerate()
        {
            // Rendering IDs identify visible row positions; every action and
            // selection captures the unchanged native entry/path instead.
            let entry = &self.entries[source_index];
            let selected = self.remote_selection.contains(&entry.path);
            let item = entry.clone();
            let directory = self.directory.clone();
            let check_directory = self.directory.clone();
            let check_path = entry.path.clone();
            let open = entry.clone();
            list = list.child(
                div()
                    .id(("remote-entry", index))
                    .h(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .bg(rgb(if selected {
                        visual.selected
                    } else if index % 2 == 0 {
                        visual.surface
                    } else {
                        visual.canvas
                    }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                        view.select_remote_entry(
                            &item.path,
                            &directory,
                            event.modifiers(),
                            false,
                            window,
                            cx,
                        );
                    }))
                    .child(
                        Checkbox::new(("select-remote-entry", index))
                            .small()
                            .checked(selected)
                            .disabled(self.suspended)
                            .accessibility_label(format!(
                                "{} {}",
                                t(cx, "选择远程项", "Select remote entry"),
                                entry.name
                            ))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                cx.stop_propagation();
                                view.select_remote_entry(
                                    &check_path,
                                    &check_directory,
                                    Modifiers::default(),
                                    true,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(150.))
                            .px_2()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(rgb(if entry.is_directory {
                                        visual.accent
                                    } else {
                                        visual.muted
                                    }))
                                    .child(if entry.is_directory {
                                        IconName::Folder
                                    } else if entry.is_symlink {
                                        IconName::Link
                                    } else {
                                        IconName::FileText
                                    }),
                            )
                            .child(div().min_w_0().text_ellipsis().child(entry.name.clone())),
                    )
                    .child(table_cell(size_label(entry.size), 88.))
                    .child(table_cell(type_label(entry, cx), 76.))
                    .child(table_cell(modified_label(entry.modified), 143.))
                    .child(table_cell(permissions_label(entry), 100.))
                    .child(
                        div().w(px(60.)).flex_shrink_0().child(
                            Button::new(("open-remote", index))
                                .disabled(
                                    self.suspended
                                        || (!entry.is_directory && self.remote_selection.len() > 1),
                                )
                                .ghost()
                                .compact()
                                .rounded(px(crate::design::components(cx).control_radius))
                                .label(if entry.is_directory {
                                    t(cx, "打开", "Open")
                                } else {
                                    t(cx, "编辑", "Edit")
                                })
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    // A row selection withdraws unexecuted proposals;
                                    // it must not clear this action's dirty-editor review.
                                    cx.stop_propagation();
                                    if open.is_directory {
                                        view.run(Operation::List(open.path.clone()), window, cx);
                                    } else if view.remote_selection.len() <= 1 {
                                        view.request_read(open.clone(), window, cx);
                                    }
                                })),
                        ),
                    )
                    .test_support(),
            );
        }
        if browser::remote_indices(&self.entries, self.remote_show_hidden, self.remote_sort)
            .is_empty()
            && !self.busy
        {
            list = list.child(div().p_3().text_color(rgb(visual.muted)).child(t(
                cx,
                "当前目录没有文件",
                "This directory is empty",
            )));
        }
        let drop_binding = self.batch_binding();
        let drop_available = self.drop_upload_available();
        let table = div()
            .id("remote-files-table")
            .flex_1()
            .min_w_0()
            .h_full()
            .when(drop_available, |table| {
                table.drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.border_color(rgb(visual.accent))
                })
            })
            .on_drop(
                cx.listener(move |panel, paths: &ExternalPaths, window, cx| {
                    cx.stop_propagation();
                    panel.request_dropped_upload(paths, drop_binding.as_ref(), window, cx);
                }),
            )
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .test_support()
            .child(
                div()
                    .min_w(px(650.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(28.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .bg(rgb(visual.canvas))
                            .border_b_1()
                            .border_color(rgb(visual.border))
                            .child(
                                Button::new("sort-remote-name")
                                    .ghost()
                                    .compact()
                                    .flex_1()
                                    .min_w(px(150.))
                                    .label(format!(
                                        "{}{}",
                                        t(cx, "远程名称", "Remote name"),
                                        self.remote_sort.indicator(browser::SortColumn::Name)
                                    ))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.remote_sort.select(browser::SortColumn::Name);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("sort-remote-size")
                                    .ghost()
                                    .compact()
                                    .w(px(88.))
                                    .label(format!(
                                        "{}{}",
                                        t(cx, "大小", "Size"),
                                        self.remote_sort.indicator(browser::SortColumn::Size)
                                    ))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.remote_sort.select(browser::SortColumn::Size);
                                        cx.notify();
                                    })),
                            )
                            .child(table_cell(t(cx, "类型", "Type"), 76.))
                            .child(
                                Button::new("sort-remote-modified")
                                    .ghost()
                                    .compact()
                                    .w(px(143.))
                                    .label(format!(
                                        "{}{}",
                                        t(cx, "修改时间 (UTC)", "Modified (UTC)"),
                                        self.remote_sort.indicator(browser::SortColumn::Modified)
                                    ))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.remote_sort.select(browser::SortColumn::Modified);
                                        cx.notify();
                                    })),
                            )
                            .child(table_cell(t(cx, "权限", "Permissions"), 100.))
                            .child(table_cell(t(cx, "操作", "Action"), 60.)),
                    )
                    .child(list),
            );
        table.into_any_element()
    }
}

impl Render for FilesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.confirmation_expanded
            && let Some((message, _)) = &self.pending
        {
            return self
                .expanded_confirmation(format!("{} · {}", self.host, message.render(cx)), cx);
        }
        if self.pending.is_none()
            && let Some(surface) = self.editor_surface
        {
            return self.editor_surface(surface, cx);
        }
        let hidden_actions = self.pending.is_some()
            || self.queue_details_visible
            || self.tool_section != Some(action_view::ToolSection::File);
        let hidden_transfer = self.pending.is_some()
            || self.queue_details_visible
            || self.tool_section != Some(action_view::ToolSection::Transfer);
        if [
            (&self.name, hidden_actions),
            (&self.mode, hidden_actions),
            (&self.local, hidden_transfer),
            (&self.editor_find, self.queue_details_visible),
            (&self.editor_replace, self.queue_details_visible),
        ]
        .iter()
        .any(|(input, hidden)| *hidden && input.read(cx).focus_handle(cx).is_focused(window))
            || (self.queue_details_visible
                && (self.editor.read(cx).focus_handle(cx).is_focused(window)
                    || self.patch.read(cx).focus_handle(cx).is_focused(window)
                    || self.merge_review.as_ref().is_some_and(|review| {
                        review.manual.read(cx).focus_handle(cx).is_focused(window)
                    })))
        {
            // Reviews and queue details can unmount retained inputs without a
            // disclosure click. Never leave keyboard events in an unseen draft.
            self.review_focus.focus(window, cx);
        }
        let visual = crate::design::palette(cx);
        let navigation = self.local_browser_view(cx);
        let table = self.remote_browser_view(cx);
        let body = div()
            .id("file-browsing-area")
            .flex_1()
            // Reserve the 28px header, a real 28px entry and scrollbar space.
            // Toolbars and secondary cards must scroll instead of taking this.
            .min_h(px(64.))
            .flex()
            .child(navigation)
            .child(table);
        let mut tools = div()
            .id("file-tools-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.tools_scroll)
            .vertical_scrollbar(&self.tools_scroll)
            .flex()
            .flex_col()
            .test_support();
        let mut editor_card = None;
        if let Some((path, _)) = &self.editing {
            let mut editor_panel = div()
                .id("remote-file-editor")
                .w_full()
                .flex_shrink_0()
                .h(px(if self.diff_preview.is_some() {
                    268.
                } else {
                    180.
                }))
                .min_h_0()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(rgb(visual.border))
                .child(
                    div()
                        .h(px(28.))
                        .flex_shrink_0()
                        .px_2()
                        .flex()
                        .items_center()
                        .bg(rgb(visual.canvas))
                        .overflow_hidden()
                        .child(format!("{} · {path}", t(cx, "编辑", "Edit"))),
                )
                .child(
                    div().flex_1().min_h_0().p_1().child(
                        gpui_kit::component::input::Editor::new(&self.editor)
                            .h(relative(1.))
                            .font_family("monospace"),
                    ),
                )
                .child(
                    div()
                        .p_1()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(rgb(visual.border))
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .child(
                            Button::new("toggle-remote-diff")
                                .disabled(self.suspended)
                                .icon(IconName::FileDiff)
                                .ghost()
                                .compact()
                                .rounded(px(crate::design::components(cx).control_radius))
                                .label(if self.diff_preview.is_some() {
                                    t(cx, "隐藏差异", "Hide diff")
                                } else {
                                    t(cx, "查看差异", "View diff")
                                })
                                .on_click(
                                    cx.listener(|view, _, _, cx| view.toggle_diff_preview(cx)),
                                ),
                        )
                        .child(self.text_actions(cx))
                        .child(
                            Button::new("save-remote-file")
                                .disabled(self.suspended)
                                .icon(IconName::Save)
                                .primary()
                                .compact()
                                .rounded(px(crate::design::components(cx).control_radius))
                                .label(t(cx, "审核并保存", "Review save"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.request_save(cx);
                                })),
                        ),
                );
            if let Some(diff) = &self.diff_preview {
                editor_panel = editor_panel.child(merge::text_scroll(
                    "remote-file-diff-preview",
                    diff,
                    &self.diff_preview_scroll,
                    88.,
                    cx,
                ));
            }
            editor_card = Some(editor_panel.test_support());
        }
        if let Some(card) = self.patch_card(cx) {
            tools = tools.child(card);
        }
        if let Some(card) = self.merge_card(cx) {
            tools = tools.child(card);
        }
        let mut panel = div()
            .track_focus(&self.review_focus)
            .w_full()
            .min_w_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .text_xs()
            .child(self.folder_navigation(cx))
            .when(self.suspended, |panel| {
                panel.child(
                    div()
                        .px_3()
                        .py_1()
                        .flex_shrink_0()
                        .bg(rgb(visual.canvas))
                        .text_color(rgb(visual.muted))
                        .child(t(
                            cx,
                            "上一会话快照 · 草稿可复制，远程操作已停用",
                            "Previous session snapshot · Copy drafts; remote actions are disabled",
                        )),
                )
            })
            .child(self.queue_entry_bar(cx))
            .when(!self.queue_details_visible, |panel| {
                panel.child(body.test_support())
            });
        if self.pending.is_none() {
            tools = match self.tool_section {
                Some(action_view::ToolSection::File) => tools.child(self.mutation_tools(cx)),
                Some(action_view::ToolSection::Transfer) => tools.child(self.transfer_tools(cx)),
                None => tools,
            };
        }
        if let Some(editor) = editor_card {
            if self.pending.is_none() {
                tools = tools.child(self.editor_entry_bar(cx));
            }
            tools = tools.child(editor);
        }
        if let Some(comparison) = &self.comparison {
            let can_apply = comparison
                .sync_plan
                .as_ref()
                .is_some_and(|p| p.operation_count() > 0);
            let disabled = self.suspended || self.busy || self.pending.is_some();
            tools = tools.child(
                div()
                    .mx_2()
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("plan-sync-to-remote")
                            .ghost()
                            .compact()
                            .label(t(cx, "校验本地 → 远端", "Verify local → remote"))
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.busy || view.pending.is_some() {
                                    return;
                                }
                                if let Some(c) = &view.comparison {
                                    view.run(
                                        Operation::PlanDirectorySync(
                                            c.local.clone(),
                                            c.remote.clone(),
                                            DirectorySyncDirection::LeftToRight,
                                            DirectorySyncDeletePolicy::PreserveDestination,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("plan-sync-to-local")
                            .ghost()
                            .compact()
                            .label(t(cx, "校验远端 → 本地", "Verify remote → local"))
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.busy || view.pending.is_some() {
                                    return;
                                }
                                if let Some(c) = &view.comparison {
                                    view.run(
                                        Operation::PlanDirectorySync(
                                            c.local.clone(),
                                            c.remote.clone(),
                                            DirectorySyncDirection::RightToLeft,
                                            DirectorySyncDeletePolicy::PreserveDestination,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("plan-mirror-to-remote")
                            .ghost()
                            .compact()
                            .label(t(cx, "镜像本地 → 远端…", "Mirror local → remote…"))
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.busy || view.pending.is_some() {
                                    return;
                                }
                                if let Some(c) = &view.comparison {
                                    view.run(
                                        Operation::PlanDirectorySync(
                                            c.local.clone(),
                                            c.remote.clone(),
                                            DirectorySyncDirection::LeftToRight,
                                            DirectorySyncDeletePolicy::IncludeDeletes,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("plan-mirror-to-local")
                            .ghost()
                            .compact()
                            .label(t(cx, "镜像远端 → 本地…", "Mirror remote → local…"))
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.busy || view.pending.is_some() {
                                    return;
                                }
                                if let Some(c) = &view.comparison {
                                    view.run(
                                        Operation::PlanDirectorySync(
                                            c.local.clone(),
                                            c.remote.clone(),
                                            DirectorySyncDirection::RightToLeft,
                                            DirectorySyncDeletePolicy::IncludeDeletes,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("review-directory-sync")
                            .primary()
                            .compact()
                            .label(t(cx, "审核同步", "Review sync"))
                            .disabled(disabled || !can_apply)
                            .on_click(cx.listener(|view, _, _, cx| view.request_sync_review(cx))),
                    )
                    .child(
                        Button::new("close-directory-comparison")
                            .ghost()
                            .compact()
                            .icon(IconName::X)
                            .label(t(cx, "收起比较", "Hide comparison"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                if view.busy {
                                    return;
                                }
                                if matches!(
                                    &view.pending,
                                    Some((_, Operation::ApplyDirectorySync(..)))
                                ) {
                                    view.pending = None;
                                }
                                view.comparison = None;
                                cx.notify();
                            })),
                    ),
            );
            tools = tools.child(directory_compare_card(comparison, cx));
        }
        if !self.mirror_conflicts.is_empty() {
            let mut conflicts = div()
                .id("directory-mirror-conflicts")
                .mx_2()
                .p_2()
                .flex_shrink_0()
                .max_h(px(160.))
                .overflow_y_scroll()
                .border_1()
                .border_color(rgb(visual.danger_border))
                .child(t(
                    cx,
                    "镜像计划已拒绝：全部冲突（只读）",
                    "Mirror refused: all conflicts (read-only)",
                ));
            for conflict in &self.mirror_conflicts {
                use keelshell_core::DirectoryMirrorConflictReason as Reason;
                let reason = match conflict.reason {
                    Reason::UnsupportedObject => t(
                        cx,
                        "链接、特殊对象或类型不明",
                        "Link, special or ambiguous object",
                    ),
                    Reason::NonPortablePath => t(cx, "非安全跨平台路径", "Unsafe portable path"),
                    Reason::TypeConflict => t(cx, "文件/目录类型冲突", "Object type conflict"),
                    Reason::CaseConflict => t(cx, "大小写路径冲突", "Case collision"),
                    Reason::IncompleteHierarchy => {
                        t(cx, "父目录证据不完整", "Incomplete ancestor evidence")
                    }
                    Reason::IncompleteContent => t(cx, "内容证据不完整", "Incomplete content"),
                    Reason::BudgetExceeded => t(
                        cx,
                        "深度或内容超出完整镜像预算",
                        "Complete mirror depth/content budget exceeded",
                    ),
                };
                conflicts = conflicts.child(
                    div()
                        .min_w_0()
                        .whitespace_normal()
                        .child(format!("{} · {reason}", conflict.path)),
                );
            }
            tools = tools.child(conflicts.test_support());
        }
        if let Some(journal) = &self.sync_journal
            && let Ok(journal) = journal.lock()
        {
            let mut rows = div()
                .id("directory-sync-results")
                .mx_2()
                .p_2()
                .flex_shrink_0()
                .max_h(px(160.))
                .overflow_y_scroll()
                .border_1()
                .border_color(rgb(visual.border))
                .child(t(
                    cx,
                    "目录同步逐项结果（不自动重试）",
                    "Directory item results (no automatic retry)",
                ));
            for step in &journal.steps {
                use super::sync::journal::StepState;
                let label = match step.state {
                    StepState::NotStarted => t(cx, "尚未开始", "Not started"),
                    StepState::Verifying => t(cx, "正在复核", "Verifying"),
                    StepState::Unknown => t(cx, "结果未知", "Outcome unknown"),
                    StepState::Completed => t(cx, "已确认完成", "Confirmed complete"),
                    StepState::Rejected => t(cx, "拒绝或失败", "Rejected or failed"),
                    StepState::CancelledBeforeWrite => {
                        t(cx, "取消前未写入", "Cancelled before write")
                    }
                    StepState::SkippedAfterFailure => {
                        t(cx, "失败后未开始", "Not started after failure")
                    }
                };
                rows = rows.child(div().min_w_0().whitespace_normal().child(format!(
                    "{} · {label} · {}",
                    if step.delete {
                        t(cx, "删除", "Delete")
                    } else {
                        t(cx, "复制/建目录", "Copy/mkdir")
                    },
                    step.path
                )));
            }
            if journal.cleanup_failed {
                rows = rows.child(t(
                    cx,
                    "操作结果保留；SFTP收尾未确认",
                    "Item results retained; SFTP cleanup unconfirmed",
                ));
            }
            tools = tools.child(rows.test_support());
        }
        if self.transfer.is_some() {
            tools = tools.child(self.transfer_card(cx));
        }
        tools = tools.child(self.parallel_queue_card(cx));
        panel = if self.queue_details_visible {
            panel.child(self.queue_details_view(cx))
        } else {
            panel.when(self.has_tool_content(), |panel| panel.child(tools))
        };
        if let Some((message, _)) = &self.pending {
            panel = panel.child(confirmation_bar(
                cx,
                format!("{} · {}", self.host, message.render(cx)),
                &self.confirmation_scroll,
                Button::new("expand-file-review")
                    .ghost()
                    .compact()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .label(t(cx, "展开审核", "Expand review"))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.expand_confirmation(window, cx)),
                    ),
                self.confirm_button(cx),
                self.cancel_review_button(cx),
            ));
        }
        panel
            .child(
                div()
                    .min_h(px(24.))
                    .px_2()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .bg(rgb(visual.canvas))
                    .child(div().text_color(rgb(visual.muted)).child(if self.busy {
                        IconName::RefreshCw
                    } else {
                        IconName::Info
                    }))
                    .child(
                        div()
                            .max_w(px(220.))
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(format!(
                                "{} {}",
                                t(cx, "当前位置：", "Location:"),
                                self.directory.as_deref().unwrap_or("—")
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(self.status.render(cx)),
                    )
                    .when(self.operation_id.is_some(), |view| {
                        view.child(
                            Button::new("cancel-active-file-operation")
                                .ghost()
                                .compact()
                                .rounded(px(crate::design::components(cx).control_radius))
                                .label(t(cx, "取消操作", "Cancel operation"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.cancel_active(cx);
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}
