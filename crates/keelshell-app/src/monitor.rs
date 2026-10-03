//! Live Linux monitoring, with a separately confirmed SIGTERM workflow.

use crate::design::{ACCENT, BORDER, CANVAS, MUTED, SURFACE, TEXT};
#[cfg(test)]
#[path = "monitor_tests.rs"]
mod tests;
use gpui_kit::assets::IconName;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    time::{Duration, Instant},
};

use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
    },
    *,
};
use keelshell_session::{
    SshSession,
    monitor::{
        LinuxMonitor, MonitorError, MonitorResult, ProcessIdentity, ProcessInfo, SampleRates,
        Snapshot, SocketInfo, TcpProbeResult,
    },
};

enum Job {
    Refresh,
    Sockets,
    Probe(SocketInfo),
    Inspect(ProcessInfo),
    Terminate(ProcessIdentity),
}
use crate::{
    i18n::{Message, t},
    terminal::spawn_transport_worker,
};

const PROCESS_PAGE_SIZE: usize = 5;

enum Outcome {
    Cancelled,
    Refreshed {
        snapshot: MonitorResult<Snapshot>,
        processes: MonitorResult<Vec<ProcessInfo>>,
    },
    Sockets(MonitorResult<Vec<SocketInfo>>),
    Probed {
        socket: SocketInfo,
        result: MonitorResult<TcpProbeResult>,
    },
    Inspected(MonitorResult<ProcessIdentity>),
    Terminated(MonitorResult<()>),
}

/// Resource and process panel bound to one authenticated SSH connection.
///
/// Opening it performs read-only collection. Automatic five-second refresh can
/// be paused; process inspection and the final SIGTERM confirmation are separate
/// actions. No process is stopped merely by selecting it.
pub struct MonitorPanel {
    monitor: Option<LinuxMonitor>,
    suspended: bool,
    host: String,
    runtime: Arc<tokio::runtime::Runtime>,
    snapshot: Option<Snapshot>,
    rates: SampleRates,
    processes: Vec<ProcessInfo>,
    sockets: Vec<SocketInfo>,
    probes: BTreeMap<String, TcpProbeResult>,
    process_page: usize,
    last_sample: Option<Instant>,
    last_process_sample: Option<Instant>,
    busy: bool,
    paused: bool,
    status: Message,
    pending: Option<ProcessIdentity>,
    worker_cancel: Option<Arc<AtomicBool>>,
    _job: Option<Task<()>>,
    _poll: Task<()>,
}

impl MonitorPanel {
    /// Start a Linux panel with an immutable, authenticated connection label.
    ///
    /// Workers belong to the application registry. Closing the panel cancels work
    /// that has not started; an already sent SIGTERM is awaited, never retried.
    /// Application shutdown joins workers after their SSH deadlines complete.
    pub fn new(
        session: SshSession,
        host: String,
        runtime: Arc<tokio::runtime::Runtime>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let executor = cx.background_executor().clone();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(5)).await;
                if this
                    .update(cx, |panel, cx| {
                        if !panel.paused && panel.pending.is_none() {
                            panel.run(Job::Refresh, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut panel = Self {
            monitor: Some(LinuxMonitor::new(session)),
            suspended: false,
            host,
            runtime,
            snapshot: None,
            rates: SampleRates::default(),
            processes: Vec::new(),
            sockets: Vec::new(),
            probes: BTreeMap::new(),
            process_page: 0,
            last_sample: None,
            last_process_sample: None,
            busy: false,
            paused: false,
            status: Message::empty(),
            pending: None,
            worker_cancel: None,
            _job: None,
            _poll: poll,
        };
        panel.run(Job::Refresh, cx);
        panel
    }

    /// Stop collection and revoke pending process actions, retaining displayed data.
    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        self.suspended = true;
        self.paused = true;
        self.pending = None;
        self.monitor = None;
        if let Some(cancel) = &self.worker_cancel {
            cancel.store(true, Ordering::Release);
        }
        cx.notify();
    }

    /// Redraw labels without restarting collection or changing the selected process.
    pub fn refresh_locale(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn apply_outcome(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        self.busy = false;
        self.worker_cancel = None;
        // A sent SIGTERM still has a meaningful receipt. Other old results must
        // not refresh snapshots or resurrect process confirmation dialogs.
        if self.suspended && !matches!(outcome, Outcome::Terminated(_)) {
            return;
        }
        match outcome {
            Outcome::Cancelled => {
                self.status = Message::new(
                    "下一条命令发送前已取消监控。",
                    "Monitoring cancelled before the next command was sent.",
                )
            }
            Outcome::Refreshed {
                snapshot,
                processes,
            } => {
                let mut errors_zh = Vec::new();
                let mut errors_en = Vec::new();
                match snapshot {
                    Ok(snapshot) => {
                        self.rates = self
                            .snapshot
                            .as_ref()
                            .map(|old| snapshot.rates_since(old))
                            .unwrap_or_default();
                        self.snapshot = Some(snapshot);
                        self.last_sample = Some(Instant::now());
                    }
                    Err(error) => {
                        self.rates = SampleRates::default();
                        let (zh, en) = error_text(&error);
                        errors_zh.push(format!("资源：{zh}"));
                        errors_en.push(format!("Resources: {en}"));
                    }
                }
                match processes {
                    Ok(processes) => {
                        self.processes = processes;
                        self.process_page = self
                            .process_page
                            .min(self.processes.len().saturating_sub(1) / PROCESS_PAGE_SIZE);
                        self.last_process_sample = Some(Instant::now());
                    }
                    Err(error) => {
                        let (zh, en) = error_text(&error);
                        errors_zh.push(format!("进程：{zh}"));
                        errors_en.push(format!("Processes: {en}"));
                    }
                }
                self.status = if errors_zh.is_empty() {
                    Message::new(
                        "已更新；CPU 与网速需要两次有效采样。",
                        "Updated; CPU and network rates need two valid samples.",
                    )
                } else {
                    Message::new(
                        format!("刷新不完整，保留的值为旧数据。{}", errors_zh.join(" · ")),
                        format!(
                            "Refresh incomplete; retained values are old. {}",
                            errors_en.join(" · ")
                        ),
                    )
                };
            }
            Outcome::Sockets(result) => match result {
                Ok(sockets) => {
                    self.status = Message::new(
                        format!("已读取 {} 个监听端口。", sockets.len()),
                        format!("Loaded {} listening sockets.", sockets.len()),
                    );
                    self.sockets = sockets;
                    self.probes.clear();
                }
                Err(error) => {
                    let (zh, en) = error_text(&error);
                    self.status = Message::new(
                        format!("端口诊断失败：{zh}"),
                        format!("Socket diagnostics failed: {en}"),
                    );
                    self.sockets.clear();
                }
            },
            Outcome::Probed { socket, result } => match result {
                Ok(probe) => {
                    self.status = Message::new(
                        if probe.reachable {
                            format!("TCP {}:{} 可建立连接。", probe.host, probe.port)
                        } else {
                            format!(
                                "TCP {}:{} 未建立连接（nc 状态 {}）。",
                                probe.host, probe.port, probe.probe_status
                            )
                        },
                        if probe.reachable {
                            format!("TCP {}:{} accepted a connection.", probe.host, probe.port)
                        } else {
                            format!(
                                "TCP {}:{} did not accept a connection (nc status {}).",
                                probe.host, probe.port, probe.probe_status
                            )
                        },
                    );
                    self.probes.insert(socket_key(&socket), probe);
                }
                Err(error) => {
                    let (zh, en) = error_text(&error);
                    self.status = Message::new(
                        format!("TCP 探测失败：{zh}"),
                        format!("TCP probe failed: {en}"),
                    );
                    self.probes.remove(&socket_key(&socket));
                }
            },
            Outcome::Inspected(result) => match result {
                Ok(identity) => {
                    self.pending = Some(identity);
                    self.status = Message::new(
                        "请核对主机与进程后确认终止。自动刷新已暂停。",
                        "Review the host and process before confirming termination. Polling is paused.",
                    );
                }
                Err(error) => {
                    let (zh, en) = error_text(&error);
                    self.status = Message::new(
                        format!("进程核对失败：{zh}"),
                        format!("Process review failed: {en}"),
                    );
                }
            },
            Outcome::Terminated(result) => {
                self.status = match result {
                    Ok(()) => Message::new(
                        "已发送 SIGTERM，请刷新确认进程是否退出。自动刷新仍暂停。",
                        "SIGTERM sent; refresh to verify whether the process exited. Polling remains paused.",
                    ),
                    Err(error) => {
                        let (zh, en) = error_text(&error);
                        Message::new(
                            format!("终止结果：{zh}。超时可能代表结果未知，请核对后再操作。"),
                            format!(
                                "SIGTERM result: {en}. After timeout the result may be unknown; inspect before retrying."
                            ),
                        )
                    }
                }
            }
        }
        cx.notify();
    }

    fn run(&mut self, job: Job, cx: &mut Context<Self>) {
        let Some(monitor) = self.monitor.clone().filter(|_| !self.suspended) else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = match &job {
            Job::Refresh => Message::new("正在刷新主机状态…", "Refreshing from the host…"),
            Job::Sockets => Message::new("正在读取监听端口…", "Reading listening sockets…"),
            Job::Probe(_) => Message::new(
                "正在执行只读 TCP 连接探测…",
                "Running a read-only TCP connect probe…",
            ),
            Job::Inspect(_) => Message::new(
                "正在核对进程身份，请稍候…",
                "Checking process identity before confirmation…",
            ),
            Job::Terminate(_) => Message::new(
                "正在发送已确认的 SIGTERM…",
                "Sending the confirmed SIGTERM once…",
            ),
        };
        let runtime = self.runtime.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (outgoing, incoming) = mpsc::sync_channel(1);
        let started = spawn_transport_worker("keelshell-monitor", cancel.clone(), move || {
            let outcome = runtime.block_on(execute_job(monitor, job, &worker_cancel));
            // Registry owns and joins the worker even after this view disappears.
            // In-flight writes run to a bounded outcome, never detached/retried.
            let failure = match &outcome {
                Outcome::Terminated(Err(error)) => Some(error.to_string()),
                _ => None,
            };
            let _ = outgoing.try_send(outcome);
            failure.map_or(Ok(()), Err)
        });
        if let Err(error) = started {
            self.busy = false;
            self.status = Message::detail(
                "无法启动监控任务",
                "Unable to start monitoring worker",
                error,
            );
            cx.notify();
            return;
        }
        self.worker_cancel = Some(cancel);
        let executor = cx.background_executor().clone();
        self._job = Some(cx.spawn(async move |this, cx| {
            let outcome = loop {
                match incoming.try_recv() {
                    Ok(outcome) => break outcome,
                    Err(TryRecvError::Empty) => executor.timer(Duration::from_millis(30)).await,
                    Err(TryRecvError::Disconnected) => {
                        let _ = this.update(cx, |panel, cx| {
                            panel.busy = false;
                            panel.status = Message::new("监控任务未返回结果，请刷新后再操作。", "Monitoring worker ended without a result. Refresh before retrying any action.");
                            cx.notify();
                        });
                        return;
                    },
                }
            };
            let _ = this.update(cx, |panel, cx| {
                panel.apply_outcome(outcome, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Drop for MonitorPanel {
    fn drop(&mut self) {
        if let Some(cancel) = &self.worker_cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}

async fn execute_job(monitor: LinuxMonitor, job: Job, cancel: &AtomicBool) -> Outcome {
    // Cancellation deliberately does not drop an in-flight SSH exec future: its
    // bounded completion closes the channel and records unknown write outcomes.
    if cancel.load(Ordering::Acquire) {
        return Outcome::Cancelled;
    }
    match job {
        Job::Refresh => {
            let snapshot = monitor.snapshot().await;
            if cancel.load(Ordering::Acquire) {
                return Outcome::Cancelled;
            }
            let processes = monitor.processes().await;
            Outcome::Refreshed {
                snapshot,
                processes,
            }
        }
        Job::Sockets => Outcome::Sockets(monitor.listening_sockets().await),
        Job::Probe(socket) => Outcome::Probed {
            result: monitor.probe_tcp(&socket).await,
            socket,
        },
        Job::Inspect(process) => Outcome::Inspected(monitor.inspect_process(&process).await),
        Job::Terminate(identity) => Outcome::Terminated(monitor.terminate(identity).await),
    }
}

fn bytes(value: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if value >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", value as f64 / GIB)
    } else {
        format!("{:.1} MiB", value as f64 / 1024.0 / 1024.0)
    }
}

fn rate(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.1} KiB/s", value / 1024.0))
        .unwrap_or_else(|| "—".into())
}

fn age(sample: Option<Instant>, cx: &App) -> String {
    sample
        .map(|at| {
            Message::new(
                format!("{} 秒前", at.elapsed().as_secs()),
                format!("{}s ago", at.elapsed().as_secs()),
            )
            .render(cx)
        })
        .unwrap_or_else(|| "—".into())
}

fn error_text(error: &MonitorError) -> (String, String) {
    let zh = match error {
        MonitorError::Session(error) => format!("SSH 通信失败：{error}"),
        MonitorError::Unsupported(_) => "目标不支持此功能，需要 Linux 与相应系统命令。".into(),
        MonitorError::RemoteFailure { exit_status, .. } => match exit_status {
            Some(status) => format!("远端操作失败，退出状态 {status}"),
            None => "远端操作未返回退出状态，不能确认成功".into(),
        },
        MonitorError::InvalidData(_) => "采集响应不完整或格式不受支持".into(),
        MonitorError::ProcessChanged => "进程已退出或身份发生变化，请刷新后重新核对".into(),
        MonitorError::SocketChanged => "监听端口已变化，请刷新后重新探测".into(),
        MonitorError::InvalidPid => "只允许向大于 1 的有效 PID 发送终止信号".into(),
        MonitorError::ReviewExpired => "进程确认已过期，请重新核对".into(),
    };
    (zh, error.to_string())
}

fn socket_key(socket: &SocketInfo) -> String {
    format!("{}|{}|{}", socket.protocol, socket.local, socket.peer)
}

fn metric(label: &str, value: String, ratio: Option<f32>, color: u32) -> impl IntoElement {
    let mut track = div().h(px(5.)).w_full().rounded_sm().bg(rgb(BORDER));
    if let Some(ratio) = ratio.filter(|value| value.is_finite()) {
        track = track.child(
            div()
                .h_full()
                .w(relative(ratio.clamp(0., 1.)))
                .rounded_sm()
                .bg(rgb(color)),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .justify_between()
                .gap_2()
                .text_xs()
                .child(div().text_color(rgb(MUTED)).child(label.to_owned()))
                .child(div().text_color(rgb(TEXT)).child(value)),
        )
        .child(track)
}

fn section(label: &'static str, icon: IconName) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .py_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(TEXT))
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(MUTED)).child(icon))
                .child(label),
        )
}

impl Render for MonitorPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(rgb(BORDER))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().text_color(rgb(ACCENT)).child(IconName::Server))
                    .child(if self.suspended {
                        t(cx, "上一会话监控快照", "Previous session snapshot")
                    } else {
                        t(cx, "主机监控", "Host monitor")
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .text_ellipsis()
                    .child(self.host.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("refresh-monitor")
                            .disabled(self.suspended)
                            .icon(IconName::RefreshCw)
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(if self.busy {
                                t(cx, "读取中…", "Reading…")
                            } else {
                                t(cx, "刷新", "Refresh")
                            })
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
                            .rounded(px(6.))
                            .label(if self.paused {
                                t(cx, "继续", "Resume")
                            } else {
                                t(cx, "暂停", "Pause")
                            })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if !panel.suspended && panel.pending.is_none() {
                                    panel.paused = !panel.paused;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("refresh-sockets")
                            .disabled(self.suspended)
                            .icon(IconName::Network)
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(t(cx, "端口", "Sockets"))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                if panel.pending.is_none() {
                                    panel.run(Job::Sockets, cx);
                                }
                            })),
                    ),
            )
            .child(
                div().text_xs().text_color(rgb(MUTED)).child(
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
            );
        let cpu = self.rates.cpu_busy_percent;
        let memory = self.snapshot.as_ref().map(|snapshot| &snapshot.memory);
        let memory_ratio = memory.and_then(|m| {
            m.used_bytes()
                .filter(|_| m.total_bytes > 0)
                .map(|used| used as f32 / m.total_bytes as f32)
        });
        let memory_value = memory
            .and_then(|m| {
                m.used_bytes()
                    .map(|used| format!("{} / {}", bytes(used), bytes(m.total_bytes)))
            })
            .unwrap_or_else(|| "—".into());
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
        let mut resources = section(t(cx, "资源使用", "Resources"), IconName::Activity)
            .child(metric(
                t(cx, "CPU", "CPU"),
                cpu.map(|value| format!("{value:.1}%"))
                    .unwrap_or_else(|| "—".into()),
                cpu.map(|value| value as f32 / 100.),
                ACCENT,
            ))
            .child(metric(
                t(cx, "内存", "Memory"),
                memory_value,
                memory_ratio,
                0x329b83,
            ))
            .child(metric(
                t(cx, "交换空间", "Swap"),
                swap_used
                    .map(|(used, total)| format!("{} / {}", bytes(used), bytes(total)))
                    .unwrap_or_else(|| "—".into()),
                swap_ratio,
                0xc08a31,
            ));
        let load = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                format!(
                    "{:.2} / {:.2} / {:.2}",
                    snapshot.load_average[0], snapshot.load_average[1], snapshot.load_average[2]
                )
            })
            .unwrap_or_else(|| "—".into());
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
            .unwrap_or_else(|| "—".into());
        resources = resources
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(t(cx, "负载 1/5/15", "Load 1/5/15"))
                    .child(load),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(t(cx, "运行时间", "Uptime"))
                    .child(uptime),
            );
        let mut networks = section(t(cx, "网络", "Network"), IconName::Network);
        if let Some(snapshot) = &self.snapshot {
            for interface in &snapshot.networks {
                let sample = self
                    .rates
                    .networks
                    .iter()
                    .find(|rate| rate.name == interface.name);
                networks = networks.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_xs()
                        .child(div().text_color(rgb(MUTED)).child(interface.name.clone()))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
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
                                        .child(IconName::ArrowUp)
                                        .child(rate(
                                            sample.and_then(|r| r.transmitted_bytes_per_second),
                                        )),
                                ),
                        )
                        .child(
                            div().text_color(rgb(MUTED)).child(
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
            networks = networks.child(div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "暂无网络采样",
                "No network sample",
            )));
        }
        let mut sockets = section(t(cx, "监听端口", "Listening sockets"), IconName::Network).child(
            div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "通过 ss 读取，结果仅供诊断",
                "Read with ss for diagnostics only",
            )),
        );
        if self.sockets.is_empty() {
            sockets = sockets.child(div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "点击上方“端口”读取",
                "Click Sockets above to load",
            )));
        } else {
            for (socket_index, socket) in self.sockets.iter().take(12).enumerate() {
                let probe_key = socket_key(socket);
                let process = socket
                    .process
                    .as_deref()
                    .map(|value| format!(" · {value}"))
                    .unwrap_or_default();
                let mut row = div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .w(px(42.))
                            .text_color(rgb(MUTED))
                            .child(socket.protocol.clone()),
                    )
                    .child(
                        div()
                            .w(px(54.))
                            .text_color(rgb(MUTED))
                            .child(socket.state.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(format!("{} → {}{}", socket.local, socket.peer, process)),
                    );
                if socket.protocol == "tcp" {
                    let selected = socket.clone();
                    row = row.child(
                        Button::new(("probe-socket", socket_index))
                            .disabled(self.busy || self.suspended || self.pending.is_some())
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .label(t(cx, "探测", "Probe"))
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                if !panel.busy && panel.pending.is_none() {
                                    panel.run(Job::Probe(selected.clone()), cx);
                                }
                            })),
                    );
                }
                if let Some(probe) = self.probes.get(&probe_key) {
                    row = row.child(
                        div()
                            .text_color(rgb(if probe.reachable { 0x247a52 } else { 0xb42318 }))
                            .child(if probe.reachable { "✓" } else { "×" }),
                    );
                }
                sockets = sockets.child(row);
            }
            if self.sockets.len() > 12 {
                sockets = sockets.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(format!("… 还有 {} 个", self.sockets.len() - 12)),
                );
            }
        }
        let mut disks = section(
            t(cx, "磁盘容量", "Filesystem capacity"),
            IconName::HardDrive,
        );
        if let Some(snapshot) = &self.snapshot {
            for filesystem in &snapshot.filesystems {
                disks = disks
                    .child(metric(
                        &filesystem.mount,
                        format!("{}%", filesystem.used_percent),
                        Some(filesystem.used_percent as f32 / 100.),
                        0x448eaa,
                    ))
                    .child(
                        div().text_xs().text_color(rgb(MUTED)).child(
                            Message::new(
                                format!(
                                    "可用 {} / {}",
                                    bytes(filesystem.available_bytes),
                                    bytes(filesystem.total_bytes)
                                ),
                                format!(
                                    "Free {} / {}",
                                    bytes(filesystem.available_bytes),
                                    bytes(filesystem.total_bytes)
                                ),
                            )
                            .render(cx),
                        ),
                    );
            }
        } else {
            disks = disks.child(div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "暂无磁盘采样",
                "No filesystem sample",
            )));
        }
        let mut processes = section(t(cx, "进程", "Processes"), IconName::Cpu).child(
            div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "按 CPU 排序 · 生命周期均值",
                "CPU order · lifetime average",
            )),
        );
        processes = processes.child(
            div()
                .flex()
                .gap_1()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(div().w(px(42.)).child("CPU"))
                .child(div().w(px(42.)).child(t(cx, "内存", "MEM")))
                .child(div().flex_1().child(t(cx, "PID / 命令", "PID / Command"))),
        );
        for process in self
            .processes
            .iter()
            .skip(self.process_page * PROCESS_PAGE_SIZE)
            .take(PROCESS_PAGE_SIZE)
        {
            let selected = process.clone();
            let mut row = div()
                .flex()
                .items_center()
                .gap_1()
                .py_1()
                .border_b_1()
                .border_color(rgb(BORDER))
                .text_xs()
                .child(
                    div()
                        .w(px(42.))
                        .flex_shrink_0()
                        .child(format!("{:.1}%", process.cpu_percent)),
                )
                .child(
                    div()
                        .w(px(42.))
                        .flex_shrink_0()
                        .child(format!("{:.1}%", process.memory_percent)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(format!("{} {}", process.pid, process.command)),
                );
            if process.pid > 1 {
                row = row.child(
                    Button::new(("review-process", process.pid as usize))
                        .text_color(rgb(0xb42318))
                        .ghost()
                        .compact()
                        .rounded(px(6.))
                        .label(t(cx, "终止", "Stop"))
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            if panel.busy || panel.pending.is_some() {
                                return;
                            }
                            panel.paused = true;
                            panel.run(Job::Inspect(selected.clone()), cx);
                        })),
                );
            }
            processes = processes.child(row);
        }
        if self.processes.is_empty() {
            processes = processes.child(div().text_xs().text_color(rgb(MUTED)).child(t(
                cx,
                "暂无进程采样",
                "No process sample",
            )));
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
                            .rounded(px(6.))
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
                            .rounded(px(6.))
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
        let content = div()
            .id("monitor-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .child(resources)
            .child(processes)
            .child(networks)
            .child(sockets)
            .child(disks);
        let mut confirmation = None;
        if let Some(identity) = &self.pending {
            let process = identity.process();
            confirmation = Some(div().id("process-confirmation").max_h(px(250.)).overflow_y_scroll().m_2().p_2().bg(rgb(0xfff8eb)).rounded(px(6.)).border_1().border_color(rgb(0xf2d19b)).flex().flex_col().gap_2()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(t(cx, "确认终止进程", "Confirm termination")))
                .child(div().text_xs().child(self.host.clone()))
                .child(div().text_xs().child(format!("PID {} · {}", process.pid, process.user)))
                .child(div().text_xs().child(process.command.clone()))
                .child(div().text_xs().child(t(cx, "将发送 SIGTERM，可能中断任务。不会强制终止或提权；启动时间复核仍有极小的 PID 重用竞争。", "SIGTERM may interrupt work. No force kill or privilege escalation; a narrow PID reuse race remains after the start-time check.")))
                .child(div().flex().gap_1()
                    .child(Button::new("confirm-process-term").icon(IconName::Square).primary().compact().rounded(px(6.)).label(t(cx, "发送 SIGTERM", "Send SIGTERM"))
                        .on_click(cx.listener(|panel, _, _, cx| { if !panel.busy && let Some(identity) = panel.pending.take() { panel.run(Job::Terminate(identity), cx); } })))
                    .child(Button::new("cancel-process-term").ghost().compact().rounded(px(6.)).label(t(cx, "取消", "Cancel"))
                        .on_click(cx.listener(|panel, _, _, cx| { panel.pending = None; panel.status = Message::new("已取消终止，自动刷新仍暂停。", "Termination cancelled. Polling remains paused."); cx.notify(); })))));
        }
        div()
            .w(px(260.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(SURFACE))
            .text_color(rgb(TEXT))
            .child(controls)
            .children(confirmation)
            .child(content)
            .child(
                div()
                    .id("monitor-operation-status")
                    .max_h(px(110.))
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .bg(rgb(CANVAS))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(self.status.render(cx)),
            )
    }
}
