//! Bounded UTF-8 line differences for remote-file review.
//!
//! This module compares text that has already been read by a caller. It does
//! not open files, decode arbitrary byte streams, execute a shell, or apply a
//! patch. The limits are deliberately enforced before the quadratic part of
//! the line matching algorithm starts so a remote file cannot turn a review
//! request into an unbounded CPU or memory operation.

use std::str;

/// Maximum UTF-8 bytes accepted from either side of a comparison.
pub const MAX_DIFF_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of logical lines accepted from either side of a comparison.
pub const MAX_DIFF_LINES: usize = 4_096;
/// Maximum context lines retained around one changed region.
pub const MAX_DIFF_CONTEXT_LINES: usize = 128;
/// Maximum rendered unified-diff bytes, including headers and hunk markers.
pub const MAX_DIFF_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
/// Context used by diff_text and diff_utf8.
pub const DEFAULT_DIFF_CONTEXT_LINES: usize = 3;
const MAX_LABEL_BYTES: usize = 1_024;

/// Identifies which comparison input caused a bounded-input error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSide {
    /// The original text, shown as the removed side of a diff.
    Original,
    /// The updated text, shown as the added side of a diff.
    Updated,
}

/// A safe failure while creating or rendering a line diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DiffError {
    /// An input contains more than MAX_DIFF_INPUT_BYTES UTF-8 bytes.
    #[error("{side:?} diff input exceeds the byte limit")]
    InputTooLarge {
        /// The side whose input was rejected.
        side: DiffSide,
        /// Number of bytes supplied by the caller.
        bytes: usize,
    },
    /// An input contains more than MAX_DIFF_LINES logical lines.
    #[error("{side:?} diff input exceeds the line limit")]
    TooManyLines {
        /// The side whose input was rejected.
        side: DiffSide,
        /// Number of lines counted before the input was rejected.
        lines: usize,
    },
    /// A byte slice was not valid UTF-8.
    #[error("{side:?} diff input is not valid UTF-8")]
    InvalidUtf8 {
        /// The side whose bytes were rejected.
        side: DiffSide,
    },
    /// The requested context would make the algorithm or result unbounded.
    #[error("diff context exceeds the line limit")]
    ContextTooLarge,
    /// A header label contains a newline, a control character, or is too long.
    #[error("diff header label is invalid")]
    InvalidLabel,
    /// The rendered result exceeds MAX_DIFF_OUTPUT_BYTES.
    #[error("rendered unified diff exceeds the byte limit")]
    OutputTooLarge,
}

/// Whether one output line is unchanged, removed, or added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// A line is present on both sides and is included as context.
    Context,
    /// A line is present only in the original text.
    Removed,
    /// A line is present only in the updated text.
    Added,
}

/// One line inside a unified-diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Whether this row is context, removed, or added.
    pub kind: DiffLineKind,
    /// Line content without its line-feed terminator.
    pub text: String,
    /// One-based line number in the original text, if present.
    pub original_line: Option<usize>,
    /// One-based line number in the updated text, if present.
    pub updated_line: Option<usize>,
    /// Whether the source line ended with a line feed.
    pub has_newline: bool,
}

impl DiffLine {
    /// Return true when the row is a context row.
    pub const fn is_context(&self) -> bool {
        matches!(self.kind, DiffLineKind::Context)
    }

    /// Return true when the row is a removed row.
    pub const fn is_removed(&self) -> bool {
        matches!(self.kind, DiffLineKind::Removed)
    }

    /// Return true when the row is an added row.
    pub const fn is_added(&self) -> bool {
        matches!(self.kind, DiffLineKind::Added)
    }
}

/// A contiguous changed region with bounded surrounding context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// One-based original start line. An insertion at the beginning uses zero.
    pub original_start: usize,
    /// Number of original lines represented by this hunk.
    pub original_count: usize,
    /// One-based updated start line. An insertion at the beginning uses one.
    pub updated_start: usize,
    /// Number of updated lines represented by this hunk.
    pub updated_count: usize,
    /// Context, removed, and added rows in unified-diff order.
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    /// Return true when this hunk contains at least one change.
    pub fn is_changed(&self) -> bool {
        self.lines.iter().any(|line| !line.is_context())
    }
}

/// The transport-independent result of a bounded line comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedDiff {
    original_line_count: usize,
    updated_line_count: usize,
    hunks: Vec<DiffHunk>,
}

impl UnifiedDiff {
    /// Number of logical lines in the original input.
    pub const fn original_line_count(&self) -> usize {
        self.original_line_count
    }

    /// Number of logical lines in the updated input.
    pub const fn updated_line_count(&self) -> usize {
        self.updated_line_count
    }

    /// All changed hunks, in source order. Equal inputs have no hunks.
    pub fn hunks(&self) -> &[DiffHunk] {
        &self.hunks
    }

    /// Return true when at least one line or line terminator changed.
    pub fn is_changed(&self) -> bool {
        !self.hunks.is_empty()
    }

    /// Render the result as a bounded unified-diff document.
    ///
    /// Labels are display labels only. They are not paths and are never opened.
    /// Labels must be at most 1,024 UTF-8 bytes and contain no controls or line
    /// breaks. The output uses LF line endings and emits the conventional
    /// no-final-newline marker when a source line did not end in a line feed.
    ///
    /// # Errors
    ///
    /// Returns InvalidLabel when either label is unsafe, or OutputTooLarge when
    /// the rendered document exceeds the output bound.
    pub fn render(&self, original_label: &str, updated_label: &str) -> Result<String, DiffError> {
        validate_label(original_label)?;
        validate_label(updated_label)?;
        if self.hunks.is_empty() {
            return Ok(String::new());
        }

        let mut output = String::new();
        push_rendered(&mut output, "--- ")?;
        push_rendered(&mut output, original_label)?;
        push_rendered(&mut output, "\n+++ ")?;
        push_rendered(&mut output, updated_label)?;
        push_rendered(&mut output, "\n")?;
        for hunk in &self.hunks {
            push_rendered(&mut output, "@@ -")?;
            push_rendered(
                &mut output,
                &format_range(hunk.original_start, hunk.original_count),
            )?;
            push_rendered(&mut output, " +")?;
            push_rendered(
                &mut output,
                &format_range(hunk.updated_start, hunk.updated_count),
            )?;
            push_rendered(&mut output, " @@\n")?;
            for line in &hunk.lines {
                let marker = match line.kind {
                    DiffLineKind::Context => ' ',
                    DiffLineKind::Removed => '-',
                    DiffLineKind::Added => '+',
                };
                push_rendered(&mut output, &marker.to_string())?;
                push_rendered(&mut output, &line.text)?;
                push_rendered(&mut output, "\n")?;
                if !line.has_newline {
                    push_rendered(&mut output, "\\ No newline at end of file\n")?;
                }
            }
        }
        Ok(output)
    }
}

/// Compare two UTF-8 documents using three context lines around each change.
///
/// # Examples
///
/// ```
/// use keelshell_core::diff_text;
/// let diff = diff_text("before\n", "after\n")?;
/// assert!(diff.is_changed());
/// # Ok::<(), keelshell_core::DiffError>(())
/// ```
///
/// # Errors
///
/// Returns a bounded-input or line-count error when either document is too
/// large. The default context is always within the context bound.
pub fn diff_text(original: &str, updated: &str) -> Result<UnifiedDiff, DiffError> {
    diff_text_with_context(original, updated, DEFAULT_DIFF_CONTEXT_LINES)
}

/// Compare two UTF-8 documents with an explicit, bounded context size.
///
/// # Errors
///
/// Returns a bounded-input, line-count, or ContextTooLarge error when the
/// requested comparison cannot be handled within the domain limits.
pub fn diff_text_with_context(
    original: &str,
    updated: &str,
    context_lines: usize,
) -> Result<UnifiedDiff, DiffError> {
    validate_input(original, DiffSide::Original)?;
    validate_input(updated, DiffSide::Updated)?;
    if context_lines > MAX_DIFF_CONTEXT_LINES {
        return Err(DiffError::ContextTooLarge);
    }

    let original_lines = split_lines(original, DiffSide::Original)?;
    let updated_lines = split_lines(updated, DiffSide::Updated)?;
    let mut operations = Vec::new();
    lcs_diff(&original_lines, &updated_lines, &mut operations);
    let hunks = make_hunks(&original_lines, &updated_lines, &operations, context_lines);
    Ok(UnifiedDiff {
        original_line_count: original_lines.len(),
        updated_line_count: updated_lines.len(),
        hunks,
    })
}

/// Compare byte slices after requiring valid UTF-8 on both sides.
///
/// # Errors
///
/// Returns InvalidUtf8 for malformed input, or a bounded-input/line-count
/// error when a valid document exceeds the domain limits.
pub fn diff_utf8(original: &[u8], updated: &[u8]) -> Result<UnifiedDiff, DiffError> {
    validate_input_bytes(original, DiffSide::Original)?;
    validate_input_bytes(updated, DiffSide::Updated)?;
    let original = str::from_utf8(original).map_err(|_| DiffError::InvalidUtf8 {
        side: DiffSide::Original,
    })?;
    let updated = str::from_utf8(updated).map_err(|_| DiffError::InvalidUtf8 {
        side: DiffSide::Updated,
    })?;
    diff_text(original, updated)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceLine {
    text: String,
    has_newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Equal { original: usize, updated: usize },
    Remove { original: usize },
    Add { updated: usize },
}

fn validate_input(input: &str, side: DiffSide) -> Result<(), DiffError> {
    validate_input_bytes(input.as_bytes(), side)
}

fn validate_input_bytes(input: &[u8], side: DiffSide) -> Result<(), DiffError> {
    if input.len() > MAX_DIFF_INPUT_BYTES {
        return Err(DiffError::InputTooLarge {
            side,
            bytes: input.len(),
        });
    }
    Ok(())
}

fn split_lines(input: &str, side: DiffSide) -> Result<Vec<SourceLine>, DiffError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in input.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        push_source_line(&mut lines, &input[start..index], true, side)?;
        start = index + 1;
    }
    if start < input.len() {
        push_source_line(&mut lines, &input[start..], false, side)?;
    }
    Ok(lines)
}

fn push_source_line(
    lines: &mut Vec<SourceLine>,
    raw: &str,
    has_newline: bool,
    side: DiffSide,
) -> Result<(), DiffError> {
    if lines.len() == MAX_DIFF_LINES {
        return Err(DiffError::TooManyLines {
            side,
            lines: lines.len() + 1,
        });
    }
    // Treat CRLF and LF as the same logical line while retaining lone CR as
    // content. The unified renderer always emits LF, independent of source
    // platform, so the result is deterministic across hosts.
    let text = raw.strip_suffix('\r').unwrap_or(raw).to_owned();
    lines.push(SourceLine { text, has_newline });
    Ok(())
}

fn lcs_diff(original: &[SourceLine], updated: &[SourceLine], output: &mut Vec<Operation>) {
    if original.is_empty() {
        output.extend((0..updated.len()).map(|updated| Operation::Add { updated }));
        return;
    }
    if updated.is_empty() {
        output.extend((0..original.len()).map(|original| Operation::Remove { original }));
        return;
    }
    if original.len() == 1 {
        if let Some(updated_index) = updated.iter().position(|line| line == &original[0]) {
            output.extend((0..updated_index).map(|updated| Operation::Add { updated }));
            output.push(Operation::Equal {
                original: 0,
                updated: updated_index,
            });
            output.extend(
                ((updated_index + 1)..updated.len()).map(|updated| Operation::Add { updated }),
            );
        } else {
            output.push(Operation::Remove { original: 0 });
            output.extend((0..updated.len()).map(|updated| Operation::Add { updated }));
        }
        return;
    }
    if updated.len() == 1 {
        if let Some(original_index) = original.iter().position(|line| line == &updated[0]) {
            output.extend((0..original_index).map(|original| Operation::Remove { original }));
            output.push(Operation::Equal {
                original: original_index,
                updated: 0,
            });
            output.extend(
                ((original_index + 1)..original.len())
                    .map(|original| Operation::Remove { original }),
            );
        } else {
            output.extend((0..original.len()).map(|original| Operation::Remove { original }));
            output.push(Operation::Add { updated: 0 });
        }
        return;
    }

    let original_mid = original.len() / 2;
    let left = lcs_lengths(&original[..original_mid], updated);
    let right = lcs_lengths_reversed(&original[original_mid..], updated);
    let mut best_length = 0;
    let mut updated_mid = 0;
    for index in 0..=updated.len() {
        let score = left[index] + right[updated.len() - index];
        if score > best_length {
            best_length = score;
            updated_mid = index;
        }
    }
    if best_length == 0 {
        output.extend((0..original.len()).map(|original| Operation::Remove { original }));
        output.extend((0..updated.len()).map(|updated| Operation::Add { updated }));
        return;
    }
    lcs_diff(&original[..original_mid], &updated[..updated_mid], output);
    let left_original = original_mid;
    let left_updated = updated_mid;
    let mut right_operations = Vec::new();
    lcs_diff(
        &original[left_original..],
        &updated[left_updated..],
        &mut right_operations,
    );
    output.extend(
        right_operations
            .into_iter()
            .map(|operation| match operation {
                Operation::Equal { original, updated } => Operation::Equal {
                    original: original + left_original,
                    updated: updated + left_updated,
                },
                Operation::Remove { original } => Operation::Remove {
                    original: original + left_original,
                },
                Operation::Add { updated } => Operation::Add {
                    updated: updated + left_updated,
                },
            }),
    );
}

fn lcs_lengths(original: &[SourceLine], updated: &[SourceLine]) -> Vec<usize> {
    let mut previous = vec![0; updated.len() + 1];
    let mut current = vec![0; updated.len() + 1];
    for original_line in original {
        current[0] = 0;
        for (updated_index, updated_line) in updated.iter().enumerate() {
            current[updated_index + 1] = if original_line == updated_line {
                previous[updated_index] + 1
            } else {
                current[updated_index].max(previous[updated_index + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous
}

fn lcs_lengths_reversed(original: &[SourceLine], updated: &[SourceLine]) -> Vec<usize> {
    // Keep the prefix table in reverse-Y coordinates. The caller indexes it
    // with `updated.len() - split`, which converts a split in the original Y
    // sequence into the corresponding suffix length.
    let mut previous = vec![0; updated.len() + 1];
    let mut current = vec![0; updated.len() + 1];
    for original_line in original.iter().rev() {
        current[0] = 0;
        for (updated_index, updated_line) in updated.iter().rev().enumerate() {
            current[updated_index + 1] = if original_line == updated_line {
                previous[updated_index] + 1
            } else {
                current[updated_index].max(previous[updated_index + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous
}

fn make_hunks(
    original: &[SourceLine],
    updated: &[SourceLine],
    operations: &[Operation],
    context_lines: usize,
) -> Vec<DiffHunk> {
    let changed_indices: Vec<usize> = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            (!matches!(operation, Operation::Equal { .. })).then_some(index)
        })
        .collect();
    if changed_indices.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = changed_indices[0].saturating_sub(context_lines);
    let mut end = (changed_indices[0] + context_lines + 1).min(operations.len());
    for &change in &changed_indices[1..] {
        if change <= end + context_lines {
            end = (change + context_lines + 1).min(operations.len());
        } else {
            ranges.push((start, end));
            start = change.saturating_sub(context_lines);
            end = (change + context_lines + 1).min(operations.len());
        }
    }
    ranges.push((start, end));

    let mut hunks = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        let mut lines = Vec::with_capacity(end - start);
        let mut original_line = 0;
        let mut updated_line = 0;
        for operation in &operations[..start] {
            match operation {
                Operation::Equal { .. } | Operation::Remove { .. } => original_line += 1,
                Operation::Add { .. } => {}
            }
            match operation {
                Operation::Equal { .. } | Operation::Add { .. } => updated_line += 1,
                Operation::Remove { .. } => {}
            }
        }
        let original_before = original_line;
        let updated_before = updated_line;
        for operation in &operations[start..end] {
            match *operation {
                Operation::Equal {
                    original: old,
                    updated: new,
                } => {
                    original_line += 1;
                    updated_line += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        text: original[old].text.clone(),
                        original_line: Some(original_line),
                        updated_line: Some(updated_line),
                        has_newline: original[old].has_newline,
                    });
                    debug_assert_eq!(original[old], updated[new]);
                }
                Operation::Remove { original: old } => {
                    original_line += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        text: original[old].text.clone(),
                        original_line: Some(original_line),
                        updated_line: None,
                        has_newline: original[old].has_newline,
                    });
                }
                Operation::Add { updated: new } => {
                    updated_line += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Added,
                        text: updated[new].text.clone(),
                        original_line: None,
                        updated_line: Some(updated_line),
                        has_newline: updated[new].has_newline,
                    });
                }
            }
        }
        let original_count = lines
            .iter()
            .filter(|line| line.original_line.is_some())
            .count();
        let updated_count = lines
            .iter()
            .filter(|line| line.updated_line.is_some())
            .count();
        hunks.push(DiffHunk {
            original_start: if original_count == 0 {
                original_before
            } else {
                original_before + 1
            },
            original_count,
            updated_start: if updated_count == 0 {
                updated_before
            } else {
                updated_before + 1
            },
            updated_count,
            lines,
        });
    }
    hunks
}

fn validate_label(label: &str) -> Result<(), DiffError> {
    if label.is_empty()
        || label.len() > MAX_LABEL_BYTES
        || label.chars().any(|character| character.is_control())
    {
        return Err(DiffError::InvalidLabel);
    }
    Ok(())
}

fn push_rendered(output: &mut String, value: &str) -> Result<(), DiffError> {
    let next = output
        .len()
        .checked_add(value.len())
        .ok_or(DiffError::OutputTooLarge)?;
    if next > MAX_DIFF_OUTPUT_BYTES {
        return Err(DiffError::OutputTooLarge);
    }
    output.push_str(value);
    Ok(())
}

fn format_range(start: usize, count: usize) -> String {
    if count == 1 {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn equal_text_has_no_hunks_or_rendered_output() -> TestResult {
        let diff = diff_text("a\n中文\n", "a\n中文\n")?;
        assert!(!diff.is_changed());
        assert!(diff.hunks().is_empty());
        assert_eq!(diff.render("old", "new")?, "");
        Ok(())
    }

    #[test]
    fn replacement_has_stable_hunk_and_line_numbers() -> TestResult {
        let diff = diff_text("one\ntwo\nthree\n", "one\nTWO\nthree\n")?;
        assert_eq!(diff.hunks().len(), 1);
        let hunk = &diff.hunks()[0];
        assert_eq!((hunk.original_start, hunk.original_count), (1, 3));
        assert_eq!((hunk.updated_start, hunk.updated_count), (1, 3));
        assert_eq!(hunk.lines[1].kind, DiffLineKind::Removed);
        assert_eq!(hunk.lines[1].original_line, Some(2));
        assert_eq!(hunk.lines[2].kind, DiffLineKind::Added);
        assert_eq!(hunk.lines[2].updated_line, Some(2));
        assert_eq!(
            diff.render("old.txt", "new.txt")?,
            "--- old.txt\n+++ new.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n"
        );
        Ok(())
    }

    #[test]
    fn insertion_and_deletion_at_boundaries_use_unified_zero_ranges() -> TestResult {
        let insertion = diff_text_with_context("b\n", "a\nb\n", 0)?;
        assert_eq!(
            insertion.render("a", "b")?,
            "--- a\n+++ b\n@@ -0,0 +1 @@\n+a\n"
        );
        let deletion = diff_text_with_context("a\nb\n", "b\n", 0)?;
        assert_eq!(
            deletion.render("a", "b")?,
            "--- a\n+++ b\n@@ -1 +0,0 @@\n-a\n"
        );
        Ok(())
    }

    #[test]
    fn unchanged_regions_are_split_into_bounded_context_hunks() -> TestResult {
        let original = "a\nb\nc\nd\ne\nf\ng\nh\ni\n";
        let updated = "a\nB\nc\nd\ne\nf\nG\nh\ni\n";
        let diff = diff_text_with_context(original, updated, 1)?;
        assert_eq!(diff.hunks().len(), 2);
        assert_eq!(
            (
                diff.hunks()[0].original_start,
                diff.hunks()[0].original_count
            ),
            (1, 3)
        );
        assert_eq!(
            (
                diff.hunks()[1].original_start,
                diff.hunks()[1].original_count
            ),
            (6, 3)
        );
        Ok(())
    }

    #[test]
    fn crlf_is_compared_as_a_line_ending_and_missing_final_newline_is_reported() -> TestResult {
        let equal = diff_text("中文\r\n", "中文\n")?;
        assert!(!equal.is_changed());
        let missing = diff_text("中文\n", "中文")?;
        assert_eq!(
            missing.render("old", "new")?,
            "--- old\n+++ new\n@@ -1 +1 @@\n-中文\n+中文\n\\ No newline at end of file\n"
        );
        Ok(())
    }

    #[test]
    fn utf8_bytes_and_rejections_are_bounded() {
        assert!(matches!(
            diff_utf8(b"ok", &[0xff]),
            Err(DiffError::InvalidUtf8 {
                side: DiffSide::Updated
            })
        ));
        let oversized = "x".repeat(MAX_DIFF_INPUT_BYTES + 1);
        assert!(matches!(
            diff_text(&oversized, ""),
            Err(DiffError::InputTooLarge {
                side: DiffSide::Original,
                ..
            })
        ));
        let too_many_lines = "x\n".repeat(MAX_DIFF_LINES + 1);
        assert!(matches!(
            diff_text(&too_many_lines, ""),
            Err(DiffError::TooManyLines {
                side: DiffSide::Original,
                ..
            })
        ));
    }

    #[test]
    fn labels_and_context_are_checked() -> TestResult {
        let diff = diff_text("a", "b")?;
        assert!(matches!(
            diff.render("old\nunsafe", "new"),
            Err(DiffError::InvalidLabel)
        ));
        assert!(matches!(
            diff_text_with_context("a", "b", MAX_DIFF_CONTEXT_LINES + 1),
            Err(DiffError::ContextTooLarge)
        ));
        Ok(())
    }
}
