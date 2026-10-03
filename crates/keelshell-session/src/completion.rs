//! Explicit, read-only remote candidates. This is not the interactive shell's
//! completion engine: PATH comes from a separate exec channel and paths come
//! from SFTP. No query text is executed, and no shell startup file is sourced by
//! an additional command. The server still controls how it starts SSH exec.

mod scan;
#[cfg(test)]
mod tests;

use crate::{SessionError, SshSession};
use std::time::Duration;
use tokio::time::{Instant, timeout_at};

pub(crate) const PATH_PROBE: &str = r#"command printf 'KEELSHELL_COMPLETION_V1\000%s\000' "$PATH""#;
const FRAME: &[u8] = b"KEELSHELL_COMPLETION_V1\0";
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PATH: usize = 4096;
const MAX_NAME: usize = 1024;

/// An explicit request, independent of the current PTY's working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionQuery {
    /// Executable file names in the separate SSH exec environment's PATH.
    Commands {
        /// Literal, case-sensitive basename prefix; never sent to exec.
        prefix: String,
    },
    /// Immediate children of an explicitly selected absolute POSIX directory.
    Paths {
        /// Absolute SFTP path. The server resolves links and `..` using REALPATH.
        directory: String,
        /// Literal, case-sensitive basename prefix.
        prefix: String,
        /// Include only directories, including links verified to directories.
        directories_only: bool,
    },
}

/// Observed file type, with symlink identity recorded separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// Regular file with at least one POSIX executable mode bit. This does not
    /// prove that the current user, ACLs, or mount options permit execution.
    Executable,
    /// Directory, or a symlink whose target was observed to be a directory.
    Directory,
    /// Regular file, or a symlink whose target was observed to be a regular file.
    File,
    /// Symlink whose target could not be classified.
    Symlink,
}

/// Unescaped literal metadata. Callers must quote inserted shell text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    /// One exact UTF-8 basename, without separators or unsafe display characters.
    pub name: String,
    /// Absolute path formed from the canonical parent and this exact basename.
    pub path: String,
    /// Observed kind. Executable classification applies to command queries.
    pub kind: CompletionKind,
    /// Whether the directory entry itself is a symlink.
    pub is_symlink: bool,
}

/// A bounded snapshot, never a promise that the filesystem will remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionResult {
    /// At most 64 candidates. Paths sort directories first, then by UTF-8 name;
    /// commands preserve PATH precedence, then sort names within each directory.
    pub candidates: Vec<CompletionCandidate>,
    /// Canonical parent for a path query; command queries have several parents.
    pub resolved_directory: Option<String>,
    /// At least one candidate, scan, name-byte, stat, or PATH-directory limit hit.
    pub limited: bool,
    /// Names rejected because they cannot be safely and exactly represented.
    pub skipped_unsafe_entries: usize,
    /// Relative, empty, unsafe, duplicate, or inaccessible PATH directories skipped.
    pub skipped_path_directories: usize,
}

/// Fixed diagnostic categories: remote status messages and probe output are
/// deliberately excluded, including from Debug and Display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompletionError {
    /// Query is not a supported absolute path or literal basename prefix.
    #[error("invalid remote completion query")]
    InvalidQuery,
    /// The server does not provide the required exec or SFTP environment.
    #[error("remote completion environment is unavailable")]
    UnsupportedEnvironment,
    /// A peer response cannot safely or exactly represent a candidate snapshot.
    #[error("invalid remote completion response")]
    InvalidResponse,
    /// The selected directory cannot be read with this account.
    #[error("remote completion permission denied")]
    PermissionDenied,
    /// The selected directory no longer exists or is unavailable.
    #[error("remote completion directory is unavailable")]
    Unavailable,
    /// The entire query exceeded its five-second budget.
    #[error("remote completion timed out")]
    Timeout,
    /// The fixed environment probe exceeded its memory limit.
    #[error("remote completion response exceeded its limit")]
    LimitExceeded,
    /// The SSH channel or transport could not complete the operation.
    #[error("remote completion channel failed")]
    Transport,
}

type Result<T> = std::result::Result<T, CompletionError>;

impl SshSession {
    /// Query remote candidates in at most five seconds, including channel opens,
    /// exec, SFTP initialization and metadata requests. Limits apply across all
    /// PATH directories: 8192 entries, 2 MiB filename/longname bytes, 64 extra
    /// metadata requests, 32 PATH directories and 64 returned candidates.
    ///
    /// Dropping this future cancels its owned channels without intentionally
    /// disconnecting unrelated channels. As with other channel operations, an
    /// unconfirmed open or failed bounded CLOSE can require transport shutdown.
    /// Cancellation confirms local abandonment, not acknowledgement by the peer.
    pub async fn complete_remote(&self, query: CompletionQuery) -> Result<CompletionResult> {
        query.validate()?;
        let until = Instant::now() + QUERY_TIMEOUT;
        timeout_at(until, scan::complete(self, query, until))
            .await
            .map_err(|_| CompletionError::Timeout)?
    }

    /// Return SFTP REALPATH(".") from an independent subsystem, bounded to five
    /// seconds. This is its default directory, not the active PTY's current
    /// directory and not necessarily the account's home directory.
    pub async fn completion_base(&self) -> Result<String> {
        let until = Instant::now() + QUERY_TIMEOUT;
        timeout_at(until, async {
            let raw = self
                .completion_sftp_until(until)
                .await
                .map_err(session_error)?;
            scan::canonical_directory(&raw, ".").await
        })
        .await
        .map_err(|_| CompletionError::Timeout)?
    }
}

impl CompletionQuery {
    fn validate(&self) -> Result<()> {
        let prefix = match self {
            Self::Commands { prefix } => prefix,
            Self::Paths {
                directory, prefix, ..
            } => {
                if !safe_path(directory) {
                    return Err(CompletionError::InvalidQuery);
                }
                prefix
            }
        };
        if prefix.len() > MAX_NAME || prefix.contains('/') || !safe_text(prefix) {
            return Err(CompletionError::InvalidQuery);
        }
        Ok(())
    }
}

fn safe_text(value: &str) -> bool {
    // russh-sftp decodes invalid UTF-8 lossily. Reject replacement characters as
    // well as controls rather than ever send a transformed filename back.
    !value.chars().any(|c| {
        c.is_control()
            || matches!(c,
        '\u{fffd}' | '\u{061c}' | '\u{200b}'..='\u{200f}' |
        '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
    })
}
fn safe_path(value: &str) -> bool {
    value.starts_with('/') && value.len() <= MAX_PATH && safe_text(value)
}
fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= MAX_NAME
        && !value.contains('/')
        && safe_text(value)
}
fn session_error(error: SessionError) -> CompletionError {
    match error {
        SessionError::Timeout(_) => CompletionError::Timeout,
        SessionError::OutputLimit(_) => CompletionError::LimitExceeded,
        SessionError::Rejected(_) | SessionError::Unsupported(_) => {
            CompletionError::UnsupportedEnvironment
        }
        _ => CompletionError::Transport,
    }
}
fn probe_directories(output: crate::ExecOutput) -> Result<(Vec<String>, usize, bool)> {
    if output.exit_status != Some(0) || !output.stderr.is_empty() {
        return Err(CompletionError::UnsupportedEnvironment);
    }
    let bytes = output
        .stdout
        .strip_prefix(FRAME)
        .and_then(|bytes| bytes.strip_suffix(b"\0"))
        .filter(|bytes| !bytes.contains(&0))
        .ok_or(CompletionError::InvalidResponse)?;
    let mut directories = Vec::new();
    let mut skipped = 0;
    let mut limited = false;
    for (index, bytes) in bytes.split(|byte| *byte == b':').enumerate() {
        if index >= 32 {
            limited = true;
            break;
        }
        let directory = std::str::from_utf8(bytes)
            .ok()
            .filter(|value| safe_path(value));
        match directory {
            Some(directory) if !directories.iter().any(|value| value == directory) => {
                directories.push(directory.to_owned())
            }
            _ => skipped += 1,
        }
    }
    Ok((directories, skipped, limited))
}
