//! Isolated, pinned-host-key SSH fixture shared by rule and workspace tests.
#![allow(clippy::expect_used)]

use super::*;
use gpui_kit::test::TestWindowExt;
use keelshell_core::{Connection, Language};
use keelshell_session::{SshAuth, SshOptions};
use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
use tokio::net::TcpListener;

pub(crate) struct SshFixture {
    pub runtime: Arc<Runtime>,
    pub session: SshSession,
    pub state: AppState,
    pub connection_id: uuid::Uuid,
    server: tokio::task::JoinHandle<()>,
}

impl SshFixture {
    pub fn new() -> Self {
        Self::with_cleanup_gate(None)
    }

    pub fn with_delayed_cleanup() -> (Self, Arc<CleanupGate>) {
        let gate = Arc::new(CleanupGate::default());
        (Self::with_cleanup_gate(Some(gate.clone())), gate)
    }

    pub fn with_rejected_cleanup() -> (Self, Arc<CleanupGate>) {
        let gate = Arc::new(CleanupGate {
            reject: true,
            ..Default::default()
        });
        (Self::with_cleanup_gate(Some(gate.clone())), gate)
    }

    fn with_cleanup_gate(cleanup: Option<Arc<CleanupGate>>) -> Self {
        let timeout = if cleanup.is_some() {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(3)
        };
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("fixture runtime"),
        );
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .expect("SSH listener");
        let address = listener.local_addr().expect("SSH address");
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x71; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(russh::server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let server = runtime.spawn(async move {
            let (socket, _) = listener.accept().await.expect("fixture accept");
            if let Some(cleanup) = cleanup {
                let running = russh::server::run_stream(
                    config,
                    socket,
                    CleanupServer {
                        cleanup,
                        listener: None,
                    },
                )
                .await
                .expect("SSH fixture");
                let _ = tokio::time::timeout(Duration::from_secs(60), running).await;
            } else {
                let running =
                    russh::server::run_stream(config, socket, super::integration_tests::Server)
                        .await
                        .expect("SSH fixture");
                let _ = tokio::time::timeout(Duration::from_secs(60), running).await;
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
                timeout,
            }))
            .expect("pinned fixture session");
        let mut connection = Connection::new("Rule fixture", address.ip().to_string(), "fixture");
        connection.port = address.port();
        let connection_id = connection.id;
        let mut state = AppState::default();
        state.connections.push(connection);
        for name in ["first proxy", "second proxy"] {
            let mut rule = TunnelRule::new(
                name,
                connection_id,
                TunnelRuleKind::Dynamic,
                "127.0.0.1",
                0,
                None,
                None,
            )
            .expect("rule");
            rule.enabled = true;
            state.add_tunnel_rule(rule).expect("add fixture rule");
        }
        Self {
            runtime,
            session,
            state,
            connection_id,
            server,
        }
    }

    pub fn mount(&self, cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<TunnelsPanel>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(Language::En, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(1400.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        TunnelsPanel::new_with_state(
                            self.session.clone(),
                            "Rule fixture".into(),
                            self.runtime.clone(),
                            Some(self.state.clone()),
                            Some(self.connection_id),
                            window,
                            cx,
                        )
                    })
                },
            )
            .expect("mount panel")
        })
    }
}

#[derive(Default)]
pub(crate) struct CleanupGate {
    pub requested: AtomicBool,
    pub address: std::sync::Mutex<Option<SocketAddr>>,
    pub release: tokio::sync::Notify,
    pub listener_dropped: AtomicBool,
    reject: bool,
}

struct CleanupServer {
    cleanup: Arc<CleanupGate>,
    listener: Option<TcpListener>,
}

impl Drop for CleanupServer {
    fn drop(&mut self) {
        self.listener = None;
        self.cleanup.listener_dropped.store(true, Ordering::Release);
    }
}

impl russh::server::Handler for CleanupServer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        russh::server::Handler::auth_password(&mut super::integration_tests::Server, user, password)
            .await
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        _: &mut russh::server::Session,
    ) -> Result<bool, Self::Error> {
        let listener = TcpListener::bind((address, *port as u16))
            .await
            .expect("owned remote listener");
        let address = listener.local_addr().expect("remote listener address");
        *port = u32::from(address.port());
        *self.cleanup.address.lock().expect("listener address") = Some(address);
        self.listener = Some(listener);
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        _: &str,
        _: u32,
        _: &mut russh::server::Session,
    ) -> Result<bool, Self::Error> {
        self.cleanup.requested.store(true, Ordering::Release);
        if self.cleanup.reject {
            // The real listener remains owned by this SSH server even though
            // the client's forwarding worker has received its terminal error.
            return Ok(false);
        }
        // A lost test assertion cannot leave the fixture waiting indefinitely.
        tokio::time::timeout(Duration::from_secs(8), self.cleanup.release.notified())
            .await
            .expect("test must release remote cleanup");
        self.listener = None;
        Ok(true)
    }
}

pub(crate) fn rule_admission(panel: &TunnelsPanel) -> (bool, bool) {
    (panel.rule_configuration_busy, panel.rule_store_blocked)
}

pub(crate) fn rule_listening(panel: &TunnelsPanel, id: uuid::Uuid) -> bool {
    panel
        .rows
        .iter()
        .any(|row| row.rule_id == Some(id) && row.listening && !row.finished)
}

pub(crate) fn rule_cleanup_unconfirmed(panel: &TunnelsPanel, id: uuid::Uuid) -> bool {
    panel
        .rows
        .iter()
        .any(|row| row.rule_id == Some(id) && row.unresolved_cleanup.is_some())
}

pub(crate) fn assert_rule_cleanup_failure(
    panel: &TunnelsPanel,
    id: uuid::Uuid,
    released: bool,
    cx: &App,
) {
    let row = panel
        .rows
        .iter()
        .find(|row| row.rule_id == Some(id))
        .expect("retained runtime row");
    assert!(row.failed, "a rejected cancellation must remain a failure");
    assert_eq!(row.finished, released);
    assert_eq!(row.unresolved_cleanup.is_some(), !released);
    if released {
        assert!(
            row.status.render(cx).contains(t(
                cx,
                "原 SSH 连接已关闭",
                "Original SSH connection closed",
            )),
            "ownership release must preserve the previous cleanup failure: {}",
            row.status.render(cx),
        );
    } else {
        let label = t(cx, "清理未确认", "Cleanup unconfirmed");
        assert!(row.status.render(cx).contains(label));
        assert!(panel.status.render(cx).contains(label));
    }
    if panel.suspended {
        assert!(
            panel.session.is_none(),
            "retirement must release the SSH handle"
        );
    }
}

pub(crate) fn try_reviewed_start(panel: &mut TunnelsPanel, cx: &mut Context<TunnelsPanel>) {
    if let Some(plan) = &panel.rule_plan {
        panel.start_named_rule(plan.review_token(), cx);
    }
}

impl Drop for SshFixture {
    fn drop(&mut self) {
        self.runtime.block_on(async {
            let _ = tokio::time::timeout(Duration::from_secs(3), self.session.close()).await;
            self.server.abort();
            let _ = (&mut self.server).await;
        });
    }
}

pub(crate) fn activate_painted(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
    for event in [
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
    }
}

pub(crate) fn click(window: &mut Window, id: impl Into<ElementId>, cx: &mut App) {
    window.activate_window();
    window.render_frame(cx);
    window.click(id, cx);
}
