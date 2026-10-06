//! Separate production Files panels share actual transport mutation ownership.
use super::*;
use crate::files::IsolationTarget;

fn click_other(window: AnyWindowHandle, cx: &mut TestAppContext, id: &str) {
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        reveal_file_control(window, cx, id);
        window.click(gpui_kit::SharedString::from(id.to_owned()), cx);
    })
    .checked("real action in the second Files panel");
}
async fn idle_other(window: AnyWindowHandle, panel: &Entity<FilesPanel>, cx: &mut TestAppContext) {
    cx.wait_for(window, Duration::from_secs(7), |_, cx| !panel.read(cx).busy)
        .await;
}
fn open_edit(
    window: AnyWindowHandle,
    panel: &Entity<FilesPanel>,
    path: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected(path, false)), window, cx)
        })
    })
    .checked("open real remote editor baseline");
    // The asynchronous caller waits for the read before setting the draft.
}
fn edit_other(
    window: AnyWindowHandle,
    panel: &Entity<FilesPanel>,
    value: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(window, |_, window, cx| {
        panel
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| editor.set_value(value, window, cx));
        window.render_frame(cx);
    })
    .checked("edit a reviewable local draft");
}

#[gpui_kit::test]
async fn separate_files_save_cannot_bypass_active_or_unknown_target_and_requires_new_review_after_risk_ack(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/shared.txt", b"original shared target");
    h.seed("/independent.txt", b"original independent target");
    let (second_window, second) = mount(cx, h.session.clone(), h.runtime.clone());
    idle_other(second_window, &second, cx).await;
    open_edit(second_window, &second, "/shared.txt", cx);
    idle_other(second_window, &second, cx).await;
    edit_other(second_window, &second, "reviewed replacement", cx);
    let source = h.source("shared.txt", &vec![0x52; 128 * 1024]);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/shared.txt")
        .checked("own actual queued second WRITE");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    let before = h.server.filesystem.transfer_writes_started();
    click_other(second_window, cx, "save-remote-file");
    second.read_with(cx, |panel, _| {
        assert!(matches!(&panel.pending,Some((_,Operation::Save {path,..})) if path=="/shared.txt"))
    });
    click_other(second_window, cx, "confirm-file-operation");
    idle_other(second_window, &second, cx).await;
    second.read_with(cx, |panel, cx| {
        assert!(panel.status.render(cx).contains("占用"));
        assert!(panel.has_unsaved_draft(cx));
        assert_eq!(
            panel.editing,
            Some(("/shared.txt".into(), b"original shared target".to_vec()))
        );
    });
    assert_eq!(h.server.filesystem.transfer_writes_started(), before);
    assert_eq!(h.read("/shared.txt"), b"original shared target");
    let (third_window, third) = mount(cx, h.session.clone(), h.runtime.clone());
    idle_other(third_window, &third, cx).await;
    open_edit(third_window, &third, "/independent.txt", cx);
    idle_other(third_window, &third, cx).await;
    edit_other(third_window, &third, "independent reviewed save", cx);
    click_other(third_window, cx, "save-remote-file");
    click_other(third_window, cx, "confirm-file-operation");
    idle_other(third_window, &third, cx).await;
    assert_eq!(h.read("/independent.txt"), b"independent reviewed save");
    let id = h.panel.read_with(cx, |panel, _| panel.transfer_jobs[0].id);
    h.click(cx, &format!("cancel-transfer-{id}"));
    h.phase(cx, TransferPhase::Uncertain).await;
    h.idle(cx).await;
    assert!(!hold.expired());
    hold.release();
    let before = h.server.filesystem.transfer_writes_started();
    click_other(second_window, cx, "save-remote-file");
    click_other(second_window, cx, "confirm-file-operation");
    idle_other(second_window, &second, cx).await;
    second.read_with(cx,|panel,cx| {
        assert!(panel.status.render(cx).contains("普通保存"));
        assert!(matches!(panel.isolation_target,Some(IsolationTarget::Remote(ref path)) if path=="/shared.txt"));
        assert!(panel.has_unsaved_draft(cx));
    });
    assert_eq!(h.server.filesystem.transfer_writes_started(), before);
    assert_eq!(h.read("/shared.txt"), b"original shared target");
    click_other(second_window, cx, "inspect-file-isolation");
    idle_other(second_window, &second, cx).await;
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(second_window, |_, window, cx| {
            i18n::set_language(language, cx);
            second.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            window.render_frame(cx);
            let panel = second.read(cx);
            let (message, operation) = panel
                .pending
                .as_ref()
                .checked_option("explicit shared mutation risk review");
            let Operation::AcknowledgeQuarantine(review) = operation else {
                panic!("expected exact risk acknowledgement");
            };
            assert_eq!(
                review.entries().len(),
                2,
                "final and exclusive staging paths must both be shown"
            );
            let text = message.render(cx);
            for entry in review.entries() {
                assert!(text.contains(&entry.destination));
                assert!(text.contains(&format!("#{}", entry.reservation_id)));
            }
            assert!(text.contains(if language == Language::ZhCn {
                "无法证明"
            } else {
                "cannot prove"
            }));
            assert!(window.find("confirm-file-operation").visible());
            assert!(window.find("cancel-file-operation").visible());
        })
        .checked("complete bilingual paths/IDs/risk review");
    }
    click_other(second_window, cx, "confirm-file-operation");
    idle_other(second_window, &second, cx).await;
    second.read_with(cx, |panel, cx| {
        assert!(panel.pending.is_none());
        assert!(panel.has_unsaved_draft(cx));
    });
    assert_eq!(
        h.read("/shared.txt"),
        b"original shared target",
        "risk consent changes no bytes"
    );
    click_other(second_window, cx, "save-remote-file");
    assert_eq!(
        h.read("/shared.txt"),
        b"original shared target",
        "a fresh save must still await separate approval"
    );
    click_other(second_window, cx, "confirm-file-operation");
    idle_other(second_window, &second, cx).await;
    assert_eq!(h.read("/shared.txt"), b"reviewed replacement");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer_jobs[0].status.phase,
            TransferPhase::Uncertain
        )
    });
}
