//! Recovery owns a fresh route and a fresh terminal identity; no operation is replayed.
use super::*;
use keelshell_core::{ConnectionRoute, ReconnectPolicy};
use keelshell_session::{ConnectionEnd, ShellEnd};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Ticket {
    pub(super) tab: EntityId,
    pub(super) series: uuid::Uuid,
}

#[derive(Clone)]
struct Series {
    id: uuid::Uuid,
    due: Option<Instant>,
    background: bool,
    discard_archive: Option<(EntityId, SharedString)>,
}

#[derive(Clone)]
pub(super) struct TabBinding {
    route: ConnectionRoute,
    trust: Vec<Option<String>>,
    policy: ReconnectPolicy,
    ended: bool,
    suppressed: bool,
    ready_since: Option<Instant>,
    attempts: usize,
    series: Option<Series>,
    notice: Message,
}

impl TabBinding {
    fn observe_terminal(&mut self, end: Option<ShellEnd>, ready: bool, now: Instant) -> bool {
        let mut newly_ended = false;
        if let Some(reason) = end {
            if !self.ended {
                self.ended = true;
                self.ready_since = None;
                newly_ended = true;
                self.notice = Message::new(
                    "会话已结束，旧输出和任务记录已保留。",
                    "Session ended. Output and task records are retained.",
                );
                let recoverable = matches!(
                    reason,
                    ShellEnd::ConnectionClosed(
                        ConnectionEnd::TransportLost | ConnectionEnd::KeepaliveTimeout
                    )
                );
                if recoverable && !self.suppressed && !self.policy.is_manual() {
                    if let Some(delay) = self.policy.retry_delay(self.attempts + 1) {
                        self.series = Some(Series {
                            id: uuid::Uuid::new_v4(),
                            due: Some(now + delay),
                            background: true,
                            discard_archive: None,
                        });
                        self.notice = Message::new(
                            "等待自动重连；不会恢复旧操作。",
                            "Waiting to reconnect; previous operations will not resume.",
                        );
                    } else {
                        self.notice = Message::new(
                            "已达到连续重连次数上限，请手动重连。",
                            "Consecutive reconnect limit reached. Reconnect manually.",
                        );
                    }
                }
            }
        } else if ready {
            let since = self.ready_since.get_or_insert(now);
            if now.duration_since(*since) >= Duration::from_secs(30) {
                self.attempts = 0;
                self.suppressed = false;
            }
        }
        newly_ended
    }

    fn begin_attempt(&mut self) -> Option<bool> {
        if self.attempts >= maximum(self.policy) {
            return None;
        }
        let series = self.series.as_mut()?;
        series.due = None;
        self.attempts += 1;
        Some(series.background)
    }

    pub(super) fn for_split(mut self) -> Self {
        self.ended = false;
        self.suppressed = false;
        self.ready_since = None;
        self.attempts = 0;
        self.series = None;
        self
    }
}

fn trust_snapshot(state: &AppState, route: &ConnectionRoute) -> Vec<Option<String>> {
    (0..route.hops().len())
        .map(|index| {
            route
                .host_key_scope(index)
                .and_then(|scope| state.host_key_for_scope(&scope).map(str::to_owned))
        })
        .collect()
}

fn maximum(policy: ReconnectPolicy) -> usize {
    match policy {
        ReconnectPolicy::Manual => 1,
        ReconnectPolicy::Automatic { max_attempts, .. } => max_attempts.into(),
    }
}

impl Workspace {
    pub(super) fn batch_route_description(&self, id: EntityId) -> Option<(String, String)> {
        let route = &self.reconnect_bindings.get(&id)?.route;
        let name = route.hops().last()?.name.clone();
        let description = route
            .hops()
            .iter()
            .map(|hop| {
                let endpoint = crate::jump_host_picker::endpoint(hop);
                match &hop.proxy {
                    Some(proxy) => format!(
                        "{} → {endpoint}",
                        crate::proxy_editor::proxy_endpoint(proxy)
                    ),
                    None => endpoint,
                }
            })
            .collect::<Vec<_>>()
            .join(" → ");
        Some((name, description))
    }

    /// Return immutable, non-secret metadata for local batch template rendering.
    /// Quick connections have no saved route, so callers use a conservative
    /// fallback context based on the visible session label.
    pub(super) fn batch_template_context(
        &self,
        id: EntityId,
    ) -> Option<keelshell_core::BatchTargetContext> {
        let target = self.reconnect_bindings.get(&id)?.route.hops().last()?;
        Some(keelshell_core::BatchTargetContext {
            name: target.name.clone(),
            host: target.host.clone(),
            port: target.port.to_string(),
            user: target.username.clone(),
            endpoint: crate::jump_host_picker::endpoint(target),
        })
    }

    /// Return the saved target profile identity for audit metadata. Ephemeral
    /// quick connections have no binding and therefore return `None`.
    pub(super) fn batch_profile_id(&self, id: EntityId) -> Option<uuid::Uuid> {
        self.reconnect_bindings
            .get(&id)
            .and_then(|binding| binding.route.hops().last().map(|hop| hop.id))
    }
    pub(super) fn bind_remote_tab(&mut self, id: EntityId, route: ConnectionRoute) {
        let policy = route
            .hops()
            .last()
            .map(|target| target.reconnect)
            .unwrap_or_default();
        let trust = trust_snapshot(&self.state, &route);
        self.reconnect_bindings.insert(
            id,
            TabBinding {
                route,
                trust,
                policy,
                ended: false,
                suppressed: false,
                ready_since: None,
                attempts: 0,
                series: None,
                notice: Message::empty(),
            },
        );
    }

    fn binding_current(&self, binding: &TabBinding) -> bool {
        binding.route.hops().last().is_some_and(|target| {
            self.state.connection_route(target.id).is_ok_and(|current| {
                binding.route.same_reconnect_target(&current)
                    && binding.trust == trust_snapshot(&self.state, &current)
            })
        })
    }

    pub(super) fn reconnect_ticket_current(&self, ticket: Ticket) -> bool {
        self.tabs.iter().any(|tab| tab.entity_id() == ticket.tab)
            && self
                .reconnect_bindings
                .get(&ticket.tab)
                .is_some_and(|binding| {
                    binding
                        .series
                        .as_ref()
                        .is_some_and(|series| series.id == ticket.series)
                        && self.binding_current(binding)
                })
    }

    fn archive_dirty(&self, id: EntityId, cx: &App) -> bool {
        self.archived_panels
            .get(&id)
            .and_then(|panels| panels.files.as_ref())
            .is_some_and(|files| files.read(cx).has_unsaved_draft(cx))
    }

    fn archive_can_replace(
        &self,
        id: EntityId,
        consent: Option<&(EntityId, SharedString)>,
        cx: &App,
    ) -> bool {
        !self.archive_dirty(id, cx)
            || self
                .archived_panels
                .get(&id)
                .and_then(|panels| panels.files.as_ref())
                .is_some_and(|files| {
                    consent.is_some_and(|(entity, text)| {
                        *entity == files.entity_id() && *text == files.read(cx).draft_snapshot(cx)
                    })
                })
    }

    fn suspend_panels(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if let Some(panels) = self.panels.get(&id) {
            if let Some(panel) = &panels.files {
                panel.update(cx, |panel, cx| panel.suspend(cx));
            }
            if let Some(panel) = &panels.monitor {
                panel.update(cx, |panel, cx| panel.suspend(cx));
            }
            if let Some(panel) = &panels.tunnels {
                panel.update(cx, |panel, cx| panel.suspend(cx));
            }
        }
        self.assistant.update(cx, |assistant, cx| {
            assistant.invalidate_session(&format!("{id:?}"), cx)
        });
        self.command_revision = self.command_revision.wrapping_add(1);
        self.suggestion_request = None;
        self.suggestion_cache.clear();
    }

    pub(super) fn poll_reconnect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let terminals = self.tabs.clone();
        for terminal in terminals {
            let id = terminal.entity_id();
            let end = terminal.read(cx).end_reason();
            let ready = terminal.read(cx).is_open();
            let newly_ended = self
                .reconnect_bindings
                .get_mut(&id)
                .is_some_and(|binding| binding.observe_terminal(end, ready, now));
            if newly_ended {
                self.suspend_panels(id, cx);
                cx.notify();
            }
        }
        self.invalidate_reconnect_profiles(window, cx);
        if self.connect_route.is_some()
            || self.connecting
            || self.saving
            || self.command_surface_blocked()
            || self.discard_archive.is_some()
        {
            return;
        }
        let due = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let id = tab.entity_id();
                let binding = self.reconnect_bindings.get(&id)?;
                let series = binding.series.as_ref()?;
                series.due.filter(|due| *due <= now).map(|_| Ticket {
                    tab: id,
                    series: series.id,
                })
            })
            .next();
        if let Some(ticket) = due {
            self.start_reconnect_attempt(ticket, window, cx);
        }
    }

    pub(super) fn request_reconnect(
        &mut self,
        id: EntityId,
        discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connect_route.is_some() || self.saving || self.command_surface_blocked() {
            return;
        }
        if self
            .tabs
            .iter()
            .find(|tab| tab.entity_id() == id)
            .is_none_or(|tab| tab.read(cx).end_reason().is_none())
        {
            return;
        }
        if self.archive_dirty(id, cx) && !discard {
            self.discard_archive = Some(id);
            self.overlay_focus.focus(window, cx);
            cx.notify();
            return;
        }
        let Some(binding) = self.reconnect_bindings.get(&id) else {
            return;
        };
        if !self.binding_current(binding) {
            self.status = Message::new(
                "配置、路线或服务器信任已变化，请从连接管理器重新连接。",
                "Profile, route or server trust changed. Start a new connection from the connection manager.",
            );
            cx.notify();
            return;
        }
        let consent = discard
            .then(|| {
                self.archived_panels
                    .get(&id)
                    .and_then(|panels| panels.files.as_ref())
                    .map(|files| (files.entity_id(), files.read(cx).draft_snapshot(cx)))
            })
            .flatten();
        self.suspend_panels(id, cx);
        let Some(binding) = self.reconnect_bindings.get_mut(&id) else {
            return;
        };
        binding.ended = true;
        // A deliberate user action starts a new bounded series. Automatic short-lived
        // connections keep their accumulated budget until 30 seconds of readiness.
        binding.attempts = 0;
        binding.suppressed = false;
        let ticket = Ticket {
            tab: id,
            series: uuid::Uuid::new_v4(),
        };
        binding.series = Some(Series {
            id: ticket.series,
            due: Some(Instant::now()),
            background: false,
            discard_archive: consent,
        });
        self.start_reconnect_attempt(ticket, window, cx);
    }

    fn start_reconnect_attempt(
        &mut self,
        ticket: Ticket,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.reconnect_ticket_current(ticket) {
            return;
        }
        let consent = self
            .reconnect_bindings
            .get(&ticket.tab)
            .and_then(|binding| binding.series.as_ref())
            .and_then(|series| series.discard_archive.as_ref());
        if !self.archive_can_replace(ticket.tab, consent, cx) {
            self.stop_reconnect(ticket.tab, window, cx);
            if let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab) {
                binding.notice = Message::new(
                    "上次会话仍有文件草稿，请先查看；再次重连需明确处理旧快照。",
                    "The previous session has an unsaved file draft. Review it before reconnecting again.",
                );
            }
            cx.notify();
            return;
        }
        if self
            .tabs
            .iter()
            .find(|tab| tab.entity_id() == ticket.tab)
            .is_none_or(|tab| !tab.read(cx).ready_to_archive())
        {
            if let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab) {
                binding.notice = Message::new(
                    "正在收取上一会话的剩余输出，随后重连…",
                    "Draining the previous session output before reconnecting…",
                );
            }
            cx.notify();
            return;
        }
        let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab) else {
            return;
        };
        let Some(background) = binding.begin_attempt() else {
            return;
        };
        binding.notice = Message::new(
            format!(
                "正在重连 · 第 {} / {} 次",
                binding.attempts,
                maximum(binding.policy)
            ),
            format!(
                "Reconnecting · attempt {} / {}",
                binding.attempts,
                maximum(binding.policy)
            ),
        );
        let Some(target) = binding.route.hops().last() else {
            return;
        };
        let Some(connection) = self
            .state
            .connections
            .iter()
            .find(|connection| connection.id == target.id)
            .cloned()
        else {
            return;
        };
        if self.begin_connect_route(&connection, cx) {
            if let Some(route) = &mut self.connect_route {
                route.reconnect = Some(ticket);
                route.background = background;
            }
            self.prepare_route_hop(window, cx);
        }
        cx.notify();
    }

    pub(super) fn reconnect_failed(
        &mut self,
        ticket: Ticket,
        error: &SessionError,
        cx: &mut Context<Self>,
    ) {
        if !self.reconnect_ticket_current(ticket) {
            return;
        }
        let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab) else {
            return;
        };
        if error.is_retryable()
            && let Some(delay) = binding.policy.retry_delay(binding.attempts + 1)
        {
            if let Some(series) = &mut binding.series {
                series.due = Some(Instant::now() + delay);
            }
            binding.notice = Message::new(
                "本次重连失败，等待下一次尝试。",
                "Reconnect failed; waiting for the next attempt.",
            );
        } else {
            binding.series = None;
            binding.suppressed = true;
            binding.notice = Message::detail(
                "重连已停止，请检查后手动重试",
                "Reconnect stopped; inspect before retrying",
                error,
            );
        }
        cx.notify();
    }

    pub(super) fn reconnect_needs_interaction(&mut self, cx: &mut Context<Self>) {
        if let Some(ticket) = self
            .connect_route
            .as_ref()
            .and_then(|route| route.reconnect)
            && let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab)
        {
            binding.notice = Message::new(
                "重连需要认证或核对服务器，请点击继续。",
                "Reconnect needs authentication or server verification. Click Continue.",
            );
        }
        cx.notify();
    }

    fn continue_reconnect(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_surface_blocked() || self.saving {
            return;
        }
        let Some(route) = &mut self.connect_route else {
            return;
        };
        if route.reconnect.is_none_or(|ticket| ticket.tab != id) || !route.awaiting_interaction {
            return;
        }
        route.background = false;
        route.awaiting_interaction = false;
        if let Some(approval) = route.deferred_approval.take() {
            self.host_approval = Some(approval);
            self.focus_current_surface(window, cx);
        } else {
            self.prepare_route_hop(window, cx);
        }
        cx.notify();
    }

    pub(super) fn cancel_reconnect_ticket(&mut self, ticket: Ticket) {
        if let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab)
            && binding
                .series
                .as_ref()
                .is_some_and(|series| series.id == ticket.series)
        {
            binding.series = None;
            binding.suppressed = true;
            binding.notice = Message::new(
                "自动重连已停止，可随时手动重连。",
                "Reconnection stopped. You can reconnect manually.",
            );
        }
    }

    pub(super) fn stop_reconnect(
        &mut self,
        id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .connect_route
            .as_ref()
            .is_some_and(|route| route.reconnect.is_some_and(|ticket| ticket.tab == id))
        {
            self.cancel_connect_route(window, cx);
        }
        if let Some(binding) = self.reconnect_bindings.get_mut(&id) {
            binding.series = None;
            binding.suppressed = true;
            binding.notice = Message::new(
                "重连已停止，可手动重试。",
                "Reconnection stopped. Retry manually when ready.",
            );
        }
        cx.notify();
    }

    pub(super) fn forget_reconnect(
        &mut self,
        id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_reconnect(id, window, cx);
        self.reconnect_bindings.remove(&id);
        self.archived_panels.remove(&id);
        self.show_archived.remove(&id);
        if self.discard_archive == Some(id) {
            self.discard_archive = None;
        }
    }

    pub(super) fn cancel_reconnect_for_profile(
        &mut self,
        profile: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<_> = self
            .reconnect_bindings
            .iter()
            .filter(|(_, binding)| binding.route.hops().iter().any(|hop| hop.id == profile))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.stop_reconnect(id, window, cx);
        }
    }

    pub(super) fn invalidate_reconnect_profiles(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<_> = self
            .reconnect_bindings
            .iter()
            .filter(|(_, binding)| {
                binding.series.is_some()
                    && (!self.binding_current(binding)
                        || binding
                            .route
                            .hops()
                            .last()
                            .and_then(|target| {
                                self.state
                                    .connections
                                    .iter()
                                    .find(|profile| profile.id == target.id)
                            })
                            .is_some_and(|profile| profile.reconnect != binding.policy))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.stop_reconnect(id, window, cx);
            if let Some(binding) = self.reconnect_bindings.get_mut(&id) {
                binding.notice = Message::new(
                    "路线、策略或服务器信任已变化，重连已取消。",
                    "Route, policy or server trust changed; reconnection cancelled.",
                );
            }
        }
        for binding in self.reconnect_bindings.values_mut() {
            if let Some(target) = binding.route.hops().last()
                && let Some(profile) = self
                    .state
                    .connections
                    .iter()
                    .find(|profile| profile.id == target.id)
            {
                binding.policy = profile.reconnect;
            }
        }
    }

    pub(super) fn accept_reconnect_trust(&mut self, attempt: uuid::Uuid, index: usize) {
        let Some(approval) = self
            .host_approval
            .as_ref()
            .filter(|approval| approval.attempt == attempt && approval.index == index)
        else {
            return;
        };
        let Some(ticket) = self
            .connect_route
            .as_ref()
            .and_then(|route| route.reconnect)
        else {
            return;
        };
        if let Some(binding) = self.reconnect_bindings.get_mut(&ticket.tab)
            && binding
                .series
                .as_ref()
                .is_some_and(|series| series.id == ticket.series)
            && let Some(pin) = binding.trust.get_mut(index)
        {
            *pin = Some(approval.fingerprint.clone());
        }
    }

    pub(super) fn finish_reconnect(
        &mut self,
        ticket: Ticket,
        session: SshSession,
        route: ConnectionRoute,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.reconnect_ticket_current(ticket) {
            return false;
        }
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.entity_id() == ticket.tab)
        else {
            return false;
        };
        let consent = self
            .reconnect_bindings
            .get(&ticket.tab)
            .and_then(|binding| binding.series.as_ref())
            .and_then(|series| series.discard_archive.as_ref());
        if !self.archive_can_replace(ticket.tab, consent, cx) {
            self.stop_reconnect(ticket.tab, window, cx);
            self.status = Message::new(
                "上次快照的草稿已变化，本次重连未替换标签；请重新核对。",
                "The archived draft changed. Reconnection did not replace the tab; review it again.",
            );
            return false;
        }
        let old = self.tabs[index].clone();
        let title = old.read(cx).title.clone();
        let focused = old.read(cx).focus_handle(cx).is_focused(window);
        let Some(connection) = route.hops().last() else {
            return false;
        };
        let host = format!(
            "{}@{}:{}",
            connection.username, connection.host, connection.port
        );
        let terminal = crate::ssh_bridge::open(
            session.clone(),
            self.runtime.clone(),
            title,
            self.state.settings.font_size,
            self.state.settings.scrollback_lines,
            cx,
        );
        let history = old.update(cx, |terminal, _| terminal.take_reconnect_history());
        terminal.update(cx, |terminal, cx| {
            terminal.inherit_reconnect_history(history, cx)
        });
        let id = terminal.entity_id();
        self.suspend_panels(ticket.tab, cx);
        self.remote_sessions.remove(&ticket.tab);
        self.remote_hosts.remove(&ticket.tab);
        self.terminal_observers.remove(&ticket.tab);
        self.terminal_focus.remove(&ticket.tab);
        self.archived_panels.remove(&ticket.tab);
        if let Some(panels) = self.panels.remove(&ticket.tab) {
            self.archived_panels.insert(id, panels);
        }
        self.show_archived.remove(&ticket.tab);
        self.remote_sessions.insert(id, session.clone());
        self.remote_hosts.insert(id, host.clone());
        if let Some(history) = self.command_histories.remove(&ticket.tab) {
            self.command_histories.insert(id, history);
        }
        if let Some(mut binding) = self.reconnect_bindings.remove(&ticket.tab) {
            binding.route = route;
            binding.trust = trust_snapshot(&self.state, &binding.route);
            binding.series = None;
            binding.ended = false;
            binding.ready_since = None;
            binding.notice = Message::new(
                "SSH 认证完成，正在打开新的远程 Shell；上次操作不会自动恢复。",
                "SSH authenticated; opening a new shell. Previous operations will not resume.",
            );
            self.reconnect_bindings.insert(id, binding);
        }
        self.tabs[index] = terminal.clone();
        if let Some((first, second)) = &mut self.split_pair {
            if *first == ticket.tab {
                *first = id;
            }
            if *second == ticket.tab {
                *second = id;
            }
        }
        self.watch_terminal(&terminal, window, cx);
        let runtime = self.runtime.clone();
        let monitor = cx.new(|cx| {
            crate::monitor::MonitorPanel::new(
                session.clone(),
                host.clone(),
                runtime.clone(),
                window,
                cx,
            )
        });
        let files = cx.new(|cx| crate::files::FilesPanel::new(session, host, runtime, window, cx));
        self.panels.insert(
            id,
            RemotePanels {
                files: Some(files),
                monitor: Some(monitor),
                tunnels: None,
            },
        );
        if focused {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn selected_panels(&self, id: EntityId) -> Option<&RemotePanels> {
        if self.show_archived.contains(&id) {
            self.archived_panels.get(&id)
        } else {
            self.panels.get(&id)
        }
    }

    pub(super) fn review_stale_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_surface_blocked() {
            return;
        }
        let Some(tab) = self
            .tabs
            .get(self.active)
            .filter(|tab| tab.read(cx).is_open())
        else {
            return;
        };
        if self
            .command_target
            .is_none_or(|target| self.tabs.iter().any(|tab| tab.entity_id() == target))
        {
            return;
        }
        let target = tab.entity_id();
        self.set_reviewed_command(
            self.command.read(cx).value().to_string(),
            Some(target),
            window,
            cx,
        );
        self.status = Message::new(
            "已重新绑定当前会话，请核对命令后点击执行。",
            "Reviewed for the current session. Inspect the command, then click Run.",
        );
        cx.notify();
    }

    pub(super) fn reconnect_banner(&self, id: EntityId, cx: &mut Context<Self>) -> AnyElement {
        let Some(binding) = self
            .reconnect_bindings
            .get(&id)
            .filter(|binding| binding.ended)
        else {
            return div().into_any_element();
        };
        let waiting = self.connect_route.as_ref().is_some_and(|route| {
            route.reconnect.is_some_and(|ticket| ticket.tab == id) && route.awaiting_interaction
        });
        div()
            .id(("reconnect-banner", id))
            .test_support()
            .flex_shrink_0()
            .px_3()
            .py_2()
            .bg(rgb(PANEL))
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .child(binding.notice.render(cx)),
            )
            .when(waiting, |body| {
                body.child(
                    Button::new(("continue-reconnect", id))
                        .compact()
                        .label(t(cx, "继续认证", "Continue"))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.continue_reconnect(id, window, cx)
                        })),
                )
            })
            .when(binding.series.is_some(), |body| {
                body.child(
                    Button::new(("cancel-reconnect", id))
                        .ghost()
                        .compact()
                        .label(t(cx, "停止重连", "Stop"))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.stop_reconnect(id, window, cx)
                        })),
                )
            })
            .when(binding.series.is_none(), |body| {
                body.child(
                    Button::new(("reconnect", id))
                        .compact()
                        .label(t(cx, "重新连接", "Reconnect"))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.request_reconnect(id, false, window, cx)
                        })),
                )
            })
            .into_any_element()
    }

    pub(super) fn archive_confirmation(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(id) = self.discard_archive else {
            return div().into_any_element();
        };
        div().track_focus(&self.overlay_focus).absolute().inset_0().occlude().bg(rgba(0x00000066)).flex().items_center().justify_center()
            .child(div().id("discard-session-archive").w(px(460.)).max_w_full().p_4().bg(rgb(PANEL)).rounded_lg().flex().flex_col().gap_3()
                .child(t(cx, "上次会话包含未保存的文件草稿。继续重连成功后会替换该快照；请先复制需要保留的内容。", "The previous session contains an unsaved file draft. A successful reconnect will replace that snapshot. Copy anything you need first."))
                .child(div().flex().flex_wrap().gap_2()
                    .child(Button::new("keep-session-archive").ghost().label(t(cx, "保留并查看", "Keep and view"))
                        .on_click(cx.listener(move |view, _, window, cx| { view.discard_archive = None; view.show_archived.insert(id); view.visible_panel = Some(ToolPanel::Files); view.focus_current_surface(window, cx); cx.notify(); })))
                    .child(Button::new("confirm-discard-session-archive").label(t(cx, "允许替换并重连", "Replace on success"))
                        .on_click(cx.listener(move |view, _, window, cx| { view.discard_archive = None; view.request_reconnect(id, true, window, cx); }))))).into_any_element()
    }
}

#[cfg(test)]
mod budget_tests {
    use super::{Series, TabBinding};
    use crate::i18n::Message;
    use keelshell_core::{AppState, Connection, ReconnectPolicy};
    use keelshell_session::{ConnectionEnd, ShellEnd};
    use std::time::{Duration, Instant};

    fn binding() -> TabBinding {
        let connection = Connection::new("budget", "example.invalid", "test");
        let mut state = AppState::default();
        state.connections.push(connection.clone());
        TabBinding {
            route: state
                .connection_route(connection.id)
                .unwrap_or_else(|error| panic!("valid fixture route: {error:?}")),
            trust: vec![None],
            policy: ReconnectPolicy::Automatic {
                max_attempts: 2,
                initial_delay_seconds: 1,
                max_delay_seconds: 2,
            },
            ended: false,
            suppressed: false,
            ready_since: None,
            attempts: 0,
            series: None,
            notice: Message::empty(),
        }
    }

    #[test]
    fn short_lived_success_does_not_reset_budget_but_thirty_seconds_ready_does() {
        let mut binding = binding();
        let now = Instant::now();
        let loss = || Some(ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost));
        assert!(binding.observe_terminal(loss(), false, now));
        assert_eq!(binding.begin_attempt(), Some(true));
        binding.ended = false;
        binding.series = None;
        binding.observe_terminal(None, true, now + Duration::from_secs(1));
        binding.observe_terminal(None, true, now + Duration::from_secs(29));
        assert_eq!(binding.attempts, 1);
        assert!(binding.observe_terminal(loss(), false, now + Duration::from_secs(30)));
        assert_eq!(binding.begin_attempt(), Some(true));
        binding.ended = false;
        binding.series = None;
        binding.observe_terminal(None, true, now + Duration::from_secs(31));
        assert!(binding.observe_terminal(loss(), false, now + Duration::from_secs(32)));
        assert!(binding.series.is_none());
        assert_eq!(binding.attempts, 2);
        assert_eq!(binding.begin_attempt(), None);
        // A later explicit successful connection must actually stay ready before reset.
        binding.ended = false;
        binding.observe_terminal(None, true, now + Duration::from_secs(33));
        binding.observe_terminal(None, true, now + Duration::from_secs(62));
        assert_eq!(binding.attempts, 2);
        binding.observe_terminal(None, true, now + Duration::from_secs(63));
        assert_eq!(binding.attempts, 0);
    }

    #[test]
    fn cancelled_series_cannot_be_rescheduled_by_repeated_end_notifications() {
        let mut binding = binding();
        let now = Instant::now();
        let loss = || Some(ShellEnd::ConnectionClosed(ConnectionEnd::KeepaliveTimeout));
        binding.observe_terminal(loss(), false, now);
        binding.series = None;
        binding.suppressed = true;
        for offset in 1..20 {
            binding.observe_terminal(loss(), false, now + Duration::from_secs(offset));
        }
        assert!(binding.series.is_none());
        assert_eq!(binding.attempts, 0);
        // Exhausted admission never allocates an extra full-route attempt.
        binding.attempts = 2;
        binding.series = Some(Series {
            id: uuid::Uuid::new_v4(),
            due: Some(now),
            background: true,
            discard_archive: None,
        });
        assert_eq!(binding.begin_attempt(), None);
        assert_eq!(binding.attempts, 2);
    }
}
