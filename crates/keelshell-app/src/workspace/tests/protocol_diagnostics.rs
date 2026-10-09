//! The actual workspace at its minimum supported size. Transport metadata is
//! controlled; these checks prove layout/authority, not native desktop acceptance.
use super::*;
use keelshell_core::Theme;
use keelshell_session::SshSession;

pub(crate) fn exercise_layout(
    monitor: Entity<crate::monitor::MonitorPanel>,
    cx: &mut TestAppContext,
) {
    let fixture = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    fixture.workspace.update(cx, |view, cx| {
        view.show_connections = false;
        view.show_assistant = false;
        view.panels
            .entry(panes[0].terminal.entity_id())
            .or_default()
            .monitor = Some(monitor);
        cx.notify();
    });
    for language in [Language::ZhCn, Language::En] {
        cx.update(|cx| i18n::set_language(language, cx));
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            cx.update_window(fixture.window, |_, window, cx| {
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                for id in [
                    "protocol-preview",
                    "protocol-cert-fingerprint",
                    "protocol-timing-total",
                ] {
                    for _ in 0..120 {
                        let viewport = window.find("monitor-scroll").bounds();
                        let target = window.find(id).bounds();
                        if target.origin.y >= viewport.origin.y
                            && target.bottom() <= viewport.bottom()
                        {
                            break;
                        }
                        window.scroll(
                            "monitor-scroll",
                            gpui_kit::ScrollDelta::Lines(point(
                                0.,
                                if target.origin.y < viewport.origin.y {
                                    1.
                                } else {
                                    -1.
                                },
                            )),
                            cx,
                        );
                        window.render_frame(cx);
                    }
                    let viewport = window.find("monitor-scroll").bounds();
                    let row = window.find(id);
                    assert!(
                        row.visible()
                            && row.bounds().origin.x >= viewport.origin.x
                            && row.bounds().right() <= viewport.right()
                            && row.bounds().origin.y >= viewport.origin.y
                            && row.bounds().bottom() <= viewport.bottom(),
                        "{language:?}/{theme:?}: {id} {:?} in {viewport:?}",
                        row.bounds()
                    );
                }
                assert_eq!(
                    window.find("protocol-preview").label(),
                    Some(if language == Language::ZhCn {
                        "预览诊断"
                    } else {
                        "Preview diagnostic"
                    })
                );
            })
            .checked("six language/theme combinations in 900x580 production workspace");
        }
    }
}

pub(crate) fn exercise_authority(
    monitor: Entity<crate::monitor::MonitorPanel>,
    session: SshSession,
    replacement: SshSession,
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("fixture", "original.test", "fixture");
    let profile_id = profile.id;
    let fixture = mount_sized(cx, vec![profile], 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let tab = panes[0].terminal.entity_id();
    fixture.workspace.update(cx, |view, cx| {
        view.remote_sessions.insert(tab, session.clone());
        let route = view
            .state
            .connection_route(profile_id)
            .checked("captured saved route");
        view.bind_remote_tab(tab, route);
        view.panels.entry(tab).or_default().monitor = Some(monitor.clone());
        let workspace = cx.weak_entity();
        monitor.update(cx, |panel, cx| {
            panel.bind_protocol_authority(workspace, tab, cx)
        });
        assert!(view.protocol_session_current(tab, &session));
        // Replacing the session map with a missing connection fails closed.
        view.remote_sessions.remove(&tab);
        assert!(!view.protocol_session_current(tab, &session));
        view.remote_sessions.insert(tab, session.clone());
        assert!(view.protocol_session_current(tab, &session));
        view.remote_sessions.insert(tab, replacement);
        assert!(!view.protocol_session_current(tab, &session));
        view.revoke_stale_protocol_diagnostics(cx);
        assert!(!monitor.read(cx).protocol_connection_matches(&session, cx));
        view.remote_sessions.insert(tab, session.clone());
        assert!(view.protocol_session_current(tab, &session));
        // The actual saved-route binding changes even while old SSH remains open.
        view.state.connections[0].host = "changed.test".into();
        assert!(!view.protocol_session_current(tab, &session));
        view.revoke_stale_protocol_diagnostics(cx);
        cx.notify();
    });
}

pub(crate) fn install_reconnected_monitor_with_silent_route_change(
    session: SshSession,
    port: u16,
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, Entity<crate::monitor::MonitorPanel>) {
    let mut profile = Connection::new("owned reconnect", "127.0.0.1", "fixture");
    profile.port = port;
    let id = profile.id;
    let fixture = mount_sized(cx, vec![profile], 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let old = panes[0].terminal.entity_id();
    let monitor = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                let route = view
                    .state
                    .connection_route(id)
                    .checked("owned reconnect route");
                view.bind_remote_tab(old, route.clone());
                view.remote_sessions.insert(old, session.clone());
                let ticket = super::super::reconnect::budget_tests::owned_fixture_ticket(view, old);
                assert!(view.finish_reconnect(ticket, session, route, window, cx));
                let current = view.tabs[0].entity_id();
                assert_ne!(current, old);
                let monitor = view
                    .panels
                    .get(&current)
                    .and_then(|p| p.monitor.clone())
                    .unwrap_or_else(|| panic!("production reconnect must install monitor"));
                // No save callback or periodic poll intervenes: preview itself must
                // check the captured route in the newly installed production panel.
                view.state.connections[0].host = "changed-owned.invalid".into();
                monitor
            })
        })
        .checked("actual production finish_reconnect with captured owned SSH");
    (fixture.window, monitor)
}

/// Install a fresh production reconnect result while retaining its workspace so
/// a held reply can be checked against a later route, pin or tab mutation.
pub(crate) fn independent_reconnect_pending_reply_fixture(
    session: SshSession,
    port: u16,
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<crate::monitor::MonitorPanel>,
    Entity<Workspace>,
) {
    let mut profile = Connection::new("owned independent reconnect", "127.0.0.1", "fixture");
    profile.port = port;
    let id = profile.id;
    let fixture = mount_sized(cx, vec![profile], 900., 580.);
    let panes = attach_remote_panes(&fixture, cx);
    let old = panes[0].terminal.entity_id();
    let monitor = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                let route = view
                    .state
                    .connection_route(id)
                    .checked("independent reconnect route");
                view.bind_remote_tab(old, route.clone());
                view.remote_sessions.insert(old, session.clone());
                let ticket = super::super::reconnect::budget_tests::owned_fixture_ticket(view, old);
                assert!(view.finish_reconnect(ticket, session, route, window, cx));
                let current = view.tabs[0].entity_id();
                assert_ne!(current, old);
                // Isolate adoption from automatic retirement callbacks in this
                // test; the real finish_reconnect binding remains unchanged.
                view._reconnect_poll = gpui_kit::Task::ready(());
                view.terminal_observers.remove(&current);
                view.panels
                    .get(&current)
                    .and_then(|p| p.monitor.clone())
                    .unwrap_or_else(|| {
                        panic!("production reconnect must install independent monitor")
                    })
            })
        })
        .checked("independent production reconnect with live captured session");
    (fixture.window, monitor, fixture.workspace)
}

/// Mutate authority from its owning test module without widening production
/// visibility. Each mutation intentionally omits the later retirement poll.
pub(crate) fn independent_mutate_pending_authority(
    view: &mut Workspace,
    mutation: &str,
    port: u16,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<Workspace>,
) {
    match mutation {
        "route" => view.state.connections[0].host = "changed-owned.invalid".into(),
        "trust" => {
            use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
            let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x75; 32]));
            let pin = key.public_key().fingerprint(HashAlg::Sha256).to_string();
            view.state
                .trust_host_key("127.0.0.1", port, &pin)
                .checked("owned changed trust pin");
        }
        "closed_tab" => view.close_tab(&super::super::CloseTab, window, cx),
        _ => unreachable!("finite independent authority mutation matrix"),
    }
}
