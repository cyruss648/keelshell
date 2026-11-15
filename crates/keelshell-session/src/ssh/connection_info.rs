//! Credential-free identity sealed only after host verification and authentication.

use std::sync::Arc;

use super::{ProxyKind, SshOptions};

/// The requested endpoint and actual route of an authenticated SSH connection.
///
/// Fields cannot be constructed or changed by consumers. A jump is the snapshot
/// of the real parent connection, including its own verified key and proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshConnectionInfo {
    host: String,
    port: u16,
    username: String,
    verified_host_key: String,
    jump: Option<Arc<Self>>,
    proxy: Option<SshProxyInfo>,
}

/// An explicit proxy endpoint with no authentication material or userinfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshProxyInfo {
    kind: ProxyKind,
    host: String,
    port: u16,
}

impl SshConnectionInfo {
    // Only authenticate_stream calls this, after successful authentication.
    // check_server_key permits authentication only for an exact matching pin.
    pub(super) fn authenticated(
        options: &SshOptions,
        verified_host_key: String,
        jump: Option<Arc<Self>>,
    ) -> Self {
        Self {
            host: options.host.clone(),
            port: options.port,
            username: options.username.clone(),
            verified_host_key,
            jump,
            proxy: options.proxy.as_ref().map(|proxy| SshProxyInfo {
                kind: proxy.kind,
                host: proxy.host.clone(),
                port: proxy.port,
            }),
        }
    }

    /// Exact requested target host; no DNS canonicalization or profile lookup.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Exact requested SSH target port.
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Username that successfully authenticated on this hop.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// SHA-256 host fingerprint accepted by the host-key verification callback.
    pub fn verified_host_key(&self) -> &str {
        &self.verified_host_key
    }

    /// Actual authenticated parent hop, recursively retaining its route.
    pub fn jump(&self) -> Option<&Self> {
        self.jump.as_deref()
    }

    /// Explicit proxy used to reach this SSH hop, if any.
    pub fn proxy(&self) -> Option<&SshProxyInfo> {
        self.proxy.as_ref()
    }

    pub(crate) fn same_endpoint(&self, other: &Self) -> bool {
        self.host == other.host
            && self.port == other.port
            && self.username == other.username
            && self.verified_host_key == other.verified_host_key
    }

    pub(crate) fn same_route(&self, other: &Self) -> bool {
        self.jump == other.jump && self.proxy == other.proxy
    }

    /// Full ordered route, with every hop's user, host, port and verified key.
    ///
    /// Values are quoted and escaped so a control character cannot forge a new
    /// review line. Proxy authentication material is never held by this type.
    pub fn review_text(&self) -> String {
        let mut text = String::new();
        self.append_review(&mut text, 1);
        text
    }

    fn append_review(&self, text: &mut String, ordinal: usize) -> usize {
        use std::fmt::Write;

        let ordinal = self
            .jump
            .as_ref()
            .map_or(ordinal, |jump| jump.append_review(text, ordinal));
        match &self.proxy {
            Some(proxy) => {
                let _ = writeln!(
                    text,
                    "Hop {ordinal} explicit proxy: {:?} host={:?} port={}",
                    proxy.kind, proxy.host, proxy.port
                );
            }
            None => {
                let _ = writeln!(text, "Hop {ordinal} explicit proxy: none");
            }
        }
        let _ = writeln!(
            text,
            "Hop {ordinal} SSH: user={:?} host={:?} port={} verified fingerprint={:?}",
            self.username, self.host, self.port, self.verified_host_key
        );
        ordinal + 1
    }
}

impl SshProxyInfo {
    /// Explicit tunnel protocol.
    pub const fn kind(&self) -> ProxyKind {
        self.kind
    }

    /// Proxy DNS name or IP literal, validated before connection.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Explicit proxy port.
    pub const fn port(&self) -> u16 {
        self.port
    }
}
