//! Controlled SSH frames prove Darwin adoption and UI semantics, not native ABI.

#![expect(
    clippy::expect_used,
    reason = "Test assertions require adopted samples and rendered accessibility labels"
)]

use super::*;
use crate::monitor::{MonitorPlatform, history::Series};

pub(super) fn resources(sample: usize, capacity: bool) -> String {
    let df = if capacity {
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 500 400 55% /\n"
    } else {
        ""
    };
    let status = if capacity { "ok" } else { "skipped" };
    let ns = (1000_u64 + sample as u64 * 5) * 1_000_000_000;
    let ticks = 100 + sample * 10;
    format!(
        "@@KS:darwin@@\nschema\t1\nplatform\tDarwin\nunits\tbytes seconds ticks pages nanoseconds\nboot_time\t1700000000\nboot_time_usec\t0\nmach_ticks\t{ns}\ntimebase_numer\t1\ntimebase_denom\t1\nuptime_ns\t{ns}\npage_size\t4096\nsysconf_page_size\t4096\nmemory_total_bytes\t1048576\nmemory_free_pages\t64\nmemory_free_bytes\t262144\ncpu_user\t{ticks}\ncpu_nice\t0\ncpu_system\t0\ncpu_idle\t{ticks}\nload_1\t0.1\nload_5\t0.2\nload_15\t0.3\nnetwork_count\t1\nnetwork\ten0\t1000\t2000\ndf_status\t{status}\ndf_bytes\t{}\n@@KS:df@@\n{df}\n@@KS:end@@\n",
        df.len()
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

#[gpui_kit::test]
async fn darwin_ssh_samples_keep_used_memory_mcp_and_trends_unknown(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        panel.run(Job::Refresh, cx);
    });
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(
            panel.rates.cpu_busy_percent.is_none(),
            "first sample after pause is a new baseline"
        );
    });
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        let snapshot = panel.snapshot.as_ref().expect("Darwin adopted");
        assert_eq!(snapshot.platform, MonitorPlatform::Darwin);
        assert_eq!(snapshot.memory.free_bytes, Some(262144));
        assert!(snapshot.memory.used_bytes().is_none());
        assert_eq!(panel.rates.cpu_busy_percent, Some(50.));
        assert!(panel.rates.cpu_iowait_percent.is_none());
        assert!(
            panel
                .history
                .vertices(Series::Memory, std::time::Instant::now())
                .is_empty()
        );
        let mcp = panel.mcp_snapshot().expect("Darwin shared sample");
        assert_eq!(mcp.memory_total_bytes, Some(1048576));
        assert_eq!(mcp.memory_used_bytes, None);
        assert!(!panel.processes.is_empty());
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_controls_reject_linux_actions_without_ssh_commands(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.control.commands.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| {
        let process = panel.processes[0].clone();
        panel.run(Job::Inspect(process), cx);
        panel.run(
            Job::Probe(keelshell_session::monitor::SocketInfo {
                protocol: "tcp".into(),
                state: "LISTEN".into(),
                local: "127.0.0.1:2222".into(),
                peer: "*:*".into(),
                process: None,
            }),
            cx,
        );
        assert!(panel.pending.is_none());
        assert!(!panel.busy);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("review-process", 42_usize)).is_none());
        assert_eq!(window.find("refresh-sockets").label(), Some("读取端口"));
    })
    .checked("Darwin disabled action");
    h.idle(cx).await;
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    assert_eq!(h.control.inspecting.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_missing_python_is_cached_until_explicit_recheck(cx: &mut TestAppContext) {
    use keelshell_session::monitor::{CapabilityReason, CapabilityState};
    let control = Arc::new(Control {
        darwin: AtomicBool::new(true),
        fail_resources: AtomicBool::new(true),
        ..Control::default()
    });
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.snapshot.is_none());
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        );
        assert!(
            !panel.processes.is_empty(),
            "process rows remain independent"
        );
    });
    let attempts = h.control.disk_samples.load(Ordering::Acquire);
    h.control.fail_resources.store(false, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), attempts);
    h.panel
        .read_with(cx, |panel, _| assert!(panel.snapshot.is_none()));
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), attempts + 1);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.snapshot.as_ref().expect("recheck recovered").platform,
            MonitorPlatform::Darwin
        );
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Available
        );
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn darwin_memory_and_capability_labels_follow_all_themes_and_languages_without_io(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.control.commands.load(Ordering::Acquire);
    for family in crate::design::registry() {
        for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    let settings = keelshell_core::Settings {
                        theme_family: keelshell_core::ThemeId::new(family.id)
                            .checked("Darwin theme"),
                        theme: mode,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    crate::i18n::set_language(language, cx);
                    h.panel
                        .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                    window.render_frame(cx);
                    let english = language == Language::En;
                    let used = if english { "Memory" } else { "内存" };
                    assert_eq!(
                        window.find(format!("monitor-metric-{used}")).label(),
                        Some(format!("{used}: —").as_str())
                    );
                    let total = if english { "Total memory" } else { "总内存" };
                    assert!(
                        window
                            .find(format!("monitor-metric-{total}"))
                            .label()
                            .expect("total label")
                            .contains("1.0 MiB")
                    );
                    let free = if english {
                        "Free memory"
                    } else {
                        "空闲内存"
                    };
                    assert!(
                        window
                            .find(format!("monitor-metric-{free}"))
                            .label()
                            .expect("free label")
                            .contains("0.2 MiB")
                    );
                    assert_eq!(
                        window.find("monitor-capabilities").label(),
                        Some(if english {
                            "Host capabilities"
                        } else {
                            "主机监控能力"
                        })
                    );
                    assert!(window.find("monitor-darwin-memory-note").label().is_some());
                    assert_eq!(
                        window.find("monitor-sockets-note").label(),
                        Some(if english {
                            "Read-only TCP listeners and UDP sockets visible to this account. macOS UDP labels show a bound address or specified peer, not reachability or service health."
                        } else {
                            "只读采集当前账户可见的 TCP 监听与 UDP 端口。macOS 的 UDP 仅表示已绑定或已指定对端，不代表连通或服务健康。"
                        })
                    );
                })
                .checked("Darwin themed labels");
            }
        }
    }
    assert_eq!(h.control.commands.load(Ordering::Acquire), before);
    h.close_history_fixture(cx).await;
}
