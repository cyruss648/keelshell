//! All local knowledge mutations use the ordinary serialized StateStore gate.

use super::*;

#[cfg(test)]
mod batch_tests;

impl Workspace {
    pub(super) fn refresh_knowledge_library(&self, cx: &mut Context<Self>) {
        let library = self.assistant.read(cx).knowledge_library.clone();
        library.update(cx, |panel, cx| panel.set_snapshot(&self.state, cx));
    }

    pub(super) fn apply_knowledge_change(
        &mut self,
        review_id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let library = self.assistant.read(cx).knowledge_library.clone();
        let Some(review) = library.read(cx).candidate(review_id, cx) else {
            return;
        };
        if review.snapshot != self.state.snapshot {
            library.update(cx, |panel, cx| {
                panel.set_snapshot(&self.state, cx);
                panel.reject_review(cx);
            });
            return;
        }
        let mut candidate = self.state.clone();
        let result = match (&review.expected[..], review.replacement) {
            ([expected], Some(replacement)) => {
                candidate.replace_knowledge_entry(expected, replacement)
            }
            (expected, None) => candidate.remove_knowledge_entries(expected),
            _ => return,
        };
        if let Err(error) = result {
            library.update(cx, |panel, cx| {
                panel.candidate_failed(review_id, &error, cx)
            });
            self.assistant.update(cx, |assistant, cx| {
                assistant.revoke_stale_knowledge_request(cx)
            });
            self.status = Message::detail(
                "本地知识未修改，请重新加载并重新审核",
                "Local knowledge was not changed; reload and review again",
                error,
            );
            cx.notify();
            return;
        }
        self.persist(
            candidate,
            AfterSave::KnowledgeLibrary {
                panel: library.downgrade(),
                review_id,
            },
            window,
            cx,
        );
        // begin_save runs inside persist after the queue's admission guards.
        // Revoke the owning Ask now, before queued ContextChanged or a late
        // reply can run. Its send/reply guards also read the library directly.
        self.assistant.update(cx, |assistant, cx| {
            assistant.revoke_stale_knowledge_request(cx)
        });
    }

    pub(super) fn reload_knowledge_library(
        &mut self,
        token: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving
            || self.state_reloading
            || self.ai_settings.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.update_panel.is_some()
            || self.configuration_recovery.is_some()
        {
            self.status = Message::new(
                "请先完成配置保存或关闭配置面板，再重新加载。",
                "Finish configuration saves or close configuration panels before reloading.",
            );
            cx.notify();
            return;
        }
        let library = self.assistant.read(cx).knowledge_library.clone();
        if !library.update(cx, |panel, cx| panel.begin_reload(token, cx)) {
            return;
        }
        self.state_reloading = true;
        self.set_tunnel_rule_configuration_busy(true, cx);
        let store = self.store.clone();
        let gate = self.configuration_gate.clone();
        let owner = self.window_id;
        let reload = match crate::app_windows::ConfigurationGate::begin_reload_lease(&gate, owner) {
            Ok(reload) => reload,
            Err(_) => {
                self.state_reloading = false;
                return;
            }
        };
        let task = cx.background_executor().spawn(async move {
            let result = store.load();
            reload.finish(result.as_ref().ok());
            result
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.state_reloading = false;
                view.set_tunnel_rule_configuration_busy(false, cx);
                match result {
                    Ok(state) => {
                        if library.update(cx, |panel, cx| panel.finish_reload(token, &state, cx)) {
                            view.accept_reloaded_knowledge_state(state, window, cx);
                            view.status = Message::new(
                                "配置已重新加载，知识修改需要重新审核。",
                                "Configuration reloaded. Review knowledge changes again.",
                            );
                        }
                    }
                    Err(error) => {
                        view.mark_tunnel_rule_store_uncertain(cx);
                        view.configuration_invalidated = true;
                        library.update(cx, |panel, cx| panel.reload_failed(token, &error, cx));
                        view.status = Message::detail(
                            "配置重新加载失败",
                            "Configuration reload failed",
                            error,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn accept_reloaded_knowledge_state(
        &mut self,
        state: AppState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state = state;
        self.shared_commit_completed(cx);
        // A fresh store snapshot cannot inherit transient keys from the old
        // profile catalog. Both knowledge reload entry points share this rule.
        self.ai_credentials = EphemeralCredentials::new();
        self.apply_language(self.state.settings.language, window, cx);
        crate::design::apply_settings(&self.state.settings, Some(window), cx);
        self.assistant.update(cx, |assistant, cx| {
            assistant.invalidate_knowledge_review(cx);
            assistant.replace_profiles(&self.state.settings.ai_profiles, &self.ai_credentials, cx)
        });
        self.refresh_knowledge_library(cx);
        self.update_service.update(cx, |panel, cx| {
            panel.set_preferences(self.state.settings.updates, cx)
        });
        self.maintain_library_selection();
        self.invalidate_reconnect_profiles(window, cx);
        self.revoke_stale_protocol_diagnostics(cx);
        self.maintain_mcp(cx);
        self.maintain_agent(cx);
        self.reload_tunnel_panels(cx);
        self.snippet_sources = Arc::new(self.state.snippets.clone());
        self.command_sources_revision = self.command_sources_revision.wrapping_add(1);
    }
}
