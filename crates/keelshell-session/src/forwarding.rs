//! Native SSH TCP forwarding with owned listener and stream lifetimes.

mod socks;

pub use socks::{DynamicForward, DynamicForwardOptions};

use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use russh::{Channel, ChannelOpenFailure, client};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::ssh::{SshSession, deadline};
use crate::{Result, SessionError};

/// Thread-safe cumulative payload counters for one forwarding listener.
///
/// `sent` is traffic flowing from the listener's local side towards the other
/// side of the tunnel. `received` is traffic flowing back. The counters are
/// runtime-only and are never persisted. A snapshot reads each counter
/// independently, so the two values can represent adjacent moments while a
/// stream is active.
#[derive(Clone, Default)]
pub struct TrafficStats {
    sent: Arc<AtomicU64>,
    received: Arc<AtomicU64>,
}

impl TrafficStats {
    /// Return the current cumulative payload totals.
    pub fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            sent: self.sent.load(Ordering::Relaxed),
            received: self.received.load(Ordering::Relaxed),
        }
    }

    fn add_sent(&self, bytes: u64) {
        saturating_add(&self.sent, bytes);
    }

    fn add_received(&self, bytes: u64) {
        saturating_add(&self.received, bytes);
    }
}

/// An instantaneous copy of a forwarding listener's cumulative payload totals.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrafficSnapshot {
    /// Payload bytes sent from the listener's local side to the other side.
    pub sent: u64,
    /// Payload bytes received from the other side by the listener's local side.
    pub received: u64,
}

fn saturating_add(counter: &AtomicU64, bytes: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(bytes))
    });
}

#[derive(Clone, Default)]
struct ActiveRelays {
    count: Arc<AtomicUsize>,
    zero: Arc<Notify>,
}

impl ActiveRelays {
    fn enter(&self) -> ActiveRelayGuard {
        self.count.fetch_add(1, Ordering::AcqRel);
        ActiveRelayGuard {
            relays: self.clone(),
        }
    }

    async fn wait_zero(&self) {
        loop {
            if self.count.load(Ordering::Acquire) == 0 {
                return;
            }
            let notified = self.zero.notified();
            if self.count.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

struct ActiveRelayGuard {
    relays: ActiveRelays,
}

impl Drop for ActiveRelayGuard {
    fn drop(&mut self) {
        if self.relays.count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.relays.zero.notify_waiters();
        }
    }
}

async fn copy_counted<R, W>(
    mut reader: R,
    mut writer: W,
    stats: TrafficStats,
    sent: bool,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            writer.shutdown().await?;
            return Ok(());
        }
        let mut offset = 0;
        while offset < read {
            let written = writer.write(&buffer[offset..read]).await?;
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "forwarding writer accepted zero bytes",
                ));
            }
            if sent {
                stats.add_sent(written as u64);
            } else {
                stats.add_received(written as u64);
            }
            offset += written;
        }
    }
}

async fn copy_bidirectional_counted<A, B>(
    first: &mut A,
    second: &mut B,
    stats: TrafficStats,
) -> io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (first_read, first_write) = tokio::io::split(&mut *first);
    let (second_read, second_write) = tokio::io::split(&mut *second);
    tokio::try_join!(
        copy_counted(first_read, second_write, stats.clone(), true),
        copy_counted(second_read, first_write, stats, false),
    )?;
    Ok(())
}

#[derive(Clone)]
struct Route {
    target: SocketAddr,
    cancel: watch::Sender<bool>,
    traffic: TrafficStats,
    permits: Arc<Semaphore>,
    active: ActiveRelays,
}

#[derive(Clone, Default)]
pub(crate) struct ForwardRoutes(Arc<Mutex<BTreeMap<(String, u32), Route>>>);

impl ForwardRoutes {
    pub(crate) fn cancel_all(&self) {
        if let Ok(mut routes) = self.0.lock() {
            for route in routes.values() {
                route.cancel.send_replace(true);
            }
            routes.clear();
        }
    }
}

// Before the acknowledgement, even the server-allocated port may be unknown.
// Disconnecting the owning SSH connection is the only reliable cancellation.
struct PendingRemoteForward {
    session: SshSession,
    armed: bool,
    runtime: tokio::runtime::Handle,
}
impl Drop for PendingRemoteForward {
    fn drop(&mut self) {
        if self.armed {
            self.session.routes.cancel_all();
            let session = self.session.clone();
            self.runtime.spawn(async move {
                let _ = session.close().await;
            });
        }
    }
}

/// A local listener tunneled to one remote host. Dropping it aborts the listener
/// and every connection still owned by that listener.
pub struct LocalForward {
    address: SocketAddr,
    traffic: TrafficStats,
    task: JoinHandle<()>,
}

impl LocalForward {
    /// Actual listener address, including an allocated port when binding port 0.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }
    /// Cumulative payload traffic observed by this listener.
    pub fn traffic(&self) -> TrafficStats {
        self.traffic.clone()
    }
    /// Close the listener and all its active streams.
    pub async fn close(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
impl Drop for LocalForward {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A remote listener tunneled to one local target. Explicit `close` waits for
/// cancellation acknowledgement; Drop removes authorization immediately and
/// schedules best-effort cancellation if a runtime is available.
pub struct RemoteForward {
    session: SshSession,
    address: String,
    port: u32,
    traffic: TrafficStats,
    closed: bool,
}

impl RemoteForward {
    /// Port selected by the remote server.
    pub fn remote_port(&self) -> u32 {
        self.port
    }
    /// Cumulative payload traffic observed by this listener.
    pub fn traffic(&self) -> TrafficStats {
        self.traffic.clone()
    }
    /// Cancel this remote listener and its active streams.
    pub async fn close(mut self) -> Result<()> {
        let active = self.remove_route();
        let result = deadline(self.session.timeout, "close remote forward", async {
            self.session
                .handle
                .cancel_tcpip_forward(&self.address, self.port)
                .await?;
            if let Some(active) = active {
                active.wait_zero().await;
            }
            Ok(())
        })
        .await;
        self.closed = true;
        result
    }
    fn remove_route(&self) -> Option<ActiveRelays> {
        if let Ok(mut routes) = self.session.routes.0.lock()
            && let Some(route) = routes.remove(&(self.address.clone(), self.port))
        {
            route.cancel.send_replace(true);
            return Some(route.active);
        }
        None
    }
}
impl Drop for RemoteForward {
    fn drop(&mut self) {
        self.remove_route();
        if !self.closed
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            let session = self.session.clone();
            let address = self.address.clone();
            let port = self.port;
            runtime.spawn(async move {
                let _ = tokio::time::timeout(
                    session.timeout,
                    session.handle.cancel_tcpip_forward(address, port),
                )
                .await;
            });
        }
    }
}

impl SshSession {
    /// Listen locally and forward accepted streams through direct-tcpip. The
    /// binding address is explicit so callers can choose loopback by default.
    /// At most 64 streams are active; additional connections are rejected.
    pub async fn forward_local(
        &self,
        bind: SocketAddr,
        remote_host: String,
        remote_port: u16,
    ) -> Result<LocalForward> {
        if remote_host.is_empty() || remote_port == 0 {
            return Err(SessionError::Invalid("forward destination must be set"));
        }
        let listener = TcpListener::bind(bind).await?;
        let address = listener.local_addr()?;
        let traffic = TrafficStats::default();
        let worker_traffic = traffic.clone();
        let session = self.clone();
        let task = tokio::spawn(async move {
            let mut streams = JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((mut socket, peer)) = incoming else { break; };
                        if streams.len() >= 64 { continue; }
                        let session = session.clone(); let destination = remote_host.clone();
                        let traffic = worker_traffic.clone();
                        streams.spawn(async move {
                            let channel = tokio::time::timeout(session.timeout, session.handle.channel_open_direct_tcpip(destination, u32::from(remote_port), peer.ip().to_string(), u32::from(peer.port()))).await;
                            if let Ok(Ok(channel)) = channel { let mut stream = channel.into_stream(); let _ = copy_bidirectional_counted(&mut socket, &mut stream, traffic).await; }
                        });
                    },
                    _ = streams.join_next(), if !streams.is_empty() => {},
                }
            }
        });
        Ok(LocalForward {
            address,
            traffic,
            task,
        })
    }

    /// Ask the SSH server to listen and forward to an explicit local target.
    /// Use a loopback bind address unless external reachability is intended.
    /// Port 0 asks the server to allocate an available port.
    pub async fn forward_remote(
        &self,
        bind_address: String,
        bind_port: u16,
        local_target: SocketAddr,
    ) -> Result<RemoteForward> {
        if bind_address.is_empty() || local_target.port() == 0 {
            return Err(SessionError::Invalid(
                "forward address and local target must be set",
            ));
        }
        let mut pending = PendingRemoteForward {
            session: self.clone(),
            armed: true,
            runtime: tokio::runtime::Handle::current(),
        };
        let port = deadline(self.timeout, "remote forward", async {
            Ok(self
                .handle
                .tcpip_forward(&bind_address, u32::from(bind_port))
                .await?)
        })
        .await?;
        let port = if bind_port == 0 {
            port
        } else {
            u32::from(bind_port)
        };
        let (cancel, _) = watch::channel(false);
        let traffic = TrafficStats::default();
        self.routes
            .0
            .lock()
            .map_err(|_| SessionError::Worker)?
            .insert(
                (bind_address.clone(), port),
                Route {
                    target: local_target,
                    cancel,
                    traffic: traffic.clone(),
                    permits: Arc::new(Semaphore::new(64)),
                    active: ActiveRelays::default(),
                },
            );
        pending.armed = false;
        Ok(RemoteForward {
            session: self.clone(),
            address: bind_address,
            port,
            traffic,
            closed: false,
        })
    }
}

pub(crate) async fn handle_forward(
    routes: &ForwardRoutes,
    channel: Channel<client::Msg>,
    address: &str,
    port: u32,
    reply: client::ChannelOpenHandle,
) -> Result<()> {
    let admission = {
        let routes = routes.0.lock().map_err(|_| SessionError::Worker)?;
        match routes.get(&(address.to_owned(), port)) {
            None => None,
            Some(route) => match route.permits.clone().try_acquire_owned() {
                Ok(permit) => Some(Ok((route.clone(), permit, route.active.enter()))),
                Err(_) => Some(Err(())),
            },
        }
    };
    let Some(admission) = admission else {
        reply
            .reject(ChannelOpenFailure::AdministrativelyProhibited)
            .await;
        return Ok(());
    };
    let Ok((route, permit, active_guard)) = admission else {
        reply.reject(ChannelOpenFailure::ResourceShortage).await;
        return Ok(());
    };
    let mut cancel = route.cancel.subscribe();
    if *cancel.borrow_and_update() {
        reply
            .reject(ChannelOpenFailure::AdministrativelyProhibited)
            .await;
        return Ok(());
    }
    tokio::spawn(async move {
        let _active_guard = active_guard;
        let _permit = permit;
        let mut relay_cancel = cancel.clone();
        let mut reject_cancel = cancel.clone();
        tokio::select! {
            _ = cancel.changed() => {},
            _ = async {
                match TcpStream::connect(route.target).await {
                    Ok(mut socket) => {
                        reply.accept().await;
                        let mut stream = channel.into_stream();
                        tokio::select! {
                            _ = relay_cancel.changed() => {},
                            _ = copy_bidirectional_counted(&mut stream, &mut socket, route.traffic) => {},
                        }
                    },
                    Err(_) => {
                        tokio::select! {
                            _ = reject_cancel.changed() => {},
                            _ = reply.reject(ChannelOpenFailure::ConnectFailed) => {},
                        }
                    },
                }
            } => {},
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn traffic_stats_accumulate_safely_across_threads() {
        let stats = TrafficStats::default();
        let workers = (0..8)
            .map(|_| {
                let stats = stats.clone();
                thread::spawn(move || {
                    for _ in 0..10_000 {
                        stats.add_sent(3);
                        stats.add_received(5);
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            assert!(worker.join().is_ok());
        }
        assert_eq!(
            stats.snapshot(),
            TrafficSnapshot {
                sent: 240_000,
                received: 400_000,
            }
        );
    }

    #[test]
    fn traffic_stats_saturate_instead_of_wrapping() {
        let stats = TrafficStats {
            sent: Arc::new(AtomicU64::new(u64::MAX - 1)),
            received: Arc::new(AtomicU64::new(0)),
        };
        stats.add_sent(4);
        assert_eq!(stats.snapshot().sent, u64::MAX);
    }
}
