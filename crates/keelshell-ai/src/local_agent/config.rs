use std::{path::PathBuf, time::Duration};

use url::Url;

use super::{LocalAgentError, LocalAgentWorkingDirectory, ValidatedLocalAgentDirectory};
use crate::ProviderConfig;

/// The installed official CLI adapter selected by the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAgentKind {
    /// Codex CLI's noninteractive exec JSONL protocol.
    Codex,
    /// Claude Code's bare, noninteractive stream-json protocol.
    ClaudeCode,
}

impl LocalAgentKind {
    /// Stable display name of the selected adapter.
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex CLI",
            Self::ClaudeCode => "Claude Code",
        }
    }

    pub(super) fn checked_versions(self) -> &'static [LocalAgentVersion] {
        match self {
            Self::Codex => &[LocalAgentVersion(0, 160, 0), LocalAgentVersion(0, 160, 1)],
            Self::ClaudeCode => &[LocalAgentVersion(2, 1, 285)],
        }
    }

    pub(super) fn supports_version(self, version: LocalAgentVersion) -> bool {
        self.checked_versions().contains(&version)
    }

    pub(super) fn supports_version_text(self, version: &str) -> bool {
        self.checked_versions()
            .iter()
            .any(|checked| checked.to_string() == version)
    }
}

/// A strictly parsed three-component installed CLI version, without raw output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAgentVersion(pub(super) u32, pub(super) u32, pub(super) u32);

impl LocalAgentVersion {
    /// Major version component.
    pub fn major(self) -> u32 {
        self.0
    }

    /// Minor version component.
    pub fn minor(self) -> u32 {
        self.1
    }

    /// Patch version component.
    pub fn patch(self) -> u32 {
        self.2
    }
}

impl std::fmt::Display for LocalAgentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Per-invocation total deadline and strict stdout/stderr/JSONL bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAgentLimits {
    pub(super) timeout: Duration,
    pub(super) output_bytes: usize,
    pub(super) line_bytes: usize,
    pub(super) answer_bytes: usize,
    pub(super) frames: usize,
}

impl Default for LocalAgentLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            output_bytes: 2 * 1024 * 1024,
            line_bytes: 256 * 1024,
            answer_bytes: 1024 * 1024,
            frames: 512,
        }
    }
}

impl LocalAgentLimits {
    /// Validate a nonzero deadline of at most 300 seconds, 1 KiB–8 MiB combined
    /// output, 1–1 MiB per line/answer within output, and 1–4096 frames.
    pub fn new(
        timeout: Duration,
        output_bytes: usize,
        line_bytes: usize,
        answer_bytes: usize,
        frames: usize,
    ) -> Result<Self, LocalAgentError> {
        if timeout.is_zero()
            || timeout > Duration::from_secs(300)
            || !(1024..=8 * 1024 * 1024).contains(&output_bytes)
            || !(1..=1024 * 1024).contains(&line_bytes)
            || line_bytes > output_bytes
            || !(1..=1024 * 1024).contains(&answer_bytes)
            || answer_bytes > output_bytes
            || !(1..=4096).contains(&frames)
        {
            return Err(LocalAgentError::InvalidLimits);
        }
        Ok(Self {
            timeout,
            output_bytes,
            line_bytes,
            answer_bytes,
            frames,
        })
    }

    /// Build user-facing Ask budgets while retaining fixed protocol safeguards.
    ///
    /// The JSONL line ceiling remains 256 KiB, reduced when total output is
    /// smaller; the 512-frame ceiling is unchanged. Answer bytes are UTF-8 bytes,
    /// and combined output also includes JSON framing and discarded stderr.
    pub fn for_ask(
        timeout: Duration,
        answer_bytes: usize,
        output_bytes: usize,
    ) -> Result<Self, LocalAgentError> {
        Self::new(
            timeout,
            output_bytes,
            output_bytes.min(256 * 1024),
            answer_bytes,
            512,
        )
    }

    /// Total deadline, including version/capability admission and inference.
    pub fn timeout(self) -> Duration {
        self.timeout
    }

    /// Combined stdout/stderr limit; stderr is counted and discarded.
    pub fn output_bytes(self) -> usize {
        self.output_bytes
    }

    /// Maximum bytes in a single complete JSONL line.
    pub fn line_bytes(self) -> usize {
        self.line_bytes
    }

    /// Maximum UTF-8 bytes in the complete assistant answer.
    pub fn answer_bytes(self) -> usize {
        self.answer_bytes
    }

    /// Maximum number of protocol frames.
    pub fn frames(self) -> usize {
        self.frames
    }
}

/// Non-secret CLI configuration. Credentials are separate and never serialized.
///
/// `scratch_parent` is only the location for a newly created, owned temporary
/// tree; it is never handed to the CLI as an existing project. Choosing an
/// executable is a trust decision: process containment is not a sandbox around
/// the CLI itself. The adapter refuses Windows `.bat`/`.cmd` launchers.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalAgentConfig {
    pub(super) kind: LocalAgentKind,
    pub(super) executable: PathBuf,
    pub(super) scratch_parent: PathBuf,
    pub(super) model: String,
    pub(super) endpoint: String,
    pub(super) limits: LocalAgentLimits,
    pub(super) credential_environment_reference: Option<String>,
    pub(super) working_directory: LocalAgentWorkingDirectory,
    pub(super) directory_launcher: Option<PathBuf>,
}

impl LocalAgentConfig {
    /// Select absolute native executable and scratch parent paths plus a model.
    /// Does not read files; executable existence is checked during probing.
    pub fn new(
        kind: LocalAgentKind,
        executable: impl Into<PathBuf>,
        scratch_parent: impl Into<PathBuf>,
        model: impl Into<String>,
    ) -> Result<Self, LocalAgentError> {
        let executable = executable.into();
        let scratch_parent = scratch_parent.into();
        if !executable.is_absolute() || !scratch_parent.is_absolute() {
            return Err(LocalAgentError::InvalidConfiguration);
        }
        if executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd"))
        {
            return Err(LocalAgentError::UnsupportedExecutable);
        }
        let model = model.into();
        ProviderConfig::new("https://api.openai.com/v1/responses", &model)
            .map_err(LocalAgentError::Context)?;
        Ok(Self {
            kind,
            executable,
            scratch_parent,
            model,
            endpoint: match kind {
                LocalAgentKind::Codex => "https://api.openai.com/v1",
                LocalAgentKind::ClaudeCode => "https://api.anthropic.com",
            }
            .to_owned(),
            limits: LocalAgentLimits::default(),
            credential_environment_reference: None,
            working_directory: LocalAgentWorkingDirectory::Isolated,
            directory_launcher: None,
        })
    }

    /// Bind the trusted same-application launcher for selected Unix directories.
    ///
    /// The executable must call `run_local_agent_directory_launcher` before any
    /// runtime or UI initialization. This is an application integration setting,
    /// never a user-selected CLI, shell command or arbitrary argument template.
    /// Defaults and Windows do not use a launcher.
    pub fn with_directory_launcher(
        mut self,
        executable: impl Into<PathBuf>,
    ) -> Result<Self, LocalAgentError> {
        let executable = executable.into();
        if !executable.is_absolute()
            || executable
                .to_str()
                .is_none_or(|path| path.len() > 4096 || path.chars().any(char::is_control))
        {
            return Err(LocalAgentError::DirectoryUnsupported);
        }
        self.directory_launcher = Some(executable);
        Ok(self)
    }

    /// Select an explicit compatible inference base URL; it is included in review.
    /// HTTPS or loopback HTTP only, without URL credentials, query or fragment.
    /// The adapter does not probe or contact this endpoint during CLI detection.
    pub fn with_inference_endpoint(mut self, endpoint: &str) -> Result<Self, LocalAgentError> {
        let url = Url::parse(endpoint).map_err(|_| LocalAgentError::InvalidEndpoint)?;
        ProviderConfig::new(endpoint, &self.model).map_err(|_| LocalAgentError::InvalidEndpoint)?;
        if url.query().is_some() || url.fragment().is_some() {
            return Err(LocalAgentError::InvalidEndpoint);
        }
        self.endpoint = url.as_str().trim_end_matches('/').to_owned();
        Ok(self)
    }

    /// Set already validated deadline and output bounds.
    pub fn with_limits(mut self, limits: LocalAgentLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Select a directory mode without performing filesystem I/O.
    ///
    /// A selected directory is not an environment/configuration/tool grant. It
    /// requires `prepare_checked` and complete human review before an Ask can
    /// run. The isolated default continues to use a fresh owned empty directory.
    pub fn with_working_directory(
        mut self,
        directory: LocalAgentWorkingDirectory,
    ) -> Result<Self, LocalAgentError> {
        directory.validate_metadata()?;
        self.working_directory = directory;
        Ok(self)
    }

    /// The exact directory mode captured by this immutable configuration.
    pub fn working_directory(&self) -> &LocalAgentWorkingDirectory {
        &self.working_directory
    }

    /// Validate an explicitly selected directory on a bounded background worker.
    ///
    /// This reads directory identity and, for Codex, at most 64 KiB of regular
    /// project configuration solely for syntax/identity checks. It does not
    /// start a CLI or send context. Cancellation revokes the returned authority;
    /// late filesystem completions are dropped. OS filesystem calls themselves
    /// cannot be interrupted, so callers must also reject stale UI revisions.
    pub async fn validate_working_directory(
        &self,
        cancellation: &crate::RequestCancellation,
    ) -> Result<Option<ValidatedLocalAgentDirectory>, LocalAgentError> {
        self.working_directory
            .validate_directory(self.kind, cancellation)
            .await
    }

    /// Add an audit-only credential source name to the immutable review.
    ///
    /// This never reads the environment and never forwards that variable name
    /// to the CLI. The application must explicitly load and freeze its value.
    /// Names use the existing secret-reference ASCII identifier grammar.
    pub fn with_credential_environment_reference(
        mut self,
        name: Option<&str>,
    ) -> Result<Self, LocalAgentError> {
        if name.is_some_and(|name| {
            name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        }) {
            return Err(LocalAgentError::InvalidConfiguration);
        }
        self.credential_environment_reference = name.map(str::to_owned);
        Ok(self)
    }

    pub(super) fn validate_context_metadata(
        &self,
        secrets: &[&str],
    ) -> Result<(), LocalAgentError> {
        let guard = crate::RequestOptions::default()
            .with_context_secrets(secrets)
            .map_err(LocalAgentError::Context)?;
        for text in [
            self.executable.to_string_lossy().as_ref(),
            self.scratch_parent.to_string_lossy().as_ref(),
            &self.model,
            &self.endpoint,
            self.credential_environment_reference
                .as_deref()
                .unwrap_or(""),
        ] {
            guard
                .validate_metadata_text(text)
                .map_err(LocalAgentError::Context)?;
        }
        if let LocalAgentWorkingDirectory::Selected(path) = &self.working_directory {
            guard
                .validate_metadata_text(path.to_string_lossy().as_ref())
                .map_err(LocalAgentError::Context)?;
        }
        Ok(())
    }

    /// Selected CLI protocol adapter.
    pub fn kind(&self) -> LocalAgentKind {
        self.kind
    }

    /// Explicit executable path, never a shell command string.
    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    /// Parent in which owned, fresh temporary run directories are created.
    pub fn scratch_parent(&self) -> &std::path::Path {
        &self.scratch_parent
    }

    /// Explicit inference service base URL shown during review.
    pub fn inference_endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Selected model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Validated deadline and stream limits.
    pub fn limits(&self) -> LocalAgentLimits {
        self.limits
    }
}

impl std::fmt::Debug for LocalAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalAgentConfig")
            .field("kind", &self.kind)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn environment_reference_is_audit_only_validated_and_hidden_from_debug() {
        let root = std::env::temp_dir();
        let config = LocalAgentConfig::new(
            LocalAgentKind::Codex,
            root.join("unused-cli"),
            &root,
            "model",
        )
        .unwrap();
        for invalid in ["", "KEY=value", "9KEY", "KEY\n", "éKEY"] {
            assert!(
                config
                    .clone()
                    .with_credential_environment_reference(Some(invalid))
                    .is_err()
            );
        }
        let reviewed = config
            .with_credential_environment_reference(Some("KEELSHELL_IMPORT_ONLY_KEY"))
            .unwrap();
        let preview = reviewed
            .prepare(
                crate::ContextDraft::new("Explain selected output"),
                &[],
                8192,
            )
            .unwrap();
        assert!(preview.preview_json().contains("KEELSHELL_IMPORT_ONLY_KEY"));
        assert!(
            !preview
                .preview_stdin()
                .contains("KEELSHELL_IMPORT_ONLY_KEY")
        );
        assert!(!format!("{reviewed:?}").contains("KEELSHELL_IMPORT_ONLY_KEY"));
        assert!(
            reviewed
                .prepare(
                    crate::ContextDraft::new("Explain output"),
                    &["KEELSHELL_IMPORT_ONLY_KEY"],
                    8192
                )
                .is_err()
        );
    }

    #[tokio::test]
    async fn actual_send_credential_cannot_be_hidden_in_audit_metadata_before_any_spawn() {
        let root = std::env::temp_dir();
        let config = LocalAgentConfig::new(
            LocalAgentKind::Codex,
            root.join("nonexistent-native-cli"),
            &root,
            "model",
        )
        .unwrap()
        .with_credential_environment_reference(Some("AUDIT_KEY_NAME"))
        .unwrap();
        let request = config
            .prepare(crate::ContextDraft::new("Explain output"), &[], 8192)
            .unwrap();
        let result = super::super::LocalAgentClient
            .ask(
                request.approve(),
                super::super::LocalAgentCredential::new("AUDIT_KEY_NAME").unwrap(),
                &crate::RequestCancellation::new(),
            )
            .await;
        assert_eq!(result.err(), Some(LocalAgentError::CredentialInContext));
    }

    #[test]
    fn user_ask_limits_keep_defaults_and_fixed_protocol_guards() {
        assert_eq!(
            LocalAgentLimits::for_ask(Duration::from_secs(120), 1024 * 1024, 2048 * 1024).unwrap(),
            LocalAgentLimits::default()
        );
        let low = LocalAgentLimits::for_ask(Duration::from_secs(1), 1024, 1024).unwrap();
        assert_eq!(
            (
                low.timeout(),
                low.answer_bytes(),
                low.output_bytes(),
                low.line_bytes(),
                low.frames()
            ),
            (Duration::from_secs(1), 1024, 1024, 1024, 512)
        );
        let high =
            LocalAgentLimits::for_ask(Duration::from_secs(300), 1024 * 1024, 8192 * 1024).unwrap();
        assert_eq!((high.line_bytes(), high.frames()), (256 * 1024, 512));
        assert_eq!(
            LocalAgentLimits::for_ask(Duration::from_secs(1), 2048, 1024),
            Err(LocalAgentError::InvalidLimits)
        );
    }

    #[test]
    fn prepared_limits_are_immutable_even_when_a_new_config_changes() {
        let root = std::env::temp_dir();
        let original = LocalAgentConfig::new(
            LocalAgentKind::Codex,
            root.join("fixture-cli"),
            root,
            "model",
        )
        .unwrap();
        let review = original
            .prepare(crate::ContextDraft::new("explicit"), &[], 8192)
            .unwrap();
        let changed = original.with_limits(
            LocalAgentLimits::for_ask(Duration::from_secs(27), 3 * 1024, 19 * 1024).unwrap(),
        );
        assert_eq!(review.config().limits(), LocalAgentLimits::default());
        assert_eq!(changed.limits().timeout(), Duration::from_secs(27));
        let preview: serde_json::Value = serde_json::from_str(review.preview_json()).unwrap();
        assert_eq!(preview["timeout_ms"], 120000);
        assert_eq!(preview["answer_bytes"], 1024 * 1024);
        assert_eq!(preview["combined_output_bytes"], 2 * 1024 * 1024);
    }

    #[test]
    fn configuration_rejects_shell_launchers_and_unbounded_limits() {
        let root = std::env::temp_dir();
        assert_eq!(
            LocalAgentConfig::new(LocalAgentKind::Codex, root.join("codex.cmd"), root, "model"),
            Err(LocalAgentError::UnsupportedExecutable),
        );
        assert_eq!(
            LocalAgentLimits::new(Duration::from_secs(301), 1024, 128, 128, 2),
            Err(LocalAgentError::InvalidLimits),
        );
        assert_eq!(
            LocalAgentLimits::new(Duration::from_secs(1), 1024, 2048, 128, 2),
            Err(LocalAgentError::InvalidLimits),
        );
    }

    #[test]
    fn review_reuses_explicit_redaction_and_binds_endpoint_and_model() {
        let root = std::env::temp_dir();
        let config = LocalAgentConfig::new(
            LocalAgentKind::ClaudeCode,
            root.join("claude"),
            root,
            "chosen-model",
        )
        .unwrap()
        .with_inference_endpoint("http://127.0.0.1:9876")
        .unwrap();
        let prepared = config
            .prepare(
                crate::ContextDraft::new("Explain chosen-private-value")
                    .add_selection("selection", "chosen-private-value failure"),
                &["chosen-private-value"],
                8192,
            )
            .unwrap();
        assert!(!prepared.preview_json().contains("chosen-private-value"));
        assert!(prepared.preview_json().contains("http://127.0.0.1:9876"));
        assert!(prepared.preview_json().contains("chosen-model"));
        assert!(prepared.redaction_report().total_redactions() >= 2);
        assert!(!format!("{prepared:?}").contains("Explain"));
    }

    #[test]
    fn endpoint_refuses_credentials_and_public_plaintext() {
        let root = std::env::temp_dir();
        let config =
            LocalAgentConfig::new(LocalAgentKind::Codex, root.join("codex"), root, "model")
                .unwrap();
        for endpoint in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com?q=secret",
        ] {
            assert_eq!(
                config.clone().with_inference_endpoint(endpoint),
                Err(LocalAgentError::InvalidEndpoint),
            );
        }
    }
}
