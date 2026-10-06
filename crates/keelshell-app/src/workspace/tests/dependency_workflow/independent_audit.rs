//! Non-author review: actual UI, loopback SSH and isolated durable state.
use super::*;
use keelshell_core::{
    Theme, WorkflowAuditOutcome, WorkflowAuditRecord, WorkflowAuditTrigger, WorkflowTaskAudit,
    format_fixed_offset_datetime,
};
fn panel(h: &Harness, cx: &impl AppContext) -> Entity<crate::workflow_commands::WorkflowPanel> {
    h.fixture.workspace.read_with(cx, |view, _| {
        view.workflow_panel
            .clone()
            .checked_option("actual workflow entity")
    })
}
async fn wait_real(
    h: &Harness,
    timeout: Duration,
    cx: &mut TestAppContext,
    mut ready: impl FnMut(&mut App) -> bool,
) {
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
            .checked("real audit transition")
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "actual wall-clock audit deadline {timeout:?}"
        );
        let timer = runtime.spawn(async { tokio::time::sleep(Duration::from_millis(40)).await });
        while !timer.is_finished() {
            cx.executor().timer(Duration::from_millis(10)).await;
        }
        runtime
            .block_on(timer)
            .checked("owned real timer completion");
        cx.run_until_parked();
    }
}
fn input(window: &mut Window, id: &'static str, value: &str, cx: &mut App) {
    click_visible(window, id, cx);
    match window
        .focused_input(cx)
        .checked_option("real focused schedule input")
    {
        AnyInputState::Input(field) => field.update(cx, |field, cx| {
            field.set_selected_range(0..field.value().len(), cx);
            field.replace(value.to_owned(), window, cx);
        }),
        _ => panic!("actual schedule Input"),
    }
}
fn sample_record() -> WorkflowAuditRecord {
    WorkflowAuditRecord {
        id: uuid::Uuid::new_v4(),
        recorded_at: 1_725_000_000,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: vec![WorkflowTaskAudit {
            id: uuid::Uuid::new_v4(),
            target_id: uuid::Uuid::new_v4(),
            outcome: WorkflowAuditOutcome::Unknown,
        }],
        cancelled: false,
        stopped_after_failure: false,
    }
}
#[gpui_kit::test]
async fn independent_audit_real_parameterized_due_receipt_then_cancel_is_sanitized_and_read_only(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let secret = "独立审查'私密\nvalue-audit-91";
    let source = "printf {{audit_marker}} {{endpoint}}";
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.set_reviewed_command(
                source.into(),
                Some(h.panes[0].terminal.entity_id()),
                window,
                cx,
            );
            view.open_workflow(false, window, cx);
        });
        click_visible(window, ("workflow-target", 0_usize), cx);
        click_visible(window, "workflow-sync-parameters", cx);
        let target = panel(&h, cx)
            .read(cx)
            .connected_destinations()
            .next()
            .checked_option("captured authenticated target")
            .destination
            .id;
        click_visible(
            window,
            format!("workflow-parameters-{target}-audit_marker"),
            cx,
        );
        let actual = match window
            .focused_input(cx)
            .checked_option("actual focused custom Textarea")
        {
            AnyInputState::Textarea(field) => field,
            _ => panic!("custom Textarea"),
        };
        assert_eq!(
            actual.entity_id(),
            panel(&h, cx)
                .read(cx)
                .parameter_for_test(target, "audit_marker")
                .checked_option("same parameter entity")
                .entity_id()
        );
        replace(window, secret, cx);
        click_visible(window, "workflow-schedule-interval", cx);
        input(window, "workflow-schedule-offset", "+00:00", cx);
        input(
            window,
            "workflow-schedule-start",
            &format_fixed_offset_datetime(chrono::Utc::now().timestamp() + 4, 0)
                .checked("future calendar"),
            cx,
        );
        input(window, "workflow-schedule-grace", "15", cx);
        input(window, "workflow-schedule-period", "60", cx);
        input(window, "workflow-schedule-count", "2", cx);
    })
    .checked("actual target, parameter and finite schedule editor");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-review-button", cx);
    })
    .checked("human full review");
    cx.run_until_parked();
    let review = panel(&h, cx).read_with(cx, |panel, _| {
        panel
            .reviewed_for_test()
            .checked_option("actual reviewed parameterized schedule")
    });
    let expected = review.plan.tasks()[0].command.as_bytes().to_vec();
    assert!(
        String::from_utf8(expected.clone())
            .checked("UTF-8 final command")
            .contains("value-audit-91")
    );
    let schedule_id = review
        .schedule
        .as_ref()
        .checked_option("finite schedule")
        .schedule_id();
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    let remaining_millis = review
        .schedule
        .as_ref()
        .checked_option("schedule deadline")
        .first_utc_seconds()
        * 1000
        - chrono::Utc::now().timestamp_millis();
    let started = std::time::Instant::now();
    h.confirm(cx);
    wait_real(&h, Duration::from_secs(15), cx, |cx| {
        h.fixture.workspace.read(cx).state.workflow_audits.len() == 1
    })
    .await;
    assert!(
        started.elapsed()
            >= Duration::from_millis(remaining_millis.saturating_sub(100).max(0) as u64),
        "real timer cannot be accelerated by test executor"
    );
    assert_eq!(h.servers[0].requests(), vec![expected.clone()]);
    assert!(h.servers[1].requests().is_empty());
    let first = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.state.workflow_audits[0].clone());
    assert_eq!(first.tasks[0].outcome, WorkflowAuditOutcome::Succeeded);
    assert!(
        matches!(first.trigger,WorkflowAuditTrigger::Scheduled{schedule_id:s,occurrence:0,..}if s==schedule_id)
    );
    let original_entity = panel(&h, cx).entity_id();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide active finite schedule");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(false, window, cx));
        assert_eq!(panel(&h, cx).entity_id(), original_entity);
        window.render_frame(cx);
        window.click("workflow-cancel", cx);
    })
    .checked("reopen retained schedule and cancel remaining occurrence");
    wait_real(&h, Duration::from_secs(8), cx, |cx| {
        let view = h.fixture.workspace.read(cx);
        !view.saving
            && view.pending_workflow_audits.is_empty()
            && view.state.workflow_audits.len() == 2
    })
    .await;
    let loaded = StateStore::new(h.fixture.store.path())
        .load()
        .checked("independent durable readback");
    assert_eq!(loaded.workflow_audits[0], first);
    let second = &loaded.workflow_audits[1];
    assert!(
        matches!(second.trigger,WorkflowAuditTrigger::Scheduled{schedule_id:s,occurrence:1,..}if s==schedule_id)
    );
    assert_eq!(second.tasks[0].outcome, WorkflowAuditOutcome::Cancelled);
    assert_ne!(first.id, second.id);
    let text =
        String::from_utf8(std::fs::read(h.fixture.store.path()).checked("actual durable body"))
            .checked("stored JSON");
    for excluded in [
        secret,
        source,
        "value-audit-91",
        "fixture stdout",
        "fixture stderr",
        "printf",
        "audit_marker",
    ] {
        assert!(
            !text.contains(excluded),
            "private execution data persisted: {excluded}"
        );
    }
    cx.simulate_window_resize(h.fixture.window, size(px(900.), px(580.)));
    cx.run_until_parked();
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                window.click("workflow-audit-history", cx);
                window.render_frame(cx);
                assert!(window.try_find("workflow-confirm").is_none());
                assert!(window.try_find("workflow-cancel").is_none());
                window.click("workflow-audit-save", cx);
                window.render_frame(cx);
                window.click("workflow-audit-back", cx);
            })
            .checked("read-only viewer both locales/three themes/minimum window");
            cx.run_until_parked();
        }
    }
    assert_eq!(h.servers[0].requests(), vec![expected]);
    assert!(h.servers[1].requests().is_empty());
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    for server in &h.servers {
        runtime
            .block_on(server.session.close())
            .checked("close owned authenticated SSH");
    }
    eprintln!(
        "independent audit: actual first due wire=1; future cancellation=1; durable records=2; history/retry wire delta=0; PTY=0"
    );
}
#[gpui_kit::test]
async fn independent_audit_external_store_conflict_preserves_unsaved_receipt_and_never_replays(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.prepare("printf unchanged-draft", cx);
    let external = StateStore::new(h.fixture.store.path());
    let mut state = external.load().checked("external writer baseline");
    state.settings.language = Language::En;
    external
        .save(&state)
        .checked("external process actual valid update");
    let external_bytes = std::fs::read(h.fixture.store.path()).checked("preserve external bytes");
    let audit = sample_record();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.record_workflow_audit(audit.clone(), window, cx)
        });
    })
    .checked("actual stale audit save");
    wait_real(&h, Duration::from_secs(8), cx, |cx| {
        !h.fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("external unchanged"),
        external_bytes
    );
    assert_eq!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.pending_workflow_audits.clone()),
        vec![audit.clone()]
    );
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide failed history save");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(true, window, cx));
        window.render_frame(cx);
        window.click("workflow-audit-history", cx);
        window.render_frame(cx);
        window.click("workflow-audit-save", cx);
    })
    .checked("new panel retains queue; actual UI retry saves only metadata");
    wait_real(&h, Duration::from_secs(8), cx, |cx| {
        !h.fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(h.fixture.store.path()).checked("conflict retry unchanged"),
        external_bytes
    );
    h.fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(view.pending_workflow_audits, vec![audit.clone()]);
        assert!(view.state.workflow_audits.is_empty());
        assert_eq!(
            panel(&h, cx).read(cx).audit_history_for_test(),
            vec![(audit.id, false)]
        );
    });
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    let runtime = h
        .fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    for server in &h.servers {
        runtime
            .block_on(server.session.close())
            .checked("close conflict fixture SSH");
    }
    eprintln!(
        "independent audit: external conflict retains exact bytes/pending receipt; actual UI retry remote wire=0"
    );
}
