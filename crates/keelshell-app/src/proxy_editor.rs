//! Secret-free upstream proxy configuration embedded in the connection draft.
use crate::{
    design::{BORDER, CANVAS, MUTED},
    i18n::{Message, t},
};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    prelude::FluentBuilder,
    *,
};
use keelshell_core::{ConnectionProxy, ProxyAuthentication, ProxyKind};

/// Draft fields retain their values when collapsed, disabled, or translated.
pub(crate) struct ProxyEditor {
    kind: Option<ProxyKind>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    authenticated: bool,
    username: Entity<InputState>,
    expanded: bool,
}

pub(crate) fn proxy_endpoint(proxy: &ConnectionProxy) -> String {
    let host = if proxy.host.contains(':') {
        format!("[{}]", proxy.host)
    } else {
        proxy.host.clone()
    };
    format!("{} · {host}:{}", proxy_kind(proxy.kind), proxy.port)
}

pub(crate) fn proxy_kind(kind: ProxyKind) -> &'static str {
    match kind {
        ProxyKind::Socks5 => "SOCKS5",
        ProxyKind::HttpConnect => "HTTP CONNECT",
    }
}

pub(crate) fn proxy_username(proxy: &ConnectionProxy) -> Option<&str> {
    match &proxy.auth {
        ProxyAuthentication::None => None,
        ProxyAuthentication::UsernamePassword { username } => Some(username),
    }
}

impl ProxyEditor {
    pub(crate) fn new(
        proxy: Option<&ConnectionProxy>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let host = cx.new(|cx| {
            let mut field = InputState::new(window, cx);
            field.set_value(
                proxy.map(|proxy| proxy.host.as_str()).unwrap_or_default(),
                window,
                cx,
            );
            field
        });
        let port = cx.new(|cx| {
            let mut field = InputState::new(window, cx);
            field.set_value(
                proxy
                    .map(|proxy| proxy.port.to_string())
                    .unwrap_or_else(|| "1080".into()),
                window,
                cx,
            );
            field
        });
        let username = cx.new(|cx| {
            let mut field = InputState::new(window, cx);
            field.set_value(
                proxy.and_then(proxy_username).unwrap_or_default(),
                window,
                cx,
            );
            field
        });
        let mut editor = Self {
            kind: proxy.map(|proxy| proxy.kind),
            host,
            port,
            authenticated: proxy.and_then(proxy_username).is_some(),
            username,
            expanded: proxy.is_some(),
        };
        editor.refresh_locale(window, cx);
        editor
    }

    pub(crate) fn signature(&self, cx: &App) -> Vec<String> {
        vec![
            self.kind.map(proxy_kind).unwrap_or_default().into(),
            self.host.read(cx).value().to_string(),
            self.port.read(cx).value().to_string(),
            self.authenticated.to_string(),
            self.username.read(cx).value().to_string(),
        ]
    }

    pub(crate) fn draft(&self, cx: &App) -> Result<Option<ConnectionProxy>, Message> {
        let Some(kind) = self.kind else {
            return Ok(None);
        };
        let port = self.port.read(cx).value().parse::<u16>().map_err(|_| {
            Message::new(
                "代理端口须为 1 至 65535 的整数",
                "Proxy port must be a number from 1 to 65535",
            )
        })?;
        let proxy = ConnectionProxy {
            kind,
            host: self.host.read(cx).value().to_string(),
            port,
            auth: if self.authenticated {
                ProxyAuthentication::UsernamePassword {
                    username: self.username.read(cx).value().to_string(),
                }
            } else {
                ProxyAuthentication::None
            },
        };
        if !proxy.host.is_ascii() {
            return Err(Message::new(
                "代理主机须为 ASCII 域名或 IP；国际域名请使用 punycode 格式。",
                "Proxy hosts require an ASCII name or IP; use punycode for internationalized domains.",
            ));
        }
        proxy.validate().map_err(|error| {
            Message::detail("代理配置无效", "Invalid proxy configuration", error)
        })?;
        Ok(Some(proxy))
    }

    pub(crate) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (field, zh, en) in [
            (&self.host, "代理主机或 IP", "Proxy host or IP"),
            (&self.port, "代理端口", "Proxy port"),
            (&self.username, "代理用户名", "Proxy username"),
        ] {
            field.update(cx, |input, cx| {
                input.set_placeholder(t(cx, zh, en), window, cx)
            });
        }
        cx.notify();
    }
}

impl Render for ProxyEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = self
            .kind
            .map(|kind| {
                format!(
                    "{} · {}:{}",
                    proxy_kind(kind),
                    self.host.read(cx).value(),
                    self.port.read(cx).value()
                )
            })
            .unwrap_or_else(|| t(cx, "不使用代理", "No proxy").into());
        let field = |id: &'static str, label: &str, state: &Entity<InputState>| {
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(label.to_owned()),
                )
                .child(Input::new(state).id(id))
        };
        div().id("proxy-editor").flex_shrink_0().test_support().min_w_0().p_3().rounded(px(8.)).bg(rgb(CANVAS)).border_1().border_color(rgb(BORDER)).flex().flex_col().gap_2()
            .child(div().flex().items_center().gap_2()
                .child(div().min_w_0().flex_1().flex().flex_col().gap_1().child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx,"网络代理","Network proxy"))).child(div().text_xs().text_color(rgb(MUTED)).truncate().child(summary)))
                .child(Button::new("toggle-proxy-editor").ghost().compact().label(if self.expanded { t(cx,"收起","Collapse") } else { t(cx,"设置","Configure") }).on_click(cx.listener(|editor,_,_,cx|{editor.expanded = !editor.expanded;cx.notify();}))))
            .when(self.expanded, |body| body
                .child(div().flex().flex_wrap().gap_2().children([(None,"proxy-off",t(cx,"不使用","None")),(Some(ProxyKind::Socks5),"proxy-socks5","SOCKS5"),(Some(ProxyKind::HttpConnect),"proxy-http","HTTP CONNECT")].into_iter().map(|(kind,id,label)| Button::new(id).ghost().compact().label(if self.kind==kind { format!("● {label}") } else { label.into() }).on_click(cx.listener(move |editor,_,_,cx|{editor.kind=kind;cx.notify();})))))
                .when(self.kind.is_some(), |body| body
                    .child(div().flex().gap_2().child(div().flex_1().min_w_0().child(field("proxy-host",t(cx,"主机","Host"),&self.host))).child(div().w(px(92.)).flex_shrink_0().child(field("proxy-port",t(cx,"端口","Port"),&self.port))))
                    .child(Button::new("proxy-auth-mode").ghost().compact().label(if self.authenticated {t(cx,"认证：用户名与密码","Authentication: username and password")} else {t(cx,"认证：无（点击启用）","Authentication: none (click to enable)")}).on_click(cx.listener(|editor,_,_,cx|{editor.authenticated = !editor.authenticated;cx.notify();})))
                    .when(self.authenticated, |body| body.child(field("proxy-username",t(cx,"代理用户名","Proxy username"),&self.username)))
                    .child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,"代理密码仅在连接时输入，不保存在配置中。代理认证本身未加密，请使用可信网络。","Proxy passwords are requested when connecting and are not saved in profiles. Proxy authentication is not encrypted; use a trusted network.")))
                    .child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,"已有跳板时，经上一跳访问此代理；目标域名由代理解析。","With a jump host, this proxy is reached through the previous hop. The proxy resolves the destination name.")))))
    }
}

#[cfg(test)]
mod tests;
