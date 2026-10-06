//! Independent combined-presenter checks over actual controlled recursive plans.
//! Test-platform rendering and loopback SFTP do not establish native acceptance.
use super::{Checked, CheckedOption, Harness, mount_layout_scene};
use crate::files::sync::journal::StepState;
use gpui_kit::{
    App, AppContext, Bounds, InputEvent, Pixels, Point, TestAppContext, Window, point, px,
    test::TestWindowExt,
};
use keelshell_core::{Language, Theme};
use sha2::{Digest, Sha256};

fn wheel(window: &mut Window, cx: &mut App, body: Bounds<Pixels>, delta: Point<Pixels>) {
    let position = point(body.origin.x + px(8.), body.center().y);
    window.dispatch_event(
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    // GPUI's default nonconcurrent scrolling keeps only the dominant axis
    // of a diagonal event. Exercise each axis with its own platform event.
    for delta in [point(delta.x, px(0.)), point(px(0.), delta.y)] {
        if delta == point(px(0.), px(0.)) {
            continue;
        }
        window.dispatch_event(
            gpui_kit::ScrollWheelEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(delta),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
}

fn seed_long_tree(h: &Harness) -> Vec<String> {
    let names = (0..110)
        .map(|index| {
            format!(
                "file-{index:03}-$(touch marker);'-{}-中文.dat",
                "x".repeat(110)
            )
        })
        .collect::<Vec<_>>();
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("combined tree seed");
        sftp.mkdir("/tree").await.checked("seed tree");
        sftp.mkdir("/tree/deep").await.checked("seed deep tree");
        for name in &names {
            sftp.write(&format!("/tree/deep/{name}"), b"reviewed literal bytes")
                .await
                .checked("seed actual review bytes");
        }
        sftp.close().await.checked("close combined seed");
    });
    names
}

fn assert_tree_retained(h: &Harness, names: &[String], late: bool) {
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("combined retained tree");
        let entries = sftp
            .list("/tree/deep")
            .await
            .checked("complete retained namespace");
        assert_eq!(entries.len(), names.len() + usize::from(late));
        for name in names {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.name == *name && !entry.is_directory)
            );
            assert_eq!(
                sftp.read(&format!("/tree/deep/{name}"), 1024)
                    .await
                    .checked("all canceled or refused bytes retained"),
                b"reviewed literal bytes"
            );
        }
        if late {
            assert_eq!(
                sftp.read("/tree/deep/late", 1024)
                    .await
                    .checked("late node retained"),
                b"external late node"
            );
        }
        sftp.close().await.checked("close complete readback");
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

async fn plan(h: &Harness, cx: &mut TestAppContext) {
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-remote");
    h.idle(cx).await;
    assert_eq!(
        h.panel.read_with(cx, |p, _| p
            .comparison
            .as_ref()
            .checked_option("actual recursive comparison")
            .sync_plan
            .as_ref()
            .checked_option("actual recursive plan")
            .operation_count()),
        112
    );
}

#[gpui_kit::test]
async fn combined_recursive_112_actions_are_readable_by_coordinate_wheel_cancel_and_async_reset(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let names = seed_long_tree(&h);
    let expected_hash = Sha256::digest(b"reviewed literal bytes")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    plan(&h, cx).await;
    for language in [Language::ZhCn, Language::En] {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
            })
            .checked("combined actual locale and theme");
            h.click(cx, "review-directory-sync");
            let lines = h.panel.read_with(cx, |p, cx| {
                format!(
                    "{} · {}",
                    p.host,
                    p.pending
                        .as_ref()
                        .checked_option("112-action review")
                        .0
                        .render(cx)
                )
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<_>>()
            });
            assert!(lines.iter().any(|line| line.contains(&names[0])));
            assert!(lines.iter().any(|line| line.contains(&names[109])));
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                let body = window.find("file-confirmation-message").bounds();
                let area = window.find("files-layout-scene").bounds();
                assert!(body.size.height <= px(72.) && body.origin.x >= area.origin.x && body.bottom() <= area.bottom());
                assert_eq!(h.panel.read(cx).confirmation_scroll.offset(), point(px(0.), px(0.)));
                for (index, text) in lines.iter().enumerate() {
                    let row = window.find(("file-confirmation-line", index));
                    assert_eq!(row.label(), Some(text.as_str()));
                    assert_eq!(row.role(), Some(gpui_kit::accesskit::Role::Label));
                }
                let first = window.find(("file-confirmation-line", 0_usize));
                assert!(first.visible());
                let tail_index = lines.len() - 1;
                assert!(!window.find(("file-confirmation-line", tail_index)).visible());
                let maximum = h.panel.read(cx).confirmation_scroll.max_offset();
                assert!(maximum.x > px(100.) && maximum.y > px(1000.));
                wheel(window, cx, body, point(px(0.), px(-100_000.)));
                let tail = window.find(("file-confirmation-line", tail_index));
                assert!(tail.visible() && tail.bounds().origin.y >= body.origin.y && tail.bounds().bottom() <= body.bottom());
                let path_index = lines.iter().rposition(|line| line.contains(&names[109])).checked_option("literal last leaf row");
                let hash_index = lines.iter().rposition(|line| line.contains("SHA-256") && line.contains(&expected_hash)).checked_option("actual final file content hash row, excluding directory None rows");
                for index in [path_index, hash_index] {
                    let row = window.find(("file-confirmation-line", index));
                    wheel(window, cx, body, point(px(100_000.), body.center().y - row.bounds().center().y));
                    let row = window.find(("file-confirmation-line", index));
                    assert!(row.visible() && row.bounds().origin.y >= body.origin.y && row.bounds().bottom() <= body.bottom());
                    let delta = body.right() - row.bounds().right();
                    wheel(window, cx, body, point(delta, px(0.)));
                    let row = window.find(("file-confirmation-line", index));
                    assert!(row.visible() && row.bounds().right() <= body.right() + px(1.) && row.bounds().right() > body.origin.x);
                    assert_eq!(row.label(), Some(lines[index].as_str()));
                }
                let after = h.panel.read(cx).confirmation_scroll.offset();
                h.panel.update(cx, |_, cx| cx.notify());
                window.render_frame(cx);
                assert_eq!(h.panel.read(cx).confirmation_scroll.offset(), after);
                // Reversing coordinate wheels must expose the original root notice.
                wheel(window, cx, body, point(px(100_000.), px(100_000.)));
                assert_eq!(h.panel.read(cx).confirmation_scroll.offset(), point(px(0.), px(0.)));
                assert!(window.find(("file-confirmation-line", 0_usize)).visible());
                wheel(window, cx, body, point(px(-100_000.), px(-100_000.)));
                assert!(h.panel.read(cx).confirmation_scroll.offset().y < px(-1000.));
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let action = window.find(id);
                    assert!(action.visible() && action.bounds().origin.x >= body.right());
                    assert!(action.bounds().right() <= area.right() && action.bounds().bottom() <= area.bottom());
                }
                println!("COMBINED_RECURSIVE_WHEEL_JSON {}", serde_json::json!({"operations":112,"rows":lines.len(),"width":900,"height":580,"assistant":true,"language":format!("{language:?}"),"theme":format!("{theme:?}"),"maximum_x":f32::from(maximum.x),"maximum_y":f32::from(maximum.y),"path_row":path_index,"hash_row":hash_index,"last_original_hash":lines[hash_index],"coordinate_wheel_tail_and_suffix":true}));
                window.click("cancel-file-operation", cx);
            }).checked("complete recursive plan original rows and real reversible coordinate wheels");
            assert_tree_retained(&h, &names, false);
            h.panel.read_with(cx, |p, _| {
                assert!(p.pending.is_none() && p.operation_id.is_none() && p.sync_journal.is_none())
            });
        }
    }
    let next = h.local.0.join("next-$(touch untouched);中文");
    std::fs::create_dir(&next).checked("independent next asynchronous source");
    std::fs::write(next.join("data.txt"), b"next review only").checked("next source bytes");
    h.local_input(cx, &next);
    h.click(cx, "upload-directory");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            h.panel.read(cx).confirmation_scroll.offset(),
            point(px(0.), px(0.))
        );
        let first = window.find(("file-confirmation-line", 0_usize));
        assert!(
            first.visible()
                && first
                    .label()
                    .is_some_and(|label| label.contains("next-$(touch untouched);中文"))
        );
        window.click("cancel-file-operation", cx);
    })
    .checked("different asynchronous review resets previously scrolled recursive evidence");
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("verify asynchronous cancel target");
        assert!(
            sftp.inspect_entry("/next-$(touch untouched);中文")
                .await
                .checked("new target absent")
                .is_none()
        );
        sftp.close()
            .await
            .checked("close asynchronous cancel readback");
    });
    assert_tree_retained(&h, &names, false);
    h.panel.read_with(cx, |p, _| {
        assert!(p.pending.is_none() && p.operation_id.is_none() && p.sync_journal.is_none())
    });
}

#[gpui_kit::test]
async fn combined_scrolled_recursive_112_action_review_refuses_new_descendant_before_any_delete(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let names = seed_long_tree(&h);
    plan(&h, cx).await;
    h.click(cx, "review-directory-sync");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let body = window.find("file-confirmation-message").bounds();
        wheel(window, cx, body, point(px(-100_000.), px(-100_000.)));
        assert!(h.panel.read(cx).confirmation_scroll.offset().y < px(-1000.));
        assert!(window.find("confirm-file-operation").visible());
    })
    .checked("scrolled original recursive approval");
    h.server
        .filesystem
        .replace_external_file("/tree/deep/late", b"external late node")
        .checked("external namespace drift after exact review");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |p, _| {
        let journal = p
            .sync_journal
            .as_ref()
            .checked_option("refused recursive journal")
            .lock()
            .checked("journal lock");
        assert_eq!(journal.steps.len(), 112);
        assert!(
            journal
                .steps
                .iter()
                .all(|step| step.state == StepState::SkippedAfterFailure)
        );
    });
    assert_tree_retained(&h, &names, true);
    assert!(!h.local.0.join("marker").exists());
    println!(
        "COMBINED_RECURSIVE_NAMESPACE_JSON {}",
        serde_json::json!({"reviewed_operations":112,"new_descendant":true,"all_steps_skipped":true,"all_110_original_bodies_retained":true,"atomic_writes_started":h.server.filesystem.atomic_writes_started()})
    );
}
