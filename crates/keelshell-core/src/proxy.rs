//! Explicit upstream proxy metadata, without passwords or executable commands.

use serde::{Deserialize, Serialize};

use crate::{
    ValidationError,
    model::{normalized_host, text},
};

/// Protocol used to reach one SSH endpoint through an upstream proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    /// SOCKS version 5 CONNECT, with the configured authentication method.
    Socks5,
    /// HTTP CONNECT, optionally with explicit Basic authentication.
    HttpConnect,
}

/// Non-secret proxy authentication metadata. Passwords are supplied per attempt.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProxyAuthentication {
    /// Explicitly request a proxy connection without authentication.
    #[default]
    None,
    /// Authenticate this exact account using a separately supplied password.
    UsernamePassword {
        /// Case-sensitive account name, encoded as UTF-8 without trimming.
        username: String,
    },
}

// Internally tagged unit variants otherwise accept unknown fields. In
// particular, an imported password must fail instead of being silently ignored.
impl<'de> Deserialize<'de> for ProxyAuthentication {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum WireAuthentication {
            None {},
            UsernamePassword { username: String },
        }
        match WireAuthentication::deserialize(deserializer)? {
            WireAuthentication::None {} => Ok(Self::None),
            WireAuthentication::UsernamePassword { username } => {
                Ok(Self::UsernamePassword { username })
            }
        }
    }
}

/// Upstream proxy for one SSH hop. This is metadata, never a proxy password.
///
/// When the hop has an SSH parent, the parent reaches this proxy before the
/// proxy connects to the hop. Invalid or unavailable proxies must not fall back
/// to connecting to the SSH endpoint directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProxy {
    /// SOCKS5 or HTTP CONNECT; no protocol guessing is performed.
    pub kind: ProxyKind,
    /// ASCII proxy hostname or IP literal without a scheme, userinfo or brackets.
    /// Internationalized domain names must use their punycode A-label form.
    pub host: String,
    /// Proxy TCP port, in 1..=65535.
    pub port: u16,
    /// Explicit authentication method and optional account name, without a secret.
    pub auth: ProxyAuthentication,
}

impl ConnectionProxy {
    /// Create proxy metadata with no authentication, trimming only its hostname.
    /// Call [`Self::validate`] before storing or using this value.
    pub fn new(kind: ProxyKind, host: impl Into<String>, port: u16) -> Self {
        Self {
            kind,
            host: host.into().trim().to_owned(),
            port,
            auth: ProxyAuthentication::None,
        }
    }

    /// Validate endpoint and account metadata without network or credential I/O.
    ///
    /// Usernames use 1–255 UTF-8 bytes, contain no controls or outer whitespace,
    /// and retain their case and interior spaces. HTTP Basic usernames cannot
    /// contain `:`. Password validation belongs to the runtime transport.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0 {
            return Err(ValidationError::new(
                "connection.proxy.port",
                "must be nonzero",
            ));
        }
        validate_proxy_host("connection.proxy.host", &self.host)?;
        if let ProxyAuthentication::UsernamePassword { username } = &self.auth {
            text("connection.proxy.auth.username", username, 255, false)?;
            if username.len() > 255 {
                return Err(ValidationError::new(
                    "connection.proxy.auth.username",
                    "must use at most 255 UTF-8 bytes",
                ));
            }
            if self.kind == ProxyKind::HttpConnect && username.contains(':') {
                return Err(ValidationError::new(
                    "connection.proxy.auth.username",
                    "HTTP Basic usernames must not contain a colon",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn canonicalized(&self) -> Result<Self, ValidationError> {
        self.validate()?;
        Ok(Self {
            host: normalized_host(&self.host),
            ..self.clone()
        })
    }
}

/// The same wire-safe address boundary applies to the proxy and its SSH target.
/// Count the original input, including trailing dots; never resolve or apply IDNA.
pub(crate) fn validate_proxy_host(field: &'static str, host: &str) -> Result<(), ValidationError> {
    let invalid = || {
        ValidationError::new(
            field,
            "proxy routes require an ASCII hostname or IP literal of at most 253 bytes; use punycode for internationalized domains",
        )
    };
    if host.is_empty() || host.len() > 253 || !host.is_ascii() || host.starts_with('-') {
        return Err(invalid());
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    if host.trim_end_matches('.').is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(invalid());
    }
    Ok(())
}
