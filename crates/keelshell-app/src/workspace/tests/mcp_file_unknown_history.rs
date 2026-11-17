//! Unknown file outcomes keep complete review text and perform no recovery write.
use super::*;
use crate::workspace::modal_scope::ModalKind;

#[gpui_kit::test]
async fn mcp_unknown_file_review_keeps_exact_diff_fixed_actions_and_modal_focus(
    cx: &mut TestAppContext,
) {
    let h = Harness::new_with_file_mutations(cx);
    let target = grant_files(&h, true, cx).await;
    let replacement = format!("{}最后一行\u{202e}", "replacement\n".repeat(5000));
    assert!(replacement.len() < 65536);
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        &replacement,
        cx,
    )
    .await;
    let gate = h
        .files
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("own the temporary-file WRITE hold");
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        gate.entered() > 0
    })
    .await;
    assert!(!gate.expired());
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_disable());
    gate.release();
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read(cx).mcp_test_workers_finished()
    })
    .await;
    let writes_after_stop = h.files.filesystem.transfer_writes_started();
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    for family in crate::design::registry() {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                for (width, height) in [(900., 580.), (1440., 900.)] {
                    cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        i18n::set_language(language, cx);
                        let mut settings = h.fixture.workspace.read(cx).state.settings.clone();
                        settings.theme = theme;
                        settings.theme_family =
                            keelshell_core::ThemeId::new(family.id).checked("registered family");
                        crate::design::apply_settings(&settings, Some(window), cx);
                        h.fixture.workspace.update(cx, |view, cx| {
                            view.open_mcp_unknown_review(id, cx);
                        });
                        window.render_frame(cx);
                        assert_eq!(
                            h.fixture.workspace.read(cx).active_modal(),
                            Some(ModalKind::McpUnknownReview)
                        );
                        assert!(window.try_find("mcp-disable").is_none());
                        let content = window.find("mcp-unknown-review-content");
                        let text = content
                            .label()
                            .checked_option("complete accessible file diff");
                        assert!(text.contains("/approved/中文.txt"));
                        assert!(text.contains("- 受控中文\n"));
                        assert_eq!(
                            text.lines().filter(|line| *line == "+ replacement").count(),
                            5000
                        );
                        assert!(text.ends_with("+ 最后一行\\u{202e} [no final newline]\n"));
                        let footer = window.find("mcp-unknown-review-footer").bounds();
                        for button in ["mcp-unknown-review-confirm", "mcp-unknown-review-cancel"] {
                            let node = window.find(button);
                            let bounds = node.bounds();
                            assert!(node.visible());
                            assert!(bounds.origin.y >= footer.origin.y);
                            assert!(bounds.bottom() <= footer.bottom());
                            assert!(bounds.bottom() <= window.bounds().bottom());
                            assert!(bounds.right() <= window.bounds().right());
                        }
                        window.scroll(
                            "mcp-unknown-review-scroll",
                            ScrollDelta::Pixels(point(px(0.), px(-1000000.))),
                            cx,
                        );
                        window.render_frame(cx);
                        assert_eq!(window.find("mcp-unknown-review-footer").bounds(), footer);
                        for key in ["tab", "shift-tab"] {
                            for _ in 0..6 {
                                window.press(key, cx);
                                window.render_frame(cx);
                                assert!(
                                    h.fixture
                                        .workspace
                                        .read(cx)
                                        .modal_focus
                                        .contains_focused(window, cx)
                                );
                            }
                        }
                        window.press("escape", cx);
                        window.render_frame(cx);
                        assert_eq!(
                            h.fixture.workspace.read(cx).active_modal(),
                            Some(ModalKind::Mcp)
                        );
                    })
                    .checked(
                        "complete unknown review across registered themes, languages and sizes",
                    );
                }
            }
        }
    }
    h.fixture.workspace.update(cx, |view, cx| {
        assert_eq!(view.mcp_test_unknown_acknowledged(id), Some(false));
        view.open_mcp_unknown_review(id, cx);
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("mcp-unknown-review-confirm", cx);
        window.render_frame(cx);
    })
    .checked("acknowledge the exact file outcome after independent fixture readback");
    h.fixture.workspace.update(cx, |view, cx| {
        assert_eq!(view.mcp_test_unknown_acknowledged(id), Some(true));
        assert!(matches!(
            view.mcp_test_state(id),
            Some(ActionState::OutcomeUnknown)
        ));
        view.review_mcp_action(id, true, cx);
    });
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        writes_after_stop
    );
    assert!(h.command.requests().is_empty());
}
