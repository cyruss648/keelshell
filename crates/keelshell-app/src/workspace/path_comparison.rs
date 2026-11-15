//! Two live transports and uninterrupted selection grant comparison authority.

use super::*;
use crate::monitor::path_comparison::Candidate;
use keelshell_session::path_comparison::PathComparisonPlan;

impl Workspace {
    pub(crate) fn path_comparison_candidates(
        &self,
        tab: EntityId,
        first: &SshSession,
    ) -> Vec<Candidate> {
        if self.protocol_selection_token(tab, first).is_none() || first.is_closed() {
            return Vec::new();
        }
        // Follow tab order rather than HashMap order, so refreshing a picker
        // cannot silently change the meaning of an existing row.
        self.tabs
            .iter()
            .map(Entity::entity_id)
            .filter(|id| *id != tab)
            .filter_map(|id| {
                let session = self.remote_sessions.get(&id)?;
                if session.is_closed()
                    || !self.protocol_session_current(id, session)
                    || PathComparisonPlan::new(first.clone(), session.clone()).is_err()
                {
                    return None;
                }
                Some(Candidate {
                    tab: id,
                    session: session.clone(),
                    label: self
                        .remote_hosts
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| "SSH".into()),
                })
            })
            .collect()
    }

    pub(crate) fn path_comparison_current(
        &self,
        tab: EntityId,
        first: &SshSession,
        candidate: &Candidate,
    ) -> Option<uuid::Uuid> {
        let selection = self.protocol_selection_token(tab, first)?;
        (!first.is_closed()
            && !candidate.session.is_closed()
            && self.protocol_session_current(candidate.tab, &candidate.session))
        .then_some(selection)
    }

    /// Called while Workspace is leased. Pass the decision into the child;
    /// asking that child to read Workspace here would create a reentrant lease.
    pub(super) fn revoke_stale_path_comparisons(&self, cx: &mut Context<Self>) {
        for monitor in self
            .panels
            .values()
            .filter_map(|panels| panels.monitor.as_ref())
        {
            let candidate = monitor.read(cx).path_comparison_candidate(cx);
            if candidate.is_some_and(|candidate| {
                candidate.session.is_closed()
                    || !self.protocol_session_current(candidate.tab, &candidate.session)
            }) {
                monitor.update(cx, |monitor, cx| {
                    monitor.invalidate_path_comparison_candidate(cx)
                });
            }
        }
    }
}
