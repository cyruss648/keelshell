//! Opt-in forwarding lifecycle checks against the disposable, pinned OpenSSH
//! runner. Every listener and destination is loopback-only and owned by a test.

use std::{error::Error, net::SocketAddr, path::PathBuf, time::Duration};

use keelshell_session::{SessionError, SshAuth, SshOptions, SshSession};
use russh::keys::{HashAlg, PublicKey};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::{JoinHandle, JoinSet},
    time::timeout,
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
const TEST_DEADLINE: Duration = Duration::from_secs(25);
const CLEANUP_DEADLINE: Duration = Duration::from_secs(3);

async fn connect() -> TestResult<SshSession> {
    let mut options = SshOptions::new("127.0.0.1", std::env::var("KEELSHELL_OPENSSH_USER")?);
    options.port = std::env::var("KEELSHELL_OPENSSH_PORT")?.parse()?;
    let public = std::fs::read_to_string(std::env::var("KEELSHELL_OPENSSH_HOST_KEY")?)?;
    options.expected_host_key = Some(
        PublicKey::from_openssh(&public)?
            .fingerprint(HashAlg::Sha256)
            .to_string(),
    );
    options.auth = SshAuth::PrivateKey {
        path: PathBuf::from(std::env::var("KEELSHELL_OPENSSH_CLIENT_KEY")?),
        passphrase: None,
    };
    options.timeout = Duration::from_secs(5);
    Ok(SshSession::connect(options).await?)
}

struct EchoServer {
    address: SocketAddr,
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl EchoServer {
    async fn start() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (cancel, mut cancelled) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut streams = JoinSet::new();
            loop {
                tokio::select! {
                    _ = cancelled.changed() => break,
                    incoming = listener.accept() => {
                        let Ok((mut stream, _)) = incoming else { break };
                        streams.spawn(async move {
                            let mut bytes = [0; 4096];
                            while let Ok(count) = stream.read(&mut bytes).await {
                                if count == 0 || stream.write_all(&bytes[..count]).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    _ = streams.join_next(), if !streams.is_empty() => {}
                }
            }
            streams.abort_all();
            while streams.join_next().await.is_some() {}
        });
        Ok(Self {
            address,
            cancel,
            task,
        })
    }

    async fn close(mut self) -> TestResult {
        self.cancel.send_replace(true);
        timeout(CLEANUP_DEADLINE, &mut self.task).await??;
        Ok(())
    }
}

impl Drop for EchoServer {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        self.task.abort();
    }
}

async fn exchange(stream: &mut TcpStream, marker: u8) -> TestResult {
    let payload: Vec<_> = (0..70_031).map(|index| (index as u8) ^ marker).collect();
    stream.write_all(&payload).await?;
    let mut received = vec![0; payload.len()];
    stream.read_exact(&mut received).await?;
    assert_eq!(received, payload, "forwarded bytes must remain exact");
    Ok(())
}

async fn stream_closed(stream: &mut TcpStream) -> TestResult {
    let mut byte = [0];
    match timeout(CLEANUP_DEADLINE, stream.read(&mut byte)).await? {
        Ok(0) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) =>
        {
            Ok(())
        }
        result => Err(format!("owned stream did not close: {result:?}").into()),
    }
}

async fn listener_released(address: SocketAddr) -> TestResult {
    assert!(
        TcpStream::connect(address).await.is_err(),
        "the old endpoint must refuse new clients"
    );
    let rebound = TcpListener::bind(address).await?;
    assert_eq!(rebound.local_addr()?, address);
    Ok(())
}

async fn local_owner_closed(owner: &keelshell_session::forwarding::LocalForward) -> TestResult {
    timeout(CLEANUP_DEADLINE, async {
        while !owner.is_closed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

async fn session_survives(session: &SshSession) -> TestResult {
    let result = session.exec("printf forwarding-owner-survives").await?;
    assert_eq!(result.exit_status, Some(0));
    assert_eq!(result.stdout, b"forwarding-owner-survives");
    Ok(())
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_local_stop_closes_only_its_listener_and_streams() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let echo = EchoServer::start().await?;
        let ssh = connect().await?;
        let bind = "127.0.0.1:0".parse()?;
        let start = || ssh.forward_local(bind, "127.0.0.1".into(), echo.address.port());
        let first = start().await?;
        let second = start().await?;
        let address = first.local_addr();
        let mut stream = TcpStream::connect(address).await?;
        exchange(&mut stream, 0x51).await?;
        let mut sibling = TcpStream::connect(second.local_addr()).await?;
        exchange(&mut sibling, 0xa3).await?;
        timeout(CLEANUP_DEADLINE, async {
            loop {
                let traffic = first.traffic().snapshot();
                if traffic.sent == 70_031 && traffic.received == 70_031 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;
        first.close().await;
        stream_closed(&mut stream).await?;
        listener_released(address).await?;
        exchange(&mut sibling, 0x64).await?;
        session_survives(&ssh).await?;
        second.close().await;
        stream_closed(&mut sibling).await?;
        ssh.close().await?;
        echo.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_local_disconnect_releases_listener_without_an_app_poller() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let echo = EchoServer::start().await?;
        let ssh = connect().await?;
        let owner = ssh
            .forward_local(
                "127.0.0.1:0".parse()?,
                "127.0.0.1".into(),
                echo.address.port(),
            )
            .await?;
        let address = owner.local_addr();
        let mut stream = TcpStream::connect(address).await?;
        exchange(&mut stream, 0x72).await?;
        ssh.close().await?;
        local_owner_closed(&owner).await?;
        stream_closed(&mut stream).await?;
        listener_released(address).await?;
        owner.close().await;
        echo.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_closed_session_cannot_allocate_a_local_listener() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let ssh = connect().await?;
        ssh.close().await?;
        let reservation = TcpListener::bind("127.0.0.1:0").await?;
        let address = reservation.local_addr()?;
        drop(reservation);
        let result = ssh.forward_local(address, "127.0.0.1".into(), 9).await;
        assert!(matches!(result, Err(SessionError::Closed)));
        listener_released(address).await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_replacement_requires_new_owner_and_old_close_cannot_stop_it() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let echo = EchoServer::start().await?;
        let old = connect().await?;
        let owner = old
            .forward_local(
                "127.0.0.1:0".parse()?,
                "127.0.0.1".into(),
                echo.address.port(),
            )
            .await?;
        let address = owner.local_addr();
        let replacement = connect().await?;
        assert!(!old.same_connection(&replacement));
        old.close().await?;
        local_owner_closed(&owner).await?;
        listener_released(address).await?;
        session_survives(&replacement).await?;
        // Starting the replacement is a distinct, explicit API call. Retaining
        // an old owner must not move its listener onto the new SSH connection.
        let fresh = replacement
            .forward_local(address, "127.0.0.1".into(), echo.address.port())
            .await?;
        owner.close().await;
        let mut stream = TcpStream::connect(address).await?;
        exchange(&mut stream, 0x94).await?;
        fresh.close().await;
        stream_closed(&mut stream).await?;
        replacement.close().await?;
        echo.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_dynamic_stop_closes_socks_stream_and_preserves_ssh() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let echo = EchoServer::start().await?;
        let ssh = connect().await?;
        let owner = ssh.forward_dynamic("127.0.0.1:0".parse()?).await?;
        let address = owner.local_addr();
        let mut stream = TcpStream::connect(address).await?;
        stream.write_all(&[5, 1, 0]).await?;
        let mut greeting = [0; 2];
        stream.read_exact(&mut greeting).await?;
        assert_eq!(greeting, [5, 0]);
        let port = echo.address.port().to_be_bytes();
        stream
            .write_all(&[5, 1, 0, 1, 127, 0, 0, 1, port[0], port[1]])
            .await?;
        let mut response = [0; 10];
        stream.read_exact(&mut response).await?;
        assert_eq!(&response[..4], &[5, 0, 0, 1]);
        exchange(&mut stream, 0xb5).await?;
        owner.close().await?;
        stream_closed(&mut stream).await?;
        listener_released(address).await?;
        session_survives(&ssh).await?;
        ssh.close().await?;
        echo.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires scripts/openssh_interop.py --forwarding"]
async fn openssh_remote_stop_waits_for_owned_streams_and_preserves_sibling() -> TestResult {
    timeout(TEST_DEADLINE, async {
        let echo = EchoServer::start().await?;
        let ssh = connect().await?;
        let first = ssh
            .forward_remote("127.0.0.1".into(), 0, echo.address)
            .await?;
        let second = ssh
            .forward_remote("127.0.0.1".into(), 0, echo.address)
            .await?;
        let address = SocketAddr::from(([127, 0, 0, 1], u16::try_from(first.remote_port())?));
        let sibling_address =
            SocketAddr::from(([127, 0, 0, 1], u16::try_from(second.remote_port())?));
        let mut stream = TcpStream::connect(address).await?;
        exchange(&mut stream, 0xd6).await?;
        let mut sibling = TcpStream::connect(sibling_address).await?;
        exchange(&mut sibling, 0xe7).await?;
        first.close().await?;
        stream_closed(&mut stream).await?;
        listener_released(address).await?;
        exchange(&mut sibling, 0xf8).await?;
        session_survives(&ssh).await?;
        second.close().await?;
        stream_closed(&mut sibling).await?;
        ssh.close().await?;
        echo.close().await
    })
    .await?
}
