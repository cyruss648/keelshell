//! TCP tunnel owners with explicit listener addresses and verified stop outcomes.

use gpui_kit::assets::IconName;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable, Selectable, Sizable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use keelshell_core::{
    AppState, TunnelRule, TunnelRuleAction, TunnelRuleKind, ValidatedTunnelRulePlan,
};
use keelshell_session::SshSession;
use keelshell_session::forwarding::{
    DynamicForward, LocalForward, RemoteForward, TrafficSnapshot, TrafficStats,
};
use tokio::runtime::Runtime;

use crate::i18n::{LocalizedTooltipExt, Message, t};
use crate::terminal::spawn_transport_worker;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Direction {
    Local,
    Remote,
    Dynamic,
}

#[derive(Clone)]
struct Specification {
    direction: Direction,
    bind_host: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
}

enum Forward {
    Local(LocalForward),
    Remote(RemoteForward),
    Dynamic(DynamicForward),
}

impl Forward {
    fn is_closed(&self) -> bool {
        matches!(self, Self::Dynamic(owner) if owner.is_closed())
    }

    fn traffic(&self) -> TrafficStats {
        match self {
            Self::Local(owner) => owner.traffic(),
            Self::Remote(owner) => owner.traffic(),
            Self::Dynamic(owner) => owner.traffic(),
        }
    }

    async fn close(self) -> Result<(), String> {
        match self {
            Self::Local(owner) => {
                owner.close().await;
                Ok(())
            }
            Self::Remote(owner) => owner.close().await.map_err(|error| error.to_string()),
            Self::Dynamic(owner) => owner.close().await.map_err(|error| error.to_string()),
        }
    }
}

enum TunnelEvent {
    Listening {
        address: String,
        traffic: TrafficStats,
    },
    Finished(Result<(), String>),
}

struct TunnelRow {
    id: usize,
    rule_id: Option<uuid::Uuid>,
    description: Message,
    status: Message,
    cancel: Arc<AtomicBool>,
    incoming: Receiver<TunnelEvent>,
    finished: bool,
    listening: bool,
    failed: bool,
    proxy_address: Option<String>,
    traffic: TrafficStats,
    last_traffic: TrafficSnapshot,
}

impl Drop for TunnelRow {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

pub struct TunnelsPanel {
    session: Option<SshSession>,
    suspended: bool,
    host: String,
    runtime: Arc<Runtime>,
    rule_state: Option<AppState>,
    connection_id: Option<uuid::Uuid>,
    selected_rule: Option<uuid::Uuid>,
    rule_plan: Option<ValidatedTunnelRulePlan>,
    rule_status: Message,
    direction: Direction,
    bind_host: Entity<InputState>,
    bind_port: Entity<InputState>,
    target_host: Entity<InputState>,
    target_port: Entity<InputState>,
    rows: Vec<TunnelRow>,
    next_id: usize,
    status: Message,
    _poll: Task<()>,
}

fn field(value: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut input = InputState::new(window, cx);
        input.set_value(value.to_owned(), window, cx);
        input
    })
}

impl TunnelsPanel {
    /// Construct the tunnel panel with the immutable application snapshot used
    /// for named-rule review. A panel without a saved connection binding keeps
    /// the existing free-form endpoint controls, while a bound panel can also
    /// execute reviewed rules for that exact connection.
    pub fn new_with_state(
        session: SshSession,
        host: String,
        runtime: Arc<Runtime>,
        rule_state: Option<AppState>,
        connection_id: Option<uuid::Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let executor = cx.background_executor().clone();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(30)).await;
                if this.update(cx, |view, cx| view.poll(cx)).is_err() {
                    break;
                }
            }
        });
        let mut panel = Self {
            session: Some(session),
            suspended: false,
            host,
            runtime,
            rule_state,
            connection_id,
            selected_rule: None,
            rule_plan: None,
            rule_status: Message::new(
                "没有可用于此连接的命名隧道规则；也可以使用下方临时端点。",
                "No named tunnel rules are available for this connection; use the temporary endpoints below.",
            ),
            direction: Direction::Local,
            bind_host: field("127.0.0.1", window, cx),
            bind_port: field("0", window, cx),
            target_host: field("127.0.0.1", window, cx),
            target_port: field("22", window, cx),
            rows: Vec::new(),
            next_id: 0,
            status: Message::new(
                "配置端点后启动 TCP 隧道；监听端口 0 表示自动分配。",
                "Choose endpoints, then start a TCP tunnel. Port 0 allocates a listener port.",
            ),
            _poll: poll,
        };
        panel.rule_status = panel.rule_availability_message();
        panel.refresh_locale(window, cx);
        panel
    }

    /// Replace the snapshot used for named-rule review.
    ///
    /// StateStore saves rotate the snapshot revision. Rebinding a panel to a
    /// newer state therefore also retires any old selection and review plan;
    /// the user must select and review the rule again before starting it.
    pub fn refresh_rule_state(
        &mut self,
        state: Option<AppState>,
        connection_id: Option<uuid::Uuid>,
        cx: &mut Context<Self>,
    ) {
        let changed = self.connection_id != connection_id
            || self.rule_state.as_ref().map(|current| current.snapshot)
                != state.as_ref().map(|current| current.snapshot)
            || self.rule_state.as_ref() != state.as_ref();
        if !changed {
            return;
        }
        self.rule_state = state;
        self.connection_id = connection_id;
        self.selected_rule = None;
        self.rule_plan = None;
        self.rule_status = self.rule_availability_message();
        cx.notify();
    }

    fn matching_rules(&self) -> Vec<TunnelRule> {
        let Some(state) = &self.rule_state else {
            return Vec::new();
        };
        let Some(connection_id) = self.connection_id else {
            return Vec::new();
        };
        state
            .tunnel_rules
            .iter()
            .filter(|rule| rule.connection_id == connection_id)
            .cloned()
            .collect()
    }

    fn rule_availability_message(&self) -> Message {
        if self.rule_state.is_none() || self.connection_id.is_none() {
            return Message::new(
                "此会话没有已保存连接绑定；可使用下方临时端点。",
                "This session has no saved-connection binding; use the temporary endpoints below.",
            );
        }
        if self.matching_rules().is_empty() {
            Message::new(
                "此连接没有已保存的命名隧道规则；可使用下方临时端点。",
                "This connection has no saved named tunnel rules; use the temporary endpoints below.",
            )
        } else {
            Message::new(
                "请选择规则并检查端点预览，然后审核并启动。",
                "Select a rule, inspect its endpoint preview, then review and start it.",
            )
        }
    }

    fn select_named_rule(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        self.selected_rule = Some(id);
        self.rule_plan = None;
        let Some(state) = self.rule_state.as_ref() else {
            self.rule_status = self.rule_availability_message();
            cx.notify();
            return;
        };
        let Some(connection_id) = self.connection_id else {
            self.rule_status = self.rule_availability_message();
            cx.notify();
            return;
        };
        if !state
            .tunnel_rules
            .iter()
            .any(|rule| rule.id == id && rule.connection_id == connection_id)
        {
            self.rule_status = Message::new(
                "规则不属于当前 SSH 连接，已拒绝审核。",
                "This rule is not owned by the current SSH connection; review was rejected.",
            );
            cx.notify();
            return;
        }
        match state.plan_tunnel_rule_action(id, TunnelRuleAction::Start) {
            Ok(plan) => {
                self.rule_status = rule_preview_message(plan.rule());
                self.rule_plan = Some(plan);
            }
            Err(error) => {
                self.rule_status =
                    Message::detail("命名规则不能启动", "Named rule cannot start", error);
            }
        }
        cx.notify();
    }

    fn start_named_rule(&mut self, cx: &mut Context<Self>) {
        if self.rows.iter().filter(|row| !row.finished).count() >= 32 {
            self.rule_status = Message::new(
                "最多同时运行 32 条隧道，请先停止现有隧道。",
                "Stop an existing tunnel before opening more than 32.",
            );
            cx.notify();
            return;
        }
        let Some(plan) = self.rule_plan.take() else {
            self.rule_status = Message::new(
                "请先选择并审核一条命名规则。",
                "Select and review a named rule first.",
            );
            cx.notify();
            return;
        };
        let Some(state) = self.rule_state.clone() else {
            self.rule_status = Message::new(
                "命名规则状态已失效，请重新加载配置后重试。",
                "Named-rule state is unavailable; reload configuration and try again.",
            );
            cx.notify();
            return;
        };
        let token = plan.review_token();
        let confirmed = match plan.confirm(&state, token) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.selected_rule = None;
                self.rule_status =
                    Message::detail("命名规则审核已过期", "Named-rule review expired", error);
                cx.notify();
                return;
            }
        };
        let rule = confirmed.plan().rule().clone();
        let specification = match specification_from_rule(&rule) {
            Ok(specification) => specification,
            Err(error) => {
                self.rule_status = error;
                cx.notify();
                return;
            }
        };
        self.rule_status = Message::new(
            format!("已审核规则“{}”，正在启动…", rule.name),
            format!("Reviewed rule “{}”; starting…", rule.name),
        );
        self.start_specification(specification, Some(rule.name), Some(rule.id), cx);
    }

    /// Retire listeners without transferring their remote authority to a new session.
    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        self.suspended = true;
        self.session = None;
        for row in &mut self.rows {
            if !row.finished {
                row.cancel.store(true, Ordering::Release);
                row.status = Message::new(
                    "正在停止，等待监听器清理…",
                    "Stopping; waiting for listener cleanup…",
                );
            }
        }
        self.status = tunnel_summary(&self.rows);
        cx.notify();
    }

    /// Translate hints in place without replacing input entities or active tunnels.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (input, zh, en) in [
            (&self.bind_host, "监听地址", "Bind host"),
            (&self.bind_port, "监听端口", "Bind port"),
            (&self.target_host, "目标地址", "Destination host"),
            (&self.target_port, "目标端口", "Destination port"),
        ] {
            let placeholder = t(cx, zh, en);
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        cx.notify();
    }
    fn specification(&self, cx: &App) -> Result<Specification, Message> {
        let bind_host = self.bind_host.read(cx).value().trim().to_owned();
        let target_host = self.target_host.read(cx).value().trim().to_owned();
        let bind_port = self
            .bind_port
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .map_err(|_| Message::new("监听端口必须为 0–65535", "Bind port must be 0–65535"))?;
        let target_port = if self.direction == Direction::Dynamic {
            dynamic_bind(&bind_host, bind_port)?;
            0
        } else {
            self.target_port
                .read(cx)
                .value()
                .trim()
                .parse::<u16>()
                .map_err(|_| {
                    Message::new("目标端口必须为 1–65535", "Destination port must be 1–65535")
                })?
        };
        if self.direction != Direction::Dynamic
            && (bind_host.is_empty() || target_host.is_empty() || target_port == 0)
        {
            return Err(Message::new(
                "请输入监听地址、目标地址和非零目标端口",
                "Both hosts and a nonzero destination port are required",
            ));
        }
        if self.rows.iter().filter(|row| !row.finished).count() >= 32 {
            return Err(Message::new(
                "最多同时运行 32 条隧道，请先停止现有隧道",
                "Stop an existing tunnel before opening more than 32",
            ));
        }
        Ok(Specification {
            direction: self.direction,
            bind_host,
            bind_port,
            target_host,
            target_port,
        })
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let spec = match self.specification(cx) {
            Ok(spec) => spec,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        self.start_specification(spec, None, None, cx);
    }

    fn start_specification(
        &mut self,
        spec: Specification,
        rule_name: Option<String>,
        rule_id: Option<uuid::Uuid>,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone().filter(|_| !self.suspended) else {
            return;
        };
        let endpoints = format!(
            "{}:{} → {}:{}",
            spec.bind_host, spec.bind_port, spec.target_host, spec.target_port
        );
        let description = if let Some(name) = rule_name {
            Message::new(
                format!("规则 {name} · {endpoints}"),
                format!("Rule {name} · {endpoints}"),
            )
        } else if spec.direction == Direction::Dynamic {
            Message::new(
                "动态 SOCKS5 · 目标与域名由 SSH 主机连接",
                "Dynamic SOCKS5 · destinations and DNS via SSH host",
            )
        } else if spec.direction == Direction::Local {
            Message::new(
                format!("本地 → SSH 主机 · {endpoints}"),
                format!("Local → SSH host · {endpoints}"),
            )
        } else {
            Message::new(
                format!("SSH 主机 → 本机 · {endpoints}"),
                format!("SSH host → this device · {endpoints}"),
            )
        };
        let (events, incoming) = mpsc::sync_channel(4);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let runtime = self.runtime.clone();
        let started = spawn_transport_worker("keelshell-tunnel", cancel.clone(), move || {
            runtime.block_on(async move {
                let started = tokio::select! {
                    result = start_forward(&session, &spec) => result,
                    _ = cancellation(&worker_cancel) => {
                        let result = Err(if spec.direction == Direction::Remote {
                            "Tunnel start cancelled before confirmation; remote allocation may require connection cleanup"
                        } else { "Tunnel start cancelled" }.to_owned());
                        let _ = events.try_send(TunnelEvent::Finished(result.clone()));
                        return result;
                    },
                };
                let (owner, bound) = match started {
                    Ok(started) => started,
                    Err(error) => {
                        let _ = events.try_send(TunnelEvent::Finished(Err(error.clone())));
                        return Err(error);
                    },
                };
                let traffic = owner.traffic();
                let disconnected = if events
                    .try_send(TunnelEvent::Listening {
                        address: bound,
                        traffic: traffic.clone(),
                    })
                    .is_ok()
                {
                    tokio::select! {
                        _ = cancellation(&worker_cancel) => false,
                        disconnected = connection_closed(&session, &owner) => disconnected,
                    }
                } else { false };
                // The guard lives until native cancellation acknowledgement;
                // the registry joins this owner during application shutdown.
                let result = owner.close().await.and_then(|()| if disconnected {
                    Err("SSH connection closed; tunnel listener has been released".to_owned())
                } else { Ok(()) });
                let _ = events.try_send(TunnelEvent::Finished(result.clone()));
                result
            })
        });
        match started {
            Ok(()) => {
                self.rows.push(TunnelRow {
                    id: self.next_id,
                    rule_id,
                    description,
                    status: Message::new("正在启动…", "Starting…"),
                    cancel,
                    incoming,
                    finished: false,
                    listening: false,
                    failed: false,
                    proxy_address: None,
                    traffic: TrafficStats::default(),
                    last_traffic: TrafficSnapshot::default(),
                });
                self.next_id += 1;
                self.status = tunnel_summary(&self.rows);
            }
            Err(error) => {
                self.status = Message::detail(
                    "无法启动隧道工作线程",
                    "Unable to start tunnel worker",
                    error,
                )
            }
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for row in &mut self.rows {
            while let Ok(event) = row.incoming.try_recv() {
                changed = true;
                match event {
                    TunnelEvent::Listening { address, traffic } => {
                        row.listening = true;
                        row.traffic = traffic;
                        if address.starts_with("socks5h://") {
                            row.proxy_address = Some(address.clone());
                        }
                        row.status = if row.cancel.load(Ordering::Acquire) {
                            Message::new(
                                format!("正在停止 {address} 的监听…"),
                                format!("Stopping listener at {address}…"),
                            )
                        } else {
                            Message::new(
                                format!("正在监听 {address}"),
                                format!("Listening at {address}"),
                            )
                        }
                    }
                    TunnelEvent::Finished(result) => {
                        row.finished = true;
                        row.failed = result.is_err();
                        row.status = match result {
                            Ok(()) => Message::new(
                                "已停止；监听器和所属连接流已关闭",
                                "Stopped; listener and owned streams closed",
                            ),
                            Err(error) => tunnel_error(&error),
                        };
                    }
                }
            }
            let snapshot = row.traffic.snapshot();
            if snapshot != row.last_traffic {
                row.last_traffic = snapshot;
                changed = true;
            }
        }
        if changed {
            self.status = tunnel_summary(&self.rows);
            cx.notify();
        }
    }
}

fn tunnel_summary(rows: &[TunnelRow]) -> Message {
    let (mut listening, mut starting, mut stopping, mut stopped, mut failed) = (0, 0, 0, 0, 0);
    for row in rows {
        if row.finished {
            if row.failed {
                failed += 1;
            } else {
                stopped += 1;
            }
        } else if row.cancel.load(Ordering::Acquire) {
            stopping += 1;
        } else if row.listening {
            listening += 1;
        } else {
            starting += 1;
        }
    }
    if listening + starting + stopping == 0 {
        return if failed == 0 {
            Message::new(
                format!("全部 {stopped} 条隧道已停止；监听器和所属连接流已关闭"),
                format!("All {stopped} tunnels stopped; listeners and owned streams closed"),
            )
        } else {
            Message::new(
                format!("无活动隧道；已停止 {stopped} 条，失败 {failed} 条，详情见列表"),
                format!(
                    "No active tunnels; {stopped} stopped, {failed} failed. See rows for details"
                ),
            )
        };
    }
    let mut zh = Vec::new();
    let mut en = Vec::new();
    for (count, zh_label, en_label) in [
        (listening, "正在监听", "Active listeners"),
        (starting, "正在启动", "Starting"),
        (stopping, "正在停止", "Stopping"),
        (failed, "失败", "Failed"),
    ] {
        if count > 0 {
            zh.push(format!("{zh_label} {count} 条"));
            en.push(format!("{en_label}: {count}"));
        }
    }
    Message::new(zh.join(" · "), en.join(" · "))
}

fn rule_preview_message(rule: &TunnelRule) -> Message {
    let kind = match rule.kind {
        TunnelRuleKind::Local => ("本地转发", "Local forwarding"),
        TunnelRuleKind::Remote => ("远程转发", "Remote forwarding"),
        TunnelRuleKind::Dynamic => ("动态 SOCKS5", "Dynamic SOCKS5"),
    };
    let endpoint = match (rule.target_host.as_deref(), rule.target_port) {
        (Some(host), Some(port)) => {
            format!("{}:{} → {}:{}", rule.bind_host, rule.bind_port, host, port)
        }
        _ => format!("{}:{}", rule.bind_host, rule.bind_port),
    };
    Message::new(
        format!("规则“{}” · {} · 预览：{endpoint}", rule.name, kind.0),
        format!("Rule “{}” · {} · Preview: {endpoint}", rule.name, kind.1),
    )
}

fn specification_from_rule(rule: &TunnelRule) -> Result<Specification, Message> {
    let (direction, target_host, target_port) = match rule.kind {
        TunnelRuleKind::Local => (
            Direction::Local,
            rule.target_host.clone().unwrap_or_default(),
            rule.target_port.unwrap_or_default(),
        ),
        TunnelRuleKind::Remote => (
            Direction::Remote,
            rule.target_host.clone().unwrap_or_default(),
            rule.target_port.unwrap_or_default(),
        ),
        TunnelRuleKind::Dynamic => (Direction::Dynamic, String::new(), 0),
    };
    if direction == Direction::Dynamic {
        dynamic_bind(&rule.bind_host, rule.bind_port)?;
    } else if target_host.is_empty() || target_port == 0 {
        return Err(Message::new(
            "命名规则目标端点无效，已阻止启动。",
            "The named rule has an invalid destination endpoint; start was blocked.",
        ));
    }
    Ok(Specification {
        direction,
        bind_host: rule.bind_host.clone(),
        bind_port: rule.bind_port,
        target_host,
        target_port,
    })
}

async fn connection_closed(session: &SshSession, owner: &Forward) -> bool {
    while !session.is_closed() && !owner.is_closed() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    session.is_closed()
}

fn tunnel_error(error: &str) -> Message {
    match error {
        "Tunnel start cancelled before confirmation; remote allocation may require connection cleanup" => {
            Message::new("启动已取消；远端端口分配可能需要关闭连接后清理", error)
        }
        "SSH connection closed; tunnel listener has been released" => {
            Message::new("SSH 连接已关闭，隧道监听端口已释放", error)
        }
        "Tunnel start cancelled" => Message::new("隧道启动已取消", error),
        "operation timed out: SOCKS5 SSH channel cleanup; shared SSH disconnected" => Message::new(
            "SOCKS5 通道清理超时；为清理未确认通道，已断开共享 SSH 连接",
            "SOCKS5 channel cleanup timed out; shared SSH disconnected to clean up the unconfirmed channel",
        ),
        "operation timed out: SOCKS5 SSH channel cleanup; shared SSH disconnect not confirmed" => {
            Message::new(
                "SOCKS5 通道清理超时；已请求断开共享 SSH，但尚未确认关闭，请关闭该 SSH 标签",
                "SOCKS5 channel cleanup timed out; shared SSH disconnect was requested but not confirmed. Close this SSH tab",
            )
        }
        "Address lookup timed out" => Message::new("地址解析超时", error),
        "Address lookup returned no endpoints" => Message::new("地址解析未返回可用端点", error),
        _ => Message::detail("隧道失败", "Tunnel failed", error),
    }
}

fn traffic_message(snapshot: TrafficSnapshot) -> Message {
    Message::new(
        format!(
            "↑ 发送 {} · ↓ 接收 {}",
            byte_size(snapshot.sent),
            byte_size(snapshot.received)
        ),
        format!(
            "↑ Sent {} · ↓ Received {}",
            byte_size(snapshot.sent),
            byte_size(snapshot.received)
        ),
    )
}

fn byte_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

impl Render for TunnelsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let matching_rules = self.matching_rules();
        let selected_rule = self.selected_rule;
        let mut named_rules = div().flex().flex_wrap().gap_1();
        for rule in matching_rules {
            let rule_id = rule.id;
            let label = if rule.enabled {
                rule.name.clone()
            } else {
                format!("{} · {}", rule.name, t(cx, "未启用", "Disabled"))
            };
            named_rules = named_rules.child(
                Button::new(format!("named-tunnel-rule-{rule_id}"))
                    .ghost()
                    .compact()
                    .rounded(px(6.))
                    .selected(selected_rule == Some(rule_id))
                    .disabled(self.suspended)
                    .label(label)
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.select_named_rule(rule_id, cx)),
                    ),
            );
        }
        let mut named_rule_section = div()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .text_color(rgb(visual.muted))
                    .child(IconName::Bookmark),
            )
            .child(
                div()
                    .w(px(120.))
                    .flex_shrink_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(cx, "命名规则", "Named rules")),
            )
            .child(named_rules)
            .child(div().flex_1());
        if self.rule_plan.is_some() {
            named_rule_section = named_rule_section.child(
                Button::new("review-start-named-tunnel")
                    .primary()
                    .compact()
                    .rounded(px(6.))
                    .disabled(self.suspended)
                    .label(t(cx, "审核并启动", "Review and start"))
                    .on_click(cx.listener(|view, _, _, cx| view.start_named_rule(cx))),
            );
        }
        named_rule_section = named_rule_section.child(
            div()
                .max_w(px(460.))
                .text_color(rgb(visual.muted))
                .child(self.rule_status.render(cx)),
        );
        let mut rows = div()
            .id("tunnels-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, row) in self.rows.iter().enumerate() {
            let id = row.id;
            let row_icon = if row.rule_id.is_some() {
                IconName::Bookmark
            } else {
                IconName::ArrowLeftRight
            };
            let mut item = div()
                .min_h(px(38.))
                .px_2()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .bg(rgb(if index % 2 == 0 {
                    visual.surface
                } else {
                    visual.canvas
                }))
                .border_b_1()
                .border_color(rgb(visual.border))
                .child(div().text_color(rgb(visual.muted)).child(row_icon))
                .child(div().flex_1().min_w_0().child(row.description.render(cx)))
                .child(
                    div()
                        .w(px(250.))
                        .flex_shrink_0()
                        .text_color(rgb(if row.failed {
                            visual.danger
                        } else {
                            visual.muted
                        }))
                        .child(row.status.render(cx)),
                )
                .child(
                    div()
                        .w(px(170.))
                        .flex_shrink_0()
                        .text_color(rgb(visual.muted))
                        .child(traffic_message(row.last_traffic).render(cx)),
                );
            if !row.finished {
                if let Some(address) = row.proxy_address.clone() {
                    item = item.child(
                        Button::new(("copy-proxy", id))
                            .disabled(self.suspended || row.cancel.load(Ordering::Acquire))
                            .icon(IconName::Copy)
                            .ghost()
                            .compact()
                            .rounded(px(6.))
                            .localized_tooltip("复制代理地址", "Copy proxy address")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(address.clone()))
                            }),
                    );
                }
                let stopping = row.cancel.load(Ordering::Acquire);
                item = item.child(
                    Button::new(("stop-tunnel", id))
                        .icon(IconName::Square)
                        .ghost()
                        .compact()
                        .rounded(px(6.))
                        .disabled(stopping)
                        .label(if stopping {
                            t(cx, "正在停止…", "Stopping…")
                        } else {
                            t(cx, "停止", "Stop")
                        })
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(row) = view.rows.iter_mut().find(|row| row.id == id) {
                                row.cancel.store(true, Ordering::Release);
                                row.status = Message::new(
                                    "正在停止，等待监听器清理…",
                                    "Stopping; waiting for listener cleanup…",
                                );
                            }
                            view.status = tunnel_summary(&view.rows);
                            cx.notify();
                        })),
                );
            } else {
                item = item.child(div().w(px(45.)));
            }
            rows = rows.child(item);
        }
        if self.rows.is_empty() {
            rows = rows.child(
                div()
                    .px_3()
                    .py_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(visual.muted))
                    .child(IconName::Network)
                    .child(t(
                        cx,
                        "此 SSH 连接尚未创建隧道，请设置端点后启动。",
                        "No tunnels yet. Configure the endpoints above to start.",
                    )),
            );
        }
        let endpoint = format!(
            "{}:{}",
            self.bind_host.read(cx).value(),
            self.bind_port.read(cx).value()
        );
        let exposure = if self.direction == Direction::Dynamic {
            Message::new(
                format!("本机代理：{endpoint}；启动后下方显示实际 socks5h:// 地址"),
                format!(
                    "Local proxy: {endpoint}; the actual socks5h:// address appears below after startup"
                ),
            )
        } else if self.direction == Direction::Local {
            Message::new(
                format!("监听端点：{endpoint}（本机）"),
                format!("Listener: {endpoint} (this device)"),
            )
        } else {
            Message::new(
                format!("监听端点：{endpoint}（SSH 主机）；外部可达性由服务器策略决定"),
                format!(
                    "Listener: {endpoint} (SSH host); server policy controls external reachability"
                ),
            )
        };
        div().h_full().min_h_0().flex().flex_col().bg(rgb(visual.surface)).text_color(rgb(visual.text)).text_xs()
            .child(div().h(px(38.)).px_3().flex_shrink_0().flex().items_center().gap_3().bg(rgb(visual.canvas)).border_b_1().border_color(rgb(visual.border))
                .child(div().text_color(rgb(visual.accent)).child(IconName::Network))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,"端口转发","Port forwarding")))
                .child(div().flex_1().text_color(rgb(visual.muted)).child(self.host.clone()))
                .child(Button::new("tunnel-local").ghost().compact().rounded(px(6.)).icon(IconName::ArrowUp).selected(self.direction==Direction::Local).label(t(cx,"本地转发","Local forwarding")).on_click(cx.listener(|view,_,_,cx| {view.direction=Direction::Local;cx.notify();})))
                .child(Button::new("tunnel-remote").ghost().compact().rounded(px(6.)).icon(IconName::ArrowDown).selected(self.direction==Direction::Remote).label(t(cx,"远程转发","Remote forwarding")).on_click(cx.listener(|view,_,_,cx| {view.direction=Direction::Remote;cx.notify();})))
                .child(Button::new("tunnel-dynamic").ghost().compact().rounded(px(6.)).icon(IconName::Network).selected(self.direction==Direction::Dynamic).label(t(cx,"动态 SOCKS5","Dynamic SOCKS5")).on_click(cx.listener(|view,_,window,cx| {view.direction=Direction::Dynamic;view.bind_host.read(cx).focus_handle(cx).focus(window,cx);cx.notify();}))))
            .child(named_rule_section)
            .child(div().px_3().py_3().flex_shrink_0().flex().items_end().gap_2().border_b_1().border_color(rgb(visual.border))
                .child(div().flex_1().flex().flex_col().gap_1().child(t(cx,"监听地址","Bind host")).child(Input::new(&self.bind_host).small().rounded(px(6.))))
                .child(div().w(px(88.)).flex().flex_col().gap_1().child(t(cx,"监听端口","Bind port")).child(Input::new(&self.bind_port).small().rounded(px(6.))))
                .when(self.direction != Direction::Dynamic, |row| row
                .child(div().pb_2().text_color(rgb(visual.muted)).child(IconName::ArrowRight))
                .child(div().flex_1().flex().flex_col().gap_1().child(t(cx,"目标地址","Destination host")).child(Input::new(&self.target_host).small().rounded(px(6.))))
                .child(div().w(px(88.)).flex().flex_col().gap_1().child(t(cx,"目标端口","Destination port")).child(Input::new(&self.target_port).small().rounded(px(6.)))))
                .when(self.direction == Direction::Dynamic, |row| row.child(div().flex_1().pb_2().text_color(rgb(visual.muted)).child(t(cx,"在客户端设置 SOCKS5 代理，并启用代理端 DNS。", "Set a SOCKS5 proxy in your client and enable proxy-side DNS."))))
                .child(Button::new("start-tunnel").disabled(self.suspended).icon(IconName::Play).primary().compact().rounded(px(6.)).label(t(cx,"启动隧道","Start tunnel")).on_click(cx.listener(|view,_,_,cx|view.start(cx)))))
            .child(div().px_3().py_1().flex_shrink_0().flex().items_center().gap_2().text_color(rgb(visual.muted)).bg(rgb(visual.canvas)).child(IconName::Info).child(exposure.render(cx)))
            .child(div().px_3().py_1().flex_shrink_0().text_color(rgb(visual.muted)).child(if self.direction == Direction::Dynamic { t(cx,"仅本机回环，无代理认证；支持 TCP CONNECT，不支持 UDP/BIND。", "Loopback only, no proxy authentication; TCP CONNECT supported, UDP/BIND unsupported.") } else { t(cx,"默认仅监听回环地址；非回环地址可能允许其他设备访问。", "Loopback is the default. A non-loopback address may allow access from other devices.") }))
            .child(div().h(px(28.)).px_3().flex_shrink_0().flex().items_center().bg(rgb(visual.canvas)).border_y_1().border_color(rgb(visual.border))
                .child(div().flex_1().child(t(cx,"转发方向 / 端点","Direction / endpoints")))
                .child(div().w(px(250.)).child(t(cx,"状态","Status")))
                .child(div().w(px(170.)).child(t(cx,"流量","Traffic")))
                .child(div().w(px(45.)).child(t(cx,"操作","Action"))))
            .when(self.suspended, |panel| panel.child(div().px_3().py_1().flex_shrink_0().text_color(rgb(visual.muted)).child(t(cx, "上一会话隧道记录 · 不会自动重建", "Previous session tunnels · Never restarted automatically"))))
            .child(rows)
            .child(div().min_h(px(28.)).px_3().flex_shrink_0().flex().items_center().bg(rgb(visual.canvas)).border_t_1().border_color(rgb(visual.border)).text_color(rgb(visual.muted)).child(self.status.render(cx)))
    }
}

async fn start_forward(
    session: &SshSession,
    spec: &Specification,
) -> Result<(Forward, String), String> {
    match spec.direction {
        Direction::Dynamic => {
            let bind = dynamic_bind(&spec.bind_host, spec.bind_port)
                .map_err(|_| "SOCKS5 bind host must be a loopback IP address".to_owned())?;
            let owner = session
                .forward_dynamic(bind)
                .await
                .map_err(|error| error.to_string())?;
            let bound = format!("socks5h://{}", owner.local_addr());
            Ok((Forward::Dynamic(owner), bound))
        }
        Direction::Local => {
            let bind = resolve(&spec.bind_host, spec.bind_port).await?;
            let owner = session
                .forward_local(bind, spec.target_host.clone(), spec.target_port)
                .await
                .map_err(|error| error.to_string())?;
            let bound = owner.local_addr().to_string();
            Ok((Forward::Local(owner), bound))
        }
        Direction::Remote => {
            let target = resolve(&spec.target_host, spec.target_port).await?;
            let owner = session
                .forward_remote(spec.bind_host.clone(), spec.bind_port, target)
                .await
                .map_err(|error| error.to_string())?;
            let bound = format!("{}:{}", spec.bind_host, owner.remote_port());
            Ok((Forward::Remote(owner), bound))
        }
    }
}

fn dynamic_bind(host: &str, port: u16) -> Result<SocketAddr, Message> {
    match host.parse::<IpAddr>() {
        Ok(ip) if ip.is_loopback() => Ok(SocketAddr::new(ip, port)),
        _ => Err(Message::new(
            "SOCKS5 监听地址必须为回环 IP，如 127.0.0.1 或 ::1",
            "SOCKS5 bind host must be a loopback IP address, such as 127.0.0.1 or ::1",
        )),
    }
}

async fn resolve(host: &str, port: u16) -> Result<SocketAddr, String> {
    tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| "Address lookup timed out".to_owned())?
    .map_err(|error| error.to_string())?
    .next()
    .ok_or_else(|| "Address lookup returned no endpoints".into())
}

async fn cancellation(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, specification_from_rule, tunnel_error};
    use keelshell_core::{TunnelRule, TunnelRuleKind};

    #[test]
    fn named_rule_adapter_preserves_forwarding_mode_and_blocks_missing_targets() {
        let connection_id = uuid::Uuid::new_v4();
        let local = TunnelRule::new(
            "Local database",
            connection_id,
            TunnelRuleKind::Local,
            "127.0.0.1",
            15432,
            Some("db.internal".into()),
            Some(5432),
        )
        .unwrap_or_else(|error| panic!("valid local rule: {error}"));
        let specification = specification_from_rule(&local)
            .unwrap_or_else(|_| panic!("local specification should be valid"));
        assert_eq!(specification.direction, Direction::Local);
        assert_eq!(specification.target_host, "db.internal");
        assert_eq!(specification.target_port, 5432);

        let dynamic = TunnelRule::new(
            "Local proxy",
            connection_id,
            TunnelRuleKind::Dynamic,
            "127.0.0.1",
            0,
            None,
            None,
        )
        .unwrap_or_else(|error| panic!("valid dynamic rule: {error}"));
        assert_eq!(
            specification_from_rule(&dynamic)
                .unwrap_or_else(|_| panic!("dynamic specification should be valid"))
                .direction,
            Direction::Dynamic
        );

        let mut invalid = local;
        invalid.target_host = None;
        assert!(specification_from_rule(&invalid).is_err());
    }

    #[gpui_kit::test]
    fn stopped_and_failed_tunnel_messages_follow_current_language(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        let cancelled = tunnel_error(
            "Tunnel start cancelled before confirmation; remote allocation may require connection cleanup",
        );
        let remote = tunnel_error("bind 127.0.0.1:9000: permission denied");
        let unconfirmed = tunnel_error(
            "operation timed out: SOCKS5 SSH channel cleanup; shared SSH disconnected",
        );
        cx.update(|cx| {
            assert!(cancelled.render(cx).starts_with("启动已取消"));
            assert!(remote.render(cx).starts_with("隧道失败："));
            assert!(unconfirmed.render(cx).contains("已断开共享 SSH 连接"));
            crate::i18n::set_language(keelshell_core::Language::En, cx);
            assert!(cancelled.render(cx).starts_with("Tunnel start cancelled"));
            assert!(remote.render(cx).starts_with("Tunnel failed:"));
            assert!(unconfirmed.render(cx).contains("shared SSH disconnected"));
            assert!(
                remote
                    .render(cx)
                    .contains("bind 127.0.0.1:9000: permission denied")
            );
        });
    }
}

#[cfg(test)]
#[path = "tunnels/tests.rs"]
mod integration_tests;
