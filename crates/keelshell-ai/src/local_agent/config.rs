use std::{path::PathBuf, time::Duration};

use url::Url;

use super::LocalAgentError;
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

    pub(super) fn checked_version(self) -> LocalAgentVersion {
        match self {
            Self::Codex => LocalAgentVersion(0, 160, 0),
            Self::ClaudeCode => LocalAgentVersion(2, 1, 285),
        }
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
        })
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
