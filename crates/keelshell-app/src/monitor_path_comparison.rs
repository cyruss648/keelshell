//! Manual comparisons borrow two authenticated connections without changing
//! either route. Consent never survives a selection or transport replacement.

use super::*;
use gpui_kit::base::TestTextExt;
use gpui_kit::component::Selectable;
use keelshell_session::path_comparison::{
    PathComparisonPlan, PathComparisonReport, PathComparisonStatus, PathProbeFailure,
    PathProbeKind, PathSampleOutcome, PathSide,
};
#[cfg(test)]
#[path = "monitor_tests/path_comparison.rs"]
mod tests;

#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) tab: EntityId,
    pub(crate) session: SshSession,
    pub(crate) label: String,
}

struct Review {
    plan: PathComparisonPlan,
    candidate: Candidate,
    selection: uuid::Uuid,
}

struct ResultSource {
    routes: [keelshell_session::SshConnectionInfo; 2],
}

pub(crate) struct PathComparisonPanel {
    authority: Option<(WeakEntity<crate::workspace::Workspace>, EntityId)>,
    session: Option<SshSession>,
    runtime: Arc<tokio::runtime::Runtime>,
    candidates: Vec<Candidate>,
    selected: Option<Candidate>,
    review: Option<Review>,
    result: Option<PathComparisonReport>,
    result_source: Option<ResultSource>,
    expanded: bool,
    show_samples: bool,
    show_result_review: bool,
    status: Message,
    request: uuid::Uuid,
    cancel: Option<Arc<AtomicBool>>,
    cleanup_unconfirmed: bool,
    _job: Option<Task<()>>,
}

impl PathComparisonPanel {
    pub(super) fn new(session: SshSession, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        Self {
            authority: None,
            session: Some(session),
            runtime,
            candidates: Vec::new(),
            selected: None,
            review: None,
            result: None,
            result_source: None,
            expanded: false,
            show_samples: false,
            show_result_review: false,
            status: Message::empty(),
            request: uuid::Uuid::new_v4(),
            cancel: None,
            cleanup_unconfirmed: false,
            _job: None,
        }
    }

    pub(super) fn bind(
        &mut self,
        workspace: WeakEntity<crate::workspace::Workspace>,
        tab: EntityId,
    ) {
        self.authority = Some((workspace, tab));
    }

    pub(super) fn busy(&self) -> bool {
        self.cancel.is_some()
    }

    fn selection(&self, candidate: &Candidate, cx: &App) -> Option<uuid::Uuid> {
        let session = self.session.as_ref()?;
        if self.cleanup_unconfirmed || session.is_closed() || candidate.session.is_closed() {
            return None;
        }
        let (workspace, tab) = self.authority.as_ref()?;
        workspace
            .upgrade()?
            .read(cx)
            .path_comparison_current(*tab, session, candidate)
    }

    fn refresh_candidates(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.cleanup_unconfirmed {
            return;
        }
        self.invalidate_selection(cx);
        self.selected = None;
        self.candidates = self
            .authority
            .as_ref()
            .and_then(|(workspace, tab)| {
                let session = self.session.as_ref()?;
                Some(
                    workspace
                        .upgrade()?
                        .read(cx)
                        .path_comparison_candidates(*tab, session),
                )
            })
            .unwrap_or_default();
        self.status = if self.candidates.is_empty() {
            Message::new(
                "请先在同一窗口连接同一目标的另一条路线。目标地址、端口、用户和已核验密钥须一致，跳板或代理路线须不同。",
                "Connect another route to this target in the same window. Address, port, user and verified key must match; jump or proxy routes must differ.",
            )
        } else {
            Message::new("选择路径 B 后预览。", "Select path B, then preview.")
        };
        cx.notify();
    }

    fn select(&mut self, candidate: Candidate, cx: &mut Context<Self>) {
        if self.busy() || self.cleanup_unconfirmed {
            return;
        }
        self.invalidate_selection(cx);
        self.selected = Some(candidate);
        self.status = Message::new(
            "已选择路径 B；预览不会执行命令。",
            "Path B selected; preview does not execute commands.",
        );
        cx.notify();
    }

    pub(super) fn invalidate_selection(&mut self, cx: &mut Context<Self>) {
        self.request = uuid::Uuid::new_v4();
        self.review = None;
        self.result = None;
        self.result_source = None;
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
        self.status = Message::new(
            "选择或路线已变化，旧审核与结果已失效；请重新预览。",
            "Selection or route changed; previous review and results are invalid. Preview again.",
        );
        cx.notify();
    }

    pub(super) fn suspend(&mut self, cx: &mut Context<Self>) {
        self.invalidate_selection(cx);
        self.session = None;
        self.selected = None;
        self.candidates.clear();
        self.status = Message::new(
            "原连接已失效；请重连后重新选择路线。",
            "Original connection is no longer current; reconnect and select routes again.",
        );
        cx.notify();
    }

    fn preview(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.cleanup_unconfirmed {
            return;
        }
        self.review = None;
        self.result = None;
        let Some(candidate) = self.selected.clone() else {
            return;
        };
        let Some(selection) = self.selection(&candidate, cx) else {
            self.invalidate_selection(cx);
            return;
        };
        let Some(first) = &self.session else {
            return;
        };
        self.status = match PathComparisonPlan::new(first.clone(), candidate.session.clone()) {
            Ok(plan) => {
                self.review = Some(Review {
                    plan,
                    candidate,
                    selection,
                });
                Message::new(
                    "核对两条路线、固定命令与 3 MiB 合成载荷后确认。",
                    "Review both routes, fixed commands and 3 MiB of synthetic payload before confirming.",
                )
            }
            Err(_) => Message::new(
                "这两条连接无法比较；请刷新可比路线。",
                "These connections cannot be compared; refresh the route list.",
            ),
        };
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.cleanup_unconfirmed {
            return;
        }
        let Some(review) = self.review.take() else {
            return;
        };
        if self.selection(&review.candidate, cx) != Some(review.selection)
            || !self.session.as_ref().is_some_and(|first| {
                review
                    .plan
                    .matches_sessions(first, &review.candidate.session)
            })
            || !self.selected.as_ref().is_some_and(|current| {
                current.tab == review.candidate.tab
                    && current.session.same_connection(&review.candidate.session)
            })
        {
            self.invalidate_selection(cx);
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let plan = review.plan.clone();
        let runtime = self.runtime.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let started = crate::terminal::spawn_owned_transport_worker(
            cx.entity_id(),
            "keelshell-path-comparison",
            cancel.clone(),
            move || {
                let result = runtime.block_on(plan.run(&worker_cancel));
                let _ = sender.try_send(result);
                Ok(())
            },
        );
        if started.is_err() {
            self.review = Some(review);
            self.status = Message::new("无法启动比较任务。", "Unable to start comparison worker.");
            cx.notify();
            return;
        }
        self.request = uuid::Uuid::new_v4();
        let request = self.request;
        self.cancel = Some(cancel);
        self.result = None;
        // Completed evidence retains credential-free metadata, not transports.
        self.result_source = Some(ResultSource {
            routes: [
                review.plan.connection_info(PathSide::A).clone(),
                review.plan.connection_info(PathSide::B).clone(),
            ],
        });
        self.status = Message::new(
            "正在交替测量两条已审核路径，最多 60 秒…",
            "Measuring both reviewed paths in alternating order, up to 60 seconds…",
        );
        let executor = cx.background_executor().clone();
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = loop {
                match receiver.try_recv() {
                    Ok(report) => break Some(report),
                    Err(TryRecvError::Empty) => executor.timer(Duration::from_millis(20)).await,
                    Err(TryRecvError::Disconnected) => break None,
                }
            };
            let _ = this.update(cx, |panel, cx| {
                panel.finish(request, review, result, cx);
            });
        }));
        cx.notify();
    }

    fn finish(
        &mut self,
        request: uuid::Uuid,
        review: Review,
        result: Option<PathComparisonReport>,
        cx: &mut Context<Self>,
    ) {
        self.cancel = None;
        let current = self.request == request
            && self.selection(&review.candidate, cx) == Some(review.selection);
        // A revoked reply cannot authorize another probe after uncertain cleanup.
        if result
            .as_ref()
            .is_none_or(|report| report.cleanup_unconfirmed)
        {
            self.cleanup_unconfirmed = true;
            self.review = None;
            self.status = Message::new(
                "未确认探测程序结束，本面板停止继续比较；请核对目标进程后重连。",
                "Probe completion is unconfirmed; further comparisons are disabled here. Inspect target processes before reconnecting.",
            );
        }
        if !current {
            cx.notify();
            return;
        }
        if let Some(report) = result {
            if !self.cleanup_unconfirmed {
                self.status = match report.status {
                    PathComparisonStatus::Complete => Message::new(
                        "18 个样本已完成。结果仅描述本次路径与负载，不能推断普遍加速。",
                        "All 18 samples completed. Results describe these paths and this load; they do not establish a general speedup.",
                    ),
                    PathComparisonStatus::Cancelled => Message::new(
                        "已取消；保留已完成样本，未生成完整比较。",
                        "Cancelled; completed samples are retained without a complete comparison.",
                    ),
                    PathComparisonStatus::Incomplete => Message::new(
                        "样本未全部成功；请查看原因，不生成完整比较。",
                        "Some samples did not succeed; inspect their reasons. No complete comparison.",
                    ),
                };
            }
            self.result = Some(report);
        }
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
            self.status = Message::new(
                "正在停止本次探测并收取结果…",
                "Stopping this probe and collecting its outcome…",
            );
            cx.notify();
        }
    }
}

impl Drop for PathComparisonPanel {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}

impl MonitorPanel {
    pub(crate) fn path_comparison_candidate(&self, cx: &App) -> Option<Candidate> {
        self.path_comparison.read(cx).selected.clone()
    }

    pub(crate) fn invalidate_path_comparison_candidate(&mut self, cx: &mut Context<Self>) {
        self.path_comparison.update(cx, |panel, cx| {
            panel.invalidate_selection(cx);
            panel.selected = None;
            panel.candidates.clear();
        });
    }
}

fn kind_name(kind: PathProbeKind, cx: &App) -> &'static str {
    match kind {
        PathProbeKind::Exec => t(cx, "命令往返", "Command round trip"),
        PathProbeKind::Upload => t(cx, "合成上传", "Synthetic upload"),
        PathProbeKind::Download => t(cx, "合成下载", "Synthetic download"),
    }
}

fn sample_time(duration: Duration, kind: PathProbeKind) -> String {
    let time = format!("{:.2} ms", duration.as_secs_f64() * 1000.);
    if kind == PathProbeKind::Exec || duration.is_zero() {
        time
    } else {
        format!("{time}  ({:.2} MiB/s)", 0.25 / duration.as_secs_f64())
    }
}

fn failure_text(reason: PathProbeFailure, cx: &App) -> &'static str {
    use PathProbeFailure::*;
    match reason {
        Cancelled => t(cx, "已取消探测", "Probe cancelled"),
        SampleDeadline => t(
            cx,
            "样本超过八秒期限",
            "Sample exceeded its eight-second deadline",
        ),
        ComparisonDeadline => t(
            cx,
            "整次比较超过六十秒期限",
            "Comparison exceeded its sixty-second deadline",
        ),
        ConnectionClosed => t(cx, "原连接已关闭", "Original connection closed"),
        Transport => t(cx, "SSH 传输失败", "SSH transport failed"),
        ChannelRejected => t(cx, "SSH 通道被拒绝", "SSH channel rejected"),
        ExecRejected => t(cx, "命令执行请求被拒绝", "Exec request rejected"),
        MissingAcknowledgement => t(
            cx,
            "未收到明确的命令接纳确认",
            "No explicit exec acknowledgement",
        ),
        InvalidSequence => t(cx, "协议消息顺序无效", "Invalid protocol message sequence"),
        InvalidStdout => t(
            cx,
            "响应与固定样本不一致",
            "Response does not match the fixed sample",
        ),
        UnexpectedStderr => t(
            cx,
            "出现非预期错误输出或扩展数据",
            "Unexpected stderr or extended data",
        ),
        OutputLimit => t(cx, "响应超过样本输出上限", "Sample output limit exceeded"),
        MissingEof => t(cx, "未确认输出结束", "Output EOF unconfirmed"),
        MissingExitStatus => t(cx, "未取得进程退出状态", "Process exit status missing"),
        UnsuccessfulExit => t(
            cx,
            "程序退出码非零；请检查 POSIX/Python 3 环境",
            "Nonzero exit; check the POSIX/Python 3 environment",
        ),
        RemoteSignal => t(cx, "程序被信号中止", "Process interrupted by a signal"),
        MissingClose => t(cx, "未确认通道结束", "Channel close unconfirmed"),
        IncompleteInput => t(cx, "样本输入未完整发送", "Sample input was not fully sent"),
    }
}

fn difference_text(a: Duration, b: Duration, cx: &App) -> String {
    if a.is_zero() || b.is_zero() || a == b {
        return t(
            cx,
            "观察差异不足，无法判断哪条路径更快。",
            "Insufficient observed difference to determine which path is faster.",
        )
        .to_owned();
    }
    if a.abs_diff(b) < Duration::from_micros(10) {
        return t(
            cx,
            "差值低于 0.01 ms 显示精度，无法判断哪条路径更快。",
            "Difference is below the 0.01 ms display precision; no faster path determined.",
        )
        .to_owned();
    }
    // Compare display precision in integer time first. Convert only for the
    // signed display; unsigned subtraction would underflow when B is faster.
    let difference = (b.as_secs_f64() - a.as_secs_f64()) * 1000.;
    format!(
        "{}: {difference:+.2} ms. {}",
        t(cx, "实测差值 B − A", "Observed difference B − A"),
        t(cx, "仅限本次观察。", "This observation only."),
    )
}

fn localized_review(plan: &PathComparisonPlan, cx: &App) -> String {
    review_text(
        [
            plan.connection_info(PathSide::A),
            plan.connection_info(PathSide::B),
        ],
        cx,
    )
}

fn observed_text(text: String) -> impl IntoElement {
    // The observer and renderer share one string, never a second description.
    // In normal builds this adds no element or state.
    let text: SharedString = text.into();
    text.clone().test_text(text)
}

fn review_text(routes: [&keelshell_session::SshConnectionInfo; 2], cx: &App) -> String {
    use std::fmt::Write;
    let mut text = String::new();
    for (side, route) in ["A", "B"].into_iter().zip(routes) {
        let _ = writeln!(text, "{} {side}", t(cx, "路径", "Path"));
        let mut hops = Vec::new();
        let mut current = Some(route);
        while let Some(hop) = current {
            hops.push(hop);
            current = hop.jump();
        }
        for (ordinal, hop) in hops.into_iter().rev().enumerate() {
            let _ = writeln!(
                text,
                "{} {}: {}={:?}, {}={:?}, {}={}, {}={:?}",
                t(cx, "SSH 跳", "SSH hop"),
                ordinal + 1,
                t(cx, "用户", "user"),
                hop.username(),
                t(cx, "主机", "host"),
                hop.host(),
                t(cx, "端口", "port"),
                hop.port(),
                t(cx, "已核验密钥", "verified key"),
                hop.verified_host_key()
            );
            let _ = match hop.proxy() {
                Some(proxy) => writeln!(
                    text,
                    "{}: {:?}, {}={:?}, {}={}",
                    t(cx, "显式代理", "Explicit proxy"),
                    proxy.kind(),
                    t(cx, "主机", "host"),
                    proxy.host(),
                    t(cx, "端口", "port"),
                    proxy.port()
                ),
                None => writeln!(
                    text,
                    "{}: {}",
                    t(cx, "显式代理", "Explicit proxy"),
                    t(cx, "无", "none")
                ),
            };
        }
    }
    text.push_str(t(cx,
        "\n最终请求的主机、端口、用户及已核验密钥一致。DNS/集群仍可能选择不同物理实例；这是同一逻辑目标的路径比较。\n固定三轮、18 个样本，每轮按命令往返、上传、下载执行；每项第一轮 A/B、第二轮 B/A、第三轮 A/B。失败或取消即停止，不重试，不补未测样本。\n每次上传/下载为 262144 字节（256 KiB），合计 3145728 字节（3 MiB）合成载荷，另有固定命令、确认与 SSH 开销。\n单样本绝对期限八秒，整次绝对期限六十秒。固定远端 Python 程序另设八秒 alarm。需要 POSIX 与 Python 3；纯内存操作，不读取用户文件、不写远端文件、不安装工具、不请求 PTY 或提权。\n计时包含 SSH 通道、命令/解释器启动、传输及 EOF、退出和 CLOSE 确认；不是 ICMP RTT 或裸链路带宽。仅完整成功结果生成中位数，不自动选路或切换。\n取消只关闭本次自有样本通道。发送 CLOSE 不能证明远端程序已退出；未确认清理时停用本面板继续比较，请核对目标进程后重连。\n",
        "\nFinal requested host, port, user and verified key match. DNS/clusters may still select different physical instances; this compares routes to one logical endpoint.\nThree rounds, eighteen samples: Exec, Upload, Download in each round. Each operation uses A/B, B/A, A/B across the rounds. Stop at the first failure or cancellation; no retries or invented samples.\nEach upload/download is 262144 bytes (256 KiB); total synthetic payload is 3145728 bytes (3 MiB), plus fixed commands, acknowledgements and SSH overhead.\nAbsolute deadlines are eight seconds per sample and sixty seconds overall. Each fixed remote Python program also arms an eight-second alarm. Requires POSIX/Python 3; memory only, no user files, remote files, installation, PTY or privilege escalation.\nTimings include SSH channel, command/interpreter startup, transfer, EOF, exit and CLOSE confirmation; neither ICMP RTT nor bare link bandwidth. Medians require complete success; no automatic route selection or switching.\nCancellation closes only this run's owned sample channels. Sending CLOSE does not prove remote process exit. Unconfirmed cleanup disables further comparisons here; inspect target processes before reconnecting.\n",
    ));
    for kind in [
        PathProbeKind::Exec,
        PathProbeKind::Upload,
        PathProbeKind::Download,
    ] {
        let _ = writeln!(
            text,
            "\n{}: {}\n{}",
            t(cx, "精确命令", "Exact command"),
            kind_name(kind, cx),
            kind.command()
        );
    }
    text
}

impl Render for PathComparisonPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let unavailable = self.session.is_none() || self.cleanup_unconfirmed;
        let busy = self.busy();
        let mut view = div()
            .id("ssh-path-comparison")
            .test_support()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(
                Button::new("path-comparison-expand")
                    .ghost()
                    .compact()
                    .label(t(cx, "路径比较", "Compare paths"))
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
                "同一逻辑目标的两条已认证路径。三轮，3 MiB 合成载荷；计时包含 SSH 通道与命令启动。",
                "Two authenticated routes to one logical target. Three rounds, 3 MiB synthetic payload; timings include SSH channel and command startup.",
            )))
            .child(
                Button::new("path-comparison-refresh")
                    .ghost()
                    .compact()
                    .disabled(busy || unavailable)
                    .label(t(cx, "刷新可比路线", "Refresh comparable routes"))
                    .on_click(cx.listener(|panel, _, _, cx| panel.refresh_candidates(cx))),
            );
        let candidates = self.candidates.clone();
        if !candidates.is_empty() {
            view = view.child(
                div()
                    .id("path-comparison-candidates")
                    .min_w_0()
                    .max_h(px(96.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(
                        candidates
                            .into_iter()
                            .enumerate()
                            .map(|(index, candidate)| {
                                let selected = self.selected.as_ref().is_some_and(|current| {
                                    current.tab == candidate.tab
                                        && current.session.same_connection(&candidate.session)
                                });
                                Button::new(("path-comparison-route", index))
                                    .ghost()
                                    .compact()
                                    .selected(selected)
                                    .disabled(busy || unavailable)
                                    .label(format!("B: {}", candidate.label))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.select(candidate.clone(), cx)
                                    }))
                            }),
                    ),
            );
        }
        if let Some(candidate) = &self.selected {
            view = view.child(
                div()
                    .id("path-comparison-selected")
                    .text_xs()
                    .min_w_0()
                    .whitespace_normal()
                    .child(format!("B: {}", candidate.label)),
            );
        }
        view = view.child(
            Button::new("path-comparison-preview")
                .ghost()
                .compact()
                .disabled(busy || unavailable || self.selected.is_none())
                .label(t(cx, "预览路径比较", "Preview comparison"))
                .on_click(cx.listener(|panel, _, _, cx| panel.preview(cx))),
        );
        if let Some(review) = &self.review {
            view = view.child(
                div()
                    .id("path-comparison-review")
                    .test_support()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .bg(rgb(visual.canvas))
                    .rounded(px(crate::design::components(cx).control_radius))
                    .child(
                        div()
                            .id("path-comparison-review-body")
                            .test_support()
                            .h(px(160.))
                            .min_w_0()
                            .overflow_y_scroll()
                            .text_xs()
                            .whitespace_normal()
                            .child(observed_text(localized_review(&review.plan, cx))),
                    )
                    .child(
                        div()
                            .id("path-comparison-review-actions")
                            .test_support()
                            .flex()
                            .flex_shrink_0()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                Button::new("path-comparison-confirm")
                                    .primary()
                                    .compact()
                                    .label(t(cx, "确认比较", "Confirm comparison"))
                                    .on_click(cx.listener(|panel, _, _, cx| panel.confirm(cx))),
                            )
                            .child(
                                Button::new("path-comparison-back")
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "返回", "Back"))
                                    .on_click(cx.listener(|panel, _, _, cx| {
                                        panel.review = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if busy {
            view = view.child(
                Button::new("path-comparison-cancel")
                    .ghost()
                    .compact()
                    .label(t(cx, "停止比较", "Stop comparison"))
                    .on_click(cx.listener(|panel, _, _, cx| panel.cancel(cx))),
            );
        }
        view = view.child(
            div()
                .id("path-comparison-status")
                .test_support()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(self.status.render(cx)),
        );
        if let Some(report) = &self.result {
            for kind in [
                PathProbeKind::Exec,
                PathProbeKind::Upload,
                PathProbeKind::Download,
            ] {
                let (a, b) = (
                    report.summary(PathSide::A, kind),
                    report.summary(PathSide::B, kind),
                );
                if let (Some(a), Some(b)) = (a, b) {
                    view = view.child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .text_xs()
                            .gap_1()
                            .child(format!(
                                "{} ({})",
                                kind_name(kind, cx),
                                t(cx, "三次中位数", "median of three"),
                            ))
                            .child(format!("A: {}", sample_time(a, kind)))
                            .child(format!("B: {}", sample_time(b, kind)))
                            .child(
                                div()
                                    .id(("path-comparison-difference", kind as usize))
                                    .test_support()
                                    .child(observed_text(difference_text(a, b, cx))),
                            ),
                    );
                } else {
                    view = view.child(div().text_xs().child(format!(
                        "{}: {}",
                        kind_name(kind, cx),
                        t(
                            cx,
                            "数据不足，无法完整比较。",
                            "Insufficient data for a complete comparison."
                        ),
                    )));
                }
            }
            view = view
                .child(
                    Button::new("path-comparison-samples-toggle")
                        .ghost()
                        .compact()
                        .label(t(cx, "原始样本", "Raw samples"))
                        .on_click(cx.listener(|panel, _, _, cx| {
                            panel.show_samples = !panel.show_samples;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("path-comparison-source-toggle")
                        .ghost()
                        .compact()
                        .label(t(cx, "本次路线与命令", "Run routes and commands"))
                        .on_click(cx.listener(|panel, _, _, cx| {
                            panel.show_result_review = !panel.show_result_review;
                            cx.notify();
                        })),
                );
            if self.show_samples {
                view = view.child(
                    div()
                        .id("path-comparison-raw-samples")
                        .test_support()
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(report.samples.iter().enumerate().map(|(index, sample)| {
                            let outcome = match &sample.outcome {
                                PathSampleOutcome::Passed => t(cx, "通过", "Passed").to_owned(),
                                PathSampleOutcome::Failed(reason) => {
                                    format!(
                                        "{}: {}",
                                        t(cx, "失败", "Failed"),
                                        failure_text(*reason, cx)
                                    )
                                }
                            };
                            div()
                                .text_xs()
                                .whitespace_normal()
                                .child(observed_text(format!(
                                    "#{} {:?}  {} {}  {}  {} B  {:.2} ms  {}",
                                    index + 1,
                                    sample.path,
                                    t(cx, "轮", "round"),
                                    sample.round,
                                    kind_name(sample.kind, cx),
                                    sample.bytes,
                                    sample.elapsed.as_secs_f64() * 1000.,
                                    outcome,
                                )))
                        })),
                );
            }
            if let Some(source) = self
                .result_source
                .as_ref()
                .filter(|_| self.show_result_review)
            {
                view = view.child(
                    div()
                        .id("path-comparison-run-source")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .min_w_0()
                        .text_xs()
                        .whitespace_normal()
                        .child(observed_text(review_text(
                            [&source.routes[0], &source.routes[1]],
                            cx,
                        ))),
                );
            }
        }
        view
    }
}
