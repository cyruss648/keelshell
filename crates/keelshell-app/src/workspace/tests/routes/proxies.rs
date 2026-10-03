//! Real UI authentication through an owned SOCKS5 → SSH → HTTP CONNECT → SSH chain.
use super::*;
use keelshell_core::{AuthMethod, ConnectionProxy, ProxyAuthentication, ProxyKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Default)]
struct ProxyState {
    requests: std::sync::Mutex<Vec<String>>,
    accepted: AtomicUsize,
    alive: AtomicUsize,
}
struct ProxyAlive(Arc<ProxyState>);
impl Drop for ProxyAlive {
    fn drop(&mut self) {
        self.0.alive.fetch_sub(1, Ordering::AcqRel);
    }
}

fn proxy_peer(
    runtime: &tokio::runtime::Runtime,
    kind: ProxyKind,
    destination: SocketAddr,
    state: Arc<ProxyState>,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = runtime
        .block_on(TcpListener::bind("127.0.0.1:0"))
        .checked("bind isolated upstream proxy");
    let address = listener.local_addr().checked("proxy address");
    let task=runtime.spawn(async move {
        let mut peers=JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept()=> {
                    let Ok((stream,_))=accepted else {break};
                    let state=state.clone();
                    state.alive.fetch_add(1,Ordering::AcqRel);
                    peers.spawn(async move {
                        let _alive=ProxyAlive(state.clone());
                        let _=tokio::time::timeout(Duration::from_secs(30),proxy_request(stream,kind,destination,state)).await;
                    });
                }
                _=peers.join_next(), if !peers.is_empty()=>{}
            }
        }
    });
    (address, task)
}

async fn proxy_request(
    mut stream: TcpStream,
    kind: ProxyKind,
    destination: SocketAddr,
    state: Arc<ProxyState>,
) -> std::io::Result<()> {
    let requested = match kind {
        ProxyKind::Socks5 => {
            let mut greeting = [0; 2];
            stream.read_exact(&mut greeting).await?;
            let mut methods = vec![0; greeting[1] as usize];
            stream.read_exact(&mut methods).await?;
            if greeting[0] != 5 || !methods.contains(&2) {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            stream.write_all(&[5, 2]).await?;
            let mut header = [0; 2];
            stream.read_exact(&mut header).await?;
            let mut username = vec![0; header[1] as usize];
            stream.read_exact(&mut username).await?;
            let length = stream.read_u8().await?;
            let mut password = vec![0; length as usize];
            stream.read_exact(&mut password).await?;
            if header[0] != 1
                || username != b"proxy-socks-user"
                || password != b"proxy-socks-password"
            {
                stream.write_all(&[1, 1]).await?;
                return Ok(());
            }
            stream.write_all(&[1, 0]).await?;
            let mut request = [0; 4];
            stream.read_exact(&mut request).await?;
            if request != [5, 1, 0, 3] {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let length = stream.read_u8().await?;
            let mut host = vec![0; length as usize];
            stream.read_exact(&mut host).await?;
            let port = stream.read_u16().await?;
            stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await?;
            format!("{}:{port}", String::from_utf8_lossy(&host))
        }
        ProxyKind::HttpConnect => {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                request.push(stream.read_u8().await?);
            }
            let text = String::from_utf8_lossy(&request);
            if !text.lines().any(|line| {
                line.trim()
                    == "Proxy-Authorization: Basic cHJveHktaHR0cC11c2VyOnByb3h5LWh0dHAtcGFzc3dvcmQ="
            }) {
                stream
                    .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
                    .await?;
                return Ok(());
            }
            let requested = text
                .lines()
                .next()
                .and_then(|line| line.strip_prefix("CONNECT "))
                .and_then(|line| line.strip_suffix(" HTTP/1.1"))
                .ok_or(std::io::ErrorKind::InvalidData)?
                .to_owned();
            stream
                .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                .await?;
            requested
        }
    };
    if let Ok(mut requests) = state.requests.lock() {
        requests.push(requested)
    }
    state.accepted.fetch_add(1, Ordering::AcqRel);
    let mut target = TcpStream::connect(destination).await?;
    tokio::io::copy_bidirectional(&mut stream, &mut target).await?;
    Ok(())
}

struct ProxyChain {
    route: RouteFixture,
    socks: Arc<ProxyState>,
    http: Arc<ProxyState>,
}

fn mount_proxy_chain(cx: &mut TestAppContext, pinned: bool) -> (Fixture, ProxyChain) {
    mount_proxy_chain_with_gate(cx, pinned, false)
}

fn mount_proxy_chain_with_gate(
    cx: &mut TestAppContext,
    pinned: bool,
    delayed: bool,
) -> (Fixture, ProxyChain) {
    let fixture = mount(cx, Vec::new());
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let target_state = Arc::new(PeerState {
        gate: delayed.then(|| Arc::new(tokio::sync::Notify::new())),
        ..Default::default()
    });
    let gateway_state = Arc::new(PeerState::default());
    let socks = Arc::new(ProxyState::default());
    let http = Arc::new(ProxyState::default());
    let (target_address, target_pin, target_task) =
        listen(&runtime, 0x39, target_state.clone(), None);
    let (http_address, http_task) = proxy_peer(
        &runtime,
        ProxyKind::HttpConnect,
        target_address,
        http.clone(),
    );
    let (gateway_address, gateway_pin, gateway_task) =
        listen(&runtime, 0x41, gateway_state.clone(), Some(http_address));
    let (socks_address, socks_task) =
        proxy_peer(&runtime, ProxyKind::Socks5, gateway_address, socks.clone());
    let mut gateway = Connection::new("SOCKS5 后的跳板", "gateway.fixture.invalid", "fixture");
    gateway.auth = AuthMethod::Password;
    gateway.proxy = Some(ConnectionProxy {
        kind: ProxyKind::Socks5,
        host: "127.0.0.1".into(),
        port: socks_address.port(),
        auth: ProxyAuthentication::UsernamePassword {
            username: "proxy-socks-user".into(),
        },
    });
    let mut target = Connection::new("HTTP 后的目标", "destination.fixture.invalid", "fixture");
    target.auth = AuthMethod::Password;
    target.jump_host = Some(gateway.id);
    // Only the preceding SSH peer can map this name to the HTTP loopback port.
    target.proxy = Some(ConnectionProxy {
        kind: ProxyKind::HttpConnect,
        host: "target.fixture.invalid".into(),
        port: 22,
        auth: ProxyAuthentication::UsernamePassword {
            username: "proxy-http-user".into(),
        },
    });
    let route = RouteFixture {
        gateway,
        target,
        pins: [gateway_pin, target_pin],
        gateway_state,
        target_state,
        tasks: vec![target_task, gateway_task, socks_task, http_task],
    };
    install_route(&fixture, &route, pinned, cx);
    (fixture, ProxyChain { route, socks, http })
}

fn submit_proxy_secret(
    fixture: &Fixture,
    connection: &Connection,
    password: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view
                .login
                .as_ref()
                .checked_option("proxy and SSH authentication prompt");
            assert_eq!(login.connection.id, connection.id);
            login.secret.update(cx, |input, cx| {
                input.set_value("route-fixture-password", window, cx)
            });
            login
                .proxy_secret
                .update(cx, |input, cx| input.set_value(password, window, cx));
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("submit two isolated credentials");
}

async fn wait_for_target(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .tabs
            .first()
            .is_some_and(|tab| tab.read(cx).visible_text().contains("TARGET READY"))
    })
    .await;
}

#[gpui_kit::test]
async fn proxy_chain_authenticates_each_hop_and_only_the_target_receives_terminal_input(
    cx: &mut TestAppContext,
) {
    let (fixture, chain) = mount_proxy_chain(cx, true);
    start_route(&fixture, &chain.route, cx);
    submit_proxy_secret(&fixture, &chain.route.gateway, "proxy-socks-password", cx);
    login_for(&fixture, chain.route.target.id, cx).await;
    submit_proxy_secret(&fixture, &chain.route.target, "proxy-http-password", cx);
    wait_for_target(&fixture, cx).await;
    assert_eq!(
        *chain.socks.requests.lock().checked("SOCKS requests"),
        vec!["gateway.fixture.invalid:22"]
    );
    assert_eq!(
        *chain.http.requests.lock().checked("HTTP requests"),
        vec!["destination.fixture.invalid:22"]
    );
    assert_eq!(chain.route.gateway_state.opened.load(Ordering::Acquire), 1);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.login.is_none());
            assert!(view.connect_route.is_none());
            assert_eq!(view.tabs.len(), 1);
            let target = view.tabs[0].entity_id();
            view.set_reviewed_command("echo proxy-chain-target".into(), Some(target), window, cx);
            view.run_command(window, cx);
        })
    })
    .checked("run reviewed command only after chain completion");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        chain
            .route
            .target_state
            .received
            .lock()
            .is_ok_and(|bytes| bytes.as_slice() == b"echo proxy-chain-target\r")
    })
    .await;
    assert!(
        chain
            .route
            .gateway_state
            .received
            .lock()
            .checked("gateway terminal input")
            .is_empty()
    );
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = false;
            view.close_tab(&crate::workspace::CloseTab, window, cx)
        })
    })
    .checked("close owned proxy chain");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        chain.socks.alive.load(Ordering::Acquire) == 0
            && chain.http.alive.load(Ordering::Acquire) == 0
            && chain.route.gateway_state.alive.load(Ordering::Acquire) == 0
            && chain.route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
}

#[gpui_kit::test]
async fn proxy_password_never_enters_saved_ssh_credential_and_unlock_requires_new_proxy_input(
    cx: &mut TestAppContext,
) {
    let (fixture, chain) = mount_proxy_chain(cx, true);
    start_route(&fixture, &chain.route, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.login
                .as_ref()
                .checked_option("login")
                .proxy_secret
                .update(cx, |input, cx| {
                    input.set_value("must-clear-on-save", window, cx)
                })
        });
        window.render_frame(cx);
        window.click("save-credential-mode", cx);
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("save mode");
            assert!(login.proxy_secret.read(cx).value().is_empty());
            for (field, value) in [
                (&login.secret, "route-fixture-password"),
                (&login.master, "proxy-test-vault-master"),
                (&login.confirmation, "proxy-test-vault-master"),
            ] {
                field.update(cx, |input, cx| input.set_value(value, window, cx));
            }
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("save SSH credential without proxy admission");
    wait_for_vault(&fixture, cx).await;
    assert_eq!(chain.socks.accepted.load(Ordering::Acquire), 0);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("locked after save");
            assert!(matches!(
                login.mode,
                crate::workspace::vault::LoginMode::Unlock
            ));
            assert!(login.proxy_secret.read(cx).value().is_empty());
            login.master.update(cx, |input, cx| {
                input.set_value("proxy-test-vault-master", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
        assert!(
            !fixture.workspace.read(cx).connecting,
            "SOCKS empty password must fail before vault/network admission"
        );
        fixture.workspace.update(cx, |view, cx| {
            view.login
                .as_ref()
                .checked_option("retained unlock prompt")
                .proxy_secret
                .update(cx, |input, cx| {
                    input.set_value("proxy-socks-password", window, cx)
                })
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("supply proxy password separately for unlock");
    login_for(&fixture, chain.route.target.id, cx).await;
    let state = fixture.store.load().checked("persisted proxy metadata");
    let reference = state.connections[0]
        .credential_ref
        .checked_option("SSH credential reference");
    let vault = keelshell_core::VaultStore::new(fixture.store.path().with_file_name("vault.json"))
        .load("proxy-test-vault-master")
        .checked("authenticate fixture vault");
    let payload = vault
        .get(
            reference,
            chain.route.gateway.id,
            keelshell_core::CredentialKind::Password,
        )
        .checked("inspect synthetic SSH payload");
    assert!(payload.contains("route-fixture-password"));
    for forbidden in [
        "proxy-socks-password",
        "must-clear-on-save",
        "proxy-http-password",
    ] {
        assert!(!payload.contains(forbidden));
        assert!(
            !std::fs::read_to_string(fixture.store.path())
                .checked("profile metadata")
                .contains(forbidden)
        );
    }
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.cancel_connect_route(window, cx))
    })
    .checked("cancel remaining target prompt");
}

#[gpui_kit::test]
async fn proxy_rejection_never_falls_back_and_a_new_attempt_cannot_reuse_old_secrets(
    cx: &mut TestAppContext,
) {
    let (fixture, chain) = mount_proxy_chain(cx, true);
    start_route(&fixture, &chain.route, cx);
    submit_proxy_secret(&fixture, &chain.route.gateway, "wrong-proxy-password", cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).connect_route.is_none()
    })
    .await;
    assert_eq!(
        chain
            .route
            .gateway_state
            .authenticated
            .load(Ordering::Acquire),
        0
    );
    assert_eq!(chain.socks.accepted.load(Ordering::Acquire), 0);
    start_route(&fixture, &chain.route, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("fresh credentials");
            assert!(login.secret.read(cx).value().is_empty());
            assert!(login.proxy_secret.read(cx).value().is_empty());
            view.cancel_connect_route(window, cx);
        })
    })
    .checked("new route has no inherited secrets");
}

#[gpui_kit::test]
async fn trusting_a_proxied_hop_requires_fresh_passwords_before_reconnecting(
    cx: &mut TestAppContext,
) {
    let (fixture, chain) = mount_proxy_chain(cx, false);
    start_route(&fixture, &chain.route, cx);
    submit_proxy_secret(&fixture, &chain.route.gateway, "proxy-socks-password", cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture.workspace.read(cx).host_approval.is_some()
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.login.is_none());
            view.accept_host_key(window, cx);
        })
    })
    .checked("trust exact proxied host scope");
    login_for(&fixture, chain.route.gateway.id, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let login = view
                .login
                .as_ref()
                .checked_option("fresh prompt after trust");
            assert!(login.secret.read(cx).value().is_empty());
            assert!(login.proxy_secret.read(cx).value().is_empty());
            assert!(
                view.state
                    .host_key(&chain.route.gateway.host, chain.route.gateway.port)
                    .is_none()
            );
            view.cancel_connect_route(window, cx);
        })
    })
    .checked("strict proxy route identity and discarded credentials");
}

fn agent_proxy_profile() -> Connection {
    let mut profile = Connection::new("Agent proxy", "destination.fixture.invalid", "fixture");
    profile.auth = AuthMethod::Agent;
    profile.proxy = Some(ConnectionProxy {
        kind: ProxyKind::Socks5,
        host: "127.0.0.1".into(),
        port: 9,
        auth: ProxyAuthentication::UsernamePassword {
            username: "proxy-user".into(),
        },
    });
    profile
}

#[gpui_kit::test]
async fn agent_proxy_prompt_keeps_input_private_and_clears_it_on_cancel_or_edit(
    cx: &mut TestAppContext,
) {
    let profile = agent_proxy_profile();
    let fixture = mount(cx, vec![profile.clone()]);
    let panes = attach_remote_panes(&fixture, cx);
    let previous = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.request_connect(profile.clone(), window, cx)
            });
            window.render_frame(cx);
            window.input("代理密码", cx);
            let previous = fixture
                .workspace
                .read(cx)
                .login
                .as_ref()
                .checked_option("Agent must still prompt for proxy")
                .proxy_secret
                .clone();
            assert!(previous.read(cx).value().contains("代理密码"));
            fixture.workspace.update(cx, |view, cx| {
                view.login_mode(crate::workspace::vault::LoginMode::Save, window, cx)
            });
            assert!(matches!(
                fixture
                    .workspace
                    .read(cx)
                    .login
                    .as_ref()
                    .checked_option("agent mode remains once")
                    .mode,
                crate::workspace::vault::LoginMode::Once
            ));
            previous.update(cx, |input, cx| {
                input.set_value("密".repeat(100), window, cx)
            });
            window.render_frame(cx);
            window.click("submit-login", cx);
            assert!(
                fixture.workspace.read(cx).login.is_some(),
                "300 UTF-8 bytes must be refused before Agent/network activity"
            );
            assert!(!fixture.workspace.read(cx).connecting);
            fixture
                .workspace
                .update(cx, |view, cx| view.switch_language(window, cx));
            assert_eq!(previous.read(cx).value().as_str(), "密".repeat(100));
            previous
        })
        .checked("type into isolated Agent proxy field and reject byte overflow");
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-login", cx);
        assert!(previous.read(cx).value().is_empty());
        fixture.workspace.update(cx, |view, cx| {
            view.request_connect(profile.clone(), window, cx)
        });
        let prompt = fixture
            .workspace
            .read(cx)
            .login
            .as_ref()
            .checked_option("new prompt")
            .proxy_secret
            .clone();
        prompt.update(cx, |input, cx| {
            input.set_value("clear-on-profile-edit", window, cx)
        });
        fixture.workspace.update(cx, |view, cx| {
            view.edit_connection(profile.clone(), window, cx)
        });
        assert!(prompt.read(cx).value().is_empty());
        assert!(fixture.workspace.read(cx).connect_route.is_none());
        assert!(fixture.workspace.read(cx).form.is_some());
    })
    .checked("cancel and profile edit both erase temporary proxy inputs");
    for pane in &panes {
        assert!(writes(pane).is_empty());
    }
}

#[gpui_kit::test]
async fn saved_proxy_change_cancels_current_prompt_and_retains_the_explanation(
    cx: &mut TestAppContext,
) {
    let profile = agent_proxy_profile();
    let fixture = mount(cx, vec![profile.clone()]);
    let secret = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.request_connect(profile.clone(), window, cx);
                let secret = view
                    .login
                    .as_ref()
                    .checked_option("proxy login")
                    .proxy_secret
                    .clone();
                secret.update(cx, |field, cx| {
                    field.set_value("never-send-stale-proxy-password", window, cx)
                });
                let mut candidate = view.state.clone();
                let mut changed = profile.clone();
                changed.proxy.as_mut().checked_option("proxy").auth =
                    ProxyAuthentication::UsernamePassword {
                        username: "different-user".into(),
                    };
                candidate
                    .update_connection(changed)
                    .checked("change route authentication identity");
                view.persist(candidate, crate::workspace::AfterSave::None, window, cx);
                secret
            })
        })
        .checked("persist a real route change while credentials are pending");
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, _, cx| {
        let view = fixture.workspace.read(cx);
        assert!(view.login.is_none());
        assert!(view.connect_route.is_none());
        assert!(secret.read(cx).value().is_empty());
        assert!(view.status.render(cx).contains("已取消"));
    })
    .checked("route cancellation survives saved-state feedback");
    assert!(
        !std::fs::read_to_string(fixture.store.path())
            .checked("stored proxy profile")
            .contains("never-send-stale-proxy-password")
    );
}

#[gpui_kit::test]
async fn connection_editor_roundtrips_proxy_metadata_and_conflict_retains_its_draft(
    cx: &mut TestAppContext,
) {
    let profile = agent_proxy_profile();
    let fixture = mount(cx, vec![profile.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.edit_connection(profile.clone(), window, cx)
        });
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("save profile using the production editor");
    wait_for_vault(&fixture, cx).await;
    assert_eq!(
        fixture
            .store
            .load()
            .checked("reload saved proxy")
            .connections[0]
            .proxy,
        profile.proxy
    );
    let snapshot = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.edit_connection(view.state.connections[0].clone(), window, cx);
                view.form
                    .as_ref()
                    .checked_option("editor")
                    .proxy_editor
                    .read(cx)
                    .signature(cx)
            })
        })
        .checked("reopen persisted proxy editor");
    let mut external = fixture.store.load().checked("competing revision");
    external.settings.font_size = 19.;
    fixture
        .store
        .save(&external)
        .checked("save competing revision");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-connection", cx)
    })
    .checked("attempt stale draft save");
    wait_for_vault(&fixture, cx).await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert_eq!(
            view.form
                .as_ref()
                .checked_option("draft retained after conflict")
                .proxy_editor
                .read(cx)
                .signature(cx),
            snapshot
        )
    });
    assert_eq!(
        fixture
            .store
            .load()
            .checked("conflict did not overwrite disk")
            .settings
            .font_size,
        19.
    );
}

#[gpui_kit::test]
async fn cancelled_proxy_chain_cannot_complete_over_a_new_attempt(cx: &mut TestAppContext) {
    let (fixture, chain) = mount_proxy_chain_with_gate(cx, true, true);
    start_route(&fixture, &chain.route, cx);
    submit_proxy_secret(&fixture, &chain.route.gateway, "proxy-socks-password", cx);
    login_for(&fixture, chain.route.target.id, cx).await;
    submit_proxy_secret(&fixture, &chain.route.target, "proxy-http-password", cx);
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        chain
            .route
            .target_state
            .authenticated
            .load(Ordering::Acquire)
            == 1
    })
    .await;
    let next = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                view.cancel_connect_route(window, cx);
                view.request_connect(chain.route.target.clone(), window, cx);
                let login = view
                    .login
                    .as_ref()
                    .checked_option("new route first-hop prompt");
                assert_eq!(login.connection.id, chain.route.gateway.id);
                assert!(login.proxy_secret.read(cx).value().is_empty());
                login.id
            })
        })
        .checked("cancel pending proxy route and immediately start a new one");
    chain
        .route
        .target_state
        .gate
        .as_ref()
        .checked_option("delayed target")
        .notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        chain.socks.alive.load(Ordering::Acquire) == 0
            && chain.http.alive.load(Ordering::Acquire) == 0
            && chain.route.gateway_state.alive.load(Ordering::Acquire) == 0
            && chain.route.target_state.alive.load(Ordering::Acquire) == 0
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert_eq!(
                view.login
                    .as_ref()
                    .checked_option("new prompt survives late completion")
                    .id,
                next
            );
            assert!(view.tabs.is_empty());
            view.cancel_connect_route(window, cx);
        })
    })
    .checked("late result cannot create a tab or erase another prompt");
}

#[gpui_kit::test]
async fn cancelling_proxy_vault_unlock_cannot_send_the_captured_password_later(
    cx: &mut TestAppContext,
) {
    let (fixture, chain) = mount_proxy_chain(cx, true);
    start_route(&fixture, &chain.route, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-credential-mode", cx);
        fixture.workspace.update(cx, |view, cx| {
            let login = view.login.as_ref().checked_option("save SSH password");
            for (field, value) in [
                (&login.secret, "route-fixture-password"),
                (&login.master, "cancelled-proxy-vault-master"),
                (&login.confirmation, "cancelled-proxy-vault-master"),
            ] {
                field.update(cx, |input, cx| input.set_value(value, window, cx));
            }
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("save fixture SSH credential without contacting proxy");
    wait_for_vault(&fixture, cx).await;
    let cleared = cx
        .update_window(fixture.window, |_, window, cx| {
            let input = fixture.workspace.update(cx, |view, cx| {
                let login = view.login.as_ref().checked_option("unlock SSH plus proxy");
                login.master.update(cx, |input, cx| {
                    input.set_value("cancelled-proxy-vault-master", window, cx)
                });
                login.proxy_secret.update(cx, |input, cx| {
                    input.set_value("proxy-socks-password", window, cx)
                });
                login.proxy_secret.clone()
            });
            window.render_frame(cx);
            window.click("submit-login", cx);
            // Same UI turn: the background KDF cannot dispatch its completion yet.
            fixture
                .workspace
                .update(cx, |view, cx| view.cancel_connect_route(window, cx));
            input
        })
        .checked("cancel admitted unlock before foreground completion");
    wait_for_vault(&fixture, cx).await;
    assert_eq!(chain.socks.accepted.load(Ordering::Acquire), 0);
    assert_eq!(
        chain
            .route
            .gateway_state
            .authenticated
            .load(Ordering::Acquire),
        0
    );
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert!(view.login.is_none());
            assert!(view.connect_route.is_none());
            assert!(view.tabs.is_empty());
            assert!(cleared.read(cx).value().is_empty());
            view.request_connect(chain.route.target.clone(), window, cx);
            let login = view.login.as_ref().checked_option("new unlock prompt");
            assert!(login.proxy_secret.read(cx).value().is_empty());
            assert!(login.master.read(cx).value().is_empty());
            view.cancel_connect_route(window, cx);
        })
    })
    .checked("late unlock neither connects nor supplies the next attempt");
}
