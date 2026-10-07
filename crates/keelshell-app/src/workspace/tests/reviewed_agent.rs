//! Real controlled GPUI controls, HTTP inference and pinned TCP SSH/SFTP peers.
//! These peers do not execute shell commands and do not prove native acceptance.
use super::*;
use crate::assistant::AssistantPanel;
use keelshell_ai::{AgentAction, AgentPhase};
use keelshell_core::{AiApiStyle, AiAuthentication, AiPreset, AiProfileCatalog, NamedAiProfile};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Mutex, atomic::Ordering},
    time::Instant,
};

mod invalidation;
mod lifecycle;

impl<T> Checked<T> for Option<T> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|| panic!("{operation}: missing value"))
    }
}

struct Model {
    endpoint: String,
    style: AiApiStyle,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    job: Option<std::thread::JoinHandle<()>>,
}
impl Model {
    fn new(actions: Vec<Value>) -> Self {
        Self::with_style(AiApiStyle::ChatCompletions, actions)
    }
    fn with_style(style: AiApiStyle, actions: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").checked("owned inference bind");
        listener
            .set_nonblocking(true)
            .checked("bounded inference listener");
        let path = match style {
            AiApiStyle::ChatCompletions => "chat/completions",
            AiApiStyle::Responses => "responses",
            AiApiStyle::AnthropicMessages => "messages",
        };
        let endpoint = format!(
            "http://{}/v1/{path}",
            listener.local_addr().checked("owned model address")
        );
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let job = std::thread::spawn(move || {
            for action in actions {
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut stream = loop {
                    if stopped.load(Ordering::Acquire) {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("bounded inference accept: {e}"),
                    }
                };
                stream
                    .set_nonblocking(false)
                    .checked("blocking bounded stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .checked("HTTP read deadline");
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .checked("HTTP write deadline");
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).checked("owned HTTP header");
                    header.push(byte[0]);
                    assert!(header.len() <= 4096);
                }
                let header = String::from_utf8(header).checked("HTTP header UTF8");
                let length = header
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .checked("HTTP length");
                assert!(length <= 1024 * 1024);
                let mut body = vec![0; length];
                stream
                    .read_exact(&mut body)
                    .checked("exact approved HTTP body");
                captured
                    .lock()
                    .checked("owned request capture")
                    .push(serde_json::from_slice(&body).checked("request JSON"));
                let text =
                    json!({"explanation":"bounded evidence-based next step","action":action})
                        .to_string();
                let response = match style {
                    AiApiStyle::ChatCompletions => json!({"choices":[{"message":{"role":"assistant","content":text}}]}),
                    AiApiStyle::Responses => json!({"output":[{"type":"message","content":[{"type":"output_text","text":text}]}]}),
                    AiApiStyle::AnthropicMessages => json!({"role":"assistant","content":[{"type":"text","text":text}]}),
                }.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).checked("owned model response");
                stream.flush().checked("owned response flush");
            }
        });
        Self {
            endpoint,
            style,
            requests,
            stop,
            job: Some(job),
        }
    }
    fn count(&self) -> usize {
        self.requests.lock().checked("request count").len()
    }
    fn bodies(&self) -> Vec<Value> {
        self.requests.lock().checked("request bodies").clone()
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.job.as_ref().is_some_and(|j| !j.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(job) = self.job.take() {
            assert!(job.is_finished(), "owned inference cleanup deadline");
            job.join().checked("join owned inference");
        }
    }
}
fn panel(f: &Fixture, model: &Model, cx: &mut TestAppContext) -> Entity<AssistantPanel> {
    let panel = f.workspace.read_with(cx, |w, _| w.assistant.clone());
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "Owned Agent".into();
    profile.api_style = model.style;
    profile.endpoint = model.endpoint.clone();
    profile.model = "owned-agent-model".into();
    profile.authentication = AiAuthentication::None;
    panel.update(cx, |p, cx| {
        p.set_profiles(
            &AiProfileCatalog {
                active_id: Some(profile.id),
                profiles: vec![profile],
            },
            &crate::ai_settings::EphemeralCredentials::new(),
            cx,
        )
    });
    cx.update_window(f.window, |_, window, cx| {
        f.workspace.update(cx, |w, cx| {
            w.show_assistant = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("context-screen", cx);
        window.render_frame(cx);
        window.click("assistant-mode-agent", cx);
        panel.update(cx, |p, cx| {
            p.set_agent_question_for_test("Investigate the selected remote evidence", window, cx)
        });
    })
    .checked("capture exact terminal and select real Agent mode");
    cx.run_until_parked();
    panel
}
fn click(f: &Fixture, id: &'static str, cx: &mut TestAppContext) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        if id == "prepare-request" {
            window.scroll(
                "assistant-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                cx,
            );
            window.render_frame(cx);
        }
        window.click(id, cx);
    })
    .checked("click production Agent control");
    cx.run_until_parked();
}
fn phase(p: &Entity<AssistantPanel>, cx: &mut TestAppContext) -> AgentPhase {
    p.read_with(cx, |p, _| {
        p.agent_snapshot_for_test().checked("agent snapshot").1
    })
}
async fn wait_phase(
    f: &Fixture,
    p: &Entity<AssistantPanel>,
    expected: AgentPhase,
    cx: &mut TestAppContext,
) {
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        f.workspace.update(cx, |w, cx| w.maintain_agent(cx));
        p.read(cx)
            .agent_snapshot_for_test()
            .is_some_and(|s| s.1 == expected)
    })
    .await;
}
async fn request(f: &Fixture, p: &Entity<AssistantPanel>, cx: &mut TestAppContext) {
    click(f, "prepare-request", cx);
    assert!(
        p.read_with(cx, |p, _| p.agent_preview_for_test()).is_some(),
        "{}; {:?}",
        p.read_with(cx, |p, cx| p.agent_status_for_test(cx)),
        p.read_with(cx, |p, _| p.agent_snapshot_for_test())
    );
    click(f, "send-approved-request", cx);
    wait_phase(f, p, AgentPhase::AwaitingAction, cx).await;
}

#[gpui_kit::test]
async fn reviewed_agent_http_ssh_loop_has_real_rounds_reviews_results_and_finish(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = super::batch_peer::Server::new(&runtime, 7);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), peer.session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"command","command":"  printf 'first 中文'"}),
        json!({"kind":"command","command":"second must be rejected"}),
        json!({"kind":"finish","summary":"Confirmed exit 7; rejected second command was not executed"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    assert_eq!(model.count(), 1);
    assert!(peer.requests().is_empty(), "model text cannot execute");
    click(&f, "agent-approve-action", cx);
    wait_phase(&f, &p, AgentPhase::Ready, cx).await;
    assert_eq!(
        peer.requests(),
        vec!["  printf 'first 中文'".as_bytes().to_vec()]
    );
    assert_eq!(model.count(), 1, "results cannot be shared automatically");
    click(&f, "agent-prepare-next", cx);
    let preview = p
        .read_with(cx, |p, _| p.agent_preview_for_test())
        .checked("full next context preview");
    assert!(
        preview.contains("fixture stdout")
            && preview.contains("exit_status")
            && preview.contains('7')
    );
    assert_eq!(model.count(), 1, "preview is not a network request");
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::AwaitingAction, cx).await;
    click(&f, "agent-reject-action", cx);
    assert_eq!(phase(&p, cx), AgentPhase::Ready);
    assert_eq!(peer.requests().len(), 1);
    click(&f, "agent-prepare-next", cx);
    assert!(
        p.read_with(cx, |p, _| p.agent_preview_for_test())
            .checked("rejection preview")
            .contains("rejected")
    );
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::Completed, cx).await;
    assert_eq!(model.count(), 3);
    assert_eq!(peer.requests().len(), 1);
    assert_eq!(
        p.read_with(cx, |p, _| p.agent_snapshot_for_test())
            .checked("final snapshot")
            .2,
        3
    );
    assert!(model.bodies()[2].to_string().contains("rejected"));
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(f.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            assert!(
                window
                    .find("agent-captured-target")
                    .label()
                    .checked("captured target AX")
                    .contains(&format!("{:?}", panes[0].terminal.entity_id()))
            );
        })
        .checked("dual language fixed target");
    }
}

#[gpui_kit::test]
async fn reviewed_agent_sftp_read_then_two_stage_file_review_and_exact_replacement(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = crate::files::test_server::Server::new(&runtime);
    let session = peer.connect(&runtime);
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("seed SFTP");
        sftp.write("/agent.txt", b"original owned content\n")
            .await
            .checked("seed existing file");
        sftp.close().await.checked("seed close");
    });
    let writes = peer.filesystem.transfer_writes_started();
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"read_file","path":"/agent.txt"}),
        json!({"kind":"write_file","path":"/agent.txt","replacement":"new owned 中文 content\n"}),
        json!({"kind":"finish","summary":"Reviewed replacement read back"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    assert_eq!(peer.filesystem.transfer_writes_started(), writes);
    click(&f, "agent-approve-action", cx);
    wait_phase(&f, &p, AgentPhase::Ready, cx).await;
    click(&f, "agent-prepare-next", cx);
    assert!(
        p.read_with(cx, |p, _| p.agent_preview_for_test())
            .checked("file read context review")
            .contains("original owned content")
    );
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::AwaitingAction, cx).await;
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(8), |_, cx| {
        p.read(cx).agent_file_review_ready_for_test()
    })
    .await;
    assert_eq!(phase(&p, cx), AgentPhase::AwaitingAction);
    assert_eq!(
        peer.filesystem.transfer_writes_started(),
        writes,
        "preparation never writes"
    );
    click(&f, "agent-approve-action", cx);
    wait_phase(&f, &p, AgentPhase::Ready, cx).await;
    runtime.block_on(async {
        let sftp = session.sftp().await.checked("independent SFTP readback");
        assert_eq!(
            sftp.read("/agent.txt", 32768)
                .await
                .checked("read replacement"),
            "new owned 中文 content\n".as_bytes()
        );
        sftp.close().await.checked("readback close");
    });
    assert!(peer.filesystem.transfer_writes_started() > writes);
    assert_eq!(model.count(), 2);
    click(&f, "agent-prepare-next", cx);
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::Completed, cx).await;
    assert_eq!(model.count(), 3);
}

#[gpui_kit::test]
async fn reviewed_agent_stop_pending_command_keeps_unknown_and_drops_late_outcome(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = super::batch_peer::Server::new(&runtime, 0);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), peer.session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![json!({"kind":"command","command":"hold"})]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    click(&f, "agent-approve-action", cx);
    cx.wait_for(f.window, Duration::from_secs(5), |_, _| {
        peer.request_count() == 1
    })
    .await;
    click(&f, "assistant-stop-agent", cx);
    assert_eq!(phase(&p, cx), AgentPhase::OutcomeUnknown);
    assert_eq!(model.count(), 1);
    assert!(f.workspace.read_with(cx, |w, _| w.agent.is_none()));
    cx.wait_for(f.window, Duration::from_secs(3), |_, cx| {
        p.read(cx)
            .agent_snapshot_for_test()
            .is_some_and(|s| s.1 == AgentPhase::OutcomeUnknown)
    })
    .await;
    assert_eq!(peer.request_count(), 1);
}

#[gpui_kit::test]
async fn reviewed_agent_does_not_rebind_replaced_ssh_handle_or_accept_tampered_action(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let old = super::batch_peer::Server::new(&runtime, 0);
    let new = super::batch_peer::Server::new(&runtime, 0);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), old.session.clone());
        w.active = 0;
        cx.notify();
    });
    let model = Model::new(vec![
        json!({"kind":"command","command":"fixed original command"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    f.workspace.update(cx, |w, cx| {
        let (run_id, id) = w
            .agent_pending_for_test()
            .checked("registered target and proposal");
        w.execute_agent_action(
            run_id,
            id,
            &AgentAction::Command {
                command: "tampered command".into(),
            },
            cx,
        );
    });
    assert!(old.requests().is_empty() && new.requests().is_empty());
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), new.session.clone());
        w.maintain_agent(cx);
    });
    assert_eq!(phase(&p, cx), AgentPhase::TargetLost);
    assert!(f.workspace.read_with(cx, |w, _| w.agent.is_none()));
    assert_eq!(model.count(), 1);
    assert!(old.requests().is_empty() && new.requests().is_empty());
}

#[gpui_kit::test]
async fn reviewed_agent_shared_api_profiles_complete_real_loop_for_all_three_protocols(
    cx: &mut TestAppContext,
) {
    for style in [
        AiApiStyle::ChatCompletions,
        AiApiStyle::Responses,
        AiApiStyle::AnthropicMessages,
    ] {
        let f = mount_sized(cx, Vec::new(), 1440., 900.);
        let panes = attach_remote_panes(&f, cx);
        let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
        let peer = super::batch_peer::Server::new(&runtime, 0);
        f.workspace.update(cx, |w, cx| {
            w.remote_sessions
                .insert(panes[0].terminal.entity_id(), peer.session.clone());
            w.active = 0;
            cx.notify();
        });
        let model = Model::with_style(
            style,
            vec![
                json!({"kind":"command","command":"protocol exact probe"}),
                json!({"kind":"finish","summary":"supplied zero exit receipt"}),
            ],
        );
        let p = panel(&f, &model, cx);
        request(&f, &p, cx).await;
        assert!(peer.requests().is_empty());
        click(&f, "agent-approve-action", cx);
        wait_phase(&f, &p, AgentPhase::Ready, cx).await;
        click(&f, "agent-prepare-next", cx);
        assert_eq!(model.count(), 1);
        click(&f, "send-approved-request", cx);
        wait_phase(&f, &p, AgentPhase::Completed, cx).await;
        assert_eq!(peer.requests(), vec![b"protocol exact probe".to_vec()]);
        assert_eq!(model.count(), 2);
        for body in model.bodies() {
            assert!(body.to_string().contains("reviewed Agent protocol v1"));
            assert!(body.get("tools").is_none());
            match style {
                AiApiStyle::ChatCompletions => assert_eq!(body["messages"][0]["role"], "system"),
                AiApiStyle::Responses => {
                    assert!(body["input"].is_string() && body["instructions"].is_string())
                }
                AiApiStyle::AnthropicMessages => {
                    assert!(body["system"].is_string() && body["max_tokens"].is_number())
                }
            }
        }
    }
}

#[gpui_kit::test]
async fn reviewed_agent_invalid_decision_and_expired_review_never_dispatch(
    cx: &mut TestAppContext,
) {
    for malformed in [true, false] {
        let f = mount_sized(cx, Vec::new(), 1440., 900.);
        let panes = attach_remote_panes(&f, cx);
        let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
        let peer = super::batch_peer::Server::new(&runtime, 0);
        f.workspace.update(cx, |w, cx| {
            w.remote_sessions
                .insert(panes[0].terminal.entity_id(), peer.session.clone());
            w.active = 0;
            cx.notify();
        });
        let action = if malformed {
            json!({"kind":"automatic_tool","command":"unreviewed"})
        } else {
            json!({"kind":"command","command":"expired"})
        };
        let model = Model::new(vec![action]);
        let p = panel(&f, &model, cx);
        click(&f, "prepare-request", cx);
        click(&f, "send-approved-request", cx);
        if malformed {
            wait_phase(&f, &p, AgentPhase::Failed, cx).await;
            assert!(f.workspace.read_with(cx, |w, _| w.agent.is_none()));
        } else {
            wait_phase(&f, &p, AgentPhase::AwaitingAction, cx).await;
            f.workspace
                .update(cx, |w, _| w.expire_agent_review_for_test());
            click(&f, "agent-approve-action", cx);
            assert_eq!(phase(&p, cx), AgentPhase::Ready);
        }
        assert_eq!(model.count(), 1);
        assert!(peer.requests().is_empty());
    }
}

#[gpui_kit::test]
async fn reviewed_agent_missing_original_refuses_write_and_releases_next_proposal(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 1440., 900.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = super::batch_peer::Server::new(&runtime, 0);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), peer.session.clone());
        w.active = 0;
        cx.notify();
    });
    let action = AgentAction::WriteFile {
        path: "/must-review-original.txt".into(),
        replacement: "replacement must never be sent".into(),
    };
    let model = Model::new(vec![
        serde_json::to_value(&action).checked("controlled write proposal"),
        json!({"kind":"command","command":"after known refusal"}),
        json!({"kind":"finish","summary":"The write was refused; only the next independently approved command ran"}),
    ]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    let (run_id, action_id) = f
        .workspace
        .read_with(cx, |w, _| w.agent_pending_for_test())
        .checked("exact pending proposal identity");
    // Exercise a programmatic event without the UI's original-content guard.
    // The root owner must fail before SFTP and release this exact proposal.
    f.workspace.update(cx, |w, cx| {
        w.execute_agent_action(run_id, action_id, &action, cx)
    });
    assert_eq!(phase(&p, cx), AgentPhase::Ready);
    assert!(
        f.workspace
            .read_with(cx, |w, _| w.agent_pending_for_test())
            .is_none()
    );
    assert!(peer.requests().is_empty());
    click(&f, "agent-prepare-next", cx);
    let preview = p
        .read_with(cx, |p, _| p.agent_preview_for_test())
        .checked("review complete known-refusal context");
    assert!(preview.contains("reviewed original file snapshot is required"));
    assert_eq!(model.count(), 1, "known refusal cannot trigger inference");
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::AwaitingAction, cx).await;
    click(&f, "agent-approve-action", cx);
    wait_phase(&f, &p, AgentPhase::Ready, cx).await;
    assert_eq!(peer.requests(), vec![b"after known refusal".to_vec()]);
    click(&f, "agent-prepare-next", cx);
    click(&f, "send-approved-request", cx);
    wait_phase(&f, &p, AgentPhase::Completed, cx).await;
    assert_eq!(model.count(), 3);
}

#[gpui_kit::test]
async fn reviewed_agent_small_window_keeps_exact_review_and_confirmation_visible(
    cx: &mut TestAppContext,
) {
    let f = mount_sized(cx, Vec::new(), 900., 580.);
    let panes = attach_remote_panes(&f, cx);
    let runtime = f.workspace.read_with(cx, |w, _| w.runtime.clone());
    let peer = super::batch_peer::Server::new(&runtime, 0);
    f.workspace.update(cx, |w, cx| {
        w.remote_sessions
            .insert(panes[0].terminal.entity_id(), peer.session.clone());
        w.active = 0;
        cx.notify();
    });
    let long = "printf '".to_owned()
        + &"中文long ".repeat(500)
        + "'\n"
        + &"printf 'vertical row'\n".repeat(100)
        + "printf 'end marker'";
    let model = Model::new(vec![json!({"kind":"command","command":long})]);
    let p = panel(&f, &model, cx);
    request(&f, &p, cx).await;
    for language in [Language::ZhCn, Language::En] {
        for theme in [
            keelshell_core::Theme::System,
            keelshell_core::Theme::Light,
            keelshell_core::Theme::Dark,
        ] {
            cx.update_window(f.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                p.update(cx, |p, _| p.reset_agent_step_scroll_for_test());
                window.render_frame(cx);
                let before = window.find("agent-approve-action").bounds();
                let footer = window.find("agent-confirmation-footer").bounds();
                let stop = window.find("assistant-stop-agent").bounds();
                assert!(
                    before.top() >= footer.top()
                        && before.bottom() <= px(580.)
                        && stop.bottom() <= px(580.)
                );
                assert!(before.left() >= px(0.) && before.right() <= px(900.));
                window.scroll(
                    "assistant-scroll",
                    gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                    cx,
                );
                window.render_frame(cx);
                let exact = window.find(("agent-exact", 0_usize)).bounds();
                assert!(exact.size.height > px(0.));
                window.scroll(
                    ("agent-exact", 0_usize),
                    gpui_kit::ScrollDelta::Lines(point(-1000., 0.)),
                    cx,
                );
                window.render_frame(cx);
                window.scroll(
                    ("agent-exact", 0_usize),
                    gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                    cx,
                );
                window.render_frame(cx);
                let offset = p
                    .read(cx)
                    .agent_step_offset_for_test()
                    .checked("persistent exact-text scroll");
                assert!(
                    offset.x < px(0.) && offset.y < px(0.),
                    "both review axes must actually move: {offset:?}"
                );
                assert_eq!(
                    window.find("agent-approve-action").bounds(),
                    before,
                    "fixed footer survives both scroll axes"
                );
            })
            .checked("controlled six language/theme minimum-window combinations");
        }
    }
    assert!(peer.requests().is_empty());
    assert_eq!(model.count(), 1);
}
