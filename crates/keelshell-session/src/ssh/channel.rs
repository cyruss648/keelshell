//! Channel ownership starts before the protocol open can be enqueued.

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use russh::{Channel, client};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};
use tokio::sync::{oneshot, watch};
use tokio::time::{Instant, timeout, timeout_at};

use super::SshSession;
use crate::{Result, SessionError};

const CLOSE_BUDGET: Duration = Duration::from_secs(2);

pub(super) struct PendingChannel {
    pub(super) channel: Option<Channel<client::Msg>>,
    session: SshSession,
    runtime: tokio::runtime::Handle,
}

impl PendingChannel {
    fn new(channel: Channel<client::Msg>, session: SshSession) -> Self {
        Self {
            channel: Some(channel),
            session,
            runtime: tokio::runtime::Handle::current(),
        }
    }

    pub(super) async fn close(mut self) {
        if let Some(channel) = self.channel.as_ref() {
            // Retain the guard through await: callers can cancel even while the
            // protocol queue is backpressured. Drop then transfers cleanup to
            // an independent bounded owner instead of dropping a bare channel.
            close_channel(channel, &self.session).await;
        }
        self.channel = None;
    }

    /// Keep the real channel inside one relay owner through initialization and
    /// its complete stream lifetime. A fixed buffer bounds backpressure.
    pub(super) fn into_stream(mut self) -> (OwnedChannelStream, ChannelStreamOwner) {
        let (stream, mut bridge) = tokio::io::duplex(64 * 1024);
        let (cancel, mut cancelled) = watch::channel(false);
        self.runtime.clone().spawn(async move {
            if let Some(channel) = self.channel.as_mut() {
                // The writer is independent; the reader borrows the guarded
                // channel only until relay exit, before explicit CLOSE below.
                let writer = channel.make_writer();
                let reader = channel.make_reader();
                let mut remote = tokio::io::join(reader, writer);
                tokio::select! {
                    biased;
                    _ = cancelled.changed() => {},
                    _ = tokio::io::copy_bidirectional(&mut bridge, &mut remote) => {},
                }
            }
            self.close().await;
        });
        (
            OwnedChannelStream {
                stream,
                cancel: cancel.clone(),
            },
            ChannelStreamOwner { cancel },
        )
    }
}

/// High-level lifetime control, independent of the upstream raw write queue.
pub(crate) struct ChannelStreamOwner {
    cancel: watch::Sender<bool>,
}
impl ChannelStreamOwner {
    pub(crate) fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    pub(super) fn is_closed(&self) -> bool {
        *self.cancel.borrow() || self.cancel.is_closed()
    }
}
impl Drop for ChannelStreamOwner {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Raw SFTP plus an owner that can stop a backpressured relay without waiting
/// for russh-sftp's queued close sentinel to reach its blocked writer.
pub(crate) struct OwnedRawSftpSession {
    raw: russh_sftp::client::RawSftpSession,
    owner: ChannelStreamOwner,
}
impl OwnedRawSftpSession {
    pub(super) fn new(raw: russh_sftp::client::RawSftpSession, owner: ChannelStreamOwner) -> Self {
        Self { raw, owner }
    }
    pub(crate) fn close_session(
        &self,
    ) -> std::result::Result<(), russh_sftp::client::error::Error> {
        self.owner.cancel();
        self.raw.close_session()
    }
}
impl std::ops::Deref for OwnedRawSftpSession {
    type Target = russh_sftp::client::RawSftpSession;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

pub(super) struct OwnedChannelStream {
    stream: DuplexStream,
    cancel: watch::Sender<bool>,
}
impl Drop for OwnedChannelStream {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}
impl AsyncRead for OwnedChannelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}
impl AsyncWrite for OwnedChannelStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buffer)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl Drop for PendingChannel {
    fn drop(&mut self) {
        if let Some(channel) = self.channel.take() {
            let session = self.session.clone();
            self.runtime.spawn(async move {
                close_channel(&channel, &session).await;
            });
        }
    }
}

async fn close_channel(channel: &Channel<client::Msg>, session: &SshSession) {
    // Success transfers CLOSE into the owned protocol queue; russh removes its
    // channel receiver there, so this cannot claim a remote acknowledgement.
    if !matches!(timeout(CLOSE_BUDGET, channel.close()).await, Ok(Ok(()))) {
        let _ = session.close_or_abort().await;
    }
}

/// An independent owner drains an admitted open even if the caller disappears.
/// The oneshot payload itself owns cleanup, including cancellation after send.
pub(super) async fn open_session(session: &SshSession) -> Result<PendingChannel> {
    let until = Instant::now()
        .checked_add(session.timeout)
        .ok_or(SessionError::Invalid("SSH timeout exceeds the clock range"))?;
    open(session, OpenKind::Session, until).await
}

pub(super) async fn open_session_until(
    session: &SshSession,
    until: Instant,
) -> Result<PendingChannel> {
    open(session, OpenKind::Session, until).await
}

pub(super) async fn open_direct(
    session: &SshSession,
    host: String,
    port: u16,
    until: Instant,
) -> Result<PendingChannel> {
    open(session, OpenKind::Direct { host, port }, until).await
}

enum OpenKind {
    Session,
    Direct { host: String, port: u16 },
}

async fn open(session: &SshSession, kind: OpenKind, until: Instant) -> Result<PendingChannel> {
    let session = session.clone();
    let (mut sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let admitted = AtomicBool::new(false);
        let opening = async {
            admitted.store(true, Ordering::Release);
            match kind {
                OpenKind::Session => session.handle.channel_open_session().await,
                OpenKind::Direct { host, port } => {
                    session
                        .handle
                        .channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0)
                        .await
                }
            }
        };
        tokio::pin!(opening);
        let result = tokio::select! {
            biased;
            _ = sender.closed() => {
                // Never initiate a request merely to clean up an unpolled future.
                if !admitted.load(Ordering::Acquire) { return; }
                match timeout_at(until, &mut opening).await {
                    Ok(Ok(channel)) => PendingChannel::new(channel, session.clone()).close().await,
                    Ok(Err(_)) => {},
                    Err(_) => { let _ = session.close_or_abort().await; },
                }
                return;
            }
            result = timeout_at(until, &mut opening) => result,
        };
        match result {
            Ok(Ok(channel)) => {
                let owned = PendingChannel::new(channel, session.clone());
                if let Err(Ok(owned)) = sender.send(Ok(owned)) {
                    owned.close().await;
                }
            }
            Ok(Err(error)) => {
                let _ = sender.send(Err(error.into()));
            }
            Err(_) => {
                // Keep `opening` alive until the shared transport cannot receive
                // a late confirmation with no owner, even if disconnect stalls.
                let _ = session.close_or_abort().await;
                let _ = sender.send(Err(SessionError::Timeout("SSH channel open")));
            }
        }
    });
    receiver.await.map_err(|_| SessionError::Closed)?
}

#[cfg(test)]
mod tests;
