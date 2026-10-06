//! Remote browser layout and explicit file-operation controls.
use super::*;
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
        card = card.child(div().text_color(rgb(visual.accent)).child(match crate::i18n::language(cx) {
            keelshell_core::Language::ZhCn => format!("内容已校验 · {direction} · {} 项 · 保留目标独有项", plan.operation_count()),
            keelshell_core::Language::En => format!("Content verified · {direction} · {} operations · preserve destination-only entries", plan.operation_count()),
        }));
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

impl Render for FilesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let mut navigation = div()
            .id("remote-directory-tree")
            .w(px(152.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .bg(rgb(visual.canvas))
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(t(cx, "目录", "Directories")),
            );
        let current = self.directory.as_deref().unwrap_or("/");
        let mut ancestors = vec![("/".to_owned(), "/".to_owned())];
        let mut path = String::new();
        for component in current.split('/').filter(|part| !part.is_empty()) {
            path.push('/');
            path.push_str(component);
            ancestors.push((component.to_owned(), path.clone()));
        }
        for (index, (name, path)) in ancestors.into_iter().enumerate() {
            let active = self.directory.as_ref() == Some(&path);
            navigation = navigation.child(
                div()
                    .id(("directory-ancestor", index))
                    .h(px(28.))
                    .pl(px(7. + index.min(5) as f32 * 9.))
                    .pr_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .overflow_hidden()
                    .bg(rgb(if active {
                        visual.selected
                    } else {
                        visual.surface
                    }))
                    .gap_2()
                    .child(IconName::FolderOpen)
                    .child(name)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.run(Operation::List(path.clone()), window, cx)
                    })),
            );
        }
        for (index, entry) in self
            .entries
            .iter()
            .filter(|entry| entry.is_directory)
            .enumerate()
        {
            let path = entry.path.clone();
            navigation = navigation.child(
                div()
                    .id(("directory-child", index))
                    .h(px(28.))
                    .pl_5()
                    .pr_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .overflow_hidden()
                    .gap_2()
                    .child(IconName::Folder)
                    .child(entry.name.clone())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.run(Operation::List(path.clone()), window, cx)
                    })),
            );
        }
        let mut list = div()
            .id("remote-files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .test_support();
        for (index, entry) in self.entries.iter().enumerate() {
            let selected = self
                .selected
                .as_ref()
                .is_some_and(|item| item.path == entry.path);
            let item = entry.clone();
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
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.selected = Some(item.clone());
                        if let Some(mode) = item.permissions {
                            view.mode.update(cx, |input, cx| {
                                input.set_value(format!("{:04o}", mode & 0o7777), window, cx);
                            });
                        }
                        cx.notify();
                    }))
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
                                .disabled(self.suspended)
                                .ghost()
                                .compact()
                                .rounded(px(6.))
                                .label(if entry.is_directory {
                                    t(cx, "打开", "Open")
                                } else {
                                    t(cx, "编辑", "Edit")
                                })
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if open.is_directory {
                                        view.run(Operation::List(open.path.clone()), window, cx);
                                    } else {
                                        view.request_read(open.clone(), window, cx);
                                    }
                                })),
                        ),
                    )
                    .test_support(),
            );
        }
        if self.entries.is_empty() && !self.busy {
            list = list.child(div().p_3().text_color(rgb(visual.muted)).child(t(
                cx,
                "当前目录没有文件",
                "This directory is empty",
            )));
        }
        let table = div()
            .id("remote-files-table")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
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
                            .child(div().flex_1().min_w(px(150.)).px_2().child(t(
                                cx,
                                "文件名",
                                "Name",
                            )))
                            .child(table_cell(t(cx, "大小", "Size"), 88.))
                            .child(table_cell(t(cx, "类型", "Type"), 76.))
                            .child(table_cell(t(cx, "修改时间 (UTC)", "Modified (UTC)"), 143.))
                            .child(table_cell(t(cx, "权限", "Permissions"), 100.))
                            .child(table_cell(t(cx, "操作", "Action"), 60.)),
                    )
                    .child(list),
            );
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
                .h(px(if self.diff_preview.is_some() { 268. } else { 180. }))
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
                    div()
                        .flex_1()
                        .min_h_0()
                        .p_1()
                        .child(Textarea::new(&self.editor).h(relative(1.)).font_family("monospace")),
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
                                .rounded(px(6.))
                                .label(if self.diff_preview.is_some() {
                                    t(cx, "隐藏差异", "Hide diff")
                                } else {
                                    t(cx, "查看差异", "View diff")
                                })
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.toggle_diff_preview(cx)
                                })),
                        )
                        .child(
                            Button::new("save-remote-file")
                                .disabled(self.suspended)
                                .icon(IconName::Save)
                                .primary()
                                .compact()
                                .rounded(px(6.))
                                .label(t(cx, "审核并保存", "Review save"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    if let Some((path, original)) = &view.editing {
                                        let content = view.editor.read(cx).value().as_bytes().to_vec();
                                        if content.len() > 1024 * 1024 {
                                            view.status = Message::new(
                                                "编辑内容超过 1 MiB，请使用外部编辑器后上传",
                                                "Content exceeds 1 MiB; use an external editor and upload instead",
                                            );
                                            cx.notify();
                                            return;
                                        }
                                        view.confirm(
                                            Message::new(
                                                format!("将 {} 替换为审核后的 {} 字节内容？", path, content.len()),
                                                format!("Replace {} with the reviewed {} bytes?", path, content.len()),
                                            ),
                                            Operation::Save {
                                                path: path.clone(),
                                                original: original.clone(),
                                                content,
                                            },
                                            cx,
                                        );
                                    }
                                })),
                        ),
                );
            if let Some(diff) = &self.diff_preview {
                editor_panel = editor_panel.child(
                    div()
                        .h(px(88.))
                        .flex_shrink_0()
                        .min_h_0()
                        .border_t_1()
                        .border_color(rgb(visual.border))
                        .bg(rgb(visual.canvas))
                        .overflow_y_scrollbar()
                        .p_2()
                        .text_color(rgb(visual.text))
                        .font_family("monospace")
                        .child(diff.clone()),
                );
            }
            editor_card = Some(editor_panel.test_support());
        }
        let selection = self
            .selected
            .as_ref()
            .map(|entry| format!("{} {}", t(cx, "已选：", "Selected:"), entry.name))
            .unwrap_or_else(|| t(cx, "未选择文件", "No file selected").to_owned());
        let has_selection = self.selected.is_some();
        let downloadable = self
            .selected
            .as_ref()
            .is_some_and(|entry| !entry.is_symlink);
        let mut panel = div()
            .w_full()
            .min_w_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .text_xs()
            .child(
                div()
                    .h(px(38.))
                    .px_3()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(
                        div()
                            .text_color(rgb(visual.accent))
                            .child(IconName::FolderOpen),
                    )
                    .child(div().text_color(rgb(visual.muted)).child(t(
                        cx,
                        "远程目录",
                        "Remote path",
                    )))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.path).small().rounded(px(6.))),
                    )
                    .child(
                        Button::new("parent-files")
                            .disabled(self.suspended)
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .icon(IconName::ArrowUp)
                            .label(t(cx, "上级", "Up"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                if let Some(directory) = &view.directory {
                                    view.run(
                                        Operation::List(format!(
                                            "{}/..",
                                            directory.trim_end_matches('/')
                                        )),
                                        window,
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new("refresh-files")
                            .disabled(self.suspended)
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .icon(IconName::RefreshCw)
                            .label(t(cx, "刷新", "Refresh"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.run(
                                    Operation::List(view.path.read(cx).value().to_string()),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        div()
                            .max_w(px(160.))
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(self.host.clone()),
                    ),
            )
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
            .child(body.test_support());
        tools = tools.when(self.pending.is_none(), |panel| panel
            .child(div().id("file-mutation-tools").min_h(px(38.)).px_3().py_1().flex_shrink_0().flex().flex_wrap().items_center().gap_2().bg(rgb(visual.canvas)).border_t_1().border_color(rgb(visual.border))
                .child(div().w(px(150.)).flex_shrink_0().text_ellipsis().text_color(rgb(if has_selection {visual.text} else {visual.muted})).child(selection.to_owned()))
                .child(div().id("file-name-draft").w(px(180.)).flex_shrink_0().child(Input::new(&self.name).small().rounded(px(6.))).test_support())
                .child(Button::new("mkdir").flex_shrink_0().disabled(self.suspended).ghost().compact().rounded(px(6.)).icon(IconName::FolderPlus).label(t(cx,"新建目录","New folder")).on_click(cx.listener(|view,_,_,cx| {
                    if let Some(path) = view.new_remote_path(cx) { view.confirm(Message::new(format!("创建目录 {path}？"),format!("Create directory {path}?")),Operation::Mkdir(path),cx); } cx.notify();
                })))
                .child(div().h(px(16.)).w(px(1.)).bg(rgb(visual.border)))
                .child(Button::new("rename").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Pencil).label(t(cx,"重命名","Rename")).disabled(self.suspended || !has_selection).on_click(cx.listener(|view,_,_,cx| {
                    if let (Some(entry),Some(path)) = (view.selected.clone(),view.new_remote_path(cx)) { view.confirm(Message::new(format!("将 {} 重命名为 {path}？",entry.path),format!("Rename {} to {path}?",entry.path)),Operation::Rename(entry.path,path),cx); } cx.notify();
                })))
                .child(Button::new("delete-file").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Trash).label(t(cx,"删除","Delete")).disabled(self.suspended || !has_selection).text_color(rgb(if self.suspended { visual.muted } else { visual.danger })).on_click(cx.listener(|view,_,_,cx| {
                    if let Some(entry) = view.selected.clone() { view.confirm(Message::new(format!("永久删除 {}？",entry.path),format!("Delete {} permanently?",entry.path)),Operation::Delete(entry),cx); }
                })))
                .child(div().h(px(16.)).w(px(1.)).bg(rgb(visual.border)))
                .child(div().id("file-mode-draft").w(px(118.)).flex_shrink_0().child(Input::new(&self.mode).small().rounded(px(6.))).test_support())
                .child(Button::new("chmod-file").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Lock).label(t(cx,"改权限","Permissions")).disabled(self.suspended || self.busy || !has_selection).on_click(cx.listener(|view,_,_,cx| {
                    let Some(entry) = view.selected.clone() else { return; };
                    if entry.is_symlink {
                        view.status = Message::new("为避免跟随链接误改目标，符号链接不支持修改权限", "Permission changes on symbolic links are disabled to avoid following a link");
                    } else if entry.permissions.is_none() {
                        view.status = Message::new("服务器未返回可审核的权限位，请刷新后重试", "The server did not return reviewable mode bits; refresh and try again");
                    } else {
                        match parse_permissions_mode(&view.mode.read(cx).value()) {
                            Ok(mode) => view.confirm(Message::new(format!("将 {} 的 POSIX 权限改为 {:04o}？仅修改权限位，不修改内容、所有者或时间。", entry.path, mode), format!("Change {} POSIX permissions to {:04o}? Only mode bits change; content, ownership and timestamps stay untouched.", entry.path, mode)), Operation::SetPermissions(entry, mode), cx),
                            Err(detail) => view.status = Message::new(format!("权限格式无效：{detail}"), format!("Invalid permissions: {detail}")),
                        }
                    }
                    cx.notify();
                }))))
            .child(div().id("file-transfer-tools").min_h(px(38.)).px_3().py_1().flex_shrink_0().flex().flex_wrap().items_center().gap_2().border_t_1().border_color(rgb(visual.border))
                .child(Button::new("file-resume-mode").flex_shrink_0().ghost().compact().disabled(self.suspended || self.operation_id.is_some() || self.pending.is_some())
                    .when(self.resume_mode, |button| button.primary()).label(t(cx,"续传模式","Resume mode"))
                    .on_click(cx.listener(|view,_,_,cx| {
                        view.resume_mode = !view.resume_mode;
                        view.status = if view.resume_mode { Message::new("续传上传：选择远端已有部分文件/目录；续传下载：填写已有本地目标。先校验，再确认追加。", "Resume upload: select the existing remote file/folder. Resume download: enter the existing local destination. Verify first, then confirm append.") } else { Message::new("普通上传可替换同名文件；普通下载仅新建本地目标。", "Normal uploads may replace files; normal downloads create new local destinations only.") };
                        cx.notify();
                    })))
                .child(div().flex_1().min_w(px(150.)).child(Input::new(&self.local).small().rounded(px(6.))))
                .child(Button::new("upload-file").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Upload).label(if self.resume_mode {t(cx,"续传文件","Resume file")} else {t(cx,"上传文件","Upload file")}).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
                    let local = PathBuf::from(view.local.read(cx).value().to_string());
                    if view.resume_mode {
                        if let Some(entry) = &view.selected && !entry.is_directory && !entry.is_symlink {
                            view.run(Operation::PlanResume(TransferSpec::upload(local,entry.path.clone()),false),window,cx);
                        } else { view.status = Message::new("请先选择远端已有部分文件", "Select the existing remote partial file first"); cx.notify(); }
                        return;
                    }
                    if let Some(name) = local.file_name().and_then(|name|name.to_str()) {
                        let Some(remote) = view.remote_child(name) else {cx.notify(); return;};
                        view.confirm(Message::new(format!("通过传输队列上传到 {remote}？同名远端文件可能被替换，取消时可能保留部分文件。"),format!("Upload to {remote} through the transfer queue? An existing remote file may be replaced, and cancellation may leave a partial file.")),Operation::Upload(local,remote),cx);
                    } else { view.status=Message::new("请输入有效的本地文件路径", "Enter a valid local file path");cx.notify(); }
                })))
                .child(Button::new("upload-directory").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Folder).label(if self.resume_mode {t(cx,"续传目录","Resume folder")} else {t(cx,"上传目录","Upload folder")}).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
                    let local = PathBuf::from(view.local.read(cx).value().to_string());
                    if view.resume_mode {
                        if let Some(entry) = &view.selected && entry.is_directory && !entry.is_symlink {
                            view.run(Operation::PlanResume(TransferSpec::upload(local,entry.path.clone()),true),window,cx);
                        } else { view.status = Message::new("请先选择远端已有部分目录", "Select the existing remote partial folder first"); cx.notify(); }
                        return;
                    }
                    if let Some(name) = local.file_name().and_then(|name|name.to_str()) {
                        let Some(remote) = view.remote_child(name) else {cx.notify(); return;};
                        view.run(Operation::PlanDirectory(TransferSpec::upload(local, remote)), window, cx);
                    } else { view.status=Message::new("请输入本地目录的绝对路径", "Enter the absolute local directory path");cx.notify(); }
                })))
                .child(Button::new("download-file").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::Download).label(if self.resume_mode {t(cx,"续传选中项","Resume selected")} else {t(cx,"下载选中项","Download selected")}).disabled(self.suspended || !downloadable || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
                    if let Some(entry) = &view.selected && !entry.is_symlink {
                        let local = PathBuf::from(view.local.read(cx).value().to_string());
                        if view.resume_mode {
                            view.run(Operation::PlanResume(TransferSpec::download(entry.path.clone(),local),entry.is_directory),window,cx);
                            return;
                        }
                        if entry.is_directory {
                            view.run(Operation::PlanDirectory(TransferSpec::download(entry.path.clone(), local)), window, cx);
                            return;
                        }
                        view.confirm(Message::new(format!("将 {} 下载到 {}？不覆盖已有文件；中断时可能保留未完成的下载。",entry.path,local.display()),format!("Download {} to {}? Existing files are preserved; an interruption may leave a partial download.",entry.path,local.display())),Operation::Download(entry.path.clone(),local),cx);
                    }
                })))
                .child(Button::new("compare-directories").flex_shrink_0().ghost().compact().rounded(px(6.)).icon(IconName::FileDiff).label(t(cx, "比较目录", "Compare folders")).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
                    let local = PathBuf::from(view.local.read(cx).value().trim());
                    // The path input is a navigation draft. Use the last
                    // successfully loaded canonical directory so a typed but
                    // unsubmitted path cannot change the comparison target.
                    let remote = view.directory.clone();
                    if !local.is_absolute() {
                        view.status = Message::new("请输入本地目录的绝对路径", "Enter an absolute local directory path");
                    } else if remote.as_deref().is_none_or(|path| path.is_empty() || path.chars().any(char::is_control)) {
                        view.status = Message::new("请输入有效的远程目录", "Enter a valid remote directory");
                    } else if let Some(remote) = remote {
                        view.run(Operation::Compare(local, remote), window, cx);
                    }
                    cx.notify();
                })))));
        if let Some(editor) = editor_card {
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
                                    Some((_, Operation::ApplyDirectorySync(_)))
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
        if self.transfer.is_some() {
            tools = tools.child(self.transfer_card(cx));
        }
        tools = tools.child(self.parallel_queue_card(cx));
        panel = panel.child(tools);
        if let Some((message, _)) = &self.pending {
            panel = panel.child(confirmation_bar(
                cx,
                format!("{} · {}", self.host, message.render(cx)),
                Button::new("confirm-file-operation")
                    .primary()
                    .compact()
                    .rounded(px(6.))
                    .label(t(cx, "确认", "Confirm"))
                    .on_click(cx.listener(|view, _, window, cx| view.execute_pending(window, cx))),
                Button::new("cancel-file-operation")
                    .ghost()
                    .compact()
                    .rounded(px(6.))
                    .label(t(cx, "取消", "Cancel"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.pending = None;
                        cx.notify();
                    })),
            ));
        }
        panel.child(
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
                            .rounded(px(6.))
                            .label(t(cx, "取消操作", "Cancel operation"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.cancel_active(cx);
                            })),
                    )
                }),
        )
    }
}
