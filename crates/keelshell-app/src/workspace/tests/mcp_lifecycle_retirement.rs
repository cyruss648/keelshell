//! Raw lifecycle authority checks while the foreground cannot maintain MCP.
use super::*;
use crate::terminal::TransportState;
use gpui_kit::App;

fn observe_workers_finish_without_foreground(h: &Harness, cx: &mut App) {
    h.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !h.fixture.workspace.read(cx).mcp_test_workers_finished() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .checked("raw lifecycle loss ends owned workers without foreground callbacks");
    });
}

#[gpui_kit::test]
async fn mcp_retirement_closed_producer_blocks_paused_write_before_ui_poll(
    cx: &mut TestAppContext,
) {
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
        "producer was closed before write\n",
        cx,
    )
    .await;
    let inspector = h.files.connect(&h.runtime);
    let held = h
        .files
        .filesystem
        .hold_canonical_path("/approved/中文.txt")
        .checked("own producer-close canonical hold");
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        held.entered() > 0
    })
    .await;
    assert!(!held.expired());
    let actual = cx
        .update_window(h.fixture.window, |_, _, cx| {
            assert!(h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
            drop(producer);
            assert!(
                !h.fixture.workspace.read(cx).mcp_test_action_authorized(id),
                "raw producer loss rejects the captured authorization before UI polling"
            );
            held.release();
            h.runtime.block_on(async {
                let sftp = inspector.sftp().await.checked("independent producer read");
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                let actual = loop {
                    let bytes = sftp
                        .read_regular("/approved/中文.txt", 65536)
                        .await
                        .checked("full producer-close bytes");
                    if bytes != "受控中文\n".as_bytes() || std::time::Instant::now() >= deadline
                    {
                        break bytes;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                };
                sftp.close().await.checked("close producer inspector");
                actual
            })
        })
        .checked("lose raw producer with foreground blocked");
    assert_eq!(actual, "受控中文\n".as_bytes());
    h.fixture
        .workspace
        .update(cx, |view, cx| view.maintain_mcp(cx));
    assert!(matches!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_state(id)),
        Some(ActionState::OutcomeUnknown)
    ));
    assert!(
        !h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_enabled())
    );
}

#[gpui_kit::test]
async fn mcp_retirement_raw_end_cancels_held_read_without_returning_content(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    let (producer, source) = tokio::sync::watch::channel(TransportState::Ready);
    h.panes[1]
        .terminal
        .update(cx, |view, _| view.attach_lifecycle(source));
    let target = grant_files(&h, true, cx).await;
    let held = h
        .files
        .filesystem
        .hold_canonical_path("/approved/中文.txt")
        .checked("own exact in-flight read hold");
    let server = h.server(cx);
    let job = h.runtime.spawn(async move {
        server
            .invoke(
                "keelshell_sftp_read",
                json!({"target":target,"path":"/approved/中文.txt","max_bytes":65536}),
                CancellationToken::new(),
            )
            .await
    });
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        held.entered() > 0
    })
    .await;
    assert!(!held.expired());
    let actual = cx
        .update_window(h.fixture.window, |_, _, cx| {
            producer.send_replace(TransportState::Ended(
                keelshell_session::ShellEnd::ChannelClosed,
            ));
            assert!(!h.panes[1].terminal.read(cx).is_open());
            held.release();
            h.runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(2), job)
                    .await
                    .checked("raw loss promptly resolves owned read")
                    .checked("join raw-loss MCP read")
            })
        })
        .checked("end actual raw source without foreground maintenance");
    assert!(matches!(
        actual,
        Err(McpFailure::StaleSession | McpFailure::Revoked)
    ));
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
}

#[gpui_kit::test]
async fn mcp_retirement_raw_end_or_producer_close_rejects_pending_command_dispatch(
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
        let target = h.target(cx).await;
        let id = h
            .propose(target, "never issue after lifetime loss", cx)
            .await;
        cx.update_window(h.fixture.window, |_, _, cx| {
            assert!(h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
            if ended {
                producer.send_replace(TransportState::Ended(
                    keelshell_session::ShellEnd::ChannelClosed,
                ));
            } else {
                drop(producer);
            }
            assert!(!h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
            h.fixture
                .workspace
                .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
            assert!(matches!(
                h.fixture.workspace.read(cx).mcp_test_state(id),
                Some(ActionState::Cancelled)
            ));
            assert!(h.command.requests().is_empty());
        })
        .checked("pending command checks raw lifetime before dispatch");
        assert_eq!(
            h.call(
                "keelshell_get_action_status",
                json!({"target":target,"action_id":id}),
                cx,
            )
            .await,
            Err(McpFailure::Disabled)
        );
        assert!(h.command.requests().is_empty());
    }
}

#[gpui_kit::test]
async fn mcp_retirement_raw_end_cancels_running_command_without_publishing_output(
    cx: &mut TestAppContext,
) {
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
    let target = h.target(cx).await;
    // The isolated peer records the command but never executes its text. It
    // deliberately leaves this exec channel pending until local cancellation.
    let id = h.propose(target, "hold", cx).await;
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        h.command.request_count() == 1
    })
    .await;
    cx.update_window(h.fixture.window, |_, _, cx| {
        producer.send_replace(TransportState::Ended(
            keelshell_session::ShellEnd::ChannelClosed,
        ));
        assert!(!h.fixture.workspace.read(cx).mcp_test_action_authorized(id));
        observe_workers_finish_without_foreground(&h, cx);
        assert!(
            matches!(
                h.fixture.workspace.read(cx).mcp_test_state(id),
                Some(ActionState::Running)
            ),
            "the background outcome must remain unpublished while the foreground is paused"
        );
        assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
    })
    .checked("cancel actual owned exec future from the raw source");
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
    assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
}

#[gpui_kit::test]
async fn mcp_retirement_raw_end_cancels_file_preparation_before_admission(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let (producer, source) = tokio::sync::watch::channel(TransportState::Ready);
    h.panes[1]
        .terminal
        .update(cx, |view, _| view.attach_lifecycle(source));
    let target = grant_files(&h, true, cx).await;
    let held = h
        .files
        .filesystem
        .hold_canonical_path("/approved/中文.txt")
        .checked("own preparing-file canonical hold");
    let server = h.server(cx);
    let job = h.runtime.spawn(async move {
        server.invoke("keelshell_propose_file_change", json!({"target":target,"path":"/approved/中文.txt","expected_sha256":content_sha256("受控中文\n"),"replacement":"never admit after raw end"}), CancellationToken::new()).await
    });
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        held.entered() > 0
    })
    .await;
    assert!(!held.expired());
    cx.update_window(h.fixture.window, |_, _, cx| {
        producer.send_replace(TransportState::Ended(
            keelshell_session::ShellEnd::ChannelClosed,
        ));
        held.release();
        observe_workers_finish_without_foreground(&h, cx);
        assert_eq!(
            h.files.filesystem.transfer_writes_started(),
            h.initial_writes
        );
    })
    .checked("cancel actual preparation before foreground can admit it");
    h.fixture
        .workspace
        .update(cx, |view, cx| view.maintain_mcp(cx));
    let result = h.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), job)
            .await
            .checked("bounded rejected preparation reply")
            .checked("join rejected preparation")
    });
    assert!(matches!(
        result,
        Err(McpFailure::Disabled | McpFailure::Revoked | McpFailure::NotConnected)
    ));
    assert_eq!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_preparing_count()),
        0
    );
    assert_eq!(bytes(&h, "/approved/中文.txt", cx), "受控中文\n".as_bytes());
}
