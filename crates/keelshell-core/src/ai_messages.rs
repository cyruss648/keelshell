//! Independently composed Messages effort and thinking settings.
use serde::{Deserialize, Serialize};

use crate::{AiReasoningSelection, ValidationError};

/// Closed Messages effort vocabulary; each model's support still needs a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMessagesEffort {
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
impl AiMessagesEffort {
    /// Protocol spelling for metadata admission and API mapping.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
    /// Parse an existing effort selection without guessing model capabilities.
    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            _ => Err(ValidationError::new(
                "ai.messages.effort",
                "unsupported Messages effort",
            )),
        }
    }
}

/// Optional thinking mode, independent of Messages output effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(
    tag = "kind",
    content = "tokens",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AiMessagesThinking {
    /// Omit the thinking field.
    #[default]
    ProviderDefault,
    /// Request adaptive thinking on a supporting model.
    Adaptive,
    /// Explicitly disable thinking on a supporting model.
    Disabled,
    /// Legacy manual budget; the output ceiling must leave room for the answer.
    LegacyBudget(u32),
}

/// Typed combination stored in the existing model-scoped reasoning map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AiMessagesInference {
    /// None omits `output_config.effort`, independently of thinking.
    pub effort: Option<AiMessagesEffort>,
    /// Thinking mode; omission differs from explicitly disabled thinking.
    pub thinking: AiMessagesThinking,
}
impl AiMessagesInference {
    /// Interpret old single-field choices without changing their wire semantics.
    /// The first UI edit upgrades the selection to the composed representation.
    pub fn from_selection(selection: &AiReasoningSelection) -> Result<Self, ValidationError> {
        Ok(match selection {
            AiReasoningSelection::ProviderDefault => Self::default(),
            AiReasoningSelection::Effort(value) => Self {
                effort: Some(AiMessagesEffort::parse(value)?),
                ..Self::default()
            },
            AiReasoningSelection::Thinking(true) => Self {
                thinking: AiMessagesThinking::Adaptive,
                ..Self::default()
            },
            AiReasoningSelection::Thinking(false) => Self {
                thinking: AiMessagesThinking::Disabled,
                ..Self::default()
            },
            AiReasoningSelection::Budget(tokens) => Self {
                thinking: AiMessagesThinking::LegacyBudget(*tokens),
                ..Self::default()
            },
            AiReasoningSelection::Messages(value) => value.clone(),
            AiReasoningSelection::Text(_) => {
                return Err(ValidationError::new(
                    "ai.messages",
                    "text is not a Messages inference setting",
                ));
            }
        })
    }
    /// Validate legacy budget bounds independently of an eventual output ceiling.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if matches!(self.thinking, AiMessagesThinking::LegacyBudget(n) if !(1024..1_000_000).contains(&n))
        {
            return Err(ValidationError::new(
                "ai.messages.thinking",
                "legacy budget must be 1024–999999",
            ));
        }
        Ok(())
    }
    /// Manual budget when present, for draft and output admission.
    pub const fn budget_tokens(&self) -> Option<u32> {
        match self.thinking {
            AiMessagesThinking::LegacyBudget(n) => Some(n),
            _ => None,
        }
    }
    /// Whether conservative sampling admission allows this combination.
    pub fn allows_sampling(&self) -> bool {
        self.effort.is_none()
            && matches!(
                self.thinking,
                AiMessagesThinking::ProviderDefault | AiMessagesThinking::Disabled
            )
    }
}
