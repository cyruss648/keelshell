//! Production GPUI controllers in independent native test-window handles.
use super::*;
use crate::workspace::Message;
use crate::workspace::modal_scope::ModalKind;
use crate::workspace::{AfterSave, NewWorkspaceWindow, window_lifecycle::CloseIntent};
use std::sync::atomic::Ordering;

pub(super) fn open_empty(fixture: &Fixture, cx: &mut TestAppContext) -> Fixture {
    let before = fixture
        .workspace
        .read_with(cx, |view, cx| view.app_windows.read(cx).windows.len());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-workspace-window", cx);
    })
    .checked("click production new-window control");
    let registration = fixture.workspace.read_with(cx, |view, cx| {
        let shared = view.app_windows.read(cx);
        assert_eq!(shared.windows.len(), before + 1);
        shared
            .windows
            .last()
            .cloned()
            .unwrap_or_else(|| panic!("new window missing"))
    });
    let workspace = registration
        .workspace
        .upgrade()
        .unwrap_or_else(|| panic!("new workspace missing"));
    let directory =
        std::env::temp_dir().join(format!("keelshell-window-fixture-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).checked("create second-window fixture lifetime guard");
    Fixture {
        window: registration.window,
        workspace,
        store: fixture.store.clone(),
        _state_directory: TemporaryState(directory),
    }
}

fn review(fixture: &Fixture, intent: CloseIntent, cx: &mut TestAppContext) -> uuid::Uuid {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.request_window_close(intent, window, cx);
            view.close_review
                .as_ref()
                .map(|review| review.id)
                .unwrap_or_else(|| panic!("close review blocked: {}", view.status.render(cx)))
        })
    })
    .checked("prepare production window close review")
}

#[gpui_kit::test]
fn new_window_is_empty_with_distinct_authority_and_shared_service(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let panes = attach_remote_panes(&first, cx);
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.command
                .update(cx, |field, cx| field.set_value("first-only", window, cx));
        });
    })
    .checked("seed first-window command draft");
    let second = open_empty(&first, cx);
    cx.update(|cx| {
        let a = first.workspace.read(cx);
        let b = second.workspace.read(cx);
        assert_ne!(first.window.window_id(), second.window.window_id());
        assert_ne!(a.window_id, b.window_id);
        assert_ne!(a.layout_window_id, b.layout_window_id);
        assert_eq!(a.app_windows.entity_id(), b.app_windows.entity_id());
        assert_eq!(a.update_service.entity_id(), b.update_service.entity_id());
        assert_eq!(a.tabs.len(), 2);
        assert!(b.tabs.is_empty() && b.remote_sessions.is_empty() && b.panels.is_empty());
        assert!(b.command.read(cx).value().is_empty());
        assert!(b.login.is_none() && b.host_approval.is_none() && b.parameter_ticket.is_none());
        assert_ne!(a.command.entity_id(), b.command.entity_id());
        assert_ne!(a.assistant.entity_id(), b.assistant.entity_id());
        assert!(
            panes
                .iter()
                .all(|pane| !pane.terminal.read(cx).visible_text().is_empty())
        );
    });
}

#[gpui_kit::test]
fn workspace_window_entry_shows_empty_and_two_window_statuses_and_activates_target(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let _panes = attach_remote_panes(&first, cx);
    let second = open_empty(&first, cx);

    cx.update_window(first.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("workspace-window-switcher").label(),
            Some("工作区窗口，2")
        );
        window.click("workspace-window-switcher", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find(("workspace-window", 1_usize)).label(),
            Some("窗口 1：2 个会话，fixture-0@example.invalid:22")
        );
        assert_eq!(
            window.find(("workspace-window", 2_usize)).label(),
            Some("窗口 2：0 个会话，快速连接")
        );
        window.click(("workspace-window", 2_usize), cx);
    })
    .checked("inspect and activate a workspace from the visible window entry");

    assert_eq!(
        cx.update(|cx| cx.active_window().map(|window| window.window_id())),
        Some(second.window.window_id())
    );
    assert!(
        cx.update(|cx| !first.workspace.read(cx).show_window_switcher),
        "activating a sibling closes the source popover"
    );
}

#[gpui_kit::test]
fn workspace_window_entry_keeps_bilingual_theme_and_gate_semantics(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());

    for language in [Language::ZhCn, Language::En] {
        for family in crate::design::registry() {
            cx.update_window(fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                let mut settings = fixture.workspace.read(cx).state.settings.clone();
                settings.theme_family =
                    keelshell_core::ThemeId::new(family.id).checked("registered theme id");
                crate::design::apply_settings(&settings, Some(window), cx);
                window.render_frame(cx);
                window.click("workspace-window-switcher", cx);
                window.render_frame(cx);

                assert!(window.find("workspace-window-switcher-popover").visible());
                assert!(window.find("workspace-window-list").visible());
                assert!(
                    window.find("workspace-window-list").bounds().bottom()
                        <= window.bounds().bottom()
                );
                assert_eq!(
                    window.find("workspace-window-switcher").label(),
                    Some(if language == Language::ZhCn {
                        "工作区窗口，1"
                    } else {
                        "Workspace windows 1"
                    })
                );
                assert_eq!(
                    window.find("workspace-new-window").label(),
                    Some(if language == Language::ZhCn {
                        "新建窗口"
                    } else {
                        "New window"
                    })
                );
                window.click("close-workspace-window-switcher", cx);
            })
            .checked("render the workspace window entry in language and theme variants");
        }
    }

    let (gate, owner, revision, epoch) = cx.update(|cx| {
        fixture.workspace.read_with(cx, |view, _| {
            (
                view.app_windows.read(cx).gate.clone(),
                view.window_id,
                view.state.snapshot,
                view.configuration_epoch,
            )
        })
    });
    let lease = crate::app_windows::ConfigurationGate::begin(&gate, owner, revision, epoch)
        .checked("hold configuration gate for workspace window entry");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-window-switcher", cx);
        window.render_frame(cx);
        assert_eq!(window.find("workspace-new-window").disabled(), Some(true));
    })
    .checked("disable new window while configuration gate is held");
    lease.cancel_before_dispatch();
}

#[gpui_kit::test]
fn workspace_window_switcher_uses_modal_scope_and_escape_restores_surface(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-window-switcher", cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);

        assert_eq!(
            fixture.workspace.read(cx).active_modal(),
            Some(ModalKind::WindowSwitcher)
        );
        let overlay = window.find("workspace-window-switcher-trap");
        assert_eq!(overlay.role(), Some(gpui_kit::accesskit::Role::Dialog));
        assert_eq!(overlay.label(), Some("工作区窗口"));
        assert!(
            fixture
                .workspace
                .read(cx)
                .modal_focus
                .contains_focused(window, cx)
        );
        for _ in 0..16 {
            window.press("tab", cx);
            window.render_frame(cx);
            assert!(
                fixture
                    .workspace
                    .read(cx)
                    .modal_focus
                    .contains_focused(window, cx)
            );
        }

        window.press("escape", cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert_eq!(fixture.workspace.read(cx).active_modal(), None);
        assert!(!fixture.workspace.read(cx).show_window_switcher);
        assert!(
            fixture
                .workspace
                .read(cx)
                .surface_focus
                .contains_focused(window, cx)
        );
    })
    .checked("close the real workspace switcher modal with Escape and restore surface focus");
}

#[gpui_kit::test]
async fn workspace_window_entry_keeps_sequence_after_a_prior_window_closes(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    let _third = open_empty(&first, cx);
    let ticket = review(&first, CloseIntent::Window, cx);

    cx.update_window(first.window, |_, window, cx| {
        first
            .workspace
            .update(cx, |view, cx| view.confirm_window_close(ticket, window, cx));
    })
    .checked("close the first window through its actual review");

    cx.wait_for(second.window, std::time::Duration::from_secs(5), |_, cx| {
        second.workspace.read(cx).app_windows.read(cx).windows.len() == 2
    })
    .await;

    cx.update_window(second.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-window-switcher", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find(("workspace-window", 2_usize)).label(),
            Some("窗口 2：0 个会话，快速连接")
        );
        assert_eq!(
            window.find(("workspace-window", 3_usize)).label(),
            Some("窗口 3：0 个会话，快速连接")
        );
        assert!(window.try_find(("workspace-window", 1_usize)).is_none());
    })
    .checked("retain stable window sequence after removing the first window");
}

#[gpui_kit::test]
fn new_window_limit_is_eight_and_saved_layout_does_not_restore_in_new_window(
    cx: &mut TestAppContext,
) {
    let mut state = super::startup_restore::scene(false);
    state.settings.updates.frequency = keelshell_core::UpdateCheckFrequency::Disabled;
    let first = mount_state_sized(cx, state, 1280., 840.);
    assert!(
        first
            .workspace
            .read_with(cx, |view, _| !view.restored_tabs.is_empty())
    );
    let mut windows = Vec::new();
    for _ in 1..keelshell_core::MAX_LAYOUT_WINDOWS {
        let next = open_empty(&first, cx);
        assert!(next.workspace.read_with(cx, |view, _| view.tabs.is_empty()
            && view.restored_tabs.is_empty()));
        windows.push(next);
    }
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.new_workspace_window(&NewWorkspaceWindow, window, cx)
        });
        assert_eq!(
            first.workspace.read(cx).app_windows.read(cx).windows.len(),
            8
        );
    })
    .checked("reject ninth running window");
}

#[gpui_kit::test]
fn window_close_cancel_preserves_drafts_and_terminal_input(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let panes = attach_remote_panes(&first, cx);
    let _second = open_empty(&first, cx);
    let ticket = review(&first, CloseIntent::Window, cx);
    cx.update_window(first.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("workspace").is_none());
        window.click("window-close-cancel", cx);
        first.workspace.update(cx, |view, cx| {
            view.confirm_window_close(ticket, window, cx);
            assert!(view.close_review.is_none() && view.closing.is_none());
            assert_eq!(view.tabs.len(), 2);
            panes[0]
                .terminal
                .update(cx, |terminal, _| {
                    terminal.try_write(b"after-cancel".to_vec())
                })
                .checked("live terminal after cancelling close");
        });
    })
    .checked("cancel close with real modal control and reject consumed ticket");
    assert_eq!(writes(&panes[0]), b"after-cancel");
    assert!(!crate::terminal::application_is_quitting_for_test());
}

#[gpui_kit::test]
fn unknown_cleanup_requires_explicit_recovery_before_a_fresh_close_review(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.closing = Some(Message::new("fixture cleanup", "fixture cleanup"));
            view.closing_intent = Some(CloseIntent::Window);
            view.finish_window_cleanup(Err("fixture worker join failed".into()), window, cx);
            assert!(view.cleanup_unknown.is_some());
            assert!(view.closing.is_none());
            assert!(view.close_review.is_none());
            assert_eq!(
                view.active_modal(),
                Some(crate::workspace::modal_scope::ModalKind::WindowClose)
            );
        });
        window.render_frame(cx);
        assert!(window.find("window-close-recover").visible());
        window.click("window-close-recover", cx);
        fixture.workspace.update(cx, |view, _| {
            assert!(view.cleanup_unknown.is_none());
            assert!(view.closing.is_none());
            assert!(view.close_review.is_some());
        });
        window.render_frame(cx);
        window.click("window-close-cancel", cx);
    })
    .checked("acknowledge unknown cleanup and create a fresh close review");
    assert!(!crate::terminal::application_is_quitting_for_test());
}

#[gpui_kit::test]
fn unknown_quit_cleanup_releases_global_quit_lock_without_retry(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.app_windows.update(cx, |shared, _| {
                shared.quitting = true;
                shared.quit_install = true;
            });
            view.closing = Some(Message::new("fixture cleanup", "fixture cleanup"));
            view.closing_intent = Some(CloseIntent::Install);
            view.finish_window_cleanup(Err("fixture install cleanup failed".into()), window, cx);
            let shared = view.app_windows.read(cx);
            assert!(!shared.quitting);
            assert!(!shared.quit_install);
            assert!(view.cleanup_unknown.is_some());
            assert!(view.close_review.is_none() && view.closing.is_none());
        });
    })
    .checked("release the global quit lock after unknown install cleanup");
}

#[gpui_kit::test]
fn aggregate_cleanup_failure_retains_sibling_after_late_success(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    let (first_id, second_id) = cx.update(|cx| {
        (
            first.workspace.read(cx).window_id,
            second.workspace.read(cx).window_id,
        )
    });
    let attempt = uuid::Uuid::new_v4();

    cx.update_window(first.window, |_, _window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.app_windows.update(cx, |shared, _| {
                shared.quitting = true;
                shared.cleanup_attempt = Some(attempt);
                shared.cleanup_intent = Some(CloseIntent::Quit);
                shared.cleanup_participants = [first_id, second_id].into_iter().collect();
                shared.cleanup_attempts.insert(first_id, attempt);
                shared.cleanup_attempts.insert(second_id, attempt);
            });
            view.closing = Some(Message::new("first cleanup", "first cleanup"));
            view.closing_intent = Some(CloseIntent::Quit);
        });
    })
    .checked("prepare failing aggregate cleanup");
    cx.update_window(second.window, |_, _, cx| {
        second.workspace.update(cx, |view, _| {
            view.closing = Some(Message::new("second cleanup", "second cleanup"));
            view.closing_intent = Some(CloseIntent::Quit);
        });
    })
    .checked("prepare sibling cleanup");

    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.finish_window_cleanup_for_attempt(
                Some(attempt),
                Err("first cleanup became unknown".into()),
                window,
                cx,
            );
        });
    })
    .checked("fail the first aggregate cleanup");

    let (window_count, quitting, cleanup_aborted) = cx.update(|cx| {
        let shared = first.workspace.read(cx).app_windows.read(cx);
        (
            shared.windows.len(),
            shared.quitting,
            shared.cleanup_aborted,
        )
    });
    assert_eq!(window_count, 2);
    assert!(!quitting);
    assert!(cleanup_aborted);
    cx.update(|cx| {
        assert!(first.workspace.read(cx).cleanup_unknown.is_some());
        assert!(second.workspace.read(cx).closing.is_none());
    });

    cx.update_window(second.window, |_, window, cx| {
        second.workspace.update(cx, |view, cx| {
            view.finish_window_cleanup_for_attempt(Some(attempt), Ok(Vec::new()), window, cx);
        });
    })
    .checked("deliver late successful sibling completion");

    cx.update(|cx| {
        assert_eq!(
            first.workspace.read(cx).app_windows.read(cx).windows.len(),
            2,
            "late success must retain the sibling owner"
        );
        assert!(second.workspace.read(cx).closing.is_none());
        assert!(second.workspace.read(cx).cleanup_unknown.is_none());
    });
    assert!(!crate::terminal::application_is_quitting_for_test());
}

#[gpui_kit::test]
fn stale_close_ticket_rejects_exact_command_change_without_stopping_any_window(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    let ticket = review(&first, CloseIntent::Window, cx);
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.command.update(cx, |draft, cx| {
                draft.set_value("new unsaved text", window, cx)
            });
            view.confirm_window_close(ticket, window, cx);
            assert!(view.closing.is_none() && view.close_review.is_none());
            assert_eq!(view.command.read(cx).value().as_str(), "new unsaved text");
            assert_eq!(view.app_windows.read(cx).windows.len(), 2);
        });
        assert!(second.workspace.read(cx).closing.is_none());
    })
    .checked("reject stale exact-text consent");
}

#[gpui_kit::test]
fn busy_save_refuses_close_without_revocation(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let panes = attach_remote_panes(&first, cx);
    let _second = open_empty(&first, cx);
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.saving = true;
            view.request_window_close(CloseIntent::Window, window, cx);
            assert!(view.close_review.is_none() && view.closing.is_none());
            view.saving = false;
            panes[0]
                .terminal
                .update(cx, |terminal, _| terminal.try_write(b"still-live".to_vec()))
                .checked("busy rejection leaves terminal live");
        });
    })
    .checked("busy close admission");
    assert_eq!(writes(&panes[0]), b"still-live");
}

#[gpui_kit::test]
fn aggregate_quit_cancel_and_sibling_stale_draft_leave_all_windows_running(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    review(&first, CloseIntent::Quit, cx);
    cx.update_window(first.window, |_, window, cx| {
        first
            .workspace
            .update(cx, |view, cx| view.cancel_window_close(window, cx));
    })
    .checked("cancel aggregate Quit");
    let ticket = review(&first, CloseIntent::Quit, cx);
    cx.update_window(second.window, |_, window, cx| {
        second.workspace.update(cx, |view, cx| {
            view.command
                .update(cx, |draft, cx| draft.set_value("B changed", window, cx))
        });
    })
    .checked("change sibling draft after aggregate review");
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.confirm_window_close(ticket, window, cx);
            assert!(!view.app_windows.read(cx).quitting);
            assert!(view.closing.is_none());
        });
        assert!(second.workspace.read(cx).closing.is_none());
    })
    .checked("reject aggregate ticket before any authority revocation");
}

#[gpui_kit::test]
fn terminal_quit_shortcut_opens_aggregate_review_before_cleanup(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let _panes = attach_remote_panes(&first, cx);
    let _second = open_empty(&first, cx);
    let quit = if cfg!(target_os = "macos") {
        "cmd-q"
    } else {
        "ctrl-shift-q"
    };

    cx.update_window(first.window, |_, window, cx| {
        window.press(quit, cx);
    })
    .checked("dispatch terminal Quit shortcut");
    cx.run_until_parked();

    cx.update(|cx| {
        let view = first.workspace.read(cx);
        assert!(
            view.close_review.is_some(),
            "terminal Quit must open aggregate review"
        );
        assert!(!view.app_windows.read(cx).quitting);
        assert!(view.closing.is_none());
    });
}

#[gpui_kit::test]
async fn close_joins_only_owned_workers_and_other_window_accepts_input(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    let first_panes = attach_remote_panes(&first, cx);
    let second = open_empty(&first, cx);
    let second_panes = attach_remote_panes(&second, cx);
    let a_stop = Arc::new(AtomicBool::new(false));
    let b_stop = Arc::new(AtomicBool::new(false));
    for (owner, stop) in [
        (first_panes[0].terminal.entity_id(), a_stop.clone()),
        (second_panes[0].terminal.entity_id(), b_stop.clone()),
    ] {
        let worker_stop = stop.clone();
        crate::terminal::spawn_owned_transport_worker(
            owner,
            "multiwindow-owned-fixture",
            stop,
            move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !worker_stop.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        )
        .checked("start bounded owned fixture worker");
    }
    let ticket = review(&first, CloseIntent::Window, cx);
    cx.update_window(first.window, |_, window, cx| {
        first
            .workspace
            .update(cx, |view, cx| view.confirm_window_close(ticket, window, cx));
    })
    .checked("approve first window close");
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        second.workspace.read(cx).app_windows.read(cx).windows.len() == 1
    })
    .await;
    assert!(a_stop.load(Ordering::Acquire));
    assert!(!b_stop.load(Ordering::Acquire));
    assert!(!crate::terminal::application_is_quitting_for_test());
    cx.update(|cx| {
        second_panes[0]
            .terminal
            .update(cx, |terminal, _| {
                terminal.try_write(b"B remains live".to_vec())
            })
            .checked("second-window terminal after actual A reap");
    });
    assert_eq!(writes(&second_panes[0]), b"B remains live");
    crate::terminal::stop_window_workers(
        &[second_panes[0].terminal.entity_id()],
        std::time::Instant::now() + Duration::from_secs(2),
    )
    .checked("explicitly reap second-window fixture worker");
}

#[gpui_kit::test]
async fn shared_store_commit_preserves_sibling_draft_requires_reload_and_then_saves_latest(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    cx.update_window(second.window, |_, window, cx| {
        second.workspace.update(cx, |view, cx| {
            view.command
                .update(cx, |draft, cx| draft.set_value("B local draft", window, cx))
        });
    })
    .checked("seed sibling draft");
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            let mut state = view.state.clone();
            state
                .connections
                .push(Connection::new("A saved", "a.invalid", "operator"));
            view.persist(state, AfterSave::None, window, cx);
        });
    })
    .checked("save shared config from A");
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        !first.workspace.read(cx).saving && second.workspace.read(cx).configuration_invalidated
    })
    .await;
    cx.update_window(second.window, |_, window, cx| {
        second.workspace.update(cx, |view, cx| {
            assert_eq!(view.command.read(cx).value().as_str(), "B local draft");
            let mut stale = view.state.clone();
            stale
                .connections
                .push(Connection::new("stale B", "stale.invalid", "operator"));
            view.persist(stale, AfterSave::None, window, cx);
            assert!(!view.saving);
            view.reload_window_configuration(window, cx);
        });
    })
    .checked("block old snapshot and explicitly reload B");
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        !second.workspace.read(cx).state_reloading
    })
    .await;
    cx.update_window(second.window, |_, window, cx| {
        second.workspace.update(cx, |view, cx| {
            assert_eq!(view.command.read(cx).value().as_str(), "B local draft");
            assert_eq!(view.state.connections[0].name, "A saved");
            let mut latest = view.state.clone();
            latest
                .connections
                .push(Connection::new("B saved", "b.invalid", "operator"));
            view.persist(latest, AfterSave::None, window, cx);
        });
    })
    .checked("save only after adopting latest shared revision");
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        !second.workspace.read(cx).saving
    })
    .await;
    let saved = first.store.load().checked("read shared CAS result");
    assert_eq!(
        saved
            .connections
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["A saved", "B saved"]
    );
}

#[gpui_kit::test]
fn maintenance_is_single_window_and_blocks_new_window_and_direct_old_panel_event(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let old_panel = cx
        .update_window(first.window, |_, window, cx| {
            first.workspace.update(cx, |view, cx| {
                view.open_configuration_recovery(window, cx);
                assert!(view.configuration_recovery.is_some());
                let old = view
                    .configuration_recovery
                    .clone()
                    .unwrap_or_else(|| panic!("maintenance panel missing"));
                view.new_workspace_window(&NewWorkspaceWindow, window, cx);
                assert_eq!(view.app_windows.read(cx).windows.len(), 1);
                view.close_active_modal(window, cx);
                old
            })
        })
        .checked("hold single-window maintenance lease");
    cx.run_until_parked();
    let second = open_empty(&first, cx);
    let snapshot = first.workspace.read_with(cx, |view, _| view.state.snapshot);
    cx.update(|cx| {
        let mut forged = first.workspace.read(cx).state.clone();
        forged
            .connections
            .push(Connection::new("late old panel", "old.invalid", "operator"));
        old_panel.update(cx, |_, cx| {
            cx.emit(crate::configuration_recovery::ConfigRecoveryEvent::Restored(Box::new(forged)))
        });
    });
    cx.run_until_parked();
    assert_eq!(
        first.workspace.read_with(cx, |view, _| view.state.snapshot),
        snapshot
    );
    assert!(
        first
            .workspace
            .read_with(cx, |view, _| view.state.connections.is_empty())
    );
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.open_configuration_recovery(window, cx);
            assert!(view.configuration_recovery.is_none());
            view.open_profile_sync(window, cx);
            assert!(view.profile_sync.is_none());
            view.open_vault_settings(window, cx);
            assert!(view.vault_settings.is_none());
        });
        assert!(second.workspace.read(cx).configuration_recovery.is_none());
    })
    .checked("reject all shared maintenance entries with two windows");
}

#[gpui_kit::test]
fn maintenance_rejects_active_route_before_configuration_recovery(cx: &mut TestAppContext) {
    let first = mount(cx, Vec::new());
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            view.route_jobs = 1;
            view.open_configuration_recovery(window, cx);
            assert!(view.configuration_recovery.is_none());
            assert!(
                !view
                    .app_windows
                    .read(cx)
                    .gate
                    .lock()
                    .is_ok_and(|gate| { gate.maintenance_owned(view.window_id) })
            );
            view.route_jobs = 0;
        });
    })
    .checked("reject configuration recovery while route work is active");
}

#[gpui_kit::test]
async fn shared_language_and_theme_update_sibling_without_replacing_inputs(
    cx: &mut TestAppContext,
) {
    let first = mount(cx, Vec::new());
    let second = open_empty(&first, cx);
    let input = second
        .workspace
        .read_with(cx, |view, _| view.command.entity_id());
    cx.update_window(first.window, |_, window, cx| {
        first.workspace.update(cx, |view, cx| {
            let mut state = view.state.clone();
            state.settings.language = Language::En;
            state.settings.theme = Theme::Dark;
            view.persist(state, AfterSave::Theme, window, cx);
        });
    })
    .checked("persist application preferences");
    cx.wait_for(second.window, Duration::from_secs(5), |_, cx| {
        let sibling = second.workspace.read(cx);
        sibling.state.settings.language == Language::En
            && sibling.state.settings.theme == Theme::Dark
    })
    .await;
    cx.update(|cx| {
        assert_eq!(second.workspace.read(cx).command.entity_id(), input);
        assert_eq!(i18n::language(cx), Language::En);
        assert_eq!(
            first.workspace.read(cx).update_service.entity_id(),
            second.workspace.read(cx).update_service.entity_id()
        );
    });
}
