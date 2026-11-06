//! Validated, credential-free inputs for explicit diagnostics executed by SSH.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Maximum input bytes accepted before URL parsing or transport work.
pub const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 2048;
/// Combined stdout/stderr budget for one diagnostic response.
pub const MAX_DIAGNOSTIC_OUTPUT_BYTES: usize = 32 * 1024;
/// Remote wall-clock budget; the SSH envelope additionally bounds channel work.
pub const DIAGNOSTIC_REMOTE_SECONDS: u64 = 8;

/// Explicit network operation to inspect from the authenticated remote host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticKind {
    /// Resolve through the remote operating system's resolver, including NSS/hosts.
    Dns,
    /// Perform a verified TLS handshake without sending application payload.
    Tls,
    /// Send a credential-free HTTP(S) HEAD request without following redirects.
    Http,
    /// Send at most four ICMP echo requests using an admitted remote ping utility.
    Icmp,
    /// Trace a single reviewed destination using the admitted Linux traceroute.
    Traceroute,
}

/// Probe method explicitly reviewed for one traceroute, with no automatic fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TracerouteProtocol {
    /// Classic UDP probes starting at destination port 33434, increasing per probe.
    Udp,
    /// ICMP echo probes; unavailable permissions are reported without escalation.
    Icmp,
}

/// Address family selected before remote resolution; another family is never tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TracerouteFamily {
    /// Resolve and trace one IPv4 address.
    Ipv4,
    /// Resolve and trace one IPv6 address if supported by the remote environment.
    Ipv6,
}

/// Validated traceroute limits. Sequential probes have at most six seconds of
/// cumulative requested waiting; resolution/admission and cleanup share the
/// existing eight-second remote budget. A deadline is never a completed route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NetworkDiagnosticTracerouteOptions {
    protocol: TracerouteProtocol,
    family: TracerouteFamily,
    max_hops: u8,
    probes_per_hop: u8,
    wait_ms: u16,
}

impl NetworkDiagnosticTracerouteOptions {
    /// The explicit probe method.
    pub fn protocol(self) -> TracerouteProtocol {
        self.protocol
    }
    /// The requested IP family, without fallback.
    pub fn family(self) -> TracerouteFamily {
        self.family
    }
    /// Maximum TTL/hop limit, in 1–16.
    pub fn max_hops(self) -> u8 {
        self.max_hops
    }
    /// Probe attempts per hop, in 1–3.
    pub fn probes_per_hop(self) -> u8 {
        self.probes_per_hop
    }
    /// Maximum wait for each sequential probe, in 100–1000 milliseconds.
    pub fn wait_ms(self) -> u16 {
        self.wait_ms
    }
    /// Maximum requested attempts, including probes that may remain unsent if stopped.
    pub fn max_probes(self) -> u16 {
        u16::from(self.max_hops) * u16::from(self.probes_per_hop)
    }
    /// Upper bound on cumulative requested probe waiting, excluding other stages.
    pub fn probe_wait_budget_ms(self) -> u32 {
        u32::from(self.max_probes()) * u32::from(self.wait_ms)
    }
}

/// Validated immutable diagnostic input. It never contains URL credentials,
/// query parameters, fragments, headers, cookies or a request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkDiagnosticRequest {
    kind: NetworkDiagnosticKind,
    host: String,
    port: u16,
    path: String,
    https: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    traceroute: Option<NetworkDiagnosticTracerouteOptions>,
}

/// Fixed validation failure, deliberately excluding the supplied input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NetworkDiagnosticInputError {
    /// Input is empty, too long, contains whitespace/control characters or is unsafe.
    #[error("invalid diagnostic host or URL")]
    InvalidInput,
    /// Only DNS names, IP literals and explicit HTTP(S) URLs are accepted.
    #[error("unsupported diagnostic endpoint")]
    UnsupportedEndpoint,
    /// URL user information, query parameters and fragments are not accepted.
    #[error("diagnostics do not accept credentials, query parameters or fragments")]
    CredentialsOrQuery,
    /// A port must be in 1–65535.
    #[error("invalid diagnostic port")]
    InvalidPort,
    /// Traceroute limits, cumulative waiting budget or literal IP family are invalid.
    #[error(
        "traceroute requires 1–16 hops, 1–3 probes, 100–1000 ms wait, and at most 6000 ms cumulative waiting in the selected IP family"
    )]
    InvalidTracerouteOptions,
}

impl NetworkDiagnosticRequest {
    /// Validate a single remote Linux traceroute without resolving or executing.
    ///
    /// Only a hostname or numeric literal is accepted. The protocol and family
    /// are explicit; at most 16 hops × 3 probes are possible, and their combined
    /// requested waiting must be at most 6000 ms. No caller-supplied tool, flags,
    /// port, interface, source address or command text is accepted.
    pub fn traceroute(
        host: &str,
        protocol: TracerouteProtocol,
        family: TracerouteFamily,
        max_hops: u8,
        probes_per_hop: u8,
        wait_ms: u16,
    ) -> Result<Self, NetworkDiagnosticInputError> {
        let mut request = Self::endpoint(NetworkDiagnosticKind::Traceroute, host, 33434)?;
        let options = NetworkDiagnosticTracerouteOptions {
            protocol,
            family,
            max_hops,
            probes_per_hop,
            wait_ms,
        };
        if !(1..=16).contains(&max_hops)
            || !(1..=3).contains(&probes_per_hop)
            || !(100..=1000).contains(&wait_ms)
            || options.probe_wait_budget_ms() > 6000
            || request
                .host
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_ipv4() != (family == TracerouteFamily::Ipv4))
        {
            return Err(NetworkDiagnosticInputError::InvalidTracerouteOptions);
        }
        request.traceroute = Some(options);
        Ok(request)
    }

    /// Exact reviewed traceroute options; absent for other diagnostic kinds.
    pub fn traceroute_options(&self) -> Option<NetworkDiagnosticTracerouteOptions> {
        self.traceroute
    }

    /// Validate a host for an explicitly reviewed remote ICMP echo operation.
    ///
    /// Validation performs no resolution or probing. Flags, URLs, shell syntax
    /// and IPv6 zone identifiers are rejected. Count and budgets are fixed by
    /// the transport; callers cannot supply executable names or utility options.
    pub fn icmp(host: &str) -> Result<Self, NetworkDiagnosticInputError> {
        Self::endpoint(NetworkDiagnosticKind::Icmp, host, 443)
    }

    /// Validate a remote system-resolver query. This does not query DNS locally.
    pub fn dns(host: &str) -> Result<Self, NetworkDiagnosticInputError> {
        Self::endpoint(NetworkDiagnosticKind::Dns, host, 443)
    }

    /// Validate a host and port for a strict remote TLS handshake.
    pub fn tls(host: &str, port: u16) -> Result<Self, NetworkDiagnosticInputError> {
        Self::endpoint(NetworkDiagnosticKind::Tls, host, port)
    }

    fn endpoint(
        kind: NetworkDiagnosticKind,
        host: &str,
        port: u16,
    ) -> Result<Self, NetworkDiagnosticInputError> {
        if port == 0 {
            return Err(NetworkDiagnosticInputError::InvalidPort);
        }
        let host = checked_host(host)?;
        Ok(Self {
            kind,
            host,
            port,
            path: String::new(),
            https: true,
            traceroute: None,
        })
    }

    /// Validate an explicit HTTP(S) URL. Redirects are observed as status codes,
    /// never followed. Query/fragment/userinfo rejection is intentional because
    /// diagnostics have no credential or signed-URL review flow.
    pub fn http(input: &str) -> Result<Self, NetworkDiagnosticInputError> {
        checked_input(input)?;
        // Reject syntax before normalization: URL parsers can discard fragments
        // and whitespace, normalize backslashes, or decode authority userinfo.
        if input.contains(['?', '#', '@']) {
            return Err(NetworkDiagnosticInputError::CredentialsOrQuery);
        }
        if input.contains('\\') {
            return Err(NetworkDiagnosticInputError::InvalidInput);
        }
        let url = url::Url::parse(input).map_err(|_| NetworkDiagnosticInputError::InvalidInput)?;
        if !matches!(url.scheme(), "http" | "https") || !input.contains("://") {
            return Err(NetworkDiagnosticInputError::UnsupportedEndpoint);
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(NetworkDiagnosticInputError::CredentialsOrQuery);
        }
        let host = match url
            .host()
            .ok_or(NetworkDiagnosticInputError::InvalidInput)?
        {
            url::Host::Domain(name) => checked_host(name)?,
            url::Host::Ipv4(ip) => ip.to_string(),
            url::Host::Ipv6(ip) => ip.to_string(),
        };
        let port = url
            .port_or_known_default()
            .filter(|port| *port > 0)
            .ok_or(NetworkDiagnosticInputError::InvalidPort)?;
        let path = url.path();
        // Reject encoded controls and delimiters as well as literal controls.
        // This also prevents CRLF request splitting in downstream implementations.
        let bytes = path.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                let part = path
                    .get(index + 1..index + 3)
                    .ok_or(NetworkDiagnosticInputError::InvalidInput)?;
                let byte = u8::from_str_radix(part, 16)
                    .map_err(|_| NetworkDiagnosticInputError::InvalidInput)?;
                if byte <= 0x20 || byte == 0x7f || matches!(byte, b'@' | b'?' | b'#' | b'\\') {
                    return Err(NetworkDiagnosticInputError::InvalidInput);
                }
                index += 3;
            } else {
                index += 1;
            }
        }
        Ok(Self {
            kind: NetworkDiagnosticKind::Http,
            host,
            port,
            path: path.to_owned(),
            https: url.scheme() == "https",
            traceroute: None,
        })
    }

    /// The operation selected by the user.
    pub fn kind(&self) -> NetworkDiagnosticKind {
        self.kind
    }
    /// Exact validated host, without IPv6 brackets.
    pub fn host(&self) -> &str {
        &self.host
    }
    /// Numeric target port. DNS/ICMP do not open this service port; traceroute
    /// uses fixed UDP ports starting at 33434 only when UDP is selected.
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Whether HTTP requires strict TLS verification.
    pub fn https(&self) -> bool {
        self.https
    }
    /// Request path; empty for DNS/TLS/ICMP/traceroute.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Credential-free endpoint for the exact operation review and result label.
    pub fn endpoint_label(&self) -> String {
        if matches!(
            self.kind,
            NetworkDiagnosticKind::Dns
                | NetworkDiagnosticKind::Icmp
                | NetworkDiagnosticKind::Traceroute
        ) {
            return self.host.clone();
        }
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.kind == NetworkDiagnosticKind::Tls {
            format!("{host}:{}", self.port)
        } else {
            format!(
                "{}://{host}:{}{}",
                if self.https { "https" } else { "http" },
                self.port,
                self.path
            )
        }
    }
}

fn checked_input(input: &str) -> Result<(), NetworkDiagnosticInputError> {
    if input.is_empty()
        || input.len() > MAX_DIAGNOSTIC_INPUT_BYTES
        || !input.is_ascii()
        || input.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(NetworkDiagnosticInputError::InvalidInput);
    }
    Ok(())
}

fn checked_host(input: &str) -> Result<String, NetworkDiagnosticInputError> {
    checked_input(input)?;
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    if input.len() > 253
        || input
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(NetworkDiagnosticInputError::InvalidInput);
    }
    let labels = input.strip_suffix('.').unwrap_or(input).split('.');
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(NetworkDiagnosticInputError::InvalidInput);
        }
    }
    Ok(input.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traceroute_rejects_argument_injection_without_echoing_input() {
        for host in [
            "",
            "--help",
            "-I",
            "a;id",
            "$(id)",
            "`id`",
            "a\n",
            "example.test -m 99",
            "fe80::1%eth0",
            "[::1]",
            "127.1",
            "https://user:secret@example.test/",
        ] {
            let error = NetworkDiagnosticRequest::traceroute(
                host,
                TracerouteProtocol::Udp,
                TracerouteFamily::Ipv4,
                6,
                1,
                500,
            )
            .err()
            .unwrap_or_else(|| panic!("unsafe host must be rejected"));
            assert!(!error.to_string().contains("secret"));
        }
    }

    #[test]
    fn traceroute_limits_include_cumulative_wait_and_literal_family() {
        for (hops, probes, wait) in [
            (0, 1, 500),
            (17, 1, 100),
            (1, 0, 500),
            (1, 4, 500),
            (1, 1, 99),
            (1, 1, 1001),
            (16, 3, 1000),
            (7, 1, 1000),
        ] {
            assert_eq!(
                NetworkDiagnosticRequest::traceroute(
                    "example.test",
                    TracerouteProtocol::Udp,
                    TracerouteFamily::Ipv4,
                    hops,
                    probes,
                    wait,
                ),
                Err(NetworkDiagnosticInputError::InvalidTracerouteOptions)
            );
        }
        assert_eq!(
            NetworkDiagnosticRequest::traceroute(
                "::1",
                TracerouteProtocol::Icmp,
                TracerouteFamily::Ipv4,
                1,
                1,
                100,
            ),
            Err(NetworkDiagnosticInputError::InvalidTracerouteOptions)
        );
        assert_eq!(
            NetworkDiagnosticRequest::traceroute(
                "127.0.0.1",
                TracerouteProtocol::Icmp,
                TracerouteFamily::Ipv6,
                1,
                1,
                100,
            ),
            Err(NetworkDiagnosticInputError::InvalidTracerouteOptions)
        );
    }

    #[test]
    fn traceroute_serialization_preserves_complete_reviewed_request() {
        for protocol in [TracerouteProtocol::Udp, TracerouteProtocol::Icmp] {
            let request = NetworkDiagnosticRequest::traceroute(
                "2001:db8::1",
                protocol,
                TracerouteFamily::Ipv6,
                16,
                3,
                125,
            )
            .unwrap_or_else(|error| panic!("bounded IPv6 request: {error}"));
            let options = request
                .traceroute_options()
                .unwrap_or_else(|| panic!("traceroute options"));
            assert_eq!(
                (options.max_probes(), options.probe_wait_budget_ms()),
                (48, 6000)
            );
            let value = serde_json::to_value(&request)
                .unwrap_or_else(|error| panic!("serialize approved request: {error}"));
            assert_eq!(value["traceroute"]["family"], "ipv6");
            assert_eq!(value["traceroute"]["max_hops"], 16);
            assert_eq!(value["traceroute"]["probes_per_hop"], 3);
            assert_eq!(value["traceroute"]["wait_ms"], 125);
            assert_eq!(request.endpoint_label(), "2001:db8::1");
            assert_eq!(request.kind(), NetworkDiagnosticKind::Traceroute);
        }
    }

    #[test]
    fn icmp_admission_is_host_only_and_never_accepts_utility_options() {
        for host in [
            "-c4",
            "--help",
            "example.test;id",
            "$(id)",
            "`id`",
            "example.test -c 9",
            "example.test\n",
            "fe80::1%eth0",
            "[::1]",
            "127.1",
            "https://example.test/",
            "https://user:secret@example.test/?token=secret",
        ] {
            assert!(NetworkDiagnosticRequest::icmp(host).is_err(), "{host:?}");
        }
        for (host, expected) in [
            ("EXAMPLE.test.", "example.test."),
            ("192.0.2.1", "192.0.2.1"),
            ("2001:db8::1", "2001:db8::1"),
        ] {
            let request = NetworkDiagnosticRequest::icmp(host)
                .unwrap_or_else(|error| panic!("admitted ICMP host: {error}"));
            assert_eq!(request.kind(), NetworkDiagnosticKind::Icmp);
            assert_eq!(request.endpoint_label(), expected);
            assert!(request.path().is_empty());
        }
    }

    #[test]
    fn endpoint_validation_precedes_every_transport() {
        for host in [
            "",
            "-a",
            "a;id",
            "a\n",
            "a\u{202e}",
            "127.1",
            "127.0.0.999",
            "a..b",
            "a/b",
            "$(id)",
            "fe80::1%en0",
        ] {
            assert!(NetworkDiagnosticRequest::dns(host).is_err(), "{host:?}");
        }
        assert_eq!(
            NetworkDiagnosticRequest::dns("EXAMPLE.test.").map(|r| r.host().to_owned()),
            Ok("example.test.".into())
        );
        assert!(NetworkDiagnosticRequest::tls("::1", 443).is_ok());
        assert_eq!(
            NetworkDiagnosticRequest::tls("valid.test", 0),
            Err(NetworkDiagnosticInputError::InvalidPort)
        );
    }
    #[test]
    fn urls_cannot_smuggle_credentials_controls_or_other_protocols() {
        for input in [
            "https://user:pass@example.test/",
            "https://example.test/?token=x",
            "https://example.test/#x",
            "https://example.test/a%0d%0aX:y",
            "https://example.test/a%7f",
            "https://example.test/a%40x",
            "https://example.test/\\evil",
            "https://example.test/\n",
            "https://example.test/%xx",
            "file:///tmp/x",
            "ftp://example.test/",
            "http://example.test:0/",
        ] {
            assert!(NetworkDiagnosticRequest::http(input).is_err(), "{input:?}");
        }
        let request = NetworkDiagnosticRequest::http("https://[::1]:8443/health")
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(request.host(), "::1");
        assert_eq!(request.path(), "/health");
        assert_eq!(request.endpoint_label(), "https://[::1]:8443/health");
    }
    #[test]
    fn errors_do_not_echo_rejected_input() {
        let error = NetworkDiagnosticRequest::http("https://password:private@example.test/").err();
        assert!(!format!("{error:?}").contains("private"));
        assert!(
            NetworkDiagnosticRequest::http(&format!(
                "http://example.test/{}",
                "x".repeat(MAX_DIAGNOSTIC_INPUT_BYTES)
            ))
            .is_err()
        );
    }
}
