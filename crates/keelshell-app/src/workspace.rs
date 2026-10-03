//! Workspace composition: connection library, terminal tabs and reviewed commands.
use gpui_kit::prelude::FluentBuilder;
use keelshell_session::{RetryPolicy, SessionError, SshAuth, SshOptions, SshSession};
use std::{collections::HashMap, sync::Arc, time::Duration};
use zeroize::Zeroizing;

use crate::i18n::{self, Message, t};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
    },
    *,
};
use keelshell_core::{AppState, AuthMethod, Connection, Language, StateStore};
mod command_workflows;
mod commands;
mod credentials;
mod library;
mod library_view;
mod modals;
mod openssh_review;
mod reconnect;
mod reconnect_editor;
mod remote_completion;
mod routing;
use library::{DestinationPrompt, DestinationTarget, FolderForm, LibraryFilter};
use reconnect_editor::ReconnectEditor;
#[cfg(test)]
mod tests;
mod vault;
mod view;

use crate::ai_settings::{AiSettingsEvent, AiSettingsPanel, EphemeralCredentials};
use crate::assistant::{AssistantEvent, AssistantPanel};
use crate::command_history::CommandHistory;
use crate::jump_host_picker::JumpHostPicker;
use crate::proxy_editor::ProxyEditor;
use crate::snippet_editor::{SnippetEditor, SnippetEditorEvent};
use crate::terminal::TerminalView;
use crate::updater::{UpdatePanel, UpdatePanelEvent};
use crate::vault_settings::{VaultSettings, VaultSettingsEvent};
use openssh_review::OpenSshImportReview;

use crate::design::{ACCENT, BORDER, CANVAS as BG, MUTED, SURFACE as PANEL};

actions!(
    keelshell,
    [
        OpenConnections,
        CloseTab,
        ToggleAssistant,
        CompleteRemoteCommand
    ]
);

struct LoginPrompt {
    id: uuid::Uuid,
    connection: Connection,
    secret: Entity<InputState>,
    proxy_secret: Entity<InputState>,
    master: Entity<InputState>,
    confirmation: Entity<InputState>,
    mode: vault::LoginMode,
    busy: bool,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    message: Option<Message>,
    pin: Option<String>,
    route_identity: keelshell_core::RouteIdentity,
}
struct HostApproval {
    attempt: uuid::Uuid,
    index: usize,
    scope: keelshell_core::HostKeyScope,
    connection: Connection,
    fingerprint: String,
    previous: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolPanel {
    Files,
    Commands,
    Tunnels,
}

#[derive(Default)]
struct RemotePanels {
    files: Option<Entity<crate::files::FilesPanel>>,
    monitor: Option<Entity<crate::monitor::MonitorPanel>>,
    tunnels: Option<Entity<crate::tunnels::TunnelsPanel>>,
}

struct ConnectionForm {
    id: Option<uuid::Uuid>,
    password: bool,
    name: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    username: Entity<InputState>,
    folder_id: Option<uuid::Uuid>,
    tags: Entity<InputState>,
    key: Entity<InputState>,
    jump_picker: Entity<JumpHostPicker>,
    proxy_editor: Entity<ProxyEditor>,
    reconnect_editor: Entity<ReconnectEditor>,
}

/// Draft for the no-session SSH entry point.
///
/// It intentionally contains only endpoint and authentication-mode inputs. A
/// submitted draft becomes an in-memory [`Connection`] and is never handed to
/// `StateStore`; the user must explicitly choose “Save as connection” to open
/// the persistent editor.
struct QuickConnectForm {
    host: Entity<InputState>,
    port: Entity<InputState>,
    username: Entity<InputState>,
    key: Entity<InputState>,
}

impl ConnectionForm {
    fn signature(&self, cx: &App) -> Vec<String> {
        let mut signature = vec![
            self.password.to_string(),
            self.name.read(cx).value().to_string(),
            self.host.read(cx).value().to_string(),
            self.port.read(cx).value().to_string(),
            self.username.read(cx).value().to_string(),
            self.folder_id.map(|id| id.to_string()).unwrap_or_default(),
            self.tags.read(cx).value().to_string(),
            self.key.read(cx).value().to_string(),
            self.jump_picker
                .read(cx)
                .selected()
                .map(|id| id.to_string())
                .unwrap_or_default(),
        ];
        signature.extend(self.proxy_editor.read(cx).signature(cx));
        signature.extend(self.reconnect_editor.read(cx).signature(cx));
        signature
    }
}

enum AfterSave {
    None,
    Ai {
        panel: WeakEntity<AiSettingsPanel>,
        credentials: EphemeralCredentials,
        revision: u64,
    },
    Connection {
        id: uuid::Uuid,
        signature: Vec<String>,
    },
    Trust {
        attempt: uuid::Uuid,
        index: usize,
    },
    ConnectionsImported {
        added: usize,
        skipped: usize,
        folders_added: usize,
        warnings: usize,
    },
    ConnectionFavorite {
        favorite: bool,
    },
    ConnectionDeleted {
        name: String,
    },
    FolderSaved {
        id: uuid::Uuid,
        token: uuid::Uuid,
    },
    FolderRemoved {
        id: uuid::Uuid,
        token: uuid::Uuid,
    },
    ConnectionMoved {
        token: uuid::Uuid,
    },
    ConnectionRestored {
        name: String,
    },
    RecentSaved,
    SnippetSaved {
        panel: EntityId,
    },
    SnippetDeleted {
        id: uuid::Uuid,
    },
    CredentialLinked {
        prompt: uuid::Uuid,
        connection: Box<Connection>,
    },
}

pub struct Workspace {
    store: Arc<StateStore>,
    state: AppState,
    tabs: Vec<Entity<TerminalView>>,
    active: usize,
    split: bool,
    split_pair: Option<(EntityId, EntityId)>,
    search: Entity<InputState>,
    command: Entity<TextareaState>,
    form: Option<ConnectionForm>,
    quick_connect: QuickConnectForm,
    quick_password: bool,
    openssh_review: Option<OpenSshImportReview>,
    status: Message,
    show_connections: bool,
    library_filter: LibraryFilter,
    folder_form: Option<FolderForm>,
    destination_prompt: Option<DestinationPrompt>,
    pending_recents: Vec<(Connection, keelshell_core::ConnectionRoute, u64)>,
    overlay_focus: FocusHandle,
    saving: bool,
    show_assistant: bool,
    assistant: Entity<AssistantPanel>,
    ai_credentials: EphemeralCredentials,
    ai_settings: Option<Entity<AiSettingsPanel>>,
    ai_settings_subscription: Option<Subscription>,
    vault_settings: Option<Entity<VaultSettings>>,
    vault_settings_subscription: Option<Subscription>,
    update_panel: Option<Entity<UpdatePanel>>,
    update_panel_subscription: Option<Subscription>,
    command_target: Option<EntityId>,
    command_revision: u64,
    command_record_history: bool,
    remote_completion: remote_completion::CompletionState,
    suggestion_selected: usize,
    suggestion_scroll: ScrollHandle,
    suggestion_request: Option<commands::SuggestionRequest>,
    suggestion_cache: Vec<commands::CommandCandidate>,
    suggestion_job_running: bool,
    command_sources_revision: u64,
    snippet_sources: Arc<Vec<keelshell_core::Snippet>>,
    suggestion_dismissed: Option<(u64, String)>,
    snippet_search: Entity<InputState>,
    snippet_editor: Option<Entity<SnippetEditor>>,
    snippet_subscription: Option<Subscription>,
    snippet_editing: bool,
    snippet_delete: Option<keelshell_core::Snippet>,
    snippet_parameters: Option<Entity<crate::snippet_parameters::SnippetParameters>>,
    parameter_subscription: Option<Subscription>,
    parameter_ticket: Option<command_workflows::ParameterTicket>,
    batch_panel: Option<Entity<crate::batch_commands::BatchPanel>>,
    batch_subscription: Option<Subscription>,
    show_batch: bool,
    runtime: Arc<tokio::runtime::Runtime>,
    remote_sessions: HashMap<EntityId, SshSession>,
    command_histories: HashMap<EntityId, CommandHistory>,
    login: Option<LoginPrompt>,
    host_approval: Option<HostApproval>,
    connecting: bool,
    connect_route: Option<routing::ConnectRoute>,
    remote_hosts: HashMap<EntityId, String>,
    panels: HashMap<EntityId, RemotePanels>,
    reconnect_bindings: HashMap<EntityId, reconnect::TabBinding>,
    archived_panels: HashMap<EntityId, RemotePanels>,
    show_archived: std::collections::HashSet<EntityId>,
    discard_archive: Option<EntityId>,
    _reconnect_poll: Task<()>,
    visible_panel: Option<ToolPanel>,
    terminal_observers: HashMap<EntityId, Subscription>,
    terminal_focus: HashMap<EntityId, Subscription>,
    _subscriptions: Vec<Subscription>,
}

fn input(placeholder: &str, value: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder(placeholder.to_owned());
        state.set_value(value.to_owned(), window, cx);
        state
    })
}

impl Workspace {
    pub fn new(
        store: Arc<StateStore>,
        state: AppState,
        load_error: Option<String>,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = input(
            t(cx, "搜索连接名称、主机、用户…", "Search connections…"),
            "",
            window,
            cx,
        );
        let command = cx.new(|cx| {
            TextareaState::new(window, cx).rows(2).placeholder(t(
                cx,
                "输入命令，确认目标后执行",
                "Enter a command and review its target",
            ))
        });
        let quick_connect = QuickConnectForm {
            host: input(
                t(cx, "主机或 IP 地址", "Host or IP address"),
                "",
                window,
                cx,
            ),
            port: input(t(cx, "端口", "Port"), "22", window, cx),
            username: input(t(cx, "SSH 用户名", "SSH username"), "", window, cx),
            key: input(
                t(
                    cx,
                    "私钥路径（可选，留空使用 SSH Agent）",
                    "Private key path (optional; empty uses SSH agent)",
                ),
                "",
                window,
                cx,
            ),
        };
        let completion_directory = input(
            t(
                cx,
                "绝对目录，或读取 SFTP 起点",
                "Absolute directory, or read SFTP base",
            ),
            "",
            window,
            cx,
        );
        let completion_directory_subscription = cx.subscribe_in(
            &completion_directory,
            window,
            |view, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.completion_directory_changed(cx);
                }
            },
        );
        let completion_cursor_subscription =
            cx.observe_in(&command, window, |view, _, window, cx| {
                view.observe_completion_cursor(window, cx)
            });
        let snippet_search = input(
            t(
                cx,
                "搜索片段名称、命令、标签…",
                "Search snippets, commands, tags…",
            ),
            "",
            window,
            cx,
        );
        let snippet_subscription =
            cx.subscribe(&snippet_search, |_, _, _: &InputEvent, cx| cx.notify());
        let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let command_subscription = cx.subscribe_in(
            &command,
            window,
            |view, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.cancel_remote_completion(cx);
                    view.command_revision = view.command_revision.wrapping_add(1);
                    view.suggestion_selected = 0;
                    // Undo can restore a deleted draft. Keep its target and
                    // history choice until an explicit new draft clears editing history.
                    if !input.read(cx).value().is_empty() && view.command_target.is_none() {
                        view.command_target = view.tabs.get(view.active).map(Entity::entity_id);
                    }
                    cx.notify();
                } else {
                    cx.notify();
                }
            },
        );
        let ai_credentials = EphemeralCredentials::new();
        let assistant = cx.new(|cx| {
            AssistantPanel::new(
                &state.settings.ai_profiles,
                &ai_credentials,
                runtime.clone(),
                window,
                cx,
            )
        });
        let assistant_subscription = cx.subscribe_in(
            &assistant,
            window,
            |view, panel, event, window, cx| match event {
                AssistantEvent::Capture { selection_only } => {
                    if let Some(terminal) = view.tabs.get(view.active) {
                        let source = terminal.read(cx);
                        let text = if *selection_only {
                            source.selected_text()
                        } else {
                            source.visible_text()
                        };
                        let host = view
                            .remote_hosts
                            .get(&terminal.entity_id())
                            .cloned()
                            .unwrap_or_default();
                        let session_id = format!("{:?}", terminal.entity_id());
                        panel.update(cx, |panel, cx| {
                            panel.set_context(text, host, session_id, cx)
                        });
                    }
                }
                AssistantEvent::OpenSettings => view.open_ai_settings(window, cx),
                AssistantEvent::SelectProfile(id) => {
                    if let Some(profile) = view
                        .state
                        .settings
                        .ai_profiles
                        .profiles
                        .iter()
                        .find(|profile| profile.id == *id)
                    {
                        view.status =
                            Message::detail("当前 AI 配置", "Current AI profile", &profile.name);
                        cx.notify();
                    }
                }
                AssistantEvent::Suggestion {
                    command,
                    session_id,
                } => {
                    if let Some(terminal) = view
                        .tabs
                        .get(view.active)
                        .filter(|tab| format!("{:?}", tab.entity_id()) == *session_id)
                    {
                        let target = terminal.entity_id();
                        view.set_reviewed_command(command.clone(), Some(target), window, cx);
                        view.status = Message::new(
                            "已填入 AI 建议，请核对目标与命令后执行。",
                            "AI suggestion inserted. Review the target and command before running.",
                        );
                    } else {
                        view.status = Message::new(
                            "此建议属于另一会话，请先选择对应终端。",
                            "Suggestion belongs to another session. Select its terminal first.",
                        );
                    }
                    cx.notify();
                }
            },
        );
        let executor = cx.background_executor().clone();
        let reconnect_poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(200)).await;
                if this
                    .update_in(cx, |view, window, cx| view.poll_reconnect(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let snippet_sources = Arc::new(state.snippets.clone());
        Self {
            store,
            state,
            tabs: Vec::new(),
            active: 0,
            split: false,
            split_pair: None,
            search,
            command,
            form: None,
            quick_connect,
            quick_password: false,
            openssh_review: None,
            status: load_error
                .map(|error| Message::detail("配置加载失败", "Configuration failed", error))
                .unwrap_or_else(|| {
                    Message::new("就绪，请选择 SSH 连接", "Ready. Select an SSH connection")
                }),
            show_connections: false,
            library_filter: LibraryFilter::All,
            folder_form: None,
            destination_prompt: None,
            pending_recents: Vec::new(),
            overlay_focus: cx.focus_handle(),
            saving: false,
            show_assistant: false,
            assistant,
            ai_credentials,
            ai_settings: None,
            ai_settings_subscription: None,
            vault_settings: None,
            vault_settings_subscription: None,
            update_panel: None,
            update_panel_subscription: None,
            command_target: None,
            command_revision: 0,
            command_record_history: true,
            remote_completion: remote_completion::CompletionState::new(completion_directory),
            suggestion_selected: 0,
            suggestion_scroll: ScrollHandle::new(),
            suggestion_request: None,
            suggestion_cache: Vec::new(),
            suggestion_job_running: false,
            command_sources_revision: 0,
            snippet_sources,
            suggestion_dismissed: None,
            snippet_search,
            snippet_editor: None,
            snippet_subscription: None,
            snippet_editing: false,
            snippet_delete: None,
            snippet_parameters: None,
            parameter_subscription: None,
            parameter_ticket: None,
            batch_panel: None,
            batch_subscription: None,
            show_batch: false,
            runtime,
            remote_sessions: HashMap::new(),
            command_histories: HashMap::new(),
            login: None,
            host_approval: None,
            connecting: false,
            connect_route: None,
            remote_hosts: HashMap::new(),
            panels: HashMap::new(),
            reconnect_bindings: HashMap::new(),
            archived_panels: HashMap::new(),
            show_archived: Default::default(),
            discard_archive: None,
            _reconnect_poll: reconnect_poll,
            visible_panel: None,
            terminal_observers: HashMap::new(),
            terminal_focus: HashMap::new(),
            _subscriptions: vec![
                subscription,
                assistant_subscription,
                command_subscription,
                snippet_subscription,
                completion_directory_subscription,
                completion_cursor_subscription,
            ],
        }
    }
    fn open_connections(
        &mut self,
        _: &OpenConnections,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_batch
            || self.vault_settings.is_some()
            || self.ai_settings.is_some()
            || self.snippet_modal_open()
            || self.openssh_review.is_some()
        {
            self.focus_current_surface(window, cx);
            return;
        }
        self.show_connections = true;
        self.search.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        // Modal close takes precedence so a keyboard shortcut cannot close its underlying SSH tab.
        if self.show_batch {
            self.show_batch = false;
        } else if self.openssh_review.is_some() {
            self.cancel_openssh_import(window, cx);
        } else if self.discard_archive.take().is_some() {
            cx.notify();
        } else if self.snippet_modal_open() {
            self.close_snippet_modal(window, cx);
        } else if let Some(panel) = self.vault_settings.clone() {
            panel.update(cx, |panel, cx| panel.close(window, cx));
        } else if self.ai_settings.is_some() {
            if !self.saving {
                self.ai_settings = None;
                self.ai_settings_subscription = None;
            }
        } else if self.update_panel.is_some() {
            self.update_panel = None;
            self.update_panel_subscription = None;
        } else if self.login.is_some() {
            self.cancel_login(window, cx);
        } else if self.host_approval.is_some() {
            self.cancel_connect_route(window, cx);
        } else if self.destination_prompt.is_some() {
            if !self.saving {
                self.close_destination(window, cx);
            }
        } else if self.folder_form.is_some() {
            if !self.saving {
                self.close_folder_form(window, cx);
            }
        } else if self.form.is_some() {
            if !self.saving {
                self.form = None;
            }
        } else if self.connecting {
            self.cancel_connect_route(window, cx);
        } else if self.show_connections {
            self.show_connections = false;
        } else if !self.tabs.is_empty() {
            self.cancel_remote_completion(cx);
            let terminal = self.tabs.remove(self.active);
            self.forget_reconnect(terminal.entity_id(), window, cx);
            self.remote_sessions.remove(&terminal.entity_id());
            self.command_histories.remove(&terminal.entity_id());
            self.remote_hosts.remove(&terminal.entity_id());
            self.panels.remove(&terminal.entity_id());
            self.terminal_observers.remove(&terminal.entity_id());
            self.terminal_focus.remove(&terminal.entity_id());
            self.maintain_command_workflows(cx);
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
            if self.tabs.len() < 2
                || self.split_pair.is_some_and(|(first, second)| {
                    first == terminal.entity_id() || second == terminal.entity_id()
                })
            {
                self.split = false;
                self.split_pair = None;
            }
        }
        self.focus_current_surface(window, cx);
        cx.notify();
    }
    fn split_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_surface_blocked() {
            self.focus_current_surface(window, cx);
            return;
        }
        if self.split_companion().is_some() {
            self.split_pair = None;
            self.split = false;
            cx.notify();
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let id = tab.entity_id();
        let Some(session) = self.remote_sessions.get(&id).cloned() else {
            return;
        };
        if !tab.read(cx).is_open() {
            return;
        }
        let binding = self.reconnect_bindings.get(&id).cloned();
        let title = tab.read(cx).title.clone();
        let host = self.remote_hosts.get(&id).cloned().unwrap_or_default();
        let original = self.active;
        self.add_remote_tab(session, title, host, window, cx);
        if let (Some(binding), Some(peer)) = (binding, self.tabs.get(self.active)) {
            self.reconnect_bindings
                .insert(peer.entity_id(), binding.for_split());
        }
        self.split_pair = self
            .tabs
            .get(self.active)
            .map(|peer| (id, peer.entity_id()));
        self.active = original;
        self.split = true;
        // The command bar and keyboard focus share the primary pane target.
        if let Some(primary) = self.tabs.get(self.active) {
            primary.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    fn split_companion(&self) -> Option<Entity<TerminalView>> {
        if !self.split {
            return None;
        }
        let active = self.tabs.get(self.active)?.entity_id();
        let (first, second) = self.split_pair?;
        let companion = if active == first {
            second
        } else if active == second {
            first
        } else {
            return None;
        };
        self.tabs
            .iter()
            .find(|tab| tab.entity_id() == companion)
            .cloned()
    }
    fn watch_terminal(
        &mut self,
        terminal: &Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = terminal.entity_id();
        self.terminal_observers.insert(
            id,
            cx.observe_in(terminal, window, |view, _, window, cx| {
                view.maintain_remote_completion(window, cx);
                view.maintain_command_workflows(cx);
                view.poll_reconnect(window, cx);
                cx.notify();
            }),
        );
        let focus = terminal.read(cx).focus_handle(cx);
        self.terminal_focus.insert(
            id,
            cx.on_focus_in(&focus, window, move |view, _, cx| {
                if let Some(index) = view.tabs.iter().position(|tab| tab.entity_id() == id) {
                    if view.active != index {
                        view.cancel_remote_completion(cx);
                    }
                    view.active = index;
                    cx.notify();
                }
            }),
        );
    }
    fn displayed_terminals(&self) -> Vec<Entity<TerminalView>> {
        if self.split_companion().is_some()
            && let Some((first, second)) = self.split_pair
        {
            return [first, second]
                .iter()
                .filter_map(|id| self.tabs.iter().find(|tab| tab.entity_id() == *id).cloned())
                .collect();
        }
        self.tabs.get(self.active).cloned().into_iter().collect()
    }
    fn add_remote_tab(
        &mut self,
        session: SshSession,
        title: String,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let terminal = crate::ssh_bridge::open(
            session.clone(),
            self.runtime.clone(),
            title,
            self.state.settings.font_size,
            self.state.settings.scrollback_lines,
            cx,
        );
        let id = terminal.entity_id();
        self.remote_hosts.insert(id, host.clone());
        self.watch_terminal(&terminal, window, cx);
        self.remote_sessions.insert(id, session.clone());
        self.command_histories.insert(id, CommandHistory::default());
        let runtime = self.runtime.clone();
        let monitor =
            cx.new(|cx| crate::monitor::MonitorPanel::new(session, host, runtime, window, cx));
        self.panels.insert(
            id,
            RemotePanels {
                monitor: Some(monitor),
                ..Default::default()
            },
        );
        self.tabs.push(terminal);
        self.active = self.tabs.len() - 1;
        self.show_connections = false;
        self.visible_panel = None;
        self.toggle_panel(ToolPanel::Files, window, cx);
        self.focus_current_surface(window, cx);
    }
    fn switch_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.vault_settings.is_some() {
            return;
        }
        let language = match i18n::language(cx) {
            Language::ZhCn => Language::En,
            Language::En => Language::ZhCn,
        };
        i18n::set_language(language, cx);
        if let Some(panel) = &self.snippet_parameters {
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        }
        if let Some(panel) = &self.batch_panel {
            panel.update(cx, |panel, cx| panel.refresh_locale(cx));
        }
        self.remote_completion.directory.update(cx, |input, cx| {
            input.set_placeholder(
                t(
                    cx,
                    "绝对目录，或读取 SFTP 起点",
                    "Absolute directory, or read SFTP base",
                ),
                window,
                cx,
            )
        });
        self.search.update(cx, |input, cx| {
            input.set_placeholder(
                t(
                    cx,
                    "搜索连接名称、主机、用户…",
                    "Search connections, hosts, users…",
                ),
                window,
                cx,
            )
        });
        self.command.update(cx, |input, cx| {
            input.set_placeholder(
                t(
                    cx,
                    "输入命令，确认目标后执行",
                    "Enter a command and review its target",
                ),
                window,
                cx,
            )
        });
        for (field, zh, en) in [
            (
                &self.quick_connect.host,
                "主机或 IP 地址",
                "Host or IP address",
            ),
            (&self.quick_connect.port, "端口", "Port"),
            (&self.quick_connect.username, "SSH 用户名", "SSH username"),
            (
                &self.quick_connect.key,
                "私钥路径（可选，留空使用 SSH Agent）",
                "Private key path (optional; empty uses SSH agent)",
            ),
        ] {
            field.update(cx, |input, cx| {
                input.set_placeholder(t(cx, zh, en), window, cx)
            });
        }
        if let Some(form) = &self.form {
            for (field, zh, en) in [
                (&form.name, "连接名称", "Connection name"),
                (&form.host, "主机或 IP 地址", "Host or IP address"),
                (&form.port, "端口", "Port"),
                (&form.username, "SSH 用户名", "SSH username"),
                (&form.tags, "标签，用逗号分隔", "Tags, separated by commas"),
                (
                    &form.key,
                    "私钥路径（留空使用 SSH Agent）",
                    "Private key path (empty: SSH agent)",
                ),
            ] {
                field.update(cx, |input, cx| {
                    input.set_placeholder(t(cx, zh, en), window, cx)
                });
            }
        }
        if let Some(login) = &self.login {
            login.proxy_secret.update(cx, |input, cx| {
                input.set_placeholder(
                    t(
                        cx,
                        "代理密码（仅本次使用）",
                        "Proxy password (this connection only)",
                    ),
                    window,
                    cx,
                )
            });
            login.secret.update(cx, |input, cx| {
                input.set_placeholder(
                    t(
                        cx,
                        "密码或私钥口令（本次使用，不保存）",
                        "Password or key passphrase (not saved)",
                    ),
                    window,
                    cx,
                )
            });
            login.master.update(cx, |input, cx| {
                input.set_placeholder(t(cx, "凭据库主密码", "Vault master password"), window, cx)
            });
            login.confirmation.update(cx, |input, cx| {
                input.set_placeholder(
                    t(cx, "再次输入主密码", "Confirm master password"),
                    window,
                    cx,
                )
            });
        }
        if let Some(form) = &self.form {
            form.proxy_editor
                .update(cx, |editor, cx| editor.refresh_locale(window, cx));
            form.reconnect_editor
                .update(cx, |editor, cx| editor.refresh_locale(cx));
            form.jump_picker
                .update(cx, |picker, cx| picker.refresh_locale(window, cx));
        }
        self.snippet_search.update(cx, |input, cx| {
            input.set_placeholder(
                t(
                    cx,
                    "搜索片段名称、命令、标签…",
                    "Search snippets, commands, tags…",
                ),
                window,
                cx,
            )
        });
        if let Some(panel) = &self.snippet_editor {
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        }
        for terminal in &self.tabs {
            terminal.update(cx, |terminal, cx| terminal.refresh_locale(window, cx));
        }
        self.assistant
            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
        if let Some(panel) = &self.ai_settings {
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        }
        for panels in self.panels.values().chain(self.archived_panels.values()) {
            if let Some(panel) = &panels.files {
                panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            }
            if let Some(panel) = &panels.monitor {
                panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            }
            if let Some(panel) = &panels.tunnels {
                panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            }
        }
        let mut candidate = self.state.clone();
        candidate.settings.language = language;
        self.persist(candidate, AfterSave::None, window, cx);
    }
    fn open_ai_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_batch
            || self.ai_settings.is_some()
            || self.vault_settings.is_some()
            || self.snippet_modal_open()
            || self.connect_route.is_some()
        {
            return;
        }
        let panel = cx.new(|cx| {
            AiSettingsPanel::new(
                &self.state.settings.ai_profiles,
                &self.ai_credentials,
                self.runtime.clone(),
                self.store.path().with_file_name("vault.json"),
                window,
                cx,
            )
        });
        self.ai_settings_subscription = Some(cx.subscribe_in(
            &panel,
            window,
            |view, panel, event, window, cx| match event {
                AiSettingsEvent::Close => {
                    if !view.saving {
                        view.ai_settings = None;
                        view.ai_settings_subscription = None;
                        view.focus_current_surface(window, cx);
                        cx.notify();
                    }
                }
                AiSettingsEvent::Apply {
                    catalog,
                    credentials,
                    revision,
                } => {
                    if view.saving {
                        panel.update(cx, |panel, cx| {
                            panel.report_failure(
                                Message::new(
                                    "请等待当前保存完成",
                                    "Wait for the current save to finish",
                                ),
                                cx,
                            )
                        });
                        return;
                    }
                    let mut candidate = view.state.clone();
                    candidate.settings.ai_profiles = catalog.clone();
                    panel.update(cx, |panel, cx| panel.set_saving(true, cx));
                    view.persist(
                        candidate,
                        AfterSave::Ai {
                            panel: panel.downgrade(),
                            credentials: credentials.clone(),
                            revision: *revision,
                        },
                        window,
                        cx,
                    );
                }
            },
        ));
        self.ai_settings = Some(panel);
        cx.notify();
    }
    fn open_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_batch
            || self.ai_settings.is_some()
            || self.vault_settings.is_some()
            || self.update_panel.is_some()
            || self.snippet_modal_open()
            || self.connect_route.is_some()
        {
            return;
        }
        let panel = cx.new(|cx| UpdatePanel::new(self.runtime.clone(), window, cx));
        self.update_panel_subscription = Some(cx.subscribe_in(
            &panel,
            window,
            |view, _panel, event, window, cx| match event {
                UpdatePanelEvent::Close => {
                    view.update_panel = None;
                    view.update_panel_subscription = None;
                    view.focus_current_surface(window, cx);
                    cx.notify();
                }
            },
        ));
        self.update_panel = Some(panel);
        cx.notify();
    }
    fn toggle_assistant(&mut self, _: &ToggleAssistant, _: &mut Window, cx: &mut Context<Self>) {
        if self.show_batch || self.snippet_modal_open() {
            return;
        }
        self.show_assistant = !self.show_assistant;
        cx.notify();
    }
    fn open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            self.status = Message::new(
                "请等待当前保存完成，再编辑其他连接。",
                "Wait for the current save before opening another connection.",
            );
            cx.notify();
            return;
        }
        self.form = Some(ConnectionForm {
            id: None,
            proxy_editor: cx.new(|cx| ProxyEditor::new(None, window, cx)),
            reconnect_editor: cx.new(|cx| {
                ReconnectEditor::new(keelshell_core::ReconnectPolicy::Manual, window, cx)
            }),
            jump_picker: cx.new(|cx| JumpHostPicker::new(&self.state, None, None, window, cx)),
            password: true,
            name: input(t(cx, "连接名称", "e.g. Staging API"), "", window, cx),
            host: input(
                t(cx, "主机或 IP 地址", "Host or IP address"),
                "",
                window,
                cx,
            ),
            port: input(t(cx, "端口", "Port"), "22", window, cx),
            username: input(t(cx, "SSH 用户名", "SSH username"), "", window, cx),
            folder_id: match self.library_filter {
                LibraryFilter::Folder(id) => id,
                _ => None,
            },
            tags: input(
                t(cx, "标签，用逗号分隔", "Tags, separated by commas"),
                "",
                window,
                cx,
            ),
            key: input(
                t(
                    cx,
                    "私钥路径（留空使用 SSH Agent）",
                    "Private key path (empty: SSH agent)",
                ),
                "",
                window,
                cx,
            ),
        });
        if let Some(form) = &self.form {
            form.name.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    fn quick_connection(&self, cx: &App) -> Result<Connection, Message> {
        let host = self.quick_connect.host.read(cx).value().trim().to_owned();
        let username = self
            .quick_connect
            .username
            .read(cx)
            .value()
            .trim()
            .to_owned();
        let port = self.quick_connect.port.read(cx).value().trim().to_owned();
        let port = port.parse::<u16>().map_err(|_| {
            Message::new(
                "端口须为 1 至 65535 的整数",
                "Port must be a number from 1 to 65535",
            )
        })?;
        let key = self.quick_connect.key.read(cx).value().trim().to_owned();
        let mut connection = Connection::new(format!("{username}@{host}"), host, username);
        connection.port = port;
        connection.auth = if self.quick_password {
            AuthMethod::Password
        } else if key.is_empty() {
            AuthMethod::Agent
        } else {
            AuthMethod::PrivateKey { path: key.into() }
        };
        connection.validate().map_err(|error| {
            Message::detail(
                "快速连接参数无效",
                "Quick connection parameters are invalid",
                error,
            )
        })?;
        Ok(connection)
    }

    /// Submit the no-session draft through the regular SSH route and
    /// authentication flow. This never writes the draft to the connection
    /// library; successful one-time sessions are therefore absent from the
    /// persistent recent-profile list as well.
    pub(super) fn connect_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            return;
        }
        let connection = match self.quick_connection(cx) {
            Ok(connection) => connection,
            Err(message) => {
                self.status = message;
                cx.notify();
                return;
            }
        };
        self.request_ephemeral_connect(connection, window, cx);
    }

    /// Copy the one-time endpoint draft into the normal connection editor.
    /// Persistence still requires the editor's explicit Save action.
    pub(super) fn save_quick_as_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || !self.tabs.is_empty() {
            return;
        }
        let host = self.quick_connect.host.read(cx).value().to_string();
        let port = self.quick_connect.port.read(cx).value().to_string();
        let username = self.quick_connect.username.read(cx).value().to_string();
        let key = self.quick_connect.key.read(cx).value().to_string();
        let name = format!("{}@{}", username.trim(), host.trim());
        let password = self.quick_password;
        self.open_form(window, cx);
        if let Some(form) = self.form.as_ref() {
            form.name
                .update(cx, |input, cx| input.set_value(name, window, cx));
            form.host
                .update(cx, |input, cx| input.set_value(host, window, cx));
            form.port
                .update(cx, |input, cx| input.set_value(port, window, cx));
            form.username
                .update(cx, |input, cx| input.set_value(username, window, cx));
            form.key
                .update(cx, |input, cx| input.set_value(key, window, cx));
        }
        if let Some(form) = self.form.as_mut() {
            form.password = password;
        }
        cx.notify();
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(form) = self.form.as_ref() else {
            return;
        };
        let mut connection = Connection::new(
            form.name.read(cx).value().to_string(),
            form.host.read(cx).value().to_string(),
            form.username.read(cx).value().to_string(),
        );
        connection.jump_host = form.jump_picker.read(cx).selected();
        match form.reconnect_editor.read(cx).draft(cx) {
            Ok(policy) => connection.reconnect = policy,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        }
        match form.proxy_editor.read(cx).draft(cx) {
            Ok(proxy) => connection.proxy = proxy,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        }
        connection.tags = form
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect();
        let folder_id = form.folder_id;
        if let Some(id) = form.id {
            connection.id = id;
            if let Some(original) = self.state.connections.iter().find(|item| item.id == id) {
                connection.group = original.group.clone();
                connection.favorite = original.favorite;
            }
        }
        let port = form.port.read(cx).value();
        match port.parse::<u16>() {
            Ok(port) => connection.port = port,
            Err(_) => {
                self.status = Message::new(
                    "端口须为 1 至 65535 的整数",
                    "Port must be a number from 1 to 65535",
                );
                cx.notify();
                return;
            }
        }
        let key = form.key.read(cx).value();
        if form.password {
            connection.auth = AuthMethod::Password;
        } else if !key.trim().is_empty() {
            connection.auth = AuthMethod::PrivateKey {
                path: key.trim().into(),
            };
        }
        if connection.proxy.is_some() && !connection.host.is_ascii() {
            self.status = Message::new(
                "使用代理时，目标主机须为 ASCII 域名或 IP；国际域名请使用 punycode 格式。",
                "A proxied target requires an ASCII hostname or IP; use punycode for internationalized domains.",
            );
            cx.notify();
            return;
        }
        if let Err(error) = connection.validate() {
            self.status = Message::detail("操作失败", "Operation failed", error);
            cx.notify();
            return;
        }
        if let Some(original) = self
            .state
            .connections
            .iter()
            .find(|item| item.id == connection.id)
            && vault::same_destination(original, &connection)
        {
            connection.credential_ref = original.credential_ref;
        }
        let after = AfterSave::Connection {
            id: connection.id,
            signature: form.signature(cx),
        };
        let mut candidate = self.state.clone();
        let connection_id = connection.id;
        if candidate
            .connections
            .iter()
            .any(|item| item.id == connection.id)
        {
            if let Err(error) = candidate.update_connection(connection) {
                self.status = Message::detail(
                    "连接或跳板路线无效，未保存",
                    "Connection or jump route invalid; not saved",
                    error,
                );
                cx.notify();
                return;
            }
        } else {
            candidate.connections.push(connection);
        }
        if let Err(error) = candidate.move_connection(connection_id, folder_id) {
            self.status = Message::detail("文件夹无效，未保存", "Folder invalid; not saved", error);
            cx.notify();
            return;
        }
        self.persist(candidate, after, window, cx);
    }
    fn export_connections(&mut self, cx: &mut Context<Self>) {
        match self.state.export_connections() {
            Ok(document) => {
                cx.write_to_clipboard(ClipboardItem::new_string(document));
                self.status = Message::new(
                    "连接 JSON 已复制到剪贴板（不包含密码或私钥内容）",
                    "Connection JSON copied to the clipboard (passwords and key contents are excluded)",
                );
            }
            Err(error) => {
                self.status = Message::detail("导出失败", "Export failed", error);
            }
        }
        cx.notify();
    }
    fn import_connections(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            self.status = Message::new(
                "请等待当前保存完成，再导入连接。",
                "Wait for the current save to finish before importing connections.",
            );
            cx.notify();
            return;
        }
        let Some(document) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.status = Message::new(
                "剪贴板中没有可读取的连接 JSON。",
                "The clipboard does not contain readable connection JSON.",
            );
            cx.notify();
            return;
        };
        let mut candidate = self.state.clone();
        match candidate.import_connections(&document) {
            Ok(report) if candidate == self.state => {
                self.status = Message::new(
                    format!("没有新增连接或文件夹，跳过 {} 条重复记录。", report.skipped),
                    format!(
                        "No connections or folders added; skipped {} duplicate records.",
                        report.skipped
                    ),
                );
                cx.notify();
            }
            Ok(report) => {
                let folders_added = candidate
                    .folders
                    .len()
                    .saturating_sub(self.state.folders.len());
                self.persist(
                    candidate,
                    AfterSave::ConnectionsImported {
                        added: report.added,
                        skipped: report.skipped,
                        folders_added,
                        warnings: 0,
                    },
                    window,
                    cx,
                )
            }
            Err(error) => {
                self.status = Message::detail(
                    "导入失败，未修改连接库",
                    "Import failed; the library was not changed",
                    error,
                );
                cx.notify();
            }
        }
    }
    fn toggle_favorite(&mut self, id: uuid::Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let mut candidate = self.state.clone();
        match candidate.toggle_connection_favorite(id) {
            Ok(favorite) => self.persist(
                candidate,
                AfterSave::ConnectionFavorite { favorite },
                window,
                cx,
            ),
            Err(error) => {
                self.status = Message::detail("收藏状态更新失败", "Favorite update failed", error);
                cx.notify();
            }
        }
    }
    fn delete_connection(&mut self, id: uuid::Uuid, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_reconnect_for_profile(id, window, cx);
        if self.saving {
            return;
        }
        let mut candidate = self.state.clone();
        let removed = match candidate.soft_delete_connection(id, library::now_seconds()) {
            Ok(connection) => connection,
            Err(error) => {
                self.status = Message::detail("移入回收站失败", "Move to trash failed", error);
                cx.notify();
                return;
            }
        };
        self.persist(
            candidate,
            AfterSave::ConnectionDeleted { name: removed.name },
            window,
            cx,
        );
    }
    fn persist(
        &mut self,
        candidate: AppState,
        after: AfterSave,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vault_settings.is_some()
            || (self.snippet_modal_open()
                && !matches!(
                    &after,
                    AfterSave::SnippetSaved { .. }
                        | AfterSave::SnippetDeleted { .. }
                        | AfterSave::None
                ))
        {
            self.status = Message::new(
                "请先关闭凭据库管理，再修改配置。",
                "Close vault management before changing configuration.",
            );
            cx.notify();
            return;
        }
        if self.saving {
            self.status = Message::new(
                "正在保存，请完成后重试本次修改。",
                "A save is in progress. Wait for it to finish, then retry this change.",
            );
            cx.notify();
            return;
        }
        self.saving = true;
        self.status = Message::new("正在保存…", "Saving…");
        let store = self.store.clone();
        let state = candidate.clone();
        let task = cx
            .background_executor()
            .spawn(async move { store.save(&state) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.saving = false;
                let saved_successfully = result.is_ok();
                match result {
                    Ok(saved) => {
                        view.state = saved;
                        if let AfterSave::Trust { attempt, index } = &after {
                            view.accept_reconnect_trust(*attempt, *index);
                        }
                        view.invalidate_reconnect_profiles(window, cx);
                        let route_changed=view.invalidate_changed_route(window,cx);
                        view.snippet_sources = Arc::new(view.state.snippets.clone());
                        view.command_sources_revision = view.command_sources_revision.wrapping_add(1);
                        view.status = Message::new("已保存到本机", "Saved locally");
                        match after {
                            AfterSave::None => {}
                            AfterSave::SnippetSaved { panel } => {
                                if view.snippet_editor.as_ref().is_some_and(|editor| editor.entity_id() == panel) {
                                    view.snippet_editor = None; view.snippet_subscription = None;
                                }
                                view.status = Message::new("命令片段已保存", "Command snippet saved");
                                view.focus_current_surface(window, cx);
                            }
                            AfterSave::SnippetDeleted { id } => {
                                if view.snippet_delete.as_ref().is_some_and(|snippet| snippet.id == id) { view.snippet_delete = None; }
                                view.status = Message::new("命令片段已删除", "Command snippet deleted");
                                view.focus_current_surface(window, cx);
                            }
                            AfterSave::Ai {
                                panel,
                                credentials,
                                revision,
                            } => {
                                view.ai_credentials = credentials;
                                view.assistant.update(cx, |assistant, cx| {
                                    assistant.set_profiles(
                                        &view.state.settings.ai_profiles,
                                        &view.ai_credentials,
                                        cx,
                                    )
                                });
                                if let Some(panel) = panel.upgrade() {
                                    panel.update(cx, |panel, cx| panel.mark_saved(revision, cx));
                                }
                            }
                            AfterSave::Connection { id, signature } => {
                                if view
                                    .form
                                    .as_ref()
                                    .is_some_and(|form| form.signature(cx) == signature)
                                {
                                    view.form = None;
                                    view.focus_current_surface(window, cx);
                                } else if let Some(form) = view.form.as_mut() {
                                    form.id = Some(id);
                                    view.status = Message::new(
                                        "已保存，后续输入的修改仍待保存。",
                                        "Saved. Your newer form edits are still pending.",
                                    );
                                }
                            }
                            AfterSave::Trust { attempt, index } => {
                                view.finish_host_trust(attempt, index, window, cx);
                            }
                            AfterSave::ConnectionsImported { added, skipped, folders_added, warnings } => {
                                view.status = Message::new(
                                    format!("已导入 {added} 条连接和 {folders_added} 个文件夹，跳过 {skipped} 条重复记录；有 {warnings} 项配置需要审阅。"),
                                    format!("Imported {added} connections and {folders_added} folders; skipped {skipped} duplicates; {warnings} config items need review."),
                                );
                            }
                            AfterSave::ConnectionFavorite { favorite } => {
                                view.status = Message::new(
                                    if favorite { "已加入收藏" } else { "已取消收藏" },
                                    if favorite { "Added to favorites" } else { "Removed from favorites" },
                                );
                            }
                            AfterSave::ConnectionDeleted { name } => {
                                view.status = Message::new(
                                    format!("已移入回收站：{name}，可随时恢复。"),
                                    format!("Moved to trash: {name}. You can restore it."),
                                );
                            }
                            AfterSave::FolderSaved { id, token } => {
                                if view.folder_form.as_ref().is_some_and(|form| form.token == token) {
                                    view.close_folder_form(window, cx);
                                }
                                view.library_filter = LibraryFilter::Folder(Some(id));
                                view.status = Message::new("文件夹已保存", "Folder saved");
                            }
                            AfterSave::FolderRemoved { id, token } => {
                                if view.folder_form.as_ref().is_some_and(|form| form.token == token) {
                                    view.close_folder_form(window, cx);
                                }
                                if view.library_filter == LibraryFilter::Folder(Some(id)) {
                                    view.library_filter = LibraryFilter::All;
                                }
                                view.status = Message::new("空文件夹已删除", "Empty folder removed");
                            }
                            AfterSave::ConnectionMoved { token } => {
                                if view.destination_prompt.as_ref().is_some_and(|prompt| prompt.token == token) {
                                    view.close_destination(window, cx);
                                }
                                view.status = Message::new("连接已移动", "Connection moved");
                            }
                            AfterSave::ConnectionRestored { name } => {
                                view.status = Message::new(format!("已恢复连接：{name}"), format!("Connection restored: {name}"));
                            }
                            AfterSave::RecentSaved => {
                                view.status = Message::new("已连接，最近使用记录已保存", "Connected; recent usage saved");
                            }
                            AfterSave::CredentialLinked { prompt, connection } => {
                                view.finish_credential_save(prompt, *connection, window, cx);
                            }
                        }
                        if route_changed { view.status=Message::new("配置已保存；连接路线已变化，当前连接尝试已取消。", "Profile saved; the route changed, so the current connection attempt was cancelled."); }
                    }
                    Err(error) => {
                        if matches!(&after, AfterSave::SnippetSaved { .. }) && let Some(panel) = &view.snippet_editor {
                            panel.update(cx, |panel, cx| panel.set_error(Message::detail("保存失败，草稿已保留；配置冲突时请重启后重试", "Save failed; draft preserved. Restart after a configuration conflict", &error), cx));
                        }
                        let organization_failure = Message::detail(
                            "保存失败，草稿已保留；如配置已被修改，请重启后重试",
                            "Save failed; draft retained. If configuration changed, restart before retrying",
                            &error,
                        );
                        if matches!(&after, AfterSave::FolderSaved { .. } | AfterSave::FolderRemoved { .. })
                            && let Some(form) = &mut view.folder_form {
                            form.message = Some(organization_failure.clone());
                        }
                        if matches!(&after, AfterSave::ConnectionMoved { .. })
                            && let Some(prompt) = &mut view.destination_prompt {
                            prompt.message = Some(organization_failure);
                        }
                        let credential_failure = if let AfterSave::CredentialLinked { prompt, .. } = &after {
                            let message = if matches!(error, keelshell_core::Error::Conflict) {
                                Message::new(
                                    "连接配置保存失败：配置已被其他操作修改，未发起连接，密码输入已清空。加密条目可能已保存。请先保存其他工作，再重启应用以重新加载配置后重试。",
                                    "Profile save failed because the configuration changed. No connection started; password inputs were cleared. An encrypted entry may remain. Save your other work, then restart the app to reload the configuration before retrying.",
                                )
                            } else {
                                Message::new(
                                    "连接配置保存失败，未发起连接，密码输入已清空。加密条目可能已保存，请检查配置文件及其访问权限。",
                                    "Profile save failed; no connection started and password inputs were cleared. An encrypted entry may remain. Check the configuration file and its permissions.",
                                )
                            };
                            if let Some(login) = view.login.as_mut().filter(|login| login.id == *prompt) {
                                login.busy = false;
                                login.message = Some(message.clone());
                            }
                            Some(message)
                        } else {
                            None
                        };
                        if let AfterSave::Ai { panel, .. } = after
                            && let Some(panel) = panel.upgrade()
                        {
                            panel.update(cx, |panel, cx| {
                                panel.report_failure(
                                    Message::detail(
                                        "保存失败，草稿已保留",
                                        "Save failed; draft preserved",
                                        &error,
                                    ),
                                    cx,
                                )
                            });
                        }
                        view.status = credential_failure.unwrap_or_else(|| Message::detail(
                            "保存失败，输入内容已保留",
                            "Save failed; input preserved",
                            error,
                        ))
                    }
                };
                view.resume_connect_route(window, cx);
                if saved_successfully {
                    view.flush_recent_connections(window, cx);
                } else {
                    view.pending_recents.clear();
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn import_openssh_connections(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.openssh_review.is_some() {
            self.status = Message::new(
                "请等待当前保存完成，再导入 SSH 配置。",
                "Wait for the current save to finish before importing SSH configuration.",
            );
            cx.notify();
            return;
        }
        let Some(document) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.status = Message::new(
                "剪贴板中没有可读取的 SSH 配置。",
                "The clipboard does not contain readable SSH configuration.",
            );
            cx.notify();
            return;
        };
        let mut candidate = self.state.clone();
        match candidate.import_openssh_config_report(&document) {
            Ok(report) if report.imported.added == 0 && report.warnings.is_empty() => {
                self.status = Message::new(
                    format!(
                        "没有新增连接，跳过 {} 条重复记录；有 {} 项需审阅。",
                        report.imported.skipped,
                        report.warnings.len()
                    ),
                    format!(
                        "No connections added; skipped {} duplicates; {} items need review.",
                        report.imported.skipped,
                        report.warnings.len()
                    ),
                );
                cx.notify();
            }
            Ok(report) => {
                self.openssh_review = Some(OpenSshImportReview::new(candidate, report));
                self.status = Message::new(
                    "请审阅 SSH 配置导入内容，确认后才会保存。",
                    "Review the SSH import; nothing is saved until you confirm.",
                );
                self.focus_current_surface(window, cx);
                cx.notify();
            }
            Err(error) => {
                self.status = Message::detail(
                    "SSH 配置导入失败，未修改连接库",
                    "SSH configuration import failed; the library was not changed",
                    error,
                );
                cx.notify();
            }
        }
    }
    fn run_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_surface_blocked() {
            return;
        }
        let text = self.command.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        if let Some(terminal) = self.tabs.get(self.active) {
            if self
                .command_target
                .is_some_and(|target| target != terminal.entity_id())
            {
                self.status = Message::new(
                    "命令属于另一会话，请切换到对应标签再执行。",
                    "Command belongs to another tab. Select that tab before running.",
                );
                cx.notify();
                return;
            }
            let queued = terminal.update(cx, |view, cx| {
                let result = view.try_write(format!("{text}\r").into_bytes());
                view.focus(window, cx);
                cx.notify();
                result
            });
            if let Err(error) = queued {
                self.status = error;
                cx.notify();
                return;
            }
            if self.command_record_history {
                self.command_histories
                    .entry(terminal.entity_id())
                    .or_default()
                    .record(&text);
            }
            self.command_sources_revision = self.command_sources_revision.wrapping_add(1);
            self.set_reviewed_command(String::new(), None, window, cx);
            self.status = Message::new(
                "命令已加入当前远程会话发送队列",
                "Command queued in the active terminal",
            );
        } else {
            self.status = Message::new(
                "请先连接 SSH 主机再执行命令",
                "Open a terminal before running a command",
            );
        }
        cx.notify();
    }
    fn clear_command_history(&mut self, cx: &mut Context<Self>) {
        let Some(terminal) = self.tabs.get(self.active) else {
            return;
        };
        let changed = self
            .command_histories
            .get_mut(&terminal.entity_id())
            .is_some_and(CommandHistory::clear);
        self.command_sources_revision = self.command_sources_revision.wrapping_add(1);
        self.status = if changed {
            Message::new(
                "当前 SSH 会话的命令历史已清空",
                "Command history cleared for this SSH session",
            )
        } else {
            Message::new(
                "当前 SSH 会话没有命令历史",
                "This SSH session has no command history",
            )
        };
        cx.notify();
    }
    fn toggle_panel(&mut self, kind: ToolPanel, window: &mut Window, cx: &mut Context<Self>) {
        if self.remote_completion.visible() {
            self.cancel_remote_completion(cx);
            if self.visible_panel == Some(kind) {
                cx.notify();
                return;
            }
        }
        if kind == ToolPanel::Commands {
            self.visible_panel = if self.visible_panel == Some(kind) {
                None
            } else {
                Some(kind)
            };
            cx.notify();
            return;
        }
        let Some(terminal) = self.tabs.get(self.active) else {
            return;
        };
        let id = terminal.entity_id();
        if self.show_archived.contains(&id) || terminal.read(cx).end_reason().is_some() {
            self.visible_panel = if self.visible_panel == Some(kind) {
                None
            } else {
                Some(kind)
            };
            cx.notify();
            return;
        }
        let Some(session) = self.remote_sessions.get(&id).cloned() else {
            self.status = Message::new(
                "请先建立 SSH 连接",
                "Remote tools are available in a connected SSH tab",
            );
            cx.notify();
            return;
        };
        let host = self.remote_hosts.get(&id).cloned().unwrap_or_default();
        let runtime = self.runtime.clone();
        let panels = self.panels.entry(id).or_default();
        match kind {
            ToolPanel::Files if panels.files.is_none() => {
                panels.files =
                    Some(cx.new(|cx| {
                        crate::files::FilesPanel::new(session, host, runtime, window, cx)
                    }));
            }
            ToolPanel::Tunnels if panels.tunnels.is_none() => {
                panels.tunnels = Some(cx.new(|cx| {
                    crate::tunnels::TunnelsPanel::new(session, host, runtime, window, cx)
                }));
            }
            _ => {}
        }
        self.visible_panel = if self.visible_panel == Some(kind) {
            None
        } else {
            Some(kind)
        };
        cx.notify();
    }
    fn edit_connection(
        &mut self,
        connection: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        self.cancel_reconnect_for_profile(connection.id, window, cx);
        if self.editing_active_route(connection.id) {
            self.cancel_connect_route(window, cx);
        }
        let key = match &connection.auth {
            AuthMethod::PrivateKey { path } => path.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        self.form = Some(ConnectionForm {
            id: Some(connection.id),
            proxy_editor: cx.new(|cx| ProxyEditor::new(connection.proxy.as_ref(), window, cx)),
            reconnect_editor: cx.new(|cx| ReconnectEditor::new(connection.reconnect, window, cx)),
            jump_picker: cx.new(|cx| {
                JumpHostPicker::new(
                    &self.state,
                    Some(connection.id),
                    connection.jump_host,
                    window,
                    cx,
                )
            }),
            password: matches!(connection.auth, AuthMethod::Password),
            name: input(t(cx, "名称", "Name"), &connection.name, window, cx),
            host: input(t(cx, "主机", "Host"), &connection.host, window, cx),
            port: input(
                t(cx, "端口", "Port"),
                &connection.port.to_string(),
                window,
                cx,
            ),
            username: input(
                t(cx, "用户名", "Username"),
                &connection.username,
                window,
                cx,
            ),
            folder_id: self.state.folder_id_of(connection.id),
            tags: input(
                t(cx, "标签，用逗号分隔", "Tags, separated by commas"),
                &connection.tags.join(", "),
                window,
                cx,
            ),
            key: input(t(cx, "私钥路径", "Private key path"), &key, window, cx),
        });
        if let Some(form) = &self.form {
            form.name.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
}

pub fn bind_keys(cx: &mut App) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl-shift"
    };
    cx.bind_keys([
        KeyBinding::new("ctrl-space", CompleteRemoteCommand, Some("Input")),
        KeyBinding::new(&format!("{modifier}-t"), OpenConnections, None),
        KeyBinding::new(&format!("{modifier}-w"), CloseTab, None),
        KeyBinding::new(&format!("{modifier}-j"), ToggleAssistant, None),
    ]);
}
