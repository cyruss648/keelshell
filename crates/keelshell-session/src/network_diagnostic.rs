//! Explicit protocol diagnostics executed only through authenticated SSH.
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

const SCRIPT: &str = include_str!("network_diagnostic/probe.py");
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
    if (kind == NetworkDiagnosticKind::Dns
        && (wire.peer.is_some() || wire.tls.is_some() || wire.http.is_some()))
        || (kind == NetworkDiagnosticKind::Tls && wire.http.is_some())
        || (kind == NetworkDiagnosticKind::Http && !request.https() && wire.tls.is_some())
    {
        return Err(InvalidResponse);
    }
    if wire.status == NetworkDiagnosticStatus::Success
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
        timing: wire.timing,
        limited: wire.limited,
    })
}

fn safe_text(text: &str, max: usize) -> bool {
    text.len() <= max && text.is_ascii() && !text.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
