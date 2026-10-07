//! Update settings and history use real optimistic storage without changing SSH UI.
use super::*;
use keelshell_core::{Theme, UpdateCheckFrequency, UpdatePreferences};

struct HeldUpdateReply {
    endpoint: String,
    observed: std::sync::mpsc::Receiver<()>,
    release: std::sync::mpsc::SyncSender<()>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    owner: Option<std::thread::JoinHandle<()>>,
}

impl HeldUpdateReply {
    fn new() -> Self {
        use std::io::{Read, Write};
        use std::sync::{atomic::Ordering, mpsc};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").checked("owned update listener");
        listener
            .set_nonblocking(true)
            .checked("nonblocking update listener");
        let endpoint = format!(
            "http://{}/latest",
            listener.local_addr().checked("update address")
        );
        let (observations, observed) = mpsc::sync_channel(1);
        let (release, gate) = mpsc::sync_channel(1);
        let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = stopped.clone();
        let owner = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let mut stream = loop {
                if stopping.load(Ordering::Acquire) {
                    return;
                }
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("bounded update accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .checked("update read bound");
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .checked("update write bound");
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream
                    .read_exact(&mut byte)
                    .checked("held update request header");
                headers.push(byte[0]);
                assert!(headers.len() <= 8192, "bounded update headers");
            }
            let headers = String::from_utf8(headers).checked("update request UTF-8");
            assert!(headers.starts_with("GET /latest HTTP/1.1\r\n"));
            assert!(!headers.to_ascii_lowercase().contains("authorization:"));
            observations
                .try_send(())
                .checked("observe held HTTP request");
            gate.recv_timeout(Duration::from_secs(5))
                .checked("bounded held update release");
            stream
                .write_all(
                    b"HTTP/1.1 404 fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .checked("hidden service still owns request");
        });
        Self {
            endpoint,
            observed,
            release,
            stopped,
            owner: Some(owner),
        }
    }
}

impl Drop for HeldUpdateReply {
    fn drop(&mut self) {
        let _ = self.release.try_send(());
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(owner) = self.owner.take() {
            owner.join().checked("owned HTTP update thread reaped");
        }
    }
}

#[gpui_kit::test]
async fn hiding_and_reopening_the_panel_preserves_an_actual_inflight_http_check(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let reply = HeldUpdateReply::new();
    let service = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.update_service.clone());
    service.update(cx, |panel, _| {
        panel.set_test_release_endpoint(reply.endpoint.clone())
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.open_updates(window, cx));
        window.render_frame(cx);
        window.click("check-updates", cx);
    })
    .checked("start a real manual release check");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, _| {
        reply.observed.try_recv().is_ok()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("close-update-panel", cx);
    })
    .checked("hide while response remains held");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.update_panel.is_none());
        assert_eq!(workspace.update_service.entity_id(), service.entity_id());
        assert!(
            service
                .read(cx)
                .preferences_snapshot()
                .last_successful_check
                .is_none()
        );
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.open_updates(window, cx));
        fixture.workspace.read_with(cx, |workspace, _| {
            assert_eq!(
                workspace
                    .update_panel
                    .as_ref()
                    .ok_or("reopened service absent")
                    .checked("same reopened service")
                    .entity_id(),
                service.entity_id()
            );
        });
        window.render_frame(cx);
        window.click("close-update-panel", cx);
    })
    .checked("reopen the same owned service and hide again");
    cx.run_until_parked();
    reply
        .release
        .try_send(())
        .checked("release actual HTTP response");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        workspace
            .state
            .settings
            .updates
            .last_successful_check
            .is_some()
            && !workspace.saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.update_panel.is_none());
        assert_eq!(workspace.update_service.entity_id(), service.entity_id());
    });
    assert!(
        fixture
            .store
            .load()
            .checked("hidden check history persisted")
            .settings
            .updates
            .last_successful_check
            .is_some()
    );
    drop(reply);
}

#[gpui_kit::test]
async fn update_policy_controls_save_without_replacing_remote_panes_or_unsent_drafts(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let (command_id, revision, sources) = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.command.update(cx, |command, cx| {
                    command.set_value("printf '未执行更新草稿'", window, cx)
                });
                workspace.open_updates(window, cx);
                (
                    workspace.command.entity_id(),
                    workspace.command_revision,
                    workspace.command_sources_revision,
                )
            })
        })
        .checked("open persistent update service beside remote panes");
    cx.update_window(fixture.window, |_, window, cx| {
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            for theme in [Theme::Light, Theme::Dark] {
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
                ] {
                    let element = window.find(id);
                    assert!(element.visible());
                    assert!(
                        element.bounds().size.width > px(0.)
                            && element.bounds().size.height > px(0.)
                    );
                    assert!(element.bounds().right() <= window.bounds().right());
                    assert!(element.bounds().bottom() <= window.bounds().bottom());
                }
            }
        }
        window.click("updates-weekly", cx);
        window.render_frame(cx);
        window.click("updates-auto-download", cx);
        window.render_frame(cx);
        window.click("save-update-preferences", cx);
    })
    .checked("save actual frequency and optional download buttons");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(
            workspace.state.settings.updates.frequency,
            UpdateCheckFrequency::Weekly
        );
        assert!(workspace.state.settings.updates.auto_download);
        assert!(
            workspace
                .state
                .settings
                .updates
                .last_successful_check
                .is_none()
        );
        assert_eq!(workspace.command.entity_id(), command_id);
        assert_eq!(workspace.command_revision, revision);
        assert_eq!(workspace.command_sources_revision, sources);
        assert_eq!(
            workspace.command.read(cx).value(),
            "printf '未执行更新草稿'"
        );
        assert_eq!(workspace.tabs[0].entity_id(), panes[0].terminal.entity_id());
        assert_eq!(workspace.tabs[1].entity_id(), panes[1].terminal.entity_id());
        assert!(workspace.update_panel.is_some());
    });
    let saved = fixture.store.load().checked("real policy readback");
    assert_eq!(
        saved.settings.updates,
        UpdatePreferences {
            frequency: UpdateCheckFrequency::Weekly,
            auto_download: true,
            last_successful_check: None
        }
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("close-update-panel", cx);
    })
    .checked("hide without destroying the service");
    cx.run_until_parked();
    fixture
        .workspace
        .read_with(cx, |workspace, _| assert!(workspace.update_panel.is_none()));
    for pane in &panes {
        assert!(
            writes(pane).is_empty(),
            "update preferences cannot dispatch SSH commands"
        );
    }
}

#[gpui_kit::test]
async fn check_history_merges_into_other_saves_without_invalidating_suggestions(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let revision = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.command_sources_revision);
    let timestamp = 1_700_000_000;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.pending_update_check = Some(timestamp);
            workspace.select_theme(Theme::Dark, window, cx);
        });
    })
    .checked("merge pending update receipt into a concurrent appearance save");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(
            workspace.state.settings.updates.last_successful_check,
            Some(timestamp)
        );
        assert!(workspace.pending_update_check.is_none());
        assert_eq!(workspace.command_sources_revision, revision);
    });
    let newer = timestamp + 10;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.pending_update_check = Some(newer);
            workspace.flush_update_check(window, cx);
        });
    })
    .checked("history-only background storage");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.command_sources_revision, revision);
        assert_eq!(
            workspace.state.settings.updates.last_successful_check,
            Some(newer)
        );
        assert!(workspace.pending_update_check.is_none());
    });
    assert_eq!(
        fixture
            .store
            .load()
            .checked("history persisted to disk")
            .settings
            .updates
            .last_successful_check,
        Some(newer)
    );
}

#[gpui_kit::test]
async fn update_policy_save_conflict_keeps_winning_disk_bytes_and_retryable_draft(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let other = StateStore::new(fixture.store.path());
    let mut winner = other.load().checked("other instance state");
    winner.settings.updates.frequency = UpdateCheckFrequency::Disabled;
    winner = other.save(&winner).checked("other instance policy save");
    let bytes = std::fs::read(fixture.store.path()).checked("winner bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.open_updates(window, cx));
        window.render_frame(cx);
        window.click("updates-weekly", cx);
        window.render_frame(cx);
        window.click("save-update-preferences", cx);
    })
    .checked("submit draft against stale optimistic snapshot");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("winner readback"),
        bytes
    );
    assert_eq!(
        StateStore::new(fixture.store.path())
            .load()
            .checked("winning state"),
        winner
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find("save-update-preferences").visible(),
            "failed save retains policy draft"
        );
        fixture.workspace.read_with(cx, |workspace, _| {
            assert_eq!(
                workspace.state.settings.updates.frequency,
                UpdateCheckFrequency::Daily
            )
        });
    })
    .checked("inspect retained draft without claiming stale save succeeded");
}

#[gpui_kit::test]
fn reload_event_applies_new_policy_to_long_lived_service(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let other = StateStore::new(fixture.store.path());
    let mut latest = other.load().checked("latest state");
    latest.settings.updates.frequency = UpdateCheckFrequency::Disabled;
    latest = other
        .save(&latest)
        .checked("disable policy in other instance");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.open_profile_sync(window, cx));
        let panel = fixture
            .workspace
            .read(cx)
            .profile_sync
            .clone()
            .ok_or("profile sync panel absent")
            .checked("profile sync panel");
        panel.update(cx, |_, cx| {
            cx.emit(crate::profile_sync::ProfileSyncEvent::Changed {
                state: Box::new(latest),
                message: crate::i18n::Message::new("自有状态重载", "Owned state reloaded"),
            })
        });
    })
    .checked("actual local-state replacement event");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(
            workspace.state.settings.updates.frequency,
            UpdateCheckFrequency::Disabled
        );
        assert_eq!(
            workspace
                .update_service
                .read(cx)
                .preferences_snapshot()
                .frequency,
            UpdateCheckFrequency::Disabled
        );
    });
}
