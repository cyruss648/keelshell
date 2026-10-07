//! Immediate user invalidation must revoke the exact backend grant.
use super::*;
use crate::i18n::Message;
use keelshell_ai::AgentOutcome;

#[gpui_kit::test]
async fn independent_agent_stop_revokes_write_before_background_resume(cx: &mut TestAppContext) {
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
            window.render_frame(cx);
            window.click("assistant-stop-agent", cx);
            assert_eq!(
                p.read(cx)
                    .agent_snapshot_for_test()
                    .checked("stop domain receipt")
                    .1,
                AgentPhase::OutcomeUnknown
            );
            let owner_retained = f.workspace.read(cx).agent.is_some();
            eprintln!("independent-stop-readback owner_retained={owner_retained}");
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
        "independent-stop-readback bytes={:?}",
        String::from_utf8_lossy(&readback)
    );
    assert_eq!(
        readback, b"original before closed target\n",
        "stopping the Agent must revoke admission before a paused writer resumes"
    );
}

#[derive(Clone, Copy, Debug)]
enum Invalidation {
    Selection,
    Screen,
    ContextSetter,
    ProfileChoice,
    ProfileApply,
    CredentialApply,
    AskMode,
    EditQuestion,
}

fn full_read_while_foreground_held(
    runtime: &tokio::runtime::Runtime,
    session: &keelshell_session::SshSession,
    path: &str,
    original: &[u8],
) -> Vec<u8> {
    runtime.block_on(async {
        let sftp = session
            .sftp()
            .await
            .checked("independent invalidation SFTP readback");
        let deadline = Instant::now() + Duration::from_secs(2);
        let bytes = loop {
            let bytes = sftp
                .read(path, 32768)
                .await
                .checked("full invalidation bytes");
            if bytes != original || Instant::now() >= deadline {
                break bytes;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        sftp.close().await.checked("close invalidation readback");
        bytes
    })
}

#[gpui_kit::test]
async fn reviewed_agent_user_invalidation_revokes_paused_write_in_current_turn(
    cx: &mut TestAppContext,
) {
    for invalidation in [
        Invalidation::Selection,
        Invalidation::Screen,
        Invalidation::ContextSetter,
        Invalidation::ProfileChoice,
        Invalidation::ProfileApply,
        Invalidation::CredentialApply,
        Invalidation::AskMode,
        Invalidation::EditQuestion,
    ] {
        let f = mount_sized(cx, Vec::new(), 1440., 900.);
        let panes = attach_remote_panes(&f, cx);
        let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
        let peer = crate::files::test_server::Server::new(&runtime);
        let session = peer.connect(&runtime);
        let path = "/agent-invalidation.txt";
        let original = b"original before user invalidation\n";
        runtime.block_on(async {
            let sftp = session.sftp().await.checked("seed user invalidation");
            sftp.write(path, original)
                .await
                .checked("seed original invalidation bytes");
            sftp.close().await.checked("close invalidation seed");
        });
        f.workspace.update(cx, |w, cx| {
            w.remote_sessions
                .insert(panes[0].terminal.entity_id(), session.clone());
            w.active = 0;
            w._reconnect_poll = gpui_kit::Task::ready(());
            cx.notify();
        });
        let model = Model::new(vec![
            json!({"kind":"write_file", "path":path, "replacement":"must not publish after user invalidation\n"}),
        ]);
        let p = panel(&f, &model, cx);
        let current = p
            .read_with(cx, |p, _| p.agent_profile_for_test())
            .checked("current shared profile");
        let mut alternative = current.clone();
        alternative.id = uuid::Uuid::new_v4();
        alternative.name = "Alternative owned profile".into();
        let catalog = AiProfileCatalog {
            active_id: Some(current.id),
            profiles: vec![current.clone(), alternative],
        };
        p.update(cx, |p, cx| {
            p.set_profiles(
                &catalog,
                &crate::ai_settings::EphemeralCredentials::new(),
                cx,
            )
        });
        request(&f, &p, cx).await;
        click(&f, "agent-approve-action", cx);
        cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
            p.read(cx).agent_file_review_ready_for_test()
        })
        .await;
        let held = peer
            .filesystem
            .hold_canonical_path(path)
            .checked("exact invalidation pre-mutation canonical hold");
        click(&f, "agent-approve-action", cx);
        cx.wait_for(f.window, Duration::from_secs(8), |_, _| held.entered() > 0)
            .await;
        assert!(!held.expired());
        let cancellation = f
            .workspace
            .read_with(cx, |w, _| w.agent_cancellation_for_test())
            .checked("exact invalidation grant");
        let (run_id, action_id) = f
            .workspace
            .read_with(cx, |w, _| w.agent_pending_for_test())
            .checked("exact invalidation IDs");
        let prompt = p.read_with(cx, |p, _| p.agent_prompt_for_test());
        cx.update_window(f.window, |_, window, cx| {
            assert!(f.workspace.read(cx).agent_backend_current_for_test());
            window.scroll(
                "assistant-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., 1000.)),
                cx,
            );
            window.render_frame(cx);
            match invalidation {
                Invalidation::Selection => window.click("context-selection", cx),
                Invalidation::Screen => window.click("context-screen", cx),
                Invalidation::ContextSetter => p.update(cx, |p, cx| {
                    p.set_context(
                        "another explicit context".into(),
                        "another host".into(),
                        "another session".into(),
                        cx,
                    )
                }),
                Invalidation::ProfileChoice => {
                    window.click("assistant-profile", cx);
                    window.render_frame(cx);
                    window.click(("assistant-profile-choice", 1usize), cx);
                }
                Invalidation::ProfileApply => {
                    let mut edited = catalog.clone();
                    edited.profiles[0].model = "changed-owned-model".into();
                    p.update(cx, |p, cx| {
                        p.set_profiles(
                            &edited,
                            &crate::ai_settings::EphemeralCredentials::new(),
                            cx,
                        )
                    });
                }
                Invalidation::CredentialApply => {
                    let mut credentials = crate::ai_settings::EphemeralCredentials::new();
                    credentials.insert(
                        current.id,
                        zeroize::Zeroizing::new("owned-nonproduction-credential".into()),
                    );
                    p.update(cx, |p, cx| p.set_profiles(&catalog, &credentials, cx));
                }
                Invalidation::AskMode => window.click("assistant-mode-ask", cx),
                Invalidation::EditQuestion => {
                    let question = prompt.read(cx).value().to_string();
                    assert!(!prompt.read(cx).is_editable());
                    prompt.update(cx, |input, cx| input.focus(window, cx));
                    window.press("secondary-a", cx);
                    window.press("secondary-c", cx);
                    assert_eq!(
                        cx.read_from_clipboard()
                            .checked("copy read-only question")
                            .text()
                            .as_deref(),
                        Some(question.as_str())
                    );
                    window.input("must not alter an active question", cx);
                    window.press("backspace", cx);
                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                        "must not paste".into(),
                    ));
                    window.press("secondary-v", cx);
                    assert_eq!(prompt.read(cx).value().to_string(), question);
                    assert!(
                        !cancellation.is_cancelled(),
                        "rejected editing attempts do not authorize a change"
                    );
                    window.render_frame(cx);
                    window.click("assistant-edit-agent-question", cx);
                    assert!(
                        cancellation.is_cancelled(),
                        "revoke before enabling any editing"
                    );
                    assert!(prompt.read(cx).is_editable());
                    window.press("secondary-a", cx);
                    window.input("revised explicit question", cx);
                    assert_eq!(prompt.read(cx).value(), "revised explicit question");
                    window.press("secondary-a", cx);
                    window.press("backspace", cx);
                    assert!(prompt.read(cx).value().is_empty());
                }
            }
            assert!(
                cancellation.is_cancelled(),
                "{invalidation:?} must synchronously revoke"
            );
            assert!(!f.workspace.read(cx).agent_backend_current_for_test());
            assert!(
                f.workspace.read(cx).agent.is_some(),
                "queued ownership cleanup has not run"
            );
            held.release();
            assert_eq!(
                full_read_while_foreground_held(&runtime, &session, path, original),
                original,
                "{invalidation:?} cannot admit a paused write after invalidation"
            );
        })
        .checked("foreground-held production invalidation and independent full readback");
        cx.run_until_parked();
        if matches!(invalidation, Invalidation::AskMode) {
            assert!(
                p.read_with(cx, |p, _| p.agent_snapshot_for_test())
                    .is_none()
            );
        } else {
            assert_eq!(phase(&p, cx), AgentPhase::OutcomeUnknown);
            assert_eq!(
                p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
                Some(keelshell_ai::AgentOutcome::Unknown)
            );
        }
        p.update(cx, |p, cx| {
            p.agent_action_finished(
                run_id,
                action_id,
                keelshell_ai::AgentOutcome::Completed {
                    exit_status: None,
                    output: "obsolete invalidation success".into(),
                },
                cx,
            )
        });
        if !matches!(invalidation, Invalidation::AskMode) {
            assert_eq!(phase(&p, cx), AgentPhase::OutcomeUnknown);
            assert_eq!(
                p.read_with(cx, |p, _| p.agent_last_outcome_for_test()),
                Some(keelshell_ai::AgentOutcome::Unknown)
            );
        }
        assert_eq!(model.count(), 1);
    }
}

#[gpui_kit::test]
async fn reviewed_agent_obsolete_queued_events_cannot_retire_or_rebind_next_run(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = crate::workspace::tests::batch_peer::Server::new(&runtime, 0);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), peer.session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"command","command":"old unapproved command"}),
        json!({"kind":"command","command":"new independently approved command"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    let (old_run, old_action) = f
        .workspace
        .read_with(cx, |w, _| w.agent_pending_for_test())
        .checked("old queued identity");
    let old_token = f
        .workspace
        .read_with(cx, |w, _| w.agent_cancellation_for_test())
        .checked("old token");
    click(&f, "assistant-stop-agent", cx);
    assert!(old_token.is_cancelled());
    request(&f, &p, cx).await;
    let (new_run, new_action) = f
        .workspace
        .read_with(cx, |w, _| w.agent_pending_for_test())
        .checked("fresh independent identity");
    assert_ne!(new_run, old_run);
    let new_token = f
        .workspace
        .read_with(cx, |w, _| w.agent_cancellation_for_test())
        .checked("fresh backend token");
    let session_id = p.read_with(cx, |p, _| p.captured_context_for_test().2.to_owned());
    let stale_acceptance = keelshell_ai::RequestCancellation::new();
    p.update(cx, |p, cx| {
        p.agent_target_accepted(old_run, Some(stale_acceptance.clone()), cx);
        p.agent_action_finished(
            old_run,
            old_action,
            keelshell_ai::AgentOutcome::Completed {
                exit_status: Some(0),
                output: "obsolete receipt".into(),
            },
            cx,
        );
        cx.emit(crate::assistant::AssistantEvent::Capture {
            selection_only: false,
            previous_run: Some(old_run),
        });
        cx.emit(crate::assistant::AssistantEvent::AgentStop { run_id: old_run });
        cx.emit(crate::assistant::AssistantEvent::AgentStart {
            run_id: old_run,
            session_id,
        });
        cx.emit(crate::assistant::AssistantEvent::AgentExecute {
            run_id: old_run,
            action_id: old_action,
            action: AgentAction::Command {
                command: "obsolete execute".into(),
            },
        });
    });
    // Delayed Change from an earlier edit describes no change to the new
    // captured question and must not retire the replacement run.
    let prompt = p.read_with(cx, |p, _| p.agent_prompt_for_test());
    prompt.update(cx, |_, cx| {
        cx.emit(gpui_kit::component::input::InputEvent::Change)
    });
    cx.run_until_parked();
    assert!(stale_acceptance.is_cancelled());
    assert!(!new_token.is_cancelled());
    assert!(
        f.workspace
            .read_with(cx, |w, _| w.agent_backend_current_for_test())
    );
    assert_eq!(
        f.workspace.read_with(cx, |w, _| w.agent_pending_for_test()),
        Some((new_run, new_action))
    );
    assert_eq!(phase(&p, cx), AgentPhase::AwaitingAction);
    assert!(peer.requests().is_empty());
    click(&f, "agent-approve-action", cx);
    wait_phase(&f, &p, AgentPhase::Ready, cx).await;
    assert_eq!(
        peer.requests(),
        vec![b"new independently approved command".to_vec()]
    );
    assert_eq!(model.count(), 2);
}

#[gpui_kit::test]
async fn reviewed_agent_stop_and_edit_question_is_reachable_in_small_bilingual_themes(
    cx: &mut TestAppContext,
) {
    for language in [Language::ZhCn, Language::En] {
        for theme in [
            keelshell_core::Theme::System,
            keelshell_core::Theme::Light,
            keelshell_core::Theme::Dark,
        ] {
            let f = mount_sized(cx, Vec::new(), 900., 580.);
            let panes = attach_remote_panes(&f, cx);
            let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
            let peer = crate::workspace::tests::batch_peer::Server::new(&runtime, 0);
            f.workspace.update(cx, |w, cx| {
                w.remote_sessions
                    .insert(panes[0].terminal.entity_id(), peer.session.clone());
                w.active = 0;
                cx.notify();
            });
            let model = Model::new(vec![
                json!({"kind":"command","command":"unapproved small-window command"}),
            ]);
            let p = panel(&f, &model, cx);
            request(&f, &p, cx).await;
            let token = f
                .workspace
                .read_with(cx, |w, _| w.agent_cancellation_for_test())
                .checked("small layout grant");
            let prompt = p.read_with(cx, |p, _| p.agent_prompt_for_test());
            cx.update_window(f.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.scroll(
                    "assistant-scroll",
                    gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                    cx,
                );
                window.render_frame(cx);
                let edit = window.find("assistant-edit-agent-question");
                let bounds = edit.bounds();
                assert!(
                    bounds.top() >= px(0.)
                        && bounds.bottom() <= px(580.)
                        && bounds.left() >= px(0.)
                        && bounds.right() <= px(900.)
                );
                assert_eq!(
                    edit.label().checked("localized question edit label"),
                    match language {
                        Language::ZhCn => "编辑问题并停止",
                        Language::En => "Stop and edit question",
                    }
                );
                assert!(window.find("assistant-stop-agent").bounds().bottom() <= px(580.));
                window.click("assistant-edit-agent-question", cx);
                assert!(token.is_cancelled());
                assert!(prompt.read(cx).is_editable());
                window.render_frame(cx);
                window.render_frame(cx);
                let question = window.find("assistant-question").bounds();
                assert!(question.top() >= px(0.) && question.bottom() <= px(580.), "editing must reveal the actual question from the scrolled history: {question:?}");
                window.press("secondary-a", cx);
                window.input("fresh question 中文", cx);
                assert_eq!(prompt.read(cx).value(), "fresh question 中文");
            })
            .checked("six real small-window edit paths");
            assert_eq!(phase(&p, cx), AgentPhase::Stopped);
            assert!(peer.requests().is_empty());
            assert_eq!(model.count(), 1);
        }
    }
}

fn seed(
    runtime: &tokio::runtime::Runtime,
    session: &keelshell_session::SshSession,
    path: &str,
    content: &[u8],
) {
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("owned closed producer seed");
        sftp.write(path, content)
            .await
            .checked("producer original bytes");
        sftp.close().await.checked("close producer seed");
    });
}
fn read_while_foreground_held(
    runtime: &tokio::runtime::Runtime,
    session: &keelshell_session::SshSession,
    path: &str,
    original: &[u8],
) -> Vec<u8> {
    full_read_while_foreground_held(runtime, session, path, original)
}

#[gpui_kit::test]
async fn independent_agent_closed_lifecycle_producer_blocks_paused_write_without_foreground_poll(
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
        drop(signal);
        // Keep this foreground turn occupied: neither the 16ms terminal poll,
        // its observer, nor the 200ms workspace maintenance can process End.
        assert!(
            panes[0].terminal.read(cx).is_open(),
            "producer closure is not yet published by Terminal.poll"
        );
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
