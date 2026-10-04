//! Actual GPUI layout regression for the production confirmation component.
use super::confirmation_bar;
use crate::i18n::t;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    Styled, TestAppContext, TestSupportExt, Window, WindowBounds, WindowOptions,
    component::button::{Button, ButtonVariants},
    div, point, px, rgb, size,
};

struct ConfirmationFixture {
    width: f32,
    message: String,
    confirmations: usize,
    cancellations: usize,
}
impl Render for ConfirmationFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        div().size_full().flex().items_end().p_4().child(
            div()
                .id("file-panel-fixture")
                .w(px(self.width))
                .h(px(260.))
                .min_w_0()
                .flex()
                .flex_col()
                .bg(rgb(visual.surface))
                .child(div().flex_1().min_h_0())
                .child(confirmation_bar(
                    cx,
                    self.message.clone(),
                    Button::new("confirm-file-operation")
                        .primary()
                        .compact()
                        .label(t(cx, "确认", "Confirm"))
                        .on_click(cx.listener(|view, _, _, _| view.confirmations += 1)),
                    Button::new("cancel-file-operation")
                        .ghost()
                        .compact()
                        .label(t(cx, "取消", "Cancel"))
                        .on_click(cx.listener(|view, _, _, _| view.cancellations += 1)),
                ))
                .test_support(),
        )
    }
}

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
fn long_confirmation_keeps_both_actions_inside_the_file_panel(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let path = format!(
        "/fixture/{}/complete-target",
        "long-directory-segment/".repeat(150)
    );
    let message = format!(
        "Source {path} → destination {path}; 10000 entries, 16 GiB, 32 levels. Cancelled transfers may retain partial files. Review every path before confirmation."
    );
    let message = format!(
        "{message}{}",
        "\nAdditional reviewed directory entry.".repeat(40)
    );
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1440.), px(900.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| ConfirmationFixture {
                    width: 1408.,
                    message,
                    confirmations: 0,
                    cancellations: 0,
                })
            },
        )
        .unwrap_or_else(|error| panic!("confirmation test window: {error}"))
    });
    for width in [1408., 760., 480.] {
        for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
            cx.update_window(window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                panel.update(cx, |panel, cx| {
                    panel.width = width;
                    cx.notify();
                });
                window.render_frame(cx);
                let area = window.find("file-panel-fixture").bounds();
                let bar = window.find("file-confirmation-bar").bounds();
                let text = window.find("file-confirmation-message").bounds();
                assert!(
                    text.size.height <= px(96.),
                    "review text must remain scrollable: {text:?}"
                );
                assert!(contained(text, bar), "review viewport escaped its bar");
                assert!(
                    contained(bar, area),
                    "confirmation escaped {width}px panel: {bar:?} vs {area:?}"
                );
                for id in ["confirm-file-operation", "cancel-file-operation"] {
                    let button = window.find(id);
                    assert!(
                        contained(button.bounds(), area),
                        "{id} escaped {width}px file panel: {:?} vs {area:?}",
                        button.bounds()
                    );
                    assert!(button.visible());
                    assert!(
                        contained(button.bounds(), window.bounds()),
                        "{id} escaped the native window"
                    );
                    assert!(
                        button.bounds().size.width > px(20.)
                            && button.bounds().size.height > px(15.),
                        "action collapsed: {:?}",
                        button.bounds()
                    );
                    assert!(
                        button.bounds().origin.x >= text.right(),
                        "text overlaps {id}"
                    );
                    window.click(id, cx);
                }
            })
            .unwrap_or_else(|error| panic!("check confirmation bounds: {error}"));
        }
    }
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.confirmations, 6);
        assert_eq!(panel.cancellations, 6);
    });
}
