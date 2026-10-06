//! Independent combinations: owned SSH results, real modal events and durable state.
use super::*;
use keelshell_core::{WorkflowAuditOutcome, WorkflowAuditTrigger, format_fixed_offset_datetime};

fn panel(h: &Harness, cx: &impl AppContext) -> Entity<crate::workflow_commands::WorkflowPanel> {
    h.fixture.workspace.read_with(cx, |view, _| {
        view.workflow_panel
            .clone()
            .checked_option("retained production workflow")
    })
}

async fn wait_real(
    h: &Harness,
    timeout: Duration,
    cx: &mut TestAppContext,
    mut ready: impl FnMut(&mut App) -> bool,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    loop {
        if cx
            .update_window(h.fixture.window, |_, window, cx| {
                window.render_frame(cx);
                ready(cx)
            })
            .checked("observe actual combination transition")
        {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        let timer = runtime.spawn(async {
            tokio::time::sleep(Duration::from_millis(40)).await;
        });
        while !timer.is_finished() {
            cx.executor().timer(Duration::from_millis(10)).await;
        }
        runtime
            .block_on(timer)
            .checked("owned wall-clock wait completed");
        cx.run_until_parked();
    }
}

fn input(window: &mut Window, id: &'static str, value: &str, cx: &mut App) {
    click_visible(window, id, cx);
    let AnyInputState::Input(field) = window
        .focused_input(cx)
        .checked_option("focused finite schedule input")
    else {
        panic!("finite schedule uses InputState")
    };
    field.update(cx, |field, cx| {
        field.set_selected_range(0..field.value().len(), cx);
        field.replace(value.to_owned(), window, cx);
    });
}

#[gpui_kit::test]
async fn independent_audit_sync_modal_close_flushes_real_completed_occurrence_without_replay(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let source = "printf 'owned-combination-audit-marker'";
    h.prepare(source, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-back", cx);
        click_visible(window, "workflow-schedule-once", cx);
        input(window, "workflow-schedule-offset", "+00:00", cx);
        input(
            window,
            "workflow-schedule-start",
            &format_fixed_offset_datetime(chrono::Utc::now().timestamp() + 5, 0)
                .checked("real future occurrence"),
            cx,
        );
        input(window, "workflow-schedule-grace", "15", cx);
    })
    .checked("actual finite schedule edits");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-review-button", cx);
    })
    .checked("later full human review");
    cx.run_until_parked();
    let review = panel(&h, cx).read_with(cx, |panel, _| {
        panel.reviewed_for_test().checked_option("finite review")
    });
    let expected = review.plan.tasks()[0].command.as_bytes().to_vec();
    let spec = review.schedule.as_ref().checked_option("finite schedule");
    let schedule_id = spec.schedule_id();
    assert_eq!(spec.count(), 1);
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.confirm(cx);
    let original_entity = panel(&h, cx).entity_id();
    let original_state = std::fs::read(h.fixture.store.path()).checked("original durable state");
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide still-armed workflow through actual control");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_profile_sync(window, cx));
        window.render_frame(cx);
        assert!(window.find("profile-sync-panel").visible());
        assert!(window.try_find("workflow-panel").is_none());
    })
    .checked("actual sync modal owns the configuration lease");
    assert!(
        wait_real(&h, Duration::from_secs(15), cx, |cx| {
            let view = h.fixture.workspace.read(cx);
            view.pending_workflow_audits.len() == 1 && !panel(&h, cx).read(cx).is_running()
        })
        .await,
        "owned SSH workflow must finish while sync modal is open"
    );
    let audit = h.fixture.workspace.read_with(cx, |view, cx| {
        assert!(view.profile_sync.is_some());
        assert!(!view.show_workflow);
        assert!(!view.saving);
        assert!(view.state.workflow_audits.is_empty());
        assert_eq!(panel(&h, cx).entity_id(), original_entity);
        let record = view.pending_workflow_audits[0].clone();
        assert_eq!(record.tasks.len(), 1);
        assert_eq!(record.tasks[0].id, review.plan.tasks()[0].id);
        assert_eq!(record.tasks[0].target_id, review.plan.tasks()[0].target_id);
        assert_eq!(record.tasks[0].outcome, WorkflowAuditOutcome::Succeeded);
        assert!(matches!(record.trigger, WorkflowAuditTrigger::Scheduled { schedule_id: id, occurrence: 0, .. } if id == schedule_id));
        assert_eq!(panel(&h, cx).read(cx).audit_history_for_test(), vec![(record.id, false)]);
        record
    });
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("lease readback"),
        original_state
    );
    assert_eq!(h.servers[0].requests(), vec![expected.clone()]);
    assert!(h.servers[1].requests().is_empty());
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-close", cx);
    })
    .checked("real sync Close event releases save lease");
    cx.run_until_parked();
    let saved = wait_real(&h, Duration::from_secs(3), cx, |cx| {
        let view = h.fixture.workspace.read(cx);
        view.profile_sync.is_none()
            && !view.saving
            && view.pending_workflow_audits.is_empty()
            && view.state.workflow_audits == vec![audit.clone()]
    })
    .await;
    let after = h
        .fixture
        .store
        .load()
        .checked("actual post-close disk readback");
    let pending = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.pending_workflow_audits.len());
    let changed =
        std::fs::read(h.fixture.store.path()).checked("post-close durable bytes") != original_state;
    eprintln!(
        "actual combination: modal closed; saved={saved}; pending={pending}; disk_changed={changed}; records={}; wire=1; replay=0",
        after.workflow_audits.len()
    );
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    for server in &h.servers {
        runtime
            .block_on(server.session.close())
            .checked("close owned SSH session");
    }
    assert_eq!(h.servers[0].requests(), vec![expected]);
    assert!(h.servers[1].requests().is_empty());
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    assert!(
        saved,
        "sync modal Close must automatically save the exact completed receipt; observed pending={pending}, records={}, disk_changed={changed}",
        after.workflow_audits.len()
    );
    assert_eq!(after.workflow_audits, vec![audit]);
    let body = std::fs::read(h.fixture.store.path()).checked("sanitized durable audit body");
    assert!(
        !String::from_utf8(body)
            .checked("durable UTF8")
            .contains(source)
    );
}

fn unknown_record() -> keelshell_core::WorkflowAuditRecord {
    keelshell_core::WorkflowAuditRecord {
        id: uuid::Uuid::new_v4(),
        recorded_at: 1_725_000_000,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: vec![keelshell_core::WorkflowTaskAudit {
            id: uuid::Uuid::new_v4(),
            target_id: uuid::Uuid::new_v4(),
            outcome: WorkflowAuditOutcome::Unknown,
        }],
        cancelled: false,
        stopped_after_failure: false,
    }
}

fn publish_local(store: &std::sync::Arc<StateStore>, directory: &std::path::Path) {
    use keelshell_core::{ProfileSyncChoice, ProfileSyncService};
    let service = ProfileSyncService::new(store.clone());
    let password = "owned-audit-combination-sync-password";
    let review = service
        .inspect(
            directory.into(),
            zeroize::Zeroizing::new(password.into()),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .checked("actual encrypted channel inspection");
    let choices = review
        .rows()
        .iter()
        .map(|row| (row.id, ProfileSyncChoice::Local))
        .collect();
    assert!(
        service
            .apply(
                review,
                choices,
                zeroize::Zeroizing::new(password.into()),
                &std::sync::atomic::AtomicBool::new(false),
            )
            .checked("actual encrypted publication")
            .published
    );
}

fn hide_and_open_sync(h: &Harness, cx: &mut TestAppContext) {
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(false, window, cx));
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("actual hide action retains the production workflow entity");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_profile_sync(window, cx));
        window.render_frame(cx);
        assert!(window.find("profile-sync-panel").visible());
    })
    .checked("actual sync modal");
    cx.run_until_parked();
}

#[gpui_kit::test]
async fn independent_audit_sync_changed_refreshes_hidden_history_after_real_disable(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let temporary = tempfile::tempdir().checked("owned shared directory");
    let shared = temporary.path().join("shared");
    std::fs::create_dir(&shared).checked("owned channel directory");
    let first = unknown_record();
    let second = unknown_record();
    let mut state = h.fixture.store.load().checked("local baseline");
    state
        .record_workflow_audit(first.clone())
        .checked("initial metadata-only result");
    h.fixture
        .store
        .save(&state)
        .checked("initial durable result");
    publish_local(&h.fixture.store, &shared);
    let state = h.fixture.store.load().checked("configured local pairing");
    h.fixture.workspace.update(cx, |view, _| view.state = state);
    hide_and_open_sync(&h, cx);
    let workflow = panel(&h, cx);
    assert_eq!(
        workflow.read_with(cx, |panel, _| panel.audit_history_for_test()),
        vec![(first.id, true)]
    );
    let other = StateStore::new(h.fixture.store.path());
    let mut external = other.load().checked("legitimate concurrent store read");
    external
        .record_workflow_audit(second.clone())
        .checked("legitimate second device-local result");
    other.save(&external).checked("actual other-store write");
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-disable", cx);
    })
    .checked("actual Disable invokes service and Changed event");
    assert!(
        wait_real(&h, Duration::from_secs(8), cx, |cx| {
            let view = h.fixture.workspace.read(cx);
            view.state.workflow_audits == vec![first.clone(), second.clone()]
                && view
                    .state
                    .profile_sync
                    .as_ref()
                    .is_some_and(|sync| !sync.enabled())
        })
        .await,
        "actual Disable worker returns current state including both local histories"
    );
    let history = workflow.read_with(cx, |panel, _| panel.audit_history_for_test());
    let loaded = h.fixture.store.load().checked("actual disabled state");
    assert_eq!(loaded.workflow_audits, vec![first.clone(), second.clone()]);
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    for server in &h.servers {
        runtime
            .block_on(server.session.close())
            .checked("close owned SSH");
    }
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    eprintln!(
        "actual Changed: local disk/state histories=2; hidden viewer histories={}; SSH wire=0",
        history.len()
    );
    assert_eq!(
        history,
        vec![(first.id, true), (second.id, true)],
        "real Changed must refresh the retained read-only viewer"
    );
}

fn sync_input(window: &mut Window, id: &'static str, value: &str, cx: &mut App) {
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
    let AnyInputState::Input(field) = window.focused_input(cx).checked_option("actual sync input")
    else {
        panic!("sync uses InputState")
    };
    field.update(cx, |field, cx| {
        field.set_selected_range(0..field.value().len(), cx);
        field.replace(value.to_owned(), window, cx);
    });
}

#[gpui_kit::test]
async fn independent_audit_sync_review_and_close_conflict_preserve_other_store_and_pending_result(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let temporary = tempfile::tempdir().checked("owned peer and shared channel");
    let shared = temporary.path().join("shared");
    std::fs::create_dir(&shared).checked("owned channel directory");
    let peer = std::sync::Arc::new(StateStore::new(temporary.path().join("peer/state.json")));
    let mut peer_state = peer.load().checked("peer state");
    peer_state.connections.push(keelshell_core::Connection::new(
        "Awaiting explicit review",
        "review-only.fixture.invalid",
        "fixture",
    ));
    peer.save(&peer_state).checked("peer metadata only");
    publish_local(&peer, &shared);
    hide_and_open_sync(&h, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        sync_input(
            window,
            "profile-sync-directory-field",
            shared.to_str().checked_option("UTF8 path"),
            cx,
        );
        sync_input(
            window,
            "profile-sync-password-field",
            "owned-audit-combination-sync-password",
            cx,
        );
    })
    .checked("actual user directory and temporary password fields");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-sync-inspect", cx);
    })
    .checked("actual inspect before human approval");
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(18);
    loop {
        if cx
            .update_window(h.fixture.window, |_, window, cx| {
                window.render_frame(cx);
                window.try_find(("profile-sync-remote", 0_usize)).is_some()
            })
            .checked("actual decrypted review controls")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "actual inspect must finish before bounded review deadline"
        );
        let timer = runtime.spawn(async {
            tokio::time::sleep(Duration::from_millis(40)).await;
        });
        while !timer.is_finished() {
            cx.executor().timer(Duration::from_millis(10)).await;
        }
        runtime.block_on(timer).checked("owned real inspect timer");
        cx.run_until_parked();
    }
    let original = std::fs::read(h.fixture.store.path()).checked("review baseline bytes");
    let audit = unknown_record();
    panel(&h, cx).update(cx, |_, cx| {
        cx.emit(crate::workflow_commands::WorkflowPanelEvent::Completed(
            audit.clone(),
        ))
    });
    cx.run_until_parked();
    h.fixture.workspace.read_with(cx, |view, _| {
        assert!(view.profile_sync.is_some());
        assert!(!view.saving);
        assert_eq!(view.pending_workflow_audits, vec![audit.clone()]);
        assert!(view.state.workflow_audits.is_empty());
    });
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("unapproved review readback"),
        original
    );
    let other = StateStore::new(h.fixture.store.path());
    let mut external = other.load().checked("legitimate concurrent source");
    let concurrent = keelshell_core::Connection::new(
        "External preserved metadata",
        "preserved.fixture.invalid",
        "fixture",
    );
    let concurrent_id = concurrent.id;
    external.connections.push(concurrent);
    other
        .save(&external)
        .checked("actual concurrent replacement");
    let external_bytes = std::fs::read(h.fixture.store.path()).checked("external exact bytes");
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("profile-sync-remote", 0_usize)).is_some());
        window.click("profile-sync-close", cx);
    })
    .checked("close unapproved review through actual action");
    cx.run_until_parked();
    assert!(
        wait_real(&h, Duration::from_secs(8), cx, |cx| {
            let view = h.fixture.workspace.read(cx);
            view.profile_sync.is_none() && !view.saving
        })
        .await
    );
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("post-close exact bytes"),
        external_bytes
    );
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.pending_workflow_audits, vec![audit.clone()]);
        assert!(view.state.workflow_audits.is_empty());
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("command-workflow", cx);
    })
    .checked("reopen retained workflow");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-audit-history", cx);
        window.render_frame(cx);
        window.click("workflow-audit-save", cx);
    })
    .checked("actual user metadata retry still respects the stale snapshot");
    cx.run_until_parked();
    assert!(
        wait_real(&h, Duration::from_secs(8), cx, |cx| !h
            .fixture
            .workspace
            .read(cx)
            .saving)
        .await
    );
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("post-retry exact bytes"),
        external_bytes
    );
    let loaded = other.load().checked("independent final state readback");
    assert_eq!(loaded.connections.len(), 1);
    assert_eq!(loaded.connections[0].id, concurrent_id);
    assert!(loaded.workflow_audits.is_empty());
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.pending_workflow_audits, vec![audit.clone()])
    });
    assert_eq!(
        panel(&h, cx).read_with(cx, |panel, _| panel.audit_history_for_test()),
        vec![(audit.id, false)]
    );
    for server in &h.servers {
        runtime
            .block_on(server.session.close())
            .checked("close owned SSH");
    }
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    eprintln!(
        "actual review/close/retry: unapproved data never saved; other-store exact bytes preserved; pending=1; SSH wire=0; PTY=0"
    );
}
