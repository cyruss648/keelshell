use super::*;

impl Workspace {
    pub(super) fn admit_mcp_request(&mut self, queued: QueuedRequest, cx: &mut Context<Self>) {
        let QueuedRequest { request, reply } = queued;
        if reply.is_closed() {
            return;
        }
        if let Err(error) = request.authorization.check() {
            let _ = reply.send(Err(error));
            return;
        }
        let target = request.operation.identity().and_then(|identity| {
            self.mcp
                .targets
                .iter()
                .find(|target| target.identity == identity)
        });
        if request.operation.identity().is_some()
            && target.is_none_or(|target| !self.mcp_target_current(target.entity, cx))
        {
            let _ = reply.send(Err(McpFailure::StaleSession));
            return;
        }
        let result = match &request.operation {
            Operation::ListSessions => Ok(BackendReply::Sessions {
                sessions: self
                    .mcp
                    .targets
                    .iter()
                    .filter(|target| target.tools.contains(&ToolKind::ListSessions))
                    .map(|target| SessionMetadata {
                        target: target.identity,
                        display_name: target.label.clone(),
                        selection_ids: target.selection.iter().map(|(id, _)| *id).collect(),
                        granted_roots: target.roots.clone(),
                    })
                    .collect(),
            }),
            Operation::ReadSelection {
                target: identity,
                selection_id,
            } => target
                .and_then(|target| target.selection.as_ref())
                .filter(|(id, _)| id == selection_id)
                .map(|(_, text)| BackendReply::Selection {
                    target: *identity,
                    selection_id: *selection_id,
                    text: text.clone(),
                })
                .ok_or(McpFailure::Forbidden),
            Operation::MonitorSnapshot { target: identity } => target
                .and_then(|target| self.panels.get(&target.entity))
                .and_then(|panels| panels.monitor.as_ref())
                .and_then(|panel| panel.read(cx).mcp_snapshot())
                .map(|snapshot| BackendReply::Monitor {
                    target: *identity,
                    snapshot,
                })
                .ok_or(McpFailure::NotConnected),
            Operation::GetActionStatus { target, action_id } => self
                .mcp
                .actions
                .iter()
                .find(|action| {
                    action.proposal.id() == *action_id
                        && action.proposal.target() == *target
                        && action.lease.check().is_ok()
                })
                .map(|action| BackendReply::ActionStatus {
                    target: *target,
                    action_id: *action_id,
                    state: action.state,
                    action_kind: action.proposal.kind(),
                })
                .ok_or(McpFailure::Forbidden),
            Operation::ProposeCommand { .. } => {
                if self.mcp.actions.len() >= 32 {
                    Err(McpFailure::Busy)
                } else if let (Some(proposal), Some(target)) = (request.proposal, target) {
                    let response = BackendReply::PendingCommand {
                        target: proposal.target,
                        action_id: proposal.id,
                        digest: proposal.digest.clone(),
                    };
                    let route = self
                        .batch_route_description(target.entity)
                        .map(|(_, route)| route)
                        .or_else(|| self.remote_hosts.get(&target.entity).cloned())
                        .unwrap_or_default();
                    self.mcp.actions.push(Action {
                        deadline: Instant::now()
                            + Duration::from_secs(u64::from(
                                proposal.expires_after_seconds.min(300),
                            )),
                        proposal: ReviewedProposal::Command(proposal),
                        lease: request.authorization,
                        entity: target.entity,
                        label: target.label.clone(),
                        route,
                        state: ActionState::PendingReview,
                        output: String::new(),
                        session: target.session.clone(),
                    });
                    cx.notify();
                    Ok(response)
                } else {
                    Err(McpFailure::BackendFailure)
                }
            }
            Operation::ProposeFileChange { .. } => {
                if self.mcp.actions.len() + self.mcp.preparing_files >= 32 {
                    Err(McpFailure::Busy)
                } else if let (Some(proposal), Some(target)) = (request.file_proposal, target) {
                    let Some(session) = self.remote_sessions.get(&target.entity).cloned() else {
                        let _ = reply.send(Err(McpFailure::StaleSession));
                        return;
                    };
                    let entity = target.entity;
                    let label = target.label.clone();
                    let route = self
                        .batch_route_description(entity)
                        .map(|(_, route)| route)
                        .or_else(|| self.remote_hosts.get(&entity).cloned())
                        .unwrap_or_default();
                    let revision = self.mcp.revision;
                    let captured_session = session.clone();
                    let lease = request.authorization;
                    let sender = self.mcp.result_sender.clone();
                    let preparation_id = self.mcp.reserve_file_preparation();
                    self.mcp.workers.push(self.runtime.spawn(async move {
                        let mut reply = reply;
                        let result = tokio::select! {
                            biased;
                            _ = reply.closed() => Err(McpFailure::Cancelled),
                            _ = lease.revoked() => Err(McpFailure::Revoked),
                            result = crate::mcp_bridge::prepare_file_change(session, &proposal, &lease) => result,
                        };
                        let _ = sender.send(Completion::FilePrepared { revision, preparation_id, entity, label, route, proposal: Box::new(proposal), lease, session: captured_session, reply, result }).await;
                    }));
                    return;
                } else {
                    Err(McpFailure::BackendFailure)
                }
            }
            Operation::SftpList { .. } | Operation::SftpRead { .. } => {
                let Some(session) = target
                    .and_then(|target| self.remote_sessions.get(&target.entity))
                    .cloned()
                else {
                    let _ = reply.send(Err(McpFailure::StaleSession));
                    return;
                };
                let operation = request.operation;
                let lease = request.authorization;
                self.mcp.workers.push(self.runtime.spawn(async move {
                    let mut reply = reply;
                    let result = tokio::select! {
                        biased;
                        _ = reply.closed() => return,
                        _ = lease.revoked() => Err(McpFailure::Revoked),
                        result = crate::mcp_bridge::read_remote(session, operation, &lease) => result,
                    };
                    let _ = reply.send(result);
                }));
                return;
            }
        };
        let _ = reply.send(result);
    }
}
