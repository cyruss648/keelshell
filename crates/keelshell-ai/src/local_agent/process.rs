use std::{
    io::ErrorKind,
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tempfile::TempDir;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::Instant,
};
use zeroize::Zeroizing;

use super::{
    ApprovedLocalAsk, LocalAgentConfig, LocalAgentCredential, LocalAgentError, LocalAgentKind,
    LocalAgentLimits, LocalAgentReply, LocalAgentVersion, LocalAskProgress, LocalAskStage,
    protocol::AnswerStream,
};
use crate::{Redactor, RequestCancellation, provider::payload_contains_secret};

const CLEANUP_DEADLINE: Duration = Duration::from_secs(3);
const PROBE_DEADLINE: Duration = Duration::from_secs(10);
const PROBE_BYTES: usize = 64 * 1024;
const STDERR_BYTES: usize = 64 * 1024;
const APPROVAL_LIFETIME: Duration = Duration::from_secs(300);

// Each checked Codex feature must be known and effectively disabled. The
// unified_exec flag is deliberately absent: 0.160.0 reports it true even when
// disabled; shell_tool=false is the admission gate, backed by the recorded
// controlled-wire test showing no tools in the actual Responses request.
const DISABLED_CODEX_FEATURES: &[&str] = &[
    "hooks",
    "shell_tool",
    "view_image",
    "image_generation",
    "plugins",
    "apps",
    "multi_agent",
    "js_repl",
    "code_mode",
    "browser_use",
    "computer_use",
    "skill_search",
    "memories",
    "goals",
    "remote_control",
    "shell_snapshot",
    "skill_mcp_dependency_install",
    "remote_plugin",
];

/// The installed version and Ask policy capability observed without inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAgentProbe {
    kind: LocalAgentKind,
    version: LocalAgentVersion,
}

impl LocalAgentProbe {
    /// CLI adapter for which version, arguments and effective flags were checked.
    pub fn kind(self) -> LocalAgentKind {
        self.kind
    }

    /// Installed version, restricted to the explicitly checked compatibility set.
    pub fn version(self) -> LocalAgentVersion {
        self.version
    }
}

/// A stateless, Ask-only CLI worker. No shell, PTY or existing project is opened.
///
/// Run these async methods on a background Tokio executor, never directly on
/// the GPUI thread. There is no automatic retry, resume, CLI install or login.
/// stdout is bounded protocol data; stderr is drained, counted and discarded.
/// Every terminal outcome kills the owned Unix process group or Windows Job
/// Object and awaits bounded cleanup before removing its scratch tree.
///
/// Containment is not a sandbox around a malicious executable, and POSIX
/// descendants which deliberately detach from the process group can escape it.
/// Ordinary contained descendants are covered by the real-process tests. Drop
/// of an in-flight future issues a best-effort group/job kill; callers should
/// cancel with the supplied signal and await the result for a cleanup receipt.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalAgentClient;

impl LocalAgentClient {
    /// Check the selected executable's version/help and effective Ask capability.
    ///
    /// The isolated probe inherits no credential/configuration environment and
    /// never invokes a model or authentication command. Existence, incompatibility,
    /// deadline and cleanup failures are reported as typed errors.
    pub async fn probe(
        &self,
        config: &LocalAgentConfig,
        cancellation: &RequestCancellation,
    ) -> Result<LocalAgentProbe, LocalAgentError> {
        check_cancelled(cancellation)?;
        let scratch = Scratch::create(config).await?;
        let result = probe_in(
            config,
            &scratch,
            Instant::now() + PROBE_DEADLINE,
            cancellation,
        )
        .await;
        scratch.close().await?;
        result
    }

    /// Invoke one exact, single-use human-approved request with an ephemeral key.
    ///
    /// The deadline includes fresh version/capability admission. Input never
    /// enters argv or a shell. The known inference key must not be in reviewed
    /// context, and a returned literal copy of it is redacted from the answer.
    /// A successful process exit alone is insufficient: the stream must contain
    /// a complete assistant message plus matching final success receipt.
    pub async fn ask(
        &self,
        approved: ApprovedLocalAsk,
        credential: LocalAgentCredential,
        cancellation: &RequestCancellation,
    ) -> Result<LocalAgentReply, LocalAgentError> {
        self.ask_inner(approved, credential, cancellation, None)
            .await
    }

    /// Invoke a reviewed Ask while reporting bounded, nonblocking lifecycle facts.
    ///
    /// The producer belongs to this request and is consumed here. Dropping or
    /// neglecting its receiver never changes protocol validation, cancellation,
    /// or cleanup. Stages contain no supplier output or request content; only
    /// this method's final result establishes success after all cleanup gates.
    pub async fn ask_with_progress(
        &self,
        approved: ApprovedLocalAsk,
        credential: LocalAgentCredential,
        cancellation: &RequestCancellation,
        progress: LocalAskProgress,
    ) -> Result<LocalAgentReply, LocalAgentError> {
        self.ask_inner(approved, credential, cancellation, Some(progress))
            .await
    }

    async fn ask_inner(
        &self,
        approved: ApprovedLocalAsk,
        credential: LocalAgentCredential,
        cancellation: &RequestCancellation,
        progress: Option<LocalAskProgress>,
    ) -> Result<LocalAgentReply, LocalAgentError> {
        if approved.approved_at.elapsed() > APPROVAL_LIFETIME {
            return Err(LocalAgentError::ReviewExpired);
        }
        check_cancelled(cancellation)?;
        if payload_contains_secret(&approved.prepared.stdin, credential.0.as_str()) {
            return Err(LocalAgentError::CredentialInContext);
        }
        let prepared = approved.prepared;
        let deadline = Instant::now() + prepared.config.limits.timeout;
        let scratch = Scratch::create(&prepared.config).await?;
        observe(progress.as_ref(), LocalAskStage::WorkspaceReady);
        let result = async {
            observe(progress.as_ref(), LocalAskStage::CheckingCli);
            let probe = probe_in(&prepared.config, &scratch, deadline, cancellation).await?;
            observe(progress.as_ref(), LocalAskStage::CliAdmitted);
            check_cancelled(cancellation)?;
            let mut command = base_command(&prepared.config, &scratch);
            match prepared.config.kind {
                LocalAgentKind::Codex => {
                    codex_policy(&mut command, &prepared.config);
                    command.args([
                        "exec",
                        "--json",
                        "--ephemeral",
                        "--ignore-user-config",
                        "--ignore-rules",
                        "--skip-git-repo-check",
                        "--color",
                        "never",
                        "--model",
                        &prepared.config.model,
                        "-",
                    ]);
                    command.env("KEELSHELL_LOCAL_AGENT_TOKEN", credential.0.as_str());
                }
                LocalAgentKind::ClaudeCode => {
                    command.args([
                        "--print",
                        "--bare",
                        "--output-format",
                        "stream-json",
                        "--verbose",
                        "--tools",
                        "",
                        "--disallowedTools",
                        "*",
                        "--strict-mcp-config",
                        "--mcp-config",
                        "{\"mcpServers\":{}}",
                        "--disable-slash-commands",
                        "--setting-sources",
                        "",
                        "--settings",
                        "{\"disableAllHooks\":true}",
                        "--permission-mode",
                        "default",
                        "--permission-prompts",
                        "none",
                        "--no-session-persistence",
                        "--max-turns",
                        "1",
                        "--model",
                        &prepared.config.model,
                    ]);
                    command.env("ANTHROPIC_API_KEY", credential.0.as_str());
                    command.env("ANTHROPIC_BASE_URL", &prepared.config.endpoint);
                }
            }
            let output = run_command(
                command,
                prepared.stdin.as_bytes(),
                OutputMode::Protocol(prepared.config.kind),
                prepared.config.limits,
                deadline,
                cancellation,
                progress.as_ref(),
            )
            .await?;
            match output {
                RunOutput::Answer(text, frames) => {
                    let (text, _) = Redactor::new(&[credential.0.as_str()]).redact(&text);
                    Ok(LocalAgentReply {
                        text: Zeroizing::new(text),
                        version: probe.version,
                        frames,
                    })
                }
                RunOutput::Text(_) => Err(LocalAgentError::InvalidProtocol),
                RunOutput::Incomplete(error) => Err(error),
            }
        }
        .await;
        observe(progress.as_ref(), LocalAskStage::Finalizing);
        scratch.close().await?;
        result
    }
}

struct Scratch {
    tree: TempDir,
    home: PathBuf,
    workspace: PathBuf,
    temporary: PathBuf,
}

impl Scratch {
    async fn create(config: &LocalAgentConfig) -> Result<Self, LocalAgentError> {
        let metadata = tokio::fs::metadata(&config.executable)
            .await
            .map_err(|_| LocalAgentError::SpawnFailed)?;
        if !metadata.is_file() {
            return Err(LocalAgentError::InvalidConfiguration);
        }
        let parent = config.scratch_parent.clone();
        tokio::task::spawn_blocking(move || {
            let tree = tempfile::Builder::new()
                .prefix("keelshell-agent-")
                .tempdir_in(parent)
                .map_err(|_| LocalAgentError::ScratchFailed)?;
            let home = tree.path().join("home");
            let workspace = tree.path().join("workspace");
            let temporary = tree.path().join("tmp");
            for directory in [
                home.clone(),
                home.join("codex"),
                home.join("claude"),
                home.join("appdata"),
                home.join("localappdata"),
                home.join("cache"),
                home.join("config"),
                workspace.clone(),
                temporary.clone(),
            ] {
                std::fs::create_dir(&directory).map_err(|_| LocalAgentError::ScratchFailed)?;
            }
            // Stop ancestor project/config discovery at this owned empty root.
            std::fs::write(workspace.join(".keelshell-isolated-workspace"), [])
                .map_err(|_| LocalAgentError::ScratchFailed)?;
            Ok(Self {
                tree,
                home,
                workspace,
                temporary,
            })
        })
        .await
        .map_err(|_| LocalAgentError::ScratchFailed)?
    }

    async fn close(self) -> Result<(), LocalAgentError> {
        tokio::task::spawn_blocking(move || self.tree.close())
            .await
            .map_err(|_| LocalAgentError::ScratchFailed)?
            .map_err(|_| LocalAgentError::ScratchFailed)
    }
}

fn base_command(config: &LocalAgentConfig, scratch: &Scratch) -> Command {
    let mut command = Command::new(&config.executable);
    command
        .env_clear()
        .current_dir(&scratch.workspace)
        .env("HOME", &scratch.home)
        .env("USERPROFILE", &scratch.home)
        .env("CODEX_HOME", scratch.home.join("codex"))
        .env("CLAUDE_CONFIG_DIR", scratch.home.join("claude"))
        .env("XDG_CONFIG_HOME", scratch.home.join("config"))
        .env("XDG_CACHE_HOME", scratch.home.join("cache"))
        .env("APPDATA", scratch.home.join("appdata"))
        .env("LOCALAPPDATA", scratch.home.join("localappdata"))
        .env("TMPDIR", &scratch.temporary)
        .env("TEMP", &scratch.temporary)
        .env("TMP", &scratch.temporary)
        .env("LANG", "en_US.UTF-8")
        .env("LC_ALL", "en_US.UTF-8")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY", "1")
        .env("CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL", "1")
        .env("DISABLE_TELEMETRY", "1")
        .env("DISABLE_ERROR_REPORTING", "1")
        .env("DISABLE_UPDATES", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        let system_root = PathBuf::from(system_root);
        command.env("SystemRoot", &system_root);
        command.env("WINDIR", &system_root);
        if let Ok(path) = std::env::join_paths([system_root.join("System32"), system_root]) {
            command.env("PATH", path);
        }
    }
    command
}

fn config_arg(command: &mut Command, value: &str) {
    command.args(["-c", value]);
}

fn codex_policy(command: &mut Command, config: &LocalAgentConfig) {
    for feature in DISABLED_CODEX_FEATURES {
        command.args(["--disable", feature]);
    }
    for value in [
        "default_permissions=\"keelshell_ask\"",
        "permissions.keelshell_ask.filesystem={\":root\"=\"deny\",\":minimal\"=\"read\",\":workspace_roots\"=\"read\"}",
        "permissions.keelshell_ask.network.enabled=false",
        "approval_policy=\"on-request\"",
        "approvals_reviewer=\"user\"",
        "web_search=\"disabled\"",
        "mcp_servers={}",
        "plugins={}",
        "analytics.enabled=false",
        "feedback.enabled=false",
        "history.persistence=\"none\"",
        "project_doc_max_bytes=0",
        "project_root_markers=[\".keelshell-isolated-workspace\"]",
        "model_provider=\"keelshell_ask\"",
    ] {
        config_arg(command, value);
    }
    // Url::as_str() has already removed controls and encoded quotes. JSON string
    // escapes are valid TOML string escapes here, and this is one argv element.
    let endpoint = serde_json::json!(&config.endpoint).to_string();
    config_arg(
        command,
        &format!(
            "model_providers.keelshell_ask={{name=\"KeelShell Ask\",base_url={endpoint},env_key=\"KEELSHELL_LOCAL_AGENT_TOKEN\",wire_api=\"responses\",requires_openai_auth=false,supports_websockets=false}}"
        ),
    );
}

async fn probe_in(
    config: &LocalAgentConfig,
    scratch: &Scratch,
    deadline: Instant,
    cancellation: &RequestCancellation,
) -> Result<LocalAgentProbe, LocalAgentError> {
    let mut version_command = base_command(config, scratch);
    version_command.arg("--version");
    let version_bytes = capture(version_command, deadline, cancellation).await?;
    let version = parse_version(config.kind, &version_bytes)?;
    if version != config.kind.checked_version() {
        return Err(LocalAgentError::UnsupportedVersion);
    }
    let mut help_command = base_command(config, scratch);
    if config.kind == LocalAgentKind::Codex {
        help_command.arg("exec");
    }
    help_command.arg("--help");
    let help = capture(help_command, deadline, cancellation).await?;
    let help = std::str::from_utf8(&help).map_err(|_| LocalAgentError::InvalidProtocol)?;
    let required: &[&str] = match config.kind {
        LocalAgentKind::Codex => &[
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
        ],
        LocalAgentKind::ClaudeCode => &[
            "--bare",
            "--output-format",
            "--tools",
            "--disallowedTools",
            "--strict-mcp-config",
            "--mcp-config",
            "--disable-slash-commands",
            "--setting-sources",
            "--settings",
            "--permission-mode",
            "--permission-prompts",
            "--no-session-persistence",
        ],
    };
    if required.iter().any(|required| !help.contains(required)) {
        return Err(LocalAgentError::IsolationUnsupported);
    }
    if config.kind == LocalAgentKind::Codex {
        let mut features_command = base_command(config, scratch);
        codex_policy(&mut features_command, config);
        features_command.args(["features", "list"]);
        let features = capture(features_command, deadline, cancellation).await?;
        check_codex_features(&features)?;
    }
    Ok(LocalAgentProbe {
        kind: config.kind,
        version,
    })
}

fn parse_version(kind: LocalAgentKind, bytes: &[u8]) -> Result<LocalAgentVersion, LocalAgentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LocalAgentError::InvalidProtocol)?;
    let version = match kind {
        LocalAgentKind::Codex => text.trim().strip_prefix("codex-cli "),
        LocalAgentKind::ClaudeCode => text.trim().strip_suffix(" (Claude Code)"),
    }
    .ok_or(LocalAgentError::UnsupportedVersion)?;
    let mut fields = version.split('.');
    let mut next = || {
        fields.next().and_then(|field| {
            (!field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| field.parse::<u32>().ok())
                .flatten()
        })
    };
    let parsed = LocalAgentVersion(
        next().ok_or(LocalAgentError::UnsupportedVersion)?,
        next().ok_or(LocalAgentError::UnsupportedVersion)?,
        next().ok_or(LocalAgentError::UnsupportedVersion)?,
    );
    if fields.next().is_some() {
        return Err(LocalAgentError::UnsupportedVersion);
    }
    Ok(parsed)
}

fn check_codex_features(bytes: &[u8]) -> Result<(), LocalAgentError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LocalAgentError::IsolationUnsupported)?;
    for required in DISABLED_CODEX_FEATURES {
        let mut matches = text.lines().filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.first() == Some(required)).then(|| fields.last().copied())
        });
        if matches.next() != Some(Some("false")) || matches.next().is_some() {
            return Err(LocalAgentError::IsolationUnsupported);
        }
    }
    Ok(())
}

async fn capture(
    command: Command,
    deadline: Instant,
    cancellation: &RequestCancellation,
) -> Result<Zeroizing<Vec<u8>>, LocalAgentError> {
    let limits = LocalAgentLimits {
        timeout: PROBE_DEADLINE,
        output_bytes: PROBE_BYTES,
        line_bytes: PROBE_BYTES,
        answer_bytes: PROBE_BYTES,
        frames: 1,
    };
    match run_command(
        command,
        &[],
        OutputMode::Text,
        limits,
        deadline,
        cancellation,
        None,
    )
    .await?
    {
        RunOutput::Text(bytes) => Ok(bytes),
        RunOutput::Answer(_, _) => Err(LocalAgentError::InvalidProtocol),
        RunOutput::Incomplete(error) => Err(error),
    }
}

enum OutputMode {
    Text,
    Protocol(LocalAgentKind),
}

enum RunOutput {
    Text(Zeroizing<Vec<u8>>),
    Answer(Zeroizing<String>, usize),
    Incomplete(LocalAgentError),
}

struct OwnedChild {
    child: Box<dyn ChildWrapper>,
    cleaned: bool,
    #[cfg(windows)]
    observed_job: std::sync::Arc<win32job::Job>,
}

impl OwnedChild {
    fn spawn(command: Command) -> Result<Self, LocalAgentError> {
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        let observed_job = {
            let mut limits = win32job::ExtendedLimitInfo::default();
            // No BREAKAWAY_OK, SILENT_BREAKAWAY_OK or UI restrictions.
            limits.limit_kill_on_job_close();
            let job = std::sync::Arc::new(
                win32job::Job::create_with_limit_info(&limits)
                    .map_err(|_| LocalAgentError::SpawnFailed)?,
            );
            // All pre_spawn hooks run before creation. JobObject suspends the
            // child, then ordered wrap_child hooks assign outer -> inner jobs,
            // and only the inner JobObject hook resumes it.
            command.wrap(WindowsExitObserver(job.clone()));
            command.wrap(process_wrap::tokio::JobObject);
            job
        };
        command
            .spawn()
            .map(|child| Self {
                child,
                cleaned: false,
                #[cfg(windows)]
                observed_job,
            })
            .map_err(|_| LocalAgentError::SpawnFailed)
    }

    async fn cleanup(&mut self) -> Result<(), LocalAgentError> {
        if self.cleaned {
            return Ok(());
        }
        let deadline = Instant::now() + CLEANUP_DEADLINE;
        // Reap an already exited leader before signalling its group: on macOS
        // a zombie-only group can otherwise report EPERM rather than ESRCH.
        self.child
            .inner_mut()
            .try_wait()
            .map_err(|_| LocalAgentError::CleanupFailed)?;
        if let Err(error) = self.child.start_kill() {
            #[cfg(unix)]
            let zombie_race = error.raw_os_error() == Some(nix::errno::Errno::EPERM as i32);
            #[cfg(not(unix))]
            let zombie_race = false;
            if zombie_race {
                // Reap the leader, then re-signal. Only ESRCH is accepted as
                // absence; a second EPERM remains a typed cleanup failure.
                tokio::time::timeout_at(deadline, self.child.inner_mut().wait())
                    .await
                    .map_err(|_| LocalAgentError::CleanupFailed)?
                    .map_err(|_| LocalAgentError::CleanupFailed)?;
                if let Err(retry) = self.child.start_kill()
                    && !already_exited(&retry)
                {
                    return Err(LocalAgentError::CleanupFailed);
                }
            } else if !already_exited(&error) {
                return Err(LocalAgentError::CleanupFailed);
            }
        }
        tokio::time::timeout_at(deadline, self.child.wait())
            .await
            .map_err(|_| LocalAgentError::CleanupFailed)?
            .map_err(|_| LocalAgentError::CleanupFailed)?;
        #[cfg(windows)]
        loop {
            if Instant::now() >= deadline {
                return Err(LocalAgentError::CleanupFailed);
            }
            // process-wrap 10.0.1 may accept an arbitrary Job completion packet.
            // Confirm the independent outer job contains no live process IDs.
            let empty = self
                .observed_job
                .query_process_id_list()
                .map_err(|_| LocalAgentError::CleanupFailed)?
                .is_empty();
            if Instant::now() >= deadline {
                return Err(LocalAgentError::CleanupFailed);
            }
            if empty {
                break;
            }
            tokio::time::sleep_until((Instant::now() + Duration::from_millis(10)).min(deadline))
                .await;
        }
        self.cleaned = true;
        Ok(())
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsExitObserver(std::sync::Arc<win32job::Job>);

#[cfg(windows)]
impl process_wrap::tokio::CommandWrapper for WindowsExitObserver {
    fn wrap_child(
        &mut self,
        child: Box<dyn ChildWrapper>,
        _core: &CommandWrap,
    ) -> std::io::Result<Box<dyn ChildWrapper>> {
        use std::os::windows::io::AsRawHandle;
        let handle = child
            .process_handle()
            .ok_or_else(|| std::io::Error::from(ErrorKind::Unsupported))?;
        self.0
            .assign_process(handle.as_raw_handle() as isize)
            .map_err(std::io::Error::other)?;
        Ok(child)
    }
}

fn already_exited(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    if error.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32) {
        return true;
    }
    error.kind() == ErrorKind::NotFound
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = self.child.start_kill();
        }
    }
}

async fn run_command(
    command: Command,
    input: &[u8],
    mode: OutputMode,
    limits: LocalAgentLimits,
    deadline: Instant,
    cancellation: &RequestCancellation,
    progress: Option<&LocalAskProgress>,
) -> Result<RunOutput, LocalAgentError> {
    check_cancelled(cancellation)?;
    if Instant::now() >= deadline {
        return Err(LocalAgentError::Timeout);
    }
    let mut child = OwnedChild::spawn(command)?;
    observe(progress, LocalAskStage::ProcessStarted);
    let stdin = child
        .child
        .stdin()
        .take()
        .ok_or(LocalAgentError::PipeFailed)?;
    let stdout = child
        .child
        .stdout()
        .take()
        .ok_or(LocalAgentError::PipeFailed)?;
    let stderr = child
        .child
        .stderr()
        .take()
        .ok_or(LocalAgentError::PipeFailed)?;
    let budget = AtomicUsize::new(0);
    let result = {
        let operation = async {
            let write = async {
                let mut stdin = stdin;
                stdin
                    .write_all(input)
                    .await
                    .map_err(|_| LocalAgentError::PipeFailed)?;
                if !input.is_empty() {
                    stdin
                        .write_all(b"\n")
                        .await
                        .map_err(|_| LocalAgentError::PipeFailed)?;
                }
                stdin
                    .shutdown()
                    .await
                    .map_err(|_| LocalAgentError::PipeFailed)?;
                drop(stdin);
                observe(progress, LocalAskStage::InputDelivered);
                Ok::<_, LocalAgentError>(())
            };
            let pipes = async {
                let (_, output, _) = tokio::try_join!(
                    write,
                    read_stdout(stdout, mode, limits, &budget, progress),
                    discard_stderr(stderr, limits.output_bytes, &budget),
                )?;
                Ok::<_, LocalAgentError>(output)
            };
            tokio::pin!(pipes);
            // Wait for the leader concurrently with pipe draining. Descendants
            // may inherit those pipes; leader exit must trigger their stop
            // instead of waiting for EOF until the request deadline.
            let (status, output) = {
                let leader = child.child.inner_mut().wait();
                tokio::pin!(leader);
                tokio::select! {
                    biased;
                    status = &mut leader => (status.map_err(|_| LocalAgentError::ProcessFailed)?, None),
                    output = &mut pipes => {
                        let output = output?;
                        (leader.await.map_err(|_| LocalAgentError::ProcessFailed)?, Some(output))
                    },
                }
            };
            observe(progress, LocalAskStage::Finalizing);
            child.cleanup().await?;
            let stdout = match output {
                Some(output) => output,
                None => tokio::time::timeout(CLEANUP_DEADLINE, pipes)
                    .await
                    .map_err(|_| LocalAgentError::CleanupFailed)??,
            };
            if !status.success() {
                return Err(LocalAgentError::ProcessFailed);
            }
            match stdout {
                RunOutput::Incomplete(error) => Err(error),
                output => Ok(output),
            }
        };
        tokio::select! {
            biased;
            _ = observe_cancellation(cancellation) => Err(LocalAgentError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => Err(LocalAgentError::Timeout),
            result = operation => result,
        }
    };
    // Cleanup also runs after a successful leader exit: a child may have closed
    // its pipes and kept running. Never preserve those contained descendants.
    observe(progress, LocalAskStage::Finalizing);
    child.cleanup().await?;
    result
}

fn observe(progress: Option<&LocalAskProgress>, stage: LocalAskStage) {
    if let Some(progress) = progress {
        progress.observe(stage);
    }
}

fn add_bytes(budget: &AtomicUsize, bytes: usize, limit: usize) -> Result<(), LocalAgentError> {
    if budget
        .fetch_add(bytes, Ordering::Relaxed)
        .saturating_add(bytes)
        > limit
    {
        return Err(LocalAgentError::OutputTooLarge);
    }
    Ok(())
}

async fn read_stdout(
    mut reader: impl AsyncRead + Unpin,
    mode: OutputMode,
    limits: LocalAgentLimits,
    budget: &AtomicUsize,
    progress: Option<&LocalAskProgress>,
) -> Result<RunOutput, LocalAgentError> {
    let mut bytes = Zeroizing::new(Vec::new());
    // This buffer spans await points and would otherwise be copied through
    // nested join/select state machines on the caller's native thread stack.
    let mut chunk = vec![0_u8; 8192];
    let mut protocol = match mode {
        OutputMode::Text => None,
        OutputMode::Protocol(kind) => Some(AnswerStream::new(kind, limits)),
    };
    loop {
        let count = reader
            .read(chunk.as_mut_slice())
            .await
            .map_err(|_| LocalAgentError::PipeFailed)?;
        if count == 0 {
            break;
        }
        add_bytes(budget, count, limits.output_bytes)?;
        for byte in &chunk[..count] {
            if let Some(protocol) = &mut protocol {
                if *byte == b'\n' {
                    if bytes.last() == Some(&b'\r') {
                        bytes.pop();
                    }
                    protocol.frame(&bytes)?;
                    // A complete frame must pass every adapter invariant before
                    // its receipt becomes visible. This is still not an answer.
                    if protocol.started() {
                        observe(progress, LocalAskStage::ProtocolStarted);
                    }
                    if protocol.completed() {
                        observe(progress, LocalAskStage::ProtocolCompleted);
                    }
                    bytes.clear();
                } else {
                    if bytes.len() >= limits.line_bytes {
                        return Err(LocalAgentError::OutputTooLarge);
                    }
                    bytes.push(*byte);
                }
            } else {
                bytes.push(*byte);
            }
        }
    }
    match protocol {
        Some(protocol) => {
            // Newline termination distinguishes complete JSONL from a truncated
            // final write, even when that write happens to be valid JSON.
            if !bytes.is_empty() {
                return Ok(RunOutput::Incomplete(LocalAgentError::InvalidProtocol));
            }
            Ok(match protocol.finish() {
                Ok((answer, frames)) => RunOutput::Answer(answer, frames),
                Err(error) => RunOutput::Incomplete(error),
            })
        }
        None => Ok(RunOutput::Text(bytes)),
    }
}

async fn discard_stderr(
    mut reader: impl AsyncRead + Unpin,
    output_limit: usize,
    budget: &AtomicUsize,
) -> Result<(), LocalAgentError> {
    let mut discarded = 0usize;
    let mut chunk = vec![0_u8; 8192];
    loop {
        let count = reader
            .read(chunk.as_mut_slice())
            .await
            .map_err(|_| LocalAgentError::PipeFailed)?;
        if count == 0 {
            return Ok(());
        }
        add_bytes(budget, count, output_limit)?;
        discarded = discarded.saturating_add(count);
        if discarded > STDERR_BYTES {
            return Err(LocalAgentError::OutputTooLarge);
        }
    }
}

fn check_cancelled(cancellation: &RequestCancellation) -> Result<(), LocalAgentError> {
    if cancellation.is_cancelled() {
        Err(LocalAgentError::Cancelled)
    } else {
        Ok(())
    }
}

async fn observe_cancellation(cancellation: &RequestCancellation) {
    loop {
        if cancellation.is_cancelled() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn local_agent_futures_keep_pipe_buffers_on_the_heap() {
        fn pipe_size<F>(_: impl FnOnce(tokio::io::Empty, &'static AtomicUsize) -> F) -> usize {
            std::mem::size_of::<F>()
        }
        fn probe_size<F>(
            _: impl FnOnce(
                &'static LocalAgentClient,
                &'static LocalAgentConfig,
                &'static RequestCancellation,
            ) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        fn ask_size<F>(
            _: impl FnOnce(
                &'static LocalAgentClient,
                ApprovedLocalAsk,
                LocalAgentCredential,
                &'static RequestCancellation,
            ) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        // Measure the unpolled state machines without spawning a CLI or using
        // credentials. Nested select/join futures must not embed pipe buffers.
        let sizes = [
            (
                "stdout",
                pipe_size(|reader, budget| {
                    read_stdout(
                        reader,
                        OutputMode::Text,
                        LocalAgentLimits::default(),
                        budget,
                        None,
                    )
                }),
                4 * 1024,
            ),
            (
                "stderr",
                pipe_size(|reader, budget| discard_stderr(reader, 1024 * 1024, budget)),
                4 * 1024,
            ),
            (
                "probe",
                probe_size(|client, config, cancellation| client.probe(config, cancellation)),
                16 * 1024,
            ),
            (
                "ask",
                ask_size(|client, approved, credential, cancellation| {
                    client.ask(approved, credential, cancellation)
                }),
                16 * 1024,
            ),
        ];
        for (name, size, _) in sizes {
            eprintln!("local agent {name} future: {size} bytes");
        }
        for (name, size, limit) in sizes {
            assert!(size < limit, "local agent {name} future uses {size} bytes");
        }
    }

    #[test]
    fn version_parser_does_not_accept_prefix_guesses_or_future_versions() {
        assert_eq!(
            parse_version(LocalAgentKind::Codex, b"codex-cli 0.160.0\n").unwrap(),
            LocalAgentVersion(0, 160, 0)
        );
        assert_eq!(
            parse_version(LocalAgentKind::ClaudeCode, b"2.1.285 (Claude Code)\n").unwrap(),
            LocalAgentVersion(2, 1, 285)
        );
        for bytes in [
            b"codex-cli 0.160.0-private-data".as_slice(),
            b"0.160.0",
            b"codex-cli 0.160.0.1",
        ] {
            assert_eq!(
                parse_version(LocalAgentKind::Codex, bytes),
                Err(LocalAgentError::UnsupportedVersion)
            );
        }
    }

    #[test]
    fn effective_features_must_all_be_known_false_without_duplicates() {
        let features = DISABLED_CODEX_FEATURES
            .iter()
            .map(|name| format!("{name} stable false\n"))
            .collect::<String>();
        assert!(check_codex_features(features.as_bytes()).is_ok());
        assert_eq!(
            check_codex_features(
                features
                    .replace("hooks stable false", "hooks stable true")
                    .as_bytes()
            ),
            Err(LocalAgentError::IsolationUnsupported)
        );
        assert_eq!(
            check_codex_features(format!("{features}hooks stable false\n").as_bytes()),
            Err(LocalAgentError::IsolationUnsupported)
        );
    }

    #[tokio::test]
    async fn jsonl_reader_handles_split_utf8_and_requires_final_newline() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(64);
        let frames = "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n{\"type\":\"turn.started\"}\n{\"type\":\"item.completed\",\"item\":{\"id\":\"a\",\"type\":\"agent_message\",\"text\":\"中文完整\"}}\n{\"type\":\"turn.completed\",\"usage\":{}}\n";
        let write = tokio::spawn(async move {
            for byte in frames.as_bytes() {
                writer.write_all(&[*byte]).await.unwrap();
            }
        });
        let budget = AtomicUsize::new(0);
        let output = read_stdout(
            reader,
            OutputMode::Protocol(LocalAgentKind::Codex),
            LocalAgentLimits::default(),
            &budget,
            None,
        )
        .await
        .unwrap();
        write.await.unwrap();
        let RunOutput::Answer(answer, _) = output else {
            panic!("expected answer")
        };
        assert_eq!(answer.as_str(), "中文完整");
        let bytes = frames.trim_end().as_bytes();
        assert!(matches!(
            read_stdout(
                bytes,
                OutputMode::Protocol(LocalAgentKind::Codex),
                LocalAgentLimits::default(),
                &AtomicUsize::new(0),
                None,
            )
            .await
            .unwrap(),
            RunOutput::Incomplete(LocalAgentError::InvalidProtocol)
        ));
    }
}
