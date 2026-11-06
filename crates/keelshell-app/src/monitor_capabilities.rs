//! Capability display reads only the panel cache. Rechecking is an explicit job.

use super::*;
use keelshell_session::monitor::CapabilityReason;

pub(super) fn reason_text(reason: CapabilityReason) -> (&'static str, &'static str) {
    match reason {
        CapabilityReason::CommandMissing => ("所需命令未找到", "Required command not found"),
        CapabilityReason::UnsupportedOptions => {
            ("命令不支持所需选项", "Required options unsupported")
        }
        CapabilityReason::PermissionDenied => ("权限不足", "Permission denied"),
        CapabilityReason::InvalidOutput => {
            ("响应格式无效或不支持", "Invalid or unsupported output")
        }
        CapabilityReason::CommandFailed => ("命令失败，原因未知", "Command failed; cause unknown"),
        CapabilityReason::UnsupportedPlatform => {
            ("仅支持 Linux 远端", "Only Linux targets supported")
        }
        CapabilityReason::Transport => ("SSH 采集失败", "SSH collection failed"),
    }
}

impl MonitorPanel {
    pub(super) fn adopt_capabilities(
        &mut self,
        report: &CapabilityReport,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> bool {
        let observations = report.observations();
        if !self
            .monitor
            .as_ref()
            .is_some_and(|monitor| monitor.accepts_capability_report(report))
            || [
                Capability::Resources,
                Capability::Filesystems,
                Capability::Processes,
                Capability::Sockets,
            ]
            .iter()
            .any(|capability| {
                observations
                    .observation(*capability)
                    .checked_at
                    .is_some_and(|at| now.checked_duration_since(at).is_none())
            })
        {
            self.history.interrupt();
            self.status = Message::new(
                "监控能力状态已变化或时间无效，响应未采用；请刷新。",
                "Monitoring capability state changed or its time is invalid; response discarded. Refresh to retry.",
            );
            cx.notify();
            return false;
        }
        // These are current connection-local cache facts, not new samples. A
        // skipped negative check keeps its real age even if its first delivery
        // expired. Outcome and resource receipt deadlines are checked separately.
        self.capabilities = observations;
        true
    }

    pub(super) fn redetect_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("redetect-monitor")
            .disabled(self.busy || self.suspended || self.pending.is_some())
            .ghost()
            .compact()
            .rounded(px(crate::design::components(cx).control_radius))
            .label(t(cx, "重新检测", "Recheck"))
            .accessibility_label(t(
                cx,
                "重新检测 Linux 监控能力",
                "Recheck Linux monitoring capabilities",
            ))
            .on_click(cx.listener(|panel, _, _, cx| {
                if panel.pending.is_none() {
                    panel.run(Job::Redetect, cx);
                }
            }))
    }

    pub(super) fn capabilities_view(&self, cx: &mut Context<Self>) -> Div {
        let visual = crate::design::palette(cx);
        let mut view = section(
            cx,
            t(cx, "Linux 监控能力", "Linux capabilities"),
            IconName::Activity,
        );
        for (index, (capability, zh, en)) in [
            (
                Capability::Resources,
                "核心资源 /proc",
                "Core resources /proc",
            ),
            (
                Capability::Filesystems,
                "磁盘容量 df",
                "Filesystem capacity df",
            ),
            (Capability::Processes, "进程 ps", "Processes ps"),
            (Capability::Sockets, "监听端口 ss", "Listening sockets ss"),
        ]
        .into_iter()
        .enumerate()
        {
            let observation = self.capabilities.observation(capability);
            let (state_zh, state_en) = match observation.state {
                CapabilityState::Unknown => ("未检测", "Not checked"),
                CapabilityState::Available => ("可用", "Available"),
                CapabilityState::Unavailable(reason) => reason_text(reason),
            };
            let label =
                Message::new(format!("{zh}：{state_zh}"), format!("{en}: {state_en}")).render(cx);
            let checked_age = observation.checked_at.map(|checked_at| {
                let seconds = checked_at.elapsed().as_secs();
                Message::new(
                    format!("{seconds} 秒前检测"),
                    format!("Checked {seconds}s ago"),
                )
                .render(cx)
            });
            let accessible = checked_age
                .as_ref()
                .map_or_else(|| label.clone(), |age| format!("{label}, {age}"));
            let mut row = div()
                .id(("monitor-capability", index))
                .test_support()
                .role(accesskit::Role::Label)
                .aria_label(accessible)
                .min_w_0()
                .text_xs()
                .text_color(rgb(visual.muted))
                .child(label);
            if let Some(age) = checked_age {
                row = row.child(div().child(age));
            }
            view = view.child(row);
        }
        view
    }
}
