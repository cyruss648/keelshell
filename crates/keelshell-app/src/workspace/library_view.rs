//! Connection manager with a persistent folder tree, recent usage and recoverable deletion.
use super::*;
use crate::jump_host_picker::endpoint;
use gpui_kit::{
    assets::IconName,
    component::{Selectable, tooltip::Tooltip},
};

fn action_label(action: &str, connection: &Connection) -> String {
    format!("{action}: {} · {}", connection.name, endpoint(connection))
}

fn cell(text: String, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .text_ellipsis()
        .child(text)
}

fn timestamp(seconds: u64) -> String {
    i64::try_from(seconds)
        .ok()
        .and_then(|value| chrono::DateTime::from_timestamp(value, 0))
        .map(|value| value.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "—".into())
}

impl Workspace {
    pub(super) fn connection_table(
        &self,
        available_width: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        // Wide windows retain comparable columns; compact regions keep each
        // target's identity beside its actions rather than hiding them off-screen.
        let stacked = available_width < px(760.);
        let query = self.search.read(cx).value();
        let trash = self.library_filter == LibraryFilter::Trash;
        let recent = self.library_filter == LibraryFilter::Recent;
        let cards = available_width < px(if recent { 1280. } else { 1240. });
        let profiles: Vec<(&Connection, Option<u64>)> = match self.library_filter {
            LibraryFilter::Trash => self
                .state
                .deleted_connections
                .iter()
                .rev()
                .map(|deleted| (&deleted.connection, Some(deleted.deleted_at)))
                .collect(),
            LibraryFilter::Recent => self
                .state
                .recent_connections
                .iter()
                .filter_map(|recent| {
                    self.state
                        .connections
                        .iter()
                        .find(|item| item.id == recent.connection_id)
                        .map(|profile| (profile, Some(recent.connected_at)))
                })
                .collect(),
            _ => self
                .state
                .connections
                .iter()
                .filter(|item| match self.library_filter {
                    LibraryFilter::Favorites => item.favorite,
                    LibraryFilter::Folder(Some(folder)) => {
                        self.state.folder_contains(folder, item.id)
                    }
                    LibraryFilter::Folder(None) => self.state.folder_id_of(item.id).is_none(),
                    _ => true,
                })
                .map(|item| (item, None))
                .collect(),
        };
        let mut rows = div()
            .id("connection-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut count = 0;
        for (index, (connection, time)) in profiles
            .into_iter()
            .filter(|(item, _)| item.matches(&query))
            .enumerate()
        {
            count += 1;
            let profile = connection.clone();
            let edit = connection.clone();
            let id = connection.id;
            let target = format!("{} · {}", connection.name, endpoint(connection));
            let favorite = Button::new(("favorite", index))
                .ghost()
                .compact()
                .label(if connection.favorite { "★" } else { "☆" })
                .accessibility_label(action_label(
                    if connection.favorite {
                        t(cx, "取消收藏", "Remove from favorites")
                    } else {
                        t(cx, "添加收藏", "Add to favorites")
                    },
                    connection,
                ))
                .tooltip(t(cx, "切换收藏", "Toggle favorite"))
                .disabled(self.saving || trash)
                .on_click(
                    cx.listener(move |view, _, window, cx| view.toggle_favorite(id, window, cx)),
                );
            let mut row = div()
                .id(("connection-row", index))
                .test_support()
                .role(Role::Group)
                .aria_label(target.clone())
                .tooltip(move |window, cx| Tooltip::new(target.clone()).build(window, cx))
                .min_w_0()
                .flex_shrink_0()
                .flex()
                .bg(rgb(if index % 2 == 0 {
                    visual.surface
                } else {
                    visual.canvas
                }))
                .border_b_1()
                .border_color(rgb(visual.border))
                .when(cards, |row| row.flex_col().p_2().gap_2())
                .when(!cards, |row| row.h(px(38.)).items_center());
            if cards {
                row = row.child(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .min_w_0()
                        .child(favorite)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .id(("connection-name", index))
                                        .test_support()
                                        .role(Role::Label)
                                        .aria_label(connection.name.clone())
                                        .text_ellipsis()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(connection.name.clone()),
                                )
                                .child(
                                    div()
                                        .id(("connection-endpoint", index))
                                        .test_support()
                                        .role(Role::Label)
                                        .aria_label(endpoint(connection))
                                        .text_ellipsis()
                                        .text_xs()
                                        .text_color(rgb(visual.muted))
                                        .child(endpoint(connection)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(visual.muted))
                                        .text_ellipsis()
                                        .child(format!(
                                            "{} · {}",
                                            self.folder_label(self.state.folder_id_of(id), cx),
                                            time.map(timestamp)
                                                .unwrap_or_else(|| connection.tags.join(", "))
                                        )),
                                ),
                        ),
                );
            } else {
                row = row
                    .child(favorite)
                    .child(cell(connection.name.clone(), 155.))
                    .child(cell(connection.host.clone(), 145.))
                    .child(cell(connection.port.to_string(), 56.))
                    .child(cell(connection.username.clone(), 90.))
                    .child(cell(
                        self.folder_label(self.state.folder_id_of(id), cx),
                        130.,
                    ));
                row = if let Some(time) = time {
                    row.child(cell(timestamp(time), 190.))
                } else {
                    row.child(cell(connection.tags.join(", "), 120.))
                };
            }
            let mut actions = div()
                .id(("connection-actions", index))
                .test_support()
                .flex()
                .items_center()
                .min_w_0()
                .flex_shrink_0()
                .when(cards, |actions| actions.flex_wrap().gap_1());
            if trash {
                actions = actions.child(
                    Button::new(("restore", index))
                        .ghost()
                        .compact()
                        .label(t(cx, "恢复", "Restore"))
                        .accessibility_label(action_label(t(cx, "恢复", "Restore"), connection))
                        .disabled(self.saving)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.restore_connection(id, window, cx)
                        })),
                );
            } else {
                actions = actions
                    .child(
                        Button::new(("connect", index))
                            .ghost()
                            .compact()
                            .label(t(cx, "连接", "Connect"))
                            .accessibility_label(action_label(t(cx, "连接", "Connect"), connection))
                            .disabled(self.connecting || self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.request_connect(profile.clone(), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("edit", index))
                            .ghost()
                            .compact()
                            .label(t(cx, "编辑", "Edit"))
                            .accessibility_label(action_label(t(cx, "编辑", "Edit"), connection))
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.edit_connection(edit.clone(), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("move", index))
                            .ghost()
                            .compact()
                            .label(t(cx, "移动", "Move"))
                            .accessibility_label(action_label(t(cx, "移动", "Move"), connection))
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.open_destination(DestinationTarget::Connection(id), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("duplicate", index))
                            .ghost()
                            .compact()
                            .label(t(cx, "复制", "Copy"))
                            .accessibility_label(action_label(t(cx, "复制", "Copy"), connection))
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let mut candidate = view.state.clone();
                                match candidate.duplicate_connection(id) {
                                    Ok(_) => view.persist(candidate, AfterSave::None, window, cx),
                                    Err(error) => {
                                        view.status =
                                            Message::detail("复制失败", "Copy failed", error)
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("delete", index))
                            .ghost()
                            .compact()
                            .label(t(cx, "移入回收站", "Trash"))
                            .accessibility_label(action_label(
                                t(cx, "移入回收站", "Trash"),
                                connection,
                            ))
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.delete_connection(id, window, cx)
                            })),
                    );
            }
            rows = rows.child(row.child(actions));
        }
        if count == 0 {
            rows=rows.child(div().p_5().text_color(rgb(visual.muted)).child(if trash {
                t(cx,"回收站中没有匹配的连接。删除连接后，可在这里恢复。","No matching connections in trash. Deleted profiles can be restored here.")
            } else if recent {
                t(cx,"尚无匹配的成功连接记录。连接失败不会计入最近使用。","No matching successful connections yet. Failed attempts are not added to recents.")
            } else {
                t(cx,"没有匹配的 SSH 连接。新建连接，或选择其他文件夹。","No matching SSH connections. Add a profile or choose another folder.")
            }));
        }
        let filters = [
            (
                "all-connections",
                LibraryFilter::All,
                t(cx, "全部连接", "All connections"),
                self.state.connections.len(),
            ),
            (
                "favorite-connections",
                LibraryFilter::Favorites,
                t(cx, "收藏", "Favorites"),
                self.state
                    .connections
                    .iter()
                    .filter(|item| item.favorite)
                    .count(),
            ),
            (
                "recent-connections",
                LibraryFilter::Recent,
                t(cx, "最近连接", "Recent"),
                self.state.recent_connections.len(),
            ),
            (
                "trashed-connections",
                LibraryFilter::Trash,
                t(cx, "回收站", "Trash"),
                self.state.deleted_connections.len(),
            ),
            (
                "unfiled-connections",
                LibraryFilter::Folder(None),
                t(cx, "未归档", "Unfiled"),
                self.state
                    .connections
                    .iter()
                    .filter(|item| self.state.folder_id_of(item.id).is_none())
                    .count(),
            ),
        ];
        let mut tree = div()
            .id("connection-folders")
            .flex_shrink_0()
            .min_w_0()
            .when(stacked, |tree| tree.w_full().h(px(100.)).border_b_1())
            .when(!stacked, |tree| tree.w(px(200.)).h_full().border_r_1())
            .overflow_y_scroll()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
            .p_2()
            .flex()
            .flex_col()
            .gap_1();
        for (id, filter, label, total) in filters {
            tree = tree.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .w_full()
                    .selected(self.library_filter == filter)
                    .label(format!("{label}  {total}"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.library_filter = filter;
                        cx.notify();
                    })),
            );
        }
        tree = tree.child(
            div()
                .mt_3()
                .mb_1()
                .px_1()
                .flex()
                .justify_between()
                .items_center()
                .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "文件夹",
                    "FOLDERS",
                )))
                .child(
                    Button::new("new-folder")
                        .ghost()
                        .compact()
                        .label("+")
                        .accessibility_label(t(cx, "新建文件夹", "New folder"))
                        .tooltip(t(cx, "新建文件夹", "New folder"))
                        .disabled(self.saving)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_folder_form(None, window, cx)
                        })),
                ),
        );
        for (index, folder) in self.state.folder_rows().into_iter().enumerate() {
            let id = folder.id;
            tree = tree.child(
                div()
                    .flex()
                    .items_center()
                    .pl(px((folder.depth.min(8) * 12) as f32))
                    .child(
                        Button::new(("folder", index))
                            .ghost()
                            .compact()
                            .flex_1()
                            .min_w_0()
                            .icon(IconName::Folder)
                            .selected(self.library_filter == LibraryFilter::Folder(Some(id)))
                            .label(folder.name)
                            .tooltip(self.folder_label(Some(id), cx))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.library_filter = LibraryFilter::Folder(Some(id));
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("manage-folder", index))
                            .ghost()
                            .compact()
                            .label("…")
                            .accessibility_label(format!(
                                "{}: {}",
                                t(cx, "管理文件夹", "Manage folder"),
                                self.folder_label(Some(id), cx),
                            ))
                            .tooltip(t(cx, "管理文件夹", "Manage folder"))
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.open_folder_form(Some(id), window, cx)
                            })),
                    ),
            );
        }
        let time_header = if trash {
            t(cx, "删除时间", "Deleted")
        } else {
            t(cx, "最近连接", "Last connected")
        };
        let header = div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.muted))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(cell("☆".into(), 36.))
            .child(cell(t(cx, "名称", "Name").into(), 155.))
            .child(cell(t(cx, "主机", "Host").into(), 145.))
            .child(cell(t(cx, "端口", "Port").into(), 56.))
            .child(cell(t(cx, "用户", "User").into(), 90.))
            .child(cell(t(cx, "文件夹", "Folder").into(), 130.))
            .child(cell(
                if trash || recent {
                    time_header
                } else {
                    t(cx, "标签", "Tags")
                }
                .into(),
                if trash || recent { 190. } else { 120. },
            ));
        div().flex().flex_col().size_full().text_sm().bg(rgb(visual.surface))
            .child(div().min_h(px(46.)).px_3().py_2().flex_shrink_0().flex().flex_wrap().items_center().gap_2()
                .border_b_1().border_color(rgb(visual.border))
                .child(Button::new("new-connection").compact().icon(IconName::Plus).label(t(cx,"新建连接","New connection"))
                    .disabled(self.saving).on_click(cx.listener(|view,_,window,cx|view.open_form(window,cx))))
                .child(Button::new("import-connections").compact().icon(IconName::Download).label(t(cx,"导入 JSON","Import JSON"))
                    .disabled(self.saving).on_click(cx.listener(|view,_,window,cx|view.import_connections(window,cx))))
                .child(Button::new("import-openssh").compact().icon(IconName::Download).label(t(cx,"从剪贴板导入 SSH 配置","Import SSH config from clipboard"))
                    .tooltip(t(cx,"从剪贴板导入受限 OpenSSH 配置","Import the supported OpenSSH subset from the clipboard"))
                    .disabled(self.saving).on_click(cx.listener(|view,_,window,cx|view.import_openssh_connections(window,cx))))
                .child(Button::new("export-connections").compact().icon(IconName::Upload).label(t(cx,"导出 JSON","Export JSON"))
                    .on_click(cx.listener(|view,_,_,cx|view.export_connections(cx))))
                .child(div().flex_1().min_w(px(120.)).when(stacked,|search|search.w_full())
                    .child(Input::new(&self.search).small().aria_label(t(cx,"搜索连接","Search connections")))))
            .child(div().flex_1().min_h_0().min_w_0().flex().when(stacked,|body|body.flex_col()).child(tree)
                .child(div().id("connection-table-scroll").flex_1().min_w_0().overflow_x_scroll()
                    .child(div().min_w_0().size_full().flex().flex_col()
                        .when(!cards,|table|table.min_w(px(if trash {880.} else if recent {1070.} else {1010.})).child(header)).child(rows))))
            .child(div().h(px(28.)).flex_shrink_0().px_3().flex().items_center().text_xs().text_color(rgb(visual.muted))
                .border_t_1().border_color(rgb(visual.border)).child(if trash {
                    format!("{} · {count}",t(cx,"仅移除连接配置，不会删除服务器或凭据库条目","Profile metadata only; servers and encrypted vault entries are retained"))
                }else{
                    format!("{} {count} · {}",t(cx,"连接","Connections"),t(cx,"文件夹筛选包含子文件夹","Folder filters include descendants"))
                })).into_any_element()
    }

    pub(super) fn library_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let root_choice = t(cx, "未归档 / 根目录", "Unfiled / root");
        let mut body = div()
            .id("library-dialog-body")
            .w(px(560.))
            .max_w_full()
            .max_h_full()
            .overflow_y_scroll()
            .p_5()
            .rounded_lg()
            .bg(rgb(visual.surface))
            .border_1()
            .border_color(rgb(visual.border))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_3();
        if let Some(prompt) = &self.destination_prompt {
            body = body
                .track_focus(&prompt.focus)
                .child(div().text_lg().font_weight(FontWeight::BOLD).child(t(
                    cx,
                    "选择目标文件夹",
                    "Choose destination folder",
                )))
                .child(div().text_sm().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "只调整连接的归档位置，当前 SSH 会话保持连接。",
                    "Move the profile without changing its active SSH session.",
                )));
            let mut choices = div()
                .id("destination-choices")
                .h(px(240.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("destination-root")
                        .ghost()
                        .selected(prompt.folder_id.is_none())
                        .disabled(self.saving)
                        .label(root_choice)
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let Some(prompt) = &mut view.destination_prompt {
                                prompt.folder_id = None;
                            }
                            cx.notify();
                        })),
                );
            for (index, folder) in self.state.folder_rows().into_iter().enumerate() {
                let id = folder.id;
                choices = choices.child(
                    Button::new(("destination-folder", index))
                        .ghost()
                        .selected(prompt.folder_id == Some(id))
                        .disabled(self.saving)
                        .label(self.folder_label(Some(id), cx))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(prompt) = &mut view.destination_prompt {
                                prompt.folder_id = Some(id);
                            }
                            cx.notify();
                        })),
                );
            }
            body = body
                .child(choices)
                .when_some(prompt.message.clone(), |el, message| {
                    el.child(div().text_sm().child(message.render(cx)))
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-destination")
                                .ghost()
                                .disabled(self.saving)
                                .label(t(cx, "取消", "Cancel"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.close_destination(window, cx)
                                })),
                        )
                        .child(
                            Button::new("save-destination")
                                .primary()
                                .disabled(self.saving)
                                .label(t(cx, "确定", "Choose"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.save_destination(window, cx)
                                })),
                        ),
                );
        } else if let Some(form) = &self.folder_form {
            body =
                body.child(div().text_lg().font_weight(FontWeight::BOLD).child(
                    if form.id.is_some() {
                        t(cx, "管理文件夹", "Manage folder")
                    } else {
                        t(cx, "新建文件夹", "New folder")
                    },
                ))
                .child(Input::new(&form.name).disabled(self.saving))
                .child(div().text_sm().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "上级文件夹",
                    "Parent folder",
                )));
            let mut choices = div()
                .id("folder-parent-choices")
                .h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("folder-parent-root")
                        .ghost()
                        .selected(form.parent_id.is_none())
                        .disabled(self.saving)
                        .label(t(cx, "根目录", "Root"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let Some(form) = &mut view.folder_form {
                                form.parent_id = None;
                            }
                            cx.notify();
                        })),
                );
            for (index, folder) in self.state.folder_rows().into_iter().enumerate() {
                let id = folder.id;
                choices = choices.child(
                    Button::new(("folder-parent", index))
                        .ghost()
                        .selected(form.parent_id == Some(id))
                        .disabled(self.saving || form.id == Some(id))
                        .label(self.folder_label(Some(id), cx))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(form) = &mut view.folder_form {
                                form.parent_id = Some(id);
                            }
                            cx.notify();
                        })),
                );
            }
            body=body.child(choices).when_some(form.message.clone(),|el,message|el.child(div().text_sm().text_color(rgb(visual.danger)).child(message.render(cx))))
                .child(div().text_xs().text_color(rgb(visual.muted)).child(t(cx,"删除空文件夹前，需移走子文件夹、连接与回收站中的关联连接。","An empty folder can be removed after moving out its children and active or trashed profiles.")))
                .child(div().flex().justify_between().gap_2()
                    .child(div().when(form.id.is_some(),|el|el.child(Button::new("remove-empty-folder").ghost().disabled(self.saving)
                        .label(t(cx,"删除空文件夹","Remove empty folder")).on_click(cx.listener(|view,_,window,cx|view.remove_empty_folder(window,cx))))))
                    .child(div().flex().gap_2()
                        .child(Button::new("cancel-folder").ghost().disabled(self.saving).label(t(cx,"取消","Cancel"))
                            .on_click(cx.listener(|view,_,window,cx|view.close_folder_form(window,cx))))
                        .child(Button::new("save-folder").primary().disabled(self.saving).label(t(cx,"保存","Save"))
                            .on_click(cx.listener(|view,_,window,cx|view.save_folder(window,cx))))));
        } else {
            return div().into_any_element();
        }
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(body)
            .into_any_element()
    }
}
