//! Desktop-owned MCP requests. Authority never travels in client metadata.
use std::sync::Arc;

use keelshell_mcp::{AuthorizedRequest, BackendFuture, BackendReply, DesktopBackend, McpFailure};
use tokio::sync::{mpsc, oneshot};

mod authorization;
pub(crate) use authorization::{
    LifecycleSource, SessionAuthorization, lifecycle_current, lifecycle_lost,
};

/// A request whose caller may have disconnected before the UI admits it.
pub(crate) struct QueuedRequest {
    pub(crate) request: AuthorizedRequest,
    pub(crate) reply: oneshot::Sender<Result<BackendReply, McpFailure>>,
}

/// The network runtime sends only bounded typed work to the UI event loop.
pub(crate) struct QueueBackend {
    sender: mpsc::Sender<QueuedRequest>,
}

impl QueueBackend {
    pub(crate) fn channel() -> (Arc<Self>, mpsc::Receiver<QueuedRequest>) {
        let (sender, receiver) = mpsc::channel(8);
        (Arc::new(Self { sender }), receiver)
    }
}

impl DesktopBackend for QueueBackend {
    fn dispatch(&self, request: AuthorizedRequest) -> BackendFuture<'_> {
        Box::pin(async move {
            request.authorization.check()?;
            let lease = request.authorization.clone();
            let (reply, result) = oneshot::channel();
            self.sender
                .try_send(QueuedRequest { request, reply })
                .map_err(|error| match error {
                    mpsc::error::TrySendError::Full(_) => McpFailure::Busy,
                    mpsc::error::TrySendError::Closed(_) => McpFailure::NotConnected,
                })?;
            tokio::select! {
                biased;
                _ = lease.revoked() => Err(McpFailure::Revoked),
                result = result => {
                    let result = result.map_err(|_| McpFailure::NotConnected)??;
                    lease.check()?;
                    Ok(result)
                }
            }
        })
    }
}

/// Perform a bounded checked SFTP read on the exact captured connection.
/// Dropping this future drops the SFTP subsystem owner. Checks are observations;
/// SFTP v3 cannot exclude a malicious concurrent server-side rename.
pub(crate) async fn read_remote(
    session: keelshell_session::SshSession,
    operation: keelshell_mcp::Operation,
    lease: &SessionAuthorization,
) -> Result<BackendReply, McpFailure> {
    use keelshell_mcp::{DirectoryEntry, EntryKind, Operation};
    lease.check()?;
    if session.is_closed() {
        return Err(McpFailure::StaleSession);
    }
    let sftp = session
        .sftp()
        .await
        .map_err(|_| McpFailure::BackendFailure)?;
    let result = async {
        lease.check()?;
        let (target, path) = match &operation {
            Operation::SftpList { target, path } | Operation::SftpRead { target, path, .. } => {
                (*target, path)
            }
            _ => return Err(McpFailure::InvalidArgument),
        };
        // Canonical equality rejects alias/symlink escapes, then inspect/read
        // verifies every parent and explicit leaf type at the actual transport.
        let canonical = sftp
            .canonicalize(path)
            .await
            .map_err(|_| McpFailure::BackendFailure)?;
        lease.check()?;
        if canonical != *path {
            return Err(McpFailure::Forbidden);
        }
        let entry = sftp
            .inspect_entry(path)
            .await
            .map_err(|_| McpFailure::BackendFailure)?
            .ok_or(McpFailure::BackendFailure)?;
        lease.check()?;
        match &operation {
            Operation::SftpList { .. } => {
                if !entry
                    .permissions
                    .is_some_and(|mode| mode & 0o170000 == 0o040000)
                    || entry.is_symlink
                {
                    return Err(McpFailure::Forbidden);
                }
                let entries = sftp
                    .list_limited(path, 256)
                    .await
                    .map_err(|error| match error {
                        keelshell_session::SessionError::EntryLimit(_) => McpFailure::OutputLimit,
                        _ => McpFailure::BackendFailure,
                    })?
                    .into_iter()
                    .map(|entry| DirectoryEntry {
                        name: entry.name,
                        kind: match entry.permissions.map(|mode| mode & 0o170000) {
                            Some(0o100000) => EntryKind::File,
                            Some(0o040000) => EntryKind::Directory,
                            _ => EntryKind::Unsupported,
                        },
                        size: entry.size,
                    })
                    .collect();
                lease.check()?;
                if sftp
                    .canonicalize(path)
                    .await
                    .map_err(|_| McpFailure::BackendFailure)?
                    != *path
                {
                    return Err(McpFailure::Forbidden);
                }
                Ok(BackendReply::Directory {
                    target,
                    path: path.clone(),
                    entries,
                })
            }
            Operation::SftpRead { max_bytes, .. } => {
                let bytes =
                    sftp.read_regular(path, *max_bytes)
                        .await
                        .map_err(|error| match error {
                            keelshell_session::SessionError::OutputLimit(_) => {
                                McpFailure::OutputLimit
                            }
                            _ => McpFailure::BackendFailure,
                        })?;
                lease.check()?;
                let text = String::from_utf8(bytes).map_err(|_| McpFailure::InvalidArgument)?;
                Ok(BackendReply::File {
                    target,
                    path: path.clone(),
                    sha256: keelshell_mcp::content_sha256(&text),
                    text,
                })
            }
            _ => Err(McpFailure::InvalidArgument),
        }
    }
    .await;
    // Scope cancellation still owns close through the SFTP owner when this
    // await is dropped; a normal return additionally observes close success.
    let closed = sftp.close().await;
    lease.check()?;
    let result = result?;
    closed.map_err(|_| McpFailure::BackendFailure)?;
    Ok(result)
}

pub(crate) async fn validate_root(
    session: keelshell_session::SshSession,
    path: &str,
    lifecycle: &LifecycleSource,
) -> Result<(), McpFailure> {
    if session.is_closed() || !lifecycle_current(lifecycle.as_ref()) {
        return Err(McpFailure::StaleSession);
    }
    let sftp = session
        .sftp()
        .await
        .map_err(|_| McpFailure::BackendFailure)?;
    let result = async {
        if sftp
            .canonicalize(path)
            .await
            .map_err(|_| McpFailure::BackendFailure)?
            != path
        {
            return Err(McpFailure::Forbidden);
        }
        let entry = sftp
            .inspect_entry(path)
            .await
            .map_err(|_| McpFailure::BackendFailure)?
            .ok_or(McpFailure::Forbidden)?;
        if !entry
            .permissions
            .is_some_and(|mode| mode & 0o170000 == 0o040000)
            || entry.is_symlink
        {
            return Err(McpFailure::Forbidden);
        }
        Ok(())
    }
    .await;
    let closed = sftp.close().await;
    if !lifecycle_current(lifecycle.as_ref()) {
        return Err(McpFailure::StaleSession);
    }
    result?;
    closed.map_err(|_| McpFailure::BackendFailure)
}

/// Observe a proposal's complete baseline privately. The caller returns only a
/// pending identity/digest: errors reveal neither old content nor current hash.
pub(crate) async fn prepare_file_change(
    session: keelshell_session::SshSession,
    proposal: &keelshell_mcp::FileChangeProposal,
    lease: &SessionAuthorization,
) -> Result<(keelshell_session::sftp::RegularFileSnapshot, String), McpFailure> {
    lease.check()?;
    if session.is_closed() {
        return Err(McpFailure::StaleSession);
    }
    let sftp = session
        .sftp()
        .await
        .map_err(|_| McpFailure::BackendFailure)?;
    let result = async {
        if sftp
            .canonicalize(&proposal.path)
            .await
            .map_err(|_| McpFailure::BackendFailure)?
            != proposal.path
        {
            return Err(McpFailure::Forbidden);
        }
        lease.check()?;
        let baseline = sftp
            .read_regular_snapshot(&proposal.path, 64 * 1024)
            .await
            .map_err(|error| match error {
                keelshell_session::SessionError::OutputLimit(_) => McpFailure::OutputLimit,
                _ => McpFailure::BackendFailure,
            })?;
        lease.check()?;
        let old =
            std::str::from_utf8(&baseline.content).map_err(|_| McpFailure::InvalidArgument)?;
        if keelshell_mcp::content_sha256(old) != proposal.expected_sha256 {
            return Err(McpFailure::BackendFailure);
        }
        // Whole-file replacement diff is linear in the two bounded byte inputs;
        // no quadratic line alignment or diff recomputation runs on the UI.
        let mut diff = String::with_capacity(old.len() + proposal.replacement.len() + 32);
        diff.push_str("--- original\n+++ replacement\n");
        for (prefix, text) in [("- ", old), ("+ ", proposal.replacement.as_str())] {
            for line in text.split_inclusive('\n') {
                diff.push_str(prefix);
                diff.push_str(&crate::command_text::visible_command(line));
                if !line.ends_with('\n') {
                    diff.push_str(" [no final newline]\n");
                }
            }
        }
        Ok((baseline, diff))
    }
    .await;
    let closed = sftp.close().await;
    lease.check()?;
    let result = result?;
    closed.map_err(|_| McpFailure::BackendFailure)?;
    Ok(result)
}
