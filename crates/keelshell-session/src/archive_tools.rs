//! Explicit discovery of archive and Zmodem command names on an authenticated SSH connection.
//!
//! Callers must review [`probe_command`] before invoking [`SshSession::inspect_archive_tools`].
//! Discovery never runs the named tools, verifies their versions or features, or grants authority
//! to use a returned path. Reports are ephemeral observations of a separate SSH exec environment.

use crate::{ConnectionState, SessionError, SshSession};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

const FRAME: &[u8] = b"KEELSHELL_ARCHIVE_TOOLS_V1\0";
const MAX_PATH_BYTES: usize = 4096;
// Seven maximum-sized paths and their fixed fields fit within the shared diagnostic cap.
const MAX_RESPONSE_BYTES: usize = keelshell_core::MAX_DIAGNOSTIC_OUTPUT_BYTES;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_RESPONSE: &str = "invalid archive tool discovery response";
// The sentinel preserves lookup output's trailing newlines. Remove just the one added by
// command -v, so an unsafe pathname cannot silently become a different valid pathname.
const PROBE: &str = r#"sh -c '
command printf "KEELSHELL_ARCHIVE_TOOLS_V1\000" || exit 65
ks_newline="
"
for ks_tool in tar gzip zip unzip python3 rz sz
do
    ks_resolution=$(
        command -v "$ks_tool" 2>/dev/null
        ks_status=$?
        command printf "."
        exit "$ks_status"
    )
    ks_status=$?
    case "$ks_status" in
        0)
            case "$ks_resolution" in
                *"$ks_newline".) ks_path=${ks_resolution%"$ks_newline".} ;;
                *) exit 65 ;;
            esac
            case "$ks_path" in
                /*) ks_availability=found ;;
                *"$ks_newline"*) ks_availability=unverified; ks_path= ;;
                *) ks_availability=unverified ;;
            esac
            ;;
        1|127)
            ks_availability=missing
            ks_path=
            ;;
        *) exit 65 ;;
    esac
    command printf "%s\000%s\000%s\000" "$ks_tool" "$ks_availability" "$ks_path" || exit 65
done
'"#;

/// Fixed command names admitted for discovery, without executing any of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveTool {
    /// The `tar` command.
    Tar,
    /// The `gzip` command.
    Gzip,
    /// The `zip` command.
    Zip,
    /// The `unzip` command.
    Unzip,
    /// The `python3` command.
    Python3,
    /// The `rz` command.
    Rz,
    /// The `sz` command.
    Sz,
}

impl ArchiveTool {
    /// All admitted tools in the exact order required by the discovery protocol.
    pub const ALL: [Self; 7] = [
        Self::Tar,
        Self::Gzip,
        Self::Zip,
        Self::Unzip,
        Self::Python3,
        Self::Rz,
        Self::Sz,
    ];

    /// Return the fixed POSIX shell command name for this tool.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tar => "tar",
            Self::Gzip => "gzip",
            Self::Zip => "zip",
            Self::Unzip => "unzip",
            Self::Python3 => "python3",
            Self::Rz => "rz",
            Self::Sz => "sz",
        }
    }
}

/// What a single `command -v` lookup observed; no tool was executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveToolAvailability {
    /// Lookup returned a safe absolute path. Version, features and usability remain unverified.
    Found,
    /// Lookup reported that the command name could not be resolved.
    Missing,
    /// Lookup succeeded without an absolute path, such as a shell function or relative PATH entry.
    Unverified,
}

/// One validated lookup result, without executable or feature authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveToolObservation {
    /// The fixed command name that was looked up.
    pub tool: ArchiveTool,
    /// Whether lookup found an absolute path, failed, or returned a shell-only resolution.
    pub availability: ArchiveToolAvailability,
    /// Exact safe UTF-8 resolution, at most 4096 bytes. Absolute only for `Found`;
    /// absent for `Missing` and for an empty `Unverified` resolution.
    pub path: Option<String>,
}

/// A complete ephemeral snapshot from one explicitly requested SSH probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveToolReport {
    /// Exactly seven observations in [`ArchiveTool::ALL`] order.
    pub tools: Vec<ArchiveToolObservation>,
}

/// Return the complete fixed command to show for human review and send verbatim over SSH.
///
/// The command starts a POSIX `sh`, uses only `command -v` to discover the seven fixed names,
/// and emits `KEELSHELL_ARCHIVE_TOOLS_V1\0` followed by three NUL-terminated fields per tool:
/// name, lowercase `found` / `missing` / `unverified`, and resolution. It has no user inputs
/// and never executes an archive, Python, or Zmodem tool.
pub fn probe_command() -> String {
    PROBE.to_owned()
}

impl SshSession {
    /// Discover seven fixed tool names after the caller has approved [`probe_command`].
    ///
    /// Uses this exact authenticated connection and an independently owned exec channel.
    /// One ten-second deadline covers channel opening, exec acknowledgement, reads and normal
    /// cleanup. The inherited diagnostic transport caps combined stdout/stderr at 32 KiB,
    /// below the 64 KiB discovery ceiling. Every path is separately bounded to 4096 bytes.
    ///
    /// Cancellation is checked before opening, after opening before exec, during the operation,
    /// and before adopting a report. Dropping the future also transfers channel cleanup to the
    /// existing bounded owner. Normal cleanup preserves other channels; an unconfirmed open
    /// or failed CLOSE can require shared transport shutdown. Cleanup can outlive local
    /// cancellation or the deadline and does not attest to remote process termination.
    ///
    /// No result is cached. `Found` attests only to an absolute `command -v` resolution and
    /// does not verify a version, feature set, executable access, or future availability.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Closed`] for cancellation or a connection already closing,
    /// [`SessionError::Timeout`] when the overall deadline expires,
    /// [`SessionError::Unsupported`] for an unavailable POSIX exec environment,
    /// [`SessionError::Invalid`] for malformed, incomplete or unsafe protocol output, and
    /// [`SessionError::OutputLimit`] for excessive combined output. Other transport failures
    /// retain their existing typed error. Missing tools are observations in a successful report.
    pub async fn inspect_archive_tools(
        &self,
        cancelled: &AtomicBool,
    ) -> crate::Result<ArchiveToolReport> {
        if cancelled.load(Ordering::Acquire)
            || self.is_closed()
            || self.connection_state() != ConnectionState::Connected
        {
            return Err(SessionError::Closed);
        }
        let until = Instant::now() + PROBE_TIMEOUT;
        let command = probe_command();
        let operation = async {
            let output = self
                .diagnostic_exec_until(&command, until, cancelled)
                .await
                .map_err(|error| match error {
                    SessionError::Rejected(_) => {
                        SessionError::Unsupported("archive tool discovery exec is unavailable")
                    }
                    error => error,
                })?;
            if cancelled.load(Ordering::Acquire) {
                return Err(SessionError::Closed);
            }
            if output.exit_status != Some(0) {
                return Err(SessionError::Unsupported(
                    "archive tool discovery requires a POSIX sh environment",
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
                result.map_err(|_| SessionError::Timeout("archive tool discovery"))?
            },
        };
        if cancelled.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        // A ready future can finish in the same poll as its deadline; never adopt a late frame.
        if Instant::now() >= until {
            return Err(SessionError::Timeout("archive tool discovery"));
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

fn parse_report(bytes: &[u8]) -> crate::Result<ArchiveToolReport> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(SessionError::OutputLimit(MAX_RESPONSE_BYTES));
    }
    let invalid = || SessionError::Invalid(INVALID_RESPONSE);
    let body = bytes.strip_prefix(FRAME).ok_or_else(invalid)?;
    let text = std::str::from_utf8(body).map_err(|_| invalid())?;
    let mut fields = text.split('\0');
    let mut tools = Vec::with_capacity(ArchiveTool::ALL.len());
    for tool in ArchiveTool::ALL {
        let name = fields.next().ok_or_else(invalid)?;
        let status = fields.next().ok_or_else(invalid)?;
        let path = fields.next().ok_or_else(invalid)?;
        if name != tool.name() || !safe_resolution(path) {
            return Err(invalid());
        }
        let availability = match status {
            "found" if path.starts_with('/') && path.len() > 1 && !path.ends_with('/') => {
                ArchiveToolAvailability::Found
            }
            "missing" if path.is_empty() => ArchiveToolAvailability::Missing,
            "unverified" if !path.starts_with('/') => ArchiveToolAvailability::Unverified,
            _ => return Err(invalid()),
        };
        tools.push(ArchiveToolObservation {
            tool,
            availability,
            path: (!path.is_empty()).then(|| path.to_owned()),
        });
    }
    // Splitting a fully NUL-terminated frame leaves exactly one empty final field.
    if fields.next() != Some("") || fields.next().is_some() {
        return Err(invalid());
    }
    Ok(ArchiveToolReport { tools })
}

fn safe_resolution(value: &str) -> bool {
    value.len() <= MAX_PATH_BYTES
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
mod tests {
    use super::*;

    fn wire(records: &[(&str, &str, &str)]) -> Vec<u8> {
        let mut bytes = FRAME.to_vec();
        for (name, status, path) in records {
            for field in [name, status, path] {
                bytes.extend_from_slice(field.as_bytes());
                bytes.push(0);
            }
        }
        bytes
    }

    fn missing_records<'a>() -> Vec<(&'a str, &'a str, &'a str)> {
        ArchiveTool::ALL
            .into_iter()
            .map(|tool| (tool.name(), "missing", ""))
            .collect()
    }

    fn assert_invalid(bytes: &[u8]) {
        assert!(
            matches!(
                parse_report(bytes),
                Err(SessionError::Invalid(INVALID_RESPONSE))
            ),
            "expected a protocol rejection"
        );
    }

    #[test]
    fn fixed_tool_order_and_names_match_the_review_contract() {
        assert_eq!(
            ArchiveTool::ALL.map(ArchiveTool::name),
            ["tar", "gzip", "zip", "unzip", "python3", "rz", "sz"]
        );
    }

    #[test]
    fn parser_preserves_exact_utf8_absolute_resolutions() {
        let paths: Vec<_> = ArchiveTool::ALL
            .into_iter()
            .map(|tool| format!("/opt/归档 tools/{name}", name = tool.name()))
            .collect();
        let records: Vec<_> = ArchiveTool::ALL
            .into_iter()
            .zip(&paths)
            .map(|(tool, path)| (tool.name(), "found", path.as_str()))
            .collect();
        let report = parse_report(&wire(&records)).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            report.tools,
            ArchiveTool::ALL
                .into_iter()
                .zip(paths)
                .map(|(tool, path)| ArchiveToolObservation {
                    tool,
                    availability: ArchiveToolAvailability::Found,
                    path: Some(path),
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_tools_form_a_complete_successful_report() {
        let report =
            parse_report(&wire(&missing_records())).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            report.tools,
            ArchiveTool::ALL
                .into_iter()
                .map(|tool| ArchiveToolObservation {
                    tool,
                    availability: ArchiveToolAvailability::Missing,
                    path: None,
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn shell_and_relative_resolutions_remain_unverified() {
        for resolution in [
            "tar",
            "./bin/tar",
            "alias tar='other tar'",
            "tar () { printf shell; }",
            "",
        ] {
            let mut records = missing_records();
            records[0] = ("tar", "unverified", resolution);
            let report = parse_report(&wire(&records)).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(
                report.tools[0],
                ArchiveToolObservation {
                    tool: ArchiveTool::Tar,
                    availability: ArchiveToolAvailability::Unverified,
                    path: (!resolution.is_empty()).then(|| resolution.to_owned()),
                }
            );
        }
    }

    #[test]
    fn every_truncated_frame_is_rejected() {
        let bytes = wire(&missing_records());
        for end in 0..bytes.len() {
            assert_invalid(&bytes[..end]);
        }
    }

    #[test]
    fn unknown_frame_names_statuses_and_versions_are_rejected() {
        let valid = wire(&missing_records());
        let mut wrong_version = valid.clone();
        wrong_version[FRAME.len() - 2] = b'2';
        assert_invalid(&wrong_version);
        let mut prefixed = b"banner\n".to_vec();
        prefixed.extend_from_slice(&valid);
        assert_invalid(&prefixed);
        for (name, status) in [
            ("rar", "missing"),
            ("Tar", "missing"),
            ("tar", "Found"),
            ("tar", "unknown"),
            ("tar", ""),
        ] {
            let mut records = missing_records();
            records[0] = (name, status, "");
            assert_invalid(&wire(&records));
        }
    }

    #[test]
    fn duplicate_reordered_and_additional_records_are_rejected() {
        let mut records = missing_records();
        records[1] = records[0];
        assert_invalid(&wire(&records));
        let mut records = missing_records();
        records.swap(0, 1);
        assert_invalid(&wire(&records));
        let mut records = missing_records();
        records.push(("sz", "missing", ""));
        assert_invalid(&wire(&records));
    }

    #[test]
    fn trailing_fields_bytes_and_embedded_nuls_are_rejected() {
        for suffix in [b"\0".as_slice(), b"extra", b"\n", b"extra\0"] {
            let mut bytes = wire(&missing_records());
            bytes.extend_from_slice(suffix);
            assert_invalid(&bytes);
        }
        let mut records = missing_records();
        records[0] = ("tar", "found", "/bin/tar\0forged");
        assert_invalid(&wire(&records));
    }

    #[test]
    fn statuses_require_consistent_absolute_relative_or_absent_paths() {
        for (status, path) in [
            ("found", ""),
            ("found", "tar"),
            ("found", "./tar"),
            ("found", "/"),
            ("found", "/bin/"),
            ("missing", "/bin/tar"),
            ("missing", "tar"),
            ("unverified", "/bin/tar"),
        ] {
            let mut records = missing_records();
            records[0] = ("tar", status, path);
            assert_invalid(&wire(&records));
        }
    }

    #[test]
    fn invalid_utf8_and_unsafe_display_characters_are_rejected() {
        let mut bytes = wire(&missing_records());
        bytes[FRAME.len()] = 0xff;
        assert_invalid(&bytes);
        let mut records = missing_records();
        records[0] = ("tar", "found", "/bin/tar");
        let mut bytes = wire(&records);
        let invalid_path_byte = FRAME.len() + b"tar\0found\0".len();
        bytes[invalid_path_byte] = 0xff;
        assert_invalid(&bytes);
        for character in [
            '\n', '\r', '\t', '\u{1b}', '\u{7f}', '\u{85}', '\u{061c}', '\u{200b}', '\u{2028}',
            '\u{202e}', '\u{2066}', '\u{feff}', '\u{fffd}',
        ] {
            for (status, prefix) in [("found", "/bin/"), ("unverified", "function ")] {
                let path = format!("{prefix}ta{character}r");
                let mut records = missing_records();
                records[0] = ("tar", status, &path);
                assert_invalid(&wire(&records));
            }
        }
    }

    #[test]
    fn path_limit_counts_utf8_bytes_and_accepts_the_exact_boundary() {
        let exact = format!("/{}", "档".repeat((MAX_PATH_BYTES - 1) / 3));
        assert_eq!(exact.len(), MAX_PATH_BYTES);
        let mut records = missing_records();
        records[0] = ("tar", "found", &exact);
        assert!(
            parse_report(&wire(&records)).is_ok(),
            "the exact path byte limit should be accepted"
        );
        let overlong = format!("{exact}x");
        records[0] = ("tar", "found", &overlong);
        assert_invalid(&wire(&records));
        records[0] = ("tar", "unverified", &overlong[1..]);
        assert!(
            parse_report(&wire(&records)).is_ok(),
            "the exact relative resolution byte limit should be accepted"
        );
        let overlong_relative = format!("x{}", &overlong[1..]);
        records[0] = ("tar", "unverified", &overlong_relative);
        assert_invalid(&wire(&records));
    }

    #[test]
    fn maximum_complete_report_fits_and_oversize_response_is_typed() {
        let path = format!("/{}", "x".repeat(MAX_PATH_BYTES - 1));
        let records: Vec<_> = ArchiveTool::ALL
            .into_iter()
            .map(|tool| (tool.name(), "found", path.as_str()))
            .collect();
        let bytes = wire(&records);
        assert!(bytes.len() < MAX_RESPONSE_BYTES);
        assert!(parse_report(&bytes).is_ok());
        assert!(matches!(
            parse_report(&vec![b'x'; MAX_RESPONSE_BYTES + 1]),
            Err(SessionError::OutputLimit(MAX_RESPONSE_BYTES))
        ));
    }
}
