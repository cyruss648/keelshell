//! Validated theme preferences independent of UI palettes and appearance modes.

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// A persisted theme family identifier, defaulting to `"frost"`.
///
/// Identifiers contain 1–64 ASCII lowercase letters, digits or hyphens and start
/// with a lowercase letter. Serialization uses a string; deserialization validates
/// the same constraints as [`Self::new`].
///
/// Unknown valid identifiers are preserved verbatim. Applications may resolve an
/// unavailable family to Frost without replacing the stored preference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ThemeId(String);

impl ThemeId {
    /// Validate an identifier without normalizing it or requiring a known family.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the identifier is not 1–64 ASCII lowercase
    /// letters, digits or hyphens, starting with a lowercase letter.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if !(1..=64).contains(&value.len())
            || !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ValidationError::new(
                "settings.theme_family",
                "must be 1–64 lowercase ASCII letters, digits or hyphens, starting with a letter",
            ));
        }
        Ok(Self(value))
    }

    /// Return the exact validated identifier as stored.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        Self("frost".into())
    }
}

impl<'de> Deserialize<'de> for ThemeId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeId;
    use serde_json::json;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn default_identifier_is_frost() {
        assert_eq!(ThemeId::default().as_str(), "frost");
    }

    #[test]
    fn valid_identifiers_round_trip_as_exact_strings() -> TestResult {
        let longest = "a".repeat(64);
        for value in [
            "a",
            "frost",
            "graphite",
            "lavender",
            "future-family-v2",
            "a0-z9",
            "a--",
            longest.as_str(),
        ] {
            let id = ThemeId::new(value)?;
            assert_eq!(id.as_str(), value);
            assert_eq!(serde_json::to_value(&id)?, json!(value));
            assert_eq!(serde_json::from_value::<ThemeId>(json!(value))?, id);
        }
        Ok(())
    }

    #[test]
    fn invalid_identifiers_fail_construction_and_deserialization() -> TestResult {
        let too_long = "a".repeat(65);
        for value in [
            "",
            "Frost",
            "frosT",
            "1frost",
            "-frost",
            "frost/other",
            "frost\\other",
            "frost.theme",
            "frost_theme",
            " frost",
            "frost ",
            "fröst",
            "霜",
            "frost\n",
            "frost\t",
            "frost\0",
            "frost\u{7f}",
            too_long.as_str(),
        ] {
            let error = ThemeId::new(value).err().ok_or("invalid ID accepted")?;
            assert_eq!(error.field, "settings.theme_family");
            assert!(
                serde_json::from_value::<ThemeId>(json!(value)).is_err(),
                "accepted {value:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn non_string_identifiers_are_rejected() {
        for value in [
            json!(null),
            json!(true),
            json!(1),
            json!(["frost"]),
            json!({"id": "frost"}),
        ] {
            assert!(
                serde_json::from_value::<ThemeId>(value.clone()).is_err(),
                "accepted {value}"
            );
        }
    }
}
