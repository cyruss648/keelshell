//! Independent control of a session's TCP socket or owned upstream channel.

use super::channel::ChannelStreamOwner;
use std::future::Future;
use std::io;
use std::net::Shutdown;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// Control remains outside russh even during a cancelled handshake. TCP uses a
/// same-socket duplicate; tunneled sessions stop only their owned channel relay.
pub(super) struct TransportControl {
    resource: Resource,
    aborted: AtomicBool,
}

enum Resource {
    Socket(std::net::TcpStream),
    Channel(ChannelStreamOwner),
}

impl TransportControl {
    pub(super) fn attach(
        stream: tokio::net::TcpStream,
    ) -> io::Result<(tokio::net::TcpStream, Arc<Self>)> {
        let socket = stream.into_std()?;
        let control = Arc::new(Self {
            resource: Resource::Socket(socket.try_clone()?),
            aborted: AtomicBool::new(false),
        });
        Ok((tokio::net::TcpStream::from_std(socket)?, control))
    }

    /// The relay owns the upstream session, but must not own this control Arc:
    /// cancelled handshakes rely on its final external Drop stopping the relay.
    pub(super) fn channel(owner: ChannelStreamOwner) -> Arc<Self> {
        Arc::new(Self {
            resource: Resource::Channel(owner),
            aborted: AtomicBool::new(false),
        })
    }

    pub(super) fn is_closed(&self) -> bool {
        self.aborted.load(Ordering::Acquire)
            || matches!(&self.resource, Resource::Channel(owner) if owner.is_closed())
    }

    pub(super) fn abort(&self) -> io::Result<()> {
        match &self.resource {
            Resource::Socket(socket) => match socket.shutdown(Shutdown::Both) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotConnected => {}
                Err(error) => return Err(error),
            },
            Resource::Channel(owner) => owner.cancel(),
        }
        self.aborted.store(true, Ordering::Release);
        Ok(())
    }

    pub(super) async fn close_after(
        &self,
        grace: Duration,
        graceful: impl Future<Output = ()>,
    ) -> io::Result<()> {
        if tokio::time::timeout(grace, graceful).await.is_err() {
            self.abort()?;
        }
        Ok(())
    }
}

impl Drop for TransportControl {
    fn drop(&mut self) {
        // Also covers cancelled connect/auth futures before a session exists.
        let _ = self.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    #[tokio::test]
    async fn stalled_graceful_disconnect_forces_real_socket_shutdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = TcpStream::connect(listener.local_addr()?).await?;
        let (mut peer, _) = listener.accept().await?;
        let (_stream, control) = TransportControl::attach(client)?;
        control
            .close_after(Duration::from_millis(30), std::future::pending())
            .await?;
        assert!(control.is_closed());
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), peer.read(&mut byte)).await??,
            0
        );
        Ok(())
    }

    #[tokio::test]
    async fn completed_graceful_cleanup_does_not_abort_socket()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = TcpStream::connect(listener.local_addr()?).await?;
        let (mut peer, _) = listener.accept().await?;
        let (mut stream, control) = TransportControl::attach(client)?;
        control
            .close_after(Duration::from_millis(30), async {})
            .await?;
        assert!(!control.is_closed());
        stream.write_all(b"x").await?;
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(2), peer.read_exact(&mut byte)).await??;
        assert_eq!(&byte, b"x");
        drop(control);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), peer.read(&mut byte)).await??,
            0
        );
        Ok(())
    }
}
