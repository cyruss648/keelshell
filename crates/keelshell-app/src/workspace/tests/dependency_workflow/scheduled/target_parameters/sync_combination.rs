//! Cross-feature probes use real encrypted stores, production modal events and wall clocks.
use super::*;
use keelshell_core::{Connection, ProfileSyncChoice, ProfileSyncService, StateStore};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
use zeroize::Zeroizing;

const PASSWORD: &str = "owned-combination-sync-password";

fn sync(store: &std::sync::Arc<StateStore>, directory: &std::path::Path) {
    let service = ProfileSyncService::new(store.clone());
    let review = service
        .inspect(
            directory.into(),
            Zeroizing::new(PASSWORD.into()),
            &AtomicBool::new(false),
        )
        .checked("real encrypted channel inspect");
    let choices = review
        .rows()
        .iter()
        .map(|r| (r.id, ProfileSyncChoice::Local))
        .collect::<BTreeMap<_, _>>();
    assert!(
        service
            .apply(
                review,
                choices,
                Zeroizing::new(PASSWORD.into()),
                &AtomicBool::new(false)
            )
            .checked("real encrypted publication")
            .published
    );
}

fn hide_and_open_sync(h: &Harness, cx: &mut TestAppContext) {
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide retained authorized workflow");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_profile_sync(window, cx));
        window.render_frame(cx);
        assert!(window.find("profile-sync-panel").visible());
        assert!(window.try_find("workflow-panel").is_none());
    })
    .checked("actual encrypted sync modal owns input and layout");
    cx.run_until_parked();
}

fn input_sync(window: &mut Window, id: &'static str, value: &str, cx: &mut App) {
    window.render_frame(cx);
    let body = window.find("profile-sync-body").bounds();
    let field = window.find(id).bounds();
    if field.bottom() > body.bottom() {
        window.scroll(
            "profile-sync-body",
            ScrollDelta::Pixels(point(px(0.), body.bottom() - field.bottom() - px(8.))),
            cx,
        );
    } else if field.top() < body.top() {
        window.scroll(
            "profile-sync-body",
            ScrollDelta::Pixels(point(px(0.), body.top() - field.top() + px(8.))),
            cx,
        );
    }
    window.render_frame(cx);
    window.click(id, cx);
    let AnyInputState::Input(field) = window
        .focused_input(cx)
        .checked_option("focused actual sync input")
    else {
        panic!("sync field is an Input")
    };
    field.update(cx, |field, cx| {
        field.set_selected_range(0..field.value().len(), cx);
        field.replace(value.to_owned(), window, cx);
    });
}

fn sync_failure_observation(
    stage: &'static str,
    h: &Harness,
    p: &Prepared,
    incoming_id: uuid::Uuid,
    cx: &App,
) -> String {
    let view = h.fixture.workspace.read(cx);
    let sync = view
        .profile_sync
        .as_ref()
        .map(|panel| panel.read(cx).diagnostics_for_test(cx));
    let workflow_panel = panel_app(h, cx);
    let workflow = workflow_panel.read(cx);
    let expected_review_signature = p.review.signature_for_test();
    // Only bounded in-memory facts. No credentials, command/output bodies,
    // endpoint names, or synchronous storage inspection enter CI diagnostics.
    format!(
        "stage={stage}; sync={sync:?}; workspace_incoming={}; original_session={}; workflow_running={}; review_retained={}; slots={:?}; peer_exec_counts={:?}",
        view.state
            .connections
            .iter()
            .any(|connection| connection.id == incoming_id),
        view.remote_sessions
            .get(&h.panes[0].terminal.entity_id())
            .is_some_and(|session| session.same_connection(&h.servers[0].session)),
        workflow.is_running(),
        workflow.review_signature_for_test() == Some(expected_review_signature),
        workflow.schedule_status_for_test(),
        h.servers
            .iter()
            .map(|server| server.request_count())
            .collect::<Vec<_>>(),
    )
}

#[gpui_kit::test]
async fn scheduled_parameter_authority_survives_real_sync_approval_without_redirect_or_persistence(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let temporary = tempfile::tempdir().checked("owned two-store fixture");
    let directory = temporary.path().join("shared");
    std::fs::create_dir(&directory).checked("owned shared directory");
    let peer = std::sync::Arc::new(StateStore::new(temporary.path().join("peer/state.json")));
    let mut state = peer.load().checked("peer state");
    let incoming = Connection::new(
        "Synced unrelated metadata",
        "unreachable.fixture.invalid",
        "fixture",
    );
    let incoming_id = incoming.id;
    state.connections.push(incoming);
    peer.save(&state).checked("peer local metadata");
    sync(&peer, &directory);
    let p = prepare_parameters_after(&h, 1, 30, cx);
    h.confirm(cx);
    hide_and_open_sync(&h, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        input_sync(
            window,
            "profile-sync-directory-field",
            directory.to_str().checked_option("fixture UTF8 directory"),
            cx,
        );
        input_sync(window, "profile-sync-password-field", PASSWORD, cx);
    })
    .checked("human directory and ephemeral password inputs");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-inspect", cx);
    })
    .checked("actual inspect action");
    let deadline = std::time::Instant::now() + Duration::from_secs(18);
    loop {
        let ready = cx
            .update_window(h.fixture.window, |_, window, cx| {
                window.render_frame(cx);
                window.try_find(("profile-sync-remote", 0_usize)).is_some()
            })
            .checked("read actual decrypted choice control");
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "background sync review deadline; {}",
            cx.update(|cx| sync_failure_observation("inspect", &h, &p, incoming_id, cx)),
        );
        let runtime = h
            .fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        real_pause(&runtime, Duration::from_millis(40), cx).await;
        cx.run_until_parked();
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click(("profile-sync-remote", 0_usize), cx);
        input_sync(window, "profile-sync-password-field", PASSWORD, cx);
    })
    .checked("human remote choice and second password");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-approve", cx);
    })
    .checked("human explicit encrypted sync approval");
    wait_real_observed(
        &h,
        Duration::from_secs(18),
        cx,
        |cx| {
            h.fixture
                .workspace
                .read(cx)
                .state
                .connections
                .iter()
                .any(|c| c.id == incoming_id)
        },
        |cx| sync_failure_observation("apply-workspace", &h, &p, incoming_id, cx),
    )
    .await;
    assert!(
        h.servers.iter().all(|server| server.requests().is_empty()),
        "sync approval itself sends no remote command"
    );
    retained(&h, &p, cx);
    completed_real(&h, Duration::from_secs(40), cx).await;
    assert_eq!(h.servers[0].requests(), vec![p.expected.clone()]);
    assert!(h.servers[1].requests().is_empty());
    no_persistence(&h, cx);
    let state = h.fixture.store.load().checked("real saved sync outcome");
    assert!(state.connections.iter().any(|c| c.id == incoming_id));
    let bytes = std::fs::read(h.fixture._state_directory.0.join("state.json"))
        .checked("actual local state body");
    assert!(
        !String::from_utf8(bytes)
            .checked("state UTF8")
            .contains(PASSWORD)
    );
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                assert!(window.find("profile-sync-panel").visible());
                let footer = window.find("profile-sync-footer").bounds();
                assert!(footer.bottom() <= px(580.) && footer.top() >= px(0.));
                assert!(window.find("profile-sync-close").visible());
            })
            .checked("minimum bilingual themed sync modal with retained finished schedule");
        }
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("profile-sync-close", cx);
    })
    .checked("close actual sync modal");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("command-workflow", cx);
    })
    .checked("reopen original timer receipt");
    cx.run_until_parked();
    retained(&h, &p, cx);
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Finished(Outcome::Succeeded)]
    );
}

#[gpui_kit::test]
async fn hidden_parameter_silent_edit_while_sync_modal_active_refuses_real_due_and_restore(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let p = prepare_parameters(&h, 1, cx);
    h.confirm(cx);
    hide_and_open_sync(&h, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        p.value.update(cx, |value, cx| {
            value.set_value(
                "silently changed while another modal owns input",
                window,
                cx,
            )
        });
    })
    .checked("silent retained parameter mutation emits no InputEvent");
    completed_real(&h, Duration::from_secs(2), cx).await;
    assert!(
        chrono::Utc::now().timestamp()
            < p.review
                .schedule
                .as_ref()
                .checked_option("original future trigger")
                .first_utc_seconds(),
        "silent change feedback precedes the original due time"
    );
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Invalidated(Reason::SessionBindingChanged)]
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    cx.update_window(h.fixture.window, |_, window, cx| {
        p.value
            .update(cx, |value, cx| value.set_value(VALUE, window, cx));
        window.click("profile-sync-close", cx);
    })
    .checked("restore cannot resurrect withdrawn review");
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    real_pause(&runtime, Duration::from_millis(350), cx).await;
    cx.run_until_parked();
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    no_persistence(&h, cx);
}

#[gpui_kit::test]
async fn captured_route_replacement_during_sync_modal_refuses_parameter_schedule_on_same_connection(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let p = prepare_parameters(&h, 1, cx);
    h.confirm(cx);
    hide_and_open_sync(&h, cx);
    let entity = h.panes[0].terminal.entity_id();
    let original = h.servers[0].session.clone();
    cx.update_window(h.fixture.window, |_, _window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            let changed = Connection::new(
                "Different captured route",
                "revoked.fixture.invalid",
                "fixture",
            );
            let mut state = keelshell_core::AppState::default();
            let id = changed.id;
            state.connections.push(changed);
            let route = state.connection_route(id).checked("different route");
            // A controlled authorization-boundary mutation, never an automatic sync reconnect.
            view.bind_remote_tab(entity, route);
            assert!(view.remote_sessions[&entity].same_connection(&original));
            cx.notify();
        });
    })
    .checked("same SSH instance with changed route authority");
    completed_real(&h, Duration::from_secs(12), cx).await;
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Invalidated(Reason::SessionBindingChanged)]
    );
    assert!(panel(&h, cx).read_with(cx, |panel, _| panel.reviewed_for_test().is_none()));
    no_persistence(&h, cx);
    drop(p);
}

#[gpui_kit::test]
async fn hidden_scheduled_parameter_connection_replacement_feedback_precedes_due_with_sync_modal(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let p = prepare_parameters_after(&h, 1, 30, cx);
    h.confirm(cx);
    hide_and_open_sync(&h, cx);
    let original = h.servers[0].session.clone();
    let replacement = h.servers[1].session.clone();
    assert!(!original.same_connection(&replacement));
    cx.update_window(h.fixture.window, |_, _window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.remote_sessions
                .insert(h.panes[0].terminal.entity_id(), replacement.clone());
            cx.notify();
        });
    })
    .checked("actual separately authenticated owned SSH connection replaces captured instance");
    // No test invokes maintenance or a timer tick: the production waiting loop
    // must deliver feedback while this other modal still owns the UI.
    completed_real(&h, Duration::from_secs(2), cx).await;
    assert!(
        chrono::Utc::now().timestamp()
            < p.review
                .schedule
                .as_ref()
                .checked_option("future plan")
                .first_utc_seconds()
    );
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.schedule_status_for_test()),
        vec![Slot::Invalidated(Reason::SessionBindingChanged)]
    );
    assert!(panel(&h, cx).read_with(cx, |panel, _| panel.reviewed_for_test().is_none()));
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    no_persistence(&h, cx);
}

mod saved_profile_review;
