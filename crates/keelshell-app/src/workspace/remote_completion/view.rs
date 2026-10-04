//! Compact, explicit query controls; local and remote candidates never share an insertion mode.
use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::assets::IconName;

impl Workspace {
    pub(in crate::workspace) fn remote_completion_controls(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let ready = self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.read(cx).is_open())
            && !self.command_surface_blocked();
        let busy = self.remote_completion.worker.is_some();
        let directory = div()
            .id("completion-directory-field")
            .test_support()
            .flex_1()
            .min_w_0()
            .localized_tooltip(
                "补全基准目录独立于终端工作目录，只用于显式补全查询",
                "The completion directory is separate from terminal cwd and only used for explicit queries",
            )
            .child(
                Input::new(&self.remote_completion.directory)
                    .small()
                    .aria_label(t(cx, "补全基准目录 · 独立于终端工作目录", "Completion directory · separate from terminal cwd"))
                    .disabled(!ready),
            );
        let complete =
            Button::new("remote-complete")
                .ghost()
                .compact()
                .label(t(cx, "远端补全", "Complete remotely"))
                .localized_tooltip(
                    "远端补全 · Ctrl+Space（只查询，不执行）",
                    "Remote completion · Ctrl+Space (query only)",
                )
                .disabled(!ready || busy)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.request_remote_completion(false, window, cx)
                }));
        let files = Button::new("completion-use-files")
            .ghost()
            .compact()
            .when(compact, |button| {
                button.icon(IconName::FolderOpen).accessibility_label(t(
                    cx,
                    "文件目录",
                    "Files directory",
                ))
            })
            .when(!compact, |button| {
                button.label(t(cx, "文件目录", "Files directory"))
            })
            .localized_tooltip(
                "将当前文件面板目录用作补全基准目录",
                "Use the current Files directory for completion",
            )
            .disabled(!ready)
            .on_click(
                cx.listener(|view, _, window, cx| view.use_files_completion_directory(window, cx)),
            );
        let base = Button::new("completion-read-base")
            .ghost()
            .compact()
            .when(compact, |button| {
                button
                    .icon(IconName::Folder)
                    .accessibility_label(t(cx, "SFTP 起点", "SFTP base"))
            })
            .when(!compact, |button| {
                button.label(t(cx, "SFTP 起点", "SFTP base"))
            })
            .localized_tooltip(
                "显式读取 SFTP 起点并用作补全基准目录",
                "Explicitly read the SFTP base for completion",
            )
            .disabled(!ready || busy)
            .on_click(
                cx.listener(|view, _, window, cx| view.request_remote_completion(true, window, cx)),
            );
        let controls = div()
            .id("remote-completion-controls")
            .test_support()
            .flex_shrink_0()
            .px_2()
            .bg(rgb(visual.surface))
            .border_t_1()
            .border_color(rgb(visual.border))
            .flex()
            .gap_1();
        if compact {
            // Only presentation changes: query handlers and persistent input entities
            // are shared with the spacious layout, and no query starts on resize.
            return controls
                .items_center()
                .child(directory)
                .child(complete)
                .child(files)
                .child(base)
                .into_any_element();
        }
        controls
            .py_1()
            .flex_col()
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
                            .text_color(rgb(visual.muted))
                            .text_ellipsis()
                            .child(t(
                                cx,
                                "补全基准目录 · 独立于终端工作目录",
                                "Completion directory · separate from terminal cwd",
                            )),
                    )
                    .child(complete),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .child(directory)
                    .child(files)
                    .child(base),
            )
            .into_any_element()
    }

    pub(in crate::workspace) fn remote_completion_list(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
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
            .bg(rgb(visual.surface))
            .border_t_1()
            .border_color(rgb(visual.border))
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
                            .text_color(rgb(visual.muted))
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
                    .text_color(rgb(visual.muted))
                    .child(format!("{} · {host}", t(cx, "查询目标", "Query target"))),
            );
        }
        if state.ticket.as_ref().is_some_and(|ticket| ticket.commands) {
            content=content.child(div().px_2().pb_1().text_xs().text_color(rgb(visual.muted))
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
            content = content.child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(format!(
                        "{} · {} {}",
                        t(
                            cx,
                            "结果受限；可缩短查询范围",
                            "Limited results; narrow the query"
                        ),
                        state.skipped,
                        t(cx, "项已跳过", "entries skipped")
                    )),
            );
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
                    .when(index == state.selected, |row| row.bg(rgb(visual.selected)))
                    .child(
                        div()
                            .w(px(65.))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(rgb(visual.muted))
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
