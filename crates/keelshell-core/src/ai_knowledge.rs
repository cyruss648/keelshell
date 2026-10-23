//! Explicitly selected local knowledge and runbook domain primitives.
//!
//! This module is deliberately a pure in-memory boundary. It accepts only
//! [`SelectedFact`] values constructed by an explicit caller action, creates
//! source-bound summaries and timelines, and renders reviewable Markdown. It
//! never reads a task audit, contacts a provider, executes a command, or writes
//! a file. Unknown outcomes and sensitive content remain visible only as
//! bounded, non-content exclusion markers and make the result non-persistable.

use std::{fmt, num::NonZeroU64};

use serde::Serialize;
use uuid::Uuid;

/// Maximum number of explicitly selected facts in one knowledge build.
pub const MAX_KNOWLEDGE_FACTS: usize = 64;
/// Maximum UTF-8 bytes in one knowledge title or summary.
pub const MAX_KNOWLEDGE_TEXT_BYTES: usize = 16 * 1024;
/// Maximum UTF-8 bytes in source metadata.
pub const MAX_KNOWLEDGE_SOURCE_LABEL_BYTES: usize = 256;
/// Maximum combined input bytes in one selected knowledge set.
pub const MAX_KNOWLEDGE_TOTAL_BYTES: usize = 128 * 1024;
/// Maximum runbook steps produced from one knowledge document.
pub const MAX_RUNBOOK_STEPS: usize = 64;
/// Maximum Markdown bytes accepted by rendering and local search.
pub const MAX_KNOWLEDGE_MARKDOWN_BYTES: usize = 256 * 1024;
/// Maximum UTF-8 bytes in a local search query.
pub const MAX_KNOWLEDGE_SEARCH_QUERY_BYTES: usize = 512;
/// Maximum search results returned for one query.
pub const MAX_KNOWLEDGE_SEARCH_MATCHES: usize = 256;
/// Maximum UTF-8 bytes in a search result snippet.
pub const MAX_KNOWLEDGE_SEARCH_SNIPPET_BYTES: usize = 256;

/// Safe, opaque source categories that can bind a knowledge entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeSourceKind {
    /// A user-selected terminal or SSH session snapshot.
    Session,
    /// A reviewed workflow or runbook-related source record.
    Workflow,
    /// A user-selected local or remote file context.
    File,
    /// A user-selected monitoring observation.
    Monitor,
    /// A fact entered directly by the user.
    Manual,
}

/// Stable identity and revision for the source of a knowledge fact.
///
/// The revision prevents a later source change from silently making an old
/// summary appear current. The label is display metadata only and is validated
/// before this binding can be constructed.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    kind: KnowledgeSourceKind,
    id: Uuid,
    revision: NonZeroU64,
    label: String,
}

impl fmt::Debug for SourceBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceBinding")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("label", &self.label)
            .finish()
    }
}

impl SourceBinding {
    /// Construct a source binding without reading or resolving the source.
    pub fn new(
        kind: KnowledgeSourceKind,
        id: Uuid,
        revision: u64,
        label: &str,
    ) -> Result<Self, KnowledgeError> {
        if id.is_nil() || revision == 0 {
            return Err(KnowledgeError::InvalidSource);
        }
        let label = checked_single_line(label, MAX_KNOWLEDGE_SOURCE_LABEL_BYTES)?;
        if contains_sensitive_marker(&label) {
            return Err(KnowledgeError::SensitiveSourceMetadata);
        }
        Ok(Self {
            kind,
            id,
            revision: NonZeroU64::new(revision).ok_or(KnowledgeError::InvalidSource)?,
            label,
        })
    }

    /// Return the source category.
    pub const fn kind(&self) -> KnowledgeSourceKind {
        self.kind
    }

    /// Return the opaque source identity.
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Return the source revision bound to the fact.
    pub const fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Return display-only source metadata.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Outcome selected by the user for one source fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFactOutcome {
    /// The fact completed successfully and has a user-selected summary.
    Completed(String),
    /// The fact completed with a known failure and has a user-selected summary.
    Failed(String),
    /// The result is unknown and has no content that may become knowledge.
    Unknown,
    /// The work did not start.
    NotStarted,
    /// The work was cancelled before a known result.
    Cancelled,
}

/// An explicitly user-selected fact candidate.
///
/// This type has no conversion from task audit records by design. Callers must
/// construct each fact through one of the named constructors after presenting
/// it for selection. The raw summary stays in memory only until the pure build
/// service either accepts it or turns it into a non-content exclusion marker.
#[derive(Clone, PartialEq, Eq)]
pub struct SelectedFact {
    source: SourceBinding,
    occurred_at: u64,
    title: String,
    outcome: SelectedFactOutcome,
}

impl fmt::Debug for SelectedFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedFact")
            .field("source", &self.source)
            .field("occurred_at", &self.occurred_at)
            .field("title", &self.title)
            .field("outcome", &self.outcome.redacted_debug_name())
            .finish()
    }
}

impl SelectedFact {
    /// Select a completed fact after the user reviews its source and summary.
    pub fn completed(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
        summary: &str,
    ) -> Result<Self, KnowledgeError> {
        Self::new(
            source,
            occurred_at,
            title,
            SelectedFactOutcome::Completed(summary.into()),
        )
    }

    /// Select a known failed fact after the user reviews its source and summary.
    pub fn failed(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
        summary: &str,
    ) -> Result<Self, KnowledgeError> {
        Self::new(
            source,
            occurred_at,
            title,
            SelectedFactOutcome::Failed(summary.into()),
        )
    }

    /// Select an unknown result. The unknown result carries no source content.
    pub fn unknown(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
    ) -> Result<Self, KnowledgeError> {
        Self::new(source, occurred_at, title, SelectedFactOutcome::Unknown)
    }

    /// Select work that never started; it cannot become saved knowledge.
    pub fn not_started(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
    ) -> Result<Self, KnowledgeError> {
        Self::new(source, occurred_at, title, SelectedFactOutcome::NotStarted)
    }

    /// Select cancelled work; it cannot become saved knowledge.
    pub fn cancelled(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
    ) -> Result<Self, KnowledgeError> {
        Self::new(source, occurred_at, title, SelectedFactOutcome::Cancelled)
    }

    fn new(
        source: SourceBinding,
        occurred_at: u64,
        title: &str,
        outcome: SelectedFactOutcome,
    ) -> Result<Self, KnowledgeError> {
        if occurred_at == 0 {
            return Err(KnowledgeError::InvalidTimestamp);
        }
        let title = checked_single_line(title, MAX_KNOWLEDGE_TEXT_BYTES)?;
        if title.is_empty() {
            return Err(KnowledgeError::EmptyText);
        }
        if let SelectedFactOutcome::Completed(summary) | SelectedFactOutcome::Failed(summary) =
            &outcome
        {
            checked_text(summary, MAX_KNOWLEDGE_TEXT_BYTES)?;
        }
        Ok(Self {
            source,
            occurred_at,
            title,
            outcome,
        })
    }

    /// Return the bound source.
    pub fn source(&self) -> &SourceBinding {
        &self.source
    }

    /// Return the Unix timestamp selected for the timeline.
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    /// Return the user-selected title.
    pub fn title(&self) -> &str {
        &self.title
    }
}

impl SelectedFactOutcome {
    fn redacted_debug_name(&self) -> &'static str {
        match self {
            Self::Completed(_) => "completed(summary omitted)",
            Self::Failed(_) => "failed(summary omitted)",
            Self::Unknown => "unknown",
            Self::NotStarted => "not_started",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Explicitly selected facts admitted to one pure knowledge build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeSelection {
    facts: Vec<SelectedFact>,
}

impl KnowledgeSelection {
    /// Admit a bounded set of facts selected by the user.
    pub fn new(facts: Vec<SelectedFact>) -> Result<Self, KnowledgeError> {
        if facts.is_empty() {
            return Err(KnowledgeError::EmptySelection);
        }
        if facts.len() > MAX_KNOWLEDGE_FACTS {
            return Err(KnowledgeError::TooManyFacts);
        }
        let total = facts.iter().try_fold(0usize, |total, fact| {
            total
                .checked_add(fact.title.len())
                .and_then(|value| match &fact.outcome {
                    SelectedFactOutcome::Completed(summary)
                    | SelectedFactOutcome::Failed(summary) => value.checked_add(summary.len()),
                    _ => Some(value),
                })
                .ok_or(KnowledgeError::InputTooLarge)
        })?;
        if total > MAX_KNOWLEDGE_TOTAL_BYTES {
            return Err(KnowledgeError::InputTooLarge);
        }
        Ok(Self { facts })
    }

    /// Borrow the exact explicit selection.
    pub fn facts(&self) -> &[SelectedFact] {
        &self.facts
    }
}

/// Why a selected fact is excluded from saved knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeExclusionReason {
    /// The source reports no known result.
    UnknownOutcome,
    /// The selected work did not reach a completed known outcome.
    NotCompleted,
    /// The title or summary contains a sensitive marker.
    SensitiveContent,
}

/// Reason a generated artifact cannot be persisted without a later review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeSaveBlock {
    /// A selected item contained a credential or other sensitive marker.
    SensitiveContent,
    /// A selected item did not have a known completed result.
    UnknownOrIncompleteOutcome,
}

/// Persistence status attached to every generated knowledge artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum SaveDisposition {
    /// All included content passed the bounded local safety checks.
    Persistable,
    /// The artifact is for review only and must not be saved or sent.
    DoNotPersist {
        /// Stable reason without echoing source content.
        reason: KnowledgeSaveBlock,
    },
}

impl SaveDisposition {
    /// Whether the artifact may be handed to a later explicit save flow.
    pub const fn is_persistable(self) -> bool {
        matches!(self, Self::Persistable)
    }
}

/// Outcome represented by a persistable knowledge summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeOutcome {
    /// The selected fact completed successfully.
    Completed,
    /// The selected fact completed with a known failure.
    Failed,
}

/// A source-bound, desensitized summary that passed the content gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeEntry {
    source: SourceBinding,
    occurred_at: u64,
    title: String,
    summary: String,
    outcome: KnowledgeOutcome,
}

impl KnowledgeEntry {
    /// Return the source binding.
    pub fn source(&self) -> &SourceBinding {
        &self.source
    }

    /// Return the timeline timestamp.
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    /// Return the bounded title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the normalized, bounded summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Return the known outcome.
    pub const fn outcome(&self) -> KnowledgeOutcome {
        self.outcome
    }
}

/// A timeline marker for an included fact or a deliberately excluded fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineEvent {
    source: SourceBinding,
    occurred_at: u64,
    title: Option<String>,
    status: TimelineStatus,
    disposition: SaveDisposition,
}

impl TimelineEvent {
    /// Return the source binding.
    pub fn source(&self) -> &SourceBinding {
        &self.source
    }

    /// Return the timestamp.
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    /// Return the title when it passed the sensitive-content gate.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Return the bounded timeline status.
    pub const fn status(&self) -> TimelineStatus {
        self.status
    }

    /// Return the explicit save status of this event.
    pub const fn disposition(&self) -> SaveDisposition {
        self.disposition
    }
}

/// Status shown in a bounded timeline without retaining excluded content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineStatus {
    /// A successful fact is included.
    Completed,
    /// A known failed fact is included.
    Failed,
    /// The result was unknown and has no saved content.
    Unknown,
    /// Work did not start and has no saved content.
    NotStarted,
    /// Work was cancelled and has no saved content.
    Cancelled,
    /// Content was withheld because a sensitive marker was detected.
    SensitiveContent,
}

/// A selected item excluded from knowledge content, with a safe source marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeExclusion {
    source: SourceBinding,
    occurred_at: u64,
    reason: KnowledgeExclusionReason,
}

impl KnowledgeExclusion {
    /// Return the source binding without exposing excluded text.
    pub fn source(&self) -> &SourceBinding {
        &self.source
    }

    /// Return the selected timestamp.
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    /// Return the stable exclusion reason.
    pub const fn reason(&self) -> KnowledgeExclusionReason {
        self.reason
    }

    /// Excluded content is always forbidden from persistence.
    pub const fn disposition(&self) -> SaveDisposition {
        SaveDisposition::DoNotPersist {
            reason: match self.reason {
                KnowledgeExclusionReason::SensitiveContent => KnowledgeSaveBlock::SensitiveContent,
                KnowledgeExclusionReason::UnknownOutcome
                | KnowledgeExclusionReason::NotCompleted => {
                    KnowledgeSaveBlock::UnknownOrIncompleteOutcome
                }
            },
        }
    }
}

/// In-memory knowledge summary and timeline produced from one explicit selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeDocument {
    entries: Vec<KnowledgeEntry>,
    timeline: Vec<TimelineEvent>,
    exclusions: Vec<KnowledgeExclusion>,
    disposition: SaveDisposition,
}

impl KnowledgeDocument {
    /// Return persistable source-bound summaries in selection order.
    pub fn entries(&self) -> &[KnowledgeEntry] {
        &self.entries
    }

    /// Return all bounded timeline markers in timestamp order.
    pub fn timeline(&self) -> &[TimelineEvent] {
        &self.timeline
    }

    /// Return excluded source markers; no excluded title or body is retained.
    pub fn exclusions(&self) -> &[KnowledgeExclusion] {
        &self.exclusions
    }

    /// Return whether the complete document may be persisted.
    pub const fn disposition(&self) -> SaveDisposition {
        self.disposition
    }
}

/// A descriptive runbook step bound to the source fact that supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookStep {
    ordinal: usize,
    source: SourceBinding,
    title: String,
    action: String,
    expected_result: String,
}

impl RunbookStep {
    /// Return the one-based display order.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Return the source binding.
    pub fn source(&self) -> &SourceBinding {
        &self.source
    }

    /// Return the step title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the descriptive action text. It is never executed by this module.
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Return the descriptive expected result.
    pub fn expected_result(&self) -> &str {
        &self.expected_result
    }
}

/// A source-bound, descriptive runbook generated from safe knowledge entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Runbook {
    title: String,
    steps: Vec<RunbookStep>,
    disposition: SaveDisposition,
}

impl Runbook {
    /// Return the bounded runbook title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return descriptive steps in deterministic order.
    pub fn steps(&self) -> &[RunbookStep] {
        &self.steps
    }

    /// Return the explicit save status inherited from the source document.
    pub const fn disposition(&self) -> SaveDisposition {
        self.disposition
    }
}

/// A generated Markdown string together with its save gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownArtifact {
    text: String,
    disposition: SaveDisposition,
}

impl MarkdownArtifact {
    /// Return the generated Markdown without performing I/O.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return the save status that a future UI must review.
    pub const fn disposition(&self) -> SaveDisposition {
        self.disposition
    }
}

/// One bounded result from local string search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    line: usize,
    column: usize,
    snippet: String,
}

impl SearchMatch {
    /// Return the one-based line number.
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Return the one-based Unicode scalar column.
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Return the bounded matching line snippet.
    pub fn snippet(&self) -> &str {
        &self.snippet
    }
}

/// Typed failures for local knowledge preparation, rendering and search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KnowledgeError {
    /// The explicit selection contains no facts.
    #[error("knowledge selection is empty")]
    EmptySelection,
    /// The selection exceeds the maximum number of facts.
    #[error("knowledge selection exceeds the 64-fact limit")]
    TooManyFacts,
    /// Combined selected text exceeds the bounded in-memory budget.
    #[error("knowledge input exceeds the bounded byte limit")]
    InputTooLarge,
    /// The source identity or revision is invalid.
    #[error("knowledge source binding is invalid")]
    InvalidSource,
    /// Source display metadata contains a sensitive marker.
    #[error("knowledge source metadata contains sensitive content")]
    SensitiveSourceMetadata,
    /// A title or text field is empty, malformed or contains a NUL byte.
    #[error("knowledge text is empty or contains unsupported control data")]
    EmptyText,
    /// A text field exceeds its bounded byte limit.
    #[error("knowledge text exceeds its bounded byte limit")]
    TextTooLarge,
    /// A timestamp must be a nonzero Unix timestamp.
    #[error("knowledge timestamp is invalid")]
    InvalidTimestamp,
    /// The generated Markdown exceeds the output budget.
    #[error("generated knowledge Markdown exceeds its bounded byte limit")]
    MarkdownTooLarge,
    /// Generated Markdown still contains a conservative sensitive-content marker.
    #[error("generated knowledge Markdown contains sensitive content")]
    SensitiveGeneratedContent,
    /// The search query is empty or contains unsupported control data.
    #[error("knowledge search query is empty or invalid")]
    InvalidSearchQuery,
    /// The search source exceeds the bounded local string budget.
    #[error("knowledge search input exceeds its bounded byte limit")]
    SearchInputTooLarge,
}

/// Pure in-memory operations for explicit local knowledge and runbooks.
#[derive(Debug, Clone, Copy, Default)]
pub struct KnowledgeService;

impl KnowledgeService {
    /// Build source-bound summaries and a timestamp-ordered timeline.
    ///
    /// Only completed or known failed facts contribute content. Unknown,
    /// incomplete, and sensitive facts become safe exclusion markers. This
    /// method never treats a task audit as a fact because its input type is the
    /// explicit [`KnowledgeSelection`] boundary.
    pub fn build(selection: &KnowledgeSelection) -> Result<KnowledgeDocument, KnowledgeError> {
        let mut entries = Vec::new();
        let mut timeline = Vec::with_capacity(selection.facts.len());
        let mut exclusions = Vec::new();

        for fact in &selection.facts {
            let sensitive_title = contains_sensitive_marker(&fact.title);
            let (status, disposition, entry) = match &fact.outcome {
                SelectedFactOutcome::Completed(summary) => {
                    let sensitive = sensitive_title || contains_sensitive_marker(summary);
                    if sensitive {
                        (
                            TimelineStatus::SensitiveContent,
                            SaveDisposition::DoNotPersist {
                                reason: KnowledgeSaveBlock::SensitiveContent,
                            },
                            None,
                        )
                    } else {
                        let summary = normalize_text(summary);
                        let entry = KnowledgeEntry {
                            source: fact.source.clone(),
                            occurred_at: fact.occurred_at,
                            title: fact.title.clone(),
                            summary,
                            outcome: KnowledgeOutcome::Completed,
                        };
                        entries.push(entry.clone());
                        (
                            TimelineStatus::Completed,
                            SaveDisposition::Persistable,
                            Some(entry),
                        )
                    }
                }
                SelectedFactOutcome::Failed(summary) => {
                    let sensitive = sensitive_title || contains_sensitive_marker(summary);
                    if sensitive {
                        (
                            TimelineStatus::SensitiveContent,
                            SaveDisposition::DoNotPersist {
                                reason: KnowledgeSaveBlock::SensitiveContent,
                            },
                            None,
                        )
                    } else {
                        let entry = KnowledgeEntry {
                            source: fact.source.clone(),
                            occurred_at: fact.occurred_at,
                            title: fact.title.clone(),
                            summary: normalize_text(summary),
                            outcome: KnowledgeOutcome::Failed,
                        };
                        entries.push(entry.clone());
                        (
                            TimelineStatus::Failed,
                            SaveDisposition::Persistable,
                            Some(entry),
                        )
                    }
                }
                SelectedFactOutcome::Unknown => (
                    TimelineStatus::Unknown,
                    SaveDisposition::DoNotPersist {
                        reason: KnowledgeSaveBlock::UnknownOrIncompleteOutcome,
                    },
                    None,
                ),
                SelectedFactOutcome::NotStarted => (
                    TimelineStatus::NotStarted,
                    SaveDisposition::DoNotPersist {
                        reason: KnowledgeSaveBlock::UnknownOrIncompleteOutcome,
                    },
                    None,
                ),
                SelectedFactOutcome::Cancelled => (
                    TimelineStatus::Cancelled,
                    SaveDisposition::DoNotPersist {
                        reason: KnowledgeSaveBlock::UnknownOrIncompleteOutcome,
                    },
                    None,
                ),
            };

            let exclusion_reason = match (&fact.outcome, sensitive_title) {
                (SelectedFactOutcome::Completed(summary), false)
                | (SelectedFactOutcome::Failed(summary), false)
                    if !contains_sensitive_marker(summary) =>
                {
                    None
                }
                (SelectedFactOutcome::Completed(_), _) | (SelectedFactOutcome::Failed(_), _) => {
                    Some(KnowledgeExclusionReason::SensitiveContent)
                }
                (SelectedFactOutcome::Unknown, _) => Some(KnowledgeExclusionReason::UnknownOutcome),
                (SelectedFactOutcome::NotStarted | SelectedFactOutcome::Cancelled, _) => {
                    Some(KnowledgeExclusionReason::NotCompleted)
                }
            };
            if let Some(reason) = exclusion_reason {
                exclusions.push(KnowledgeExclusion {
                    source: fact.source.clone(),
                    occurred_at: fact.occurred_at,
                    reason,
                });
            }
            timeline.push(TimelineEvent {
                source: fact.source.clone(),
                occurred_at: fact.occurred_at,
                title: entry.as_ref().map(|entry| entry.title.clone()),
                status,
                disposition,
            });
            debug_assert!(entry.is_none() || exclusion_reason.is_none());
        }

        timeline.sort_by_key(|event| event.occurred_at);
        let disposition = exclusions
            .iter()
            .map(KnowledgeExclusion::disposition)
            .fold(SaveDisposition::Persistable, combine_disposition);
        Ok(KnowledgeDocument {
            entries,
            timeline,
            exclusions,
            disposition,
        })
    }

    /// Convert safe knowledge entries into descriptive, non-executable steps.
    pub fn to_runbook(
        document: &KnowledgeDocument,
        title: &str,
    ) -> Result<Runbook, KnowledgeError> {
        let title = checked_single_line(title, MAX_KNOWLEDGE_TEXT_BYTES)?;
        if title.is_empty() {
            return Err(KnowledgeError::EmptyText);
        }
        if contains_sensitive_marker(&title) {
            return Err(KnowledgeError::SensitiveGeneratedContent);
        }
        if document.entries.len() > MAX_RUNBOOK_STEPS {
            return Err(KnowledgeError::TooManyFacts);
        }
        let steps = document
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| RunbookStep {
                ordinal: index + 1,
                source: entry.source.clone(),
                title: entry.title.clone(),
                action: entry.summary.clone(),
                expected_result: match entry.outcome {
                    KnowledgeOutcome::Completed => "已完成并由来源事实支持".to_owned(),
                    KnowledgeOutcome::Failed => "已记录已知失败；执行前需重新审核".to_owned(),
                },
            })
            .collect();
        Ok(Runbook {
            title,
            steps,
            disposition: document.disposition,
        })
    }

    /// Render a knowledge document as bounded Markdown for preview.
    pub fn render_markdown(
        document: &KnowledgeDocument,
    ) -> Result<MarkdownArtifact, KnowledgeError> {
        let mut markdown = String::from("# KeelShell Local Knowledge\n\n");
        markdown.push_str("## Source-bound summaries\n\n");
        for entry in &document.entries {
            push_entry_markdown(&mut markdown, entry);
        }
        if document.entries.is_empty() {
            markdown.push_str("_No persistable completed facts._\n\n");
        }
        markdown.push_str("## Timeline\n\n");
        for event in &document.timeline {
            push_timeline_markdown(&mut markdown, event);
        }
        markdown.push_str("## Excluded from knowledge\n\n");
        if document.exclusions.is_empty() {
            markdown.push_str("_None._\n");
        } else {
            for exclusion in &document.exclusions {
                markdown.push_str("- ");
                push_source(&mut markdown, exclusion.source());
                markdown.push_str(": ");
                markdown.push_str(exclusion_reason_text(exclusion.reason()));
                markdown.push_str("; **do not persist or send**.\n");
            }
        }
        bounded_markdown(markdown, document.disposition)
    }

    /// Render a descriptive runbook as bounded Markdown; this never executes steps.
    pub fn render_runbook(runbook: &Runbook) -> Result<MarkdownArtifact, KnowledgeError> {
        let mut markdown = format!("# {}\n\n", escape_markdown(&runbook.title));
        markdown.push_str("> This is descriptive text generated from reviewed local facts.\n");
        markdown.push_str(
            "> It is not an execution plan and must be reviewed before saving or sending.\n\n",
        );
        if runbook.steps.is_empty() {
            markdown.push_str("_No persistable steps._\n");
        } else {
            for step in &runbook.steps {
                markdown.push_str(&format!(
                    "{}. **{}**\n   - Action: {}\n   - Expected result: {}\n   - Source: ",
                    step.ordinal,
                    escape_markdown(&step.title),
                    escape_markdown(&one_line(&step.action)),
                    escape_markdown(&step.expected_result),
                ));
                push_source(&mut markdown, &step.source);
                markdown.push('\n');
            }
        }
        if !runbook.disposition.is_persistable() {
            markdown.push_str("\n> **Do not persist or send:** selected input contains excluded results or sensitive content.\n");
        }
        bounded_markdown(markdown, runbook.disposition)
    }

    /// Search a bounded local Markdown string without network, command or file I/O.
    ///
    /// Search is intended for Markdown returned by [`Self::render_markdown`] or
    /// [`Self::render_runbook`]. It does not turn arbitrary text into knowledge.
    pub fn search_markdown(
        markdown: &str,
        query: &str,
    ) -> Result<Vec<SearchMatch>, KnowledgeError> {
        checked_search_text(markdown)?;
        if query.is_empty() || query.contains('\0') || query.chars().any(char::is_control) {
            return Err(KnowledgeError::InvalidSearchQuery);
        }
        if query.len() > MAX_KNOWLEDGE_SEARCH_QUERY_BYTES {
            return Err(KnowledgeError::InvalidSearchQuery);
        }
        let folded_query = query.to_lowercase();
        let mut matches = Vec::new();
        for (line_index, line) in markdown.lines().enumerate() {
            let Some(byte_column) = case_insensitive_byte_column(line, &folded_query) else {
                continue;
            };
            let snippet = bounded_snippet(line);
            matches.push(SearchMatch {
                line: line_index + 1,
                column: line[..byte_column].chars().count() + 1,
                snippet,
            });
            if matches.len() == MAX_KNOWLEDGE_SEARCH_MATCHES {
                break;
            }
        }
        Ok(matches)
    }
}

fn checked_text(value: &str, limit: usize) -> Result<(), KnowledgeError> {
    if value.trim().is_empty() || value.contains('\0') || value.chars().any(char::is_control) {
        return Err(KnowledgeError::EmptyText);
    }
    if value.len() > limit {
        return Err(KnowledgeError::TextTooLarge);
    }
    Ok(())
}

fn checked_single_line(value: &str, limit: usize) -> Result<String, KnowledgeError> {
    if value.contains('\n') || value.contains('\r') {
        return Err(KnowledgeError::EmptyText);
    }
    checked_text(value, limit)?;
    Ok(value.trim().to_owned())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_sensitive_marker(value: &str) -> bool {
    let folded = value.to_ascii_lowercase();
    [
        "authorization",
        "bearer ",
        "api key",
        "api-key",
        "api_key",
        "apikey",
        "password",
        "passwd",
        "pwd=",
        "secret",
        "token",
        "private key",
        "-----begin",
        "x-api-key",
    ]
    .iter()
    .any(|marker| folded.contains(marker))
}

fn combine_disposition(current: SaveDisposition, next: SaveDisposition) -> SaveDisposition {
    match (current, next) {
        (
            SaveDisposition::DoNotPersist {
                reason: KnowledgeSaveBlock::SensitiveContent,
            },
            _,
        )
        | (
            _,
            SaveDisposition::DoNotPersist {
                reason: KnowledgeSaveBlock::SensitiveContent,
            },
        ) => SaveDisposition::DoNotPersist {
            reason: KnowledgeSaveBlock::SensitiveContent,
        },
        (SaveDisposition::DoNotPersist { .. }, _) | (_, SaveDisposition::DoNotPersist { .. }) => {
            SaveDisposition::DoNotPersist {
                reason: KnowledgeSaveBlock::UnknownOrIncompleteOutcome,
            }
        }
        (SaveDisposition::Persistable, SaveDisposition::Persistable) => {
            SaveDisposition::Persistable
        }
    }
}

fn bounded_markdown(
    markdown: String,
    disposition: SaveDisposition,
) -> Result<MarkdownArtifact, KnowledgeError> {
    if markdown.len() > MAX_KNOWLEDGE_MARKDOWN_BYTES || markdown.contains('\0') {
        return Err(KnowledgeError::MarkdownTooLarge);
    }
    if contains_sensitive_marker(&markdown) {
        return Err(KnowledgeError::SensitiveGeneratedContent);
    }
    Ok(MarkdownArtifact {
        text: markdown,
        disposition,
    })
}

fn checked_search_text(markdown: &str) -> Result<(), KnowledgeError> {
    if markdown.len() > MAX_KNOWLEDGE_MARKDOWN_BYTES || markdown.contains('\0') {
        return Err(KnowledgeError::SearchInputTooLarge);
    }
    Ok(())
}

fn bounded_snippet(line: &str) -> String {
    if line.len() <= MAX_KNOWLEDGE_SEARCH_SNIPPET_BYTES {
        return line.to_owned();
    }
    let mut end = MAX_KNOWLEDGE_SEARCH_SNIPPET_BYTES.saturating_sub('…'.len_utf8());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &line[..end])
}

fn case_insensitive_byte_column(line: &str, folded_query: &str) -> Option<usize> {
    let mut folded_line = String::with_capacity(line.len());
    let mut original_offsets = Vec::with_capacity(line.len());
    for (original_offset, character) in line.char_indices() {
        let lowered = character.to_lowercase().collect::<String>();
        for _ in 0..lowered.len() {
            original_offsets.push(original_offset);
        }
        folded_line.push_str(&lowered);
    }
    let folded_offset = folded_line.find(folded_query)?;
    original_offsets.get(folded_offset).copied()
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_markdown(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\\' | '`' | '*' | '_' | '[' | ']' | '#' => format!("\\{character}"),
            _ => character.to_string(),
        })
        .collect()
}

fn push_source(markdown: &mut String, source: &SourceBinding) {
    markdown.push_str(&format!(
        "{} `{}` rev {} ({})",
        format_source_kind(source.kind),
        source.id,
        source.revision,
        escape_markdown(source.label()),
    ));
}

fn push_entry_markdown(markdown: &mut String, entry: &KnowledgeEntry) {
    markdown.push_str("- **");
    markdown.push_str(&escape_markdown(&entry.title));
    markdown.push_str("** — ");
    markdown.push_str(&escape_markdown(&entry.summary));
    markdown.push_str(" [");
    markdown.push_str(match entry.outcome {
        KnowledgeOutcome::Completed => "completed",
        KnowledgeOutcome::Failed => "failed",
    });
    markdown.push_str("]\n  - Source: ");
    push_source(markdown, &entry.source);
    markdown.push('\n');
}

fn push_timeline_markdown(markdown: &mut String, event: &TimelineEvent) {
    markdown.push_str("- t=");
    markdown.push_str(&event.occurred_at.to_string());
    markdown.push_str(" [");
    markdown.push_str(format_timeline_status(event.status));
    markdown.push_str("] ");
    if let Some(title) = event.title() {
        markdown.push_str(&escape_markdown(title));
    } else {
        markdown.push_str("[内容已省略]");
    }
    markdown.push_str(" — ");
    push_source(markdown, &event.source);
    if !event.disposition.is_persistable() {
        markdown.push_str("; **不可保存**");
    }
    markdown.push('\n');
}

fn format_source_kind(kind: KnowledgeSourceKind) -> &'static str {
    match kind {
        KnowledgeSourceKind::Session => "session",
        KnowledgeSourceKind::Workflow => "workflow",
        KnowledgeSourceKind::File => "file",
        KnowledgeSourceKind::Monitor => "monitor",
        KnowledgeSourceKind::Manual => "manual",
    }
}

fn format_timeline_status(status: TimelineStatus) -> &'static str {
    match status {
        TimelineStatus::Completed => "completed",
        TimelineStatus::Failed => "failed",
        TimelineStatus::Unknown => "unknown",
        TimelineStatus::NotStarted => "not_started",
        TimelineStatus::Cancelled => "cancelled",
        TimelineStatus::SensitiveContent => "sensitive_content",
    }
}

fn exclusion_reason_text(reason: KnowledgeExclusionReason) -> &'static str {
    match reason {
        KnowledgeExclusionReason::UnknownOutcome => "result is unknown",
        KnowledgeExclusionReason::NotCompleted => "work did not reach a known completed result",
        KnowledgeExclusionReason::SensitiveContent => "sensitive content was detected",
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "fixture construction keeps each test failure local and descriptive"
)]
mod tests {
    use super::*;

    fn source(kind: KnowledgeSourceKind, label: &str) -> SourceBinding {
        SourceBinding::new(kind, Uuid::new_v4(), 1, label).expect("valid source")
    }

    fn selection(facts: Vec<SelectedFact>) -> KnowledgeSelection {
        KnowledgeSelection::new(facts).expect("valid selection")
    }

    #[test]
    fn build_keeps_source_bound_completed_facts_and_orders_timeline() {
        let first_source = source(KnowledgeSourceKind::Session, "dev session");
        let second_source = source(KnowledgeSourceKind::File, "config.toml");
        let selection = selection(vec![
            SelectedFact::completed(second_source.clone(), 20, "Config checked", "Loaded safely")
                .expect("completed fact"),
            SelectedFact::failed(first_source.clone(), 10, "Probe failed", "Remote refused")
                .expect("failed fact"),
        ]);

        let document = KnowledgeService::build(&selection).expect("knowledge document");

        assert_eq!(document.entries().len(), 2);
        assert_eq!(document.entries()[0].source(), &second_source);
        assert_eq!(document.timeline()[0].occurred_at(), 10);
        assert_eq!(document.timeline()[1].occurred_at(), 20);
        assert!(document.disposition().is_persistable());
    }

    #[test]
    fn unknown_and_sensitive_facts_are_marked_without_retaining_content() {
        let unknown = SelectedFact::unknown(
            source(KnowledgeSourceKind::Workflow, "deploy workflow"),
            10,
            "Deployment result",
        )
        .expect("unknown fact");
        let secret_value = "token=super-secret-value";
        let sensitive = SelectedFact::completed(
            source(KnowledgeSourceKind::Session, "ops session"),
            20,
            "Credential output",
            secret_value,
        )
        .expect("sensitive fact");
        let document =
            KnowledgeService::build(&selection(vec![unknown, sensitive])).expect("build");

        assert!(document.entries().is_empty());
        assert_eq!(document.exclusions().len(), 2);
        assert_eq!(document.timeline()[0].status(), TimelineStatus::Unknown);
        assert_eq!(
            document.timeline()[1].status(),
            TimelineStatus::SensitiveContent
        );
        assert!(document.timeline()[0].title().is_none());
        assert!(document.timeline()[1].title().is_none());
        assert!(!document.disposition().is_persistable());
        assert_eq!(
            document.disposition(),
            SaveDisposition::DoNotPersist {
                reason: KnowledgeSaveBlock::SensitiveContent
            }
        );
        let markdown = KnowledgeService::render_markdown(&document).expect("render");
        assert!(!markdown.text().contains(secret_value));
        assert!(!markdown.text().contains("Deployment result"));
        assert!(markdown.text().contains("不可保存"));
        assert!(!markdown.disposition().is_persistable());
    }

    #[test]
    fn source_metadata_rejects_authorization_and_private_key_markers() {
        for label in ["Authorization header", "private key file", "api token"] {
            assert_eq!(
                SourceBinding::new(KnowledgeSourceKind::Manual, Uuid::new_v4(), 1, label),
                Err(KnowledgeError::SensitiveSourceMetadata)
            );
        }
    }

    #[test]
    fn runbook_and_local_search_are_bounded_and_non_executable() {
        let fact = SelectedFact::completed(
            source(KnowledgeSourceKind::Manual, "operator note"),
            42,
            "Restart verification",
            "Confirm the service reports ready",
        )
        .expect("fact");
        let document = KnowledgeService::build(&selection(vec![fact])).expect("document");
        let runbook = KnowledgeService::to_runbook(&document, "Recovery notes").expect("runbook");
        assert_eq!(runbook.steps().len(), 1);
        assert!(runbook.steps()[0].action().contains("Confirm"));
        let markdown = KnowledgeService::render_runbook(&runbook).expect("markdown");
        let matches =
            KnowledgeService::search_markdown(markdown.text(), "recovery").expect("search");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line(), 1);
        assert!(matches[0].snippet().contains("Recovery"));
        assert_eq!(
            KnowledgeService::to_runbook(&document, "token recovery"),
            Err(KnowledgeError::SensitiveGeneratedContent)
        );
    }

    #[test]
    fn selection_and_search_limits_return_typed_errors() {
        let fact = SelectedFact::completed(
            source(KnowledgeSourceKind::Manual, "bounded"),
            1,
            "title",
            "summary",
        )
        .expect("fact");
        let too_many = vec![fact; MAX_KNOWLEDGE_FACTS + 1];
        assert_eq!(
            KnowledgeSelection::new(too_many),
            Err(KnowledgeError::TooManyFacts)
        );
        assert_eq!(
            KnowledgeService::search_markdown("text", ""),
            Err(KnowledgeError::InvalidSearchQuery)
        );
        assert_eq!(
            KnowledgeService::search_markdown(
                "text",
                &"x".repeat(MAX_KNOWLEDGE_SEARCH_QUERY_BYTES + 1)
            ),
            Err(KnowledgeError::InvalidSearchQuery)
        );
        assert_eq!(
            KnowledgeSelection::new(Vec::new()),
            Err(KnowledgeError::EmptySelection)
        );
        assert_eq!(
            SourceBinding::new(KnowledgeSourceKind::Manual, Uuid::new_v4(), 0, "source"),
            Err(KnowledgeError::InvalidSource)
        );
        assert_eq!(
            SourceBinding::new(KnowledgeSourceKind::Manual, Uuid::new_v4(), 1, "   "),
            Err(KnowledgeError::EmptyText)
        );
        assert_eq!(
            SelectedFact::completed(
                source(KnowledgeSourceKind::Manual, "source"),
                1,
                "title",
                "   ",
            ),
            Err(KnowledgeError::EmptyText)
        );
        assert_eq!(
            SelectedFact::completed(
                source(KnowledgeSourceKind::Manual, "source"),
                1,
                "title",
                "bad\0text",
            ),
            Err(KnowledgeError::EmptyText)
        );
        assert_eq!(
            SelectedFact::completed(
                source(KnowledgeSourceKind::Manual, "source"),
                1,
                "title",
                &"x".repeat(MAX_KNOWLEDGE_TEXT_BYTES + 1),
            ),
            Err(KnowledgeError::TextTooLarge)
        );
    }

    #[test]
    fn unicode_search_maps_folded_offsets_to_original_columns() {
        let matches = KnowledgeService::search_markdown("aİx", "\u{307}").expect("search");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line(), 1);
        assert_eq!(matches[0].column(), 2);
    }

    #[test]
    fn sensitive_markers_include_spaced_api_keys_and_snippets_stay_bounded() {
        let fact = SelectedFact::completed(
            source(KnowledgeSourceKind::Manual, "operator note"),
            1,
            "API key output",
            "value omitted",
        )
        .expect("fact");
        let document = KnowledgeService::build(&selection(vec![fact])).expect("document");
        assert_eq!(
            document.disposition(),
            SaveDisposition::DoNotPersist {
                reason: KnowledgeSaveBlock::SensitiveContent
            }
        );
        let matches = KnowledgeService::search_markdown(&"a".repeat(300), "a").expect("search");
        assert!(matches[0].snippet().len() <= MAX_KNOWLEDGE_SEARCH_SNIPPET_BYTES);
    }
}
