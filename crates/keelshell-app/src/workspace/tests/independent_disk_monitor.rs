//! Controlled GPUI workspace: actual controls/input, with no native GUI claim.
use super::*;
use gpui_kit::{
    App, Pixels, ScrollDelta, Window,
    component::{WindowExt, input::AnyInputState},
};
use keelshell_core::Theme;

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

fn reveal_device(window: &mut Window, index: usize, cx: &mut App) {
    for _ in 0..60 {
        let viewport = window.find("monitor-scroll").bounds();
        let choices = window.find("monitor-disk-devices").bounds();
        if contained(choices, viewport) {
            break;
        }
        let direction = if choices.origin.y < viewport.origin.y {
            1.
        } else {
            -1.
        };
        window.dispatch_event(
            gpui_kit::ScrollWheelEvent {
                position: point(viewport.origin.x + px(2.), viewport.center().y),
                delta: ScrollDelta::Lines(point(0., direction)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
    let viewport = window.find("monitor-scroll").bounds();
    let choices = window.find("monitor-disk-devices").bounds();
    assert!(
        contained(choices, viewport),
        "outer wheel cannot reveal chooser {choices:?} in {viewport:?}"
    );
    for _ in 0..60 {
        let choices = window.find("monitor-disk-devices").bounds();
        let row = window.find(("monitor-disk-device", index));
        if row.visible() && contained(row.bounds(), choices) {
            break;
        }
        window.scroll(
            "monitor-disk-devices",
            ScrollDelta::Lines(point(0., -1.)),
            cx,
        );
        window.render_frame(cx);
    }
    let choices = window.find("monitor-disk-devices").bounds();
    let row = window.find(("monitor-disk-device", index));
    assert!(
        row.visible() && contained(row.bounds(), choices),
        "last disk is clipped: {:?} in {choices:?}",
        row.bounds()
    );
    window.click(("monitor-disk-device", index), cx);
    window.render_frame(cx);
}

pub(crate) async fn exercise(
    monitor: Entity<crate::monitor::MonitorPanel>,
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let monitor_id = monitor.entity_id();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = false;
            workspace
                .panels
                .entry(panes[0].terminal.entity_id())
                .or_default()
                .monitor = Some(monitor.clone());
            cx.notify();
        });
        window.render_frame(cx);
        if window.try_find("close-manager").is_some() {
            window.click("close-manager", cx);
            window.render_frame(cx);
        }
        window.click("command-input-container", cx);
        match window
            .focused_input(cx)
            .unwrap_or_else(|| panic!("focused production command"))
        {
            AnyInputState::Textarea(input) => input.update(cx, |input, cx| {
                input.replace("printf '保留中文 draft'\n# 未执行", window, cx);
            }),
            _ => panic!("production command is not a textarea"),
        }
    })
    .checked("mount sampled monitor beside sidebar and enter unsent text through focused input");
    cx.run_until_parked();
    let identity = fixture.workspace.read_with(cx, |view, cx| {
        (
            view.command.entity_id(),
            view.command.read(cx).value().to_string(),
            view.command_revision,
            view.tabs.iter().map(Entity::entity_id).collect::<Vec<_>>(),
        )
    });
    assert_eq!(identity.1, "printf '保留中文 draft'\n# 未执行");
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| i18n::set_language(language, cx));
        for (theme, id) in [
            (Theme::Dark, "theme-dark"),
            (Theme::Light, "theme-light"),
            (Theme::System, "theme-system"),
        ] {
            cx.update_window(fixture.window, |_, window, cx| {
                window.render_frame(cx);
                window.click(id, cx);
            })
            .checked("switch appearance with actual production toolbar button");
            cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
                !fixture.workspace.read(cx).saving
            })
            .await;
            cx.update_window(fixture.window, |_, window, cx| {
                window.render_frame(cx);
                let scene = format!("{language:?}/{theme:?}/900x580/manager-closed");
                let terminal = window.find(("terminal-pane", panes[0].terminal.entity_id())).bounds();
                assert!(terminal.size.width >= px(200.) && terminal.size.height >= px(80.), "{scene}: terminal {terminal:?}");
                for id in ["new-session", "theme-dark", "theme-light", "theme-system", "language", "refresh-monitor", "pause-monitor", "command-input-container"] {
                    let control = window.find(id);
                    assert!(control.visible() && contained(control.bounds(), window.bounds()), "{scene}: {id} {:?}", control.bounds());
                }
                reveal_device(window, 11, cx);
                assert_eq!(window.find(("monitor-disk-device", 11_usize)).label(), Some("fixture11 · 8:11"));
                for _ in 0..60 {
                    let viewport = window.find("monitor-scroll").bounds();
                    let metrics = window.find("monitor-disk-metrics").bounds();
                    if contained(metrics, viewport) { break; }
                    let direction = if metrics.origin.y < viewport.origin.y { 1. } else { -1. };
                    window.dispatch_event(gpui_kit::ScrollWheelEvent {
                        position: point(viewport.origin.x + px(2.), viewport.center().y),
                        delta: ScrollDelta::Lines(point(0., direction)),
                        ..Default::default()
                    }.to_platform_input(), cx);
                    window.render_frame(cx);
                }
                let metrics = window.find("monitor-disk-metrics").bounds();
                let column = window.find("monitor-column").bounds();
                let viewport = window.find("monitor-scroll").bounds();
                assert!(contained(metrics, viewport), "{scene}: metrics {metrics:?} are not fully readable in {viewport:?}");
                assert!(metrics.origin.x >= column.origin.x && metrics.right() <= column.right(), "{scene}: metrics {metrics:?} in {column:?}");
                for id in ["monitor-disk-read-rate", "monitor-disk-write-rate", "monitor-disk-read-iops", "monitor-disk-write-iops", "monitor-disk-read-duration", "monitor-disk-write-duration", "monitor-disk-inflight"] {
                    let metric = window.find(id);
                    assert!(metric.visible() && contained(metric.bounds(), viewport), "{scene}: {id} {:?}", metric.bounds());
                }
                let status = window.find("monitor-operation-status");
                assert!(status.visible() && contained(status.bounds(), window.bounds()));
                let view = fixture.workspace.read(cx);
                assert_eq!(view.state.settings.theme, theme);
                assert_eq!(view.panels[&panes[0].terminal.entity_id()].monitor.as_ref().map(Entity::entity_id), Some(monitor_id));
                assert_eq!((view.command.entity_id(), view.command.read(cx).value().to_string(), view.command_revision, view.tabs.iter().map(Entity::entity_id).collect::<Vec<_>>()), identity);
            }).checked("minimum workspace keeps sampled monitor, last disk, actual command draft and terminal");
            cx.run_until_parked();
            assert_eq!(
                fixture
                    .store
                    .load()
                    .checked("read saved production appearance")
                    .settings
                    .theme,
                theme
            );
        }
    }
    assert!(
        panes.iter().all(|pane| writes(pane).is_empty()),
        "appearance and unsent command must not send PTY bytes"
    );
}
