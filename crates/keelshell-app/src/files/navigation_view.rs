//! Local and remote paths remain available while a pending review is shown.
use super::*;
use crate::i18n::LocalizedTooltipExt;

impl FilesPanel {
    fn remote_directory_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("parent-files")
                    .disabled(self.suspended)
                    .ghost()
                    .small()
                    .size(px(24.))
                    .compact()
                    .icon(IconName::ArrowUp)
                    .accessibility_label(t(cx, "远程上级目录", "Remote parent folder"))
                    .localized_tooltip("远程上级目录", "Remote parent folder")
                    .on_click(cx.listener(|view, _, window, cx| {
                        if let Some(directory) = &view.directory {
                            view.run(
                                Operation::List(format!("{}/..", directory.trim_end_matches('/'))),
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
                    .small()
                    .size(px(24.))
                    .compact()
                    .icon(IconName::RefreshCw)
                    .accessibility_label(t(
                        cx,
                        "刷新远程目录草稿",
                        "Browse the remote folder draft",
                    ))
                    .localized_tooltip("刷新远程目录草稿", "Browse the remote folder draft")
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.run(
                            Operation::List(view.path.read(cx).value().to_string()),
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                Button::new("toggle-remote-hidden")
                    .disabled(self.suspended)
                    .ghost()
                    .small()
                    .size(px(24.))
                    .compact()
                    .selected(self.remote_show_hidden)
                    .toggled(self.remote_show_hidden)
                    .icon(if self.remote_show_hidden {
                        IconName::Eye
                    } else {
                        IconName::EyeOff
                    })
                    .accessibility_label(t(cx, "切换远程隐藏文件", "Toggle remote hidden files"))
                    .localized_tooltip("切换远程隐藏文件", "Toggle remote hidden files")
                    .on_click(cx.listener(|view, _, _, cx| view.toggle_remote_hidden(cx))),
            )
            .into_any_element()
    }

    pub(super) fn folder_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let reviewing = self.pending.is_some() || self.suspended;
        let mut remote = div().flex_1().min_w_0().h_full().flex().flex_col().px_1();
        if !reviewing {
            remote = remote.child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(visual.muted))
                            .child(t(cx, "远程文件", "Remote files")),
                    )
                    .child(self.remote_directory_actions(cx)),
            );
        }
        remote = remote.child(
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .flex()
                .items_center()
                .child(
                    div()
                        .id("remote-folder-path")
                        .flex_1()
                        .min_w_0()
                        .child(
                            Input::new(&self.path)
                                .small()
                                .rounded(px(crate::design::components(cx).control_radius)),
                        )
                        .test_support(),
                )
                .when(reviewing, |row| {
                    row.child(self.remote_directory_actions(cx))
                }),
        );
        div()
            .h(px(if reviewing { 29. } else { 61. }))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(self.local_navigation(cx))
            .child(remote)
            .into_any_element()
    }
}
