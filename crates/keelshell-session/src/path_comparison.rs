//! Explicit, bounded comparisons of two authenticated routes to one logical endpoint.
//!
//! Review [`PathComparisonPlan::review_text`] before running a plan. Measurements
//! include SSH channel opening and Python startup; they are neither ICMP RTT nor
//! bare link bandwidth. No profiles, user files, AI or persistence are involved.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::{ConnectionState, SshSession, ssh::path_comparison_transport};
use tokio::time::Instant;

pub(crate) const ROUNDS: u8 = 3;
pub(crate) const PAYLOAD_BYTES: usize = 256 * 1024;
pub(crate) const SAMPLE_TIMEOUT: Duration = Duration::from_secs(8);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const EXEC_REPLY: &[u8] = b"KEELSHELL_PATH_EXEC_V1\n";
pub(crate) const UPLOAD_REPLY: &[u8] = b"KEELSHELL_PATH_UPLOAD_262144_V1\n";
pub(crate) const PAYLOAD_BYTE: u8 = b'K';

const EXEC_COMMAND: &str = concat!(
    "python3 -I -S -B -c '# KEELSHELL_PATH_PROBE exec\n",
    "import signal,sys\n",
    "signal.alarm(8)\n",
    "if sys.stdin.buffer.read(1) != b\"\": sys.exit(65)\n",
    "sys.stdout.buffer.write(b\"KEELSHELL_PATH_EXEC_V1\\n\")\n",
    "sys.stdout.buffer.flush()\n",
    "'"
);
const UPLOAD_COMMAND: &str = concat!(
    "python3 -I -S -B -c '# KEELSHELL_PATH_PROBE upload\n",
    "import signal,sys\n",
    "signal.alarm(8)\n",
    "data = sys.stdin.buffer.read(262145)\n",
    "if data != b\"K\" * 262144: sys.exit(65)\n",
    "sys.stdout.buffer.write(b\"KEELSHELL_PATH_UPLOAD_262144_V1\\n\")\n",
    "sys.stdout.buffer.flush()\n",
    "'"
);
const DOWNLOAD_COMMAND: &str = concat!(
    "python3 -I -S -B -c '# KEELSHELL_PATH_PROBE download\n",
    "import signal,sys\n",
    "signal.alarm(8)\n",
    "if sys.stdin.buffer.read(1) != b\"\": sys.exit(65)\n",
    "sys.stdout.buffer.write(b\"K\" * 262144)\n",
    "sys.stdout.buffer.flush()\n",
    "'"
);

/// Side of the human-reviewed comparison, bound to one exact connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathSide {
    /// First authenticated route.
    A,
    /// Second authenticated route.
    B,
}

/// One of three fixed, memory-only probes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathProbeKind {
    /// Fixed exec request and exact small acknowledgement, with no payload.
    Exec,
    /// Upload exactly 256 KiB of synthetic bytes and validate the acknowledgement.
    Upload,
    /// Download and validate exactly 256 KiB of synthetic bytes.
    Download,
}

impl PathProbeKind {
    pub(crate) const ALL: [Self; 3] = [Self::Exec, Self::Upload, Self::Download];

    /// Exact fixed command, suitable for a localized review without changing
    /// any bytes that the approved plan will dispatch.
    pub const fn command(self) -> &'static str {
        match self {
            Self::Exec => EXEC_COMMAND,
            Self::Upload => UPLOAD_COMMAND,
            Self::Download => DOWNLOAD_COMMAND,
        }
    }
}

/// Fixed eligibility failures; no authentication material or peer diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathComparisonError {
    /// Clones of a single connection cannot represent two paths.
    #[error("path comparison requires two distinct authenticated connections")]
    SameConnection,
    /// Host, port, username or verified host key differs.
    #[error("path comparison requires the same requested endpoint and verified host key")]
    DifferentEndpoint,
    /// Both connections used the same recursive jump and explicit proxy route.
    #[error("path comparison requires different actual routes")]
    SameRoute,
    /// At least one connection is closing or locally known to be closed.
    #[error("path comparison requires two live authenticated connections")]
    ConnectionClosed,
}

/// Fixed sample failure categories, safe to display without exposing remote text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathProbeFailure {
    /// Explicit cancellation revoked this sample.
    #[error("path probe cancelled")]
    Cancelled,
    /// Its eight-second absolute budget expired.
    #[error("path probe deadline expired")]
    SampleDeadline,
    /// The complete comparison's sixty-second absolute budget expired.
    #[error("path comparison deadline expired")]
    ComparisonDeadline,
    /// A connection was closing or locally known to be closed.
    #[error("path probe connection is closed")]
    ConnectionClosed,
    /// The SSH channel failed without exposing peer-supplied error text.
    #[error("path probe SSH transport failed")]
    Transport,
    /// The peer explicitly rejected opening the channel.
    #[error("path probe channel was rejected")]
    ChannelRejected,
    /// The peer explicitly rejected the sole exec request.
    #[error("path probe exec was rejected")]
    ExecRejected,
    /// No positive exec acknowledgement preceded the response.
    #[error("path probe exec was not acknowledged")]
    MissingAcknowledgement,
    /// The peer sent a duplicated or out-of-order control message.
    #[error("path probe protocol sequence is invalid")]
    InvalidSequence,
    /// stdout did not equal the complete fixed expected bytes.
    #[error("path probe stdout did not match the fixed sample")]
    InvalidStdout,
    /// Any extended output, including stderr, prevents success.
    #[error("path probe produced unexpected stderr or extended output")]
    UnexpectedStderr,
    /// The complete response exceeded this probe's own byte limit.
    #[error("path probe output limit exceeded")]
    OutputLimit,
    /// EOF was absent before channel CLOSE.
    #[error("path probe did not confirm output EOF")]
    MissingEof,
    /// An explicit process exit status was absent.
    #[error("path probe did not confirm process exit")]
    MissingExitStatus,
    /// The process reported a nonzero exit; no raw exit diagnostics are included.
    #[error("path probe process exited unsuccessfully")]
    UnsuccessfulExit,
    /// The process reported a signal instead of the required zero exit status.
    #[error("path probe process was signalled")]
    RemoteSignal,
    /// The stream ended without an explicit channel CLOSE.
    #[error("path probe did not confirm channel close")]
    MissingClose,
    /// Upload or stdin EOF was not completely admitted to the SSH writer.
    #[error("path probe input was not completed")]
    IncompleteInput,
}

/// Validated outcome of one attempted sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathSampleOutcome {
    /// Exact stdout, no stderr, acknowledgement, EOF, exit zero and CLOSE.
    Passed,
    /// A fixed safe failure category; never remote output.
    Failed(PathProbeFailure),
}

/// One sample in actual execution order; absent operations are not fabricated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathSample {
    /// Exact reviewed route.
    pub path: PathSide,
    /// Fixed operation.
    pub kind: PathProbeKind,
    /// One-based round, from one through three.
    pub round: u8,
    /// Local elapsed time, including channel open, process startup and completion.
    pub elapsed: Duration,
    /// Synthetic payload bytes admitted to the upload writer or received for
    /// download, capped at 256 KiB. Exec and protocol acknowledgement bytes are
    /// excluded. A partial upload count does not attest remote receipt.
    pub bytes: usize,
    /// Strictly validated outcome, including failures and cancellation.
    pub outcome: PathSampleOutcome,
}

/// Completion of the entire comparison, independently of individual rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathComparisonStatus {
    /// All eighteen samples succeeded within the original budgets.
    Complete,
    /// Explicit cancellation prevented completion.
    Cancelled,
    /// Failure, closure or deadline prevented completion.
    Incomplete,
}

/// Ephemeral local report; it grants no authority to reconnect or switch routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathComparisonReport {
    /// Whole-comparison status.
    pub status: PathComparisonStatus,
    /// Attempted samples in actual order, including the first unsuccessful one.
    pub samples: Vec<PathSample>,
    /// An admitted channel or possibly dispatched process could not be confirmed
    /// finished. Sending CLOSE alone cannot clear this flag. The caller should
    /// revoke further comparisons until the connection has been checked/replaced.
    pub cleanup_unconfirmed: bool,
}

impl PathComparisonReport {
    /// Median only for a complete report with three successful, distinct rounds
    /// for this route and operation. Missing, duplicate or failed rows give None.
    pub fn summary(&self, path: PathSide, kind: PathProbeKind) -> Option<Duration> {
        if self.status != PathComparisonStatus::Complete || self.cleanup_unconfirmed {
            return None;
        }
        let mut rounds = [None; ROUNDS as usize];
        for row in self
            .samples
            .iter()
            .filter(|row| row.path == path && row.kind == kind)
        {
            if !(1..=ROUNDS).contains(&row.round)
                || row.outcome != PathSampleOutcome::Passed
                || rounds[usize::from(row.round - 1)]
                    .replace(row.elapsed)
                    .is_some()
            {
                return None;
            }
        }
        let [Some(first), Some(second), Some(third)] = rounds else {
            return None;
        };
        let mut times = [first, second, third];
        times.sort_unstable();
        Some(times[1])
    }
}

/// Human-reviewable plan retaining two real connections with immutable routes.
#[derive(Clone)]
pub struct PathComparisonPlan {
    first: SshSession,
    second: SshSession,
}

impl PathComparisonPlan {
    /// Bind two different live connections to the same logical endpoint and
    /// verified host key, using different actual jump/proxy routes.
    ///
    /// # Errors
    ///
    /// Rejects a shared connection, differing endpoints, identical routes and
    /// locally closing/closed connections. Matching keys do not prove that DNS
    /// or a cluster selected the same physical server instance.
    pub fn new(first: SshSession, second: SshSession) -> Result<Self, PathComparisonError> {
        if first.same_connection(&second) {
            return Err(PathComparisonError::SameConnection);
        }
        if !first
            .connection_info()
            .same_endpoint(second.connection_info())
        {
            return Err(PathComparisonError::DifferentEndpoint);
        }
        if first.connection_info().same_route(second.connection_info()) {
            return Err(PathComparisonError::SameRoute);
        }
        if !live(&first) || !live(&second) {
            return Err(PathComparisonError::ConnectionClosed);
        }
        Ok(Self { first, second })
    }

    /// Immutable authenticated route for the specified side of this plan.
    ///
    /// This permits localized presentation without accepting replacement
    /// profile metadata or exposing either connection's authentication material.
    pub fn connection_info(&self, path: PathSide) -> &crate::SshConnectionInfo {
        match path {
            PathSide::A => self.first.connection_info(),
            PathSide::B => self.second.connection_info(),
        }
    }

    /// Complete routes and exact commands, quantities, ordering and deadlines.
    /// This performs no network I/O and contains no proxy credentials.
    pub fn review_text(&self) -> String {
        format!(
            "Path A (first target is the final SSH hop):\n{}\n\
             Path B (second target is the final SSH hop):\n{}\n\
             Same requested host/port/user and verified key; DNS/clusters may select \
             different physical instances.\n\
             Three rounds; each round executes Exec, Upload, Download. Within each \
             operation: round 1 A/B, round 2 B/A, round 3 A/B. Eighteen samples, no retries. \
             Stop at the first failure or cancellation; absent samples are not measured.\n\
             Each upload/download is 262144 bytes (256 KiB); total synthetic payload \
             3145728 bytes (3 MiB), plus fixed commands, acknowledgements and SSH overhead.\n\
             One absolute 60-second comparison deadline; each sample has an absolute \
             8-second deadline including opening, acknowledgement and completion. \
             Each fixed remote Python program also arms an 8-second POSIX alarm. \
             Requires POSIX/Python 3; no installation, files, PTY or sudo. Memory only.\n\
             Timings include channel open, interpreter/command startup, transfer, EOF, \
             exit and CLOSE; neither ICMP RTT nor bare link bandwidth. Median results \
             require a complete comparison. No automatic path selection or switching.\n\
             Cancellation closes only owned sample channels. If completion cannot be \
             confirmed, cleanup remains unconfirmed; a local CLOSE is not process-exit proof.\n\
             Exact Exec command:\n{}\n\
             Exact Upload command:\n{}\n\
             Exact Download command:\n{}\n",
            self.first.connection_info().review_text(),
            self.second.connection_info().review_text(),
            EXEC_COMMAND,
            UPLOAD_COMMAND,
            DOWNLOAD_COMMAND
        )
    }

    /// True only for these exact connections in their original A/B order.
    pub fn matches_sessions(&self, first: &SshSession, second: &SshSession) -> bool {
        self.first.same_connection(first) && self.second.same_connection(second)
    }

    /// Run the explicitly approved plan once with no retries or automatic switch.
    ///
    /// Cancellation is checked before open, after open/before EXEC, in flight
    /// and before result adoption. Each admitted channel retains an independent
    /// cleanup owner if this future is dropped. Ordinary cleanup preserves the
    /// SSH connections and their parents/siblings; exceptionally unconfirmed
    /// opens retain the existing channel owner's transport-stop policy.
    pub async fn run(&self, cancelled: &AtomicBool) -> PathComparisonReport {
        let until = Instant::now() + TOTAL_TIMEOUT;
        let mut report = PathComparisonReport {
            status: PathComparisonStatus::Incomplete,
            samples: Vec::with_capacity(usize::from(ROUNDS) * 6),
            cleanup_unconfirmed: false,
        };
        for round in 1..=ROUNDS {
            let sides = if round % 2 == 1 {
                [PathSide::A, PathSide::B]
            } else {
                [PathSide::B, PathSide::A]
            };
            for kind in PathProbeKind::ALL {
                for path in sides {
                    if cancelled.load(Ordering::Acquire) {
                        report.status = PathComparisonStatus::Cancelled;
                        return report;
                    }
                    if Instant::now() >= until {
                        return report;
                    }
                    if !live(&self.first) || !live(&self.second) {
                        return report;
                    }
                    let session = match path {
                        PathSide::A => &self.first,
                        PathSide::B => &self.second,
                    };
                    let result = path_comparison_transport::sample(
                        session, path, kind, round, until, cancelled,
                    )
                    .await;
                    let passed = result.sample.outcome == PathSampleOutcome::Passed;
                    let sample_cancelled = result.sample.outcome
                        == PathSampleOutcome::Failed(PathProbeFailure::Cancelled);
                    report.cleanup_unconfirmed |= result.cleanup_unconfirmed;
                    report.samples.push(result.sample);
                    if sample_cancelled || cancelled.load(Ordering::Acquire) {
                        report.status = PathComparisonStatus::Cancelled;
                        return report;
                    }
                    if !passed {
                        return report;
                    }
                }
            }
        }
        if Instant::now() >= until {
            return report;
        }
        report.status = PathComparisonStatus::Complete;
        report
    }
}

pub(crate) fn live(session: &SshSession) -> bool {
    !session.is_closed() && session.connection_state() == ConnectionState::Connected
}

#[cfg(test)]
mod tests;
