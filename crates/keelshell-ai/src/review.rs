use std::{
    fmt,
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::AiError;

/// A suggested command with human-readable rationale and exact execution scope.
///
/// This is an editable draft. Risks are model/user statements, not a trusted shell
/// safety verdict. This crate never spawns processes or writes to a terminal.
#[derive(Clone, PartialEq, Eq)]
pub struct CommandProposal {
    /// Exact command text, without automatic trimming or shell rewriting.
    pub command: String,
    /// Why this command is suggested and which evidence supports it.
    pub explanation: String,
    /// Known effects or caveats, displayed for review.
    pub risks: Vec<String>,
    /// Visible destination such as a host label or local terminal name.
    pub target: String,
    /// Exact live session identifier, independent of the host display name.
    pub session_id: String,
}

impl fmt::Debug for CommandProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandProposal").finish_non_exhaustive()
    }
}

impl CommandProposal {
    /// Issue a ticket after user review, valid at most five minutes.
    ///
    /// Editing any field invalidates its match with the ticket. The resulting
    /// ticket only permits returning text for insertion into an editable field;
    /// it is not an authorization to execute a shell action.
    pub fn review(&self, lifetime: Duration) -> Result<ReviewTicket, AiError> {
        if self.command.trim().is_empty()
            || self.target.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.command.contains('\0')
            || self.target.contains('\0')
            || self.session_id.contains('\0')
            || lifetime.is_zero()
            || lifetime > Duration::from_secs(300)
        {
            return Err(AiError::InvalidProposal);
        }
        Ok(ReviewTicket {
            id: Uuid::new_v4(),
            proposal: self.clone(),
            created: Instant::now(),
            lifetime,
        })
    }
}

/// One-use proof of review for exact draft content and a particular live session.
pub struct ReviewTicket {
    id: Uuid,
    proposal: CommandProposal,
    created: Instant,
    lifetime: Duration,
}

impl fmt::Debug for ReviewTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewTicket")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl ReviewTicket {
    /// Non-secret identifier suitable for local review events.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Check exact proposal equality, active session and monotonic expiry.
    pub fn validate(
        &self,
        current: &CommandProposal,
        active_session_id: &str,
    ) -> Result<(), AiError> {
        if self.created.elapsed() >= self.lifetime {
            return Err(AiError::ReviewExpired);
        }
        if &self.proposal != current || current.session_id != active_session_id {
            return Err(AiError::ReviewMismatch);
        }
        Ok(())
    }

    /// Consume the ticket and return reviewed text for an editable input field.
    /// The caller still decides whether and when a terminal receives it.
    pub fn into_suggestion(
        self,
        current: &CommandProposal,
        active_session_id: &str,
    ) -> Result<String, AiError> {
        self.validate(current, active_session_id)?;
        Ok(self.proposal.command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal() -> CommandProposal {
        CommandProposal {
            command: "df -h".into(),
            explanation: "Inspect filesystem capacity".into(),
            risks: vec![],
            target: "development".into(),
            session_id: "session-1".into(),
        }
    }

    #[test]
    fn changed_command_invalidates_review() -> Result<(), AiError> {
        let mut proposal = proposal();
        let ticket = proposal.review(Duration::from_secs(60))?;
        proposal.command = "rm -rf data".into();
        assert_eq!(
            ticket.validate(&proposal, "session-1"),
            Err(AiError::ReviewMismatch)
        );
        Ok(())
    }

    #[test]
    fn changed_session_invalidates_review() -> Result<(), AiError> {
        let proposal = proposal();
        let ticket = proposal.review(Duration::from_secs(60))?;
        assert_eq!(
            ticket.validate(&proposal, "session-2"),
            Err(AiError::ReviewMismatch)
        );
        Ok(())
    }

    #[test]
    fn expiry_is_checked_without_wall_clock_assumptions() -> Result<(), AiError> {
        let proposal = proposal();
        let mut ticket = proposal.review(Duration::from_secs(60))?;
        ticket.created -= Duration::from_secs(61);
        assert_eq!(
            ticket.validate(&proposal, "session-1"),
            Err(AiError::ReviewExpired)
        );
        Ok(())
    }

    #[test]
    fn valid_review_returns_only_exact_suggestion() -> Result<(), AiError> {
        let proposal = proposal();
        let ticket = proposal.review(Duration::from_secs(60))?;
        assert_eq!(ticket.into_suggestion(&proposal, "session-1")?, "df -h");
        Ok(())
    }
}
