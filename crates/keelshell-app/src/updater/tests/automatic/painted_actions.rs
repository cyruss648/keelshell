//! Activate previously painted controls without a redraw after service changes.
use super::*;
use gpui_kit::{App, Context, InputEvent, MouseButton, Point, Window};

fn staged(panel: &UpdatePanel, parent: &std::path::Path, name: &str) -> StagedUpdate {
    let root = parent.join(name);
    fs::create_dir(&root).expect("isolated stage");
    let archive = root.join("archive.tar.gz");
    fs::write(&archive, b"owned staged-action fixture").expect("isolated archive");
    StagedUpdate {
        release: test_release(panel.current_target.as_deref().expect("test platform")),
        digest: hex_digest(&Sha256::digest(b"owned staged-action fixture")),
        archive,
        payload: root.join("payload"),
        cleanup: Some(StageCleanup {
            root,
            armed: true,
            executor: panel.runtime.handle().clone(),
        }),
    }
}

fn pending_check(panel: &mut UpdatePanel, cx: &mut Context<UpdatePanel>) -> Arc<AtomicBool> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let (owner, completion) = worker::spawn(
        &panel.runtime,
        cx.background_executor().clone(),
        cancelled.clone(),
        std::future::pending::<()>(),
    );
    drop(completion);
    panel.request = Some(InFlight {
        identity: panel.identity(panel.current_target.clone().expect("test target"), None),
        origin: RequestOrigin::Automatic,
        _worker: owner,
    });
    panel.state = PanelState::Checking;
    cancelled
}

fn activate_painted(window: &mut Window, position: Point<gpui_kit::Pixels>, cx: &mut App) {
    for input in [
        gpui_kit::MouseDownEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        gpui_kit::MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(input, cx);
    }
}

#[gpui_kit::test]
fn painted_return_cannot_move_a_retained_stage_into_an_active_check(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            let old = staged(panel, parent.path(), "old");
            panel.state = PanelState::Available(old.release.clone());
            panel.retained_stage = Some(old);
        });
        window.render_frame(cx);
        let position = window.find("keep-ready-update").bounds().center();
        let cancelled = panel.update(cx, pending_check);
        activate_painted(window, position, cx);
        panel.update(cx, |panel, cx| {
            assert!(matches!(panel.state, PanelState::Checking));
            assert!(panel.request.is_some());
            assert!(panel.retained_stage.is_some());
            assert!(!cancelled.load(Ordering::Acquire));
            panel.cancel(cx);
            assert!(matches!(panel.state, PanelState::Ready(_)));
            panel.cancel(cx);
        });
    })
    .expect("old return control cannot change a newer request");
}

#[gpui_kit::test]
fn painted_return_cannot_select_a_replaced_retained_package(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            let old = staged(panel, parent.path(), "old");
            panel.state = PanelState::Available(old.release.clone());
            panel.retained_stage = Some(old);
        });
        window.render_frame(cx);
        let position = window.find("keep-ready-update").bounds().center();
        panel.update(cx, |panel, _| {
            panel.retained_stage = Some(staged(panel, parent.path(), "replacement"));
        });
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            assert!(matches!(panel.state, PanelState::Available(_)));
            assert_eq!(
                panel
                    .retained_stage
                    .as_ref()
                    .expect("replacement retained")
                    .archive,
                parent.path().join("replacement/archive.tar.gz")
            );
        });
    })
    .expect("return must refer to the package painted for review");
}

#[gpui_kit::test]
fn painted_cancel_cannot_discard_a_package_after_its_request_completed(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        let cancelled = panel.update(cx, pending_check);
        window.render_frame(cx);
        let position = window.find("cancel-update-request").bounds().center();
        panel.update(cx, |panel, _| {
            panel.cancel_inflight();
            panel.state = PanelState::Ready(staged(panel, parent.path(), "completed"));
        });
        assert!(cancelled.load(Ordering::Acquire));
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            let PanelState::Ready(stage) = &panel.state else {
                panic!("old Cancel must not become Discard after completion");
            };
            assert_eq!(
                stage.archive,
                parent.path().join("completed/archive.tar.gz")
            );
            assert!(stage.archive.exists());
        });
    })
    .expect("cancel intent remains tied to its completed request");
}

#[gpui_kit::test]
fn painted_discard_cannot_cancel_a_new_request(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Ready(staged(panel, parent.path(), "old"));
        });
        window.render_frame(cx);
        let position = window.find("cancel-update-request").bounds().center();
        let cancelled = panel.update(cx, |panel, cx| {
            panel.retain_ready();
            pending_check(panel, cx)
        });
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            assert!(matches!(panel.state, PanelState::Checking));
            assert!(panel.request.is_some());
            assert!(panel.retained_stage.is_some());
            assert!(!cancelled.load(Ordering::Acquire));
        });
    })
    .expect("old Discard must not become Cancel for a new owner");
}

#[gpui_kit::test]
fn painted_download_cannot_choose_an_unreviewed_release(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Available(test_release(
                panel.current_target.as_deref().expect("test target"),
            ));
        });
        window.render_frame(cx);
        let position = window.find("download-update").bounds().center();
        panel.update(cx, |panel, _| {
            let PanelState::Available(release) = &mut panel.state else {
                panic!("available release");
            };
            release.tag = "v100.0.0".into();
        });
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            assert!(
                matches!(&panel.state, PanelState::Available(release) if release.tag == "v100.0.0")
            );
            assert!(panel.request.is_none());
        });
    })
    .expect("download uses the release displayed when the control was painted");
}

#[gpui_kit::test]
fn painted_install_cannot_authorize_a_different_stage_of_the_same_release(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Ready(staged(panel, parent.path(), "old"));
        });
        window.render_frame(cx);
        let position = window.find("install-update").bounds().center();
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Ready(staged(panel, parent.path(), "replacement"));
            // If the old handler enters install, fail before current_exe,
            // helper creation, restart or any installation-root access on all OSes.
            panel.current_target = None;
        });
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            let PanelState::Ready(stage) = &panel.state else {
                panic!("old Install must not authorize a replacement package");
            };
            assert_eq!(
                stage.archive,
                parent.path().join("replacement/archive.tar.gz")
            );
            assert!(stage.archive.exists());
            assert!(panel.request.is_none());
        });
    })
    .expect("install intent binds the exact displayed stage; no helper runs");
}

#[gpui_kit::test]
fn painted_cancel_cannot_cancel_a_replacement_request_owner(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    cx.update_window(window, |_, window, cx| {
        let old_cancelled = panel.update(cx, pending_check);
        window.render_frame(cx);
        let position = window.find("cancel-update-request").bounds().center();
        let new_cancelled = panel.update(cx, pending_check);
        let new_id = panel
            .read(cx)
            .request
            .as_ref()
            .expect("replacement request")
            .identity
            .id;
        assert!(old_cancelled.load(Ordering::Acquire));
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            assert!(matches!(panel.state, PanelState::Checking));
            assert_eq!(
                panel
                    .request
                    .as_ref()
                    .expect("replacement kept")
                    .identity
                    .id,
                new_id
            );
            assert!(!new_cancelled.load(Ordering::Acquire));
        });
    })
    .expect("cancel intent cannot transfer to a replacement request");
}

#[gpui_kit::test]
fn painted_discard_cannot_discard_a_replacement_stage(cx: &mut TestAppContext) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Ready(staged(panel, parent.path(), "old"));
        });
        window.render_frame(cx);
        let position = window.find("cancel-update-request").bounds().center();
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Ready(staged(panel, parent.path(), "replacement"));
        });
        activate_painted(window, position, cx);
        panel.read_with(cx, |panel, _| {
            let PanelState::Ready(stage) = &panel.state else {
                panic!("old Discard must not discard a replacement stage");
            };
            assert_eq!(
                stage.archive,
                parent.path().join("replacement/archive.tar.gz")
            );
            assert!(stage.archive.exists());
        });
    })
    .expect("discard intent remains tied to the displayed stage");
}

#[gpui_kit::test]
fn fresh_download_return_and_install_controls_still_perform_the_reviewed_action(
    cx: &mut TestAppContext,
) {
    let (window, panel) = mount_panel(cx, UpdatePreferences::default());
    let parent = tempfile::tempdir().expect("stage parent");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, _| {
            panel.state = PanelState::Available(test_release(
                panel.current_target.as_deref().expect("test platform"),
            ));
        });
        window.render_frame(cx);
        activate_painted(window, window.find("download-update").bounds().center(), cx);
        panel.update(cx, |panel, cx| {
            assert!(matches!(panel.state, PanelState::Downloading(_)));
            assert!(
                panel
                    .request
                    .as_ref()
                    .expect("reviewed download")
                    .identity
                    .release
                    .is_some()
            );
            panel.cancel(cx);
            let old = staged(panel, parent.path(), "reviewed");
            panel.state = PanelState::Available(old.release.clone());
            panel.retained_stage = Some(old);
        });
        window.render_frame(cx);
        activate_painted(
            window,
            window.find("keep-ready-update").bounds().center(),
            cx,
        );
        panel.update(cx, |panel, _| {
            assert!(matches!(panel.state, PanelState::Ready(_)));
            assert!(panel.retained_stage.is_none());
            // Exercise entry for the correct displayed stage while ensuring
            // installation fails before touching any OS installation root.
            panel.current_target = None;
        });
        window.render_frame(cx);
        activate_painted(window, window.find("install-update").bounds().center(), cx);
        panel.read_with(cx, |panel, _| {
            assert!(matches!(panel.state, PanelState::Failed));
            assert!(panel.request.is_none());
        });
    })
    .expect("fresh review controls remain usable; no helper or installation runs");
}
