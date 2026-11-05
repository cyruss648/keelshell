//! Production GPUI bounds, pointer dispatch and inert persistence; not OS acceptance.

use super::*;
use gpui_kit::{
    App, ElementId, EntityId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Role, SharedString, WeakFocusHandle, Window,
};
use keelshell_core::{PaneNode, PaneNodeId, SplitAxis};

use crate::workspace::pane_sessions::SessionPanes;

fn pane_id(prefix: &str, node: PaneNodeId) -> SharedString {
    format!("{prefix}-{}", node.0).into()
}

fn redraw(window: &mut Window, cx: &mut App) {
    // Canvas measurement requests a deferred repaint when compact mode changes.
    for _ in 0..3 {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
    }
    window.render_frame(cx);
}

fn attach_count(fixture: &Fixture, count: usize, cx: &mut TestAppContext) -> Vec<RemotePane> {
    let mut panes = Vec::new();
    for _ in 0..count.div_ceil(2) {
        panes.extend(attach_remote_panes(fixture, cx));
    }
    panes.truncate(count);
    panes
}

fn arrange(
    fixture: &Fixture,
    panes: &[RemotePane],
    splits: &[(usize, usize, SplitAxis)],
    active: usize,
    cx: &mut TestAppContext,
) -> Vec<EntityId> {
    let ids: Vec<_> = panes.iter().map(|pane| pane.terminal.entity_id()).collect();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            // Only the initial geometry is seeded. All later interactions use
            // the production controls and these observed transport entities.
            let mut mapping = SessionPanes::new();
            mapping
                .reconcile(&ids[..1], Some(ids[0]))
                .checked("seed first leaf");
            for (primary, peer, axis) in splits {
                mapping
                    .split(ids[*primary], ids[*peer], *axis)
                    .checked("seed nested pane topology");
            }
            mapping
                .reconcile(&ids, Some(ids[active]))
                .checked("register all fixture sessions");
            workspace.tabs = panes.iter().map(|pane| pane.terminal.clone()).collect();
            workspace.active = active;
            workspace.session_panes = mapping;
            workspace.show_connections = false;
            workspace.show_navigation = false;
            workspace.show_assistant = false;
            workspace.visible_panel = None;
            assert!(workspace.sync_session_panes());
            workspace.select_session_tab(active, window, cx);
            cx.notify();
        });
        redraw(window, cx);
    })
    .checked("mount deterministic sessions in the production pane canvas");
    ids
}

fn nested_scene(cx: &mut TestAppContext) -> (Fixture, Vec<RemotePane>, Vec<EntityId>) {
    let fixture = mount_sized(cx, Vec::new(), 1600., 1000.);
    let panes = attach_count(&fixture, 4, cx);
    let ids = arrange(
        &fixture,
        &panes,
        &[
            (0, 1, SplitAxis::Horizontal),
            (1, 2, SplitAxis::Vertical),
            (2, 3, SplitAxis::Horizontal),
        ],
        0,
        cx,
    );
    (fixture, panes, ids)
}

fn owner(workspace: &Workspace, key: EntityId) -> PaneNodeId {
    workspace
        .session_panes
        .pane_for(key)
        .ok_or("registered session must retain its pane")
        .checked("resolve leaf identity")
}

fn parent(workspace: &Workspace, key: EntityId) -> PaneNodeId {
    let leaf = owner(workspace, key);
    workspace
        .session_panes
        .layout()
        .nodes
        .iter()
        .find_map(|node| match node {
            PaneNode::Split {
                id, first, second, ..
            } if *first == leaf || *second == leaf => Some(*id),
            _ => None,
        })
        .ok_or("expected a parent split")
        .checked("resolve observed divider")
}

fn ratio(workspace: &Workspace, split: PaneNodeId) -> u16 {
    workspace
        .session_panes
        .layout()
        .nodes
        .iter()
        .find_map(|node| match node {
            PaneNode::Split { id, ratio_bps, .. } if *id == split => Some(*ratio_bps),
            _ => None,
        })
        .ok_or("expected a split ratio")
        .checked("read stored basis points")
}

fn near(actual: Pixels, expected: Pixels) {
    assert!(
        (f32::from(actual) - f32::from(expected)).abs() <= 2.,
        "expected {expected:?}, observed {actual:?}"
    );
}

fn inside(child: Bounds<Pixels>, container: Bounds<Pixels>) {
    assert!(child.size.width > px(0.) && child.size.height > px(0.));
    assert!(child.left() >= container.left() - px(1.));
    assert!(child.top() >= container.top() - px(1.));
    assert!(child.right() <= container.right() + px(1.));
    assert!(child.bottom() <= container.bottom() + px(1.));
}

fn live_ids(workspace: &Workspace) -> Vec<EntityId> {
    workspace.tabs.iter().map(Entity::entity_id).collect()
}

fn only_visible(window: &Window, ids: &[EntityId], active: EntityId) {
    for id in ids {
        let visible = window
            .try_find(("terminal-pane", *id))
            .is_some_and(|pane| pane.visible());
        assert_eq!(visible, *id == active, "pane {id:?} visibility");
    }
}

fn no_writes(panes: &[RemotePane]) {
    for pane in panes {
        assert!(
            writes(pane).is_empty(),
            "layout controls must not write to SSH"
        );
    }
}

fn begin_drag(window: &mut Window, cx: &mut App, from: Point<Pixels>, axis: SplitAxis) {
    assert!(!cx.has_active_drag());
    window.dispatch_event(
        MouseMoveEvent {
            position: from,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.dispatch_event(
        MouseDownEvent {
            position: from,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    let start = match axis {
        SplitAxis::Horizontal => point(from.x - px(8.), from.y),
        SplitAxis::Vertical => point(from.x, from.y - px(8.)),
    };
    window.dispatch_event(
        MouseMoveEvent {
            position: start,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    assert!(
        cx.has_active_drag(),
        "the real resize handle must start its on_drag payload"
    );
}

fn finish_drag(window: &mut Window, cx: &mut App, to: Point<Pixels>) {
    move_drag(window, cx, to);
    window.dispatch_event(
        MouseUpEvent {
            position: to,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    redraw(window, cx);
    assert!(
        !cx.has_active_drag(),
        "mouse-up must release the active drag"
    );
}

fn move_drag(window: &mut Window, cx: &mut App, to: Point<Pixels>) {
    assert!(
        cx.has_active_drag(),
        "move the payload created by the actual handle"
    );
    window.dispatch_event(
        MouseMoveEvent {
            position: to,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

#[gpui_kit::test]
fn nested_horizontal_vertical_bounds_and_ratio_buttons_follow_the_topology(
    cx: &mut TestAppContext,
) {
    let (fixture, panes, ids) = nested_scene(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        let root = workspace.session_panes.layout().root;
        let vertical = parent(workspace, ids[1]);
        let bottom = parent(workspace, ids[2]);
        let canvas = window.find("pane-canvas").bounds();
        let root_bounds = window.find(pane_id("pane-split", root)).bounds();
        let right_bounds = window.find(pane_id("pane-split", vertical)).bounds();
        let bottom_bounds = window.find(pane_id("pane-split", bottom)).bounds();
        let bounds: Vec<_> = ids
            .iter()
            .map(|id| window.find(("terminal-pane", *id)).bounds())
            .collect();
        inside(root_bounds, canvas);
        for (id, bounds) in ids.iter().zip(&bounds) {
            inside(*bounds, canvas);
            let tabs = window.find(pane_id("pane-tabs", owner(workspace, *id)));
            assert!(tabs.visible());
            inside(tabs.bounds(), *bounds);
        }
        near(bounds[0].left(), root_bounds.left());
        near(bounds[0].right(), right_bounds.left());
        near(bounds[0].size.width, root_bounds.size.width * 0.5);
        near(bounds[0].size.height, root_bounds.size.height);
        near(bounds[1].top(), right_bounds.top());
        near(bounds[1].bottom(), bottom_bounds.top());
        near(bounds[1].size.height, right_bounds.size.height * 0.5);
        near(bounds[2].right(), bounds[3].left());
        near(bounds[2].size.width, bottom_bounds.size.width * 0.5);
        near(bounds[3].right(), right_bounds.right());
        near(bounds[2].bottom(), right_bounds.bottom());
        assert_eq!(workspace.session_panes.displayed_sessions(), ids);

        window.click(("pane-smaller", ids[2]), cx);
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(ratio(workspace, bottom), 4500);
        assert_eq!(ratio(workspace, root), 5000);
        assert_eq!(ratio(workspace, vertical), 5000);
        assert!(window.find(("terminal-pane", ids[2])).bounds().size.width < bounds[2].size.width);
        window.click(("pane-larger", ids[2]), cx);
        redraw(window, cx);
        assert_eq!(ratio(fixture.workspace.read(cx), bottom), 5000);
        for (id, original) in ids.iter().zip(bounds) {
            assert_eq!(window.find(("terminal-pane", *id)).bounds(), original);
        }
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
    })
    .checked("measure production nested bounds and operate the actual ratio controls");
    no_writes(&panes);
}

#[gpui_kit::test]
fn single_move_pointer_drag_commits_its_release_position_on_both_axes(cx: &mut TestAppContext) {
    short_pointer_drag(cx, 1);
}

#[gpui_kit::test]
fn repeated_target_move_pointer_drag_commits_its_release_position_on_both_axes(
    cx: &mut TestAppContext,
) {
    short_pointer_drag(cx, 2);
}

fn short_pointer_drag(cx: &mut TestAppContext, move_count: usize) {
    let (fixture, panes, ids) = nested_scene(cx);
    let mut observed = Vec::new();
    cx.update_window(fixture.window, |_, window, cx| {
        for (key, axis) in [
            (ids[0], SplitAxis::Horizontal),
            (ids[1], SplitAxis::Vertical),
        ] {
            let workspace = fixture.workspace.read(cx);
            let split = parent(workspace, key);
            let ownership: Vec<_> = ids.iter().map(|id| (*id, owner(workspace, *id))).collect();
            let scope = window.find(pane_id("pane-split", split)).bounds();
            let from = window
                .find(pane_id("pane-divider", split))
                .bounds()
                .center();
            let to = match axis {
                SplitAxis::Horizontal => {
                    point(scope.left() + scope.size.width * 0.65, scope.center().y)
                }
                SplitAxis::Vertical => {
                    point(scope.center().x, scope.top() + scope.size.height * 0.65)
                }
            };
            window.dispatch_event(
                MouseMoveEvent {
                    position: from,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                MouseDownEvent {
                    position: from,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            // A native drag may coalesce motion into one threshold-crossing
            // event. No preliminary 8px move is inserted in this regression.
            for _ in 0..move_count {
                window.dispatch_event(
                    MouseMoveEvent {
                        position: to,
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.render_frame(cx);
            }
            assert!(cx.has_active_drag());
            window.dispatch_event(
                MouseUpEvent {
                    position: to,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            redraw(window, cx);
            assert!(!cx.has_active_drag());
            let workspace = fixture.workspace.read(cx);
            observed.push((axis, ratio(workspace, split)));
            assert_eq!(live_ids(workspace), ids);
            assert_eq!(workspace.session_panes.active_session(), Some(ids[0]));
            assert_eq!(
                ids.iter()
                    .map(|id| (*id, owner(workspace, *id)))
                    .collect::<Vec<_>>(),
                ownership
            );
            assert!(
                panes[0]
                    .terminal
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        }
    })
    .checked("compare one and two native-style target moves without an extra drag step");
    no_writes(&panes);
    assert_eq!(
        observed,
        vec![(SplitAxis::Horizontal, 6500), (SplitAxis::Vertical, 6500)],
        "{move_count} target move(s) must reach the final requested ratio"
    );
}

#[gpui_kit::test]
fn pointer_drags_resize_both_axes_through_on_drag_and_clamp_the_ratio(cx: &mut TestAppContext) {
    let (fixture, panes, ids) = nested_scene(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        for (key, axis, fraction, expected) in [
            (ids[0], SplitAxis::Horizontal, 0.65, 6500_u16),
            (ids[1], SplitAxis::Vertical, 0.35, 3500),
            (ids[0], SplitAxis::Horizontal, 0.99, 9000),
            (ids[0], SplitAxis::Horizontal, 0.01, 1000),
            (ids[0], SplitAxis::Horizontal, 0.5, 5000),
        ] {
            let workspace = fixture.workspace.read(cx);
            let split = parent(workspace, key);
            let old_ratio = ratio(workspace, split);
            let untouched: Vec<_> = workspace
                .session_panes
                .layout()
                .nodes
                .iter()
                .filter(|node| node.id() != split)
                .cloned()
                .collect();
            let scope = window.find(pane_id("pane-split", split)).bounds();
            let handle = window.find(pane_id("pane-divider", split));
            assert!(handle.visible());
            inside(handle.bounds(), scope);
            let to = match axis {
                SplitAxis::Horizontal => {
                    point(scope.left() + scope.size.width * fraction, scope.center().y)
                }
                SplitAxis::Vertical => {
                    point(scope.center().x, scope.top() + scope.size.height * fraction)
                }
            };
            begin_drag(window, cx, handle.bounds().center(), axis);
            finish_drag(window, cx, to);

            let workspace = fixture.workspace.read(cx);
            let new_ratio = ratio(workspace, split);
            assert_ne!(new_ratio, old_ratio);
            if expected == 1000 || expected == 9000 {
                assert_eq!(
                    new_ratio, expected,
                    "stored ratio must retain the exact limit"
                );
            } else {
                assert!(
                    new_ratio.abs_diff(expected) <= 1,
                    "{axis:?}: {new_ratio} != {expected}"
                );
                let first = window.find(("terminal-pane", key)).bounds();
                match axis {
                    SplitAxis::Horizontal => near(
                        first.size.width,
                        scope.size.width * (f32::from(new_ratio) / 10_000.),
                    ),
                    SplitAxis::Vertical => near(
                        first.size.height,
                        scope.size.height * (f32::from(new_ratio) / 10_000.),
                    ),
                }
            }
            // The requested limit can exceed the usable visual ratio: every
            // descendant must still fit its minimum size and its parent.
            let canvas = window.find("pane-canvas").bounds();
            for id in &ids {
                let pane = window.find(("terminal-pane", *id));
                let bounds = pane.bounds();
                assert!(pane.visible());
                assert!(bounds.size.width >= px(200.), "leaf {id:?}: {bounds:?}");
                assert!(bounds.size.height >= px(96.), "leaf {id:?}: {bounds:?}");
                inside(bounds, canvas);
                inside(
                    bounds,
                    window
                        .find(pane_id("pane-split", parent(workspace, *id)))
                        .bounds(),
                );
            }
            assert_eq!(
                workspace
                    .session_panes
                    .layout()
                    .nodes
                    .iter()
                    .filter(|node| node.id() != split)
                    .cloned()
                    .collect::<Vec<_>>(),
                untouched
            );
            assert_eq!(workspace.session_panes.active_session(), Some(ids[0]));
            assert_eq!(live_ids(workspace), ids);
            assert!(
                panes[0]
                    .terminal
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        }
    })
    .checked("drag real resize handles through platform pointer dispatch");
    no_writes(&panes);
}

#[gpui_kit::test]
fn modal_open_and_close_invalidates_a_held_resize_drag(cx: &mut TestAppContext) {
    let (fixture, panes, ids) = nested_scene(cx);
    let (before, generation, split, to) = cx
        .update_window(fixture.window, |_, window, cx| {
            let workspace = fixture.workspace.read(cx);
            let generation = workspace.pane_input_generation();
            let split = parent(workspace, ids[0]);
            let scope = window.find(pane_id("pane-split", split)).bounds();
            let to = point(scope.left() + scope.size.width * 0.65, scope.center().y);
            let handle = window
                .find(pane_id("pane-divider", split))
                .bounds()
                .center();
            begin_drag(window, cx, handle, SplitAxis::Horizontal);
            // The first legitimate move is already applied before the modal
            // invalidates this payload. Only subsequent input must be inert.
            let before = fixture.workspace.read(cx).session_panes.clone();
            window.dispatch_action(Box::new(crate::workspace::OpenConnections), cx);
            (before, generation, split, to)
        })
        .checked("hold the real resize drag while opening the connection manager");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.show_connections);
        assert!(workspace.active_modal().is_some());
        assert_ne!(workspace.pane_input_generation(), generation);
        assert!(window.find("connection-manager-dialog").visible());
        move_drag(window, cx, to);
        assert_eq!(fixture.workspace.read(cx).session_panes, before);
        window.press("escape", cx);
        redraw(window, cx);
        assert!(fixture.workspace.read(cx).active_modal().is_none());
        assert!(
            cx.has_active_drag(),
            "modal dismissal must leave the old payload observable"
        );
        finish_drag(window, cx, to);
        assert_eq!(fixture.workspace.read(cx).session_panes, before);
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );

        let handle = window
            .find(pane_id("pane-divider", split))
            .bounds()
            .center();
        begin_drag(window, cx, handle, SplitAxis::Horizontal);
        finish_drag(window, cx, to);
        assert!(ratio(fixture.workspace.read(cx), split).abs_diff(6500) <= 1);
    })
    .checked("reject the pre-modal payload after dismissal but accept a fresh drag");
    no_writes(&panes);
}

#[gpui_kit::test]
fn closing_another_leaf_invalidates_a_held_drag_on_a_surviving_split(cx: &mut TestAppContext) {
    let (fixture, panes, ids) = nested_scene(cx);
    let (split, generation, started_ratio) = cx
        .update_window(fixture.window, |_, window, cx| {
            let workspace = fixture.workspace.read(cx);
            let split = parent(workspace, ids[2]);
            let generation = workspace.pane_generation;
            assert_eq!(workspace.session_panes.active_session(), Some(ids[0]));
            let handle = window
                .find(pane_id("pane-divider", split))
                .bounds()
                .center();
            begin_drag(window, cx, handle, SplitAxis::Horizontal);
            let started_ratio = ratio(fixture.workspace.read(cx), split);
            window.dispatch_action(Box::new(crate::workspace::CloseTab), cx);
            (split, generation, started_ratio)
        })
        .checked("hold the nested divider while closing the active outer leaf");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_ne!(workspace.pane_generation, generation);
        assert_eq!(live_ids(workspace), ids[1..]);
        assert_eq!(workspace.session_panes.active_session(), Some(ids[1]));
        assert_eq!(parent(workspace, ids[2]), split);
        assert_eq!(ratio(workspace, split), started_ratio);
        let after_close = workspace.session_panes.clone();
        let scope = window.find(pane_id("pane-split", split)).bounds();
        let to = point(scope.left() + scope.size.width * 0.7, scope.center().y);
        finish_drag(window, cx, to);
        assert_eq!(fixture.workspace.read(cx).session_panes, after_close);
        assert!(
            panes[1]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(
            panes[1..]
                .iter()
                .all(|pane| pane.terminal.read(cx).is_open())
        );

        let handle = window
            .find(pane_id("pane-divider", split))
            .bounds()
            .center();
        begin_drag(window, cx, handle, SplitAxis::Horizontal);
        finish_drag(window, cx, to);
        assert!(ratio(fixture.workspace.read(cx), split).abs_diff(7000) <= 1);
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids[1..]);
    })
    .checked("reject the old topology payload while its split still exists");
    no_writes(&panes);
}

#[gpui_kit::test]
fn balanced_dividers_are_tab_reachable_and_adjust_their_own_axis(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 1600., 1000.);
    let panes = attach_count(&fixture, 4, cx);
    let ids = arrange(
        &fixture,
        &panes,
        &[
            (0, 1, SplitAxis::Horizontal),
            (0, 2, SplitAxis::Vertical),
            (1, 3, SplitAxis::Vertical),
        ],
        0,
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        let splits: Vec<_> = fixture
            .workspace
            .read(cx)
            .session_panes
            .layout()
            .nodes
            .iter()
            .filter_map(|node| match node {
                PaneNode::Split { id, axis, .. } => Some((*id, *axis)),
                _ => None,
            })
            .collect();
        assert_eq!(splits.len(), 3);
        let focus = fixture
            .workspace
            .read(cx)
            .modal_button_focus
            .borrow()
            .get(&ElementId::from(("pane-larger", ids[0])))
            .and_then(WeakFocusHandle::upgrade)
            .ok_or("initial pane button")
            .checked("start keyboard navigation at a real Kit control");
        focus.focus(window, cx);
        redraw(window, cx);
        let mut visited = std::collections::HashSet::new();
        for _ in 0..128 {
            window.press("tab", cx);
            redraw(window, cx);
            for (split, axis) in &splits {
                let handle = window.find(pane_id("pane-divider", *split));
                // Kit observes its internal resize band after the caller binds
                // focus. Read the production handle while retaining real Tab input.
                let focused = fixture
                    .workspace
                    .read(cx)
                    .pane_divider_focus
                    .borrow()
                    .get(split)
                    .is_some_and(|focus| focus.is_focused(window));
                if focused && visited.insert(*split) {
                    assert_eq!(handle.role(), Some(Role::Splitter));
                    let before = fixture.workspace.read(cx).session_panes.clone();
                    window.press(
                        if *axis == SplitAxis::Horizontal {
                            "down"
                        } else {
                            "right"
                        },
                        cx,
                    );
                    redraw(window, cx);
                    assert_eq!(fixture.workspace.read(cx).session_panes, before);
                    window.press(
                        if *axis == SplitAxis::Horizontal {
                            "right"
                        } else {
                            "down"
                        },
                        cx,
                    );
                    redraw(window, cx);
                    assert_eq!(ratio(fixture.workspace.read(cx), *split), 5500);
                    let mut expected = before;
                    expected
                        .set_ratio(*split, 5500)
                        .checked("only the focused divider changes its ratio");
                    assert_eq!(fixture.workspace.read(cx).session_panes, expected);
                }
            }
            if visited.len() == splits.len() {
                break;
            }
        }
        assert_eq!(
            visited.len(),
            3,
            "all three dividers, including the root, must be Tab reachable"
        );
        for (split, _) in splits {
            assert_eq!(ratio(fixture.workspace.read(cx), split), 5500);
        }
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
        assert_eq!(
            fixture.workspace.read(cx).session_panes.active_session(),
            Some(ids[0])
        );
    })
    .checked("navigate every balanced-tree divider by Tab and resize with arrow keys");
    no_writes(&panes);
}

fn assert_compact_retires_divider_focus(cx: &mut TestAppContext, from_modal: bool) {
    let fixture = mount_sized(cx, Vec::new(), 1800., 1000.);
    let panes = attach_count(&fixture, 6, cx);
    let ids = arrange(
        &fixture,
        &panes,
        &[
            (0, 3, SplitAxis::Horizontal),
            (0, 1, SplitAxis::Horizontal),
            (1, 2, SplitAxis::Horizontal),
            (3, 4, SplitAxis::Horizontal),
            (4, 5, SplitAxis::Horizontal),
        ],
        0,
        cx,
    );
    let layout = cx
        .update_window(fixture.window, |_, window, cx| {
            let root = fixture.workspace.read(cx).session_panes.layout().root;
            let focus = fixture
                .workspace
                .read(cx)
                .modal_button_focus
                .borrow()
                .get(&ElementId::from(("pane-larger", ids[0])))
                .and_then(WeakFocusHandle::upgrade)
                .ok_or("initial pane button")
                .checked("start navigation at a visible pane button");
            focus.focus(window, cx);
            redraw(window, cx);
            for _ in 0..128 {
                window.press("tab", cx);
                redraw(window, cx);
                if fixture
                    .workspace
                    .read(cx)
                    .pane_divider_focus
                    .borrow()
                    .get(&root)
                    .is_some_and(|focus| focus.is_focused(window))
                {
                    return fixture.workspace.read(cx).session_panes.clone();
                }
            }
            panic!("the root divider must be reachable before shrinking");
        })
        .checked("reach the real root divider with Tab before it is hidden");

    if from_modal {
        cx.update_window(fixture.window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::workspace::OpenConnections), cx);
        })
        .checked("dispatch the connection manager action from the focused divider");
        cx.run_until_parked();
        cx.update_window(fixture.window, |_, window, cx| {
            redraw(window, cx);
            assert!(window.find("connection-manager-dialog").visible());
        })
        .checked("unmount the focused divider behind the real connection manager");
    }
    cx.simulate_window_resize(fixture.window, size(px(900.), px(1000.)));
    cx.update_window(fixture.window, |_, window, cx| {
        redraw(window, cx);
        if from_modal {
            window.click("close-manager", cx);
            redraw(window, cx);
        }
        assert!(window.find("compact-pane-notice").visible());
        assert!(
            fixture.workspace.read(cx).surface_focus.is_focused(window),
            "unmounting the focused divider must return to the inert workspace"
        );
        window.press("right", cx);
        window.press("q", cx);
        redraw(window, cx);
        assert_eq!(fixture.workspace.read(cx).session_panes, layout);
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
        window.press("tab", cx);
        redraw(window, cx);
        assert!(!fixture.workspace.read(cx).surface_focus.is_focused(window));
    })
    .checked("keep compact keyboard input inert and retain Tab navigation");
    no_writes(&panes);
}

#[gpui_kit::test]
fn compact_window_retires_hidden_divider_focus_without_sending_terminal_input(
    cx: &mut TestAppContext,
) {
    assert_compact_retires_divider_focus(cx, false);
}

#[gpui_kit::test]
fn compact_window_modal_return_retires_hidden_divider_focus_without_terminal_input(
    cx: &mut TestAppContext,
) {
    assert_compact_retires_divider_focus(cx, true);
}

#[gpui_kit::test]
fn closing_an_inactive_leaf_keeps_the_reviewed_active_session(cx: &mut TestAppContext) {
    let (fixture, panes, ids) = nested_scene(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.set_reviewed_command("printf 'pane-draft'".into(), Some(ids[0]), window, cx);
        });
        redraw(window, cx);
        window.click(("close-tab", 2_usize), cx);
        redraw(window, cx);
        let view = fixture.workspace.read(cx);
        assert_eq!(live_ids(view), vec![ids[0], ids[1], ids[3]]);
        assert_eq!(view.session_panes.active_session(), Some(ids[0]));
        assert_eq!(view.tabs[view.active].entity_id(), ids[0]);
        assert_eq!(view.command_target, Some(ids[0]));
        assert_eq!(view.command.read(cx).value(), "printf 'pane-draft'");
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("close an inactive pane using its actual session close control");
    no_writes(&panes);
}

#[gpui_kit::test]
fn resize_drag_cannot_change_a_second_workspace_with_the_same_node_ids(cx: &mut TestAppContext) {
    let (source, source_panes, source_ids) = nested_scene(cx);
    let (target, target_panes, target_ids) = nested_scene(cx);
    let (mut mapping, generation, input_generation) = source.workspace.read_with(cx, |view, _| {
        (
            view.session_panes.clone(),
            view.pane_generation,
            view.pane_input_generation(),
        )
    });
    for (old, new) in source_ids.iter().zip(&target_ids) {
        mapping
            .replace(*old, *new)
            .checked("mirror node identities with distinct target sessions");
    }
    cx.update_window(target.window, |_, window, cx| {
        target.workspace.update(cx, |view, cx| {
            view.session_panes = mapping.clone();
            view.pane_generation = generation;
            assert_eq!(view.pane_input_generation(), input_generation);
            cx.notify();
        });
        redraw(window, cx);
    })
    .checked("mount a distinct workspace with identical node IDs and generations");
    let split = source
        .workspace
        .read_with(cx, |view, _| parent(view, source_ids[0]));
    cx.update_window(source.window, |_, window, cx| {
        let handle = window
            .find(pane_id("pane-divider", split))
            .bounds()
            .center();
        begin_drag(window, cx, handle, SplitAxis::Horizontal);
    })
    .checked("start a real resize payload in the source window");
    let source_before = source
        .workspace
        .read_with(cx, |view, _| view.session_panes.clone());
    cx.update_window(target.window, |_, window, cx| {
        let scope = window.find(pane_id("pane-split", split)).bounds();
        finish_drag(
            window,
            cx,
            point(scope.left() + scope.size.width * 0.7, scope.center().y),
        );
        assert_eq!(target.workspace.read(cx).session_panes, mapping);
        assert_eq!(live_ids(target.workspace.read(cx)), target_ids);
    })
    .checked("dispatch the held drag in a foreign workspace and reject it");
    source
        .workspace
        .read_with(cx, |view, _| assert_eq!(view.session_panes, source_before));
    no_writes(&source_panes);
    no_writes(&target_panes);
}

#[gpui_kit::test]
fn focused_ratio_buttons_accept_keyboard_activation_without_terminal_input(
    cx: &mut TestAppContext,
) {
    let (fixture, panes, ids) = nested_scene(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let split = parent(fixture.workspace.read(cx), ids[0]);
        let focus = fixture
            .workspace
            .read(cx)
            .modal_button_focus
            .borrow()
            .get(&ElementId::from(("pane-larger", ids[0])))
            .and_then(WeakFocusHandle::upgrade)
            .ok_or("rendered ratio button focus")
            .checked("focus the actual Kit ratio control");
        focus.focus(window, cx);
        window.press("enter", cx);
        redraw(window, cx);
        assert_eq!(ratio(fixture.workspace.read(cx), split), 5500);
        assert_eq!(
            fixture.workspace.read(cx).session_panes.active_session(),
            Some(ids[0])
        );
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
    })
    .checked("activate the focused Kit ratio button by keyboard");
    no_writes(&panes);
}

#[gpui_kit::test]
fn merge_keeps_every_session_and_closing_tabs_then_collapses_only_the_empty_leaf(
    cx: &mut TestAppContext,
) {
    let (fixture, panes, ids) = nested_scene(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        let destination = owner(workspace, ids[1]);
        let bottom_left = owner(workspace, ids[2]);
        let bottom_right = owner(workspace, ids[3]);
        let promoted = parent(workspace, ids[1]);
        let last_split = parent(workspace, ids[2]);
        window.click("collapse-pane", cx);
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(live_ids(workspace), ids);
        assert_eq!(workspace.session_panes.layout().root, promoted);
        assert_eq!(
            workspace.session_panes.tabs_in(destination),
            &[ids[1], ids[0]]
        );
        assert_eq!(workspace.session_panes.active_session(), Some(ids[0]));
        assert_eq!(
            workspace.session_panes.displayed_sessions(),
            vec![ids[0], ids[2], ids[3]]
        );
        assert!(panes.iter().all(|pane| pane.terminal.read(cx).is_open()));
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        no_writes(&panes);

        window.click(("pane-tab", ids[1]), cx);
        assert!(
            panes[1]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.click(("close-tab", 1_usize), cx);
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(live_ids(workspace), vec![ids[0], ids[2], ids[3]]);
        assert_eq!(workspace.session_panes.layout().root, promoted);
        assert_eq!(workspace.session_panes.tabs_in(destination), &[ids[0]]);
        assert_eq!(workspace.session_panes.active_session(), Some(ids[0]));
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        no_writes(&panes);

        window.click(("close-tab", 0_usize), cx);
        redraw(window, cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(live_ids(workspace), vec![ids[2], ids[3]]);
        assert_eq!(workspace.session_panes.layout().root, last_split);
        assert_eq!(
            workspace
                .session_panes
                .layout()
                .leaf_ids()
                .checked("remaining sibling leaves"),
            vec![bottom_left, bottom_right]
        );
        assert_eq!(workspace.session_panes.active_session(), Some(ids[2]));
        assert_eq!(
            workspace.session_panes.displayed_sessions(),
            vec![ids[2], ids[3]]
        );
        for pane in &panes[2..] {
            assert!(pane.terminal.read(cx).is_open());
            assert!(
                window
                    .find(("terminal-pane", pane.terminal.entity_id()))
                    .visible()
            );
        }
        assert!(
            panes[2]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("merge via the toolbar and close real session controls");
    no_writes(&panes);
}

#[gpui_kit::test]
fn zoom_and_compact_width_show_only_the_active_pane_then_restore_all_bounds(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 1800., 1000.);
    let panes = attach_count(&fixture, 6, cx);
    let ids = arrange(
        &fixture,
        &panes,
        &[
            (0, 3, SplitAxis::Horizontal),
            (0, 1, SplitAxis::Horizontal),
            (1, 2, SplitAxis::Horizontal),
            (3, 4, SplitAxis::Horizontal),
            (4, 5, SplitAxis::Horizontal),
        ],
        4,
        cx,
    );
    let (topology, bounds) = cx
        .update_window(fixture.window, |_, window, cx| {
            let nodes = fixture
                .workspace
                .read(cx)
                .session_panes
                .layout()
                .nodes
                .clone();
            let bounds: Vec<_> = ids
                .iter()
                .map(|id| {
                    let pane = window.find(("terminal-pane", *id));
                    assert!(pane.visible());
                    pane.bounds()
                })
                .collect();
            window.click("zoom-pane", cx);
            redraw(window, cx);
            assert!(fixture.workspace.read(cx).pane_zoomed);
            only_visible(window, &ids, ids[4]);
            window.click(("pane-smaller", ids[4]), cx);
            window.click(("pane-larger", ids[4]), cx);
            assert_eq!(
                fixture.workspace.read(cx).session_panes.layout().nodes,
                nodes
            );
            window.click(("session-tab", 1_usize), cx);
            redraw(window, cx);
            only_visible(window, &ids, ids[1]);
            assert!(
                panes[1]
                    .terminal
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            window.click("zoom-pane", cx);
            redraw(window, cx);
            assert!(!fixture.workspace.read(cx).pane_zoomed);
            for (id, bounds) in ids.iter().zip(&bounds) {
                assert_eq!(window.find(("terminal-pane", *id)).bounds(), *bounds);
            }
            (nodes, bounds)
        })
        .checked("zoom the production active pane without changing topology");
    no_writes(&panes);

    cx.simulate_window_resize(fixture.window, size(px(900.), px(1000.)));
    cx.update_window(fixture.window, |_, window, cx| {
        redraw(window, cx);
        assert_eq!(window.viewport_size().width, px(900.));
        assert!(window.find("compact-pane-notice").visible());
        only_visible(window, &ids, ids[1]);
        window.click(("pane-smaller", ids[1]), cx);
        window.click(("pane-larger", ids[1]), cx);
        assert_eq!(
            fixture.workspace.read(cx).session_panes.layout().nodes,
            topology
        );
        for active in 2..6 {
            window.click("next-session", cx);
            redraw(window, cx);
            only_visible(window, &ids, ids[active]);
            assert_eq!(
                fixture.workspace.read(cx).session_panes.active_session(),
                Some(ids[active])
            );
            assert!(
                panes[active]
                    .terminal
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        }
        assert_eq!(
            fixture.workspace.read(cx).session_panes.layout().nodes,
            topology
        );
        assert_eq!(live_ids(fixture.workspace.read(cx)), ids);
    })
    .checked("switch live sessions while the narrow canvas shows only one pane");

    cx.simulate_window_resize(fixture.window, size(px(1800.), px(1000.)));
    cx.update_window(fixture.window, |_, window, cx| {
        redraw(window, cx);
        assert!(window.try_find("compact-pane-notice").is_none());
        for (id, bounds) in ids.iter().zip(&bounds) {
            let pane = window.find(("terminal-pane", *id));
            assert!(pane.visible());
            assert_eq!(pane.bounds(), *bounds);
        }
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.session_panes.layout().nodes, topology);
        assert_eq!(live_ids(workspace), ids);
        assert_eq!(workspace.session_panes.active_session(), Some(ids[5]));
        assert!(
            panes[5]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("restore the same pane identities and bounds after widening");
    no_writes(&panes);
}

fn json_fields(value: &serde_json::Value, expected: &[&str]) {
    let object = value
        .as_object()
        .ok_or("expected metadata object")
        .checked("inspect saved layout shape");
    let mut actual: Vec<_> = object.keys().map(String::as_str).collect();
    let mut expected = expected.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn assert_inert_wire(layout: &keelshell_core::WorkspaceLayout) {
    let wire = serde_json::to_value(layout).checked("serialize persisted layout metadata");
    json_fields(&wire, &["version", "windows", "active_window"]);
    for window in wire["windows"]
        .as_array()
        .ok_or("saved windows")
        .checked("read windows")
    {
        json_fields(window, &["id", "layout", "panes"]);
        json_fields(&window["layout"], &["root", "active", "nodes"]);
        for node in window["layout"]["nodes"]
            .as_array()
            .ok_or("saved nodes")
            .checked("read nodes")
        {
            if node["type"] == "leaf" {
                json_fields(node, &["id", "type"]);
            } else {
                assert_eq!(node["type"], "split");
                json_fields(
                    node,
                    &["id", "type", "axis", "ratio_bps", "first", "second"],
                );
            }
        }
        for pane in window["panes"]
            .as_array()
            .ok_or("saved panes")
            .checked("read panes")
        {
            json_fields(pane, &["pane", "tabs", "active_tab"]);
            for tab in pane["tabs"]
                .as_array()
                .ok_or("saved tabs")
                .checked("read tabs")
            {
                json_fields(tab, &["id", "connection_id"]);
            }
        }
    }
}

#[gpui_kit::test]
async fn explicit_save_round_trips_only_layout_references_and_reopen_creates_no_sessions_or_grants(
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("saved profile", "saved.example.invalid", "reader");
    let fixture = mount_sized(cx, vec![profile.clone()], 1600., 1000.);
    let panes = attach_count(&fixture, 4, cx);
    let ids = arrange(
        &fixture,
        &panes,
        &[(0, 1, SplitAxis::Horizontal), (1, 2, SplitAxis::Vertical)],
        0,
        cx,
    );
    let disk_before =
        std::fs::read(fixture.store.path()).checked("read state before layout interactions");
    let (mapping, tab_ids, window_id, runtime) = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.remote_connection_ids.insert(ids[0], profile.id);
                workspace.remote_connection_ids.insert(ids[2], profile.id);
                for (index, pane) in panes.iter().enumerate() {
                    let marker = format!("runtime-only-pane-{index}");
                    workspace
                        .remote_hosts
                        .insert(pane.terminal.entity_id(), marker.clone());
                    pane.terminal.update(cx, |terminal, _| {
                        terminal.title = marker.clone();
                        terminal.emulator.feed(marker.as_bytes());
                    });
                }
                workspace.command.update(cx, |input, cx| {
                    input.set_value("printf 'UNSENT-PANE-DRAFT'", window, cx)
                });
                cx.notify();
            });
            redraw(window, cx);
            window.click(("pane-smaller", ids[0]), cx);
            window.click("zoom-pane", cx);
            window.click("zoom-pane", cx);
            redraw(window, cx);
            let workspace = fixture.workspace.read(cx);
            assert!(!workspace.saving);
            assert!(workspace.state.workspace_layout.is_none());
            assert!(!workspace.mcp_test_enabled());
            assert!(workspace.agent.is_none());
            (
                workspace.session_panes.clone(),
                workspace.pane_tab_ids.clone(),
                workspace.layout_window_id,
                workspace.runtime.clone(),
            )
        })
        .checked("exercise unsaved layout and retain runtime-only drafts");
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read state before explicit save"),
        disk_before
    );
    no_writes(&panes);
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("save-pane-layout", cx);
    })
    .checked("explicitly save layout using the production toolbar");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;

    let loaded = fixture
        .store
        .load()
        .checked("reload actual saved StateStore");
    let saved = loaded
        .workspace_layout
        .as_ref()
        .ok_or("explicit save must persist a layout")
        .checked("read the committed layout");
    saved.validate().checked("validate saved metadata");
    assert_eq!(saved.windows.len(), 1);
    assert_eq!(saved.active_window, window_id);
    assert_eq!(saved.windows[0].id, window_id);
    assert_eq!(&saved.windows[0].layout, mapping.layout());
    for pane in &saved.windows[0].panes {
        let live = mapping.tabs_in(pane.pane);
        assert_eq!(pane.tabs.len(), live.len());
        for (tab, key) in pane.tabs.iter().zip(live) {
            assert_eq!(Some(&tab.id), tab_ids.get(key));
            assert!(!tab.id.is_nil());
            let expected_profile = (*key == ids[0] || *key == ids[2]).then_some(profile.id);
            assert_eq!(tab.connection_id, expected_profile);
        }
        assert_eq!(
            pane.active_tab,
            mapping
                .selected_in(pane.pane)
                .and_then(|key| tab_ids.get(&key).copied())
        );
    }
    assert_inert_wire(saved);
    let wire = serde_json::to_string(saved).checked("inspect persisted layout text");
    assert!(!wire.contains("runtime-only-pane-"));
    assert!(!wire.contains("UNSENT-PANE-DRAFT"));
    for id in &ids {
        assert!(!wire.contains(&format!("{id:?}")));
    }
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.session_panes, mapping);
        assert_eq!(workspace.pane_tab_ids, tab_ids);
        assert_eq!(live_ids(workspace), ids);
        assert_eq!(
            workspace.command.read(cx).value(),
            "printf 'UNSENT-PANE-DRAFT'"
        );
        assert!(!workspace.mcp_test_enabled());
        assert!(workspace.agent.is_none());
    });
    no_writes(&panes);

    let expected_saved = saved.clone();
    let reopened = cx
        .update_window(fixture.window, |_, window, cx| {
            let store = fixture.store.clone();
            cx.new(|cx| Workspace::new(store, loaded, None, runtime, window, cx))
        })
        .checked("construct a fresh production Workspace from saved metadata");
    reopened.read_with(cx, |workspace, cx| {
        assert_eq!(
            workspace.state.workspace_layout.as_ref(),
            Some(&expected_saved)
        );
        assert_eq!(workspace.tabs.len(), ids.len());
        assert!(workspace.tabs.iter().all(|tab| !tab.read(cx).is_open()));
        assert!(workspace.remote_sessions.is_empty());
        assert!(workspace.remote_hosts.is_empty());
        assert!(workspace.remote_connection_ids.is_empty());
        assert!(workspace.panels.is_empty());
        assert!(workspace.reconnect_bindings.is_empty());
        assert_eq!(workspace.pane_tab_ids.len(), tab_ids.len());
        assert_eq!(workspace.restored_tabs.len(), ids.len());
        assert_eq!(workspace.session_panes.layout(), mapping.layout());
        for saved in &expected_saved.windows[0].panes {
            assert_eq!(
                workspace
                    .session_panes
                    .tabs_in(saved.pane)
                    .iter()
                    .map(|key| workspace.pane_tab_ids[key])
                    .collect::<Vec<_>>(),
                saved.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>()
            );
            assert_eq!(
                workspace
                    .session_panes
                    .selected_in(saved.pane)
                    .map(|key| workspace.pane_tab_ids[&key]),
                saved.active_tab
            );
        }
        assert!(!workspace.connecting);
        assert!(workspace.connect_route.is_none());
        assert!(workspace.login.is_none());
        assert!(workspace.host_approval.is_none());
        assert!(workspace.agent.is_none());
        assert!(!workspace.mcp_test_enabled());
        assert!(workspace.mcp_test_client().is_err());
        assert!(workspace.command_target.is_none());
        assert!(workspace.command.read(cx).value().is_empty());
        assert_eq!(
            workspace.assistant.read(cx).captured_context_for_test(),
            ("", "", "")
        );
    });
    no_writes(&panes);
}

#[gpui_kit::test]
async fn conflicting_layout_save_keeps_disk_and_live_panes_unchanged(cx: &mut TestAppContext) {
    let (fixture, panes, ids) = nested_scene(cx);
    let before = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.session_panes.clone());
    let other_store = StateStore::new(fixture.store.path());
    let mut external = other_store.load().checked("read competing writer snapshot");
    external.connections.push(Connection::new(
        "external profile",
        "external.example.invalid",
        "reader",
    ));
    other_store
        .save(&external)
        .checked("commit competing state revision");
    let external_bytes = std::fs::read(fixture.store.path()).checked("capture competing bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("save-pane-layout", cx);
    })
    .checked("submit layout from the stale workspace revision");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read after conflict"),
        external_bytes
    );
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.state.workspace_layout.is_none());
        assert_eq!(workspace.session_panes, before);
        assert_eq!(live_ids(workspace), ids);
        assert!(panes.iter().all(|pane| pane.terminal.read(cx).is_open()));
        assert!(!workspace.mcp_test_enabled());
        assert!(workspace.agent.is_none());
    });
    assert!(
        fixture
            .store
            .load()
            .checked("read preserved external state")
            .workspace_layout
            .is_none()
    );
    no_writes(&panes);
}
