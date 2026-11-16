//! Controlled SSH verifies the Darwin listener UI without claiming native lsof.

use super::*;
use crate::monitor::{CapabilityState, SocketInfo};
use keelshell_session::monitor::CapabilityReason;

pub(super) fn wire() -> &'static str {
    "@@KS:darwin-sockets:1@@\np42\0cfixture\0\nf3\0tIPv4\0PTCP\0n127.0.0.1:2222\0TST=LISTEN\0\n@@KS:lsof-status@@\n0\n@@KS:end@@\n"
}

fn harness(cx: &mut TestAppContext) -> Harness {
    Harness::with_control(
        cx,
        Arc::new(Control {
            darwin: AtomicBool::new(true),
            ..Control::default()
        }),
    )
}

#[gpui_kit::test]
async fn darwin_socket_button_reads_tcp_without_offering_linux_actions(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    assert_eq!(h.control.sockets.load(Ordering::Acquire), 0);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("refresh-sockets").label(), Some("读取监听端口"));
        window.click("refresh-sockets", cx);
    })
    .checked("explicit Darwin socket button");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.paused);
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.sockets.len(), 1);
        assert_eq!(panel.sockets[0].local, "127.0.0.1:2222");
        assert_eq!(panel.sockets[0].state, "LISTEN");
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("monitor-socket", 0_usize)).is_some());
        assert!(window.try_find(("probe-socket", 0_usize)).is_none());
        assert!(window.try_find(("review-process", 42_usize)).is_none());
    })
    .checked("Darwin read-only listener row");
    assert_eq!(h.control.sockets.load(Ordering::Acquire), 1);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_missing_lsof_stays_cached_until_explicit_recheck(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.control.socket_failure.store(66, Ordering::Release);
    h.load_sockets(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.sockets.state,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        );
        assert!(panel.snapshot.is_some());
        assert!(!panel.processes.is_empty());
    });
    let before = h.control.sockets.load(Ordering::Acquire);
    h.control.socket_failure.store(0, Ordering::Release);
    h.load_sockets(cx);
    h.idle(cx).await;
    assert_eq!(h.control.sockets.load(Ordering::Acquire), before);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    assert_eq!(h.control.sockets.load(Ordering::Acquire), before + 1);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.paused, "explicit recheck does not resume polling");
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.sockets.len(), 1);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_socket_read_does_not_depend_on_python_resource_collection(cx: &mut TestAppContext) {
    let mut h = Harness::with_control(
        cx,
        Arc::new(Control {
            darwin: AtomicBool::new(true),
            fail_resources: AtomicBool::new(true),
            ..Control::default()
        }),
    );
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.snapshot.is_none());
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.sockets.len(), 1);
    });
    let before = h.control.commands.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| {
        panel.run(Job::Probe(panel.sockets[0].clone()), cx);
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("probe-socket", 0_usize)).is_none());
    })
    .checked("no Linux actions without a Linux sample");
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_pausing_pending_socket_read_discards_late_list_and_capability(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| panel.capabilities);
    h.control.hold_sockets.store(true, Ordering::Release);
    h.load_sockets(cx);
    cx.wait_for(h.window, Duration::from_secs(4), |_, cx| {
        h.control.sockets.load(Ordering::Acquire) == 1 && h.panel.read(cx).busy
    })
    .await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(false, cx);
        panel.set_paused(true, cx);
    });
    h.control.hold_sockets.store(false, Ordering::Release);
    h.control.socket_release.add_permits(1);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.sockets.is_empty());
        assert_eq!(panel.capabilities, before);
        assert!(panel.paused);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn unknown_platform_rejects_probe_even_with_a_displayed_tcp_row(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        panel.snapshot = None;
        panel.sockets = vec![SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*:*".into(),
            process: None,
        }];
        panel.run(Job::Probe(panel.sockets[0].clone()), cx);
        assert!(!panel.busy);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("probe-socket", 0_usize)).is_none());
    })
    .checked("unknown platform remains read-only");
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}
