#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{KnowledgeLibrary, LibraryEvent, Phase};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, InputEvent as _, MouseButton,
    MouseDownEvent, MouseUpEvent, Role, ScrollDelta, TestAppContext, WindowBounds, WindowOptions,
    point, px, size, test::TestWindowExt,
};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiPreset, AiProfileCatalog, AppState, Language, NamedAiProfile,
    SavedKnowledgeEntry, Settings, StateStore, Theme, ThemeId,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use uuid::Uuid;

fn state(count: usize) -> AppState {
    AppState {
        knowledge_entries: (0..count)
            .map(|index| {
                super::tests::entry(
                    &format!("知识 Entry {index}"),
                    &format!("Complete body {index} · {}", "已确认事实 ".repeat(40)),
                )
            })
            .collect(),
        ..AppState::default()
    }
}

fn mount(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<KnowledgeLibrary>, AppState) {
    cx.update(gpui_kit::init);
    let state = state(3);
    let (window, library) = cx.update(|cx| {
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
    library.update(cx, |view, cx| view.set_snapshot(&state, cx));
    (window, library, state)
}

fn select_batch(
    view: &mut KnowledgeLibrary,
    entries: &[SavedKnowledgeEntry],
    cx: &mut Context<KnowledgeLibrary>,
) {
    for entry in entries {
        view.toggle_management(entry, view.management_revision, cx);
    }
    view.prepare_batch_delete(view.management_revision, cx);
}

#[gpui_kit::test]
fn knowledge_batch_selection_is_independent_bounded_and_empty_review_writes_nothing(
    cx: &mut TestAppContext,
) {
    let (window, library, _) = mount(cx);
    let state = state(33);
    cx.update_window(window, |_, window, cx| {
        library.update(cx, |view, cx| {
            view.set_snapshot(&state, cx);
            view.prepare_batch_delete(view.management_revision, cx);
            assert!(view.review.is_none());
            view.select(&state.knowledge_entries[32], window, cx);
            view.toggle_context(&state.knowledge_entries[32], cx);
            let context_revision = view.context_revision();
            select_batch(view, &state.knowledge_entries[..32], cx);
            assert_eq!(view.management.len(), 32);
            let id = view.review.as_ref().unwrap().id;
            view.toggle_management(&state.knowledge_entries[32], view.management_revision, cx);
            assert_eq!(view.management.len(), 32);
            assert!(view.candidate(id, cx).is_some());
            assert_eq!(view.selected.as_ref(), Some(&state.knowledge_entries[32]));
            assert_eq!(view.context.entries(), &state.knowledge_entries[32..]);
            assert_eq!(view.context_revision(), context_revision);
            view.cancel_review(id, cx);
            assert_eq!(view.entries, state.knowledge_entries);
            assert_eq!(view.management.len(), 32);
            assert!(view.review.is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_batch_duplicate_or_ambiguous_selection_cannot_produce_a_candidate(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    library.update(cx, |view, cx| {
        view.management = vec![state.knowledge_entries[0].clone(); 2];
        view.prepare_batch_delete(view.management_revision, cx);
        assert!(view.review.is_none());
        view.management.truncate(1);
        view.entries.push(state.knowledge_entries[0].clone());
        view.prepare_batch_delete(view.management_revision, cx);
        assert!(view.review.is_none());
        assert_eq!(view.management.len(), 1);
    });
}

#[gpui_kit::test]
fn knowledge_batch_hidden_selection_has_full_source_and_body_without_changing_editor_or_ask(
    cx: &mut TestAppContext,
) {
    let (window, library, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        for entry in &state.knowledge_entries[..2] {
            window.click(format!("knowledge-management-toggle-{}", entry.id()), cx);
        }
        library.update(cx, |view, cx| {
            view.query
                .update(cx, |input, cx| input.set_value("Entry 1", window, cx));
            assert_eq!(
                view.matching_entries(cx).unwrap(),
                state.knowledge_entries[1..2]
            );
            assert_eq!(view.management, state.knowledge_entries[..2]);
        });
        window.render_frame(cx);
        assert!(
            window
                .try_find(format!(
                    "knowledge-entry-{}",
                    state.knowledge_entries[0].id()
                ))
                .is_none()
        );
        assert!(library.read(cx).selected.is_none());
        assert!(library.read(cx).context.entries().is_empty());
        window.click("knowledge-management-review", cx);
        window.render_frame(cx);
        for entry in &state.knowledge_entries[..2] {
            let full = window.find(format!("knowledge-management-full-{}", entry.id()));
            assert_eq!(full.description(), Some(entry.entry().summary()));
            let label = full.label().unwrap();
            assert!(label.contains(&entry.id().to_string()));
            assert!(label.contains(entry.entry().title()));
            assert!(label.contains(entry.entry().source().label()));
        }
        assert_eq!(
            library.read(cx).review.as_ref().unwrap().expected,
            state.knowledge_entries[..2]
        );
    })
    .unwrap();
}

fn old_painted_batch_action(action: &'static str, cx: &mut TestAppContext) {
    let (window, library, state) = mount(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&library, move |_, event, _| {
            if let LibraryEvent::Apply(id) = event {
                observed.borrow_mut().push(*id);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        library.update(cx, |view, cx| {
            select_batch(view, &state.knowledge_entries[..2], cx)
        });
        window.render_frame(cx);
        let position = window.find(action).bounds().center();
        let current = library.update(cx, |view, cx| {
            let old_revision = view.management_revision;
            view.toggle_management(&state.knowledge_entries[1], old_revision, cx);
            view.toggle_management(&state.knowledge_entries[2], view.management_revision, cx);
            view.prepare_batch_delete(view.management_revision, cx);
            view.review.as_ref().unwrap().id
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
        assert_eq!(library.read(cx).review.as_ref().unwrap().id, current);
        assert_eq!(library.read(cx).phase, Phase::Ready);
        assert_eq!(library.read(cx).entries, state.knowledge_entries);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(events.borrow().is_empty());
}

#[gpui_kit::test]
fn knowledge_batch_old_painted_confirm_cannot_submit_replacement(cx: &mut TestAppContext) {
    old_painted_batch_action("knowledge-library-confirm", cx);
}

#[gpui_kit::test]
fn knowledge_batch_old_painted_cancel_cannot_discard_replacement(cx: &mut TestAppContext) {
    old_painted_batch_action("knowledge-library-cancel", cx);
}

#[gpui_kit::test]
fn knowledge_batch_selection_aba_and_old_review_button_never_regain_authority(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    library.update(cx, |view, cx| {
        select_batch(view, &state.knowledge_entries[..2], cx);
        let old = view.review.as_ref().unwrap().id;
        let revision = view.management_revision;
        view.toggle_management(&state.knowledge_entries[1], revision, cx);
        view.toggle_management(&state.knowledge_entries[1], view.management_revision, cx);
        assert_eq!(view.management, state.knowledge_entries[..2]);
        view.prepare_batch_delete(revision, cx);
        assert!(view.review.is_none());
        view.prepare_batch_delete(view.management_revision, cx);
        let current = view.review.as_ref().unwrap().id;
        view.confirm(old, cx);
        view.cancel_review(old, cx);
        assert_ne!(old, current);
        assert_eq!(view.review.as_ref().unwrap().id, current);
        view.clear_management(revision, cx);
        assert_eq!(view.management.len(), 2);
    });
}

#[gpui_kit::test]
fn knowledge_batch_delayed_checkbox_and_clear_cannot_change_a_new_selection(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    library.update(cx, |view, cx| {
        let painted = view.management_revision;
        view.toggle_management(&state.knowledge_entries[0], painted, cx);
        view.toggle_management(&state.knowledge_entries[1], painted, cx);
        view.clear_management(painted, cx);
        assert_eq!(view.management, state.knowledge_entries[..1]);
        view.prepare_batch_delete(view.management_revision, cx);
        let review = view.review.as_ref().unwrap().id;
        view.entries[0] = state.knowledge_entries[0]
            .with_reviewed_content("Same identity, new bytes", "Different body")
            .unwrap();
        // Synchronous admission is independent of a queued snapshot/UI event.
        assert!(view.candidate(review, cx).is_none());
        assert!(!view.begin_save(review, cx));
        assert_eq!(view.phase, Phase::Ready);
    });
}

#[gpui_kit::test]
fn knowledge_batch_candidate_conflict_requires_reload_without_entering_saving(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    library.update(cx, |view, cx| {
        select_batch(view, &state.knowledge_entries[..2], cx);
        let id = view.review.as_ref().unwrap().id;
        view.candidate_failed(Uuid::new_v4(), &keelshell_core::Error::Conflict, cx);
        assert!(view.candidate(id, cx).is_some());
        view.candidate_failed(id, &keelshell_core::Error::Conflict, cx);
        assert!(matches!(view.phase, Phase::NeedsReload(_)));
        assert!(view.review.is_none());
        view.prepare_batch_delete(view.management_revision, cx);
        assert!(view.review.is_none());
        assert!(!view.begin_save(id, cx));
        assert_eq!(view.entries, state.knowledge_entries);
    });
}

#[gpui_kit::test]
fn knowledge_batch_content_or_configuration_snapshot_change_revokes_review(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    for content_change in [true, false] {
        library.update(cx, |view, cx| {
            view.management.clear();
            view.set_snapshot(&state, cx);
            select_batch(view, &state.knowledge_entries[..2], cx);
            let old = view.review.as_ref().unwrap().id;
            let mut changed = state.clone();
            if content_change {
                changed.knowledge_entries[1] = changed.knowledge_entries[1]
                    .with_reviewed_content("Changed bytes", "New original text")
                    .unwrap();
            } else {
                let fresh = StateStore::new(
                    std::env::temp_dir()
                        .join(format!("keelshell-absent-{}", Uuid::new_v4()))
                        .join("state.json"),
                )
                .load()
                .unwrap();
                changed.snapshot = fresh.snapshot;
                changed.settings.language = Language::En;
            }
            view.set_snapshot(&changed, cx);
            assert!(view.review.is_none());
            assert!(view.candidate(old, cx).is_none());
            assert_eq!(view.management.len(), if content_change { 1 } else { 2 });
        });
    }
}

#[gpui_kit::test]
fn knowledge_batch_conflict_unknown_and_io_require_explicit_reload_and_new_selection(
    cx: &mut TestAppContext,
) {
    let (_, library, state) = mount(cx);
    for error in [
        keelshell_core::Error::Conflict,
        keelshell_core::Error::Durability(std::io::Error::other("owned uncertainty")),
        keelshell_core::Error::Io(std::io::Error::other("owned failure")),
    ] {
        library.update(cx, |view, cx| {
            select_batch(view, &state.knowledge_entries[..2], cx);
            let old = view.review.as_ref().unwrap().id;
            assert!(view.begin_save(old, cx));
            view.save_failed(old, &error, cx);
            let Phase::NeedsReload(token) = view.phase else {
                panic!("reload required")
            };
            view.prepare_batch_delete(view.management_revision, cx);
            view.confirm(old, cx);
            assert!(view.review.is_none());
            assert!(!view.begin_save(old, cx));
            assert!(!view.begin_reload(Uuid::new_v4(), cx));
            assert!(view.begin_reload(token, cx));
            assert!(view.finish_reload(token, &state, cx));
            assert!(view.management.is_empty());
            assert!(view.review.is_none());
            assert!(view.candidate(old, cx).is_none());
            assert_eq!(view.entries, state.knowledge_entries);
        });
    }
}

fn assistant(
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<super::super::AssistantPanel>,
    AppState,
) {
    cx.update(gpui_kit::init);
    let mut profile = NamedAiProfile::draft(AiPreset::Custom);
    profile.name = "Owned batch knowledge fixture".into();
    profile.api_style = AiApiStyle::ChatCompletions;
    profile.authentication = AiAuthentication::None;
    profile.endpoint = "https://provider.example/v1/request".into();
    profile.model = "owned-model".into();
    profile.validate_current_transport().unwrap();
    let catalog = AiProfileCatalog {
        active_id: Some(profile.id),
        profiles: vec![profile],
    };
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap(),
    );
    let (window, panel) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                super::super::AssistantPanel::new(
                    &catalog,
                    &crate::ai_settings::EphemeralCredentials::new(),
                    runtime,
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    let state = state(3);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.prompt.update(cx, |input, cx| {
                input.set_value("Explain historical facts", window, cx)
            });
            panel.knowledge_library.update(cx, |view, cx| {
                view.set_snapshot(&state, cx);
                for entry in &state.knowledge_entries[..2] {
                    view.toggle_context(entry, cx);
                }
            });
        })
    })
    .unwrap();
    cx.run_until_parked();
    (window, panel, state)
}

#[gpui_kit::test]
fn knowledge_batch_saving_blocks_old_send_before_context_event_is_processed(
    cx: &mut TestAppContext,
) {
    let (_, panel, state) = assistant(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let send = panel.prepared_review_id.unwrap();
        assert!(panel.prepared.is_some());
        panel.knowledge_library.update(cx, |view, cx| {
            select_batch(
                view,
                &[
                    state.knowledge_entries[0].clone(),
                    state.knowledge_entries[2].clone(),
                ],
                cx,
            );
            let id = view.review.as_ref().unwrap().id;
            assert!(view.begin_save(id, cx));
            assert_eq!(view.context.entries(), &state.knowledge_entries[1..2]);
        });
        // Still in the same entity update: ContextChanged cannot have run.
        assert!(!panel.knowledge_request_is_current(cx));
        panel.send_reviewed(send, cx);
        assert!(panel.prepared.is_none());
        assert!(panel.prepared_review_id.is_none());
        assert!(!panel.busy);
        assert!(panel._job.is_none());
    });
}

#[gpui_kit::test]
fn knowledge_batch_saving_cancels_inflight_and_rejects_reply_before_or_after_context_event(
    cx: &mut TestAppContext,
) {
    for before_event in [true, false] {
        let (_, panel, state) = assistant(cx);
        panel.update(cx, |panel, cx| {
            panel.prepare(cx);
            assert!(panel.prepared.is_some());
            let revision = panel.request_revision;
            let target = (panel.host.clone(), panel.session_id.clone());
            let cancellation = keelshell_ai::RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.busy = true;
            panel.knowledge_library.update(cx, |view, cx| {
                select_batch(view, &state.knowledge_entries[..2], cx);
                assert!(view.begin_save(view.review.as_ref().unwrap().id, cx));
                assert!(view.context.entries().is_empty());
            });
            if !before_event {
                panel.revoke_stale_knowledge_request(cx);
                assert!(cancellation.is_cancelled());
                assert!(!panel.busy);
            }
            panel.finish_reply(revision, target, Ok("obsolete knowledge reply".into()), cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.response.is_empty());
            assert!(panel.suggestions.is_empty());
            assert!(!panel.busy);
            assert!(panel.prepared.is_none());
        });
    }
}

#[gpui_kit::test]
fn knowledge_batch_keyboard_toggle_and_tab_navigation_leave_ask_untouched(cx: &mut TestAppContext) {
    let (window, library, state) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        let first = format!(
            "knowledge-management-toggle-{}",
            state.knowledge_entries[0].id()
        );
        let second = format!(
            "knowledge-management-toggle-{}",
            state.knowledge_entries[1].id()
        );
        window.click(first.clone(), cx);
        assert_eq!(library.read(cx).management.len(), 1);
        // Root's Tab binding needs an existing focus path in this standalone
        // test window. Enter at the first stop, then use real keyboard events.
        window.focus_next(cx);
        window.render_frame(cx);
        assert_eq!(window.find("knowledge-library-close").focused(), Some(true));
        for target in [&first, &second] {
            let mut reached = false;
            for _ in 0..24 {
                window.press("tab", cx);
                if window.find(target.clone()).focused() == Some(true) {
                    reached = true;
                    break;
                }
            }
            assert!(reached, "management row must be Tab reachable: {target}");
            window.press("space", cx);
            if target == &first {
                assert!(library.read(cx).management.is_empty());
            }
        }
        assert_eq!(library.read(cx).management, state.knowledge_entries[1..2]);
        window.render_frame(cx);
        assert_eq!(window.find(second).role(), Some(Role::CheckBox));
        assert!(library.read(cx).context.entries().is_empty());
        assert!(library.read(cx).selected.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn knowledge_batch_full_review_scrolls_and_fixed_actions_fit_all_theme_language_variants(
    cx: &mut TestAppContext,
) {
    let (window, library, _) = mount(cx);
    let state = state(32);
    library.update(cx, |view, cx| {
        view.set_snapshot(&state, cx);
        select_batch(view, &state.knowledge_entries, cx);
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
                        let action = window.find(id);
                        let bounds = action.bounds();
                        assert!(
                            action.visible()
                                && bounds.left() >= px(0.)
                                && bounds.right() <= px(380.)
                                && bounds.top() >= px(0.)
                                && bounds.bottom() <= px(580.),
                            "{family}/{theme:?}/{language:?} {id} {bounds:?}"
                        );
                    }
                    window.scroll(
                        "knowledge-library-scroll",
                        ScrollDelta::Lines(point(0., -100000.)),
                        cx,
                    );
                    window.render_frame(cx);
                    let last = state.knowledge_entries.last().unwrap();
                    assert!(
                        window
                            .find(format!("knowledge-management-full-{}", last.id()))
                            .visible()
                    );
                    assert_eq!(
                        library.read(cx).review.as_ref().unwrap().expected,
                        state.knowledge_entries
                    );
                })
                .unwrap();
            }
        }
    }
}
