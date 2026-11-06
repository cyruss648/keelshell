//! Production GPUI controls and real pinned loopback SSH, with controlled frames.
//! They prove dispatch/adoption and presentation, not native/Linux packet acceptance.
use super::*;
use keelshell_core::{Language, NetworkDiagnosticKind, Settings, Theme};
use keelshell_session::network_diagnostic::{
    NetworkDiagnosticStatus, NetworkDiagnosticTracerouteReason,
};

pub(super) fn response(input: &serde_json::Value, mode: usize) -> serde_json::Value {
    let options = &input["traceroute"];
    let address = if options["family"] == "ipv6" {
        "::1"
    } else {
        "127.0.0.1"
    };
    let (status, reason) = match mode {
        1 => ("utility_unavailable", "utility_missing"),
        2 => ("unsupported_environment", "unsupported_options"),
        3 => ("timeout", "deadline"),
        4 => ("traceroute_hop_limit", "hop_limit"),
        5 => ("traceroute_unreachable", "reported_unreachable"),
        6 => ("cleanup_unknown", "cleanup_unknown"),
        _ => ("success", "destination_responded"),
    };
    let absent = matches!(mode, 1 | 2 | 3 | 6);
    let mut rows = Vec::new();
    if !absent {
        let count = if mode == 4 {
            options["max_hops"].as_u64().unwrap_or(0)
        } else {
            1
        };
        for ttl in 1..=count {
            let probes: Vec<_> = (0..options["probes_per_hop"].as_u64().unwrap_or(0))
                .map(|index| {
                    if mode == 4 || index > 0 {
                        serde_json::json!({"address":null,"rtt_us":null,"annotation":null})
                    } else {
                        serde_json::json!({"address":address,"rtt_us":10,"annotation":if mode==5{Some("!X")}else{None}})
                    }
                }).collect();
            rows.push(serde_json::json!({"ttl":ttl,"probes":probes}));
        }
    }
    serde_json::json!({
        "version":1,"status":status,
        "addresses":if absent{serde_json::json!([])}else{serde_json::json!([{"family":if address=="::1"{"IPv6"}else{"IPv4"},"address":address}])},
        "peer":null,"tls":null,"http":null,"icmp":null,"limited":false,
        "timing":{"resolve_ms":if absent{None}else{Some(1)},"connect_ms":null,"tls_ms":null,"headers_ms":null,"total_ms":5},
        "traceroute":{"reason":reason,"implementation":if absent{None}else{Some("linux_traceroute_2_1_5")},
            "address":if absent{None}else{Some(address)},"hops":rows}
    })
}

fn configure(h: &Harness, cx: &mut TestAppContext) {
    h.click("protocol-expand", cx);
    h.click("protocol-mode-traceroute", cx);
    h.set_endpoint("fixture.test", cx);
}

fn set_limit(h: &Harness, field: &str, value: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            let input = match field {
                "hops" => &panel.trace_hops,
                "probes" => &panel.trace_probes,
                "wait" => &panel.trace_wait,
                _ => unreachable!("fixed test input field"),
            };
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        });
    })
    .checked("set production traceroute limit");
}

#[gpui_kit::test]
async fn traceroute_gui_review_captures_all_options_and_dispatches_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    configure(&h, cx);
    h.click("trace-protocol-icmp", cx);
    h.click("trace-family-v6", cx);
    set_limit(&h, "hops", "4", cx);
    set_limit(&h, "probes", "3", cx);
    set_limit(&h, "wait", "250", cx);
    h.click("protocol-preview", cx);
    let approved = h
        .panel
        .read_with(cx, |panel, _| panel.review.clone())
        .unwrap_or_else(|| panic!("reviewed request"));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    assert_eq!(approved.kind(), NetworkDiagnosticKind::Traceroute);
    let options = approved
        .traceroute_options()
        .unwrap_or_else(|| panic!("reviewed options"));
    assert_eq!(
        (
            options.max_hops(),
            options.probes_per_hop(),
            options.wait_ms()
        ),
        (4, 3, 250)
    );
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("protocol-review-traceroute")
                .bounds()
                .size
                .width
                > px(0.)
        );
    })
    .checked("actual command review rendered");
    h.control.gate.add_permits(1);
    h.click("protocol-confirm", cx);
    h.idle(cx).await;
    assert_eq!(
        h.control
            .reviewed_requests
            .lock()
            .checked("fixture request lock")
            .as_slice(),
        &[serde_json::to_value(&approved).checked("approved wire request")]
    );
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.result.as_ref().is_some_and(|report| {
                report.request == approved
                    && report
                        .traceroute
                        .as_ref()
                        .is_some_and(|trace| trace.address.as_deref() == Some("::1"))
            }))
    );
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "protocol-traceroute-reason",
            "protocol-traceroute-address",
            "protocol-traceroute-hops",
        ] {
            assert!(window.find(id).bounds().size.width > px(0.));
        }
    })
    .checked("actual structured traceroute result rendered");
    h.panel.update(cx, |panel, cx| panel.confirm(cx));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    h.finish();
}

#[gpui_kit::test]
async fn traceroute_gui_injection_limits_and_every_draft_edit_revoke_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    configure(&h, cx);
    for endpoint in [
        "--help",
        "a;id",
        "$(id)",
        "fe80::1%eth0",
        "https://user:secret@fixture.test/",
    ] {
        h.set_endpoint(endpoint, cx);
        h.click("protocol-preview", cx);
        assert!(h.panel.read_with(cx, |panel, _| panel.review.is_none()));
    }
    h.set_endpoint("fixture.test", cx);
    for (field, invalid, valid) in [
        ("hops", "17", "6"),
        ("probes", "4", "1"),
        ("wait", "500;id", "500"),
        ("wait", "+500", "500"),
        ("wait", "1001", "500"),
    ] {
        set_limit(&h, field, invalid, cx);
        h.click("protocol-preview", cx);
        assert!(h.panel.read_with(cx, |panel, _| panel.review.is_none()));
        set_limit(&h, field, valid, cx);
        h.click("protocol-preview", cx);
        assert!(h.panel.read_with(cx, |panel, _| panel.review.is_some()));
        set_limit(&h, field, invalid, cx);
        set_limit(&h, field, valid, cx);
        h.panel.update(cx, |panel, cx| panel.confirm(cx));
        assert!(h.panel.read_with(cx, |panel, _| panel.review.is_none()));
    }
    set_limit(&h, "hops", "16", cx);
    set_limit(&h, "probes", "3", cx);
    h.click("protocol-preview", cx);
    assert!(
        h.panel.read_with(cx, |panel, _| panel.review.is_none()),
        "cumulative wait cap"
    );
    set_limit(&h, "hops", "6", cx);
    set_limit(&h, "probes", "1", cx);
    for button in ["trace-protocol-icmp", "trace-family-v6"] {
        h.click("protocol-preview", cx);
        h.click(button, cx);
        h.panel.update(cx, |panel, cx| panel.confirm(cx));
        assert!(h.panel.read_with(cx, |panel, _| panel.review.is_none()));
    }
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    h.finish();
}

#[gpui_kit::test]
async fn traceroute_gui_bilingual_registered_themes_render_without_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    configure(&h, cx);
    h.click("protocol-preview", cx);
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| crate::i18n::set_language(language, cx));
        for definition in crate::design::registry() {
            for appearance in [Theme::Light, Theme::Dark] {
                cx.update_window(h.window, |_, window, cx| {
                    let settings = Settings {
                        theme_family: keelshell_core::ThemeId::new(definition.id)
                            .checked("registered trace theme"),
                        theme: appearance,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    window.render_frame(cx);
                    assert_eq!(
                        window.find("protocol-confirm").label(),
                        Some(if language == Language::ZhCn {
                            "确认执行"
                        } else {
                            "Confirm probe"
                        })
                    );
                    assert!(
                        window
                            .find("protocol-review-traceroute")
                            .bounds()
                            .size
                            .width
                            > px(0.)
                    );
                })
                .checked("registered traceroute theme and language");
            }
        }
    }
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    h.finish();
}

#[gpui_kit::test]
async fn traceroute_gui_missing_options_timeout_hop_limit_unreachable_and_unknown_are_distinct(
    cx: &mut TestAppContext,
) {
    for (mode, status, reason) in [
        (
            1,
            NetworkDiagnosticStatus::UtilityUnavailable,
            NetworkDiagnosticTracerouteReason::UtilityMissing,
        ),
        (
            2,
            NetworkDiagnosticStatus::UnsupportedEnvironment,
            NetworkDiagnosticTracerouteReason::UnsupportedOptions,
        ),
        (
            3,
            NetworkDiagnosticStatus::Timeout,
            NetworkDiagnosticTracerouteReason::Deadline,
        ),
        (
            4,
            NetworkDiagnosticStatus::TracerouteHopLimit,
            NetworkDiagnosticTracerouteReason::HopLimit,
        ),
        (
            5,
            NetworkDiagnosticStatus::TracerouteUnreachable,
            NetworkDiagnosticTracerouteReason::ReportedUnreachable,
        ),
        (
            6,
            NetworkDiagnosticStatus::CleanupUnknown,
            NetworkDiagnosticTracerouteReason::CleanupUnknown,
        ),
    ] {
        let h = Harness::new(cx);
        configure(&h, cx);
        h.control.traceroute_mode.store(mode, Ordering::Release);
        h.click("protocol-preview", cx);
        h.control.gate.add_permits(1);
        h.click("protocol-confirm", cx);
        h.idle(cx).await;
        assert!(
            h.panel
                .read_with(cx, |panel, _| panel.result.as_ref().is_some_and(|report| {
                    report.status == status
                        && report
                            .traceroute
                            .as_ref()
                            .is_some_and(|trace| trace.reason == reason)
                })),
            "{mode}: incorrect trace outcome"
        );
        if mode == 6 {
            assert!(h.panel.read_with(cx, |panel, _| panel.session.is_none()));
        }
        h.finish();
    }
}

#[gpui_kit::test]
async fn traceroute_gui_cancel_edit_and_missing_terminal_discard_late_results(
    cx: &mut TestAppContext,
) {
    for mutation in ["cancel", "input", "session", "exit"] {
        let h = Harness::new(cx);
        configure(&h, cx);
        h.click("protocol-preview", cx);
        if mutation == "exit" {
            h.control.exit_mode.store(1, Ordering::Release);
        }
        h.click("protocol-confirm", cx);
        cx.wait_for(h.window, Duration::from_secs(3), |_, _| {
            h.control.requests.load(Ordering::Acquire) == 1
        })
        .await;
        match mutation {
            "cancel" => h.click("protocol-cancel", cx),
            "input" => {
                set_limit(&h, "hops", "5", cx);
                set_limit(&h, "hops", "6", cx);
            }
            "session" => h.monitor.update(cx, |panel, cx| panel.suspend(cx)),
            "exit" => {}
            _ => unreachable!("fixed trace mutation"),
        }
        h.control.gate.add_permits(1);
        h.idle(cx).await;
        assert!(h.panel.read_with(cx, |panel, _| panel.result.is_none()
            && panel.review.is_none()));
        if mutation == "exit" {
            assert!(
                h.panel.read_with(cx, |panel, _| panel.session.is_none()),
                "unverifiable exit retires authority"
            );
        }
        h.panel.update(cx, |panel, cx| panel.confirm(cx));
        assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
        h.finish();
    }
}

#[gpui_kit::test]
async fn traceroute_gui_real_workspace_route_trust_tab_and_selection_revision_revoke(
    cx: &mut TestAppContext,
) {
    use crate::workspace::tests::protocol_diagnostics::{
        change_selection, independent_mutate_pending_authority,
        independent_reconnect_pending_reply_fixture,
    };
    for mutation in ["route", "trust", "closed_tab", "aba"] {
        let h = Harness::new(cx);
        cx.update_window(h.window, |_, window, _| window.remove_window())
            .checked("retire initial trace window");
        let (window, monitor, workspace) =
            independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.shells_ready.load(Ordering::Acquire) == 1
        })
        .await;
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.kind = NetworkDiagnosticKind::Traceroute;
                panel
                    .endpoint
                    .update(cx, |input, cx| input.set_value("fixture.test", window, cx));
                panel.preview(cx);
                assert!(panel.review.is_some());
                panel.confirm(cx);
            });
        })
        .checked("review against actual workspace captured route and SSH");
        h.control.gate.add_permits(1);
        cx.wait_for(window, Duration::from_secs(5), |_, cx| {
            panel.read(cx).cancel.is_none()
        })
        .await;
        assert!(panel.read_with(cx, |panel, _| {
            panel
                .result
                .as_ref()
                .is_some_and(|r| r.status == NetworkDiagnosticStatus::Success)
        }));
        panel.update(cx, |panel, cx| {
            panel.preview(cx);
            panel.confirm(cx);
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.requests.load(Ordering::Acquire) == 2
        })
        .await;
        cx.update_window(window, |_, window, cx| {
            workspace.update(cx, |view, cx| {
                if mutation == "aba" {
                    change_selection(view, "aba", window, cx);
                } else {
                    independent_mutate_pending_authority(view, mutation, h.port, window, cx);
                }
            });
        })
        .checked("mutate actual workspace authority before late trace reply");
        h.control.gate.add_permits(1);
        cx.wait_for(window, Duration::from_secs(5), |_, cx| {
            panel.read(cx).cancel.is_none()
        })
        .await;
        assert!(
            panel.read_with(cx, |panel, _| panel.result.is_none()
                && panel.review.is_none()),
            "{mutation}: late trace adopted"
        );
        panel.update(cx, |panel, cx| panel.confirm(cx));
        assert_eq!(h.control.requests.load(Ordering::Acquire), 2);
        cx.update_window(window, |_, window, _| window.remove_window())
            .checked("close controlled trace workspace");
        h.finish();
    }
}

#[gpui_kit::test]
async fn traceroute_gui_stale_workspace_review_never_dispatches_before_next_render(
    cx: &mut TestAppContext,
) {
    use crate::workspace::tests::protocol_diagnostics::{
        change_selection, independent_mutate_pending_authority,
        independent_reconnect_pending_reply_fixture,
    };
    for mutation in ["route", "trust", "closed_tab", "aba"] {
        let h = Harness::new(cx);
        cx.update_window(h.window, |_, window, _| window.remove_window())
            .checked("retire initial traceroute review window");
        let (window, monitor, workspace) =
            independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.shells_ready.load(Ordering::Acquire) == 1
        })
        .await;
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.kind = NetworkDiagnosticKind::Traceroute;
                panel
                    .endpoint
                    .update(cx, |input, cx| input.set_value("fixture.test", window, cx));
                panel.preview(cx);
                assert!(
                    panel.review.is_some(),
                    "current bound workspace must admit review"
                );
            });
            workspace.update(cx, |view, cx| {
                if mutation == "aba" {
                    change_selection(view, "aba", window, cx);
                } else {
                    independent_mutate_pending_authority(view, mutation, h.port, window, cx);
                }
            });
            panel.update(cx, |panel, cx| {
                panel.confirm(cx);
                assert!(panel.review.is_none() && panel.cancel.is_none());
            });
        })
        .checked("stale traceroute review checked without an intervening draw");
        assert_eq!(
            h.control.requests.load(Ordering::Acquire),
            0,
            "{mutation}: stale exec"
        );
        cx.update_window(window, |_, window, _| window.remove_window())
            .checked("close stale traceroute review workspace");
        h.finish();
    }
}
