//! Rendered review identity, metadata validation and real worker ownership.
#![allow(clippy::expect_used)]

use super::{Message, TunnelRuleKind, TunnelsPanel};
use crate::tunnels::test_support::{SshFixture, activate_painted, click};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Context, TestAppContext, Window};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

fn review_edit(
    panel: &mut TunnelsPanel,
    id: uuid::Uuid,
    window: &mut Window,
    cx: &mut Context<TunnelsPanel>,
) -> uuid::Uuid {
    panel.begin_edit_rule(id, panel.rule_epoch, window, cx);
    let editor = panel.rule_editor.as_ref().expect("editor");
    let identity = editor.identity;
    editor
        .name
        .update(cx, |input, cx| input.set_value("changed rule", window, cx));
    panel.review_rule_save(identity, cx);
    panel
        .rule_change_review
        .as_ref()
        .expect("review")
        .event
        .operation
}

#[gpui_kit::test]
fn editor_validates_all_modes_without_starting_a_listener(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        click(window, "new-tunnel-rule", cx);
        panel.update(cx, |panel, cx| {
            let editor = panel.rule_editor.as_ref().expect("new editor");
            assert!(!editor.enabled);
            assert!(editor.draft(cx).is_err(), "empty name must fail");
            editor
                .name
                .update(cx, |input, cx| input.set_value("new rule", window, cx));
            let local = editor.draft(cx).expect("valid local");
            assert_eq!(local.kind, TunnelRuleKind::Local);
            assert_eq!(local.bind_port, 0);
            let identity = editor.identity;
            panel.set_rule_kind(identity, TunnelRuleKind::Remote, cx);
            let editor = panel.rule_editor.as_ref().expect("remote editor");
            assert_eq!(
                editor.draft(cx).expect("remote").kind,
                TunnelRuleKind::Remote
            );
            editor
                .target_port
                .update(cx, |input, cx| input.set_value("0", window, cx));
            assert!(editor.draft(cx).is_err());
            panel.set_rule_kind(identity, TunnelRuleKind::Dynamic, cx);
            let editor = panel.rule_editor.as_ref().expect("dynamic editor");
            assert!(
                editor
                    .draft(cx)
                    .expect("hidden destination ignored")
                    .target_host
                    .is_none()
            );
            editor
                .bind_host
                .update(cx, |input, cx| input.set_value("0.0.0.0", window, cx));
            assert!(editor.draft(cx).is_err());
            editor
                .bind_host
                .update(cx, |input, cx| input.set_value("::1", window, cx));
            panel.review_rule_save(identity, cx);
            assert!(panel.rule_change_review.is_some());
            assert!(panel.rule_mutation.is_none());
            assert!(panel.rows.is_empty());
        });
        click(window, "cancel-tunnel-rule-review", cx);
        assert!(
            panel.read(cx).rule_editor.is_some(),
            "cancel review keeps draft"
        );
        click(window, "cancel-tunnel-rule", cx);
        assert!(panel.read(cx).rule_editor.is_none());
    })
    .expect("all editor modes");
}

#[gpui_kit::test]
fn painted_save_and_cancel_cannot_act_on_replacement_reviews(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        for control in ["confirm-tunnel-rule-change", "cancel-tunnel-rule-review"] {
            let old = panel.update(cx, |panel, cx| {
                review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx)
            });
            window.render_frame(cx);
            let position = window.find(control).bounds().center();
            let new = panel.update(cx, |panel, cx| {
                panel.cancel_rule_change(old, cx);
                review_edit(panel, fixture.state.tunnel_rules[1].id, window, cx)
            });
            assert_ne!(old, new);
            activate_painted(window, position, cx);
            assert!(panel.read(cx).rule_mutation.is_none());
            assert_eq!(
                panel
                    .read(cx)
                    .rule_change_review
                    .as_ref()
                    .expect("new review retained")
                    .event
                    .operation,
                new
            );
            panel.update(cx, |panel, cx| panel.cancel_rule_change(new, cx));
        }
    })
    .expect("stale painted mutation controls");
}

#[gpui_kit::test]
fn painted_delete_and_start_reject_replacement_rules_and_reviews(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let first = fixture.state.tunnel_rules[0].id;
    let second = fixture.state.tunnel_rules[1].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("delete-tunnel-rule-{first}"), cx);
        window.render_frame(cx);
        let confirm = window.find("confirm-tunnel-rule-change").bounds().center();
        panel.update(cx, |panel, cx| {
            panel.request_delete_rule(second, panel.rule_epoch, cx)
        });
        let token = panel
            .read(cx)
            .rule_change_review
            .as_ref()
            .expect("second delete review")
            .event
            .operation;
        activate_painted(window, confirm, cx);
        assert!(panel.read(cx).rule_mutation.is_none());
        assert_eq!(
            panel
                .read(cx)
                .rule_change_review
                .as_ref()
                .expect("still second")
                .event
                .operation,
            token
        );
        panel.update(cx, |panel, cx| panel.cancel_rule_change(token, cx));
        click(window, format!("named-tunnel-rule-{first}"), cx);
        window.render_frame(cx);
        let start = window.find("review-start-named-tunnel").bounds().center();
        panel.update(cx, |panel, cx| {
            panel.select_named_rule(second, panel.rule_epoch, cx)
        });
        activate_painted(window, start, cx);
        assert!(panel.read(cx).rows.is_empty());
        assert_eq!(
            panel
                .read(cx)
                .rule_plan
                .as_ref()
                .expect("second plan")
                .rule()
                .id,
            second
        );
        window.render_frame(cx);
        let delete = window
            .find(format!("delete-tunnel-rule-{first}"))
            .bounds()
            .center();
        panel.update(cx, |panel, cx| {
            let mut state = fixture.state.clone();
            state.tunnel_rules[0].name = "same ID changed".into();
            panel.refresh_rule_state(Some(state), Some(fixture.connection_id), cx);
        });
        activate_painted(window, delete, cx);
        assert!(panel.read(cx).rule_change_review.is_none());
    })
    .expect("stale painted delete/start controls");
}

#[gpui_kit::test]
fn edit_and_confirmation_bind_state_rule_connection_and_draft(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        for changed in 0..4 {
            panel.update(cx, |panel, cx| {
                panel.refresh_rule_state(
                    Some(fixture.state.clone()),
                    Some(fixture.connection_id),
                    cx,
                );
                let operation = review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx);
                match changed {
                    0 => {
                        panel.rule_state.as_mut().expect("state").settings.theme =
                            keelshell_core::Theme::Dark
                    }
                    1 => {
                        panel.rule_state.as_mut().expect("state").tunnel_rules[0].bind_port = 12345
                    }
                    2 => {
                        panel.rule_state.as_mut().expect("state").connections[0].host =
                            "changed.invalid".into()
                    }
                    _ => panel
                        .rule_editor
                        .as_ref()
                        .expect("draft")
                        .name
                        .update(cx, |input, cx| {
                            input.set_value("unreviewed text", window, cx)
                        }),
                }
                panel.confirm_rule_change(operation, cx);
                assert!(panel.rule_mutation.is_none());
                assert!(panel.rule_change_review.is_none());
                assert!(panel.rule_editor.is_some());
            });
        }
        panel.update(cx, |panel, cx| {
            panel.refresh_rule_state(Some(fixture.state.clone()), Some(fixture.connection_id), cx);
            let operation = review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx);
            panel.confirm_rule_change(operation, cx);
            let pending = panel.rule_mutation.clone().expect("pending save");
            assert!(!panel.mark_rule_mutation_succeeded(uuid::Uuid::new_v4(), cx));
            assert!(panel.accepts_rule_mutation(&pending));
            assert!(panel.mark_rule_mutation_failed(
                operation,
                Message::new("冲突", "Conflict"),
                true,
                cx
            ));
            assert!(panel.rule_store_blocked);
            assert!(panel.rule_editor.is_some());
            panel.confirm_rule_change(operation, cx);
            assert!(
                panel.rule_mutation.is_none(),
                "no automatic retry after uncertain storage"
            );
        });
    })
    .expect("review binding and pending operation ownership");
}

#[gpui_kit::test]
fn stale_editor_and_suspended_panel_cannot_save_or_retarget(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.begin_new_rule(panel.rule_epoch, window, cx);
            let editor = panel.rule_editor.as_ref().expect("draft");
            editor
                .name
                .update(cx, |input, cx| input.set_value("my draft", window, cx));
            let identity = editor.identity;
            let other = keelshell_core::Connection::new("other", "other.invalid", "user");
            let mut state = fixture.state.clone();
            state.connections.push(other.clone());
            panel.refresh_rule_state(Some(state), Some(other.id), cx);
            panel.review_rule_save(identity, cx);
            assert!(panel.rule_change_review.is_none());
            assert_eq!(
                panel
                    .rule_editor
                    .as_ref()
                    .expect("retained original")
                    .connection_id,
                fixture.connection_id
            );
            panel.cancel_rule_edit(identity, cx);
            panel.refresh_rule_state(Some(fixture.state.clone()), Some(fixture.connection_id), cx);
            let operation = review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx);
            panel.suspend(cx);
            panel.confirm_rule_change(operation, cx);
            assert!(panel.rule_mutation.is_none());
        });
    })
    .expect("editor cannot rebind across connections or session retirement");
}

#[gpui_kit::test]
async fn rule_stop_preserves_other_workers_and_original_ownership_after_edit(
    cx: &mut TestAppContext,
) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let first = fixture.state.tunnel_rules[0].id;
    let second = fixture.state.tunnel_rules[1].id;
    cx.update_window(window, |_, window, cx| {
        for id in [first, second, first] {
            click(window, format!("named-tunnel-rule-{id}"), cx);
            click(window, "review-start-named-tunnel", cx);
        }
    })
    .expect("start real named listeners");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.len() == 3 && panel.read(cx).rows.iter().all(|row| row.listening)
    })
    .await;
    let addresses = panel.read_with(cx, |panel, _| {
        panel
            .rows
            .iter()
            .map(|row| {
                row.proxy_address
                    .clone()
                    .expect("listener URI")
                    .trim_start_matches("socks5h://")
                    .parse::<SocketAddr>()
                    .expect("address")
            })
            .collect::<Vec<_>>()
    });
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        window.render_frame(cx);
        let old_stop = window.find("confirm-tunnel-rule-stop").bounds().center();
        let owners = panel
            .read(cx)
            .rows
            .iter()
            .map(|row| row.cancel.clone())
            .collect::<Vec<_>>();
        panel.update(cx, |panel, cx| {
            let mut state = fixture.state.clone();
            state.tunnel_rules[0].name = "edited metadata".into();
            state.tunnel_rules[0].enabled = false;
            state.tunnel_rules[0].bind_port = 34567;
            panel.refresh_rule_state(Some(state), Some(fixture.connection_id), cx);
        });
        activate_painted(window, old_stop, cx);
        assert!(
            panel
                .read(cx)
                .rows
                .iter()
                .all(|row| !row.cancel.load(Ordering::Acquire))
        );
        for (row, owner) in panel.read(cx).rows.iter().zip(&owners) {
            assert!(
                Arc::ptr_eq(&row.cancel, owner),
                "editing cannot replace worker ownership"
            );
        }
        click(window, ("stop-tunnel", 0_usize), cx);
        assert_eq!(
            panel
                .read(cx)
                .rule_stop_review
                .as_ref()
                .expect("disabled rule can stop")
                .workers,
            vec![0, 2]
        );
        assert!(
            panel.read(cx).rows[0]
                .description
                .render(cx)
                .contains("first proxy"),
            "runtime preview retains original rule name"
        );
        click(window, "confirm-tunnel-rule-stop", cx);
        assert!(owners[0].load(Ordering::Acquire));
        assert!(owners[2].load(Ordering::Acquire));
        assert!(!owners[1].load(Ordering::Acquire));
        assert!(
            !panel.read(cx).rows[0].finished,
            "stop request is not a cleanup receipt"
        );
    })
    .expect("review stops only the selected rule's original workers");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].finished && panel.read(cx).rows[2].finished
    })
    .await;
    fixture.runtime.block_on(async {
        for index in [0, 2] {
            assert!(
                tokio::net::TcpStream::connect(addresses[index])
                    .await
                    .is_err()
            );
        }
        assert!(
            tokio::net::TcpStream::connect(addresses[1]).await.is_ok(),
            "other rule still listening"
        );
    });
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 1_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("stop other named listener");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.iter().all(|row| row.finished)
    })
    .await;
    assert!(
        !fixture.session.is_closed(),
        "stopping rules must not replace or close the SSH session"
    );
}

#[gpui_kit::test]
fn rule_review_footer_stays_visible_in_bounded_tool_area(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.simulate_window_resize(
        window,
        gpui_kit::size(gpui_kit::px(900.), gpui_kit::px(260.)),
    );
    cx.update_window(window, |_, window, cx| {
        for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
            for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                let operation = panel.update(cx, |panel, cx| {
                    review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx)
                });
                window.render_frame(cx);
                let viewport = window.find("tunnels-panel").bounds();
                let footer = window.find("tunnel-review-footer").bounds();
                for id in ["confirm-tunnel-rule-change", "cancel-tunnel-rule-review"] {
                    let control = window.find(id);
                    assert!(control.visible(), "{language:?}/{theme:?}: {id}");
                    assert!(
                        viewport.contains(&control.bounds().origin)
                            && viewport.contains(&control.bounds().bottom_right())
                    );
                    assert!(
                        footer.contains(&control.bounds().origin)
                            && footer.contains(&control.bounds().bottom_right())
                    );
                    assert!(control.bounds().size.height >= gpui_kit::px(32.));
                }
                window.scroll(
                    "tunnel-management-scroll",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                        gpui_kit::px(0.),
                        gpui_kit::px(-500.),
                    )),
                    cx,
                );
                window.render_frame(cx);
                assert_eq!(window.find("tunnel-review-footer").bounds(), footer);
                panel.update(cx, |panel, cx| panel.cancel_rule_change(operation, cx));
            }
        }
    })
    .expect("fixed review footer in all locales and themes");
}

#[gpui_kit::test]
async fn stop_review_rejects_instances_started_after_the_preview(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let rule = &fixture.state.tunnel_rules[0];
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{}", rule.id), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start original instance");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].listening
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        window.render_frame(cx);
        let old_confirm = window.find("confirm-tunnel-rule-stop").bounds().center();
        panel.update(cx, |panel, cx| {
            // Exercise the runtime-membership guard independently of the UI
            // selection guard which normally invalidates the old preview first.
            let specification = crate::tunnels::specification_from_rule(rule)
                .unwrap_or_else(|_| panic!("valid fixture rule"));
            panel.start_specification(specification, Some(rule.name.clone()), Some(rule.id), cx);
        });
        activate_painted(window, old_confirm, cx);
        assert!(panel.read(cx).rule_stop_review.is_none());
        assert!(
            panel
                .read(cx)
                .rows
                .iter()
                .all(|row| !row.cancel.load(Ordering::Acquire))
        );
    })
    .expect("old stop cannot expand to an unseen instance set");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.iter().all(|row| row.listening)
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("review both current instances");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows.iter().all(|row| row.finished)
    })
    .await;
}

#[gpui_kit::test]
fn authoritative_identical_reload_expires_mutation_review_and_normal_refresh_stays_blocked(
    cx: &mut TestAppContext,
) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let operation = review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx);
            panel.reload_rule_state(fixture.state.clone(), Some(fixture.connection_id), cx);
            panel.confirm_rule_change(operation, cx);
            assert!(panel.rule_mutation.is_none());
            assert!(panel.rule_change_review.is_none());
            assert!(panel.rule_editor.is_some(), "reload preserves draft text");

            let identity = panel.rule_editor.as_ref().expect("draft").identity;
            panel.cancel_rule_edit(identity, cx);
            let operation = review_edit(panel, fixture.state.tunnel_rules[0].id, window, cx);
            panel.confirm_rule_change(operation, cx);
            let event = panel.rule_mutation.clone().expect("pending mutation");
            assert!(panel.accepts_rule_mutation(&event));
            panel.set_rule_configuration_busy(true, cx);
            assert!(!panel.accepts_rule_mutation(&event));
            panel.set_rule_configuration_busy(false, cx);
            panel.mark_rule_store_uncertain(cx);
            assert!(!panel.accepts_rule_mutation(&event));
            panel.mark_rule_mutation_failed(
                operation,
                Message::new("结果未知", "Outcome unknown"),
                true,
                cx,
            );
            let mut changed = fixture.state.clone();
            changed.settings.scrollback_lines += 1;
            panel.refresh_rule_state(Some(changed), Some(fixture.connection_id), cx);
            assert!(panel.rule_store_blocked);
            panel.reload_rule_state(fixture.state.clone(), Some(fixture.connection_id), cx);
            assert!(!panel.rule_store_blocked);
        });
    })
    .expect("snapshot equality does not preserve reviewed mutation authority");
}

#[gpui_kit::test]
async fn authoritative_identical_reload_expires_runtime_stop_review(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let id = fixture.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start retained owner");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].listening
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        panel.update(cx, |panel, cx| {
            let token = panel
                .rule_stop_review
                .as_ref()
                .expect("runtime review")
                .token;
            panel.reload_rule_state(fixture.state.clone(), Some(fixture.connection_id), cx);
            panel.confirm_rule_stop(token, cx);
            assert!(!panel.rows[0].cancel.load(Ordering::Acquire));
            assert!(panel.rule_stop_review.is_none());
            // The metadata binding can also disappear while the runtime owner
            // remains on this panel. Stop authority comes from that owner.
            panel.refresh_rule_state(None, None, cx);
        });
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
        assert!(panel.read(cx).rows[0].cancel.load(Ordering::Acquire));
    })
    .expect("fresh stop review binds runtime even without a metadata snapshot");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].finished
    })
    .await;
    assert!(!fixture.session.is_closed());
}

#[gpui_kit::test]
fn unresolved_cleanup_ignores_worker_exit_and_closing_until_original_watch_is_closed(
    cx: &mut TestAppContext,
) {
    use crate::tunnels::{TrafficStats, TunnelEvent, TunnelRow, UnresolvedCleanup};
    use keelshell_session::{ConnectionEnd, ConnectionState};
    use std::sync::{atomic::AtomicBool, mpsc};

    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let rule_id = fixture.state.tunnel_rules[0].id;
    // Drive the original lifecycle explicitly: a real loopback disconnect can
    // move past Closing before the UI has a chance to observe that boundary.
    let (original, connection_state) = tokio::sync::watch::channel(ConnectionState::Connected);
    let (events, incoming) = mpsc::sync_channel(1);
    events
        .send(TunnelEvent::CleanupUnconfirmed(UnresolvedCleanup {
            connection_state,
            error: "remote cancellation rejected".into(),
        }))
        .expect("terminal worker event");
    drop(events);
    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, cx| {
            panel.rows.push(TunnelRow {
                id: 0,
                rule_id: Some(rule_id),
                description: Message::new("生命周期夹具", "Lifecycle fixture"),
                status: Message::new("正在停止…", "Stopping…"),
                cancel: Arc::new(AtomicBool::new(true)),
                incoming,
                finished: false,
                listening: true,
                failed: false,
                unresolved_cleanup: None,
                proxy_address: None,
                traffic: TrafficStats::default(),
                last_traffic: Default::default(),
            });
            assert!(panel.retire_workers(cx), "worker exit is not cleanup proof");
            assert!(
                panel.session.is_none(),
                "only the lifecycle receiver survives retirement"
            );
            assert!(panel.has_active_rule(rule_id));
            assert!(panel.rows[0].failed);
            let failure = panel.rows[0].status.clone();
            panel.stop_worker(0);
            assert_eq!(
                panel.rows[0].status, failure,
                "a stale stop must preserve the failure"
            );

            original.send_replace(ConnectionState::Closing);
            assert!(
                panel.retire_workers(cx),
                "a shutdown request is not closure"
            );
            assert!(panel.has_active_rule(rule_id));
            assert_eq!(panel.rows[0].status, failure);
            assert!(panel.status.render(cx).contains("Cleanup unconfirmed"));
            original.send_replace(ConnectionState::Closed(ConnectionEnd::LocalClosed));
            assert!(
                !panel.retire_workers(cx),
                "actual closure releases the retiring owner"
            );
            assert!(!panel.has_active_rule(rule_id));
            assert!(panel.rows[0].finished && panel.rows[0].failed);
            assert!(panel.rows[0].unresolved_cleanup.is_none());
            assert!(panel.rows[0].status.render(cx).contains("cleanup failure"));
            assert!(panel.status.render(cx).contains("1 failed"));
        });
    })
    .expect("original lifecycle closure is the ownership boundary");
}

#[gpui_kit::test]
async fn rejected_cancel_lifecycle_observer_does_not_keep_original_session_alive(
    cx: &mut TestAppContext,
) {
    use crate::tunnels::test_support::{assert_rule_cleanup_failure, rule_cleanup_unconfirmed};
    use keelshell_session::ConnectionState;

    let (mut fixture, cleanup) = SshFixture::with_rejected_cleanup();
    let rule = &mut fixture.state.tunnel_rules[0];
    rule.kind = TunnelRuleKind::Remote;
    rule.target_host = Some("127.0.0.1".into());
    rule.target_port = Some(22);
    let rule_id = rule.id;
    let original_state = fixture.session.subscribe_state();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start original listener");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].listening
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("request cancellation rejected by the real SSH server");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        rule_cleanup_unconfirmed(panel.read(cx), rule_id)
    })
    .await;
    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, cx| {
            assert!(panel.retire_workers(cx));
            assert_rule_cleanup_failure(panel, rule_id, false, cx);
        });
    })
    .expect("retire the panel's strong session authority");
    assert_eq!(*original_state.borrow(), ConnectionState::Connected);

    // Keep the original runtime/server alive, replacing only the fixture's last
    // strong session handle. No explicit close/abort can mask an observer leak.
    let replacement = SshFixture::new();
    drop(std::mem::replace(
        &mut fixture.session,
        replacement.session.clone(),
    ));
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        matches!(*original_state.borrow(), ConnectionState::Closed(_))
            && cleanup.listener_dropped.load(Ordering::Acquire)
            && panel.read(cx).rows[0].finished
    })
    .await;
    cx.update_window(window, |_, _, cx| {
        panel.update(cx, |panel, cx| {
            assert!(!panel.retire_workers(cx));
            assert_rule_cleanup_failure(panel, rule_id, true, cx);
        });
    })
    .expect("passive lifecycle watch releases the original owner after its last handle drops");
    assert!(!replacement.session.is_closed());
}
