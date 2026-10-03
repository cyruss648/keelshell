//! Explicit per-use unlocking; no unlocked vault or master password lives in the workspace.

use std::sync::atomic::{AtomicBool, Ordering};

use keelshell_core::{CredentialKind, Error, VaultStore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LoginMode {
    Once,
    Save,
    Unlock,
}

pub(super) fn needs_proxy_password(connection: &Connection) -> bool {
    connection.proxy.as_ref().is_some_and(|proxy| {
        matches!(
            proxy.auth,
            keelshell_core::ProxyAuthentication::UsernamePassword { .. }
        )
    })
}

impl LoginPrompt {
    pub(super) fn clear_inputs(&self, window: &mut Window, cx: &mut App) {
        for input in [
            &self.secret,
            &self.proxy_secret,
            &self.master,
            &self.confirmation,
        ] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        let field = if self.mode != LoginMode::Save && needs_proxy_password(&self.connection) {
            &self.proxy_secret
        } else if self.mode == LoginMode::Unlock {
            &self.master
        } else {
            &self.secret
        };
        field.read(cx).focus_handle(cx).focus(window, cx);
    }
}

impl Drop for LoginPrompt {
    fn drop(&mut self) {
        // KDF cannot be interrupted midway. A write already admitted may finish
        // (including a state save); the dismissed prompt cannot start SSH.
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundCredential {
    version: u32,
    host: String,
    port: u16,
    username: String,
    auth: AuthMethod,
    secret: String,
    #[serde(default)]
    route: Option<keelshell_core::RouteIdentity>,
}

impl Drop for BoundCredential {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

pub(super) fn same_destination(a: &Connection, b: &Connection) -> bool {
    a.id == b.id
        && keelshell_core::RouteEndpoint::from_connection(a)
            .ok()
            .zip(keelshell_core::RouteEndpoint::from_connection(b).ok())
            .is_some_and(|(a, b)| a == b)
        && a.auth == b.auth
        && a.jump_host == b.jump_host
}

fn kind(connection: &Connection) -> Result<CredentialKind, Error> {
    match connection.auth {
        AuthMethod::Password => Ok(CredentialKind::Password),
        AuthMethod::PrivateKey { .. } => Ok(CredentialKind::PrivateKeyPassphrase),
        AuthMethod::Agent => Err(Error::VaultEntryMismatch),
    }
}

fn encode(
    connection: &Connection,
    route: &keelshell_core::RouteIdentity,
    secret: Zeroizing<String>,
) -> Result<Zeroizing<String>, Error> {
    let payload = BoundCredential {
        version: 2,
        route: Some(route.clone()),
        host: connection.host.clone(),
        port: connection.port,
        username: connection.username.clone(),
        auth: connection.auth.clone(),
        secret: secret.to_string(),
    };
    serde_json::to_string(&payload)
        .map(Zeroizing::new)
        .map_err(|_| Error::VaultCorrupt)
}

fn decode(
    connection: &Connection,
    route: &keelshell_core::RouteIdentity,
    payload: Zeroizing<String>,
) -> Result<Zeroizing<String>, Error> {
    let mut payload: BoundCredential =
        serde_json::from_str(&payload).map_err(|_| Error::VaultCorrupt)?;
    let route_matches = match payload.version {
        1 => {
            payload.route.is_none()
                && connection.jump_host.is_none()
                && connection.proxy.is_none()
                && route.endpoints().len() == 1
                && route
                    .endpoints()
                    .first()
                    .is_some_and(|endpoint| endpoint.proxy.is_none())
        }
        2 => payload.route.as_ref() == Some(route),
        _ => false,
    };
    let mut bound = connection.clone();
    bound.host = payload.host.clone();
    bound.port = payload.port;
    bound.username = payload.username.clone();
    if !route_matches || !same_destination(&bound, connection) || payload.auth != connection.auth {
        return Err(Error::VaultEntryMismatch);
    }
    Ok(Zeroizing::new(std::mem::take(&mut payload.secret)))
}

enum Completion {
    Saved(uuid::Uuid),
    Unlocked(Zeroizing<String>),
    Cancelled,
}

fn vault_operation(
    path: std::path::PathBuf,
    connection: &Connection,
    route: &keelshell_core::RouteIdentity,
    mode: LoginMode,
    master: Zeroizing<String>,
    secret: Zeroizing<String>,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    let store = VaultStore::new(path);
    let mut vault = store.load(&master)?;
    // Drop the passphrase immediately after deriving/authenticating the key.
    drop(master);
    if cancelled.load(Ordering::Acquire) {
        return Ok(Completion::Cancelled);
    }
    let kind = kind(connection)?;
    match mode {
        LoginMode::Unlock => {
            let reference = connection.credential_ref.ok_or(Error::VaultEntryNotFound)?;
            let secret = decode(
                connection,
                route,
                vault.get(reference, connection.id, kind)?,
            )?;
            Ok(Completion::Unlocked(secret))
        }
        LoginMode::Save => {
            // Never overwrite a referenced entry. A state save failure may
            // leave an orphan, but cannot destroy a previously usable secret.
            let reference = uuid::Uuid::new_v4();
            let payload = encode(connection, route, secret)?;
            vault.set(reference, connection.id, kind, &payload)?;
            if cancelled.load(Ordering::Acquire) {
                return Ok(Completion::Cancelled);
            }
            store.save(&mut vault)?;
            Ok(Completion::Saved(reference))
        }
        LoginMode::Once => Err(Error::VaultInvalidEntry),
    }
}

impl Workspace {
    pub(super) fn cancel_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_connect_route(window, cx);
    }

    pub(super) fn clear_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(login) = self.login.take() {
            login.clear_inputs(window, cx);
        }
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn login_mode(
        &mut self,
        mode: LoginMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ephemeral = self.route_is_ephemeral();
        if let Some(login) = &mut self.login {
            if login.busy
                || self.saving
                || matches!(login.connection.auth, AuthMethod::Agent) && mode != LoginMode::Once
                || ephemeral && mode != LoginMode::Once
            {
                return;
            }
            login.clear_inputs(window, cx);
            login.mode = mode;
            if mode != LoginMode::Once
                && let Some(route) = &mut self.connect_route
            {
                route.keyboard_interactive = false;
            }
            login.message = None;
            login.focus(window, cx);
            cx.notify();
        }
    }

    pub(super) fn login_is_current(&self, connection: &Connection) -> bool {
        let route_current = self.login.as_ref().is_some_and(|login| {
            self.login_route_identity(connection).as_ref() == Some(&login.route_identity)
        });
        if !route_current {
            return false;
        }
        let saved_profile = self.state.connections.iter().any(|current| {
            same_destination(current, connection)
                && current.credential_ref == connection.credential_ref
        });
        let ephemeral_route = self.ephemeral_route_matches(connection);
        saved_profile || ephemeral_route
    }

    pub(super) fn submit_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(login) = &self.login else {
            return;
        };
        if login.busy || self.saving || self.connecting {
            return;
        }
        let ephemeral = self.route_is_ephemeral();
        if ephemeral && login.mode != LoginMode::Once {
            self.status = Message::new(
                "临时连接只能使用本次凭据，不能保存到凭据库。",
                "One-time connections can only use an ephemeral secret; they cannot save to the vault.",
            );
            self.cancel_login(window, cx);
            return;
        }
        if !self.login_is_current(&login.connection) {
            self.status = Message::new(
                "连接目标已变更，请重新打开认证窗口。",
                "The connection changed. Reopen authentication.",
            );
            self.cancel_login(window, cx);
            return;
        }
        if login.mode != LoginMode::Save && needs_proxy_password(&login.connection) {
            let password = login.proxy_secret.read(cx).value();
            let invalid = login
                .connection
                .proxy
                .as_ref()
                .is_some_and(|proxy| match proxy.kind {
                    keelshell_core::ProxyKind::Socks5 => {
                        password.is_empty() || password.len() > 255
                    }
                    keelshell_core::ProxyKind::HttpConnect => {
                        password.len() > 4096 || password.chars().any(char::is_control)
                    }
                });
            if invalid {
                if let Some(login) = &mut self.login {
                    login.message = Some(Message::new(
                        "SOCKS5 代理密码须为 1–255 字节；HTTP 代理密码最多 4096 字节且不能包含控制字符。",
                        "SOCKS5 passwords require 1–255 bytes; HTTP proxy passwords allow up to 4096 bytes and no control characters.",
                    ));
                    login
                        .proxy_secret
                        .read(cx)
                        .focus_handle(cx)
                        .focus(window, cx);
                }
                cx.notify();
                return;
            }
        }
        if login.mode == LoginMode::Once {
            let secret = Zeroizing::new(login.secret.read(cx).value().to_string());
            let proxy_secret = Zeroizing::new(login.proxy_secret.read(cx).value().to_string());
            let connection = login.connection.clone();
            let pin = login.pin.clone();
            self.clear_login(window, cx);
            self.connect_with_proxy_secret(connection, secret, proxy_secret, pin, window, cx);
            return;
        }
        let Some(login) = &mut self.login else {
            return;
        };
        let master = Zeroizing::new(login.master.read(cx).value().to_string());
        if master.is_empty() || master.len() > 4096 {
            login.message = Some(Message::new(
                "请输入主密码（最多 4096 字节）。",
                "Enter the master password (up to 4096 bytes).",
            ));
            cx.notify();
            return;
        }
        let secret = Zeroizing::new(login.secret.read(cx).value().to_string());
        if login.mode == LoginMode::Save {
            if secret.is_empty() {
                login.message = Some(Message::new(
                    "空口令无需保存，请选择仅本次使用。",
                    "An empty passphrase does not need saving. Choose one-time authentication.",
                ));
                cx.notify();
                return;
            }
            let confirmation = Zeroizing::new(login.confirmation.read(cx).value().to_string());
            if *master != *confirmation {
                login.message = Some(Message::new(
                    "两次输入的主密码不一致。",
                    "The master passwords do not match.",
                ));
                cx.notify();
                return;
            }
        }
        // Proxy authentication never enters the vault operation or its payload.
        // Only Unlock carries this ephemeral value to the network completion.
        let proxy_secret = if login.mode == LoginMode::Unlock {
            Zeroizing::new(login.proxy_secret.read(cx).value().to_string())
        } else {
            Zeroizing::new(String::new())
        };
        login.clear_inputs(window, cx);
        login.busy = true;
        login.message = Some(Message::new("正在处理凭据…", "Processing credential…"));
        let prompt = login.id;
        let mode = login.mode;
        let connection = login.connection.clone();
        let operation_connection = connection.clone();
        let route_identity = login.route_identity.clone();
        let cancelled = login.cancelled.clone();
        let path = self.store.path().with_file_name("vault.json");
        // Freeze state mutations until the operation completes. Closing the
        // dialog remains available and invalidates its completion token.
        self.saving = true;
        let job = cx.background_executor().spawn(async move {
            vault_operation(
                path,
                &operation_connection,
                &route_identity,
                mode,
                master,
                secret,
                &cancelled,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = job.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.saving = false;
                let current = view.login.as_ref().is_some_and(|login| login.id == prompt)
                    && view.login_is_current(&connection);
                if !current {
                    if view.login.as_ref().is_some_and(|login| login.id == prompt) {
                        view.cancel_login(window, cx);
                        view.status = Message::new(
                            "连接目标已变更，已取消认证。请重新选择连接。",
                            "The connection changed; authentication was cancelled. Select it again.",
                        );
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Completion::Saved(reference)) => {
                        let mut candidate = view.state.clone();
                        if let Some(profile) = candidate
                            .connections
                            .iter_mut()
                            .find(|profile| profile.id == connection.id)
                        {
                            profile.credential_ref = Some(reference);
                            let connection = profile.clone();
                            view.persist(
                                candidate,
                                AfterSave::CredentialLinked { prompt, connection: Box::new(connection) },
                                window,
                                cx,
                            );
                        } else {
                            view.cancel_login(window, cx);
                            view.status = Message::new(
                                "临时连接不能保存凭据，已取消本次操作。",
                                "A one-time connection cannot save credentials; the operation was cancelled.",
                            );
                        }
                    }
                    Ok(Completion::Unlocked(secret)) => {
                        let pin = view.login.as_ref().and_then(|login| login.pin.clone());
                        view.clear_login(window, cx);
                        view.connect_with_proxy_secret(connection, secret, proxy_secret, pin, window, cx);
                    }
                    Ok(Completion::Cancelled) => view.cancel_login(window, cx),
                    Err(error) => {
                        if let Some(login) = &mut view.login {
                            login.busy = false;
                            login.message = Some(vault_error(&error));
                            login.focus(window, cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn forget_credential(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(login) = &self.login else {
            return;
        };
        if login.busy || self.saving || !self.login_is_current(&login.connection) {
            return;
        }
        let prompt = login.id;
        let mut candidate = self.state.clone();
        if let Some(profile) = candidate
            .connections
            .iter_mut()
            .find(|profile| profile.id == login.connection.id)
        {
            profile.credential_ref = None;
            let connection = profile.clone();
            if let Some(login) = &mut self.login {
                login.clear_inputs(window, cx);
                login.busy = true;
            }
            self.persist(
                candidate,
                AfterSave::CredentialLinked {
                    prompt,
                    connection: Box::new(connection),
                },
                window,
                cx,
            );
        }
    }

    pub(super) fn finish_credential_save(
        &mut self,
        prompt: uuid::Uuid,
        connection: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_route_metadata();
        if let Some(login) = self.login.as_mut().filter(|login| login.id == prompt) {
            let saved = connection.credential_ref.is_some();
            login.connection = connection;
            login.busy = false;
            login.mode = if saved {
                LoginMode::Unlock
            } else {
                LoginMode::Once
            };
            login.message = Some(if saved {
                Message::new(
                    "已保存并锁定。输入主密码后可解锁连接。",
                    "Saved and locked. Enter the master password to connect.",
                )
            } else {
                Message::new(
                    "已解除凭据关联。加密条目仍保留在本机凭据库中。",
                    "Credential unlinked. Its encrypted entry remains in the local vault.",
                )
            });
            login.clear_inputs(window, cx);
            login.focus(window, cx);
        }
    }
}

fn vault_error(error: &Error) -> Message {
    match error {
        Error::VaultUnlockFailed => Message::new(
            "无法解锁：主密码错误或凭据库已损坏。请重新输入。",
            "Unlock failed: incorrect master password or damaged vault. Try again.",
        ),
        Error::VaultEntryMismatch => Message::new(
            "保存的凭据不属于当前连接目标。请使用一次性凭据或解除关联。",
            "Saved credential does not match this destination. Use one-time authentication or unlink it.",
        ),
        Error::VaultEntryNotFound => Message::new(
            "找不到保存的凭据。请使用一次性凭据或解除关联。",
            "Saved credential is missing. Use one-time authentication or unlink it.",
        ),
        Error::Conflict | Error::VaultConflict | Error::Busy => Message::new(
            "凭据库正在使用或已被修改，请重试。",
            "The vault is busy or changed. Retry the operation.",
        ),
        _ => Message::new(
            "凭据操作失败，未发起连接。请检查本机凭据库文件和权限。",
            "Credential operation failed; no connection started. Check the local vault file and permissions.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn equivalent_dns_spellings_preserve_encrypted_route_binding() -> Result<(), Error> {
        let mut connection = Connection::new("Target", "SERVER.Example.", "Operator");
        connection.auth = AuthMethod::Password;
        let mut state = AppState {
            connections: vec![connection.clone()],
            ..Default::default()
        };
        let route = state.connection_route(connection.id)?.identity();
        let payload = encode(&connection, &route, Zeroizing::new("fixture-secret".into()))?;
        connection.host = "server.example".into();
        state.update_connection(connection.clone())?;
        let equivalent = state.connection_route(connection.id)?.identity();
        assert_eq!(
            decode(&connection, &equivalent, payload)?.as_str(),
            "fixture-secret"
        );
        Ok(())
    }

    #[::core::prelude::v1::test]
    fn encrypted_secret_is_bound_to_every_upstream_destination() -> Result<(), Error> {
        let mut gateway = Connection::new("Gateway", "gateway.test", "operator");
        gateway.auth = AuthMethod::Password;
        let mut target = Connection::new("Target", "private.test", "root");
        target.auth = AuthMethod::Password;
        target.jump_host = Some(gateway.id);
        let state = AppState {
            connections: vec![gateway.clone(), target.clone()],
            ..Default::default()
        };
        let original = state.connection_route(target.id)?.identity();
        let payload = encode(
            &target,
            &original,
            Zeroizing::new("route-fixture-secret".into()),
        )?;
        assert_eq!(
            decode(&target, &original, payload.clone())?.as_str(),
            "route-fixture-secret"
        );
        for changed in [
            {
                let mut hop = gateway.clone();
                hop.host = "other.test".into();
                hop
            },
            {
                let mut hop = gateway.clone();
                hop.port = 2200;
                hop
            },
            {
                let mut hop = gateway.clone();
                hop.username = "other-user".into();
                hop
            },
        ] {
            let mut current = state.clone();
            current.update_connection(changed)?;
            let identity = current.connection_route(target.id)?.identity();
            assert!(matches!(
                decode(&target, &identity, payload.clone()),
                Err(Error::VaultEntryMismatch)
            ));
        }
        let mut renamed = state;
        gateway.name = "New gateway display name".into();
        renamed.update_connection(gateway)?;
        assert_eq!(
            decode(
                &target,
                &renamed.connection_route(target.id)?.identity(),
                payload
            )?
            .as_str(),
            "route-fixture-secret"
        );
        Ok(())
    }

    #[::core::prelude::v1::test]
    fn legacy_secret_can_only_unlock_a_direct_connection() -> Result<(), Error> {
        let mut target = Connection::new("Target", "private.test", "root");
        target.auth = AuthMethod::Password;
        let mut state = AppState {
            connections: vec![target.clone()],
            ..Default::default()
        };
        let direct = state.connection_route(target.id)?.identity();
        let payload = Zeroizing::new(
            serde_json::to_string(&serde_json::json!({
                "version": 1, "host": target.host, "port": target.port,
                "username": target.username, "auth": target.auth, "secret": "legacy-fixture-secret",
            }))
            .map_err(|_| Error::VaultCorrupt)?,
        );
        assert_eq!(
            decode(&target, &direct, payload.clone())?.as_str(),
            "legacy-fixture-secret"
        );
        let mut proxied = target.clone();
        proxied.proxy = Some(keelshell_core::ConnectionProxy::new(
            keelshell_core::ProxyKind::Socks5,
            "proxy.test",
            1080,
        ));
        state.update_connection(proxied.clone())?;
        let proxied_route = state.connection_route(target.id)?.identity();
        assert!(matches!(
            decode(&proxied, &proxied_route, payload.clone()),
            Err(Error::VaultEntryMismatch)
        ));
        assert!(
            matches!(
                decode(&target, &proxied_route, payload.clone()),
                Err(Error::VaultEntryMismatch)
            ),
            "even a mismatched internal connection snapshot cannot unlock v1 through a proxy"
        );
        state.update_connection(target.clone())?;
        let gateway = Connection::new("Gateway", "gateway.test", "operator");
        target.jump_host = Some(gateway.id);
        state.connections.push(gateway);
        state.update_connection(target.clone())?;
        let routed = state.connection_route(target.id)?.identity();
        assert!(matches!(
            decode(&target, &routed, payload),
            Err(Error::VaultEntryMismatch)
        ));
        Ok(())
    }

    #[::core::prelude::v1::test]
    fn encrypted_payload_refuses_changed_destination_user_auth_or_private_key_path()
    -> Result<(), Error> {
        let mut connection = Connection::new("Node", "host.test", "operator");
        connection.auth = AuthMethod::PrivateKey {
            path: "keys/first".into(),
        };
        let state = AppState {
            connections: vec![connection.clone()],
            ..AppState::default()
        };
        let route = state.connection_route(connection.id)?.identity();
        let payload = encode(
            &connection,
            &route,
            Zeroizing::new("private-fixture-passphrase".into()),
        )?;
        assert_eq!(
            decode(&connection, &route, payload.clone())?.as_str(),
            "private-fixture-passphrase"
        );
        let mut variants = Vec::new();
        let mut edited = connection.clone();
        edited.host = "other.test".into();
        variants.push(edited);
        let mut edited = connection.clone();
        edited.port = 2222;
        variants.push(edited);
        let mut edited = connection.clone();
        edited.username = "root".into();
        variants.push(edited);
        let mut edited = connection.clone();
        edited.auth = AuthMethod::Password;
        variants.push(edited);
        let mut edited = connection.clone();
        edited.auth = AuthMethod::PrivateKey {
            path: "keys/second".into(),
        };
        variants.push(edited);
        for edited in variants {
            assert!(matches!(
                decode(&edited, &route, payload.clone()),
                Err(Error::VaultEntryMismatch)
            ));
        }
        connection.name = "Different label".into();
        connection.group = "Different folder".into();
        assert!(decode(&connection, &route, payload).is_ok());
        Ok(())
    }
}
