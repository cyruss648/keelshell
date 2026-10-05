#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use keelshell_mcp::*;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn target() -> SessionIdentity {
    SessionIdentity {
        connection_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
        route_revision: Uuid::from_u128(3),
    }
}
fn selection() -> Uuid {
    Uuid::from_u128(4)
}
fn tools() -> [ToolKind; 8] {
    [
        ToolKind::ListSessions,
        ToolKind::ReadSelection,
        ToolKind::SftpList,
        ToolKind::SftpRead,
        ToolKind::MonitorSnapshot,
        ToolKind::ProposeCommand,
        ToolKind::ProposeFileChange,
        ToolKind::GetActionStatus,
    ]
}
fn enabled(allowed: &[ToolKind]) -> PolicyController {
    let controller = PolicyController::default();
    let grant = SessionGrant::new(
        target(),
        allowed.iter().copied(),
        vec!["/approved".into()],
        [selection()],
    )
    .unwrap();
    controller
        .replace(AccessPolicy::enabled(vec![grant]).unwrap())
        .unwrap();
    controller
}

#[derive(Default)]
struct Backend {
    calls: AtomicUsize,
    proposals: Mutex<Vec<CommandProposal>>,
    replacement: Mutex<Option<BackendReply>>,
}
impl DesktopBackend for Backend {
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            request.authorization.check()?;
            if let Some(reply) = self.replacement.lock().unwrap().clone() {
                return Ok(reply);
            }
            let reply = match request.operation {
                Operation::ListSessions => BackendReply::Sessions {
                    sessions: vec![SessionMetadata {
                        target: target(),
                        display_name: "隔离SSH".into(),
                        selection_ids: Vec::new(),
                        granted_roots: Vec::new(),
                    }],
                },
                Operation::ReadSelection {
                    target,
                    selection_id,
                } => BackendReply::Selection {
                    target,
                    selection_id,
                    text: "手动选择".into(),
                },
                Operation::SftpList { target, path } => BackendReply::Directory {
                    target,
                    path,
                    entries: vec![DirectoryEntry {
                        name: "file.txt".into(),
                        kind: EntryKind::File,
                        size: Some(3),
                    }],
                },
                Operation::SftpRead { target, path, .. } => BackendReply::File {
                    target,
                    path,
                    text: "读".into(),
                    sha256: content_sha256("读"),
                },
                Operation::MonitorSnapshot { target } => BackendReply::Monitor {
                    target,
                    snapshot: MonitorSnapshot {
                        sample_id: Uuid::from_u128(5),
                        age_milliseconds: 100,
                        cpu_percent: Some(5.0),
                        memory_used_bytes: Some(100),
                        memory_total_bytes: Some(1000),
                    },
                },
                Operation::ProposeCommand { target, .. } => {
                    let proposal = request.proposal.unwrap();
                    let reply = BackendReply::PendingCommand {
                        target,
                        action_id: proposal.id,
                        digest: proposal.digest.clone(),
                    };
                    self.proposals.lock().unwrap().push(proposal);
                    reply
                }
                Operation::ProposeFileChange { target, .. } => {
                    let proposal = request.file_proposal.unwrap();
                    BackendReply::PendingFileChange {
                        target,
                        action_id: proposal.id,
                        digest: proposal.digest,
                    }
                }
                Operation::GetActionStatus { target, action_id } => BackendReply::ActionStatus {
                    target,
                    action_id,
                    state: ActionState::PendingReview,
                    action_kind: ActionKind::Command,
                },
            };
            request.authorization.check()?;
            Ok(reply)
        })
    }
}
fn make_server(backend: Arc<dyn DesktopBackend>, policy: PolicyController) -> KeelShellMcpServer {
    KeelShellMcpServer::new(backend, policy, 8, Duration::from_secs(1)).unwrap()
}
async fn invoke(
    server: &KeelShellMcpServer,
    kind: ToolKind,
    args: Value,
) -> Result<Value, McpFailure> {
    Ok(server
        .invoke(kind.name(), args, CancellationToken::new())
        .await?
        .structured_content
        .unwrap())
}

#[tokio::test]
async fn disabled_default_never_dispatches_or_grants_itself() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), PolicyController::default());
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Disabled
    );
    assert_eq!(
        server
            .invoke("keelshell_enable", json!({}), CancellationToken::new())
            .await
            .unwrap_err(),
        McpFailure::InvalidArgument
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn explicit_grant_still_requires_connected_desktop_backend() {
    let server = make_server(Arc::new(DisconnectedBackend), enabled(&tools()));
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::NotConnected
    );
}

#[tokio::test]
async fn fixed_reads_accept_exact_scope_and_preserve_utf8() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap()["sessions"][0]["display_name"],
        "隔离SSH"
    );
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":target(),"selection_id":selection()})
        )
        .await
        .unwrap()["text"],
        "手动选择"
    );
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpList,
            json!({"target":target(),"path":"/approved"})
        )
        .await
        .unwrap()["entries"][0]["name"],
        "file.txt"
    );
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpRead,
            json!({"target":target(),"path":"/approved/file.txt","max_bytes":3})
        )
        .await
        .unwrap()["text"],
        "读"
    );
    assert_eq!(
        invoke(
            &server,
            ToolKind::MonitorSnapshot,
            json!({"target":target()})
        )
        .await
        .unwrap()["snapshot"]["cpu_percent"],
        5.0
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn tool_connection_selection_and_reconnected_identity_are_separate_boundaries() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&[ToolKind::ReadSelection]));
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":target(),"selection_id":Uuid::new_v4()})
        )
        .await
        .unwrap_err(),
        McpFailure::Forbidden
    );
    let mut stale = target();
    stale.session_id = Uuid::new_v4();
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":stale,"selection_id":selection()})
        )
        .await
        .unwrap_err(),
        McpFailure::StaleSession
    );
    stale = target();
    stale.route_revision = Uuid::new_v4();
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":stale,"selection_id":selection()})
        )
        .await
        .unwrap_err(),
        McpFailure::StaleSession
    );
    stale.connection_id = Uuid::new_v4();
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":stale,"selection_id":selection()})
        )
        .await
        .unwrap_err(),
        McpFailure::Forbidden
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn path_component_and_traversal_checks_fail_before_backend() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    for path in [
        "/approved/../private",
        "relative",
        "/approved/./file",
        "/approved//file",
        "/approved/",
        "/approved\\file",
        "/approved/\nfile",
    ] {
        assert_eq!(
            invoke(
                &server,
                ToolKind::SftpList,
                json!({"target":target(),"path":path})
            )
            .await
            .unwrap_err(),
            McpFailure::InvalidArgument,
            "{path:?}"
        );
    }
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpList,
            json!({"target":target(),"path":"/approved-sibling"})
        )
        .await
        .unwrap_err(),
        McpFailure::Forbidden
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unrecognized_arguments_and_action_methods_never_admit_work() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    for name in [
        "keelshell_approve_command",
        "keelshell_exec",
        "keelshell_unlock_vault",
        "keelshell_sftp_write",
        "keelshell_connect",
        "keelshell_mcp_connect",
    ] {
        assert_eq!(
            server
                .invoke(name, json!({"target":target()}), CancellationToken::new())
                .await
                .unwrap_err(),
            McpFailure::InvalidArgument
        );
    }
    for args in [
        json!({"target":target(),"path":"/approved/file","max_bytes":0}),
        json!({"target":target(),"path":"/approved/file","max_bytes":65537}),
        json!({"target":target(),"path":"/approved/file","max_bytes":3,"approve":true}),
        json!({"target":target(),"path":"/approved/file","max_bytes":-1}),
    ] {
        assert_eq!(
            invoke(&server, ToolKind::SftpRead, args).await.unwrap_err(),
            McpFailure::InvalidArgument
        );
    }
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({"enable":true}))
            .await
            .unwrap_err(),
        McpFailure::InvalidArgument
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn command_proposals_are_unique_bound_and_only_pending_review() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    let command = "printf '人工审核\\n'";
    let first = invoke(
        &server,
        ToolKind::ProposeCommand,
        json!({"target":target(),"command":command}),
    )
    .await
    .unwrap();
    let second = invoke(
        &server,
        ToolKind::ProposeCommand,
        json!({"target":target(),"command":command}),
    )
    .await
    .unwrap();
    assert_ne!(first["action_id"], second["action_id"]);
    assert_ne!(first["digest"], second["digest"]);
    let proposals = backend.proposals.lock().unwrap();
    assert_eq!(proposals.len(), 2);
    assert_eq!(proposals[0].command, command);
    assert_eq!(proposals[0].target, target());
    assert_eq!(proposals[0].expires_after_seconds, 300);
    assert_eq!(proposals[0].digest.len(), 64);
    assert_eq!(first["kind"], "pending_command");
}

#[tokio::test]
async fn malformed_empty_large_nul_commands_never_enqueue() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    for command in [" \t".to_string(), "x".repeat(32769), "hidden\0tail".into()] {
        assert_eq!(
            invoke(
                &server,
                ToolKind::ProposeCommand,
                json!({"target":target(),"command":command})
            )
            .await
            .unwrap_err(),
            McpFailure::InvalidArgument
        );
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert!(backend.proposals.lock().unwrap().is_empty());
}

#[tokio::test]
async fn backend_mismatched_identity_selection_and_proposal_are_rejected() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    let mut stale = target();
    stale.session_id = Uuid::new_v4();
    *backend.replacement.lock().unwrap() = Some(BackendReply::File {
        target: stale,
        path: "/approved/file".into(),
        text: "x".into(),
        sha256: content_sha256("x"),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpRead,
            json!({"target":target(),"path":"/approved/file","max_bytes":100})
        )
        .await
        .unwrap_err(),
        McpFailure::BackendFailure
    );
    *backend.replacement.lock().unwrap() = Some(BackendReply::Selection {
        target: target(),
        selection_id: Uuid::new_v4(),
        text: "private".into(),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":target(),"selection_id":selection()})
        )
        .await
        .unwrap_err(),
        McpFailure::BackendFailure
    );
    *backend.replacement.lock().unwrap() = Some(BackendReply::PendingCommand {
        target: target(),
        action_id: Uuid::new_v4(),
        digest: "fake".into(),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::ProposeCommand,
            json!({"target":target(),"command":"pwd"})
        )
        .await
        .unwrap_err(),
        McpFailure::BackendFailure
    );
}

#[tokio::test]
async fn leaked_or_duplicate_sessions_fail_closed() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    let mut other = target();
    other.session_id = Uuid::new_v4();
    *backend.replacement.lock().unwrap() = Some(BackendReply::Sessions {
        sessions: vec![SessionMetadata {
            target: other,
            display_name: "private".into(),
            selection_ids: Vec::new(),
            granted_roots: Vec::new(),
        }],
    });
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
    let entry = SessionMetadata {
        target: target(),
        display_name: "ok".into(),
        selection_ids: Vec::new(),
        granted_roots: Vec::new(),
    };
    *backend.replacement.lock().unwrap() = Some(BackendReply::Sessions {
        sessions: vec![entry.clone(), entry],
    });
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
}

#[tokio::test]
async fn complete_byte_bounds_and_escaped_json_size_are_enforced() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    *backend.replacement.lock().unwrap() = Some(BackendReply::File {
        target: target(),
        path: "/approved/file".into(),
        text: "读".into(),
        sha256: content_sha256("读"),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpRead,
            json!({"target":target(),"path":"/approved/file","max_bytes":2})
        )
        .await
        .unwrap_err(),
        McpFailure::OutputLimit
    );
    *backend.replacement.lock().unwrap() = Some(BackendReply::File {
        target: target(),
        path: "/approved/file".into(),
        text: "\0".repeat(65536),
        sha256: content_sha256(&"\0".repeat(65536)),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpRead,
            json!({"target":target(),"path":"/approved/file","max_bytes":65536})
        )
        .await
        .unwrap_err(),
        McpFailure::OutputLimit
    );
    *backend.replacement.lock().unwrap() = Some(BackendReply::Selection {
        target: target(),
        selection_id: selection(),
        text: "x".repeat(16385),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::ReadSelection,
            json!({"target":target(),"selection_id":selection()})
        )
        .await
        .unwrap_err(),
        McpFailure::OutputLimit
    );
}

struct DropMarker(Arc<AtomicBool>);
impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[derive(Default)]
struct SlowBackend {
    entered: Notify,
    dropped: Arc<AtomicBool>,
}
impl DesktopBackend for SlowBackend {
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async move {
            request.authorization.check()?;
            let _marker = DropMarker(self.dropped.clone());
            self.entered.notify_one();
            std::future::pending().await
        })
    }
}

#[tokio::test]
async fn revocation_drops_inflight_backend_and_blocks_subsequent_admission() {
    let backend = Arc::new(SlowBackend::default());
    let authority = enabled(&tools());
    let server = make_server(backend.clone(), authority.clone());
    let pending =
        tokio::spawn(async move { invoke(&server, ToolKind::ListSessions, json!({})).await });
    tokio::time::timeout(Duration::from_secs(2), backend.entered.notified())
        .await
        .unwrap();
    authority.disable().unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), pending)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        McpFailure::Revoked
    );
    assert!(backend.dropped.load(Ordering::SeqCst));
    let server = make_server(Arc::new(Backend::default()), authority);
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Disabled
    );
}

#[tokio::test]
async fn deadlines_drop_owned_backend_work() {
    let backend = Arc::new(SlowBackend::default());
    let server = KeelShellMcpServer::new(
        backend.clone(),
        enabled(&tools()),
        1,
        Duration::from_millis(20),
    )
    .unwrap();
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Timeout
    );
    assert!(backend.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancellation_and_busy_do_not_expand_admission() {
    let backend = Arc::new(SlowBackend::default());
    let server = KeelShellMcpServer::new(
        backend.clone(),
        enabled(&tools()),
        1,
        Duration::from_secs(2),
    )
    .unwrap();
    let cancellation = CancellationToken::new();
    let s = server.clone();
    let c = cancellation.clone();
    let pending =
        tokio::spawn(async move { s.invoke(ToolKind::ListSessions.name(), json!({}), c).await });
    tokio::time::timeout(Duration::from_secs(1), backend.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Busy
    );
    cancellation.cancel();
    assert_eq!(pending.await.unwrap().unwrap_err(), McpFailure::Cancelled);
    assert!(backend.dropped.load(Ordering::SeqCst));
}

#[test]
fn policy_and_runtime_configuration_reject_unbounded_or_duplicate_grants() {
    assert!(SessionGrant::new(target(), tools(), vec!["/root/..".into()], []).is_err());
    let grant = SessionGrant::new(target(), tools(), vec!["/".into()], []).unwrap();
    assert!(AccessPolicy::enabled(vec![grant.clone(), grant]).is_err());
    for (concurrency, timeout) in [
        (0, Duration::from_secs(1)),
        (9, Duration::from_secs(1)),
        (1, Duration::ZERO),
        (1, Duration::from_secs(31)),
    ] {
        assert!(
            KeelShellMcpServer::new(
                Arc::new(DisconnectedBackend),
                PolicyController::default(),
                concurrency,
                timeout
            )
            .is_err()
        );
    }
}

#[tokio::test]
async fn session_metadata_cannot_disclose_ungranted_selection_ids_or_roots() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    for entry in [
        SessionMetadata {
            target: target(),
            display_name: "ok".into(),
            selection_ids: vec![Uuid::new_v4()],
            granted_roots: vec![],
        },
        SessionMetadata {
            target: target(),
            display_name: "ok".into(),
            selection_ids: vec![],
            granted_roots: vec!["/outside".into()],
        },
    ] {
        *backend.replacement.lock().unwrap() = Some(BackendReply::Sessions {
            sessions: vec![entry],
        });
        assert_eq!(
            invoke(&server, ToolKind::ListSessions, json!({}))
                .await
                .unwrap_err(),
            McpFailure::Forbidden
        );
    }
    let controller = enabled(&[ToolKind::ListSessions]);
    let server = make_server(backend.clone(), controller);
    *backend.replacement.lock().unwrap() = Some(BackendReply::Sessions {
        sessions: vec![SessionMetadata {
            target: target(),
            display_name: "ok".into(),
            selection_ids: vec![selection()],
            granted_roots: vec!["/approved".into()],
        }],
    });
    assert_eq!(
        invoke(&server, ToolKind::ListSessions, json!({}))
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
}

#[tokio::test]
async fn file_proposal_has_separate_permission_scoped_path_and_never_returns_preimage() {
    let backend = Arc::new(Backend::default());
    let args = json!({"target":target(),"path":"/approved/file","expected_sha256":content_sha256("old"),"replacement":"new中文\n"});
    let denied = make_server(
        backend.clone(),
        enabled(&[ToolKind::SftpRead, ToolKind::ProposeCommand]),
    );
    assert_eq!(
        invoke(&denied, ToolKind::ProposeFileChange, args.clone())
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    let server = make_server(backend.clone(), enabled(&[ToolKind::ProposeFileChange]));
    let first = invoke(&server, ToolKind::ProposeFileChange, args.clone())
        .await
        .unwrap();
    let second = invoke(&server, ToolKind::ProposeFileChange, args.clone())
        .await
        .unwrap();
    assert_eq!(first["kind"], "pending_file_change");
    assert_ne!(first["action_id"], second["action_id"]);
    assert_ne!(first["digest"], second["digest"]);
    assert!(first.get("text").is_none() && first.get("sha256").is_none());
    let mut outside = args;
    outside["path"] = json!("/approved-other/file");
    assert_eq!(
        invoke(&server, ToolKind::ProposeFileChange, outside)
            .await
            .unwrap_err(),
        McpFailure::Forbidden
    );
}

#[tokio::test]
async fn file_proposal_rejects_unknown_fields_bad_hashes_and_utf8_byte_overflow() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&[ToolKind::ProposeFileChange]));
    let base = json!({"target":target(),"path":"/approved/file","expected_sha256":content_sha256("old"),"replacement":""});
    for hash in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
        let mut bad = base.clone();
        bad["expected_sha256"] = json!(hash);
        assert_eq!(
            invoke(&server, ToolKind::ProposeFileChange, bad)
                .await
                .unwrap_err(),
            McpFailure::InvalidArgument
        );
    }
    for (key, value) in [("approve", json!(true)), ("old_text", json!("private"))] {
        let mut bad = base.clone();
        bad[key] = value;
        assert_eq!(
            invoke(&server, ToolKind::ProposeFileChange, bad)
                .await
                .unwrap_err(),
            McpFailure::InvalidArgument
        );
    }
    let mut bad = base.clone();
    bad["replacement"] = json!("中".repeat(21846));
    assert_eq!(
        invoke(&server, ToolKind::ProposeFileChange, bad)
            .await
            .unwrap_err(),
        McpFailure::InvalidArgument
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert!(
        invoke(&server, ToolKind::ProposeFileChange, base)
            .await
            .is_ok(),
        "empty replacement is allowed for an existing file"
    );
}

#[tokio::test]
async fn sftp_content_hash_and_file_proposal_reply_binding_are_verified() {
    let backend = Arc::new(Backend::default());
    let server = make_server(backend.clone(), enabled(&tools()));
    *backend.replacement.lock().unwrap() = Some(BackendReply::File {
        target: target(),
        path: "/approved/file".into(),
        text: "old".into(),
        sha256: content_sha256("different"),
    });
    assert_eq!(
        invoke(
            &server,
            ToolKind::SftpRead,
            json!({"target":target(),"path":"/approved/file","max_bytes":128})
        )
        .await
        .unwrap_err(),
        McpFailure::BackendFailure
    );
    *backend.replacement.lock().unwrap() = Some(BackendReply::PendingFileChange {
        target: target(),
        action_id: Uuid::new_v4(),
        digest: "fake".into(),
    });
    assert_eq!(invoke(&server, ToolKind::ProposeFileChange, json!({"target":target(),"path":"/approved/file","expected_sha256":content_sha256("old"),"replacement":"new"})).await.unwrap_err(), McpFailure::BackendFailure);
}
