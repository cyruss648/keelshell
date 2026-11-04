//! In-place appearance persistence and draft identity; no GPU pixel/OS-theme claim.
use super::*;
use crate::design::{Palette, palette};
use gpui_kit::{
    WindowAppearance,
    component::{Theme as UiTheme, ThemeMode},
    rgb,
};
use keelshell_core::Theme;

#[gpui_kit::test]
fn minimum_window_keeps_bilingual_appearance_and_workspace_actions_visible(
    cx: &mut TestAppContext,
) {
    for remote_panes in [false, true] {
        let fixture = mount_sized(cx, Vec::new(), 900., 580.);
        let _panes = remote_panes.then(|| attach_remote_panes(&fixture, cx));
        cx.update_window(fixture.window, |_, window, cx| {
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let mut previous_right = window.bounds().origin.x;
                for id in [
                    "new-session",
                    "split-session",
                    "toggle-assistant",
                    "vault-settings",
                    "about-updates",
                    "theme-settings",
                    "language",
                ] {
                    let button = window.find(id);
                    let bounds = button.bounds();
                    assert!(
                        button.visible(),
                        "{id} hidden with {language:?}/{remote_panes}"
                    );
                    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                    assert!(
                        bounds.origin.x >= previous_right,
                        "{id} overlaps its preceding action: {bounds:?}"
                    );
                    assert!(
                        bounds.right() <= window.bounds().right(),
                        "{id} escapes minimum window with {language:?}/{remote_panes}: {bounds:?}"
                    );
                    assert!(
                        bounds.origin.y >= window.bounds().origin.y
                            && bounds.bottom() <= window.bounds().origin.y + px(42.)
                    );
                    previous_right = bounds.right();
                }
                let mcp = window.find("mcp-settings");
                let bounds = mcp.bounds();
                assert!(mcp.visible());
                assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                assert!(bounds.right() <= window.bounds().right());
                assert!(bounds.bottom() <= window.bounds().bottom());
                assert!(bounds.origin.y > window.bounds().origin.y + px(42.));
            }
            i18n::set_language(Language::ZhCn, cx);
        })
        .checked("render minimum bilingual toolbar with empty and two-session states");
    }
}

#[gpui_kit::test]
async fn theme_buttons_persist_without_replacing_terminal_or_unsent_drafts(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&fixture, cx);
    let (identities, revision, sources) = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.command.update(cx, |input, cx| {
                    input.set_value("printf '未执行'", window, cx)
                });
                workspace
                    .quick_connect
                    .host
                    .update(cx, |input, cx| input.set_value("draft.invalid", window, cx));
                (
                    (
                        workspace.command.entity_id(),
                        workspace.assistant.entity_id(),
                        workspace.quick_connect.host.entity_id(),
                    ),
                    workspace.command_revision,
                    workspace.command_sources_revision,
                )
            })
        })
        .checked("prepare unsent drafts before appearance switch");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-settings", cx);
        window.render_frame(cx);
    })
    .checked("open appearance preferences through its toolbar entry");
    for (theme, button, language, label) in [
        (Theme::Dark, "theme-dark", Language::ZhCn, "深色"),
        (Theme::Light, "theme-light", Language::En, "Light"),
        (Theme::System, "theme-system", Language::ZhCn, "跟随系统"),
    ] {
        cx.update_window(fixture.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            assert_eq!(window.find(button).label(), Some(label));
            window.click(button, cx);
        })
        .checked("click the actual bilingual appearance button");
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            !fixture.workspace.read(cx).saving
        })
        .await;
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.state.settings.theme, theme);
            assert_eq!(
                (
                    workspace.command.entity_id(),
                    workspace.assistant.entity_id(),
                    workspace.quick_connect.host.entity_id()
                ),
                identities
            );
            assert_eq!(workspace.command_revision, revision);
            assert_eq!(workspace.command_sources_revision, sources);
            assert_eq!(workspace.command.read(cx).value(), "printf '未执行'");
            assert_eq!(
                workspace.quick_connect.host.read(cx).value(),
                "draft.invalid"
            );
            assert_eq!(
                workspace
                    .tabs
                    .iter()
                    .map(|tab| tab.entity_id())
                    .collect::<Vec<_>>(),
                panes
                    .iter()
                    .map(|pane| pane.terminal.entity_id())
                    .collect::<Vec<_>>()
            );
            let p = palette(cx);
            let colors = &cx.global::<UiTheme>().colors;
            assert_eq!(colors.background, rgb(p.surface).into());
            assert_eq!(colors.foreground, rgb(p.text).into());
            assert_eq!(colors.button_primary_foreground, rgb(0xffffff).into());
            assert_eq!(
                p,
                if theme == Theme::Dark {
                    Palette::DARK
                } else {
                    Palette::LIGHT
                }
            );
        });
        assert_eq!(
            fixture
                .store
                .load()
                .checked("read saved appearance")
                .settings
                .theme,
            theme
        );
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
async fn system_handler_changes_visuals_only_and_explicit_choice_is_stable(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let path = fixture._state_directory.0.join("state.json");
    let before = std::fs::read(&path).checked("read initial settings bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.on_appearance_changed(WindowAppearance::VibrantDark, window, cx);
            assert_eq!(cx.global::<UiTheme>().mode, ThemeMode::Dark);
            assert_eq!(palette(cx), Palette::DARK);
            assert!(!workspace.saving);
            assert_eq!(workspace.command_sources_revision, 0);
            workspace.on_appearance_changed(WindowAppearance::VibrantLight, window, cx);
            assert_eq!(cx.global::<UiTheme>().mode, ThemeMode::Light);
        });
    })
    .checked("exercise registered appearance handler inputs");
    assert_eq!(std::fs::read(&path).checked("read unchanged bytes"), before);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.select_theme(Theme::Light, window, cx)
        });
    })
    .checked("save explicit Light preference");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let explicit = std::fs::read(&path).checked("read explicit preference bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.on_appearance_changed(WindowAppearance::Dark, window, cx);
            assert_eq!(workspace.state.settings.theme, Theme::Light);
            assert_eq!(cx.global::<UiTheme>().mode, ThemeMode::Light);
            assert!(!workspace.saving);
        });
    })
    .checked("explicit preference ignores system dark notification");
    assert_eq!(
        std::fs::read(&path).checked("read stable explicit bytes"),
        explicit
    );
}

#[gpui_kit::test]
async fn conflicting_theme_save_keeps_original_visuals_and_late_input(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let other = StateStore::new(fixture._state_directory.0.join("state.json"));
    let mut changed = other.load().checked("load independent writer");
    changed.settings.theme = Theme::Dark;
    other
        .save(&changed)
        .checked("commit independent appearance edit");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.select_theme(Theme::Light, window, cx);
            assert!(workspace.saving);
            workspace.command.update(cx, |input, cx| {
                input.set_value("late unsent draft", window, cx)
            });
        });
    })
    .checked("start stale save and edit during worker execution");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.state.settings.theme, Theme::System);
        assert_eq!(palette(cx), Palette::LIGHT);
        assert_eq!(workspace.command.read(cx).value(), "late unsent draft");
        assert_eq!(workspace.command_sources_revision, 0);
        assert!(workspace.status.render(cx).contains("保存失败"));
    });
    assert_eq!(
        other
            .load()
            .checked("preserve foreign config")
            .settings
            .theme,
        Theme::Dark
    );
}

#[gpui_kit::test]
async fn all_theme_presets_persist_and_keep_live_session_and_draft_identity(
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let before = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.command.update(cx, |input, cx| {
                    input.set_value("printf '颜色不执行命令'", window, cx);
                });
                view.quick_connect.host.update(cx, |input, cx| {
                    input.set_value("keep-draft.invalid", window, cx);
                });
                (
                    view.command.entity_id(),
                    view.assistant.entity_id(),
                    view.command_revision,
                    view.command_sources_revision,
                    view.quick_connect.host.entity_id(),
                )
            })
        })
        .checked("prepare live unsent drafts");
    for theme in crate::design::registry() {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                window.click("theme-settings", cx);
                window.render_frame(cx);
                let mut previous_right = window.bounds().origin.x;
                for family in crate::design::registry() {
                    let button = window.find(format!("theme-family-{}", family.id));
                    let bounds = button.bounds();
                    assert!(button.visible());
                    assert!(
                        bounds.origin.x >= previous_right
                            && bounds.right() <= window.bounds().right()
                    );
                    assert!(
                        bounds.origin.y >= window.bounds().origin.y
                            && bounds.bottom() <= window.bounds().bottom()
                    );
                    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                    previous_right = bounds.right();
                }
                for id in [
                    "theme-system",
                    "theme-light",
                    "theme-dark",
                    "theme-done",
                    "theme-save-status",
                ] {
                    let control = window.find(id);
                    let bounds = control.bounds();
                    assert!(control.visible(), "{id}/{language:?}");
                    assert!(
                        bounds.origin.x >= window.bounds().origin.x
                            && bounds.right() <= window.bounds().right()
                    );
                    assert!(
                        bounds.origin.y >= window.bounds().origin.y
                            && bounds.bottom() <= window.bounds().bottom()
                    );
                }
                window.click(format!("theme-family-{}", theme.id), cx);
            })
            .checked("select a registered family using the actual minimum-window picker");
            cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
                !fixture.workspace.read(cx).saving
            })
            .await;
            fixture.workspace.read_with(cx, |view, cx| {
                assert_eq!(view.state.settings.theme_family.as_str(), theme.id);
                assert_eq!(view.state.settings.theme, theme.preferred_appearance);
                assert_eq!(crate::design::current_definition(cx).id, theme.id);
                assert_eq!(
                    (
                        view.command.entity_id(),
                        view.assistant.entity_id(),
                        view.command_revision,
                        view.command_sources_revision,
                        view.quick_connect.host.entity_id()
                    ),
                    before
                );
                assert_eq!(view.command.read(cx).value(), "printf '颜色不执行命令'");
                assert_eq!(
                    view.quick_connect.host.read(cx).value(),
                    "keep-draft.invalid"
                );
                assert_eq!(
                    view.tabs.iter().map(Entity::entity_id).collect::<Vec<_>>(),
                    panes
                        .iter()
                        .map(|pane| pane.terminal.entity_id())
                        .collect::<Vec<_>>()
                );
            });
            let settings = fixture
                .store
                .load()
                .checked("read durable theme family")
                .settings;
            assert_eq!(settings.theme_family.as_str(), theme.id);
            assert_eq!(settings.theme, theme.preferred_appearance);
            cx.update_window(fixture.window, |_, window, cx| {
                window.render_frame(cx);
                window.click("theme-done", cx);
                window.render_frame(cx);
                assert!(window.find("theme-settings").visible());
            })
            .checked("return to the same workspace after applying a family");
            assert!(panes.iter().all(|pane| writes(pane).is_empty()));
        }
    }
}

#[gpui_kit::test]
async fn conflicting_family_save_does_not_apply_the_failed_preset(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let mut newer = fixture.store.load().checked("load competing writer");
    newer.settings.font_size = 18.;
    fixture
        .store
        .save(&newer)
        .checked("create settings conflict");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-settings", cx);
        window.render_frame(cx);
        window.click("theme-family-graphite", cx);
    })
    .checked("attempt conflicting preset save");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.state.settings.theme_family.as_str(), "frost");
        assert_eq!(view.state.settings.theme, Theme::System);
        assert_eq!(crate::design::current_definition(cx).id, "frost");
        assert!(view.status.render(cx).contains("保存失败"));
    });
    let saved = fixture.store.load().checked("read competing data");
    assert_eq!(saved.settings.font_size, 18.);
    assert_eq!(saved.settings.theme_family.as_str(), "frost");
}

#[gpui_kit::test]
async fn system_appearance_retains_family_without_rewriting_state(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-settings", cx);
        window.render_frame(cx);
        window.click("theme-family-lavender", cx);
    })
    .checked("select lavender");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-system", cx);
    })
    .checked("select independent System mode");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let path = fixture._state_directory.0.join("state.json");
    let before = std::fs::read(&path).checked("capture persisted preferences");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            for (appearance, mode) in [
                (WindowAppearance::Dark, ThemeMode::Dark),
                (WindowAppearance::Light, ThemeMode::Light),
            ] {
                view.on_appearance_changed(appearance, window, cx);
                assert_eq!(crate::design::current_definition(cx).id, "lavender");
                assert_eq!(
                    palette(cx),
                    crate::design::definition("lavender").palette(mode)
                );
                assert_eq!(view.state.settings.theme_family.as_str(), "lavender");
                assert!(!view.saving);
            }
        });
    })
    .checked("system notification chooses only the family variant");
    assert_eq!(std::fs::read(&path).checked("read unchanged state"), before);
}
