//! Probe-specific bounded protocol; included as an SSH child to borrow its open owner.

use super::{SshSession, open_session_until};
use crate::{
    SessionError,
    path_comparison::{
        EXEC_REPLY, PAYLOAD_BYTE, PAYLOAD_BYTES, PathProbeFailure as Failure, PathProbeKind,
        PathSample, PathSampleOutcome, PathSide, SAMPLE_TIMEOUT, UPLOAD_REPLY, live,
    },
};
use russh::{Channel, ChannelMsg, client};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    time::{Instant, sleep, sleep_until, timeout_at},
};

// Separate from archive/diagnostic limits; output is compared as it arrives and
// never retained. The cap includes all stdout and extended-data bytes.
const MAX_OUTPUT_BYTES: usize = PAYLOAD_BYTES + 128;
static INPUT_CHUNK: [u8; 16 * 1024] = [PAYLOAD_BYTE; 16 * 1024];

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

pub(crate) struct SampleResult {
    pub(crate) sample: PathSample,
    pub(crate) cleanup_unconfirmed: bool,
}

pub(crate) async fn sample(
    session: &SshSession,
    path: PathSide,
    kind: PathProbeKind,
    round: u8,
    comparison_until: Instant,
    cancelled: &AtomicBool,
) -> SampleResult {
    let started = Instant::now();
    let until = (started + SAMPLE_TIMEOUT).min(comparison_until);
    let deadline_failure = if comparison_until <= started + SAMPLE_TIMEOUT {
        Failure::ComparisonDeadline
    } else {
        Failure::SampleDeadline
    };
    let mut result = SampleResult {
        sample: PathSample {
            path,
            kind,
            round,
            elapsed: Duration::ZERO,
            bytes: 0,
            outcome: PathSampleOutcome::Failed(Failure::Transport),
        },
        cleanup_unconfirmed: false,
    };
    let (outcome, bytes, unknown) =
        execute(session, kind, until, deadline_failure, cancelled).await;
    result.sample.outcome = outcome;
    result.sample.bytes = bytes;
    result.sample.elapsed = started.elapsed();
    result.cleanup_unconfirmed = unknown;
    result
}

async fn execute(
    session: &SshSession,
    kind: PathProbeKind,
    until: Instant,
    deadline_failure: Failure,
    cancelled: &AtomicBool,
) -> (PathSampleOutcome, usize, bool) {
    let failed = |failure, unknown| (PathSampleOutcome::Failed(failure), 0, unknown);
    if cancelled.load(Ordering::Acquire) {
        return failed(Failure::Cancelled, false);
    }
    if !live(session) {
        return failed(Failure::ConnectionClosed, false);
    }
    if Instant::now() >= until {
        return failed(deadline_failure, false);
    }

    let admitted = AtomicBool::new(false);
    let opening = async {
        if cancelled.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        admitted.store(true, Ordering::Release);
        open_session_until(session, until).await
    };
    tokio::pin!(opening);
    // Keep the same open future alive after cancellation, to collect late-open
    // cleanup inside its original budget. No late confirmation can grant EXEC.
    let opened = tokio::select! {
        biased;
        _ = wait_cancelled(cancelled) => {
            // Cancellation before the opening future's first poll grants no OPEN.
            if !admitted.load(Ordering::Acquire) {
                return failed(Failure::Cancelled, false);
            }
            timeout_at(until, &mut opening).await
        },
        value = timeout_at(until, &mut opening) => value,
    };
    let mut owned = match opened {
        Ok(Ok(owned)) => owned,
        Ok(Err(error)) => {
            let failure = if cancelled.load(Ordering::Acquire) {
                Failure::Cancelled
            } else {
                match &error {
                    SessionError::Timeout(_) => deadline_failure,
                    SessionError::Ssh(russh::Error::ChannelOpenFailure(_)) => {
                        Failure::ChannelRejected
                    }
                    _ => Failure::Transport,
                }
            };
            return failed(failure, matches!(error, SessionError::Timeout(_)));
        }
        Err(_) => {
            return failed(
                if cancelled.load(Ordering::Acquire) {
                    Failure::Cancelled
                } else {
                    deadline_failure
                },
                true,
            );
        }
    };

    let mut protocol = Protocol::new(kind);
    let written = AtomicUsize::new(0);
    let mut issued = false;
    let operation = async {
        if cancelled.load(Ordering::Acquire) {
            return Err(Failure::Cancelled);
        }
        if !live(session) {
            return Err(Failure::ConnectionClosed);
        }
        if Instant::now() >= until {
            return Err(deadline_failure);
        }
        let channel = owned.channel.as_mut().ok_or(Failure::Transport)?;
        // Mark before polling send: an interrupted enqueue may already be in
        // the protocol queue. A missing ACK cannot establish non-dispatch.
        issued = true;
        channel
            .exec(true, kind.command())
            .await
            .map_err(|_| Failure::Transport)?;
        while !protocol.acknowledged && !protocol.closed && !protocol.rejected {
            let message = channel.wait().await;
            if message.is_none() {
                protocol.fail(Failure::MissingClose);
                return Ok(());
            }
            protocol.observe(message);
        }
        if protocol.rejected || protocol.closed {
            return Ok(());
        }
        if protocol.failure.is_some() {
            // Never supply stdin after an invalid acknowledgement sequence.
            drain(channel, &mut protocol).await;
            return Ok(());
        }
        let mut writer = channel.make_writer();
        let send = async {
            if kind == PathProbeKind::Upload {
                while written.load(Ordering::Acquire) < PAYLOAD_BYTES {
                    let remaining = PAYLOAD_BYTES - written.load(Ordering::Acquire);
                    let count = writer
                        .write(&INPUT_CHUNK[..remaining.min(INPUT_CHUNK.len())])
                        .await
                        .map_err(|_| Failure::Transport)?;
                    if count == 0 {
                        return Err(Failure::IncompleteInput);
                    }
                    written.fetch_add(count, Ordering::Release);
                }
            }
            writer.shutdown().await.map_err(|_| Failure::Transport)
        };
        // The reader runs while window-limited stdin is being written. No
        // capture buffer grows with output, including during failure cleanup.
        let (sent, ()) = tokio::join!(send, drain(channel, &mut protocol));
        sent?;
        Ok(())
    };
    let completion = tokio::select! {
        biased;
        _ = wait_cancelled(cancelled) => Err(Failure::Cancelled),
        _ = sleep_until(until) => Err(deadline_failure),
        result = operation => result,
    };
    // EOF + explicit process termination + CLOSE establish completion even for
    // an invalid sample. A locally enqueued CLOSE cannot establish it.
    let remote_finished = protocol.finished() || protocol.rejected;
    let bytes = match kind {
        PathProbeKind::Exec => 0,
        PathProbeKind::Upload => written.load(Ordering::Acquire).min(PAYLOAD_BYTES),
        PathProbeKind::Download => protocol.stdout_bytes.min(PAYLOAD_BYTES),
    };
    let mut outcome = match completion {
        Err(failure) => PathSampleOutcome::Failed(failure),
        Ok(()) => protocol.outcome(),
    };
    let local_cleanup = if protocol.closed {
        // russh acknowledges peer CLOSE and removes its channel before delivery.
        // Disarming avoids substituting a redundant local CLOSE for that proof.
        owned.channel.take();
        true
    } else {
        timeout_at(until, owned.close()).await.is_ok()
    };
    if cancelled.load(Ordering::Acquire) {
        outcome = PathSampleOutcome::Failed(Failure::Cancelled);
    } else if Instant::now() >= until {
        outcome = PathSampleOutcome::Failed(deadline_failure);
    }
    (
        outcome,
        bytes,
        !local_cleanup || (issued && !remote_finished),
    )
}

async fn drain(channel: &mut Channel<client::Msg>, protocol: &mut Protocol) {
    while !protocol.closed {
        let message = channel.wait().await;
        if message.is_none() {
            protocol.fail(Failure::MissingClose);
            break;
        }
        protocol.observe(message);
    }
}

async fn wait_cancelled(cancelled: &AtomicBool) {
    loop {
        if cancelled.load(Ordering::Acquire) {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
}

struct Protocol {
    kind: PathProbeKind,
    acknowledged: bool,
    rejected: bool,
    eof: bool,
    closed: bool,
    exit: Option<u32>,
    signalled: bool,
    stdout_bytes: usize,
    total_bytes: usize,
    failure: Option<Failure>,
}

impl Protocol {
    fn new(kind: PathProbeKind) -> Self {
        Self {
            kind,
            acknowledged: false,
            rejected: false,
            eof: false,
            closed: false,
            exit: None,
            signalled: false,
            stdout_bytes: 0,
            total_bytes: 0,
            failure: None,
        }
    }

    fn fail(&mut self, failure: Failure) {
        if failure == Failure::OutputLimit {
            self.failure = Some(failure);
        } else {
            self.failure.get_or_insert(failure);
        }
    }

    fn finished(&self) -> bool {
        self.eof && self.closed && (self.exit.is_some() || self.signalled)
    }

    fn observe(&mut self, message: Option<ChannelMsg>) {
        match message {
            Some(ChannelMsg::Success) if !self.acknowledged && !self.rejected => {
                self.acknowledged = true;
            }
            Some(ChannelMsg::Failure)
                if !self.acknowledged
                    && self.total_bytes == 0
                    && self.exit.is_none()
                    && !self.eof =>
            {
                self.rejected = true;
                self.fail(Failure::ExecRejected);
            }
            Some(ChannelMsg::Data { data }) => {
                if !self.acknowledged {
                    self.fail(Failure::MissingAcknowledgement);
                }
                if self.eof || self.closed {
                    self.fail(Failure::InvalidSequence);
                }
                self.total_bytes = self.total_bytes.saturating_add(data.len());
                if self.total_bytes > MAX_OUTPUT_BYTES {
                    self.fail(Failure::OutputLimit);
                }
                let matches = match self.kind {
                    PathProbeKind::Download => {
                        self.stdout_bytes.saturating_add(data.len()) <= PAYLOAD_BYTES
                            && data.iter().all(|byte| *byte == PAYLOAD_BYTE)
                    }
                    PathProbeKind::Exec | PathProbeKind::Upload => {
                        let expected = if self.kind == PathProbeKind::Exec {
                            EXEC_REPLY
                        } else {
                            UPLOAD_REPLY
                        };
                        expected
                            .get(self.stdout_bytes..self.stdout_bytes.saturating_add(data.len()))
                            == Some(data.as_ref())
                    }
                };
                self.stdout_bytes = self.stdout_bytes.saturating_add(data.len());
                if !matches {
                    self.fail(Failure::InvalidStdout);
                }
            }
            Some(ChannelMsg::ExtendedData { data, .. }) => {
                self.total_bytes = self.total_bytes.saturating_add(data.len());
                self.fail(if self.total_bytes > MAX_OUTPUT_BYTES {
                    Failure::OutputLimit
                } else {
                    Failure::UnexpectedStderr
                });
            }
            Some(ChannelMsg::ExitStatus { exit_status })
                if self.exit.is_none() && !self.signalled =>
            {
                if !self.acknowledged {
                    self.fail(Failure::MissingAcknowledgement);
                }
                self.exit = Some(exit_status);
            }
            Some(ChannelMsg::ExitSignal { .. }) if self.exit.is_none() && !self.signalled => {
                self.signalled = true;
                self.fail(Failure::RemoteSignal);
            }
            Some(ChannelMsg::Eof) if !self.eof => self.eof = true,
            Some(ChannelMsg::Close) => self.closed = true,
            Some(ChannelMsg::WindowAdjusted { .. }) => {}
            None => self.fail(Failure::MissingClose),
            _ => self.fail(Failure::InvalidSequence),
        }
    }

    fn outcome(&self) -> PathSampleOutcome {
        let failure = self.failure.or_else(|| {
            if !self.acknowledged {
                Some(Failure::MissingAcknowledgement)
            } else if !self.eof {
                Some(Failure::MissingEof)
            } else if !self.closed {
                Some(Failure::MissingClose)
            } else if self.exit.is_none() {
                Some(Failure::MissingExitStatus)
            } else if self.exit != Some(0) {
                Some(Failure::UnsuccessfulExit)
            } else if self.stdout_bytes
                != match self.kind {
                    PathProbeKind::Exec => EXEC_REPLY.len(),
                    PathProbeKind::Upload => UPLOAD_REPLY.len(),
                    PathProbeKind::Download => PAYLOAD_BYTES,
                }
            {
                Some(Failure::InvalidStdout)
            } else {
                None
            }
        });
        failure.map_or(PathSampleOutcome::Passed, PathSampleOutcome::Failed)
    }
}
