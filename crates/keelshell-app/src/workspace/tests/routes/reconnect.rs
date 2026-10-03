//! Same-slot recovery uses real SSH; lifecycle scheduling has explicit controlled signals.
use super::*;
use crate::command_suggestions::SuggestionSource;
use crate::terminal::TransportState;
use keelshell_core::ReconnectPolicy;
use keelshell_session::{ConnectionEnd, ShellEnd};

async fn connect_ready(
    fixture: &Fixture,
    route: &RouteFixture,
    cx: &mut TestAppContext,
) -> Entity<TerminalView> {
    start_route(fixture, route, cx);
    authenticate_route(fixture, route, cx).await;
    ready_terminal(fixture, cx).await
}

async fn authenticate_route(fixture: &Fixture, route: &RouteFixture, cx: &mut TestAppContext) {
    for id in [route.gateway.id, route.target.id] {
        login_for(fixture, id, cx).await;
        submit_secret(fixture, id, cx);
    }
}

async fn ready_terminal(fixture: &Fixture, cx: &mut TestAppContext) -> Entity<TerminalView> {
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let view = fixture.workspace.read(cx);
        !view.saving
            && view.tabs.first().is_some_and(|terminal| {
                terminal.read(cx).is_open()
                    && terminal.read(cx).visible_text().contains("TARGET READY")
            })
    })
    .await;
    fixture
        .workspace
        .read_with(cx, |view, _| view.tabs[0].clone())
}

async fn close_transport(
    fixture: &Fixture,
    terminal: &Entity<TerminalView>,
    cx: &mut TestAppContext,
) {
    let (runtime, session) = fixture.workspace.read_with(cx, |view, _| {
        (
            view.runtime.clone(),
            view.remote_sessions[&terminal.entity_id()].clone(),
        )
    });
    runtime.spawn(async move {
        let _ = session.close().await;
    });
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        terminal.read(cx).end_reason().is_some()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.poll_reconnect(window, cx));
        window.render_frame(cx);
    })
    .checked("observe transport end and suspend its panels");
}

fn click_reconnect(fixture: &Fixture, id: gpui_kit::EntityId, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("reconnect", id), cx);
    })
    .checked("request recovery from the original tab banner");
}

#[gpui_kit::test]
async fn reconnect_keeps_slot_history_and_split_but_requires_new_command_review(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    let old = connect_ready(&fixture, &route, cx).await;
    let old_id = old.entity_id();
    let ticket = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.set_reviewed_command(
                    "before-reconnect-marker".into(),
                    Some(old_id),
                    window,
                    cx,
                );
                view.run_command(window, cx);
                view.split_remote(window, cx);
                view.set_reviewed_command(
                    "before-reconnect-marker".into(),
                    Some(old_id),
                    window,
                    cx,
                );
                view.assistant.update(cx, |assistant, cx| {
                    assistant.set_context(
                        "old-secret-free-context".into(),
                        "fixture".into(),
                        format!("{old_id:?}"),
                        cx,
                    )
                });
                view.candidate(
                    "before-reconnect-marker".into(),
                    "old suggestion".into(),
                    SuggestionSource::History,
                    cx,
                )
                .checked_option("old command suggestion")
            })
        })
        .checked("record history, capture context, split and prepare an old capability");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .tabs
            .iter()
            .all(|tab| tab.read(cx).is_open())
            && old
                .read(cx)
                .visible_text()
                .contains("before-reconnect-marker")
    })
    .await;
    let peer_id = fixture
        .workspace
        .read_with(cx, |view, _| view.tabs[1].entity_id());
    close_transport(&fixture, &old, cx).await;
    click_reconnect(&fixture, old_id, cx);
    authenticate_route(&fixture, &route, cx).await;
    let current = ready_terminal(&fixture, cx).await;
    let current_id = current.entity_id();
    assert_ne!(old_id, current_id);
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.tabs.len(), 2);
        assert_eq!(view.tabs[1].entity_id(), peer_id);
        assert_eq!(view.split_pair, Some((current_id, peer_id)));
        assert_eq!(view.command_target, Some(old_id));
        assert!(
            view.command_histories[&current_id]
                .newest_first()
                .any(|command| command == "before-reconnect-marker")
        );
        assert!(view.archived_panels.contains_key(&current_id));
        assert_eq!(
            view.assistant.read(cx).captured_context_for_test(),
            ("", "", "")
        );
    });
    let before = route
        .target_state
        .received
        .lock()
        .checked("received bytes")
        .clone();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.insert_candidate(ticket, window, cx);
            view.run_command(window, cx);
        });
        window.render_frame(cx);
        window.click("review-reconnected-command", cx);
        assert_eq!(fixture.workspace.read(cx).command_target, Some(current_id));
        window.click("run-command", cx);
    })
    .checked("reject old ticket, explicitly review original draft and run once");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        route
            .target_state
            .received
            .lock()
            .is_ok_and(|bytes| bytes.len() > before.len())
    })
    .await;
    let received = route
        .target_state
        .received
        .lock()
        .checked("received reviewed bytes")
        .clone();
    assert_eq!(
        received,
        [before, b"before-reconnect-marker\r".to_vec()].concat()
    );
    current.update(cx, |terminal, _| {
        assert_eq!(
            terminal.emulator.search(
                "before-reconnect-marker",
                crate::emulator::SearchDirection::Previous
            ),
            crate::emulator::SearchOutcome::Found
        );
    });
}

#[gpui_kit::test]
async fn cancelled_reconnect_discards_late_gateway_authentication_and_keeps_original_tab(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    let old = connect_ready(&fixture, &route, cx).await;
    close_transport(&fixture, &old, cx).await;
    let previous_target_auth = route.target_state.authenticated.load(Ordering::Acquire);
    click_reconnect(&fixture, old.entity_id(), cx);
    login_for(&fixture, route.gateway.id, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("reconnect login");
            login.secret.update(cx, |field, cx| {
                field.set_value("route-fixture-password", window, cx)
            });
            view.submit_login(window, cx);
            view.cancel_connect_route(window, cx);
        });
    })
    .checked("cancel within the same UI turn as starting real SSH authentication");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).connecting
    })
    .await;
    cx.background_executor
        .timer(Duration::from_millis(400))
        .await;
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.tabs[0].entity_id(), old.entity_id());
        assert!(view.login.is_none());
        assert!(view.connect_route.is_none());
    });
    assert_eq!(
        route.target_state.authenticated.load(Ordering::Acquire),
        previous_target_auth
    );
}

#[gpui_kit::test]
async fn route_and_trust_edits_revoke_reconnect_without_retargeting_old_output(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    let old = connect_ready(&fixture, &route, cx).await;
    close_transport(&fixture, &old, cx).await;
    click_reconnect(&fixture, old.entity_id(), cx);
    login_for(&fixture, route.gateway.id, cx).await;
    let captured_secret = fixture.workspace.read_with(cx, |view, _| {
        view.login
            .as_ref()
            .checked_option("old authentication")
            .secret
            .clone()
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            captured_secret.update(cx, |field, cx| {
                field.set_value("never-retarget-this-secret", window, cx)
            });
            let resolved = view
                .state
                .connection_route(route.target.id)
                .checked("current route");
            view.state
                .trust_host_key_for_scope(
                    &resolved.host_key_scope(0).checked_option("gateway scope"),
                    &route.pins[1],
                )
                .checked("controlled trust change");
            view.poll_reconnect(window, cx);
            assert!(view.connect_route.is_none());
            assert!(view.login.is_none());
            assert!(captured_secret.read(cx).value().is_empty());
            view.request_reconnect(old.entity_id(), false, window, cx);
            assert!(view.connect_route.is_none());
            assert_eq!(view.tabs[0].entity_id(), old.entity_id());
        });
    })
    .checked("changed trust cancels a captured prompt and blocks reuse of the old tab route");
}

fn controlled_loss(
    fixture: &Fixture,
    route: &RouteFixture,
    reason: ShellEnd,
    cx: &mut TestAppContext,
) -> (Vec<RemotePane>, tokio::sync::watch::Sender<TransportState>) {
    let mut panes = attach_remote_panes(fixture, cx);
    let (unused, _receiver) = mpsc::sync_channel(1);
    drop(std::mem::replace(&mut panes[0]._output, unused));
    let (signal, source) = tokio::sync::watch::channel(TransportState::Ready);
    cx.update_window(fixture.window, |_, window, cx| {
        panes[0]
            .terminal
            .update(cx, |terminal, _| terminal.attach_lifecycle(source));
        fixture.workspace.update(cx, |view, cx| {
            view.state
                .connections
                .iter_mut()
                .find(|connection| connection.id == route.target.id)
                .checked_option("target policy")
                .reconnect = ReconnectPolicy::Automatic {
                max_attempts: 2,
                initial_delay_seconds: 1,
                max_delay_seconds: 1,
            };
            let resolved = view
                .state
                .connection_route(route.target.id)
                .checked("resolve controlled signal route");
            view.bind_remote_tab(panes[0].terminal.entity_id(), resolved);
            signal.send_replace(TransportState::Ended(reason));
            view.poll_reconnect(window, cx);
        });
    })
    .checked("inject a typed lifecycle signal; transport behavior is tested separately");
    (panes, signal)
}

#[gpui_kit::test]
async fn automatic_reconnect_waits_for_explicit_authentication_and_cancel_is_sticky(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    let (panes, _signal) = controlled_loss(
        &fixture,
        &route,
        ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost),
        cx,
    );
    let id = panes[0].terminal.entity_id();
    cx.wait_for(fixture.window, Duration::from_secs(4), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .connect_route
            .as_ref()
            .is_some_and(|route| route.awaiting_interaction)
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        assert!(
            view.login.is_none(),
            "a background reconnect must not create an authentication modal"
        );
    });
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 0);
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.render_frame(cx);
        window.click(("continue-reconnect", id), cx);
        assert!(fixture.workspace.read(cx).login.is_some());
        fixture
            .workspace
            .update(cx, |view, cx| view.cancel_login(window, cx));
    })
    .checked("explicitly open authentication then cancel the entire series");
    cx.background_executor
        .timer(Duration::from_millis(1400))
        .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.poll_reconnect(window, cx));
        assert!(fixture.workspace.read(cx).connect_route.is_none());
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("redraw and scheduler cannot resurrect cancelled recovery");
    assert!(writes(&panes[0]).is_empty());
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 0);
}

#[gpui_kit::test]
async fn normal_exit_and_route_edit_during_backoff_never_start_automatic_authentication(
    cx: &mut TestAppContext,
) {
    for normal_exit in [true, false] {
        let (fixture, route) = mount_route(cx, false, true);
        let reason = if normal_exit {
            ShellEnd::Exited { code: 0 }
        } else {
            ShellEnd::ConnectionClosed(ConnectionEnd::KeepaliveTimeout)
        };
        let (panes, _signal) = controlled_loss(&fixture, &route, reason, cx);
        if !normal_exit {
            cx.update_window(fixture.window, |_, window, cx| {
                fixture.workspace.update(cx, |view, cx| {
                    view.edit_connection(route.gateway.clone(), window, cx)
                });
            })
            .checked("editing an upstream profile cancels a scheduled reconnect");
        }
        cx.background_executor
            .timer(Duration::from_millis(1300))
            .await;
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.form = None;
                view.poll_reconnect(window, cx);
            });
            assert!(fixture.workspace.read(cx).connect_route.is_none());
        })
        .checked("normal exit and explicitly cancelled schedules stay stopped");
        assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 0);
        assert!(writes(&panes[0]).is_empty());
    }
}

#[gpui_kit::test]
async fn archived_draft_requires_confirmation_and_new_edits_revoke_that_consent(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    let old = connect_ready(&fixture, &route, cx).await;
    let old_files = fixture.workspace.read_with(cx, |view, _| {
        view.panels[&old.entity_id()]
            .files
            .clone()
            .checked_option("original file panel")
    });
    cx.update_window(fixture.window, |_, window, cx| {
        old_files.update(cx, |files, cx| {
            files.seed_draft_for_test(
                "/draft.txt",
                "original",
                "preserve this unsaved draft",
                window,
                cx,
            )
        });
    })
    .checked("seed an explicit controlled file editing state");
    close_transport(&fixture, &old, cx).await;
    click_reconnect(&fixture, old.entity_id(), cx);
    authenticate_route(&fixture, &route, cx).await;
    let current = ready_terminal(&fixture, cx).await;
    assert_ne!(current.entity_id(), old.entity_id());
    close_transport(&fixture, &current, cx).await;
    click_reconnect(&fixture, current.entity_id(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(fixture.workspace.read(cx).discard_archive.is_some());
        assert!(fixture.workspace.read(cx).overlay_focus.is_focused(window));
        assert!(fixture.workspace.read(cx).connect_route.is_none());
        window.click("keep-session-archive", cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .show_archived
                .contains(&current.entity_id())
        );
        assert_eq!(
            old_files.read(cx).draft_snapshot(cx).as_str(),
            "preserve this unsaved draft"
        );
    })
    .checked("keep and display the exact archived draft without starting SSH");
    click_reconnect(&fixture, current.entity_id(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-discard-session-archive", cx);
    })
    .checked("approve the exact archive snapshot");
    login_for(&fixture, route.gateway.id, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        old_files.update(cx, |files, cx| {
            files.seed_draft_for_test(
                "/draft.txt",
                "original",
                "newer edit after confirmation",
                window,
                cx,
            )
        });
    })
    .checked("edit the archived draft after the reconnect attempt has started");
    authenticate_route(&fixture, &route, cx).await;
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).connect_route.is_none()
    })
    .await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.tabs[0].entity_id(), current.entity_id());
        assert_eq!(
            view.archived_panels[&current.entity_id()]
                .files
                .as_ref()
                .checked_option("retained archive")
                .entity_id(),
            old_files.entity_id()
        );
        assert_eq!(
            old_files.read(cx).draft_snapshot(cx).as_str(),
            "newer edit after confirmation"
        );
        assert!(view.status.render(cx).contains("草稿已变化"));
    });
    click_reconnect(&fixture, current.entity_id(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-discard-session-archive", cx);
    })
    .checked("approve the newer snapshot explicitly");
    authenticate_route(&fixture, &route, cx).await;
    let replacement = ready_terminal(&fixture, cx).await;
    fixture.workspace.read_with(cx, |view, _| {
        assert_ne!(replacement.entity_id(), current.entity_id());
        assert_eq!(view.archived_panels.len(), 1);
        assert_ne!(
            view.archived_panels[&replacement.entity_id()]
                .files
                .as_ref()
                .checked_option("new previous session archive")
                .entity_id(),
            old_files.entity_id()
        );
    });
}

#[gpui_kit::test]
async fn profile_reconnect_policy_saves_through_the_real_connection_form(cx: &mut TestAppContext) {
    let (fixture, route) = mount_route(cx, false, true);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.edit_connection(route.target.clone(), window, cx)
        });
        window.render_frame(cx);
        window.click("reconnect-policy", cx);
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("enable the default bounded policy and save the profile");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let expected = ReconnectPolicy::Automatic {
        max_attempts: 3,
        initial_delay_seconds: 2,
        max_delay_seconds: 30,
    };
    let saved = fixture.store.load().checked("read saved profile policy");
    let saved = saved
        .connections
        .iter()
        .find(|connection| connection.id == route.target.id)
        .checked_option("saved target")
        .clone();
    assert_eq!(saved.reconnect, expected);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.edit_connection(saved, window, cx));
        i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        let view = fixture.workspace.read(cx);
        assert_eq!(
            view.form
                .as_ref()
                .checked_option("reopened policy")
                .reconnect_editor
                .read(cx)
                .draft(cx),
            Ok(expected)
        );
        window.click("reconnect-policy", cx);
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("reopen in English, disable automatic recovery, and save again");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        fixture
            .store
            .load()
            .checked("manual policy persisted")
            .connections
            .iter()
            .find(|connection| connection.id == route.target.id)
            .checked_option("saved manual target")
            .reconnect,
        ReconnectPolicy::Manual
    );
}
