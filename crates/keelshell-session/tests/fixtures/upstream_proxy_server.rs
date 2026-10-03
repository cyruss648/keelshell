//! Owned TCP proxy fixture with synthetic credentials and observable routing.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

#[derive(Clone, Copy, Default)]
pub(super) enum Mode {
    #[default]
    Tunnel,
    Reject,
    Stall(u8),
    Delay(Duration),
}
#[derive(Default)]
pub(super) struct Observed {
    pub accepted: AtomicUsize,
    pub closed: AtomicUsize,
    pub stage: AtomicUsize,
    pub failed: AtomicUsize,
    pub requests: Mutex<Vec<(String, u16)>>,
}
struct Closed(Arc<Observed>);
impl Drop for Closed {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.failed.fetch_add(1, Ordering::Release);
        }
        self.0.closed.fetch_add(1, Ordering::Release);
    }
}
pub(super) struct ProxyServer {
    pub address: std::net::SocketAddr,
    pub observed: Arc<Observed>,
    task: JoinHandle<()>,
}
impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl ProxyServer {
    pub fn assert_healthy(&self) {
        assert_eq!(
            self.observed.failed.load(Ordering::Acquire),
            0,
            "proxy fixture failed"
        );
    }
    pub async fn start(
        kind: ProxyKind,
        target: std::net::SocketAddr,
        authenticated: bool,
        mode: Mode,
    ) -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let observed = Arc::new(Observed::default());
        let shared = observed.clone();
        let task = tokio::spawn(async move {
            let mut jobs = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break; };
                        shared.accepted.fetch_add(1, Ordering::Release);
                        let observed = shared.clone();
                        jobs.spawn(async move {
                            let _closed = Closed(observed.clone());
                            handle(socket, kind, target, authenticated, mode, observed).await
                        });
                    }
                    result = jobs.join_next(), if !jobs.is_empty() => {
                        // Surface fixture panics instead of silently losing their evidence.
                        if let Some(Err(error)) = result && error.is_panic() { shared.failed.fetch_add(1, Ordering::Release); }
                    }
                }
            }
        });
        Ok(Self {
            address,
            observed,
            task,
        })
    }
    pub fn options(&self, kind: ProxyKind, authenticated: bool) -> SshProxy {
        SshProxy {
            kind,
            host: self.address.ip().to_string(),
            port: self.address.port(),
            credentials: authenticated.then(|| ProxyCredentials {
                username: Zeroizing::new("代理-user".into()),
                password: Zeroizing::new(" pa:ss-秘密 ".into()),
            }),
        }
    }
}
async fn stage(
    socket: &mut TcpStream,
    mode: Mode,
    observed: &Observed,
    current: u8,
) -> std::io::Result<bool> {
    observed
        .stage
        .store(usize::from(current), Ordering::Release);
    if matches!(mode, Mode::Stall(stage) if stage == current) {
        let mut byte = [0];
        assert_eq!(
            socket.read(&mut byte).await?,
            0,
            "cancelled client must close without sending another handshake frame"
        );
        return Ok(false);
    }
    Ok(true)
}
async fn handle(
    mut socket: TcpStream,
    kind: ProxyKind,
    target: std::net::SocketAddr,
    authenticated: bool,
    mode: Mode,
    observed: Arc<Observed>,
) -> std::io::Result<()> {
    socket.set_nodelay(true)?;
    let requested = match kind {
        ProxyKind::Socks5 => {
            let mut greeting = [0; 3];
            socket.read_exact(&mut greeting).await?;
            assert_eq!(greeting, [5, 1, if authenticated { 2 } else { 0 }]);
            if !stage(&mut socket, mode, &observed, 1).await? {
                return Ok(());
            }
            socket.write_all(&[5, greeting[2]]).await?;
            if authenticated {
                assert_eq!(socket.read_u8().await?, 1);
                let count = socket.read_u8().await?;
                let mut username = vec![0; usize::from(count)];
                socket.read_exact(&mut username).await?;
                let count = socket.read_u8().await?;
                let mut password = vec![0; usize::from(count)];
                socket.read_exact(&mut password).await?;
                assert_eq!(username, "代理-user".as_bytes());
                assert_eq!(password, " pa:ss-秘密 ".as_bytes());
                if !stage(&mut socket, mode, &observed, 2).await? {
                    return Ok(());
                }
                socket.write_all(&[1, 0]).await?;
            }
            let mut header = [0; 4];
            socket.read_exact(&mut header).await?;
            assert_eq!(&header[..3], &[5, 1, 0]);
            let host = match header[3] {
                1 => {
                    let mut ip = [0; 4];
                    socket.read_exact(&mut ip).await?;
                    std::net::Ipv4Addr::from(ip).to_string()
                }
                4 => {
                    let mut ip = [0; 16];
                    socket.read_exact(&mut ip).await?;
                    std::net::Ipv6Addr::from(ip).to_string()
                }
                3 => {
                    let count = socket.read_u8().await?;
                    let mut name = vec![0; usize::from(count)];
                    socket.read_exact(&mut name).await?;
                    String::from_utf8(name).map_err(std::io::Error::other)?
                }
                _ => return Err(std::io::Error::other("invalid fixture address")),
            };
            let port = socket.read_u16().await?;
            (host, port)
        }
        ProxyKind::HttpConnect => {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                request.push(socket.read_u8().await?);
            }
            let request = String::from_utf8(request).map_err(std::io::Error::other)?;
            let line = request
                .lines()
                .next()
                .ok_or_else(|| std::io::Error::other("missing CONNECT"))?;
            let authority = line
                .strip_prefix("CONNECT ")
                .and_then(|line| line.strip_suffix(" HTTP/1.1"))
                .ok_or_else(|| std::io::Error::other("bad CONNECT"))?;
            assert!(request.contains(&format!("\r\nHost: {authority}\r\n")));
            if authenticated {
                let expected = STANDARD.encode("代理-user: pa:ss-秘密 ");
                assert!(
                    request.contains(&format!("\r\nProxy-Authorization: Basic {expected}\r\n"))
                );
            } else {
                assert!(!request.contains("Proxy-Authorization"));
            }
            let (host, port) = authority
                .rsplit_once(':')
                .ok_or_else(|| std::io::Error::other("authority missing port"))?;
            if host.contains(':') {
                assert!(host.starts_with('[') && host.ends_with(']'));
            }
            (
                host.trim_matches(['[', ']']).to_owned(),
                port.parse().map_err(std::io::Error::other)?,
            )
        }
    };
    observed
        .requests
        .lock()
        .map_err(|_| std::io::Error::other("requests poisoned"))?
        .push(requested);
    if !stage(&mut socket, mode, &observed, 3).await? {
        return Ok(());
    }
    if let Mode::Delay(delay) = mode {
        tokio::time::sleep(delay).await;
    }
    if matches!(mode, Mode::Reject) {
        match kind {
            ProxyKind::Socks5 => socket.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await?,
            ProxyKind::HttpConnect => socket
                .write_all(
                    b"HTTP/1.1 407 canary-password\r\nProxy-Authenticate: canary-password\r\n\r\n",
                )
                .await?,
        }
        return Ok(());
    }
    // Resolve only to the fixed owned target; requested names cannot direct this
    // fixture outside its loopback server even when they are deliberately invalid.
    let mut target = TcpStream::connect(target).await?;
    match kind {
        ProxyKind::Socks5 => {
            // A domain bound-address verifies consuming the full reply, not ten bytes.
            socket
                .write_all(&[5, 0, 0, 3, 3, b'b', b'n', b'd', 0, 22])
                .await?;
        }
        ProxyKind::HttpConnect => {
            let mut response = b"HTTP/1.1 200 Tunnel\r\nContent-Length: 999\r\n\r\n".to_vec();
            loop {
                // Read precisely the actual banner line, then coalesce it with headers.
                let byte = target.read_u8().await?;
                response.push(byte);
                if byte == b'\n' {
                    break;
                }
                if response.len() > 2048 {
                    return Err(std::io::Error::other("fixture banner exceeded bound"));
                }
            }
            socket.write_all(&response).await?;
        }
    }
    observed.stage.store(4, Ordering::Release);
    let _ = tokio::io::copy_bidirectional(&mut socket, &mut target).await;
    Ok(())
}
