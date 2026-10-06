//! Explicit selection, metadata review and one revision-safe persistence transaction.
use super::*;
use keelshell_core::{ConnectionLibraryAction, ConnectionLibraryBatchError};
use std::collections::BTreeSet;
mod view;

pub(super) struct LibraryBatchPrompt {
    pub(super) token: uuid::Uuid,
    ids: Vec<uuid::Uuid>,
    selection: Option<BTreeSet<uuid::Uuid>>,
    action: ConnectionLibraryAction,
    pub(super) tags: Entity<InputState>,
    review: Option<LibraryReview>,
    pub(super) message: Option<Message>,
}

struct LibraryReview {
    source: AppState,
    candidate: AppState,
    action: ConnectionLibraryAction,
    sessions: Vec<Vec<EntityId>>,
}

fn library_unchanged(left: &AppState, right: &AppState) -> bool {
    left.connections == right.connections
        && left.deleted_connections == right.deleted_connections
        && left.folders == right.folders
        && left.connection_folders == right.connection_folders
        && left.recent_connections == right.recent_connections
        && left.known_hosts == right.known_hosts
        && left.route_known_hosts == right.route_known_hosts
}

fn profile(state: &AppState, id: uuid::Uuid) -> Option<&Connection> {
    state
        .connections
        .iter()
        .chain(
            state
                .deleted_connections
                .iter()
                .map(|entry| &entry.connection),
        )
        .find(|entry| entry.id == id)
}

fn action_title(action: &ConnectionLibraryAction, cx: &App) -> &'static str {
    match action {
        ConnectionLibraryAction::Move(_) => t(cx, "移动所选连接", "Move selected profiles"),
        ConnectionLibraryAction::AddTags(_) => t(cx, "添加标签", "Add tags"),
        ConnectionLibraryAction::RemoveTags(_) => t(cx, "移除标签", "Remove tags"),
        ConnectionLibraryAction::ReplaceTags(_) => t(cx, "替换全部标签", "Replace all tags"),
        ConnectionLibraryAction::Favorite(true) => t(cx, "加入收藏", "Add to favorites"),
        ConnectionLibraryAction::Favorite(false) => t(cx, "取消收藏", "Remove from favorites"),
        ConnectionLibraryAction::Trash(_) => t(cx, "移入回收站", "Move to trash"),
        ConnectionLibraryAction::Restore => t(cx, "恢复连接", "Restore profiles"),
        ConnectionLibraryAction::Purge => t(cx, "永久删除连接配置", "Permanently delete profiles"),
    }
}

impl Workspace {
    pub(super) fn maintain_library_selection(&mut self) {
        let ids: BTreeSet<_> = if self.library_filter == LibraryFilter::Trash {
            self.state
                .deleted_connections
                .iter()
                .map(|entry| entry.connection.id)
                .collect()
        } else {
            self.state
                .connections
                .iter()
                .map(|entry| entry.id)
                .collect()
        };
        self.library_selection.retain(|id| ids.contains(id));
        self.library_trash_undo.retain(|id| {
            self.state
                .deleted_connections
                .iter()
                .any(|entry| entry.connection.id == *id)
        });
    }

    pub(super) fn set_library_filter(&mut self, filter: LibraryFilter, cx: &mut Context<Self>) {
        if (self.library_filter == LibraryFilter::Trash) != (filter == LibraryFilter::Trash) {
            self.library_selection.clear();
        }
        self.library_filter = filter;
        cx.notify();
    }

    pub(super) fn toggle_library_selection(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        if self.saving || self.library_batch_prompt.is_some() {
            return;
        }
        if !self.library_selection.remove(&id) {
            self.library_selection.insert(id);
        }
        cx.notify();
    }

    pub(super) fn open_selected_library_batch(
        &mut self,
        action: ConnectionLibraryAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids = self.library_selection.iter().copied().collect();
        self.open_library_batch(ids, action, true, window, cx);
    }

    pub(super) fn open_library_batch(
        &mut self,
        ids: Vec<uuid::Uuid>,
        action: ConnectionLibraryAction,
        selected: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving
            || ids.is_empty()
            || self.library_batch_prompt.is_some()
            || self.form.is_some()
            || self.folder_form.is_some()
            || self.destination_prompt.is_some()
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.snippet_modal_open()
            || self.show_batch
            || self.show_workflow
            || self.mcp.show
            || self.ai_settings.is_some()
        {
            return;
        }
        let tags = input(
            t(cx, "标签，用逗号分隔", "Tags, separated by commas"),
            "",
            window,
            cx,
        );
        self.overlay_focus.focus(window, cx);
        self.library_batch_prompt = Some(LibraryBatchPrompt {
            token: uuid::Uuid::new_v4(),
            ids,
            selection: selected.then(|| self.library_selection.clone()),
            action,
            tags,
            review: None,
            message: None,
        });
        cx.notify();
    }

    pub(super) fn close_library_batch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.library_batch_prompt = None;
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_library_batch_locale(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prompt) = &self.library_batch_prompt {
            prompt.tags.update(cx, |input, cx| {
                input.set_placeholder(
                    t(cx, "标签，用逗号分隔", "Tags, separated by commas"),
                    window,
                    cx,
                )
            });
        }
    }

    fn library_batch_error(&self, error: ConnectionLibraryBatchError) -> Message {
        if let ConnectionLibraryBatchError::JumpDependent {
            jump_host,
            dependent,
        } = error
        {
            let label = |id| {
                profile(&self.state, id)
                    .map(|item| {
                        format!(
                            "{} · {}",
                            item.name,
                            crate::jump_host_picker::endpoint(item)
                        )
                    })
                    .unwrap_or_else(|| id.to_string())
            };
            Message::new(
                format!(
                    "未修改任何连接：{} 仍依赖跳板 {}。请一并选择依赖连接，或先修改其路线。",
                    label(dependent),
                    label(jump_host)
                ),
                format!(
                    "No profiles changed: {} still depends on jump host {}. Select the dependent too, or change its route first.",
                    label(dependent),
                    label(jump_host)
                ),
            )
        } else {
            Message::detail("未修改任何连接", "No profiles changed", error)
        }
    }

    pub(super) fn review_library_batch(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(prompt) = &self.library_batch_prompt else {
            return;
        };
        if prompt
            .selection
            .as_ref()
            .is_some_and(|selection| *selection != self.library_selection)
        {
            self.reject_library_review(cx);
            return;
        }
        let tags: Vec<String> = prompt
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let action = match &prompt.action {
            ConnectionLibraryAction::AddTags(_) => ConnectionLibraryAction::AddTags(tags),
            ConnectionLibraryAction::RemoveTags(_) => ConnectionLibraryAction::RemoveTags(tags),
            ConnectionLibraryAction::ReplaceTags(_) => ConnectionLibraryAction::ReplaceTags(tags),
            other => other.clone(),
        };
        let mut candidate = self.state.clone();
        match candidate.apply_connection_library_batch(&prompt.ids, &action) {
            Ok(_) => {
                let sessions = prompt
                    .ids
                    .iter()
                    .map(|id| self.library_session_references(*id, cx))
                    .collect();
                if let Some(prompt) = &mut self.library_batch_prompt {
                    prompt.review = Some(LibraryReview {
                        source: self.state.clone(),
                        candidate,
                        action,
                        sessions,
                    });
                    prompt.message = None;
                }
            }
            Err(error) => {
                let message = self.library_batch_error(error);
                if let Some(prompt) = &mut self.library_batch_prompt {
                    prompt.message = Some(message);
                }
            }
        }
        cx.notify();
    }

    fn reject_library_review(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = &mut self.library_batch_prompt {
            prompt.review = None;
            prompt.message = Some(Message::new(
                "连接库、选区或活动会话已变化。草稿已保留，请重新审阅。",
                "Library, selection or active sessions changed. Draft retained; review again.",
            ));
        }
        cx.notify();
    }

    pub(super) fn confirm_library_batch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(prompt) = &self.library_batch_prompt else {
            return;
        };
        let Some(review) = &prompt.review else {
            return;
        };
        if !library_unchanged(&review.source, &self.state)
            || prompt
                .selection
                .as_ref()
                .is_some_and(|selection| *selection != self.library_selection)
            || prompt.ids.iter().enumerate().any(|(index, id)| {
                self.library_session_references(*id, cx)
                    != review.sessions.get(index).cloned().unwrap_or_default()
            })
        {
            self.reject_library_review(cx);
            return;
        }
        // Build from the current snapshot so a locale/theme save never copies an
        // obsolete revision or settings over the user's latest preferences.
        let mut candidate = self.state.clone();
        let token = prompt.token;
        let ids = prompt.ids.clone();
        let action = review.action.clone();
        match candidate.apply_connection_library_batch(&ids, &action) {
            Ok(_) => self.persist(
                candidate,
                AfterSave::LibraryBatch { token, action, ids },
                window,
                cx,
            ),
            Err(error) => {
                let message = self.library_batch_error(error);
                if let Some(prompt) = &mut self.library_batch_prompt {
                    prompt.review = None;
                    prompt.message = Some(message);
                }
                cx.notify();
            }
        }
    }

    pub(super) fn finish_library_batch(
        &mut self,
        token: uuid::Uuid,
        action: ConnectionLibraryAction,
        ids: Vec<uuid::Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(action, ConnectionLibraryAction::Trash(_)) {
            self.library_trash_undo = ids.clone();
        }
        if matches!(
            action,
            ConnectionLibraryAction::Restore | ConnectionLibraryAction::Purge
        ) {
            self.library_trash_undo.retain(|id| !ids.contains(id));
        }
        self.library_selection.retain(|id| !ids.contains(id));
        if self
            .library_batch_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.token == token)
        {
            self.library_batch_prompt = None;
        }
        self.status = Message::new(
            format!("已完成 {} 条连接的批量修改", ids.len()),
            format!("Updated {} profiles in one transaction", ids.len()),
        );
        self.focus_current_surface(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn review_binding_allows_preferences_but_rejects_trust_or_recent_changes() {
        let mut source = AppState::default();
        let item = Connection::new("Review target", "review.example.test", "operator");
        source.connections.push(item.clone());
        let mut current = source.clone();
        current.settings.language = Language::En;
        current.settings.theme = keelshell_core::Theme::Dark;
        assert!(library_unchanged(&source, &current));
        current
            .recent_connections
            .push(keelshell_core::RecentConnection {
                connection_id: item.id,
                connected_at: 10,
            });
        assert!(!library_unchanged(&source, &current));
        current.recent_connections.clear();
        current
            .known_hosts
            .insert("[review.example.test]:22".into(), "changed".into());
        assert!(!library_unchanged(&source, &current));
    }
}
