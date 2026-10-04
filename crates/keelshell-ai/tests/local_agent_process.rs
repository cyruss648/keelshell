//! Self-hosted process fixture: no installed supplier CLI, model or account.
//!
//! A custom harness lets the *same compiled test executable* act as a native
//! CLI fixture without shell scripts, platform runtimes or libtest stdout.
//! The default entry point runs assertions; owned copies handle fixed CLI argv.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use keelshell_ai::{
    ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentError,
    LocalAgentKind, LocalAgentLimits, RequestCancellation,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--fixture-descendant")
    {
        descendant();
    } else if std::env::current_exe()
        .unwrap()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .contains("agent-fixture")
    {
        fixture(&arguments);
    } else {
        fn future_size<F>(_: impl FnOnce() -> F) -> usize {
            std::mem::size_of::<F>()
        }
        let controller_size = future_size(integration_cases);
        eprintln!("local agent integration controller future: {controller_size} bytes");
        assert!(
            controller_size < 16 * 1024,
            "controller embeds oversized process futures: {controller_size} bytes"
        );
        if arguments
            .first()
            .is_some_and(|arg| arg == "--controller-small-stack")
        {
            // Exercise the identical controller without relying on the native
            // process main stack. The normal CI entry point remains unchanged.
            std::thread::Builder::new()
                .name("local-agent-small-stack".into())
                .stack_size(2 * 1024 * 1024)
                .spawn(|| {
                    tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(async {
                            tokio::time::timeout(Duration::from_secs(45), integration_cases())
                                .await
                                .expect("small-stack controller exceeded its overall deadline");
                        });
                })
                .unwrap()
                .join()
                .unwrap();
            return;
        }
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(integration_cases());
    }
}

struct Fixture {
    root: TempDir,
    executable: PathBuf,
    scratch: PathBuf,
    kind: LocalAgentKind,
}

impl Fixture {
    fn new(kind: LocalAgentKind, suffix: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let name = match kind {
            LocalAgentKind::Codex => "codex-agent-fixture",
            LocalAgentKind::ClaudeCode => "claude-agent-fixture",
        };
        let executable = root
            .path()
            .join(format!("{name}{suffix}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let scratch = root.path().join("scratch");
        std::fs::create_dir(&scratch).unwrap();
        Self {
            root,
            executable,
            scratch,
            kind,
        }
    }

    fn config(&self, timeout: Duration) -> LocalAgentConfig {
        LocalAgentConfig::new(
            self.kind,
            &self.executable,
            &self.scratch,
            "fixture-selected-model",
        )
        .unwrap()
        .with_limits(
            LocalAgentLimits::new(timeout, 1024 * 1024, 128 * 1024, 128 * 1024, 128).unwrap(),
        )
    }

    async fn ask(&self, question: &str) -> Result<String, LocalAgentError> {
        let review = self
            .config(Duration::from_secs(5))
            .prepare(
                ContextDraft::new(question).add_selection("explicit selection", "selected-only"),
                &[],
                8192,
            )
            .unwrap();
        LocalAgentClient
            .ask(review.approve(), credential(), &RequestCancellation::new())
            .await
            .map(|reply| reply.text().to_owned())
    }

    fn assert_clean(&self) {
        assert_eq!(
            std::fs::read_dir(&self.scratch).unwrap().count(),
            0,
            "all owned scratch trees removed"
        );
        assert_eq!(
            std::fs::read_dir(self.root.path()).unwrap().count(),
            2,
            "only controller executable/scratch remain"
        );
    }

    async fn descendant_port(&self) -> u16 {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            for entry in std::fs::read_dir(&self.scratch).unwrap() {
                let path = entry.unwrap().path().join("workspace/descendant-port");
                if let Ok(text) = std::fs::read_to_string(path) {
                    // The child may have created/truncated the file before its
                    // port write finishes. Keep the original readiness bound.
                    if let Ok(port) = text.parse() {
                        return port;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "fixture descendant did not start within bound"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

fn credential() -> LocalAgentCredential {
    LocalAgentCredential::new("fixture_ephemeral_token").unwrap()
}

async fn integration_cases() {
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        let fixture = Fixture::new(kind, "");
        let probe = LocalAgentClient
            .probe(
                &fixture.config(Duration::from_secs(5)),
                &RequestCancellation::new(),
            )
            .await
            .unwrap();
        assert_eq!(probe.kind(), kind);
        assert_eq!(
            fixture
                .ask("complete 中文 ' ; $(never-execute)")
                .await
                .unwrap(),
            "完整中文回答; selected-only"
        );
        fixture.assert_clean();

        for (question, expected) in [
            ("truncate", LocalAgentError::InvalidProtocol),
            ("late", LocalAgentError::InvalidProtocol),
            ("tool", LocalAgentError::UnexpectedOperation),
            ("malformed", LocalAgentError::InvalidProtocol),
            ("nonzero", LocalAgentError::ProcessFailed),
            ("stderr-secret", LocalAgentError::ProcessFailed),
            ("stderr-flood", LocalAgentError::OutputTooLarge),
            ("line-flood", LocalAgentError::OutputTooLarge),
        ] {
            let error = fixture.ask(question).await.unwrap_err();
            assert_eq!(error, expected, "{kind:?} {question}");
            assert!(!format!("{error:?} {error}").contains("private-fixture-secret"));
            fixture.assert_clean();
        }
        assert_eq!(fixture.ask("echo-credential").await.unwrap(), "[REDACTED]");

        let signal = RequestCancellation::new();
        signal.cancel();
        let review = fixture
            .config(Duration::from_secs(5))
            .prepare(ContextDraft::new("complete"), &[], 8192)
            .unwrap();
        assert_eq!(
            LocalAgentClient
                .ask(review.approve(), credential(), &signal)
                .await
                .err(),
            Some(LocalAgentError::Cancelled)
        );
        fixture.assert_clean();

        for cancel in [true, false] {
            let timeout = if cancel {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(2)
            };
            let review = fixture
                .config(timeout)
                .prepare(ContextDraft::new("wait-descendant"), &[], 8192)
                .unwrap();
            let signal = RequestCancellation::new();
            let observing = signal.clone();
            let task = tokio::spawn(async move {
                LocalAgentClient
                    .ask(review.approve(), credential(), &observing)
                    .await
            });
            let port = fixture.descendant_port().await;
            assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
            if cancel {
                signal.cancel();
            }
            let outcome = tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                outcome.err(),
                Some(if cancel {
                    LocalAgentError::Cancelled
                } else {
                    LocalAgentError::Timeout
                })
            );
            assert!(
                TcpStream::connect(("127.0.0.1", port)).is_err(),
                "contained descendant listener released"
            );
            fixture.assert_clean();
        }

        let answer = fixture.ask("leader-exits-descendant").await.unwrap();
        let port: u16 = answer.parse().unwrap();
        assert!(
            TcpStream::connect(("127.0.0.1", port)).is_err(),
            "successful leader exit also kills contained descendants"
        );
        fixture.assert_clean();

        let started = Instant::now();
        let answer = fixture
            .ask("leader-exits-descendant-inherits-pipes")
            .await
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "inherited pipes must not delay a completed leader to timeout"
        );
        assert!(TcpStream::connect(("127.0.0.1", answer.parse::<u16>().unwrap())).is_err());
        fixture.assert_clean();

        let review = fixture
            .config(Duration::from_secs(5))
            .prepare(ContextDraft::new("wait-descendant"), &[], 8192)
            .unwrap();
        let task = tokio::spawn(async move {
            LocalAgentClient
                .ask(review.approve(), credential(), &RequestCancellation::new())
                .await
        });
        let port = fixture.descendant_port().await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let deadline = Instant::now() + Duration::from_secs(2);
        while TcpStream::connect(("127.0.0.1", port)).is_ok() {
            assert!(
                Instant::now() < deadline,
                "dropping future left contained process alive"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        fixture.assert_clean();

        let unreviewed = fixture
            .config(Duration::from_secs(5))
            .prepare(ContextDraft::new("fixture_ephemeral_token"), &[], 8192)
            .unwrap();
        assert_eq!(
            LocalAgentClient
                .ask(
                    unreviewed.approve(),
                    credential(),
                    &RequestCancellation::new()
                )
                .await
                .err(),
            Some(LocalAgentError::CredentialInContext)
        );
        fixture.assert_clean();
    }
    // Known credentials must be rejected before any version/help/model spawn,
    // including the escaped nested JSON representation of selected_context.
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        let fixture = Fixture::new(kind, "-sensitive");
        for secret in ["fixture\"value", "fixture\\value", "fixture\"value\\mixed"] {
            for location in 0..3 {
                let context = match location {
                    0 => ContextDraft::new(secret),
                    1 => ContextDraft::new("complete").add_selection("explicit", secret),
                    _ => ContextDraft::new("complete").add_selection(secret, "explicit"),
                };
                let review = fixture
                    .config(Duration::from_secs(5))
                    .prepare(context, &[], 8192)
                    .unwrap();
                assert_eq!(
                    LocalAgentClient
                        .ask(
                            review.approve(),
                            LocalAgentCredential::new(secret).unwrap(),
                            &RequestCancellation::new()
                        )
                        .await
                        .err(),
                    Some(LocalAgentError::CredentialInContext)
                );
                assert!(
                    !fixture.root.path().join("spawned-fixture").exists(),
                    "credential rejection precedes every subprocess"
                );
                fixture.assert_clean();
            }
        }
    }
    let future = Fixture::new(LocalAgentKind::Codex, "-future");
    assert_eq!(
        LocalAgentClient
            .probe(
                &future.config(Duration::from_secs(5)),
                &RequestCancellation::new()
            )
            .await
            .err(),
        Some(LocalAgentError::UnsupportedVersion)
    );
    future.assert_clean();
    let unsafe_flags = Fixture::new(LocalAgentKind::Codex, "-unsafe");
    assert_eq!(
        LocalAgentClient
            .probe(
                &unsafe_flags.config(Duration::from_secs(5)),
                &RequestCancellation::new()
            )
            .await
            .err(),
        Some(LocalAgentError::IsolationUnsupported)
    );
    unsafe_flags.assert_clean();
    println!(
        "local agent process integration: all assertions/scenario groups passed; no supplier CLI or model calls"
    );
}

fn fixture(arguments: &[String]) {
    let executable = std::env::current_exe().unwrap();
    let filename = executable.file_stem().unwrap().to_string_lossy();
    if filename.contains("sensitive") {
        std::fs::write(executable.parent().unwrap().join("spawned-fixture"), []).unwrap();
    }
    let claude = filename.starts_with("claude");
    if arguments == ["--version"] {
        println!(
            "{}",
            if claude {
                "2.1.285 (Claude Code)"
            } else if filename.contains("future") {
                "codex-cli 0.161.0"
            } else {
                "codex-cli 0.160.0"
            }
        );
        return;
    }
    if arguments.iter().any(|argument| argument == "--help") {
        println!(
            "--json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check --bare --output-format --tools --disallowedTools --strict-mcp-config --mcp-config --disable-slash-commands --setting-sources --settings --permission-mode --permission-prompts --no-session-persistence --max-turns"
        );
        return;
    }
    if arguments
        .windows(2)
        .any(|fields| fields == ["features", "list"])
    {
        for name in [
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
        ] {
            println!(
                "{name} stable {}",
                if name == "hooks" && filename.contains("unsafe") {
                    "true"
                } else {
                    "false"
                }
            );
        }
        return;
    }
    assert_policy(arguments, claude);
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let frame: Value = serde_json::from_str(&input).unwrap();
    let selected: Value =
        serde_json::from_str(frame["selected_context"].as_str().unwrap()).unwrap();
    let question = selected["question"].as_str().unwrap();
    assert!(
        !arguments.iter().any(|arg| arg == question),
        "question must only enter stdin"
    );
    if question == "stderr-secret" {
        eprintln!("private-fixture-secret credential detail");
        std::process::exit(7);
    }
    if question == "stderr-flood" {
        std::io::stderr().write_all(&vec![b'e'; 70 * 1024]).unwrap();
        std::thread::sleep(Duration::from_secs(5));
        return;
    }
    if question == "line-flood" {
        std::io::stdout()
            .write_all(&vec![b'o'; 160 * 1024])
            .unwrap();
        std::thread::sleep(Duration::from_secs(5));
        return;
    }
    if question == "malformed" {
        println!("not-json-private-fixture-secret");
        return;
    }
    let mut answer = if question == "echo-credential" {
        "fixture_ephemeral_token".to_owned()
    } else {
        "完整中文回答; selected-only".to_owned()
    };
    if question.contains("descendant") {
        spawn_contained_descendant(executable, question.contains("inherits-pipes"));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !Path::new("descendant-port").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        answer = std::fs::read_to_string("descendant-port").unwrap();
        if question.starts_with("wait") {
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
    }
    if claude {
        emit(
            json!({"type":"system","subtype":"init","tools":[],"mcp_servers":[],"plugins":[],"agents":[],"skills":[],"slash_commands":[],"analytics_disabled":true,"product_feedback_disabled":true,"permissionMode":"default","claude_code_version":"2.1.285","apiKeySource":"ANTHROPIC_API_KEY"}),
        );
        if question == "tool" {
            emit(
                json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"a","role":"assistant","content":[{"type":"tool_use","name":"Bash"}]}}),
            );
            return;
        }
        emit(
            json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"a","role":"assistant","content":[{"type":"text","text":answer}]}}),
        );
        if question == "truncate" {
            print!(
                "{}",
                json!({"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":answer})
            );
            return;
        }
        emit(
            json!({"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":answer,"permission_denials":[]}),
        );
    } else {
        emit(json!({"type":"thread.started","thread_id":"owned-fixture"}));
        emit(json!({"type":"turn.started"}));
        if question == "tool" {
            emit(json!({"type":"item.started","item":{"id":"tool","type":"command_execution"}}));
            return;
        }
        emit(
            json!({"type":"item.completed","item":{"id":"a","type":"agent_message","text":answer}}),
        );
        if question == "truncate" {
            print!("{}", json!({"type":"turn.completed","usage":{}}));
            return;
        }
        emit(json!({"type":"turn.completed","usage":{}}));
    }
    if question == "late" {
        emit(json!({"type":"unreviewed-late-event"}));
    }
    if question == "nonzero" {
        std::process::exit(7);
    }
}

fn assert_policy(arguments: &[String], claude: bool) {
    for forbidden in [
        "--dangerously-bypass-approvals-and-sandbox",
        "--dangerously-bypass-hook-trust",
        "--dangerously-skip-permissions",
        "--resume",
        "--continue",
    ] {
        assert!(!arguments.iter().any(|argument| argument == forbidden));
    }
    assert!(std::env::var("HOME").unwrap().contains("keelshell-agent-"));
    assert!(std::env::var("PATH").unwrap().contains(if cfg!(windows) {
        "System32"
    } else {
        "/usr/bin"
    }));
    assert!(std::env::var_os("SSH_AUTH_SOCK").is_none());
    assert!(std::env::var_os("NODE_OPTIONS").is_none());
    assert!(std::env::var_os("CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD").is_none());
    let directory = std::env::current_dir().unwrap();
    assert!(directory.ends_with("workspace"));
    assert!(directory.join(".keelshell-isolated-workspace").is_file());
    if claude {
        for key in [
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY",
            "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
            "DISABLE_TELEMETRY",
            "DISABLE_ERROR_REPORTING",
            "DISABLE_UPDATES",
        ] {
            assert_eq!(std::env::var(key).unwrap(), "1");
        }
        for pair in [
            ["--tools", ""],
            ["--disallowedTools", "*"],
            ["--setting-sources", ""],
            ["--max-turns", "1"],
            ["--permission-mode", "default"],
        ] {
            assert!(arguments.windows(2).any(|arguments| arguments == pair));
        }
        assert!(arguments.iter().any(|argument| argument == "--bare"));
        assert_eq!(
            std::env::var("ANTHROPIC_API_KEY").unwrap(),
            "fixture_ephemeral_token"
        );
    } else {
        assert!(
            arguments
                .windows(2)
                .any(|arguments| arguments == ["--disable", "hooks"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|arguments| arguments == ["--disable", "shell_tool"])
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.contains("\":root\"=\"deny\""))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "permissions.keelshell_ask.network.enabled=false")
        );
        assert_eq!(
            std::env::var("KEELSHELL_LOCAL_AGENT_TOKEN").unwrap(),
            "fixture_ephemeral_token"
        );
    }
}

fn emit(value: Value) {
    let bytes = format!("{value}\n");
    for chunk in bytes.as_bytes().chunks(7) {
        std::io::stdout().write_all(chunk).unwrap();
        std::io::stdout().flush().unwrap();
    }
}

fn descendant() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        "descendant-port",
        listener.local_addr().unwrap().port().to_string(),
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(30));
    drop(listener);
}

// The fixture deliberately leaves a contained descendant after its leader
// exits; the adapter, rather than this fake CLI, must stop it.
#[allow(
    clippy::zombie_processes,
    reason = "intentional process-tree cleanup fixture"
)]
fn spawn_contained_descendant(executable: PathBuf, inherit: bool) {
    Command::new(executable)
        .arg("--fixture-descendant")
        .stdin(Stdio::null())
        .stdout(if inherit {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stderr(if inherit {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .spawn()
        .unwrap();
}
