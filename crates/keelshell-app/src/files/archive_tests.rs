//! Production archive actions and exact bytes on owned GPUI/SFTP fixtures.
use super::*;
use keelshell_core::{
    ArchiveBundle, ArchiveEntryKind, ArchiveFormat, ArchiveMember, Settings, Theme, ThemeId,
};

#[test]
fn archive_unobserved_worker_result_keeps_the_unknown_close_hold() {
    use crate::files::{FileFailure, Outcome, archive, terminal_transfer_phase};
    use keelshell_session::SessionError;

    let result: Result<Outcome, FileFailure> = Err(SessionError::MutationUncertain.into());
    assert!(matches!(&result, Err(FileFailure::OutcomeUncertain(_))));
    assert!(archive::retains_close_hold(&result));
    assert_eq!(
        terminal_transfer_phase(true, &result),
        Some(TransferPhase::Uncertain)
    );
}

async fn browse_and_select(h: &Harness, names: &[&str], cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.browse_local(h.local.0.clone(), window, cx)
        });
    })
    .checked("browse real local archive sources");
    cx.wait_for(h.window, Duration::from_secs(12), |_, cx| {
        !h.panel.read(cx).local_browser.reading()
    })
    .await;
    for name in names {
        let index = h.panel.read_with(cx, |panel, _| {
            panel
                .local_browser
                .listing
                .as_ref()
                .checked_option("loaded archive source listing")
                .entries
                .iter()
                .position(|entry| entry.name.to_str() == Some(name))
                .checked_option("archive source row")
        });
        cx.update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(("select-local-entry", index), cx);
        })
        .checked("select a real local source checkbox");
    }
}

fn name(h: &Harness, value: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .read(cx)
            .archive_name
            .clone()
            .update(cx, |input, cx| {
                input.set_value(value, window, cx);
                // Kit's programmatic setter deliberately suppresses Change.
                // Emit the same event as a real edit to exercise invalidation.
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
    })
    .checked("set archive destination draft");
}

async fn remote_selection(h: &Harness, path: &str, cx: &mut TestAppContext) {
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::List("/".into()), window, cx)
        });
    })
    .checked("refresh real remote archive listing");
    h.idle(cx).await;
    let index = h.panel.read_with(cx, |panel, _| {
        crate::files::browser::remote_indices(
            &panel.entries,
            panel.remote_show_hidden,
            panel.remote_sort,
        )
        .iter()
        .position(|index| panel.entries[*index].path == path)
        .checked_option("remote archive checkbox")
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("select-remote-entry", index), cx);
    })
    .checked("select the actual remote archive");
}

fn pending_kind(pending: &Option<(crate::i18n::Message, Operation)>) -> &'static str {
    let Some((_, operation)) = pending else {
        return "None";
    };
    match operation {
        Operation::PlanArchive(_) => "PlanArchive",
        Operation::TransferArchive(_) => "TransferArchive",
        // Only archive admission matters here. Never format another operation,
        // since its payload may contain private paths or document contents.
        _ => "Other",
    }
}

fn display_os_error(detail: &str) -> Option<u32> {
    detail
        .rsplit_once("(os error ")
        .and_then(|(_, suffix)| suffix.strip_suffix(')'))
        .filter(|digits| {
            !digits.is_empty()
                && digits.len() <= 10
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        .and_then(|digits| digits.parse::<u32>().ok())
}

fn safe_review_status(message: &crate::i18n::Message) -> String {
    // Status details can contain paths or peer text. Only fixed classifications
    // and an optional numeric I/O display suffix may enter a failing CI log.
    let (_, en) = message.translations();
    let Some(detail) = en.strip_prefix("Review incomplete; transfer not started: ") else {
        let stage = match en {
            "Preparing the archive and complete manifest read-only; review is required…" => {
                "preparing"
            }
            "Archive prepared; review the complete manifest and new destination." => "prepared",
            "Archive review expired: the session, folder, selection, format or path changed. Prepare again." => {
                "archive-binding-expired"
            }
            "Read-only review preparation completed, but the path or selection changed; the old plan was not adopted. Prepare it again." => {
                "browser-review-expired"
            }
            "Wait for or cancel the active file operation" => "active-operation-blocked",
            "Select ordinary files or folders in the local list." => "local-selection-rejected",
            "Enter a portable new archive name ending in .zip."
            | "Enter a portable new archive name ending in .tar.gz." => "archive-name-rejected",
            "Select one remote archive and enter an absolute new folder in the local transfer path." => {
                "extraction-selection-rejected"
            }
            _ if en.starts_with("Unable to start file worker: ") => "worker-start-failed",
            _ => "other-redacted",
        };
        let code = en
            .strip_prefix("Unable to start file worker: ")
            .and_then(display_os_error);
        return format!("stage={stage} display_os_error={code:?}");
    };
    let (kind, reason, code) = if let Some(io) = detail.strip_prefix("I/O: ") {
        let reason = match io {
            "published local name changed or has a case alias" => "local-name-alias",
            "local path must be an absolute drive path" => "absolute-drive-path-required",
            "local name is not a portable literal component" => "nonportable-component",
            "local name aliases a DOS device" => "dos-device-alias",
            "local publication requires a local drive" => "nonlocal-drive",
            "local publication requires a verified local disk volume" => "unverified-local-volume",
            "local object is a reparse point, link alias or pending deletion" => {
                "unsafe-local-object"
            }
            "invalid native directory-name response" => "native-directory-response",
            "native open returned an unconfirmed outcome" => "native-open-unconfirmed",
            "local destination or case alias already exists" => "local-destination-exists",
            _ => "other-redacted",
        };
        ("io", reason, display_os_error(io))
    } else {
        let kind = match detail {
            "session is closed" => "closed",
            "session worker state is unavailable" => "session-worker",
            "File worker stopped without a result" => "file-worker",
            "Operation completed but SFTP cleanup failed" => "sftp-cleanup",
            "File operation cancelled before SFTP initialization" => "cancelled-before-sftp",
            "invalid session option: archive source, destination or ancestors changed; review again" => {
                "archive-observation-changed"
            }
            "invalid session option: archive codec rejected invalid, unsafe or over-limit contents" => {
                "archive-codec-rejected"
            }
            "invalid session option: archive rejects remote canonical path aliases" => {
                "remote-path-alias"
            }
            "invalid session option: archive remote destination already exists" => {
                "remote-destination-exists"
            }
            _ if detail.starts_with("invalid session option: ") => "session-invalid",
            _ if detail.starts_with("operation timed out: ") => "timeout",
            _ if detail.starts_with("output exceeds ") => "output-limit",
            _ if detail.starts_with("SFTP: ") => "sftp",
            _ if detail.starts_with("SSH: ") => "ssh",
            _ if detail.starts_with("file target is ") => "mutation-isolation",
            _ => "other-redacted",
        };
        (kind, "redacted", None)
    };
    // This code is parsed from the existing display string, not a retained
    // typed io::Error or evidence of a specific failed native syscall.
    format!("stage=review-error kind={kind} reason={reason} display_os_error={code:?}")
}

#[test]
fn archive_review_status_diagnostics_redact_peer_and_path_text() {
    for (detail, kind, reason, code) in [
        (
            "I/O: fixture-private-path (os error 5)",
            "io",
            "other-redacted",
            "Some(5)",
        ),
        (
            "SFTP: fixture-private-path (os error 5)",
            "sftp",
            "redacted",
            "None",
        ),
        (
            "I/O: fixture-private-path (os error 4294967296)",
            "io",
            "other-redacted",
            "None",
        ),
        (
            "I/O: fixture-private-path (os error 5)\nfixture-peer-text",
            "io",
            "other-redacted",
            "None",
        ),
    ] {
        let message = crate::i18n::Message::detail(
            "fixture-private-translation",
            "Review incomplete; transfer not started",
            detail,
        );
        assert_eq!(
            safe_review_status(&message),
            format!("stage=review-error kind={kind} reason={reason} display_os_error={code}")
        );
    }
    assert_eq!(
        safe_review_status(&crate::i18n::Message::new("", "fixture-private-status")),
        "stage=other-redacted display_os_error=None"
    );
}

#[test]
fn archive_review_status_diagnostics_distinguish_rejected_and_stale_preparation() {
    for (text, expected) in [
        (
            "Review incomplete; transfer not started: I/O: published local name changed or has a case alias",
            "stage=review-error kind=io reason=local-name-alias display_os_error=None",
        ),
        (
            "Review incomplete; transfer not started: operation timed out: SFTP archive review",
            "stage=review-error kind=timeout reason=redacted display_os_error=None",
        ),
        (
            "Archive review expired: the session, folder, selection, format or path changed. Prepare again.",
            "stage=archive-binding-expired display_os_error=None",
        ),
        (
            "Unable to start file worker: fixture-private-detail (os error 1450)",
            "stage=worker-start-failed display_os_error=Some(1450)",
        ),
    ] {
        assert_eq!(
            safe_review_status(&crate::i18n::Message::new("", text)),
            expected,
        );
    }
}

#[track_caller]
fn reviewed(h: &Harness, cx: &mut TestAppContext) -> Operation {
    let result = h.panel.read_with(cx, |panel, _| match &panel.pending {
        Some((message, Operation::TransferArchive(plan))) => {
            let (zh, en) = message.translations();
            assert!(zh.contains("完整成员清单"));
            assert!(en.contains("Complete member manifest"));
            assert!(!plan.transfer.members().is_empty());
            Ok(Operation::TransferArchive(plan.clone()))
        }
        _ => Err(format!(
            "busy={} operation_id_present={} pending_kind={} session_present={} suspended={} \
             directory_present={} local_listing_present={} local_reading={} \
             local_selection_count={} remote_selection_count={} selected_present={} \
             isolation_present={} close_hold={} atomic_writes_started={} status=[{}]",
            panel.busy,
            panel.operation_id.is_some(),
            pending_kind(&panel.pending),
            panel.session.is_some(),
            panel.suspended,
            panel.directory.is_some(),
            panel.local_browser.listing.is_some(),
            panel.local_browser.reading(),
            panel.local_browser.selection.len(),
            panel.remote_selection.len(),
            panel.selected.is_some(),
            panel.isolation_target.is_some(),
            panel.window_close_busy(),
            h.server.filesystem.atomic_writes_started(),
            safe_review_status(&panel.status),
        )),
    });
    match result {
        Ok(operation) => operation,
        Err(diagnostic) => {
            let controls = cx
                .update_window(h.window, |_, window, _| {
                    ["review-archive-upload", "review-archive-extract"].map(|id| {
                        window.try_find(id).map(|control| {
                            (id, control.visible(), control.disabled(), control.bounds())
                        })
                    })
                })
                .checked("inspect archive controls after a missing review");
            panic!(
                "archive preparation must produce a full immutable review: \
                 {diagnostic} controls={controls:?}"
            )
        }
    }
}

#[gpui_kit::test]
async fn archive_zip_upload_requires_confirmation_and_preserves_exact_binary_members(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("first.bin", &[0, 255, 10, 128]);
    h.source("second.txt", "完整内容\n".as_bytes());
    browse_and_select(&h, &["first.bin", "second.txt"], cx).await;
    name(&h, "selected.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    let Operation::TransferArchive(plan) = reviewed(&h, cx) else {
        unreachable!()
    };
    assert_eq!(plan.transfer.members().len(), 2);
    h.missing("/selected.zip");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    let archive = ArchiveBundle::decode(ArchiveFormat::Zip, &h.read("/selected.zip"))
        .checked("decode published archive from real SFTP bytes");
    assert_eq!(
        archive
            .members()
            .iter()
            .find(|m| m.path() == "first.bin")
            .checked_option("binary member")
            .bytes(),
        &[0, 255, 10, 128]
    );
    assert_eq!(
        archive
            .members()
            .iter()
            .find(|m| m.path() == "second.txt")
            .checked_option("Unicode member")
            .bytes(),
        "完整内容\n".as_bytes()
    );
    assert!(
        archive
            .members()
            .iter()
            .all(|member| member.kind() == ArchiveEntryKind::File)
    );
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.recovery_records.is_empty())
    );
    assert!(
        !h.panel.read_with(cx, |panel, _| panel.window_close_busy()),
        "verified archive success must release its window-close hold"
    );
}

#[gpui_kit::test]
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
async fn archive_tar_gz_download_extract_reviews_empty_folders_and_publishes_new_tree(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bundle = ArchiveBundle::new(vec![
        ArchiveMember::new_directory("nested").checked("nested directory"),
        ArchiveMember::new_directory("empty").checked("empty directory"),
        ArchiveMember::new_file("nested/data.bin", vec![1, 0, 254]).checked("binary file"),
    ])
    .checked("bounded bundle");
    h.seed(
        "/download.tar.gz",
        &bundle.encode(ArchiveFormat::TarGz).checked("tar.gz bytes"),
    );
    remote_selection(&h, "/download.tar.gz", cx).await;
    let target = h.local.0.join("new-extraction");
    h.local_input(cx, &target);
    h.click(cx, "archive-format-tar-gz");
    h.click(cx, "review-archive-extract");
    h.idle(cx).await;
    let Operation::TransferArchive(plan) = reviewed(&h, cx) else {
        unreachable!()
    };
    assert!(
        plan.transfer
            .members()
            .iter()
            .any(|member| member.path() == "empty" && member.kind() == ArchiveEntryKind::Directory)
    );
    assert!(
        !target.exists(),
        "read-only preparation cannot create the target"
    );
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        std::fs::read(target.join("nested/data.bin")).checked("extracted bytes"),
        [1, 0, 254]
    );
    assert!(target.join("empty").is_dir());
    assert!(
        !h.panel.read_with(cx, |panel, _| panel.window_close_busy()),
        "verified extraction must release its window-close hold"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(target.join("nested/data.bin"))
                .checked("file mode")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&target)
                .checked("directory mode")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[gpui_kit::test]
async fn archive_cancel_and_selection_change_never_publish_an_old_approval(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("selected.txt", b"do not publish");
    browse_and_select(&h, &["selected.txt"], cx).await;
    name(&h, "cancel.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    let original = reviewed(&h, cx);
    h.click(cx, "cancel-file-operation");
    h.missing("/cancel.zip");
    h.click(cx, "clear-file-selection");
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.run(original, window, cx));
    })
    .checked("replay stale selection-bound archive approval");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.operation_id.is_none()
                && panel.pending.is_none())
    );
    h.missing("/cancel.zip");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn archive_changed_local_content_is_rejected_before_any_publication(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("source.txt", b"original bytes");
    browse_and_select(&h, &["source.txt"], cx).await;
    name(&h, "changed.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    reviewed(&h, cx);
    std::fs::write(source, b"modified bytes").checked("change reviewed source");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    h.missing("/changed.zip");
    assert!(
        !h.panel.read_with(cx, |panel, _| panel.window_close_busy()),
        "a source rejected before staging must not retain a close hold"
    );
    assert!(h.panel.read_with(cx, |panel, cx| {
        !panel.status.render(cx).contains("归档传输完成")
    }));
}

#[gpui_kit::test]
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
async fn archive_corrupt_download_and_existing_local_target_preserve_destination(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/corrupt.zip", b"not a ZIP archive");
    remote_selection(&h, "/corrupt.zip", cx).await;
    let target = h.local.0.join("new-target");
    h.local_input(cx, &target);
    h.click(cx, "review-archive-extract");
    h.idle(cx).await;
    assert!(!target.exists());
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()));

    let bytes = ArchiveBundle::new(vec![
        ArchiveMember::new_file("safe.txt", b"new".to_vec()).checked("safe member"),
    ])
    .checked("safe archive")
    .encode(ArchiveFormat::Zip)
    .checked("safe ZIP");
    h.seed("/safe.zip", &bytes);
    remote_selection(&h, "/safe.zip", cx).await;
    h.click(cx, "review-archive-extract");
    h.idle(cx).await;
    reviewed(&h, cx);
    std::fs::create_dir(&target).checked("concurrent existing destination");
    std::fs::write(target.join("keep.txt"), b"existing").checked("protected destination bytes");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        std::fs::read(target.join("keep.txt")).checked("preserved destination"),
        b"existing"
    );
    assert!(!target.join("safe.txt").exists());
}

#[gpui_kit::test]
async fn archive_draft_change_and_retired_session_reject_review(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.source("selected.txt", b"old host cannot write");
    browse_and_select(&h, &["selected.txt"], cx).await;
    name(&h, "old.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    let original = reviewed(&h, cx);
    name(&h, "new.zip", cx);
    cx.update_window(h.window, |_, window, cx| {
        h.panel
            .update(cx, |panel, cx| panel.run(original, window, cx));
    })
    .checked("reject stale destination draft");
    assert!(
        h.panel
            .read_with(cx, |panel, _| panel.operation_id.is_none())
    );
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    reviewed(&h, cx);
    cx.update_window(h.window, |_, _, cx| {
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
    })
    .checked("retire archive session");
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()));
    h.missing("/old.zip");
    h.missing("/new.zip");
}

#[gpui_kit::test]
async fn archive_pending_write_cancel_retains_close_hold_across_another_verified_upload(
    cx: &mut TestAppContext,
) {
    use crate::files::IsolationTarget;
    let h = Harness::new(cx);
    h.idle(cx).await;
    let mut state = 0x7c1c_0001_u32;
    let bytes: Vec<_> = (0..180_001)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    h.source("source.bin", &bytes);
    browse_and_select(&h, &["source.bin"], cx).await;
    name(&h, "unknown.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    reviewed(&h, cx);
    let hold = h
        .server
        .filesystem
        .hold_atomic_upload_after_first("/unknown.zip")
        .checked("hold an issued archive WRITE");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| hold.entered() == 1)
        .await;
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    assert!(!hold.expired());
    hold.release();
    h.missing("/unknown.zip");
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.window_close_busy(), "unknown writes must retain a close hold");
        assert!(matches!(&panel.isolation_target, Some(IsolationTarget::Remote(path)) if path == "/unknown.zip"));
        assert!(panel.status.render(cx).contains("未知"));
    });
    name(&h, "verified.zip", cx);
    h.click(cx, "review-archive-upload");
    h.idle(cx).await;
    reviewed(&h, cx);
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    ArchiveBundle::decode(ArchiveFormat::Zip, &h.read("/verified.zip"))
        .checked("verified later upload");
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.window_close_busy(), "success cannot retire an older unknown write");
        assert!(matches!(&panel.isolation_target, Some(IsolationTarget::Remote(path)) if path == "/unknown.zip"));
    });
    h.runtime.block_on(async {
        let sftp = h
            .session
            .sftp()
            .await
            .checked("inspect original unknown archive");
        assert!(
            !sftp
                .inspect_remote_mutation_quarantine("/unknown.zip")
                .await
                .checked("original quarantine")
                .entries()
                .is_empty()
        );
        sftp.close().await.checked("close quarantine inspection");
    });
}

#[gpui_kit::test]
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
async fn archive_large_manifest_keeps_last_member_and_fixed_review_actions(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let members = (0..128)
        .map(|index| {
            ArchiveMember::new_file(format!("entry-{index:03}.bin"), vec![index as u8])
                .checked("manifest member")
        })
        .collect();
    let bytes = ArchiveBundle::new(members)
        .checked("large manifest")
        .encode(ArchiveFormat::Zip)
        .checked("large ZIP");
    h.seed("/manifest.zip", &bytes);
    remote_selection(&h, "/manifest.zip", cx).await;
    let target = h.local.0.join("manifest-target");
    h.local_input(cx, &target);
    h.click(cx, "review-archive-extract");
    h.idle(cx).await;
    reviewed(&h, cx);
    h.panel.read_with(cx, |panel, _| {
        let (message, _) = panel
            .pending
            .as_ref()
            .checked_option("complete manifest review");
        for text in [message.translations().0, message.translations().1] {
            assert!(text.contains("entry-000.bin"));
            assert!(
                text.contains("entry-127.bin"),
                "review must not truncate after 100 members"
            );
        }
    });
    h.click(cx, "expand-file-review");
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for id in ["confirm-file-operation", "cancel-file-operation"] {
            let button = window.find(id);
            assert!(button.visible());
            assert!(button.bounds().bottom() <= window.bounds().bottom());
        }
    })
    .checked("long manifest keeps fixed review controls reachable");
    h.click(cx, "cancel-file-operation");
    assert!(!target.exists());
}

#[gpui_kit::test]
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
async fn archive_unsupported_extraction_is_disabled_without_creating_a_target(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = ArchiveBundle::new(vec![
        ArchiveMember::new_file("safe.txt", b"bytes".to_vec()).checked("safe member"),
    ])
    .checked("safe bundle")
    .encode(ArchiveFormat::Zip)
    .checked("safe archive");
    h.seed("/safe.zip", &bytes);
    remote_selection(&h, "/safe.zip", cx).await;
    let target = h.local.0.join("unsupported-target");
    h.local_input(cx, &target);
    cx.update_window(h.window, |_, window, cx| {
        reveal_file_control(window, cx, "review-archive-extract");
        assert_eq!(window.find("review-archive-extract").disabled(), Some(true));
        window.click("review-archive-extract", cx);
    })
    .checked("unsupported extraction is visibly disabled");
    assert!(!target.exists());
    assert!(h.panel.read_with(cx, |panel, _| panel.pending.is_none()
        && panel.operation_id.is_none()));
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}

#[gpui_kit::test]
async fn archive_controls_share_all_themes_and_locales_without_changing_drafts(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    name(&h, "retained.zip", cx);
    h.click(cx, "toggle-file-transfer-settings");
    let identity = h.panel.read_with(cx, |panel, _| {
        (panel.archive_name.entity_id(), panel.session_token)
    });
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    i18n::set_language(language, cx);
                    crate::design::apply_settings(
                        &Settings {
                            theme_family: ThemeId::new(family.id).checked("theme id"),
                            theme,
                            ..Settings::default()
                        },
                        Some(window),
                        cx,
                    );
                    window.render_frame(cx);
                    reveal_file_control(window, cx, "review-archive-upload");
                    for id in ["review-archive-upload", "review-archive-extract"] {
                        let button = window.find(id);
                        assert!(button.visible());
                        assert!(
                            button.bounds().left() >= window.bounds().left()
                                && button.bounds().right() <= window.bounds().right()
                        );
                    }
                    assert_eq!(
                        window.find("review-archive-upload").label(),
                        Some(if language == Language::ZhCn {
                            "审核打包上传"
                        } else {
                            "Review packed upload"
                        })
                    );
                })
                .checked("render archive actions through registry theme and locale");
                h.panel.read_with(cx, |panel, cx| {
                    assert_eq!(
                        (panel.archive_name.entity_id(), panel.session_token),
                        identity
                    );
                    assert_eq!(panel.archive_name.read(cx).value(), "retained.zip");
                    assert!(panel.pending.is_none());
                });
            }
        }
    }
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
