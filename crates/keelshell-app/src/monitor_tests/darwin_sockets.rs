//! Controlled SSH verifies the Darwin socket UI without claiming native lsof.

use super::*;
use crate::monitor::{CapabilityState, SocketInfo};
use keelshell_session::monitor::CapabilityReason;

pub(super) fn wire() -> &'static str {
    concat!(
        "@@KS:darwin-sockets:2@@\np42\0cfixture\0\n",
        "f3\0tIPv4\0PTCP\0n127.0.0.1:2222\0TST=LISTEN\0\n",
        "f4\0tIPv4\0PUDP\0n*:5353\0\n",
        "f5\0tIPv6\0PUDP\0n[::1]:53000->[::1]:5353\0\n",
        "f6\0tIPv4\0PUDP\0n*:*\0\n",
        "@@KS:lsof-status@@\n0\n@@KS:end@@\n",
    )
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

fn scroll_to_pager(window: &mut gpui_kit::Window, id: &'static str, cx: &mut gpui_kit::App) {
    for _ in 0..100 {
        let target = window.find(id).bounds();
        let viewport = window.find("monitor-scroll").bounds();
        if target.origin.y >= viewport.origin.y && target.bottom() <= viewport.bottom() {
            assert!(window.find(id).visible());
            return;
        }
        let direction = if target.origin.y < viewport.origin.y {
            4.
        } else {
            -4.
        };
        window.scroll(
            "monitor-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., direction)),
            cx,
        );
    }
    panic!("socket pager must be reachable through the production scroll view");
}

#[gpui_kit::test]
async fn darwin_socket_button_reads_tcp_and_udp_without_offering_linux_actions(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    assert_eq!(h.control.sockets.load(Ordering::Acquire), 0);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("refresh-sockets").label(), Some("读取端口"));
        window.click("refresh-sockets", cx);
    })
    .checked("explicit Darwin socket button");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.paused);
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.sockets.len(), 3);
        assert_eq!(panel.sockets[0].local, "127.0.0.1:2222");
        assert_eq!(panel.sockets[0].state, "LISTEN");
        assert_eq!(panel.sockets[1].protocol, "udp");
        assert_eq!(panel.sockets[1].local, "0.0.0.0:5353");
        assert_eq!(panel.sockets[1].state, "BOUND");
        assert_eq!(panel.sockets[2].state, "PEER");
        assert_eq!(panel.sockets[2].peer, "[::1]:5353");
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for index in 0..3_usize {
            assert!(window.try_find(("monitor-socket", index)).is_some());
            assert!(window.try_find(("probe-socket", index)).is_none());
        }
        assert!(window.try_find(("monitor-socket", 3_usize)).is_none());
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
        assert_eq!(panel.sockets.len(), 3);
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
        assert_eq!(panel.sockets.len(), 3);
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

#[gpui_kit::test]
async fn darwin_udp_rows_retain_endpoints_across_themes_and_languages_without_io(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    let before = h.control.commands.load(Ordering::Acquire);
    let sockets = h.panel.read_with(cx, |panel, _| panel.sockets.clone());
    for family in crate::design::registry() {
        for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    let settings = keelshell_core::Settings {
                        theme_family: keelshell_core::ThemeId::new(family.id)
                            .checked("Darwin UDP theme"),
                        theme: mode,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    crate::i18n::set_language(language, cx);
                    h.panel
                        .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                    window.render_frame(cx);
                    let english = language == Language::En;
                    let bound = if english { "Bound" } else { "已绑定" };
                    let peer = if english {
                        "Peer set"
                    } else {
                        "已指定对端"
                    };
                    assert_eq!(
                        window.find(("monitor-socket-state", 1_usize)).label(),
                        Some(bound)
                    );
                    assert_eq!(
                        window.find(("monitor-socket-state", 2_usize)).label(),
                        Some(peer)
                    );
                    assert_eq!(
                        window.find(("monitor-socket", 1_usize)).label(),
                        Some(format!("udp: {bound}, 0.0.0.0:5353, fixture (pid=42)").as_str())
                    );
                    assert_eq!(
                        window.find(("monitor-socket", 2_usize)).label(),
                        Some(
                            format!("udp: {peer}, [::1]:53000 → [::1]:5353, fixture (pid=42)")
                                .as_str()
                        )
                    );
                    for index in 0..3_usize {
                        assert!(window.try_find(("probe-socket", index)).is_none());
                    }
                })
                .checked("Darwin UDP themed labels");
                assert_eq!(
                    h.panel.read_with(cx, |panel, _| panel.sockets.clone()),
                    sockets
                );
            }
        }
    }
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_udp_probe_dispatch_stays_read_only_with_and_without_resource_sample(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    let before = h.control.commands.load(Ordering::Acquire);
    for has_sample in [true, false] {
        h.panel.update(cx, |panel, cx| {
            if !has_sample {
                panel.snapshot = None;
            }
            for index in [1, 2] {
                panel.run(Job::Probe(panel.sockets[index].clone()), cx);
                assert!(!panel.busy);
                assert!(panel.pending.is_none());
            }
        });
    }
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn socket_pagination_reaches_every_cached_row_without_ssh_and_clamps_on_refresh(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        panel.replace_sockets(
            (0..25)
                .map(|index| SocketInfo {
                    protocol: "udp".into(),
                    state: "BOUND".into(),
                    local: format!("127.0.0.1:{}", 53000 + index),
                    peer: "*:*".into(),
                    process: Some("fixture (pid=42)".into()),
                })
                .collect(),
        );
    });
    let before = h.control.commands.load(Ordering::Acquire);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("socket-page-previous").disabled(), Some(true));
        assert_eq!(
            window.find("monitor-socket-page").label(),
            Some("第 1 / 3 页，共 25 个端口条目")
        );
        for index in 0..12_usize {
            assert!(window.try_find(("monitor-socket", index)).is_some());
        }
        assert!(window.try_find(("monitor-socket", 12_usize)).is_none());
        scroll_to_pager(window, "socket-page-next", cx);
        window.click("socket-page-next", cx);
        window.render_frame(cx);
        for index in 12..24_usize {
            assert!(window.try_find(("monitor-socket", index)).is_some());
        }
        assert!(window.try_find(("monitor-socket", 0_usize)).is_none());
        scroll_to_pager(window, "socket-page-next", cx);
        window.click("socket-page-next", cx);
        window.render_frame(cx);
        assert_eq!(window.find("socket-page-next").disabled(), Some(true));
        assert!(window.try_find(("monitor-socket", 24_usize)).is_some());
        assert!(window.try_find(("monitor-socket", 23_usize)).is_none());
        crate::i18n::set_language(Language::En, cx);
        h.panel
            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("monitor-socket-page").label(),
            Some("Page 3 of 3, 25 socket entries")
        );
        assert_eq!(
            window.find("socket-page-previous").label(),
            Some("Previous socket page")
        );
        scroll_to_pager(window, "socket-page-previous", cx);
        window.click("socket-page-previous", cx);
        window.render_frame(cx);
        assert!(window.try_find(("monitor-socket", 12_usize)).is_some());
    })
    .checked("cached socket pagination");
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    h.load_sockets(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.socket_page, 0);
        assert_eq!(panel.sockets.len(), 3);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("socket-page-next").is_none());
        assert!(window.try_find(("monitor-socket", 0_usize)).is_some());
    })
    .checked("refreshed socket page is clamped");
    assert_eq!(h.control.commands.load(Ordering::Acquire), before + 1);
    h.close_history_fixture(cx).await;
}
