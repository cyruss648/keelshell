//! SSH with explicit host identity approval and ephemeral credentials.

mod batch;
mod channel;
mod lifecycle;
pub use lifecycle::{ConnectionEnd, ConnectionState, ShellEnd};
use lifecycle::{ConnectionMonitor, ShellMonitor};
mod proxy;
pub use proxy::{ProxyCredentials, ProxyError, ProxyKind, SshProxy};
mod transport;
pub(crate) use channel::{ChannelStreamOwner, OwnedRawSftpSession};
use channel::{open_direct, open_session, open_session_until};

use transport::TransportControl;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use russh::client;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh::{ChannelMsg, ChannelWriteHalf, Disconnect};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout_at};
use zeroize::Zeroizing;

use crate::forwarding::{ForwardRoutes, handle_forward};
use crate::sftp::SftpSession;
use crate::{Result, SessionError, SessionEvent};

/// Authentication material, deliberately without `Debug` or serialization.
pub enum SshAuth {
    /// Use the user's SSH agent. Unix uses SSH_AUTH_SOCK; Windows uses OpenSSH's pipe.
    Agent,
    /// An ephemeral password, erased when this value is dropped.
    Password(Zeroizing<String>),
    /// Answer server keyboard-interactive prompts with ephemeral responses.
    /// Responses are consumed in prompt order and are never logged or serialized.
    KeyboardInteractive {
        /// One response per server prompt, in order.
        responses: Vec<Zeroizing<String>>,
    },
    /// Ask the application to collect each server challenge at connection time.
    ///
    /// The sender and every answer are ephemeral. A dropped response is treated
    /// as an explicit cancellation and never falls back to an empty answer.
    KeyboardInteractivePrompted {
        /// Bounded channel used to deliver one server challenge at a time.
        challenges: mpsc::Sender<KeyboardInteractiveChallenge>,
    },
    /// Authenticate with a private key and then collect server-driven MFA
    /// prompts. The key passphrase and challenge answers remain ephemeral.
    PrivateKeyKeyboardInteractive {
        /// Private key path loaded only after host identity verification.
        path: PathBuf,
        /// Optional in-memory key decryption passphrase.
        passphrase: Option<Zeroizing<String>>,
        /// Bounded channel used to deliver one server challenge at a time.
        challenges: mpsc::Sender<KeyboardInteractiveChallenge>,
    },
    /// A private key file and optional ephemeral passphrase.
    PrivateKey {
        /// File loaded only after the host identity is verified.
        path: PathBuf,
        /// Decryption passphrase, never persisted by this crate.
        passphrase: Option<Zeroizing<String>>,
    },
}

/// One server keyboard-interactive challenge awaiting an explicit UI answer.
///
/// The response channel is intentionally one-shot: a challenge cannot be
/// replayed after cancellation or after the owning route changes.
pub struct KeyboardInteractiveChallenge {
    /// Optional server-provided challenge name.
    pub name: String,
    /// Optional server-provided instructions.
    pub instructions: String,
    /// Ordered prompts for this challenge batch.
    pub prompts: Vec<KeyboardInteractivePrompt>,
    /// Send `Some` with one ephemeral answer per prompt, or `None` to cancel.
    pub response: oneshot::Sender<Option<Vec<Zeroizing<String>>>>,
}

/// One bounded keyboard-interactive prompt.
pub struct KeyboardInteractivePrompt {
    /// Server-provided text. It is displayed as untrusted text only.
    pub prompt: String,
    /// Whether the server allows the answer to be displayed while typing.
    pub echo: bool,
}

/// Connection options. The caller owns the association between host/port and its
/// stored SHA256 fingerprint; this transport never trusts a first key silently.
pub struct SshOptions {
    /// DNS name or IP address.
    pub host: String,
    /// TCP port, usually 22.
    pub port: u16,
    /// Remote login name.
    pub username: String,
    /// Previously explicitly approved `SHA256:...` fingerprint.
    pub expected_host_key: Option<String>,
    /// Ephemeral authentication configuration.
    pub auth: SshAuth,
    /// Explicit upstream proxy for this hop. No system proxy is read.
    pub proxy: Option<SshProxy>,
    /// Deadline for connection/authentication and individual protocol requests.
    /// Streaming transfers use it as an active idle wait renewed by confirmed
    /// I/O; read-only full-content/tree validation keeps its separate fixed limit.
    pub timeout: Duration,
}

/// Bounded retry and backoff settings for a complete SSH connection attempt.
///
/// `max_attempts` includes the first attempt and is clamped to at least one.
/// Delays use capped exponential backoff without jitter so callers can make
/// deterministic UI and integration-test promises. Dropping the returned
/// future stops the retry loop and any pending backoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
}

impl RetryPolicy {
    /// Create a policy; zero attempts become one and the delay cap is never
    /// lower than the initial delay.
    pub fn new(max_attempts: u32, initial_delay: Duration, max_delay: Duration) -> Self {
        let max_delay = if max_delay < initial_delay {
            initial_delay
        } else {
            max_delay
        };
        Self {
            max_attempts: max_attempts.max(1),
            initial_delay,
            max_delay,
        }
    }

    /// Maximum number of complete connection attempts, including the first.
    pub const fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    /// Return the delay before the one-based retry number.
    pub fn delay_before_retry(self, retry_number: u32) -> Duration {
        let mut delay = self.initial_delay;
        if delay.is_zero() {
            return Duration::ZERO;
        }
        for _ in 1..retry_number.max(1) {
            if delay == self.max_delay {
                return self.max_delay;
            }
            let Some(next) = delay.checked_mul(2) else {
                return self.max_delay;
            };
            delay = next.min(self.max_delay);
        }
        delay.min(self.max_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(3, Duration::from_millis(250), Duration::from_secs(4))
    }
}

impl SshOptions {
    /// Construct options with SSH agent authentication and a 15-second deadline.
    /// The first connection returns `UnknownHostKey` until a pin is supplied.
    pub fn new(host: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: 22,
            username: username.into(),
            expected_host_key: None,
            auth: SshAuth::Agent,
            proxy: None,
            timeout: Duration::from_secs(15),
        }
    }
}

pub(crate) struct Client {
    lifecycle: ConnectionMonitor,
    expected: Option<String>,
    pub(crate) routes: ForwardRoutes,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.lifecycle.closed(ConnectionEnd::Unknown);
    }
}
impl client::Handler for Client {
    async fn disconnected(&mut self, reason: client::DisconnectReason<Self::Error>) -> Result<()> {
        match reason {
            client::DisconnectReason::ReceivedDisconnect(info) => {
                self.lifecycle.closed(ConnectionEnd::RemoteDisconnected {
                    code: info.reason_code as u32,
                });
                Ok(())
            }
            client::DisconnectReason::Error(error) => {
                self.lifecycle.closed(lifecycle::classify_error(&error));
                Err(error)
            }
        }
    }
    async fn exit_status(
        &mut self,
        id: russh::ChannelId,
        code: u32,
        _: &mut client::Session,
    ) -> Result<()> {
        self.lifecycle.observe(id, ShellEnd::Exited { code });
        Ok(())
    }
    async fn exit_signal(
        &mut self,
        id: russh::ChannelId,
        signal: russh::Sig,
        _: bool,
        _: &str,
        _: &str,
        _: &mut client::Session,
    ) -> Result<()> {
        self.lifecycle.observe(
            id,
            ShellEnd::Signalled {
                signal: lifecycle::signal_name(&signal),
            },
        );
        Ok(())
    }
    async fn channel_close(&mut self, id: russh::ChannelId, _: &mut client::Session) -> Result<()> {
        self.lifecycle.channel_closed(id);
        Ok(())
    }
    type Error = SessionError;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool> {
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        match &self.expected {
            None => Err(SessionError::UnknownHostKey { fingerprint }),
            Some(expected) if *expected != fingerprint => Err(SessionError::ChangedHostKey {
                expected: expected.clone(),
                actual: fingerprint,
            }),
            Some(_) => Ok(true),
        }
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        connected_address: &str,
        connected_port: u32,
        _: &str,
        _: u32,
        reply: client::ChannelOpenHandle,
        _: &mut client::Session,
    ) -> Result<()> {
        handle_forward(
            &self.routes,
            channel,
            connected_address,
            connected_port,
            reply,
        )
        .await
    }
}

/// Authenticated SSH connection. Clones share one encrypted connection.
/// Channel opens retain an independent owner through cancellation and
/// late confirmation. Normal cleanup preserves this connection; an open that
/// remains unconfirmed through its deadline can force shared transport shutdown.
#[derive(Clone)]
pub struct SshSession {
    pub(crate) handle: Arc<client::Handle<Client>>,
    pub(crate) routes: ForwardRoutes,
    pub(crate) timeout: Duration,
    _lifecycle: Arc<ConnectionLifecycle>,
    transport: Arc<TransportControl>,
    state: ConnectionMonitor,
    pub(crate) transfer_reservations: Arc<crate::sftp::TransferReservations>,
}

struct ConnectionLifecycle {
    state: ConnectionMonitor,
    transport: Arc<TransportControl>,
    handle: Arc<client::Handle<Client>>,
    routes: ForwardRoutes,
    timeout: Duration,
    runtime: tokio::runtime::Handle,
}

impl Drop for ConnectionLifecycle {
    fn drop(&mut self) {
        self.state.closing();
        self.routes.cancel_all();
        let handle = self.handle.clone();
        let transport = self.transport.clone();
        let timeout = self.timeout;
        self.runtime.spawn(async move {
            let _ = tokio::time::timeout(
                timeout,
                handle.disconnect(Disconnect::ByApplication, "Session released", "en"),
            )
            .await;
            drop(transport);
        });
    }
}

/// Bounded command output. Exit status remains `None` if the server closed
/// without reporting one; callers must not assume success in that case.
#[derive(Debug)]
pub struct ExecOutput {
    /// Standard output bytes.
    pub stdout: Vec<u8>,
    /// Standard error bytes.
    pub stderr: Vec<u8>,
    /// Remote process exit code, if supplied.
    pub exit_status: Option<u32>,
}

impl SshSession {
    /// Connect, verify the host key, then authenticate. All three stages share a
    /// deadline. Authentication is never attempted for an unknown/changed key.
    pub async fn connect(options: SshOptions) -> Result<Self> {
        Self::connect_once(&options).await
    }

    /// Connect with bounded retries for transient transport failures.
    ///
    /// Unknown or changed host keys, authentication and credential errors,
    /// invalid options and explicit server-policy rejections are returned
    /// immediately. Only [`SessionError::is_retryable`] failures consume a
    /// retry attempt. Dropping the future stops the retry loop, pending backoff
    /// and awaiting the active attempt. A private-key read that already entered
    /// a blocking worker may finish afterward; its result cannot start a retry.
    pub async fn connect_with_retry(options: SshOptions, policy: RetryPolicy) -> Result<Self> {
        retry_with_policy(policy, || Self::connect_once(&options)).await
    }

    /// Connect to a target through an already authenticated SSH jump session.
    ///
    /// The jump server opens a `direct-tcpip` channel to the configured proxy,
    /// or to `options.host` when no proxy is configured. A proxy performs target
    /// DNS resolution; proxy negotiation shares the same absolute deadline.
    /// Target host identity and authentication are checked independently using
    /// `options`; no local listener, local DNS lookup or direct TCP fallback is
    /// used. Channel opening, target handshake and authentication share one
    /// absolute deadline. The returned target retains the jump's lifetime.
    ///
    /// Dropping this future or the target requests owned channel cleanup. Normal
    /// rejection and cleanup preserve the jump. An unconfirmed channel open or
    /// blocked CLOSE can require disconnecting the jump shared by its clones.
    /// Applications needing isolation should create a dedicated jump chain for
    /// each route rather than borrowing an unrelated interactive connection.
    pub async fn connect_through(jump: &Self, options: SshOptions) -> Result<Self> {
        Self::connect_through_once(jump, &options).await
    }

    /// Retry transient target connection failures through the same jump.
    ///
    /// Each complete attempt has its own `options.timeout`; identity, credential
    /// and explicit channel rejection errors are not retried. Cancellation stops
    /// retries while an admitted channel remains independently owned for cleanup.
    /// A jump disconnected during exceptional cleanup is not reconnected here.
    pub async fn connect_through_with_retry(
        jump: &Self,
        options: SshOptions,
        policy: RetryPolicy,
    ) -> Result<Self> {
        retry_with_policy(policy, || Self::connect_through_once(jump, &options)).await
    }

    fn validate_options(options: &SshOptions) -> Result<()> {
        if options.host.trim().is_empty()
            || options.username.is_empty()
            || options.port == 0
            || options.timeout.is_zero()
        {
            return Err(SessionError::Invalid(
                "SSH host, user, port and timeout must be set",
            ));
        }
        if let Some(proxy) = &options.proxy {
            proxy.validate(&options.host)?;
        }
        Ok(())
    }

    async fn connect_through_once(jump: &Self, options: &SshOptions) -> Result<Self> {
        Self::validate_options(options)?;
        if jump.is_closed() {
            return Err(SessionError::Closed);
        }
        let until = Instant::now()
            .checked_add(options.timeout)
            .ok_or(SessionError::Invalid("SSH timeout exceeds the clock range"))?;
        timeout_at(until, async {
            let (host, port) = options
                .proxy
                .as_ref()
                .map_or((options.host.as_str(), options.port), |proxy| {
                    (proxy.host.as_str(), proxy.port)
                });
            let channel = open_direct(jump, host.to_owned(), port, until).await?;
            let (stream, owner) = channel.into_stream();
            let transport = TransportControl::channel(owner);
            let stream =
                proxy::negotiate(stream, options.proxy.as_ref(), &options.host, options.port)
                    .await?;
            Self::authenticate_stream(options, stream, transport).await
        })
        .await
        .map_err(|_| SessionError::Timeout("SSH jump connect/authenticate"))?
    }

    async fn connect_once(options: &SshOptions) -> Result<Self> {
        Self::validate_options(options)?;
        deadline(options.timeout, "SSH connect/authenticate", async move {
            let (host, port) = options
                .proxy
                .as_ref()
                .map_or((options.host.as_str(), options.port), |proxy| {
                    (proxy.host.as_str(), proxy.port)
                });
            let stream = tokio::net::TcpStream::connect((host, port)).await?;
            stream.set_nodelay(true)?;
            let (stream, transport) = TransportControl::attach(stream)?;
            let stream =
                proxy::negotiate(stream, options.proxy.as_ref(), &options.host, options.port)
                    .await?;
            Self::authenticate_stream(options, stream, transport).await
        })
        .await
    }

    async fn authenticate_stream<R>(
        options: &SshOptions,
        stream: R,
        transport: Arc<TransportControl>,
    ) -> Result<Self>
    where
        R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let timeout = options.timeout;
        let routes = ForwardRoutes::default();
        let config = client::Config {
            keepalive_interval: Some(Duration::from_secs(30)),
            keepalive_max: 3,
            ..Default::default()
        };
        let state = ConnectionMonitor::new();
        let mut handle = client::connect_stream(
            Arc::new(config),
            stream,
            Client {
                lifecycle: state.clone(),
                expected: options.expected_host_key.clone(),
                routes: routes.clone(),
            },
        )
        .await?;
        let accepted = match &options.auth {
            SshAuth::Password(password) => handle
                .authenticate_password(&options.username, password.as_str())
                .await?
                .success(),
            SshAuth::KeyboardInteractive { responses } => {
                authenticate_keyboard_interactive(&mut handle, &options.username, responses).await?
            }
            SshAuth::KeyboardInteractivePrompted { challenges } => {
                authenticate_keyboard_interactive_prompted(
                    &mut handle,
                    &options.username,
                    challenges,
                )
                .await?
            }
            SshAuth::PrivateKey { path, passphrase } => {
                let path = path.clone();
                let passphrase = passphrase.clone();
                let key = tokio::task::spawn_blocking(move || {
                    load_secret_key(path, passphrase.as_deref().map(String::as_str))
                })
                .await
                .map_err(|_| SessionError::Worker)?
                .map_err(|error| SessionError::Credential(error.to_string()))?;
                let hash = handle.best_supported_rsa_hash().await?.flatten();
                handle
                    .authenticate_publickey(
                        &options.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await?
                    .success()
            }
            SshAuth::PrivateKeyKeyboardInteractive {
                path,
                passphrase,
                challenges,
            } => {
                let path = path.clone();
                let passphrase = passphrase.clone();
                let key = tokio::task::spawn_blocking(move || {
                    load_secret_key(path, passphrase.as_deref().map(String::as_str))
                })
                .await
                .map_err(|_| SessionError::Worker)?
                .map_err(|error| SessionError::Credential(error.to_string()))?;
                let hash = handle.best_supported_rsa_hash().await?.flatten();
                let key_result = handle
                    .authenticate_publickey(
                        &options.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await?;
                match key_result {
                    client::AuthResult::Success => true,
                    client::AuthResult::Failure {
                        partial_success: true,
                        ..
                    } => {
                        authenticate_keyboard_interactive_prompted(
                            &mut handle,
                            &options.username,
                            challenges,
                        )
                        .await?
                    }
                    client::AuthResult::Failure { .. } => false,
                }
            }
            SshAuth::Agent => authenticate_agent(&mut handle, &options.username).await?,
        };
        if !accepted {
            return Err(SessionError::Authentication);
        }
        let handle = Arc::new(handle);
        let lifecycle = Arc::new(ConnectionLifecycle {
            state: state.clone(),
            transport: transport.clone(),
            handle: handle.clone(),
            routes: routes.clone(),
            timeout,
            runtime: tokio::runtime::Handle::current(),
        });
        Ok(Self {
            handle,
            routes,
            timeout,
            _lifecycle: lifecycle,
            transport,
            state,
            transfer_reservations: Arc::new(crate::sftp::TransferReservations::new(options)),
        })
    }

    /// Request an interactive remote PTY and shell, checking both acknowledgements.
    pub async fn start_shell(&self, rows: u16, cols: u16) -> Result<SshShell> {
        if rows == 0 || cols == 0 {
            return Err(SessionError::Invalid("terminal dimensions must be nonzero"));
        }
        deadline(self.timeout, "SSH shell", async {
            let mut pending = open_session(self).await?;
            let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
            let monitor = self.state.shell(channel.id());
            channel
                .request_pty(
                    true,
                    "xterm-256color",
                    u32::from(cols),
                    u32::from(rows),
                    0,
                    0,
                    &[],
                )
                .await?;
            acknowledge(channel, "PTY allocation").await?;
            channel.request_shell(true).await?;
            acknowledge(channel, "interactive shell").await?;
            let (mut reader, writer) = pending.channel.take().ok_or(SessionError::Closed)?.split();
            let (tx, output) = mpsc::channel(64);
            let observed = monitor.clone();
            let connection = self.state.clone();
            let task = tokio::spawn(async move {
                while let Some(message) = reader.wait().await {
                    let event = match message {
                        ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                            Some(SessionEvent::Data(data.to_vec()))
                        }
                        ChannelMsg::ExitStatus { exit_status } => {
                            observed.observe(ShellEnd::Exited { code: exit_status });
                            None
                        }
                        ChannelMsg::ExitSignal { signal_name, .. } => {
                            observed.observe(ShellEnd::Signalled {
                                signal: lifecycle::signal_name(&signal_name),
                            });
                            None
                        }
                        ChannelMsg::Close => {
                            observed.channel_closed();
                            break;
                        }
                        _ => None,
                    };
                    if let Some(event) = event
                        && tx.send(event).await.is_err()
                    {
                        observed.cancel();
                        return;
                    }
                }
                if observed.completion().is_none() {
                    // russh drops channel senders before its disconnect callback.
                    // Wait for that callback (or Client::drop fallback), rather
                    // than freezing an Unknown result ahead of the real cause.
                    let mut state = connection.subscribe();
                    let reason = loop {
                        match *state.borrow_and_update() {
                            ConnectionState::Closed(reason) => break reason,
                            ConnectionState::Closing => break ConnectionEnd::LocalClosed,
                            ConnectionState::Connected => {}
                        }
                        if state.changed().await.is_err() {
                            break ConnectionEnd::Unknown;
                        }
                    };
                    observed.connection_closed(reason);
                }
                let event = match observed.completion() {
                    Some(ShellEnd::Exited { code }) => SessionEvent::Exited {
                        code,
                        success: code == 0,
                    },
                    _ => SessionEvent::Error("SSH shell ended".into()),
                };
                let _ = tx.send(event).await;
            });
            Ok(SshShell {
                monitor,
                writer: Arc::new(writer),
                output,
                task,
                timeout: self.timeout,
                _session: self.clone(),
            })
        })
        .await
    }

    /// Execute an explicitly supplied remote shell command. The SSH protocol
    /// accepts a command string; callers must review it and quote their own data.
    /// Output is capped at 8 MiB and the connection deadline bounds execution.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput> {
        self.exec_limited(command, 8 * 1024 * 1024).await
    }

    /// Execute with an explicit combined stdout/stderr memory limit.
    pub async fn exec_limited(&self, command: &str, max_bytes: usize) -> Result<ExecOutput> {
        let mut pending = deadline(self.timeout, "SSH channel open", open_session(self)).await?;
        let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
        let result = deadline(self.timeout, "SSH exec", async {
            channel.exec(true, command).await?;
            let mut output = ExecOutput {
                stdout: Vec::new(),
                stderr: Vec::new(),
                exit_status: None,
            };
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, ext: 0 } => {
                        check_output(&output, data.len(), max_bytes)?;
                        output.stdout.extend_from_slice(&data);
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        check_output(&output, data.len(), max_bytes)?;
                        output.stderr.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        output.exit_status = Some(exit_status)
                    }
                    ChannelMsg::Failure => return Err(SessionError::Rejected("exec")),
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            Ok(output)
        })
        .await;
        pending.close().await;
        result
    }

    // Completion callers supply one absolute deadline covering every channel and
    // request. The open owner shares it, including a late open after cancellation.
    pub(crate) async fn completion_exec_until(&self, until: Instant) -> Result<ExecOutput> {
        let mut pending = open_session_until(self, until).await?;
        let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
        channel.exec(true, crate::completion::PATH_PROBE).await?;
        let mut output = ExecOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_status: None,
        };
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, ext: 0 } => {
                    check_output(&output, data.len(), 16 * 1024)?;
                    output.stdout.extend_from_slice(&data);
                }
                ChannelMsg::ExtendedData { data, .. } => {
                    check_output(&output, data.len(), 16 * 1024)?;
                    output.stderr.extend_from_slice(&data);
                }
                ChannelMsg::ExitStatus { exit_status } => output.exit_status = Some(exit_status),
                ChannelMsg::Failure => return Err(SessionError::Rejected("completion probe")),
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        pending.close().await;
        Ok(output)
    }

    pub(crate) async fn completion_sftp_until(
        &self,
        until: Instant,
    ) -> Result<OwnedRawSftpSession> {
        let mut pending = open_session_until(self, until).await?;
        let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
        channel.request_subsystem(true, "sftp").await?;
        acknowledge(channel, "SFTP subsystem").await?;
        let (stream, initialization) = pending.into_stream();
        let raw = russh_sftp::client::RawSftpSession::new(stream);
        raw.init()
            .await
            .map_err(|_| SessionError::Rejected("completion SFTP initialization"))?;
        Ok(OwnedRawSftpSession::new(raw, initialization))
    }

    pub(crate) async fn sftp_raw(&self) -> Result<OwnedRawSftpSession> {
        Ok(self.sftp_raw_with_version().await?.0)
    }

    pub(crate) async fn sftp_raw_with_version(
        &self,
    ) -> Result<(OwnedRawSftpSession, russh_sftp::protocol::Version)> {
        deadline(self.timeout, "SFTP initialization", async {
            let mut pending = open_session(self).await?;
            crate::sftp::observed_transfer_io();
            let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
            channel.request_subsystem(true, "sftp").await?;
            acknowledge(channel, "SFTP subsystem").await?;
            crate::sftp::observed_transfer_io();
            let (stream, initialization) = pending.into_stream();
            let raw = russh_sftp::client::RawSftpSession::new(stream);
            let version = raw
                .init()
                .await
                .map_err(|error| SessionError::Sftp(error.to_string()))?;
            crate::sftp::observed_transfer_io();
            Ok((OwnedRawSftpSession::new(raw, initialization), version))
        })
        .await
    }

    /// Open an independent SFTP subsystem on this connection.
    pub async fn sftp(&self) -> Result<SftpSession> {
        deadline(self.timeout, "SFTP initialization", async {
            let mut pending = open_session(self).await?;
            let channel = pending.channel.as_mut().ok_or(SessionError::Closed)?;
            channel.request_subsystem(true, "sftp").await?;
            acknowledge(channel, "SFTP subsystem").await?;
            let (stream, initialization) = pending.into_stream();
            SftpSession::from_stream(stream, self.timeout, self.clone(), initialization).await
        })
        .await
    }

    /// Disconnect this connection and all channels shared by its clones.
    pub async fn close(&self) -> Result<()> {
        self.state.closing();
        self.routes.cancel_all();
        deadline(self.timeout, "SSH disconnect", async {
            Ok(self
                .handle
                .disconnect(Disconnect::ByApplication, "Session closed", "en")
                .await?)
        })
        .await
    }

    /// Snapshot of observed connection lifecycle, without network I/O.
    pub fn connection_state(&self) -> ConnectionState {
        self.state.state()
    }

    /// Observe closure independently of shell byte queues. This does not probe
    /// remote health, and protocol backpressure can delay detecting a failure.
    pub fn subscribe_state(&self) -> tokio::sync::watch::Receiver<ConnectionState> {
        self.state.subscribe()
    }

    /// Whether the protocol task has stopped or its owned transport was stopped.
    /// This is local transport state, not an active remote-health probe.
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed() || self.transport.is_closed()
    }

    /// Whether both handles share the exact authenticated encrypted connection.
    ///
    /// This compares connection ownership, not an endpoint or username. A newly
    /// authenticated connection to the same server is deliberately different.
    /// Callers can use this when revalidating an ephemeral human review.
    pub fn same_connection(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.handle, &other.handle)
    }

    /// Abnormal channel cleanup cannot abandon an unconfirmed remote resource.
    /// If protocol disconnect stalls, stop this session's socket or channel relay.
    pub(crate) async fn close_or_abort(&self) -> Result<()> {
        self.routes.cancel_all();
        self.transport
            .close_after(Duration::from_secs(2), async {
                let _ = self.close().await;
                while !self.is_closed() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await?;
        // A TCP control duplicates the socket; a channel control independently
        // owns cancellation. Release either even when russh has already stopped.
        self.transport.abort()?;
        Ok(())
    }
}

/// An interactive SSH shell with a bounded event receiver.
pub struct SshShell {
    monitor: Arc<ShellMonitor>,
    writer: Arc<ChannelWriteHalf<client::Msg>>,
    output: mpsc::Receiver<SessionEvent>,
    task: JoinHandle<()>,
    timeout: Duration,
    _session: SshSession,
}

impl SshShell {
    /// Observe the shell completion reason independently of unread terminal output.
    /// A published outcome does not mean all previously queued bytes were read.
    pub fn subscribe_completion(&self) -> tokio::sync::watch::Receiver<Option<ShellEnd>> {
        self.monitor.subscribe()
    }
    /// Return the final reason once observed, without consuming output.
    pub fn completion(&self) -> Option<ShellEnd> {
        self.monitor.completion()
    }
    /// Clone a write handle so receiving can continue while SSH flow control
    /// suspends an input write. Input ordering is the caller's responsibility.
    pub fn writer(&self) -> SshShellWriter {
        SshShellWriter {
            writer: self.writer.clone(),
            timeout: self.timeout,
            _session: self._session.clone(),
        }
    }
    /// Write raw terminal input with a deadline.
    pub async fn write(&self, bytes: &[u8]) -> Result<()> {
        deadline(self.timeout, "SSH terminal write", async {
            Ok(self.writer.data(bytes).await?)
        })
        .await
    }
    /// Request new remote PTY dimensions.
    pub async fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        if rows == 0 || cols == 0 {
            return Err(SessionError::Invalid("terminal dimensions must be nonzero"));
        }
        deadline(self.timeout, "SSH resize", async {
            Ok(self
                .writer
                .window_change(u32::from(cols), u32::from(rows), 0, 0)
                .await?)
        })
        .await
    }
    /// Poll an output event without blocking.
    pub fn try_recv(&mut self) -> Option<SessionEvent> {
        self.output.try_recv().ok()
    }
    /// Await the next terminal event.
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        self.output.recv().await
    }
    /// Close this channel; the parent connection remains usable.
    pub async fn close(&self) -> Result<()> {
        self.monitor.cancel();
        self.task.abort();
        deadline(self.timeout, "SSH shell close", async {
            self.writer.eof().await?;
            self.writer.close().await?;
            Ok(())
        })
        .await
    }
}

/// An owned write half for full-duplex terminal transport. Closing its shell
/// invalidates the channel; dropping this handle does not close other channels.
#[derive(Clone)]
pub struct SshShellWriter {
    writer: Arc<ChannelWriteHalf<client::Msg>>,
    timeout: Duration,
    _session: SshSession,
}

impl SshShellWriter {
    /// Send raw bytes without borrowing the event receiver. A timeout can have
    /// sent a prefix, so callers must report failure rather than replaying input.
    pub async fn write(&self, bytes: &[u8]) -> Result<()> {
        deadline(self.timeout, "SSH terminal write", async {
            Ok(self.writer.data(bytes).await?)
        })
        .await
    }
}

impl Drop for SshShell {
    fn drop(&mut self) {
        self.monitor.cancel();
        self.task.abort();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let writer = self.writer.clone();
            let timeout = self.timeout;
            runtime.spawn(async move {
                let _ = tokio::time::timeout(timeout, writer.close()).await;
            });
        }
    }
}

async fn retry_with_policy<T, F, Fut>(policy: RetryPolicy, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 1;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) if attempt < policy.max_attempts() && error.is_retryable() => {
                tokio::time::sleep(policy.delay_before_retry(attempt)).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) async fn deadline<T>(
    duration: Duration,
    operation: &'static str,
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| SessionError::Timeout(operation))?
}

async fn acknowledge(
    channel: &mut russh::Channel<client::Msg>,
    request: &'static str,
) -> Result<()> {
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(()),
            Some(ChannelMsg::Failure) => return Err(SessionError::Rejected(request)),
            // OpenSSH can advertise its receive window before acknowledging
            // a subsystem or PTY request. russh has already applied the update.
            Some(ChannelMsg::WindowAdjusted { .. }) => continue,
            _ => return Err(SessionError::Closed),
        }
    }
}

fn check_output(output: &ExecOutput, more: usize, limit: usize) -> Result<()> {
    if output
        .stdout
        .len()
        .saturating_add(output.stderr.len())
        .saturating_add(more)
        > limit
    {
        Err(SessionError::OutputLimit(limit))
    } else {
        Ok(())
    }
}

const MAX_KEYBOARD_INTERACTIVE_PROMPTS: usize = 8;
const MAX_KEYBOARD_INTERACTIVE_TEXT: usize = 16 * 1024;

fn validate_keyboard_interactive(
    name: &str,
    instructions: &str,
    prompts: &[client::Prompt],
    responses: &[Zeroizing<String>],
    response_offset: usize,
) -> Result<(Vec<String>, usize)> {
    if name.len() > MAX_KEYBOARD_INTERACTIVE_TEXT
        || instructions.len() > MAX_KEYBOARD_INTERACTIVE_TEXT
        || prompts.len() > MAX_KEYBOARD_INTERACTIVE_PROMPTS
    {
        return Err(SessionError::Invalid(
            "keyboard-interactive challenge is too large",
        ));
    }
    let mut answers = Vec::with_capacity(prompts.len());
    for (index, prompt) in prompts.iter().enumerate() {
        let response_index = response_offset.saturating_add(index);
        if prompt.prompt.len() > MAX_KEYBOARD_INTERACTIVE_TEXT {
            return Err(SessionError::Invalid(
                "keyboard-interactive prompt is too large",
            ));
        }
        let Some(response) = responses.get(response_index) else {
            return Err(SessionError::Credential(
                "keyboard-interactive response is missing".to_owned(),
            ));
        };
        if response.len() > MAX_KEYBOARD_INTERACTIVE_TEXT {
            return Err(SessionError::Invalid(
                "keyboard-interactive response is too large",
            ));
        }
        answers.push(response.to_string());
    }
    Ok((answers, prompts.len()))
}

async fn authenticate_keyboard_interactive(
    handle: &mut client::Handle<Client>,
    username: &str,
    responses: &[Zeroizing<String>],
) -> Result<bool> {
    use client::KeyboardInteractiveAuthResponse;
    let mut next = handle
        .authenticate_keyboard_interactive_start(username, None::<String>)
        .await?;
    let mut response_offset = 0usize;
    loop {
        next = match next {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let (answers, consumed) = validate_keyboard_interactive(
                    &name,
                    &instructions,
                    &prompts,
                    responses,
                    response_offset,
                )?;
                response_offset = response_offset.saturating_add(consumed);
                handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?
            }
        };
    }
}

async fn authenticate_keyboard_interactive_prompted(
    handle: &mut client::Handle<Client>,
    username: &str,
    challenges: &mpsc::Sender<KeyboardInteractiveChallenge>,
) -> Result<bool> {
    use client::KeyboardInteractiveAuthResponse;
    let mut next = handle
        .authenticate_keyboard_interactive_start(username, None::<String>)
        .await?;
    loop {
        next = match next {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                if name.len() > MAX_KEYBOARD_INTERACTIVE_TEXT
                    || instructions.len() > MAX_KEYBOARD_INTERACTIVE_TEXT
                    || prompts.len() > MAX_KEYBOARD_INTERACTIVE_PROMPTS
                {
                    return Err(SessionError::Invalid(
                        "keyboard-interactive challenge is too large",
                    ));
                }
                let prompts = prompts
                    .into_iter()
                    .map(|prompt| {
                        if prompt.prompt.len() > MAX_KEYBOARD_INTERACTIVE_TEXT {
                            return Err(SessionError::Invalid(
                                "keyboard-interactive prompt is too large",
                            ));
                        }
                        Ok(KeyboardInteractivePrompt {
                            prompt: prompt.prompt,
                            echo: prompt.echo,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let expected = prompts.len();
                let (response, received) = oneshot::channel();
                challenges
                    .send(KeyboardInteractiveChallenge {
                        name,
                        instructions,
                        prompts,
                        response,
                    })
                    .await
                    .map_err(|_| {
                        SessionError::Credential(
                            "keyboard-interactive response channel closed".to_owned(),
                        )
                    })?;
                let answers = received.await.map_err(|_| {
                    SessionError::Credential(
                        "keyboard-interactive response was cancelled".to_owned(),
                    )
                })?;
                let Some(answers) = answers else {
                    return Err(SessionError::Credential(
                        "keyboard-interactive response was cancelled".to_owned(),
                    ));
                };
                if answers.len() != expected {
                    return Err(SessionError::Credential(
                        "keyboard-interactive response count mismatch".to_owned(),
                    ));
                }
                let answers = answers
                    .into_iter()
                    .map(|answer| {
                        if answer.len() > MAX_KEYBOARD_INTERACTIVE_TEXT {
                            return Err(SessionError::Invalid(
                                "keyboard-interactive response is too large",
                            ));
                        }
                        Ok(answer.to_string())
                    })
                    .collect::<Result<Vec<_>>>()?;
                handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?
            }
        };
    }
}

async fn authenticate_agent(handle: &mut client::Handle<Client>, username: &str) -> Result<bool> {
    #[cfg(unix)]
    let mut agent = russh::keys::agent::client::AgentClient::connect_env()
        .await
        .map_err(|e| SessionError::Credential(e.to_string()))?;
    #[cfg(windows)]
    let mut agent =
        russh::keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
            .await
            .map_err(|e| SessionError::Credential(e.to_string()))?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| SessionError::Credential(e.to_string()))?;
    let hash = handle.best_supported_rsa_hash().await?.flatten();
    for identity in identities {
        if handle
            .authenticate_publickey_with(
                username,
                identity.public_key().into_owned(),
                hash,
                &mut agent,
            )
            .await
            .map_err(|e| SessionError::Credential(e.to_string()))?
            .success()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use super::{
        KeyboardInteractiveChallenge, KeyboardInteractivePrompt, RetryPolicy, SshOptions,
        SshSession, retry_with_policy, validate_keyboard_interactive,
    };
    use crate::SessionError;
    use russh::client;
    use tokio::sync::{mpsc, oneshot};
    use zeroize::Zeroizing;

    #[test]
    fn keyboard_interactive_answers_are_bounded_and_ordered() {
        let prompts = vec![
            client::Prompt {
                prompt: "Password:".into(),
                echo: false,
            },
            client::Prompt {
                prompt: "OTP:".into(),
                echo: false,
            },
        ];
        let responses = vec![
            Zeroizing::new("secret".into()),
            Zeroizing::new("123456".into()),
        ];
        let result = validate_keyboard_interactive("login", "", &prompts, &responses, 0);
        assert!(result.is_ok());
        let (answers, consumed) = match result {
            Ok(value) => value,
            Err(error) => panic!("valid keyboard-interactive prompts: {error}"),
        };
        assert_eq!(answers, vec!["secret", "123456"]);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn keyboard_interactive_missing_response_does_not_echo_prompt_or_secret() {
        let prompts = vec![client::Prompt {
            prompt: "Password:".into(),
            echo: false,
        }];
        let result = validate_keyboard_interactive("", "", &prompts, &[], 0);
        assert!(result.is_err());
        let error = match result {
            Ok(_) => panic!("missing response must fail"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "SSH credential provider: keyboard-interactive response is missing"
        );
        assert!(!error.to_string().contains("Password"));
    }

    #[tokio::test]
    async fn prompted_keyboard_interactive_challenge_is_one_shot_and_cancelable() {
        let (sender, mut receiver) = mpsc::channel(1);
        let (response, received) = oneshot::channel();
        sender
            .send(KeyboardInteractiveChallenge {
                name: "MFA".into(),
                instructions: "Approve the sign-in".into(),
                prompts: vec![KeyboardInteractivePrompt {
                    prompt: "Code".into(),
                    echo: false,
                }],
                response,
            })
            .await
            .expect("test channel remains open");
        let challenge = receiver.recv().await.expect("challenge delivered");
        challenge.response.send(None).expect("one-shot is open");
        assert!(received.await.expect("cancellation delivered").is_none());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn retry_policy_clamps_attempts_and_caps_exponential_backoff() {
        let single = RetryPolicy::new(0, Duration::from_millis(10), Duration::from_millis(5));
        assert_eq!(single.max_attempts(), 1);
        assert_eq!(single.delay_before_retry(1), Duration::from_millis(10));

        let policy = RetryPolicy::new(5, Duration::from_millis(10), Duration::from_millis(25));
        assert_eq!(policy.delay_before_retry(1), Duration::from_millis(10));
        assert_eq!(policy.delay_before_retry(2), Duration::from_millis(20));
        assert_eq!(policy.delay_before_retry(3), Duration::from_millis(25));
        assert_eq!(policy.delay_before_retry(8), Duration::from_millis(25));
        assert_eq!(
            policy.delay_before_retry(u32::MAX),
            Duration::from_millis(25)
        );
        assert_eq!(
            RetryPolicy::new(3, Duration::ZERO, Duration::ZERO).delay_before_retry(u32::MAX),
            Duration::ZERO
        );
    }

    #[test]
    fn retryable_classification_excludes_identity_auth_and_invalid_errors() {
        assert!(SessionError::Timeout("connect").is_retryable());
        assert!(
            SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "reset",
            ))
            .is_retryable()
        );
        assert!(
            SessionError::Io(std::io::Error::from(std::io::ErrorKind::WouldBlock,)).is_retryable()
        );
        assert!(SessionError::Ssh(russh::Error::Kex).is_retryable());
        assert!(SessionError::Ssh(russh::Error::ConnectionTimeout).is_retryable());
        assert!(
            !SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid",
            ))
            .is_retryable()
        );
        assert!(
            !SessionError::UnknownHostKey {
                fingerprint: "SHA256:test".into(),
            }
            .is_retryable()
        );
        assert!(
            !SessionError::ChangedHostKey {
                expected: "SHA256:old".into(),
                actual: "SHA256:new".into(),
            }
            .is_retryable()
        );
        assert!(!SessionError::Authentication.is_retryable());
        assert!(!SessionError::Credential("agent unavailable".into()).is_retryable());
        assert!(!SessionError::Invalid("bad option").is_retryable());
    }

    #[tokio::test]
    async fn retry_loop_retries_transient_failures_but_stops_on_terminal_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let result = retry_with_policy(RetryPolicy::new(3, Duration::ZERO, Duration::ZERO), {
            let calls = calls.clone();
            move || {
                let attempt = calls.fetch_add(1, Ordering::Relaxed) + 1;
                async move {
                    if attempt < 3 {
                        Err(SessionError::Timeout("connect"))
                    } else {
                        Ok(7_u8)
                    }
                }
            }
        })
        .await;
        assert!(matches!(result, Ok(7)));
        assert_eq!(calls.load(Ordering::Relaxed), 3);

        let calls = Arc::new(AtomicUsize::new(0));
        let result = retry_with_policy(RetryPolicy::new(4, Duration::ZERO, Duration::ZERO), {
            let calls = calls.clone();
            move || {
                calls.fetch_add(1, Ordering::Relaxed);
                async { Err::<u8, _>(SessionError::Authentication) }
            }
        })
        .await;
        assert!(matches!(result, Err(SessionError::Authentication)));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn public_connect_with_retry_returns_invalid_options_without_retrying() {
        let options = SshOptions::new("", "operator");
        let result = SshSession::connect_with_retry(
            options,
            RetryPolicy::new(4, Duration::from_millis(1), Duration::from_millis(1)),
        )
        .await;
        assert!(matches!(result, Err(SessionError::Invalid(_))));
    }

    #[tokio::test]
    async fn dropping_retry_future_cancels_pending_backoff() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let future = retry_with_policy(
            RetryPolicy::new(3, Duration::from_millis(50), Duration::from_millis(50)),
            move || {
                calls.fetch_add(1, Ordering::Relaxed);
                async { Err::<(), _>(SessionError::Timeout("connect")) }
            },
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), future)
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(observed.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn exhausted_retry_budget_returns_last_error_after_exact_attempt_count() {
        let calls = AtomicUsize::new(0);
        let result = retry_with_policy(RetryPolicy::new(2, Duration::ZERO, Duration::ZERO), || {
            let attempt = calls.fetch_add(1, Ordering::Relaxed);
            async move {
                if attempt == 0 {
                    Err::<(), _>(SessionError::Timeout("first"))
                } else {
                    Err(SessionError::Timeout("last"))
                }
            }
        })
        .await;
        assert!(matches!(result, Err(SessionError::Timeout("last"))));
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
}
