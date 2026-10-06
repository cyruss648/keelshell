//! Independent production-control tests; headless GPUI is not native acceptance.
use super::{Checked, Harness};
use crate::files::sync::journal::StepState;
use gpui_kit::{AppContext, TestAppContext, test::TestAppContextExt};
use std::time::Duration;

async fn plan(h: &Harness, cx: &mut TestAppContext) {
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-remote");
    h.idle(cx).await;
}

#[gpui_kit::test]
async fn acknowledged_delete_survives_cancellation_of_second_remove_and_late_reply(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for (path, bytes) in [
        ("/a-completed", b"first".as_slice()),
        ("/b-unknown", b"second".as_slice()),
        ("/c-unstarted", b"third".as_slice()),
    ] {
        h.seed(path, bytes);
    }
    plan(&h, cx).await;
    h.click(cx, "review-directory-sync");
    let hold = h
        .server
        .filesystem
        .hold_remove_path("/b-unknown")
        .checked("second exact REMOVE gate");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| hold.entered() > 0)
        .await;
    assert!(!hold.expired());
    h.missing("/a-completed");
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    hold.release();
    h.panel.read_with(cx, |panel, _| {
        let journal = panel
            .sync_journal
            .as_ref()
            .unwrap_or_else(|| panic!("reviewed mirror journal"))
            .lock()
            .unwrap_or_else(|error| panic!("journal: {error}"));
        assert!(journal.finished);
        assert_eq!(
            journal
                .steps
                .iter()
                .map(|step| (step.path.as_str(), step.state))
                .collect::<Vec<_>>(),
            vec![
                ("a-completed", StepState::Completed),
                ("b-unknown", StepState::Unknown),
                ("c-unstarted", StepState::CancelledBeforeWrite)
            ]
        );
    });
    assert_eq!(h.read("/c-unstarted"), b"third");
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("post-cancellation quarantine observation");
        assert!(matches!(
            sftp.remove("/b-unknown").await,
            Err(keelshell_session::SessionError::MutationQuarantined)
        ));
        assert!(matches!(
            sftp.remove("/c-unstarted").await,
            Err(keelshell_session::SessionError::MutationQuarantined)
        ));
        sftp.close().await.checked("post-cancellation close");
    });
}

#[gpui_kit::test]
async fn retiring_authenticated_panel_invalidates_mirror_review_and_keeps_original_target(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/delete", b"keep authenticated target");
    plan(&h, cx).await;
    h.click(cx, "review-directory-sync");
    let prior = h.panel.read_with(cx, |panel, _| panel.session_token);
    h.panel.update(cx, |panel, cx| {
        assert!(panel.pending.is_some());
        panel.suspend(cx);
        assert!(panel.pending.is_none() && panel.session.is_none() && panel.suspended);
        assert_ne!(panel.session_token, prior);
    });
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.execute_pending(window, cx));
    })
    .checked("stale review cannot execute");
    assert_eq!(h.read("/delete"), b"keep authenticated target");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.panel
        .read_with(cx, |panel, _| assert!(panel.sync_journal.is_none()));
}
