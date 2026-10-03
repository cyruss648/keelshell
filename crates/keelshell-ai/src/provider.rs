use std::{fmt, io::Read, time::Duration};

use reqwest::{
    blocking::Client,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use serde::Deserialize;
use url::Url;
use zeroize::Zeroizing;

use crate::{AiError, ApprovedRequest, Redactor, discovery::ProviderEndpoint};

/// A validated complete chat-completions endpoint and model, without credentials.
///
/// Supply the full URL, for example `https://provider.example/v1/chat/completions`.
/// Query strings, URL credentials and fragments are rejected. HTTP is limited to
/// loopback IP addresses or `localhost`; all other hosts require verified TLS.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    endpoint: Url,
    model: String,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig").finish_non_exhaustive()
    }
}

impl ProviderConfig {
    /// Validate a full endpoint URL and nonempty model identifier.
    pub fn new(endpoint: &str, model: &str) -> Result<Self, AiError> {
        let endpoint = ProviderEndpoint::new(endpoint)?.url;
        if !valid_model(model) {
            return Err(AiError::InvalidModel);
        }
        Ok(Self {
            endpoint,
            model: model.to_owned(),
        })
    }

    /// Full destination to display alongside the payload preview.
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    /// Provider-specific model identifier.
    pub fn model(&self) -> &str {
        &self.model
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
}

impl AiClient {
    /// Create a client with an overall deadline and a bounded response body.
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
            max_response_bytes,
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
        let mut builder = self
            .client
            .post(request.provider.endpoint.clone())
            .header(CONTENT_TYPE, "application/json");
        if let Some(key) = api_key {
            if key.is_empty() || key.chars().any(char::is_control) {
                return Err(AiError::InvalidApiKey);
            }
            if payload_contains_secret(&request.json, key) {
                return Err(AiError::CredentialInContext);
            }
            let value = Zeroizing::new(format!("Bearer {key}"));
            let mut header = HeaderValue::from_str(&value).map_err(|_| AiError::InvalidApiKey)?;
            header.set_sensitive(true);
            builder = builder.header(AUTHORIZATION, header);
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
        let response: ChatResponse =
            serde_json::from_slice(&body).map_err(|_| AiError::InvalidResponse)?;
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
        // A provider may echo the authorization key. Never reflect that key into UI.
        let (text, _) = Redactor::new(&api_key.into_iter().collect::<Vec<_>>()).redact(&text);
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
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    content: Option<String>,
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
