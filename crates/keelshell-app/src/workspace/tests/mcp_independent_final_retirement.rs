//! Independent, privately scoped lifecycle counterexamples at additional boundaries.
use super::*;
use crate::terminal::TransportState;

fn finish_owned_workers(h: &Harness, cx: &gpui_kit::App) {
    h.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !h.fixture.workspace.read(cx).mcp_test_workers_finished() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .checked("raw loss resolves the owned future with foreground paused");
    });
}

#[gpui_kit::test]
async fn independent_final_mcp_initial_grant_validation_rejects_raw_loss(cx: &mut TestAppContext) {
    for ended in [true, false] {
        let h = Harness::new(cx);
        let (producer, source) = tokio::sync::watch::channel(TransportState::Ready);
        h.panes[1]
            .terminal
            .update(cx, |view, _| view.attach_lifecycle(source));
        let held = h
            .files
            .filesystem
            .hold_canonical_path("/approved")
            .checked("hold real first grant canonical response");
        cx.update_window(h.fixture.window, |_, window, cx| {
            h.fixture.workspace.update(cx, |view, cx| {
                view.active = 1;
                view.open_mcp(window, cx);
                view.mcp
                    .root
                    .update(cx, |input, cx| input.set_value("/approved", window, cx));
            });
            window.render_frame(cx);
            window.click("keelshell_list_sessions", cx);
            window.click("keelshell_sftp_read", cx);
            window.click("mcp-grant-session", cx);
        })
        .checked("actual initial grant controls");
        cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
            held.entered() > 0
        })
        .await;
        assert!(!held.expired());
        cx.update_window(h.fixture.window, |_, _, cx| {
            assert!(h.fixture.workspace.read(cx).mcp.busy);
            assert!(!h.fixture.workspace.read(cx).mcp_test_enabled());
            if ended {
                producer.send_replace(TransportState::Ended(
                    keelshell_session::ShellEnd::ChannelClosed,
                ));
            } else {
                drop(producer);
            }
            // Root validation must be cancelled while its original response is
            // still held and the UI cannot consume the completion packet.
            finish_owned_workers(&h, cx);
            assert!(!h.fixture.workspace.read(cx).mcp_test_enabled());
            held.release();
        })
        .checked("raw loss during initial grant with foreground paused");
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read_with(cx, |view, _| {
            assert!(!view.mcp.busy);
            assert!(!view.mcp_test_enabled());
        });
        assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
        assert_eq!(
            h.files.filesystem.transfer_writes_started(),
            h.initial_writes
        );
    }
}

#[gpui_kit::test]
async fn independent_final_mcp_real_success_queued_before_raw_loss_never_publishes(
    cx: &mut TestAppContext,
) {
    for ended in [true, false] {
        let h = Harness::new(cx);
        let (producer, source) = tokio::sync::watch::channel(TransportState::Ready);
        h.panes[0]
            .terminal
            .update(cx, |view, _| view.attach_lifecycle(source));
        h.grant(
            0,
            &[
                ToolKind::ListSessions,
                ToolKind::ProposeCommand,
                ToolKind::GetActionStatus,
            ],
            "",
            cx,
        )
        .await;
        let old = h.target(cx).await;
        let id = h.propose(old, "own successful response", cx).await;
        cx.update_window(h.fixture.window, |_, _, cx| {
            h.fixture
                .workspace
                .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
            finish_owned_workers(&h, cx);
            // Consume and immediately return the actual worker packet. This
            // establishes successful backend completion before raw retirement.
            h.fixture.workspace.update(cx, |view, _| {
                view.mcp_test_assert_actual_success_packet_and_return(id);
            });
            if ended {
                producer.send_replace(TransportState::Ended(
                    keelshell_session::ShellEnd::ChannelClosed,
                ));
            } else {
                drop(producer);
            }
            assert!(!h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
            assert!(matches!(
                h.fixture.workspace.read(cx).mcp_test_state(id),
                Some(ActionState::Running)
            ));
        })
        .checked("raw retirement before success completion admission");
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        assert!(matches!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_state(id)),
            Some(ActionState::OutcomeUnknown)
        ));
        assert_eq!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_output(id).map(str::to_owned)),
            Some(String::new())
        );
        assert_eq!(
            h.command.requests(),
            vec![b"own successful response".to_vec()]
        );
        let fresh = grant_files(&h, true, cx).await;
        assert_ne!(fresh, old);
        assert!(matches!(
            h.call(
                "keelshell_get_action_status",
                json!({"target":old,"action_id":id}),
                cx
            )
            .await,
            Err(McpFailure::Forbidden | McpFailure::StaleSession)
        ));
        let fresh_id = propose_file(
            &h,
            fresh,
            "/approved/中文.txt",
            "受控中文\n",
            "fresh draft only",
            cx,
        )
        .await;
        assert_ne!(fresh_id, id);
        assert!(matches!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_state(fresh_id)),
            Some(ActionState::PendingReview)
        ));
        assert_eq!(
            h.files.filesystem.transfer_writes_started(),
            h.initial_writes
        );
    }
}

#[gpui_kit::test]
async fn independent_final_mcp_raw_loss_during_atomic_staging_never_publishes(
    cx: &mut TestAppContext,
) {
    for ended in [true, false] {
        let h = Harness::new(cx);
        let (producer, source) = tokio::sync::watch::channel(TransportState::Ready);
        h.panes[1]
            .terminal
            .update(cx, |view, _| view.attach_lifecycle(source));
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
        let inspector = h.files.connect(&h.runtime);
        let held = h
            .files
            .filesystem
            .hold_atomic_writes_after_first()
            .checked("hold real temporary second WRITE");
        h.fixture
            .workspace
            .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
        cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
            held.entered() > 0
        })
        .await;
        assert!(!held.expired());
        // This counter increments before the hold: an already submitted remote
        // WRITE may finish after cancellation, but no new request may be issued.
        let staged_requests = h.files.filesystem.transfer_writes_started();
        assert!(
            staged_requests > h.initial_writes,
            "the real atomic temporary has begun staging"
        );
        let actual = cx
            .update_window(h.fixture.window, |_, _, cx| {
                if ended {
                    producer.send_replace(TransportState::Ended(
                        keelshell_session::ShellEnd::ChannelClosed,
                    ));
                } else {
                    drop(producer);
                }
                assert!(!h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
                held.release();
                finish_owned_workers(&h, cx);
                h.runtime.block_on(async {
                    let sftp = inspector
                        .sftp()
                        .await
                        .checked("separate authenticated midwrite inspector");
                    let actual = sftp
                        .read_regular("/approved/中文.txt", 65536)
                        .await
                        .checked("complete destination after raw midwrite cancellation");
                    sftp.close()
                        .await
                        .checked("close independent midwrite inspector");
                    actual
                })
            })
            .checked("lose original producer during atomic staging with foreground paused");
        assert_eq!(actual, "受控中文\n".as_bytes());
        assert_eq!(
            h.files.filesystem.transfer_writes_started(),
            staged_requests
        );
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        assert!(matches!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_state(id)),
            Some(ActionState::OutcomeUnknown)
        ));
        assert_eq!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_output(id).map(str::to_owned)),
            Some(String::new())
        );
        let original = h
            .fixture
            .workspace
            .read_with(cx, |view, _| {
                view.remote_sessions
                    .get(&h.panes[1].terminal.entity_id())
                    .cloned()
            })
            .checked_option("original connection remains independently quarantined");
        h.runtime.block_on(async {
            let sftp = original
                .sftp()
                .await
                .checked("inspect unknown mutation isolation");
            assert!(matches!(
                sftp.write_atomic("/approved/中文.txt", b"must not pass unknown ownership")
                    .await,
                Err(keelshell_session::SessionError::MutationQuarantined)
            ));
            sftp.close()
                .await
                .checked("close unknown isolation observation");
        });
        assert_eq!(
            h.files.filesystem.transfer_writes_started(),
            staged_requests
        );
    }
}
