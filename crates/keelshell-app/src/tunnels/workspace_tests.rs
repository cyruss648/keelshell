//! Production subscription and background StateStore round trips.
#![allow(clippy::expect_used)]

use super::{RemotePanels, ToolPanel, Workspace};
use crate::terminal::TerminalView;
use crate::tunnels::test_support::{SshFixture, click};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use keelshell_core::{StateStore, TunnelRuleKind};
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
