//! Separate approval and ephemeral results for server tool versions and stream checks.
use super::*;
use keelshell_core::Language;
use keelshell_session::archive_validation::{
    ArchiveValidationReport, ToolCheckStatus, validation_command,
};
use std::time::Instant;

struct Observation {
    probe: archive_inspection::Probe,
    report: ArchiveValidationReport,
    checked: Instant,
}

#[derive(Default)]
pub(super) struct State {
    observation: Option<Observation>,
    running: bool,
    pub(super) failed: bool,
    expanded: bool,
}

impl State {
    pub(super) fn begin(&mut self) {
        self.observation = None;
        self.running = true;
        self.failed = false;
    }

    pub(super) fn finish(&mut self, failed: bool) {
        self.running = false;
        self.failed = failed;
    }

    #[cfg(test)]
    pub(super) fn report(&self) -> Option<&ArchiveValidationReport> {
        self.observation.as_ref().map(|item| &item.report)
    }
}

impl FilesPanel {
    fn review_archive_validation(&mut self, cx: &mut Context<Self>) {
        if !self.can_inspect_archive_tools() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let command = validation_command();
        let probe = archive_inspection::Probe::new(
            self.session_token,
            session,
            self.host.clone(),
            archive_inspection::Kind::Validation,
        );
        self.confirm(
            Message::new(
                format!(
                    "当前 SSH 主机：{}\n\n运行 tar、gzip、zip、unzip、python3、rz、sz 的固定版本参数，并用固定微小内存样本检查 tar、gzip 与 Python 归档流。需要远端 POSIX 环境及 Python 3.8+。\n不创建文件、不读取用户归档、不安装软件、不启动 Zmodem。zip、unzip、rz、sz 只观察版本，不作功能自测。每项有时间及输出限制；取消 SSH 请求不证明远端进程已经结束。\n结果仅属于本次实际 SSH 连接，不授权后续打包或传输。所有远端版本文字均为观察值。\n\n完整核验命令：\n{command}",
                    self.host,
                ),
                format!(
                    "Current SSH host: {}\n\nRun fixed version arguments for tar, gzip, zip, unzip, python3, rz and sz, then check tar, gzip and Python archive streams using fixed small in-memory samples. Requires remote POSIX and Python 3.8+.\nCreates no files, reads no user archives, installs nothing and never starts Zmodem. zip, unzip, rz and sz receive version checks only. Each check has time and output limits; cancelling SSH does not prove remote process termination.\nResults belong only to this actual SSH connection and authorize no packing or transfer. Remote version text is an observation.\n\nComplete validation command:\n{command}",
                    self.host,
                ),
            ),
            Operation::InspectArchiveTools(probe),
            cx,
        );
    }

    pub(super) fn receive_archive_validation(
        &mut self,
        probe: archive_inspection::Probe,
        report: ArchiveValidationReport,
        cx: &mut Context<Self>,
    ) {
        // The caller also checks identity; retain it here so this adoption path
        // cannot become an authority shortcut when another caller is added.
        if !self.archive_probe_is_current(&probe) || !probe.validates_tools() {
            self.status = Message::new(
                "核验期间 SSH 会话已变化，未采用旧工具结果。",
                "The SSH session changed during validation; old tool results were discarded.",
            );
            cx.notify();
            return;
        }
        let state = &mut self.archive_inspection.validation;
        state.finish(false);
        state.observation = Some(Observation {
            probe,
            report,
            checked: Instant::now(),
        });
        self.status = Message::new(
            "工具核验完成，请查看各项版本观察与流式检查；未测试项不代表可用。",
            "Tool checks completed. Review each version observation and stream result; untested features remain unverified.",
        );
        cx.notify();
    }

    pub(super) fn archive_validation_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let state = &self.archive_inspection.validation;
        let observation = state
            .observation
            .as_ref()
            .filter(|item| self.archive_probe_is_current(&item.probe));
        let summary = if state.running {
            t(cx, "正在核验…", "Checking…").to_owned()
        } else if state.failed {
            t(
                cx,
                "上次核验未完成，请重新审核。",
                "The last check did not complete. Review a new check.",
            )
            .to_owned()
        } else if let Some(item) = observation {
            let passed = item
                .report
                .tools
                .iter()
                .filter(|tool| tool.stream_check == ToolCheckStatus::Passed)
                .count();
            let seconds = item.checked.elapsed().as_secs();
            match crate::i18n::language(cx) {
                Language::ZhCn => format!("{passed} 项流式检查通过；{seconds} 秒前核验"),
                Language::En => format!("{passed} stream checks passed; checked {seconds}s ago"),
            }
        } else {
            t(
                cx,
                "尚未核验版本与功能",
                "Versions and features not checked",
            )
            .to_owned()
        };
        let expected_token = self.session_token;
        let expected_session = self.session.clone();
        let mut section = div()
            .id("archive-tool-validation")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("review-archive-tool-validation")
                            .ghost()
                            .compact()
                            .small()
                            .rounded(px(crate::design::components(cx).control_radius))
                            .label(t(cx, "版本与自测", "Versions and checks"))
                            .accessibility_label(t(
                                cx,
                                "审核当前 SSH 主机的工具版本与流式归档自测",
                                "Review tool versions and archive stream checks on the current SSH host",
                            ))
                            .disabled(!self.can_inspect_archive_tools())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.session_token == expected_token
                                    && view.session.as_ref().zip(expected_session.as_ref())
                                        .is_some_and(|(current, painted)| current.same_connection(painted))
                                {
                                    view.review_archive_validation(cx);
                                }
                            })),
                    )
                    .child(div().text_color(rgb(visual.muted)).child(summary))
                    .when(observation.is_some(), |row| {
                        row.child(
                            Button::new("toggle-archive-validation-details")
                                .ghost()
                                .compact()
                                .small()
                                .label(t(cx, "核验详情", "Check details"))
                                .accessibility_label(t(
                                    cx,
                                    "展开或收起七项工具版本与自测结果",
                                    "Expand or collapse seven tool version and stream results",
                                ))
                                .selected(state.expanded)
                                .toggled(state.expanded)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let state = &mut view.archive_inspection.validation;
                                    state.expanded = !state.expanded;
                                    cx.notify();
                                })),
                        )
                    }),
            );
        if state.expanded
            && let Some(item) = observation
        {
            let mut rows = div()
                .id("archive-validation-results")
                .w_full()
                .min_w_0()
                .max_h(px(144.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2();
            for (index, tool) in item.report.tools.iter().enumerate() {
                let color = match tool.stream_check {
                    ToolCheckStatus::Passed => visual.success,
                    ToolCheckStatus::Failed | ToolCheckStatus::OutputLimit => visual.danger,
                    ToolCheckStatus::TimedOut => visual.warning,
                    ToolCheckStatus::Unavailable | ToolCheckStatus::NotTested => visual.muted,
                };
                let labels = match crate::i18n::language(cx) {
                    Language::ZhCn => format!(
                        "{}  版本：{}；流式检查：{}",
                        tool.tool.name(),
                        status_label(tool.version_status, cx),
                        status_label(tool.stream_check, cx),
                    ),
                    Language::En => format!(
                        "{}  version: {}; stream check: {}",
                        tool.tool.name(),
                        status_label(tool.version_status, cx),
                        status_label(tool.stream_check, cx),
                    ),
                };
                rows = rows.child(
                    div()
                        .id(("archive-validation-row", index))
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_color(rgb(color)).child(labels))
                        .child(
                            div()
                                .text_color(rgb(visual.muted))
                                .child(tool.path.clone().unwrap_or_else(|| "—".to_owned())),
                        )
                        .when_some(tool.version.as_ref(), |row, version| {
                            row.child(div().child(version.clone()))
                        })
                        .test_support(),
                );
            }
            section = section.child(rows.test_support()).child(
                div().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "版本文字来自远端程序；未测试 ZIP 工具功能及 Zmodem 互通，不授权传输。",
                    "Version text comes from remote programs. ZIP tool features and Zmodem interoperability remain untested; no transfer is authorized.",
                )),
            );
        }
        section.test_support().into_any_element()
    }
}

fn status_label(status: ToolCheckStatus, cx: &App) -> &'static str {
    match status {
        ToolCheckStatus::Passed => t(cx, "通过", "Passed"),
        ToolCheckStatus::Failed => t(cx, "失败", "Failed"),
        ToolCheckStatus::Unavailable => t(cx, "不可用", "Unavailable"),
        ToolCheckStatus::NotTested => t(cx, "未测试", "Not tested"),
        ToolCheckStatus::TimedOut => t(cx, "超时", "Timed out"),
        ToolCheckStatus::OutputLimit => t(cx, "输出超限", "Output limit"),
    }
}
