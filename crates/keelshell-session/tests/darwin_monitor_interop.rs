//! Opt-in kernel monitoring against an owned, pinned macOS OpenSSH service.
//!
//! Run with `python3 scripts/openssh_interop.py --monitor-darwin`. These tests
//! read the local host through SSH; no process termination or installation occurs.

use std::{path::PathBuf, time::Duration};

use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    monitor::{CapabilityState, DiskIoError, LinuxMonitor, MonitorError, MonitorPlatform},
};
use russh::keys::{HashAlg, PublicKey};

#[path = "darwin_monitor_interop/udp.rs"]
mod udp;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn is_owned_socket(socket: &keelshell_session::monitor::SocketInfo, pid: u32) -> bool {
    socket
        .process
        .as_deref()
        .is_some_and(|process| process.ends_with(&format!(" (pid={pid})")))
}

async fn connect() -> TestResult<SshSession> {
    let mut options = SshOptions::new("127.0.0.1", std::env::var("KEELSHELL_OPENSSH_USER")?);
    options.port = std::env::var("KEELSHELL_OPENSSH_PORT")?.parse()?;
    let key = std::fs::read_to_string(std::env::var("KEELSHELL_OPENSSH_HOST_KEY")?)?;
    options.expected_host_key = Some(
        PublicKey::from_openssh(&key)?
            .fingerprint(HashAlg::Sha256)
            .to_string(),
    );
    options.auth = SshAuth::PrivateKey {
        path: PathBuf::from(std::env::var("KEELSHELL_OPENSSH_CLIENT_KEY")?),
        passphrase: None,
    };
    options.timeout = Duration::from_secs(10);
    let session = SshSession::connect(options).await?;
    let platform = session.exec("/usr/bin/uname -s").await?;
    assert_eq!(platform.stdout, b"Darwin\n", "requires a real Darwin host");
    Ok(session)
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_resources_are_real_kernel_counters_with_explicit_unknowns() -> TestResult {
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let snapshot = monitor.snapshot().await?;
    assert_eq!(snapshot.platform, MonitorPlatform::Darwin);
    assert!(snapshot.boot_time > 0);
    assert!(snapshot.uptime_seconds.is_finite() && snapshot.uptime_seconds > 0.);
    assert!(snapshot.cpu.ticks.iter().any(|ticks| *ticks > 0));
    assert!(
        snapshot
            .load_average
            .iter()
            .all(|load| load.is_finite() && *load >= 0.)
    );
    assert!(!snapshot.networks.is_empty());
    assert_eq!(snapshot.disk_io, Err(DiskIoError::MissingData));
    assert_eq!(
        monitor.capabilities().await.resources.state,
        CapabilityState::Available
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_memory_reports_total_and_free_without_inventing_used() -> TestResult {
    let session = connect().await?;
    let snapshot = LinuxMonitor::new(session.clone()).snapshot().await?;
    assert!(snapshot.memory.total_bytes > 0);
    assert!(
        snapshot
            .memory
            .free_bytes
            .is_some_and(|free| free <= snapshot.memory.total_bytes)
    );
    assert!(snapshot.memory.available_bytes.is_none());
    assert!(snapshot.memory.used_bytes().is_none());
    assert!(snapshot.memory.swap_total_bytes.is_none());
    assert!(snapshot.memory.swap_free_bytes.is_none());
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_interval_has_cpu_and_network_but_no_linux_iowait_or_disk_rates() -> TestResult {
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let first = monitor.snapshot().await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    let second = monitor.snapshot().await?;
    let rates = second.rates_since(&first);
    assert!(second.uptime_seconds > first.uptime_seconds);
    assert!(
        rates
            .cpu_busy_percent
            .is_some_and(|cpu| (0. ..=100.).contains(&cpu))
    );
    assert!(rates.cpu_iowait_percent.is_none());
    assert!(rates.disks.is_empty());
    assert!(rates.networks.iter().any(|net| {
        net.received_bytes_per_second.is_some_and(f64::is_finite)
            && net.transmitted_bytes_per_second.is_some_and(f64::is_finite)
    }));
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_filesystems_use_actual_df_capacity() -> TestResult {
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let sample = monitor.snapshot().await?;
    assert!(!sample.filesystems.is_empty());
    assert!(
        sample
            .filesystems
            .iter()
            .any(|fs| fs.mount == "/" && fs.total_bytes > 0)
    );
    assert_eq!(
        monitor.capabilities().await.filesystems.state,
        CapabilityState::Available
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_ps_is_read_only_and_linux_actions_fail_closed() -> TestResult {
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let processes = monitor.processes().await?;
    let process = processes
        .iter()
        .find(|process| process.pid > 1)
        .ok_or("no process rows")?;
    assert!(
        processes
            .iter()
            .all(|p| p.cpu_percent.is_finite() && p.memory_percent.is_finite())
    );
    assert!(matches!(
        monitor.inspect_process(process).await,
        Err(MonitorError::Unsupported(_))
    ));
    assert_eq!(
        monitor.capabilities().await.processes.state,
        CapabilityState::Available
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_tcp_listeners_include_owned_ipv4_and_exclude_connected_endpoints() -> TestResult {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let client = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    let client_address = client.local_addr()?.to_string();
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let sockets = monitor.listening_sockets().await?;
    let owned = sockets
        .iter()
        .find(|socket| {
            socket.protocol == "tcp"
                && socket.local == address.to_string()
                && is_owned_socket(socket, std::process::id())
        })
        .ok_or("owned IPv4 TCP listener was not observed")?;
    assert_eq!((&*owned.protocol, &*owned.state), ("tcp", "LISTEN"));
    assert!(
        sockets
            .iter()
            .filter(|socket| socket.protocol == "tcp")
            .all(|socket| socket.state == "LISTEN"),
        "every Darwin TCP row must remain strictly LISTEN"
    );
    assert!(
        sockets.iter().all(|socket| {
            socket.protocol != "tcp"
                || !is_owned_socket(socket, std::process::id())
                || socket.local != client_address
        }),
        "the owned connected client endpoint is not a listener"
    );
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    drop(client);
    drop(listener);
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_tcp_listeners_include_owned_ipv6_and_recheck_revokes_report() -> TestResult {
    let listener = std::net::TcpListener::bind("[::1]:0")?;
    let address = listener.local_addr()?.to_string();
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let sockets = monitor.listening_sockets().await?;
    assert!(
        sockets.iter().any(|socket| {
            socket.protocol == "tcp"
                && socket.local == address
                && socket.state == "LISTEN"
                && is_owned_socket(socket, std::process::id())
        }),
        "the owned IPv6 loopback listener must be visible"
    );
    let report = monitor.capability_report().await;
    assert!(monitor.accepts_capability_report(&report));
    monitor.reset_capabilities().await;
    assert!(!monitor.accepts_capability_report(&report));
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Unknown
    );
    assert!(monitor.listening_sockets().await?.iter().any(|socket| {
        socket.protocol == "tcp"
            && socket.local == address
            && socket.state == "LISTEN"
            && is_owned_socket(socket, std::process::id())
    }));
    drop(listener);
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_ipv4_mapped_ipv6_listener_retains_lsof_numeric_ipv4_endpoint() -> TestResult {
    let listener = std::net::TcpListener::bind("[::ffff:127.0.0.1]:0")?;
    let endpoint = format!("127.0.0.1:{}", listener.local_addr()?.port());
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let sockets = monitor.listening_sockets().await?;
    assert!(
        sockets.iter().any(|socket| {
            socket.local == endpoint
                && socket.protocol == "tcp"
                && socket.state == "LISTEN"
                && is_owned_socket(socket, std::process::id())
        }),
        "the owned mapped listener must preserve the observed numeric IPv4 endpoint"
    );
    drop(listener);
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_recheck_and_new_connection_revoke_cached_capability_reports() -> TestResult {
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    monitor.snapshot().await?;
    let report = monitor.capability_report().await;
    assert!(monitor.accepts_capability_report(&report));
    monitor.reset_capabilities().await;
    assert!(!monitor.accepts_capability_report(&report));
    assert_eq!(
        monitor.capabilities().await.resources.state,
        CapabilityState::Unknown
    );
    monitor.snapshot().await?;
    let second = connect().await?;
    let new_monitor = LinuxMonitor::new(second.clone());
    assert!(!new_monitor.accepts_capability_report(&monitor.capability_report().await));
    assert_eq!(
        new_monitor.capabilities().await.resources.state,
        CapabilityState::Unknown
    );
    new_monitor.snapshot().await?;
    second.close().await?;
    session.close().await?;
    Ok(())
}
