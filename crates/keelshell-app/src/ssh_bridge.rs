//! Bridge an SSH shell to the native terminal without blocking GPUI.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

use gpui_kit::{App, AppContext, Entity};
use keelshell_session::{
    ConnectionEnd, ConnectionState, SessionEvent, ShellEnd, SshSession, SshShell,
};
use tokio::runtime::Runtime;
use tokio::sync::watch;

use crate::terminal::{TerminalCommand, TerminalView, TransportState, spawn_transport_worker};

/// Open one SSH terminal entity. The connection is shared with independent SFTP
/// and terminal consumers; closing this view closes only its shell channel.
pub fn open(
    session: SshSession,
    runtime: Arc<Runtime>,
    title: String,
    font_size: f32,
    history: usize,
    cx: &mut App,
) -> Entity<TerminalView> {
    let (output, incoming) = mpsc::sync_channel(128);
    let (outgoing, commands) = mpsc::sync_channel(128);
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_stop = cancelled.clone();
    let failure_output = output.clone();
    let (lifecycle, state) = watch::channel(TransportState::Starting);
    let started = spawn_transport_worker("keelshell-ssh-shell", cancelled.clone(), move || {
        runtime.block_on(run(session, commands, output, worker_stop, lifecycle))
    });
    if let Err(error) = started {
        // TerminalView supplies the translated error label; preserve this
        // operating-system diagnostic without translating a host or title.
        let _ = failure_output.try_send(SessionEvent::Error(error.to_string()));
        cancelled.store(true, Ordering::Release);
    }
    cx.new(|cx| {
        let mut view = TerminalView::from_transport(
            title, font_size, history, incoming, outgoing, cancelled, cx,
        );
        view.attach_lifecycle(state);
        view
    })
}

async fn run(
    session: SshSession,
    commands: Receiver<TerminalCommand>,
    output: SyncSender<SessionEvent>,
    cancelled: Arc<AtomicBool>,
    lifecycle: watch::Sender<TransportState>,
) -> Result<(), String> {
    let startup = tokio::select! {
        _ = cancellation(&cancelled) => { lifecycle.send_replace(TransportState::Ended(ShellEnd::Cancelled)); return Ok(()); },
        result = session.start_shell(30, 100) => result,
    };
    let mut shell = match startup {
        Ok(shell) => shell,
        Err(error) => {
            lifecycle.send_replace(TransportState::Ended(connection_end(&session)));
            let message = error.to_string();
            let _ = output.try_send(SessionEvent::Error(message.clone()));
            return Err(message);
        }
    };
    lifecycle.send_replace(TransportState::Ready);
    let result = pump(&mut shell, &commands, &output, &cancelled, &lifecycle).await;
    let reason = shell.completion().unwrap_or_else(|| {
        if cancelled.load(Ordering::Acquire) {
            ShellEnd::Cancelled
        } else {
            connection_end(&session)
        }
    });
    lifecycle.send_replace(TransportState::Ended(reason));
    // Cleanup has its own bounded deadline and is not interrupted by the view's
    // cancellation signal. The worker registry observes this final outcome.
    let cleanup = tokio::time::timeout(Duration::from_secs(2), shell.close())
        .await
        .map_err(|_| "SSH_SHELL_CLEANUP_TIMEOUT".to_owned())
        .and_then(|result| result.map_err(|error| error.to_string()));
    result.and(cleanup)
}

async fn pump(
    shell: &mut SshShell,
    commands: &Receiver<TerminalCommand>,
    output: &SyncSender<SessionEvent>,
    cancelled: &AtomicBool,
    lifecycle: &watch::Sender<TransportState>,
) -> Result<(), String> {
    let mut pending_output = None;
    let mut completion_seen = false;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Ok(());
        }

        if !completion_seen && let Some(reason) = shell.completion() {
            lifecycle.send_replace(TransportState::Ended(reason));
            completion_seen = true;
        }
        if drain_output(shell, &mut pending_output, output)? {
            return Ok(());
        }

        if completion_seen {
            tokio::time::sleep(Duration::from_millis(4)).await;
            continue;
        }

        // Merge consecutive resize requests, but preserve their position before
        // the next write. No write is dequeued until this worker can own it.
        let mut resize = None;
        let mut write = None;
        for _ in 0..128 {
            match commands.try_recv() {
                Ok(TerminalCommand::Resize(rows, cols)) => resize = Some((rows, cols)),
                Ok(TerminalCommand::Write(bytes)) => {
                    write = Some(bytes);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if let Some((rows, cols)) = resize {
            let mut completion = shell.subscribe_completion();
            tokio::select! {
                biased;
                _ = cancellation(cancelled) => return Ok(()),
                reason = shell_end(&mut completion) => {
                    lifecycle.send_replace(TransportState::Ended(reason));
                    completion_seen = true;
                    continue;
                },
                result = shell.resize(rows, cols) => result.map_err(|error| error.to_string())?,
            }
        }
        if let Some(bytes) = write {
            // A timeout may have written a prefix, so the bridge reports failure
            // instead of replaying bytes and risking a duplicate command.
            let writer = shell.writer();
            let writing = writer.write(&bytes);
            let mut completion = shell.subscribe_completion();
            tokio::pin!(writing);
            loop {
                // SSH flow control may suspend input until remote output is
                // consumed. Drain independently while preserving this one
                // write future: cancelling/restarting it could duplicate bytes.
                tokio::select! {
                    biased;
                    _ = cancellation(cancelled) => return Ok(()),
                    reason = shell_end(&mut completion) => {
                        lifecycle.send_replace(TransportState::Ended(reason));
                        completion_seen = true;
                        break;
                    },
                    result = &mut writing => {
                        if let Some(reason) = shell.completion() {
                            lifecycle.send_replace(TransportState::Ended(reason));
                            completion_seen = true;
                        } else {
                            result.map_err(|error| error.to_string())?;
                        }
                        break;
                    },
                    _ = tokio::time::sleep(Duration::from_millis(4)) => {
                        if let Some(reason) = shell.completion() {
                            lifecycle.send_replace(TransportState::Ended(reason));
                            completion_seen = true;
                            break;
                        }
                        if drain_output(shell, &mut pending_output, output)? { return Ok(()); }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(4)).await;
    }
}

fn drain_output(
    shell: &mut SshShell,
    pending: &mut Option<SessionEvent>,
    output: &SyncSender<SessionEvent>,
) -> Result<bool, String> {
    // One retained event plus two bounded queues provide output backpressure.
    for _ in 0..64 {
        let Some(event) = pending.take().or_else(|| shell.try_recv()) else {
            break;
        };
        let completed = matches!(event, SessionEvent::Exited { .. });
        let failed = match &event {
            SessionEvent::Error(message) if shell.completion().is_none() => Some(message.clone()),
            _ => None,
        };
        let completed =
            completed || matches!(event, SessionEvent::Error(_)) && shell.completion().is_some();
        match output.try_send(event) {
            Ok(()) => {
                if let Some(error) = failed {
                    return Err(error);
                }
                if completed {
                    return Ok(true);
                }
            }
            Err(TrySendError::Full(event)) => {
                *pending = Some(event);
                break;
            }
            Err(TrySendError::Disconnected(_)) => return Ok(true),
        }
    }
    Ok(false)
}

async fn cancellation(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn shell_end(completion: &mut watch::Receiver<Option<ShellEnd>>) -> ShellEnd {
    loop {
        if let Some(reason) = completion.borrow_and_update().clone() {
            return reason;
        }
        if completion.changed().await.is_err() {
            return ShellEnd::ConnectionClosed(ConnectionEnd::Unknown);
        }
    }
}

fn connection_end(session: &SshSession) -> ShellEnd {
    ShellEnd::ConnectionClosed(match session.connection_state() {
        ConnectionState::Closed(reason) => reason,
        ConnectionState::Closing => ConnectionEnd::LocalClosed,
        ConnectionState::Connected => ConnectionEnd::Unknown,
    })
}

#[cfg(test)]
#[path = "ssh_bridge_tests.rs"]
mod tests;
