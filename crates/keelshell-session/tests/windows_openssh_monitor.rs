//! Opt-in monitoring of an owned Windows OpenSSH server, never frame replay.
//!
//! `KEELSHELL_WINDOWS_SSH_LAB` names a private, temporary JSON file with exactly
//! these fields: `schema: 1`, `host: "127.0.0.1"`, `port: 49152..=65535`,
//! `username` (the runner's USERNAME), `private_key` (an absolute temporary
//! unencrypted Ed25519 key), `host_fingerprint` (the pinned SHA256 fingerprint),
//! `fixture` (owned_fixture.py's complete ready object), and `output_path`
//! (a new absolute JSON file). Unknown and duplicate fields fail.
//!
//! The parent owns sshd, ACLs, the live Python fixture, independent Win32 samples
//! and original-process/debug receipts. This test owns only its SSH connection
//! and report. PASS signs the Rust assertions, not the independent oracle,
//! server cleanup, native desktop acceptance or release acceptance.

use std::{
    collections::BTreeSet,
    future::Future,
    io::ErrorKind,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use keelshell_session::{
    ConnectionState, SessionError, SshAuth, SshOptions, SshSession,
    monitor::{
        CapabilityObservation, CapabilityReason, CapabilityState, CpuSample, DiskIoError,
        LinuxMonitor, MonitorCapabilities, MonitorError, MonitorPlatform, MonitorRuntime,
        ProcessInfo, Snapshot, SocketInfo,
    },
};
use russh::keys::{PrivateKey, ssh_key::Algorithm};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    time::{Instant, timeout_at},
};
use zeroize::Zeroizing;

const TEST_NAME: &str = "actual_windows_openssh_monitor_matches_owned_fixture";
const TOTAL_BUDGET: Duration = Duration::from_secs(90);
const CLEANUP_RESERVE: Duration = Duration::from_secs(12);
const REPORT_RESERVE: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONFIG_BYTES: u64 = 32 * 1024;
const MAX_KEY_BYTES: u64 = 16 * 1024;

type LabResult<T = ()> = Result<T, LabError>;

// Error messages contain only fixed labels and OS error kinds. In particular,
// serde errors can echo input, and SSH credential errors can contain paths.
#[derive(Debug, thiserror::Error)]
enum LabError {
    #[error("this opt-in test requires a real Windows runner")]
    WindowsRequired,
    #[error("invalid lab configuration: {0}")]
    Configuration(&'static str),
    #[error("{operation}: I/O kind {kind:?}")]
    Io {
        operation: &'static str,
        kind: ErrorKind,
    },
    #[error("absolute lab deadline expired")]
    Deadline,
    #[error("assertion failed: {0}")]
    Assertion(&'static str),
    #[error("SSH failed: {0}")]
    Session(&'static str),
    #[error("monitor failed: {0}")]
    Monitor(&'static str),
    #[error("could not encode the sanitized JSON report")]
    ReportEncoding,
}

#[derive(Debug, thiserror::Error)]
#[error("Windows OpenSSH monitor lab failed: {errors:?}")]
struct RunFailure {
    errors: Vec<LabError>,
}

// Deliberately neither Debug nor Serialize: do not accidentally print paths,
// authentication configuration or the complete configuration document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabConfig {
    schema: u32,
    host: String,
    port: u16,
    username: String,
    private_key: PathBuf,
    host_fingerprint: String,
    fixture: OwnedFixture,
    output_path: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnedFixture {
    version: u32,
    pid: u32,
    endpoints: Vec<Endpoint>,
    connected_only: Vec<Endpoint>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
enum Protocol {
    Tcp,
    Udp,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    protocol: Protocol,
    address: String,
    port: String,
    pid: String,
}

#[derive(Serialize)]
struct AssertionEvidence {
    stage: &'static str,
    assertion: &'static str,
    passed: bool,
    evidence: Value,
}

#[derive(Serialize)]
struct ErrorEvidence {
    stage: &'static str,
    error: String,
}

#[derive(Serialize)]
struct Report {
    schema: u32,
    status: &'static str,
    test: &'static str,
    expected_identity: Value,
    identity: Option<Value>,
    stage: &'static str,
    stage_evidence: Vec<Value>,
    assertion_evidence: Vec<AssertionEvidence>,
    errors: Vec<ErrorEvidence>,
    fixture: Option<Value>,
    windows_resources: Option<Value>,
    windows_processes: Option<Value>,
    windows_sockets: Option<Value>,
    sample_semantics: Option<Value>,
    capabilities: Option<Value>,
    authority: Vec<Value>,
    api_calls: Value,
    disconnect: Value,
    data_provenance: Value,
    independent_oracle_status: &'static str,
    elapsed_seconds: f64,
    budget_seconds: u64,
}

impl Report {
    fn new() -> Self {
        Self {
            schema: 1,
            status: "FAIL",
            test: TEST_NAME,
            expected_identity: json!({
                "platform": "Windows",
                "runtime": "WindowsPowerShell",
                "runtime_version_prefix": [5, 1],
            }),
            identity: None,
            stage: "configuration",
            stage_evidence: Vec::new(),
            assertion_evidence: Vec::new(),
            errors: Vec::new(),
            fixture: None,
            windows_resources: None,
            windows_processes: None,
            windows_sockets: None,
            sample_semantics: None,
            capabilities: None,
            authority: Vec::new(),
            api_calls: json!({"snapshot": 0, "processes": 0, "listening_sockets": 0}),
            disconnect: json!({"attempted": false, "local_transport_closed": null}),
            data_provenance: json!({
                "source": "SshSession::connect -> LinuxMonitor public API",
                "wire_frames_retained": false,
                "boot_time": "Snapshot.boot_time; production CIM Unix boot estimate",
                "memory_free_bytes": "Snapshot.memory.free_bytes; no available/used substitution",
                "total_free_bytes": "checked total_bytes - used_bytes; inverse of production parser",
                "capacity_status": "observed filesystem capability from this first snapshot",
                "process_name": "ProcessInfo.command; Windows ProcessName, not arguments",
                "socket_pid": "SocketInfo.process's validated pid= decimal display metadata",
                "socket_address": "SocketInfo.local's numeric address without IPv6 brackets",
                "counts": "lengths of complete public API result vectors",
                "cpu_null": "CpuSample::Unavailable, never substituted with zero",
                "exec_count_and_original_process_terminal": "parent debug owner must verify",
                "temporary_directory_acl_and_fixture_original_identity": "parent must verify",
            }),
            independent_oracle_status: "NOT_RUN_BY_RUST",
            elapsed_seconds: 0.0,
            budget_seconds: TOTAL_BUDGET.as_secs(),
        }
    }

    fn enter(&mut self, stage: &'static str, start: Instant) {
        self.stage = stage;
        self.stage_evidence.push(json!({
            "stage": stage,
            "elapsed_millis": start.elapsed().as_millis(),
        }));
    }

    fn check(&mut self, assertion: &'static str, passed: bool, evidence: Value) -> LabResult {
        self.assertion_evidence.push(AssertionEvidence {
            stage: self.stage,
            assertion,
            passed,
            evidence,
        });
        if passed {
            Ok(())
        } else {
            Err(LabError::Assertion(assertion))
        }
    }

    fn error(&mut self, error: &LabError) {
        self.errors.push(ErrorEvidence {
            stage: self.stage,
            error: error.to_string(),
        });
    }
}

#[derive(Clone, Copy)]
struct Budget {
    start: Instant,
    end: Instant,
}

impl Budget {
    fn new() -> Self {
        let start = Instant::now();
        Self {
            start,
            end: start + TOTAL_BUDGET,
        }
    }

    async fn until<T>(
        &self,
        end: Instant,
        future: impl Future<Output = LabResult<T>>,
    ) -> LabResult<T> {
        if Instant::now() >= end {
            return Err(LabError::Deadline);
        }
        timeout_at(end, future)
            .await
            .map_err(|_| LabError::Deadline)?
    }
}

fn io_error(operation: &'static str, error: std::io::Error) -> LabError {
    LabError::Io {
        operation,
        kind: error.kind(),
    }
}

fn session_error(error: &SessionError) -> LabError {
    LabError::Session(match error {
        SessionError::Io(_) => "io",
        SessionError::Ssh(_) => "protocol",
        SessionError::Authentication => "authentication",
        SessionError::Credential(_) => "credential_provider",
        SessionError::ChangedHostKey { .. } => "changed_host_key",
        SessionError::UnknownHostKey { .. } => "unknown_host_key",
        SessionError::Timeout(_) => "timeout",
        SessionError::Closed => "closed",
        _ => "other_typed_session_error",
    })
}

fn monitor_error(error: &MonitorError) -> LabError {
    match error {
        MonitorError::Session(error) => session_error(error),
        MonitorError::CapabilityUnavailable { reason, .. } => LabError::Monitor(match reason {
            CapabilityReason::CommandMissing => "command_missing",
            CapabilityReason::UnsupportedOptions => "unsupported_options",
            CapabilityReason::PermissionDenied => "permission_denied",
            CapabilityReason::InvalidOutput => "invalid_output",
            CapabilityReason::CommandFailed => "command_failed",
            CapabilityReason::UnsupportedPlatform => "unsupported_platform",
            CapabilityReason::Transport => "transport",
        }),
        MonitorError::Cancelled => LabError::Monitor("cancelled"),
        MonitorError::EnvironmentUnconfirmed => LabError::Monitor("environment_unconfirmed"),
        MonitorError::InvalidData(_) => LabError::Monitor("invalid_data"),
        MonitorError::RemoteFailure { .. } => LabError::Monitor("remote_failure"),
        _ => LabError::Monitor("other_typed_monitor_error"),
    }
}

fn canonical_decimal(value: &str, maximum: u64, minimum: u64) -> LabResult<u64> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(LabError::Configuration("noncanonical endpoint decimal"));
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|number| (minimum..=maximum).contains(number))
        .ok_or(LabError::Configuration("endpoint decimal outside range"))
}

fn validate_fixture(fixture: &OwnedFixture) -> LabResult {
    if fixture.version != 1
        || fixture.pid == 0
        || fixture.endpoints.len() != 4
        || fixture.connected_only.len() != 2
    {
        return Err(LabError::Configuration("fixture ready identity/counts"));
    }
    let mut identities = BTreeSet::new();
    for endpoint in fixture.endpoints.iter().chain(&fixture.connected_only) {
        canonical_decimal(&endpoint.port, u64::from(u16::MAX), 1)?;
        let pid = canonical_decimal(&endpoint.pid, u64::from(u32::MAX), 1)?;
        if pid != u64::from(fixture.pid)
            || !matches!(endpoint.address.as_str(), "127.0.0.1" | "::1")
            || !identities.insert(endpoint)
        {
            return Err(LabError::Configuration(
                "fixture endpoint ownership/uniqueness",
            ));
        }
    }
    let listeners: BTreeSet<_> = fixture
        .endpoints
        .iter()
        .map(|endpoint| (endpoint.protocol, endpoint.address.as_str()))
        .collect();
    let expected = BTreeSet::from([
        (Protocol::Tcp, "127.0.0.1"),
        (Protocol::Udp, "127.0.0.1"),
        (Protocol::Tcp, "::1"),
        (Protocol::Udp, "::1"),
    ]);
    let connected: BTreeSet<_> = fixture
        .connected_only
        .iter()
        .map(|endpoint| (endpoint.protocol, endpoint.address.as_str()))
        .collect();
    if listeners != expected
        || connected != BTreeSet::from([(Protocol::Tcp, "127.0.0.1"), (Protocol::Tcp, "::1")])
    {
        return Err(LabError::Configuration(
            "fixture dual-stack protocol coverage",
        ));
    }
    Ok(())
}

fn absolute_local_path(path: &Path) -> LabResult {
    let text = path.to_str().ok_or(LabError::Configuration(
        "expected a Unicode absolute local path",
    ))?;
    let bytes = text.as_bytes();
    let drive_root = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && bytes
            .get(2)
            .is_some_and(|byte| matches!(byte, b'\\' | b'/'));
    if !path.is_absolute()
        || !drive_root
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || text.chars().any(char::is_control)
        || text
            .char_indices()
            .any(|(index, character)| character == ':' && index != 1)
    {
        return Err(LabError::Configuration(
            "expected an absolute local drive path without traversal or streams",
        ));
    }
    Ok(())
}

async fn temporary_file(path: &Path, temporary_root: &Path) -> LabResult<PathBuf> {
    absolute_local_path(path)?;
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|error| io_error("temporary file metadata", error))?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(LabError::Configuration(
            "temporary input must be a regular file",
        ));
    }
    let canonical = fs::canonicalize(path)
        .await
        .map_err(|error| io_error("canonical temporary input", error))?;
    if !canonical.starts_with(temporary_root) {
        return Err(LabError::Configuration(
            "input is outside the runner temporary directory",
        ));
    }
    Ok(canonical)
}

async fn read_bounded(path: &Path, maximum: u64) -> LabResult<Vec<u8>> {
    let file = File::open(path)
        .await
        .map_err(|error| io_error("open bounded lab input", error))?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| io_error("read bounded lab input", error))?;
    if bytes.len() as u64 > maximum {
        return Err(LabError::Configuration("lab input exceeds size limit"));
    }
    Ok(bytes)
}

async fn reserve_output(path: &Path) -> LabResult<File> {
    absolute_local_path(path)?;
    if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
        return Err(LabError::Configuration("output must be a new JSON file"));
    }
    let parent = path
        .parent()
        .ok_or(LabError::Configuration("output has no parent"))?;
    let parent = fs::canonicalize(parent)
        .await
        .map_err(|error| io_error("canonical output parent", error))?;
    let name = path
        .file_name()
        .ok_or(LabError::Configuration("output has no filename"))?;
    // Reserve before authentication; create_new also rejects an existing symlink
    // or an accidental attempt to overwrite configuration/key/evidence files.
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(name))
        .await
        .map_err(|error| io_error("reserve new JSON report", error))
}

async fn load_config(output: &mut Option<File>) -> LabResult<LabConfig> {
    if !cfg!(target_os = "windows") {
        return Err(LabError::WindowsRequired);
    }
    let path = std::env::var_os("KEELSHELL_WINDOWS_SSH_LAB")
        .map(PathBuf::from)
        .ok_or(LabError::Configuration(
            "KEELSHELL_WINDOWS_SSH_LAB is required",
        ))?;
    let temporary_root = fs::canonicalize(std::env::temp_dir())
        .await
        .map_err(|error| io_error("runner temporary directory", error))?;
    let path = temporary_file(&path, &temporary_root).await?;
    let bytes = Zeroizing::new(read_bounded(&path, MAX_CONFIG_BYTES).await?);
    // Deserialize the typed object directly so duplicates are not lost through
    // a Value intermediate. Do not format a deserializer error containing input.
    let mut config: LabConfig = serde_json::from_slice(&bytes)
        .map_err(|_| LabError::Configuration("invalid strict schema-1 JSON"))?;
    *output = Some(reserve_output(&config.output_path).await?);
    if config.schema != 1 || config.host != "127.0.0.1" || config.port < 49152 {
        return Err(LabError::Configuration(
            "schema, loopback host or high port",
        ));
    }
    let login = std::env::var("USERNAME")
        .map_err(|_| LabError::Configuration("runner USERNAME is required"))?;
    let same_login = config.username.eq_ignore_ascii_case(&login)
        || std::env::var("USERDOMAIN").is_ok_and(|domain| {
            config
                .username
                .eq_ignore_ascii_case(&format!("{domain}\\{login}"))
        });
    if config.username.is_empty()
        || config.username.len() > 256
        || config.username.trim() != config.username
        || config.username.chars().any(char::is_control)
        || !same_login
    {
        return Err(LabError::Configuration(
            "username must match the current runner login",
        ));
    }
    validate_fixture(&config.fixture)?;
    fingerprint_bytes(&config.host_fingerprint)?;
    config.private_key = temporary_file(&config.private_key, &temporary_root).await?;
    let bytes = Zeroizing::new(read_bounded(&config.private_key, MAX_KEY_BYTES).await?);
    let key = PrivateKey::from_openssh(&*bytes)
        .map_err(|_| LabError::Configuration("unreadable temporary Ed25519 private key"))?;
    if key.is_encrypted() || key.algorithm() != Algorithm::Ed25519 {
        return Err(LabError::Configuration(
            "temporary key must be unencrypted Ed25519",
        ));
    }
    Ok(config)
}

fn fingerprint_bytes(value: &str) -> LabResult<[u8; 32]> {
    let encoded = value
        .strip_prefix("SHA256:")
        .filter(|text| text.len() == 43)
        .ok_or(LabError::Configuration(
            "canonical SHA256 host fingerprint required",
        ))?;
    let decoded = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| LabError::Configuration("invalid SHA256 fingerprint encoding"))?;
    if STANDARD_NO_PAD.encode(&decoded) != encoded {
        return Err(LabError::Configuration(
            "noncanonical SHA256 fingerprint encoding",
        ));
    }
    decoded
        .try_into()
        .map_err(|_| LabError::Configuration("SHA256 fingerprint length"))
}

fn options(config: &LabConfig, fingerprint: String) -> SshOptions {
    let mut options = SshOptions::new(&config.host, &config.username);
    options.port = config.port;
    options.expected_host_key = Some(fingerprint);
    options.auth = SshAuth::PrivateKey {
        path: config.private_key.clone(),
        passphrase: None,
    };
    options.timeout = REQUEST_TIMEOUT;
    options
}

async fn reject_changed_key(
    config: &LabConfig,
    session: &mut Option<SshSession>,
    report: &mut Report,
) -> LabResult {
    let mut wrong = fingerprint_bytes(&config.host_fingerprint)?;
    wrong[0] ^= 1;
    let wrong = format!("SHA256:{}", STANDARD_NO_PAD.encode(wrong));
    match SshSession::connect(options(config, wrong.clone())).await {
        Err(SessionError::ChangedHostKey { expected, actual }) => report.check(
            "wrong_pin_rejected_before_authenticated_session",
            expected == wrong && actual == config.host_fingerprint,
            json!({
                "error": "ChangedHostKey",
                "actual_matches_fixture_pin": actual == config.host_fingerprint,
                "authenticated_session_returned": false,
                "server_authentication_log_check": "parent_required",
            }),
        ),
        Ok(unexpected) => {
            // Even an unexpectedly authenticated negative control must reach
            // the same explicit disconnect path before this test fails.
            *session = Some(unexpected);
            report.check(
                "wrong_pin_rejected_before_authenticated_session",
                false,
                json!({"authenticated_session_returned": true}),
            )
        }
        Err(error) => Err(session_error(&error)),
    }
}

fn observation_json(observation: CapabilityObservation) -> Value {
    let (state, reason) = match observation.state {
        CapabilityState::Unknown => ("unknown", None),
        CapabilityState::Available => ("available", None),
        CapabilityState::Unavailable(reason) => ("unavailable", Some(reason.to_string())),
    };
    json!({
        "state": state,
        "reason": reason,
        "checked": observation.checked_at.is_some(),
        "age_millis_at_report_read": observation.checked_at.map(|at| at.elapsed().as_millis()),
    })
}

fn capabilities_json(capabilities: MonitorCapabilities) -> Value {
    json!({
        "environment_present": capabilities.environment.is_some(),
        "resources": observation_json(capabilities.resources),
        "filesystems": observation_json(capabilities.filesystems),
        "processes": observation_json(capabilities.processes),
        "sockets": observation_json(capabilities.sockets),
    })
}

fn identity_json(capabilities: MonitorCapabilities) -> Option<Value> {
    capabilities.environment.map(|environment| {
        let (runtime, version) = match environment.runtime {
            MonitorRuntime::WindowsPowerShell { version } => (
                "WindowsPowerShell",
                Some(format!(
                    "{}.{}.{}.{}",
                    version[0], version[1], version[2], version[3]
                )),
            ),
            MonitorRuntime::Posix => ("Posix", None),
        };
        json!({
            "platform": format!("{:?}", environment.platform),
            "runtime": runtime,
            "runtime_version": version,
            "source": "LinuxMonitor.capabilities.environment; independent production identity probe",
        })
    })
}

fn capacity_status(state: CapabilityState) -> LabResult<&'static str> {
    match state {
        CapabilityState::Available => Ok("ok"),
        CapabilityState::Unavailable(CapabilityReason::CommandMissing) => Ok("command_missing"),
        CapabilityState::Unavailable(CapabilityReason::UnsupportedOptions) => {
            Ok("unsupported_options")
        }
        CapabilityState::Unavailable(CapabilityReason::PermissionDenied) => Ok("permission_denied"),
        CapabilityState::Unavailable(CapabilityReason::CommandFailed) => Ok("command_failed"),
        // A first, non-skipped snapshot cannot honestly be reported as skipped.
        _ => Err(LabError::Assertion(
            "first_snapshot_has_observed_capacity_status",
        )),
    }
}

fn resources_data(sample: &Snapshot, capabilities: MonitorCapabilities) -> LabResult<Value> {
    let free = sample.memory.free_bytes.ok_or(LabError::Assertion(
        "Windows free physical memory is present",
    ))?;
    let mut filesystems = Vec::with_capacity(sample.filesystems.len());
    for filesystem in &sample.filesystems {
        // This is the exact inverse of the production parser's subtraction;
        // available_bytes is a different observation and is never substituted.
        let total_free = filesystem
            .total_bytes
            .checked_sub(filesystem.used_bytes)
            .ok_or(LabError::Assertion(
                "filesystem used bytes do not exceed total",
            ))?;
        if filesystem.total_bytes == 0
            || filesystem.available_bytes > total_free
            || u128::from(filesystem.used_bytes) * 100 / u128::from(filesystem.total_bytes)
                != u128::from(filesystem.used_percent)
        {
            return Err(LabError::Assertion(
                "filesystem byte/percentage relationships",
            ));
        }
        filesystems.push(json!({
            "source": filesystem.source,
            "mount": filesystem.mount,
            "total_bytes": filesystem.total_bytes.to_string(),
            "total_free_bytes": total_free.to_string(),
            "available_bytes": filesystem.available_bytes.to_string(),
        }));
    }
    let networks: Vec<_> = sample
        .networks
        .iter()
        .map(|network| {
            json!({
                "name": network.name,
                "received_bytes": network.received_bytes.to_string(),
                "transmitted_bytes": network.transmitted_bytes.to_string(),
            })
        })
        .collect();
    Ok(json!({
        "boot_time": sample.boot_time.to_string(),
        "uptime_seconds": sample.uptime_seconds,
        "cpu_percent": sample.cpu.reported_percent(),
        "memory_total_bytes": sample.memory.total_bytes.to_string(),
        "memory_free_bytes": free.to_string(),
        "network_count": networks.len(),
        "networks": networks,
        "capacity_status": capacity_status(capabilities.filesystems.state)?,
        "filesystem_count": filesystems.len(),
        "filesystems": filesystems,
    }))
}

fn processes_data(processes: &[ProcessInfo]) -> Value {
    let rows: Vec<_> = processes
        .iter()
        .map(|process| {
            json!({
                "pid": process.pid.to_string(),
                "name": process.command,
                "working_set_bytes": process.working_set_bytes.map(|bytes| bytes.to_string()),
            })
        })
        .collect();
    json!({"count": rows.len(), "processes": rows})
}

fn socket_endpoints(sockets: &[SocketInfo]) -> LabResult<Vec<Endpoint>> {
    let mut rows = Vec::with_capacity(sockets.len());
    let mut identities = BTreeSet::new();
    for socket in sockets {
        let protocol = match (socket.protocol.as_str(), socket.state.as_str()) {
            ("tcp", "LISTEN") => Protocol::Tcp,
            ("udp", "BOUND") => Protocol::Udp,
            _ => return Err(LabError::Assertion("Windows TCP LISTEN / UDP BOUND states")),
        };
        let address = socket
            .local
            .parse::<SocketAddr>()
            .map_err(|_| LabError::Assertion("public socket endpoint is numeric"))?;
        let (host, _) = socket
            .local
            .rsplit_once(':')
            .ok_or(LabError::Assertion("public socket endpoint has a port"))?;
        let host = host
            .strip_prefix('[')
            .and_then(|text| text.strip_suffix(']'))
            .unwrap_or(host);
        let pid = socket
            .process
            .as_deref()
            .and_then(|text| text.strip_prefix("pid="))
            .ok_or(LabError::Assertion(
                "public socket owner metadata is present",
            ))?;
        canonical_decimal(pid, u64::from(u32::MAX), 0)?;
        if address.port() == 0 || socket.peer != "*" {
            return Err(LabError::Assertion("public socket peer/port semantics"));
        }
        let row = Endpoint {
            protocol,
            address: host.to_owned(),
            port: address.port().to_string(),
            pid: pid.to_owned(),
        };
        if !identities.insert((
            row.protocol,
            row.address.clone(),
            row.port.clone(),
            row.pid.clone(),
        )) {
            return Err(LabError::Assertion("public socket identities are unique"));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn assert_sample(sample: &Snapshot, report: &mut Report) -> LabResult {
    let rates = sample.rates_since(sample);
    let cpu_valid = match sample.cpu {
        CpuSample::Unavailable => true,
        CpuSample::ReportedPercent(value) => value.is_finite() && (0.0..=100.0).contains(&value),
        CpuSample::Counters(_) => false,
    };
    report.sample_semantics = Some(json!({
        "platform": format!("{:?}", sample.platform),
        "cpu_source": match sample.cpu {
            CpuSample::Unavailable => "unavailable",
            CpuSample::ReportedPercent(_) => "reported_percent",
            CpuSample::Counters(_) => "counters",
        },
        "cpu_percent": sample.cpu.reported_percent(),
        "load_average": sample.load_average,
        "memory_available_bytes": sample.memory.available_bytes,
        "memory_used_bytes": sample.memory.used_bytes(),
        "swap_total_bytes": sample.memory.swap_total_bytes,
        "swap_free_bytes": sample.memory.swap_free_bytes,
        "disk_io_missing": sample.disk_io == Err(DiskIoError::MissingData),
        "derived_cpu_busy_percent": rates.cpu_busy_percent,
        "derived_cpu_iowait_percent": rates.cpu_iowait_percent,
        "derived_network_rate_count": rates.networks.len(),
        "derived_disk_rate_count": rates.disks.len(),
    }));
    report.check(
        "Windows_resource_identity_and_real_byte_observations",
        sample.platform == MonitorPlatform::Windows
            && sample.boot_time > 0
            && sample.uptime_seconds.is_finite()
            && sample.uptime_seconds > 0.0
            && sample.memory.total_bytes > 0
            && sample
                .memory
                .free_bytes
                .is_some_and(|free| free <= sample.memory.total_bytes),
        json!({"boot_time": sample.boot_time.to_string(), "uptime_seconds": sample.uptime_seconds}),
    )?;
    report.check(
        "Windows_unknown_fields_and_CPU_gauge_are_not_invented",
        cpu_valid
            && sample.cpu.cumulative().is_none()
            && sample.load_average.is_none()
            && sample.memory.available_bytes.is_none()
            && sample.memory.used_bytes().is_none()
            && sample.memory.swap_total_bytes.is_none()
            && sample.memory.swap_free_bytes.is_none()
            && sample.disk_io == Err(DiskIoError::MissingData)
            && rates.cpu_busy_percent == sample.cpu.reported_percent()
            && rates.cpu_iowait_percent.is_none()
            && rates.networks.is_empty()
            && rates.disks.is_empty(),
        json!({
            "cpu_measurement_valid": cpu_valid,
            "unknown_fields": "see sample_semantics",
            "rate_source": "same sample; Windows gauge only",
        }),
    )
}

async fn assert_cache_authority(
    monitor: &LinuxMonitor,
    clone: &LinuxMonitor,
    report: &mut Report,
    budget: Budget,
) -> LabResult {
    report.enter("cached_reads", budget.start);
    let before = monitor.capabilities().await;
    let original = monitor.capability_report().await;
    for _ in 0..3 {
        let cached = clone.capability_report().await;
        report.check(
            "cache_reads_and_clone_preserve_observations_and_authority",
            clone.capabilities().await == before
                && cached.observations() == before
                && monitor.accepts_capability_report(&cached)
                && clone.accepts_capability_report(&original),
            json!({"checked_at_equality_included": true, "collector_invoked": false}),
        )?;
    }
    report.enter("pause_authority", budget.start);
    monitor.invalidate_pending_capabilities();
    let paused = clone.capability_report().await;
    let pause_ok = !monitor.accepts_capability_report(&original)
        && !clone.accepts_capability_report(&original)
        && paused.observations() == before
        && clone.capabilities().await == before
        && monitor.accepts_capability_report(&paused)
        && clone.accepts_capability_report(&paused);
    report.authority.push(json!({
        "action": "pause/invalidate_pending_capabilities",
        "old_report_accepted": monitor.accepts_capability_report(&original),
        "observations_unchanged": paused.observations() == before,
        "current_report_accepted_by_both_clones": monitor.accepts_capability_report(&paused)
            && clone.accepts_capability_report(&paused),
    }));
    report.check(
        "pause_revokes_old_report_but_preserves_completed_observations",
        pause_ok,
        json!({"collector_invoked": false}),
    )?;
    report.enter("revoked_authority", budget.start);
    clone.invalidate_pending_capabilities();
    let revoked = monitor.capability_report().await;
    report.check(
        "clone_revocation_is_shared_and_old_reports_stay_revoked",
        !monitor.accepts_capability_report(&paused)
            && !clone.accepts_capability_report(&paused)
            && !monitor.accepts_capability_report(&original)
            && revoked.observations() == before
            && monitor.capabilities().await == before
            && clone.accepts_capability_report(&revoked),
        json!({"collector_invoked": false, "checked_at_equality_included": true}),
    )?;
    report.enter("reset_authority", budget.start);
    clone.reset_capabilities().await;
    let reset = monitor.capability_report().await;
    report.check(
        "explicit_reset_clears_shared_cache_without_redetection",
        !monitor.accepts_capability_report(&revoked)
            && !clone.accepts_capability_report(&revoked)
            && monitor.capabilities().await == MonitorCapabilities::default()
            && reset.observations() == MonitorCapabilities::default()
            && monitor.accepts_capability_report(&reset)
            && clone.accepts_capability_report(&reset),
        json!({"collector_invoked": false, "cache_after_reset": capabilities_json(reset.observations())}),
    )?;
    // Invalidation revokes pending work and reports; it is not a permanent
    // collection-disable switch. Calling snapshot again would admit NEW work.
    // Do not call any collector here and do not infer exec counts from timing.
    report.authority.push(json!({
        "action": "reset_capabilities",
        "observations": capabilities_json(reset.observations()),
        "revision_proof": "public report authority only; counters are private",
        "actual_exec_count_proof": "parent_debug_original_process_receipts_required",
    }));
    Ok(())
}

async fn collect_once(
    session: &SshSession,
    config: &LabConfig,
    report: &mut Report,
    budget: Budget,
) -> LabResult {
    let monitor = LinuxMonitor::new(session.clone());
    let clone = monitor.clone();
    let initial = monitor.capability_report().await;
    report.check(
        "fresh_monitor_is_unknown_and_clone_shares_its_authority",
        initial.observations() == MonitorCapabilities::default()
            && clone.accepts_capability_report(&initial)
            && session.connection_state() == ConnectionState::Connected,
        json!({"authenticated_connections": 1, "initial_cache": capabilities_json(initial.observations())}),
    )?;
    report.enter("resources", budget.start);
    report.api_calls["snapshot"] = json!(1);
    // The public route performs the fixed POSIX selector, fixed PowerShell
    // identity probe and ONE resource command. This test supplies no commands.
    let collected = monitor.snapshot().await;
    let capabilities = clone.capabilities().await;
    report.capabilities = Some(capabilities_json(capabilities));
    report.identity = identity_json(capabilities);
    let sample = collected.map_err(|error| monitor_error(&error))?;
    let environment = capabilities.environment.ok_or(LabError::Assertion(
        "independently observed Windows environment is present",
    ))?;
    let version = match environment.runtime {
        MonitorRuntime::WindowsPowerShell { version } => version,
        MonitorRuntime::Posix => {
            return Err(LabError::Assertion("Windows PowerShell runtime required"));
        }
    };
    report.check(
        "independent_identity_is_Windows_PowerShell_5_1",
        environment.platform == MonitorPlatform::Windows && version[..2] == [5, 1],
        json!({"runtime_version_components": version}),
    )?;
    assert_sample(&sample, report)?;
    report.windows_resources = Some(resources_data(&sample, capabilities)?);
    report.check(
        "snapshot_observes_resources_and_capacity_and_revokes_initial_report",
        capabilities.resources.state == CapabilityState::Available
            && capabilities.resources.checked_at.is_some()
            && capabilities.filesystems.state == CapabilityState::Available
            && capabilities.filesystems.checked_at.is_some()
            && !monitor.accepts_capability_report(&initial)
            && !clone.accepts_capability_report(&initial),
        capabilities_json(capabilities),
    )?;

    report.enter("processes", budget.start);
    report.api_calls["processes"] = json!(1);
    let collected = clone.processes().await;
    report.capabilities = Some(capabilities_json(monitor.capabilities().await));
    let processes = collected.map_err(|error| monitor_error(&error))?;
    report.windows_processes = Some(processes_data(&processes));
    report.check(
        "Windows_processes_preserve_unknown_metrics_and_PID_order",
        processes.len() <= 20_000
            && processes.windows(2).all(|pair| pair[0].pid < pair[1].pid)
            && processes.iter().all(|process| {
                !process.command.is_empty()
                    && process.user.is_none()
                    && process.cpu_percent.is_none()
                    && process.memory_percent.is_none()
            }),
        json!({
            "count": processes.len(),
            "unknown_user_cpu_memory_percent": processes.iter().all(|process| {
                process.user.is_none() && process.cpu_percent.is_none() && process.memory_percent.is_none()
            }),
        }),
    )?;
    let matching_rows = processes
        .iter()
        .filter(|process| process.pid == config.fixture.pid)
        .count();
    let owned = processes
        .iter()
        .find(|process| process.pid == config.fixture.pid);
    report.check(
        "owned_fixture_PID_and_real_working_set_are_observed",
        matching_rows == 1
            && owned.is_some_and(|process| process.working_set_bytes.is_some_and(|bytes| bytes > 0)),
        json!({
            "fixture_pid": config.fixture.pid,
            "matching_rows": matching_rows,
            "name": owned.map(|process| process.command.as_str()),
            "working_set_bytes": owned.and_then(|process| process.working_set_bytes).map(|bytes| bytes.to_string()),
            "independent_image_and_working_set_comparison": "parent_required",
        }),
    )?;

    report.enter("sockets", budget.start);
    report.api_calls["listening_sockets"] = json!(1);
    let collected = monitor.listening_sockets().await;
    report.capabilities = Some(capabilities_json(clone.capabilities().await));
    let sockets = collected.map_err(|error| monitor_error(&error))?;
    let endpoints = socket_endpoints(&sockets)?;
    report.windows_sockets = Some(json!({"count": endpoints.len(), "sockets": endpoints}));
    for expected in &config.fixture.endpoints {
        report.check(
            "owned_dual_stack_listener_or_UDP_binding_is_visible",
            endpoints.iter().any(|actual| actual == expected),
            json!({"expected_endpoint": expected}),
        )?;
    }
    for connected in &config.fixture.connected_only {
        report.check(
            "connected_only_TCP_endpoint_is_excluded",
            !endpoints.iter().any(|actual| {
                actual.protocol == connected.protocol
                    && actual.address == connected.address
                    && actual.port == connected.port
            }),
            json!({"excluded_endpoint": connected}),
        )?;
    }
    let capabilities = clone.capabilities().await;
    report.capabilities = Some(capabilities_json(capabilities));
    report.check(
        "all_read_only_capabilities_are_available_with_real_observation_times",
        [
            capabilities.resources,
            capabilities.filesystems,
            capabilities.processes,
            capabilities.sockets,
        ]
        .iter()
        .all(|observation| {
            observation.state == CapabilityState::Available && observation.checked_at.is_some()
        }) && capabilities.environment == Some(environment),
        capabilities_json(capabilities),
    )?;
    assert_cache_authority(&monitor, &clone, report, budget).await
}

async fn observe(
    output: &mut Option<File>,
    session: &mut Option<SshSession>,
    report: &mut Report,
    budget: Budget,
) -> LabResult {
    report.enter("configuration", budget.start);
    let config = load_config(output).await?;
    report.fixture = Some(json!(config.fixture));
    report.check(
        "strict_temporary_loopback_lab_configuration",
        true,
        json!({
            "schema": config.schema,
            "host": config.host,
            "port": config.port,
            "username_matches_runner": true,
            "key_type": "Ed25519",
            "key_temporary_and_absolute": true,
            "configuration_and_key_paths": "omitted",
        }),
    )?;
    report.enter("host_key_rejection_control", budget.start);
    reject_changed_key(&config, session, report).await?;
    report.enter("authentication", budget.start);
    *session = Some(
        SshSession::connect(options(&config, config.host_fingerprint.clone()))
            .await
            .map_err(|error| session_error(&error))?,
    );
    let authenticated = session.as_ref().ok_or(LabError::Assertion(
        "authenticated session ownership retained",
    ))?;
    collect_once(authenticated, &config, report, budget).await
}

async fn disconnect(
    session: SshSession,
    report: &mut Report,
    errors: &mut Vec<LabError>,
    budget: Budget,
) {
    report.enter("disconnect", budget.start);
    let until = budget.end - REPORT_RESERVE;
    let close = budget
        .until(until, async {
            session.close().await.map_err(|error| session_error(&error))
        })
        .await;
    let request_ok = close.is_ok();
    if let Err(error) = close {
        report.error(&error);
        errors.push(error);
    }
    // A queued disconnect alone is not local transport completion. Poll only
    // the public local state; remote process termination is the parent's proof.
    let closed = budget
        .until(until, async {
            while !session.is_closed() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok(())
        })
        .await;
    if let Err(error) = closed {
        report.error(&error);
        errors.push(error);
    }
    report.disconnect = json!({
        "attempted": true,
        "request_succeeded": request_ok,
        "local_transport_closed": session.is_closed(),
        "connection_state": format!("{:?}", session.connection_state()),
        "server_and_PowerShell_original_process_terminal": "parent_required",
    });
    drop(session);
}

async fn write_json(mut output: File, report: &Report) -> LabResult {
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|_| LabError::ReportEncoding)?;
    bytes.push(b'\n');
    output
        .write_all(&bytes)
        .await
        .map_err(|error| io_error("write JSON report", error))?;
    output
        .flush()
        .await
        .map_err(|error| io_error("flush JSON report", error))?;
    output
        .sync_all()
        .await
        .map_err(|error| io_error("sync JSON report", error))
}

async fn run_lab(budget: Budget) -> Result<(), RunFailure> {
    let mut report = Report::new();
    let mut output = None;
    let mut session = None;
    let mut errors = Vec::new();
    // The owner slots live outside the cancellable observation future. Every
    // ordinary failure/timeout therefore still attempts disconnect and writes
    // a sanitized failure report. No assertion panics while a session is owned.
    let result = budget
        .until(
            budget.end - CLEANUP_RESERVE,
            observe(&mut output, &mut session, &mut report, budget),
        )
        .await;
    if let Err(error) = result {
        report.error(&error);
        errors.push(error);
    }
    if let Some(session) = session.take() {
        disconnect(session, &mut report, &mut errors, budget).await;
    }
    report.status = if errors.is_empty() { "PASS" } else { "FAIL" };
    report.enter("write_json", budget.start);
    report.elapsed_seconds = budget.start.elapsed().as_secs_f64();
    if let Some(output) = output.take() {
        if let Err(error) = budget.until(budget.end, write_json(output, &report)).await {
            errors.push(error);
        }
    } else if errors.is_empty() {
        errors.push(LabError::Assertion("new JSON report was reserved"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(RunFailure { errors })
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an owned real Windows OpenSSH lab via KEELSHELL_WINDOWS_SSH_LAB"]
async fn actual_windows_openssh_monitor_matches_owned_fixture() -> Result<(), RunFailure> {
    let budget = Budget::new();
    // This final absolute fence includes configuration, collection, cleanup and
    // report I/O. The earlier fences reserve twelve seconds for the latter two.
    timeout_at(budget.end, run_lab(budget))
        .await
        .map_err(|_| RunFailure {
            errors: vec![LabError::Deadline],
        })?
}
