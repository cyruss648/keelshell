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
impl<T> Checked<T> for Option<T> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|| panic!("{operation}: missing value"))
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
    fn parameters(&self, cx: &mut TestAppContext, values: &[&[(&str, &str)]]) {
        cx.update_window(self.window, |_, window, cx| {
            self.panel.update(cx, |panel, cx| {
                panel.sync_parameters(window, cx);
                for (row, values) in panel.targets.iter_mut().zip(values) {
                    for (name, value) in *values {
                        let field = row
                            .parameters
                            .iter_mut()
                            .find(|field| field.name == *name)
                            .checked("required target parameter field");
                        field
                            .value
                            .update(cx, |input, cx| input.set_value(*value, window, cx));
                        field.allow_empty = value.is_empty();
                    }
                }
                panel.changed(cx);
            });
        })
        .checked("explicit transient target parameter mapping");
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
async fn workflow_target_parameters_bind_distinct_literals_and_shared_target_task_union(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[0, 0], 960., 720.);
    h.graph(
        cx,
        &[
            ("printf '%s' {{path}}", 0, &[]),
            ("echo {{release}} {{host}}", 0, &[0]),
            ("printf '%s' {{path}}", 1, &[]),
        ],
        false,
    );
    h.parameters(
        cx,
        &[
            &[("path", "中文'$(no)"), ("release", "v1; no")],
            &[("path", "other\n\tpath")],
        ],
    );
    let review = h.review(cx);
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    let expected = [
        "printf '%s' '中文'\\''$(no)'",
        "echo 'v1; no' 'example.invalid'",
        "printf '%s' 'other\n\tpath'",
    ];
    h.panel.read_with(cx, |panel, _| {
        for (task, expected) in panel.tasks.iter().zip(expected) {
            assert_eq!(
                review
                    .plan
                    .tasks()
                    .iter()
                    .find(|spec| spec.id == task.id)
                    .checked("review task")
                    .command,
                expected
            );
        }
    });
    h.start(&review, cx);
    h.complete(cx).await;
    assert_eq!(
        h.servers[0].requests(),
        vec![
            expected[0].as_bytes().to_vec(),
            expected[1].as_bytes().to_vec()
        ]
    );
    assert_eq!(
        h.servers[1].requests(),
        vec![expected[2].as_bytes().to_vec()]
    );
}

#[gpui_kit::test]
fn workflow_missing_empty_unused_and_edited_parameter_values_require_new_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx, &[0], 960., 720.);
    h.graph(cx, &[("echo {{path}}", 0, &[])], false);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.prepare(cx);
            assert!(panel.review.is_none());
            panel.sync_parameters(window, cx);
            panel.prepare(cx);
            assert!(panel.review.is_none());
            let target = panel.targets[0].connected.destination.id;
            panel.toggle_parameter_empty(target, 0, cx);
            panel.prepare(cx);
            assert_eq!(
                panel
                    .review
                    .as_ref()
                    .checked("explicit empty review")
                    .plan
                    .tasks()[0]
                    .command,
                "echo ''"
            );
            let old = panel.review.clone().checked("old review");
            panel.targets[0].parameters[0]
                .value
                .update(cx, |field, cx| field.set_value("changed", window, cx));
            assert!(!panel.review_current(&old, cx));
            panel.confirm(cx);
            assert!(!panel.is_running());
            panel.back(cx);
            panel.tasks[0]
                .command
                .update(cx, |field, cx| field.set_value("echo {{new}}", window, cx));
            panel.prepare(cx);
            assert!(panel.review.is_none());
            panel.sync_parameters(window, cx);
            assert_eq!(panel.targets[0].parameters[0].name, "new");
            assert!(
                panel.targets[0].parameters[0]
                    .value
                    .read(cx)
                    .value()
                    .is_empty()
            );
            panel.prepare(cx);
            assert!(panel.review.is_none());
        })
    })
    .checked("missing / explicit empty / old mapping cannot be reused");
    assert!(h.servers[0].requests().is_empty());
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

#[gpui_kit::test]
fn workflow_parameter_fields_scroll_at_small_windows_and_preserve_draft_across_locale_theme(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{ScrollDelta, point};
    let h = Harness::new(cx, &[0], 900., 580.);
    let source = format!(
        "printf '%s' {}",
        (0..12)
            .map(|i| format!("{{{{p{i}}}}}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    h.graph(cx, &[(&source, 0, &[])], false);
    h.parameters(cx, &[&[("p0", "first"), ("p11", "中文'\u{202e}last")]]);
    let target = h
        .panel
        .read_with(cx, |panel, _| panel.targets[0].connected.destination.id);
    let last = format!("workflow-parameters-{target}-p11");
    for theme in [
        keelshell_core::Theme::System,
        keelshell_core::Theme::Light,
        keelshell_core::Theme::Dark,
    ] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                let body = window.find("workflow-body").bounds();
                let end = window.find(last.clone()).bounds();
                window.scroll(
                    "workflow-body",
                    ScrollDelta::Pixels(point(px(0.), body.origin.y + px(12.) - end.bottom())),
                    cx,
                );
                window.render_frame(cx);
                assert!(window.find(last.clone()).visible());
                for id in ["workflow-footer", "workflow-review-button", "workflow-hide"] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.origin.x >= px(0.)
                            && bounds.origin.y >= px(0.)
                            && bounds.right() <= px(900.)
                            && bounds.bottom() <= px(580.)
                    );
                }
                h.panel.read_with(cx, |panel, cx| {
                    assert_eq!(
                        panel.targets[0].parameters[11].value.read(cx).value(),
                        "中文'\u{202e}last"
                    );
                    assert_eq!(panel.tasks[0].command.read(cx).value(), source);
                });
            })
            .checked("actual last field reachability and fixed footer / bilingual themes");
        }
    }
    assert!(h.servers[0].requests().is_empty());
}

#[gpui_kit::test]
fn workflow_parameter_count_value_and_render_limits_fail_before_ssh(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[0], 960., 720.);
    let names = (0..33).map(|i| format!("{{{{p{i}}}}}")).collect::<Vec<_>>();
    let first = format!("echo {}", names[..16].join(" "));
    let second = format!("echo {}", names[16..].join(" "));
    h.graph(cx, &[(&first, 0, &[]), (&second, 0, &[])], false);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.sync_parameters(window, cx);
            assert!(panel.message.is_some());
            assert!(panel.targets[0].parameters.is_empty());
            panel.prepare(cx);
            assert!(panel.review.is_none());
            panel.remove_task(panel.tasks[1].id, cx);
            panel.tasks[0]
                .command
                .update(cx, |input, cx| input.set_value("echo {{p}}", window, cx));
            panel.sync_parameters(window, cx);
            for value in ["x".repeat(4097), "bad\0control".into()] {
                panel.targets[0].parameters[0]
                    .value
                    .update(cx, |input, cx| input.set_value(value, window, cx));
                panel.prepare(cx);
                assert!(panel.review.is_none());
            }
            panel.tasks[0].command.update(cx, |input, cx| {
                input.set_value(format!("echo {}", "{{p}} ".repeat(17)), window, cx)
            });
            panel.targets[0].parameters[0]
                .value
                .update(cx, |input, cx| {
                    input.set_value("x".repeat(4096), window, cx)
                });
            panel.prepare(cx);
            assert!(panel.review.is_none());
        })
    })
    .checked("count / per-value / rendered-byte limits at actual panel admission");
    assert!(h.servers[0].requests().is_empty());
}
