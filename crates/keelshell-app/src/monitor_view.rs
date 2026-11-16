//! Build each monitor section in a separate, bounded rendering stack frame.
use super::*;

// GPUI style builders keep large by-value temporaries in debug builds. Separate
// frames let one section finish before another starts, including when the whole
// panel is rendered inside the workspace. Do not inline them into `render`.
impl MonitorPanel {
    #[inline(never)]
    pub(super) fn controls_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let components = crate::design::components(cx);
        let monitor_state = if self.suspended {
            (
                t(cx, "快照", "Snapshot"),
                t(
                    cx,
                    "上一会话快照；不会继续采集",
                    "Previous-session snapshot; collection is stopped",
                ),
                visual.warning,
            )
        } else if self.busy {
            (
                t(cx, "读取中", "Updating"),
                t(
                    cx,
                    "正在从主机读取监控数据",
                    "Reading monitoring data from the host",
                ),
                visual.accent,
            )
        } else if self.paused {
            (
                t(cx, "已暂停", "Paused"),
                t(
                    cx,
                    "自动刷新已暂停，可手动刷新",
                    "Automatic refresh is paused; manual refresh is available",
                ),
                visual.warning,
            )
        } else {
            (
                t(cx, "实时", "Live"),
                t(
                    cx,
                    "自动刷新每 5 秒运行",
                    "Automatic refresh runs every 5 seconds",
                ),
                visual.success,
            )
        };
        let controls = div()
            .id("monitor-controls")
            .test_support()
            .role(accesskit::Role::Group)
            .aria_label(t(cx, "主机监控控制区", "Host monitor controls"))
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(
                                div()
                                    .w(px(22.))
                                    .h(px(22.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(components.control_radius))
                                    .bg(rgb(visual.surface_active))
                                    .text_color(rgb(visual.accent))
                                    .child(IconName::Server),
                            )
                            .child(div().flex_1().min_w_0().text_ellipsis().child(
                                if self.suspended {
                                    t(cx, "上一会话监控快照", "Previous session snapshot")
                                } else {
                                    t(cx, "主机监控", "Host monitor")
                                },
                            )),
                    )
                    .child(
                        div()
                            .id("monitor-state")
                            .test_support()
                            .role(accesskit::Role::Label)
                            .aria_label(monitor_state.1)
                            .flex_shrink_0()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(visual.surface_active))
                            .text_xs()
                            .text_color(rgb(monitor_state.2))
                            .child(monitor_state.0),
                    ),
            )
            .child(
                div()
                    .id("monitor-host")
                    .test_support()
                    .role(accesskit::Role::Label)
                    .aria_label(self.host.clone())
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(IconName::Globe)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(self.host.clone()),
                    ),
            )
            .children(self.windows_identity_view(cx))
            .child(
                div()
                    .id("monitor-actions")
                    .test_support()
                    .role(accesskit::Role::Group)
                    .aria_label(t(cx, "主机监控操作", "Host monitor actions"))
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("refresh-monitor")
                            .disabled(self.suspended)
                            .icon(IconName::RefreshCw)
                            .ghost()
                            .compact()
                            .rounded(px(components.control_radius))
                            .label(if self.busy {
                                t(cx, "读取中…", "Reading…")
                            } else {
                                t(cx, "刷新", "Refresh")
                            })
                            .accessibility_label(t(
                                cx,
                                "刷新主机监控数据",
                                "Refresh host monitoring data",
                            ))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if panel.pending.is_none() {
                                    panel.run(Job::Refresh, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("pause-monitor")
                            .disabled(self.suspended)
                            .icon(if self.paused {
                                IconName::Play
                            } else {
                                IconName::Pause
                            })
                            .ghost()
                            .compact()
                            .rounded(px(components.control_radius))
                            .label(if self.paused {
                                t(cx, "继续", "Resume")
                            } else {
                                t(cx, "暂停", "Pause")
                            })
                            .accessibility_label(if self.paused {
                                t(cx, "继续自动刷新", "Resume automatic refresh")
                            } else {
                                t(cx, "暂停自动刷新", "Pause automatic refresh")
                            })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if !panel.suspended && panel.pending.is_none() {
                                    panel.set_paused(!panel.paused, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("refresh-sockets")
                            .disabled(self.suspended || self.busy || self.pending.is_some())
                            .icon(IconName::Network)
                            .ghost()
                            .compact()
                            .rounded(px(components.control_radius))
                            .label(t(cx, "端口", "Sockets"))
                            .accessibility_label(t(cx, "读取端口", "Read sockets"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if panel.pending.is_none() {
                                    panel.run(Job::Sockets, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(visual.muted))
                            .child(
                                Message::new(
                                    format!(
                                        "资源 {} · 进程 {}",
                                        age(self.last_sample, cx),
                                        age(self.last_process_sample, cx)
                                    ),
                                    format!(
                                        "Resources {} · Processes {}",
                                        age(self.last_sample, cx),
                                        age(self.last_process_sample, cx)
                                    ),
                                )
                                .render(cx),
                            ),
                    )
                    .child(self.redetect_button(cx)),
            );
        controls.into_any_element()
    }

    #[inline(never)]
    pub(super) fn resources_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let unknown = if self.is_windows() {
            t(cx, "未知", "Unknown")
        } else {
            "—"
        };
        let history_now = Instant::now();
        let cpu = self.rates.cpu_busy_percent;
        let memory = self.snapshot.as_ref().map(|snapshot| &snapshot.memory);
        let memory_ratio = memory.and_then(|m| {
            m.used_bytes()
                .filter(|_| m.total_bytes > 0)
                .map(|used| used as f32 / m.total_bytes as f32)
        });
        let memory_value = memory
            .and_then(|m| {
                m.used_bytes().filter(|_| m.total_bytes > 0).map(|used| {
                    format!(
                        "{:.1}% · {} / {}",
                        used as f64 / m.total_bytes as f64 * 100.,
                        bytes(used),
                        bytes(m.total_bytes)
                    )
                })
            })
            .unwrap_or_else(|| unknown.into());
        let swap = memory.and_then(|m| Some((m.swap_total_bytes?, m.swap_free_bytes?)));
        let swap_used =
            swap.and_then(|(total, free)| total.checked_sub(free).map(|used| (used, total)));
        let swap_ratio = swap_used.map(|(used, total)| {
            if total == 0 {
                0.
            } else {
                used as f32 / total as f32
            }
        });
        let mut resources = section(cx, t(cx, "资源使用", "Resources"), IconName::Activity)
            .id("monitor-resources")
            .test_support()
            .role(accesskit::Role::Group)
            .aria_label(t(cx, "资源使用", "Resources"))
            .child(metric(
                cx,
                t(cx, "CPU", "CPU"),
                cpu.map(|value| format!("{value:.1}%"))
                    .unwrap_or_else(|| unknown.into()),
                cpu.map(|value| value as f32 / 100.),
                visual.accent,
            ))
            .child(history::view(
                &self.history,
                history::Series::Cpu,
                history_now,
                cx,
            ))
            .child(metric(
                cx,
                t(cx, "内存", "Memory"),
                memory_value,
                memory_ratio,
                visual.success,
            ))
            .child(history::view(
                &self.history,
                history::Series::Memory,
                history_now,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(self.history.time_range(history_now, cx)),
            )
            .child(metric(
                cx,
                t(cx, "交换空间", "Swap"),
                swap_used
                    .map(|(used, total)| format!("{} / {}", bytes(used), bytes(total)))
                    .unwrap_or_else(|| unknown.into()),
                swap_ratio,
                visual.warning,
            ));
        if self.is_darwin() || self.is_windows() {
            resources = resources
                .child(metric(
                    cx,
                    t(cx, "总内存", "Total memory"),
                    memory.map(|m| bytes(m.total_bytes)).unwrap_or_else(|| unknown.into()),
                    None,
                    visual.success,
                ))
                .child(metric(
                    cx,
                    t(cx, "空闲内存", "Free memory"),
                    memory.and_then(|m| m.free_bytes).map(bytes).unwrap_or_else(|| unknown.into()),
                    None,
                    visual.success,
                ))
                .children(self.is_darwin().then(|| {
                    div()
                        .id("monitor-darwin-memory-note")
                        .test_support()
                        .role(accesskit::Role::Label)
                        .aria_label(t(cx, "空闲页不等于可用内存；已用内存估计不可用。", "Free pages are not available memory; used-memory estimate unavailable."))
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .whitespace_normal()
                        .child(t(cx, "空闲页不等于可用内存；已用内存估计不可用。", "Free pages are not available memory; used-memory estimate unavailable."))
                }));
        }
        let load = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.load_average)
            .map(|load| format!("{:.2} / {:.2} / {:.2}", load[0], load[1], load[2]))
            .unwrap_or_else(|| unknown.into());
        let uptime = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                Message::new(
                    format!("{:.1} 小时", snapshot.uptime_seconds / 3600.),
                    format!("{:.1} hours", snapshot.uptime_seconds / 3600.),
                )
                .render(cx)
            })
            .unwrap_or_else(|| unknown.into());
        resources = resources
            .child(
                div()
                    .id("monitor-load-average")
                    .test_support()
                    .role(accesskit::Role::Label)
                    .aria_label(format!("{}: {load}", t(cx, "负载 1/5/15", "Load 1/5/15")))
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(t(cx, "负载 1/5/15", "Load 1/5/15"))
                    .child(load),
            )
            .child(
                div()
                    .id("monitor-uptime")
                    .test_support()
                    .role(accesskit::Role::Label)
                    .aria_label(format!("{}: {uptime}", t(cx, "运行时间", "Uptime")))
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(t(cx, "运行时间", "Uptime"))
                    .child(uptime),
            )
            .children(self.windows_sources_view(cx));
        resources.into_any_element()
    }

    #[inline(never)]
    fn windows_identity_view(&self, cx: &App) -> Option<AnyElement> {
        self.is_windows().then(|| {
            let visual = crate::design::palette(cx);
            let label = t(cx, "Windows · 只读监控", "Windows · Read-only monitoring");
            div()
                .id("monitor-windows-identity")
                .test_support()
                .role(accesskit::Role::Label)
                .aria_label(label)
                .min_w_0()
                .text_xs()
                .text_color(rgb(visual.muted))
                .whitespace_normal()
                .child(label)
                .into_any_element()
        })
    }

    #[inline(never)]
    fn windows_sources_view(&self, cx: &App) -> Option<AnyElement> {
        if !self.is_windows() {
            return None;
        }
        let visual = crate::design::palette(cx);
        let mut sources = div().min_w_0().flex().flex_col().gap_1();
        for (id, zh, en) in [
            (
                "monitor-windows-cpu-source",
                "CPU 来源：CIM Win32_Processor.LoadPercentage，提供程序的一秒均值；不是本次刷新区间的 tick 差值。",
                "CPU source: CIM Win32_Processor.LoadPercentage, the provider's one-second average; not tick deltas over this refresh interval.",
            ),
            (
                "monitor-windows-uptime-source",
                "运行时间来源：CIM LocalDateTime − LastBootUpTime，系统墙上时钟估计，可能随校时变化。",
                "Uptime source: CIM LocalDateTime − LastBootUpTime, a system wall-clock estimate that can change with clock adjustments.",
            ),
            (
                "monitor-windows-unknown-metrics",
                "未知：负载、可用／已用内存、交换空间、网速、CPU I/O 等待与磁盘 I/O。空闲内存不代表可用内存；网络仅显示实际累计字节。",
                "Unknown: load, available/used memory, swap, network rates, CPU iowait and disk I/O. Free memory is not available memory; network bytes are actual cumulative counters.",
            ),
        ] {
            let label = t(cx, zh, en);
            sources = sources.child(
                div()
                    .id(id)
                    .test_support()
                    .role(accesskit::Role::Label)
                    .aria_label(label)
                    .min_w_0()
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .whitespace_normal()
                    .child(label),
            );
        }
        Some(sources.into_any_element())
    }

    #[inline(never)]
    pub(super) fn networks_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let components = crate::design::components(cx);
        let mut networks = section(cx, t(cx, "网络", "Network"), IconName::Network)
            .id("monitor-network")
            .test_support()
            .role(accesskit::Role::Group)
            .aria_label(t(cx, "网络", "Network"));
        if let Some(snapshot) = &self.snapshot {
            for (interface_index, interface) in snapshot.networks.iter().enumerate() {
                let sample = self
                    .rates
                    .networks
                    .iter()
                    .find(|rate| rate.name == interface.name);
                networks = networks.child(
                    div()
                        .id(("monitor-network-interface", interface_index))
                        .test_support()
                        .role(accesskit::Role::Group)
                        .aria_label(interface.name.clone())
                        .min_w_0()
                        .p_2()
                        .rounded(px(components.control_radius))
                        .bg(rgb(visual.surface))
                        .border_1()
                        .border_color(rgb(visual.border))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_xs()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(visual.muted))
                                .child(interface.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_color(rgb(visual.success))
                                        .child(IconName::ArrowDown)
                                        .child(rate(
                                            sample.and_then(|r| r.received_bytes_per_second),
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_color(rgb(visual.accent))
                                        .child(IconName::ArrowUp)
                                        .child(rate(
                                            sample.and_then(|r| r.transmitted_bytes_per_second),
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .text_color(rgb(visual.muted))
                                .whitespace_normal()
                                .child(
                                    Message::new(
                                        format!(
                                            "接收 {} · 发送 {}",
                                            bytes(interface.received_bytes),
                                            bytes(interface.transmitted_bytes)
                                        ),
                                        format!(
                                            "Received {} · Sent {}",
                                            bytes(interface.received_bytes),
                                            bytes(interface.transmitted_bytes)
                                        ),
                                    )
                                    .render(cx),
                                ),
                        ),
                );
            }
        } else {
            networks = networks.child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "暂无网络采样",
                "No network sample",
            )));
        }
        networks.into_any_element()
    }

    #[inline(never)]
    pub(super) fn filesystems_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut disks = section(
            cx,
            t(cx, "磁盘容量", "Filesystem capacity"),
            IconName::HardDrive,
        )
        .id("monitor-filesystems")
        .test_support()
        .role(accesskit::Role::Group)
        .aria_label(t(cx, "磁盘容量", "Filesystem capacity"));
        if self.is_windows() {
            disks = disks.child(
                div()
                    .id("monitor-windows-capacity-source")
                    .test_support()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(rgb(visual.muted))
                    .child(t(
                        cx,
                        "来源：DriveInfo 就绪固定卷；已用容量按总容量减总空闲计算，可用容量保留账户限制。",
                        "Source: DriveInfo ready fixed volumes; usage is total minus total free. Available capacity retains account limits.",
                    )),
            );
        }
        if let Some(snapshot) = &self.snapshot {
            for filesystem in &snapshot.filesystems {
                disks = disks
                    .child(metric(
                        cx,
                        &filesystem.mount,
                        format!("{}%", filesystem.used_percent),
                        Some(filesystem.used_percent as f32 / 100.),
                        visual.resource,
                    ))
                    .child(
                        div().text_xs().text_color(rgb(visual.muted)).child(
                            Message::new(
                                format!(
                                    "可用 {} / {}",
                                    bytes(filesystem.available_bytes),
                                    bytes(filesystem.total_bytes)
                                ),
                                format!(
                                    "{} {} / {}",
                                    if self.is_windows() {
                                        "Available"
                                    } else {
                                        "Free"
                                    },
                                    bytes(filesystem.available_bytes),
                                    bytes(filesystem.total_bytes)
                                ),
                            )
                            .render(cx),
                        ),
                    );
            }
            if snapshot.filesystems.is_empty() {
                let empty = t(
                    cx,
                    if self.capabilities.filesystems.state == CapabilityState::Available {
                        "未发现挂载点"
                    } else {
                        "容量不可用；CPU、内存和网络独立采集"
                    },
                    if self.capabilities.filesystems.state == CapabilityState::Available {
                        "No mounts found"
                    } else {
                        "Capacity unavailable; CPU, memory and network collect independently"
                    },
                );
                disks = disks.child(
                    div()
                        .id("monitor-filesystem-empty")
                        .test_support()
                        .role(accesskit::Role::Label)
                        .aria_label(empty)
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(empty),
                );
            }
        } else {
            disks = disks.child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "暂无磁盘采样",
                "No filesystem sample",
            )));
        }
        disks.into_any_element()
    }

    #[inline(never)]
    pub(super) fn processes_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut processes = section(cx, t(cx, "进程", "Processes"), IconName::Cpu)
            .id("monitor-processes")
            .test_support()
            .role(accesskit::Role::Group)
            .aria_label(t(cx, "进程", "Processes"))
            .child(div().text_xs().whitespace_normal().text_color(rgb(visual.muted)).child(t(
                cx,
                if self.is_windows() {
                    "Windows · Get-Process · PID 排序 · 只读。显示进程名与工作集；用户、CPU% 和内存% 未知。工作集不是已用内存。"
                } else if self.is_darwin() {
                    "macOS · 按 CPU 排序 · 只读列表，不支持终止"
                } else {
                    "按 CPU 排序 · 生命周期均值"
                },
                if self.is_windows() {
                    "Windows · Get-Process · PID order · Read-only. Process names and working sets; user, CPU% and memory% unknown. Working set is not used memory."
                } else if self.is_darwin() {
                    "macOS · CPU order · Read-only list; stopping unavailable"
                } else {
                    "CPU order · lifetime average"
                },
            )));
        processes = processes.child(
            div()
                .flex()
                .gap_1()
                .text_xs()
                .text_color(rgb(visual.muted))
                .child(
                    div()
                        .w(px(if self.is_windows() { 50. } else { 42. }))
                        .child("CPU"),
                )
                .child(
                    div()
                        .w(px(if self.is_windows() { 70. } else { 42. }))
                        .child(if self.is_windows() {
                            t(cx, "工作集", "Working set")
                        } else {
                            t(cx, "内存", "MEM")
                        }),
                )
                .child(div().flex_1().min_w_0().child(if self.is_windows() {
                    t(cx, "PID / 进程名", "PID / Name")
                } else {
                    t(cx, "PID / 命令", "PID / Command")
                })),
        );
        for process in self
            .processes
            .iter()
            .skip(self.process_page * PROCESS_PAGE_SIZE)
            .take(PROCESS_PAGE_SIZE)
        {
            let selected = process.clone();
            let unknown = t(cx, "未知", "Unknown");
            let cpu = process
                .cpu_percent
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| unknown.into());
            let memory = if self.is_windows() {
                process
                    .working_set_bytes
                    .map(bytes)
                    .unwrap_or_else(|| unknown.into())
            } else {
                process
                    .memory_percent
                    .map(|value| format!("{value:.1}%"))
                    .unwrap_or_else(|| unknown.into())
            };
            let user = process.user.as_deref().unwrap_or(unknown);
            let label = if self.is_windows() {
                Message::new(
                    format!("PID {} · 进程名 {} · 用户 {user} · CPU {cpu} · 内存% 未知 · 工作集 {memory}", process.pid, process.command),
                    format!("PID {} · Process name {} · User {user} · CPU {cpu} · Memory% unknown · Working set {memory}", process.pid, process.command),
                ).render(cx)
            } else {
                format!(
                    "PID {} · {} · {user} · CPU {cpu} · {} {memory}",
                    process.pid,
                    process.command,
                    t(cx, "内存", "MEM")
                )
            };
            let mut row = div()
                .id(("monitor-process", process.pid as usize))
                .test_support()
                .role(accesskit::Role::Group)
                .aria_label(label)
                .min_w_0()
                .flex()
                .items_center()
                .gap_1()
                .py_1()
                .border_b_1()
                .border_color(rgb(visual.border))
                .text_xs()
                .child(
                    div()
                        .w(px(if self.is_windows() { 50. } else { 42. }))
                        .flex_shrink_0()
                        .truncate()
                        .child(cpu),
                )
                .child(
                    div()
                        .w(px(if self.is_windows() { 70. } else { 42. }))
                        .flex_shrink_0()
                        .child(memory),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(format!("{} {}", process.pid, process.command)),
                );
            if process.pid > 1 && self.linux_monitor_actions_available() {
                row = row.child(
                    Button::new(("review-process", process.pid as usize))
                        .text_color(rgb(visual.danger))
                        .ghost()
                        .compact()
                        .rounded(px(crate::design::components(cx).control_radius))
                        .label(t(cx, "终止", "Stop"))
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            if panel.busy || panel.pending.is_some() {
                                return;
                            }
                            panel.set_paused(true, cx);
                            panel.run(Job::Inspect(selected.clone()), cx);
                        })),
                );
            }
            processes = processes.child(row);
        }
        if self.processes.is_empty() {
            let empty = t(
                cx,
                if self.is_windows()
                    && self.capabilities.processes.state == CapabilityState::Available
                {
                    "当前账户未观察到进程"
                } else {
                    "暂无进程采样"
                },
                if self.is_windows()
                    && self.capabilities.processes.state == CapabilityState::Available
                {
                    "No processes visible to this account"
                } else {
                    "No process sample"
                },
            );
            processes = processes.child(
                div()
                    .id("monitor-process-empty")
                    .test_support()
                    .role(accesskit::Role::Label)
                    .aria_label(empty)
                    .text_xs()
                    .text_color(rgb(visual.muted))
                    .child(empty),
            );
        }
        let pages = self.processes.len().div_ceil(PROCESS_PAGE_SIZE).max(1);
        if pages > 1 {
            processes = processes.child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("process-page-previous")
                            .icon(IconName::ChevronLeft)
                            .ghost()
                            .compact()
                            .rounded(px(crate::design::components(cx).control_radius))
                            .label(t(cx, "上一页", "Previous"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.process_page = panel.process_page.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{} / {pages}", self.process_page + 1)),
                    )
                    .child(
                        Button::new("process-page-next")
                            .icon(IconName::ChevronRight)
                            .ghost()
                            .compact()
                            .rounded(px(crate::design::components(cx).control_radius))
                            .label(t(cx, "下一页", "Next"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if (panel.process_page + 1) * PROCESS_PAGE_SIZE
                                    < panel.processes.len()
                                {
                                    panel.process_page += 1;
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        processes.into_any_element()
    }

    #[inline(never)]
    pub(super) fn process_confirmation_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.linux_monitor_actions_available() {
            return None;
        }
        let visual = crate::design::palette(cx);
        let mut confirmation = None;
        if let Some(identity) = &self.pending {
            let process = identity.process();
            confirmation = Some(div().id("process-confirmation").max_h(px(250.)).overflow_y_scroll().m_2().p_2().bg(rgb(visual.danger_surface)).rounded(px(crate::design::components(cx).control_radius)).border_1().border_color(rgb(visual.danger_border)).flex().flex_col().gap_2()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(t(cx, "确认终止进程", "Confirm termination")))
                .child(div().text_xs().child(self.host.clone()))
                .child(div().text_xs().child(format!("PID {} · {}", process.pid, process.user.as_deref().unwrap_or(t(cx, "未知", "Unknown")))))
                .child(div().text_xs().child(process.command.clone()))
                .child(div().text_xs().child(t(cx, "将发送 SIGTERM，可能中断任务。不会强制终止或提权；启动时间复核仍有极小的 PID 重用竞争。", "SIGTERM may interrupt work. No force kill or privilege escalation; a narrow PID reuse race remains after the start-time check.")))
                .child(div().flex().gap_1()
                    .child(Button::new("confirm-process-term").icon(IconName::Square).primary().compact().rounded(px(crate::design::components(cx).control_radius)).label(t(cx, "发送 SIGTERM", "Send SIGTERM"))
                        .on_click(cx.listener(|panel, _, _, cx| { if !panel.busy && let Some(identity) = panel.pending.take() { panel.run(Job::Terminate(identity), cx); } })))
                    .child(Button::new("cancel-process-term").ghost().compact().rounded(px(crate::design::components(cx).control_radius)).label(t(cx, "取消", "Cancel"))
                        .on_click(cx.listener(|panel, _, _, cx| { panel.pending = None; panel.status = Message::new("已取消终止，自动刷新仍暂停。", "Termination cancelled. Polling remains paused."); cx.notify(); })))));
        }
        confirmation.map(|view| view.into_any_element())
    }
}
