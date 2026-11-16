//! Shared store admission without replacing a sibling's live transports or drafts.
use super::*;
use crate::app_windows::ConfigurationGate;

impl Workspace {
    pub(super) fn observe_shared_configuration_if_needed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.configuration_token_current()
            && !self.configuration_invalidated
            && !self.saving
            && !self.state_reloading
            && self.closing.is_none()
        {
            self.observe_shared_configuration(window, cx);
        }
    }
    pub(super) fn configuration_current(&self, _cx: &App) -> bool {
        self.configuration_token_current()
    }

    pub(super) fn configuration_token_current(&self) -> bool {
        self.closing.is_none()
            && !self.configuration_invalidated
            && self
                .configuration_gate
                .lock()
                .is_ok_and(|gate| gate.current(self.state.snapshot, self.configuration_epoch))
    }

    pub(super) fn observe_shared_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observe_shared_configuration_with_options(window, cx, false, false);
    }

    pub(super) fn observe_shared_configuration_preserving_login(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observe_shared_configuration_with_options(window, cx, true, false);
    }

    pub(super) fn observe_shared_configuration_preserving_knowledge_review(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observe_shared_configuration_with_options(window, cx, false, true);
    }

    fn observe_shared_configuration_with_options(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        preserve_login: bool,
        preserve_knowledge_review: bool,
    ) {
        // The local window temporarily observes the new gate epoch between the
        // background read and its foreground adoption. Treating that interval
        // as a sibling commit would revoke the very review that authorized
        // this reload.
        if self.state_reloading {
            return;
        }
        let shared = self.app_windows.read(cx);
        let Ok(gate) = shared.gate.lock() else {
            self.configuration_invalidated = true;
            return;
        };
        let current = gate.current(self.state.snapshot, self.configuration_epoch);
        let settings = gate.state().settings.clone();
        drop(gate);
        if !current && !self.configuration_invalidated && self.closing.is_none() {
            self.configuration_invalidated = true;
            if preserve_login {
                // A vault write can leave an encrypted entry behind while the
                // StateStore commit is rejected. Keep the reviewable login
                // surface so the user can see the durable failure and close it
                // explicitly; the route itself is still invalidated.
                let login = self.login.take();
                self.cancel_connect_route(window, cx);
                self.login = login;
                if let Some(login) = &self.login {
                    login.focus(window, cx);
                }
            } else {
                self.cancel_connect_route(window, cx);
            }
            self.invalidate_window_reconnect();
            self.cancel_remote_completion(cx);
            self.command_revision = self.command_revision.wrapping_add(1);
            self.command_sources_revision = self.command_sources_revision.wrapping_add(1);
            self.suggestion_request = None;
            self.suggestion_cache.clear();
            self.parameter_ticket = None;
            self.openssh_review = None;
            self.assistant.update(cx, |panel, cx| {
                if preserve_knowledge_review {
                    panel.revoke_stale_knowledge_request(cx);
                } else {
                    panel.invalidate_knowledge_review(cx);
                }
            });
            self.revoke_stale_protocol_diagnostics(cx);
            self.maintain_mcp(cx);
            self.maintain_agent(cx);
            // A stale catalog needs an explicit read, not a permanent busy
            // state that would disable the panel's recovery entry point.
            self.mark_tunnel_rule_store_uncertain(cx);
            self.status = Message::new(
                "另一窗口已提交配置；本窗草稿和现有 SSH 保留。请显式重新加载配置，再重新审核配置操作。",
                "Another window committed configuration. Drafts and live SSH are retained. Explicitly reload configuration, then review configuration actions again.",
            );
        }
        // These are application preferences, not adoption of a new authority
        // token. The rest of the local catalog stays stale until explicit reload.
        self.state.settings.theme = settings.theme;
        self.state.settings.theme_family = settings.theme_family;
        self.state.settings.language = settings.language;
        crate::design::apply_settings(&self.state.settings, Some(window), cx);
        self.apply_language(settings.language, window, cx);
        cx.notify();
    }

    pub(super) fn acquire_configuration_write(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crate::app_windows::WriteLease> {
        if self.closing.is_some() || self.configuration_invalidated {
            self.status = Message::new(
                "配置审核已失效，请显式重新加载后再审核。",
                "Configuration review is stale. Explicitly reload and review again.",
            );
            cx.notify();
            return None;
        }
        let gate = self.app_windows.read(cx).gate.clone();
        match ConfigurationGate::begin(
            &gate,
            self.window_id,
            self.state.snapshot,
            self.configuration_epoch,
        ) {
            Ok(lease) => Some(lease),
            Err(detail) => {
                self.status = Message::detail(
                    "配置写入被阻止，草稿已保留",
                    "Configuration write blocked; draft retained",
                    detail,
                );
                cx.notify();
                None
            }
        }
    }

    pub(super) fn shared_commit_completed(&mut self, cx: &mut Context<Self>) {
        if let Ok(gate) = self.app_windows.read(cx).gate.lock()
            && gate.state().snapshot == self.state.snapshot
        {
            self.configuration_epoch = gate.epoch();
            self.configuration_invalidated = false;
        }
        self.app_windows.update(cx, |shared, cx| shared.changed(cx));
    }

    pub(super) fn begin_window_maintenance(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.configuration_current(cx) || self.close_review.is_some() {
            return false;
        }
        let shared = self.app_windows.read(cx);
        let accepted = shared
            .gate
            .lock()
            .is_ok_and(|mut gate| gate.begin_maintenance(self.window_id, shared.windows.len()));
        if !accepted {
            self.status = Message::new(
                "共享配置维护仅允许在单个空闲窗口中进行；请先关闭其他窗口，并完成配置写入。",
                "Shared configuration maintenance requires a single idle window. Close other windows and finish configuration writes first.",
            );
            cx.notify();
        }
        accepted
    }

    pub(super) fn window_maintenance_idle(&self, cx: &App) -> bool {
        self.shared_maintenance_idle(cx) && self.remote_panels_idle(cx, true)
    }

    /// Profile synchronization only replaces saved metadata. Existing file
    /// panels keep their captured authenticated session and transfer queue, so
    /// an in-flight SFTP operation may continue while the sync review owns the
    /// configuration maintenance lease.
    pub(super) fn profile_sync_maintenance_idle(&self, cx: &App) -> bool {
        self.shared_maintenance_idle(cx) && self.remote_panels_idle(cx, false)
    }

    fn shared_maintenance_idle(&self, cx: &App) -> bool {
        !self.saving
            && !self.state_reloading
            && !self.connecting
            && self.route_jobs == 0
            && self.connect_route.is_none()
            && self.closing.is_none()
            && self.close_review.is_none()
            && self.assistant.read(cx).config_recovery_idle()
            && !self.window_agent_busy(cx)
            && self.mcp.config_recovery_idle()
            && self
                .batch_panel
                .as_ref()
                .is_none_or(|panel| !panel.read(cx).is_running())
            && self
                .workflow_panel
                .as_ref()
                .is_none_or(|panel| !panel.read(cx).has_active_run())
    }

    fn remote_panels_idle(&self, cx: &App, include_files: bool) -> bool {
        self.panels
            .values()
            .chain(self.archived_panels.values())
            .chain(self.retired_window_panels.iter())
            .all(|panels| {
                (!include_files
                    || panels
                        .files
                        .as_ref()
                        .is_none_or(|panel| !panel.read(cx).window_close_busy()))
                    && panels
                        .monitor
                        .as_ref()
                        .is_none_or(|panel| !panel.read(cx).window_close_busy(cx))
                    && panels
                        .tunnels
                        .as_ref()
                        .is_none_or(|panel| !panel.read(cx).window_close_busy())
            })
            && self
                .tunnel_owners
                .values()
                .all(|panel| !panel.read(cx).window_close_busy())
    }

    pub(super) fn window_maintenance_current(&self, cx: &App) -> bool {
        self.closing.is_none()
            && self.app_windows.read(cx).windows.len() == 1
            && self
                .app_windows
                .read(cx)
                .gate
                .lock()
                .is_ok_and(|gate| gate.maintenance_owned(self.window_id))
    }

    pub(super) fn end_window_maintenance(&mut self, cx: &mut Context<Self>) {
        if let Ok(mut gate) = self.app_windows.read(cx).gate.lock() {
            gate.end_maintenance(self.window_id);
        }
        self.app_windows.update(cx, |shared, cx| shared.changed(cx));
    }

    pub(super) fn accept_maintenance_commit(
        &mut self,
        state: &AppState,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.window_maintenance_current(cx) {
            return false;
        }
        self.app_windows
            .read(cx)
            .gate
            .lock()
            .is_ok_and(|mut gate| gate.maintenance_committed(self.window_id, state))
    }

    pub(super) fn reload_window_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_window_configuration_reload(None, window, cx);
    }

    pub(super) fn begin_window_configuration_reload(
        &mut self,
        origin: Option<tunnel_reload::TunnelReloadOrigin>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.saving
            || self.state_reloading
            || self.closing.is_some()
            || self.active_modal().is_some()
            || self.connecting
            || self.connect_route.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.configuration_recovery.is_some()
            || !self
                .app_windows
                .read(cx)
                .gate
                .lock()
                .is_ok_and(|gate| gate.available())
        {
            return false;
        }
        let store = self.store.clone();
        let gate = self.app_windows.read(cx).gate.clone();
        // A read occupies the same slot as writes: it may not race a committed
        // revision notification and accidentally clear an uncertainty marker.
        let owner = self.window_id;
        let reload = match ConfigurationGate::begin_reload_lease(&gate, owner) {
            Ok(reload) => reload,
            Err(_) => return false,
        };
        self.state_reloading = true;
        self.set_tunnel_rule_configuration_busy(true, cx);
        self.status = Message::new("正在重新加载本机配置…", "Reloading local configuration…");
        let task = cx.background_executor().spawn(async move { store.load() });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.state_reloading = false;
                view.set_tunnel_rule_configuration_busy(false, cx);
                if view.closing.is_some()
                    || origin.as_ref().is_some_and(|origin| !view.tunnel_reload_current(origin, cx))
                {
                    // Dropping the unfinished lease keeps the shared store
                    // uncertain. A retired panel must not authorize adoption
                    // or clear the replacement owner's write barrier.
                    drop(reload);
                    view.configuration_invalidated = true;
                    view.mark_tunnel_rule_store_uncertain(cx);
                    view.app_windows.update(cx, |shared, cx| shared.changed(cx));
                    view.status = Message::new(
                        "配置读取已结束，但原面板已失效；请从当前面板重新加载。",
                        "Configuration reading ended, but the original panel expired. Reload from the current panel.",
                    );
                    cx.notify();
                    return;
                }
                // Retain the lease through foreground adoption. Releasing it
                // in the worker would let a sibling write between this read
                // and adoption of its authority token.
                reload.finish(result.as_ref().ok());
                match result {
                    Ok(state) => {
                        if let Some(origin) = &origin {
                            view.finish_tunnel_reload(origin, Message::new(
                                "配置已重新加载；草稿保留，请重新审核。",
                                "Configuration reloaded. Draft retained; review again.",
                            ), cx);
                        }
                        view.accept_reloaded_knowledge_state(state, window, cx);
                        view.status = Message::new(
                            "配置已重新加载；草稿与现有 SSH 保留，请重新审核。",
                            "Configuration reloaded. Drafts and live SSH are retained; review again.",
                        );
                    }
                    Err(error) => {
                        view.configuration_invalidated = true;
                        if let Some(origin) = &origin {
                            view.finish_tunnel_reload(origin, Message::new(
                                "配置读取失败；草稿保留，请修复配置后重试。",
                                "Configuration reload failed. Draft retained; repair the configuration and retry.",
                            ), cx);
                        }
                        view.mark_tunnel_rule_store_uncertain(cx);
                        view.app_windows.update(cx, |shared, cx| shared.changed(cx));
                        view.status = Message::detail("配置读取失败，草稿保留", "Configuration reload failed; drafts retained", error);
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
        true
    }
}
