use super::*;

fn clearable(state: ActionState) -> bool {
    matches!(
        state,
        ActionState::Rejected
            | ActionState::Expired
            | ActionState::Cancelled
            | ActionState::Succeeded
            | ActionState::Failed
    )
}

impl McpState {
    pub(super) fn has_clearable_actions(&self) -> bool {
        self.actions.iter().any(|action| clearable(action.state))
    }

    pub(super) fn clear_finished_actions(&mut self) {
        // Unknown records remain available even after explicit acknowledgement.
        // Bulk history cleanup cannot silently release the window-close hold.
        self.actions.retain(|action| !clearable(action.state));
    }
}

impl Workspace {
    pub(in crate::workspace) fn open_mcp_unknown_review(
        &mut self,
        id: uuid::Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.mcp.actions.iter().any(|action| {
            action.proposal.id() == id
                && matches!(action.state, ActionState::OutcomeUnknown)
                && !action.unknown_acknowledged
        }) {
            self.mcp.reviewing = None;
            self.mcp.unknown_reviewing = Some(id);
            cx.notify();
        }
    }

    pub(in crate::workspace) fn acknowledge_mcp_unknown(
        &mut self,
        id: uuid::Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.mcp.unknown_reviewing != Some(id) {
            return;
        }
        if let Some(action) = self.mcp.actions.iter_mut().find(|action| {
            action.proposal.id() == id && matches!(action.state, ActionState::OutcomeUnknown)
        }) {
            action.unknown_acknowledged = true;
        }
        self.mcp.unknown_reviewing = None;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionState, clearable};

    #[test]
    fn unknown_pending_and_running_are_never_bulk_clearable() {
        for state in [
            ActionState::PendingReview,
            ActionState::Running,
            ActionState::OutcomeUnknown,
        ] {
            assert!(!clearable(state), "{state:?}");
        }
        for state in [
            ActionState::Rejected,
            ActionState::Expired,
            ActionState::Cancelled,
            ActionState::Succeeded,
            ActionState::Failed,
        ] {
            assert!(clearable(state), "{state:?}");
        }
    }
}
