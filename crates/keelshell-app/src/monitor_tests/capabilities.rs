//! Real SSH packets exercise production collection and the panel's authority.

use super::*;
use crate::monitor::{Capability, CapabilityState, Outcome};
use keelshell_session::monitor::{CapabilityReason, LinuxMonitor};
use std::time::Instant;

#[gpui_kit::test]
async fn pausing_recheck_between_commands_cancels_unsent_ps_and_ss(cx: &mut TestAppContext) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h
        .panel
        .read_with(cx, |panel, _| (panel.capabilities, panel.mcp_sample_id));
    let commands = h.control.commands.load(Ordering::Acquire);
    h.control.hold_resources.store(true, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    cx.wait_for(h.window, Duration::from_secs(4), |_, cx| {
        h.control.commands.load(Ordering::Acquire) == commands + 1 && h.panel.read(cx).busy
    })
    .await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(false, cx);
        panel.set_paused(true, cx);
    });
    h.control.hold_resources.store(false, Ordering::Release);
    h.control.resource_release.add_permits(1);
    h.idle(cx).await;
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 1);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!((panel.capabilities, panel.mcp_sample_id), before);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn df_failures_keep_core_samples_live_and_cache_only_the_failed_item(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    for (status, reason) in [
        (127, CapabilityReason::CommandMissing),
        (2, CapabilityReason::UnsupportedOptions),
        (126, CapabilityReason::PermissionDenied),
    ] {
        h.control.df_failure.store(status, Ordering::Release);
        let previous = h.panel.read_with(cx, |panel, _| panel.mcp_sample_id);
        h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, _| {
            assert_eq!(
                panel.capabilities.resources.state,
                CapabilityState::Available
            );
            assert_eq!(
                panel.capabilities.filesystems.state,
                CapabilityState::Unavailable(reason)
            );
            assert_ne!(panel.mcp_sample_id, previous);
            let snapshot = panel
                .snapshot
                .as_ref()
                .unwrap_or_else(|| panic!("core snapshot survived df"));
            assert!(snapshot.filesystems.is_empty());
            assert!(snapshot.memory.used_bytes().is_some());
            assert!(!snapshot.networks.is_empty());
            assert!(snapshot.disk_io.is_ok());
        });
        let df_commands = h.control.df_commands.load(Ordering::Acquire);
        h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
        h.idle(cx).await;
        assert_eq!(h.control.df_commands.load(Ordering::Acquire), df_commands);
        h.panel.read_with(cx, |panel, _| {
            assert_eq!(panel.rates.cpu_busy_percent, Some(50.));
            assert_eq!(panel.rates.networks[0].received_bytes_per_second, Some(0.));
            assert_eq!(
                panel.capabilities.filesystems.state,
                CapabilityState::Unavailable(reason)
            );
        });
    }
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn ps_ss_negative_capabilities_require_explicit_recheck_and_recover(cx: &mut TestAppContext) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.control.process_failure.store(2, Ordering::Release);
    h.control.socket_failure.store(66, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Unavailable(CapabilityReason::UnsupportedOptions)
        );
        assert_eq!(
            panel.capabilities.sockets.state,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        );
        assert!(panel.snapshot.is_some());
        assert!(panel.sockets.is_empty());
    });
    let commands = h.control.commands.load(Ordering::Acquire);
    let process_commands = h.control.process_commands.load(Ordering::Acquire);
    let sockets = h.control.sockets.load(Ordering::Acquire);
    h.control.process_failure.store(0, Ordering::Release);
    h.control.socket_failure.store(0, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.load_sockets(cx);
    h.idle(cx).await;
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 1);
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        process_commands
    );
    assert_eq!(h.control.sockets.load(Ordering::Acquire), sockets);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("redetect-monitor", cx);
    })
    .checked("explicit production capability recheck");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.paused, "rechecking never resumes automatic polling");
        for capability in [
            Capability::Resources,
            Capability::Filesystems,
            Capability::Processes,
            Capability::Sockets,
        ] {
            assert_eq!(
                panel.capabilities.observation(capability).state,
                CapabilityState::Available
            );
        }
        assert_eq!(panel.sockets.len(), 1);
    });
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        process_commands + 1
    );
    assert_eq!(h.control.sockets.load(Ordering::Acquire), sockets + 1);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn recheck_pause_aba_and_retirement_discard_held_capability_responses(
    cx: &mut TestAppContext,
) {
    for suspend in [false, true] {
        let mut h = Harness::new(cx);
        h.idle(cx).await;
        h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
        let previous = h.panel.read_with(cx, |panel, _| {
            (panel.capabilities, panel.mcp_sample_id, panel.last_sample)
        });
        let monitor = h
            .panel
            .read_with(cx, |panel, _| panel.monitor.clone())
            .unwrap_or_else(|| panic!("live monitor"));
        let commands = h.control.commands.load(Ordering::Acquire);
        h.control.hold_sockets.store(true, Ordering::Release);
        h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
        cx.wait_for(h.window, Duration::from_secs(4), |_, cx| {
            h.control.commands.load(Ordering::Acquire) == commands + 3 && h.panel.read(cx).busy
        })
        .await;
        h.panel.update(cx, |panel, cx| {
            if suspend {
                panel.suspend(cx);
            } else {
                panel.set_paused(false, cx);
                panel.set_paused(true, cx);
            }
        });
        h.control.hold_sockets.store(false, Ordering::Release);
        h.control.socket_release.add_permits(1);
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, _| {
            assert_eq!(
                (panel.capabilities, panel.mcp_sample_id, panel.last_sample),
                previous
            );
            assert!(panel.pending.is_none());
        });
        let cached = h._runtime.block_on(monitor.capabilities());
        assert_eq!(
            cached.sockets.state,
            CapabilityState::Unknown,
            "retired in-flight check cannot populate the cache"
        );
        assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 3);
        h.close_history_fixture(cx).await;
    }
}

#[gpui_kit::test]
async fn capability_cache_is_shared_by_clones_and_fresh_for_a_new_authenticated_panel(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.control.process_failure.store(126, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("live monitor"));
    let clone = monitor.clone();
    let process_commands = h.control.process_commands.load(Ordering::Acquire);
    let result = h._runtime.block_on(clone.processes());
    assert!(matches!(
        result,
        Err(
            keelshell_session::monitor::MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::PermissionDenied,
                ..
            }
        )
    ));
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        process_commands
    );
    let empty = h
        ._runtime
        .block_on(LinuxMonitor::new(h.session.clone()).capabilities());
    assert_eq!(empty.processes.state, CapabilityState::Unknown);
    let mut fresh = Harness::new(cx);
    fresh.idle(cx).await;
    fresh.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Available
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Unknown);
    });
    fresh.close_history_fixture(cx).await;
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn expired_or_future_capability_receipts_do_not_replace_current_state(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("live monitor"));
    let report = h._runtime.block_on(monitor.capability_report());
    h.panel.update(cx, |panel, cx| {
        let previous = panel.capabilities;
        let now = Instant::now();
        for completed_at in [now - Duration::from_secs(11), now + Duration::from_secs(11)] {
            panel.adopt_outcome(
                Outcome::Sockets {
                    result: Ok(Vec::new()),
                    capabilities: report.clone(),
                },
                Some(panel.sample_generation),
                completed_at,
                cx,
            );
            assert_eq!(panel.capabilities, previous);
            assert!(panel.status.translations().1.contains("expired"));
        }
        assert!(!panel.adopt_capabilities(&report, now - Duration::from_secs(11), cx));
        assert_eq!(panel.capabilities, previous);
        monitor.invalidate_pending_capabilities();
        assert!(!panel.adopt_capabilities(&report, now, cx));
        assert_eq!(panel.capabilities, previous);
    });
    let foreign = h
        ._runtime
        .block_on(LinuxMonitor::new(h.session.clone()).capability_report());
    h.panel.update(cx, |panel, cx| {
        let previous = panel.capabilities;
        assert!(!panel.adopt_capabilities(&foreign, Instant::now(), cx));
        assert_eq!(panel.capabilities, previous);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn expired_negative_cache_converges_on_next_refresh_without_rechecking(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    let before = h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.filesystems.state,
            CapabilityState::Available
        );
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Available
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        (panel.capabilities, panel.mcp_sample_id)
    });
    h.control.df_failure.store(127, Ordering::Release);
    h.control.process_failure.store(2, Ordering::Release);
    h.control.socket_failure.store(66, Ordering::Release);
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("live monitor"));
    let cancel = AtomicBool::new(false);
    let rejected = h._runtime.block_on(crate::monitor::execute_job(
        monitor.clone(),
        Job::Redetect,
        &cancel,
    ));
    let completed_at = Instant::now();
    let cached = h._runtime.block_on(monitor.capabilities());
    let later = completed_at + Duration::from_secs(11);
    for capability in [
        Capability::Filesystems,
        Capability::Processes,
        Capability::Sockets,
    ] {
        let observation = cached.observation(capability);
        assert!(matches!(observation.state, CapabilityState::Unavailable(_)));
        assert!(!crate::monitor::fresh(
            observation
                .checked_at
                .unwrap_or_else(|| panic!("real negative check")),
            later
        ));
    }
    h.panel.update(cx, |panel, cx| {
        panel.adopt_outcome_at(
            rejected,
            Some(panel.sample_generation),
            completed_at,
            later,
            cx,
        );
        assert_eq!((panel.capabilities, panel.mcp_sample_id), before);
        assert!(panel.status.translations().1.contains("expired"));
    });
    let commands = h.control.commands.load(Ordering::Acquire);
    let df_commands = h.control.df_commands.load(Ordering::Acquire);
    let process_commands = h.control.process_commands.load(Ordering::Acquire);
    let sockets = h.control.sockets.load(Ordering::Acquire);
    let mut refreshed = h._runtime.block_on(crate::monitor::execute_job(
        monitor.clone(),
        Job::Refresh,
        &cancel,
    ));
    let next_receipt = later + Duration::from_secs(1);
    if let Outcome::Refreshed {
        snapshot,
        sampled_at,
        process_sampled_at,
        capabilities,
        ..
    } = &mut refreshed
    {
        assert!(snapshot.as_ref().as_ref().is_ok_and(|sample| {
            sample.filesystems.is_empty()
                && !sample.networks.is_empty()
                && sample.memory.used_bytes().is_some()
        }));
        for capability in [
            Capability::Filesystems,
            Capability::Processes,
            Capability::Sockets,
        ] {
            assert_eq!(
                capabilities.observations().observation(capability),
                cached.observation(capability),
                "the actual session cache and its original check time must stay unchanged"
            );
        }
        // Advance the UI receipt clock deterministically. Only this next
        // collection's receipt times move with it; the real SSH negative-cache
        // observations above are neither rewritten nor rechecked.
        *sampled_at = next_receipt;
        *process_sampled_at = next_receipt;
    } else {
        panic!("production refresh outcome");
    }
    h.panel.update(cx, |panel, cx| {
        panel.adopt_outcome_at(
            refreshed,
            Some(panel.sample_generation),
            next_receipt,
            next_receipt,
            cx,
        );
        assert_ne!(panel.mcp_sample_id, before.1);
        assert_eq!(panel.last_sample, Some(next_receipt));
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Available
        );
        for capability in [
            Capability::Filesystems,
            Capability::Processes,
            Capability::Sockets,
        ] {
            assert_eq!(
                panel.capabilities.observation(capability),
                cached.observation(capability)
            );
        }
        let status = panel.status.translations().1;
        assert!(status.contains("Some items were not updated"));
        assert!(status.contains("Filesystem capacity: required command is missing"));
        assert!(!status.starts_with("Updated"));
    });
    cx.update_window(h.window, |_, window, cx| {
        crate::i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("monitor-filesystem-empty").label(),
            Some("Capacity unavailable; CPU, memory and network collect independently")
        );
    })
    .checked("cached failure cannot render a successful empty filesystem list");
    let cached_sockets =
        h._runtime
            .block_on(crate::monitor::execute_job(monitor, Job::Sockets, &cancel));
    h.panel.update(cx, |panel, cx| {
        panel.adopt_outcome_at(
            cached_sockets,
            Some(panel.sample_generation),
            next_receipt,
            next_receipt,
            cx,
        );
        assert!(
            panel
                .status
                .translations()
                .1
                .contains("Socket diagnostics failed")
        );
        assert_eq!(panel.capabilities.sockets, cached.sockets);
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 1);
    assert_eq!(h.control.df_commands.load(Ordering::Acquire), df_commands);
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        process_commands
    );
    assert_eq!(h.control.sockets.load(Ordering::Acquire), sockets);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn real_ssh_exec_timeouts_are_transport_failures_and_retry_ps_ss(cx: &mut TestAppContext) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("live monitor"));
    for (capability, flag, count) in [
        (
            Capability::Processes,
            &h.control.timeout_process_once,
            &h.control.process_commands,
        ),
        (
            Capability::Sockets,
            &h.control.timeout_socket_once,
            &h.control.sockets,
        ),
    ] {
        let before = count.load(Ordering::Acquire);
        flag.store(true, Ordering::Release);
        for attempt in 1..=2 {
            // Tokio owns the real SSH clock. The GPUI simulated clock must not
            // spend its 8-second budget before the real 5-second timeout runs.
            let result = h
                ._runtime
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(8), async {
                        match capability {
                            Capability::Processes => {
                                monitor.processes().await.map(|rows| rows.len())
                            }
                            Capability::Sockets => {
                                monitor.listening_sockets().await.map(|rows| rows.len())
                            }
                            _ => unreachable!("ps/ss retry fixture"),
                        }
                    })
                    .await
                })
                .checked("bounded real SSH collection");
            let expected = if attempt == 1 {
                assert!(matches!(
                    result,
                    Err(keelshell_session::monitor::MonitorError::Session(
                        keelshell_session::SessionError::Timeout("SSH exec")
                    ))
                ));
                CapabilityState::Unavailable(CapabilityReason::Transport)
            } else {
                assert_eq!(result.checked("retry without resetting capabilities"), 1);
                CapabilityState::Available
            };
            assert_eq!(count.load(Ordering::Acquire), before + attempt);
            let report = h._runtime.block_on(monitor.capability_report());
            h.panel.update(cx, |panel, cx| {
                assert!(panel.adopt_capabilities(&report, Instant::now(), cx));
                assert_eq!(panel.capabilities.observation(capability).state, expected);
            });
        }
    }
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn degraded_capability_display_locale_and_theme_changes_add_no_commands(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.control.df_failure.store(127, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    let previous = h.panel.read_with(cx, |panel, _| panel.capabilities);
    for family in crate::design::registry() {
        for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    let settings = keelshell_core::Settings {
                        theme_family: keelshell_core::ThemeId::new(family.id)
                            .checked("capability theme"),
                        theme: mode,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    crate::i18n::set_language(language, cx);
                    h.panel
                        .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                    window.render_frame(cx);
                    assert!(window.find("redetect-monitor").visible());
                })
                .checked("degraded capability display and locale repaint");
            }
        }
    }
    assert_eq!(
        h.panel.read_with(cx, |panel, _| panel.capabilities),
        previous
    );
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn compact_capability_reasons_ages_and_recheck_are_keyboard_accessible(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.control.df_failure.store(127, Ordering::Release);
    h.control.process_failure.store(2, Ordering::Release);
    h.control.socket_failure.store(126, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    let previous = h.panel.read_with(cx, |panel, _| panel.capabilities);
    let panel = h.panel.clone();
    let (compact, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| HistoryTestSurface(panel)),
        )
        .checked("900x580 combined layout with the compact monitor")
    });
    for family in crate::design::registry() {
        for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(compact, |_, window, cx| {
                    let settings = keelshell_core::Settings {
                        theme_family: keelshell_core::ThemeId::new(family.id)
                            .checked("compact capability theme"),
                        theme: mode,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    crate::i18n::set_language(language, cx);
                    h.panel
                        .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                    window.render_frame(cx);
                    let recheck = window.find("redetect-monitor");
                    assert_eq!(recheck.role(), Some(gpui_kit::accesskit::Role::Button));
                    assert_eq!(
                        recheck.label(),
                        Some(if language == Language::En {
                            "Recheck Linux monitoring capabilities"
                        } else {
                            "重新检测 Linux 监控能力"
                        })
                    );
                    assert!(recheck.visible());
                    let bounds = recheck.bounds();
                    assert!(bounds.origin.x >= px(0.) && bounds.right() <= px(900.));
                    assert!(bounds.origin.y >= px(0.) && bounds.bottom() <= px(580.));
                    if window.focused(cx).is_none() {
                        window.focus_next(cx);
                        window.render_frame(cx);
                        assert_eq!(window.find("refresh-monitor").focused(), Some(true));
                    }
                    for _ in 0..12 {
                        if window.find("redetect-monitor").focused() == Some(true) {
                            break;
                        }
                        window.press("tab", cx);
                    }
                    assert_eq!(window.find("redetect-monitor").focused(), Some(true));
                    for (index, (capability, zh, en)) in [
                        (
                            Capability::Resources,
                            "核心资源 /proc：可用",
                            "Core resources /proc: Available",
                        ),
                        (
                            Capability::Filesystems,
                            "磁盘容量 df：所需命令未找到",
                            "Filesystem capacity df: Required command not found",
                        ),
                        (
                            Capability::Processes,
                            "进程 ps：命令不支持所需选项",
                            "Processes ps: Required options unsupported",
                        ),
                        (
                            Capability::Sockets,
                            "监听端口 ss：权限不足",
                            "Listening sockets ss: Permission denied",
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let checked_at = previous
                            .observation(capability)
                            .checked_at
                            .unwrap_or_else(|| panic!("real capability check time"));
                        let youngest = checked_at.elapsed().as_secs();
                        for _ in 0..100 {
                            let row = window.find(("monitor-capability", index)).bounds();
                            let viewport = window.find("monitor-scroll").bounds();
                            if row.origin.y >= viewport.origin.y
                                && row.bottom() <= viewport.bottom()
                            {
                                break;
                            }
                            let direction = if row.origin.y < viewport.origin.y {
                                2.
                            } else {
                                -2.
                            };
                            window.scroll(
                                "monitor-scroll",
                                gpui_kit::ScrollDelta::Lines(point(0., direction)),
                                cx,
                            );
                        }
                        window.render_frame(cx);
                        let row = window.find(("monitor-capability", index));
                        let viewport = window.find("monitor-scroll").bounds();
                        let bounds = row.bounds();
                        assert!(row.visible());
                        assert_eq!(row.role(), Some(gpui_kit::accesskit::Role::Label));
                        assert!(bounds.size.width > px(0.) && bounds.size.width <= px(320.));
                        assert!(
                            bounds.origin.x >= viewport.origin.x
                                && bounds.right() <= viewport.right()
                        );
                        assert!(
                            bounds.origin.y >= viewport.origin.y
                                && bounds.bottom() <= viewport.bottom()
                        );
                        let label = row
                            .label()
                            .unwrap_or_else(|| panic!("capability AX reason and age"));
                        let (reason, age) = label
                            .split_once(", ")
                            .unwrap_or_else(|| panic!("separate capability age"));
                        assert_eq!(reason, if language == Language::En { en } else { zh });
                        let seconds = if language == Language::En {
                            age.strip_prefix("Checked ")
                                .and_then(|age| age.strip_suffix("s ago"))
                        } else {
                            age.strip_suffix(" 秒前检测")
                        }
                        .unwrap_or_else(|| panic!("localized capability age"))
                        .parse::<u64>()
                        .checked("numeric capability age");
                        assert!((youngest..=checked_at.elapsed().as_secs()).contains(&seconds));
                    }
                })
                .checked("compact capability bounds, exact bilingual AX text and Tab reachability");
            }
        }
    }
    assert_eq!(
        h.panel.read_with(cx, |panel, _| panel.capabilities),
        previous
    );
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    cx.update_window(compact, |_, window, cx| {
        assert_eq!(window.find("redetect-monitor").focused(), Some(true));
        window.press("enter", cx);
    })
    .checked("activate capability recheck with the keyboard");
    h.idle(cx).await;
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 3);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.paused);
        assert!(panel.capabilities.filesystems.checked_at > previous.filesystems.checked_at);
        assert_eq!(
            panel.capabilities.filesystems.state,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        );
    });
    h.close_history_fixture(cx).await;
}
