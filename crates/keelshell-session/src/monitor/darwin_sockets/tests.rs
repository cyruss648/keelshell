#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Controlled fixtures and test assertions fail immediately on unexpected values"
)]

mod udp;

use super::super::{CapabilityState, LinuxMonitor, SOCKETS_SCRIPT};
use super::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, Semaphore};

const PROCESS: &str = "p42\0cfixture\0\n";
const FILE: &str = "f3\0tIPv4\0PTCP\0n127.0.0.1:2222\0TST=LISTEN\0\n";

fn frame(body: &str, status: u32) -> Vec<u8> {
    format!("{START}{body}{STATUS}{status}{END}").into_bytes()
}

fn output(body: &str, status: u32) -> ExecOutput {
    ExecOutput {
        stdout: frame(body, status),
        stderr: Vec::new(),
        exit_status: Some(0),
    }
}

fn sample() -> String {
    format!("{PROCESS}{FILE}")
}

fn assert_invalid(body: &str) {
    assert!(matches!(
        parse(output(body, 0)),
        Err(MonitorError::InvalidData(_))
    ));
}

#[test]
fn darwin_tcp_listen_preserves_account_visible_command_pid_and_peer() {
    let rows = parse(output(&sample(), 0)).unwrap();
    assert_eq!(
        rows,
        vec![SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*:*".into(),
            process: Some("fixture (pid=42)".into()),
        }]
    );
}

#[test]
fn darwin_socket_parser_accepts_ipv4_ipv6_wildcards_and_scoped_ipv6() {
    for (family, address, expected) in [
        ("IPv4", "127.0.0.1:65535", "127.0.0.1:65535"),
        ("IPv4", "0.0.0.0:80", "0.0.0.0:80"),
        ("IPv4", "*:443", "0.0.0.0:443"),
        ("IPv6", "*:443", "[::]:443"),
        ("IPv6", "[::1]:80", "[::1]:80"),
        ("IPv6", "[::]:80", "[::]:80"),
        ("IPv6", "[fe80::1%lo0]:80", "[fe80::1%lo0]:80"),
        ("IPv6", "[::ffff:127.0.0.1]:80", "[::ffff:127.0.0.1]:80"),
    ] {
        let body = format!("{PROCESS}f0\0t{family}\0PTCP\0n{address}\0TST=LISTEN\0\n");
        assert_eq!(parse(output(&body, 0)).unwrap()[0].local, expected);
    }
}

#[test]
fn darwin_socket_parser_accepts_multiple_files_processes_and_field_order() {
    let body = format!(
        "{PROCESS}{FILE}f4\0TST=LISTEN\0n[::1]:2222\0PTCP\0tIPv6\0\n\
         p1\0cinit worker\0\n{FILE}"
    );
    let rows = parse(output(&body, 0)).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].local, "[::1]:2222");
    assert_eq!(rows[2].process.as_deref(), Some("init worker (pid=1)"));
}

#[test]
fn darwin_socket_parser_preserves_ipv4_literal_of_ipv4_mapped_ipv6_listener() {
    let body = sample().replace("tIPv4", "tIPv6");
    let rows = parse(output(&body, 0)).unwrap();
    assert_eq!(rows[0].local, "127.0.0.1:2222");
    assert_eq!(rows[0].state, "LISTEN");
}

#[test]
fn darwin_socket_empty_requires_quiet_exit_one_and_complete_wrapper() {
    assert!(parse(output("", 1)).unwrap().is_empty());
    assert!(matches!(
        parse(output("", 0)),
        Err(MonitorError::InvalidData(_))
    ));
    assert!(matches!(
        parse(output(&sample(), 1)),
        Err(MonitorError::CapabilityUnavailable {
            reason: CapabilityReason::CommandFailed,
            ..
        })
    ));
    for bytes in [
        Vec::new(),
        START.as_bytes().to_vec(),
        format!("{START}{STATUS}1\n").into_bytes(),
        format!("{START}{STATUS}1{END}trailing").into_bytes(),
    ] {
        let result = parse(ExecOutput {
            stdout: bytes,
            stderr: Vec::new(),
            exit_status: Some(0),
        });
        assert!(matches!(result, Err(MonitorError::InvalidData(_))));
    }
}

#[test]
fn darwin_socket_parser_rejects_truncated_nul_and_set_boundaries() {
    let body = sample();
    for end in 0..body.len() {
        // The only complete prefix is a process without a file, also invalid.
        assert_invalid(&body[..end]);
    }
    for body in [
        sample().replace('\0', ""),
        sample().replace("\0\n", "\n"),
        sample().replace("\0\n", "\0\0\n"),
        format!("\n{}", sample()),
        format!("{}\n", sample()),
        format!("{PROCESS}{STATUS}0{END}"),
    ] {
        assert_invalid(&body);
    }
}

#[test]
fn darwin_socket_parser_rejects_missing_duplicate_and_unselected_fields() {
    for removed in [
        "p42\0",
        "cfixture\0",
        "f3\0",
        "tIPv4\0",
        "PTCP\0",
        "n127.0.0.1:2222\0",
        "TST=LISTEN\0",
    ] {
        assert_invalid(&sample().replace(removed, ""));
        assert_invalid(&sample().replace(removed, &format!("{removed}{removed}")));
    }
    assert_invalid(&sample().replace("TST=LISTEN\0", "TST=LISTEN\0TQR=0\0"));
    assert_invalid(&sample().replace("cfixture\0", "cfixture\0u501\0"));
    assert_invalid(&format!("{FILE}{PROCESS}"));
    assert_invalid(&format!("{PROCESS}{PROCESS}{FILE}"));
    assert_invalid(&format!("{PROCESS}{FILE}{PROCESS}{FILE}"));
    assert_invalid(&format!("{PROCESS}{FILE}{FILE}"));
    assert_invalid(&format!("{PROCESS}{FILE}p43\0cother\0\n"));
}

#[test]
fn darwin_socket_parser_rejects_udp_with_tcp_state_and_non_listening_tcp_records() {
    for invalid in ["UDP", "tcp", "SCTP", ""] {
        assert_invalid(&sample().replace("PTCP\0", &format!("P{invalid}\0")));
    }
    for invalid in ["ESTABLISHED", "CLOSED", "SYN_SENT", "LISTEN extra", ""] {
        assert_invalid(&sample().replace("TST=LISTEN\0", &format!("TST={invalid}\0")));
    }
    assert_invalid(&sample().replace("127.0.0.1:2222", "127.0.0.1:2222->127.0.0.1:1234"));
}

#[test]
fn darwin_socket_parser_rejects_invalid_pid_descriptor_and_port_numbers() {
    for invalid in ["0", "-1", "+42", "2147483648", "4294967296", " 42", "42x"] {
        assert_invalid(&sample().replace("p42\0", &format!("p{invalid}\0")));
    }
    for invalid in ["-1", "+3", "2147483648", "cwd", "3u", "3x"] {
        assert_invalid(&sample().replace("f3\0", &format!("f{invalid}\0")));
    }
    for invalid in [
        "0",
        "-1",
        "+22",
        "65536",
        "4294967296",
        "http",
        "*",
        "22 ",
        "",
    ] {
        assert_invalid(&sample().replace(":2222\0", &format!(":{invalid}\0")));
    }
}

#[test]
fn darwin_socket_parser_rejects_hostnames_family_mismatch_and_invalid_scopes() {
    for invalid in [
        "localhost:2222",
        "999.1.2.3:2222",
        "[::1]:2222",
        "127.0.0.1",
        "127.0.0.1:22;id",
        "127.0.0.1:22\n",
        "127.0.0.1:22\t",
    ] {
        assert_invalid(&sample().replace("127.0.0.1:2222", invalid));
    }
    for invalid in [
        "::1:2222",
        "[127.0.0.1]:2222",
        "999.1.2.3:2222",
        "[::gg]:2222",
        "[::1%]:2222",
        "[::1%lo0%evil]:2222",
        "[::1%lo 0]:2222",
        "[::1%lo/0]:2222",
    ] {
        assert_invalid(
            &sample()
                .replace("tIPv4", "tIPv6")
                .replace("127.0.0.1:2222", invalid),
        );
    }
    assert_invalid(&sample().replace("tIPv4", "tunknown"));
}

#[test]
fn darwin_socket_parser_bounds_command_endpoint_set_and_total_bytes() {
    assert_invalid(&sample().replace("fixture", &"c".repeat(MAX_COMMAND + 1)));
    assert_invalid(&sample().replace("fixture", "secret\u{001b}[31m"));
    assert_invalid(&sample().replace("fixture", "line\rbreak"));
    assert_invalid(&sample().replace("127.0.0.1:2222", &"1".repeat(MAX_ENDPOINT + 1)));
    assert_invalid(&format!("p42\0c{}\0\n{FILE}", "c".repeat(MAX_SET + 1)));
    let mut oversized = output("", 1);
    oversized.stderr = vec![b'x'; MAX_OUTPUT + 1];
    assert!(matches!(
        parse(oversized),
        Err(MonitorError::InvalidData(_))
    ));
    let mut oversized = output("", 1);
    oversized.stdout = vec![0; MAX_OUTPUT + 1];
    assert!(matches!(
        parse(oversized),
        Err(MonitorError::InvalidData(_))
    ));
}

#[test]
fn darwin_socket_parser_enforces_socket_count_without_truncation() {
    let mut body = PROCESS.to_owned();
    for fd in 0..MAX_SOCKETS {
        body.push_str(&FILE.replacen("f3", &format!("f{fd}"), 1));
    }
    assert_eq!(parse(output(&body, 0)).unwrap().len(), MAX_SOCKETS);
    body.push_str(&FILE.replacen("f3", &format!("f{MAX_SOCKETS}"), 1));
    assert_invalid(&body);
}

#[test]
fn darwin_socket_parser_rejects_invalid_encoding_and_wrapper_status() {
    let mut invalid_utf8 = output(&sample(), 0);
    invalid_utf8.stdout[START.len()] = 0xff;
    assert!(matches!(
        parse(invalid_utf8),
        Err(MonitorError::InvalidData(_))
    ));
    for status in ["", "-1", "+0", "4294967296", "0\n", "0\n@@KS:end@@\n"] {
        let bytes = format!("{START}{}{STATUS}{status}{END}", sample()).into_bytes();
        assert!(matches!(
            parse(ExecOutput {
                stdout: bytes,
                stderr: Vec::new(),
                exit_status: Some(0)
            }),
            Err(MonitorError::InvalidData(_))
        ));
    }
}

#[test]
fn darwin_socket_stderr_rejects_partial_success_and_empty_failure_without_echo() {
    for status in [0, 1, 2] {
        for (stderr, reason) in [
            (
                "permission denied: PRIVATE_REMOTE_TEXT",
                CapabilityReason::PermissionDenied,
            ),
            (
                "Operation not permitted: PRIVATE_REMOTE_TEXT",
                CapabilityReason::PermissionDenied,
            ),
            (
                "unknown option: PRIVATE_REMOTE_TEXT",
                CapabilityReason::UnsupportedOptions,
            ),
            (
                "warning: PRIVATE_REMOTE_TEXT",
                CapabilityReason::CommandFailed,
            ),
            (" ", CapabilityReason::CommandFailed),
        ] {
            for body in ["", &sample()] {
                let mut result = output(body, status);
                result.stderr = stderr.as_bytes().to_vec();
                let error = parse(result).unwrap_err();
                assert!(
                    matches!(error, MonitorError::CapabilityUnavailable { reason: actual, .. } if actual == reason)
                );
                assert!(!error.to_string().contains("PRIVATE_REMOTE_TEXT"));
            }
        }
    }
}

#[test]
fn darwin_socket_remote_failure_or_missing_exit_never_publishes_rows() {
    for status in [None, Some(1), Some(64), Some(66), Some(255)] {
        let mut result = output(&sample(), 0);
        result.exit_status = status;
        assert!(matches!(parse(result),
            Err(MonitorError::RemoteFailure { exit_status, .. }) if exit_status == status));
    }
    for status in [2, 65, 126, 127, 255] {
        assert!(matches!(
            parse(output("", status)),
            Err(MonitorError::CapabilityUnavailable { .. })
        ));
    }
}

#[test]
fn socket_command_keeps_linux_ss_and_absolute_read_only_darwin_selection() {
    assert!(SOCKETS_SCRIPT.contains(super::super::PLATFORM_SELECTOR));
    assert!(SOCKETS_SCRIPT.contains("\nss -H -lntupn\nks_ss_status=$?\n"));
    assert!(SOCKETS_SCRIPT.contains("printf '@@KS:linux-sockets:1@@\\n'"));
    assert!(SOCKETS_SCRIPT.contains("@@KS:ss-status@@\\n%s\\n@@KS:end@@\\n"));
    assert!(
        SOCKETS_SCRIPT.contains("/usr/sbin/lsof -nP -a -iTCP -sTCP:LISTEN -iUDP -F0pcftPnT -Ts")
    );
    assert!(SOCKETS_SCRIPT.contains("printf '@@KS:darwin-sockets:2@@\\n'"));
    for forbidden in ["sudo", "kill ", "nc ", " -w", " -Q"] {
        assert!(!SOCKETS_SCRIPT.contains(forbidden));
    }
}

#[derive(Clone)]
struct Reply {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: Option<u32>,
}

impl Reply {
    fn listening() -> Self {
        Self {
            stdout: frame(&sample(), 0),
            stderr: Vec::new(),
            status: Some(0),
        }
    }
}

struct Control {
    reply: Mutex<Reply>,
    calls: AtomicUsize,
    platform_calls: AtomicUsize,
    unexpected_command: AtomicBool,
    held: AtomicBool,
    entered: Notify,
    release: Semaphore,
}

#[derive(Clone)]
struct Peer(Arc<Control>);

impl russh::server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "fixture-test-only" {
            russh::server::Auth::Accept
        } else {
            russh::server::Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _: russh::Channel<russh::server::Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        let identity = data == super::super::platform::POSIX_PROBE.as_bytes();
        if !identity && data != SOCKETS_SCRIPT.as_bytes() {
            self.0.unexpected_command.store(true, Ordering::Release);
            session.channel_failure(channel)?;
            session.close(channel)?;
            return Ok(());
        }
        let mut reply = self.0.reply.lock().expect("controlled reply").clone();
        if identity {
            self.0.platform_calls.fetch_add(1, Ordering::SeqCst);
            let platform = if reply.stdout.starts_with(b"@@KS:linux-sockets:1@@\n") {
                "Linux"
            } else {
                "Darwin"
            };
            reply = Reply {
                stdout: format!(
                    "@@KS:monitor-platform:1@@\nplatform\t{platform}\nruntime\tposix\n@@KS:end@@\n"
                )
                .into_bytes(),
                stderr: Vec::new(),
                status: Some(0),
            };
        } else {
            self.0.calls.fetch_add(1, Ordering::SeqCst);
        }
        session.channel_success(channel)?;
        if !identity && self.0.held.load(Ordering::Acquire) {
            self.0.entered.notify_one();
            if let Ok(Ok(permit)) =
                tokio::time::timeout(Duration::from_secs(3), self.0.release.acquire()).await
            {
                permit.forget();
            } else {
                session.close(channel)?;
                return Ok(());
            }
        }
        if !reply.stdout.is_empty() {
            session.data(channel, reply.stdout)?;
        }
        if !reply.stderr.is_empty() {
            session.extended_data(channel, 1, reply.stderr)?;
        }
        if let Some(status) = reply.status {
            session.exit_status_request(channel, status)?;
        }
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

struct Fixture {
    control: Arc<Control>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture(reply: Reply) -> Result<(Fixture, LinuxMonitor), Box<dyn std::error::Error>> {
    use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(1),
        auth_rejection_time_initial: Some(Duration::from_millis(1)),
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let control = Arc::new(Control {
        reply: Mutex::new(reply),
        calls: AtomicUsize::new(0),
        platform_calls: AtomicUsize::new(0),
        unexpected_command: AtomicBool::new(false),
        held: AtomicBool::new(false),
        entered: Notify::new(),
        release: Semaphore::new(0),
    });
    let handler = Peer(control.clone());
    let task = tokio::spawn(async move {
        if let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(5), listener.accept()).await
            && let Ok(session) = russh::server::run_stream(config, socket, handler).await
        {
            let _ = session.await;
        }
    });
    let fixture = Fixture { control, task };
    let options = crate::SshOptions {
        host: address.ip().to_string(),
        port: address.port(),
        username: "fixture".into(),
        proxy: None,
        expected_host_key: Some(fingerprint),
        auth: crate::SshAuth::Password(zeroize::Zeroizing::new("fixture-test-only".into())),
        timeout: Duration::from_secs(4),
    };
    let monitor = LinuxMonitor::new(super::super::SshSession::connect(options).await?);
    Ok((fixture, monitor))
}

#[tokio::test]
async fn ssh_darwin_socket_warnings_are_cached_until_reset_then_recover()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reply = Reply::listening();
    reply.stderr = b"permission denied: fixture".to_vec();
    let (fixture, monitor) = fixture(reply).await?;
    for _ in 0..2 {
        assert!(matches!(
            monitor.listening_sockets().await,
            Err(MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::PermissionDenied,
                ..
            })
        ));
    }
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 1);
    assert_eq!(fixture.control.platform_calls.load(Ordering::Acquire), 1);
    let previous = monitor.capability_report().await;
    *fixture.control.reply.lock().unwrap() = Reply::listening();
    monitor.reset_capabilities().await;
    assert!(!monitor.accepts_capability_report(&previous));
    assert_eq!(monitor.listening_sockets().await?.len(), 1);
    let current = monitor.capabilities().await;
    assert_eq!(current.sockets.state, CapabilityState::Available);
    assert_eq!(current.resources.state, CapabilityState::Unknown);
    assert_eq!(current.processes.state, CapabilityState::Unknown);
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 2);
    assert_eq!(fixture.control.platform_calls.load(Ordering::Acquire), 2);
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_darwin_socket_quiet_empty_is_available_and_missing_tool_is_not()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(Reply {
        stdout: frame("", 1),
        stderr: Vec::new(),
        status: Some(0),
    })
    .await?;
    assert!(monitor.listening_sockets().await?.is_empty());
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    *fixture.control.reply.lock().unwrap() = Reply {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: Some(66),
    };
    assert!(matches!(
        monitor.listening_sockets().await,
        Err(MonitorError::CapabilityUnavailable {
            reason: CapabilityReason::CommandMissing,
            ..
        })
    ));
    assert!(matches!(
        monitor.listening_sockets().await,
        Err(MonitorError::CapabilityUnavailable {
            reason: CapabilityReason::CommandMissing,
            ..
        })
    ));
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 2);
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_darwin_socket_reset_rejects_a_late_capability_observation()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(Reply::listening()).await?;
    fixture.control.held.store(true, Ordering::Release);
    let pending_monitor = monitor.clone();
    let pending = tokio::spawn(async move { pending_monitor.listening_sockets().await });
    tokio::time::timeout(Duration::from_secs(2), fixture.control.entered.notified()).await?;
    monitor.reset_capabilities().await;
    fixture.control.release.add_permits(1);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(3), pending).await??,
        Err(MonitorError::Cancelled)
    ));
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Unknown
    );
    fixture.control.held.store(false, Ordering::Release);
    assert_eq!(monitor.listening_sockets().await?.len(), 1);
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    assert_eq!(fixture.control.platform_calls.load(Ordering::Acquire), 2);
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_socket_frame_cannot_change_the_cached_platform_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(Reply::listening()).await?;
    assert_eq!(monitor.listening_sockets().await?.len(), 1);
    *fixture.control.reply.lock().unwrap() = Reply {
        stdout: b"@@KS:linux-sockets:1@@\n@@KS:ss-status@@\n0\n@@KS:end@@\n".to_vec(),
        stderr: Vec::new(),
        status: Some(0),
    };
    assert!(matches!(
        monitor.listening_sockets().await,
        Err(MonitorError::InvalidData(
            "socket platform differs from the validated identity"
        ))
    ));
    assert_eq!(
        monitor
            .capabilities()
            .await
            .environment
            .map(|value| value.platform),
        Some(super::super::MonitorPlatform::Darwin)
    );
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Unavailable(CapabilityReason::InvalidOutput)
    );
    monitor.reset_capabilities().await;
    assert!(monitor.listening_sockets().await?.is_empty());
    *fixture.control.reply.lock().unwrap() = Reply::listening();
    assert!(matches!(
        monitor.listening_sockets().await,
        Err(MonitorError::InvalidData(
            "socket platform differs from the validated identity"
        ))
    ));
    assert_eq!(
        monitor
            .capabilities()
            .await
            .environment
            .map(|value| value.platform),
        Some(super::super::MonitorPlatform::Linux)
    );
    assert_eq!(fixture.control.platform_calls.load(Ordering::Acquire), 2);
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 4);
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_darwin_socket_incomplete_exit_and_overflow_never_become_empty_success()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(Reply::listening()).await?;
    for reply in [
        Reply {
            stdout: frame("", 1),
            stderr: Vec::new(),
            status: None,
        },
        Reply {
            stdout: format!("{START}{PROCESS}").into_bytes(),
            stderr: Vec::new(),
            status: Some(0),
        },
        Reply {
            stdout: vec![b'x'; MAX_OUTPUT + 1],
            stderr: Vec::new(),
            status: Some(0),
        },
    ] {
        monitor.reset_capabilities().await;
        *fixture.control.reply.lock().unwrap() = reply;
        assert!(monitor.listening_sockets().await.is_err());
        assert_ne!(
            monitor.capabilities().await.sockets.state,
            CapabilityState::Available
        );
    }
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_socket_dispatch_requires_platform_frame_and_preserves_framed_linux_empty()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(Reply::listening()).await?;
    for (stdout, stderr) in [
        (Vec::new(), Vec::new()),
        (b" \n\t".to_vec(), Vec::new()),
        (Vec::new(), b"permission denied".to_vec()),
        (
            b"tcp LISTEN 0 128 127.0.0.1:2222 0.0.0.0:*\n".to_vec(),
            Vec::new(),
        ),
        (
            b"@@KS:linux-sockets:1@@\n@@KS:ss-status@@\n0\n".to_vec(),
            Vec::new(),
        ),
        (
            b"@@KS:linux-sockets:1@@\n@@KS:ss-status@@\n0\n@@KS:end@@\n".to_vec(),
            b"partial output warning".to_vec(),
        ),
    ] {
        monitor.reset_capabilities().await;
        *fixture.control.reply.lock().unwrap() = Reply {
            stdout,
            stderr,
            status: Some(0),
        };
        assert!(
            monitor.listening_sockets().await.is_err(),
            "an unframed, incomplete or warning-bearing collection is not a valid empty result"
        );
        assert_ne!(
            monitor.capabilities().await.sockets.state,
            CapabilityState::Available
        );
    }
    monitor.reset_capabilities().await;
    *fixture.control.reply.lock().unwrap() = Reply {
        stdout: b"@@KS:linux-sockets:1@@\n@@KS:ss-status@@\n0\n@@KS:end@@\n".to_vec(),
        stderr: Vec::new(),
        status: Some(0),
    };
    assert!(monitor.listening_sockets().await?.is_empty());
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    monitor.session.close().await?;
    Ok(())
}
