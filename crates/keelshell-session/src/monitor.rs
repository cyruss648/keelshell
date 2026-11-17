//! Host resource and process inspection using bounded, explicit SSH commands.
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

mod capabilities;
pub use capabilities::{
    Capability, CapabilityObservation, CapabilityReason, CapabilityReport, CapabilityState,
    MonitorCapabilities,
};
use capabilities::{CapabilityCache, cached_reason, command_text, error_state, failure_reason};
mod darwin;
mod darwin_sockets;
mod disk;
mod platform;
mod windows;
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
df_status=0
if command -v df >/dev/null 2>&1; then
  df -Pk || df_status=$?
else
  df_status=127
fi
printf '\n@@KS:df-status@@\n%s\n' "$df_status"
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
const PLATFORM_SELECTOR: &str = r#"ks_uname=''
for ks_candidate in /usr/bin/uname /bin/uname; do
  if [ -f "$ks_candidate" ] && [ -x "$ks_candidate" ]; then ks_uname="$ks_candidate"; break; fi
done
[ -n "$ks_uname" ] || exit 66
"#;
const PROCESSES_SCRIPT: &str = r#"LC_ALL=C; export LC_ALL
ks_uname=''
for ks_candidate in /usr/bin/uname /bin/uname; do
  if [ -f "$ks_candidate" ] && [ -x "$ks_candidate" ]; then ks_uname="$ks_candidate"; break; fi
done
[ -n "$ks_uname" ] || exit 66
case "$("$ks_uname" -s)" in
  Linux) ;;
  Darwin)
    [ -x /bin/ps ] || exit 66
    /bin/ps -ww -axo pid=,user=,pcpu=,pmem=,args= || exit 65
    exit 0
    ;;
  *) exit 64 ;;
esac
command -v ps >/dev/null 2>&1 || exit 66
ps -ww -eo pid=,user=,pcpu=,pmem=,args= || exit 65
"#;
const SOCKETS_SCRIPT: &str = r#"LC_ALL=C; export LC_ALL
ks_uname=''
for ks_candidate in /usr/bin/uname /bin/uname; do
  if [ -f "$ks_candidate" ] && [ -x "$ks_candidate" ]; then ks_uname="$ks_candidate"; break; fi
done
[ -n "$ks_uname" ] || exit 66
case "$("$ks_uname" -s)" in
  Linux) ;;
  Darwin)
    [ -f /usr/sbin/lsof ] && [ -x /usr/sbin/lsof ] || exit 66
    printf '@@KS:darwin-sockets:2@@\n'
    /usr/sbin/lsof -nP -a -iTCP -sTCP:LISTEN -iUDP -F0pcftPnT -Ts
    ks_lsof_status=$?
    printf '@@KS:lsof-status@@\n%s\n@@KS:end@@\n' "$ks_lsof_status"
    exit 0
    ;;
  *) exit 64 ;;
esac
command -v ss >/dev/null 2>&1 || exit 66
printf '@@KS:linux-sockets:1@@\n'
ss -H -lntupn
ks_ss_status=$?
printf '@@KS:ss-status@@\n%s\n@@KS:end@@\n' "$ks_ss_status"
"#;
const DF_COLLECTION: &str = r#"df_status=0
if command -v df >/dev/null 2>&1; then
  df -Pk || df_status=$?
else
  df_status=127
fi
printf '\n@@KS:df-status@@\n%s\n' "$df_status""#;

/// Typed monitor failure; remote output is never interpolated into an error.
#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    /// The SSH transport did not complete successfully.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// The target OS or required command capability is not supported.
    #[error("monitoring is unsupported: {0}")]
    Unsupported(&'static str),
    /// An independent facility failed; other monitoring can remain available.
    #[error("{capability} is unavailable: {reason}")]
    CapabilityUnavailable {
        /// Facility that was checked.
        capability: Capability,
        /// Typed reason, without untrusted remote diagnostic text.
        reason: CapabilityReason,
    },
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
    /// Neither fixed platform probe established a supported execution environment.
    #[error("remote monitoring platform and runtime could not be confirmed")]
    EnvironmentUnconfirmed,
    /// A pause, reset or retirement revoked this collection before publication.
    #[error("monitoring collection authority was revoked")]
    Cancelled,
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

/// Provenance of a validated resource collection, never the local application's OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorPlatform {
    /// Linux `/proc` counters with optional fixed system commands.
    Linux,
    /// macOS Mach counters, sysctl and fixed system commands.
    Darwin,
    /// Windows, collected with the explicitly observed Windows PowerShell runtime.
    Windows,
}

/// Runtime independently observed over this authenticated connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRuntime {
    /// A POSIX shell with a validated absolute-path `uname` identity probe.
    Posix,
    /// Desktop Windows PowerShell 5.1, including its observed version components.
    WindowsPowerShell {
        /// Major, minor, build and revision from the fixed identity probe.
        version: [u32; 4],
    },
}

/// Validated remote platform and runtime, independent of resource availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorEnvironment {
    /// The remote operating system; never inferred from the application's OS.
    pub platform: MonitorPlatform,
    /// The execution runtime that completed a fixed identity probe.
    pub runtime: MonitorRuntime,
}

/// Aggregate CPU counters, measured in the remote kernel's clock ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuCounters {
    /// user, nice, system, idle, iowait, irq, softirq and steal counters.
    /// Darwin supplies its four Mach states in the first four slots; the last
    /// four have no Darwin equivalent and are excluded from iowait reporting.
    pub ticks: [u64; 8],
}

/// CPU measurements retain their source instead of inventing counter values.
#[derive(Debug, Clone, PartialEq)]
pub enum CpuSample {
    /// The collector could not obtain a CPU measurement for this observation.
    Unavailable,
    /// Cumulative Linux or Darwin clock ticks, requiring two forward samples.
    Counters(CpuCounters),
    /// A provider-reported busy percentage, independent of local refresh timing.
    ///
    /// Windows uses the logical-processor-weighted `Win32_Processor`
    /// `LoadPercentage`, whose provider reports its preceding one-second mean.
    ReportedPercent(f64),
}

impl CpuSample {
    /// Read kernel counters only when the source actually supplies them.
    pub fn cumulative(&self) -> Option<&CpuCounters> {
        match self {
            Self::Counters(counters) => Some(counters),
            Self::Unavailable | Self::ReportedPercent(_) => None,
        }
    }

    /// Read a finite provider percentage, including a legitimate idle zero.
    pub fn reported_percent(&self) -> Option<f64> {
        match self {
            Self::ReportedPercent(value) if value.is_finite() && (0.0..=100.0).contains(value) => {
                Some(*value)
            }
            _ => None,
        }
    }
}

/// Physical memory and swap counters, in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySnapshot {
    /// Total physical memory visible to the kernel.
    pub total_bytes: u64,
    /// Kernel estimate of available memory, or `None` on kernels without it.
    pub available_bytes: Option<u64>,
    /// Free physical pages in bytes when supplied by the kernel. Free pages
    /// are not a reclaimable-memory estimate and never populate `available_bytes`.
    pub free_bytes: Option<u64>,
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

/// Mounted-filesystem capacity; this is not disk I/O throughput.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSnapshot {
    /// Filesystem source reported by df, or a Windows fixed-volume root.
    pub source: String,
    /// Mounted directory; spaces are preserved.
    pub mount: String,
    /// Capacity in bytes; Unix df's 1024-byte blocks are converted explicitly.
    pub total_bytes: u64,
    /// Used bytes.
    pub used_bytes: u64,
    /// Bytes available to unprivileged users.
    pub available_bytes: u64,
    /// Percentage printed by df, or derived from Windows total/free bytes.
    /// Unix values may exceed 100 for reserved-space scenarios.
    pub used_percent: u32,
}

/// A single real host collection with explicit metric provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Remote operating system and counter provenance.
    pub platform: MonitorPlatform,
    /// Unix boot timestamp, used to reject cross-reboot deltas.
    pub boot_time: u64,
    /// Seconds since boot. Windows supplies a same-CIM-object wall-clock estimate.
    pub uptime_seconds: f64,
    /// One-, five- and fifteen-minute load averages, absent on Windows.
    pub load_average: Option<[f64; 3]>,
    /// Aggregate CPU counters or an explicitly provider-reported percentage.
    pub cpu: CpuSample,
    /// Memory counters.
    pub memory: MemorySnapshot,
    /// Per-interface network counters.
    pub networks: Vec<NetworkCounters>,
    /// Mounted-filesystem capacities. Empty when capacity could not be collected;
    /// consult [`LinuxMonitor::capabilities`] to distinguish absence from failure.
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
    /// Optional filesystem failure does not discard valid core resources.
    pub fn parse(input: &str) -> MonitorResult<Self> {
        Self::parse_with_capacity(input, &[]).map(|(snapshot, _)| snapshot)
    }

    fn parse_with_capacity(input: &str, stderr: &[u8]) -> MonitorResult<(Self, CapabilityState)> {
        if input.len() > MAX_OUTPUT {
            return Err(MonitorError::InvalidData("snapshot exceeds 2 MiB"));
        }
        if input.starts_with('{') {
            return windows::parse_resources(ExecOutput {
                stdout: input.as_bytes().to_vec(),
                stderr: stderr.to_vec(),
                exit_status: Some(0),
            });
        }
        if input.starts_with("@@KS:darwin@@\n") {
            return darwin::parse(input, stderr);
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
        let (filesystems, capacity) = parse_capacity(&sections, stderr);
        Ok((
            Self {
                platform: MonitorPlatform::Linux,
                boot_time,
                uptime_seconds,
                load_average: Some(load_average),
                cpu: CpuSample::Counters(CpuCounters { ticks }),
                memory: parse_memory(required(&sections, "meminfo")?)?,
                networks: parse_networks(required(&sections, "net")?)?,
                filesystems,
                disk_io: match (sections.get("bootid"), sections.get("diskstats")) {
                    (Some(boot), Some(counters)) => DiskIoSnapshot::parse(boot, counters),
                    _ => Err(DiskIoError::MissingData),
                },
            },
            capacity,
        ))
    }

    /// Compare with a previous sample. Reboots, non-forward time and decreasing
    /// counters invalidate affected rates instead of producing negative spikes.
    pub fn rates_since(&self, previous: &Self) -> SampleRates {
        let elapsed = self.uptime_seconds - previous.uptime_seconds;
        // Windows network counters are real cumulative observations, but its
        // wall-clock uptime estimate is not a monotonic rate denominator.
        if self.platform == MonitorPlatform::Windows {
            return SampleRates {
                cpu_busy_percent: self.cpu.reported_percent(),
                ..SampleRates::default()
            };
        }
        let mut result = SampleRates {
            disks: self.disk_io.as_ref().map_or_else(
                |_| Vec::new(),
                |current| {
                    current.rates_since(
                        previous.disk_io.as_ref().ok(),
                        elapsed,
                        self.platform == previous.platform && self.boot_time == previous.boot_time,
                    )
                },
            ),
            ..SampleRates::default()
        };
        if self.platform != previous.platform
            || self.boot_time != previous.boot_time
            || !elapsed.is_finite()
            || elapsed <= 0.0
        {
            return result;
        }
        let deltas: Option<Vec<u64>> = self
            .cpu
            .cumulative()
            .zip(previous.cpu.cumulative())
            .and_then(|(now, before)| {
                now.ticks
                    .iter()
                    .zip(before.ticks)
                    .map(|(now, before)| now.checked_sub(before))
                    .collect()
            });
        if let Some(deltas) = deltas {
            let total: Option<u64> = deltas
                .iter()
                .try_fold(0_u64, |sum, delta| sum.checked_add(*delta));
            if let Some(total) = total.filter(|total| *total > 0) {
                let idle = deltas[3].saturating_add(deltas[4]);
                result.cpu_busy_percent =
                    Some((total.saturating_sub(idle)) as f64 / total as f64 * 100.0);
                if self.platform == MonitorPlatform::Linux {
                    result.cpu_iowait_percent = Some(deltas[4] as f64 / total as f64 * 100.0);
                }
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

/// A bounded process row from the platform's fixed read-only interface.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    /// Numeric process identifier.
    pub pid: u32,
    /// Account name reported by ps (may be truncated); absent on Windows.
    pub user: Option<String>,
    /// ps's lifetime CPU percentage; may exceed 100 on multicore workloads.
    /// Windows CPU seconds are not a percentage and leave this field absent.
    pub cpu_percent: Option<f64>,
    /// ps's physical-memory percentage, absent on Windows.
    pub memory_percent: Option<f64>,
    /// Observed working-set bytes, absent when not supplied or not readable.
    pub working_set_bytes: Option<u64>,
    /// Unix command/arguments or Windows ProcessName, never a string to execute.
    pub command: String,
}

/// A socket visible to the authenticated remote account.
///
/// Linux supplies TCP/UDP rows from `ss`; Darwin supplies TCP LISTEN and bound
/// UDP rows from system `lsof`. Windows supplies only TCP LISTEN and UDP bound
/// endpoints through its read-only networking APIs. No collector proves
/// whole-host visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketInfo {
    /// Transport protocol, currently `tcp` or `udp`.
    pub protocol: String,
    /// Kernel state from `ss`, or `LISTEN` for Darwin TCP. Darwin UDP uses
    /// `BOUND` or `PEER` to describe its observed address, not a kernel state,
    /// handshake, reachability or service health.
    pub state: String,
    /// Numeric local address and port; Darwin wildcards are normalized by family.
    pub local: String,
    /// Peer address and port, if the protocol reports one.
    pub peer: String,
    /// Optional bounded process metadata, retained only as display text.
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

/// Read-only host collection with a separate Linux-only SIGTERM operation.
///
/// The historical name remains source-compatible. Resource collection accepts
/// Linux, Darwin and the confirmed Windows PowerShell runtime. Connect probes
/// and reviewed process termination retain their Linux-specific contracts.
#[derive(Clone)]
pub struct LinuxMonitor {
    session: SshSession,
    capabilities: Arc<CapabilityCache>,
}

impl LinuxMonitor {
    /// Bind monitoring to one authenticated SSH connection.
    pub fn new(session: SshSession) -> Self {
        Self {
            session,
            capabilities: Arc::new(CapabilityCache::default()),
        }
    }

    /// Read cached observations without collecting or exposing remote output.
    pub async fn capabilities(&self) -> MonitorCapabilities {
        self.capabilities.report().await
    }

    /// Read observations with immutable proof of this connection's cache state.
    pub async fn capability_report(&self) -> CapabilityReport {
        self.capabilities.authenticated_report().await
    }

    /// Check report authority using only connection identity and atomic counters.
    ///
    /// A cache write, reset or invalidation revokes older reports. This check
    /// performs no I/O and does not refresh any observation time.
    pub fn accepts_capability_report(&self, report: &CapabilityReport) -> bool {
        report.is_current_for(&self.capabilities)
    }

    /// Forget observations before an explicit, read-only capability recheck.
    ///
    /// The cache belongs only to this monitor's authenticated SSH session.
    /// Pending read-only collectors and probe admission waits are cancelled.
    /// Their channel owners perform bounded cleanup; this does not establish
    /// remote process termination. Failed channel cleanup can abort the shared
    /// transport under the existing SSH ownership contract.
    pub async fn reset_capabilities(&self) {
        self.capabilities.reset().await;
    }

    /// Revoke publication by already running checks without blocking the caller.
    /// Used when a panel pauses or retires its authenticated session authority.
    ///
    /// Pending read-only collectors and probe admission waits are woken without
    /// waiting for the remote response. Completed observations remain cached.
    /// Cancellation closes the owned channel but is no proof that the remote
    /// process stopped; uncertain writes and reviewed mutations are unaffected.
    pub fn invalidate_pending_capabilities(&self) {
        self.capabilities.invalidate_pending();
    }

    /// Execute the platform's fixed read-only collector and optional capacity API.
    ///
    /// A failed capacity command is skipped until explicit redetection. Its
    /// failure does not discard CPU, memory, network or block-device counters.
    pub async fn snapshot(&self) -> MonitorResult<Snapshot> {
        let (revision, resources) = self.capabilities.begin(Capability::Resources).await;
        if let Some(reason) = cached_reason(resources) {
            // Windows errors come from typed API frames and remain explicit
            // recheck decisions. POSIX resource errors retain their existing
            // transient retry behavior, except missing/unsupported collectors.
            let windows = matches!(
                self.capabilities.route().await,
                Some(platform::Route::WindowsPowerShell(_))
            );
            self.ensure_current(revision)?;
            if windows
                || matches!(
                    reason,
                    CapabilityReason::UnsupportedPlatform | CapabilityReason::CommandMissing
                )
            {
                return Err(MonitorError::CapabilityUnavailable {
                    capability: Capability::Resources,
                    reason,
                });
            }
        }
        let (_, capacity) = self.capabilities.begin(Capability::Filesystems).await;
        let skip_capacity = cached_reason(capacity).is_some();
        let linux_script = if skip_capacity {
            SNAPSHOT_SCRIPT.replace(DF_COLLECTION, "printf '\\n@@KS:df-status@@\\ncached\\n'")
        } else {
            SNAPSHOT_SCRIPT.to_owned()
        };
        let script = format!(
            "{PLATFORM_SELECTOR}case \"$(\"$ks_uname\" -s)\" in\nDarwin)\n{}\n;;\nLinux)\n{linux_script}\n;;\n*) exit 64 ;;\nesac\n",
            darwin::script(skip_capacity)
        );
        let result = async {
            let route = self.route(revision).await?;
            self.ensure_current(revision)?;
            if matches!(route, platform::Route::WindowsPowerShell(_)) {
                let command = windows::command(windows::Operation::Resources { skip_capacity })?;
                self.ensure_current(revision)?;
                let output = self.exec_current(revision, &command, MAX_OUTPUT).await?;
                self.ensure_current(revision)?;
                return windows::parse_resources(output);
            }
            let platform::Route::Posix(expected) = route else {
                return Err(MonitorError::Unsupported(
                    "no resource collector for the confirmed target platform",
                ));
            };
            let output = self.exec_current(revision, &script, MAX_OUTPUT).await?;
            self.ensure_current(revision)?;
            let ExecOutput {
                stdout,
                stderr,
                exit_status,
            } = output;
            let text = command_text(
                ExecOutput {
                    stdout,
                    stderr: stderr.clone(),
                    exit_status,
                },
                Capability::Resources,
            )?;
            let (snapshot, capacity) = Snapshot::parse_with_capacity(&text, &stderr)?;
            if snapshot.platform != expected {
                return Err(MonitorError::InvalidData(
                    "resource platform differs from the validated identity",
                ));
            }
            Ok((snapshot, capacity))
        }
        .await;
        let state = result
            .as_ref()
            .map_or_else(error_state, |_| CapabilityState::Available);
        self.capabilities
            .record(revision, Capability::Resources, state)
            .await;
        match result {
            Ok((snapshot, capacity)) => {
                if !skip_capacity {
                    self.capabilities
                        .record(revision, Capability::Filesystems, capacity)
                        .await;
                }
                Ok(snapshot)
            }
            Err(error) => Err(error),
        }
    }

    /// Read fixed ps columns or Windows process names and working-set bytes.
    /// Unix rows sort by reported CPU usage; Windows rows sort by numeric PID.
    pub async fn processes(&self) -> MonitorResult<Vec<ProcessInfo>> {
        let (revision, state) = self.capabilities.begin(Capability::Processes).await;
        if let Some(reason) = cached_reason(state) {
            return Err(MonitorError::CapabilityUnavailable {
                capability: Capability::Processes,
                reason,
            });
        }
        let result = async {
            let route = self.route(revision).await?;
            self.ensure_current(revision)?;
            if matches!(route, platform::Route::WindowsPowerShell(_)) {
                let command = windows::command(windows::Operation::Processes)?;
                self.ensure_current(revision)?;
                let output = self.exec_current(revision, &command, MAX_OUTPUT).await?;
                self.ensure_current(revision)?;
                return windows::parse_processes(output);
            }
            if matches!(route, platform::Route::UnsupportedPosix) {
                return Err(MonitorError::Unsupported(
                    "no process collector for the confirmed target platform",
                ));
            }
            let output = self
                .exec_current(revision, PROCESSES_SCRIPT, MAX_OUTPUT)
                .await?;
            self.ensure_current(revision)?;
            parse_processes(&command_text(output, Capability::Processes)?)
        }
        .await;
        let state = result
            .as_ref()
            .map_or_else(error_state, |_| CapabilityState::Available);
        self.capabilities
            .record(revision, Capability::Processes, state)
            .await;
        result
    }

    /// Collect sockets visible to the authenticated remote account.
    ///
    /// Linux uses fixed `ss` TCP/UDP output. Darwin uses the absolute system
    /// `lsof` path for TCP LISTEN and UDP sockets with an assigned local port.
    /// Darwin UDP rows retain a specified peer when present; unbound sockets
    /// without a local port are omitted. Permission diagnostics, incomplete
    /// frames and nonempty failed collections are rejected.
    /// Results are diagnostic data, not executable commands, whole-host
    /// visibility, or proof that a service accepted a connection.
    pub async fn listening_sockets(&self) -> MonitorResult<Vec<SocketInfo>> {
        let (revision, state) = self.capabilities.begin(Capability::Sockets).await;
        if let Some(reason) = cached_reason(state) {
            return Err(MonitorError::CapabilityUnavailable {
                capability: Capability::Sockets,
                reason,
            });
        }
        let result = async {
            let route = self.route(revision).await?;
            self.ensure_current(revision)?;
            if matches!(route, platform::Route::WindowsPowerShell(_)) {
                let command = windows::command(windows::Operation::Sockets)?;
                self.ensure_current(revision)?;
                let output = self.exec_current(revision, &command, MAX_OUTPUT).await?;
                self.ensure_current(revision)?;
                return windows::parse_sockets(output);
            }
            let platform::Route::Posix(expected) = route else {
                return Err(MonitorError::Unsupported(
                    "no socket collector for the confirmed target platform",
                ));
            };
            let output = self
                .exec_current(revision, SOCKETS_SCRIPT, MAX_OUTPUT)
                .await?;
            self.ensure_current(revision)?;
            parse_socket_collection(output, expected)
        }
        .await;
        let state = result
            .as_ref()
            .map_or_else(error_state, |_| CapabilityState::Available);
        self.capabilities
            .record(revision, Capability::Sockets, state)
            .await;
        result
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
        let (generation, _) = self.capabilities.begin(Capability::Sockets).await;
        self.require_linux(generation).await?;
        let current = self.listening_sockets().await?;
        self.ensure_current(generation)?;
        if !current.iter().any(|candidate| candidate == socket) {
            return Err(MonitorError::SocketChanged);
        }
        let (host, port) = probe_endpoint(&socket.local)?;
        let command = format!(
            "LC_ALL=C; export LC_ALL\nif [ \"$(uname -s)\" != Linux ]; then exit 64; fi\ncommand -v nc >/dev/null 2>&1 || exit 66\nstatus=0\nnc -z -w 2 {host} {port} >/dev/null 2>&1 || status=$?\nprintf '@@KS:probe@@\\n%s\\n' \"$status\"\n",
            host = shell_literal(&host)
        );
        self.ensure_current(generation)?;
        let output = self.exec_current(generation, &command, 4 * 1024).await?;
        self.ensure_current(generation)?;
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
        let (generation, _) = self.capabilities.begin(Capability::Processes).await;
        self.require_linux(generation).await?;
        let pid = selected.pid;
        let command = format!(
            "LC_ALL=C; export LC_ALL\nif [ \"$(uname -s)\" != Linux ]; then exit 64; fi\ncat /proc/{pid}/stat || exit 65\nprintf '\\n@@KS:process@@\\n'\nps -ww -p {pid} -o pid=,user=,pcpu=,pmem=,args= || exit 65\n"
        );
        self.ensure_current(generation)?;
        let output = self.exec_current(generation, &command, 64 * 1024).await?;
        self.ensure_current(generation)?;
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
        let (generation, _) = self.capabilities.begin(Capability::Processes).await;
        self.require_linux(generation).await?;
        let command = format!(
            "pid={}; expected={}\nvalue=$(cat \"/proc/$pid/stat\") || exit 65\nvalue=${{value##*) }}\nset -- $value\n[ \"$#\" -ge 20 ] || exit 65\nshift 19\n[ \"$1\" = \"$expected\" ] || exit 67\nkill -TERM \"$pid\"\n",
            identity.process.pid, identity.start_ticks
        );
        // Admission can wait for a channel reply. Check again before sending,
        // but retain the real receipt once SIGTERM has actually been sent.
        let output = self
            .session
            .exec_limited_guarded(&command, 64 * 1024, || {
                self.capabilities.is_current(generation)
                    && identity.inspected.elapsed() <= Duration::from_secs(60)
            })
            .await?;
        let Some(output) = output else {
            self.ensure_current(generation)?;
            return Err(MonitorError::ReviewExpired);
        };
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
            user: Some(fields[1].to_owned()),
            cpu_percent: Some(finite_positive(fields[2], "process CPU")?),
            memory_percent: Some(finite_positive(fields[3], "process memory")?),
            working_set_bytes: None,
            command: command.to_owned(),
        });
    }
    processes.sort_by(|a, b| {
        b.cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.pid.cmp(&b.pid))
    });
    Ok(processes)
}

fn parse_socket_collection(
    output: ExecOutput,
    expected: MonitorPlatform,
) -> MonitorResult<Vec<SocketInfo>> {
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_OUTPUT {
        return Err(MonitorError::InvalidData("socket collection exceeds 2 MiB"));
    }
    if output.exit_status != Some(0) {
        command_text(output, Capability::Sockets)?;
        return Err(MonitorError::InvalidData(
            "socket collector did not complete",
        ));
    }
    if output.stdout.starts_with(darwin_sockets::START.as_bytes()) {
        if expected != MonitorPlatform::Darwin {
            return Err(MonitorError::InvalidData(
                "socket platform differs from the validated identity",
            ));
        }
        return darwin_sockets::parse(output);
    }
    // An absent Darwin frame must never masquerade as ss's valid empty output.
    // Both platforms therefore prove their origin and completion explicitly.
    let text = String::from_utf8(output.stdout)
        .map_err(|_| MonitorError::InvalidData("non-UTF-8 socket collection"))?;
    let text = text
        .strip_prefix("@@KS:linux-sockets:1@@\n")
        .ok_or(MonitorError::InvalidData("missing socket collector frame"))?;
    if expected != MonitorPlatform::Linux {
        return Err(MonitorError::InvalidData(
            "socket platform differs from the validated identity",
        ));
    }
    let (rows, status) = text
        .split_once("@@KS:ss-status@@\n")
        .ok_or(MonitorError::InvalidData("missing Linux socket status"))?;
    let status = status
        .strip_suffix("\n@@KS:end@@\n")
        .filter(|status| {
            !status.is_empty()
                && status.len() <= 10
                && status.bytes().all(|byte| byte.is_ascii_digit())
        })
        .and_then(|status| status.parse::<u32>().ok())
        .ok_or(MonitorError::InvalidData(
            "invalid Linux socket status or completion",
        ))?;
    if status != 0 || !output.stderr.is_empty() {
        return Err(MonitorError::CapabilityUnavailable {
            capability: Capability::Sockets,
            reason: failure_reason(status, &output.stderr),
        });
    }
    parse_sockets(rows)
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
            "this monitoring facility is unsupported on the target",
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
                "df-status",
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
        free_bytes: None,
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

fn parse_capacity(
    sections: &BTreeMap<String, String>,
    stderr: &[u8],
) -> (Vec<FilesystemSnapshot>, CapabilityState) {
    if let Some(status) = sections.get("df-status") {
        match status.trim().parse::<u32>() {
            Ok(0) => {}
            Ok(status @ 1..=255) => {
                return (
                    Vec::new(),
                    CapabilityState::Unavailable(failure_reason(status, stderr)),
                );
            }
            _ => {
                return (
                    Vec::new(),
                    CapabilityState::Unavailable(CapabilityReason::InvalidOutput),
                );
            }
        }
    }
    match sections
        .get("df")
        .and_then(|text| parse_filesystems(text).ok())
    {
        Some(filesystems) => (filesystems, CapabilityState::Available),
        None => (
            Vec::new(),
            CapabilityState::Unavailable(CapabilityReason::InvalidOutput),
        ),
    }
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

    pub(super) const SAMPLE: &str = "@@KS:platform@@\nLinux\n@@KS:stat@@\ncpu 100 20 30 400 50 0 0 0 80 10\ncpu0 1 1 1 1\nbtime 1700000000\n@@KS:meminfo@@\nMemTotal: 1024 kB\nMemAvailable: 256 kB\nSwapTotal: 512 kB\nSwapFree: 128 kB\n@@KS:loadavg@@\n1.25 0.50 0.25 1/100 45\n@@KS:uptime@@\n100.00 75.00\n@@KS:net@@\nInter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 100 1 0 0 0 0 0 0 200 1 0 0 0 0 0 0\neth0: 1000 10 0 0 0 0 0 0 2000 10 0 0 0 0 0 0\n@@KS:df@@\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 600 350 64% /\ntmpfs 200 1 199 1% /with spaces\n@@KS:end@@\n";
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
        assert_eq!(sample.load_average, Some([1.25, 0.5, 0.25]));
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
        let CpuSample::Counters(cpu) = &mut after.cpu else {
            panic!("Linux counters")
        };
        cpu.ticks[0] = 0;
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
    fn windows_cpu_is_a_point_observation_without_invented_counter_or_network_rates()
    -> MonitorResult<()> {
        let before = Snapshot::parse(SAMPLE)?;
        let mut windows = before.clone();
        windows.platform = MonitorPlatform::Windows;
        windows.cpu = CpuSample::ReportedPercent(12.5);
        windows.load_average = None;
        windows.memory.available_bytes = None;
        windows.filesystems.clear();
        windows.disk_io = Err(DiskIoError::MissingData);
        windows.uptime_seconds += 5.;
        assert_eq!(windows.cpu.cumulative(), None);
        assert_eq!(windows.cpu.reported_percent(), Some(12.5));
        assert_eq!(windows.memory.used_bytes(), None);
        let rates = windows.rates_since(&before);
        assert_eq!(rates.cpu_busy_percent, Some(12.5));
        assert_eq!(rates.cpu_iowait_percent, None);
        assert!(rates.networks.is_empty() && rates.disks.is_empty());
        let previous = windows.clone();
        windows.cpu = CpuSample::ReportedPercent(0.);
        windows.boot_time += 1;
        assert_eq!(windows.rates_since(&previous).cpu_busy_percent, Some(0.));
        windows.cpu = CpuSample::Unavailable;
        assert_eq!(windows.rates_since(&previous).cpu_busy_percent, None);
        for value in [-1., 100.1, f64::NAN, f64::INFINITY] {
            assert_eq!(CpuSample::ReportedPercent(value).reported_percent(), None);
        }
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn platform_selector_supports_bin_only_uname_and_reports_both_paths_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let private = tempfile::tempdir()?;
        let binary = private.path().join("bin-uname");
        std::fs::write(&binary, "#!/bin/sh\nprintf 'Linux\\n'\n")?;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))?;
        let missing = private.path().join("missing");
        assert!(PROCESSES_SCRIPT.contains(PLATFORM_SELECTOR));
        for (second, code, output) in [
            (&binary, 0, b"Linux\n".as_slice()),
            (&missing, 66, b"".as_slice()),
        ] {
            let selector = PLATFORM_SELECTOR
                .replace("/usr/bin/uname", &shell_literal(&missing.to_string_lossy()))
                .replace("/bin/uname", &shell_literal(&second.to_string_lossy()));
            let mut child = std::process::Command::new("/bin/sh")
                .args(["-c", &format!("{selector}\"$ks_uname\" -s")])
                .env_clear()
                .env("PATH", private.path())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while child.try_wait()?.is_none() {
                if std::time::Instant::now() >= deadline {
                    child.kill()?;
                    child.wait()?;
                    return Err("platform selection exceeded deadline".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let result = child.wait_with_output()?;
            assert_eq!(result.status.code(), Some(code));
            assert_eq!(result.stdout, output);
        }
        Ok(())
    }

    #[test]
    fn platform_change_invalidates_rates_and_darwin_has_no_iowait() -> MonitorResult<()> {
        let linux = Snapshot::parse(SAMPLE)?;
        let mut darwin = linux.clone();
        darwin.platform = MonitorPlatform::Darwin;
        darwin.uptime_seconds += 5.;
        darwin.cpu = CpuSample::Counters(CpuCounters {
            ticks: [200, 20, 60, 600, 0, 0, 0, 0],
        });
        assert_eq!(darwin.rates_since(&linux), SampleRates::default());
        let mut next = darwin.clone();
        next.uptime_seconds += 5.;
        let CpuSample::Counters(cpu) = &mut next.cpu else {
            panic!("Darwin counters")
        };
        cpu.ticks[0] += 20;
        cpu.ticks[3] += 20;
        let rates = next.rates_since(&darwin);
        assert_eq!(rates.cpu_busy_percent, Some(50.));
        assert_eq!(rates.cpu_iowait_percent, None);
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
        assert_eq!(rows[0].cpu_percent, Some(125.0));
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
            let (text, status) = if command == platform::POSIX_PROBE {
                let platform = if self.unsupported {
                    "Unsupported"
                } else {
                    "Linux"
                };
                (
                    format!(
                        "@@KS:monitor-platform:1@@\nplatform\t{platform}\nruntime\tposix\n@@KS:end@@\n"
                    ),
                    0,
                )
            } else if self.unsupported {
                (String::new(), 64)
            } else if command.starts_with(PLATFORM_SELECTOR) && command.contains(SNAPSHOT_SCRIPT) {
                (SAMPLE.to_owned(), 0)
            } else if command == PROCESSES_SCRIPT {
                (PROCESS_ROWS.to_owned(), 0)
            } else if command == SOCKETS_SCRIPT {
                (
                    "@@KS:linux-sockets:1@@\ntcp LISTEN 0 128 127.0.0.1:2222 0.0.0.0:* users:((\"fixture\",pid=42,fd=3))\n@@KS:ss-status@@\n0\n@@KS:end@@\n"
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
            assert_eq!(commands.len(), 4);
            assert_eq!(commands[0], platform::POSIX_PROBE);
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
        assert_eq!(commands.len(), 5);
        assert!(commands[4].starts_with("pid=42; expected=12345\n"));
        assert!(commands[4].contains("kill -TERM \"$pid\""));
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
            assert_eq!(commands.len(), 3);
            assert_eq!(commands[0], platform::POSIX_PROBE);
            assert_eq!(commands[1], SOCKETS_SCRIPT);
            assert!(commands[2].contains("nc -z -w 2 '127.0.0.1' 2222"));
            assert!(!commands[2].contains("echo "));
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
        assert_eq!(commands.len(), 4);
        Ok(())
    }

    #[tokio::test]
    async fn loopback_ssh_reports_unsupported_platform() -> Result<(), Box<dyn std::error::Error>> {
        let (server, monitor) = fixture(true).await?;
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::Unsupported(_))
        ));
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::Unsupported(_))
        ));
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::CapabilityUnavailable {
                capability: Capability::Resources,
                reason: CapabilityReason::UnsupportedPlatform,
            })
        ));
        assert_eq!(
            server
                .commands
                .lock()
                .map_err(|_| "fixture lock poisoned")?
                .len(),
            1,
            "an observed unsupported platform is not repeatedly collected"
        );
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
        assert_eq!(commands.len(), 4);
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
