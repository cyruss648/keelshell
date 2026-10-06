//! Exact decimal sampling values and model-specific declarations.
use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// A nonnegative sampling decimal, stored exactly in thousandths (0–2000).
///
/// Integer storage retains `Eq`, rejects non-finite values, and makes persisted
/// precision explicit. The API adapter emits the corresponding JSON number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct AiSamplingValue(u16);

impl AiSamplingValue {
    /// Construct a bounded exact value in thousandths.
    pub fn from_millis(value: u16) -> Result<Self, ValidationError> {
        if value > 2000 {
            return Err(ValidationError::new("ai.sampling.value", "must be 0–2"));
        }
        Ok(Self(value))
    }

    /// Parse a plain decimal with at most three fractional digits.
    /// Exponents, signs, NaN, infinity and rounding are deliberately rejected.
    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        let value = value.trim();
        let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
        if whole.is_empty()
            || !whole.bytes().all(|b| b.is_ascii_digit())
            || fraction.len() > 3
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(ValidationError::new(
                "ai.sampling.value",
                "use a decimal with at most three fractional digits",
            ));
        }
        let whole: u16 = whole
            .parse()
            .map_err(|_| ValidationError::new("ai.sampling.value", "must be 0–2"))?;
        let fraction: u16 = format!("{fraction:0<3}")
            .parse()
            .map_err(|_| ValidationError::new("ai.sampling.value", "invalid decimal"))?;
        let millis = whole
            .checked_mul(1000)
            .and_then(|n| n.checked_add(fraction))
            .ok_or_else(|| ValidationError::new("ai.sampling.value", "must be 0–2"))?;
        Self::from_millis(millis)
    }

    /// Exact stored thousandths.
    pub const fn millis(self) -> u16 {
        self.0
    }

    /// Plain decimal for the editor, without approximation or exponent notation.
    pub fn decimal(self) -> String {
        if self.0.is_multiple_of(1000) {
            (self.0 / 1000).to_string()
        } else {
            format!("{}.{:03}", self.0 / 1000, self.0 % 1000)
                .trim_end_matches('0')
                .to_owned()
        }
    }
}

impl TryFrom<u16> for AiSamplingValue {
    type Error = ValidationError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::from_millis(value)
    }
}
impl From<AiSamplingValue> for u16 {
    fn from(value: AiSamplingValue) -> Self {
        value.0
    }
}

/// Explicit model support declaration plus optional sampling values.
/// Missing values mean omission, including when a model is declared supported.
/// Select temperature or nucleus sampling, rather than combining both knobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AiModelSampling {
    /// User checked this exact model's documentation for sampling support.
    pub declared_supported: bool,
    /// Optional temperature (0–2; Messages is limited to 0–1).
    pub temperature: Option<AiSamplingValue>,
    /// Optional nucleus sampling probability (0–1).
    pub top_p: Option<AiSamplingValue>,
}

impl AiModelSampling {
    /// Whether a non-default sampling field would be sent.
    pub fn is_explicit(&self) -> bool {
        self.temperature.is_some() || self.top_p.is_some()
    }

    /// Validate declaration, probability bounds and mutually exclusive knobs.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if (!self.declared_supported && self.is_explicit())
            || (self.temperature.is_some() && self.top_p.is_some())
            || self.top_p.is_some_and(|v| v.millis() > 1000)
        {
            return Err(ValidationError::new(
                "ai.sampling",
                "declare support and select one bounded sampling parameter",
            ));
        }
        Ok(())
    }
}
