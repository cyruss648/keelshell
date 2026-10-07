//! Vertical budgets with real listed data and completed/active remote work.
use super::*;

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

fn measure_state(
    h: &Harness,
    cx: &mut TestAppContext,
    state: &str,
    width: f32,
    height: f32,
    assistant: bool,
) {
    for language in [Language::ZhCn, Language::En] {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            cx.update_window(h.window, |_, window, cx| {
                crate::design::apply(theme, Some(window), cx);
                i18n::set_language(language, cx);
                h.panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
                window.render_frame(cx);
                assert_true_file_row(window);
                let area = window.find("files-layout-scene").bounds();
                let body = window.find("file-browsing-area").bounds();
                let tools = window.find("file-tools-scroll").bounds();
                assert!(contains(body, area) && contains(tools, area),
                    "{state}/{language:?}/{theme:?}: browsing/tools must stay in {area:?}");
                assert!(tools.size.height >= px(28.), "one scrollable control must fit: {tools:?}");
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
                    assert!(window.try_find(id).is_some(), "{state} must render its {id} control");
                }
                for id in ["parent-files", "refresh-files", "mkdir", "rename", "delete-file",
                    "chmod-file", "file-resume-mode", "upload-file", "upload-directory",
                    "download-file", "compare-directories", "confirm-file-operation",
                    "cancel-file-operation", "file-transfer-details", "pause-file-transfer",
                    "resume-file-transfer", "cancel-active-file-operation", "plan-sync-to-remote",
                    "plan-sync-to-local", "review-directory-sync", "close-directory-comparison",
                    "toggle-remote-diff", "save-remote-file", "read-remote-merge",
                    "toggle-text-patch"] {
                    if window.try_find(id).is_none() { continue; }
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
                    "area":bounds_json(area),"body":bounds_json(body),"tools":bounds_json(tools),
                    "first_row":bounds_json(window.find(("remote-entry",0_usize)).bounds()),
                    "reachable_controls":controls}));
            }).checked("measure production themed layout after real remote state changes");
        }
    }
}

#[gpui_kit::test]
async fn real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        for assistant in [false, true] {
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
                crate::design::apply(keelshell_core::Theme::Light, Some(window), cx);
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
            measure_state(&h, cx, "normal", width, height, assistant);

            // The existing table has horizontal scrolling. Use a real wheel
            // over its header to reveal Edit without scrolling the data list.
            cx.update_window(h.window, |_, window, cx| {
                let table = window.find("remote-files-table").bounds();
                let position = point(
                    table.origin.x + table.size.width / 2.,
                    table.origin.y + px(8.),
                );
                window.dispatch_event(
                    gpui_kit::ScrollWheelEvent {
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
            measure_state(&h, cx, "editor", width, height, assistant);

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
            measure_state(&h, cx, "pending-long-upload", width, height, assistant);
            let hold = h
                .server
                .filesystem
                .hold_atomic_upload_after_first(&remote)
                .checked("hold this reviewed upload after its first real WRITE ACK");
            h.click(cx, "confirm-file-operation");
            cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
                h.panel.read(cx).transfer.as_ref().is_some_and(|state| {
                    state.phase == TransferPhase::Running
                        && state.transferred > 0
                        && hold.entered() > 0
                })
            })
            .await;
            let held_entries = hold.entered();
            let measured_at = Instant::now();
            measure_state(&h, cx, "running-transfer", width, height, assistant);
            let running_measure_seconds = measured_at.elapsed().as_secs_f64();
            assert_eq!(
                hold.entered(),
                held_entries,
                "no later WRITE may enter while the real handler is held"
            );
            assert!(
                !hold.expired(),
                "running layout must finish before the fixture fallback"
            );
            h.click(cx, "pause-file-transfer");
            h.panel.read_with(cx, |panel, _| {
                assert_eq!(
                    panel.transfer.as_ref().map(|s| s.phase),
                    Some(TransferPhase::Pausing)
                );
            });
            // Pausing proves the UI published its request, not worker ACK.
            // Release the pending real WRITE so the worker can acknowledge a
            // complete chunk, then demand the actual Paused event below.
            hold.release();
            h.phase(cx, TransferPhase::Paused).await;
            let paused_bytes = h.staged(&remote);
            assert!(!paused_bytes.is_empty() && paused_bytes.len() < bytes.len());
            measure_state(&h, cx, "paused-transfer", width, height, assistant);
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
            measure_state(&h, cx, "completed-transfer", width, height, assistant);

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
            measure_state(
                &h,
                cx,
                "pending-permissions-with-transfer-editor",
                width,
                height,
                assistant,
            );
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
            measure_state(
                &h,
                cx,
                "comparison-with-editor-after-transfer",
                width,
                height,
                assistant,
            );

            cx.update_window(h.window, |_, window, cx| {
                h.panel.read(cx).editor.clone().update(cx, |editor, cx| {
                    editor.set_value("unsent reviewed draft", window, cx)
                });
            })
            .checked("change an isolated editor draft");
            h.click(cx, "toggle-remote-diff");
            measure_state(&h, cx, "diff-with-comparison", width, height, assistant);
            h.click(cx, "save-remote-file");
            measure_state(
                &h,
                cx,
                "pending-save-with-comparison",
                width,
                height,
                assistant,
            );
            h.click(cx, "cancel-file-operation");
            assert_eq!(h.read("/first-real-entry.txt"), b"original editor content");
            h.panel.update(cx, |panel, cx| panel.suspend(cx));
            h.idle(cx).await;
            measure_state(
                &h,
                cx,
                "suspended-editor-with-comparison",
                width,
                height,
                assistant,
            );
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
                "height":height,"assistant":assistant,"held_entries":held_entries,
                "running_measure_seconds":running_measure_seconds,"paused_bytes":paused_bytes.len(),
                "completed_bytes":bytes.len(),"active_subsystems":0,"active_handles":0,
                "fixture_transport_timeout_seconds":5,"write_gate_fallback_seconds":10})
            );
        }
    }
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
