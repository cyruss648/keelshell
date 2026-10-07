//! Monotonic scheduling separates policy/clock changes from transport ownership.

use std::time::{Duration, Instant};

use keelshell_core::UpdatePreferences;

pub(super) const STARTUP_DELAY: Duration = Duration::from_secs(60);
const INITIAL_BACKOFF: Duration = Duration::from_secs(15 * 60);
const MAX_BACKOFF: Duration = Duration::from_secs(6 * 60 * 60);

pub(super) struct Schedule {
    next: Option<Instant>,
    failures: u32,
}

impl Schedule {
    pub(super) fn new(preferences: UpdatePreferences, now: Instant, unix_seconds: u64) -> Self {
        let mut schedule = Self {
            next: None,
            failures: 0,
        };
        schedule.reset(preferences, now, unix_seconds);
        schedule
    }

    pub(super) fn reset(
        &mut self,
        preferences: UpdatePreferences,
        now: Instant,
        unix_seconds: u64,
    ) {
        self.failures = 0;
        self.next = preferences.frequency.interval().map(|interval| {
            let remaining = preferences
                .last_successful_check
                .map_or(Duration::ZERO, |last| {
                    // A backward wall-clock jump waits at most one normal interval;
                    // it must not suspend checks until an arbitrary future timestamp.
                    interval.saturating_sub(Duration::from_secs(unix_seconds.saturating_sub(last)))
                });
            now + remaining.max(STARTUP_DELAY)
        });
    }

    pub(super) fn due(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| now >= next)
    }

    pub(super) fn succeeded(&mut self, preferences: UpdatePreferences, now: Instant) {
        self.failures = 0;
        self.next = preferences
            .frequency
            .interval()
            .map(|interval| now + interval);
    }

    pub(super) fn failed(&mut self, preferences: UpdatePreferences, now: Instant) {
        self.failures = self.failures.saturating_add(1).min(6);
        self.next = preferences.frequency.interval().map(|interval| {
            let factor = 1u32 << self.failures.saturating_sub(1);
            now + INITIAL_BACKOFF
                .saturating_mul(factor)
                .min(MAX_BACKOFF)
                .min(interval)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelshell_core::UpdateCheckFrequency;

    #[test]
    fn startup_is_delayed_and_a_restart_honors_the_saved_daily_check() {
        let now = Instant::now();
        let preferences = UpdatePreferences::default();
        let schedule = Schedule::new(preferences, now, 1_700_000_000);
        assert!(!schedule.due(now + STARTUP_DELAY - Duration::from_millis(1)));
        assert!(schedule.due(now + STARTUP_DELAY));
        let schedule = Schedule::new(
            UpdatePreferences {
                last_successful_check: Some(1_699_999_000),
                ..preferences
            },
            now,
            1_700_000_000,
        );
        assert!(!schedule.due(now + Duration::from_secs(85_399)));
        assert!(schedule.due(now + Duration::from_secs(85_400)));
    }

    #[test]
    fn failure_delays_are_bounded_and_disabled_policy_has_no_deadline() {
        let now = Instant::now();
        let preferences = UpdatePreferences::default();
        let mut schedule = Schedule::new(preferences, now, 0);
        for seconds in [900, 1_800, 3_600, 7_200, 14_400, 21_600, 21_600] {
            schedule.failed(preferences, now);
            assert!(!schedule.due(now + Duration::from_secs(seconds - 1)));
            assert!(schedule.due(now + Duration::from_secs(seconds)));
        }
        let disabled = UpdatePreferences {
            frequency: UpdateCheckFrequency::Disabled,
            ..preferences
        };
        schedule.reset(disabled, now, 0);
        schedule.failed(disabled, now);
        assert!(!schedule.due(now + Duration::from_secs(100 * 86_400)));
        schedule.reset(preferences, now, 0);
        schedule.succeeded(preferences, now);
        assert!(!schedule.due(now + Duration::from_secs(86_399)));
        assert!(schedule.due(now + Duration::from_secs(86_400)));
    }

    #[test]
    fn clock_changes_cannot_create_a_tight_loop_or_unbounded_suspension() {
        let now = Instant::now();
        let preferences = UpdatePreferences {
            last_successful_check: Some(2_000_000_000),
            ..UpdatePreferences::default()
        };
        let schedule = Schedule::new(preferences, now, 1_700_000_000);
        assert!(!schedule.due(now + Duration::from_secs(86_399)));
        assert!(schedule.due(now + Duration::from_secs(86_400)));
        let schedule = Schedule::new(preferences, now, 3_000_000_000);
        assert!(!schedule.due(now));
        assert!(schedule.due(now + STARTUP_DELAY));
    }
}
