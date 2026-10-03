//! Target SSH sessions carried by a separately authenticated SSH relay.
use super::jump_fixture as fixture;

use fixture::{Server, wait_until};
use keelshell_session::{RetryPolicy, SessionError, SshAuth, SshSession};
use std::{error::Error, sync::atomic::Ordering, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use zeroize::Zeroizing;
type TestResult = Result<(), Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(3);

#[tokio::test]
async fn jump_resolves_target_remotely_and_supports_real_exec_shell_and_sftp() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = super::serve().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let mut options = super::options(&target_server);
    options.host = "remote-only.invalid".into();
    let target = SshSession::connect_through(&jump, options).await?;
    assert_eq!(target.exec("through jump").await?.stdout, b"through jump");
    assert_eq!(
        *jump_server
            .observed
            .requests
            .lock()
            .map_err(|_| "requests poisoned")?,
        vec![(
            "remote-only.invalid".into(),
            u32::from(target_server.address.port())
        )]
    );
    let mut shell = target.start_shell(24, 80).await?;
    shell.write(b"remote echo").await?;
    let event = tokio::time::timeout(TIMEOUT, shell.recv())
        .await?
        .ok_or("shell ended")?;
    assert!(
        matches!(event, keelshell_session::SessionEvent::Data(bytes) if bytes == b"remote echo")
    );
    shell.close().await?;
    let sftp = target.sftp().await?;
    let data = vec![0x5a; 256 * 1024];
    sftp.write("/jump.bin", &data).await?;
    assert_eq!(sftp.read("/jump.bin", 512 * 1024).await?, data);
    sftp.close().await?;
    target.close().await?;
    wait_until(|| {
        jump_server
            .observed
            .was_closed(jump_server.observed.opens()[0])
    })
    .await?;
    assert_eq!(
        jump.exec("parent survives").await?.stdout,
        b"parent survives"
    );
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn each_target_checks_its_own_key_before_authentication_and_rejections_keep_parent()
-> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    for expected in [None, Some(jump_server.fingerprint.clone())] {
        let mut options = target_server.options(TIMEOUT);
        options.expected_host_key = expected.clone();
        let result = SshSession::connect_through_with_retry(
            &jump,
            options,
            RetryPolicy::new(3, Duration::ZERO, Duration::ZERO),
        )
        .await;
        assert!(match expected {
            None => matches!(result, Err(SessionError::UnknownHostKey { .. })),
            Some(_) => matches!(result, Err(SessionError::ChangedHostKey { .. })),
        });
        assert_eq!(
            target_server.observed.auth_started.load(Ordering::Acquire),
            0
        );
    }
    assert_eq!(
        jump_server.observed.opens().len(),
        2,
        "identity failures must not retry"
    );
    let mut options = target_server.options(TIMEOUT);
    options.auth = SshAuth::Password(Zeroizing::new("wrong fixture password".into()));
    assert!(matches!(
        SshSession::connect_through(&jump, options).await,
        Err(SessionError::Authentication)
    ));
    wait_until(|| {
        jump_server
            .observed
            .opens()
            .iter()
            .all(|id| jump_server.observed.was_closed(*id))
    })
    .await?;
    assert_eq!(
        jump.exec("still authenticated").await?.stdout,
        b"still authenticated"
    );
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_options_and_explicit_direct_rejection_do_not_fall_back_or_retry() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let mut invalid = target_server.options(TIMEOUT);
    invalid.port = 0;
    assert!(matches!(
        SshSession::connect_through(&jump, invalid).await,
        Err(SessionError::Invalid(_))
    ));
    assert!(jump_server.observed.opens().is_empty());
    let too_long = target_server.options(Duration::MAX);
    assert!(matches!(
        SshSession::connect_through(&jump, too_long).await,
        Err(SessionError::Invalid(_))
    ));
    assert!(jump_server.observed.opens().is_empty());
    let mut blocked = target_server.options(TIMEOUT);
    blocked.host = "blocked.invalid".into();
    let result = SshSession::connect_through_with_retry(
        &jump,
        blocked,
        RetryPolicy::new(4, Duration::ZERO, Duration::ZERO),
    )
    .await;
    assert!(matches!(
        result,
        Err(SessionError::Ssh(russh::Error::ChannelOpenFailure(_)))
    ));
    assert_eq!(jump_server.observed.opens().len(), 1);
    assert_eq!(
        target_server.observed.auth_started.load(Ordering::Acquire),
        0
    );
    assert!(!jump.is_closed());
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn cancelled_direct_open_closes_late_confirmation_without_harming_parent_or_sibling()
-> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let sibling = SshSession::connect_through(&jump, target_server.options(TIMEOUT)).await?;
    jump_server
        .observed
        .next_open_delay
        .store(250, Ordering::Release);
    let owned_jump = jump.clone();
    let options = target_server.options(TIMEOUT);
    let task = tokio::spawn(async move { SshSession::connect_through(&owned_jump, options).await });
    wait_until(|| jump_server.observed.opens().len() == 2).await?;
    let id = jump_server.observed.opens()[1];
    task.abort();
    assert!(task.await.is_err());
    wait_until(|| jump_server.observed.was_closed(id)).await?;
    assert_eq!(
        sibling.exec("sibling lives").await?.stdout,
        b"sibling lives"
    );
    assert_eq!(jump.exec("parent lives").await?.stdout, b"parent lives");
    sibling.close().await?;
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn unconfirmed_open_cancellation_closes_only_the_dedicated_parent_connection() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let unrelated = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    jump_server
        .observed
        .next_open_delay
        .store(usize::MAX, Ordering::Release);
    let options = target_server.options(Duration::from_millis(250));
    let owned_jump = jump.clone();
    let started = tokio::time::Instant::now();
    let task = tokio::spawn(async move { SshSession::connect_through(&owned_jump, options).await });
    wait_until(|| !jump_server.observed.opens().is_empty()).await?;
    task.abort();
    assert!(task.await.is_err());
    wait_until(|| {
        jump.is_closed() && jump_server.observed.disconnected.load(Ordering::Acquire) > 0
    })
    .await?;
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(
        unrelated.exec("independent connection").await?.stdout,
        b"independent connection"
    );
    assert!(matches!(
        SshSession::connect_through(&jump, target_server.options(TIMEOUT)).await,
        Err(SessionError::Closed)
    ));
    unrelated.close().await?;
    Ok(())
}

#[tokio::test]
async fn cancelled_banner_and_key_exchange_close_target_stream_without_waiting_for_handshake()
-> TestResult {
    for send_banner in [false, true] {
        let jump_server = Server::start().await?;
        let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let mut options = jump_server.options(TIMEOUT);
        options.port = listener.local_addr()?.port();
        let owned_jump = jump.clone();
        let task =
            tokio::spawn(async move { SshSession::connect_through(&owned_jump, options).await });
        let (mut peer, _) = tokio::time::timeout(TIMEOUT, listener.accept()).await??;
        if send_banner {
            peer.write_all(b"SSH-2.0-jump-fixture\r\n").await?;
        }
        let mut bytes = [0; 4096];
        let mut received = Vec::new();
        tokio::time::timeout(TIMEOUT, async {
            loop {
                let count = peer.read(&mut bytes).await?;
                if count == 0 {
                    return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
                }
                received.extend_from_slice(&bytes[..count]);
                if let Some(newline) = received.iter().position(|byte| *byte == b'\n')
                    && (!send_banner || received.len() >= newline + 5)
                {
                    break;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await??;
        task.abort();
        assert!(task.await.is_err());
        wait_until(|| {
            jump_server
                .observed
                .was_closed(jump_server.observed.opens()[0])
        })
        .await?;
        tokio::time::timeout(TIMEOUT, async {
            loop {
                if peer.read(&mut bytes).await? == 0 {
                    break;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await??;
        assert_eq!(
            jump.exec("after handshake cancellation").await?.stdout,
            b"after handshake cancellation"
        );
        jump.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn cancelling_target_authentication_releases_only_its_channel() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    target_server
        .observed
        .auth_delay
        .store(800, Ordering::Release);
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let owned_jump = jump.clone();
    let options = target_server.options(TIMEOUT);
    let task = tokio::spawn(async move { SshSession::connect_through(&owned_jump, options).await });
    wait_until(|| target_server.observed.auth_started.load(Ordering::Acquire) == 1).await?;
    task.abort();
    assert!(task.await.is_err());
    wait_until(|| {
        jump_server
            .observed
            .was_closed(jump_server.observed.opens()[0])
    })
    .await?;
    assert_eq!(
        jump.exec("after auth cancellation").await?.stdout,
        b"after auth cancellation"
    );
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn target_retains_parent_and_multihop_chain_until_final_owner_drops() -> TestResult {
    let first_server = Server::start().await?;
    let second_server = Server::start().await?;
    let third_server = Server::start().await?;
    let first = SshSession::connect(first_server.options(TIMEOUT)).await?;
    let second = SshSession::connect_through(&first, second_server.options(TIMEOUT)).await?;
    let third = SshSession::connect_through(&second, third_server.options(TIMEOUT)).await?;
    drop(first);
    drop(second);
    let clone = third.clone();
    drop(third);
    assert_eq!(clone.exec("third hop").await?.stdout, b"third hop");
    drop(clone);
    wait_until(|| {
        third_server.observed.disconnected.load(Ordering::Acquire) > 0
            && second_server.observed.disconnected.load(Ordering::Acquire) > 0
            && first_server.observed.disconnected.load(Ordering::Acquire) > 0
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn channel_open_and_target_handshake_share_one_deadline() -> TestResult {
    let jump_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let mut options = jump_server.options(Duration::from_millis(500));
    options.port = listener.local_addr()?.port();
    jump_server
        .observed
        .next_open_delay
        .store(300, Ordering::Release);
    let start = tokio::time::Instant::now();
    let result = SshSession::connect_through(&jump, options).await;
    assert!(matches!(result, Err(SessionError::Timeout(_))));
    assert!(
        start.elapsed() < Duration::from_millis(750),
        "handshake received a fresh timeout after opening"
    );
    wait_until(|| {
        jump_server
            .observed
            .was_closed(jump_server.observed.opens()[0])
    })
    .await?;
    assert!(!jump.is_closed());
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn target_exceptional_cleanup_stops_its_relay_and_preserves_parent_sibling() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    let sibling = SshSession::connect_through(&jump, target_server.options(TIMEOUT)).await?;
    let target =
        SshSession::connect_through(&jump, target_server.options(Duration::from_millis(250)))
            .await?;
    let target_channel = jump_server.observed.opens()[1];
    target_server
        .observed
        .never_confirm_session
        .store(1, Ordering::Release);
    assert!(target.sftp().await.is_err());
    wait_until(|| target.is_closed() && jump_server.observed.was_closed(target_channel)).await?;
    assert_eq!(
        sibling.exec("sibling survived target abort").await?.stdout,
        b"sibling survived target abort"
    );
    assert_eq!(
        jump.exec("parent survived target abort").await?.stdout,
        b"parent survived target abort"
    );
    sibling.close().await?;
    jump.close().await?;
    Ok(())
}

#[tokio::test]
async fn transient_target_handshake_failure_retries_with_a_new_owned_channel() -> TestResult {
    let jump_server = Server::start().await?;
    let target_server = Server::start().await?;
    let jump = SshSession::connect(jump_server.options(TIMEOUT)).await?;
    target_server
        .observed
        .drop_connections
        .store(1, Ordering::Release);
    let target = SshSession::connect_through_with_retry(
        &jump,
        target_server.options(TIMEOUT),
        RetryPolicy::new(2, Duration::ZERO, Duration::ZERO),
    )
    .await?;
    assert_eq!(jump_server.observed.opens().len(), 2);
    wait_until(|| {
        jump_server
            .observed
            .was_closed(jump_server.observed.opens()[0])
    })
    .await?;
    assert_eq!(
        target_server.observed.auth_started.load(Ordering::Acquire),
        1
    );
    assert_eq!(
        target.exec("retry uses a fresh channel").await?.stdout,
        b"retry uses a fresh channel"
    );
    target.close().await?;
    jump.close().await?;
    Ok(())
}
