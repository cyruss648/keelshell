#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{KnowledgeLibrary, LibraryEvent, Phase};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size, test::TestWindowExt,
};
use gpui_kit::{InputEvent as _, MouseButton, MouseDownEvent, MouseUpEvent};
use keelshell_core::{
    AppState, KnowledgeCaptureType, KnowledgeSelection, KnowledgeService, KnowledgeSourceKind,
    SavedKnowledgeEntry, SelectedFact, SourceBinding,
};
use std::{cell::RefCell, rc::Rc};
use uuid::Uuid;

fn old_painted_action(action: &'static str, cx: &mut TestAppContext) {
    let (window, panel, state) = mount(cx);
    let events = Rc::new(RefCell::new(0));
    let received = events.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if matches!(event, LibraryEvent::Apply(_)) {
                *received.borrow_mut() += 1;
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.select(&state.knowledge_entries[0], window, cx);
            panel.prepare(&state.knowledge_entries[0], false, cx);
        });
        window.render_frame(cx);
        let position = window.find(action).bounds().center();
        let newer = panel.update(cx, |panel, cx| {
            panel.prepare(&state.knowledge_entries[0], true, cx);
            panel.review.as_ref().unwrap().id
        });
        // Dispatch against the already painted handlers. The Kit click helper
        // repaints and would replace the stale callbacks being exercised.
        window.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        assert_eq!(panel.read(cx).review.as_ref().unwrap().id, newer);
        assert_eq!(panel.read(cx).phase, Phase::Ready);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*events.borrow(), 0);
}

#[gpui_kit::test]
fn knowledge_library_old_painted_confirm_cannot_submit_new_review(cx: &mut TestAppContext) {
    old_painted_action("knowledge-library-confirm", cx);
}

#[gpui_kit::test]
fn knowledge_library_old_painted_cancel_cannot_discard_new_review(cx: &mut TestAppContext) {
    old_painted_action("knowledge-library-cancel", cx);
}

#[gpui_kit::test]
fn knowledge_library_unknown_and_reload_failure_remain_blocked(cx: &mut TestAppContext) {
    for error in [
        keelshell_core::Error::Durability(std::io::Error::other("owned fixture")),
        keelshell_core::Error::Io(std::io::Error::other("owned fixture")),
    ] {
        let (window, panel, state) = mount(cx);
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.select(&state.knowledge_entries[0], window, cx);
                panel.prepare(&state.knowledge_entries[0], true, cx);
                let id = panel.review.as_ref().unwrap().id;
                assert!(panel.begin_save(id, cx));
                panel.save_failed(id, &error, cx);
                let Phase::NeedsReload(token) = panel.phase else {
                    panic!("reload required")
                };
                assert!(panel.begin_reload(token, cx));
                panel.reload_failed(token, &error, cx);
                assert!(!panel.finish_reload(token, &state, cx));
                assert!(panel.candidate(id, cx).is_none());
                panel.prepare(&state.knowledge_entries[0], true, cx);
                assert!(panel.review.is_none());
                assert_eq!(panel.entries, state.knowledge_entries);
            })
        })
        .unwrap();
    }
}

pub(crate) fn entry(title: &str, summary: &str) -> SavedKnowledgeEntry {
    let source = SourceBinding::new(
        KnowledgeSourceKind::Session,
        Uuid::new_v4(),
        1,
        "Local fixture",
    )
    .unwrap();
    let fact = SelectedFact::completed(source, 100, title, summary).unwrap();
    let selection = KnowledgeSelection::new(vec![fact]).unwrap();
    let document = KnowledgeService::build(&selection).unwrap();
    SavedKnowledgeEntry::from_document(
        Uuid::new_v4(),
        &document,
        "fixture-session",
        KnowledgeCaptureType::Selection,
    )
    .unwrap()
}

fn mount(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<KnowledgeLibrary>, AppState) {
    cx.update(gpui_kit::init);
    let mut state = AppState::default();
    state
        .record_knowledge_entry(entry("磁盘空间", "Cleaned temporary cache"))
        .unwrap();
    state
        .record_knowledge_entry(entry("Network", "Verified DNS resolution"))
        .unwrap();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(520.), px(700.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| KnowledgeLibrary::new(window, cx)),
        )
        .unwrap()
    });
    panel.update(cx, |panel, cx| panel.set_snapshot(&state, cx));
    (window, panel, state)
}

#[gpui_kit::test]
fn knowledge_library_searches_local_unicode_and_rejects_unbounded_queries(cx: &mut TestAppContext) {
    let (window, panel, _) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel
                .query
                .update(cx, |input, cx| input.set_value("磁盘", window, cx));
            assert_eq!(panel.matching_entries(cx).unwrap().len(), 1);
            panel
                .query
                .update(cx, |input, cx| input.set_value("dns", window, cx));
            assert_eq!(
                panel.matching_entries(cx).unwrap()[0].entry().title(),
                "Network"
            );
            panel
                .query
                .update(cx, |input, cx| input.set_value("x".repeat(513), window, cx));
            assert!(panel.matching_entries(cx).is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_library_old_review_cannot_approve_or_cancel_replacement(cx: &mut TestAppContext) {
    let (window, panel, state) = mount(cx);
    let applied = Rc::new(RefCell::new(Vec::new()));
    let events = applied.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event, _| {
            if let LibraryEvent::Apply(id) = event {
                events.borrow_mut().push(*id);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let entry = &state.knowledge_entries[0];
            panel.select(entry, window, cx);
            panel.prepare(entry, false, cx);
            let old = panel.review.as_ref().unwrap().id;
            panel.prepare(entry, true, cx);
            let current = panel.review.as_ref().unwrap().id;
            assert_ne!(old, current);
            panel.confirm(old, cx);
            panel.cancel_review(old, cx);
            assert_eq!(panel.review.as_ref().unwrap().id, current);
            panel.confirm(current, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(applied.borrow().len(), 1);
}

#[gpui_kit::test]
fn knowledge_library_text_change_invalidates_review_and_sensitive_edit_is_rejected(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    let old = cx
        .update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.select(&state.knowledge_entries[0], window, cx);
                panel.prepare(&state.knowledge_entries[0], false, cx);
                let old = panel.review.as_ref().unwrap().id;
                panel.summary.update(cx, |input, cx| {
                    input.set_value("password = fixture-only-value", window, cx)
                });
                old
            })
        })
        .unwrap();
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        assert!(panel.candidate(old, cx).is_none());
        panel.prepare(&state.knowledge_entries[0], false, cx);
        assert!(panel.review.is_none());
        assert_eq!(panel.entries, state.knowledge_entries);
    });
}

#[gpui_kit::test]
fn knowledge_library_snapshot_change_and_failed_save_require_new_review(cx: &mut TestAppContext) {
    let (window, panel, mut state) = mount(cx);
    let old = cx
        .update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.select(&state.knowledge_entries[0], window, cx);
                panel.prepare(&state.knowledge_entries[0], true, cx);
                panel.review.as_ref().unwrap().id
            })
        })
        .unwrap();
    panel.update(cx, |panel, cx| {
        state.knowledge_entries[0] = state.knowledge_entries[0]
            .with_reviewed_content("Changed elsewhere", "Updated facts")
            .unwrap();
        panel.set_snapshot(&state, cx);
        assert!(panel.candidate(old, cx).is_none());
        assert!(panel.selected.is_none());
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.select(&state.knowledge_entries[0], window, cx);
            panel.prepare(&state.knowledge_entries[0], true, cx);
            let id = panel.review.as_ref().unwrap().id;
            assert!(panel.begin_save(id, cx));
            panel.save_failed(id, &keelshell_core::Error::Conflict, cx);
            assert!(panel.candidate(id, cx).is_none());
            panel.prepare(&state.knowledge_entries[0], true, cx);
            assert!(panel.review.is_none());
            let Phase::NeedsReload(token) = panel.phase else {
                panic!("reload required")
            };
            assert!(!panel.begin_reload(Uuid::new_v4(), cx));
            assert!(panel.begin_reload(token, cx));
            assert!(panel.finish_reload(token, &state, cx));
            assert!(panel.selected.is_none());
            assert!(panel.review.is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_library_review_footer_remains_visible_with_long_text(cx: &mut TestAppContext) {
    let (window, panel, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.select(&state.knowledge_entries[0], window, cx);
            panel.summary.update(cx, |input, cx| {
                input.set_value("review line\n".repeat(500), window, cx)
            });
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.prepare(&state.knowledge_entries[0], false, cx);
            assert!(panel.review.is_some(), "{}", panel.message.render(cx));
        });
        window.render_frame(cx);
        assert!(window.find("knowledge-library-confirm").visible());
        assert!(window.find("knowledge-library-cancel").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_library_context_edit_save_failure_and_reload_discard_old_selection(
    cx: &mut TestAppContext,
) {
    let (window, panel, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            let entry = &state.knowledge_entries[0];
            panel.toggle_context(entry, cx);
            let selected_revision = panel.context_revision();
            panel.select(entry, window, cx);
            assert_eq!(
                panel.context_selection().entries(),
                std::slice::from_ref(entry)
            );
            panel.summary.update(cx, |input, cx| {
                input.set_value("An unsaved edit", window, cx)
            });
            panel.invalidate_review(cx);
            assert!(panel.context_selection().entries().is_empty());
            assert_ne!(panel.context_revision(), selected_revision);
            // An unsaved draft cannot be silently selected in place of saved text.
            panel.toggle_context(entry, cx);
            assert!(panel.context_selection().entries().is_empty());
            panel.select(entry, window, cx);
            panel.toggle_context(entry, cx);
            panel.prepare(entry, true, cx);
            let review = panel.review.as_ref().unwrap().id;
            assert!(panel.begin_save(review, cx));
            assert!(panel.context_selection().entries().is_empty());
            panel.save_failed(review, &keelshell_core::Error::Conflict, cx);
            panel.toggle_context(entry, cx);
            assert!(panel.context_selection().entries().is_empty());
            let Phase::NeedsReload(token) = panel.phase else {
                panic!("reload required")
            };
            assert!(panel.begin_reload(token, cx));
            let reloading = panel.context_revision();
            assert!(panel.finish_reload(token, &state, cx));
            assert_ne!(panel.context_revision(), reloading);
            assert!(panel.context_selection().entries().is_empty());
            panel.toggle_context(entry, cx);
            assert_eq!(
                panel.context_selection().entries(),
                std::slice::from_ref(entry)
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_library_context_old_painted_add_cannot_select_changed_record(cx: &mut TestAppContext) {
    let (window, panel, mut state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let position = window
            .find(format!(
                "knowledge-context-toggle-{}",
                state.knowledge_entries[0].id()
            ))
            .bounds()
            .center();
        panel.update(cx, |panel, cx| {
            state.knowledge_entries[0] = state.knowledge_entries[0]
                .with_reviewed_content("Replacement", "Different facts")
                .unwrap();
            panel.set_snapshot(&state, cx);
        });
        window.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        assert!(panel.read(cx).context_selection().entries().is_empty());
    })
    .unwrap();
}
