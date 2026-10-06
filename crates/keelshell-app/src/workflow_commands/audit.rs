//! Read-only projections; history never constructs a review or execution handle.
use super::*;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use keelshell_core::{
    WorkflowAuditNotStarted as Never, WorkflowAuditOutcome as Outcome, WorkflowAuditRecord,
    WorkflowAuditTrigger, WorkflowScheduleSlotStatus, WorkflowTaskAudit,
};

fn now() -> u64 {
    // A backwards clock is allowed. The store retains insertion order rather
    // than inventing chronology or deriving execution authority from time.
    chrono::Utc::now().timestamp().max(1) as u64
}

impl WorkflowPanel {
    #[cfg(test)]
    pub(crate) fn audit_history_for_test(&self) -> Vec<(Uuid, bool)> {
        self.audit_history
            .iter()
            .map(|(record, saved)| (record.id, *saved))
            .collect()
    }
    #[cfg(test)]
    pub(crate) fn has_schedule_for_test(&self) -> bool {
        self.scheduled.is_some()
    }
    #[cfg(test)]
    pub(crate) fn audit_backpressure_for_test(&self) -> bool {
        self.audit_backpressure
    }
    #[cfg(test)]
    pub(crate) fn install_audit_ledger_for_test(
        &mut self,
        ledger: keelshell_core::WorkflowScheduleLedger,
        tasks: Vec<(Uuid, Uuid)>,
        cx: &mut Context<Self>,
    ) {
        self.scheduled = Some(super::schedule::ScheduledRun {
            ledger,
            anchor: std::time::Instant::now(),
            pending: None,
            running: None,
            audit_tasks: tasks,
            audited: HashSet::new(),
        });
        self.record_terminal_schedule_audits(cx);
        self.record_terminal_schedule_audits(cx);
    }

    pub(crate) fn set_audit_history(
        &mut self,
        saved: &[WorkflowAuditRecord],
        pending: &[WorkflowAuditRecord],
        cx: &mut Context<Self>,
    ) {
        self.audit_history = saved.iter().cloned().map(|record| (record, true)).collect();
        for record in pending {
            if let Some(entry) = self
                .audit_history
                .iter_mut()
                .find(|entry| entry.0.id == record.id)
            {
                entry.1 = false;
            } else {
                self.audit_history.push((record.clone(), false));
            }
        }
        // Reserve space for all 32 occurrences before arming another schedule.
        // Pending records remain in memory across panel hide/replacement; only
        // successful persistence removes them. A retry here never issues SSH.
        self.audit_backpressure = pending.len() > 68;
        cx.notify();
    }

    fn audit_trigger(&self) -> WorkflowAuditTrigger {
        self.scheduled
            .as_ref()
            .and_then(|run| run.running.as_ref())
            .map(|ticket| WorkflowAuditTrigger::Scheduled {
                schedule_id: ticket.schedule_id(),
                occurrence: ticket.slot_index(),
                scheduled_at: ticket.scheduled_utc_seconds(),
            })
            .unwrap_or(WorkflowAuditTrigger::Manual)
    }

    fn emit_audit(&mut self, record: WorkflowAuditRecord, cx: &mut Context<Self>) {
        if record.validate().is_err() {
            self.message = Some(Message::new(
                "任务记录校验失败，未持久化",
                "Task history failed validation and was not persisted",
            ));
            return;
        }
        if let WorkflowAuditTrigger::Scheduled { occurrence, .. } = record.trigger
            && let Some(run) = self.scheduled.as_mut()
            && !run.audited.insert(occurrence)
        {
            return;
        }
        cx.emit(WorkflowPanelEvent::Completed(record));
    }

    pub(super) fn record_completed_audit(
        &mut self,
        id: Uuid,
        receipt: Option<&WorkflowReceipt>,
        cx: &mut Context<Self>,
    ) {
        let Some(review) = self.review.as_ref() else {
            return;
        };
        let trigger = self.audit_trigger();
        let recorded_at = now();
        let record = receipt
            .and_then(|receipt| {
                receipt
                    .audit_record(&review.plan, review.options, id, recorded_at, trigger)
                    .ok()
            })
            .unwrap_or_else(|| WorkflowAuditRecord {
                id,
                recorded_at,
                trigger,
                tasks: review
                    .plan
                    .tasks()
                    .iter()
                    .map(|task| WorkflowTaskAudit {
                        id: task.id,
                        target_id: task.target_id,
                        outcome: Outcome::Unknown,
                    })
                    .collect(),
                cancelled: self.cancelling,
                stopped_after_failure: false,
            });
        self.emit_audit(record, cx);
    }

    pub(super) fn record_start_rejected_audit(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.review.as_ref() else {
            return;
        };
        let record = WorkflowAuditRecord {
            id: Uuid::new_v4(),
            recorded_at: now(),
            trigger: self.audit_trigger(),
            tasks: review
                .plan
                .tasks()
                .iter()
                .map(|task| WorkflowTaskAudit {
                    id: task.id,
                    target_id: task.target_id,
                    outcome: Outcome::NotStarted {
                        reason: Never::StartRejected,
                    },
                })
                .collect(),
            cancelled: false,
            stopped_after_failure: false,
        };
        self.emit_audit(record, cx);
    }

    pub(super) fn record_terminal_schedule_audits(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.scheduled.as_ref() else {
            return;
        };
        let mut records = Vec::new();
        for slot in run.ledger.slots() {
            if run.audited.contains(&slot.index()) {
                continue;
            }
            let outcome = match slot.status() {
                WorkflowScheduleSlotStatus::Missed => Outcome::NotStarted {
                    reason: Never::ScheduleMissed,
                },
                WorkflowScheduleSlotStatus::SkippedBusy => Outcome::NotStarted {
                    reason: Never::ScheduleBusy,
                },
                WorkflowScheduleSlotStatus::Cancelled => Outcome::Cancelled,
                WorkflowScheduleSlotStatus::Invalidated(_) => Outcome::NotStarted {
                    reason: Never::ScheduleInvalidated,
                },
                // A running/finished occurrence must be recorded from its exact
                // aggregate. Ledger status alone is not a task success receipt.
                _ => continue,
            };
            records.push(WorkflowAuditRecord {
                id: Uuid::new_v4(),
                recorded_at: now(),
                trigger: WorkflowAuditTrigger::Scheduled {
                    schedule_id: run.ledger.spec().schedule_id(),
                    occurrence: slot.index(),
                    scheduled_at: slot.scheduled_utc_seconds(),
                },
                tasks: run
                    .audit_tasks
                    .iter()
                    .map(|(id, target_id)| WorkflowTaskAudit {
                        id: *id,
                        target_id: *target_id,
                        outcome,
                    })
                    .collect(),
                cancelled: matches!(outcome, Outcome::Cancelled),
                stopped_after_failure: false,
            });
        }
        for record in records {
            self.emit_audit(record, cx);
        }
    }

    pub(super) fn audit_history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let selected = self
            .audit_selected
            .and_then(|id| self.audit_history.iter().find(|entry| entry.0.id == id))
            .or_else(|| self.audit_history.last());
        let mut list = div()
            .id("workflow-audit-list")
            .test_support()
            .flex_shrink_0()
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        if self.audit_history.is_empty() {
            list = list.child(t(
                cx,
                "尚无任务记录。完成工作流后在此查看。",
                "No task history yet. Completed workflows appear here.",
            ));
        }
        for (index, (record, saved)) in self.audit_history.iter().rev().enumerate() {
            let when = i64::try_from(record.recorded_at)
                .ok()
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| record.recorded_at.to_string());
            let origin = match record.trigger {
                WorkflowAuditTrigger::Manual => t(cx, "手动工作流", "Manual workflow").to_string(),
                WorkflowAuditTrigger::Scheduled {
                    occurrence,
                    scheduled_at,
                    ..
                } => format!(
                    "{} #{} · epoch {}",
                    t(cx, "定时触发", "Scheduled occurrence"),
                    occurrence + 1,
                    scheduled_at
                ),
            };
            let id = record.id;
            list = list.child(
                div()
                    .id(("workflow-audit-run", index))
                    .test_support()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .text_sm()
                            .child(format!(
                                "{origin} · {when} · {} {} · {}",
                                record.tasks.len(),
                                t(cx, "任务", "tasks"),
                                if *saved {
                                    t(cx, "已保存", "Saved")
                                } else {
                                    t(cx, "未保存", "Unsaved")
                                }
                            )),
                    )
                    .child(
                        Button::new(("workflow-audit-select", index))
                            .ghost()
                            .small()
                            .label(t(cx, "查看结果", "View results"))
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.audit_selected = Some(id);
                                cx.notify();
                            })),
                    ),
            );
        }
        let mut detail = div()
            .id("workflow-audit-details")
            .test_support()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        if let Some((record, _)) = selected {
            detail = detail.child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                "{} {}",
                t(cx, "运行 ID", "Run ID"),
                record.id
            )));
            for (index, task) in record.tasks.iter().enumerate() {
                let (label, color) = outcome_label(task.outcome, cx);
                detail = detail.child(
                    div()
                        .id(("workflow-audit-task", index))
                        .test_support()
                        .min_w_0()
                        .whitespace_normal()
                        .text_sm()
                        .text_color(rgb(color))
                        .child(format!(
                            "#{} · {} · {} {} · {label}",
                            index + 1,
                            task.id,
                            t(cx, "目标 ID", "Target ID"),
                            task.target_id
                        )),
                );
            }
        }
        div().id("workflow-audit-history").test_support().role(Role::Dialog).aria_label(t(cx,"任务记录","Task history")).size_full().flex().flex_col().track_focus(&self.focus).text_color(rgb(visual.text))
            .child(div().p_3().border_b_1().border_color(rgb(visual.border)).child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(t(cx,"任务记录","Task history")))
                .child(div().min_w_0().whitespace_normal().text_sm().child(t(cx,"只读结果；不包含命令、输出、地址、参数或摘要。重启仅载入记录，不恢复任务。最多 100 次运行及累计 2048 个任务，按写入顺序保留。","Read-only results without commands, output, addresses, parameters or digests. Restart loads history only; it never restores tasks. Up to 100 runs and 2048 tasks are retained in insertion order."))))
            .child(div().flex_1().min_h_0().p_3().flex().flex_col().gap_3().child(list).child(detail))
            .child(div().p_3().border_t_1().border_color(rgb(visual.border)).flex().gap_2()
                .child(Button::new("workflow-audit-back").ghost().label(t(cx,"返回工作流","Back to workflow")).on_click(cx.listener(|panel,_,_,cx|{panel.show_audit_history=false;cx.notify();})))
                .child(Button::new("workflow-audit-save").ghost().disabled(!self.audit_history.iter().any(|entry|!entry.1)).label(t(cx,"重试保存记录","Retry saving history")).on_click(cx.listener(|_,_,_,cx|cx.emit(WorkflowPanelEvent::RetryAuditSave))))
                .child(Button::new("workflow-audit-hide").ghost().label(t(cx,"关闭","Close")).on_click(cx.listener(|_,_,_,cx|cx.emit(WorkflowPanelEvent::Hide)))))
            .into_any_element()
    }
}

pub(super) fn outcome_label(outcome: Outcome, cx: &App) -> (String, u32) {
    let visual = crate::design::palette(cx);
    match outcome {
        Outcome::Succeeded => (
            t(cx, "成功 · exit 0", "Succeeded · exit 0").into(),
            visual.success,
        ),
        Outcome::Failed { exit_code } => (
            format!("{} · exit {exit_code}", t(cx, "失败", "Failed")),
            visual.danger,
        ),
        Outcome::Rejected => (
            t(cx, "明确拒绝", "Explicitly rejected").into(),
            visual.danger,
        ),
        Outcome::Unknown => (t(cx, "结果未知", "Outcome unknown").into(), visual.warning),
        Outcome::Cancelled => (
            t(cx, "未放行 · 已取消", "Not admitted · cancelled").into(),
            visual.muted,
        ),
        Outcome::DependencyBlocked { dependency } => (
            format!(
                "{} {dependency}",
                t(cx, "依赖阻断 · 前置", "Dependency blocked · prerequisite")
            ),
            visual.muted,
        ),
        Outcome::StoppedAfterFailure => (
            t(cx, "未放行 · 失败策略停止", "Not admitted · failure policy").into(),
            visual.muted,
        ),
        Outcome::NotStarted { reason } => (
            t(
                cx,
                match reason {
                    Never::AdmissionRejected => "未放行 · 通道拒绝",
                    Never::SessionUnavailable => "未放行 · 会话不可用",
                    Never::StartRejected => "未放行 · 启动复核未通过",
                    Never::Deadline => "未放行 · 截止时间",
                    Never::WorkerFailed => "未放行 · 调度器失败",
                    Never::ScheduleMissed => "未放行 · 定时过期",
                    Never::ScheduleBusy => "未放行 · 前次占用",
                    Never::ScheduleInvalidated => "未放行 · 定时授权失效",
                },
                match reason {
                    Never::AdmissionRejected => "Not admitted · channel rejected",
                    Never::SessionUnavailable => "Not admitted · session unavailable",
                    Never::StartRejected => "Not admitted · start review rejected",
                    Never::Deadline => "Not admitted · deadline",
                    Never::WorkerFailed => "Not admitted · worker failed",
                    Never::ScheduleMissed => "Not admitted · occurrence expired",
                    Never::ScheduleBusy => "Not admitted · previous occurrence busy",
                    Never::ScheduleInvalidated => "Not admitted · schedule invalidated",
                },
            )
            .into(),
            visual.muted,
        ),
    }
}
