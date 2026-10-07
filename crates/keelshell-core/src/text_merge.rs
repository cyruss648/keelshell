//! Bounded, byte-preserving three-way text merge. No filesystem or network I/O.
use std::ops::Range;

/// Maximum bytes in each document or completed merge.
pub const MAX_TEXT_EDIT_BYTES: usize = 1024 * 1024;
/// Maximum logical document lines admitted by merge and patch application.
pub const MAX_TEXT_EDIT_LINES: usize = 4096;

/// A rejected text edit, before any mutation is authorized.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextEditError {
    /// A document exceeds the bounded editor budget.
    #[error("text exceeds the 1 MiB or 4096-line edit budget")]
    Limit,
    /// Text contains a NUL byte, which the text editor cannot safely represent.
    #[error("text contains a NUL byte")]
    Binary,
    /// A conflict has no explicit resolution.
    #[error("resolve every text conflict before adopting the merge")]
    Unresolved,
    /// A supplied choice refers to no conflict.
    #[error("merge resolution count differs from the reviewed conflicts")]
    Choices,
    /// Unified patch syntax is unsupported or malformed.
    #[error("malformed or unsupported single-file unified patch")]
    PatchSyntax,
    /// A patch header addresses another file.
    #[error("patch headers must name the exact selected remote file")]
    PatchTarget,
    /// A context/deletion line or range does not match the exact draft.
    #[error("patch context or line range does not match the exact draft")]
    PatchMismatch,
}

/// One overlapping region that requires an explicit choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMergeConflict {
    /// Zero-based base line range affected by this conflict.
    pub base_lines: Range<usize>,
    /// Exact baseline text in that range.
    pub base: String,
    /// Exact result from the local draft in that range.
    pub draft: String,
    /// Exact result from the current remote document in that range.
    pub remote: String,
}

/// Explicit resolution of one immutable conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextMergeChoice {
    /// Keep the local draft's change for this region.
    Draft,
    /// Keep the current remote change for this region.
    Remote,
    /// Use the complete user-edited replacement for this region.
    Manual(String),
}

#[derive(Debug, Clone)]
enum Segment {
    Text(String),
    Conflict(usize),
}

/// Immutable three-way comparison; callers retain their original draft separately.
#[derive(Debug, Clone)]
pub struct TextMergePlan {
    segments: Vec<Segment>,
    conflicts: Vec<TextMergeConflict>,
}
impl TextMergePlan {
    /// Conflicts in base order. Non-overlapping changes are already combined.
    pub fn conflicts(&self) -> &[TextMergeConflict] {
        &self.conflicts
    }

    /// Produce the exact final draft only after every conflict has a choice.
    /// The caller still must display/review the full result before remote writes.
    pub fn resolve(&self, choices: &[Option<TextMergeChoice>]) -> Result<String, TextEditError> {
        if choices.len() != self.conflicts.len() {
            return Err(TextEditError::Choices);
        }
        let mut result = String::new();
        for segment in &self.segments {
            let text = match segment {
                Segment::Text(text) => text,
                Segment::Conflict(index) => match choices[*index].as_ref() {
                    Some(TextMergeChoice::Draft) => &self.conflicts[*index].draft,
                    Some(TextMergeChoice::Remote) => &self.conflicts[*index].remote,
                    Some(TextMergeChoice::Manual(text)) => text,
                    None => return Err(TextEditError::Unresolved),
                },
            };
            if result.len().saturating_add(text.len()) > MAX_TEXT_EDIT_BYTES {
                return Err(TextEditError::Limit);
            }
            result.push_str(text);
        }
        checked_lines(&result)?;
        Ok(result)
    }
}

/// Compare exact UTF-8 documents, preserving CRLF and missing final newlines.
/// The bounded quadratic line matching happens entirely in memory. Desktop callers
/// run this function on their background worker, never on the render thread.
/// Insertion at the same position or an edited boundary conservatively conflicts.
///
/// # Errors
/// Rejects oversized or NUL-containing documents before line matching.
///
/// # Examples
/// ```
/// let plan = keelshell_core::merge_text("a\nb\n", "A\nb\n", "a\nB\n")?;
/// assert_eq!(plan.resolve(&[])?, "A\nB\n");
/// # Ok::<(), keelshell_core::TextEditError>(())
/// ```
pub fn merge_text(base: &str, draft: &str, remote: &str) -> Result<TextMergePlan, TextEditError> {
    let base = checked_lines(base)?;
    let draft = checked_lines(draft)?;
    let remote = checked_lines(remote)?;
    let local = edits(&base, &draft);
    let distant = edits(&base, &remote);
    let mut all: Vec<(bool, &Edit)> = local
        .iter()
        .map(|e| (true, e))
        .chain(distant.iter().map(|e| (false, e)))
        .collect();
    all.sort_by_key(|(_, edit)| (edit.range.start, edit.range.end));
    let mut segments = Vec::new();
    let mut conflicts = Vec::new();
    let mut cursor = 0;
    let mut index = 0;
    while index < all.len() {
        let start = all[index].1.range.start;
        let mut end = all[index].1.range.end;
        let first = index;
        index += 1;
        while index < all.len() {
            let next = &all[index].1.range;
            if next.start < end || (next.start == end && (next.is_empty() || start == end)) {
                end = end.max(next.end);
                index += 1;
            } else {
                break;
            }
        }
        segments.push(Segment::Text(base[cursor..start].concat()));
        let cluster = &all[first..index];
        let local: Vec<&Edit> = cluster
            .iter()
            .filter_map(|(local, e)| local.then_some(*e))
            .collect();
        let remote: Vec<&Edit> = cluster
            .iter()
            .filter_map(|(local, e)| (!local).then_some(*e))
            .collect();
        let draft = apply_region(&base, start..end, &local);
        let distant = apply_region(&base, start..end, &remote);
        if local.is_empty() {
            segments.push(Segment::Text(distant));
        } else if remote.is_empty() || draft == distant {
            segments.push(Segment::Text(draft));
        } else {
            segments.push(Segment::Conflict(conflicts.len()));
            conflicts.push(TextMergeConflict {
                base_lines: start..end,
                base: base[start..end].concat(),
                draft,
                remote: distant,
            });
        }
        cursor = end;
    }
    segments.push(Segment::Text(base[cursor..].concat()));
    Ok(TextMergePlan {
        segments,
        conflicts,
    })
}

pub(crate) fn checked_lines(text: &str) -> Result<Vec<&str>, TextEditError> {
    if text.len() > MAX_TEXT_EDIT_BYTES {
        return Err(TextEditError::Limit);
    }
    if text.contains('\0') {
        return Err(TextEditError::Binary);
    }
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    if lines.len() > MAX_TEXT_EDIT_LINES {
        return Err(TextEditError::Limit);
    }
    Ok(lines)
}
struct Edit {
    range: Range<usize>,
    replacement: String,
}
fn apply_region(base: &[&str], region: Range<usize>, edits: &[&Edit]) -> String {
    let mut result = String::new();
    let mut cursor = region.start;
    for edit in edits {
        result.push_str(&base[cursor..edit.range.start].concat());
        result.push_str(&edit.replacement);
        cursor = edit.range.end;
    }
    result.push_str(&base[cursor..region.end].concat());
    result
}
fn edits(base: &[&str], updated: &[&str]) -> Vec<Edit> {
    // u16 safely holds the admitted 4096-line LCS; one table is released before
    // the second comparison. At the maximum input this owns about 32 MiB.
    let width = updated.len() + 1;
    let mut lengths = vec![0_u16; (base.len() + 1) * width];
    for old in (0..base.len()).rev() {
        for new in (0..updated.len()).rev() {
            lengths[old * width + new] = if base[old] == updated[new] {
                1 + lengths[(old + 1) * width + new + 1]
            } else {
                lengths[(old + 1) * width + new].max(lengths[old * width + new + 1])
            };
        }
    }
    let mut result = Vec::new();
    let (mut old, mut new) = (0, 0);
    while old < base.len() || new < updated.len() {
        if old < base.len() && new < updated.len() && base[old] == updated[new] {
            old += 1;
            new += 1;
            continue;
        }
        let start = old;
        let mut replacement = String::new();
        while old < base.len() || new < updated.len() {
            if old < base.len() && new < updated.len() && base[old] == updated[new] {
                break;
            }
            if new < updated.len()
                && (old == base.len()
                    || lengths[old * width + new + 1] > lengths[(old + 1) * width + new])
            {
                replacement.push_str(updated[new]);
                new += 1;
            } else {
                old += 1;
            }
        }
        result.push(Edit {
            range: start..old,
            replacement,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_adjacent_edits_merge_without_conflict() {
        let p = merge_text("a\nb\nc\n", "A\nb\nc\n", "a\nB\nc\n")
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(p.conflicts().is_empty());
        assert_eq!(
            p.resolve(&[]).unwrap_or_else(|error| panic!("{error}")),
            "A\nB\nc\n"
        );
    }
    #[test]
    fn same_edit_and_unchanged_sides_do_not_conflict() {
        for (draft, remote, expected) in [
            ("A\n", "A\n", "A\n"),
            ("a\n", "A\n", "A\n"),
            ("A\n", "a\n", "A\n"),
        ] {
            assert_eq!(
                merge_text("a\n", draft, remote)
                    .unwrap_or_else(|error| panic!("{error}"))
                    .resolve(&[])
                    .unwrap_or_else(|error| panic!("{error}")),
                expected
            );
        }
    }
    #[test]
    fn conflict_requires_explicit_choice_and_manual_text() {
        let p = merge_text("a\nb\n", "a\nLOCAL\n", "a\nREMOTE\n")
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(p.conflicts()[0].base, "b\n");
        assert_eq!(p.resolve(&[None]), Err(TextEditError::Unresolved));
        assert_eq!(
            p.resolve(&[Some(TextMergeChoice::Manual("中文\r\n".into()))])
                .unwrap_or_else(|error| panic!("{error}")),
            "a\n中文\r\n"
        );
        assert_eq!(
            p.resolve(&[Some(TextMergeChoice::Remote)])
                .unwrap_or_else(|error| panic!("{error}")),
            "a\nREMOTE\n"
        );
    }
    #[test]
    fn simultaneous_insertions_and_delete_edit_are_conflicts() {
        for (base, draft, remote) in [
            ("", "local\n", "remote\n"),
            ("a\nb\n", "a\n", "a\nB\n"),
            ("a\n", "x\na\n", "y\na\n"),
        ] {
            assert_eq!(
                merge_text(base, draft, remote)
                    .unwrap_or_else(|error| panic!("{error}"))
                    .conflicts()
                    .len(),
                1
            );
        }
    }
    #[test]
    fn exact_line_endings_and_eof_are_preserved() {
        let p =
            merge_text("a\r\nb", "A\r\nb", "a\r\nb\n").unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            p.resolve(&[]).unwrap_or_else(|error| panic!("{error}")),
            "A\r\nb\n"
        );
    }
    #[test]
    fn small_document_matrix_preserves_every_exact_one_sided_change() {
        let documents = [
            "",
            "a",
            "a\n",
            "a\r\n",
            "a\nb",
            "a\nb\n",
            "b\na\n",
            "a\na\n",
            "x\na\nb\n",
            "a\nx\nb\n",
            "a\nb\nx",
        ];
        for base in documents {
            for updated in documents {
                assert_eq!(
                    merge_text(base, updated, base)
                        .unwrap_or_else(|error| panic!("{error}"))
                        .resolve(&[])
                        .unwrap_or_else(|error| panic!("{error}")),
                    updated
                );
                assert_eq!(
                    merge_text(base, base, updated)
                        .unwrap_or_else(|error| panic!("{error}"))
                        .resolve(&[])
                        .unwrap_or_else(|error| panic!("{error}")),
                    updated
                );
                assert_eq!(
                    merge_text(base, updated, updated)
                        .unwrap_or_else(|error| panic!("{error}"))
                        .resolve(&[])
                        .unwrap_or_else(|error| panic!("{error}")),
                    updated
                );
            }
        }
    }
    #[test]
    fn bounded_inputs_outputs_and_nul_fail_closed() {
        assert_eq!(
            merge_text("", &"a\n".repeat(MAX_TEXT_EDIT_LINES + 1), "")
                .err()
                .unwrap_or_else(|| panic!("expected rejection")),
            TextEditError::Limit
        );
        let p = merge_text("a", "b", "c").unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            p.resolve(&[Some(TextMergeChoice::Manual("\0".into()))]),
            Err(TextEditError::Binary)
        );
        assert_eq!(p.resolve(&[]), Err(TextEditError::Choices));
        assert_eq!(
            p.resolve(&[Some(TextMergeChoice::Manual(
                "x".repeat(MAX_TEXT_EDIT_BYTES + 1)
            ))]),
            Err(TextEditError::Limit)
        );
    }
}
