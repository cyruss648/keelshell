//! Sync profile changes affect saved metadata, never active remote session instances.
use super::*;
use crate::profile_sync::{ProfileSyncEvent, ProfileSyncPanel};
impl Workspace {
    pub(super) fn open_profile_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_open_vault() {
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
                cx.subscribe_in(&panel, window, |view, _, event, window, cx| match event {
                    ProfileSyncEvent::Changed { state, message } => {
                        // Tabs retain their captured authenticated transports. A renamed,
                        // changed or deleted saved endpoint cannot replace a live session.
                        view.state = (**state).clone();
                        view.status = message.clone();
                        view.refresh_workflow_audit_history(cx);
                        cx.notify();
                    }
                    ProfileSyncEvent::Close => {
                        view.profile_sync = None;
                        view.profile_sync_subscription = None;
                        view.flush_recent_connections(window, cx);
                        view.flush_batch_audits(window, cx);
                        view.flush_workflow_audits(window, cx);
                        view.focus_current_surface(window, cx);
                        cx.notify();
                    }
                }),
            );
        self.profile_sync = Some(panel);
        cx.notify();
    }
}
