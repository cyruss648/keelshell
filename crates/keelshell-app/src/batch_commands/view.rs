use super::*;
use crate::design::{ACCENT, BORDER, CANVAS, MUTED, SELECTED, SURFACE, TEXT};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
    },
    prelude::FluentBuilder,
};
use keelshell_session::{BatchNotStartedReason, BatchOutcome, BatchUnknownReason};

fn hint(text: impl Into<SharedString>) -> Div {
    div().text_xs().text_color(rgb(MUTED)).child(text.into())
}

fn outcome(outcome: &BatchOutcome, cx: &App) -> String {
    match outcome {
        BatchOutcome::Exited { code } => format!(
            "{} · {code}",
            if *code == 0 {
                t(cx, "成功", "Succeeded")
            } else {
                t(cx, "失败", "Failed")
            }
        ),
        BatchOutcome::Rejected => t(cx, "服务器拒绝执行", "Server rejected exec").into(),
        BatchOutcome::NotStarted { reason } => format!(
            "{} · {}",
            t(cx, "未开始", "Not started"),
            match reason {
                BatchNotStartedReason::Cancelled => t(cx, "已取消", "Cancelled"),
                BatchNotStartedReason::StoppedAfterFailure =>
                    t(cx, "前序失败后停止", "Stopped after failure"),
                BatchNotStartedReason::Timeout => t(cx, "打开通道超时", "Channel open timed out"),
                BatchNotStartedReason::ChannelRejected => t(cx, "通道被拒绝", "Channel rejected"),
                BatchNotStartedReason::ConnectionLost => t(cx, "连接已断开", "Connection lost"),
                BatchNotStartedReason::WorkerFailed => t(cx, "执行器异常", "Worker failed"),
            }
        ),
        BatchOutcome::Unknown { reason } => format!(
            "{} · {}",
            t(cx, "结果未知", "Outcome unknown"),
            match reason {
                BatchUnknownReason::Cancelled => t(cx, "执行中取消", "Cancelled after starting"),
                BatchUnknownReason::Timeout => t(cx, "超时", "Timed out"),
                BatchUnknownReason::ConnectionLost => t(cx, "连接已断开", "Connection lost"),
                BatchUnknownReason::NoExitStatus => t(cx, "无退出状态", "No exit status"),
                BatchUnknownReason::OutputLimit => t(cx, "输出超限", "Output limit"),
                BatchUnknownReason::RemoteSignal => t(cx, "收到远端信号", "Remote signal"),
                BatchUnknownReason::Protocol => t(cx, "协议异常", "Protocol error"),
                BatchUnknownReason::WorkerFailed => t(cx, "执行器异常", "Worker failed"),
            }
        ),
    }
}

impl BatchPanel {
    fn destination_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let editable = self.editable();
        let mut list = div()
            .id("batch-destinations")
            .test_support()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .min_w_0();
        for (index, row) in self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| editable || row.selected)
        {
            let id = row.destination.id;
            let status = if let Some(receipt) = &row.receipt {
                outcome(&receipt.outcome, cx)
            } else if self.complete {
                t(
                    cx,
                    "结果未知 · 缺少回执",
                    "Outcome unknown · missing receipt",
                )
                .into()
            } else if row.started {
                t(
                    cx,
                    "执行中 · 结束后显示输出",
                    "Running · output shown when finished",
                )
                .into()
            } else if !row.available {
                t(cx, "已断开", "Disconnected").into()
            } else if self.handle.is_some() {
                t(cx, "等待并发槽位", "Queued").into()
            } else {
                t(cx, "已连接", "Connected").into()
            };
            let mut item = div()
                .id(("batch-target-row", index))
                .test_support()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(if self.detail == Some(id) {
                    SELECTED
                } else {
                    SURFACE
                }))
                .flex()
                .flex_col()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(editable, |el| {
                            el.child(
                                Button::new(("batch-select", index))
                                    .ghost()
                                    .compact()
                                    .label(if row.selected { "☑" } else { "☐" })
                                    .disabled(!row.available)
                                    .tooltip(t(cx, "选择此 SSH 会话", "Select this SSH session"))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        if panel.editable()
                                            && let Some(row) = panel.rows.get_mut(index)
                                            && row.available
                                        {
                                            row.selected = !row.selected;
                                            cx.notify();
                                        }
                                    })),
                            )
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(row.destination.endpoint.clone()),
                        )
                        .when(!editable, |el| {
                            el.child(
                                Button::new(("batch-detail", index))
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "输出", "Output"))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.detail = Some(id);
                                        panel.refresh_detail(cx);
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .child(hint(row.destination.name.clone()))
                .child(hint(row.destination.route.clone()));
            if !editable {
                item = item.child(
                    div()
                        .text_xs()
                        .text_color(rgb(
                            if row.receipt.as_ref().is_some_and(|r| {
                                matches!(r.outcome, BatchOutcome::Exited { code: 0 })
                            }) {
                                ACCENT
                            } else {
                                MUTED
                            },
                        ))
                        .child(status),
                );
            } else if !row.available {
                item = item.child(hint(status));
            }
            list = list.child(item);
        }
        if self.rows.is_empty() {
            list = list.child(hint(t(
                cx,
                "尚无已连接会话。先连接所需 SSH 主机，再新建批量任务。",
                "No connected sessions. Connect the required SSH hosts, then create a new batch.",
            )));
        }
        list.into_any_element()
    }
}

impl Render for BatchPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editable = self.editable();
        let selected = self.rows.iter().filter(|r| r.selected).count();
        let done = self.rows.iter().filter(|r| r.receipt.is_some()).count();
        let title = if self.complete {
            t(cx, "批量执行结果", "Batch results")
        } else if self.is_running() {
            t(cx, "批量执行中", "Batch running")
        } else if self.review.is_some() {
            t(cx, "审核批量命令", "Review batch command")
        } else {
            t(cx, "新建批量任务", "New batch task")
        };
        div().id("batch-panel").test_support().track_focus(&self.focus).key_context("BatchPanel").size_full().min_w_0().min_h_0().flex().flex_col().overflow_hidden().bg(rgb(SURFACE)).text_color(rgb(TEXT)).text_sm()
            .child(div().flex_shrink_0().p_3().border_b_1().border_color(rgb(BORDER)).flex().flex_col().gap_1()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(hint(t(cx,"独立 SSH 命令环境 · 不继承终端 cwd、别名或临时环境变量。","Independent SSH exec environment · terminal cwd, aliases and temporary variables are not inherited."))))
            .child(div().id("batch-body").test_support().flex_1().min_w_0().min_h_0().overflow_y_scroll().p_3().flex().flex_col().gap_3()
                .child(hint(format!("{} {selected} / 32",t(cx,"目标会话","Target sessions"))))
                .child(self.destination_list(cx))
                .when(editable,|el|el.child(div().flex_shrink_0().flex().flex_col().gap_2()
                    .child(hint(t(cx,"命令正文 · 不自动保存历史","Command · never added to terminal history")))
                    .child(hint(t(cx,"可按目标展开元数据：{{name}}、{{host}}、{{port}}、{{user}}、{{endpoint}}；下一步审核时逐目标显示最终命令。","Use per-target metadata markers: {{name}}, {{host}}, {{port}}, {{user}}, {{endpoint}}; the next review shows each final command.")))
                    .child(div().id("batch-command-container").test_support().h(px(156.)).flex_shrink_0().child(Textarea::new(&self.command).h_full()))
                    .child(div().flex().flex_wrap().gap_3()
                        .child(div().w(px(140.)).child(hint(t(cx,"并发数 1–8","Concurrency 1–8"))).child(Input::new(&self.concurrency).id("batch-concurrency")))
                        .child(div().w(px(160.)).child(hint(t(cx,"单主机超时（秒）","Per-host timeout (s)"))).child(Input::new(&self.timeout).id("batch-timeout"))))
                    .child(Button::new("batch-failure-policy").ghost().small().label(if self.stop_after_failure {t(cx,"失败后停止后续任务","Stop pending work after failure")} else {t(cx,"失败后继续其他任务","Continue other work after failure")}).on_click(cx.listener(|panel,_,_,cx|{if panel.editable(){panel.stop_after_failure = !panel.stop_after_failure;cx.notify();}})))))
                .when_some(self.review.as_ref(),|el,review| el.child(div().id("batch-review").test_support().flex_shrink_0().min_w_0().p_3().rounded_md().bg(rgb(CANVAS)).flex().flex_col().gap_2()
                    .child(hint(format!("{} {} · {} {}s · {}",t(cx,"并发","Concurrency"),review.concurrency,t(cx,"单主机超时","Per-host timeout"),review.timeout_seconds,if review.stop_after_failure {t(cx,"失败后停止等待项","Stop pending on failure")} else {t(cx,"失败后继续","Continue on failure")})))
                    .child(hint(t(cx,"模板源（仅支持 {{name}}、{{host}}、{{port}}、{{user}}、{{endpoint}}；以下为每个目标的最终命令）","Template source (supports only {{name}}, {{host}}, {{port}}, {{user}}, {{endpoint}}; final command per target follows)")))
                    .child(div().id("batch-reviewed-command").test_support().font_family("monospace").child(review.command.clone()))
                    .child({
                        let mut rendered = div().id("batch-reviewed-target-commands").test_support().flex().flex_col().gap_2().max_h(px(320.));
                        for (index, (id, command)) in review.commands.iter().enumerate() {
                            let label = self.rows.iter().find(|row| row.destination.id == *id).map(|row| row.destination.endpoint.clone()).unwrap_or_else(|| id.to_string());
                            rendered = rendered.child(div().id(("batch-reviewed-target-command", index)).test_support().min_w_0().p_2().rounded_md().bg(rgb(SURFACE)).child(hint(label)).child(div().font_family("monospace").child(command.clone())));
                        }
                        rendered
                    })))
                .when(self.handle.is_some()||self.complete,|el|el.child(hint(format!("{} {done}/{selected} · {}",t(cx,"已返回回执","Receipts"),t(cx,"每主机合计输出上限 1 MiB；预览分流显示前 64 KiB。","Combined output limit: 1 MiB per host; each stream preview shows its first 64 KiB."))))
                    .child(div().id("batch-output").test_support().p_3().min_h(px(100.)).flex_shrink_0().bg(rgb(CANVAS)).font_family("monospace").child(self.detail_text.clone())))
                .child(hint(t(cx,"结果仅保留在本次工作区。取消不能保证远端进程已停止；未知结果不会自动重试。","Results stay in this workspace only. Cancellation cannot confirm remote process termination; unknown outcomes are never retried.")))
                .when_some(self.message.as_ref(),|el,message|el.child(div().id("batch-message").test_support().text_color(rgb(0xb14c2c)).child(message.render(cx)))))
            .child(div().id("batch-footer").test_support().flex_shrink_0().p_3().border_t_1().border_color(rgb(BORDER)).flex().flex_wrap().justify_end().gap_2()
                .child(Button::new("batch-hide").ghost().label(t(cx,"返回工作区","Back to workspace")).on_click(cx.listener(|_,_,_,cx|cx.emit(BatchPanelEvent::Hide))))
                .when(self.handle.is_some(),|el|el.child(Button::new("batch-cancel").label(if self.cancelling {t(cx,"正在停止…","Stopping…")} else {t(cx,"停止任务","Stop batch")}).disabled(self.cancelling).on_click(cx.listener(|panel,_,_,cx|panel.cancel(cx)))))
                .when(self.review.is_some()&&!self.is_running()&&!self.complete,|el|el.child(Button::new("batch-back").ghost().label(t(cx,"返回修改","Edit plan")).on_click(cx.listener(|panel,_,_,cx|panel.back(cx))))
                    .child(Button::new("batch-confirm").primary().label(t(cx,"确认执行","Confirm execution")).disabled(!self.review.as_ref().is_some_and(|r|self.review_current(r,cx))).on_click(cx.listener(|panel,_,_,cx|panel.confirm(cx)))))
                .when(editable,|el|el.child(Button::new("batch-review-button").primary().label(t(cx,"下一步：审核","Next: review")).on_click(cx.listener(|panel,_,_,cx|panel.prepare(cx)))))
                .when(!self.is_running(),|el|el.child(Button::new("batch-new").ghost().label(t(cx,"新建任务","New task")).on_click(cx.listener(|_,_,_,cx|cx.emit(BatchPanelEvent::New)))))
                .when(self.detail.is_some(),|el|el.child(Button::new("batch-copy-output").ghost().label(t(cx,"复制输出预览","Copy output preview")).on_click(cx.listener(|panel,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(panel.detail_text.to_string())))))))
    }
}
