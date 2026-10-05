//! Explicit desktop grants and single-use native review for external agents.
use super::*;
use crate::mcp_bridge::{QueueBackend, QueuedRequest};
use keelshell_mcp::{
    AccessPolicy, ActionKind, ActionState, AuthorizationLease, BackendReply, CommandProposal,
    FileChangeProposal, KeelShellMcpServer, McpFailure, Operation, PolicyController, SessionGrant,
    SessionIdentity, SessionMetadata, ToolKind,
};
use std::{collections::BTreeSet, time::Instant};

pub(super) struct Target {
    entity: EntityId,
    identity: SessionIdentity,
    label: String,
    roots: Vec<String>,
    tools: BTreeSet<ToolKind>,
    selection: Option<(uuid::Uuid, String)>,
    session: keelshell_session::SshSession,
}

struct Action {
    proposal: ReviewedProposal,
    lease: AuthorizationLease,
    entity: EntityId,
    route: String,
    label: String,
    deadline: Instant,
    state: ActionState,
    output: String,
    session: keelshell_session::SshSession,
}

enum ReviewedProposal {
    Command(CommandProposal),
    File {
        proposal: Box<FileChangeProposal>,
        baseline: keelshell_session::sftp::RegularFileSnapshot,
        diff: String,
    },
}
impl ReviewedProposal {
    fn id(&self) -> uuid::Uuid {
        match self {
            Self::Command(p) => p.id,
            Self::File { proposal, .. } => proposal.id,
        }
    }
    fn target(&self) -> SessionIdentity {
        match self {
            Self::Command(p) => p.target,
            Self::File { proposal, .. } => proposal.target,
        }
    }
    fn digest(&self) -> &str {
        match self {
            Self::Command(p) => &p.digest,
            Self::File { proposal, .. } => &proposal.digest,
        }
    }
    fn kind(&self) -> ActionKind {
        match self {
            Self::Command(_) => ActionKind::Command,
            Self::File { .. } => ActionKind::FileChange,
        }
    }
}

enum Completion {
    FilePrepared {
        revision: uuid::Uuid,
        preparation_id: uuid::Uuid,
        entity: EntityId,
        label: String,
        route: String,
        proposal: Box<FileChangeProposal>,
        lease: AuthorizationLease,
        session: keelshell_session::SshSession,
        reply: tokio::sync::oneshot::Sender<Result<BackendReply, McpFailure>>,
        result: Result<(keelshell_session::sftp::RegularFileSnapshot, String), McpFailure>,
    },
    Granted {
        revision: uuid::Uuid,
        target: Target,
        result: Result<(), McpFailure>,
    },
    Started {
        revision: uuid::Uuid,
        result: Result<keelshell_mcp::DesktopIpcHost, McpFailure>,
        executable: Option<std::path::PathBuf>,
    },
    Executed {
        id: uuid::Uuid,
        state: ActionState,
        output: String,
    },
}

#[cfg(test)]
pub(in crate::workspace) struct HeldFilePreparation(Completion);

pub(super) struct McpState {
    pub(super) show: bool,
    pub(super) root: Entity<InputState>,
    draft_entity: Option<EntityId>,
    draft_tools: BTreeSet<ToolKind>,
    draft_selection: Option<(uuid::Uuid, String)>,
    targets: Vec<Target>,
    actions: Vec<Action>,
    pub(super) reviewing: Option<uuid::Uuid>,
    preparing_files: usize,
    // Each completion releases only its own reservation, including after a
    // failed regrant changes the UI revision without replacing authority.
    preparing_file_ids: BTreeSet<(uuid::Uuid, uuid::Uuid)>,
    #[cfg(test)]
    defer_file_completions: bool,
    #[cfg(test)]
    held_file_completions: Vec<Completion>,
    authority: PolicyController,
    backend: Arc<QueueBackend>,
    requests: mpsc::Receiver<QueuedRequest>,
    results: mpsc::Receiver<Completion>,
    result_sender: mpsc::Sender<Completion>,
    workers: Vec<tokio::task::JoinHandle<()>>,
    revision: uuid::Uuid,
    pub(super) busy: bool,
    pub(super) status: Message,
    // Listener and temporary capability die with this desktop grant scope.
    // A policy never confers execution authority on the external client.
    host: Option<keelshell_mcp::DesktopIpcHost>,
    executable: Option<std::path::PathBuf>,
}

impl McpState {
    pub(super) fn new(root: Entity<InputState>) -> Self {
        let (backend, requests) = QueueBackend::channel();
        let (result_sender, results) = mpsc::channel(8);
        Self {
            show: false,
            root,
            draft_entity: None,
            draft_tools: BTreeSet::new(),
            draft_selection: None,
            targets: Vec::new(),
            actions: Vec::new(),
            reviewing: None,
            preparing_files: 0,
            preparing_file_ids: BTreeSet::new(),
            #[cfg(test)]
            defer_file_completions: false,
            #[cfg(test)]
            held_file_completions: Vec::new(),
            authority: PolicyController::default(),
            backend,
            requests,
            results,
            result_sender,
            workers: Vec::new(),
            revision: uuid::Uuid::new_v4(),
            busy: false,
            status: Message::new(
                "默认关闭；授权只在本次应用运行中有效。",
                "Off by default; grants last only for this application run.",
            ),
            host: None,
            executable: None,
        }
    }
    fn stop(&mut self) {
        self.revision = uuid::Uuid::new_v4();
        let _ = self.authority.disable();
        self.host = None;
        self.executable = None;
        self.targets.clear();
        self.busy = false;
        self.reviewing = None;
        self.clear_file_preparations();
        #[cfg(test)]
        {
            self.defer_file_completions = false;
            self.held_file_completions.clear();
        }
        for worker in self.workers.drain(..) {
            worker.abort();
        }
        for action in &mut self.actions {
            action.state = match action.state {
                ActionState::PendingReview => ActionState::Cancelled,
                ActionState::Running => ActionState::OutcomeUnknown,
                state => state,
            };
        }
        while self.results.try_recv().is_ok() {}
        while let Ok(request) = self.requests.try_recv() {
            let _ = request.reply.send(Err(McpFailure::Disabled));
        }
    }
    fn reserve_file_preparation(&mut self) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        self.preparing_file_ids.insert((self.revision, id));
        self.preparing_files = self.preparing_file_ids.len();
        id
    }
    fn release_file_preparation(&mut self, revision: uuid::Uuid, id: uuid::Uuid) -> bool {
        let owned = self.preparing_file_ids.remove(&(revision, id));
        self.preparing_files = self.preparing_file_ids.len();
        owned
    }
    fn clear_file_preparations(&mut self) {
        self.preparing_file_ids.clear();
        self.preparing_files = 0;
    }
}
impl Drop for McpState {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Workspace {
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_preparing_count(&self) -> usize {
        assert_eq!(self.mcp.preparing_files, self.mcp.preparing_file_ids.len());
        self.mcp.preparing_files
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_defer_file_prepared(&mut self) {
        self.mcp.defer_file_completions = true;
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_hold_file_prepared(
        &mut self,
    ) -> Option<HeldFilePreparation> {
        self.mcp
            .held_file_completions
            .pop()
            .map(HeldFilePreparation)
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_return_file_prepared(
        &mut self,
        held: HeldFilePreparation,
    ) {
        self.mcp.defer_file_completions = false;
        self.mcp
            .result_sender
            .try_send(held.0)
            .unwrap_or_else(|error| panic!("return owned file completion: {error}"));
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_disable(&mut self) {
        self.mcp.stop();
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_draft_contains(&self, tool: ToolKind) -> bool {
        self.mcp.draft_tools.contains(&tool)
    }
    #[cfg(test)]
    pub(in crate::workspace) fn mcp_test_file_review(
        &self,
        id: uuid::Uuid,
    ) -> Option<(&str, &str)> {
        self.mcp
            .actions
            .iter()
            .find(|action| action.proposal.id() == id)
            .and_then(|action| match &action.proposal {
                ReviewedProposal::File { proposal, diff, .. } => {
                    Some((proposal.path.as_str(), diff.as_str()))
                }
                ReviewedProposal::Command(_) => None,
            })
    }
    pub(super) fn mcp_toolbar_label(&self) -> String {
        let pending = self
            .mcp
            .actions
            .iter()
            .filter(|action| matches!(action.state, ActionState::PendingReview))
            .count();
        if pending == 0 {
            "MCP".into()
        } else {
            format!("MCP ({pending})")
        }
    }
    pub(super) fn open_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_surface_blocked() {
            self.focus_current_surface(window, cx);
            return;
        }
        self.mcp.show = true;
        self.mcp.draft_entity = self.tabs.get(self.active).map(Entity::entity_id);
        self.mcp.draft_selection = None;
        self.mcp.draft_tools.clear();
        if let Some(target) = self
            .mcp
            .targets
            .iter()
            .find(|target| Some(target.entity) == self.mcp.draft_entity)
        {
            self.mcp.draft_tools = target.tools.clone();
            self.mcp.draft_selection = target.selection.clone();
            let path = target.roots.first().cloned().unwrap_or_default();
            self.mcp
                .root
                .update(cx, |input, cx| input.set_value(path, window, cx));
        } else {
            self.mcp
                .root
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.overlay_focus.focus(window, cx);
        cx.notify();
    }

    fn mcp_target_current(&self, entity: EntityId, cx: &App) -> bool {
        self.tabs
            .iter()
            .find(|tab| tab.entity_id() == entity)
            .is_some_and(|tab| tab.read(cx).is_open())
            && self.remote_sessions.get(&entity).is_some_and(|session| {
                !session.is_closed()
                    && self
                        .mcp
                        .targets
                        .iter()
                        .find(|target| target.entity == entity)
                        .is_none_or(|target| target.session.same_connection(session))
            })
            && self
                .reconnect_bindings
                .get(&entity)
                .is_none_or(|binding| self.binding_current(binding))
    }

    pub(super) fn maintain_mcp(&mut self, cx: &mut Context<Self>) {
        if self
            .mcp
            .targets
            .iter()
            .any(|target| !self.mcp_target_current(target.entity, cx))
        {
            self.mcp.stop();
            self.mcp.status = Message::new(
                "会话、路线或信任已变化；全部 MCP 授权已撤销。",
                "Session, route or trust changed; all MCP grants were revoked.",
            );
            cx.notify();
        }
        self.mcp.workers.retain(|worker| !worker.is_finished());
        for action in &mut self.mcp.actions {
            if matches!(action.state, ActionState::PendingReview)
                && Instant::now() >= action.deadline
            {
                action.state = ActionState::Expired;
                cx.notify();
            }
        }
        for _ in 0..8 {
            let Ok(result) = self.mcp.results.try_recv() else {
                break;
            };
            // Test support owns completed packets before admission so a real
            // background UI tick cannot race the deliberate generation change.
            #[cfg(test)]
            if self.mcp.defer_file_completions && matches!(result, Completion::FilePrepared { .. })
            {
                self.mcp.held_file_completions.push(result);
                continue;
            }
            match result {
                Completion::FilePrepared {
                    revision,
                    preparation_id,
                    entity,
                    label,
                    route,
                    proposal,
                    lease,
                    session,
                    reply,
                    result,
                } => {
                    // Release the precise reservation before rejecting a stale
                    // completion. Old callbacks cannot release a new grant's slot.
                    if !self.mcp.release_file_preparation(revision, preparation_id) {
                        continue;
                    }
                    if revision != self.mcp.revision {
                        continue;
                    }
                    if reply.is_closed() {
                        continue;
                    }
                    let result = result.and_then(|(baseline, diff)| {
                        lease.check()?;
                        if !self.mcp_target_current(entity, cx) {
                            return Err(McpFailure::StaleSession);
                        }
                        if self.mcp.actions.len() >= 32 {
                            return Err(McpFailure::Busy);
                        }
                        let response = BackendReply::PendingFileChange {
                            target: proposal.target,
                            action_id: proposal.id,
                            digest: proposal.digest.clone(),
                        };
                        self.mcp.actions.push(Action {
                            proposal: ReviewedProposal::File {
                                proposal,
                                baseline,
                                diff,
                            },
                            lease,
                            entity,
                            route,
                            label,
                            deadline: Instant::now() + Duration::from_secs(300),
                            state: ActionState::PendingReview,
                            output: String::new(),
                            session,
                        });
                        Ok(response)
                    });
                    let _ = reply.send(result);
                    cx.notify();
                }
                Completion::Granted {
                    revision,
                    target,
                    result,
                } => {
                    if revision != self.mcp.revision {
                        continue;
                    }
                    self.mcp.busy = false;
                    if result.is_ok() && self.mcp_target_current(target.entity, cx) {
                        self.mcp.targets.retain(|old| old.entity != target.entity);
                        self.mcp.targets.push(target);
                        self.apply_mcp_grants(cx);
                    } else {
                        self.mcp.status = Message::new(
                            "授权未启用；请核对活动会话及真实 canonical 目录。",
                            "Grant not enabled; check the active session and actual canonical directory.",
                        );
                    }
                    cx.notify();
                }
                Completion::Started {
                    revision,
                    result,
                    executable,
                } => {
                    if revision != self.mcp.revision {
                        continue;
                    }
                    self.mcp.busy = false;
                    match result {
                        Ok(host)
                            if self
                                .mcp
                                .targets
                                .iter()
                                .all(|target| self.mcp_target_current(target.entity, cx)) =>
                        {
                            self.mcp.host = Some(host);
                            self.mcp.executable = executable;
                            self.mcp.status = if self.mcp.executable.is_some() {
                                Message::new(
                                    "授权已启用；复制一次性配置后可连接外部智能体。",
                                    "Grant enabled; copy temporary launch settings to connect an external agent.",
                                )
                            } else {
                                Message::new(
                                    "授权已启用，但未找到同目录 MCP 程序；请先构建或安装完整应用。",
                                    "Grant enabled, but the companion MCP program is missing; build or install the complete application first.",
                                )
                            };
                        }
                        _ => {
                            self.mcp.stop();
                            self.mcp.status = Message::new(
                                "监听未启动；授权已关闭。",
                                "Listener did not start; access is off.",
                            );
                        }
                    }
                    cx.notify();
                }
                Completion::Executed { id, state, output } => {
                    if let Some(action) = self.mcp.actions.iter_mut().find(|action| {
                        action.proposal.id() == id && matches!(action.state, ActionState::Running)
                    }) {
                        if action.lease.check().is_ok() {
                            action.state = state;
                            action.output = output;
                        } else {
                            // Authority may be replaced after the worker queued
                            // a success but before this UI completion is admitted.
                            action.state = ActionState::OutcomeUnknown;
                            action.output.clear();
                        }
                        cx.notify();
                    }
                }
            }
        }
        for _ in 0..8 {
            let Ok(request) = self.mcp.requests.try_recv() else {
                break;
            };
            self.admit_mcp_request(request, cx);
        }
    }
}

mod grants;
mod requests;
mod review;
mod view;

#[cfg(test)]
impl Workspace {
    pub(in crate::workspace) fn mcp_test_server(&self) -> KeelShellMcpServer {
        KeelShellMcpServer::new(
            self.mcp.backend.clone(),
            self.mcp.authority.clone(),
            8,
            Duration::from_secs(5),
        )
        .unwrap_or_else(|error| panic!("MCP test server: {error}"))
    }
    pub(in crate::workspace) fn mcp_test_enabled(&self) -> bool {
        self.mcp.host.is_some() && !self.mcp.busy
    }
    pub(in crate::workspace) fn mcp_test_client(
        &self,
    ) -> Result<keelshell_mcp::DesktopIpcClient, keelshell_mcp::IpcFailure> {
        use keelshell_mcp::IpcFailure;
        let settings: serde_json::Value = serde_json::from_str(
            &self
                .mcp
                .host
                .as_ref()
                .ok_or(IpcFailure::Unavailable)?
                .launch_environment(),
        )
        .map_err(|_| IpcFailure::InvalidConfiguration)?;
        keelshell_mcp::DesktopIpcClient::new(
            settings["env"][keelshell_mcp::MCP_ADDRESS_ENV]
                .as_str()
                .ok_or(IpcFailure::InvalidConfiguration)?,
            settings["env"][keelshell_mcp::MCP_SECRET_ENV]
                .as_str()
                .ok_or(IpcFailure::InvalidConfiguration)?,
        )
    }
    pub(in crate::workspace) fn mcp_test_state(&self, id: uuid::Uuid) -> Option<ActionState> {
        self.mcp
            .actions
            .iter()
            .find(|action| action.proposal.id() == id)
            .map(|action| action.state)
    }
    pub(in crate::workspace) fn mcp_test_expire(&mut self, id: uuid::Uuid) {
        if let Some(action) = self
            .mcp
            .actions
            .iter_mut()
            .find(|action| action.proposal.id() == id)
        {
            action.deadline = Instant::now() - Duration::from_secs(1);
        }
    }
    pub(in crate::workspace) fn mcp_test_late_success(
        &mut self,
        id: uuid::Uuid,
        cx: &mut Context<Self>,
    ) {
        if let Some(action) = self
            .mcp
            .actions
            .iter_mut()
            .find(|action| action.proposal.id() == id)
        {
            action.state = ActionState::Running;
            self.mcp
                .result_sender
                .try_send(Completion::Executed {
                    id,
                    state: ActionState::Succeeded,
                    output: "must not publish after lease loss".into(),
                })
                .unwrap_or_else(|error| panic!("owned completion fixture: {error}"));
        }
        self.apply_mcp_grants(cx);
    }
    pub(in crate::workspace) fn mcp_test_output(&self, id: uuid::Uuid) -> Option<&str> {
        self.mcp
            .actions
            .iter()
            .find(|action| action.proposal.id() == id)
            .map(|action| action.output.as_str())
    }
}
