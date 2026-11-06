//! Structured traceroute wire contract, independently checked against consent.
use super::{NetworkDiagnosticError, NetworkDiagnosticStatus, Wire};
use keelshell_core::{NetworkDiagnosticRequest, TracerouteFamily};
use serde::Deserialize;
use std::net::IpAddr;

/// The single admitted upstream argument/output implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NetworkDiagnosticTracerouteImplementation {
    /// Dmitry Butskoy's Linux traceroute 2.1.5, without vendor output extensions.
    #[serde(rename = "linux_traceroute_2_1_5")]
    Linux215,
}

/// Fixed terminal reason; remote stderr and names never become display text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticTracerouteReason {
    /// A probe in the final complete row has the selected destination address.
    DestinationResponded,
    /// All requested hop rows completed without a destination reply.
    HopLimit,
    /// A complete row contains an admitted utility ICMP error annotation.
    ReportedUnreachable,
    /// The independent supervisor deadline stopped an owned stage.
    Deadline,
    /// Shutdown stopped and reaped the owned stage.
    Interrupted,
    /// Bounded actual wait could not confirm child exit.
    CleanupUnknown,
    /// Neither fixed executable location contained traceroute.
    UtilityMissing,
    /// Executable access or an exact socket errno denied permission.
    PermissionDenied,
    /// The target is not Linux/POSIX.
    UnsupportedPlatform,
    /// The exact upstream version output did not match.
    UnsupportedImplementation,
    /// The admitted command's utility rejected options or configuration.
    UnsupportedOptions,
    /// No address in the explicitly selected family was resolved.
    ResolutionFailed,
    /// An owned process could not start or exited unsuccessfully.
    ProcessFailed,
    /// Output was missing, malformed or inconsistent with the approved request.
    InvalidOutput,
    /// Combined stage stdout/stderr exceeded 32 KiB.
    OutputLimit,
}

/// One utility-observed probe slot. A star is unknown, never zero latency or loss.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticTracerouteProbe {
    /// Numeric response address, absent for an unanswered probe.
    pub address: Option<String>,
    /// Reported RTT in integer microseconds, absent with no response.
    pub rtt_us: Option<u64>,
    /// Strict upstream ICMP annotation such as `!H`; no arbitrary tool text.
    pub annotation: Option<String>,
}

/// One complete hop row with exactly the reviewed number of probe slots.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticTracerouteHop {
    /// Sequential hop/TTL starting at one.
    pub ttl: u8,
    /// Complete probe slots in utility output order.
    pub probes: Vec<NetworkDiagnosticTracerouteProbe>,
}

/// A request-bound traceroute outcome. Missing rows remain unknown.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDiagnosticTraceroute {
    /// Fixed result reason consistent with the outer status.
    pub reason: NetworkDiagnosticTracerouteReason,
    /// Present only after exact utility version admission.
    pub implementation: Option<NetworkDiagnosticTracerouteImplementation>,
    /// First resolved address in the selected family; never multiple traces.
    pub address: Option<String>,
    /// At most the reviewed hop limit; partial trailing rows are discarded.
    pub hops: Vec<NetworkDiagnosticTracerouteHop>,
}

fn annotation_is_valid(value: &str) -> bool {
    if matches!(value, "!N" | "!H" | "!X" | "!P" | "!S" | "!V" | "!C") {
        return true;
    }
    if let Some(mtu) = value.strip_prefix("!F-") {
        return !mtu.is_empty()
            && mtu.len() <= 10
            && mtu.bytes().all(|byte| byte.is_ascii_digit())
            && mtu.parse::<u32>().is_ok();
    }
    value
        .strip_prefix("!<")
        .and_then(|part| part.strip_suffix('>'))
        .is_some_and(|part| {
            let components: Vec<_> = part.split('-').collect();
            (1..=2).contains(&components.len())
                && components.iter().all(|part| {
                    !part.is_empty()
                        && part.len() <= 3
                        && part.bytes().all(|byte| byte.is_ascii_digit())
                        && part.parse::<u8>().is_ok()
                })
        })
}

pub(super) fn validate(
    request: &NetworkDiagnosticRequest,
    wire: &Wire,
) -> Result<(), NetworkDiagnosticError> {
    use NetworkDiagnosticError::InvalidResponse;
    use NetworkDiagnosticStatus as Status;
    use NetworkDiagnosticTracerouteReason as Reason;
    let options = request.traceroute_options().ok_or(InvalidResponse)?;
    if wire.icmp.is_some()
        || wire.peer.is_some()
        || wire.tls.is_some()
        || wire.http.is_some()
        || wire.timing.connect_ms.is_some()
        || wire.timing.tls_ms.is_some()
        || wire.timing.headers_ms.is_some()
        || wire.addresses.iter().any(|address| {
            address
                .address
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_ipv4() != (options.family() == TracerouteFamily::Ipv4))
        })
    {
        return Err(InvalidResponse);
    }
    let Some(trace) = &wire.traceroute else {
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
    let status = match trace.reason {
        Reason::DestinationResponded => Status::Success,
        Reason::HopLimit => Status::TracerouteHopLimit,
        Reason::ReportedUnreachable => Status::TracerouteUnreachable,
        Reason::Deadline => Status::Timeout,
        Reason::Interrupted => Status::Interrupted,
        Reason::CleanupUnknown => Status::CleanupUnknown,
        Reason::UtilityMissing => Status::UtilityUnavailable,
        Reason::PermissionDenied => Status::PermissionDenied,
        Reason::UnsupportedPlatform
        | Reason::UnsupportedImplementation
        | Reason::UnsupportedOptions => Status::UnsupportedEnvironment,
        Reason::ResolutionFailed => Status::DnsFailure,
        Reason::ProcessFailed | Reason::InvalidOutput | Reason::OutputLimit => {
            Status::TracerouteFailure
        }
    };
    if wire.status != status
        || trace.hops.len() > usize::from(options.max_hops())
        || trace.address.is_some()
            != (!wire.addresses.is_empty() && wire.timing.resolve_ms.is_some())
        || (trace.address.is_none()
            && (!wire.addresses.is_empty() || wire.timing.resolve_ms.is_some()))
        || trace.address.as_ref().is_some_and(|address| {
            trace.implementation.is_none()
                || address.parse::<IpAddr>().is_err()
                || wire
                    .addresses
                    .first()
                    .is_none_or(|first| &first.address != address)
                || request
                    .host()
                    .parse::<IpAddr>()
                    .is_ok_and(|literal| address.parse::<IpAddr>().ok() != Some(literal))
        })
        || (matches!(
            trace.reason,
            Reason::UnsupportedPlatform | Reason::UnsupportedImplementation
        ) && trace.implementation.is_some())
        || (!trace.hops.is_empty() && (trace.address.is_none() || trace.implementation.is_none()))
    {
        return Err(InvalidResponse);
    }
    for (index, hop) in trace.hops.iter().enumerate() {
        if usize::from(hop.ttl) != index + 1
            || hop.probes.len() != usize::from(options.probes_per_hop())
        {
            return Err(InvalidResponse);
        }
        for probe in &hop.probes {
            if probe.address.is_some() != probe.rtt_us.is_some()
                || probe.address.as_ref().is_some_and(|value| {
                    value.parse::<IpAddr>().ok().is_none_or(|ip| {
                        ip.is_ipv4() != (options.family() == TracerouteFamily::Ipv4)
                    })
                })
                || probe.rtt_us.is_some_and(|rtt| {
                    rtt > u64::from(options.wait_ms()) * 1000
                        || rtt > (wire.timing.total_ms + 1) * 1000
                })
                || probe.annotation.as_ref().is_some_and(|annotation| {
                    probe.address.is_none() || !annotation_is_valid(annotation)
                })
            {
                return Err(InvalidResponse);
            }
        }
    }
    let error_reported = trace
        .hops
        .iter()
        .flat_map(|hop| &hop.probes)
        .any(|p| p.annotation.is_some());
    let destination_responded = trace.hops.last().is_some_and(|hop| {
        hop.probes
            .iter()
            .any(|p| p.address.is_some() && p.address == trace.address && p.annotation.is_none())
    });
    let consistent = match trace.reason {
        Reason::DestinationResponded => destination_responded && !error_reported,
        Reason::HopLimit => {
            trace.hops.len() == usize::from(options.max_hops())
                && !destination_responded
                && !error_reported
        }
        Reason::ReportedUnreachable => error_reported,
        Reason::Deadline | Reason::Interrupted => true,
        _ => trace.hops.is_empty(),
    };
    if !consistent {
        return Err(InvalidResponse);
    }
    Ok(())
}
