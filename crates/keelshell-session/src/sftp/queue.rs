//! Bounded admission, conflict reservations and independently owned SFTP jobs.
use super::*;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::task::{JoinHandle, JoinSet};

/// Maximum number of concurrently admitted transfer workers on one queue.
pub const MAX_PARALLEL_TRANSFERS: usize = 4;
/// Maximum admitted transfers, including running and paused jobs.
pub const MAX_QUEUED_TRANSFERS: usize = 32;

/// A session-bound SFTP queue with adjustable, bounded parallelism.
///
/// Independent jobs use separate raw SFTP channels. Overlapping paths are
/// serialized whenever at least one job writes; directory roots reserve their
/// descendants. Paused jobs keep their worker slot and reservations. Existing
/// in-place destinations conservatively reserve their entire filesystem side
/// because portable SFTP cannot identify hard-link aliases. Atomic uploads
/// reserve their publication paths. Local claims are shared across this process;
/// remote claims share a verified endpoint scope, including other queues and
/// connections. External programs and different host aliases remain outside it.
///
/// No job is retried or rebound to a new SSH connection. Dropping the queue
/// requests cancellation and drains its owned workers in the background; a
/// caller requiring observed cleanup should use [`Self::close`].
pub struct TransferQueue {
    commands: mpsc::Sender<TransferCommand>,
    next_id: AtomicU64,
    admission: Arc<Semaphore>,
    parallelism: watch::Sender<usize>,
    stop: watch::Sender<bool>,
    worker: Option<JoinHandle<()>>,
    sftp: Arc<SftpSession>,
}

enum TransferJob {
    File(TransferSpec),
    AtomicUpload(TransferSpec),
    ReviewedFile(FileTransferPlan),
    Directory(DirectoryTransferPlan),
    Resume(FileResumePlan),
    DirectoryResume(DirectoryResumePlan),
}
impl TransferJob {
    fn spec(&self) -> TransferSpec {
        match self {
            Self::File(spec) | Self::AtomicUpload(spec) => spec.clone(),
            Self::ReviewedFile(plan) => TransferSpec {
                direction: plan.direction(),
                local: plan.local_path().to_owned(),
                remote: plan.remote_path().to_owned(),
            },
            Self::Directory(plan) => TransferSpec {
                direction: plan.direction(),
                local: plan.local_path().to_owned(),
                remote: plan.remote_path().to_owned(),
            },
            Self::Resume(plan) => TransferSpec {
                direction: plan.direction(),
                local: plan.local_path().to_owned(),
                remote: plan.remote_path().to_owned(),
            },
            Self::DirectoryResume(plan) => TransferSpec {
                direction: plan.direction(),
                local: plan.local_path().to_owned(),
                remote: plan.remote_path().to_owned(),
            },
        }
    }
}

struct TransferCommand {
    id: u64,
    spec: TransferJob,
    claims: Claims,
    events: mpsc::Sender<TransferEvent>,
    terminal: oneshot::Sender<TransferEvent>,
    control: Arc<TransferControl>,
    _admission: OwnedSemaphorePermit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Claim {
    pub(super) path: Vec<String>,
    pub(super) write: bool,
    pub(super) whole_side: bool,
    pub(super) target: reservations::Target,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Claims {
    local: Claim,
    remote: Claim,
}

mod reservations;
pub(crate) use reservations::TransferReservations;
pub(super) use reservations::{Admission, Reservation, ResourceClaims, TemporaryReservation};
pub use reservations::{TransferQuarantineEntry, TransferQuarantineReview};

fn overlap(a: &Claim, b: &Claim) -> bool {
    (a.write || b.write)
        && (a.whole_side
            || b.whole_side
            || a.path.starts_with(&b.path)
            || b.path.starts_with(&a.path))
}
impl Claims {
    fn conflicts(&self, other: &Self) -> bool {
        overlap(&self.local, &other.local) || overlap(&self.remote, &other.remote)
    }
}

pub(super) async fn local_claim(path: &Path, write: bool) -> Result<Claim> {
    use std::path::Component;
    if path.as_os_str().is_empty() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(SessionError::Invalid(
            "transfer paths must not contain parent traversal",
        ));
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let whole_side = write
        && local_io(tokio::fs::symlink_metadata(&absolute))
            .await
            .is_ok();
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    let canonical = loop {
        match local_io(tokio::fs::canonicalize(ancestor)).await {
            Ok(canonical) => break canonical,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or(SessionError::Invalid("transfer parent unavailable"))?
                        .to_owned(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or(SessionError::Invalid("transfer parent unavailable"))?;
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut target = canonical.clone();
    for part in missing.iter().rev() {
        target.push(part);
    }
    let mut components: Vec<_> = canonical
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    components.extend(
        missing
            .into_iter()
            .rev()
            .map(|s| s.to_string_lossy().to_lowercase()),
    );
    Ok(Claim {
        path: components,
        write,
        whole_side,
        target: reservations::Target::Local(target),
    })
}

pub(super) async fn remote_claim(
    sftp: &SftpSession,
    path: &str,
    write: bool,
    atomic: bool,
) -> Result<Claim> {
    valid_path(path)?;
    if path.split('/').any(|part| part == "..") {
        return Err(SessionError::Invalid(
            "transfer paths must not contain parent traversal",
        ));
    }
    let whole_side = write && !atomic && remote_io(sftp.inner.symlink_metadata(path)).await.is_ok();
    let mut missing = Vec::new();
    let mut ancestor = if atomic {
        valid_atomic_path(path)?;
        let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
        missing.push(name.to_owned());
        if parent.is_empty() {
            "/".to_owned()
        } else {
            parent.to_owned()
        }
    } else {
        path.trim_end_matches('/').to_owned()
    };
    if ancestor.is_empty() {
        ancestor = "/".into();
    }
    let canonical = loop {
        match remote_io(sftp.inner.canonicalize(&ancestor)).await {
            Ok(value) => break value,
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile =>
            {
                let (parent, name) = ancestor.rsplit_once('/').unwrap_or((".", &ancestor));
                if name.is_empty() || matches!(name, "." | "..") {
                    return Err(SessionError::Invalid("transfer parent unavailable"));
                }
                missing.push(name.to_owned());
                ancestor = if parent.is_empty() {
                    "/".into()
                } else {
                    parent.to_owned()
                };
            }
            Err(error) => return Err(sftp_error(error)),
        }
    };
    if !canonical.starts_with('/') || canonical.split('/').any(|part| part == "..") {
        return Err(SessionError::Invalid(
            "server returned an unsafe canonical transfer path",
        ));
    }
    let mut target = canonical.trim_end_matches('/').to_owned();
    for part in missing.iter().rev() {
        target.push('/');
        target.push_str(part);
    }
    if target.is_empty() {
        target.push('/');
    }
    let mut components: Vec<_> = canonical
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(str::to_lowercase)
        .collect();
    components.extend(missing.into_iter().rev().map(|s| s.to_lowercase()));
    Ok(Claim {
        path: components,
        write,
        whole_side,
        target: reservations::Target::Remote(target),
    })
}

async fn claims(sftp: &SftpSession, job: &TransferJob) -> Result<Claims> {
    let spec = job.spec();
    let (local, remote) = tokio::try_join!(
        local_claim(&spec.local, spec.direction == TransferDirection::Download),
        remote_claim(
            sftp,
            &spec.remote,
            spec.direction == TransferDirection::Upload,
            matches!(job, TransferJob::AtomicUpload(_))
                || matches!(job, TransferJob::ReviewedFile(plan) if plan.direction() == TransferDirection::Upload)
        ),
    )?;
    Ok(Claims { local, remote })
}

impl TransferQueue {
    /// Create a one-worker queue inside an active Tokio runtime.
    pub fn new(sftp: Arc<SftpSession>) -> Self {
        let (commands, receiver) = mpsc::channel(MAX_QUEUED_TRANSFERS);
        let (parallelism, limits) = watch::channel(1);
        let (stop, stopped) = watch::channel(false);
        let worker = tokio::spawn(schedule(sftp.clone(), receiver, limits, stopped));
        Self {
            commands,
            next_id: AtomicU64::new(1),
            admission: Arc::new(Semaphore::new(MAX_QUEUED_TRANSFERS)),
            parallelism,
            stop,
            worker: Some(worker),
            sftp,
        }
    }
    /// Set the concurrency bound (1–4). Lowering it does not interrupt jobs;
    /// new jobs wait until the number already admitted falls below the bound.
    pub fn set_parallelism(&self, parallelism: usize) -> Result<()> {
        if !(1..=MAX_PARALLEL_TRANSFERS).contains(&parallelism) {
            return Err(SessionError::Invalid(
                "transfer parallelism must be between 1 and 4",
            ));
        }
        self.parallelism.send_replace(parallelism);
        Ok(())
    }
    /// Current requested concurrency bound.
    pub fn parallelism(&self) -> usize {
        *self.parallelism.borrow()
    }
    /// Request cancellation of every admitted entry; no job is replayed.
    pub fn cancel_all(&self) {
        self.stop.send_replace(true);
        self.admission.close();
    }
    /// Cancel and await the owned scheduler. Terminal events report per-job
    /// outcomes; cancellation still cannot prove a pending remote WRITE was undone.
    pub async fn close(mut self) -> Result<()> {
        self.cancel_all();
        if let Some(worker) = self.worker.take() {
            worker.await.map_err(|_| SessionError::Worker)?;
        }
        Ok(())
    }
    /// Enqueue an exclusive new-directory transfer after review.
    pub async fn enqueue_directory(&self, plan: DirectoryTransferPlan) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::Directory(plan)).await
    }
    /// Enqueue a reviewed file continuation on its original connection.
    pub async fn enqueue_resume(&self, plan: FileResumePlan) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::Resume(plan)).await
    }
    /// Enqueue a reviewed directory continuation on its original connection.
    pub async fn enqueue_directory_resume(
        &self,
        plan: DirectoryResumePlan,
    ) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::DirectoryResume(plan)).await
    }
    /// Admit one file. A full queue fails immediately without writing.
    pub async fn enqueue(&self, spec: TransferSpec) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::File(spec)).await
    }
    /// Enqueue a regular upload through an exclusive temporary and atomic
    /// POSIX rename. Existing targets remain intact until publication.
    /// Unsupported servers fail closed; there is no in-place fallback.
    pub async fn enqueue_atomic_upload(&self, spec: TransferSpec) -> Result<TransferHandle> {
        if spec.direction != TransferDirection::Upload {
            return Err(SessionError::Invalid(
                "atomic upload requires upload direction",
            ));
        }
        self.enqueue_job(TransferJob::AtomicUpload(spec)).await
    }
    /// Enqueue an immutable reviewed file transfer on its captured SSH connection.
    /// Source and destination observations are checked again after queue wait and
    /// before mutation; changed objects fail without a replacement operation.
    pub async fn enqueue_reviewed_file(&self, plan: FileTransferPlan) -> Result<TransferHandle> {
        self.enqueue_job(TransferJob::ReviewedFile(plan)).await
    }
    async fn enqueue_job(&self, spec: TransferJob) -> Result<TransferHandle> {
        if self.sftp.is_closed() {
            return Err(SessionError::Closed);
        }
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| SessionError::Invalid("transfer queue is full or closed"))?;
        let claims = deadline(
            self.sftp.timeout,
            "transfer path reservation",
            claims(&self.sftp, &spec),
        )
        .await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let control = Arc::new(TransferControl::new());
        let (events, receiver) = mpsc::channel(32);
        let (terminal, terminal_receiver) = oneshot::channel();
        let _ = events.try_send(TransferEvent::Queued { id });
        self.commands
            .send(TransferCommand {
                id,
                spec,
                claims,
                events,
                terminal,
                control: control.clone(),
                _admission: permit,
            })
            .await
            .map_err(|_| SessionError::Worker)?;
        Ok(TransferHandle {
            id,
            events: receiver,
            terminal: Some(terminal_receiver),
            control,
        })
    }
}
impl Drop for TransferQueue {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

async fn schedule(
    sftp: Arc<SftpSession>,
    mut receiver: mpsc::Receiver<TransferCommand>,
    mut limits: watch::Receiver<usize>,
    mut stopped: watch::Receiver<bool>,
) {
    let mut pending = VecDeque::<TransferCommand>::new();
    let mut running = Vec::<(u64, Claims, Arc<TransferControl>)>::new();
    let mut tasks = JoinSet::new();
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    loop {
        let stopping = *stopped.borrow_and_update() || sftp.is_closed();
        if stopping {
            receiver.close();
            for (_, _, control) in &running {
                control.cancel();
            }
            while let Ok(command) = receiver.try_recv() {
                pending.push_back(command);
            }
        }
        let mut index = 0;
        while index < pending.len() {
            if stopping || *pending[index].control.cancelled.borrow() {
                if let Some(command) = pending.remove(index) {
                    let _ = command.terminal.send(TransferEvent::Cancelled {
                        id: command.id,
                        bytes: 0,
                    });
                }
                continue;
            }
            if running.len() >= *limits.borrow_and_update() {
                index += 1;
                continue;
            }
            let blocked = running
                .iter()
                .any(|(_, reserved, _)| reserved.conflicts(&pending[index].claims))
                || pending
                    .iter()
                    .take(index)
                    .any(|earlier| earlier.claims.conflicts(&pending[index].claims));
            if blocked {
                index += 1;
                continue;
            }
            let reservation = match TransferReservations::admit(
                &sftp._connection.transfer_reservations,
                &pending[index].claims,
            ) {
                Admission::Busy => {
                    index += 1;
                    continue;
                }
                Admission::Quarantined => {
                    if let Some(command) = pending.remove(index) {
                        let _ = command.terminal.send(TransferEvent::Failed { id: command.id, error: "destination conflicts with an unknown transfer result; inspect and explicitly acknowledge its risk before a new review; reconnect does not resolve it".into() });
                    }
                    continue;
                }
                Admission::Ready(reservation) => reservation,
            };
            if let Some(command) = pending.remove(index) {
                running.push((command.id, command.claims.clone(), command.control.clone()));
                let sftp = sftp.clone();
                tasks.spawn(async move {
                    let id = command.id;
                    let (known, terminal, event) =
                        execute(sftp, command, reservation.clone()).await;
                    reservation.finish(known);
                    let _ = terminal.send(event);
                    id
                });
            }
        }
        if stopping && tasks.is_empty() && pending.is_empty() {
            break;
        }
        tokio::select! {
            command = receiver.recv(), if !stopping => match command {
                Some(command) => pending.push_back(command),
                None => { stopped = watch::channel(true).1; }
            },
            result = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(Ok(id)) = result { running.retain(|(running_id, _, _)| *running_id != id); }
                else { for (_, _, control) in &running { control.cancel(); } stopped = watch::channel(true).1; }
            },
            _ = limits.changed(), if !stopping => {},
            _ = stopped.changed(), if !stopping => {},
            _ = tick.tick() => {},
        }
    }
}

async fn execute(
    sftp: Arc<SftpSession>,
    command: TransferCommand,
    reservation: Reservation,
) -> (bool, oneshot::Sender<TransferEvent>, TransferEvent) {
    let TransferCommand {
        id,
        spec,
        claims: reserved,
        events,
        terminal,
        control,
        _admission,
    } = command;
    let prepare = async {
        let now = claims(&sftp, &spec).await?;
        // A new destination may have been created by an earlier serialized
        // job. Its same canonical path is still reserved; expanding a claim
        // to a filesystem-wide lock must fail rather than race other jobs.
        if now != reserved {
            return Err(SessionError::Invalid(
                "transfer paths changed while queued; review again",
            ));
        }
        Ok(match &spec {
            TransferJob::ReviewedFile(plan) => {
                sftp.validate_file_transfer(plan).await?;
                Some(plan.bytes())
            }
            TransferJob::Directory(plan) => Some(plan.bytes()),
            TransferJob::DirectoryResume(plan) => Some(plan.bytes()),
            TransferJob::Resume(plan) => Some(plan.bytes()),
            TransferJob::File(spec) | TransferJob::AtomicUpload(spec) => match spec.direction {
                TransferDirection::Upload => local_io(tokio::fs::metadata(&spec.local))
                    .await
                    .ok()
                    .map(|m| m.len()),
                TransferDirection::Download => remote_io(sftp.inner.metadata(&spec.remote))
                    .await
                    .ok()
                    .and_then(|m| m.size),
            },
        })
    };
    let prepared = tokio::select! {
        biased;
        _ = control.cancelled() => Err(SessionError::Closed),
        result = tokio::time::timeout(sftp.timeout, prepare) => result.unwrap_or(Err(SessionError::Timeout("SFTP transfer preparation"))),
    };
    let total = match prepared {
        Ok(total) => total,
        Err(error) => {
            let event = if *control.cancelled.borrow() {
                TransferEvent::Cancelled { id, bytes: 0 }
            } else {
                TransferEvent::Failed {
                    id,
                    error: error.to_string(),
                }
            };
            return (true, terminal, event);
        }
    };
    let mut context = TransferContext::new(id, total, events, control);
    context.reservation = Some(reservation);
    let result = context
        .run(sftp.timeout, "SFTP transfer idle wait", async {
            context.event(TransferEvent::Started { id, total }).await?;
            context.checkpoint().await?;
            match spec {
                TransferJob::AtomicUpload(spec) => {
                    sftp.queued_atomic_upload(&spec.local, &spec.remote, &context)
                        .await
                }
                TransferJob::ReviewedFile(plan) => sftp.queued_reviewed_file(plan, &context).await,
                TransferJob::Directory(plan) => sftp.queued_directory(plan, &context).await,
                TransferJob::DirectoryResume(plan) => {
                    sftp.queued_directory_resume(&plan, &context).await
                }
                TransferJob::Resume(plan) => sftp.execute_file_resume(&plan, &context).await,
                TransferJob::File(spec) => match spec.direction {
                    TransferDirection::Upload => {
                        sftp.queued_upload(&spec.local, &spec.remote, &context)
                            .await
                    }
                    TransferDirection::Download => {
                        sftp.queued_download(&spec.remote, &spec.local, &context)
                            .await
                    }
                },
            }
        })
        .await;
    let known = !context.mutation_pending()
        && match &context.reservation {
            Some(ticket) => ticket.cleanup_known().await,
            None => false,
        };
    let outcome = terminal_transfer_event(id, context.bytes(), known, result);
    (known, terminal, outcome)
}

// Preserve the operation's own category. A later cancellation request must
// not relabel an acknowledged I/O/connection failure; unknown I/O stays unknown.
pub(super) fn terminal_transfer_event(
    id: u64,
    bytes: u64,
    known: bool,
    result: TransferExecutionResult<()>,
) -> TransferEvent {
    if !known {
        TransferEvent::Uncertain { id, bytes, error: "a destination mutation has no confirmed reply; the destination remains isolated across reconnects; inspect and explicitly acknowledge the unresolved risk before reviewing a conflicting transfer".into() }
    } else {
        match result {
            Ok(()) => TransferEvent::Completed { id, bytes },
            Err(TransferExecutionError::Cancelled(bytes)) => TransferEvent::Cancelled { id, bytes },
            Err(TransferExecutionError::Error(error)) => TransferEvent::Failed {
                id,
                error: error.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn claim(path: &[&str], write: bool) -> Claim {
        Claim {
            path: path.iter().map(|s| s.to_string()).collect(),
            write,
            whole_side: false,
            target: reservations::Target::Remote(format!("/{}", path.join("/"))),
        }
    }
    #[test]
    fn reservations_cover_descendants_but_allow_shared_sources_and_siblings() {
        assert!(overlap(
            &claim(&["root"], true),
            &claim(&["root", "child"], false)
        ));
        assert!(!overlap(
            &claim(&["root"], false),
            &claim(&["root", "child"], false)
        ));
        assert!(!overlap(
            &claim(&["root", "one"], true),
            &claim(&["root", "two"], true)
        ));
        let mut existing = claim(&["alias"], true);
        existing.whole_side = true;
        assert!(overlap(&existing, &claim(&["another-name"], false)));
    }
    #[tokio::test]
    async fn local_aliases_and_case_variants_reserve_the_same_target() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let a = local_claim(&temp.path().join("New"), true).await?;
        let b = local_claim(&temp.path().join("new"), true).await?;
        assert!(overlap(&a, &b));
        let relative = temp.path().join(".").join("New");
        assert_eq!(local_claim(&relative, true).await?, a);
        #[cfg(unix)]
        {
            let actual = temp.path().join("actual");
            let alias = temp.path().join("alias");
            tokio::fs::create_dir(&actual).await?;
            tokio::fs::symlink(&actual, &alias).await?;
            assert_eq!(
                local_claim(&actual.join("missing"), true).await?,
                local_claim(&alias.join("missing"), true).await?
            );
        }
        assert!(
            local_claim(&temp.path().join("../escape"), true)
                .await
                .is_err()
        );
        Ok(())
    }
}
