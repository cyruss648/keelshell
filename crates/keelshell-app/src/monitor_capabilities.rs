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
        CapabilityReason::UnsupportedPlatform => (
            "目标系统不支持此项能力",
            "Facility unsupported on target platform",
        ),
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
                "重新检测主机监控能力",
                "Recheck host monitoring capabilities",
            ))
            .on_click(cx.listener(|panel, _, _, cx| {
                if panel.pending.is_none() {
                    panel.run(Job::Redetect, cx);
                }
            }))
    }

    pub(super) fn capabilities_view(&self, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        let visual = crate::design::palette(cx);
        let mut view = section(
            cx,
            t(cx, "主机监控能力", "Host capabilities"),
            IconName::Activity,
        )
        .id("monitor-capabilities")
        .test_support()
        .role(accesskit::Role::Group)
        .aria_label(t(cx, "主机监控能力", "Host capabilities"));
        for (index, (capability, zh, en)) in [
            (Capability::Resources, "核心资源", "Core resources"),
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
            let (state_zh, state_en, state_color) = match observation.state {
                CapabilityState::Unknown => ("未检测", "Not checked", visual.muted),
                CapabilityState::Available => ("可用", "Available", visual.success),
                CapabilityState::Unavailable(reason) => {
                    let (zh, en) = reason_text(reason);
                    (zh, en, visual.warning)
                }
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
            let mut details = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .text_xs()
                .text_color(rgb(visual.muted))
                .whitespace_normal()
                .child(label);
            let mut metadata = div().flex().items_center().flex_wrap().gap_1().child(
                div()
                    .flex_shrink_0()
                    .px_1()
                    .rounded_full()
                    .bg(rgb(visual.surface_active))
                    .text_xs()
                    .whitespace_nowrap()
                    .text_color(rgb(state_color))
                    .child(Message::new(state_zh, state_en).render(cx)),
            );
            if let Some(age) = checked_age {
                metadata = metadata.child(div().text_xs().child(age));
            }
            details = details.child(metadata);
            let row = div()
                .id(("monitor-capability", index))
                .test_support()
                .role(accesskit::Role::Label)
                .aria_label(accessible)
                .flex()
                .items_start()
                .gap_2()
                .p_1()
                .rounded(px(crate::design::components(cx).control_radius))
                .min_w_0()
                .child(
                    div()
                        .mt(px(4.))
                        .w(px(7.))
                        .h(px(7.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(state_color)),
                )
                .child(details);
            view = view.child(row);
        }
        view
    }
}
