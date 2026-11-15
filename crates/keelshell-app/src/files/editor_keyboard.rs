//! Scoped draft shortcuts only reveal existing review and search surfaces.
use super::*;
use crate::i18n::LocalizedTooltipExt;

actions!(files_editor, [ReviewEditorSave, FindEditorText]);

pub(super) const SAVE_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-s"
} else {
    "ctrl-s"
};
pub(super) const FIND_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-f"
} else {
    "ctrl-f"
};
pub(super) const SAVE_TOOLTIP_ZH: &str = if cfg!(target_os = "macos") {
    "草稿区域内按 Cmd+S 展开完整保存审核；仍须单独确认，重复按键不会保存。"
} else {
    "草稿区域内按 Ctrl+S 展开完整保存审核；仍须单独确认，重复按键不会保存。"
};
pub(super) const SAVE_TOOLTIP_EN: &str = if cfg!(target_os = "macos") {
    "Cmd+S in the draft area opens complete save review. Confirm separately; repeating the shortcut never saves."
} else {
    "Ctrl+S in the draft area opens complete save review. Confirm separately; repeating the shortcut never saves."
};
const FIND_TOOLTIP_ZH: &str = if cfg!(target_os = "macos") {
    "草稿区域内按 Cmd+F 展开草稿并聚焦查找框；只查找本地草稿。"
} else {
    "草稿区域内按 Ctrl+F 展开草稿并聚焦查找框；只查找本地草稿。"
};
const FIND_TOOLTIP_EN: &str = if cfg!(target_os = "macos") {
    "Cmd+F in the draft area expands the draft and focuses Find. Search stays in the local draft."
} else {
    "Ctrl+F in the draft area expands the draft and focuses Find. Search stays in the local draft."
};

struct BindingsInstalled;
impl Global for BindingsInstalled {}

pub(super) fn bind_keys(cx: &mut App) {
    if cx.has_global::<BindingsInstalled>() {
        return;
    }
    // Match the nested Input at its own depth as well: Kit has an Input search
    // binding, while KeelShell deliberately supplies the sole search surface.
    for context in ["KeelShellFileEditor", "KeelShellFileEditor > Input"] {
        cx.bind_keys([
            KeyBinding::new(SAVE_KEY, ReviewEditorSave, Some(context)),
            KeyBinding::new(FIND_KEY, FindEditorText, Some(context)),
        ]);
    }
    cx.set_global(BindingsInstalled);
}

#[derive(Clone, PartialEq, Eq)]
struct EditorBinding {
    session: uuid::Uuid,
    connection: Option<uuid::Uuid>,
    path: Option<String>,
    surface: Option<review::EditorSurface>,
}

impl FilesPanel {
    fn editor_binding(&self) -> EditorBinding {
        EditorBinding {
            session: self.session_token,
            connection: self.connection_id,
            path: self.editing.as_ref().map(|(path, _)| path.clone()),
            surface: self.editor_surface,
        }
    }

    fn editor_shortcuts_available(&self) -> bool {
        !self.suspended
            && self.session.is_some()
            && !self.busy
            && self.operation_id.is_none()
            && self.pending.is_none()
            && self.editing.is_some()
            && !self.queue_details_visible
            && !self.confirmation_expanded
            && matches!(
                self.editor_surface,
                None | Some(review::EditorSurface::Draft)
            )
    }

    fn editor_binding_is_current(&self, binding: &EditorBinding) -> bool {
        self.editor_shortcuts_available()
            && binding.session == self.session_token
            && binding.connection == self.connection_id
            && binding.path.as_deref() == self.editing.as_ref().map(|(path, _)| path.as_str())
            && binding.surface == self.editor_surface
    }

    pub(super) fn editor_keyboard_scope(
        &self,
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let save_binding = self.editor_binding();
        let find_binding = save_binding.clone();
        element
            .key_context("KeelShellFileEditor")
            .track_focus(&self.editor_keyboard_focus)
            .on_action(cx.listener(move |panel, _: &ReviewEditorSave, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                if !panel.editor_keyboard_focus.contains_focused(window, cx)
                    || !panel.editor_binding_is_current(&save_binding)
                {
                    return;
                }
                // This is the same proposal builder as the save button. Never
                // dispatch the confirmation action, including before repaint.
                panel.request_save(cx);
                if matches!(&panel.pending, Some((_, Operation::Save { .. }))) {
                    panel.expand_confirmation(window, cx);
                }
            }))
            .on_action(cx.listener(move |panel, _: &FindEditorText, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                if panel.editor_keyboard_focus.contains_focused(window, cx)
                    && panel.editor_binding_is_current(&find_binding)
                {
                    panel.open_editor_search(window, cx);
                }
            }))
    }

    fn open_editor_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editor_shortcuts_available() {
            return;
        }
        self.focus_editor(review::EditorSurface::Draft, window, cx);
        self.editor_find.read(cx).focus_handle(cx).focus(window, cx);
    }

    pub(super) fn editor_find_button(&self, cx: &mut Context<Self>) -> Button {
        let binding = self.editor_binding();
        Button::new("focus-file-search")
            .ghost()
            .compact()
            .icon(IconName::Search)
            .disabled(!self.editor_shortcuts_available())
            .label(t(cx, "查找", "Find"))
            .localized_tooltip(FIND_TOOLTIP_ZH, FIND_TOOLTIP_EN)
            .on_click(cx.listener(move |panel, _, window, cx| {
                if panel.editor_binding_is_current(&binding) {
                    panel.open_editor_search(window, cx);
                }
            }))
    }
}
