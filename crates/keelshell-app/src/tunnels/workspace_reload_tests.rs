//! Real panel events, shared reload leases and isolated StateStore round trips.

use super::*;
use crate::{app_windows::ConfigurationGate, tunnels::TunnelRuleReloadEvent};

async fn conflict_with_external_rule(fixture: &Fixture, cx: &mut TestAppContext) -> uuid::Uuid {
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
        fill(window, "tunnel-rule-name", "retained draft 中文  ", cx);
        click(window, "review-save-tunnel-rule", cx);
    })
    .expect("review against original state");
    let mut changed = fixture.store.load().expect("external state");
    changed.tunnel_rules[0].name = "external winner".into();
    fixture.store.save(&changed).expect("external CAS commit");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("submit the old review");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        !workspace.saving && workspace.tunnel_rule_store_blocked
    })
    .await;
    rule_id
}

async fn wait_for_reload(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).state_reloading
    })
    .await;
}

fn observe_reload_admission(
    fixture: &Fixture,
    panel: &Entity<TunnelsPanel>,
    cx: &mut TestAppContext,
) -> (Rc<RefCell<Vec<bool>>>, gpui_kit::Subscription) {
    let admissions = Rc::new(RefCell::new(Vec::new()));
    let observed = admissions.clone();
    let subscription = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |_, cx| {
                // GPUI dispatches emitted events after the click's local
                // handler. Observe after the production subscriber, rather
                // than treating emission itself as Workspace admission.
                cx.subscribe_in(
                    panel,
                    window,
                    move |workspace, _, _: &TunnelRuleReloadEvent, _, _| {
                        observed.borrow_mut().push(workspace.state_reloading);
                    },
                )
            })
        })
        .expect("observe actual reload admission");
    (admissions, subscription)
}

#[gpui_kit::test]
async fn panel_reload_recovers_conflict_retains_draft_and_requires_new_review(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let rule_id = conflict_with_external_rule(&fixture, cx).await;
    let panel = current_tunnel_panel(&fixture, cx);
    let (admissions, _subscription) = observe_reload_admission(&fixture, &panel, cx);
    let before = std::fs::read(fixture.store.path()).expect("external bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("explicit panel reload");
    wait_for_reload(&fixture, cx).await;
    assert_eq!(*admissions.borrow(), [true]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("tunnel-rule-name").value(),
            Some("retained draft 中文  ")
        );
        assert!(window.try_find("confirm-tunnel-rule-change").is_none());
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.state.tunnel_rules[0].name, "external winner");
        assert!(!workspace.tunnel_rule_store_blocked);
        assert!(workspace.configuration_current(cx));
        assert_eq!(
            rule_admission(current_panel(workspace).read(cx)),
            (false, false)
        );

        // A retained editor still owns its original snapshot. Reload cannot
        // silently turn those bytes into a reviewed write against the winner.
        click(window, "review-save-tunnel-rule", cx);
        window.render_frame(cx);
        assert!(window.try_find("confirm-tunnel-rule-change").is_none());
        click(window, "cancel-tunnel-rule", cx);
        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
        fill(window, "tunnel-rule-name", "reviewed latest rule", cx);
        click(window, "review-save-tunnel-rule", cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("a fresh review can save against the latest snapshot");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let saved = fixture.store.load().expect("latest committed rule");
    assert_eq!(saved.tunnel_rules[0].name, "reviewed latest rule");
    assert_ne!(
        std::fs::read(fixture.store.path()).expect("new bytes"),
        before
    );
}

#[gpui_kit::test]
async fn panel_reload_failure_keeps_existing_and_new_panels_blocked_then_retries(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let first = current_tunnel_panel(&fixture, cx);
    let valid = std::fs::read(fixture.store.path()).expect("recoverable fixture");
    std::fs::write(fixture.store.path(), b"{broken").expect("isolated broken state");
    let opened = Rc::new(RefCell::new(None::<TunnelTab>));
    let observed = opened.clone();
    let existing = first.clone();
    let _subscription = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |_, cx| {
                cx.subscribe_in(
                    &first,
                    window,
                    move |workspace, _, _: &TunnelRuleReloadEvent, window, cx| {
                        assert!(workspace.state_reloading);
                        assert!(rule_admission(existing.read(cx)).0);
                        let tab = add_tab(workspace, window, cx);
                        assert!(rule_admission(tab.panel.read(cx)).0);
                        *observed.borrow_mut() = Some(tab);
                    },
                )
            })
        })
        .expect("observe synchronous busy propagation");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("start broken reload");
    wait_for_reload(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.configuration_invalidated);
        assert!(workspace.tunnel_rule_store_blocked);
        assert_eq!(rule_admission(first.read(cx)), (false, true));
        assert_eq!(
            rule_admission(opened.borrow().as_ref().expect("new tab").panel.read(cx)),
            (false, true)
        );
    });
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("broken bytes"),
        b"{broken"
    );
    std::fs::write(fixture.store.path(), valid).expect("explicit fixture repair");
    // This is the newly opened panel's recovery action, not a direct call to
    // the authority-adoption helper.
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("retry from the new blocked panel");
    wait_for_reload(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.configuration_current(cx));
        assert!(!workspace.tunnel_rule_store_blocked);
        assert_eq!(rule_admission(first.read(cx)), (false, false));
        assert_eq!(
            rule_admission(opened.borrow().as_ref().expect("new tab").panel.read(cx)),
            (false, false)
        );
    });
}

#[gpui_kit::test]
async fn panel_reload_unchanged_snapshot_invalidates_sibling_start_review(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    let sibling = cx
        .update_window(fixture.window, |_, window, cx| {
            let tab = fixture
                .workspace
                .update(cx, |workspace, cx| add_tab(workspace, window, cx));
            click(window, format!("named-tunnel-rule-{rule_id}"), cx);
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.active = 0;
                cx.notify();
            });
            tab
        })
        .expect("prepare a review in a sibling tab");
    let before = std::fs::read(fixture.store.path()).expect("unchanged bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("reload an unchanged snapshot");
    wait_for_reload(&fixture, cx).await;
    sibling.panel.update(cx, try_reviewed_start);
    assert!(
        !sibling
            .panel
            .read_with(cx, |panel, _| panel.has_active_rule(rule_id))
    );
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("unchanged bytes"),
        before
    );
}

#[gpui_kit::test]
async fn panel_reload_rejects_competing_lease_without_leaking_busy(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let (admissions, _subscription) = observe_reload_admission(&fixture, &panel, cx);
    let state = fixture.store.load().expect("fixture state");
    let gate = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.configuration_gate.clone());
    let lease = ConfigurationGate::begin_reload_lease(&gate, uuid::Uuid::new_v4())
        .expect("another window owns the store");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
        assert!(!fixture.workspace.read(cx).state_reloading);
        assert_eq!(rule_admission(panel.read(cx)), (false, false));
    })
    .expect("the real event rejects competing store ownership");
    lease.finish(Some(&state));
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("the rejected request did not retain its token");
    wait_for_reload(&fixture, cx).await;
    assert_eq!(*admissions.borrow(), [false, true]);
    assert!(
        fixture
            .workspace
            .read_with(cx, |workspace, cx| workspace.configuration_current(cx))
    );
}

#[gpui_kit::test]
async fn panel_reload_retired_origin_cannot_unblock_replacement(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let original = current_tunnel_panel(&fixture, cx);
    let original_id = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.tabs[0].entity_id());
    let before_state = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.state.clone());
    let mut external = fixture.store.load().expect("external read");
    external.tunnel_rules[0].name = "not adopted by retired origin".into();
    fixture.store.save(&external).expect("external save");
    let replacement = Rc::new(RefCell::new(None::<TunnelTab>));
    let observed = replacement.clone();
    let _subscription = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |_, cx| {
                cx.subscribe_in(
                    &original,
                    window,
                    move |workspace, panel, _: &TunnelRuleReloadEvent, window, cx| {
                        assert!(workspace.state_reloading);
                        panel.update(cx, |panel, cx| panel.suspend(cx));
                        let retired = workspace.panels.remove(&original_id).expect("live owner");
                        workspace.archived_panels.insert(original_id, retired);
                        *observed.borrow_mut() = Some(add_tab(workspace, window, cx));
                    },
                )
            })
        })
        .expect("retire the origin after production admission");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("start read from the original owner");
    wait_for_reload(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.state, before_state);
        assert!(workspace.configuration_invalidated);
        assert!(workspace.tunnel_rule_store_blocked);
        assert_eq!(
            rule_admission(
                replacement
                    .borrow()
                    .as_ref()
                    .expect("replacement")
                    .panel
                    .read(cx)
            ),
            (false, true)
        );
    });
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("replacement explicitly acquires its own reload authority");
    wait_for_reload(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(
            workspace.state.tunnel_rules[0].name,
            "not adopted by retired origin"
        );
        assert!(workspace.configuration_current(cx));
    });
}

#[gpui_kit::test]
async fn panel_reload_retains_running_tunnel_and_original_ssh(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("explicitly start a real loopback listener");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_listening(panel.read(cx), rule_id)
            && !fixture.workspace.read(cx).saving
            && !fixture
                .workspace
                .read(cx)
                .state
                .tunnel_runtime_snapshots
                .is_empty()
    })
    .await;
    let snapshot = fixture.workspace.read_with(cx, |workspace, _| {
        workspace.state.tunnel_runtime_snapshots[0].clone()
    });
    let mut external = fixture.store.load().expect("external state");
    external.tunnel_rules[0].name = "metadata for a future start".into();
    fixture
        .store
        .save(&external)
        .expect("external metadata change");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("reload metadata while the original worker runs");
    wait_for_reload(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(
            workspace.state.tunnel_rules[0].name,
            "metadata for a future start"
        );
        assert_eq!(workspace.state.tunnel_runtime_snapshots[0], snapshot);
        assert!(rule_listening(panel.read(cx), rule_id));
        assert!(!fixture.ssh.session.is_closed());
    });
    panel.update(cx, |panel, cx| panel.suspend(cx));
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !panel.read(cx).has_active_rule(rule_id)
    })
    .await;
}

#[gpui_kit::test]
async fn stale_window_configuration_can_reload_from_tunnel_panel(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let (admissions, _subscription) = observe_reload_admission(&fixture, &panel, cx);
    let mut external = fixture.store.load().expect("external state");
    external.settings.scrollback_lines += 1;
    let external = fixture
        .store
        .save(&external)
        .expect("sibling configuration commit");
    let gate = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.configuration_gate.clone());
    ConfigurationGate::begin_reload_lease(&gate, uuid::Uuid::new_v4())
        .expect("publish sibling's authoritative state")
        .finish(Some(&external));
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.observe_shared_configuration(window, cx);
            assert!(workspace.configuration_invalidated);
            assert_ne!(workspace.state.snapshot, external.snapshot);
        });
        assert_eq!(rule_admission(panel.read(cx)), (false, true));
        click(window, "reload-tunnel-configuration", cx);
    })
    .expect("stale catalog is recoverable rather than permanently busy");
    wait_for_reload(&fixture, cx).await;
    assert_eq!(*admissions.borrow(), [true]);
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.state.snapshot, external.snapshot);
        assert!(workspace.configuration_current(cx));
        assert_eq!(rule_admission(panel.read(cx)), (false, false));
    });
}
