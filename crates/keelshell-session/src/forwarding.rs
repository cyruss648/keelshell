//! Native SSH TCP forwarding with owned listener and stream lifetimes.

mod socks;

pub use socks::{DynamicForward, DynamicForwardOptions};

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use russh::{Channel, ChannelOpenFailure, client};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::ssh::{SshSession, deadline};
use crate::{Result, SessionError};

#[derive(Clone)]
struct Route {
    target: SocketAddr,
    cancel: watch::Sender<bool>,
    permits: Arc<Semaphore>,
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
    task: JoinHandle<()>,
}

impl LocalForward {
    /// Actual listener address, including an allocated port when binding port 0.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
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
    closed: bool,
}

impl RemoteForward {
    /// Port selected by the remote server.
    pub fn remote_port(&self) -> u32 {
        self.port
    }
    /// Cancel this remote listener and its active streams.
    pub async fn close(mut self) -> Result<()> {
        self.remove_route();
        let result = deadline(self.session.timeout, "cancel remote forward", async {
            Ok(self
                .session
                .handle
                .cancel_tcpip_forward(&self.address, self.port)
                .await?)
        })
        .await;
        self.closed = true;
        result
    }
    fn remove_route(&self) {
        if let Ok(mut routes) = self.session.routes.0.lock()
            && let Some(route) = routes.remove(&(self.address.clone(), self.port))
        {
            route.cancel.send_replace(true);
        }
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
        let session = self.clone();
        let task = tokio::spawn(async move {
            let mut streams = JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((mut socket, peer)) = incoming else { break; };
                        if streams.len() >= 64 { continue; }
                        let session = session.clone(); let destination = remote_host.clone();
                        streams.spawn(async move {
                            let channel = tokio::time::timeout(session.timeout, session.handle.channel_open_direct_tcpip(destination, u32::from(remote_port), peer.ip().to_string(), u32::from(peer.port()))).await;
                            if let Ok(Ok(channel)) = channel { let mut stream = channel.into_stream(); let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await; }
                        });
                    },
                    _ = streams.join_next(), if !streams.is_empty() => {},
                }
            }
        });
        Ok(LocalForward { address, task })
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
        self.routes
            .0
            .lock()
            .map_err(|_| SessionError::Worker)?
            .insert(
                (bind_address.clone(), port),
                Route {
                    target: local_target,
                    cancel,
                    permits: Arc::new(Semaphore::new(64)),
                },
            );
        pending.armed = false;
        Ok(RemoteForward {
            session: self.clone(),
            address: bind_address,
            port,
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
    let route = routes
        .0
        .lock()
        .map_err(|_| SessionError::Worker)?
        .get(&(address.to_owned(), port))
        .cloned();
    let Some(route) = route else {
        reply
            .reject(ChannelOpenFailure::AdministrativelyProhibited)
            .await;
        return Ok(());
    };
    let Ok(permit) = route.permits.clone().try_acquire_owned() else {
        reply.reject(ChannelOpenFailure::ResourceShortage).await;
        return Ok(());
    };
    let mut cancel = route.cancel.subscribe();
    if *cancel.borrow() {
        reply
            .reject(ChannelOpenFailure::AdministrativelyProhibited)
            .await;
        return Ok(());
    }
    tokio::spawn(async move {
        let _permit = permit;
        tokio::select! {
            _ = cancel.changed() => {},
            _ = async {
                match TcpStream::connect(route.target).await {
                    Ok(mut socket) => { reply.accept().await; let mut stream = channel.into_stream(); let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await; },
                    Err(_) => { reply.reject(ChannelOpenFailure::ConnectFailed).await; },
                }
            } => {},
        }
    });
    Ok(())
}
