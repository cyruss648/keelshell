//! File-table keys operate on native paths and the exact painted listing.
use super::local_catalog::{LocalEntryKind, LocalListing};
use super::selection::CursorMove;
use super::*;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BrowserSide {
    Local,
    Remote,
}

impl BrowserSide {
    fn index(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Remote => 1,
        }
    }
}

enum Listing {
    Local {
        listing: Option<LocalListing>,
        sort: browser::BrowserSort,
        hidden: bool,
    },
    Remote {
        directory: Option<String>,
        entries: Vec<RemoteEntry>,
        sort: browser::BrowserSort,
        hidden: bool,
    },
}

struct Binding {
    session: uuid::Uuid,
    connection: Option<uuid::Uuid>,
    listing: Listing,
}

pub(super) struct RenameIntent {
    binding: Binding,
    source: RemoteEntry,
}

#[derive(Clone, Copy)]
enum Command {
    Move(CursorMove, bool),
    Page(bool, bool),
    Toggle,
    All,
    Clear,
    Open,
    Parent,
    Refresh,
    Rename,
    Delete,
}

fn command(key: &Keystroke) -> Option<Command> {
    let modifiers = key.modifiers;
    let primary = if cfg!(target_os = "macos") {
        modifiers.platform && !modifiers.control
    } else {
        modifiers.control && !modifiers.platform
    };
    if primary && !modifiers.alt && !modifiers.shift {
        return match key.key.as_str() {
            "a" => Some(Command::All),
            "backspace" if cfg!(target_os = "macos") => Some(Command::Delete),
            "up" if cfg!(target_os = "macos") => Some(Command::Parent),
            "down" if cfg!(target_os = "macos") => Some(Command::Open),
            _ => None,
        };
    }
    if modifiers.alt && !modifiers.control && !modifiers.platform && !modifiers.shift {
        return (key.key == "up").then_some(Command::Parent);
    }
    if modifiers.alt || modifiers.control || modifiers.platform {
        return None;
    }
    match key.key.as_str() {
        "up" => Some(Command::Move(CursorMove::Previous, modifiers.shift)),
        "down" => Some(Command::Move(CursorMove::Next, modifiers.shift)),
        "home" => Some(Command::Move(CursorMove::First, modifiers.shift)),
        "end" => Some(Command::Move(CursorMove::Last, modifiers.shift)),
        "pageup" => Some(Command::Page(false, modifiers.shift)),
        "pagedown" => Some(Command::Page(true, modifiers.shift)),
        _ if modifiers.shift => None,
        "space" => Some(Command::Toggle),
        "escape" => Some(Command::Clear),
        "enter" => Some(Command::Open),
        "backspace" => Some(Command::Parent),
        "f5" => Some(Command::Refresh),
        "f2" => Some(Command::Rename),
        "delete" => Some(Command::Delete),
        _ => None,
    }
}

fn same_entry(left: &RemoteEntry, right: &RemoteEntry) -> bool {
    left.name == right.name
        && left.path == right.path
        && left.is_directory == right.is_directory
        && left.is_symlink == right.is_symlink
        && left.size == right.size
        && left.permissions == right.permissions
        && left.modified == right.modified
}

impl FilesPanel {
    pub(super) fn browser_focus(&self, side: BrowserSide) -> &FocusHandle {
        &self.browser_keyboard_focus[side.index()]
    }

    pub(super) fn browser_scroll_handle(&self, side: BrowserSide) -> &ScrollHandle {
        &self.browser_scroll[side.index()]
    }

    fn browser_available(&self, side: BrowserSide) -> bool {
        !self.suspended
            && self.session.is_some()
            && self.pending.is_none()
            && !self.confirmation_expanded
            && !self.queue_details_visible
            && self.editor_surface.is_none()
            && self.merge_review.is_none()
            && match side {
                BrowserSide::Remote => self.directory.is_some(),
                BrowserSide::Local => {
                    self.local_browser.listing.is_some()
                        && !self.local_browser.reading()
                        && !self.local_browser.choosing()
                }
            }
    }

    fn browser_binding(&self, side: BrowserSide) -> Binding {
        Binding {
            session: self.session_token,
            connection: self.connection_id,
            listing: match side {
                BrowserSide::Local => Listing::Local {
                    listing: self.local_browser.listing.clone(),
                    sort: self.local_browser.sort,
                    hidden: self.local_browser.show_hidden,
                },
                BrowserSide::Remote => Listing::Remote {
                    directory: self.directory.clone(),
                    entries: self.entries.clone(),
                    sort: self.remote_sort,
                    hidden: self.remote_show_hidden,
                },
            },
        }
    }

    fn browser_binding_is_current(&self, side: BrowserSide, binding: &Binding) -> bool {
        if !self.browser_available(side)
            || binding.session != self.session_token
            || binding.connection != self.connection_id
        {
            return false;
        }
        match (&binding.listing, side) {
            (
                Listing::Local {
                    listing,
                    sort,
                    hidden,
                },
                BrowserSide::Local,
            ) => {
                listing == &self.local_browser.listing
                    && *sort == self.local_browser.sort
                    && *hidden == self.local_browser.show_hidden
            }
            (
                Listing::Remote {
                    directory,
                    entries,
                    sort,
                    hidden,
                },
                BrowserSide::Remote,
            ) => {
                directory == &self.directory
                    && *sort == self.remote_sort
                    && *hidden == self.remote_show_hidden
                    && entries.len() == self.entries.len()
                    && entries
                        .iter()
                        .zip(&self.entries)
                        .all(|(a, b)| same_entry(a, b))
            }
            _ => false,
        }
    }

    pub(super) fn browser_keyboard_scope(
        &self,
        element: Stateful<Div>,
        side: BrowserSide,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let binding = Rc::new(self.browser_binding(side));
        let click_binding = binding.clone();
        let visual = crate::design::palette(cx);
        // GPUI clamps scroll ranges to child extents, excluding the scroller's
        // own border. Keep the focus frame outside it so the first/last row
        // can be fully visible rather than clipped by a border-width pixel.
        div()
            .id(match side {
                BrowserSide::Local => "local-files",
                BrowserSide::Remote => "remote-files",
            })
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .track_focus(self.browser_focus(side))
            .key_context("KeelShellFileBrowser")
            .role(accesskit::Role::ListBox)
            .aria_label(match side {
                BrowserSide::Local => t(cx, "本地文件列表", "Local file list"),
                BrowserSide::Remote => t(cx, "远程文件列表", "Remote file list"),
            })
            .aria_description(t(
                cx,
                "方向键选择，Shift 连续选择，Page Up/Down 翻页，Home/End 首尾，空格切换选择，Enter 打开或使用，Escape 清空选择；最多32项。",
                "Arrow keys select, Shift extends, Page Up/Down move a page, Home/End move to an edge, Space toggles, Enter opens or uses, Escape clears; at most 32 entries.",
            ))
            .border_1()
            .border_color(rgb(visual.border))
            .focus(|style| style.border_color(rgb(visual.accent)))
            .on_click(cx.listener(move |panel, _, window, cx| {
                if panel.browser_binding_is_current(side, &click_binding) {
                    panel.browser_focus(side).focus(window, cx);
                }
            }))
            .child(element.test_support())
            .on_key_down(cx.listener(move |panel, event: &KeyDownEvent, window, cx| {
                // Child buttons and inputs retain their native key handling.
                // Unlike a context-wide key binding, this cannot steal Enter
                // from a focused checkbox or an Open button.
                if !panel.browser_focus(side).is_focused(window) {
                    return;
                }
                let Some(command) = command(&event.keystroke) else {
                    return;
                };
                if side == BrowserSide::Local
                    && matches!(command, Command::Rename | Command::Delete)
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                if panel.browser_binding_is_current(side, &binding) {
                    panel.browser_command(side, command, window, cx);
                }
            }))
    }

    fn browser_command(
        &mut self,
        side: BrowserSide,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            command,
            Command::Move(..) | Command::Page(..) | Command::Toggle | Command::All | Command::Clear
        ) {
            self.browser_select(side, command, window, cx);
            return;
        }
        if self.busy || self.operation_id.is_some() {
            return;
        }
        match (side, command) {
            (BrowserSide::Remote, Command::Open) => {
                if let Some(entry) = self.selected.clone() {
                    if entry.is_directory {
                        self.run(Operation::List(entry.path), window, cx);
                    } else {
                        self.request_read(entry, window, cx);
                    }
                }
            }
            (BrowserSide::Local, Command::Open) => {
                if let Some(entry) = self.local_browser.selected_entry().cloned() {
                    match entry.kind {
                        LocalEntryKind::Directory => self.browse_local(entry.path, window, cx),
                        LocalEntryKind::File => self.use_local_path(entry.path, window, cx),
                        LocalEntryKind::Symlink | LocalEntryKind::Other => {}
                    }
                }
            }
            (BrowserSide::Remote, Command::Parent | Command::Refresh) => {
                if let Some(directory) = &self.directory {
                    let path = if matches!(command, Command::Parent) {
                        format!("{}/..", directory.trim_end_matches('/'))
                    } else {
                        directory.clone()
                    };
                    self.run(Operation::List(path), window, cx);
                }
            }
            (BrowserSide::Local, Command::Parent) => self.parent_local_folder(window, cx),
            (BrowserSide::Local, Command::Refresh) => {
                if let Some(listing) = &self.local_browser.listing {
                    self.browse_local(listing.directory.clone(), window, cx);
                }
            }
            (BrowserSide::Remote, Command::Rename) => {
                if let Some(entry) = self.selected.clone() {
                    let name = entry.name.clone();
                    self.tool_section = Some(action_view::ToolSection::File);
                    self.tools_scroll.set_offset(point(px(0.), px(0.)));
                    self.name
                        .update(cx, |input, cx| input.set_value(name, window, cx));
                    self.browser_rename_intent = Some(RenameIntent {
                        binding: self.browser_binding(BrowserSide::Remote),
                        source: entry,
                    });
                    self.name.read(cx).focus_handle(cx).focus(window, cx);
                    cx.notify();
                }
            }
            (BrowserSide::Remote, Command::Delete) => {
                if let Some(entry) = self.selected.clone() {
                    self.confirm(
                        Message::new(
                            format!("永久删除 {}？", entry.path),
                            format!("Delete {} permanently?", entry.path),
                        ),
                        Operation::Delete(entry),
                        cx,
                    );
                    self.expand_confirmation(window, cx);
                }
            }
            _ => {}
        }
    }

    pub(super) fn review_keyboard_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(intent) = self.browser_rename_intent.take() else {
            return;
        };
        if self.busy
            || self.operation_id.is_some()
            || self.tool_section != Some(action_view::ToolSection::File)
            || !self.name.read(cx).focus_handle(cx).is_focused(window)
            || !self.browser_binding_is_current(BrowserSide::Remote, &intent.binding)
            || self
                .selected
                .as_ref()
                .is_none_or(|entry| !same_entry(entry, &intent.source))
        {
            return;
        }
        let Some(destination) = self.new_remote_path(cx) else {
            self.browser_rename_intent = Some(intent);
            cx.notify();
            return;
        };
        let source = intent.source.path;
        self.confirm(
            Message::new(
                format!("将 {source} 重命名为 {destination}？"),
                format!("Rename {source} to {destination}?"),
            ),
            Operation::Rename(source, destination),
            cx,
        );
        self.expand_confirmation(window, cx);
    }

    fn browser_select(
        &mut self,
        side: BrowserSide,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page_rows = ((self.browser_scroll_handle(side).bounds().size.height / px(28.)).floor()
            as usize)
            .max(1);
        let success = match side {
            BrowserSide::Remote => {
                let visible: Vec<_> = browser::remote_indices(
                    &self.entries,
                    self.remote_show_hidden,
                    self.remote_sort,
                )
                .into_iter()
                .map(|index| self.entries[index].path.clone())
                .collect();
                let success =
                    apply_selection(&mut self.remote_selection, &visible, command, page_rows);
                if success {
                    self.selected = self
                        .remote_selection
                        .single()
                        .and_then(|path| self.entries.iter().find(|entry| &entry.path == path))
                        .cloned();
                    if let Some(mode) = self.selected.as_ref().and_then(|entry| entry.permissions) {
                        self.mode.update(cx, |input, cx| {
                            input.set_value(format!("{:04o}", mode & 0o7777), window, cx)
                        });
                    }
                    if let Some(index) = self
                        .remote_selection
                        .cursor()
                        .and_then(|path| visible.iter().position(|item| item == path))
                    {
                        self.browser_scroll[side.index()].scroll_to_item(index);
                    }
                }
                success
            }
            BrowserSide::Local => {
                let Some(listing) = &self.local_browser.listing else {
                    return;
                };
                let visible: Vec<_> = browser::local_indices(
                    &listing.entries,
                    self.local_browser.show_hidden,
                    self.local_browser.sort,
                )
                .into_iter()
                .map(|index| listing.entries[index].path.clone())
                .collect();
                let success = apply_selection(
                    &mut self.local_browser.selection,
                    &visible,
                    command,
                    page_rows,
                );
                if success {
                    self.local_browser.selected = self.local_browser.selection.single().cloned();
                    if let Some(index) = self
                        .local_browser
                        .selection
                        .cursor()
                        .and_then(|path| visible.iter().position(|item| item == path))
                    {
                        self.browser_scroll[side.index()].scroll_to_item(index);
                    }
                }
                success
            }
        };
        if success {
            self.browser_rename_intent = None;
            self.withdraw_browser_review();
        } else {
            self.status = Message::new(
                "最多选择32项；已保留原选区。",
                "Select at most 32 entries; the previous selection is retained.",
            );
        }
        cx.notify();
    }
}

fn apply_selection<T: Ord + Clone>(
    selection: &mut selection::Selection<T>,
    visible: &[T],
    command: Command,
    page_rows: usize,
) -> bool {
    match command {
        Command::Move(_, _) if visible.is_empty() => true,
        Command::Move(direction, extend) => {
            let extend = extend && selection.cursor().is_some();
            selection.move_cursor(direction, visible, extend)
        }
        Command::Page(down, extend) => {
            let Some(last) = visible.len().checked_sub(1) else {
                return true;
            };
            let current = selection
                .cursor()
                .and_then(|path| visible.iter().position(|item| item == path));
            let destination = if down {
                current.map_or(0, |index| index.saturating_add(page_rows).min(last))
            } else {
                current.map_or(last, |index| index.saturating_sub(page_rows))
            };
            let extend = extend && selection.cursor().is_some();
            selection.click(&visible[destination], visible, false, extend)
        }
        Command::All => selection.select_all(visible),
        Command::Clear => {
            selection.clear();
            true
        }
        Command::Toggle => {
            let Some(path) = selection.cursor().or_else(|| visible.first()).cloned() else {
                return true;
            };
            selection.click(&path, visible, true, false)
        }
        _ => false,
    }
}
