//! Explicit upstream tunnels; no environment proxy lookup or direct fallback.
mod http;
mod socks;
mod stream;

use std::{fmt, net::IpAddr};
use tokio::io::{AsyncRead, AsyncWrite};
use zeroize::Zeroizing;

use crate::{Result, SessionError};
use stream::PrefixedStream;

/// Explicit upstream tunnel protocol for one SSH hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    /// SOCKS5 CONNECT with remote destination DNS resolution.
    Socks5,
    /// HTTP CONNECT using an unencrypted HTTP proxy connection.
    HttpConnect,
}

/// Ephemeral UTF-8 proxy credentials. Debug deliberately hides both fields.
///
/// SOCKS5 requires 1–255 bytes for each field. HTTP Basic requires a username
/// of 1–255 bytes without `:` and a password of 0–4096 bytes. Neither protocol
/// encrypts credentials itself. Fields are never trimmed or normalized.
pub struct ProxyCredentials {
    /// Exact account name, with no controls or surrounding whitespace.
    pub username: Zeroizing<String>,
    /// Exact secret; HTTP Basic excludes control characters.
    pub password: Zeroizing<String>,
}
impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyCredentials([redacted])")
    }
}

/// Upstream proxy for one SSH hop, without implicit environment configuration.
///
/// For a hop through SSH, the parent opens a channel to this proxy first.
/// Credentials select exactly username/password authentication; their absence
/// selects anonymous access. No other authentication method or route is tried.
pub struct SshProxy {
    /// Tunnel protocol.
    pub kind: ProxyKind,
    /// Proxy DNS name or unbracketed IP literal.
    pub host: String,
    /// Nonzero proxy port.
    pub port: u16,
    /// Ephemeral credentials, erased on drop and excluded from diagnostics.
    pub credentials: Option<ProxyCredentials>,
}
impl fmt::Debug for SshProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Even invalid, manually constructed options must not leak userinfo.
        f.debug_struct("SshProxy")
            .field("kind", &self.kind)
            .field("endpoint", &"[redacted]")
            .field("authenticated", &self.credentials.is_some())
            .finish()
    }
}

/// Proxy failures contain no received header values, reason text or secrets.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// The HTTP proxy requires authentication.
    #[error("proxy authentication is required")]
    AuthenticationRequired,
    /// The proxy rejected the configured credentials.
    #[error("proxy authentication was rejected")]
    AuthenticationRejected,
    /// The proxy did not select the sole configured authentication method.
    #[error("proxy authentication method is unsupported")]
    UnsupportedAuthentication,
    /// The SOCKS5 proxy returned a nonzero reply code.
    #[error("SOCKS5 proxy rejected CONNECT (code {0})")]
    Socks5Rejected(u8),
    /// The HTTP proxy returned a final non-success status.
    #[error("HTTP proxy rejected CONNECT (status {0})")]
    HttpRejected(u16),
    /// The peer sent a malformed or unsupported response.
    #[error("invalid proxy response: {0}")]
    InvalidResponse(&'static str),
    /// Header bytes, field count or informational response count exceeded bounds.
    #[error("proxy response exceeds protocol limits")]
    ResponseTooLarge,
}

impl SshProxy {
    pub(super) fn validate(&self, target: &str) -> Result<()> {
        validate_host(&self.host)?;
        validate_host(target)?;
        if self.port == 0 {
            return Err(SessionError::Invalid("proxy port must be nonzero"));
        }
        if let Some(credentials) = &self.credentials {
            let username = credentials.username.as_str();
            let password = credentials.password.as_str();
            if username.is_empty()
                || username.len() > 255
                || username.trim() != username
                || username.chars().any(char::is_control)
            {
                return Err(SessionError::Invalid("invalid proxy username"));
            }
            match self.kind {
                ProxyKind::Socks5 if password.is_empty() || password.len() > 255 => {
                    return Err(SessionError::Invalid("invalid SOCKS5 password length"));
                }
                ProxyKind::HttpConnect
                    if username.contains(':')
                        || password.len() > 4096
                        || password.chars().any(char::is_control) =>
                {
                    return Err(SessionError::Invalid("invalid HTTP Basic credentials"));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn validate_host(host: &str) -> Result<()> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    if host.trim_end_matches('.').is_empty()
        || host.len() > 253
        || host.starts_with('-')
        || !host.is_ascii()
        || host
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(SessionError::Invalid(
            "proxy hosts must be DNS names or IP literals",
        ));
    }
    Ok(())
}

pub(super) async fn negotiate<S>(
    mut stream: S,
    proxy: Option<&SshProxy>,
    host: &str,
    port: u16,
) -> Result<PrefixedStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let prefix = match proxy {
        None => Vec::new(),
        Some(proxy) => match proxy.kind {
            ProxyKind::Socks5 => {
                socks::connect(&mut stream, proxy.credentials.as_ref(), host, port).await?;
                Vec::new()
            }
            ProxyKind::HttpConnect => {
                http::connect(&mut stream, proxy.credentials.as_ref(), host, port).await?
            }
        },
    };
    Ok(PrefixedStream::new(stream, prefix))
}

#[cfg(test)]
mod tests;
