//! Real rendering and input behavior; parameter review never creates a transport.
use super::{SnippetParameters, SnippetParametersEvent};
use crate::i18n::{Message, set_language};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Focusable, ScrollDelta, Subscription,
    TestAppContext, WindowBounds, WindowOptions, point, px, size,
};
use keelshell_core::{Language, Snippet};
use std::{cell::RefCell, rc::Rc};

trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}
#[derive(Default)]
struct Events {
    rendered: Vec<(Snippet, String)>,
    cancelled: usize,
}
struct Harness {
    window: AnyWindowHandle,
    panel: Entity<SnippetParameters>,
    events: Rc<RefCell<Events>>,
    _subscription: Subscription,
}
fn snippet(command: &str) -> Snippet {
    let mut snippet = Snippet::new("模板 / Template", command);
    snippet.parameterized = true;
    snippet
}
fn mount(cx: &mut TestAppContext, snippet: Snippet) -> Harness {
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_language(Language::ZhCn, cx);
        let (window, panel) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(820.), px(640.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    SnippetParameters::new(
                        snippet,
                        "生产 · operator@ssh.example.test:22".into(),
                        window,
                        cx,
                    )
                })
            },
        )
        .checked("mount production parameter panel");
        let events = Rc::new(RefCell::new(Events::default()));
        let received = events.clone();
        let subscription = cx.subscribe(&panel, move |_, event, _| match event {
            SnippetParametersEvent::Rendered { snippet, text } => received
                .borrow_mut()
                .rendered
                .push((snippet.clone(), text.clone())),
            SnippetParametersEvent::Cancel => received.borrow_mut().cancelled += 1,
        });
        Harness {
            window,
            panel,
            events,
            _subscription: subscription,
        }
    })
}
fn fill(h: &Harness, index: usize, value: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("snippet-parameter-input", index), cx);
        window.input(value, cx);
    })
    .checked("type into actual parameter textarea");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn quotes_unicode_and_multiline_preview_emit_exact_snapshot_only_after_review(
    cx: &mut TestAppContext,
) {
    let source = snippet("printf '%s\\n' {{path}}\nprintf '%s' {{path}} --label={{label}}");
    let h = mount(cx, source.clone());
    fill(&h, 0, "中文'$(printf unsafe)\nnext", cx);
    fill(&h, 1, "--help", cx);
    let quoted = "'中文'\\''$(printf unsafe)\nnext'";
    let expected = format!("printf '%s\\n' {quoted}\nprintf '%s' {quoted} --label='--help'");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.preview.read(cx).value().as_str(), expected);
        assert_eq!(panel.rendered.as_deref(), Some(expected.as_str()));
        assert_eq!(panel.snippet, source);
    });
    assert!(h.events.borrow().rendered.is_empty());
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-parameters-preview", cx);
        window.input("must-not-change-preview", cx);
        window.press("backspace", cx);
        assert_eq!(h.panel.read(cx).preview.read(cx).value().as_str(), expected);
        window.click("snippet-parameters-insert", cx);
    })
    .checked("explicit final insertion request");
    cx.run_until_parked();
    assert_eq!(h.events.borrow().rendered, vec![(source, expected)]);
    assert!(h.panel.read_with(cx, |panel, _| panel.is_saving()));
}

#[gpui_kit::test]
fn missing_and_explicit_empty_are_distinct_and_current_inputs_override_stale_preview(
    cx: &mut TestAppContext,
) {
    let h = mount(cx, snippet("printf '%s' {{value}}"));
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-parameters-insert", cx);
        assert!(h.panel.read(cx).rendered.is_none());
        window.click(("snippet-parameter-empty", 0_usize), cx);
        assert_eq!(h.panel.read(cx).rendered.as_deref(), Some("printf '%s' ''"));
        window.click(("snippet-parameter-empty", 0_usize), cx);
        assert!(h.panel.read(cx).rendered.is_none());
    })
    .checked("empty requires an explicit choice");
    fill(&h, 0, "valid", cx);
    cx.update_window(h.window, |_, window, cx| {
        let input = h.panel.read(cx).parameters[0].value.clone();
        input.update(cx, |input, cx| input.set_value("", window, cx));
        // Programmatic mutation deliberately emits no Change. Submit must reread.
        h.panel.update(cx, |panel, cx| panel.submit(window, cx));
        assert!(!h.panel.read(cx).saving);
        assert!(h.panel.read(cx).rendered.is_none());
        window.render_frame(cx);
        window.click(("snippet-parameter-empty", 0_usize), cx);
        window.render_frame(cx);
        window.click("snippet-parameters-insert", cx);
    })
    .checked("stale enabled preview cannot bypass current input validation");
    cx.run_until_parked();
    assert_eq!(h.events.borrow().rendered.len(), 1);
    assert_eq!(h.events.borrow().rendered[0].1, "printf '%s' ''");
}

#[gpui_kit::test]
fn pending_review_freezes_controls_and_failed_review_keeps_values(cx: &mut TestAppContext) {
    let h = mount(cx, snippet("cat {{path}}"));
    fill(&h, 0, "/保留/路径", cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-parameters-insert", cx);
        window.render_frame(cx);
        window.click(("snippet-parameter-input", 0_usize), cx);
        window.input("must-not-edit", cx);
        window.press("backspace", cx);
        window.click("snippet-parameters-cancel", cx);
        window.click("snippet-parameters-insert", cx);
        assert_eq!(
            h.panel.read(cx).parameters[0]
                .value
                .read(cx)
                .value()
                .as_str(),
            "/保留/路径"
        );
        assert!(!h.panel.read(cx).parameters[0].value.read(cx).is_editable());
        h.panel.update(cx, |panel, cx| {
            panel.set_error(
                Message::new(
                    "目标已变化，请保留草稿。",
                    "Target changed; keep these values.",
                ),
                cx,
            )
        });
        assert!(!h.panel.read(cx).saving);
        assert!(h.panel.read(cx).parameters[0].value.read(cx).is_editable());
        window.render_frame(cx);
        window.click("snippet-parameters-cancel", cx);
    })
    .checked("freeze before emit and recover after rejected workspace capability");
    cx.run_until_parked();
    assert_eq!(h.events.borrow().rendered.len(), 1);
    assert_eq!(h.events.borrow().cancelled, 1);
}

#[gpui_kit::test]
fn cancellation_and_enter_never_emit_a_rendered_command(cx: &mut TestAppContext) {
    let h = mount(cx, snippet("cat {{path}}"));
    fill(&h, 0, "/draft", cx);
    cx.update_window(h.window, |_, window, cx| {
        window.press("enter", cx);
        window.input("second line", cx);
        assert!(
            h.panel.read(cx).parameters[0]
                .value
                .read(cx)
                .value()
                .contains('\n')
        );
        window.render_frame(cx);
        window.click("snippet-parameters-cancel", cx);
    })
    .checked("Enter edits parameter text; Cancel only emits cancellation");
    cx.run_until_parked();
    assert!(h.events.borrow().rendered.is_empty());
    assert_eq!(h.events.borrow().cancelled, 1);
}

#[gpui_kit::test]
fn actual_small_windows_scroll_all_parameters_and_keep_bilingual_footer(cx: &mut TestAppContext) {
    let source = snippet(
        &(0..32)
            .map(|index| format!("printf '%s' {{{{p{index}}}}}\n"))
            .collect::<String>(),
    );
    let h = mount(cx, source);
    fill(&h, 0, "首个值不丢失", cx);
    for (width, height) in [(820., 640.), (480., 420.)] {
        for language in [Language::ZhCn, Language::En] {
            cx.simulate_window_resize(h.window, size(px(width), px(height)));
            cx.run_until_parked();
            cx.update_window(h.window, |_, window, cx| {
                assert_eq!(window.viewport_size(), size(px(width), px(height)));
                set_language(language, cx);
                h.panel
                    .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                window.render_frame(cx);
                let panel = window.find("snippet-parameters").bounds();
                let footer = window.find("snippet-parameters-footer").bounds();
                assert!(footer.bottom() <= panel.bottom() && footer.origin.y >= panel.origin.y);
                for id in ["snippet-parameters-cancel", "snippet-parameters-insert"] {
                    let button = window.find(id).bounds();
                    assert!(
                        button.origin.x >= footer.origin.x
                            && button.right() <= footer.right()
                            && button.bottom() <= footer.bottom()
                    );
                }
                assert_eq!(
                    h.panel.read(cx).parameters[0]
                        .value
                        .read(cx)
                        .value()
                        .as_str(),
                    "首个值不丢失"
                );
                let body = window.find("snippet-parameters-body").bounds();
                let last = window.find(("snippet-parameter-input", 31_usize)).bounds();
                window.scroll(
                    "snippet-parameters-body",
                    ScrollDelta::Pixels(point(px(0.), body.origin.y + px(8.) - last.origin.y)),
                    cx,
                );
                window.render_frame(cx);
                let last = window.find(("snippet-parameter-input", 31_usize)).bounds();
                assert!(
                    last.origin.y >= body.origin.y && last.bottom() <= body.bottom(),
                    "last field {last:?} not visible in {body:?}"
                );
                window.click(("snippet-parameter-input", 31_usize), cx);
                window.input("值", cx);
                assert!(
                    h.panel.read(cx).parameters[31]
                        .value
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
            })
            .checked("actual resize, scrolling, footer hit area and focus");
            cx.run_until_parked();
        }
    }
    assert_eq!(
        h.panel.read_with(cx, |panel, cx| panel.parameters[31]
            .value
            .read(cx)
            .value()
            .to_string()),
        "值值值值"
    );
    assert!(h.events.borrow().rendered.is_empty());
}

#[gpui_kit::test]
fn oversized_value_shows_bilingual_error_without_echoing_it_and_no_parameter_template_is_reviewable(
    cx: &mut TestAppContext,
) {
    let h = mount(cx, snippet("cat {{path}}"));
    fill(&h, 0, &"secret-value-sentinel".repeat(250), cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for language in [Language::ZhCn, Language::En] {
            set_language(language, cx);
            h.panel
                .update(cx, |panel, cx| panel.refresh_locale(window, cx));
            window.render_frame(cx);
            let panel = h.panel.read(cx);
            let message = panel
                .validation
                .as_ref()
                .map(|error| error.render(cx))
                .unwrap_or_default();
            assert!(message.contains("4096"));
            assert!(!message.contains("secret-value-sentinel"));
            assert!(panel.rendered.is_none());
        }
    })
    .checked("bounded value diagnostics never echo user values");
    assert!(h.events.borrow().rendered.is_empty());
    let h = mount(cx, snippet("printf 'no parameters'"));
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(h.panel.read(cx).parameters.is_empty());
        window.click("snippet-parameters-insert", cx);
    })
    .checked("explicit template with zero variables");
    cx.run_until_parked();
    assert_eq!(h.events.borrow().rendered[0].1, "printf 'no parameters'");
}
