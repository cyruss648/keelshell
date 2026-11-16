//! Panel-local reload admission, stale callbacks and native component geometry.
#![allow(clippy::expect_used)]

use super::*;
use crate::tunnels::test_support::{SshFixture, activate_painted, click};
use ::core::prelude::v1::test;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use keelshell_core::{Language, Settings, Theme, ThemeId};
use std::{cell::RefCell, rc::Rc};

fn pending(panel: &TunnelsPanel) -> TunnelRuleReloadEvent {
    panel.rule_reload_pending.clone().expect("pending reload")
}

fn result_message() -> Message {
    Message::new("重新加载回执", "Reload receipt")
}

fn assert_in_view(window: &Window, id: &'static str) {
    let viewport = window.find("tunnels-panel").bounds();
    let node = window.find(id);
    let bounds = node.bounds();
    assert!(node.visible(), "{id} must remain visible");
    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
    assert!(
        bounds.origin.x >= viewport.origin.x && bounds.right() <= viewport.right(),
        "{id} must fit horizontally: {bounds:?} in {viewport:?}",
    );
    assert!(
        bounds.top() >= viewport.top() && bounds.bottom() <= viewport.bottom(),
        "{id} must fit vertically: {bounds:?} in {viewport:?}",
    );
}

#[gpui_kit::test]
fn reload_button_emits_one_exact_request_without_snapshot_or_binding(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event: &TunnelRuleReloadEvent, _| {
            observed.borrow_mut().push(event.clone());
        })
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.refresh_rule_state(None, None, cx);
            panel.mark_rule_store_uncertain(cx);
        });
        click(window, "reload-tunnel-configuration", cx);
        let request = pending(panel.read(cx));
        assert_eq!(request.connection_id, None);
        assert_eq!(request.epoch, panel.read(cx).rule_epoch);
        assert!(panel.read(cx).accepts_rule_reload(&request));
        assert!(panel.read(cx).rule_store_blocked);
        assert!(!panel.read(cx).rule_configuration_busy);
        panel.update(cx, |panel, cx| {
            panel.request_rule_reload(panel.rule_epoch, cx)
        });
        click(window, "reload-tunnel-configuration", cx);
        assert_eq!(panel.read(cx).rule_reload_pending.as_ref(), Some(&request));
        window.render_frame(cx);
        assert_eq!(
            window.find("reload-tunnel-configuration").disabled(),
            Some(true)
        );
        assert_eq!(
            window.find("tunnel-rule-feedback").label(),
            Some("Reloading configuration; draft retained and previous reviews expired…"),
        );
        assert!(panel.read(cx).rule_mutation.is_none());
        assert!(panel.read(cx).rows.is_empty());
    })
    .expect("reload remains available for missing or uncertain configuration");
    panel.read_with(cx, |panel, _| {
        assert_eq!(*events.borrow(), vec![pending(panel)]);
    });
}

#[gpui_kit::test]
fn reload_busy_and_suspension_reject_new_requests_and_retire_pending(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.set_rule_configuration_busy(true, cx));
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            panel.request_rule_reload(panel.rule_epoch, cx);
            assert!(panel.rule_reload_pending.is_none());
            panel.set_rule_configuration_busy(false, cx);
        });
        click(window, "reload-tunnel-configuration", cx);
        let request = pending(panel.read(cx));
        panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(!panel.accepts_rule_reload(&request));
            assert!(!panel.finish_rule_reload(&request, result_message(), cx));
            panel.request_rule_reload(panel.rule_epoch, cx);
            assert!(panel.rule_reload_pending.is_none());
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("reload-tunnel-configuration").disabled(),
            Some(true)
        );
    })
    .expect("busy admission and retired panel authority");
}

#[gpui_kit::test]
fn reload_refuses_an_inflight_metadata_mutation(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let rule_id = fixture.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
        click(window, "review-save-tunnel-rule", cx);
        click(window, "confirm-tunnel-rule-change", cx);
        let mutation = panel.read(cx).rule_mutation.clone().expect("pending save");
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            panel.request_rule_reload(panel.rule_epoch, cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(
                panel
                    .rule_mutation
                    .as_ref()
                    .is_some_and(|event| event == &mutation)
            );
            assert!(panel.accepts_rule_mutation(&mutation));
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("reload-tunnel-configuration").disabled(),
            Some(true)
        );
    })
    .expect("reload cannot replace a pending metadata write");
}

#[gpui_kit::test]
fn reload_finish_accepts_busy_exact_token_without_clearing_storage_block(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.mark_rule_store_uncertain(cx));
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let request = pending(panel);
            panel.set_rule_configuration_busy(true, cx);
            assert!(panel.accepts_rule_reload(&request));
            for forged in [
                TunnelRuleReloadEvent {
                    operation: uuid::Uuid::new_v4(),
                    ..request.clone()
                },
                TunnelRuleReloadEvent {
                    epoch: uuid::Uuid::new_v4(),
                    ..request.clone()
                },
                TunnelRuleReloadEvent {
                    connection_id: None,
                    ..request.clone()
                },
            ] {
                assert!(!panel.accepts_rule_reload(&forged));
                assert!(!panel.finish_rule_reload(&forged, result_message(), cx));
                panel.reject_rule_reload(&forged, result_message(), cx);
                assert_eq!(panel.rule_reload_pending.as_ref(), Some(&request));
            }
            assert!(panel.finish_rule_reload(&request, result_message(), cx));
            assert!(panel.rule_reload_pending.is_none());
            assert_eq!(panel.rule_status, result_message());
            assert!(panel.rule_store_blocked);
            assert!(panel.rule_configuration_busy);
            assert!(!panel.finish_rule_reload(&request, Message::empty(), cx));
            assert_eq!(panel.rule_status, result_message());
            panel.set_rule_configuration_busy(false, cx);
            assert!(panel.rule_store_blocked);
            panel.reload_rule_state(fixture.state.clone(), Some(fixture.connection_id), cx);
            assert!(!panel.rule_store_blocked);
        });
    })
    .expect("only authoritative adoption resolves an uncertain store");
}

#[gpui_kit::test]
fn reload_rejection_clears_matching_expired_epoch_but_preserves_replacement(
    cx: &mut TestAppContext,
) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.mark_rule_store_uncertain(cx));
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let old = pending(panel);
            panel.rule_epoch = uuid::Uuid::new_v4();
            assert!(!panel.finish_rule_reload(&old, result_message(), cx));
            panel.reject_rule_reload(&old, result_message(), cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(panel.rule_store_blocked);
            assert_eq!(panel.rule_status, result_message());
            panel.request_rule_reload(panel.rule_epoch, cx);
            let replacement = pending(panel);
            assert_ne!(old.operation, replacement.operation);
            let current_status = panel.rule_status.clone();
            panel.reject_rule_reload(&old, Message::empty(), cx);
            assert_eq!(panel.rule_reload_pending.as_ref(), Some(&replacement));
            assert_eq!(panel.rule_status, current_status);
            assert!(panel.accepts_rule_reload(&replacement));
            assert!(panel.rule_store_blocked);
        });
    })
    .expect("rejection is token-bound rather than epoch-bound");
}

#[gpui_kit::test]
fn reload_refresh_retires_changed_state_or_connection_but_keeps_identical_refresh(
    cx: &mut TestAppContext,
) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let old = pending(panel);
            panel.refresh_rule_state(Some(fixture.state.clone()), Some(fixture.connection_id), cx);
            assert!(panel.accepts_rule_reload(&old));
            let mut changed = fixture.state.clone();
            changed.settings.scrollback_lines += 1;
            panel.refresh_rule_state(Some(changed.clone()), Some(fixture.connection_id), cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(!panel.finish_rule_reload(&old, result_message(), cx));
            panel.request_rule_reload(panel.rule_epoch, cx);
            let replacement = pending(panel);
            panel.reject_rule_reload(&old, Message::empty(), cx);
            assert!(panel.accepts_rule_reload(&replacement));
            panel.refresh_rule_state(Some(changed), None, cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(!panel.finish_rule_reload(&replacement, result_message(), cx));
        });
    })
    .expect("ordinary refresh has exact snapshot and connection ownership");
}

#[gpui_kit::test]
fn reload_authoritative_identical_snapshot_retires_pending_token(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.mark_rule_store_uncertain(cx));
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let old = pending(panel);
            panel.reload_rule_state(fixture.state.clone(), Some(fixture.connection_id), cx);
            assert!(panel.rule_reload_pending.is_none());
            assert_ne!(panel.rule_epoch, old.epoch);
            assert!(!panel.finish_rule_reload(&old, result_message(), cx));
            assert!(!panel.rule_store_blocked);
        });
    })
    .expect("authoritative reads invalidate tokens even if bytes are unchanged");
}

#[gpui_kit::test]
fn reload_uncertainty_retires_pending_without_leaving_a_busy_panel(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let old = pending(panel);
            panel.mark_rule_store_uncertain(cx);
            assert!(panel.rule_reload_pending.is_none());
            assert!(panel.rule_store_blocked);
            assert!(!panel.rule_configuration_busy);
            assert!(!panel.accepts_rule_reload(&old));
            assert!(!panel.finish_rule_reload(&old, result_message(), cx));
        });
        window.render_frame(cx);
        assert_ne!(window.find("reload-tunnel-configuration").disabled(), Some(true));
        assert_eq!(
            window.find("tunnel-rule-feedback").label(),
            Some("Configuration changed or a write outcome is unknown. Draft retained; use Reload in this panel before reviewing again."),
        );
        click(window, "reload-tunnel-configuration", cx);
        assert!(panel.read(cx).rule_reload_pending.is_some());
        assert!(panel.read(cx).rule_store_blocked);
    })
    .expect("needs-reload is distinct from an actual configuration operation");
}

#[gpui_kit::test]
fn reload_old_painted_epoch_cannot_reload_replacement_configuration(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let position = window.find("reload-tunnel-configuration").bounds().center();
        panel.update(cx, |panel, cx| {
            let mut changed = fixture.state.clone();
            changed.settings.scrollback_lines += 1;
            panel.refresh_rule_state(Some(changed), Some(fixture.connection_id), cx);
        });
        activate_painted(window, position, cx);
        assert!(panel.read(cx).rule_reload_pending.is_none());
        click(window, "reload-tunnel-configuration", cx);
        assert!(panel.read(cx).rule_reload_pending.is_some());
    })
    .expect("painted reload callbacks retain their original epoch");
}

#[gpui_kit::test]
fn reload_preserves_draft_snapshot_and_expires_metadata_review(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let rule_id = fixture.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
        panel.update(cx, |panel, cx| {
            panel
                .rule_editor
                .as_ref()
                .expect("editor")
                .name
                .update(cx, |input, cx| {
                    input.set_value("retained draft 中文  ", window, cx);
                });
        });
        click(window, "review-save-tunnel-rule", cx);
        let (identity, snapshot, name_entity, operation) = {
            let panel = panel.read(cx);
            let editor = panel.rule_editor.as_ref().expect("reviewed editor");
            (
                editor.identity,
                editor.state.clone(),
                editor.name.entity_id(),
                panel
                    .rule_change_review
                    .as_ref()
                    .expect("review")
                    .event
                    .operation,
            )
        };
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            let request = pending(panel);
            assert_eq!(request.epoch, panel.rule_epoch);
            assert!(panel.rule_plan.is_none());
            assert!(panel.rule_stop_review.is_none());
            assert!(panel.recovery_review.is_none());
            assert!(panel.rule_change_review.is_none());
            let editor = panel.rule_editor.as_ref().expect("retained draft");
            let kind = editor.kind;
            let enabled = editor.enabled;
            assert_eq!(editor.identity, identity);
            assert_eq!(editor.name.entity_id(), name_entity);
            assert!(Arc::ptr_eq(&editor.state, &snapshot));
            assert_eq!(
                editor.name.read(cx).value(),
                SharedString::from("retained draft 中文  ")
            );
            panel.confirm_rule_change(operation, cx);
            panel.review_rule_save(identity, cx);
            panel.cancel_rule_edit(identity, cx);
            panel.set_rule_kind(identity, TunnelRuleKind::Local, cx);
            panel.toggle_rule_enabled(identity, cx);
            panel.begin_new_rule(panel.rule_epoch, window, cx);
            assert!(panel.rule_mutation.is_none());
            assert!(panel.rule_change_review.is_none());
            let editor = panel
                .rule_editor
                .as_ref()
                .expect("pending reload retains draft");
            assert_eq!(editor.identity, identity);
            assert_eq!(editor.kind, kind);
            assert_eq!(editor.enabled, enabled);
            assert!(panel.finish_rule_reload(&request, result_message(), cx));
            let mut changed = fixture.state.clone();
            changed.tunnel_rules[0].name = "external winner".into();
            panel.reload_rule_state(changed, Some(fixture.connection_id), cx);
            panel.review_rule_save(identity, cx);
            assert!(panel.rule_change_review.is_none());
            assert!(
                panel
                    .rule_status
                    .render(cx)
                    .contains("editor snapshot expired")
            );
            let editor = panel.rule_editor.as_ref().expect("original snapshot");
            assert!(Arc::ptr_eq(&editor.state, &snapshot));
            assert_eq!(
                editor.name.read(cx).value(),
                SharedString::from("retained draft 中文  ")
            );
        });
    })
    .expect("reload never silently rebases retained editor text");
}

#[gpui_kit::test]
fn reload_expires_named_start_and_recovery_reviews_without_starting_workers(
    cx: &mut TestAppContext,
) {
    let mut fixture = SshFixture::new();
    let rule = fixture.state.tunnel_rules[0].clone();
    let snapshot = TunnelRuntimeSnapshot::new(
        rule.clone(),
        TunnelConnectionBinding::from_state(&fixture.state, fixture.connection_id)
            .expect("fixture connection"),
        "127.0.0.1:40123",
        1_700_000_000,
    )
    .expect("previous observation");
    let snapshot_id = snapshot.id;
    fixture
        .state
        .upsert_tunnel_runtime_snapshot(snapshot)
        .expect("retain candidate");
    let (window, panel) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{}", rule.id), cx);
        let start = panel
            .read(cx)
            .rule_plan
            .as_ref()
            .expect("start review")
            .review_token();
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            panel.start_named_rule(start, cx);
            panel.select_named_rule(rule.id, panel.rule_epoch, cx);
            assert!(panel.rule_plan.is_none());
            assert!(panel.rows.is_empty());
            let request = pending(panel);
            panel.reject_rule_reload(&request, result_message(), cx);
        });
        click(window, format!("review-recovery-start-{snapshot_id}"), cx);
        let recovery = panel
            .read(cx)
            .recovery_review
            .as_ref()
            .expect("recovery review")
            .plan
            .review_token();
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            panel.confirm_recovery_start(recovery, cx);
            panel.review_recovery_start(snapshot_id, cx);
            panel.discard_recovery_candidate(snapshot_id, cx);
            assert!(panel.recovery_review.is_none());
            assert!(panel.recovery_discard_pending.is_none());
            assert_eq!(panel.recovery_candidates().len(), 1);
            assert!(panel.rows.is_empty());
        });
    })
    .expect("reload retires both start authorities while preserving the candidate");
}

#[gpui_kit::test]
async fn reload_retires_stop_review_and_retains_original_live_listener(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, panel) = fixture.mount(cx);
    let rule_id = fixture.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start real loopback listener");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].listening
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        let stop = panel
            .read(cx)
            .rule_stop_review
            .as_ref()
            .expect("stop review")
            .token;
        let owner = panel.read(cx).runtime_owner;
        let listener = panel.read(cx).rows[0].bound_address.clone();
        click(window, "reload-tunnel-configuration", cx);
        panel.update(cx, |panel, cx| {
            panel.confirm_rule_stop(stop, cx);
            let workers = panel.rule_workers(rule_id);
            panel.review_rule_stop(rule_id, panel.rule_epoch, &workers, cx);
            assert!(panel.rule_stop_review.is_none());
            assert!(!panel.rows[0].cancel.load(Ordering::Acquire));
            assert_eq!(panel.runtime_owner, owner);
            assert_eq!(panel.rows[0].bound_address, listener);
            let request = pending(panel);
            panel.reject_rule_reload(&request, result_message(), cx);
        });
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
        assert!(panel.read(cx).rows[0].cancel.load(Ordering::Acquire));
    })
    .expect("a new explicit review is required to stop the retained owner");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).rows[0].finished
    })
    .await;
}

#[gpui_kit::test]
fn reload_button_ax_and_feedback_fit_registered_themes_languages_and_sizes(
    cx: &mut TestAppContext,
) {
    let fixture = SshFixture::new();
    let rule_id = fixture.state.tunnel_rules[0].id;
    for (width, height) in [(900., 580.), (1400., 900.)] {
        let (window, panel) = fixture.mount_sized(width, height, cx);
        for family in crate::design::registry() {
            for theme in [Theme::Light, Theme::Dark] {
                for language in [Language::En, Language::ZhCn] {
                    cx.update_window(window, |_, window, cx| {
                        crate::design::apply_settings(
                            &Settings {
                                theme_family: ThemeId::new(family.id).expect("registered theme"),
                                theme,
                                language,
                                ..Default::default()
                            },
                            Some(window),
                            cx,
                        );
                        crate::i18n::set_language(language, cx);
                        panel.update(cx, |panel, cx| {
                            panel.reload_rule_state(
                                fixture.state.clone(),
                                Some(fixture.connection_id),
                                cx,
                            );
                            panel.refresh_locale(window, cx);
                        });
                        window.render_frame(cx);
                        assert_in_view(window, "reload-tunnel-configuration");
                        assert_in_view(window, "new-tunnel-rule");
                        assert_in_view(window, "tunnel-rule-feedback");
                        let reload = window.find("reload-tunnel-configuration");
                        assert_eq!(reload.role(), Some(accesskit::Role::Button));
                        assert_eq!(
                            reload.label(),
                            Some(if language == Language::En {
                                "Reload local configuration"
                            } else {
                                "重新加载本机配置"
                            })
                        );
                        assert!(
                            window.find("tunnel-rule-feedback").bounds().top()
                                >= reload.bounds().bottom()
                        );
                        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
                        window.render_frame(cx);
                        for id in [
                            "reload-tunnel-configuration",
                            "review-save-tunnel-rule",
                            "cancel-tunnel-rule",
                            "tunnel-rule-feedback",
                        ] {
                            assert_in_view(window, id);
                        }
                        click(window, "review-save-tunnel-rule", cx);
                        window.render_frame(cx);
                        for id in [
                            "reload-tunnel-configuration",
                            "confirm-tunnel-rule-change",
                            "cancel-tunnel-rule-review",
                            "tunnel-rule-feedback",
                        ] {
                            assert_in_view(window, id);
                        }
                        click(window, "reload-tunnel-configuration", cx);
                        window.render_frame(cx);
                        assert_in_view(window, "reload-tunnel-configuration");
                        assert_in_view(window, "tunnel-rule-feedback");
                        assert_eq!(
                            window.find("reload-tunnel-configuration").disabled(),
                            Some(true)
                        );
                        panel.update(cx, |panel, cx| panel.mark_rule_store_uncertain(cx));
                        click(window, "cancel-tunnel-rule", cx);
                        window.render_frame(cx);
                        assert_in_view(window, "reload-tunnel-configuration");
                        assert_in_view(window, "tunnel-rule-feedback");
                        assert!(
                            window.find("tunnel-rule-feedback").bounds().top()
                                >= window.find("reload-tunnel-configuration").bounds().bottom()
                        );
                        assert!(panel.read(cx).rule_reload_pending.is_none());
                        assert!(panel.read(cx).rule_store_blocked);
                        assert!(panel.read(cx).rule_mutation.is_none());
                        assert!(panel.read(cx).rows.is_empty());
                    })
                    .expect("registered theme, bilingual AX and actual painted compact geometry");
                }
            }
        }
    }
}

#[gpui_kit::test]
fn reload_button_tooltip_retranslates_while_hovered(cx: &mut TestAppContext) {
    let fixture = SshFixture::new();
    let (window, _) = fixture.mount(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.hover("reload-tunnel-configuration", cx);
    })
    .expect("hover real reload button");
    cx.run_until_parked();
    cx.dispatcher.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    for (language, label, tooltip_label) in [
        (
            Language::En,
            "Reload local configuration",
            "Reload local configuration; retain drafts and expire previous reviews",
        ),
        (
            Language::ZhCn,
            "重新加载本机配置",
            "重新加载本机配置；保留草稿并撤销旧审核",
        ),
    ] {
        cx.update_window(window, |_, window, cx| {
            let pointer = window.mouse_position();
            crate::i18n::set_language(language, cx);
            window.render_frame(cx);
            let tooltip = window.find("localized-tooltip-content");
            assert!(tooltip.visible());
            assert_eq!(tooltip.label(), Some(tooltip_label));
            assert_eq!(
                window.find("reload-tunnel-configuration").label(),
                Some(label)
            );
            assert_eq!(window.mouse_position(), pointer);
        })
        .expect("full tooltip and AX label follow the current language");
    }
}
