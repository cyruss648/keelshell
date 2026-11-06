//! Explicit network diagnostics executed only through authenticated SSH.
//! The fixed Python 3 stdlib probe requires POSIX interval timers. Nothing is
//! installed; DNS uses the target's OS resolver, TLS uses its trust store, and
//! HTTP sends one HEAD without credentials, redirects, proxies or a body.

use crate::{SessionError, SshSession};
use base64::{Engine, engine::general_purpose::STANDARD};
use keelshell_core::{
    MAX_DIAGNOSTIC_OUTPUT_BYTES, NetworkDiagnosticKind, NetworkDiagnosticRequest,
};
use serde::Deserialize;
use std::{
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

mod traceroute;
pub use traceroute::{
    NetworkDiagnosticTraceroute, NetworkDiagnosticTracerouteHop,
    NetworkDiagnosticTracerouteImplementation, NetworkDiagnosticTracerouteProbe,
    NetworkDiagnosticTracerouteReason,
};

const SCRIPT: &str = concat!(
    include_str!("network_diagnostic/icmp.py"),
    "\n",
    include_str!("network_diagnostic/traceroute.py"),
    "\n",
    include_str!("network_diagnostic/probe.py")
);
const WORKER: &str = include_str!("network_diagnostic/worker.py");
const FRAME: &str = "KEELSHELL_DIAGNOSTIC_V1\n";
const ENVELOPE: Duration = Duration::from_secs(10);

/// Fixed result categories, without reflecting remote exception or header text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticStatus {
    /// The selected protocol completed; HTTP status is reported separately.
    Success,
    /// Remote resolver failed, without inventing a local fallback result.
    DnsFailure,
    /// Resolved remote TCP endpoints could not be connected.
    ConnectionFailure,
    /// Remote trust store, hostname or certificate validity rejected TLS.
    CertificateRejected,
    /// TLS negotiation failed before a verified certificate was available.
    TlsFailure,
    /// No valid bounded HTTP response headers were received.
    HttpFailure,
    /// The remote eight-second wall-clock deadline expired.
    Timeout,
    /// Python version/modules or POSIX interval timers are unavailable.
    UnsupportedEnvironment,
    /// Supervisor termination could not confirm remote worker exit.
    CleanupUnknown,
    /// Remote supervisor received shutdown and confirmed its worker exited.
    Interrupted,
    /// The tool counted four send attempts and one to three unique echo replies.
    /// Unanswered attempts do not establish network packet loss: quiet mode can
    /// hide local send failures.
    IcmpPartial,
    /// The tool counted four send attempts without an echo reply.
    /// This is not a supervisor timeout or proof of successful transmission.
    IcmpNoReply,
    /// The admitted remote utility or its executable access was unavailable.
    UtilityUnavailable,
    /// Executable access or an exact admitted utility errno reported denied permission.
    PermissionDenied,
    /// ICMP output, exit status, output budget or reported ICMP errors prevented success.
    IcmpFailure,
    /// The reviewed hop limit was exhausted without an observed destination reply.
    TracerouteHopLimit,
    /// The utility reported an ICMP unreachable/error marker; no healthy route is claimed.
    TracerouteUnreachable,
    /// Traceroute start, output, exit status or capture budget failed validation.
    TracerouteFailure,
}

/// The single upstream utility output/argument contract currently admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticIcmpImplementation {
    /// Linux iputils release 20240117; other reported versions fail closed.
    #[serde(rename = "iputils_20240117")]
    Iputils20240117,
}

/// A fixed ICMP reason, never remote stderr or arbitrary error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticIcmpReason {
    /// The tool counted four send attempts and reported four unique echo replies.
    Completed,
    /// The tool reported fewer unique echo replies than counted send attempts.
    /// The wire name is retained; it does not establish network packet loss,
    /// because quiet mode can hide local send failures.
    PacketLoss,
    /// The tool counted four send attempts and reported no echo reply.
    /// Neither successful transmission nor the cause of missing replies is known.
    NoEchoReply,
    /// A supervised stage exhausted its remaining wall-clock budget.
    Deadline,
    /// Supervisor shutdown interrupted an owned stage.
    Interrupted,
    /// An actual bounded child wait could not confirm exit.
    CleanupUnknown,
    /// Neither fixed remote executable location contained a ping utility.
    UtilityMissing,
    /// Executable admission or a recognized C-locale errno denied permission.
    PermissionDenied,
    /// This adapter supports Linux with the existing POSIX/Python prerequisites.
    UnsupportedPlatform,
    /// The version/configuration did not match the admitted upstream release.
    UnsupportedImplementation,
    /// The remote system resolver could not resolve the reviewed host.
    ResolutionFailed,
    /// A subprocess failed to start or returned an unrecognized failure status.
    ProcessFailed,
    /// Counts, framing, RTT or exit status violated the admitted output contract.
    InvalidOutput,
    /// Combined captured output across all stages exceeded 32 KiB.
    OutputLimit,
    /// The admitted utility reported ICMP errors or corrupted packets.
    ReportedErrors,
}

/// Utility-reported ICMP summary counters, absent without a valid summary.
///
/// Quiet iputils counts some hard local send failures as transmitted attempts
/// and can omit their error diagnostics. The difference between `transmitted`
/// and `received` means unanswered attempts, not measured network packet loss.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticIcmpCounts {
    /// Send attempts counted by the utility, never fabricated from the request.
    ///
    /// This counter includes iputils `hard_local_error` increments. It does not
    /// prove successful sends or packets on the wire. Quiet summary output
    /// cannot distinguish those local failures from transmitted but unanswered
    /// echo requests.
    pub transmitted: u8,
    /// Unique echo replies reported by the utility, excluding duplicates.
    pub received: u8,
    /// Additional replies reported by the utility.
    pub duplicates: u16,
    /// Corrupted responses reported by the utility.
    pub corrupted: u16,
    /// ICMP errors reported by the utility.
    /// Zero does not exclude local send errors hidden by quiet mode.
    pub errors: u16,
}

/// Utility-measured round-trip statistics in integer microseconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticIcmpRtt {
    /// Minimum observed echo round trip.
    pub min_us: u64,
    /// Mean observed echo round trip, including utility-reported duplicates.
    pub avg_us: u64,
    /// Maximum observed echo round trip.
    pub max_us: u64,
    /// Population standard deviation as reported by the utility.
    pub mdev_us: u64,
}

/// Structured ICMP outcome with no executable output or user-controlled reason.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticIcmp {
    /// Fixed reason matching the outer result status.
    pub reason: NetworkDiagnosticIcmpReason,
    /// Admitted version, absent before version admission succeeds.
    pub implementation: Option<NetworkDiagnosticIcmpImplementation>,
    /// The one selected numeric address; there is no multi-address fallback.
    pub address: Option<String>,
    /// Utility summary counters, absent for missing, killed or invalid summaries.
    pub counts: Option<NetworkDiagnosticIcmpCounts>,
    /// RTT summary, present exactly when a valid summary includes echo replies.
    pub rtt: Option<NetworkDiagnosticIcmpRtt>,
}

/// A bounded address from the remote OS resolver, potentially including hosts/NSS.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticAddress {
    /// `IPv4` or `IPv6`, checked against the literal address.
    pub family: String,
    /// A numeric literal, never a local DNS lookup.
    pub address: String,
}

/// Strictly verified remote peer certificate and negotiated TLS information.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticTls {
    /// Negotiated TLS version.
    pub protocol: String,
    /// Negotiated cipher name.
    pub cipher: String,
    /// Escaped, bounded leaf subject, with truncation exposed by `limited`.
    pub subject: String,
    /// Escaped, bounded leaf issuer.
    pub issuer: String,
    /// Certificate's notBefore string from Python's verified certificate.
    pub not_before: String,
    /// Certificate's notAfter string.
    pub not_after: String,
    /// Up to sixteen escaped DNS/IP subject alternative names.
    pub names: Vec<String>,
    /// SHA-256 fingerprint of the leaf certificate's DER bytes.
    pub sha256: String,
}

/// HTTP response metadata only; no headers, body, cookies or redirect targets.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticHttp {
    /// Actual HTTP status, including non-success and redirect responses.
    pub status: u16,
    /// The remote probe currently speaks HTTP/1.0 or HTTP/1.1.
    pub version: String,
}

/// Remote monotonic timings in milliseconds, not local SSH round-trip estimates.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticTiming {
    /// Completed OS resolution, absent if resolution failed.
    pub resolve_ms: Option<u64>,
    /// Completed TCP connect attempts, absent if no connection completed.
    pub connect_ms: Option<u64>,
    /// Completed verified TLS handshake, absent on rejection/failure.
    pub tls_ms: Option<u64>,
    /// HEAD send through receipt of HTTP headers, absent on failure.
    pub headers_ms: Option<u64>,
    /// Entire remote script, bounded to the wall-clock budget.
    pub total_ms: u64,
}

/// A validated snapshot bound to the exact request supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDiagnosticReport {
    /// Exact credential-free request whose remote result was checked.
    pub request: NetworkDiagnosticRequest,
    /// Completed protocol or fixed remote failure category.
    pub status: NetworkDiagnosticStatus,
    /// Up to sixteen unique system-resolver addresses.
    pub addresses: Vec<NetworkDiagnosticAddress>,
    /// Actual selected numeric remote peer, when a connection completed.
    pub peer: Option<String>,
    /// Verified certificate only; invalid certificates never use an insecure retry.
    pub tls: Option<NetworkDiagnosticTls>,
    /// HEAD status and version, if bounded headers were received.
    pub http: Option<NetworkDiagnosticHttp>,
    /// ICMP counters/RTT or a fixed capability/failure reason; absent for other modes.
    pub icmp: Option<NetworkDiagnosticIcmp>,
    /// Structured Linux traceroute rows/reason, absent for other operations.
    pub traceroute: Option<NetworkDiagnosticTraceroute>,
    /// Remote elapsed stages.
    pub timing: NetworkDiagnosticTiming,
    /// Address or certificate display fields reached a stated cap.
    pub limited: bool,
}

/// Fixed channel/admission failures. No remote output or supplied input is exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NetworkDiagnosticError {
    /// Cancellation revokes future dispatch and discards in-flight result adoption.
    #[error("remote diagnostic cancelled")]
    Cancelled,
    /// The ten-second SSH envelope expired, including opening the channel.
    #[error("remote diagnostic channel deadline expired")]
    Timeout,
    /// The POSIX exec environment has no discoverable Python 3 executable.
    #[error("remote Python 3 is required; install/configure it separately if desired")]
    PythonUnavailable,
    /// The server cannot run the fixed POSIX/Python environment.
    #[error("remote diagnostic environment is unsupported")]
    UnsupportedEnvironment,
    /// SSH failed; raw peer diagnostics are excluded.
    #[error("remote diagnostic SSH channel failed")]
    Transport,
    /// The output budget was exceeded.
    #[error("remote diagnostic output limit exceeded")]
    OutputLimit,
    /// Framing, exit status, types or protocol invariants were invalid.
    #[error("remote diagnostic response is invalid or incomplete")]
    InvalidResponse,
}

impl SshSession {
    /// Execute one explicitly approved diagnostic through this authenticated
    /// connection. No local network result is used. The fixed remote script is
    /// bounded to eight seconds; the channel envelope is ten seconds and uses
    /// existing bounded channel cleanup. Cancellation is checked after channel
    /// open and before exec, and in flight. Closing the owned channel does not
    /// prove immediate peer-process termination; the remote timer still bounds it.
    pub async fn diagnose_remote(
        &self,
        request: NetworkDiagnosticRequest,
        cancel: &AtomicBool,
    ) -> Result<NetworkDiagnosticReport, NetworkDiagnosticError> {
        if cancel.load(Ordering::Acquire) {
            return Err(NetworkDiagnosticError::Cancelled);
        }
        let encoded = STANDARD.encode(
            serde_json::to_vec(&request).map_err(|_| NetworkDiagnosticError::InvalidResponse)?,
        );
        let worker = STANDARD.encode(WORKER.as_bytes());
        let command = format!(
            "command -v python3 >/dev/null 2>&1 || exit 66\npython3 -I -S -B - '{encoded}' '{worker}' <<'KEELSHELL_PROTOCOL_PROBE'\n{SCRIPT}\nKEELSHELL_PROTOCOL_PROBE\n"
        );
        let until = Instant::now() + ENVELOPE;
        let operation = async {
            let output = self
                .diagnostic_exec_until(&command, until, cancel)
                .await
                .map_err(|error| match error {
                    SessionError::OutputLimit(_) => NetworkDiagnosticError::OutputLimit,
                    SessionError::Timeout(_) => NetworkDiagnosticError::Timeout,
                    _ if cancel.load(Ordering::Acquire) => NetworkDiagnosticError::Cancelled,
                    _ => NetworkDiagnosticError::Transport,
                })?;
            if cancel.load(Ordering::Acquire) {
                return Err(NetworkDiagnosticError::Cancelled);
            }
            if output.exit_status == Some(66) {
                return Err(NetworkDiagnosticError::PythonUnavailable);
            }
            if output.exit_status != Some(0) {
                return Err(NetworkDiagnosticError::UnsupportedEnvironment);
            }
            if !output.stderr.is_empty() {
                return Err(NetworkDiagnosticError::InvalidResponse);
            }
            parse_report(request, &output.stdout)
        };
        tokio::select! {
            biased;
            _ = cancelled(cancel) => Err(NetworkDiagnosticError::Cancelled),
            result = timeout_at(until, operation) => result.map_err(|_| NetworkDiagnosticError::Timeout)?,
        }
    }
}

async fn cancelled(cancel: &AtomicBool) {
    loop {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u8,
    status: NetworkDiagnosticStatus,
    addresses: Vec<NetworkDiagnosticAddress>,
    peer: Option<String>,
    tls: Option<NetworkDiagnosticTls>,
    http: Option<NetworkDiagnosticHttp>,
    icmp: Option<NetworkDiagnosticIcmp>,
    traceroute: Option<NetworkDiagnosticTraceroute>,
    timing: NetworkDiagnosticTiming,
    limited: bool,
}

fn parse_report(
    request: NetworkDiagnosticRequest,
    bytes: &[u8],
) -> Result<NetworkDiagnosticReport, NetworkDiagnosticError> {
    use NetworkDiagnosticError::InvalidResponse;
    if bytes.len() > MAX_DIAGNOSTIC_OUTPUT_BYTES {
        return Err(NetworkDiagnosticError::OutputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| InvalidResponse)?;
    let wire: Wire = serde_json::from_str(text.strip_prefix(FRAME).ok_or(InvalidResponse)?)
        .map_err(|_| InvalidResponse)?;
    if wire.version != 1 || wire.addresses.len() > 16 || wire.timing.total_ms > 10_000 {
        return Err(InvalidResponse);
    }
    let mut addresses = std::collections::BTreeSet::new();
    for address in &wire.addresses {
        let ip = address
            .address
            .parse::<IpAddr>()
            .map_err(|_| InvalidResponse)?;
        if address.family != if ip.is_ipv4() { "IPv4" } else { "IPv6" }
            || !addresses.insert(address.address.clone())
        {
            return Err(InvalidResponse);
        }
    }
    if wire
        .peer
        .as_ref()
        .is_some_and(|peer| !addresses.contains(peer))
    {
        return Err(InvalidResponse);
    }
    for value in [
        wire.timing.resolve_ms,
        wire.timing.connect_ms,
        wire.timing.tls_ms,
        wire.timing.headers_ms,
    ]
    .into_iter()
    .flatten()
    {
        if value > wire.timing.total_ms {
            return Err(InvalidResponse);
        }
    }
    if let Some(tls) = &wire.tls {
        for (value, max) in [
            (&tls.protocol, 64),
            (&tls.cipher, 128),
            (&tls.subject, 1024),
            (&tls.issuer, 1024),
            (&tls.not_before, 64),
            (&tls.not_after, 64),
        ] {
            if !safe_text(value, max) {
                return Err(InvalidResponse);
            }
        }
        if tls.names.len() > 16
            || tls.names.iter().any(|name| !safe_text(name, 256))
            || tls.sha256.len() != 64
            || !tls.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || wire.peer.is_none()
            || wire.timing.tls_ms.is_none()
        {
            return Err(InvalidResponse);
        }
    }
    if let Some(http) = &wire.http
        && (!(100..=599).contains(&http.status)
            || !matches!(http.version.as_str(), "HTTP/1.0" | "HTTP/1.1")
            || wire.peer.is_none()
            || wire.timing.headers_ms.is_none())
    {
        return Err(InvalidResponse);
    }
    let kind = request.kind();
    if kind == NetworkDiagnosticKind::Traceroute {
        traceroute::validate(&request, &wire)?;
    } else if wire.traceroute.is_some()
        || matches!(
            wire.status,
            NetworkDiagnosticStatus::TracerouteHopLimit
                | NetworkDiagnosticStatus::TracerouteUnreachable
                | NetworkDiagnosticStatus::TracerouteFailure
        )
    {
        return Err(InvalidResponse);
    }
    if kind == NetworkDiagnosticKind::Icmp {
        validate_icmp(&wire)?;
    } else if kind != NetworkDiagnosticKind::Traceroute
        && (wire.icmp.is_some()
            || matches!(
                wire.status,
                NetworkDiagnosticStatus::IcmpPartial
                    | NetworkDiagnosticStatus::IcmpNoReply
                    | NetworkDiagnosticStatus::UtilityUnavailable
                    | NetworkDiagnosticStatus::PermissionDenied
                    | NetworkDiagnosticStatus::IcmpFailure
            ))
    {
        return Err(InvalidResponse);
    }
    if (kind == NetworkDiagnosticKind::Dns
        && (wire.peer.is_some() || wire.tls.is_some() || wire.http.is_some()))
        || (kind == NetworkDiagnosticKind::Tls && wire.http.is_some())
        || (kind == NetworkDiagnosticKind::Http && !request.https() && wire.tls.is_some())
    {
        return Err(InvalidResponse);
    }
    if !matches!(
        kind,
        NetworkDiagnosticKind::Icmp | NetworkDiagnosticKind::Traceroute
    ) && wire.status == NetworkDiagnosticStatus::Success
        && (wire.addresses.is_empty()
            || wire.timing.resolve_ms.is_none()
            || (kind != NetworkDiagnosticKind::Dns
                && (wire.peer.is_none() || wire.timing.connect_ms.is_none()))
            || (kind == NetworkDiagnosticKind::Tls
                || (kind == NetworkDiagnosticKind::Http && request.https()))
                && wire.tls.is_none()
            || kind == NetworkDiagnosticKind::Http && wire.http.is_none())
    {
        return Err(InvalidResponse);
    }
    if wire.status == NetworkDiagnosticStatus::CertificateRejected && wire.tls.is_some() {
        return Err(InvalidResponse);
    }
    Ok(NetworkDiagnosticReport {
        request,
        status: wire.status,
        addresses: wire.addresses,
        peer: wire.peer,
        tls: wire.tls,
        http: wire.http,
        icmp: wire.icmp,
        traceroute: wire.traceroute,
        timing: wire.timing,
        limited: wire.limited,
    })
}

fn validate_icmp(wire: &Wire) -> Result<(), NetworkDiagnosticError> {
    use NetworkDiagnosticError::InvalidResponse;
    use NetworkDiagnosticIcmpReason as Reason;
    use NetworkDiagnosticStatus as Status;
    if wire.peer.is_some()
        || wire.tls.is_some()
        || wire.http.is_some()
        || wire.timing.connect_ms.is_some()
        || wire.timing.tls_ms.is_some()
        || wire.timing.headers_ms.is_some()
    {
        return Err(InvalidResponse);
    }
    let Some(icmp) = &wire.icmp else {
        // Legacy supervisor terminals can fail before the ICMP adapter starts.
        return if matches!(
            wire.status,
            Status::Timeout
                | Status::Interrupted
                | Status::CleanupUnknown
                | Status::UnsupportedEnvironment
        ) && wire.addresses.is_empty()
            && wire.timing.resolve_ms.is_none()
        {
            Ok(())
        } else {
            Err(InvalidResponse)
        };
    };
    let status_matches_reason = match icmp.reason {
        Reason::Completed => wire.status == Status::Success,
        Reason::PacketLoss => wire.status == Status::IcmpPartial,
        Reason::NoEchoReply => wire.status == Status::IcmpNoReply,
        Reason::Deadline => wire.status == Status::Timeout,
        Reason::Interrupted => wire.status == Status::Interrupted,
        Reason::CleanupUnknown => wire.status == Status::CleanupUnknown,
        Reason::UtilityMissing => wire.status == Status::UtilityUnavailable,
        Reason::PermissionDenied => wire.status == Status::PermissionDenied,
        Reason::UnsupportedPlatform | Reason::UnsupportedImplementation => {
            wire.status == Status::UnsupportedEnvironment
        }
        Reason::ResolutionFailed => wire.status == Status::DnsFailure,
        Reason::ProcessFailed
        | Reason::InvalidOutput
        | Reason::OutputLimit
        | Reason::ReportedErrors => wire.status == Status::IcmpFailure,
    };
    if !status_matches_reason
        || icmp.address.as_ref().is_some_and(|address| {
            icmp.implementation.is_none()
                || wire.timing.resolve_ms.is_none()
                || wire.addresses.first().is_none_or(|first| {
                    first.address.parse::<IpAddr>().ok() != address.parse::<IpAddr>().ok()
                })
                || address.parse::<IpAddr>().is_err()
        })
        || (matches!(
            icmp.reason,
            Reason::UnsupportedImplementation | Reason::UtilityMissing
        ) && icmp.implementation.is_some())
    {
        return Err(InvalidResponse);
    }
    let Some(counts) = &icmp.counts else {
        return if icmp.rtt.is_none()
            && !matches!(
                icmp.reason,
                Reason::Completed
                    | Reason::PacketLoss
                    | Reason::NoEchoReply
                    | Reason::ReportedErrors
            ) {
            Ok(())
        } else {
            Err(InvalidResponse)
        };
    };
    if icmp.address.is_none()
        || icmp.implementation.is_none()
        || !(1..=4).contains(&counts.transmitted)
        || counts.received > counts.transmitted
        || (counts.received == 0 && counts.duplicates != 0)
        || counts.duplicates > 9999
        || counts.corrupted > 9999
        || counts.errors > 9999
        || (counts.received > 0) != icmp.rtt.is_some()
        || icmp.rtt.as_ref().is_some_and(|rtt| {
            rtt.min_us > rtt.avg_us
                || rtt.avg_us > rtt.max_us
                || rtt.max_us > 8_000_000
                || rtt.max_us > (wire.timing.total_ms + 1) * 1000
                || rtt.mdev_us > rtt.max_us
        })
    {
        return Err(InvalidResponse);
    }
    let counters_match = match icmp.reason {
        Reason::Completed => counts.transmitted == 4 && counts.received == 4,
        Reason::PacketLoss => counts.transmitted == 4 && (1..4).contains(&counts.received),
        Reason::NoEchoReply => counts.transmitted == 4 && counts.received == 0,
        Reason::ReportedErrors => {
            counts.transmitted == 4 && (counts.errors > 0 || counts.corrupted > 0)
        }
        Reason::Deadline => true,
        _ => false,
    };
    if !counters_match
        || (matches!(
            icmp.reason,
            Reason::Completed | Reason::PacketLoss | Reason::NoEchoReply
        ) && (counts.errors != 0 || counts.corrupted != 0))
    {
        return Err(InvalidResponse);
    }
    Ok(())
}

fn safe_text(text: &str, max: usize) -> bool {
    text.len() <= max && text.is_ascii() && !text.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
