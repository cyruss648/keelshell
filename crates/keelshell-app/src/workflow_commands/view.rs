use super::*;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
    },
    prelude::FluentBuilder,
};
use keelshell_core::BatchTaskSkipReason;
use keelshell_session::BatchOutcome;

fn hint(cx: &App, text: impl Into<SharedString>) -> Div {
    div()
        .flex_shrink_0()
        .text_xs()
        .text_color(rgb(crate::design::palette(cx).muted))
        .child(text.into())
}
fn command(text: &str) -> Div {
    div()
        .min_w_0()
        .whitespace_normal()
        .font_family("monospace")
        .child(crate::command_text::visible_command(text))
}

impl WorkflowPanel {
    pub(super) fn status(&self, id: Uuid, cx: &App) -> (String, u32) {
        let visual = crate::design::palette(cx);
        let row = self.progress.iter().find(|row| row.id == id);
        if let Some(receipt) = row.and_then(|row| row.receipt.as_ref()) {
            return match &receipt.result {
                WorkflowTaskResult::Transport { row } => (
                    crate::batch_commands::outcome(&row.outcome, cx),
                    match row.outcome {
                        BatchOutcome::Exited { code: 0 } => visual.success,
                        BatchOutcome::Exited { .. } | BatchOutcome::Rejected => visual.danger,
                        BatchOutcome::Unknown { .. } => visual.warning,
                        BatchOutcome::NotStarted { .. } => visual.muted,
                    },
                ),
                WorkflowTaskResult::Skipped { reason } => (
                    match reason {
                        BatchTaskSkipReason::Cancelled => {
                            t(cx, "未放行 · 已取消", "Not admitted · cancelled").into()
                        }
                        BatchTaskSkipReason::StoppedAfterFailure => {
                            t(cx, "未放行 · 失败策略停止", "Not admitted · failure policy").into()
                        }
                        BatchTaskSkipReason::DependencyNotSucceeded { dependency } => format!(
                            "{} {}",
                            t(
                                cx,
                                "未放行 · 前置未成功",
                                "Not admitted · prerequisite did not succeed"
                            ),
                            self.label(*dependency, cx)
                        ),
                    },
                    visual.muted,
                ),
            };
        }
        if self.complete {
            return (
                t(
                    cx,
                    "结果未知 · 缺少回执",
                    "Outcome unknown · missing receipt",
                )
                .into(),
                visual.warning,
            );
        }
        if row.is_some_and(|row| row.admitted) {
            return (
                t(
                    cx,
                    "已放行 · 等待 SSH 回执",
                    "Admitted · awaiting SSH receipt",
                )
                .into(),
                visual.accent,
            );
        }
        let waiting: Vec<_> = self.review.as_ref().and_then(|review| review.plan.tasks().iter().find(|task| task.id == id))
            .map(|task| task.dependencies.iter().filter(|dependency| !self.progress.iter().any(|row| row.id == **dependency
                && row.receipt.as_ref().is_some_and(|receipt| matches!(&receipt.result,
                    WorkflowTaskResult::Transport { row } if matches!(row.outcome, BatchOutcome::Exited { code: 0 })))))
                .map(|dependency| self.label(*dependency, cx)).collect()).unwrap_or_default();
        if !waiting.is_empty() {
            (
                format!(
                    "{} {}",
                    t(cx, "等待前置成功：", "Awaiting prerequisite success:"),
                    waiting.join(", ")
                ),
                visual.muted,
            )
        } else {
            (
                t(cx, "等待并发槽位", "Awaiting concurrency slot").into(),
                visual.muted,
            )
        }
    }

    fn task_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let editable = self.editable();
        let mut list = div()
            .id("workflow-task-list")
            .test_support()
            .flex_shrink_0()
            .min_h_0()
            .max_h(px(190.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (index, task) in self.tasks.iter().enumerate() {
            let id = task.id;
            let target = task.target.and_then(|id| {
                self.targets
                    .iter()
                    .find(|row| row.connected.destination.id == id)
            });
            let label = self.label(id, cx);
            let target_label = target
                .map(|row| {
                    crate::command_text::visible_command(&format!(
                        "{} · {}",
                        row.connected.destination.name, row.connected.destination.endpoint
                    ))
                })
                .unwrap_or_else(|| t(cx, "尚未选择 SSH 目标", "SSH target not selected").into());
            let (status, color) = self.status(id, cx);
            list = list.child(
                div()
                    .id(("workflow-task-row", index))
                    .test_support()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(visual.border))
                    .bg(rgb(if self.selected == Some(id) {
                        visual.selected
                    } else {
                        visual.surface
                    }))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label.clone()),
                            )
                            .child(
                                Button::new(("workflow-task-select", index))
                                    .accessibility_label(format!(
                                        "{} {} · {}",
                                        if editable {
                                            t(cx, "编辑任务", "Edit task")
                                        } else {
                                            t(cx, "任务输出", "Task output")
                                        },
                                        label,
                                        target_label
                                    ))
                                    .ghost()
                                    .compact()
                                    .label(if editable {
                                        t(cx, "编辑任务", "Edit task")
                                    } else {
                                        t(cx, "任务输出", "Task output")
                                    })
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.selected = Some(id);
                                        panel.detail = Some(id);
                                        panel.refresh_detail(cx);
                                        cx.notify();
                                    })),
                            )
                            .when(editable, |el| {
                                el.child(
                                    Button::new(("workflow-task-remove", index))
                                        .accessibility_label(format!(
                                            "{} {}",
                                            t(cx, "删除任务", "Remove task"),
                                            label
                                        ))
                                        .ghost()
                                        .compact()
                                        .label(t(cx, "删除任务", "Remove task"))
                                        .on_click(cx.listener(move |panel, _, _, cx| {
                                            panel.remove_task(id, cx)
                                        })),
                                )
                            }),
                    )
                    .child(hint(cx, target_label))
                    .when(!editable, |el| {
                        el.child(div().text_xs().text_color(rgb(color)).child(status))
                    }),
            );
        }
        list.into_any_element()
    }

    fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(task) = self
            .selected
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
        else {
            return hint(
                cx,
                t(
                    cx,
                    "添加任务后选择命令、目标和前置任务。",
                    "Add a task, then choose its command, target and prerequisites.",
                ),
            )
            .into_any_element();
        };
        let task_id = task.id;
        let mut targets = div()
            .id("workflow-target-list")
            .test_support()
            .flex_shrink_0()
            .min_h_0()
            .max_h(px(170.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (index, row) in self.targets.iter().enumerate() {
            let target = row.connected.destination.id;
            let selected = task.target == Some(target);
            targets = targets.child(
                div()
                    .id(("workflow-target-row", index))
                    .test_support()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(visual.border))
                    .bg(rgb(if selected {
                        visual.selected
                    } else {
                        visual.surface
                    }))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Button::new(("workflow-target", index))
                            .ghost()
                            .small()
                            .label(format!(
                                "{} {} · {}",
                                if selected { "☑" } else { "☐" },
                                row.connected.destination.name,
                                row.connected.destination.endpoint
                            ))
                            .disabled(!row.available || row.connected.session.is_closed())
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                if panel.editable()
                                    && panel.targets.iter().any(|row| {
                                        row.connected.destination.id == target
                                            && row.available
                                            && !row.connected.session.is_closed()
                                    })
                                    && let Some(task) =
                                        panel.tasks.iter_mut().find(|task| task.id == task_id)
                                {
                                    task.target = Some(target);
                                    panel.changed(cx);
                                }
                            })),
                    )
                    .child(hint(
                        cx,
                        crate::command_text::visible_command(&row.connected.destination.route),
                    ))
                    .when(!row.available, |el| {
                        el.child(hint(
                            cx,
                            t(
                                cx,
                                "已失效：请选择在线目标",
                                "Unavailable: select a connected target",
                            ),
                        ))
                    }),
            );
        }
        let mut dependencies = div()
            .id("workflow-dependency-list")
            .test_support()
            .flex_shrink_0()
            .min_h_0()
            .max_h(px(160.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (index, other) in self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, other)| other.id != task_id)
        {
            let dependency = other.id;
            let selected = task.dependencies.contains(&dependency);
            dependencies = dependencies.child(
                Button::new(("workflow-dependency", index))
                    .ghost()
                    .small()
                    .label(format!(
                        "{} {}",
                        if selected { "☑" } else { "☐" },
                        self.label(dependency, cx)
                    ))
                    .disabled(!selected && task.dependencies.len() >= 32)
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if panel.editable()
                            && let Some(task) =
                                panel.tasks.iter_mut().find(|task| task.id == task_id)
                        {
                            if task.dependencies.contains(&dependency) {
                                task.dependencies.retain(|id| *id != dependency);
                            } else if task.dependencies.len() < 32 {
                                task.dependencies.push(dependency);
                            }
                            panel.changed(cx);
                        }
                    })),
            );
        }
        div().id("workflow-editor").test_support().flex_shrink_0().flex().flex_col().gap_2().min_w_0()
            .child(hint(cx,self.label(task_id,cx)))
            .child(hint(cx,t(cx,"任务名称（可选）","Task name (optional)")))
            .child(Input::new(&task.name).id("workflow-task-name").aria_label(t(cx,"任务名称","Task name")))
            .child(hint(cx,t(cx,"命令原文。可使用 {{name}}、{{host}}、{{port}}、{{user}}、{{endpoint}}；审核显示完整展开。",
                "Exact command. Supports {{name}}, {{host}}, {{port}}, {{user}}, {{endpoint}}; review shows the complete expansion.")))
            .child(div().id("workflow-command-container").test_support().h(px(145.)).flex_shrink_0().child(Textarea::new(&task.command).aria_label(t(cx,"此任务的完整命令草稿","Complete command draft for this task")).h_full()))
            .child(hint(cx,t(cx,"此任务的已认证 SSH 目标","Authenticated SSH target for this task")))
            .child(targets)
            .child(Button::new("workflow-refresh-targets").ghost().small().label(t(cx,"刷新已连接会话","Refresh connected sessions"))
                .on_click(cx.listener(|panel,_,_,cx| {if panel.editable(){cx.emit(WorkflowPanelEvent::RefreshTargets);}})))
            .child(hint(cx,t(cx,"前置任务：每项必须明确成功才放行，最多 32 项","Prerequisites: every task must explicitly succeed; up to 32")))
            .child(dependencies).into_any_element()
    }

    fn review_view(&self, review: &WorkflowReview, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut list=div().id("workflow-review-tasks").test_support().flex_shrink_0().flex().flex_col().gap_3().min_w_0()
            .child(hint(cx,format!("{} · {} {} · {} {}s · {} · {}",t(cx,"完整拓扑审核","Complete topological review"),
                t(cx,"并发","Concurrency"),review.options.concurrency,t(cx,"单任务超时","Per-task timeout"),review.options.timeout.as_secs(),
                if review.options.policy==BatchPolicy::StopAfterFailure {t(cx,"失败后停止等待任务","Stop pending tasks after failure")}else{t(cx,"继续独立分支","Continue independent branches")},
                t(cx,"每任务合计输出上限 256 KiB","Combined output limit: 256 KiB per task"))))
            .child(hint(cx,format!("{} {}",t(cx,"计划指纹","Plan fingerprint"),review.plan.review_token().hex())))
            .child(hint(cx,t(cx,"确认会执行以下所有精确命令。正文完整显示，隐形字符以转义显示；命令不会进入历史或配置。",
                "Confirmation executes every exact command below. Text is complete; invisible characters are escaped. Commands do not enter history or configuration.")));
        for (index, task) in review.plan.tasks().iter().enumerate() {
            let destination = review
                .destinations
                .iter()
                .find(|destination| destination.id == task.target_id);
            let source = review
                .sources
                .iter()
                .find(|(id, _)| *id == task.id)
                .map(|(_, source)| source.as_str())
                .unwrap_or_default();
            let deps = task
                .dependencies
                .iter()
                .map(|id| self.label(*id, cx))
                .collect::<Vec<_>>()
                .join(", ");
            let mut item = div()
                .id(("workflow-reviewed-task", index))
                .test_support()
                .p_3()
                .rounded_md()
                .bg(rgb(visual.canvas))
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.label(task.id, cx)),
                )
                .child(hint(
                    cx,
                    format!(
                        "{} {}",
                        t(cx, "前置", "Prerequisites"),
                        if deps.is_empty() {
                            t(cx, "无", "None")
                        } else {
                            &deps
                        }
                    ),
                ))
                .child(hint(cx, t(cx, "模板源", "Template source")))
                .child(command(source))
                .child(hint(
                    cx,
                    t(cx, "将发送的完整命令", "Complete command to send"),
                ))
                .child(
                    div()
                        .id(("workflow-reviewed-command", index))
                        .test_support()
                        .child(command(&task.command)),
                );
            if let Some(target) = destination {
                item = item
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(
                        crate::command_text::visible_command(&format!(
                            "{} · {}",
                            target.name, target.endpoint
                        )),
                    ))
                    .child(hint(
                        cx,
                        crate::command_text::visible_command(&target.route),
                    ))
                    .child(hint(
                        cx,
                        format!("{} {}", t(cx, "本次会话绑定", "Session binding"), target.id),
                    ));
            }
            list = list.child(item);
        }
        list.into_any_element()
    }
}

impl Render for WorkflowPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let editable = self.editable();
        let done = self
            .progress
            .iter()
            .filter(|row| row.receipt.is_some())
            .count();
        let title = if self.complete {
            t(cx, "依赖工作流结果", "Dependency workflow results")
        } else if self.is_running() {
            t(cx, "依赖工作流运行中", "Dependency workflow running")
        } else if self.review.is_some() {
            t(cx, "审核依赖工作流", "Review dependency workflow")
        } else {
            t(cx, "编辑依赖工作流", "Edit dependency workflow")
        };
        div().id("workflow-panel").role(Role::Dialog).aria_label(title).test_support().size_full().flex().flex_col().min_w_0().track_focus(&self.focus)
            .text_color(rgb(visual.text))
            .child(div().id("workflow-header").test_support().p_3().flex_shrink_0().border_b_1().border_color(rgb(visual.border))
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(hint(cx,format!("{} {} / 128 · {} {done}/{}",t(cx,"任务","Tasks"),self.tasks.len(),t(cx,"终态回执","Terminal receipts"),self.tasks.len()))))
            .child(div().id("workflow-body").test_support().flex_1().min_h_0().min_w_0().overflow_y_scroll().p_3().flex().flex_col().gap_3()
                .child(self.task_list(cx))
                .when(editable,|el|el.child(Button::new("workflow-add-task").ghost().small().label(t(cx,"添加任务","Add task"))
                    .disabled(self.tasks.len()>=128).on_click(cx.listener(|panel,_,window,cx|panel.add_task(String::new(),window,cx))))
                    .child(div().flex_shrink_0().flex().flex_wrap().gap_3()
                        .child(div().w(px(140.)).child(hint(cx,t(cx,"并发 1–8","Concurrency 1–8"))).child(Input::new(&self.concurrency).id("workflow-concurrency").aria_label(t(cx,"工作流并发数 1 至 8","Workflow concurrency from 1 to 8"))))
                        .child(div().w(px(160.)).child(hint(cx,t(cx,"单任务超时（秒）","Per-task timeout (s)"))).child(Input::new(&self.timeout).id("workflow-timeout").aria_label(t(cx,"每任务超时秒数 1 至 300","Per-task timeout in seconds from 1 to 300")))))
                    .child(Button::new("workflow-failure-policy").ghost().small().label(if self.stop_after_failure {t(cx,"失败后停止等待任务","Stop pending tasks after failure")}else{t(cx,"失败后继续独立分支","Continue independent branches")})
                        .on_click(cx.listener(|panel,_,_,cx|{if panel.editable(){panel.stop_after_failure = !panel.stop_after_failure;panel.changed(cx);}})))
                    .child(self.editor(cx)))
                .when_some(self.review.as_ref(),|el,review|el.child(self.review_view(review,cx)))
                .when(self.handle.is_some()||self.complete,|el|el.child(hint(cx,t(cx,"逐任务输出保留本次捕获的完整 256 KiB 范围，控制字符以转义显示；可选择上方任务查看。",
                    "Task output retains the complete captured 256 KiB bound, with escaped controls. Select a task above to inspect it.")))
                    .child(div().id("workflow-output").test_support().min_h(px(100.)).flex_shrink_0().p_3().bg(rgb(visual.canvas)).font_family("monospace").whitespace_normal().child(self.detail_text.clone())))
                .child(hint(cx,t(cx,"工作流和输出仅在内存。只执行当前已审核会话，不重连、重试或回放。取消停止等待任务并取消本地等待，不能证明远端进程停止。",
                    "Workflow and output stay in memory. Only reviewed sessions are used; no reconnect, retry or replay. Cancellation stops pending admission and local waits; it cannot prove remote process termination.")))
                .when_some(self.message.as_ref(),|el,message|el.child(div().id("workflow-message").test_support().text_color(rgb(visual.warning)).child(message.render(cx)))))
            .child(div().id("workflow-footer").test_support().flex_shrink_0().p_3().border_t_1().border_color(rgb(visual.border)).flex().flex_wrap().justify_end().gap_2()
                .child(Button::new("workflow-hide").ghost().label(t(cx,"返回工作区","Back to workspace")).on_click(cx.listener(|_,_,_,cx|cx.emit(WorkflowPanelEvent::Hide))))
                .when(self.handle.is_some(),|el|el.child(Button::new("workflow-cancel").label(if self.cancelling{t(cx,"正在取消本地等待…","Cancelling local waits…")}else{t(cx,"取消工作流","Cancel workflow")})
                    .disabled(self.cancelling).on_click(cx.listener(|panel,_,_,cx|panel.cancel(cx)))))
                .when(self.review.is_some()&&!self.is_running()&&!self.complete,|el|el.child(Button::new("workflow-back").ghost().label(t(cx,"返回修改","Edit plan"))
                    .on_click(cx.listener(|panel,_,_,cx|panel.back(cx))))
                    .child(Button::new("workflow-confirm").primary().label(t(cx,"确认全部任务并执行","Confirm all tasks and execute"))
                        .disabled(!self.review.as_ref().is_some_and(|review|self.review_current(review,cx)))
                        .on_click(cx.listener(|panel,_,_,cx|panel.confirm(cx)))))
                .when(editable,|el|el.child(Button::new("workflow-review-button").primary().label(t(cx,"下一步：完整审核","Next: complete review"))
                    .on_click(cx.listener(|panel,_,_,cx|panel.prepare(cx)))))
                .when(!self.is_running(),|el|el.child(Button::new("workflow-new").ghost().label(t(cx,"新建工作流","New workflow"))
                    .on_click(cx.listener(|_,_,_,cx|cx.emit(WorkflowPanelEvent::New)))))
                .when(self.detail.is_some()&&!self.detail_text.is_empty(),|el|el.child(Button::new("workflow-copy-output").ghost().label(t(cx,"复制此任务输出","Copy task output"))
                    .on_click(cx.listener(|panel,_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(panel.detail_text.to_string())))))))
    }
}
