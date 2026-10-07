//! Strict single-file unified patch application against an exact in-memory draft.
use crate::text_merge::{MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES, TextEditError, checked_lines};

/// Apply one bounded unified patch to an exact UTF-8 draft, without I/O.
///
/// Both headers must name `target` exactly, without timestamps or `a/` prefixes.
/// Only ordered unified hunks and the conventional no-final-newline marker are
/// accepted. Context/removal bytes and both line coordinates must match; no fuzz,
/// offset search, path lookup, cross-file patch, binary patch or shell is used.
/// This produces a new draft only, never remote mutation authority.
///
/// # Errors
/// Rejects input/output budget violations, unsafe headers, unsupported syntax
/// and any exact context/coordinate mismatch before producing a draft.
///
/// # Examples
/// ```
/// let patch = "--- /file\n+++ /file\n@@ -1 +1 @@\n-old\n+new\n";
/// assert_eq!(keelshell_core::apply_text_patch("/file", "old\n", patch)?, "new\n");
/// # Ok::<(), keelshell_core::TextEditError>(())
/// ```
pub fn apply_text_patch(target: &str, draft: &str, patch: &str) -> Result<String, TextEditError> {
    if target.is_empty() || target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(TextEditError::PatchTarget);
    }
    let original = checked_lines(draft)?;
    if patch.len() > 4 * MAX_TEXT_EDIT_BYTES {
        return Err(TextEditError::Limit);
    }
    if patch.contains('\0') {
        return Err(TextEditError::Binary);
    }
    let lines: Vec<_> = patch.split_inclusive('\n').collect();
    if lines.len() > 4 * MAX_TEXT_EDIT_LINES + 1024 || lines.len() < 3 {
        return Err(TextEditError::PatchSyntax);
    }
    if lines[0] != format!("--- {target}\n") || lines[1] != format!("+++ {target}\n") {
        return Err(TextEditError::PatchTarget);
    }
    let mut output: Vec<String> = Vec::new();
    let mut cursor = 0;
    let mut index = 2;
    let mut changed = false;
    let mut prior_start = None;
    while index < lines.len() {
        let (old_start, old_count, new_start, new_count) = hunk_header(lines[index])?;
        let start = coordinate(old_start, old_count)?;
        let expected_new = coordinate(new_start, new_count)?;
        if start < cursor || start > original.len() || prior_start == Some(start) {
            return Err(TextEditError::PatchMismatch);
        }
        output.extend(
            original[cursor..start]
                .iter()
                .map(|line| (*line).to_owned()),
        );
        if output.len() != expected_new {
            return Err(TextEditError::PatchMismatch);
        }
        cursor = start;
        prior_start = Some(start);
        index += 1;
        let mut rows: Vec<(u8, String)> = Vec::new();
        while index < lines.len() && !lines[index].starts_with("@@ ") {
            let line = lines[index];
            if line == "\\ No newline at end of file\n" {
                let previous = rows.last_mut().ok_or(TextEditError::PatchSyntax)?;
                if !previous.1.ends_with('\n') {
                    return Err(TextEditError::PatchSyntax);
                }
                previous.1.pop();
            } else {
                if !line.ends_with('\n')
                    || !matches!(line.as_bytes().first(), Some(b' ' | b'-' | b'+'))
                {
                    return Err(TextEditError::PatchSyntax);
                }
                rows.push((line.as_bytes()[0], line[1..].to_owned()));
            }
            index += 1;
        }
        let old_seen = rows.iter().filter(|(kind, _)| *kind != b'+').count();
        let new_seen = rows.iter().filter(|(kind, _)| *kind != b'-').count();
        if old_seen != old_count || new_seen != new_count || rows.is_empty() {
            return Err(TextEditError::PatchSyntax);
        }
        for (kind, text) in rows {
            if kind != b'+' {
                if original.get(cursor).copied() != Some(text.as_str()) {
                    return Err(TextEditError::PatchMismatch);
                }
                cursor += 1;
            }
            if kind != b'-' {
                output.push(text);
            }
            changed |= kind != b' ';
        }
    }
    if !changed {
        return Err(TextEditError::PatchSyntax);
    }
    output.extend(original[cursor..].iter().map(|line| (*line).to_owned()));
    if output
        .iter()
        .take(output.len().saturating_sub(1))
        .any(|line| !line.ends_with('\n'))
    {
        return Err(TextEditError::PatchSyntax);
    }
    let bytes: usize = output.iter().map(String::len).sum();
    if bytes > MAX_TEXT_EDIT_BYTES {
        return Err(TextEditError::Limit);
    }
    let result = output.concat();
    checked_lines(&result)?;
    Ok(result)
}
fn coordinate(start: usize, count: usize) -> Result<usize, TextEditError> {
    if count == 0 {
        Ok(start)
    } else {
        start.checked_sub(1).ok_or(TextEditError::PatchSyntax)
    }
}
fn hunk_header(line: &str) -> Result<(usize, usize, usize, usize), TextEditError> {
    let range = line
        .strip_prefix("@@ -")
        .and_then(|s| s.strip_suffix(" @@\n"))
        .ok_or(TextEditError::PatchSyntax)?;
    let (old, new) = range.split_once(" +").ok_or(TextEditError::PatchSyntax)?;
    let (old_start, old_count) = range_pair(old)?;
    let (new_start, new_count) = range_pair(new)?;
    if old_count == 0 && new_count == 0 {
        return Err(TextEditError::PatchSyntax);
    }
    Ok((old_start, old_count, new_start, new_count))
}
fn range_pair(value: &str) -> Result<(usize, usize), TextEditError> {
    let (start, count) = value.split_once(',').map_or((value, "1"), |(a, b)| (a, b));
    let parse = |s: &str| {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(TextEditError::PatchSyntax);
        }
        s.parse::<usize>().map_err(|_| TextEditError::PatchSyntax)
    };
    let start = parse(start)?;
    let count = parse(count)?;
    if start > MAX_TEXT_EDIT_LINES || count > MAX_TEXT_EDIT_LINES {
        return Err(TextEditError::Limit);
    }
    Ok((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_patch_preserves_crlf_and_missing_eof() {
        assert_eq!(apply_text_patch("/config","one\r\ntwo", "--- /config\n+++ /config\n@@ -1,2 +1,2 @@\n one\r\n-two\n\\ No newline at end of file\n+中文\n\\ No newline at end of file\n").unwrap_or_else(|error| panic!("{error}")),"one\r\n中文");
    }
    #[test]
    fn additions_deletions_and_ordered_hunks_apply() {
        assert_eq!(
            apply_text_patch(
                "/f",
                "a\nb\nc\nd\n",
                "--- /f\n+++ /f\n@@ -0,0 +1 @@\n+x\n@@ -2 +2,0 @@\n-b\n@@ -4 +4 @@\n-d\n+D\n"
            )
            .unwrap_or_else(|error| panic!("{error}")),
            "x\na\nc\nD\n"
        );
        assert_eq!(
            apply_text_patch("/f", "", "--- /f\n+++ /f\n@@ -0,0 +1 @@\n+x\n")
                .unwrap_or_else(|error| panic!("{error}")),
            "x\n"
        );
    }
    #[test]
    fn mismatch_wrong_coordinates_and_cross_file_are_rejected() {
        for p in [
            "--- /f\n+++ /f\n@@ -1 +1 @@\n-wrong\n+x\n",
            "--- /f\n+++ /f\n@@ -1 +2 @@\n-a\n+x\n",
            "--- /f\n+++ /f\n@@ -1 +1 @@\n-a\n+x\n--- /other\n+++ /other\n@@ -1 +1 @@\n-x\n+y\n",
        ] {
            assert!(apply_text_patch("/f", "a\n", p).is_err());
        }
        assert_eq!(
            apply_text_patch("/f", "a\n", "--- /other\n+++ /other\n@@ -1 +1 @@\n-a\n+x\n"),
            Err(TextEditError::PatchTarget)
        );
    }
    #[test]
    fn malformed_counts_markers_and_binary_are_rejected() {
        assert_eq!(
            apply_text_patch("/f", "a", "\0"),
            Err(TextEditError::Binary)
        );
        for p in [
            "--- /f\n+++ /f\n@@ -1,2 +1 @@\n-a\n+x\n",
            "--- /f\n+++ /f\n@@ -1 +1 @@ trailing\n-a\n+x\n",
            "--- /f\n+++ /f\n@@ -1 +1 @@\n\\ No newline at end of file\n-a\n+x\n",
            "--- /f\n+++ /f\n@@ -1 +1 @@\n-a\n+x\n\\ No newline at end of file\n\\ No newline at end of file\n",
        ] {
            assert!(apply_text_patch("/f", "a\n", p).is_err());
        }
    }
    #[test]
    fn generated_lf_unified_diffs_roundtrip_exact_small_document_matrix() {
        let documents = [
            "",
            "a",
            "a\n",
            "a\nb",
            "a\nb\n",
            "b\na\n",
            "a\na\n",
            "x\na\nb\n",
            "a\nx\nb\n",
            "a\nb\nx",
        ];
        for original in documents {
            for updated in documents {
                if original == updated {
                    continue;
                }
                let patch = crate::diff_text(original, updated)
                    .unwrap_or_else(|error| panic!("{error}"))
                    .render("/f", "/f")
                    .unwrap_or_else(|error| panic!("{error}"));
                assert_eq!(
                    apply_text_patch("/f", original, &patch)
                        .unwrap_or_else(|error| panic!("{error}: {patch:?}")),
                    updated
                );
            }
        }
    }
    #[test]
    fn no_final_newline_cannot_create_unreviewed_line_concatenation() {
        assert!(
            apply_text_patch(
                "/f",
                "a\nb\n",
                "--- /f\n+++ /f\n@@ -1 +1 @@\n-a\n+x\n\\ No newline at end of file\n"
            )
            .is_err()
        );
    }
}
