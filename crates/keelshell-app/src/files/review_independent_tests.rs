//! Independent counterexamples for the minimum workspace with its assistant visible.
use super::{Checked, CheckedOption, Harness, mount_layout_scene};
use gpui_kit::{AppContext, InputEvent, TestAppContext, point, px, test::TestWindowExt};
use keelshell_core::{Language, Theme};

#[gpui_kit::test]
async fn independent_minimum_assistant_review_keeps_literals_and_async_review_restarts_at_top(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with(cx, |cx, session, runtime| {
        mount_layout_scene(cx, session, runtime, 900., 580., true)
    });
    h.idle(cx).await;
    let literal_name = format!("审核-$(touch untouched);'-{}.txt", "x".repeat(110));
    h.source(&literal_name, b"literal source bytes");
    h.seed("/target-only.txt", b"preserve target bytes");
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("independent seed SFTP");
        sftp.mkdir("/empty-target")
            .await
            .checked("seed empty target");
        sftp.close().await.checked("close seed SFTP");
    });
    h.local_input(cx, &h.local.0);
    cx.update_window(h.window, |_, window, cx| {
        crate::i18n::set_language(Language::En, cx);
        crate::design::apply(Theme::Dark, Some(window), cx);
    })
    .checked("minimum English dark assistant review");
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-remote");
    h.idle(cx).await;
    h.click(cx, "review-directory-sync");
    let original = h.panel.read_with(cx, |p, cx| {
        let pending = p
            .pending
            .as_ref()
            .checked_option("independent mirror review");
        format!("{} · {}", p.host, pending.0.render(cx))
    });
    assert!(original.contains(&literal_name));
    assert!(original.contains("irreversible") || original.contains("cannot be undone"));
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let body = window.find("file-confirmation-message").bounds();
        let area = window.find("files-layout-scene").bounds();
        assert!(body.size.height <= px(72.));
        for (index, text) in original.split('\n').enumerate() {
            assert_eq!(window.find(("file-confirmation-line", index)).label(), Some(text));
        }
        let position = point(body.origin.x + px(8.), body.center().y);
        window.dispatch_event(gpui_kit::MouseMoveEvent { position, ..Default::default() }.to_platform_input(), cx);
        window.dispatch_event(gpui_kit::ScrollWheelEvent { position, delta: gpui_kit::ScrollDelta::Pixels(point(px(-100_000.), px(0.))), ..Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        window.dispatch_event(
            gpui_kit::ScrollWheelEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-100_000.))),
                ..Default::default()
            }.to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        let offset = h.panel.read(cx).confirmation_scroll.offset();
        assert!(offset.x < px(-100.) && offset.y < px(-100.));
        for id in ["confirm-file-operation", "cancel-file-operation"] {
            let action = window.find(id);
            assert!(action.visible() && action.bounds().origin.x >= body.right());
            assert!(action.bounds().right() <= area.right() && action.bounds().bottom() <= area.bottom());
        }
        window.click("cancel-file-operation", cx);
        println!("INDEPENDENT_MINIMUM_ASSISTANT_JSON {}", serde_json::json!({"width":900,"height":580,"assistant":true,"language":"En","theme":"Dark","offset_x":f32::from(offset.x),"offset_y":f32::from(offset.y),"rows":original.split('\n').count()}));
    }).checked("minimum assistant coordinate wheel and fixed Cancel");
    assert_eq!(h.read("/target-only.txt"), b"preserve target bytes");
    h.missing(&format!("/{literal_name}"));
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("read canceled namespace");
        assert!(
            sftp.list("/")
                .await
                .checked("read canceled root")
                .iter()
                .any(|e| e.name == "empty-target" && e.is_directory)
        );
        sftp.close().await.checked("close canceled namespace");
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    let next_directory = h.local.0.join("next-literal-$(touch unchanged);中文");
    std::fs::create_dir(&next_directory).checked("independent next directory");
    std::fs::write(next_directory.join("data.txt"), b"next review only")
        .checked("next source file");
    h.local_input(cx, &next_directory);
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
                    .is_some_and(|label| label.contains("next-literal-$(touch unchanged);中文"))
        );
        window.click("cancel-file-operation", cx);
    })
    .checked("a different asynchronous plan starts at original source/root");
    h.missing("/next-literal-$(touch unchanged);中文/data.txt");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.panel.read_with(cx, |p, _| {
        assert!(p.pending.is_none() && p.operation_id.is_none() && p.sync_journal.is_none())
    });
}
