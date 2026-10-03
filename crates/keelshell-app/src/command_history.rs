//! In-memory command history for each remote terminal tab.
//!
//! History is deliberately scoped to the current application process. It is
//! useful for repeating reviewed remote commands without putting command text
//! (which may contain secrets) into the persisted profile store.

use std::collections::VecDeque;

/// Maximum number of commands retained for one remote terminal tab.
pub const MAX_ENTRIES: usize = 200;

/// A bounded, newest-last command history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandHistory {
    entries: VecDeque<String>,
}

impl CommandHistory {
    /// Record one command after removing only transport line terminators.
    ///
    /// Empty input is ignored. Consecutive identical commands are collapsed so
    /// pressing Run twice does not fill the history with duplicate rows.
    pub fn record(&mut self, command: &str) -> bool {
        let command = command.trim_end_matches(['\r', '\n']);
        if command.trim().is_empty() {
            return false;
        }
        if self
            .entries
            .back()
            .is_some_and(|previous| previous == command)
        {
            return false;
        }
        self.entries.push_back(command.to_owned());
        while self.entries.len() > MAX_ENTRIES {
            self.entries.pop_front();
        }
        true
    }

    /// Iterate from the most recent command to the oldest command.
    pub fn newest_first(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().rev().map(String::as_str)
    }

    /// Remove every command and report whether the history changed.
    pub fn clear(&mut self) -> bool {
        let changed = !self.entries.is_empty();
        self.entries.clear();
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandHistory, MAX_ENTRIES};

    #[test]
    fn ignores_empty_input_and_collapses_consecutive_duplicates() {
        let mut history = CommandHistory::default();
        assert!(!history.record(" \r\n"));
        assert!(history.record("  printf 'ok'  \r\n"));
        assert!(!history.record("  printf 'ok'  \r\n"));
        assert_eq!(
            history.newest_first().collect::<Vec<_>>(),
            ["  printf 'ok'  "]
        );
    }

    #[test]
    fn retains_multiline_commands_and_returns_newest_first() {
        let mut history = CommandHistory::default();
        assert!(history.record("first"));
        assert!(history.record("printf '%s\\n' one\nprintf '%s\\n' two"));
        assert_eq!(
            history.newest_first().collect::<Vec<_>>(),
            ["printf '%s\\n' one\nprintf '%s\\n' two", "first"]
        );
    }

    #[test]
    fn bounds_history_without_dropping_the_newest_command() {
        let mut history = CommandHistory::default();
        for index in 0..(MAX_ENTRIES + 3) {
            assert!(history.record(&format!("command-{index}")));
        }
        assert_eq!(history.newest_first().count(), MAX_ENTRIES);
        assert_eq!(history.newest_first().next(), Some("command-202"));
        assert!(!history.newest_first().any(|command| command == "command-0"));
        assert!(history.newest_first().any(|command| command == "command-3"));
    }

    #[test]
    fn clear_reports_changes_once() {
        let mut history = CommandHistory::default();
        assert!(!history.clear());
        assert!(history.record("id"));
        assert!(history.clear());
        assert!(!history.clear());
        assert!(history.newest_first().next().is_none());
    }
}
