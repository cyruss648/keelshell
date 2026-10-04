//! Bounded library selection toolbar and review modal.
use super::*;
use gpui_kit::component::Selectable;

impl Workspace {
    pub(in crate::workspace) fn library_batch_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let trash = self.library_filter == LibraryFilter::Trash;
        let disabled = self.saving || self.library_selection.is_empty();
        let mut bar = div()
            .id("library-bulk-toolbar")
            .test_support()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(div().text_xs().child(format!(
                "{}: {}",
                t(cx, "已选", "Selected"),
                self.library_selection.len()
            )))
            .child(
                Button::new("library-clear-selection")
                    .ghost()
                    .compact()
                    .disabled(disabled)
                    .label(t(cx, "清除选区", "Clear selection"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.library_selection.clear();
                        cx.notify();
                    })),
            );
        let actions = if trash {
            vec![
                ("library-bulk-restore", ConnectionLibraryAction::Restore),
                ("library-bulk-purge", ConnectionLibraryAction::Purge),
            ]
        } else {
            vec![
                ("library-bulk-move", ConnectionLibraryAction::Move(None)),
                (
                    "library-bulk-tags",
                    ConnectionLibraryAction::AddTags(Vec::new()),
                ),
                (
                    "library-bulk-favorite",
                    ConnectionLibraryAction::Favorite(true),
                ),
                (
                    "library-bulk-unfavorite",
                    ConnectionLibraryAction::Favorite(false),
                ),
                (
                    "library-bulk-trash",
                    ConnectionLibraryAction::Trash(library::now_seconds()),
                ),
            ]
        };
        if self.library_selection.is_empty() {
            bar = bar.child(
                div()
                    .text_xs()
                    .text_color(rgb(crate::design::palette(cx).muted))
                    .child(t(
                        cx,
                        "勾选连接后批量组织",
                        "Select profiles to organize them together",
                    )),
            );
        } else {
            for (id, action) in actions {
                bar = bar.child(
                    Button::new(id)
                        .ghost()
                        .compact()
                        .disabled(disabled)
                        .label(action_title(&action, cx))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.open_selected_library_batch(action.clone(), window, cx)
                        })),
                );
            }
        }
        if trash {
            bar = bar.child(
                Button::new("library-empty-trash")
                    .ghost()
                    .compact()
                    .disabled(self.saving || self.state.deleted_connections.is_empty())
                    .label(t(cx, "清空回收站…", "Empty trash…"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        let ids = view
                            .state
                            .deleted_connections
                            .iter()
                            .map(|entry| entry.connection.id)
                            .collect();
                        view.open_library_batch(
                            ids,
                            ConnectionLibraryAction::Purge,
                            false,
                            window,
                            cx,
                        );
                    })),
            );
        }
        if !self.library_trash_undo.is_empty() {
            bar = bar.child(
                Button::new("library-undo-trash")
                    .ghost()
                    .compact()
                    .disabled(self.saving)
                    .label(t(cx, "撤销上次批量删除…", "Undo last batch trash…"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_library_batch(
                            view.library_trash_undo.clone(),
                            ConnectionLibraryAction::Restore,
                            false,
                            window,
                            cx,
                        )
                    })),
            );
        }
        bar.into_any_element()
    }

    pub(in crate::workspace) fn library_batch_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(prompt) = &self.library_batch_prompt else {
            return div().into_any_element();
        };
        let visual = crate::design::palette(cx);
        let mut body = div().flex().flex_col().gap_3().min_w_0();
        body = body.child(self.library_batch_editor(prompt, cx));
        let action = prompt
            .review
            .as_ref()
            .map(|review| &review.action)
            .unwrap_or(&prompt.action);
        body=body.child(div().text_sm().text_color(rgb(visual.muted)).child(t(cx,"仅修改本机连接配置。现有 SSH 会话保持打开；已移入回收站的路线停止后续自动重连。加密凭据与服务器信任记录保留。","Only local profile metadata changes. Existing SSH sessions stay open; trashed routes stop future automatic reconnection. Encrypted credentials and host trust records are retained.")));
        if matches!(action, ConnectionLibraryAction::Purge) {
            body=body.child(div().text_sm().text_color(rgb(visual.danger)).child(t(cx,"永久删除无法撤销。以下连接、标签、收藏和文件夹关联将清除；不会删除服务器文件或擦除凭据库条目。","Permanent deletion cannot be undone. The profiles, tags, favorites and folder memberships below will be removed; server files and vault entries are retained.")));
        }
        let source = prompt
            .review
            .as_ref()
            .map(|review| &review.source)
            .unwrap_or(&self.state);
        for (index, id) in prompt.ids.iter().copied().enumerate() {
            body = body.child(self.library_batch_target(prompt, source, index, id, cx));
        }
        body = body.when_some(prompt.message.clone(), |body, message| {
            body.child(
                div()
                    .id("library-bulk-error")
                    .test_support()
                    .text_sm()
                    .text_color(rgb(visual.danger))
                    .child(message.render(cx)),
            )
        });
        let footer = self.library_batch_footer(prompt, action, cx);
        div()
            .track_focus(&self.overlay_focus)
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(modals::framed_modal(
                cx,
                "library-bulk-dialog",
                px(680.),
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(action_title(action, cx))
                    .into_any_element(),
                body.into_any_element(),
                footer,
            ))
            .into_any_element()
    }
    fn library_batch_editor(
        &self,
        prompt: &LibraryBatchPrompt,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div().flex().flex_col().gap_2();
        if prompt.review.is_none() {
            match &prompt.action {
                ConnectionLibraryAction::Move(folder) => {
                    let mut choices = div().flex().flex_col().gap_1().child(
                        Button::new("library-bulk-folder-root")
                            .ghost()
                            .selected(folder.is_none())
                            .label(t(cx, "未归档 / 根目录", "Unfiled / root"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let Some(prompt) = &mut view.library_batch_prompt {
                                    prompt.action = ConnectionLibraryAction::Move(None);
                                }
                                cx.notify();
                            })),
                    );
                    for (index, row) in self.state.folder_rows().iter().enumerate() {
                        let id = row.id;
                        choices = choices.child(
                            Button::new(("library-bulk-folder", index))
                                .ghost()
                                .selected(*folder == Some(id))
                                .label(self.folder_label(Some(id), cx))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(prompt) = &mut view.library_batch_prompt {
                                        prompt.action = ConnectionLibraryAction::Move(Some(id));
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    body = body.child(choices);
                }
                ConnectionLibraryAction::AddTags(_)
                | ConnectionLibraryAction::RemoveTags(_)
                | ConnectionLibraryAction::ReplaceTags(_) => {
                    let mut modes = div().flex().flex_wrap().gap_1();
                    for (id, action) in [
                        (
                            "library-tags-add",
                            ConnectionLibraryAction::AddTags(Vec::new()),
                        ),
                        (
                            "library-tags-remove",
                            ConnectionLibraryAction::RemoveTags(Vec::new()),
                        ),
                        (
                            "library-tags-replace",
                            ConnectionLibraryAction::ReplaceTags(Vec::new()),
                        ),
                    ] {
                        modes = modes.child(
                            Button::new(id)
                                .ghost()
                                .selected(
                                    std::mem::discriminant(&prompt.action)
                                        == std::mem::discriminant(&action),
                                )
                                .label(action_title(&action, cx))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(prompt) = &mut view.library_batch_prompt {
                                        prompt.action = action.clone();
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    body=body.child(modes).child(Input::new(&prompt.tags).id("library-bulk-tags-input").aria_label(t(cx,"标签，用逗号分隔","Tags, separated by commas"))).child(div().text_xs().child(t(cx,"替换全部标签会覆盖原标签；留空可清空。添加/移除保留其他标签。","Replace all tags overwrites existing tags; empty clears them. Add/remove retains other tags.")));
                }
                _ => {}
            }
        }
        body.into_any_element()
    }

    fn library_batch_target(
        &self,
        prompt: &LibraryBatchPrompt,
        source: &AppState,
        index: usize,
        id: uuid::Uuid,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(item) = profile(source, id) else {
            return div().into_any_element();
        };
        let mut row = div()
            .id(("library-bulk-target", index))
            .test_support()
            .role(Role::Group)
            .aria_label(format!(
                "{} · {}",
                item.name,
                crate::jump_host_picker::endpoint(item)
            ))
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(rgb(visual.border))
            .flex()
            .flex_col()
            .gap_1()
            .min_w_0()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "{} · {}",
                        item.name,
                        crate::jump_host_picker::endpoint(item)
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .child(id.to_string()),
            );
        let folder = source
            .folder_id_of(id)
            .and_then(|folder| source.folder_path(folder))
            .unwrap_or_else(|| t(cx, "未归档", "Unfiled").into());
        row = row.child(div().text_xs().child(format!(
            "{}: {folder} · {}: {} · {}: {}",
            t(cx, "文件夹", "Folder"),
            t(cx, "标签", "Tags"),
            item.tags.join(", "),
            t(cx, "收藏", "Favorite"),
            if item.favorite {
                t(cx, "是", "Yes")
            } else {
                t(cx, "否", "No")
            }
        )));
        if let Some(review) = &prompt.review {
            if let Some(next) = profile(&review.candidate, id) {
                let folder = review
                    .candidate
                    .folder_id_of(id)
                    .and_then(|folder| review.candidate.folder_path(folder))
                    .unwrap_or_else(|| t(cx, "未归档", "Unfiled").into());
                row = row.child(
                    div()
                        .text_xs()
                        .text_color(rgb(visual.accent))
                        .child(format!(
                            "{} → {folder} · {} → {} · {} → {}",
                            t(cx, "文件夹", "Folder"),
                            t(cx, "标签", "Tags"),
                            next.tags.join(", "),
                            t(cx, "收藏", "Favorite"),
                            if next.favorite {
                                t(cx, "是", "Yes")
                            } else {
                                t(cx, "否", "No")
                            }
                        )),
                );
            }
            row = row.child(div().text_xs().child(format!(
                "{}: {}",
                t(cx, "关联的活动会话", "Related active sessions"),
                review.sessions.get(index).map_or(0, Vec::len)
            )));
            for tab in review.sessions.get(index).into_iter().flatten() {
                if let Some((name, route)) = self.batch_route_description(*tab) {
                    row = row.child(div().text_xs().child(format!("{name} · {route}")));
                }
            }
        }
        if let Some(jump) = item.jump_host.and_then(|jump| profile(source, jump)) {
            row = row.child(div().text_xs().child(format!(
                "{}: {} · {}",
                t(cx, "跳板", "Jump host"),
                jump.name,
                crate::jump_host_picker::endpoint(jump)
            )));
        }
        row = row.child(
            div()
                .text_xs()
                .child(if let Some(reference) = item.credential_ref {
                    format!(
                        "{}: {reference} · {}",
                        t(cx, "凭据引用", "Credential reference"),
                        t(cx, "加密条目保留", "Encrypted entry retained")
                    )
                } else {
                    t(cx, "无已存凭据引用", "No saved credential reference").into()
                }),
        );
        row.into_any_element()
    }

    fn library_batch_footer(
        &self,
        prompt: &LibraryBatchPrompt,
        action: &ConnectionLibraryAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut buttons = div().flex().flex_wrap().justify_end().gap_2().child(
            Button::new("library-bulk-cancel")
                .ghost()
                .disabled(self.saving)
                .label(t(cx, "取消", "Cancel"))
                .on_click(cx.listener(|view, _, window, cx| view.close_library_batch(window, cx))),
        );
        if prompt.review.is_some() {
            buttons = buttons
                .child(
                    Button::new("library-bulk-back")
                        .ghost()
                        .disabled(self.saving)
                        .label(t(cx, "返回修改", "Back to edit"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let Some(prompt) = &mut view.library_batch_prompt {
                                prompt.review = None;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("library-bulk-confirm")
                        .primary()
                        .disabled(self.saving)
                        .label(if matches!(action, ConnectionLibraryAction::Purge) {
                            t(cx, "确认永久删除", "Confirm permanent deletion")
                        } else {
                            t(cx, "确认本次修改", "Confirm changes")
                        })
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.confirm_library_batch(window, cx)
                        })),
                );
        } else {
            buttons = buttons.child(
                Button::new("library-bulk-review")
                    .primary()
                    .disabled(self.saving)
                    .label(t(cx, "审阅影响对象", "Review affected profiles"))
                    .on_click(cx.listener(|view, _, _, cx| view.review_library_batch(cx))),
            );
        }
        let footer=div().flex().flex_col().gap_2().child(div().text_xs().child(format!("{} {}",prompt.ids.len(),t(cx,"条精确选中的连接；筛选之外的选项也列在上方","explicitly selected profiles; selections outside the filter are also listed above")))).child(buttons);
        footer.into_any_element()
    }
}
