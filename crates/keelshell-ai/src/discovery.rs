//! Explicit settings operations for one validated AI provider destination.

use std::{
    fmt,
    future::Future,
    time::{Duration, Instant},
};

use reqwest::{
    Client, Method,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use serde::Deserialize;
use tokio::sync::watch;
use url::{Host, Url};
use zeroize::Zeroizing;

use crate::provider::{
    ProviderProtocol, parse_assistant_response, payload_contains_secret, valid_model,
};
use crate::{AiError, ApprovedRequest, AssistantReply, ProviderConfig, Redactor};

/// Exact prompt used only after the user explicitly requests a connection test.
/// No terminal content, host, files, user question or environment is included.
pub const CONNECTIVITY_PROMPT: &str = "Reply with the single word OK. This is a connection test.";

/// A validated HTTP destination independent of a selected model or secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderEndpoint {
    pub(crate) url: Url,
    pub(crate) protocol: ProviderProtocol,
}

impl ProviderEndpoint {
    /// Accept verified HTTPS or loopback HTTP, without URL authentication,
    /// query, fragment, whitespace or control characters.
    pub fn new(endpoint: &str) -> Result<Self, AiError> {
        // Preserve the original constructor's path-based discovery behavior for
        // existing callers while new_with_protocol remains the authority for
        // authentication and wire-format selection.
        let protocol = if endpoint.ends_with("/messages") {
            ProviderProtocol::AnthropicMessages
        } else if endpoint.ends_with("/responses") {
            ProviderProtocol::Responses
        } else {
            ProviderProtocol::ChatCompletions
        };
        Self::new_with_protocol(endpoint, protocol)
    }

    /// Validate an endpoint while binding its wire protocol for discovery and
    /// authentication. The protocol is explicit so `/messages` is never guessed
    /// to be an OpenAI-compatible endpoint.
    pub fn new_with_protocol(endpoint: &str, protocol: ProviderProtocol) -> Result<Self, AiError> {
        if endpoint.len() > 2048
            || endpoint
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(AiError::InvalidEndpoint);
        }
        let url = Url::parse(endpoint).map_err(|_| AiError::InvalidEndpoint)?;
        let loopback = match url.host() {
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        if url.host().is_none()
            || url.port() == Some(0)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        {
            return Err(AiError::InvalidEndpoint);
        }
        Ok(Self { url, protocol })
    }

    /// Exact normalized provider destination to show in the configuration UI.
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// Derive the model catalog URL by replacing a literal final
    /// `/chat/completions`, `/responses` or Anthropic `/messages` with
    /// `/models`, retaining the origin and path prefix.
    ///
    /// Unknown paths fail before network access. No endpoint is guessed, probed
    /// or followed to another host when this convention is unsupported.
    pub fn models_endpoint(&self) -> Result<String, AiError> {
        let path = self.url.path();
        let suffix = match self.protocol {
            ProviderProtocol::ChatCompletions => "/chat/completions",
            ProviderProtocol::Responses => "/responses",
            ProviderProtocol::AnthropicMessages => "/messages",
        };
        let prefix = path
            .strip_suffix(suffix)
            .or_else(|| {
                // `ProviderEndpoint::new` predates explicit protocol selection and
                // historically accepted both OpenAI endpoint suffixes for discovery.
                // Keep that constructor compatible while protocol-aware constructors
                // remain strict for Anthropic `/messages`.
                (self.protocol == ProviderProtocol::ChatCompletions)
                    .then(|| path.strip_suffix("/responses"))
                    .flatten()
            })
            .ok_or(AiError::UnsupportedDiscoveryEndpoint)?;
        let mut models = self.url.clone();
        models.set_path(&format!("{prefix}/models"));
        if models.origin() != self.url.origin() {
            return Err(AiError::InvalidEndpoint);
        }
        Ok(models.into())
    }
}

impl fmt::Debug for ProviderEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderEndpoint").finish_non_exhaustive()
    }
}

/// Cloneable cancellation signal. Cancelling drops the in-flight HTTP future;
/// it cannot undo processing already accepted by the provider.
#[derive(Clone)]
pub struct RequestCancellation {
    state: watch::Sender<bool>,
}

impl Default for RequestCancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestCancellation {
    /// Create a fresh, not-yet-cancelled operation signal.
    pub fn new() -> Self {
        let (state, _) = watch::channel(false);
        Self { state }
    }

    /// Idempotently cancel all operations observing this signal.
    pub fn cancel(&self) {
        self.state.send_replace(true);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        *self.state.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.state.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

/// Sorted, deduplicated model identifiers actually returned by the provider.
/// An empty catalog does not imply that manual model entry is unsupported.
#[derive(Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    models: Vec<String>,
    truncated: bool,
}

impl ModelCatalog {
    /// Provider identifiers, suitable for selection without transforming names.
    pub fn models(&self) -> &[String] {
        &self.models
    }

    /// Whether the provider explicitly indicated more pages than this bounded
    /// catalog could return. Current transports reject this case rather than
    /// exposing a silently incomplete list; the flag remains for future APIs.
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

impl fmt::Debug for ModelCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelCatalog")
            .field("count", &self.models.len())
            .field("truncated", &self.truncated)
            .finish()
    }
}

/// A completed fixed connection test, not evidence about terminal commands or
/// general model capability. The arbitrary provider answer is not retained.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectivityReport {
    elapsed: Duration,
    actual_model: Option<String>,
}

impl ConnectivityReport {
    /// Total HTTP round-trip and response validation time.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Model identifier reported by the server. `None` means the server omitted
    /// it; callers must not replace this with the requested model as evidence.
    pub fn actual_model(&self) -> Option<&str> {
        self.actual_model.as_deref()
    }
}

impl fmt::Debug for ConnectivityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectivityReport")
            .field("elapsed", &self.elapsed)
            .field("model_reported", &self.actual_model.is_some())
            .finish()
    }
}

/// Cancellable asynchronous Chat Completions transport for settings and chat.
///
/// Creating this client makes no request. Every operation must be explicitly
/// requested by the user. Redirects, retries, environment proxies and cookies
/// are disabled. Authentication is borrowed per call and never persisted.
pub struct ProviderClient {
    client: Client,
    timeout: Duration,
    max_response_bytes: usize,
}

impl ProviderClient {
    /// Configure one deadline covering headers and the complete response body.
    pub fn new(timeout: Duration, max_response_bytes: usize) -> Result<Self, AiError> {
        if timeout.is_zero()
            || timeout > Duration::from_secs(300)
            || max_response_bytes == 0
            || max_response_bytes > 8 * 1024 * 1024
        {
            return Err(AiError::InvalidLimits);
        }
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(15)))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .user_agent("KeelShell/0.1")
            .build()
            .map_err(|_| AiError::ClientInitialization)?;
        Ok(Self {
            client,
            timeout,
            max_response_bytes,
        })
    }

    /// Explicitly GET the same-origin model catalog. No chat context is accepted
    /// or sent. At most 4096 valid model IDs may appear in the bounded response.
    pub async fn discover_models(
        &self,
        endpoint: &ProviderEndpoint,
        api_key: Option<&str>,
        cancellation: &RequestCancellation,
    ) -> Result<ModelCatalog, AiError> {
        let url = endpoint.models_endpoint()?;
        self.bounded(cancellation, async {
            let mut models = Vec::new();
            let mut after_id: Option<String> = None;
            let mut pages = 0usize;
            loop {
                pages += 1;
                if pages > 64 {
                    return Err(AiError::InvalidModelCatalog);
                }
                let page_url = if endpoint.protocol == ProviderProtocol::AnthropicMessages {
                    let mut parsed = Url::parse(&url).map_err(|_| AiError::InvalidModelCatalog)?;
                    parsed.query_pairs_mut().append_pair("limit", "1000");
                    if let Some(cursor) = after_id.as_deref() {
                        parsed.query_pairs_mut().append_pair("after_id", cursor);
                    }
                    parsed.into()
                } else {
                    url.clone()
                };
                let body = self
                    .request(Method::GET, &page_url, None, api_key, endpoint.protocol)
                    .await?;
                let raw: RawCatalog =
                    serde_json::from_slice(&body).map_err(|_| AiError::InvalidModelCatalog)?;
                if raw.data.iter().any(|model| {
                    !valid_model(&model.id) || api_key.is_some_and(|key| model.id.contains(key))
                }) {
                    return Err(AiError::InvalidModelCatalog);
                }
                if models.len().saturating_add(raw.data.len()) > 4096 {
                    return Err(AiError::InvalidModelCatalog);
                }
                let has_more = raw.has_more;
                let last_id = raw.last_id;
                models.extend(raw.data.into_iter().map(|model| model.id));
                if endpoint.protocol != ProviderProtocol::AnthropicMessages || !has_more {
                    break;
                }
                let Some(next) = last_id.filter(|id| !id.is_empty()) else {
                    return Err(AiError::InvalidModelCatalog);
                };
                if after_id.as_deref() == Some(next.as_str()) {
                    return Err(AiError::InvalidModelCatalog);
                }
                after_id = Some(next);
            }
            models.sort();
            models.dedup();
            Ok(ModelCatalog {
                models,
                truncated: false,
            })
        })
        .await
    }

    /// Send only [`CONNECTIVITY_PROMPT`] and the selected model after an explicit
    /// user click. No host, question, selection, file or tool call can be supplied.
    /// This can incur provider usage. There is no automatic invocation or retry.
    pub async fn test_connection(
        &self,
        provider: &ProviderConfig,
        api_key: Option<&str>,
        cancellation: &RequestCancellation,
    ) -> Result<ConnectivityReport, AiError> {
        let body = match provider.protocol() {
            ProviderProtocol::ChatCompletions => serde_json::json!({
                "model": provider.model(),
                "stream": false,
                "messages": [{"role": "user", "content": CONNECTIVITY_PROMPT}],
            })
            .to_string(),
            ProviderProtocol::Responses => serde_json::json!({
                "model": provider.model(),
                "stream": false,
                "input": CONNECTIVITY_PROMPT,
            })
            .to_string(),
            ProviderProtocol::AnthropicMessages => serde_json::json!({
                "model": provider.model(),
                "system": "Reply with the single word OK. This is a connection test.",
                "messages": [{"role": "user", "content": CONNECTIVITY_PROMPT}],
                "max_tokens": 4096,
                "stream": false,
            })
            .to_string(),
        };
        reject_context_credential(&body, api_key)?;
        let started = Instant::now();
        self.bounded(cancellation, async {
            let body = self
                .request(
                    Method::POST,
                    provider.endpoint(),
                    Some(body),
                    api_key,
                    provider.protocol(),
                )
                .await?;
            let response = parse_assistant_response(&body, provider.protocol(), api_key)?;
            Ok(ConnectivityReport {
                elapsed: started.elapsed(),
                actual_model: response.model,
            })
        })
        .await
    }

    /// Consume one approved snapshot, sending its exact immutable body and
    /// destination. Changing a profile cannot mutate an already prepared request.
    pub async fn send_approved(
        &self,
        request: ApprovedRequest,
        api_key: Option<&str>,
        cancellation: &RequestCancellation,
    ) -> Result<AssistantReply, AiError> {
        let request = request.0;
        let protocol = request.provider.protocol();
        let endpoint = request.provider.endpoint().to_owned();
        let json = request.json;
        reject_context_credential(&json, api_key)?;
        self.bounded(cancellation, async {
            let body = self
                .request(Method::POST, &endpoint, Some(json), api_key, protocol)
                .await?;
            let response = parse_assistant_response(&body, protocol, api_key)?;
            let (text, _) =
                Redactor::new(&api_key.into_iter().collect::<Vec<_>>()).redact(&response.text);
            Ok(AssistantReply { text })
        })
        .await
    }

    async fn bounded<T>(
        &self,
        cancellation: &RequestCancellation,
        operation: impl Future<Output = Result<T, AiError>>,
    ) -> Result<T, AiError> {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AiError::Cancelled),
            result = tokio::time::timeout(self.timeout, operation) => result.map_err(|_| AiError::Timeout)?,
        }
    }

    async fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<String>,
        api_key: Option<&str>,
        protocol: ProviderProtocol,
    ) -> Result<Zeroizing<Vec<u8>>, AiError> {
        let mut builder = self.client.request(method, url);
        if let Some(key) = api_key {
            if key.is_empty() || key.chars().any(char::is_control) {
                return Err(AiError::InvalidApiKey);
            }
            let value = Zeroizing::new(match protocol {
                ProviderProtocol::AnthropicMessages => key.to_owned(),
                ProviderProtocol::ChatCompletions | ProviderProtocol::Responses => {
                    format!("Bearer {key}")
                }
            });
            let mut header = HeaderValue::from_str(&value).map_err(|_| AiError::InvalidApiKey)?;
            header.set_sensitive(true);
            builder = match protocol {
                ProviderProtocol::AnthropicMessages => builder.header("x-api-key", header),
                ProviderProtocol::ChatCompletions | ProviderProtocol::Responses => {
                    builder.header(AUTHORIZATION, header)
                }
            };
        }
        if protocol == ProviderProtocol::AnthropicMessages {
            builder = builder.header("anthropic-version", "2023-06-01");
        }
        if let Some(body) = body {
            builder = builder.header(CONTENT_TYPE, "application/json").body(body);
        }
        let mut response = builder.send().await.map_err(transport_error)?;
        if !response.status().is_success() {
            return Err(AiError::HttpStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(AiError::ResponseTooLarge);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if chunk.len() > self.max_response_bytes.saturating_sub(bytes.len()) {
                return Err(AiError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn transport_error(error: reqwest::Error) -> AiError {
    if error.is_timeout() {
        AiError::Timeout
    } else {
        AiError::Transport
    }
}

fn reject_context_credential(body: &str, api_key: Option<&str>) -> Result<(), AiError> {
    if api_key.is_some_and(|key| !key.is_empty() && payload_contains_secret(body, key)) {
        return Err(AiError::CredentialInContext);
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawCatalog {
    data: Vec<RawModel>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    last_id: Option<String>,
}
#[derive(Deserialize)]
struct RawModel {
    id: String,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_url_preserves_origin_prefix_port_and_encoded_path() -> Result<(), AiError> {
        for (chat, models) in [
            (
                "https://api.example/v1/chat/completions",
                "https://api.example/v1/models",
            ),
            (
                "https://api.example:9443/tenant%20id/v1/chat/completions",
                "https://api.example:9443/tenant%20id/v1/models",
            ),
            (
                "http://[::1]:9000/chat/completions",
                "http://[::1]:9000/models",
            ),
            (
                "https://api.example/v1beta/openai/chat/completions",
                "https://api.example/v1beta/openai/models",
            ),
            (
                "https://api.example/v1/messages",
                "https://api.example/v1/models",
            ),
            (
                "https://api.example/v1/responses",
                "https://api.example/v1/models",
            ),
        ] {
            assert_eq!(ProviderEndpoint::new(chat)?.models_endpoint()?, models);
        }
        Ok(())
    }

    #[test]
    fn unknown_or_encoded_completion_suffix_is_not_guessed() -> Result<(), AiError> {
        for path in [
            "/api",
            "/chat/completions/",
            "/chat/%63ompletions",
            "/chat/completions/other",
            "/v1/models",
            "/responses/other",
        ] {
            let endpoint = ProviderEndpoint::new(&format!("https://example.test{path}"))?;
            assert_eq!(
                endpoint.models_endpoint(),
                Err(AiError::UnsupportedDiscoveryEndpoint)
            );
        }
        Ok(())
    }

    #[test]
    fn endpoint_rejects_input_normalization_that_would_hide_controls() {
        for endpoint in [
            " https://example.test/chat/completions",
            "https://example.test/chat/\ncompletions",
            "https://example.test/a b/chat/completions",
        ] {
            assert_eq!(
                ProviderEndpoint::new(endpoint),
                Err(AiError::InvalidEndpoint)
            );
        }
    }

    #[test]
    fn profile_and_transport_limits_agree() -> Result<(), AiError> {
        assert_eq!(
            ProviderEndpoint::new("https://example.test:0/chat/completions"),
            Err(AiError::InvalidEndpoint)
        );
        assert_eq!(
            ProviderEndpoint::new(&format!(
                "https://example.test/{}/chat/completions",
                "x".repeat(2048)
            )),
            Err(AiError::InvalidEndpoint)
        );
        let endpoint = "https://example.test/v1/chat/completions";
        ProviderConfig::new(endpoint, &"x".repeat(200))?;
        assert_eq!(
            ProviderConfig::new(endpoint, &"x".repeat(201)),
            Err(AiError::InvalidModel)
        );
        Ok(())
    }
}
