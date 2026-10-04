//! Production GPUI panel plus isolated TCP/SSH fixtures; no shell interprets input.
use super::{ConnectedDestination, Destination, WorkflowPanel, WorkflowReview};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Focusable, TestAppContext, WindowBounds,
    WindowOptions, point, px, size,
};
use keelshell_core::{BatchTaskSkipReason, Language};
use keelshell_session::{
    BatchOutcome, BatchUnknownReason, WorkflowReceipt, WorkflowTaskReceipt, WorkflowTaskResult,
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

use crate::workspace::tests::batch_peer as peer;

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|error| panic!("{operation}: {error:?}"))
    }
}
struct Harness {
    window: AnyWindowHandle,
    panel: Entity<WorkflowPanel>,
    runtime: Arc<tokio::runtime::Runtime>,
    servers: Vec<peer::Server>,
    _entities: Vec<Entity<usize>>,
}
impl Harness {
    fn new(cx: &mut TestAppContext, codes: &[u32], width: f32, height: f32) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("fixture runtime"),
        );
        let servers = codes
            .iter()
            .map(|code| peer::Server::new(&runtime, *code))
            .collect::<Vec<_>>();
        let (window, panel, entities) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(Language::En, cx);
            let entities = (0..codes.len())
                .map(|index| cx.new(|_| index))
                .collect::<Vec<_>>();
            let destinations = entities
                .iter()
                .zip(&servers)
                .enumerate()
                .map(|(index, (entity, server))| ConnectedDestination {
                    destination: Destination {
                        id: Uuid::new_v4(),
                        profile_id: None,
                        entity: entity.entity_id(),
                        name: format!("Connected {index}"),
                        endpoint: format!("fixture-{index}@example.invalid:22"),
                        route: format!("direct → fixture-{index}@example.invalid:22"),
                        template_context: keelshell_core::BatchTargetContext {
                            name: format!("Connected {index}"),
                            host: "example.invalid".into(),
                            port: "22".into(),
                            user: format!("fixture-{index}"),
                            endpoint: format!("fixture-{index}@example.invalid:22"),
                        },
                    },
                    session: server.session.clone(),
                })
                .collect();
            let (window, panel) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(width), px(height)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| WorkflowPanel::new(destinations, "safe".into(), window, cx))
                },
            )
            .checked("mount workflow panel");
            (window, panel, entities)
        });
        cx.update_window(window, |_, window, _| window.activate_window())
            .checked("activate workflow window");
        Self {
            window,
            panel,
            runtime,
            servers,
            _entities: entities,
        }
    }
    fn graph(&self, cx: &mut TestAppContext, tasks: &[(&str, usize, &[usize])], stop: bool) {
        cx.update_window(self.window, |_, window, cx| {
            self.panel.update(cx, |panel, cx| {
                while panel.tasks.len() < tasks.len() {
                    panel.add_task(String::new(), window, cx);
                }
                let ids = panel.tasks.iter().map(|task| task.id).collect::<Vec<_>>();
                for (index, (command, target, deps)) in tasks.iter().enumerate() {
                    panel.tasks[index]
                        .command
                        .update(cx, |field, cx| field.set_value(*command, window, cx));
                    panel.tasks[index].target =
                        Some(panel.targets[*target].connected.destination.id);
                    panel.tasks[index].dependencies =
                        deps.iter().map(|index| ids[*index]).collect();
                }
                panel.stop_after_failure = stop;
                panel.selected = ids.first().copied();
                panel.changed(cx);
            })
        })
        .checked("configure transient dependency graph");
    }
    fn review(&self, cx: &mut TestAppContext) -> WorkflowReview {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("workflow-review-button", cx);
        })
        .checked("actual review button");
        cx.run_until_parked();
        self.panel.read_with(cx, |panel, _| {
            panel
                .review
                .clone()
                .unwrap_or_else(|| panic!("validated review"))
        })
    }
    fn start(&self, review: &WorkflowReview, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("workflow-confirm", cx);
            self.panel.update(cx, |panel, cx| {
                panel.begin(review, &self.runtime, window, cx)
            });
        })
        .checked("human confirmation and captured-session dispatcher");
        cx.run_until_parked();
    }
    async fn complete(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(8), |_, cx| {
            self.panel.read(cx).complete
        })
        .await;
    }
    fn receipt(&self, index: usize, cx: &mut TestAppContext) -> Arc<WorkflowTaskReceipt> {
        self.panel.read_with(cx, |panel, _| {
            let id = panel.tasks[index].id;
            panel
                .progress
                .iter()
                .find(|row| row.id == id)
                .and_then(|row| row.receipt.clone())
                .unwrap_or_else(|| panic!("complete receipt"))
        })
    }
}

#[gpui_kit::test]
async fn workflow_complete_review_preserves_exact_templates_and_releases_only_success(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[0, 7], 960., 720.);
    h.graph(
        cx,
        &[
            ("printf {{endpoint}}\nprintf '中文'", 0, &[]),
            ("dependent", 0, &[0]),
            ("fails", 1, &[]),
            ("must-not-run", 1, &[2]),
        ],
        false,
    );
    let review = h.review(cx);
    assert_eq!(review.destinations.len(), 2);
    assert!(
        review
            .plan
            .tasks()
            .iter()
            .any(|task| task.command == "printf 'fixture-0@example.invalid:22'\nprintf '中文'")
    );
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.start(&review, cx);
    h.complete(cx).await;
    assert_eq!(
        h.servers[0].requests(),
        vec![
            b"printf 'fixture-0@example.invalid:22'\nprintf '\xe4\xb8\xad\xe6\x96\x87'".to_vec(),
            b"dependent".to_vec()
        ]
    );
    assert_eq!(h.servers[1].requests(), vec![b"fails".to_vec()]);
    assert!(matches!(
        h.receipt(3, cx).result,
        WorkflowTaskResult::Skipped {
            reason: BatchTaskSkipReason::DependencyNotSucceeded { .. }
        }
    ));
    h.panel.read_with(cx, |panel, cx| {
        assert!(
            panel.progress.iter().all(|row| row.receipt.is_some())
                && panel.receipt_matches(&WorkflowReceipt {
                    tasks: review
                        .plan
                        .tasks()
                        .iter()
                        .map(|task| panel
                            .progress
                            .iter()
                            .find(|row| row.id == task.id)
                            .and_then(|row| row.receipt.clone())
                            .unwrap_or_else(|| panic!("all rows")))
                        .collect(),
                    cancelled: false,
                    stopped_after_failure: false,
                    fingerprint: review.plan.review_token(),
                    options: review.options,
                })
                && panel
                    .status(panel.tasks[3].id, cx)
                    .0
                    .contains("prerequisite")
        )
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("workflow-task-select", 0_usize), cx);
        window.render_frame(cx);
        window.click("workflow-copy-output", cx);
    })
    .checked("inspect full captured task output");
    let copied = cx.update(|cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    });
    assert!(
        copied.contains("fixture stdout")
            && copied.contains("\\u{001b}")
            && !copied.contains('\u{1b}')
    );
}

#[gpui_kit::test]
async fn workflow_unknown_timeout_and_cancellation_never_release_descendants(
    cx: &mut TestAppContext,
) {
    for cancel in [false, true] {
        let h = Harness::new(cx, &[0], 960., 720.);
        h.graph(cx, &[("hold", 0, &[]), ("must-not-run", 0, &[0])], false);
        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                panel
                    .timeout
                    .update(cx, |field, cx| field.set_value("1", window, cx))
            })
        })
        .checked("bounded timeout");
        let review = h.review(cx);
        h.start(&review, cx);
        cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
            !h.servers[0].requests().is_empty()
        })
        .await;
        if cancel {
            cx.update_window(h.window, |_, window, cx| {
                window.render_frame(cx);
                window.click("workflow-cancel", cx);
            })
            .checked("explicit cancellation");
        }
        h.complete(cx).await;
        assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
        assert!(
            matches!(&h.receipt(0,cx).result,WorkflowTaskResult::Transport {row} if matches!(row.outcome,BatchOutcome::Unknown {reason:BatchUnknownReason::Cancelled|BatchUnknownReason::Timeout}))
        );
        assert!(matches!(
            h.receipt(1, cx).result,
            WorkflowTaskResult::Skipped { .. }
        ));
        let previous = h.panel.read_with(cx, |panel, _| panel.run_id);
        h.panel
            .update(cx, |panel, cx| assert!(!panel.poll(Uuid::new_v4(), cx)));
        h.panel
            .read_with(cx, |panel, _| assert_eq!(panel.run_id, previous));
    }
}

#[gpui_kit::test]
fn workflow_invalid_drafts_cycles_and_programmatic_edits_fail_closed(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[0], 960., 720.);
    h.graph(cx, &[("one", 0, &[1]), ("two", 0, &[0])], true);
    h.panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.review.is_none());
        assert!(panel.message.is_some());
    });
    h.graph(cx, &[("one", 0, &[]), ("two", 0, &[0])], true);
    let review = h.review(cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.tasks[0]
                .command
                .update(cx, |field, cx| field.set_value("changed", window, cx));
            assert!(!panel.review_current(&review, cx));
            panel.confirm(cx);
            assert!(!panel.starting);
        })
    })
    .checked("programmatic set_value cannot inherit old review");
    h.panel.update(cx, |panel, cx| panel.back(cx));
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel
                .concurrency
                .update(cx, |field, cx| field.set_value("invalid 中文", window, cx));
            crate::i18n::set_language(Language::ZhCn, cx);
            panel.refresh_locale(cx);
            assert_eq!(panel.concurrency.read(cx).value(), "invalid 中文");
            assert_eq!(panel.tasks[0].command.read(cx).value(), "changed");
            panel.prepare(cx);
            assert!(panel.review.is_none());
        })
    })
    .checked("invalid raw drafts survive locale and block dispatch");
    assert!(h.servers[0].requests().is_empty());
}

#[gpui_kit::test]
fn workflow_same_endpoint_replacement_invalidates_review_and_requires_explicit_target_refresh(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[0], 960., 720.);
    h.graph(cx, &[("safe", 0, &[])], true);
    let _ = h.review(cx);
    let replacement = peer::Server::new(&h.runtime, 0);
    h.panel.update(cx, |panel, cx| {
        let destination = panel.targets[0].connected.destination.clone();
        let old = destination.id;
        let replacement = ConnectedDestination {
            destination: Destination {
                id: Uuid::new_v4(),
                ..destination
            },
            session: replacement.session.clone(),
        };
        panel.update_available(std::slice::from_ref(&replacement), cx);
        assert!(panel.review.is_none() && !panel.starting && !panel.targets[0].available);
        panel.refresh_targets(vec![replacement], cx);
        assert_eq!(panel.tasks[0].target, Some(old));
        assert_eq!(panel.targets.len(), 2);
        panel.prepare(cx);
        assert!(panel.review.is_none());
    });
    assert!(h.servers[0].requests().is_empty() && replacement.requests().is_empty());
}

#[gpui_kit::test]
async fn workflow_stop_policy_and_hidden_running_panel_keep_ownership_and_task_receipts(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[7, 0], 960., 720.);
    h.graph(
        cx,
        &[
            ("fails", 0, &[]),
            ("independent", 1, &[]),
            ("dependent", 1, &[0]),
        ],
        true,
    );
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel
                .concurrency
                .update(cx, |field, cx| field.set_value("1", window, cx))
        })
    })
    .checked("serialize policy admissions");
    let review = h.review(cx);
    h.start(&review, cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide without replacing run");
    h.complete(cx).await;
    assert_eq!(h.servers[0].requests(), vec![b"fails".to_vec()]);
    assert!(matches!(
        h.receipt(2, cx).result,
        WorkflowTaskResult::Skipped { .. }
    ));
    // Topological UUID tie order may admit the independent branch first, but
    // once failure is observed no pending replacement can be admitted.
    assert!(h.servers[1].requests().len() <= 1);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.complete && panel.progress.len() == 3)
    });
}

#[gpui_kit::test]
fn workflow_task_controls_edit_real_drafts_and_remove_edges(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[0], 960., 720.);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-add-task", cx);
        h.panel.update(cx, |panel, cx| {
            let first = panel.tasks[0].id;
            panel.tasks[1].dependencies.push(first);
            panel.remove_task(first, cx);
            assert_eq!(panel.tasks.len(), 1);
            assert!(panel.tasks[0].dependencies.is_empty());
        });
        window.render_frame(cx);
        window.click("workflow-task-name", cx);
        window.input("检查服务", cx);
    })
    .checked("actual task addition/name editing and explicit removal");
    cx.run_until_parked();
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.tasks[0].name.read(cx).value(), "检查服务")
    });
}

#[gpui_kit::test]
fn workflow_long_review_fixed_footer_survives_small_window_locale_and_maximum_graph(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(960., 720.), (760., 560.), (480., 480.)] {
        let h = Harness::new(cx, &[0], width, height);
        cx.update_window(h.window, |_, window, cx| {
            h.panel.update(cx, |panel, cx| {
                for _ in 1..128 {
                    panel.add_task("printf 'long review'\n".repeat(6), window, cx);
                }
                let target = panel.targets[0].connected.destination.id;
                for task in &mut panel.tasks {
                    task.target = Some(target);
                }
            })
        })
        .checked("maximum task count with long review body");
        let review = h.review(cx);
        assert_eq!(review.plan.tasks().len(), 128);
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                h.panel.update(cx, |panel, cx| panel.refresh_locale(cx));
                window.render_frame(cx);
                for id in [
                    "workflow-footer",
                    "workflow-confirm",
                    "workflow-hide",
                    "workflow-back",
                ] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.size.height > px(0.)
                            && bounds.origin.x >= px(0.)
                            && bounds.origin.y >= px(0.)
                            && bounds.right() <= px(width)
                            && bounds.bottom() <= px(height),
                        "{id}: {bounds:?}"
                    );
                }
                assert!(
                    window
                        .try_find(("workflow-reviewed-task", 127_usize))
                        .is_some()
                );
                h.panel
                    .read_with(cx, |panel, cx| assert!(panel.review_current(&review, cx)));
            })
            .checked("complete review remains scrollable and footer stays inside window");
        }
        assert!(h.servers[0].requests().is_empty());
    }
}

#[gpui_kit::test]
fn workflow_input_change_revokes_review_and_preserves_entity_values(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[0], 960., 720.);
    h.graph(cx, &[("safe", 0, &[])], true);
    let review = h.review(cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.back(cx);
            let revision = panel.revision;
            panel.concurrency.update(cx, |field, cx| {
                field.focus_handle(cx).focus(window, cx);
                field.set_selected_range(0..field.value().len(), cx);
                field.replace("3", window, cx);
            });
            assert!(!panel.review_current(&review, cx));
            assert!(panel.revision >= revision);
        })
    })
    .checked("user edit uses real InputEvent path");
    cx.run_until_parked();
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.concurrency.read(cx).value(), "3")
    });
}

#[gpui_kit::test]
async fn workflow_live_target_loss_cancels_waits_without_rebinding_or_late_result_mutation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[0], 960., 720.);
    h.graph(cx, &[("hold", 0, &[]), ("must-not-run", 0, &[0])], false);
    let review = h.review(cx);
    h.start(&review, cx);
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    let replacement = peer::Server::new(&h.runtime, 0);
    h.panel.update(cx, |panel, cx| {
        let destination = panel.targets[0].connected.destination.clone();
        assert!(!panel.poll(Uuid::new_v4(), cx));
        panel.update_available(
            &[ConnectedDestination {
                destination,
                session: replacement.session.clone(),
            }],
            cx,
        );
        assert!(panel.is_running() && panel.cancelling);
    });
    h.complete(cx).await;
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
    assert!(replacement.requests().is_empty());
    assert!(
        matches!(&h.receipt(0,cx).result,WorkflowTaskResult::Transport {row} if matches!(row.outcome,BatchOutcome::Unknown {..}))
    );
    assert!(matches!(
        h.receipt(1, cx).result,
        WorkflowTaskResult::Skipped { .. }
    ));
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.progress.iter().all(|row| row.receipt.is_some()) && panel.complete)
    });
}

#[gpui_kit::test]
fn workflow_execution_options_edges_and_target_metadata_are_bound_to_the_complete_review(
    cx: &mut TestAppContext,
) {
    for change in ["option", "edge", "name"] {
        let h = Harness::new(cx, &[0], 960., 720.);
        h.graph(cx, &[("one", 0, &[]), ("two", 0, &[0])], true);
        let review = h.review(cx);
        h.panel.update(cx, |panel, cx| {
            match change {
                "option" => panel.stop_after_failure = false,
                "edge" => panel.tasks[1].dependencies.clear(),
                _ => {
                    let mut destination = panel.targets[0].connected.destination.clone();
                    destination.name = "changed display name".into();
                    let session = panel.targets[0].connected.session.clone();
                    panel.update_available(
                        &[ConnectedDestination {
                            destination,
                            session,
                        }],
                        cx,
                    );
                    assert!(panel.review.is_none());
                }
            }
            assert!(!panel.review_current(&review, cx));
            panel.confirm(cx);
            assert!(!panel.is_running());
        });
        assert!(h.servers[0].requests().is_empty());
    }
}
