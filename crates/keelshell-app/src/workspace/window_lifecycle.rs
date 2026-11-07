//! Exact window consent precedes authority revocation and bounded owner cleanup.
use super::*;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseIntent {
    Window,
    Quit,
    Install,
}

#[derive(Clone, PartialEq, Eq)]
struct Drafts {
    command: SharedString,
    command_revision: u64,
    quick: Vec<String>,
    form: Option<Vec<String>>,
    files: Vec<(EntityId, crate::files::CloseDraft)>,
    assistant: SharedString,
}

#[derive(Clone)]
struct Snapshot {
    id: uuid::Uuid,
    native: WindowId,
    tabs: Vec<EntityId>,
    pane_revision: u64,
    sessions: Vec<SshSession>,
    drafts: Drafts,
}

impl Snapshot {
    fn same(&self, other: &Self) -> bool {
        self.id == other.id
            && self.native == other.native
            && self.tabs == other.tabs
            && self.pane_revision == other.pane_revision
            && self.drafts == other.drafts
            && self.sessions.len() == other.sessions.len()
            && self
                .sessions
                .iter()
                .zip(&other.sessions)
                .all(|(a, b)| a.same_connection(b))
    }
}

pub(super) struct CloseReview {
    pub(super) id: uuid::Uuid,
    intent: CloseIntent,
    windows: Vec<Snapshot>,
}

pub(super) struct CleanupUnknown {
    intent: CloseIntent,
    message: Message,
}

impl Workspace {
    pub(super) fn new_workspace_window(
        &mut self,
        _: &NewWorkspaceWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal().is_some()
            || !self.configuration_current(cx)
            || !self.app_windows.read(cx).can_open()
        {
            self.status = Message::new(
                "无法新建窗口：最多 8 窗，且不能处于配置写入、维护、关闭或待重载状态。",
                "Cannot open a window: the limit is eight; finish configuration writes, maintenance, closing or explicit reload first.",
            );
            self.focus_current_surface(window, cx);
            cx.notify();
            return;
        }
        let state = match self.app_windows.read(cx).gate.lock() {
            Ok(gate) => gate.state().clone(),
            Err(_) => return,
        };
        match crate::app_windows::open(
            self.store.clone(),
            state,
            None,
            self.runtime.clone(),
            false,
            cx,
        ) {
            Ok((_, _)) => {}
            Err(error) => {
                self.status = Message::detail("无法新建窗口", "Unable to open a window", error);
                cx.notify();
            }
        }
    }

    pub(super) fn close_workspace_window(
        &mut self,
        _: &CloseWorkspaceWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_window_close(CloseIntent::Window, window, cx);
    }

    fn window_close_blocker(&self, cx: &App) -> Option<Message> {
        if self.closing.is_some() {
            return Some(Message::new(
                "窗口清理尚未确认；请保留窗口并核对原连接，不能自动重试。",
                "Window cleanup is unconfirmed. Retain the window and inspect its original connection; no automatic retry is permitted.",
            ));
        }
        if self.cleanup_unknown.is_some() {
            return Some(Message::new(
                "请先确认未知清理结果，再重新审核关闭。",
                "A cleanup result is unknown. Acknowledge it before reviewing close again.",
            ));
        }
        if self.saving
            || self.state_reloading
            || !self
                .app_windows
                .read(cx)
                .gate
                .lock()
                .is_ok_and(|gate| gate.available())
        {
            return Some(Message::new(
                "请等待配置保存、重新加载或维护结束。",
                "Wait for configuration save, reload or maintenance to settle.",
            ));
        }
        if self.connecting
            || self.route_jobs != 0
            || self.connect_route.is_some()
            || self.login.is_some()
            || self.keyboard_interactive.is_some()
            || self.host_approval.is_some()
        {
            return Some(Message::new(
                "请先在 SSH 认证面板取消连接并等待结束。",
                "Cancel the SSH connection in its authentication panel and wait for completion.",
            ));
        }
        if !self.assistant.read(cx).config_recovery_idle() || self.window_agent_busy(cx) {
            return Some(Message::new(
                "请先停止 AI / Agent，并等待当前操作结束。",
                "Stop AI / Agent and wait for the current operation to finish.",
            ));
        }
        if !self.mcp.window_close_idle() {
            return Some(Message::new(
                "请先在 MCP 面板关闭服务、停止操作并确认结果。",
                "Turn off the MCP service, stop operations and confirm their results in the MCP panel.",
            ));
        }
        if self
            .batch_panel
            .as_ref()
            .is_some_and(|panel| panel.read(cx).is_running())
            || self
                .workflow_panel
                .as_ref()
                .is_some_and(|panel| panel.read(cx).is_running())
        {
            return Some(Message::new(
                "请先停止批量 / 工作流 / 定时任务，并等待结果。",
                "Stop batch, workflow and scheduled tasks, then wait for their results.",
            ));
        }
        if self.ai_settings.is_some()
            || self.snippet_modal_open()
            || self.folder_form.is_some()
            || self.destination_prompt.is_some()
            || self.library_batch_prompt.is_some()
            || self.profile_sync.is_some()
            || self.vault_settings.is_some()
            || self.configuration_recovery.is_some()
        {
            return Some(Message::new(
                "请先完成或取消当前配置草稿面板，再关闭窗口。",
                "Finish or cancel the current configuration draft panel before closing the window.",
            ));
        }
        for panels in self
            .panels
            .values()
            .chain(self.archived_panels.values())
            .chain(self.retired_window_panels.iter())
        {
            if panels
                .files
                .as_ref()
                .is_some_and(|panel| panel.read(cx).window_close_busy())
            {
                return Some(Message::new(
                    "请先在文件面板停止传输 / 远端写入，并核对未知结果。",
                    "Stop transfers / remote writes in the Files panel and reconcile unknown results first.",
                ));
            }
            if panels
                .monitor
                .as_ref()
                .is_some_and(|panel| panel.read(cx).window_close_busy(cx))
            {
                return Some(Message::new(
                    "请先停止网络诊断 / 进程操作，并核对清理结果。",
                    "Stop network diagnostics / process actions and confirm their cleanup first.",
                ));
            }
        }
        if self
            .tunnel_owners
            .values()
            .any(|panel| panel.read(cx).window_close_busy())
        {
            return Some(Message::new(
                "请先在隧道面板停止监听并确认原连接清理。",
                "Stop listeners in the Tunnels panel and confirm cleanup on their original connections.",
            ));
        }
        None
    }

    fn window_close_snapshot(&self, cx: &App) -> Snapshot {
        let mut files: Vec<_> = self
            .panels
            .values()
            .chain(self.archived_panels.values())
            .chain(self.retired_window_panels.iter())
            .filter_map(|panels| panels.files.as_ref())
            .map(|panel| (panel.entity_id(), panel.read(cx).window_close_draft(cx)))
            .collect();
        files.sort_by_key(|(id, _)| *id);
        files.dedup_by_key(|(id, _)| *id);
        let sessions = self.owned_window_sessions.clone();
        Snapshot {
            id: self.window_id,
            native: self.native_window.window_id(),
            tabs: self.tabs.iter().map(Entity::entity_id).collect(),
            pane_revision: self.pane_generation,
            sessions,
            drafts: Drafts {
                command: self.command.read(cx).value(),
                command_revision: self.command_revision,
                quick: [
                    &self.quick_connect.host,
                    &self.quick_connect.port,
                    &self.quick_connect.username,
                    &self.quick_connect.key,
                ]
                .into_iter()
                .map(|field| field.read(cx).value().to_string())
                .chain([self.quick_password.to_string()])
                .collect(),
                form: self.form.as_ref().map(|form| form.signature(cx)),
                files,
                assistant: self.assistant.read(cx).window_close_draft(cx),
            },
        }
    }

    pub(crate) fn request_window_close(
        &mut self,
        intent: CloseIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_review.is_some()
            || self.cleanup_unknown.is_some()
            || self.app_windows.read(cx).quitting
        {
            self.focus_current_surface(window, cx);
            return;
        }
        self.remember_window_workers(cx);
        let registrations = self.app_windows.read(cx).windows.clone();
        let mut snapshots = Vec::new();
        for registration in registrations
            .iter()
            .filter(|entry| intent != CloseIntent::Window || entry.id == self.window_id)
        {
            let (blocker, snapshot) = if registration.id == self.window_id {
                (
                    self.window_close_blocker(cx),
                    self.window_close_snapshot(cx),
                )
            } else if let Some(other) = registration.workspace.upgrade() {
                let other = other.read(cx);
                (
                    other.window_close_blocker(cx),
                    other.window_close_snapshot(cx),
                )
            } else {
                self.status = Message::new(
                    "窗口 owner 已变化，请重新审核。",
                    "Window ownership changed. Review again.",
                );
                cx.notify();
                return;
            };
            if let Some(blocker) = blocker {
                self.status = Message::new(
                    format!(
                        "窗口 {}：{}",
                        &registration.id.to_string()[..8],
                        blocker.render(cx)
                    ),
                    format!(
                        "Window {}: {}",
                        &registration.id.to_string()[..8],
                        blocker.render(cx)
                    ),
                );
                self.focus_current_surface(window, cx);
                cx.notify();
                return;
            }
            snapshots.push(snapshot);
        }
        self.close_review = Some(CloseReview {
            id: uuid::Uuid::new_v4(),
            intent,
            windows: snapshots,
        });
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn cancel_window_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing.is_none() {
            self.close_review = None;
            if self.cleanup_unknown.take().is_some() {
                self.status = Message::new(
                    "已保留本窗；未知清理结果仍需在下一次关闭前重新核对。",
                    "This window is retained; reconcile the unknown cleanup result before the next close.",
                );
            }
            self.focus_current_surface(window, cx);
            cx.notify();
        }
    }

    pub(super) fn retry_unknown_window_cleanup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(unknown) = self.cleanup_unknown.take() else {
            return;
        };
        self.close_review = None;
        self.closing = None;
        self.closing_intent = None;
        self.status = Message::new(
            "已确认清理结果未知，正在重新生成精确关闭审核。",
            "Cleanup is acknowledged as unknown; generating a fresh exact close review.",
        );
        self.request_window_close(unknown.intent, window, cx);
    }

    pub(super) fn confirm_window_close(
        &mut self,
        ticket: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(review) = self
            .close_review
            .as_ref()
            .filter(|review| review.id == ticket)
        else {
            return;
        };
        let registrations = self.app_windows.read(cx).windows.clone();
        let current_ids: Vec<_> = registrations
            .iter()
            .filter(|entry| review.intent != CloseIntent::Window || entry.id == self.window_id)
            .map(|entry| entry.id)
            .collect();
        let expected_ids: Vec<_> = review.windows.iter().map(|snapshot| snapshot.id).collect();
        let valid = current_ids == expected_ids
            && review.windows.iter().all(|snapshot| {
                if snapshot.id == self.window_id {
                    self.window_close_blocker(cx).is_none()
                        && snapshot.same(&self.window_close_snapshot(cx))
                } else {
                    registrations
                        .iter()
                        .find(|entry| entry.id == snapshot.id)
                        .and_then(|entry| entry.workspace.upgrade())
                        .is_some_and(|other| {
                            let other = other.read(cx);
                            other.close_review.is_none()
                                && other.window_close_blocker(cx).is_none()
                                && snapshot.same(&other.window_close_snapshot(cx))
                        })
                }
            });
        if !valid {
            self.close_review = None;
            self.status = Message::new(
                "关闭审核已过期；窗口、会话或精确草稿 / 格式已变化，请重新审核。",
                "Close review is stale: a window, session or exact draft / format changed. Review again.",
            );
            self.focus_current_surface(window, cx);
            cx.notify();
            return;
        }
        let intent = review.intent;
        let ids = expected_ids;
        if intent != CloseIntent::Window {
            self.app_windows.update(cx, |shared, _| {
                shared.begin_aggregate_cleanup(&ids, intent);
            });
        }
        if intent == CloseIntent::Install {
            let updater = self.update_service.clone();
            if !updater.update(cx, |panel, cx| panel.install_reviewed(window, cx)) {
                self.app_windows.update(cx, |shared, _| {
                    shared.quitting = false;
                    shared.quit_install = false;
                    shared.cleanup_attempt = None;
                    shared.cleanup_intent = None;
                    shared.cleanup_participants.clear();
                    shared.cleanup_aborted = false;
                });
                self.close_review = None;
                self.focus_current_surface(window, cx);
                cx.notify();
                return;
            }
        }
        // Validate the entire set before changing any window. All participating
        // views revoke authority in this foreground turn, before workers run.
        for registration in registrations
            .iter()
            .filter(|entry| ids.contains(&entry.id) && entry.id != self.window_id)
        {
            let _ = registration
                .workspace
                .update(cx, |other, cx| other.begin_window_cleanup(intent, cx));
        }
        self.begin_window_cleanup(intent, cx);
    }

    pub(super) fn remember_window_workers(&mut self, cx: &App) {
        self.worker_owners
            .extend(self.tabs.iter().map(Entity::entity_id));
        for panels in self
            .panels
            .values()
            .chain(self.archived_panels.values())
            .chain(self.retired_window_panels.iter())
        {
            if let Some(files) = &panels.files {
                self.worker_owners.insert(files.entity_id());
            }
            if let Some(monitor) = &panels.monitor {
                self.worker_owners.insert(monitor.entity_id());
                self.worker_owners
                    .extend(monitor.read(cx).window_worker_owners(cx));
            }
            if let Some(tunnels) = &panels.tunnels {
                self.worker_owners.insert(tunnels.entity_id());
            }
        }
        self.worker_owners
            .extend(self.tunnel_owners.values().map(Entity::entity_id));
        for session in self.remote_sessions.values() {
            if !self
                .owned_window_sessions
                .iter()
                .any(|old| old.same_connection(session))
            {
                self.owned_window_sessions.push(session.clone());
            }
        }
    }

    fn begin_window_cleanup(&mut self, intent: CloseIntent, cx: &mut Context<Self>) {
        self.remember_window_workers(cx);
        let attempt = self.app_windows.update(cx, |shared, _| {
            if intent == CloseIntent::Window {
                shared.begin_window_cleanup(self.window_id)
            } else {
                shared
                    .cleanup_attempt
                    .unwrap_or_else(|| shared.begin_aggregate_cleanup(&[self.window_id], intent))
            }
        });
        self.close_review = None;
        self.cleanup_unknown = None;
        self.closing_intent = Some(intent);
        self.closing = Some(Message::new(
            "正在确认本窗连接和 worker 回收…",
            "Confirming this window's connection and worker cleanup…",
        ));
        self.invalidate_window_reconnect();
        self.cancel_remote_completion(cx);
        self.parameter_ticket = None;
        self.suggestion_cache.clear();
        self.assistant
            .update(cx, |panel, cx| panel.invalidate_knowledge_review(cx));
        self.agent = None;
        for terminal in &self.tabs {
            terminal.update(cx, |terminal, _| terminal.revoke_window_input());
        }
        for panels in self
            .panels
            .values()
            .chain(self.archived_panels.values())
            .chain(self.retired_window_panels.iter())
        {
            if let Some(files) = &panels.files {
                files.update(cx, |panel, cx| panel.suspend(cx));
            }
            if let Some(monitor) = &panels.monitor {
                monitor.update(cx, |panel, cx| panel.suspend(cx));
            }
            if let Some(tunnels) = &panels.tunnels {
                tunnels.update(cx, |panel, cx| panel.suspend(cx));
            }
        }
        let sessions = self.owned_window_sessions.clone();
        let owners: Vec<_> = self.worker_owners.iter().copied().collect();
        let runtime = self.runtime.clone();
        let deadline = Instant::now() + Duration::from_secs(8);
        let requested = crate::terminal::request_stop_window_workers(&owners);
        let task = cx.background_executor().spawn(async move {
            requested?;
            let connections = runtime.block_on(async {
                let mut jobs = tokio::task::JoinSet::new();
                for session in sessions {
                    if !session.is_closed() {
                        jobs.spawn(async move {
                            let close = session.close().await;
                            while !session.is_closed() {
                                if Instant::now() >= deadline {
                                    return Err(close.err().map_or_else(
                                        || "SSH close returned without a closed connection".into(),
                                        |error| format!("SSH cleanup unconfirmed: {error}"),
                                    ));
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                            Ok(())
                        });
                    }
                }
                while !jobs.is_empty() {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    match tokio::time::timeout(remaining, jobs.join_next()).await {
                        Ok(Some(Ok(Ok(())))) => {}
                        Ok(Some(Ok(Err(error)))) => return Err(error),
                        Ok(Some(Err(error))) => {
                            return Err(format!("SSH cleanup worker failed: {error}"));
                        }
                        Ok(None) => break,
                        Err(_) => {
                            return Err(
                                "window SSH cleanup deadline elapsed; owner retained".into()
                            );
                        }
                    }
                }
                Ok(())
            });
            connections?;
            crate::terminal::stop_window_workers(&owners, deadline)
        });
        let native = self.native_window;
        let attempt = Some(attempt);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = native.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| {
                    view.finish_window_cleanup_for_attempt(attempt, result, window, cx);
                });
            });
        })
        .detach();
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn finish_window_cleanup(
        &mut self,
        result: Result<Vec<String>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let attempt = self
            .closing_intent
            .filter(|intent| *intent != CloseIntent::Window)
            .and_then(|_| {
                self.app_windows
                    .read(cx)
                    .cleanup_attempt_for(self.window_id)
            });
        self.finish_window_cleanup_for_attempt(attempt, result, window, cx);
    }

    pub(super) fn finish_window_cleanup_for_attempt(
        &mut self,
        attempt: Option<uuid::Uuid>,
        result: Result<Vec<String>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(attempt) = attempt
            && self
                .app_windows
                .read(cx)
                .cleanup_attempt_for(self.window_id)
                != Some(attempt)
        {
            // A completion from an abandoned cleanup must never mutate a
            // newer close review or remove its owner.
            return;
        }
        let intent = self.closing_intent.or_else(|| {
            let shared = self.app_windows.read(cx);
            (attempt.is_some()
                && shared.cleanup_attempt == attempt
                && shared.cleanup_participants.contains(&self.window_id))
            .then_some(shared.cleanup_intent)
            .flatten()
        });
        let intent = intent.unwrap_or(CloseIntent::Window);
        match result {
            Err(error) => {
                self.closing_intent = None;
                self.closing = None;
                self.cleanup_unknown = Some(CleanupUnknown {
                    intent,
                    message: Message::detail(
                        "清理结果未知；owner 保留，请核对原连接；未自动重试",
                        "Cleanup unknown; owner retained. Inspect the original connection; no automatic retry",
                        error,
                    ),
                });
                if intent != CloseIntent::Window {
                    let registrations = self.app_windows.read(cx).windows.clone();
                    let retained_peers = self.app_windows.update(cx, |shared, _| {
                        shared.abort_aggregate_cleanup(attempt, self.window_id)
                    });
                    for registration in registrations {
                        if retained_peers.contains(&registration.id)
                            && let Some(peer) = registration.workspace.upgrade()
                        {
                            peer.update(cx, |view, cx| {
                                if view.cleanup_unknown.is_none() {
                                    view.closing = None;
                                    view.closing_intent = None;
                                    view.status = Message::new(
                                        "退出/安装已暂停；另一窗口清理结果未知，请保留并重新审核。",
                                        "Quit/install paused because another window's cleanup is unknown; retain it and review again.",
                                    );
                                }
                                cx.notify();
                            });
                        }
                    }
                }
                self.status = Message::new(
                    "清理结果未知；请核对原连接后重新审核，系统不会自动重试。",
                    "Cleanup is unknown. Inspect the original connection and explicitly review again; no automatic retry will run.",
                );
                cx.notify();
                return;
            }
            Ok(failures) => {
                if intent != CloseIntent::Window
                    && self.app_windows.read(cx).aggregate_cleanup_aborted(attempt)
                {
                    self.closing = None;
                    self.closing_intent = None;
                    self.status = Message::new(
                        "退出/安装已暂停；本窗清理随后完成，但 owner 仍需重新审核。",
                        "Quit/install paused; this window finished cleanup later, but its owner remains until a fresh review.",
                    );
                    cx.notify();
                    return;
                }
                self.closing = Some(if failures.is_empty() {
                    Message::new(
                        "本窗 worker 已 join，SSH 已关闭。",
                        "This window's workers were joined and SSH closed.",
                    )
                } else {
                    Message::new(
                        format!(
                            "本窗 worker 已 join，SSH 已关闭；{} 个 worker 返回失败回执，请检查状态。",
                            failures.len()
                        ),
                        format!(
                            "This window's workers were joined and SSH closed; {} worker failure receipts were reported. Inspect their status.",
                            failures.len()
                        ),
                    )
                });
                self.closing_intent = None;
            }
        }
        let (quitting, all_clean, group_window_count) = self.app_windows.update(cx, |shared, _| {
            shared.cleaned.insert(self.window_id);
            let all_clean = shared
                .windows
                .iter()
                .all(|entry| shared.cleaned.contains(&entry.id));
            (shared.quitting, all_clean, shared.windows.len())
        });
        let last = crate::app_windows::is_last_window(
            group_window_count,
            crate::app_windows::total_window_count(cx),
        );
        if quitting {
            if all_clean {
                crate::terminal::shutdown_and_quit(cx);
            }
        } else if last {
            crate::terminal::shutdown_and_quit(cx);
        } else {
            self.app_windows.update(cx, |shared, _| {
                shared.windows.retain(|entry| entry.id != self.window_id);
                if shared.update_owner == Some(self.window_id) {
                    shared.update_owner = None;
                }
            });
            window.remove_window();
        }
    }

    pub(super) fn window_close_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = div()
            .text_lg()
            .child(t(cx, "关闭窗口审核", "Review window close"))
            .into_any_element();
        let mut body = div().flex().flex_col().gap_3();
        if let Some(unknown) = &self.cleanup_unknown {
            body = body.child(unknown.message.render(cx)).child(t(
                cx,
                "请核对原连接和 worker 状态，再选择重新审核；也可以保留窗口继续处理。",
                "Inspect the original connection and worker state, then choose a fresh review. You can also retain the window and continue working.",
            ));
        } else if let Some(message) = &self.closing {
            body = body.child(message.render(cx));
        } else if let Some(review) = &self.close_review {
            body = body.child(t(cx,
                "确认后将丢弃这些窗口的未保存草稿、撤销审核，并关闭各自的 SSH。不会自动保存布局。",
                "Confirmation discards these windows' unsaved drafts, revokes reviews and closes their own SSH. Layouts are not saved automatically.",
            ));
            for snapshot in &review.windows {
                body = body.child(format!(
                    "{} · {} SSH · {} {}",
                    &snapshot.id.to_string()[..8],
                    snapshot.sessions.len(),
                    snapshot.drafts.files.len(),
                    t(cx, "文件草稿快照", "file draft snapshots")
                ));
            }
        }
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                Button::new("window-close-cancel")
                    .label(if self.cleanup_unknown.is_some() {
                        t(cx, "保留窗口", "Keep window")
                    } else {
                        t(cx, "取消", "Cancel")
                    })
                    .disabled(self.closing.is_some())
                    .on_click(
                        cx.listener(|view, _, window, cx| view.cancel_window_close(window, cx)),
                    ),
            )
            .when(self.cleanup_unknown.is_some(), |footer| {
                footer.child(
                    Button::new("window-close-recover")
                        .label(t(cx, "已核对，重新审核", "Acknowledge and review again"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.retry_unknown_window_cleanup(window, cx)
                        })),
                )
            })
            .when_some(
                self.close_review.as_ref().map(|review| review.id),
                |footer, id| {
                    footer.child(
                        Button::new("window-close-confirm")
                            .danger()
                            .label(t(cx, "确认关闭", "Confirm close"))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.confirm_window_close(id, window, cx)
                            })),
                    )
                },
            )
            .into_any_element();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(modals::framed_modal(
                cx,
                "window-close-review",
                px(420.),
                title,
                body.into_any_element(),
                footer,
            ))
            .into_any_element()
    }
}
