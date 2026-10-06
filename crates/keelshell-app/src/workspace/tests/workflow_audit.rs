//! Production GPUI/history/save paths, isolated state only. No SSH or agent starts.
use super::*;
use keelshell_core::{
    WorkflowAuditOutcome, WorkflowAuditRecord, WorkflowAuditTrigger, WorkflowTaskAudit,
};

fn record() -> WorkflowAuditRecord {
    WorkflowAuditRecord {
        id: uuid::Uuid::new_v4(),
        recorded_at: 1_725_000_000,
        trigger: WorkflowAuditTrigger::Manual,
        tasks: vec![WorkflowTaskAudit {
            id: uuid::Uuid::new_v4(),
            target_id: uuid::Uuid::new_v4(),
            outcome: WorkflowAuditOutcome::Unknown,
        }],
        cancelled: true,
        stopped_after_failure: false,
    }
}

#[gpui_kit::test]
fn workflow_audit_save_failure_retry_deduplicates_and_restart_loads_only_history(
    cx: &mut TestAppContext,
) {
    let f = mount(cx, vec![]);
    let audit = record();
    let path = f.store.path().to_path_buf();
    let backup = path.with_extension("before-failure");
    std::fs::rename(&path, &backup).checked("preserve original state bytes");
    std::fs::create_dir(&path).checked("inject state path error");
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.open_workflow(false, window, cx);
            let panel = view
                .workflow_panel
                .clone()
                .checked_option("transient schedule draft");
            panel.update(cx, |panel, cx| {
                panel.schedule_grace_for_test().update(cx, |input, cx| {
                    input.set_value("UNSAVED-SCHEDULE-MARKER", window, cx)
                })
            });
            view.record_workflow_audit(audit.clone(), window, cx);
        })
    })
    .checked("queue actual audit save");
    cx.run_until_parked();
    f.workspace.read_with(cx, |view, cx| {
        assert!(!view.saving);
        assert_eq!(view.pending_workflow_audits, vec![audit.clone()]);
        let panel = view
            .workflow_panel
            .as_ref()
            .checked_option("history panel")
            .read(cx);
        assert_eq!(panel.audit_history_for_test(), vec![(audit.id, false)]);
        assert!(view.remote_sessions.is_empty());
    });
    std::fs::remove_dir(&path).checked("remove injected path error");
    std::fs::rename(&backup, &path).checked("restore exact state baseline");
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.record_workflow_audit(audit.clone(), window, cx)
        });
    })
    .checked("retry exact same result without execution");
    cx.run_until_parked();
    let loaded = f.store.load().checked("actual restart readback");
    assert_eq!(loaded.workflow_audits, vec![audit.clone()]);
    f.workspace.read_with(cx, |view, _| {
        assert!(view.pending_workflow_audits.is_empty())
    });
    let runtime = f.workspace.read_with(cx, |view, _| view.runtime.clone());
    let reopened = cx
        .update_window(f.window, |_, window, cx| {
            let store = f.store.clone();
            cx.new(|cx| Workspace::new(store, loaded, None, runtime, window, cx))
        })
        .checked("reconstruct workspace from persisted history");
    cx.update_window(f.window, |_, window, cx| {
        reopened.update(cx, |view, cx| view.open_workflow(false, window, cx))
    })
    .checked("read-only history after restart");
    reopened.read_with(cx, |view, cx| {
        let panel = view
            .workflow_panel
            .as_ref()
            .checked_option("reopened history panel")
            .read(cx);
        assert_eq!(panel.audit_history_for_test(), vec![(audit.id, true)]);
        assert!(!panel.is_running());
        assert!(!panel.has_schedule_for_test());
        assert!(panel.reviewed_for_test().is_none());
        assert!(panel.schedule_status_for_test().is_empty());
        assert_ne!(
            panel.schedule_grace_for_test().read(cx).value().as_str(),
            "UNSAVED-SCHEDULE-MARKER"
        );
        assert!(
            panel
                .parameter_for_test(audit.tasks[0].target_id, "CUSTOM_TOKEN")
                .is_none()
        );
        assert!(view.remote_sessions.is_empty());
    });
}

#[gpui_kit::test]
fn workflow_audit_modal_close_releases_save_lease_and_hidden_panel_keeps_pending(
    cx: &mut TestAppContext,
) {
    let f = mount(cx, vec![]);
    let audit = record();
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.open_snippet_editor(None, window, cx);
            view.record_workflow_audit(audit.clone(), window, cx);
            assert!(!view.saving);
            assert_eq!(view.pending_workflow_audits.len(), 1);
            view.show_workflow = false;
            view.close_snippet_modal(window, cx);
            assert!(view.saving);
        })
    })
    .checked("close actual modal and flush result metadata");
    cx.run_until_parked();
    assert_eq!(
        f.store
            .load()
            .checked("modal-close readback")
            .workflow_audits,
        vec![audit]
    );
    let next = record();
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.open_vault_settings(window, cx);
            view.record_workflow_audit(next.clone(), window, cx);
            assert!(!view.saving);
            let panel = view.vault_settings.clone().checked_option("vault modal");
            panel.update(cx, |_, cx| {
                cx.emit(crate::vault_settings::VaultSettingsEvent::Close { message: None })
            });
        })
    })
    .checked("vault close event releases actual metadata save lease");
    cx.run_until_parked();
    assert_eq!(
        f.store
            .load()
            .checked("vault-close readback")
            .workflow_audits
            .len(),
        2
    );
}

#[gpui_kit::test]
fn workflow_audit_history_buttons_are_read_only_in_both_locales(cx: &mut TestAppContext) {
    let f = mount(cx, vec![]);
    let audit = record();
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.record_workflow_audit(audit.clone(), window, cx);
            view.open_workflow(false, window, cx);
        })
    })
    .checked("seed task history and open real panel");
    cx.run_until_parked();
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(f.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            window.click("workflow-audit-history", cx);
            window.render_frame(cx);
            assert!(window.try_find("workflow-audit-history").is_some());
            assert!(window.try_find("workflow-confirm").is_none());
            window.click("workflow-audit-save", cx);
            window.render_frame(cx);
            window.click("workflow-audit-back", cx);
        })
        .checked("history is readable without execute/replay controls");
        cx.run_until_parked();
    }
    f.workspace.read_with(cx, |view, cx| {
        assert!(view.remote_sessions.is_empty());
        let panel = view
            .workflow_panel
            .as_ref()
            .checked_option("panel")
            .read(cx);
        assert!(!panel.is_running());
        assert_eq!(view.state.workflow_audits, vec![audit]);
    });
}

#[gpui_kit::test]
fn workflow_audit_each_finite_occurrence_is_sanitized_once_and_ledger_is_not_success_evidence(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{
        BatchTaskSpec, BatchWorkflowPlan, ScheduleClockSample, WorkflowAuditNotStarted,
        WorkflowScheduleBinding, WorkflowScheduleLedger, WorkflowScheduleOutcome,
        WorkflowScheduleSpec,
    };
    let f = mount(cx, vec![]);
    let epoch = 1_725_000_000;
    let task = uuid::Uuid::new_v4();
    let target = uuid::Uuid::new_v4();
    let plan = BatchWorkflowPlan::new(vec![BatchTaskSpec {
        id: task,
        target_id: target,
        command: "sensitive-fixture-command".into(),
        dependencies: vec![],
    }])
    .checked("pure reviewed graph");
    let sample = |ms: i64| ScheduleClockSample {
        utc_millis: epoch * 1000 + ms,
        monotonic: Duration::from_millis(ms.max(0) as u64),
    };
    let ledger = |count| {
        let id = uuid::Uuid::new_v4();
        let spec = WorkflowScheduleSpec::new(
            id,
            WorkflowScheduleBinding::new(id, 0, plan.review_token()).checked("pure binding"),
            epoch,
            0,
            1,
            Some(60),
            count,
        )
        .checked("finite schedule");
        WorkflowScheduleLedger::new(spec, sample(0)).checked("pure ledger")
    };
    let mut cancelled = ledger(32);
    cancelled.cancel();
    let mut missed = ledger(3);
    assert!(
        missed
            .tick(sample(3000))
            .checked("expired occurrence")
            .is_none()
    );
    missed.cancel();
    let mut busy = ledger(3);
    let ticket = busy
        .tick(sample(0))
        .checked("offered occurrence")
        .checked_option("due ticket");
    busy.claim(&ticket, sample(0))
        .checked("claim pure ledger ticket");
    assert!(
        busy.tick(sample(60000))
            .checked("busy occurrence")
            .is_none()
    );
    busy.finish(&ticket, WorkflowScheduleOutcome::Succeeded)
        .checked("pure finished ledger");
    busy.cancel();
    let mut invalidated = ledger(3);
    assert!(
        invalidated
            .tick(ScheduleClockSample {
                utc_millis: epoch * 1000 - 1,
                monotonic: Duration::from_millis(1)
            })
            .is_err()
    );
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.open_workflow(false, window, cx);
            view.show_workflow = false;
            view.open_snippet_editor(None, window, cx);
            assert!(
                view.snippet_modal_open(),
                "hold a real modal save lease before emitting receipts"
            );
            let panel = view
                .workflow_panel
                .clone()
                .checked_option("production workflow panel");
            for ledger in [cancelled, missed, busy, invalidated] {
                panel.update(cx, |panel, cx| {
                    panel.install_audit_ledger_for_test(ledger, vec![(task, target)], cx)
                });
            }
        })
    })
    .checked("project terminal occurrences through real panel events");
    cx.run_until_parked();
    f.workspace.read_with(cx,|view,_| {
        assert_eq!(view.pending_workflow_audits.len(),40,"32 cancelled + 3 expired/cancelled + 2 busy/cancelled + 3 invalidated; finished success has no task receipt");
        let tasks:Vec<_>=view.pending_workflow_audits.iter().flat_map(|record|&record.tasks).collect();
        assert!(!tasks.iter().any(|task|matches!(task.outcome,WorkflowAuditOutcome::Succeeded)));
        assert_eq!(tasks.iter().filter(|task|matches!(task.outcome,WorkflowAuditOutcome::NotStarted{reason:WorkflowAuditNotStarted::ScheduleMissed})).count(),1);
        assert_eq!(tasks.iter().filter(|task|matches!(task.outcome,WorkflowAuditOutcome::NotStarted{reason:WorkflowAuditNotStarted::ScheduleBusy})).count(),1);
        assert_eq!(tasks.iter().filter(|task|matches!(task.outcome,WorkflowAuditOutcome::NotStarted{reason:WorkflowAuditNotStarted::ScheduleInvalidated})).count(),3);
    });
    cx.update_window(f.window, |_, window, cx| {
        f.workspace
            .update(cx, |view, cx| view.close_snippet_modal(window, cx))
    })
    .checked("flush only sanitized occurrence notes");
    cx.run_until_parked();
    let loaded = f
        .store
        .load()
        .checked("read complete finite occurrence notes");
    assert_eq!(loaded.workflow_audits.len(), 40);
    let text = std::fs::read_to_string(f.store.path()).checked("read audit state bytes");
    assert!(!text.contains("sensitive-fixture-command"));
}

#[gpui_kit::test]
fn workflow_audit_pending_budget_rejects_conflicting_old_receipts_after_fifo_projection(
    cx: &mut TestAppContext,
) {
    let f = mount(cx, vec![]);
    let mut entries: Vec<_> = (0..100).map(|_| record()).collect();
    // A held metadata-save lease is modeled here; no filesystem worker is
    // started. It exercises the bounded queue independently of disk retention.
    for entry in &mut entries {
        entry.tasks = (0..128)
            .map(|_| WorkflowTaskAudit {
                id: uuid::Uuid::new_v4(),
                target_id: entry.tasks[0].target_id,
                outcome: WorkflowAuditOutcome::Unknown,
            })
            .collect();
    }
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.open_workflow(false, window, cx);
            view.saving = true;
            view.pending_workflow_audits = entries.clone();
            view.refresh_workflow_audit_history(cx);
            let mut conflict = entries[0].clone();
            conflict.tasks[0].outcome = WorkflowAuditOutcome::Succeeded;
            view.record_workflow_audit(conflict, window, cx);
            assert_eq!(view.pending_workflow_audits, entries);
            view.record_workflow_audit(record(), window, cx);
            assert_eq!(view.pending_workflow_audits, entries);
            assert!(
                view.workflow_panel
                    .as_ref()
                    .checked_option("bounded audit panel")
                    .read(cx)
                    .audit_backpressure_for_test()
            );
            assert!(view.remote_sessions.is_empty());
            view.saving = false;
        })
    })
    .checked("bounded pending queue and conflicting receipt guard");
}

#[gpui_kit::test]
fn workflow_audit_long_id_and_dependency_results_wrap_in_compact_themes_and_locales(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, vec![], 900., 580.);
    let mut audit = record();
    let prerequisite = audit.tasks[0].id;
    audit.tasks[0].outcome = WorkflowAuditOutcome::Failed {
        exit_code: u32::MAX,
    };
    audit.tasks.push(WorkflowTaskAudit {
        id: uuid::Uuid::new_v4(),
        target_id: audit.tasks[0].target_id,
        outcome: WorkflowAuditOutcome::DependencyBlocked {
            dependency: prerequisite,
        },
    });
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |view, cx| {
            view.record_workflow_audit(audit, window, cx);
            view.open_workflow(false, window, cx);
        })
    })
    .checked("compact real history panel");
    cx.run_until_parked();
    for language in [Language::ZhCn, Language::En] {
        for theme in [
            keelshell_core::Theme::System,
            keelshell_core::Theme::Light,
            keelshell_core::Theme::Dark,
        ] {
            cx.update_window(f.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                window.click("workflow-audit-history", cx);
                window.render_frame(cx);
                let row = window.find(("workflow-audit-task", 1usize)).bounds();
                let details = window.find("workflow-audit-details").bounds();
                assert!(
                    row.size.height > px(24.),
                    "long IDs and full dependency label wrap rather than clip"
                );
                assert!(
                    row.origin.x >= details.origin.x
                        && row.origin.x + row.size.width
                            <= details.origin.x + details.size.width + px(1.)
                );
                assert!(window.find("workflow-audit-back").visible());
                assert!(window.try_find("workflow-confirm").is_none());
                window.click("workflow-audit-back", cx);
            })
            .checked("compact history remains readable with fixed safe controls");
        }
    }
}
