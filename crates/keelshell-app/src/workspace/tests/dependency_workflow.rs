//! Workspace dispatch uses the production editor and human approval on real SSH.
use super::batch_peer as peer;
use super::*;
use gpui_kit::{
    App, ElementId, Pixels, ScrollDelta, Window,
    component::{WindowExt, input::AnyInputState},
};

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

fn assert_entry_layout(window: &mut Window, cx: &mut App) {
    window.render_frame(cx);
    let column = window.find("command-column").bounds();
    let actions = window.find("command-actions").bounds();
    assert!(contained(column, window.bounds()), "column {column:?}");
    assert!(
        contained(actions, column),
        "actions {actions:?}, column {column:?}"
    );
    let mut previous = Vec::<Bounds<Pixels>>::new();
    for id in [
        "command-history-policy",
        "new-command-draft",
        "command-batch",
        "command-workflow",
    ] {
        let button = window.find(id);
        let bounds = button.bounds();
        assert!(button.visible() && button.label().is_some(), "{id} hidden");
        assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        assert!(
            contained(bounds, actions),
            "{id} {bounds:?} escapes actions {actions:?} in column {column:?}"
        );
        for other in &previous {
            assert!(
                bounds.right() <= other.origin.x
                    || other.right() <= bounds.origin.x
                    || bounds.bottom() <= other.origin.y
                    || other.bottom() <= bounds.origin.y,
                "{id} {bounds:?} overlaps another action {other:?}"
            );
        }
        previous.push(bounds);
    }
    let input = window.find("command-input-container");
    let input_bounds = input.bounds();
    assert!(input.visible() && contained(input_bounds, column));
    assert!(
        input_bounds.size.width >= px(120.) && input_bounds.size.height >= px(60.),
        "unusable command input {input_bounds:?}"
    );
    assert!(input_bounds.bottom() <= actions.origin.y);
    let run = window.find("run-command").bounds();
    assert!(contained(run, column) && input_bounds.right() <= run.origin.x);
    if let Some(assistant) = window.try_find("assistant-column") {
        assert!(column.right() <= assistant.bounds().origin.x);
    }
}

fn click_visible(window: &mut Window, id: impl Into<ElementId> + Clone, cx: &mut App) {
    window.render_frame(cx);
    let body = window.find("workflow-body").bounds();
    let target = window.find(id.clone()).bounds();
    let delta = if target.bottom() > body.bottom() - px(8.) {
        body.bottom() - px(8.) - target.bottom()
    } else if target.origin.y < body.origin.y + px(8.) {
        body.origin.y + px(8.) - target.origin.y
    } else {
        px(0.)
    };
    if delta != px(0.) {
        window.scroll(
            "workflow-body",
            ScrollDelta::Pixels(point(px(0.), delta)),
            cx,
        );
        window.render_frame(cx);
    }
    window.click(id, cx);
}
fn replace(window: &mut Window, text: &str, cx: &mut App) {
    match window
        .focused_input(cx)
        .unwrap_or_else(|| panic!("workflow focused command"))
    {
        AnyInputState::Textarea(field) => field.update(cx, |field, cx| {
            field.set_selected_range(0..field.value().len(), cx);
            field.replace(text.to_owned(), window, cx);
        }),
        _ => panic!("workflow command textarea"),
    }
}
struct Harness {
    fixture: Fixture,
    panes: Vec<RemotePane>,
    servers: Vec<peer::Server>,
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let fixture = mount(cx, Vec::new());
        let panes = attach_remote_panes(&fixture, cx);
        let runtime = fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        let servers = (0..2)
            .map(|_| peer::Server::new(&runtime, 0))
            .collect::<Vec<_>>();
        fixture.workspace.update(cx, |view, cx| {
            for (pane, server) in panes.iter().zip(&servers) {
                view.remote_sessions
                    .insert(pane.terminal.entity_id(), server.session.clone());
            }
            cx.notify();
        });
        Self {
            fixture,
            panes,
            servers,
        }
    }
    fn prepare(&self, text: &str, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            self.fixture.workspace.update(cx, |view, cx| {
                view.set_reviewed_command(
                    text.into(),
                    Some(self.panes[0].terminal.entity_id()),
                    window,
                    cx,
                );
                view.open_workflow(false, window, cx);
            });
            click_visible(window, ("workflow-target", 0_usize), cx);
            window.render_frame(cx);
            window.click("workflow-review-button", cx);
        })
        .checked("choose actual connected target and prepare full workflow review");
        cx.run_until_parked();
    }
    fn confirm(&self, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("workflow-confirm", cx);
        })
        .checked("manual workflow confirmation");
        cx.run_until_parked();
    }
    async fn complete(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.fixture.window, Duration::from_secs(8), |_, cx| {
            self.fixture
                .workspace
                .read(cx)
                .workflow_panel
                .as_ref()
                .is_some_and(|panel| !panel.read(cx).is_running())
        })
        .await;
    }
}

#[gpui_kit::test]
fn dependency_workflow_entry_actions_fit_and_operate_with_themes_languages_and_ai(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    for (width, height) in [(900., 580.), (1440., 900.)] {
        cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
        cx.run_until_parked();
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                for assistant in [false, true] {
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        let draft = h.fixture.workspace.read(cx);
                        let before = (
                            draft.command.entity_id(),
                            draft.command.read(cx).value().to_string(),
                            draft.command_target,
                            draft.command_revision,
                        );
                        i18n::set_language(language, cx);
                        crate::design::apply(theme, Some(window), cx);
                        h.fixture.workspace.update(cx, |view, cx| {
                            view.show_assistant = assistant;
                            view.state.settings.theme = theme;
                            view.state.settings.language = language;
                            cx.notify();
                        });
                        let draft = h.fixture.workspace.read(cx);
                        assert_eq!(
                            (
                                draft.command.entity_id(),
                                draft.command.read(cx).value().to_string(),
                                draft.command_target,
                                draft.command_revision,
                            ),
                            before,
                            "appearance and AI layout retain input identity and draft binding"
                        );
                        assert_eq!(window.viewport_size(), size(px(width), px(height)));
                        for history in [false, true] {
                            h.fixture.workspace.update(cx, |view, cx| {
                                view.command_record_history = history;
                                cx.notify();
                            });
                            assert_entry_layout(window, cx);
                            let terminal = window
                                .find(("terminal-pane", h.panes[0].terminal.entity_id()))
                                .bounds();
                            assert!(
                                terminal.size.height >= px(80.),
                                "terminal collapsed {terminal:?}"
                            );
                            let first = window.find("command-history-policy").bounds();
                            let last = window.find("command-workflow").bounds();
                            if width == 900. && assistant && language == Language::En {
                                assert!(
                                    last.origin.y > first.origin.y,
                                    "compact English entry must wrap"
                                );
                            } else if width == 1440. {
                                assert_eq!(
                                    last.origin.y, first.origin.y,
                                    "wide actions remain on one line"
                                );
                            }
                            window.click("command-history-policy", cx);
                            assert_eq!(
                                h.fixture.workspace.read(cx).command_record_history,
                                !history
                            );
                        }
                        window.click("command-input-container", cx);
                        replace(window, "layout-entry-seed", cx);
                    })
                    .checked(
                        "both history labels and input fit the actual production command column",
                    );
                    cx.run_until_parked();
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        window.render_frame(cx);
                        window.click("new-command-draft", cx);
                        assert!(
                            h.fixture
                                .workspace
                                .read(cx)
                                .command
                                .read(cx)
                                .value()
                                .is_empty()
                        );
                        window.click("command-input-container", cx);
                        replace(window, "layout-entry-seed", cx);
                    })
                    .checked("new-command action clears the real focused textarea");
                    cx.run_until_parked();
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        window.render_frame(cx);
                        window.click("command-batch", cx);
                        window.render_frame(cx);
                        assert!(h.fixture.workspace.read(cx).show_batch);
                        window.click("batch-hide", cx);
                    })
                    .checked("ordinary batch entry remains reachable");
                    cx.run_until_parked();
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        assert_entry_layout(window, cx);
                        window.click("command-workflow", cx);
                        window.render_frame(cx);
                        assert!(h.fixture.workspace.read(cx).show_workflow);
                        assert!(window.find("workflow-hide").visible());
                        click_visible(window, "workflow-command-container", cx);
                        match window
                            .focused_input(cx)
                            .unwrap_or_else(|| panic!("workflow input"))
                        {
                            AnyInputState::Textarea(field) => {
                                assert_eq!(field.read(cx).value(), "layout-entry-seed")
                            }
                            _ => panic!("workflow command textarea"),
                        }
                        window.click("workflow-hide", cx);
                    })
                    .checked(
                        "workflow entry opens the production editor with retained command text",
                    );
                    cx.run_until_parked();
                    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
                    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
                }
            }
        }
    }
}

#[gpui_kit::test]
async fn dependency_workflow_entry_running_labels_fit_while_two_captured_runs_are_hidden(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.prepare("hold", cx);
    h.confirm(cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("retain running dependency workflow behind its entry");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("command-batch", cx);
        window.render_frame(cx);
        window.click(("batch-select", 1_usize), cx);
        window.click("batch-review-button", cx);
    })
    .checked("prepare separate ordinary batch on the second captured connection");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-confirm", cx);
    })
    .checked("human approval starts the separately owned ordinary batch");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("batch-hide", cx);
    })
    .checked("retain ordinary batch behind its running entry");
    cx.run_until_parked();
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        h.servers
            .iter()
            .all(|server| server.requests() == vec![b"hold".to_vec()])
    })
    .await;
    for (width, height) in [(900., 580.), (1440., 900.)] {
        cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
        cx.run_until_parked();
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                for assistant in [false, true] {
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        i18n::set_language(language, cx);
                        crate::design::apply(theme, Some(window), cx);
                        h.fixture.workspace.update(cx, |view, cx| {
                            view.show_assistant = assistant;
                            cx.notify();
                        });
                        assert_entry_layout(window, cx);
                        let (batch, workflow) = if language == Language::En {
                            ("Batch · running", "Workflow · running")
                        } else {
                            ("批量任务 · 运行中", "工作流 · 运行中")
                        };
                        assert_eq!(window.find("command-batch").label(), Some(batch));
                        assert_eq!(window.find("command-workflow").label(), Some(workflow));
                    })
                    .checked("both actual running labels fit the production command column");
                }
            }
        }
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("command-batch", cx);
        window.render_frame(cx);
        window.click("batch-cancel", cx);
        window.click("batch-hide", cx);
    })
    .checked("running batch entry reopens the original run for local cancellation");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("command-workflow", cx);
        window.render_frame(cx);
        window.click("workflow-cancel", cx);
    })
    .checked("running workflow entry reopens the original run for local cancellation");
    h.complete(cx).await;
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, cx| {
        h.fixture
            .workspace
            .read(cx)
            .batch_panel
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).is_running())
    })
    .await;
    assert!(
        h.servers
            .iter()
            .all(|server| server.requests() == vec![b"hold".to_vec()])
    );
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}

#[gpui_kit::test]
async fn dependency_workflow_editor_executes_reviewed_target_template_and_keeps_history_private(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.prepare("printf {{endpoint}}", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-back", cx);
        click_visible(window, "workflow-add-task", cx);
        click_visible(window, "workflow-command-container", cx);
        replace(window, "after-prerequisite", cx);
        click_visible(window, ("workflow-target", 1_usize), cx);
        click_visible(window, ("workflow-dependency", 0_usize), cx);
    })
    .checked("edit second command, exact target and dependency using production controls");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-review-button", cx);
    })
    .checked("review after all edit events are delivered");
    cx.run_until_parked();
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.confirm(cx);
    h.complete(cx).await;
    assert_eq!(
        h.servers[0].requests(),
        vec![b"printf 'fixture-0@example.invalid:22'".to_vec()]
    );
    assert_eq!(
        h.servers[1].requests(),
        vec![b"after-prerequisite".to_vec()]
    );
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    h.fixture
        .workspace
        .read_with(cx, |view, _| assert!(view.command_histories.is_empty()));
    let state = h
        .fixture
        .store
        .load()
        .checked("persistent state after ephemeral workflow");
    assert!(state.batch_audits.is_empty());
    let disk = std::fs::read_to_string(h.fixture.store.path()).checked("persisted metadata");
    assert!(!disk.contains("printf") && !disk.contains("fixture stdout"));
}

#[gpui_kit::test]
fn dependency_workflow_workspace_rejects_same_endpoint_session_replacement_and_blocks_background_actions(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.prepare("must-not-run", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.remote_sessions.insert(
                h.panes[0].terminal.entity_id(),
                h.servers[1].session.clone(),
            );
            let old_tabs = view.tabs.len();
            let old_assistant = view.show_assistant;
            view.open_connections(&crate::workspace::OpenConnections, window, cx);
            view.open_ai_settings(window, cx);
            view.open_vault_settings(window, cx);
            view.split_remote(window, cx);
            view.toggle_assistant(&crate::workspace::ToggleAssistant, window, cx);
            assert!(
                view.show_workflow
                    && !view.show_connections
                    && view.ai_settings.is_none()
                    && view.vault_settings.is_none()
            );
            assert_eq!(view.tabs.len(), old_tabs);
            assert_eq!(view.show_assistant, old_assistant);
        });
        window.render_frame(cx);
        assert!(window.try_find("workflow-confirm").is_none());
    })
    .checked("replacement cannot inherit reviewed instance and modal owns actions");
    cx.run_until_parked();
    assert!(h.servers.iter().all(|server| server.requests().is_empty()));
    h.fixture.workspace.read_with(cx, |view, cx| {
        assert!(
            !view
                .workflow_panel
                .as_ref()
                .unwrap_or_else(|| panic!("workflow panel"))
                .read(cx)
                .is_running()
        )
    });
}

#[gpui_kit::test]
async fn dependency_workflow_hide_preserves_owned_cancelled_receipts_without_replay(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.prepare("hold", cx);
    h.confirm(cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-hide", cx);
    })
    .checked("hide running workflow");
    let owned = h.fixture.workspace.read_with(cx, |view, _| {
        view.workflow_panel.as_ref().map(Entity::entity_id)
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.open_workflow(false, window, cx));
        window.render_frame(cx);
        window.click("workflow-cancel", cx);
    })
    .checked("reopen same run and cancel local waits");
    h.complete(cx).await;
    h.fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.workflow_panel.as_ref().map(Entity::entity_id), owned)
    });
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-new", cx);
    })
    .checked("new workflow needs new explicit target selection and approval");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("workflow-review-button", cx);
    })
    .checked("new unselected target fails review");
    assert_eq!(h.servers[0].requests(), vec![b"hold".to_vec()]);
}
