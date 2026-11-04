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
        let footer = if let Some(review) = &self.rule_change_review {
            let operation = review.event.operation;
            let rule = &review.event.rule;
            body = body
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,
                    if review.event.deleting { "确认删除命名规则" } else { "确认保存命名规则" },
                    if review.event.deleting { "Confirm rule deletion" } else { "Confirm rule save" })))
                .child(rule_preview_message(rule).render(cx))
                .child(format!("{}: {}", t(cx, "规则 ID", "Rule ID"), rule.id))
                .child(if rule.enabled { t(cx, "已启用", "Enabled") } else { t(cx, "已停用", "Disabled") })
                .child(t(cx, "只更新配置。活动隧道保持原端点，非回环监听可能允许其他设备访问。",
                    "Updates configuration only. Active tunnels keep their endpoints. Non-loopback listeners may allow access from other devices."));
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
                .child(format!(
                    "{}: {}",
                    t(cx, "规则 ID", "Rule ID"),
                    review.rule_id
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
                        .child(row.description.render(cx)),
                );
            }
            body = body.child(t(
                cx,
                "确认只请求停止，清理完成后才显示终态。",
                "Confirmation requests a stop. Completion appears after cleanup.",
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
