//! Production controls with real authenticated SSH routes. Synthetic completed
//! reports exercise presentation only; protocol acceptance belongs to session.

use super::{
    Arc, Duration, Ordering, PathComparisonPanel, PathComparisonReport, PathComparisonStatus,
    PathProbeFailure, PathProbeKind, PathSampleOutcome, PathSide, SshSession,
};
#[path = "path_comparison_localization.rs"]
mod localization;
use crate::workspace::tests::protocol_diagnostics::{
    change_path_comparison_candidate, change_selection, path_comparison_fixture,
};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ElementId, Entity, TestAppContext, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use keelshell_session::{SshAuth, SshOptions, path_comparison::PathSample};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{collections::HashMap, net::SocketAddr, sync::atomic::AtomicUsize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinSet,
};

trait Checked<T> {
    fn checked(self, message: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, message: &str) -> T {
        self.unwrap_or_else(|error| panic!("{message}: {error:?}"))
    }
}
impl<T> Checked<T> for Option<T> {
    fn checked(self, message: &str) -> T {
        self.unwrap_or_else(|| panic!("{message}"))
    }
}

struct Control {
    probes: AtomicUsize,
    gate: Semaphore,
}

struct Peer {
    target: Option<SocketAddr>,
    control: Arc<Control>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
}
impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "path-test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let Some(target) = self
            .target
            .filter(|target| host == "127.0.0.1" && port == u32::from(target.port()))
        else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        let mut socket = TcpStream::connect(target).await?;
        reply.accept().await;
        self.jobs.spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut socket).await;
        });
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if String::from_utf8_lossy(command).contains("KEELSHELL_PATH_PROBE") {
            self.control.probes.fetch_add(1, Ordering::AcqRel);
            session.channel_success(id)?;
            let permit = self
                .control
                .gate
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?;
            permit.forget();
            session.exit_status_request(id, 64)?;
            session.eof(id)?;
        } else {
            session.channel_failure(id)?;
        }
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }
}

struct Harness {
    window: AnyWindowHandle,
    panel: Entity<PathComparisonPanel>,
    workspace: Entity<crate::workspace::Workspace>,
    runtime: Arc<tokio::runtime::Runtime>,
    sessions: [SshSession; 3],
    control: Arc<Control>,
    servers: Vec<tokio::task::JoinHandle<()>>,
}

fn listen(
    runtime: &tokio::runtime::Runtime,
    seed: u8,
    target: Option<SocketAddr>,
    control: Arc<Control>,
) -> (SocketAddr, String, tokio::task::JoinHandle<()>) {
    let listener = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("path listener");
    let address = listener.local_addr().checked("path listener address");
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let server = runtime.spawn(async move {
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { break };
                    let config = config.clone();
                    let peer = Peer { target, control: control.clone(), channels: HashMap::new(), jobs: JoinSet::new() };
                    sessions.spawn(async move {
                        if let Ok(session) = server::run_stream(config, stream, peer).await {
                            let _ = tokio::time::timeout(Duration::from_secs(60), session).await;
                        }
                    });
                }
                _ = sessions.join_next(), if !sessions.is_empty() => {}
            }
        }
    });
    (address, fingerprint, server)
}

impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("path runtime"),
        );
        let control = Arc::new(Control {
            probes: AtomicUsize::new(0),
            gate: Semaphore::new(0),
        });
        let (target, pin, target_task) = listen(&runtime, 0x46, None, control.clone());
        let (jump, jump_pin, jump_task) = listen(&runtime, 0x47, Some(target), control.clone());
        let options = |address: SocketAddr, fingerprint: &str| {
            let mut options = SshOptions::new("127.0.0.1", "fixture");
            options.port = address.port();
            options.expected_host_key = Some(fingerprint.to_owned());
            options.auth = SshAuth::Password(zeroize::Zeroizing::new("path-test-only".into()));
            options.timeout = Duration::from_secs(3);
            options
        };
        let first = runtime
            .block_on(SshSession::connect(options(target, &pin)))
            .checked("direct path");
        let relay = runtime
            .block_on(SshSession::connect(options(jump, &jump_pin)))
            .checked("relay");
        let second = runtime
            .block_on(SshSession::connect_through(&relay, options(target, &pin)))
            .checked("relayed path");
        let (window, monitor, workspace) =
            path_comparison_fixture(first.clone(), second.clone(), runtime.clone(), cx);
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.path_comparison.clone()
        });
        panel.update(cx, |panel, cx| {
            panel.expanded = true;
            panel.refresh_candidates(cx);
            assert_eq!(panel.candidates.len(), 1);
            panel.select(panel.candidates[0].clone(), cx);
        });
        Self {
            window,
            panel,
            workspace,
            runtime,
            sessions: [first, second, relay],
            control,
            servers: vec![target_task, jump_task],
        }
    }

    fn preview(&self, cx: &mut TestAppContext) {
        self.panel.update(cx, |panel, cx| {
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
    }

    fn finish(self, cx: &mut TestAppContext) {
        self.control.gate.add_permits(64);
        cx.update_window(self.window, |_, window, _| window.remove_window())
            .checked("close path test window");
        for session in &self.sessions {
            let result = self.runtime.block_on(session.close());
            // A tab-close test already released its original transport. Accept
            // a duplicate disconnect only with observed local closure.
            assert!(
                result.is_ok() || session.is_closed(),
                "close owned SSH: {result:?}"
            );
        }
        for server in &self.servers {
            server.abort();
        }
        self.runtime.block_on(async {
            for server in self.servers {
                let _ = server.await;
            }
        });
    }
}

#[gpui_kit::test]
fn path_comparison_preview_is_local_and_aba_requires_new_review(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.preview(cx);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    cx.update_window(h.window, |_, window, cx| {
        h.workspace
            .update(cx, |view, cx| change_selection(view, "aba", window, cx));
        h.panel.update(cx, |panel, cx| {
            assert!(panel.review.is_none());
            panel.confirm(cx);
            assert!(!panel.busy());
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
    })
    .checked("ABA revokes consent before another draw");
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h.finish(cx);
}

#[gpui_kit::test]
fn path_comparison_second_route_change_blocks_confirm_before_retirement_poll(
    cx: &mut TestAppContext,
) {
    for mutation in ["route", "removed"] {
        let h = Harness::new(cx);
        h.preview(cx);
        h.workspace.update(cx, |view, cx| {
            change_path_comparison_candidate(view, mutation, cx)
        });
        h.panel.update(cx, |panel, cx| {
            panel.confirm(cx);
            assert!(panel.review.is_none());
            assert!(!panel.busy());
        });
        assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
        h.finish(cx);
    }
}

#[gpui_kit::test]
fn path_comparison_candidate_retirement_revokes_review_synchronously(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.preview(cx);
    h.workspace.update(cx, |view, cx| {
        change_path_comparison_candidate(view, "route", cx);
        change_path_comparison_candidate(view, "retire", cx);
    });
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.review.is_none());
        assert!(panel.selected.is_none());
        assert!(panel.candidates.is_empty());
    });
    h.finish(cx);
}

#[gpui_kit::test]
fn path_comparison_closing_inactive_candidate_revokes_review_without_selecting_it(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.preview(cx);
    cx.update_window(h.window, |_, window, cx| {
        h.workspace.update(cx, |view, cx| {
            change_selection(view, "close_inactive", window, cx)
        });
        h.panel.read_with(cx, |panel, _| {
            assert!(panel.review.is_none());
            assert!(panel.selected.is_none());
        });
    })
    .checked("close actual inactive comparison candidate");
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h.finish(cx);
}

fn report(status: PathComparisonStatus) -> PathComparisonReport {
    let samples = (1..=3)
        .flat_map(|round| {
            [
                PathProbeKind::Exec,
                PathProbeKind::Upload,
                PathProbeKind::Download,
            ]
            .into_iter()
            .flat_map(move |kind| {
                [PathSide::A, PathSide::B].map(|path| PathSample {
                    path,
                    kind,
                    round,
                    elapsed: Duration::from_millis(20),
                    bytes: if kind == PathProbeKind::Exec {
                        0
                    } else {
                        256 * 1024
                    },
                    outcome: PathSampleOutcome::Passed,
                })
            })
        })
        .collect();
    PathComparisonReport {
        status,
        samples,
        cleanup_unconfirmed: false,
    }
}

#[gpui_kit::test]
fn path_comparison_stale_result_never_resurrects_after_selection_aba(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.preview(cx);
    let (request, review) = h.panel.update(cx, |panel, _| {
        (
            panel.request,
            panel.review.take().checked("prepared review"),
        )
    });
    cx.update_window(h.window, |_, window, cx| {
        h.workspace
            .update(cx, |view, cx| change_selection(view, "aba", window, cx));
        h.panel.update(cx, |panel, cx| {
            panel.finish(
                request,
                review,
                Some(report(PathComparisonStatus::Complete)),
                cx,
            );
            assert!(panel.result.is_none());
        });
    })
    .checked("late comparison reply after ABA");
    h.finish(cx);
}

#[gpui_kit::test]
fn path_comparison_late_result_rechecks_second_transport_before_adoption(cx: &mut TestAppContext) {
    for mutation in ["route", "removed"] {
        let h = Harness::new(cx);
        h.preview(cx);
        let (request, review) = h.panel.update(cx, |panel, _| {
            (
                panel.request,
                panel.review.take().checked("prepared reply review"),
            )
        });
        h.workspace.update(cx, |view, cx| {
            change_path_comparison_candidate(view, mutation, cx)
        });
        h.panel.update(cx, |panel, cx| {
            panel.finish(
                request,
                review,
                Some(report(PathComparisonStatus::Complete)),
                cx,
            );
            assert!(panel.result.is_none());
        });
        h.finish(cx);
    }
}

#[gpui_kit::test]
fn path_comparison_unknown_completion_disables_next_run_and_keeps_raw_samples(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.preview(cx);
    h.panel.update(cx, |panel, cx| {
        let review = panel.review.take().checked("prepared unknown review");
        let mut result = report(PathComparisonStatus::Incomplete);
        result.samples.truncate(1);
        result.cleanup_unconfirmed = true;
        panel.finish(panel.request, review, Some(result), cx);
        assert!(panel.cleanup_unconfirmed);
        assert_eq!(panel.result.as_ref().map(|r| r.samples.len()), Some(1));
        panel.preview(cx);
        assert!(panel.review.is_none());
        assert!(
            !panel.busy(),
            "unknown read-only completion must allow release and reconnect"
        );
    });
    h.finish(cx);
}

#[gpui_kit::test]
async fn path_comparison_running_cancel_owns_worker_and_discards_revoked_reply(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.preview(cx);
    h.panel.update(cx, |panel, cx| panel.confirm(cx));
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.control.probes.load(Ordering::Acquire) > 0
    })
    .await;
    h.panel.update(cx, |panel, cx| {
        assert!(panel.busy());
        panel.invalidate_selection(cx);
        assert!(
            panel
                .cancel
                .as_ref()
                .is_some_and(|cancel| cancel.load(Ordering::Acquire))
        );
    });
    h.control.gate.add_permits(64);
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        !h.panel.read(cx).busy()
    })
    .await;
    h.panel
        .read_with(cx, |panel, _| assert!(panel.result.is_none()));
    assert!(h.sessions.iter().all(|session| !session.is_closed()));
    h.finish(cx);
}

#[gpui_kit::test]
fn path_comparison_three_themes_keep_review_actions_outside_scrolling_body(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.preview(cx);
    // Mount the actual comparison component directly, keeping its real Workspace
    // authority alive; this isolates the review geometry from monitor scroll.
    let panel = h.panel.clone();
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(320.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, _| panel,
        )
        .checked("bounded comparison view")
    });
    for family in crate::design::registry() {
        for mode in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
                cx.update_window(window, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &keelshell_core::Settings {
                            theme_family: keelshell_core::ThemeId::new(family.id)
                                .checked("registered theme"),
                            theme: mode,
                            ..Default::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    let body = window.find("path-comparison-review-body").bounds();
                    let actions = window.find("path-comparison-review-actions").bounds();
                    assert!(actions.origin.y >= body.bottom());
                    let before = window.find("path-comparison-confirm").bounds();
                    window.scroll(
                        "path-comparison-review-body",
                        gpui_kit::ScrollDelta::Lines(point(0., -100.)),
                        cx,
                    );
                    window.render_frame(cx);
                    assert_eq!(window.find("path-comparison-confirm").bounds(), before);
                    assert!(before.bottom() <= px(580.) && before.right() <= px(320.));
                })
                .checked("three themes, two modes and two languages");
            }
        }
    }
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close review view");
    h.finish(cx);
}

#[gpui_kit::test]
fn path_comparison_minimum_workspace_can_reach_review_and_result_controls(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.preview(cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("navigation-monitor", cx);
        window.render_frame(cx);
        for _ in 0..100 {
            let viewport = window.find("monitor-scroll").bounds();
            let action = window.find("path-comparison-confirm").bounds();
            if action.origin.y >= viewport.origin.y && action.bottom() <= viewport.bottom() {
                break;
            }
            window.scroll(
                "monitor-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., -2.)),
                cx,
            );
            window.render_frame(cx);
        }
        let viewport = window.find("monitor-scroll").bounds();
        let action = window.find("path-comparison-confirm").bounds();
        assert!(action.origin.y >= viewport.origin.y && action.bottom() <= viewport.bottom());
        assert!(action.origin.x >= viewport.origin.x && action.right() <= viewport.right());
        window.click("path-comparison-back", cx);
        h.panel.update(cx, |panel, cx| {
            panel.result = Some(report(PathComparisonStatus::Complete));
            panel.show_samples = true;
            panel.show_result_review = false;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(
            window
                .find("path-comparison-raw-samples")
                .bounds()
                .size
                .height
                <= px(160.)
        );
        assert!(
            window
                .find("path-comparison-samples-toggle")
                .label()
                .is_some()
        );
    })
    .checked("actual 900x580 workspace review reachability and bounded result list");
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h.finish(cx);
}
