use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{McpFailure, Operation};

/// Exact desktop connection and live transport identity. Reconnects get a new
/// `session_id`; editing a route gets a new `route_revision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionIdentity {
    /// Saved connection identifier, or an ephemeral desktop target identifier.
    pub connection_id: Uuid,
    /// Non-reusable identity of the captured, already connected SSH session.
    pub session_id: Uuid,
    /// Identity of the reviewed route, changed whenever route metadata changes.
    pub route_revision: Uuid,
}

/// Fixed server capabilities; these names confer no authority by themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// Enumerate explicitly granted active sessions, with no credentials.
    ListSessions,
    /// Read a specific desktop-selected terminal fragment.
    ReadSelection,
    /// List a scoped SFTP directory.
    SftpList,
    /// Read a scoped regular UTF-8 SFTP file.
    SftpRead,
    /// Read an existing fixed monitor snapshot; never run arbitrary shell.
    MonitorSnapshot,
    /// Enqueue a suggestion for desktop human review, with no execution.
    ProposeCommand,
    /// Enqueue replacement of one existing UTF-8 file for desktop human review.
    ProposeFileChange,
    /// Inspect a proposal's desktop-owned status.
    GetActionStatus,
}

impl ToolKind {
    /// Stable public MCP tool name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::ListSessions => "keelshell_list_sessions",
            Self::ReadSelection => "keelshell_read_selection",
            Self::SftpList => "keelshell_sftp_list",
            Self::SftpRead => "keelshell_sftp_read",
            Self::MonitorSnapshot => "keelshell_monitor_snapshot",
            Self::ProposeCommand => "keelshell_propose_command",
            Self::ProposeFileChange => "keelshell_propose_file_change",
            Self::GetActionStatus => "keelshell_get_action_status",
        }
    }
}

/// A grant created only by a desktop user's explicit consent.
#[derive(Debug, Clone)]
pub struct SessionGrant {
    identity: SessionIdentity,
    tools: BTreeSet<ToolKind>,
    paths: Vec<String>,
    selections: BTreeSet<Uuid>,
}

impl SessionGrant {
    /// Create a bounded grant. Paths must already be canonical remote POSIX
    /// directories; the backend must additionally reject symlinks and recheck
    /// actual canonical paths before reading. Lexical checks alone are not a
    /// remote filesystem security boundary.
    pub fn new(
        identity: SessionIdentity,
        tools: impl IntoIterator<Item = ToolKind>,
        paths: Vec<String>,
        selections: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, McpFailure> {
        let tools = tools.into_iter().collect();
        let selections: BTreeSet<_> = selections.into_iter().collect();
        if paths.len() > 16 || selections.len() > 128 {
            return Err(McpFailure::InvalidArgument);
        }
        for path in &paths {
            validate_path(path)?;
        }
        Ok(Self {
            identity,
            tools,
            paths,
            selections,
        })
    }

    /// Exact granted session identity.
    pub fn identity(&self) -> SessionIdentity {
        self.identity
    }

    /// Canonical remote roots allowed for file reads and replacement proposals.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

/// Desktop-owned authority snapshot. Disabled with no grants by default.
#[derive(Debug, Clone, Default)]
pub struct AccessPolicy {
    enabled: bool,
    grants: Vec<SessionGrant>,
}

impl AccessPolicy {
    /// Enable scoped access to at most 32 distinct captured sessions. The MCP
    /// client has no method to install or change this policy.
    pub fn enabled(grants: Vec<SessionGrant>) -> Result<Self, McpFailure> {
        let unique: BTreeSet<_> = grants.iter().map(|g| g.identity).collect();
        if grants.len() > 32 || unique.len() != grants.len() {
            return Err(McpFailure::InvalidArgument);
        }
        Ok(Self {
            enabled: true,
            grants,
        })
    }
}

struct Revision {
    policy: Arc<AccessPolicy>,
    cancelled: CancellationToken,
}

/// Shared current authority. Replacing or disabling it revokes all existing
/// leases before the new revision becomes visible, including in-flight reads.
#[derive(Clone)]
pub struct PolicyController {
    current: Arc<Mutex<Revision>>,
}

impl Default for PolicyController {
    fn default() -> Self {
        Self {
            current: Arc::new(Mutex::new(Revision {
                policy: Arc::new(AccessPolicy::default()),
                cancelled: CancellationToken::new(),
            })),
        }
    }
}

impl PolicyController {
    /// Install explicit desktop consent, invalidating earlier work. A poisoned
    /// authority lock fails closed; it cannot be recovered by an MCP request.
    pub fn replace(&self, policy: AccessPolicy) -> Result<(), McpFailure> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| McpFailure::BackendFailure)?;
        current.cancelled.cancel();
        *current = Revision {
            policy: Arc::new(policy),
            cancelled: CancellationToken::new(),
        };
        Ok(())
    }

    /// Revoke every current grant and turn access off.
    pub fn disable(&self) -> Result<(), McpFailure> {
        self.replace(AccessPolicy::default())
    }

    pub(crate) fn authorize(
        &self,
        operation: &Operation,
    ) -> Result<AuthorizationLease, McpFailure> {
        let current = self
            .current
            .lock()
            .map_err(|_| McpFailure::BackendFailure)?;
        if !current.policy.enabled {
            return Err(McpFailure::Disabled);
        }
        let policy = &current.policy;
        if let Some(identity) = operation.identity() {
            let Some(grant) = policy.grants.iter().find(|g| g.identity == identity) else {
                return Err(
                    if policy
                        .grants
                        .iter()
                        .any(|g| g.identity.connection_id == identity.connection_id)
                    {
                        McpFailure::StaleSession
                    } else {
                        McpFailure::Forbidden
                    },
                );
            };
            if !grant.tools.contains(&operation.kind()) {
                return Err(McpFailure::Forbidden);
            }
            if let Some(path) = operation.path() {
                validate_path(path)?;
                if !grant.paths.iter().any(|root| path_in_root(path, root)) {
                    return Err(McpFailure::Forbidden);
                }
            }
            if let Operation::ReadSelection { selection_id, .. } = operation
                && !grant.selections.contains(selection_id)
            {
                return Err(McpFailure::Forbidden);
            }
        } else if !policy
            .grants
            .iter()
            .any(|g| g.tools.contains(&ToolKind::ListSessions))
        {
            return Err(McpFailure::Forbidden);
        }
        Ok(AuthorizationLease {
            policy: policy.clone(),
            cancelled: current.cancelled.clone(),
        })
    }
}

/// Non-serializable authorization for one request. Backends must check it before
/// admission, before each remote read or proposal enqueue, and after awaits.
/// Cancellation stops admission and future local work; it cannot undo already
/// submitted remote I/O or an already enqueued proposal.
#[derive(Clone)]
pub struct AuthorizationLease {
    policy: Arc<AccessPolicy>,
    cancelled: CancellationToken,
}

impl AuthorizationLease {
    /// Fail if desktop authority has been revoked or replaced.
    pub fn check(&self) -> Result<(), McpFailure> {
        if self.cancelled.is_cancelled() {
            Err(McpFailure::Revoked)
        } else {
            Ok(())
        }
    }

    /// Wait until the desktop invalidates this request's authority.
    pub async fn revoked(&self) {
        self.cancelled.cancelled().await;
    }

    /// Granted sessions for this request, including their canonical path roots.
    /// Call `check()` again before using this snapshot after any await.
    pub fn grants(&self) -> &[SessionGrant] {
        &self.policy.grants
    }

    pub(crate) fn permits_shared_metadata(
        &self,
        identity: SessionIdentity,
        selections: &[Uuid],
        paths: &[String],
    ) -> bool {
        self.policy
            .grants
            .iter()
            .find(|grant| grant.identity == identity)
            .is_some_and(|grant| {
                (selections.is_empty() || grant.tools.contains(&ToolKind::ReadSelection))
                    && selections.iter().all(|id| grant.selections.contains(id))
                    && (paths.is_empty()
                        || grant.tools.contains(&ToolKind::SftpList)
                        || grant.tools.contains(&ToolKind::SftpRead)
                        || grant.tools.contains(&ToolKind::ProposeFileChange))
                    && paths.iter().all(|path| grant.paths.contains(path))
            })
    }

    pub(crate) fn permits_list_identity(&self, identity: SessionIdentity) -> bool {
        self.policy
            .grants
            .iter()
            .any(|g| g.identity == identity && g.tools.contains(&ToolKind::ListSessions))
    }
}

pub(crate) fn validate_path(path: &str) -> Result<(), McpFailure> {
    if !path.starts_with('/')
        || path.len() > 4096
        || path.contains(['\\', '\0'])
        || path.chars().any(char::is_control)
        || (path != "/" && path.ends_with('/'))
        || path
            .split('/')
            .skip(1)
            .any(|part| part == "." || part == ".." || (part.is_empty() && path != "/"))
    {
        return Err(McpFailure::InvalidArgument);
    }
    Ok(())
}

fn path_in_root(path: &str, root: &str) -> bool {
    root == "/"
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_paths_use_complete_components_and_unicode_without_normalization() {
        assert!(path_in_root("/批准/file", "/批准"));
        assert!(path_in_root("/批准", "/批准"));
        assert!(!path_in_root("/批准-other/file", "/批准"));
        assert!(!path_in_root("/Private/file", "/private"));
        assert!(path_in_root("/any", "/"));
    }

    #[test]
    fn absolute_path_grammar_refuses_aliases_and_control_characters() {
        for path in ["/", "/批准/file", "/directory with spaces/it's.txt"] {
            assert_eq!(validate_path(path), Ok(()));
        }
        for path in [
            "", "relative", "//host", "/x//y", "/x/./y", "/x/../y", "/x/", "/x\0y", "/x\ry",
            "/x\\y",
        ] {
            assert_eq!(validate_path(path), Err(McpFailure::InvalidArgument));
        }
    }
}
