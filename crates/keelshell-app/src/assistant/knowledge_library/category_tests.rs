#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{KnowledgeLibrary, LibraryEvent, Phase, categories::CategoryFilter};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, InputEvent as _, MouseButton, MouseDownEvent,
    MouseUpEvent, ScrollDelta, TestAppContext, WindowBounds, WindowOptions, point, px, size,
    test::TestWindowExt,
};
use keelshell_core::{AppState, Language, Settings, Theme, ThemeId};
use std::{cell::RefCell, rc::Rc};

fn mount(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<KnowledgeLibrary>, AppState) {
    cx.update(gpui_kit::init);
    let state = AppState {
        knowledge_entries: [
            ("First Linux", "Alpha note", Some("Ops")),
            ("Second Linux", "Beta note", Some("ops")),
            ("Third BSD", "Alpha note", None),
            ("Fourth Linux", "Alpha note", Some("Ops")),
        ]
        .into_iter()
        .map(|(title, text, category)| {
            super::tests::entry(title, text)
                .with_reviewed_category(category)
                .unwrap()
        })
        .collect(),
        ..Default::default()
    };
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(380.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| KnowledgeLibrary::new(window, cx)),
        )
        .unwrap()
    });
    panel.update(cx, |view, cx| view.set_snapshot(&state, cx));
    (window, panel, state)
}

fn edit_category(
    window: AnyWindowHandle,
    panel: &Entity<KnowledgeLibrary>,
    target: &keelshell_core::SavedKnowledgeEntry,
    value: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, cx| {
            view.open_category(target, window, cx);
            view.set_category_draft_for_test(value, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn knowledge_category_counts_and_text_intersection_preserve_all_three_selections(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, cx| {
            view.select(&state.knowledge_entries[2], window, cx);
            view.toggle_context(&state.knowledge_entries[2], cx);
            view.toggle_management(&state.knowledge_entries[0], view.management_revision, cx);
            let revision = view.context_revision();
            let (uncategorized, counts) = view.category_counts();
            assert_eq!(uncategorized, 1);
            assert_eq!(
                counts.into_iter().collect::<Vec<_>>(),
                vec![("Ops", 2), ("ops", 1)]
            );
            view.query
                .update(cx, |input, cx| input.set_value("ALPHA", window, cx));
            view.set_category_filter(CategoryFilter::Named("Ops".into()), cx);
            assert_eq!(
                view.matching_entries(cx).unwrap(),
                vec![
                    state.knowledge_entries[0].clone(),
                    state.knowledge_entries[3].clone()
                ]
            );
            view.set_category_filter(CategoryFilter::Named("ops".into()), cx);
            assert!(view.matching_entries(cx).unwrap().is_empty());
            view.set_category_filter(CategoryFilter::Uncategorized, cx);
            assert_eq!(
                view.matching_entries(cx).unwrap(),
                state.knowledge_entries[2..3]
            );
            assert_eq!(view.selected.as_ref(), Some(&state.knowledge_entries[2]));
            assert_eq!(view.context.entries(), &state.knowledge_entries[2..3]);
            assert_eq!(view.context_revision(), revision);
            assert_eq!(view.management, state.knowledge_entries[..1]);
            assert_eq!(
                view.category_counts().1["Ops"],
                2,
                "counts use the full library"
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_category_review_is_separate_from_content_draft_management_and_ask(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, cx| {
            let target = &state.knowledge_entries[0];
            view.select(target, window, cx);
            view.set_draft_for_test("Unsaved title", "Unsaved text", window, cx);
            view.toggle_context(&state.knowledge_entries[2], cx);
            view.toggle_management(&state.knowledge_entries[1], view.management_revision, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    edit_category(window, &panel, &state.knowledge_entries[0], "Network", cx);
    panel.update(cx, |view, cx| {
        view.prepare_category(&state.knowledge_entries[0], cx);
        let id = view.review.as_ref().unwrap().id;
        let candidate = view.candidate(id, cx).unwrap();
        let replacement = candidate.replacement.unwrap();
        assert_eq!(replacement.category(), Some("Network"));
        assert_eq!(replacement.entry(), state.knowledge_entries[0].entry());
        assert_eq!(view.title.read(cx).value().as_ref(), "Unsaved title");
        assert_eq!(view.summary.read(cx).value().as_ref(), "Unsaved text");
        assert_eq!(view.management, state.knowledge_entries[1..2]);
        assert_eq!(view.context.entries(), &state.knowledge_entries[2..3]);
        view.cancel_review(id, cx);
        assert!(view.review.is_none());
        assert_eq!(view.entries, state.knowledge_entries);
        assert_eq!(
            view.category_target.as_ref(),
            Some(&state.knowledge_entries[0])
        );
    });
}

#[gpui_kit::test]
fn knowledge_category_draft_revokes_old_ask_and_aba_review_without_granting_new_selection(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    panel.update(cx, |view, cx| {
        view.toggle_context(&state.knowledge_entries[0], cx)
    });
    let revision = panel.read_with(cx, |view, _| view.context_revision());
    edit_category(window, &panel, &state.knowledge_entries[0], "New class", cx);
    let old = panel.update(cx, |view, cx| {
        assert!(view.context.entries().is_empty());
        assert_ne!(view.context_revision(), revision);
        view.toggle_context(&state.knowledge_entries[0], cx);
        assert!(view.context.entries().is_empty());
        view.prepare_category(&state.knowledge_entries[0], cx);
        view.review.as_ref().unwrap().id
    });
    for value in ["Another class", "New class"] {
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |view, cx| {
                view.category_input.update(cx, |input, cx| {
                    input.set_value(value, window, cx);
                    cx.emit(gpui_kit::component::input::InputEvent::Change);
                });
                assert!(view.candidate(old, cx).is_none());
            });
        })
        .unwrap();
        cx.run_until_parked();
    }
    panel.update(cx, |view, cx| {
        assert!(view.candidate(old, cx).is_none());
        view.confirm(old, cx);
        assert!(view.context.entries().is_empty());
        assert_eq!(view.entries, state.knowledge_entries);
    });
}

#[gpui_kit::test]
fn knowledge_category_only_refresh_revokes_old_edit_delete_batch_and_ask(cx: &mut TestAppContext) {
    for kind in 0..3 {
        let (window, panel, mut state) = mount(cx);
        let old = cx
            .update_window(window, |_, window, cx| {
                panel.update(cx, |view, cx| {
                    let target = &state.knowledge_entries[0];
                    view.select(target, window, cx);
                    view.toggle_context(target, cx);
                    view.toggle_management(target, view.management_revision, cx);
                    if kind == 2 {
                        view.prepare_batch_delete(view.management_revision, cx);
                    } else {
                        view.prepare(target, kind == 1, cx);
                    }
                    view.review.as_ref().unwrap().id
                })
            })
            .unwrap();
        panel.update(cx, |view, cx| {
            state.knowledge_entries[0] = state.knowledge_entries[0]
                .with_reviewed_category(Some("Changed"))
                .unwrap();
            view.set_snapshot(&state, cx);
            assert!(view.candidate(old, cx).is_none());
            assert!(view.context.entries().is_empty());
            assert!(view.management.is_empty());
            assert!(view.selected.is_none());
        });
    }
}

#[gpui_kit::test]
fn knowledge_category_unchanged_or_invalid_input_has_no_candidate_and_no_label_in_error(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    for value in [
        "  Ops  ".to_owned(),
        "Personal fixture\tlabel".to_owned(),
        "界".repeat(43),
    ] {
        edit_category(window, &panel, &state.knowledge_entries[0], &value, cx);
        panel.update(cx, |view, cx| {
            view.prepare_category(&state.knowledge_entries[0], cx);
            assert!(view.review.is_none());
            assert!(!view.message.render(cx).contains("Personal fixture"));
            assert_eq!(view.entries, state.knowledge_entries);
        });
    }
}

#[gpui_kit::test]
fn knowledge_category_conflict_unknown_and_reload_failure_cannot_replay(cx: &mut TestAppContext) {
    for error in [
        keelshell_core::Error::Conflict,
        keelshell_core::Error::Durability(std::io::Error::other("owned fixture")),
    ] {
        let (window, panel, state) = mount(cx);
        edit_category(window, &panel, &state.knowledge_entries[0], "", cx);
        panel.update(cx, |view, cx| {
            view.prepare_category(&state.knowledge_entries[0], cx);
            let old = view.review.as_ref().unwrap().id;
            assert!(view.begin_save(old, cx));
            view.save_failed(old, &error, cx);
            assert!(view.candidate(old, cx).is_none());
            let Phase::NeedsReload(token) = view.phase else {
                panic!("reload required")
            };
            assert!(view.begin_reload(token, cx));
            view.reload_failed(token, &error, cx);
            assert!(!view.finish_reload(token, &state, cx));
            let Phase::NeedsReload(next) = view.phase else {
                panic!("reload still required")
            };
            assert!(view.begin_reload(next, cx));
            assert!(view.finish_reload(next, &state, cx));
            assert!(view.category_target.is_none());
            assert!(view.candidate(old, cx).is_none());
            assert!(view.context.entries().is_empty());
        });
    }
}

#[gpui_kit::test]
fn knowledge_category_old_painted_confirm_cannot_approve_replacement_review(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    edit_category(window, &panel, &state.knowledge_entries[0], "New", cx);
    let applied = Rc::new(RefCell::new(0));
    let observed = applied.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if matches!(event, LibraryEvent::Apply(_)) {
                *observed.borrow_mut() += 1;
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, cx| {
            view.prepare_category(&state.knowledge_entries[0], cx)
        });
        window.render_frame(cx);
        let position = window.find("knowledge-library-confirm").bounds().center();
        let newer = panel.update(cx, |view, cx| {
            view.prepare_category(&state.knowledge_entries[0], cx);
            view.review.as_ref().unwrap().id
        });
        for event in [
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
        ] {
            window.dispatch_event(event, cx);
        }
        assert_eq!(panel.read(cx).review.as_ref().unwrap().id, newer);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*applied.borrow(), 0);
}

#[gpui_kit::test]
fn knowledge_category_review_complete_content_and_footer_fit_theme_language_matrix(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    edit_category(
        window,
        &panel,
        &state.knowledge_entries[0],
        &"类".repeat(42),
        cx,
    );
    panel.update(cx, |view, cx| {
        view.prepare_category(&state.knowledge_entries[0], cx)
    });
    for family in ["frost", "graphite", "lavender"] {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(window, |_, window, cx| {
                    let settings = Settings {
                        theme_family: ThemeId::new(family).unwrap(),
                        theme,
                        language,
                        ..Default::default()
                    };
                    crate::design::apply_settings(&settings, Some(window), cx);
                    crate::i18n::set_language(language, cx);
                    window.render_frame(cx);
                    for id in ["knowledge-library-confirm", "knowledge-library-cancel"] {
                        let button = window.find(id);
                        let bounds = button.bounds();
                        assert!(
                            button.visible()
                                && bounds.left() >= px(0.)
                                && bounds.right() <= px(380.)
                                && bounds.top() >= px(0.)
                                && bounds.bottom() <= px(580.),
                            "{family}/{theme:?}/{language:?} {id} {bounds:?}"
                        );
                    }
                    let before = window.find(format!(
                        "knowledge-category-before-{}",
                        state.knowledge_entries[0].id()
                    ));
                    assert_eq!(
                        before.description(),
                        Some(state.knowledge_entries[0].entry().summary())
                    );
                    let after = window.find(format!(
                        "knowledge-category-after-{}",
                        state.knowledge_entries[0].id()
                    ));
                    assert_eq!(
                        after.description(),
                        Some(state.knowledge_entries[0].entry().summary())
                    );
                })
                .unwrap();
            }
        }
    }
    assert_eq!(
        panel.read_with(cx, |view, _| view.entries.clone()),
        state.knowledge_entries
    );
}

#[gpui_kit::test]
fn knowledge_category_removed_last_name_resets_filter_without_retargeting_selection(
    cx: &mut TestAppContext,
) {
    let (_, panel, mut state) = mount(cx);
    panel.update(cx, |view, cx| {
        view.set_category_filter(CategoryFilter::Named("ops".into()), cx);
        view.toggle_management(&state.knowledge_entries[1], view.management_revision, cx);
        state.knowledge_entries[1] = state.knowledge_entries[1]
            .with_reviewed_category(None)
            .unwrap();
        view.set_snapshot(&state, cx);
        assert!(view.category_filter == CategoryFilter::All);
        assert!(!view.category_counts().1.contains_key("ops"));
        assert_eq!(view.category_counts().0, 2);
        assert!(view.management.is_empty());
        assert!(view.context.entries().is_empty());
    });
}

#[gpui_kit::test]
fn knowledge_category_256_long_equal_prefix_names_keep_list_and_keyboard_actions_reachable(
    cx: &mut TestAppContext,
) {
    let (window, panel, mut state) = mount(cx);
    let names: Vec<_> = (0..256)
        .map(|index| format!("Shared {} {index:03}", "a".repeat(117)))
        .collect();
    assert!(names.iter().all(|name| name.len() == 128));
    state.knowledge_entries = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            super::tests::entry(&format!("Entry {index}"), "Reviewed text")
                .with_reviewed_category(Some(name))
                .unwrap()
        })
        .collect();
    panel.update(cx, |view, cx| view.set_snapshot(&state, cx));
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        let filters = window.find("knowledge-category-filters").bounds();
        assert_eq!(filters.size.height, px(112.));
        assert!(
            window
                .find(format!(
                    "knowledge-entry-{}",
                    state.knowledge_entries[0].id()
                ))
                .visible()
        );
        assert!(window.find("knowledge-management-review").visible());
        for index in [0, 1, 255] {
            assert_eq!(
                window
                    .find(format!("knowledge-category-filter-{index}"))
                    .label(),
                Some(format!("{} (1)", names[index]).as_str()),
            );
        }
        window.scroll(
            "knowledge-category-filters",
            ScrollDelta::Lines(point(0., -100000.)),
            cx,
        );
        window.render_frame(cx);
        let last = window.find("knowledge-category-filter-255");
        assert!(
            last.visible(),
            "last {:?}, viewport {:?}, offset {:?}",
            last.bounds(),
            window.find("knowledge-category-filters").bounds(),
            panel.read(cx).category_scroll.offset(),
        );
        assert!(
            window
                .find(format!(
                    "knowledge-entry-{}",
                    state.knowledge_entries[0].id()
                ))
                .visible()
        );
        window.scroll(
            "knowledge-category-filters",
            ScrollDelta::Lines(point(0., 100000.)),
            cx,
        );
        window.render_frame(cx);
        window.click("knowledge-category-all", cx);
        window.focus_next(cx);
        let mut reached = false;
        for _ in 0..280 {
            window.press("tab", cx);
            if window.find("knowledge-category-filter-255").focused() == Some(true) {
                reached = true;
                break;
            }
        }
        assert!(reached, "the last full category is keyboard reachable");
        window.press("enter", cx);
        panel.read_with(cx, |view, cx| {
            assert_eq!(
                view.matching_entries(cx).unwrap(),
                state.knowledge_entries[255..]
            );
            assert!(view.management.is_empty());
            assert!(view.context.entries().is_empty());
            assert!(view.selected.is_none());
        });
        window.render_frame(cx);
        let action = format!(
            "knowledge-category-edit-{}",
            state.knowledge_entries[255].id()
        );
        assert!(window.find(action.clone()).visible());
        window.click(action, cx);
        window.render_frame(cx);
        assert!(window.find("knowledge-category-review").visible());
    })
    .unwrap();
}
