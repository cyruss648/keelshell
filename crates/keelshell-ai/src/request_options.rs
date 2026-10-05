//! Validated, immutable HTTP headers and routing, without serializable secrets.

use std::{collections::BTreeSet, fmt};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;
use zeroize::Zeroizing;

use crate::AiError;

/// Explicit proxy authentication. Both fields are secrets and are zeroized on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct ProxyCredentials {
    username: Zeroizing<String>,
    password: Zeroizing<String>,
    basic: Zeroizing<String>,
    basic_authorization: Zeroizing<String>,
}

impl ProxyCredentials {
    /// Validate a nonempty username and password, each at most 255 UTF-8 bytes.
    /// A colon in the username is rejected because HTTP Basic authentication
    /// cannot represent it unambiguously. Controls are forbidden for both schemes.
    pub fn new(username: Zeroizing<String>, password: Zeroizing<String>) -> Result<Self, AiError> {
        if [&username, &password]
            .iter()
            .any(|v| v.is_empty() || v.len() > 255 || v.chars().any(char::is_control))
            || username.contains(':')
        {
            return Err(AiError::InvalidRequestOptions);
        }
        let pair = Zeroizing::new(format!("{}:{}", username.as_str(), password.as_str()));
        let basic = Zeroizing::new(STANDARD.encode(pair.as_bytes()));
        let basic_authorization = Zeroizing::new(format!("Basic {}", basic.as_str()));
        Ok(Self {
            username,
            password,
            basic,
            basic_authorization,
        })
    }
}

impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyCredentials").finish_non_exhaustive()
    }
}

/// A direct route or one explicit proxy. There are no exclusions or fallback routes.
#[derive(Clone, PartialEq, Eq, Default)]
pub enum ProxyRoute {
    /// Ignore system/environment proxies and connect directly.
    #[default]
    Direct,
    /// Use this validated proxy for every HTTP(S) request.
    Explicit {
        /// Credential-free proxy origin, suitable for human review.
        url: Url,
        /// Resolved authentication, never included in review or Debug output.
        credentials: Option<ProxyCredentials>,
    },
}

impl ProxyRoute {
    /// Accept HTTP(S), SOCKS5 (local DNS), or SOCKS5h (proxy DNS) origins.
    /// Reject URL authentication, query, fragment, paths, controls and port zero.
    pub fn explicit(url: &str, credentials: Option<ProxyCredentials>) -> Result<Self, AiError> {
        let parsed = Url::parse(url).map_err(|_| AiError::InvalidRequestOptions)?;
        if url.len() > 2048
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || !matches!(parsed.scheme(), "http" | "https" | "socks5" | "socks5h")
            || url.split_once("://").is_some_and(|(_, rest)| {
                rest.split(['/', '?', '#'])
                    .next()
                    .is_some_and(|authority| authority.contains('@'))
            })
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
            || parsed.port() == Some(0)
        {
            return Err(AiError::InvalidRequestOptions);
        }
        Ok(Self::Explicit {
            url: parsed,
            credentials,
        })
    }

    pub(crate) fn proxy(&self) -> Result<Option<reqwest::Proxy>, AiError> {
        match self {
            Self::Direct => Ok(None),
            Self::Explicit { url, credentials } => {
                // Explicitly omit NO_PROXY exclusions, including loopback.
                let mut proxy = reqwest::Proxy::all(url.as_str())
                    .map_err(|_| AiError::InvalidRequestOptions)?
                    .no_proxy(None);
                if let Some(credentials) = credentials {
                    proxy = proxy.basic_auth(&credentials.username, &credentials.password);
                }
                Ok(Some(proxy))
            }
        }
    }
}

impl fmt::Debug for ProxyRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Direct => "ProxyRoute::Direct",
            Self::Explicit { .. } => "ProxyRoute::Explicit(..)",
        })
    }
}

/// Resolved request options frozen into a reviewed provider configuration.
/// Values are bounded, sensitive and ephemeral; this type has no serialization API.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct RequestOptions {
    headers: Vec<(String, Zeroizing<String>)>,
    route: ProxyRoute,
    // These values are only a disclosure guard. They never become request headers.
    context_secrets: Vec<Zeroizing<String>>,
}

impl RequestOptions {
    /// Validate at most 32 unique custom headers. Names are ASCII HTTP tokens,
    /// at most 128 bytes; values are nonempty, control-free, at most 8 KiB each,
    /// and names plus values are bounded by 64 KiB in total.
    pub fn new(
        headers: Vec<(String, Zeroizing<String>)>,
        route: ProxyRoute,
    ) -> Result<Self, AiError> {
        if headers.len() > 32 {
            return Err(AiError::InvalidRequestOptions);
        }
        // The public route variant is checked again to prevent manual construction
        // from bypassing the credential-free URL invariant.
        if let ProxyRoute::Explicit { url, credentials } = &route {
            ProxyRoute::explicit(url.as_str(), credentials.clone())?;
        }
        let mut unique = BTreeSet::new();
        let mut bytes = 0usize;
        for (name, value) in &headers {
            if !valid_custom_header_name(name)
                || !unique.insert(name.to_ascii_lowercase())
                || value.is_empty()
                || value.len() > 8192
                || value.chars().any(char::is_control)
                || HeaderValue::from_str(value).is_err()
            {
                return Err(AiError::InvalidRequestOptions);
            }
            bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
        }
        if bytes > 65536 {
            return Err(AiError::InvalidRequestOptions);
        }
        Ok(Self {
            headers,
            route,
            context_secrets: Vec::new(),
        })
    }

    /// Bind all already-known credentials to this immutable disclosure boundary.
    ///
    /// Values are copied into zeroizing storage, deduplicated, never serialized,
    /// and never delivered as authentication or headers. Inactive configurations
    /// must participate too: provider-controlled model IDs and paging cursors
    /// cannot be forwarded merely because their credential belongs elsewhere.
    /// At most 4096 values, 1 MiB per value and 8 MiB total are accepted; excessive
    /// input fails closed with [`AiError::ContextTooLarge`]. Existing explicit
    /// header/proxy delivery is unaffected. Known values in custom header names
    /// are rejected before review or transport, including ASCII case changes
    /// made by HTTP header normalization. Equality binds this guard to approval.
    pub fn with_context_secrets(mut self, secrets: &[&str]) -> Result<Self, AiError> {
        if secrets.len() > 4096
            || secrets.iter().any(|secret| secret.len() > 1024 * 1024)
            || secrets
                .iter()
                .fold(0usize, |n, s| n.saturating_add(s.len()))
                > 8 * 1024 * 1024
        {
            return Err(AiError::ContextTooLarge);
        }
        let unique: BTreeSet<_> = secrets.iter().copied().filter(|s| !s.is_empty()).collect();
        self.context_secrets = unique
            .into_iter()
            .map(|secret| Zeroizing::new(secret.to_owned()))
            .collect();
        Ok(self)
    }

    /// Custom header names only; secret values are never exposed by this API.
    pub fn header_names(&self) -> impl Iterator<Item = &str> {
        self.headers.iter().map(|(n, _)| n.as_str())
    }

    /// Reviewable credential-free proxy origin, or `None` for direct routing.
    pub fn proxy_url(&self) -> Option<&str> {
        match &self.route {
            ProxyRoute::Direct => None,
            ProxyRoute::Explicit { url, .. } => Some(url.as_str()),
        }
    }

    /// Whether explicit proxy authentication is part of this snapshot.
    pub fn proxy_authenticated(&self) -> bool {
        matches!(
            self.route,
            ProxyRoute::Explicit {
                credentials: Some(_),
                ..
            }
        )
    }

    /// Redact text used in routing/header review using every resolved option secret.
    pub fn redact_for_review(&self, text: &str) -> String {
        crate::Redactor::new(&self.secrets()).redact(text).0
    }

    /// A human-review routing/header summary, with literal secret redaction.
    pub fn review_summary(&self) -> String {
        let route = self
            .proxy_url()
            .unwrap_or("Direct (environment proxies ignored)");
        let summary = format!(
            "Route: {route}; proxy authentication: {}; custom headers: {}",
            self.proxy_authenticated(),
            self.header_names().collect::<Vec<_>>().join(", ")
        );
        crate::Redactor::new(&self.secrets()).redact(&summary).0
    }

    pub(crate) fn header_map(&self) -> Result<HeaderMap, AiError> {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| AiError::InvalidRequestOptions)?;
            let mut value =
                HeaderValue::from_str(value).map_err(|_| AiError::InvalidRequestOptions)?;
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(headers)
    }

    /// Reject a metadata header name containing a known credential, including
    /// ASCII case changes made by HTTP name normalization. This checks one
    /// bounded HTTP token and grants no permission to deliver its value.
    pub fn validate_metadata_header_name(&self, name: &str) -> Result<(), AiError> {
        if name.is_empty() || name.len() > 128 || HeaderName::from_bytes(name.as_bytes()).is_err() {
            return Err(AiError::InvalidRequestOptions);
        }
        if self
            .secrets()
            .iter()
            .any(|secret| header_name_contains_secret(name, secret))
        {
            return Err(AiError::CredentialInContext);
        }
        Ok(())
    }

    /// Reject known credentials in already-bounded, user-controlled metadata.
    /// Literal and URL-percent-decoded forms use the same rule as endpoint/model
    /// admission. This neither resolves references nor reveals the matching text.
    pub fn validate_metadata_text(&self, text: &str) -> Result<(), AiError> {
        if self
            .secrets()
            .iter()
            .any(|secret| contains_context_secret(text, secret))
        {
            return Err(AiError::CredentialInContext);
        }
        Ok(())
    }

    // Names are metadata, never an explicitly authorized credential destination.
    // Check both snapshot values and credentials supplied only at review/send;
    // this invariant must not depend on with_context_secrets construction order.
    pub(crate) fn reject_header_name_secrets(&self, extra_secrets: &[&str]) -> Result<(), AiError> {
        let known = self.secrets();
        if self.headers.iter().any(|(name, _)| {
            known
                .iter()
                .copied()
                .chain(extra_secrets.iter().copied())
                .any(|secret| header_name_contains_secret(name, secret))
        }) {
            return Err(AiError::CredentialInContext);
        }
        Ok(())
    }

    pub(crate) fn proxy(&self) -> Result<Option<reqwest::Proxy>, AiError> {
        self.route.proxy()
    }

    pub(crate) fn secrets(&self) -> Vec<&str> {
        let mut secrets: Vec<_> = self.headers.iter().map(|(_, v)| v.as_str()).collect();
        if let ProxyRoute::Explicit {
            credentials: Some(credentials),
            ..
        } = &self.route
        {
            secrets.extend([
                credentials.username.as_str(),
                credentials.password.as_str(),
                credentials.basic.as_str(),
                credentials.basic_authorization.as_str(),
            ]);
        }
        secrets.extend(self.context_secrets.iter().map(|secret| secret.as_str()));
        secrets.sort_unstable();
        secrets.dedup();
        secrets
    }
}

fn header_name_contains_secret(name: &str, secret: &str) -> bool {
    // HTTP field names normalize ASCII case on the wire. Bound matching by the
    // name, rather than copying a potentially 1 MiB credential into lowercase.
    !secret.is_empty()
        && secret.len() <= name.len()
        && name
            .as_bytes()
            .windows(secret.len())
            .any(|part| part.eq_ignore_ascii_case(secret.as_bytes()))
}

/// URL normalization must not disguise a known value through percent encoding.
pub(crate) fn contains_context_secret(value: &str, secret: &str) -> bool {
    if secret.is_empty() {
        return false;
    }
    if value.contains(secret) {
        return true;
    }
    if !value.contains('%') {
        return false;
    }
    // Parse one form value to reuse URL's UTF-8 percent decoder, preserving
    // literal delimiters and '+' in paths rather than treating them as a query.
    let encoded = Zeroizing::new(format!(
        "value={}",
        value.replace('&', "%26").replace('+', "%2B")
    ));
    let decoded = url::form_urlencoded::parse(encoded.as_bytes())
        .next()
        .map(|(_, text)| Zeroizing::new(text.into_owned()));
    decoded.is_some_and(|text| text.contains(secret))
}

impl fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestOptions")
            .field("header_count", &self.headers.len())
            .field("route", &self.route)
            .finish()
    }
}

/// Validate a custom header name, excluding authentication, protocol, framing,
/// cookies, compression and hop-by-hop/routing headers managed by the transport.
pub fn valid_custom_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && HeaderName::from_bytes(name.as_bytes()).is_ok()
        && !["proxy-", "x-forwarded-", "sec-"]
            .iter()
            .any(|prefix| name.to_ascii_lowercase().starts_with(prefix))
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "host"
                | "authorization"
                | "proxy-authorization"
                | "proxy-authenticate"
                | "x-api-key"
                | "anthropic-version"
                | "content-type"
                | "content-length"
                | "connection"
                | "proxy-connection"
                | "transfer-encoding"
                | "upgrade"
                | "te"
                | "trailer"
                | "keep-alive"
                | "cookie"
                | "set-cookie"
                | "user-agent"
                | "accept"
                | "accept-encoding"
                | "content-encoding"
                | "expect"
                | "forwarded"
                | "via"
                | "x-forwarded-for"
                | "x-forwarded-host"
                | "x-forwarded-proto"
                | "x-original-url"
                | "x-rewrite-url"
                | "origin"
                | "referer"
                | "range"
                | "proxy"
                | "x-real-ip"
        )
}

#[cfg(test)]
mod header_name_tests {
    use super::{AiError, ProxyCredentials, ProxyRoute, RequestOptions};
    use zeroize::Zeroizing;

    fn options(name: &str, value: &str) -> RequestOptions {
        RequestOptions::new(
            vec![(name.to_owned(), Zeroizing::new(value.to_owned()))],
            ProxyRoute::Direct,
        )
        .unwrap_or_else(|error| panic!("fixture options: {error}"))
    }

    #[test]
    fn metadata_matching_covers_original_and_normalized_ascii_case_and_substrings() {
        for (name, secret) in [
            ("x-Known-Secret-tail", "Known-Secret"),
            ("x-known-secret-tail", "Known-Secret"),
            ("X-KNOWN-SECRET-TAIL", "known-secret"),
        ] {
            let options = options(name, "independent-value")
                .with_context_secrets(&[secret])
                .unwrap_or_else(|error| panic!("fixture secret: {error}"));
            assert!(matches!(
                options.reject_header_name_secrets(&[]),
                Err(AiError::CredentialInContext)
            ));
        }
    }

    #[test]
    fn explicit_value_delivery_does_not_authorize_a_metadata_name() {
        let direct = options("x-known-secret", "KNOWN-SECRET");
        assert!(direct.reject_header_name_secrets(&[]).is_err());
        let proxy = RequestOptions::new(
            vec![(
                "x-proxy-secret".into(),
                Zeroizing::new("independent-value".into()),
            )],
            ProxyRoute::explicit(
                "http://127.0.0.1:9",
                Some(
                    ProxyCredentials::new(
                        Zeroizing::new("PROXY-SECRET".into()),
                        Zeroizing::new("independent-password".into()),
                    )
                    .unwrap_or_else(|error| panic!("fixture credentials: {error}")),
                ),
            )
            .unwrap_or_else(|error| panic!("fixture route: {error}")),
        )
        .unwrap_or_else(|error| panic!("fixture proxy options: {error}"));
        assert!(proxy.reject_header_name_secrets(&[]).is_err());
    }

    #[test]
    fn safe_names_keep_explicit_delivery_and_ignore_empty_or_nonmatching_values() {
        let mut options = options("X-Approved-Key", "known-secret");
        options = options
            .with_context_secrets(&["known-secret", "", "approved-key-longer"])
            .unwrap_or_else(|error| panic!("fixture secrets: {error}"));
        assert!(
            options
                .reject_header_name_secrets(&["other-secret"])
                .is_ok()
        );
        let headers = options
            .header_map()
            .unwrap_or_else(|error| panic!("fixture headers: {error}"));
        assert_eq!(headers["x-approved-key"], "known-secret");
        assert!(headers["x-approved-key"].is_sensitive());
        assert!(
            options
                .reject_header_name_secrets(&["APPROVED-KEY"])
                .is_err()
        );
    }
}
