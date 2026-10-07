//! Compact local controls share the remote browser's fixed vertical budget.

use super::browser::SortColumn;
use super::local_catalog::LocalEntryKind;
use super::*;
use crate::i18n::LocalizedTooltipExt;

impl FilesPanel {
    pub(super) fn local_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("local-folder-navigation")
            .w(relative(0.32))
            .min_w(px(168.))
            .max_w(px(290.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px_1()
            .border_r_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.local_browser.path).small().rounded(px(6.))),
            )
            .child(
                Button::new("choose-local-folder")
                    .disabled(self.suspended || self.local_browser.choosing())
                    .ghost()
                    .compact()
                    .icon(IconName::FolderOpen)
                    .accessibility_label(t(cx, "选择本地文件夹", "Choose local folder"))
                    .localized_tooltip("选择本地文件夹", "Choose local folder")
                    .on_click(
                        cx.listener(|panel, _, window, cx| panel.choose_local_folder(window, cx)),
                    ),
            )
            .child(
                Button::new("parent-local-folder")
                    .disabled(self.suspended || self.local_browser.listing.is_none())
                    .ghost()
                    .compact()
                    .icon(IconName::ArrowUp)
                    .accessibility_label(t(cx, "本地上级目录", "Local parent folder"))
                    .localized_tooltip("本地上级目录", "Local parent folder")
                    .on_click(
                        cx.listener(|panel, _, window, cx| panel.parent_local_folder(window, cx)),
                    ),
            )
            .child(
                Button::new("refresh-local-folder")
                    .disabled(self.suspended)
                    .ghost()
                    .compact()
                    .icon(IconName::RefreshCw)
                    .accessibility_label(t(cx, "刷新本地目录草稿", "Browse the local folder draft"))
                    .localized_tooltip("刷新本地目录草稿", "Browse the local folder draft")
                    .on_click(
                        cx.listener(|panel, _, window, cx| panel.refresh_local_folder(window, cx)),
                    ),
            )
            .child(
                Button::new("toggle-local-hidden")
                    .disabled(self.suspended)
                    .ghost()
                    .compact()
                    .icon(if self.local_browser.show_hidden {
                        IconName::Eye
                    } else {
                        IconName::EyeOff
                    })
                    .accessibility_label(t(cx, "切换本地隐藏文件", "Toggle local hidden files"))
                    .localized_tooltip("切换本地隐藏文件", "Toggle local hidden files")
                    .on_click(cx.listener(|panel, _, _, cx| panel.toggle_local_hidden(cx))),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn local_browser_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut list = div()
            .id("local-files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .test_support();
        if let Some(listing) = &self.local_browser.listing {
            for index in browser::local_indices(
                &listing.entries,
                self.local_browser.show_hidden,
                self.local_browser.sort,
            ) {
                let entry = &listing.entries[index];
                let selected = self.local_browser.selected.as_ref() == Some(&entry.path);
                let path = entry.path.clone();
                let action_path = entry.path.clone();
                let kind = entry.kind;
                let utf8 = entry.path.to_str().is_some();
                let name = if entry.name.to_str().is_some() {
                    entry.name.to_string_lossy().into_owned()
                } else {
                    format!(
                        "{} [{}]",
                        entry.name.to_string_lossy(),
                        t(cx, "非 UTF-8", "non-UTF-8")
                    )
                };
                list = list.child(
                    div()
                        .id(("local-entry", index))
                        .h(px(28.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .px_1()
                        .gap_1()
                        .min_w_0()
                        .bg(rgb(if selected {
                            visual.selected
                        } else if index % 2 == 0 {
                            visual.surface
                        } else {
                            visual.canvas
                        }))
                        .cursor_pointer()
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            panel.local_browser.selected = Some(path.clone());
                            panel.withdraw_browser_review();
                            cx.notify();
                        }))
                        .child(match kind {
                            LocalEntryKind::Directory => IconName::Folder,
                            LocalEntryKind::File => IconName::FileText,
                            LocalEntryKind::Symlink => IconName::Link,
                            LocalEntryKind::Other => IconName::CircleAlert,
                        })
                        .child(div().flex_1().min_w_0().text_ellipsis().child(name))
                        .child(
                            div()
                                .text_color(rgb(visual.muted))
                                .text_xs()
                                .child(size_label(entry.size)),
                        )
                        .child(
                            Button::new(("use-open-local", index))
                                .ghost()
                                .compact()
                                .disabled(
                                    self.suspended
                                        || !matches!(
                                            kind,
                                            LocalEntryKind::Directory | LocalEntryKind::File
                                        )
                                        || (kind == LocalEntryKind::File && !utf8),
                                )
                                .label(match kind {
                                    LocalEntryKind::Directory => t(cx, "打开", "Open"),
                                    LocalEntryKind::File => t(cx, "使用", "Use"),
                                    LocalEntryKind::Symlink => t(cx, "链接", "Link"),
                                    LocalEntryKind::Other => t(cx, "特殊", "Other"),
                                })
                                .on_click(cx.listener(move |panel, _, window, cx| {
                                    // Navigation clears the old selection; the painted
                                    // parent row must not restore it after this action.
                                    cx.stop_propagation();
                                    if kind == LocalEntryKind::Directory {
                                        panel.browse_local(action_path.clone(), window, cx);
                                    } else if kind == LocalEntryKind::File {
                                        panel.use_local_path(action_path.clone(), window, cx);
                                    }
                                })),
                        )
                        .test_support(),
                );
            }
        }
        if self.local_browser.listing.as_ref().is_none_or(|listing| {
            browser::local_indices(
                &listing.entries,
                self.local_browser.show_hidden,
                self.local_browser.sort,
            )
            .is_empty()
        }) {
            list = list.child(
                div()
                    .id("local-browser-status")
                    .p_2()
                    .text_color(rgb(visual.muted))
                    .child(self.local_browser.status.render(cx)),
            );
        }
        div()
            .id("local-files-pane")
            .w(relative(0.32))
            .min_w(px(168.))
            .max_w(px(290.))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
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
                        Button::new("sort-local-name")
                            .ghost()
                            .compact()
                            .flex_1()
                            .min_w_0()
                            .label(format!(
                                "{}{}",
                                t(cx, "本地", "Local"),
                                self.local_browser.sort.indicator(SortColumn::Name)
                            ))
                            .accessibility_label(t(cx, "按本地名称排序", "Sort local names"))
                            .localized_tooltip("按本地名称排序", "Sort local names")
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.local_browser.sort.select(SortColumn::Name);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("sort-local-size")
                            .ghost()
                            .compact()
                            .label(format!(
                                "{}{}",
                                t(cx, "大小", "Size"),
                                self.local_browser.sort.indicator(SortColumn::Size)
                            ))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.local_browser.sort.select(SortColumn::Size);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("sort-local-modified")
                            .ghost()
                            .compact()
                            .label(format!(
                                "{}{}",
                                t(cx, "时间", "Time"),
                                self.local_browser.sort.indicator(SortColumn::Modified)
                            ))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.local_browser.sort.select(SortColumn::Modified);
                                cx.notify();
                            })),
                    ),
            )
            .child(list)
            .test_support()
            .into_any_element()
    }
}
