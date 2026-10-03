//! Bounded, loopback-only SOCKS5 CONNECT over SSH direct-tcpip channels.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use russh::{ChannelOpenFailure, client};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{Instant, timeout_at};

use crate::{Result, SessionError, SshSession};

/// Resource limits for a local SOCKS5 proxy. These bounds apply independently
/// to each listener; established streams have no idle timeout.
#[derive(Clone, Copy, Debug)]
pub struct DynamicForwardOptions {
    /// Total greeting, request and SSH channel-open deadline (nonzero, <= 60 s),
    /// further limited by the owning SSH session's operation timeout.
    pub handshake_timeout: Duration,
    /// Maximum simultaneous clients, including incomplete handshakes (1–256).
    /// Excess TCP connections are closed without admitting another handshake.
    pub max_connections: usize,
}

impl Default for DynamicForwardOptions {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(10),
            max_connections: 64,
        }
    }
}

/// A local, unauthenticated SOCKS5 TCP CONNECT proxy through one SSH session.
/// IPv4, IPv6 and domain targets are supported; domain resolution is remote.
/// BIND, UDP ASSOCIATE and authentication methods other than no-auth are rejected.
///
/// Drop requests cleanup; [`Self::close`] also waits for it. An unconfirmed SSH
/// channel open is drained until its original deadline so late confirmation can
/// be closed. If no confirmation arrives, cleanup requests shared SSH disconnect
/// and waits up to two seconds before forcibly shutting down the shared TCP
/// socket. Enqueueing known channel closes is likewise bounded. Cleanup failures are
/// returned explicitly; remote cleanup is never assumed.
pub struct DynamicForward {
    address: SocketAddr,
    cancel: watch::Sender<bool>,
    task: JoinHandle<Result<()>>,
}

impl DynamicForward {
    /// Actual loopback address, including an allocated port for a port-0 bind.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Whether the listener owner has completed cleanup. Use [`Self::close`]
    /// to retrieve any terminal listener or unconfirmed-channel failure.
    pub fn is_closed(&self) -> bool {
        self.task.is_finished()
    }

    /// Release the listener and all clients, waiting for owned cleanup tasks.
    pub async fn close(mut self) -> Result<()> {
        self.cancel.send_replace(true);
        (&mut self.task).await.map_err(|_| SessionError::Worker)?
    }
}

impl Drop for DynamicForward {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

impl SshSession {
    /// Start a no-auth SOCKS5 CONNECT proxy with default resource limits.
    /// Only an explicit loopback address is accepted; port 0 allocates a port.
    pub async fn forward_dynamic(&self, bind: SocketAddr) -> Result<DynamicForward> {
        self.forward_dynamic_with_options(bind, DynamicForwardOptions::default())
            .await
    }

    /// Start a SOCKS5 CONNECT proxy with bounded handshakes and concurrency.
    /// Neither the proxy nor the local device resolves SOCKS domain targets.
    pub async fn forward_dynamic_with_options(
        &self,
        bind: SocketAddr,
        options: DynamicForwardOptions,
    ) -> Result<DynamicForward> {
        if !bind.ip().is_loopback() {
            return Err(SessionError::Invalid("SOCKS5 listener must use loopback"));
        }
        if options.handshake_timeout.is_zero()
            || options.handshake_timeout > Duration::from_secs(60)
            || !(1..=256).contains(&options.max_connections)
        {
            return Err(SessionError::Invalid("invalid SOCKS5 resource limits"));
        }
        if self.is_closed() {
            return Err(SessionError::Closed);
        }
        let listener = TcpListener::bind(bind).await?;
        let address = listener.local_addr()?;
        let (cancel, mut cancelled) = watch::channel(false);
        let worker_cancel = cancel.clone();
        let session = self.clone();
        let task = tokio::spawn(async move {
            let mut streams = JoinSet::new();
            let unconfirmed = Arc::new(AtomicBool::new(false));
            let mut health = tokio::time::interval(Duration::from_millis(50));
            let listener_error = loop {
                tokio::select! {
                    biased;
                    _ = cancellation(&mut cancelled) => break None,
                    _ = health.tick() => { if session.is_closed() || unconfirmed.load(Ordering::Acquire) { break None; } },
                    _ = streams.join_next(), if !streams.is_empty() => {},
                    accepted = listener.accept() => {
                        let (socket, peer) = match accepted { Ok(accepted) => accepted, Err(error) => break Some(error) };
                        if streams.len() >= options.max_connections { continue; }
                        streams.spawn(serve_client(socket, peer, session.clone(), cancelled.clone(), options, unconfirmed.clone()));
                    },
                }
            };
            drop(listener);
            worker_cancel.send_replace(true);
            // Do not abort a pending channel-open: its ID is unavailable until
            // acknowledgement. Each client drains it or disconnects at deadline.
            while streams.join_next().await.is_some() {}
            if unconfirmed.load(Ordering::Acquire) {
                Err(SessionError::Timeout(if session.is_closed() {
                    "SOCKS5 SSH channel cleanup; shared SSH disconnected"
                } else {
                    "SOCKS5 SSH channel cleanup; shared SSH disconnect not confirmed"
                }))
            } else if let Some(error) = listener_error {
                Err(SessionError::Io(error))
            } else {
                Ok(())
            }
        });
        Ok(DynamicForward {
            address,
            cancel,
            task,
        })
    }
}

async fn cancellation(cancelled: &mut watch::Receiver<bool>) {
    while !*cancelled.borrow_and_update() {
        if cancelled.changed().await.is_err() {
            break;
        }
    }
}

async fn serve_client(
    mut socket: TcpStream,
    peer: SocketAddr,
    session: SshSession,
    mut cancelled: watch::Receiver<bool>,
    options: DynamicForwardOptions,
    unconfirmed: Arc<AtomicBool>,
) {
    let until = Instant::now() + options.handshake_timeout.min(session.timeout);
    let target = tokio::select! {
        biased;
        _ = cancellation(&mut cancelled) => return,
        result = timeout_at(until, negotiate(&mut socket)) => match result {
            Ok(Ok(Some(target))) => target,
            _ => return,
        },
    };
    let admitted = AtomicBool::new(false);
    let opening = async {
        admitted.store(true, Ordering::Release);
        session
            .handle
            .channel_open_direct_tcpip(
                target.0,
                u32::from(target.1),
                peer.ip().to_string(),
                u32::from(peer.port()),
            )
            .await
    };
    tokio::pin!(opening);
    let channel = tokio::select! {
        biased;
        _ = cancellation(&mut cancelled) => {
            drop(socket);
            // Cancellation may win before the open future was ever polled.
            // Draining must not initiate a previously unadmitted request.
            if !admitted.load(Ordering::Acquire) { return; }
            match timeout_at(until, &mut opening).await {
                Ok(Ok(channel)) => close_channel(channel, &session, &unconfirmed).await,
                Ok(Err(_)) => {},
                Err(_) => disconnect_unconfirmed(&session, &unconfirmed).await,
            }
            return;
        },
        result = timeout_at(until, &mut opening) => match result {
            Ok(Ok(channel)) => channel,
            Ok(Err(error)) => {
                tokio::select! {
                    biased;
                    _ = cancellation(&mut cancelled) => {},
                    _ = timeout_at(until, reply(&mut socket, open_failure(&error))) => {},
                }
                return;
            },
            Err(_) => {
                drop(socket);
                disconnect_unconfirmed(&session, &unconfirmed).await;
                return;
            },
        },
    };
    relay(
        socket,
        channel,
        until,
        &mut cancelled,
        &session,
        &unconfirmed,
    )
    .await;
}

async fn disconnect_unconfirmed(session: &SshSession, unconfirmed: &AtomicBool) {
    unconfirmed.store(true, Ordering::Release);
    // Graceful disconnect has a bounded physical TCP shutdown fallback; the
    // pending open is not abandoned while a shared transport can remain alive.
    let _ = session.close_or_abort().await;
}

async fn close_channel(
    channel: russh::Channel<client::Msg>,
    session: &SshSession,
    unconfirmed: &AtomicBool,
) {
    // russh removes the local channel table entry when enqueueing CLOSE, so its
    // receiver cannot be used to await a peer acknowledgement afterward. Await
    // ownership transfer to the session's protocol queue, never a detached Drop.
    let closed = tokio::time::timeout(Duration::from_secs(2), channel.close()).await;
    if !matches!(closed, Ok(Ok(()))) && !session.is_closed() {
        disconnect_unconfirmed(session, unconfirmed).await;
    }
}

async fn relay(
    mut socket: TcpStream,
    mut channel: russh::Channel<client::Msg>,
    until: Instant,
    cancelled: &mut watch::Receiver<bool>,
    session: &SshSession,
    unconfirmed: &AtomicBool,
) {
    tokio::select! {
        biased;
        _ = cancellation(cancelled) => {},
        _ = async {
            if matches!(timeout_at(until, reply(&mut socket, 0)).await, Ok(Ok(()))) {
                let (mut local_read, mut local_write) = socket.split();
                let mut remote_write = channel.make_writer();
                let mut remote_read = channel.make_reader();
                let _ = tokio::try_join!(
                    async {
                        tokio::io::copy(&mut local_read, &mut remote_write).await?;
                        remote_write.shutdown().await
                    },
                    async {
                        tokio::io::copy(&mut remote_read, &mut local_write).await?;
                        local_write.shutdown().await
                    }
                );
            }
        } => {},
    }
    drop(socket);
    // Keep the channel owner until its close enters the owned protocol queue
    // (or shared TCP is aborted). ChannelStream Drop alone is only best effort.
    close_channel(channel, session, unconfirmed).await;
}

async fn negotiate(socket: &mut TcpStream) -> io::Result<Option<(String, u16)>> {
    let mut greeting = [0; 2];
    socket.read_exact(&mut greeting).await?;
    if greeting[0] != 5 || greeting[1] == 0 {
        reject(socket, &[5, 0xff]).await?;
        return Ok(None);
    }
    let mut methods = [0; 255];
    let methods = &mut methods[..usize::from(greeting[1])];
    socket.read_exact(methods).await?;
    if !methods.contains(&0) {
        reject(socket, &[5, 0xff]).await?;
        return Ok(None);
    }
    socket.write_all(&[5, 0]).await?;
    let mut header = [0; 4];
    socket.read_exact(&mut header).await?;
    let failure = if header[0] != 5 || header[2] != 0 {
        Some(1)
    } else if header[1] != 1 {
        Some(7)
    } else {
        None
    };
    if let Some(code) = failure {
        reply(socket, code).await?;
        return Ok(None);
    }
    let host = match header[3] {
        1 => {
            let mut address = [0; 4];
            socket.read_exact(&mut address).await?;
            Ipv4Addr::from(address).to_string()
        }
        4 => {
            let mut address = [0; 16];
            socket.read_exact(&mut address).await?;
            Ipv6Addr::from(address).to_string()
        }
        3 => {
            let length = usize::from(socket.read_u8().await?);
            let mut domain = [0; 255];
            socket.read_exact(&mut domain[..length]).await?;
            // SOCKS has no character encoding negotiation. Accept ASCII DNS
            // names (including punycode); never silently normalize or resolve.
            if length == 0
                || !domain[..length]
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
            {
                reply(socket, 8).await?;
                return Ok(None);
            }
            String::from_utf8(domain[..length].to_vec())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?
        }
        _ => {
            reply(socket, 8).await?;
            return Ok(None);
        }
    };
    let port = socket.read_u16().await?;
    if port == 0 {
        reply(socket, 1).await?;
        return Ok(None);
    }
    Ok(Some((host, port)))
}

async fn reply(socket: &mut TcpStream, code: u8) -> io::Result<()> {
    // direct-tcpip does not report the remote socket's bound address. The
    // unspecified BND endpoint is deliberate; it must not pretend to be local.
    let response = [5, code, 0, 1, 0, 0, 0, 0, 0, 0];
    if code == 0 {
        socket.write_all(&response).await
    } else {
        reject(socket, &response).await
    }
}

const REJECT_DRAIN_BYTES: usize = 4096;
const REJECT_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

async fn reject(
    socket: &mut (impl AsyncRead + AsyncWrite + Unpin),
    response: &[u8],
) -> io::Result<()> {
    socket.write_all(response).await?;
    // Closing a socket with an unread request tail can reset the connection on
    // Windows before its peer reads our reply. Send FIN after the response, then
    // consume a bounded tail while the peer receives it and closes its side.
    // The caller still owns the original handshake deadline and cancellation.
    socket.shutdown().await?;
    let drain = async {
        let mut buffer = [0; 512];
        let mut remaining = REJECT_DRAIN_BYTES;
        while remaining > 0 {
            let length = remaining.min(buffer.len());
            let received = socket.read(&mut buffer[..length]).await?;
            if received == 0 {
                break;
            }
            remaining -= received;
        }
        Ok::<_, io::Error>(())
    };
    match tokio::time::timeout(REJECT_DRAIN_TIMEOUT, drain).await {
        Ok(result) => result,
        // A peer that never closes or sends more than the cap cannot retain a
        // listener slot indefinitely. No delivery guarantee applies past a cap.
        Err(_) => Ok(()),
    }
}

fn open_failure(error: &russh::Error) -> u8 {
    match error {
        russh::Error::ChannelOpenFailure(ChannelOpenFailure::AdministrativelyProhibited) => 2,
        // SSH's ConnectFailed does not distinguish refusal from DNS/routing
        // failures. Report general failure instead of inventing that precision.
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejection_drains_only_the_bounded_tail_after_reply_and_fin() -> io::Result<()> {
        let (mut client, mut server) = tokio::io::duplex(REJECT_DRAIN_BYTES * 2);
        client
            .write_all(&vec![0x42; REJECT_DRAIN_BYTES + 3])
            .await?;
        tokio::time::timeout(Duration::from_secs(2), reject(&mut server, &[5, 255])).await??;
        let mut response = [0; 2];
        tokio::time::timeout(Duration::from_secs(2), client.read_exact(&mut response)).await??;
        assert_eq!(response, [5, 255]);
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), client.read(&mut byte)).await??,
            0
        );
        let mut retained = [0; 3];
        tokio::time::timeout(Duration::from_secs(2), server.read_exact(&mut retained)).await??;
        assert_eq!(retained, [0x42; 3]);
        Ok(())
    }

    #[tokio::test]
    async fn rejection_releases_a_peer_that_keeps_its_write_half_open() -> io::Result<()> {
        let (mut client, mut server) = tokio::io::duplex(64);
        tokio::time::timeout(Duration::from_secs(2), reject(&mut server, &[5, 255])).await??;
        let mut response = [0; 2];
        tokio::time::timeout(Duration::from_secs(2), client.read_exact(&mut response)).await??;
        assert_eq!(response, [5, 255]);
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), client.read(&mut byte)).await??,
            0
        );
        Ok(())
    }
}
