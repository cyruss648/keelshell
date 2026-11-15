#![cfg(test)]
//! Opt-in Linux old-API acceptance against the runner's owned OpenSSH container.
//! Successful samples come from real /proc, df, ps and ss. Fixture controls are
//! token-authenticated and confined to this disposable localhost service.

use std::{
    error::Error,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    monitor::{
        Capability, CapabilityReason, CapabilityState, LinuxMonitor, MonitorCapabilities,
        MonitorError, MonitorResult, ProcessInfo, Snapshot, SocketInfo,
    },
};
use russh::keys::{HashAlg, PublicKey, load_secret_key};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const RECOVERY_TEST: &str = "openssh_monitor_linux_capabilities_degrade_and_recover";
const TRANSPORT_TEST: &str = "openssh_monitor_linux_closed_transport_is_not_command_missing";
const TOOLS: [(&str, Capability); 3] = [
    ("df", Capability::Filesystems),
    ("ps", Capability::Processes),
    ("ss", Capability::Sockets),
];

fn environment(name: &str) -> TestResult<String> {
    Ok(std::env::var(format!("KEELSHELL_MONITOR_{name}"))?)
}

fn lower_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn binary_digest() -> TestResult<String> {
    let mut file = std::fs::File::open(std::env::current_exe()?)?;
    let mut hash = Sha256::new();
    let mut bytes = [0_u8; 65536];
    loop {
        let size = file.read(&mut bytes)?;
        if size == 0 {
            break;
        }
        hash.update(&bytes[..size]);
    }
    use std::fmt::Write as _;
    let mut digest = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut digest, "{byte:02x}")?;
    }
    Ok(digest)
}

fn options() -> TestResult<SshOptions> {
    // Validate explicit consent and full provenance before any socket is opened.
    if environment("OPT_IN")? != "localhost-owned-fixture"
        || !lower_hex(&environment("SOURCE_SHA")?, 40)
        || !lower_hex(&environment("BINARY_SHA256")?, 64)
        || binary_digest()? != environment("BINARY_SHA256")?
        || environment("USER")? != "ksmonitor"
    {
        return Err(
            "Explicit localhost opt-in, parent SHA, binary digest and dedicated user required"
                .into(),
        );
    }
    let public = std::fs::read_to_string(environment("HOST_KEY")?)?;
    let host_pin = PublicKey::from_openssh(&public)?
        .fingerprint(HashAlg::Sha256)
        .to_string();
    if host_pin != environment("HOST_PIN")? {
        return Err("Host public key does not match the runner's explicit pin".into());
    }
    let client_path = PathBuf::from(environment("CLIENT_KEY")?);
    let client = load_secret_key(&client_path, None)?;
    if client.public_key().fingerprint(HashAlg::Sha256).to_string() != environment("CLIENT_PIN")? {
        return Err("Private client key does not match the runner's explicit public pin".into());
    }
    let mut options = SshOptions::new("127.0.0.1", "ksmonitor");
    options.port = environment("PORT")?.parse()?;
    if options.port == 0 {
        return Err("Runner must supply a nonzero localhost port".into());
    }
    options.expected_host_key = Some(host_pin);
    options.auth = SshAuth::PrivateKey {
        path: client_path,
        passphrase: None,
    };
    options.timeout = Duration::from_secs(6);
    Ok(options)
}

fn write_new(directory: &Path, name: &str, value: &Value) -> TestResult {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(name))?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

struct Fixture {
    ssh: SshSession,
    owner: String,
    token: String,
    directory: PathBuf,
}

impl Fixture {
    async fn connect() -> TestResult<Self> {
        let owner = environment("OWNER")?;
        if !owner
            .strip_prefix("keelshell-monitor-")
            .is_some_and(|suffix| lower_hex(suffix, 16))
        {
            return Err("Invalid disposable fixture owner".into());
        }
        let token = environment("TOKEN")?;
        if !lower_hex(&token, 64) {
            return Err("Invalid ephemeral fixture control token".into());
        }
        let directory = PathBuf::from(environment("REPORT_DIR")?);
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("Runner must supply a new absolute report directory".into());
        }
        let ssh = SshSession::connect(options()?).await?;
        let fixture = Self {
            ssh,
            owner,
            token,
            directory,
        };
        let identity = fixture.counts().await?;
        write_new(&fixture.directory, "fixture.json", &identity)?;
        Ok(fixture)
    }

    async fn control(&self, action: &str) -> TestResult<Value> {
        // Callers supply only fixed words; the token consists solely of hex.
        let command = format!("KS_MONITOR_LAB_V1 {} {action}", self.token);
        let output = self.ssh.exec_limited(&command, 65536).await?;
        if output.exit_status != Some(0) || !output.stderr.is_empty() {
            return Err("Owned fixture control failed; remote command text is withheld".into());
        }
        let value: Value = serde_json::from_slice(&output.stdout)?;
        if value["schema"] != 1 || value["owner"] != self.owner {
            return Err("Fixture reply belongs to a different owner/schema".into());
        }
        Ok(value)
    }

    async fn counts(&self) -> TestResult<Value> {
        self.control("read-counts").await
    }

    async fn mode(&self, tool: &str, mode: &str) -> TestResult {
        if !TOOLS.iter().any(|(name, _)| *name == tool)
            || !["real", "missing", "unsupported", "permission"].contains(&mode)
        {
            return Err("Unrecognized fixed fixture tool/mode".into());
        }
        let value = self.control(&format!("set-mode {tool} {mode}")).await?;
        if value["modes"][tool] != mode {
            return Err("Fixture did not acknowledge the requested tool mode".into());
        }
        Ok(())
    }

    async fn observe(&self, monitor: &LinuxMonitor, label: &str) -> TestResult<Round> {
        let round = Round {
            snapshot: monitor.snapshot().await,
            processes: monitor.processes().await,
            sockets: monitor.listening_sockets().await,
            capabilities: monitor.capabilities().await,
            counts: self.counts().await?,
        };
        // Save partial/error observations before assertions, preserving a failed
        // baseline (including real ss absence) as well as later failed phases.
        write_new(&self.directory, &format!("{label}.json"), &round.value())?;
        Ok(round)
    }

    fn summary(&self, test: &str, extra: Value) -> TestResult {
        let mut value = json!({
            "schema": 1, "test": test, "owner": self.owner, "status": "PASSED",
            "native_app": "NOT_RUN", "source_sha": environment("SOURCE_SHA")?,
            "binary_sha256": environment("BINARY_SHA256")?,
        });
        if let Some(fields) = extra.as_object() {
            for (name, field) in fields {
                value[name] = field.clone();
            }
        }
        write_new(&self.directory, "summary.json", &value)
    }
}

fn error_value(error: &MonitorError) -> Value {
    match error {
        MonitorError::Session(_) => json!({"type": "Session", "reason": "Transport"}),
        MonitorError::CapabilityUnavailable { capability, reason } => json!({
            "type": "CapabilityUnavailable", "capability": format!("{capability:?}"),
            "reason": format!("{reason:?}"),
        }),
        other => json!({"type": "OtherMonitorError", "message": other.to_string()}),
    }
}

fn capability_value(capabilities: MonitorCapabilities) -> Value {
    let mut value = json!({});
    for capability in [
        Capability::Resources,
        Capability::Filesystems,
        Capability::Processes,
        Capability::Sockets,
    ] {
        let observation = capabilities.observation(capability);
        value[format!("{capability:?}")] = json!({
            "state": format!("{:?}", observation.state),
            "checked": observation.checked_at.is_some(),
        });
    }
    value
}

struct Round {
    snapshot: MonitorResult<Snapshot>,
    processes: MonitorResult<Vec<ProcessInfo>>,
    sockets: MonitorResult<Vec<SocketInfo>>,
    capabilities: MonitorCapabilities,
    counts: Value,
}

impl Round {
    fn value(&self) -> Value {
        json!({
            "snapshot": match &self.snapshot {
                Ok(sample) => json!({
                    "uptime_seconds": sample.uptime_seconds, "boot_time": sample.boot_time,
                    "cpu_ticks": sample.cpu.ticks, "memory_total_bytes": sample.memory.total_bytes,
                    "network_count": sample.networks.len(), "filesystem_count": sample.filesystems.len(),
                }),
                Err(error) => error_value(error),
            },
            "processes": match &self.processes {
                Ok(rows) => json!({"count": rows.len()}),
                Err(error) => error_value(error),
            },
            "sockets": match &self.sockets {
                Ok(rows) => json!({"count": rows.len()}),
                Err(error) => error_value(error),
            },
            "capabilities": capability_value(self.capabilities), "counts": self.counts,
        })
    }

    fn require(&self, failed: Option<(Capability, CapabilityReason)>) -> TestResult {
        let sample = self
            .snapshot
            .as_ref()
            .map_err(|_| "Real Linux core collection failed; see saved observation")?;
        assert!(sample.uptime_seconds > 0.0);
        assert!(sample.cpu.ticks.iter().any(|value| *value > 0));
        assert!(sample.memory.total_bytes > 0);
        assert!(!sample.networks.is_empty());
        assert_eq!(
            self.capabilities.resources.state,
            CapabilityState::Available
        );
        if failed.is_none() && self.counts["tools"]["ss"]["path"].is_null() {
            return Err(
                "CapabilityUnavailable: existing image has no real ss; partial baseline was saved"
                    .into(),
            );
        }
        for (_, capability) in TOOLS {
            let expected = failed
                .filter(|(selected, _)| *selected == capability)
                .map_or(CapabilityState::Available, |(_, reason)| {
                    CapabilityState::Unavailable(reason)
                });
            let observation = self.capabilities.observation(capability);
            assert_eq!(observation.state, expected, "{capability:?}");
            assert!(observation.checked_at.is_some());
        }
        if failed.is_some_and(|(capability, _)| capability == Capability::Filesystems) {
            assert!(sample.filesystems.is_empty());
        } else {
            assert!(!sample.filesystems.is_empty(), "real df must return mounts");
        }
        require_rows(&self.processes, Capability::Processes, failed)?;
        require_rows(&self.sockets, Capability::Sockets, failed)?;
        if failed.is_none_or(|(capability, _)| capability != Capability::Sockets) {
            if self.counts["tools"]["ss"]["path"].is_null() {
                return Err(
                    "CapabilityUnavailable: the existing Linux image has no real ss".into(),
                );
            }
            let sockets = self.sockets.as_ref().map_err(
                |_| "CapabilityUnavailable: the image's real ss cannot provide Linux socket rows",
            )?;
            assert!(sockets.iter().any(|socket| socket.local.ends_with(":2222")));
        }
        Ok(())
    }
}

fn require_rows<T>(
    result: &MonitorResult<Vec<T>>,
    capability: Capability,
    failed: Option<(Capability, CapabilityReason)>,
) -> TestResult {
    match failed.filter(|(selected, _)| *selected == capability) {
        Some((_, expected_reason)) => {
            assert!(matches!(
                result,
                Err(MonitorError::CapabilityUnavailable { capability: actual, reason })
                    if *actual == capability && *reason == expected_reason
            ));
        }
        None => {
            let rows = result.as_ref().map_err(|_| {
                format!("CapabilityUnavailable: real Linux {capability} did not succeed")
            })?;
            assert!(!rows.is_empty(), "real {capability} must return rows");
        }
    }
    Ok(())
}

fn count(value: &Value, group: &str, tool: &str) -> TestResult<u64> {
    value[group][tool]
        .as_u64()
        .ok_or_else(|| format!("Missing fixture counter {group}/{tool}").into())
}

fn no_tool_retry(before: &Value, after: &Value, tool: &str) -> TestResult {
    for group in ["probes", "invocations", "delegations"] {
        assert_eq!(count(before, group, tool)?, count(after, group, tool)?);
    }
    assert_eq!(
        count(after, "requests", "core")?,
        count(before, "requests", "core")? + 1,
        "core still executes on every ordinary refresh"
    );
    for (other, _) in TOOLS {
        if other != tool {
            assert_eq!(
                count(after, "delegations", other)?,
                count(before, "delegations", other)? + 1,
                "unaffected tools still delegate to real executables"
            );
        }
    }
    Ok(())
}

fn unknown(capabilities: MonitorCapabilities) {
    assert_eq!(capabilities, MonitorCapabilities::default());
}

async fn recovery(fixture: &Fixture) -> TestResult {
    let monitor = LinuxMonitor::new(fixture.ssh.clone());
    for (tool, _) in TOOLS {
        fixture.mode(tool, "real").await?;
    }
    let baseline = fixture.observe(&monitor, "baseline").await?;
    baseline.require(None)?;
    for (tool, _) in TOOLS {
        assert!(count(&baseline.counts, "delegations", tool)? > 0);
    }
    let mut completed = Vec::new();
    for (tool, capability) in TOOLS {
        for (mode, reason) in [
            ("missing", CapabilityReason::CommandMissing),
            ("unsupported", CapabilityReason::UnsupportedOptions),
            ("permission", CapabilityReason::PermissionDenied),
        ] {
            let case = format!("{tool}-{mode}");
            fixture.mode(tool, mode).await?;
            monitor.reset_capabilities().await;
            unknown(monitor.capabilities().await);
            let before = fixture.counts().await?;
            let first = fixture
                .observe(&monitor, &format!("{case}-degraded"))
                .await?;
            first.require(Some((capability, reason)))?;
            assert_eq!(
                count(&first.counts, "probes", tool)?,
                count(&before, "probes", tool)? + 1
            );
            assert_eq!(
                count(&first.counts, "invocations", tool)?,
                count(&before, "invocations", tool)? + u64::from(mode != "missing")
            );
            assert_eq!(
                count(&first.counts, "delegations", tool)?,
                count(&before, "delegations", tool)?,
                "injected failures cannot count as real tool success"
            );
            let failed_observation = first.capabilities.observation(capability);
            let mut previous = first.counts;
            for repetition in 0..2 {
                let cached = fixture
                    .observe(&monitor, &format!("{case}-cached-{repetition}"))
                    .await?;
                cached.require(Some((capability, reason)))?;
                no_tool_retry(&previous, &cached.counts, tool)?;
                assert_eq!(
                    cached.capabilities.observation(capability),
                    failed_observation,
                    "cached failure keeps its original observation time"
                );
                previous = cached.counts;
            }
            fixture.mode(tool, "real").await?;
            let repaired = fixture
                .observe(&monitor, &format!("{case}-repaired-without-reset"))
                .await?;
            repaired.require(Some((capability, reason)))?;
            no_tool_retry(&previous, &repaired.counts, tool)?;
            assert_eq!(
                repaired.capabilities.observation(capability),
                failed_observation
            );

            // This is a newly authenticated session, not a clone of the old one.
            let ssh = SshSession::connect(options()?).await?;
            assert!(!ssh.same_connection(&fixture.ssh));
            let fresh = LinuxMonitor::new(ssh.clone());
            unknown(fresh.capabilities().await);
            let fresh_result = async {
                let round = fixture
                    .observe(&fresh, &format!("{case}-new-connection"))
                    .await?;
                round.require(None)?;
                assert_eq!(
                    count(&round.counts, "delegations", tool)?,
                    count(&repaired.counts, "delegations", tool)? + 1
                );
                assert_eq!(
                    monitor.capabilities().await.observation(capability),
                    failed_observation
                );
                let report = fresh.capability_report().await;
                assert!(!monitor.accepts_capability_report(&report));
                Ok::<_, Box<dyn Error + Send + Sync>>(())
            }
            .await;
            let close = ssh.close().await;
            fresh_result?;
            close?;

            let before_reset = fixture.counts().await?;
            monitor.reset_capabilities().await;
            unknown(monitor.capabilities().await);
            let restored = fixture
                .observe(&monitor, &format!("{case}-explicit-reset"))
                .await?;
            restored.require(None)?;
            assert_eq!(
                count(&restored.counts, "delegations", tool)?,
                count(&before_reset, "delegations", tool)? + 1,
                "explicit reset permits a real tool invocation"
            );
            let current = restored
                .snapshot
                .as_ref()
                .map_err(|_| "Restored core sample is missing")?;
            let original = baseline
                .snapshot
                .as_ref()
                .map_err(|_| "Baseline core sample is missing")?;
            assert_eq!(current.boot_time, original.boot_time);
            assert!(current.uptime_seconds >= original.uptime_seconds);
            completed.push(case);
        }
    }
    fixture.summary(RECOVERY_TEST, json!({"completed_cases": completed}))
}

#[tokio::test]
#[ignore = "requires scripts/monitor_recovery_lab.py and a parent-pinned prebuilt binary"]
async fn openssh_monitor_linux_capabilities_degrade_and_recover() -> TestResult {
    let fixture = Fixture::connect().await?;
    let result = tokio::time::timeout(Duration::from_secs(165), recovery(&fixture)).await;
    let close = fixture.ssh.close().await;
    result??;
    close?;
    Ok(())
}

async fn transport(fixture: &Fixture) -> TestResult {
    let monitor = LinuxMonitor::new(fixture.ssh.clone());
    let before = fixture.counts().await?;
    fixture.ssh.close().await?;
    tokio::time::timeout(Duration::from_secs(4), async {
        while !fixture.ssh.is_closed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    for repetition in 0..2 {
        let snapshot = monitor.snapshot().await;
        let processes = monitor.processes().await;
        let sockets = monitor.listening_sockets().await;
        let capabilities = monitor.capabilities().await;
        write_new(
            &fixture.directory,
            &format!("transport-{repetition}.json"),
            &json!({
                "snapshot": snapshot.as_ref().err().map(error_value),
                "processes": processes.as_ref().err().map(error_value),
                "sockets": sockets.as_ref().err().map(error_value),
                "capabilities": capability_value(capabilities),
            }),
        )?;
        assert!(matches!(snapshot, Err(MonitorError::Session(_))));
        assert!(matches!(processes, Err(MonitorError::Session(_))));
        assert!(matches!(sockets, Err(MonitorError::Session(_))));
        for capability in [
            Capability::Resources,
            Capability::Processes,
            Capability::Sockets,
        ] {
            assert_eq!(
                capabilities.observation(capability).state,
                CapabilityState::Unavailable(CapabilityReason::Transport)
            );
        }
        assert_eq!(capabilities.filesystems.state, CapabilityState::Unknown);
        let report = monitor.capability_report().await;
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::Session(_))
        ));
        assert!(
            !monitor.accepts_capability_report(&report),
            "transport failures are attempted again, rather than negative-cached"
        );
    }
    // An independent authenticated verifier confirms that no closed-session
    // operation ran a remote tool or modified the fixture's mode state.
    let verifier = SshSession::connect(options()?).await?;
    let reply = verifier
        .exec_limited(
            &format!("KS_MONITOR_LAB_V1 {} read-counts", fixture.token),
            65536,
        )
        .await;
    let close = verifier.close().await;
    let output = reply?;
    close?;
    assert_eq!(output.exit_status, Some(0));
    assert!(output.stderr.is_empty());
    let after: Value = serde_json::from_slice(&output.stdout)?;
    write_new(&fixture.directory, "transport-readback.json", &after)?;
    assert_eq!(before, after);
    fixture.summary(TRANSPORT_TEST, json!({"transport_reason": "Transport"}))
}

#[tokio::test]
#[ignore = "requires scripts/monitor_recovery_lab.py and a parent-pinned prebuilt binary"]
async fn openssh_monitor_linux_closed_transport_is_not_command_missing() -> TestResult {
    let fixture = Fixture::connect().await?;
    let result = tokio::time::timeout(Duration::from_secs(30), transport(&fixture)).await;
    // The test deliberately closed this session; the runner owns server cleanup.
    result?
}
