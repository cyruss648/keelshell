//! Non-secret update policy. Release transport and installation belong to the app.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// User-selected cadence for background release checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckFrequency {
    /// Only explicit checks are performed; automatic downloads are suspended.
    Disabled,
    /// Check once per day, the first-launch and legacy-configuration default.
    #[default]
    Daily,
    /// Check once per week.
    Weekly,
}

impl UpdateCheckFrequency {
    /// Normal interval; disabled checks have no background deadline.
    pub const fn interval(self) -> Option<Duration> {
        match self {
            Self::Disabled => None,
            Self::Daily => Some(Duration::from_secs(24 * 60 * 60)),
            Self::Weekly => Some(Duration::from_secs(7 * 24 * 60 * 60)),
        }
    }
}

/// Persistent, credential-free background update preferences and check history.
///
/// Automatic download never authorizes installation. The timestamp describes a
/// completed release check, not a successful download, signature or installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePreferences {
    /// Background cadence. Manual checks remain available when disabled.
    pub frequency: UpdateCheckFrequency,
    /// Download and verify a newer matching release without installing it.
    pub auto_download: bool,
    /// Local wall-clock seconds since the Unix epoch at the last completed check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_check: Option<u64>,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            frequency: UpdateCheckFrequency::Daily,
            auto_download: false,
            last_successful_check: None,
        }
    }
}

impl UpdatePreferences {
    /// Reject timestamps beyond the representable calendar range used by metadata.
    pub fn validate(self) -> Result<(), ValidationError> {
        if self
            .last_successful_check
            .is_some_and(|seconds| seconds > 253_402_300_799)
        {
            return Err(ValidationError::new(
                "settings.updates.last_successful_check",
                "must be Unix seconds within the supported calendar range",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_is_strict_and_never_enables_automatic_installation() {
        let preferences = UpdatePreferences::default();
        assert_eq!(
            preferences.frequency.interval(),
            Some(Duration::from_secs(86_400))
        );
        assert!(!preferences.auto_download);
        assert!(preferences.last_successful_check.is_none());
        assert!(UpdateCheckFrequency::Disabled.interval().is_none());
        for value in [
            serde_json::json!({"frequency":"daily","auto_download":false,"auto_install":true}),
            serde_json::json!({"frequency":"hourly","auto_download":false}),
            serde_json::json!({"frequency":"daily","auto_download":"yes"}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<UpdatePreferences>(value).is_err());
        }
    }

    #[test]
    fn check_metadata_round_trips_and_rejects_unbounded_timestamps()
    -> Result<(), Box<dyn std::error::Error>> {
        let preferences = UpdatePreferences {
            frequency: UpdateCheckFrequency::Weekly,
            auto_download: true,
            last_successful_check: Some(1_700_000_000),
        };
        assert_eq!(
            serde_json::from_slice::<UpdatePreferences>(&serde_json::to_vec(&preferences)?)?,
            preferences
        );
        assert!(
            UpdatePreferences {
                last_successful_check: Some(u64::MAX),
                ..preferences
            }
            .validate()
            .is_err()
        );
        Ok(())
    }
}
