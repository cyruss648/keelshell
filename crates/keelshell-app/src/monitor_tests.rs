//! Real SSH process responses arriving after the panel has retired its authority.
#[path = "monitor_tests/independent_disk_monitor.rs"]
mod independent_disk_monitor;
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
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    sockets: AtomicUsize,
    probes: AtomicUsize,
    inspecting: AtomicUsize,
    terminating: AtomicUsize,
    inspect: Semaphore,
    terminate: Semaphore,
    disk_samples: AtomicUsize,
    hide_first_disk: AtomicBool,
    hold_processes: AtomicBool,
    process_release: Semaphore,
    fail_resources: AtomicBool,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            commands: AtomicUsize::new(0),
            sockets: AtomicUsize::new(0),
            probes: AtomicUsize::new(0),
            inspecting: AtomicUsize::new(0),
            terminating: AtomicUsize::new(0),
            inspect: Semaphore::new(0),
            terminate: Semaphore::new(0),
            disk_samples: AtomicUsize::new(0),
            hide_first_disk: AtomicBool::new(false),
            hold_processes: AtomicBool::new(false),
            process_release: Semaphore::new(0),
            fail_resources: AtomicBool::new(false),
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
        let (body, status) = if command.contains("ss -H -lntup") {
            self.0.sockets.fetch_add(1, Ordering::Release);
            (
                "tcp LISTEN 0 128 127.0.0.1:2222 0.0.0.0:* users:((\"fixture\",pid=42,fd=3))\n"
                    .into(),
                0,
            )
        } else if command.contains("command -v nc") {
            self.0.probes.fetch_add(1, Ordering::Release);
            ("@@KS:probe@@\n0\n".into(), 0)
        } else if command.contains("@@KS:process@@") {
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
        } else if command.contains("cat /proc/stat") {
            let sample = self.0.disk_samples.fetch_add(1, Ordering::AcqRel);
            let mut disks = String::from(
                "@@KS:bootid@@\n11ac8d57-72c6-4ee6-93bf-68724d27e715\n@@KS:diskstats@@\n",
            );
            for index in 0..12 {
                if index == 0 && self.0.hide_first_disk.load(Ordering::Acquire) {
                    continue;
                }
                let ticks = 100 + sample * 10;
                let sectors = 200 + sample * 20;
                disks.push_str(&format!("8 {index} fixture{index} {ticks} 0 {sectors} {ticks} {ticks} 0 {sectors} {ticks} 0 {ticks} {ticks}\n"));
            }
            let body = "@@KS:platform@@\nLinux\n@@KS:stat@@\ncpu 100 20 30 400 50 0 0 0\nbtime 1700000000\n@@KS:meminfo@@\nMemTotal: 1024 kB\nMemAvailable: 256 kB\n@@KS:loadavg@@\n1.25 0.50 0.25 1/100 45\n@@KS:uptime@@\n100.00 75.00\n@@KS:net@@\nInter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 100 1 0 0 0 0 0 0 200 1 0 0 0 0 0 0\n@@KS:df@@\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 600 350 64% /\n@@KS:end@@\n";
            (
                body.replace("100.00 75.00", &format!("{}.00 75.00", 100 + sample * 5))
                    .replace(
                        "cpu 100 20 30 400 50 0 0 0",
                        &format!(
                            "cpu {} 20 30 {} 50 0 0 0",
                            100 + sample * 10,
                            400 + sample * 10
                        ),
                    )
                    .replace("@@KS:end@@", &(disks + "@@KS:end@@")),
                if self.0.fail_resources.load(Ordering::Acquire) {
                    65
                } else {
                    0
                },
            )
        } else if command.contains("ps -ww -eo") {
            if self.0.hold_processes.load(Ordering::Acquire) {
                let permit = self
                    .0
                    .process_release
                    .acquire()
                    .await
                    .map_err(|_| russh::Error::Disconnect)?;
                permit.forget();
            }
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

    fn load_sockets(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, _, cx| {
            self.panel
                .update(cx, |panel, cx| panel.run(Job::Sockets, cx));
        })
        .checked("load sockets through production action");
    }

    async fn close_history_fixture(&mut self, cx: &mut TestAppContext) {
        self.panel.update(cx, |panel, cx| panel.suspend(cx));
        self.idle(cx).await;
        self._runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), self.session.close())
                .await
                .checked("bounded history SSH close")
                .checked("close history SSH");
            self.server.abort();
            let joined = tokio::time::timeout(Duration::from_secs(2), &mut self.server)
                .await
                .checked("bounded history peer join");
            assert!(joined.is_ok() || joined.as_ref().is_err_and(|error| error.is_cancelled()));
        });
    }
}

#[gpui_kit::test]
async fn resource_history_real_ssh_errors_do_not_append_cached_values(cx: &mut TestAppContext) {
    use super::history::Series;
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| {
        panel.set_paused(true, cx);
        assert!(
            panel
                .history
                .vertices(Series::Cpu, std::time::Instant::now())
                .is_empty()
        );
    });
    for _ in 0..2 {
        h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
        h.idle(cx).await;
    }
    let before = h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.rates.cpu_busy_percent, Some(50.));
        assert_eq!(
            panel
                .history
                .vertices(Series::Memory, std::time::Instant::now())
                .len(),
            3
        );
        (panel.mcp_sample_id, panel.last_sample)
    });
    h.control.fail_resources.store(true, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!((panel.mcp_sample_id, panel.last_sample), before);
        assert_eq!(panel.rates.cpu_busy_percent, None);
        assert_eq!(
            panel
                .history
                .vertices(Series::Memory, std::time::Instant::now())
                .len(),
            3
        );
    });
    h.control.fail_resources.store(false, Ordering::Release);
    h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.rates.cpu_busy_percent, None,
            "no CPU interval across the error"
        );
        let points = panel
            .history
            .vertices(Series::Memory, std::time::Instant::now());
        assert_eq!(points.len(), 4);
        assert!(!points[3].connect);
    });
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn resource_history_pause_aba_and_suspend_reject_held_ssh_samples(cx: &mut TestAppContext) {
    use super::history::Series;
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    let original = h
        .panel
        .read_with(cx, |panel, _| (panel.mcp_sample_id, panel.last_sample));
    for suspend in [false, true] {
        let commands = h.control.commands.load(Ordering::Acquire);
        h.control.hold_processes.store(true, Ordering::Release);
        h.panel.update(cx, |panel, cx| {
            panel.run(Job::Refresh, cx);
            panel.run(Job::Refresh, cx); // Existing busy guard must still hold.
        });
        cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
            h.control.commands.load(Ordering::Acquire) == commands + 2 && h.panel.read(cx).busy
        })
        .await;
        let completion = h.panel.read_with(cx, |panel, _| {
            let cancel = panel
                .worker_cancel
                .as_ref()
                .unwrap_or_else(|| panic!("active refresh"));
            crate::terminal::transport_completion_for_test(cancel)
                .unwrap_or_else(|| panic!("registered refresh completion"))
        });
        h.panel.update(cx, |panel, cx| {
            if suspend {
                panel.suspend(cx);
            } else {
                // Both changes occur synchronously, without an intervening draw.
                panel.set_paused(false, cx);
                assert_eq!(
                    panel.status.translations(),
                    (
                        "自动刷新已恢复，等待下一次采样。",
                        "Polling resumed; waiting for the next sample."
                    )
                );
                panel.set_paused(true, cx);
                assert_eq!(
                    panel.status.translations(),
                    ("自动刷新已暂停。", "Polling paused.")
                );
            }
        });
        h.control.hold_processes.store(false, Ordering::Release);
        h.control.process_release.add_permits(1);
        h.idle(cx).await;
        cx.wait_for(h.window, Duration::from_secs(3), |_, _| {
            completion
                .lock()
                .checked("history worker completion")
                .is_some()
        })
        .await;
        assert_eq!(
            *completion.lock().checked("completed history worker"),
            Some(Ok(()))
        );
        h.panel.read_with(cx, |panel, _| {
            assert_eq!((panel.mcp_sample_id, panel.last_sample), original);
            assert_eq!(
                panel
                    .history
                    .vertices(Series::Memory, std::time::Instant::now())
                    .len(),
                1
            );
            assert!(!panel.busy && panel.worker_cancel.is_none());
            if !suspend {
                assert_eq!(
                    panel.status.translations(),
                    ("自动刷新已暂停。", "Polling paused.")
                );
            }
        });
        assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 2);
    }
    let mut fresh = Harness::new(cx);
    fresh.idle(cx).await;
    fresh.panel.read_with(cx, |panel, _| {
        assert!(
            panel
                .history
                .vertices(Series::Cpu, std::time::Instant::now())
                .is_empty()
        );
        assert_eq!(
            panel
                .history
                .vertices(Series::Memory, std::time::Instant::now())
                .len(),
            1
        );
        assert_ne!(
            panel.mcp_sample_id, original.0,
            "new authenticated panel starts a new series"
        );
    });
    assert!(
        !h.session.is_closed(),
        "retiring monitor authority does not close shared SSH"
    );
    fresh.close_history_fixture(cx).await;
    h.close_history_fixture(cx).await;
}

struct HistoryTestSurface(Entity<MonitorPanel>);
impl gpui_kit::Render for HistoryTestSurface {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::*;
        div()
            .size_full()
            .flex()
            .justify_end()
            .child(div().w(px(320.)).h_full().child(self.0.clone()))
    }
}

#[gpui_kit::test]
async fn resource_history_drawing_and_ax_all_families_languages_and_sizes_add_no_exec(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, cx| panel.set_paused(true, cx));
    for _ in 0..2 {
        h.panel.update(cx, |panel, cx| panel.run(Job::Refresh, cx));
        h.idle(cx).await;
    }
    let commands = h.control.commands.load(Ordering::Acquire);
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.mcp_sample_id,
            panel.last_sample,
        )
    });
    for (width, height) in [(900., 580.), (1440., 960.)] {
        let panel = h.panel.clone();
        let (window, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(width), px(height)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| HistoryTestSurface(panel)),
            )
            .checked("history viewport with a compact 320px monitor")
        });
        for family in crate::design::registry() {
            for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(window, |_, window, cx| {
                        let settings = keelshell_core::Settings {
                            theme_family: keelshell_core::ThemeId::new(family.id)
                                .checked("registered trend theme"),
                            theme: mode,
                            ..Default::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        crate::i18n::set_language(language, cx);
                        window.render_frame(cx);
                        let viewport = window.find("monitor-scroll").bounds();
                        for id in ["monitor-cpu-trend", "monitor-memory-trend"] {
                            let chart = window.find(id);
                            let bounds = chart.bounds();
                            assert_eq!(chart.role(), Some(gpui_kit::accesskit::Role::Image));
                            let label = chart.label().unwrap_or_else(|| panic!("trend AX text"));
                            assert!(label.contains("0–100%"));
                            assert!(label.contains(if language == Language::En {
                                "Local snapshot receipt"
                            } else {
                                "本机收到采样"
                            }));
                            assert!(bounds.size.width > px(0.) && bounds.size.width <= px(320.));
                            assert!(
                                bounds.origin.x >= viewport.origin.x
                                    && bounds.right() <= viewport.right()
                            );
                            assert!(
                                bounds.origin.y >= viewport.origin.y
                                    && bounds.bottom() <= viewport.bottom()
                            );
                        }
                        for id in ["refresh-monitor", "pause-monitor"] {
                            assert!(window.find(id).visible());
                        }
                    })
                    .checked("real chart paint, accessible summaries and compact bounds");
                }
            }
        }
    }
    let after = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.mcp_sample_id,
            panel.last_sample,
        )
    });
    assert_eq!(before, after);
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    h.close_history_fixture(cx).await;
}

#[gpui_kit::test]
async fn tcp_socket_probe_is_explicit_and_keeps_the_result_reviewable(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.load_sockets(cx);
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.control.sockets.load(Ordering::Acquire) == 1
    })
    .await;
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("probe-socket", 0_usize)).is_some());
        for _ in 0..40 {
            let button = window.find(("probe-socket", 0_usize)).bounds();
            let viewport = window.find("monitor-scroll").bounds();
            if button.origin.y >= viewport.origin.y && button.bottom() <= viewport.bottom() {
                break;
            }
            window.scroll(
                "monitor-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., -1.)),
                cx,
            );
            window.render_frame(cx);
        }
        window.click(("probe-socket", 0_usize), cx);
    })
    .checked("start TCP probe through explicit row action");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.control.probes.load(Ordering::Acquire) == 1
    })
    .await;
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        let result = match panel.probes.values().next() {
            Some(result) => result,
            None => panic!("probe result missing"),
        };
        assert!(result.reachable);
        assert_eq!(result.port, 2222);
        assert!(panel.status.render(cx).contains("可建立连接"));
    });
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

#[gpui_kit::test]
async fn mcp_monitor_reads_existing_fixed_cache_without_collecting_and_suspension_denies(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    let first = h
        .panel
        .read_with(cx, |panel, _| panel.mcp_snapshot())
        .unwrap_or_else(|| panic!("existing monitor cache"));
    let second = h
        .panel
        .read_with(cx, |panel, _| panel.mcp_snapshot())
        .unwrap_or_else(|| panic!("same cached sample"));
    assert_eq!(first.sample_id, second.sample_id);
    assert!(second.age_milliseconds >= first.age_milliseconds);
    assert_eq!(first.memory_total_bytes, Some(1048576));
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    h.panel.update(cx, |panel, cx| panel.suspend(cx));
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.mcp_snapshot())
            .is_none()
    );
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
}

#[gpui_kit::test]
async fn disk_device_selection_is_local_and_disappearance_cannot_display_old_rate(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("pause-monitor", cx);
        window.scroll(
            "monitor-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -4.)),
            cx,
        );
        window.render_frame(cx);
        window.click(("monitor-disk-device", 0_usize), cx);
        assert!(h.panel.read(cx).paused);
        assert!(window.try_find("monitor-disk-unavailable").is_some());
    })
    .checked("select a real first sampled disk without a command");
    let commands = h.control.commands.load(Ordering::Acquire);
    assert_eq!(h.control.disk_samples.load(Ordering::Acquire), 1);
    cx.update_window(h.window, |_, window, cx| {
        window.click("refresh-monitor", cx)
    })
    .checked("explicit second sample");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.selected_disk.as_ref().map(|id| id.name.as_str()),
            Some("fixture0")
        );
        let rate = &panel.rates.disks[0].observation;
        assert_eq!(
            rate.as_ref().map(|rate| rate.read_bytes_per_second),
            Ok(2048.)
        );
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands + 2);
    h.control.hide_first_disk.store(true, Ordering::Release);
    cx.update_window(h.window, |_, window, cx| {
        window.click("refresh-monitor", cx)
    })
    .checked("explicit disappearance sample");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("monitor-disk-unavailable").is_some());
        assert!(window.try_find("monitor-disk-metrics").is_none());
        assert_eq!(
            h.panel
                .read(cx)
                .selected_disk
                .as_ref()
                .map(|id| id.name.as_str()),
            Some("fixture0")
        );
    })
    .checked("disappeared device keeps its identity but no old numeric rates");
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
}

#[gpui_kit::test]
async fn disk_monitor_minimum_workspace_languages_themes_scroll_and_selection_are_stable(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.update(cx, |panel, _| panel.paused = true);
    cx.update_window(h.window, |_, window, cx| {
        window.click("refresh-monitor", cx)
    })
    .checked("second real disk sample");
    h.idle(cx).await;
    let commands = h.control.commands.load(Ordering::Acquire);
    crate::workspace::tests::verify_disk_monitor_minimum_workspace(h.panel.clone(), cx);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.selected_disk.as_ref().map(|id| id.name.as_str()),
            Some("fixture11")
        );
        assert_eq!(panel.rates.disks.len(), 12);
        assert!(panel.paused);
    });
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
}
