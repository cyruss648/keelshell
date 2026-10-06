//! Explicit desktop file review over real isolated SSH/SFTP and authenticated IPC.
use super::*;
use keelshell_mcp::content_sha256;
#[path = "mcp_transfer_isolation.rs"]
mod transfer_isolation;

async fn grant_files(h: &Harness, read: bool, cx: &mut TestAppContext) -> SessionIdentity {
    let mut tools = vec![
        ToolKind::ListSessions,
        ToolKind::ProposeFileChange,
        ToolKind::GetActionStatus,
    ];
    if read {
        tools.push(ToolKind::SftpRead);
    }
    h.grant(1, &tools, "/approved", cx).await;
    h.target(cx).await
}
async fn propose_file(
    h: &Harness,
    target: SessionIdentity,
    path: &str,
    old: &str,
    replacement: &str,
    cx: &mut TestAppContext,
) -> uuid::Uuid {
    let reply = h.call("keelshell_propose_file_change", json!({"target":target,"path":path,"expected_sha256":content_sha256(old),"replacement":replacement}), cx).await.checked("enqueue exact existing-file replacement");
    assert_eq!(reply["kind"], "pending_file_change");
    assert!(
        reply.get("text").is_none() && reply.get("sha256").is_none() && reply.get("path").is_none()
    );
    serde_json::from_value(reply["action_id"].clone()).checked("file proposal identity")
}
fn bytes(h: &Harness, path: &str, cx: &mut TestAppContext) -> Vec<u8> {
    let session = h
        .fixture
        .workspace
        .read_with(cx, |view, _| {
            view.remote_sessions
                .get(&h.panes[1].terminal.entity_id())
                .cloned()
        })
        .checked_option("owned file session");
    h.runtime.block_on(async {
        let sftp = session.sftp().await.checked("inspect exact fixture bytes");
        let bytes = sftp
            .read_regular(path, 64 * 1024)
            .await
            .checked("read complete fixture");
        sftp.close().await.checked("close inspection");
        bytes
    })
}
fn open_review(h: &Harness, id: uuid::Uuid, cx: &mut TestAppContext) {
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.mcp.reviewing = Some(id);
            cx.notify();
        });
        window.render_frame(cx);
    })
    .checked("open immutable review without approving");
}
async fn wait_state(h: &Harness, id: uuid::Uuid, state: ActionState, cx: &mut TestAppContext) {
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture
            .workspace
            .read(cx)
            .mcp_test_state(id)
            .is_some_and(|actual| std::mem::discriminant(&actual) == std::mem::discriminant(&state))
    })
    .await;
}

fn queue_file_request(
    h: &Harness,
    target: SessionIdentity,
    cx: &mut TestAppContext,
) -> tokio::task::JoinHandle<Result<Value, McpFailure>> {
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_defer_file_prepared());
    let server = h.server(cx);
    h.runtime.spawn(async move {
        server
            .invoke(
                "keelshell_propose_file_change",
                json!({"target":target,"path":"/approved/中文.txt","expected_sha256":content_sha256("受控中文\n"),"replacement":"held proposal"}),
                CancellationToken::new(),
            )
            .await?
            .structured_content
            .ok_or(McpFailure::BackendFailure)
    })
}
async fn hold_preparation(
    h: &Harness,
    cx: &mut TestAppContext,
) -> crate::workspace::mcp::HeldFilePreparation {
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read(cx).mcp_test_preparing_count() == 1
    })
    .await;
    let mut held = None;
    // Test support holds the genuine SFTP completion before admission even if
    // an automatic UI tick runs, then delivers it after a new reservation exists.
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        held = h.fixture.workspace.update(cx, |view, cx| {
            view.maintain_mcp(cx);
            view.mcp_test_hold_file_prepared()
        });
        held.is_some()
    })
    .await;
    held.checked_option("hold exact owned file preparation")
}

#[gpui_kit::test]
async fn mcp_file_preparation_ownership_survives_regrant_revoke_and_closed_reply(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let original = grant_files(&h, false, cx).await;
    let old_job = queue_file_request(&h, original, cx);
    let old_frame = hold_preparation(&h, cx).await;
    let fresh = grant_files(&h, false, cx).await;
    assert_ne!(fresh, original);
    assert!(
        h.runtime
            .block_on(old_job)
            .checked("join revoked old request")
            .is_err()
    );
    let new_job = queue_file_request(&h, fresh, cx);
    let new_frame = hold_preparation(&h, cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_return_file_prepared(old_frame);
        view.maintain_mcp(cx);
        assert_eq!(
            view.mcp_test_preparing_count(),
            1,
            "old frame cannot release new grant's slot"
        );
        assert_eq!(
            view.mcp_toolbar_label(),
            "MCP",
            "old frame cannot enter review"
        );
        view.mcp_test_return_file_prepared(new_frame);
        view.maintain_mcp(cx);
        assert_eq!(view.mcp_test_preparing_count(), 0);
    });
    let reply = h
        .runtime
        .block_on(new_job)
        .checked("join current request")
        .checked("admit current preparation");
    let id = serde_json::from_value(reply["action_id"].clone()).checked("current action identity");
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::PendingReview)
    ));
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, false, cx));

    let cancelled_job = queue_file_request(&h, fresh, cx);
    let cancelled_frame = hold_preparation(&h, cx).await;
    cancelled_job.abort();
    assert!(h.runtime.block_on(cancelled_job).is_err());
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_return_file_prepared(cancelled_frame);
        view.maintain_mcp(cx);
        assert_eq!(
            view.mcp_test_preparing_count(),
            0,
            "closed IPC reply releases only its own slot"
        );
    });

    let revoked_job = queue_file_request(&h, fresh, cx);
    let revoked_frame = hold_preparation(&h, cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_disable();
        view.mcp_test_return_file_prepared(revoked_frame);
        view.maintain_mcp(cx);
        assert_eq!(view.mcp_test_preparing_count(), 0);
    });
    assert!(
        h.runtime
            .block_on(revoked_job)
            .checked("join disabled request")
            .is_err()
    );
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
}

#[gpui_kit::test]
async fn mcp_file_failed_regrant_releases_finished_preparation_and_keeps_old_scope(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, false, cx).await;
    for _ in 0..2 {
        let job = queue_file_request(&h, target, cx);
        let held = hold_preparation(&h, cx).await;
        let validation = h
            .files
            .filesystem
            .hold_canonical_path("/approved/missing")
            .checked("own exact production regrant REALPATH response");
        cx.update_window(h.fixture.window, |_, window, cx| {
            h.fixture.workspace.update(cx, |view, cx| {
                view.mcp.root.update(cx, |input, cx| {
                    input.set_value("/approved/missing", window, cx)
                });
                assert_eq!(view.mcp_test_preparing_count(), 1);
            });
            window.render_frame(cx);
            window.click("mcp-grant-session", cx);
        })
        .checked("start production regrant against missing canonical directory");
        // A click alone does not prove validation is still in flight. Hold the
        // real response and observe admission before asserting its busy state.
        cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
            validation.entered() == 1
        })
        .await;
        assert!(!validation.expired());
        h.fixture.workspace.read_with(cx, |view, _| {
            assert!(view.mcp.busy);
            assert_eq!(view.mcp_test_preparing_count(), 1);
        });
        validation.release();
        cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
            h.fixture
                .workspace
                .update(cx, |view, cx| view.maintain_mcp(cx));
            h.fixture.workspace.read(cx).mcp_test_enabled()
        })
        .await;
        h.fixture.workspace.update(cx, |view, cx| {
            view.mcp_test_return_file_prepared(held);
            view.maintain_mcp(cx);
            assert_eq!(
                view.mcp_test_preparing_count(),
                0,
                "failed regrant must release finished old revision's reservation"
            );
            assert_eq!(
                view.mcp_toolbar_label(),
                "MCP",
                "stale completion cannot enter review"
            );
        });
        assert!(
            h.runtime
                .block_on(job)
                .checked("join stale old revision request")
                .is_err()
        );
        assert_eq!(
            h.target(cx).await,
            target,
            "failed regrant does not replace old authority"
        );
    }
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "still admissible",
        cx,
    )
    .await;
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::PendingReview)
    ));
    assert_eq!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_preparing_count()),
        0
    );
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
}

#[gpui_kit::test]
async fn mcp_file_proposal_requires_separate_grant_and_exact_baseline_without_leaking_content(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.grant(
        1,
        &[
            ToolKind::ListSessions,
            ToolKind::SftpRead,
            ToolKind::ProposeCommand,
        ],
        "/approved",
        cx,
    )
    .await;
    let old_target = h.target(cx).await;
    let args = json!({"target":old_target,"path":"/approved/中文.txt","expected_sha256":content_sha256("受控中文\n"),"replacement":"new"});
    assert_eq!(
        h.call("keelshell_propose_file_change", args, cx).await,
        Err(McpFailure::Forbidden)
    );
    let target = grant_files(&h, false, cx).await;
    assert_eq!(
        h.call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/中文.txt","max_bytes":65536}),
            cx
        )
        .await,
        Err(McpFailure::Forbidden)
    );
    for (path, hash) in [
        ("/outside", content_sha256("not granted")),
        ("/approved/中文.txt", content_sha256("wrong baseline")),
        ("/approved/missing", content_sha256("")),
        ("/approved", content_sha256("")),
        ("/approved/link", content_sha256("")),
        ("/approved/binary", content_sha256("")),
    ] {
        let result = h
            .call(
                "keelshell_propose_file_change",
                json!({"target":target,"path":path,"expected_sha256":hash,"replacement":"new"}),
                cx,
            )
            .await;
        assert!(result.is_err(), "denial for {path}");
    }
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "替换中文\n",
        cx,
    )
    .await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
    let status = h
        .call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":id}),
            cx,
        )
        .await
        .checked("file status without read permission");
    assert_eq!(status["action_kind"], "file_change");
    assert_eq!(status["state"], "pending_review");
    assert!(status.get("text").is_none());
}

#[gpui_kit::test]
async fn mcp_file_human_approval_writes_exact_bytes_once_and_read_hash_matches(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let read = h
        .call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/中文.txt","max_bytes":65536}),
            cx,
        )
        .await
        .checked("review complete content");
    assert_eq!(read["sha256"], content_sha256("受控中文\n"));
    let replacement = "replacement\0中文\nno final newline";
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        replacement,
        cx,
    )
    .await;
    open_review(&h, id, cx);
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-file-review-approve", cx);
    })
    .checked("human confirms exact file review");
    wait_state(&h, id, ActionState::Succeeded, cx).await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), replacement.as_bytes());
    let write_count = h.files.filesystem.transfer_writes_started();
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    assert_eq!(h.files.filesystem.transfer_writes_started(), write_count);
    assert!(h.command.requests().is_empty() && h.panes.iter().all(|pane| writes(pane).is_empty()));
    assert!(
        h.fixture
            .store
            .load()
            .checked("ephemeral proposal metadata")
            .connections
            .is_empty()
    );
}

fn write_file(h: &Harness, path: &str, content: &[u8], cx: &mut TestAppContext) {
    let session = h
        .fixture
        .workspace
        .read_with(cx, |view, _| {
            view.remote_sessions
                .get(&h.panes[1].terminal.entity_id())
                .cloned()
        })
        .checked_option("captured fixture session");
    h.runtime.block_on(async {
        let sftp = session.sftp().await.checked("fixture external writer");
        sftp.write(path, content)
            .await
            .checked("fixture external write");
        sftp.close().await.checked("close external writer");
    });
}

#[gpui_kit::test]
async fn mcp_file_rejection_expiry_revoke_and_cancel_admission_produce_no_write(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, false, cx).await;
    let rejected = propose_file(&h, target, "/approved/中文.txt", "受控中文\n", "reject", cx).await;
    open_review(&h, rejected, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-file-review-reject", cx);
    })
    .checked("reject full file review");
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(rejected)),
        Some(ActionState::Rejected)
    ));
    let expired = propose_file(&h, target, "/approved/中文.txt", "受控中文\n", "expire", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_expire(expired);
        view.review_mcp_action(expired, true, cx);
    });
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(expired)),
        Some(ActionState::Expired)
    ));
    let token = CancellationToken::new();
    token.cancel();
    let denied = h.runtime.block_on(h.server(cx).invoke("keelshell_propose_file_change", json!({"target":target,"path":"/approved/中文.txt","expected_sha256":content_sha256("受控中文\n"),"replacement":"cancelled"}), token));
    assert!(matches!(denied, Err(McpFailure::Cancelled)));
    let revoked = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "revoked",
        cx,
    )
    .await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("mcp-disable", cx);
    })
    .checked("human revokes pending proposals");
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(revoked, true, cx));
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(revoked)),
        Some(ActionState::Cancelled)
    ));
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
}

#[gpui_kit::test]
async fn mcp_file_changes_after_review_and_missing_atomic_extension_do_not_overwrite(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let stale = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "replace",
        cx,
    )
    .await;
    write_file(&h, "/approved/中文.txt", "其它写入\n".as_bytes(), cx);
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(stale, true, cx));
    wait_state(&h, stale, ActionState::Failed, cx).await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "其它写入\n".as_bytes());
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
    let missing = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "其它写入\n",
        "replace",
        cx,
    )
    .await;
    h.files.filesystem.set_atomic_unsupported(true);
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(missing, true, cx));
    wait_state(&h, missing, ActionState::OutcomeUnknown, cx).await;
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "其它写入\n".as_bytes());
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
    write_file(&h, "/approved/too-large", &vec![b'x'; 65537], cx);
    assert_eq!(h.call("keelshell_propose_file_change", json!({"target":target,"path":"/approved/too-large","expected_sha256":content_sha256(&"x".repeat(65537)),"replacement":"replace"}), cx).await, Err(McpFailure::OutputLimit));
}

#[gpui_kit::test]
async fn mcp_file_running_revoke_reports_unknown_preserves_original_and_ignores_late_success(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        &"x".repeat(65536),
        cx,
    )
    .await;
    let gate = h
        .files
        .filesystem
        .hold_atomic_writes_after_first()
        .checked("own generated temporary WRITE hold");
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        gate.entered() > 0
    })
    .await;
    assert!(!gate.expired());
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_disable());
    gate.release();
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::OutcomeUnknown)
    ));
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
    assert_eq!(
        h.call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":id}),
            cx
        )
        .await,
        Err(McpFailure::Disabled)
    );
    // Even a success receipt queued just before policy replacement is not published.
    let fresh = grant_files(&h, true, cx).await;
    let late = propose_file(&h, fresh, "/approved/中文.txt", "受控中文\n", "late", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_late_success(late, cx);
        view.maintain_mcp(cx);
    });
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(late)),
        Some(ActionState::OutcomeUnknown)
    ));
    assert_eq!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_output(late).map(str::to_owned)),
        Some(String::new())
    );
}

#[gpui_kit::test]
async fn mcp_file_same_entity_with_replaced_ssh_handle_invalidates_old_grant(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let id = propose_file(
        &h,
        target,
        "/approved/中文.txt",
        "受控中文\n",
        "stale route",
        cx,
    )
    .await;
    let replacement = h.files.connect(&h.runtime);
    h.fixture.workspace.update(cx, |view, cx| {
        view.remote_sessions
            .insert(h.panes[1].terminal.entity_id(), replacement);
        view.maintain_mcp(cx);
        view.review_mcp_action(id, true, cx);
    });
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::Cancelled)
    ));
    assert_eq!(h.call("keelshell_propose_file_change", json!({"target":target,"path":"/approved/中文.txt","expected_sha256":content_sha256("受控中文\n"),"replacement":"stale"}), cx).await, Err(McpFailure::Disabled));
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
}

#[gpui_kit::test]
async fn mcp_file_review_compact_languages_themes_has_complete_diff_and_fixed_buttons(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let target = grant_files(&h, true, cx).await;
    let old = "old完整中文\n".repeat(2500);
    let new = format!("{}最后一行\u{202e}", "replacement\n".repeat(4000));
    assert!(old.len() <= 65536 && new.len() <= 65536);
    let mut path = String::from("/approved");
    let session = h
        .fixture
        .workspace
        .read_with(cx, |view, _| {
            view.remote_sessions
                .get(&h.panes[1].terminal.entity_id())
                .cloned()
        })
        .checked_option("long-path fixture session");
    h.runtime.block_on(async {
        let sftp = session.sftp().await.checked("nested fixture directories");
        for index in 0..8 {
            path.push_str(&format!("/part{index}-{}", "x".repeat(120)));
            sftp.mkdir(&path).await.checked("create literal parent");
        }
        sftp.close().await.checked("close nested directory seed");
    });
    path.push_str("/完整中文-file.txt");
    assert!(path.len() > 1024 && path.len() < 4096);
    write_file(&h, &path, old.as_bytes(), cx);
    let id = propose_file(&h, target, &path, &old, &new, cx).await;
    h.fixture.workspace.read_with(cx, |view, _| {
        let (actual_path, diff) = view
            .mcp_test_file_review(id)
            .checked_option("full immutable review payload");
        assert_eq!(actual_path, path);
        assert_eq!(diff.matches("- old完整中文\n").count(), 2500);
        assert_eq!(
            diff.lines().filter(|line| *line == "+ replacement").count(),
            4000
        );
        assert!(diff.ends_with("+ 最后一行\\u{202e} [no final newline]\n"));
    });
    open_review(&h, id, cx);
    cx.simulate_window_resize(h.fixture.window, size(px(900.), px(580.)));
    for theme in [
        keelshell_core::Theme::System,
        keelshell_core::Theme::Light,
        keelshell_core::Theme::Dark,
    ] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                h.fixture.workspace.update(cx, |view, cx| {
                    view.state.settings.theme = theme;
                    view.state.settings.language = language;
                    cx.notify();
                });
                window.render_frame(cx);
                assert_eq!(
                    window.find("mcp-file-review-path").label(),
                    Some(path.as_str())
                );
                let viewport = window.find("mcp-file-review-scroll").bounds();
                let footer = window.find("mcp-file-review-footer").bounds();
                for button in [
                    "mcp-file-review-approve",
                    "mcp-file-review-reject",
                    "mcp-file-review-close",
                ] {
                    let node = window.find(button);
                    let bounds = node.bounds();
                    assert!(
                        node.visible()
                            && bounds.origin.y >= footer.origin.y
                            && bounds.bottom() <= footer.bottom()
                    );
                    assert!(
                        bounds.right() <= window.bounds().right()
                            && bounds.bottom() <= window.bounds().bottom()
                    );
                }
                window.scroll(
                    "mcp-file-review-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-1000000.))),
                    cx,
                );
                window.render_frame(cx);
                assert_eq!(window.find("mcp-file-review-footer").bounds(), footer);
                assert!(viewport.size.height > px(100.));
                assert!(window.find("mcp-file-review-approve").visible());
                assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
                // The grants layer is unmounted, so even its AX buttons do not
                // coexist with a human file review or receive keyboard input.
                assert!(window.try_find("mcp-disable").is_none());
            })
            .checked("compact full review in production themes and languages");
        }
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-file-review-close", cx);
        window.render_frame(cx);
    })
    .checked("hide review without approving");
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::PendingReview)
    ));
    open_review(&h, id, cx);
    assert_eq!(bytes(&h, &path, cx), old.as_bytes());
}

#[gpui_kit::test]
async fn mcp_file_authenticated_stdio_returns_eight_schemas_and_exact_reviewed_state(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    grant_files(&h, true, cx).await;
    let client = StdioClient::new(&h, cx);
    let init = client.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"owned-file-change-fixture","version":"1.0"}}}), &h, cx).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    h.runtime.block_on(async {
        let mut stream = client.stream.lock().await;
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .checked("initialize fixture");
        stream.flush().await.checked("flush fixture");
    });
    let catalog = client
        .rpc(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            &h,
            cx,
        )
        .await;
    assert_eq!(catalog["result"]["tools"].as_array().map(Vec::len), Some(8));
    let list = client
        .call(3, "keelshell_list_sessions", json!({}), &h, cx)
        .await;
    let target = list["sessions"][0]["target"].clone();
    let read = client
        .call(
            4,
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/中文.txt","max_bytes":65536}),
            &h,
            cx,
        )
        .await;
    assert_eq!(read["sha256"], content_sha256("受控中文\n"));
    let proposal = client.call(5, "keelshell_propose_file_change", json!({"target":target,"path":"/approved/中文.txt","expected_sha256":read["sha256"],"replacement":"stdio replacement\n"}), &h, cx).await;
    let id: uuid::Uuid =
        serde_json::from_value(proposal["action_id"].clone()).checked("actual stdio action ID");
    assert_eq!(h.files.filesystem.atomic_writes_started(), 0);
    open_review(&h, id, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click("mcp-file-review-approve", cx);
    })
    .checked("human approval for actual IPC proposal");
    wait_state(&h, id, ActionState::Succeeded, cx).await;
    let status = client
        .call(
            6,
            "keelshell_get_action_status",
            json!({"target":target,"action_id":id}),
            &h,
            cx,
        )
        .await;
    assert_eq!(status["action_kind"], "file_change");
    assert_eq!(status["state"], "succeeded");
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), b"stdio replacement\n");
}
