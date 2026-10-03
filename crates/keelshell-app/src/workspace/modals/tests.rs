//! Render the actual workspace modals at constrained desktop sizes.
use crate::{
    i18n::{self, Message},
    workspace::Workspace,
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Focusable, Pixels, TestAppContext, WindowBounds,
    WindowOptions, point, px, size, test::TestWindowExt,
};
use keelshell_core::{AuthMethod, Connection, Language, StateStore};
use std::sync::Arc;

trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}

struct Fixture {
    window: AnyWindowHandle,
    workspace: Entity<Workspace>,
    directory: std::path::PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn mount(cx: &mut TestAppContext, width: f32, height: f32) -> Fixture {
    let directory =
        std::env::temp_dir().join(format!("keelshell-route-layout-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).checked("create layout state directory");
    let store = Arc::new(StateStore::new(directory.join("state.json")));
    let mut state = store.load().checked("load empty layout state");
    let mut connection = Connection::new(
        "测试连接名称".repeat(18),
        format!("{}.example.test", "host".repeat(50)),
        "operator".repeat(12),
    );
    connection.auth = AuthMethod::Password;
    state.connections = vec![connection];
    state = store
        .save(&state)
        .checked("seed only synthetic connection metadata");
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .checked("layout runtime"),
    );
    let (window, workspace) = cx.update(|cx| {
        gpui_kit::init(cx);
        i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(width), px(height)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::new(store, state, None, runtime, window, cx)),
        )
        .checked("mount production workspace for layout")
    });
    Fixture {
        window,
        workspace,
        directory,
    }
}

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
fn connection_form_keeps_save_and_cancel_outside_long_scrollable_drafts(cx: &mut TestAppContext) {
    for (width, height) in [(1280., 840.), (760., 560.), (480., 420.)] {
        let fixture = mount(cx, width, height);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                let connection = workspace.state.connections[0].clone();
                workspace.edit_connection(connection, window, cx);
                workspace.status = Message::new(
                    "请审核配置。".repeat(80),
                    "Review this profile before saving. ".repeat(80),
                );
            });
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let area = window.find("connection-editor").bounds();
                let body = window.find("connection-editor-body").bounds();
                let footer = window.find("connection-editor-footer").bounds();
                assert!(
                    contained(area, window.bounds()),
                    "editor escaped {width}x{height}: {area:?}"
                );
                assert!(contained(footer, area));
                assert!(body.bottom() <= footer.origin.y);
                for id in ["cancel-connection", "save-connection"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    assert!(
                        contained(button.bounds(), footer),
                        "{id} escaped footer at {width}x{height}"
                    );
                }
            }
            window.click("cancel-connection", cx);
            assert!(fixture.workspace.read(cx).form.is_none());
        })
        .checked("bilingual connection form footer remains clickable");
    }
}

#[gpui_kit::test]
fn authentication_footer_remains_visible_with_route_metadata_and_long_vault_feedback(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1280., 840.), (760., 560.), (480., 420.)] {
        let fixture = mount(cx, width, height);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                let connection = workspace.state.connections[0].clone();
                workspace.prepare_login(connection, None, window, cx);
                let login = workspace
                    .login
                    .as_mut()
                    .unwrap_or_else(|| panic!("password prompt must precede any network request"));
                login.mode = super::super::vault::LoginMode::Save;
                login.message = Some(Message::new(
                    "凭据校验未完成。".repeat(100),
                    "Credential verification is incomplete. ".repeat(100),
                ));
                cx.notify();
            });
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let area = window.find("ssh-authentication").bounds();
                let body = window.find("ssh-authentication-body").bounds();
                let footer = window.find("ssh-authentication-footer").bounds();
                assert!(
                    contained(area, window.bounds()),
                    "authentication escaped {width}x{height}: {area:?}"
                );
                assert!(contained(footer, area));
                assert!(body.bottom() <= footer.origin.y);
                for id in ["cancel-login", "submit-login"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    assert!(contained(button.bounds(), footer));
                }
                assert_eq!(
                    window.find("submit-login").label(),
                    Some(if language == Language::En {
                        "Save credential"
                    } else {
                        "保存凭据"
                    })
                );
            }
            assert!(!fixture.workspace.read(cx).connecting);
            window.click("cancel-login", cx);
            assert!(fixture.workspace.read(cx).login.is_none());
        })
        .checked("credential review stays bounded without contacting a server");
    }
}

#[gpui_kit::test]
fn ordinary_authentication_is_compact_without_a_duplicate_endpoint(cx: &mut TestAppContext) {
    let fixture = mount(cx, 1280., 840.);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let connection = &mut workspace.state.connections[0];
            connection.name = "Production".into();
            connection.host = "server.example.test".into();
            connection.username = "operator".into();
            let connection = connection.clone();
            workspace.prepare_login(connection, None, window, cx);
        });
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            let panel = window.find("ssh-authentication").bounds();
            let body = window.find("ssh-authentication-body").bounds();
            let content = window.find("ssh-login-content").bounds();
            let footer = window.find("ssh-authentication-footer").bounds();
            assert!(panel.size.height <= px(520.));
            assert!(contained(content, body));
            assert!(
                body.bottom() - content.bottom() <= px(160.),
                "ordinary authentication has excessive empty body space: {body:?} / {content:?}"
            );
            assert!(window.try_find("ssh-login-endpoint").is_none());
            assert!(window.find("connection-route-status").visible());
            assert!(contained(window.find("submit-login").bounds(), footer));
            assert!(contained(window.find("cancel-login").bounds(), footer));
        }
        window.click("cancel-login", cx);
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("ordinary password authentication stays compact in both languages");
}

#[gpui_kit::test]
fn proxy_authentication_keeps_footer_and_password_focus_in_small_bilingual_windows(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{ConnectionProxy, ProxyAuthentication, ProxyKind};
    for (width, height) in [(1280., 840.), (760., 560.), (480., 420.)] {
        let fixture = mount(cx, width, height);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                let mut connection = view.state.connections[0].clone();
                connection.proxy = Some(ConnectionProxy {
                    kind: ProxyKind::HttpConnect,
                    host: format!("{}.example.test", "proxy".repeat(35)),
                    port: 8080,
                    auth: ProxyAuthentication::UsernamePassword {
                        username: "代理账户".repeat(20),
                    },
                });
                view.state
                    .update_connection(connection.clone())
                    .checked("valid long proxy metadata");
                view.prepare_login(connection, None, window, cx);
            });
            window.render_frame(cx);
            window.input("draft-only-proxy-password", cx);
            let secret = fixture
                .workspace
                .read(cx)
                .login
                .as_ref()
                .unwrap_or_else(|| panic!("proxy login"))
                .proxy_secret
                .clone();
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let area = window.find("ssh-authentication").bounds();
                let body = window.find("ssh-authentication-body").bounds();
                let footer = window.find("ssh-authentication-footer").bounds();
                assert!(contained(area, window.bounds()));
                assert!(contained(footer, area));
                assert!(body.bottom() <= footer.origin.y);
                for id in ["cancel-login", "submit-login"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    assert!(contained(button.bounds(), footer));
                }
                assert!(secret.read(cx).focus_handle(cx).is_focused(window));
                assert_eq!(
                    secret.read(cx).value().as_str(),
                    "draft-only-proxy-password"
                );
                assert!(window.try_find("proxy-login-section").is_some());
            }
            window.click("cancel-login", cx);
            assert!(secret.read(cx).value().is_empty());
        })
        .checked("proxy auth scrolls without hiding footer or losing input");
    }
}

#[gpui_kit::test]
fn proxy_connection_editor_can_scroll_to_click_and_edit_its_username(cx: &mut TestAppContext) {
    use keelshell_core::{ConnectionProxy, ProxyAuthentication, ProxyKind};
    for (width, height) in [(1280., 840.), (760., 560.), (480., 420.)] {
        let fixture = mount(cx, width, height);
        cx.update_window(fixture.window,|_,window,cx| {
            fixture.workspace.update(cx,|view,cx| {
                let mut connection=view.state.connections[0].clone();
                connection.proxy=Some(ConnectionProxy{kind:ProxyKind::HttpConnect,host:"proxy.example".into(),port:8080,auth:ProxyAuthentication::UsernamePassword{username:"old-proxy-user".into()}});
                view.state.update_connection(connection.clone()).checked("seed proxy metadata");
                view.edit_connection(connection,window,cx);
            });
            for language in [Language::ZhCn,Language::En] {
                i18n::set_language(language,cx);
                window.render_frame(cx);
                let body=window.find("connection-editor-body").bounds();
                let before=window.find("proxy-username").bounds();
                if !contained(before,body) {
                    let delta=body.center().y-before.center().y;
                    window.scroll("connection-editor-body",gpui_kit::ScrollDelta::Pixels(point(px(0.),delta)),cx);
                    window.render_frame(cx);
                }
                let username=window.find("proxy-username");
                assert!(username.visible());
                assert!(contained(username.bounds(),body),"proxy username did not scroll into body at {width}x{height}: before={before:?}, after={:?}, body={body:?}, content={:?}",username.bounds(),window.find("connection-editor-content").bounds());
                window.click("proxy-username",cx);
                window.press(if cfg!(target_os="macos") {"cmd-a"} else {"ctrl-a"},cx);
                window.input("edited-proxy-user",cx);
                let view=fixture.workspace.read(cx);
                let proxy=view.form.as_ref().unwrap_or_else(||panic!("editor")).proxy_editor.read(cx).draft(cx).unwrap_or_else(|error|panic!("{error:?}")).unwrap_or_else(||panic!("proxy"));
                assert!(matches!(proxy.auth,ProxyAuthentication::UsernamePassword{username} if username=="edited-proxy-user"));
                assert_eq!(proxy.port,8080,"typing must not land in the previous port field");
            }
        }).checked("scroll actual modal body and type into proxy username");
    }
}
