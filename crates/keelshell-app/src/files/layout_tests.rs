//! Actual GPUI layout regression for the production confirmation component.
use super::confirmation_bar;
use crate::i18n::t;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Context, InputEvent, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, TestAppContext, TestSupportExt, Window, WindowBounds, WindowOptions,
    component::button::{Button, ButtonVariants},
    div, point, px, rgb, size,
};

struct ConfirmationFixture {
    width: f32,
    message: String,
    scroll: gpui_kit::ScrollHandle,
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
                    &self.scroll,
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
                    scroll: gpui_kit::ScrollHandle::new(),
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

#[gpui_kit::test]
fn multiline_review_has_a_real_scroll_range_and_wheel_reaches_the_end(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut original_lines = (0..48)
        .map(|n| format!("Item {n}: verify complete source and destination before any writes"))
        .collect::<Vec<_>>();
    original_lines.push(format!("/{}-complete-target", "x".repeat(512)));
    original_lines.push(format!("SHA-256: {}", "a".repeat(64)));
    original_lines.push("END-OF-COMPLETE-REVIEW-中文".into());
    let message = original_lines.join("\n");
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| ConfirmationFixture {
                    width: 868.,
                    message,
                    scroll: gpui_kit::ScrollHandle::new(),
                    confirmations: 0,
                    cancellations: 0,
                })
            },
        )
        .unwrap_or_else(|error| panic!("review scroll fixture: {error}"))
    });
    for (width, height) in [(900., 580.), (1440., 900.)] {
        for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
            for theme in [
                keelshell_core::Theme::System,
                keelshell_core::Theme::Light,
                keelshell_core::Theme::Dark,
            ] {
                cx.update_window(window, |_,window,cx| {
                    window.resize(size(px(width),px(height))); window.bounds_changed(cx);
                    crate::i18n::set_language(language,cx); crate::design::apply(theme,Some(window),cx);
                    panel.update(cx,|p,cx| {p.width=width-32.;p.scroll.set_offset(point(px(0.),px(0.)));cx.notify();});
                    window.render_frame(cx);
                    let body = window.find("file-confirmation-message").bounds();
                    let maximum = panel.read(cx).scroll.max_offset();
                    for (index,text) in original_lines.iter().enumerate() {
                        let row=window.find(("file-confirmation-line",index));
                        assert_eq!(row.label(),Some(text.as_str()),"complete original line is exposed without omission");
                        assert_eq!(row.role(),Some(gpui_kit::accesskit::Role::Label));
                    }
                    assert!(window.find(("file-confirmation-line",0_usize)).visible());
                    assert!(!window.find(("file-confirmation-line",50_usize)).visible(),"tail must actually start outside the clipped review body");
                    assert!(maximum.x>px(500.) && maximum.y>px(400.),"complete unbroken paths and all original lines require real two-axis extent: {maximum:?}");
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(px(0.),px(-100000.))),cx);
                    let bottom=panel.read(cx).scroll.offset();
                    let tail=window.find(("file-confirmation-line",50_usize));
                    assert!(tail.visible() && contained(tail.bounds(),body),"last original line must paint inside the review viewport after a real wheel: {:?} in {body:?}",tail.bounds());
                    let tail_bounds=tail.bounds();
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(px(-100000.),px(0.))),cx);
                    let right=panel.read(cx).scroll.offset();
                    let path=window.find(("file-confirmation-line",48_usize));
                    assert!(right.x<px(-500.) && path.visible() && path.bounds().right()<=body.right(),"unbroken path suffix must enter the real viewport: {:?} in {body:?}, offset={right:?}",path.bounds());
                    assert!(path.bounds().origin.x<body.origin.x,"long path really moved across the clipped horizontal region");
                    window.scroll("file-confirmation-message",gpui_kit::ScrollDelta::Pixels(point(px(100000.),px(0.))),cx);
                    let restored=window.find(("file-confirmation-line",50_usize));
                    assert!(restored.visible() && contained(restored.bounds(),body));
                    for id in ["confirm-file-operation","cancel-file-operation"] {
                        let action=window.find(id);assert!(action.visible() && contained(action.bounds(),window.bounds()));assert!(action.bounds().origin.x>=body.right());
                        window.click(id,cx);
                    }
                    println!("FILE_REVIEW_RENDER_JSON {}",serde_json::json!({"width":width,"height":height,"language":format!("{language:?}"),"theme":format!("{theme:?}"),"rows":original_lines.len(),"max_x":f32::from(maximum.x),"max_y":f32::from(maximum.y),"bottom_y":f32::from(bottom.y),"right_x":f32::from(right.x),"tail_y":f32::from(tail_bounds.origin.y),"body_y":f32::from(body.origin.y),"body_height":f32::from(body.size.height)}));
                }).unwrap_or_else(|error| panic!("production review axes and tail: {error}"));
            }
        }
    }
    panel.read_with(cx, |p, _| {
        assert_eq!(p.confirmations, 12);
        assert_eq!(p.cancellations, 12);
    });
}

#[gpui_kit::test]
fn independent_large_literal_review_uses_reversible_coordinate_wheels(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut lines = (0..256)
        .map(|index| {
            if index % 7 == 0 {
                String::new()
            } else {
                format!(
                    "{index}: /审核/$(touch untouched);quoted-'\"'-{}-尾端",
                    "long-literal-path/".repeat(24)
                )
            }
        })
        .collect::<Vec<_>>();
    lines.push("END 原文保留".into());
    lines.push(String::new());
    let message = lines.join("\n");
    assert!(message.len() > 96 * 1024 && message.len() < 128 * 1024);
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| ConfirmationFixture {
                    width: 480.,
                    message,
                    scroll: gpui_kit::ScrollHandle::new(),
                    confirmations: 0,
                    cancellations: 0,
                })
            },
        )
        .unwrap_or_else(|error| panic!("independent large literal review: {error}"))
    });
    for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
        cx.update_window(window, |_, window, cx| {
            crate::i18n::set_language(language, cx);
            panel.update(cx, |p, cx| {
                p.scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            });
            window.render_frame(cx);
            let body = window.find("file-confirmation-message").bounds();
            assert!(body.size.height <= px(72.));
            let action_bounds = ["confirm-file-operation", "cancel-file-operation"]
                .map(|id| window.find(id).bounds());
            for (index, text) in lines.iter().enumerate() {
                let line = window.find(("file-confirmation-line", index));
                assert_eq!(line.label(), Some(text.as_str()));
                assert_eq!(line.role(), Some(gpui_kit::accesskit::Role::Label));
                assert!(line.bounds().size.height >= px(18.));
            }
            let wheel = |window: &mut Window, delta, cx: &mut gpui_kit::App| {
                // Window-local coordinates exercise the same pointer hit test as
                // a platform event without mutating the ScrollHandle in the test.
                let position = point(body.origin.x + px(8.), body.center().y);
                window.dispatch_event(
                    gpui_kit::MouseMoveEvent {
                        position,
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.dispatch_event(
                    gpui_kit::ScrollWheelEvent {
                        position,
                        delta: gpui_kit::ScrollDelta::Pixels(delta),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.render_frame(cx);
            };
            wheel(window, point(px(-60.), px(0.)), cx);
            wheel(window, point(px(0.), px(-36.)), cx);
            let gradual = panel.read(cx).scroll.offset();
            assert!(gradual.x < px(0.) && gradual.y < px(0.));
            wheel(window, point(px(-100_000.), px(0.)), cx);
            wheel(window, point(px(0.), px(-100_000.)), cx);
            let end = panel.read(cx).scroll.offset();
            assert!(end.x < px(-500.) && end.y < px(-3000.));
            wheel(window, point(px(100_000.), px(0.)), cx);
            let tail = window.find(("file-confirmation-line", lines.len() - 2));
            assert!(tail.visible() && contained(tail.bounds(), body));
            panel.update(cx, |_, cx| cx.notify());
            window.render_frame(cx);
            assert_eq!(panel.read(cx).scroll.offset().y, end.y);
            wheel(window, point(px(0.), px(100_000.)), cx);
            assert_eq!(panel.read(cx).scroll.offset(), point(px(0.), px(0.)));
            for (index, id) in ["confirm-file-operation", "cancel-file-operation"]
                .into_iter()
                .enumerate()
            {
                let action = window.find(id);
                assert_eq!(action.bounds(), action_bounds[index]);
                assert!(action.visible() && contained(action.bounds(), window.bounds()));
                window.click(id, cx);
            }
            println!(
                "INDEPENDENT_LITERAL_REVIEW_JSON {}",
                serde_json::json!({"language":format!("{language:?}"),"rows":lines.len(),"body_height":f32::from(body.size.height),"gradual_x":f32::from(gradual.x),"gradual_y":f32::from(gradual.y),"end_x":f32::from(end.x),"end_y":f32::from(end.y)})
            );
        })
        .unwrap_or_else(|error| panic!("independent reversible coordinate wheels: {error}"));
    }
    panel.read_with(cx, |p, _| {
        assert_eq!(p.confirmations, 2);
        assert_eq!(p.cancellations, 2);
    });
}
