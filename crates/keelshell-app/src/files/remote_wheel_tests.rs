//! Real wheel reachability after a completed transfer in the production workspace.
use super::*;
use crate::workspace::tests::mirror_sync_combination::mount_scene;
use gpui_kit::{
    App, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollDelta, ScrollEvent, Window,
};
use keelshell_core::{Settings, Theme, ThemeId};

fn intersection(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    Bounds::new(
        point(left, top),
        size(
            (a.right().min(b.right()) - left).max(px(0.)),
            (a.bottom().min(b.bottom()) - top).max(px(0.)),
        ),
    )
}

fn visible_list(window: &mut Window) -> Bounds<Pixels> {
    let mut clip = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
    for id in ["file-browsing-area", "remote-files-table", "remote-files"] {
        clip = intersection(clip, window.find(id).bounds());
    }
    assert!(clip.size.width > px(80.) && clip.size.height >= px(28.));
    clip
}

fn row_inside(window: &mut Window, index: usize) -> bool {
    let clip = visible_list(window);
    let row = window.find(("remote-entry", index));
    row.visible()
        && row.bounds().origin.y >= clip.origin.y
        && row.bounds().bottom() <= clip.bottom()
}

fn row_point(window: &mut Window, index: usize) -> Point<Pixels> {
    assert!(row_inside(window, index), "only target a fully visible row");
    let clip = visible_list(window);
    let row = window.find(("remote-entry", index)).bounds();
    // Stay in the name cell, away from the checkbox, header and right action.
    point(clip.origin.x + px(60.), row.center().y)
}

fn pointer_click(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
    window.dispatch_event(
        MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn wheel(window: &mut Window, cx: &mut App, position: Point<Pixels>, delta: ScrollDelta) {
    window.dispatch_event(
        MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        ScrollEvent {
            position,
            delta,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

async fn exercise_after_upload(cx: &mut TestAppContext, height: f32) {
    let mut scene = None;
    let h = Harness::new_with(cx, |cx, session, runtime| {
        let mounted = mount_scene(cx, session, runtime);
        let pair = (mounted.window, mounted.files.clone());
        scene = Some(mounted);
        pair
    });
    let _scene = scene.checked_option("retain the complete production Workspace");
    cx.simulate_window_resize(h.window, size(px(900.), px(height)));
    cx.run_until_parked();
    cx.update_window(h.window, |_, window, cx| {
        let settings = Settings {
            language: Language::En,
            theme: Theme::Dark,
            theme_family: ThemeId::new("lavender").checked("registered theme"),
            ..Settings::default()
        };
        i18n::set_language(Language::En, cx);
        crate::design::apply_settings(&settings, Some(window), cx);
        assert_eq!(window.viewport_size(), size(px(900.), px(height)));
    })
    .checked("use the reported native dimensions and appearance");
    h.idle(cx).await;
    for index in 0..7 {
        h.seed(
            &format!("/wheel-row-{index:02}.txt"),
            b"owned wheel fixture",
        );
    }
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    let source_bytes = [0x4b_u8; 34];
    let source = h.source("wheel-upload.txt", &source_bytes);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/wheel-upload.txt"), source_bytes);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none() && !panel.busy);
        assert_eq!(panel.transfer_jobs.len(), 1);
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Completed
        );
        assert!(!panel.queue_details_visible);
    });
    // Preserve the completed result. Do not refresh after upload, since the
    // reported state still shows its original listing beside the result.
    h.click(cx, "toggle-file-transfer-settings");
    let (first_path, last_path, last_index) = h.panel.read_with(cx, |panel, _| {
        assert!(panel.tool_section.is_none());
        let rows = super::super::browser::remote_indices(
            &panel.entries,
            panel.remote_show_hidden,
            panel.remote_sort,
        );
        assert!(
            rows.len() >= 7,
            "real listed entries provide an actual scroll extent"
        );
        (
            panel.entries[rows[0]].path.clone(),
            panel.entries[*rows.last().checked_option("last listed entry")]
                .path
                .clone(),
            rows.len() - 1,
        )
    });
    let writes_before = h.server.filesystem.atomic_writes_started();
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("file-tools-scroll").visible());
        assert!(
            window
                .try_find("file-transfer-card")
                .is_some_and(|card| card.visible())
                || window
                    .try_find("transfer-queue-list")
                    .is_some_and(|queue| queue.visible()),
            "the completed transfer result must remain in the rendered tools"
        );
        let mut modes_passed = Vec::new();
        for mode in ["pixels", "lines"] {
            let clip = visible_list(window);
            let position = point(clip.origin.x + px(60.), clip.center().y);
            wheel(
                window,
                cx,
                position,
                ScrollDelta::Pixels(point(px(0.), px(100_000.))),
            );
            assert!(
                row_inside(window, 0),
                "positive control must start on an actual visible row"
            );
            assert!(
                !row_inside(window, last_index),
                "the last row must initially need scrolling"
            );
            let position = row_point(window, 0);
            pointer_click(window, cx, position);
            assert_eq!(
                h.panel
                    .read(cx)
                    .selected
                    .as_ref()
                    .map(|entry| entry.path.as_str()),
                Some(first_path.as_str()),
                "the same pointer position used for wheel input must hit a real row"
            );
            let first_y = window.find(("remote-entry", 0_usize)).bounds().origin.y;
            for _ in 0..16 {
                let delta = if mode == "pixels" {
                    ScrollDelta::Pixels(point(px(0.), px(-120.)))
                } else {
                    ScrollDelta::Lines(point(0., -3.))
                };
                wheel(window, cx, position, delta);
                if row_inside(window, last_index) {
                    break;
                }
            }
            let moved = window.find(("remote-entry", 0_usize)).bounds().origin.y < first_y;
            let reached = row_inside(window, last_index);
            modes_passed.push((mode, moved, reached));
            if reached {
                let tail_position = row_point(window, last_index);
                pointer_click(window, cx, tail_position);
                assert_eq!(
                    h.panel
                        .read(cx)
                        .selected
                        .as_ref()
                        .map(|entry| entry.path.as_str()),
                    Some(last_path.as_str()),
                    "the last row must be actionable after scrolling, not merely present in AX"
                );
            }
            wheel(
                window,
                cx,
                position,
                ScrollDelta::Pixels(point(px(0.), px(100_000.))),
            );
            assert!(row_inside(window, 0), "scrolling must remain reversible");
        }
        assert!(
            modes_passed
                .iter()
                .all(|(_, moved, reached)| *moved && *reached),
            "real wheel events must move the remote list and expose its last row: {modes_passed:?}"
        );
        assert!(h.panel.read(cx).pending.is_none() && !h.panel.read(cx).busy);
    })
    .checked("wheel the real remote table while the completed result remains mounted");
    assert_eq!(h.server.filesystem.atomic_writes_started(), writes_before);
    assert_eq!(h.read("/wheel-upload.txt"), source_bytes);
    h.runtime
        .block_on(h.session.close())
        .checked("close owned SSH");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn remote_wheel_after_34_byte_upload_at_900x900(cx: &mut TestAppContext) {
    exercise_after_upload(cx, 900.).await;
}

#[gpui_kit::test]
async fn remote_wheel_after_34_byte_upload_at_900x580(cx: &mut TestAppContext) {
    exercise_after_upload(cx, 580.).await;
}
