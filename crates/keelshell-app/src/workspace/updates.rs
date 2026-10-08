//! Update preferences use the same optimistic, off-thread state transaction.

use super::*;
use keelshell_core::UpdatePreferences;

impl Workspace {
    pub(super) fn save_update_preferences(
        &mut self,
        mut preferences: UpdatePreferences,
        revision: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving
            || self.configuration_recovery.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.snippet_modal_open()
            || self.update_panel.is_none()
            || preferences.validate().is_err()
        {
            self.update_service
                .update(cx, |panel, cx| panel.preferences_failed(cx));
            return;
        }
        // A UI draft chooses policy, never the successful-check clock metadata.
        preferences.last_successful_check = self.state.settings.updates.last_successful_check;
        let mut candidate = self.state.clone();
        candidate.settings.updates = preferences;
        self.persist(candidate, AfterSave::Updates { revision }, window, cx);
    }

    pub(super) fn flush_update_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(seconds) = self.pending_update_check else {
            return;
        };
        if self.saving
            || self.configuration_recovery.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.snippet_modal_open()
            || self
                .update_history_retry_after
                .is_some_and(|after| std::time::Instant::now() < after)
        {
            return;
        }
        if self
            .state
            .settings
            .updates
            .last_successful_check
            .is_some_and(|saved| saved >= seconds)
        {
            self.pending_update_check = None;
            return;
        }
        let mut candidate = self.state.clone();
        candidate.settings.updates.last_successful_check = Some(seconds);
        self.persist(candidate, AfterSave::UpdateHistory, window, cx);
    }
}
