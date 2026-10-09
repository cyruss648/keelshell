//! Validated, credential-free inputs for explicit diagnostics executed by SSH.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Maximum input bytes accepted before URL parsing or transport work.
pub const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 2048;
/// Combined stdout/stderr budget for one diagnostic response.
pub const MAX_DIAGNOSTIC_OUTPUT_BYTES: usize = 32 * 1024;
/// Remote wall-clock budget; the SSH envelope additionally bounds channel work.
pub const DIAGNOSTIC_REMOTE_SECONDS: u64 = 8;

/// Explicit application protocol to inspect from the authenticated remote host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDiagnosticKind {
    /// Resolve through the remote operating system's resolver, including NSS/hosts.
    Dns,
    /// Perform a verified TLS handshake without sending application payload.
    Tls,
    /// Send a credential-free HTTP(S) HEAD request without following redirects.
    Http,
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
}

impl NetworkDiagnosticRequest {
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
    /// Numeric target port. DNS queries do not open this service port.
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Whether HTTP requires strict TLS verification.
    pub fn https(&self) -> bool {
        self.https
    }
    /// Request path; empty for DNS/TLS.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Credential-free endpoint for the exact operation review and result label.
    pub fn endpoint_label(&self) -> String {
        if self.kind == NetworkDiagnosticKind::Dns {
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
