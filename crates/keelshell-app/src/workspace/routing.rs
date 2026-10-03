//! One owned connection route, with explicit per-hop identity and authentication.

use super::*;
use keelshell_core::{ConnectionRoute, RouteIdentity};
use tokio::sync::watch;

pub(super) struct ConnectRoute {
    id: uuid::Uuid,
    snapshot: ConnectionRoute,
    current: usize,
    upstream: Option<SshSession>,
    waiting_for_save: bool,
    cancel: watch::Sender<bool>,
    pub(super) reconnect: Option<super::reconnect::Ticket>,
    pub(super) background: bool,
    pub(super) awaiting_interaction: bool,
    pub(super) deferred_approval: Option<HostApproval>,
}

impl Drop for ConnectRoute {
    fn drop(&mut self) {
        // A dismissed workspace must stop its network future, including retry
        // backoff. Each authenticated prefix belongs only to this route.
        let _ = self.cancel.send(true);
    }
}

pub(super) fn same_route(a: &ConnectionRoute, b: &ConnectionRoute) -> bool {
    a.hops().len() == b.hops().len()
        && a.hops()
            .iter()
            .zip(b.hops())
            .all(|(a, b)| vault::same_destination(a, b))
}

impl Workspace {
    pub(super) fn editing_active_route(&self, id: uuid::Uuid) -> bool {
        self.connect_route
            .as_ref()
            .is_some_and(|route| route.snapshot.hops().iter().any(|hop| hop.id == id))
    }

    pub(super) fn invalidate_changed_route(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.connect_route.is_some() && !self.route_is_current();
        if changed {
            self.route_changed(window, cx);
        }
        changed
    }

    pub(super) fn route_progress(&self, _: &App) -> Option<(Vec<Connection>, usize)> {
        let route = self.connect_route.as_ref()?;
        Some((route.snapshot.hops().to_vec(), route.current))
    }

    fn route_is_current(&self) -> bool {
        let Some(route) = &self.connect_route else {
            return false;
        };
        let Some(target) = route.snapshot.hops().last() else {
            return false;
        };
        self.state
            .connection_route(target.id)
            .is_ok_and(|current| same_route(&route.snapshot, &current))
            && route
                .reconnect
                .is_none_or(|ticket| self.reconnect_ticket_current(ticket))
    }

    pub(super) fn begin_connect_route(
        &mut self,
        connection: &Connection,
        cx: &mut Context<Self>,
    ) -> bool {
        let snapshot = match self.state.connection_route(connection.id) {
            Ok(snapshot)
                if snapshot
                    .hops()
                    .last()
                    .is_some_and(|target| vault::same_destination(target, connection)) =>
            {
                snapshot
            }
            Ok(_) => {
                self.status = Message::new(
                    "连接已变化，请重新选择。",
                    "Connection changed. Select it again.",
                );
                cx.notify();
                return false;
            }
            Err(error) => {
                self.status =
                    Message::detail("无法解析跳板路线", "Cannot resolve jump route", error);
                cx.notify();
                return false;
            }
        };
        let (cancel, _) = watch::channel(false);
        self.connect_route = Some(ConnectRoute {
            id: uuid::Uuid::new_v4(),
            snapshot,
            current: 0,
            upstream: None,
            waiting_for_save: false,
            cancel,
            reconnect: None,
            background: false,
            awaiting_interaction: false,
            deferred_approval: None,
        });
        true
    }

    pub(super) fn request_connect(
        &mut self,
        connection: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connecting || self.saving || self.connect_route.is_some() {
            self.status = Message::new(
                "请先完成或取消当前连接。",
                "Finish or cancel the current connection first.",
            );
            cx.notify();
            return;
        }
        if self.begin_connect_route(&connection, cx) {
            self.prepare_route_hop(window, cx);
        }
    }

    pub(super) fn prepare_login(
        &mut self,
        connection: Connection,
        pin: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connecting || self.saving {
            self.status = Message::new(
                "正在连接或保存，请稍候",
                "A connection or save is in progress",
            );
            cx.notify();
            return;
        }
        if self.connect_route.is_none() {
            if !self.begin_connect_route(&connection, cx) {
                return;
            }
            if self
                .connect_route
                .as_ref()
                .and_then(|route| route.snapshot.hops().first())
                .is_some_and(|first| first.id != connection.id)
            {
                self.prepare_route_hop(window, cx);
                return;
            }
        }
        let Some(route_identity) = self.login_route_identity(&connection) else {
            self.route_changed(window, cx);
            return;
        };
        if matches!(connection.auth, AuthMethod::Agent)
            && !super::vault::needs_proxy_password(&connection)
        {
            self.connect(connection, Zeroizing::new(String::new()), pin, window, cx);
        } else {
            self.clear_login(window, cx);
            let mode = if connection.credential_ref.is_some() {
                vault::LoginMode::Unlock
            } else {
                vault::LoginMode::Once
            };
            self.login = Some(LoginPrompt {
                id: uuid::Uuid::new_v4(),
                connection,
                pin,
                route_identity,
                mode,
                busy: false,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                message: None,
                master: cx.new(|cx| {
                    InputState::new(window, cx).masked(true).placeholder(t(
                        cx,
                        "凭据库主密码",
                        "Vault master password",
                    ))
                }),
                confirmation: cx.new(|cx| {
                    InputState::new(window, cx).masked(true).placeholder(t(
                        cx,
                        "再次输入主密码",
                        "Confirm master password",
                    ))
                }),
                proxy_secret: cx.new(|cx| {
                    InputState::new(window, cx).masked(true).placeholder(t(
                        cx,
                        "代理密码（仅本次使用）",
                        "Proxy password (this connection only)",
                    ))
                }),
                secret: cx.new(|cx| {
                    InputState::new(window, cx).masked(true).placeholder(t(
                        cx,
                        "密码或私钥口令（本次使用，不保存）",
                        "Password or key passphrase (not saved)",
                    ))
                }),
            });
            if let Some(login) = &self.login {
                login.focus(window, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn prepare_route_hop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            if let Some(route) = &mut self.connect_route {
                route.waiting_for_save = true;
            }
            cx.notify();
            return;
        }
        if let Some(route) = &mut self.connect_route {
            route.waiting_for_save = false;
        }
        if !self.route_is_current() {
            self.route_changed(window, cx);
            return;
        }
        let Some(connection) = self
            .connect_route
            .as_ref()
            .and_then(|route| route.snapshot.hops().get(route.current))
            .cloned()
        else {
            return;
        };
        let needs_interaction = !matches!(connection.auth, AuthMethod::Agent)
            || vault::needs_proxy_password(&connection);
        if needs_interaction
            && self
                .connect_route
                .as_ref()
                .is_some_and(|route| route.background)
        {
            if let Some(route) = &mut self.connect_route {
                route.awaiting_interaction = true;
            }
            self.reconnect_needs_interaction(cx);
            return;
        }
        self.prepare_login(connection, None, window, cx);
    }

    pub(super) fn resume_connect_route(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.saving
            && !self.connecting
            && self.login.is_none()
            && self.host_approval.is_none()
            && self
                .connect_route
                .as_ref()
                .is_some_and(|route| route.waiting_for_save)
        {
            self.prepare_route_hop(window, cx);
        }
    }

    pub(super) fn cancel_connect_route(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let background = self
            .connect_route
            .as_ref()
            .is_some_and(|route| route.background);
        if let Some(ticket) = self
            .connect_route
            .as_ref()
            .and_then(|route| route.reconnect)
        {
            self.cancel_reconnect_ticket(ticket);
        }
        self.connect_route = None;
        self.connecting = false;
        self.host_approval = None;
        if !background || self.login.is_some() {
            self.clear_login(window, cx);
        }
        self.status = Message::new("已取消连接", "Connection cancelled");
        if !background {
            self.focus_current_surface(window, cx);
        }
        cx.notify();
    }

    fn route_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_connect_route(window, cx);
        self.status = Message::new(
            "连接或跳板路线已变化，请重新选择连接。",
            "The connection or jump route changed. Select the connection again.",
        );
        cx.notify();
    }

    pub(super) fn login_route_identity(&self, connection: &Connection) -> Option<RouteIdentity> {
        if let Some(route) = &self.connect_route {
            if !self.route_is_current()
                || !route
                    .snapshot
                    .hops()
                    .get(route.current)
                    .is_some_and(|hop| vault::same_destination(hop, connection))
            {
                return None;
            }
            route.snapshot.identity_prefix(route.current)
        } else {
            self.state
                .connection_route(connection.id)
                .ok()
                .map(|route| route.identity())
        }
    }

    pub(super) fn refresh_route_metadata(&mut self) {
        if !self.route_is_current() {
            return;
        }
        if let Some(route) = &mut self.connect_route
            && let Some(target) = route.snapshot.hops().last()
            && let Ok(snapshot) = self.state.connection_route(target.id)
        {
            // Successful explicit credential linking changes metadata, not the
            // frozen network route. Later hops may use those saved references.
            route.snapshot = snapshot;
        }
    }

    pub(super) fn connect(
        &mut self,
        connection: Connection,
        secret: Zeroizing<String>,
        pin: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.connect_with_proxy_secret(
            connection,
            secret,
            Zeroizing::new(String::new()),
            pin,
            window,
            cx,
        );
    }

    pub(super) fn connect_with_proxy_secret(
        &mut self,
        connection: Connection,
        secret: Zeroizing<String>,
        proxy_secret: Zeroizing<String>,
        pin: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connecting {
            return;
        }
        if self.connect_route.is_none() && !self.begin_connect_route(&connection, cx) {
            return;
        }
        if !self.route_is_current() {
            self.route_changed(window, cx);
            return;
        }
        let Some(route) = &self.connect_route else {
            return;
        };
        if !route
            .snapshot
            .hops()
            .get(route.current)
            .is_some_and(|hop| vault::same_destination(hop, &connection))
        {
            self.route_changed(window, cx);
            return;
        }
        let Some(scope) = route.snapshot.host_key_scope(route.current) else {
            return;
        };
        let id = route.id;
        let index = route.current;
        let reconnect = route.reconnect;
        let background = route.background;
        let parent = route.upstream.clone();
        let mut cancelled = route.cancel.subscribe();
        let mut options = SshOptions::new(connection.host.clone(), connection.username.clone());
        options.port = connection.port;
        options.proxy = connection
            .proxy
            .as_ref()
            .map(|proxy| keelshell_session::SshProxy {
                kind: match proxy.kind {
                    keelshell_core::ProxyKind::Socks5 => keelshell_session::ProxyKind::Socks5,
                    keelshell_core::ProxyKind::HttpConnect => {
                        keelshell_session::ProxyKind::HttpConnect
                    }
                },
                host: proxy.host.clone(),
                port: proxy.port,
                credentials: match &proxy.auth {
                    keelshell_core::ProxyAuthentication::None => None,
                    keelshell_core::ProxyAuthentication::UsernamePassword { username } => {
                        Some(keelshell_session::ProxyCredentials {
                            username: Zeroizing::new(username.clone()),
                            password: proxy_secret,
                        })
                    }
                },
            });
        options.expected_host_key =
            pin.or_else(|| self.state.host_key_for_scope(&scope).map(str::to_owned));
        options.auth = match &connection.auth {
            AuthMethod::Agent => SshAuth::Agent,
            AuthMethod::Password => SshAuth::Password(secret),
            AuthMethod::PrivateKey { path } => SshAuth::PrivateKey {
                path: path.clone(),
                passphrase: (!secret.is_empty()).then_some(secret),
            },
        };
        options.timeout = Duration::from_secs(15);
        self.connecting = true;
        self.status = Message::new(
            format!("正在连接 {}…", connection.name),
            format!("Connecting to {}…", connection.name),
        );
        let operation = async move {
            if *cancelled.borrow() {
                return None;
            }
            let connect = async move {
                match parent {
                    Some(parent) if reconnect.is_some() => {
                        SshSession::connect_through(&parent, options).await
                    }
                    None if reconnect.is_some() => SshSession::connect(options).await,
                    Some(parent) => {
                        SshSession::connect_through_with_retry(
                            &parent,
                            options,
                            RetryPolicy::default(),
                        )
                        .await
                    }
                    None => SshSession::connect_with_retry(options, RetryPolicy::default()).await,
                }
            };
            tokio::select! {
                biased;
                _ = cancelled.changed() => None,
                result = connect => Some(result),
            }
        };
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            operation,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = job.await.unwrap_or(Some(Err(SessionError::Worker)));
            let _ = this.update_in(cx, |view, window, cx| {
                if !view
                    .connect_route
                    .as_ref()
                    .is_some_and(|route| route.id == id && route.current == index)
                {
                    return;
                }
                view.connecting = false;
                if !view.route_is_current() {
                    view.route_changed(window, cx);
                    return;
                }
                match result {
                    Some(Ok(session)) => {
                        let Some(route) = &mut view.connect_route else {
                            return;
                        };
                        if index + 1 < route.snapshot.hops().len() {
                            route.upstream = Some(session);
                            route.current += 1;
                            view.prepare_route_hop(window, cx);
                        } else {
                            let snapshot = route.snapshot.clone();
                            view.connect_route = None;
                            if let Some(ticket) = reconnect {
                                if !view.finish_reconnect(ticket, session, snapshot, window, cx) {
                                    return;
                                }
                            } else {
                                view.add_remote_tab(
                                    session,
                                    connection.name.clone(),
                                    format!(
                                        "{}@{}:{}",
                                        connection.username, connection.host, connection.port
                                    ),
                                    window,
                                    cx,
                                );
                                if let Some(terminal) = view.tabs.get(view.active) {
                                    view.bind_remote_tab(terminal.entity_id(), snapshot);
                                }
                            }
                            view.status = Message::new(
                                format!("已连接 {}", connection.name),
                                format!("Connected to {}", connection.name),
                            );
                            view.remember_successful_connection(connection, window, cx);
                        }
                    }
                    Some(Err(SessionError::UnknownHostKey { fingerprint })) => {
                        view.host_approval = Some(HostApproval {
                            attempt: id,
                            index,
                            scope,
                            connection,
                            fingerprint,
                            previous: None,
                        });
                        view.status = Message::new(
                            "连接前请核对当前服务器指纹",
                            "Review this server's identity before connecting",
                        );
                        view.present_route_approval(background, window, cx);
                    }
                    Some(Err(SessionError::ChangedHostKey { expected, actual })) => {
                        view.host_approval = Some(HostApproval {
                            attempt: id,
                            index,
                            scope,
                            connection,
                            fingerprint: actual,
                            previous: Some(expected),
                        });
                        view.status = Message::new(
                            "服务器指纹发生变化，已阻止连接。",
                            "Server identity changed. Connection was blocked.",
                        );
                        view.present_route_approval(background, window, cx);
                    }
                    Some(Err(error)) => {
                        view.connect_route = None;
                        view.status = connection_error(&connection, &error);
                        if let Some(ticket) = reconnect {
                            view.reconnect_failed(ticket, &error, cx);
                        }
                        if !background {
                            view.focus_current_surface(window, cx);
                        }
                    }
                    None => view.cancel_connect_route(window, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn present_route_approval(
        &mut self,
        background: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if background {
            if let Some(route) = &mut self.connect_route {
                route.deferred_approval = self.host_approval.take();
                route.awaiting_interaction = true;
            }
            self.reconnect_needs_interaction(cx);
        } else {
            self.focus_current_surface(window, cx);
        }
    }

    pub(super) fn reject_host_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_connect_route(window, cx);
    }

    pub(super) fn accept_host_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if !self.route_is_current() {
            self.route_changed(window, cx);
            return;
        }
        let Some(approval) = &self.host_approval else {
            return;
        };
        if !self
            .connect_route
            .as_ref()
            .is_some_and(|route| route.id == approval.attempt && route.current == approval.index)
        {
            return;
        }
        let mut candidate = self.state.clone();
        match candidate.trust_host_key_for_scope(&approval.scope, &approval.fingerprint) {
            Ok(()) => self.persist(
                candidate,
                AfterSave::Trust {
                    attempt: approval.attempt,
                    index: approval.index,
                },
                window,
                cx,
            ),
            Err(error) => {
                self.status = Message::detail("保存失败", "Save failed", error);
                cx.notify();
            }
        }
    }

    pub(super) fn finish_host_trust(
        &mut self,
        attempt: uuid::Uuid,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .connect_route
            .as_ref()
            .is_some_and(|route| route.id == attempt && route.current == index)
        {
            return;
        }
        self.host_approval = None;
        self.prepare_route_hop(window, cx);
    }
}

/// Translate bounded protocol categories, never proxy headers or response text.
fn connection_error(connection: &Connection, error: &SessionError) -> Message {
    use keelshell_session::ProxyError;
    match error {
        SessionError::Proxy(ProxyError::AuthenticationRequired) => Message::new(
            "代理要求认证，请在连接配置中启用代理认证并填写用户名。",
            "The proxy requires authentication. Enable proxy authentication and set its username in the profile.",
        ),
        SessionError::Proxy(ProxyError::AuthenticationRejected) => Message::new(
            "代理认证失败，请检查代理用户名和本次输入的密码。",
            "Proxy authentication failed. Check the proxy username and the password entered for this connection.",
        ),
        SessionError::Proxy(ProxyError::UnsupportedAuthentication) => Message::new(
            "代理不支持已配置的认证方式，已停止连接。",
            "The proxy does not support the configured authentication method; the connection was stopped.",
        ),
        SessionError::Proxy(ProxyError::Socks5Rejected(code)) => Message::new(
            format!("SOCKS5 代理拒绝连接目标（代码 {code}）。"),
            format!("The SOCKS5 proxy refused the destination (code {code})."),
        ),
        SessionError::Proxy(ProxyError::HttpRejected(status)) => Message::new(
            format!("HTTP 代理拒绝 CONNECT（状态 {status}）。"),
            format!("The HTTP proxy refused CONNECT (status {status})."),
        ),
        SessionError::Proxy(ProxyError::InvalidResponse(_)) => Message::new(
            "代理响应无效或协议不受支持，已停止连接。",
            "The proxy response was invalid or unsupported; the connection was stopped.",
        ),
        SessionError::Proxy(ProxyError::ResponseTooLarge) => Message::new(
            "代理响应超过限制，已停止连接。",
            "The proxy response exceeded protocol limits; the connection was stopped.",
        ),
        _ => Message::detail(
            &format!("连接 {} 失败", connection.name),
            &format!("Connection to {} failed", connection.name),
            error,
        ),
    }
}
