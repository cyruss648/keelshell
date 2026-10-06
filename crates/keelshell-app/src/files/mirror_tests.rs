//! Native-protocol fixture behavior through actual headless production controls.
use super::{Checked, Harness, reveal_file_control};
use crate::files::sync::journal::StepState;
use gpui_kit::{
    AppContext, TestAppContext, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{Language, Theme, hash_directory_content};
use std::time::Duration;

async fn plan(h: &Harness, cx: &mut TestAppContext, remote: bool) {
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(
        cx,
        if remote {
            "plan-mirror-to-remote"
        } else {
            "plan-mirror-to-local"
        },
    );
    h.idle(cx).await;
}
fn states(h: &Harness, cx: &TestAppContext) -> Vec<(String, StepState)> {
    h.panel.read_with(cx, |panel, _| {
        panel
            .sync_journal
            .as_ref()
            .unwrap_or_else(|| panic!("actual execution journal"))
            .lock()
            .unwrap_or_else(|error| panic!("journal lock: {error}"))
            .steps
            .iter()
            .map(|s| (s.path.clone(), s.state))
            .collect()
    })
}
#[gpui_kit::test]
async fn mirror_upload_review_cancellation_and_exact_confirmed_deletions(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("new", b"new");
    h.seed("/old", b"old");
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("seed");
        s.mkdir("/empty").await.checked("empty");
        s.close().await.checked("close seed");
    });
    plan(&h, cx, true).await;
    h.click(cx, "review-directory-sync");
    let hash: String = hash_directory_content(b"old")
        .checked("hash")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    h.panel.read_with(cx, |p, cx| {
        let message = p
            .pending
            .as_ref()
            .unwrap_or_else(|| panic!("review"))
            .0
            .render(cx);
        assert!(message.contains(&hash) && message.contains("old") && message.contains("empty"));
    });
    assert_eq!(h.read("/old"), b"old");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "cancel-file-operation");
    assert_eq!(h.read("/old"), b"old");
    h.click(cx, "review-directory-sync");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.missing("/old");
    h.missing("/empty");
    assert_eq!(h.read("/new"), b"new");
    let rows = states(&h, cx);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|(_, state)| *state == StepState::Completed));
}
#[gpui_kit::test]
async fn mirror_download_deletes_local_file_and_empty_directory_only_after_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let old = h.source("old", b"old");
    std::fs::create_dir(h.local.0.join("empty")).checked("empty");
    h.seed("/new", b"new");
    plan(&h, cx, false).await;
    assert!(old.exists());
    h.click(cx, "review-directory-sync");
    assert!(old.exists());
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert!(!old.exists() && !h.local.0.join("empty").exists());
    assert_eq!(
        std::fs::read(h.local.0.join("new")).checked("copied"),
        b"new"
    );
    assert!(
        states(&h, cx)
            .iter()
            .all(|(_, s)| *s == StepState::Completed)
    );
}
#[gpui_kit::test]
async fn mirror_lists_all_link_conflicts_without_discarding_safe_nonempty_subtrees(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("seed");
        s.mkdir("/tree").await.checked("tree");
        s.write("/tree/child", b"keep").await.checked("child");
        s.close().await.checked("close");
    });
    h.server
        .filesystem
        .insert_symlink("/link-a")
        .checked("link");
    h.server
        .filesystem
        .insert_symlink("/link-z")
        .checked("link");
    plan(&h, cx, true).await;
    h.panel.read_with(cx, |p, _| {
        assert!(p.pending.is_none());
        assert!(p.comparison.as_ref().is_none_or(|c| c.sync_plan.is_none()));
        let paths: Vec<_> = p.mirror_conflicts.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"link-a") && paths.contains(&"link-z") && !paths.contains(&"tree"));
    });
    assert_eq!(h.read("/tree/child"), b"keep");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("directory-mirror-conflicts").is_some());
        assert!(window.try_find("confirm-file-operation").is_none());
    })
    .checked("read-only conflict view");
}
#[gpui_kit::test]
async fn mirror_rejects_same_size_target_change_and_new_source_without_any_mutation(
    cx: &mut TestAppContext,
) {
    for source_change in [false, true] {
        let h = Harness::new(cx);
        h.idle(cx).await;
        h.seed("/delete", b"old");
        plan(&h, cx, true).await;
        h.click(cx, "review-directory-sync");
        if source_change {
            h.source("delete", b"new");
        } else {
            h.server
                .filesystem
                .replace_external_file("/delete", b"new")
                .checked("external change");
        }
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(
            h.read("/delete"),
            if source_change { b"old" } else { b"new" }
        );
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
        assert!(
            states(&h, cx)
                .iter()
                .all(|(_, s)| *s == StepState::SkippedAfterFailure)
        );
    }
}
#[gpui_kit::test]
async fn cancellation_preserves_confirmed_copy_unknown_remove_and_unstarted_items(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("a-copy", b"copy");
    h.seed("/b-delete", b"old");
    h.seed("/c-unstarted", b"keep");
    plan(&h, cx, true).await;
    h.click(cx, "review-directory-sync");
    let hold = h
        .server
        .filesystem
        .hold_remove_path("/b-delete")
        .checked("owned remove hold");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| hold.entered() > 0)
        .await;
    assert!(!hold.expired());
    assert_eq!(h.read("/a-copy"), b"copy");
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    hold.release();
    assert_eq!(
        states(&h, cx),
        vec![
            ("a-copy".into(), StepState::Completed),
            ("b-delete".into(), StepState::Unknown),
            ("c-unstarted".into(), StepState::CancelledBeforeWrite)
        ]
    );
    assert_eq!(h.read("/c-unstarted"), b"keep");
    h.runtime.block_on(async {
        let s = h.session.sftp().await.checked("inspect isolated");
        assert!(matches!(
            s.remove("/b-delete").await,
            Err(keelshell_session::SessionError::MutationQuarantined)
        ));
        s.close().await.checked("close isolated inspection");
    });
}
#[gpui_kit::test]
async fn mirror_controls_and_full_digest_confirmation_fit_both_languages_and_three_themes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/long-target-name", b"old");
    plan(&h, cx, true).await;
    for language in [Language::ZhCn, Language::En] {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.resize(size(px(480.), px(440.)));
                window.bounds_changed(cx);
                window.render_frame(cx);
                for id in [
                    "plan-mirror-to-remote",
                    "plan-mirror-to-local",
                    "review-directory-sync",
                ] {
                    reveal_file_control(window, cx, id);
                    let b = window.find(id);
                    assert!(
                        b.visible()
                            && b.bounds().right() <= window.bounds().right()
                            && b.bounds().bottom() <= window.bounds().bottom()
                    );
                }
            })
            .checked("bilingual themed mirror controls");
            h.click(cx, "review-directory-sync");
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let b = window.find(id);
                    assert!(
                        b.visible()
                            && b.bounds().right() <= window.bounds().right()
                            && b.bounds().bottom() <= window.bounds().bottom()
                    );
                }
                let label = window
                    .find("confirm-file-operation")
                    .label()
                    .unwrap_or_else(|| panic!("mirror label"))
                    .to_owned();
                assert!(label.contains("镜像") || label.contains("mirror"));
            })
            .checked("fixed explicit mirror approval");
            h.click(cx, "cancel-file-operation");
        }
    }
    assert_eq!(h.read("/long-target-name"), b"old");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
