//! Actual review buttons and SSH replies, with no command execution before approval.
use super::*;
use keelshell_session::archive_tools::{ArchiveToolAvailability, probe_command};

fn calls(h: &Harness) -> usize {
    h.server.archive_tools.calls.load(Ordering::Acquire)
}

fn probe_review(h: &Harness, cx: &mut TestAppContext) -> Operation {
    h.panel.read_with(cx, |panel, _| {
        let (message, operation) = panel
            .pending
            .as_ref()
            .checked_option("tool inspection review");
        assert!(matches!(operation, Operation::InspectArchiveTools(_)));
        for text in [message.translations().0, message.translations().1] {
            assert!(text.contains(&probe_command()));
            assert!(text.contains(&panel.host));
        }
        operation.clone()
    })
}

#[gpui_kit::test]
async fn archive_tools_require_complete_review_and_run_once_without_sftp(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let before = h.server.active.load(Ordering::Acquire);
    h.click(cx, "review-archive-tool-inspection");
    probe_review(&h, cx);
    assert_eq!(calls(&h), 0);
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for id in ["confirm-file-operation", "cancel-file-operation"] {
            assert!(window.find(id).visible());
        }
    })
    .checked("complete command keeps confirmation reachable");
    h.click(cx, "cancel-file-operation");
    assert_eq!(calls(&h), 0);
    h.click(cx, "review-archive-tool-inspection");
    probe_review(&h, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 1);
    h.panel.read_with(cx, |panel, _| {
        let report = &panel
            .archive_inspection
            .observation
            .as_ref()
            .checked_option("actual SSH inspection")
            .report;
        assert_eq!(report.tools.len(), 7);
        assert_eq!(
            report
                .tools
                .iter()
                .filter(|tool| tool.availability == ArchiveToolAvailability::Found)
                .count(),
            3
        );
        assert_eq!(
            report.tools[2].availability,
            ArchiveToolAvailability::Missing
        );
        assert_eq!(
            report.tools[3].availability,
            ArchiveToolAvailability::Unverified
        );
        assert_eq!(report.tools[0].path.as_deref(), Some("/usr/bin/tar"));
        assert!(panel.isolation_target.is_none());
        assert!(!panel.window_close_busy());
        assert!(panel.transfer.is_none());
    });
    assert_eq!(h.server.active.load(Ordering::Acquire), before);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn archive_tool_review_rejects_a_replaced_connection_without_token_change(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    h.click(cx, "review-archive-tool-inspection");
    probe_review(&h, cx);
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            // Same host text and UUID are insufficient; the authenticated
            // transport itself must still be the one shown for approval.
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("replace the underlying SSH identity");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 0);
    assert_eq!(calls(&sibling), 0);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.observation.is_none());
        assert!(panel.operation_id.is_none());
    });
}

#[gpui_kit::test]
async fn archive_tool_late_reply_cannot_attach_to_a_replacement_connection(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    let release = h.server.archive_tools.hold_next();
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| calls(&h) == 1)
        .await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("retire reply ownership without altering the display host");
    release.send(()).checked("release the issued SSH reply");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.observation.is_none());
        assert!(!panel.window_close_busy());
        assert!(panel.status.translations().1.contains("discarded"));
    });
    assert_eq!(calls(&sibling), 0);
}

#[gpui_kit::test]
async fn archive_tool_cancel_and_retry_need_separate_explicit_reviews(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let release = h.server.archive_tools.hold_next();
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| calls(&h) == 1)
        .await;
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    drop(release);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.observation.is_none());
        assert!(panel.archive_inspection.failed);
        assert!(!panel.window_close_busy());
    });
    h.click(cx, "review-archive-tool-inspection");
    assert_eq!(calls(&h), 1);
    probe_review(&h, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 2);
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.archive_inspection.observation.is_some()
    }));
}

#[gpui_kit::test]
async fn archive_tool_failed_refresh_retires_the_previous_success(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.archive_inspection.observation.is_some()
    }));
    h.server
        .archive_tools
        .set_response(b"malformed reply".to_vec());
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 2);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.observation.is_none());
        assert!(panel.archive_inspection.failed);
        assert!(!panel.window_close_busy());
    });
}

#[gpui_kit::test]
async fn archive_tool_observations_render_in_all_themes_and_languages_without_reprobing(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{Settings, Theme, ThemeId};
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.click(cx, "toggle-archive-tool-details");
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &Settings {
                            theme_family: ThemeId::new(family.id).checked("registered theme id"),
                            theme,
                            language,
                            ..Settings::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    reveal_file_control(window, cx, "review-archive-tool-inspection");
                    assert!(window.find("review-archive-tool-inspection").visible());
                    assert!(window.try_find(("archive-tool-row", 0_usize)).is_some());
                    assert!(window.try_find(("archive-tool-row", 6_usize)).is_some());
                })
                .checked("render real observations using shared native theme tokens");
                assert_eq!(calls(&h), 1, "drawing and appearance changes never run SSH");
            }
        }
    }
}
