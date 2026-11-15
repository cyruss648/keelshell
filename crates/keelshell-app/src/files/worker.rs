//! Session-bound SFTP worker and acknowledged transfer control.
use super::*;
use std::path::{Path, PathBuf};

const MAX_COMPARE_DEPTH: usize = 32;

/// Read a bounded local tree for the read-only directory comparison action.
///
/// The worker already runs on a dedicated transport thread, so filesystem I/O
/// never blocks the GPUI event loop. Symlinks are represented as entries and
/// are never traversed. Relative paths are built from directory-entry names so
/// the local root cannot escape into a comparison key.
pub(super) fn snapshot_local_tree(
    root: &Path,
    stop: &AtomicBool,
) -> Result<Vec<DirectoryEntrySnapshot>, FileFailure> {
    if !root.is_absolute() {
        return Err(FileFailure::Comparison(
            "local comparison root must be absolute".to_owned(),
        ));
    }
    let snapshot = crate::directory_compare::snapshot_local_directory(
        root,
        keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES,
        MAX_COMPARE_DEPTH,
    )
    .map_err(|error| FileFailure::Comparison(error.to_string()))?;
    if stop.load(Ordering::Acquire) {
        return Err(FileFailure::Cancelled);
    }
    Ok(snapshot)
}

pub(super) fn remote_relative_path(root: &str, path: &str) -> Option<String> {
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        path.strip_prefix('/')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
    } else {
        path.strip_prefix(root)
            .and_then(|path| path.strip_prefix('/'))
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
    }
}

async fn compare_directories(
    sftp: Arc<SftpSession>,
    local_root: PathBuf,
    remote_root: String,
    stop: &AtomicBool,
) -> Result<Outcome, FileFailure> {
    let local = snapshot_local_tree(&local_root, stop)?;
    let remote_root = sftp.canonicalize(&remote_root).await?;
    let remote_entries = sftp
        .snapshot_tree_limited(
            &remote_root,
            keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES,
            MAX_COMPARE_DEPTH,
        )
        .await?;
    if stop.load(Ordering::Acquire) {
        return Err(FileFailure::Cancelled);
    }
    let mut remote = Vec::with_capacity(remote_entries.len());
    for entry in remote_entries {
        let Some(path) = remote_relative_path(&remote_root, &entry.path) else {
            return Err(FileFailure::Comparison(format!(
                "remote snapshot entry is outside the selected root: {}",
                entry.path
            )));
        };
        let kind = if entry.is_symlink {
            DirectoryEntryKind::Symlink
        } else if entry.is_directory {
            DirectoryEntryKind::Directory
        } else {
            DirectoryEntryKind::File
        };
        remote.push(DirectoryEntrySnapshot::new(
            path,
            kind,
            (kind == DirectoryEntryKind::File)
                .then_some(entry.size)
                .flatten(),
            entry.modified.map(u64::from),
        ));
    }
    let report = keelshell_core::compare_directories(&local, &remote)
        .map_err(|error| FileFailure::Comparison(error.to_string()))?;
    Ok(Outcome::Compared(DirectoryComparison {
        local: local_root,
        remote: remote_root,
        report,
        sync_plan: None,
    }))
}

async fn queued_transfer(
    sftp: Arc<SftpSession>,
    spec: TransferSpec,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> Result<Outcome, FileFailure> {
    if stop.load(Ordering::Acquire) {
        return Err(FileFailure::Cancelled);
    }
    let direction = spec.direction;
    let queue = sftp.transfer_queue();
    let mut transfer = queue.enqueue(spec).await.map_err(FileFailure::from)?;
    observe_transfer(&mut transfer, direction, None, stop, pause, progress, None).await
}

pub(super) async fn observe_transfer(
    transfer: &mut keelshell_session::sftp::TransferHandle,
    direction: TransferDirection,
    existing: Option<(u64, bool)>,
    stop: Arc<AtomicBool>,
    mut pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
    recovery: Option<&mut TransferRecoveryWriter>,
) -> Result<Outcome, FileFailure> {
    let mut recovery = recovery;
    let mut last_bytes = existing.map_or(0, |(bytes, _)| bytes);
    let mut last_total = None;
    #[cfg(test)]
    let pause_handoff = super::test_server::pause_handoff_fixture::before_forward(&pause).await;
    if *pause.borrow_and_update() {
        transfer.pause();
        #[cfg(test)]
        if let Some(handoff) = pause_handoff {
            handoff.forwarded();
        }
    }
    loop {
        tokio::select! {
            biased;
            _ = cancellation(&stop) => {
                transfer.cancel();
                let (result, bytes, total) = tokio::time::timeout(
                    Duration::from_secs(6),
                    finish_cancelled_transfer(transfer, progress),
                )
                .await
                .unwrap_or((
                    Err(FileFailure::CancellationUnconfirmed),
                    last_bytes,
                    last_total,
                ));
                recovery_finish(&mut recovery, &result, bytes, total);
                return result;
            }
            changed = pause.changed() => {
                if changed.is_err() {
                    transfer.cancel();
                    let (result, bytes, total) = tokio::time::timeout(
                        Duration::from_secs(6),
                        finish_cancelled_transfer(transfer, progress),
                    )
                    .await
                    .unwrap_or((
                        Err(FileFailure::CancellationUnconfirmed),
                        last_bytes,
                        last_total,
                    ));
                    recovery_finish(&mut recovery, &result, bytes, total);
                    return result;
                }
                #[cfg(test)]
                let pause_handoff = super::test_server::pause_handoff_fixture::before_forward(&pause).await;
                if *pause.borrow_and_update() {
                    transfer.pause();
                    #[cfg(test)]
                    if let Some(handoff) = pause_handoff {
                        handoff.forwarded();
                    }
                } else { transfer.resume(); }
            }
            event = transfer.recv() => {
                match event {
                    Some(TransferEvent::Queued { .. }) => { let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Queued)); }
                    Some(TransferEvent::Started { total, .. }) => {
                        last_total = total;
                        recovery_checkpoint(&mut recovery, last_bytes, total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Started(total)));
                        if existing.is_none() { send_worker_progress(progress, transfer_progress_message(direction, 0, total)); }
                    }
                    Some(TransferEvent::Progress { transferred, total, .. }) => {
                        last_bytes = transferred;
                        last_total = total;
                        recovery_checkpoint(&mut recovery, transferred, total, progress);
                        let _ = progress.try_send(WorkerMessage::Transfer(TransferUpdate::Progress(transferred,total)));
                        send_worker_progress(progress, existing.map_or_else(|| transfer_progress_message(direction,transferred,total), |(existing,directory)| resume_progress_message(transferred,total,existing,directory)));
                    }
                    Some(TransferEvent::Paused { transferred, total, .. }) => {
                        last_bytes = transferred;
                        last_total = total;
                        recovery_checkpoint(&mut recovery, transferred, total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Paused(transferred,total)));
                    }
                    Some(TransferEvent::Resumed { transferred, total, .. }) => {
                        last_bytes = transferred;
                        last_total = total;
                        recovery_checkpoint(&mut recovery, transferred, total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Resumed(transferred,total)));
                    }
                    Some(TransferEvent::Completed { bytes, .. }) => {
                        recovery_checkpoint(&mut recovery, bytes, last_total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                        let result = if let Some((existing,directory)) = existing {
                            if directory {
                                Ok(Outcome::Done(Message::new(format!("目录续传完成：{bytes} 字节（含原有部分）；请刷新列表"),format!("Folder continuation complete: {bytes} bytes including existing content; refresh to reload"))))
                            } else {
                                Ok(Outcome::Done(Message::new(
                                    format!("续传完成：目标 {bytes} 字节，本次新增 {} 字节；请刷新列表",bytes.saturating_sub(existing)),
                                    format!("Continuation complete: {bytes} destination bytes, {} new bytes; refresh to reload",bytes.saturating_sub(existing)),
                                )))
                            }
                        } else {
                            let (zh, en) = match direction {
                                TransferDirection::Upload => (
                                    format!("已通过传输队列上传 {bytes} 字节，请刷新列表"),
                                    format!("Uploaded {bytes} bytes through the transfer queue; refresh to reload"),
                                ),
                                TransferDirection::Download => (
                                    format!("已通过传输队列下载 {bytes} 字节"),
                                    format!("Downloaded {bytes} bytes through the transfer queue"),
                                ),
                            };
                            Ok(Outcome::Done(Message::new(zh, en)))
                        };
                        recovery_finish(&mut recovery, &result, bytes, last_total);
                        return result;
                    }
                    Some(TransferEvent::Cancelled { bytes, .. }) => {
                        recovery_checkpoint(&mut recovery, bytes, last_total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                        let result = Err(FileFailure::Cancelled);
                        recovery_finish(&mut recovery, &result, bytes, last_total);
                        return result;
                    }
                    Some(TransferEvent::Uncertain { bytes, error, .. }) => {
                        recovery_checkpoint(&mut recovery, bytes, last_total, progress);
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                        let result = Err(FileFailure::OutcomeUncertain(error));
                        recovery_finish(&mut recovery, &result, bytes, last_total);
                        return result;
                    }
                    Some(TransferEvent::Failed { error, .. }) => {
                        let result = Err(FileFailure::Transport(error));
                        recovery_finish(&mut recovery, &result, last_bytes, last_total);
                        return result;
                    }
                    None => {
                        let result = Err(FileFailure::WorkerStopped);
                        recovery_finish(&mut recovery, &result, last_bytes, last_total);
                        return result;
                    }
                }
            }
        }
    }
}

fn recovery_checkpoint(
    recovery: &mut Option<&mut TransferRecoveryWriter>,
    bytes: u64,
    total: Option<u64>,
    progress: &mpsc::SyncSender<WorkerMessage>,
) {
    if let Some(writer) = recovery.as_deref_mut() {
        writer.checkpoint(bytes, total);
        if let Some(error) = writer.take_error() {
            send_worker_progress(
                progress,
                Message::new(
                    format!("传输恢复记录暂时无法保存：{error}"),
                    format!("Transfer recovery metadata could not be saved: {error}"),
                ),
            );
        }
    }
}

fn recovery_finish(
    recovery: &mut Option<&mut TransferRecoveryWriter>,
    result: &Result<Outcome, FileFailure>,
    bytes: u64,
    total: Option<u64>,
) {
    if let Some(writer) = recovery.as_deref_mut() {
        writer.finish(result, bytes, total);
    }
}

async fn finish_cancelled_transfer(
    transfer: &mut keelshell_session::sftp::TransferHandle,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> (Result<Outcome, FileFailure>, u64, Option<u64>) {
    let mut last_bytes = 0;
    let mut last_total = None;
    while let Some(event) = transfer.recv().await {
        match event {
            TransferEvent::Completed { bytes, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return (
                    Ok(Outcome::Done(Message::new(
                        format!("传输在取消请求前已完成 {bytes} 字节"),
                        format!("Transfer completed {bytes} bytes before cancellation took effect"),
                    ))),
                    bytes,
                    last_total,
                );
            }
            TransferEvent::Cancelled { bytes, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return (Err(FileFailure::Cancelled), bytes, last_total);
            }
            TransferEvent::Uncertain { bytes, error, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return (Err(FileFailure::OutcomeUncertain(error)), bytes, last_total);
            }
            TransferEvent::Failed { error, .. } => {
                return (Err(FileFailure::Transport(error)), last_bytes, last_total);
            }
            TransferEvent::Queued { .. } => {}
            TransferEvent::Started { total, .. } => last_total = total,
            TransferEvent::Progress {
                transferred, total, ..
            }
            | TransferEvent::Paused {
                transferred, total, ..
            }
            | TransferEvent::Resumed {
                transferred, total, ..
            } => {
                last_bytes = transferred;
                last_total = total;
            }
        }
    }
    (Err(FileFailure::WorkerStopped), last_bytes, last_total)
}

pub(super) async fn operate(
    session: SshSession,
    operation: Operation,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
    recovery_context: Option<(Arc<TransferRecoveryStore>, TransferRecoveryClaim)>,
) -> Result<Outcome, FileFailure> {
    let journal = if let Operation::ApplyDirectorySync(_, journal) = &operation {
        Some(journal.clone())
    } else {
        None
    };
    let result = operate_inner(
        session,
        operation,
        stop.clone(),
        pause,
        progress,
        journal.as_ref(),
        recovery_context,
    )
    .await;
    if let Some(journal) = journal
        && let Ok(mut journal) = journal.lock()
    {
        journal.finish(
            stop.load(Ordering::Acquire),
            matches!(&result, Err(FileFailure::Cleanup)),
        );
    }
    result
}

async fn operate_inner(
    session: SshSession,
    operation: Operation,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
    journal: Option<&super::sync::journal::SharedJournal>,
    recovery_context: Option<(Arc<TransferRecoveryStore>, TransferRecoveryClaim)>,
) -> Result<Outcome, FileFailure> {
    if let Operation::InspectArchiveTools(probe) = &operation {
        if !probe.matches_session(&session) {
            return Err(FileFailure::CancelledBeforeStart);
        }
        let report = tokio::select! {
            biased;
            _ = cancellation(&stop) => return Err(FileFailure::CancelledBeforeStart),
            result = session.inspect_archive_tools(&stop) => result?,
        };
        return Ok(Outcome::ArchiveTools(probe.clone(), report));
    }
    let sftp = tokio::select! {
        biased;
        _ = cancellation(&stop) => return Err(FileFailure::CancelledBeforeStart),
        result = session.sftp() => Arc::new(result.map_err(FileFailure::from)?),
    };
    let transfer = matches!(
        &operation,
        Operation::Upload(..)
            | Operation::Download(..)
            | Operation::TransferDirectory(..)
            | Operation::TransferFile(..)
            | Operation::ResumeFile(..)
            | Operation::ResumeDirectory(..)
            | Operation::ResumePersistedFile(..)
            | Operation::ResumePersistedDirectory(..)
            | Operation::TransferArchive(_)
    );
    let action = async {
        match operation {
            Operation::InspectFileQuarantine(target) => {
                Ok(Outcome::QuarantineInspected(match target {
                    IsolationTarget::Remote(path) => {
                        sftp.inspect_remote_mutation_quarantine(&path).await?
                    }
                    IsolationTarget::Local(path) => {
                        sftp.inspect_local_mutation_quarantine(&path).await?
                    }
                }))
            }
            Operation::InspectQuarantine(spec) => Ok(Outcome::QuarantineInspected(
                sftp.inspect_transfer_quarantine(&spec).await?,
            )),
            Operation::AcknowledgeQuarantine(review) => {
                sftp.acknowledge_transfer_quarantine(&review, &stop).await?;
                Ok(Outcome::Done(Message::new(
                    "已解除审核目标的应用隔离；原任务仍为结果未知。新的传输必须单独审核。",
                    "Released application isolation for the reviewed targets; previous jobs remain unknown. Review any new transfer separately.",
                )))
            }
            Operation::List(path) => {
                let canonical = sftp.canonicalize(&path).await?;
                let mut entries = sftp.list(&canonical).await?;
                entries.sort_by(|a, b| {
                    b.is_directory
                        .cmp(&a.is_directory)
                        .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                Ok(Outcome::Listed(canonical, entries))
            }
            Operation::Read(entry) => {
                let snapshot = sftp.read_regular_snapshot(&entry.path, 1024 * 1024).await?;
                let document =
                    TextDocument::decode(&snapshot.content).map_err(FileFailure::TextDocument)?;
                Ok(Outcome::Read(snapshot, document))
            }
            Operation::ReadMerge {
                path,
                base,
                format,
                draft,
            } => {
                let snapshot = sftp.read_regular_snapshot(&path, 1024 * 1024).await?;
                merge_outcome(snapshot, base, format, draft)
            }
            Operation::ApplyPatchToDraft { path, draft, patch } => {
                let result = keelshell_core::apply_text_patch(&path, &draft, &patch)
                    .map_err(FileFailure::TextEdit)?;
                Ok(Outcome::PatchApplied(path, draft, result))
            }
            Operation::Mkdir(path) => {
                sftp.mkdir(&path).await?;
                Ok(Outcome::Done(Message::new(
                    "目录已创建，请刷新列表",
                    "Directory created; refresh to reload",
                )))
            }
            Operation::Rename(from, to) => {
                sftp.rename(&from, &to).await?;
                Ok(Outcome::Done(Message::new(
                    "已重命名，请刷新列表",
                    "Renamed; refresh to reload",
                )))
            }
            Operation::Delete(entry) => {
                if entry.is_directory {
                    sftp.rmdir(&entry.path).await?;
                } else {
                    sftp.remove(&entry.path).await?;
                }
                Ok(Outcome::Done(Message::new(
                    "已删除，请刷新列表",
                    "Deleted; refresh to reload",
                )))
            }
            Operation::SetPermissions(entry, mode) => {
                if entry.is_symlink {
                    return Err(FileFailure::Symlink);
                }
                let _updated = sftp.set_permissions_reviewed(&entry, mode).await?;
                Ok(Outcome::Done(Message::new(
                    format!("已将 {} 的权限改为 {:04o}，请刷新列表", entry.path, mode),
                    format!(
                        "Changed {} permissions to {:04o}; refresh to reload",
                        entry.path, mode
                    ),
                )))
            }
            Operation::Upload(local, remote) => {
                queued_transfer(
                    sftp.clone(),
                    TransferSpec::upload(local, remote),
                    stop.clone(),
                    pause,
                    progress,
                )
                .await
            }
            Operation::Download(remote, local) => {
                queued_transfer(
                    sftp.clone(),
                    TransferSpec::download(remote, local),
                    stop.clone(),
                    pause,
                    progress,
                )
                .await
            }
            Operation::PlanBatch(request) => Ok(Outcome::PlannedBatch(Box::new(
                batch::prepare(&sftp, request, &stop).await?,
            ))),
            Operation::PlanArchive(request) => Ok(Outcome::PlannedArchive(Box::new(
                archive::prepare(&sftp, request, &stop).await?,
            ))),
            Operation::TransferArchive(plan) => {
                let receipt = sftp
                    .execute_archive_transfer(plan.transfer, &|| !stop.load(Ordering::Acquire))
                    .await?;
                Ok(Outcome::Done(Message::new(
                    format!(
                        "归档传输完成：{} 项，归档 {} 字节，展开 {} 字节。请刷新目标列表。",
                        receipt.member_count, receipt.archive_bytes, receipt.expanded_bytes
                    ),
                    format!(
                        "Archive transfer completed: {} members, {} archive bytes, {} expanded bytes. Refresh the destination list.",
                        receipt.member_count, receipt.archive_bytes, receipt.expanded_bytes
                    ),
                )))
            }
            Operation::TransferBatch(_) => Err(FileFailure::WorkerStopped),
            Operation::InspectArchiveTools(_) => Err(FileFailure::WorkerStopped),
            Operation::DiscardPersistedRecoveryBatch(_) => Err(FileFailure::WorkerStopped),
            Operation::TransferFile(plan) => {
                let direction = plan.direction();
                let queue = sftp.clone().transfer_queue();
                let mut transfer = queue.enqueue_reviewed_file(plan).await?;
                observe_transfer(
                    &mut transfer,
                    direction,
                    None,
                    stop.clone(),
                    pause,
                    progress,
                    None,
                )
                .await
            }
            Operation::PlanDirectory(spec) => {
                let plan = sftp.plan_directory_transfer(spec).await?;
                Ok(Outcome::PlannedDirectory(plan))
            }
            Operation::Compare(local, remote) => {
                compare_directories(sftp.clone(), local, remote, &stop).await
            }
            Operation::PlanDirectorySync(local, remote, direction, policy) => {
                let comparison = tokio::time::timeout(
                    Duration::from_secs(30),
                    super::sync::plan(&sftp, local, remote, direction, policy, &stop),
                )
                .await
                .map_err(|_| {
                    FileFailure::Comparison("content review exceeded 30 seconds".into())
                })??;
                Ok(Outcome::DirectorySyncPlanned(comparison))
            }
            Operation::ApplyDirectorySync(review, journal) => {
                let destination = if review.plan.direction() == DirectorySyncDirection::LeftToRight
                {
                    review.remote.clone()
                } else {
                    review.local.display().to_string()
                };
                tokio::time::timeout(
                    Duration::from_secs(15 * 60),
                    super::sync::apply(&sftp, *review, &stop, progress, &journal),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(FileFailure::Comparison(
                        "directory synchronization exceeded 15 minutes".into(),
                    ))
                })
                .map_err(|error| FileFailure::DirectoryTransfer {
                    destination,
                    error: Box::new(error),
                })
            }
            Operation::PlanResume(spec, directory) => {
                if directory {
                    Ok(Outcome::PlannedDirectoryResume(
                        sftp.plan_directory_resume(spec).await?,
                    ))
                } else {
                    Ok(Outcome::PlannedFileResume(
                        sftp.plan_file_resume(spec).await?,
                    ))
                }
            }
            Operation::PlanPersistedRecovery {
                id,
                spec,
                directory,
            } => {
                let Some((recovery_store, recovery_claim)) = recovery_context else {
                    return Err(FileFailure::RecoveryUnavailable(
                        "the recovery claim was not retained for inspection".to_owned(),
                    ));
                };
                if directory {
                    let plan = sftp.plan_directory_resume(spec).await?;
                    let mut records = recovery_store
                        .load()
                        .map_err(|error| FileFailure::RecoveryUnavailable(error.to_string()))?;
                    let mut record = records
                        .drain(..)
                        .find(|record| record.id == id)
                        .ok_or_else(|| {
                            FileFailure::RecoveryUnavailable(
                                "the claimed recovery record disappeared".to_owned(),
                            )
                        })?;
                    super::update_persisted_recovery_progress(
                        &mut record,
                        plan.existing_bytes(),
                        plan.bytes(),
                    )
                    .map_err(FileFailure::RecoveryUnavailable)?;
                    record.updated_at = super::current_unix_timestamp().max(record.created_at);
                    recovery_store
                        .upsert_with_claim(&recovery_claim, record)
                        .map_err(|error| FileFailure::RecoveryUnavailable(error.to_string()))?;
                    Ok(Outcome::PlannedPersistedDirectoryResume {
                        id,
                        plan,
                        recovery_claim,
                    })
                } else {
                    let plan = sftp.plan_file_resume(spec).await?;
                    let mut records = recovery_store
                        .load()
                        .map_err(|error| FileFailure::RecoveryUnavailable(error.to_string()))?;
                    let mut record = records
                        .drain(..)
                        .find(|record| record.id == id)
                        .ok_or_else(|| {
                            FileFailure::RecoveryUnavailable(
                                "the claimed recovery record disappeared".to_owned(),
                            )
                        })?;
                    super::update_persisted_recovery_progress(
                        &mut record,
                        plan.existing_bytes(),
                        plan.bytes(),
                    )
                    .map_err(FileFailure::RecoveryUnavailable)?;
                    record.updated_at = super::current_unix_timestamp().max(record.created_at);
                    recovery_store
                        .upsert_with_claim(&recovery_claim, record)
                        .map_err(|error| FileFailure::RecoveryUnavailable(error.to_string()))?;
                    Ok(Outcome::PlannedPersistedFileResume {
                        id,
                        plan,
                        recovery_claim,
                    })
                }
            }
            Operation::ResumeFile(plan) => {
                let direction = plan.direction();
                let existing = plan.existing_bytes();
                let queue = sftp.clone().transfer_queue();
                let mut transfer = queue
                    .enqueue_resume(plan)
                    .await
                    .map_err(FileFailure::from)?;
                observe_transfer(
                    &mut transfer,
                    direction,
                    Some((existing, false)),
                    stop.clone(),
                    pause,
                    progress,
                    None,
                )
                .await
            }
            Operation::ResumePersistedFile(_, plan) => {
                let direction = plan.direction();
                let existing = plan.existing_bytes();
                let queue = sftp.clone().transfer_queue();
                let mut transfer = queue
                    .enqueue_resume(plan)
                    .await
                    .map_err(FileFailure::from)?;
                observe_transfer(
                    &mut transfer,
                    direction,
                    Some((existing, false)),
                    stop.clone(),
                    pause,
                    progress,
                    None,
                )
                .await
            }
            Operation::ResumeDirectory(plan) => {
                let direction = plan.direction();
                let existing = plan.existing_bytes();
                let queue = sftp.clone().transfer_queue();
                let mut transfer = queue
                    .enqueue_directory_resume(plan)
                    .await
                    .map_err(FileFailure::from)?;
                observe_transfer(
                    &mut transfer,
                    direction,
                    Some((existing, true)),
                    stop.clone(),
                    pause,
                    progress,
                    None,
                )
                .await
            }
            Operation::ResumePersistedDirectory(_, plan) => {
                let direction = plan.direction();
                let existing = plan.existing_bytes();
                let queue = sftp.clone().transfer_queue();
                let mut transfer = queue
                    .enqueue_directory_resume(plan)
                    .await
                    .map_err(FileFailure::from)?;
                observe_transfer(
                    &mut transfer,
                    direction,
                    Some((existing, true)),
                    stop.clone(),
                    pause,
                    progress,
                    None,
                )
                .await
            }
            Operation::TransferDirectory(plan) => {
                let direction = plan.direction();
                let destination = match direction {
                    TransferDirection::Upload => plan.remote_path().to_owned(),
                    TransferDirection::Download => plan.local_path().display().to_string(),
                };
                let queue = sftp.clone().transfer_queue();
                let result = async {
                    let mut transfer = queue
                        .enqueue_directory(plan)
                        .await
                        .map_err(FileFailure::from)?;
                    observe_transfer(
                        &mut transfer,
                        direction,
                        None,
                        stop.clone(),
                        pause,
                        progress,
                        None,
                    )
                    .await
                }
                .await;
                match result {
                    Ok(_) => Ok(Outcome::Done(Message::new(
                        format!("目录传输已完成：{destination}；请刷新列表"),
                        format!("Directory transfer completed: {destination}; refresh to reload"),
                    ))),
                    Err(error) => Err(FileFailure::DirectoryTransfer {
                        destination,
                        error: Box::new(error),
                    }),
                }
            }
            Operation::Save {
                path,
                reviewed,
                format,
                content,
            } => {
                if path != reviewed.entry.path {
                    return Err(FileFailure::InvalidEditor);
                }
                let current = sftp.read_regular_snapshot(&path, 1024 * 1024).await?;
                if current.content != reviewed.content
                    || current.entry.size != reviewed.entry.size
                    || current.entry.permissions != reviewed.entry.permissions
                    || current.entry.modified != reviewed.entry.modified
                {
                    // The reviewed format is authoritative here. In a
                    // BOM-less UTF-8 document, leading U+FEFF bytes are
                    // editable content rather than a newly observed BOM.
                    let draft = format
                        .decode_body(&content)
                        .map_err(FileFailure::TextDocument)?;
                    return merge_outcome(current, reviewed.content, format, draft);
                }
                let backup = sftp
                    .write_editor_reviewed_authorized(&reviewed, &content, &|| {
                        !stop.load(Ordering::Acquire)
                    })
                    .await?;
                let saved = sftp
                    .read_regular_snapshot(&path, 1024 * 1024)
                    .await
                    .map_err(|error| FileFailure::SavedReadback(error.to_string()))?;
                if saved.content != content {
                    return Err(FileFailure::SavedReadback("saved contents differ from the acknowledged review; inspect the remote file".into()));
                }
                let observed = TextDocument::decode(&saved.content)
                    .map_err(|error| FileFailure::SavedReadback(error.to_string()))?;
                let format = format.with_line_ending(observed.format().line_ending());
                let body = format
                    .decode_body(&saved.content)
                    .map_err(|error| FileFailure::SavedReadback(error.to_string()))?;
                Ok(Outcome::Saved {
                    snapshot: saved,
                    backup,
                    format,
                    body,
                })
            }
        }
    };
    // Dropping a cancelled staged write cannot truncate the original target. A
    // rename already accepted by the server may still commit; never replay it.
    let outcome = if transfer {
        // queued_transfer owns cancellation of its TransferHandle and drains
        // the queue's terminal event before returning. Dropping the action at
        // the outer select would otherwise lose the remote transfer outcome.
        action.await
    } else {
        tokio::select! {
            biased;
            _ = cancellation(&stop) => Err(FileFailure::Cancelled),
            result = action => result,
        }
    };
    // SFTP close drains staged-file cleanup (2 s), then protocol close (2 s).
    let closed = tokio::time::timeout(Duration::from_secs(5), sftp.close()).await;
    if !matches!(&closed, Ok(Ok(())))
        && let Some(journal) = journal
        && let Ok(mut journal) = journal.lock()
    {
        journal.cleanup_failed = true;
    }
    match outcome {
        Ok(value) => match closed {
            Ok(Ok(())) => Ok(value),
            _ => Err(FileFailure::Cleanup),
        },
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{FileFailure, remote_relative_path, snapshot_local_tree};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn local_snapshot_adapter_is_bounded_and_honors_cancellation() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("temp root: {error}"));
        fs::create_dir(root.path().join("nested"))
            .unwrap_or_else(|error| panic!("nested: {error}"));
        fs::write(root.path().join("nested").join("file.txt"), b"data")
            .unwrap_or_else(|error| panic!("file: {error}"));

        let stop = AtomicBool::new(false);
        let snapshot = snapshot_local_tree(root.path(), &stop)
            .unwrap_or_else(|error| panic!("snapshot: {error}"));
        assert_eq!(
            snapshot
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["nested", "nested/file.txt"]
        );

        stop.store(true, Ordering::Release);
        assert!(matches!(
            snapshot_local_tree(root.path(), &stop),
            Err(FileFailure::Cancelled)
        ));
    }

    #[test]
    fn remote_snapshot_paths_are_relative_only_inside_the_selected_root() {
        assert_eq!(
            remote_relative_path("/", "/var/log/app.log"),
            Some("var/log/app.log".to_owned())
        );
        assert_eq!(
            remote_relative_path("/srv/app", "/srv/app/config.toml"),
            Some("config.toml".to_owned())
        );
        assert_eq!(
            remote_relative_path("/srv/app", "/srv/app-old/config.toml"),
            None
        );
        assert_eq!(remote_relative_path("/srv/app", "/srv/app"), None);
    }
}

#[cfg(test)]
mod future_size_tests {
    use super::{
        Arc, AtomicBool, Operation, SshSession, TransferRecoveryClaim, TransferRecoveryStore,
        WorkerMessage, mpsc, operate,
    };
    #[test]
    fn file_worker_keeps_directory_mirror_state_within_its_stack_budget() {
        fn size<F>(
            _: impl FnOnce(
                SshSession,
                Operation,
                Arc<AtomicBool>,
                tokio::sync::watch::Receiver<bool>,
                &'static mpsc::SyncSender<WorkerMessage>,
                Option<(Arc<TransferRecoveryStore>, TransferRecoveryClaim)>,
            ) -> F,
        ) -> usize {
            std::mem::size_of::<F>()
        }
        let bytes = size(operate);
        eprintln!("production file worker future bytes: {bytes}");
        assert!(bytes < 64 * 1024, "file worker embeds {bytes} state bytes");
    }
}

fn merge_outcome(
    snapshot: RegularFileSnapshot,
    base: Vec<u8>,
    format: TextFormat,
    draft: String,
) -> Result<Outcome, FileFailure> {
    let original = format
        .decode_body(&base)
        .map_err(FileFailure::TextDocument)?;
    let remote_document =
        TextDocument::decode(&snapshot.content).map_err(FileFailure::TextDocument)?;
    if !format.is_compatible_with(remote_document.format()) {
        return Err(FileFailure::EditorFormatChanged);
    }
    let plan = keelshell_core::merge_text(&original, &draft, remote_document.body())
        .map_err(FileFailure::TextEdit)?;
    Ok(Outcome::MergeLoaded {
        snapshot,
        plan,
        base,
        draft,
    })
}
