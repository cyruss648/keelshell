//! In-memory reviewed triggers; no timer owns reconnection or command authority.
use gpui_kit::{
    component::{
        Selectable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
    },
    prelude::FluentBuilder,
};
use std::time::Instant;

use keelshell_core::{
    ScheduleClockSample, WorkflowScheduleBinding, WorkflowScheduleDueToken,
    WorkflowScheduleInvalidationReason, WorkflowScheduleLedger, WorkflowScheduleOutcome,
    WorkflowScheduleSlotStatus, WorkflowScheduleSpec, format_fixed_offset,
    format_fixed_offset_datetime, parse_fixed_offset, parse_fixed_offset_datetime,
};

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Immediate,
    Once,
    Interval,
}

/// Raw schedule input is part of the immutable human review, including a
/// numerically equivalent programmatic edit that emits no InputEvent.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ScheduleReviewInputs {
    mode: Mode,
    values: [String; 5],
}

pub(super) struct ScheduleDraft {
    mode: Mode,
    start: Entity<InputState>,
    offset: Entity<InputState>,
    grace: Entity<InputState>,
    interval: Entity<InputState>,
    count: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl ScheduleDraft {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkflowPanel>) -> Self {
        // Use an explicit UTC default without loading OS time-zone files on
        // the UI thread. The user can choose any acknowledged fixed offset.
        let offset_minutes = 0;
        let start = input(
            &format_fixed_offset_datetime(chrono::Utc::now().timestamp() + 300, offset_minutes)
                .unwrap_or_default(),
            window,
            cx,
        );
        let offset = input(
            &format_fixed_offset(offset_minutes).unwrap_or_default(),
            window,
            cx,
        );
        let grace = input("15", window, cx);
        let interval = input("300", window, cx);
        let count = input("2", window, cx);
        let subscriptions = [&start, &offset, &grace, &interval, &count]
            .into_iter()
            .map(|field| {
                cx.subscribe(field, |panel, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        panel.changed(cx);
                    }
                })
            })
            .collect();
        Self {
            mode: Mode::Immediate,
            start,
            offset,
            grace,
            interval,
            count,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn set_disabled(&self, disabled: bool, cx: &mut Context<WorkflowPanel>) {
        for field in [
            &self.start,
            &self.offset,
            &self.grace,
            &self.interval,
            &self.count,
        ] {
            field.update(cx, |field, cx| field.set_disabled(disabled, cx));
        }
    }

    pub(super) fn review_inputs(&self, cx: &App) -> ScheduleReviewInputs {
        ScheduleReviewInputs {
            mode: self.mode,
            values: [
                &self.start,
                &self.offset,
                &self.grace,
                &self.interval,
                &self.count,
            ]
            .map(|field| field.read(cx).value().to_string()),
        }
    }
}

pub(super) struct ScheduledRun {
    ledger: WorkflowScheduleLedger,
    anchor: Instant,
    pending: Option<WorkflowScheduleDueToken>,
    running: Option<WorkflowScheduleDueToken>,
}

fn sample(anchor: Instant) -> ScheduleClockSample {
    ScheduleClockSample {
        utc_millis: chrono::Utc::now().timestamp_millis(),
        monotonic: anchor.elapsed(),
    }
}

fn hint(cx: &App, value: impl Into<SharedString>) -> AnyElement {
    div()
        .text_sm()
        .text_color(rgb(crate::design::palette(cx).muted))
        .whitespace_normal()
        .child(value.into())
        .into_any_element()
}

fn slot_label(status: WorkflowScheduleSlotStatus, cx: &App) -> &'static str {
    match status {
        WorkflowScheduleSlotStatus::Pending => t(cx, "等待触发", "Waiting"),
        WorkflowScheduleSlotStatus::Due => t(cx, "正在复核授权", "Validating authorization"),
        WorkflowScheduleSlotStatus::Running => t(cx, "运行中", "Running"),
        WorkflowScheduleSlotStatus::Finished(WorkflowScheduleOutcome::Succeeded) => {
            t(cx, "已明确成功", "Confirmed success")
        }
        WorkflowScheduleSlotStatus::Finished(WorkflowScheduleOutcome::Failed) => {
            t(cx, "失败", "Failed")
        }
        WorkflowScheduleSlotStatus::Finished(WorkflowScheduleOutcome::Unknown) => {
            t(cx, "结果未知", "Unknown outcome")
        }
        WorkflowScheduleSlotStatus::Finished(WorkflowScheduleOutcome::Cancelled)
        | WorkflowScheduleSlotStatus::Cancelled => t(cx, "已取消", "Cancelled"),
        WorkflowScheduleSlotStatus::Missed => t(cx, "已过期，未补跑", "Expired; no catch-up"),
        WorkflowScheduleSlotStatus::SkippedBusy => t(
            cx,
            "前次占用，已跳过",
            "Skipped while previous work was busy",
        ),
        WorkflowScheduleSlotStatus::Invalidated(_) => {
            t(cx, "授权已失效", "Authorization invalidated")
        }
    }
}

fn scheduled_time(spec: &WorkflowScheduleSpec, seconds: i64) -> String {
    format_fixed_offset_datetime(seconds, spec.fixed_offset_minutes()).unwrap_or_default()
}

impl WorkflowPanel {
    pub(super) fn schedule_snapshot(
        &self,
        token: Uuid,
        plan: &BatchWorkflowPlan,
        cx: &App,
    ) -> Result<Option<WorkflowScheduleSpec>, Message> {
        let draft = &self.schedule_draft;
        // Bound every raw field before it is copied into the immutable review,
        // including inactive fields retained when switching execution modes.
        if [
            &draft.start,
            &draft.offset,
            &draft.grace,
            &draft.interval,
            &draft.count,
        ]
        .iter()
        .any(|field| field.read(cx).value().len() > 32)
        {
            return Err(Message::new(
                "定时字段最多 32 字节，请缩短输入",
                "Schedule fields are limited to 32 bytes; shorten the input",
            ));
        }
        if draft.mode == Mode::Immediate {
            return Ok(None);
        }
        let parse = || {
            let offset = parse_fixed_offset(draft.offset.read(cx).value().as_str())?;
            let start = parse_fixed_offset_datetime(draft.start.read(cx).value().as_str(), offset)?;
            let decimal = |field: &Entity<InputState>| {
                !field.read(cx).value().is_empty()
                    && field
                        .read(cx)
                        .value()
                        .bytes()
                        .all(|byte| byte.is_ascii_digit())
            };
            if !decimal(&draft.grace) {
                return Err(keelshell_core::WorkflowScheduleError::InvalidGrace);
            }
            if draft.mode == Mode::Interval && !decimal(&draft.interval) {
                return Err(keelshell_core::WorkflowScheduleError::InvalidInterval);
            }
            if draft.mode == Mode::Interval && !decimal(&draft.count) {
                return Err(keelshell_core::WorkflowScheduleError::InvalidCount);
            }
            let grace = draft
                .grace
                .read(cx)
                .value()
                .parse::<u32>()
                .map_err(|_| keelshell_core::WorkflowScheduleError::InvalidGrace)?;
            let interval = (draft.mode == Mode::Interval)
                .then(|| draft.interval.read(cx).value().parse::<u64>().ok())
                .flatten();
            let count = if draft.mode == Mode::Interval {
                draft
                    .count
                    .read(cx)
                    .value()
                    .parse::<u32>()
                    .map_err(|_| keelshell_core::WorkflowScheduleError::InvalidCount)?
            } else {
                1
            };
            if draft.mode == Mode::Interval && interval.is_none() {
                return Err(keelshell_core::WorkflowScheduleError::InvalidInterval);
            }
            WorkflowScheduleSpec::new(
                token,
                WorkflowScheduleBinding::new(token, self.revision, plan.review_token())?,
                start,
                offset,
                grace,
                interval,
                count,
            )
        };
        parse().map(Some).map_err(|error| Message::detail(
            "定时配置无效：日期 YYYY-MM-DD HH:MM:SS；偏移 ±HH:MM；宽限 1–60 秒；间隔至少 60 秒；1–32 次且七天内结束",
            "Invalid schedule: YYYY-MM-DD HH:MM:SS; offset ±HH:MM; grace 1–60 s; interval ≥60 s; 1–32 occurrences within seven days",
            error,
        ))
    }

    #[cfg(test)]
    pub(crate) fn schedule_grace_for_test(&self) -> Entity<InputState> {
        self.schedule_draft.grace.clone()
    }

    #[cfg(test)]
    pub(crate) fn schedule_status_for_test(&self) -> Vec<WorkflowScheduleSlotStatus> {
        self.scheduled
            .as_ref()
            .map(|run| {
                run.ledger
                    .slots()
                    .iter()
                    .map(|slot| slot.status())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn schedule_active(&self) -> bool {
        self.scheduled
            .as_ref()
            .is_some_and(|run| !run.ledger.is_terminal())
    }

    /// Only a consumed timer ticket can bypass the visible-panel requirement.
    pub(crate) fn scheduled_due(&self, review: &WorkflowReview) -> bool {
        self.scheduled
            .as_ref()
            .is_some_and(|run| run.pending.is_some())
            && self.starting
            && self.review.as_ref() == Some(review)
    }

    pub(super) fn admit_schedule(
        &mut self,
        review: &WorkflowReview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(spec) = review.schedule.as_ref() else {
            return true;
        };
        if self.scheduled.is_none() {
            let anchor = Instant::now();
            match WorkflowScheduleLedger::new(spec.clone(), sample(anchor)) {
                Ok(ledger) => {
                    self.scheduled = Some(ScheduledRun {
                        ledger,
                        anchor,
                        pending: None,
                        running: None,
                    });
                    self.starting = false;
                    self.message = Some(Message::new(
                        "定时计划已启用；到时再次核对原认证连接。关闭应用将取消，错过不补跑。",
                        "Schedule armed; the original authenticated connection is checked again when due. Closing the app cancels it; missed occurrences never catch up.",
                    ));
                    self._schedule_poll = Some(cx.spawn_in(window, async move |this, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(250))
                                .await;
                            if !this
                                .update_in(cx, |panel, _, cx| panel.tick_schedule(cx))
                                .unwrap_or(false)
                            {
                                break;
                            }
                        }
                    }));
                    cx.notify();
                }
                Err(error) => {
                    self.starting = false;
                    self.message = Some(Message::detail(
                        "定时计划未启用，请返回修改并审核",
                        "Schedule was not armed; edit and review again",
                        error,
                    ));
                    cx.notify();
                }
            }
            return false;
        }
        let Some(run) = self.scheduled.as_mut() else {
            return false;
        };
        let Some(ticket) = run.pending.take() else {
            self.starting = false;
            return false;
        };
        if let Err(error) = run.ledger.claim(&ticket, sample(run.anchor)) {
            self.starting = false;
            self.complete = run.ledger.is_terminal() && self.handle.is_none();
            self.message = Some(Message::detail(
                "本次触发已过期或失效，未执行",
                "Occurrence expired or invalidated; nothing executed",
                error,
            ));
            cx.notify();
            return false;
        }
        run.running = Some(ticket);
        self.complete = false;
        self.cancelling = false;
        self.progress.clear();
        self.detail = None;
        self.detail_text = "".into();
        true
    }

    pub(super) fn tick_schedule(&mut self, cx: &mut Context<Self>) -> bool {
        // InputState::set_value does not emit Change. While waiting, recheck
        // the same bounded immutable review instead of waiting until its due
        // instant. This performs no I/O and never creates execution authority.
        if self.handle.is_none()
            && !self.complete
            && self.schedule_active()
            && self
                .review
                .as_ref()
                .is_some_and(|review| !self.review_current(review, cx))
        {
            self.fail_start(cx);
            return false;
        }
        let Some(run) = self.scheduled.as_mut() else {
            return false;
        };
        if run.ledger.is_terminal() {
            self.complete = self.handle.is_none();
            return false;
        }
        match run.ledger.tick(sample(run.anchor)) {
            Ok(Some(ticket)) => {
                run.pending = Some(ticket);
                let Some(review) = self.review.clone() else {
                    self.stop_schedule(
                        WorkflowScheduleInvalidationReason::SessionBindingChanged,
                        cx,
                    );
                    return false;
                };
                self.starting = true;
                cx.emit(WorkflowPanelEvent::Start(Box::new(review)));
            }
            Ok(None) => {}
            Err(error) => {
                self.message = Some(Message::detail(
                    "定时计划失效，未补跑；新计划需要重新审核",
                    "Schedule invalidated without catch-up; review a new plan",
                    error,
                ));
            }
        }
        if !self.schedule_active() && self.handle.is_none() {
            self.complete = true;
        }
        cx.notify();
        self.schedule_active()
    }

    pub(super) fn finish_schedule(&mut self, outcome: WorkflowScheduleOutcome) -> bool {
        let Some(run) = self.scheduled.as_mut() else {
            return true;
        };
        if let Some(ticket) = run.running.take()
            && let Err(error) = run.ledger.finish(&ticket, outcome)
        {
            self.message = Some(Message::detail(
                "定时回执不匹配，重复计划停止",
                "Schedule receipt mismatch; repetition stopped",
                error,
            ));
            run.ledger
                .invalidate(WorkflowScheduleInvalidationReason::PriorRunNotSucceeded);
        }
        run.ledger.is_terminal()
    }

    pub(super) fn cancel_schedule(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = self.scheduled.as_mut() {
            run.ledger.cancel();
            run.pending = None;
            self._schedule_poll = None;
            self.starting = false;
            self.complete = self.handle.is_none();
            self.message = Some(Message::new(
                "已取消未来触发；在途任务只取消本地等待，不保证远端终止。",
                "Future triggers cancelled. For in-flight work, only local waits are cancelled; remote termination is not guaranteed.",
            ));
            cx.notify();
        }
    }

    pub(super) fn stop_schedule(
        &mut self,
        reason: WorkflowScheduleInvalidationReason,
        cx: &mut Context<Self>,
    ) {
        if let Some(run) = self.scheduled.as_mut() {
            run.ledger.invalidate(reason);
            run.pending = None;
            self._schedule_poll = None;
            self.starting = false;
            self.complete = self.handle.is_none();
            cx.notify();
        }
    }

    pub(super) fn schedule_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut modes = div().flex().flex_wrap().gap_2();
        for (mode, id, zh, en) in [
            (
                Mode::Immediate,
                "workflow-schedule-now",
                "立即执行",
                "Run now",
            ),
            (
                Mode::Once,
                "workflow-schedule-once",
                "一次定时",
                "Schedule once",
            ),
            (
                Mode::Interval,
                "workflow-schedule-interval",
                "有限间隔重复",
                "Bounded interval",
            ),
        ] {
            modes = modes.child(
                Button::new(id)
                    .ghost()
                    .small()
                    .label(t(cx, zh, en))
                    .selected(self.schedule_draft.mode == mode)
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if panel.editable() {
                            panel.schedule_draft.mode = mode;
                            panel.changed(cx);
                        }
                    })),
            );
        }
        let field = |entity: &Entity<InputState>,
                     id: &'static str,
                     zh: &'static str,
                     en: &'static str,
                     width: f32| {
            div()
                .w(px(width))
                .max_w_full()
                .child(hint(cx, t(cx, zh, en)))
                .child(Input::new(entity).id(id).aria_label(t(cx, zh, en)))
        };
        div().id("workflow-schedule-editor").test_support().flex_shrink_0().p_3().rounded_md().bg(rgb(visual.canvas)).flex().flex_col().gap_2()
            .child(hint(cx, t(cx, "执行时间 · 默认立即执行", "Execution time · immediate by default")))
            .child(modes)
            .when(self.schedule_draft.mode != Mode::Immediate, |el| el.child(div().flex().flex_wrap().gap_2()
                .child(field(&self.schedule_draft.start, "workflow-schedule-start", "首次日期时间", "First date/time", 250.))
                .child(field(&self.schedule_draft.offset, "workflow-schedule-offset", "固定 UTC 偏移", "Fixed UTC offset", 160.))
                .child(field(&self.schedule_draft.grace, "workflow-schedule-grace", "过期宽限秒数 1–60", "Expiry grace 1–60 s", 170.)))
                .when(self.schedule_draft.mode == Mode::Interval, |el| el.child(div().flex().flex_wrap().gap_2()
                    .child(field(&self.schedule_draft.interval, "workflow-schedule-period", "重复间隔秒数 ≥60", "Repeat interval ≥60 s", 180.))
                    .child(field(&self.schedule_draft.count, "workflow-schedule-count", "触发次数 1–32", "Occurrences 1–32", 180.))))
                .child(hint(cx,t(cx,"日期格式 YYYY-MM-DD HH:MM:SS；UTC 偏移 ±HH:MM（不随夏令时变化）。首次须在未来七天内，末次加宽限也在七天内。关闭应用不恢复，过期/忙碌不补跑，失败或未知停止重复。",
                    "YYYY-MM-DD HH:MM:SS; UTC offset ±HH:MM (no daylight-saving changes). First trigger and final expiry are within seven days. No restoration after exit or catch-up for expired/busy slots; failure/unknown stops repetition."))))
            .into_any_element()
    }

    pub(super) fn schedule_review_view(&self, spec: &WorkflowScheduleSpec, cx: &App) -> AnyElement {
        let mut list = div().id("workflow-schedule-review").test_support().flex().flex_col().gap_2()
            .child(hint(cx, format!("{} {} · UTC{} · {} {}s", t(cx,"定时授权","Scheduled authorization"), spec.schedule_id(), format_fixed_offset(spec.fixed_offset_minutes()).unwrap_or_default(), t(cx,"每次过期宽限","Per-occurrence expiry grace"), spec.grace_seconds())))
            .child(hint(cx,t(cx,"以下每次触发均授权同一完整任务/目标/原认证连接与执行选项。无重连、补跑、重叠或自动重试；失败/未知停止后续。只保留最近一次输出，计划与触发摘要只在内存。",
                "Each occurrence authorizes the same complete tasks, targets, original authenticated connections and options. No reconnect, catch-up, overlap or retry; failure/unknown stops future work. Only latest output is retained; plan/slot summaries remain in memory.")));
        for index in 0..spec.count() {
            if let Some(seconds) = spec.scheduled_utc_seconds(index) {
                list = list.child(hint(
                    cx,
                    format!(
                        "#{} · {} · {} UTC · epoch {}",
                        index + 1,
                        scheduled_time(spec, seconds),
                        format_fixed_offset_datetime(seconds, 0).unwrap_or_default(),
                        seconds
                    ),
                ));
            }
        }
        list.child(hint(
            cx,
            format!(
                "{} {} · {} UTC · epoch {}",
                t(cx, "最终结束时间", "Final expiry"),
                scheduled_time(spec, spec.final_window_end_utc_seconds()),
                format_fixed_offset_datetime(spec.final_window_end_utc_seconds(), 0)
                    .unwrap_or_default(),
                spec.final_window_end_utc_seconds()
            ),
        ))
        .into_any_element()
    }

    pub(super) fn schedule_status_view(&self, cx: &App) -> AnyElement {
        let Some(run) = self.scheduled.as_ref() else {
            return div().into_any_element();
        };
        let spec = run.ledger.spec();
        let mut view = div()
            .id("workflow-schedule-status")
            .test_support()
            .flex_shrink_0()
            .p_3()
            .rounded_md()
            .bg(rgb(crate::design::palette(cx).canvas))
            .flex()
            .flex_col()
            .gap_2()
            .child(hint(
                cx,
                t(
                    cx,
                    "定时触发记录 · 无补跑 · 原连接授权",
                    "Scheduled occurrences · no catch-up · original connection authority",
                ),
            ));
        for slot in run.ledger.slots() {
            view = view.child(hint(
                cx,
                format!(
                    "#{} · {} · {}",
                    slot.index() + 1,
                    scheduled_time(spec, slot.scheduled_utc_seconds()),
                    slot_label(slot.status(), cx)
                ),
            ));
        }
        view.into_any_element()
    }
}
