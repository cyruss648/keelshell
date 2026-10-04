use std::{borrow::Cow, sync::Arc, time::Duration};

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CustomRequest, CustomResult,
        ErrorCode, Implementation, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::RequestContext,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    AuthorizationLease, AuthorizedRequest, BackendReply, CommandProposal, DesktopBackend,
    DisconnectedBackend, McpFailure, Operation, PolicyController, SessionIdentity, ToolKind,
};

const MAX_REPLY_BYTES: usize = 256 * 1024;
const MAX_COMMAND_BYTES: usize = 32 * 1024;

/// Cloneable official-SDK MCP service. The production default is disabled and
/// disconnected. Constructing a server never creates SSH sessions or reads a
/// credential store. Only a trusted desktop integration installs authority.
#[derive(Clone)]
pub struct KeelShellMcpServer {
    backend: Arc<dyn DesktopBackend>,
    authority: PolicyController,
    capacity: Arc<Semaphore>,
    timeout: Duration,
    connection_closed: CancellationToken,
}

impl Default for KeelShellMcpServer {
    fn default() -> Self {
        Self {
            backend: Arc::new(DisconnectedBackend),
            authority: PolicyController::default(),
            capacity: Arc::new(Semaphore::new(8)),
            timeout: Duration::from_secs(5),
            connection_closed: CancellationToken::new(),
        }
    }
}

impl KeelShellMcpServer {
    /// Bind a trusted backend and desktop authority. Concurrency is 1–8 and
    /// timeout is 1 ms–30 s. Backends must release owned work on future drop and
    /// repeat lease/live-session/path checks at the authoritative I/O boundary.
    pub fn new(
        backend: Arc<dyn DesktopBackend>,
        authority: PolicyController,
        concurrency: usize,
        timeout: Duration,
    ) -> Result<Self, McpFailure> {
        if !(1..=8).contains(&concurrency)
            || timeout < Duration::from_millis(1)
            || timeout > Duration::from_secs(30)
        {
            return Err(McpFailure::InvalidArgument);
        }
        Ok(Self {
            backend,
            authority,
            capacity: Arc::new(Semaphore::new(concurrency)),
            timeout,
            connection_closed: CancellationToken::new(),
        })
    }

    pub(crate) fn with_connection_lifecycle(mut self, closed: CancellationToken) -> Self {
        self.connection_closed = closed;
        self
    }

    /// Validate and admit a fixed tool without bypassing the same server-side
    /// authority checks used by JSON-RPC. Cancellation never means remote
    /// rollback. This API exists for trusted integration and contract tests.
    pub async fn invoke(
        &self,
        name: &str,
        arguments: Value,
        cancellation: CancellationToken,
    ) -> Result<CallToolResult, McpFailure> {
        let operation = parse_operation(name, arguments)?;
        if cancellation.is_cancelled() {
            return Err(McpFailure::Cancelled);
        }
        if self.connection_closed.is_cancelled() {
            return Err(McpFailure::NotConnected);
        }
        let authorization = self.authority.authorize(&operation)?;
        let _permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| McpFailure::Busy)?;
        let proposal = match &operation {
            Operation::ProposeCommand { target, command } => {
                Some(CommandProposal::new(*target, command.clone()))
            }
            _ => None,
        };
        authorization.check()?;
        let request = AuthorizedRequest {
            operation: operation.clone(),
            authorization: authorization.clone(),
            proposal: proposal.clone(),
        };
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(McpFailure::Cancelled),
            _ = self.connection_closed.cancelled() => Err(McpFailure::NotConnected),
            _ = authorization.revoked() => Err(McpFailure::Revoked),
            reply = tokio::time::timeout(self.timeout, self.backend.dispatch(request)) => reply.map_err(|_| McpFailure::Timeout)?,
        }?;
        authorization.check()?;
        if cancellation.is_cancelled() {
            return Err(McpFailure::Cancelled);
        }
        if self.connection_closed.is_cancelled() {
            return Err(McpFailure::NotConnected);
        }
        validate_reply(&operation, &authorization, proposal.as_ref(), &result)?;
        let value = serde_json::to_value(result).map_err(|_| McpFailure::BackendFailure)?;
        let result = CallToolResult::structured(value);
        if serde_json::to_vec(&result)
            .map_err(|_| McpFailure::BackendFailure)?
            .len()
            > MAX_REPLY_BYTES
        {
            return Err(McpFailure::OutputLimit);
        }
        Ok(result)
    }
}

impl ServerHandler for KeelShellMcpServer {
    async fn on_custom_request(
        &self,
        _request: CustomRequest,
        _context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        // The SDK default echoes the arbitrary client method as its message.
        // Keep unsupported-method errors bounded and independent of input.
        Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "unsupported method",
            None,
        ))
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("keelshell-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("External-agent server only. Access is disabled until the desktop user grants exact live sessions/tools/paths. Commands create pending desktop review proposals only. There is no client approval, SSH login, credential unlock, arbitrary command execution or generic MCP client. Explicit temporary launch configuration connects the stdio adapter to the authenticated desktop authority.")
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![
            ProtocolVersion::V_2026_07_28,
            ProtocolVersion::V_2025_11_25,
            ProtocolVersion::V_2025_06_18,
        ])
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.is_some_and(|request| request.cursor.is_some()) {
            return Err(ErrorData::invalid_params(
                "pagination cursors are not supported",
                None,
            ));
        }
        Ok(ListToolsResult::with_all_items(tool_definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        match self.invoke(&request.name, arguments, context.ct).await {
            Ok(result) => Ok(result.into()),
            Err(McpFailure::InvalidArgument) => Err(ErrorData::invalid_params(
                "invalid or unsupported tool arguments",
                None,
            )),
            Err(error) => Ok(CallToolResult::structured_error(
                json!({"error": {"code": error, "message": error.to_string()}}),
            )
            .into()),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetArgs {
    target: SessionIdentity,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionArgs {
    target: SessionIdentity,
    selection_id: Uuid,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathArgs {
    target: SessionIdentity,
    path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    target: SessionIdentity,
    path: String,
    max_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandArgs {
    target: SessionIdentity,
    command: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusArgs {
    target: SessionIdentity,
    action_id: Uuid,
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, McpFailure> {
    serde_json::from_value(value).map_err(|_| McpFailure::InvalidArgument)
}

fn parse_operation(name: &str, value: Value) -> Result<Operation, McpFailure> {
    let operation = match name {
        "keelshell_list_sessions" => {
            if !value.as_object().is_some_and(|v| v.is_empty()) {
                return Err(McpFailure::InvalidArgument);
            }
            Operation::ListSessions
        }
        "keelshell_read_selection" => {
            let args: SelectionArgs = parse(value)?;
            Operation::ReadSelection {
                target: args.target,
                selection_id: args.selection_id,
            }
        }
        "keelshell_sftp_list" => {
            let args: PathArgs = parse(value)?;
            Operation::SftpList {
                target: args.target,
                path: args.path,
            }
        }
        "keelshell_sftp_read" => {
            let args: ReadArgs = parse(value)?;
            if !(1..=64 * 1024).contains(&args.max_bytes) {
                return Err(McpFailure::InvalidArgument);
            }
            Operation::SftpRead {
                target: args.target,
                path: args.path,
                max_bytes: args.max_bytes,
            }
        }
        "keelshell_monitor_snapshot" => {
            let args: TargetArgs = parse(value)?;
            Operation::MonitorSnapshot {
                target: args.target,
            }
        }
        "keelshell_propose_command" => {
            let args: CommandArgs = parse(value)?;
            if args.command.trim().is_empty()
                || args.command.len() > MAX_COMMAND_BYTES
                || args.command.contains('\0')
            {
                return Err(McpFailure::InvalidArgument);
            }
            Operation::ProposeCommand {
                target: args.target,
                command: args.command,
            }
        }
        "keelshell_get_action_status" => {
            let args: StatusArgs = parse(value)?;
            Operation::GetActionStatus {
                target: args.target,
                action_id: args.action_id,
            }
        }
        _ => return Err(McpFailure::InvalidArgument),
    };
    Ok(operation)
}

fn validate_reply(
    operation: &Operation,
    lease: &AuthorizationLease,
    proposal: Option<&CommandProposal>,
    reply: &BackendReply,
) -> Result<(), McpFailure> {
    match (operation, reply) {
        (Operation::ListSessions, BackendReply::Sessions { sessions }) => {
            if sessions.len() > 32 {
                return Err(McpFailure::OutputLimit);
            }
            let mut seen = std::collections::BTreeSet::new();
            for session in sessions {
                if !lease.permits_list_identity(session.target) || !seen.insert(session.target) {
                    return Err(McpFailure::Forbidden);
                }
                if session.selection_ids.len() > 128 || session.granted_roots.len() > 16 {
                    return Err(McpFailure::OutputLimit);
                }
                if !lease.permits_shared_metadata(
                    session.target,
                    &session.selection_ids,
                    &session.granted_roots,
                ) {
                    return Err(McpFailure::Forbidden);
                }
                if session.display_name.len() > 256 {
                    return Err(McpFailure::OutputLimit);
                }
            }
        }
        (
            Operation::ReadSelection {
                target,
                selection_id,
            },
            BackendReply::Selection {
                target: actual,
                selection_id: selected,
                text,
            },
        ) if target == actual && selection_id == selected => {
            if text.len() > 16 * 1024 {
                return Err(McpFailure::OutputLimit);
            }
        }
        (
            Operation::SftpList { target, path },
            BackendReply::Directory {
                target: actual,
                path: read,
                entries,
            },
        ) if target == actual && path == read => {
            if entries.len() > 256 {
                return Err(McpFailure::OutputLimit);
            }
            for entry in entries {
                if entry.name.len() > 255 {
                    return Err(McpFailure::OutputLimit);
                }
                if entry.name.is_empty()
                    || entry.name == "."
                    || entry.name == ".."
                    || entry.name.contains(['/', '\\'])
                    || entry.name.chars().any(char::is_control)
                {
                    return Err(McpFailure::BackendFailure);
                }
            }
        }
        (
            Operation::SftpRead {
                target,
                path,
                max_bytes,
            },
            BackendReply::File {
                target: actual,
                path: read,
                text,
            },
        ) if target == actual && path == read => {
            if text.len() > *max_bytes {
                return Err(McpFailure::OutputLimit);
            }
        }
        (
            Operation::MonitorSnapshot { target },
            BackendReply::Monitor {
                target: actual,
                snapshot,
            },
        ) if target == actual => {
            if snapshot
                .cpu_percent
                .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
            {
                return Err(McpFailure::BackendFailure);
            }
        }
        (
            Operation::ProposeCommand { target, .. },
            BackendReply::PendingCommand {
                target: actual,
                action_id,
                digest,
            },
        ) if target == actual => {
            if !proposal.is_some_and(|p| p.id == *action_id && p.digest == *digest) {
                return Err(McpFailure::BackendFailure);
            }
        }
        (
            Operation::GetActionStatus { target, action_id },
            BackendReply::ActionStatus {
                target: actual,
                action_id: action,
                ..
            },
        ) if target == actual && action_id == action => {}
        _ => return Err(McpFailure::BackendFailure),
    }
    Ok(())
}

fn tool_definitions() -> Vec<Tool> {
    let target = json!({"type":"object","additionalProperties":false,"properties":{"connection_id":{"type":"string","format":"uuid"},"session_id":{"type":"string","format":"uuid"},"route_revision":{"type":"string","format":"uuid"}},"required":["connection_id","session_id","route_revision"]});
    let specs = [
        (
            ToolKind::ListSessions,
            "List explicitly granted active session labels and immutable identities; no credentials.",
            json!({}),
            vec![],
        ),
        (
            ToolKind::ReadSelection,
            "Read one explicitly desktop-selected terminal fragment, at most 16 KiB.",
            json!({"target":target,"selection_id":{"type":"string","format":"uuid"}}),
            vec!["target", "selection_id"],
        ),
        (
            ToolKind::SftpList,
            "List up to 256 entries of an allowed canonical remote directory; no recursive traversal.",
            json!({"target":target,"path":{"type":"string","maxLength":4096}}),
            vec!["target", "path"],
        ),
        (
            ToolKind::SftpRead,
            "Read a complete allowed regular UTF-8 remote file; symlinks/special files/overflow fail closed.",
            json!({"target":target,"path":{"type":"string","maxLength":4096},"max_bytes":{"type":"integer","minimum":1,"maximum":65536}}),
            vec!["target", "path", "max_bytes"],
        ),
        (
            ToolKind::MonitorSnapshot,
            "Read fixed cached desktop monitor fields. This tool accepts no shell command.",
            json!({"target":target}),
            vec!["target"],
        ),
        (
            ToolKind::ProposeCommand,
            "Enqueue an immutable command proposal for desktop human review only. It never approves or executes.",
            json!({"target":target,"command":{"type":"string","minLength":1,"maxLength":32768}}),
            vec!["target", "command"],
        ),
        (
            ToolKind::GetActionStatus,
            "Read the desktop-owned state of a proposal for this exact target. No approval capability.",
            json!({"target":target,"action_id":{"type":"string","format":"uuid"}}),
            vec!["target", "action_id"],
        ),
    ];
    specs.into_iter().map(|(kind, description, properties, required)| {
        let schema = json!({"type":"object","additionalProperties":false,"properties":properties,"required":required});
        Tool::new(kind.name(), description, schema.as_object().cloned().unwrap_or_default())
            .with_annotations(ToolAnnotations::new().read_only(kind != ToolKind::ProposeCommand).destructive(false).idempotent(kind != ToolKind::ProposeCommand).open_world(false))
    }).collect()
}
