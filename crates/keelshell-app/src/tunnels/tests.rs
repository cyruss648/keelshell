//! Real panel handlers with an isolated SSH server and SOCKS TCP byte exchange.

use super::{Direction, TunnelsPanel, dynamic_bind};
use gpui_kit::{
    AppContext, Bounds, Focusable, TestAppContext, WindowBounds, WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::Language;
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

trait Checked<T> {
    fn checked(self, action: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, action: &str) -> T {
        self.unwrap_or_else(|error| panic!("{action}: {error:?}"))
    }
}

struct Server;
impl russh::server::Handler for Server {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "test-only-password" {
            russh::server::Auth::Accept
        } else {
            russh::server::Auth::reject()
        })
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<russh::server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: russh::server::ChannelOpenHandle,
        _: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        let connected = tokio::time::timeout(
            Duration::from_secs(3),
            TcpStream::connect((host, port as u16)),
        )
        .await;
        if let Ok(Ok(mut socket)) = connected {
            reply.accept().await;
            tokio::spawn(async move {
                let mut stream = channel.into_stream();
                let _ = tokio::time::timeout(
                    Duration::from_secs(15),
                    tokio::io::copy_bidirectional(&mut socket, &mut stream),
                )
                .await;
            });
        } else {
            reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
        }
        Ok(())
    }
}

#[gpui_kit::test]
async fn dynamic_panel_starts_real_proxy_copies_address_switches_language_and_stops(
    cx: &mut TestAppContext,
) {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .checked("runtime"),
    );
    let listener = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("SSH fixture listener");
    let ssh_address = listener.local_addr().checked("SSH fixture address");
    // Deterministic, public test-only host key. No user credential or host is used.
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x53; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let server = runtime.spawn(async move {
        let (socket, _) = listener.accept().await.checked("fixture accept");
        let session = russh::server::run_stream(config, socket, Server)
            .await
            .checked("SSH server");
        let _ = tokio::time::timeout(Duration::from_secs(20), session).await;
    });
    let session = runtime
        .block_on(SshSession::connect(SshOptions {
            proxy: None,
            host: ssh_address.ip().to_string(),
            port: ssh_address.port(),
            username: "fixture".into(),
            expected_host_key: Some(fingerprint),
            auth: SshAuth::Password(zeroize::Zeroizing::new("test-only-password".into())),
            timeout: Duration::from_secs(3),
        }))
        .checked("real SSH connection");
    let panel_session = session.clone();
    let panel_runtime = runtime.clone();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1200.), px(500.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    TunnelsPanel::new(
                        panel_session,
                        "SSH fixture".into(),
                        panel_runtime,
                        window,
                        cx,
                    )
                })
            },
        )
        .checked("mount tunnels")
    });
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        panel
            .read(cx)
            .target_host
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        window.click("tunnel-dynamic", cx);
        assert!(
            panel
                .read(cx)
                .bind_host
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        panel.update(cx, |panel, cx| {
            assert!(panel.direction == Direction::Dynamic);
            panel
                .target_host
                .update(cx, |input, cx| input.set_value("", window, cx));
            panel.target_port.update(cx, |input, cx| {
                input.set_value("invalid hidden value", window, cx)
            });
            panel
                .bind_host
                .update(cx, |input, cx| input.set_value("0.0.0.0", window, cx));
        });
        window.render_frame(cx);
        window.click("start-tunnel", cx);
        assert!(panel.read(cx).rows.is_empty());
        assert!(panel.read(cx).status.render(cx).contains("必须为回环 IP"));
        panel.update(cx, |panel, cx| {
            panel
                .bind_host
                .update(cx, |input, cx| input.set_value("127.0.0.1", window, cx))
        });
        window.render_frame(cx);
        window.click("start-tunnel", cx);
    })
    .checked("select dynamic mode and start using real handlers");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel
            .read(cx)
            .rows
            .first()
            .is_some_and(|row| row.proxy_address.is_some())
    })
    .await;
    let proxy_address = cx
        .update_window(window, |_, window, cx| {
            let proxy = panel.read(cx).rows[0]
                .proxy_address
                .clone()
                .unwrap_or_else(|| panic!("confirmed proxy address"));
            assert!(proxy.starts_with("socks5h://127.0.0.1:"));
            assert!(!proxy.ends_with(":0"));
            assert!(
                panel.read(cx).rows[0]
                    .status
                    .render(cx)
                    .starts_with("正在监听")
            );
            assert_eq!(panel.read(cx).status.render(cx).as_str(), "正在监听 1 条");
            window.render_frame(cx);
            window.click(("copy-proxy", 0_usize), cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(proxy.clone())
            );
            crate::i18n::set_language(Language::En, cx);
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            assert!(panel.read(cx).direction == Direction::Dynamic);
            assert_eq!(
                panel.read(cx).status.render(cx).as_str(),
                "Active listeners: 1"
            );
            assert!(
                panel.read(cx).rows[0]
                    .status
                    .render(cx)
                    .starts_with("Listening at")
            );
            assert_eq!(
                panel.read(cx).bind_host.read(cx).value().as_str(),
                "127.0.0.1"
            );
            proxy
                .trim_start_matches("socks5h://")
                .parse::<std::net::SocketAddr>()
                .checked("real proxy address")
        })
        .checked("copy actual URI and change language without restarting listener");
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let echo = TcpListener::bind("127.0.0.1:0").await?;
            let port = echo.local_addr()?.port();
            let peer = tokio::spawn(async move {
                let (mut socket, _) = echo.accept().await?;
                let mut bytes = [0; 8];
                socket.read_exact(&mut bytes).await?;
                socket.write_all(&bytes).await?;
                Ok::<_, std::io::Error>(bytes)
            });
            let mut client = TcpStream::connect(proxy_address).await?;
            client.write_all(&[5, 1, 0]).await?;
            let mut method = [0; 2];
            client.read_exact(&mut method).await?;
            assert_eq!(method, [5, 0]);
            let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
            request.extend_from_slice(&port.to_be_bytes());
            client.write_all(&request).await?;
            let mut response = [0; 10];
            client.read_exact(&mut response).await?;
            assert_eq!(response[1], 0);
            client.write_all(b"ui-proxy").await?;
            let mut echoed = [0; 8];
            client.read_exact(&mut echoed).await?;
            assert_eq!(&echoed, b"ui-proxy");
            assert_eq!(peer.await??, echoed);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
        .checked("SOCKS UI byte deadline")
        .checked("real SOCKS exchange")
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("start-tunnel", cx);
        assert_eq!(
            panel.read(cx).status.render(cx).as_str(),
            "Active listeners: 1 · Starting: 1"
        );
    })
    .checked("start a second listener without losing the active count");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.get(1).is_some_and(|row| row.listening)
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            panel.read(cx).status.render(cx).as_str(),
            "Active listeners: 2"
        );
        window.render_frame(cx);
        window.click(("stop-tunnel", 0_usize), cx);
        assert_eq!(
            panel.read(cx).status.render(cx).as_str(),
            "Active listeners: 1 · Stopping: 1"
        );
        assert!(
            panel.read(cx).rows[0]
                .status
                .render(cx)
                .starts_with("Stopping")
        );
    })
    .checked("stop via panel action");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].finished
    })
    .await;
    cx.update(|cx| {
        assert!(!panel.read(cx).rows[0].failed);
        assert_eq!(
            panel.read(cx).status.render(cx).as_str(),
            "Active listeners: 1"
        );
        assert!(
            panel.read(cx).rows[0]
                .status
                .render(cx)
                .starts_with("Stopped")
        );
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("stop-tunnel", 1_usize), cx);
        assert_eq!(panel.read(cx).status.render(cx).as_str(), "Stopping: 1");
    })
    .checked("stop the remaining listener");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[1].finished
    })
    .await;
    cx.update(|cx| {
        assert!(
            panel
                .read(cx)
                .status
                .render(cx)
                .starts_with("All 2 tunnels stopped")
        );
        crate::i18n::set_language(Language::ZhCn, cx);
        assert!(
            panel
                .read(cx)
                .status
                .render(cx)
                .starts_with("全部 2 条隧道已停止")
        );
        crate::i18n::set_language(Language::En, cx);
    });
    // A real bind failure must replace the startup footer without claiming success.
    let occupied = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("reserve conflicting port");
    let occupied_port = occupied.local_addr().checked("reserved address").port();
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.bind_port.update(cx, |input, cx| {
                input.set_value(occupied_port.to_string(), window, cx)
            })
        });
        window.render_frame(cx);
        window.click("start-tunnel", cx);
    })
    .checked("start a listener on an occupied port");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.get(2).is_some_and(|row| row.finished)
    })
    .await;
    cx.update(|cx| {
        assert!(panel.read(cx).rows[2].failed);
        assert!(
            panel
                .read(cx)
                .status
                .render(cx)
                .starts_with("No active tunnels; 2 stopped, 1 failed")
        );
    });
    drop(occupied);
    assert!(!session.is_closed());
    let rebound = runtime
        .block_on(TcpListener::bind(proxy_address))
        .checked("stopped listener released");
    drop(rebound);
    runtime
        .block_on(session.close())
        .checked("close fixture SSH");
    server.abort();
}

#[gpui_kit::test]
fn dynamic_bind_validation_is_loopback_only_and_bilingual(cx: &mut TestAppContext) {
    assert!(dynamic_bind("127.0.0.1", 0).is_ok());
    assert!(dynamic_bind("::1", 1080).is_ok());
    for host in ["0.0.0.0", "::", "192.0.2.1", "localhost", "proxy.invalid"] {
        let error = dynamic_bind(host, 1080)
            .err()
            .unwrap_or_else(|| panic!("exposed or nonliteral bind accepted"));
        cx.update(|cx| {
            crate::i18n::set_language(Language::ZhCn, cx);
            assert!(error.render(cx).contains("回环 IP"));
            crate::i18n::set_language(Language::En, cx);
            assert!(error.render(cx).contains("loopback IP"));
        });
    }
}

#[gpui_kit::test]
async fn suspended_tunnels_wait_for_cleanup_close_owned_streams_and_never_restart(
    cx: &mut TestAppContext,
) {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .checked("suspended tunnel runtime"),
    );
    let listener = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("suspended tunnel SSH listener");
    let address = listener.local_addr().checked("SSH address");
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x64; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let server = runtime.spawn(async move {
        if let Ok((socket, _)) = listener.accept().await
            && let Ok(running) = russh::server::run_stream(config, socket, Server).await
        {
            let _ = running.await;
        }
    });
    let session = runtime
        .block_on(SshSession::connect(SshOptions {
            proxy: None,
            host: address.ip().to_string(),
            port: address.port(),
            username: "fixture".into(),
            expected_host_key: Some(fingerprint),
            auth: SshAuth::Password(zeroize::Zeroizing::new("test-only-password".into())),
            timeout: Duration::from_secs(3),
        }))
        .checked("suspended tunnel SSH");
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1000.), px(500.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    TunnelsPanel::new(
                        session.clone(),
                        "suspend fixture".into(),
                        runtime.clone(),
                        window,
                        cx,
                    )
                })
            },
        )
        .checked("mount real suspended tunnel panel")
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("tunnel-dynamic", cx);
        window.render_frame(cx);
        window.click("start-tunnel", cx);
    })
    .checked("start real dynamic listener");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel
            .read(cx)
            .rows
            .first()
            .is_some_and(|row| row.proxy_address.is_some())
    })
    .await;
    let address = panel.read_with(cx, |panel, _| {
        panel.rows[0]
            .proxy_address
            .as_ref()
            .unwrap_or_else(|| panic!("listener URI"))
            .trim_start_matches("socks5h://")
            .parse::<std::net::SocketAddr>()
            .checked("listener address")
    });
    let mut stream = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut stream = TcpStream::connect(address).await?;
            stream.write_all(&[5, 1, 0]).await?;
            let mut method = [0; 2];
            stream.read_exact(&mut method).await?;
            assert_eq!(method, [5, 0]);
            Ok::<_, std::io::Error>(stream)
        })
        .await
        .checked("owned stream handshake deadline")
        .checked("owned SOCKS stream accepted")
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("start-tunnel", cx);
        assert_eq!(panel.read(cx).rows.len(), 2);
        panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            assert!(panel.suspended && panel.session.is_none());
            for row in &panel.rows {
                assert!(row.cancel.load(std::sync::atomic::Ordering::Acquire));
                assert!(!row.finished, "requesting stop is not a cleanup receipt");
                assert!(row.status.render(cx).contains("正在停止"));
            }
        });
    })
    .checked("suspend an active and an unacknowledged listener without inventing completion");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.iter().all(|row| row.finished)
    })
    .await;
    runtime.block_on(async {
        let mut byte = [0];
        let result = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .checked("owned stream cleanup deadline");
        assert!(
            matches!(result, Ok(0))
                || result.is_err_and(|error| matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                )),
            "owned stream survived listener retirement"
        );
    });
    let rebound = runtime
        .block_on(TcpListener::bind(address))
        .checked("cleanup receipt released actual listener port");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.start(cx);
            panel.poll(cx);
            panel.suspend(cx);
        });
        window.render_frame(cx);
        window.click("start-tunnel", cx);
        assert_eq!(panel.read(cx).rows.len(), 2);
        assert!(panel.read(cx).rows.iter().all(|row| row.finished));
        assert!(panel.read(cx).session.is_none());
    })
    .checked("archived panel cannot restart listeners through stale handlers or controls");
    assert!(!session.is_closed());
    drop(rebound);
    runtime
        .block_on(session.close())
        .checked("close tunnel test SSH");
    server.abort();
}
