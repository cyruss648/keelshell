//! Real GPUI controls with a deterministic bridge; no protocol/native claim.
use super::*;
use ::core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;
use keelshell_core::{Language, Settings, Theme, ThemeId};
use keelshell_session::zmodem::ZmodemFileReview;
use std::sync::mpsc::{Receiver, SyncSender};

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, operation: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{operation}: {error:?}"),
        }
    }
}

impl<T> Checked<T> for Option<T> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|| panic!("{operation}: missing value"))
    }
}

struct Harness {
    window: AnyWindowHandle,
    terminal: Entity<TerminalView>,
    output: SyncSender<SessionEvent>,
    commands: Receiver<TerminalCommand>,
}

impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        cx.update(gpui_kit::init);
        let (output, incoming) = mpsc::sync_channel(128);
        let (outgoing, commands) = mpsc::sync_channel(128);
        let (window, terminal) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(900.), px(580.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|cx| {
                        let mut view = TerminalView::from_transport(
                            "Remote title".into(),
                            14.,
                            1_000,
                            incoming,
                            outgoing,
                            Arc::new(AtomicBool::new(false)),
                            cx,
                        );
                        view.attach_transport_target("operator@reviewed.example:2222".into());
                        view
                    })
                },
            )
            .checked("mount the production terminal and Zmodem card")
        });
        Self {
            window,
            terminal,
            output,
            commands,
        }
    }

    fn event(&self, event: ZmodemUiEvent, cx: &mut TestAppContext) {
        self.output
            .send(SessionEvent::Zmodem(event))
            .checked("queue a bridge notification");
        self.terminal.update(cx, |view, cx| view.poll(cx));
    }

    fn bridge_events(
        &self,
        events: impl IntoIterator<Item = ZmodemUiEvent>,
        cx: &mut TestAppContext,
    ) {
        self.terminal.update(cx, |view, cx| {
            for event in events {
                view.zmodem
                    .safety
                    .publish(event)
                    .checked("publish through the production bridge mailbox");
            }
            view.poll(cx);
        });
    }

    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find(id).visible(), "{id} must be reachable");
            window.click(id.to_owned(), cx);
            window.render_frame(cx);
        })
        .checked("dispatch the actual transfer control");
    }

    fn controls(&self) -> Vec<ZmodemControl> {
        self.commands
            .try_iter()
            .filter_map(|command| match command {
                TerminalCommand::Zmodem(control) => Some(*control),
                TerminalCommand::Write(_)
                | TerminalCommand::Resize(_, _)
                | TerminalCommand::ResumeInput(_) => None,
            })
            .collect()
    }

    fn review(&self, id: Uuid, review: Uuid, count: usize, cx: &mut TestAppContext) {
        self.event(ZmodemUiEvent::UploadRequested { id }, cx);
        self.event(
            ZmodemUiEvent::Review {
                transfer: id,
                review,
                direction: ZmodemDirection::Upload,
                files: (0..count)
                    .map(|index| ZmodemFileReview {
                        path: std::env::temp_dir().join(format!("reviewed-源-{index}.bin")),
                        name: format!("reviewed-源-{index}.bin"),
                        size: 1024,
                        sha256: Some("01".repeat(32)),
                    })
                    .collect(),
                destination: None,
            },
            cx,
        );
    }
}

#[gpui_kit::test]
fn zmodem_review_binds_the_actual_endpoint_despite_remote_osc_titles(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output
        .send(SessionEvent::Data(
            b"\x1b]0;different@untrusted.example\x07".to_vec(),
        ))
        .checked("queue an untrusted remote window title");
    let id = Uuid::new_v4();
    let review = Uuid::new_v4();
    h.review(id, review, 2, cx);
    h.terminal.read_with(cx, |view, _| {
        assert_eq!(view.title, "different@untrusted.example");
        let summary = view
            .zmodem
            .current
            .as_ref()
            .map(|current| current.summary.translations());
        let Some((zh, en)) = summary else {
            panic!("complete review missing");
        };
        for text in [zh, en] {
            assert!(text.contains("operator@reviewed.example:2222"));
            assert!(text.contains("reviewed-源-0.bin"));
            assert!(text.contains("reviewed-源-1.bin"));
            assert!(text.contains(&"01".repeat(32)));
            assert!(!text.contains("different@untrusted.example"));
        }
    });
    assert!(h.controls().is_empty());
    h.click("zmodem-confirm", cx);
    assert_eq!(
        h.controls(),
        vec![ZmodemControl::Confirm {
            transfer: id,
            review
        }]
    );
}

#[gpui_kit::test]
fn zmodem_retained_confirm_cannot_authorize_a_replaced_offer(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let first = Uuid::new_v4();
    let review = Uuid::new_v4();
    h.review(first, review, 1, cx);
    let second = Uuid::new_v4();
    h.event(ZmodemUiEvent::UploadRequested { id: second }, cx);
    h.terminal.update(cx, |view, cx| {
        view.zmodem_control(
            ZmodemControl::Confirm {
                transfer: first,
                review,
            },
            cx,
        )
    });
    assert!(h.controls().is_empty());
    h.click("zmodem-cancel", cx);
    assert_eq!(
        h.controls(),
        vec![ZmodemControl::Cancel { transfer: second }]
    );
}

#[gpui_kit::test]
fn zmodem_live_lease_blocks_input_before_its_ui_notification_is_read(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.terminal.update(cx, |view, _| {
        view.pending
            .write(b"stale queued command\n".to_vec())
            .checked("queue stale input");
        view.zmodem.safety.active.store(true, Ordering::Release);
        assert!(view.zmodem_close_busy());
        assert!(view.try_write(b"AI command\n".to_vec()).is_err());
        view.write(b"typed text".to_vec());
        view.paste("pasted text\n");
        view.flush_input();
    });
    assert!(
        !h.commands
            .try_iter()
            .any(|command| matches!(command, TerminalCommand::Write(_)))
    );
}

#[gpui_kit::test]
fn zmodem_completion_requests_exact_input_handoff_without_replaying_old_bytes(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let id = Uuid::new_v4();
    h.event(ZmodemUiEvent::UploadRequested { id }, cx);
    h.terminal.update(cx, |view, _| {
        view.zmodem.safety.active.store(true, Ordering::Release);
        assert!(view.try_write(b"must not replay\n".to_vec()).is_err());
    });
    h.event(ZmodemUiEvent::Finished { id }, cx);
    let commands: Vec<_> = h.commands.try_iter().collect();
    assert!(commands.contains(&TerminalCommand::ResumeInput(id)));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, TerminalCommand::Write(_)))
    );
    h.terminal.update(cx, |view, _| {
        assert!(view.zmodem_close_busy());
        // Only the bridge may release this live latch after reading the exact
        // ResumeInput. The UI's Finished event alone cannot release it.
        view.zmodem.safety.active.store(false, Ordering::Release);
        assert!(!view.zmodem_close_busy());
    });
}

fn assert_preoffer_handoff(id: Uuid, event: ZmodemUiEvent, cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.terminal.update(cx, |view, _| {
        view.pending
            .write(b"queued before ZRQINIT\n".to_vec())
            .checked("queue input before the receive lease");
        // The real bridge acquires this latch at ZRQINIT, before any ZFILE
        // notification exists. Only its exact handoff can release the latch.
        view.zmodem.safety.active.store(true, Ordering::Release);
        assert!(view.zmodem.current.is_none());
        assert!(
            view.try_write(b"blocked during receive\n".to_vec())
                .is_err()
        );
        assert!(view.zmodem_close_busy());
    });
    h.bridge_events([event], cx);
    let commands: Vec<_> = h
        .commands
        .try_iter()
        .filter(|command| !matches!(command, TerminalCommand::Resize(_, _)))
        .collect();
    assert_eq!(commands, vec![TerminalCommand::ResumeInput(id)]);
    h.terminal.update(cx, |view, _| {
        assert!(view.zmodem.current.is_none());
        assert!(view.zmodem.safety.active.load(Ordering::Acquire));
        assert!(view.zmodem_close_busy());
        assert!(
            view.try_write(b"blocked before handoff\n".to_vec())
                .is_err()
        );
        view.zmodem.safety.active.store(false, Ordering::Release);
        assert!(!view.zmodem_close_busy());
        view.try_write(b"fresh after handoff\n".to_vec())
            .checked("admit only fresh input after the bridge releases ownership");
    });
    assert_eq!(
        h.commands.try_iter().collect::<Vec<_>>(),
        vec![TerminalCommand::Write(b"fresh after handoff\n".to_vec())]
    );
}

#[gpui_kit::test]
fn zmodem_preoffer_cancelled_requests_exact_handoff(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    assert_preoffer_handoff(
        id,
        ZmodemUiEvent::Cancelled {
            id,
            uncertain: false,
        },
        cx,
    );
}

#[gpui_kit::test]
fn zmodem_preoffer_failed_requests_exact_handoff(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    assert_preoffer_handoff(
        id,
        ZmodemUiEvent::Failed {
            id,
            reason: ZmodemFailure::Protocol,
            uncertain: false,
        },
        cx,
    );
}

#[gpui_kit::test]
fn zmodem_preoffer_finished_requests_exact_handoff(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    assert_preoffer_handoff(id, ZmodemUiEvent::Finished { id }, cx);
}

#[gpui_kit::test]
fn zmodem_preoffer_handoffs_survive_backpressure_and_stale_ids(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let id = Uuid::new_v4();
    let stale = Uuid::new_v4();
    h.terminal.update(cx, |view, _| {
        view.zmodem.safety.active.store(true, Ordering::Release);
        while view
            .outgoing
            .try_send(TerminalCommand::Resize(24, 80))
            .is_ok()
        {}
    });
    h.bridge_events(
        [
            ZmodemUiEvent::Finished { id },
            ZmodemUiEvent::Cancelled {
                id: stale,
                uncertain: false,
            },
            ZmodemUiEvent::Finished { id },
        ],
        cx,
    );
    h.terminal.read_with(cx, |view, _| {
        assert_eq!(view.zmodem.pending_resumes, VecDeque::from([id, stale]));
        assert!(view.zmodem_close_busy());
    });
    assert!(
        h.commands
            .try_iter()
            .all(|command| matches!(command, TerminalCommand::Resize(_, _)))
    );
    h.terminal.update(cx, |view, cx| view.poll(cx));
    assert_eq!(
        h.commands
            .try_iter()
            .filter(|command| !matches!(command, TerminalCommand::Resize(_, _)))
            .collect::<Vec<_>>(),
        vec![
            TerminalCommand::ResumeInput(id),
            TerminalCommand::ResumeInput(stale)
        ]
    );
    h.terminal.read_with(cx, |view, _| {
        assert!(view.zmodem.pending_resumes.is_empty());
        assert!(view.zmodem.safety.active.load(Ordering::Acquire));
    });
}

#[gpui_kit::test]
fn zmodem_stale_terminal_ids_cannot_end_or_unlock_the_current_offer(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let old = Uuid::new_v4();
    let current = Uuid::new_v4();
    let offer = Uuid::new_v4();
    h.terminal.update(cx, |view, _| {
        view.zmodem.safety.active.store(true, Ordering::Release)
    });
    h.bridge_events(
        [ZmodemUiEvent::DownloadOffered {
            id: current,
            offer,
            name: "current.bin".into(),
            size: 8,
        }],
        cx,
    );
    let summary = h.terminal.read_with(cx, |view, _| {
        view.zmodem
            .current
            .as_ref()
            .map(|current| current.summary.clone())
    });
    h.bridge_events(
        [
            ZmodemUiEvent::Cancelled {
                id: old,
                uncertain: false,
            },
            ZmodemUiEvent::Failed {
                id: old,
                reason: ZmodemFailure::Protocol,
                uncertain: false,
            },
            ZmodemUiEvent::Finished { id: old },
            ZmodemUiEvent::ActionRejected {
                id: old,
                reason: ZmodemFailure::Stale,
            },
        ],
        cx,
    );
    let handoffs: Vec<_> = h
        .commands
        .try_iter()
        .filter(|command| !matches!(command, TerminalCommand::Resize(_, _)))
        .collect();
    assert!(
        !handoffs.is_empty()
            && handoffs
                .iter()
                .all(|command| *command == TerminalCommand::ResumeInput(old))
    );
    h.terminal.update(cx, |view, _| {
        let active = view.zmodem.current.as_ref().checked("current offer remains visible");
        assert_eq!(active.id, current);
        assert!(matches!(active.stage, Stage::Download { offer: active_offer, .. } if active_offer == offer));
        assert_eq!(Some(&active.summary), summary.as_ref());
        assert!(view.zmodem.safety.active.load(Ordering::Acquire));
        assert!(view.zmodem_close_busy());
        assert!(view.try_write(b"stale handoff cannot authorize input\n".to_vec()).is_err());
    });
    h.bridge_events([ZmodemUiEvent::Finished { id: current }], cx);
    assert_eq!(
        h.commands.try_iter().collect::<Vec<_>>(),
        vec![TerminalCommand::ResumeInput(current)]
    );
}

#[gpui_kit::test]
fn zmodem_cancel_submits_once_while_waiting_for_its_terminal_event(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let id = Uuid::new_v4();
    h.review(id, Uuid::new_v4(), 1, cx);
    h.click("zmodem-confirm", cx);
    assert!(matches!(
        h.controls().as_slice(),
        [ZmodemControl::Confirm { .. }]
    ));
    h.click("zmodem-cancel", cx);
    h.click("zmodem-cancel", cx);
    h.terminal.update(cx, |view, cx| {
        // A callback retained from a prior frame must obey the same guard.
        view.zmodem_control(ZmodemControl::Cancel { transfer: id }, cx);
    });
    assert_eq!(h.controls(), vec![ZmodemControl::Cancel { transfer: id }]);
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("zmodem-cancel").disabled(), Some(true));
    })
    .checked("observe pending cancellation on the real control");
}

#[gpui_kit::test]
fn zmodem_late_stale_rejection_preserves_each_terminal_result(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    for terminal in [
        ZmodemUiEvent::Cancelled {
            id,
            uncertain: false,
        },
        ZmodemUiEvent::Failed {
            id,
            reason: ZmodemFailure::Protocol,
            uncertain: false,
        },
        ZmodemUiEvent::Finished { id },
    ] {
        let h = Harness::new(cx);
        h.review(id, Uuid::new_v4(), 1, cx);
        h.terminal.update(cx, |view, _| {
            view.zmodem.safety.active.store(true, Ordering::Release);
        });
        h.click("zmodem-cancel", cx);
        assert_eq!(h.controls(), vec![ZmodemControl::Cancel { transfer: id }]);
        h.bridge_events([terminal], cx);
        let feedback = h.terminal.read_with(cx, |view, _| {
            view.zmodem
                .current
                .as_ref()
                .map(|current| current.feedback.clone())
        });
        assert_eq!(
            h.commands.try_iter().collect::<Vec<_>>(),
            vec![TerminalCommand::ResumeInput(id)]
        );
        h.terminal.update(cx, |view, _| {
            view.zmodem.safety.active.store(false, Ordering::Release);
        });
        h.bridge_events(
            [
                ZmodemUiEvent::ActionRejected {
                    id,
                    reason: ZmodemFailure::Stale,
                },
                ZmodemUiEvent::Progress {
                    id,
                    bytes: 1,
                    total: 2,
                },
                ZmodemUiEvent::InputRejected { id },
                ZmodemUiEvent::Review {
                    transfer: id,
                    review: Uuid::new_v4(),
                    direction: ZmodemDirection::Upload,
                    files: Vec::new(),
                    destination: None,
                },
            ],
            cx,
        );
        h.terminal.update(cx, |view, cx| {
            view.zmodem_control(ZmodemControl::Cancel { transfer: id }, cx);
            let current = view
                .zmodem
                .current
                .as_ref()
                .checked("retained terminal receipt");
            assert!(matches!(current.stage, Stage::Ended));
            assert_eq!(Some(&current.feedback), feedback.as_ref());
            assert!(!view.zmodem_input_blocked());
            assert!(!view.zmodem_close_busy());
            view.try_write(b"fresh after retirement\n".to_vec())
                .checked("late rejection must not restore the input fence");
        });
        assert_eq!(
            h.commands.try_iter().collect::<Vec<_>>(),
            vec![TerminalCommand::Write(b"fresh after retirement\n".to_vec())]
        );
        cx.update_window(h.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("zmodem-dismiss").visible());
            assert!(window.try_find("zmodem-cancel").is_none());
            assert!(window.try_find("zmodem-choose-upload").is_none());
        })
        .checked("ended UI cannot be revived by late notifications");
    }
}

#[gpui_kit::test]
fn zmodem_verified_saves_survive_next_offer_finished_and_dismissal(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let id = Uuid::new_v4();
    let next_offer = Uuid::new_v4();
    let first_path = std::env::temp_dir().join("已验证-original.bin");
    let second_path = std::env::temp_dir().join("已验证-second.bin");
    let first_digest = "12".repeat(32);
    let second_digest = "ab".repeat(32);
    h.terminal.update(cx, |view, _| {
        view.zmodem.safety.active.store(true, Ordering::Release);
    });
    h.bridge_events(
        [
            ZmodemUiEvent::DownloadOffered {
                id,
                offer: Uuid::new_v4(),
                name: "original.bin".into(),
                size: 4107,
            },
            ZmodemUiEvent::FileSaved {
                id,
                path: first_path.clone(),
                bytes: 4107,
                sha256: first_digest.clone(),
            },
            ZmodemUiEvent::DownloadOffered {
                id,
                offer: next_offer,
                name: "second.bin".into(),
                size: 8294,
            },
        ],
        cx,
    );
    h.terminal.read_with(cx, |view, _| {
        let current = view.zmodem.current.as_ref().checked("next file offer");
        assert!(matches!(current.stage, Stage::Download { offer, .. } if offer == next_offer));
        let saved = view
            .zmodem
            .verified_saves
            .front()
            .checked("first file receipt");
        assert_eq!(
            (&saved.path, saved.bytes, &saved.sha256),
            (&first_path, 4107, &first_digest)
        );
    });
    // No intervening frame may be needed to retain either real save receipt.
    h.bridge_events(
        [
            ZmodemUiEvent::FileSaved {
                id,
                path: second_path.clone(),
                bytes: 8294,
                sha256: second_digest.clone(),
            },
            ZmodemUiEvent::Finished { id },
        ],
        cx,
    );
    let expected = vec![
        (id, first_path, 4107, first_digest),
        (id, second_path, 8294, second_digest),
    ];
    h.terminal.update(cx, |view, _| {
        assert_eq!(
            view.zmodem
                .verified_saves
                .iter()
                .map(|saved| (
                    saved.id,
                    saved.path.clone(),
                    saved.bytes,
                    saved.sha256.clone()
                ))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(matches!(
            view.zmodem.current.as_ref().map(|current| &current.stage),
            Some(Stage::Ended)
        ));
        view.zmodem.safety.active.store(false, Ordering::Release);
        assert!(
            !view.zmodem_close_busy(),
            "verified history is not an input or close hold"
        );
    });
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(h.window, |_, window, cx| {
            crate::i18n::set_language(language, cx);
            window.render_frame(cx);
            for (index, (_, path, bytes, digest)) in expected.iter().enumerate() {
                let receipt = window.find(("zmodem-verified-save", index));
                let text = receipt.label().checked("complete verified-save UI label");
                assert!(text.contains(path.to_string_lossy().as_ref()));
                assert!(text.contains(&format!("\n{bytes} ")));
                assert!(text.contains(digest));
                assert!(text.contains(match language {
                    Language::ZhCn => "已验证保存",
                    Language::En => "Verified saved file",
                }));
            }
        })
        .checked("render complete bilingual save receipts after protocol completion");
    }
    h.click("zmodem-dismiss", cx);
    h.bridge_events(
        [ZmodemUiEvent::DownloadOffered {
            id: Uuid::new_v4(),
            offer: Uuid::new_v4(),
            name: "later-transfer.bin".into(),
            size: 0,
        }],
        cx,
    );
    h.terminal.read_with(cx, |view, _| {
        assert_eq!(
            view.zmodem
                .verified_saves
                .iter()
                .map(|saved| (
                    saved.id,
                    saved.path.clone(),
                    saved.bytes,
                    saved.sha256.clone()
                ))
                .collect::<Vec<_>>(),
            expected
        );
    });
    cx.update_window(h.window, |_, window, cx| {
        window.render_frame(cx);
        for (index, (_, path, _, digest)) in expected.iter().enumerate() {
            let receipt = window.find(("zmodem-verified-save", index));
            let text = receipt
                .label()
                .checked("previous transfer's original receipt");
            assert!(text.contains(path.to_string_lossy().as_ref()));
            assert!(text.contains(digest));
        }
    })
    .checked("a new transfer still exposes the retained receipt history");
    assert!(
        h.controls().is_empty(),
        "observing receipts grants no file or protocol action"
    );
}

#[gpui_kit::test]
fn zmodem_verified_save_history_preserves_complete_recent_receipts_with_a_bound(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let expected: Vec<_> = (0..35_u64)
        .map(|index| {
            (
                Uuid::new_v4(),
                std::env::temp_dir().join(format!("verified-文件-{index}.bin")),
                4096 + index,
                format!("{index:064x}"),
            )
        })
        .collect();
    for (id, path, bytes, sha256) in &expected {
        h.bridge_events(
            [
                ZmodemUiEvent::DownloadOffered {
                    id: *id,
                    offer: Uuid::new_v4(),
                    name: "verified.bin".into(),
                    size: *bytes,
                },
                ZmodemUiEvent::FileSaved {
                    id: *id,
                    path: path.clone(),
                    bytes: *bytes,
                    sha256: sha256.clone(),
                },
                ZmodemUiEvent::Finished { id: *id },
            ],
            cx,
        );
    }
    let (id, path, bytes, sha256) = expected.last().checked("latest verified file");
    h.bridge_events(
        [ZmodemUiEvent::FileSaved {
            id: *id,
            path: path.clone(),
            bytes: *bytes,
            sha256: sha256.clone(),
        }],
        cx,
    );
    h.terminal.read_with(cx, |view, _| {
        assert_eq!(
            view.zmodem
                .verified_saves
                .iter()
                .map(|saved| (
                    saved.id,
                    saved.path.clone(),
                    saved.bytes,
                    saved.sha256.clone()
                ))
                .collect::<Vec<_>>(),
            expected[3..],
            "keep the complete newest 32 receipts in order; duplicate delivery cannot evict one"
        );
    });
}

#[gpui_kit::test]
fn zmodem_unknown_acknowledgement_waits_for_the_real_publication_latch(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let id = Uuid::new_v4();
    let publication = Arc::new(AtomicBool::new(true));
    h.terminal.update(cx, |view, _| {
        view.attach_zmodem_safety(Arc::new(Safety::new(publication.clone())));
        view.zmodem.safety.active.store(true, Ordering::Release);
    });
    h.bridge_events(
        [ZmodemUiEvent::Cancelled {
            id,
            uncertain: true,
        }],
        cx,
    );
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.update(cx, |view, _| {
        assert!(view.zmodem.unresolved.contains_key(&id));
        assert!(view.zmodem.acknowledge_review.is_none());
        view.zmodem.safety.active.store(false, Ordering::Release);
    });
    h.click("zmodem-acknowledge-unknown", cx);
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.read_with(cx, |view, _| {
        assert!(publication.load(Ordering::Acquire));
        assert!(view.zmodem.unresolved.contains_key(&id));
        assert!(view.zmodem.acknowledge_review.is_none());
        assert_eq!(
            view.zmodem
                .safety
                .acknowledged_epoch
                .load(Ordering::Acquire),
            0
        );
        assert!(view.zmodem_close_busy());
    });
    publication.store(false, Ordering::Release);
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.read_with(cx, |view, _| {
        assert!(view.zmodem.acknowledge_review.is_some());
        assert!(view.zmodem.unresolved.contains_key(&id));
        assert!(view.zmodem_close_busy());
    });
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.read_with(cx, |view, _| {
        assert!(view.zmodem.unresolved.is_empty());
        assert_eq!(
            view.zmodem
                .safety
                .acknowledged_epoch
                .load(Ordering::Acquire),
            1
        );
        assert!(!view.zmodem.safety.active.load(Ordering::Acquire));
        assert!(!publication.load(Ordering::Acquire));
        assert!(!view.zmodem_close_busy());
    });
    assert!(
        h.controls().is_empty(),
        "both confirmations acknowledge UI risk only; they cannot release backend quarantine"
    );
}

#[gpui_kit::test]
fn zmodem_unknown_result_survives_later_success_and_rejects_unread_acknowledgement(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let first = Uuid::new_v4();
    h.review(first, Uuid::new_v4(), 1, cx);
    h.terminal.update(cx, |view, _| {
        view.zmodem
            .safety
            .uncertain_epoch
            .store(1, Ordering::Release);
    });
    h.event(
        ZmodemUiEvent::Failed {
            id: first,
            reason: ZmodemFailure::UnverifiedPublication,
            uncertain: true,
        },
        cx,
    );
    let next = Uuid::new_v4();
    h.event(ZmodemUiEvent::UploadRequested { id: next }, cx);
    h.event(ZmodemUiEvent::Finished { id: next }, cx);
    h.terminal.update(cx, |view, _| {
        assert!(view.zmodem.unresolved.contains_key(&first));
        view.zmodem
            .safety
            .uncertain_epoch
            .store(2, Ordering::Release);
    });
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.read_with(cx, |view, _| {
        assert!(view.zmodem.acknowledge_review.is_none());
        assert!(view.zmodem_close_busy());
    });
    h.event(
        ZmodemUiEvent::Cancelled {
            id: Uuid::new_v4(),
            uncertain: true,
        },
        cx,
    );
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal
        .read_with(cx, |view, _| assert!(view.zmodem_close_busy()));
    h.click("zmodem-acknowledge-unknown", cx);
    h.terminal.read_with(cx, |view, _| {
        assert!(view.zmodem.unresolved.is_empty());
        assert!(!view.zmodem_close_busy());
    });
    assert!(
        h.controls().is_empty(),
        "risk acknowledgement has no filesystem or protocol action"
    );
}

#[gpui_kit::test]
fn zmodem_complete_reviews_keep_actions_visible_across_registered_themes(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let saved_id = Uuid::new_v4();
    let last_path = std::env::temp_dir()
        .join("verified-路径-with-complete-receipt".repeat(8))
        .join("saved-31.bin");
    h.bridge_events(
        (0..32).map(|index| ZmodemUiEvent::FileSaved {
            id: saved_id,
            path: last_path.with_file_name(format!("saved-{index}.bin")),
            bytes: 1024 + index,
            sha256: "fe".repeat(32),
        }),
        cx,
    );
    h.review(Uuid::new_v4(), Uuid::new_v4(), 32, cx);
    for family in crate::design::registry() {
        for theme in [Theme::Light, Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                cx.update_window(h.window, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
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
                    for id in [
                        "zmodem-transfer",
                        "zmodem-review-body",
                        "zmodem-review-actions",
                        "zmodem-confirm",
                        "zmodem-cancel",
                    ] {
                        assert!(window.find(id).visible(), "{id} must remain visible");
                    }
                    let body = window.find("zmodem-review-body").bounds();
                    let footer = window.find("zmodem-review-actions").bounds();
                    assert!(footer.top() >= body.bottom());
                    assert!(footer.bottom() <= px(580.));
                    window.scroll(
                        "zmodem-review-body",
                        gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                        cx,
                    );
                    window.render_frame(cx);
                    let last = window.find(("zmodem-verified-save", 31_usize));
                    assert!(
                        last.visible(),
                        "the final complete receipt must be reachable"
                    );
                    let text = last
                        .label()
                        .checked("original saved receipt after scrolling");
                    assert!(text.contains(last_path.to_string_lossy().as_ref()));
                    assert!(text.contains("\n1055 "));
                    assert!(text.contains(&"fe".repeat(32)));
                    assert_eq!(window.find("zmodem-review-actions").bounds(), footer);
                    for id in ["zmodem-confirm", "zmodem-cancel"] {
                        let action = window.find(id);
                        assert!(action.visible(), "{id} remains reachable after scrolling");
                        assert!(action.bounds().top() >= footer.top());
                        assert!(action.bounds().bottom() <= footer.bottom());
                    }
                    window.scroll(
                        "zmodem-review-body",
                        gpui_kit::ScrollDelta::Lines(point(0., 1000.)),
                        cx,
                    );
                    window.render_frame(cx);
                    assert_eq!(window.find("zmodem-review-actions").bounds(), footer);
                })
                .checked("render a complete long review with fixed actions");
            }
        }
    }
    assert!(h.controls().is_empty());
}
