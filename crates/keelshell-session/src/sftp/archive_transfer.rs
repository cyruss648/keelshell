//! Reviewed, bounded client-side archives. No remote shell or recovery journal.

use std::future::Future;
use std::sync::atomic::AtomicBool;

use keelshell_core::{ArchiveBundle, ArchiveFormat, ArchiveMember, MAX_ARCHIVE_BYTES};

use super::*;

#[cfg_attr(windows, path = "archive_transfer/local_windows.rs")]
mod local;
#[cfg(any(windows, test))]
mod mutation_worker;
mod remote;

/// An archive operation whose complete contents must be reviewed before execution.
///
/// Paths must be absolute, normalized, UTF-8 and free of symbolic links, reparse
/// points and portable filename aliases. Existing destinations are never replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveTransferRequest {
    /// Pack one to 32 local files or directories, retaining each selected basename,
    /// and publish a new remote archive using standard SFTP rename.
    PackUpload {
        /// Exact selected local source paths, including selected empty directories.
        sources: Vec<PathBuf>,
        /// New absolute remote archive pathname.
        remote: String,
        /// Explicit archive codec; never inferred from an untrusted filename.
        format: ArchiveFormat,
    },
    /// Read and decode every remote member before review, then extract to a new
    /// local directory. Archive modes, executable bits and ownership are discarded.
    DownloadExtract {
        /// Absolute remote regular archive pathname.
        remote: String,
        /// New local directory, with an existing checked parent.
        local: PathBuf,
        /// Explicit archive codec.
        format: ArchiveFormat,
    },
}

impl ArchiveTransferRequest {
    fn format(&self) -> ArchiveFormat {
        match self {
            Self::PackUpload { format, .. } | Self::DownloadExtract { format, .. } => *format,
        }
    }

    fn remote(&self) -> &str {
        match self {
            Self::PackUpload { remote, .. } | Self::DownloadExtract { remote, .. } => remote,
        }
    }
}

/// Immutable archive review, bound to its original authenticated SSH connection.
///
/// Clones share bounded content and metadata without creating independent write
/// authority. Execution re-reads all source bytes and checks the complete manifest
/// before staging, and again before publication. Application reservations exclude
/// cooperating writers; portable SFTP observations cannot lock out server-side
/// processes or provide atomic no-follow/version checks.
#[derive(Clone)]
pub struct ArchiveTransferPlan {
    connection: SshSession,
    request: ArchiveTransferRequest,
    reviewed: Arc<ReviewedArchive>,
}

struct ReviewedArchive {
    bundle: ArchiveBundle,
    encoded: Vec<u8>,
    local: local::Manifest,
    remote: remote::Observation,
}

impl std::fmt::Debug for ArchiveTransferPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveTransferPlan")
            .field("request", &self.request)
            .field("member_count", &self.members().len())
            .field("archive_bytes", &self.archive_bytes())
            .field("expanded_bytes", &self.expanded_bytes())
            .finish_non_exhaustive()
    }
}

impl ArchiveTransferPlan {
    /// Exact request whose paths and format were inspected.
    pub fn request(&self) -> &ArchiveTransferRequest {
        &self.request
    }

    /// Complete validated member manifest, including all file bytes.
    pub fn members(&self) -> &[ArchiveMember] {
        self.reviewed.bundle.members()
    }

    /// Exact encoded archive length, never a truncated or guessed value.
    pub fn archive_bytes(&self) -> u64 {
        self.reviewed.encoded.len() as u64
    }

    /// Sum of all fully decoded regular-file member lengths.
    pub fn expanded_bytes(&self) -> u64 {
        self.reviewed.bundle.expanded_bytes()
    }

    /// Explicit reviewed archive codec.
    pub fn format(&self) -> ArchiveFormat {
        self.request.format()
    }
}

/// A completely published archive operation with verified readback.
///
/// This acknowledges observed publication, not filesystem crash durability or
/// protection against unrelated processes changing the result after readback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveTransferReceipt {
    /// Number of reviewed file and directory members.
    pub member_count: usize,
    /// Exact encoded archive length.
    pub archive_bytes: u64,
    /// Sum of the reviewed regular-file contents.
    pub expanded_bytes: u64,
}

/// Check current human authority around every await. Revocation while a mutation
/// is awaiting its reply leaves the owning scope's pending flag set.
struct Authority<'a> {
    sftp: &'a SftpSession,
    authorized: &'a (dyn Fn() -> bool + Sync),
}

impl Authority<'_> {
    fn check(&self) -> Result<()> {
        if self.sftp.is_closed() || !(self.authorized)() {
            Err(SessionError::Closed)
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
                if self.check().is_err() {
                    return;
                }
            }
        };
        let result = tokio::select! {
            biased;
            _ = self.sftp.cancelled() => Err(SessionError::Closed),
            _ = revoked => Err(SessionError::Closed),
            result = operation => result,
        };
        self.check()?;
        result
    }

    #[cfg(not(windows))]
    async fn io<T>(&self, operation: impl Future<Output = std::io::Result<T>>) -> Result<T> {
        self.wait(async { operation.await.map_err(Into::into) })
            .await
    }

    async fn packet<T>(
        &self,
        operation: impl Future<Output = std::result::Result<T, russh_sftp::client::error::Error>>,
    ) -> Result<T> {
        self.wait(async { operation.await.map_err(sftp_error) })
            .await
    }

    async fn mutate<T>(
        &self,
        scope: &FileMutationScope<'_>,
        operation: impl Future<Output = std::result::Result<T, russh_sftp::client::error::Error>>,
    ) -> Result<T> {
        self.wait(scope.remote(operation)).await
    }

    #[cfg(not(windows))]
    async fn local_mutation<T>(
        &self,
        scope: &FileMutationScope<'_>,
        operation: impl Future<Output = std::io::Result<T>>,
    ) -> Result<T> {
        self.wait(scope.revalidate()).await?;
        self.check()?;
        self.wait(async {
            scope.pending.store(true, Ordering::Release);
            let result = operation.await;
            scope.pending.store(false, Ordering::Release);
            result.map_err(Into::into)
        })
        .await
    }
}

/// Keep an acknowledged staging tree isolated if it is not fully published and
/// verified. No Drop implementation deletes evidence or spawns cleanup writes.
struct UnverifiedStage {
    ticket: Option<queue::Reservation>,
}

impl Drop for UnverifiedStage {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket.take() {
            ticket.finish(false);
        }
    }
}

impl SftpSession {
    /// Prepare a complete archive review without creating either destination.
    ///
    /// Local sources are traversed under the core codec bounds with full content
    /// digests and metadata. Remote archives are strictly read under the encoded
    /// size bound and entirely decoded before returning. Cancellation is checked
    /// around I/O; codec work runs on a blocking worker and never writes files.
    pub async fn plan_archive_transfer(
        &self,
        request: ArchiveTransferRequest,
        authorized: &(dyn Fn() -> bool + Sync),
    ) -> Result<ArchiveTransferPlan> {
        let authority = Authority {
            sftp: self,
            authorized,
        };
        deadline(
            self.timeout,
            "SFTP archive review",
            Box::pin(self.inspect_archive(request, &authority)),
        )
        .await
    }

    async fn inspect_archive(
        &self,
        request: ArchiveTransferRequest,
        authority: &Authority<'_>,
    ) -> Result<ArchiveTransferPlan> {
        authority.check()?;
        directory::validate_remote_root(request.remote())?;
        let raw = DirectoryChannel(authority.wait(self._connection.sftp_raw()).await?);
        let remote = remote::observe(&raw.0, &request, authority).await?;
        let (bundle, encoded, local) = match &request {
            ArchiveTransferRequest::PackUpload {
                sources, format, ..
            } => {
                let (manifest, members) = local::scan(sources, authority).await?;
                let format = *format;
                let (bundle, encoded) = authority
                    .wait(async {
                        tokio::task::spawn_blocking(move || {
                            let bundle =
                                ArchiveBundle::new(members).map_err(|_| invalid_archive())?;
                            let encoded = bundle.encode(format).map_err(|_| invalid_archive())?;
                            Ok::<_, SessionError>((bundle, encoded))
                        })
                        .await
                        .map_err(|_| SessionError::Worker)?
                    })
                    .await?;
                (bundle, encoded, manifest)
            }
            ArchiveTransferRequest::DownloadExtract { local, format, .. } => {
                local::extraction_supported()?;
                let manifest = local::destination(local, authority).await?;
                let snapshot = remote::read(&raw.0, request.remote(), authority).await?;
                if remote.leaf.as_ref() != Some(&snapshot.0) {
                    return Err(changed());
                }
                let encoded = snapshot.1;
                let format = *format;
                let (bundle, encoded) = authority
                    .wait(async {
                        tokio::task::spawn_blocking(move || {
                            let bundle = ArchiveBundle::decode(format, &encoded)
                                .map_err(|_| invalid_archive())?;
                            Ok::<_, SessionError>((bundle, encoded))
                        })
                        .await
                        .map_err(|_| SessionError::Worker)?
                    })
                    .await?;
                local::validate_destinations(local, bundle.members())?;
                (bundle, encoded, manifest)
            }
        };
        if encoded.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(SessionError::OutputLimit(MAX_ARCHIVE_BYTES as usize));
        }
        if remote::observe(&raw.0, &request, authority).await? != remote {
            return Err(changed());
        }
        authority.check()?;
        Ok(ArchiveTransferPlan {
            connection: self._connection.clone(),
            request,
            reviewed: Arc::new(ReviewedArchive {
                bundle,
                encoded,
                local,
                remote,
            }),
        })
    }

    /// Execute exactly one previously reviewed archive on its original SSH identity.
    ///
    /// All source bytes, metadata and ancestors are rechecked before any write and
    /// before publication. Uploads use a private EXCL file and standard SFTP rename,
    /// never replacement rename. Extraction uses owner-only staging in the existing
    /// parent and an OS no-replace directory rename. Published bytes are read back.
    /// Lost mutation replies, failed readback and abandoned staging retain in-memory
    /// quarantine and evidence; no ephemeral pathname enters the recovery journal.
    ///
    /// SFTP v3 cannot prevent hostile server-side parent swaps. Local ancestor
    /// checks detect observed changes but are not filesystem-wide locks. Directory
    /// publication refuses replacement atomically on supported filesystems and
    /// fails closed when that primitive or owner-only staging is unavailable.
    pub async fn execute_archive_transfer(
        &self,
        plan: ArchiveTransferPlan,
        authorized: &(dyn Fn() -> bool + Sync),
    ) -> Result<ArchiveTransferReceipt> {
        if !self._connection.same_connection(&plan.connection) {
            return Err(SessionError::Invalid(
                "archive review belongs to a different SSH connection",
            ));
        }
        let authority = Authority {
            sftp: self,
            authorized,
        };
        authority.check()?;
        let expires = tokio::time::Instant::now() + self.timeout;
        let staging = match &plan.request {
            ArchiveTransferRequest::DownloadExtract { local, .. } => {
                Some(local::staging_path(local)?)
            }
            _ => None,
        };
        let scope = tokio::time::timeout_at(
            expires,
            authority.wait(self.reserve_archive_transfer(
                &plan.request,
                staging.as_deref(),
                authorized,
            )),
        )
        .await
        .map_err(|_| SessionError::Timeout("SFTP archive admission"))??;
        let result = tokio::time::timeout_at(
            expires,
            Box::pin(async {
                let raw = DirectoryChannel(authority.wait(self._connection.sftp_raw()).await?);
                self.revalidate_archive(&plan, &raw.0, &authority).await?;
                match &plan.request {
                    ArchiveTransferRequest::PackUpload { remote, .. } => {
                        remote::publish(self, &plan, &raw.0, remote, &scope, &authority).await?;
                    }
                    ArchiveTransferRequest::DownloadExtract { local, .. } => {
                        local::extract(
                            self,
                            &plan,
                            &raw.0,
                            local,
                            staging.as_deref().ok_or(SessionError::Worker)?,
                            &scope,
                            &authority,
                        )
                        .await?;
                    }
                }
                authority.check()?;
                Ok(ArchiveTransferReceipt {
                    member_count: plan.members().len(),
                    archive_bytes: plan.archive_bytes(),
                    expanded_bytes: plan.expanded_bytes(),
                })
            }),
        )
        .await
        .map_err(|_| SessionError::Timeout("SFTP reviewed archive transfer"))
        .and_then(std::convert::identity);
        scope.complete(result).await
    }

    async fn revalidate_archive(
        &self,
        plan: &ArchiveTransferPlan,
        raw: &RawSftpSession,
        authority: &Authority<'_>,
    ) -> Result<()> {
        authority.check()?;
        if remote::observe(raw, &plan.request, authority).await? != plan.reviewed.remote {
            return Err(changed());
        }
        match &plan.request {
            ArchiveTransferRequest::PackUpload { sources, .. } => {
                let (manifest, members) = local::scan(sources, authority).await?;
                if manifest != plan.reviewed.local || members != plan.members() {
                    return Err(changed());
                }
            }
            ArchiveTransferRequest::DownloadExtract { remote, local, .. } => {
                if local::destination(local, authority).await? != plan.reviewed.local {
                    return Err(changed());
                }
                let (stamp, bytes) = remote::read(raw, remote, authority).await?;
                if Some(&stamp) != plan.reviewed.remote.leaf.as_ref()
                    || bytes != plan.reviewed.encoded
                {
                    return Err(changed());
                }
            }
        }
        authority.check()
    }
}

fn changed() -> SessionError {
    SessionError::Invalid("archive source, destination or ancestors changed; review again")
}

fn invalid_archive() -> SessionError {
    SessionError::Invalid("archive codec rejected invalid, unsafe or over-limit contents")
}
