//! Secret-free, per-profile policy for recovering an established SSH connection.
use super::*;
use keelshell_core::ReconnectPolicy;

pub(super) struct ReconnectEditor {
    automatic: bool,
    attempts: Entity<InputState>,
    initial: Entity<InputState>,
    maximum: Entity<InputState>,
}

impl ReconnectEditor {
    pub(super) fn new(
        policy: ReconnectPolicy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (automatic, attempts, initial, maximum) = match policy {
            ReconnectPolicy::Manual => (false, 3, 2, 30),
            ReconnectPolicy::Automatic {
                max_attempts,
                initial_delay_seconds,
                max_delay_seconds,
            } => (true, max_attempts, initial_delay_seconds, max_delay_seconds),
        };
        Self {
            automatic,
            attempts: input("1–10", &attempts.to_string(), window, cx),
            initial: input("1–60", &initial.to_string(), window, cx),
            maximum: input("1–300", &maximum.to_string(), window, cx),
        }
    }

    pub(super) fn refresh_locale(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    pub(super) fn signature(&self, cx: &App) -> Vec<String> {
        vec![
            self.automatic.to_string(),
            self.attempts.read(cx).value().to_string(),
            self.initial.read(cx).value().to_string(),
            self.maximum.read(cx).value().to_string(),
        ]
    }

    pub(super) fn draft(&self, cx: &App) -> Result<ReconnectPolicy, Message> {
        if !self.automatic {
            return Ok(ReconnectPolicy::Manual);
        }
        let invalid = || {
            Message::new(
                "重连次数须为 1–10，首次等待 1–60 秒，最长等待不小于首次且不超过 300 秒。",
                "Use 1–10 attempts, an initial delay of 1–60 seconds, and a maximum between the initial delay and 300 seconds.",
            )
        };
        let policy = ReconnectPolicy::Automatic {
            max_attempts: self
                .attempts
                .read(cx)
                .value()
                .parse()
                .map_err(|_| invalid())?,
            initial_delay_seconds: self
                .initial
                .read(cx)
                .value()
                .parse()
                .map_err(|_| invalid())?,
            max_delay_seconds: self
                .maximum
                .read(cx)
                .value()
                .parse()
                .map_err(|_| invalid())?,
        };
        policy.validate().map_err(|_| invalid())?;
        Ok(policy)
    }
}

impl Render for ReconnectEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("reconnect-editor").test_support().flex_shrink_0().min_w_0()
            .p_3().rounded_md().border_1().border_color(rgb(BORDER)).flex().flex_col().gap_2()
            .child(Button::new("reconnect-policy").ghost().compact()
                .label(if self.automatic { t(cx, "断线重连：自动", "Reconnect: automatic") }
                    else { t(cx, "断线重连：手动", "Reconnect: manual") })
                .on_click(cx.listener(|view, _, _, cx| { view.automatic = !view.automatic; cx.notify(); })))
            .child(div().text_xs().text_color(rgb(MUTED)).child(t(cx,
                "仅重建 SSH 会话，不重放命令、传输或隧道。需要凭据时等待你处理。",
                "Reopens SSH only. Commands, transfers and tunnels are never replayed. Credentials require your action.")))
            .when(self.automatic, |body| body.child(div().flex().flex_wrap().gap_2()
                .children([
                    ("reconnect-attempts", t(cx, "最多次数", "Attempts"), self.attempts.clone()),
                    ("reconnect-initial", t(cx, "首次等待（秒）", "Initial delay (s)"), self.initial.clone()),
                    ("reconnect-maximum", t(cx, "最长等待（秒）", "Maximum delay (s)"), self.maximum.clone()),
                ].into_iter().map(|(id, label, state)| div().w(px(160.)).min_w_0().flex().flex_col().gap_1()
                    .child(div().text_xs().text_color(rgb(MUTED)).child(label))
                    .child(Input::new(&state).id(id))))))
    }
}

#[cfg(test)]
mod tests {
    use super::ReconnectEditor;
    use crate::i18n;
    use gpui_kit::{
        AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size,
        test::TestWindowExt,
    };
    use keelshell_core::{Language, ReconnectPolicy};

    #[gpui_kit::test]
    fn compact_bilingual_policy_keeps_draft_and_requires_bounded_numbers(cx: &mut TestAppContext) {
        let (window, editor) = cx.update(|cx| {
            gpui_kit::init(cx);
            i18n::set_language(Language::ZhCn, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(360.), px(460.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| ReconnectEditor::new(ReconnectPolicy::Manual, window, cx)),
            )
            .unwrap_or_else(|error| panic!("mount real policy editor: {error:?}"))
        });
        cx.update_window(window, |_, window, cx| {
            assert_eq!(editor.read(cx).draft(cx), Ok(ReconnectPolicy::Manual));
            window.render_frame(cx);
            window.click("reconnect-policy", cx);
            window.render_frame(cx);
            assert_eq!(
                editor.read(cx).draft(cx),
                Ok(ReconnectPolicy::Automatic {
                    max_attempts: 3,
                    initial_delay_seconds: 2,
                    max_delay_seconds: 30
                })
            );
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let panel = window.find("reconnect-editor").bounds();
                let field = window.find("reconnect-maximum").bounds();
                assert!(field.right() <= panel.right() && field.bottom() <= panel.bottom());
            }
            editor.update(cx, |editor, cx| {
                editor
                    .attempts
                    .update(cx, |field, cx| field.set_value("11", window, cx))
            });
            assert!(editor.read(cx).draft(cx).is_err());
            window.click("reconnect-policy", cx);
            assert_eq!(editor.read(cx).draft(cx), Ok(ReconnectPolicy::Manual));
            window.render_frame(cx);
            window.click("reconnect-policy", cx);
            assert_eq!(editor.read(cx).attempts.read(cx).value().as_str(), "11");
            editor.update(cx, |editor, cx| {
                editor
                    .attempts
                    .update(cx, |field, cx| field.set_value("10", window, cx));
                editor
                    .initial
                    .update(cx, |field, cx| field.set_value("60", window, cx));
                editor
                    .maximum
                    .update(cx, |field, cx| field.set_value("59", window, cx));
            });
            assert!(
                editor.read(cx).draft(cx).is_err(),
                "maximum cannot be shorter than the initial delay"
            );
            editor.update(cx, |editor, cx| {
                editor
                    .maximum
                    .update(cx, |field, cx| field.set_value("300", window, cx))
            });
            assert_eq!(
                editor.read(cx).draft(cx),
                Ok(ReconnectPolicy::Automatic {
                    max_attempts: 10,
                    initial_delay_seconds: 60,
                    max_delay_seconds: 300
                })
            );
        })
        .unwrap_or_else(|error| panic!("interact with bilingual bounded policy fields: {error:?}"));
    }
}
