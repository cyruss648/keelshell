//! Recovery replaces metadata only after the user has retired all live authority.
use super::*;
use crate::configuration_recovery::{ConfigRecoveryEvent, ConfigRecoveryPanel};

impl Workspace {
    fn config_recovery_idle(&self, cx: &App) -> bool {
        !self.saving
            && self.tabs.is_empty()
            && self.remote_sessions.is_empty()
            && !self.connecting
            && self.connect_route.is_none()
            && self.agent.is_none()
            && self.assistant.read(cx).config_recovery_idle()
            && self.mcp.config_recovery_idle()
            && self.pending_recents.is_empty()
            && self.pending_batch_audits.is_empty()
            && self.pending_workflow_audits.is_empty()
            && self
                .batch_panel
                .as_ref()
                .is_none_or(|panel| !panel.read(cx).is_running())
            && self
                .workflow_panel
                .as_ref()
                .is_none_or(|panel| !panel.read(cx).is_running())
    }

    pub(super) fn open_configuration_recovery(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.configuration_recovery.is_some()
            || self
                .active_modal()
                .is_some_and(|kind| kind != modal_scope::ModalKind::Manager)
        {
            self.focus_current_surface(window, cx);
            return;
        }
        if !self.config_recovery_idle(cx) {
            self.status = Message::new(
                "恢复前请关闭全部 SSH 标签，停止 AI、批量、定时和外部 MCP 操作，并等待配置保存完成。",
                "Before recovery, close all SSH tabs, stop AI, batch, scheduled and external MCP work, and wait for configuration saves.",
            );
            cx.notify();
            return;
        }
        let panel = cx.new(|cx| ConfigRecoveryPanel::new(self.store.clone(), cx));
        self.configuration_recovery_subscription = Some(cx.subscribe_in(
            &panel, window, |view, owner, event, window, cx| {
                if view.configuration_recovery.as_ref()
                    .is_none_or(|current| current.entity_id() != owner.entity_id())
                {
                    return;
                }
                match event {
                    ConfigRecoveryEvent::RequestRestore { generation } => {
                        if view.config_recovery_idle(cx) {
                            owner.update(cx, |panel, cx| panel.restore_approved(*generation, cx));
                        } else {
                            owner.update(cx, |panel, cx| panel.refuse_restore(cx));
                        }
                    }
                    ConfigRecoveryEvent::Restored(state) => {
                        view.state = (**state).clone();
                        // Recovery never transports a previous runtime capability.
                        // The modal precondition already required no active work.
                        view.agent = None;
                        view.mcp = mcp::McpState::new(view.mcp.root.clone());
                        view.ai_credentials = EphemeralCredentials::new();
                        view.assistant.update(cx, |assistant, cx| {
                            assistant.set_context(String::new(), String::new(), String::new(), cx);
                            assistant.set_profiles(&view.state.settings.ai_profiles, &view.ai_credentials, cx);
                        });
                        view.update_service.update(cx, |panel, cx| panel.set_preferences(view.state.settings.updates, cx));
                        view.pending_update_check = None;
                        view.update_history_retry_after = None;
                        view.library_filter = LibraryFilter::All;
                        view.library_selection.clear();
                        view.library_trash_undo.clear();
                        view.snippet_sources = Arc::new(view.state.snippets.clone());
                        view.command_sources_revision = view.command_sources_revision.wrapping_add(1);
                        view.command_target = None;
                        view.cancel_remote_completion(cx);
                        crate::design::apply(view.state.settings.theme, Some(window), cx);
                        view.apply_language(view.state.settings.language, window, cx);
                        view.refresh_workflow_audit_history(cx);
                        view.status = Message::new(
                            "配置已恢复；原配置文件存在时已保留完整副本，会话、命令、传输和授权均未重放。",
                            "Configuration restored; the exact original was preserved if a configuration file existed. No sessions, commands, transfers or grants were replayed.",
                        );
                    }
                    ConfigRecoveryEvent::Close => {
                        view.configuration_recovery = None;
                        view.configuration_recovery_subscription = None;
                        view.focus_current_surface(window, cx);
                    }
                }
                cx.notify();
            },
        ));
        self.show_connections = false;
        self.configuration_recovery = Some(panel);
        cx.notify();
    }
}
