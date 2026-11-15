#![cfg(test)]
//! Exercise actual review actions and encrypted SSH replies without running tools in the peer.
use super::*;
use keelshell_session::archive_validation::{ToolCheckStatus, validation_command};

fn calls(h: &Harness) -> usize {
    h.server.archive_validation.calls.load(Ordering::Acquire)
}

fn reviewed_probe(h: &Harness, cx: &mut TestAppContext) -> Operation {
    h.panel.read_with(cx, |panel, _| {
        let (message, operation) = panel.pending.as_ref().checked_option("validation review");
        let Operation::InspectArchiveTools(probe) = operation else {
            panic!("expected a tool review");
        };
        assert!(probe.validates_tools());
        for text in [message.translations().0, message.translations().1] {
            assert!(text.contains(&validation_command()));
            assert!(text.contains(&panel.host));
        }
        operation.clone()
    })
}

#[gpui_kit::test]
async fn archive_validation_reviews_full_command_and_runs_once_without_sftp(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let subsystems = h.server.active.load(Ordering::Acquire);
    h.click(cx, "review-archive-tool-validation");
    reviewed_probe(&h, cx);
    assert_eq!(calls(&h), 0);
    h.click(cx, "expand-file-review");
    for (width, height) in [(900., 580.), (1440., 900.)] {
        cx.update_window(h.window, |_, window, cx| {
            window.resize(size(px(width), px(height)));
            window.render_frame(cx);
            for id in ["confirm-file-operation", "cancel-file-operation"] {
                assert!(window.find(id).visible());
            }
        })
        .checked("long complete validation command keeps fixed actions visible");
    }
    h.click(cx, "cancel-file-operation");
    assert_eq!(calls(&h), 0);
    h.click(cx, "review-archive-tool-validation");
    reviewed_probe(&h, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        let report = panel
            .archive_inspection
            .validation
            .report()
            .checked_option("validated SSH reply");
        assert_eq!(report.tools.len(), 7);
        assert_eq!(
            report
                .tools
                .iter()
                .filter(|tool| tool.stream_check == ToolCheckStatus::Passed)
                .count(),
            3,
        );
        for index in [2, 3, 5, 6] {
            assert_eq!(report.tools[index].stream_check, ToolCheckStatus::NotTested);
        }
        assert!(panel.isolation_target.is_none());
        assert!(!panel.window_close_busy());
        assert!(panel.transfer.is_none());
    });
    assert_eq!(calls(&h), 1);
    assert_eq!(h.server.archive_tools.calls.load(Ordering::Acquire), 0);
    assert_eq!(h.server.active.load(Ordering::Acquire), subsystems);
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn archive_validation_rejects_old_approval_after_transport_replacement(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    h.click(cx, "review-archive-tool-validation");
    reviewed_probe(&h, cx);
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("replace authenticated transport while retaining display identity");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 0);
    assert_eq!(calls(&sibling), 0);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.validation.report().is_none());
        assert!(panel.operation_id.is_none());
    });
}

#[gpui_kit::test]
async fn archive_validation_discards_a_late_result_for_the_same_display_host(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    let release = h.server.archive_validation.hold_next();
    h.click(cx, "review-archive-tool-validation");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| calls(&h) == 1)
        .await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("retire validation result ownership");
    release.send(()).checked("release issued reply");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.validation.report().is_none());
        assert!(!panel.window_close_busy());
        assert!(panel.status.translations().1.contains("discarded"));
    });
}

#[gpui_kit::test]
async fn archive_validation_cancellation_does_not_replay_without_new_approval(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let release = h.server.archive_validation.hold_next();
    h.click(cx, "review-archive-tool-validation");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| calls(&h) == 1)
        .await;
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    drop(release);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.validation.report().is_none());
        assert!(panel.archive_inspection.validation.failed);
        assert!(!panel.window_close_busy());
    });
    h.click(cx, "review-archive-tool-validation");
    reviewed_probe(&h, cx);
    assert_eq!(calls(&h), 1);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 2);
}

#[gpui_kit::test]
async fn archive_validation_failed_refresh_retires_only_its_previous_result(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    for review in [
        "review-archive-tool-inspection",
        "review-archive-tool-validation",
    ] {
        h.click(cx, review);
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
    }
    assert!(h.panel.read_with(cx, |panel, _| {
        panel.archive_inspection.validation.report().is_some()
    }));
    h.server
        .archive_validation
        .set_response(b"invalid".to_vec());
    h.click(cx, "review-archive-tool-validation");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.validation.report().is_none());
        assert!(panel.archive_inspection.validation.failed);
        assert!(panel.archive_inspection.observation.is_some());
        assert!(!panel.window_close_busy());
    });
    assert_eq!(calls(&h), 2);
    assert_eq!(h.server.archive_tools.calls.load(Ordering::Acquire), 1);
}

#[gpui_kit::test]
async fn archive_validation_path_discovery_does_not_grant_stream_check_results(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.click(cx, "review-archive-tool-inspection");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(calls(&h), 0);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.archive_inspection.observation.is_some());
        assert!(panel.archive_inspection.validation.report().is_none());
    });
}

#[gpui_kit::test]
async fn archive_validation_theme_language_and_detail_changes_do_not_run_tools(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{Settings, Theme, ThemeId};
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.click(cx, "review-archive-tool-validation");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.click(cx, "toggle-archive-validation-details");
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    window.resize(size(px(900.), px(580.)));
                    i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &Settings {
                            theme_family: ThemeId::new(family.id).checked("registered theme"),
                            theme,
                            language,
                            ..Settings::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    reveal_file_control(window, cx, "review-archive-tool-validation");
                    assert!(window.find("review-archive-tool-validation").visible());
                    for index in [0_usize, 6] {
                        assert!(window.try_find(("archive-validation-row", index)).is_some());
                    }
                })
                .checked("render version and stream results with shared theme tokens");
                assert_eq!(calls(&h), 1);
            }
        }
    }
}
