use super::*;
use keelshell_core::{
    KnowledgeCaptureType, KnowledgeSelection, KnowledgeService, KnowledgeSourceKind,
    SavedKnowledgeEntry, SelectedFact, SourceBinding,
};
use uuid::Uuid;

#[gpui_kit::test]
async fn workspace_knowledge_library_reload_revokes_old_assistant_capture_review(
    cx: &mut TestAppContext,
) {
    let (fixture, _) = mount_library(cx);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-close", cx);
    })
    .checked("return from the child library panel");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("context-screen", cx);
    })
    .checked("capture before the library reload");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("knowledge-preview-action", cx);
        window.render_frame(cx);
        window.click("knowledge-confirm-completed", cx);
    })
    .checked("prepare an independent assistant capture review");
    let old = fixture.workspace.read_with(cx, |workspace, cx| {
        workspace
            .assistant
            .read(cx)
            .knowledge_review_id_for_test()
            .unwrap_or_else(|| panic!("capture review exists"))
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-knowledge-library", cx);
        window.render_frame(cx);
        window.click("knowledge-library-review-delete", cx);
    })
    .checked("prepare library deletion while retaining the capture review");
    let mut external = fixture.store.load().checked("load competing state");
    external.settings.language = Language::En;
    fixture
        .store
        .save(&external)
        .checked("change state externally");
    let before = std::fs::read(fixture.store.path()).checked("read competing bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-confirm", cx);
    })
    .checked("submit stale library deletion");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-reload", cx);
    })
    .checked("reload the latest configuration");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).state_reloading
    })
    .await;
    fixture.workspace.update(cx, |workspace, cx| {
        assert!(
            workspace
                .assistant
                .read(cx)
                .knowledge_save_candidate(old)
                .is_none()
        );
        workspace.assistant.update(cx, |_, cx| {
            cx.emit(crate::assistant::AssistantEvent::SaveKnowledge { review_id: old });
        });
    });
    cx.run_until_parked();
    fixture
        .workspace
        .read_with(cx, |workspace, _| assert!(!workspace.saving));
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("read after stale capture event"),
        before
    );
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}

fn mount_library(cx: &mut TestAppContext) -> (Fixture, SavedKnowledgeEntry) {
    mount_library_sized(cx, 1280., 840.)
}

fn mount_library_sized(
    cx: &mut TestAppContext,
    width: f32,
    height: f32,
) -> (Fixture, SavedKnowledgeEntry) {
    let fixture = mount_sized(cx, Vec::new(), width, height);
    let source = SourceBinding::new(
        KnowledgeSourceKind::Session,
        Uuid::new_v4(),
        1,
        "Owned test capture",
    )
    .checked("knowledge source");
    let fact = SelectedFact::completed(source, 100, "Initial title", "Initial text")
        .checked("known completed fact");
    let selection = KnowledgeSelection::new(vec![fact]).checked("explicit selection");
    let document = KnowledgeService::build(&selection).checked("local document");
    let entry = SavedKnowledgeEntry::from_document(
        Uuid::new_v4(),
        &document,
        "owned-test-session",
        KnowledgeCaptureType::Selection,
    )
    .checked("saved entry");
    let mut state = fixture.store.load().checked("load initial state");
    state
        .record_knowledge_entry(entry.clone())
        .checked("record test knowledge");
    let state = fixture
        .store
        .save(&state)
        .checked("seed isolated knowledge fixture");
    adopt_fixture_state(&fixture, state.clone(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.refresh_knowledge_library(cx);
            workspace.show_assistant = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("open-knowledge-library", cx);
        window.render_frame(cx);
        window.click(format!("knowledge-entry-{}", entry.id()), cx);
    })
    .checked("open knowledge library through production UI");
    cx.run_until_parked();
    (fixture, entry)
}

#[gpui_kit::test]
fn workspace_knowledge_library_minimum_window_keeps_long_review_actions_in_bounds(
    cx: &mut TestAppContext,
) {
    for (language, theme) in [(Language::ZhCn, Theme::Light), (Language::En, Theme::Dark)] {
        let (fixture, original) = mount_library_sized(cx, 900., 580.);
        let mut state = fixture.store.load().checked("load large-list fixture");
        let mut last_id = original.id();
        for index in 1..256 {
            let mut wire =
                serde_json::to_value(&original).checked("serialize source-bound fixture");
            last_id = Uuid::new_v4();
            wire["id"] = serde_json::Value::String(last_id.to_string());
            wire["title"] =
                serde_json::Value::String(format!("{index} {}", "长标题 Long title ".repeat(20)));
            state
                .knowledge_entries
                .push(serde_json::from_value(wire).checked("validate fixture entry"));
        }
        state.settings.language = language;
        state.settings.theme = theme;
        let state = fixture
            .store
            .save(&state)
            .checked("save bounded 256-entry fixture");
        adopt_fixture_state(&fixture, state, cx);
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.apply_language(language, window, cx);
                crate::design::apply(theme, Some(window), cx);
                workspace.refresh_knowledge_library(cx);
            });
            window.render_frame(cx);
            window.click("knowledge-library-back", cx);
            window.render_frame(cx);
            window.scroll(
                "knowledge-library-scroll",
                ScrollDelta::Lines(point(0., -10000.)),
                cx,
            );
            window.render_frame(cx);
            let row = window.find(format!("knowledge-entry-{last_id}"));
            let area = window.find("assistant-column").bounds();
            assert!(row.visible());
            assert!(row.bounds().origin.x >= area.origin.x && row.bounds().right() <= area.right());
            window.click(format!("knowledge-entry-{last_id}"), cx);
            let library = fixture
                .workspace
                .read(cx)
                .assistant
                .read(cx)
                .knowledge_library
                .clone();
            library.update(cx, |panel, cx| {
                panel.set_draft_for_test(
                    "长标题 Long title",
                    &"Long reviewed summary 复核内容 ".repeat(300),
                    window,
                    cx,
                )
            });
        })
        .checked("reach the final knowledge row at the minimum workspace size");
        cx.run_until_parked();
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("knowledge-library-review-edit", cx);
            window.render_frame(cx);
            let area = window.find("assistant-column").bounds();
            for id in ["knowledge-library-confirm", "knowledge-library-cancel"] {
                let button = window.find(id);
                let bounds = button.bounds();
                assert!(
                    button.visible()
                        && bounds.origin.x >= area.origin.x
                        && bounds.right() <= area.right()
                        && bounds.origin.y >= area.origin.y
                        && bounds.bottom() <= area.bottom(),
                    "{language:?}/{theme:?} {id} {bounds:?} escapes {area:?}"
                );
            }
            window.click("knowledge-library-cancel", cx);
        })
        .checked("review and cancel with fully visible fixed actions");
    }
}

#[gpui_kit::test]
async fn workspace_knowledge_library_edit_cancel_and_delete_persist_only_after_confirmation(
    cx: &mut TestAppContext,
) {
    let (fixture, original) = mount_library(cx);
    let before = std::fs::read(fixture.store.path()).checked("read initial bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        let library = fixture
            .workspace
            .read(cx)
            .assistant
            .read(cx)
            .knowledge_library
            .clone();
        library.update(cx, |panel, cx| {
            panel.set_draft_for_test("Reviewed title", "Reviewed text", window, cx)
        });
    })
    .checked("edit the local draft");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-review-edit", cx);
        window.render_frame(cx);
        window.click("knowledge-library-cancel", cx);
    })
    .checked("cancel first review");
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("bytes after cancellation"),
        before
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-review-edit", cx);
        window.render_frame(cx);
        window.click("knowledge-library-confirm", cx);
    })
    .checked("confirm edited content");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    let edited = fixture.store.load().checked("read edited knowledge");
    let entry = &edited.knowledge_entries[0];
    assert_eq!(entry.entry().title(), "Reviewed title");
    assert_eq!(entry.entry().summary(), "Reviewed text");
    assert_eq!(entry.entry().source(), original.entry().source());
    assert_eq!(entry.session_id(), original.session_id());
    assert_eq!(entry.entry().outcome(), original.entry().outcome());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("knowledge-entry-{}", original.id()), cx);
    })
    .checked("select persisted entry for deletion");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-review-delete", cx);
        window.render_frame(cx);
        window.click("knowledge-library-confirm", cx);
    })
    .checked("confirm exact saved entry deletion");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert!(
        fixture
            .store
            .load()
            .checked("read after deletion")
            .knowledge_entries
            .is_empty()
    );
}

#[gpui_kit::test]
async fn workspace_knowledge_library_conflict_reload_never_replays_deletion(
    cx: &mut TestAppContext,
) {
    let (fixture, original) = mount_library(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-review-delete", cx);
    })
    .checked("prepare deletion");
    let mut external = fixture.store.load().checked("external snapshot");
    external.settings.language = Language::En;
    let changed = original
        .with_reviewed_content("External title", "External text")
        .checked("external content");
    external
        .replace_knowledge_entry(&original, changed.clone())
        .checked("external edit");
    fixture
        .store
        .save(&external)
        .checked("external writer commit");
    let before = std::fs::read(fixture.store.path()).checked("external bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("knowledge-library-confirm", cx);
    })
    .checked("submit stale deletion");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("bytes after conflict"),
        before
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("knowledge-library-reload").visible());
        assert!(window.try_find("knowledge-library-confirm").is_none());
        window.click("knowledge-library-reload", cx);
    })
    .checked("explicitly reload after conflict");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).state_reloading
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.state.settings.language, Language::En);
        assert_eq!(workspace.state.knowledge_entries, vec![changed]);
    });
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("bytes after read-only reload"),
        before
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("knowledge-library-confirm").is_none());
        assert!(
            window
                .find(format!("knowledge-entry-{}", original.id()))
                .visible()
        );
    })
    .checked("fresh selection is required after reload");
}
