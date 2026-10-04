//! Memory-only, authenticated desktop IPC for the outward MCP stdio adapter.
//!
//! A copied capability identifies access to the current listener, not an OS user
//! or executable. No bootstrap file, peer credentials, or persisted secret is
//! used. Mutual challenge proofs precede encrypted protocol bytes.

use std::{
    fmt,
    future::Future,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
};
use hmac::{Hmac, KeyInit, Mac};
use rand::TryRngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{
        TcpListener, TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use crate::{KeelShellMcpServer, serve_stream_with_shutdown};

/// Environment variable for a copied, current loopback listener address.
pub const MCP_ADDRESS_ENV: &str = "KEELSHELL_MCP_ADDRESS";
/// Environment variable for the copied 256-bit capability. Never use argv.
pub const MCP_SECRET_ENV: &str = "KEELSHELL_MCP_SECRET";
/// Maximum simultaneous connections, including incomplete authentications.
pub const MAX_IPC_CONNECTIONS: usize = 8;
/// Maximum plaintext bytes in one authenticated data record.
pub const MAX_IPC_RECORD_BYTES: usize = 16 * 1024;
const AUTH_TIMEOUT: Duration = Duration::from_secs(2);
const MAGIC: &[u8; 8] = b"KSMCP001";
const HEADER_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const MAX_CIPHER_BYTES: usize = MAX_IPC_RECORD_BYTES + 1 + TAG_BYTES;

type HmacSha256 = Hmac<Sha256>;

/// Static IPC failures. Diagnostic text never contains copied capabilities,
/// protocol bodies, filesystem paths, or private operating-system errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum IpcFailure {
    /// Both environment fields must be present and have the exact format.
    #[error("invalid MCP desktop launch configuration")]
    InvalidConfiguration,
    /// The current desktop listener could not be created or reached.
    #[error("MCP desktop listener is unavailable")]
    Unavailable,
    /// Mutual authentication failed; no MCP context was sent.
    #[error("MCP desktop authentication failed")]
    Authentication,
    /// Connecting or authentication exceeded two seconds.
    #[error("MCP desktop authentication exceeded its deadline")]
    AuthenticationDeadline,
    /// An underlying stream stopped unexpectedly.
    #[error("MCP desktop transport stopped unexpectedly")]
    Transport,
    /// A record was truncated, oversized, reordered, replayed, or modified.
    #[error("MCP desktop encrypted record was rejected")]
    Record,
    /// Desktop ownership was cancelled or revoked.
    #[error("MCP desktop connection was cancelled")]
    Cancelled,
    /// Owned connection cleanup did not finish within its fixed deadline.
    #[error("MCP desktop shutdown exceeded its deadline")]
    Shutdown,
}

/// Owns one memory-only desktop listener and its accepted MCP services.
/// Dropping the host cancels its streams and backend connection lifecycles.
/// The desktop remains responsible for policy replacement/revocation: this
/// host never disables a shared controller that a newer host may already use.
pub struct DesktopIpcHost {
    address: SocketAddr,
    secret: Arc<Zeroizing<[u8; 32]>>,
    cancelled: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl fmt::Debug for DesktopIpcHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopIpcHost")
            .field("cancelled", &self.cancelled.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl DesktopIpcHost {
    /// Bind a random IPv4 loopback port and serve the original desktop-owned
    /// server after authentication. Call on a Tokio worker, never the UI thread.
    /// No SSH connection or grant is created by binding this listener.
    ///
    /// # Errors
    /// Returns a static error if entropy or listener creation fails.
    pub async fn bind(server: KeelShellMcpServer) -> Result<Self, IpcFailure> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| IpcFailure::Unavailable)?;
        let address = listener.local_addr().map_err(|_| IpcFailure::Unavailable)?;
        let secret = Arc::new(random_bytes()?);
        let cancelled = CancellationToken::new();
        let task = tokio::spawn(accept_connections(
            listener,
            server,
            secret.clone(),
            cancelled.clone(),
        ));
        Ok(Self {
            address,
            secret,
            cancelled,
            task: Some(task),
        })
    }

    /// Create the explicit clipboard launch configuration. The returned JSON
    /// contains a sensitive memory capability; display/copy it only after a
    /// deliberate user action. It expires on host drop or desktop restart and
    /// is never suitable for logging, metadata storage, or command arguments.
    pub fn launch_environment(&self) -> String {
        self.configuration("keelshell-mcp")
    }

    /// Serialize an explicitly chosen adapter path into the copied launch
    /// configuration. This performs no filesystem access or process launch;
    /// the desktop checks the packaged adapter on a background worker first.
    ///
    /// # Errors
    /// Returns [`IpcFailure::InvalidConfiguration`] for an empty or non-UTF-8
    /// command path. Returned capability text must not be logged or persisted.
    pub fn launch_environment_for_command(
        &self,
        command: &std::path::Path,
    ) -> Result<String, IpcFailure> {
        let command = command
            .to_str()
            .filter(|value| !value.is_empty())
            .ok_or(IpcFailure::InvalidConfiguration)?;
        Ok(self.configuration(command))
    }

    fn configuration(&self, command: &str) -> String {
        let secret = encode_secret(self.secret.as_ref());
        serde_json::json!({
            "command": command, "args": [], "env": {
                MCP_ADDRESS_ENV: self.address.to_string(), MCP_SECRET_ENV: secret.as_str()
            }
        })
        .to_string()
    }

    /// Cancel this listener and wait for bounded cleanup of accepted services.
    /// This closes transport ownership; it does not undo remote SSH effects.
    ///
    /// # Errors
    /// Returns [`IpcFailure::Shutdown`] if worker cleanup exceeds three seconds.
    pub async fn close(mut self) -> Result<(), IpcFailure> {
        self.cancelled.cancel();
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
                Ok(Ok(())) => Ok(()),
                _ => {
                    task.abort();
                    let _ = task.await;
                    Err(IpcFailure::Shutdown)
                }
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for DesktopIpcHost {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}

async fn accept_connections(
    listener: TcpListener,
    server: KeelShellMcpServer,
    secret: Arc<Zeroizing<[u8; 32]>>,
    cancelled: CancellationToken,
) {
    let capacity = Arc::new(Semaphore::new(MAX_IPC_CONNECTIONS));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {},
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else { break; };
                if peer.ip() != Ipv4Addr::LOCALHOST { continue; }
                let Ok(permit) = capacity.clone().try_acquire_owned() else { continue; };
                let secret = secret.clone();
                let shutdown = cancelled.child_token();
                let cancelled_for_connection = cancelled.clone();
                let server = server.clone();
                connections.spawn(async move {
                    let _permit = permit;
                    let authenticated = tokio::select! {
                        biased;
                        _ = shutdown.cancelled() => return,
                        result = tokio::time::timeout(AUTH_TIMEOUT, authenticate_server(stream, secret.as_ref(), shutdown.clone())) => result,
                    };
                    let Ok(Ok(stream)) = authenticated else { return; };
                    let stream = Arc::new(Mutex::new(stream));
                    let (reader, writer) = tokio::io::split(SharedIpcStream(stream.clone()));
                    let _ = serve_stream_with_shutdown(server, reader, writer, shutdown).await;
                    if !cancelled_for_connection.is_cancelled() {
                        // The SDK drops its writer instead of calling shutdown.
                        // Retain socket ownership until its service has drained,
                        // then emit an authenticated EOF rather than accepting a
                        // truncated TCP stream as a successful protocol close.
                        let _ = tokio::time::timeout(Duration::from_millis(250), std::future::poll_fn(|cx| {
                            match stream.lock() {
                                Ok(mut stream) => stream.poll_finish(cx),
                                Err(_) => Poll::Ready(Err(io::Error::other(IpcFailure::Transport))),
                            }
                        })).await;
                    }
                });
            }
        }
    }
    drop(listener);
    cancelled.cancel();
    // RunningService receives explicit cancellation and has a two-second drain.
    // The final bound prevents an unexpectedly stuck child from retaining sockets.
    if tokio::time::timeout(Duration::from_millis(2500), async {
        while connections.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        connections.shutdown().await;
    }
}

/// Validated current desktop launch capability for the transparent adapter.
/// Debug formatting deliberately omits both secret and endpoint.
pub struct DesktopIpcClient {
    address: SocketAddr,
    secret: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for DesktopIpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopIpcClient").finish_non_exhaustive()
    }
}

impl DesktopIpcClient {
    /// Validate explicitly supplied launch fields without connecting.
    /// Only a nonzero `127.0.0.1:port` and exactly 64 hex digits are accepted.
    ///
    /// # Errors
    /// Returns [`IpcFailure::InvalidConfiguration`] for any invalid field.
    pub fn new(address: &str, secret: &str) -> Result<Self, IpcFailure> {
        let address: SocketAddr = address
            .parse()
            .map_err(|_| IpcFailure::InvalidConfiguration)?;
        if !matches!(address, SocketAddr::V4(value) if *value.ip() == Ipv4Addr::LOCALHOST && value.port() != 0)
        {
            return Err(IpcFailure::InvalidConfiguration);
        }
        Ok(Self {
            address,
            secret: decode_secret(secret)?,
        })
    }

    /// Read the two dedicated environment fields. No fields preserves disabled
    /// standalone discovery. A missing partner, non-UTF-8 value, or invalid field
    /// fails closed and must never silently fall back to the disconnected server.
    ///
    /// # Errors
    /// Returns [`IpcFailure::InvalidConfiguration`] for partial/invalid settings.
    pub fn from_environment() -> Result<Option<Self>, IpcFailure> {
        match (
            std::env::var_os(MCP_ADDRESS_ENV),
            std::env::var_os(MCP_SECRET_ENV),
        ) {
            (None, None) => Ok(None),
            (Some(address), Some(secret)) => {
                let address = address.to_str().ok_or(IpcFailure::InvalidConfiguration)?;
                let secret = secret
                    .into_string()
                    .map(Zeroizing::new)
                    .map_err(|_| IpcFailure::InvalidConfiguration)?;
                Self::new(address, secret.as_str()).map(Some)
            }
            _ => Err(IpcFailure::InvalidConfiguration),
        }
    }

    /// Authenticate the desktop before sending any protocol bytes. The complete
    /// connect/challenge exchange has a two-second deadline. No SDK or copied
    /// authority exists on this side of the connection.
    ///
    /// # Errors
    /// Returns static availability, authentication, or deadline failures.
    pub async fn connect(&self) -> Result<AuthenticatedIpcStream, IpcFailure> {
        tokio::time::timeout(AUTH_TIMEOUT, async {
            let stream = TcpStream::connect(self.address)
                .await
                .map_err(|_| IpcFailure::Unavailable)?;
            authenticate_client(stream, &self.secret, CancellationToken::new()).await
        })
        .await
        .map_err(|_| IpcFailure::AuthenticationDeadline)?
    }

    /// Transparently bridge stdin/stdout to the authenticated desktop stream.
    /// Stdout contains only desktop-produced JSON-RPC bytes. On stdin EOF, send
    /// authenticated EOF and bound the response drain to two seconds.
    ///
    /// # Errors
    /// Returns static authentication/transport failures without private payloads.
    pub async fn bridge_stdio(&self) -> Result<(), IpcFailure> {
        self.bridge(tokio::io::stdin(), tokio::io::stdout()).await
    }

    /// Bridge arbitrary owned byte streams using the exact stdio adapter path.
    /// Useful for integration checks without subprocess or environment mutation.
    ///
    /// # Errors
    /// Returns authentication/transport failures or a bounded drain failure.
    pub async fn bridge<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
        &self,
        mut input: R,
        mut output: W,
    ) -> Result<(), IpcFailure> {
        let stream = self.connect().await?;
        let (mut reader, mut writer) = tokio::io::split(stream);
        let send = async {
            tokio::io::copy(&mut input, &mut writer)
                .await
                .map_err(|_| IpcFailure::Transport)?;
            writer.shutdown().await.map_err(|_| IpcFailure::Transport)
        };
        let receive = async {
            tokio::io::copy(&mut reader, &mut output)
                .await
                .map_err(|_| IpcFailure::Transport)?;
            output.flush().await.map_err(|_| IpcFailure::Transport)
        };
        tokio::pin!(send, receive);
        tokio::select! {
            result = &mut send => {
                result?;
                tokio::time::timeout(Duration::from_secs(2), &mut receive).await.map_err(|_| IpcFailure::Shutdown)?
            },
            result = &mut receive => result,
        }
    }
}

fn random_bytes() -> Result<Zeroizing<[u8; 32]>, IpcFailure> {
    let mut bytes = Zeroizing::new([0; 32]);
    rand::rngs::OsRng
        .try_fill_bytes(bytes.as_mut())
        .map_err(|_| IpcFailure::Unavailable)?;
    Ok(bytes)
}

fn encode_secret(secret: &[u8; 32]) -> Zeroizing<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = Zeroizing::new(String::with_capacity(64));
    for byte in secret {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    text
}

fn decode_secret(secret: &str) -> Result<Zeroizing<[u8; 32]>, IpcFailure> {
    if secret.len() != 64 {
        return Err(IpcFailure::InvalidConfiguration);
    }
    let mut bytes = Zeroizing::new([0; 32]);
    for (index, pair) in secret.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = char::from(pair[0])
            .to_digit(16)
            .ok_or(IpcFailure::InvalidConfiguration)?;
        let low = char::from(pair[1])
            .to_digit(16)
            .ok_or(IpcFailure::InvalidConfiguration)?;
        bytes[index] = ((high << 4) | low) as u8;
    }
    Ok(bytes)
}

fn transcript(client: &[u8; 32], server: &[u8; 32]) -> [u8; 72] {
    let mut result = [0; 72];
    result[..8].copy_from_slice(MAGIC);
    result[8..40].copy_from_slice(client);
    result[40..].copy_from_slice(server);
    result
}

fn mac(secret: &[u8; 32], domain: &[u8], transcript: &[u8; 72]) -> Result<HmacSha256, IpcFailure> {
    let mut mac =
        <HmacSha256 as KeyInit>::new_from_slice(secret).map_err(|_| IpcFailure::Authentication)?;
    mac.update(domain);
    mac.update(transcript);
    Ok(mac)
}

fn proof(secret: &[u8; 32], domain: &[u8], transcript: &[u8; 72]) -> Result<[u8; 32], IpcFailure> {
    Ok(mac(secret, domain, transcript)?
        .finalize()
        .into_bytes()
        .into())
}

fn verify(
    secret: &[u8; 32],
    domain: &[u8],
    transcript: &[u8; 72],
    tag: &[u8],
) -> Result<(), IpcFailure> {
    mac(secret, domain, transcript)?
        .verify_slice(tag)
        .map_err(|_| IpcFailure::Authentication)
}

async fn authenticate_client(
    mut stream: TcpStream,
    secret: &[u8; 32],
    cancel: CancellationToken,
) -> Result<AuthenticatedIpcStream, IpcFailure> {
    let client = random_bytes()?;
    stream
        .write_all(MAGIC)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    stream
        .write_all(client.as_ref())
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    let mut response = [0; 64];
    stream
        .read_exact(&mut response)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    let mut server = [0; 32];
    server.copy_from_slice(&response[..32]);
    let transcript = transcript(&client, &server);
    verify(
        secret,
        b"keelshell-mcp/server-proof/v1",
        &transcript,
        &response[32..],
    )?;
    stream
        .write_all(&proof(
            secret,
            b"keelshell-mcp/client-proof/v1",
            &transcript,
        )?)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    let mut acknowledgement = [0; 32];
    stream
        .read_exact(&mut acknowledgement)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    verify(
        secret,
        b"keelshell-mcp/finished/v1",
        &transcript,
        &acknowledgement,
    )?;
    AuthenticatedIpcStream::new(stream, secret, &transcript, false, cancel)
}

async fn authenticate_server(
    mut stream: TcpStream,
    secret: &[u8; 32],
    cancel: CancellationToken,
) -> Result<AuthenticatedIpcStream, IpcFailure> {
    let mut hello = [0; 40];
    stream
        .read_exact(&mut hello)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    if &hello[..8] != MAGIC {
        return Err(IpcFailure::Authentication);
    }
    let mut client = [0; 32];
    client.copy_from_slice(&hello[8..]);
    let server = random_bytes()?;
    let transcript = transcript(&client, &server);
    stream
        .write_all(server.as_ref())
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    stream
        .write_all(&proof(
            secret,
            b"keelshell-mcp/server-proof/v1",
            &transcript,
        )?)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    let mut tag = [0; 32];
    stream
        .read_exact(&mut tag)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    verify(secret, b"keelshell-mcp/client-proof/v1", &transcript, &tag)?;
    stream
        .write_all(&proof(secret, b"keelshell-mcp/finished/v1", &transcript)?)
        .await
        .map_err(|_| IpcFailure::Authentication)?;
    AuthenticatedIpcStream::new(stream, secret, &transcript, true, cancel)
}

struct RecordCodec {
    cipher: ChaCha20Poly1305,
    transcript_hash: [u8; 32],
    direction: u8,
    counter: u64,
}

impl RecordCodec {
    fn new(secret: &[u8; 32], transcript: &[u8; 72], direction: u8) -> Result<Self, IpcFailure> {
        // Domain-separated HMAC acts as a PRF over the full fresh transcript.
        // Each direction receives its own key and nonce namespace.
        let domain = if direction == 0 {
            b"keelshell-mcp/client-to-server/key/v1".as_slice()
        } else {
            b"keelshell-mcp/server-to-client/key/v1".as_slice()
        };
        let key = Zeroizing::new(proof(secret, domain, transcript)?);
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_ref())
            .map_err(|_| IpcFailure::Authentication)?;
        Ok(Self {
            cipher,
            transcript_hash: Sha256::digest(transcript).into(),
            direction,
            counter: 0,
        })
    }

    fn nonce(&self) -> Nonce {
        let mut bytes = [0; 12];
        bytes[3] = self.direction;
        bytes[4..].copy_from_slice(&self.counter.to_be_bytes());
        bytes.into()
    }

    fn associated_data(&self, header: &[u8; HEADER_BYTES]) -> [u8; 45] {
        let mut aad = [0; 45];
        aad[..32].copy_from_slice(&self.transcript_hash);
        aad[32] = self.direction;
        aad[33..].copy_from_slice(header);
        aad
    }

    fn encode(&mut self, data: &[u8], eof: bool) -> Result<Vec<u8>, IpcFailure> {
        if self.counter == u64::MAX
            || data.len() > MAX_IPC_RECORD_BYTES
            || (!eof && data.is_empty())
            || (eof && !data.is_empty())
        {
            return Err(IpcFailure::Record);
        }
        let mut plain = Zeroizing::new(Vec::with_capacity(data.len() + 1));
        plain.push(u8::from(!eof));
        plain.extend_from_slice(data);
        let mut header = [0; HEADER_BYTES];
        header[..4].copy_from_slice(&((plain.len() + TAG_BYTES) as u32).to_be_bytes());
        header[4..].copy_from_slice(&self.counter.to_be_bytes());
        let mut wire = header.to_vec();
        wire.extend_from_slice(
            &self
                .cipher
                .encrypt(
                    &self.nonce(),
                    Payload {
                        msg: plain.as_ref(),
                        aad: &self.associated_data(&header),
                    },
                )
                .map_err(|_| IpcFailure::Record)?,
        );
        self.counter += 1;
        Ok(wire)
    }

    fn decode(
        &mut self,
        header: &[u8; HEADER_BYTES],
        ciphertext: &[u8],
    ) -> Result<Option<Zeroizing<Vec<u8>>>, IpcFailure> {
        if self.counter == u64::MAX
            || u64::from_be_bytes(header[4..].try_into().map_err(|_| IpcFailure::Record)?)
                != self.counter
            || ciphertext.len() > MAX_CIPHER_BYTES
            || ciphertext.len() < TAG_BYTES + 1
            || u32::from_be_bytes(header[..4].try_into().map_err(|_| IpcFailure::Record)?) as usize
                != ciphertext.len()
        {
            return Err(IpcFailure::Record);
        }
        let mut plain = Zeroizing::new(
            self.cipher
                .decrypt(
                    &self.nonce(),
                    Payload {
                        msg: ciphertext,
                        aad: &self.associated_data(header),
                    },
                )
                .map_err(|_| IpcFailure::Record)?,
        );
        self.counter += 1;
        match plain.first() {
            Some(0) if plain.len() == 1 => Ok(None),
            Some(1) if plain.len() > 1 => {
                plain.remove(0);
                Ok(Some(plain))
            }
            _ => Err(IpcFailure::Record),
        }
    }
}

type ReadFuture = Pin<
    Box<
        dyn Future<
                Output = (
                    OwnedReadHalf,
                    RecordCodec,
                    Result<Option<Zeroizing<Vec<u8>>>, IpcFailure>,
                ),
            > + Send,
    >,
>;
type WriteFuture = Pin<Box<dyn Future<Output = (OwnedWriteHalf, Result<(), IpcFailure>)> + Send>>;

/// Authenticated and encrypted byte stream. Each read publishes bytes only
/// after complete record authentication. Memory and ciphertext allocations are
/// bounded to one 16 KiB record per direction; no crypto pump tasks are detached.
/// Drop closes owned socket halves and cancels this connection's lifecycle.
pub struct AuthenticatedIpcStream {
    reader: Option<(OwnedReadHalf, RecordCodec)>,
    read_pending: Option<ReadFuture>,
    read_buffer: Zeroizing<Vec<u8>>,
    read_offset: usize,
    read_eof: bool,
    writer: Option<OwnedWriteHalf>,
    writer_codec: RecordCodec,
    write_pending: Option<WriteFuture>,
    write_closed: bool,
    failure: Option<IpcFailure>,
    cancelled: CancellationToken,
    cancellation: Pin<Box<dyn Future<Output = ()> + Send>>,
}

struct SharedIpcStream(Arc<Mutex<AuthenticatedIpcStream>>);

impl AsyncRead for SharedIpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.0.lock() {
            Ok(mut stream) => Pin::new(&mut *stream).poll_read(cx, buffer),
            Err(_) => Poll::Ready(Err(io::Error::other(IpcFailure::Transport))),
        }
    }
}

impl AsyncWrite for SharedIpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.0.lock() {
            Ok(mut stream) => Pin::new(&mut *stream).poll_write(cx, bytes),
            Err(_) => Poll::Ready(Err(io::Error::other(IpcFailure::Transport))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.0.lock() {
            Ok(mut stream) => Pin::new(&mut *stream).poll_flush(cx),
            Err(_) => Poll::Ready(Err(io::Error::other(IpcFailure::Transport))),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.0.lock() {
            Ok(mut stream) => Pin::new(&mut *stream).poll_shutdown(cx),
            Err(_) => Poll::Ready(Err(io::Error::other(IpcFailure::Transport))),
        }
    }
}

impl fmt::Debug for AuthenticatedIpcStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthenticatedIpcStream")
            .field("closed", &self.write_closed)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedIpcStream {
    fn new(
        stream: TcpStream,
        secret: &[u8; 32],
        transcript: &[u8; 72],
        server: bool,
        cancelled: CancellationToken,
    ) -> Result<Self, IpcFailure> {
        let (reader, writer) = stream.into_split();
        let write_direction = u8::from(server);
        Ok(Self {
            reader: Some((
                reader,
                RecordCodec::new(secret, transcript, 1 - write_direction)?,
            )),
            read_pending: None,
            read_buffer: Zeroizing::new(Vec::new()),
            read_offset: 0,
            read_eof: false,
            writer: Some(writer),
            writer_codec: RecordCodec::new(secret, transcript, write_direction)?,
            write_pending: None,
            write_closed: false,
            failure: None,
            cancellation: Box::pin(cancelled.clone().cancelled_owned()),
            cancelled,
        })
    }

    fn check(&mut self, cx: &mut Context<'_>) -> Result<(), io::Error> {
        if self.cancellation.as_mut().poll(cx).is_ready() {
            self.failure = Some(IpcFailure::Cancelled);
        }
        match self.failure {
            Some(error) => Err(io::Error::other(error)),
            None => Ok(()),
        }
    }

    fn fail(&mut self, error: IpcFailure) -> io::Error {
        self.failure = Some(error);
        self.cancelled.cancel();
        self.reader.take();
        self.writer.take();
        self.read_pending.take();
        self.write_pending.take();
        io::Error::other(error)
    }

    fn poll_pending_write(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Some(pending) = self.write_pending.as_mut() else {
            return Poll::Ready(Ok(()));
        };
        match pending.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready((writer, result)) => {
                self.write_pending = None;
                self.writer = Some(writer);
                match result {
                    Ok(()) => Poll::Ready(Ok(())),
                    Err(error) => Poll::Ready(Err(self.fail(error))),
                }
            }
        }
    }

    fn queue_record(&mut self, data: &[u8], eof: bool) -> Result<(), io::Error> {
        let wire = self
            .writer_codec
            .encode(data, eof)
            .map_err(|error| self.fail(error))?;
        let Some(mut writer) = self.writer.take() else {
            return Err(self.fail(IpcFailure::Transport));
        };
        self.write_pending = Some(Box::pin(async move {
            let result = async {
                writer
                    .write_all(&wire)
                    .await
                    .map_err(|_| IpcFailure::Transport)?;
                if eof {
                    writer.shutdown().await.map_err(|_| IpcFailure::Transport)?;
                }
                Ok(())
            }
            .await;
            (writer, result)
        }));
        Ok(())
    }

    fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(error) = self.failure.filter(|error| *error != IpcFailure::Cancelled) {
            return Poll::Ready(Err(io::Error::other(error)));
        }
        match self.poll_pending_write(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {}
        }
        if !self.write_closed {
            if let Err(error) = self.queue_record(&[], true) {
                return Poll::Ready(Err(error));
            }
            self.write_closed = true;
        }
        self.poll_pending_write(cx)
    }
}

impl Drop for AuthenticatedIpcStream {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}

impl AsyncRead for AuthenticatedIpcStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Err(error) = self.check(cx) {
            return Poll::Ready(Err(error));
        }
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.read_offset < self.read_buffer.len() {
            let length = buffer
                .remaining()
                .min(self.read_buffer.len() - self.read_offset);
            buffer.put_slice(&self.read_buffer[self.read_offset..self.read_offset + length]);
            self.read_offset += length;
            return Poll::Ready(Ok(()));
        }
        if self.read_eof {
            return Poll::Ready(Ok(()));
        }
        self.read_buffer = Zeroizing::new(Vec::new());
        self.read_offset = 0;
        if self.read_pending.is_none() {
            let Some((mut reader, mut codec)) = self.reader.take() else {
                return Poll::Ready(Err(self.fail(IpcFailure::Transport)));
            };
            self.read_pending = Some(Box::pin(async move {
                let result = async {
                    let mut header = [0; HEADER_BYTES];
                    reader
                        .read_exact(&mut header)
                        .await
                        .map_err(|_| IpcFailure::Record)?;
                    let length =
                        u32::from_be_bytes(header[..4].try_into().map_err(|_| IpcFailure::Record)?)
                            as usize;
                    if !(TAG_BYTES + 1..=MAX_CIPHER_BYTES).contains(&length) {
                        return Err(IpcFailure::Record);
                    }
                    let mut ciphertext = vec![0; length];
                    reader
                        .read_exact(&mut ciphertext)
                        .await
                        .map_err(|_| IpcFailure::Record)?;
                    codec.decode(&header, &ciphertext)
                }
                .await;
                (reader, codec, result)
            }));
        }
        let Some(pending) = self.read_pending.as_mut() else {
            return Poll::Ready(Err(self.fail(IpcFailure::Transport)));
        };
        match pending.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready((reader, codec, result)) => {
                self.read_pending = None;
                self.reader = Some((reader, codec));
                match result {
                    Ok(Some(plain)) => {
                        self.read_buffer = plain;
                        self.poll_read(cx, buffer)
                    }
                    Ok(None) => {
                        self.read_eof = true;
                        Poll::Ready(Ok(()))
                    }
                    Err(error) => Poll::Ready(Err(self.fail(error))),
                }
            }
        }
    }
}

impl AsyncWrite for AuthenticatedIpcStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.check(cx) {
            return Poll::Ready(Err(error));
        }
        if self.write_closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "MCP IPC write side is closed",
            )));
        }
        match self.poll_pending_write(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {}
        }
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let length = bytes.len().min(MAX_IPC_RECORD_BYTES);
        if let Err(error) = self.queue_record(&bytes[..length], false) {
            return Poll::Ready(Err(error));
        }
        // One accepted record may remain buffered until the next write/flush.
        // Explicit wake ensures a caller that immediately flushes makes progress.
        cx.waker().wake_by_ref();
        Poll::Ready(Ok(length))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check(cx) {
            return Poll::Ready(Err(error));
        }
        self.poll_pending_write(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.check(cx) {
            return Poll::Ready(Err(error));
        }
        self.poll_finish(cx)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use tokio::io::AsyncReadExt;

    const TEST_SECRET: [u8; 32] = [73; 32];
    fn codecs() -> (RecordCodec, RecordCodec) {
        let transcript = transcript(&[1; 32], &[2; 32]);
        (
            RecordCodec::new(&TEST_SECRET, &transcript, 0).unwrap(),
            RecordCodec::new(&TEST_SECRET, &transcript, 0).unwrap(),
        )
    }
    fn decode(
        codec: &mut RecordCodec,
        wire: &[u8],
    ) -> Result<Option<Zeroizing<Vec<u8>>>, IpcFailure> {
        codec.decode(
            wire[..HEADER_BYTES].try_into().unwrap(),
            &wire[HEADER_BYTES..],
        )
    }
    async fn pair() -> (AuthenticatedIpcStream, AuthenticatedIpcStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            let (client, server) = tokio::join!(
                async {
                    authenticate_client(
                        TcpStream::connect(address).await.unwrap(),
                        &TEST_SECRET,
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap()
                },
                async {
                    authenticate_server(
                        listener.accept().await.unwrap().0,
                        &TEST_SECRET,
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap()
                }
            );
            (client, server)
        })
        .await
        .unwrap()
    }

    #[test]
    fn records_encrypt_bounded_payload_and_authenticate_eof() {
        let (mut sender, mut receiver) = codecs();
        let payload = vec![b'K'; MAX_IPC_RECORD_BYTES];
        let wire = sender.encode(&payload, false).unwrap();
        assert!(!wire.windows(32).any(|window| window == [b'K'; 32]));
        assert_eq!(
            decode(&mut receiver, &wire).unwrap().unwrap().as_slice(),
            payload
        );
        assert!(
            decode(&mut receiver, &sender.encode(&[], true).unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn modified_ciphertext_and_header_fail_before_plaintext_admission() {
        for location in [0, 11, HEADER_BYTES, HEADER_BYTES + 7] {
            let (mut sender, mut receiver) = codecs();
            let mut wire = sender.encode(b"private context", false).unwrap();
            wire[location] ^= 1;
            assert_eq!(
                decode(&mut receiver, &wire).unwrap_err(),
                IpcFailure::Record
            );
        }
    }

    #[test]
    fn repeated_or_out_of_order_records_are_rejected() {
        let (mut sender, mut receiver) = codecs();
        let first = sender.encode(b"first", false).unwrap();
        let second = sender.encode(b"second", false).unwrap();
        assert_eq!(
            decode(&mut receiver, &second).unwrap_err(),
            IpcFailure::Record
        );
        assert!(decode(&mut receiver, &first).is_ok());
        assert_eq!(
            decode(&mut receiver, &first).unwrap_err(),
            IpcFailure::Record
        );
    }

    #[test]
    fn opposite_direction_and_fresh_transcript_cannot_decrypt_a_record() {
        let (mut sender, _) = codecs();
        let wire = sender.encode(b"private context", false).unwrap();
        let mut other_direction =
            RecordCodec::new(&TEST_SECRET, &transcript(&[1; 32], &[2; 32]), 1).unwrap();
        let mut other_transcript =
            RecordCodec::new(&TEST_SECRET, &transcript(&[1; 32], &[3; 32]), 0).unwrap();
        assert_eq!(
            decode(&mut other_direction, &wire).unwrap_err(),
            IpcFailure::Record
        );
        assert_eq!(
            decode(&mut other_transcript, &wire).unwrap_err(),
            IpcFailure::Record
        );
    }

    #[test]
    fn record_counter_exhaustion_never_wraps_to_reuse_a_nonce() {
        let (mut sender, mut receiver) = codecs();
        sender.counter = u64::MAX;
        assert_eq!(
            sender.encode(b"data", false).unwrap_err(),
            IpcFailure::Record
        );
        assert_eq!(sender.counter, u64::MAX);
        receiver.counter = u64::MAX;
        let mut normal_sender = codecs().0;
        assert_eq!(
            decode(
                &mut receiver,
                &normal_sender.encode(b"data", false).unwrap()
            )
            .unwrap_err(),
            IpcFailure::Record
        );
    }

    #[test]
    fn data_records_reject_empty_oversized_or_invalid_close_payloads() {
        let (mut sender, _) = codecs();
        for (payload, eof) in [
            (&[][..], false),
            (&b"x"[..], true),
            (&vec![0; MAX_IPC_RECORD_BYTES + 1][..], false),
        ] {
            assert_eq!(sender.encode(payload, eof).unwrap_err(), IpcFailure::Record);
        }
    }

    #[tokio::test]
    async fn wrong_capability_cannot_complete_mutual_authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (client, server) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(
                async {
                    authenticate_client(
                        TcpStream::connect(address).await.unwrap(),
                        &[72; 32],
                        CancellationToken::new(),
                    )
                    .await
                },
                async {
                    authenticate_server(
                        listener.accept().await.unwrap().0,
                        &TEST_SECRET,
                        CancellationToken::new(),
                    )
                    .await
                }
            )
        })
        .await
        .unwrap();
        assert_eq!(client.unwrap_err(), IpcFailure::Authentication);
        assert_eq!(server.unwrap_err(), IpcFailure::Authentication);
    }

    #[tokio::test]
    async fn fake_endpoint_receives_only_client_challenge_and_no_context_or_proof() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = DesktopIpcClient::new(
            &listener.local_addr().unwrap().to_string(),
            &encode_secret(&TEST_SECRET),
        )
        .unwrap();
        let (result, received) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(config.connect(), async {
                let mut peer = listener.accept().await.unwrap().0;
                let mut hello = [0; 40];
                peer.read_exact(&mut hello).await.unwrap();
                peer.write_all(&[0; 64]).await.unwrap();
                let mut after_challenge = Vec::new();
                peer.read_to_end(&mut after_challenge).await.unwrap();
                (hello, after_challenge)
            })
        })
        .await
        .unwrap();
        assert_eq!(result.unwrap_err(), IpcFailure::Authentication);
        assert_eq!(&received.0[..8], MAGIC);
        assert!(received.1.is_empty());
    }

    #[tokio::test]
    async fn fresh_server_challenge_rejects_a_replayed_client_proof() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut prior_proof: Option<[u8; 32]> = None;
        let mut prior_nonce: Option<[u8; 32]> = None;
        for _ in 0..2 {
            let mut client = TcpStream::connect(address).await.unwrap();
            let accepted = listener.accept().await.unwrap().0;
            let server = tokio::spawn(authenticate_server(
                accepted,
                &TEST_SECRET,
                CancellationToken::new(),
            ));
            client.write_all(MAGIC).await.unwrap();
            client.write_all(&[7; 32]).await.unwrap();
            let mut response = [0; 64];
            client.read_exact(&mut response).await.unwrap();
            let nonce: [u8; 32] = response[..32].try_into().unwrap();
            let proof = proof(
                &TEST_SECRET,
                b"keelshell-mcp/client-proof/v1",
                &transcript(&[7; 32], &nonce),
            )
            .unwrap();
            if let Some(old) = prior_proof {
                assert_ne!(prior_nonce.unwrap(), nonce);
                client.write_all(&old).await.unwrap();
                let result = tokio::time::timeout(Duration::from_secs(1), server)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(result.unwrap_err(), IpcFailure::Authentication);
            } else {
                prior_proof = Some(proof);
                prior_nonce = Some(nonce);
                client.write_all(&proof).await.unwrap();
                let mut ack = [0; 32];
                client.read_exact(&mut ack).await.unwrap();
                drop(
                    tokio::time::timeout(Duration::from_secs(1), server)
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap(),
                );
            }
        }
    }

    #[tokio::test]
    async fn truncated_or_oversized_record_keeps_readbuf_unmodified_on_error() {
        for oversized in [false, true] {
            let (mut client, mut server) = pair().await;
            let mut raw = server.writer.take().unwrap();
            if oversized {
                let mut header = [0; HEADER_BYTES];
                header[..4].copy_from_slice(&u32::MAX.to_be_bytes());
                raw.write_all(&header).await.unwrap();
            } else {
                let wire = server
                    .writer_codec
                    .encode(b"must stay private", false)
                    .unwrap();
                raw.write_all(&wire[..wire.len() - 1]).await.unwrap();
                raw.shutdown().await.unwrap();
            }
            let mut destination = [b'Z'; 32];
            let result =
                tokio::time::timeout(Duration::from_secs(1), client.read(&mut destination))
                    .await
                    .unwrap();
            assert!(result.is_err());
            assert_eq!(destination, [b'Z'; 32]);
        }
    }

    #[tokio::test]
    async fn ciphertext_tamper_and_record_replay_on_tcp_never_publish_partial_context() {
        for replay in [false, true] {
            let (mut client, mut server) = pair().await;
            let mut raw = server.writer.take().unwrap();
            let mut wire = server
                .writer_codec
                .encode(b"private context", false)
                .unwrap();
            if replay {
                raw.write_all(&wire).await.unwrap();
                let mut first = [0; 15];
                client.read_exact(&mut first).await.unwrap();
                assert_eq!(&first, b"private context");
            } else {
                let end = wire.len() - 1;
                wire[end] ^= 1;
            }
            raw.write_all(&wire).await.unwrap();
            let mut destination = [b'Z'; 32];
            assert!(
                tokio::time::timeout(Duration::from_secs(1), client.read(&mut destination))
                    .await
                    .unwrap()
                    .is_err()
            );
            assert_eq!(destination, [b'Z'; 32]);
        }
    }

    #[tokio::test]
    async fn cancellation_wakes_pending_read_and_rejects_buffered_plaintext() {
        let (mut client, mut server) = pair().await;
        server.write_all(b"private context").await.unwrap();
        server.flush().await.unwrap();
        let mut first = [0; 1];
        client.read_exact(&mut first).await.unwrap();
        client.cancelled.cancel();
        let mut destination = [b'Z'; 32];
        assert!(client.read(&mut destination).await.is_err());
        assert_eq!(destination, [b'Z'; 32]);
        let (mut client, _server) = pair().await;
        let cancel = client.cancelled.clone();
        let reader = tokio::spawn(async move {
            let mut byte = [0];
            client.read(&mut byte).await
        });
        tokio::task::yield_now().await;
        cancel.cancel();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), reader)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
    }

    #[tokio::test]
    async fn authenticated_stream_preserves_multiple_records_and_graceful_half_close() {
        let (mut client, mut server) = pair().await;
        let bytes = vec![93; MAX_IPC_RECORD_BYTES * 3 + 7];
        let expected = bytes.clone();
        let sender = tokio::spawn(async move {
            client.write_all(&bytes).await.unwrap();
            client.shutdown().await.unwrap();
            client
        });
        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(2), server.read_to_end(&mut received))
            .await
            .unwrap()
            .unwrap();
        let mut client = sender.await.unwrap();
        assert_eq!(received, expected);
        server.write_all(b"finished").await.unwrap();
        server.shutdown().await.unwrap();
        let mut reply = Vec::new();
        client.read_to_end(&mut reply).await.unwrap();
        assert_eq!(reply, b"finished");
    }
}
