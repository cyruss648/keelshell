//! Bounded filename-based language selection for the remote draft editor.
//!
//! The editor receives only a filename or remote path.  This module deliberately
//! performs no content sniffing: an unknown or ambiguous name remains plain text
//! and never changes the bytes in the draft.

/// Return the GPUI Kit language name for a remote file path.
///
/// The result is one of the built-in Tree-sitter language names or
/// `"text"` when the filename is unknown. Matching is case-insensitive
/// and uses the final filename component, so a directory name containing a
/// familiar suffix does not select a language.
pub(super) fn language_for_path(path: &str) -> &'static str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();

    match lower.as_str() {
        "dockerfile" | "containerfile" | "makefile" | "gnumakefile" => "text",
        ".bashrc" | ".zshrc" | ".profile" => "bash",
        _ => match lower.rsplit_once('.') {
            Some((_, "rs")) => "rust",
            Some((_, "py" | "pyi")) => "python",
            Some((_, "json" | "jsonc")) => "json",
            Some((_, "toml")) => "toml",
            Some((_, "yaml" | "yml")) => "yaml",
            Some((_, "sh" | "bash" | "zsh")) => "bash",
            _ => "text",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::language_for_path;

    #[test]
    fn recognizes_supported_extensions_case_insensitively() {
        for (path, expected) in [
            ("src/main.rs", "rust"),
            ("scripts/BUILD.PY", "python"),
            ("config.JSONC", "json"),
            ("settings.toml", "toml"),
            ("deploy.YML", "yaml"),
            (".config/zshrc.SH", "bash"),
            (".bashrc", "bash"),
        ] {
            assert_eq!(language_for_path(path), expected, "{path}");
        }
    }

    #[test]
    fn unknown_and_ambiguous_names_remain_plaintext() {
        for path in [
            "README",
            "archive.rs.bak",
            "/tmp/.config",
            "folder.rs/file",
            "queries/report.sql",
            "Dockerfile",
            "scripts/deploy.fish",
        ] {
            assert_eq!(language_for_path(path), "text", "{path}");
        }
    }
}
