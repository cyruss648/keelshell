//! Headless production controls with owned SSH packets. TLS metadata here is a
//! controlled presentation fixture; actual TLS acceptance has separate tests.
use super::{Arc, AtomicBool, Duration, MonitorPanel, Ordering, ProtocolPanel, SshSession};
use base64::{Engine, engine::general_purpose::STANDARD};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::sync::atomic::AtomicUsize;
use tokio::{net::TcpListener, sync::Semaphore, task::JoinHandle};

struct Control {
    requests: AtomicUsize,
    gate: Semaphore,
    unknown: AtomicBool,
}
struct Peer(Arc<Control>);
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: russh::ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        let command = String::from_utf8_lossy(data);
        let (body, status) = if command.contains("KEELSHELL_PROTOCOL_PROBE") {
            self.0.requests.fetch_add(1, Ordering::Release);
            let permit = self
                .0
                .gate
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
            let encoded = command
                .lines()
                .nth(1)
                .and_then(|line| line.split_whitespace().nth(5))
                .unwrap_or("")
                .trim_matches('\'');
            let input: serde_json::Value = STANDARD
                .decode(encoded)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .ok_or(russh::Error::Disconnect)?;
            let kind = input["kind"].as_str().unwrap_or("");
            let encrypted = kind == "tls" || (kind == "http" && input["https"] == true);
            let tls = if encrypted {
                serde_json::json!({"protocol":"TLSv1.3","cipher":"TLS_AES_256_GCM_SHA384","subject":"commonName=fixture.test","issuer":"commonName=owned fixture CA","not_before":"Oct  6 00:00:00 2026 GMT","not_after":"Oct  7 00:00:00 2026 GMT","names":["fixture.test","127.0.0.1"],"sha256":"ab".repeat(32)})
            } else {
                serde_json::Value::Null
            };
            let http = if kind == "http" {
                serde_json::json!({"status":403,"version":"HTTP/1.1"})
            } else {
                serde_json::Value::Null
            };
            let body = if self.0.unknown.load(Ordering::Acquire) {
                serde_json::json!({"version":1,"status":"cleanup_unknown","addresses":[],"peer":null,"tls":null,"http":null,"limited":false,"timing":{"resolve_ms":null,"connect_ms":null,"tls_ms":null,"headers_ms":null,"total_ms":8000}})
            } else {
                serde_json::json!({"version":1,"status":"success","addresses":[{"family":"IPv4","address":"127.0.0.1"}],"peer":if kind=="dns"{serde_json::Value::Null}else{serde_json::json!("127.0.0.1")},"tls":tls,"http":http,"limited":false,"timing":{"resolve_ms":1,"connect_ms":if kind=="dns"{None}else{Some(1)},"tls_ms":if encrypted{Some(1)}else{None},"headers_ms":if kind=="http"{Some(1)}else{None},"total_ms":4}})
            };
            (format!("KEELSHELL_DIAGNOSTIC_V1\n{body}"), 0)
        } else {
            (String::new(), 64)
        };
        if !body.is_empty() {
            session.data(id, body.into_bytes())?;
        }
        session.exit_status_request(id, status)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}
trait Checked<T> {
    fn checked(self, action: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, action: &str) -> T {
        self.unwrap_or_else(|e| panic!("{action}: {e:?}"))
    }
}
struct Harness {
    window: AnyWindowHandle,
    monitor: Entity<MonitorPanel>,
    panel: Entity<ProtocolPanel>,
    runtime: Arc<tokio::runtime::Runtime>,
    session: SshSession,
    control: Arc<Control>,
    server: JoinHandle<()>,
    port: u16,
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("owned protocol runtime"),
        );
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("owned protocol listener");
        let address = listener.local_addr().checked("listener address");
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x73; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let control = Arc::new(Control {
            requests: AtomicUsize::new(0),
            gate: Semaphore::new(0),
            unknown: AtomicBool::new(false),
        });
        let peer = control.clone();
        let server = runtime.spawn(async move {
            if let Ok((socket, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, socket, Peer(peer)).await
            {
                let _ = running.await;
            }
        });
        let mut options = keelshell_session::SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth =
            keelshell_session::SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(3);
        let session = runtime
            .block_on(SshSession::connect(options))
            .checked("authenticated controlled SSH");
        let captured = session.clone();
        let panel_runtime = runtime.clone();
        let (window, monitor) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(keelshell_core::Language::ZhCn, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(320.), px(580.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        MonitorPanel::new(
                            captured,
                            "fixture SSH target".into(),
                            panel_runtime,
                            window,
                            cx,
                        )
                    })
                },
            )
            .checked("production monitor window")
        });
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        Self {
            window,
            monitor,
            panel,
            runtime,
            session,
            control,
            server,
            port: address.port(),
        }
    }
    fn set_endpoint(&self, value: &str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            self.panel.update(cx, |panel, cx| {
                panel
                    .endpoint
                    .update(cx, |input, cx| input.set_value(value, window, cx))
            })
        })
        .checked("set explicit endpoint");
    }
    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            for _ in 0..100 {
                let viewport = window.find("monitor-scroll").bounds();
                let target = window.find(id).bounds();
                if target.origin.y >= viewport.origin.y && target.bottom() <= viewport.bottom() {
                    break;
                }
                window.scroll(
                    "monitor-scroll",
                    gpui_kit::ScrollDelta::Lines(point(
                        0.,
                        if target.origin.y < viewport.origin.y {
                            1.
                        } else {
                            -1.
                        },
                    )),
                    cx,
                );
                window.render_frame(cx);
            }
            window.click(id, cx);
        })
        .checked("click actual protocol control");
    }
    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(5), |_, cx| {
            self.panel.read(cx).cancel.is_none()
        })
        .await;
    }
    fn finish(mut self) {
        self.control.gate.add_permits(8);
        self.runtime.block_on(async{self.session.close().await.checked("close own SSH");self.server.abort();let joined=tokio::time::timeout(Duration::from_secs(2),&mut self.server).await.checked("join own listener task");assert!(joined.is_ok()||joined.as_ref().is_err_and(|e|e.is_cancelled()));assert!(matches!(tokio::net::TcpStream::connect(("127.0.0.1",self.port)).await,Err(e)if e.kind()==std::io::ErrorKind::ConnectionRefused));});
    }
}

#[gpui_kit::test]
async fn protocol_review_rechecks_input_and_theme_locale_never_dispatch(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.click("protocol-expand", cx);
    h.set_endpoint("fixture.test", cx);
    h.click("protocol-preview", cx);
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    assert!(h.panel.read_with(cx, |p, _| p.review.is_some()));
    h.set_endpoint("changed.test", cx);
    h.click("protocol-confirm", cx);
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    assert!(h.panel.read_with(cx, |p, _| p.review.is_none()));
    h.click("protocol-mode-tls", cx);
    h.set_endpoint("fixture.test", cx);
    h.click("protocol-preview", cx);
    h.control.gate.add_permits(1);
    h.click("protocol-confirm", cx);
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |p, _| {
        p.result.as_ref().is_some_and(|r| r.tls.is_some())
    }));
    // Input entities must belong to one window at a time. Keeping this panel
    // mounted twice makes their shared layout state oscillate between widths.
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire original monitor window before workspace mounting");
    crate::workspace::tests::protocol_diagnostics::exercise_layout(h.monitor.clone(), cx);
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    h.finish();
}

#[gpui_kit::test]
async fn protocol_actual_cancel_and_suspend_cannot_adopt_late_results(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.click("protocol-expand", cx);
    h.set_endpoint("fixture.test", cx);
    h.click("protocol-preview", cx);
    h.click("protocol-confirm", cx);
    cx.wait_for(h.window, Duration::from_secs(3), |_, _| {
        h.control.requests.load(Ordering::Acquire) == 1
    })
    .await;
    h.click("protocol-cancel", cx);
    h.control.gate.add_permits(1);
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |p, _| p.result.is_none()));
    h.click("protocol-preview", cx);
    h.click("protocol-confirm", cx);
    cx.wait_for(h.window, Duration::from_secs(3), |_, _| {
        h.control.requests.load(Ordering::Acquire) == 2
    })
    .await;
    h.monitor.update(cx, |p, cx| p.suspend(cx));
    h.control.gate.add_permits(1);
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |p, _| p.session.is_none()
        && p.review.is_none()
        && p.result.is_none()));
    h.panel.update(cx, |p, cx| p.confirm(cx));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 2);
    h.finish();
}

#[gpui_kit::test]
async fn protocol_captured_connection_and_saved_route_changes_revoke_immediately(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let replacement = Harness::new(cx);
    crate::workspace::tests::protocol_diagnostics::exercise_authority(
        h.monitor.clone(),
        h.session.clone(),
        replacement.session.clone(),
        cx,
    );
    assert!(h.panel.read_with(cx, |p, _| p.session.is_none()
        && p.review.is_none()
        && p.cancel.is_none()));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    assert_eq!(replacement.control.requests.load(Ordering::Acquire), 0);
    h.finish();
    replacement.finish();
}

#[gpui_kit::test]
async fn protocol_unknown_worker_cleanup_disables_further_dispatch(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.click("protocol-expand", cx);
    h.set_endpoint("fixture.test", cx);
    h.click("protocol-preview", cx);
    h.control.unknown.store(true, Ordering::Release);
    h.control.gate.add_permits(1);
    h.click("protocol-confirm", cx);
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |panel, _| panel.session.is_none()
        && panel.result.as_ref().is_some_and(|r| {
            r.status
                == keelshell_session::network_diagnostic::NetworkDiagnosticStatus::CleanupUnknown
        })));
    h.panel.update(cx, |panel, cx| {
        panel.preview(cx);
        panel.confirm(cx);
        assert!(panel.review.is_none());
    });
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    h.finish();
}

#[gpui_kit::test]
async fn protocol_reconnect_installed_panel_refuses_stale_route_before_preview(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let (window, monitor) = crate::workspace::tests::protocol_diagnostics::install_reconnected_monitor_with_silent_route_change(
        h.session.clone(), h.port, cx,
    );
    let panel = monitor.read_with(cx, |monitor, _| monitor.protocol.clone());
    let refused = cx
        .update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel
                    .endpoint
                    .update(cx, |input, cx| input.set_value("fixture.test", window, cx));
                panel.preview(cx);
                panel.review.is_none() && panel.session.is_none()
            })
        })
        .checked("stale-route preview on newly reconnected production panel");
    let requests = h.control.requests.load(Ordering::Acquire);
    h.finish();
    assert!(
        refused,
        "reconnected panel must bind route authority before preview"
    );
    assert_eq!(requests, 0);
}
