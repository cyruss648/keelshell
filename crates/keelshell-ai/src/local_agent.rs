//! Explicit Ask-only CLI invocations with owned processes and bounded JSONL.
//!
//! Local CLIs still send context to an inference service. They are not an offline
//! model or a local terminal. Only specifically checked versions are admitted;
//! a new version requires a fresh capability and controlled-wire review.

mod bootstrap;
mod config;
mod directory;
mod process;
mod progress;
mod protocol;

use std::{fmt, time::Instant};

use serde_json::Value;
use zeroize::Zeroizing;

use crate::{AiError, ContextDraft, ProviderConfig, ProviderProtocol, RedactionReport};

pub use bootstrap::run_local_agent_directory_launcher;
pub use config::{LocalAgentConfig, LocalAgentKind, LocalAgentLimits, LocalAgentVersion};
pub use directory::{LocalAgentWorkingDirectory, ValidatedLocalAgentDirectory};
pub use process::{LocalAgentClient, LocalAgentProbe};
pub use progress::{LocalAskProgress, LocalAskProgressReceiver, LocalAskStage};

/// CLI failures omit process output, user paths, context and credential values.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalAgentError {
    /// The executable or scratch-directory configuration is invalid.
    #[error("Local agent requires an absolute executable and an existing scratch directory")]
    InvalidConfiguration,
    /// Windows batch launchers implicitly invoke a shell and are not accepted.
    #[error("Select the native CLI executable; batch and command launchers are unsupported")]
    UnsupportedExecutable,
    /// The configured inference URL is not HTTPS or loopback HTTP.
    #[error("Local agent inference endpoint must be HTTPS or loopback HTTP without credentials")]
    InvalidEndpoint,
    /// The request timeout or byte/frame limits exceed supported bounds.
    #[error("Local agent limits are outside the bounded supported range")]
    InvalidLimits,
    /// The CLI could not be spawned; no OS error text is exposed.
    #[error("Local agent executable could not be started")]
    SpawnFailed,
    /// Scratch data could not be created or removed.
    #[error("Local agent isolated scratch data could not be created or removed")]
    ScratchFailed,
    /// Selected directory metadata is not a bounded absolute native path.
    #[error("Local agent working directory must be a bounded absolute native path")]
    DirectoryInvalid,
    /// The explicitly selected directory no longer exists.
    #[error("Local agent working directory does not exist")]
    DirectoryMissing,
    /// A selected path component is a file or another non-directory object.
    #[error("Local agent working directory path contains a non-directory object")]
    DirectoryNotDirectory,
    /// A selected path or project-metadata entry is a symbolic link/reparse point.
    #[error("Local agent working directory or project metadata contains a symbolic link")]
    DirectorySymlink,
    /// Directory access was refused or filesystem identity could not be read.
    #[error("Local agent working directory cannot be accessed")]
    DirectoryUnavailable,
    /// A reviewed directory was moved, replaced or otherwise ceased to match.
    #[error("Local agent reviewed working directory changed; review again")]
    DirectoryChanged,
    /// Codex project metadata is malformed, special or outside its size bound.
    #[error("Local agent project metadata is invalid or outside its supported bound")]
    DirectoryMetadataInvalid,
    /// Codex project metadata appeared, disappeared or changed after review.
    #[error("Local agent reviewed project metadata changed; review again")]
    DirectoryMetadataChanged,
    /// A selected directory requires background validation and an exact preview.
    #[error("Local agent selected directory requires background validation and review")]
    DirectoryReviewRequired,
    /// Native directory anchoring cannot be enforced on this platform.
    #[error("Local agent working directory cannot be safely anchored on this platform")]
    DirectoryUnsupported,
    /// Background directory validation exceeded its separate bounded deadline.
    #[error("Local agent working directory validation timed out; authority is revoked")]
    DirectoryValidationTimedOut,
    /// The reviewed native CLI was moved, replaced or changed during admission.
    #[error("Local agent executable changed during admission; review again")]
    ExecutableChanged,
    /// The CLI version is not within the specifically checked compatibility set.
    #[error("Local agent version is outside the checked compatibility set")]
    UnsupportedVersion,
    /// Required arguments or effective disabling flags are missing.
    #[error("Local agent cannot enforce the required Ask capability policy")]
    IsolationUnsupported,
    /// No explicit ephemeral inference credential was supplied.
    #[error("Local agent Ask requires an explicit ephemeral inference credential")]
    MissingCredential,
    /// The credential is malformed or appears in the approved context.
    #[error("Local agent credential is invalid or present in the approved context")]
    CredentialInContext,
    /// The immutable Ask review was not consumed within five minutes.
    #[error("Local agent Ask review expired; prepare and review the request again")]
    ReviewExpired,
    /// The local deadline expired. This does not cancel service-side inference.
    #[error("Local agent timed out locally; service processing may already have begun")]
    Timeout,
    /// Cancellation was requested. This does not prove service-side cancellation.
    #[error("Local agent cancelled locally; service processing may already have begun")]
    Cancelled,
    /// A pipe operation failed without exposing captured content.
    #[error("Local agent input or output pipe failed")]
    PipeFailed,
    /// Combined output, a JSONL line, answer or frame count exceeded its bound.
    #[error("Local agent output exceeded its configured bound")]
    OutputTooLarge,
    /// JSONL does not match the supported ordering or complete-answer contract.
    #[error("Local agent returned invalid or incomplete supported-protocol JSONL")]
    InvalidProtocol,
    /// A supposedly Ask-only stream advertises or attempts a tool or hook.
    #[error("Local agent advertised or attempted an operation outside Ask scope")]
    UnexpectedOperation,
    /// A structured service error occurred. Its arbitrary message is suppressed.
    #[error("Local agent reported an inference failure")]
    InferenceFailed,
    /// The process ended unsuccessfully without a successful protocol receipt.
    #[error("Local agent exited unsuccessfully")]
    ProcessFailed,
    /// Killing or reaping the owned process containment did not complete.
    #[error("Local agent process cleanup could not be confirmed")]
    CleanupFailed,
    /// No complete non-whitespace assistant answer was returned.
    #[error("Local agent returned no complete textual answer")]
    EmptyReply,
    /// Explicit context preparation failed before any process was started.
    #[error("Local agent context could not be prepared: {0}")]
    Context(AiError),
}

/// An in-memory inference credential; it is never serialized or placed in argv.
///
/// This first adapter intentionally does not reuse CLI subscription logins.
/// Claude bare mode excludes OAuth and keychain reads; both CLIs receive only
/// this supplied API credential. The caller must resolve secret references in
/// its credential service rather than storing values in profile metadata.
pub struct LocalAgentCredential(Zeroizing<String>);

impl LocalAgentCredential {
    /// Take ownership of a nonempty credential without reading the environment.
    pub fn new(value: impl Into<String>) -> Result<Self, LocalAgentError> {
        let value = Zeroizing::new(value.into());
        if value.trim().is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
            return Err(LocalAgentError::MissingCredential);
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for LocalAgentCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LocalAgentCredential(<redacted>)")
    }
}

/// An immutable exact stdin/context preview bound to executable and policy.
pub struct PreparedLocalAsk {
    config: LocalAgentConfig,
    stdin: Zeroizing<String>,
    preview: Zeroizing<String>,
    report: RedactionReport,
    directory: Option<ValidatedLocalAgentDirectory>,
}

impl PreparedLocalAsk {
    /// The full review: CLI, inference endpoint, limits, isolation policy and stdin.
    ///
    /// It may contain deliberately selected private information. Do not log it.
    pub fn preview_json(&self) -> &str {
        &self.preview
    }

    /// The exact bytes sent to stdin, excluding its final newline.
    pub fn preview_stdin(&self) -> &str {
        &self.stdin
    }

    /// Literal/heuristic redaction and truncation statistics for the review.
    pub fn redaction_report(&self) -> RedactionReport {
        self.report
    }

    /// The immutable executable, service and permissions bound to this request.
    pub fn config(&self) -> &LocalAgentConfig {
        &self.config
    }

    /// Consume the exact review only after an explicit human send action.
    pub fn approve(self) -> ApprovedLocalAsk {
        ApprovedLocalAsk {
            prepared: self,
            approved_at: Instant::now(),
        }
    }
}

impl fmt::Debug for PreparedLocalAsk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedLocalAsk")
            .field("kind", &self.config.kind())
            .field("stdin_bytes", &self.stdin.len())
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

/// A single-use, five-minute human approval of a complete immutable Ask request.
pub struct ApprovedLocalAsk {
    prepared: PreparedLocalAsk,
    approved_at: Instant,
}

/// A successful, complete assistant answer, distinct from reasoning and tool output.
pub struct LocalAgentReply {
    text: Zeroizing<String>,
    version: LocalAgentVersion,
    frames: usize,
}

impl LocalAgentReply {
    /// Complete assistant text. Treat any proposed command as untrusted text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The checked installed CLI version actually used for this invocation.
    pub fn version(&self) -> LocalAgentVersion {
        self.version
    }

    /// Validated protocol frame count, without retaining reasoning or diagnostics.
    pub fn frame_count(&self) -> usize {
        self.frames
    }
}

impl fmt::Debug for LocalAgentReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalAgentReply")
            .field("answer_bytes", &self.text.len())
            .field("version", &self.version)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

impl LocalAgentConfig {
    /// Sanitize explicitly supplied context and create an immutable CLI Ask review.
    ///
    /// Uses the same conservative context admission as the HTTP adapters, then
    /// extracts only the instructions and selected user context. No HTTP request
    /// is created or sent and no CLI/configuration file is read by preparation.
    /// `byte_budget` must be 1–65536 bytes; show the preview and redaction report.
    ///
    /// ```
    /// use keelshell_ai::{ContextDraft, LocalAgentConfig, LocalAgentKind};
    /// # fn main() -> Result<(), keelshell_ai::LocalAgentError> {
    /// let root = std::env::temp_dir();
    /// let config = LocalAgentConfig::new(
    ///     LocalAgentKind::Codex, root.join("codex"), root, "selected-model",
    /// )?;
    /// let review = config.prepare(ContextDraft::new("Explain selected output"), &[], 8192)?;
    /// assert!(review.preview_stdin().contains("Explain selected output"));
    /// // Display review.preview_json(); an explicit send action consumes approve().
    /// # Ok(())
    /// # }
    /// ```
    pub fn prepare(
        &self,
        context: ContextDraft,
        secrets: &[&str],
        byte_budget: usize,
    ) -> Result<PreparedLocalAsk, LocalAgentError> {
        if self.working_directory != LocalAgentWorkingDirectory::Isolated {
            return Err(LocalAgentError::DirectoryReviewRequired);
        }
        self.prepare_inner(context, secrets, byte_budget, None)
    }

    /// Prepare an exact Ask preview after background directory validation.
    ///
    /// A selected path authorizes inspecting its project metadata, not sending
    /// its contents or activating tools/hooks/MCP. The preview displays selected
    /// and canonical paths and warns that directory metadata can reach the
    /// inference service. Only explicit stdin context is included; the runtime
    /// rechecks the retained directory authority after every admission wait.
    pub async fn prepare_checked(
        &self,
        context: ContextDraft,
        secrets: &[&str],
        byte_budget: usize,
        cancellation: &crate::RequestCancellation,
    ) -> Result<PreparedLocalAsk, LocalAgentError> {
        self.validate_context_metadata(secrets)?;
        let directory = self.validate_working_directory(cancellation).await?;
        if let Some(directory) = &directory {
            crate::RequestOptions::default()
                .with_context_secrets(secrets)
                .map_err(LocalAgentError::Context)?
                .validate_metadata_text(directory.canonical_path().to_string_lossy().as_ref())
                .map_err(LocalAgentError::Context)?;
        }
        let config = self.clone();
        #[cfg(unix)]
        let config = {
            let mut config = config;
            if directory.is_some() && config.directory_launcher.is_none() {
                // Resolve the current trusted application on a worker, never the UI.
                config.directory_launcher = Some(
                    tokio::task::spawn_blocking(std::env::current_exe)
                        .await
                        .map_err(|_| LocalAgentError::DirectoryUnsupported)?
                        .map_err(|_| LocalAgentError::DirectoryUnsupported)?,
                );
            }
            config
        };
        config.prepare_inner(context, secrets, byte_budget, directory)
    }

    fn prepare_inner(
        &self,
        context: ContextDraft,
        secrets: &[&str],
        byte_budget: usize,
        directory: Option<ValidatedLocalAgentDirectory>,
    ) -> Result<PreparedLocalAsk, LocalAgentError> {
        self.validate_context_metadata(secrets)?;
        // Reuse the existing admission/redaction implementation. This formatter
        // never sends HTTP; its endpoint is not part of the CLI request or review.
        let formatter = ProviderConfig::new_with_protocol(
            "https://api.openai.com/v1/responses",
            &self.model,
            ProviderProtocol::Responses,
        )
        .map_err(LocalAgentError::Context)?;
        let sanitized = context
            .prepare(&formatter, secrets, byte_budget)
            .map_err(LocalAgentError::Context)?;
        let fields: Value = serde_json::from_str(sanitized.preview_json())
            .map_err(|_| LocalAgentError::InvalidProtocol)?;
        let stdin = serde_json::to_string_pretty(&serde_json::json!({
            "instructions": fields["instructions"],
            "selected_context": fields["input"],
        }))
        .map_err(|_| LocalAgentError::InvalidProtocol)?;
        let workspace = match &directory {
            None => serde_json::json!({
                "mode":"isolated",
                "description":"fresh empty temporary directory; no existing project is exposed",
            }),
            Some(directory) => serde_json::json!({
                "mode":"selected",
                "selected_path":directory.selected_path(),
                "canonical_path":directory.canonical_path(),
                "directory_authority":"CLI may inspect project metadata in this explicitly selected directory; metadata contents are not approved model context",
                "inference_metadata":"the selected directory path and CLI-generated directory metadata may be sent to the inference service",
                "context":"no automatic AGENTS.md, CLAUDE.md, skills or project-file context; only the selected stdin below is approved",
                "configuration":"project configuration cannot enable hooks, MCP, tools or inherited credentials; home, environment and temporary data remain isolated",
            }),
        };
        let preview = serde_json::to_string_pretty(&serde_json::json!({
            "kind": self.kind.label(),
            "executable": self.executable,
            "scratch_parent": self.scratch_parent,
            "workspace": workspace,
            "inference_endpoint": self.endpoint,
            "model": self.model,
            "mode": "Ask only; no CLI tools, installed hooks/plugins, skills or MCP; fixed Claude built-in metadata admitted",
            "credential": "explicit ephemeral API credential; subscription login not reused",
            "credential_environment_reference": self.credential_environment_reference,
            "codex_permissions": if directory.is_some() {"root deny; minimal runtime paths and reviewed directory read; command network disabled"}
                else {"root deny; minimal runtime paths read; isolated workspace read; command network disabled"},
            "timeout_ms": self.limits.timeout.as_millis(),
            "combined_output_bytes": self.limits.output_bytes,
            "jsonl_line_bytes": self.limits.line_bytes,
            "answer_bytes": self.limits.answer_bytes,
            "maximum_frames": self.limits.frames,
            "stdin": serde_json::from_str::<Value>(&stdin).map_err(|_| LocalAgentError::InvalidProtocol)?,
        }))
        .map_err(|_| LocalAgentError::InvalidProtocol)?;
        Ok(PreparedLocalAsk {
            config: self.clone(),
            stdin: Zeroizing::new(stdin),
            preview: Zeroizing::new(preview),
            report: sanitized.redaction_report(),
            directory,
        })
    }
}
