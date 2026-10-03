use super::ProxyEditor;
use crate::i18n::{self};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions};
use keelshell_core::Language;
use keelshell_core::{ConnectionProxy, ProxyKind};

#[gpui_kit::test]
fn proxy_editor_preserves_raw_draft_across_collapse_type_and_language(cx: &mut TestAppContext) {
    let (window, editor) = cx.update(|cx| {
        gpui_kit::init(cx);
        i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ProxyEditor::new(None, window, cx))
        })
        .checked("mount proxy draft")
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-proxy-editor", cx);
        window.render_frame(cx);
        window.click("proxy-socks5", cx);
        editor.update(cx, |editor, cx| {
            editor
                .host
                .update(cx, |field, cx| field.set_value("proxy.example", window, cx));
            editor
                .port
                .update(cx, |field, cx| field.set_value("3128", window, cx));
            editor
                .username
                .update(cx, |field, cx| field.set_value("代理用户", window, cx));
        });
        window.render_frame(cx);
        window.click("proxy-auth-mode", cx);
        let signature = editor.read(cx).signature(cx);
        let expected = editor
            .read(cx)
            .draft(cx)
            .checked("valid SOCKS draft")
            .checked("enabled");
        window.render_frame(cx);
        window.click("toggle-proxy-editor", cx);
        editor.update(cx, |editor, cx| {
            i18n::set_language(Language::En, cx);
            editor.refresh_locale(window, cx)
        });
        assert_eq!(editor.read(cx).signature(cx), signature);
        window.render_frame(cx);
        window.click("toggle-proxy-editor", cx);
        window.render_frame(cx);
        window.click("proxy-off", cx);
        assert!(
            editor
                .read(cx)
                .draft(cx)
                .checked("disabled draft")
                .is_none()
        );
        window.render_frame(cx);
        window.click("proxy-socks5", cx);
        assert_eq!(
            editor.read(cx).draft(cx).checked("restored draft"),
            Some(expected)
        );
    })
    .checked("exercise real draft controls");
}

#[gpui_kit::test]
fn proxy_editor_rejects_invalid_fields_without_rewriting_them(cx: &mut TestAppContext) {
    let (window, editor) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                ProxyEditor::new(
                    Some(&ConnectionProxy::new(
                        ProxyKind::HttpConnect,
                        "proxy.example",
                        8080,
                    )),
                    window,
                    cx,
                )
            })
        })
        .checked("mount HTTP draft")
    });
    cx.update_window(window, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.authenticated = true;
            editor
                .username
                .update(cx, |field, cx| field.set_value(" user:pass ", window, cx));
        });
        let before = editor.read(cx).signature(cx);
        assert!(editor.read(cx).draft(cx).is_err());
        assert_eq!(editor.read(cx).signature(cx), before);
        editor.update(cx, |editor, cx| {
            editor
                .username
                .update(cx, |field, cx| field.set_value("user", window, cx));
            editor
                .port
                .update(cx, |field, cx| field.set_value("0", window, cx));
        });
        assert!(editor.read(cx).draft(cx).is_err());
        assert_eq!(editor.read(cx).port.read(cx).value().as_str(), "0");
    })
    .checked("validate malformed fields");
}

trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}
impl<T> Checked<T> for Option<T> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|| panic!("{label}"))
    }
}
