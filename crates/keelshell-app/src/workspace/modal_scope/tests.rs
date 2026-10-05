//! Real GPUI frames, key dispatch and accessibility semantics for modal isolation.
use super::ModalKind;
use crate::{
    i18n,
    workspace::{self, Workspace},
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Focusable, TestAppContext, Window,
    WindowBounds, WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{AuthMethod, Connection, Language, StateStore, Theme};
use std::{path::PathBuf, sync::Arc};

struct Fixture {
    window: AnyWindowHandle,
    workspace: Entity<Workspace>,
    directory: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn mount(cx: &mut TestAppContext) -> Fixture {
    mount_sized(cx, 1280., 840.)
}

fn mount_sized(cx: &mut TestAppContext, width: f32, height: f32) -> Fixture {
    let directory = std::env::temp_dir().join(format!("keelshell-modal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory)
        .unwrap_or_else(|error| panic!("isolated modal fixture: {error:?}"));
    let store = Arc::new(StateStore::new(directory.join("state.json")));
    let mut state = store
        .load()
        .unwrap_or_else(|error| panic!("empty state: {error:?}"));
    let mut connection = Connection::new("Modal target", "modal.example.test", "operator");
    connection.auth = AuthMethod::Password;
    state.connections = vec![connection];
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("fixture runtime: {error:?}")),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        workspace::bind_keys(cx);
        i18n::set_language(Language::ZhCn, cx);
    });
    let (window, workspace) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(width), px(height)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::new(store, state, None, runtime, window, cx)),
        )
        .unwrap_or_else(|error| panic!("production workspace: {error:?}"))
    });
    cx.update_window(window, |_, window, _| window.activate_window())
        .unwrap_or_else(|error| panic!("activate: {error:?}"));
    Fixture {
        window,
        workspace,
        directory,
    }
}
fn frame(window: &mut Window, cx: &mut App) {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
}
fn assert_scope(fixture: &Fixture, window: &Window, cx: &App) {
    assert!(
        fixture
            .workspace
            .read(cx)
            .modal_focus
            .contains_focused(window, cx),
        "focus escaped the only mounted modal"
    );
    for id in [
        "quick-connect",
        "new-session",
        "toggle-assistant",
        "ai-settings",
        "connection-manager",
        "language",
        "theme-system",
        "theme-light",
        "theme-dark",
        "vault-settings",
        "about-updates",
        "mcp-settings",
    ] {
        assert!(
            window.try_find(id).is_none(),
            "background {id} remains mounted"
        );
    }
    let scope = window.find("workspace-modal-trap");
    assert_eq!(scope.role(), Some(gpui_kit::accesskit::Role::Dialog));
    assert!(scope.label().is_some());
}

#[gpui_kit::test]
fn nested_library_modals_remove_every_underlying_layer_and_trap_both_tab_directions(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        window.click("connection-manager", cx);
        frame(window, cx);
        assert_scope(&fixture, window, cx);
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            frame(window, cx);
            window.click("new-connection", cx);
            frame(window, cx);
            assert_scope(&fixture, window, cx);
            assert!(window.try_find("close-manager").is_none());
            window.click("choose-connection-folder", cx);
            frame(window, cx);
            assert_scope(&fixture, window, cx);
            assert!(window.try_find("save-connection").is_none());
            for key in ["tab", "shift-tab"] {
                for _ in 0..96 {
                    window.press(key, cx);
                    frame(window, cx);
                    assert_scope(&fixture, window, cx);
                }
            }
            window.press("escape", cx);
            frame(window, cx);
            assert_eq!(
                fixture.workspace.read(cx).active_modal(),
                Some(ModalKind::Connection)
            );
            assert!(window.try_find("destination-root").is_none());
            window.press("escape", cx);
            frame(window, cx);
            assert_eq!(
                fixture.workspace.read(cx).active_modal(),
                Some(ModalKind::Manager)
            );
        }
        window.press("escape", cx);
        frame(window, cx);
        assert_eq!(fixture.workspace.read(cx).active_modal(), None);
        assert!(window.try_find("quick-connect").is_some());
    })
    .unwrap_or_else(|error| {
        panic!("bilingual three-layer isolation and real keyboard dispatch: {error:?}")
    });
}

#[gpui_kit::test]
fn keyboard_modal_openers_restore_the_exact_retained_trigger_after_each_close(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        let root = fixture
            .workspace
            .read(cx)
            .modal_button_focus
            .borrow()
            .get(&gpui_kit::ElementId::from("connection-manager"))
            .and_then(gpui_kit::WeakFocusHandle::upgrade)
            .unwrap_or_else(|| panic!("root trigger"));
        root.focus(window, cx);
        window.press("enter", cx);
        frame(window, cx);
        let manager = fixture
            .workspace
            .read(cx)
            .modal_button_focus
            .borrow()
            .get(&gpui_kit::ElementId::from("new-connection"))
            .and_then(gpui_kit::WeakFocusHandle::upgrade)
            .unwrap_or_else(|| panic!("parent trigger"));
        manager.focus(window, cx);
        window.press("enter", cx);
        frame(window, cx);
        let form = fixture
            .workspace
            .read(cx)
            .modal_button_focus
            .borrow()
            .get(&gpui_kit::ElementId::from("choose-connection-folder"))
            .and_then(gpui_kit::WeakFocusHandle::upgrade)
            .unwrap_or_else(|| panic!("nested trigger"));
        form.focus(window, cx);
        window.press("enter", cx);
        frame(window, cx);
        window.press("escape", cx);
        frame(window, cx);
        assert!(
            form.is_focused(window),
            "close child returns to folder trigger"
        );
        window.press("escape", cx);
        frame(window, cx);
        assert!(
            manager.is_focused(window),
            "close editor returns to new connection"
        );
        window.press("escape", cx);
        frame(window, cx);
        assert!(
            root.is_focused(window),
            "close manager returns to root trigger"
        );
    })
    .unwrap_or_else(|error| {
        panic!("return real button focus through unmounted parents: {error:?}")
    });
}

#[gpui_kit::test]
fn background_global_shortcuts_cannot_replace_or_bypass_a_modal(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.open_form(window, cx));
        frame(window, cx);
        let before = fixture
            .workspace
            .read(cx)
            .form
            .as_ref()
            .unwrap_or_else(|| panic!("form"))
            .name
            .clone();
        let assistant = fixture.workspace.read(cx).show_assistant;
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl-shift"
        };
        window.press(&format!("{modifier}-t"), cx);
        window.press(&format!("{modifier}-j"), cx);
        frame(window, cx);
        let view = fixture.workspace.read(cx);
        assert_eq!(
            view.form
                .as_ref()
                .unwrap_or_else(|| panic!("same draft"))
                .name
                .entity_id(),
            before.entity_id()
        );
        assert_eq!(view.show_assistant, assistant);
        assert!(!view.show_connections);
        window.press(&format!("{modifier}-w"), cx);
        frame(window, cx);
        assert!(fixture.workspace.read(cx).form.is_none());
        assert_eq!(fixture.workspace.read(cx).active_modal(), None);
    })
    .unwrap_or_else(|error| panic!("global actions resolve the visible scope: {error:?}"));
}

#[gpui_kit::test]
fn asynchronous_mfa_interrupts_and_restores_a_draft_without_stale_focus_callbacks(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        fixture
            .workspace
            .update(cx, |view, cx| view.open_form(window, cx));
        frame(window, cx);
        let draft = fixture
            .workspace
            .read(cx)
            .form
            .as_ref()
            .unwrap_or_else(|| panic!("draft"))
            .name
            .clone();
        draft.update(cx, |input, cx| {
            input.set_value("未保存草稿 · draft", window, cx)
        });
        draft.read(cx).focus_handle(cx).focus(window, cx);
        fixture.workspace.update(cx, |view, cx| {
            view.remember_surface_focus(window, cx);
            let answer = workspace::input("MFA", "123456", window, cx);
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity: uuid::Uuid::new_v4(),
                route_id: uuid::Uuid::new_v4(),
                index: 0,
                name: "MFA fixture".into(),
                instructions: "Owned challenge".into(),
                fields: vec![workspace::KeyboardInteractiveField {
                    prompt: "Code".into(),
                    answer,
                }],
                response: None,
            });
            cx.notify();
        });
        frame(window, cx);
        assert_eq!(
            fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::KeyboardInteractive)
        );
        assert_scope(&fixture, window, cx);
        assert!(window.try_find("save-connection").is_none());
        let answer = fixture
            .workspace
            .read(cx)
            .keyboard_interactive
            .as_ref()
            .unwrap_or_else(|| panic!("challenge"))
            .fields[0]
            .answer
            .clone();
        for (language, theme) in [(Language::En, Theme::Dark), (Language::ZhCn, Theme::Light)] {
            i18n::set_language(language, cx);
            crate::design::apply(theme, Some(window), cx);
            frame(window, cx);
            assert_eq!(draft.read(cx).value().as_str(), "未保存草稿 · draft");
            assert_eq!(answer.read(cx).value().as_str(), "123456");
            assert_scope(&fixture, window, cx);
        }
        window.press("escape", cx);
        frame(window, cx);
        assert!(
            answer.read(cx).value().is_empty(),
            "cancel clears ephemeral MFA response"
        );
        assert_eq!(
            fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::Connection)
        );
        assert!(draft.read(cx).focus_handle(cx).is_focused(window));
        assert_eq!(draft.read(cx).value().as_str(), "未保存草稿 · draft");
    })
    .unwrap_or_else(|error| {
        panic!("asynchronous auth uses the same single modal scope: {error:?}")
    });
}

#[gpui_kit::test]
async fn every_root_modal_exposes_only_its_active_scope_in_both_languages_and_themes(
    cx: &mut TestAppContext,
) {
    for kind in [
        ModalKind::Login,
        ModalKind::HostApproval,
        ModalKind::Updates,
        ModalKind::Vault,
        ModalKind::AiSettings,
        ModalKind::Mcp,
        ModalKind::Workflow,
        ModalKind::Batch,
        ModalKind::Archive,
        ModalKind::SnippetParameters,
        ModalKind::SnippetEditor,
        ModalKind::SnippetDelete,
        ModalKind::OpenSshImport,
        ModalKind::LibraryBatch,
        ModalKind::Folder,
    ] {
        let fixture = mount_sized(cx, 900., 580.);
        cx.update_window(fixture.window, |_, window, cx| {
            frame(window, cx);
            fixture.workspace.update(cx, |view, cx| {
                match kind {
                    ModalKind::Login => view.prepare_login(view.state.connections[0].clone(), None, window, cx),
                    ModalKind::HostApproval => {
                        let profile = view.state.connections[0].clone();
                        let scope = view.state.connection_route(profile.id).unwrap_or_else(|error| panic!("synthetic route: {error:?}")).host_key_scope(0).unwrap_or_else(|| panic!("target scope"));
                        view.host_approval = Some(workspace::HostApproval { attempt: uuid::Uuid::new_v4(), index: 0, scope, connection: profile, fingerprint: "SHA256:synthetic-test-only".into(), previous: None });
                    }
                    ModalKind::Updates => view.open_updates(window, cx),
                    ModalKind::Vault => view.open_vault_settings(window, cx),
                    ModalKind::AiSettings => view.open_ai_settings(window, cx),
                    ModalKind::Mcp => view.open_mcp(window, cx),
                    ModalKind::Workflow => view.open_workflow(false, window, cx),
                    ModalKind::Batch => view.open_batch_commands(false, window, cx),
                    ModalKind::Archive => view.discard_archive = Some(cx.entity_id()),
                    ModalKind::SnippetParameters => {
                        let mut snippet = keelshell_core::Snippet::new("Modal parameters", "printf '%s' '{{name}}'");
                        snippet.parameterized = true;
                        view.snippet_parameters = Some(cx.new(|cx| crate::snippet_parameters::SnippetParameters::new(snippet, "Isolated target".into(), window, cx)));
                    }
                    ModalKind::SnippetEditor => view.open_snippet_editor(None, window, cx),
                    ModalKind::SnippetDelete => view.snippet_delete = Some(keelshell_core::Snippet::new("Modal snippet", "printf 'fixture'")),
                    ModalKind::OpenSshImport => {
                        let mut candidate = view.state.clone();
                        let report = candidate.import_openssh_config_report("Host modal-import\n HostName import.example.test\n User operator\n").unwrap_or_else(|error| panic!("isolated import metadata: {error:?}"));
                        view.openssh_review = Some(workspace::OpenSshImportReview::new(candidate, report));
                    }
                    ModalKind::LibraryBatch => view.open_library_batch(vec![view.state.connections[0].id], keelshell_core::ConnectionLibraryAction::AddTags(Vec::new()), false, window, cx),
                    ModalKind::Folder => view.open_folder_form(None, window, cx),
                    _ => panic!("matrix case is not configured"),
                }
                cx.notify();
            });
            for (language, theme) in [(Language::ZhCn, Theme::Light), (Language::ZhCn, Theme::Dark), (Language::En, Theme::Light), (Language::En, Theme::Dark)] {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                frame(window, cx);
                assert_eq!(fixture.workspace.read(cx).active_modal(), Some(kind));
                assert_scope(&fixture, window, cx);
                let bar = window.find("modal-appearance-bar").bounds();
                for id in ["modal-theme-system", "modal-theme-light", "modal-theme-dark", "modal-language"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    let bounds = button.bounds();
                    assert!(bounds.origin.x >= bar.origin.x && bounds.right() <= bar.right());
                    assert!(bounds.origin.y >= bar.origin.y && bounds.bottom() <= bar.bottom());
                }
                for id in ["close-update-panel", "ai-settings-cancel", "ai-settings-apply", "library-bulk-cancel", "cancel-openssh-import", "confirm-openssh-import", "cancel-login", "reject-host-key"] {
                    if let Some(button) = window.try_find(id) {
                        let bounds = button.bounds();
                        assert!(button.visible(), "{kind:?} {id} is hidden");
                        assert!(bounds.origin.y >= bar.bottom() && bounds.bottom() <= window.bounds().bottom(), "{kind:?} {id} escapes the content budget: {bounds:?}");
                    }
                }
                for key in ["tab", "shift-tab"] {
                    for _ in 0..24 { window.press(key, cx); frame(window, cx); assert_scope(&fixture, window, cx); }
                }
                assert_eq!(fixture.workspace.read(cx).state.connections.len(), 1);
                assert!(fixture.workspace.read(cx).remote_sessions.is_empty());
            }
            window.press("escape", cx);
        }).unwrap_or_else(|error| panic!("all root modal renderers use the common scope: {error:?}"));
        cx.wait_for(
            fixture.window,
            std::time::Duration::from_secs(5),
            |_, cx| fixture.workspace.read(cx).active_modal().is_none(),
        )
        .await;
        cx.update_window(fixture.window, |_, window, cx| {
            frame(window, cx);
            assert!(window.try_find("workspace-modal-trap").is_none());
            assert!(window.try_find("quick-connect").is_some());
        })
        .unwrap_or_else(|error| panic!("Escape closes only the active root scope: {error:?}"));
    }
}

#[gpui_kit::test]
fn background_programmatic_focus_and_obsolete_frame_callbacks_cannot_steal_an_auth_challenge(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        fixture
            .workspace
            .update(cx, |view, cx| view.open_form(window, cx));
        // Deliberately retain the editor's first pending post-frame callback.
        window.render_frame(cx);
        let draft = fixture
            .workspace
            .read(cx)
            .form
            .as_ref()
            .unwrap_or_else(|| panic!("retained draft"))
            .name
            .clone();
        fixture.workspace.update(cx, |view, cx| {
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity: uuid::Uuid::new_v4(),
                route_id: uuid::Uuid::new_v4(),
                index: 0,
                name: "Asynchronous MFA".into(),
                instructions: String::new(),
                fields: vec![workspace::KeyboardInteractiveField {
                    prompt: "Code".into(),
                    answer: workspace::input("Code", "", window, cx),
                }],
                response: None,
            });
            cx.notify();
        });
        frame(window, cx);
        assert_eq!(
            fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::KeyboardInteractive)
        );
        assert_scope(&fixture, window, cx);
        draft.read(cx).focus_handle(cx).focus(window, cx);
        frame(window, cx);
        assert_scope(&fixture, window, cx);
        assert!(!draft.read(cx).focus_handle(cx).is_focused(window));
    })
    .unwrap_or_else(|error| {
        panic!("generation and membership guard protects the newest modal: {error:?}")
    });
}

#[gpui_kit::test]
async fn modal_appearance_controls_persist_preferences_without_replacing_unsaved_entities(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        fixture
            .workspace
            .update(cx, |view, cx| view.open_form(window, cx));
        frame(window, cx);
        let draft = fixture
            .workspace
            .read(cx)
            .form
            .as_ref()
            .unwrap_or_else(|| panic!("profile draft"))
            .name
            .clone();
        draft.update(cx, |input, cx| {
            input.set_value("中文草稿 / draft", window, cx)
        });
        window.click("modal-theme-dark", cx);
    })
    .unwrap_or_else(|error| panic!("theme action lives inside the active modal: {error:?}"));
    cx.wait_for(
        fixture.window,
        std::time::Duration::from_secs(5),
        |_, cx| !fixture.workspace.read(cx).saving,
    )
    .await;
    let draft = cx
        .update_window(fixture.window, |_, window, cx| {
            frame(window, cx);
            let view = fixture.workspace.read(cx);
            assert_eq!(view.state.settings.theme, Theme::Dark);
            assert_eq!(view.active_modal(), Some(ModalKind::Connection));
            let draft = view
                .form
                .as_ref()
                .unwrap_or_else(|| panic!("same draft"))
                .name
                .clone();
            assert_eq!(draft.read(cx).value().as_str(), "中文草稿 / draft");
            window.click("modal-language", cx);
            draft
        })
        .unwrap_or_else(|error| panic!("theme save keeps the same visible draft: {error:?}"));
    cx.wait_for(
        fixture.window,
        std::time::Duration::from_secs(5),
        |_, cx| !fixture.workspace.read(cx).saving,
    )
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        frame(window, cx);
        let view = fixture.workspace.read(cx);
        assert_eq!(view.state.settings.language, Language::En);
        assert_eq!(
            view.form
                .as_ref()
                .unwrap_or_else(|| panic!("retained draft"))
                .name
                .entity_id(),
            draft.entity_id()
        );
        assert_eq!(draft.read(cx).value().as_str(), "中文草稿 / draft");
        assert_scope(&fixture, window, cx);
        let saved = view
            .store
            .load()
            .unwrap_or_else(|error| panic!("isolated preferences persisted: {error:?}"));
        assert_eq!(saved.settings.theme, Theme::Dark);
        assert_eq!(saved.settings.language, Language::En);
        assert_eq!(saved.connections.len(), 1);
    })
    .unwrap_or_else(|error| {
        panic!("language save keeps entity identity and synthetic connection metadata: {error:?}")
    });
}

#[gpui_kit::test]
fn async_challenge_preserves_draft_against_raw_old_frame_activation(cx: &mut TestAppContext) {
    use gpui_kit::InputEvent as _;
    for mouse_was_down in [false, true] {
        let fixture = mount(cx);
        cx.update_window(fixture.window, |_, window, cx| {
            frame(window, cx);
            fixture
                .workspace
                .update(cx, |view, cx| view.open_form(window, cx));
            frame(window, cx);
            let draft = fixture
                .workspace
                .read(cx)
                .form
                .as_ref()
                .unwrap_or_else(|| panic!("draft"))
                .name
                .clone();
            draft.update(cx, |input, cx| {
                input.set_value("must survive old-frame event", window, cx)
            });
            let position = window.find("cancel-connection").bounds().center();
            let cancel = fixture
                .workspace
                .read(cx)
                .modal_button_focus
                .borrow()
                .get(&gpui_kit::ElementId::from("cancel-connection"))
                .and_then(gpui_kit::WeakFocusHandle::upgrade)
                .unwrap_or_else(|| panic!("retained cancel button"));
            cancel.focus(window, cx);
            frame(window, cx);
            let press_pointer = |window: &mut Window, cx: &mut App| {
                window.dispatch_event(
                    gpui_kit::MouseDownEvent {
                        button: gpui_kit::MouseButton::Left,
                        position,
                        modifiers: Default::default(),
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            };
            if mouse_was_down {
                press_pointer(window, cx);
            }
            fixture.workspace.update(cx, |view, cx| {
                view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                    identity: uuid::Uuid::new_v4(),
                    route_id: uuid::Uuid::new_v4(),
                    index: 0,
                    name: "raw event challenge".into(),
                    instructions: String::new(),
                    fields: vec![workspace::KeyboardInteractiveField {
                        prompt: "Code".into(),
                        answer: workspace::input("Code", "", window, cx),
                    }],
                    response: None,
                });
                cx.notify();
            });
            // TestWindowExt::click/press render before dispatch. Raw dispatch intentionally
            // exercises the previous frame while the new async challenge is unpainted.
            if !mouse_was_down {
                press_pointer(window, cx);
            }
            assert!(
                fixture.workspace.read(cx).form.is_some(),
                "form removed by raw MouseDown"
            );
            window.dispatch_event(
                gpui_kit::MouseUpEvent {
                    button: gpui_kit::MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            assert!(
                fixture.workspace.read(cx).form.is_some(),
                "form removed by raw MouseUp"
            );
            window.dispatch_event(
                gpui_kit::KeyDownEvent {
                    keystroke: gpui_kit::Keystroke::parse("enter")
                        .unwrap_or_else(|error| panic!("fixture enter: {error:?}")),
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input(),
                cx,
            );
            assert_eq!(
                fixture.workspace.read(cx).active_modal(),
                Some(ModalKind::KeyboardInteractive)
            );
            assert_eq!(
                fixture
                    .workspace
                    .read(cx)
                    .form
                    .as_ref()
                    .unwrap_or_else(|| panic!("old cancel must not run"))
                    .name
                    .entity_id(),
                draft.entity_id()
            );
            assert_eq!(
                draft.read(cx).value().as_str(),
                "must survive old-frame event"
            );
            frame(window, cx);
            assert_scope(&fixture, window, cx);
            window.press("escape", cx);
            frame(window, cx);
            assert_eq!(
                fixture.workspace.read(cx).active_modal(),
                Some(ModalKind::Connection)
            );
            assert_eq!(
                draft.read(cx).value().as_str(),
                "must survive old-frame event"
            );
            // Releasing the old press again after remounting must not activate a
            // retained button. A fresh current-frame click must still work.
            window.dispatch_event(
                gpui_kit::MouseUpEvent {
                    button: gpui_kit::MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            assert!(fixture.workspace.read(cx).form.is_some());
            window.click("cancel-connection", cx);
            frame(window, cx);
            assert!(fixture.workspace.read(cx).form.is_none());
        })
        .unwrap_or_else(|error| panic!("raw stale frame guard: {error:?}"));
    }
}

#[gpui_kit::test]
fn same_kind_replacement_challenge_rejects_old_frame_buttons(cx: &mut TestAppContext) {
    use gpui_kit::InputEvent as _;
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let route_id = uuid::Uuid::new_v4();
        fixture.workspace.update(cx, |view, cx| {
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity: uuid::Uuid::new_v4(),
                route_id,
                index: 0,
                name: "first empty challenge".into(),
                instructions: String::new(),
                fields: Vec::new(),
                response: None,
            });
            cx.notify();
        });
        frame(window, cx);
        let position = window.find("submit-keyboard-interactive").bounds().center();
        let (response, mut receiver) = tokio::sync::oneshot::channel();
        fixture.workspace.update(cx, |view, cx| {
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity: uuid::Uuid::new_v4(),
                route_id,
                index: 0,
                name: "second empty challenge".into(),
                instructions: String::new(),
                fields: Vec::new(),
                response: Some(response),
            });
            cx.notify();
        });
        // Same route, hop and modal kind; no input EntityId can identify either
        // zero-field challenge. Do not paint the replacement before dispatch.
        window.dispatch_event(
            gpui_kit::MouseDownEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            gpui_kit::MouseUpEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        assert!(
            fixture.workspace.read(cx).keyboard_interactive.is_some(),
            "old frame consumed the replacement challenge"
        );
        assert_eq!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );
        frame(window, cx);
        assert_scope(&fixture, window, cx);
        window.press("escape", cx);
        frame(window, cx);
        assert_eq!(receiver.try_recv(), Ok(None));
    })
    .unwrap_or_else(|error| panic!("same-kind challenge boundary: {error:?}"));
}

#[gpui_kit::test]
fn held_activation_key_cannot_transfer_to_replacement_challenge(cx: &mut TestAppContext) {
    use gpui_kit::InputEvent as _;
    for key in ["enter", "space"] {
        let fixture = mount(cx);
        cx.update_window(fixture.window, |_, window, cx| {
            let route = uuid::Uuid::new_v4();
            fixture.workspace.update(cx, |view, cx| {
                view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                    identity: uuid::Uuid::new_v4(),
                    route_id: route,
                    index: 0,
                    name: "keyboard first zero-field".into(),
                    instructions: String::new(),
                    fields: Vec::new(),
                    response: None,
                });
                cx.notify();
            });
            frame(window, cx);
            let focus = fixture
                .workspace
                .read(cx)
                .modal_button_focus
                .borrow()
                .get(&gpui_kit::ElementId::from("submit-keyboard-interactive"))
                .and_then(gpui_kit::WeakFocusHandle::upgrade)
                .unwrap_or_else(|| panic!("retained submit focus"));
            focus.focus(window, cx);
            frame(window, cx);
            window.dispatch_event(
                gpui_kit::KeyDownEvent {
                    keystroke: gpui_kit::Keystroke::parse(key)
                        .unwrap_or_else(|error| panic!("activation key: {error:?}")),
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input(),
                cx,
            );
            assert!(
                fixture.workspace.read(cx).keyboard_interactive.is_some(),
                "first challenge must still exist after activation Down"
            );
            let (response, mut receiver) = tokio::sync::oneshot::channel();
            let identity = uuid::Uuid::new_v4();
            fixture.workspace.update(cx, |view, cx| {
                view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                    identity,
                    route_id: route,
                    index: 0,
                    name: "keyboard replacement zero-field".into(),
                    instructions: String::new(),
                    fields: Vec::new(),
                    response: Some(response),
                });
                cx.notify();
            });
            // Let GPUI's key dispatcher repaint its dirty frame automatically.
            // No manual focus change and no TestWindowExt helper clears old pending state.
            window.dispatch_event(
                gpui_kit::KeyUpEvent {
                    keystroke: gpui_kit::Keystroke::parse(key)
                        .unwrap_or_else(|error| panic!("activation release: {error:?}")),
                }
                .to_platform_input(),
                cx,
            );
            assert_eq!(
                fixture
                    .workspace
                    .read(cx)
                    .keyboard_interactive
                    .as_ref()
                    .unwrap_or_else(|| panic!("{key} KeyUp consumed replacement nonce"))
                    .identity,
                identity
            );
            assert_eq!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            );
            frame(window, cx);
            assert_scope(&fixture, window, cx);
            window.press("escape", cx);
            frame(window, cx);
            assert_eq!(receiver.try_recv(), Ok(None));
        })
        .unwrap_or_else(|error| panic!("{key} Down/nonce replacement/Up boundary: {error:?}"));
    }
}

fn dispatch_pointer_down(
    position: gpui_kit::Point<gpui_kit::Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    use gpui_kit::InputEvent as _;
    window.dispatch_event(
        gpui_kit::MouseDownEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
}
fn dispatch_pointer_up(
    position: gpui_kit::Point<gpui_kit::Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    use gpui_kit::InputEvent as _;
    window.dispatch_event(
        gpui_kit::MouseUpEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
}
#[gpui_kit::test]
fn old_pointer_press_cannot_survive_challenge_repaint(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let route = uuid::Uuid::new_v4();
        fixture.workspace.update(cx, |view, cx| {
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity: uuid::Uuid::new_v4(),
                route_id: route,
                index: 0,
                name: "old pointer challenge".into(),
                instructions: String::new(),
                fields: Vec::new(),
                response: None,
            });
            cx.notify();
        });
        frame(window, cx);
        let position = window.find("submit-keyboard-interactive").bounds().center();
        dispatch_pointer_down(position, window, cx);
        let (response, mut receiver) = tokio::sync::oneshot::channel();
        let identity = uuid::Uuid::new_v4();
        fixture.workspace.update(cx, |view, cx| {
            view.keyboard_interactive = Some(workspace::KeyboardInteractivePrompt {
                identity,
                route_id: route,
                index: 0,
                name: "new pointer challenge".into(),
                instructions: String::new(),
                fields: Vec::new(),
                response: Some(response),
            });
            cx.notify();
        });
        dispatch_pointer_up(position, window, cx);
        assert_eq!(
            fixture
                .workspace
                .read(cx)
                .keyboard_interactive
                .as_ref()
                .unwrap_or_else(|| panic!("first stale Up consumed replacement"))
                .identity,
            identity
        );
        frame(window, cx);
        assert_scope(&fixture, window, cx);
        let current = window.find("submit-keyboard-interactive").bounds().center();
        assert_eq!(
            position, current,
            "same control geometry for the regression"
        );
        dispatch_pointer_up(current, window, cx);
        assert_eq!(
            fixture
                .workspace
                .read(cx)
                .keyboard_interactive
                .as_ref()
                .unwrap_or_else(|| panic!("old pending MouseDown survived nonce repaint"))
                .identity,
            identity
        );
        assert_eq!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );
        window.click("cancel-keyboard-interactive", cx);
        frame(window, cx);
        assert_eq!(receiver.try_recv(), Ok(None));
    })
    .unwrap_or_else(|error| panic!("same-kind repaint must discard old pending press: {error:?}"));
}
