//! Desktop-owned Agent transport. Model text cannot grant or replace authority.
use super::*;
use keelshell_ai::{AgentAction, AgentOutcome, RequestCancellation};
use keelshell_session::sftp::RegularFileSnapshot;
use std::time::Instant;
use tokio::sync::watch;

use crate::terminal::TransportState;

const ACTION_DEADLINE: Duration = Duration::from_secs(120);
const OPERATION_DEADLINE: Duration = Duration::from_secs(30);
const MAX_BYTES: usize = 32 * 1024;

pub(super) struct AgentState {
    run_id: uuid::Uuid,
    entity: EntityId,
    session: SshSession,
    lifecycle: Option<watch::Receiver<TransportState>>,
    cancellation: RequestCancellation,
    pending: Option<PendingAction>,
    job: Option<Task<()>>,
}
struct PendingAction {
    id: uuid::Uuid,
    action: AgentAction,
    deadline: Instant,
    baseline: Option<RegularFileSnapshot>,
    preparing: bool,
    dispatched: bool,
}
impl Drop for AgentState {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
impl Workspace {
    #[cfg(test)]
    pub(super) fn agent_cancellation_for_test(&self) -> Option<RequestCancellation> {
        self.agent.as_ref().map(|state| state.cancellation.clone())
    }
    #[cfg(test)]
    pub(super) fn agent_backend_current_for_test(&self) -> bool {
        self.agent.as_ref().is_some_and(|state| {
            !state.cancellation.is_cancelled()
                && !state.session.is_closed()
                && lifecycle_current(state.lifecycle.as_ref())
        })
    }
    #[cfg(test)]
    pub(super) fn expire_agent_review_for_test(&mut self) {
        if let Some(state) = &mut self.agent
            && let Some(pending) = &mut state.pending
        {
            pending.deadline = Instant::now() - Duration::from_secs(1);
        }
    }
    #[cfg(test)]
    pub(super) fn agent_pending_for_test(&self) -> Option<(uuid::Uuid, uuid::Uuid)> {
        let state = self.agent.as_ref()?;
        Some((state.run_id, state.pending.as_ref()?.id))
    }

    fn agent_target_current(&self, state: &AgentState, cx: &App) -> bool {
        !state.cancellation.is_cancelled()
            && !state.session.is_closed()
            && lifecycle_current(state.lifecycle.as_ref())
            && self.protocol_session_current(state.entity, &state.session)
            && self
                .tabs
                .iter()
                .find(|tab| tab.entity_id() == state.entity)
                .is_some_and(|tab| tab.read(cx).is_open())
    }
    pub(super) fn start_agent(
        &mut self,
        run_id: uuid::Uuid,
        session_id: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.assistant.read(cx).agent_target_requested(run_id) {
            return;
        }
        self.agent = None;
        let entity = self
            .tabs
            .iter()
            .find(|tab| format!("{:?}", tab.entity_id()) == session_id)
            .map(Entity::entity_id);
        if let Some(entity) = entity
            && let Some(session) = self.remote_sessions.get(&entity).cloned()
        {
            let state = AgentState {
                run_id,
                entity,
                session,
                lifecycle: self
                    .tabs
                    .iter()
                    .find(|tab| tab.entity_id() == entity)
                    .and_then(|tab| tab.read(cx).subscribe_lifecycle()),
                cancellation: RequestCancellation::new(),
                pending: None,
                job: None,
            };
            if self.agent_target_current(&state, cx) {
                self.agent = Some(state);
            }
        }
        let cancellation = self.agent.as_ref().map(|state| state.cancellation.clone());
        self.assistant.update(cx, |panel, cx| {
            panel.agent_target_accepted(run_id, cancellation, cx)
        });
    }
    pub(super) fn maintain_agent(&mut self, cx: &mut Context<Self>) {
        if self
            .agent
            .as_ref()
            .is_some_and(|state| !self.agent_target_current(state, cx))
        {
            let Some(state) = self.agent.take() else {
                return;
            };
            let run_id = state.run_id;
            drop(state);
            self.assistant
                .update(cx, |panel, cx| panel.agent_target_lost(run_id, cx));
        }
        let expired = self.agent.as_ref().and_then(|state| {
            state
                .pending
                .as_ref()
                .filter(|p| !p.dispatched && !p.preparing && Instant::now() >= p.deadline)
                .map(|p| (state.run_id, p.id))
        });
        if let Some((run_id, id)) = expired {
            self.reject_agent_action(run_id, id);
            self.assistant.update(cx, |panel, cx| {
                panel.agent_action_finished(
                    run_id,
                    id,
                    AgentOutcome::Failed {
                        reason: "Action review expired before dispatch".into(),
                    },
                    cx,
                )
            });
        }
    }
    pub(super) fn stop_agent(&mut self, run_id: uuid::Uuid) {
        if self
            .agent
            .as_ref()
            .is_some_and(|state| state.run_id == run_id)
        {
            self.agent = None;
        }
    }
    pub(super) fn register_agent_proposal(
        &mut self,
        run_id: uuid::Uuid,
        id: uuid::Uuid,
        action: &AgentAction,
        cx: &mut Context<Self>,
    ) {
        self.maintain_agent(cx);
        let Some(state) = &mut self.agent else {
            return;
        };
        if state.run_id != run_id || state.pending.is_some() {
            return;
        }
        state.pending = Some(PendingAction {
            id,
            action: action.clone(),
            deadline: Instant::now() + ACTION_DEADLINE,
            baseline: None,
            preparing: false,
            dispatched: false,
        });
    }
    pub(super) fn reject_agent_action(&mut self, run_id: uuid::Uuid, id: uuid::Uuid) {
        if let Some(state) = &mut self.agent
            && state.run_id == run_id
            && state
                .pending
                .as_ref()
                .is_some_and(|p| p.id == id && !p.dispatched)
        {
            state.pending = None;
        }
    }
    fn admit_agent_action(
        &mut self,
        run_id: uuid::Uuid,
        id: uuid::Uuid,
        action: &AgentAction,
        cx: &mut Context<Self>,
    ) -> bool {
        self.maintain_agent(cx);
        self.agent.as_ref().is_some_and(|state| {
            state.run_id == run_id
                && self.agent_target_current(state, cx)
                && state.pending.as_ref().is_some_and(|p| {
                    p.id == id
                        && &p.action == action
                        && !p.preparing
                        && !p.dispatched
                        && Instant::now() < p.deadline
                })
        })
    }
    pub(super) fn prepare_agent_file(
        &mut self,
        run_id: uuid::Uuid,
        id: uuid::Uuid,
        action: &AgentAction,
        cx: &mut Context<Self>,
    ) {
        if !self.admit_agent_action(run_id, id, action, cx) {
            self.reject_agent_action(run_id, id);
            self.assistant.update(cx, |panel, cx| {
                panel.agent_file_prepared(run_id, id, Err(()), cx)
            });
            return;
        }
        let AgentAction::WriteFile { path, .. } = action else {
            return;
        };
        let Some(state) = &mut self.agent else {
            return;
        };
        let Some(pending) = &mut state.pending else {
            return;
        };
        pending.preparing = true;
        let path = path.clone();
        let session = state.session.clone();
        let cancellation = state.cancellation.clone();
        let lifecycle = state.lifecycle.clone();
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                tokio::select! {
                    biased;
                    _ = cancellation.wait_cancelled() => Err(()),
                    _ = wait_lifecycle_loss(lifecycle) => {
                        cancellation.cancel();
                        Err(())
                    },
                    result = tokio::time::timeout(OPERATION_DEADLINE, read_snapshot(session, &path)) => result.unwrap_or(Err(())),
                }
            },
        );
        state.job = Some(cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(Err(()));
            let _ = this.update(cx, |view, cx| {
                view.maintain_agent(cx);
                let Some(state) = &mut view.agent else {
                    return;
                };
                if state.run_id != run_id {
                    return;
                }
                let Some(pending) = &mut state.pending else {
                    return;
                };
                if pending.id != id || !pending.preparing || pending.dispatched {
                    return;
                }
                pending.preparing = false;
                let original = result.and_then(|snapshot| {
                    let original = String::from_utf8(snapshot.content.clone()).map_err(|_| ())?;
                    pending.baseline = Some(snapshot);
                    Ok(original)
                });
                if original.is_err() {
                    state.pending = None;
                }
                view.assistant.update(cx, |panel, cx| {
                    panel.agent_file_prepared(run_id, id, original, cx)
                });
            });
        }));
    }
    pub(super) fn execute_agent_action(
        &mut self,
        run_id: uuid::Uuid,
        id: uuid::Uuid,
        action: &AgentAction,
        cx: &mut Context<Self>,
    ) {
        if !self.admit_agent_action(run_id, id, action, cx) {
            self.reject_agent_action(run_id, id);
            self.assistant.update(cx, |panel, cx| {
                panel.agent_action_finished(
                    run_id,
                    id,
                    AgentOutcome::Failed {
                        reason:
                            "Action review expired or captured authority is no longer available"
                                .into(),
                    },
                    cx,
                )
            });
            return;
        }
        let Some(state) = &mut self.agent else {
            return;
        };
        let Some(pending) = &mut state.pending else {
            return;
        };
        if matches!(action, AgentAction::WriteFile { .. }) && pending.baseline.is_none() {
            // A rejected dispatch must not leave authority attached to an old
            // proposal; a later reviewed round needs its own pending action.
            state.pending = None;
            self.assistant.update(cx, |panel, cx| {
                panel.agent_action_finished(
                    run_id,
                    id,
                    AgentOutcome::Failed {
                        reason: "A reviewed original file snapshot is required before replacement"
                            .into(),
                    },
                    cx,
                )
            });
            return;
        }
        // Consume on the foreground thread before starting any transport future.
        pending.dispatched = true;
        let session = state.session.clone();
        let cancellation = state.cancellation.clone();
        let lifecycle = state.lifecycle.clone();
        let action = pending.action.clone();
        let baseline = pending.baseline.take();
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                if cancellation.is_cancelled()
                    || session.is_closed()
                    || !lifecycle_current(lifecycle.as_ref())
                {
                    cancellation.cancel();
                    return AgentOutcome::Failed {
                        reason: "The captured session was unavailable before dispatch".into(),
                    };
                }
                tokio::select! {
                    biased;
                    _ = cancellation.wait_cancelled() => AgentOutcome::Unknown,
                    _ = wait_lifecycle_loss(lifecycle.clone()) => {
                        cancellation.cancel();
                        AgentOutcome::Unknown
                    },
                    result = tokio::time::timeout(OPERATION_DEADLINE, execute(session, action, baseline, &cancellation, lifecycle.as_ref())) => result.unwrap_or(AgentOutcome::Unknown),
                }
            },
        );
        state.job = Some(cx.spawn(async move |this, cx| {
            let result = job.await.unwrap_or(AgentOutcome::Unknown);
            let _ = this.update(cx, |view, cx| {
                view.maintain_agent(cx);
                let Some(state) = &mut view.agent else {
                    return;
                };
                if state.run_id != run_id
                    || !state
                        .pending
                        .as_ref()
                        .is_some_and(|p| p.id == id && p.dispatched)
                {
                    return;
                }
                state.pending = None;
                view.assistant.update(cx, |panel, cx| {
                    panel.agent_action_finished(run_id, id, result, cx)
                });
            });
        }));
    }
}

fn lifecycle_current(source: Option<&watch::Receiver<TransportState>>) -> bool {
    // The live SSH bridge owns a sticky typed end source. None is the legacy
    // controlled byte-only transport; its foreground observer still revokes.
    source.is_none_or(|source| {
        source.has_changed().is_ok() && matches!(*source.borrow(), TransportState::Ready)
    })
}

async fn wait_lifecycle_loss(source: Option<watch::Receiver<TransportState>>) {
    let Some(mut source) = source else {
        std::future::pending::<()>().await;
        return;
    };
    while lifecycle_current(Some(&source)) {
        if source.changed().await.is_err() {
            break;
        }
    }
}

async fn read_snapshot(session: SshSession, path: &str) -> Result<RegularFileSnapshot, ()> {
    let sftp = session.sftp().await.map_err(|_| ())?;
    let result = async {
        if sftp.canonicalize(path).await.map_err(|_| ())? != path {
            return Err(());
        }
        let snapshot = sftp
            .read_regular_snapshot(path, MAX_BYTES)
            .await
            .map_err(|_| ())?;
        std::str::from_utf8(&snapshot.content).map_err(|_| ())?;
        Ok(snapshot)
    }
    .await;
    let closed = sftp.close().await;
    if closed.is_err() {
        return Err(());
    }
    result
}
async fn execute(
    session: SshSession,
    action: AgentAction,
    baseline: Option<RegularFileSnapshot>,
    cancellation: &RequestCancellation,
    lifecycle: Option<&watch::Receiver<TransportState>>,
) -> AgentOutcome {
    match action {
        AgentAction::Command { command } => {
            match session.exec_limited(&command, MAX_BYTES).await {
                Ok(output) if !cancellation.is_cancelled() && lifecycle_current(lifecycle) => {
                    let Some(status) = output.exit_status else {
                        return AgentOutcome::Unknown;
                    };
                    let mut bytes = output.stdout;
                    bytes.extend(output.stderr);
                    match String::from_utf8(bytes) {
                    Ok(output) => AgentOutcome::Completed {
                        exit_status: Some(status),
                        output,
                    },
                    Err(_) => AgentOutcome::Completed { exit_status: Some(status), output: "The remote exit receipt was confirmed; non-UTF-8 output was omitted".into() },
                }
                }
                _ => AgentOutcome::Unknown,
            }
        }
        AgentAction::ReadFile { path } => match read_snapshot(session, &path).await {
            Ok(snapshot) if !cancellation.is_cancelled() && lifecycle_current(lifecycle) => {
                match String::from_utf8(snapshot.content) {
                    Ok(output) => AgentOutcome::Completed {
                        exit_status: None,
                        output,
                    },
                    Err(_) => AgentOutcome::Failed {
                        reason: "File is not complete bounded UTF-8".into(),
                    },
                }
            }
            _ => AgentOutcome::Failed {
                reason: "Regular UTF-8 file read was not confirmed".into(),
            },
        },
        AgentAction::WriteFile { replacement, .. } => {
            let Some(baseline) = baseline else {
                return AgentOutcome::Failed {
                    reason: "Reviewed original snapshot is missing".into(),
                };
            };
            let Ok(sftp) = session.sftp().await else {
                return AgentOutcome::Failed {
                    reason: "SFTP setup failed before writing".into(),
                };
            };
            let authorized = || {
                !cancellation.is_cancelled() && !session.is_closed() && lifecycle_current(lifecycle)
            };
            let result = if !authorized() {
                AgentOutcome::Failed {
                    reason: "Authority unavailable before writing".into(),
                }
            } else {
                match sftp
                    .write_regular_reviewed_authorized(
                        &baseline,
                        replacement.as_bytes(),
                        &authorized,
                    )
                    .await
                {
                    Ok(()) if authorized() => {
                        match sftp
                            .read_regular_snapshot(&baseline.entry.path, MAX_BYTES)
                            .await
                        {
                            Ok(current)
                                if current.content == replacement.as_bytes() && authorized() =>
                            {
                                AgentOutcome::Completed {
                                    exit_status: None,
                                    output: String::new(),
                                }
                            }
                            _ => AgentOutcome::Unknown,
                        }
                    }
                    Err(SessionError::MutationBusy | SessionError::MutationQuarantined) => {
                        AgentOutcome::Failed {
                            reason:
                                "A conflicting or unknown file mutation must be resolved separately"
                                    .into(),
                        }
                    }
                    _ => AgentOutcome::Unknown,
                }
            };
            if sftp.close().await.is_err() {
                AgentOutcome::Unknown
            } else {
                result
            }
        }
        AgentAction::Finish { .. } => AgentOutcome::Failed {
            reason: "A final model summary is not an executable operation".into(),
        },
    }
}
