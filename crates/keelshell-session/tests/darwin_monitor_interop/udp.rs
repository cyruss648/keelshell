//! Owned UDP endpoints observed through the real Darwin SSH collector.
//!
//! BOUND and PEER describe lsof endpoint shapes, not kernel connection state,
//! a completed handshake, reachability or service health.

use std::{
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use keelshell_session::monitor::{CapabilityState, LinuxMonitor, SocketInfo};

use super::{TestResult, connect, is_owned_socket};

fn owned_endpoint<'a>(
    sockets: &'a [SocketInfo],
    pid: u32,
    protocol: &str,
    local: SocketAddr,
) -> TestResult<&'a SocketInfo> {
    let local = local.to_string();
    let mut matches = sockets.iter().filter(|socket| {
        socket.protocol == protocol && socket.local == local && is_owned_socket(socket, pid)
    });
    let socket = matches.next().ok_or("owned socket was not observed")?;
    assert!(
        matches.next().is_none(),
        "a single owned endpoint must not produce duplicate rows"
    );
    Ok(socket)
}

fn assert_bound(sockets: &[SocketInfo], pid: u32, local: SocketAddr) -> TestResult {
    let socket = owned_endpoint(sockets, pid, "udp", local)?;
    assert_eq!((&*socket.state, &*socket.peer), ("BOUND", "*:*"));
    Ok(())
}

async fn check_loopback_bound(address: &str) -> TestResult {
    let socket = UdpSocket::bind(address)?;
    let local = socket.local_addr()?;
    assert_ne!(local.port(), 0);
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    let sockets = monitor.listening_sockets().await?;
    assert_bound(&sockets, std::process::id(), local)?;
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    session.close().await?;
    drop(socket);
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_udp_ipv4_loopback_is_bound_with_unspecified_peer() -> TestResult {
    check_loopback_bound("127.0.0.1:0").await
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_udp_ipv6_loopback_is_bound_with_unspecified_peer() -> TestResult {
    check_loopback_bound("[::1]:0").await
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_udp_wildcards_preserve_ipv4_and_ipv6_families() -> TestResult {
    let ipv4 = UdpSocket::bind("0.0.0.0:0")?;
    let ipv6 = UdpSocket::bind("[::]:0")?;
    let session = connect().await?;
    let sockets = LinuxMonitor::new(session.clone())
        .listening_sockets()
        .await?;
    assert_bound(&sockets, std::process::id(), ipv4.local_addr()?)?;
    assert_bound(&sockets, std::process::id(), ipv6.local_addr()?)?;
    session.close().await?;
    drop((ipv4, ipv6));
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_udp_peers_preserve_exact_ipv4_and_ipv6_endpoints() -> TestResult {
    let ipv4_peer = UdpSocket::bind("127.0.0.1:0")?;
    let ipv4 = UdpSocket::bind("127.0.0.1:0")?;
    ipv4.connect(ipv4_peer.local_addr()?)?;
    let ipv6_peer = UdpSocket::bind("[::1]:0")?;
    let ipv6 = UdpSocket::bind("[::1]:0")?;
    ipv6.connect(ipv6_peer.local_addr()?)?;
    let session = connect().await?;
    let sockets = LinuxMonitor::new(session.clone())
        .listening_sockets()
        .await?;
    for (socket, peer) in [(&ipv4, &ipv4_peer), (&ipv6, &ipv6_peer)] {
        let expected_peer = peer.local_addr()?.to_string();
        assert_eq!(socket.peer_addr()?.to_string(), expected_peer);
        let observed = owned_endpoint(&sockets, std::process::id(), "udp", socket.local_addr()?)?;
        assert_eq!(
            (&*observed.state, &*observed.peer),
            ("PEER", &*expected_peer)
        );
        assert_bound(&sockets, std::process::id(), peer.local_addr()?)?;
    }
    session.close().await?;
    drop((ipv4, ipv4_peer, ipv6, ipv6_peer));
    Ok(())
}

// std exposes only bound UDP constructors, and socket2 is not a direct
// dependency. The existing Python runner's stdlib can hold unbound descriptors
// without adding a dependency or unsafe code. A TCP control channel carries
// only readiness metadata and keeps their lifetime under this fixture's owner.
const UNBOUND_FIXTURE: &str = r#"
import contextlib
import os
import socket
import struct
import sys

with contextlib.ExitStack() as stack:
    sockets = [
        stack.enter_context(socket.socket(family, socket.SOCK_DGRAM))
        for family in (socket.AF_INET, socket.AF_INET6)
    ]
    assert all(sock.getsockname()[1] == 0 for sock in sockets)
    marker = stack.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
    marker.bind(("127.0.0.1", 0))
    control = stack.enter_context(
        socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=3)
    )
    control.settimeout(90)
    control.sendall(struct.pack("!4sIH", b"KSUB", os.getpid(), marker.getsockname()[1]))
    assert control.recv(1) == b""
    assert all(sock.getsockname()[1] == 0 for sock in sockets)
"#;

struct UnboundUdpFixture {
    child: Child,
    control: Option<TcpStream>,
    marker: SocketAddr,
}

impl UnboundUdpFixture {
    async fn start() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let child = Command::new("python3")
            .args(["-I", "-S", "-B", "-c", UNBOUND_FIXTURE])
            .arg(listener.local_addr()?.port().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut fixture = Self {
            child,
            control: None,
            marker: SocketAddr::from(([127, 0, 0, 1], 0)),
        };
        let until = Instant::now() + Duration::from_secs(5);
        let mut control = loop {
            fixture.assert_running()?;
            match listener.accept() {
                Ok((control, _)) => break control,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error.into()),
            }
            if Instant::now() >= until {
                return Err("owned unbound UDP fixture readiness timed out".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        control.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut ready = [0; 10];
        control.read_exact(&mut ready)?;
        assert_eq!(&ready[..4], b"KSUB");
        assert_eq!(
            u32::from_be_bytes([ready[4], ready[5], ready[6], ready[7]]),
            fixture.child.id(),
            "readiness must belong to the owned child"
        );
        let port = u16::from_be_bytes([ready[8], ready[9]]);
        assert_ne!(port, 0);
        fixture.marker.set_port(port);
        fixture.control = Some(control);
        Ok(fixture)
    }

    fn assert_running(&mut self) -> TestResult {
        if self.child.try_wait()?.is_some() {
            return Err("owned unbound UDP fixture exited before collection completed".into());
        }
        Ok(())
    }

    async fn close(mut self) -> TestResult {
        drop(self.control.take());
        let until = Instant::now() + Duration::from_secs(3);
        let status: ExitStatus = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            if Instant::now() >= until {
                return Err("owned unbound UDP fixture shutdown timed out".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert!(status.success(), "owned unbound UDP fixture failed");
        Ok(())
    }
}

impl Drop for UnboundUdpFixture {
    fn drop(&mut self) {
        drop(self.control.take());
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.child.kill();
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        eprintln!("owned unbound UDP fixture cleanup did not finish within its deadline");
    }
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_udp_unbound_sockets_are_omitted_without_failing_inventory() -> TestResult {
    let mut fixture = UnboundUdpFixture::start().await?;
    let session = connect().await?;
    let monitor = LinuxMonitor::new(session.clone());
    fixture.assert_running()?;
    let sockets = monitor.listening_sockets().await?;
    fixture.assert_running()?;
    assert_bound(&sockets, fixture.child.id(), fixture.marker)?;
    assert_eq!(
        sockets
            .iter()
            .filter(|socket| is_owned_socket(socket, fixture.child.id()))
            .count(),
        1,
        "only the bound marker may be listed for the PID holding two unbound UDP sockets"
    );
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    fixture.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the disposable macOS OpenSSH monitoring fixture"]
async fn darwin_combined_tcp_udp_inventory_still_excludes_owned_tcp_client() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let tcp_local = listener.local_addr()?;
    let client = TcpStream::connect_timeout(&tcp_local, Duration::from_secs(2))?;
    let client_local = client.local_addr()?.to_string();
    let bound = UdpSocket::bind("127.0.0.1:0")?;
    let peer = UdpSocket::bind("127.0.0.1:0")?;
    let connected = UdpSocket::bind("127.0.0.1:0")?;
    connected.connect(peer.local_addr()?)?;
    let session = connect().await?;
    let sockets = LinuxMonitor::new(session.clone())
        .listening_sockets()
        .await?;
    let pid = std::process::id();
    let tcp = owned_endpoint(&sockets, pid, "tcp", tcp_local)?;
    assert_eq!((&*tcp.state, &*tcp.peer), ("LISTEN", "*:*"));
    assert_bound(&sockets, pid, bound.local_addr()?)?;
    assert_bound(&sockets, pid, peer.local_addr()?)?;
    let udp = owned_endpoint(&sockets, pid, "udp", connected.local_addr()?)?;
    assert_eq!(
        (&*udp.state, &*udp.peer),
        ("PEER", &*peer.local_addr()?.to_string())
    );
    assert!(
        sockets
            .iter()
            .filter(|socket| socket.protocol == "tcp")
            .all(|socket| socket.state == "LISTEN"),
        "adding UDP must not admit any non-LISTEN TCP row"
    );
    assert!(
        sockets.iter().all(|socket| {
            socket.protocol != "tcp"
                || !is_owned_socket(socket, pid)
                || socket.local != client_local
        }),
        "the owned TCP client must remain excluded from the combined inventory"
    );
    session.close().await?;
    drop((listener, client, bound, peer, connected));
    Ok(())
}
