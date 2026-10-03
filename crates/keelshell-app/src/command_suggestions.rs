//! Local command-bar matches from this tab's history and explicitly saved snippets.
//!
//! This module never queries a remote shell. Accepting a result only supplies
//! text for review; the workspace owns target binding and explicit execution.

use crate::command_history::CommandHistory;
use keelshell_core::Snippet;
use uuid::Uuid;

/// Maximum number of automatic matches shown at once.
pub(crate) const MAX_SUGGESTIONS: usize = 8;
const HISTORY_TITLE_CHARS: usize = 120;

/// Provenance shown alongside a local command match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SuggestionSource {
    History,
    Snippet(Uuid),
}

/// An owned snapshot for a later, explicitly reviewed insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Suggestion {
    pub(crate) command: String,
    pub(crate) title: String,
    pub(crate) source: SuggestionSource,
}

/// Return bounded local matches, preserving command text byte for byte.
///
/// Search trims the query and compares Unicode lowercase characters without
/// normalization. Prefixes of any searchable field precede substring matches.
/// Ties retain newest-first history order, then saved snippet order. Identical
/// commands appear once, with the provenance of their highest-ranked match.
/// Empty or whitespace-only queries never open automatic suggestions.
pub(crate) fn suggest(
    history: &CommandHistory,
    snippets: &[Snippet],
    query: &str,
) -> Vec<Suggestion> {
    let Some(query) = Query::new(query) else {
        return Vec::new();
    };
    let mut matches = Vec::with_capacity(MAX_SUGGESTIONS + 1);
    for command in history.newest_first() {
        if let Some(rank) = query.rank(command) {
            insert_match(
                &mut matches,
                Candidate {
                    command,
                    title: None,
                    source: SuggestionSource::History,
                    rank,
                },
            );
        }
        if best_matches_are_full(&matches) {
            break;
        }
    }
    if !best_matches_are_full(&matches) {
        for snippet in snippets {
            if let Some(rank) = query.snippet_rank(snippet) {
                insert_match(
                    &mut matches,
                    Candidate {
                        command: &snippet.command,
                        title: Some(&snippet.name),
                        source: SuggestionSource::Snippet(snippet.id),
                        rank,
                    },
                );
            }
            if best_matches_are_full(&matches) {
                break;
            }
        }
    }
    // Keep references while ranking: a large unmatched command is never cloned.
    matches
        .into_iter()
        .map(|candidate| Suggestion {
            command: candidate.command.to_owned(),
            title: candidate
                .title
                .map_or_else(|| history_title(candidate.command), str::to_owned),
            source: candidate.source,
        })
        .collect()
}

/// Match a saved snippet's name, command, description or tags for a local list.
///
/// Unlike automatic suggestions, an empty query includes every snippet.
pub(crate) fn snippet_matches(snippet: &Snippet, query: &str) -> bool {
    Query::new(query).is_none_or(|query| query.snippet_rank(snippet).is_some())
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Prefix,
    Contains,
}

struct Candidate<'a> {
    command: &'a str,
    title: Option<&'a str>,
    source: SuggestionSource,
    rank: Rank,
}

fn best_matches_are_full(matches: &[Candidate<'_>]) -> bool {
    matches.len() == MAX_SUGGESTIONS && matches.last().is_some_and(|last| last.rank == Rank::Prefix)
}

fn insert_match<'a>(matches: &mut Vec<Candidate<'a>>, candidate: Candidate<'a>) {
    if let Some(index) = matches
        .iter()
        .position(|existing| existing.command == candidate.command)
    {
        if matches[index].rank <= candidate.rank {
            return;
        }
        matches.remove(index);
    }
    let index = matches
        .iter()
        .position(|existing| existing.rank > candidate.rank)
        .unwrap_or(matches.len());
    if index < MAX_SUGGESTIONS {
        matches.insert(index, candidate);
        matches.truncate(MAX_SUGGESTIONS);
    }
}

fn history_title(command: &str) -> String {
    let first_line = command.lines().next().unwrap_or_default();
    let mut chars = first_line.chars();
    let mut title: String = chars.by_ref().take(HISTORY_TITLE_CHARS).collect();
    if chars.next().is_some() || command.contains('\n') {
        title.push('…');
    }
    title
}

/// A streaming substring matcher avoids allocating a folded copy of every
/// potentially 64 KiB command on each command-bar edit.
struct Query {
    folded: Vec<char>,
    fallback: Vec<usize>,
}

impl Query {
    fn new(query: &str) -> Option<Self> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        let folded: Vec<_> = query.chars().flat_map(char::to_lowercase).collect();
        let mut fallback = vec![0; folded.len()];
        let mut matched = 0;
        for index in 1..folded.len() {
            while matched > 0 && folded[index] != folded[matched] {
                matched = fallback[matched - 1];
            }
            if folded[index] == folded[matched] {
                matched += 1;
            }
            fallback[index] = matched;
        }
        Some(Self { folded, fallback })
    }

    fn rank(&self, value: &str) -> Option<Rank> {
        let mut matched = 0;
        for (index, character) in value.chars().flat_map(char::to_lowercase).enumerate() {
            while matched > 0 && character != self.folded[matched] {
                matched = self.fallback[matched - 1];
            }
            if character == self.folded[matched] {
                matched += 1;
                if matched == self.folded.len() {
                    return Some(if index + 1 == matched {
                        Rank::Prefix
                    } else {
                        Rank::Contains
                    });
                }
            }
        }
        None
    }

    fn snippet_rank(&self, snippet: &Snippet) -> Option<Rank> {
        let fields = [
            snippet.name.as_str(),
            snippet.command.as_str(),
            snippet.description.as_str(),
        ]
        .into_iter()
        .chain(snippet.tags.iter().map(String::as_str));
        let mut best = None;
        for field in fields {
            match self.rank(field) {
                Some(Rank::Prefix) => return Some(Rank::Prefix),
                Some(Rank::Contains) => best = Some(Rank::Contains),
                None => {}
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_SUGGESTIONS, SuggestionSource, snippet_matches, suggest};
    use crate::command_history::CommandHistory;
    use keelshell_core::Snippet;

    #[test]
    fn empty_queries_never_suggest_but_keep_snippet_lists_unfiltered() {
        let mut history = CommandHistory::default();
        history.record("id");
        let snippets = [Snippet::new("Identity", "whoami")];
        for query in ["", " \t\r\n", "\u{3000}"] {
            assert!(suggest(&history, &snippets, query).is_empty());
            assert!(snippet_matches(&snippets[0], query));
        }
    }

    #[test]
    fn prefixes_precede_contains_with_latest_history_then_saved_order() {
        let mut history = CommandHistory::default();
        for command in ["tail old.log", "tail new.log", "sudo tail newest.log"] {
            history.record(command);
        }
        let snippets = [
            Snippet::new("Tail first", "journalctl -f"),
            Snippet::new("Tail second", "less server.log"),
            Snippet::new("Details", "sudo tail snippet.log"),
        ];
        let actual = suggest(&history, &snippets, "TAIL");
        let commands: Vec<_> = actual.iter().map(|item| item.command.as_str()).collect();
        assert_eq!(
            commands,
            [
                "tail new.log",
                "tail old.log",
                "journalctl -f",
                "less server.log",
                "sudo tail newest.log",
                "sudo tail snippet.log",
            ]
        );
        assert_eq!(actual, suggest(&history, &snippets, "TAIL"));
    }

    #[test]
    fn exact_duplicates_keep_best_rank_and_stable_provenance() {
        let mut history = CommandHistory::default();
        for command in ["sudo tail a", "tail b", "sudo tail a"] {
            history.record(command);
        }
        let snippets = [
            Snippet::new("Tail A", "sudo tail a"),
            Snippet::new("Tail B", "tail b"),
            Snippet::new("Tail again", "sudo tail a"),
        ];
        let actual = suggest(&history, &snippets, "tail");
        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].command, "tail b");
        assert_eq!(actual[0].source, SuggestionSource::History);
        assert_eq!(actual[1].source, SuggestionSource::Snippet(snippets[0].id));
        assert_eq!(actual[1].title, "Tail A");
    }

    #[test]
    fn bounded_results_allow_later_prefixes_to_displace_earlier_contains() {
        let mut history = CommandHistory::default();
        for index in 0..20 {
            history.record(&format!("sudo inspect history-{index}"));
        }
        let snippets: Vec<_> = (0..20)
            .map(|index| Snippet::new(format!("Inspect {index}"), format!("show {index}")))
            .collect();
        let actual = suggest(&history, &snippets, "inspect");
        assert_eq!(actual.len(), MAX_SUGGESTIONS);
        assert!(actual.iter().enumerate().all(|(index, item)| {
            item.source == SuggestionSource::Snippet(snippets[index].id)
                && item.command == format!("show {index}")
        }));
    }

    #[test]
    fn newest_matching_history_wins_at_the_cap() {
        let mut history = CommandHistory::default();
        for index in 0..20 {
            history.record(&format!("inspect {index}"));
        }
        let snippets = [Snippet::new("Inspect", "inspect snippet")];
        let actual = suggest(&history, &snippets, "inspect");
        assert_eq!(actual.len(), MAX_SUGGESTIONS);
        assert_eq!(
            actual.first().map(|item| item.command.as_str()),
            Some("inspect 19")
        );
        assert_eq!(
            actual.last().map(|item| item.command.as_str()),
            Some("inspect 12")
        );
        assert!(
            actual
                .iter()
                .all(|item| item.source == SuggestionSource::History)
        );
    }

    #[test]
    fn snippet_search_uses_each_metadata_field_without_rewriting_commands() {
        let mut snippet = Snippet::new("Deployment", "  printf 'ready'\n\tprintf 'done'  \n");
        snippet.description = "Read 服务状态 carefully".into();
        snippet.tags = vec!["运维".into(), "Production".into()];
        for query in ["DEPLOY", "  服务状态  ", "运维", "production", "printf"] {
            assert!(snippet_matches(&snippet, query));
            let actual = suggest(&CommandHistory::default(), &[snippet.clone()], query);
            assert_eq!(actual[0].command, snippet.command);
            assert_eq!(actual[0].title, snippet.name);
        }
        assert!(!snippet_matches(&snippet, "missing"));
    }

    #[test]
    fn unicode_lowercase_expansion_and_overlapping_patterns_match() {
        let snippets = [
            Snippet::new("İSTANBUL", "printf 'İSTANBUL'"),
            Snippet::new("服务状态", "printf '运维服务状态'"),
            Snippet::new("Overlap", "ababababac"),
        ];
        assert!(snippet_matches(&snippets[0], "i\u{307}stan"));
        assert!(snippet_matches(&snippets[1], "服务"));
        assert!(snippet_matches(&snippets[2], "ababac"));
        assert!(!snippet_matches(&snippets[2], "abababb"));
    }

    #[test]
    fn case_and_whitespace_variants_are_distinct_commands() {
        let snippets = [
            Snippet::new("One", "Echo value"),
            Snippet::new("Two", "echo value"),
            Snippet::new("Three", " echo value"),
            Snippet::new("Four", "echo value\n"),
        ];
        let actual = suggest(&CommandHistory::default(), &snippets, "echo");
        assert_eq!(actual.len(), 4);
        assert_eq!(actual[0].command, "Echo value");
        assert_eq!(actual[1].command, "echo value");
        assert_eq!(actual[2].command, "echo value\n");
        assert_eq!(actual[3].command, " echo value");
    }

    #[test]
    fn full_commands_survive_while_history_titles_are_bounded_unicode_text() {
        let mut history = CommandHistory::default();
        let command = format!("查{}\n\tprintf 'done'", "看".repeat(20_000));
        history.record(&command);
        let actual = suggest(&history, &[], "查");
        assert_eq!(actual[0].command, command);
        assert_eq!(actual[0].title.chars().count(), 121);
        assert!(actual[0].title.ends_with('…'));
    }

    #[test]
    fn long_nonmatching_text_does_not_hide_a_late_substring() {
        let command = format!("{}needle", "a".repeat(65_520));
        let snippets = [Snippet::new("Large command", command.clone())];
        let actual = suggest(&CommandHistory::default(), &snippets, "NEEDLE");
        assert_eq!(actual[0].command, command);
    }
}
