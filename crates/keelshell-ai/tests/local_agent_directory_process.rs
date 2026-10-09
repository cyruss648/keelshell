//! Owned native fixtures exercise the real selected-directory adapter/bootstrap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keelshell_ai::{
    ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentError,
    LocalAgentKind, LocalAgentLimits, LocalAgentWorkingDirectory, RequestCancellation,
    run_local_agent_directory_launcher,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const BOOTSTRAP: &str = "--keelshell-internal-local-ask-v1";

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "--forbidden-project-start")
        && args.len() == 2
    {
        let marker = PathBuf::from(&args[1]);
        assert!(marker.is_absolute());
        assert_eq!(
            marker.file_name(),
            Some(std::ffi::OsStr::new("forbidden-project-start"))
        );
        std::fs::write(marker, b"").unwrap();
        return;
    }
    let executable = std::env::current_exe().unwrap();
    let stem = executable.file_stem().unwrap().to_string_lossy();
    if args == [BOOTSTRAP] && stem.contains("gate-launcher") {
        std::fs::write(executable.parent().unwrap().join("gate-ready"), b"").unwrap();
        bounded_fixture_gate(executable.parent().unwrap().join("gate-release"));
    }
    if args == [BOOTSTRAP] && stem.contains("tamper-launcher") {
        tamper(
            &executable,
            if stem.contains("binary") {
                "binary"
            } else if stem.contains("schema") {
                "schema"
            } else if stem.contains("unknown") {
                "unknown"
            } else {
                "directory"
            },
        );
        return;
    }
    if run_local_agent_directory_launcher() {
        return;
    }
    if stem.starts_with("directory-agent-fixture") {
        fixture(
            &args,
            &executable,
            stem.contains("claude"),
            stem.contains("future"),
        );
        return;
    }
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(40), controller())
                .await
                .expect("bounded directory process controller");
        });
}

fn bounded_fixture_gate(path: PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.exists() {
        assert!(Instant::now() < deadline, "owned fixture gate");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn tamper(executable: &std::path::Path, mode: &str) {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let (header, body) = input.split_once('\n').unwrap();
    let mut frame: Value = serde_json::from_str(header).unwrap();
    if mode == "binary" {
        frame["executable"]["digest"][0] =
            json!(frame["executable"]["digest"][0].as_u64().unwrap() ^ 1);
    } else if mode == "schema" {
        frame["schema"] = json!(2);
    } else if mode == "unknown" {
        frame["unreviewed_field"] = json!(true);
    } else {
        frame["directory"]["nodes"][0][1] = json!(
            frame["directory"]["nodes"][0][1]
                .as_u64()
                .unwrap()
                .wrapping_add(1)
        );
    }
    let child_path = executable.parent().unwrap().join("trusted-launcher");
    let mut child = Command::new(child_path)
        .arg(BOOTSTRAP)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let _delivery = write!(pipe, "{frame}\n{body}");
    drop(pipe);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            std::process::exit(status.code().unwrap_or(1));
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Fixture {
    root: tempfile::TempDir,
    selected: PathBuf,
    scratch: PathBuf,
    executable: PathBuf,
    kind: LocalAgentKind,
}
impl Fixture {
    fn new(kind: LocalAgentKind, future: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let selected = base.join("selected 运维 folder");
        let scratch = base.join("scratch");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&scratch).unwrap();
        std::fs::write(selected.join("reviewed-marker"), b"reviewed").unwrap();
        let executable = base.join(format!(
            "directory-agent-fixture-{}{}{}",
            if kind == LocalAgentKind::ClaudeCode {
                "claude"
            } else {
                "codex"
            },
            if future { "-future" } else { "" },
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        Self {
            root,
            selected,
            scratch,
            executable,
            kind,
        }
    }
    fn config(&self, launcher: Option<&str>) -> LocalAgentConfig {
        let mut config = LocalAgentConfig::new(
            self.kind,
            &self.executable,
            &self.scratch,
            format!(
                "directory-fixture:{}",
                self.root.path().canonicalize().unwrap().display()
            ),
        )
        .unwrap()
        .with_limits(LocalAgentLimits::for_ask(Duration::from_secs(8), 8192, 128 * 1024).unwrap())
        .with_working_directory(LocalAgentWorkingDirectory::Selected(self.selected.clone()))
        .unwrap();
        if let Some(name) = launcher {
            let path = self.root.path().canonicalize().unwrap().join(name);
            std::fs::copy(std::env::current_exe().unwrap(), &path).unwrap();
            config = config.with_directory_launcher(path).unwrap();
        }
        config
    }
    async fn review(
        &self,
        question: &str,
        launcher: Option<&str>,
    ) -> keelshell_ai::ApprovedLocalAsk {
        let review = self
            .config(launcher)
            .prepare_checked(
                ContextDraft::new(question),
                &[],
                8192,
                &RequestCancellation::new(),
            )
            .await
            .unwrap();
        assert!(
            review
                .preview_json()
                .contains(self.selected.to_str().unwrap())
        );
        assert!(
            !review
                .preview_stdin()
                .contains(self.selected.to_str().unwrap())
        );
        review.approve()
    }
    fn records(&self) -> Vec<Value> {
        let path = self.root.path().join("events.jsonl");
        if !path.exists() {
            return vec![];
        }
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn clean(&self) {
        assert_eq!(
            std::fs::read_dir(&self.scratch).unwrap().count(),
            0,
            "owned scratch cleaned"
        );
    }
}
fn credential() -> LocalAgentCredential {
    LocalAgentCredential::new("fixture-only-ephemeral-key").unwrap()
}
async fn await_file(
    path: &std::path::Path,
    task: &mut tokio::task::JoinHandle<Result<keelshell_ai::LocalAgentReply, LocalAgentError>>,
) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.exists() {
        if task.is_finished() {
            let outcome = task.await.unwrap().map(|reply| reply.frame_count());
            panic!("owned request ended before readiness: {outcome:?}");
        }
        assert!(
            Instant::now() < deadline,
            "bounded owned readiness; request still running"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn controller() {
    for kind in [LocalAgentKind::Codex, LocalAgentKind::ClaudeCode] {
        let fixture = Fixture::new(kind, false);
        let approved = fixture.review("cwd", None).await;
        assert!(fixture.records().is_empty(), "preview does not start CLI");
        let reply = LocalAgentClient
            .ask(approved, credential(), &RequestCancellation::new())
            .await
            .unwrap();
        let answer: Value = serde_json::from_str(reply.text()).unwrap();
        assert_eq!(answer["cwd"], fixture.selected.to_string_lossy().as_ref());
        assert_eq!(answer["reviewed_marker"], true);
        let records = fixture.records();
        assert_eq!(
            records
                .iter()
                .filter(|record| record["phase"] != "ask")
                .count(),
            if kind == LocalAgentKind::Codex { 3 } else { 2 },
            "every real owned-byte probe is observed"
        );
        let ask = records
            .iter()
            .find(|record| record["phase"] == "ask")
            .unwrap();
        assert_eq!(ask["cwd"], fixture.selected.to_string_lossy().as_ref());
        assert_eq!(ask["fixed_policy"], true);
        assert_eq!(ask["private_home"], true);
        assert!(
            records
                .iter()
                .filter(|record| record["phase"] != "ask")
                .all(|record| record["cwd"] != ask["cwd"])
        );
        fixture.clean();
        println!(
            "{}",
            json!({"case":"selected_actual_child_cwd","kind":format!("{kind:?}"),"probes_isolated":true,"fixed_policy":true,"scratch_cleaned":true})
        );

        let fixture = Fixture::new(kind, false);
        let approved = fixture.review("held-cwd", None).await;
        let mut task = tokio::spawn(async move {
            LocalAgentClient
                .ask(approved, credential(), &RequestCancellation::new())
                .await
        });
        await_file(&fixture.root.path().join("ask-ready"), &mut task).await;
        let moved = fixture.selected.with_file_name("moved-after-fchdir");
        std::fs::rename(&fixture.selected, &moved).unwrap();
        std::fs::create_dir(&fixture.selected).unwrap();
        std::fs::write(fixture.selected.join("reviewed-marker"), b"replacement").unwrap();
        std::fs::write(fixture.root.path().join("ask-release"), b"").unwrap();
        assert_eq!(
            task.await.unwrap().unwrap_err(),
            LocalAgentError::DirectoryChanged
        );
        let proof: Value = serde_json::from_slice(
            &std::fs::read(fixture.root.path().join("held-cwd-proof.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(proof["cwd"], moved.to_string_lossy().as_ref());
        assert_eq!(
            proof["reviewed_marker"], true,
            "relative child access retained authorized inode, never replacement"
        );
        fixture.clean();

        let fixture = Fixture::new(kind, false);
        let approved = fixture.review("cwd", None).await;
        std::fs::rename(&fixture.selected, fixture.selected.with_file_name("moved")).unwrap();
        std::fs::create_dir(&fixture.selected).unwrap();
        assert_eq!(
            LocalAgentClient
                .ask(approved, credential(), &RequestCancellation::new())
                .await
                .unwrap_err(),
            LocalAgentError::DirectoryChanged
        );
        assert!(fixture.records().is_empty());
        fixture.clean();

        let fixture = Fixture::new(kind, true);
        let approved = fixture.review("cwd", None).await;
        assert_eq!(
            LocalAgentClient
                .ask(approved, credential(), &RequestCancellation::new())
                .await
                .unwrap_err(),
            LocalAgentError::UnsupportedVersion
        );
        fixture.clean();

        let fixture = Fixture::new(kind, false);
        let approved = fixture.review("wait", None).await;
        let cancellation = RequestCancellation::new();
        let token = cancellation.clone();
        let mut task =
            tokio::spawn(async move { LocalAgentClient.ask(approved, credential(), &token).await });
        await_file(&fixture.root.path().join("ask-ready"), &mut task).await;
        cancellation.cancel();
        assert_eq!(task.await.unwrap().unwrap_err(), LocalAgentError::Cancelled);
        fixture.clean();
        let record = fixture
            .records()
            .into_iter()
            .find(|record| record["phase"] == "ask")
            .unwrap();
        #[cfg(unix)]
        assert_eq!(
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(record["pid"].as_i64().unwrap() as i32),
                None
            ),
            Err(nix::errno::Errno::ESRCH)
        );
        println!(
            "{}",
            json!({"case":"selected_cancel","kind":format!("{kind:?}"),"owned_child_reaped":true,"scratch_cleaned":true})
        );
    }
    #[cfg(unix)]
    {
        for (launcher, error) in [
            ("tamper-launcher", LocalAgentError::DirectoryChanged),
            ("tamper-launcher-binary", LocalAgentError::ExecutableChanged),
            (
                "tamper-launcher-schema",
                LocalAgentError::DirectoryUnsupported,
            ),
            (
                "tamper-launcher-unknown",
                LocalAgentError::DirectoryUnsupported,
            ),
        ] {
            let fixture = Fixture::new(LocalAgentKind::Codex, false);
            std::fs::copy(
                std::env::current_exe().unwrap(),
                fixture.root.path().join("trusted-launcher"),
            )
            .unwrap();
            let approved = fixture.review("cwd", Some(launcher)).await;
            assert_eq!(
                LocalAgentClient
                    .ask(approved, credential(), &RequestCancellation::new())
                    .await
                    .unwrap_err(),
                error
            );
            assert!(
                !fixture
                    .records()
                    .iter()
                    .any(|record| record["phase"] == "ask")
            );
            fixture.clean();
        }
        let fixture = Fixture::new(LocalAgentKind::ClaudeCode, false);
        let approved = fixture.review("cwd", Some("gate-launcher")).await;
        let mut task = tokio::spawn(async move {
            LocalAgentClient
                .ask(approved, credential(), &RequestCancellation::new())
                .await
        });
        await_file(&fixture.root.path().join("gate-ready"), &mut task).await;
        std::fs::rename(
            &fixture.selected,
            fixture.selected.with_file_name("moved-before-child-open"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            fixture.selected.with_file_name("moved-before-child-open"),
            &fixture.selected,
        )
        .unwrap();
        std::fs::write(fixture.root.path().join("gate-release"), b"").unwrap();
        assert_eq!(
            task.await.unwrap().unwrap_err(),
            LocalAgentError::DirectoryChanged
        );
        assert!(
            !fixture
                .records()
                .iter()
                .any(|record| record["phase"] == "ask")
        );
        fixture.clean();
    }
    let fixture = Fixture::new(LocalAgentKind::Codex, false);
    let approved = fixture.review("cwd", None).await;
    std::fs::create_dir(fixture.selected.join(".codex")).unwrap();
    assert_eq!(
        LocalAgentClient
            .ask(approved, credential(), &RequestCancellation::new())
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryMetadataChanged
    );
    assert!(fixture.records().is_empty());
    fixture.clean();
    for input in [
        "{}\n".to_owned(),
        "{\"schema\":2}\n".to_owned(),
        "not-json\n".to_owned(),
        format!("{}\n", "x".repeat(64 * 1024 + 1)),
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg(BOOTSTRAP)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let _delivery = child.stdin.take().unwrap().write_all(input.as_bytes());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(77));
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bootstrap malformed bound");
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    println!("directory native process controller: 20 cases passed");
}

fn fixture(args: &[String], _exe: &std::path::Path, claude: bool, future: bool) {
    let phase = if args == ["--version"] {
        "version"
    } else if args.iter().any(|arg| arg == "--help") {
        "help"
    } else if args.windows(2).any(|pair| pair == ["features", "list"]) {
        "features"
    } else {
        "ask"
    };
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let root = if phase == "ask" {
        let model = args.windows(2).find(|pair| pair[0] == "--model").unwrap();
        PathBuf::from(model[1].strip_prefix("directory-fixture:").unwrap())
    } else {
        home.parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned()
    };
    let cwd = std::env::current_dir().unwrap();
    let fixed_policy = if claude {
        args.iter().any(|arg| arg == "--bare")
            && args.windows(2).any(|pair| pair == ["--tools", ""])
            && args
                .windows(2)
                .any(|pair| pair == ["--disallowedTools", "*"])
    } else {
        args.iter().any(|arg| arg == "project_root_markers=[]")
            && args
                .iter()
                .any(|arg| arg == "skills.include_instructions=false")
            && args
                .windows(2)
                .any(|pair| pair == ["--enable", "skip_host_skill_discovery"])
            && args
                .windows(2)
                .any(|pair| pair == ["--disable", "code_mode_host"])
    };
    let record = json!({"phase":phase,"pid":std::process::id(),"cwd":cwd,"fixed_policy":fixed_policy,"private_home":home!=cwd && home.file_name()==Some(std::ffi::OsStr::new("home"))});
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("events.jsonl"))
        .unwrap();
    writeln!(file, "{record}").unwrap();
    drop(file);
    if phase == "version" {
        println!(
            "{}",
            if claude {
                if future {
                    "2.1.286 (Claude Code)"
                } else {
                    "2.1.285 (Claude Code)"
                }
            } else if future {
                "codex-cli 0.161.0"
            } else {
                "codex-cli 0.160.0"
            }
        );
        return;
    }
    if phase == "help" {
        println!(
            "--json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check --bare --output-format --tools --disallowedTools --strict-mcp-config --mcp-config --disable-slash-commands --setting-sources --settings --permission-mode --permission-prompts --no-session-persistence --max-turns"
        );
        return;
    }
    if phase == "features" {
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
            println!("{name} stable false");
        }
        println!("skip_host_skill_discovery stable true");
        println!("code_mode_host stable false");
        return;
    }
    assert!(fixed_policy);
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let frame: Value = serde_json::from_str(&input).unwrap();
    assert!(
        frame.get("schema").is_none(),
        "control metadata is not supplier context"
    );
    let selected: Value =
        serde_json::from_str(frame["selected_context"].as_str().unwrap()).unwrap();
    if selected["question"] == "wait" {
        std::fs::write(root.join("ask-ready"), b"").unwrap();
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    if selected["question"] == "held-cwd" {
        std::fs::write(root.join("ask-ready"), b"").unwrap();
        bounded_fixture_gate(root.join("ask-release"));
        let proof = json!({"cwd":std::env::current_dir().unwrap(),"reviewed_marker":std::fs::read("reviewed-marker").unwrap()==b"reviewed"});
        std::fs::write(
            root.join("held-cwd-proof.json"),
            serde_json::to_vec(&proof).unwrap(),
        )
        .unwrap();
    }
    let answer=json!({"cwd":std::env::current_dir().unwrap(),"reviewed_marker":std::fs::read("reviewed-marker").unwrap()==b"reviewed"}).to_string();
    if claude {
        println!(
            "{}",
            json!({"type":"system","subtype":"init","tools":[],"mcp_servers":[],"plugins":[],"agents":[],"skills":[],"slash_commands":[],"analytics_disabled":true,"product_feedback_disabled":true,"permissionMode":"default","claude_code_version":"2.1.285","apiKeySource":"ANTHROPIC_API_KEY"})
        );
        println!(
            "{}",
            json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"a","role":"assistant","content":[{"type":"text","text":answer}]}})
        );
        println!(
            "{}",
            json!({"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":answer,"permission_denials":[]})
        );
    } else {
        println!(
            "{}",
            json!({"type":"thread.started","thread_id":"owned-directory"})
        );
        println!("{}", json!({"type":"turn.started"}));
        println!(
            "{}",
            json!({"type":"item.completed","item":{"id":"a","type":"agent_message","text":answer}})
        );
        println!("{}", json!({"type":"turn.completed","usage":{}}));
    }
}
