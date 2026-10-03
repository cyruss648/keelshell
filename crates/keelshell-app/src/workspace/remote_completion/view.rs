//! Compact, explicit query controls; local and remote candidates never share an insertion mode.
use super::*;

impl Workspace {
    pub(in crate::workspace) fn remote_completion_controls(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ready = self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.read(cx).is_open())
            && !self.command_surface_blocked();
        let busy = self.remote_completion.worker.is_some();
        div()
            .id("remote-completion-controls")
            .test_support()
            .flex_shrink_0()
            .px_2()
            .py_1()
            .bg(rgb(PANEL))
            .border_t_1()
            .border_color(rgb(BORDER))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .text_ellipsis()
                            .child(t(
                                cx,
                                "补全基准目录 · 独立于终端工作目录",
                                "Completion directory · separate from terminal cwd",
                            )),
                    )
                    .child(
                        Button::new("remote-complete")
                            .ghost()
                            .compact()
                            .label(t(cx, "远端补全", "Complete remotely"))
                            .tooltip(t(
                                cx,
                                "远端补全 · Ctrl+Space（只查询，不执行）",
                                "Remote completion · Ctrl+Space (query only)",
                            ))
                            .disabled(!ready || busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.request_remote_completion(false, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .id("completion-directory-field")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Input::new(&self.remote_completion.directory)
                                    .small()
                                    .disabled(!ready),
                            ),
                    )
                    .child(
                        Button::new("completion-use-files")
                            .ghost()
                            .compact()
                            .label(t(cx, "文件目录", "Files directory"))
                            .disabled(!ready)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.use_files_completion_directory(window, cx)
                            })),
                    )
                    .child(
                        Button::new("completion-read-base")
                            .ghost()
                            .compact()
                            .label(t(cx, "SFTP 起点", "SFTP base"))
                            .disabled(!ready || busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.request_remote_completion(true, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(in crate::workspace) fn remote_completion_list(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.remote_completion;
        if !state.visible() || self.command_surface_blocked() {
            return div().into_any_element();
        }
        let mut content = div()
            .id("remote-completion-panel")
            .test_support()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .text_ellipsis()
                            .child(
                                state
                                    .message
                                    .as_ref()
                                    .map(|message| message.render(cx))
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(
                        Button::new("cancel-remote-completion")
                            .ghost()
                            .compact()
                            .label(if state.worker.is_some() {
                                t(cx, "取消查询", "Cancel query")
                            } else {
                                t(cx, "关闭", "Dismiss")
                            })
                            .on_click(
                                cx.listener(|view, _, _, cx| view.cancel_remote_completion(cx)),
                            ),
                    ),
            );
        if let Some(host) = state
            .ticket
            .as_ref()
            .and_then(|ticket| self.remote_hosts.get(&ticket.target))
        {
            content = content.child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(MUTED))
                    .child(format!("{} · {host}", t(cx, "查询目标", "Query target"))),
            );
        }
        if state.ticket.as_ref().is_some_and(|ticket| ticket.commands) {
            content=content.child(div().px_2().pb_1().text_xs().text_color(rgb(MUTED))
                .child(t(cx,"远端 PATH · 独立 SSH 查询环境，不包含交互终端的别名、函数。", "Remote PATH · independent SSH query environment; no interactive aliases or functions.")));
        } else if let Some(directory) = &state.resolved_directory {
            content = content.child(
                div()
                    .id("completion-resolved-directory")
                    .test_support()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .min_w_0()
                    .text_ellipsis()
                    .child(format!(
                        "{}: {directory}",
                        t(cx, "实际查询目录", "Resolved directory")
                    )),
            );
        }
        if state.limited || state.skipped > 0 {
            content = content.child(div().px_2().pb_1().text_xs().text_color(rgb(MUTED)).child(
                format!(
                    "{} · {} {}",
                    t(
                        cx,
                        "结果受限；可缩短查询范围",
                        "Limited results; narrow the query"
                    ),
                    state.skipped,
                    t(cx, "项已跳过", "entries skipped")
                ),
            ));
        }
        let mut list = div()
            .id("remote-completion-list")
            .test_support()
            .max_h(px(144.))
            .overflow_y_scroll()
            .track_scroll(&state.scroll)
            .flex_shrink_0();
        for (index, choice) in state.choices.iter().cloned().enumerate() {
            let source = match choice.kind {
                CompletionKind::Executable => t(cx, "远端命令", "Command"),
                CompletionKind::Directory => t(cx, "远端目录", "Directory"),
                CompletionKind::File => t(cx, "远端文件", "File"),
                CompletionKind::Symlink => t(cx, "符号链接", "Symlink"),
            };
            let label = if choice.is_symlink {
                format!("{} ↗", choice.candidate.name)
            } else {
                choice.candidate.name.clone()
            };
            list = list.child(
                div()
                    .id(("remote-completion-choice", index))
                    .test_support()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(index == state.selected, |row| {
                        row.bg(rgb(crate::design::SELECTED))
                    })
                    .child(
                        div()
                            .w(px(65.))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(source),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .font_family("monospace")
                            .child(label),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.insert_remote_completion(choice.clone(), window, cx)
                    })),
            );
        }
        content.child(list).into_any_element()
    }
}
