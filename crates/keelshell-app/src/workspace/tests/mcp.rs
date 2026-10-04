//! Real GPUI consent/review actions over pinned loopback SSH handles.
#[path = "mcp_first_proposal.rs"]
mod first_proposal;

use super::*;
use gpui_kit::ScrollDelta;
use keelshell_mcp::{ActionState, KeelShellMcpServer, McpFailure, SessionIdentity, ToolKind};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use super::batch_peer as command_peer;
use crate::files::test_server as file_peer;

struct Harness {
    fixture: Fixture,
    panes: Vec<RemotePane>,
    command: command_peer::Server,
    files: file_peer::Server,
    runtime: Arc<tokio::runtime::Runtime>,
    initial_writes: usize,
}

struct StdioClient {
    stream: Arc<tokio::sync::Mutex<tokio::io::DuplexStream>>,
    bridge: tokio::task::JoinHandle<Result<(), keelshell_mcp::IpcFailure>>,
}
impl StdioClient {
    fn new(h: &Harness, cx: &mut TestAppContext) -> Self {
        let client = h
            .fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_client())
            .checked("enabled runtime capability");
        let (agent, stdio) = tokio::io::duplex(4096);
        let (reader, writer) = tokio::io::split(stdio);
        Self {
            stream: Arc::new(tokio::sync::Mutex::new(agent)),
            bridge: h
                .runtime
                .spawn(async move { client.bridge(reader, writer).await }),
        }
    }
    async fn rpc(&self, request: Value, h: &Harness, cx: &mut TestAppContext) -> Value {
        let stream = self.stream.clone();
        let job = h.runtime.spawn(async move {
            let mut stream = stream.lock().await;
            let mut bytes = serde_json::to_vec(&request).checked("serialize fixture request");
            bytes.push(b'\n');
            stream
                .write_all(&bytes)
                .await
                .checked("write owned stdio request");
            stream.flush().await.checked("flush owned stdio request");
            let mut line = String::new();
            let mut reader = tokio::io::BufReader::new(&mut *stream);
            tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line))
                .await
                .checked("bounded stdio reply")
                .checked("read owned stdio reply");
            serde_json::from_str(&line).checked("parse desktop JSON-RPC")
        });
        cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
            h.fixture
                .workspace
                .update(cx, |view, cx| view.maintain_mcp(cx));
            job.is_finished()
        })
        .await;
        h.runtime.block_on(job).checked("join owned bridge RPC")
    }
    async fn call(
        &self,
        id: usize,
        tool: &str,
        arguments: Value,
        h: &Harness,
        cx: &mut TestAppContext,
    ) -> Value {
        let reply = self.rpc(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":tool,"arguments":arguments}}), h, cx).await;
        assert_eq!(reply["id"], id);
        assert!(reply.get("error").is_none(), "fixed RPC error");
        assert_ne!(reply["result"]["isError"], true, "fixed tool error");
        reply["result"]["structuredContent"].clone()
    }
}
impl Drop for StdioClient {
    fn drop(&mut self) {
        self.bridge.abort();
    }
}
impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        let fixture = mount_sized(cx, Vec::new(), 1440., 900.);
        let panes = attach_remote_panes(&fixture, cx);
        let runtime = fixture
            .workspace
            .read_with(cx, |view, _| view.runtime.clone());
        let command = command_peer::Server::new(&runtime, 0);
        let files = file_peer::Server::new(&runtime);
        let session = files.connect(&runtime);
        runtime.block_on(async {
            let sftp = session.sftp().await.checked("seed MCP SFTP");
            sftp.mkdir("/approved").await.checked("seed granted root");
            sftp.write("/approved/中文.txt", "受控中文\n".as_bytes())
                .await
                .checked("seed UTF8");
            sftp.write("/approved/binary", &[255, 0])
                .await
                .checked("seed nonUTF8");
            sftp.write("/approved/large", &vec![b'x'; 1024])
                .await
                .checked("seed oversized");
            sftp.write("/outside", b"not granted")
                .await
                .checked("seed outside");
            sftp.close().await.checked("close seed");
        });
        files
            .filesystem
            .insert_symlink("/approved/link")
            .checked("seed link");
        fixture.workspace.update(cx, |view, cx| {
            view.remote_sessions
                .insert(panes[0].terminal.entity_id(), command.session.clone());
            view.remote_sessions
                .insert(panes[1].terminal.entity_id(), session);
            view.active = 0;
            cx.notify();
        });
        let initial_writes = files.filesystem.transfer_writes_started();
        Self {
            fixture,
            panes,
            command,
            files,
            runtime,
            initial_writes,
        }
    }
    fn server(&self, cx: &mut TestAppContext) -> KeelShellMcpServer {
        self.fixture
            .workspace
            .read_with(cx, |view, _| view.mcp_test_server())
    }
    async fn call(
        &self,
        name: &'static str,
        args: Value,
        cx: &mut TestAppContext,
    ) -> Result<Value, McpFailure> {
        let server = self.server(cx);
        let job = self
            .runtime
            .spawn(async move { server.invoke(name, args, CancellationToken::new()).await });
        cx.wait_for(self.fixture.window, Duration::from_secs(7), |_, cx| {
            self.fixture
                .workspace
                .update(cx, |view, cx| view.maintain_mcp(cx));
            job.is_finished()
        })
        .await;
        self.runtime
            .block_on(job)
            .checked("join bounded MCP request")?
            .structured_content
            .ok_or(McpFailure::BackendFailure)
    }
    async fn grant(&self, tab: usize, tools: &[ToolKind], root: &str, cx: &mut TestAppContext) {
        cx.update_window(self.fixture.window, |_, window, cx| {
            self.fixture.workspace.update(cx, |view, cx| {
                view.mcp.show = false;
                view.active = tab;
                view.open_mcp(window, cx);
                view.mcp
                    .root
                    .update(cx, |input, cx| input.set_value(root, window, cx));
            });
            window.render_frame(cx);
            for tool in tools {
                window.click(tool.name(), cx);
            }
            if tools.contains(&ToolKind::ReadSelection) {
                window.click("mcp-capture-selection", cx);
            }
            window.click("mcp-grant-session", cx);
        })
        .checked("explicit capability selection and grant");
        cx.wait_for(self.fixture.window, Duration::from_secs(7), |_, cx| {
            self.fixture
                .workspace
                .update(cx, |view, cx| view.maintain_mcp(cx));
            self.fixture.workspace.read(cx).mcp_test_enabled()
        })
        .await;
    }
    async fn target(&self, cx: &mut TestAppContext) -> SessionIdentity {
        let reply = self
            .call("keelshell_list_sessions", json!({}), cx)
            .await
            .checked("list granted SSH");
        serde_json::from_value(reply["sessions"][0]["target"].clone()).checked("exact target")
    }
    async fn propose(
        &self,
        target: SessionIdentity,
        command: &str,
        cx: &mut TestAppContext,
    ) -> uuid::Uuid {
        let reply = self
            .call(
                "keelshell_propose_command",
                json!({"target":target,"command":command}),
                cx,
            )
            .await
            .checked("enqueue suggestion only");
        serde_json::from_value(reply["action_id"].clone()).checked("proposal identity")
    }
}

#[gpui_kit::test]
async fn mcp_starts_disabled_and_human_review_sends_exact_command_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    assert_eq!(
        h.call("keelshell_list_sessions", json!({}), cx).await,
        Err(McpFailure::Disabled)
    );
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
    let command = "printf '中文'\nprintf 'two'";
    let id = h.propose(target, command, cx).await;
    assert!(h.command.requests().is_empty());
    let reply = h
        .call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":id}),
            cx,
        )
        .await
        .checked("pending status");
    assert_eq!(reply["state"], "pending_review");
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("mcp-execute-{id}"), cx);
    })
    .checked("native explicit execution");
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        matches!(
            h.fixture.workspace.read(cx).mcp_test_state(id),
            Some(ActionState::Succeeded)
        )
    })
    .await;
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(id, true, cx));
    assert_eq!(h.command.requests(), vec![command.as_bytes().to_vec()]);
    assert!(
        h.panes.iter().all(|pane| writes(pane).is_empty()),
        "exec never writes PTY"
    );
    assert!(
        h.fixture
            .store
            .load()
            .checked("reload temporary grant state")
            .connections
            .is_empty()
    );
    assert_eq!(
        h.call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":id}),
            cx
        )
        .await
        .checked("finished status")["state"],
        "succeeded"
    );
}

#[gpui_kit::test]
async fn mcp_authenticated_stdio_reaches_desktop_human_review_and_real_ssh(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
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
    let mut client = StdioClient::new(&h, cx);
    let init = client.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"owned-gpui-stdio-fixture","version":"1.0"}}}), &h, cx).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    h.runtime.block_on(async {
        let mut stream = client.stream.lock().await;
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .checked("notify initialized");
        stream
            .flush()
            .await
            .checked("flush initialize notification");
    });
    let sessions = client
        .call(2, "keelshell_list_sessions", json!({}), &h, cx)
        .await;
    let target = sessions["sessions"][0]["target"].clone();
    let command = "printf 'bridge 中文'\nprintf 'two'";
    let pending = client
        .call(
            3,
            "keelshell_propose_command",
            json!({"target":target,"command":command}),
            &h,
            cx,
        )
        .await;
    let id: uuid::Uuid =
        serde_json::from_value(pending["action_id"].clone()).checked("desktop proposal ID");
    assert!(h.command.requests().is_empty());
    assert_eq!(
        client
            .call(
                4,
                "keelshell_get_action_status",
                json!({"target":target,"action_id":id}),
                &h,
                cx
            )
            .await["state"],
        "pending_review"
    );
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("mcp-execute-{id}"), cx);
    })
    .checked("human consumes precise IPC proposal");
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        matches!(
            h.fixture.workspace.read(cx).mcp_test_state(id),
            Some(ActionState::Succeeded)
        )
    })
    .await;
    assert_eq!(
        client
            .call(
                5,
                "keelshell_get_action_status",
                json!({"target":target,"action_id":id}),
                &h,
                cx
            )
            .await["state"],
        "succeeded"
    );
    assert_eq!(h.command.requests(), vec![command.as_bytes().to_vec()]);
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
    h.runtime.block_on(async {
        client
            .stream
            .lock()
            .await
            .shutdown()
            .await
            .checked("stdio EOF");
    });
    cx.wait_for(h.fixture.window, Duration::from_secs(3), |_, _| {
        client.bridge.is_finished()
    })
    .await;
    assert!(
        h.runtime
            .block_on(&mut client.bridge)
            .checked("bridge exit")
            .is_ok()
    );
}

#[gpui_kit::test]
async fn mcp_rotation_denies_held_exec_and_a_success_queued_before_lease_loss(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
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
    let held = h.propose(target, "hold", cx).await;
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("mcp-execute-{held}"), cx);
    })
    .checked("begin owned pending SSH exec");
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, _| {
        !h.command.requests().is_empty()
    })
    .await;
    h.grant(1, &[ToolKind::ListSessions], "", cx).await;
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        matches!(
            h.fixture.workspace.read(cx).mcp_test_state(held),
            Some(ActionState::OutcomeUnknown)
        )
    })
    .await;
    assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
    assert_eq!(
        h.call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":held}),
            cx
        )
        .await,
        Err(McpFailure::Forbidden)
    );
    let raced = h.propose(target, "fixture-queued-success", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        // Model a completed worker queued just before policy replacement, in
        // one UI turn; this phase performs no remote execution.
        view.mcp_test_late_success(raced, cx);
        view.maintain_mcp(cx);
        assert!(matches!(
            view.mcp_test_state(raced),
            Some(ActionState::OutcomeUnknown)
        ));
        assert_eq!(view.mcp_test_output(raced), Some(""));
    });
    assert_eq!(h.command.requests(), vec![b"hold".to_vec()]);
}

#[gpui_kit::test]
async fn mcp_long_multiple_proposals_scroll_to_review_and_execute_in_both_languages(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
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
    let command = format!(
        "printf '审阅开始'\n{}printf '最后一行'",
        "printf '#fixture-line'\n".repeat(1400)
    );
    assert!(command.len() <= 32 * 1024 && command.len() > 30 * 1024);
    let mut ids = Vec::new();
    for _ in 0..4 {
        ids.push(h.propose(target, &command, cx).await);
    }
    let last = ids[3];
    for language in [Language::ZhCn, Language::En] {
        cx.simulate_window_resize(h.fixture.window, size(px(900.), px(580.)));
        cx.update_window(h.fixture.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            let scroll = window.find("mcp-scroll").bounds();
            let preview = window.find(format!("mcp-command-preview-{last}")).bounds();
            assert!(
                preview.size.height >= px(120.),
                "long precise command preview was squeezed: {preview:?}"
            );
            let button = window.find(format!("mcp-execute-{last}")).bounds();
            window.scroll(
                "mcp-scroll",
                ScrollDelta::Pixels(point(px(0.), scroll.origin.y + px(8.) - button.bottom())),
                cx,
            );
            window.render_frame(cx);
            let button = window.find(format!("mcp-execute-{last}"));
            let bounds = button.bounds();
            assert!(button.visible());
            assert!(
                bounds.origin.y >= scroll.origin.y && bounds.bottom() <= scroll.bottom(),
                "review action unreachable after wheel: {bounds:?}, scroll {scroll:?}"
            );
            assert!(window.find("mcp-grant-session").visible());
            assert!(window.find("mcp-disable").visible());
        })
        .checked("minimum bilingual long proposal scrolling");
    }
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.click(format!("mcp-execute-{last}"), cx);
    })
    .checked("execute only the reached exact long proposal");
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        matches!(
            h.fixture.workspace.read(cx).mcp_test_state(last),
            Some(ActionState::Succeeded)
        )
    })
    .await;
    assert_eq!(h.command.requests(), vec![command.as_bytes().to_vec()]);
    for id in &ids[..3] {
        assert!(matches!(
            h.fixture
                .workspace
                .read_with(cx, |view, _| view.mcp_test_state(*id)),
            Some(ActionState::PendingReview)
        ));
    }
}

#[gpui_kit::test]
async fn mcp_sftp_reads_only_granted_regular_complete_utf8_and_releases_subsystems(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.grant(
        1,
        &[
            ToolKind::ListSessions,
            ToolKind::SftpList,
            ToolKind::SftpRead,
        ],
        "/approved",
        cx,
    )
    .await;
    let target = h.target(cx).await;
    let list = h
        .call(
            "keelshell_sftp_list",
            json!({"target":target,"path":"/approved"}),
            cx,
        )
        .await
        .checked("actual granted directory");
    assert_eq!(list["entries"].as_array().map(Vec::len), Some(4));
    let file = h
        .call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/中文.txt","max_bytes":128}),
            cx,
        )
        .await
        .checked("complete actual UTF8 read");
    assert_eq!(file["text"], "受控中文\n");
    assert_eq!(
        h.call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/outside","max_bytes":128}),
            cx
        )
        .await,
        Err(McpFailure::Forbidden)
    );
    assert_eq!(
        h.call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/binary","max_bytes":128}),
            cx
        )
        .await,
        Err(McpFailure::InvalidArgument)
    );
    assert_eq!(
        h.call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/large","max_bytes":128}),
            cx
        )
        .await,
        Err(McpFailure::OutputLimit)
    );
    assert!(
        h.call(
            "keelshell_sftp_read",
            json!({"target":target,"path":"/approved/link","max_bytes":128}),
            cx
        )
        .await
        .is_err()
    );
    cx.wait_for(h.fixture.window, Duration::from_secs(5), |_, _| {
        h.files.active.load(std::sync::atomic::Ordering::Acquire) == 0
    })
    .await;
    assert_eq!(h.files.filesystem.active_directory_handles(), 0);
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
}

#[gpui_kit::test]
async fn mcp_expiry_rejection_and_closed_entity_prevent_execution_and_old_status(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
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
    let rejected = h.propose(target, "reject", cx).await;
    h.fixture
        .workspace
        .update(cx, |view, cx| view.review_mcp_action(rejected, false, cx));
    let expired = h.propose(target, "expired", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.mcp_test_expire(expired);
        view.review_mcp_action(expired, true, cx);
    });
    assert!(h.command.requests().is_empty());
    assert_eq!(
        h.call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":expired}),
            cx
        )
        .await
        .checked("expired state")["state"],
        "expired"
    );
    let pending = h.propose(target, "stale", cx).await;
    h.fixture.workspace.update(cx, |view, cx| {
        view.tabs.remove(0);
        view.maintain_mcp(cx);
        view.review_mcp_action(pending, true, cx);
    });
    assert_eq!(
        h.call(
            "keelshell_get_action_status",
            json!({"target":target,"action_id":pending}),
            cx
        )
        .await,
        Err(McpFailure::Disabled)
    );
    assert!(h.command.requests().is_empty());
}

#[gpui_kit::test]
fn mcp_modal_bilingual_fixed_actions_fit_and_cannot_open_other_workflows(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    for language in [Language::ZhCn, Language::En] {
        for (width, height) in [(900., 580.), (1440., 900.)] {
            cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
            cx.update_window(h.fixture.window, |_, window, cx| {
                i18n::set_language(language, cx);
                h.fixture.workspace.update(cx, |view, cx| {
                    view.mcp.show = false;
                    view.open_mcp(window, cx);
                    view.open_connections(&super::super::OpenConnections, window, cx);
                    view.open_batch_commands(false, window, cx);
                    assert!(view.mcp.show && !view.show_connections && !view.show_batch);
                });
                window.render_frame(cx);
                for id in [
                    "mcp-close",
                    "mcp-disable",
                    "mcp-copy-launch",
                    "mcp-grant-session",
                ] {
                    let bounds = window.find(id).bounds();
                    assert!(window.find(id).visible());
                    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                    assert!(
                        bounds.right() <= window.bounds().right()
                            && bounds.bottom() <= window.bounds().bottom()
                    );
                }
            })
            .checked("responsive bilingual MCP surface");
        }
    }
}

#[gpui_kit::test]
async fn mcp_reads_only_explicit_selected_snapshot_and_replacing_grant_invalidates_old_ids(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.panes[0].terminal.update(cx, |terminal, _| {
        terminal.emulator.start_selection(0, 0);
        terminal.emulator.update_selection(0, 3);
    });
    h.grant(
        0,
        &[ToolKind::ListSessions, ToolKind::ReadSelection],
        "",
        cx,
    )
    .await;
    let metadata = h
        .call("keelshell_list_sessions", json!({}), cx)
        .await
        .checked("discover authorized fragment id");
    let target: SessionIdentity = serde_json::from_value(metadata["sessions"][0]["target"].clone())
        .checked("selection target");
    let selection: uuid::Uuid =
        serde_json::from_value(metadata["sessions"][0]["selection_ids"][0].clone())
            .checked("granted selection id");
    let selected = h
        .call(
            "keelshell_read_selection",
            json!({"target":target,"selection_id":selection}),
            cx,
        )
        .await
        .checked("explicit fragment read");
    assert_eq!(selected["text"], "left");
    assert_eq!(
        h.call(
            "keelshell_read_selection",
            json!({"target":target,"selection_id":uuid::Uuid::new_v4()}),
            cx
        )
        .await,
        Err(McpFailure::Forbidden)
    );
    h.panes[0].terminal.update(cx, |terminal, _| {
        terminal.emulator.feed(b"new output never shared")
    });
    assert_eq!(
        h.call(
            "keelshell_read_selection",
            json!({"target":target,"selection_id":selection}),
            cx
        )
        .await
        .checked("immutable selected snapshot")["text"],
        "left"
    );
    cx.update_window(h.fixture.window, |_, window, cx| {
        h.fixture.workspace.update(cx, |view, cx| {
            view.mcp.show = false;
            view.open_mcp(window, cx);
        });
        window.render_frame(cx);
        window.click("mcp-grant-session", cx);
    })
    .checked("explicitly replace current grant");
    cx.wait_for(h.fixture.window, Duration::from_secs(7), |_, cx| {
        h.fixture
            .workspace
            .update(cx, |view, cx| view.maintain_mcp(cx));
        h.fixture.workspace.read(cx).mcp_test_enabled()
    })
    .await;
    assert_eq!(
        h.call(
            "keelshell_read_selection",
            json!({"target":target,"selection_id":selection}),
            cx
        )
        .await,
        Err(McpFailure::Forbidden)
    );
    assert!(h.command.requests().is_empty());
    assert!(h.panes.iter().all(|pane| writes(pane).is_empty()));
}
