//! Exercise real SOCKS wire bytes through an ephemeral SSH server and TCP peers.

use super::*;
use keelshell_session::forwarding::DynamicForwardOptions;
use tokio::time::{sleep, timeout};

const LIMIT: Duration = Duration::from_secs(5);

async fn greeting(address: SocketAddr) -> Result<TcpStream, Box<dyn Error>> {
    let mut socket = timeout(LIMIT, TcpStream::connect(address)).await??;
    // Fragment the greeting deliberately, rather than relying on one TCP read.
    socket.write_all(&[5]).await?;
    socket.write_all(&[2, 2, 0]).await?;
    let mut method = [0; 2];
    timeout(LIMIT, socket.read_exact(&mut method)).await??;
    assert_eq!(method, [5, 0]);
    Ok(socket)
}

fn request(host: &str, port: u16) -> Vec<u8> {
    let mut data = vec![5, 1, 0];
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            data.push(1);
            data.extend_from_slice(&ip.octets());
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            data.push(4);
            data.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            data.extend_from_slice(&[3, host.len() as u8]);
            data.extend_from_slice(host.as_bytes());
        }
    }
    data.extend_from_slice(&port.to_be_bytes());
    data
}

async fn response(socket: &mut TcpStream, expected: u8) -> Result<(), Box<dyn Error>> {
    let mut bytes = [0; 10];
    timeout(LIMIT, socket.read_exact(&mut bytes)).await??;
    assert_eq!(bytes, [5, expected, 0, 1, 0, 0, 0, 0, 0, 0]);
    Ok(())
}

async fn rejected_eof(socket: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let mut byte = [0; 1];
    // A bounded rejected request receives FIN after its explicit response. A
    // reset here is a regression, not an acceptable substitute for the reply.
    assert_eq!(timeout(LIMIT, socket.read(&mut byte)).await??, 0);
    Ok(())
}

async fn closed(socket: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let mut byte = [0; 1];
    match timeout(LIMIT, socket.read(&mut byte)).await? {
        Ok(0) | Err(_) => Ok(()),
        Ok(_) => Err("closed socket unexpectedly delivered bytes".into()),
    }
}

async fn wait_request(server: &Server) -> Result<(), Box<dyn Error>> {
    timeout(LIMIT, async {
        while server
            .direct_requests
            .lock()
            .map(|requests| requests.is_empty())
            .unwrap_or(true)
        {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

async fn released(address: SocketAddr) -> Result<(), Box<dyn Error>> {
    timeout(LIMIT, async {
        loop {
            if let Ok(listener) = TcpListener::bind(address).await {
                drop(listener);
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

async fn roundtrip(target: &str, ipv6: bool) -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let listener = TcpListener::bind(if ipv6 { "[::1]:0" } else { "127.0.0.1:0" }).await?;
    let port = listener.local_addr()?.port();
    let forward = session
        .forward_dynamic(if ipv6 { "[::1]:0" } else { "127.0.0.1:0" }.parse()?)
        .await?;
    let address = forward.local_addr();
    let target_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        socket.write_all(b"server-first").await?;
        let mut data = vec![0; 128 * 1024];
        socket.read_exact(&mut data).await?;
        socket.write_all(&data).await?;
        socket.shutdown().await?;
        Ok::<_, std::io::Error>(data)
    });
    let mut socket = greeting(address).await?;
    socket.write_all(&request(target, port)).await?;
    response(&mut socket, 0).await?;
    let mut greeting = [0; 12];
    timeout(LIMIT, socket.read_exact(&mut greeting)).await??;
    assert_eq!(&greeting, b"server-first");
    let payload: Vec<u8> = (0..128 * 1024).map(|n| (n % 251) as u8).collect();
    timeout(LIMIT, socket.write_all(&payload)).await??;
    let mut echoed = vec![0; payload.len()];
    timeout(LIMIT, socket.read_exact(&mut echoed)).await??;
    assert_eq!(echoed, payload);
    assert_eq!(timeout(LIMIT, target_task).await???, payload);
    assert_eq!(
        *server.direct_requests.lock().map_err(|_| "request lock")?,
        vec![(target.to_owned(), u32::from(port))]
    );
    forward.close().await?;
    released(address).await?;
    assert!(!session.is_closed());
    assert_eq!(session.exec("still-open").await?.stdout, b"still-open");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_ipv4_streams_bidirectional_binary_bytes() -> Result<(), Box<dyn Error>> {
    roundtrip("127.0.0.1", false).await
}

#[tokio::test]
async fn socks_ipv6_listener_and_destination_stream_bytes() -> Result<(), Box<dyn Error>> {
    roundtrip("::1", true).await
}

#[tokio::test]
async fn socks_domain_is_sent_unchanged_for_remote_dns() -> Result<(), Box<dyn Error>> {
    roundtrip("remote-only.invalid", false).await
}

#[tokio::test]
async fn socks_rejects_auth_commands_addresses_and_malformed_requests() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    for unsupported in [[5, 1, 2], [5, 1, 1], [5, 0, 0], [4, 1, 0]] {
        let mut socket = TcpStream::connect(owner.local_addr()).await?;
        socket.write_all(&unsupported).await?;
        // Leave the response unread briefly to expose reset-on-close races.
        // The client deliberately does not shutdown its write half first.
        sleep(Duration::from_millis(20)).await;
        let mut rejected = [0; 2];
        timeout(LIMIT, socket.read_exact(&mut rejected))
            .await?
            .map_err(|error| format!("greeting {unsupported:02x?}: {error}"))?;
        assert_eq!(rejected, [5, 255]);
        rejected_eof(&mut socket).await?;
    }
    let mut bind = request("127.0.0.1", 8080);
    bind[1] = 2;
    let mut udp = request("0.0.0.0", 0);
    udp[1] = 3;
    let mut bad_reserved = request("127.0.0.1", 8080);
    bad_reserved[2] = 1;
    bad_reserved.extend_from_slice(b"buffered-tail");
    for (wire, expected) in [
        (bind, 7),
        (udp, 7),
        (bad_reserved, 1),
        (vec![5, 2, 0, 1], 7),
        (vec![5, 3, 0, 1], 7),
        (vec![5, 1, 0, 9], 8),
        (vec![4, 1, 0, 1], 1),
        (vec![5, 1, 1, 1], 1),
        (vec![5, 1, 0, 3, 0, 0, 80], 8),
        (vec![5, 1, 0, 3, 1, 0, 0, 80], 8),
        (request("127.0.0.1", 0), 1),
    ] {
        let mut socket = greeting(owner.local_addr()).await?;
        socket.write_all(&wire).await?;
        sleep(Duration::from_millis(20)).await;
        response(&mut socket, expected)
            .await
            .map_err(|error| format!("request {wire:02x?}: {error}"))?;
        rejected_eof(&mut socket).await?;
    }
    assert!(
        server
            .direct_requests
            .lock()
            .map_err(|_| "request lock")?
            .is_empty()
    );
    owner.close().await?;
    assert_eq!(
        session.exec("after-rejection").await?.stdout,
        b"after-rejection"
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_remote_rejection_has_explicit_failure_reply() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    for (host, expected) in [("blocked.invalid", 2), ("127.0.0.1", 1)] {
        let reserved = TcpListener::bind("127.0.0.1:0").await?;
        let port = reserved.local_addr()?.port();
        drop(reserved);
        let mut socket = greeting(owner.local_addr()).await?;
        let mut wire = request(host, port);
        wire.extend_from_slice(b"premature-application-data");
        socket.write_all(&wire).await?;
        sleep(Duration::from_millis(20)).await;
        response(&mut socket, expected).await?;
        rejected_eof(&mut socket).await?;
    }
    owner.close().await?;
    assert!(!session.is_closed());
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_limits_slow_handshakes_and_recovers_capacity() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let owner = session
        .forward_dynamic_with_options(
            "127.0.0.1:0".parse()?,
            DynamicForwardOptions {
                handshake_timeout: Duration::from_millis(800),
                max_connections: 1,
            },
        )
        .await?;
    let mut slow = greeting(owner.local_addr()).await?;
    slow.write_all(&[5, 1, 0, 3, 250, b'a']).await?;
    let mut excess = TcpStream::connect(owner.local_addr()).await?;
    timeout(Duration::from_millis(150), closed(&mut excess)).await??;
    closed(&mut slow).await?;
    // Reaping completed streams precedes admitting a new one.
    let mut fresh = greeting(owner.local_addr()).await?;
    fresh.write_all(&[5, 3, 0, 1]).await?;
    response(&mut fresh, 7).await?;
    owner.close().await?;
    assert!(!session.is_closed());
    assert!(
        server
            .direct_requests
            .lock()
            .map_err(|_| "request lock")?
            .is_empty()
    );
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_stop_closes_active_streams_remote_peer_and_listener() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    let address = owner.local_addr();
    let mut socket = greeting(address).await?;
    socket
        .write_all(&request("127.0.0.1", listener.local_addr()?.port()))
        .await?;
    response(&mut socket, 0).await?;
    let (mut peer, _) = timeout(LIMIT, listener.accept()).await??;
    timeout(LIMIT, owner.close()).await??;
    closed(&mut socket).await?;
    closed(&mut peer).await?;
    released(address).await?;
    assert_eq!(session.exec("still-alive").await?.stdout, b"still-alive");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_drop_and_ssh_disconnect_release_listeners_and_streams() -> Result<(), Box<dyn Error>>
{
    for action in 0..3 {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
        let address = owner.local_addr();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let mut socket = greeting(address).await?;
        socket
            .write_all(&request("127.0.0.1", listener.local_addr()?.port()))
            .await?;
        response(&mut socket, 0).await?;
        let (mut peer, _) = timeout(LIMIT, listener.accept()).await??;
        match action {
            0 => {
                drop(owner);
            }
            1 => {
                session.close().await?;
                timeout(LIMIT, owner.close()).await??;
            }
            _ => {
                server.disconnect.send_replace(true);
                released(address).await?;
                timeout(LIMIT, owner.close()).await??;
            }
        }
        closed(&mut socket).await?;
        closed(&mut peer).await?;
        released(address).await?;
    }
    Ok(())
}

#[tokio::test]
async fn socks_stop_drains_late_open_without_disconnecting_ssh() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    server.direct_delay.store(150, Ordering::Release);
    let session = SshSession::connect(options(&server)).await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    let address = owner.local_addr();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let mut socket = greeting(address).await?;
    socket
        .write_all(&request("127.0.0.1", listener.local_addr()?.port()))
        .await?;
    wait_request(&server).await?;
    timeout(LIMIT, owner.close()).await??;
    closed(&mut socket).await?;
    let (mut peer, _) = timeout(LIMIT, listener.accept()).await??;
    closed(&mut peer).await?;
    released(address).await?;
    assert_eq!(session.exec("after-stop").await?.stdout, b"after-stop");
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_unconfirmed_open_timeout_reports_shared_ssh_disconnect() -> Result<(), Box<dyn Error>>
{
    let server = serve().await?;
    server.direct_delay.store(500, Ordering::Release);
    let session = SshSession::connect(options(&server)).await?;
    let owner = session
        .forward_dynamic_with_options(
            "127.0.0.1:0".parse()?,
            DynamicForwardOptions {
                handshake_timeout: Duration::from_millis(100),
                max_connections: 1,
            },
        )
        .await?;
    let address = owner.local_addr();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let mut socket = greeting(address).await?;
    socket
        .write_all(&request("127.0.0.1", listener.local_addr()?.port()))
        .await?;
    wait_request(&server).await?;
    let result = timeout(LIMIT, owner.close()).await?;
    assert!(matches!(
        result,
        Err(SessionError::Timeout(
            "SOCKS5 SSH channel cleanup; shared SSH disconnected"
        ))
    ));
    closed(&mut socket).await?;
    released(address).await?;
    timeout(LIMIT, async {
        while !session.is_closed() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert!(session.exec("must-not-run").await.is_err());
    Ok(())
}

#[tokio::test]
async fn socks_refuses_exposed_binds_and_invalid_limits() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    for address in ["0.0.0.0:0", "[::]:0", "192.0.2.1:0"] {
        assert!(matches!(
            session.forward_dynamic(address.parse()?).await,
            Err(SessionError::Invalid(_))
        ));
    }
    for options in [
        DynamicForwardOptions {
            handshake_timeout: Duration::ZERO,
            ..Default::default()
        },
        DynamicForwardOptions {
            handshake_timeout: Duration::from_secs(61),
            ..Default::default()
        },
        DynamicForwardOptions {
            max_connections: 0,
            ..Default::default()
        },
        DynamicForwardOptions {
            max_connections: 257,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            session
                .forward_dynamic_with_options("127.0.0.1:0".parse()?, options)
                .await,
            Err(SessionError::Invalid(_))
        ));
    }
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_cancel_partial_request_never_opens_remote_channel() -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    let address = owner.local_addr();
    let mut socket = greeting(address).await?;
    socket.write_all(&[5, 1, 0, 3, 10, b'a']).await?;
    timeout(LIMIT, owner.close()).await??;
    closed(&mut socket).await?;
    released(address).await?;
    assert!(
        server
            .direct_requests
            .lock()
            .map_err(|_| "request lock")?
            .is_empty()
    );
    assert!(!session.is_closed());
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn socks_stop_cancels_rejection_drain_without_starting_ssh_channel()
-> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let owner = session.forward_dynamic("127.0.0.1:0".parse()?).await?;
    let address = owner.local_addr();
    let mut socket = greeting(address).await?;
    socket.write_all(&[5, 3, 0, 1]).await?;
    response(&mut socket, 7).await?;
    rejected_eof(&mut socket).await?;
    // Keep this client's write half open while the rejection drain is pending.
    // Stop must cancel that owned task instead of waiting for another request.
    timeout(LIMIT, owner.close()).await??;
    released(address).await?;
    assert!(
        server
            .direct_requests
            .lock()
            .map_err(|_| "request lock")?
            .is_empty()
    );
    assert!(!session.is_closed());
    session.close().await?;
    Ok(())
}
