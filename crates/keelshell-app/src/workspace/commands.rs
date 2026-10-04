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

impl Workspace {
    pub(super) fn snippet_modal_open(&self) -> bool {
        self.snippet_editor.is_some()
            || self.snippet_delete.is_some()
            || self.snippet_parameters.is_some()
    }

    pub(super) fn command_surface_blocked(&self) -> bool {
        self.snippet_modal_open()
            || self.mcp.show
            || self.show_batch
            || self.show_workflow
            || self.discard_archive.is_some()
            || self.vault_settings.is_some()
            || self.ai_settings.is_some()
            || self.show_connections
            || self.openssh_review.is_some()
            || self.form.is_some()
            || self.folder_form.is_some()
            || self.destination_prompt.is_some()
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
        let panel = cx.new(|cx| SnippetEditor::new(snippet, window, cx));
        self.snippet_subscription = Some(cx.subscribe_in(
            &panel,
            window,
            |view, panel, event, window, cx| match event {
                SnippetEditorEvent::Cancel => view.close_snippet_modal(window, cx),
                SnippetEditorEvent::Save(snippet) => {
                    if view.saving {
                        panel.update(cx, |panel, cx| {
                            panel.set_error(
                                Message::new("请等待当前保存完成", "Wait for the current save"),
                                cx,
                            )
                        });
                        return;
                    }
                    let mut candidate = view.state.clone();
                    let result = if view.snippet_editing {
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
                    view.persist(
                        candidate,
                        AfterSave::SnippetSaved {
                            panel: panel.entity_id(),
                        },
                        window,
                        cx,
                    );
                }
            },
        ));
        self.snippet_editor = Some(panel);
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
        self.snippet_delete = None;
        self.snippet_parameters = None;
        self.parameter_subscription = None;
        self.parameter_ticket = None;
        self.focus_current_surface(window, cx);
        self.flush_recent_connections(window, cx);
        self.flush_batch_audits(window, cx);
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
