//! Explicitly run protocol integration tests with owned, ephemeral services.
//! TLS needs OpenSSL; DNS needs a pre-existing local Python image and Podman.
//! None of these tests install tools, change host trust/DNS, or contact a host
//! outside the controlled fixture. These are not native desktop acceptance.

#![cfg(unix)]

use super::*;
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
};
use tempfile::TempDir;

const CERTIFICATES: &str = r#"
import datetime, json, os, pathlib, subprocess, sys
os.umask(0o077)
root = pathlib.Path(sys.argv[1])
def run(*args):
    subprocess.run(['openssl', *args], check=True, timeout=8,
                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
run('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256',
    '-out', str(root / 'ca.key'))
run('req', '-new', '-x509', '-key', str(root / 'ca.key'), '-days', '3',
    '-subj', '/CN=KeelShell ephemeral test CA', '-out', str(root / 'ca.pem'),
    '-addext', 'basicConstraints=critical,CA:TRUE',
    '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
run('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256',
    '-out', str(root / 'leaf.key'))
(root / 'index').write_text('')
(root / 'index.attr').write_text('unique_subject = no\n')
(root / 'serial').write_text('1000\n')
(root / 'certs').mkdir()
now = datetime.datetime.now(datetime.timezone.utc)
def date(delta):
    return (now + delta).strftime('%Y%m%d%H%M%SZ')
for name, san, expired in [('valid', 'IP:127.0.0.1', False),
                           ('mismatch', 'DNS:wrong-owned.invalid', False),
                           ('expired', 'IP:127.0.0.1', True)]:
    csr, cert, config = [root / (name + suffix) for suffix in ['.csr', '.pem', '.cnf']]
    run('req', '-new', '-key', str(root / 'leaf.key'), '-subj',
        '/CN=KeelShell ephemeral leaf', '-out', str(csr))
    config.write_text('''[ca]
default_ca = owned
[owned]
database = {root}/index
serial = {root}/serial
new_certs_dir = {root}/certs
certificate = {root}/ca.pem
private_key = {root}/ca.key
default_md = sha256
policy = policy
x509_extensions = leaf
[policy]
commonName = supplied
[leaf]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
subjectAltName = {san}
'''.format(root=root, san=san))
    start = datetime.timedelta(days=-4) if expired else datetime.timedelta(minutes=-1)
    end = datetime.timedelta(days=-2) if expired else datetime.timedelta(days=2)
    run('ca', '-batch', '-notext', '-config', str(config), '-in', str(csr),
        '-out', str(cert), '-startdate', date(start), '-enddate', date(end))
run('req', '-new', '-x509', '-key', str(root / 'leaf.key'), '-days', '2',
    '-subj', '/CN=KeelShell self-signed test leaf', '-out', str(root / 'selfsigned.pem'),
    '-addext', 'subjectAltName=IP:127.0.0.1',
    '-addext', 'basicConstraints=critical,CA:FALSE',
    '-addext', 'keyUsage=critical,digitalSignature',
    '-addext', 'extendedKeyUsage=serverAuth')
print(json.dumps({'generated': ['valid', 'mismatch', 'expired', 'selfsigned']}))
"#;

const SERVICE: &str = r#"
import hashlib, json, pathlib, signal, socket, ssl, sys, time
root, mode, cert = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError()))
signal.alarm(30)
listener = socket.socket()
listener.bind(('127.0.0.1', 0))
listener.listen(8)
listener.settimeout(0.1)
context = None
fingerprint = None
if mode in ('tls', 'https'):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(str(root / (cert + '.pem')), str(root / 'leaf.key'))
    pem = (root / (cert + '.pem')).read_text()
    fingerprint = hashlib.sha256(ssl.PEM_cert_to_DER_cert(pem)).hexdigest()
stats = {'connections': 0, 'verified_sessions': 0, 'rejected': 0,
         'heads': 0, 'peer_closed': 0, 'requests': []}
(root / 'ready.json').write_text(json.dumps({'port': listener.getsockname()[1],
                                           'sha256': fingerprint}))
try:
    while not (root / 'stop').exists():
        try:
            peer, _ = listener.accept()
        except socket.timeout:
            continue
        stats['connections'] += 1
        peer.settimeout(10)
        try:
            if mode == 'slow-tls':
                # Read the real ClientHello, then withhold all handshake bytes.
                first = peer.recv(8192)
                assert first and first[0] == 22
                while peer.recv(8192):
                    pass
                stats['peer_closed'] += 1
                continue
            if context is not None:
                try:
                    peer = context.wrap_socket(peer, server_side=True)
                    stats['verified_sessions'] += 1
                except ssl.SSLError:
                    stats['rejected'] += 1
                    continue
            data = b''
            while len(data) < 8192 and not data.endswith(b'\r\n\r\n'):
                piece = peer.recv(1024)
                if not piece:
                    stats['peer_closed'] += 1
                    break
                data += piece
            if data:
                stats['requests'].append(data.decode('ascii'))
                stats['heads'] += int(data.startswith(b'HEAD '))
            if mode == 'slow-http':
                assert data.startswith(b'HEAD ')
                assert peer.recv(1) == b''
                stats['peer_closed'] += 1
            elif data:
                peer.sendall(b'HTTP/1.1 307 Temporary Redirect\r\n'
                             b'Location: https://not-followed.invalid/private\r\n'
                             b'Set-Cookie: fixture=not-reflected\r\n'
                             b'Content-Length: 0\r\nConnection: close\r\n\r\n')
        except ConnectionResetError:
            # A diagnostic-only TLS client may close without reading TLS 1.3
            # session tickets; reset is a terminal peer close, not a retry.
            stats['peer_closed'] += 1
        finally:
            peer.close()
finally:
    listener.close()
    signal.alarm(0)
    (root / 'stats.json').write_text(json.dumps(stats))
"#;

const DNS_SERVICE: &str = r#"
import json, pathlib, signal, socket, struct, sys, threading
root = pathlib.Path('/tmp/owned-dns')
root.mkdir(mode=0o700)
stop = threading.Event()
queries = []
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(('127.0.0.1', 53))
sock.settimeout(0.1)
def serve():
    while not stop.is_set():
        try:
            data, peer = sock.recvfrom(4096)
        except socket.timeout:
            continue
        at, labels = 12, []
        while data[at]:
            size = data[at]
            labels.append(data[at+1:at+1+size].decode('ascii'))
            at += size + 1
        end = at + 5
        name = '.'.join(labels)
        kind, cls = struct.unpack('!HH', data[at+1:end])
        queries.append({'name': name, 'type': kind})
        if name == 'stall-owned-diagnostic.test':
            continue
        answer = b''
        missing = name != 'owned-diagnostic.test'
        if not missing and cls == 1 and kind in (1, 28):
            value = socket.inet_pton(socket.AF_INET if kind == 1 else socket.AF_INET6,
                                     '192.0.2.41' if kind == 1 else '2001:db8::41')
            answer = b'\xc0\x0c' + struct.pack('!HHIH', kind, 1, 0, len(value)) + value
        response = data[:2] + struct.pack('!HHHHH', 0x8183 if missing else 0x8180,
                                         1, int(bool(answer)), 0, 0)
        sock.sendto(response + data[12:end] + answer, peer)
thread = threading.Thread(target=serve, name='owned-dns', daemon=False)
thread.start()
signal.signal(signal.SIGTERM, lambda *_: stop.set())
signal.signal(signal.SIGINT, lambda *_: stop.set())
signal.signal(signal.SIGALRM, lambda *_: stop.set())
signal.alarm(45)
(root / 'ready').write_text('ready\n')
print('owned DNS ready', flush=True)
stop.wait()
thread.join(timeout=2)
assert not thread.is_alive()
sock.close()
print(json.dumps({'queries': queries, 'resolver_thread_joined': True}), flush=True)
"#;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

// The process has no unbounded inherited pipe; every failure still kills and
// waits for its owned child. This helper executes only in blocking test work.
fn bounded_output(
    mut command: Command,
    seconds: u64,
) -> Result<Output, Box<dyn std::error::Error + Send + Sync>> {
    let scratch = TempDir::new()?;
    let stdout_path = scratch.path().join("stdout");
    let stderr_path = scratch.path().join("stderr");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(File::create(&stdout_path)?)
        .stderr(File::create(&stderr_path)?)
        .spawn()?;
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= until {
            child.kill()?;
            child.wait()?;
            return Err("owned helper deadline expired after kill/wait".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = std::fs::read(stdout_path)?;
    let stderr = std::fs::read(stderr_path)?;
    if stdout.len() > 65_536 || stderr.len() > 65_536 {
        return Err("owned helper output exceeds test cap".into());
    }
    println!(
        "owned protocol helper pid={} actual_wait_exit={status}",
        child.id()
    );
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn run_bounded(command: Command, seconds: u64) -> TestResult<Output> {
    let output = tokio::task::spawn_blocking(move || bounded_output(command, seconds))
        .await?
        .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })?;
    if !output.status.success() {
        return Err(format!(
            "owned helper failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

async fn certificates() -> TestResult<TempDir> {
    let root = TempDir::new()?;
    let mut command = Command::new("python3");
    command
        .args(["-I", "-S", "-B", "-c", CERTIFICATES])
        .arg(root.path());
    run_bounded(command, 35).await?;
    Ok(root)
}

struct OwnedService {
    child: Child,
    root: PathBuf,
    port: u16,
    sha256: Option<String>,
}

impl Drop for OwnedService {
    fn drop(&mut self) {
        // Failure cleanup is owned and joined; normal tests use finish first.
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl OwnedService {
    async fn start(root: &Path, mode: &str, cert: &str) -> TestResult<Self> {
        let child = Command::new("python3")
            .args(["-I", "-S", "-B", "-c", SERVICE])
            .arg(root)
            .args([mode, cert])
            .stdin(Stdio::null())
            .stdout(File::create(root.join("service.stdout"))?)
            .stderr(File::create(root.join("service.stderr"))?)
            .spawn()?;
        let mut service = Self {
            child,
            root: root.into(),
            port: 0,
            sha256: None,
        };
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(body) = tokio::fs::read(root.join("ready.json")).await
                && let Ok(ready) = serde_json::from_slice::<serde_json::Value>(&body)
            {
                service.port = u16::try_from(ready["port"].as_u64().ok_or("missing port")?)?;
                service.sha256 = ready["sha256"].as_str().map(str::to_owned);
                return Ok(service);
            }
            if service.child.try_wait()?.is_some() || Instant::now() >= until {
                return Err("owned service failed before readiness".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn finish(&mut self) -> TestResult<serde_json::Value> {
        tokio::fs::write(self.root.join("stop"), b"stop\n").await?;
        let until = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            if Instant::now() >= until {
                return Err("owned service failed to exit".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert!(status.success(), "owned protocol service {status}");
        assert!(
            matches!(tokio::net::TcpStream::connect(("127.0.0.1", self.port)).await,
                         Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused)
        );
        let stats = serde_json::from_slice(&tokio::fs::read(self.root.join("stats.json")).await?)?;
        println!(
            "owned protocol service pid={} actual_wait_exit={status}; listener refused; {stats}",
            self.child.id()
        );
        Ok(stats)
    }
}

async fn one_diagnostic(
    root: &Path,
    mode: &str,
    certificate: &str,
    request_tls: bool,
) -> TestResult<(NetworkDiagnosticReport, serde_json::Value, Option<String>)> {
    let mut service = OwnedService::start(root, mode, certificate).await?;
    let f = Fixture::new(Mode::ExecuteTrusted(root.join("ca.pem"))).await?;
    let request = if request_tls {
        NetworkDiagnosticRequest::tls("127.0.0.1", service.port)?
    } else {
        let scheme = if mode == "slow-http" { "http" } else { "https" };
        NetworkDiagnosticRequest::http(&format!("{scheme}://127.0.0.1:{}/health", service.port))?
    };
    let result = f
        .session
        .diagnose_remote(request, &AtomicBool::new(false))
        .await;
    let stats = service.finish().await;
    let ssh_cleanup = f.finish().await;
    let stats = stats?;
    ssh_cleanup?;
    Ok((result?, stats, service.sha256.take()))
}

#[tokio::test]
#[ignore = "requires POSIX Python 3 and OpenSSL; explicitly run actual_services --ignored"]
async fn trusted_tls_metadata_and_https_head_use_one_verified_connection() -> TestResult {
    for mode in ["tls", "https"] {
        let root = certificates().await?;
        let (report, stats, fingerprint) =
            one_diagnostic(root.path(), mode, "valid", mode == "tls").await?;
        assert_eq!(report.status, NetworkDiagnosticStatus::Success);
        assert_eq!(report.peer.as_deref(), Some("127.0.0.1"));
        let tls = report.tls.as_ref().ok_or("missing verified TLS")?;
        assert!(matches!(tls.protocol.as_str(), "TLSv1.2" | "TLSv1.3"));
        assert!(!tls.cipher.is_empty());
        assert_eq!(Some(&tls.sha256), fingerprint.as_ref());
        assert_eq!(tls.names, ["127.0.0.1"]);
        assert!(tls.subject.contains("KeelShell ephemeral leaf"));
        assert!(tls.issuer.contains("KeelShell ephemeral test CA"));
        assert!(!tls.not_before.is_empty() && !tls.not_after.is_empty());
        assert_eq!(stats["connections"], 1);
        assert_eq!(stats["verified_sessions"], 1);
        assert_eq!(stats["rejected"], 0);
        if mode == "https" {
            assert_eq!(report.http.as_ref().map(|http| http.status), Some(307));
            assert_eq!(stats["heads"], 1);
            let wire = stats["requests"][0].as_str().ok_or("missing HTTPS wire")?;
            assert!(wire.starts_with("HEAD /health HTTP/1.1\r\n"));
            assert!(!wire.contains("Authorization:") && !wire.contains("Cookie:"));
            assert!(!format!("{report:?}").contains("not-reflected"));
            assert!(!format!("{report:?}").contains("not-followed"));
        } else {
            assert_eq!(report.http, None);
            assert_eq!(stats["heads"], 0);
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires POSIX Python 3 and OpenSSL; explicitly run actual_services --ignored"]
async fn expired_mismatched_and_self_signed_certificates_never_retry_insecurely() -> TestResult {
    for cert in ["expired", "mismatch", "selfsigned"] {
        let root = certificates().await?;
        let (report, stats, _) = one_diagnostic(root.path(), "https", cert, false).await?;
        assert_eq!(
            report.status,
            NetworkDiagnosticStatus::CertificateRejected,
            "{cert}"
        );
        assert_eq!(report.tls, None, "{cert}");
        assert_eq!(report.http, None, "{cert}");
        assert_eq!(report.timing.tls_ms, None, "{cert}");
        assert_eq!(stats["connections"], 1, "{cert}: no retry");
        assert_eq!(stats["verified_sessions"], 0, "{cert}");
        assert_eq!(stats["heads"], 0, "{cert}: no HEAD after rejection");
        assert_eq!(stats["rejected"], 1, "{cert}");
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires POSIX Python 3; executes two real eight-second deadlines"]
async fn withheld_http_headers_and_tls_handshake_reach_remote_eight_second_deadline() -> TestResult
{
    for mode in ["slow-http", "slow-tls"] {
        let root = TempDir::new()?;
        let (report, stats, _) = one_diagnostic(root.path(), mode, "", mode == "slow-tls").await?;
        assert_eq!(report.status, NetworkDiagnosticStatus::Timeout, "{mode}");
        assert!(
            (7900..=9000).contains(&report.timing.total_ms),
            "{mode}: {report:?}"
        );
        assert_eq!(report.timing.headers_ms, None);
        assert_eq!(report.timing.tls_ms, None);
        assert_eq!(report.tls, None);
        assert_eq!(report.http, None);
        assert_eq!(stats["connections"], 1);
        assert_eq!(stats["peer_closed"], 1);
        assert_eq!(stats["heads"], usize::from(mode == "slow-http"));
    }
    Ok(())
}

struct OwnedDnsContainer {
    name: String,
    removed: bool,
}

impl Drop for OwnedDnsContainer {
    fn drop(&mut self) {
        if !self.removed {
            // Also contain a failed startup or an unwinding assertion. Only
            // this UUID namespace is touched, and each CLI is killed/waited on
            // its own deadline. Normal finish additionally proves service join.
            for args in [
                ["stop", "--time", "1"].as_slice(),
                ["wait"].as_slice(),
                ["rm", "--force"].as_slice(),
            ] {
                let mut command = Command::new("podman");
                command.args(args).arg(&self.name);
                match bounded_output(command, 5) {
                    Ok(output) => println!(
                        "owned DNS failure cleanup {:?}: {}; {}",
                        args,
                        output.status,
                        String::from_utf8_lossy(&output.stdout)
                    ),
                    Err(error) => eprintln!("owned DNS failure cleanup {:?}: {error}", args),
                }
            }
        }
    }
}

impl OwnedDnsContainer {
    async fn command(&self, args: &[&str]) -> TestResult<Output> {
        let mut command = Command::new("podman");
        command.args(args).arg(&self.name);
        run_bounded(command, 15).await
    }

    async fn finish(&mut self) -> TestResult<serde_json::Value> {
        self.command(&["stop", "--time", "3"]).await?;
        let output = self.command(&["wait"]).await?;
        assert_eq!(std::str::from_utf8(&output.stdout)?.trim(), "0");
        let logs = self.command(&["logs"]).await?;
        let text = std::str::from_utf8(&logs.stdout)?;
        let stats = serde_json::from_str(text.lines().last().ok_or("missing DNS service stats")?)?;
        self.command(&["rm"]).await?;
        let mut inspect = Command::new("python3");
        inspect.args(["-I", "-S", "-B", "-c",
                      "import subprocess,sys; p=subprocess.run(['podman','container','exists',sys.argv[1]],timeout=5); assert p.returncode==1",
                      &self.name]);
        run_bounded(inspect, 7).await?;
        self.removed = true;
        println!("owned DNS container stopped, actual wait 0, removed, exists=1; {stats}");
        Ok(stats)
    }
}

#[tokio::test]
#[ignore = "requires Podman and explicit KEELSHELL_PROTOCOL_TEST_IMAGE with Python 3; uses --pull never"]
async fn actual_isolated_dns_answers_nxdomain_and_stall_traverse_authenticated_ssh() -> TestResult {
    let image = std::env::var("KEELSHELL_PROTOCOL_TEST_IMAGE")
        .map_err(|_| "set KEELSHELL_PROTOCOL_TEST_IMAGE to a pre-existing local Python image")?;
    let root = TempDir::new()?;
    tokio::fs::write(root.path().join("dns.py"), DNS_SERVICE).await?;
    tokio::fs::write(
        root.path().join("resolv.conf"),
        b"nameserver 127.0.0.1\noptions timeout:30 attempts:1\n",
    )
    .await?;
    let name = format!("keelshell-owned-dns-{}", uuid::Uuid::new_v4());
    let volume = format!("{}:/fixture:ro", root.path().display());
    let resolver_volume = format!(
        "{}:/etc/resolv.conf:ro",
        root.path().join("resolv.conf").display()
    );
    let mut start = Command::new("podman");
    start.args([
        "run",
        "--detach",
        "--name",
        &name,
        "--pull",
        "never",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "all",
        "--cap-add",
        "NET_BIND_SERVICE",
        "--tmpfs",
        "/tmp:rw,noexec,nosuid,size=16m",
        "--volume",
        &volume,
        "--volume",
        &resolver_volume,
        &image,
        "python3",
        "-I",
        "-S",
        "-B",
        "/fixture/dns.py",
    ]);
    let mut container = OwnedDnsContainer {
        name,
        removed: false,
    };
    let work = async {
        run_bounded(start, 20).await?;
        let mut ready = Command::new("podman");
        ready.args(["exec", &container.name, "python3", "-I", "-S", "-B", "-c",
                "import pathlib,time; p=pathlib.Path('/tmp/owned-dns/ready'); end=time.monotonic()+3\nwhile not p.exists() and time.monotonic()<end: time.sleep(.01)\nassert p.exists()"]);
        run_bounded(ready, 5).await?;
        let f = Fixture::new(Mode::ExecuteInContainer(container.name.clone())).await?;
        let cancel = AtomicBool::new(false);
        let answer = f.session.diagnose_remote(NetworkDiagnosticRequest::dns("owned-diagnostic.test")?, &cancel).await;
        let missing = f.session.diagnose_remote(NetworkDiagnosticRequest::dns("missing-owned-diagnostic.test")?, &cancel).await;
        let stall = f.session.diagnose_remote(NetworkDiagnosticRequest::dns("stall-owned-diagnostic.test")?, &cancel).await;
        let mut workers = Command::new("podman");
        workers.args(["exec", &container.name, "python3", "-I", "-S", "-B", "-c",
            "import glob,os,pathlib\nmarker=b'KEELSHELL_DIAGNOSTIC_WORKER_V1'; found=[]\nfor name in glob.glob('/proc/[0-9]*/cmdline'):\n if int(name.split('/')[2])==os.getpid(): continue\n try: body=pathlib.Path(name).read_bytes()\n except FileNotFoundError: continue\n if marker in body: found.append(name)\nprint('owned DNS namespace live protocol workers:',len(found),flush=True)\nassert not found"]);
        run_bounded(workers, 5).await?;
        f.finish().await?;
        Ok::<_, Box<dyn std::error::Error>>((answer, missing, stall))
    }.await;
    let stats = container.finish().await;
    let stats = stats?;
    let (answer, missing, stall) = work?;
    let answer = answer?;
    assert_eq!(answer.status, NetworkDiagnosticStatus::Success);
    let addresses: std::collections::BTreeSet<_> = answer
        .addresses
        .iter()
        .map(|item| item.address.as_str())
        .collect();
    assert_eq!(
        addresses,
        ["192.0.2.41", "2001:db8::41"].into_iter().collect()
    );
    let missing = missing?;
    assert_eq!(missing.status, NetworkDiagnosticStatus::DnsFailure);
    assert!(missing.addresses.is_empty() && missing.peer.is_none());
    let stall = stall?;
    assert_eq!(stall.status, NetworkDiagnosticStatus::Timeout);
    assert!((7900..=9000).contains(&stall.timing.total_ms));
    assert!(stall.addresses.is_empty() && stall.peer.is_none());
    assert_eq!(stats["resolver_thread_joined"], true);
    let queries = stats["queries"].as_array().ok_or("missing DNS requests")?;
    for name in [
        "owned-diagnostic.test",
        "missing-owned-diagnostic.test",
        "stall-owned-diagnostic.test",
    ] {
        for kind in [1, 28] {
            assert!(
                queries
                    .iter()
                    .any(|q| q["name"] == name && q["type"] == kind),
                "{name} type {kind}"
            );
        }
    }
    Ok(())
}
