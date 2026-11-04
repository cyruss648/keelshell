//! Presentation-only file settings; all mutations retain their reviewed handlers.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolSection {
    File,
    Transfer,
}

impl ToolSection {
    fn index(self) -> usize {
        match self {
            Self::File => 0,
            Self::Transfer => 1,
        }
    }
}

impl FilesPanel {
    pub(super) fn tool_section_button(
        &self,
        section: ToolSection,
        cx: &mut Context<Self>,
    ) -> Button {
        let (id, label) = match section {
            ToolSection::File => ("toggle-file-actions", t(cx, "文件操作", "File actions")),
            ToolSection::Transfer => (
                "toggle-file-transfer-settings",
                t(cx, "传输设置", "Transfer"),
            ),
        };
        let expected_session = self.session_token;
        let selected = self.tool_section == Some(section);
        Button::new(id)
            .flex_shrink_0()
            .ghost()
            .compact()
            .small()
            .rounded(px(crate::design::components(cx).control_radius))
            .icon(if selected {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .label(label)
            .accessibility_label(label)
            .selected(selected)
            .toggled(selected)
            .track_focus(&self.tool_section_focus[section.index()])
            .disabled(self.pending.is_some() || self.queue_details_visible)
            .on_click(cx.listener(move |view, _, window, cx| {
                if view.session_token != expected_session
                    || view.pending.is_some()
                    || view.queue_details_visible
                {
                    return;
                }
                view.tool_section = if view.tool_section == Some(section) {
                    None
                } else {
                    Some(section)
                };
                // Retained input entities must not receive typing after their
                // settings are unmounted. The visible trigger owns focus.
                view.tool_section_focus[section.index()].focus(window, cx);
                view.tools_scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }))
    }

    pub(super) fn has_tool_content(&self) -> bool {
        (self.tool_section.is_some() && self.pending.is_none())
            || self.editing.is_some()
            || self.comparison.is_some()
            || !self.mirror_conflicts.is_empty()
            || self.sync_journal.is_some()
            || self.transfer.is_some()
            || !self.transfer_jobs.is_empty()
    }

    pub(super) fn mutation_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let has_selection = self.selected.is_some();
        let selection = self
            .selected
            .as_ref()
            .map(|entry| format!("{} {}", t(cx, "已选：", "Selected:"), entry.name))
            .unwrap_or_else(|| t(cx, "未选择文件", "No file selected").to_owned());
        div().id("file-mutation-tools").min_h(px(38.)).px_3().py_1().flex_shrink_0().flex().flex_wrap().items_center().gap_2().bg(rgb(visual.canvas)).border_t_1().border_color(rgb(visual.border))
                .child(div().w(px(150.)).flex_shrink_0().text_ellipsis().text_color(rgb(if has_selection {visual.text} else {visual.muted})).child(selection.to_owned()))
                .child(div().id("file-name-draft").w(px(180.)).flex_shrink_0().child(Input::new(&self.name).small().rounded(px(crate::design::components(cx).control_radius))).test_support())
                .child(Button::new("mkdir").flex_shrink_0().disabled(self.suspended).ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::FolderPlus).label(t(cx,"新建目录","New folder")).on_click(cx.listener(|view,_,_,cx| {
                    if let Some(path) = view.new_remote_path(cx) { view.confirm(Message::new(format!("创建目录 {path}？"),format!("Create directory {path}?")),Operation::Mkdir(path),cx); } cx.notify();
                })))
                .child(div().h(px(16.)).w(px(1.)).bg(rgb(visual.border)))
                .child(Button::new("rename").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Pencil).label(t(cx,"重命名","Rename")).disabled(self.suspended || !has_selection).on_click(cx.listener(|view,_,_,cx| {
                    if let (Some(entry),Some(path)) = (view.selected.clone(),view.new_remote_path(cx)) { view.confirm(Message::new(format!("将 {} 重命名为 {path}？",entry.path),format!("Rename {} to {path}?",entry.path)),Operation::Rename(entry.path,path),cx); } cx.notify();
                })))
                .child(Button::new("delete-file").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Trash).label(t(cx,"删除","Delete")).disabled(self.suspended || !has_selection).text_color(rgb(if self.suspended { visual.muted } else { visual.danger })).on_click(cx.listener(|view,_,_,cx| {
                    if let Some(entry) = view.selected.clone() { view.confirm(Message::new(format!("永久删除 {}？",entry.path),format!("Delete {} permanently?",entry.path)),Operation::Delete(entry),cx); }
                })))
                .child(div().h(px(16.)).w(px(1.)).bg(rgb(visual.border)))
                .child(div().id("file-mode-draft").w(px(118.)).flex_shrink_0().child(Input::new(&self.mode).small().rounded(px(crate::design::components(cx).control_radius))).test_support())
                .child(Button::new("chmod-file").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Lock).label(t(cx,"改权限","Permissions")).disabled(self.suspended || self.busy || !has_selection).on_click(cx.listener(|view,_,_,cx| {
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
                }))).test_support().into_any_element()
    }

    pub(super) fn transfer_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let downloadable = self
            .selected
            .as_ref()
            .is_some_and(|entry| !entry.is_symlink);
        div().id("file-transfer-tools").min_h(px(38.)).px_3().py_1().flex_shrink_0().flex().flex_wrap().items_center().gap_2().border_t_1().border_color(rgb(visual.border))
                .child(Button::new("file-resume-mode").flex_shrink_0().ghost().compact().disabled(self.suspended || self.operation_id.is_some() || self.pending.is_some())
                    .when(self.resume_mode, |button| button.primary()).label(t(cx,"续传模式","Resume mode"))
                    .on_click(cx.listener(|view,_,_,cx| {
                        view.resume_mode = !view.resume_mode;
                        view.status = if view.resume_mode { Message::new("续传上传：选择远端已有部分文件/目录；续传下载：填写已有本地目标。先校验，再确认追加。", "Resume upload: select the existing remote file/folder. Resume download: enter the existing local destination. Verify first, then confirm append.") } else { Message::new("普通上传可替换同名文件；普通下载仅新建本地目标。", "Normal uploads may replace files; normal downloads create new local destinations only.") };
                        cx.notify();
                    })))
                .child(div().flex_1().min_w(px(150.)).child(Input::new(&self.local).small().rounded(px(crate::design::components(cx).control_radius))))
                .child(Button::new("use-local-selection").ghost().compact().label(t(cx,"使用本地所选","Use local selection"))
                    .disabled(self.suspended || self.local_browser.selected_entry().is_none_or(|entry| !matches!(entry.kind, local_catalog::LocalEntryKind::Directory | local_catalog::LocalEntryKind::File) || entry.path.to_str().is_none()))
                    .on_click(cx.listener(|view,_,window,cx| view.use_local_selection(window,cx))))
                .child(Button::new("use-local-folder").ghost().compact().label(t(cx,"使用本地目录","Use local folder"))
                    .disabled(self.suspended || self.local_browser.listing.as_ref().is_none_or(|listing| listing.directory.to_str().is_none()))
                    .on_click(cx.listener(|view,_,window,cx| view.use_local_folder(window,cx))))
                .child(Button::new("download-into-local-folder").ghost().compact().label(t(cx,"选作下载目录","Use as download folder"))
                    .disabled(self.suspended || self.selected.is_none() || self.local_browser.listing.as_ref().is_none_or(|listing| listing.directory.to_str().is_none()))
                    .on_click(cx.listener(|view,_,window,cx| view.prepare_local_destination(window,cx))))
                .child(Button::new("upload-file").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Upload).label(if self.resume_mode {t(cx,"续传文件","Resume file")} else {t(cx,"上传文件","Upload file")}).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
                    let local = PathBuf::from(view.local.read(cx).value().to_string());
                    if view.resume_mode {
                        if let Some(entry) = &view.selected && !entry.is_directory && !entry.is_symlink {
                            view.run(Operation::PlanResume(TransferSpec::upload(local,entry.path.clone()),false),window,cx);
                        } else { view.status = Message::new("请先选择远端已有部分文件", "Select the existing remote partial file first"); cx.notify(); }
                        return;
                    }
                    if let Some(name) = local.file_name().and_then(|name|name.to_str()) {
                        let Some(remote) = view.remote_child(name) else {cx.notify(); return;};
                        view.confirm(Message::new(format!("通过传输队列将 {} 上传到 {remote}？同名远端文件可能被替换，取消时可能保留部分文件。", local.display()),format!("Upload {} to {remote} through the transfer queue? An existing remote file may be replaced, and cancellation may leave a partial file.", local.display())),Operation::Upload(local,remote),cx);
                    } else { view.status=Message::new("请输入有效的本地文件路径", "Enter a valid local file path");cx.notify(); }
                })))
                .child(Button::new("upload-directory").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Folder).label(if self.resume_mode {t(cx,"续传目录","Resume folder")} else {t(cx,"上传目录","Upload folder")}).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
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
                .child(Button::new("download-file").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::Download).label(if self.resume_mode {t(cx,"续传选中项","Resume selected")} else {t(cx,"下载选中项","Download selected")}).disabled(self.suspended || !downloadable || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
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
                .child(Button::new("compare-directories").flex_shrink_0().ghost().compact().rounded(px(crate::design::components(cx).control_radius)).icon(IconName::FileDiff).label(t(cx, "比较目录", "Compare folders")).disabled(self.suspended || self.operation_id.is_some()).on_click(cx.listener(|view,_,window,cx| {
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
                }))).test_support().into_any_element()
    }
}
