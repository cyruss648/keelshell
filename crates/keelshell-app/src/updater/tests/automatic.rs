//! Owned loopback transport and controlled GPUI service behavior; no installation.
mod painted_actions;
use super::super::{
    CURRENT_VERSION, InFlight, PROJECT_URL, PanelState, RequestOrigin, StageCleanup, StagedUpdate,
    UpdateError, UpdatePanel, hex_digest, hex_file_digest, package_name, packaged_binary_path,
    platform_name, release_info, schedule, target_triple, worker,
};
use super::{companion_payload, test_release};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{UpdateCheckFrequency, UpdatePreferences};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Cursor, Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

struct Reply {
    path: &'static str,
    status: u16,
    body: Vec<u8>,
    held: bool,
}
struct HttpFixture {
    address: std::net::SocketAddr,
    observed: mpsc::Receiver<String>,
    release: mpsc::SyncSender<()>,
    stopped: Arc<AtomicBool>,
    owner: Option<std::thread::JoinHandle<()>>,
}
impl HttpFixture {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned loopback listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("fixture address");
        let (observations, observed) = mpsc::channel();
        let (release, gate) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        let owner = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            for reply in replies {
                let mut stream = loop {
                    if stopping.load(Ordering::Acquire) {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("bounded fixture accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("read bound");
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .expect("write bound");
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).expect("owned request header");
                    headers.push(byte[0]);
                    assert!(headers.len() <= 8192, "request header bound");
                }
                let headers = String::from_utf8(headers).expect("HTTP request UTF-8");
                assert!(headers.starts_with(&format!("GET {} HTTP/1.1\r\n", reply.path)));
                assert!(!headers.to_ascii_lowercase().contains("authorization:"));
                let _ = observations.send(reply.path.to_owned());
                if reply.held {
                    gate.recv_timeout(Duration::from_secs(5))
                        .expect("bounded reply gate");
                }
                // A cancelled request may close the stream before a held reply.
                let _ = write!(
                    stream,
                    "HTTP/1.1 {} fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.status,
                    reply.body.len()
                );
                let _ = stream.write_all(&reply.body);
            }
        });
        Self {
            address,
            observed,
            release,
            stopped,
            owner: Some(owner),
        }
    }
    fn endpoint(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
    fn release(&self) {
        let _ = self.release.try_send(());
    }
    fn finish(&mut self) {
        self.release();
        self.stopped.store(true, Ordering::Release);
        if let Some(owner) = self.owner.take() {
            owner.join().expect("fixture thread reaped");
        }
    }
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.finish();
    }
}

fn mount_panel(
    cx: &mut TestAppContext,
    preferences: UpdatePreferences,
) -> (AnyWindowHandle, Entity<UpdatePanel>) {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("owned update runtime"),
    );
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_window, cx| cx.new(|cx| UpdatePanel::new(runtime, preferences, cx)),
        )
        .expect("mount production update service")
    })
}

fn metadata(tag: &str, target: &str, size: usize) -> Vec<u8> {
    let archive = package_name(tag, target).expect("archive name");
    serde_json::to_vec(&serde_json::json!({
        "tag_name":tag,"name":"Owned update fixture","body":"第一行\nOwned release changes",
        "html_url":format!("{PROJECT_URL}/releases/tag/{tag}"),
        "assets":[
            {"name":archive,"size":size,"browser_download_url":format!("{PROJECT_URL}/releases/download/{tag}/{archive}")},
            {"name":format!("{archive}.sha256"),"size":100,"browser_download_url":format!("{PROJECT_URL}/releases/download/{tag}/{archive}.sha256")}
        ]
    })).expect("owned metadata")
}

fn archive_bytes() -> (Vec<u8>, String) {
    let (_directory, payload, target, manifest) = companion_payload();
    let mut names = manifest["files"]
        .as_object()
        .expect("manifest files")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    names.push("package-manifest.json".into());
    let bytes = if package_name("v99.0.0", &target)
        .expect("package name")
        .ends_with(".zip")
    {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            archive
                .start_file(
                    &name,
                    zip::write::SimpleFileOptions::default().unix_permissions(0o755),
                )
                .expect("ZIP entry");
            archive
                .write_all(&fs::read(payload.join(name)).expect("fixture bytes"))
                .expect("ZIP bytes");
        }
        archive.finish().expect("ZIP finish").into_inner()
    } else {
        let compressor = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(compressor);
        for name in names {
            archive
                .append_path_with_name(payload.join(&name), name)
                .expect("tar entry");
        }
        archive
            .into_inner()
            .expect("tar finish")
            .finish()
            .expect("gzip finish")
    };
    (bytes, target)
}

#[gpui_kit::test]
async fn delayed_automatic_check_is_single_flight_and_persists_success_metadata(
    cx: &mut TestAppContext,
) {
    let target = target_triple().expect("target");
    let mut fixture = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 200,
        body: metadata(&format!("v{CURRENT_VERSION}"), &target, 1),
        held: true,
    }]);
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let now = Instant::now();
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(fixture.endpoint("/latest"));
        panel.poll_at(now, cx);
        assert!(
            panel.request.is_none(),
            "startup cannot request immediately"
        );
        panel.poll_at(now + schedule::STARTUP_DELAY, cx);
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        fixture.observed.try_recv().is_ok()
    })
    .await;
    let nonce = panel.read_with(cx, |panel, _| {
        panel.request.as_ref().expect("automatic check").identity.id
    });
    panel.update(cx, |panel, cx| {
        panel.poll_at(now + Duration::from_secs(100), cx);
        assert_eq!(
            panel.request.as_ref().expect("same check").identity.id,
            nonce
        );
        assert!(panel.preferences.last_successful_check.is_none());
    });
    cx.run_until_parked();
    fixture.release();
    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        panel.read(cx).request.is_none()
    })
    .await;
    panel.read_with(cx, |panel, _| {
        assert!(matches!(panel.state, PanelState::UpToDate(_)));
        assert!(panel.preferences.last_successful_check.is_some());
        assert!(!panel.schedule.due(Instant::now()));
        assert!(!matches!(panel.state, PanelState::Installing));
    });
    fixture.finish();
}

#[gpui_kit::test]
async fn automatic_download_verifies_real_archive_and_ready_continues_periodic_checks(
    cx: &mut TestAppContext,
) {
    let (archive, target) = archive_bytes();
    let name = package_name("v99.0.0", &target).expect("archive name");
    let checksum = format!("{}  {name}\n", hex_digest(&Sha256::digest(&archive)));
    let release = metadata("v99.0.0", &target, archive.len());
    let mut fixture = HttpFixture::new(vec![
        Reply {
            path: "/latest",
            status: 200,
            body: release.clone(),
            held: false,
        },
        Reply {
            path: "/checksums",
            status: 200,
            body: checksum.into_bytes(),
            held: false,
        },
        Reply {
            path: "/archive",
            status: 200,
            body: archive,
            held: false,
        },
        Reply {
            path: "/latest",
            status: 200,
            body: release.clone(),
            held: true,
        },
        Reply {
            path: "/latest",
            status: 200,
            body: release,
            held: false,
        },
    ]);
    let (window, panel) = mount_panel(
        cx,
        UpdatePreferences {
            auto_download: true,
            ..UpdatePreferences::default()
        },
    );
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(fixture.endpoint("/latest"));
        panel.download_endpoints =
            Some((fixture.endpoint("/checksums"), fixture.endpoint("/archive")));
        panel.poll_at(Instant::now() + schedule::STARTUP_DELAY, cx);
    });
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        matches!(panel.read(cx).state, PanelState::Ready(_))
    })
    .await;
    let root = panel.read_with(cx, |panel, _| match &panel.state {
        PanelState::Ready(staged) => {
            assert_eq!(
                hex_file_digest(&staged.archive).expect("archive readback"),
                staged.digest
            );
            assert_eq!(
                fs::read(
                    staged
                        .payload
                        .join(packaged_binary_path(platform_name()).expect("app path"))
                )
                .expect("payload readback"),
                b"new application"
            );
            assert!(staged.cleanup.as_ref().expect("owned stage").armed);
            staged.cleanup.as_ref().expect("owned stage").root.clone()
        }
        _ => panic!("verified ready state"),
    });
    assert_eq!(
        fixture.observed.try_iter().collect::<Vec<_>>(),
        ["/latest", "/checksums", "/archive"]
    );
    panel.update(cx, |panel, cx| {
        panel.poll_at(Instant::now() + Duration::from_secs(86_401), cx);
        assert!(matches!(panel.state, PanelState::Checking));
        assert!(
            panel.retained_stage.is_some(),
            "checking cannot discard the ready package"
        );
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        fixture.observed.try_recv().is_ok()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-update-request", cx);
    })
    .expect("cancel the held periodic check through the real button");
    panel.read_with(cx, |panel, _| {
        let PanelState::Ready(staged) = &panel.state else {
            panic!("cancelling a new check must preserve the earlier verified package");
        };
        assert_eq!(
            staged.cleanup.as_ref().expect("still-owned stage").root,
            root
        );
        assert!(panel.request.is_none());
        assert!(panel.retained_stage.is_none());
        assert_eq!(
            hex_file_digest(&staged.archive).expect("cancelled-check archive readback"),
            staged.digest
        );
    });
    fixture.release();
    panel.update(cx, |panel, cx| {
        panel.poll_at(Instant::now() + Duration::from_secs(86_401), cx)
    });
    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        panel.read(cx).request.is_none()
    })
    .await;
    panel.read_with(cx, |panel, _| {
        let PanelState::Ready(staged) = &panel.state else {
            panic!("ready restored after same release check");
        };
        assert_eq!(staged.cleanup.as_ref().expect("same stage").root, root);
        assert!(panel.retained_stage.is_none());
        assert!(!matches!(panel.state, PanelState::Installing));
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-update-request", cx);
    })
    .expect("discard the ready package through the explicitly labelled button");
    cx.wait_for(window, Duration::from_secs(3), |_, _| !root.exists())
        .await;
    fixture.finish();
}

#[gpui_kit::test]
async fn manual_check_preempts_automatic_owner_and_policy_change_cancels_actual_task(
    cx: &mut TestAppContext,
) {
    let target = target_triple().expect("target");
    let mut first = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 200,
        body: metadata("v99.0.0", &target, 1),
        held: true,
    }]);
    let mut second = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 404,
        body: Vec::new(),
        held: true,
    }]);
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(first.endpoint("/latest"));
        panel.poll_at(Instant::now() + schedule::STARTUP_DELAY, cx);
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        first.observed.try_recv().is_ok()
    })
    .await;
    let (old, cancelled) = panel.read_with(cx, |panel, _| {
        let request = panel.request.as_ref().expect("automatic owner");
        (request.identity.clone(), request._worker.cancelled.clone())
    });
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(second.endpoint("/latest"));
        panel.begin_check(RequestOrigin::Manual, cx);
    });
    assert!(cancelled.load(Ordering::Acquire));
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        second.observed.try_recv().is_ok()
    })
    .await;
    panel.update(cx, |panel, cx| {
        let replacement = panel.request.as_ref().expect("manual owner").identity.id;
        assert_ne!(replacement, old.id);
        assert!(
            panel
                .request
                .as_ref()
                .is_some_and(|request| request.origin == RequestOrigin::Manual)
        );
        panel.finish_check(
            &old,
            RequestOrigin::Automatic,
            release_info(
                serde_json::from_slice(&metadata("v99.0.0", &target, 1)).expect("release"),
                &target,
            ),
            cx,
        );
        assert_eq!(
            panel
                .request
                .as_ref()
                .expect("manual still owns")
                .identity
                .id,
            replacement
        );
        let cancellation = panel
            .request
            .as_ref()
            .expect("manual owner")
            ._worker
            .cancelled
            .clone();
        panel.set_preferences(
            UpdatePreferences {
                frequency: UpdateCheckFrequency::Disabled,
                ..UpdatePreferences::default()
            },
            cx,
        );
        assert!(cancellation.load(Ordering::Acquire));
        assert!(panel.request.is_none());
        assert!(
            !panel
                .schedule
                .due(Instant::now() + Duration::from_secs(1_000_000))
        );
    });
    first.release();
    second.release();
    first.finish();
    second.finish();
}

#[gpui_kit::test]
async fn failed_replacement_download_restores_the_previous_verified_package(
    cx: &mut TestAppContext,
) {
    let (archive, target) = archive_bytes();
    let name = package_name("v99.0.0", &target).expect("archive name");
    let checksum = format!("{}  {name}\n", hex_digest(&Sha256::digest(&archive)));
    let replacement_name = package_name("v100.0.0", &target).expect("replacement name");
    let mut fixture = HttpFixture::new(vec![
        Reply {
            path: "/latest",
            status: 200,
            body: metadata("v99.0.0", &target, archive.len()),
            held: false,
        },
        Reply {
            path: "/checksums",
            status: 200,
            body: checksum.into_bytes(),
            held: false,
        },
        Reply {
            path: "/archive",
            status: 200,
            body: archive.clone(),
            held: false,
        },
        Reply {
            path: "/latest",
            status: 200,
            body: metadata("v100.0.0", &target, archive.len()),
            held: false,
        },
        Reply {
            path: "/checksums",
            status: 200,
            body: format!("{}  {replacement_name}\n", "0".repeat(64)).into_bytes(),
            held: false,
        },
        Reply {
            path: "/archive",
            status: 200,
            body: archive,
            held: false,
        },
    ]);
    let (window, panel) = mount_panel(
        cx,
        UpdatePreferences {
            auto_download: true,
            ..UpdatePreferences::default()
        },
    );
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(fixture.endpoint("/latest"));
        panel.download_endpoints =
            Some((fixture.endpoint("/checksums"), fixture.endpoint("/archive")));
        panel.poll_at(Instant::now() + schedule::STARTUP_DELAY, cx);
    });
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        matches!(panel.read(cx).state, PanelState::Ready(_))
    })
    .await;
    let (root, digest) = panel.read_with(cx, |panel, _| {
        let PanelState::Ready(staged) = &panel.state else {
            panic!("first verified package");
        };
        (
            staged
                .cleanup
                .as_ref()
                .expect("first cleanup owner")
                .root
                .clone(),
            staged.digest.clone(),
        )
    });
    panel.update(cx, |panel, cx| {
        panel.poll_at(Instant::now() + Duration::from_secs(86_401), cx)
    });
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        let panel = panel.read(cx);
        panel.request.is_none() && matches!(panel.state, PanelState::Ready(_))
    })
    .await;
    panel.read_with(cx, |panel, cx| {
        let PanelState::Ready(staged) = &panel.state else {
            panic!("previous verified package restored");
        };
        assert_eq!(staged.release.tag, "v99.0.0");
        assert_eq!(
            staged.cleanup.as_ref().expect("same cleanup owner").root,
            root
        );
        assert_eq!(
            hex_file_digest(&staged.archive).expect("old archive readback"),
            digest
        );
        assert!(panel.retained_stage.is_none());
        assert_eq!(
            panel.status.render(cx),
            UpdateError::Checksum.message().render(cx)
        );
    });
    assert_eq!(
        fixture.observed.try_iter().collect::<Vec<_>>(),
        [
            "/latest",
            "/checksums",
            "/archive",
            "/latest",
            "/checksums",
            "/archive"
        ]
    );
    assert!(
        root.exists(),
        "failed replacement cannot consume the original package"
    );
    panel.update(cx, |panel, cx| panel.cancel(cx));
    cx.wait_for(window, Duration::from_secs(3), |_, _| !root.exists())
        .await;
    fixture.finish();
}

#[gpui_kit::test]
async fn stale_download_cannot_replace_state_and_cleanup_stays_off_foreground(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let directory = tempfile::tempdir().expect("owned stage parent");
    let root = directory.path().join("late-stage");
    fs::create_dir(&root).expect("late root");
    fs::write(root.join("owned.txt"), b"owned late output").expect("late bytes");
    panel.update(cx, |panel, cx| {
        let target = panel.current_target.clone().expect("target");
        let release = test_release(&target);
        let identity = panel.identity(target, Some(release.clone()));
        panel.state = PanelState::UpToDate(release.clone());
        panel.finish_download(
            &identity,
            Ok(StagedUpdate {
                release,
                digest: "0".repeat(64),
                archive: root.join("archive.zip"),
                payload: root.join("payload"),
                cleanup: Some(StageCleanup {
                    root: root.clone(),
                    armed: true,
                    executor: panel.runtime.handle().clone(),
                }),
            }),
            cx,
        );
        assert!(
            matches!(panel.state, PanelState::UpToDate(_)),
            "unowned result cannot alter visible state"
        );
        assert!(panel.request.is_none());
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| !root.exists())
        .await;
}

#[gpui_kit::test]
fn policy_updates_preserve_dirty_drafts_and_bilingual_minimum_controls(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    panel.update(cx, |panel, cx| {
        panel.draft.frequency = UpdateCheckFrequency::Weekly;
        panel.draft_revision += 1;
        let disabled = UpdatePreferences {
            frequency: UpdateCheckFrequency::Disabled,
            last_successful_check: Some(1_700_000_000),
            ..UpdatePreferences::default()
        };
        panel.set_preferences(disabled, cx);
        assert_eq!(panel.draft.frequency, UpdateCheckFrequency::Weekly);
        assert_eq!(
            panel.draft.last_successful_check,
            disabled.last_successful_check
        );
        panel.preferences_saved(0, disabled, cx);
        assert_eq!(
            panel.draft.frequency,
            UpdateCheckFrequency::Weekly,
            "a stale save cannot replace edits"
        );
        assert_eq!(
            panel.preferences_status.render(cx),
            crate::i18n::Message::new(
                "先前设置已保存；当前编辑尚未保存。",
                "Earlier settings were saved; current edits are not saved yet.",
            )
            .render(cx),
            "a stale completion must not label newer draft choices as saved",
        );
        panel.preferences_failed(cx);
        assert!(!panel.preferences_saving);
    });
    cx.update_window(window, |_, window, cx| {
        for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
            crate::i18n::set_language(language, cx);
            for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                for id in [
                    "updates-off",
                    "updates-daily",
                    "updates-weekly",
                    "updates-auto-download",
                    "save-update-preferences",
                    "check-updates",
                    "close-update-panel",
                    "cancel-update-panel",
                ] {
                    let element = window.find(id);
                    assert!(element.visible(), "{id} visible in {language:?}/{theme:?}");
                    let bounds = element.bounds();
                    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                    assert!(
                        bounds.right() <= window.bounds().right()
                            && bounds.bottom() <= window.bounds().bottom(),
                        "{id} remains inside minimum panel"
                    );
                }
            }
        }
    })
    .expect("bilingual controls in minimum GPUI window");
}

#[gpui_kit::test]
async fn worker_drop_aborts_actual_future_and_late_blocking_stage_is_cleaned(
    cx: &mut TestAppContext,
) {
    struct DropAck(mpsc::SyncSender<()>);
    impl Drop for DropAck {
        fn drop(&mut self) {
            let _ = self.0.try_send(());
        }
    }
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let runtime = panel.read_with(cx, |panel, _| panel.runtime.clone());
    let executor = cx.background_executor.clone();
    let cancellation = Arc::new(AtomicBool::new(false));
    let (started, start) = mpsc::sync_channel(1);
    let (dropped, drop_ack) = mpsc::sync_channel(1);
    let (owner, completion) = worker::spawn(
        &runtime,
        executor.clone(),
        cancellation.clone(),
        async move {
            let _ack = DropAck(dropped);
            started.try_send(()).expect("future started");
            std::future::pending::<()>().await;
        },
    );
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        start.try_recv().is_ok()
    })
    .await;
    drop(owner);
    drop(completion);
    assert!(cancellation.load(Ordering::Acquire));
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        drop_ack.try_recv().is_ok()
    })
    .await;

    let directory = tempfile::tempdir().expect("owned blocking-stage parent");
    let root = directory.path().join("blocked-stage");
    let blocking_root = root.clone();
    let cleanup_executor = runtime.handle().clone();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let cancellation = Arc::new(AtomicBool::new(false));
    let (owner, completion) = worker::spawn(&runtime, executor, cancellation.clone(), async move {
        tokio::task::spawn_blocking(move || {
            fs::create_dir(&blocking_root).expect("owned stage creation");
            let cleanup = StageCleanup {
                root: blocking_root,
                armed: true,
                executor: cleanup_executor,
            };
            entered.try_send(()).expect("blocking stage entered");
            gate.recv_timeout(Duration::from_secs(3))
                .expect("bounded blocking release");
            cleanup
        })
        .await
        .expect("blocking stage completion")
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        entry.try_recv().is_ok()
    })
    .await;
    assert!(root.exists());
    drop(owner);
    drop(completion);
    assert!(cancellation.load(Ordering::Acquire));
    release.try_send(()).expect("finish owned blocking work");
    cx.wait_for(window, Duration::from_secs(3), |_, _| !root.exists())
        .await;
}

#[gpui_kit::test]
async fn failed_configuration_load_pauses_background_but_manual_checks_remain_available(
    cx: &mut TestAppContext,
) {
    let mut fixture = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 404,
        body: Vec::new(),
        held: false,
    }]);
    let (window, panel) = mount_panel(
        cx,
        UpdatePreferences {
            auto_download: true,
            ..UpdatePreferences::default()
        },
    );
    panel.update(cx, |panel, cx| {
        panel.release_endpoint = Some(fixture.endpoint("/latest"));
        panel.suspend_background(cx);
        panel.poll_at(Instant::now() + Duration::from_secs(2 * 86_400), cx);
        assert!(
            panel.request.is_none(),
            "fallback defaults do not authorize background networking"
        );
        panel.begin_check(RequestOrigin::Manual, cx);
    });
    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        panel.read(cx).request.is_none()
    })
    .await;
    panel.read_with(cx, |panel, _| {
        assert!(matches!(panel.state, PanelState::Idle));
        assert!(
            panel.preferences.last_successful_check.is_some(),
            "no-release 404 is a completed check"
        );
        assert!(
            panel.background_suspended,
            "manual check cannot repair configuration authority"
        );
    });
    assert_eq!(
        fixture.observed.try_recv().expect("one explicit request"),
        "/latest"
    );
    panel.update(cx, |panel, cx| {
        panel.set_preferences(UpdatePreferences::default(), cx);
        assert!(
            !panel.background_suspended,
            "only successful state acceptance resumes scheduling"
        );
        assert!(!panel.schedule.due(Instant::now()));
    });
    fixture.finish();
}

#[gpui_kit::test]
fn completion_requires_nonce_source_version_target_generation_and_exact_release(
    cx: &mut TestAppContext,
) {
    let (_window, panel) = mount_panel(cx, UpdatePreferences::default());
    panel.update(cx, |panel, cx| {
        let target = panel.current_target.clone().expect("captured target");
        let release = test_release(&target);
        let ticket = panel.identity(target.clone(), Some(release.clone()));
        let (owner, completion) = worker::spawn(
            &panel.runtime,
            cx.background_executor().clone(),
            Arc::new(AtomicBool::new(false)),
            std::future::pending::<()>(),
        );
        drop(completion);
        panel.request = Some(InFlight {
            identity: ticket.clone(),
            origin: RequestOrigin::Manual,
            _worker: owner,
        });
        panel.state = PanelState::Checking;
        let mut variants = Vec::new();
        let mut wrong = ticket.clone();
        wrong.id = uuid::Uuid::new_v4();
        variants.push(wrong);
        let mut wrong = ticket.clone();
        wrong.source = "https://invalid.example/other-source";
        variants.push(wrong);
        let mut wrong = ticket.clone();
        wrong.version = "0.0.0";
        variants.push(wrong);
        let mut wrong = ticket.clone();
        wrong.target = "unmatched-platform".into();
        variants.push(wrong);
        let mut wrong = ticket.clone();
        wrong.generation += 1;
        variants.push(wrong);
        let mut wrong = ticket.clone();
        wrong.release.as_mut().expect("release").archive_size += 1;
        variants.push(wrong);
        for wrong in variants {
            assert!(!panel.accepts(&wrong));
            panel.finish_check(&wrong, RequestOrigin::Manual, Ok(release.clone()), cx);
            assert!(matches!(panel.state, PanelState::Checking));
            assert_eq!(
                panel.request.as_ref().expect("original owner").identity.id,
                ticket.id
            );
            assert!(panel.preferences.last_successful_check.is_none());
        }
        assert!(panel.accepts(&ticket));
        panel.cancel(cx);
        assert!(!panel.accepts(&ticket));
    });
}
