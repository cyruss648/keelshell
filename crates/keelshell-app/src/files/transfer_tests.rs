//! Real GPUI actions against controlled SSH/SFTP peers, with byte-level outcomes.
use super::{FilesPanel, Operation, TransferPhase};
use crate::i18n;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::Language;
use keelshell_session::{SshSession, sftp::RemoteEntry};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

#[path = "test_server.rs"]
mod test_server;
use test_server::{Checked, Server};

struct LocalDirectory(PathBuf);
impl Drop for LocalDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Harness {
    window: AnyWindowHandle,
    panel: Entity<FilesPanel>,
    session: SshSession,
    server: Server,
    runtime: Arc<tokio::runtime::Runtime>,
    local: LocalDirectory,
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("file UI runtime"),
        );
        let server = Server::new(&runtime);
        let session = server.connect(&runtime);
        let directory =
            std::env::temp_dir().join(format!("keelshell-file-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).checked("create isolated file fixture directory");
        let directory = directory
            .canonicalize()
            .checked("canonical temporary directory without /var symlink");
        let (window, panel) = mount(cx, session.clone(), runtime.clone());
        Self {
            window,
            panel,
            session,
            server,
            runtime,
            local: LocalDirectory(directory),
        }
    }
    fn seed(&self, remote: &str, bytes: &[u8]) {
        self.runtime.block_on(async {
            let sftp = self.session.sftp().await.checked("seed SFTP");
            sftp.write(remote, bytes).await.checked("seed remote bytes");
            sftp.close().await.checked("close seed SFTP");
        });
    }
    fn read(&self, remote: &str) -> Vec<u8> {
        self.runtime.block_on(async {
            let sftp = self.session.sftp().await.checked("inspect SFTP");
            let bytes = sftp
                .read(remote, 1024 * 1024)
                .await
                .checked("inspect remote bytes");
            sftp.close().await.checked("close inspection SFTP");
            bytes
        })
    }
    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.local.0.join(name);
        std::fs::write(&path, bytes).checked("write local source");
        path
    }
    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(12), |_, cx| {
            !self.panel.read(cx).busy
        })
        .await;
    }
    async fn phase(&self, cx: &mut TestAppContext, phase: TransferPhase) {
        cx.wait_for(self.window, Duration::from_secs(12), |_, cx| {
            self.panel
                .read(cx)
                .transfer
                .as_ref()
                .is_some_and(|state| state.phase == phase)
        })
        .await;
    }
    fn local_input(&self, cx: &mut TestAppContext, path: &std::path::Path) {
        cx.update_window(self.window, |_, window, cx| {
            self.panel.read(cx).local.clone().update(cx, |input, cx| {
                input.set_value(path.display().to_string(), window, cx)
            });
            window.render_frame(cx);
        })
        .checked("set local path draft");
    }
    fn click(&self, cx: &mut TestAppContext, id: &str) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(gpui_kit::SharedString::from(id.to_owned()), cx);
        })
        .checked("click real file control");
    }
}
fn mount(
    cx: &mut TestAppContext,
    session: SshSession,
    runtime: Arc<tokio::runtime::Runtime>,
) -> (AnyWindowHandle, Entity<FilesPanel>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1100.), px(440.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| FilesPanel::new(session, "fixture SSH".into(), runtime, window, cx))
            },
        )
        .checked("mount production FilesPanel")
    })
}
fn selected(path: &str, directory: bool) -> RemoteEntry {
    RemoteEntry {
        name: path.rsplit('/').next().unwrap_or(path).into(),
        path: path.into(),
        size: None,
        is_directory: directory,
        is_symlink: false,
        permissions: None,
        modified: None,
    }
}

#[gpui_kit::test]
async fn upload_pause_acknowledges_a_stable_boundary_and_continue_preserves_destination(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x4b; 768 * 1024];
    let source = h.source("upload.bin", &bytes);
    h.server.filesystem.set_transfer_write_delay(55);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|s| s.transferred > 0 && s.phase == TransferPhase::Running)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Pausing)
        )
    });
    h.phase(cx, TransferPhase::Paused).await;
    let paused = h.read("/upload.bin");
    assert!(!paused.is_empty() && paused.len() < bytes.len());
    let start = Instant::now();
    cx.wait_for(h.window, Duration::from_secs(2), |_, _| {
        start.elapsed() > Duration::from_millis(170)
    })
    .await;
    assert_eq!(
        h.read("/upload.bin"),
        paused,
        "acknowledged pause must not admit later writes"
    );
    cx.update_window(h.window, |_, window, cx| {
        for width in [1100., 760., 480.] {
            window.resize(size(px(width), px(440.)));
            window.bounds_changed(cx);
            for language in [Language::ZhCn, Language::En] {
                i18n::set_language(language, cx);
                window.render_frame(cx);
                let card = window.find("file-transfer-card").bounds();
                assert!(
                    card.right() <= window.bounds().right()
                        && card.bottom() <= window.bounds().bottom(),
                    "transfer card escaped {width}px window: {card:?}"
                );
                for id in [
                    "resume-file-transfer",
                    "cancel-active-file-operation",
                    "file-transfer-bytes",
                ] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.origin.x >= card.origin.x
                            && bounds.right() <= card.right()
                            && bounds.origin.y >= card.origin.y
                            && bounds.bottom() <= card.bottom(),
                        "{id} escaped {width}px card: {bounds:?}"
                    );
                    assert!(window.find(id).visible());
                }
            }
        }
    })
    .checked("paused transfer actions and exact byte counts stay inside small bilingual panels");
    cx.update_window(h.window, |_, window, cx| {
        i18n::set_language(Language::En, cx);
        h.panel
            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("resume-file-transfer").label(),
            Some("Continue")
        );
        assert_eq!(
            h.panel.read(cx).transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Paused)
        );
    })
    .checked("translate paused production transfer");
    // Editing the next path draft cannot redirect an already admitted transfer.
    h.local_input(cx, &h.local.0.join("unrelated-next-draft"));
    h.click(cx, "resume-file-transfer");
    h.idle(cx).await;
    assert_eq!(h.read("/upload.bin"), bytes);
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Completed)
        )
    });
}

#[gpui_kit::test]
async fn reviewed_permissions_change_uses_the_exact_selected_entry_and_refreshable_result(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.seed("/mode.txt", b"content");
    h.idle(cx).await;
    let mut entry = selected("/mode.txt", false);
    entry.permissions = Some(0o100644);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.selected = Some(entry.clone());
            panel.mode.update(cx, |input, cx| {
                input.set_value("0640", window, cx);
            });
        });
        window.render_frame(cx);
    })
    .checked("prepare reviewed permissions operation");
    h.click(cx, "chmod-file");
    cx.update_window(h.window, |_, _, cx| {
        assert!(matches!(
            h.panel.read(cx).pending,
            Some((_, Operation::SetPermissions(_, 0o640)))
        ));
    })
    .checked("inspect reviewed permissions confirmation");
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    let mode = h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect permissions");
        let mode = sftp
            .list("/")
            .await
            .checked("list permissions")
            .into_iter()
            .find(|entry| entry.path == "/mode.txt")
            .and_then(|entry| entry.permissions)
            .map(|mode| mode & 0o7777);
        sftp.close().await.checked("close permissions inspection");
        mode
    });
    assert_eq!(mode, Some(0o640));
    cx.update_window(h.window, |_, _, cx| {
        assert!(h.panel.read(cx).status.render(cx).contains("请刷新列表"));
    })
    .checked("inspect permission result");
}

#[gpui_kit::test]
async fn cancel_while_paused_is_bounded_and_does_not_cancel_another_panel(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x7a; 768 * 1024];
    let source = h.source("cancel.bin", &bytes);
    h.server.filesystem.set_transfer_write_delay(60);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|s| s.transferred > 0 && s.phase == TransferPhase::Running)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.phase(cx, TransferPhase::Paused).await;
    let (other_window, other) = mount(cx, h.server.connect(&h.runtime), h.runtime.clone());
    cx.wait_for(other_window, Duration::from_secs(8), |_, cx| {
        !other.read(cx).busy
    })
    .await;
    let sibling = h.source("sibling.bin", b"independent panel bytes");
    cx.update_window(other_window, |_, window, cx| {
        other.update(cx, |panel, cx| {
            panel.run(
                Operation::Upload(sibling, "/sibling.bin".into()),
                window,
                cx,
            )
        })
    })
    .checked("start unrelated panel transfer");
    let started = Instant::now();
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    assert!(started.elapsed() < Duration::from_secs(8));
    cx.wait_for(other_window, Duration::from_secs(8), |_, cx| {
        !other.read(cx).busy
    })
    .await;
    assert_eq!(h.read("/sibling.bin"), b"independent panel bytes");
    let partial = h.read("/cancel.bin");
    assert!(!partial.is_empty() && partial.len() < bytes.len());
    assert_eq!(&bytes[..partial.len()], partial);
    assert!(!h.session.is_closed());
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Cancelled)
        )
    });
}

#[gpui_kit::test]
async fn reconnect_review_and_confirm_resume_upload_keeps_the_reviewed_target(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x35; 320 * 1024];
    let source = h.source("different-local-name.bin", &bytes);
    h.seed("/partial.bin", &bytes[..8192]);
    // A fresh SSH session has no in-memory transfer job; explicit planning recovers the partial file.
    let (window, panel) = mount(cx, h.server.connect(&h.runtime), h.runtime.clone());
    cx.wait_for(window, Duration::from_secs(8), |_, cx| !panel.read(cx).busy)
        .await;
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, cx| {
            view.selected = Some(selected("/partial.bin", false));
            view.local.update(cx, |input, cx| {
                input.set_value(source.display().to_string(), window, cx)
            });
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("upload-file", cx);
    })
    .checked("request read-only resume review");
    cx.wait_for(window, Duration::from_secs(8), |_, cx| !panel.read(cx).busy)
        .await;
    assert_eq!(h.read("/partial.bin"), &bytes[..8192]);
    panel.read_with(cx, |view, _| {
        assert!(matches!(view.pending, Some((_, Operation::ResumeFile(_)))))
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |view, _| {
            view.selected = Some(selected("/unrelated.bin", false))
        });
        i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        window.click("confirm-file-operation", cx);
    })
    .checked("confirm frozen continuation destination");
    cx.wait_for(window, Duration::from_secs(8), |_, cx| !panel.read(cx).busy)
        .await;
    assert_eq!(h.read("/partial.bin"), bytes);
    panel.read_with(cx, |view, _| {
        let transfer = view
            .transfer
            .as_ref()
            .unwrap_or_else(|| panic!("resume status"));
        assert_eq!(transfer.phase, TransferPhase::Completed);
        assert_eq!(transfer.reviewed_existing, 8192);
        assert_eq!(transfer.transferred, 320 * 1024);
    });
}

#[gpui_kit::test]
async fn resume_mismatch_and_cancelled_review_never_write_or_fall_back_to_replacement(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let source = h.source("source.bin", b"matching full contents");
    h.seed("/different.bin", b"not a prefix");
    h.local_input(cx, &source);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/different.bin", false))
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("upload-file", cx);
    })
    .checked("review incompatible destination");
    h.idle(cx).await;
    assert_eq!(h.read("/different.bin"), b"not a prefix");
    h.panel
        .read_with(cx, |panel, _| assert!(panel.pending.is_none()));
    h.seed("/matching.bin", b"matching");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/matching.bin", false))
        });
        window.render_frame(cx);
        window.click("upload-file", cx);
    })
    .checked("review compatible destination");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert!(matches!(panel.pending, Some((_, Operation::ResumeFile(_)))))
    });
    h.click(cx, "cancel-file-operation");
    assert_eq!(h.read("/matching.bin"), b"matching");
    h.panel.read_with(cx, |panel, _| {
        assert!(panel.pending.is_none());
        assert!(!panel.busy);
    });
}

#[gpui_kit::test]
async fn explicit_download_continuation_preserves_partial_bytes_until_confirmation(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x6b; 144 * 1024];
    h.seed("/source.bin", &bytes);
    let destination = h.source("partial-download.bin", &bytes[..4096]);
    h.local_input(cx, &destination);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/source.bin", false))
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("download-file", cx);
    })
    .checked("review selected download continuation");
    h.idle(cx).await;
    assert_eq!(
        std::fs::read(&destination).checked("partial local bytes"),
        bytes[..4096]
    );
    h.click(cx, "confirm-file-operation");
    h.idle(cx).await;
    assert_eq!(
        std::fs::read(&destination).checked("completed local continuation"),
        bytes
    );
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Completed)
        )
    });
}

#[gpui_kit::test]
async fn directory_continuation_uses_reviewed_tree_and_finishes_missing_content(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let directory = h.local.0.join("tree");
    std::fs::create_dir(&directory).checked("local tree root");
    std::fs::create_dir(directory.join("empty")).checked("empty source folder");
    let large = vec![0x73; 192 * 1024];
    let later = vec![0x62; 192 * 1024];
    std::fs::write(directory.join("data.bin"), &large).checked("tree source file");
    std::fs::write(directory.join("later.bin"), &later).checked("second partial source file");
    std::fs::write(directory.join("complete.txt"), b"already complete")
        .checked("complete source file");
    h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("seed partial tree SFTP");
        sftp.mkdir("/partial-tree")
            .await
            .checked("partial tree root");
        sftp.close().await.checked("seed tree cleanup");
    });
    h.seed("/partial-tree/data.bin", &large[..5000]);
    h.seed("/partial-tree/later.bin", &later[..140_000]);
    h.seed("/partial-tree/complete.txt", b"already complete");
    h.local_input(cx, &directory);
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, _| {
            panel.selected = Some(selected("/partial-tree", true))
        });
        window.render_frame(cx);
        window.click("file-resume-mode", cx);
        window.click("upload-directory", cx);
    })
    .checked("review partial directory");
    h.idle(cx).await;
    assert_eq!(h.read("/partial-tree/data.bin"), large[..5000]);
    h.panel.read_with(cx, |panel, _| {
        assert!(matches!(
            panel.pending,
            Some((_, Operation::ResumeDirectory(_)))
        ))
    });
    h.server.filesystem.set_transfer_write_delay(75);
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel.read(cx).transfer.as_ref().is_some_and(|state| {
            state.transferred > 5016 && state.transferred < state.reviewed_existing
        })
    })
    .await;
    cx.update_window(h.window, |_, window, cx| {
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            let state = h
                .panel
                .read(cx)
                .transfer
                .as_ref()
                .unwrap_or_else(|| panic!("active directory continuation"));
            let label = state.bytes_label(cx);
            assert!(
                !label.contains("本次新增") && !label.contains("New"),
                "unprocessed prefixes cannot be subtracted from current directory progress: {label}"
            );
            assert!(
                label.contains("145016"),
                "reviewed existing bytes remain distinct: {label}"
            );
            assert!(window.find("file-transfer-card").visible());
        }
    })
    .checked("inspect truthful intermediate multi-file continuation progress");
    h.idle(cx).await;
    assert_eq!(h.read("/partial-tree/data.bin"), large);
    assert_eq!(h.read("/partial-tree/complete.txt"), b"already complete");
    assert_eq!(h.read("/partial-tree/later.bin"), later);
    let entries = h.runtime.block_on(async {
        let sftp = h.session.sftp().await.checked("inspect tree SFTP");
        let entries = sftp
            .list("/partial-tree")
            .await
            .checked("inspect final tree");
        sftp.close().await.checked("inspect tree cleanup");
        entries
    });
    assert!(
        entries
            .iter()
            .any(|entry| entry.name == "empty" && entry.is_directory)
    );
}

#[gpui_kit::test]
async fn closing_the_paused_file_panel_releases_its_subsystem_without_closing_ssh(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let bytes = vec![0x61; 768 * 1024];
    let source = h.source("close.bin", &bytes);
    h.server.filesystem.set_transfer_write_delay(60);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|s| s.transferred > 0 && s.phase == TransferPhase::Running)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.phase(cx, TransferPhase::Paused).await;
    let Harness {
        window,
        panel,
        session,
        server,
        runtime,
        local,
    } = h;
    let (other_window, other) = mount(cx, session.clone(), runtime.clone());
    cx.wait_for(other_window, Duration::from_secs(8), |_, cx| {
        !other.read(cx).busy
    })
    .await;
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close file window");
    drop(panel);
    cx.run_until_parked();
    cx.wait_for(other_window, Duration::from_secs(8), |_, _| {
        server.active.load(Ordering::Acquire) == 0
    })
    .await;
    assert!(!session.is_closed());
    let partial = runtime.block_on(async {
        let sftp = session.sftp().await.checked("inspect after panel close");
        let bytes = sftp
            .read("/close.bin", 1024 * 1024)
            .await
            .checked("partial close output");
        sftp.close().await.checked("inspection cleanup");
        bytes
    });
    assert!(!partial.is_empty() && partial.len() < bytes.len());
    drop(local);
}

#[gpui_kit::test]
async fn ordinary_folder_transfer_pause_and_cancel_preserve_partial_tree_status(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    let folder = h.local.0.join("ordinary-tree");
    std::fs::create_dir(&folder).checked("ordinary source tree");
    let data = vec![0x4e; 768 * 1024];
    std::fs::write(folder.join("data.bin"), &data).checked("ordinary tree source");
    h.server.filesystem.set_transfer_write_delay(55);
    h.local_input(cx, &folder);
    h.click(cx, "upload-directory");
    h.idle(cx).await;
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|s| s.transferred > 0 && s.phase == TransferPhase::Running)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.phase(cx, TransferPhase::Paused).await;
    h.click(cx, "cancel-active-file-operation");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.transfer.as_ref().map(|s| s.phase),
            Some(TransferPhase::Cancelled)
        )
    });
    let partial = h.read("/ordinary-tree/data.bin");
    assert!(!partial.is_empty() && partial.len() < data.len());
    assert_eq!(partial, &data[..partial.len()]);
    assert!(!h.session.is_closed());
}

#[gpui_kit::test]
async fn suspended_paused_transfer_waits_for_cancel_ack_and_keeps_unsaved_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/config.txt", b"original\n");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected("/config.txt", false)), window, cx)
        });
    })
    .checked("load actual remote editor baseline");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("unsaved 中文 draft\n", window, cx)
        });
        assert!(h.panel.read(cx).has_unsaved_draft(cx));
    })
    .checked("edit an unsaved local draft");
    let source = h.source("suspend.bin", &vec![0x72; 768 * 1024]);
    h.server.filesystem.set_transfer_write_delay(55);
    h.local_input(cx, &source);
    h.click(cx, "upload-file");
    h.click(cx, "confirm-file-operation");
    cx.wait_for(h.window, Duration::from_secs(8), |_, cx| {
        h.panel
            .read(cx)
            .transfer
            .as_ref()
            .is_some_and(|state| state.transferred > 0)
    })
    .await;
    h.click(cx, "pause-file-transfer");
    h.phase(cx, TransferPhase::Paused).await;
    let partial = h.read("/suspend.bin");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            assert!(panel.suspended && panel.session.is_none());
            assert!(
                panel.busy,
                "suspend requests cancellation; it cannot invent the ACK"
            );
            assert_eq!(
                panel.transfer.as_ref().map(|state| state.phase),
                Some(TransferPhase::Cancelling)
            );
            assert!(
                panel
                    .operation_stop
                    .as_ref()
                    .is_some_and(|stop| stop.load(Ordering::Acquire))
            );
            assert!(panel.pending.is_none());
            panel.confirm(
                crate::i18n::Message::new("禁止", "Blocked"),
                Operation::Upload(source.clone(), "/replay.bin".into()),
                cx,
            );
            panel.execute_pending(window, cx);
            assert!(panel.pending.is_none());
        });
    })
    .checked("suspend paused transfer without claiming cleanup or retaining authority");
    h.idle(cx).await;
    h.phase(cx, TransferPhase::Cancelled).await;
    cx.wait_for(h.window, Duration::from_secs(5), |_, _| {
        h.server.active.load(Ordering::Acquire) == 0
    })
    .await;
    let writes = h.server.filesystem.transfer_writes_started();
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "unsaved 中文 draft\n"
            );
            assert_eq!(
                panel.editing,
                Some(("/config.txt".into(), b"original\n".to_vec()))
            );
            assert!(panel.has_unsaved_draft(cx));
            panel.run(
                Operation::Upload(source.clone(), "/replay.bin".into()),
                window,
                cx,
            );
            assert!(!panel.busy);
            assert!(panel.operation_id.is_none());
        });
    })
    .checked("archived draft and original baseline survive; transfer cannot be replayed");
    assert_eq!(h.read("/suspend.bin"), partial);
    assert_eq!(h.server.filesystem.transfer_writes_started(), writes);
    assert_eq!(h.read("/config.txt"), b"original\n");
    assert!(!h.session.is_closed());
}

/// Keep the UI turn occupied while the real worker computes its result. The
/// remaining owners are the panel, registry and this local guard; the worker's
/// cancellation Arc disappears only after `operate` has returned. Its queued
/// result therefore reaches the panel after suspension, not after cancellation.
fn wait_for_computed_file_result(panel: &FilesPanel) {
    let stop = panel
        .operation_stop
        .as_ref()
        .checked_option("live worker")
        .clone();
    let start = Instant::now();
    while Arc::strong_count(&stop) > 3 {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "file worker did not compute its result"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

trait CheckedOption<T> {
    fn checked_option(self, action: &str) -> T;
}
impl<T> CheckedOption<T> for Option<T> {
    fn checked_option(self, action: &str) -> T {
        self.unwrap_or_else(|| panic!("{action}"))
    }
}

#[gpui_kit::test]
async fn suspension_discards_successful_late_read_and_resume_plan_without_changing_archive(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/late.txt", b"must not replace archived text\n");
    let source = h.source("resume.bin", b"prefix and remaining bytes");
    h.seed("/resume.bin", b"prefix");
    for operation in [
        Operation::Read(selected("/late.txt", false)),
        Operation::PlanResume(
            keelshell_session::sftp::TransferSpec::upload(&source, "/resume.bin"),
            false,
        ),
    ] {
        let (window, panel) = mount(cx, h.session.clone(), h.runtime.clone());
        cx.wait_for(window, Duration::from_secs(5), |_, cx| !panel.read(cx).busy)
            .await;
        cx.update_window(window, |_, window, cx| {
            panel.read(cx).editor.clone().update(cx, |input, cx| {
                input.set_value("archived local draft", window, cx)
            });
            panel.update(cx, |panel, cx| panel.run(operation, window, cx));
            wait_for_computed_file_result(panel.read(cx));
            panel.update(cx, |panel, cx| panel.suspend(cx));
        })
        .checked("suspend after successful worker computation but before UI delivery");
        cx.wait_for(window, Duration::from_secs(5), |_, cx| !panel.read(cx).busy)
            .await;
        panel.read_with(cx, |panel, cx| {
            assert!(
                panel.status.render(cx).contains("已完成"),
                "fixture must deliver success, not merely a cancellation: {}",
                panel.status.render(cx)
            );
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "archived local draft"
            );
            assert!(panel.editing.is_none());
            assert!(panel.has_unsaved_draft(cx));
            assert!(
                panel.pending.is_none(),
                "late reviewed plans cannot regain remote authority"
            );
            assert!(panel.session.is_none());
        });
    }
    assert_eq!(h.read("/resume.bin"), b"prefix");
}

#[gpui_kit::test]
async fn late_save_acknowledges_remote_commit_without_rewriting_the_archived_baseline(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/saved.txt", b"original");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected("/saved.txt", false)), window, cx)
        });
    })
    .checked("open remote file for reviewed save");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("reviewed version", window, cx)
        });
        window.render_frame(cx);
        window.click("save-remote-file", cx);
        window.render_frame(cx);
        window.click("confirm-file-operation", cx);
        wait_for_computed_file_result(h.panel.read(cx));
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("newer local draft", window, cx)
        });
        h.panel.update(cx, |panel, cx| panel.suspend(cx));
    })
    .checked("retire session after remote save completes but before callback applies");
    h.idle(cx).await;
    h.panel.read_with(cx, |panel, cx| {
        assert!(panel.status.render(cx).contains("已完成"));
        assert_eq!(panel.editor.read(cx).value().as_str(), "newer local draft");
        assert_eq!(
            panel.editing,
            Some(("/saved.txt".into(), b"original".to_vec()))
        );
        assert!(panel.has_unsaved_draft(cx));
        assert!(panel.pending.is_none() && panel.session.is_none());
    });
    assert_eq!(h.read("/saved.txt"), b"reviewed version");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 1);
}

#[gpui_kit::test]
async fn suspended_editor_revokes_an_existing_save_review_without_losing_the_draft(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.idle(cx).await;
    h.seed("/review.txt", b"original");
    cx.update_window(h.window, |_, window, cx| {
        h.panel.update(cx, |panel, cx| {
            panel.run(Operation::Read(selected("/review.txt", false)), window, cx)
        });
    })
    .checked("read the remote baseline for save review");
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        h.panel.read(cx).editor.clone().update(cx, |input, cx| {
            input.set_value("reviewed but not sent", window, cx)
        });
        window.render_frame(cx);
        window.click("save-remote-file", cx);
        assert!(matches!(
            h.panel.read(cx).pending,
            Some((_, Operation::Save { .. }))
        ));
        h.panel.update(cx, |panel, cx| {
            panel.suspend(cx);
            assert!(panel.pending.is_none());
            panel.execute_pending(window, cx);
            assert!(!panel.busy && panel.operation_id.is_none());
            assert_eq!(
                panel.editor.read(cx).value().as_str(),
                "reviewed but not sent"
            );
            assert!(panel.has_unsaved_draft(cx));
        });
        window.render_frame(cx);
        assert!(window.try_find("confirm-file-operation").is_none());
    })
    .checked("revoke the already displayed approval and preserve its unsent draft");
    assert_eq!(h.read("/review.txt"), b"original");
    assert_eq!(h.server.filesystem.atomic_writes_started(), 0);
}
