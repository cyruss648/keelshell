//! Independent same-profile sync review with actual modal approval and authenticated fixture SSH.
use super::*;

#[gpui_kit::test]
async fn approved_saved_profile_retarget_does_not_replace_captured_schedule_or_authenticated_session(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let entity = h.panes[0].terminal.entity_id();
    let original_session = h.servers[0].session.clone();
    let original_profile = Connection::new("Captured saved target", "example.invalid", "fixture");
    let mut local = h
        .fixture
        .store
        .load()
        .checked("local saved metadata before review");
    local.connections.push(original_profile.clone());
    let local = h
        .fixture
        .store
        .save(&local)
        .checked("persist original saved target");
    let captured_route = local
        .connection_route(original_profile.id)
        .checked("original captured route");
    h.fixture.workspace.update(cx, |view, cx| {
        view.state = local;
        // Controlled metadata labels a real owned TCP SSH fixture; no DNS target is contacted.
        view.remote_hosts
            .insert(entity, "fixture@example.invalid:22".into());
        view.bind_remote_tab(entity, captured_route.clone());
        assert!(view.remote_sessions[&entity].same_connection(&original_session));
        cx.notify();
    });
    let temporary = tempfile::tempdir().checked("owned two-store fixture");
    let directory = temporary.path().join("shared");
    std::fs::create_dir(&directory).checked("owned shared directory");
    let peer = std::sync::Arc::new(StateStore::new(temporary.path().join("peer/state.json")));
    let mut state = peer.load().checked("peer state");
    let mut incoming = original_profile.clone();
    incoming.name = "Approved saved target replacement".into();
    incoming.host = "changed.fixture.invalid".into();
    let incoming_id = incoming.id;
    state.connections.push(incoming);
    peer.save(&state).checked("peer local metadata");
    sync(&peer, &directory);
    let p = prepare_parameters_after(&h, 1, 45, cx);
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
            "background sync review deadline"
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
        window.render_frame(cx);
        assert!(window.find("profile-sync-acknowledge-effects").visible());
        window.click("profile-sync-acknowledge-effects", cx);
        input_sync(window, "profile-sync-password-field", PASSWORD, cx);
    })
    .checked("human remote choice and second password");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-approve", cx);
    })
    .checked("human explicit encrypted sync approval");
    wait_real(&h, Duration::from_secs(18), cx, |cx| {
        h.fixture
            .workspace
            .read(cx)
            .state
            .connections
            .iter()
            .any(|c| c.id == incoming_id && c.host == "changed.fixture.invalid")
    })
    .await;
    assert!(
        h.servers.iter().all(|server| server.requests().is_empty()),
        "sync approval itself sends no remote command"
    );
    h.fixture.workspace.read_with(cx, |view, _| {
        let changed = view
            .state
            .connections
            .iter()
            .find(|c| c.id == incoming_id)
            .checked_option("approved metadata applied to the same saved profile");
        assert_eq!(changed.name, "Approved saved target replacement");
        assert_eq!(changed.host, "changed.fixture.invalid");
        assert!(view.remote_sessions[&entity].same_connection(&original_session));
        let binding = view
            .reconnect_bindings
            .get(&entity)
            .checked_option("retained route binding");
        assert_eq!(
            view.batch_route_description(entity),
            Some((
                "Captured saved target".into(),
                "fixture@example.invalid:22".into()
            ))
        );
        assert_eq!(view.batch_profile_id(entity), Some(original_profile.id));
        assert!(
            !view.binding_current(binding),
            "a new reconnect cannot reuse the stale saved route"
        );
        assert_eq!(
            view.batch_template_context(entity)
                .checked_option("captured template context")
                .host,
            "example.invalid"
        );
    });
    assert!(
        chrono::Utc::now().timestamp()
            < p.review
                .schedule
                .as_ref()
                .checked_option("original future schedule")
                .first_utc_seconds()
    );
    retained(&h, &p, cx);
    completed_real(&h, Duration::from_secs(60), cx).await;
    assert_eq!(h.servers[0].requests(), vec![p.expected.clone()]);
    assert!(h.servers[1].requests().is_empty());
    no_persistence(&h, cx);
    let state = h.fixture.store.load().checked("real saved sync outcome");
    assert!(
        state
            .connections
            .iter()
            .any(|c| c.id == incoming_id && c.host == "changed.fixture.invalid")
    );
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
