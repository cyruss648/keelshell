#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::super::Workspace;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, ScrollDelta, TestAppContext, WindowBounds,
    WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{
    AppState, KnowledgeCaptureType, KnowledgeSelection, KnowledgeService, KnowledgeSourceKind,
    Language, SavedKnowledgeEntry, SelectedFact, SourceBinding, StateStore, Theme, ThemeId,
};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};
use uuid::Uuid;

struct Fixture {
    window: AnyWindowHandle,
    workspace: Entity<Workspace>,
    store: Arc<StateStore>,
    state: AppState,
    _directory: tempfile::TempDir,
}

fn fixture(cx: &mut TestAppContext, width: f32, height: f32) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(StateStore::new(directory.path().join("state.json")));
    let mut state = AppState::default();
    for index in 0..4 {
        let source = SourceBinding::new(
            KnowledgeSourceKind::Session,
            Uuid::new_v4(),
            7,
            &format!("Owned source {index}"),
        )
        .unwrap();
        let fact = SelectedFact::failed(
            source,
            100,
            &format!("Knowledge {index}"),
            &format!(
                "Full saved text {index} {}",
                "原始知识 Reviewed facts ".repeat(120)
            ),
        )
        .unwrap();
        let document =
            KnowledgeService::build(&KnowledgeSelection::new(vec![fact]).unwrap()).unwrap();
        state
            .record_knowledge_entry(
                SavedKnowledgeEntry::from_document(
                    Uuid::new_v4(),
                    &document,
                    "historical-owned-session",
                    KnowledgeCaptureType::Selection,
                )
                .unwrap(),
            )
            .unwrap();
    }
    let state = store.save(&state).unwrap();
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap(),
    );
    let workspace_store = store.clone();
    let workspace_state = state.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(state.settings.language, cx);
    });
    let (window, workspace) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(width), px(height)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    Workspace::new(workspace_store, workspace_state, None, runtime, window, cx)
                })
            },
        )
        .unwrap()
    });
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = true;
            workspace.refresh_knowledge_library(cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("open-knowledge-library", cx);
    })
    .unwrap();
    cx.run_until_parked();
    Fixture {
        window,
        workspace,
        store,
        state,
        _directory: directory,
    }
}

fn select(fixture: &Fixture, indices: &[usize], cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        for index in indices {
            window.click(
                format!(
                    "knowledge-management-toggle-{}",
                    fixture.state.knowledge_entries[*index].id()
                ),
                cx,
            );
        }
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
async fn workspace_knowledge_batch_cancel_zero_writes_then_one_save_preserves_survivors_on_restart(
    cx: &mut TestAppContext,
) {
    let fixture = fixture(cx, 1280., 840.);
    let before = std::fs::read(fixture.store.path()).unwrap();
    let backups = fixture.store.config_backups().unwrap().len();
    select(&fixture, &[0, 1], cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("knowledge-management-review", cx);
        window.click("knowledge-library-cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
    assert_eq!(fixture.store.config_backups().unwrap().len(), backups);

    let applied = Rc::new(RefCell::new(Vec::new()));
    let received = applied.clone();
    let library = fixture.workspace.read_with(cx, |workspace, cx| {
        workspace.assistant.read(cx).knowledge_library.clone()
    });
    let _subscription = cx.update(|cx| {
        cx.subscribe(&library, move |_, event, _| {
            if let crate::assistant::knowledge_library::LibraryEvent::Apply(id) = event {
                received.borrow_mut().push(*id);
            }
        })
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("knowledge-management-review", cx);
        window.click("knowledge-library-confirm", cx);
    })
    .unwrap();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(applied.borrow().len(), 1);
    assert_eq!(
        fixture.store.config_backups().unwrap().len(),
        backups + 1,
        "one serialized persist for the whole batch"
    );
    let saved = fixture.store.load().unwrap();
    let expected = &fixture.state.knowledge_entries[2..];
    assert_eq!(
        serde_json::to_vec(&saved.knowledge_entries).unwrap(),
        serde_json::to_vec(expected).unwrap()
    );
    let restarted = StateStore::new(fixture.store.path().to_owned())
        .load()
        .unwrap();
    assert_eq!(restarted.knowledge_entries, expected);
    let bytes = std::fs::read(fixture.store.path()).unwrap();
    let old = applied.borrow()[0];
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.apply_knowledge_change(old, window, cx)
        });
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), bytes);
    assert_eq!(fixture.store.config_backups().unwrap().len(), backups + 1);
}

#[gpui_kit::test]
async fn workspace_knowledge_batch_external_conflict_reload_never_replays_old_set(
    cx: &mut TestAppContext,
) {
    let fixture = fixture(cx, 1280., 840.);
    select(&fixture, &[0, 1], cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("knowledge-management-review", cx)
    })
    .unwrap();
    let other = StateStore::new(fixture.store.path().to_owned());
    let mut external = other.load().unwrap();
    external.knowledge_entries[1] = external.knowledge_entries[1]
        .with_reviewed_content("External change", "Newly saved facts")
        .unwrap();
    external.settings.language = Language::En;
    other.save(&external).unwrap();
    let before = std::fs::read(fixture.store.path()).unwrap();
    let backups = other.config_backups().unwrap().len();
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("knowledge-library-confirm", cx)
    })
    .unwrap();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
    assert_eq!(other.config_backups().unwrap().len(), backups);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
        assert!(window.find("knowledge-library-reload").visible());
        window.click("knowledge-library-reload", cx);
    })
    .unwrap();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).state_reloading
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(
            workspace.state.knowledge_entries,
            external.knowledge_entries
        );
        assert_eq!(workspace.state.settings.language, Language::En);
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
        assert_eq!(
            window.find("knowledge-management-review").disabled(),
            Some(true)
        );
        for entry in &external.knowledge_entries {
            assert_eq!(
                window
                    .find(format!("knowledge-management-toggle-{}", entry.id()))
                    .checked(),
                Some(false)
            );
        }
    })
    .unwrap();
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
    assert_eq!(other.config_backups().unwrap().len(), backups);
}

#[gpui_kit::test]
fn workspace_knowledge_batch_flat_review_footer_is_reachable_in_compact_and_wide_theme_language_matrix(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1280., 840.)] {
        let fixture = fixture(cx, width, height);
        select(&fixture, &[0, 1], cx);
        cx.update_window(fixture.window, |_, window, cx| {
            window.click("knowledge-management-review", cx)
        })
        .unwrap();
        for family in ["frost", "graphite", "lavender"] {
            for theme in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(fixture.window, |_, window, cx| {
                        let settings = keelshell_core::Settings {
                            theme_family: ThemeId::new(family).unwrap(), theme, language, ..Default::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        crate::i18n::set_language(language, cx);
                        window.render_frame(cx);
                        let area = window.find("assistant-column").bounds();
                        for id in ["knowledge-library-confirm", "knowledge-library-cancel"] {
                            let action = window.find(id);
                            let bounds = action.bounds();
                            assert!(action.visible() && bounds.left() >= area.left() && bounds.right() <= area.right()
                                && bounds.top() >= area.top() && bounds.bottom() <= area.bottom(),
                                "{width}/{family}/{theme:?}/{language:?} {id} {bounds:?} outside {area:?}");
                        }
                        window.scroll("knowledge-library-scroll", ScrollDelta::Lines(point(0., -100000.)), cx);
                        window.render_frame(cx);
                        let last = &fixture.state.knowledge_entries[1];
                        let full = window.find(format!("knowledge-management-full-{}", last.id()));
                        assert!(full.visible());
                        assert_eq!(full.description(), Some(last.entry().summary()));
                    }).unwrap();
                }
            }
        }
        // The matrix changes presentation only. No confirmation was issued.
        assert_eq!(
            fixture.store.load().unwrap().knowledge_entries,
            fixture.state.knowledge_entries
        );
        assert!(fixture.store.config_backups().unwrap().is_empty());
    }
}

fn category_draft(fixture: &Fixture, value: &str, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        let library = fixture
            .workspace
            .read(cx)
            .assistant
            .read(cx)
            .knowledge_library
            .clone();
        library.update(cx, |view, cx| {
            view.set_category_draft_for_test(value, window, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
async fn workspace_knowledge_category_cancel_zero_write_then_set_change_clear_save_once(
    cx: &mut TestAppContext,
) {
    let fixture = fixture(cx, 1280., 840.);
    let original = fixture.state.clone();
    for value in ["Ops", "ops", ""] {
        let before = std::fs::read(fixture.store.path()).unwrap();
        let backups = fixture.store.config_backups().unwrap().len();
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(
                format!(
                    "knowledge-category-edit-{}",
                    original.knowledge_entries[0].id()
                ),
                cx,
            );
        })
        .unwrap();
        category_draft(&fixture, value, cx);
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("knowledge-category-review", cx);
            window.render_frame(cx);
            assert!(window.find("knowledge-library-confirm").visible());
            window.click("knowledge-library-cancel", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
        assert_eq!(fixture.store.config_backups().unwrap().len(), backups);
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("knowledge-category-review", cx);
            window.render_frame(cx);
            window.click("knowledge-library-confirm", cx);
        })
        .unwrap();
        cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
            !fixture.workspace.read(cx).saving
        })
        .await;
        let saved = fixture.store.load().unwrap();
        let expected_category = (!value.is_empty()).then_some(value);
        assert_eq!(saved.knowledge_entries[0].category(), expected_category);
        assert_eq!(
            saved.knowledge_entries[0]
                .with_reviewed_category(None)
                .unwrap(),
            original.knowledge_entries[0]
        );
        assert_eq!(
            saved.knowledge_entries[1..],
            original.knowledge_entries[1..]
        );
        assert_eq!(fixture.store.config_backups().unwrap().len(), backups + 1);
        fixture.workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.state, saved);
        });
    }
}

#[gpui_kit::test]
async fn workspace_knowledge_category_external_conflict_reload_never_replays_review(
    cx: &mut TestAppContext,
) {
    let fixture = fixture(cx, 1280., 840.);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(
            format!(
                "knowledge-category-edit-{}",
                fixture.state.knowledge_entries[0].id()
            ),
            cx,
        );
    })
    .unwrap();
    category_draft(&fixture, "Ops", cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-category-review", cx);
    })
    .unwrap();
    let other = StateStore::new(fixture.store.path().to_owned());
    let mut external = other.load().unwrap();
    external.knowledge_entries[0] = external.knowledge_entries[0]
        .with_reviewed_category(Some("External"))
        .unwrap();
    external.settings.language = Language::En;
    let external = other.save(&external).unwrap();
    let before = std::fs::read(fixture.store.path()).unwrap();
    let backups = fixture.store.config_backups().unwrap().len();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-confirm", cx);
    })
    .unwrap();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
    assert_eq!(fixture.store.config_backups().unwrap().len(), backups);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
        window.click("knowledge-library-reload", cx);
    })
    .unwrap();
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).state_reloading
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
        assert!(window.try_find("knowledge-category-review").is_none());
        fixture.workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.state, external);
        });
    })
    .unwrap();
    assert_eq!(std::fs::read(fixture.store.path()).unwrap(), before);
    assert_eq!(fixture.store.config_backups().unwrap().len(), backups);
}
