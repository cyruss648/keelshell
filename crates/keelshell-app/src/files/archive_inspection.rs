//! Explicit, session-bound observations of server-side archive tools.
use super::*;
use keelshell_session::archive_tools::{ArchiveToolAvailability, ArchiveToolReport, probe_command};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Paths,
    Validation,
}

#[derive(Clone)]
pub(super) struct Probe {
    token: uuid::Uuid,
    session: SshSession,
    host: String,
    kind: Kind,
}

impl Probe {
    pub(super) fn new(token: uuid::Uuid, session: SshSession, host: String, kind: Kind) -> Self {
        Self {
            token,
            session,
            host,
            kind,
        }
    }

    pub(super) fn matches_session(&self, session: &SshSession) -> bool {
        self.session.same_connection(session)
    }

    pub(super) fn validates_tools(&self) -> bool {
        self.kind == Kind::Validation
    }
}

pub(super) enum Reply {
    Paths(ArchiveToolReport),
    Validation(keelshell_session::archive_validation::ArchiveValidationReport),
}

pub(super) struct Observation {
    probe: Probe,
    pub(super) report: ArchiveToolReport,
    checked: Instant,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) observation: Option<Observation>,
    running: bool,
    pub(super) failed: bool,
    expanded: bool,
    pub(super) validation: super::archive_validation::State,
}

impl FilesPanel {
    pub(super) fn archive_probe_is_current(&self, probe: &Probe) -> bool {
        !self.suspended
            && self.session_token == probe.token
            && self.host == probe.host
            && self
                .session
                .as_ref()
                .is_some_and(|session| probe.matches_session(session))
    }

    pub(super) fn can_inspect_archive_tools(&self) -> bool {
        !self.suspended
            && self.session.is_some()
            && self.operation_id.is_none()
            && self.pending.is_none()
            && (!self.busy || self.has_active_transfers())
    }

    fn review_archive_tools(&mut self, cx: &mut Context<Self>) {
        if !self.can_inspect_archive_tools() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let command = probe_command();
        let probe = Probe {
            token: self.session_token,
            session,
            host: self.host.clone(),
            kind: Kind::Paths,
        };
        self.confirm(
            Message::new(
                format!(
                    "当前 SSH 主机：{}\n\n只观察 tar、gzip、zip、unzip、python3、rz、sz 在当前 PATH 中的解析结果，不运行这些程序，不安装工具，不读写归档。\n找到路径不等于版本、功能或 Zmodem 互通已经验证。仅在当前会话执行一次；取消或重新连接后需重新审核。\n\n完整检测命令：\n{command}",
                    self.host,
                ),
                format!(
                    "Current SSH host: {}\n\nObserve only how tar, gzip, zip, unzip, python3, rz and sz resolve in the current PATH. Does not run those programs, install tools or read/write archives.\nA discovered path does not verify a version, feature or Zmodem interoperability. Runs once on the current session; review again after cancellation or reconnection.\n\nComplete inspection command:\n{command}",
                    self.host,
                ),
            ),
            Operation::InspectArchiveTools(probe),
            cx,
        );
    }

    pub(super) fn begin_archive_probe(&mut self, probe: &Probe) {
        if probe.validates_tools() {
            self.archive_inspection.validation.begin();
            return;
        }
        // A failed refresh cannot leave an older successful observation looking
        // like the result of the user's latest explicit inspection.
        self.archive_inspection.observation = None;
        self.archive_inspection.running = true;
        self.archive_inspection.failed = false;
    }

    pub(super) fn fail_archive_probe(&mut self, probe: &Probe) {
        if probe.validates_tools() {
            self.archive_inspection.validation.finish(true);
            return;
        }
        self.archive_inspection.running = false;
        self.archive_inspection.failed = true;
    }

    pub(super) fn finish_archive_probe(&mut self, probe: &Probe, failed: bool) {
        if self.archive_probe_is_current(probe) {
            if probe.validates_tools() {
                self.archive_inspection.validation.finish(failed);
            } else {
                self.archive_inspection.running = false;
                self.archive_inspection.failed = failed;
            }
        } else {
            self.archive_inspection = State::default();
        }
    }

    pub(super) fn receive_archive_tools(
        &mut self,
        probe: Probe,
        reply: Reply,
        cx: &mut Context<Self>,
    ) {
        if !self.archive_probe_is_current(&probe) {
            self.status = Message::new(
                "检测期间 SSH 会话已变化，未采用旧工具结果。",
                "The SSH session changed during inspection; old tool observations were discarded.",
            );
            cx.notify();
            return;
        }
        let report = match reply {
            Reply::Validation(report) if probe.validates_tools() => {
                self.receive_archive_validation(probe, report, cx);
                return;
            }
            Reply::Paths(report) if !probe.validates_tools() => report,
            _ => {
                self.fail_archive_probe(&probe);
                self.status = Message::new(
                    "工具结果与已审核的检测类型不一致，请重新审核。",
                    "The tool result did not match the reviewed check. Review a new check.",
                );
                cx.notify();
                return;
            }
        };
        self.archive_inspection.running = false;
        self.archive_inspection.failed = false;
        self.archive_inspection.observation = Some(Observation {
            probe,
            report,
            checked: Instant::now(),
        });
        self.status = Message::new(
            "服务端工具检测完成；结果只表示当前 PATH 观察，版本和实际互通仍需验证。",
            "Server tool inspection completed; this observes the current PATH, not verified versions or interoperability.",
        );
        cx.notify();
    }

    pub(super) fn archive_inspection_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let expected_token = self.session_token;
        let expected_session = self.session.clone();
        let observation = self
            .archive_inspection
            .observation
            .as_ref()
            .filter(|observation| self.archive_probe_is_current(&observation.probe));
        let summary = if self.archive_inspection.running {
            t(cx, "正在检测…", "Inspecting…").to_string()
        } else if self.archive_inspection.failed {
            t(
                cx,
                "上次检测未完成，请重新审核检测。",
                "The last inspection did not complete. Review a new inspection.",
            )
            .to_string()
        } else if let Some(observation) = observation {
            let found = observation
                .report
                .tools
                .iter()
                .filter(|tool| tool.availability == ArchiveToolAvailability::Found)
                .count();
            let seconds = observation.checked.elapsed().as_secs();
            match crate::i18n::language(cx) {
                keelshell_core::Language::ZhCn => {
                    format!("找到 {found} / 7 项 · {seconds} 秒前观察")
                }
                keelshell_core::Language::En => {
                    format!("{found} / 7 found · observed {seconds}s ago")
                }
            }
        } else {
            t(cx, "尚未检测", "Not inspected").to_string()
        };
        let mut card = div()
            .id("archive-server-tools")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .border_t_1()
            .border_color(rgb(visual.border))
            .pt_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(cx, "服务端工具", "Server tools")),
                    )
                    .child(div().text_color(rgb(visual.muted)).child(summary))
                    .child(
                        Button::new("review-archive-tool-inspection")
                            .ghost()
                            .compact()
                            .small()
                            .rounded(px(crate::design::components(cx).control_radius))
                            .label(t(cx, "审核检测", "Review inspection"))
                            .accessibility_label(t(
                                cx,
                                "审核当前 SSH 主机的归档工具检测",
                                "Review archive tool inspection on the current SSH host",
                            ))
                            .disabled(!self.can_inspect_archive_tools())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.session_token != expected_token
                                    || !view.session.as_ref().zip(expected_session.as_ref())
                                        .is_some_and(|(current, painted)| current.same_connection(painted))
                                {
                                    return;
                                }
                                view.review_archive_tools(cx);
                            })),
                    )
                    .when(observation.is_some(), |row| {
                        row.child(
                            Button::new("toggle-archive-tool-details")
                                .ghost()
                                .compact()
                                .small()
                                .label(t(cx, "工具详情", "Tool details"))
                                .accessibility_label(t(cx, "展开或收起服务端工具观察", "Expand or collapse server tool observations"))
                                .selected(self.archive_inspection.expanded)
                                .toggled(self.archive_inspection.expanded)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.archive_inspection.expanded = !view.archive_inspection.expanded;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "只观察 PATH；不运行所列工具，不代表归档或 Zmodem 已可用。",
                    "Observes PATH without running listed tools; archive or Zmodem support is not verified.",
                )),
            );
        if self.archive_inspection.expanded
            && let Some(observation) = observation
        {
            let mut rows = div()
                .id("archive-tool-observations")
                .w_full()
                .min_w_0()
                .max_h(px(112.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1();
            for (index, item) in observation.report.tools.iter().enumerate() {
                let status = match item.availability {
                    ArchiveToolAvailability::Found => t(cx, "已找到", "Found"),
                    ArchiveToolAvailability::Missing => t(cx, "未找到", "Not found"),
                    ArchiveToolAvailability::Unverified => t(cx, "待确认", "Unverified"),
                };
                rows = rows.child(
                    div()
                        .id(("archive-tool-row", index))
                        .flex()
                        .min_w_0()
                        .gap_2()
                        .child(div().w(px(58.)).flex_shrink_0().child(item.tool.name()))
                        .child(div().w(px(86.)).flex_shrink_0().child(status))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_color(rgb(visual.muted))
                                .child(item.path.clone().unwrap_or_else(|| "—".to_owned())),
                        )
                        .test_support(),
                );
            }
            card = card.child(rows.test_support());
        }
        card.child(self.archive_validation_view(cx))
            .test_support()
            .into_any_element()
    }
}
