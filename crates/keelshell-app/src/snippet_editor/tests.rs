//! Exercise production fields and buttons through the GPUI input/render paths.

use std::{cell::RefCell, rc::Rc};

use super::{SnippetEditor, SnippetEditorEvent, format_tags, parse_tags};
use crate::i18n::{Message, set_language};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Styled, Subscription, TestAppContext,
    TestSupportExt, Window, WindowBounds, WindowOptions, div, point, px, size, test::TestWindowExt,
};
use keelshell_core::{Language, Snippet};

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
    saved: Vec<Snippet>,
    cancelled: usize,
}

struct FixtureView {
    editor: Entity<SnippetEditor>,
    width: f32,
    height: f32,
}
impl Render for FixtureView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("snippet-fixture")
                .test_support()
                .w(px(self.width))
                .h(px(self.height))
                .child(self.editor.clone()),
        )
    }
}

struct Fixture {
    window: AnyWindowHandle,
    view: Entity<FixtureView>,
    editor: Entity<SnippetEditor>,
    events: Rc<RefCell<Events>>,
    _subscription: Subscription,
}

fn mount(cx: &mut TestAppContext, snippet: Option<Snippet>) -> Fixture {
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_language(Language::ZhCn, cx);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1000.), px(760.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let editor = cx.new(|cx| SnippetEditor::new(snippet, window, cx));
                cx.new(|_| FixtureView {
                    editor,
                    width: 900.,
                    height: 560.,
                })
            },
        )
        .checked("mount snippet editor");
        let editor = view.read(cx).editor.clone();
        let events = Rc::new(RefCell::new(Events::default()));
        let received = Rc::clone(&events);
        let subscription = cx.subscribe(&editor, move |_, event, _| match event {
            SnippetEditorEvent::Save(snippet) => received.borrow_mut().saved.push(snippet.clone()),
            SnippetEditorEvent::Cancel => received.borrow_mut().cancelled += 1,
        });
        Fixture {
            window,
            view,
            editor,
            events,
            _subscription: subscription,
        }
    })
}

fn sample() -> Snippet {
    let mut snippet = Snippet::new("检查 / Check", "  printf '状态\\n'\n\tprintf 'done'\n");
    snippet.description = "运行前审核路径 / Review paths first".into();
    snippet.tags = vec!["诊断".into(), "disk,space".into(), "quote\"tag".into()];
    snippet
}

#[test]
fn csv_tags_round_trip_existing_commas_quotes_and_unicode() {
    let tags = sample().tags;
    assert_eq!(
        parse_tags(&format_tags(&tags)).checked("CSV round trip"),
        tags
    );
    assert_eq!(
        parse_tags(" , 诊断, disk, ").checked("optional empty separators"),
        vec!["诊断", "disk"]
    );
    for invalid in [
        "\"unfinished",
        "\"closed\"suffix",
        "unquoted\"quote",
        "\"a\",\"b",
    ] {
        assert!(
            parse_tags(invalid).is_err(),
            "accepted malformed CSV {invalid:?}"
        );
    }
    assert!(parse_tags(&vec!["tag"; 33].join(",")).is_err());
}

#[gpui_kit::test]
fn new_draft_is_blank_and_validation_keeps_focus_inside_the_editor(cx: &mut TestAppContext) {
    let fixture = mount(cx, None);
    cx.update_window(fixture.window, |_, window, cx| {
        let editor = fixture.editor.read(cx);
        assert!(!editor.id.is_nil());
        assert!(editor.name.read(cx).value().is_empty());
        assert!(editor.description.read(cx).value().is_empty());
        assert!(editor.tags.read(cx).value().is_empty());
        assert!(editor.command.read(cx).value().is_empty());
        assert!(editor.name.read(cx).focus_handle(cx).is_focused(window));
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        assert!(!fixture.editor.read(cx).is_saving());
        assert!(fixture.editor.read(cx).error.is_some());
        assert!(
            fixture
                .editor
                .read(cx)
                .name
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("状态检查", cx);
        window.click("snippet-editor-save", cx);
        assert!(
            fixture
                .editor
                .read(cx)
                .command
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .checked("blank validation focuses its invalid field");
    cx.run_until_parked();
    assert!(fixture.events.borrow().saved.is_empty());
}

#[gpui_kit::test]
fn locale_change_preserves_identity_command_whitespace_and_current_focus(cx: &mut TestAppContext) {
    let original = sample();
    let fixture = mount(cx, Some(original.clone()));
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .editor
            .read(cx)
            .command
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        for language in [Language::En, Language::ZhCn] {
            set_language(language, cx);
            fixture
                .editor
                .update(cx, |editor, cx| editor.refresh_locale(window, cx));
            window.render_frame(cx);
            assert_eq!(
                fixture.editor.read(cx).draft(cx).checked("localized draft"),
                original
            );
            assert!(
                fixture
                    .editor
                    .read(cx)
                    .command
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            assert_eq!(
                window.find("snippet-editor-save").label(),
                Some(if language == Language::En {
                    "Save snippet"
                } else {
                    "保存片段"
                })
            );
        }
        window.press("enter", cx);
        assert!(!fixture.editor.read(cx).saving);
        assert!(
            fixture
                .editor
                .read(cx)
                .command
                .read(cx)
                .value()
                .contains('\n')
        );
    })
    .checked("locale and command input");
    cx.run_until_parked();
    assert!(
        fixture.events.borrow().saved.is_empty(),
        "command Enter must not save or run a snippet"
    );
}

#[gpui_kit::test]
fn save_freezes_exact_snapshot_and_failure_restores_the_draft(cx: &mut TestAppContext) {
    let original = sample();
    let fixture = mount(cx, Some(original.clone()));
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        assert!(fixture.editor.read(cx).is_saving());
        window.click("snippet-editor-cancel", cx);
        window.click("snippet-editor-save", cx);
        window.click("snippet-name", cx);
        window.input("must-not-edit", cx);
        window.press("backspace", cx);
        assert_eq!(
            fixture.editor.read(cx).draft(cx).checked("frozen draft"),
            original
        );
        for field in [
            &fixture.editor.read(cx).name,
            &fixture.editor.read(cx).description,
            &fixture.editor.read(cx).tags,
        ] {
            assert!(!field.read(cx).is_editable());
        }
        assert!(!fixture.editor.read(cx).command.read(cx).is_editable());
        fixture.editor.update(cx, |editor, cx| {
            editor.cancel(cx);
            editor.set_error(
                Message::new(
                    "磁盘状态已更改，请保留草稿后重试。",
                    "Disk state changed. Keep the draft and retry.",
                ),
                cx,
            );
        });
        window.render_frame(cx);
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .draft(cx)
                .checked("retained failed draft"),
            original
        );
        assert!(!fixture.editor.read(cx).is_saving());
        window.click("snippet-name", cx);
        window.input("追加", cx);
        assert!(
            fixture
                .editor
                .read(cx)
                .name
                .read(cx)
                .value()
                .contains("追加")
        );
    })
    .checked("save freeze and failure recovery");
    cx.run_until_parked();
    assert_eq!(fixture.events.borrow().saved, vec![original]);
    assert_eq!(fixture.events.borrow().cancelled, 0);
}

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
fn long_drafts_and_errors_keep_actions_visible_in_both_languages(cx: &mut TestAppContext) {
    let mut snippet = sample();
    snippet.name = "长".repeat(120);
    snippet.description = "说".repeat(2048);
    snippet.command = "printf 'diagnostic status'\n".repeat(1500);
    let fixture = mount(cx, Some(snippet));
    for (width, height) in [(900., 560.), (640., 480.), (480., 420.)] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(fixture.window, |_, window, cx| {
                set_language(language, cx);
                fixture.view.update(cx, |view, cx| {
                    view.width = width;
                    view.height = height;
                    cx.notify();
                });
                fixture.editor.update(cx, |editor, cx| {
                    editor.refresh_locale(window, cx);
                    editor.set_error(
                        Message::new(
                            "可恢复错误。".repeat(100),
                            "Recoverable failure. ".repeat(100),
                        ),
                        cx,
                    );
                });
                window.render_frame(cx);
                let area = window.find("snippet-fixture").bounds();
                let footer = window.find("snippet-editor-footer").bounds();
                assert!(
                    contained(footer, area),
                    "footer escaped {width}x{height}: {footer:?} vs {area:?}"
                );
                assert!(window.find("snippet-editor-error").bounds().size.height <= px(64.));
                assert_eq!(
                    window
                        .find("snippet-command-container")
                        .bounds()
                        .size
                        .height,
                    px(176.)
                );
                for id in ["snippet-editor-cancel", "snippet-editor-save"] {
                    let button = window.find(id);
                    assert!(button.visible());
                    assert!(contained(button.bounds(), footer), "{id} escaped footer");
                    assert!(contained(button.bounds(), area), "{id} escaped modal");
                    assert!(button.bounds().size.width > px(20.));
                    assert!(button.bounds().size.height > px(15.));
                }
                window.click("snippet-editor-cancel", cx);
            })
            .checked("small bilingual editor bounds and action hit testing");
        }
    }
    cx.run_until_parked();
    assert_eq!(fixture.events.borrow().cancelled, 6);
    assert!(fixture.events.borrow().saved.is_empty());
}

#[gpui_kit::test]
fn invalid_fields_remain_unmodified_and_never_emit_a_save(cx: &mut TestAppContext) {
    let fixture = mount(cx, Some(sample()));
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.editor.update(cx, |editor, cx| {
            editor.name.update(cx, |field, cx| {
                field.set_value(" invalid whitespace ", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        assert_eq!(
            fixture.editor.read(cx).name.read(cx).value().as_ref(),
            " invalid whitespace "
        );
        fixture.editor.update(cx, |editor, cx| {
            editor
                .name
                .update(cx, |field, cx| field.set_value("valid", window, cx));
            editor.command.update(cx, |field, cx| {
                field.set_value("printf '\u{1b}[2J'", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("snippet-editor-save", cx);
        assert!(
            fixture
                .editor
                .read(cx)
                .command
                .read(cx)
                .value()
                .contains('\u{1b}')
        );
        assert!(!fixture.editor.read(cx).saving);
    })
    .checked("reject malformed text without rewriting it");
    cx.run_until_parked();
    assert!(fixture.events.borrow().saved.is_empty());
}

#[gpui_kit::test]
fn parameter_mode_is_explicit_and_invalid_quoted_markers_never_save(cx: &mut TestAppContext) {
    let original = Snippet::new("literal braces", "printf '{{literal}}'");
    let fixture = mount(cx, Some(original.clone()));
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .draft(cx)
                .checked("legacy literal source"),
            original
        );
        assert!(!fixture.editor.read(cx).parameterized);
        window.click("snippet-parameters-toggle", cx);
        window.render_frame(cx);
        assert!(fixture.editor.read(cx).parameterized);
        assert!(
            fixture
                .editor
                .read(cx)
                .template_feedback
                .as_ref()
                .is_some_and(|feedback| feedback.error.is_some())
        );
        window.click("snippet-editor-save", cx);
        assert!(!fixture.editor.read(cx).saving);
        assert!(fixture.editor.read(cx).error.is_some());
        fixture
            .editor
            .update(cx, |editor, cx| editor.toggle_parameters(cx));
        window.render_frame(cx);
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .draft(cx)
                .checked("turning off preserves original source"),
            original
        );
        window.click("snippet-editor-save", cx);
    })
    .checked("explicit opt in and fail closed syntax validation");
    cx.run_until_parked();
    assert_eq!(fixture.events.borrow().saved, vec![original]);
}

#[gpui_kit::test]
fn valid_template_lists_unique_names_and_freezes_parameter_mode_with_snapshot(
    cx: &mut TestAppContext,
) {
    let mut source = Snippet::new(
        "变量片段",
        "cat {{path}}\nprintf '%s' {{path}} --label={{label}}",
    );
    source.parameterized = true;
    let fixture = mount(cx, Some(source.clone()));
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .template_feedback
                .as_ref()
                .map(|feedback| feedback.variables.clone()),
            Some(vec!["path".into(), "label".into()])
        );
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .draft(cx)
                .checked("enabled template round trip"),
            source
        );
        window.click("snippet-editor-save", cx);
        fixture
            .editor
            .update(cx, |editor, cx| editor.toggle_parameters(cx));
        assert!(fixture.editor.read(cx).parameterized);
        assert!(fixture.editor.read(cx).saving);
        fixture.editor.update(cx, |editor, cx| {
            editor.set_error(
                Message::new("保存冲突，保留模板", "Save conflict; keep template"),
                cx,
            )
        });
        assert_eq!(
            fixture
                .editor
                .read(cx)
                .draft(cx)
                .checked("template retained after failure"),
            source
        );
    })
    .checked("inferred parameters and immutable saving snapshot");
    cx.run_until_parked();
    assert_eq!(fixture.events.borrow().saved, vec![source]);
}

#[gpui_kit::test]
fn actual_small_editor_window_keeps_template_feedback_and_footer_inside_modal(
    cx: &mut TestAppContext,
) {
    let mut source = Snippet::new(
        "多参数模板",
        (0..32)
            .map(|index| format!("printf {{{{parameter_{index}}}}}\n"))
            .collect::<String>(),
    );
    source.parameterized = true;
    let fixture = mount(cx, Some(source.clone()));
    for (width, height) in [(900., 560.), (480., 420.)] {
        for language in [Language::ZhCn, Language::En] {
            cx.simulate_window_resize(fixture.window, size(px(width), px(height)));
            cx.run_until_parked();
            cx.update_window(fixture.window, |_, window, cx| {
                assert_eq!(window.viewport_size(), size(px(width), px(height)));
                fixture.view.update(cx, |view, cx| {
                    view.width = width;
                    view.height = height;
                    cx.notify();
                });
                set_language(language, cx);
                fixture
                    .editor
                    .update(cx, |editor, cx| editor.refresh_locale(window, cx));
                window.render_frame(cx);
                let modal = window.find("snippet-editor").bounds();
                let footer = window.find("snippet-editor-footer").bounds();
                assert!(contained(footer, modal));
                assert!(contained(
                    window.find("snippet-editor-save").bounds(),
                    footer
                ));
                assert!(
                    window
                        .find("snippet-template-feedback")
                        .bounds()
                        .size
                        .height
                        <= px(72.)
                );
                assert_eq!(
                    fixture
                        .editor
                        .read(cx)
                        .draft(cx)
                        .checked("locale and actual size retain template"),
                    source
                );
            })
            .checked("actual native resize keeps template footer visible");
        }
    }
}
