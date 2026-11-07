//! Connection manager with a persistent folder tree, recent usage and recoverable deletion.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use crate::jump_host_picker::endpoint;
use gpui_kit::{
    assets::IconName,
    component::{Icon, Selectable, tooltip::Tooltip},
};
use keelshell_core::ConnectionLibraryAction;

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
    fn library_scope_presentation(
        &self,
        cx: &mut Context<Self>,
    ) -> (IconName, String, String, u32, u32) {
        let visual = crate::design::palette(cx);
        match self.library_filter {
            LibraryFilter::All => (
                IconName::Server,
                t(cx, "全部连接", "All connections").to_string(),
                t(
                    cx,
                    "所有已保存的 SSH 连接配置，可直接连接或继续整理。",
                    "Every saved SSH profile, ready to connect or organize.",
                )
                .to_string(),
                visual.accent,
                visual.selected,
            ),
            LibraryFilter::Favorites => (
                IconName::Bookmark,
                t(cx, "收藏", "Favorites").to_string(),
                t(
                    cx,
                    "把常用连接集中在这里，快速回到高频工作目标。",
                    "Keep frequently used profiles close to your daily work.",
                )
                .to_string(),
                visual.warning,
                visual.selected,
            ),
            LibraryFilter::Recent => (
                IconName::Activity,
                t(cx, "最近连接", "Recent").to_string(),
                t(
                    cx,
                    "按成功连接记录整理；失败尝试不会进入最近列表。",
                    "Successful connection history only; failed attempts stay out.",
                )
                .to_string(),
                visual.success,
                visual.selected,
            ),
            LibraryFilter::Trash => (
                IconName::Trash,
                t(cx, "回收站", "Trash").to_string(),
                t(
                    cx,
                    "已移除的连接配置仍可恢复；永久删除需要再次审核。",
                    "Removed profiles remain recoverable; permanent deletion requires review.",
                )
                .to_string(),
                visual.danger,
                visual.danger_surface,
            ),
            LibraryFilter::Folder(folder) => (
                IconName::FolderOpen,
                self.folder_label(folder, cx),
                if folder.is_some() {
                    t(
                        cx,
                        "显示此文件夹及其子文件夹中的连接。",
                        "Profiles in this folder and its descendants.",
                    )
                } else {
                    t(
                        cx,
                        "显示尚未归档到文件夹的连接。",
                        "Profiles that are not assigned to a folder.",
                    )
                }
                .to_string(),
                visual.accent,
                visual.selected,
            ),
        }
    }

    fn library_filter_summary(
        &self,
        total_count: usize,
        visible_count: usize,
        query: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let components = crate::design::components(cx);
        let (icon, title, description, accent, accent_surface) =
            self.library_scope_presentation(cx);
        let query = query.trim();
        let count_label = format!(
            "{visible_count} / {total_count} {}",
            t(cx, "个连接", "profiles")
        );
        let selected_count = self.library_selection.len();
        let selection_label = if selected_count == 0 {
            t(cx, "未选择连接", "No profiles selected").to_string()
        } else {
            format!(
                "{} {}",
                selected_count,
                t(cx, "个连接已选择", "profiles selected")
            )
        };
        let aria_label = if query.is_empty() {
            format!(
                "{}: {title}. {count_label}. {selection_label}",
                t(cx, "连接库筛选", "Connection library filter")
            )
        } else {
            format!(
                "{}: {title}. {}: {query}. {count_label}. {selection_label}",
                t(cx, "连接库筛选", "Connection library filter"),
                t(cx, "搜索", "Search")
            )
        };
        let mut metrics = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap_1()
            .child(
                div()
                    .id("library-filter-count")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(count_label.clone())
                    .px_2()
                    .py_1()
                    .rounded(px(components.control_radius))
                    .bg(rgb(visual.surface_active))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(count_label),
            );
        if selected_count > 0 {
            metrics = metrics.child(
                div()
                    .id("library-selection-count")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(selection_label.clone())
                    .px_2()
                    .py_1()
                    .rounded(px(components.control_radius))
                    .bg(rgb(accent_surface))
                    .text_color(rgb(accent))
                    .text_xs()
                    .child(selection_label),
            );
        }
        if !query.is_empty() {
            metrics = metrics
                .child(
                    div()
                        .id("library-query-summary")
                        .test_support()
                        .role(Role::Label)
                        .aria_label(format!("{}: {query}", t(cx, "搜索", "Search")))
                        .max_w(px(180.))
                        .px_2()
                        .py_1()
                        .rounded(px(components.control_radius))
                        .bg(rgb(visual.surface_hover))
                        .text_color(rgb(visual.muted))
                        .text_xs()
                        .text_ellipsis()
                        .child(format!("{}: {query}", t(cx, "搜索", "Search"))),
                )
                .child(
                    Button::new("library-clear-filter")
                        .ghost()
                        .compact()
                        .icon(IconName::Close)
                        .label(t(cx, "清除筛选", "Clear filter"))
                        .accessibility_label(t(cx, "清除搜索筛选", "Clear search filter"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.search.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                            });
                            cx.notify();
                        }))
                        .map(|button| self.retain_modal_button("library-clear-filter", button, cx)),
                );
        }
        div()
            .id("library-filter-summary")
            .test_support()
            .role(Role::Group)
            .aria_label(aria_label)
            .px_3()
            .py_2()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .bg(rgb(visual.chrome))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .id("library-scope-icon")
                            .test_support()
                            .flex_shrink_0()
                            .p_2()
                            .rounded(px(components.control_radius))
                            .bg(rgb(accent_surface))
                            .text_color(rgb(accent))
                            .child(Icon::new(icon).size_3()),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .id("library-scope-title")
                                    .test_support()
                                    .role(Role::Label)
                                    .aria_label(title.clone())
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .id("library-scope-description")
                                    .test_support()
                                    .role(Role::Label)
                                    .aria_label(description.clone())
                                    .text_xs()
                                    .text_color(rgb(visual.muted))
                                    .text_ellipsis()
                                    .child(description),
                            ),
                    ),
            )
            .child(metrics)
            .into_any_element()
    }

    fn library_empty_state(
        &self,
        query: &str,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let components = crate::design::components(cx);
        let (icon, _scope_title, scope_description, accent, accent_surface) =
            self.library_scope_presentation(cx);
        let query = query.trim();
        let (title, description) = if !query.is_empty() {
            (
                t(cx, "没有匹配的连接", "No matching profiles").to_string(),
                t(
                    cx,
                    "清除搜索条件，或切换左侧的收藏、最近连接、回收站和文件夹。",
                    "Clear the search or switch between favorites, recent, trash and folders.",
                )
                .to_string(),
            )
        } else {
            match self.library_filter {
                LibraryFilter::Trash => (
                    t(cx, "回收站为空", "Trash is empty").to_string(),
                    t(
                        cx,
                        "移入回收站的连接会在这里保留，之后可以恢复或经过审核永久删除。",
                        "Profiles moved to trash stay here until you restore or review permanent deletion.",
                    )
                    .to_string(),
                ),
                LibraryFilter::Recent => (
                    t(cx, "暂无最近连接", "No recent connections").to_string(),
                    t(
                        cx,
                        "成功建立 SSH 会话后，连接会按最近使用时间出现在这里。",
                        "Profiles appear here after a successful SSH session.",
                    )
                    .to_string(),
                ),
                LibraryFilter::Favorites => (
                    t(cx, "还没有收藏连接", "No favorite profiles").to_string(),
                    t(
                        cx,
                        "在连接行中点击星标，把常用目标固定到这个工作区。",
                        "Use the star on a profile row to pin a frequent target here.",
                    )
                    .to_string(),
                ),
                _ => (
                    t(cx, "还没有连接配置", "No saved profiles").to_string(),
                    t(
                        cx,
                        "新建或导入一个 SSH 连接，开始建立你的远程工作区。",
                        "Create or import an SSH profile to start building your remote workspace.",
                    )
                    .to_string(),
                ),
            }
        };
        let empty_aria_label = format!("{title}: {description}");
        let mut card = div()
            .id("library-empty-card")
            .test_support()
            .max_w(px(520.))
            .w_full()
            .p_5()
            .rounded(px(components.panel_radius))
            .border_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("library-empty-icon")
                    .test_support()
                    .p_3()
                    .rounded(px(components.control_radius))
                    .bg(rgb(accent_surface))
                    .text_color(rgb(accent))
                    .child(Icon::new(icon).size_4()),
            )
            .child(
                div()
                    .id("library-empty-title")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(title.clone())
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.clone()),
            )
            .child(
                div()
                    .id("library-empty-description")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(description.clone())
                    .text_sm()
                    .text_color(rgb(visual.muted))
                    .child(description.clone()),
            );
        if !query.is_empty() {
            card = card.child(
                Button::new("library-empty-clear-filter")
                    .ghost()
                    .compact()
                    .icon(IconName::Close)
                    .label(t(cx, "清除搜索", "Clear search"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.search.update(cx, |input, cx| {
                            input.set_value("", window, cx);
                        });
                        cx.notify();
                    }))
                    .map(|button| {
                        self.retain_modal_button("library-empty-clear-filter", button, cx)
                    }),
            );
        } else if total_count == 0 && matches!(self.library_filter, LibraryFilter::All) {
            card = card.child(
                Button::new("library-empty-new-connection")
                    .primary()
                    .compact()
                    .icon(IconName::Plus)
                    .label(t(cx, "新建连接", "New connection"))
                    .disabled(self.saving)
                    .on_click(cx.listener(|view, _, window, cx| view.open_form(window, cx)))
                    .map(|button| {
                        self.retain_modal_button("library-empty-new-connection", button, cx)
                    }),
            );
        } else {
            card = card.child(
                div()
                    .id("library-empty-scope-hint")
                    .test_support()
                    .role(Role::Label)
                    .aria_label(scope_description.clone())
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(scope_description),
            );
        }
        div()
            .id("library-empty-state")
            .test_support()
            .role(Role::Group)
            .aria_label(empty_aria_label)
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(rgb(visual.canvas))
            .child(card)
            .into_any_element()
    }

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
        let scope_count = profiles.len();
        let profiles: Vec<_> = profiles
            .into_iter()
            .filter(|(item, _)| item.matches(&query))
            .collect();
        let total_count = profiles.len();
        let mut rows = div()
            .id("connection-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut count = 0;
        for (index, (connection, time)) in profiles.into_iter().enumerate() {
            count += 1;
            rows =
                rows.child(self.connection_library_row(index, connection, time, cards, trash, cx));
        }
        if count == 0 {
            rows = rows.child(self.library_empty_state(&query, total_count, cx));
        }
        let tree = self.library_folder_tree(stacked, cx);
        let header = self.library_table_header(trash, recent, cx);
        div().flex().flex_col().size_full().text_sm().bg(rgb(visual.surface))
            .child(self.library_filter_summary(scope_count, count, &query, cx))
            .child(self.library_import_toolbar(stacked,cx))
            .child(self.library_batch_toolbar(cx))
            .child(div().flex_1().min_h_0().min_w_0().flex().when(stacked,|body|body.flex_col()).child(tree)
                .child(div().id("connection-table-scroll").flex_1().min_w_0().overflow_x_scroll()
                    .child(div().min_w_0().size_full().flex().flex_col()
                        .when(!cards,|table|table.min_w(px(if trash {880.} else if recent {1070.} else {1010.})).child(header)).child(rows))))
            .child(div().h(px(28.)).flex_shrink_0().px_3().flex().items_center().text_xs().text_color(rgb(visual.muted))
                .border_t_1().border_color(rgb(visual.border)).child(if trash {
                    format!("{} · {count} / {scope_count}",t(cx,"仅移除连接配置，不会删除服务器或凭据库条目","Profile metadata only; servers and encrypted vault entries are retained"))
                }else{
                    format!("{} {count} / {scope_count} · {}",t(cx,"连接","Connections"),t(cx,"文件夹筛选包含子文件夹","Folder filters include descendants"))
                })).into_any_element()
    }

    fn connection_library_row(
        &self,
        index: usize,
        connection: &Connection,
        time: Option<u64>,
        cards: bool,
        trash: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let profile = connection.clone();
        let edit = connection.clone();
        let id = connection.id;
        let selected = self.library_selection.contains(&id);
        let target = format!("{} · {}", connection.name, endpoint(connection));
        let select = Button::new(("library-select", index))
            .ghost()
            .compact()
            .label(if selected { "☑" } else { "☐" })
            .selected(selected)
            .accessibility_label(action_label(
                if selected {
                    t(cx, "取消选择", "Deselect")
                } else {
                    t(cx, "选择连接", "Select profile")
                },
                connection,
            ))
            .disabled(self.saving)
            .on_click(cx.listener(move |view, _, _, cx| view.toggle_library_selection(id, cx)))
            .map(|button| self.retain_modal_button(("library-select", index), button, cx));
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
            .localized_tooltip("切换收藏", "Toggle favorite")
            .disabled(self.saving || trash)
            .on_click(cx.listener(move |view, _, window, cx| view.toggle_favorite(id, window, cx)))
            .map(|button| self.retain_modal_button(("favorite", index), button, cx));
        let mut row = div()
            .id(("connection-row", index))
            .test_support()
            .role(Role::Group)
            .aria_label(target.clone())
            .aria_selected(selected)
            .tooltip(move |window, cx| Tooltip::new(target.clone()).build(window, cx))
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .bg(rgb(if index.is_multiple_of(2) {
                visual.surface
            } else {
                visual.canvas
            }))
            .hover(|style| style.bg(rgb(visual.surface_hover)))
            .border_b_1()
            .border_color(rgb(visual.border))
            .when(cards, |row| row.flex_col().p_2().gap_2())
            .when(!cards, |row| row.h(px(38.)).items_center())
            .when(selected, |row| {
                row.bg(rgb(visual.surface_active))
                    .border_l_1()
                    .border_color(rgb(visual.accent))
            });
        if cards {
            row = row.child(
                div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .min_w_0()
                    .child(select)
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
                .child(select)
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
            actions = actions
                .child(
                    Button::new(("restore", index))
                        .ghost()
                        .compact()
                        .label(t(cx, "恢复", "Restore"))
                        .accessibility_label(action_label(t(cx, "恢复", "Restore"), connection))
                        .disabled(self.saving)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.restore_connection(id, window, cx)
                        }))
                        .map(|button| self.retain_modal_button(("restore", index), button, cx)),
                )
                .child(
                    Button::new(("library-purge", index))
                        .ghost()
                        .compact()
                        .label(t(cx, "永久删除…", "Delete permanently…"))
                        .accessibility_label(action_label(
                            t(cx, "永久删除连接配置", "Permanently delete profile"),
                            connection,
                        ))
                        .disabled(self.saving)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.open_library_batch(
                                vec![id],
                                ConnectionLibraryAction::Purge,
                                false,
                                window,
                                cx,
                            )
                        }))
                        .map(|button| {
                            self.retain_modal_button(("library-purge", index), button, cx)
                        }),
                );
        } else {
            actions = actions
                .child(
                    Button::new(("connect", index))
                        .primary()
                        .compact()
                        .label(t(cx, "连接", "Connect"))
                        .accessibility_label(action_label(t(cx, "连接", "Connect"), connection))
                        .disabled(self.connecting || self.saving)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.request_connect(profile.clone(), window, cx)
                        }))
                        .map(|button| self.retain_modal_button(("connect", index), button, cx)),
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
                        }))
                        .map(|button| self.retain_modal_button(("edit", index), button, cx)),
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
                        }))
                        .map(|button| self.retain_modal_button(("move", index), button, cx)),
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
                                    view.status = Message::detail("复制失败", "Copy failed", error)
                                }
                            }
                            cx.notify();
                        }))
                        .map(|button| self.retain_modal_button(("duplicate", index), button, cx)),
                )
                .child(
                    Button::new(("delete", index))
                        .ghost()
                        .compact()
                        .label(t(cx, "移入回收站", "Trash"))
                        .accessibility_label(action_label(t(cx, "移入回收站", "Trash"), connection))
                        .disabled(self.saving)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.delete_connection(id, window, cx)
                        }))
                        .map(|button| self.retain_modal_button(("delete", index), button, cx)),
                );
        }
        row.child(actions).into_any_element()
    }

    fn library_folder_tree(&self, stacked: bool, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let filters = [
            (
                "all-connections",
                LibraryFilter::All,
                IconName::Server,
                t(cx, "全部连接", "All connections"),
                self.state.connections.len(),
            ),
            (
                "favorite-connections",
                LibraryFilter::Favorites,
                IconName::Bookmark,
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
                IconName::Activity,
                t(cx, "最近连接", "Recent"),
                self.state.recent_connections.len(),
            ),
            (
                "trashed-connections",
                LibraryFilter::Trash,
                IconName::Trash,
                t(cx, "回收站", "Trash"),
                self.state.deleted_connections.len(),
            ),
            (
                "unfiled-connections",
                LibraryFilter::Folder(None),
                IconName::FolderOpen,
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
        for (id, filter, icon, label, total) in filters {
            let selected = self.library_filter == filter;
            tree = tree.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .w_full()
                    .icon(icon)
                    .selected(selected)
                    .toggled(selected)
                    .label(format!("{label}  {total}"))
                    .accessibility_label(format!(
                        "{label}, {total} {}",
                        t(cx, "个连接", "profiles")
                    ))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.set_library_filter(filter, cx);
                    }))
                    .map(|button| self.retain_modal_button(id, button, cx)),
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
                        .localized_tooltip("新建文件夹", "New folder")
                        .disabled(self.saving)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_folder_form(None, window, cx)
                        }))
                        .map(|button| self.retain_modal_button("new-folder", button, cx)),
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
                                view.set_library_filter(LibraryFilter::Folder(Some(id)), cx);
                            }))
                            .map(|button| self.retain_modal_button(("folder", index), button, cx)),
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
                            .localized_tooltip("管理文件夹", "Manage folder")
                            .disabled(self.saving)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.open_folder_form(Some(id), window, cx)
                            }))
                            .map(|button| {
                                self.retain_modal_button(("manage-folder", index), button, cx)
                            }),
                    ),
            );
        }
        tree.into_any_element()
    }

    fn library_table_header(
        &self,
        trash: bool,
        recent: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let time_header = if trash {
            t(cx, "删除时间", "Deleted")
        } else {
            t(cx, "最近连接", "Last connected")
        };
        let header = div()
            .id("connection-table-header")
            .test_support()
            .role(Role::Group)
            .aria_label(t(cx, "连接表格列标题", "Connection table headings"))
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.muted))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(cell(t(cx, "选择", "Select").into(), 36.))
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
        header.into_any_element()
    }

    fn library_import_toolbar(&self, stacked: bool, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("library-actions-toolbar")
            .test_support()
            .role(Role::Group)
            .aria_label(t(cx, "连接库操作", "Connection library actions"))
            .min_h(px(46.))
            .px_3()
            .py_2()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                Button::new("new-connection")
                    .compact()
                    .icon(IconName::Plus)
                    .label(t(cx, "新建连接", "New connection"))
                    .disabled(self.saving)
                    .on_click(cx.listener(|view, _, window, cx| view.open_form(window, cx)))
                    .map(|button| self.retain_modal_button("new-connection", button, cx)),
            )
            .child(
                Button::new("import-connections")
                    .compact()
                    .icon(IconName::Download)
                    .label(t(cx, "导入 JSON", "Import JSON"))
                    .disabled(self.saving)
                    .on_click(
                        cx.listener(|view, _, window, cx| view.import_connections(window, cx)),
                    )
                    .map(|button| self.retain_modal_button("import-connections", button, cx)),
            )
            .child(
                Button::new("import-openssh")
                    .compact()
                    .icon(IconName::Download)
                    .label(t(
                        cx,
                        "从剪贴板导入 SSH 配置",
                        "Import SSH config from clipboard",
                    ))
                    .localized_tooltip(
                        "从剪贴板导入受限 OpenSSH 配置",
                        "Import the supported OpenSSH subset from the clipboard",
                    )
                    .disabled(self.saving)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.import_openssh_connections(window, cx)
                    }))
                    .map(|button| self.retain_modal_button("import-openssh", button, cx)),
            )
            .child(
                Button::new("profile-sync-open")
                    .compact()
                    .label(t(cx, "加密同步", "Encrypted sync"))
                    .disabled(!self.can_open_vault() || !self.profile_sync_maintenance_idle(cx))
                    .on_click(cx.listener(|view, _, window, cx| view.open_profile_sync(window, cx)))
                    .map(|button| self.retain_modal_button("profile-sync-open", button, cx)),
            )
            .child(
                Button::new("export-connections")
                    .compact()
                    .icon(IconName::Upload)
                    .label(t(cx, "导出 JSON", "Export JSON"))
                    .on_click(cx.listener(|view, _, _, cx| view.export_connections(cx)))
                    .map(|button| self.retain_modal_button("export-connections", button, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(120.))
                    .when(stacked, |search| search.w_full())
                    .child(Input::new(&self.search).small().aria_label(t(
                        cx,
                        "搜索连接",
                        "Search connections",
                    ))),
            )
            .into_any_element()
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
                        }))
                        .map(|button| self.retain_modal_button("destination-root", button, cx)),
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
                        }))
                        .map(|button| {
                            self.retain_modal_button(("destination-folder", index), button, cx)
                        }),
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
                                }))
                                .map(|button| {
                                    self.retain_modal_button("cancel-destination", button, cx)
                                }),
                        )
                        .child(
                            Button::new("save-destination")
                                .primary()
                                .disabled(self.saving)
                                .label(t(cx, "确定", "Choose"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.save_destination(window, cx)
                                }))
                                .map(|button| {
                                    self.retain_modal_button("save-destination", button, cx)
                                }),
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
                        }))
                        .map(|button| self.retain_modal_button("folder-parent-root", button, cx)),
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
                        }))
                        .map(|button| {
                            self.retain_modal_button(("folder-parent", index), button, cx)
                        }),
                );
            }
            body=body.child(choices).when_some(form.message.clone(),|el,message|el.child(div().text_sm().text_color(rgb(visual.danger)).child(message.render(cx))))
                .child(div().text_xs().text_color(rgb(visual.muted)).child(t(cx,"删除空文件夹前，需移走子文件夹、连接与回收站中的关联连接。","An empty folder can be removed after moving out its children and active or trashed profiles.")))
                .child(div().flex().justify_between().gap_2()
                    .child(div().when(form.id.is_some(),|el|el.child(Button::new("remove-empty-folder").ghost().disabled(self.saving)
                        .label(t(cx,"删除空文件夹","Remove empty folder")).on_click(cx.listener(|view,_,window,cx|view.remove_empty_folder(window,cx))).map(|button| self.retain_modal_button("remove-empty-folder", button, cx)))))
                    .child(div().flex().gap_2()
                        .child(Button::new("cancel-folder").ghost().disabled(self.saving).label(t(cx,"取消","Cancel"))
                            .on_click(cx.listener(|view,_,window,cx|view.close_folder_form(window,cx))).map(|button| self.retain_modal_button("cancel-folder", button, cx)))
                        .child(Button::new("save-folder").primary().disabled(self.saving).label(t(cx,"保存","Save"))
                            .on_click(cx.listener(|view,_,window,cx|view.save_folder(window,cx))).map(|button| self.retain_modal_button("save-folder", button, cx)))));
        } else {
            return div().into_any_element();
        }
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(visual.overlay))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(body)
            .into_any_element()
    }
}
