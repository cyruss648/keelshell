//! Production file-review wheel behavior after real controlled SSH/SFTP planning.
use super::{Checked, CheckedOption, Harness, mount_layout_scene};
use gpui_kit::{AppContext, TestAppContext, point, px, test::TestWindowExt};
use keelshell_core::{Language, Theme};

#[gpui_kit::test]
async fn planned_mirror_review_scrolls_through_actual_policy_and_five_operations_without_writes(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, false)
        });
        h.idle(cx).await;
        h.source("changed.txt", b"new!");
        h.source("local-only-a.txt", b"copy-a");
        let last_name = format!("local-only-z-{}.txt", "x".repeat(150));
        h.source(&last_name, b"copy-z");
        h.seed("/changed.txt", b"old!");
        h.seed("/delete-remote.txt", b"keep-until-approved");
        h.runtime.block_on(async {
            let s = h
                .session
                .sftp()
                .await
                .checked("empty remote directory fixture");
            s.mkdir("/delete-empty")
                .await
                .checked("seed empty directory");
            s.close().await.checked("close seeded SFTP");
        });
        h.local_input(cx, &h.local.0);
        h.click(cx, "compare-directories");
        h.idle(cx).await;
        h.click(cx, "plan-mirror-to-remote");
        h.idle(cx).await;
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                cx.update_window(h.window, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                })
                .checked("set the actual panel language and theme");
                h.click(cx, "review-directory-sync");
                let original_lines = h.panel.read_with(cx, |p, cx| {
                    let review = p.pending.as_ref().checked_option("five-operation review");
                    assert_eq!(
                        p.comparison
                            .as_ref()
                            .checked_option("real comparison")
                            .sync_plan
                            .as_ref()
                            .checked_option("real mirror")
                            .operation_count(),
                        5
                    );
                    format!("{} · {}", p.host, review.0.render(cx))
                        .split('\n')
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                });
                cx.update_window(h.window,|_,window,cx| {
                    window.render_frame(cx);
                    let body=window.find("file-confirmation-message").bounds();
                    assert_eq!(h.panel.read(cx).confirmation_scroll.offset(),point(px(0.),px(0.)),"a newly opened approval starts at the complete target/root notice");
                    for (index,line) in original_lines.iter().enumerate() {
                        assert_eq!(window.find(("file-confirmation-line",index)).label(),Some(line.as_str()),"each actual policy, fingerprint, operation and hash line is retained");
                    }
                    let maximum=h.panel.read(cx).confirmation_scroll.max_offset();
                    assert!(maximum.y>px(100.),"actual complete plan requires meaningful vertical extent: {maximum:?}");
                    // A narrow platform monospace font can fit every actual
                    // line at 1440 px. The minimum window must still pan paths;
                    // the 512-character presenter regression checks both sizes.
                    if width==900. { assert!(maximum.x>px(100.),"minimum-window actual plan needs horizontal inspection: {maximum:?}"); }
                    let last_index=original_lines.len()-1;
                    assert!(!window.find(("file-confirmation-line",last_index)).visible(),"the final actual evidence must initially be outside the viewport");
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(px(0.),px(-100000.))),cx);
                    let tail=window.find(("file-confirmation-line",last_index));
                    assert!(tail.visible() && tail.bounds().origin.y>=body.origin.y && tail.bounds().bottom()<=body.bottom(),"final actual SHA row is vertically reachable: {:?} in {body:?}",tail.bounds());
                    let hash_index=original_lines.iter().rposition(|line|line.contains("SHA-256")).checked_option("the actual operation content hash row");
                    let hash=window.find(("file-confirmation-line",hash_index));
                    let vertical=body.center().y-hash.bounds().center().y;
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(px(0.),vertical)),cx);
                    let hash=window.find(("file-confirmation-line",hash_index));
                    assert!(hash.visible() && hash.bounds().origin.y>=body.origin.y && hash.bounds().bottom()<=body.bottom(),"last actual SHA row is vertically reachable: {:?} in {body:?}",hash.bounds());
                    let delta=body.right()-hash.bounds().right();
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(delta,px(0.))),cx);
                    let tail=window.find(("file-confirmation-line",hash_index));
                    assert!(tail.visible() && tail.bounds().right()<=body.right()+px(1.) && tail.bounds().right()>body.origin.x,"final hash suffix is horizontally reachable: {:?} in {body:?}",tail.bounds());
                    let after=h.panel.read(cx).confirmation_scroll.offset();
                    h.panel.update(cx,|_,cx|cx.notify());window.render_frame(cx);
                    assert_eq!(h.panel.read(cx).confirmation_scroll.offset(),after,"production repaint keeps reviewed scroll position");
                    for id in ["confirm-file-operation","cancel-file-operation"] {
                        let button=window.find(id);assert!(button.visible() && button.bounds().origin.x>=body.right() && button.bounds().bottom()<=window.bounds().bottom());
                    }
                    println!("ACTUAL_MIRROR_REVIEW_JSON {}",serde_json::json!({"width":width,"height":height,"language":format!("{language:?}"),"theme":format!("{theme:?}"),"operations":5,"original_rows":original_lines.len(),"max_x":f32::from(maximum.x),"max_y":f32::from(maximum.y),"after_x":f32::from(after.x),"after_y":f32::from(after.y),"last_review_line":original_lines[last_index],"actual_hash_line":original_lines[hash_index],"hash_row":hash_index}));
                    window.click("cancel-file-operation",cx);
                }).checked("complete actual planned review renderer, both wheel axes and visible cancel");
                assert_eq!(h.read("/changed.txt"), b"old!");
                assert_eq!(h.read("/delete-remote.txt"), b"keep-until-approved");
                h.missing("/local-only-a.txt");
                h.missing(&format!("/{last_name}"));
                h.runtime.block_on(async {
                    let sftp = h
                        .session
                        .sftp()
                        .await
                        .checked("read canceled target namespace");
                    let entries = sftp.list("/").await.checked("list canceled target");
                    assert!(
                        entries
                            .iter()
                            .any(|entry| entry.name == "delete-empty" && entry.is_directory),
                        "cancel keeps the target-only empty directory"
                    );
                    sftp.close().await.checked("close canceled target readback");
                });
                assert_eq!(
                    std::fs::read(h.local.0.join("changed.txt")).checked("unchanged local source"),
                    b"new!"
                );
                assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
                h.panel.read_with(cx, |p, _| {
                    assert!(
                        p.pending.is_none() && p.operation_id.is_none() && p.sync_journal.is_none()
                    );
                });
            }
        }
    }
}
