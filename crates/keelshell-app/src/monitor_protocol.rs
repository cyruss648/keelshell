//! Explicit, reviewed protocol probes. This child owns one captured SSH session;
//! reconnect/route retirement revokes both dispatch and result adoption.
use super::*;
use gpui_kit::component::{
    Selectable, Sizable,
    input::{Input, InputState},
};
use keelshell_core::{
    NetworkDiagnosticInputError, NetworkDiagnosticKind, NetworkDiagnosticRequest,
};
use keelshell_session::network_diagnostic::{
    NetworkDiagnosticError, NetworkDiagnosticIcmpReason, NetworkDiagnosticReport,
    NetworkDiagnosticStatus,
};
#[cfg(test)]
#[path = "monitor_tests/protocol_diagnostics.rs"]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct DiagnosticDraft {
    kind: NetworkDiagnosticKind,
    endpoint: String,
    port: String,
}

pub(super) struct ProtocolPanel {
    authority: Option<(WeakEntity<crate::workspace::Workspace>, EntityId)>,
    session: Option<SshSession>,
    host: String,
    runtime: Arc<tokio::runtime::Runtime>,
    kind: NetworkDiagnosticKind,
    endpoint: Entity<InputState>,
    port: Entity<InputState>,
    expanded: bool,
    review: Option<NetworkDiagnosticRequest>,
    review_draft: Option<DiagnosticDraft>,
    review_selection: Option<uuid::Uuid>,
    observed_draft: DiagnosticDraft,
    result: Option<NetworkDiagnosticReport>,
    status: Message,
    token: uuid::Uuid,
    cancel: Option<Arc<AtomicBool>>,
    _job: Option<Task<()>>,
    _input_observers: Vec<Subscription>,
}

impl ProtocolPanel {
    pub(super) fn new(
        session: SshSession,
        host: String,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let endpoint = cx.new(|cx| InputState::new(window, cx));
        let port = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value("443", window, cx);
            input
        });
        let input_observers = [&endpoint, &port]
            .into_iter()
            .map(|input| cx.observe(input, |panel, _, cx| panel.sync_input(cx)))
            .collect();
        Self {
            authority: None,
            session: Some(session),
            host,
            runtime,
            kind: NetworkDiagnosticKind::Dns,
            endpoint,
            port,
            expanded: false,
            review: None,
            review_draft: None,
            review_selection: None,
            observed_draft: DiagnosticDraft {
                kind: NetworkDiagnosticKind::Dns,
                endpoint: String::new(),
                port: "443".into(),
            },
            result: None,
            status: Message::empty(),
            token: uuid::Uuid::new_v4(),
            cancel: None,
            _job: None,
            _input_observers: input_observers,
        }
    }

    fn draft(&self, cx: &App) -> DiagnosticDraft {
        DiagnosticDraft {
            kind: self.kind,
            endpoint: self.endpoint.read(cx).value().to_string(),
            port: self.port.read(cx).value().to_string(),
        }
    }

    fn sync_input(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        if self.observed_draft == draft {
            return;
        }
        self.observed_draft = draft;
        self.review = None;
        self.review_draft = None;
        self.review_selection = None;
        self.result = None;
        self.cancel(cx);
        self.token = uuid::Uuid::new_v4();
        self.status = Message::new(
            "输入已变化，旧审核与结果已失效；请重新预览。",
            "Input changed; previous review and results are invalid. Preview again.",
        );
        cx.notify();
    }

    pub(super) fn suspend(&mut self, cx: &mut Context<Self>) {
        if self.session.is_none() {
            return;
        }
        self.session = None;
        self.review = None;
        self.review_draft = None;
        self.review_selection = None;
        self.token = uuid::Uuid::new_v4();
        self.cancel(cx);
        self.status = Message::new(
            "会话或保存路线已变化；请重连后重新审核诊断。历史结果已保留。",
            "Session or saved route changed; reconnect and review diagnostics again. Historical results are retained.",
        );
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
            self.status = Message::new(
                "已取消，结果不会采用；远端探测有 8 秒期限。",
                "Cancelled; results will be discarded. The remote probe has an eight-second deadline.",
            );
            self.token = uuid::Uuid::new_v4();
        }
        cx.notify();
    }

    fn invalidate_selection(&mut self, cx: &mut Context<Self>) {
        self.review = None;
        self.review_draft = None;
        self.review_selection = None;
        self.result = None;
        self.cancel(cx);
        self.token = uuid::Uuid::new_v4();
        self.status = Message::new(
            "活动面板或会话已变化；请选择此会话并重新预览。",
            "Active pane or session changed; select this session and preview again.",
        );
        cx.notify();
    }

    fn selection_token(&self, cx: &App) -> Option<uuid::Uuid> {
        let session = self.session.as_ref()?;
        match &self.authority {
            None => Some(uuid::Uuid::nil()),
            Some((workspace, tab)) => workspace
                .upgrade()?
                .read(cx)
                .protocol_selection_token(*tab, session),
        }
    }

    fn input(&self, cx: &App) -> Result<NetworkDiagnosticRequest, NetworkDiagnosticInputError> {
        let value = self.endpoint.read(cx).value();
        match self.kind {
            NetworkDiagnosticKind::Dns => NetworkDiagnosticRequest::dns(&value),
            NetworkDiagnosticKind::Tls => NetworkDiagnosticRequest::tls(
                &value,
                self.port
                    .read(cx)
                    .value()
                    .parse()
                    .map_err(|_| NetworkDiagnosticInputError::InvalidPort)?,
            ),
            NetworkDiagnosticKind::Http => NetworkDiagnosticRequest::http(&value),
            NetworkDiagnosticKind::Icmp => NetworkDiagnosticRequest::icmp(&value),
        }
    }

    fn preview(&mut self, cx: &mut Context<Self>) {
        self.sync_input(cx);
        if self.session.is_none() || self.cancel.is_some() {
            return;
        }
        if !self.authorized(cx) {
            self.suspend(cx);
            return;
        }
        let Some(selection) = self.selection_token(cx) else {
            self.invalidate_selection(cx);
            return;
        };
        self.review = None;
        self.review_draft = None;
        self.review_selection = None;
        self.status = match self.input(cx) {
            Ok(request) => {
                self.review = Some(request);
                self.review_draft = Some(self.draft(cx));
                self.review_selection = Some(selection);
                Message::new(
                    "核对执行主机、端点与操作后确认。",
                    "Review the execution host, endpoint and operation before confirming.",
                )
            }
            Err(NetworkDiagnosticInputError::CredentialsOrQuery) => Message::new(
                "URL 不接受凭据、查询参数或片段；请使用无敏感参数的地址。",
                "URLs cannot contain credentials, query parameters or fragments; use an endpoint without sensitive parameters.",
            ),
            Err(NetworkDiagnosticInputError::InvalidPort) => {
                Message::new("端口必须为 1–65535。", "Port must be 1–65535.")
            }
            Err(_) => Message::new(
                "请输入有效 DNS 名称、IP 或明确的 HTTP(S) URL；不接受空白、控制字符或区域标识。",
                "Enter a valid DNS name, IP or explicit HTTP(S) URL; whitespace, controls and zone identifiers are not accepted.",
            ),
        };
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        self.sync_input(cx);
        if self.cancel.is_some() {
            return;
        }
        if !self.authorized(cx) {
            self.suspend(cx);
            return;
        }
        let Some(selection) = self.selection_token(cx) else {
            self.invalidate_selection(cx);
            return;
        };
        if self.review_selection.take() != Some(selection) {
            self.invalidate_selection(cx);
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let Some(request) = self.review.take() else {
            return;
        };
        let draft = self.draft(cx);
        if self.review_draft.take().as_ref() != Some(&draft)
            || self.input(cx).as_ref() != Ok(&request)
        {
            self.status = Message::new("输入已变化，请重新预览。", "Input changed; preview again.");
            cx.notify();
            return;
        }
        let runtime = self.runtime.clone();
        let icmp_operation = request.kind() == NetworkDiagnosticKind::Icmp;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let token = uuid::Uuid::new_v4();
        self.token = token;
        self.result = None;
        let (sender, receiver) = mpsc::sync_channel(1);
        let started =
            spawn_transport_worker("keelshell-protocol-diagnostic", cancel.clone(), move || {
                let result = runtime.block_on(session.diagnose_remote(request, &worker_cancel));
                let _ = sender.try_send(result);
                Ok(())
            });
        if started.is_err() {
            self.status = Message::new("无法启动诊断任务。", "Unable to start diagnostic worker.");
            cx.notify();
            return;
        }
        self.cancel = Some(cancel);
        self.status = Message::new(
            "正在由已认证 SSH 主机执行协议诊断…",
            "Running protocol diagnostics from the authenticated SSH host…",
        );
        let executor = cx.background_executor().clone();
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = loop {
                match receiver.try_recv() {
                    Ok(result) => break result,
                    Err(TryRecvError::Empty) => executor.timer(Duration::from_millis(20)).await,
                    Err(TryRecvError::Disconnected) => {
                        break Err(NetworkDiagnosticError::Transport);
                    }
                }
            };
            let _ = this.update(cx, |panel, cx| {
                panel.sync_input(cx);
                panel.cancel = None;
                // SSH cancellation/transport failure cannot confirm remote reap.
                // Retire this diagnostic authority even for a stale token: a
                // discarded result must not re-admit another uncertain process.
                if icmp_operation && result.is_err() {
                    panel.session = None;
                    panel.review = None;
                    panel.review_draft = None;
                    panel.review_selection = None;
                    panel.result = None;
                    panel.status = Message::new(
                        "ICMP 通道未取得可验证的退出结果；诊断权限已停用。远端有独立 8 秒预算，请核对进程后重连。",
                        "ICMP channel has no verifiable exit result; diagnostic authority is disabled. The remote has an independent eight-second budget. Inspect processes before reconnecting.",
                    );
                    cx.notify();
                    return;
                }
                if panel.token != token
                    && result.as_ref().is_ok_and(|report| report.status == NetworkDiagnosticStatus::CleanupUnknown) {
                    panel.session = None;
                    panel.review = None;
                    panel.review_draft = None;
                    panel.review_selection = None;
                    panel.result = None;
                    panel.status = outcome_message(NetworkDiagnosticStatus::CleanupUnknown);
                    cx.notify();
                    return;
                }
                if !panel.authorized(cx) {
                    panel.suspend(cx);
                    return;
                }
                if panel.token != token
                    || panel.draft(cx) != draft
                    || panel.selection_token(cx) != Some(selection)
                {
                    cx.notify();
                    return;
                }
                panel.status = match result {
                    Ok(report) => {
                        let message = outcome_message(report.status);
                        if report.status == NetworkDiagnosticStatus::CleanupUnknown {
                            panel.session = None;
                            panel.review = None;
                            panel.review_draft = None;
                            panel.review_selection = None;
                        }
                        panel.result = Some(report);
                        message
                    }
                    Err(error) => error_message(error),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn authorized(&self, cx: &App) -> bool {
        let Some(session) = &self.session else {
            return false;
        };
        self.authority.as_ref().is_none_or(|(workspace, tab)| {
            workspace
                .upgrade()
                .is_some_and(|workspace| workspace.read(cx).protocol_session_current(*tab, session))
        })
    }
}

impl MonitorPanel {
    /// Match the child's captured connection, rather than comparing the current
    /// workspace map entry to itself during retirement polling.
    pub(crate) fn protocol_connection_matches(&self, session: &SshSession, cx: &App) -> bool {
        self.protocol
            .read(cx)
            .session
            .as_ref()
            .is_some_and(|captured| captured.same_connection(session))
    }

    /// Bind explicit diagnostics to the workspace's current session and route.
    pub(crate) fn bind_protocol_authority(
        &mut self,
        workspace: WeakEntity<crate::workspace::Workspace>,
        tab: EntityId,
        cx: &mut Context<Self>,
    ) {
        self.protocol
            .update(cx, |panel, _| panel.authority = Some((workspace, tab)));
    }
    /// Revoke diagnostics synchronously when saved route/trust metadata changes.
    pub(crate) fn revoke_protocol_authority(&mut self, cx: &mut Context<Self>) {
        self.protocol.update(cx, |panel, cx| panel.suspend(cx));
    }

    /// Retain the connection while withdrawing selection-scoped consent.
    pub(crate) fn invalidate_protocol_selection(&mut self, cx: &mut Context<Self>) {
        self.protocol
            .update(cx, |panel, cx| panel.invalidate_selection(cx));
    }
}

impl Drop for ProtocolPanel {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}

fn outcome_message(status: NetworkDiagnosticStatus) -> Message {
    use NetworkDiagnosticStatus::*;
    match status {
        Success => Message::new(
            "协议操作已完成；HTTP 状态不代表业务健康。",
            "Protocol operation completed; HTTP status does not establish application health.",
        ),
        DnsFailure => Message::new(
            "远端解析失败，请检查目标主机的 DNS/NSS/hosts 配置。",
            "Remote resolution failed; check the host's DNS/NSS/hosts configuration.",
        ),
        ConnectionFailure => Message::new(
            "远端 TCP 连接失败，请检查地址、监听服务与防火墙。",
            "Remote TCP connection failed; check addresses, listeners and firewall.",
        ),
        CertificateRejected => Message::new(
            "远端证书校验拒绝；请检查信任库、名称、有效期与系统时间。没有跳过校验。",
            "Remote certificate verification rejected; check trust store, hostname, validity and system time. Verification was not bypassed.",
        ),
        TlsFailure => Message::new(
            "远端 TLS 握手失败，请检查端口与协议支持。",
            "Remote TLS handshake failed; check port and protocol support.",
        ),
        HttpFailure => Message::new(
            "未收到有效的有界 HTTP 响应头；请检查服务是否支持 HEAD。",
            "No valid bounded HTTP headers; check whether the service supports HEAD.",
        ),
        Timeout => Message::new(
            "远端诊断达到阶段期限（总预算 8 秒）。未确认成功。",
            "Remote diagnostic reached a stage deadline within its eight-second total budget. Success is unconfirmed.",
        ),
        UnsupportedEnvironment => Message::new(
            "目标需要 POSIX 环境及 Python 3.8+ 的 socket/ssl/http/signal 标准库；请自行配置，不会自动安装。",
            "Target requires POSIX and Python 3.8+ socket/ssl/http/signal stdlib; configure separately. Nothing is installed automatically.",
        ),
        CleanupUnknown => Message::new(
            "未确认远端诊断 worker 已退出；已停用此会话的诊断，请核对目标进程后重连。",
            "Remote diagnostic worker exit is unconfirmed; diagnostics for this session are disabled. Inspect target processes before reconnecting.",
        ),
        Interrupted => Message::new(
            "远端诊断被中断，已确认探测 worker 退出。",
            "Remote diagnostic interrupted; the probe worker exited.",
        ),
        IcmpPartial => Message::new(
            "工具报告四次发送尝试，部分未获回复；请查看工具计数与 RTT。",
            "The utility reports four attempts with some unanswered; inspect its counters and RTT.",
        ),
        IcmpNoReply => Message::new(
            "工具报告四次发送尝试，未收到 Echo 回复；不能据此断定主机离线或请求已成功发出。",
            "The utility reports four attempts without echo replies; this establishes neither an offline host nor successful sends.",
        ),
        UtilityUnavailable => Message::new(
            "远端 /usr/bin/ping 或 /bin/ping 不可用；不会安装工具或改用 TCP。",
            "Remote /usr/bin/ping or /bin/ping is unavailable; no installation or TCP fallback.",
        ),
        PermissionDenied => Message::new(
            "远端 ping 的执行或 ICMP 权限被拒绝；请管理员核对权限，不会提权。",
            "Remote ping execution or ICMP permission was denied; ask an administrator to inspect permissions. No privilege escalation.",
        ),
        IcmpFailure => Message::new(
            "ICMP 诊断未通过输出、退出状态或预算校验；请查看固定原因。",
            "ICMP diagnostic failed output, exit-status or budget checks; inspect the fixed reason.",
        ),
    }
}

fn icmp_reason(reason: NetworkDiagnosticIcmpReason, cx: &App) -> &'static str {
    use NetworkDiagnosticIcmpReason::*;
    match reason {
        Completed => t(cx, "工具报告四次回复", "Utility reports four replies"),
        PacketLoss => t(cx, "部分尝试未获回复", "Some attempts unanswered"),
        NoEchoReply => t(cx, "没有 Echo 回复", "No echo reply"),
        Deadline => t(
            cx,
            "监督期限到达；缺失计数未知",
            "Supervisor deadline; missing counts are unknown",
        ),
        Interrupted => t(cx, "已中断并回收", "Interrupted and reaped"),
        CleanupUnknown => t(
            cx,
            "进程退出未确认，须检查后重连",
            "Process exit unconfirmed; inspect before reconnecting",
        ),
        UtilityMissing => t(cx, "缺少 ping 工具", "Ping utility missing"),
        PermissionDenied => t(
            cx,
            "执行或 ICMP 权限不足",
            "Execution or ICMP permission denied",
        ),
        UnsupportedPlatform => t(
            cx,
            "仅支持 Linux POSIX / Python 3.8+",
            "Requires Linux POSIX / Python 3.8+",
        ),
        UnsupportedImplementation => t(
            cx,
            "仅支持 iputils 20240117 的固定输出契约",
            "Requires the fixed iputils 20240117 output contract",
        ),
        ResolutionFailed => t(cx, "远端系统解析失败", "Remote system resolution failed"),
        ProcessFailed => t(cx, "工具启动或退出失败", "Utility start or exit failed"),
        InvalidOutput => t(
            cx,
            "未知或不一致的工具输出",
            "Unknown or inconsistent utility output",
        ),
        OutputLimit => t(
            cx,
            "工具输出超过总计 32 KiB",
            "Combined utility output exceeded 32 KiB",
        ),
        ReportedErrors => t(
            cx,
            "工具报告 ICMP 错误或损坏回包",
            "Utility reported ICMP errors or corrupted replies",
        ),
    }
}
fn error_message(error: NetworkDiagnosticError) -> Message {
    use NetworkDiagnosticError::*;
    match error {
        Cancelled => Message::new(
            "诊断已取消，结果不采用。",
            "Diagnostic cancelled; results discarded.",
        ),
        Timeout => Message::new(
            "SSH 诊断通道超过 10 秒期限，请检查连接。",
            "SSH diagnostic channel exceeded its ten-second deadline; check the connection.",
        ),
        PythonUnavailable => Message::new(
            "远端 PATH 中缺少 Python 3；请自行配置 Python 3.8+ 后重试，不会自动安装。",
            "Python 3 is missing from remote PATH; configure Python 3.8+ separately and retry. Nothing is installed automatically.",
        ),
        UnsupportedEnvironment => outcome_message(NetworkDiagnosticStatus::UnsupportedEnvironment),
        Transport => Message::new(
            "SSH 诊断通道失败，请确认当前会话仍已连接。",
            "SSH diagnostic channel failed; verify the current session is connected.",
        ),
        OutputLimit => Message::new(
            "远端响应超过 32 KiB 上限，结果未采用。",
            "Remote response exceeded 32 KiB; result discarded.",
        ),
        InvalidResponse => Message::new(
            "远端响应格式、退出状态或协议字段无效，不能确认成功。",
            "Remote framing, exit status or protocol fields were invalid; success is unconfirmed.",
        ),
    }
}

fn value_row(id: &str, label: &str, value: String, cx: &App) -> AnyElement {
    let visual = crate::design::palette(cx);
    div()
        .id(id.to_owned())
        .test_support()
        .flex()
        .flex_col()
        .min_w_0()
        .gap_1()
        .text_xs()
        .child(div().text_color(rgb(visual.muted)).child(label.to_owned()))
        .child(div().min_w_0().whitespace_normal().child(value))
        .into_any_element()
}

impl Render for ProtocolPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let busy = self.cancel.is_some();
        let retired = self.session.is_none();
        let mut view = div()
            .id("remote-protocol-diagnostics")
            .test_support()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                Button::new("protocol-expand")
                    .ghost()
                    .compact()
                    .label(t(cx, "协议诊断", "Protocol diagnostics"))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.expanded = !panel.expanded;
                        cx.notify();
                    })),
            );
        if !self.expanded {
            return view;
        }
        view = view
            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "由当前 SSH 主机执行 · 手动审核 · 最多 8 秒",
                "Executed by this SSH host · Manual review · Up to 8s",
            )))
            .child(
                div().flex().flex_wrap().gap_1().children(
                    [
                        (NetworkDiagnosticKind::Dns, "DNS"),
                        (NetworkDiagnosticKind::Tls, "TLS"),
                        (NetworkDiagnosticKind::Http, "HTTP(S)"),
                        (NetworkDiagnosticKind::Icmp, "ICMP ping"),
                    ]
                    .into_iter()
                    .map(|(kind, label)| {
                        Button::new(match kind {
                            NetworkDiagnosticKind::Dns => "protocol-mode-dns",
                            NetworkDiagnosticKind::Tls => "protocol-mode-tls",
                            NetworkDiagnosticKind::Http => "protocol-mode-http",
                            NetworkDiagnosticKind::Icmp => "protocol-mode-icmp",
                        })
                        .ghost()
                        .compact()
                        .selected(self.kind == kind)
                        .disabled(busy || retired)
                        .label(label)
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            panel.kind = kind;
                            panel.sync_input(cx);
                        }))
                    }),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .child(if self.kind == NetworkDiagnosticKind::Http {
                        t(cx, "HTTP(S) URL（无查询参数）", "HTTP(S) URL (no query)")
                    } else {
                        t(cx, "DNS 名称或 IP", "DNS name or IP")
                    }),
            )
            .child(Input::new(&self.endpoint).small().disabled(busy || retired));
        if self.kind == NetworkDiagnosticKind::Tls {
            view = view
                .child(div().text_xs().child(t(cx, "TLS 端口", "TLS port")))
                .child(Input::new(&self.port).small().disabled(busy || retired));
        }
        if self.kind == NetworkDiagnosticKind::Icmp {
            view = view.child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "仅支持远端 Linux iputils 20240117 · 最多 4 个 Echo 请求 · 只探测首个解析地址 · 不自动安装或回退",
                "Remote Linux iputils 20240117 · At most 4 echo requests · First resolved address only · No install or fallback",
            )));
        }
        view = view.child(
            Button::new("protocol-preview")
                .ghost()
                .compact()
                .disabled(busy || retired)
                .label(t(cx, "预览诊断", "Preview diagnostic"))
                .on_click(cx.listener(|panel, _, _, cx| panel.preview(cx))),
        );
        if let Some(request) = &self.review {
            let operation = match request.kind() {
                NetworkDiagnosticKind::Dns => t(
                    cx,
                    "远端系统解析器（含 DNS/NSS/hosts），不连接服务。",
                    "Remote OS resolver (DNS/NSS/hosts); no service connection.",
                ),
                NetworkDiagnosticKind::Tls => t(
                    cx,
                    "严格验证 TLS 握手及证书；不发送应用正文。",
                    "Strict verified TLS handshake and certificate; no application payload.",
                ),
                NetworkDiagnosticKind::Http => t(
                    cx,
                    "发送一次 HEAD；不跟随重定向，不发送凭据、Cookie 或正文。",
                    "Send one HEAD; no redirects, credentials, cookies or body.",
                ),
                NetworkDiagnosticKind::Icmp => t(
                    cx,
                    "由此 SSH 主机解析目标，并向首个地址发送最多 4 个 ICMP Echo 请求。总期限 8 秒，SSH 通道 10 秒。不会提权、安装工具或回退到 TCP。",
                    "This SSH host resolves the target and sends at most 4 ICMP echo requests to its first address. Total budget 8s, SSH envelope 10s. No privilege escalation, installation or TCP fallback.",
                ),
            };
            view = view.child(
                div()
                    .id("protocol-review")
                    .test_support()
                    .min_w_0()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(visual.canvas))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(value_row(
                        "protocol-review-host",
                        t(cx, "执行主机", "Execution host"),
                        self.host.clone(),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-review-endpoint",
                        t(cx, "诊断端点", "Diagnostic endpoint"),
                        request.endpoint_label(),
                        cx,
                    ))
                    .child(div().text_xs().child(operation))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                Button::new("protocol-confirm")
                                    .primary()
                                    .compact()
                                    .disabled(retired)
                                    .label(t(cx, "确认执行", "Confirm probe"))
                                    .on_click(cx.listener(|panel, _, _, cx| panel.confirm(cx))),
                            )
                            .child(
                                Button::new("protocol-dismiss-review")
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "返回", "Back"))
                                    .on_click(cx.listener(|panel, _, _, cx| {
                                        panel.review = None;
                                        panel.review_draft = None;
                                        panel.review_selection = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if busy {
            view = view.child(
                Button::new("protocol-cancel")
                    .ghost()
                    .compact()
                    .label(t(cx, "取消诊断", "Cancel diagnostic"))
                    .on_click(cx.listener(|panel, _, _, cx| panel.cancel(cx))),
            );
        }
        if let Some(report) = &self.result {
            let mut result = div()
                .id("protocol-result")
                .test_support()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .rounded_md()
                .bg(rgb(visual.canvas))
                .child(value_row(
                    "protocol-result-endpoint",
                    t(cx, "本次端点", "This endpoint"),
                    report.request.endpoint_label(),
                    cx,
                ));
            result = result.child(value_row(
                "protocol-addresses",
                t(cx, "远端解析结果", "Remote resolver results"),
                report
                    .addresses
                    .iter()
                    .map(|a| format!("{} {}", a.family, a.address))
                    .collect::<Vec<_>>()
                    .join("\n"),
                cx,
            ));
            if let Some(peer) = &report.peer {
                result = result.child(value_row(
                    "protocol-peer",
                    t(cx, "实际连接地址", "Connected address"),
                    peer.clone(),
                    cx,
                ));
            }
            if let Some(tls) = &report.tls {
                result = result
                    .child(value_row(
                        "protocol-tls",
                        t(cx, "TLS / 密码套件", "TLS / Cipher"),
                        format!("{} · {}", tls.protocol, tls.cipher),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-cert-subject",
                        t(cx, "证书主体", "Certificate subject"),
                        tls.subject.clone(),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-cert-issuer",
                        t(cx, "签发者", "Issuer"),
                        tls.issuer.clone(),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-cert-validity",
                        t(cx, "有效期", "Validity"),
                        format!("{} → {}", tls.not_before, tls.not_after),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-cert-names",
                        t(cx, "证书名称", "Certificate names"),
                        tls.names.join("\n"),
                        cx,
                    ))
                    .child(value_row(
                        "protocol-cert-fingerprint",
                        "SHA-256",
                        tls.sha256.clone(),
                        cx,
                    ));
            }
            if let Some(http) = &report.http {
                result = result.child(value_row(
                    "protocol-http-status",
                    t(cx, "HTTP 状态", "HTTP status"),
                    format!("{} {}", http.version, http.status),
                    cx,
                ));
            }
            if let Some(icmp) = &report.icmp {
                result = result.child(value_row(
                    "protocol-icmp-reason",
                    t(cx, "ICMP 结果原因", "ICMP result reason"),
                    icmp_reason(icmp.reason, cx).into(),
                    cx,
                ));
                if let Some(address) = &icmp.address {
                    result = result.child(value_row(
                        "protocol-icmp-address",
                        t(cx, "本次 ICMP 地址", "This ICMP address"),
                        address.clone(),
                        cx,
                    ));
                }
                if let Some(counts) = &icmp.counts {
                    result = result
                        .child(value_row(
                            "protocol-icmp-counts",
                            t(
                                cx,
                                "工具尝试 / 回复 / 未回复",
                                "Utility attempts / Replies / Unanswered",
                            ),
                            format!(
                                "{} / {} / {}",
                                counts.transmitted,
                                counts.received,
                                counts.transmitted - counts.received
                            ),
                            cx,
                        ))
                        .child(value_row(
                            "protocol-icmp-errors",
                            t(
                                cx,
                                "重复 / 损坏 / ICMP 错误",
                                "Duplicates / Corrupted / ICMP errors",
                            ),
                            format!(
                                "{} / {} / {}",
                                counts.duplicates, counts.corrupted, counts.errors
                            ),
                            cx,
                        ))
                        .child(
                            div()
                                .id("protocol-icmp-counts-limit")
                                .text_xs()
                                .text_color(rgb(visual.muted))
                                .child(t(
                                    cx,
                                    "工具的静默汇总无法区分部分本地发送失败与网络丢包；未回复不证明请求已成功发出。",
                                    "Quiet summaries cannot distinguish some local send failures from network loss; unanswered attempts do not prove successful sends.",
                                )),
                        );
                }
                if let Some(rtt) = &icmp.rtt {
                    result = result.child(value_row(
                        "protocol-icmp-rtt",
                        t(
                            cx,
                            "RTT 最小 / 平均 / 最大 / 标准差",
                            "RTT Min / Mean / Max / Stddev",
                        ),
                        format!(
                            "{} / {} / {} / {} µs",
                            rtt.min_us, rtt.avg_us, rtt.max_us, rtt.mdev_us
                        ),
                        cx,
                    ));
                }
            }
            for (id, label, value) in [
                (
                    "resolve",
                    t(cx, "解析", "Resolve"),
                    report.timing.resolve_ms,
                ),
                (
                    "connect",
                    t(cx, "连接", "Connect"),
                    report.timing.connect_ms,
                ),
                ("tls", t(cx, "握手", "Handshake"), report.timing.tls_ms),
                (
                    "headers",
                    t(cx, "响应头", "Headers"),
                    report.timing.headers_ms,
                ),
                (
                    "total",
                    t(cx, "远端总耗时", "Remote total"),
                    Some(report.timing.total_ms),
                ),
            ] {
                result = result.child(value_row(
                    &format!("protocol-timing-{id}"),
                    label,
                    value.map_or_else(|| "—".into(), |v| format!("{v} ms")),
                    cx,
                ));
            }
            if report.limited {
                result = result.child(div().text_xs().text_color(rgb(visual.warning)).child(t(
                    cx,
                    "地址或证书显示达到上限。",
                    "Address or certificate display reached its limit.",
                )));
            }
            view = view.child(result);
        }
        view.child(
            div()
                .id("protocol-status")
                .test_support()
                .text_xs()
                .min_w_0()
                .text_color(rgb(visual.muted))
                .child(self.status.render(cx)),
        )
    }
}
