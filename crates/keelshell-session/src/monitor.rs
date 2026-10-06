//! Linux resource and process inspection using bounded, explicit SSH commands.
//!
//! CPU fields follow <https://docs.kernel.org/filesystems/proc.html>: guest time
//! is already included in user/nice, so it is not added twice. Network rates use
//! remote uptime, not the latency of local UI refreshes. No agent is installed.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{ExecOutput, SessionError, SshSession};

mod disk;
pub use disk::{
    DiskCounters, DiskDevice, DiskIoError, DiskIoRates, DiskIoSnapshot, DiskRate,
    DiskRateUnavailable,
};

const MAX_OUTPUT: usize = 2 * 1024 * 1024;
const MAX_PROCESSES: usize = 20_000;
const SNAPSHOT_SCRIPT: &str = r#"LC_ALL=C; export LC_ALL
if [ "$(uname -s)" != Linux ]; then exit 64; fi
printf '@@KS:platform@@\nLinux\n@@KS:stat@@\n'
cat /proc/stat || exit 65
printf '\n@@KS:meminfo@@\n'
cat /proc/meminfo || exit 65
printf '\n@@KS:loadavg@@\n'
cat /proc/loadavg || exit 65
printf '\n@@KS:uptime@@\n'
cat /proc/uptime || exit 65
printf '\n@@KS:net@@\n'
cat /proc/net/dev || exit 65
printf '\n@@KS:df@@\n'
df -Pk || exit 65
printf '\n@@KS:bootid@@\n'
if [ -r /proc/sys/kernel/random/boot_id ]; then
  cat /proc/sys/kernel/random/boot_id 2>/dev/null || printf '\n!unavailable\n'
fi
printf '\n@@KS:diskstats@@\n'
if [ -r /proc/diskstats ]; then
  cat /proc/diskstats 2>/dev/null || printf '\n!unavailable\n'
fi
printf '\n@@KS:end@@\n'
"#;
const PROCESSES_SCRIPT: &str = r#"LC_ALL=C; export LC_ALL
if [ "$(uname -s)" != Linux ]; then exit 64; fi
ps -ww -eo pid=,user=,pcpu=,pmem=,args= || exit 65
"#;
const SOCKETS_SCRIPT: &str = r#"LC_ALL=C; export LC_ALL
if [ "$(uname -s)" != Linux ]; then exit 64; fi
command -v ss >/dev/null 2>&1 || exit 66
ss -H -lntupn || exit 65
"#;

/// Typed monitor failure; remote output is never interpolated into an error.
#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    /// The SSH transport did not complete successfully.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// The target OS or required command capability is not supported.
    #[error("monitoring is unsupported: {0}")]
    Unsupported(&'static str),
    /// A fixed command returned failure or did not provide an exit status.
    #[error("remote {operation} did not succeed (exit {exit_status:?})")]
    RemoteFailure {
        /// Name of the attempted operation, not user content.
        operation: &'static str,
        /// Exit status, absent when the connection closed without one.
        exit_status: Option<u32>,
    },
    /// An expected proc/command output format was absent or invalid.
    #[error("invalid monitoring data: {0}")]
    InvalidData(&'static str),
    /// Process identity changed, disappeared or cannot be inspected.
    #[error("process changed or exited; refresh and review it again")]
    ProcessChanged,
    /// A socket row changed between display and the explicit probe.
    #[error("socket changed; refresh and review it again")]
    SocketChanged,
    /// A PID outside the permitted numeric range was selected.
    #[error("only a numeric PID greater than 1 may receive SIGTERM")]
    InvalidPid,
    /// The reviewed process identity is older than one minute.
    #[error("process review expired; inspect and confirm it again")]
    ReviewExpired,
}

/// Result of resource/process inspection.
pub type MonitorResult<T> = std::result::Result<T, MonitorError>;

/// Aggregate CPU counters from `/proc/stat`, measured in kernel clock ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuCounters {
    /// user, nice, system, idle, iowait, irq, softirq and steal counters.
    pub ticks: [u64; 8],
}

/// Physical memory and swap counters, in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySnapshot {
    /// Total physical memory visible to the kernel.
    pub total_bytes: u64,
    /// Kernel estimate of available memory, or `None` on kernels without it.
    pub available_bytes: Option<u64>,
    /// Total configured swap, if reported.
    pub swap_total_bytes: Option<u64>,
    /// Unused swap, if reported.
    pub swap_free_bytes: Option<u64>,
}

impl MemorySnapshot {
    /// Used memory calculated as total minus the kernel's available estimate.
    pub fn used_bytes(&self) -> Option<u64> {
        self.total_bytes.checked_sub(self.available_bytes?)
    }
}

/// Cumulative network byte counters for one interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkCounters {
    /// Kernel interface name.
    pub name: String,
    /// Bytes received since interface initialization.
    pub received_bytes: u64,
    /// Bytes transmitted since interface initialization.
    pub transmitted_bytes: u64,
}

/// Mounted-filesystem capacity from `df -Pk`; this is not disk I/O throughput.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSnapshot {
    /// Filesystem source reported by df.
    pub source: String,
    /// Mounted directory; spaces are preserved.
    pub mount: String,
    /// Capacity in bytes, converted from df's 1024-byte blocks.
    pub total_bytes: u64,
    /// Used bytes.
    pub used_bytes: u64,
    /// Bytes available to unprivileged users.
    pub available_bytes: u64,
    /// Percentage printed by df; may exceed 100 for reserved-space scenarios.
    pub used_percent: u32,
}

/// A single real Linux collection. A snapshot alone has no CPU/network rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Unix boot timestamp from `/proc/stat`, used to reject cross-reboot deltas.
    pub boot_time: u64,
    /// Seconds since boot, from the target kernel.
    pub uptime_seconds: f64,
    /// One-, five- and fifteen-minute load averages.
    pub load_average: [f64; 3],
    /// Aggregate CPU counters.
    pub cpu: CpuCounters,
    /// Memory counters.
    pub memory: MemorySnapshot,
    /// Per-interface network counters.
    pub networks: Vec<NetworkCounters>,
    /// Mounted-filesystem capacities.
    pub filesystems: Vec<FilesystemSnapshot>,
    /// Optional block-device counters. Unsupported or invalid optional data
    /// does not discard otherwise valid CPU, memory, network or capacity data.
    pub disk_io: Result<DiskIoSnapshot, DiskIoError>,
}

/// Per-second network rates for one interface after two valid samples.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkRate {
    /// Kernel interface name.
    pub name: String,
    /// Received bytes per second; absent if the interface is new or counters reset.
    pub received_bytes_per_second: Option<f64>,
    /// Sent bytes per second; absent if the interface is new or counters reset.
    pub transmitted_bytes_per_second: Option<f64>,
}

/// Derived utilization. Missing rates are unavailable, not invented zero values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SampleRates {
    /// Aggregate busy CPU percentage, excluding idle and iowait.
    pub cpu_busy_percent: Option<f64>,
    /// Kernel iowait fraction, which is not a direct disk utilization measure.
    pub cpu_iowait_percent: Option<f64>,
    /// Per-interface transfer rates.
    pub networks: Vec<NetworkRate>,
    /// Individual block-device rates, without summing parent disks/partitions.
    pub disks: Vec<DiskRate>,
}

impl Snapshot {
    /// Parse the collector's versioned section markers and validate numeric fields.
    pub fn parse(input: &str) -> MonitorResult<Self> {
        if input.len() > MAX_OUTPUT {
            return Err(MonitorError::InvalidData("snapshot exceeds 2 MiB"));
        }
        let sections = sections(input)?;
        if required(&sections, "platform")?.trim() != "Linux" {
            return Err(MonitorError::Unsupported(
                "only Linux /proc targets are supported",
            ));
        }
        let stat = required(&sections, "stat")?;
        let cpu_line = stat
            .lines()
            .find(|line| line.starts_with("cpu "))
            .ok_or(MonitorError::InvalidData("missing aggregate CPU counters"))?;
        let counters: Vec<_> = cpu_line
            .split_whitespace()
            .skip(1)
            .take(8)
            .map(|value| number(value, "CPU counter"))
            .collect::<MonitorResult<_>>()?;
        if counters.len() < 4 {
            return Err(MonitorError::InvalidData("incomplete CPU counters"));
        }
        let mut ticks = [0; 8];
        ticks[..counters.len()].copy_from_slice(&counters);
        let boot_time = stat
            .lines()
            .find_map(|line| line.strip_prefix("btime "))
            .ok_or(MonitorError::InvalidData("missing boot timestamp"))?;
        let boot_time = number(boot_time.trim(), "boot timestamp")?;
        let uptime_seconds =
            finite_positive(first(required(&sections, "uptime")?, "uptime")?, "uptime")?;
        let values: Vec<_> = required(&sections, "loadavg")?
            .split_whitespace()
            .take(3)
            .map(|value| finite_positive(value, "load average"))
            .collect::<MonitorResult<_>>()?;
        let load_average: [f64; 3] = values
            .try_into()
            .map_err(|_| MonitorError::InvalidData("missing load averages"))?;
        Ok(Self {
            boot_time,
            uptime_seconds,
            load_average,
            cpu: CpuCounters { ticks },
            memory: parse_memory(required(&sections, "meminfo")?)?,
            networks: parse_networks(required(&sections, "net")?)?,
            filesystems: parse_filesystems(required(&sections, "df")?)?,
            disk_io: match (sections.get("bootid"), sections.get("diskstats")) {
                (Some(boot), Some(counters)) => DiskIoSnapshot::parse(boot, counters),
                _ => Err(DiskIoError::MissingData),
            },
        })
    }

    /// Compare with a previous sample. Reboots, non-forward time and decreasing
    /// counters invalidate affected rates instead of producing negative spikes.
    pub fn rates_since(&self, previous: &Self) -> SampleRates {
        let elapsed = self.uptime_seconds - previous.uptime_seconds;
        let mut result = SampleRates {
            disks: self.disk_io.as_ref().map_or_else(
                |_| Vec::new(),
                |current| {
                    current.rates_since(
                        previous.disk_io.as_ref().ok(),
                        elapsed,
                        self.boot_time == previous.boot_time,
                    )
                },
            ),
            ..SampleRates::default()
        };
        if self.boot_time != previous.boot_time || !elapsed.is_finite() || elapsed <= 0.0 {
            return result;
        }
        let deltas: Option<Vec<u64>> = self
            .cpu
            .ticks
            .iter()
            .zip(previous.cpu.ticks)
            .map(|(now, before)| now.checked_sub(before))
            .collect();
        if let Some(deltas) = deltas {
            let total: Option<u64> = deltas
                .iter()
                .try_fold(0_u64, |sum, delta| sum.checked_add(*delta));
            if let Some(total) = total.filter(|total| *total > 0) {
                let idle = deltas[3].saturating_add(deltas[4]);
                result.cpu_busy_percent =
                    Some((total.saturating_sub(idle)) as f64 / total as f64 * 100.0);
                result.cpu_iowait_percent = Some(deltas[4] as f64 / total as f64 * 100.0);
            }
        }
        result.networks = self
            .networks
            .iter()
            .map(|now| {
                let before = previous.networks.iter().find(|item| item.name == now.name);
                NetworkRate {
                    name: now.name.clone(),
                    received_bytes_per_second: before
                        .and_then(|before| now.received_bytes.checked_sub(before.received_bytes))
                        .map(|bytes| bytes as f64 / elapsed),
                    transmitted_bytes_per_second: before
                        .and_then(|before| {
                            now.transmitted_bytes.checked_sub(before.transmitted_bytes)
                        })
                        .map(|bytes| bytes as f64 / elapsed),
                }
            })
            .collect();
        result
    }
}

/// A process row from Linux `ps`; percentages are ps's reported values, not interval CPU samples.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    /// Numeric process identifier.
    pub pid: u32,
    /// Account name reported by ps (may be truncated by that implementation).
    pub user: String,
    /// ps's lifetime CPU percentage; may exceed 100 on multicore workloads.
    pub cpu_percent: f64,
    /// ps's physical-memory percentage.
    pub memory_percent: f64,
    /// Command and arguments, not a string to execute.
    pub command: String,
}

/// A listening TCP or UDP socket reported by the remote Linux host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketInfo {
    /// Transport protocol, currently `tcp` or `udp`.
    pub protocol: String,
    /// Kernel socket state, normally `LISTEN` or `UNCONN`.
    pub state: String,
    /// Local address and port as printed by `ss`.
    pub local: String,
    /// Peer address and port, if the protocol reports one.
    pub peer: String,
    /// Optional process metadata from `ss`, retained as display text.
    pub process: Option<String>,
}

/// Result of an explicit, read-only TCP connect check for a listening socket.
///
/// The probe sends no application bytes. `reachable` means that the remote
/// `nc` command completed the TCP handshake; it does not establish protocol
/// health, authentication, or readiness beyond the kernel accept path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpProbeResult {
    /// Address used by the remote probe. Wildcard listeners are mapped to the
    /// loopback address so the result is deterministic and does not leave the
    /// target host.
    pub host: String,
    /// TCP port checked by the remote probe.
    pub port: u16,
    /// Whether the TCP handshake was accepted.
    pub reachable: bool,
    /// Exit status returned by `nc`; zero means reachable.
    pub probe_status: u32,
}

/// An inspected process identity bound to its kernel start tick.
///
/// It is created only after rechecking a selected process and is valid for one
/// minute. Linux `kill` is still PID-based: rechecking immediately before it
/// reduces, but cannot eliminate, the check-to-signal PID reuse race.
pub struct ProcessIdentity {
    process: ProcessInfo,
    start_ticks: u64,
    inspected: Instant,
    connection: SshSession,
}

impl ProcessIdentity {
    /// The freshly inspected process row to show in the final confirmation.
    pub fn process(&self) -> &ProcessInfo {
        &self.process
    }
}

/// Read-only Linux collection with a separate, explicit SIGTERM operation.
#[derive(Clone)]
pub struct LinuxMonitor {
    session: SshSession,
}

impl LinuxMonitor {
    /// Bind monitoring to one authenticated SSH connection.
    pub fn new(session: SshSession) -> Self {
        Self { session }
    }

    /// Execute the fixed read-only `/proc` and `df -Pk` collector.
    pub async fn snapshot(&self) -> MonitorResult<Snapshot> {
        let output = self
            .session
            .exec_limited(SNAPSHOT_SCRIPT, MAX_OUTPUT)
            .await?;
        Snapshot::parse(checked_text(output, "resource collection")?.as_str())
    }

    /// Execute fixed ps columns and sort by descending reported CPU usage.
    pub async fn processes(&self) -> MonitorResult<Vec<ProcessInfo>> {
        let output = self
            .session
            .exec_limited(PROCESSES_SCRIPT, MAX_OUTPUT)
            .await?;
        if output.exit_status == Some(65) {
            return Err(MonitorError::Unsupported(
                "Linux ps must support pid,user,pcpu,pmem,args fields",
            ));
        }
        parse_processes(&checked_text(output, "process collection")?)
    }

    /// Collect listening TCP/UDP sockets using the fixed `ss` command.
    ///
    /// The result is diagnostic text only. It is never treated as an executable
    /// command and it does not imply that a service accepted a connection.
    pub async fn listening_sockets(&self) -> MonitorResult<Vec<SocketInfo>> {
        let output = self
            .session
            .exec_limited(SOCKETS_SCRIPT, MAX_OUTPUT)
            .await?;
        match output.exit_status {
            Some(66) => Err(MonitorError::Unsupported(
                "Linux ss is required for socket diagnostics",
            )),
            _ => parse_sockets(&checked_text(output, "socket collection")?),
        }
    }

    /// Probe one TCP listening row through the authenticated remote host.
    ///
    /// Only endpoint data parsed from a fresh `ss` row is accepted. The
    /// command requires `nc`, has a two-second connect timeout and emits a
    /// fixed marker; no user-provided shell text is executed and no payload is
    /// sent after the handshake. UDP rows are intentionally unsupported.
    pub async fn probe_tcp(&self, socket: &SocketInfo) -> MonitorResult<TcpProbeResult> {
        if socket.protocol != "tcp" {
            return Err(MonitorError::Unsupported(
                "only TCP listening sockets support a connect probe",
            ));
        }
        let current = self.listening_sockets().await?;
        if !current.iter().any(|candidate| candidate == socket) {
            return Err(MonitorError::SocketChanged);
        }
        let (host, port) = probe_endpoint(&socket.local)?;
        let command = format!(
            "LC_ALL=C; export LC_ALL\nif [ \"$(uname -s)\" != Linux ]; then exit 64; fi\ncommand -v nc >/dev/null 2>&1 || exit 66\nstatus=0\nnc -z -w 2 {host} {port} >/dev/null 2>&1 || status=$?\nprintf '@@KS:probe@@\\n%s\\n' \"$status\"\n",
            host = shell_literal(&host)
        );
        let output = self.session.exec_limited(&command, 4 * 1024).await?;
        match output.exit_status {
            Some(66) => Err(MonitorError::Unsupported(
                "Linux nc is required for TCP connect probes",
            )),
            _ => parse_probe(checked_text(output, "TCP probe")?.as_str(), host, port),
        }
    }

    /// Recheck a selected row and obtain its start tick before UI confirmation.
    pub async fn inspect_process(&self, selected: &ProcessInfo) -> MonitorResult<ProcessIdentity> {
        valid_pid(selected.pid)?;
        let pid = selected.pid;
        let command = format!(
            "LC_ALL=C; export LC_ALL\nif [ \"$(uname -s)\" != Linux ]; then exit 64; fi\ncat /proc/{pid}/stat || exit 65\nprintf '\\n@@KS:process@@\\n'\nps -ww -p {pid} -o pid=,user=,pcpu=,pmem=,args= || exit 65\n"
        );
        let output = self.session.exec_limited(&command, 64 * 1024).await?;
        if output.exit_status == Some(65) {
            return Err(MonitorError::ProcessChanged);
        }
        let text = checked_text(output, "process inspection")?;
        let (stat, ps) = text
            .split_once("\n@@KS:process@@\n")
            .ok_or(MonitorError::InvalidData(
                "missing process inspection marker",
            ))?;
        let start_ticks = process_start_ticks(stat.trim(), pid)?;
        let mut rows = parse_processes(ps)?;
        if rows.len() != 1 {
            return Err(MonitorError::ProcessChanged);
        }
        let process = rows.remove(0);
        if process.pid != pid
            || process.user != selected.user
            || process.command != selected.command
        {
            return Err(MonitorError::ProcessChanged);
        }
        Ok(ProcessIdentity {
            process,
            start_ticks,
            inspected: Instant::now(),
            connection: self.session.clone(),
        })
    }

    /// Send SIGTERM only after the UI confirms the exact inspected identity.
    ///
    /// The command interpolates only validated numeric PID/start ticks. It does
    /// not escalate privileges or fall back to SIGKILL. A timeout has an unknown
    /// outcome and must never trigger automatic retry.
    pub async fn terminate(&self, identity: ProcessIdentity) -> MonitorResult<()> {
        valid_pid(identity.process.pid)?;
        if !Arc::ptr_eq(&identity.connection.handle, &self.session.handle) {
            return Err(MonitorError::ProcessChanged);
        }
        if identity.inspected.elapsed() > Duration::from_secs(60) {
            return Err(MonitorError::ReviewExpired);
        }
        let command = format!(
            "pid={}; expected={}\nvalue=$(cat \"/proc/$pid/stat\") || exit 65\nvalue=${{value##*) }}\nset -- $value\n[ \"$#\" -ge 20 ] || exit 65\nshift 19\n[ \"$1\" = \"$expected\" ] || exit 67\nkill -TERM \"$pid\"\n",
            identity.process.pid, identity.start_ticks
        );
        let output = self.session.exec_limited(&command, 64 * 1024).await?;
        if matches!(output.exit_status, Some(65 | 67)) {
            return Err(MonitorError::ProcessChanged);
        }
        checked_text(output, "SIGTERM")?;
        Ok(())
    }
}

/// Parse headerless fixed ps columns, retaining all spaces in command arguments.
pub fn parse_processes(input: &str) -> MonitorResult<Vec<ProcessInfo>> {
    if input.len() > MAX_OUTPUT {
        return Err(MonitorError::InvalidData("process output exceeds 2 MiB"));
    }
    let mut processes = Vec::new();
    for line in input.lines().filter(|line| !line.trim().is_empty()) {
        if processes.len() == MAX_PROCESSES {
            return Err(MonitorError::InvalidData("process count exceeds 20000"));
        }
        let (fields, command) = columns(line, 4)?;
        let pid = fields[0]
            .parse::<u32>()
            .map_err(|_| MonitorError::InvalidData("process PID"))?;
        if pid == 0 {
            return Err(MonitorError::InvalidData("process PID must be positive"));
        }
        processes.push(ProcessInfo {
            pid,
            user: fields[1].to_owned(),
            cpu_percent: finite_positive(fields[2], "process CPU")?,
            memory_percent: finite_positive(fields[3], "process memory")?,
            command: command.to_owned(),
        });
    }
    processes.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then(a.pid.cmp(&b.pid))
    });
    Ok(processes)
}

/// Parse headerless `ss -H -lntup` output.
pub fn parse_sockets(input: &str) -> MonitorResult<Vec<SocketInfo>> {
    if input.len() > MAX_OUTPUT {
        return Err(MonitorError::InvalidData("socket output exceeds 2 MiB"));
    }
    let mut sockets = Vec::new();
    for line in input.lines().filter(|line| !line.trim().is_empty()) {
        if sockets.len() == 10_000 {
            return Err(MonitorError::InvalidData("socket count exceeds 10000"));
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 6 {
            return Err(MonitorError::InvalidData("incomplete socket row"));
        }
        let protocol = fields[0].to_ascii_lowercase();
        if protocol != "tcp" && protocol != "udp" {
            return Err(MonitorError::InvalidData("unsupported socket protocol"));
        }
        let process = (fields.len() > 6).then(|| fields[6..].join(" "));
        sockets.push(SocketInfo {
            protocol,
            state: fields[1].to_owned(),
            local: fields[4].to_owned(),
            peer: fields[5].to_owned(),
            process,
        });
    }
    Ok(sockets)
}

/// Parse the fixed probe marker and retain the `nc` status for review.
pub fn parse_probe(input: &str, host: String, port: u16) -> MonitorResult<TcpProbeResult> {
    if input.len() > 1024 {
        return Err(MonitorError::InvalidData("probe output exceeds 1 KiB"));
    }
    let status = input
        .strip_prefix("@@KS:probe@@\n")
        .and_then(|value| value.lines().next())
        .ok_or(MonitorError::InvalidData("missing probe marker"))?
        .parse::<u32>()
        .map_err(|_| MonitorError::InvalidData("probe status"))?;
    if status > 255 {
        return Err(MonitorError::InvalidData("probe status range"));
    }
    Ok(TcpProbeResult {
        host,
        port,
        reachable: status == 0,
        probe_status: status,
    })
}

fn probe_endpoint(local: &str) -> MonitorResult<(String, u16)> {
    let (host, port) = if let Some(rest) = local.strip_prefix('[') {
        let (host, port) = rest
            .split_once("]:")
            .ok_or(MonitorError::InvalidData("invalid IPv6 socket endpoint"))?;
        (host, port)
    } else {
        local
            .rsplit_once(':')
            .ok_or(MonitorError::InvalidData("invalid socket endpoint"))?
    };
    let port = port
        .parse::<u16>()
        .map_err(|_| MonitorError::InvalidData("socket port"))?;
    let host = match host {
        "*" | "0.0.0.0" => "127.0.0.1",
        "::" | "[::]" => "::1",
        value => value,
    };
    if host.is_empty()
        || host.starts_with('-')
        || host.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | ':' | '%' | '_' | '-'))
        })
    {
        return Err(MonitorError::InvalidData("unsafe socket address"));
    }
    Ok((host.to_owned(), port))
}

fn shell_literal(value: &str) -> String {
    // probe_endpoint excludes shell metacharacters; quotes still keep the
    // fixed command robust if a future endpoint parser admits a safe name.
    format!("'{value}'")
}

fn checked_text(output: ExecOutput, operation: &'static str) -> MonitorResult<String> {
    if output.exit_status == Some(64) {
        return Err(MonitorError::Unsupported(
            "only Linux targets are supported",
        ));
    }
    if output.exit_status != Some(0) {
        return Err(MonitorError::RemoteFailure {
            operation,
            exit_status: output.exit_status,
        });
    }
    String::from_utf8(output.stdout)
        .map_err(|_| MonitorError::InvalidData("non-UTF-8 command output"))
}

fn sections(input: &str) -> MonitorResult<BTreeMap<String, String>> {
    let mut sections = BTreeMap::new();
    let mut current = None;
    for line in input.lines() {
        if let Some(marker) = line
            .strip_prefix("@@KS:")
            .and_then(|line| line.strip_suffix("@@"))
        {
            if ![
                "platform",
                "stat",
                "meminfo",
                "loadavg",
                "uptime",
                "net",
                "df",
                "bootid",
                "diskstats",
                "end",
            ]
            .contains(&marker)
                || sections.contains_key(marker)
            {
                return Err(MonitorError::InvalidData(
                    "invalid or repeated section marker",
                ));
            }
            sections.insert(marker.to_owned(), String::new());
            current = Some(marker.to_owned());
        } else if let Some(section) = current.as_ref().and_then(|name| sections.get_mut(name)) {
            section.push_str(line);
            section.push('\n');
        } else if !line.trim().is_empty() {
            return Err(MonitorError::InvalidData("unexpected collector preamble"));
        }
    }
    if !sections.contains_key("end") {
        return Err(MonitorError::InvalidData("incomplete collector output"));
    }
    Ok(sections)
}

fn required<'a>(sections: &'a BTreeMap<String, String>, key: &str) -> MonitorResult<&'a str> {
    sections
        .get(key)
        .map(String::as_str)
        .ok_or(MonitorError::InvalidData("missing collector section"))
}

fn number(input: &str, field: &'static str) -> MonitorResult<u64> {
    input.parse().map_err(|_| MonitorError::InvalidData(field))
}

fn first<'a>(input: &'a str, field: &'static str) -> MonitorResult<&'a str> {
    input
        .split_whitespace()
        .next()
        .ok_or(MonitorError::InvalidData(field))
}

fn finite_positive(input: &str, field: &'static str) -> MonitorResult<f64> {
    let value: f64 = input
        .parse()
        .map_err(|_| MonitorError::InvalidData(field))?;
    if !value.is_finite() || value < 0.0 {
        return Err(MonitorError::InvalidData(field));
    }
    Ok(value)
}

fn parse_memory(input: &str) -> MonitorResult<MemorySnapshot> {
    let mut values = BTreeMap::new();
    for line in input.lines() {
        let Some((name, data)) = line.split_once(':') else {
            continue;
        };
        if !["MemTotal", "MemAvailable", "SwapTotal", "SwapFree"].contains(&name) {
            continue;
        }
        let fields: Vec<_> = data.split_whitespace().collect();
        if fields.len() != 2 || fields[1] != "kB" {
            return Err(MonitorError::InvalidData("memory unit must be kB"));
        }
        let value = number(fields[0], "memory counter")?
            .checked_mul(1024)
            .ok_or(MonitorError::InvalidData("memory overflow"))?;
        if values.insert(name, value).is_some() {
            return Err(MonitorError::InvalidData("duplicate memory counter"));
        }
    }
    let total_bytes = *values
        .get("MemTotal")
        .ok_or(MonitorError::InvalidData("missing total memory"))?;
    let available_bytes = values.get("MemAvailable").copied();
    if total_bytes == 0 || available_bytes.is_some_and(|value| value > total_bytes) {
        return Err(MonitorError::InvalidData("inconsistent memory counters"));
    }
    Ok(MemorySnapshot {
        total_bytes,
        available_bytes,
        swap_total_bytes: values.get("SwapTotal").copied(),
        swap_free_bytes: values.get("SwapFree").copied(),
    })
}

fn parse_networks(input: &str) -> MonitorResult<Vec<NetworkCounters>> {
    let mut networks = Vec::new();
    for line in input.lines().filter(|line| line.contains(':')) {
        let (name, counters) = line
            .rsplit_once(':')
            .ok_or(MonitorError::InvalidData("network row"))?;
        let fields: Vec<_> = counters.split_whitespace().collect();
        if name.trim().is_empty() || fields.len() < 16 {
            return Err(MonitorError::InvalidData("incomplete network row"));
        }
        networks.push(NetworkCounters {
            name: name.trim().to_owned(),
            received_bytes: number(fields[0], "received bytes")?,
            transmitted_bytes: number(fields[8], "transmitted bytes")?,
        });
    }
    Ok(networks)
}

fn parse_filesystems(input: &str) -> MonitorResult<Vec<FilesystemSnapshot>> {
    let mut lines = input.lines().filter(|line| !line.trim().is_empty());
    let header = lines
        .next()
        .ok_or(MonitorError::InvalidData("missing df header"))?;
    if !header.contains("1024-blocks") {
        return Err(MonitorError::InvalidData("df must report 1024-byte blocks"));
    }
    lines
        .map(|line| {
            let (fields, mount) = columns(line, 5)?;
            let bytes = |field| {
                number(field, "filesystem blocks")?
                    .checked_mul(1024)
                    .ok_or(MonitorError::InvalidData("filesystem size overflow"))
            };
            let used_percent = fields[4]
                .strip_suffix('%')
                .ok_or(MonitorError::InvalidData("filesystem percent"))?
                .parse()
                .map_err(|_| MonitorError::InvalidData("filesystem percent"))?;
            Ok(FilesystemSnapshot {
                source: fields[0].to_owned(),
                mount: mount.to_owned(),
                total_bytes: bytes(fields[1])?,
                used_bytes: bytes(fields[2])?,
                available_bytes: bytes(fields[3])?,
                used_percent,
            })
        })
        .collect()
}

fn columns(mut line: &str, count: usize) -> MonitorResult<(Vec<&str>, &str)> {
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        line = line.trim_start();
        let end = line
            .find(char::is_whitespace)
            .ok_or(MonitorError::InvalidData("incomplete row"))?;
        fields.push(&line[..end]);
        line = &line[end..];
    }
    let remainder = line.trim_start();
    if remainder.is_empty() {
        return Err(MonitorError::InvalidData("empty final column"));
    }
    Ok((fields, remainder))
}

fn valid_pid(pid: u32) -> MonitorResult<()> {
    if pid <= 1 || pid > i32::MAX as u32 {
        Err(MonitorError::InvalidPid)
    } else {
        Ok(())
    }
}

fn process_start_ticks(stat: &str, expected_pid: u32) -> MonitorResult<u64> {
    let (pid, _) = stat
        .split_once(" (")
        .ok_or(MonitorError::InvalidData("process stat PID"))?;
    if number(pid, "process stat PID")? != u64::from(expected_pid) {
        return Err(MonitorError::ProcessChanged);
    }
    let (_, fields) = stat
        .rsplit_once(") ")
        .ok_or(MonitorError::InvalidData("process stat command"))?;
    let start = fields
        .split_whitespace()
        .nth(19)
        .ok_or(MonitorError::InvalidData("process start tick"))?;
    number(start, "process start tick")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@@KS:platform@@\nLinux\n@@KS:stat@@\ncpu 100 20 30 400 50 0 0 0 80 10\ncpu0 1 1 1 1\nbtime 1700000000\n@@KS:meminfo@@\nMemTotal: 1024 kB\nMemAvailable: 256 kB\nSwapTotal: 512 kB\nSwapFree: 128 kB\n@@KS:loadavg@@\n1.25 0.50 0.25 1/100 45\n@@KS:uptime@@\n100.00 75.00\n@@KS:net@@\nInter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 100 1 0 0 0 0 0 0 200 1 0 0 0 0 0 0\neth0: 1000 10 0 0 0 0 0 0 2000 10 0 0 0 0 0 0\n@@KS:df@@\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 600 350 64% /\ntmpfs 200 1 199 1% /with spaces\n@@KS:end@@\n";
    const PROCESS_ROWS: &str =
        "1 root 0.0 0.1 /sbin/init\n42 tester 125.0 2.5 worker --arg 'two  spaces'\n";
    const SOCKET_ROWS: &str = "tcp LISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=42,fd=3))\nudp UNCONN 0 0 [::1]:5353 [::]:* users:((\"dns\",pid=7,fd=4))\n";

    #[test]
    fn resource_parser_preserves_units_and_mount_spaces() -> MonitorResult<()> {
        let sample = Snapshot::parse(SAMPLE)?;
        assert_eq!(sample.memory.used_bytes(), Some(768 * 1024));
        assert_eq!(sample.filesystems[1].mount, "/with spaces");
        assert_eq!(sample.filesystems[0].total_bytes, 1000 * 1024);
        assert_eq!(sample.networks[1].transmitted_bytes, 2000);
        assert_eq!(sample.load_average, [1.25, 0.5, 0.25]);
        Ok(())
    }

    #[test]
    fn cpu_delta_excludes_guest_double_counting_and_iowait() -> MonitorResult<()> {
        let before = Snapshot::parse(SAMPLE)?;
        let after = Snapshot::parse(
            &SAMPLE
                .replace(
                    "cpu 100 20 30 400 50 0 0 0 80 10",
                    "cpu 120 20 50 450 60 0 0 0 100 10",
                )
                .replace("100.00 75.00", "105.00 78.00"),
        )?;
        let rates = after.rates_since(&before);
        assert_eq!(rates.cpu_busy_percent, Some(40.0));
        assert_eq!(rates.cpu_iowait_percent, Some(10.0));
        Ok(())
    }

    #[test]
    fn network_rates_use_remote_elapsed_time() -> MonitorResult<()> {
        let before = Snapshot::parse(SAMPLE)?;
        let mut after = before.clone();
        after.uptime_seconds += 5.0;
        after.networks[1].received_bytes += 500;
        after.networks[1].transmitted_bytes += 1000;
        let rates = after.rates_since(&before);
        assert_eq!(rates.networks[1].received_bytes_per_second, Some(100.0));
        assert_eq!(rates.networks[1].transmitted_bytes_per_second, Some(200.0));
        Ok(())
    }

    #[test]
    fn resets_and_reboots_produce_unavailable_rates() -> MonitorResult<()> {
        let before = Snapshot::parse(SAMPLE)?;
        let mut after = before.clone();
        after.uptime_seconds += 5.0;
        after.cpu.ticks[0] = 0;
        after.networks[0].received_bytes = 0;
        let rates = after.rates_since(&before);
        assert_eq!(rates.cpu_busy_percent, None);
        assert_eq!(rates.networks[0].received_bytes_per_second, None);
        after.boot_time += 1;
        assert_eq!(after.rates_since(&before), SampleRates::default());
        Ok(())
    }

    #[test]
    fn equal_timestamps_do_not_invent_zero_utilization() -> MonitorResult<()> {
        let sample = Snapshot::parse(SAMPLE)?;
        assert_eq!(sample.rates_since(&sample), SampleRates::default());
        Ok(())
    }

    #[test]
    fn missing_available_memory_is_not_confused_with_free_memory() -> MonitorResult<()> {
        let sample = Snapshot::parse(&SAMPLE.replace("MemAvailable: 256 kB", "MemFree: 128 kB"))?;
        assert_eq!(sample.memory.used_bytes(), None);
        Ok(())
    }

    #[test]
    fn malformed_truncated_and_nonfinite_snapshots_are_rejected() {
        for input in [
            SAMPLE.replace("@@KS:end@@", ""),
            SAMPLE.replace("100.00 75.00", "NaN 75.00"),
            SAMPLE.replace("MemTotal: 1024 kB", "MemTotal: 18446744073709551615 kB"),
            SAMPLE.replace("1.25 0.50 0.25", "-1.0 0.50 0.25"),
        ] {
            assert!(Snapshot::parse(&input).is_err());
        }
    }

    #[test]
    fn non_linux_platform_is_explicitly_unsupported() {
        assert!(matches!(
            Snapshot::parse(&SAMPLE.replace("\nLinux\n", "\nDarwin\n")),
            Err(MonitorError::Unsupported(_))
        ));
    }

    #[test]
    fn optional_disk_absence_and_invalid_data_preserve_original_resources() -> MonitorResult<()> {
        let original = Snapshot::parse(SAMPLE)?;
        assert_eq!(original.disk_io, Err(DiskIoError::MissingData));
        for extra in [
            "@@KS:bootid@@\n\n@@KS:diskstats@@\n",
            "@@KS:bootid@@\n11ac8d57-72c6-4ee6-93bf-68724d27e715\n@@KS:diskstats@@\n8 0 sda 1 2 3 4\n",
            "@@KS:bootid@@\n11ac8d57-72c6-4ee6-93bf-68724d27e715\n@@KS:diskstats@@\n!unavailable\n",
        ] {
            let parsed =
                Snapshot::parse(&SAMPLE.replace("@@KS:end@@", &format!("{extra}@@KS:end@@")))?;
            assert!(parsed.disk_io.is_err());
            assert_eq!(parsed.cpu, original.cpu);
            assert_eq!(parsed.memory, original.memory);
            assert_eq!(parsed.networks, original.networks);
            assert_eq!(parsed.filesystems, original.filesystems);
        }
        Ok(())
    }

    #[test]
    fn ps_parser_preserves_command_spacing_and_multicore_cpu() -> MonitorResult<()> {
        let rows = parse_processes(PROCESS_ROWS)?;
        assert_eq!(rows[0].pid, 42);
        assert_eq!(rows[0].cpu_percent, 125.0);
        assert_eq!(rows[0].command, "worker --arg 'two  spaces'");
        Ok(())
    }

    #[test]
    fn ps_parser_rejects_negative_pid_and_nonfinite_percentages() {
        for row in [
            "-1 root 0 0 command",
            "0 root 0 0 command",
            "42 root NaN 0 command",
            "42 root 0 inf command",
        ] {
            assert!(parse_processes(row).is_err());
        }
        assert!(valid_pid(0).is_err());
        assert!(valid_pid(1).is_err());
        assert!(valid_pid(u32::MAX).is_err());
    }

    #[test]
    fn socket_parser_retains_addresses_and_process_metadata() -> MonitorResult<()> {
        let rows = parse_sockets(SOCKET_ROWS)?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].protocol, "tcp");
        assert_eq!(rows[0].state, "LISTEN");
        assert_eq!(rows[0].local, "0.0.0.0:22");
        assert_eq!(rows[0].peer, "0.0.0.0:*");
        assert_eq!(
            rows[0].process.as_deref(),
            Some("users:((\"sshd\",pid=42,fd=3))")
        );
        assert_eq!(rows[1].local, "[::1]:5353");
        Ok(())
    }

    #[test]
    fn socket_parser_rejects_unknown_protocol_and_truncated_rows() {
        assert!(matches!(
            parse_sockets("unix LISTEN 0 1 /tmp/socket *"),
            Err(MonitorError::InvalidData("unsupported socket protocol"))
        ));
        assert!(matches!(
            parse_sockets("tcp LISTEN 0 1 127.0.0.1:22"),
            Err(MonitorError::InvalidData("incomplete socket row"))
        ));
    }

    fn process_stat() -> String {
        format!(
            "42 (worker with ) spaces) S {} 12345 0 0",
            std::iter::repeat_n("0", 18).collect::<Vec<_>>().join(" ")
        )
    }

    #[test]
    fn process_starttime_handles_parentheses_inside_the_comm_field() -> MonitorResult<()> {
        assert_eq!(process_start_ticks(&process_stat(), 42)?, 12345);
        assert!(matches!(
            process_start_ticks(&process_stat(), 43),
            Err(MonitorError::ProcessChanged)
        ));
        Ok(())
    }

    #[test]
    fn missing_exit_status_is_not_success() {
        let output = ExecOutput {
            stdout: SAMPLE.as_bytes().to_vec(),
            stderr: Vec::new(),
            exit_status: None,
        };
        assert!(matches!(
            checked_text(output, "test"),
            Err(MonitorError::RemoteFailure {
                exit_status: None,
                ..
            })
        ));
    }

    #[derive(Clone)]
    struct Fixture {
        commands: Arc<std::sync::Mutex<Vec<String>>>,
        unsupported: bool,
    }

    impl russh::server::Handler for Fixture {
        type Error = russh::Error;

        async fn auth_password(
            &mut self,
            user: &str,
            password: &str,
        ) -> Result<russh::server::Auth, Self::Error> {
            Ok(if user == "fixture" && password == "fixture-password" {
                russh::server::Auth::Accept
            } else {
                russh::server::Auth::reject()
            })
        }

        async fn channel_open_session(
            &mut self,
            _: russh::Channel<russh::server::Msg>,
            reply: russh::server::ChannelOpenHandle,
            _: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn exec_request(
            &mut self,
            channel: russh::ChannelId,
            data: &[u8],
            session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            let command = String::from_utf8_lossy(data).into_owned();
            if let Ok(mut commands) = self.commands.lock() {
                commands.push(command.clone());
            }
            session.channel_success(channel)?;
            let (text, status) = if self.unsupported {
                (String::new(), 64)
            } else if command == SNAPSHOT_SCRIPT {
                (SAMPLE.to_owned(), 0)
            } else if command == PROCESSES_SCRIPT {
                (PROCESS_ROWS.to_owned(), 0)
            } else if command == SOCKETS_SCRIPT {
                (
                    "tcp LISTEN 0 128 127.0.0.1:2222 0.0.0.0:* users:((\"fixture\",pid=42,fd=3))\n"
                        .into(),
                    0,
                )
            } else if command.contains("command -v nc") {
                ("@@KS:probe@@\n0\n".into(), 0)
            } else if command.contains("@@KS:process@@") {
                (
                    format!(
                        "{}\n@@KS:process@@\n42 tester 125.0 2.5 worker --arg 'two  spaces'\n",
                        process_stat()
                    ),
                    0,
                )
            } else if command.starts_with("pid=42; expected=12345\n") {
                (String::new(), 0)
            } else {
                (String::new(), 66)
            };
            if !text.is_empty() {
                session.data(channel, text.into_bytes())?;
            }
            session.exit_status_request(channel, status)?;
            session.eof(channel)?;
            session.close(channel)?;
            Ok(())
        }
    }

    struct FixtureServer {
        task: tokio::task::JoinHandle<()>,
        commands: Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl Drop for FixtureServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn fixture(
        unsupported: bool,
    ) -> Result<(FixtureServer, LinuxMonitor), Box<dyn std::error::Error>> {
        use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(russh::server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let handler = Fixture {
            commands: commands.clone(),
            unsupported,
        };
        let task = tokio::spawn(async move {
            let accepted = tokio::time::timeout(Duration::from_secs(5), listener.accept()).await;
            if let Ok(Ok((socket, _))) = accepted
                && let Ok(session) = russh::server::run_stream(config, socket, handler).await
            {
                let _ = session.await;
            }
        });
        let options = crate::SshOptions {
            host: address.ip().to_string(),
            port: address.port(),
            username: "fixture".into(),
            proxy: None,
            expected_host_key: Some(fingerprint),
            auth: crate::SshAuth::Password(zeroize::Zeroizing::new("fixture-password".into())),
            timeout: Duration::from_secs(3),
        };
        let monitor = LinuxMonitor::new(SshSession::connect(options).await?);
        Ok((FixtureServer { task, commands }, monitor))
    }

    #[tokio::test]
    async fn loopback_ssh_collects_real_packets_and_does_not_signal_by_default()
    -> Result<(), Box<dyn std::error::Error>> {
        let (server, monitor) = fixture(false).await?;
        let sample = monitor.snapshot().await?;
        let processes = monitor.processes().await?;
        assert_eq!(sample.memory.total_bytes, 1024 * 1024);
        let identity = monitor.inspect_process(&processes[0]).await?;
        assert_eq!(identity.process().pid, 42);
        {
            let commands = server
                .commands
                .lock()
                .map_err(|_| "fixture lock poisoned")?;
            assert_eq!(commands.len(), 3);
            assert!(
                commands
                    .iter()
                    .all(|command| !command.contains("kill -TERM"))
            );
        }

        monitor.terminate(identity).await?;
        let commands = server
            .commands
            .lock()
            .map_err(|_| "fixture lock poisoned")?;
        assert_eq!(commands.len(), 4);
        assert!(commands[3].starts_with("pid=42; expected=12345\n"));
        assert!(commands[3].contains("kill -TERM \"$pid\""));
        Ok(())
    }

    #[tokio::test]
    async fn loopback_ssh_tcp_probe_is_bounded_and_does_not_send_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let (server, monitor) = fixture(false).await?;
        let socket = SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "0.0.0.0:*".into(),
            process: Some("users:((\"fixture\",pid=42,fd=3))".into()),
        };
        let result = monitor.probe_tcp(&socket).await?;
        assert!(result.reachable);
        assert_eq!(result.port, 2222);
        {
            let commands = server
                .commands
                .lock()
                .map_err(|_| "fixture lock poisoned")?;
            assert_eq!(commands.len(), 2);
            assert_eq!(commands[0], SOCKETS_SCRIPT);
            assert!(commands[1].contains("nc -z -w 2 '127.0.0.1' 2222"));
            assert!(!commands[1].contains("echo "));
        }
        let stale = SocketInfo {
            process: None,
            ..socket
        };
        assert!(matches!(
            monitor.probe_tcp(&stale).await,
            Err(MonitorError::SocketChanged)
        ));
        let commands = server
            .commands
            .lock()
            .map_err(|_| "fixture lock poisoned")?;
        assert_eq!(commands.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn loopback_ssh_reports_unsupported_platform() -> Result<(), Box<dyn std::error::Error>> {
        let (_server, monitor) = fixture(true).await?;
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::Unsupported(_))
        ));
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::Unsupported(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn loopback_ssh_rejects_wrong_connection_expired_review_and_protected_pid()
    -> Result<(), Box<dyn std::error::Error>> {
        let (server, monitor) = fixture(false).await?;
        let (other_server, other_monitor) = fixture(false).await?;
        let processes = monitor.processes().await?;
        let identity = monitor.inspect_process(&processes[0]).await?;
        assert!(matches!(
            other_monitor.terminate(identity).await,
            Err(MonitorError::ProcessChanged)
        ));
        assert!(
            other_server
                .commands
                .lock()
                .map_err(|_| "fixture lock poisoned")?
                .is_empty()
        );
        let mut identity = monitor.inspect_process(&processes[0]).await?;
        identity.inspected = Instant::now() - Duration::from_secs(61);
        assert!(matches!(
            monitor.terminate(identity).await,
            Err(MonitorError::ReviewExpired)
        ));
        let mut protected = processes[0].clone();
        protected.pid = 1;
        assert!(matches!(
            monitor.inspect_process(&protected).await,
            Err(MonitorError::InvalidPid)
        ));
        let commands = server
            .commands
            .lock()
            .map_err(|_| "fixture lock poisoned")?;
        assert_eq!(commands.len(), 3);
        assert!(
            commands
                .iter()
                .all(|command| !command.contains("kill -TERM"))
        );
        Ok(())
    }

    #[test]
    fn tcp_probe_parses_reachability_and_rejects_unsafe_endpoints() -> MonitorResult<()> {
        let reachable = parse_probe("@@KS:probe@@\n0\n", "127.0.0.1".into(), 8080)?;
        assert_eq!(reachable.probe_status, 0);
        assert!(reachable.reachable);
        let refused = parse_probe("@@KS:probe@@\n111\n", "127.0.0.1".into(), 8080)?;
        assert!(!refused.reachable);
        assert_eq!(refused.probe_status, 111);

        let socket = SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:8080".into(),
            peer: "0.0.0.0:*".into(),
            process: None,
        };
        assert_eq!(probe_endpoint(&socket.local)?, ("127.0.0.1".into(), 8080));
        assert_eq!(probe_endpoint("0.0.0.0:22")?, ("127.0.0.1".into(), 22));
        assert_eq!(probe_endpoint("[::]:22")?, ("::1".into(), 22));
        assert!(probe_endpoint("127.0.0.1:22;id").is_err());
        assert!(probe_endpoint("-dash:22").is_err());
        assert!(matches!(
            parse_probe("@@KS:probe@@\n256\n", "127.0.0.1".into(), 8080),
            Err(MonitorError::InvalidData("probe status range"))
        ));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "manual Linux acceptance: requires responsive local /proc and mounts"]
    fn collector_executes_against_real_linux_proc_and_ps() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", SNAPSHOT_SCRIPT])
            .output()?;
        assert!(output.status.success());
        let sample = Snapshot::parse(std::str::from_utf8(&output.stdout)?)?;
        assert!(sample.memory.total_bytes > 0);
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", PROCESSES_SCRIPT])
            .output()?;
        assert!(output.status.success());
        assert!(!parse_processes(std::str::from_utf8(&output.stdout)?)?.is_empty());
        Ok(())
    }
}
