//! Named-rule drafts and explicit state-bound metadata reviews.

use super::*;

#[cfg(test)]
#[path = "management_tests.rs"]
mod tests;

/// One confirmed metadata change. The workspace validates it again before I/O.
#[derive(Clone, PartialEq)]
pub(crate) struct TunnelRuleEvent {
    pub expected_state: Arc<AppState>,
    pub rule: TunnelRule,
    pub operation: uuid::Uuid,
    pub deleting: bool,
}

impl TunnelRuleEvent {
    pub(crate) fn candidate(&self, state: &AppState) -> Result<AppState, keelshell_core::Error> {
        if state.snapshot != self.expected_state.snapshot || state != self.expected_state.as_ref() {
            return Err(keelshell_core::Error::Conflict);
        }
        let mut candidate = state.clone();
        if self.deleting {
            if !state.tunnel_rules.contains(&self.rule) {
                return Err(keelshell_core::Error::TunnelRuleNotFound);
            }
            candidate.remove_tunnel_rule(self.rule.id)?;
        } else if state
            .tunnel_rules
            .iter()
            .any(|rule| rule.id == self.rule.id)
        {
            candidate.update_tunnel_rule(self.rule.clone())?;
        } else {
            candidate.add_tunnel_rule(self.rule.clone())?;
        }
        Ok(candidate)
    }
}

impl TunnelsPanel {
    pub(crate) fn recovery_candidates(&self) -> Vec<TunnelRuntimeSnapshot> {
        self.rule_state
            .as_ref()
            .into_iter()
            .flat_map(|state| state.tunnel_runtime_snapshots.iter())
            // A snapshot created by the currently listening row is already
            // represented by the live runtime row. Keep the recovery area
            // reserved for previous-session candidates, and retain a
            // CleanupUnconfirmed snapshot so the user can inspect or discard
            // the unresolved cleanup explicitly.
            .filter(|snapshot| {
                !self.rows.iter().any(|row| {
                    !row.finished
                        && row.unresolved_cleanup.is_none()
                        && row
                            .runtime_snapshot
                            .as_ref()
                            .is_some_and(|current| current.id == snapshot.id)
                })
            })
            .cloned()
            .collect()
    }

    fn inspect_recovery_candidate(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        if self
            .recovery_inspected
            .is_some_and(|inspected| inspected == id)
        {
            self.recovery_inspected = None;
        } else {
            self.recovery_inspected = Some(id);
        }
        self.recovery_review = None;
        cx.notify();
    }

    fn review_recovery_start(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        if !self.accept_rule_action(self.rule_epoch)
            || self.rule_store_blocked
            || self.recovery_discard_pending.is_some()
        {
            return;
        }
        let Some(state) = self.rule_state.as_ref() else {
            self.rule_status = Message::new(
                "配置快照不可用，请重新加载配置后再审核恢复。",
                "Configuration snapshot unavailable. Reload it before reviewing recovery.",
            );
            cx.notify();
            return;
        };
        let Some(snapshot) = state
            .tunnel_runtime_snapshots
            .iter()
            .find(|snapshot| snapshot.id == id)
        else {
            self.rule_status = recovery_error_message(TunnelRecoveryError::SnapshotNotFound);
            cx.notify();
            return;
        };
        if self.connection_id != Some(snapshot.connection.connection_id) {
            self.rule_status = Message::new(
                "当前面板连接与候选 connection_id 不匹配；请重新选择原连接后再审核。",
                "The current panel connection does not match the candidate connection_id. Select the original connection before reviewing.",
            );
            cx.notify();
            return;
        }
        match state.plan_tunnel_runtime_recovery(id) {
            Ok(plan) => {
                self.recovery_inspected = Some(id);
                self.recovery_review = Some(RuntimeRecoveryReview { plan });
                self.rule_plan = None;
                self.clear_change_review(cx);
                self.rule_stop_review = None;
                self.rule_status = Message::new(
                    "恢复候选已通过当前规则和连接绑定检查；请检查详情后确认重新启动。",
                    "The candidate matches the current rule and connection binding. Inspect the details, then confirm a fresh start.",
                );
                cx.notify();
            }
            Err(error) => {
                self.rule_status = recovery_error_message(error);
                cx.notify();
            }
        }
    }

    fn cancel_recovery_review(&mut self, token: TunnelRecoveryReviewToken, cx: &mut Context<Self>) {
        if self
            .recovery_review
            .as_ref()
            .is_some_and(|review| review.plan.review_token() == token)
        {
            self.recovery_review = None;
            self.rule_status = Message::new(
                "已取消恢复审核；候选仍保留。",
                "Recovery review cancelled; the candidate was retained.",
            );
            cx.notify();
        }
    }

    fn confirm_recovery_start(&mut self, token: TunnelRecoveryReviewToken, cx: &mut Context<Self>) {
        if self.suspended
            || self.rule_configuration_busy
            || self.rule_store_blocked
            || self.rule_mutation.is_some()
        {
            return;
        }
        let Some(review) = self
            .recovery_review
            .as_ref()
            .filter(|review| review.plan.review_token() == token)
        else {
            return;
        };
        let Some(state) = self.rule_state.clone() else {
            self.rule_status = Message::new(
                "配置快照不可用，未启动恢复隧道。",
                "Configuration snapshot unavailable; recovery was not started.",
            );
            cx.notify();
            return;
        };
        let confirmed = match review.plan.clone().confirm(&state, token) {
            Ok(confirmed) => confirmed,
            Err(error) => {
                self.recovery_review = None;
                self.rule_status = recovery_error_message(error);
                cx.notify();
                return;
            }
        };
        let rule = confirmed.plan().snapshot().rule.clone();
        if self.connection_id != Some(rule.connection_id) {
            self.recovery_review = None;
            self.rule_status = Message::new(
                "当前连接已变化，未启动恢复隧道；请重新选择原连接并审核。",
                "The current connection changed, so recovery was not started. Select the original connection and review again.",
            );
            cx.notify();
            return;
        }
        let specification = match specification_from_rule(&rule) {
            Ok(specification) => specification,
            Err(error) => {
                self.rule_status = error;
                cx.notify();
                return;
            }
        };
        let connection_binding =
            match TunnelConnectionBinding::from_state(&state, rule.connection_id) {
                Ok(binding) => binding,
                Err(error) => {
                    self.rule_status = Message::detail(
                        "无法确认当前连接路由",
                        "Unable to confirm the current connection route",
                        error,
                    );
                    cx.notify();
                    return;
                }
            };
        self.recovery_review = None;
        self.rule_status = Message::new(
            format!(
                "已确认恢复审核；将为规则“{}”创建全新的隧道 worker。",
                rule.name
            ),
            format!(
                "Recovery review confirmed; a new tunnel worker will be created for “{}”.",
                rule.name
            ),
        );
        self.start_specification(
            specification,
            Some(rule.name.clone()),
            Some(rule.id),
            Some(rule),
            Some(connection_binding),
            cx,
        );
    }

    fn discard_recovery_candidate(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        if self.suspended
            || self.rule_configuration_busy
            || self.rule_store_blocked
            || self.recovery_discard_pending.is_some()
        {
            return;
        }
        let Some(state) = self.rule_state.clone() else {
            self.rule_status = Message::new(
                "配置快照不可用，无法安全丢弃候选。",
                "Configuration snapshot unavailable; the candidate could not be safely discarded.",
            );
            cx.notify();
            return;
        };
        if !state
            .tunnel_runtime_snapshots
            .iter()
            .any(|snapshot| snapshot.id == id)
        {
            self.rule_status = Message::new(
                "恢复候选已不在当前快照中，请重新加载配置。",
                "The recovery candidate is absent from the current snapshot. Reload configuration.",
            );
            cx.notify();
            return;
        }
        let operation = uuid::Uuid::new_v4();
        self.recovery_discard_pending = Some((id, operation));
        self.recovery_review = None;
        self.rule_status = Message::new("正在丢弃恢复候选…", "Discarding recovery candidate…");
        cx.emit(TunnelRuntimeEvent::Remove {
            expected_state: Arc::new(state),
            snapshot_id: id,
            operation,
        });
        cx.notify();
    }

    pub(super) fn rule_feedback(&self) -> Message {
        if self.rule_mutation.is_some() {
            Message::new("正在保存命名规则…", "Saving named tunnel rule…")
        } else if self.rule_configuration_busy {
            Message::new(
                "配置正在保存或重新加载，请稍候…",
                "Saving or reloading configuration. Please wait…",
            )
        } else if self.rule_store_blocked {
            Message::new(
                "规则保存失败或结果未知。草稿已保留，请重新打开应用或重新加载配置后再编辑。",
                "Rule saving failed or its outcome is unknown. Draft retained; reopen the app or reload configuration before editing.",
            )
        } else if self.rule_state.is_none() {
            Message::new(
                "配置快照不可用，请重新打开应用。",
                "Configuration snapshot unavailable. Reopen the app.",
            )
        } else {
            self.rule_status.clone()
        }
    }

    pub(super) fn accept_rule_action(&self, epoch: uuid::Uuid) -> bool {
        !self.suspended
            && !self.rule_configuration_busy
            && self.rule_epoch == epoch
            && self.rule_mutation.is_none()
    }

    fn editor_matches(&self, identity: uuid::Uuid) -> bool {
        !self.suspended
            && !self.rule_configuration_busy
            && self.rule_mutation.is_none()
            && self
                .rule_editor
                .as_ref()
                .is_some_and(|editor| editor.identity == identity)
    }

    pub(super) fn begin_new_rule(
        &mut self,
        epoch: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accept_rule_action(epoch) || self.rule_store_blocked {
            return;
        }
        let (Some(connection_id), Some(state)) = (self.connection_id, self.rule_state.as_ref())
        else {
            return;
        };
        if !state
            .connections
            .iter()
            .any(|connection| connection.id == connection_id)
        {
            return;
        }
        let editor = RuleEditor::new(None, Arc::new(state.clone()), connection_id, window, cx);
        self.clear_change_review(cx);
        self.rule_editor = Some(editor);
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_plan = None;
        self.rule_stop_review = None;
        self.rule_status = Message::new(
            "新规则默认停用。保存只更新配置，启动仍需单独审核。",
            "New rules are disabled by default. Saving changes configuration; starting requires a separate review.",
        );
        cx.notify();
    }

    pub(super) fn begin_edit_rule(
        &mut self,
        id: uuid::Uuid,
        epoch: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accept_rule_action(epoch) || self.rule_store_blocked {
            return;
        }
        let (Some(connection_id), Some(state)) = (self.connection_id, self.rule_state.as_ref())
        else {
            return;
        };
        let Some(rule) = state
            .tunnel_rules
            .iter()
            .find(|rule| rule.id == id && rule.connection_id == connection_id)
        else {
            return;
        };
        let editor = RuleEditor::new(
            Some(rule),
            Arc::new(state.clone()),
            connection_id,
            window,
            cx,
        );
        self.clear_change_review(cx);
        self.rule_editor = Some(editor);
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_plan = None;
        self.rule_stop_review = None;
        self.rule_status = Message::new(
            "修改仅影响下次启动；活动隧道继续使用原端点和原 SSH 会话。",
            "Edits apply to the next start. Active tunnels keep their original endpoints and SSH session.",
        );
        cx.notify();
    }

    fn cancel_rule_edit(&mut self, identity: uuid::Uuid, cx: &mut Context<Self>) {
        if !self.editor_matches(identity) {
            return;
        }
        self.clear_change_review(cx);
        self.rule_editor = None;
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_status = self.rule_availability_message();
        cx.notify();
    }

    fn set_rule_kind(
        &mut self,
        identity: uuid::Uuid,
        kind: TunnelRuleKind,
        cx: &mut Context<Self>,
    ) {
        if self.editor_matches(identity) && self.rule_change_review.is_none() {
            if let Some(editor) = &mut self.rule_editor {
                editor.kind = kind;
            }
            cx.notify();
        }
    }

    fn toggle_rule_enabled(&mut self, identity: uuid::Uuid, cx: &mut Context<Self>) {
        if self.editor_matches(identity) && self.rule_change_review.is_none() {
            if let Some(editor) = &mut self.rule_editor {
                editor.enabled = !editor.enabled;
            }
            cx.notify();
        }
    }

    fn review_rule_save(&mut self, identity: uuid::Uuid, cx: &mut Context<Self>) {
        if !self.editor_matches(identity)
            || self.rule_store_blocked
            || self.rule_change_review.is_some()
        {
            return;
        }
        let Some(editor) = self.rule_editor.as_ref() else {
            return;
        };
        if self.connection_id != Some(editor.connection_id)
            || !self.rule_state.as_ref().is_some_and(|state| {
                state.snapshot == editor.state.snapshot && state == editor.state.as_ref()
            })
        {
            self.rule_status = Message::new(
                "编辑快照已过期；请保留所需文字，取消草稿并重新编辑。",
                "The editor snapshot expired. Retain any needed text, cancel the draft and edit again.",
            );
            cx.notify();
            return;
        }
        let rule = match editor.draft(cx) {
            Ok(rule) => rule,
            Err(error) => {
                self.rule_status = error;
                cx.notify();
                return;
            }
        };
        let event = TunnelRuleEvent {
            expected_state: editor.state.clone(),
            rule,
            operation: uuid::Uuid::new_v4(),
            deleting: false,
        };
        if let Err(error) = event.candidate(&editor.state) {
            self.rule_status = Message::detail("规则无效", "Invalid rule", error);
            cx.notify();
            return;
        }
        self.rule_change_review = Some(RuleChangeReview {
            event,
            editor_id: Some(identity),
        });
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_stop_review = None;
        if let Some(editor) = &mut self.rule_editor {
            editor.set_disabled(true, cx);
        }
        cx.notify();
    }

    pub(super) fn request_delete_rule(
        &mut self,
        id: uuid::Uuid,
        epoch: uuid::Uuid,
        cx: &mut Context<Self>,
    ) {
        if !self.accept_rule_action(epoch) || self.rule_store_blocked || self.has_active_rule(id) {
            return;
        }
        let (Some(state), Some(connection_id)) = (&self.rule_state, self.connection_id) else {
            return;
        };
        let Some(rule) = state
            .tunnel_rules
            .iter()
            .find(|rule| rule.id == id && rule.connection_id == connection_id)
        else {
            return;
        };
        let event = TunnelRuleEvent {
            expected_state: Arc::new(state.clone()),
            rule: rule.clone(),
            operation: uuid::Uuid::new_v4(),
            deleting: true,
        };
        self.clear_change_review(cx);
        self.rule_change_review = Some(RuleChangeReview {
            event,
            editor_id: None,
        });
        self.rule_editor = None;
        self.rule_stop_review = None;
        self.rule_plan = None;
        cx.notify();
    }

    fn confirm_rule_change(&mut self, operation: uuid::Uuid, cx: &mut Context<Self>) {
        if self.suspended
            || self.rule_configuration_busy
            || self.rule_mutation.is_some()
            || self.rule_store_blocked
        {
            return;
        }
        let Some(review) = self
            .rule_change_review
            .as_ref()
            .filter(|review| review.event.operation == operation)
        else {
            return;
        };
        let event = &review.event;
        if self.connection_id != Some(event.rule.connection_id)
            || !self
                .rule_state
                .as_ref()
                .is_some_and(|state| event.candidate(state).is_ok())
            || (event.deleting && self.has_active_rule(event.rule.id))
            || review.editor_id.is_some_and(|id| {
                !self.rule_editor.as_ref().is_some_and(|editor| {
                    editor.identity == id && editor.draft(cx).is_ok_and(|rule| rule == event.rule)
                })
            })
        {
            self.clear_change_review(cx);
            self.rule_status = Message::new(
                "规则审核已过期；没有发起保存。请重新检查配置和草稿。",
                "Rule review expired; no save was started. Inspect the configuration and draft again.",
            );
            cx.notify();
            return;
        }
        let event = event.clone();
        self.rule_mutation = Some(event.clone());
        self.rule_change_review = None;
        self.rule_status = Message::new("正在保存命名规则…", "Saving named tunnel rule…");
        cx.emit(event);
        cx.notify();
    }

    fn cancel_rule_change(&mut self, operation: uuid::Uuid, cx: &mut Context<Self>) {
        if self
            .rule_change_review
            .as_ref()
            .is_some_and(|review| review.event.operation == operation)
            && self.rule_mutation.is_none()
        {
            self.clear_change_review(cx);
            cx.notify();
        }
    }

    pub(super) fn clear_change_review(&mut self, cx: &mut Context<Self>) {
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_change_review = None;
        if let Some(editor) = &mut self.rule_editor {
            editor.set_disabled(self.suspended || self.rule_mutation.is_some(), cx);
        }
    }

    pub(crate) fn set_rule_configuration_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.rule_configuration_busy = busy;
        cx.notify();
    }

    pub(crate) fn mark_rule_store_uncertain(&mut self, cx: &mut Context<Self>) {
        self.rule_store_blocked = true;
        self.rule_epoch = uuid::Uuid::new_v4();
        self.rule_plan = None;
        self.clear_change_review(cx);
        self.rule_status = Message::new(
            "规则保存失败或结果未知；请重新打开应用或重新加载配置后，再编辑或启动命名规则。",
            "Rule saving failed or its outcome is unknown. Reopen the app or reload configuration before editing or starting named rules.",
        );
        cx.notify();
    }

    /// Validate the exact pending event at the workspace boundary.
    pub(crate) fn accepts_rule_mutation(&self, event: &TunnelRuleEvent) -> bool {
        !self.suspended
            && !self.rule_store_blocked
            && !self.rule_configuration_busy
            && self.connection_id == Some(event.rule.connection_id)
            && self.rule_mutation.as_ref().is_some_and(|pending| {
                pending == event && pending.expected_state.snapshot == event.expected_state.snapshot
            })
            && (!event.deleting || !self.has_active_rule(event.rule.id))
    }

    pub(crate) fn mark_rule_mutation_succeeded(
        &mut self,
        operation: uuid::Uuid,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .rule_mutation
            .as_ref()
            .is_some_and(|pending| pending.operation == operation)
        {
            return false;
        }
        self.rule_mutation = None;
        self.rule_editor = None;
        self.clear_change_review(cx);
        self.rule_status = Message::new("命名规则变更已保存", "Named rule change saved");
        cx.notify();
        true
    }

    pub(crate) fn mark_rule_mutation_failed(
        &mut self,
        operation: uuid::Uuid,
        message: Message,
        storage_uncertain: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .rule_mutation
            .as_ref()
            .is_some_and(|pending| pending.operation == operation)
        {
            return false;
        }
        self.rule_mutation = None;
        self.rule_store_blocked |= storage_uncertain;
        self.clear_change_review(cx);
        self.rule_status = message;
        cx.notify();
        true
    }

    pub(crate) fn has_active_rule(&self, id: uuid::Uuid) -> bool {
        self.rows
            .iter()
            .any(|row| row.rule_id == Some(id) && !row.finished)
    }

    #[cfg(test)]
    pub(crate) fn runtime_row_count(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub(crate) fn any_runtime_listener(&self) -> bool {
        self.rows.iter().any(|row| row.listening && !row.finished)
    }

    #[cfg(test)]
    pub(crate) fn all_runtime_rows_finished(&self) -> bool {
        self.rows.iter().all(|row| row.finished)
    }

    pub(super) fn rule_workers(&self, id: uuid::Uuid) -> Vec<usize> {
        self.rows
            .iter()
            .filter(|row| {
                row.rule_id == Some(id) && !row.finished && !row.cancel.load(Ordering::Acquire)
            })
            .map(|row| row.id)
            .collect()
    }

    pub(super) fn review_rule_stop(
        &mut self,
        id: uuid::Uuid,
        epoch: uuid::Uuid,
        workers: &[usize],
        cx: &mut Context<Self>,
    ) {
        if self.suspended
            || self.rule_epoch != epoch
            || workers.is_empty()
            || self.rule_workers(id) != workers
        {
            return;
        }
        // Runtime ownership outlives saved metadata. A reload can remove the
        // rule or its connection without revoking the ability to stop its rows.
        self.rule_stop_review = Some(RuleStopReview {
            token: uuid::Uuid::new_v4(),
            owner: self.runtime_owner,
            rule_id: id,
            workers: workers.to_vec(),
        });
        self.rule_plan = None;
        self.clear_change_review(cx);
        cx.notify();
    }

    fn confirm_rule_stop(&mut self, token: uuid::Uuid, cx: &mut Context<Self>) {
        let Some(review) = self
            .rule_stop_review
            .as_ref()
            .filter(|review| review.token == token)
        else {
            return;
        };
        if self.suspended {
            return;
        }
        let valid = self.runtime_owner == review.owner
            && self.rule_workers(review.rule_id) == review.workers;
        if valid {
            let workers = review.workers.clone();
            // Only cancellation requests cross into the original worker. Its
            // owner, SSH authority and Finished acknowledgement stay unchanged.
            for worker in workers {
                self.stop_worker(worker);
            }
            self.status = tunnel_summary(&self.rows);
        } else {
            self.rule_status = Message::new(
                "停止审核已过期；请重新审核当前活动实例。",
                "Stop review expired; review the current active instances again.",
            );
        }
        self.rule_stop_review = None;
        cx.notify();
    }

    pub(super) fn stop_worker(&mut self, id: usize) {
        if let Some(row) = self
            .rows
            .iter_mut()
            .find(|row| row.id == id && !row.finished && row.unresolved_cleanup.is_none())
        {
            row.cancel.store(true, Ordering::Release);
            row.status = Message::new(
                "正在停止，等待监听器清理…",
                "Stopping; waiting for listener cleanup…",
            );
        }
    }
}

pub(super) struct RuleChangeReview {
    event: TunnelRuleEvent,
    editor_id: Option<uuid::Uuid>,
}

pub(super) struct RuleStopReview {
    pub token: uuid::Uuid,
    pub owner: uuid::Uuid,
    pub rule_id: uuid::Uuid,
    // Panel-local IDs are never reused; a later start cannot inherit this review.
    pub workers: Vec<usize>,
}

pub(super) struct RuntimeRecoveryReview {
    pub plan: ValidatedTunnelRecoveryPlan,
}

pub(super) struct RuleManagementView {
    pub body: AnyElement,
    pub footer: AnyElement,
}

pub(super) struct RuleEditor {
    identity: uuid::Uuid,
    state: Arc<AppState>,
    connection_id: uuid::Uuid,
    draft_id: uuid::Uuid,
    id: Option<uuid::Uuid>,
    name: Entity<InputState>,
    bind_host: Entity<InputState>,
    bind_port: Entity<InputState>,
    target_host: Entity<InputState>,
    target_port: Entity<InputState>,
    kind: TunnelRuleKind,
    enabled: bool,
}

impl RuleEditor {
    fn new(
        rule: Option<&TunnelRule>,
        state: Arc<AppState>,
        connection_id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<TunnelsPanel>,
    ) -> Self {
        let fallback = (
            "",
            "127.0.0.1",
            String::from("0"),
            "127.0.0.1",
            String::from("22"),
            TunnelRuleKind::Local,
            false,
        );
        let (name, bind_host, bind_port, target_host, target_port, kind, enabled) = rule
            .map(|rule| {
                (
                    rule.name.as_str(),
                    rule.bind_host.as_str(),
                    rule.bind_port.to_string(),
                    rule.target_host.as_deref().unwrap_or_default(),
                    rule.target_port
                        .map_or_else(String::new, |port| port.to_string()),
                    rule.kind,
                    rule.enabled,
                )
            })
            .unwrap_or(fallback);
        Self {
            identity: uuid::Uuid::new_v4(),
            state,
            connection_id,
            draft_id: rule.map_or_else(uuid::Uuid::new_v4, |rule| rule.id),
            id: rule.map(|rule| rule.id),
            name: field(name, window, cx),
            bind_host: field(bind_host, window, cx),
            bind_port: field(&bind_port, window, cx),
            target_host: field(target_host, window, cx),
            target_port: field(&target_port, window, cx),
            kind,
            enabled,
        }
    }

    fn draft(&self, cx: &App) -> Result<TunnelRule, Message> {
        let bind_port = self
            .bind_port
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .map_err(|_| Message::new("监听端口必须为 0–65535", "Bind port must be 0–65535"))?;
        let (target_host, target_port) = match self.kind {
            TunnelRuleKind::Dynamic => (None, None),
            TunnelRuleKind::Local | TunnelRuleKind::Remote => {
                let target_port = self
                    .target_port
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| {
                        Message::new("目标端口必须为 1–65535", "Destination port must be 1–65535")
                    })?;
                (
                    Some(self.target_host.read(cx).value().trim().to_owned()),
                    Some(target_port),
                )
            }
        };
        let mut rule = TunnelRule::new(
            self.name.read(cx).value().to_string(),
            self.connection_id,
            self.kind,
            self.bind_host.read(cx).value().to_string(),
            bind_port,
            target_host,
            target_port,
        )
        .map_err(|error| rule_validation_message(&error))?;
        rule.id = self.draft_id;
        rule.enabled = self.enabled;
        rule.validate()
            .map_err(|error| rule_validation_message(&error))?;
        Ok(rule)
    }

    pub(super) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<TunnelsPanel>) {
        for (input, zh, en) in [
            (&self.name, "规则名称", "Rule name"),
            (&self.bind_host, "监听地址", "Bind host"),
            (&self.bind_port, "监听端口", "Bind port"),
            (&self.target_host, "目标地址", "Destination host"),
            (&self.target_port, "目标端口", "Destination port"),
        ] {
            input.update(cx, |input, cx| {
                input.set_placeholder(t(cx, zh, en), window, cx)
            });
        }
    }

    fn set_disabled(&mut self, disabled: bool, cx: &mut Context<TunnelsPanel>) {
        for input in [
            &self.name,
            &self.bind_host,
            &self.bind_port,
            &self.target_host,
            &self.target_port,
        ] {
            input.update(cx, |input, cx| input.set_disabled(disabled, cx));
        }
    }
}

fn rule_validation_message(error: &keelshell_core::ValidationError) -> Message {
    match error.field {
        "tunnel_rule.name" => Message::new(
            "请输入规则名称，且不要包含首尾空白或控制字符。",
            "Enter a rule name without surrounding whitespace or control characters.",
        ),
        "tunnel_rule.bind_host" => Message::new(
            "监听地址必须是主机名或 IP；动态 SOCKS5 只能使用回环 IP。",
            "The bind address must be a host or IP; dynamic SOCKS5 requires a loopback IP.",
        ),
        "tunnel_rule.target_host" => Message::new(
            "本地和远程转发必须填写目标地址。",
            "Local and remote forwarding require a destination host.",
        ),
        "tunnel_rule.target_port" => Message::new(
            "本地和远程转发必须填写 1–65535 的目标端口。",
            "Local and remote forwarding require a destination port from 1 to 65535.",
        ),
        "tunnel_rule.target" => Message::new(
            "动态 SOCKS5 不保存目标地址或端口。",
            "Dynamic SOCKS5 does not store a destination host or port.",
        ),
        "tunnel_rule.connection_id" => Message::new(
            "规则必须绑定当前已保存的 SSH 连接。",
            "The rule must belong to the current saved SSH connection.",
        ),
        _ => Message::detail("规则内容无效", "Invalid rule", error),
    }
}

fn recovery_error_message(error: TunnelRecoveryError) -> Message {
    match error {
        TunnelRecoveryError::SnapshotNotFound => Message::new(
            "恢复候选不存在，请重新加载配置。",
            "The recovery candidate is missing. Reload configuration.",
        ),
        TunnelRecoveryError::UnsupportedSnapshotVersion => Message::new(
            "恢复候选版本不受支持；候选已保留，请使用支持该版本的构建或明确丢弃。",
            "The recovery candidate uses an unsupported version. It was retained; use a compatible build or explicitly discard it.",
        ),
        TunnelRecoveryError::CleanupUnconfirmed => Message::new(
            "上次清理结果未确认，已阻止恢复；请先重新加载并确认清理，或明确丢弃候选。",
            "Previous cleanup was not confirmed, so recovery is blocked. Reload and confirm cleanup, or explicitly discard the candidate.",
        ),
        TunnelRecoveryError::InvalidSnapshot => Message::new(
            "恢复候选数据无效，已阻止启动；请重新加载配置。",
            "The recovery candidate is invalid. Start was blocked; reload configuration.",
        ),
        TunnelRecoveryError::RuleMissing => Message::new(
            "当前规则已不存在；候选已保留，请重新加载规则或明确丢弃。",
            "The current rule no longer exists. The candidate was retained; reload the rule or explicitly discard it.",
        ),
        TunnelRecoveryError::RuleChanged => Message::new(
            "当前规则内容已变化；必须重新检查规则后才能恢复。",
            "The current rule changed. Inspect the current rule before recovery.",
        ),
        TunnelRecoveryError::ConnectionMissing => Message::new(
            "当前连接已不存在；候选已保留，请重新加载连接或明确丢弃。",
            "The saved connection no longer exists. The candidate was retained; reload the connection or explicitly discard it.",
        ),
        TunnelRecoveryError::ConnectionRouteChanged => Message::new(
            "连接路由或跳板目标已变化；已阻止恢复，请重新选择连接并审核。",
            "The connection route or jump target changed. Recovery was blocked; select and review the connection again.",
        ),
        TunnelRecoveryError::AuthenticationTargetChanged => Message::new(
            "连接认证目标已变化；已阻止恢复，请重新审核。",
            "The connection authentication target changed. Recovery was blocked; review it again.",
        ),
        TunnelRecoveryError::StateInvalid => Message::new(
            "当前配置状态无效；请重新加载配置后再审核恢复。",
            "The current configuration is invalid. Reload it before reviewing recovery.",
        ),
        TunnelRecoveryError::TokenMismatch => Message::new(
            "恢复审核 token 不匹配，未启动。",
            "The recovery review token did not match; nothing was started.",
        ),
        TunnelRecoveryError::StateChanged => Message::new(
            "恢复审核已过期；请重新 Inspect 并 Review start。",
            "The recovery review expired. Inspect the candidate and choose Review start again.",
        ),
    }
}

fn tunnel_kind_label(kind: TunnelRuleKind, cx: &App) -> String {
    t(
        cx,
        match kind {
            TunnelRuleKind::Local => "本地转发",
            TunnelRuleKind::Remote => "远程转发",
            TunnelRuleKind::Dynamic => "动态 SOCKS5",
        },
        match kind {
            TunnelRuleKind::Local => "Local forwarding",
            TunnelRuleKind::Remote => "Remote forwarding",
            TunnelRuleKind::Dynamic => "Dynamic SOCKS5",
        },
    )
    .to_owned()
}

impl TunnelsPanel {
    pub(super) fn rule_review_view(&self, cx: &mut Context<Self>) -> Option<RuleManagementView> {
        let visual = crate::design::palette(cx);
        let mut body = div()
            .id("tunnel-rule-review")
            .test_support()
            .px_3()
            .py_3()
            .flex()
            .flex_col()
            .gap_2();
        let footer = if let Some(review) = &self.recovery_review {
            let token = review.plan.review_token();
            let snapshot = review.plan.snapshot();
            let assessment = self
                .rule_state
                .as_ref()
                .and_then(|state| state.assess_tunnel_runtime_snapshot(snapshot.id).err());
            body = body
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t(cx, "确认恢复命名隧道", "Confirm named tunnel recovery")),
                )
                .child(tunnel_text(
                    "tunnel-recovery-review-target",
                    format!(
                        "{}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}",
                        t(cx, "规则", "Rule"),
                        t(cx, "规则 ID", "Rule ID"),
                        snapshot.rule.id,
                        t(cx, "连接 ID", "Connection ID"),
                        snapshot.connection.connection_id,
                        t(cx, "上次实际监听", "Last assigned listener"),
                        snapshot.assigned_listener,
                        t(cx, "上次观察时间", "Last observed"),
                        snapshot.started_at,
                        t(
                            cx,
                            "这只会创建全新的 worker，不会复用旧 listener、SSH session 或流量。",
                            "This creates a new worker; it never reuses the old listener, SSH session or traffic.",
                        ),
                    ),
                ))
                .child(tunnel_text(
                    "tunnel-recovery-review-effect",
                    assessment
                        .map(recovery_error_message)
                        .unwrap_or_else(|| {
                            Message::new(
                                "当前规则、路由和认证目标均与候选一致；确认后仍需重新建立当前连接并重新启动。",
                                "The current rule, route and authentication target match; confirmation still creates a fresh start on the current connection.",
                            )
                        })
                        .render(cx),
                ));
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("cancel-tunnel-recovery-review")
                        .ghost()
                        .min_h(px(32.))
                        .label(t(cx, "取消审核", "Cancel review"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.cancel_recovery_review(token, cx)
                        })),
                )
                .child(
                    Button::new("confirm-tunnel-recovery-start")
                        .primary()
                        .min_h(px(32.))
                        .disabled(
                            self.rule_configuration_busy
                                || self.rule_store_blocked
                                || assessment.is_some(),
                        )
                        .label(t(cx, "确认并重新启动", "Confirm and start fresh"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.confirm_recovery_start(token, cx)
                        })),
                )
        } else if let Some(review) = &self.rule_change_review {
            let operation = review.event.operation;
            let rule = &review.event.rule;
            body = body
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,
                    if review.event.deleting { "确认删除命名规则" } else { "确认保存命名规则" },
                    if review.event.deleting { "Confirm rule deletion" } else { "Confirm rule save" })))
                .child(tunnel_text("tunnel-rule-review-target", format!(
                    "{}\n{}: {}\n{}: {}\nSSH: {}\n{}",
                    rule_preview_message(rule).render(cx),
                    t(cx, "规则 ID", "Rule ID"), rule.id,
                    t(cx, "连接 ID", "Connection ID"), rule.connection_id,
                    self.host,
                    if rule.enabled { t(cx, "已启用", "Enabled") } else { t(cx, "已停用", "Disabled") },
                )))
                .child(tunnel_text("tunnel-rule-review-effect", t(cx, "只更新配置。活动隧道保持原端点，非回环监听可能允许其他设备访问。",
                    "Updates configuration only. Active tunnels keep their endpoints. Non-loopback listeners may allow access from other devices.")));
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("cancel-tunnel-rule-review")
                        .ghost()
                        .min_h(px(32.))
                        .label(t(cx, "取消审核", "Cancel review"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.cancel_rule_change(operation, cx)
                        })),
                )
                .child(
                    Button::new("confirm-tunnel-rule-change")
                        .primary()
                        .min_h(px(32.))
                        .disabled(self.rule_configuration_busy)
                        .label(t(cx, "确认", "Confirm"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.confirm_rule_change(operation, cx)
                        })),
                )
        } else if let Some(review) = &self.rule_stop_review {
            let token = review.token;
            body = body
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(
                    cx,
                    "审核停止此规则的活动实例",
                    "Review stopping this rule's active instances",
                )))
                .child(tunnel_text(
                    "tunnel-rule-review-target",
                    format!(
                        "{}: {}\nSSH: {}",
                        t(cx, "规则 ID", "Rule ID"),
                        review.rule_id,
                        self.host,
                    ),
                ));
            for row in self
                .rows
                .iter()
                .filter(|row| review.workers.contains(&row.id))
            {
                body = body.child(
                    div()
                        .py_2()
                        .border_b_1()
                        .border_color(rgb(visual.border))
                        .child(tunnel_text(
                            ("tunnel-stop-review-instance", row.id),
                            format!(
                                "{}\n{}\n{}",
                                row.details(cx),
                                row.status.render(cx),
                                traffic_message(row.last_traffic).render(cx),
                            ),
                        )),
                );
            }
            body = body.child(tunnel_text(
                "tunnel-rule-review-effect",
                t(
                    cx,
                    "确认只请求停止，清理完成后才显示终态。",
                    "Confirmation requests a stop. Completion appears after cleanup.",
                ),
            ));
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("cancel-tunnel-rule-stop")
                        .ghost()
                        .min_h(px(32.))
                        .label(t(cx, "取消审核", "Cancel review"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view
                                .rule_stop_review
                                .as_ref()
                                .is_some_and(|review| review.token == token)
                            {
                                view.rule_stop_review = None;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    Button::new("confirm-tunnel-rule-stop")
                        .primary()
                        .min_h(px(32.))
                        .label(t(cx, "确认停止", "Confirm stop"))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.confirm_rule_stop(token, cx)),
                        ),
                )
        } else {
            return None;
        };
        Some(RuleManagementView {
            body: body.into_any_element(),
            footer: footer.into_any_element(),
        })
    }

    pub(super) fn recovery_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let visual = crate::design::palette(cx);
        let candidates = self.recovery_candidates();
        if candidates.is_empty() {
            return None;
        }
        let inspected = self.recovery_inspected;
        let mut cards = div().flex().flex_col().gap_1();
        for snapshot in candidates {
            let id = snapshot.id;
            let connection_matches = self.connection_id == Some(snapshot.connection.connection_id);
            let assessment_error = self
                .rule_state
                .as_ref()
                .and_then(|state| state.assess_tunnel_runtime_snapshot(id).err());
            let review_blocked = !connection_matches || assessment_error.is_some();
            let status = if !connection_matches {
                t(
                    cx,
                    "当前连接不匹配；请重新选择原连接",
                    "Current connection does not match; select the original connection",
                )
            } else {
                match snapshot.status {
                    TunnelRuntimeStatus::PossibleRunning => t(
                        cx,
                        "可能仍在运行；需要重新审核",
                        "May still be running; review required",
                    ),
                    TunnelRuntimeStatus::CleanupUnconfirmed => t(
                        cx,
                        "清理未确认；候选保留",
                        "Cleanup unconfirmed; candidate retained",
                    ),
                }
            };
            let reason = if !connection_matches {
                Message::new(
                    "请打开候选所属的已保存连接；候选仍保留。",
                    "Open the saved connection that owns this candidate; the candidate was retained.",
                )
            } else {
                assessment_error
                    .map(recovery_error_message)
                    .unwrap_or_else(|| {
                        Message::new(
                            "当前规则和连接绑定可供审核",
                            "Current rule and connection binding are reviewable",
                        )
                    })
            };
            let details = format!(
                "{}\n{}: {}\n{}: {}\n{}: {}",
                snapshot.rule.name,
                t(cx, "类型", "Type"),
                tunnel_kind_label(snapshot.rule.kind, cx),
                t(cx, "实际监听", "Assigned listener"),
                snapshot.assigned_listener,
                t(cx, "观察时间", "Observed"),
                snapshot.started_at,
            );
            let mut card = div()
                .id(format!("tunnel-recovery-candidate-{id}"))
                .test_support()
                .px_2()
                .py_2()
                .flex()
                .flex_col()
                .gap_1()
                .border_b_1()
                .border_color(rgb(visual.border))
                .bg(rgb(if inspected == Some(id) {
                    visual.canvas
                } else {
                    visual.surface
                }))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .pt_1()
                                .text_color(rgb(visual.warning))
                                .child(IconName::CircleAlert),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(tunnel_text(
                                    format!("tunnel-recovery-candidate-details-{id}"),
                                    details,
                                ))
                                .child(div().text_color(rgb(visual.warning)).child(tunnel_text(
                                    format!("tunnel-recovery-candidate-status-{id}"),
                                    format!("{}\n{}", status, reason.render(cx)),
                                ))),
                        )
                        .child(
                            Button::new(format!("inspect-tunnel-recovery-{id}"))
                                .ghost()
                                .compact()
                                .icon(IconName::Eye)
                                .label(t(cx, "Inspect", "Inspect"))
                                .accessibility_label(format!(
                                    "{}: {}",
                                    t(cx, "检查恢复候选", "Inspect recovery candidate"),
                                    snapshot.rule.name
                                ))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.inspect_recovery_candidate(id, cx)
                                })),
                        )
                        .child(
                            Button::new(format!("review-recovery-start-{id}"))
                                .ghost()
                                .compact()
                                .icon(IconName::Play)
                                .label(t(cx, "Review start", "Review start"))
                                .disabled(
                                    review_blocked
                                        || self.suspended
                                        || self.rule_store_blocked
                                        || self.rule_configuration_busy
                                        || self.recovery_discard_pending.is_some(),
                                )
                                .accessibility_label(format!(
                                    "{}: {}",
                                    t(cx, "审核恢复启动", "Review recovery start"),
                                    snapshot.rule.name
                                ))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.review_recovery_start(id, cx)
                                })),
                        )
                        .child(
                            Button::new(format!("discard-tunnel-recovery-{id}"))
                                .ghost()
                                .compact()
                                .icon(IconName::Trash)
                                .label(t(cx, "Discard", "Discard"))
                                .disabled(
                                    self.suspended
                                        || self.rule_store_blocked
                                        || self.rule_configuration_busy
                                        || self.recovery_discard_pending.is_some(),
                                )
                                .accessibility_label(format!(
                                    "{}: {}",
                                    t(cx, "丢弃恢复候选", "Discard recovery candidate"),
                                    snapshot.rule.name
                                ))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.discard_recovery_candidate(id, cx)
                                })),
                        ),
                );
            if inspected == Some(id) {
                card = card.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(visual.muted))
                        .child(tunnel_text(
                            format!("tunnel-recovery-candidate-inspected-{id}"),
                            format!(
                                "{}\n{}",
                                t(
                                    cx,
                                    "只读详情：不会复用旧 listener、SSH session、worker 或流量。",
                                    "Read-only details: the old listener, SSH session, worker and traffic are never reused.",
                                ),
                                snapshot.connection.connection_id
                            ),
                        )),
                );
            }
            cards = cards.child(card);
        }
        Some(
            div()
                .id("tunnel-recovery-section")
                .test_support()
                .px_3()
                .py_2()
                .flex_shrink_0()
                .border_b_1()
                .border_color(rgb(visual.border))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_color(rgb(visual.warning))
                        .child(IconName::CircleAlert)
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t(cx, "上次可能运行过的命名隧道", "Named tunnels that may have survived")),
                        ),
                )
                .child(
                    div()
                        .py_1()
                        .text_color(rgb(visual.muted))
                        .child(t(
                            cx,
                            "这是恢复候选，不代表当前仍在线；必须选择当前连接并重新审核后才能启动。",
                            "These are recovery candidates, not an online indicator. Select the current connection and review again before starting.",
                        )),
                )
                .child(cards)
                .into_any_element(),
        )
    }

    pub(super) fn rule_editor_view(&self, cx: &mut Context<Self>) -> Option<RuleManagementView> {
        let editor = self.rule_editor.as_ref()?;
        let visual = crate::design::palette(cx);
        let disabled =
            self.suspended || self.rule_mutation.is_some() || self.rule_change_review.is_some();
        let identity = editor.identity;
        let title = if editor.id.is_some() {
            t(cx, "编辑命名规则", "Edit named rule")
        } else {
            t(cx, "新建命名规则", "New named rule")
        };
        let target_fields = div()
            .flex()
            .items_end()
            .gap_2()
            .when(editor.kind != TunnelRuleKind::Dynamic, |row| {
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(t(cx, "目标地址", "Destination host"))
                        .child(
                            Input::new(&editor.target_host)
                                .id("tunnel-rule-target-host")
                                .aria_label(t(cx, "目标地址", "Destination host"))
                                .small()
                                .disabled(disabled),
                        ),
                )
                .child(
                    div()
                        .w(px(96.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(t(cx, "目标端口", "Destination port"))
                        .child(
                            Input::new(&editor.target_port)
                                .id("tunnel-rule-target-port")
                                .aria_label(t(cx, "目标端口", "Destination port"))
                                .small()
                                .disabled(disabled),
                        ),
                )
            })
            .when(editor.kind == TunnelRuleKind::Dynamic, |row| {
                row.child(
                    div()
                        .flex_1()
                        .pb_2()
                        .text_color(rgb(visual.muted))
                        .child(t(
                            cx,
                            "动态 SOCKS5 不预设目标；客户端通过代理请求目标。",
                            "Dynamic SOCKS5 has no preset destination; the client requests it through the proxy.",
                        )),
                )
            });
        let body = div()
            .id("tunnel-rule-editor")
            .test_support()
            .px_3()
            .py_3()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(visual.canvas))
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(t(cx, "名称", "Name"))
                            .child(
                                Input::new(&editor.name)
                                    .id("tunnel-rule-name")
                                    .aria_label(t(cx, "规则名称", "Rule name"))
                                    .small()
                                    .disabled(disabled),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(t(cx, "监听地址", "Bind host"))
                            .child(
                                Input::new(&editor.bind_host)
                                    .id("tunnel-rule-bind-host")
                                    .aria_label(t(cx, "监听地址", "Bind host"))
                                    .small()
                                    .disabled(disabled),
                            ),
                    )
                    .child(
                        div()
                            .w(px(96.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(t(cx, "监听端口", "Bind port"))
                            .child(
                                Input::new(&editor.bind_port)
                                    .id("tunnel-rule-bind-port")
                                    .aria_label(t(cx, "监听端口", "Bind port"))
                                    .small()
                                    .disabled(disabled),
                            ),
                    ),
            )
            .child(target_fields)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(t(cx, "转发类型", "Forwarding type"))
                    .child(
                        Button::new("tunnel-rule-kind-local")
                            .ghost()
                            .compact()
                            .selected(editor.kind == TunnelRuleKind::Local)
                            .toggled(editor.kind == TunnelRuleKind::Local)
                            .disabled(disabled)
                            .label(t(cx, "本地", "Local"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.set_rule_kind(identity, TunnelRuleKind::Local, cx)
                            })),
                    )
                    .child(
                        Button::new("tunnel-rule-kind-remote")
                            .ghost()
                            .compact()
                            .selected(editor.kind == TunnelRuleKind::Remote)
                            .toggled(editor.kind == TunnelRuleKind::Remote)
                            .disabled(disabled)
                            .label(t(cx, "远程", "Remote"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.set_rule_kind(identity, TunnelRuleKind::Remote, cx)
                            })),
                    )
                    .child(
                        Button::new("tunnel-rule-kind-dynamic")
                            .ghost()
                            .compact()
                            .selected(editor.kind == TunnelRuleKind::Dynamic)
                            .toggled(editor.kind == TunnelRuleKind::Dynamic)
                            .disabled(disabled)
                            .label(t(cx, "动态 SOCKS5", "Dynamic SOCKS5"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.set_rule_kind(identity, TunnelRuleKind::Dynamic, cx)
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("tunnel-rule-enabled")
                            .ghost()
                            .compact()
                            .selected(editor.enabled)
                            .toggled(editor.enabled)
                            .accessibility_label(t(cx, "启用规则", "Enable rule"))
                            .disabled(disabled)
                            .label(if editor.enabled {
                                t(cx, "已启用", "Enabled")
                            } else {
                                t(cx, "已停用", "Disabled")
                            })
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.toggle_rule_enabled(identity, cx)
                            })),
                    ),
            )
            .into_any_element();
        let footer = div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel-tunnel-rule")
                    .ghost()
                    .min_h(px(32.))
                    .disabled(disabled)
                    .label(t(cx, "取消", "Cancel"))
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.cancel_rule_edit(identity, cx)),
                    ),
            )
            .child(
                Button::new("review-save-tunnel-rule")
                    .primary()
                    .min_h(px(32.))
                    .disabled(disabled || self.rule_store_blocked || self.rule_configuration_busy)
                    .icon(IconName::Save)
                    .label(t(cx, "审核保存", "Review save"))
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.review_rule_save(identity, cx)),
                    ),
            )
            .into_any_element();
        Some(RuleManagementView { body, footer })
    }
}
