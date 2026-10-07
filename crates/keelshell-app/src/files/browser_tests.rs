//! Owned metadata browsing and actual reviewed SFTP routes; not native acceptance.
use super::*;
use crate::files::local_catalog::LocalEntryKind;

async fn local_idle(h: &Harness, cx: &mut TestAppContext) {
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
}

fn browse(h: &Harness, path: PathBuf, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.browse_local(path, window, cx));
    })
    .checked("explicit local folder navigation");
}

fn header_click(h: &Harness, id: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(gpui_kit::SharedString::from(id.to_owned()), cx);
    })
    .checked("click an actual browser control");
}

fn painted_pointer_click(h: &Harness, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) {
    let id = id.into();
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        let target = window.find(id);
        assert!(
            target.visible(),
            "the browser action must be painted before pointer input"
        );
        let position = target.bounds().center();
        // Keep the same painted child and parent listeners throughout the
        // pointer pair so bubbling cannot be hidden by an intermediate frame.
        for input in [
            gpui_kit::MouseMoveEvent {
                position,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::MouseDownEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            gpui_kit::MouseUpEvent {
                button: gpui_kit::MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
        ] {
            window.dispatch_event(input, cx);
        }
    })
    .checked("dispatch a painted browser pointer action without repainting between events");
}

fn remote_visible_index(h: &Harness, path: &str, cx: &mut TestAppContext) -> usize {
    h.panel.read_with(cx, |panel, _| {
        crate::files::browser::remote_indices(
            &panel.entries,
            panel.remote_show_hidden,
            panel.remote_sort,
        )
        .iter()
        .position(|index| panel.entries[*index].path == path)
        .checked_option("listed remote target")
    })
}

#[gpui_kit::test]
async fn painted_remote_edit_preserves_dirty_review_until_cancel_or_confirm(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/z-original.txt", b"original remote bytes\n");
    painted_pointer_click(&h, "refresh-files", cx);
    h.idle(cx).await;
    let original = remote_visible_index(&h, "/z-original.txt", cx);
    painted_pointer_click(&h, ("open-remote", original), cx);
    h.idle(cx).await;
    painted_pointer_click(&h, "focus-file-draft", cx);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window
            .within("file-editor-input")
            .input("unsaved original draft\n", cx);
        assert_eq!(
            h.panel.read(cx).editor.read(cx).value().as_str(),
            "unsaved original draft\n"
        );
    })
    .checked("type the unsaved draft through the rendered editor");
    painted_pointer_click(&h, "close-file-editor", cx);
    h.seed("/a-replacement.txt", b"replacement remote bytes\n");
    painted_pointer_click(&h, "refresh-files", cx);
    h.idle(cx).await;

    for confirm in [false, true] {
        let replacement = remote_visible_index(&h, "/a-replacement.txt", cx);
        painted_pointer_click(&h, ("open-remote", replacement), cx);
        h.panel.read_with(cx, |panel, cx| {
            let (message, operation) = panel.pending.as_ref().checked_option(
                "the painted Edit must retain its dirty-editor review after parent bubbling",
            );
            assert!(
                matches!(operation, Operation::Read(entry) if entry.path == "/a-replacement.txt")
            );
            assert!(message.render(cx).contains("/a-replacement.txt"));
            assert_eq!(
                panel
                    .editing
                    .as_ref()
                    .checked_option("original editor remains open")
                    .0,
                "/z-original.txt"
            );
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "unsaved original draft\n"
            );
            assert!(panel.operation_id.is_none() && panel.transfer_jobs.is_empty());
        });
        assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
        painted_pointer_click(
            &h,
            if confirm {
                "confirm-file-operation"
            } else {
                "cancel-file-operation"
            },
            cx,
        );
        h.idle(cx).await;
        h.panel.read_with(cx, |panel, cx| {
            assert!(panel.pending.is_none());
            let expected = if confirm {
                ("/a-replacement.txt", "replacement remote bytes\n")
            } else {
                ("/z-original.txt", "unsaved original draft\n")
            };
            assert_eq!(
                panel
                    .editing
                    .as_ref()
                    .checked_option("review-selected editor")
                    .0,
                expected.0
            );
            assert_eq!(panel.editor.read(cx).value().as_str(), expected.1);
        });
    }
    assert_eq!(h.read("/z-original.txt"), b"original remote bytes\n");
    assert_eq!(h.read("/a-replacement.txt"), b"replacement remote bytes\n");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn painted_local_child_actions_do_not_restore_their_parent_row_selection(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let child = h.local.0.join("child");
    std::fs::create_dir(&child).checked("owned child folder");
    let file = child.join("visible.txt");
    std::fs::write(&file, b"local child content").checked("owned child file");
    browse(&h, h.local.0.clone(), cx);
    local_idle(&h, cx).await;
    let index = h.panel.read_with(cx, |panel, _| {
        panel
            .local_browser
            .listing
            .as_ref()
            .checked_option("owned parent listing")
            .entries
            .iter()
            .position(|entry| entry.path == child)
            .checked_option("owned child row")
    });
    painted_pointer_click(&h, ("local-entry", index), cx);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(panel.local_browser.selected.as_ref(), Some(&child))
    });
    painted_pointer_click(&h, ("use-open-local", index), cx);
    h.panel.read_with(cx, |panel, _| {
        assert!(
            panel.local_browser.selected.is_none(),
            "the old painted row must not restore its path after Open clears navigation"
        )
    });
    local_idle(&h, cx).await;
    let index = h.panel.read_with(cx, |panel, _| {
        let listing = panel
            .local_browser
            .listing
            .as_ref()
            .checked_option("owned child listing");
        assert_eq!(listing.directory, child);
        assert!(panel.local_browser.selected.is_none());
        listing
            .entries
            .iter()
            .position(|entry| entry.path == file)
            .checked_option("child file row")
    });
    painted_pointer_click(&h, ("use-open-local", index), cx);
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.local.read(cx).value().as_str(),
            file.to_str().checked_option("owned UTF-8 file")
        );
        assert!(
            panel.local_browser.selected.is_none(),
            "Use must not activate the parent row selection"
        );
        assert!(
            panel.pending.is_none()
                && panel.operation_id.is_none()
                && panel.transfer_jobs.is_empty()
        );
    });
    assert_eq!(
        std::fs::read(&file).checked("unchanged child file"),
        b"local child content"
    );
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn local_browsing_starts_empty_and_native_picker_is_single_directory_only(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.local_browser.listing.is_none());
        assert!(!panel.local_browser.reading());
        assert!(panel.local_browser.path.read(cx).value().is_empty());
        assert!(panel.local.read(cx).value().is_empty());
    });
    header_click(&h, "choose-local-folder", cx);
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|options| {
        assert!(options.directories && !options.files && !options.multiple);
        Some(vec![h.local.0.clone()])
    });
    cx.run_until_parked();
    local_idle(&h, cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel
                .local_browser
                .listing
                .as_ref()
                .checked_option("selected folder listing")
                .directory,
            h.local.0
        );
        assert!(
            panel.local.read(cx).value().is_empty(),
            "choosing a folder does not choose a transfer or write"
        );
        assert!(panel.pending.is_none());
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn browsing_a_local_source_fills_input_then_upload_requires_the_existing_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("browser-source.txt", b"owned browsing source");
    browse(&h, h.local.0.clone(), cx);
    local_idle(&h, cx).await;
    let index = h.panel.read_with(cx, |panel, _| {
        panel
            .local_browser
            .listing
            .as_ref()
            .checked_option("local listing")
            .entries
            .iter()
            .position(|entry| entry.path == source)
            .checked_option("actual local source row")
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(("local-entry", index)).visible());
        window.click(("use-open-local", index), cx);
    })
    .checked("use an actually listed local source");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.local.read(cx).value().as_ref(),
            source.to_str().checked_option("owned UTF-8 source")
        );
        assert!(panel.pending.is_none());
    });
    h.missing("/browser-source.txt");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.click(cx, "upload-file");
    h.panel.read_with(cx, |panel, cx| {
        let (message, operation) = panel.pending.as_ref().checked_option("explicit upload review");
        assert!(matches!(operation, Operation::Upload(local, remote) if local == &source && remote == "/browser-source.txt"));
        assert!(message.render(cx).contains(source.to_str().checked_option("UTF-8 source path")));
    });
    h.missing("/browser-source.txt");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(h.read("/browser-source.txt"), b"owned browsing source");
}

#[gpui_kit::test]
async fn late_local_navigation_cannot_replace_the_last_explicit_directory(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("first.txt", b"first");
    let second = h.local.0.join("second");
    std::fs::create_dir(&second).checked("owned second folder");
    std::fs::write(second.join("last.txt"), b"last").checked("owned second file");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.browse_local(h.local.0.clone(), window, cx);
            panel.browse_local(second.clone(), window, cx);
        });
    })
    .checked("two explicit navigations before UI adoption");
    local_idle(&h, cx).await;
    h.panel.read_with(cx, |panel, _| {
        let listing = panel
            .local_browser
            .listing
            .as_ref()
            .checked_option("latest listing");
        assert_eq!(listing.directory, second);
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].name, "last.txt");
        assert_eq!(listing.entries[0].kind, LocalEntryKind::File);
    });
    browse(&h, second.join(".."), cx);
    local_idle(&h, cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(
            panel.local_browser.listing.is_none(),
            "failed navigation must not leave an old listing under a new path"
        );
        assert!(panel.local_browser.status.render(cx).contains(".."));
    });
}

#[gpui_kit::test]
async fn hidden_selection_and_real_input_changes_do_not_cancel_an_issued_upload(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/.hidden.txt", b"hidden remote file");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List("/".into()), window, cx)
        });
    })
    .checked("list the actual hidden entry");
    h.idle(cx).await;
    let bytes = vec![0x62; 768 * 1024];
    let source = h.source("kept-owner.bin", &bytes);
    h.server.filesystem.set_transfer_write_delay(55);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|state| state.transferred > 0 && state.phase == TransferPhase::Running)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.phase(cx, TransferPhase::Paused).await;
    let (owner, session_token, stop) = h.panel.read_with(cx, |panel, _| {
        (
            panel.transfer_jobs[0].id,
            panel.session_token,
            panel
                .operation_stop
                .as_ref()
                .checked_option("actual upload stop owner")
                .clone(),
        )
    });
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.remote_show_hidden = true;
            panel.selected = panel
                .entries
                .iter()
                .find(|entry| entry.name == ".hidden.txt")
                .cloned();
            assert!(panel.selected.is_some());
            panel.toggle_remote_hidden(cx);
            assert!(panel.selected.is_none());
        });
        let local = h.panel.read(cx).local.clone();
        local.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        window.render_frame(cx);
    })
    .checked("hide the selected entry and focus the actual transfer input");
    cx.simulate_input(h.window, "new-draft");
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.local.read(cx).value().as_ref(), "new-draft");
        assert_eq!(panel.transfer_jobs[0].id, owner);
        assert_eq!(panel.session_token, session_token);
        assert!(Arc::ptr_eq(
            panel
                .operation_stop
                .as_ref()
                .checked_option("unchanged stop owner"),
            &stop
        ));
        assert!(!stop.load(Ordering::Acquire));
        assert_eq!(
            panel.transfer.as_ref().map(|state| state.phase),
            Some(TransferPhase::Paused)
        );
        assert!(panel.pending.is_none());
    });
    h.click(cx, "resume-file-transfer");
    h.idle(cx).await;
    assert_eq!(h.read("/kept-owner.bin"), bytes);
}

#[gpui_kit::test]
async fn typing_a_visible_local_navigation_draft_withdraws_the_old_unexecuted_review(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("review-source.txt", b"must remain unexecuted");
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.panel
        .read_with(cx, |panel, _| assert!(panel.pending.is_some()));
    cx.update_window(h.window, |_, window, cx| {
        let local = h.panel.read(cx).local_browser.path.clone();
        local.update(cx, |input, cx| input.focus(window, cx));
        window.render_frame(cx);
    })
    .checked("focus the local navigation input that remains visible during compact review");
    cx.simulate_input(h.window, "x");
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert!(!panel.confirmation_expanded);
        assert!(panel.transfer_jobs.is_empty());
    });
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
    h.missing("/review-source.txt");
}

#[gpui_kit::test]
async fn a_late_native_picker_result_cannot_replace_a_manually_edited_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    header_click(&h, "choose-local-folder", cx);
    assert!(cx.did_prompt_for_paths());
    cx.update_window(h.window, |_, window, cx| {
        let path = h.panel.read(cx).local_browser.path.clone();
        path.update(cx, |input, cx| input.focus(window, cx));
        window.render_frame(cx);
    })
    .checked("focus the explicit local draft");
    cx.simulate_input(h.window, "edited-draft");
    cx.simulate_path_prompt_response(|_| Some(vec![h.local.0.clone()]));
    cx.run_until_parked();
    h.panel.read_with(cx, |panel, cx| {
        assert_eq!(
            panel.local_browser.path.read(cx).value().as_ref(),
            "edited-draft"
        );
        assert!(panel.local_browser.listing.is_none());
        assert!(!panel.local_browser.reading());
    });
}

#[gpui_kit::test]
async fn a_late_read_only_resume_plan_cannot_recreate_review_after_input_changes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x73; 768 * 1024];
    h.seed("/resume-source.bin", &bytes);
    let partial = h.source("resume-partial.bin", &bytes[..96 * 1024]);
    h.server.filesystem.set_transfer_read_delay(75);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(
                Operation::PlanResume(
                    keelshell_session::sftp::TransferSpec::download(
                        "/resume-source.bin",
                        partial.clone(),
                    ),
                    false,
                ),
                window,
                cx,
            )
        });
        assert!(h.panel.read(cx).operation_id.is_some());
        let input = h.panel.read(cx).local.clone();
        input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        window.render_frame(cx);
    })
    .checked("start an actual read-only resume plan and focus its path draft");
    cx.simulate_input(h.window, "replacement-draft");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.operation_id.is_none());
        assert!(
            panel.pending.is_none(),
            "an obsolete plan must not recreate the withdrawn proposal"
        );
        assert!(panel.status.render(cx).contains("旧计划"));
    });
    assert_eq!(
        std::fs::read(&partial).checked("unchanged owned partial file"),
        bytes[..96 * 1024]
    );
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn loaded_local_and_remote_rows_fit_original_minimum_budgets_in_both_languages_and_themes(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(900., 580.), (1440., 900.)] {
        for assistant in [false, true] {
            let h = Harness::new_with(cx, |cx, session, runtime| {
                mount_layout_scene(cx, session, runtime, width, height, assistant)
            });
            h.idle(cx).await;
            h.source("local-visible.txt", b"local");
            h.seed("/remote-visible.txt", b"remote");
            cx.update_window(h.window, |_, window, cx| {
                h.panel.update(cx, |panel, cx| {
                    panel.run(Operation::List("/".into()), window, cx)
                });
            })
            .checked("load the controlled remote row");
            h.idle(cx).await;
            browse(&h, h.local.0.clone(), cx);
            local_idle(&h, cx).await;
            for language in [Language::ZhCn, Language::En] {
                for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
                    cx.update_window(h.window, |_, window, cx| {
                        crate::design::apply(theme, Some(window), cx);
                        i18n::set_language(language, cx);
                        h.panel
                            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                        window.render_frame(cx);
                        assert_true_file_row(window);
                        let body = window.find("file-browsing-area").bounds();
                        assert!(body.size.height >= px(64.));
                        assert!(window.find("file-tools-scroll").bounds().size.height >= px(28.));
                        let local = window.find(("local-entry", 0_usize));
                        let pane = window.find("local-files-pane").bounds();
                        assert!(
                            local.visible()
                                && local.bounds().origin.y >= pane.origin.y
                                && local.bounds().bottom() <= pane.bottom()
                        );
                        for id in [
                            "choose-local-folder",
                            "parent-local-folder",
                            "refresh-local-folder",
                            "toggle-local-hidden",
                            "parent-files",
                            "refresh-files",
                            "toggle-remote-hidden",
                        ] {
                            let control = window.find(id);
                            let area = window.find("files-layout-scene").bounds();
                            assert!(
                                control.visible()
                                    && control.bounds().origin.x >= area.origin.x
                                    && control.bounds().right() <= area.right(),
                                "{width}/{assistant}/{language:?}/{theme:?}/{id}"
                            );
                        }
                    })
                    .checked("actual GPUI local and remote row measurements");
                }
            }
        }
    }
}
