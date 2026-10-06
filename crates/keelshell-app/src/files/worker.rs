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
    observe_transfer(&mut transfer, direction, None, stop, pause, progress).await
}

pub(super) async fn observe_transfer(
    transfer: &mut keelshell_session::sftp::TransferHandle,
    direction: TransferDirection,
    existing: Option<(u64, bool)>,
    stop: Arc<AtomicBool>,
    mut pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> Result<Outcome, FileFailure> {
    if *pause.borrow_and_update() {
        transfer.pause();
    }
    loop {
        tokio::select! {
            biased;
            _ = cancellation(&stop) => {
                transfer.cancel();
                return tokio::time::timeout(Duration::from_secs(6), finish_cancelled_transfer(transfer,progress))
                    .await.unwrap_or(Err(FileFailure::CancellationUnconfirmed));
            }
            changed = pause.changed() => {
                if changed.is_err() {
                    transfer.cancel();
                    return tokio::time::timeout(Duration::from_secs(6), finish_cancelled_transfer(transfer, progress))
                        .await.unwrap_or(Err(FileFailure::CancellationUnconfirmed));
                }
                if *pause.borrow_and_update() { transfer.pause(); } else { transfer.resume(); }
            }
            event = transfer.recv() => {
                match event {
                    Some(TransferEvent::Queued { .. }) => { let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Queued)); }
                    Some(TransferEvent::Started { total, .. }) => {
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Started(total)));
                        if existing.is_none() { send_worker_progress(progress, transfer_progress_message(direction, 0, total)); }
                    }
                    Some(TransferEvent::Progress { transferred, total, .. }) => {
                        let _ = progress.try_send(WorkerMessage::Transfer(TransferUpdate::Progress(transferred,total)));
                        send_worker_progress(progress, existing.map_or_else(|| transfer_progress_message(direction,transferred,total), |(existing,directory)| resume_progress_message(transferred,total,existing,directory)));
                    }
                    Some(TransferEvent::Paused { transferred, total, .. }) => {
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Paused(transferred,total)));
                    }
                    Some(TransferEvent::Resumed { transferred, total, .. }) => {
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Resumed(transferred,total)));
                    }
                    Some(TransferEvent::Completed { bytes, .. }) => {
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                        if let Some((existing,directory)) = existing {
                            if directory { return Ok(Outcome::Done(Message::new(format!("目录续传完成：{bytes} 字节（含原有部分）；请刷新列表"),format!("Folder continuation complete: {bytes} bytes including existing content; refresh to reload")))); }
                            return Ok(Outcome::Done(Message::new(
                                format!("续传完成：目标 {bytes} 字节，本次新增 {} 字节；请刷新列表",bytes.saturating_sub(existing)),
                                format!("Continuation complete: {bytes} destination bytes, {} new bytes; refresh to reload",bytes.saturating_sub(existing)),
                            )));
                        }
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
                        return Ok(Outcome::Done(Message::new(zh, en)));
                    }
                    Some(TransferEvent::Cancelled { bytes, .. }) => {
                        let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                        return Err(FileFailure::Cancelled);
                    }
                    Some(TransferEvent::Uncertain { bytes, error, .. }) => { let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes))); return Err(FileFailure::OutcomeUncertain(error)); }
                    Some(TransferEvent::Failed { error, .. }) => return Err(FileFailure::Transport(error)),
                    None => return Err(FileFailure::WorkerStopped),
                }
            }
        }
    }
}

async fn finish_cancelled_transfer(
    transfer: &mut keelshell_session::sftp::TransferHandle,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> Result<Outcome, FileFailure> {
    while let Some(event) = transfer.recv().await {
        match event {
            TransferEvent::Completed { bytes, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return Ok(Outcome::Done(Message::new(
                    format!("传输在取消请求前已完成 {bytes} 字节"),
                    format!("Transfer completed {bytes} bytes before cancellation took effect"),
                )));
            }
            TransferEvent::Cancelled { bytes, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return Err(FileFailure::Cancelled);
            }
            TransferEvent::Uncertain { bytes, error, .. } => {
                let _ = progress.send(WorkerMessage::Transfer(TransferUpdate::Finished(bytes)));
                return Err(FileFailure::OutcomeUncertain(error));
            }
            TransferEvent::Failed { error, .. } => return Err(FileFailure::Transport(error)),
            TransferEvent::Queued { .. }
            | TransferEvent::Started { .. }
            | TransferEvent::Progress { .. }
            | TransferEvent::Paused { .. }
            | TransferEvent::Resumed { .. } => {}
        }
    }
    Err(FileFailure::WorkerStopped)
}

pub(super) async fn operate(
    session: SshSession,
    operation: Operation,
    stop: Arc<AtomicBool>,
    pause: tokio::sync::watch::Receiver<bool>,
    progress: &mpsc::SyncSender<WorkerMessage>,
) -> Result<Outcome, FileFailure> {
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
            | Operation::ResumeFile(..)
            | Operation::ResumeDirectory(..)
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
                let content = sftp.read(&entry.path, 1024 * 1024).await?;
                if std::str::from_utf8(&content).is_err() {
                    return Err(FileFailure::InvalidEditor);
                }
                Ok(Outcome::Read(entry.path, content))
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
            Operation::ApplyDirectorySync(review) => {
                let destination = if review.plan.direction() == DirectorySyncDirection::LeftToRight
                {
                    review.remote.clone()
                } else {
                    review.local.display().to_string()
                };
                tokio::time::timeout(
                    Duration::from_secs(15 * 60),
                    super::sync::apply(&sftp, *review, &stop, progress),
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
                original,
                content,
            } => {
                let current = sftp.read(&path, 1024 * 1024).await?;
                if current != original {
                    return Err(FileFailure::Conflict);
                }
                sftp.write_atomic(&path, &content).await?;
                Ok(Outcome::Saved(path, content))
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
