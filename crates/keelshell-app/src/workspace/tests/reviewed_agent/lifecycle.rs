//! Captured-target retirement uses production close, observe and reconnect paths.
use super::*;
use crate::i18n::Message;
use crate::workspace::CloseTab;
use keelshell_ai::AgentOutcome;
use keelshell_session::SshSession;

#[gpui_kit::test]
async fn independent_agent_closed_tab_revokes_write_before_background_resume(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = crate::files::test_server::Server::new(&runtime);
    let session = peer.connect(&runtime);
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("seed independent SFTP");
        sftp.write("/agent-close.txt", b"original before closed target\n")
            .await
            .checked("seed old content");
        sftp.close().await.checked("seed close");
    });
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"write_file","path":"/agent-close.txt","replacement":"changed after target tab closed\n"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        p.read(cx).agent_file_review_ready_for_test()
    })
    .await;
    let held = peer
        .filesystem
        .hold_canonical_path("/agent-close.txt")
        .checked("own exact pre-mutation hold");
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, _| held.entered() > 0)
        .await;
    assert!(
        !held.expired(),
        "a held request must be explicitly released"
    );
    let readback = cx
        .update_window(f.window, |_, window, cx| {
            f.workspace
                .update(cx, |w, cx| w.close_tab(&CloseTab, window, cx));
            assert!(
                !f.workspace
                    .read(cx)
                    .tabs
                    .iter()
                    .any(|t| t.entity_id() == panes[0].terminal.entity_id())
            );
            let owner_retained = f.workspace.read(cx).agent.is_some();
            eprintln!("independent-close-readback owner_retained={owner_retained}");
            held.release();
            // Hold foreground maintenance while the existing background worker resumes.
            // This models delayed foreground scheduling, not production I/O on the UI.
            runtime.block_on(async {
                let sftp = session.sftp().await.checked("independent readback channel");
                let deadline = Instant::now() + Duration::from_secs(2);
                let result = loop {
                    let bytes = sftp
                        .read("/agent-close.txt", 32768)
                        .await
                        .checked("complete independent file bytes");
                    if bytes != b"original before closed target\n" || Instant::now() >= deadline {
                        break bytes;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                };
                sftp.close().await.checked("independent readback close");
                result
            })
        })
        .checked("remove captured tab before held background resumes");
    eprintln!(
        "independent-close-readback bytes={:?}",
        String::from_utf8_lossy(&readback)
    );
    assert_eq!(
        readback, b"original before closed target\n",
        "closing the captured tab must revoke admission before a paused writer resumes"
    );
}

fn seed(runtime: &tokio::runtime::Runtime, session: &SshSession, path: &str, content: &[u8]) {
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("owned lifecycle SFTP seed");
        sftp.write(path, content)
            .await
            .checked("original lifecycle file");
        sftp.close().await.checked("close lifecycle seed");
    });
}

fn read_while_foreground_held(
    runtime: &tokio::runtime::Runtime,
    session: &SshSession,
    path: &str,
    original: &[u8],
) -> Vec<u8> {
    // As in the independent counterexample, delayed foreground scheduling must
    // not give the paused backend any authority. This is test-only readback.
    runtime.block_on(async {
        let sftp = session
            .sftp()
            .await
            .checked("independent lifecycle readback");
        let deadline = Instant::now() + Duration::from_secs(2);
        let bytes = loop {
            let bytes = sftp.read(path, 32768).await.checked("full lifecycle file");
            if bytes != original || Instant::now() >= deadline {
                break bytes;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        sftp.close().await.checked("close lifecycle readback");
        bytes
    })
}

#[gpui_kit::test]
async fn reviewed_agent_terminal_observer_revokes_unbound_end_before_background_resume(
    cx: &mut TestAppContext,
) {
    for typed in [false, true] {
        let f = mount_sized(cx, Vec::new(), 1440., 900.);
        let panes = attach_remote_panes(&f, cx);
        let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
        let peer = crate::files::test_server::Server::new(&runtime);
        let session = peer.connect(&runtime);
        let path = "/agent-natural-end.txt";
        let original = b"original before natural end\n";
        seed(&runtime, &session, path, original);
        let (signal, source) = tokio::sync::watch::channel(crate::terminal::TransportState::Ready);
        if typed {
            panes[0]
                .terminal
                .update(cx, |terminal, _| terminal.attach_lifecycle(source));
        }
        f.workspace.update(cx, |w, cx| {
            w.remote_sessions
                .insert(panes[0].terminal.entity_id(), session.clone());
            w.active = 0;
            // The production terminal observer is the only foreground authority
            // maintenance in this case. No saved reconnect binding exists.
            w._reconnect_poll = gpui_kit::Task::ready(());
            assert!(
                !w.reconnect_bindings
                    .contains_key(&panes[0].terminal.entity_id())
            );
            cx.notify();
        });
        let model = Model::new(vec![
            json!({"kind":"write_file","path":path,"replacement":"must not publish after natural end\n"}),
        ]);
        let p = panel(&f, &model, cx);
        request(&f, &p, cx).await;
        click(&f, "agent-approve-action", cx);
        cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
            p.read(cx).agent_file_review_ready_for_test()
        })
        .await;
        let held = peer
            .filesystem
            .hold_canonical_path(path)
            .checked("natural-end pre-mutation hold");
        click(&f, "agent-approve-action", cx);
        cx.wait_for(f.window, Duration::from_secs(8), |_, _| held.entered() > 0)
            .await;
        assert!(!held.expired());
        let cancellation = f
            .workspace
            .read_with(cx, |w, _| w.agent_cancellation_for_test())
            .checked("captured backend cancellation");
        let (run_id, action_id) = f
            .workspace
            .read_with(cx, |w, _| w.agent_pending_for_test())
            .checked("captured running action");
        if typed {
            signal.send_replace(crate::terminal::TransportState::Ended(
                keelshell_session::ShellEnd::Exited { code: 0 },
            ));
        } else {
            panes[0]
                ._output
                .send(SessionEvent::Exited {
                    code: 0,
                    success: true,
                })
                .checked("deliver real terminal output lifecycle event");
        }
        cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
            let terminal = panes[0].terminal.read(cx);
            // A typed watch value changes is_open before the terminal poll has
            // observed it. Wait for the real poll's published end notification.
            !terminal.is_open() && terminal.status == Message::new("会话已退出（0）", "Exited (0)")
        })
        .await;
        assert!(
            cancellation.is_cancelled(),
            "terminal observer must synchronously revoke even without a reconnect binding; typed={typed}"
        );
        assert!(f.workspace.read_with(cx, |w, _| w.agent.is_none()));
        assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
        assert_eq!(
            p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
            Some(AgentOutcome::Unknown)
        );
        held.release();
        let bytes = read_while_foreground_held(&runtime, &session, path, original);
        assert_eq!(
            bytes, original,
            "natural terminal end cannot authorize a paused write; typed={typed}"
        );
        // A late success cannot replace the unknown receipt or resume a round.
        p.update(cx, |p, cx| {
            p.agent_action_finished(
                run_id,
                action_id,
                AgentOutcome::Completed {
                    exit_status: None,
                    output: "obsolete success".into(),
                },
                cx,
            )
        });
        assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
        assert_eq!(
            p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
            Some(AgentOutcome::Unknown)
        );
        assert_eq!(model.count(), 1);
    }
}

#[gpui_kit::test]
async fn reviewed_agent_reconnect_install_retires_old_write_before_background_resume(
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("owned lifecycle route", "example.invalid", "fixture");
    let profile_id = profile.id;
    let f = mount_sized(cx, vec![profile], 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let old = panes[0].terminal.entity_id();
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = crate::files::test_server::Server::new(&runtime);
    let session = peer.connect(&runtime);
    let replacement = peer.connect(&runtime);
    assert!(!session.same_connection(&replacement));
    let path = "/agent-reconnect.txt";
    let original = b"original before reconnect installation\n";
    seed(&runtime, &session, path, original);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions.insert(old, session.clone());
        w.active = 0;
        w._reconnect_poll = gpui_kit::Task::ready(());
        let route = w
            .state
            .connection_route(profile_id)
            .checked("captured fixture route");
        w.bind_remote_tab(old, route);
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"write_file","path":path,"replacement":"must not publish after reconnect\n"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        p.read(cx).agent_file_review_ready_for_test()
    })
    .await;
    let held = peer
        .filesystem
        .hold_canonical_path(path)
        .checked("reconnect pre-mutation hold");
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, _| held.entered() > 0)
        .await;
    assert!(!held.expired());
    let cancellation = f
        .workspace
        .read_with(cx, |w, _| w.agent_cancellation_for_test())
        .checked("old backend cancellation");
    let (run_id, action_id) = f
        .workspace
        .read_with(cx, |w, _| w.agent_pending_for_test())
        .checked("old run action identity");
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |w, cx| {
            let route = w
                .state
                .connection_route(profile_id)
                .checked("same saved fixture route");
            // Arrange only the completed attempt identity; install the actual
            // owned SSH handle through production finish_reconnect.
            let ticket = crate::workspace::reconnect::budget_tests::owned_fixture_ticket(w, old);
            assert!(w.finish_reconnect(ticket, replacement.clone(), route, window, cx));
            assert!(!w.remote_sessions.contains_key(&old));
            assert_ne!(w.tabs[0].entity_id(), old);
            assert!(w.remote_sessions[&w.tabs[0].entity_id()].same_connection(&replacement));
            assert!(w.agent.is_none());
            assert!(
                cancellation.is_cancelled(),
                "replacement must revoke before this foreground turn yields"
            );
        });
        held.release();
        assert_eq!(
            read_while_foreground_held(&runtime, &session, path, original),
            original
        );
    })
    .checked("production same-slot reconnect installation retires exact target");
    assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
    assert_eq!(
        p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
        Some(AgentOutcome::Unknown)
    );
    p.update(cx, |p, cx| {
        p.agent_action_finished(
            run_id,
            action_id,
            AgentOutcome::Completed {
                exit_status: None,
                output: "obsolete reconnect success".into(),
            },
            cx,
        )
    });
    assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
    assert_eq!(
        p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
        Some(AgentOutcome::Unknown)
    );
    assert_eq!(
        model.count(),
        1,
        "replacement cannot inherit old context or approve a new request"
    );
}

#[gpui_kit::test]
async fn reviewed_agent_raw_typed_end_blocks_paused_write_without_foreground_poll(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = crate::files::test_server::Server::new(&runtime);
    let session = peer.connect(&runtime);
    let path = "/agent-raw-typed-end.txt";
    let original = b"original before raw typed end\n";
    seed(&runtime, &session, path, original);
    let (signal, source) = tokio::sync::watch::channel(crate::terminal::TransportState::Ready);
    panes[0]
        .terminal
        .update(cx, |terminal, _| terminal.attach_lifecycle(source));
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), session.clone());
        w.active = 0;
        w._reconnect_poll = gpui_kit::Task::ready(());
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"write_file","path":path,"replacement":"must not publish while foreground end polling is delayed\n"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        p.read(cx).agent_file_review_ready_for_test()
    })
    .await;
    let held = peer
        .filesystem
        .hold_canonical_path(path)
        .checked("raw-end pre-mutation canonical hold");
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, _| held.entered() > 0)
        .await;
    assert!(!held.expired());
    let cancellation = f
        .workspace
        .read_with(cx, |w, _| w.agent_cancellation_for_test())
        .checked("raw-end owned cancellation");
    let (run_id, action_id) = f
        .workspace
        .read_with(cx, |w, _| w.agent_pending_for_test())
        .checked("raw-end exact running proposal");
    cx.update_window(f.window, |_, _, cx| {
        assert!(f.workspace.read(cx).agent_backend_current_for_test());
        signal.send_replace(crate::terminal::TransportState::Ended(
            keelshell_session::ShellEnd::Exited { code: 0 },
        ));
        // Keep this foreground turn occupied: neither the 16ms terminal poll,
        // its observer, nor the 200ms workspace maintenance can process End.
        assert!(!panes[0].terminal.read(cx).is_open());
        assert_ne!(
            panes[0].terminal.read(cx).status,
            Message::new("会话已退出（0）", "Exited (0)")
        );
        assert!(
            f.workspace.read(cx).agent.is_some(),
            "foreground owner has not yet been retired"
        );
        assert!(
            !f.workspace.read(cx).agent_backend_current_for_test(),
            "the captured raw lifecycle must already deny backend authorization"
        );
        held.release();
        assert_eq!(
            read_while_foreground_held(&runtime, &session, path, original),
            original,
            "raw closed lifecycle cannot publish during delayed foreground scheduling"
        );
        assert!(
            cancellation.is_cancelled(),
            "the backend lifecycle wait must observe loss without foreground maintenance"
        );
    })
    .checked("raw typed end and independent full SFTP readback before foreground polling");
    // Backend denial and full unchanged bytes were proved with the foreground
    // held. Only the UI receipt now waits for its real queued lifecycle callback.
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        f.workspace.read(cx).agent.is_none()
            && p.read(cx)
                .agent_snapshot_for_test()
                .is_some_and(|snapshot| snapshot.1 == AgentPhase::TargetLost)
    })
    .await;
    assert!(f.workspace.read_with(cx, |w, _| w.agent.is_none()));
    assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
    assert_eq!(
        p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
        Some(AgentOutcome::Unknown)
    );
    p.update(cx, |p, cx| {
        p.agent_action_finished(
            run_id,
            action_id,
            AgentOutcome::Completed {
                exit_status: None,
                output: "obsolete raw-end success".into(),
            },
            cx,
        )
    });
    assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
    assert_eq!(
        p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
        Some(AgentOutcome::Unknown)
    );
    assert_eq!(model.count(), 1);
}
