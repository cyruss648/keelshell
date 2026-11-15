//! Actual GPUI confirmation controls precede any server archive command dispatch.
use super::*;
use keelshell_core::{
    ArchiveFormat, Settings, Theme, ThemeId, server_archive::ServerArchiveRequest,
};

async fn selected_source(h: &Harness, cx: &mut TestAppContext) {
    h.seed("/server-source.txt", b"reviewed server source\n");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List("/".into()), window, cx)
        });
    })
    .checked("refresh server archive source");
    h.idle(cx).await;
    let index = h.panel.read_with(cx, |panel, _| {
        crate::files::browser::remote_indices(
            &panel.entries,
            panel.remote_show_hidden,
            panel.remote_sort,
        )
        .iter()
        .position(|index| panel.entries[*index].path == "/server-source.txt")
        .checked_option("server source checkbox")
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("select-remote-entry", index), cx);
    })
    .checked("select the server source");
    set_name(h, "server-output.zip", cx);
}

fn set_name(h: &Harness, name: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .archive_name
            .clone()
            .update(cx, |input, cx| {
                input.set_value(name, window, cx);
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
    })
    .checked("edit the new server archive name");
}

fn exec_requests(h: &Harness) -> usize {
    h.server.exec_requests.load(Ordering::Acquire)
}

fn inspected_review(h: &Harness, cx: &mut TestAppContext) -> Operation {
    let expected = ServerArchiveRequest::new(
        vec!["/server-source.txt".into()],
        "/server-output.zip".into(),
        ArchiveFormat::Zip,
    )
    .checked("strict server archive request")
    .inspection_command();
    h.panel.read_with(cx, |panel, _| {
        let (message, operation) = panel
            .pending
            .as_ref()
            .checked_option("server archive inspection review");
        assert!(matches!(operation, Operation::PlanServerArchive(_)));
        for text in [message.translations().0, message.translations().1] {
            assert!(text.contains(&expected));
            assert!(text.contains(&panel.host));
            assert!(text.contains("/server-output.zip"));
        }
        operation.clone()
    })
}

#[gpui_kit::test]
async fn server_archive_inspection_is_reviewed_in_full_and_cancel_never_dispatches(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    selected_source(&h, cx).await;
    h.click(cx, "review-server-archive");
    inspected_review(&h, cx);
    assert_eq!(exec_requests(&h), 0);
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for id in ["confirm-file-operation", "cancel-file-operation"] {
            assert!(window.find(id).visible());
        }
    })
    .checked("complete server command retains fixed confirmation actions");
    h.click(cx, "cancel-file-operation");
    h.missing("/server-output.zip");
    assert_eq!(exec_requests(&h), 0);
    assert_eq!(h.read("/server-source.txt"), b"reviewed server source\n");
    assert!(!h.panel.read_with(cx, |panel, _| panel.window_close_busy()));
}

#[gpui_kit::test]
async fn server_archive_replaced_connection_cannot_confirm_with_the_same_host_and_token(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    selected_source(&h, cx).await;
    h.click(cx, "review-server-archive");
    inspected_review(&h, cx);
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("replace the authenticated transport without changing display identity");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(exec_requests(&h), 0);
    assert_eq!(exec_requests(&sibling), 0);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.operation_id.is_none());
        assert!(panel.pending.is_none());
    });
}

#[gpui_kit::test]
async fn server_archive_name_change_retires_the_review_and_rejects_a_retained_operation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    selected_source(&h, cx).await;
    h.click(cx, "review-server-archive");
    let old = inspected_review(&h, cx);
    set_name(&h, "changed.zip", cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            assert!(panel.pending.is_none());
            panel.run(old, window, cx);
        });
    })
    .checked("retained stale callback cannot authorize a changed target");
    h.idle(cx).await;
    assert_eq!(exec_requests(&h), 0);
    h.missing("/server-output.zip");
    h.missing("/changed.zip");
}

#[gpui_kit::test]
async fn server_archive_failed_inspection_never_creates_a_write_review_or_close_hold(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    selected_source(&h, cx).await;
    h.click(cx, "review-server-archive");
    inspected_review(&h, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    // This peer deliberately rejects every server packing command. That
    // rejection exercises the actual SSH/UI failure path, not a success mock.
    assert_eq!(exec_requests(&h), 1);
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert!(panel.isolation_target.is_none());
        assert!(!panel.window_close_busy());
    });
    h.missing("/server-output.zip");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn server_archive_inspection_controls_use_all_themes_and_languages_without_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    selected_source(&h, cx).await;
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &Settings {
                            theme_family: ThemeId::new(family.id).checked("registered theme"),
                            theme,
                            language,
                            ..Settings::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    reveal_file_control(window, cx, "review-server-archive");
                    assert!(window.find("review-server-archive").visible());
                })
                .checked("render the server action through semantic theme tokens");
                h.click(cx, "review-server-archive");
                inspected_review(&h, cx);
                h.click(cx, "cancel-file-operation");
                assert_eq!(exec_requests(&h), 0);
            }
        }
    }
}

#[cfg(unix)]
async fn owned_source(
    h: &Harness,
    format: ArchiveFormat,
    cx: &mut TestAppContext,
) -> (ServerArchiveRequest, PathBuf) {
    let source = h.local.0.join("源 '$(printf never).bin");
    std::fs::write(&source, [0, 255, 10, 128]).checked("create an owned actual helper source");
    let directory = h.local.0.to_str().checked_option("UTF-8 owned root");
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("seed helper virtual ancestors");
        for path in h.local.0.ancestors().collect::<Vec<_>>().into_iter().rev() {
            if path == std::path::Path::new("/") {
                continue;
            }
            let path = path.to_str().checked_option("UTF-8 fixture ancestor");
            if sftp
                .inspect_entry(path)
                .await
                .checked("inspect fixture parent")
                .is_none()
            {
                sftp.mkdir(path).await.checked("create fixture parent");
            }
        }
        sftp.write(
            source.to_str().checked_option("UTF-8 source"),
            &[0, 255, 10, 128],
        )
        .await
        .checked("mirror actual source into the packet fixture");
        sftp.close().await.checked("close helper seed connection");
    });
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List(directory.to_owned()), window, cx)
        });
    })
    .checked("browse the same owned root in the actual GPUI controller");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("select-remote-entry", 0_usize), cx);
    })
    .checked("select owned actual helper source");
    let name = match format {
        ArchiveFormat::Zip => "server-output.zip",
        ArchiveFormat::TarGz => "server-output.tar.gz",
    };
    set_name(h, name, cx);
    h.click(
        cx,
        match format {
            ArchiveFormat::Zip => "archive-format-zip",
            ArchiveFormat::TarGz => "archive-format-tar-gz",
        },
    );
    let destination = h.local.0.join(name);
    let request = ServerArchiveRequest::new(
        vec![source.to_str().checked_option("UTF-8 source").to_owned()],
        destination
            .to_str()
            .checked_option("UTF-8 archive")
            .to_owned(),
        format,
    )
    .checked("owned source request");
    h.server
        .server_archives
        .allow(request.inspection_command(), None);
    (request, destination)
}

#[gpui_kit::test]
#[cfg(unix)]
async fn server_archive_both_reviews_execute_the_real_helper_and_verify_both_formats(
    cx: &mut TestAppContext,
) {
    use keelshell_core::ArchiveBundle;
    for format in [ArchiveFormat::Zip, ArchiveFormat::TarGz] {
        let h = Harness::new(cx);
        h.idle(cx).await;
        let (_, destination) = owned_source(&h, format, cx).await;
        h.click(cx, "review-server-archive");
        assert_eq!(exec_requests(&h), 0);
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        let (command, input, staging) = h.panel.read_with(cx, |panel, _| {
            let Some((message, Operation::PackServerArchive(plan))) = &panel.pending else {
                panic!("actual helper inspection must produce the second complete review");
            };
            let input = std::str::from_utf8(plan.transfer.input()).checked("exact UTF-8 input");
            for text in [message.translations().0, message.translations().1] {
                assert!(text.contains(plan.transfer.command()));
                assert!(text.contains(input));
                assert!(text.contains(plan.transfer.staging_path()));
            }
            (
                plan.transfer.command().to_owned(),
                plan.transfer.input().to_vec(),
                plan.transfer.staging_path().to_owned(),
            )
        });
        assert!(!destination.exists());
        h.missing(destination.to_str().checked_option("UTF-8 archive"));
        h.server
            .server_archives
            .allow(command, Some(destination.clone()));
        h.click(cx, "expand-file-review");
        cx.update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("confirm-file-operation").visible());
            assert!(window.find("cancel-file-operation").visible());
        })
        .checked("complete packing command and stdin retain fixed actions");
        h.click(cx, "confirm-file-operation");
        h.idle(cx).await;
        assert_eq!(h.server.server_archives.inputs(), vec![vec![], input]);
        assert_eq!(exec_requests(&h), 2);
        assert!(!std::path::Path::new(&staging).exists());
        let bytes = h.read(destination.to_str().checked_option("UTF-8 archive"));
        assert_eq!(
            bytes,
            std::fs::read(&destination).checked("actual helper archive")
        );
        let bundle = ArchiveBundle::decode(format, &bytes).checked("decode actual helper result");
        assert_eq!(bundle.members().len(), 1);
        assert_eq!(bundle.members()[0].path(), "源 '$(printf never).bin");
        assert_eq!(bundle.members()[0].bytes(), &[0, 255, 10, 128]);
        h.panel.read_with(cx, |panel, _| {
            assert!(panel.pending.is_none());
            assert!(
                panel
                    .status
                    .translations()
                    .1
                    .contains("created and fully verified")
            );
            assert!(!panel.window_close_busy());
            assert!(panel.isolation_target.is_none());
            assert!(panel.recovery_records.is_empty());
        });
    }
}

#[gpui_kit::test]
#[cfg(unix)]
async fn server_archive_late_scan_cannot_authorize_a_replaced_actual_connection(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let sibling = Harness::new_in(cx, &h);
    sibling.idle(cx).await;
    let (_, destination) = owned_source(&h, ArchiveFormat::Zip, cx).await;
    let release = h.server.server_archives.hold_next();
    h.click(cx, "review-server-archive");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, _| {
        h.server.server_archives.calls.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, _| {
            panel.session = Some(sibling.session.clone());
        });
    })
    .checked("replace the actual connection while read-only inspection is held");
    release.send(()).checked("release old read-only scan");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert!(panel.isolation_target.is_none());
        assert!(!panel.window_close_busy());
    });
    assert_eq!(exec_requests(&h), 1);
    assert_eq!(exec_requests(&sibling), 0);
    assert!(!destination.exists());
}
