//! Real TCP upstream proxies, independently authenticated target SSH and cleanup.
use super::jump_fixture;
#[path = "upstream_proxy_server.rs"]
mod proxy_fixture;

use jump_fixture::wait_until;
use keelshell_session::{
    ProxyCredentials, ProxyKind, RetryPolicy, SessionError, SshProxy, SshSession,
};
use proxy_fixture::{Mode, ProxyServer};
use std::{error::Error, sync::atomic::Ordering, time::Duration};
use zeroize::Zeroizing;
type TestResult = Result<(), Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(3);

#[tokio::test]
async fn proxy_remote_dns_ip_families_and_auth_reach_real_ssh_shell_and_sftp() -> TestResult {
    bounded(async {
    let target = super::serve().await?;
    for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
        for authenticated in [false, true] {
            for host in ["target.remote-only.invalid", "192.0.2.7", "2001:db8::7"] {
                let proxy = ProxyServer::start(kind, target.address, authenticated, Mode::Tunnel).await?;
                let mut options = super::options(&target);
                options.host = host.into();
                options.proxy = Some(proxy.options(kind, authenticated));
                let session = SshSession::connect(options).await?;
                assert_eq!(session.exec("proxied exec").await?.stdout, b"proxied exec");
                let mut shell = session.start_shell(24,80).await?;
                shell.write(b"proxied shell").await?;
                assert!(matches!(tokio::time::timeout(TIMEOUT, shell.recv()).await?, Some(keelshell_session::SessionEvent::Data(bytes)) if bytes == b"proxied shell"));
                shell.close().await?;
                let sftp = session.sftp().await?;
                sftp.write("/proxy.bin", b"proxied sftp").await?;
                assert_eq!(sftp.read("/proxy.bin", 100).await?, b"proxied sftp");
                sftp.close().await?;
                assert_eq!(*proxy.observed.requests.lock().map_err(|_| "requests poisoned")?, vec![(host.into(), target.address.port())]);
                session.close().await?;
                drop(sftp); drop(shell); drop(session);
                wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
            proxy.assert_healthy();
            }
        }
    }
    Ok(())
    }).await
}
#[tokio::test]
async fn each_jump_opens_its_proxy_endpoint_then_proxy_resolves_target_and_retains_parent()
-> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = super::serve().await?;
        for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
            let proxy = ProxyServer::start(kind, target.address, true, Mode::Tunnel).await?;
            let jump = SshSession::connect(gateway.options(TIMEOUT)).await?;
            let mut options = super::options(&target);
            options.host = "target.remote-only.invalid".into();
            let mut upstream = proxy.options(kind, true);
            upstream.host = "remote-only.invalid".into();
            options.proxy = Some(upstream);
            let child = SshSession::connect_through(&jump, options).await?;
            assert_eq!(
                gateway
                    .observed
                    .requests
                    .lock()
                    .map_err(|_| "requests poisoned")?
                    .last(),
                Some(&(
                    "remote-only.invalid".into(),
                    u32::from(proxy.address.port())
                ))
            );
            assert_eq!(child.exec("routed").await?.stdout, b"routed");
            drop(jump);
            assert_eq!(
                child.exec("parent retained").await?.stdout,
                b"parent retained"
            );
            drop(child);
            wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
            proxy.assert_healthy();
        }
        Ok(())
    })
    .await
}
#[tokio::test]
async fn proxy_rejection_never_retries_or_falls_back_and_hides_response_secrets() -> TestResult {
    bounded(async {
        let target = jump_fixture::Server::start().await?;
        for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
            let proxy = ProxyServer::start(kind, target.address, true, Mode::Reject).await?;
            let mut options = target.options(TIMEOUT);
            options.proxy = Some(proxy.options(kind, true));
            let result = SshSession::connect_with_retry(
                options,
                RetryPolicy::new(3, Duration::ZERO, Duration::ZERO),
            )
            .await;
            assert!(matches!(&result, Err(SessionError::Proxy(_))));
            if let Err(error) = result {
                assert!(!format!("{error:?} {error}").contains("canary-password"));
            }
            wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
            proxy.assert_healthy();
            assert_eq!(proxy.observed.accepted.load(Ordering::Acquire), 1);
            assert_eq!(target.observed.auth_started.load(Ordering::Acquire), 0);
        }
        Ok(())
    })
    .await
}
#[tokio::test]
async fn proxy_does_not_bypass_target_key_checks_or_close_authenticated_parent() -> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = jump_fixture::Server::start().await?;
        let parent = SshSession::connect(gateway.options(TIMEOUT)).await?;
        for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
            let proxy = ProxyServer::start(kind, target.address, false, Mode::Tunnel).await?;
            for pin in [None, Some(gateway.fingerprint.clone())] {
                let mut options = target.options(TIMEOUT);
                options.proxy = Some(proxy.options(kind, false));
                options.expected_host_key = pin.clone();
                let result = SshSession::connect_through(&parent, options).await;
                assert!(match pin {
                    None => matches!(result, Err(SessionError::UnknownHostKey { .. })),
                    Some(_) => matches!(result, Err(SessionError::ChangedHostKey { .. })),
                });
            }
            wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 2).await?;
            proxy.assert_healthy();
            assert_eq!(target.observed.auth_started.load(Ordering::Acquire), 0);
            assert_eq!(
                parent.exec("parent survives").await?.stdout,
                b"parent survives"
            );
        }
        parent.close().await?;
        Ok(())
    })
    .await
}
#[tokio::test]
async fn cancelling_each_proxy_phase_closes_owned_transport_and_preserves_jump() -> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = super::serve().await?;
        let parent = SshSession::connect(gateway.options(TIMEOUT)).await?;
        for through_jump in [false, true] {
            for (kind, stage) in [
                (ProxyKind::Socks5, 1),
                (ProxyKind::Socks5, 2),
                (ProxyKind::Socks5, 3),
                (ProxyKind::HttpConnect, 3),
            ] {
                let proxy =
                    ProxyServer::start(kind, target.address, true, Mode::Stall(stage)).await?;
                let mut options = super::options(&target);
                options.proxy = Some(proxy.options(kind, true));
                let parent = parent.clone();
                let task = tokio::spawn(async move {
                    if through_jump {
                        SshSession::connect_through(&parent, options).await
                    } else {
                        SshSession::connect(options).await
                    }
                });
                wait_until(|| proxy.observed.stage.load(Ordering::Acquire) == usize::from(stage))
                    .await?;
                task.abort();
                let _ = task.await;
                wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
                proxy.assert_healthy();
                if through_jump {
                    let opened = gateway.observed.opens();
                    let id = *opened.last().ok_or("missing owned channel")?;
                    wait_until(|| gateway.observed.was_closed(id)).await?;
                }
            }
            assert_eq!(
                parent.exec("unrelated parent survives").await?.stdout,
                b"unrelated parent survives"
            );
        }
        parent.close().await?;
        Ok(())
    })
    .await
}
#[tokio::test]
async fn proxy_and_ssh_authentication_share_one_deadline_direct_and_through_jump() -> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = jump_fixture::Server::start().await?;
        target.observed.auth_delay.store(350, Ordering::Release);
        let parent = SshSession::connect(gateway.options(TIMEOUT)).await?;
        for through_jump in [false, true] {
            let proxy = ProxyServer::start(
                ProxyKind::HttpConnect,
                target.address,
                false,
                Mode::Delay(Duration::from_millis(350)),
            )
            .await?;
            let mut options = target.options(Duration::from_millis(550));
            options.proxy = Some(proxy.options(ProxyKind::HttpConnect, false));
            let started = std::time::Instant::now();
            let result = if through_jump {
                SshSession::connect_through(&parent, options).await
            } else {
                SshSession::connect(options).await
            };
            assert!(matches!(result, Err(SessionError::Timeout(_))));
            assert!(started.elapsed() < Duration::from_secs(2));
            wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
            proxy.assert_healthy();
        }
        assert_eq!(parent.exec("alive").await?.stdout, b"alive");
        parent.close().await?;
        Ok(())
    })
    .await
}
#[tokio::test]
async fn invalid_credentials_fail_before_opening_any_tcp_or_jump_channel() -> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = super::serve().await?;
        let parent = SshSession::connect(gateway.options(TIMEOUT)).await?;
        let proxy =
            ProxyServer::start(ProxyKind::Socks5, target.address, true, Mode::Tunnel).await?;
        for through_jump in [false, true] {
            let mut options = super::options(&target);
            let mut upstream = proxy.options(ProxyKind::Socks5, true);
            upstream.credentials = Some(ProxyCredentials {
                username: Zeroizing::new("user".into()),
                password: Zeroizing::new("é".repeat(128)),
            });
            options.proxy = Some(upstream);
            let result = if through_jump {
                SshSession::connect_through(&parent, options).await
            } else {
                SshSession::connect(options).await
            };
            assert!(matches!(result, Err(SessionError::Invalid(_))));
        }
        assert_eq!(proxy.observed.accepted.load(Ordering::Acquire), 0);
        assert!(
            gateway
                .observed
                .requests
                .lock()
                .map_err(|_| "requests poisoned")?
                .is_empty()
        );
        parent.close().await?;
        Ok(())
    })
    .await
}

async fn bounded(test: impl std::future::Future<Output = TestResult>) -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), test).await?
}

#[tokio::test]
async fn cancelled_proxy_target_authentication_and_late_jump_open_keep_parent_usable() -> TestResult
{
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = jump_fixture::Server::start().await?;
        target.observed.auth_delay.store(2000, Ordering::Release);
        let parent = SshSession::connect(gateway.options(TIMEOUT)).await?;
        for through_jump in [false, true] {
            let before = target.observed.auth_started.load(Ordering::Acquire);
            let proxy =
                ProxyServer::start(ProxyKind::HttpConnect, target.address, true, Mode::Tunnel)
                    .await?;
            let mut options = target.options(TIMEOUT);
            options.proxy = Some(proxy.options(ProxyKind::HttpConnect, true));
            let owned_parent = parent.clone();
            let task = tokio::spawn(async move {
                if through_jump {
                    SshSession::connect_through(&owned_parent, options).await
                } else {
                    SshSession::connect(options).await
                }
            });
            wait_until(|| target.observed.auth_started.load(Ordering::Acquire) > before).await?;
            task.abort();
            let _ = task.await;
            wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
            proxy.assert_healthy();
        }
        let proxy =
            ProxyServer::start(ProxyKind::Socks5, target.address, true, Mode::Tunnel).await?;
        gateway
            .observed
            .next_open_delay
            .store(200, Ordering::Release);
        let mut options = target.options(TIMEOUT);
        options.proxy = Some(proxy.options(ProxyKind::Socks5, true));
        let before = gateway.observed.opens().len();
        let owned_parent = parent.clone();
        let task =
            tokio::spawn(async move { SshSession::connect_through(&owned_parent, options).await });
        wait_until(|| gateway.observed.opens().len() > before).await?;
        let id = gateway.observed.opens()[before];
        task.abort();
        let _ = task.await;
        wait_until(|| gateway.observed.was_closed(id)).await?;
        wait_until(|| proxy.observed.closed.load(Ordering::Acquire) == 1).await?;
        proxy.assert_healthy();
        assert_eq!(
            parent.exec("parent still available").await?.stdout,
            b"parent still available"
        );
        parent.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn every_hop_can_use_its_own_distinct_proxy_in_one_owned_route() -> TestResult {
    bounded(async {
        let gateway = jump_fixture::Server::start().await?;
        let target = super::serve().await?;
        let first_proxy =
            ProxyServer::start(ProxyKind::HttpConnect, gateway.address, true, Mode::Tunnel).await?;
        let second_proxy =
            ProxyServer::start(ProxyKind::Socks5, target.address, true, Mode::Tunnel).await?;
        let mut first_options = gateway.options(TIMEOUT);
        first_options.host = "gateway.remote-only.invalid".into();
        first_options.proxy = Some(first_proxy.options(ProxyKind::HttpConnect, true));
        let parent = SshSession::connect(first_options).await?;
        let mut second_options = super::options(&target);
        second_options.host = "target.remote-only.invalid".into();
        let mut upstream = second_proxy.options(ProxyKind::Socks5, true);
        upstream.host = "remote-only.invalid".into();
        second_options.proxy = Some(upstream);
        let child = SshSession::connect_through(&parent, second_options).await?;
        assert_eq!(
            first_proxy
                .observed
                .requests
                .lock()
                .map_err(|_| "requests poisoned")?
                .as_slice(),
            &[("gateway.remote-only.invalid".into(), gateway.address.port())]
        );
        assert_eq!(
            gateway
                .observed
                .requests
                .lock()
                .map_err(|_| "requests poisoned")?
                .as_slice(),
            &[(
                "remote-only.invalid".into(),
                u32::from(second_proxy.address.port())
            )]
        );
        assert_eq!(
            second_proxy
                .observed
                .requests
                .lock()
                .map_err(|_| "requests poisoned")?
                .as_slice(),
            &[("target.remote-only.invalid".into(), target.address.port())]
        );
        drop(parent);
        assert_eq!(
            child.exec("two distinct proxies").await?.stdout,
            b"two distinct proxies"
        );
        drop(child);
        wait_until(|| {
            first_proxy.observed.closed.load(Ordering::Acquire) == 1
                && second_proxy.observed.closed.load(Ordering::Acquire) == 1
        })
        .await?;
        first_proxy.assert_healthy();
        second_proxy.assert_healthy();
        Ok(())
    })
    .await
}
