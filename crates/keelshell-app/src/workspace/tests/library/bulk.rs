use super::*;
use keelshell_core::ConnectionLibraryAction as Action;

fn profiles() -> Vec<Connection> {
    vec![
        Connection::new("First profile", "first.example.test", "operator"),
        Connection::new("Second profile", "second.example.test", "operator"),
    ]
}

fn select_all(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("library-select", 0_usize), cx);
        window.render_frame(cx);
        window.click(("library-select", 1_usize), cx);
    })
    .checked("explicitly select both profile rows");
}

fn review_confirm(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-review", cx);
        window.render_frame(cx);
        assert!(window.try_find(("library-bulk-target", 0_usize)).is_some());
        assert!(window.try_find(("library-bulk-target", 1_usize)).is_some());
        window.click("library-bulk-confirm", cx);
    })
    .checked("review exact targets then confirm one transaction");
}

fn prepare_trash(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let ids: Vec<_> = view.state.connections.iter().map(|item| item.id).collect();
            view.state
                .apply_connection_library_batch(&ids, &Action::Trash(10))
                .checked("prepare isolated deleted profiles");
            view.state = fixture
                .store
                .save(&view.state)
                .checked("save isolated trash fixture");
            view.set_library_filter(LibraryFilter::Trash, cx);
        });
    })
    .checked("prepare metadata-only trash fixture");
}

#[gpui_kit::test]
async fn bulk_favorites_move_and_tags_use_review_and_one_persistence_lane(cx: &mut TestAppContext) {
    let fixture = mount(cx, profiles());
    let original = fixture.store.load().checked("original state");
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-favorite", cx);
        assert!(!fixture.workspace.read(cx).saving);
    })
    .checked("draft cannot persist before review");
    assert_eq!(fixture.store.load().checked("no early write"), original);
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("favorite transaction")
            .connections
            .iter()
            .all(|item| item.favorite)
    );
    create_folder(&fixture, "Organized", cx);
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("all-connections", cx);
    })
    .checked("return to all active profiles");
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-move", cx);
        window.render_frame(cx);
        window.click(("library-bulk-folder", 0_usize), cx);
    })
    .checked("choose exact folder for both profiles");
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("move transaction")
            .connections
            .iter()
            .all(|item| item.group == "Organized")
    );
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-tags", cx);
        fixture.workspace.update(cx, |view, cx| {
            view.library_batch_prompt
                .as_ref()
                .checked_option("tags draft")
                .tags
                .update(cx, |input, cx| {
                    input.set_value("fleet, checked", window, cx)
                })
        });
    })
    .checked("enter tag organization draft");
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("tag transaction")
            .connections
            .iter()
            .all(|item| item.tags == vec!["checked", "fleet"])
    );
}

#[gpui_kit::test]
async fn invalid_tag_draft_and_locale_change_preserve_values_and_cancel_has_no_write(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, profiles());
    select_all(&fixture, cx);
    let invalid = "t".repeat(65);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-tags", cx);
        fixture.workspace.update(cx, |view, cx| {
            view.library_batch_prompt
                .as_ref()
                .checked_option("draft")
                .tags
                .update(cx, |input, cx| input.set_value(invalid.clone(), window, cx));
            view.review_library_batch(cx);
        });
        window.render_frame(cx);
        assert!(window.try_find("library-bulk-error").is_some());
        assert!(window.try_find("library-bulk-confirm").is_none());
        fixture
            .workspace
            .update(cx, |view, cx| view.switch_language(window, cx));
    })
    .checked("reject invalid tag and switch language without dropping draft");
    saved(&fixture, cx).await;
    let before = fixture.store.load().checked("locale-only persisted state");
    cx.update_window(fixture.window, |_, window, cx| {
        assert_eq!(
            fixture
                .workspace
                .read(cx)
                .library_batch_prompt
                .as_ref()
                .checked_option("retained draft")
                .tags
                .read(cx)
                .value(),
            invalid
        );
        window.render_frame(cx);
        assert_eq!(
            window.find("library-bulk-review").label(),
            Some("Review affected profiles")
        );
        window.click("library-bulk-cancel", cx);
    })
    .checked("cancel retained draft");
    assert_eq!(fixture.store.load().checked("cancel disk state"), before);
    assert!(before.connections.iter().all(|item| item.tags.is_empty()));
}

#[gpui_kit::test]
fn review_rejects_selection_and_profile_changes_and_preserves_pending_edits(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, profiles());
    select_all(&fixture, cx);
    let before = fixture.store.load().checked("before stale review");
    for selection_change in [true, false] {
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |view, cx| {
                if view.library_batch_prompt.is_none() {
                    view.open_library_batch(
                        view.state.connections.iter().map(|item| item.id).collect(),
                        Action::Favorite(true),
                        selection_change,
                        window,
                        cx,
                    );
                }
                view.review_library_batch(cx);
                if selection_change {
                    view.library_selection.clear();
                } else {
                    view.state.connections[0].tags = vec!["newer edit".into()];
                }
                view.confirm_library_batch(window, cx);
                assert!(!view.saving);
            });
            window.render_frame(cx);
            assert!(window.try_find("library-bulk-confirm").is_none());
            assert!(window.try_find("library-bulk-error").is_some());
            window.click("library-bulk-cancel", cx);
        })
        .checked("reject changed selection or profile before save");
        assert_eq!(fixture.store.load().checked("no stale write"), before);
    }
}

#[gpui_kit::test]
fn search_keeps_selection_explicit_and_review_lists_hidden_selected_profiles(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, profiles());
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.search
                .update(cx, |input, cx| input.set_value("First profile", window, cx))
        });
        window.render_frame(cx);
        assert!(window.try_find(("connection-row", 1_usize)).is_none());
        window.click("library-bulk-favorite", cx);
        window.render_frame(cx);
        window.click("library-bulk-review", cx);
        window.render_frame(cx);
        let labels: Vec<_> = (0_usize..2)
            .map(|index| {
                window
                    .find(("library-bulk-target", index))
                    .label()
                    .checked_option("full target identity")
                    .to_owned()
            })
            .collect();
        assert!(labels.iter().any(|label| label.contains("First profile")));
        assert!(labels.iter().any(|label| label.contains("Second profile")));
        assert!(
            fixture
                .workspace
                .read(cx)
                .state
                .connections
                .iter()
                .all(|item| !item.favorite)
        );
        window.click("library-bulk-cancel", cx);
    })
    .checked("hidden selected targets remain visible in the exact review");
}

#[gpui_kit::test]
async fn disk_revision_conflict_preserves_review_and_every_profile(cx: &mut TestAppContext) {
    let fixture = mount(cx, profiles());
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-favorite", cx);
        fixture
            .workspace
            .update(cx, |view, cx| view.review_library_batch(cx));
    })
    .checked("prepare accepted local review");
    let other = StateStore::new(fixture.store.path());
    let mut external = other.load().checked("external snapshot");
    external.connections[0].name = "External edit".into();
    other.save(&external).checked("external revision advance");
    let bytes = std::fs::read(fixture.store.path()).checked("external bytes");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.confirm_library_batch(window, cx))
    })
    .checked("attempt stale disk write");
    saved(&fixture, cx).await;
    assert_eq!(
        std::fs::read(fixture.store.path()).checked("disk after conflict"),
        bytes
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("library-bulk-error").is_some());
        let view = fixture.workspace.read(cx);
        assert!(view.library_batch_prompt.is_some());
        assert!(view.state.connections.iter().all(|item| !item.favorite));
    })
    .checked("retain complete review and local state after write conflict");
}

#[gpui_kit::test]
async fn batch_trash_checks_jump_dependents_and_explicit_undo_restores_chain(
    cx: &mut TestAppContext,
) {
    let mut values = profiles();
    values[1].jump_host = Some(values[0].id);
    let fixture = mount(cx, values);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("library-select", 0_usize), cx);
        window.render_frame(cx);
        window.click("library-bulk-trash", cx);
        window.render_frame(cx);
        window.click("library-bulk-review", cx);
        window.render_frame(cx);
        assert!(window.try_find("library-bulk-confirm").is_none());
        assert!(window.try_find("library-bulk-error").is_some());
        window.click("library-bulk-cancel", cx);
        window.render_frame(cx);
        window.click(("library-select", 1_usize), cx);
        window.render_frame(cx);
        window.click("library-bulk-trash", cx);
    })
    .checked("missing dependent blocks review, selecting whole chain admits it");
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    let trashed = fixture.store.load().checked("atomic trash");
    assert!(trashed.connections.is_empty());
    assert_eq!(trashed.deleted_connections.len(), 2);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-undo-trash", cx);
    })
    .checked("undo is a new explicit restoration review");
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    let restored = fixture.store.load().checked("restored chain");
    assert_eq!(restored.connections.len(), 2);
    assert!(restored.deleted_connections.is_empty());
    assert!(
        restored
            .connection_route(
                restored
                    .connections
                    .iter()
                    .find(|item| item.jump_host.is_some())
                    .checked_option("restored target")
                    .id
            )
            .is_ok()
    );
}

#[gpui_kit::test]
async fn undo_scope_tracks_remaining_trash_after_single_restore(cx: &mut TestAppContext) {
    let fixture = mount(cx, profiles());
    let ids: Vec<_> = fixture.workspace.read_with(cx, |view, _| {
        view.state.connections.iter().map(|item| item.id).collect()
    });
    select_all(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-bulk-trash", cx);
    })
    .checked("review both independent trash targets");
    review_confirm(&fixture, cx);
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |view, cx| view.restore_connection(ids[0], window, cx))
    })
    .checked("restore one trashed target separately");
    saved(&fixture, cx).await;
    fixture.workspace.read_with(cx, |view, _| {
        assert_eq!(view.library_trash_undo, vec![ids[1]])
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-undo-trash", cx);
        window.render_frame(cx);
        window.click("library-bulk-review", cx);
        window.render_frame(cx);
        assert!(window.try_find(("library-bulk-target", 0_usize)).is_some());
        assert!(window.try_find(("library-bulk-target", 1_usize)).is_none());
        window.click("library-bulk-confirm", cx);
    })
    .checked("undo reviews only the remaining trash identity");
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("undo final state")
            .deleted_connections
            .is_empty()
    );
    assert!(
        fixture
            .workspace
            .read_with(cx, |view, _| view.library_trash_undo.is_empty())
    );
}

#[gpui_kit::test]
async fn permanent_cleanup_single_selected_and_empty_trash_need_exact_confirmation(
    cx: &mut TestAppContext,
) {
    for scope in ["single", "selected", "empty"] {
        let fixture = mount(cx, profiles());
        prepare_trash(&fixture, cx);
        let before = fixture
            .store
            .load()
            .checked("trash before permanent review");
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            match scope {
                "single" => window.click(("library-purge", 0_usize), cx),
                "selected" => {
                    window.click(("library-select", 0_usize), cx);
                    window.render_frame(cx);
                    window.click("library-bulk-purge", cx);
                }
                _ => window.click("library-empty-trash", cx),
            }
            window.render_frame(cx);
            assert!(window.try_find("library-bulk-confirm").is_none());
            window.click("library-bulk-review", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("library-bulk-confirm").label(),
                Some("确认永久删除")
            );
            let target = window.find(("library-bulk-target", 0_usize));
            assert!(target.bounds().size.height > gpui_kit::px(0.));
        })
        .checked("single, selected and full-trash cleanup all require a review");
        assert_eq!(fixture.store.load().checked("review did not write"), before);
        cx.update_window(fixture.window, |_, window, cx| {
            window.click("library-bulk-confirm", cx)
        })
        .checked("confirm exact permanent cleanup");
        saved(&fixture, cx).await;
        let after = fixture.store.load().checked("read cleanup result");
        assert_eq!(
            after.deleted_connections.len(),
            if scope == "empty" { 0 } else { 1 }
        );
        assert!(after.connections.is_empty());
        assert!(
            fixture
                .workspace
                .read_with(cx, |view, _| view.tabs.is_empty())
        );
    }
}

#[gpui_kit::test]
fn narrow_review_keeps_footer_in_window_in_both_themes_and_languages(cx: &mut TestAppContext) {
    use keelshell_core::Theme;
    let fixture = mount_sized(cx, profiles(), 480., 640.);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.open_library_batch(
                view.state.connections.iter().map(|item| item.id).collect(),
                Action::Purge,
                false,
                window,
                cx,
            )
        });
        // A permanent-delete draft is still bounded before domain validation.
        for theme in [Theme::Light, Theme::Dark] {
            crate::design::apply(theme, Some(window), cx);
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let dialog = window.find("library-bulk-dialog").bounds();
                let footer = window.find("library-bulk-dialog-footer").bounds();
                assert!(contained(dialog, window.bounds()));
                assert!(contained(footer, dialog));
                for button in ["library-bulk-cancel", "library-bulk-review"] {
                    assert!(contained(window.find(button).bounds(), footer));
                    assert!(window.find(button).visible());
                }
            }
        }
    })
    .checked("bounded narrow permanent cleanup draft with reachable controls");
}

#[gpui_kit::test]
async fn trash_review_binds_real_ssh_session_and_keeps_it_open_without_terminal_writes(
    cx: &mut TestAppContext,
) {
    use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
    let fixture = mount(cx, vec![password_profile()]);
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .checked("bind owned SSH fixture");
    let port = listener.local_addr().checked("SSH address").port();
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x79; 32]));
    let pin = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let gate = Arc::new(tokio::sync::Notify::new());
    gate.notify_one();
    let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handler = DelayedShell {
        gate,
        authenticating: Arc::new(AtomicBool::new(false)),
        received: received.clone(),
        channels: Default::default(),
        jobs: tokio::task::JoinSet::new(),
    };
    let server = runtime.spawn(async move {
        if let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(20), listener.accept()).await
            && let Ok(session) = russh::server::run_stream(config, socket, handler).await
        {
            let _ = tokio::time::timeout(Duration::from_secs(20), session).await;
        }
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.state.connections[0].port = port;
            view.state = fixture
                .store
                .save(&view.state)
                .checked("save owned fixture endpoint");
            view.connect(
                view.state.connections[0].clone(),
                zeroize::Zeroizing::new("delayed-test-password".into()),
                Some(pin),
                window,
                cx,
            );
        })
    })
    .checked("connect actual bounded SSH fixture");
    cx.wait_for(fixture.window, Duration::from_secs(15), |_, cx| {
        !fixture.workspace.read(cx).connecting
    })
    .await;
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            assert_eq!(view.remote_sessions.len(), 1, "fixture must connect");
            let id = view.state.connections[0].id;
            assert_eq!(view.library_session_references(id, cx).len(), 1);
            view.open_library_batch(vec![id], Action::Trash(10), false, window, cx);
            view.review_library_batch(cx);
        });
        window.render_frame(cx);
        window.input("never-send", cx);
    })
    .checked("review actual session and isolate terminal input");
    cx.run_until_parked();
    assert_eq!(received.load(std::sync::atomic::Ordering::Acquire), 0);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let tab = view.tabs[0].entity_id();
            let session = view
                .remote_sessions
                .remove(&tab)
                .checked_option("owned session");
            view.confirm_library_batch(window, cx);
            assert!(!view.saving, "changed live session invalidates review");
            view.remote_sessions.insert(tab, session);
            view.review_library_batch(cx);
            view.confirm_library_batch(window, cx);
        })
    })
    .checked("session-reference change rejects old review; fresh review succeeds");
    saved(&fixture, cx).await;
    fixture.workspace.read_with(cx, |view, cx| {
        assert!(view.state.connections.is_empty());
        assert_eq!(view.state.deleted_connections.len(), 1);
        assert_eq!(view.tabs.len(), 1);
        assert_eq!(view.remote_sessions.len(), 1);
        assert!(view.tabs[0].read(cx).is_open());
    });
    assert_eq!(received.load(std::sync::atomic::Ordering::Acquire), 0);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.close_tab(&crate::workspace::CloseTab, window, cx)
        })
    })
    .checked("close owned session after metadata assertion");
    server.abort();
}
