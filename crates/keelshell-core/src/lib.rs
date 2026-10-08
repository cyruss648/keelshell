//! KeelShell domain types and local persistence, independent of UI and transports.
//!
//! Profiles contain credential references, never passwords or API keys. Validation
//! runs both at import and before persistence; transports must still enforce their
//! own protocol and host-key policies. [`StateStore`] uses blocking filesystem I/O
//! and belongs on a worker, outside the GPUI render/event thread.

#![deny(missing_docs)]

mod ai_backends;
mod ai_messages;
mod ai_profiles;
mod ai_sampling;
mod batch_audit;
mod batch_parameters;
mod batch_template;
mod batch_workflow;
mod completion;
mod connection_library;
mod connection_library_batch;
mod diff;
mod directory_compare;
mod directory_sync;
mod error;
mod model;
mod network_diagnostic;
mod openssh;
mod profile_sync;
mod proxy;
mod reconnect;
mod routes;
mod scheduled_workflow;
mod snippet_template;
mod snippets;
mod store;
mod text_merge;
mod text_patch;
mod updates;
mod vault;
mod workflow_audit;

pub use ai_backends::{AiBackend, AiLocalAgent, AiLocalAgentLimits, AiLocalAgentWorkingDirectory};
pub use ai_messages::{AiMessagesEffort, AiMessagesInference, AiMessagesThinking};
pub use ai_profiles::{
    AiApiStyle, AiAuthentication, AiCustomHeader, AiModelReasoning, AiPreset, AiProfileCatalog,
    AiProxy, AiReasoningCapability, AiReasoningSelection, AiSecretRef, NamedAiProfile,
};
pub use ai_sampling::{AiModelSampling, AiSamplingValue};
pub use batch_audit::{
    BatchAuditRecord, BatchAuditSummary, MAX_BATCH_AUDIT_TARGETS, MAX_BATCH_AUDITS, command_sha256,
};
pub use batch_parameters::{BatchParameterError, BatchParameterValues, BatchParameterizedTemplate};
pub use batch_template::{
    BATCH_TEMPLATE_VARIABLES, BatchCommandTemplate, BatchTargetContext, BatchTemplateError,
};
pub use batch_workflow::{
    BatchTaskOutcome, BatchTaskSkipReason, BatchTaskSpec, BatchTaskStatus, BatchWorkflowError,
    BatchWorkflowLedger, BatchWorkflowPlan, BatchWorkflowReviewToken, ConfirmedBatchWorkflow,
    MAX_BATCH_TASK_COMMAND_BYTES, MAX_BATCH_TASK_DEPENDENCIES, MAX_BATCH_WORKFLOW_COMMAND_BYTES,
    MAX_BATCH_WORKFLOW_TARGETS, MAX_BATCH_WORKFLOW_TASKS,
};
pub use completion::{
    CompletionAnalysis, CompletionEdit, CompletionError, CompletionPlan, CompletionQuery,
    CompletionUnsupported, LiteralCandidate, MAX_COMPLETION_INPUT_BYTES, MAX_COMPLETION_PATH_BYTES,
    analyze_completion,
};
pub use connection_library::{
    ConnectionFolder, DeletedConnection, FolderRow, MAX_RECENT_CONNECTIONS, RecentConnection,
};
pub use connection_library_batch::{ConnectionLibraryAction, ConnectionLibraryBatchError};
pub use diff::{
    DEFAULT_DIFF_CONTEXT_LINES, DiffError, DiffHunk, DiffLine, DiffLineKind, DiffSide,
    MAX_DIFF_CONTEXT_LINES, MAX_DIFF_INPUT_BYTES, MAX_DIFF_LINES, MAX_DIFF_OUTPUT_BYTES,
    UnifiedDiff, diff_text, diff_text_with_context, diff_utf8,
};
pub use directory_compare::{
    DirectoryCompareError, DirectoryCompareReport, DirectoryCompareRow, DirectoryCompareSide,
    DirectoryContentHash, DirectoryEntryKind, DirectoryEntrySnapshot, DirectoryEntryStatus,
    DirectoryHashError, MAX_DIRECTORY_COMPARE_ENTRIES, MAX_DIRECTORY_COMPARE_PATH_BYTES,
    MAX_DIRECTORY_HASH_BYTES, compare_directories, hash_directory_content,
};
pub use directory_sync::{
    ConfirmedDirectorySync, DirectoryMirrorConflict, DirectoryMirrorConflictReason,
    DirectorySyncConfirmError, DirectorySyncDeletePolicy, DirectorySyncDirection,
    DirectorySyncOperation, DirectorySyncPlan, DirectorySyncPlanError, DirectorySyncReviewToken,
    MAX_DIRECTORY_MIRROR_CONTENT_BYTES, MAX_DIRECTORY_MIRROR_DEPTH, directory_mirror_conflicts,
    plan_directory_mirror, plan_directory_sync,
};
pub use error::{Error, ValidationError};
pub use model::{
    AiSettings, AppState, AuthMethod, Connection, ImportReport, Language, SCHEMA_VERSION, Settings,
    SnapshotRevision, Snippet, Theme,
};
pub use network_diagnostic::{
    DIAGNOSTIC_REMOTE_SECONDS, MAX_DIAGNOSTIC_INPUT_BYTES, MAX_DIAGNOSTIC_OUTPUT_BYTES,
    NetworkDiagnosticInputError, NetworkDiagnosticKind, NetworkDiagnosticRequest,
};
pub use openssh::{
    MAX_OPENSSH_CONFIG_BYTES, MAX_OPENSSH_ENTRIES, MAX_OPENSSH_INCLUDE_DEPTH,
    MAX_OPENSSH_LINE_BYTES, OpenSshConfig, OpenSshEntry, OpenSshImportReport, OpenSshWarning,
    parse_openssh_config, parse_openssh_config_with_includes,
};
pub use proxy::{ConnectionProxy, ProxyAuthentication, ProxyKind};
pub use reconnect::ReconnectPolicy;
pub use routes::{ConnectionRoute, HostKeyScope, MAX_JUMP_HOSTS, RouteEndpoint, RouteIdentity};
pub use scheduled_workflow::{
    MAX_WORKFLOW_SCHEDULE_COUNT, MAX_WORKFLOW_SCHEDULE_GRACE_SECONDS,
    MAX_WORKFLOW_SCHEDULE_SPAN_SECONDS, MIN_WORKFLOW_SCHEDULE_INTERVAL_SECONDS,
    ScheduleClockSample, WORKFLOW_SCHEDULE_CLOCK_DRIFT_MILLIS, WorkflowScheduleBinding,
    WorkflowScheduleDueToken, WorkflowScheduleError, WorkflowScheduleInvalidationReason,
    WorkflowScheduleLedger, WorkflowScheduleOutcome, WorkflowScheduleSlot,
    WorkflowScheduleSlotStatus, WorkflowScheduleSpec, WorkflowScheduleStatus, format_fixed_offset,
    format_fixed_offset_datetime, parse_fixed_offset, parse_fixed_offset_datetime,
};
pub use snippet_template::{
    MAX_SNIPPET_TEMPLATE_BYTES, MAX_SNIPPET_VALUE_BYTES, MAX_SNIPPET_VARIABLES, SnippetTemplate,
    SnippetTemplateContext, SnippetTemplateError, compile_snippet_template,
};
pub use store::{
    ConfigBackup, ConfigBackupId, ConfigBackupStatus, ConfigRecoveryPreview, ConfigRecoverySummary,
    ConfigSourceStatus, MAX_CONFIG_BACKUPS, MAX_CONFIG_ORIGINALS, StateStore,
};
pub use updates::{UpdateCheckFrequency, UpdatePreferences};
pub use vault::{CredentialKind, CredentialMetadata, CredentialVault, VaultStore};

pub use profile_sync::{
    ProfileSyncChoice, ProfileSyncError, ProfileSyncLocal, ProfileSyncOutcome, ProfileSyncPreview,
    ProfileSyncReview, ProfileSyncRouteChange, ProfileSyncRow, ProfileSyncService, SyncProfile,
};

pub use workflow_audit::{
    MAX_WORKFLOW_AUDIT_TASKS, MAX_WORKFLOW_AUDIT_TOTAL_TASKS, MAX_WORKFLOW_AUDITS,
    WorkflowAuditNotStarted, WorkflowAuditOutcome, WorkflowAuditRecord, WorkflowAuditTrigger,
    WorkflowTaskAudit,
};

pub use text_merge::{
    MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES, TextEditError, TextMergeChoice, TextMergeConflict,
    TextMergePlan, merge_text,
};
pub use text_patch::apply_text_patch;
