#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::super::tests::{fixture_profile, mount};
use super::{AiLocalAgent, LocalAgentError, OperationKind, RequestCancellation};
use gpui_kit::{AppContext, TestAppContext};

#[gpui_kit::test]
fn directory_edit_revokes_pending_check_and_keeps_invalid_draft(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_backend(Some(AiLocalAgent::Codex), window, cx);
            panel.set_directory_mode(true, cx);
            let cancellation = RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.operation = Some(OperationKind::DirectoryCheck);
            let old_revision = panel.revision;
            panel.local_directory.update(cx, |field, cx| {
                field.set_value("relative invalid draft", window, cx)
            });
            panel.sync_editor(cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.revision > old_revision);
            assert!(panel.operation.is_none());
            panel.load_editor(window, cx);
            assert_eq!(
                panel.local_directory.read(cx).value(),
                "relative invalid draft"
            );
            assert!(panel.local_directory_checked.is_none());
        });
    })
    .expect("directory edit");
}

#[gpui_kit::test]
fn late_directory_check_does_not_adopt_after_revision_change(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx, fixture_profile());
    panel.update(cx, |panel, cx| {
        let owner = panel.selected.expect("owner");
        let revision = panel.revision;
        let operation_revision = panel.operation_revision;
        panel.changed(false, cx);
        let status = panel.status.clone();
        panel.finish_directory_check(
            owner,
            revision,
            operation_revision,
            Err(LocalAgentError::DirectoryMissing),
            cx,
        );
        assert_eq!(panel.status, status);
        assert!(panel.local_directory_checked.is_none());
    });
}

#[gpui_kit::test]
fn native_picker_late_selection_cannot_overwrite_a_newer_directory_draft(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_backend(Some(AiLocalAgent::ClaudeCode), window, cx);
            panel.choose_directory(window, cx);
            panel.local_directory.update(cx, |field, cx| {
                field.set_value("newer manual draft", window, cx)
            });
            panel.sync_editor(cx);
        })
    })
    .expect("picker request");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|options| {
        assert!(options.directories);
        assert!(!options.files);
        assert!(!options.multiple);
        Some(vec![std::env::temp_dir().join("late-directory")])
    });
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.local_directory.read(cx).value(), "newer manual draft");
        assert!(panel.operation.is_none());
        assert!(panel.local_directory_checked.is_none());
    });
}

#[gpui_kit::test]
fn selected_path_keeps_complete_draft_across_modes_profiles_and_locales(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx, fixture_profile());
    let path = format!("/opt/{}", "完整-directory-".repeat(80));
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_backend(Some(AiLocalAgent::Codex), window, cx);
            panel.set_directory_mode(true, cx);
            panel
                .local_directory
                .update(cx, |field, cx| field.set_value(path.clone(), window, cx));
            panel.sync_editor(cx);
            let original = panel.selected.expect("original");
            panel.set_directory_mode(false, cx);
            panel.load_editor(window, cx);
            assert_eq!(panel.local_directory.read(cx).value(), path);
            panel.set_directory_mode(true, cx);
            let second = fixture_profile();
            let id = second.id;
            panel.catalog.profiles.push(second);
            panel.select(id, window, cx);
            panel.select(original, window, cx);
            crate::i18n::set_language(keelshell_core::Language::En, cx);
            panel.refresh_locale(window, cx);
            assert_eq!(panel.local_directory.read(cx).value(), path);
            assert_eq!(super::path_value(panel.profile().expect("profile")), path);
            assert!(panel.local_directory_checked.is_none());
        })
    })
    .expect("retained full directory draft");
}
