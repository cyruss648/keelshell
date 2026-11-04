//! Actual disclosure navigation, retained drafts and keyboard routing.
use super::*;
use gpui_kit::{Focusable, test::TestWindowExt};
use keelshell_core::{Settings, Theme, ThemeId};

#[gpui_kit::test]
async fn settings_disclosures_preserve_drafts_and_real_rows_across_all_theme_palettes(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        let h = Harness::new_with(cx, |cx, session, runtime| {
            mount_layout_scene(cx, session, runtime, width, height, true)
        });
        h.idle(cx).await;
        h.seed("/retained.txt", b"untouched");
        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel
                    .name
                    .update(cx, |input, cx| input.set_value("retained-name", window, cx));
                panel
                    .mode
                    .update(cx, |input, cx| input.set_value("0640", window, cx));
                panel.local.update(cx, |input, cx| {
                    input.set_value("/retained/local/path", window, cx)
                });
                panel.run(Operation::List("/".into()), window, cx);
            });
        })
        .checked("load real SFTP row and unsent settings");
        h.idle(cx).await;
        let before = h.panel.read_with(cx, |panel, _| {
            (
                panel.name.entity_id(),
                panel.mode.entity_id(),
                panel.local.entity_id(),
                panel.browser_review_revision,
                panel.session_token,
            )
        });
        for family in crate::design::registry() {
            for appearance in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(h.window, |_, window, cx| {
                        i18n::set_language(language, cx);
                        crate::design::apply_settings(
                            &Settings {
                                theme_family: ThemeId::new(family.id).checked("theme id"),
                                theme: appearance,
                                ..Settings::default()
                            },
                            Some(window),
                            cx,
                        );
                        window.render_frame(cx);
                        assert!(window.try_find("file-tools-scroll").is_none());
                        let browsing = window.find("file-browsing-area").bounds();
                        assert_true_file_row(window);
                        for id in [
                            "toggle-file-actions",
                            "toggle-file-transfer-settings",
                            "batch-upload",
                            "batch-download",
                            "toggle-file-transfer-queue",
                        ] {
                            let control = window.find(id);
                            assert!(control.visible());
                            assert!(
                                control.bounds().left() >= window.bounds().left()
                                    && control.bounds().right() <= window.bounds().right()
                            );
                        }
                        reveal_file_control(window, cx, "file-name-draft");
                        assert!(window.try_find("file-transfer-tools").is_none());
                        assert!(
                            window.find("file-browsing-area").bounds().size.height
                                < browsing.size.height,
                            "collapsed settings must return measurable space to files"
                        );
                        assert_true_file_row(window);
                        h.panel.read(cx).name.clone().update(cx, |input, cx| {
                            input.focus(window, cx);
                        });
                        window.click("toggle-file-transfer-settings", cx);
                        window.render_frame(cx);
                        assert!(window.try_find("file-mutation-tools").is_none());
                        assert!(window.try_find("file-transfer-tools").is_some());
                        assert!(
                            !h.panel
                                .read(cx)
                                .name
                                .read(cx)
                                .focus_handle(cx)
                                .is_focused(window)
                        );
                        window.click("toggle-file-transfer-settings", cx);
                        window.render_frame(cx);
                        assert!(window.try_find("file-tools-scroll").is_none());
                        assert_eq!(window.find("file-browsing-area").bounds(), browsing);
                        assert_true_file_row(window);
                    })
                    .checked("navigate real settings controls through each palette and locale");
                    // Positive typing targets were focused above; after closing,
                    // the trigger is focused and input cannot reach hidden drafts.
                    cx.simulate_input(h.window, "hidden-typing");
                    h.panel.read_with(cx, |panel, cx| {
                        assert_eq!(panel.name.read(cx).value(), "retained-name");
                        assert_eq!(panel.mode.read(cx).value(), "0640");
                        assert_eq!(panel.local.read(cx).value(), "/retained/local/path");
                        assert_eq!(
                            (
                                panel.name.entity_id(),
                                panel.mode.entity_id(),
                                panel.local.entity_id(),
                                panel.browser_review_revision,
                                panel.session_token,
                            ),
                            before
                        );
                        assert!(panel.pending.is_none());
                    });
                }
            }
        }
        assert_eq!(h.read("/retained.txt"), b"untouched");
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    }
}

#[gpui_kit::test]
async fn entering_review_unmounts_settings_without_typing_into_their_retained_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.click(cx, "toggle-file-actions");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.name.update(cx, |input, cx| {
                input.set_value("/unsubmitted-draft", window, cx);
                input.focus(window, cx);
            });
        });
    })
    .checked("focus a visible settings draft");
    cx.simulate_input(h.window, "x");
    let draft = h
        .panel
        .read_with(cx, |panel, cx| panel.name.read(cx).value().to_string());
    assert!(
        draft.contains('x'),
        "positive control reaches visible input"
    );
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.confirm(
                crate::i18n::Message::new("审核目录", "Review folder"),
                Operation::Mkdir("/review-target".into()),
                cx,
            );
        });
        window.render_frame(cx);
        assert!(window.try_find("file-name-draft").is_none());
        assert!(window.find("confirm-file-operation").visible());
        assert!(h.panel.read(cx).review_focus.is_focused(window));
    })
    .checked("enter review while the retained settings input had focus");
    cx.simulate_input(h.window, "must-not-enter-draft");
    cx.update_window(h.window, |_, window, cx| {
        window.press("enter", cx);
    })
    .checked("typing and Enter on the inert review surface");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.name.read(cx).value(), draft);
        assert!(
            matches!(&panel.pending, Some((_, Operation::Mkdir(path))) if path == "/review-target")
        );
        assert!(panel.operation_id.is_none());
    });
    h.click(cx, "cancel-file-operation");
}

#[gpui_kit::test]
async fn queue_details_cannot_keep_keyboard_focus_in_the_hidden_editor(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/queue-draft.txt", b"base");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(
                Operation::Read(selected("/queue-draft.txt", false)),
                window,
                cx,
            );
        });
    })
    .checked("read real SFTP bytes into the inline editor");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        h.panel
            .read(cx)
            .editor
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
    })
    .checked("focus the actual retained editor");
    cx.simulate_input(h.window, "visible-edit");
    let draft = h
        .panel
        .read_with(cx, |panel, cx| panel.editor.read(cx).value().to_string());
    assert!(draft.contains("visible-edit"));
    cx.update_window(h.window, |_, window, cx| {
        window.click("toggle-file-transfer-queue", cx);
        window.render_frame(cx);
        assert!(window.try_find("remote-file-editor").is_none());
        assert!(window.find("file-transfer-queue-details").visible());
        assert!(h.panel.read(cx).review_focus.is_focused(window));
    })
    .checked("queue replaces editor presentation and moves focus");
    cx.simulate_input(h.window, "invisible-edit");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.editor.read(cx).value(), draft);
        assert!(panel.pending.is_none());
    });
    assert_eq!(h.read("/queue-draft.txt"), b"base");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
