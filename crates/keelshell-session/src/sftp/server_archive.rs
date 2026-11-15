//! Reviewed server packing with read-only planning and conservative write outcomes.

use std::future::Future;
use std::sync::atomic::AtomicBool;

use keelshell_core::MAX_ARCHIVE_BYTES;
use keelshell_core::server_archive::{
    MAX_SERVER_ARCHIVE_INPUT_BYTES, ServerArchiveManifest, ServerArchiveReceipt,
    ServerArchiveRequest,
};
use sha2::{Digest, Sha256};
use tokio::time::{Instant, timeout_at};

use super::*;
use crate::ssh::ServerArchiveExec;

/// Immutable server archive review bound to one actual authenticated connection.
///
/// Planning performs no writes. The command, complete stdin and private staging
/// pathname are fixed before review. Clones share the same review, not independent
/// write authority. Application reservations exclude cooperating writers only;
/// SFTP observations cannot atomically lock paths against other server processes.
#[derive(Clone)]
pub struct ServerArchivePlan {
    connection: SshSession,
    reviewed: Arc<Reviewed>,
}

struct Reviewed {
    request: ServerArchiveRequest,
    manifest: ServerArchiveManifest,
    command: String,
    input: Vec<u8>,
    staging: String,
    parents: Vec<(String, ParentStamp)>,
}

impl std::fmt::Debug for ServerArchivePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerArchivePlan")
            .field("request", self.request())
            .field("member_count", &self.manifest().members().len())
            .field("expanded_bytes", &self.manifest().expanded_bytes())
            .field("staging_path", &self.staging_path())
            .finish_non_exhaustive()
    }
}

impl ServerArchivePlan {
    /// Exact selected sources, new destination and codec.
    pub fn request(&self) -> &ServerArchiveRequest {
        &self.reviewed.request
    }

    /// Complete inspected member paths, kinds, sizes and content digests.
    pub fn manifest(&self) -> &ServerArchiveManifest {
        &self.reviewed.manifest
    }

    /// Exact packing command to display in the confirmation UI.
    pub fn command(&self) -> &str {
        &self.reviewed.command
    }

    /// Complete canonical reviewed manifest sent to the helper on stdin.
    pub fn input(&self) -> &[u8] {
        &self.reviewed.input
    }

    /// Exact private sibling staging tree reserved for this review.
    pub fn staging_path(&self) -> &str {
        &self.reviewed.staging
    }
}

/// An observed server publication whose complete archive contents were verified.
///
/// This is readback evidence, not crash durability or a lock against later changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerArchiveTransferReceipt {
    /// Number of reviewed files and directories.
    pub member_count: usize,
    /// Exact final regular-file length.
    pub archive_bytes: u64,
    /// Sum of reviewed regular-file member sizes.
    pub expanded_bytes: u64,
    /// Exact remote destination.
    pub remote: String,
}

struct Authority<'a> {
    sftp: &'a SftpSession,
    cancelled: &'a AtomicBool,
    until: Instant,
}

impl Authority<'_> {
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) || self.sftp.is_closed() {
            Err(SessionError::Closed)
        } else if Instant::now() >= self.until {
            Err(SessionError::Timeout("server archive operation"))
        } else {
            Ok(())
        }
    }

    async fn wait<T>(&self, operation: impl Future<Output = Result<T>>) -> Result<T> {
        self.check()?;
        let revoked = async {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            loop {
                interval.tick().await;
                if self.cancelled.load(Ordering::Acquire) || self.sftp.is_closed() {
                    break;
                }
            }
        };
        let result = tokio::select! {
            biased;
            _ = self.sftp.cancelled() => Err(SessionError::Closed),
            _ = revoked => Err(SessionError::Closed),
            result = timeout_at(self.until, operation) => {
                result.map_err(|_| SessionError::Timeout("server archive operation"))?
            }
        };
        self.check()?;
        result
    }

    async fn packet<T>(
        &self,
        operation: impl Future<Output = std::result::Result<T, russh_sftp::client::error::Error>>,
    ) -> Result<T> {
        self.wait(async { operation.await.map_err(sftp_error) })
            .await
    }
}

impl SftpSession {
    /// Inspect a server-side archive after the caller reviews its inspection command.
    ///
    /// Runs the core's read-only helper on this SSH connection, parses its complete
    /// bounded report off the UI thread, and fixes the later command and stdin.
    /// No destination or staging tree is created. One deadline covers all work.
    pub async fn plan_server_archive(
        &self,
        request: ServerArchiveRequest,
        cancelled: &AtomicBool,
    ) -> Result<ServerArchivePlan> {
        let authority = Authority {
            sftp: self,
            cancelled,
            until: archive_deadline(self.timeout)?,
        };
        authority
            .wait(async {
                let staging = staging_path(request.destination())?;
                let raw = DirectoryChannel(self._connection.sftp_raw().await?);
                let parents = observe_new(&raw.0, &request, &staging, &authority).await?;
                let command = request.inspection_command();
                let output = self
                    ._connection
                    .server_archive_exec_until(
                        ServerArchiveExec {
                            command: &command,
                            input: &[],
                            until: authority.until,
                            cancelled,
                            pending: None,
                        },
                        || async { authority.check() },
                    )
                    .await?;
                accepted(&output)?;
                let reviewed = tokio::task::spawn_blocking(move || {
                    let manifest = ServerArchiveManifest::parse(&request, &output.stdout)
                        .map_err(|_| invalid_report())?;
                    let input = manifest.packing_input();
                    if input.len() > MAX_SERVER_ARCHIVE_INPUT_BYTES {
                        return Err(SessionError::OutputLimit(MAX_SERVER_ARCHIVE_INPUT_BYTES));
                    }
                    let command = request
                        .packing_command(&staging)
                        .map_err(|_| invalid_report())?;
                    Ok::<_, SessionError>(Reviewed {
                        request,
                        manifest,
                        command,
                        input,
                        staging,
                        parents,
                    })
                })
                .await
                .map_err(|_| SessionError::Worker)??;
                if observe_new(&raw.0, &reviewed.request, &reviewed.staging, &authority).await?
                    != reviewed.parents
                {
                    return Err(changed());
                }
                authority.check()?;
                Ok(ServerArchivePlan {
                    connection: self._connection.clone(),
                    reviewed: Arc::new(reviewed),
                })
            })
            .await
    }

    /// Execute the exact reviewed command and stdin once on its original connection.
    ///
    /// Reserves source trees for reading and destination/staging trees for writing.
    /// Every dispatched failure, cancellation, timeout or dropped future retains
    /// quarantine for both write paths. A process error or SSH close never proves
    /// no write occurred. There is no automatic retry, deletion or reconciliation.
    ///
    /// Success requires exec acknowledgement, complete output and zero exit status,
    /// a valid receipt, regular-file SFTP readback with matching size and SHA256,
    /// full core member verification and absence of the exact staging tree. Parser,
    /// digest and codec work runs on blocking workers under the same overall deadline.
    pub async fn execute_server_archive(
        &self,
        plan: ServerArchivePlan,
        cancelled: &AtomicBool,
    ) -> Result<ServerArchiveTransferReceipt> {
        let authority = Authority {
            sftp: self,
            cancelled,
            until: archive_deadline(self.timeout)?,
        };
        same_connection(self, &plan)?;
        authority.check()?;
        let authorized = || authority.check().is_ok();
        let scope = authority
            .wait(self.reserve_server_archive(plan.request(), plan.staging_path(), &authorized))
            .await?;
        let result = authority
            .wait(async {
                let raw = DirectoryChannel(self._connection.sftp_raw().await?);
                let output = self
                    ._connection
                    .server_archive_exec_until(
                        ServerArchiveExec {
                            command: plan.command(),
                            input: plan.input(),
                            until: authority.until,
                            cancelled,
                            pending: Some(scope.pending.as_ref()),
                        },
                        || async {
                            scope.revalidate().await?;
                            let current = observe_new(
                                &raw.0,
                                plan.request(),
                                plan.staging_path(),
                                &authority,
                            )
                            .await?;
                            if current != plan.reviewed.parents {
                                return Err(changed());
                            }
                            same_connection(self, &plan)?;
                            if !scope.ticket()?.is_active() {
                                return Err(SessionError::MutationQuarantined);
                            }
                            authority.check()
                        },
                    )
                    .await?;
                accepted(&output)?;
                let receipt = tokio::task::spawn_blocking(move || {
                    ServerArchiveReceipt::parse(&output.stdout).map_err(|_| invalid_report())
                })
                .await
                .map_err(|_| SessionError::Worker)??;
                let archive_bytes = receipt.archive_bytes();
                let bytes = read_published(
                    &raw.0,
                    plan.request().destination(),
                    receipt.archive_bytes(),
                    &plan.reviewed.parents,
                    &authority,
                )
                .await?;
                let reviewed = plan.reviewed.clone();
                tokio::task::spawn_blocking(move || {
                    let digest = Sha256::digest(&bytes)
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();
                    if bytes.len() as u64 != receipt.archive_bytes() || digest != receipt.sha256() {
                        return Err(invalid_report());
                    }
                    reviewed
                        .manifest
                        .verify_archive(&bytes)
                        .map_err(|_| invalid_report())
                })
                .await
                .map_err(|_| SessionError::Worker)??;
                absent(&raw.0, plan.staging_path(), &authority).await?;
                if parents(&raw.0, plan.request().destination(), &authority).await?
                    != plan.reviewed.parents
                {
                    return Err(changed());
                }
                same_connection(self, &plan)?;
                authority.check()?;
                if !scope.ticket()?.is_active() {
                    return Err(SessionError::MutationUncertain);
                }
                Ok(ServerArchiveTransferReceipt {
                    member_count: plan.manifest().members().len(),
                    archive_bytes,
                    expanded_bytes: plan.manifest().expanded_bytes(),
                    remote: plan.request().destination().to_owned(),
                })
            })
            .await;
        // No await follows this point: dropping a verification future can never
        // release isolation before its complete result was observed.
        if result.is_ok() {
            scope.pending.store(false, Ordering::Release);
        }
        scope.result(result)
    }
}

fn same_connection(sftp: &SftpSession, plan: &ServerArchivePlan) -> Result<()> {
    if !sftp._connection.same_connection(&plan.connection) {
        Err(SessionError::Invalid(
            "server archive review belongs to a different SSH connection",
        ))
    } else {
        Ok(())
    }
}

fn archive_deadline(timeout: Duration) -> Result<Instant> {
    Instant::now()
        .checked_add(timeout)
        .ok_or(SessionError::Invalid(
            "server archive timeout exceeds the clock range",
        ))
}

fn staging_path(destination: &str) -> Result<String> {
    let parent = directory::remote_parent(destination)?;
    let separator = if parent == "/" { "" } else { parent };
    Ok(format!(
        "{separator}/.keelshell-server-archive-{}",
        uuid::Uuid::new_v4()
    ))
}

fn accepted(output: &crate::ssh::ExecOutput) -> Result<()> {
    if output.exit_status != Some(0) || !output.stderr.is_empty() || output.stdout.is_empty() {
        Err(invalid_report())
    } else {
        Ok(())
    }
}

fn invalid_report() -> SessionError {
    SessionError::Invalid("invalid or unverified server archive result")
}

fn changed() -> SessionError {
    SessionError::Invalid("server archive paths changed; inspect and review again")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParentStamp {
    mode: u32,
    uid: Option<u32>,
    gid: Option<u32>,
}

async fn parents(
    raw: &RawSftpSession,
    destination: &str,
    authority: &Authority<'_>,
) -> Result<Vec<(String, ParentStamp)>> {
    let parent = directory::remote_parent(destination)?;
    let mut current = String::new();
    let mut paths = vec!["/".to_owned()];
    for part in parent.split('/').filter(|part| !part.is_empty()) {
        current.push('/');
        current.push_str(part);
        paths.push(current.clone());
    }
    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        let attrs = authority.packet(raw.lstat(&path)).await?.attrs;
        let mode = attrs
            .permissions
            .filter(|mode| mode & 0o170000 == 0o040000)
            .ok_or_else(changed)?;
        let canonical = authority.packet(raw.realpath(&path)).await?;
        if canonical.files.len() != 1 || canonical.files[0].filename != path {
            return Err(changed());
        }
        result.push((
            path,
            ParentStamp {
                mode,
                uid: attrs.uid,
                gid: attrs.gid,
            },
        ));
    }
    Ok(result)
}

async fn absent(raw: &RawSftpSession, path: &str, authority: &Authority<'_>) -> Result<()> {
    authority
        .wait(async {
            match raw.lstat(path).await {
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::NoSuchFile =>
                {
                    Ok(())
                }
                Ok(_) => Err(SessionError::Invalid(
                    "server archive destination or staging already exists",
                )),
                Err(error) => Err(sftp_error(error)),
            }
        })
        .await
}

async fn observe_new(
    raw: &RawSftpSession,
    request: &ServerArchiveRequest,
    staging: &str,
    authority: &Authority<'_>,
) -> Result<Vec<(String, ParentStamp)>> {
    let chain = parents(raw, request.destination(), authority).await?;
    absent(raw, request.destination(), authority).await?;
    absent(raw, staging, authority).await?;
    Ok(chain)
}

#[derive(Debug, PartialEq, Eq)]
struct FileStamp {
    mode: u32,
    bytes: u64,
    modified: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
}

impl FileStamp {
    fn parse(attrs: FileAttributes, expected: u64) -> Result<Self> {
        let mode = attrs
            .permissions
            .filter(|mode| mode & 0o170000 == 0o100000 && mode & 0o7777 == 0o600)
            .ok_or_else(invalid_report)?;
        let bytes = attrs
            .size
            .filter(|size| *size == expected && *size <= MAX_ARCHIVE_BYTES)
            .ok_or_else(invalid_report)?;
        Ok(Self {
            mode,
            bytes,
            modified: attrs.mtime,
            uid: attrs.uid,
            gid: attrs.gid,
        })
    }
}

async fn read_published(
    raw: &RawSftpSession,
    path: &str,
    expected: u64,
    reviewed_parents: &[(String, ParentStamp)],
    authority: &Authority<'_>,
) -> Result<Vec<u8>> {
    if expected == 0 || expected > MAX_ARCHIVE_BYTES {
        return Err(invalid_report());
    }
    if parents(raw, path, authority).await?.as_slice() != reviewed_parents {
        return Err(changed());
    }
    let before = FileStamp::parse(authority.packet(raw.lstat(path)).await?.attrs, expected)?;
    let handle = authority
        .packet(raw.open(path, OpenFlags::READ, FileAttributes::empty()))
        .await?
        .handle;
    let result = async {
        if FileStamp::parse(authority.packet(raw.fstat(&handle)).await?.attrs, expected)? != before
        {
            return Err(changed());
        }
        let mut bytes = Vec::new();
        loop {
            let length = (expected.saturating_sub(bytes.len() as u64) + 1).min(64 * 1024) as u32;
            let packet = authority
                .wait(async {
                    match raw.read(&handle, bytes.len() as u64, length).await {
                        Ok(packet) => Ok(Some(packet.data)),
                        Err(russh_sftp::client::error::Error::Status(status))
                            if status.status_code == StatusCode::Eof =>
                        {
                            Ok(None)
                        }
                        Err(error) => Err(sftp_error(error)),
                    }
                })
                .await?;
            let Some(data) = packet else { break };
            if data.is_empty()
                || data.len() > length as usize
                || (bytes.len() as u64).saturating_add(data.len() as u64) > expected
            {
                return Err(invalid_report());
            }
            bytes.extend_from_slice(&data);
        }
        if bytes.len() as u64 != expected
            || FileStamp::parse(authority.packet(raw.fstat(&handle)).await?.attrs, expected)?
                != before
            || FileStamp::parse(authority.packet(raw.lstat(path)).await?.attrs, expected)? != before
            || parents(raw, path, authority).await?.as_slice() != reviewed_parents
        {
            return Err(changed());
        }
        Ok(bytes)
    }
    .await;
    let close = authority.packet(raw.close(handle)).await;
    let bytes = result?;
    close?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_archive_stage_is_an_exact_uuid_sibling_before_review() -> Result<()> {
        let stage = staging_path("/home/fixture/packed.zip")?;
        let suffix = stage
            .strip_prefix("/home/fixture/.keelshell-server-archive-")
            .ok_or(SessionError::Invalid("stage was not a sibling"))?;
        assert!(uuid::Uuid::parse_str(suffix).is_ok());
        Ok(())
    }

    #[test]
    fn server_archive_root_sibling_stage_has_no_double_separator() -> Result<()> {
        let stage = staging_path("/packed.zip")?;
        assert!(stage.starts_with("/.keelshell-server-archive-") && !stage.starts_with("//"));
        Ok(())
    }

    fn file(mode: u32, size: Option<u64>) -> FileAttributes {
        let mut attrs = FileAttributes::empty();
        attrs.permissions = Some(mode);
        attrs.size = size;
        attrs
    }

    #[test]
    fn server_archive_published_file_stamp_rejects_symlinks_even_with_correct_size() {
        assert!(FileStamp::parse(file(0o120600, Some(16)), 16).is_err());
    }

    #[test]
    fn server_archive_published_file_stamp_rejects_unknown_size() {
        assert!(FileStamp::parse(file(0o100600, None), 16).is_err());
    }

    #[test]
    fn server_archive_published_file_stamp_rejects_size_over_encoded_cap() {
        assert!(
            FileStamp::parse(
                file(0o100600, Some(MAX_ARCHIVE_BYTES + 1)),
                MAX_ARCHIVE_BYTES + 1
            )
            .is_err()
        );
    }

    #[test]
    fn server_archive_published_file_stamp_rejects_unexpected_public_permissions() {
        assert!(FileStamp::parse(file(0o100644, Some(16)), 16).is_err());
    }

    #[test]
    fn server_archive_published_file_stamp_rejects_receipt_length_mismatch() {
        assert!(FileStamp::parse(file(0o100600, Some(17)), 16).is_err());
    }

    #[test]
    fn server_archive_published_file_stamp_accepts_private_complete_regular_file() {
        assert!(FileStamp::parse(file(0o100600, Some(16)), 16).is_ok());
    }

    #[test]
    fn server_archive_receipt_requires_an_explicit_successful_exit() {
        let output = crate::ssh::ExecOutput {
            stdout: b"{}".to_vec(),
            stderr: vec![],
            exit_status: None,
        };
        assert!(accepted(&output).is_err());
    }

    #[test]
    fn server_archive_receipt_rejects_nonempty_stderr_even_with_zero_exit() {
        let output = crate::ssh::ExecOutput {
            stdout: b"{}".to_vec(),
            stderr: b"warning".to_vec(),
            exit_status: Some(0),
        };
        assert!(accepted(&output).is_err());
    }

    #[test]
    fn server_archive_receipt_rejects_empty_stdout_even_with_zero_exit() {
        let output = crate::ssh::ExecOutput {
            stdout: vec![],
            stderr: vec![],
            exit_status: Some(0),
        };
        assert!(accepted(&output).is_err());
    }
}
