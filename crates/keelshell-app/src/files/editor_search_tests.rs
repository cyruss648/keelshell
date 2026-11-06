#[cfg(test)]
mod tests {
    use crate::files::editor_search::{DRAFT_LIMIT, editor_matches, replace_editor_draft};

    fn draft<T>(result: Result<T, impl std::fmt::Debug>) -> T {
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

    #[test]
    fn editor_search_and_patch_address_body_offsets_and_keep_bom_crlf_envelope() {
        use keelshell_core::{Utf8TextDocument, apply_text_patch};

        let original = "\u{feff}中文\u{feff}\r\nkeep\r\n";
        let document = draft(Utf8TextDocument::decode(original.as_bytes()));
        let matches = draft(editor_matches(document.body(), "中文"));
        assert_eq!((matches[0].start, matches[0].end), (0, "中文".len()));
        assert_eq!(
            draft(editor_matches(document.body(), "\u{feff}")).len(),
            1,
            "only the internal U+FEFF is searchable"
        );
        let replaced = draft(replace_editor_draft(document.body(), "中文", "替换", true));
        assert_eq!(replaced, "替换\u{feff}\r\nkeep\r\n");
        let patch = "--- /body.txt\n+++ /body.txt\n@@ -2 +2 @@\n-keep\r\n+patched\n";
        let patched = draft(apply_text_patch("/body.txt", &replaced, patch));
        assert_eq!(patched, "替换\u{feff}\r\npatched\n");
        assert_eq!(
            draft(document.format().prepare_save(&patched)),
            "\u{feff}替换\u{feff}\r\npatched\r\n".as_bytes()
        );
        assert_eq!(document.body(), "中文\u{feff}\r\nkeep\r\n");
    }
}
