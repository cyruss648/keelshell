//! Fixed Windows PowerShell 5.1 collectors and strict, bounded JSON frames.
//!
//! Every integer byte counter travels as a canonical decimal string, preserving
//! values above JSON's binary64 exact range. CIM uptime is a wall-clock estimate;
//! neither CPU ticks nor network rates are inferred here.

use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, Ipv6Addr},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, de};

use super::{
    Capability, CapabilityReason, CapabilityState, CpuSample, DiskIoError, ExecOutput,
    FilesystemSnapshot, MemorySnapshot, MonitorError, MonitorPlatform, MonitorResult,
    NetworkCounters, ProcessInfo, Snapshot, SocketInfo,
};

const COMMAND_PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";
const COMMON: &str = include_str!("windows/common.ps1");
const SUFFIX: &str = "}catch{F (E $_) $null 1}\n";
const MAX_COMMAND: usize = 8000;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_NETWORKS: usize = 256;
const MAX_FILESYSTEMS: usize = 512;
const MAX_ROWS: usize = 20_000;
const MAX_TEXT: usize = 1024;
const MAX_ADDRESS: usize = 128;
const UNITS: &str = "bytes seconds percent";

/// Fixed command selections; no remote or user text becomes script content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    /// Establish Desktop Windows PowerShell 5.1 and the remote OS identity.
    Probe,
    /// Collect resources, optionally bypassing independently cached capacity.
    Resources {
        /// Omit capacity collection using a fixed literal variant.
        skip_capacity: bool,
    },
    /// List account-visible process names and optional working sets.
    Processes,
    /// Collect both TCP listeners and bound UDP endpoints.
    Sockets,
}

impl Operation {
    fn parts(self) -> (&'static str, &'static str) {
        match self {
            Self::Probe => (
                "$op='probe';$skip=$false;\n",
                include_str!("windows/probe.ps1"),
            ),
            Self::Resources {
                skip_capacity: false,
            } => (
                "$op='resources';$skip=$false;\n",
                include_str!("windows/resources.ps1"),
            ),
            Self::Resources {
                skip_capacity: true,
            } => (
                "$op='resources';$skip=$true;\n",
                include_str!("windows/resources.ps1"),
            ),
            Self::Processes => (
                "$op='processes';$skip=$false;\n",
                include_str!("windows/processes.ps1"),
            ),
            Self::Sockets => (
                "$op='sockets';$skip=$false;\n",
                include_str!("windows/sockets.ps1"),
            ),
        }
    }
}

/// Assemble only checked-in literals as UTF-16LE Base64, with no BOM or wrapping.
///
/// Rejects a command that reaches the conservative default-shell length bound.
pub(super) fn command(operation: Operation) -> MonitorResult<String> {
    let (prefix, body) = operation.parts();
    let script = [prefix, COMMON, body, SUFFIX].concat();
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let command = format!("{COMMAND_PREFIX}{}", STANDARD.encode(bytes));
    if !command.is_ascii() || command.len() >= MAX_COMMAND {
        return invalid("Windows command exceeds its fixed ASCII bound");
    }
    Ok(command)
}

/// Validate one complete identity frame from Desktop Windows PowerShell 5.1.
pub(super) fn parse_probe(output: ExecOutput) -> MonitorResult<[u32; 4]> {
    let (data, version) = frame::<Probe>(output, Kind::Probe)?;
    if data.os != "Win32NT" {
        return invalid("invalid Windows identity");
    }
    Ok(version)
}

/// Validate core resources and an independent capacity observation.
///
/// A typed capacity failure preserves the core sample; malformed frames,
/// duplicate fields, counters or inconsistent partial arrays reject the frame.
pub(super) fn parse_resources(output: ExecOutput) -> MonitorResult<(Snapshot, CapabilityState)> {
    let (data, _) = frame::<Resources>(output, Kind::Resources)?;
    if !data.uptime_seconds.is_finite() || data.uptime_seconds < 0.0 {
        return invalid("invalid Windows uptime estimate");
    }
    let cpu = match data.cpu_percent {
        None => CpuSample::Unavailable,
        Some(percent) if percent.is_finite() && (0.0..=100.0).contains(&percent) => {
            CpuSample::ReportedPercent(percent)
        }
        Some(_) => return invalid("invalid Windows CPU percentage"),
    };
    if data.memory_total_bytes.0 == 0 || data.memory_free_bytes.0 > data.memory_total_bytes.0 {
        return invalid("invalid Windows physical memory");
    }
    check_count(data.network_count, data.networks.len(), MAX_NETWORKS)?;
    let mut names = BTreeSet::new();
    let mut networks = Vec::with_capacity(data.networks.len());
    for network in data.networks {
        display_text(&network.name)?;
        if !names.insert(network.name.clone()) {
            return invalid("duplicate Windows interface");
        }
        networks.push(NetworkCounters {
            name: network.name,
            received_bytes: network.received_bytes.0,
            transmitted_bytes: network.transmitted_bytes.0,
        });
    }
    check_count(
        data.filesystem_count,
        data.filesystems.len(),
        MAX_FILESYSTEMS,
    )?;
    let (filesystems, capacity) = capacity(data.capacity_status, data.filesystems)?;
    Ok((
        Snapshot {
            platform: MonitorPlatform::Windows,
            boot_time: data.boot_time.0,
            uptime_seconds: data.uptime_seconds,
            load_average: None,
            cpu,
            memory: MemorySnapshot {
                total_bytes: data.memory_total_bytes.0,
                available_bytes: None,
                free_bytes: Some(data.memory_free_bytes.0),
                swap_total_bytes: None,
                swap_free_bytes: None,
            },
            networks,
            filesystems,
            disk_io: Err(DiskIoError::MissingData),
        },
        capacity,
    ))
}

/// Validate the complete PID-ordered process inventory, preserving unknowns.
pub(super) fn parse_processes(output: ExecOutput) -> MonitorResult<Vec<ProcessInfo>> {
    let (data, _) = frame::<Processes>(output, Kind::Processes)?;
    check_count(data.count, data.processes.len(), MAX_ROWS)?;
    let mut processes = Vec::with_capacity(data.processes.len());
    let mut previous = None;
    for process in data.processes {
        let pid = pid(process.pid)?;
        if previous.is_some_and(|previous| previous >= pid) {
            return invalid("unordered or duplicate Windows process PID");
        }
        display_text(&process.name)?;
        previous = Some(pid);
        processes.push(ProcessInfo {
            pid,
            command: process.name,
            working_set_bytes: process.working_set_bytes.map(|bytes| bytes.0),
            user: None,
            cpu_percent: None,
            memory_percent: None,
        });
    }
    Ok(processes)
}

/// Validate a complete combined TCP/UDP result; PIDs are display metadata only.
pub(super) fn parse_sockets(output: ExecOutput) -> MonitorResult<Vec<SocketInfo>> {
    let (data, _) = frame::<Sockets>(output, Kind::Sockets)?;
    check_count(data.count, data.sockets.len(), MAX_ROWS)?;
    let mut identities = BTreeSet::new();
    let mut sockets = Vec::with_capacity(data.sockets.len());
    for socket in data.sockets {
        let state = match socket.protocol.as_str() {
            "tcp" => "LISTEN",
            "udp" => "BOUND",
            _ => return invalid("invalid Windows socket protocol"),
        };
        let port = u16::try_from(socket.port.0)
            .ok()
            .filter(|port| *port != 0)
            .ok_or(MonitorError::InvalidData("invalid Windows socket port"))?;
        let pid = pid(socket.pid)?;
        let address = address(&socket.address)?;
        let local = format!("{address}:{port}");
        if !identities.insert((socket.protocol.clone(), local.clone(), pid)) {
            return invalid("duplicate Windows socket");
        }
        sockets.push(SocketInfo {
            protocol: socket.protocol,
            state: state.to_owned(),
            local,
            peer: "*".to_owned(),
            process: Some(format!("pid={pid}")),
        });
    }
    Ok(sockets)
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Probe,
    Resources,
    Processes,
    Sockets,
}

impl Kind {
    fn operation(self) -> &'static str {
        match self {
            Self::Probe => "Windows environment probe",
            Self::Resources => "Windows resource collection",
            Self::Processes => "Windows process collection",
            Self::Sockets => "Windows socket collection",
        }
    }

    fn capability(self) -> Capability {
        match self {
            Self::Probe | Self::Resources => Capability::Resources,
            Self::Processes => Capability::Processes,
            Self::Sockets => Capability::Sockets,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    CommandMissing,
    PermissionDenied,
    CommandFailed,
    UnsupportedOptions,
}

impl Status {
    fn reason(self) -> Option<CapabilityReason> {
        match self {
            Self::Ok => None,
            Self::CommandMissing => Some(CapabilityReason::CommandMissing),
            Self::PermissionDenied => Some(CapabilityReason::PermissionDenied),
            Self::CommandFailed => Some(CapabilityReason::CommandFailed),
            Self::UnsupportedOptions => Some(CapabilityReason::UnsupportedOptions),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct Frame<T> {
    version: u32,
    operation: Kind,
    platform: String,
    runtime: String,
    runtime_version: String,
    units: String,
    status: Status,
    // deserialize_with intentionally makes this nullable field mandatory.
    #[serde(deserialize_with = "nullable")]
    data: Option<T>,
}

fn frame<T: de::DeserializeOwned>(output: ExecOutput, kind: Kind) -> MonitorResult<(T, [u32; 4])> {
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_BYTES {
        return invalid("Windows monitoring output exceeds 2 MiB");
    }
    let exit_status = output.exit_status.ok_or(MonitorError::RemoteFailure {
        operation: kind.operation(),
        exit_status: None,
    })?;
    if !output.stderr.is_empty() {
        return invalid("Windows monitoring stderr is not empty");
    }
    let input = std::str::from_utf8(&output.stdout)
        .map_err(|_| MonitorError::InvalidData("non-UTF-8 Windows monitoring output"))?;
    let input = input
        .strip_suffix("\r\n")
        .or_else(|| input.strip_suffix('\n'))
        .unwrap_or(input);
    if !input.starts_with('{')
        || !input.ends_with('}')
        || input
            .as_bytes()
            .iter()
            .enumerate()
            .any(|(i, byte)| *byte == b'\r' && input.as_bytes().get(i + 1) != Some(&b'\n'))
    {
        return invalid("incomplete or unframed Windows monitoring output");
    }
    // Deserialize the typed envelope directly. A Value intermediate would
    // discard duplicate-key evidence before nested records could reject it.
    let frame: Frame<T> = serde_json::from_str(input)
        .map_err(|_| MonitorError::InvalidData("invalid Windows monitoring frame"))?;
    if frame.version != 1
        || frame.operation != kind
        || frame.platform != "Windows"
        || frame.runtime != "WindowsPowerShell"
        || frame.units != UNITS
    {
        return invalid("invalid Windows monitoring envelope");
    }
    let version = runtime_version(&frame.runtime_version)?;
    match (frame.status.reason(), exit_status, frame.data) {
        (None, 0, Some(data)) => Ok((data, version)),
        (Some(reason), 1.., None) => Err(MonitorError::CapabilityUnavailable {
            capability: kind.capability(),
            reason,
        }),
        (None, 1.., _) => Err(MonitorError::RemoteFailure {
            operation: kind.operation(),
            exit_status: Some(exit_status),
        }),
        _ => invalid("inconsistent Windows status, data or exit"),
    }
}

fn runtime_version(value: &str) -> MonitorResult<[u32; 4]> {
    let mut parts = value.split('.');
    let mut version = [0; 4];
    for component in &mut version {
        let value = parts
            .next()
            .ok_or(MonitorError::InvalidData("missing Windows runtime version"))?;
        *component = u32::try_from(decimal(value)?)
            .map_err(|_| MonitorError::InvalidData("Windows runtime version overflow"))?;
    }
    if parts.next().is_some() || version[..2] != [5, 1] {
        return invalid("unsupported Windows runtime version");
    }
    Ok(version)
}

#[derive(Debug)]
struct Decimal(u64);

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        decimal(&value).map(Self).map_err(de::Error::custom)
    }
}

fn decimal(value: &str) -> MonitorResult<u64> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return invalid("invalid Windows canonical unsigned decimal");
    }
    value
        .parse()
        .map_err(|_| MonitorError::InvalidData("Windows unsigned decimal overflow"))
}

fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    os: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    boot_time: Decimal,
    uptime_seconds: f64,
    #[serde(deserialize_with = "nullable")]
    cpu_percent: Option<f64>,
    memory_total_bytes: Decimal,
    memory_free_bytes: Decimal,
    network_count: usize,
    networks: Vec<Network>,
    capacity_status: CapacityStatus,
    filesystem_count: usize,
    filesystems: Vec<Filesystem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Network {
    name: String,
    received_bytes: Decimal,
    transmitted_bytes: Decimal,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Filesystem {
    source: String,
    mount: String,
    total_bytes: Decimal,
    total_free_bytes: Decimal,
    available_bytes: Decimal,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum CapacityStatus {
    Ok,
    Skipped,
    CommandMissing,
    PermissionDenied,
    CommandFailed,
    UnsupportedOptions,
}

fn capacity(
    status: CapacityStatus,
    rows: Vec<Filesystem>,
) -> MonitorResult<(Vec<FilesystemSnapshot>, CapabilityState)> {
    let state = match status {
        CapacityStatus::Ok => CapabilityState::Available,
        CapacityStatus::Skipped => CapabilityState::Unknown,
        CapacityStatus::CommandMissing => {
            CapabilityState::Unavailable(CapabilityReason::CommandMissing)
        }
        CapacityStatus::PermissionDenied => {
            CapabilityState::Unavailable(CapabilityReason::PermissionDenied)
        }
        CapacityStatus::CommandFailed => {
            CapabilityState::Unavailable(CapabilityReason::CommandFailed)
        }
        CapacityStatus::UnsupportedOptions => {
            CapabilityState::Unavailable(CapabilityReason::UnsupportedOptions)
        }
    };
    if state != CapabilityState::Available {
        if !rows.is_empty() {
            return invalid("partial failed Windows capacity data");
        }
        return Ok((Vec::new(), state));
    }
    let mut sources = BTreeSet::new();
    let mut mounts = BTreeSet::new();
    let mut filesystems = Vec::with_capacity(rows.len());
    for row in rows {
        display_text(&row.source)?;
        display_text(&row.mount)?;
        if !sources.insert(row.source.to_lowercase()) || !mounts.insert(row.mount.to_lowercase()) {
            return invalid("duplicate Windows volume");
        }
        if row.total_bytes.0 == 0
            || row.total_free_bytes.0 > row.total_bytes.0
            || row.available_bytes.0 > row.total_free_bytes.0
        {
            return invalid("invalid Windows volume capacity");
        }
        let used_bytes = row.total_bytes.0 - row.total_free_bytes.0;
        let used_percent =
            u32::try_from(u128::from(used_bytes) * 100 / u128::from(row.total_bytes.0))
                .map_err(|_| MonitorError::InvalidData("Windows capacity percentage overflow"))?;
        filesystems.push(FilesystemSnapshot {
            source: row.source,
            mount: row.mount,
            total_bytes: row.total_bytes.0,
            used_bytes,
            available_bytes: row.available_bytes.0,
            used_percent,
        });
    }
    Ok((filesystems, state))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Processes {
    count: usize,
    processes: Vec<Process>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Process {
    pid: Decimal,
    name: String,
    #[serde(deserialize_with = "nullable")]
    working_set_bytes: Option<Decimal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sockets {
    count: usize,
    sockets: Vec<Socket>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Socket {
    protocol: String,
    address: String,
    port: Decimal,
    pid: Decimal,
}

fn pid(value: Decimal) -> MonitorResult<u32> {
    u32::try_from(value.0).map_err(|_| MonitorError::InvalidData("Windows PID overflow"))
}

fn address(value: &str) -> MonitorResult<String> {
    if value.is_empty() || value.len() > MAX_ADDRESS || !value.is_ascii() {
        return invalid("invalid Windows socket address");
    }
    if let Ok(address) = value.parse::<Ipv4Addr>() {
        return Ok(address.to_string());
    }
    let (host, scope) = match value.split_once('%') {
        None => (value, None),
        Some((host, scope)) => {
            let scope = u32::try_from(decimal(scope)?)
                .map_err(|_| MonitorError::InvalidData("Windows IPv6 scope overflow"))?;
            (host, Some(scope))
        }
    };
    let address = host
        .parse::<Ipv6Addr>()
        .map_err(|_| MonitorError::InvalidData("invalid Windows numeric socket address"))?;
    Ok(match scope {
        None => format!("[{address}]"),
        Some(scope) => format!("[{address}%{scope}]"),
    })
}

fn check_count(declared: usize, actual: usize, limit: usize) -> MonitorResult<()> {
    if declared > limit || actual > limit || declared != actual {
        return invalid("invalid Windows monitoring row count");
    }
    Ok(())
}

fn display_text(value: &str) -> MonitorResult<()> {
    if value.is_empty()
        || value.len() > MAX_TEXT
        || value.trim().is_empty()
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{2069}'
                        | '\u{feff}'
                        | '\u{fffd}'
                )
        })
    {
        return invalid("invalid Windows display text");
    }
    Ok(())
}

fn invalid<T>(message: &'static str) -> MonitorResult<T> {
    Err(MonitorError::InvalidData(message))
}

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
