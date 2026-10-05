//! Named API configurations. This module stores metadata and credential
//! references only; it neither resolves secrets nor starts local executables.

use crate::{AiBackend, AiLocalAgent, AiSettings, ValidationError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};
use url::{Host, Url};
use uuid::Uuid;

const MAX_PROFILES: usize = 64;
const MAX_TOKENS: u32 = 16 * 1024 * 1024;

/// Wire formats understood by the configuration schema. A declared format is
/// not necessarily implemented by the currently shipped HTTP transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiApiStyle {
    /// OpenAI-compatible chat/completions request and response bodies.
    ChatCompletions,
    /// OpenAI Responses request/response format.
    Responses,
    /// Anthropic Messages request/response format.
    AnthropicMessages,
}
impl AiApiStyle {
    /// Whether the current KeelShell transport implements this body format.
    /// This does not assert provider availability or model compatibility.
    pub const fn supports_current_transport(self) -> bool {
        matches!(
            self,
            Self::ChatCompletions | Self::Responses | Self::AnthropicMessages
        )
    }
}

/// Provider presets establish editable defaults, never a fixed model catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPreset {
    /// Anthropic's native Messages endpoint.
    Claude,
    /// OpenAI Chat Completions.
    OpenAi,
    /// Gemini's documented OpenAI-compatible endpoint.
    Gemini,
    /// DeepSeek's documented OpenAI-compatible endpoint.
    DeepSeek,
    /// Qwen via DashScope Beijing; region and billing plan must match the key.
    Qwen,
    /// MiniMax's documented OpenAI-compatible endpoint.
    MiniMax,
    /// An already running Ollama HTTP service; no process is started.
    Ollama,
    /// A manually configured chat/completions endpoint.
    OpenAiCompatible,
    /// A manually configured Anthropic Messages endpoint.
    AnthropicCompatible,
    /// A user-selected endpoint and declared API format.
    Custom,
}
impl AiPreset {
    /// Preset identities for settings. Disable unsupported formats in request UI.
    pub const ALL: [Self; 10] = [
        Self::Claude,
        Self::OpenAi,
        Self::Gemini,
        Self::DeepSeek,
        Self::Qwen,
        Self::MiniMax,
        Self::Ollama,
        Self::OpenAiCompatible,
        Self::AnthropicCompatible,
        Self::Custom,
    ];

    /// Default wire format, without implying that a provider account is usable.
    pub const fn api_style(self) -> AiApiStyle {
        match self {
            Self::Claude | Self::AnthropicCompatible => AiApiStyle::AnthropicMessages,
            _ => AiApiStyle::ChatCompletions,
        }
    }

    /// Full request endpoint. Compatible/custom presets require explicit input.
    /// Defaults were checked against official provider documentation on 2026-10-03:
    /// [OpenAI](https://platform.openai.com/docs/api-reference/chat),
    /// [Anthropic](https://platform.claude.com/docs/en/api/messages/create),
    /// [Gemini](https://ai.google.dev/gemini-api/docs/openai),
    /// [DeepSeek](https://api-docs.deepseek.com/),
    /// [Qwen](https://www.alibabacloud.com/help/en/model-studio/base-url),
    /// [MiniMax](https://platform.minimax.io/docs/api-reference/text-openai-api), and
    /// [Ollama](https://docs.ollama.com/api/openai-compatibility).
    /// Qwen uses the documented Beijing compatible endpoint; keys for other
    /// regions or billing plans require the user to edit this default.
    pub const fn endpoint(self) -> &'static str {
        match self {
            Self::Claude => "https://api.anthropic.com/v1/messages",
            Self::OpenAi => "https://api.openai.com/v1/chat/completions",
            Self::Gemini => {
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            }
            Self::DeepSeek => "https://api.deepseek.com/chat/completions",
            Self::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            Self::MiniMax => "https://api.minimax.io/v1/chat/completions",
            Self::Ollama => "http://localhost:11434/v1/chat/completions",
            Self::OpenAiCompatible | Self::AnthropicCompatible | Self::Custom => "",
        }
    }
}

/// A reference to a secret, never the secret value. An environment reference
/// names a variable; a store reference is an opaque item ID resolved separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiSecretRef {
    /// Read an explicitly selected environment variable at request time.
    Environment {
        /// Variable identifier, restricted to ASCII letters, digits and `_`.
        name: String,
    },
    /// Resolve a value held only in this process. Persisting the ID never persists its value.
    Ephemeral {
        /// Non-nil identity of this temporary secret slot.
        id: Uuid,
    },
    /// Resolve an opaque item in the authenticated local credential vault.
    SecretStore {
        /// Non-nil identifier. This module does not implement the secret store.
        id: Uuid,
    },
}
impl AiSecretRef {
    /// Validate the reference syntax without resolving or revealing a value.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Environment { name } => {
                if name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
                    || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                {
                    return Err(invalid(
                        "ai.secret_ref",
                        "must name an environment variable, not contain its value",
                    ));
                }
            }
            Self::SecretStore { id } | Self::Ephemeral { id } if id.is_nil() => {
                return Err(invalid(
                    "ai.secret_ref",
                    "credential store ID cannot be nil",
                ));
            }
            Self::SecretStore { .. } | Self::Ephemeral { .. } => {}
        }
        Ok(())
    }
}

/// Authentication metadata. A missing reference means a secret must be supplied
/// transiently when needed; it never authorizes silently omitting authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiAuthentication {
    /// The provider is explicitly configured to receive no authentication.
    #[default]
    None,
    /// HTTP Authorization with the Bearer scheme.
    Bearer {
        /// Optional persistent reference; never an inline API key.
        credential: Option<AiSecretRef>,
    },
    /// A dedicated key header, for example Anthropic's `x-api-key`.
    Header {
        /// Header name; routing and framing headers cannot be overridden.
        name: String,
        /// Optional persistent reference; never an inline header value.
        credential: Option<AiSecretRef>,
    },
}

// Internally tagged unit variants would otherwise discard unknown fields.
// Empty struct wire variants reject inline credentials even on `none`/`direct`.
impl<'de> Deserialize<'de> for AiAuthentication {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            None {},
            Bearer {
                credential: Option<AiSecretRef>,
            },
            Header {
                name: String,
                credential: Option<AiSecretRef>,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::None {} => Self::None,
            Wire::Bearer { credential } => Self::Bearer { credential },
            Wire::Header { name, credential } => Self::Header { name, credential },
        })
    }
}

/// Custom header values can be sensitive even when their names look ordinary.
/// Consequently every value is resolved from a reference, including tenant IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiCustomHeader {
    /// RFC HTTP token header name, unique ignoring ASCII case.
    pub name: String,
    /// A value reference; plaintext values are intentionally unrepresentable.
    pub value_ref: AiSecretRef,
}

/// Explicit per-profile routing. The application must not silently use ambient
/// proxy environment variables instead of this setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiProxy {
    /// Direct connection; ignore ambient proxy variables.
    #[default]
    Direct,
    /// An explicitly configured HTTP(S) or SOCKS5 proxy.
    Explicit {
        /// Proxy origin without userinfo, query, fragment or non-root path.
        url: String,
        /// Optional reference to proxy credentials, resolved by the transport.
        credentials: Option<AiSecretRef>,
    },
}

impl<'de> Deserialize<'de> for AiProxy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Direct {},
            Explicit {
                url: String,
                credentials: Option<AiSecretRef>,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Direct {} => Self::Direct,
            Wire::Explicit { url, credentials } => Self::Explicit { url, credentials },
        })
    }
}

/// Model-specific reasoning controls. These are discovered or explicitly
/// configured capabilities, not inferred from a model name or preset alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiReasoningCapability {
    /// The provider has not supplied a usable capability description.
    #[default]
    Unknown,
    /// The provider explicitly reports no configurable reasoning.
    Unsupported,
    /// A provider-specific set of named effort values.
    Effort {
        /// Supported values, without inventing one universal cross-vendor scale.
        values: Vec<String>,
    },
    /// A Boolean thinking switch.
    ThinkingToggle,
    /// A provider-specific integer reasoning-token budget.
    TokenBudget {
        /// Smallest accepted budget, inclusive.
        min: u32,
        /// Largest accepted budget, inclusive.
        max: u32,
    },
    /// A bounded provider-specific textual reasoning setting.
    Text {
        /// Maximum number of UTF-8 characters accepted by the provider.
        max_chars: u16,
    },
}

impl<'de> Deserialize<'de> for AiReasoningCapability {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Unknown {},
            Unsupported {},
            Effort { values: Vec<String> },
            ThinkingToggle {},
            TokenBudget { min: u32, max: u32 },
            Text { max_chars: u16 },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Unknown {} => Self::Unknown,
            Wire::Unsupported {} => Self::Unsupported,
            Wire::Effort { values } => Self::Effort { values },
            Wire::ThinkingToggle {} => Self::ThinkingToggle,
            Wire::TokenBudget { min, max } => Self::TokenBudget { min, max },
            Wire::Text { max_chars } => Self::Text { max_chars },
        })
    }
}

/// A selection interpreted only with the matching model's advertised capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AiReasoningSelection {
    /// Omit explicit reasoning parameters.
    #[default]
    ProviderDefault,
    /// One exact provider-advertised effort value.
    Effort(String),
    /// Enable or disable the provider's thinking switch.
    Thinking(bool),
    /// Reasoning token budget, interpreted by the eventual API adapter.
    Budget(u32),
    /// Provider-specific reasoning text; never a secret or arbitrary JSON body.
    Text(String),
}

/// Capability plus remembered selection for one model ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AiModelReasoning {
    /// The matching model's capability, unknown until explicitly established.
    pub capability: AiReasoningCapability,
    /// A setting validated against that capability.
    pub selection: AiReasoningSelection,
}

/// Persistent metadata for one independently named API configuration.
/// Debug intentionally omits endpoint, model, names and custom routing metadata.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedAiProfile {
    /// Stable non-nil configuration identity.
    pub id: Uuid,
    /// User-visible name, unique within a catalog ignoring letter case.
    pub name: String,
    /// Explicit execution choice. Missing metadata in older profiles stays API.
    #[serde(default)]
    pub backend: AiBackend,
    /// Editable preset provenance; it is not a verified provider identity.
    pub preset: AiPreset,
    /// Explicit request/response protocol.
    pub api_style: AiApiStyle,
    /// Complete HTTP request endpoint or a local CLI's inference base URL.
    /// Requires HTTPS except for explicitly selected loopback HTTP services.
    pub endpoint: String,
    /// Explicit model ID; never filled with an assumed latest model.
    pub model: String,
    /// Authentication scheme and optional credential reference.
    pub authentication: AiAuthentication,
    /// Additional referenced headers shared by discovery/test/request adapters.
    #[serde(default)]
    pub custom_headers: Vec<AiCustomHeader>,
    /// Explicit network route, with direct connection as the default.
    #[serde(default)]
    pub proxy: AiProxy,
    /// Declared model context budget, not a tokenizer-derived usage measurement.
    pub context_window_tokens: Option<u32>,
    /// Requested output limit; adapters must implement it or reject the request.
    pub max_output_tokens: Option<u32>,
    /// Model-keyed controls; changing models does not reuse another model's value.
    #[serde(default)]
    pub reasoning_by_model: BTreeMap<String, AiModelReasoning>,
}
impl fmt::Debug for NamedAiProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedAiProfile")
            .field("id", &self.id)
            .field("backend", &self.backend)
            .field("preset", &self.preset)
            .field("api_style", &self.api_style)
            .finish_non_exhaustive()
    }
}

impl NamedAiProfile {
    /// Make an unsaved editor draft. Name/model and custom endpoints remain empty
    /// until the user fills them; a draft is deliberately not a valid profile.
    pub fn draft(preset: AiPreset) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            backend: AiBackend::Api,
            preset,
            api_style: preset.api_style(),
            endpoint: preset.endpoint().into(),
            model: String::new(),
            authentication: match preset {
                AiPreset::Ollama => AiAuthentication::None,
                AiPreset::Claude | AiPreset::AnthropicCompatible => AiAuthentication::Header {
                    name: "x-api-key".into(),
                    credential: None,
                },
                _ => AiAuthentication::Bearer { credential: None },
            },
            custom_headers: Vec::new(),
            proxy: AiProxy::Direct,
            context_window_tokens: None,
            max_output_tokens: None,
            reasoning_by_model: BTreeMap::new(),
        }
    }

    /// Validate storage metadata without resolving credentials or making requests.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(invalid("ai.profile.id", "cannot be nil"));
        }
        bounded("ai.profile.name", &self.name, 120, true)?;
        self.backend.validate()?;
        model_id(&self.model)?;
        endpoint(&self.endpoint, self.backend != AiBackend::Api)?;
        let mut names = HashSet::new();
        match &self.authentication {
            AiAuthentication::None => {}
            AiAuthentication::Bearer { credential } => {
                names.insert("authorization".to_owned());
                if let Some(reference) = credential {
                    reference.validate()?;
                }
            }
            AiAuthentication::Header { name, credential } => {
                header_name(name)?;
                names.insert(name.to_ascii_lowercase());
                if let Some(reference) = credential {
                    reference.validate()?;
                }
            }
        }
        if self.custom_headers.len() > 32 {
            return Err(invalid(
                "ai.profile.headers",
                "at most 32 custom headers are allowed",
            ));
        }
        for header in &self.custom_headers {
            header_name(&header.name)?;
            if matches!(
                header.name.to_ascii_lowercase().as_str(),
                "x-api-key" | "anthropic-version"
            ) {
                return Err(invalid(
                    "ai.profile.headers",
                    "protocol-managed headers cannot be overridden",
                ));
            }
            if !names.insert(header.name.to_ascii_lowercase()) {
                return Err(invalid(
                    "ai.profile.headers",
                    "header names must be unique including authentication headers",
                ));
            }
            header.value_ref.validate()?;
        }
        if let AiProxy::Explicit { url, credentials } = &self.proxy {
            let parsed = clean_url("ai.profile.proxy", url)?;
            if !matches!(parsed.scheme(), "http" | "https" | "socks5" | "socks5h")
                || !matches!(parsed.path(), "" | "/")
            {
                return Err(invalid(
                    "ai.profile.proxy",
                    "must be an HTTP(S) or SOCKS5 origin without a path",
                ));
            }
            if let Some(reference) = credentials {
                reference.validate()?;
            }
        }
        for (field, value) in [
            ("ai.profile.context_window", self.context_window_tokens),
            ("ai.profile.max_output", self.max_output_tokens),
        ] {
            if value.is_some_and(|value| value == 0 || value > MAX_TOKENS) {
                return Err(invalid(field, "must be between 1 and 16777216 tokens"));
            }
        }
        if matches!((self.context_window_tokens, self.max_output_tokens), (Some(context), Some(output)) if output >= context)
        {
            return Err(invalid(
                "ai.profile.max_output",
                "must leave context capacity for input tokens",
            ));
        }
        if self.reasoning_by_model.len() > 64 {
            return Err(invalid(
                "ai.profile.reasoning",
                "at most 64 model settings are allowed",
            ));
        }
        for (model, reasoning) in &self.reasoning_by_model {
            model_id(model)?;
            reasoning.validate()?;
            if model == &self.model
                && let AiReasoningSelection::Budget(tokens) = reasoning.selection
                && self.max_output_tokens.is_some_and(|output| tokens > output)
            {
                return Err(invalid(
                    "ai.profile.reasoning",
                    "reasoning budget cannot exceed the output budget",
                ));
            }
        }
        Ok(())
    }

    /// Validate options understood by the current HTTP adapter without network
    /// access. Persistable metadata can describe future adapters, but discovery,
    /// connection tests and completion requests must call this gate before use.
    /// Authentication is checked against the selected protocol, but no secret
    /// is resolved here. The transport caller still supplies a transient value
    /// for a configured credential reference.
    pub fn validate_current_transport(&self) -> Result<(), ValidationError> {
        self.validate()?;
        if self.backend != AiBackend::Api {
            return Err(invalid(
                "ai.profile.backend",
                "this profile requires the local CLI adapter",
            ));
        }
        if !self.api_style.supports_current_transport() {
            return Err(invalid(
                "ai.profile.api_style",
                "this API style has no implemented transport",
            ));
        }
        let authentication_supported = match self.api_style {
            AiApiStyle::ChatCompletions | AiApiStyle::Responses => matches!(
                &self.authentication,
                AiAuthentication::None | AiAuthentication::Bearer { .. }
            ),
            // Anthropic's native API uses x-api-key. Keep the accepted header
            // exact so a later adapter cannot accidentally send a bearer token
            // or arbitrary user-selected header as provider authentication.
            AiApiStyle::AnthropicMessages => match &self.authentication {
                AiAuthentication::None => true,
                AiAuthentication::Header { name, .. } if name.eq_ignore_ascii_case("x-api-key") => {
                    true
                }
                _ => false,
            },
        };
        if self
            .reasoning_by_model
            .get(&self.model)
            .is_some_and(|setting| setting.selection != AiReasoningSelection::ProviderDefault)
            || !authentication_supported
        {
            return Err(invalid(
                "ai.profile",
                "current transport does not implement these options or credential references",
            ));
        }
        if self
            .max_output_tokens
            .is_some_and(|value| value > 1_000_000)
        {
            return Err(invalid(
                "ai.profile.max_output",
                "current transport supports at most 1000000 tokens",
            ));
        }
        if self
            .context_window_tokens
            .is_some_and(|context| self.max_output_tokens.unwrap_or(4096) >= context)
        {
            return Err(invalid(
                "ai.profile.context_window",
                "must leave input capacity after the output reserve (4096 by default)",
            ));
        }
        Ok(())
    }

    /// Validate supported local Ask metadata without probing or starting a CLI.
    /// The application must separately detect capabilities and resolve a key.
    /// Token controls and advanced routing are rejected rather than silently lost.
    pub fn validate_local_agent_transport(&self) -> Result<(), ValidationError> {
        self.validate()?;
        let AiBackend::LocalAgent { agent, .. } = self.backend else {
            return Err(invalid(
                "ai.profile.backend",
                "this profile requires the HTTP adapter",
            ));
        };
        let authentication_supported = match agent {
            AiLocalAgent::Codex => {
                self.api_style == AiApiStyle::Responses
                    && matches!(
                        self.authentication,
                        AiAuthentication::Bearer {
                            credential: None | Some(AiSecretRef::SecretStore { .. })
                        }
                    )
            }
            AiLocalAgent::ClaudeCode => {
                self.api_style == AiApiStyle::AnthropicMessages
                    && matches!(
                        &self.authentication,
                        AiAuthentication::Header { name, credential: None | Some(AiSecretRef::SecretStore { .. }) }
                            if name.eq_ignore_ascii_case("x-api-key")
                    )
            }
        };
        if !authentication_supported
            || !self.custom_headers.is_empty()
            || self.proxy != AiProxy::Direct
            || self.max_output_tokens.is_some()
            || self.context_window_tokens.is_some()
            || self
                .reasoning_by_model
                .get(&self.model)
                .is_some_and(|setting| setting.selection != AiReasoningSelection::ProviderDefault)
        {
            return Err(invalid(
                "ai.profile",
                "local Ask requires its fixed protocol and explicit key; advanced API options are unsupported",
            ));
        }
        Ok(())
    }

    /// Produce legacy chat preferences only when no protocol or advanced option
    /// would be silently lost. The legacy shape has no protocol field, so only
    /// Chat Completions can be projected into it.
    /// Never persist this projection as a replacement for the named catalog.
    pub fn legacy_projection(&self, enabled: bool) -> Result<AiSettings, ValidationError> {
        if self.api_style != AiApiStyle::ChatCompletions {
            return Err(invalid(
                "ai.profile.api_style",
                "legacy settings support Chat Completions only",
            ));
        }
        self.validate_current_transport()?;
        if matches!(
            &self.authentication,
            AiAuthentication::Bearer {
                credential: Some(AiSecretRef::Environment { .. } | AiSecretRef::Ephemeral { .. })
            } | AiAuthentication::Header {
                credential: Some(AiSecretRef::Environment { .. } | AiSecretRef::Ephemeral { .. }),
                ..
            }
        ) {
            return Err(invalid(
                "ai.profile",
                "legacy settings cannot preserve this credential reference",
            ));
        }
        if self.context_window_tokens.is_some()
            || self.max_output_tokens.is_some()
            || !self.custom_headers.is_empty()
            || self.proxy != AiProxy::Direct
        {
            return Err(invalid(
                "ai.profile",
                "legacy settings cannot preserve token limits",
            ));
        }
        Ok(AiSettings {
            enabled,
            base_url: self.endpoint.clone(),
            model: self.model.clone(),
        })
    }
}

impl AiModelReasoning {
    /// Reject controls or values not present in this model's capability metadata.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match &self.capability {
            AiReasoningCapability::Effort { values } => {
                if values.is_empty() || values.len() > 32 {
                    return Err(invalid(
                        "ai.reasoning.capability",
                        "effort choices must contain 1–32 values",
                    ));
                }
                let mut unique = HashSet::new();
                for value in values {
                    bounded("ai.reasoning.capability", value, 64, false)?;
                    if !unique.insert(value) {
                        return Err(invalid(
                            "ai.reasoning.capability",
                            "effort choices must be unique",
                        ));
                    }
                }
            }
            AiReasoningCapability::TokenBudget { min, max }
                if *min == 0 || min > max || *max > MAX_TOKENS =>
            {
                return Err(invalid(
                    "ai.reasoning.capability",
                    "invalid token budget range",
                ));
            }
            AiReasoningCapability::Text { max_chars } if *max_chars == 0 || *max_chars > 1024 => {
                return Err(invalid(
                    "ai.reasoning.capability",
                    "text limit must be between 1 and 1024 characters",
                ));
            }
            _ => {}
        }
        let valid = match (&self.capability, &self.selection) {
            (_, AiReasoningSelection::ProviderDefault) => true,
            (AiReasoningCapability::Effort { values }, AiReasoningSelection::Effort(value)) => {
                values.contains(value)
            }
            (AiReasoningCapability::ThinkingToggle, AiReasoningSelection::Thinking(_)) => true,
            (
                AiReasoningCapability::TokenBudget { min, max },
                AiReasoningSelection::Budget(value),
            ) => (*min..=*max).contains(value),
            (AiReasoningCapability::Text { max_chars }, AiReasoningSelection::Text(value)) => {
                !value.trim().is_empty()
                    && value.chars().count() <= usize::from(*max_chars)
                    && !value.chars().any(char::is_control)
            }
            _ => false,
        };
        if !valid {
            return Err(invalid(
                "ai.reasoning.selection",
                "not supported by this model's declared capability",
            ));
        }
        Ok(())
    }
}

/// Independent named configurations and an optional active choice. Empty state
/// is valid and never triggers model discovery or requests during app startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AiProfileCatalog {
    /// Stored metadata; each item owns its own endpoint/model/references.
    #[serde(default)]
    pub profiles: Vec<NamedAiProfile>,
    /// Selected configuration; `None` means AI is not enabled for requests.
    pub active_id: Option<Uuid>,
}
impl AiProfileCatalog {
    /// Validate unique IDs/names and reject dangling active selections.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.profiles.len() > MAX_PROFILES {
            return Err(invalid(
                "ai.profiles",
                "at most 64 configurations are allowed",
            ));
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            if !ids.insert(profile.id) || !names.insert(profile.name.to_lowercase()) {
                return Err(invalid(
                    "ai.profiles",
                    "configuration IDs and names must be unique",
                ));
            }
        }
        if self.active_id.is_some_and(|id| !ids.contains(&id)) {
            return Err(invalid(
                "ai.active_id",
                "must reference an existing configuration",
            ));
        }
        if self
            .active()
            .is_some_and(|profile| !profile.api_style.supports_current_transport())
        {
            return Err(invalid(
                "ai.active_id",
                "an unimplemented API style cannot be active",
            ));
        }
        Ok(())
    }

    /// Return the selected metadata without resolving secrets or sending traffic.
    pub fn active(&self) -> Option<&NamedAiProfile> {
        self.active_id
            .and_then(|id| self.profiles.iter().find(|profile| profile.id == id))
    }

    /// Insert or replace a profile transactionally; invalid changes leave self intact.
    pub fn upsert(&mut self, profile: NamedAiProfile) -> Result<(), ValidationError> {
        let mut next = self.clone();
        if let Some(existing) = next.profiles.iter_mut().find(|item| item.id == profile.id) {
            *existing = profile;
        } else {
            next.profiles.push(profile);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Select a saved profile with an implemented wire format. Runtime adapters
    /// must still validate advanced-option support before making a request.
    pub fn activate(&mut self, id: Uuid) -> Result<(), ValidationError> {
        self.validate()?;
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| invalid("ai.active_id", "configuration was not found"))?;
        if !profile.api_style.supports_current_transport() {
            return Err(invalid(
                "ai.profile.api_style",
                "this API style has no implemented transport",
            ));
        }
        self.active_id = Some(id);
        Ok(())
    }

    /// Remove one profile and clear selection when it was active. Secret-store
    /// garbage collection is a separate explicit operation, outside this module.
    pub fn remove(&mut self, id: Uuid) -> Option<NamedAiProfile> {
        let index = self.profiles.iter().position(|profile| profile.id == id)?;
        if self.active_id == Some(id) {
            self.active_id = None;
        }
        Some(self.profiles.remove(index))
    }

    /// Migrate legacy preferences once. Empty legacy model selection remains
    /// empty. Complete chat endpoints are retained; old API roots get exactly
    /// one `/chat/completions` suffix, matching the previous editor behavior.
    pub fn from_legacy(settings: &AiSettings) -> Result<Self, ValidationError> {
        settings.validate()?;
        if settings.model.is_empty() {
            return Ok(Self::default());
        }
        let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
        profile.name = "默认配置".into();
        profile.model = settings.model.clone();
        let base = settings.base_url.trim_end_matches('/');
        profile.endpoint = if base.ends_with("/chat/completions") {
            base.to_owned()
        } else {
            format!("{base}/chat/completions")
        };
        profile.validate()?;
        let active_id = settings.enabled.then_some(profile.id);
        Ok(Self {
            profiles: vec![profile],
            active_id,
        })
    }
}

fn invalid(field: &'static str, reason: &'static str) -> ValidationError {
    ValidationError::new(field, reason)
}
fn bounded(
    field: &'static str,
    value: &str,
    max: usize,
    whitespace: bool,
) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > max
        || value.trim() != value
        || value.chars().any(char::is_control)
        || (!whitespace && value.chars().any(char::is_whitespace))
    {
        return Err(invalid(
            field,
            "must be nonempty bounded text without controls or surrounding whitespace",
        ));
    }
    Ok(())
}
fn model_id(value: &str) -> Result<(), ValidationError> {
    bounded("ai.profile.model", value, 200, false)
}
fn clean_url(field: &'static str, value: &str) -> Result<Url, ValidationError> {
    bounded(field, value, 2048, false)?;
    let parsed = Url::parse(value).map_err(|_| invalid(field, "must be an absolute URL"))?;
    if value.split_once("://").is_some_and(|(_, rest)| {
        rest.split(['/', '?', '#'])
            .next()
            .is_some_and(|authority| authority.contains('@'))
    }) || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.port() == Some(0)
    {
        return Err(invalid(
            field,
            "URL credentials, query strings and fragments are not allowed",
        ));
    }
    Ok(parsed)
}

fn endpoint(value: &str, allow_origin: bool) -> Result<(), ValidationError> {
    let parsed = clean_url("ai.profile.endpoint", value)?;
    let loopback = match parsed.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
        return Err(invalid(
            "ai.profile.endpoint",
            "requires HTTPS except for an explicit loopback HTTP endpoint",
        ));
    }
    if !allow_origin && matches!(parsed.path(), "" | "/") {
        return Err(invalid(
            "ai.profile.endpoint",
            "must be the full request endpoint, not only an API origin",
        ));
    }
    Ok(())
}
fn header_name(value: &str) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
    {
        return Err(invalid(
            "ai.profile.headers",
            "header name must be an HTTP token without controls",
        ));
    }
    if ["proxy-", "x-forwarded-", "sec-"]
        .iter()
        .any(|prefix| value.to_ascii_lowercase().starts_with(prefix))
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "host"
                | "authorization"
                | "proxy-authorization"
                | "proxy-authenticate"
                | "content-type"
                | "content-length"
                | "connection"
                | "transfer-encoding"
                | "upgrade"
                | "te"
                | "trailer"
                | "cookie"
                | "set-cookie"
                | "user-agent"
                | "accept"
                | "proxy-connection"
                | "keep-alive"
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
    {
        return Err(invalid(
            "ai.profile.headers",
            "transport-managed headers cannot be overridden",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> NamedAiProfile {
        let mut profile = NamedAiProfile::draft(AiPreset::OpenAi);
        profile.name = name.into();
        profile.model = "explicit-model".into();
        profile
    }
    fn environment(name: &str) -> AiSecretRef {
        AiSecretRef::Environment { name: name.into() }
    }

    #[test]
    fn ephemeral_references_are_value_free_and_reserved_headers_rejected_without_authentication()
    -> Result<(), Box<dyn std::error::Error>> {
        let reference = AiSecretRef::Ephemeral { id: Uuid::new_v4() };
        reference.validate()?;
        let wire = serde_json::to_string(&reference)?;
        assert_eq!(serde_json::from_str::<AiSecretRef>(&wire)?, reference);
        assert!(
            serde_json::from_str::<AiSecretRef>(&format!(
                r#"{{"source":"ephemeral","id":"{}","value":"plaintext"}}"#,
                Uuid::new_v4()
            ))
            .is_err()
        );
        assert!(
            AiSecretRef::Ephemeral { id: Uuid::nil() }
                .validate()
                .is_err()
        );
        for name in [
            "Authorization",
            "x-api-key",
            "anthropic-version",
            "HOST",
            "Content-Type",
            "Proxy-Connection",
            "X-Forwarded-Port",
            "Sec-Fetch-Mode",
            "X-Real-IP",
        ] {
            let mut p = profile("Fixture");
            p.authentication = AiAuthentication::None;
            p.custom_headers = vec![AiCustomHeader {
                name: name.into(),
                value_ref: reference.clone(),
            }];
            assert!(p.validate().is_err());
        }
        let mut p = profile("Fixture");
        p.custom_headers = vec![AiCustomHeader {
            name: "x-project".into(),
            value_ref: reference,
        }];
        p.proxy = AiProxy::Explicit {
            url: "socks5h://localhost:1080".into(),
            credentials: None,
        };
        p.validate_current_transport()?;
        assert!(p.legacy_projection(true).is_err());
        for url in [
            "http://@localhost:8888",
            "http://user:password@localhost:8888",
            "http://localhost:8888/path",
        ] {
            p.proxy = AiProxy::Explicit {
                url: url.into(),
                credentials: None,
            };
            assert!(p.validate().is_err());
        }
        Ok(())
    }

    #[test]
    fn legacy_migration_keeps_empty_state_and_does_not_append_twice()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            AiProfileCatalog::from_legacy(&AiSettings::default())?,
            AiProfileCatalog::default()
        );
        for base in [
            "https://example.test/v1",
            "https://example.test/v1/chat/completions",
            "https://example.test/v1/chat/completions/",
        ] {
            let legacy = AiSettings {
                enabled: true,
                base_url: base.into(),
                model: "chosen-model".into(),
            };
            let catalog = AiProfileCatalog::from_legacy(&legacy)?;
            let selected = catalog.active().ok_or("missing active profile")?;
            assert_eq!(
                selected.endpoint,
                "https://example.test/v1/chat/completions"
            );
            assert_eq!(selected.model, "chosen-model");
            assert_eq!(selected.legacy_projection(true)?.model, "chosen-model");
        }
        let disabled = AiProfileCatalog::from_legacy(&AiSettings {
            enabled: false,
            base_url: "https://example.test/v1".into(),
            model: "chosen-model".into(),
        })?;
        assert_eq!(disabled.profiles.len(), 1);
        assert!(disabled.active().is_none());
        Ok(())
    }

    #[test]
    fn catalog_edit_is_transactional_and_selection_cannot_dangle()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = profile("Production");
        let id = first.id;
        let mut catalog = AiProfileCatalog::default();
        catalog.upsert(first)?;
        catalog.activate(id)?;
        let before = catalog.clone();
        assert!(catalog.upsert(profile("production")).is_err());
        assert_eq!(catalog, before);
        assert!(catalog.activate(Uuid::new_v4()).is_err());
        assert_eq!(catalog.active_id, Some(id));
        assert!(catalog.remove(id).is_some());
        assert!(catalog.active().is_none());
        assert!(catalog.validate().is_ok());
        Ok(())
    }

    #[test]
    fn serialized_profiles_have_references_and_reject_plaintext_secret_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut profile = profile("Named profile");
        profile.authentication = AiAuthentication::Bearer {
            credential: Some(environment("KEELSHELL_AI_KEY")),
        };
        profile.custom_headers.push(AiCustomHeader {
            name: "X-Tenant".into(),
            value_ref: environment("KEELSHELL_AI_TENANT"),
        });
        profile.validate()?;
        let encoded = serde_json::to_value(&profile)?;
        assert_eq!(
            serde_json::from_value::<NamedAiProfile>(encoded.clone())?,
            profile
        );
        let mut invalid = encoded;
        invalid["api_key"] = serde_json::json!("do-not-store-this");
        assert!(serde_json::from_value::<NamedAiProfile>(invalid).is_err());
        assert!(
            serde_json::from_value::<AiCustomHeader>(
                serde_json::json!({"name":"X-Tenant","value":"do-not-store-this"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<AiSecretRef>(
                serde_json::json!({"source":"environment","name":"KEY","value":"do-not-store-this"})
            )
            .is_err()
        );
        let debug = format!("{profile:?}");
        assert!(!debug.contains("Named profile"));
        assert!(!debug.contains("api.openai.com"));
        assert!(!debug.contains("KEELSHELL_AI_KEY"));
        Ok(())
    }

    #[test]
    fn credentials_queries_and_non_loopback_http_are_refused() {
        let mut profile = profile("Test");
        for address in [
            "https://key:secret@example.test/v1/chat/completions",
            "https://example.test/v1/chat/completions?api_key=secret",
            "https://example.test/v1/chat/completions#secret",
            "http://example.test/v1/chat/completions",
            "http://localhost.example.test/v1/chat/completions",
            "file:///tmp/config",
            "https://example.test:0/v1/chat/completions",
        ] {
            profile.endpoint = address.into();
            let error = profile.validate().err();
            assert!(error.is_some());
            assert!(!format!("{error:?}").contains("secret"));
        }
        for address in [
            "http://127.0.0.1:11434/v1/chat/completions",
            "http://[::1]:11434/v1/chat/completions",
            "https://example.test/custom-route",
        ] {
            profile.endpoint = address.into();
            assert!(profile.validate().is_ok());
        }
    }

    #[test]
    fn header_injection_and_authentication_collisions_are_refused() {
        let mut profile = profile("Test");
        for name in [
            "X-Test\r\nHost",
            "Host",
            "Authorization",
            "Content-Length",
            "Cookie",
            "Proxy-Authorization",
            "Bad Header",
        ] {
            profile.custom_headers = vec![AiCustomHeader {
                name: name.into(),
                value_ref: environment("KEY"),
            }];
            assert!(profile.validate().is_err());
        }
        profile.authentication = AiAuthentication::Header {
            name: "X-API-Key".into(),
            credential: Some(environment("API_KEY")),
        };
        profile.custom_headers = vec![AiCustomHeader {
            name: "x-api-key".into(),
            value_ref: environment("OTHER_KEY"),
        }];
        assert!(profile.validate().is_err());
        assert!(environment("KEY=value").validate().is_err());
        assert!(environment("9KEY").validate().is_err());
        assert!(
            AiSecretRef::SecretStore { id: Uuid::nil() }
                .validate()
                .is_err()
        );
    }

    #[test]
    fn reasoning_values_are_bound_to_their_model_capability()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut profile = profile("Test");
        profile.context_window_tokens = Some(32_768);
        profile.max_output_tokens = Some(4_096);
        profile.reasoning_by_model.insert(
            profile.model.clone(),
            AiModelReasoning {
                capability: AiReasoningCapability::TokenBudget {
                    min: 1024,
                    max: 8192,
                },
                selection: AiReasoningSelection::Budget(2048),
            },
        );
        profile.validate()?;
        profile.max_output_tokens = Some(1024);
        assert!(profile.validate().is_err());
        profile.model = "other-model".into();
        profile.validate()?;
        assert!(!profile.reasoning_by_model.contains_key(&profile.model));
        assert!(
            AiModelReasoning {
                capability: AiReasoningCapability::Unknown,
                selection: AiReasoningSelection::Effort("high".into())
            }
            .validate()
            .is_err()
        );
        assert!(
            AiModelReasoning {
                capability: AiReasoningCapability::Effort {
                    values: vec!["low".into()]
                },
                selection: AiReasoningSelection::Effort("high".into())
            }
            .validate()
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn implemented_protocols_and_advanced_options_are_not_silently_downgraded()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut catalog = AiProfileCatalog::default();
        let mut profile = profile("Test");
        profile.api_style = AiApiStyle::AnthropicMessages;
        profile.endpoint = "https://api.anthropic.com/v1/messages".into();
        profile.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        let id = profile.id;
        catalog.upsert(profile.clone())?;
        catalog.activate(id)?;
        assert!(profile.legacy_projection(true).is_err());
        profile.api_style = AiApiStyle::ChatCompletions;
        profile.proxy = AiProxy::Explicit {
            url: "socks5h://localhost:1080".into(),
            credentials: None,
        };
        profile.validate()?;
        assert!(profile.legacy_projection(true).is_err());
        profile.proxy = AiProxy::Direct;
        profile.authentication = AiAuthentication::Bearer {
            credential: Some(environment("KEY")),
        };
        assert!(profile.legacy_projection(true).is_err());
        profile.proxy = AiProxy::Explicit {
            url: "http://user:secret@localhost:8080".into(),
            credentials: None,
        };
        assert!(profile.validate().is_err());
        Ok(())
    }

    #[test]
    fn anthropic_transport_requires_x_api_key_header_and_secret_store_or_transient_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let draft = NamedAiProfile::draft(AiPreset::Claude);
        assert_eq!(draft.api_style, AiApiStyle::AnthropicMessages);
        assert_eq!(draft.endpoint, "https://api.anthropic.com/v1/messages");
        assert!(matches!(
            draft.authentication,
            AiAuthentication::Header {
                name,
                credential: None
            } if name.eq_ignore_ascii_case("x-api-key")
        ));

        let mut profile = profile("Anthropic");
        profile.api_style = AiApiStyle::AnthropicMessages;
        profile.endpoint = "https://api.anthropic.com/v1/messages".into();
        profile.authentication = AiAuthentication::Header {
            name: "X-API-Key".into(),
            credential: None,
        };
        profile.validate_current_transport()?;
        profile.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        profile.validate_current_transport()?;

        for authentication in [
            AiAuthentication::Bearer { credential: None },
            AiAuthentication::Header {
                name: "authorization".into(),
                credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
            },
        ] {
            profile.authentication = authentication;
            assert!(profile.validate_current_transport().is_err());
        }

        profile.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: None,
        };
        profile.max_output_tokens = Some(4096);
        profile.validate_current_transport()?;
        profile.max_output_tokens = None;
        profile.custom_headers.push(AiCustomHeader {
            name: "x-tenant".into(),
            value_ref: AiSecretRef::SecretStore { id: Uuid::new_v4() },
        });
        profile.validate_current_transport()?;
        profile.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: Some(environment("ANTHROPIC_API_KEY")),
        };
        profile.validate_current_transport()?;
        Ok(())
    }

    #[test]
    fn token_limits_are_supported_but_cannot_be_lost_in_legacy_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut profile = profile("Token limits");
        profile.context_window_tokens = Some(32_768);
        profile.max_output_tokens = Some(8192);
        profile.validate_current_transport()?;
        assert!(profile.legacy_projection(true).is_err());
        profile.max_output_tokens = Some(1_000_001);
        assert!(profile.validate_current_transport().is_err());
        profile.max_output_tokens = None;
        profile.context_window_tokens = Some(4096);
        assert!(profile.validate_current_transport().is_err());
        Ok(())
    }
}
