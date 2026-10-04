use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{AuthorizationLease, SessionIdentity, ToolKind};

/// Stable, bounded errors that never include backend errors, paths, commands,
/// credentials, request bodies, or SSH output in diagnostic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpFailure {
    /// Desktop MCP access is off.
    #[error("MCP access is disabled in the desktop application")]
    Disabled,
    /// The current explicit grant does not permit the requested capability.
    #[error("the request is outside the current desktop grant")]
    Forbidden,
    /// No authenticated desktop bridge or connected session exists.
    #[error("the desktop bridge or SSH session is not connected")]
    NotConnected,
    /// A reconnect or route edit invalidated the captured target.
    #[error("the captured SSH session or route is stale")]
    StaleSession,
    /// A desktop authority change invalidated an admitted request.
    #[error("desktop authority was revoked or replaced")]
    Revoked,
    /// The client cancelled the request. This is not remote rollback evidence.
    #[error("the request was cancelled")]
    Cancelled,
    /// The local backend deadline elapsed; remote effects may be unknown.
    #[error("the backend deadline elapsed")]
    Timeout,
    /// The bounded backend concurrency limit is full.
    #[error("the server is at its request concurrency limit")]
    Busy,
    /// Arguments are malformed, unsupported, or exceed a fixed input bound.
    #[error("invalid or unsupported tool arguments")]
    InvalidArgument,
    /// Output exceeded the server's fixed bound.
    #[error("backend output exceeded the permitted bound")]
    OutputLimit,
    /// A backend violated its contract or encountered a private error.
    #[error("the desktop backend could not complete the request")]
    BackendFailure,
}

/// Validated client operation. No variant approves, executes a command, writes a
/// file, connects SSH, installs a host key, or unlocks a credential store.
#[derive(Debug, Clone)]
pub enum Operation {
    /// Enumerate explicitly granted live sessions.
    ListSessions,
    /// Read a desktop-selected fragment of at most 16 KiB.
    ReadSelection {
        /// Exact captured target.
        target: SessionIdentity,
        /// Explicitly selected fragment, with its own desktop identity.
        selection_id: Uuid,
    },
    /// List up to 256 entries from a granted canonical remote directory.
    SftpList {
        /// Exact captured target.
        target: SessionIdentity,
        /// Canonical absolute remote POSIX path.
        path: String,
    },
    /// Read a regular UTF-8 remote file with a client bound of 1–64 KiB.
    SftpRead {
        /// Exact captured target.
        target: SessionIdentity,
        /// Canonical absolute remote POSIX path.
        path: String,
        /// Maximum complete content size; overflow fails, never silently truncates.
        max_bytes: usize,
    },
    /// Read a fixed cached desktop monitoring snapshot, without shell input.
    MonitorSnapshot {
        /// Exact captured target.
        target: SessionIdentity,
    },
    /// Enqueue an immutable command for separate desktop human review.
    ProposeCommand {
        /// Exact captured target.
        target: SessionIdentity,
        /// Exact command to show in desktop review, bounded to 32 KiB.
        command: String,
    },
    /// Inspect one proposal belonging to this exact session and authority.
    GetActionStatus {
        /// Exact captured target.
        target: SessionIdentity,
        /// Desktop proposal identity.
        action_id: Uuid,
    },
}

impl Operation {
    /// Stable capability needed to admit the operation.
    pub fn kind(&self) -> ToolKind {
        match self {
            Self::ListSessions => ToolKind::ListSessions,
            Self::ReadSelection { .. } => ToolKind::ReadSelection,
            Self::SftpList { .. } => ToolKind::SftpList,
            Self::SftpRead { .. } => ToolKind::SftpRead,
            Self::MonitorSnapshot { .. } => ToolKind::MonitorSnapshot,
            Self::ProposeCommand { .. } => ToolKind::ProposeCommand,
            Self::GetActionStatus { .. } => ToolKind::GetActionStatus,
        }
    }

    /// Exact target; only session enumeration is global within the grant.
    pub fn identity(&self) -> Option<SessionIdentity> {
        match self {
            Self::ListSessions => None,
            Self::ReadSelection { target, .. }
            | Self::SftpList { target, .. }
            | Self::SftpRead { target, .. }
            | Self::MonitorSnapshot { target }
            | Self::ProposeCommand { target, .. }
            | Self::GetActionStatus { target, .. } => Some(*target),
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::SftpList { path, .. } | Self::SftpRead { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// Server-created immutable suggestion. The desktop must bind its eventual
/// human approval to this ID, exact target and digest; approval is single-use,
/// expires within five minutes, and is revalidated against live authority.
#[derive(Debug, Clone, Serialize)]
pub struct CommandProposal {
    /// Server-generated identity; the client cannot choose or reuse it.
    pub id: Uuid,
    /// Captured session and reviewed route identity.
    pub target: SessionIdentity,
    /// Exact command that a human must inspect.
    pub command: String,
    /// SHA-256 binding of version, proposal ID, target, and exact UTF-8 command.
    pub digest: String,
    /// Maximum desktop review validity from enqueue, in seconds.
    pub expires_after_seconds: u32,
}

impl CommandProposal {
    pub(crate) fn new(target: SessionIdentity, command: String) -> Self {
        let id = Uuid::new_v4();
        let mut hash = Sha256::new();
        hash.update(b"keelshell-mcp-command-v1\0");
        for part in [
            id,
            target.connection_id,
            target.session_id,
            target.route_revision,
        ] {
            hash.update(part.as_bytes());
        }
        hash.update((command.len() as u64).to_be_bytes());
        hash.update(command.as_bytes());
        let digest = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Self {
            id,
            target,
            command,
            digest,
            expires_after_seconds: 300,
        }
    }
}

/// One admitted operation and its revocable authority. Desktop bridge handlers
/// must atomically capture/compare the live SSH handle against `operation`'s
/// full identity, and recheck the lease before work and after awaits. Never map
/// an old ID onto a reconnected handle. SFTP reads require canonical path and
/// no-symlink/type/size checks at the real transport boundary.
pub struct AuthorizedRequest {
    /// Validated operation.
    pub operation: Operation,
    /// Current authority; non-serializable and controlled by the desktop.
    pub authorization: AuthorizationLease,
    /// Immutable suggestion for `ProposeCommand`, otherwise absent.
    pub proposal: Option<CommandProposal>,
}

/// Cancellation-safe future owned by one request.
pub type BackendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<BackendReply, McpFailure>> + Send + 'a>>;

/// Desktop bridge contract. An implementation must authenticate local IPC,
/// obtain policy from the running desktop, and repeat authority/live-session
/// checks on the desktop side of every request. It must own and release local
/// work when the returned future is dropped. A dropped request cannot establish
/// that previously admitted remote I/O stopped or that a proposal was removed.
///
/// Without explicit launch capabilities, the standalone executable uses
/// [`DisconnectedBackend`]. With capabilities copied from an active desktop
/// grant, the executable relays stdio through authenticated desktop IPC. The
/// desktop retains SSH handles and authority; there is no implicit SSH login
/// or credential access.
pub trait DesktopBackend: Send + Sync + 'static {
    /// Perform only the typed read or proposal admission represented by the
    /// request. `ProposeCommand` must only enqueue review, never approve/execute.
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_>;
}

/// Production default that exposes no desktop, SSH, or credential capability.
#[derive(Debug, Default)]
pub struct DisconnectedBackend;

impl DesktopBackend for DisconnectedBackend {
    fn dispatch(&self, _request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async { Err(McpFailure::NotConnected) })
    }
}

/// Metadata explicitly consented for sharing, excluding host/user/credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Full immutable target identity.
    pub target: SessionIdentity,
    /// Desktop display label, up to 256 bytes.
    pub display_name: String,
    /// Desktop-selected fragment IDs the user explicitly granted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selection_ids: Vec<Uuid>,
    /// Canonical roots whose visibility was explicitly included in the grant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_roots: Vec<String>,
}

/// An SFTP directory entry, with no recursive or executable behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    /// One basename, at most 255 bytes; separators and control bytes are rejected.
    pub name: String,
    /// Regular file, directory, or unsupported item; links remain unsupported.
    pub kind: EntryKind,
    /// Known length, otherwise absent.
    pub size: Option<u64>,
}

/// Read-only directory type classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Link or special object, never traversed/read by this capability.
    Unsupported,
}

/// Fixed cached monitoring fields; absence means unknown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    /// Unique desktop snapshot identity.
    pub sample_id: Uuid,
    /// Age at sampling, in milliseconds.
    pub age_milliseconds: u64,
    /// Known CPU utilization percentage.
    pub cpu_percent: Option<f64>,
    /// Known memory use.
    pub memory_used_bytes: Option<u64>,
    /// Known total memory.
    pub memory_total_bytes: Option<u64>,
}

/// Desktop-owned proposal status. This server supplies no approval method.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    /// Awaiting explicit desktop human review.
    PendingReview,
    /// Human rejected the proposal.
    Rejected,
    /// Review deadline passed without execution.
    Expired,
    /// Cancelled before execution; does not prove running remote work stopped.
    Cancelled,
    /// The desktop started a separately human-approved operation.
    Running,
    /// Desktop operation completed successfully.
    Succeeded,
    /// Desktop operation failed.
    Failed,
    /// The remote outcome cannot be established.
    OutcomeUnknown,
}

/// Bounded typed backend output. The server rejects mismatched reply variants,
/// stale targets, unauthorized sessions and oversized complete results.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendReply {
    /// Explicitly permitted session metadata.
    Sessions {
        /// At most 32 currently granted session entries.
        sessions: Vec<SessionMetadata>,
    },
    /// Desktop-selected terminal content.
    Selection {
        /// Exact captured target.
        target: SessionIdentity,
        /// Exact selected fragment.
        selection_id: Uuid,
        /// Complete UTF-8 text, at most 16 KiB.
        text: String,
    },
    /// Nonrecursive directory list.
    Directory {
        /// Exact captured target.
        target: SessionIdentity,
        /// The canonical remote directory that was read.
        path: String,
        /// At most 256 entries.
        entries: Vec<DirectoryEntry>,
    },
    /// Complete regular UTF-8 file content.
    File {
        /// Exact captured target.
        target: SessionIdentity,
        /// The canonical remote file that was read.
        path: String,
        /// Complete content within the requested bound.
        text: String,
    },
    /// Fixed monitor sample.
    Monitor {
        /// Exact captured target.
        target: SessionIdentity,
        /// Cached fixed fields.
        snapshot: MonitorSnapshot,
    },
    /// Confirmation that an immutable command was only enqueued for review.
    PendingCommand {
        /// Exact captured target.
        target: SessionIdentity,
        /// ID and digest must match the server-created proposal.
        action_id: Uuid,
        /// Digest binding the desktop review contents.
        digest: String,
    },
    /// Status for a proposal authorized by the same target/grant.
    ActionStatus {
        /// Exact captured target.
        target: SessionIdentity,
        /// Exact requested proposal.
        action_id: Uuid,
        /// Desktop-owned state.
        state: ActionState,
    },
}
