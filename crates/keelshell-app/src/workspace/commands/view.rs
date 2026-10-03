//! Command library, review suggestions and snippet modal composition.
use super::*;

impl Workspace {
    pub(in crate::workspace) fn suggestion_list(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.command.read(cx).focus_handle(cx).is_focused(window) {
            return div().into_any_element();
        }
        let candidates = self.command_candidates(cx);
        if candidates.is_empty() {
            return div().into_any_element();
        }
        let mut list = div()
            .id("command-suggestions")
            .test_support()
            .track_scroll(&self.suggestion_scroll)
            .flex_shrink_0()
            .max_h(px(176.))
            .overflow_y_scroll()
            .bg(rgb(PANEL));
        for (index, ticket) in candidates.into_iter().enumerate() {
            let source = match ticket.source {
                SuggestionSource::History => t(cx, "历史", "History"),
                SuggestionSource::Snippet(_) => t(cx, "片段", "Snippet"),
            };
            let title = ticket.title.clone();
            let preview: String = ticket
                .command
                .chars()
                .take(180)
                .map(|c| if c == '\n' { '↵' } else { c })
                .collect();
            list = list.child(
                div()
                    .id(("command-suggestion", index))
                    .test_support()
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(index == self.suggestion_selected, |el| {
                        el.bg(rgb(crate::design::SELECTED))
                    })
                    .child(
                        div()
                            .w(px(56.))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(source),
                    )
                    .child(div().w(px(150.)).min_w_0().text_ellipsis().child(title))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .font_family("monospace")
                            .child(preview),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.insert_candidate(ticket.clone(), window, cx)
                    })),
            );
        }
        // ScrollHandle measures the scroll viewport before border insets. Keep
        // the separator outside it so the last keyboard-selected row is visible.
        div()
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(list)
            .into_any_element()
    }

    pub(in crate::workspace) fn snippet_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let content = if let Some(panel) = &self.snippet_parameters {
            div()
                .w(px(900.))
                .h(px(660.))
                .max_w_full()
                .max_h_full()
                .child(panel.clone())
                .into_any_element()
        } else if let Some(panel) = &self.snippet_editor {
            div()
                .w(px(900.))
                .h(px(560.))
                .max_w_full()
                .max_h_full()
                .child(panel.clone())
                .into_any_element()
        } else if let Some(snippet) = &self.snippet_delete {
            div().w(px(560.)).max_w_full().p_4().flex().flex_col().gap_3().track_focus(&self.overlay_focus)
                .child(t(cx,"删除命令片段？","Delete command snippet?"))
                .child(div().max_h(px(120.)).overflow_hidden().child(snippet.name.clone()))
                .child(t(cx,"删除本机保存的片段，不会修改命令栏中已填入的内容。", "Deletes the saved snippet; text already inserted in the command bar stays unchanged."))
                .child(div().text_color(rgb(MUTED)).child(self.status.render(cx)))
                .child(div().flex().justify_end().gap_2()
                    .child(Button::new("cancel-delete-snippet").ghost().disabled(self.saving).label(t(cx,"取消","Cancel")).on_click(cx.listener(|view,_,window,cx|view.close_snippet_modal(window,cx))))
                    .child(Button::new("confirm-delete-snippet").primary().disabled(self.saving).label(t(cx,"确认删除","Delete")).on_click(cx.listener(|view,_,window,cx|view.delete_snippet(window,cx)))))
                .into_any_element()
        } else {
            return div().into_any_element();
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w_full()
                    .max_h_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    .child(content),
            )
            .into_any_element()
    }
    pub(in crate::workspace) fn command_library(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div()
            .id("commands-list")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.snippet_search).small()),
                    )
                    .child(
                        Button::new("open-batch-commands")
                            .ghost()
                            .compact()
                            .label(t(cx, "批量执行", "Batch commands"))
                            .disabled(self.command_surface_blocked())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_batch_commands(false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("new-snippet")
                            .primary()
                            .compact()
                            .label(t(cx, "新建片段", "New snippet"))
                            .disabled(!self.can_manage_snippets())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_snippet_editor(None, window, cx)
                            })),
                    ),
            );
        let mut rows = div()
            .id("command-library-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let query = self.snippet_search.read(cx).value();
        let history = self
            .tabs
            .get(self.active)
            .and_then(|terminal| self.command_histories.get(&terminal.entity_id()));
        rows = rows.child(
            div()
                .h(px(30.))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .bg(rgb(BG))
                .child(t(
                    cx,
                    "当前会话历史 · 仅本次运行",
                    "Session history · this app run only",
                ))
                .child(
                    Button::new("clear-command-history")
                        .ghost()
                        .compact()
                        .label(t(cx, "清空", "Clear"))
                        .disabled(
                            history.is_none_or(|history| history.newest_first().next().is_none()),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.clear_command_history(cx))),
                ),
        );
        if let Some(history) = history {
            for (index, command) in history.newest_first().enumerate() {
                if !query.trim().is_empty()
                    && !command
                        .to_lowercase()
                        .contains(&query.trim().to_lowercase())
                {
                    continue;
                }
                let ticket =
                    self.candidate(command.into(), String::new(), SuggestionSource::History, cx);
                rows = rows.child(
                    div()
                        .h(px(34.))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family("monospace")
                                .text_ellipsis()
                                .child(command.to_owned()),
                        )
                        .child(
                            Button::new(("use-history", index))
                                .ghost()
                                .compact()
                                .label(t(cx, "填入", "Insert"))
                                .disabled(ticket.is_none())
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if let Some(ticket) = ticket.clone() {
                                        view.insert_candidate(ticket, window, cx)
                                    }
                                })),
                        ),
                );
            }
        }
        rows = rows.child(div().px_3().py_1().bg(rgb(BG)).child(t(
            cx,
            "保存的命令片段",
            "Saved command snippets",
        )));
        let mut count = 0;
        for snippet in &self.state.snippets {
            if !command_suggestions::snippet_matches(snippet, query.as_str()) {
                continue;
            }
            count += 1;
            let ticket = self.candidate(
                snippet.command.clone(),
                snippet.name.clone(),
                SuggestionSource::Snippet(snippet.id),
                cx,
            );
            let editing = snippet.clone();
            let deleting = snippet.clone();
            let id = snippet.id.to_string();
            let preview: String = snippet
                .command
                .chars()
                .take(180)
                .map(|c| if c == '\n' { '↵' } else { c })
                .collect();
            rows = rows.child(
                div()
                    .min_h(px(56.))
                    .px_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div().min_w_0().text_ellipsis().child(snippet.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .text_ellipsis()
                                            .text_color(rgb(MUTED))
                                            .child(snippet.tags.join(" · ")),
                                    ),
                            )
                            .when(!snippet.description.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_ellipsis()
                                        .text_color(rgb(MUTED))
                                        .child(snippet.description.clone()),
                                )
                            })
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_ellipsis()
                                    .child(preview),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .gap_1()
                            .child(
                                Button::new(SharedString::from(format!("insert-snippet-{id}")))
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "填入", "Insert"))
                                    .disabled(ticket.is_none())
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        if let Some(ticket) = ticket.clone() {
                                            view.insert_candidate(ticket, window, cx)
                                        }
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("edit-snippet-{id}")))
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "编辑", "Edit"))
                                    .disabled(!self.can_manage_snippets())
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.open_snippet_editor(Some(editing.clone()), window, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("delete-snippet-{id}")))
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "删除", "Delete"))
                                    .disabled(!self.can_manage_snippets())
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        if view.can_manage_snippets() {
                                            view.snippet_delete = Some(deleting.clone());
                                            view.status = Message::empty();
                                            view.focus_current_surface(window, cx);
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            );
        }
        if count == 0 {
            rows = rows.child(div().p_3().text_color(rgb(MUTED)).child(t(
                cx,
                "没有匹配片段，可调整搜索或新建片段。",
                "No matching snippets. Change your search or create one.",
            )));
        }
        list = list.child(rows);
        list.into_any_element()
    }
}
