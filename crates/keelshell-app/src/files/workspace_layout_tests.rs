//! Vertical budgets with real listed data and completed/active remote work.
use super::super::{
    sftp_test_filesystem::TransferWriteHold, test_server::pause_handoff_fixture::PauseHandoff,
};
use super::*;

#[derive(Clone, Copy, Debug)]
struct LayoutScene {
    width: f32,
    height: f32,
    assistant: bool,
    language: Language,
    theme: keelshell_core::Theme,
}

impl LayoutScene {
    fn matrix() -> impl Iterator<Item = Self> {
        [(900., 580.), (1440., 900.)]
            .into_iter()
            .flat_map(|(width, height)| {
                [false, true].into_iter().flat_map(move |assistant| {
                    [Language::ZhCn, Language::En]
                        .into_iter()
                        .flat_map(move |language| {
                            [keelshell_core::Theme::Light, keelshell_core::Theme::Dark]
                                .into_iter()
                                .map(move |theme| Self {
                                    width,
                                    height,
                                    assistant,
                                    language,
                                    theme,
                                })
                        })
                })
            })
    }

    fn json(self) -> serde_json::Value {
        serde_json::json!({
            "width": self.width, "height": self.height, "assistant": self.assistant,
            "language": format!("{:?}", self.language), "theme": format!("{:?}", self.theme),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HandshakeError {
    WriteNotHeld,
    RepeatedRunningMeasurement,
    RunningNotMeasured,
    IdleBudgetElapsed,
    WriteFallbackExpired,
    PauseNotForwarded,
    PauseGateExpired,
}

struct RunningLayoutHandshake {
    hold: Option<TransferWriteHold>,
    measured: bool,
}

impl RunningLayoutHandshake {
    fn new(hold: TransferWriteHold) -> Self {
        Self {
            hold: Some(hold),
            measured: false,
        }
    }

    fn held(&self) -> Result<&TransferWriteHold, HandshakeError> {
        let hold = self.hold.as_ref().ok_or(HandshakeError::WriteNotHeld)?;
        if hold.entered() != 1 {
            return Err(HandshakeError::WriteNotHeld);
        }
        if hold.expired() {
            return Err(HandshakeError::WriteFallbackExpired);
        }
        let entered_at = hold.entered_at().ok_or(HandshakeError::WriteNotHeld)?;
        // The ten-second fallback alone cannot establish the five-second idle
        // precondition. A slow single appearance fails here, without retry.
        if entered_at.elapsed() >= Duration::from_secs(5) {
            return Err(HandshakeError::IdleBudgetElapsed);
        }
        Ok(hold)
    }

    fn measure_running(
        &mut self,
        h: &Harness,
        cx: &mut TestAppContext,
        scene: LayoutScene,
        confirm_started_at: Instant,
    ) -> Result<f64, HandshakeError> {
        if self.measured {
            return Err(HandshakeError::RepeatedRunningMeasurement);
        }
        self.held()?;
        self.measured = true;
        let measured_at = Instant::now();
        measure_state(h, cx, "running-transfer", scene);
        let running_measure_seconds = measured_at.elapsed().as_secs_f64();
        let hold = self.hold.as_ref().ok_or(HandshakeError::WriteNotHeld)?;
        h.panel.read_with(cx, |panel, _| {
            trace_layout_transfer(
                "running-measured",
                &scene.json(),
                confirm_started_at,
                &layout_transfer_snapshot(h, panel),
                serde_json::json!({
                    "running_measure_seconds": running_measure_seconds,
                    "running_appearances_per_hold": 1,
                    "wall_seconds_since_write_entry": hold.entered_at().map(|at| at.elapsed().as_secs_f64()),
                    "hold_entered": hold.entered(), "hold_expired": hold.expired(),
                    "fixture_idle_budget_seconds": 5, "write_gate_fallback_seconds": 10,
                }),
            );
        });
        self.held()?;
        Ok(running_measure_seconds)
    }

    fn release_after_pause(&mut self, pause: &PauseHandoff) -> Result<Instant, HandshakeError> {
        if !self.measured {
            return Err(HandshakeError::RunningNotMeasured);
        }
        self.held()?;
        if pause.expired() {
            return Err(HandshakeError::PauseGateExpired);
        }
        let forwarded_at = pause
            .forwarded_at()
            .ok_or(HandshakeError::PauseNotForwarded)?;
        let released_at = Instant::now();
        assert!(forwarded_at <= released_at);
        self.hold
            .take()
            .ok_or(HandshakeError::WriteNotHeld)?
            .release();
        Ok(released_at)
    }

    async fn wait_for_pause_forwarding(
        &self,
        h: &Harness,
        cx: &mut TestAppContext,
        pause: &PauseHandoff,
        scene: LayoutScene,
        confirm_started_at: Instant,
    ) {
        let entered_at = self
            .held()
            .checked("live WRITE while waiting for the second handoff")
            .entered_at()
            .checked_option("handler entry timestamp");
        let budget = Duration::from_secs(5).saturating_sub(entered_at.elapsed());
        let mut previous = None;
        cx.wait_for(h.window, budget, |_, cx| {
            let panel = h.panel.read(cx);
            let snapshot = serde_json::json!({
                "panel": layout_transfer_snapshot(h, panel),
                "worker_reached_handoff": pause.reached(),
                "session_pause_forwarded": pause.forwarded(),
                "pause_gate_expired": pause.expired(),
            });
            if previous.as_ref() != Some(&snapshot) {
                trace_layout_transfer(
                    "waiting-for-session-pause",
                    &scene.json(),
                    confirm_started_at,
                    &snapshot,
                    serde_json::json!({
                        "wall_seconds_since_write_entry": entered_at.elapsed().as_secs_f64(),
                        "gpui_test_clock_budget_seconds": budget.as_secs_f64(),
                        "shared_fixture_idle_budget_seconds": 5,
                    }),
                );
            }
            previous = Some(snapshot);
            pause.forwarded()
        })
        .await;
    }
}

// Test-only snapshots: never probe the peer or change transfer state for diagnostics.
fn layout_transfer_snapshot(h: &Harness, panel: &FilesPanel) -> serde_json::Value {
    serde_json::json!({
        "phase": panel.transfer.as_ref().map(|state| format!("{:?}", state.phase)),
        "transferred": panel.transfer.as_ref().map(|state| state.transferred),
        "total": panel.transfer.as_ref().and_then(|state| state.total),
        "busy": panel.busy,
        "suspended": panel.suspended,
        "operation_active": panel.operation_id.is_some(),
        "pending_review": panel.pending.is_some(),
        "selected_transfer": panel.selected_transfer.map(|id| id.to_string()),
        "jobs": panel.transfer_jobs.iter().map(|job| serde_json::json!({
            "id": job.id.to_string(), "phase": format!("{:?}", job.status.phase),
            "transferred": job.status.transferred,
        })).collect::<Vec<_>>(),
        "status_en": panel.status.translations().1,
        "session_closed": h.session.is_closed(),
        "active_subsystems": h.server.active.load(Ordering::Acquire),
        "active_directory_handles": h.server.filesystem.active_directory_handles(),
    })
}

fn trace_layout_transfer(
    stage: &str,
    scene: &serde_json::Value,
    started_at: Instant,
    snapshot: &serde_json::Value,
    details: serde_json::Value,
) {
    println!(
        "FILES_LAYOUT_DIAGNOSTIC_JSON {}",
        serde_json::json!({
            "stage": stage, "scene": scene,
            "wall_seconds_since_confirm_start": started_at.elapsed().as_secs_f64(),
            "snapshot": snapshot, "details": details,
        })
    );
}

fn bounds_json(bounds: Bounds<gpui_kit::Pixels>) -> serde_json::Value {
    serde_json::json!({"x": f32::from(bounds.origin.x), "y": f32::from(bounds.origin.y),
        "w": f32::from(bounds.size.width), "h": f32::from(bounds.size.height)})
}

fn contains(inner: Bounds<gpui_kit::Pixels>, outer: Bounds<gpui_kit::Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.right() <= outer.right()
        && inner.origin.y >= outer.origin.y
        && inner.bottom() <= outer.bottom()
}

fn measure_state(h: &Harness, cx: &mut TestAppContext, state: &str, scene: LayoutScene) {
    let LayoutScene {
        width,
        height,
        assistant,
        language,
        theme,
    } = scene;
    cx.update_window(h.window, |_, window, cx| {
                crate::design::apply(theme, Some(window), cx);
                i18n::set_language(language, cx);
                h.panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
                window.render_frame(cx);
                assert_true_file_row(window);
                let area = window.find("files-layout-scene").bounds();
                assert_eq!(area.size.height, px(if height == 580. { 212. } else { 340. }),
                    "use the actual production file-pane budget");
                let body = window.find("file-browsing-area").bounds();
                let tools = window.try_find("file-tools-scroll").map(|tools| tools.bounds());
                assert!(contains(body, area) && tools.is_none_or(|tools| contains(tools, area)),
                    "{state}/{language:?}/{theme:?}: browsing/tools must stay in {area:?}");
                if let Some(tools) = tools { assert!(tools.size.height >= px(28.), "one scrollable control must fit: {tools:?}"); }
                let mut controls = serde_json::Map::new();
                let required = h.panel.read_with(cx, |panel, _| {
                    let mut ids = vec!["parent-files", "refresh-files"];
                    if panel.pending.is_some() {
                        ids.extend(["confirm-file-operation", "cancel-file-operation"]);
                    } else {
                        ids.extend(["mkdir", "rename", "delete-file", "chmod-file",
                            "file-resume-mode", "upload-file", "upload-directory", "download-file",
                            "compare-directories"]);
                    }
                    if panel.transfer.is_some() { ids.push("file-transfer-details"); }
                    if state == "running-transfer" {
                        assert!(panel.busy);
                        assert_eq!(panel.transfer.as_ref().map(|s| s.phase), Some(TransferPhase::Running));
                        ids.extend(["pause-file-transfer", "cancel-active-file-operation"]);
                    } else if state == "paused-transfer" {
                        assert!(panel.busy);
                        assert_eq!(panel.transfer.as_ref().map(|s| s.phase), Some(TransferPhase::Paused));
                        ids.extend(["resume-file-transfer", "cancel-active-file-operation"]);
                    } else if state == "completed-transfer" {
                        assert!(!panel.busy);
                        assert_eq!(panel.transfer.as_ref().map(|s| s.phase), Some(TransferPhase::Completed));
                    }
                    if panel.comparison.is_some() { ids.extend(["plan-sync-to-remote", "plan-sync-to-local",
                        "review-directory-sync", "close-directory-comparison"]); }
                    if panel.editing.is_some() { ids.extend(["toggle-remote-diff", "save-remote-file",
                        "read-remote-merge", "toggle-text-patch"]); }
                    ids
                });
                for id in required {
                    reveal_file_control(window, cx, id);
                    let target = window.find(id);
                    assert!(target.visible() && contains(target.bounds(), area),
                        "{state}/{language:?}/{theme:?}/{width}/{assistant}: {id} {:?} escapes {area:?}", target.bounds());
                    controls.insert(id.into(), bounds_json(target.bounds()));
                    assert_true_file_row(window);
                }
                if let Some(message) = window.try_find("file-confirmation-message") {
                    assert!(message.bounds().size.height <= px(48.));
                    assert!(contains(message.bounds(), area));
                }
                println!("FILES_LAYOUT_JSON {}", serde_json::json!({"state":state,
                    "width":width,"height":height,"assistant":assistant,
                    "language":format!("{language:?}"),"theme":format!("{theme:?}"),
                    "area":bounds_json(area),"body":bounds_json(body),"tools":tools.map(bounds_json),
                    "first_row":bounds_json(window.find(("remote-entry",0_usize)).bounds()),
                    "reachable_controls":controls}));
            }).checked("measure production themed layout after real remote state changes");
}

#[gpui_kit::test]
async fn real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states(
    cx: &mut TestAppContext,
) {
    // Each appearance owns a fresh upload and WRITE hold; no held request
    // spans the four language/theme renders that used to share one idle budget.
    for scene in LayoutScene::matrix() {
        let LayoutScene {
            width,
            height,
            assistant,
            ..
        } = scene;
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, assistant)
        });
        h.idle(cx).await;
        h.seed("/first-real-entry.txt", b"original editor content");
        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel.run(Operation::List("/".into()), window, cx)
            });
        })
        .checked("list non-empty controlled SFTP directory");
        h.idle(cx).await;
        cx.update_window(h.window, |_, window, cx| {
            crate::design::apply(scene.theme, Some(window), cx);
            window.render_frame(cx);
            assert_true_file_row(window);
            window.click(("remote-entry", 0_usize), cx);
            assert_eq!(
                h.panel
                    .read(cx)
                    .selected
                    .as_ref()
                    .map(|entry| entry.path.as_str()),
                Some("/first-real-entry.txt")
            );
        })
        .checked("select the actually painted first SFTP entry");
        measure_state(&h, cx, "normal", scene);

        // The existing table has horizontal scrolling. Use a real wheel
        // over its header to reveal Edit without scrolling the data list.
        cx.update_window(h.window, |_, window, cx| {
            let table = window.find("remote-files-table").bounds();
            let position = point(
                table.origin.x + table.size.width / 2.,
                table.origin.y + px(8.),
            );
            window.dispatch_event(
                gpui_kit::ScrollEvent {
                    position,
                    delta: gpui_kit::ScrollDelta::Pixels(point(px(-10000.), px(0.))),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            let button = window.find(("open-remote", 0_usize));
            assert!(button.visible() && contains(button.bounds(), table));
            window.click(("open-remote", 0_usize), cx);
        })
        .checked("open actual listed remote file with a visible Edit control");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, _| {
            assert_eq!(
                panel.editing.as_ref().map(|(path, _)| path.as_str()),
                Some("/first-real-entry.txt")
            );
        });
        measure_state(&h, cx, "editor", scene);

        let bytes = vec![0x4b; 768 * 1024];
        let name = format!("uploaded-{}.bin", "x".repeat(140));
        let source = h.source(&name, &bytes);
        let remote = format!("/{name}");
        h.local_input(cx, &source);
        h.click(cx, "upload-file");
        h.panel.read_with(cx, |panel, _| {
            assert!(
                matches!(&panel.pending.as_ref().checked_option("long-path upload review").1,
                    Operation::Upload(local,target) if local == &source && target == &remote)
            );
        });
        measure_state(&h, cx, "pending-long-upload", scene);
        let hold = h
            .server
            .filesystem
            .hold_atomic_upload_after_first(&remote)
            .checked("hold this reviewed upload after its first real WRITE ACK");
        let diagnostic_scene = scene.json();
        let confirm_started_at = Instant::now();
        h.click(cx, "confirm-file-operation");
        cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
            h.panel.read(cx).transfer.as_ref().is_some_and(|state| {
                state.phase == TransferPhase::Running && state.transferred > 0 && hold.entered() > 0
            })
        })
        .await;
        let held_entries = hold.entered();
        // This is an observation time, not the server's exact WRITE-entry time.
        let hold_observed_at = Instant::now();
        h.panel.read_with(cx, |panel, _| {
            trace_layout_transfer(
                "write-hold-observed",
                &diagnostic_scene,
                confirm_started_at,
                &layout_transfer_snapshot(&h, panel),
                serde_json::json!({
                    "hold_entered": held_entries, "hold_expired": hold.expired(),
                }),
            );
        });
        let mut handshake = RunningLayoutHandshake::new(hold);
        let running_measure_seconds = handshake
            .measure_running(&h, cx, scene, confirm_started_at)
            .checked("measure one appearance while its real WRITE remains held");
        let pause = h.panel.read_with(cx, |panel, _| {
            PauseHandoff::install(
                panel
                    .transfer_pause
                    .as_ref()
                    .checked_option("selected job pause channel"),
                false,
            )
        });
        h.click(cx, "pause-file-transfer");
        h.panel.read_with(cx, |panel, _| {
            let hold = handshake
                .held()
                .checked("pause click retains a live WRITE hold");
            trace_layout_transfer(
                "pause-click-returned",
                &diagnostic_scene,
                confirm_started_at,
                &layout_transfer_snapshot(&h, panel),
                serde_json::json!({
                    "wall_seconds_since_hold_observed": hold_observed_at.elapsed().as_secs_f64(),
                    "hold_entered": hold.entered(), "hold_expired": hold.expired(),
                    "session_pause_forwarded": pause.forwarded(),
                }),
            );
            assert_eq!(
                panel.transfer.as_ref().map(|s| s.phase),
                Some(TransferPhase::Pausing)
            );
        });
        // Pausing is the UI handoff. Keep the WRITE held until the worker
        // has synchronously called the session handle's real pause method.
        // Use what remains of the same five-second held-WRITE interval.
        let write_entered_at = handshake
            .held()
            .checked("held WRITE before handoff")
            .entered_at()
            .checked_option("handler entry timestamp");
        handshake
            .wait_for_pause_forwarding(&h, cx, &pause, scene, confirm_started_at)
            .await;
        let released_at = handshake
            .release_after_pause(&pause)
            .checked("session pause forwarding must precede WRITE release");
        h.panel.read_with(cx, |panel, _| {
                trace_layout_transfer(
                    "session-pause-forwarded-before-write-release",
                    &diagnostic_scene,
                    confirm_started_at,
                    &layout_transfer_snapshot(&h, panel),
                    serde_json::json!({
                        "wall_seconds_since_write_entry": released_at.duration_since(write_entered_at).as_secs_f64(),
                        "session_pause_forwarded": pause.forwarded(), "pause_gate_expired": pause.expired(),
                        "forwarded_before_release": pause.forwarded_at().is_some_and(|at| at <= released_at),
                        "shared_fixture_idle_budget_seconds": 5,
                    }),
                );
            });
        let mut previous_snapshot = None;
        let mut polls = 0_usize;
        // Same predicate and 12-second GPUI budget as Harness::phase. The
        // locked Kit 0.9.0 polls at 10ms; poll 1201 is its final observation.
        cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
            polls += 1;
            let panel = h.panel.read(cx);
            let ready = panel
                .transfer
                .as_ref()
                .is_some_and(|state| state.phase == TransferPhase::Paused);
            let snapshot = layout_transfer_snapshot(&h, panel);
            if previous_snapshot.as_ref() != Some(&snapshot) || ready || polls == 1_201 {
                trace_layout_transfer(
                    "waiting-for-paused",
                    &diagnostic_scene,
                    confirm_started_at,
                    &snapshot,
                    serde_json::json!({
                        "expected_phase": "Paused", "poll": polls,
                        "gpui_test_clock_budget_seconds": 12,
                        "wall_seconds_since_hold_release": released_at.elapsed().as_secs_f64(),
                    }),
                );
            }
            previous_snapshot = Some(snapshot);
            ready
        })
        .await;
        let paused_bytes = h.staged(&remote);
        assert!(!paused_bytes.is_empty() && paused_bytes.len() < bytes.len());
        assert_eq!(
            paused_bytes,
            bytes[..paused_bytes.len()],
            "paused staging is an exact source prefix"
        );
        h.panel.read_with(cx, |panel, _| {
            assert_eq!(
                panel.transfer.as_ref().map(|state| state.transferred),
                Some(paused_bytes.len() as u64)
            );
        });
        measure_state(&h, cx, "paused-transfer", scene);
        assert_eq!(
            h.staged(&remote),
            paused_bytes,
            "paused layout must admit no later WRITE"
        );
        h.click(cx, "resume-file-transfer");
        h.idle(cx).await;
        assert_eq!(
            h.read(&remote),
            bytes,
            "a real approved upload must finish with exact bytes"
        );
        measure_state(&h, cx, "completed-transfer", scene);

        cx.update_window(h.window, |_, window, cx| {
            h.panel
                .read(cx)
                .mode
                .clone()
                .update(cx, |input, cx| input.set_value("0640", window, cx));
        })
        .checked("set exact permissions review draft");
        h.click(cx, "chmod-file");
        h.panel.read_with(cx, |panel,_| {
                assert!(panel.transfer.is_some() && panel.editing.is_some());
                assert!(matches!(&panel.pending.as_ref().checked_option("permissions review with completed transfer").1,
                    Operation::SetPermissions(entry,mode) if entry.path == "/first-real-entry.txt" && *mode == 0o640));
            });
        measure_state(&h, cx, "pending-permissions-with-transfer-editor", scene);
        h.click(cx, "cancel-file-operation");
        assert_eq!(h.read("/first-real-entry.txt"), b"original editor content");

        h.local_input(cx, &h.local.0);
        h.click(cx, "compare-directories");
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, _| {
            assert!(
                panel.comparison.is_some(),
                "actual directory comparison finished"
            );
            assert!(panel.editing.is_some());
            assert!(
                panel.transfer.is_none(),
                "comparison starts a new workflow and retires the transfer card"
            );
        });
        measure_state(&h, cx, "comparison-with-editor-after-transfer", scene);

        cx.update_window(h.window, |_, window, cx| {
            h.panel.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_value("unsent reviewed draft", window, cx)
            });
        })
        .checked("change an isolated editor draft");
        h.click(cx, "toggle-remote-diff");
        measure_state(&h, cx, "diff-with-comparison", scene);
        h.click(cx, "save-remote-file");
        measure_state(&h, cx, "pending-save-with-comparison", scene);
        h.click(cx, "cancel-file-operation");
        assert_eq!(h.read("/first-real-entry.txt"), b"original editor content");
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
        h.idle(cx).await;
        measure_state(&h, cx, "suspended-editor-with-comparison", scene);
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.has_unsaved_draft(cx));
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "unsent reviewed draft"
            );
        });
        cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
            h.server.active.load(Ordering::Acquire) == 0
                && h.server.filesystem.active_directory_handles() == 0
        })
        .await;
        println!(
            "FILES_LAYOUT_TRANSFER_JSON {}",
            serde_json::json!({"width":width,
                "height":height,"assistant":assistant,
                "language":format!("{:?}",scene.language),"theme":format!("{:?}",scene.theme),
                "held_entries":held_entries,"running_appearances_per_hold":1,
                "session_pause_forwarded_before_release":pause.forwarded_at().is_some_and(|at| at <= released_at),
                "wall_seconds_write_entry_to_release":released_at.duration_since(write_entered_at).as_secs_f64(),
                "running_measure_seconds":running_measure_seconds,"paused_bytes":paused_bytes.len(),
                "completed_bytes":bytes.len(),"active_subsystems":0,"active_handles":0,
                "fixture_transport_timeout_seconds":5,"write_gate_fallback_seconds":10})
        );
        // Each scene uses one process-wide admission permit. Close its
        // window before the next iteration; the test App otherwise retains
        // every previous scene until the whole function returns.
        close_layout_scene(h, cx);
    }
}

fn close_layout_scene(h: Harness, cx: &mut TestAppContext) {
    let Harness {
        window,
        panel,
        server: _server,
        runtime: _runtime,
        session: _session,
        local: _local,
        mutation_group,
    } = h;
    let weak_group = Arc::downgrade(&mutation_group);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close completed layout scene");
    drop(panel);
    drop(mutation_group);
    cx.update(|_| {});
    assert_eq!(
        weak_group.strong_count(),
        0,
        "completed layout scene must release fixture admission"
    );
}

#[gpui_kit::test]
async fn running_layout_rejects_repeated_appearance_and_release_before_session_pause(
    cx: &mut TestAppContext,
) {
    let scene = LayoutScene::matrix()
        .next()
        .checked_option("first appearance");
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(
            cx,
            session,
            runtime,
            scene.width,
            scene.height,
            scene.assistant,
        )
    });
    h.idle(cx).await;
    h.seed("/first-real-entry.txt", b"negative-control row");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List("/".into()), window, cx)
        });
    })
    .checked("list the negative control's actual remote row");
    h.idle(cx).await;

    let bytes: Vec<_> = (0..384 * 1024).map(|index| (index % 251) as u8).collect();
    let remote = "/handshake-negative.bin";
    let source = h.source("handshake-negative.bin", &bytes);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.panel.read_with(cx, |panel, _| {
        assert!(
            matches!(&panel.pending.as_ref().checked_option("negative exact upload review").1,
                Operation::Upload(local, target) if local == &source && target == remote)
        );
    });
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first(remote)
        .checked("hold a real WRITE for deterministic old-order controls");
    let confirm_started_at = Instant::now();
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        hold.entered() == 1
            && h.panel
                .read(cx)
                .transfer
                .as_ref()
                .is_some_and(|state| state.phase == TransferPhase::Running && state.transferred > 0)
    })
    .await;
    let writes_at_entry = h.server.filesystem.transfer_writes_started();
    let mut handshake = RunningLayoutHandshake::new(hold);
    handshake
        .measure_running(&h, cx, scene, confirm_started_at)
        .checked("first appearance is measured under its real WRITE");
    // Attempt the old four-appearance sequence. The next three renders must
    // be rejected before any layout work can consume this hold's idle budget.
    for next in LayoutScene::matrix().skip(1).take(3) {
        assert_eq!(
            handshake.measure_running(&h, cx, next, confirm_started_at),
            Err(HandshakeError::RepeatedRunningMeasurement)
        );
    }

    let pause = h.panel.read_with(cx, |panel, _| {
        PauseHandoff::install(
            panel
                .transfer_pause
                .as_ref()
                .checked_option("negative control's selected job pause channel"),
            true,
        )
    });
    h.click(cx, "pause-file-transfer");
    let entered_at = handshake
        .held()
        .checked("negative control retains its WRITE")
        .entered_at()
        .checked_option("negative WRITE entry timestamp");
    let budget = Duration::from_secs(5).saturating_sub(entered_at.elapsed());
    cx.wait_for(h.window, budget, |_, _| pause.reached()).await;
    assert!(!pause.forwarded() && !pause.expired());
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|state| state.phase),
            Some(TransferPhase::Pausing)
        );
    });
    assert_eq!(
        handshake.release_after_pause(&pause),
        Err(HandshakeError::PauseNotForwarded),
        "UI Pausing alone must not release the pending WRITE"
    );
    assert_eq!(
        handshake
            .held()
            .checked("rejected release retains hold")
            .entered(),
        1
    );
    assert_eq!(
        h.server.filesystem.transfer_writes_started(),
        writes_at_entry
    );
    h.panel.read_with(cx, |panel, _| {
        trace_layout_transfer(
            "negative-old-order-rejected",
            &scene.json(),
            confirm_started_at,
            &layout_transfer_snapshot(&h, panel),
            serde_json::json!({
                "repeated_appearances_rejected": 3, "worker_reached_handoff": pause.reached(),
                "session_pause_forwarded": pause.forwarded(), "pause_gate_expired": pause.expired(),
                "write_still_held": handshake.held().is_ok(), "write_release_rejected": true,
                "wall_seconds_since_write_entry": entered_at.elapsed().as_secs_f64(),
            }),
        );
    });
    pause.allow_forwarding();
    handshake
        .wait_for_pause_forwarding(&h, cx, &pause, scene, confirm_started_at)
        .await;
    let released_at = handshake
        .release_after_pause(&pause)
        .checked("corrected order admits release after real session pause");
    h.phase(cx, TransferPhase::Paused).await;
    let paused_bytes = h.staged(remote);
    assert!(!paused_bytes.is_empty() && paused_bytes.len() < bytes.len());
    assert_eq!(paused_bytes, bytes[..paused_bytes.len()]);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|state| state.transferred),
            Some(paused_bytes.len() as u64)
        );
    });
    measure_state(&h, cx, "paused-transfer", scene);
    assert_eq!(
        h.staged(remote),
        paused_bytes,
        "paused WRITE count remains stable"
    );
    h.click(cx, "resume-file-transfer");
    h.idle(cx).await;
    h.phase(cx, TransferPhase::Completed).await;
    assert_eq!(h.read(remote), bytes);
    h.panel.update(cx, |panel, cx| panel.suspend(cx));
    h.idle(cx).await;
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
            && h.server.filesystem.active_directory_handles() == 0
    })
    .await;
    println!(
        "FILES_LAYOUT_HANDSHAKE_NEGATIVE_JSON {}",
        serde_json::json!({
            "old_repeated_measurements_rejected": 3, "old_release_order_rejected": true,
            "session_pause_forwarded_before_release": pause.forwarded_at().is_some_and(|at| at <= released_at),
            "paused_bytes": paused_bytes.len(), "completed_bytes": bytes.len(),
            "active_subsystems": h.server.active.load(Ordering::Acquire),
            "active_handles": h.server.filesystem.active_directory_handles(),
            "wall_seconds_write_entry_to_release": released_at.duration_since(entered_at).as_secs_f64(),
        })
    );
    close_layout_scene(h, cx);
}

#[gpui_kit::test]
async fn owned_write_hold_releases_real_uploads_on_early_exit_and_cancel(cx: &mut TestAppContext) {
    for cancel in [false, true] {
        let h = Harness::new(cx);
        h.idle(cx).await;
        let bytes = vec![0x4b; 384 * 1024];
        let remote = if cancel {
            "/cancel-held.bin"
        } else {
            "/released-before-entry.bin"
        };
        let source = h.source(remote.trim_start_matches('/'), &bytes);
        let mut hold = Some(
            h.server
                .filesystem
                .hold_atomic_upload_after_first(remote)
                .checked("own the exact reviewed target's write hold"),
        );
        h.local_input(cx, &source);
        h.click(cx, "upload-file");
        h.panel.read_with(cx, |panel, _| {
            assert!(
                matches!(&panel.pending.as_ref().checked_option("owned hold exact upload review").1,
                Operation::Upload(local, target) if local == &source && target == remote)
            );
        });
        if !cancel {
            hold.take()
                .checked_option("early release remains owned")
                .release();
        }
        h.click(cx, "confirm-file-operation");
        if cancel {
            let hold = hold.take().checked_option("cancelled hold remains owned");
            cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
                h.panel
                    .read(cx)
                    .transfer
                    .as_ref()
                    .is_some_and(|s| s.phase == TransferPhase::Running && s.transferred > 0)
                    && hold.entered() > 0
            })
            .await;
            assert!(!hold.expired());
            h.click(cx, "cancel-active-file-operation");
            h.panel.read_with(cx, |panel, _| {
                assert_eq!(
                    panel.transfer.as_ref().map(|s| s.phase),
                    Some(TransferPhase::Cancelling)
                );
            });
            h.phase(cx, TransferPhase::Uncertain).await;
            drop(hold);
        }
        h.idle(cx).await;
        h.phase(
            cx,
            if cancel {
                TransferPhase::Uncertain
            } else {
                TransferPhase::Completed
            },
        )
        .await;
        cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
            h.server.active.load(Ordering::Acquire) == 0
                && h.server.filesystem.active_directory_handles() == 0
        })
        .await;
        let actual = if cancel {
            h.missing(remote);
            Vec::new()
        } else {
            h.read(remote)
        };
        if !cancel {
            assert_eq!(actual, bytes);
        }
        cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
            h.server.active.load(Ordering::Acquire) == 0
                && h.server.filesystem.active_directory_handles() == 0
        })
        .await;
        println!(
            "FILES_HOLD_CLEANUP cancel={cancel} bytes={} subsystems={} handles={}",
            actual.len(),
            h.server.active.load(Ordering::Acquire),
            h.server.filesystem.active_directory_handles()
        );
    }
}
