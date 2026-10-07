//! Only the active modal participates in layout, input dispatch and the AX tree.
use super::*;
use gpui_kit::base::{FocusTrapElement, Selectable};

#[cfg(test)]
mod tests;

/// Seeds the Kit button's public keyed focus state with a workspace-owned handle.
/// Kit controls ordinarily discard that state when a modal unmounts their parent.
#[derive(IntoElement)]
pub(super) struct RetainedModalButton {
    id: ElementId,
    button: Button,
    focus: FocusHandle,
}

impl RenderOnce for RetainedModalButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Both calls use the same element namespace. Calling the actual Kit
        // renderer directly keeps its appearance, disabled and AX semantics.
        window.use_keyed_state(self.id, cx, |_, _| self.focus);
        RenderOnce::render(self.button, window, cx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModalKind {
    KeyboardInteractive,
    HostApproval,
    Login,
    Updates,
    Vault,
    ProfileSync,
    AiSettings,
    Mcp,
    McpFileReview,
    Workflow,
    Batch,
    Archive,
    SnippetParameters,
    SnippetEditor,
    SnippetDelete,
    OpenSshImport,
    LibraryBatch,
    Destination,
    Folder,
    Connection,
    Manager,
}

impl ModalKind {
    fn title(self, cx: &App) -> &'static str {
        match self {
            Self::KeyboardInteractive | Self::HostApproval | Self::Login => {
                t(cx, "SSH 连接与认证", "SSH connection and authentication")
            }
            Self::Updates => t(cx, "关于与更新", "About and updates"),
            Self::ProfileSync => t(cx, "加密连接同步", "Encrypted profile sync"),
            Self::Vault => t(cx, "凭据库", "Credential vault"),
            Self::AiSettings => t(cx, "AI 配置", "AI configurations"),
            Self::Mcp => t(cx, "对外 MCP 授权与审阅", "External MCP grants and review"),
            Self::McpFileReview => t(cx, "远程文件修改审阅", "Remote file change review"),
            Self::Workflow => t(cx, "依赖工作流", "Dependency workflow"),
            Self::Batch => t(cx, "批量任务", "Batch tasks"),
            Self::Archive => t(cx, "上次会话草稿", "Previous session drafts"),
            Self::SnippetParameters => t(cx, "命令参数", "Command parameters"),
            Self::SnippetEditor | Self::SnippetDelete => t(cx, "命令片段", "Command snippet"),
            Self::OpenSshImport => t(cx, "导入 SSH 配置", "Import SSH configuration"),
            Self::LibraryBatch => t(cx, "连接批量审核", "Connection bulk review"),
            Self::Destination => t(cx, "选择目标文件夹", "Choose destination folder"),
            Self::Folder => t(cx, "文件夹", "Folder"),
            Self::Connection => t(cx, "SSH 连接配置", "SSH connection profile"),
            Self::Manager => t(cx, "连接管理器", "Connection manager"),
        }
    }
}

struct ModalFrame {
    kind: ModalKind,
    return_focus: Option<FocusHandle>,
}

/// Registers capture before the child paints its pointer handlers. Div capture
/// callbacks require a hovered hitbox, which an occluding modal can hide.
struct SurfaceInputBoundary {
    child: AnyElement,
    workspace: WeakEntity<Workspace>,
    painted_kind: Option<ModalKind>,
    painted_generation: u64,
    painted_challenge: Option<uuid::Uuid>,
}

impl SurfaceInputBoundary {
    fn register_pointer_guard<E: MouseEvent>(&self, window: &mut Window) {
        let workspace = self.workspace.clone();
        let kind = self.painted_kind;
        let generation = self.painted_generation;
        let challenge = self.painted_challenge;
        window.on_mouse_event(move |_: &E, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let live = workspace.update(cx, |view, cx| {
                let current = view.active_modal() == kind
                    && view.modal_scope.current == kind
                    && view.modal_scope.generation == generation
                    && view.surface_challenge() == challenge;
                if current {
                    view.remember_surface_focus(window, cx);
                }
                current
            });
            if !matches!(live, Ok(true)) {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
    }
}

impl IntoElement for SurfaceInputBoundary {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SurfaceInputBoundary {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.register_pointer_guard::<MouseDownEvent>(window);
        self.register_pointer_guard::<MouseUpEvent>(window);
        self.child.paint(window, cx);
    }
}

#[derive(Default)]
pub(super) struct ModalScope {
    pub(super) current: Option<ModalKind>,
    current_challenge: Option<uuid::Uuid>,
    frames: Vec<ModalFrame>,
    last_focus: Option<FocusHandle>,
    generation: u64,
}

impl Workspace {
    fn surface_challenge(&self) -> Option<uuid::Uuid> {
        self.keyboard_interactive
            .as_ref()
            .map(|prompt| prompt.identity)
            .or_else(|| (self.mcp.show).then_some(self.mcp.reviewing).flatten())
    }
    pub(super) fn protect_surface_input(
        &self,
        child: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        SurfaceInputBoundary {
            child,
            workspace: cx.entity().downgrade(),
            painted_kind: self.modal_scope.current,
            painted_generation: self.modal_scope.generation,
            painted_challenge: self.surface_challenge(),
        }
        .into_any_element()
    }

    pub(super) fn retain_modal_button(
        &self,
        id: impl Into<ElementId>,
        button: Button,
        cx: &App,
    ) -> RetainedModalButton {
        let id = id.into();
        let mut cache = self.modal_button_focus.borrow_mut();
        let focus = cache
            .get(&id)
            .and_then(WeakFocusHandle::upgrade)
            .unwrap_or_else(|| cx.focus_handle());
        cache.insert(id.clone(), focus.downgrade());
        RetainedModalButton { id, button, focus }
    }

    pub(super) fn active_modal(&self) -> Option<ModalKind> {
        // Authentication can arrive asynchronously. It must interrupt a settings
        // draft rather than leave a security decision hidden behind that draft.
        [
            (
                self.keyboard_interactive.is_some(),
                ModalKind::KeyboardInteractive,
            ),
            (self.login.is_some(), ModalKind::Login),
            (self.host_approval.is_some(), ModalKind::HostApproval),
            (self.update_panel.is_some(), ModalKind::Updates),
            (self.vault_settings.is_some(), ModalKind::Vault),
            (self.profile_sync.is_some(), ModalKind::ProfileSync),
            (self.ai_settings.is_some(), ModalKind::AiSettings),
            (
                self.mcp.show && self.mcp.reviewing.is_some(),
                ModalKind::McpFileReview,
            ),
            (self.mcp.show, ModalKind::Mcp),
            (self.show_workflow, ModalKind::Workflow),
            (self.show_batch, ModalKind::Batch),
            (self.discard_archive.is_some(), ModalKind::Archive),
            (
                self.snippet_parameters.is_some(),
                ModalKind::SnippetParameters,
            ),
            (self.snippet_editor.is_some(), ModalKind::SnippetEditor),
            (self.snippet_delete.is_some(), ModalKind::SnippetDelete),
            (self.openssh_review.is_some(), ModalKind::OpenSshImport),
            (self.library_batch_prompt.is_some(), ModalKind::LibraryBatch),
            (self.destination_prompt.is_some(), ModalKind::Destination),
            (self.folder_form.is_some(), ModalKind::Folder),
            (self.form.is_some(), ModalKind::Connection),
            (self.show_connections, ModalKind::Manager),
        ]
        .into_iter()
        .find_map(|(present, kind)| present.then_some(kind))
    }

    pub(super) fn remember_surface_focus(&mut self, window: &Window, cx: &App) {
        self.modal_scope.last_focus = window.focused(cx);
    }

    pub(super) fn capture_surface_input(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.active_modal() != self.modal_scope.current {
            // An asynchronous transition can invalidate the previous frame
            // before the replacement is painted. Do not activate its controls.
            cx.stop_propagation();
            return;
        }
        self.remember_surface_focus(window, cx);
    }

    pub(super) fn synchronize_modal_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Only mounted controls and actual return targets retain strong handles.
        // Repeated creation/deletion of snippets cannot grow this cache forever.
        self.modal_button_focus
            .borrow_mut()
            .retain(|_, focus| focus.upgrade().is_some());
        let next = self.active_modal();
        let next_challenge = self.surface_challenge();
        if self.modal_scope.current == next && self.modal_scope.current_challenge != next_challenge
        {
            self.modal_scope.current_challenge = next_challenge;
            self.modal_scope.generation = self.modal_scope.generation.wrapping_add(1);
            // A replacement task must not retain a held activation key's focus
            // generation, even when its kind, route and input count recur.
            self.modal_focus.focus(window, cx);
            if let Some(prompt) = &self.keyboard_interactive {
                prompt.focus(window, cx);
            }
        }
        if self.modal_scope.current == next {
            if next.is_none() || self.modal_focus.contains_focused(window, cx) {
                self.remember_surface_focus(window, cx);
            } else {
                let generation = self.modal_scope.generation;
                cx.on_next_frame(window, move |view, window, cx| {
                    if view.modal_scope.generation == generation
                        && view.active_modal() == next
                        && !view.modal_focus.contains_focused(window, cx)
                    {
                        view.modal_focus.focus(window, cx);
                        window.focus_next(cx);
                    }
                });
            }
            return;
        }
        let restore = match next {
            None => {
                let restore = self
                    .modal_scope
                    .frames
                    .first()
                    .and_then(|frame| frame.return_focus.clone());
                self.modal_scope.frames.clear();
                restore
            }
            Some(kind) => {
                if let Some(index) = self
                    .modal_scope
                    .frames
                    .iter()
                    .position(|frame| frame.kind == kind)
                {
                    let restore = self
                        .modal_scope
                        .frames
                        .get(index + 1)
                        .and_then(|frame| frame.return_focus.clone());
                    self.modal_scope.frames.truncate(index + 1);
                    restore
                } else {
                    self.modal_scope.frames.push(ModalFrame {
                        kind,
                        return_focus: self.modal_scope.last_focus.clone(),
                    });
                    None
                }
            }
        };
        self.modal_scope.current = next;
        self.modal_scope.current_challenge = next_challenge;
        self.modal_scope.generation = self.modal_scope.generation.wrapping_add(1);
        let generation = self.modal_scope.generation;
        // Restored controls are not mounted until this frame finishes. A stale
        // callback must not steal focus from a newer asynchronous auth challenge.
        cx.on_next_frame(window, move |view, window, cx| {
            if view.modal_scope.generation != generation || view.active_modal() != next {
                return;
            }
            if let Some(focus) = restore {
                focus.focus(window, cx);
            }
            if next.is_some() && !view.modal_focus.contains_focused(window, cx) {
                view.modal_focus.focus(window, cx);
                window.focus_next(cx);
            } else if next.is_none() && !view.surface_focus.contains_focused(window, cx) {
                view.focus_current_surface(window, cx);
            }
            view.remember_surface_focus(window, cx);
        });
    }

    pub(super) fn close_active_modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(kind) = self.active_modal() else {
            return false;
        };
        match kind {
            ModalKind::KeyboardInteractive | ModalKind::HostApproval => {
                self.cancel_connect_route(window, cx)
            }
            ModalKind::Login => self.cancel_login(window, cx),
            ModalKind::Updates => {
                self.update_panel = None;
            }
            ModalKind::ProfileSync => {
                if let Some(panel) = self.profile_sync.clone() {
                    panel.update(cx, |panel, cx| panel.close(window, cx));
                }
            }
            ModalKind::Vault => {
                if let Some(panel) = self.vault_settings.clone() {
                    panel.update(cx, |panel, cx| panel.close(window, cx));
                }
            }
            ModalKind::AiSettings if !self.saving => {
                self.ai_settings = None;
                self.ai_settings_subscription = None;
            }
            ModalKind::McpFileReview => self.mcp.reviewing = None,
            ModalKind::Mcp => self.mcp.show = false,
            ModalKind::Workflow => self.show_workflow = false,
            ModalKind::Batch => self.show_batch = false,
            ModalKind::Archive => self.discard_archive = None,
            ModalKind::SnippetParameters | ModalKind::SnippetEditor | ModalKind::SnippetDelete => {
                self.close_snippet_modal(window, cx)
            }
            ModalKind::OpenSshImport => self.cancel_openssh_import(window, cx),
            ModalKind::LibraryBatch if !self.saving => self.close_library_batch(window, cx),
            ModalKind::Destination if !self.saving => self.close_destination(window, cx),
            ModalKind::Folder if !self.saving => self.close_folder_form(window, cx),
            ModalKind::Connection if !self.saving => self.form = None,
            ModalKind::Manager => self.show_connections = false,
            _ => {}
        }
        self.focus_current_surface(window, cx);
        cx.notify();
        true
    }

    pub(super) fn render_active_modal(
        &self,
        kind: ModalKind,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match kind {
            ModalKind::KeyboardInteractive | ModalKind::HostApproval | ModalKind::Login => {
                self.authentication_modal(cx)
            }
            ModalKind::Updates => {
                self.panel_modal(self.update_panel.clone(), px(760.), px(600.), cx)
            }
            ModalKind::ProfileSync => {
                self.panel_modal(self.profile_sync.clone(), px(1000.), px(680.), cx)
            }
            ModalKind::Vault => {
                self.panel_modal(self.vault_settings.clone(), px(900.), px(650.), cx)
            }
            ModalKind::AiSettings => {
                self.panel_modal(self.ai_settings.clone(), px(1080.), px(700.), cx)
            }
            ModalKind::Mcp => self.mcp_modal(cx),
            ModalKind::McpFileReview => self.mcp_file_review_modal(cx),
            ModalKind::Workflow => self.workflow_modal(cx),
            ModalKind::Batch => self.batch_modal(cx),
            ModalKind::Archive => self.archive_confirmation(cx),
            ModalKind::SnippetParameters | ModalKind::SnippetEditor | ModalKind::SnippetDelete => {
                self.snippet_modal(cx)
            }
            ModalKind::OpenSshImport => self.openssh_import_modal(cx),
            ModalKind::LibraryBatch => self.library_batch_modal(cx),
            ModalKind::Destination | ModalKind::Folder => self.library_modal(cx),
            ModalKind::Connection => self.connection_form(cx),
            ModalKind::Manager => self.connection_manager(window.viewport_size().width, cx),
        }
    }

    fn panel_modal<V: Render>(
        &self,
        panel: Option<Entity<V>>,
        width: Pixels,
        height: Pixels,
        cx: &App,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .flex()
            .items_center()
            .justify_center()
            .when_some(panel, |layer, panel| {
                layer.child(
                    div()
                        .w(width)
                        .h(height)
                        .max_w_full()
                        .max_h_full()
                        .bg(rgb(visual.surface))
                        .rounded_lg()
                        .shadow_lg()
                        .overflow_hidden()
                        .child(panel),
                )
            })
            .into_any_element()
    }

    pub(super) fn isolated_modal_surface(
        &self,
        kind: ModalKind,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let surface = div()
            .id("workspace-modal-scope")
            .test_support()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(visual.canvas))
            .text_color(rgb(visual.text))
            .text_sm()
            .font_family(".SystemUIFont")
            .role(accesskit::Role::Dialog)
            .accessibility_id("keelshell.active-modal")
            .aria_label(kind.title(cx))
            .capture_key_down(
                cx.listener(|view, _, window, cx| view.capture_surface_input(window, cx)),
            )
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && !event.keystroke.modifiers.modified()
                    && view.close_active_modal(window, cx)
                {
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(Self::open_connections))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_assistant))
            .child(self.modal_chrome(kind, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_active_modal(kind, window, cx)),
            )
            .focus_trap("workspace-modal-trap", &self.modal_focus)
            .into_any_element();
        self.protect_surface_input(surface, cx)
    }

    fn modal_chrome(&self, kind: ModalKind, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let disabled = self.saving || matches!(kind, ModalKind::Vault | ModalKind::ProfileSync);
        let mut themes = div().flex().items_center().gap_1().flex_shrink_0();
        for (id, theme, zh, en) in [
            (
                "modal-theme-system",
                keelshell_core::Theme::System,
                "跟随系统",
                "System",
            ),
            (
                "modal-theme-light",
                keelshell_core::Theme::Light,
                "浅色",
                "Light",
            ),
            (
                "modal-theme-dark",
                keelshell_core::Theme::Dark,
                "深色",
                "Dark",
            ),
        ] {
            themes =
                themes.child(
                    Button::new(id)
                        .ghost()
                        .compact()
                        .label(t(cx, zh, en))
                        .selected(self.state.settings.theme == theme)
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.select_theme(theme, window, cx)
                        }))
                        .map(|button| self.retain_modal_button(id, button, cx)),
                );
        }
        div()
            .id("modal-appearance-bar")
            .test_support()
            .min_h(px(34.))
            .flex_shrink_0()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.surface))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(visual.muted))
                    .child(kind.title(cx)),
            )
            .child(themes)
            .child(
                Button::new("modal-language")
                    .ghost()
                    .compact()
                    .label(t(cx, "English", "中文"))
                    .accessibility_label(t(cx, "切换界面语言", "Change interface language"))
                    .disabled(disabled)
                    .on_click(cx.listener(|view, _, window, cx| view.switch_language(window, cx)))
                    .map(|button| self.retain_modal_button("modal-language", button, cx)),
            )
            .into_any_element()
    }
}
