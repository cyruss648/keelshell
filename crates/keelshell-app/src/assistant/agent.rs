//! Explicit model requests and desktop-reviewed actions for one captured SSH target.
use super::*;
use gpui_kit::prelude::FluentBuilder as _;
use keelshell_ai::{AgentAction, AgentLimits, AgentOutcome, AgentPhase, AgentRun, AgentTarget};

#[derive(Default)]
pub(super) struct AgentPanelState {
    pub(super) mode: bool,
    pub(super) run: Option<AgentRun>,
    limits: AgentLimits,
    pub(super) target_admitted: bool,
    pub(super) backend: Option<(Uuid, RequestCancellation)>,
    profile_label: String,
    question: Zeroizing<String>,
    pub(super) round: Option<u8>,
    original_file: Option<String>,
    file_preparing: bool,
    step_scrolls: std::collections::HashMap<Uuid, ScrollHandle>,
    original_scroll: ScrollHandle,
}

impl AssistantPanel {
    #[cfg(test)]
    pub(crate) fn agent_prompt_for_test(&self) -> Entity<TextareaState> {
        self.prompt.clone()
    }
    #[cfg(test)]
    pub(crate) fn agent_profile_for_test(&self) -> Option<NamedAiProfile> {
        self.profile.clone()
    }
    /// Ignore context selection queued before the user started another run.
    pub(crate) fn agent_capture_current(&self, previous_run: Option<Uuid>) -> bool {
        self.agent.run.as_ref().map(AgentRun::id) == previous_run
    }
    /// Admit a queued start only while the panel still requests that exact run.
    pub(crate) fn agent_target_requested(&self, run_id: Uuid) -> bool {
        self.agent.mode
            && !self.agent.target_admitted
            && self.agent.backend.is_none()
            && self
                .agent
                .run
                .as_ref()
                .is_some_and(|run| run.id() == run_id && !run.phase().is_terminal())
    }
    #[cfg(test)]
    pub(crate) fn agent_snapshot_for_test(&self) -> Option<(Uuid, AgentPhase, usize, u8)> {
        self.agent
            .run
            .as_ref()
            .map(|r| (r.id(), r.phase(), r.steps().len(), r.rounds_used()))
    }
    #[cfg(test)]
    pub(crate) fn agent_last_outcome_for_test(&self) -> Option<AgentOutcome> {
        self.agent.run.as_ref()?.steps().last()?.outcome().cloned()
    }
    #[cfg(test)]
    pub(crate) fn set_agent_question_for_test(
        &mut self,
        question: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_request(cx);
        self.prompt
            .update(cx, |p, cx| p.set_value(question, window, cx));
    }
    #[cfg(test)]
    pub(crate) fn agent_file_review_ready_for_test(&self) -> bool {
        self.agent.original_file.is_some()
    }
    #[cfg(test)]
    pub(crate) fn reset_agent_step_scroll_for_test(&self) {
        if let Some(run) = &self.agent.run
            && let Some(step) = run.steps().first()
            && let Some(scroll) = self.agent.step_scrolls.get(&step.id())
        {
            scroll.set_offset(point(px(0.), px(0.)));
        }
    }
    #[cfg(test)]
    pub(crate) fn agent_step_offset_for_test(&self) -> Option<Point<Pixels>> {
        let step = self.agent.run.as_ref()?.steps().first()?;
        Some(self.agent.step_scrolls.get(&step.id())?.offset())
    }
    #[cfg(test)]
    pub(crate) fn agent_status_for_test(&self, cx: &App) -> String {
        self.status.render(cx)
    }
    #[cfg(test)]
    pub(crate) fn agent_preview_for_test(&self) -> Option<String> {
        self.prepared.as_ref().map(|p| p.preview_json().to_owned())
    }
    pub(super) fn agent_active(&self) -> bool {
        self.agent.mode
            && self
                .agent
                .run
                .as_ref()
                .is_some_and(|run| !run.phase().is_terminal())
    }
    pub(super) fn obsolete_agent_question_change(&self, cx: &App) -> bool {
        self.agent_active() && self.agent.question.as_str() == self.prompt.read(cx).value().as_ref()
    }
    pub(super) fn agent_prepare_disabled(&self) -> bool {
        self.agent.mode
            && self.agent.run.as_ref().is_some_and(|run| {
                !matches!(run.phase(), AgentPhase::Ready) && !run.phase().is_terminal()
            })
    }
    fn set_agent_mode(&mut self, mode: bool, cx: &mut Context<Self>) {
        if self.agent.mode == mode {
            return;
        }
        self.invalidate_request(cx);
        self.agent = AgentPanelState {
            mode,
            ..Default::default()
        };
        self.status = Message::new(
            "模式已切换；请核对目标、配置和完整请求。",
            "Mode changed; review the target, profile and complete request.",
        );
        cx.notify();
    }
    pub(super) fn prepare_agent_question(&mut self, cx: &mut Context<Self>) -> Option<String> {
        if !self.agent.mode {
            return Some(self.prompt.read(cx).value().to_string());
        }
        if self
            .agent
            .run
            .as_ref()
            .is_none_or(|run| run.phase().is_terminal())
        {
            self.stop_agent(cx);
            let result =
                AgentTarget::new(self.host.clone(), self.session_id.clone()).and_then(|target| {
                    AgentRun::new(
                        target,
                        self.prompt.read(cx).value().to_string(),
                        self.context.clone(),
                        self.agent.limits,
                    )
                });
            match result {
                Ok(run) => {
                    cx.emit(AssistantEvent::AgentStart {
                        run_id: run.id(),
                        session_id: run.target().session_id().to_owned(),
                    });
                    let source = self
                        .profile
                        .as_ref()
                        .map(|profile| format!("{} · {}", profile.name, profile.model))
                        .unwrap_or_default();
                    let mut secrets = self.credentials.all_secrets();
                    if let Some(key) = &self.key {
                        secrets.push(key.as_str());
                    }
                    self.agent.profile_label =
                        keelshell_ai::Redactor::new(&secrets).redact(&source).0;
                    self.agent.question = Zeroizing::new(self.prompt.read(cx).value().to_string());
                    self.agent.run = Some(run);
                    self.agent.target_admitted = false;
                    self.agent.original_file = None;
                    self.agent.step_scrolls.clear();
                    // Kit Change is queued and programmatic set_value is silent.
                    // Editing requires synchronous revocation before unlocking.
                    self.prompt
                        .update(cx, |input, cx| input.set_readonly(true, cx));
                }
                Err(_) => {
                    self.status = Message::new(
                        "Agent 需要明确选择真实 SSH 目标及有效问题。",
                        "Agent requires an explicitly selected real SSH target and a valid question.",
                    );
                    cx.notify();
                    return None;
                }
            }
        }
        let Some(run) = &mut self.agent.run else {
            return None;
        };
        match run.next_prompt() {
            Ok(prompt) => Some(prompt),
            Err(_) => {
                let run_id = run.id();
                self.revoke_agent_backend(run_id, cx);
                cx.emit(AssistantEvent::AgentStop { run_id });
                self.status = Message::new(
                    "无法继续：状态或回合／动作／上下文预算已到达限制。",
                    "Cannot continue: the phase or round/action/context allowance prevents another request.",
                );
                cx.notify();
                None
            }
        }
    }
    /// Bind a cancellation grant to the exact run; obsolete grants cancel themselves.
    pub(crate) fn agent_target_accepted(
        &mut self,
        run_id: Uuid,
        cancellation: Option<RequestCancellation>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .agent
            .run
            .as_ref()
            .is_some_and(|run| run.id() == run_id && !run.phase().is_terminal())
        {
            if let Some(cancellation) = cancellation {
                cancellation.cancel();
            }
            return;
        }
        let accepted = cancellation
            .as_ref()
            .is_some_and(|token| !token.is_cancelled());
        self.revoke_agent_backend(run_id, cx);
        self.agent.backend = cancellation.map(|token| (run_id, token));
        self.agent.target_admitted = accepted;
        if accepted {
            self.prompt
                .update(cx, |input, cx| input.set_readonly(true, cx));
        } else {
            self.lose_agent_target(cx);
        }
        cx.notify();
    }
    pub(super) fn begin_agent_round(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.agent.mode {
            return true;
        }
        if !self.agent.target_admitted {
            self.status = Message::new(
                "捕获的 SSH 目标未就绪或已失效；请重新选择。",
                "The captured SSH target is unavailable or stale; select it again.",
            );
            cx.notify();
            return false;
        }
        match self.agent.run.as_mut().map(AgentRun::begin_round) {
            Some(Ok(round)) => {
                self.agent.round = Some(round);
                true
            }
            _ => {
                self.status = Message::new(
                    "此回合未准入；请检查步骤和预算。",
                    "This round was not admitted; check the steps and allowances.",
                );
                cx.notify();
                false
            }
        }
    }
    pub(super) fn accept_agent_reply(&mut self, cx: &mut Context<Self>) {
        let Some(run) = &mut self.agent.run else {
            return;
        };
        let Some(round) = self.agent.round.take() else {
            return;
        };
        let id = run.id();
        match run.receive_decision(id, round, &self.response) {
            Ok(()) => {
                self.agent.original_file = None;
                self.agent.file_preparing = false;
                if let Some(step) = run.steps().last() {
                    self.agent
                        .step_scrolls
                        .insert(step.id(), ScrollHandle::new());
                }
                if let Some(step) = run.pending_step() {
                    cx.emit(AssistantEvent::AgentProposed {
                        run_id: id,
                        action_id: step.id(),
                        action: step.decision().action.clone(),
                    });
                    self.status = Message::new(
                        "本回合已提出操作；请逐字审核并批准或拒绝。尚未执行。",
                        "This round proposed an action. Review its exact content, then approve or reject. Nothing has run.",
                    );
                } else {
                    self.revoke_agent_backend(id, cx);
                    cx.emit(AssistantEvent::AgentStop { run_id: id });
                    self.status = Message::new(
                        "Agent 已结束；结论来自模型，请结合已确认的步骤结果判断。",
                        "Agent finished. Its conclusion is model text; assess it against confirmed step results.",
                    );
                }
            }
            Err(_) => {
                self.revoke_agent_backend(id, cx);
                cx.emit(AssistantEvent::AgentStop { run_id: id });
                self.status = Message::new(
                    "模型未返回有效受限决策，或预算已耗尽；已停止，无操作获批。",
                    "The model returned an invalid restricted decision or exhausted a budget. Stopped; no action was approved.",
                );
            }
        }
    }
    /// Revoke this run's backend grant before any queued owner notification.
    pub(super) fn revoke_agent_backend(&mut self, run_id: Uuid, cx: &mut Context<Self>) {
        if self
            .agent
            .backend
            .as_ref()
            .is_some_and(|(id, _)| *id == run_id)
            && let Some((_, cancellation)) = self.agent.backend.take()
        {
            cancellation.cancel();
        }
        if self
            .agent
            .run
            .as_ref()
            .is_some_and(|run| run.id() == run_id)
        {
            self.prompt
                .update(cx, |input, cx| input.set_readonly(false, cx));
        }
    }
    pub(super) fn stop_agent(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &mut self.agent.run {
            let run_id = run.id();
            let active = !run.phase().is_terminal();
            if active {
                run.stop();
            }
            self.revoke_agent_backend(run_id, cx);
            if active {
                cx.emit(AssistantEvent::AgentStop { run_id });
            }
        }
        self.agent.round = None;
        self.agent.original_file = None;
        self.agent.file_preparing = false;
    }
    pub(super) fn lose_agent_target(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &mut self.agent.run {
            run.lose_target();
            let run_id = run.id();
            self.revoke_agent_backend(run_id, cx);
            cx.emit(AssistantEvent::AgentStop { run_id });
        }
        self.agent.target_admitted = false;
        self.agent.original_file = None;
        self.agent.file_preparing = false;
    }
    fn edit_agent_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.invalidate_request(cx);
        self.question_anchor.scroll_to(window, cx);
        self.prompt.update(cx, |input, cx| input.focus(window, cx));
        self.status = Message::new(
            "已停止旧 Agent，可以编辑问题；下一次需要重新审核。",
            "The previous Agent stopped. Edit the question and review again before starting.",
        );
        cx.notify();
    }
    pub(super) fn agent_question_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().when(self.agent_active(), |view| {
            view.child(
                Button::new("assistant-edit-agent-question")
                    .ghost()
                    .label(t(cx, "编辑问题并停止", "Stop and edit question"))
                    .on_click(
                        cx.listener(|panel, _, window, cx| panel.edit_agent_question(window, cx)),
                    ),
            )
        })
    }
    fn stop_agent_button(&mut self, cx: &mut Context<Self>) {
        self.stop_agent(cx);
        self.request_revision = self.request_revision.wrapping_add(1);
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self._job = None;
        self.busy = false;
        self.prepared = None;
        self.prepared_key = None;
        self.preview = false;
        self.status = Message::new(
            "已停止此 Agent。已发送操作可能仍有远程影响；未知结果不会重试。",
            "This Agent stopped. Dispatched operations may still have remote effects; unknown outcomes are never retried.",
        );
        cx.notify();
    }
    fn review_agent_action(&mut self, approve: bool, cx: &mut Context<Self>) {
        let Some(run) = &mut self.agent.run else {
            return;
        };
        if run.phase() != AgentPhase::AwaitingAction
            || self.agent.file_preparing
            || !self.agent.target_admitted
        {
            return;
        }
        let Some(step) = run.pending_step() else {
            return;
        };
        let id = step.id();
        let run_id = run.id();
        let action = step.decision().action.clone();
        if !approve {
            if run
                .action_result(run_id, id, AgentOutcome::Rejected)
                .is_ok()
            {
                cx.emit(AssistantEvent::AgentActionRejected {
                    run_id,
                    action_id: id,
                });
                self.status = Message::new(
                    "已拒绝，没有执行；下一回合发送前需审核拒绝记录。",
                    "Rejected without execution; review this record before sending another round.",
                );
            }
        } else if matches!(action, AgentAction::WriteFile { .. })
            && self.agent.original_file.is_none()
        {
            self.agent.file_preparing = true;
            cx.emit(AssistantEvent::AgentPrepareFile {
                run_id,
                action_id: id,
                action,
            });
            self.status = Message::new(
                "正在读取原文件用于审核；替换尚未批准。",
                "Reading the original file for review; replacement has not been approved.",
            );
        } else if let Ok(action) = run.approve_action(id) {
            cx.emit(AssistantEvent::AgentExecute {
                run_id,
                action_id: id,
                action,
            });
            self.status = Message::new(
                "已批准这一个操作；正在等待确认结果。",
                "This single action was approved; awaiting a confirmed result.",
            );
        }
        cx.notify();
    }
    /// Receive a captured-target file snapshot for human review, never inference.
    pub(crate) fn agent_file_prepared(
        &mut self,
        run_id: Uuid,
        action_id: Uuid,
        original: Result<String, ()>,
        cx: &mut Context<Self>,
    ) {
        let Some(run) = &mut self.agent.run else {
            return;
        };
        if run.id() != run_id
            || run.phase() != AgentPhase::AwaitingAction
            || !run.pending_step().is_some_and(|s| s.id() == action_id)
        {
            return;
        }
        self.agent.file_preparing = false;
        match original {
            Ok(text) => {
                self.agent.original_file = Some(text);
                self.status = Message::new(
                    "原文已读回；请完整审核原文与替换内容，再批准写入。",
                    "Original content retrieved. Review the complete original and replacement before approving the write.",
                );
            }
            Err(()) => {
                let _ = run.action_result(
                    run_id,
                    action_id,
                    AgentOutcome::Failed {
                        reason: "Original regular UTF-8 file could not be prepared for review"
                            .into(),
                    },
                );
                self.status = Message::new(
                    "原文准备失败；未写入，需审核结果后再决定下一回合。",
                    "Original preparation failed. No write occurred; review the result before choosing another round.",
                );
            }
        }
        cx.notify();
    }
    /// Admit a desktop-owned operation outcome only for the same run/action.
    pub(crate) fn agent_action_finished(
        &mut self,
        run_id: Uuid,
        action_id: Uuid,
        mut result: AgentOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(run) = &mut self.agent.run else {
            return;
        };
        if run.id() != run_id
            || !run
                .pending_step()
                .is_some_and(|step| step.id() == action_id)
        {
            return;
        }
        if let AgentOutcome::Completed { output, .. } = &mut result {
            *output = keelshell_ai::Redactor::new(&self.credentials.all_secrets())
                .redact(output)
                .0;
        }
        if run.action_result(run_id, action_id, result).is_err() {
            self.status = Message::new(
                "操作结果无法准入；此 Agent 已停止，请核查步骤记录。",
                "The action result could not be admitted; this Agent stopped. Check its step record.",
            );
            cx.notify();
            return;
        }
        self.agent.original_file = None;
        self.agent.file_preparing = false;
        self.status = Message::new(
            "已记录桌面确认结果。下一回合不会自动发送，请审核完整上下文。",
            "Desktop result recorded. The next round is not sent automatically; review its complete context.",
        );
        cx.notify();
    }
    pub(crate) fn agent_target_lost(&mut self, run_id: Uuid, cx: &mut Context<Self>) {
        if self.agent.run.as_ref().is_some_and(|r| r.id() == run_id) {
            self.lose_agent_target(cx);
            self.request_revision = self.request_revision.wrapping_add(1);
            if let Some(cancel) = self.cancellation.take() {
                cancel.cancel();
            }
            self._job = None;
            self.busy = false;
            self.prepared = None;
            self.prepared_key = None;
            self.preview = false;
            self.status = Message::new(
                "捕获的会话／路线／信任已失效；此 Agent 已停止。已发起操作的结果可能未知。",
                "The captured session/route/trust is stale. This Agent stopped; dispatched action outcomes may be unknown.",
            );
            cx.notify();
        }
    }
    pub(super) fn agent_status_headline(&self, cx: &App) -> String {
        self.agent
            .run
            .as_ref()
            .map(|run| phase_message(run.phase()).render(cx))
            .unwrap_or_else(|| self.status.render(cx))
    }
    pub(super) fn agent_stop_button(&self, cx: &mut Context<Self>) -> AnyElement {
        Button::new("assistant-stop-agent")
            .ghost()
            .label(t(cx, "停止 Agent", "Stop Agent"))
            .on_click(cx.listener(|panel, _, _, cx| panel.stop_agent_button(cx)))
            .into_any_element()
    }
    pub(super) fn agent_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div().id("assistant-mode-controls").test_support().flex().flex_col().gap_2()
            .child(div().flex().gap_2()
                .child(Button::new("assistant-mode-ask").ghost().label("Ask").disabled(!self.agent.mode).on_click(cx.listener(|p, _, _, cx| p.set_agent_mode(false, cx))))
                .child(Button::new("assistant-mode-agent").ghost().label("Agent").disabled(self.agent.mode).on_click(cx.listener(|p, _, _, cx| p.set_agent_mode(true, cx)))))
            .children(self.agent.mode.then(|| {
                let mut limits = div().flex().flex_col().gap_1().child(div().text_xs().child(t(cx, "有限回合 · 每次操作和上下文均由你审核", "Bounded rounds · You review every action and context")));
                for (index, (rounds, actions)) in [(3, 2), (6, 4), (12, 8)].into_iter().enumerate() {
                    limits = limits.child(Button::new(("agent-budget", index)).ghost().disabled(self.agent_active())
                        .label(Message::new(format!("{rounds} 回合 / {actions} 操作{}", if self.agent.limits.rounds() == rounds { " ✓" } else { "" }), format!("{rounds} rounds / {actions} actions{}", if self.agent.limits.rounds() == rounds { " ✓" } else { "" })).render(cx))
                        .on_click(cx.listener(move |p, _, _, cx| { if let Ok(v) = AgentLimits::new(rounds, actions, 64 * 1024) { p.agent.limits = v; } cx.notify(); })));
                }
                limits.child(div().text_xs().child(t(cx, "远程命令、SFTP常规文件读取及已有文件替换。供应商CLI工具保持关闭，步骤由KeelShell编排。", "Remote commands, SFTP regular-file reads and existing-file replacement. Supplier CLI tools stay disabled; KeelShell coordinates the steps.")))
            })).into_any_element()
    }
    pub(super) fn agent_action_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.agent.mode {
            return None;
        }
        let run = self.agent.run.as_ref()?;
        let visual = crate::design::palette(cx);
        let mut footer = div()
            .id("agent-confirmation-footer")
            .test_support()
            .flex_shrink_0()
            .p_3()
            .border_t_1()
            .border_color(rgb(visual.border))
            .flex()
            .flex_col()
            .gap_2();
        if run.phase() == AgentPhase::Ready && self.preview {
            return None;
        }
        if run.phase() == AgentPhase::Ready && !run.steps().is_empty() {
            footer = footer.child(
                Button::new("agent-prepare-next")
                    .primary()
                    .disabled(self.busy)
                    .label(t(
                        cx,
                        "审核下一回合完整请求",
                        "Review the next full request",
                    ))
                    .on_click(cx.listener(|p, _, _, cx| p.prepare(cx))),
            );
        } else {
            let step = run.pending_step()?;
            footer = footer
                .child(div().text_xs().child(format!(
                    "{} · {}",
                    run.target().label(),
                    run.target().session_id()
                )))
                .child(
                    Button::new("agent-approve-action")
                        .primary()
                        .disabled(
                            run.phase() != AgentPhase::AwaitingAction
                                || self.agent.file_preparing
                                || !self.agent.target_admitted,
                        )
                        .label(
                            if matches!(step.decision().action, AgentAction::WriteFile { .. })
                                && self.agent.original_file.is_none()
                            {
                                t(cx, "读取原文并准备审核", "Read original for review")
                            } else {
                                t(cx, "批准这一个操作", "Approve this single action")
                            },
                        )
                        .on_click(cx.listener(|p, _, _, cx| p.review_agent_action(true, cx))),
                )
                .child(
                    Button::new("agent-reject-action")
                        .ghost()
                        .disabled(
                            run.phase() != AgentPhase::AwaitingAction || self.agent.file_preparing,
                        )
                        .label(t(cx, "拒绝这一个操作", "Reject this action"))
                        .on_click(cx.listener(|p, _, _, cx| p.review_agent_action(false, cx))),
                );
        }
        Some(footer.into_any_element())
    }
    pub(super) fn agent_view(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(run) = &self.agent.run else {
            return div().into_any_element();
        };
        let mut view = div()
            .id("agent-step-history")
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "Agent · {} · {}/{}",
                        phase_message(run.phase()).render(cx),
                        run.rounds_used(),
                        run.limits().rounds()
                    )),
            )
            .child(
                div()
                    .id("agent-captured-target")
                    .test_support()
                    .aria_label(format!(
                        "{} · {}",
                        run.target().label(),
                        run.target().session_id()
                    ))
                    .text_xs()
                    .child(format!(
                        "{} · {}",
                        run.target().label(),
                        run.target().session_id()
                    )),
            );
        view = view.child(
            div().text_xs().text_color(rgb(visual.muted)).child(
                Message::detail(
                    "本任务的推理配置",
                    "Inference profile for this run",
                    &self.agent.profile_label,
                )
                .render(cx),
            ),
        );
        for (index, step) in run.steps().iter().enumerate() {
            let mut card = div()
                .id(("agent-step", index))
                .test_support()
                .p_2()
                .rounded(px(6.))
                .bg(rgb(visual.canvas))
                .border_1()
                .border_color(rgb(visual.border))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!(
                            "{}. {}",
                            step.round(),
                            action_message(&step.decision().action).render(cx)
                        )),
                )
                .child(div().text_xs().child(step.decision().explanation.clone()));
            let exact = match &step.decision().action {
                AgentAction::Command { command } => crate::command_text::visible_command(command),
                AgentAction::ReadFile { path } => crate::command_text::visible_command(path),
                AgentAction::WriteFile { path, replacement } => format!(
                    "{}\n{}\n{}",
                    crate::command_text::visible_command(path),
                    t(cx, "完整替换内容：", "Complete replacement:"),
                    crate::command_text::visible_command(replacement)
                ),
                AgentAction::Finish { summary } => summary.clone(),
            };
            if matches!(step.decision().action, AgentAction::WriteFile { .. }) {
                card = card.child(div().text_xs().text_color(rgb(visual.muted)).child(t(cx, "原子替换复制普通 rwx 权限，不保证所有者、ACL 或特殊权限位。已观察到内容变化会拒绝；SFTP 不能排除服务器端并发替换。", "Atomic replacement copies ordinary rwx permissions; ownership, ACLs and special bits are not guaranteed. Observed content changes are rejected; SFTP cannot exclude concurrent server-side replacement.")));
            }
            if let Some(scroll) = self.agent.step_scrolls.get(&step.id()) {
                card = card.child(exact_text_review(
                    ("agent-exact", index).into(),
                    exact,
                    scroll,
                    window,
                    cx,
                ));
            } else {
                card = card.child(div().text_xs().font_family("monospace").child(exact));
            }
            if let Some(outcome) = step.outcome() {
                let text = match outcome {
                    AgentOutcome::Completed {
                        exit_status,
                        output,
                    } => {
                        let label = match exit_status {
                            Some(0) => t(
                                cx,
                                "已确认执行成功 · 退出码 0",
                                "Execution confirmed successful · Exit 0",
                            )
                            .to_owned(),
                            Some(code) => Message::new(
                                format!("已确认执行失败 · 退出码 {code}"),
                                format!("Execution confirmed failed · Exit {code}"),
                            )
                            .render(cx),
                            None => t(
                                cx,
                                "已确认文件操作完成",
                                "File operation confirmed complete",
                            )
                            .to_owned(),
                        };
                        format!("{label}\n{}", crate::command_text::visible_command(output))
                    }
                    AgentOutcome::Rejected => {
                        t(cx, "已拒绝；未执行", "Rejected; not executed").into()
                    }
                    AgentOutcome::Failed { reason } => format!(
                        "{}: {}",
                        t(cx, "已知失败", "Known failure"),
                        failure_message(reason).render(cx)
                    ),
                    AgentOutcome::Unknown => t(
                        cx,
                        "结果未知；禁止自动重试",
                        "Outcome unknown; automatic retry forbidden",
                    )
                    .into(),
                };
                card = card.child(
                    div()
                        .id(("agent-result", index))
                        .test_support()
                        .text_xs()
                        .child(text),
                );
            } else if run
                .pending_step()
                .is_some_and(|pending| pending.id() == step.id())
            {
                if let Some(original) = &self.agent.original_file {
                    card = card.child(exact_text_review(
                        "agent-original-file".into(),
                        format!(
                            "{}\n{}",
                            t(cx, "完整原文：", "Complete original:"),
                            crate::command_text::visible_command(original)
                        ),
                        &self.agent.original_scroll,
                        window,
                        cx,
                    ));
                }
                card = card.child(div().text_xs().text_color(rgb(visual.warning)).child(t(cx, "模型输出不可信。批准只针对上方捕获的目标与这个操作。", "Model output is untrusted. Approval applies only to the captured target above and this single action.")));
            }
            view = view.child(card);
        }
        view.into_any_element()
    }
}
fn phase_message(phase: AgentPhase) -> Message {
    match phase {
        AgentPhase::Ready => Message::new("等待完整请求审核", "Awaiting full request review"),
        AgentPhase::ModelRunning => {
            Message::new("等待本回合模型回复", "Awaiting this round's model reply")
        }
        AgentPhase::AwaitingAction => {
            Message::new("等待单次操作审核", "Awaiting single-action review")
        }
        AgentPhase::ActionRunning => Message::new(
            "操作已发起，结果待确认",
            "Action dispatched; outcome pending",
        ),
        AgentPhase::Completed => Message::new("模型已结束", "Model finished"),
        AgentPhase::Stopped => Message::new("已停止", "Stopped"),
        AgentPhase::OutcomeUnknown => Message::new("结果未知，已停止", "Unknown outcome; stopped"),
        AgentPhase::TargetLost => Message::new("目标失效，已停止", "Target stale; stopped"),
        AgentPhase::Failed => Message::new("失败，已停止", "Failed; stopped"),
        AgentPhase::BudgetExceeded => Message::new("预算耗尽，已停止", "Budget exhausted; stopped"),
    }
}

fn exact_text_review(
    id: ElementId,
    text: String,
    scroll: &ScrollHandle,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let visual = crate::design::palette(cx);
    // A no-wrap label can paint beyond its layout width without extending the
    // scroller. Shape the bounded lines and give it the complete natural width.
    let width = text
        .split('\n')
        .map(|line| {
            window
                .text_system()
                .shape_line(
                    line.to_owned().into(),
                    px(12.),
                    &[TextRun {
                        len: line.len(),
                        font: font("monospace"),
                        color: rgb(visual.text).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                )
                .width
        })
        .fold(px(0.), |width, next| width.max(next))
        + px(4.);
    div()
        .relative()
        .w_full()
        .h(px(192.))
        .border_1()
        .border_color(rgb(visual.border))
        .child(
            div()
                .id(id)
                .test_support()
                .size_full()
                .flex()
                .flex_col()
                .items_start()
                .overflow_scroll()
                .track_scroll(scroll)
                .p_2()
                .pr_5()
                .pb_5()
                .font_family("monospace")
                .child(
                    Label::new(text)
                        .text_size(px(12.))
                        .min_w(width)
                        .whitespace_nowrap()
                        .flex_shrink_0(),
                ),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .child(Scrollbar::new(scroll).mode(ScrollbarMode::Always)),
        )
        .into_any_element()
}

fn action_message(action: &AgentAction) -> Message {
    match action {
        AgentAction::Command { .. } => Message::new("远程命令", "Remote command"),
        AgentAction::ReadFile { .. } => Message::new("读取远程文件", "Read remote file"),
        AgentAction::WriteFile { .. } => Message::new("修改远程文件", "Replace remote file"),
        AgentAction::Finish { .. } => Message::new("模型结论", "Model conclusion"),
    }
}

fn failure_message(reason: &str) -> Message {
    let zh = match reason {
        "Action review expired before dispatch" => "操作审核已到期，未发起操作",
        "Action review expired or captured authority is no longer available" => {
            "操作审核已到期或捕获授权已失效，未发起操作"
        }
        "A reviewed original file snapshot is required before replacement"
        | "Reviewed original snapshot is missing" => "缺少已审核的原文件快照，未写入",
        "The captured session was unavailable before dispatch" => "发起操作前捕获的会话已不可用",
        "Original regular UTF-8 file could not be prepared for review" => {
            "无法准备完整常规 UTF-8 原文件用于审核，未写入"
        }
        "File is not complete bounded UTF-8" => "文件不符合完整、有界的 UTF-8 要求",
        "Regular UTF-8 file read was not confirmed" => "未确认完整常规 UTF-8 文件读取完成",
        "SFTP setup failed before writing" => "写入前 SFTP 初始化失败，未写入",
        "Authority unavailable before writing" => "写入前授权已失效，未写入",
        "A conflicting or unknown file mutation must be resolved separately" => {
            "文件存在冲突或未知修改，需要在文件面板独立解决"
        }
        "A final model summary is not an executable operation" => "模型结论不能作为可执行操作",
        _ => reason,
    };
    Message::new(zh, reason)
}
