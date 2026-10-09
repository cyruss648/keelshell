use super::*;
use crate::{SshAuth, SshOptions};
use russh::server;
use std::{
    process::Command,
    sync::{Arc, Mutex, atomic::AtomicUsize},
};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinHandle};
mod actual_services;
#[cfg(unix)]
mod supervisor;

fn dns_request() -> NetworkDiagnosticRequest {
    NetworkDiagnosticRequest::dns("localhost").unwrap_or_else(|e| panic!("{e}"))
}
fn dns_body() -> Vec<u8> {
    format!("{FRAME}{}",serde_json::json!({"version":1,"status":"success","addresses":[{"family":"IPv4","address":"127.0.0.1"}],"peer":null,"tls":null,"http":null,"limited":false,"timing":{"resolve_ms":1,"connect_ms":null,"tls_ms":null,"headers_ms":null,"total_ms":2}})).into_bytes()
}

#[test]
fn response_validation_rejects_missing_phase_forgery_and_unsafe_metadata() {
    let body = dns_body();
    assert!(parse_report(dns_request(), &body).is_ok());
    for (from, to) in [
        ("\"version\":1", "\"version\":2"),
        ("\"family\":\"IPv4\"", "\"family\":\"IPv6\""),
        ("\"total_ms\":2", "\"total_ms\":0"),
        ("\"resolve_ms\":1", "\"resolve_ms\":null"),
        ("\"peer\":null", "\"peer\":\"8.8.8.8\""),
    ] {
        let text = String::from_utf8(body.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            parse_report(dns_request(), text.replace(from, to).as_bytes()).is_err(),
            "{from}"
        );
    }
    let tls = NetworkDiagnosticRequest::tls("localhost", 443).unwrap_or_else(|e| panic!("{e}"));
    assert!(parse_report(tls, &body).is_err());
    assert!(parse_report(dns_request(), &vec![0; MAX_DIAGNOSTIC_OUTPUT_BYTES + 1]).is_err());
    assert!(!safe_text("name\u{202e}evil", 1024));
    assert!(!safe_text("name\nissuer", 1024));
}

#[derive(Clone)]
enum Mode {
    Body(Vec<u8>, u32),
    HoldOpen,
    HoldReply,
    #[cfg(unix)]
    Execute,
    #[cfg(unix)]
    ExecuteTrusted(std::path::PathBuf),
    #[cfg(unix)]
    ExecuteInContainer(String),
}
struct Control {
    commands: AtomicUsize,
    opened: AtomicUsize,
    closed: AtomicUsize,
    gate: Semaphore,
    seen: Mutex<Vec<Vec<u8>>>,
}
struct Peer {
    mode: Mode,
    control: Arc<Control>,
}
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.control.opened.fetch_add(1, Ordering::Release);
        if matches!(self.mode, Mode::HoldOpen) {
            let permit = self
                .control
                .gate
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
        }
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: russh::ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.control.commands.fetch_add(1, Ordering::Release);
        if let Ok(mut seen) = self.control.seen.lock() {
            seen.push(data.to_vec());
        }
        session.channel_success(id)?;
        if matches!(self.mode, Mode::HoldReply) {
            let permit = self
                .control
                .gate
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
        }
        let (body, status) = match &self.mode {
            Mode::Body(body, status) => (body.clone(), *status),
            #[cfg(unix)]
            Mode::Execute | Mode::ExecuteTrusted(_) | Mode::ExecuteInContainer(_) => {
                let trust = match &self.mode {
                    Mode::ExecuteTrusted(path) => Some(path.clone()),
                    _ => None,
                };
                let container = match &self.mode {
                    Mode::ExecuteInContainer(name) => Some(name.clone()),
                    _ => None,
                };
                let command = std::str::from_utf8(data)
                    .map_err(|_| russh::Error::Disconnect)?
                    .to_owned();
                tokio::task::spawn_blocking(move || {
                    let mut child = if let Some(container) = container {
                        let mut child = Command::new("podman");
                        child.args(["exec", &container, "/bin/sh", "-c", &command]);
                        child
                    } else {
                        let mut child = Command::new("/bin/sh");
                        child.arg("-c").arg(command);
                        child
                    };
                    if let Some(trust) = trust {
                        child.env("SSL_CERT_FILE", trust);
                    }
                    let output = child.output();
                    match output {
                        Ok(output) => (
                            output.stdout,
                            output
                                .status
                                .code()
                                .and_then(|n| u32::try_from(n).ok())
                                .unwrap_or(255),
                        ),
                        Err(_) => (Vec::new(), 255),
                    }
                })
                .await
                .map_err(|_| russh::Error::Disconnect)?
            }
            _ => (dns_body(), 0),
        };
        if !body.is_empty() {
            session.data(id, body)?;
        }
        session.exit_status_request(id, status)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
    async fn channel_close(
        &mut self,
        _: russh::ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.control.closed.fetch_add(1, Ordering::Release);
        Ok(())
    }
}
struct Fixture {
    task: JoinHandle<()>,
    session: SshSession,
    control: Arc<Control>,
    port: u16,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new(mode: Mode) -> Result<Self, Box<dyn std::error::Error>> {
        use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x46; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let control = Arc::new(Control {
            commands: AtomicUsize::new(0),
            opened: AtomicUsize::new(0),
            closed: AtomicUsize::new(0),
            gate: Semaphore::new(0),
            seen: Mutex::new(Vec::new()),
        });
        let peer = Peer {
            mode,
            control: control.clone(),
        };
        let task = tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, socket, peer).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(3);
        let session = SshSession::connect(options).await?;
        Ok(Self {
            task,
            session,
            control,
            port: address.port(),
        })
    }
    async fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.control.gate.add_permits(8);
        self.session.close().await?;
        self.task.abort();
        let joined = tokio::time::timeout(Duration::from_secs(2), &mut self.task).await?;
        assert!(joined.is_ok() || joined.as_ref().is_err_and(|e| e.is_cancelled()));
        assert!(
            matches!(tokio::net::TcpStream::connect(("127.0.0.1",self.port)).await,Err(e) if e.kind()==std::io::ErrorKind::ConnectionRefused)
        );
        Ok(())
    }
}
async fn wait_counter(counter: &AtomicUsize) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(2), async {
        while counter.load(Ordering::Acquire) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn explicit_ssh_frame_is_bounded_and_missing_python_is_actionable()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new(Mode::Body(dns_body(), 0)).await?;
    let cancel = AtomicBool::new(false);
    let report = f.session.diagnose_remote(dns_request(), &cancel).await?;
    assert_eq!(report.status, NetworkDiagnosticStatus::Success);
    {
        let seen = f.control.seen.lock().map_err(|_| "lock")?;
        let command = std::str::from_utf8(&seen[0])?;
        assert!(command.contains("python3 -I -S -B"));
        assert!(!command.contains("pip install"));
        assert!(!command.contains("curl "));
    }
    f.finish().await?;
    let f = Fixture::new(Mode::Body(Vec::new(), 66)).await?;
    assert_eq!(
        f.session.diagnose_remote(dns_request(), &cancel).await,
        Err(NetworkDiagnosticError::PythonUnavailable)
    );
    f.finish().await?;
    Ok(())
}

#[tokio::test]
async fn revoke_while_channel_open_is_held_never_dispatches_exec()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new(Mode::HoldOpen).await?;
    let cancel = Arc::new(AtomicBool::new(false));
    let control = cancel.clone();
    let session = f.session.clone();
    let job = tokio::spawn(async move { session.diagnose_remote(dns_request(), &control).await });
    wait_counter(&f.control.opened).await?;
    cancel.store(true, Ordering::Release);
    f.control.gate.add_permits(1);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), job).await??,
        Err(NetworkDiagnosticError::Cancelled)
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(f.control.commands.load(Ordering::Acquire), 0);
    f.finish().await?;
    Ok(())
}

#[tokio::test]
async fn cancel_inflight_reply_discards_result_and_pre_cancel_opens_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new(Mode::HoldReply).await?;
    let cancel = Arc::new(AtomicBool::new(true));
    assert_eq!(
        f.session.diagnose_remote(dns_request(), &cancel).await,
        Err(NetworkDiagnosticError::Cancelled)
    );
    assert_eq!(f.control.opened.load(Ordering::Acquire), 0);
    cancel.store(false, Ordering::Release);
    let control = cancel.clone();
    let session = f.session.clone();
    let job = tokio::spawn(async move { session.diagnose_remote(dns_request(), &control).await });
    wait_counter(&f.control.commands).await?;
    cancel.store(true, Ordering::Release);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), job).await??,
        Err(NetworkDiagnosticError::Cancelled)
    );
    f.control.gate.add_permits(1);
    f.finish().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_exit_and_flood_cannot_be_success() -> Result<(), Box<dyn std::error::Error>> {
    let cancel = AtomicBool::new(false);
    for (body, status, error) in [
        (
            dns_body(),
            1,
            NetworkDiagnosticError::UnsupportedEnvironment,
        ),
        (b"bad".to_vec(), 0, NetworkDiagnosticError::InvalidResponse),
        (
            vec![b'x'; MAX_DIAGNOSTIC_OUTPUT_BYTES + 1],
            0,
            NetworkDiagnosticError::OutputLimit,
        ),
    ] {
        let f = Fixture::new(Mode::Body(body, status)).await?;
        assert_eq!(
            f.session.diagnose_remote(dns_request(), &cancel).await,
            Err(error)
        );
        f.finish().await?;
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn real_remote_python_resolves_and_sends_one_unauthenticated_head()
-> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = Fixture::new(Mode::Execute).await?;
    let cancel = AtomicBool::new(false);
    let resolved = f.session.diagnose_remote(dns_request(), &cancel).await?;
    assert_eq!(resolved.status, NetworkDiagnosticStatus::Success);
    assert!(resolved.addresses.iter().any(|a| a.address == "127.0.0.1"));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let http = tokio::spawn(async move {
        let (mut socket, _) =
            tokio::time::timeout(Duration::from_secs(10), listener.accept()).await??;
        let mut bytes = vec![0; 4096];
        let count = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut bytes)).await??;
        socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://not-followed.invalid/secret\r\nSet-Cookie: private=hidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(bytes[..count].to_vec())
    });
    let request = NetworkDiagnosticRequest::http(&format!("http://127.0.0.1:{port}/health"))?;
    let report = f.session.diagnose_remote(request, &cancel).await?;
    assert_eq!(report.status, NetworkDiagnosticStatus::Success);
    assert_eq!(report.http.as_ref().map(|h| h.status), Some(302));
    assert!(report.timing.headers_ms.is_some());
    assert_eq!(report.tls, None);
    let request_bytes = http.await?.map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&request_bytes)?;
    assert!(text.starts_with("HEAD /health HTTP/1.1\r\n"));
    assert!(!text.contains("Authorization:"));
    assert!(!text.contains("Cookie:"));
    assert!(!format!("{report:?}").contains("private=hidden"));
    assert!(!format!("{report:?}").contains("not-followed"));
    assert!(
        matches!(tokio::net::TcpStream::connect(("127.0.0.1",port)).await,Err(e) if e.kind()==std::io::ErrorKind::ConnectionRefused)
    );
    f.finish().await?;
    Ok(())
}
