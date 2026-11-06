//! Native pane geometry; live session ownership remains in `Workspace`.

use super::*;
use crate::i18n::LocalizedTooltipExt;
use gpui_kit::base::{ElementExt, HandleEdge, ResizeHandleRenderer};
use gpui_kit::component::Selectable;
use keelshell_core::{
    LayoutPane, LayoutTab, LayoutWindow, PaneNode, PaneNodeId, SplitAxis, WorkspaceLayout,
};

#[derive(Clone, Copy)]
struct PaneResizeDrag {
    workspace: EntityId,
    node: PaneNodeId,
    generation: u64,
    input_generation: u64,
}

impl Render for PaneResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[derive(Debug, thiserror::Error)]
enum PaneSnapshotError {
    #[error("A session no longer has a stable layout identity")]
    MissingIdentity,
    #[error(transparent)]
    Invalid(#[from] keelshell_core::ValidationError),
}

impl Workspace {
    /// Reconcile legacy session owners without recreating transports or changing tab order.
    pub(super) fn sync_session_panes(&mut self) -> bool {
        let ids: Vec<_> = self.tabs.iter().map(Entity::entity_id).collect();
        let active = self.tabs.get(self.active).map(Entity::entity_id);
        let old_nodes: Vec<_> = self
            .session_panes
            .layout()
            .nodes
            .iter()
            .map(PaneNode::id)
            .collect();
        if let Err(error) = self.session_panes.reconcile(&ids, active) {
            self.status = Message::detail("无法更新面板布局", "Cannot update pane layout", error);
            return false;
        }
        let nodes: Vec<_> = self
            .session_panes
            .layout()
            .nodes
            .iter()
            .map(PaneNode::id)
            .collect();
        if old_nodes != nodes {
            self.pane_generation = self.pane_generation.wrapping_add(1);
        }
        self.pane_bounds
            .borrow_mut()
            .retain(|id, _| nodes.contains(id));
        self.pane_divider_focus
            .borrow_mut()
            .retain(|id, _| nodes.contains(id));
        self.pane_tab_ids.retain(|id, _| ids.contains(id));
        self.restored_tabs.retain(|id, _| ids.contains(id));
        for id in ids {
            self.pane_tab_ids
                .entry(id)
                .or_insert_with(uuid::Uuid::new_v4);
        }
        true
    }

    pub(super) fn admit_new_session(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.configuration_current(cx) || self.close_review.is_some() {
            return false;
        }
        if self.sync_session_panes() && self.session_panes.can_add() {
            return true;
        }
        self.status = Message::new(
            "当前面板的会话数量已达上限，请关闭会话或选择其他面板。",
            "This pane has reached its session limit. Close a session or select another pane.",
        );
        cx.notify();
        false
    }

    pub(super) fn collapse_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_modal().is_some() || self.connecting || !self.sync_session_panes() {
            return;
        }
        let pane = self.session_panes.active_pane();
        match self.session_panes.collapse(pane) {
            Ok(()) => {
                self.pane_generation = self.pane_generation.wrapping_add(1);
                self.pane_zoomed = false;
                self.active = self
                    .session_panes
                    .active_session()
                    .and_then(|id| self.tabs.iter().position(|tab| tab.entity_id() == id))
                    .unwrap_or(self.tabs.len());
                self.refresh_protocol_selection(cx);
                self.status = Message::new(
                    "已合并面板，会话继续保持。",
                    "Panes merged. Sessions remain open.",
                );
                self.focus_current_surface(window, cx);
            }
            Err(error) => {
                self.status = Message::detail("无法合并面板", "Cannot merge panes", error);
            }
        }
        cx.notify();
    }

    fn pane_snapshot(&self) -> Result<WorkspaceLayout, PaneSnapshotError> {
        let layout = self.session_panes.layout().clone();
        let panes = layout
            .leaf_ids()?
            .into_iter()
            .map(|pane| {
                let tabs = self
                    .session_panes
                    .tabs_in(pane)
                    .iter()
                    .map(|key| {
                        Ok(LayoutTab {
                            id: *self
                                .pane_tab_ids
                                .get(key)
                                .ok_or(PaneSnapshotError::MissingIdentity)?,
                            connection_id: self.restored_tabs.get(key).map_or_else(
                                || self.remote_connection_ids.get(key).copied(),
                                |tab| tab.connection_id,
                            ),
                        })
                    })
                    .collect::<Result<Vec<_>, PaneSnapshotError>>()?;
                let active_tab = self
                    .session_panes
                    .selected_in(pane)
                    .map(|key| {
                        self.pane_tab_ids
                            .get(&key)
                            .copied()
                            .ok_or(PaneSnapshotError::MissingIdentity)
                    })
                    .transpose()?;
                Ok(LayoutPane {
                    pane,
                    tabs,
                    active_tab,
                })
            })
            .collect::<Result<Vec<_>, PaneSnapshotError>>()?;
        let window = LayoutWindow {
            id: self.layout_window_id,
            layout,
            panes,
        };
        let mut snapshot = self
            .state
            .workspace_layout
            .clone()
            .unwrap_or_else(|| WorkspaceLayout {
                version: keelshell_core::WORKSPACE_LAYOUT_VERSION,
                windows: Vec::new(),
                active_window: self.layout_window_id,
            });
        // Other saved windows belong to their own scopes, even before native
        // multi-window restoration is available.
        if let Some(saved) = snapshot
            .windows
            .iter_mut()
            .find(|saved| saved.id == window.id)
        {
            *saved = window;
        } else {
            snapshot.windows.push(window);
        }
        snapshot.active_window = self.layout_window_id;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(super) fn save_pane_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_modal().is_some()
            || self.connecting
            || self.saving
            || self.state_reloading
            || !self.sync_session_panes()
        {
            return;
        }
        match self.pane_snapshot() {
            Ok(layout) => {
                let mut candidate = self.state.clone();
                candidate.workspace_layout = Some(layout);
                self.persist(candidate, AfterSave::None, window, cx);
            }
            Err(error) => {
                self.status = Message::detail("无法保存布局", "Cannot save layout", error);
                cx.notify();
            }
        }
    }

    fn pane_resize_input_current(&self, target: PaneResizeDrag, cx: &Context<Self>) -> bool {
        target.workspace == cx.entity_id()
            && target.generation == self.pane_generation
            && target.input_generation == self.pane_input_generation()
            && self.active_modal().is_none()
            && !self.connecting
            && !self.pane_zoomed
            && !self.compact_panes()
    }

    fn resize_pane(
        &mut self,
        drag: PaneResizeDrag,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.pane_resize_input_current(drag, cx) {
            return;
        }
        let Some(PaneNode::Split { axis, .. }) = self
            .session_panes
            .layout()
            .nodes
            .iter()
            .find(|node| node.id() == drag.node)
        else {
            return;
        };
        let Some(bounds) = self.pane_bounds.borrow().get(&drag.node).copied() else {
            return;
        };
        let (offset, length) = match axis {
            SplitAxis::Horizontal => (position.x - bounds.left(), bounds.size.width),
            SplitAxis::Vertical => (position.y - bounds.top(), bounds.size.height),
        };
        let length = f32::from(length);
        let offset = f32::from(offset);
        if !length.is_finite() || length <= 0. || !offset.is_finite() {
            return;
        }
        let ratio = ((offset / length) * 10_000.).round().clamp(1000., 9000.) as u16;
        if self.session_panes.set_ratio(drag.node, ratio).is_ok() {
            cx.notify();
        }
    }

    fn adjust_pane_ratio(&mut self, node: PaneNodeId, delta: i32, cx: &mut Context<Self>) {
        if self.active_modal().is_some()
            || self.connecting
            || self.pane_zoomed
            || self.compact_panes()
        {
            return;
        }
        let Some(PaneNode::Split { ratio_bps, .. }) = self
            .session_panes
            .layout()
            .nodes
            .iter()
            .find(|candidate| candidate.id() == node)
        else {
            return;
        };
        let next = (i32::from(*ratio_bps) + delta).clamp(1000, 9000) as u16;
        if self.session_panes.set_ratio(node, next).is_ok() {
            cx.notify();
        }
    }

    fn pane_minimum(&self, node: PaneNodeId) -> Size<Pixels> {
        match self
            .session_panes
            .layout()
            .nodes
            .iter()
            .find(|candidate| candidate.id() == node)
        {
            Some(PaneNode::Split {
                axis,
                first,
                second,
                ..
            }) => {
                let a = self.pane_minimum(*first);
                let b = self.pane_minimum(*second);
                match axis {
                    SplitAxis::Horizontal => size(a.width + b.width, a.height.max(b.height)),
                    SplitAxis::Vertical => size(a.width.max(b.width), a.height + b.height),
                }
            }
            _ => size(px(200.), px(96.)),
        }
    }

    pub(super) fn compact_panes(&self) -> bool {
        let available = self.pane_canvas_size.get();
        let minimum = self.pane_minimum(self.session_panes.layout().root);
        self.session_panes.layout().nodes.len() > 1
            && available.width > px(0.)
            && available.height > px(0.)
            && (available.width < minimum.width || available.height < minimum.height)
    }

    pub(super) fn pane_divider_visible(&self, id: PaneNodeId) -> bool {
        !self.pane_zoomed
            && !self.compact_panes()
            && self
                .session_panes
                .layout()
                .nodes
                .iter()
                .any(|node| matches!(node, PaneNode::Split { id: node_id, .. } if *node_id == id))
    }

    #[inline(never)]
    pub(super) fn native_pane_canvas(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let root = self.session_panes.layout().root;
        let available = self.pane_canvas_size.get();
        let compact = self.compact_panes();
        if (self.pane_zoomed || compact)
            && self
                .pane_divider_focus
                .borrow()
                .values()
                .any(|focus| focus.is_focused(window))
        {
            // A hidden splitter must neither strand keyboard navigation nor
            // transfer subsequent typing into an SSH terminal.
            self.surface_focus.focus(window, cx);
        }
        let node = if self.pane_zoomed || compact {
            self.session_panes.active_pane()
        } else {
            root
        };
        let geometry = self.pane_canvas_size.clone();
        let workspace = cx.weak_entity();
        let visual = crate::design::palette(cx);
        div().id("pane-canvas").test_support().size_full().min_w_0().min_h_0()
            .flex().flex_col().overflow_hidden()
            .on_prepaint(move |bounds, window, cx| {
                if geometry.replace(bounds.size) != bounds.size {
                    let workspace = workspace.clone();
                    window.defer(cx, move |_, cx| { let _ = workspace.update(cx, |_, cx| cx.notify()); });
                }
            })
            .on_drag_move(cx.listener(|view, event: &DragMoveEvent<PaneResizeDrag>, _, cx| {
                if let Some(drag) = event.dragged_item().downcast_ref::<PaneResizeDrag>() {
                    view.resize_pane(*drag, event.event.position, cx);
                }
            }))
            .when(compact, |canvas| {
                canvas.child(div().id("compact-pane-notice").test_support().h(px(24.))
                    .flex_shrink_0().px_2().text_xs().text_color(rgb(visual.muted))
                    .bg(rgb(visual.surface)).child(t(cx,
                        "窗口空间不足，仅显示活动面板；可用上方会话栏切换。",
                        "Showing the active pane in this compact window. Use the session strip to switch.",
                    )))
            })
            .child(div().flex_1().min_w_0().min_h_0().child(self.render_pane_node(node, available, cx)))
            .into_any_element()
    }

    #[inline(never)]
    fn render_pane_node(
        &self,
        id: PaneNodeId,
        available: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let node = self
            .session_panes
            .layout()
            .nodes
            .iter()
            .find(|node| node.id() == id);
        let Some(PaneNode::Split {
            axis,
            ratio_bps,
            first,
            second,
            ..
        }) = node
        else {
            return self.render_session_pane(id, cx);
        };
        let axis = *axis;
        let mut ratio = f32::from(*ratio_bps) / 10_000.;
        let a = self.pane_minimum(*first);
        let b = self.pane_minimum(*second);
        let (length, first_min, second_min) = match axis {
            SplitAxis::Horizontal => (available.width, a.width, b.width),
            SplitAxis::Vertical => (available.height, a.height, b.height),
        };
        if length >= first_min + second_min {
            // Preserve the requested ratio in the model while keeping every
            // rendered leaf usable as the window or side panels change size.
            ratio = ratio.clamp(first_min / length, 1. - second_min / length);
        }
        let (a, b) = match axis {
            SplitAxis::Horizontal => (
                size(available.width * ratio, available.height),
                size(available.width * (1. - ratio), available.height),
            ),
            SplitAxis::Vertical => (
                size(available.width, available.height * ratio),
                size(available.width, available.height * (1. - ratio)),
            ),
        };
        let first = self.render_pane_node(*first, a, cx);
        let second = self.render_pane_node(*second, b, cx);
        let geometry = self.pane_bounds.clone();
        let handle = self.pane_divider(id, axis, *ratio_bps, cx);
        div()
            .id(SharedString::from(format!("pane-split-{}", id.0)))
            .test_support()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .when(axis == SplitAxis::Vertical, |view| view.flex_col())
            .on_prepaint(move |bounds, _, _| {
                geometry.borrow_mut().insert(id, bounds);
            })
            .child(
                div()
                    .min_w_0()
                    .min_h_0()
                    .flex_shrink_0()
                    .flex_grow_0()
                    .flex_basis(relative(ratio))
                    .when(axis == SplitAxis::Horizontal, |pane| pane.h_full())
                    .when(axis == SplitAxis::Vertical, |pane| pane.w_full())
                    .child(first),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .when(axis == SplitAxis::Horizontal, |pane| pane.h_full())
                    .when(axis == SplitAxis::Vertical, |pane| pane.w_full())
                    .child(second)
                    .child(handle),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn pane_divider(
        &self,
        id: PaneNodeId,
        axis: SplitAxis,
        ratio_bps: u16,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let focus = self
            .pane_divider_focus
            .borrow_mut()
            .entry(id)
            // Explicit handles carry their own tab policy; the element's
            // tab_index only configures handles created by GPUI itself.
            .or_insert_with(|| cx.focus_handle().tab_stop(true).tab_index(0))
            .clone();
        let (label_zh, label_en, hint_zh, hint_en) = match axis {
            SplitAxis::Horizontal => (
                "左右分隔条",
                "Left/right divider",
                "拖动，或按 Tab 聚焦后使用左右方向键，每次调整左侧目标比例 5%。面板保留最小可用尺寸。",
                "Drag, or focus with Tab and use Left/Right to change the requested left share by 5%. Panes retain their minimum usable size.",
            ),
            SplitAxis::Vertical => (
                "上下分隔条",
                "Top/bottom divider",
                "拖动，或按 Tab 聚焦后使用上下方向键，每次调整上方目标比例 5%。面板保留最小可用尺寸。",
                "Drag, or focus with Tab and use Up/Down to change the requested top share by 5%. Panes retain their minimum usable size.",
            ),
        };
        let drag = PaneResizeDrag {
            workspace: cx.entity_id(),
            node: id,
            generation: self.pane_generation,
            input_generation: self.pane_input_generation(),
        };
        let handle_axis = match axis {
            SplitAxis::Horizontal => Axis::Horizontal,
            SplitAxis::Vertical => Axis::Vertical,
        };
        let appearance: ResizeHandleRenderer = std::rc::Rc::new(|handle, _, cx| {
            let visual = crate::design::palette(cx);
            Some(
                div()
                    .size_full()
                    .bg(rgb(if handle.is_active() {
                        visual.accent
                    } else {
                        visual.border
                    }))
                    .into_any_element(),
            )
        });
        let increase = cx.weak_entity();
        let decrease = increase.clone();
        let start_resize = increase.clone();
        gpui_kit::component::resizable::resize_handle(
            SharedString::from(format!("pane-divider-{}", id.0)),
            handle_axis,
        )
        .inside(HandleEdge::Leading)
        .with_appearance(appearance)
        .track_focus(&focus)
        .tab_index(0)
        .role(Role::Splitter)
        .aria_label(t(cx, label_zh, label_en))
        .aria_description(t(cx, hint_zh, hint_en))
        .aria_numeric_value(f64::from(ratio_bps) / 100.)
        .aria_min_numeric_value(10.)
        .aria_max_numeric_value(90.)
        .aria_numeric_value_step(5.)
        .aria_value(format!("{:.2}%", f64::from(ratio_bps) / 100.))
        .aria_orientation(if axis == SplitAxis::Horizontal {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        })
        .localized_tooltip(hint_zh, hint_en)
        .focus_visible(|style| style.bg(rgb(visual.accent)))
        // Pointer resizing keeps the current SSH keyboard focus. Tab and
        // accessibility focus explicitly enter this separator instead.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
            let modifiers = event.keystroke.modifiers;
            if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
                return;
            }
            let delta = match (axis, event.keystroke.key.as_str()) {
                (SplitAxis::Horizontal, "left") | (SplitAxis::Vertical, "up") => -500,
                (SplitAxis::Horizontal, "right") | (SplitAxis::Vertical, "down") => 500,
                _ => return,
            };
            window.prevent_default();
            cx.stop_propagation();
            if view.pane_resize_input_current(drag, cx) {
                view.adjust_pane_ratio(id, delta, cx);
            }
        }))
        .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
            let _ = increase.update(cx, |view, cx| {
                if view.pane_resize_input_current(drag, cx) {
                    view.adjust_pane_ratio(id, 500, cx);
                }
            });
        })
        .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
            let _ = decrease.update(cx, |view, cx| {
                if view.pane_resize_input_current(drag, cx) {
                    view.adjust_pane_ratio(id, -500, cx);
                }
            });
        })
        .on_drag(drag, move |drag, _, window, cx| {
            // GPUI creates the drag after capture-phase move listeners run.
            // Apply this first position too, even if no later move arrives.
            let position = window.mouse_position();
            let _ = start_resize.update(cx, |view, cx| {
                view.resize_pane(*drag, position, cx);
            });
            cx.stop_propagation();
            cx.new(|_| *drag)
        })
        .into_any_element()
    }

    #[inline(never)]
    fn render_session_pane(&self, pane: PaneNodeId, cx: &mut Context<Self>) -> AnyElement {
        let Some(id) = self.session_panes.selected_in(pane) else {
            return self.render_empty_restored_pane(pane, cx);
        };
        let Some(terminal) = self.tabs.iter().find(|tab| tab.entity_id() == id) else {
            return div().size_full().into_any_element();
        };
        let visual = crate::design::palette(cx);
        let active = self.session_panes.active_pane() == pane;
        let restored = self.restored_tabs.contains_key(&id);
        let mut view = div()
            .id(("terminal-pane", id))
            .test_support()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .when(!restored, |pane| pane.p_2())
            .bg(rgb(if restored {
                visual.canvas
            } else {
                crate::design::terminal_palette(cx).background
            }))
            .border_t_2()
            .border_color(rgb(if active { visual.accent } else { visual.border }));
        if self.session_panes.layout().nodes.len() > 1 {
            view = view.child(self.pane_header(pane, id, cx));
        }
        if restored {
            return view
                .child(self.render_restored_tab(id, cx))
                .into_any_element();
        }
        view.child(self.reconnect_banner(id, cx))
            .child(
                div()
                    .id(("terminal-input-pane", id))
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    // Capture before the terminal takes its entity lease or
                    // consumes a mouse-reporting event. GPUI focus callbacks
                    // are frame-based and cannot preserve an intervening ABA.
                    .capture_any_mouse_down(cx.listener(move |view, _, window, cx| {
                        if let Some(index) = view.tabs.iter().position(|tab| tab.entity_id() == id)
                        {
                            view.select_session_tab(index, window, cx);
                        }
                    }))
                    .child(terminal.clone()),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn pane_header(
        &self,
        pane: PaneNodeId,
        selected: EntityId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visual = crate::design::palette(cx);
        let chrome = crate::design::chrome(cx);
        let mut tabs = div()
            .id(SharedString::from(format!("pane-tabs-{}", pane.0)))
            .test_support()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .overflow_x_scroll()
            .overflow_y_hidden();
        for key in self.session_panes.tabs_in(pane) {
            if let Some(tab) = self.tabs.iter().find(|tab| tab.entity_id() == *key) {
                let key = *key;
                tabs = tabs.child(
                    Button::new(("pane-tab", key))
                        .ghost()
                        .compact()
                        .label(self.session_title(tab.entity_id(), cx))
                        .selected(key == selected)
                        .toggled(key == selected)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            if let Some(index) =
                                view.tabs.iter().position(|tab| tab.entity_id() == key)
                            {
                                view.select_session_tab(index, window, cx);
                            }
                        }))
                        .map(|button| self.retain_modal_button(("pane-tab", key), button, cx)),
                );
            }
        }
        div()
            .h(px(chrome.pane_header_height))
            .flex_shrink_0()
            .min_w_0()
            .flex()
            .items_center()
            .bg(rgb(visual.surface))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(tabs)
            .child(self.pane_ratio_controls(pane, selected, cx))
            .into_any_element()
    }

    #[inline(never)]
    fn pane_ratio_controls(
        &self,
        pane: PaneNodeId,
        id: EntityId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let parent = self.session_panes.layout().nodes.iter().find_map(|node| {
            if let PaneNode::Split {
                id, first, second, ..
            } = node
            {
                if *first == pane {
                    return Some((*id, true));
                }
                if *second == pane {
                    return Some((*id, false));
                }
            }
            None
        });
        let Some((split, first_side)) = parent else {
            return Empty.into_any_element();
        };
        let generation = self.pane_generation;
        let input_generation = self.pane_input_generation();
        div()
            .flex_none()
            .flex()
            .items_center()
            .children([
                Button::new(("pane-smaller", id))
                    .ghost()
                    .compact()
                    .label("−")
                    .accessibility_label(t(cx, "缩小此面板", "Make this pane smaller"))
                    .disabled(self.pane_zoomed || self.compact_panes())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.pane_generation == generation
                            && view.pane_input_generation() == input_generation
                        {
                            view.adjust_pane_ratio(split, if first_side { -500 } else { 500 }, cx);
                        }
                    }))
                    .map(|button| self.retain_modal_button(("pane-smaller", id), button, cx)),
                Button::new(("pane-larger", id))
                    .ghost()
                    .compact()
                    .label("+")
                    .accessibility_label(t(cx, "扩大此面板", "Make this pane larger"))
                    .disabled(self.pane_zoomed || self.compact_panes())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.pane_generation == generation
                            && view.pane_input_generation() == input_generation
                        {
                            view.adjust_pane_ratio(split, if first_side { 500 } else { -500 }, cx);
                        }
                    }))
                    .map(|button| self.retain_modal_button(("pane-larger", id), button, cx)),
            ])
            .into_any_element()
    }
}
