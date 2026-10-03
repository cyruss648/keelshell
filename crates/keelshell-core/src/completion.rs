//! Bounded shell-word completion without expansion, I/O, or command execution.

use std::ops::Range;

mod lexer;
#[cfg(test)]
mod tests;

/// Maximum UTF-8 input and edited command size accepted by the completion model.
pub const MAX_COMPLETION_INPUT_BYTES: usize = 65_536;
/// Maximum remote path size in UTF-8 bytes, including any retained path suffix.
pub const MAX_COMPLETION_PATH_BYTES: usize = 4_096;
const MAX_NAME_BYTES: usize = 1_024;

/// A literal remote lookup; no field is shell source to evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionQuery {
    /// Look up executable names in the remote command catalogue.
    Commands {
        /// Case-sensitive literal name prefix, at most 1,024 UTF-8 bytes.
        prefix: String,
    },
    /// List one absolute remote directory and filter its immediate entries.
    Paths {
        /// Absolute POSIX path, at most 4,096 UTF-8 bytes; dot components retain
        /// their remote meaning.
        directory: String,
        /// Case-sensitive literal basename prefix, at most 1,024 UTF-8 bytes.
        prefix: String,
        /// Whether a following path suffix or the command requires a directory.
        directories_only: bool,
    },
}

/// Either a safe completion plan or a normal, explicitly unsupported context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionAnalysis {
    /// A word can be completed without evaluating shell syntax.
    Ready(CompletionPlan),
    /// Leave the command unchanged and explain why completion is unavailable.
    Unsupported(CompletionUnsupported),
}

/// Shell contexts for which a literal lookup cannot be planned reliably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompletionUnsupported {
    /// The cursor is inside a shell comment.
    #[error("the cursor is in a shell comment")]
    Comment,
    /// Expansion would be required, and this model never evaluates it.
    #[error("shell expansions are not evaluated for completion")]
    Expansion,
    /// A compound command, here-document or unsupported operator was encountered.
    #[error("this shell syntax is not supported for completion")]
    ComplexSyntax,
    /// The cursor splits a backslash escape, or the escape is incomplete.
    #[error("the cursor is inside an incomplete shell escape")]
    EscapeBoundary,
    /// An unfinished quote has text beyond the cursor that cannot safely be replaced.
    #[error("an unfinished quote continues beyond the cursor")]
    IncompleteQuote,
    /// Completion is not defined for an assignment's variable name.
    #[error("the cursor is in an assignment name")]
    AssignmentName,
    /// File descriptor duplication does not take a filename.
    #[error("file descriptor operands are not filesystem paths")]
    FileDescriptor,
    /// The cursor is in an operator or the simple-command grammar is incomplete.
    #[error("the cursor is in an unsupported shell grammar position")]
    Grammar,
    /// A literal word contains controls that cannot be presented as a safe candidate.
    #[error("the word contains unsupported control characters")]
    ControlCharacter,
}

/// Invalid arguments, stale plans or remote candidates that fail validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompletionError {
    /// The input, query prefix or replacement exceeds its bounded size.
    #[error("completion input or query exceeds the size limit")]
    InputTooLong,
    /// The cursor is out of bounds or splits a UTF-8 character.
    #[error("completion cursor is not a UTF-8 boundary")]
    InvalidCursor,
    /// Relative paths need an explicitly supplied remote completion directory.
    #[error("an explicit remote completion directory is required")]
    MissingBaseDirectory,
    /// A remote directory is not an acceptable absolute POSIX path.
    #[error("invalid absolute remote completion directory")]
    InvalidDirectory,
    /// A path result has not been bound to the exact request and resolved directory.
    #[error("completion result directory does not match its query")]
    DirectoryMismatch,
    /// The command text or cursor changed after analysis.
    #[error("completion input or cursor changed")]
    StaleInput,
    /// A candidate is unsafe, mismatched, or outside the queried directory.
    #[error("completion candidate does not match the literal query")]
    InvalidCandidate,
}

/// A transport-independent candidate containing only literal remote metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralCandidate {
    /// One nonempty basename, at most 1,024 UTF-8 bytes, without slash or
    /// dot/dot-dot entries. Controls, U+FFFD, directional and selected zero-width
    /// characters are rejected, including U+200D used in joined emoji names.
    pub name: String,
    /// Absolute source path, at most 4,096 UTF-8 bytes, formed from a canonical
    /// parent and this exact basename. The entry itself may be a symlink;
    /// commands insert only their basename.
    pub path: String,
    /// Whether the remote entry resolves to a directory.
    pub is_directory: bool,
}

/// An edit against the original UTF-8 input, applied without executing anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionEdit {
    /// Byte range in the original input to replace; outside text remains unchanged.
    pub range: Range<usize>,
    /// A safely quoted shell word or assignment value.
    pub replacement: String,
    /// Absolute UTF-8 byte cursor offset in the edited input.
    pub caret_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WordQuery {
    Commands {
        prefix: String,
        suffix: String,
    },
    Paths {
        directory: String,
        prefix: String,
        suffix: String,
        tail: String,
        directories_only: bool,
    },
}

/// A bounded snapshot of one literal shell word and its completion context.
///
/// This plan owns no transport. Call [`Self::query`] with an explicit base, then
/// bind path results using [`Self::bind_resolved_directory`]. UI callers must
/// additionally bind the request to their session, selection and IME state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionPlan {
    source: String,
    caret: usize,
    range: Range<usize>,
    word: WordQuery,
    requested_directory: Option<String>,
    resolved_directory: Option<String>,
}

impl CompletionPlan {
    /// Build a pure lookup and remember its exact absolute directory.
    ///
    /// Relative words require an explicit remote base. Neither the local working
    /// directory, the SSH shell working directory nor HOME is inferred. Dot
    /// components are not collapsed because remote symlinks affect their meaning.
    /// A successful new query invalidates any previous resolved-directory binding.
    pub fn query(&mut self, base: Option<&str>) -> Result<CompletionQuery, CompletionError> {
        let (WordQuery::Commands { prefix, .. } | WordQuery::Paths { prefix, .. }) = &self.word;
        if prefix.len() > MAX_NAME_BYTES {
            return Err(CompletionError::InputTooLong);
        }
        match &self.word {
            WordQuery::Commands { prefix, .. } => Ok(CompletionQuery::Commands {
                prefix: prefix.clone(),
            }),
            WordQuery::Paths {
                directory,
                prefix,
                directories_only,
                ..
            } => {
                let directory = if directory.starts_with('/') {
                    directory.clone()
                } else {
                    let base = base.ok_or(CompletionError::MissingBaseDirectory)?;
                    if !valid_absolute_path(base) {
                        return Err(CompletionError::InvalidDirectory);
                    }
                    if directory.is_empty() {
                        base.to_owned()
                    } else {
                        join_path(base, directory)
                    }
                };
                if !valid_absolute_path(&directory) {
                    return Err(CompletionError::InvalidDirectory);
                }
                self.requested_directory = Some(directory.clone());
                self.resolved_directory = None;
                Ok(CompletionQuery::Paths {
                    directory,
                    prefix: prefix.clone(),
                    directories_only: *directories_only,
                })
            }
        }
    }

    /// Bind a path reply to the exact prior query and its remote canonical parent.
    ///
    /// The caller obtains `resolved_directory` from the response to that same
    /// request, after verifying session/request identity. Canonical parents must
    /// not contain empty, dot or dot-dot components. A new immutable plan is
    /// returned; a failure leaves the original plan unchanged.
    pub fn bind_resolved_directory(
        &self,
        query_directory: &str,
        resolved_directory: &str,
    ) -> Result<Self, CompletionError> {
        if self.requested_directory.as_deref() != Some(query_directory)
            || !matches!(self.word, WordQuery::Paths { .. })
        {
            return Err(CompletionError::DirectoryMismatch);
        }
        if !valid_canonical_path(resolved_directory) {
            return Err(CompletionError::InvalidDirectory);
        }
        let mut bound = self.clone();
        bound.resolved_directory = Some(resolved_directory.to_owned());
        Ok(bound)
    }

    /// Produce a local edit after rechecking the whole input, caret and candidate.
    ///
    /// A command inserts its quoted basename. A path inserts the canonical parent
    /// plus the exact candidate basename and preserves the following path suffix.
    /// The candidate itself and any suffix are not resolved again: an entry may
    /// remain a symlink, and this edit does not prove the resulting path exists.
    /// Candidate metadata is never interpreted as shell source. No whitespace or
    /// execution key is appended.
    pub fn edit(
        &self,
        current_input: &str,
        current_caret: usize,
        candidate: &LiteralCandidate,
    ) -> Result<CompletionEdit, CompletionError> {
        if current_input != self.source || current_caret != self.caret {
            return Err(CompletionError::StaleInput);
        }
        if !valid_name(&candidate.name)
            || !valid_canonical_path(&candidate.path)
            || candidate.path.rsplit('/').next() != Some(candidate.name.as_str())
        {
            return Err(CompletionError::InvalidCandidate);
        }
        let (value, cursor, inside_quote) = match &self.word {
            WordQuery::Commands { prefix, suffix } => {
                if candidate.is_directory || !matches_component(&candidate.name, prefix, suffix) {
                    return Err(CompletionError::InvalidCandidate);
                }
                (candidate.name.clone(), candidate.name.len(), false)
            }
            WordQuery::Paths {
                prefix,
                suffix,
                tail,
                directories_only,
                ..
            } => {
                let parent = self
                    .resolved_directory
                    .as_deref()
                    .ok_or(CompletionError::DirectoryMismatch)?;
                if candidate.path != join_path(parent, &candidate.name)
                    || !matches_component(&candidate.name, prefix, suffix)
                    || (*directories_only && !candidate.is_directory)
                {
                    return Err(CompletionError::InvalidCandidate);
                }
                let mut value = candidate.path.clone();
                let mut cursor = value.len();
                if tail.is_empty() && candidate.is_directory {
                    value.push('/');
                    cursor += 1;
                } else {
                    value.push_str(tail);
                }
                if value.len() > MAX_COMPLETION_PATH_BYTES {
                    return Err(CompletionError::InvalidCandidate);
                }
                (value, cursor, candidate.is_directory || !tail.is_empty())
            }
        };
        let (replacement, relative_cursor) = quote_word(&value, cursor, inside_quote);
        let edited_len = self.source.len() - self.range.len() + replacement.len();
        if edited_len > MAX_COMPLETION_INPUT_BYTES {
            return Err(CompletionError::InputTooLong);
        }
        Ok(CompletionEdit {
            range: self.range.clone(),
            replacement,
            caret_byte: self.range.start + relative_cursor,
        })
    }
}

/// Analyze the shell word at a UTF-8 byte caret, without expansion or I/O.
///
/// Supports simple-command lists, pipelines, literal assignments, common file
/// redirections and POSIX quoting. Unsupported compound/expansion contexts fail
/// closed, including here-documents. An unfinished quote is allowed only when
/// the caret is at the end of the input. Non-empty selections and IME composition
/// must be excluded by the UI before invoking this single-caret API.
pub fn analyze_completion(
    input: &str,
    caret_byte: usize,
) -> Result<CompletionAnalysis, CompletionError> {
    if input.len() > MAX_COMPLETION_INPUT_BYTES {
        return Err(CompletionError::InputTooLong);
    }
    if caret_byte > input.len() || !input.is_char_boundary(caret_byte) {
        return Err(CompletionError::InvalidCursor);
    }
    match lexer::analyze(input, caret_byte) {
        Ok((range, word)) => Ok(CompletionAnalysis::Ready(CompletionPlan {
            source: input.to_owned(),
            caret: caret_byte,
            range,
            word,
            requested_directory: None,
            resolved_directory: None,
        })),
        Err(reason) => Ok(CompletionAnalysis::Unsupported(reason)),
    }
}

fn safe_char(c: char) -> bool {
    !c.is_control()
        && !matches!(c, '\u{fffd}' | '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !matches!(name, "." | "..")
        && !name.contains('/')
        && name.chars().all(safe_char)
}

fn valid_absolute_path(path: &str) -> bool {
    path.starts_with('/') && path.len() <= MAX_COMPLETION_PATH_BYTES && path.chars().all(safe_char)
}

fn valid_canonical_path(path: &str) -> bool {
    valid_absolute_path(path)
        && (path == "/"
            || path[1..]
                .split('/')
                .all(|part| !matches!(part, "" | "." | "..")))
}

fn join_path(parent: &str, name: &str) -> String {
    format!("{}/{name}", parent.trim_end_matches('/'))
}

fn matches_component(name: &str, prefix: &str, suffix: &str) -> bool {
    name.len() >= prefix.len() + suffix.len() && name.starts_with(prefix) && name.ends_with(suffix)
}

fn quote_word(value: &str, cursor: usize, inside_quote: bool) -> (String, usize) {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('\'');
    let mut mapped = output.len();
    for (offset, ch) in value.char_indices() {
        if offset == cursor {
            mapped = output.len();
        }
        if ch == '\'' {
            output.push_str("'\\''");
        } else {
            output.push(ch);
        }
    }
    if cursor == value.len() {
        mapped = output.len();
    }
    output.push('\'');
    if !inside_quote {
        mapped = output.len();
    }
    (output, mapped)
}
