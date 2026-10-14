#[cfg(test)]
mod tests {
    use crate::files::editor_search::{DRAFT_LIMIT, editor_matches, replace_editor_draft};

    fn draft(result: Result<String, impl std::fmt::Debug>) -> String {
        result.unwrap_or_else(|error| panic!("editor draft: {error:?}"))
    }

    #[test]
    fn literal_search_is_bounded_and_nonoverlapping() {
        let matches = editor_matches("a.a.a", ".")
            .unwrap_or_else(|error| panic!("editor matches: {error:?}"));
        assert_eq!(matches.len(), 2);
        assert!(editor_matches("draft", "").is_err());
        assert!(editor_matches("draft\0", "a").is_err());
        assert!(editor_matches(&"x".repeat(DRAFT_LIMIT + 1), "x").is_err());
    }

    #[test]
    fn replacement_changes_only_the_requested_matches() {
        assert_eq!(draft(replace_editor_draft("a-a", "a", "b", true)), "b-a");
        assert_eq!(
            draft(replace_editor_draft("a-a", "a", "bb", false)),
            "bb-bb"
        );
        assert_eq!(
            draft(replace_editor_draft("draft", "missing", "x", false)),
            "draft"
        );
        assert!(replace_editor_draft("a", "a", &"b".repeat(DRAFT_LIMIT + 1), false).is_err());
    }
}
