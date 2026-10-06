//! Typed inference fields. No arbitrary request-body extensions are accepted.
use serde_json::{Value, json};

use crate::{AiError, ProviderProtocol};

/// Protocol-defined reasoning effort values; model support must be declared
/// separately by the caller. A protocol-level value is not model discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    /// Explicitly disable reasoning where the model supports it.
    None,
    /// Minimal reasoning (OpenAI protocols only).
    Minimal,
    /// Low effort.
    Low,
    /// Medium effort.
    Medium,
    /// High effort.
    High,
    /// Extra-high effort.
    Xhigh,
    /// Maximum effort.
    Max,
}
impl ReasoningEffort {
    /// Exact protocol spelling, without provider-specific strings or JSON.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
    /// Parse one exact supported protocol value.
    pub fn parse(value: &str) -> Result<Self, AiError> {
        match value {
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            _ => Err(AiError::InvalidInferenceOptions),
        }
    }
}

/// Messages thinking mode, independently composed with output effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessagesThinking {
    /// Omit the thinking field.
    #[default]
    ProviderDefault,
    /// Adaptive thinking on a declared supporting model.
    Adaptive,
    /// Explicitly disabled thinking on a declared supporting model.
    Disabled,
    /// Legacy manual token budget, strictly below the output limit.
    LegacyBudget(u32),
}

/// Optional reasoning instruction. Default means omission, not a chosen effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReasoningOption {
    /// Leave all reasoning fields absent.
    #[default]
    ProviderDefault,
    /// Set the protocol's effort field.
    Effort(ReasoningEffort),
    /// Messages adaptive thinking or explicit disabled thinking.
    Thinking(bool),
    /// Legacy Messages manual thinking, only for models supporting it.
    TokenBudget(u32),
    /// Independent Messages effort and thinking fields.
    Messages {
        /// None omits the effort field.
        effort: Option<ReasoningEffort>,
        /// Thinking mode, independent of effort.
        thinking: MessagesThinking,
    },
}

/// One optional sampling parameter in exact thousandths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SamplingOption {
    /// Leave temperature and top_p absent.
    #[default]
    ProviderDefault,
    /// Temperature in 0–2000 thousandths (Messages: at most 1000).
    Temperature(u16),
    /// Nucleus sampling in 0–1000 thousandths.
    TopP(u16),
}

/// Immutable, typed inference fields bound to a provider before preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InferenceOptions {
    /// Explicit reasoning instruction or omission.
    pub reasoning: ReasoningOption,
    /// Explicit sampling instruction or omission.
    pub sampling: SamplingOption,
}
impl InferenceOptions {
    pub(crate) fn is_omitted(self) -> bool {
        self.sampling == SamplingOption::ProviderDefault
            && matches!(
                self.reasoning,
                ReasoningOption::ProviderDefault
                    | ReasoningOption::Messages {
                        effort: None,
                        thinking: MessagesThinking::ProviderDefault
                    }
            )
    }

    pub(crate) fn validate(
        self,
        protocol: ProviderProtocol,
        output: Option<u32>,
    ) -> Result<(), AiError> {
        let messages = protocol == ProviderProtocol::AnthropicMessages;
        let reasoning_valid = match self.reasoning {
            ReasoningOption::ProviderDefault => true,
            ReasoningOption::Effort(ReasoningEffort::None | ReasoningEffort::Minimal) => !messages,
            ReasoningOption::Effort(_) => true,
            ReasoningOption::Thinking(_) => messages,
            ReasoningOption::TokenBudget(n) => messages && n >= 1024 && n < output.unwrap_or(4096),
            ReasoningOption::Messages { effort, thinking } => {
                messages
                    && !matches!(
                        effort,
                        Some(ReasoningEffort::None | ReasoningEffort::Minimal)
                    )
                    && !matches!(thinking, MessagesThinking::LegacyBudget(n) if n < 1024 || n >= output.unwrap_or(4096))
            }
        };
        let sampling_valid = match self.sampling {
            SamplingOption::ProviderDefault => true,
            SamplingOption::Temperature(n) => n <= if messages { 1000 } else { 2000 },
            SamplingOption::TopP(n) => n <= 1000,
        };
        let mixed = self.sampling != SamplingOption::ProviderDefault
            && !matches!(
                self.reasoning,
                ReasoningOption::ProviderDefault
                    | ReasoningOption::Thinking(false)
                    | ReasoningOption::Effort(ReasoningEffort::None)
                    | ReasoningOption::Messages {
                        effort: None,
                        thinking: MessagesThinking::ProviderDefault | MessagesThinking::Disabled
                    }
            );
        if !reasoning_valid || !sampling_valid || mixed {
            return Err(AiError::InvalidInferenceOptions);
        }
        Ok(())
    }

    pub(crate) fn apply(
        self,
        body: &mut Value,
        protocol: ProviderProtocol,
        output: Option<u32>,
    ) -> Result<(), AiError> {
        self.validate(protocol, output)?;
        match self.reasoning {
            ReasoningOption::ProviderDefault => {}
            ReasoningOption::Effort(effort) => match protocol {
                ProviderProtocol::ChatCompletions => {
                    body["reasoning_effort"] = json!(effort.as_str())
                }
                ProviderProtocol::Responses => {
                    body["reasoning"] = json!({"effort": effort.as_str()})
                }
                ProviderProtocol::AnthropicMessages => {
                    body["output_config"] = json!({"effort": effort.as_str()})
                }
            },
            ReasoningOption::Thinking(enabled) => {
                body["thinking"] = json!({"type": if enabled {"adaptive"} else {"disabled"}})
            }
            ReasoningOption::TokenBudget(tokens) => {
                body["thinking"] = json!({"type":"enabled", "budget_tokens":tokens})
            }
            ReasoningOption::Messages { effort, thinking } => {
                if let Some(effort) = effort {
                    body["output_config"] = json!({"effort": effort.as_str()});
                }
                match thinking {
                    MessagesThinking::ProviderDefault => {}
                    MessagesThinking::Adaptive => body["thinking"] = json!({"type":"adaptive"}),
                    MessagesThinking::Disabled => body["thinking"] = json!({"type":"disabled"}),
                    MessagesThinking::LegacyBudget(tokens) => {
                        body["thinking"] = json!({"type":"enabled", "budget_tokens":tokens})
                    }
                }
            }
        }
        match self.sampling {
            SamplingOption::ProviderDefault => {}
            SamplingOption::Temperature(n) => body["temperature"] = json!(f64::from(n) / 1000.),
            SamplingOption::TopP(n) => body["top_p"] = json!(f64::from(n) / 1000.),
        }
        Ok(())
    }
}
