//! Regression coverage for Workspace admission, reload and retiring owners.

use super::*;
use crate::assistant::AssistantEvent;
use crate::tunnels::{
    TunnelsPanel,
    test_support::{
        assert_rule_cleanup_failure, rule_admission, rule_cleanup_unconfirmed, rule_listening,
        try_reviewed_start,
    },
};
use gpui_kit::{Context, Window};
use keelshell_session::{ConnectionState, SshSession};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

struct TunnelTab {
    panel: Entity<TunnelsPanel>,
    _output: mpsc::SyncSender<keelshell_session::SessionEvent>,
    _commands: mpsc::Receiver<crate::terminal::TerminalCommand>,
}

fn current_panel(workspace: &Workspace) -> Entity<TunnelsPanel> {
    workspace.panels[&workspace.tabs[workspace.active].entity_id()]
        .tunnels
        .clone()
        .expect("active tunnel panel")
}

fn add_tab(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> TunnelTab {
    let source = workspace.tabs[0].entity_id();
    let session = workspace.remote_sessions[&source].clone();
    let connection = workspace.remote_connection_ids[&source];
    add_tab_with_session(workspace, session, connection, window, cx)
}

fn add_tab_with_session(
    workspace: &mut Workspace,
    session: SshSession,
    connection: uuid::Uuid,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> TunnelTab {
    let (output, incoming) = mpsc::sync_channel(32);
    let (outgoing, commands) = mpsc::sync_channel(32);
    let terminal = cx.new(|cx| {
        TerminalView::from_transport(
            "second fixture".into(),
            14.,
            1000,
            incoming,
            outgoing,
            Arc::new(AtomicBool::new(false)),
            cx,
        )
    });
    let id = terminal.entity_id();
    workspace.tabs.push(terminal);
    workspace.active = workspace.tabs.len() - 1;
    workspace.remote_sessions.insert(id, session);
    workspace.remote_connection_ids.insert(id, connection);
    workspace.remote_hosts.insert(id, "second fixture".into());
    workspace.visible_panel = None;
    workspace.toggle_panel(ToolPanel::Tunnels, window, cx);
    TunnelTab {
        panel: current_panel(workspace),
        _output: output,
        _commands: commands,
    }
}

#[gpui_kit::test]
async fn conflict_blocks_new_workspace_panels_until_authoritative_reload(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{rule_id}"), cx);
        fill(window, "tunnel-rule-name", "stale draft", cx);
        click(window, "review-save-tunnel-rule", cx);
    })
    .expect("review named mutation");
    let external = StateStore::new(fixture.store.path());
    let mut changed = external.load().expect("external snapshot");
    changed.tunnel_rules[0].name = "external winner".into();
    external.save(&changed).expect("external write");
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("submit conflicting mutation");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        !workspace.saving && workspace.tunnel_rule_store_blocked
    })
    .await;

    let new_tab = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture
                .workspace
                .update(cx, |workspace, cx| add_tab(workspace, window, cx))
        })
        .expect("open a real tab after the failed save");
    let before = std::fs::read(fixture.store.path()).expect("external bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        assert_eq!(rule_admission(new_tab.panel.read(cx)), (false, true));
        for action in [
            format!("named-tunnel-rule-{rule_id}"),
            format!("edit-tunnel-rule-{rule_id}"),
            format!("delete-tunnel-rule-{rule_id}"),
            "new-tunnel-rule".into(),
        ] {
            click(window, action, cx);
        }
        window.render_frame(cx);
        assert!(window.try_find("review-start-named-tunnel").is_none());
        assert!(window.try_find("tunnel-rule-editor").is_none());
        assert!(window.try_find("confirm-tunnel-rule-change").is_none());
        assert!(!new_tab.panel.read(cx).has_active_rule(rule_id));

        let state = fixture.store.load().expect("authoritative fixture read");
        fixture.workspace.update(cx, |workspace, cx| {
            // Receiving different bytes/revision through an ordinary refresh
            // must not masquerade as completion of the explicit reload.
            workspace.state = state.clone();
            workspace.refresh_tunnel_panels(cx);
            assert!(workspace.tunnel_rule_store_blocked);
            assert_eq!(rule_admission(new_tab.panel.read(cx)), (false, true));
            workspace.accept_reloaded_knowledge_state(state, window, cx);
            assert!(!workspace.tunnel_rule_store_blocked);
        });
        assert_eq!(rule_admission(new_tab.panel.read(cx)), (false, false));
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
        assert!(new_tab.panel.read(cx).has_active_rule(rule_id));
        new_tab.panel.update(cx, |panel, cx| panel.suspend(cx));
    })
    .expect("only an authoritative read restores admission");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !new_tab.panel.read(cx).has_active_rule(rule_id)
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("final bytes"),
        before
    );
}

fn prepare_capture(fixture: &Fixture, cx: &mut TestAppContext) -> uuid::Uuid {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = true;
            let session_id = format!("{:?}", workspace.tabs[0].entity_id());
            workspace.assistant.update(cx, |assistant, cx| {
                assistant.set_context(
                    "An explicitly captured test result.".into(),
                    "isolated SSH fixture".into(),
                    session_id,
                    cx,
                );
            });
            cx.notify();
        });
        click(window, "knowledge-preview-action", cx);
        click(window, "knowledge-confirm-completed", cx);
    })
    .expect("prepare explicit knowledge review");
    fixture.workspace.read_with(cx, |workspace, cx| {
        workspace
            .assistant
            .read(cx)
            .knowledge_review_id_for_test()
            .expect("capture review")
    })
}

async fn reload_updates_existing_and_new_panels(
    library: bool,
    fail: bool,
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let first = fixture
        .workspace
        .read_with(cx, |workspace, _| current_panel(workspace));
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    let review_id = prepare_capture(&fixture, cx);
    if library {
        let entry = fixture.workspace.read_with(cx, |workspace, cx| {
            workspace
                .assistant
                .read(cx)
                .knowledge_save_candidate(review_id)
                .expect("saved entry")
        });
        let mut state = fixture.store.load().expect("load knowledge fixture");
        state
            .record_knowledge_entry(entry.clone())
            .expect("seed entry");
        let saved = fixture.store.save(&state).expect("persist fixture");
        adopt_fixture_state(&fixture, saved.clone(), cx);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.refresh_knowledge_library(cx);
                workspace.refresh_tunnel_panels(cx);
            });
            click(window, "open-knowledge-library", cx);
            click(window, format!("knowledge-entry-{}", entry.id()), cx);
            click(window, "knowledge-library-review-delete", cx);
        })
        .expect("prepare library deletion");
        let external = StateStore::new(fixture.store.path());
        let mut changed = external.load().expect("external read");
        changed.settings.scrollback_lines += 1;
        external.save(&changed).expect("conflict fixture");
        cx.update_window(fixture.window, |_, window, cx| {
            click(window, "knowledge-library-confirm", cx);
        })
        .expect("produce library reload request");
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            let workspace = fixture.workspace.read(cx);
            !workspace.saving && workspace.tunnel_rule_store_blocked
        })
        .await;
    } else {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.assistant.update(cx, |assistant, cx| {
                assistant.mark_knowledge_save_pending(review_id, cx);
                assistant.mark_knowledge_save_failed(review_id, true, cx);
            });
        });
        // Keep a valid pre-reload start review, exercising handler admission
        // independently of the disabled control's painting.
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.show_assistant = false;
                cx.notify();
            });
            click(window, format!("named-tunnel-rule-{rule_id}"), cx);
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.show_assistant = true;
                cx.notify();
            });
        })
        .expect("prepare named start before reload");
    }
    if fail {
        std::fs::write(fixture.store.path(), b"{broken").expect("isolated corrupt reload");
    }
    let opened = Rc::new(RefCell::new(None::<TunnelTab>));
    let observed = opened.clone();
    let first_observed = first.clone();
    let _subscription = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                let assistant = workspace.assistant.clone();
                // Registered after the production subscription: observe its
                // synchronous admission before the async load can complete.
                cx.subscribe_in(
                    &assistant,
                    window,
                    move |workspace, _, event: &AssistantEvent, window, cx| {
                        if !matches!(
                            event,
                            AssistantEvent::ReloadKnowledge { .. }
                                | AssistantEvent::ReloadKnowledgeLibrary { .. }
                        ) {
                            return;
                        }
                        assert!(workspace.state_reloading);
                        assert!(rule_admission(first_observed.read(cx)).0);
                        first_observed.update(cx, try_reviewed_start);
                        assert!(!first_observed.read(cx).has_active_rule(rule_id));
                        let tab = add_tab(workspace, window, cx);
                        assert!(rule_admission(tab.panel.read(cx)).0);
                        *observed.borrow_mut() = Some(tab);
                    },
                )
            })
        })
        .expect("observe real reload admission");
    cx.update_window(fixture.window, |_, window, cx| {
        click(
            window,
            if library {
                "knowledge-library-reload"
            } else {
                "knowledge-reload"
            },
            cx,
        );
    })
    .expect("invoke real reload entry");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        opened.borrow().is_some() && !fixture.workspace.read(cx).state_reloading
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        let opened = opened.borrow();
        let new_panel = &opened.as_ref().expect("tab opened during reload").panel;
        assert_eq!(rule_admission(first.read(cx)), (false, fail));
        assert_eq!(rule_admission(new_panel.read(cx)), (false, fail));
        assert_eq!(fixture.workspace.read(cx).tunnel_rule_store_blocked, fail);
        if !fail {
            first.update(cx, try_reviewed_start);
            assert!(
                !first.read(cx).has_active_rule(rule_id),
                "even an unchanged authoritative reload invalidates the old start review"
            );
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.show_assistant = false;
                cx.notify();
            });
            click(window, format!("named-tunnel-rule-{rule_id}"), cx);
            click(window, "review-start-named-tunnel", cx);
            assert!(
                new_panel.read(cx).has_active_rule(rule_id),
                "new panel must leave busy"
            );
            new_panel.update(cx, |panel, cx| panel.suspend(cx));
        }
    })
    .expect("completion and errors synchronize all panels");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !opened
            .borrow()
            .as_ref()
            .expect("new tab")
            .panel
            .read(cx)
            .has_active_rule(rule_id)
    })
    .await;
}

#[gpui_kit::test]
async fn capture_reload_propagates_busy_and_success(cx: &mut TestAppContext) {
    reload_updates_existing_and_new_panels(false, false, cx).await;
}

#[gpui_kit::test]
async fn capture_reload_propagates_busy_and_failure(cx: &mut TestAppContext) {
    reload_updates_existing_and_new_panels(false, true, cx).await;
}

#[gpui_kit::test]
async fn library_reload_propagates_busy_and_success(cx: &mut TestAppContext) {
    reload_updates_existing_and_new_panels(true, false, cx).await;
}

#[gpui_kit::test]
async fn library_reload_propagates_busy_and_failure(cx: &mut TestAppContext) {
    reload_updates_existing_and_new_panels(true, true, cx).await;
}

#[gpui_kit::test]
async fn closing_tab_guards_deletion_until_remote_cleanup_ack_and_releases_owner(
    cx: &mut TestAppContext,
) {
    let (mut ssh, cleanup) = SshFixture::with_delayed_cleanup();
    let rule = &mut ssh.state.tunnel_rules[0];
    rule.kind = TunnelRuleKind::Remote;
    rule.target_host = Some("127.0.0.1".into());
    rule.target_port = Some(22);
    let rule_id = rule.id;
    let fixture = mount_with_ssh(ssh, cx);
    let closing = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture
                .workspace
                .update(cx, |workspace, cx| add_tab(workspace, window, cx))
        })
        .expect("open owner tab");
    let owner = closing.panel.entity_id();
    let weak = closing.panel.downgrade();
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start real remote listener");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_listening(closing.panel.read(cx), rule_id)
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.close_tab(&super::super::CloseTab, window, cx);
            assert!(workspace.tunnel_owners.contains_key(&owner));
        });
    })
    .expect("close tab before cleanup acknowledgement");
    drop(closing);
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        cleanup.requested.load(Ordering::Acquire)
    })
    .await;
    let before = std::fs::read(fixture.store.path()).expect("pre-delete bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(
            weak.upgrade().is_some(),
            "retiring runtime owner remains reachable"
        );
        click(window, format!("delete-tunnel-rule-{rule_id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("deletion must include the retiring owner");
    cx.run_until_parked();
    fixture
        .workspace
        .read_with(cx, |workspace, _| assert!(!workspace.saving));
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("protected bytes"),
        before
    );
    let address = cleanup
        .address
        .lock()
        .expect("remote address")
        .expect("bound address");
    assert!(std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok());

    cleanup.release.notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture
            .workspace
            .read(cx)
            .tunnel_owners
            .contains_key(&owner)
            && weak.upgrade().is_none()
    })
    .await;
    assert!(std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_err());
    assert!(
        !fixture.ssh.session.is_closed(),
        "shared SSH remains usable"
    );
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("delete-tunnel-rule-{rule_id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("delete after the actual cleanup receipt");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        !workspace.saving
            && !workspace
                .state
                .tunnel_rules
                .iter()
                .any(|rule| rule.id == rule_id)
    })
    .await;
    assert!(
        !fixture
            .store
            .load()
            .expect("saved deletion")
            .tunnel_rules
            .iter()
            .any(|rule| rule.id == rule_id)
    );
}

#[gpui_kit::test]
async fn rejected_remote_cancel_retains_owner_until_original_transport_closes(
    cx: &mut TestAppContext,
) {
    let (mut ssh, cleanup) = SshFixture::with_rejected_cleanup();
    let rule = &mut ssh.state.tunnel_rules[0];
    rule.kind = TunnelRuleKind::Remote;
    rule.target_host = Some("127.0.0.1".into());
    rule.target_port = Some(22);
    let rule_id = rule.id;
    let original_state = ssh.session.subscribe_state();
    let fixture = mount_with_ssh(ssh, cx);
    let closing = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture
                .workspace
                .update(cx, |workspace, cx| add_tab(workspace, window, cx))
        })
        .expect("open runtime owner tab");
    let owner = closing.panel.entity_id();
    let weak = closing.panel.downgrade();
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start real server-owned listener");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_listening(closing.panel.read(cx), rule_id)
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("request reviewed cancellation");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_cleanup_unconfirmed(closing.panel.read(cx), rule_id)
    })
    .await;
    assert!(cleanup.requested.load(Ordering::Acquire));
    assert_eq!(*original_state.borrow(), ConnectionState::Connected);
    assert!(!fixture.ssh.session.is_closed());
    let address = cleanup
        .address
        .lock()
        .expect("listener address")
        .expect("allocated remote port");
    assert!(std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok());
    let before = std::fs::read(fixture.store.path()).expect("protected metadata");
    cx.update_window(fixture.window, |_, window, cx| {
        assert_rule_cleanup_failure(closing.panel.read(cx), rule_id, false, cx);
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.close_tab(&super::super::CloseTab, window, cx);
            assert!(workspace.tunnel_owners.contains_key(&owner));
            // Neither UI retirement nor a successful metadata read confirms
            // that the server accepted cancellation of the original listener.
            workspace.refresh_tunnel_panels(cx);
            let state = fixture.store.load().expect("authoritative metadata read");
            workspace.accept_reloaded_knowledge_state(state, window, cx);
            workspace.retire_tunnel_owners(cx);
            assert!(workspace.tunnel_owners.contains_key(&owner));
        });
        assert_rule_cleanup_failure(closing.panel.read(cx), rule_id, false, cx);
        click(window, format!("delete-tunnel-rule-{rule_id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("retiring owner blocks deletion after a rejected cancellation");
    cx.run_until_parked();
    fixture
        .workspace
        .read_with(cx, |workspace, _| assert!(!workspace.saving));
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("guarded metadata"),
        before
    );

    let unrelated = SshFixture::new();
    let unrelated_state = unrelated.session.subscribe_state();
    unrelated
        .runtime
        .block_on(unrelated.session.close())
        .expect("close a different authenticated connection");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        matches!(*unrelated_state.borrow(), ConnectionState::Closed(_))
    })
    .await;
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.retire_tunnel_owners(cx);
            assert!(workspace.tunnel_owners.contains_key(&owner));
        });
        assert_rule_cleanup_failure(closing.panel.read(cx), rule_id, false, cx);
    })
    .expect("another connection's closure cannot release this listener");
    assert_eq!(*original_state.borrow(), ConnectionState::Connected);
    assert!(std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok());

    fixture
        .ssh
        .runtime
        .block_on(fixture.ssh.session.close())
        .expect("request original transport closure");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        matches!(*original_state.borrow(), ConnectionState::Closed(_))
            && cleanup.listener_dropped.load(Ordering::Acquire)
            && !fixture
                .workspace
                .read(cx)
                .tunnel_owners
                .contains_key(&owner)
    })
    .await;
    cx.update_window(fixture.window, |_, _, cx| {
        assert_rule_cleanup_failure(closing.panel.read(cx), rule_id, true, cx);
        assert!(!closing.panel.read(cx).has_active_rule(rule_id));
    })
    .expect("actual closure releases ownership without relabeling the failure as success");
    assert!(std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_err());
    drop(closing);
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        weak.upgrade().is_none()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, format!("delete-tunnel-rule-{rule_id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("delete after original transport closure");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        !workspace.saving
            && !workspace
                .state
                .tunnel_rules
                .iter()
                .any(|rule| rule.id == rule_id)
    })
    .await;
    assert!(
        !fixture
            .store
            .load()
            .expect("persisted deletion")
            .tunnel_rules
            .iter()
            .any(|rule| rule.id == rule_id)
    );
}

#[gpui_kit::test]
async fn authoritative_removal_of_rule_metadata_keeps_live_rows_stoppable(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let panel = fixture
        .workspace
        .read_with(cx, |workspace, _| current_panel(workspace));
    let first = fixture.ssh.state.tunnel_rules[0].id;
    let second = fixture.ssh.state.tunnel_rules[1].id;
    cx.update_window(fixture.window, |_, window, cx| {
        for id in [first, second, first] {
            click(window, format!("named-tunnel-rule-{id}"), cx);
            click(window, "review-start-named-tunnel", cx);
        }
    })
    .expect("start independent named owners");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_listening(panel.read(cx), first) && rule_listening(panel.read(cx), second)
    })
    .await;

    let external = StateStore::new(fixture.store.path());
    let mut changed = external.load().expect("external read");
    changed
        .remove_tunnel_rule(first)
        .expect("remove saved metadata");
    external.save(&changed).expect("publish metadata removal");
    cx.update_window(fixture.window, |_, window, cx| {
        let state = fixture.store.load().expect("reload removed metadata");
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.accept_reloaded_knowledge_state(state, window, cx);
        });
        window.render_frame(cx);
        assert!(
            window
                .try_find(format!("named-tunnel-rule-{first}"))
                .is_none()
        );
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
        assert!(
            panel.read(cx).has_active_rule(first),
            "confirmation is not a cleanup acknowledgement"
        );
    })
    .expect("review retained runtime owners after deletion");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !panel.read(cx).has_active_rule(first)
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(
            panel.read(cx).has_active_rule(second),
            "other rule keeps its listener"
        );
        assert!(
            !fixture.ssh.session.is_closed(),
            "shared SSH session is retained"
        );
        click(window, ("stop-tunnel", 1_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("stop remaining independent owner");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !panel.read(cx).has_active_rule(second)
    })
    .await;
    assert!(!fixture.ssh.session.is_closed());
}

#[gpui_kit::test]
fn uncertainty_without_open_panels_survives_refresh_and_identical_reload_clears_it(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.panels.clear();
            workspace.retire_tunnel_owners(cx);
            assert!(workspace.tunnel_owners.is_empty());
            workspace.mark_tunnel_rule_store_uncertain(cx);
            workspace.visible_panel = None;
            workspace.toggle_panel(ToolPanel::Tunnels, window, cx);
            let panel = current_panel(workspace);
            assert_eq!(rule_admission(panel.read(cx)), (false, true));
            workspace.refresh_tunnel_panels(cx);
            assert_eq!(rule_admission(panel.read(cx)), (false, true));
            let state = fixture.store.load().expect("unchanged authoritative read");
            assert_eq!(state, workspace.state);
            workspace.accept_reloaded_knowledge_state(state, window, cx);
            assert_eq!(rule_admission(panel.read(cx)), (false, false));
        });
        click(window, "new-tunnel-rule", cx);
        window.render_frame(cx);
        assert!(window.try_find("tunnel-rule-editor").is_some());
    })
    .expect("workspace keeps uncertainty independently of visible panel lifetime");
}

#[gpui_kit::test]
async fn configuration_recovery_clears_uncertainty_before_a_new_tunnel_panel(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    fixture
        .store
        .create_config_backup()
        .expect("valid rule backup");
    std::fs::write(fixture.store.path(), b"{damaged-recovery-fixture")
        .expect("damage isolated configuration");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.mark_tunnel_rule_store_uncertain(cx);
            workspace.close_tab(&super::super::CloseTab, window, cx);
            assert!(workspace.tabs.is_empty());
            assert!(workspace.tunnel_owners.is_empty());
            assert!(workspace.tunnel_rule_store_blocked);
            workspace.open_configuration_recovery(window, cx);
        });
    })
    .expect("open recovery with no live Workspace authority");
    wait_for_recovery(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, ("configuration-backup", 0_usize), cx);
    })
    .expect("review actual backup");
    wait_for_recovery(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(fixture.workspace.read(cx).tunnel_rule_store_blocked);
        click(window, "configuration-recovery-acknowledge", cx);
        click(window, "configuration-recovery-restore", cx);
    })
    .expect("approve exact restoration through production subscription");
    wait_for_recovery(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        assert!(!workspace.tunnel_rule_store_blocked);
        assert_eq!(workspace.state.tunnel_rules, fixture.ssh.state.tunnel_rules);
        assert!(workspace.tabs.is_empty());
        assert!(workspace.remote_sessions.is_empty());
        assert!(workspace.tunnel_owners.is_empty());
        click(window, "configuration-recovery-close", cx);
    })
    .expect("accepted recovery clears uncertainty without replaying runtime owners");

    // Closing the last production tab closes its authenticated connection.
    // Reconnect through a fresh isolated SSH fixture before exercising a rule
    // on the recovered configuration.
    let reconnected = SshFixture::new();
    let new_tab = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                assert!(workspace.configuration_recovery.is_none());
                add_tab_with_session(
                    workspace,
                    reconnected.session.clone(),
                    fixture.ssh.connection_id,
                    window,
                    cx,
                )
            })
        })
        .expect("explicitly attach a fresh panel after recovery");
    cx.update_window(fixture.window, |_, window, cx| {
        assert_eq!(rule_admission(new_tab.panel.read(cx)), (false, false));
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("review a recovered rule in the new panel");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        rule_listening(new_tab.panel.read(cx), rule_id)
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("stop the newly reviewed runtime owner");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !new_tab.panel.read(cx).has_active_rule(rule_id)
    })
    .await;
    assert_eq!(
        fixture
            .store
            .load()
            .expect("restored rule metadata")
            .tunnel_rules,
        fixture.ssh.state.tunnel_rules
    );
    assert!(!reconnected.session.is_closed());
}

async fn wait_for_recovery(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .configuration_recovery
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).pending_for_test())
    })
    .await;
}
