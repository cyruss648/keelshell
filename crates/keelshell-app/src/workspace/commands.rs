//! Reviewed command insertion and explicit, revision-safe snippet management.
use super::*;
use crate::command_suggestions::{self, SuggestionSource};
use keelshell_core::Snippet;
mod view;

#[derive(Clone)]
pub(super) struct CommandCandidate {
    target: EntityId,
    revision: u64,
    input: String,
    command: String,
    title: String,
    source: SuggestionSource,
    snippet_snapshot: Option<Snippet>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SuggestionRequest {
    target: EntityId,
    revision: u64,
    input: String,
    sources_revision: u64,
}

pub(super) struct AiSnippetSaveRequest {
    pub(super) command: String,
    pub(super) target: String,
    pub(super) session_id: String,
    pub(super) review_id: uuid::Uuid,
}

impl Workspace {
    pub(super) fn snippet_modal_open(&self) -> bool {
        self.snippet_editor.is_some()
            || self.snippet_delete.is_some()
            || self.snippet_parameters.is_some()
    }

    pub(super) fn command_surface_blocked(&self) -> bool {
        !self.configuration_token_current()
            || self.close_review.is_some()
            || self.snippet_modal_open()
            || self.mcp.show
            || self.show_batch
            || self.show_workflow
            || self.discard_archive.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.ai_settings.is_some()
            || self.show_connections
            || self.openssh_review.is_some()
            || self.form.is_some()
            || self.folder_form.is_some()
            || self.destination_prompt.is_some()
            || self.library_batch_prompt.is_some()
            || self.login.is_some()
            || self.host_approval.is_some()
    }

    pub(super) fn candidate(
        &self,
        command: String,
        title: String,
        source: SuggestionSource,
        cx: &App,
    ) -> Option<CommandCandidate> {
        let terminal = self.tabs.get(self.active)?;
        let target = terminal.entity_id();
        if !terminal.read(cx).is_open() || self.command_target.is_some_and(|id| id != target) {
            return None;
        }
        Some(CommandCandidate {
            target,
            revision: self.command_revision,
            input: self.command.read(cx).value().to_string(),
            command,
            title,
            source,
            snippet_snapshot: match source {
                SuggestionSource::Snippet(id) => {
                    self.state.snippets.iter().find(|s| s.id == id).cloned()
                }
                SuggestionSource::History => None,
            },
        })
    }

    fn suggestion_request_current(&self, request: &SuggestionRequest, cx: &App) -> bool {
        self.tabs
            .get(self.active)
            .is_some_and(|tab| tab.entity_id() == request.target && tab.read(cx).is_open())
            && self
                .command_target
                .is_none_or(|target| target == request.target)
            && self.command_revision == request.revision
            && self.command_sources_revision == request.sources_revision
            && self.command.read(cx).value().as_str() == request.input
    }

    pub(super) fn refresh_command_suggestions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.remote_completion.visible()
            || self.suggestion_job_running
            || self.command_surface_blocked()
            || !self.command.read(cx).focus_handle(cx).is_focused(window)
        {
            return;
        }
        let value = self.command.read(cx).value();
        if value.trim().is_empty()
            || self
                .suggestion_dismissed
                .as_ref()
                .is_some_and(|(revision, text)| {
                    *revision == self.command_revision && *text == value.as_str()
                })
        {
            return;
        }
        if self
            .suggestion_request
            .as_ref()
            .is_some_and(|request| self.suggestion_request_current(request, cx))
        {
            return;
        }
        let Some(tab) = self
            .tabs
            .get(self.active)
            .filter(|tab| tab.read(cx).is_open())
        else {
            return;
        };
        let target = tab.entity_id();
        if self.command_target.is_some_and(|id| id != target) {
            return;
        }
        let request = SuggestionRequest {
            target,
            revision: self.command_revision,
            input: value.to_string(),
            sources_revision: self.command_sources_revision,
        };
        self.suggestion_request = Some(request.clone());
        self.suggestion_cache.clear();
        self.suggestion_job_running = true;
        let history = self
            .command_histories
            .get(&target)
            .cloned()
            .unwrap_or_default();
        let snippets = self.snippet_sources.clone();
        let snippet_snapshots = snippets.clone();
        let query = request.input.clone();
        // One worker per workspace; edits during matching coalesce into the next
        // current request, so slow searches cannot build an unbounded work queue.
        let task = cx
            .background_executor()
            .spawn(async move { command_suggestions::suggest(&history, &snippets, &query) });
        cx.spawn_in(window, async move |this, cx| {
            let suggestions = task.await;
            let _ = this.update_in(cx, |view, _, cx| {
                view.suggestion_job_running = false;
                if view.suggestion_request_current(&request, cx) {
                    view.suggestion_selected = 0;
                    view.suggestion_scroll.scroll_to_item(0);
                    view.suggestion_cache = suggestions
                        .into_iter()
                        .map(|suggestion| CommandCandidate {
                            target: request.target,
                            revision: request.revision,
                            input: request.input.clone(),
                            command: suggestion.command,
                            title: suggestion.title,
                            source: suggestion.source,
                            snippet_snapshot: match suggestion.source {
                                SuggestionSource::Snippet(id) => {
                                    snippet_snapshots.iter().find(|s| s.id == id).cloned()
                                }
                                SuggestionSource::History => None,
                            },
                        })
                        .collect();
                } else {
                    view.suggestion_request = None;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn command_candidates(&self, cx: &App) -> Vec<CommandCandidate> {
        if self.remote_completion.visible()
            || self.command_surface_blocked()
            || self
                .suggestion_dismissed
                .as_ref()
                .is_some_and(|(revision, text)| {
                    *revision == self.command_revision
                        && *text == self.command.read(cx).value().as_str()
                })
            || !self
                .suggestion_request
                .as_ref()
                .is_some_and(|request| self.suggestion_request_current(request, cx))
        {
            return Vec::new();
        }
        self.suggestion_cache.clone()
    }

    pub(super) fn insert_candidate(
        &mut self,
        ticket: CommandCandidate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Verify both the rendered input and event revision: input Change can still
        // be queued when a pointer callback reaches this workspace.
        let source_exists = match ticket.source {
            SuggestionSource::History => {
                self.command_histories
                    .get(&ticket.target)
                    .is_some_and(|history| {
                        history
                            .newest_first()
                            .any(|command| command == ticket.command)
                    })
            }
            SuggestionSource::Snippet(id) => self.state.snippets.iter().any(|snippet| {
                snippet.id == id
                    && snippet.command == ticket.command
                    && ticket.snippet_snapshot.as_ref() == Some(snippet)
            }),
        };
        let valid = !self.command_surface_blocked()
            && source_exists
            && self.tabs.get(self.active).is_some_and(|terminal| {
                terminal.entity_id() == ticket.target && terminal.read(cx).is_open()
            })
            && self.command_revision == ticket.revision
            && self.command.read(cx).value().as_str() == ticket.input
            && self
                .command_target
                .is_none_or(|target| target == ticket.target);
        if !valid {
            self.status = Message::new(
                "命令、来源或目标已变化，请重新选择。",
                "Command, source or target changed. Select it again.",
            );
            cx.notify();
            return;
        }
        if let Some(snippet) = ticket.snippet_snapshot.filter(|s| s.parameterized) {
            self.open_snippet_parameters(
                snippet,
                ticket.target,
                ticket.revision,
                ticket.input,
                window,
                cx,
            );
            return;
        }
        self.set_reviewed_command(ticket.command, Some(ticket.target), window, cx);
        self.command_record_history = true;
        self.command.read(cx).focus_handle(cx).focus(window, cx);
        self.status = Message::new(
            "已填入，请核对完整命令与目标后点击执行。",
            "Inserted. Review the complete command and target, then click Run.",
        );
        cx.notify();
    }

    pub(super) fn set_reviewed_command(
        &mut self,
        text: String,
        target: Option<EntityId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_remote_completion(cx);
        if text.is_empty() {
            self.command_record_history = true;
        }
        // Programmatic set_value deliberately emits no InputEvent::Change.
        self.command_revision = self.command_revision.wrapping_add(1);
        self.command_target = target;
        self.suggestion_selected = 0;
        self.suggestion_dismissed = Some((self.command_revision, text.clone()));
        self.command
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }

    pub(super) fn suggestion_action(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.command.read(cx).focus_handle(cx).is_focused(window)
            || self.command_surface_blocked()
        {
            return false;
        }
        let composing = self.command.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        });
        if composing {
            return false;
        }
        if self.remote_completion_action(key, window, cx) {
            return true;
        }
        if key == "escape" && !self.command.read(cx).value().trim().is_empty() {
            self.suggestion_dismissed = Some((
                self.command_revision,
                self.command.read(cx).value().to_string(),
            ));
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        let candidates = self.command_candidates(cx);
        if candidates.is_empty() {
            return false;
        }
        match key {
            "down" => self.suggestion_selected = (self.suggestion_selected + 1) % candidates.len(),
            "up" => {
                self.suggestion_selected =
                    (self.suggestion_selected + candidates.len() - 1) % candidates.len()
            }
            "enter" => {
                if let Some(ticket) = candidates.get(self.suggestion_selected).cloned() {
                    self.insert_candidate(ticket, window, cx);
                }
            }
            "escape" => {
                self.suggestion_dismissed = Some((
                    self.command_revision,
                    self.command.read(cx).value().to_string(),
                ))
            }
            _ => return false,
        }
        self.suggestion_scroll
            .scroll_to_item(self.suggestion_selected);
        cx.stop_propagation();
        cx.notify();
        true
    }

    pub(super) fn open_snippet_editor(
        &mut self,
        snippet: Option<Snippet>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_manage_snippets() {
            return;
        }
        self.snippet_editing = snippet.is_some();
        self.ai_snippet_review = None;
        let panel = cx.new(|cx| SnippetEditor::new(snippet, window, cx));
        self.snippet_subscription = Some(Self::subscribe_snippet_editor(&panel, window, cx));
        self.snippet_editor = Some(panel);
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    fn subscribe_snippet_editor(
        panel: &Entity<SnippetEditor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            panel,
            window,
            |view, panel, event, window, cx| match event {
                SnippetEditorEvent::Cancel => view.close_snippet_modal(window, cx),
                SnippetEditorEvent::ReReview { command } => {
                    view.re_review_snippet(command, window, cx)
                }
                SnippetEditorEvent::Save(snippet) => {
                    view.save_snippet_from_editor(panel, snippet, window, cx)
                }
            },
        )
    }

    fn re_review_snippet(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(binding) = self.ai_snippet_review.clone() else {
            return;
        };
        if binding.command != command {
            self.close_snippet_modal(window, cx);
            self.status = Message::new(
                "重新审核候选已变化，片段保存流程已取消。",
                "The re-review candidate changed; the snippet save flow was canceled.",
            );
            cx.notify();
            return;
        }
        if let Err(message) = self.admit_ai_snippet_binding(&binding, cx) {
            self.close_snippet_modal(window, cx);
            self.status = message;
            cx.notify();
            return;
        }
        let target = binding.terminal;
        self.close_snippet_modal(window, cx);
        self.set_reviewed_command(command.to_owned(), Some(target), window, cx);
        self.status = Message::new(
            "已取消片段保存，请在命令栏中重新审核命令。",
            "Snippet saving was canceled. Review the command again in the command bar.",
        );
        cx.notify();
    }

    fn save_snippet_from_editor(
        &mut self,
        panel: &Entity<SnippetEditor>,
        snippet: &Snippet,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            panel.update(cx, |panel, cx| {
                panel.set_error(
                    Message::new("请等待当前保存完成", "Wait for the current save"),
                    cx,
                )
            });
            return;
        }
        if panel.read(cx).is_ai_reviewed() {
            let Some(binding) = self.ai_snippet_review.clone() else {
                panel.update(cx, |panel, cx| {
                    panel.set_error(
                        Message::new(
                            "AI 片段审核已失效，草稿未保存。",
                            "The AI snippet review is stale; the draft was not saved.",
                        ),
                        cx,
                    )
                });
                return;
            };
            if binding.command != snippet.command
                || self.admit_ai_snippet_binding(&binding, cx).is_err()
            {
                let message = Message::new(
                    "AI 片段审核已失效，当前目标或配置已变化；未保存片段。",
                    "The AI snippet review is stale because the target or configuration changed; no snippet was saved.",
                );
                panel.update(cx, |panel, cx| panel.set_error(message.clone(), cx));
                self.ai_snippet_review = None;
                self.close_snippet_modal(window, cx);
                self.status = message;
                cx.notify();
                return;
            }
        }
        let mut candidate = self.state.clone();
        let result = if self.snippet_editing {
            candidate.update_snippet(snippet.clone())
        } else {
            candidate.insert_snippet(snippet.clone())
        };
        if let Err(error) = result {
            panel.update(cx, |panel, cx| {
                panel.set_error(
                    Message::detail("片段未保存", "Snippet not saved", error),
                    cx,
                )
            });
            return;
        }
        self.persist(
            candidate,
            AfterSave::SnippetSaved {
                panel: panel.entity_id(),
            },
            window,
            cx,
        );
    }

    /// Admit a reviewed AI event against the live SSH tab and configuration.
    ///
    /// The event carries only process-local identity. This second check is the
    /// authority boundary that prevents a delayed review from opening a draft
    /// for a replaced session, target, configuration or modal scope.
    pub(super) fn admit_ai_snippet_target(
        &self,
        target: &str,
        session_id: &str,
        cx: &App,
    ) -> Result<EntityId, Message> {
        if self.saving
            || self.state_reloading
            || self.configuration_recovery.is_some()
            || self.configuration_invalidated
            || !self.configuration_current(cx)
        {
            return Err(Message::new(
                "配置正在切换、重新加载或关闭，AI 片段审核已丢弃。",
                "The configuration is switching, reloading or closing; the AI snippet review was discarded.",
            ));
        }
        if self.active_modal().is_some()
            || self.modal_scope.current.is_some()
            || self.snippet_modal_open()
        {
            return Err(Message::new(
                "当前操作范围已被其他面板替换，AI 片段审核已丢弃。",
                "The current operation scope was replaced by another panel; the AI snippet review was discarded.",
            ));
        }
        let Some(terminal) = self.tabs.get(self.active) else {
            return Err(Message::new(
                "当前没有活动 SSH 标签，AI 片段审核已丢弃。",
                "There is no active SSH tab; the AI snippet review was discarded.",
            ));
        };
        let terminal_id = terminal.entity_id();
        let active_session_id = format!("{terminal_id:?}");
        if active_session_id != session_id
            || !terminal.read(cx).is_open()
            || self.remote_hosts.get(&terminal_id).map(String::as_str) != Some(target)
        {
            return Err(Message::new(
                "当前活动 SSH 标签或目标已变化，AI 片段审核已丢弃。",
                "The active SSH tab or target changed; the AI snippet review was discarded.",
            ));
        }
        Ok(terminal_id)
    }

    fn admit_ai_snippet_binding(
        &self,
        binding: &AiSnippetReviewBinding,
        cx: &App,
    ) -> Result<(), Message> {
        if binding.review_id.is_nil() {
            return Err(Message::new(
                "AI 命令审核凭据无效，片段未保存。",
                "The AI command review receipt is invalid; the snippet was not saved.",
            ));
        }
        if self.saving
            || self.state_reloading
            || self.configuration_recovery.is_some()
            || self.configuration_invalidated
            || !self.configuration_current(cx)
        {
            return Err(Message::new(
                "配置正在切换、重新加载或关闭，AI 片段审核已失效。",
                "The configuration is switching, reloading or closing; the AI snippet review is stale.",
            ));
        }
        if self.active_modal() != Some(modal_scope::ModalKind::SnippetEditor)
            || self.modal_scope.current != Some(modal_scope::ModalKind::SnippetEditor)
            || self.snippet_editor.is_none()
        {
            return Err(Message::new(
                "片段编辑器操作范围已被替换，AI 片段审核已失效。",
                "The snippet editor scope was replaced; the AI snippet review is stale.",
            ));
        }
        let Some(terminal) = self.tabs.get(self.active) else {
            return Err(Message::new(
                "当前没有活动 SSH 标签，AI 片段审核已失效。",
                "There is no active SSH tab; the AI snippet review is stale.",
            ));
        };
        let terminal_id = terminal.entity_id();
        if terminal_id != binding.terminal
            || format!("{terminal_id:?}") != binding.session_id
            || !terminal.read(cx).is_open()
            || self.remote_hosts.get(&terminal_id).map(String::as_str)
                != Some(binding.target.as_str())
        {
            return Err(Message::new(
                "当前活动 SSH 标签或目标已变化，AI 片段审核已失效。",
                "The active SSH tab or target changed; the AI snippet review is stale.",
            ));
        }
        Ok(())
    }

    /// Handle the AI event without touching StateStore.
    pub(super) fn handle_ai_snippet_save(
        &mut self,
        panel: &Entity<AssistantPanel>,
        request: AiSnippetSaveRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let AiSnippetSaveRequest {
            command,
            target,
            session_id,
            review_id,
        } = request;
        let terminal = match self.admit_ai_snippet_target(&target, &session_id, cx) {
            Ok(terminal) => terminal,
            Err(message) => {
                panel.update(cx, |panel, cx| {
                    panel.reject_snippet_save_review(review_id, message.clone(), cx)
                });
                self.status = message;
                cx.notify();
                return;
            }
        };
        let accepted = panel.update(cx, |panel, cx| {
            panel.consume_snippet_save_review(&command, &target, &session_id, review_id, cx)
        });
        if !accepted {
            let message = Message::new(
                "AI 命令审核已过期或已使用，未打开片段草稿。",
                "The AI command review is stale or already used; no snippet draft was opened.",
            );
            self.status = message.clone();
            cx.notify();
            return;
        }
        let snippet = Snippet::new(t(cx, "AI 命令片段", "AI command snippet"), command.clone());
        self.snippet_editing = false;
        self.ai_snippet_review = Some(AiSnippetReviewBinding {
            command: command.clone(),
            target,
            session_id,
            terminal,
            review_id,
        });
        let editor = cx.new(|cx| SnippetEditor::new_reviewed_draft(snippet, window, cx));
        self.snippet_subscription = Some(Self::subscribe_snippet_editor(&editor, window, cx));
        self.snippet_editor = Some(editor);
        self.status = Message::new(
            "已打开锁定的片段草稿；填写信息后需再次确认保存。",
            "A locked snippet draft is open; complete its details and confirm saving.",
        );
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    fn can_manage_snippets(&self) -> bool {
        !self.saving && !self.connecting && !self.command_surface_blocked()
    }

    pub(super) fn close_snippet_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving
            || self
                .snippet_parameters
                .as_ref()
                .is_some_and(|panel| panel.read(cx).is_saving())
            || self
                .snippet_editor
                .as_ref()
                .is_some_and(|panel| panel.read(cx).is_saving())
        {
            return;
        }
        self.snippet_editor = None;
        self.snippet_subscription = None;
        self.ai_snippet_review = None;
        self.snippet_delete = None;
        self.snippet_parameters = None;
        self.parameter_subscription = None;
        self.parameter_ticket = None;
        self.focus_current_surface(window, cx);
        self.flush_recent_connections(window, cx);
        self.flush_batch_audits(window, cx);
        self.flush_workflow_audits(window, cx);
        cx.notify();
    }

    fn delete_snippet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(snippet) = self.snippet_delete.as_ref() else {
            return;
        };
        let id = snippet.id;
        let mut candidate = self.state.clone();
        if candidate.snippets.iter().find(|item| item.id == id) != Some(snippet) {
            self.status = Message::new(
                "片段已变化，请取消后重新选择",
                "Snippet changed. Cancel and select it again",
            );
            cx.notify();
            return;
        }
        if let Err(error) = candidate.remove_snippet(id) {
            self.status = Message::detail("删除失败", "Delete failed", error);
            cx.notify();
            return;
        }
        self.persist(candidate, AfterSave::SnippetDeleted { id }, window, cx);
    }
}
