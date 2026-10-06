//! Shared admission for application mutations, separate from command authority.
use super::*;
use queue::{Admission, Reservation, ResourceClaims};
use std::sync::atomic::AtomicBool;

#[derive(Clone)]
enum RequestedTarget {
    Local(PathBuf, bool, bool),
    Remote(String, bool, bool),
}
async fn resolve(sftp: &SftpSession, targets: &[RequestedTarget]) -> Result<ResourceClaims> {
    let mut claims = ResourceClaims::default();
    for target in targets {
        match target {
            RequestedTarget::Local(path, write, namespace) => {
                let mut claim = queue::local_claim(path, *write).await?;
                if *namespace {
                    claim.whole_side = false;
                }
                claims.local.push(claim);
            }
            RequestedTarget::Remote(path, write, namespace) => {
                // A synchronization may reserve the canonical filesystem root,
                // which has no atomic file basename. Its namespace is still a
                // tree claim, not an in-place inode write.
                let mut claim =
                    queue::remote_claim(sftp, path, *write, *namespace && path != "/").await?;
                if *namespace {
                    claim.whole_side = false;
                }
                // An in-place CREATE changes existence before the first WRITE.
                // Reserve the conservative inode side from admission onward,
                // so revalidation cannot silently broaden after creation.
                if *write && !*namespace {
                    claim.whole_side = true;
                }
                claims.remote.push(claim);
            }
        }
    }
    Ok(claims)
}

/// A reviewed directory synchronization's shared resource ownership.
///
/// The source tree is reserved for reading and the destination for writing.
/// Child mutations reuse this owner, including locally published temporary files.
/// Keep this scope through fresh plan validation and every child operation. It
/// does not authorize a write: `authorized` is checked after awaits and before
/// each mutation. Dropping a pending mutation retains process-wide quarantine.
/// All synchronous local operations must run off the application's UI thread.
pub struct FileMutationScope<'a> {
    sftp: &'a SftpSession,
    authorized: &'a (dyn Fn() -> bool + Sync),
    targets: Vec<RequestedTarget>,
    claims: ResourceClaims,
    reservation: Option<Reservation>,
    pub(super) pending: Arc<AtomicBool>,
    child_active: AtomicBool,
}
struct ChildOperation<'a>(&'a AtomicBool);
impl Drop for ChildOperation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Drop for FileMutationScope<'_> {
    fn drop(&mut self) {
        if let Some(reservation) = self.reservation.take() {
            reservation.finish(!self.pending.load(Ordering::Acquire));
        }
    }
}
impl FileMutationScope<'_> {
    fn begin_child(&self) -> Result<ChildOperation<'_>> {
        self.child_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| SessionError::MutationBusy)?;
        Ok(ChildOperation(&self.child_active))
    }
    pub(super) fn ticket(&self) -> Result<Reservation> {
        self.reservation.clone().ok_or(SessionError::Closed)
    }
    pub(super) fn result<T>(&self, result: Result<T>) -> Result<T> {
        if self.pending.load(Ordering::Acquire) {
            Err(SessionError::MutationUncertain)
        } else {
            result
        }
    }
    pub(super) async fn complete<T>(&self, result: Result<T>) -> Result<T> {
        if self.pending.load(Ordering::Acquire) {
            return Err(SessionError::MutationUncertain);
        }
        if !self.ticket()?.is_active() {
            return Err(SessionError::MutationUncertain);
        }
        if !self.ticket()?.cleanup_known().await {
            return Err(SessionError::MutationUncertain);
        }
        self.result(result)
    }
    fn authority(&self) -> Result<()> {
        if self.sftp.is_closed() || !(self.authorized)() {
            Err(SessionError::Closed)
        } else {
            Ok(())
        }
    }
    pub(super) async fn revalidate(&self) -> Result<()> {
        self.authority()?;
        if !self.ticket()?.is_active() {
            return Err(SessionError::MutationQuarantined);
        }
        if self.pending.load(Ordering::Acquire) {
            return Err(SessionError::MutationUncertain);
        }
        let current = resolve(self.sftp, &self.targets).await?;
        self.authority()?;
        if !self.ticket()?.is_active() {
            return Err(SessionError::MutationQuarantined);
        }
        if current != self.claims {
            return Err(SessionError::Invalid(
                "mutation paths changed after admission; review again",
            ));
        }
        Ok(())
    }
    pub(super) async fn remote<T>(
        &self,
        operation: impl std::future::Future<
            Output = std::result::Result<T, russh_sftp::client::error::Error>,
        >,
    ) -> Result<T> {
        self.revalidate().await?;
        let operation = async {
            self.pending.store(true, Ordering::Release);
            let result = operation.await;
            if result.is_ok() || matches!(&result, Err(russh_sftp::client::error::Error::Status(_)))
            {
                self.pending.store(false, Ordering::Release);
            }
            result.map_err(sftp_error)
        };
        let result = tokio::select! {
            biased;
            _=self.sftp.cancelled()=>Err(SessionError::Closed),
            result=operation=>result,
        };
        self.result(result)
    }

    async fn remote_child(&self, path: &str) -> Result<()> {
        let child = queue::remote_claim(self.sftp, path, true, true).await?;
        if !self
            .claims
            .remote
            .iter()
            .any(|root| root.write && child.path.starts_with(&root.path))
        {
            return Err(SessionError::Invalid(
                "mutation is outside the reserved remote tree",
            ));
        }
        self.authority()
    }
    /// Create one directory inside the reserved remote destination tree.
    pub async fn mkdir_remote(&self, path: &str) -> Result<()> {
        let _child = self.begin_child()?;
        self.remote_child(path).await?;
        let raw = DirectoryChannel(self.sftp._connection.sftp_raw().await?);
        self.remote(raw.0.mkdir(path, FileAttributes::empty()))
            .await?;
        Ok(())
    }
    /// Atomically publish a file inside the reserved remote destination tree.
    /// Uses the same exclusive ownership and no-fallback rules as `write_atomic`.
    pub async fn write_remote_atomic(&self, path: &str, data: &[u8]) -> Result<()> {
        let _child = self.begin_child()?;
        self.remote_child(path).await?;
        self.sftp
            .replace_from_reader_checked(path, &mut &data[..], None, None, Some(self))
            .await?;
        Ok(())
    }
    /// Perform one synchronous local mutation and observe its actual completion.
    /// The closure may only change the reserved local destination or descendants;
    /// it must not launch detached work. A panic leaves its outcome quarantined.
    pub fn local_operation<T>(&self, operation: impl FnOnce() -> std::io::Result<T>) -> Result<T> {
        let _child = self.begin_child()?;
        self.authority()?;
        if !self.ticket()?.is_active() {
            return Err(SessionError::MutationQuarantined);
        }
        if self.pending.load(Ordering::Acquire) {
            return Err(SessionError::MutationUncertain);
        }
        if !self.claims.local.iter().any(|c| c.write) {
            return Err(SessionError::Invalid("scope has no local destination"));
        }
        self.pending.store(true, Ordering::Release);
        let result = operation();
        self.pending.store(false, Ordering::Release);
        result.map_err(Into::into)
    }
    pub(super) fn context(&self) -> Result<TransferContext> {
        let (events, _receiver) = mpsc::channel(1);
        let mut context = TransferContext::new(0, None, events, Arc::new(TransferControl::new()));
        context.mutation_pending = self.pending.clone();
        context.reservation = Some(self.ticket()?);
        Ok(context)
    }
}
impl SftpSession {
    async fn reserve_mutation<'a>(
        &'a self,
        targets: Vec<RequestedTarget>,
        authorized: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<FileMutationScope<'a>> {
        deadline(self.timeout, "file mutation admission", async {
            if self.is_closed() || !authorized() {
                return Err(SessionError::Closed);
            }
            let claims = resolve(self, &targets).await?;
            if self.is_closed() || !authorized() {
                return Err(SessionError::Closed);
            }
            let reservation = match TransferReservations::admit_resources(
                &self._connection.transfer_reservations,
                &claims,
            ) {
                Admission::Busy => return Err(SessionError::MutationBusy),
                Admission::Quarantined => return Err(SessionError::MutationQuarantined),
                Admission::Ready(reservation) => reservation,
            };
            let scope = FileMutationScope {
                sftp: self,
                authorized,
                targets,
                claims,
                reservation: Some(reservation),
                pending: Arc::new(AtomicBool::new(false)),
                child_active: AtomicBool::new(false),
            };
            scope.revalidate().await?;
            Ok(scope)
        })
        .await
    }
    pub(super) async fn reserve_remote<'a>(
        &'a self,
        paths: &[&str],
        namespace: bool,
        authorized: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<FileMutationScope<'a>> {
        self.reserve_mutation(
            paths
                .iter()
                .map(|p| RequestedTarget::Remote((*p).to_owned(), true, namespace))
                .collect(),
            authorized,
        )
        .await
    }
    pub(super) async fn reserve_transfer(
        &self,
        spec: &TransferSpec,
        atomic: bool,
    ) -> Result<FileMutationScope<'_>> {
        self.reserve_mutation(
            vec![
                RequestedTarget::Local(
                    spec.local.clone(),
                    spec.direction == TransferDirection::Download,
                    false,
                ),
                RequestedTarget::Remote(
                    spec.remote.clone(),
                    spec.direction == TransferDirection::Upload,
                    atomic,
                ),
            ],
            &|| true,
        )
        .await
    }
    /// Admit both trees of an explicitly reviewed directory synchronization.
    /// This is immediate bounded exclusion, not permission or automatic waiting.
    /// Source reads are shared; the destination subtree excludes all application
    /// writers and unknown results. Child methods must reuse the returned scope.
    pub async fn reserve_directory_sync<'a>(
        &'a self,
        spec: &TransferSpec,
        authorized: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<FileMutationScope<'a>> {
        self.reserve_mutation(
            vec![
                RequestedTarget::Local(
                    spec.local.clone(),
                    spec.direction == TransferDirection::Download,
                    true,
                ),
                RequestedTarget::Remote(
                    spec.remote.clone(),
                    spec.direction == TransferDirection::Upload,
                    spec.direction == TransferDirection::Upload,
                ),
            ],
            authorized,
        )
        .await
    }
}
