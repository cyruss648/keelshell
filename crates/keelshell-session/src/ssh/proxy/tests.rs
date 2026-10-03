use super::*;
use std::{error::Error, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
type TestResult = std::result::Result<(), Box<dyn Error>>;

fn credentials(username: &str, password: &str) -> ProxyCredentials {
    ProxyCredentials {
        username: Zeroizing::new(username.into()),
        password: Zeroizing::new(password.into()),
    }
}
fn options(kind: ProxyKind, username: &str, password: &str) -> SshProxy {
    SshProxy {
        kind,
        host: "proxy.invalid".into(),
        port: 1080,
        credentials: Some(credentials(username, password)),
    }
}
#[test]
fn credential_limits_count_utf8_bytes_and_do_not_trim_secrets() {
    for (kind, username, password, valid) in [
        (ProxyKind::Socks5, "u".repeat(255), "p".repeat(255), true),
        (ProxyKind::Socks5, "é".repeat(128), "p".into(), false),
        (ProxyKind::Socks5, "u".into(), "é".repeat(128), false),
        (ProxyKind::Socks5, "u".into(), "".into(), false),
        (ProxyKind::Socks5, "u:v".into(), " \0 ".into(), true),
        (ProxyKind::HttpConnect, "u".into(), "".into(), true),
        (ProxyKind::HttpConnect, "u:v".into(), "p".into(), false),
        (ProxyKind::HttpConnect, "u".into(), " a:b ".into(), true),
        (ProxyKind::HttpConnect, "u".into(), "p".repeat(4096), true),
        (ProxyKind::HttpConnect, "u".into(), "p".repeat(4097), false),
        (ProxyKind::HttpConnect, "u".into(), "p\r\nx".into(), false),
        (ProxyKind::Socks5, " u".into(), "p".into(), false),
        (ProxyKind::Socks5, "u\0".into(), "p".into(), false),
    ] {
        assert_eq!(
            options(kind, &username, &password)
                .validate("target.invalid")
                .is_ok(),
            valid
        );
    }
}
#[test]
fn debug_and_validation_diagnostics_hide_credentials_and_invalid_endpoints() {
    let mut proxy = options(ProxyKind::HttpConnect, "canary-account", "canary-password");
    proxy.host = "canary-password@proxy.invalid".into();
    let diagnostic = format!("{proxy:?} {:?}", proxy.validate("target.invalid"));
    for secret in ["canary-account", "canary-password"] {
        assert!(!diagnostic.contains(secret));
    }
}
#[test]
fn proxy_host_validation_rejects_request_injection_and_scoped_or_bracketed_ips() {
    let proxy = options(ProxyKind::Socks5, "u", "p");
    for host in [
        "a\r\nInjected: b",
        "http://host",
        "u@host",
        "[::1]",
        "fe80::1%en0",
        "",
        "目标.invalid",
        "-option",
        "...",
    ] {
        assert!(proxy.validate(host).is_err());
    }
    assert!(proxy.validate(&"a".repeat(253)).is_ok());
    assert!(proxy.validate(&"a".repeat(254)).is_err());
    for host in [
        "::1",
        "127.0.0.1",
        "xn--fsqu00a.invalid",
        "remote-only.invalid",
    ] {
        assert!(proxy.validate(host).is_ok());
    }
}

async fn http_response(response: Vec<u8>, fragment: usize, authenticated: bool) -> Result<Vec<u8>> {
    let (mut client, mut peer) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
        let mut request = Vec::new();
        loop {
            request.push(peer.read_u8().await?);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        for chunk in response.chunks(fragment) {
            peer.write_all(chunk).await?;
            tokio::task::yield_now().await;
        }
        std::io::Result::Ok(())
    });
    let credentials = authenticated.then(|| credentials("u", "p"));
    let result = http::connect(&mut client, credentials.as_ref(), "::1", 22).await;
    drop(client);
    let _ = server.await;
    result
}
#[tokio::test]
async fn http_fragmented_interims_and_coalesced_banner_keep_exact_bytes() -> TestResult {
    bounded(async {
    let response = b"HTTP/1.1 103 Early Hints\r\nX-Info: yes\r\n\r\nHTTP/1.0 201 Tunnel\r\nContent-Length: 9999\r\nTransfer-Encoding: chunked\r\n\r\nSSH-2.0-coalesced\r\n";
    for fragment in [1, 2, 3, 7, response.len()] {
        let (mut client, mut peer) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") { request.push(peer.read_u8().await?); }
            assert_eq!(request, b"CONNECT [::1]:22 HTTP/1.1\r\nHost: [::1]:22\r\n\r\n");
            for chunk in response.chunks(fragment) { peer.write_all(chunk).await?; tokio::task::yield_now().await; }
            std::io::Result::Ok(())
        });
        let prefix = http::connect(&mut client, None, "::1", 22).await?;
        let mut tunnel = PrefixedStream::new(client, prefix);
        let mut banner = Vec::new();
        tunnel.read_to_end(&mut banner).await?;
        assert_eq!(banner, b"SSH-2.0-coalesced\r\n");
        task.await??;
    }
    Ok(())
    }).await
}
#[tokio::test]
async fn http_rejects_bounded_malformed_and_secret_echo_responses() -> TestResult {
    bounded(async {
        for response in [
            b"HTTP/1.1 200 OK\r\n folded: value\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nBad\r\n\r\n".to_vec(),
            b"HTTP/1.1 200\r\n\r\n".to_vec(),
            b"HTTP/2 200 OK\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nX: bad\0value\r\n\r\n".to_vec(),
            b"HTTP/1.1 407 canary-password\r\nProxy-Authenticate: canary-password\r\n\r\n".to_vec(),
            b"HTTP/1.1 101 Switching Protocols\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nX: "
                .iter()
                .copied()
                .chain(std::iter::repeat_n(b'x', 17000))
                .collect(),
            format!("HTTP/1.1 200 OK\r\n{}\r\n", "X: y\r\n".repeat(101)).into_bytes(),
            format!(
                "{}HTTP/1.1 200 OK\r\n\r\n",
                "HTTP/1.1 100 Continue\r\n\r\n".repeat(5)
            )
            .into_bytes(),
        ] {
            let result =
                tokio::time::timeout(Duration::from_secs(2), http_response(response, 8192, true))
                    .await?;
            assert!(matches!(&result, Err(SessionError::Proxy(_))));
            assert!(!format!("{result:?}").contains("canary-password"));
            assert!(result.is_err_and(|error| !error.is_retryable()));
        }
        assert!(matches!(
            http_response(b"HTTP/1.1 407 Required\r\n\r\n".to_vec(), 30, false).await,
            Err(SessionError::Proxy(ProxyError::AuthenticationRequired))
        ));
        assert!(matches!(
            http_response(b"HTTP/1.1 407 Required\r\n\r\n".to_vec(), 30, true).await,
            Err(SessionError::Proxy(ProxyError::AuthenticationRejected))
        ));
        Ok(())
    })
    .await
}
#[tokio::test]
async fn socks_auth_method_cannot_downgrade_and_replies_are_validated() -> TestResult {
    bounded(async {
        for (configured, response) in [
            (true, [5, 0]),
            (false, [5, 2]),
            (true, [5, 255]),
            (false, [4, 0]),
        ] {
            let (mut client, mut peer) = tokio::io::duplex(1024);
            let task = tokio::spawn(async move {
                let mut greeting = [0; 3];
                peer.read_exact(&mut greeting).await?;
                assert_eq!(greeting, [5, 1, if configured { 2 } else { 0 }]);
                peer.write_all(&response).await?;
                let mut next = [0];
                assert_eq!(peer.read(&mut next).await?, 0);
                std::io::Result::Ok(())
            });
            let credentials = configured.then(|| credentials("u", "p"));
            let result =
                socks::connect(&mut client, credentials.as_ref(), "target.invalid", 22).await;
            assert!(matches!(result, Err(SessionError::Proxy(_))));
            drop(client);
            task.await??;
        }
        for reply in [
            vec![4, 0, 0, 1],
            vec![5, 0, 1, 1],
            vec![5, 0, 0, 2],
            vec![5, 0, 0, 3, 0],
            vec![5, 5, 0, 1],
        ] {
            let (mut client, mut peer) = tokio::io::duplex(1024);
            let task = tokio::spawn(async move {
                let mut greeting = [0; 3];
                peer.read_exact(&mut greeting).await?;
                peer.write_all(&[5, 0]).await?;
                let mut request = [0; 10];
                peer.read_exact(&mut request).await?;
                peer.write_all(&reply).await?;
                std::io::Result::Ok(())
            });
            assert!(matches!(
                socks::connect(&mut client, None, "127.0.0.1", 22).await,
                Err(SessionError::Proxy(_))
            ));
            task.await??;
        }
        Ok(())
    })
    .await
}

async fn bounded(test: impl std::future::Future<Output = TestResult>) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), test).await?
}

#[tokio::test]
async fn socks_consumes_each_complete_bound_address_and_preserves_banner() -> TestResult {
    bounded(async {
        for bound in [
            vec![1, 127, 0, 0, 1],
            vec![4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![3, 3, b'd', b'n', b's'],
        ] {
            let (mut client, mut peer) = tokio::io::duplex(1024);
            let task = tokio::spawn(async move {
                let mut greeting = [0; 3];
                peer.read_exact(&mut greeting).await?;
                peer.write_all(&[5, 0]).await?;
                let mut request = [0; 22];
                peer.read_exact(&mut request).await?;
                assert_eq!(&request[..4], &[5, 1, 0, 4]);
                assert_eq!(&request[20..], &22_u16.to_be_bytes());
                let mut reply = vec![5, 0, 0];
                reply.extend_from_slice(&bound);
                reply.extend_from_slice(&[1, 2]);
                reply.extend_from_slice(b"SSH-2.0-bound\r\n");
                for byte in reply {
                    peer.write_all(&[byte]).await?;
                    tokio::task::yield_now().await;
                }
                std::io::Result::Ok(())
            });
            socks::connect(&mut client, None, "::1", 22).await?;
            let mut banner = Vec::new();
            client.read_to_end(&mut banner).await?;
            assert_eq!(banner, b"SSH-2.0-bound\r\n");
            task.await??;
        }
        Ok(())
    })
    .await
}
#[tokio::test]
async fn socks_username_password_authentication_rejection_and_version_are_explicit() -> TestResult {
    bounded(async {
        for response in [[1, 1], [5, 0]] {
            let (mut client, mut peer) = tokio::io::duplex(1024);
            let task = tokio::spawn(async move {
                let mut greeting = [0; 3];
                peer.read_exact(&mut greeting).await?;
                peer.write_all(&[5, 2]).await?;
                let mut auth = [0; 5];
                peer.read_exact(&mut auth).await?;
                assert_eq!(auth, [1, 1, b'u', 1, b'p']);
                peer.write_all(&response).await?;
                std::io::Result::Ok(())
            });
            let credentials = credentials("u", "p");
            let result =
                socks::connect(&mut client, Some(&credentials), "target.invalid", 22).await;
            assert!(matches!(result, Err(SessionError::Proxy(_))));
            task.await??;
        }
        Ok(())
    })
    .await
}
#[tokio::test]
async fn http_basic_preserves_empty_password_and_utf8_colons_and_spaces() -> TestResult {
    bounded(async {
        for password in ["", " 密码: spaced "] {
            let (mut client, mut peer) = tokio::io::duplex(8192);
            let task = tokio::spawn(async move {
                use base64::Engine;
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(peer.read_u8().await?);
                }
                let request = String::from_utf8(request).map_err(std::io::Error::other)?;
                let expected =
                    base64::engine::general_purpose::STANDARD.encode(format!("用户名:{password}"));
                assert!(request.contains(&format!("Proxy-Authorization: Basic {expected}\r\n")));
                peer.write_all(b"HTTP/1.1 204 Tunnel\r\n\r\n").await?;
                std::io::Result::Ok(())
            });
            let credentials = credentials("用户名", password);
            http::connect(&mut client, Some(&credentials), "host.invalid", 22).await?;
            task.await??;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn http_accepts_exact_header_limit_and_limits_cumulative_interim_bytes() -> TestResult {
    bounded(async {
        let prefix = "HTTP/1.1 200 Tunnel\r\nX: ";
        let response = format!(
            "{prefix}{}\r\n\r\n",
            "x".repeat(16 * 1024 - prefix.len() - 4)
        );
        assert_eq!(response.len(), 16 * 1024);
        assert!(
            http_response(response.into_bytes(), 2048, false)
                .await
                .is_ok()
        );
        let interim = format!("HTTP/1.1 100 Continue\r\nX: {}\r\n\r\n", "x".repeat(5000));
        let response = format!("{}HTTP/1.1 200 Tunnel\r\n\r\n", interim.repeat(4));
        assert!(matches!(
            http_response(response.into_bytes(), 2048, false).await,
            Err(SessionError::Proxy(ProxyError::ResponseTooLarge))
        ));
        Ok(())
    })
    .await
}
