//! Typed connection and channel completion, independent of terminal byte queues.
use crate::SessionError;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;

/// Observed local state of one authenticated SSH transport, not a health probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Authentication completed and transport shutdown has not been observed.
    Connected,
    /// The caller requested shutdown; remote completion is not yet confirmed.
    Closing,
    /// The protocol worker or owned transport has stopped.
    Closed(ConnectionEnd),
}
/// Credential-free reason why an SSH transport ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEnd {
    /// A local caller requested shutdown.
    LocalClosed,
    /// An established byte transport failed or reached unexpected EOF.
    TransportLost,
    /// SSH keepalive replies did not arrive within the configured budget.
    KeepaliveTimeout,
    /// The server explicitly disconnected. Its free-form message is discarded.
    RemoteDisconnected {
        /// SSH disconnect reason code, without peer-provided text.
        code: u32,
    },
    /// A protocol error occurred; blindly reconnecting is not justified.
    ProtocolFailure,
    /// No reliable cause was observed; this must not imply transient failure.
    Unknown,
}
/// One shell's final outcome. A remote channel close is not a TCP failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEnd {
    /// The remote process supplied an exit status, including unsuccessful exits.
    Exited {
        /// Exact remote status.
        code: u32,
    },
    /// The process ended due to a signal; untrusted descriptive text is omitted.
    Signalled {
        /// A bounded, printable protocol signal name.
        signal: String,
    },
    /// The server closed the channel without reporting process status.
    ChannelClosed,
    /// The parent transport ended without a prior process/channel completion.
    ConnectionClosed(ConnectionEnd),
    /// The local owner cancelled or closed this shell.
    Cancelled,
}
impl ShellEnd {
    /// Whether a new connection might recover from this established-shell loss.
    /// This never authorizes replaying input or bypassing authentication policy.
    pub fn is_reconnectable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionClosed(ConnectionEnd::TransportLost | ConnectionEnd::KeepaliveTimeout)
        )
    }
}

struct Inner {
    state: watch::Sender<ConnectionState>,
    shells: Mutex<HashMap<russh::ChannelId, Weak<ShellMonitor>>>,
}
#[derive(Clone)]
pub(super) struct ConnectionMonitor(Arc<Inner>);
impl ConnectionMonitor {
    pub(super) fn new() -> Self {
        let (state, _) = watch::channel(ConnectionState::Connected);
        Self(Arc::new(Inner {
            state,
            shells: Mutex::new(HashMap::new()),
        }))
    }
    pub(super) fn state(&self) -> ConnectionState {
        *self.0.state.borrow()
    }
    pub(super) fn subscribe(&self) -> watch::Receiver<ConnectionState> {
        self.0.state.subscribe()
    }
    pub(super) fn closing(&self) {
        self.0.state.send_if_modified(|state| {
            if *state == ConnectionState::Connected {
                *state = ConnectionState::Closing;
                true
            } else {
                false
            }
        });
    }
    pub(super) fn closed(&self, reason: ConnectionEnd) {
        self.0.state.send_if_modified(|state| match *state {
            ConnectionState::Closed(_) => false,
            ConnectionState::Closing => {
                *state = ConnectionState::Closed(ConnectionEnd::LocalClosed);
                true
            }
            ConnectionState::Connected => {
                *state = ConnectionState::Closed(reason);
                true
            }
        });
        if let ConnectionState::Closed(reason) = self.state()
            && let Ok(mut shells) = self.0.shells.lock()
        {
            for (_, shell) in shells.drain() {
                if let Some(shell) = shell.upgrade() {
                    shell.connection_closed(reason);
                }
            }
        }
    }
    pub(super) fn shell(&self, id: russh::ChannelId) -> Arc<ShellMonitor> {
        let (completion, _) = watch::channel(None);
        let shell = Arc::new(ShellMonitor {
            completion,
            evidence: Mutex::new(None),
        });
        if let Ok(mut shells) = self.0.shells.lock() {
            shells.retain(|_, shell| shell.strong_count() != 0);
            shells.insert(id, Arc::downgrade(&shell));
        }
        shell
    }
    pub(super) fn observe(&self, id: russh::ChannelId, evidence: ShellEnd) {
        if let Ok(shells) = self.0.shells.lock()
            && let Some(shell) = shells.get(&id).and_then(Weak::upgrade)
        {
            shell.observe(evidence);
        }
    }
    pub(super) fn channel_closed(&self, id: russh::ChannelId) {
        if let Ok(mut shells) = self.0.shells.lock()
            && let Some(shell) = shells.remove(&id).and_then(|shell| shell.upgrade())
        {
            shell.channel_closed();
        }
    }
}

pub(super) struct ShellMonitor {
    completion: watch::Sender<Option<ShellEnd>>,
    evidence: Mutex<Option<ShellEnd>>,
}
impl ShellMonitor {
    pub(super) fn subscribe(&self) -> watch::Receiver<Option<ShellEnd>> {
        self.completion.subscribe()
    }
    pub(super) fn completion(&self) -> Option<ShellEnd> {
        self.completion.borrow().clone()
    }
    pub(super) fn observe(&self, evidence: ShellEnd) {
        if let Ok(mut value) = self.evidence.lock()
            && value.is_none()
        {
            *value = Some(evidence);
        }
    }
    fn finish(&self, fallback: ShellEnd) {
        let evidence = self.evidence.lock().ok().and_then(|value| value.clone());
        self.completion.send_if_modified(|value| {
            if value.is_none() {
                *value = Some(evidence.unwrap_or(fallback));
                true
            } else {
                false
            }
        });
    }
    pub(super) fn channel_closed(&self) {
        self.finish(ShellEnd::ChannelClosed);
    }
    pub(super) fn connection_closed(&self, reason: ConnectionEnd) {
        self.finish(ShellEnd::ConnectionClosed(reason));
    }
    pub(super) fn cancel(&self) {
        self.completion.send_if_modified(|value| {
            if value.is_none() {
                *value = Some(ShellEnd::Cancelled);
                true
            } else {
                false
            }
        });
    }
}

pub(super) fn signal_name(signal: &russh::Sig) -> String {
    let standard;
    let raw = match signal {
        russh::Sig::Custom(name) => name.as_str(),
        _ => {
            standard = format!("{signal:?}");
            standard.as_str()
        }
    };
    raw.chars()
        .filter(|character| character.is_ascii_alphanumeric() || "@._-".contains(*character))
        .take(64)
        .collect()
}
pub(super) fn classify_error(error: &SessionError) -> ConnectionEnd {
    match error {
        SessionError::Ssh(russh::Error::KeepaliveTimeout) => ConnectionEnd::KeepaliveTimeout,
        SessionError::Io(error) | SessionError::Ssh(russh::Error::IO(error)) => {
            match error.kind() {
                std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::HostUnreachable
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::NetworkDown => ConnectionEnd::TransportLost,
                _ => ConnectionEnd::Unknown,
            }
        }
        SessionError::Ssh(
            russh::Error::Disconnect | russh::Error::HUP | russh::Error::ConnectionTimeout,
        ) => ConnectionEnd::TransportLost,
        SessionError::Ssh(_) => ConnectionEnd::ProtocolFailure,
        _ => ConnectionEnd::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_close_and_first_channel_evidence_are_sticky() {
        let connection = ConnectionMonitor::new();
        connection.closing();
        connection.closed(ConnectionEnd::TransportLost);
        connection.closed(ConnectionEnd::KeepaliveTimeout);
        assert_eq!(
            connection.state(),
            ConnectionState::Closed(ConnectionEnd::LocalClosed)
        );
        let (completion, _) = watch::channel(None);
        let shell = ShellMonitor {
            completion,
            evidence: Mutex::new(None),
        };
        shell.observe(ShellEnd::Exited { code: 19 });
        shell.connection_closed(ConnectionEnd::TransportLost);
        shell.cancel();
        assert_eq!(shell.completion(), Some(ShellEnd::Exited { code: 19 }));
    }

    #[test]
    fn custom_signal_is_bounded_and_has_no_terminal_control_bytes() {
        let signal = russh::Sig::Custom(format!("TERM\x1b[31m\n{}", "x".repeat(1000)));
        let name = signal_name(&signal);
        assert!(name.len() <= 64);
        assert!(
            name.chars()
                .all(|character| character.is_ascii_alphanumeric() || "@._-".contains(character))
        );
    }

    #[tokio::test]
    async fn keepalive_timeout_is_observed_from_real_silent_tcp_peer()
    -> Result<(), Box<dyn std::error::Error>> {
        use russh::{
            keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
            server,
        };
        use std::time::Duration;
        use tokio::net::{TcpListener, TcpStream};
        struct Peer {
            channels: Vec<russh::Channel<server::Msg>>,
        }
        impl server::Handler for Peer {
            type Error = russh::Error;
            async fn auth_password(
                &mut self,
                _: &str,
                _: &str,
            ) -> Result<server::Auth, Self::Error> {
                Ok(server::Auth::Accept)
            }
            async fn channel_open_session(
                &mut self,
                channel: russh::Channel<server::Msg>,
                reply: server::ChannelOpenHandle,
                _: &mut server::Session,
            ) -> Result<(), Self::Error> {
                self.channels.push(channel);
                reply.accept().await;
                Ok(())
            }
            async fn data(
                &mut self,
                _: russh::ChannelId,
                _: &[u8],
                _: &mut server::Session,
            ) -> Result<(), Self::Error> {
                // Keep the real socket open without replying to SSH requests.
                // The bounded handler delay also guarantees fixture teardown.
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(())
            }
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
            let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let socket = TcpStream::connect(listener.local_addr()?).await?;
            let (peer, _) = listener.accept().await?;
            let server = tokio::spawn(async move {
                let running = server::run_stream(
                    Arc::new(server::Config {
                        keys: vec![key],
                        ..Default::default()
                    }),
                    peer,
                    Peer {
                        channels: Vec::new(),
                    },
                )
                .await?;
                running.await
            });
            let monitor = ConnectionMonitor::new();
            let mut state = monitor.subscribe();
            let mut handle = russh::client::connect_stream(
                Arc::new(russh::client::Config {
                    keepalive_interval: Some(Duration::from_millis(20)),
                    keepalive_max: 1,
                    ..Default::default()
                }),
                socket,
                super::super::Client {
                    lifecycle: monitor.clone(),
                    expected: Some(fingerprint),
                    routes: crate::forwarding::ForwardRoutes::default(),
                },
            )
            .await?;
            assert!(
                handle
                    .authenticate_password("fixture", "fixture")
                    .await?
                    .success()
            );
            let channel = handle.channel_open_session().await?;
            let shell = monitor.shell(channel.id());
            channel.data(&b"stall"[..]).await?;
            loop {
                if matches!(*state.borrow_and_update(), ConnectionState::Closed(_)) {
                    break;
                }
                state.changed().await?;
            }
            assert_eq!(
                monitor.state(),
                ConnectionState::Closed(ConnectionEnd::KeepaliveTimeout)
            );
            assert_eq!(
                shell.completion(),
                Some(ShellEnd::ConnectionClosed(ConnectionEnd::KeepaliveTimeout))
            );
            let _ = server.await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await?
    }
}
