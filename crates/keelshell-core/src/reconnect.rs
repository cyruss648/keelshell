//! Persisted, bounded preferences for rebuilding an established SSH session.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// Reconnection preferences for the final profile of an SSH route.
///
/// This policy stores no secrets or runtime attempt state. Automatic attempts
/// rebuild the whole route; jump profiles do not add nested retry budgets.
/// The caller must classify the disconnection, check the current route and host
/// trust, and obtain any required authentication before connecting again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReconnectPolicy {
    /// Wait for the user to request another connection after disconnection.
    #[default]
    Manual,
    /// Allow a bounded sequence of automatic attempts after a transient loss.
    Automatic {
        /// Maximum whole-route attempts in one disconnection episode, 1–10.
        max_attempts: u8,
        /// Delay before the first attempt, 1–60 seconds.
        initial_delay_seconds: u16,
        /// Cap for exponential backoff, at least the initial delay and at most 300 seconds.
        max_delay_seconds: u16,
    },
}

// Struct wire variants reject fields that Serde otherwise ignores on internally
// tagged unit variants, including attempts to smuggle secrets into Manual.
impl<'de> Deserialize<'de> for ReconnectPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum WirePolicy {
            Manual {},
            Automatic {
                max_attempts: u8,
                initial_delay_seconds: u16,
                max_delay_seconds: u16,
            },
        }
        match WirePolicy::deserialize(deserializer)? {
            WirePolicy::Manual {} => Ok(Self::Manual),
            WirePolicy::Automatic {
                max_attempts,
                initial_delay_seconds,
                max_delay_seconds,
            } => Ok(Self::Automatic {
                max_attempts,
                initial_delay_seconds,
                max_delay_seconds,
            }),
        }
    }
}

impl ReconnectPolicy {
    /// Whether this policy requires an explicit user request after disconnection.
    pub const fn is_manual(&self) -> bool {
        matches!(self, Self::Manual)
    }

    /// Validate finite retry counts and nonzero, bounded backoff intervals.
    ///
    /// Deserialization validates the wire shape; callers must still validate
    /// these numeric bounds before accepting a profile or scheduling attempts.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let Self::Automatic {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } = *self
        else {
            return Ok(());
        };
        if !(1..=10).contains(&max_attempts) {
            return Err(ValidationError::new(
                "connection.reconnect.max_attempts",
                "must be between 1 and 10",
            ));
        }
        if !(1..=60).contains(&initial_delay_seconds) {
            return Err(ValidationError::new(
                "connection.reconnect.initial_delay_seconds",
                "must be between 1 and 60 seconds",
            ));
        }
        if !(initial_delay_seconds..=300).contains(&max_delay_seconds) {
            return Err(ValidationError::new(
                "connection.reconnect.max_delay_seconds",
                "must be at least the initial delay and at most 300 seconds",
            ));
        }
        Ok(())
    }

    /// Return the capped exponential delay before a one-based whole-route attempt.
    ///
    /// Manual, invalid policies, attempt zero and exhausted budgets return `None`.
    /// Attempt counters belong to the caller and must not reset merely because a
    /// short-lived connection succeeds; otherwise a flapping route can retry forever.
    pub fn retry_delay(&self, attempt: usize) -> Option<Duration> {
        self.validate().ok()?;
        let Self::Automatic {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } = *self
        else {
            return None;
        };
        if !(1..=usize::from(max_attempts)).contains(&attempt) {
            return None;
        }
        let seconds = (1..attempt).fold(initial_delay_seconds, |delay, _| {
            delay.saturating_mul(2).min(max_delay_seconds)
        });
        Some(Duration::from_secs(u64::from(seconds)))
    }
}
