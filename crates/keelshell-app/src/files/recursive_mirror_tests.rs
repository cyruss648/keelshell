//! Production GPUI controls with exact isolated SFTP outcomes, not native acceptance.
use super::{Checked, CheckedOption, Harness};
use crate::files::sync::journal::StepState;
use gpui_kit::{
    AppContext, ScrollDelta, TestAppContext, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{DirectorySyncOperation, Language, Theme};
use std::time::Duration;

async fn plan(h: &Harness, cx: &mut TestAppContext, upload: bool) {
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(
        cx,
        if upload {
            "plan-mirror-to-remote"
        } else {
            "plan-mirror-to-local"
        },
    );
    h.idle(cx).await;
}
fn remote_tree(h: &Harness) {
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("seed recursive tree");
        s.mkdir("/old-tree").await.checked("old root");
        s.mkdir("/old-tree/deep").await.checked("old deep");
        s.write("/old-tree/deep/a", b"reviewed")
            .await
            .checked("old a");
        s.write("/old-tree/deep/b", b"reviewed")
            .await
            .checked("old b");
        s.close().await.checked("close seed");
    });
}
fn local_tree(h: &Harness) {
    std::fs::create_dir_all(h.local.0.join("old-tree/deep")).checked("isolated fixture setup only");
    h.source("old-tree/deep/a", b"reviewed");
    h.source("old-tree/deep/b", b"reviewed");
}
fn journal(h: &Harness, cx: &TestAppContext) -> Vec<(String, StepState)> {
    h.panel.read_with(cx, |panel, _| {
        panel
            .sync_journal
            .as_ref()
            .checked_option("recursive journal")
            .lock()
            .checked("journal lock")
            .steps
            .iter()
            .map(|step| (step.path.clone(), step.state))
            .collect()
    })
}
#[gpui_kit::test]
async fn both_direction_recursive_review_cancel_and_confirmed_copy_delete_use_exact_rows(
    cx: &mut TestAppContext,
) {
    for upload in [true, false] {
        let h = Harness::new(cx);
        h.idle(cx).await;
        if upload {
            remote_tree(&h);
            std::fs::create_dir_all(h.local.0.join("new-tree/deep"))
                .checked("fixture new hierarchy");
            h.source("new-tree/deep/file", b"copied");
        } else {
            local_tree(&h);
            h.runtime.block_on(async {
                let s = h.session.sftp().await.checked("source hierarchy");
                s.mkdir("/new-tree").await.checked("new root");
                s.mkdir("/new-tree/deep").await.checked("new deep");
                s.write("/new-tree/deep/file", b"copied")
                    .await
                    .checked("new source");
                s.close().await.checked("close");
            });
        }
        plan(&h, cx, upload).await;
        h.click(cx, "review-directory-sync");
        h.panel.read_with(cx, |panel, cx| {
            let pending = panel
                .pending
                .as_ref()
                .checked_option("full recursive review");
            let message = pending.0.render(cx);
            assert!(
                message.contains("old-tree/deep/a")
                    && message.contains("old-tree/deep/b")
                    && message.contains("new-tree/deep/file")
            );
            let crate::files::Operation::ApplyDirectorySync(review, _) = &pending.1 else {
                panic!("actual reviewed mirror");
            };
            let deletes: Vec<_> = review
                .plan
                .operations()
                .iter()
                .filter_map(|op| match op {
                    DirectorySyncOperation::Delete { path, .. } => Some(path.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                deletes,
                [
                    "old-tree/deep/a",
                    "old-tree/deep/b",
                    "old-tree/deep",
                    "old-tree"
                ]
            );
        });
        h.click(cx, "cancel-file-operation");
        if upload {
            assert_eq!(h.read("/old-tree/deep/a"), b"reviewed");
        } else {
            assert_eq!(
                std::fs::read(h.local.0.join("old-tree/deep/a")).checked("retained local"),
                b"reviewed"
            );
        }
        h.click(cx, "review-directory-sync");
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        if upload {
            assert_eq!(h.read("/new-tree/deep/file"), b"copied");
            h.runtime.block_on(async {
                let s = h.session.sftp().await.checked("verify old root absent");
                assert!(
                    s.inspect_entry("/old-tree")
                        .await
                        .checked("inspect old root")
                        .is_none()
                );
                s.close().await.checked("close");
            });
        } else {
            assert!(!h.local.0.join("old-tree").exists());
            assert_eq!(
                std::fs::read(h.local.0.join("new-tree/deep/file")).checked("local exact copy"),
                b"copied"
            );
        }
        let steps = journal(&h, cx);
        assert_eq!(steps.len(), 7);
        assert!(
            steps
                .iter()
                .all(|(_, state)| *state == StepState::Completed),
            "{steps:?}"
        );
    }
}
#[gpui_kit::test]
async fn new_subtree_node_after_review_refuses_whole_execution_without_any_delete(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    remote_tree(&h);
    plan(&h, cx, true).await;
    h.click(cx, "review-directory-sync");
    h.server
        .filesystem
        .replace_external_file("/old-tree/late", b"keep new node")
        .checked("external tree addition");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/old-tree/deep/a"), b"reviewed");
    assert_eq!(h.read("/old-tree/deep/b"), b"reviewed");
    assert_eq!(h.read("/old-tree/late"), b"keep new node");
    assert!(
        journal(&h, cx)
            .iter()
            .all(|(_, state)| *state == StepState::SkippedAfterFailure)
    );
}
#[gpui_kit::test]
async fn partial_recursive_cancel_retains_completed_unknown_and_unstarted_parent_states(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    remote_tree(&h);
    plan(&h, cx, true).await;
    h.click(cx, "review-directory-sync");
    let hold = h
        .server
        .filesystem
        .hold_remove_path("/old-tree/deep/b")
        .checked("second REMOVE owned hold");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| hold.entered() > 0)
        .await;
    assert!(!hold.expired());
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    hold.release();
    let steps = journal(&h, cx);
    assert_eq!(
        steps,
        vec![
            ("old-tree/deep/a".into(), StepState::Completed),
            ("old-tree/deep/b".into(), StepState::Unknown),
            ("old-tree/deep".into(), StepState::CancelledBeforeWrite),
            ("old-tree".into(), StepState::CancelledBeforeWrite)
        ]
    );
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("inspect isolation");
        assert!(
            s.inspect_entry("/old-tree")
                .await
                .checked("root still present")
                .is_some()
        );
        assert!(matches!(
            s.remove("/old-tree/deep").await,
            Err(keelshell_session::SessionError::MutationQuarantined)
        ));
        s.close().await.checked("close");
    });
}
#[gpui_kit::test]
async fn complete_long_recursive_plan_can_scroll_while_approval_controls_fit_six_locale_theme_combinations(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("long tree");
        s.mkdir("/tree").await.checked("root");
        s.mkdir("/tree/deep").await.checked("deep");
        for index in 0..110 {
            s.write(
                &format!("/tree/deep/reviewed-long-file-{index:03}"),
                b"reviewed",
            )
            .await
            .checked("file");
        }
        s.close().await.checked("close");
    });
    plan(&h, cx, true).await;
    for language in [Language::ZhCn, Language::En] {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.resize(size(px(480.), px(440.)));
                window.bounds_changed(cx);
                window.render_frame(cx);
            })
            .checked("minimum themed window");
            h.click(cx, "review-directory-sync");
            h.panel.read_with(cx, |panel, cx| {
                let message = panel
                    .pending
                    .as_ref()
                    .checked_option("full long review")
                    .0
                    .render(cx);
                assert!(
                    message.contains("reviewed-long-file-000")
                        && message.contains("reviewed-long-file-109")
                );
                assert!(message.contains("112"));
            });
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let bounds = window.find("file-confirmation-message").bounds();
                // The current two-axis presenter reserves a 72 px viewport;
                // approval controls and the clipped body remain in the panel.
                assert!(bounds.size.height <= px(72.));
                let bar = window.find("file-confirmation-bar").bounds();
                assert!(bounds.origin.x >= bar.origin.x && bounds.origin.y >= bar.origin.y);
                assert!(bounds.right() <= bar.right() && bounds.bottom() <= bar.bottom());
                window.scroll(
                    "file-confirmation-message",
                    ScrollDelta::Pixels(point(px(-20000.), px(-50000.))),
                    cx,
                );
                window.render_frame(cx);
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let button = window.find(id);
                    assert!(
                        button.visible()
                            && button.bounds().right() <= window.bounds().right()
                            && button.bounds().bottom() <= window.bounds().bottom()
                    );
                }
            })
            .checked("real wheel scrolling preserves visible review actions");
            h.click(cx, "cancel-file-operation");
        }
    }
    assert_eq!(h.read("/tree/deep/reviewed-long-file-109"), b"reviewed");
}
