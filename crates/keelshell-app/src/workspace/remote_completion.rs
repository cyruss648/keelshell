//! Explicit remote discovery. Every response is a capability for one immutable draft.
use super::*;
use keelshell_core::{
    CompletionAnalysis, CompletionPlan, CompletionQuery, LiteralCandidate, analyze_completion,
};
use keelshell_session::{CompletionKind, CompletionResult};
use std::ops::Range;
use tokio::sync::oneshot;

mod view;

#[derive(Clone, PartialEq, Eq)]
struct CursorSnapshot {
    text: SharedString,
    caret: usize,
    selection: Range<usize>,
    composing: bool,
}

#[derive(Clone)]
pub(super) struct Ticket {
    token: uuid::Uuid,
    target: EntityId,
    revision: u64,
    directory_revision: u64,
    directory: String,
    cursor: CursorSnapshot,
    plan: Option<CompletionPlan>,
    commands: bool,
    query_directory: Option<String>,
}

#[derive(Clone)]
pub(super) struct Choice {
    ticket: Arc<Ticket>,
    candidate: LiteralCandidate,
    kind: CompletionKind,
    is_symlink: bool,
}

struct Worker {
    token: uuid::Uuid,
    cancel: Option<oneshot::Sender<()>>,
}

pub(super) struct CompletionState {
    pub directory: Entity<InputState>,
    directories: HashMap<EntityId, String>,
    target: Option<EntityId>,
    directory_revision: u64,
    observed: Option<CursorSnapshot>,
    worker: Option<Worker>,
    ticket: Option<Arc<Ticket>>,
    pub message: Option<Message>,
    pub choices: Vec<Choice>,
    selected: usize,
    scroll: ScrollHandle,
    limited: bool,
    skipped: usize,
    resolved_directory: Option<String>,
}

impl CompletionState {
    pub fn new(directory: Entity<InputState>) -> Self {
        Self {
            directory,
            directories: HashMap::new(),
            target: None,
            directory_revision: 0,
            observed: None,
            worker: None,
            ticket: None,
            message: None,
            choices: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            limited: false,
            skipped: 0,
            resolved_directory: None,
        }
    }
    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn visible(&self) -> bool {
        self.message.is_some() || self.ticket.is_some()
    }
}

enum Reply {
    Candidates(CompletionResult),
    Base(String),
}

impl Workspace {
    fn completion_cursor(&self, window: &mut Window, cx: &mut Context<Self>) -> CursorSnapshot {
        self.command.update(cx, |input, cx| CursorSnapshot {
            text: input.value(),
            caret: input.cursor(),
            selection: input.selected_range(),
            composing: input.marked_text_range(window, cx).is_some(),
        })
    }

    /// Selection notifications are separate from Change; cursor-only moves revoke results too.
    pub(super) fn observe_completion_cursor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.completion_cursor(window, cx);
        if self.remote_completion.observed.as_ref() != Some(&cursor) {
            self.cancel_remote_completion(cx);
            self.remote_completion.observed = Some(cursor);
        }
    }

    pub(super) fn cancel_remote_completion(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.remote_completion;
        let changed = state.visible() || !state.choices.is_empty();
        if let Some(worker) = &mut state.worker
            && let Some(cancel) = worker.cancel.take()
        {
            let _ = cancel.send(());
        }
        state.ticket = None;
        state.message = None;
        state.choices.clear();
        state.resolved_directory = None;
        if changed {
            cx.notify();
        }
    }

    pub(super) fn maintain_remote_completion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.tabs.get(self.active).map(Entity::entity_id);
        if self.remote_completion.target != target {
            self.cancel_remote_completion(cx);
            self.remote_completion.target = target;
            self.remote_completion.directory_revision =
                self.remote_completion.directory_revision.wrapping_add(1);
            self.remote_completion
                .directories
                .retain(|id, _| self.tabs.iter().any(|tab| tab.entity_id() == *id));
            let value = target
                .and_then(|id| self.remote_completion.directories.get(&id))
                .cloned()
                .unwrap_or_default();
            self.remote_completion
                .directory
                .update(cx, |input, cx| input.set_value(value, window, cx));
        }
        if self.command_surface_blocked()
            || self
                .tabs
                .get(self.active)
                .is_none_or(|tab| !tab.read(cx).is_open())
        {
            self.cancel_remote_completion(cx);
        }
    }

    pub(super) fn completion_directory_changed(&mut self, cx: &mut Context<Self>) {
        self.cancel_remote_completion(cx);
        let value = self
            .remote_completion
            .directory
            .read(cx)
            .value()
            .to_string();
        self.remote_completion.directory_revision =
            self.remote_completion.directory_revision.wrapping_add(1);
        if let Some(target) = self.remote_completion.target {
            self.remote_completion.directories.insert(target, value);
        }
        cx.notify();
    }

    fn set_completion_directory(
        &mut self,
        directory: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remote_completion
            .directory
            .update(cx, |input, cx| input.set_value(directory, window, cx));
        self.completion_directory_changed(cx);
    }

    pub(super) fn use_files_completion_directory(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = self
            .tabs
            .get(self.active)
            .and_then(|tab| self.panels.get(&tab.entity_id()))
            .and_then(|panels| panels.files.as_ref())
            .and_then(|files| files.read(cx).completion_directory())
            .map(str::to_owned);
        if let Some(directory) = directory {
            self.set_completion_directory(directory, window, cx);
        } else {
            self.remote_completion.message = Some(Message::new(
                "当前文件面板尚无已读取的目录。",
                "The current file panel has no loaded directory.",
            ));
            cx.notify();
        }
    }

    fn completion_ticket_current(
        &self,
        ticket: &Ticket,
        cursor: &CursorSnapshot,
        cx: &App,
    ) -> bool {
        !self.command_surface_blocked()
            && self
                .tabs
                .get(self.active)
                .is_some_and(|tab| tab.entity_id() == ticket.target && tab.read(cx).is_open())
            && (ticket.plan.is_none() || self.command_target.is_none_or(|id| id == ticket.target))
            && self.command_revision == ticket.revision
            && self.remote_completion.directory_revision == ticket.directory_revision
            && self.remote_completion.directory.read(cx).value().as_str() == ticket.directory
            && &ticket.cursor == cursor
            && !cursor.composing
            && self
                .remote_completion
                .ticket
                .as_ref()
                .is_some_and(|current| current.token == ticket.token)
    }

    pub(super) fn request_remote_completion(
        &mut self,
        base_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.maintain_remote_completion(window, cx);
        if self.command_surface_blocked() || self.remote_completion.busy() {
            return;
        }
        let Some(target) = self
            .tabs
            .get(self.active)
            .filter(|tab| tab.read(cx).is_open())
            .map(Entity::entity_id)
        else {
            return;
        };
        let Some(session) = self.remote_sessions.get(&target).cloned() else {
            return;
        };
        let cursor = self.completion_cursor(window, cx);
        if cursor.composing || (!base_only && self.command_target.is_some_and(|id| id != target)) {
            return;
        }
        self.cancel_remote_completion(cx);
        if !base_only && !cursor.selection.is_empty() {
            self.remote_completion.message = Some(Message::new(
                "请先将光标放在要补全的词中，取消文字选区。",
                "Place a single caret in the word to complete; clear the text selection.",
            ));
            cx.notify();
            return;
        }
        let directory = self
            .remote_completion
            .directory
            .read(cx)
            .value()
            .to_string();
        let mut plan = if base_only {
            None
        } else {
            match analyze_completion(cursor.text.as_str(), cursor.caret) {
                Ok(CompletionAnalysis::Ready(plan)) => Some(plan),
                _ => {
                    self.remote_completion.message = Some(Message::new(
                        "此位置暂不支持安全补全；请使用普通命令或字面路径。",
                        "Completion is unavailable here. Use a plain command or literal path.",
                    ));
                    cx.notify();
                    return;
                }
            }
        };
        let query = if let Some(plan) = &mut plan {
            match plan.query((!directory.is_empty()).then_some(directory.as_str())) {
                Ok(query) => Some(query),
                Err(_) => {
                    self.remote_completion.message = Some(Message::new(
                        "相对路径需要绝对补全目录。可读取 SFTP 起点或使用文件面板目录。",
                        "Relative paths need an absolute completion directory. Read the SFTP base or use the file panel directory.",
                    ));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let commands = matches!(query, Some(CompletionQuery::Commands { .. }));
        let query_directory = match &query {
            Some(CompletionQuery::Paths { directory, .. }) => Some(directory.clone()),
            _ => None,
        };
        let ticket = Ticket {
            token: uuid::Uuid::new_v4(),
            target,
            revision: self.command_revision,
            directory_revision: self.remote_completion.directory_revision,
            directory,
            cursor: cursor.clone(),
            plan,
            commands,
            query_directory,
        };
        let (cancel, cancelled) = oneshot::channel();
        self.remote_completion.observed = Some(cursor);
        self.remote_completion.ticket = Some(Arc::new(ticket.clone()));
        self.remote_completion.message = Some(if base_only {
            Message::new(
                "正在读取 SFTP 起始目录…",
                "Reading the SFTP base directory…",
            )
        } else {
            Message::new("正在只读查询远端…", "Querying the remote host (read only)…")
        });
        self.remote_completion.worker = Some(Worker {
            token: ticket.token,
            cancel: Some(cancel),
        });
        self.remote_completion.selected = 0;
        self.remote_completion.limited = false;
        self.remote_completion.skipped = 0;
        // A network operation outlives its GPUI receiver unless it owns explicit cancellation.
        // Dropping Workspace also drops this sender and cancels the Tokio select branch.
        let receiver = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                tokio::select! {
                    biased;
                    _ = cancelled => None,
                    reply = async {
                        match query {
                            Some(query) => session.complete_remote(match query {
                                CompletionQuery::Commands { prefix } => keelshell_session::CompletionQuery::Commands { prefix },
                                CompletionQuery::Paths { directory, prefix, directories_only } => keelshell_session::CompletionQuery::Paths { directory, prefix, directories_only },
                            }).await.map(Reply::Candidates),
                            None => session.completion_base().await.map(Reply::Base),
                        }
                    } => Some(reply),
                }
            },
        );
        self.command.read(cx).focus_handle(cx).focus(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver.await;
            let _ = this.update_in(cx, |view, window, cx| {
                if view.remote_completion.worker.as_ref().is_some_and(|worker| worker.token == ticket.token) {
                    view.remote_completion.worker = None;
                }
                let cursor = view.completion_cursor(window, cx);
                if !view.completion_ticket_current(&ticket, &cursor, cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Some(Ok(Reply::Base(directory)))) => {
                        view.set_completion_directory(directory, window, cx);
                        view.remote_completion.message = Some(Message::new("已使用 SFTP 起始目录；它不代表终端当前工作目录。", "Using the SFTP base directory; it is not the terminal's working directory."));
                    }
                    Ok(Some(Ok(Reply::Candidates(result)))) => view.receive_remote_completions(ticket, result, cx),
                    Ok(Some(Err(error))) => { view.remote_completion.message = Some(completion_error(error)); }
                    Err(_) => { view.remote_completion.message = Some(completion_error(keelshell_session::CompletionError::Transport)); }
                    Ok(None) => view.cancel_remote_completion(cx),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn receive_remote_completions(
        &mut self,
        mut ticket: Ticket,
        result: CompletionResult,
        cx: &mut Context<Self>,
    ) {
        if let Some(query_directory) = &ticket.query_directory {
            let bound = ticket
                .plan
                .as_ref()
                .zip(result.resolved_directory.as_deref())
                .and_then(|(plan, resolved)| {
                    plan.bind_resolved_directory(query_directory, resolved).ok()
                });
            let Some(bound) = bound else {
                self.remote_completion.message = Some(Message::new(
                    "远端目录校验失败，请重新查询。",
                    "Remote directory validation failed. Query again.",
                ));
                return;
            };
            ticket.plan = Some(bound);
        }
        let ticket = Arc::new(ticket);
        self.remote_completion.ticket = Some(ticket.clone());
        self.remote_completion.limited = result.limited;
        self.remote_completion.skipped =
            result.skipped_unsafe_entries + result.skipped_path_directories;
        self.remote_completion.resolved_directory = result.resolved_directory;
        self.remote_completion.choices = result
            .candidates
            .into_iter()
            .map(|candidate| Choice {
                ticket: ticket.clone(),
                candidate: LiteralCandidate {
                    name: candidate.name,
                    path: candidate.path,
                    is_directory: candidate.kind == CompletionKind::Directory,
                },
                kind: candidate.kind,
                is_symlink: candidate.is_symlink,
            })
            .filter(|choice| {
                choice.ticket.plan.as_ref().is_some_and(|plan| {
                    plan.edit(
                        choice.ticket.cursor.text.as_str(),
                        choice.ticket.cursor.caret,
                        &choice.candidate,
                    )
                    .is_ok()
                })
            })
            .collect();
        self.remote_completion.message = Some(if self.remote_completion.choices.is_empty() {
            Message::new("没有匹配的远端项目。", "No matching remote entries.")
        } else {
            Message::new(
                "选择仅填入当前词；核对完整命令后执行。",
                "Selection replaces this word only. Review the complete command before running.",
            )
        });
        cx.notify();
    }

    pub(super) fn insert_remote_completion(
        &mut self,
        choice: Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.completion_cursor(window, cx);
        if !self.completion_ticket_current(&choice.ticket, &cursor, cx) {
            return;
        }
        let Some(plan) = &choice.ticket.plan else {
            return;
        };
        let Ok(edit) = plan.edit(cursor.text.as_str(), cursor.caret, &choice.candidate) else {
            return;
        };
        self.cancel_remote_completion(cx);
        self.command_target = Some(choice.ticket.target);
        self.command.update(cx, |input, cx| {
            if edit.range.is_empty() {
                input.set_selected_range(edit.range, cx);
                input.replace(edit.replacement, window, cx);
            } else {
                // Replacing an explicit nonempty range is atomic and records the
                // original caret for Undo. Selecting that word first would instead
                // restore a selection the user never made. The platform API uses
                // UTF-16, while the validated domain snapshot uses UTF-8 bytes.
                let start = cursor.text[..edit.range.start].encode_utf16().count();
                let end = start + cursor.text[edit.range.clone()].encode_utf16().count();
                input.replace_text_in_range(Some(start..end), &edit.replacement, window, cx);
            }
            input.set_selected_range(edit.caret_byte..edit.caret_byte, cx);
        });
        self.command.read(cx).focus_handle(cx).focus(window, cx);
        self.status = Message::new(
            "已补全当前词。请核对完整命令和目标，再点击执行。",
            "Word completed. Review the full command and target, then click Run.",
        );
        cx.notify();
    }

    pub(super) fn remote_completion_action(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.remote_completion.visible() {
            return false;
        }
        if key == "escape" {
            self.cancel_remote_completion(cx);
        } else if !self.remote_completion.choices.is_empty() {
            let len = self.remote_completion.choices.len();
            match key {
                "down" => {
                    self.remote_completion.selected = (self.remote_completion.selected + 1) % len
                }
                "up" => {
                    self.remote_completion.selected =
                        (self.remote_completion.selected + len - 1) % len
                }
                "enter" => {
                    let choice =
                        self.remote_completion.choices[self.remote_completion.selected].clone();
                    self.insert_remote_completion(choice, window, cx);
                }
                _ => return false,
            }
            self.remote_completion
                .scroll
                .scroll_to_item(self.remote_completion.selected);
        } else {
            return false;
        }
        cx.stop_propagation();
        cx.notify();
        true
    }
}

fn completion_error(error: keelshell_session::CompletionError) -> Message {
    use keelshell_session::CompletionError as E;
    match error {
        E::InvalidQuery => Message::new(
            "补全目录或名称不受支持，请检查绝对路径。",
            "Unsupported directory or name. Check the absolute path.",
        ),
        E::UnsupportedEnvironment => Message::new(
            "此服务器不支持所需的 SSH 查询或 SFTP 环境。",
            "This server does not support the required SSH query or SFTP environment.",
        ),
        E::InvalidResponse => Message::new(
            "远端响应无法安全用于补全，请重新查询。",
            "The remote response cannot be safely used for completion. Query again.",
        ),
        E::PermissionDenied => Message::new(
            "当前账户无权读取补全目录。",
            "This account cannot read the completion directory.",
        ),
        E::Unavailable => Message::new(
            "补全目录不存在或不可用。",
            "The completion directory does not exist or is unavailable.",
        ),
        E::Timeout => Message::new(
            "远端查询已超时，可以手动重试。",
            "The remote query timed out. You can retry manually.",
        ),
        E::LimitExceeded => Message::new(
            "远端查询响应超过大小限制。",
            "The remote query response exceeded its size limit.",
        ),
        E::Transport => Message::new(
            "远端查询通道失败，请检查 SSH 连接。",
            "The remote query channel failed. Check the SSH connection.",
        ),
    }
}
