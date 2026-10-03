//! Traceable, review-only diagnostic plans extracted from an assistant reply.
//!
//! A plan keeps only bounded command blocks and digests of the selected context
//! and response. It never stores the original terminal text, sends a request, or
//! executes a command. Each step remains an editable command proposal and must
//! be reviewed again before it can enter the command bar.

use std::{
    fmt,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{AiError, CommandProposal};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 65_536;
const MAX_STEPS: usize = 8;
const MAX_COMMAND_BYTES: usize = 16 * 1024;
const MAX_TARGET_BYTES: usize = 512;

/// Conservative risk label for a command extracted from a fenced shell block.
///
/// `ReadOnly` means that the command uses a small allow-list of inspection
/// programs and contains no shell composition or redirection. It is still
/// reviewable; the label is never an execution authorization. All other text is
/// `ReviewRequired` because an assistant response is untrusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticRisk {
    /// A bounded, allow-listed inspection command without shell operators.
    ReadOnly,
    /// The command needs normal human review before it can be inserted.
    ReviewRequired,
}

/// One command and its source location in the assistant response.
#[derive(Clone, PartialEq, Eq)]
pub struct DiagnosticStep {
    index: usize,
    source_line: usize,
    command: String,
    risk: DiagnosticRisk,
}

impl fmt::Debug for DiagnosticStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticStep")
            .field("index", &self.index)
            .field("source_line", &self.source_line)
            .field("bytes", &self.command.len())
            .field("risk", &self.risk)
            .finish_non_exhaustive()
    }
}

impl DiagnosticStep {
    /// One-based position in the plan.
    pub fn index(&self) -> usize {
        self.index
    }

    /// One-based line where the fenced block starts in the model response.
    pub fn source_line(&self) -> usize {
        self.source_line
    }

    /// Exact command bytes, without shell rewriting.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Conservative risk label shown beside the review action.
    pub fn risk(&self) -> DiagnosticRisk {
        self.risk
    }

    /// Turn this step into an editable proposal bound to the plan target.
    pub fn proposal(&self, plan: &DiagnosticPlan) -> CommandProposal {
        let explanation = format!(
            "Diagnostic plan step {} from response line {} (context {}, response {})",
            self.index,
            self.source_line,
            plan.context_fingerprint(),
            plan.response_fingerprint(),
        );
        let risk = match self.risk {
            DiagnosticRisk::ReadOnly => "allow-listed inspection command; review output and target",
            DiagnosticRisk::ReviewRequired => {
                "untrusted model command; inspect every argument and effect"
            }
        };
        CommandProposal {
            command: self.command.clone(),
            explanation,
            risks: vec![risk.to_owned()],
            target: plan.target.clone(),
            session_id: plan.session_id.clone(),
        }
    }
}

/// A bounded diagnostic plan tied to one host session and one assistant reply.
///
/// The two fingerprints let the UI show what evidence a plan came from without
/// retaining raw logs in the plan. A plan has no network or process authority.
#[derive(Clone, PartialEq, Eq)]
pub struct DiagnosticPlan {
    id: Uuid,
    target: String,
    session_id: String,
    context_digest: [u8; 32],
    response_digest: [u8; 32],
    steps: Vec<DiagnosticStep>,
}

impl fmt::Debug for DiagnosticPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticPlan")
            .field("id", &self.id)
            .field("steps", &self.steps.len())
            .field("target_bytes", &self.target.len())
            .finish_non_exhaustive()
    }
}

impl DiagnosticPlan {
    /// Extract closed shell blocks from a response after the user explicitly
    /// asks for a plan. The response and selected context are never retained.
    pub fn from_response(
        target: impl Into<String>,
        session_id: impl Into<String>,
        selected_context: &str,
        response: &str,
    ) -> Result<Self, AiError> {
        let target = target.into();
        let session_id = session_id.into();
        validate_identity(&target, &session_id)?;
        if selected_context.len() > MAX_CONTEXT_BYTES || response.len() > MAX_RESPONSE_BYTES {
            return Err(AiError::DiagnosticPlanTooLarge);
        }
        let steps = extract_steps(response)?;
        if steps.is_empty() {
            return Err(AiError::NoDiagnosticSteps);
        }
        Ok(Self {
            id: Uuid::new_v4(),
            target,
            session_id,
            context_digest: digest(selected_context.as_bytes()),
            response_digest: digest(response.as_bytes()),
            steps,
        })
    }

    /// Stable local identifier for this plan; it carries no source text.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Human-readable target label captured when the plan was made.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Exact live session identity captured when the plan was made.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Steps in response order, each requiring separate command review.
    pub fn steps(&self) -> &[DiagnosticStep] {
        &self.steps
    }

    /// Lowercase hexadecimal digest of the explicitly selected context.
    pub fn context_fingerprint(&self) -> String {
        hex(&self.context_digest)
    }

    /// Lowercase hexadecimal digest of the model response.
    pub fn response_fingerprint(&self) -> String {
        hex(&self.response_digest)
    }

    /// Issue a short-lived ticket for one exact step and target session.
    pub fn review_step(
        &self,
        index: usize,
        lifetime: Duration,
    ) -> Result<DiagnosticReview, AiError> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(300) {
            return Err(AiError::InvalidDiagnosticPlan);
        }
        if index >= self.steps.len() {
            return Err(AiError::InvalidDiagnosticPlan);
        }
        Ok(DiagnosticReview {
            plan_id: self.id,
            step_index: index,
            target: self.target.clone(),
            session_id: self.session_id.clone(),
            context_digest: self.context_digest,
            response_digest: self.response_digest,
            created: Instant::now(),
            lifetime,
        })
    }
}

/// One-use review proof for a single plan step.
pub struct DiagnosticReview {
    plan_id: Uuid,
    step_index: usize,
    target: String,
    session_id: String,
    context_digest: [u8; 32],
    response_digest: [u8; 32],
    created: Instant,
    lifetime: Duration,
}

impl fmt::Debug for DiagnosticReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticReview")
            .field("plan_id", &self.plan_id)
            .field("step_index", &self.step_index)
            .finish_non_exhaustive()
    }
}

impl DiagnosticReview {
    /// Non-secret plan identifier suitable for local UI events.
    pub fn plan_id(&self) -> Uuid {
        self.plan_id
    }

    /// Consume the ticket only when the same plan and live session are present.
    /// This returns an editable proposal; it never executes or sends it.
    pub fn into_proposal(
        self,
        plan: &DiagnosticPlan,
        active_session_id: &str,
    ) -> Result<CommandProposal, AiError> {
        if self.created.elapsed() >= self.lifetime {
            return Err(AiError::ReviewExpired);
        }
        if plan.id != self.plan_id
            || plan.context_digest != self.context_digest
            || plan.response_digest != self.response_digest
            || plan.target != self.target
            || plan.session_id != self.session_id
            || active_session_id != self.session_id
        {
            return Err(AiError::DiagnosticPlanMismatch);
        }
        let step = plan
            .steps
            .get(self.step_index)
            .ok_or(AiError::InvalidDiagnosticPlan)?;
        Ok(step.proposal(plan))
    }
}

fn validate_identity(target: &str, session_id: &str) -> Result<(), AiError> {
    if target.is_empty()
        || session_id.is_empty()
        || target.len() > MAX_TARGET_BYTES
        || session_id.len() > MAX_TARGET_BYTES
        || target.chars().any(char::is_control)
        || session_id.chars().any(char::is_control)
    {
        Err(AiError::InvalidDiagnosticPlan)
    } else {
        Ok(())
    }
}

fn extract_steps(response: &str) -> Result<Vec<DiagnosticStep>, AiError> {
    let mut steps = Vec::new();
    let mut collecting = false;
    let mut source_line = 0;
    let mut current = String::new();
    for (line_index, line) in response.lines().enumerate() {
        let line_number = line_index + 1;
        if let Some(language) = line.trim().strip_prefix("```") {
            if collecting {
                let command = current.trim_end().to_owned();
                if !command.is_empty() {
                    steps.push(step(command, source_line)?);
                    if steps.len() > MAX_STEPS {
                        return Err(AiError::DiagnosticPlanTooLarge);
                    }
                }
                collecting = false;
                current.clear();
            } else if shell_language(language.trim()) {
                collecting = true;
                source_line = line_number;
            }
            continue;
        }
        if collecting {
            current.push_str(line);
            current.push('\n');
            if current.len() > MAX_COMMAND_BYTES {
                return Err(AiError::DiagnosticPlanTooLarge);
            }
        }
    }
    // An unclosed fence is deliberately ignored; no incomplete command becomes
    // a plan step.
    for (index, step) in steps.iter_mut().enumerate() {
        step.index = index;
    }
    Ok(steps)
}

fn step(command: String, source_line: usize) -> Result<DiagnosticStep, AiError> {
    if command
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(AiError::InvalidDiagnosticPlan);
    }
    Ok(DiagnosticStep {
        index: 0, // fixed after extraction, keeping the parser's checks local
        source_line,
        risk: classify(&command),
        command,
    })
}

fn shell_language(language: &str) -> bool {
    matches!(
        language,
        "sh" | "bash" | "zsh" | "shell" | "powershell" | "ps1" | "cmd"
    )
}

fn classify(command: &str) -> DiagnosticRisk {
    let first = command.lines().next().unwrap_or_default().trim();
    let token = first.split_whitespace().next().unwrap_or_default();
    let basename = token.rsplit('/').next().unwrap_or(token);
    let read_only = matches!(
        basename,
        "cat"
            | "df"
            | "du"
            | "echo"
            | "free"
            | "grep"
            | "head"
            | "id"
            | "ip"
            | "journalctl"
            | "ls"
            | "lsof"
            | "netstat"
            | "ps"
            | "ss"
            | "stat"
            | "tail"
            | "uname"
            | "uptime"
            | "vmstat"
            | "who"
            | "whoami"
    );
    let shell_operator = [";", "&&", "||", "|", ">", "<", "$(", "`", "\n"]
        .iter()
        .any(|operator| command.contains(operator));
    if read_only && !shell_operator && !first.contains(" -i") {
        DiagnosticRisk::ReadOnly
    } else {
        DiagnosticRisk::ReviewRequired
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_closed_shell_steps_with_risk_and_fingerprints() -> Result<(), AiError> {
        let plan = DiagnosticPlan::from_response(
            "ops@example:22",
            "session-a",
            "load average 8",
            "text\n```sh\nuptime\n```\n```bash\nrm -rf /tmp/x\n```\n```json\n{}\n```",
        )?;
        assert_eq!(plan.steps().len(), 2);
        assert_eq!(plan.steps()[0].index(), 0);
        assert_eq!(plan.steps()[0].source_line(), 2);
        assert_eq!(plan.steps()[0].risk(), DiagnosticRisk::ReadOnly);
        assert_eq!(plan.steps()[1].risk(), DiagnosticRisk::ReviewRequired);
        assert_eq!(plan.context_fingerprint().len(), 64);
        assert_eq!(plan.response_fingerprint().len(), 64);
        Ok(())
    }

    #[test]
    fn unclosed_or_non_shell_blocks_do_not_become_steps() {
        assert_eq!(
            error(DiagnosticPlan::from_response(
                "host",
                "session",
                "ctx",
                "```sh\nuptime"
            )),
            AiError::NoDiagnosticSteps
        );
        assert_eq!(
            error(DiagnosticPlan::from_response(
                "host",
                "session",
                "ctx",
                "```json\n{}\n```"
            )),
            AiError::NoDiagnosticSteps
        );
    }

    #[test]
    fn review_binds_plan_and_active_session() -> Result<(), AiError> {
        let plan = DiagnosticPlan::from_response("host", "session", "ctx", "```sh\nss -ltn\n```")?;
        let review = plan.review_step(0, Duration::from_secs(60))?;
        let proposal = review.into_proposal(&plan, "session")?;
        assert_eq!(proposal.command, "ss -ltn");
        let review = plan.review_step(0, Duration::from_secs(60))?;
        assert_eq!(
            review.into_proposal(&plan, "other"),
            Err(AiError::DiagnosticPlanMismatch)
        );
        Ok(())
    }

    #[test]
    fn malformed_identity_and_oversized_step_are_rejected() {
        assert_eq!(
            error(DiagnosticPlan::from_response(
                "",
                "session",
                "ctx",
                "```sh\nuptime\n```"
            )),
            AiError::InvalidDiagnosticPlan
        );
        let command = "x".repeat(MAX_COMMAND_BYTES);
        assert_eq!(
            error(DiagnosticPlan::from_response(
                "host",
                "session",
                "ctx",
                &format!("```sh\n{command}\n```")
            )),
            AiError::DiagnosticPlanTooLarge
        );
    }

    fn error(result: Result<DiagnosticPlan, AiError>) -> AiError {
        match result {
            Ok(_) => panic!("expected diagnostic plan error"),
            Err(error) => error,
        }
    }
}
