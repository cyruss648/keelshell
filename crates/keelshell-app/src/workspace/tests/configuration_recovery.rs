//! Controlled GPUI handlers with real metadata files; no desktop-native claim.
use super::*;

async fn ready(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .configuration_recovery
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).pending_for_test())
    })
    .await;
}

#[gpui_kit::test]
async fn failed_restore_retains_visible_owner_after_deferred_close(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    fixture
        .store
        .create_config_backup()
        .checked("valid metadata backup");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery", cx);
    })
    .checked("open actual recovery modal");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("configuration-backup", 0_usize), cx);
    })
    .checked("review actual backup");
    ready(&fixture, cx).await;
    let changed = b"external-change-before-restoration-and-close";
    std::fs::write(fixture.store.path(), changed).checked("invalidate exact reviewed bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery-acknowledge", cx);
        window.render_frame(cx);
        window.click("configuration-recovery-restore", cx);
    })
    .checked("dispatch the stale restore through the real App effect boundary");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .configuration_recovery
                .as_ref()
                .checked_option("mounted owner")
                .read(cx)
                .pending_for_test()
        );
        window.click("configuration-recovery-close", cx);
    })
    .checked("dispatch stale restore, then close during owned work");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .configuration_recovery
            .as_ref()
            .is_none_or(|panel| !panel.read(cx).pending_for_test())
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("unchanged external bytes"),
        changed
    );
    assert!(
        !fixture
            .store
            .path()
            .with_file_name("state.json.originals")
            .exists()
    );
    cx.update_window(fixture.window, |_, window, cx| {
        let panel = fixture
            .workspace
            .read(cx)
            .configuration_recovery
            .clone()
            .checked_option("the actual failure must retain its visible owner");
        assert!(panel.read(cx).status_for_test(cx).contains("stale"));
        window.render_frame(cx);
        assert!(window.find("configuration-recovery-failure").visible());
        window.click("configuration-recovery-close", cx);
    })
    .checked("actual typed failure remains visible until a new explicit close");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        fixture.workspace.read(cx).configuration_recovery.is_none()
    })
    .await;
}

#[gpui_kit::test]
async fn recovery_preview_and_unapproved_restore_leave_current_bytes_unchanged(
    cx: &mut TestAppContext,
) {
    let fixture = mount(
        cx,
        vec![Connection::new(
            "backup-profile",
            "fixture.invalid",
            "fixture",
        )],
    );
    fixture
        .store
        .create_config_backup()
        .checked("metadata backup");
    let corrupt = b"{truncated-state";
    std::fs::write(fixture.store.path(), corrupt).checked("damage isolated owner-only state");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery", cx);
    })
    .checked("open actual recovery modal");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("configuration-backup", 0_usize), cx);
    })
    .checked("select actual snapshot");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("configuration-recovery-preview").is_some());
        window.click("configuration-recovery-restore", cx);
        assert!(
            !fixture
                .workspace
                .read(cx)
                .configuration_recovery
                .as_ref()
                .checked_option("mounted panel")
                .read(cx)
                .pending_for_test()
        );
        window.click("configuration-recovery-close", cx);
    })
    .checked("unapproved restore is disabled and closing cancels review");
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("current bytes"),
        corrupt
    );
    assert!(
        !fixture
            .store
            .path()
            .with_file_name("state.json.originals")
            .exists()
    );
    cx.update(|cx| assert!(fixture.workspace.read(cx).configuration_recovery.is_none()));
}

#[gpui_kit::test]
async fn explicit_recovery_updates_workspace_preserves_original_and_never_opens_a_session(
    cx: &mut TestAppContext,
) {
    let fixture = mount(
        cx,
        vec![Connection::new(
            "recovered-profile",
            "fixture.invalid",
            "fixture",
        )],
    );
    fixture
        .store
        .create_config_backup()
        .checked("metadata backup");
    let corrupt = b"invalid-config-for-controlled-ui";
    std::fs::write(fixture.store.path(), corrupt).checked("damage isolated file");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.command.update(cx, |draft, cx| {
                draft.set_value("unsent command draft", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("configuration-recovery", cx);
    })
    .checked("open recovery");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("configuration-backup", 0_usize), cx);
    })
    .checked("preview");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery-acknowledge", cx);
        window.render_frame(cx);
        window.click("configuration-recovery-restore", cx);
    })
    .checked("dispatch the approved restore through the real App effect boundary");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .configuration_recovery
                .as_ref()
                .checked_option("mounted owner")
                .read(cx)
                .pending_for_test()
        );
        window.click("configuration-recovery-close", cx);
    })
    .checked("approve exact preview, dispatch restore, then request close");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).configuration_recovery.is_none()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        let view = fixture.workspace.read(cx);
        assert_eq!(view.state.connections[0].name, "recovered-profile");
        assert!(view.tabs.is_empty());
        assert!(view.remote_sessions.is_empty());
        assert!(!view.connecting);
        assert!(view.agent.is_none());
        assert_eq!(view.command.read(cx).value(), "unsent command draft");
        assert!(view.command_target.is_none());
        window.render_frame(cx);
    })
    .checked("restore metadata without transport or command dispatch");
    let originals = std::fs::read_dir(fixture.store.path().with_file_name("state.json.originals"))
        .checked("preserved originals")
        .collect::<Result<Vec<_>, _>>()
        .checked("original entries");
    assert_eq!(originals.len(), 1);
    assert_eq!(
        std::fs::read(originals[0].path()).checked("exact original"),
        corrupt
    );
    assert_eq!(
        fixture
            .store
            .load()
            .checked("read restored file")
            .connections[0]
            .name,
        "recovered-profile"
    );
}

#[gpui_kit::test]
async fn disk_change_after_ui_review_refuses_restore_and_keeps_external_bytes(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    fixture
        .store
        .create_config_backup()
        .checked("backup valid metadata");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery", cx);
    })
    .checked("open");
    ready(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("configuration-backup", 0_usize), cx);
    })
    .checked("review");
    ready(&fixture, cx).await;
    std::fs::write(fixture.store.path(), b"external-change-after-ui-preview")
        .checked("change actual current bytes after review");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery-acknowledge", cx);
        window.render_frame(cx);
        window.click("configuration-recovery-restore", cx);
    })
    .checked("approve stale review");
    ready(&fixture, cx).await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("current bytes"),
        b"external-change-after-ui-preview"
    );
    assert!(
        !fixture
            .store
            .path()
            .with_file_name("state.json.originals")
            .exists()
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("configuration-recovery-status").visible());
        assert!(
            fixture
                .workspace
                .read(cx)
                .configuration_recovery
                .as_ref()
                .checked_option("panel")
                .read(cx)
                .status_for_test(cx)
                .contains("stale")
        );
        window.click("configuration-recovery-close", cx);
    })
    .checked("typed conflict remains visible");
}

#[gpui_kit::test]
fn live_remote_tabs_prevent_recovery_without_writing_their_transports(cx: &mut TestAppContext) {
    let fixture = mount(
        cx,
        vec![Connection::new("live", "fixture.invalid", "fixture")],
    );
    let panes = attach_remote_panes(&fixture, cx);
    let before = std::fs::read(fixture.store.path()).checked("saved metadata");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("configuration-recovery", cx);
        let view = fixture.workspace.read(cx);
        assert!(view.configuration_recovery.is_none());
        assert_eq!(view.tabs.len(), 2);
        assert!(view.status.render(cx).contains("关闭全部 SSH"));
    })
    .checked("deny restore admission while actual tab entities exist");
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("unchanged metadata"),
        before
    );
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn recovery_modal_supports_both_languages_themes_and_minimum_footer(cx: &mut TestAppContext) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    for (language, theme, label) in [
        (
            Language::ZhCn,
            keelshell_core::Theme::Light,
            "明确恢复所选备份",
        ),
        (
            Language::En,
            keelshell_core::Theme::Dark,
            "Restore selected backup",
        ),
    ] {
        cx.update_window(fixture.window, |_, window, cx| {
            i18n::set_language(language, cx);
            crate::design::apply(theme, Some(window), cx);
            fixture
                .workspace
                .update(cx, |view, cx| view.open_configuration_recovery(window, cx));
        })
        .checked("open localized minimum-size modal");
        ready(&fixture, cx).await;
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("configuration-recovery-restore").label(),
                Some(label)
            );
            let footer = window.find("configuration-recovery-footer").bounds();
            assert!(footer.origin.y >= window.bounds().origin.y);
            assert!(footer.bottom() <= window.bounds().bottom());
            window.click("configuration-recovery-close", cx);
        })
        .checked("fixed controls fit actual controlled GPUI viewport");
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            fixture.workspace.read(cx).configuration_recovery.is_none()
        })
        .await;
    }
    cx.update(|cx| i18n::set_language(Language::ZhCn, cx));
}
