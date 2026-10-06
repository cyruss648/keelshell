//! Independent consumer checks; headless controls do not prove native acceptance.
use super::{Checked, Harness};
use crate::files::sync::journal::StepState;
use gpui_kit::{TestAppContext, test::TestAppContextExt};
use keelshell_session::sftp::TransferSpec;
use std::time::Duration;

async fn local_plan(h: &Harness, cx: &mut TestAppContext) {
    h.local_input(cx, &h.local.0);
    h.click(cx, "compare-directories");
    h.idle(cx).await;
    h.click(cx, "plan-mirror-to-local");
    h.idle(cx).await;
    h.click(cx, "review-directory-sync");
}

#[gpui_kit::test]
async fn local_final_lstat_cancel_preserves_completed_and_deferred_bytes_then_fresh_review_finishes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let first = h.source("a-completed", b"first reviewed target");
    let deferred = h.source("b-deferred", "保留完整字节".as_bytes());
    let later = h.source("c-later", b"untouched later item");
    local_plan(&h, cx).await;
    let initial = h
        .server
        .filesystem
        .hold_metadata_path("/b-deferred")
        .checked("first source absence gate");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| {
        initial.entered() > 0
    })
    .await;
    assert!(!initial.expired() && !first.exists());
    let tree = h
        .server
        .filesystem
        .hold_canonical_path("/")
        .checked("subsequent owner revalidation gate");
    initial.release();
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| tree.entered() > 0)
        .await;
    let final_lstat = h
        .server
        .filesystem
        .hold_metadata_path("/b-deferred")
        .checked("second source absence gate after owner revalidation");
    assert!(!tree.expired());
    tree.release();
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| {
        final_lstat.entered() > 0
    })
    .await;
    assert_eq!(final_lstat.entered(), 1);
    assert!(!final_lstat.expired());
    h.click(cx, "cancel-active-file-operation");
    final_lstat.release();
    h.idle(cx).await;
    assert!(!first.exists());
    assert_eq!(
        std::fs::read(&deferred).checked("deferred complete bytes"),
        "保留完整字节".as_bytes()
    );
    assert_eq!(
        std::fs::read(&later).checked("later complete bytes"),
        b"untouched later item"
    );
    h.panel.read_with(cx, |panel, _| {
        let journal = panel
            .sync_journal
            .as_ref()
            .unwrap_or_else(|| panic!("actual journal"))
            .lock()
            .unwrap_or_else(|error| panic!("journal: {error}"));
        assert!(journal.finished && !journal.cleanup_failed);
        assert_eq!(journal.steps.len(), 3);
        assert_eq!(journal.steps[0].state, StepState::Completed);
        // The outer cancellation select may drop the pre-dispatch future, or
        // the explicit authority check may observe Closed first. Both refuse
        // completion; the actual target and new owner prove the write boundary.
        assert!(matches!(
            journal.steps[1].state,
            StepState::Unknown | StepState::Rejected
        ));
        assert_eq!(journal.steps[2].state, StepState::CancelledBeforeWrite);
    });
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("fresh exact owner probe");
        let allowed = || true;
        let spec = TransferSpec::download("/", &h.local.0);
        let owner = sftp
            .reserve_directory_sync(&spec, &allowed)
            .await
            .checked("known pre-dispatch local cancellation does not quarantine roots");
        drop(owner);
        sftp.close().await.checked("close owner probe");
    });
    // A new plan and explicit second approval are required; no failed action
    // resumes merely because a new owner became available.
    local_plan(&h, cx).await;
    assert!(deferred.exists() && later.exists());
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert!(!deferred.exists() && !later.exists());
    h.panel.read_with(cx, |panel, _| {
        let journal = panel
            .sync_journal
            .as_ref()
            .unwrap_or_else(|| panic!("new review journal"))
            .lock()
            .unwrap_or_else(|error| panic!("journal: {error}"));
        assert_eq!(journal.steps.len(), 2);
        assert!(
            journal.finished
                && journal
                    .steps
                    .iter()
                    .all(|step| step.state == StepState::Completed)
        );
    });
}
