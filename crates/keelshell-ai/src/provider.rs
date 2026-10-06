use std::{fmt, io::Read, time::Duration};

use reqwest::{
    blocking::Client,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use serde::Deserialize;
use url::Url;
use zeroize::Zeroizing;

use crate::{AiError, ApprovedRequest, Redactor, RequestOptions, discovery::ProviderEndpoint};

/// Request/response wire formats implemented by the provider transport.
///
/// Each protocol has a dedicated immutable preview and response parser. The
/// transport never infers a format from the URL or model name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderProtocol {
    /// OpenAI-compatible `/chat/completions` messages.
    ChatCompletions,
    /// OpenAI Responses API requests and `output_text` message items.
    Responses,
    /// Anthropic Messages API requests and assistant text blocks.
    AnthropicMessages,
}

/// A validated complete provider endpoint, model and explicit protocol, without credentials.
///
/// Supply the full URL, for example `https://provider.example/v1/chat/completions`.
/// Query strings, URL credentials and fragments are rejected. HTTP is limited to
/// loopback IP addresses or `localhost`; all other hosts require verified TLS.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    endpoint: Url,
    model: String,
    protocol: ProviderProtocol,
    options: RequestOptions,
    inference: crate::InferenceOptions,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig").finish_non_exhaustive()
    }
}

impl ProviderConfig {
    /// Validate a full endpoint URL and nonempty model identifier using chat completions.
    pub fn new(endpoint: &str, model: &str) -> Result<Self, AiError> {
        Self::new_with_protocol(endpoint, model, ProviderProtocol::ChatCompletions)
    }

    /// Validate a full endpoint, model and explicit wire protocol.
    pub fn new_with_protocol(
        endpoint: &str,
        model: &str,
        protocol: ProviderProtocol,
    ) -> Result<Self, AiError> {
        let endpoint = ProviderEndpoint::new_with_protocol(endpoint, protocol)?.url;
        if !valid_model(model) {
            return Err(AiError::InvalidModel);
        }
        Ok(Self {
            endpoint,
            model: model.to_owned(),
            protocol,
            options: RequestOptions::default(),
            inference: crate::InferenceOptions::default(),
        })
    }

    /// Bind validated resolved headers and routing to this immutable target.
    pub fn with_request_options(mut self, options: RequestOptions) -> Self {
        self.options = options;
        self
    }

    /// Bind typed inference settings. Output-relative budget admission is
    /// repeated during preparation, when the effective output limit is known.
    pub fn with_inference_options(
        mut self,
        options: crate::InferenceOptions,
    ) -> Result<Self, AiError> {
        options.validate(self.protocol, Some(1_000_000))?;
        self.inference = options;
        Ok(self)
    }

    /// Inference fields frozen into this target and its exact payload preview.
    pub fn inference_options(&self) -> crate::InferenceOptions {
        self.inference
    }

    /// Resolved options frozen into request preparation and human review.
    pub fn request_options(&self) -> &RequestOptions {
        &self.options
    }

    /// Full destination to display alongside the payload preview.
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    /// Provider-specific model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Explicit wire protocol bound to this immutable request target.
    pub fn protocol(&self) -> ProviderProtocol {
        self.protocol
    }
}

/// Non-streaming chat client. Create, use and drop it outside async/UI threads.
///
/// Credentials are accepted per request only. Redirects, automatic retries and
/// implicit environment proxies are disabled so an approval binds one endpoint.
/// A failed request is never automatically retried.
pub struct AiClient {
    client: Client,
    max_response_bytes: usize,
    options: RequestOptions,
}

impl AiClient {
    /// Create a client with an overall deadline and a bounded response body.
    pub fn new(timeout: Duration, max_response_bytes: usize) -> Result<Self, AiError> {
        Self::new_with_options(timeout, max_response_bytes, RequestOptions::default())
    }

    /// Create a blocking client for one validated routing/header snapshot.
    pub fn new_with_options(
        timeout: Duration,
        max_response_bytes: usize,
        options: RequestOptions,
    ) -> Result<Self, AiError> {
        if timeout.is_zero()
            || timeout > Duration::from_secs(300)
            || max_response_bytes == 0
            || max_response_bytes > 8 * 1024 * 1024
        {
            return Err(AiError::InvalidLimits);
        }
        let mut builder = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(15)))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .user_agent("KeelShell/0.1");
        if let Some(proxy) = options.proxy()? {
            builder = builder.proxy(proxy);
        }
        let client = builder.build().map_err(|_| AiError::ClientInitialization)?;
        Ok(Self {
            client,
            max_response_bytes,
            options,
        })
    }

    /// Consume one approved request and send its exact preview body.
    ///
    /// `api_key` is borrowed for this call, never persisted or included in Debug
    /// output. Pass it to `ContextDraft::prepare`'s secret list as well; a defense
    /// here rejects keys found in the payload instead of changing the approved
    /// preview. Callers remain responsible for clearing their own key storage.
    pub fn send(
        &self,
        request: ApprovedRequest,
        api_key: Option<&str>,
    ) -> Result<AssistantReply, AiError> {
        let request = request.0;
        if request.provider.options != self.options {
            return Err(AiError::RequestOptionsMismatch);
        }
        self.options
            .reject_header_name_secrets(api_key.as_slice())?;
        let secrets = self.options.secrets();
        if secrets.iter().copied().chain(api_key).any(|secret| {
            crate::request_options::contains_context_secret(request.provider.endpoint(), secret)
                || self
                    .options
                    .proxy_url()
                    .is_some_and(|url| crate::request_options::contains_context_secret(url, secret))
        }) {
            return Err(AiError::CredentialInContext);
        }
        if secrets
            .iter()
            .any(|secret| payload_contains_secret(&request.json, secret))
        {
            return Err(AiError::CredentialInContext);
        }
        let protocol = request.provider.protocol;
        let mut builder = self
            .client
            .post(request.provider.endpoint.clone())
            .header(CONTENT_TYPE, "application/json")
            .headers(self.options.header_map()?);
        if let Some(key) = api_key {
            if key.is_empty() || key.chars().any(char::is_control) {
                return Err(AiError::InvalidApiKey);
            }
            if payload_contains_secret(&request.json, key) {
                return Err(AiError::CredentialInContext);
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
        let response = builder.body(request.json).send().map_err(|error| {
            if error.is_timeout() {
                AiError::Timeout
            } else {
                AiError::Transport
            }
        })?;
        if !response.status().is_success() {
            // Error bodies often echo credentials or request context. Discard them.
            return Err(AiError::HttpStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(AiError::ResponseTooLarge);
        }
        let mut body = Zeroizing::new(Vec::new());
        response
            .take(self.max_response_bytes as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::TimedOut
                    || error
                        .get_ref()
                        .and_then(|e| e.downcast_ref::<reqwest::Error>())
                        .is_some_and(reqwest::Error::is_timeout)
                {
                    AiError::Timeout
                } else {
                    AiError::Transport
                }
            })?;
        if body.len() > self.max_response_bytes {
            return Err(AiError::ResponseTooLarge);
        }
        let parsed = parse_assistant_response(&body, protocol, api_key)?;
        // A provider may echo the authorization key. Never reflect that key into UI.
        let (text, _) = Redactor::new(&secrets.into_iter().chain(api_key).collect::<Vec<_>>())
            .redact(&parsed.text);
        Ok(AssistantReply { text })
    }
}

pub(crate) fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 200
        && !model.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub(crate) fn payload_contains_secret(payload: &str, secret: &str) -> bool {
    if payload.contains(secret) {
        return true;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return true;
    };
    fn contains(value: &serde_json::Value, secret: &str, depth: usize) -> bool {
        match value {
            serde_json::Value::String(text) => {
                text.contains(secret)
                    || (depth < 2
                        && serde_json::from_str::<serde_json::Value>(text)
                            .is_ok_and(|nested| contains(&nested, secret, depth + 1)))
            }
            serde_json::Value::Array(values) => {
                values.iter().any(|value| contains(value, secret, depth))
            }
            serde_json::Value::Object(values) => values
                .iter()
                .any(|(key, value)| key.contains(secret) || contains(value, secret, depth)),
            _ => false,
        }
    }
    contains(&value, secret, 0)
}

/// Text returned by the provider. It is advice, never an executable action.
pub struct AssistantReply {
    pub(crate) text: String,
}

impl AssistantReply {
    /// Get the provider's text for a display or editable suggestion field.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Take ownership of the provider's text.
    pub fn into_text(self) -> String {
        self.text
    }
}

impl fmt::Debug for AssistantReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssistantReply")
            .field("bytes", &self.text.len())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
pub(crate) struct ParsedAssistantResponse {
    pub(crate) text: String,
    pub(crate) model: Option<String>,
}

pub(crate) fn parse_assistant_response(
    body: &[u8],
    protocol: ProviderProtocol,
    api_key: Option<&str>,
) -> Result<ParsedAssistantResponse, AiError> {
    match protocol {
        ProviderProtocol::ChatCompletions => parse_chat_response(body, api_key),
        ProviderProtocol::Responses => parse_responses_response(body, api_key),
        ProviderProtocol::AnthropicMessages => parse_anthropic_response(body, api_key),
    }
}

fn parse_chat_response(
    body: &[u8],
    api_key: Option<&str>,
) -> Result<ParsedAssistantResponse, AiError> {
    let response: ChatResponse =
        serde_json::from_slice(body).map_err(|_| AiError::InvalidResponse)?;
    if response
        .model
        .as_ref()
        .is_some_and(|model| !valid_model(model) || api_key.is_some_and(|key| model.contains(key)))
    {
        return Err(AiError::InvalidResponse);
    }
    let message = response
        .choices
        .into_iter()
        .next()
        .ok_or(AiError::EmptyReply)?
        .message;
    let text = message.content.ok_or(AiError::EmptyReply)?;
    if text.trim().is_empty() {
        return Err(AiError::EmptyReply);
    }
    Ok(ParsedAssistantResponse {
        text,
        model: response.model,
    })
}

fn parse_responses_response(
    body: &[u8],
    api_key: Option<&str>,
) -> Result<ParsedAssistantResponse, AiError> {
    let response: ResponsesResponse =
        serde_json::from_slice(body).map_err(|_| AiError::InvalidResponse)?;
    if response
        .model
        .as_ref()
        .is_some_and(|model| !valid_model(model) || api_key.is_some_and(|key| model.contains(key)))
    {
        return Err(AiError::InvalidResponse);
    }
    let text = response
        .output
        .into_iter()
        .filter(|item| item.kind == "message")
        .filter_map(|item| item.content)
        .flatten()
        .filter(|content| content.kind == "output_text")
        .map(|content| content.text)
        .collect::<String>();
    if text.trim().is_empty() {
        return Err(AiError::EmptyReply);
    }
    Ok(ParsedAssistantResponse {
        text,
        model: response.model,
    })
}

fn parse_anthropic_response(
    body: &[u8],
    api_key: Option<&str>,
) -> Result<ParsedAssistantResponse, AiError> {
    let response: AnthropicResponse =
        serde_json::from_slice(body).map_err(|_| AiError::InvalidResponse)?;
    if response.role != "assistant"
        || response.model.as_ref().is_some_and(|model| {
            !valid_model(model) || api_key.is_some_and(|key| model.contains(key))
        })
    {
        return Err(AiError::InvalidResponse);
    }
    let mut text = String::new();
    for block in response.content {
        match block.kind.as_str() {
            "text" => {
                let Some(value) = block.text else {
                    return Err(AiError::InvalidResponse);
                };
                text.push_str(&value);
            }
            // Tool and thinking blocks are intentionally never rendered as text.
            "tool_use" | "thinking" | "redacted_thinking" => {}
            _ => {}
        }
    }
    if text.trim().is_empty() {
        return Err(AiError::EmptyReply);
    }
    Ok(ParsedAssistantResponse {
        text,
        model: response.model,
    })
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    model: Option<String>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ResponsesResponse {
    output: Vec<ResponsesOutput>,
    model: Option<String>,
}
#[derive(Deserialize)]
struct ResponsesOutput {
    #[serde(rename = "type")]
    kind: String,
    content: Option<Vec<ResponsesContent>>,
}
#[derive(Deserialize)]
struct ResponsesContent {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    role: String,
    content: Vec<AnthropicContent>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_http_and_credentials_in_urls_are_rejected() {
        for endpoint in [
            "http://example.test/api",
            "https://user:password@example.test/api",
            "https://example.test/api?key=secret",
            "https://example.test/api#secret",
            "file:///tmp/chat",
            "http://localhost.evil.test/api",
        ] {
            assert_eq!(
                ProviderConfig::new(endpoint, "model"),
                Err(AiError::InvalidEndpoint)
            );
        }
    }

    #[test]
    fn https_and_loopback_http_are_accepted() -> Result<(), AiError> {
        for endpoint in [
            "https://example.test/api",
            "http://127.0.0.1:1234/v1/chat/completions",
            "http://[::1]:1234/chat",
            "http://localhost:1234/chat",
        ] {
            ProviderConfig::new(endpoint, "qwen2.5:7b")?;
        }
        Ok(())
    }

    #[test]
    fn nested_json_cannot_hide_the_transport_key() {
        let secret = "a\"b\\c";
        let inner = serde_json::json!({"selection":secret}).to_string();
        let payload = serde_json::json!({"messages":[{"content":inner}]}).to_string();
        assert!(payload_contains_secret(&payload, secret));
    }
}
