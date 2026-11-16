//! Controlled SSH/GPUI coverage, independent of a native Windows desktop.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use keelshell_session::monitor::{
    CapabilityReason, CapabilityState, CpuSample, MonitorEnvironment, MonitorError,
    MonitorPlatform, MonitorRuntime, Snapshot, SocketInfo,
};

#[gpui_kit::test]
async fn unconfirmed_recheck_cannot_authorize_actions_from_a_retained_linux_sample(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.linux_monitor_actions_available())
    });
    h.control
        .environment_unconfirmed
        .store(true, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| {
        assert!(
            panel
                .snapshot
                .as_ref()
                .is_some_and(|sample| sample.platform == MonitorPlatform::Linux)
        );
        assert_eq!(panel.capabilities.environment, None);
        assert!(!panel.linux_monitor_actions_available());
        let selected = panel.processes[0].clone();
        panel.run(Job::Inspect(selected), cx);
        panel.run(
            Job::Probe(SocketInfo {
                protocol: "tcp".into(),
                state: "LISTEN".into(),
                local: "127.0.0.1:2222".into(),
                peer: "*".into(),
                process: None,
            }),
            cx,
        );
        assert!(panel.pending.is_none() && !panel.busy);
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.inspecting.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}
use serde_json::{Value, json};
use std::time::Instant;

use crate::monitor::{
    Outcome,
    history::{ResourceHistory, Series},
};

/// Decode the production command instead of matching an encoded byte fragment.
pub(super) fn operation(command: &str) -> Option<(&'static str, bool)> {
    let encoded = command
        .strip_prefix("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ")?;
    assert!(command.len() < 8000, "Windows default shell command budget");
    let bytes = STANDARD
        .decode(encoded)
        .checked("fixed UTF-16LE Base64 script");
    assert_eq!(bytes.len() % 2, 0);
    let units: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let script = String::from_utf16(&units).checked("fixed PowerShell Unicode");
    let operation = ["probe", "resources", "processes", "sockets"]
        .into_iter()
        .find(|operation| {
            script.starts_with(&format!("$op='{operation}';$skip=$false;\n"))
                || (*operation == "resources"
                    && script.starts_with("$op='resources';$skip=$true;\n"))
        })
        .unwrap_or_else(|| panic!("fixed script did not identify its operation"));
    Some((
        operation,
        script.starts_with("$op='resources';$skip=$true;\n"),
    ))
}

fn envelope(operation: &str, status: &str, data: Value) -> String {
    let frame = json!({
        "version": 1,
        "operation": operation,
        "platform": "Windows",
        "runtime": "WindowsPowerShell",
        "runtime_version": "5.1.26100.123",
        "units": "bytes seconds percent",
        "status": status,
        "data": data,
    });
    format!("{frame}\n")
}

fn failure(operation: &str, status: usize) -> (String, u32) {
    let reason = match status {
        66 | 127 => "command_missing",
        126 => "permission_denied",
        2 => "unsupported_options",
        _ => "command_failed",
    };
    (envelope(operation, reason, Value::Null), status as u32)
}

pub(super) async fn reply(
    control: &Control,
    operation: &str,
    skip_capacity: bool,
) -> Result<Option<(String, u32)>, russh::Error> {
    let empty = control.windows_empty.load(Ordering::Acquire);
    let data = match operation {
        "probe" => {
            control.windows_probes.fetch_add(1, Ordering::Release);
            if control.environment_unconfirmed.load(Ordering::Acquire)
                || !control.windows.load(Ordering::Acquire)
            {
                return Ok(Some(failure(operation, 64)));
            }
            json!({"os": "Win32NT"})
        }
        "resources" => {
            let sample = control.disk_samples.fetch_add(1, Ordering::AcqRel);
            if control.hold_resources.load(Ordering::Acquire) {
                control
                    .resource_release
                    .acquire()
                    .await
                    .map_err(|_| russh::Error::Disconnect)?
                    .forget();
            }
            if control.fail_resources.load(Ordering::Acquire) {
                return Ok(Some(failure(operation, 126)));
            }
            let capacity = control.df_failure.load(Ordering::Acquire);
            if !skip_capacity {
                control.df_commands.fetch_add(1, Ordering::Release);
            }
            let filesystems = if capacity == 0 && !skip_capacity {
                vec![json!({
                    "source": "C:\\",
                    "mount": "C:\\",
                    "total_bytes": "107374182400",
                    "total_free_bytes": "42949672960",
                    "available_bytes": "37580963840",
                })]
            } else {
                Vec::new()
            };
            let capacity_status = match (skip_capacity, capacity) {
                (true, _) => "skipped",
                (false, 0) => "ok",
                (false, 66 | 127) => "command_missing",
                (false, 126) => "permission_denied",
                (false, 2) => "unsupported_options",
                _ => "command_failed",
            };
            let cpu = control.windows_cpu_basis_points.load(Ordering::Acquire);
            let cpu = if cpu == usize::MAX {
                Value::Null
            } else {
                json!(cpu as f64 / 100.)
            };
            json!({
                "boot_time": "1700000000",
                "uptime_seconds": 1000 + sample * 5,
                "cpu_percent": cpu,
                "memory_total_bytes": "8589934592",
                "memory_free_bytes": "2147483648",
                "network_count": 1,
                "networks": [{
                    "name": "以太网 α",
                    "received_bytes": (1000 + sample * 100).to_string(),
                    "transmitted_bytes": (2000 + sample * 100).to_string(),
                }],
                "capacity_status": capacity_status,
                "filesystem_count": filesystems.len(),
                "filesystems": filesystems,
            })
        }
        "processes" => {
            control.process_commands.fetch_add(1, Ordering::Release);
            if control.timeout_process_once.swap(false, Ordering::AcqRel) {
                return Ok(None);
            }
            if control.hold_processes.load(Ordering::Acquire) {
                control
                    .process_release
                    .acquire()
                    .await
                    .map_err(|_| russh::Error::Disconnect)?
                    .forget();
            }
            let status = control.process_failure.load(Ordering::Acquire);
            if status != 0 {
                return Ok(Some(failure(operation, status)));
            }
            let processes: Vec<_> = if empty {
                Vec::new()
            } else {
                (0..8).map(|index| json!({
                    "pid": (index * 7).to_string(),
                    "name": if index == 0 { "Idle".to_owned() } else { format!("进程 α {index}") },
                    "working_set_bytes": if index == 0 { Value::Null } else { json!("16777216") },
                })).collect()
            };
            json!({"count": processes.len(), "processes": processes})
        }
        "sockets" => {
            control.sockets.fetch_add(1, Ordering::Release);
            if control.timeout_socket_once.swap(false, Ordering::AcqRel) {
                return Ok(None);
            }
            if control.hold_sockets.load(Ordering::Acquire) {
                control
                    .socket_release
                    .acquire()
                    .await
                    .map_err(|_| russh::Error::Disconnect)?
                    .forget();
            }
            let status = control.socket_failure.load(Ordering::Acquire);
            if status != 0 {
                return Ok(Some(failure(operation, status)));
            }
            let sockets: Vec<_> = if empty {
                Vec::new()
            } else {
                (0..15)
                    .map(|index| {
                        json!({
                            "protocol": if index == 0 { "tcp" } else { "udp" },
                            "address": if index == 0 { "127.0.0.1" } else { "::" },
                            "port": (2200 + index).to_string(),
                            "pid": "7",
                        })
                    })
                    .collect()
            };
            json!({"count": sockets.len(), "sockets": sockets})
        }
        _ => panic!("unrecognized fixed Windows operation"),
    };
    Ok(Some((envelope(operation, "ok", data), 0)))
}

fn control() -> Arc<Control> {
    Arc::new(Control {
        windows: AtomicBool::new(true),
        ..Control::default()
    })
}

fn harness(cx: &mut TestAppContext) -> Harness {
    Harness::with_control(cx, control())
}

fn snapshot(cpu: CpuSample, uptime_seconds: f64) -> Snapshot {
    use keelshell_session::monitor::{DiskIoError, MemorySnapshot};
    Snapshot {
        platform: MonitorPlatform::Windows,
        boot_time: 1_700_000_000,
        uptime_seconds,
        load_average: None,
        cpu,
        memory: MemorySnapshot {
            total_bytes: 8_589_934_592,
            available_bytes: None,
            free_bytes: Some(2_147_483_648),
            swap_total_bytes: None,
            swap_free_bytes: None,
        },
        networks: Vec::new(),
        filesystems: Vec::new(),
        disk_io: Err(DiskIoError::MissingData),
    }
}

fn assert_windows_environment(panel: &MonitorPanel) {
    assert_eq!(
        panel.capabilities.environment,
        Some(MonitorEnvironment {
            platform: MonitorPlatform::Windows,
            runtime: MonitorRuntime::WindowsPowerShell {
                version: [5, 1, 26100, 123]
            },
        })
    );
    assert!(panel.is_windows());
    assert!(!panel.linux_monitor_actions_available());
}

#[test]
fn windows_provider_first_sample_and_idle_zero_need_no_counter_baseline() {
    let at = Instant::now();
    let first = snapshot(CpuSample::ReportedPercent(25.), 1000.);
    let mut history = ResourceHistory::default();
    assert_eq!(history.record(&first, None, at, at), Ok(Some(25.)));
    let second = snapshot(CpuSample::ReportedPercent(0.), 1005.);
    let next_at = at + Duration::from_secs(5);
    assert_eq!(
        history.record(&second, Some(&first), next_at, next_at),
        Ok(Some(0.))
    );
    let points = history.vertices(Series::Cpu, next_at);
    assert_eq!(points.len(), 2);
    assert!(!points[0].connect && points[1].connect);
    assert_eq!(points[1].y, 1.);
    assert!(history.vertices(Series::Memory, next_at).is_empty());
}

#[test]
fn windows_provider_values_survive_interruptions_without_joining_history_gaps() {
    for interruption in 0..7 {
        let at = Instant::now();
        let first = snapshot(CpuSample::ReportedPercent(25.), 1000.);
        let mut history = ResourceHistory::default();
        assert_eq!(history.record(&first, None, at, at), Ok(Some(25.)));
        let mut second = snapshot(CpuSample::ReportedPercent(75.), 1005.);
        let mut next_at = at + Duration::from_secs(5);
        match interruption {
            0 | 1 => history.interrupt(), // Pause and collection error share this boundary.
            2 => {
                next_at = at + Duration::from_secs(20);
                second.uptime_seconds = 1020.;
            }
            3 => second.boot_time += 1,
            4 => second.uptime_seconds = 999.,
            5 => second.uptime_seconds = first.uptime_seconds,
            _ => second.uptime_seconds = 2000., // Wall-clock jump.
        }
        assert_eq!(
            history.record(&second, Some(&first), next_at, next_at),
            Ok(Some(75.))
        );
        let points = history.vertices(Series::Cpu, next_at);
        assert_eq!(points.len(), 2);
        assert!(
            !points[1].connect,
            "interruption {interruption} must stay a gap"
        );
    }
}

#[test]
fn windows_unavailable_cpu_breaks_history_without_inventing_an_idle_zero() {
    let at = Instant::now();
    let mut history = ResourceHistory::default();
    let mut previous = None;
    for (index, cpu, expected) in [
        (0, CpuSample::ReportedPercent(25.), Some(25.)),
        (1, CpuSample::Unavailable, None),
        (2, CpuSample::ReportedPercent(0.), Some(0.)),
        (3, CpuSample::ReportedPercent(50.), Some(50.)),
    ] {
        let snapshot = snapshot(cpu, 1000. + index as f64 * 5.);
        let receipt = at + Duration::from_secs(index * 5);
        assert_eq!(
            history.record(&snapshot, previous.as_ref(), receipt, receipt),
            Ok(expected)
        );
        previous = Some(snapshot);
    }
    let points = history.vertices(Series::Cpu, at + Duration::from_secs(15));
    assert_eq!(points.len(), 3);
    assert!(!points[1].connect && points[2].connect);
    assert!(
        history
            .vertices(Series::Memory, at + Duration::from_secs(15))
            .is_empty()
    );
}

#[test]
fn windows_provider_percentages_cannot_bypass_receipt_and_uptime_validation() {
    let at = Instant::now();
    let first = snapshot(CpuSample::ReportedPercent(25.), 1000.);
    let mut history = ResourceHistory::default();
    assert!(history.record(&first, None, at, at).is_ok());
    for (receipt, now, uptime) in [
        (at, at, 1005.),
        (at + Duration::from_secs(5), at, 1005.),
        (
            at + Duration::from_secs(5),
            at + Duration::from_secs(16),
            1005.,
        ),
        (
            at + Duration::from_secs(5),
            at + Duration::from_secs(5),
            f64::NAN,
        ),
    ] {
        let sample = snapshot(CpuSample::ReportedPercent(75.), uptime);
        assert!(history.record(&sample, Some(&first), receipt, now).is_err());
        assert_eq!(history.vertices(Series::Cpu, now).len(), 1);
    }
}

#[test]
fn windows_unknown_environment_and_cancelled_errors_have_explicit_bilingual_text() {
    let (zh, en) = crate::monitor::error_text(&MonitorError::EnvironmentUnconfirmed);
    assert!(zh.contains("未知") && en.contains("unknown"));
    assert!(!zh.contains("Windows") && !en.contains("Windows"));
    assert!(!zh.contains("命令未找到") && !en.contains("missing"));
    let (zh, en) = crate::monitor::error_text(&MonitorError::Cancelled);
    assert!(zh.contains("撤销") && en.contains("revoked"));
}

#[gpui_kit::test]
async fn windows_first_ssh_sample_exposes_provider_cpu_and_matching_mcp_current_point(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let commands = h.control.commands.load(Ordering::Acquire);
    h.panel.read_with(cx, |panel, _| {
        assert_windows_environment(panel);
        let snapshot = panel
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("Windows sample"));
        assert_eq!(snapshot.cpu.reported_percent(), Some(25.));
        assert!(snapshot.cpu.cumulative().is_none());
        assert!(snapshot.load_average.is_none() && snapshot.disk_io.is_err());
        assert!(snapshot.memory.used_bytes().is_none());
        assert_eq!(snapshot.memory.free_bytes, Some(2_147_483_648));
        assert_eq!(panel.rates.cpu_busy_percent, Some(25.));
        assert!(panel.rates.cpu_iowait_percent.is_none() && panel.rates.disks.is_empty());
        assert!(panel.rates.networks.iter().all(|rate| {
            rate.received_bytes_per_second.is_none() && rate.transmitted_bytes_per_second.is_none()
        }));
        let points = panel.history.vertices(Series::Cpu, Instant::now());
        assert_eq!(points.len(), 1);
        assert!(!points[0].connect);
        assert!((points[0].y - 0.75).abs() < f32::EPSILON);
        assert!(
            panel
                .history
                .vertices(Series::Memory, Instant::now())
                .is_empty()
        );
        let mcp = panel
            .mcp_snapshot()
            .unwrap_or_else(|| panic!("shared Windows point"));
        assert_eq!(
            mcp.sample_id,
            panel.mcp_sample_id.unwrap_or_else(|| panic!("sample ID"))
        );
        assert_eq!(mcp.cpu_percent, panel.rates.cpu_busy_percent);
        assert_eq!(mcp.memory_total_bytes, Some(8_589_934_592));
        assert_eq!(mcp.memory_used_bytes, None);
        let fs = &snapshot.filesystems[0];
        assert_eq!(fs.used_bytes, 64_424_509_440);
        assert_eq!(fs.available_bytes, 37_580_963_840);
        assert_eq!(fs.used_percent, 60);
        assert_eq!(panel.processes[0].pid, 0);
        assert!(panel.processes[0].working_set_bytes.is_none());
        assert_eq!(panel.processes[1].working_set_bytes, Some(16_777_216));
        assert!(panel.processes.iter().all(|p| {
            p.user.is_none() && p.cpu_percent.is_none() && p.memory_percent.is_none()
        }));
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("monitor-metric-CPU").label(),
            Some("CPU: 25.0%")
        );
        assert_eq!(
            window.find("monitor-metric-内存").label(),
            Some("内存: 未知")
        );
    })
    .checked("first Windows CPU gauge and unknown used memory");
    assert_eq!(h.control.platform_probes.load(Ordering::Acquire), 1);
    assert_eq!(h.control.windows_probes.load(Ordering::Acquire), 1);
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_null_cpu_and_real_idle_zero_remain_distinct_in_ui_mcp_and_history(
    cx: &mut TestAppContext,
) {
    let control = control();
    control
        .windows_cpu_basis_points
        .store(usize::MAX, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.panel.read_with(cx, |panel, _| {
        assert!(matches!(
            panel.snapshot.as_ref().map(|s| &s.cpu),
            Some(CpuSample::Unavailable)
        ));
        assert_eq!(panel.mcp_snapshot().and_then(|s| s.cpu_percent), None);
        assert!(
            panel
                .history
                .vertices(Series::Cpu, Instant::now())
                .is_empty()
        );
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("monitor-metric-CPU").label(), Some("CPU: 未知"));
    })
    .checked("unavailable provider CPU is explicit");
    h.control
        .windows_cpu_basis_points
        .store(0, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.rates.cpu_busy_percent, Some(0.));
        assert_eq!(panel.mcp_snapshot().and_then(|s| s.cpu_percent), Some(0.));
        let points = panel.history.vertices(Series::Cpu, Instant::now());
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].y, 1.);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("monitor-metric-CPU").label(), Some("CPU: 0.0%"));
    })
    .checked("actual idle zero remains visible");
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_resource_failure_preserves_independent_processes_and_known_read_only_environment(
    cx: &mut TestAppContext,
) {
    let control = control();
    control.fail_resources.store(true, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.panel.read_with(cx, |panel, _| {
        assert_windows_environment(panel);
        assert!(panel.snapshot.is_none() && panel.mcp_snapshot().is_none());
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Unavailable(CapabilityReason::PermissionDenied)
        );
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Available
        );
        assert_eq!(panel.processes.len(), 8);
    });
    let commands = h.control.commands.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| {
        panel.run(Job::Inspect(panel.processes[1].clone()), cx);
        panel.run(
            Job::Probe(keelshell_session::monitor::SocketInfo {
                protocol: "tcp".into(),
                state: "LISTEN".into(),
                local: "127.0.0.1:2200".into(),
                peer: "*".into(),
                process: Some("PID 7".into()),
            }),
            cx,
        );
        assert!(!panel.busy && panel.pending.is_none());
        assert!(panel.status.translations().0.contains("Windows 监控只读"));
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("monitor-windows-identity").label().is_some());
        for pid in [0_usize, 7, 14, 21, 28] {
            assert!(window.try_find(("review-process", pid)).is_none());
        }
        assert!(
            window
                .find(("monitor-process", 7_usize))
                .label()
                .is_some_and(|text| text.contains("工作集 16.0 MiB") && text.contains("CPU 未知"))
        );
    })
    .checked("known Windows without resource sample stays read-only");
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.inspecting.load(Ordering::Acquire), 0);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_socket_first_collection_establishes_environment_without_a_resource_sample(
    cx: &mut TestAppContext,
) {
    let control = control();
    control.fail_resources.store(true, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        panel.monitor = Some(keelshell_session::monitor::LinuxMonitor::new(
            h.session.clone(),
        ));
        panel.capabilities = Default::default();
        assert!(panel.snapshot.is_none());
    });
    let resources = h.control.disk_samples.load(Ordering::Acquire);
    let processes = h.control.process_commands.load(Ordering::Acquire);
    let posix = h.control.platform_probes.load(Ordering::Acquire);
    let powershell = h.control.windows_probes.load(Ordering::Acquire);
    h.load_sockets(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_windows_environment(panel);
        assert!(panel.snapshot.is_none());
        assert_eq!(panel.capabilities.resources.state, CapabilityState::Unknown);
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.sockets.len(), 15);
        assert!(panel.sockets.iter().all(|socket| socket.peer == "*"));
    });
    let commands = h.control.commands.load(Ordering::Acquire);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("probe-socket", 0_usize)).is_none());
        assert!(
            window
                .find(("monitor-socket", 0_usize))
                .label()
                .is_some_and(|label| !label.contains('→') && label.contains("127.0.0.1:2200"))
        );
        assert!(
            window
                .find("monitor-sockets-note")
                .label()
                .is_some_and(|label| label.contains("Windows") && label.contains("PID"))
        );
    })
    .checked("socket-first Windows uses cached environment facts");
    assert_eq!(h.control.platform_probes.load(Ordering::Acquire), posix + 1);
    assert_eq!(
        h.control.windows_probes.load(Ordering::Acquire),
        powershell + 1
    );
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), resources);
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        processes
    );
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_resource_negative_cache_retries_only_after_explicit_recheck(
    cx: &mut TestAppContext,
) {
    let control = control();
    control.fail_resources.store(true, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h
        .panel
        .read_with(cx, |panel, _| panel.capabilities.resources);
    let attempts = h.control.disk_samples.load(Ordering::Acquire);
    let processes = h.control.process_commands.load(Ordering::Acquire);
    h.control.fail_resources.store(false, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), attempts);
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        processes + 1
    );
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.capabilities.resources, before);
        assert!(panel.snapshot.is_none());
    });
    let posix = h.control.platform_probes.load(Ordering::Acquire);
    let powershell = h.control.windows_probes.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), attempts + 1);
    assert_eq!(h.control.platform_probes.load(Ordering::Acquire), posix + 1);
    assert_eq!(
        h.control.windows_probes.load(Ordering::Acquire),
        powershell + 1
    );
    h.panel.read_with(cx, |panel, _| {
        assert_windows_environment(panel);
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Available
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.rates.cpu_busy_percent, Some(25.));
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_process_and_socket_failures_are_independent_cached_and_rechecked(
    cx: &mut TestAppContext,
) {
    let control = control();
    control.process_failure.store(126, Ordering::Release);
    control.socket_failure.store(2, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    let before = h.panel.read_with(cx, |panel, _| {
        assert!(panel.snapshot.is_some());
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Unavailable(CapabilityReason::PermissionDenied)
        );
        assert_eq!(
            panel.capabilities.sockets.state,
            CapabilityState::Unavailable(CapabilityReason::UnsupportedOptions)
        );
        assert!(panel.processes.is_empty() && panel.sockets.is_empty());
        (panel.capabilities.processes, panel.capabilities.sockets)
    });
    let processes = h.control.process_commands.load(Ordering::Acquire);
    let sockets = h.control.sockets.load(Ordering::Acquire);
    h.control.process_failure.store(0, Ordering::Release);
    h.control.socket_failure.store(0, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.load_sockets(cx);
    h.idle(cx).await;
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        processes
    );
    assert_eq!(h.control.sockets.load(Ordering::Acquire), sockets);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            (panel.capabilities.processes, panel.capabilities.sockets),
            before
        );
        assert_eq!(panel.rates.cpu_busy_percent, Some(25.));
    });
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Available
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
        assert_eq!(panel.processes.len(), 8);
        assert_eq!(panel.sockets.len(), 15);
    });
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        processes + 1
    );
    assert_eq!(h.control.sockets.load(Ordering::Acquire), sockets + 1);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_capacity_failure_keeps_core_resources_and_skips_cached_drive_collection(
    cx: &mut TestAppContext,
) {
    let control = control();
    control.df_failure.store(126, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| {
        assert_windows_environment(panel);
        assert!(
            panel
                .snapshot
                .as_ref()
                .is_some_and(|s| s.filesystems.is_empty())
        );
        assert_eq!(
            panel.capabilities.resources.state,
            CapabilityState::Available
        );
        assert_eq!(
            panel.capabilities.filesystems.state,
            CapabilityState::Unavailable(CapabilityReason::PermissionDenied)
        );
        panel.capabilities.filesystems
    });
    let attempts = h.control.df_commands.load(Ordering::Acquire);
    h.control.df_failure.store(0, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    assert_eq!(h.control.df_commands.load(Ordering::Acquire), attempts);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.capabilities.filesystems, before);
        assert_eq!(panel.rates.cpu_busy_percent, Some(25.));
        assert!(
            panel
                .snapshot
                .as_ref()
                .is_some_and(|s| !s.networks.is_empty())
        );
    });
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    assert_eq!(h.control.df_commands.load(Ordering::Acquire), attempts + 1);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.capabilities.filesystems.state,
            CapabilityState::Available
        );
        assert!(
            panel
                .snapshot
                .as_ref()
                .is_some_and(|s| s.filesystems.len() == 1)
        );
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_refresh_failure_keeps_old_points_and_recovers_with_a_visible_new_segment(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h
        .panel
        .read_with(cx, |panel, _| (panel.mcp_sample_id, panel.last_sample));
    h.control.fail_resources.store(true, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!((panel.mcp_sample_id, panel.last_sample), before);
        assert_eq!(panel.rates.cpu_busy_percent, None);
        assert_eq!(panel.mcp_snapshot().and_then(|s| s.cpu_percent), None);
        assert_eq!(panel.history.vertices(Series::Cpu, Instant::now()).len(), 1);
        assert!(!panel.processes.is_empty());
    });
    h.control.fail_resources.store(false, Ordering::Release);
    h.control
        .windows_cpu_basis_points
        .store(7500, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_ne!(panel.mcp_sample_id, before.0);
        assert_eq!(panel.rates.cpu_busy_percent, Some(75.));
        assert_eq!(panel.mcp_snapshot().and_then(|s| s.cpu_percent), Some(75.));
        let points = panel.history.vertices(Series::Cpu, Instant::now());
        assert_eq!(points.len(), 2);
        assert!(!points[1].connect);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_pause_aba_discards_held_refresh_and_preserves_completed_environment(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.capabilities,
            panel.mcp_sample_id,
            panel.last_sample,
            panel.last_process_sample,
        )
    });
    let processes = h.control.process_commands.load(Ordering::Acquire);
    let probes = (
        h.control.platform_probes.load(Ordering::Acquire),
        h.control.windows_probes.load(Ordering::Acquire),
    );
    h.control.hold_processes.store(true, Ordering::Release);
    h.control
        .windows_cpu_basis_points
        .store(7500, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.control.process_commands.load(Ordering::Acquire) == processes + 1 && h.panel.read(cx).busy
    })
    .await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(false, cx);
        panel.set_paused(true, cx);
    });
    h.control.hold_processes.store(false, Ordering::Release);
    h.control.process_release.add_permits(1);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            (
                panel.snapshot.clone(),
                panel.rates.clone(),
                panel.capabilities,
                panel.mcp_sample_id,
                panel.last_sample,
                panel.last_process_sample
            ),
            before
        );
        assert!(!panel.busy && panel.worker_cancel.is_none());
        assert_eq!(
            panel.status.translations(),
            ("自动刷新已暂停。", "Polling paused.")
        );
        assert_eq!(panel.history.vertices(Series::Cpu, Instant::now()).len(), 1);
    });
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    assert_eq!(
        (
            h.control.platform_probes.load(Ordering::Acquire),
            h.control.windows_probes.load(Ordering::Acquire)
        ),
        probes
    );
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.rates.cpu_busy_percent, Some(75.));
        let points = panel.history.vertices(Series::Cpu, Instant::now());
        assert_eq!(points.len(), 2);
        assert!(!points[1].connect);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_suspension_discards_held_resources_before_sending_process_collection(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.capabilities,
            panel.mcp_sample_id,
        )
    });
    let resources = h.control.disk_samples.load(Ordering::Acquire);
    let processes = h.control.process_commands.load(Ordering::Acquire);
    h.control.hold_resources.store(true, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.control.disk_samples.load(Ordering::Acquire) == resources + 1 && h.panel.read(cx).busy
    })
    .await;
    h.panel.update(cx, |panel, cx| panel.suspend(cx));
    h.control.hold_resources.store(false, Ordering::Release);
    h.control.resource_release.add_permits(1);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            (
                panel.snapshot.clone(),
                panel.capabilities,
                panel.mcp_sample_id
            ),
            before
        );
        assert!(panel.suspended && panel.monitor.is_none() && panel.pending.is_none());
        assert!(panel.mcp_snapshot().is_none());
        assert_eq!(panel.history.vertices(Series::Cpu, Instant::now()).len(), 1);
    });
    assert_eq!(
        h.control.process_commands.load(Ordering::Acquire),
        processes
    );
    assert!(
        !h.session.is_closed(),
        "monitor retirement does not close shared SSH"
    );
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_pause_aba_discards_late_socket_rows_and_capability_results(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| panel.capabilities);
    h.control.hold_sockets.store(true, Ordering::Release);
    h.load_sockets(cx);
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
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
        assert!(panel.sockets.is_empty() && panel.probes.is_empty());
        assert_eq!(panel.capabilities, before);
        assert!(panel.pending.is_none() && !panel.busy);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_expired_outcome_cannot_publish_cpu_mcp_or_process_receipts(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.capabilities,
            panel.mcp_sample_id,
            panel.last_process_sample,
        )
    });
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("active monitor"));
    let outcome = h._runtime.block_on(crate::monitor::execute_job(
        monitor,
        Job::Refresh,
        &AtomicBool::new(false),
    ));
    let completed = Instant::now();
    h.panel.update(cx, |panel, cx| {
        panel.adopt_outcome_at(
            outcome,
            Some(panel.sample_generation),
            completed,
            completed + Duration::from_secs(11),
            cx,
        );
        assert_eq!(
            (
                panel.snapshot.clone(),
                panel.capabilities,
                panel.mcp_sample_id,
                panel.last_process_sample
            ),
            before
        );
        assert!(panel.status.translations().0.contains("过期"));
        assert_eq!(panel.history.vertices(Series::Cpu, Instant::now()).len(), 1);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_stale_resources_do_not_discard_a_fresh_independent_process_response(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let before = h
        .panel
        .read_with(cx, |panel, _| (panel.mcp_sample_id, panel.last_sample));
    let monitor = h
        .panel
        .read_with(cx, |panel, _| panel.monitor.clone())
        .unwrap_or_else(|| panic!("Windows monitor"));
    let capabilities = h._runtime.block_on(monitor.capability_report());
    let now = Instant::now();
    let stale = now
        .checked_sub(Duration::from_secs(11))
        .unwrap_or_else(|| panic!("monotonic receipt baseline"));
    let process = keelshell_session::monitor::ProcessInfo {
        pid: 101,
        command: "新进程".into(),
        user: None,
        cpu_percent: None,
        memory_percent: None,
        working_set_bytes: Some(32 * 1024 * 1024),
    };
    h.panel.update(cx, |panel, cx| {
        panel.apply_outcome(
            Outcome::Refreshed {
                snapshot: Box::new(Ok(snapshot(CpuSample::ReportedPercent(99.), 1005.))),
                sampled_at: stale,
                processes: Ok(vec![process]),
                process_sampled_at: now,
                sockets: None,
                capabilities,
            },
            now,
            cx,
        );
        assert_eq!((panel.mcp_sample_id, panel.last_sample), before);
        assert_eq!(panel.rates.cpu_busy_percent, None);
        assert_eq!(panel.processes[0].pid, 101);
        assert_eq!(panel.last_process_sample, Some(now));
        assert_eq!(panel.history.vertices(Series::Cpu, now).len(), 1);
    });
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn unconfirmed_environment_stays_unknown_without_starting_windows_collectors(
    cx: &mut TestAppContext,
) {
    let control = Arc::new(Control {
        environment_unconfirmed: AtomicBool::new(true),
        ..Control::default()
    });
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.capabilities.environment.is_none());
        assert!(panel.snapshot.is_none() && panel.processes.is_empty());
        assert!(!panel.is_windows() && !panel.is_darwin());
        assert!(!panel.linux_monitor_actions_available());
        let (zh, en) = panel.status.translations();
        assert!(zh.contains("环境") && en.contains("environment"));
        assert!(!zh.contains("命令未找到") && !en.contains("command is missing"));
    });
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("monitor-windows-identity").is_none());
        assert_eq!(
            window.find("monitor-environment").label(),
            Some("平台与运行时未知")
        );
    })
    .checked("unconfirmed environment does not imply Windows");
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), 0);
    assert_eq!(h.control.process_commands.load(Ordering::Acquire), 0);
    assert_eq!(h.control.sockets.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_environment_blocks_even_a_retained_linux_snapshot_and_reviewed_sigterm(
    cx: &mut TestAppContext,
) {
    let mut donor = Harness::new(cx);
    donor.idle(cx).await;
    donor
        .panel
        .update(cx, |panel, cx| panel.set_paused(true, cx));
    donor.control.inspect.add_permits(1);
    let (monitor, process, linux_snapshot) = donor.panel.read_with(cx, |panel, _| {
        (
            panel
                .monitor
                .clone()
                .unwrap_or_else(|| panic!("Linux monitor")),
            panel.processes[0].clone(),
            panel.snapshot.clone(),
        )
    });
    let identity = donor
        ._runtime
        .block_on(monitor.inspect_process(&process))
        .checked("controlled Linux identity for stale-action guard");
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        panel.snapshot = linux_snapshot;
        panel.pending = Some(identity);
        assert_windows_environment(panel);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("process-confirmation").is_none());
        assert!(window.try_find("confirm-process-term").is_none());
        assert!(window.try_find(("review-process", 7_usize)).is_none());
    })
    .checked("Windows environment revokes retained Linux confirmation UI");
    let commands = h.control.commands.load(Ordering::Acquire);
    h.panel.update(cx, |panel, cx| {
        let identity = panel
            .pending
            .take()
            .unwrap_or_else(|| panic!("test stale identity"));
        panel.run(Job::Terminate(identity), cx);
        assert!(!panel.busy && panel.pending.is_none());
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    assert_eq!(donor.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
    donor.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn windows_explicit_empty_lists_are_distinct_from_failed_collection(cx: &mut TestAppContext) {
    let control = control();
    control.windows_empty.store(true, Ordering::Release);
    let mut h = Harness::with_control(cx, control);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.processes.is_empty() && panel.sockets.is_empty());
        assert_eq!(
            panel.capabilities.processes.state,
            CapabilityState::Available
        );
        assert_eq!(panel.capabilities.sockets.state, CapabilityState::Available);
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("monitor-process-empty").label(),
            Some("当前账户未观察到进程")
        );
        assert_eq!(
            window.find("monitor-socket-empty").label(),
            Some("当前账户未观察到 TCP 监听或已绑定的 UDP 端口")
        );
    })
    .checked("explicit empty arrays retain successful account scope");
    h.control.process_failure.store(126, Ordering::Release);
    h.control.socket_failure.store(126, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Redetect, cx));
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("monitor-process-empty").label(),
            Some("暂无进程采样")
        );
        assert_eq!(
            window.find("monitor-socket-empty").label(),
            Some("暂无可用端口采样；可点击端口或重新检测")
        );
    })
    .checked("failed collection does not claim an empty success");
    h.close_history_fixture(cx).await;
}

fn click_scrolled(window: &mut gpui_kit::Window, id: &'static str, cx: &mut gpui_kit::App) {
    for _ in 0..160 {
        window.render_frame(cx);
        let bounds = window.find(id).bounds();
        let viewport = window.find("monitor-scroll").bounds();
        if bounds.origin.y >= viewport.origin.y && bounds.bottom() <= viewport.bottom() {
            window.click(id, cx);
            window.render_frame(cx);
            return;
        }
        let lines = if bounds.origin.y < viewport.origin.y {
            2.
        } else {
            -2.
        };
        window.scroll(
            "monitor-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., lines)),
            cx,
        );
    }
    panic!("{id} was not reachable within the bounded monitor scroll");
}

#[gpui_kit::test]
async fn windows_three_theme_bilingual_compact_layout_and_paging_add_zero_ssh_commands(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    h.load_sockets(cx);
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.capabilities,
            panel.mcp_sample_id,
            panel.last_sample,
            panel.processes.clone(),
            panel.sockets.clone(),
        )
    });
    for (width, height) in [(900., 580.), (1440., 960.)] {
        let panel = h.panel.clone();
        let (window, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(width), px(height)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| HistoryTestSurface(panel)),
            )
            .checked("compact Windows monitor surface")
        });
        for family in crate::design::registry() {
            for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(window, |_, window, cx| {
                        let settings = keelshell_core::Settings {
                            theme_family: keelshell_core::ThemeId::new(family.id)
                                .checked("registered Windows monitor theme"),
                            theme: mode,
                            ..Default::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        crate::i18n::set_language(language, cx);
                        h.panel
                            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                        window.render_frame(cx);
                        let english = language == Language::En;
                        assert_eq!(
                            window.find("monitor-windows-identity").label(),
                            Some(if english {
                                "Windows · Read-only monitoring"
                            } else {
                                "Windows · 只读监控"
                            })
                        );
                        let used = if english { "Memory" } else { "内存" };
                        assert_eq!(
                            window.find(format!("monitor-metric-{used}")).label(),
                            Some(
                                format!("{used}: {}", if english { "Unknown" } else { "未知" })
                                    .as_str()
                            )
                        );
                        assert_eq!(
                            window.find("monitor-load-average").label(),
                            Some(if english {
                                "Load 1/5/15: Unknown"
                            } else {
                                "负载 1/5/15: 未知"
                            })
                        );
                        for (id, required) in [
                            (
                                "monitor-windows-cpu-source",
                                if english {
                                    "one-second average"
                                } else {
                                    "一秒均值"
                                },
                            ),
                            (
                                "monitor-windows-uptime-source",
                                if english {
                                    "wall-clock estimate"
                                } else {
                                    "墙上时钟估计"
                                },
                            ),
                            (
                                "monitor-windows-unknown-metrics",
                                if english { "iowait" } else { "I/O 等待" },
                            ),
                        ] {
                            let element = window.find(id);
                            assert!(
                                element
                                    .label()
                                    .is_some_and(|label| label.contains(required))
                            );
                            let bounds = element.bounds();
                            let viewport = window.find("monitor-scroll").bounds();
                            assert!(
                                bounds.origin.x >= viewport.origin.x
                                    && bounds.right() <= viewport.right()
                            );
                        }
                        assert!(
                            window.find("monitor-environment").label().is_some_and(
                                |text| text.contains("Windows PowerShell 5.1.26100.123")
                            )
                        );
                        assert!(
                            window
                                .find(("monitor-process", 7_usize))
                                .label()
                                .is_some_and(|text| text.contains(if english {
                                    "Working set 16.0 MiB"
                                } else {
                                    "工作集 16.0 MiB"
                                }))
                        );
                        assert!(window.try_find(("review-process", 7_usize)).is_none());
                        assert!(window.try_find(("probe-socket", 0_usize)).is_none());
                        assert!(window.try_find("confirm-process-term").is_none());
                        for id in [
                            "refresh-monitor",
                            "pause-monitor",
                            "refresh-sockets",
                            "redetect-monitor",
                        ] {
                            let bounds = window.find(id).bounds();
                            assert!(window.find(id).visible());
                            assert!(bounds.origin.x >= px(0.) && bounds.right() <= px(width));
                            assert!(bounds.origin.y >= px(0.) && bounds.bottom() <= px(height));
                        }
                        click_scrolled(window, "process-page-next", cx);
                        assert_eq!(h.panel.read(cx).process_page, 1);
                        assert!(window.find(("monitor-process", 35_usize)).label().is_some());
                        click_scrolled(window, "process-page-previous", cx);
                        assert_eq!(h.panel.read(cx).process_page, 0);
                        click_scrolled(window, "socket-page-next", cx);
                        assert_eq!(h.panel.read(cx).socket_page, 1);
                        assert!(
                            window
                                .find("monitor-socket-page")
                                .label()
                                .is_some_and(|label| {
                                    label.contains(if english {
                                        "Page 2 of 2"
                                    } else {
                                        "第 2 / 2 页"
                                    })
                                })
                        );
                        click_scrolled(window, "socket-page-previous", cx);
                        assert_eq!(h.panel.read(cx).socket_page, 0);
                    })
                    .checked("all Windows themes languages bounds and real scrolling pagination");
                    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
                }
            }
        }
    }
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            (
                panel.snapshot.clone(),
                panel.rates.clone(),
                panel.capabilities,
                panel.mcp_sample_id,
                panel.last_sample,
                panel.processes.clone(),
                panel.sockets.clone()
            ),
            before
        );
        let mcp = panel
            .mcp_snapshot()
            .unwrap_or_else(|| panic!("cached Windows MCP sample"));
        assert_eq!(
            mcp.sample_id,
            before.3.unwrap_or_else(|| panic!("original sample ID"))
        );
        assert_eq!(mcp.cpu_percent, Some(25.));
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.inspecting.load(Ordering::Acquire), 0);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}
