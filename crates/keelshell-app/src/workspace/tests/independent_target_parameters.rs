//! Fresh nonauthor production-control probes with owned, explicitly stopped SSH peers.
use super::*;
use gpui_kit::{
    App, ElementId, ScrollDelta, Window,
    component::{WindowExt, input::AnyInputState},
};
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::{
    ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{collections::HashMap, sync::Mutex};
use tokio::{net::TcpListener, sync::oneshot, task::JoinSet};
use zeroize::Zeroizing;

struct Peer {
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    channels: HashMap<ChannelId, russh::Channel<server::Msg>>,
}
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "reviewer" && password == "owned-fixture" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.requests
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(data.to_vec());
        session.channel_success(id)?;
        if data.starts_with(b"hold ") {
            return Ok(());
        }
        session.data(id, b"owned reviewer receipt".to_vec())?;
        session.exit_status_request(id, 0)?;
        session.eof(id)?;
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        Ok(())
    }
}

struct OwnedServer {
    session: SshSession,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
    runtime: Arc<tokio::runtime::Runtime>,
}
impl OwnedServer {
    fn new(runtime: Arc<tokio::runtime::Runtime>) -> Self {
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("owned listener");
        let port = listener.local_addr().checked("owned address").port();
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x71; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = requests.clone();
        let (stop, mut stopped) = oneshot::channel();
        let task = runtime.spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break };
                        let config = config.clone();
                        let peer = Peer { requests: observed.clone(), channels: HashMap::new() };
                        clients.spawn(async move {
                            if let Ok(connection) = server::run_stream(config, socket, peer).await {
                                let _ = connection.await;
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
            clients.abort_all();
            while clients.join_next().await.is_some() {}
        });
        let mut options = SshOptions::new("127.0.0.1", "reviewer");
        options.port = port;
        options.timeout = Duration::from_secs(5);
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("owned-fixture".into()));
        let session = runtime
            .block_on(SshSession::connect(options))
            .checked("owned authenticated SSH");
        Self {
            session,
            requests,
            stop: Some(stop),
            task: Some(task),
            runtime,
        }
    }
    fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().checked("owned observations").clone()
    }
    fn shutdown(&mut self) {
        let Some(mut task) = self.task.take() else {
            return;
        };
        self.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), self.session.close())
                .await
                .checked("bounded SSH stop")
                .checked("SSH close");
            tokio::time::timeout(Duration::from_secs(5), async {
                while !self.session.is_closed() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .checked("SSH close observed");
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(result) => result.checked("listener and client tasks joined"),
                Err(_) => {
                    task.abort();
                    let _ = tokio::time::timeout(Duration::from_secs(5), &mut task)
                        .await
                        .checked("aborted listener joined");
                    panic!("owned listener did not stop in bounded time");
                }
            }
        });
        eprintln!("owned reviewer SSH closed, listener stopped, clients aborted and joined");
    }
}
impl Drop for OwnedServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Harness {
    fixture: Fixture,
    panes: Vec<RemotePane>,
    servers: Vec<OwnedServer>,
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let fixture = mount(cx, Vec::new());
        let panes = attach_remote_panes(&fixture, cx);
        let runtime = fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        let servers = (0..2)
            .map(|_| OwnedServer::new(runtime.clone()))
            .collect::<Vec<_>>();
        fixture.workspace.update(cx, |view, cx| {
            for (pane, server) in panes.iter().zip(&servers) {
                view.remote_sessions
                    .insert(pane.terminal.entity_id(), server.session.clone());
            }
            cx.notify();
        });
        cx.simulate_window_resize(fixture.window, size(px(900.), px(580.)));
        cx.run_until_parked();
        Self {
            fixture,
            panes,
            servers,
        }
    }
    fn open(&self, source: &str, workflow: bool, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            self.fixture.workspace.update(cx, |view, cx| {
                view.set_reviewed_command(
                    source.into(),
                    Some(self.panes[0].terminal.entity_id()),
                    window,
                    cx,
                );
                if workflow {
                    view.open_workflow(false, window, cx);
                } else {
                    view.open_batch_commands(false, window, cx);
                }
            });
        })
        .checked("open production review surface");
        cx.run_until_parked();
    }
    fn no_requests(&self) {
        assert!(
            self.servers
                .iter()
                .all(|server| server.requests().is_empty())
        );
    }
    fn shutdown(&mut self) {
        for server in &mut self.servers {
            server.shutdown();
        }
    }
}

fn click_visible(
    window: &mut Window,
    body: &'static str,
    id: impl Into<ElementId> + Clone,
    cx: &mut App,
) {
    window.render_frame(cx);
    let viewport = window.find(body).bounds();
    let bounds = window.find(id.clone()).bounds();
    let delta = if bounds.bottom() > viewport.bottom() - px(8.) {
        viewport.bottom() - px(8.) - bounds.bottom()
    } else if bounds.origin.y < viewport.origin.y + px(8.) {
        viewport.origin.y + px(8.) - bounds.origin.y
    } else {
        px(0.)
    };
    if delta != px(0.) {
        window.scroll(body, ScrollDelta::Pixels(point(px(0.), delta)), cx);
        window.render_frame(cx);
    }
    let bounds = window.find(id.clone()).bounds();
    assert!(
        window.find(id.clone()).visible()
            && bounds.origin.y >= viewport.origin.y
            && bounds.bottom() <= viewport.bottom(),
        "owned last control is fully visible"
    );
    window.click(id, cx);
}
fn replace(
    window: &mut Window,
    text: &str,
    cx: &mut App,
) -> Entity<gpui_kit::component::input::TextareaState> {
    match window
        .focused_input(cx)
        .checked_option("actual keyboard focus")
    {
        AnyInputState::Textarea(field) => {
            field.update(cx, |input, cx| {
                input.set_selected_range(0..input.value().len(), cx);
                input.replace(text.to_owned(), window, cx);
            });
            field
        }
        _ => panic!("expected keyboard-accessible multiline parameter"),
    }
}
fn action(h: &Harness, id: &'static str, cx: &mut TestAppContext) {
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .checked("explicit production action");
    cx.run_until_parked();
}

#[gpui_kit::test]
async fn independent_batch_minimum_window_parameter_keyboard_review_and_new_draft_lifecycle(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    let source = format!(
        "printf '%s' {}",
        (0..12)
            .map(|i| format!("{{{{p{i}}}}}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    h.open(&source, false, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("batch-select", 0_usize), cx);
        window.click("batch-sync-parameters", cx);
        for index in 0..12 {
            click_visible(
                window,
                "batch-body",
                format!("batch-parameters-0-p{index}"),
                cx,
            );
            replace(window, &format!("value{index}"), cx);
        }
    })
    .checked("twelve actual parameter inputs");
    cx.run_until_parked();
    let mut captured = None;
    for theme in [
        keelshell_core::Theme::System,
        keelshell_core::Theme::Light,
        keelshell_core::Theme::Dark,
    ] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                click_visible(window, "batch-body", "batch-parameters-0-p11", cx);
                captured = Some(replace(window, "last 中文'\n\t", cx));
                let footer = window.find("batch-footer").bounds();
                assert!(footer.origin.y >= px(0.) && footer.bottom() <= px(580.));
                assert!(window.find("batch-review-button").visible());
                eprintln!("independent batch 900x580 {language:?}/{theme:?}: final field visible and keyboard input reached");
            }).checked("minimum batch bilingual/theme form reachability");
            cx.run_until_parked();
        }
    }
    let captured = captured.checked_option("last input identity");
    action(&h, "batch-hide", cx);
    action(&h, "command-batch", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "batch-body", "batch-parameters-0-p11", cx);
        let focused = window.focused_input(cx).checked_option("retained focused field");
        assert!(matches!(focused, AnyInputState::Textarea(ref field) if field.entity_id() == captured.entity_id()));
        assert_eq!(captured.read(cx).value(), "last 中文'\n\t");
    }).checked("hide/reopen retains original temporary value entity");
    action(&h, "batch-review-button", cx);
    h.no_requests();
    cx.update_window(h.fixture.window, |_, window, cx| {
        captured.update(cx, |field, cx| {
            field.set_value("unreviewed edit", window, cx)
        });
        window.render_frame(cx);
        if window.try_find("batch-confirm").is_some() {
            window.click("batch-confirm", cx);
        }
    })
    .checked("same-frame changed parameter cannot inherit old approval");
    cx.run_until_parked();
    h.no_requests();
    action(&h, "batch-back", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "batch-body", "batch-parameters-0-p11", cx);
        replace(window, "last 中文'\n\t", cx);
    })
    .checked("corrected actual input needs fresh review");
    cx.run_until_parked();
    action(&h, "batch-review-button", cx);
    h.no_requests();
    action(&h, "batch-confirm", cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(8), |_, cx| {
        h.fixture
            .workspace
            .read(cx)
            .batch_panel
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).is_running())
    })
    .await;
    let expected = format!(
        "printf '%s' {} 'last 中文'\\''\n\t'",
        (0..11)
            .map(|i| format!("'value{i}'"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert_eq!(h.servers[0].requests(), vec![expected.as_bytes().to_vec()]);
    assert!(h.servers[1].requests().is_empty());
    assert!(
        h.fixture
            .store
            .load()
            .checked("private audit metadata")
            .batch_audits
            .is_empty()
    );
    action(&h, "batch-new", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("batch-select", 0_usize), cx);
        window.click("batch-sync-parameters", cx);
        click_visible(window, "batch-body", "batch-parameters-0-p11", cx);
        match window.focused_input(cx).checked_option("new draft field") {
            AnyInputState::Textarea(field) => {
                assert_ne!(field.entity_id(), captured.entity_id());
                assert!(field.read(cx).value().is_empty());
            }
            _ => panic!("new draft textarea"),
        }
        window.click("batch-review-button", cx);
        window.render_frame(cx);
        assert!(window.try_find("batch-confirm").is_none());
    })
    .checked("new draft clears values and blank defaults remain missing");
    assert!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.command_histories.is_empty())
    );
    h.shutdown();
}

#[gpui_kit::test]
async fn independent_workflow_actual_union_input_replacement_refresh_and_empty_confirmation(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.open("printf '%s' {{path}}", true, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "workflow-body", ("workflow-target", 0_usize), cx);
        click_visible(window, "workflow-body", "workflow-add-task", cx);
        click_visible(window, "workflow-body", "workflow-command-container", cx);
        replace(window, "printf '%s' {{release}} {{path}}", cx);
        click_visible(window, "workflow-body", ("workflow-target", 0_usize), cx);
        click_visible(
            window,
            "workflow-body",
            ("workflow-dependency", 0_usize),
            cx,
        );
        click_visible(window, "workflow-body", "workflow-sync-parameters", cx);
    })
    .checked("actual dependent task, target and union synchronization");
    cx.run_until_parked();
    let target = h.fixture.workspace.read_with(cx, |view, cx| {
        view.workflow_panel
            .as_ref()
            .checked_option("workflow panel")
            .read(cx)
            .connected_destinations()
            .next()
            .checked_option("first target")
            .destination
            .id
    });
    let path_id = format!("workflow-parameters-{target}-path");
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "workflow-body", path_id.clone(), cx);
        replace(window, "original 中文'\n\t", cx);
        click_visible(
            window,
            "workflow-body",
            format!("workflow-parameters-{target}-empty-release"),
            cx,
        );
    })
    .checked("actual target value and explicit empty opt-in");
    cx.run_until_parked();
    for theme in [
        keelshell_core::Theme::System,
        keelshell_core::Theme::Light,
        keelshell_core::Theme::Dark,
    ] {
        for language in [Language::ZhCn, Language::En] {
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                click_visible(window, "workflow-body", path_id.clone(), cx);
                match window.focused_input(cx).checked_option("accessible workflow field") {
                    AnyInputState::Textarea(field) => assert_eq!(field.read(cx).value(), "original 中文'\n\t"),
                    _ => panic!("workflow parameter keyboard focus"),
                }
                let footer = window.find("workflow-footer").bounds();
                assert!(footer.origin.y >= px(0.) && footer.bottom() <= px(580.));
                eprintln!("independent workflow 900x580 {language:?}/{theme:?}: value field visible and keyboard reachable");
            }).checked("workflow minimum bilingual/theme input matrix");
        }
    }
    action(&h, "workflow-review-button", cx);
    h.no_requests();
    action(&h, "workflow-hide", cx);
    action(&h, "command-workflow", cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("workflow-confirm").visible());
        h.fixture.workspace.update(cx, |view, _| {
            view.remote_sessions.insert(
                h.panes[0].terminal.entity_id(),
                h.servers[1].session.clone(),
            );
        });
        window.render_frame(cx);
        if window.try_find("workflow-confirm").is_some() {
            window.click("workflow-confirm", cx);
        }
    })
    .checked("same-endpoint separately authenticated replacement expires complete review");
    cx.run_until_parked();
    h.no_requests();
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "workflow-body", "workflow-refresh-targets", cx);
    })
    .checked("explicit refresh request");
    cx.run_until_parked();
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(window, "workflow-body", "workflow-target-list", cx);
        click_visible(
            window,
            "workflow-target-list",
            ("workflow-target", 2_usize),
            cx,
        );
        click_visible(
            window,
            "workflow-body",
            ("workflow-task-select", 0_usize),
            cx,
        );
        click_visible(window, "workflow-body", "workflow-target-list", cx);
        click_visible(
            window,
            "workflow-target-list",
            ("workflow-target", 2_usize),
            cx,
        );
        click_visible(window, "workflow-body", "workflow-sync-parameters", cx);
    })
    .checked("replacement requires explicit target refresh and both task reselections");
    cx.run_until_parked();
    let fresh = h.fixture.workspace.read_with(cx, |view, cx| {
        view.workflow_panel
            .as_ref()
            .checked_option("refreshed panel")
            .read(cx)
            .connected_destinations()
            .last()
            .checked_option("refreshed target")
            .destination
            .id
    });
    assert_ne!(fresh, target);
    cx.update_window(h.fixture.window, |_, window, cx| {
        click_visible(
            window,
            "workflow-body",
            format!("workflow-parameters-{fresh}-path"),
            cx,
        );
        match window
            .focused_input(cx)
            .checked_option("replacement blank field")
        {
            AnyInputState::Textarea(field) => assert!(field.read(cx).value().is_empty()),
            _ => panic!("replacement field"),
        }
        replace(window, "replacement 中文'\n\t", cx);
        click_visible(
            window,
            "workflow-body",
            format!("workflow-parameters-{fresh}-empty-release"),
            cx,
        );
    })
    .checked("new authenticated connection receives fresh values and explicit empty selection");
    cx.run_until_parked();
    action(&h, "workflow-review-button", cx);
    h.no_requests();
    action(&h, "workflow-confirm", cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(8), |_, cx| {
        h.fixture
            .workspace
            .read(cx)
            .workflow_panel
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).is_running())
    })
    .await;
    assert!(h.servers[0].requests().is_empty());
    assert_eq!(
        h.servers[1].requests(),
        vec![
            "printf '%s' 'replacement 中文'\\''\n\t'"
                .as_bytes()
                .to_vec(),
            "printf '%s' '' 'replacement 中文'\\''\n\t'"
                .as_bytes()
                .to_vec()
        ]
    );
    assert!(
        h.fixture
            .store
            .load()
            .checked("workflow private audit metadata")
            .batch_audits
            .is_empty()
    );
    assert!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.command_histories.is_empty())
    );
    h.shutdown();
}

#[gpui_kit::test]
async fn independent_parameter_batch_cancel_hide_reopen_does_not_repeat_or_persist(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.open("hold {{path}}", false, cx);
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("batch-select", 0_usize), cx);
        window.click("batch-sync-parameters", cx);
        click_visible(window, "batch-body", "batch-parameters-0-path", cx);
        replace(window, "cancel-private", cx);
    })
    .checked("custom cancellation actual form");
    cx.run_until_parked();
    action(&h, "batch-review-button", cx);
    h.no_requests();
    action(&h, "batch-confirm", cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(8), |_, _| {
        !h.servers[0].requests().is_empty()
    })
    .await;
    action(&h, "batch-hide", cx);
    action(&h, "command-batch", cx);
    action(&h, "batch-cancel", cx);
    cx.wait_for(h.fixture.window, Duration::from_secs(8), |_, cx| {
        h.fixture
            .workspace
            .read(cx)
            .batch_panel
            .as_ref()
            .is_some_and(|panel| !panel.read(cx).is_running())
    })
    .await;
    action(&h, "batch-hide", cx);
    action(&h, "command-batch", cx);
    assert_eq!(
        h.servers[0].requests(),
        vec![b"hold 'cancel-private'".to_vec()]
    );
    assert!(h.servers[1].requests().is_empty());
    assert!(
        h.fixture
            .store
            .load()
            .checked("cancelled custom audit")
            .batch_audits
            .is_empty()
    );
    assert!(
        h.fixture
            .workspace
            .read_with(cx, |view, _| view.pending_batch_audits.is_empty()
                && view.command_histories.is_empty())
    );
    h.shutdown();
}
