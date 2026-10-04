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
use keelshell_session::SshSession;
use keelshell_session::forwarding::{DynamicForward, LocalForward, RemoteForward};
use tokio::runtime::Runtime;

use crate::i18n::{Message, t};
use crate::terminal::spawn_transport_worker;

#[derive(Clone, Copy, PartialEq)]
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
    Listening(String),
    Finished(Result<(), String>),
}

struct TunnelRow {
    id: usize,
    description: Message,
    status: Message,
    cancel: Arc<AtomicBool>,
    incoming: Receiver<TunnelEvent>,
    finished: bool,
    listening: bool,
    failed: bool,
    proxy_address: Option<String>,
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
    pub fn new(
        session: SshSession,
        host: String,
        runtime: Arc<Runtime>,
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
        panel.refresh_locale(window, cx);
        panel
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
        let Some(session) = self.session.clone().filter(|_| !self.suspended) else {
            return;
        };
        let spec = match self.specification(cx) {
            Ok(spec) => spec,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        let endpoints = format!(
            "{}:{} → {}:{}",
            spec.bind_host, spec.bind_port, spec.target_host, spec.target_port
        );
        let description = if spec.direction == Direction::Dynamic {
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
                let disconnected = if events.try_send(TunnelEvent::Listening(bound)).is_ok() {
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
                    description,
                    status: Message::new("正在启动…", "Starting…"),
                    cancel,
                    incoming,
                    finished: false,
                    listening: false,
                    failed: false,
                    proxy_address: None,
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
                    TunnelEvent::Listening(address) => {
                        row.listening = true;
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

impl Render for TunnelsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let mut rows = div()
            .id("tunnels-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, row) in self.rows.iter().enumerate() {
            let id = row.id;
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
                .child(
                    div()
                        .text_color(rgb(visual.muted))
                        .child(IconName::ArrowLeftRight),
                )
                .child(div().flex_1().min_w_0().child(row.description.render(cx)))
                .child(
                    div()
                        .w(px(280.))
                        .flex_shrink_0()
                        .text_color(rgb(if row.failed {
                            visual.danger
                        } else {
                            visual.muted
                        }))
                        .child(row.status.render(cx)),
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
                            .tooltip(t(cx, "复制代理地址", "Copy proxy address"))
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
                .child(div().w(px(280.)).child(t(cx,"状态","Status")))
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
    use super::tunnel_error;

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
