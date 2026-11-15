//! Explicit, reviewed version observations and tiny in-memory archive stream checks over SSH.
//!
//! This is independent of [`crate::archive_tools`] PATH discovery. Review the entire
//! [`validation_command`] before calling [`SshSession::validate_archive_tools`]. Neither version
//! text nor a successful fixed sample authenticates an executable or authorizes later file access.

use crate::{ConnectionState, SessionError, SshSession, archive_tools::ArchiveTool};
use serde::{Deserialize, Deserializer};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

const HELPER: &str = include_str!("archive_validation.py");
const PROTOCOL: &str = "keelshell.archive-validation.v1";
const MAX_PATH_BYTES: usize = 4096;
const MAX_VERSION_BYTES: usize = 256;
const MAX_RESPONSE_BYTES: usize = keelshell_core::MAX_DIAGNOSTIC_OUTPUT_BYTES;
const VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_RESPONSE: &str = "invalid archive tool validation response";

/// Outcome of one fixed check, without granting executable or future feature authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCheckStatus {
    /// The fixed command exited zero and returned safe version text, or its fixed sample matched.
    Passed,
    /// Execution, output validation or the fixed sample comparison failed.
    Failed,
    /// No admitted absolute executable path or executable access was available.
    Unavailable,
    /// No functional check is admitted for this tool in this batch.
    NotTested,
    /// The check's own or the shared helper deadline expired, possibly before dispatch.
    TimedOut,
    /// The check's own or shared combined stdout/stderr budget was exhausted.
    OutputLimit,
}

/// One ephemeral version observation and a separately classified fixed stream check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveToolCheck {
    /// One of the seven fixed tools in [`ArchiveTool::ALL`].
    pub tool: ArchiveTool,
    /// Observed safe absolute executable path, at most 4096 UTF-8 bytes.
    pub path: Option<String>,
    /// First safe nonempty version line, at most 256 UTF-8 bytes; never an authenticity claim.
    /// Present only when [`Self::version_status`] is [`ToolCheckStatus::Passed`].
    pub version: Option<String>,
    /// Result of the tool's fixed version invocation, independent of its functional check.
    pub version_status: ToolCheckStatus,
    /// Result for the tiny fixed in-memory sample. `zip`, `unzip`, `rz` and `sz` are always
    /// [`ToolCheckStatus::NotTested`]; their interoperability has not been tested.
    pub stream_check: ToolCheckStatus,
}

/// A complete report from one explicitly requested check on one authenticated SSH connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveValidationReport {
    /// Exactly seven checks in [`ArchiveTool::ALL`] order; no cached capability or authority.
    pub tools: Vec<ArchiveToolCheck>,
}

/// Return the complete fixed POSIX command for human review and verbatim SSH execution.
///
/// Starts `python3 -I -S -c` with the entire stdlib-only helper as one shell-quoted argument.
/// No user arguments, uploaded scripts, temporary files, installation or networking are used.
/// Version commands are fixed: `tar --version`, `gzip --version`, `zip -v`, `unzip -v`,
/// `python3 -I -S --version`, `rz --version`, `sz --version`.
///
/// Functional checks only send a fixed in-memory tar to `tar -xOf - keelshell-probe.txt`,
/// a fixed in-memory gzip to `gzip -dc`, and run a fixed isolated Python BytesIO ZIP/tar.gz
/// round trip. Each output is compared with a fixed tiny payload; no file is extracted.
/// Relative PATH entries are excluded and subprocesses receive a minimal environment.
/// Each invocation retains a live Python supervisor as its process-group leader. A separate
/// bounded status pipe reports the tool's exit before group termination and leader reaping,
/// including ordinary descendants that close their standard streams. Nonblocking bounded
/// pipes, a supervisor watchdog and bounded kill/wait cleanup retain ownership. This is not
/// a sandbox for a malicious executable or an authenticity certification.
pub fn validation_command() -> String {
    format!("python3 -I -S -c '{}'", HELPER.replace('\'', "'\\''"))
}

impl SshSession {
    /// Run the reviewed [`validation_command`] on this exact authenticated connection.
    ///
    /// A single ten-second deadline covers opening, exec acknowledgement, output and normal
    /// cleanup. The remote helper has a six-second shared work budget, one second per command,
    /// 4096 combined stdout/stderr bytes per command and 16 KiB across its commands. It reserves
    /// bounded cleanup time and fails closed if the direct child's exit cannot be confirmed.
    /// The SSH transport separately caps the complete response plus stderr at 32 KiB.
    ///
    /// Cancellation is checked before opening, before exec, while pending and before adoption.
    /// A late result or result from a closing connection cannot become a report. Dropping this
    /// future transfers channel cleanup to the existing bounded channel owner. Exceptional
    /// unconfirmed-open or failed CLOSE cleanup can shut down the shared transport. Local
    /// cancellation, channel CLOSE and timeouts do not attest that remote processes have stopped.
    ///
    /// Requires remote POSIX Python 3.8+. No local fallback is used. Reports are not cached.
    /// `Passed` versions are display observations; `Passed` samples only establish this tiny
    /// exchange, not general archive compatibility, Zmodem interoperability or safe future use.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Closed`] for cancellation or a closing connection,
    /// [`SessionError::Timeout`] for the shared SSH deadline,
    /// [`SessionError::Unsupported`] for rejected exec, unavailable Python/POSIX support or an
    /// unsuccessful helper (including unconfirmed child cleanup),
    /// [`SessionError::Invalid`] for unsafe, inconsistent or malformed output and
    /// [`SessionError::OutputLimit`] for excessive combined output. Other transport errors retain
    /// their existing typed variants. Individual unavailable or failed tools remain report rows.
    pub async fn validate_archive_tools(
        &self,
        cancelled: &AtomicBool,
    ) -> crate::Result<ArchiveValidationReport> {
        if cancelled.load(Ordering::Acquire)
            || self.is_closed()
            || self.connection_state() != ConnectionState::Connected
        {
            return Err(SessionError::Closed);
        }
        let until = Instant::now() + VALIDATION_TIMEOUT;
        let command = validation_command();
        let operation = async {
            let output = self
                .diagnostic_exec_until(&command, until, cancelled)
                .await
                .map_err(|error| match error {
                    SessionError::Rejected(_) => {
                        SessionError::Unsupported("archive tool validation exec is unavailable")
                    }
                    error => error,
                })?;
            if cancelled.load(Ordering::Acquire) {
                return Err(SessionError::Closed);
            }
            if output.exit_status != Some(0) {
                return Err(SessionError::Unsupported(
                    "archive tool validation requires POSIX Python 3.8+ and confirmed cleanup",
                ));
            }
            if !output.stderr.is_empty() {
                return Err(SessionError::Invalid(INVALID_RESPONSE));
            }
            parse_report(&output.stdout)
        };
        let result = tokio::select! {
            biased;
            _ = wait_cancelled(cancelled) => Err(SessionError::Closed),
            result = timeout_at(until, operation) => {
                result.map_err(|_| SessionError::Timeout("archive tool validation"))?
            },
        };
        if cancelled.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        if Instant::now() >= until {
            return Err(SessionError::Timeout("archive tool validation"));
        }
        if result.is_ok()
            && (self.is_closed() || self.connection_state() != ConnectionState::Connected)
        {
            return Err(SessionError::Closed);
        }
        result
    }
}

async fn wait_cancelled(cancelled: &AtomicBool) {
    loop {
        if cancelled.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReport {
    protocol: String,
    tools: Vec<WireCheck>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCheck {
    tool: String,
    #[serde(deserialize_with = "required_nullable")]
    path: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    version: Option<String>,
    version_status: ToolCheckStatus,
    stream_check: ToolCheckStatus,
}

// Plain Option fields also accept missing keys. These two keys must be present as text or null.
fn required_nullable<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

fn parse_report(bytes: &[u8]) -> crate::Result<ArchiveValidationReport> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(SessionError::OutputLimit(MAX_RESPONSE_BYTES));
    }
    let invalid = || SessionError::Invalid(INVALID_RESPONSE);
    let wire: WireReport = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if wire.protocol != PROTOCOL || wire.tools.len() != ArchiveTool::ALL.len() {
        return Err(invalid());
    }
    let mut tools = Vec::with_capacity(ArchiveTool::ALL.len());
    for (tool, row) in ArchiveTool::ALL.into_iter().zip(wire.tools) {
        if row.tool != tool.name()
            || row.path.as_deref().is_some_and(|path| {
                !safe_text(path, MAX_PATH_BYTES)
                    || !path.starts_with('/')
                    || path.rsplit('/').next() != Some(tool.name())
            })
            || row.version.as_deref().is_some_and(|version| {
                version.is_empty()
                    || version.trim() != version
                    || !safe_text(version, MAX_VERSION_BYTES)
            })
            || (row.version_status == ToolCheckStatus::Passed)
                != (row.path.is_some() && row.version.is_some())
            || (row.version_status != ToolCheckStatus::Passed && row.version.is_some())
            || row.version_status == ToolCheckStatus::NotTested
            || (row.path.is_none() && row.version_status != ToolCheckStatus::Unavailable)
        {
            return Err(invalid());
        }
        let stream_admitted = matches!(
            tool,
            ArchiveTool::Tar | ArchiveTool::Gzip | ArchiveTool::Python3
        );
        if (stream_admitted
            && (row.stream_check == ToolCheckStatus::NotTested
                || (row.path.is_none() && row.stream_check != ToolCheckStatus::Unavailable)))
            || (!stream_admitted && row.stream_check != ToolCheckStatus::NotTested)
        {
            return Err(invalid());
        }
        tools.push(ArchiveToolCheck {
            tool,
            path: row.path,
            version: row.version,
            version_status: row.version_status,
            stream_check: row.stream_check,
        });
    }
    Ok(ArchiveValidationReport { tools })
}

fn safe_text(value: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{206f}'
                        | '\u{feff}'
                        | '\u{fffd}'
                )
        })
}

#[cfg(test)]
#[path = "archive_validation/tests.rs"]
mod tests;
