//! Connection organization commands; every mutation uses the existing revision-safe save lane.
use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum LibraryFilter {
    #[default]
    All,
    Favorites,
    Recent,
    Trash,
    Folder(Option<uuid::Uuid>),
}

pub(super) struct FolderForm {
    pub token: uuid::Uuid,
    pub id: Option<uuid::Uuid>,
    pub name: Entity<InputState>,
    pub parent_id: Option<uuid::Uuid>,
    pub message: Option<Message>,
}

#[derive(Clone, Copy)]
pub(super) enum DestinationTarget {
    Draft,
    Connection(uuid::Uuid),
}

pub(super) struct DestinationPrompt {
    pub token: uuid::Uuid,
    pub target: DestinationTarget,
    pub focus: FocusHandle,
    pub folder_id: Option<uuid::Uuid>,
    pub message: Option<Message>,
}

pub(super) fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Workspace {
    pub(super) fn focus_current_surface(&self, window: &mut Window, cx: &mut App) {
        if self.mcp.show {
            self.overlay_focus.focus(window, cx);
            return;
        }
        if self.show_workflow
            && let Some(panel) = &self.workflow_panel
        {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
            return;
        }
        if self.show_batch
            && let Some(panel) = &self.batch_panel
        {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
            return;
        }
        if let Some(panel) = &self.snippet_parameters {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
            return;
        }
        if let Some(panel) = &self.vault_settings {
            panel.update(cx, |panel, cx| {
                panel.update_references(credentials::credential_references(&self.state), cx);
                panel.focus(window, cx);
            });
            return;
        }
        if let Some(panel) = &self.snippet_editor {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
            return;
        }
        if self.snippet_delete.is_some() {
            self.overlay_focus.focus(window, cx);
            return;
        }
        // AI settings already owns a panel/input focus; do not interrupt its draft.
        if self.ai_settings.is_some() {
            return;
        }
        if let Some(login) = &self.login {
            login.focus(window, cx);
        } else if self.host_approval.is_some() || self.library_batch_prompt.is_some() {
            self.overlay_focus.focus(window, cx);
        } else if let Some(prompt) = &self.destination_prompt {
            prompt.focus.focus(window, cx);
        } else if let Some(form) = &self.folder_form {
            form.name.read(cx).focus_handle(cx).focus(window, cx);
        } else if let Some(form) = &self.form {
            form.name.read(cx).focus_handle(cx).focus(window, cx);
        } else if self.visible_panel == Some(ToolPanel::Commands) && !self.show_connections {
            self.snippet_search
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        } else if self.show_connections || self.tabs.is_empty() {
            self.search.read(cx).focus_handle(cx).focus(window, cx);
        } else if let Some(terminal) = self.tabs.get(self.active) {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    pub(super) fn folder_label(&self, id: Option<uuid::Uuid>, cx: &App) -> String {
        id.and_then(|id| self.state.folder_path(id))
            .unwrap_or_else(|| t(cx, "未归档", "Unfiled").to_owned())
    }

    pub(super) fn open_folder_form(
        &mut self,
        id: Option<uuid::Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        let (name, parent_id) = if let Some(id) = id {
            let Some(folder) = self.state.folders.iter().find(|folder| folder.id == id) else {
                return;
            };
            (folder.name.clone(), folder.parent_id)
        } else {
            (
                String::new(),
                match self.library_filter {
                    LibraryFilter::Folder(id) => id,
                    _ => None,
                },
            )
        };
        let name = input(t(cx, "文件夹名称", "Folder name"), &name, window, cx);
        name.read(cx).focus_handle(cx).focus(window, cx);
        self.folder_form = Some(FolderForm {
            token: uuid::Uuid::new_v4(),
            id,
            name,
            parent_id,
            message: None,
        });
        cx.notify();
    }

    pub(super) fn close_folder_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_form = None;
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn save_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(form) = self.folder_form.as_ref() else {
            return;
        };
        let token = form.token;
        let mut candidate = self.state.clone();
        let name = form.name.read(cx).value().trim().to_owned();
        let result = if let Some(id) = form.id {
            candidate
                .update_folder(id, name, form.parent_id)
                .map(|()| id)
        } else {
            candidate.create_folder(name, form.parent_id)
        };
        match result {
            Ok(id) => self.persist(candidate, AfterSave::FolderSaved { id, token }, window, cx),
            Err(error) => {
                if let Some(form) = self.folder_form.as_mut() {
                    form.message = Some(Message::detail("文件夹未保存", "Folder not saved", error));
                }
                cx.notify();
            }
        }
    }

    pub(super) fn remove_empty_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(form) = self.folder_form.as_ref() else {
            return;
        };
        let Some(id) = form.id else {
            return;
        };
        let token = form.token;
        let mut candidate = self.state.clone();
        match candidate.remove_folder(id) {
            Ok(_) => self.persist(
                candidate,
                AfterSave::FolderRemoved { id, token },
                window,
                cx,
            ),
            Err(error) => {
                if let Some(form) = self.folder_form.as_mut() {
                    form.message = Some(Message::detail(
                        "无法删除：请先移走子文件夹、连接及回收站中的关联连接",
                        "Move child folders and active or trashed connections out before deleting",
                        error,
                    ));
                }
                cx.notify();
            }
        }
    }

    pub(super) fn open_destination(
        &mut self,
        target: DestinationTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        let folder_id = match target {
            DestinationTarget::Draft => self.form.as_ref().and_then(|form| form.folder_id),
            DestinationTarget::Connection(id) => self.state.folder_id_of(id),
        };
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.destination_prompt = Some(DestinationPrompt {
            token: uuid::Uuid::new_v4(),
            target,
            folder_id,
            focus,
            message: None,
        });
        cx.notify();
    }

    pub(super) fn close_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.destination_prompt = None;
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn save_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(prompt) = &self.destination_prompt else {
            return;
        };
        let token = prompt.token;
        let folder_id = prompt.folder_id;
        if let DestinationTarget::Connection(id) = prompt.target {
            let mut candidate = self.state.clone();
            match candidate.move_connection(id, folder_id) {
                Ok(()) => self.persist(candidate, AfterSave::ConnectionMoved { token }, window, cx),
                Err(error) => {
                    if let Some(prompt) = &mut self.destination_prompt {
                        prompt.message = Some(Message::detail("移动失败", "Move failed", error));
                    }
                    cx.notify();
                }
            }
        } else {
            if let Some(form) = &mut self.form {
                form.folder_id = folder_id;
            }
            self.close_destination(window, cx);
        }
    }

    pub(super) fn restore_connection(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        let mut candidate = self.state.clone();
        match candidate.restore_connection(id) {
            Ok(id) => {
                let name = candidate
                    .connections
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.name.clone())
                    .unwrap_or_default();
                self.persist(
                    candidate,
                    AfterSave::ConnectionRestored { name },
                    window,
                    cx,
                );
            }
            Err(error) => {
                self.status = Message::detail(
                    "恢复失败，回收站内容未改变",
                    "Restore failed; trash is unchanged",
                    error,
                );
                cx.notify();
            }
        }
    }

    pub(super) fn remember_successful_connection(
        &mut self,
        connection: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .state
            .connections
            .iter()
            .any(|current| vault::same_destination(current, &connection))
        {
            return;
        }
        // A connect can finish while another settings/profile save is in flight.
        // Merge only into the state returned by that save, never its stale snapshot.
        let Ok(route) = self.state.connection_route(connection.id) else {
            return;
        };
        self.pending_recents
            .retain(|(previous, _, _)| previous.id != connection.id);
        self.pending_recents
            .push((connection, route, now_seconds()));
        if self.pending_recents.len() > 50 {
            self.pending_recents.remove(0);
        }
        self.flush_recent_connections(window, cx);
    }

    pub(super) fn flush_recent_connections(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving
            || self.vault_settings.is_some()
            || self.snippet_modal_open()
            || self.pending_recents.is_empty()
        {
            return;
        }
        let pending = std::mem::take(&mut self.pending_recents);
        let mut candidate = self.state.clone();
        let mut changed = false;
        for (connection, route, connected_at) in pending {
            // Deleting a profile during connection does not resurrect metadata.
            if !candidate
                .connections
                .iter()
                .any(|item| vault::same_destination(item, &connection))
                || !candidate
                    .connection_route(connection.id)
                    .is_ok_and(|current| routing::same_route(&route, &current))
            {
                continue;
            }
            match candidate.record_successful_connection(connection.id, connected_at) {
                Ok(()) => changed = true,
                Err(error) => {
                    self.status = Message::detail(
                        "连接已建立，但最近记录未保存",
                        "Connected, but recent usage was not saved",
                        error,
                    );
                    cx.notify();
                    return;
                }
            }
        }
        if changed {
            self.persist(candidate, AfterSave::RecentSaved, window, cx);
        }
    }
}
