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
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use keelshell_ai::{
    ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentError,
    LocalAgentKind, LocalAgentLimits, LocalAskProgress, LocalAskProgressReceiver, LocalAskStage,
    RequestCancellation,
};
use serde_json::{Value, json};
use tempfile::TempDir;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Controller,
    Adapter,
    FixtureSetup,
    Probe,
    Ask,
    ConfiguredBudget,
    ConfiguredAsk,
    ConfiguredDescendant,
    Progress,
    ProgressAsk,
    ProgressGate,
    ProgressAdmission,
    ProtocolErrors,
    Redaction,
    PreCancelled,
    Descendant,
    DescendantPort,
    LeaderExit,
    InheritedPipes,
    Abort,
    SensitiveContext,
    CredentialReject,
    ScratchCheck,
    Capacity,
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Begin,
    Returned,
    End,
    GateObserved,
    CancelRequested,
    ReleaseRequested,
    AbortRequested,
    DeadlineExceeded,
    LimitReached,
    ConnectBegin,
    ConnectReturned,
    AwaitBegin,
    AwaitReturned,
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SocketOutcome {
    Connected,
    Refused,
    TimedOut,
    WouldBlock,
    OtherError,
}

static STARTED: OnceLock<Instant> = OnceLock::new();
static SMALL_STACK: AtomicBool = AtomicBool::new(false);
static RECORDS: AtomicUsize = AtomicUsize::new(0);
const RECORD_LIMIT: usize = 1024;

// Windows loopback refusal probes took about two seconds each in the complete
// controller (26 probes / 52.5 s in CI). Give that same coverage a bounded total
// budget; individual Ask/cancellation deadlines and the 2 MiB stack stay fixed.
const SMALL_STACK_CONTROLLER_DEADLINE: Duration =
    Duration::from_secs(if cfg!(windows) { 90 } else { 45 });

#[derive(serde::Serialize)]
struct StageRecord {
    mode: &'static str,
    sequence: usize,
    elapsed_us: u128,
    kind: Option<&'static str>,
    stage: Stage,
    phase: Phase,
    case_index: Option<usize>,
    socket: Option<SocketOutcome>,
    os_error: Option<i32>,
}

fn record(
    kind: Option<LocalAgentKind>,
    stage: Stage,
    phase: Phase,
    case_index: Option<usize>,
    socket: Option<SocketOutcome>,
    os_error: Option<i32>,
) {
    let Some(started) = STARTED.get() else {
        // Owned executable copies implement only the fixture protocol.
        return;
    };
    let stderr = std::io::stderr();
    let mut sink = stderr.lock();
    let sequence = RECORDS.fetch_add(1, Ordering::Relaxed);
    if sequence > RECORD_LIMIT {
        return;
    }
    let (stage, phase) = if sequence == RECORD_LIMIT {
        (Stage::Capacity, Phase::LimitReached)
    } else {
        (stage, phase)
    };
    let observation = StageRecord {
        mode: if SMALL_STACK.load(Ordering::Relaxed) {
            "small_stack"
        } else {
            "default"
        },
        sequence,
        elapsed_us: started.elapsed().as_micros(),
        kind: kind.map(|kind| match kind {
            LocalAgentKind::Codex => "codex",
            LocalAgentKind::ClaudeCode => "claude_code",
        }),
        stage,
        phase,
        case_index,
        socket,
        os_error,
    };
    // Observation failure cannot panic before an owned Ask is cancelled/joined.
    // This fixed metadata never accepts input, output, environment or secrets.
    if sink.write_all(b"controller-stage ").is_ok() {
        let _ = serde_json::to_writer(&mut sink, &observation);
        let _ = sink.write_all(b"\n");
    }
}

fn mark(kind: Option<LocalAgentKind>, stage: Stage, phase: Phase, case_index: Option<usize>) {
    record(kind, stage, phase, case_index, None, None);
}

fn connect_once(kind: LocalAgentKind, stage: Stage, port: u16) -> std::io::Result<TcpStream> {
    mark(Some(kind), stage, Phase::ConnectBegin, None);
    // Preserve the original synchronous call, once, and its immediate verdict.
    let result = TcpStream::connect(("127.0.0.1", port));
    let outcome = match &result {
        Ok(_) => SocketOutcome::Connected,
        Err(error) => match error.kind() {
            std::io::ErrorKind::ConnectionRefused => SocketOutcome::Refused,
            std::io::ErrorKind::TimedOut => SocketOutcome::TimedOut,
            std::io::ErrorKind::WouldBlock => SocketOutcome::WouldBlock,
            _ => SocketOutcome::OtherError,
        },
    };
    record(
        Some(kind),
        stage,
        Phase::ConnectReturned,
        None,
        Some(outcome),
        result.as_ref().err().and_then(std::io::Error::raw_os_error),
    );
    result
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--fixture-descendant")
    {
        descendant(true);
    } else if arguments
        .first()
        .is_some_and(|arg| arg == "--fixture-descendant-no-ack")
    {
        descendant(false);
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
            SMALL_STACK.store(true, Ordering::Relaxed);
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
                            tokio::time::timeout(
                                SMALL_STACK_CONTROLLER_DEADLINE,
                                integration_cases(),
                            )
                            .await
                            .inspect_err(|_| {
                                mark(None, Stage::Controller, Phase::DeadlineExceeded, None);
                            })
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
        mark(Some(kind), Stage::FixtureSetup, Phase::Begin, None);
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
        let fixture = Self {
            root,
            executable,
            scratch,
            kind,
        };
        mark(Some(kind), Stage::FixtureSetup, Phase::End, None);
        fixture
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
        mark(Some(self.kind), Stage::Ask, Phase::Begin, None);
        let review = self
            .config(Duration::from_secs(5))
            .prepare(
                ContextDraft::new(question).add_selection("explicit selection", "selected-only"),
                &[],
                8192,
            )
            .unwrap();
        let result = LocalAgentClient
            .ask(review.approve(), credential(), &RequestCancellation::new())
            .await
            .map(|reply| reply.text().to_owned());
        mark(Some(self.kind), Stage::Ask, Phase::Returned, None);
        result
    }

    fn assert_clean(&self) {
        mark(Some(self.kind), Stage::ScratchCheck, Phase::Begin, None);
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
        mark(Some(self.kind), Stage::ScratchCheck, Phase::End, None);
    }

    async fn descendant_port(&self) -> u16 {
        mark(Some(self.kind), Stage::DescendantPort, Phase::Begin, None);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            for entry in std::fs::read_dir(&self.scratch).unwrap() {
                let path = entry.unwrap().path().join("workspace/descendant-port");
                if let Ok(text) = std::fs::read_to_string(path) {
                    // The child may have created/truncated the file before its
                    // port write finishes. Keep the original readiness bound.
                    if let Ok(port) = text.parse() {
                        mark(
                            Some(self.kind),
                            Stage::DescendantPort,
                            Phase::Returned,
                            None,
                        );
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

    fn descendant_identity(&self) -> Value {
        if let Ok(entries) = std::fs::read_dir(&self.scratch) {
            for entry in entries.flatten() {
                let path = entry.path().join("workspace/descendant-identity");
                if let Ok(text) = std::fs::read_to_string(path) {
                    return serde_json::from_str(&text)
                        .unwrap_or_else(|_| json!({"error":"invalid_identity"}));
                }
            }
        }
        // Missing diagnostics must not panic before the owned Ask finishes.
        json!({"error":"missing_identity"})
    }
}

fn credential() -> LocalAgentCredential {
    LocalAgentCredential::new("fixture_ephemeral_token").unwrap()
}

async fn integration_cases() {
    let _ = STARTED.set(Instant::now());
    mark(None, Stage::Controller, Phase::Begin, None);
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        mark(Some(kind), Stage::Adapter, Phase::Begin, None);
        let fixture = Fixture::new(kind, "");
        mark(Some(kind), Stage::Probe, Phase::Begin, None);
        let probe = LocalAgentClient
            .probe(
                &fixture.config(Duration::from_secs(5)),
                &RequestCancellation::new(),
            )
            .await
            .unwrap();
        assert_eq!(probe.kind(), kind);
        mark(Some(kind), Stage::Probe, Phase::End, None);
        assert_eq!(
            fixture
                .ask("complete 中文 ' ; $(never-execute)")
                .await
                .unwrap(),
            "完整中文回答; selected-only"
        );
        fixture.assert_clean();
        mark(Some(kind), Stage::ConfiguredBudget, Phase::Begin, None);
        Box::pin(configured_budget_cases(&fixture)).await;
        mark(Some(kind), Stage::ConfiguredBudget, Phase::End, None);
        mark(Some(kind), Stage::Progress, Phase::Begin, None);
        Box::pin(progress_cases(&fixture)).await;
        mark(Some(kind), Stage::Progress, Phase::End, None);

        for (case_index, (question, expected)) in [
            ("truncate", LocalAgentError::InvalidProtocol),
            ("late", LocalAgentError::InvalidProtocol),
            ("tool", LocalAgentError::UnexpectedOperation),
            ("malformed", LocalAgentError::InvalidProtocol),
            ("nonzero", LocalAgentError::ProcessFailed),
            ("stderr-secret", LocalAgentError::ProcessFailed),
            ("stderr-flood", LocalAgentError::OutputTooLarge),
            ("line-flood", LocalAgentError::OutputTooLarge),
        ]
        .into_iter()
        .enumerate()
        {
            mark(
                Some(kind),
                Stage::ProtocolErrors,
                Phase::Begin,
                Some(case_index),
            );
            let error = fixture.ask(question).await.unwrap_err();
            assert_eq!(error, expected, "{kind:?} {question}");
            assert!(!format!("{error:?} {error}").contains("private-fixture-secret"));
            fixture.assert_clean();
            mark(
                Some(kind),
                Stage::ProtocolErrors,
                Phase::End,
                Some(case_index),
            );
        }
        mark(Some(kind), Stage::Redaction, Phase::Begin, None);
        assert_eq!(fixture.ask("echo-credential").await.unwrap(), "[REDACTED]");
        mark(Some(kind), Stage::Redaction, Phase::End, None);

        mark(Some(kind), Stage::PreCancelled, Phase::Begin, None);
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
        mark(Some(kind), Stage::PreCancelled, Phase::End, None);

        for (case_index, cancel) in [true, false].into_iter().enumerate() {
            mark(
                Some(kind),
                Stage::Descendant,
                Phase::Begin,
                Some(case_index),
            );
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
            assert!(connect_once(kind, Stage::Descendant, port).is_ok());
            if cancel {
                signal.cancel();
                mark(
                    Some(kind),
                    Stage::Descendant,
                    Phase::CancelRequested,
                    Some(case_index),
                );
            }
            mark(
                Some(kind),
                Stage::Descendant,
                Phase::AwaitBegin,
                Some(case_index),
            );
            let outcome = tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
            mark(
                Some(kind),
                Stage::Descendant,
                Phase::AwaitReturned,
                Some(case_index),
            );
            assert_eq!(
                outcome.err(),
                Some(if cancel {
                    LocalAgentError::Cancelled
                } else {
                    LocalAgentError::Timeout
                })
            );
            assert!(
                connect_once(kind, Stage::Descendant, port).is_err(),
                "contained descendant listener released"
            );
            fixture.assert_clean();
            mark(Some(kind), Stage::Descendant, Phase::End, Some(case_index));
        }

        mark(Some(kind), Stage::LeaderExit, Phase::Begin, None);
        let answer = fixture.ask("leader-exits-descendant").await.unwrap();
        let port: u16 = answer.parse().unwrap();
        assert!(
            connect_once(kind, Stage::LeaderExit, port).is_err(),
            "successful leader exit also kills contained descendants"
        );
        fixture.assert_clean();
        mark(Some(kind), Stage::LeaderExit, Phase::End, None);

        mark(Some(kind), Stage::InheritedPipes, Phase::Begin, None);
        let started = Instant::now();
        let answer = fixture
            .ask("leader-exits-descendant-inherits-pipes")
            .await
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "inherited pipes must not delay a completed leader to timeout"
        );
        assert!(connect_once(kind, Stage::InheritedPipes, answer.parse::<u16>().unwrap()).is_err());
        fixture.assert_clean();
        mark(Some(kind), Stage::InheritedPipes, Phase::End, None);

        mark(Some(kind), Stage::Abort, Phase::Begin, None);
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
        mark(Some(kind), Stage::Abort, Phase::AbortRequested, None);
        assert!(task.await.unwrap_err().is_cancelled());
        mark(Some(kind), Stage::Abort, Phase::AwaitReturned, None);
        let deadline = Instant::now() + Duration::from_secs(2);
        while connect_once(kind, Stage::Abort, port).is_ok() {
            assert!(
                Instant::now() < deadline,
                "dropping future left contained process alive"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        fixture.assert_clean();
        mark(Some(kind), Stage::Abort, Phase::End, None);

        mark(Some(kind), Stage::CredentialReject, Phase::Begin, None);
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
        mark(Some(kind), Stage::CredentialReject, Phase::End, None);
        mark(Some(kind), Stage::Adapter, Phase::End, None);
    }
    // Known credentials must be rejected before any version/help/model spawn,
    // including the escaped nested JSON representation of selected_context.
    mark(None, Stage::SensitiveContext, Phase::Begin, None);
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
                mark(
                    Some(kind),
                    Stage::SensitiveContext,
                    Phase::AwaitReturned,
                    Some(location),
                );
                assert!(
                    !fixture.root.path().join("spawned-fixture").exists(),
                    "credential rejection precedes every subprocess"
                );
                fixture.assert_clean();
            }
        }
    }
    mark(None, Stage::SensitiveContext, Phase::End, None);
    mark(None, Stage::ProgressAdmission, Phase::Begin, None);
    Box::pin(progress_admission_cases()).await;
    mark(None, Stage::ProgressAdmission, Phase::End, None);
    mark(
        Some(LocalAgentKind::Codex),
        Stage::Probe,
        Phase::Begin,
        Some(1),
    );
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
    mark(
        Some(LocalAgentKind::Codex),
        Stage::Probe,
        Phase::End,
        Some(1),
    );
    mark(
        Some(LocalAgentKind::Codex),
        Stage::Probe,
        Phase::Begin,
        Some(2),
    );
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
    mark(
        Some(LocalAgentKind::Codex),
        Stage::Probe,
        Phase::End,
        Some(2),
    );
    mark(None, Stage::Controller, Phase::End, None);
    println!(
        "local agent process integration: all assertions/scenario groups passed; no supplier CLI or model calls"
    );
}

async fn progress_admission_cases() {
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        let fixture = Fixture::new(kind, "-future");
        let (progress, receiver) = LocalAskProgress::channel();
        let task = start_progress_ask(&fixture, "complete", progress, &RequestCancellation::new());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap(),
            Err(LocalAgentError::UnsupportedVersion)
        );
        let mut facts = Vec::new();
        drain_facts(&receiver, &mut facts);
        assert_eq!(
            facts,
            [
                LocalAskStage::WorkspaceReady,
                LocalAskStage::CheckingCli,
                LocalAskStage::Finalizing
            ]
        );
        fixture.assert_clean();
    }
    let fixture = Fixture::new(LocalAgentKind::Codex, "-unsafe");
    let (progress, receiver) = LocalAskProgress::channel();
    let task = start_progress_ask(&fixture, "complete", progress, &RequestCancellation::new());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        Err(LocalAgentError::IsolationUnsupported)
    );
    let mut facts = Vec::new();
    drain_facts(&receiver, &mut facts);
    assert_eq!(
        facts,
        [
            LocalAskStage::WorkspaceReady,
            LocalAskStage::CheckingCli,
            LocalAskStage::Finalizing
        ]
    );
    fixture.assert_clean();
}

fn start_progress_ask(
    fixture: &Fixture,
    question: &str,
    progress: LocalAskProgress,
    signal: &RequestCancellation,
) -> tokio::task::JoinHandle<Result<String, LocalAgentError>> {
    let review = fixture
        .config(Duration::from_secs(5))
        .prepare(ContextDraft::new(question), &[], 8192)
        .unwrap();
    let signal = signal.clone();
    let kind = fixture.kind;
    // Numeric cases are fixed controller positions, never the question text.
    let case_index = match question {
        "progress-final-gate" => Some(0),
        "progress-nonzero-gate" => Some(1),
        "progress-cancel-descendant-gate" => Some(2),
        "malformed" => Some(3),
        "out-of-order" => Some(4),
        "tool" => Some(5),
        "late" => Some(6),
        "complete" => Some(7),
        "leader-exits-descendant-inherits-pipes" => Some(8),
        "wait-descendant" => Some(9),
        _ => None,
    };
    tokio::spawn(async move {
        mark(Some(kind), Stage::ProgressAsk, Phase::Begin, case_index);
        let result = LocalAgentClient
            .ask_with_progress(review.approve(), credential(), &signal, progress)
            .await
            .map(|reply| reply.text().to_owned());
        mark(Some(kind), Stage::ProgressAsk, Phase::Returned, case_index);
        result
    })
}

fn drain_facts(receiver: &LocalAskProgressReceiver, facts: &mut Vec<LocalAskStage>) {
    while let Ok(stage) = receiver.try_recv() {
        assert!(!facts.contains(&stage), "duplicate lifecycle fact");
        facts.push(stage);
        assert!(facts.len() <= 8, "unbounded lifecycle history");
    }
}

async fn wait_progress_gate(
    fixture: &Fixture,
    receiver: &LocalAskProgressReceiver,
    facts: &mut Vec<LocalAskStage>,
    required: LocalAskStage,
) -> PathBuf {
    mark(Some(fixture.kind), Stage::ProgressGate, Phase::Begin, None);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        drain_facts(receiver, facts);
        for tree in std::fs::read_dir(&fixture.scratch).unwrap() {
            let workspace = tree.unwrap().path().join("workspace");
            if workspace.join("progress-ready").is_file() && facts.contains(&required) {
                mark(
                    Some(fixture.kind),
                    Stage::ProgressGate,
                    Phase::GateObserved,
                    None,
                );
                return workspace;
            }
        }
        assert!(
            Instant::now() < deadline,
            "actual progress/gate not observed: {facts:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn progress_cases(fixture: &Fixture) {
    let (progress, receiver) = LocalAskProgress::channel();
    let task = start_progress_ask(
        fixture,
        "progress-final-gate",
        progress,
        &RequestCancellation::new(),
    );
    let mut facts = Vec::new();
    let workspace = wait_progress_gate(
        fixture,
        &receiver,
        &mut facts,
        LocalAskStage::ProtocolStarted,
    )
    .await;
    assert!(facts.contains(&LocalAskStage::WorkspaceReady));
    assert!(facts.contains(&LocalAskStage::CheckingCli));
    assert!(facts.contains(&LocalAskStage::CliAdmitted));
    assert!(facts.contains(&LocalAskStage::ProcessStarted));
    assert!(facts.contains(&LocalAskStage::InputDelivered));
    assert!(!facts.contains(&LocalAskStage::ProtocolCompleted));
    assert!(!facts.contains(&LocalAskStage::Finalizing));
    assert!(
        !task.is_finished(),
        "candidate text must not complete Ask before final receipt"
    );
    std::fs::write(workspace.join("progress-release"), b"release").unwrap();
    mark(
        Some(fixture.kind),
        Stage::ProgressAsk,
        Phase::ReleaseRequested,
        Some(0),
    );
    mark(
        Some(fixture.kind),
        Stage::ProgressAsk,
        Phase::AwaitBegin,
        Some(0),
    );
    let reply = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    mark(
        Some(fixture.kind),
        Stage::ProgressAsk,
        Phase::AwaitReturned,
        Some(0),
    );
    assert!(reply.contains("完整中文回答"));
    drain_facts(&receiver, &mut facts);
    assert_eq!(facts.len(), 8);
    fixture.assert_clean();

    for question in ["progress-nonzero-gate", "progress-cancel-descendant-gate"] {
        let signal = RequestCancellation::new();
        let (progress, receiver) = LocalAskProgress::channel();
        let task = start_progress_ask(fixture, question, progress, &signal);
        let mut facts = Vec::new();
        let workspace = wait_progress_gate(
            fixture,
            &receiver,
            &mut facts,
            LocalAskStage::ProtocolCompleted,
        )
        .await;
        assert!(
            !task.is_finished(),
            "validated end receipt cannot bypass process/cleanup"
        );
        let port = if question.contains("descendant") {
            Some(fixture.descendant_port().await)
        } else {
            None
        };
        if let Some(port) = port {
            assert!(connect_once(fixture.kind, Stage::ProgressAsk, port).is_ok());
            signal.cancel();
            mark(
                Some(fixture.kind),
                Stage::ProgressAsk,
                Phase::CancelRequested,
                Some(2),
            );
        } else {
            std::fs::write(workspace.join("progress-release"), b"release").unwrap();
            mark(
                Some(fixture.kind),
                Stage::ProgressAsk,
                Phase::ReleaseRequested,
                Some(1),
            );
        }
        let error = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        mark(
            Some(fixture.kind),
            Stage::ProgressAsk,
            Phase::AwaitReturned,
            None,
        );
        assert_eq!(
            error,
            if port.is_some() {
                LocalAgentError::Cancelled
            } else {
                LocalAgentError::ProcessFailed
            }
        );
        if let Some(port) = port {
            assert!(connect_once(fixture.kind, Stage::ProgressAsk, port).is_err());
        }
        drain_facts(&receiver, &mut facts);
        assert!(facts.contains(&LocalAskStage::Finalizing));
        fixture.assert_clean();
    }

    for (question, expected, started, ended) in [
        ("malformed", LocalAgentError::InvalidProtocol, false, false),
        (
            "out-of-order",
            LocalAgentError::InvalidProtocol,
            false,
            false,
        ),
        ("tool", LocalAgentError::UnexpectedOperation, true, false),
        ("late", LocalAgentError::InvalidProtocol, true, true),
    ] {
        let (progress, receiver) = LocalAskProgress::channel();
        let task = start_progress_ask(fixture, question, progress, &RequestCancellation::new());
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        mark(
            Some(fixture.kind),
            Stage::ProgressAsk,
            Phase::AwaitReturned,
            None,
        );
        assert_eq!(result, Err(expected));
        let mut facts = Vec::new();
        drain_facts(&receiver, &mut facts);
        assert_eq!(facts.contains(&LocalAskStage::ProtocolStarted), started);
        assert_eq!(facts.contains(&LocalAskStage::ProtocolCompleted), ended);
        fixture.assert_clean();
    }

    // Observability is optional: unread and already closed consumers must not
    // delay normal protocol delivery or cancellation of actual owned children.
    for closed in [false, true] {
        let (progress, receiver) = LocalAskProgress::channel();
        let receiver = if closed {
            drop(receiver);
            None
        } else {
            Some(receiver)
        };
        let task = start_progress_ask(fixture, "complete", progress, &RequestCancellation::new());
        assert!(
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap()
                .is_ok()
        );
        drop(receiver);
        fixture.assert_clean();
        let (progress, receiver) = LocalAskProgress::channel();
        let receiver = if closed {
            drop(receiver);
            None
        } else {
            Some(receiver)
        };
        let task = start_progress_ask(
            fixture,
            "leader-exits-descendant-inherits-pipes",
            progress,
            &RequestCancellation::new(),
        );
        let answer = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            connect_once(
                fixture.kind,
                Stage::ProgressAsk,
                answer.parse::<u16>().unwrap()
            )
            .is_err(),
            "end receipt with inherited pipes still requires descendant cleanup"
        );
        drop(receiver);
        fixture.assert_clean();
        let signal = RequestCancellation::new();
        let (progress, receiver) = LocalAskProgress::channel();
        let receiver = if closed {
            drop(receiver);
            None
        } else {
            Some(receiver)
        };
        let task = start_progress_ask(fixture, "wait-descendant", progress, &signal);
        let port = fixture.descendant_port().await;
        signal.cancel();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap(),
            Err(LocalAgentError::Cancelled)
        );
        assert!(connect_once(fixture.kind, Stage::ProgressAsk, port).is_err());
        drop(receiver);
        fixture.assert_clean();
    }
}

// Exercise the same user-facing factory used by the GPUI metadata adapter;
// these are owned native fixture processes, not supplier or model acceptance.
async fn configured_budget_cases(fixture: &Fixture) {
    async fn ask(
        fixture: &Fixture,
        question: &str,
        limits: LocalAgentLimits,
    ) -> Result<String, LocalAgentError> {
        mark(Some(fixture.kind), Stage::ConfiguredAsk, Phase::Begin, None);
        let review = fixture
            .config(Duration::from_secs(5))
            .with_limits(limits)
            .prepare(ContextDraft::new(question), &[], 8192)
            .unwrap();
        let result = LocalAgentClient
            .ask(review.approve(), credential(), &RequestCancellation::new())
            .await
            .map(|reply| reply.text().to_owned());
        mark(
            Some(fixture.kind),
            Stage::ConfiguredAsk,
            Phase::Returned,
            None,
        );
        result
    }
    let limits = LocalAgentLimits::for_ask(Duration::from_secs(5), 1024, 8 * 1024).unwrap();
    assert_eq!(
        ask(fixture, "answer-exact-budget", limits)
            .await
            .unwrap()
            .len(),
        1024
    );
    fixture.assert_clean();
    assert_eq!(
        ask(fixture, "answer-excess-budget", limits).await.err(),
        Some(LocalAgentError::OutputTooLarge)
    );
    fixture.assert_clean();
    let combined = LocalAgentLimits::for_ask(Duration::from_secs(5), 1024, 1024).unwrap();
    assert_eq!(
        ask(fixture, "combined-budget", combined).await.err(),
        Some(LocalAgentError::OutputTooLarge)
    );
    fixture.assert_clean();
    let output_large =
        LocalAgentLimits::for_ask(Duration::from_secs(5), 1024, 1024 * 1024).unwrap();
    assert_eq!(
        ask(fixture, "fixed-line-flood", output_large).await.err(),
        Some(LocalAgentError::OutputTooLarge)
    );
    fixture.assert_clean();
    for (case_index, (cancel, expected_ack)) in [(false, true), (true, true), (false, false)]
        .into_iter()
        .enumerate()
    {
        mark(
            Some(fixture.kind),
            Stage::ConfiguredDescendant,
            Phase::Begin,
            Some(case_index),
        );
        let diagnostic_began = Instant::now();
        let limits = LocalAgentLimits::for_ask(
            Duration::from_secs(if cancel { 30 } else { 2 }),
            1024,
            8 * 1024,
        )
        .unwrap();
        let review = fixture
            .config(Duration::from_secs(5))
            .with_limits(limits)
            .prepare(
                ContextDraft::new(if expected_ack {
                    "wait-descendant"
                } else {
                    "wait-descendant-no-ack"
                }),
                &[],
                8192,
            )
            .unwrap();
        let cancellation = RequestCancellation::new();
        let child_signal = cancellation.clone();
        let task = tokio::spawn(async move {
            LocalAgentClient
                .ask(review.approve(), credential(), &child_signal)
                .await
        });
        let port = fixture.descendant_port().await;
        let identity = fixture.descendant_identity();
        let ready_deadline = diagnostic_began + Duration::from_secs(if cancel { 30 } else { 2 });
        // Diagnostic reads consume the existing operation budget. Capture the
        // original connect verdict once; errors must not detach the Ask task.
        let ready_connected = connect_once(fixture.kind, Stage::ConfiguredDescendant, port).is_ok();
        let ready_accepted = descendant_ack(port, ready_deadline);
        let ready_elapsed = diagnostic_began.elapsed().as_micros();
        eprintln!(
            "budget-cleanup-diagnostic {}",
            json!({"phase":"ready", "kind":format!("{:?}",fixture.kind),"cancel":cancel,"expected_ack":expected_ack,"elapsed_us":ready_elapsed,"identity":identity,"wall_ns":wall_ns(),"accepted":diagnostic_ack(&ready_accepted),"connected":ready_connected,"port":port})
        );
        let expected_pid = identity["pid"]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok());
        let ready_ok = ready_connected
            && ready_accepted
                .as_ref()
                .is_ok_and(|pid| Some(*pid) == expected_pid);
        if cancel || !ready_ok {
            cancellation.cancel();
            mark(
                Some(fixture.kind),
                Stage::ConfiguredDescendant,
                Phase::CancelRequested,
                Some(case_index),
            );
        }
        // Retain the original five-second join bound; any post-return ACK also
        // uses this same absolute deadline, never a new relative wait budget.
        let join_deadline = Instant::now() + Duration::from_secs(5);
        mark(
            Some(fixture.kind),
            Stage::ConfiguredDescendant,
            Phase::AwaitBegin,
            Some(case_index),
        );
        let result = tokio::time::timeout_at(tokio::time::Instant::from_std(join_deadline), task)
            .await
            .unwrap()
            .unwrap();
        mark(
            Some(fixture.kind),
            Stage::ConfiguredDescendant,
            Phase::AwaitReturned,
            Some(case_index),
        );
        let result_elapsed = diagnostic_began.elapsed().as_micros();
        let after = connect_once(fixture.kind, Stage::ConfiguredDescendant, port);
        let connected_elapsed = diagnostic_began.elapsed().as_micros();
        let accepted = after.as_ref().ok().map(|stream| {
            stream
                .try_clone()
                .and_then(|stream| ack_stream(stream, join_deadline))
        });
        eprintln!(
            "budget-cleanup-diagnostic {}",
            json!({"phase":"returned","kind":format!("{:?}",fixture.kind),"cancel":cancel,"expected_ack":expected_ack,"result_elapsed_us":result_elapsed,"connect_elapsed_us":connected_elapsed,"identity":identity,"wall_ns":wall_ns(),"result":result.as_ref().err().map(|error|format!("{error:?}")),"connected":after.is_ok(),"connect_error":after.as_ref().err().map(|error|format!("{:?}:{}",error.kind(),error.raw_os_error().unwrap_or(0))),"accepted":accepted.as_ref().map(diagnostic_ack),"port":port})
        );
        if expected_ack {
            assert!(
                ready_ok,
                "budget fixture was not ready: kind={:?}, cancel={cancel}",
                fixture.kind
            );
        } else {
            // A real peer which accepts but sends no PID must still cause the
            // cancellation path to finish before the diagnostic is rejected.
            assert!(ready_connected);
            assert!(ready_accepted.is_err());
            assert!(!ready_ok);
        }
        assert_eq!(
            result.err(),
            Some(if cancel || !expected_ack {
                LocalAgentError::Cancelled
            } else {
                LocalAgentError::Timeout
            })
        );
        // The optional ACK cannot change or retry this immediate verdict.
        assert!(
            after.is_err(),
            "configured budget request cleaned its contained descendant: kind={:?}, cancel={cancel}, pid={}, port={port}",
            fixture.kind,
            identity["pid"]
        );
        fixture.assert_clean();
        mark(
            Some(fixture.kind),
            Stage::ConfiguredDescendant,
            Phase::End,
            Some(case_index),
        );
    }
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
            if claude && filename.contains("future") {
                "2.1.286 (Claude Code)"
            } else if claude {
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
    if question == "combined-budget" {
        std::io::stderr().write_all(&vec![b'e'; 1024]).unwrap();
        std::io::stderr().flush().unwrap();
    }
    if question == "fixed-line-flood" {
        std::io::stdout()
            .write_all(&vec![b'o'; 300 * 1024])
            .unwrap();
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
    if question == "out-of-order" {
        emit(if claude {
            json!({"type":"assistant","message":{"id":"a","role":"assistant","content":[]}})
        } else {
            json!({"type":"turn.started"})
        });
        return;
    }
    let mut answer = if question == "echo-credential" {
        "fixture_ephemeral_token".to_owned()
    } else {
        "完整中文回答; selected-only".to_owned()
    };
    if question == "answer-exact-budget" {
        answer = "中".repeat(341) + "a";
    }
    if question == "answer-excess-budget" {
        answer = "中".repeat(342);
    }
    if question.contains("descendant") {
        spawn_contained_descendant(
            executable,
            question.contains("inherits-pipes") || question == "progress-cancel-descendant-gate",
            !question.ends_with("no-ack"),
        );
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
        if question == "progress-final-gate" {
            progress_gate();
        }
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
        if question == "progress-final-gate" {
            progress_gate();
        }
        if question == "truncate" {
            print!("{}", json!({"type":"turn.completed","usage":{}}));
            return;
        }
        emit(json!({"type":"turn.completed","usage":{}}));
    }
    if question == "progress-nonzero-gate" {
        progress_gate();
        std::process::exit(7);
    }
    if question == "progress-cancel-descendant-gate" {
        progress_gate();
    }
    if question == "late" {
        emit(json!({"type":"unreviewed-late-event"}));
    }
    if question == "nonzero" {
        std::process::exit(7);
    }
}

fn progress_gate() {
    std::fs::write("progress-ready", b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !Path::new("progress-release").is_file() {
        assert!(
            Instant::now() < deadline,
            "controller did not release owned gate"
        );
        std::thread::sleep(Duration::from_millis(5));
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

fn descendant(acknowledge: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        "descendant-identity",
        json!({"pid":std::process::id()}).to_string(),
    )
    .unwrap();
    std::fs::write(
        "descendant-port",
        listener.local_addr().unwrap().port().to_string(),
    )
    .unwrap();
    {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_write_timeout(Some(Duration::from_millis(100)))
                        .unwrap();
                    if acknowledge {
                        let _ = stream.write(&std::process::id().to_be_bytes());
                    } else {
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(error) => panic!("owned fixture accept failed: {error}"),
            }
        }
    }
    drop(listener);
}

fn descendant_ack(port: u16, deadline: Instant) -> std::io::Result<u32> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(std::io::ErrorKind::TimedOut)?;
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    ack_stream(
        TcpStream::connect_timeout(&address, remaining.min(Duration::from_millis(100)))?,
        deadline,
    )
}

fn ack_stream(mut stream: TcpStream, deadline: Instant) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    let mut used = 0;
    while used < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(std::io::ErrorKind::TimedOut)?;
        stream.set_read_timeout(Some(remaining.min(Duration::from_millis(100))))?;
        let n = stream.read(&mut bytes[used..])?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        if Instant::now() >= deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        used += n;
    }
    Ok(u32::from_be_bytes(bytes))
}

fn diagnostic_ack(result: &std::io::Result<u32>) -> Value {
    match result {
        Ok(pid) => json!({"pid":pid}),
        Err(error) => {
            json!({"error_kind":format!("{:?}",error.kind()),"raw_os_error":error.raw_os_error()})
        }
    }
}

fn wall_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

// The fixture deliberately leaves a contained descendant after its leader
// exits; the adapter, rather than this fake CLI, must stop it.
#[allow(
    clippy::zombie_processes,
    reason = "intentional process-tree cleanup fixture"
)]
fn spawn_contained_descendant(executable: PathBuf, inherit: bool, acknowledge: bool) {
    Command::new(executable)
        .arg(if acknowledge {
            "--fixture-descendant"
        } else {
            "--fixture-descendant-no-ack"
        })
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
