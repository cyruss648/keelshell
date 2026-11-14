//! Production subscription and background StateStore round trips.
#![allow(clippy::expect_used)]

use super::{RemotePanels, ToolPanel, Workspace};
use crate::terminal::TerminalView;
use crate::tunnels::test_support::{SshFixture, click, rule_cleanup_unconfirmed};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use keelshell_core::{
    StateStore, TunnelConnectionBinding, TunnelRuleKind, TunnelRuntimeSnapshot, TunnelRuntimeStatus,
};
use std::{
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::Duration,
};

#[path = "workspace_review_tests.rs"]
mod review_regressions;

struct Fixture {
    window: AnyWindowHandle,
    workspace: Entity<Workspace>,
    store: Arc<StateStore>,
    ssh: SshFixture,
    _directory: tempfile::TempDir,
    _output: mpsc::SyncSender<keelshell_session::SessionEvent>,
    _commands: mpsc::Receiver<crate::terminal::TerminalCommand>,
}

fn mount(cx: &mut TestAppContext) -> Fixture {
    mount_with_ssh(SshFixture::new(), cx)
}

fn mount_with_ssh(ssh: SshFixture, cx: &mut TestAppContext) -> Fixture {
    let directory = tempfile::tempdir().expect("isolated state");
    let store = Arc::new(StateStore::new(directory.path().join("state.json")));
    let mut state = store.load().expect("load state");
    state.connections = ssh.state.connections.clone();
    state.tunnel_rules = ssh.state.tunnel_rules.clone();
    state.settings.updates.frequency = keelshell_core::UpdateCheckFrequency::Disabled;
    let state = store.save(&state).expect("save fixture");
    let (output, incoming) = mpsc::sync_channel(32);
    let (outgoing, commands) = mpsc::sync_channel(32);
    let (window, workspace) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1600.), px(1200.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    Workspace::new(store.clone(), state, None, ssh.runtime.clone(), window, cx)
                })
            },
        )
        .expect("mount workspace")
    });
    cx.update_window(window, |_, window, cx| {
        let terminal = cx.new(|cx| {
            TerminalView::from_transport(
                "fixture".into(),
                14.,
                1000,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            )
        });
        workspace.update(cx, |workspace, cx| {
            let id = terminal.entity_id();
            workspace.tabs.push(terminal);
            workspace.active = 0;
            workspace.remote_sessions.insert(id, ssh.session.clone());
            workspace
                .remote_connection_ids
                .insert(id, ssh.connection_id);
            workspace.remote_hosts.insert(id, "rule fixture".into());
            workspace.show_connections = false;
            workspace.toggle_panel(ToolPanel::Tunnels, window, cx);
        });
    })
    .expect("production tunnel subscription");
    Fixture {
        window,
        workspace,
        store,
        ssh,
        _directory: directory,
        _output: output,
        _commands: commands,
    }
}

/// Adopt a fixture snapshot through the same shared gate boundary as a
/// completed production reload.
///
/// Tunnel review regressions sometimes seed additional rules or knowledge
/// entries after mounting the real workspace. Updating only `Workspace::state`
/// would leave the process-wide configuration gate at the older revision, so
/// every guarded action would be rejected before it reaches StateStore.
fn adopt_fixture_state(
    fixture: &Fixture,
    state: keelshell_core::AppState,
    cx: &mut TestAppContext,
) {
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.state = state.clone();
            let gate = workspace.app_windows.read(cx).gate.clone();
            let reload = crate::app_windows::ConfigurationGate::begin_reload_lease(
                &gate,
                workspace.window_id,
            )
            .expect("begin fixture configuration adoption");
            reload.finish(Some(&state));
            workspace.configuration_epoch = gate
                .lock()
                .expect("lock fixture configuration gate")
                .epoch();
            workspace.configuration_invalidated = false;
            cx.notify();
        });
    })
    .expect("adopt persisted fixture configuration");
}

fn fill(window: &mut gpui_kit::Window, id: &str, value: &str, cx: &mut gpui_kit::App) {
    click(window, id.to_owned(), cx);
    window.press(
        if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        },
        cx,
    );
    window.input(value, cx);
}

fn current_tunnel_panel(
    fixture: &Fixture,
    cx: &mut TestAppContext,
) -> Entity<crate::tunnels::TunnelsPanel> {
    fixture.workspace.read_with(cx, |workspace, _| {
        let terminal = workspace.tabs[workspace.active].entity_id();
        workspace.panels[&terminal]
            .tunnels
            .clone()
            .expect("tunnel panel")
    })
}

fn seed_runtime_candidate(fixture: &Fixture, cx: &mut TestAppContext) -> uuid::Uuid {
    let mut state = fixture.store.load().expect("load seed state");
    let rule = state.tunnel_rules[0].clone();
    let snapshot = TunnelRuntimeSnapshot::new(
        rule,
        TunnelConnectionBinding::from_state(&state, fixture.ssh.connection_id)
            .expect("connection binding"),
        "127.0.0.1:40123",
        1_700_000_000,
    )
    .expect("runtime candidate");
    let id = snapshot.id;
    state
        .upsert_tunnel_runtime_snapshot(snapshot)
        .expect("upsert runtime candidate");
    let saved = fixture
        .store
        .save(&state)
        .expect("persist runtime candidate");
    adopt_fixture_state(fixture, saved, cx);
    cx.update_window(fixture.window, |_, _, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.refresh_tunnel_panels(cx));
    })
    .expect("refresh runtime candidate");
    id
}

#[gpui_kit::test]
async fn tunnel_rule_crud_round_trips_through_workspace_background_store(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let window = fixture.window;
    let before = std::fs::read(fixture.store.path()).expect("initial bytes");
    cx.update_window(window, |_, window, cx| {
        click(window, "new-tunnel-rule", cx);
        fill(window, "tunnel-rule-name", "Saved via UI", cx);
        fill(window, "tunnel-rule-target-port", "5432", cx);
        click(window, "tunnel-rule-enabled", cx);
        click(window, "review-save-tunnel-rule", cx);
        assert_eq!(
            std::fs::read(fixture.store.path()).expect("unconfirmed bytes"),
            before
        );
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("create rule via actual controls");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
            && fixture.workspace.read(cx).state.tunnel_rules.len() == 3
    })
    .await;
    let saved = fixture.store.load().expect("read saved rule");
    let rule = saved
        .tunnel_rules
        .iter()
        .find(|rule| rule.name == "Saved via UI")
        .expect("new rule");
    let id = rule.id;
    assert_eq!(rule.connection_id, fixture.ssh.connection_id);
    assert_eq!(rule.target_port, Some(5432));
    assert!(rule.enabled);
    cx.update_window(window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{id}"), cx);
        fill(window, "tunnel-rule-name", "Edited via UI", cx);
        click(window, "tunnel-rule-kind-dynamic", cx);
        click(window, "review-save-tunnel-rule", cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("edit saved rule");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
            && fixture
                .workspace
                .read(cx)
                .state
                .tunnel_rules
                .iter()
                .any(|rule| rule.name == "Edited via UI")
    })
    .await;
    let edited = fixture.store.load().expect("edited disk");
    let rule = edited
        .tunnel_rules
        .iter()
        .find(|rule| rule.id == id)
        .expect("same stable rule ID");
    assert_eq!(rule.kind, TunnelRuleKind::Dynamic);
    assert!(rule.target_host.is_none() && rule.target_port.is_none());
    let before_delete = std::fs::read(fixture.store.path()).expect("before delete");
    cx.update_window(window, |_, window, cx| {
        click(window, format!("delete-tunnel-rule-{id}"), cx);
        click(window, "cancel-tunnel-rule-review", cx);
        assert_eq!(
            std::fs::read(fixture.store.path()).expect("cancelled delete"),
            before_delete
        );
        click(window, format!("delete-tunnel-rule-{id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("cancel then confirm deletion");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
            && fixture.workspace.read(cx).state.tunnel_rules.len() == 2
    })
    .await;
    let final_state = fixture.store.load().expect("final disk");
    assert!(!final_state.tunnel_rules.iter().any(|rule| rule.id == id));
    assert_eq!(final_state.connections, saved.connections);
}

#[gpui_kit::test]
async fn tunnel_rule_external_conflict_retains_external_bytes_and_requires_reload(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let window = fixture.window;
    let id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("edit-tunnel-rule-{id}"), cx);
        fill(window, "tunnel-rule-name", "conflicting draft", cx);
        click(window, "review-save-tunnel-rule", cx);
    })
    .expect("prepare review");
    let external = StateStore::new(fixture.store.path());
    let mut state = external.load().expect("external read");
    state.tunnel_rules[0].name = "external winner".into();
    external.save(&state).expect("external write");
    let winner = std::fs::read(fixture.store.path()).expect("external bytes");
    cx.update_window(window, |_, window, cx| {
        click(window, "confirm-tunnel-rule-change", cx)
    })
    .expect("attempt stale store save");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        let workspace = fixture.workspace.read(cx);
        let status = workspace.status.render(cx);
        !workspace.saving && status.contains("保存失败")
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("conflict bytes"),
        winner
    );
    cx.update_window(window, |_, window, cx| {
        click(window, "review-save-tunnel-rule", cx);
        window.render_frame(cx);
        assert!(window.try_find("confirm-tunnel-rule-change").is_none());
        assert!(
            window.try_find("tunnel-rule-editor").is_some(),
            "draft retained after conflict"
        );
    })
    .expect("conflict cannot silently retry");
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("after retry"),
        winner
    );
}

#[gpui_kit::test]
async fn tunnel_rule_delete_rechecks_active_owners_in_other_tabs(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let window = fixture.window;
    let (other_window, other_panel) = fixture.ssh.mount(cx);
    let id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(other_window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("start other panel worker");
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, _| {
            workspace.archived_panels.insert(
                other_panel.entity_id(),
                RemotePanels {
                    tunnels: Some(other_panel.clone()),
                    ..Default::default()
                },
            );
        })
    })
    .expect("retain original worker panel");
    let before = std::fs::read(fixture.store.path()).expect("before deletion");
    cx.update_window(window, |_, window, cx| {
        click(window, format!("delete-tunnel-rule-{id}"), cx);
        click(window, "confirm-tunnel-rule-change", cx);
    })
    .expect("delete from a different panel");
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(fixture.store.path()).expect("protected disk"),
        before
    );
    assert!(
        !fixture
            .workspace
            .read_with(cx, |workspace, _| workspace.saving)
    );
    assert!(other_panel.read_with(cx, |panel, _| panel.has_active_rule(id)));
    cx.update_window(other_window, |_, _, cx| {
        other_panel.update(cx, |panel, cx| panel.suspend(cx))
    })
    .expect("retire fixture workers");
    cx.wait_for(other_window, Duration::from_secs(5), |_, cx| {
        !other_panel.read(cx).has_active_rule(id)
    })
    .await;
}

#[gpui_kit::test]
async fn crash_candidate_is_read_only_until_explicit_review_and_confirm(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let window = fixture.window;
    let candidate_id = seed_runtime_candidate(&fixture, cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let inspect = format!("inspect-tunnel-recovery-{candidate_id}");
    let review = format!("review-recovery-start-{candidate_id}");

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let section = window.find("tunnel-recovery-section");
        assert_eq!(
            section.role(),
            Some(gpui_kit::accesskit::Role::Group),
            "recovery section is announced as a group"
        );
        assert!(
            section
                .label()
                .unwrap_or_else(|| panic!("recovery section AX label"))
                .contains("隧道恢复候选")
        );
        let card = window.find(format!("tunnel-recovery-candidate-{candidate_id}"));
        assert_eq!(
            card.role(),
            Some(gpui_kit::accesskit::Role::Group),
            "recovery card is announced as a group"
        );
        assert!(
            card.label()
                .unwrap_or_else(|| panic!("recovery card AX label"))
                .contains(&fixture.ssh.state.tunnel_rules[0].name)
        );
        let state = window.find(format!("tunnel-recovery-candidate-state-{candidate_id}"));
        assert_eq!(
            state.role(),
            Some(gpui_kit::accesskit::Role::Status),
            "recovery state is announced as status"
        );
        assert!(
            state
                .label()
                .unwrap_or_else(|| panic!("recovery state AX label"))
                .contains("可能仍在运行")
        );
        let actions = window.find(format!("tunnel-recovery-candidate-actions-{candidate_id}"));
        assert!(
            actions.bounds().top() >= state.bounds().bottom(),
            "actions are separated from the state callout"
        );
        assert!(window.try_find(inspect.clone()).is_some());
        assert!(panel.read(cx).runtime_row_count() == 0);
        click(window, inspect, cx);
        assert!(
            window
                .try_find(format!(
                    "tunnel-recovery-candidate-inspected-{candidate_id}"
                ))
                .is_some()
        );
        click(window, review, cx);
        assert!(window.try_find("confirm-tunnel-recovery-start").is_some());
        assert!(panel.read(cx).runtime_row_count() == 0);
    })
    .expect("candidate remains read-only before confirmation");

    cx.update_window(window, |_, window, cx| {
        click(window, "confirm-tunnel-recovery-start", cx);
        assert_eq!(panel.read(cx).runtime_row_count(), 1);
    })
    .expect("explicit recovery confirmation creates a fresh worker");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).any_runtime_listener()
    })
    .await;

    let persisted = fixture.store.load().expect("read recovery state");
    assert!(
        persisted
            .tunnel_runtime_snapshots
            .iter()
            .any(|snapshot| snapshot.id == candidate_id),
        "old candidate stays until cleanup or explicit discard"
    );
    panel.update(cx, |panel, cx| panel.suspend(cx));
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).all_runtime_rows_finished()
    })
    .await;
}

#[gpui_kit::test]
async fn recovery_discard_is_explicit_and_persisted_through_workspace_worker(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let window = fixture.window;
    let candidate_id = seed_runtime_candidate(&fixture, cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let discard = format!("discard-tunnel-recovery-{candidate_id}");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        click(window, discard, cx);
    })
    .expect("explicit discard");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
            && fixture
                .store
                .load()
                .expect("load after discard")
                .tunnel_runtime_snapshots
                .iter()
                .all(|snapshot| snapshot.id != candidate_id)
    })
    .await;
    assert!(panel.read_with(cx, |panel, _| panel.recovery_candidates().is_empty()));
}

#[gpui_kit::test]
async fn changed_rule_keeps_candidate_and_blocks_review_start(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let window = fixture.window;
    let candidate_id = seed_runtime_candidate(&fixture, cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let mut changed = fixture.store.load().expect("load current state");
    changed.tunnel_rules[0].name = "Changed after crash".into();
    let changed = fixture.store.save(&changed).expect("persist changed rule");
    adopt_fixture_state(&fixture, changed, cx);
    cx.update_window(window, |_, _, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.refresh_tunnel_panels(cx));
    })
    .expect("refresh changed rule");
    let review = format!("review-recovery-start-{candidate_id}");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        click(window, review, cx);
        assert!(window.try_find("confirm-tunnel-recovery-start").is_none());
        assert!(panel.read(cx).runtime_row_count() == 0);
    })
    .expect("changed rule cannot be reviewed for recovery");
    assert!(
        fixture
            .store
            .load()
            .expect("candidate retained")
            .tunnel_runtime_snapshots
            .iter()
            .any(|snapshot| snapshot.id == candidate_id)
    );
}

#[gpui_kit::test]
async fn mismatched_current_connection_keeps_candidate_visible_and_blocks_review(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let window = fixture.window;
    let candidate_id = seed_runtime_candidate(&fixture, cx);
    let panel = current_tunnel_panel(&fixture, cx);
    let state = fixture.store.load().expect("load candidate state");
    let other_connection_id = uuid::Uuid::new_v4();

    panel.update(cx, |panel, cx| {
        panel.refresh_rule_state(Some(state), Some(other_connection_id), cx);
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(format!("tunnel-recovery-candidate-status-{candidate_id}"))
                .visible()
        );
        assert_eq!(
            window
                .find(format!("review-recovery-start-{candidate_id}"))
                .disabled(),
            Some(true)
        );
        assert!(
            window
                .try_find(format!("discard-tunnel-recovery-{candidate_id}"))
                .is_some()
        );
        click(window, format!("review-recovery-start-{candidate_id}"), cx);
        assert!(window.try_find("confirm-tunnel-recovery-start").is_none());
        assert_eq!(panel.read(cx).runtime_row_count(), 0);
    })
    .expect("mismatched connection keeps candidate visible but non-startable");
}

#[gpui_kit::test]
async fn confirmed_normal_stop_removes_the_persisted_runtime_candidate(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    let window = fixture.window;
    let panel = current_tunnel_panel(&fixture, cx);
    let rule_id = fixture.ssh.state.tunnel_rules[0].id;
    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("explicit named start");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).any_runtime_listener()
    })
    .await;
    cx.wait_for(window, Duration::from_secs(5), |_, _cx| {
        fixture
            .store
            .load()
            .expect("load persisted runtime")
            .tunnel_runtime_snapshots
            .iter()
            .any(|snapshot| snapshot.rule.id == rule_id)
    })
    .await;

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "tunnel-management-scroll",
            gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-500.))),
            cx,
        );
        window.render_frame(cx);
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("explicit named stop");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).all_runtime_rows_finished()
            && !fixture.workspace.read(cx).saving
            && fixture
                .store
                .load()
                .expect("load after confirmed cleanup")
                .tunnel_runtime_snapshots
                .iter()
                .all(|snapshot| snapshot.rule.id != rule_id)
    })
    .await;
}

#[gpui_kit::test]
async fn unconfirmed_cleanup_retains_candidate_and_blocks_recovery(cx: &mut TestAppContext) {
    let (mut ssh, _cleanup) = SshFixture::with_rejected_cleanup();
    let rule = &mut ssh.state.tunnel_rules[0];
    rule.kind = TunnelRuleKind::Remote;
    rule.target_host = Some("127.0.0.1".into());
    rule.target_port = Some(22);
    let rule_id = rule.id;
    let fixture = mount_with_ssh(ssh, cx);
    let window = fixture.window;
    let panel = current_tunnel_panel(&fixture, cx);

    cx.update_window(window, |_, window, cx| {
        click(window, format!("named-tunnel-rule-{rule_id}"), cx);
        click(window, "review-start-named-tunnel", cx);
    })
    .expect("explicit named remote start");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).any_runtime_listener()
    })
    .await;
    cx.wait_for(window, Duration::from_secs(5), |_, _cx| {
        fixture
            .store
            .load()
            .expect("load persisted runtime")
            .tunnel_runtime_snapshots
            .iter()
            .any(|snapshot| snapshot.rule.id == rule_id)
    })
    .await;

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "tunnel-management-scroll",
            gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-500.))),
            cx,
        );
        window.render_frame(cx);
        click(window, ("stop-tunnel", 0_usize), cx);
        click(window, "confirm-tunnel-rule-stop", cx);
    })
    .expect("request cleanup that the fixture rejects");
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        let persisted = fixture.store.load().expect("load unconfirmed runtime");
        rule_cleanup_unconfirmed(panel.read(cx), rule_id)
            && persisted.tunnel_runtime_snapshots.iter().any(|snapshot| {
                snapshot.rule.id == rule_id
                    && snapshot.status == TunnelRuntimeStatus::CleanupUnconfirmed
            })
    })
    .await;

    let snapshot_id = fixture
        .store
        .load()
        .expect("load retained candidate")
        .tunnel_runtime_snapshots
        .into_iter()
        .find(|snapshot| snapshot.rule.id == rule_id)
        .expect("retained cleanup candidate")
        .id;
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(format!("tunnel-recovery-candidate-status-{snapshot_id}"))
                .visible()
        );
        assert_eq!(
            window
                .find(format!("review-recovery-start-{snapshot_id}"))
                .disabled(),
            Some(true)
        );
        assert!(
            window
                .try_find(format!("discard-tunnel-recovery-{snapshot_id}"))
                .is_some()
        );
        assert_eq!(panel.read(cx).runtime_row_count(), 1);
    })
    .expect("unconfirmed cleanup remains a visible, non-recoverable candidate");
}
