//! Real SSH process responses arriving after the panel has retired its authority.
use super::{Job, MonitorPanel};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::Language;
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinHandle};

trait Checked<T> {
    fn checked(self, action: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, action: &str) -> T {
        self.unwrap_or_else(|error| panic!("{action}: {error:?}"))
    }
}
struct Control {
    commands: AtomicUsize,
    inspecting: AtomicUsize,
    terminating: AtomicUsize,
    inspect: Semaphore,
    terminate: Semaphore,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            commands: AtomicUsize::new(0),
            inspecting: AtomicUsize::new(0),
            terminating: AtomicUsize::new(0),
            inspect: Semaphore::new(0),
            terminate: Semaphore::new(0),
        }
    }
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
        self.0.commands.fetch_add(1, Ordering::Release);
        session.channel_success(id)?;
        let command = String::from_utf8_lossy(data);
        let (body, status) = if command.contains("@@KS:process@@") {
            self.0.inspecting.fetch_add(1, Ordering::Release);
            let permit = self
                .0
                .inspect
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
            (
                format!(
                    "42 (fixture worker) S {} 12345 0 0\n@@KS:process@@\n42 tester 2.5 1.0 fixture-worker\n",
                    std::iter::repeat_n("0", 18).collect::<Vec<_>>().join(" ")
                ),
                0,
            )
        } else if command.starts_with("pid=42; expected=12345\n") {
            self.0.terminating.fetch_add(1, Ordering::Release);
            let permit = self
                .0
                .terminate
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
            (String::new(), 0)
        } else if command.contains("ps -ww -eo") {
            ("42 tester 2.5 1.0 fixture-worker\n".into(), 0)
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
struct Harness {
    window: AnyWindowHandle,
    panel: Entity<MonitorPanel>,
    control: Arc<Control>,
    session: SshSession,
    _runtime: Arc<tokio::runtime::Runtime>,
    server: JoinHandle<()>,
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
                .checked("monitor runtime"),
        );
        let control = Arc::new(Control::default());
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("monitor fixture listener");
        let address = listener.local_addr().checked("monitor address");
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x62; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let peer = control.clone();
        let server = runtime.spawn(async move {
            if let Ok((socket, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, socket, Peer(peer)).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(5);
        let session = runtime
            .block_on(SshSession::connect(options))
            .checked("monitor real SSH");
        let panel_session = session.clone();
        let panel_runtime = runtime.clone();
        let (window, panel) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(Language::ZhCn, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(480.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        MonitorPanel::new(
                            panel_session,
                            "fixture host".into(),
                            panel_runtime,
                            window,
                            cx,
                        )
                    })
                },
            )
            .checked("mount production monitor")
        });
        Self {
            window,
            panel,
            control,
            session,
            _runtime: runtime,
            server,
        }
    }
    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(8), |_, cx| {
            !self.panel.read(cx).busy
        })
        .await;
    }
    fn inspect(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(("review-process", 42_usize), cx);
        })
        .checked("review process through production action");
    }
}

#[gpui_kit::test]
async fn suspended_monitor_discards_a_late_successful_inspection_and_cannot_signal(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.inspect(cx);
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.control.inspecting.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            assert!(panel.busy && panel.pending.is_none());
            assert!(panel.monitor.is_none() && panel.paused);
            assert!(
                panel
                    .worker_cancel
                    .as_ref()
                    .is_some_and(|cancel| cancel.load(Ordering::Acquire))
            );
        });
    })
    .checked("suspend while real process inspection is still in flight");
    h.control.inspect.add_permits(1);
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    cx.update_window(h.window, |_, window, cx| {
        assert!(h.panel.read(cx).pending.is_none());
        assert_eq!(h.panel.read(cx).processes[0].pid, 42);
        window.render_frame(cx);
        assert!(window.try_find("process-confirmation").is_none());
        window.click(("review-process", 42_usize), cx);
        h.panel.update(cx, |panel, cx| {
            panel.run(Job::Refresh, cx);
            panel.run(Job::Sockets, cx);
        });
        assert!(!h.panel.read(cx).busy);
        assert!(h.panel.read(cx).pending.is_none());
    })
    .checked("late identity cannot restore approval and archived controls cannot issue commands");
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    assert!(!h.session.is_closed());
}

#[gpui_kit::test]
async fn suspended_monitor_retains_one_sent_sigterm_receipt_without_replaying_it(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.control.inspect.add_permits(1);
    h.inspect(cx);
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        assert!(h.panel.read(cx).pending.is_some());
        window.render_frame(cx);
        window.click("confirm-process-term", cx);
    })
    .checked("confirm exact real inspected process once");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.control.terminating.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx))
    })
    .checked("retire monitoring while confirmed SIGTERM is awaiting receipt");
    h.control.terminate.add_permits(1);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.status.render(cx).contains("已发送 SIGTERM"));
        assert!(panel.pending.is_none() && panel.monitor.is_none() && panel.paused);
    });
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 1);
}
