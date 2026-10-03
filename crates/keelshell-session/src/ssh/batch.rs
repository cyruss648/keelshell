//! One batch row borrows a guarded independent SSH channel through every await.
use super::{SshSession, open_session_until};
use crate::{
    SessionError,
    batch::{
        BatchNotStartedReason as NotStarted, BatchOptions, BatchOutcome as Outcome,
        BatchRowReceipt, BatchUnknownReason as Unknown, cancelled,
    },
};
use russh::ChannelMsg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    sync::watch,
    time::{Instant, sleep_until},
};
use uuid::Uuid;

impl SshSession {
    pub(crate) async fn batch_exec(
        &self,
        id: Uuid,
        command: String,
        options: BatchOptions,
        mut cancel: watch::Receiver<bool>,
        failed: Arc<AtomicBool>,
    ) -> BatchRowReceipt {
        let until = Instant::now() + options.timeout;
        let mut row = BatchRowReceipt {
            id,
            outcome: Outcome::NotStarted {
                reason: NotStarted::ConnectionLost,
            },
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let opening = tokio::select! {
            biased;
            _=cancelled(&mut cancel)=>Err(NotStarted::Cancelled),
            _=sleep_until(until)=>Err(NotStarted::Timeout),
            result=open_session_until(self,until)=>result.map_err(|error|match error {
                SessionError::Timeout(_)=>NotStarted::Timeout,
                SessionError::Ssh(russh::Error::ChannelOpenFailure(_))=>NotStarted::ChannelRejected,
                _=>NotStarted::ConnectionLost,
            }),
        };
        let mut pending = match opening {
            Ok(pending) => pending,
            Err(reason) => {
                row.outcome = Outcome::NotStarted { reason };
                failed.store(true, Ordering::Release);
                return row;
            }
        };
        let Some(channel) = pending.channel.as_mut() else {
            failed.store(true, Ordering::Release);
            return row;
        };
        let mut issued = false;
        let work = async {
            // Set only when the send future is actually polled. Cancellation
            // winning before that is provably NotStarted; once polled, a queued
            // request may reach the peer even if send later returns an error.
            issued = true;
            channel
                .exec(true, command.into_bytes())
                .await
                .map_err(|_| Unknown::ConnectionLost)?;
            // No stdin is supplied. EOF lets noninteractive commands that read
            // stdin finish; this does not request a PTY or a shell startup mode.
            channel.eof().await.map_err(|_| Unknown::ConnectionLost)?;
            let mut accepted = false;
            let mut status = None;
            let mut data_seen = false;
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Success => {
                        if accepted {
                            return Err(Unknown::Protocol);
                        }
                        accepted = true;
                    }
                    ChannelMsg::Failure => {
                        return if accepted || status.is_some() || data_seen {
                            Err(Unknown::Protocol)
                        } else {
                            Ok(Outcome::Rejected)
                        };
                    }
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, ext: 0 } => {
                        data_seen = true;
                        append(&mut row.stdout, &row.stderr, &data, options.output_limit)?
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        data_seen = true;
                        append(&mut row.stderr, &row.stdout, &data, options.output_limit)?
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        if status.replace(exit_status).is_some() {
                            return Err(Unknown::Protocol);
                        }
                    }
                    ChannelMsg::ExitSignal { .. } => return Err(Unknown::RemoteSignal),
                    ChannelMsg::Close => {
                        return status
                            .map(|code| Outcome::Exited { code })
                            .ok_or(Unknown::NoExitStatus);
                    }
                    _ => {}
                }
            }
            Err(Unknown::ConnectionLost)
        };
        let result = tokio::select! {
            biased;
            _=cancelled(&mut cancel)=>Err(Unknown::Cancelled),
            _=sleep_until(until)=>Err(Unknown::Timeout),
            result=work=>result,
        };
        row.outcome = match result {
            Ok(outcome) => outcome,
            Err(Unknown::Cancelled) if !issued => Outcome::NotStarted {
                reason: NotStarted::Cancelled,
            },
            Err(Unknown::Timeout) if !issued => Outcome::NotStarted {
                reason: NotStarted::Timeout,
            },
            Err(reason) => Outcome::Unknown { reason },
        };
        if !row.outcome.is_success() {
            failed.store(true, Ordering::Release);
        }
        pending.close().await;
        row
    }
}
fn append(target: &mut Vec<u8>, other: &[u8], bytes: &[u8], limit: usize) -> Result<(), Unknown> {
    let remaining = limit.saturating_sub(target.len() + other.len());
    let count = remaining.min(bytes.len());
    // Exact growth avoids geometric spare capacity doubling the aggregate
    // capture allocation. The event and final receipt share this one buffer.
    target.reserve_exact(count);
    target.extend_from_slice(&bytes[..count]);
    if count < bytes.len() {
        Err(Unknown::OutputLimit)
    } else {
        Ok(())
    }
}
