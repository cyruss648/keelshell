use super::*;

impl Workspace {
    pub(in crate::workspace) fn review_mcp_action(
        &mut self,
        id: uuid::Uuid,
        approve: bool,
        cx: &mut Context<Self>,
    ) {
        if approve
            && self
                .mcp
                .actions
                .iter()
                .any(|action| matches!(action.state, ActionState::Running))
        {
            return;
        }
        let Some(index) = self
            .mcp
            .actions
            .iter()
            .position(|action| action.proposal.id == id)
        else {
            return;
        };
        let current = self.mcp_target_current(self.mcp.actions[index].entity, cx);
        let action = &mut self.mcp.actions[index];
        if !matches!(action.state, ActionState::PendingReview) {
            return;
        }
        if Instant::now() >= action.deadline {
            action.state = ActionState::Expired;
            cx.notify();
            return;
        }
        if action.lease.check().is_err() || !current {
            action.state = ActionState::Cancelled;
            cx.notify();
            return;
        }
        if !approve {
            action.state = ActionState::Rejected;
            cx.notify();
            return;
        }
        let Some(session) = self.remote_sessions.get(&action.entity).cloned() else {
            action.state = ActionState::Cancelled;
            cx.notify();
            return;
        };
        // Consume exactly once on the UI thread before any network execution.
        // External MCP methods can neither approve this path nor modify bytes.
        action.state = ActionState::Running;
        let command = action.proposal.command.clone();
        let lease = action.lease.clone();
        let sender = self.mcp.result_sender.clone();
        self.mcp.workers.push(self.runtime.spawn(async move {
            let result = tokio::select! {
                biased;
                _ = lease.revoked() => Err(McpFailure::Revoked),
                result = tokio::time::timeout(Duration::from_secs(30), session.exec_limited(&command, 64 * 1024)) =>
                    result.map_err(|_| McpFailure::Timeout).and_then(|result| result.map_err(|_| McpFailure::BackendFailure)),
            };
            let (state, output) = match result {
                Ok(output) if lease.check().is_ok() => {
                    let state = match output.exit_status { Some(0) => ActionState::Succeeded, Some(_) => ActionState::Failed, None => ActionState::OutcomeUnknown };
                    let output = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
                    (state, output.chars().filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t')).take(4096).collect())
                }
                Ok(_) | Err(_) => (ActionState::OutcomeUnknown, String::new()),
            };
            let _ = sender.send(Completion::Executed { id, state, output }).await;
        }));
        cx.notify();
    }
}

pub(super) use crate::command_text::visible_command;

#[cfg(test)]
mod tests {
    use super::visible_command;
    #[test]
    fn review_exposes_hidden_controls_without_changing_ordinary_multiline_text() {
        assert_eq!(
            visible_command("printf '中文'\nprintf 'two'"),
            "printf '中文'\nprintf 'two'"
        );
        assert_eq!(
            visible_command("echo\u{202e}x\u{1b}\t"),
            "echo\\u{202e}x\\u{001b}\\u{0009}"
        );
        assert_eq!(visible_command("e\u{061c}\u{2060}"), "e\\u{061c}\\u{2060}");
    }
}
