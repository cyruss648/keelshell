//! Sync profile changes affect saved metadata, never active remote session instances.
use super::*;
use crate::profile_sync::{ProfileSyncEvent, ProfileSyncPanel};
impl Workspace {
    pub(super) fn open_profile_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_open_vault()
            || !self.profile_sync_maintenance_idle(cx)
            || !self.begin_window_maintenance(cx)
        {
            return;
        }
        let panel = cx.new(|cx| {
            ProfileSyncPanel::new(
                self.store.clone(),
                &self.state,
                self.runtime.clone(),
                window,
                cx,
            )
        });
        self.profile_sync_subscription =
            Some(
                cx.subscribe_in(&panel, window, |view, owner, event, window, cx| {
                    if view.profile_sync.as_ref().is_none_or(|panel| panel.entity_id() != owner.entity_id())
                        || !view.window_maintenance_current(cx) { return; }
                    match event {
                    ProfileSyncEvent::Changed { state, message } => {
                        if view.state_reloading || !view.accept_maintenance_commit(state, cx) {
                            view.status = Message::new(
                                "配置正在重新加载，已忽略过期的同步状态；请重新打开同步面板。",
                                "Configuration is reloading; the stale sync result was ignored. Reopen sync after reload.",
                            );
                            cx.notify();
                            return;
                        }
                        // Tabs retain their captured authenticated transports. A renamed,
                        // changed or deleted saved endpoint cannot replace a live session.
                        view.state = (**state).clone();
                        view.shared_commit_completed(cx);
                        view.refresh_knowledge_library(cx);
                        view.update_service.update(cx, |panel, cx| {
                            panel.set_preferences(view.state.settings.updates, cx)
                        });
                        view.revoke_stale_protocol_diagnostics(cx);
                        view.status = message.clone();
                        view.refresh_workflow_audit_history(cx);
                        cx.notify();
                    }
                    ProfileSyncEvent::Close => {
                        if owner.read(cx).busy() { return; }
                        view.profile_sync = None;
                        view.profile_sync_subscription = None;
                        view.end_window_maintenance(cx);
                        view.flush_recent_connections(window, cx);
                        view.flush_batch_audits(window, cx);
                        view.flush_workflow_audits(window, cx);
                        view.focus_current_surface(window, cx);
                        cx.notify();
                    }
                }}),
            );
        self.profile_sync = Some(panel);
        cx.notify();
    }
}
