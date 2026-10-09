//! Real production multi-selection and reviewed queue flow on owned SFTP peers.
use super::*;
use gpui_kit::{ElementId, Modifiers};

async fn browse(h: &Harness, folder: PathBuf, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.browse_local(folder, window, cx));
    })
    .checked("browse the explicit batch source or destination");
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
}

fn click_element(h: &Harness, id: impl Into<ElementId>, cx: &mut TestAppContext) {
    let id = id.into();
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find(id.clone()).visible(),
            "batch selection control must be painted"
        );
        window.click(id, cx);
    })
    .checked("click the production batch checkbox");
}

fn select_local(h: &Harness, name: &str, cx: &mut TestAppContext) {
    let index = h.panel.read_with(cx, |panel, _| {
        panel
            .local_browser
            .listing
            .as_ref()
            .checked_option("local batch listing")
            .entries
            .iter()
            .position(|entry| entry.name.to_str() == Some(name))
            .checked_option("local batch source")
    });
    click_element(h, ("select-local-entry", index), cx);
}

fn select_remote(h: &Harness, path: &str, cx: &mut TestAppContext) {
    let index = h.panel.read_with(cx, |panel, _| {
        crate::files::browser::remote_indices(
            &panel.entries,
            panel.remote_show_hidden,
            panel.remote_sort,
        )
        .iter()
        .position(|index| panel.entries[*index].path == path)
        .checked_option("visible remote batch source")
    });
    click_element(h, ("select-remote-entry", index), cx);
}

fn ready(h: &Harness, cx: &mut TestAppContext) -> usize {
    h.panel.read_with(cx, |panel, _| match &panel.pending {
        Some((_, Operation::TransferBatch(plan))) => plan.ready_count(),
        _ => panic!("the real batch preparation must produce an immutable review"),
    })
}

#[gpui_kit::test]
async fn batch_upload_download_reviews_each_file_and_directory_before_exact_queue_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("one.txt", b"one full bytes\n");
    h.source("two.txt", "二号文件\n".as_bytes());
    let nested = h.local.0.join("nested");
    std::fs::create_dir(&nested).checked("create owned batch directory");
    std::fs::write(nested.join("child.txt"), b"nested complete bytes")
        .checked("seed nested batch file");
    browse(&h, h.local.0.clone(), cx).await;
    for name in ["one.txt", "two.txt", "nested"] {
        select_local(&h, name, cx);
    }
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.local_browser.selection.len(), 3);
        assert!(panel.local_browser.selected.is_none());
    });
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    assert_eq!(ready(&h, cx), 3);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.transfer_jobs.is_empty())
    );
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| i18n::set_language(language, cx));
        h.panel.read_with(cx, |panel, cx| {
            let message = &panel
                .pending
                .as_ref()
                .checked_option("complete batch review")
                .0;
            let rendered = message.render(cx);
            for name in ["one.txt", "two.txt", "nested"] {
                assert!(rendered.contains(name));
            }
            assert!(rendered.contains("13"));
            assert!(rendered.contains("/one.txt"));
            let directory_row = rendered
                .split("\n\n")
                .find(|row| row.contains(" · nested\n"))
                .checked_option("complete reviewed directory target");
            match language {
                Language::ZhCn => {
                    assert!(rendered.contains("普通文件上传仅原子发布"));
                    assert!(directory_row.contains("整棵目录非原子传输"));
                    assert!(directory_row.contains("失败或取消保留已完成项"));
                }
                Language::En => {
                    assert!(rendered.contains("Regular-file uploads publish atomically"));
                    assert!(directory_row.contains("the folder transfer is not atomic"));
                    assert!(
                        directory_row
                            .contains("retains completed items after failure or cancellation")
                    );
                }
            }
        });
    }
    h.click(cx, "cancel-file-operation");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.missing("/one.txt");
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 3);
        assert!(
            panel
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Completed)
        );
    });
    assert_eq!(h.read("/one.txt"), b"one full bytes\n");
    assert_eq!(h.read("/two.txt"), "二号文件\n".as_bytes());
    assert_eq!(h.read("/nested/child.txt"), b"nested complete bytes");
    let destination = h.local.0.join("downloads");
    std::fs::create_dir(&destination).checked("create explicit batch download destination");
    browse(&h, destination.clone(), cx).await;
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    for path in ["/one.txt", "/two.txt", "/nested"] {
        select_remote(&h, path, cx);
    }
    h.click(cx, "batch-download");
    h.idle(cx).await;
    assert_eq!(ready(&h, cx), 3);
    assert!(!destination.join("one.txt").exists());
    h.click(cx, "cancel-file-operation");
    assert!(!destination.join("one.txt").exists());
    h.click(cx, "batch-download");
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        std::fs::read(destination.join("one.txt")).checked("read exact batch destination"),
        b"one full bytes\n"
    );
    assert_eq!(
        std::fs::read(destination.join("two.txt")).checked("read exact bilingual destination"),
        "二号文件\n".as_bytes()
    );
    assert_eq!(
        std::fs::read(destination.join("nested/child.txt"))
            .checked("read exact nested destination"),
        b"nested complete bytes"
    );
}

#[gpui_kit::test]
async fn batch_changed_source_fails_only_its_exact_target_and_reports_each_result(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let changing = h.source("changing.txt", b"reviewed");
    h.source("stable.txt", b"stable full bytes");
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "changing.txt", cx);
    select_local(&h, "stable.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    assert_eq!(ready(&h, cx), 2);
    std::fs::write(changing, b"changed after approval preparation")
        .checked("mutate owned source after review");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.transfer_jobs.len(), 2);
        assert_eq!(
            panel
                .transfer_jobs
                .iter()
                .filter(|job| job.status.phase == TransferPhase::Failed)
                .count(),
            1
        );
        assert_eq!(
            panel
                .transfer_jobs
                .iter()
                .filter(|job| job.status.phase == TransferPhase::Completed)
                .count(),
            1
        );
    });
    h.missing("/changing.txt");
    assert_eq!(h.read("/stable.txt"), b"stable full bytes");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
}

#[cfg(unix)]
#[gpui_kit::test]
async fn batch_mixed_link_and_regular_selection_reports_the_rejected_target_and_never_follows_it(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("ordinary.txt", b"ordinary reviewed bytes");
    std::os::unix::fs::symlink(&source, h.local.0.join("link.txt"))
        .checked("create owned link rejection fixture");
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "ordinary.txt", cx);
    select_local(&h, "link.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    assert_eq!(ready(&h, cx), 1);
    h.panel.read_with(cx, |panel, cx| {
        let rendered = panel
            .pending
            .as_ref()
            .checked_option("mixed target review")
            .0
            .render(cx);
        assert!(rendered.contains("link.txt"));
        assert!(rendered.contains("1"));
        assert!(rendered.contains("拒绝") || rendered.contains("rejected"));
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        h.panel.read_with(cx, |panel, _| panel.transfer_jobs.len()),
        1
    );
    assert_eq!(h.read("/ordinary.txt"), b"ordinary reviewed bytes");
    h.missing("/link.txt");
}

#[gpui_kit::test]
async fn approved_batch_uses_original_pause_cancel_and_unknown_result_controls(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x56; 256 * 1024];
    h.source("first.bin", &bytes);
    h.source("second.bin", &bytes);
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "first.bin", cx);
    select_local(&h, "second.bin", cx);
    let hold = h
        .server
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("hold exact approved batch mutations");
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        hold.entered() == 2
            && h.panel
                .read(cx)
                .transfer_jobs
                .iter()
                .all(|job| job.status.phase == TransferPhase::Running)
    })
    .await;
    let (first, second) = h.panel.read_with(cx, |panel, _| {
        (panel.transfer_jobs[0].id, panel.transfer_jobs[1].id)
    });
    h.click(cx, &format!("pause-transfer-{first}"));
    h.click(cx, &format!("cancel-transfer-{second}"));
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).transfer_jobs[1].status.phase == TransferPhase::Uncertain
    })
    .await;
    assert!(!hold.expired());
    hold.release();
    cx.wait_for(h.window, Duration::from_secs(5), |_, cx| {
        h.panel.read(cx).transfer_jobs[0].status.phase == TransferPhase::Paused
    })
    .await;
    h.click(cx, &format!("pause-transfer-{first}"));
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Completed
        );
        assert_eq!(
            panel.transfer_jobs[1].status.phase,
            TransferPhase::Uncertain
        );
    });
    assert_eq!(h.read("/first.bin"), bytes);
    h.missing("/second.bin");
}

#[gpui_kit::test]
async fn batch_selection_navigation_and_session_changes_revoke_unexecuted_plans(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("selected.txt", b"not dispatched");
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "selected.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    let original = h.panel.read_with(cx, |panel, _| {
        panel
            .pending
            .clone()
            .checked_option("original batch review")
            .1
    });
    h.click(cx, "clear-file-selection");
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.run(original, window, cx));
    })
    .checked("reject stale selected batch plan");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.transfer_jobs.is_empty())
    );
    select_local(&h, "selected.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    let previous = h.panel.read_with(cx, |panel, _| {
        panel
            .pending
            .clone()
            .checked_option("navigation-bound plan")
            .1
    });
    let other = h.local.0.join("other");
    std::fs::create_dir(&other).checked("create second explicit directory");
    browse(&h, other, cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.run(previous, window, cx));
    })
    .checked("reject stale folder-bound batch plan");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.transfer_jobs.is_empty())
    );
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "selected.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
    })
    .checked("retire original SSH capability");
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()
        && panel.local_browser.selection.len() == 0));
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn old_painted_batch_approval_cannot_execute_a_replacement_plan(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("original.txt", b"old target");
    h.source("replacement.txt", b"new target");
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "original.txt", cx);
    h.click(cx, "batch-upload");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find("confirm-file-operation").bounds().center();
        let old_id = h
            .panel
            .read(cx)
            .pending_batch_id()
            .checked_option("painted original plan");
        // Retain the exact painted callback within this window transaction.
        // Entity notification or a second transaction would schedule a fresh
        // frame and test its new callback instead of the old authorization.
        h.panel.update(cx, |panel, _| {
            if let Some((_, Operation::TransferBatch(plan))) = &mut panel.pending {
                plan.id = uuid::Uuid::new_v4();
            }
        });
        assert_ne!(h.panel.read(cx).pending_batch_id(), Some(old_id));
        for input in [
            gpui_kit::MouseDownEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::MouseUpEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
        ] {
            window.dispatch_event(input, cx);
        }
        h.panel.read_with(cx, |panel, _| {
            assert!(panel.pending.is_some() && panel.transfer_jobs.is_empty());
        });
    })
    .checked("activate old painted approval without a new frame");
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_some()
        && panel.transfer_jobs.is_empty()));
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browser_modifier_ranges_hidden_filters_and_single_actions_use_current_paths(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for name in ["a.txt", "b.txt", "c.txt", ".hidden"] {
        h.source(name, name.as_bytes());
    }
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "a.txt", cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let index = h
            .panel
            .read(cx)
            .local_browser
            .listing
            .as_ref()
            .checked_option("range listing")
            .entries
            .iter()
            .position(|entry| entry.name.to_str() == Some("c.txt"))
            .checked_option("range end");
        let position = window.find(("local-entry", index)).bounds().center();
        let modifiers = Modifiers {
            shift: true,
            ..Default::default()
        };
        for input in [
            gpui_kit::MouseMoveEvent {
                position,
                modifiers,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::MouseDownEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::MouseUpEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
            }
            .to_platform_input(),
        ] {
            window.dispatch_event(input, cx);
        }
    })
    .checked("actual shifted row pointer selection");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.local_browser.selection.len(), 3);
        assert!(panel.local_browser.selected.is_none());
    });
    click_element(&h, "toggle-local-hidden", cx);
    select_local(&h, ".hidden", cx);
    assert_eq!(
        h.panel
            .read_with(cx, |panel, _| panel.local_browser.selection.len()),
        4
    );
    click_element(&h, "toggle-local-hidden", cx);
    assert_eq!(
        h.panel
            .read_with(cx, |panel, _| panel.local_browser.selection.len()),
        3
    );
    click_element(&h, "toggle-local-hidden", cx);
    assert!(!h.panel.read_with(cx, |panel, _| {
        panel
            .local_browser
            .selection
            .contains(&h.local.0.join(".hidden"))
    }));
    h.click(cx, "clear-file-selection");
    select_local(&h, "a.txt", cx);
    h.click(cx, "use-local-selection");
    assert_eq!(
        h.panel
            .read_with(cx, |panel, cx| panel.local.read(cx).value().to_string()),
        h.local.0.join("a.txt").display().to_string()
    );
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn batch_review_preserves_minimum_workspace_browsing_and_fixed_actions_in_both_languages_and_themes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    h.source("minimum.txt", b"minimum window review");
    browse(&h, h.local.0.clone(), cx).await;
    select_local(&h, "minimum.txt", cx);
    for language in [Language::ZhCn, Language::En] {
        for theme in [
            keelshell_core::Theme::System,
            keelshell_core::Theme::Light,
            keelshell_core::Theme::Dark,
        ] {
            cx.update_window(h.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                assert!(window.find("file-browsing-area").bounds().size.height >= px(64.));
            })
            .checked("measure original browsing budget with batch controls");
            h.click(cx, "batch-upload");
            h.idle(cx).await;
            assert_eq!(ready(&h, cx), 1);
            h.click(cx, "expand-file-review");
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let area = window.find("file-review-expanded").bounds();
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    assert!(
                        button.bounds().origin.x >= area.origin.x
                            && button.bounds().right() <= area.right()
                    );
                    assert!(
                        button.bounds().origin.y >= area.origin.y
                            && button.bounds().bottom() <= area.bottom()
                    );
                }
            })
            .checked("fixed batch review actions at the minimum workspace size");
            h.click(cx, "cancel-file-operation");
            assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
        }
    }
}

#[gpui_kit::test]
async fn admitted_remote_refresh_clears_selection_on_failure_and_cancellation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/selected.txt", b"remote selection identity");
    h.click(cx, "refresh-files");
    h.idle(cx).await;
    select_remote(&h, "/selected.txt", cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List("/error".into()), window, cx);
            assert!(panel.remote_selection.len() == 0);
            assert!(panel.selected.is_none());
        });
    })
    .checked("admit directory navigation before its permission failure");
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.remote_selection.len() == 0 && panel.selected.is_none()
    }));
    select_remote(&h, "/selected.txt", cx);
    let held = h
        .server
        .filesystem
        .hold_canonical_path("/")
        .checked("hold the original refresh owner");
    h.click(cx, "refresh-files");
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.remote_selection.len() == 0 && panel.selected.is_none()
    }));
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| held.entered() > 0)
        .await;
    h.click(cx, "cancel-active-file-operation");
    assert!(!held.expired());
    held.release();
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.remote_selection.len() == 0 && panel.selected.is_none()
    }));
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
