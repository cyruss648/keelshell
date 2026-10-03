//! Real SSH peers behind the production workspace's per-hop authentication UI.

use super::*;
use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
use russh::{Channel, ChannelId, ChannelMsg, server};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinSet,
};

#[derive(Default)]
struct PeerState {
    authenticated: AtomicUsize,
    opened: AtomicUsize,
    alive: AtomicUsize,
    gate: Option<Arc<tokio::sync::Notify>>,
    received: std::sync::Mutex<Vec<u8>>,
}

struct Alive(Arc<PeerState>);
impl Drop for Alive {
    fn drop(&mut self) {
        self.0.alive.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Peer {
    state: Arc<PeerState>,
    target: Option<SocketAddr>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.state.authenticated.fetch_add(1, Ordering::AcqRel);
        if let Some(gate) = &self.state.gate {
            gate.notified().await;
        }
        Ok(
            if user == "fixture" && password == "route-fixture-password" {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let Some(target) = self
            .target
            .filter(|_| host == "target.fixture.invalid" && port == 22)
        else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        let mut socket = TcpStream::connect(target).await?;
        self.state.opened.fetch_add(1, Ordering::AcqRel);
        reply.accept().await;
        self.jobs.spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut socket).await;
        });
        Ok(())
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        id: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        let Some(mut channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        let state = self.state.clone();
        self.jobs.spawn(async move {
            let _ = channel.data(&b"TARGET READY\r\n"[..]).await;
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Data { data } => {
                        if let Ok(mut received) = state.received.lock() {
                            received.extend_from_slice(&data);
                        }
                        let _ = channel.data(&data[..]).await;
                    }
                    ChannelMsg::Close | ChannelMsg::Eof => break,
                    _ => {}
                }
            }
            let _ = channel.close().await;
        });
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        _: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(id)?;
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        _: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(id)?;
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }
}

struct RouteFixture {
    gateway: Connection,
    target: Connection,
    pins: [String; 2],
    gateway_state: Arc<PeerState>,
    target_state: Arc<PeerState>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for RouteFixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn listen(
    runtime: &tokio::runtime::Runtime,
    seed: u8,
    state: Arc<PeerState>,
    target: Option<SocketAddr>,
) -> (SocketAddr, String, tokio::task::JoinHandle<()>) {
    let listener = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("bind route peer");
    let address = listener.local_addr().checked("route peer address");
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]));
    let pin = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let task = runtime.spawn(async move {
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { break };
                    let state = state.clone();
                    let config = config.clone();
                    state.alive.fetch_add(1, Ordering::AcqRel);
                    let alive = Alive(state.clone());
                    sessions.spawn(async move {
                        let _alive = alive;
                        let peer = Peer { state, target, channels: HashMap::new(), jobs: JoinSet::new() };
                        if let Ok(session) = server::run_stream(config, stream, peer).await {
                            let _ = tokio::time::timeout(Duration::from_secs(30), session).await;
                        }
                    });
                }
                _ = sessions.join_next(), if !sessions.is_empty() => {}
            }
        }
    });
    (address, pin, task)
}

fn route_fixture(runtime: &tokio::runtime::Runtime, delayed: bool) -> RouteFixture {
    route_fixture_with_gates(runtime, delayed, false)
}

fn route_fixture_with_gates(
    runtime: &tokio::runtime::Runtime,
    delayed_target: bool,
    delayed_gateway: bool,
) -> RouteFixture {
    let target_state = Arc::new(PeerState {
        gate: delayed_target.then(|| Arc::new(tokio::sync::Notify::new())),
        ..Default::default()
    });
    let gateway_state = Arc::new(PeerState {
        gate: delayed_gateway.then(|| Arc::new(tokio::sync::Notify::new())),
        ..Default::default()
    });
    let (target_address, target_pin, target_task) =
        listen(runtime, 0x39, target_state.clone(), None);
    let (gateway_address, gateway_pin, gateway_task) =
        listen(runtime, 0x41, gateway_state.clone(), Some(target_address));
    let mut gateway = Connection::new("测试跳板", "127.0.0.1", "fixture");
    gateway.port = gateway_address.port();
    gateway.auth = keelshell_core::AuthMethod::Password;
    let mut target = Connection::new("私网目标", "target.fixture.invalid", "fixture");
    target.auth = keelshell_core::AuthMethod::Password;
    target.jump_host = Some(gateway.id);
    RouteFixture {
        gateway,
        target,
        pins: [gateway_pin, target_pin],
        gateway_state,
        target_state,
        tasks: vec![gateway_task, target_task],
    }
}

fn mount_route(cx: &mut TestAppContext, delayed: bool, pinned: bool) -> (Fixture, RouteFixture) {
    let fixture = mount(cx, Vec::new());
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let route = route_fixture(&runtime, delayed);
    install_route(&fixture, &route, pinned, cx);
    (fixture, route)
}

fn install_route(fixture: &Fixture, route: &RouteFixture, pinned: bool, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let mut state = view.state.clone();
            state
                .connections
                .extend([route.gateway.clone(), route.target.clone()]);
            if pinned {
                let resolved = state
                    .connection_route(route.target.id)
                    .checked("resolve fixture route");
                for (index, pin) in route.pins.iter().enumerate() {
                    state
                        .trust_host_key_for_scope(
                            &resolved.host_key_scope(index).checked_option("scope"),
                            pin,
                        )
                        .checked("trust fixture identity");
                }
            }
            view.state = fixture.store.save(&state).checked("save isolated route");
            cx.notify();
        });
    })
    .checked("initialize real route peers");
}

fn start_route(fixture: &Fixture, route: &RouteFixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.request_connect(route.target.clone(), window, cx)
        });
        window.render_frame(cx);
    })
    .checked("start route through workspace");
}

fn submit_secret(fixture: &Fixture, expected: uuid::Uuid, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view
                .login
                .as_ref()
                .checked_option("current route authentication");
            assert_eq!(login.connection.id, expected);
            login.secret.update(cx, |input, cx| {
                input.set_value("route-fixture-password", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("explicitly submit per-hop secret");
}

async fn login_for(fixture: &Fixture, expected: uuid::Uuid, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .login
            .as_ref()
            .is_some_and(|login| login.connection.id == expected)
    })
    .await;
}

#[gpui_kit::test]
async fn route_prompts_and_trust_are_per_hop_then_only_target_becomes_a_terminal(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, false);
    start_route(&fixture, &route, cx);
    for (index, profile) in [&route.gateway, &route.target].into_iter().enumerate() {
        login_for(&fixture, profile.id, cx).await;
        submit_secret(&fixture, profile.id, cx);
        cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
            fixture.workspace.read(cx).host_approval.is_some()
        })
        .await;
        fixture.workspace.read_with(cx, |view, cx| {
            let approval = view
                .host_approval
                .as_ref()
                .checked_option("per-hop host identity");
            assert_eq!(approval.connection.id, profile.id);
            assert_eq!(approval.fingerprint, route.pins[index]);
            assert!(approval.previous.is_none());
            let (hops, current) = view
                .route_progress(cx)
                .checked_option("frozen route presentation");
            assert_eq!(hops.len(), 2);
            assert_eq!(current, index);
        });
        assert_eq!(
            [&route.gateway_state, &route.target_state][index]
                .authenticated
                .load(Ordering::Acquire),
            0
        );
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("accept-host-key", cx);
        })
        .checked("approve exactly the displayed route fingerprint");
        login_for(&fixture, profile.id, cx).await;
        submit_secret(&fixture, profile.id, cx);
    }
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .tabs
            .first()
            .is_some_and(|terminal| terminal.read(cx).visible_text().contains("TARGET READY"))
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert_eq!(view.tabs.len(), 1);
            assert!(view.connect_route.is_none());
            let target = view.tabs[0].entity_id();
            view.set_reviewed_command("echo reviewed-route".into(), Some(target), window, cx);
            view.run_command(window, cx);
        });
    })
    .checked("send reviewed command only to final target");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        route
            .target_state
            .received
            .lock()
            .is_ok_and(|bytes| bytes.as_slice() == b"echo reviewed-route\r")
    })
    .await;
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.gateway_state.opened.load(Ordering::Acquire), 2);
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let saved = fixture.store.load().checked("read final route trust");
    assert_eq!(saved.known_hosts.len(), 1);
    assert_eq!(saved.route_known_hosts.len(), 1);
    assert_eq!(saved.recent_connections.len(), 1);
    assert_eq!(saved.recent_connections[0].connection_id, route.target.id);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = false;
            view.close_tab(&crate::workspace::CloseTab, window, cx);
        });
    })
    .checked("close sole target tab and release its private chain");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn cancel_during_target_authentication_discards_late_success_and_owned_prefix(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, true, true);
    start_route(&fixture, &route, cx);
    submit_secret(&fixture, route.gateway.id, cx);
    login_for(&fixture, route.target.id, cx).await;
    submit_secret(&fixture, route.target.id, cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.target_state.authenticated.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-connect-route", cx);
    })
    .checked("cancel active network operation");
    route
        .target_state
        .gate
        .as_ref()
        .checked_option("authentication gate")
        .notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        assert!(!view.connecting);
        assert!(view.connect_route.is_none());
        assert!(view.tabs.is_empty());
        assert!(view.state.recent_connections.is_empty());
    });
}

#[gpui_kit::test]
async fn cancelled_attempt_cannot_replace_a_new_route_at_the_same_step(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let old = route_fixture_with_gates(&runtime, false, true);
    let mut next = route_fixture(&runtime, false);
    next.gateway.name = "新路线跳板".into();
    next.target.name = "新路线目标".into();
    install_route(&fixture, &old, true, cx);
    install_route(&fixture, &next, true, cx);
    start_route(&fixture, &old, cx);
    submit_secret(&fixture, old.gateway.id, cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        old.gateway_state.authenticated.load(Ordering::Acquire) == 1
    })
    .await;
    let next_prompt = cx
        .update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("cancel-connect-route", cx);
            fixture.workspace.update(cx, |view, cx| {
                // Do not yield between cancellation and replacement. Both are
                // at hop zero, so only the attempt token can isolate completion.
                view.request_connect(next.target.clone(), window, cx);
                let login = view.login.as_ref().checked_option("replacement prompt");
                assert_eq!(login.connection.id, next.gateway.id);
                login.id
            })
        })
        .checked("replace a cancelled request before its completion is delivered");
    old.gateway_state
        .gate
        .as_ref()
        .checked_option("old request authentication gate")
        .notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        old.gateway_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |view, cx| {
        let login = view.login.as_ref().checked_option("replacement survives");
        assert_eq!(login.id, next_prompt);
        assert_eq!(login.connection.id, next.gateway.id);
        let (hops, current) = view.route_progress(cx).checked_option("replacement route");
        assert_eq!(current, 0);
        assert_eq!(hops[0].id, next.gateway.id);
        assert_eq!(hops[1].id, next.target.id);
        assert!(!view.connecting);
        assert!(view.host_approval.is_none());
        assert!(view.tabs.is_empty());
        assert!(view.state.recent_connections.is_empty());
    });
    assert_eq!(old.gateway_state.opened.load(Ordering::Acquire), 0);
    assert_eq!(old.target_state.authenticated.load(Ordering::Acquire), 0);
    assert_eq!(next.gateway_state.authenticated.load(Ordering::Acquire), 0);
    submit_secret(&fixture, next.gateway.id, cx);
    login_for(&fixture, next.target.id, cx).await;
    submit_secret(&fixture, next.target.id, cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        let view = fixture.workspace.read(cx);
        !view.saving
            && view
                .tabs
                .first()
                .is_some_and(|terminal| terminal.read(cx).visible_text().contains("TARGET READY"))
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.connect_route.is_none());
        assert_eq!(view.tabs.len(), 1);
        assert_eq!(view.state.recent_connections.len(), 1);
        assert_eq!(
            view.state.recent_connections[0].connection_id,
            next.target.id
        );
    });
    assert_eq!(next.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(next.target_state.authenticated.load(Ordering::Acquire), 1);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = false;
            view.close_tab(&crate::workspace::CloseTab, window, cx);
        });
    })
    .checked("close replacement target");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        next.gateway_state.alive.load(Ordering::Acquire) == 0
            && next.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn saving_then_unlocking_the_target_credential_preserves_its_authenticated_parent(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    start_route(&fixture, &route, cx);
    submit_secret(&fixture, route.gateway.id, cx);
    login_for(&fixture, route.target.id, cx).await;
    let prompt = cx
        .update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("save-credential-mode", cx);
            let prompt = fixture.workspace.update(cx, |view, cx| {
                let login = view.login.as_ref().checked_option("target save prompt");
                assert_eq!(login.connection.id, route.target.id);
                assert_eq!(login.route_identity.endpoints().len(), 2);
                for (input, value) in [
                    (&login.secret, "route-fixture-password"),
                    (&login.master, "vault-fixture-master"),
                    (&login.confirmation, "vault-fixture-master"),
                ] {
                    input.update(cx, |input, cx| input.set_value(value, window, cx));
                }
                login.id
            });
            window.render_frame(cx);
            window.click("submit-login", cx);
            prompt
        })
        .checked("save target credential through its production prompt");
    wait_for_vault(&fixture, cx).await;
    let reference = cx
        .update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            let view = fixture.workspace.read(cx);
            let login = view
                .login
                .as_ref()
                .checked_option("saved target stays locked");
            assert_eq!(login.id, prompt);
            assert!(matches!(
                login.mode,
                crate::workspace::vault::LoginMode::Unlock
            ));
            assert!(!login.busy);
            assert!(!view.connecting);
            assert!(view.tabs.is_empty());
            for input in [&login.secret, &login.master, &login.confirmation] {
                assert!(input.read(cx).value().is_empty());
            }
            assert!(login.master.read(cx).focus_handle(cx).is_focused(window));
            let reference = login
                .connection
                .credential_ref
                .checked_option("saved reference");
            let (hops, current) = view
                .route_progress(cx)
                .checked_option("saved route snapshot");
            assert_eq!(current, 1);
            assert_eq!(hops[1].credential_ref, Some(reference));
            assert!(hops[0].credential_ref.is_none());
            reference
        })
        .checked("verify token and metadata refresh before unlocking");
    let saved = fixture
        .store
        .load()
        .checked("reload linked target credential");
    assert_eq!(saved.connections[1].credential_ref, Some(reference));
    assert!(saved.connections[0].credential_ref.is_none());
    let vault_path = fixture.store.path().with_file_name("vault.json");
    for path in [fixture.store.path(), vault_path.as_path()] {
        let content = std::fs::read_to_string(path).checked("read isolated persisted document");
        assert!(!content.contains("route-fixture-password"));
        assert!(!content.contains("vault-fixture-master"));
    }
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.gateway_state.alive.load(Ordering::Acquire), 1);
    assert_eq!(route.gateway_state.opened.load(Ordering::Acquire), 0);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 0);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("target unlock prompt");
            login.master.update(cx, |input, cx| {
                input.set_value("vault-fixture-master", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("explicitly unlock saved target credential");
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .tabs
            .first()
            .is_some_and(|terminal| terminal.read(cx).visible_text().contains("TARGET READY"))
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.login.is_none());
            assert!(view.connect_route.is_none());
            assert_eq!(view.tabs.len(), 1);
            let target = view.tabs[0].entity_id();
            view.set_reviewed_command("echo unlocked-target".into(), Some(target), window, cx);
            view.run_command(window, cx);
        });
    })
    .checked("use final target through its retained parent");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        route
            .target_state
            .received
            .lock()
            .is_ok_and(|bytes| bytes.as_slice() == b"echo unlocked-target\r")
    })
    .await;
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 1);
    assert_eq!(route.gateway_state.opened.load(Ordering::Acquire), 1);
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 1);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = false;
            view.close_tab(&crate::workspace::CloseTab, window, cx);
        });
    })
    .checked("close unlocked target and its parent chain");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
            && route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn changing_an_upstream_profile_invalidates_target_authentication(cx: &mut TestAppContext) {
    let (fixture, route) = mount_route(cx, false, true);
    start_route(&fixture, &route, cx);
    submit_secret(&fixture, route.gateway.id, cx);
    login_for(&fixture, route.target.id, cx).await;
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |view, _| {
            let mut gateway = route.gateway.clone();
            gateway.username = "another-operator".into();
            view.state
                .update_connection(gateway)
                .checked("edit upstream destination");
        });
    })
    .checked("change saved upstream before submitting the target secret");
    submit_secret(&fixture, route.target.id, cx);
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.login.is_none());
        assert!(view.connect_route.is_none());
        assert!(!view.connecting);
        assert!(view.tabs.is_empty());
    });
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 0);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn stale_target_fingerprint_cannot_be_saved_after_an_upstream_edit(cx: &mut TestAppContext) {
    let (fixture, route) = mount_route(cx, false, true);
    cx.update_window(fixture.window, |_, _, cx| {
        fixture
            .workspace
            .update(cx, |view, _| view.state.route_known_hosts.clear());
    })
    .checked("require target identity approval");
    start_route(&fixture, &route, cx);
    submit_secret(&fixture, route.gateway.id, cx);
    login_for(&fixture, route.target.id, cx).await;
    submit_secret(&fixture, route.target.id, cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).host_approval.is_some()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, _| {
            let mut gateway = route.gateway.clone();
            gateway.host = "another.gateway.invalid".into();
            view.state
                .update_connection(gateway)
                .checked("invalidate observed trust scope");
        });
        window.render_frame(cx);
        window.click("accept-host-key", cx);
        let view = fixture.workspace.read(cx);
        assert!(view.host_approval.is_none());
        assert!(view.connect_route.is_none());
        assert!(view.state.route_known_hosts.is_empty());
        assert!(!view.saving);
    })
    .checked("reject trust from the old route");
    assert_eq!(route.target_state.authenticated.load(Ordering::Acquire), 0);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        route.gateway_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn cancelling_while_trust_is_saved_does_not_restart_authentication(cx: &mut TestAppContext) {
    let (fixture, route) = mount_route(cx, false, false);
    start_route(&fixture, &route, cx);
    submit_secret(&fixture, route.gateway.id, cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).host_approval.is_some()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("accept-host-key", cx);
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.saving);
            view.close_tab(&crate::workspace::CloseTab, window, cx);
            assert!(view.connect_route.is_none());
        });
    })
    .checked("dismiss after authorizing trust persistence");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.connect_route.is_none());
        assert!(view.login.is_none());
        assert!(!view.connecting);
        assert!(view.tabs.is_empty());
    });
    // Explicitly authorized trust may finish, but its stale completion token
    // cannot create a new authentication prompt or network operation.
    assert_eq!(
        fixture
            .store
            .load()
            .checked("load completed trust write")
            .known_hosts
            .len(),
        1
    );
    assert_eq!(route.gateway_state.authenticated.load(Ordering::Acquire), 0);
}

#[gpui_kit::test]
fn delayed_recent_success_does_not_survive_upstream_authentication_changes(
    cx: &mut TestAppContext,
) {
    let (fixture, route) = mount_route(cx, false, true);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.saving = true;
            view.remember_successful_connection(route.target.clone(), window, cx);
            assert_eq!(view.pending_recents.len(), 1);
            let mut gateway = route.gateway.clone();
            gateway.auth = keelshell_core::AuthMethod::Agent;
            view.state
                .update_connection(gateway)
                .checked("change upstream auth during save");
            view.saving = false;
            view.flush_recent_connections(window, cx);
            assert!(view.pending_recents.is_empty());
            assert!(view.state.recent_connections.is_empty());
            assert!(!view.saving);
        });
    })
    .checked("discard stale success after authentication changed");
}

mod proxies;

mod reconnect;
