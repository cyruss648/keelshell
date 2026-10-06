//! Session-bound directory merge: bounded review, revalidation, atomic files.
use super::*;
use keelshell_core::{DirectorySyncOperation, MAX_DIRECTORY_HASH_BYTES, hash_directory_content};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub(in crate::files) mod journal;

const MAX_HASH_TOTAL: u64 = keelshell_core::MAX_DIRECTORY_MIRROR_CONTENT_BYTES;
const REVIEW_BYTES: usize = 128 * 1024;

fn check_stop(stop: &AtomicBool) -> Result<(), FileFailure> {
    if stop.load(Ordering::Acquire) {
        Err(FileFailure::Cancelled)
    } else {
        Ok(())
    }
}

fn problem(detail: impl Into<String>) -> FileFailure {
    FileFailure::Comparison(detail.into())
}

/// Join portable relative components instead of interpreting Windows drive,
/// device, or alternate-data-stream syntax supplied by a remote filename.
fn local_child(root: &Path, relative: &str) -> Result<PathBuf, FileFailure> {
    let mut path = root.to_path_buf();
    for name in relative.split('/') {
        let stem = name
            .split('.')
            .next()
            .unwrap_or(name)
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        let device = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
        ) || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(
                stem.get(3..),
                Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
            ));
        if name.is_empty()
            || matches!(name, "." | "..")
            || device
            || name.ends_with(['.', ' '])
            || name
                .chars()
                .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
        {
            return Err(problem("sync path contains a non-portable component"));
        }
        path.push(name);
    }
    Ok(path)
}

fn remote_child(root: &str, relative: &str) -> String {
    format!("{}/{relative}", root.trim_end_matches('/'))
}

fn local_parents(path: &Path) -> Result<(), FileFailure> {
    let parent = path
        .parent()
        .ok_or_else(|| problem("sync path has no parent"))?;
    for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|e| problem(e.to_string()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(problem("sync parent is not a real directory"));
        }
    }
    Ok(())
}

fn local_bytes(path: &Path, stop: &AtomicBool) -> Result<Vec<u8>, FileFailure> {
    check_stop(stop)?;
    local_parents(path)?;
    let before = fs::symlink_metadata(path).map_err(|e| problem(e.to_string()))?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.len() > MAX_DIRECTORY_HASH_BYTES as u64
    {
        return Err(problem(
            "sync content requires a regular file no larger than 64 MiB",
        ));
    }
    let mut file = fs::File::open(path).map_err(|e| problem(e.to_string()))?;
    let opened = file.metadata().map_err(|e| problem(e.to_string()))?;
    if !opened.is_file()
        || before.len() != opened.len()
        || before.modified().ok() != opened.modified().ok()
    {
        return Err(problem("local file changed while opening"));
    }
    let mut bytes = Vec::new();
    let mut buffer = vec![0; 32 * 1024];
    loop {
        check_stop(stop)?;
        let count = file.read(&mut buffer).map_err(|e| problem(e.to_string()))?;
        if count == 0 {
            break;
        }
        if bytes.len().saturating_add(count) > MAX_DIRECTORY_HASH_BYTES {
            return Err(problem("sync file exceeds the 64 MiB content bound"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let after = fs::symlink_metadata(path).map_err(|e| problem(e.to_string()))?;
    if !after.is_file()
        || after.file_type().is_symlink()
        || after.len() != bytes.len() as u64
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(problem("local file changed while reading"));
    }
    Ok(bytes)
}

fn budget(total: &mut u64, size: u64) -> Result<(), FileFailure> {
    *total = total
        .checked_add(size)
        .filter(|value| *value <= MAX_HASH_TOTAL)
        .ok_or_else(|| {
            problem("directory content verification exceeds 256 MiB across both sides")
        })?;
    if size > MAX_DIRECTORY_HASH_BYTES as u64 {
        return Err(problem("sync file exceeds 64 MiB"));
    }
    Ok(())
}

pub(super) async fn plan(
    sftp: &SftpSession,
    local: PathBuf,
    remote: String,
    direction: DirectorySyncDirection,
    policy: DirectorySyncDeletePolicy,
    stop: &AtomicBool,
) -> Result<DirectorySyncComparison, FileFailure> {
    let metadata = fs::symlink_metadata(&local).map_err(|e| problem(e.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(problem("local sync root must be a real directory"));
    }
    let local = local.canonicalize().map_err(|e| problem(e.to_string()))?;
    // Resolve the explicitly selected root once; all descendants are checked
    // from this canonical root and never followed as symbolic links.
    if policy == DirectorySyncDeletePolicy::IncludeDeletes {
        let selected = sftp
            .inspect_entry(&remote)
            .await?
            .ok_or_else(|| problem("mirror root is missing"))?;
        if selected.permissions.map(|m| m & 0o170000) != Some(0o040000) {
            return Err(problem(
                "mirror root must be a real explicitly typed directory",
            ));
        }
    }
    let remote = sftp.canonicalize(&remote).await?;
    let root = sftp
        .inspect_entry(&remote)
        .await?
        .ok_or_else(|| problem("remote sync root is missing"))?;
    if root.is_symlink || !root.is_directory {
        return Err(problem("remote sync root must be a real directory"));
    }
    let mut left = super::worker::snapshot_local_tree(&local, stop)?;
    let remote_entries = sftp
        .snapshot_tree_limited(&remote, keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES, 32)
        .await?;
    if policy == DirectorySyncDeletePolicy::IncludeDeletes {
        let metadata_right: Vec<_> = remote_entries
            .iter()
            .map(|entry| {
                let relative = super::worker::remote_relative_path(&remote, &entry.path)
                    .ok_or_else(|| problem("mirror entry escaped the selected root"))?;
                let kind = match entry.permissions.map(|m| m & 0o170000) {
                    Some(0o100000) => DirectoryEntryKind::File,
                    Some(0o040000) => DirectoryEntryKind::Directory,
                    Some(0o120000) => DirectoryEntryKind::Symlink,
                    _ => DirectoryEntryKind::Other,
                };
                Ok(DirectoryEntrySnapshot::new(
                    relative,
                    kind,
                    if kind == DirectoryEntryKind::File {
                        entry.size
                    } else {
                        None
                    },
                    Some(0),
                ))
            })
            .collect::<Result<_, FileFailure>>()?;
        let metadata_report = keelshell_core::compare_directories(&left, &metadata_right)
            .map_err(|e| problem(e.to_string()))?;
        let conflicts: Vec<_> =
            keelshell_core::directory_mirror_conflicts(&metadata_report, direction)
                .into_iter()
                .filter(|conflict| {
                    conflict.reason
                        != keelshell_core::DirectoryMirrorConflictReason::IncompleteContent
                })
                .collect();
        if !conflicts.is_empty() {
            if conflicts.iter().map(|c| c.path.len() + 64).sum::<usize>() > REVIEW_BYTES {
                return Err(problem(
                    "mirror conflicts exceed the complete review budget; choose a smaller folder",
                ));
            }
            return Err(FileFailure::MirrorConflicts(conflicts));
        }
    }
    let mut right = Vec::with_capacity(remote_entries.len());
    let mut total = 0;
    for snapshot in &mut left {
        check_stop(stop)?;
        let path = local_child(&local, &snapshot.path)?;
        match snapshot.kind {
            DirectoryEntryKind::File => {
                budget(
                    &mut total,
                    snapshot
                        .size
                        .ok_or_else(|| problem("local file size is missing"))?,
                )?;
                let bytes = local_bytes(&path, stop)?;
                if snapshot.size != Some(bytes.len() as u64) {
                    return Err(problem("local snapshot changed before hashing"));
                }
                snapshot.content_hash =
                    Some(hash_directory_content(&bytes).map_err(|e| problem(e.to_string()))?);
                // Content is the equality criterion; times are not copied.
                snapshot.modified = Some(0);
            }
            DirectoryEntryKind::Directory => snapshot.modified = Some(0),
            _ => {
                return Err(problem(
                    "directory merge refuses symbolic links and special files",
                ));
            }
        }
    }
    for remote_entry in remote_entries {
        check_stop(stop)?;
        let relative = super::worker::remote_relative_path(&remote, &remote_entry.path)
            .ok_or_else(|| problem("remote sync entry escaped the selected root"))?;
        local_child(&local, &relative)?;
        let mode = remote_entry
            .permissions
            .ok_or_else(|| problem("remote sync entry type is missing"))?
            & 0o170000;
        let kind = match mode {
            0o040000 => DirectoryEntryKind::Directory,
            0o100000 => DirectoryEntryKind::File,
            _ => {
                return Err(problem(
                    "directory merge refuses remote links, special files and missing types",
                ));
            }
        };
        let mut snapshot = DirectoryEntrySnapshot::new(
            relative,
            kind,
            if kind == DirectoryEntryKind::File {
                remote_entry.size
            } else {
                None
            },
            Some(0),
        );
        if kind == DirectoryEntryKind::File {
            budget(
                &mut total,
                remote_entry
                    .size
                    .ok_or_else(|| problem("remote file size is missing"))?,
            )?;
            let bytes = sftp
                .read_regular(&remote_entry.path, MAX_DIRECTORY_HASH_BYTES)
                .await?;
            if snapshot.size != Some(bytes.len() as u64) {
                return Err(problem("remote snapshot changed before hashing"));
            }
            snapshot.content_hash =
                Some(hash_directory_content(&bytes).map_err(|e| problem(e.to_string()))?);
        }
        right.push(snapshot);
    }
    let report =
        keelshell_core::compare_directories(&left, &right).map_err(|e| problem(e.to_string()))?;
    if report
        .rows()
        .iter()
        .any(|row| matches!((&row.left, &row.right), (Some(a), Some(b)) if a.kind != b.kind))
    {
        return Err(problem(
            "directory merge refuses file/directory type conflicts",
        ));
    }
    let mut portable_names = std::collections::BTreeMap::new();
    for row in report.rows() {
        if let Some(previous) = portable_names.insert(row.path.to_lowercase(), &row.path)
            && previous != &row.path
        {
            return Err(problem(
                "directory merge refuses paths that differ only in letter case",
            ));
        }
    }
    let plan = match policy {
        DirectorySyncDeletePolicy::PreserveDestination => {
            keelshell_core::plan_directory_sync(&report, direction, policy)
        }
        DirectorySyncDeletePolicy::IncludeDeletes => {
            keelshell_core::plan_directory_mirror(&report, direction)
        }
    }
    .map_err(|e| problem(e.to_string()))?;
    let comparison = DirectorySyncComparison {
        local,
        remote,
        report,
        plan,
    };
    review_message(&comparison)?;
    Ok(comparison)
}

pub(super) fn review_message(review: &DirectorySyncComparison) -> Result<Message, FileFailure> {
    let (zh, en) = if review.plan.direction() == DirectorySyncDirection::LeftToRight {
        ("本地 → 远端", "Local → remote")
    } else {
        ("远端 → 本地", "Remote → local")
    };
    let hash_text = |hash: Option<keelshell_core::DirectoryContentHash>| {
        hash.map(|bytes| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
            .unwrap_or_else(|| "—".into())
    };
    let paths = |chinese| {
        review
            .plan
            .operations()
            .iter()
            .map(|op| match op {
                DirectorySyncOperation::Copy {
                    path,
                    source_kind,
                    expected_source_hash,
                    expected_source_size,
                    expected_destination_kind,
                    expected_destination_size,
                    expected_destination_hash,
                } => {
                    let label = if *source_kind == DirectoryEntryKind::Directory {
                        if chinese {
                            "新建目录"
                        } else {
                            "Create directory"
                        }
                    } else if expected_destination_kind.is_some() {
                        if chinese {
                            "替换常规文件"
                        } else {
                            "Replace regular file"
                        }
                    } else if chinese {
                        "新增常规文件"
                    } else {
                        "Create regular file"
                    };
                    format!(
                        "{label}: {path}\n  {}: {:?} / {} · {}: {:?} / {}",
                        if chinese {
                            "源字节/SHA-256"
                        } else {
                            "Source bytes/SHA-256"
                        },
                        expected_source_size,
                        hash_text(*expected_source_hash),
                        if chinese {
                            "目标字节/SHA-256"
                        } else {
                            "Destination bytes/SHA-256"
                        },
                        expected_destination_size,
                        hash_text(*expected_destination_hash)
                    )
                }
                DirectorySyncOperation::Delete {
                    path,
                    expected_destination_kind,
                    expected_destination_size,
                    expected_destination_hash,
                } => {
                    let label = match (chinese, expected_destination_kind) {
                        (true, DirectoryEntryKind::File) => "删除目标独有常规文件",
                        (false, DirectoryEntryKind::File) => "Delete destination-only regular file",
                        (true, _) => "删除目标独有目录（全部子节点完成后）",
                        (false, _) => "Delete destination-only directory (after all children)",
                    };
                    format!(
                        "{label}: {path}\n  {}: {:?} / {}",
                        if chinese {
                            "目标字节/SHA-256"
                        } else {
                            "Destination bytes/SHA-256"
                        },
                        expected_destination_size,
                        hash_text(*expected_destination_hash)
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let deletes = review
        .plan
        .operations()
        .iter()
        .filter(|op| matches!(op, DirectorySyncOperation::Delete { .. }))
        .count();
    let (zh_policy, en_policy) = if review.plan.is_bounded_mirror() {
        (
            format!(
                "镜像：明确删除 {deletes} 项，删除不可撤销；子树已逐项完整展开；先删除子节点，最后删除空父目录。新增、变化或重新出现的节点会停止后续删除。"
            ),
            format!(
                "Mirror: {deletes} explicit irreversible deletions; subtrees expanded into complete reviewed rows; children first, empty parents last. Added, changed or reappearing nodes stop further deletion."
            ),
        )
    } else {
        (
            "合并：保留目标独有项。".into(),
            "Merge: destination-only entries preserved.".into(),
        )
    };
    let zh_text = format!(
        "{zh}\n本地：{}\n远端：{}\n{zh_policy}\n{} 项；64 MiB/文件，256 MiB/两侧。逐项重新复核；取消或失败保留已完成和未知结果。无远端原子比较锁，不自动重试。\n审核指纹：{}\n{}\n确认此完整范围？",
        review.local.display(),
        review.remote,
        review.plan.operation_count(),
        review.plan.review_fingerprint(),
        paths(true)
    );
    let en_text = format!(
        "{en}\nLocal: {}\nRemote: {}\n{en_policy}\n{} operations; 64 MiB/file, 256 MiB/both sides. Every item rechecked; cancellation/failure retains completed and unknown results. No remote compare-and-swap; no automatic retry.\nReview fingerprint: {}\n{}\nConfirm this complete scope?",
        review.local.display(),
        review.remote,
        review.plan.operation_count(),
        review.plan.review_fingerprint(),
        paths(false)
    );
    if zh_text.len().max(en_text.len()) > REVIEW_BYTES {
        return Err(problem(
            "sync plan exceeds the 128 KiB full review text bound; choose a smaller folder",
        ));
    }
    Ok(Message::new(zh_text, en_text))
}

async fn current_remote(
    sftp: &SftpSession,
    path: &str,
    stop: &AtomicBool,
) -> Result<Option<DirectoryEntrySnapshot>, FileFailure> {
    check_stop(stop)?;
    let Some(entry) = sftp.inspect_entry(path).await? else {
        return Ok(None);
    };
    let kind = match entry.permissions.map(|m| m & 0o170000) {
        Some(0o040000) => DirectoryEntryKind::Directory,
        Some(0o100000) => DirectoryEntryKind::File,
        _ => {
            return Err(problem(
                "remote sync target is a link, special file or untyped entry",
            ));
        }
    };
    let mut snapshot = DirectoryEntrySnapshot::new(
        "leaf",
        kind,
        if kind == DirectoryEntryKind::File {
            entry.size
        } else {
            None
        },
        Some(0),
    );
    if kind == DirectoryEntryKind::File {
        snapshot.content_hash = Some(
            hash_directory_content(&sftp.read_regular(path, MAX_DIRECTORY_HASH_BYTES).await?)
                .map_err(|e| problem(e.to_string()))?,
        );
    }
    Ok(Some(snapshot))
}

fn current_local(
    path: &Path,
    stop: &AtomicBool,
) -> Result<Option<DirectoryEntrySnapshot>, FileFailure> {
    check_stop(stop)?;
    local_parents(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(problem(e.to_string())),
    };
    if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(problem("local sync target is a link or special file"));
    }
    let kind = if metadata.is_dir() {
        DirectoryEntryKind::Directory
    } else {
        DirectoryEntryKind::File
    };
    let mut snapshot = DirectoryEntrySnapshot::new(
        "leaf",
        kind,
        if metadata.is_file() {
            Some(metadata.len())
        } else {
            None
        },
        Some(0),
    );
    if metadata.is_file() {
        snapshot.content_hash = Some(
            hash_directory_content(&local_bytes(path, stop)?)
                .map_err(|e| problem(e.to_string()))?,
        );
    }
    Ok(Some(snapshot))
}

fn expected(
    actual: Option<&DirectoryEntrySnapshot>,
    kind: Option<DirectoryEntryKind>,
    size: Option<u64>,
    hash: Option<keelshell_core::DirectoryContentHash>,
) -> Result<(), FileFailure> {
    if actual.map(|e| e.kind) != kind
        || actual.and_then(|e| e.size) != size
        || actual.and_then(|e| e.content_hash) != hash
    {
        return Err(problem(
            "source or destination changed after review; rebuild the synchronization plan",
        ));
    }
    Ok(())
}

pub(super) async fn apply(
    sftp: &SftpSession,
    review: DirectorySyncComparison,
    stop: &AtomicBool,
    progress: &mpsc::SyncSender<WorkerMessage>,
    journal: &journal::SharedJournal,
) -> Result<Outcome, FileFailure> {
    let authorized = || !stop.load(Ordering::Acquire);
    let spec = if review.plan.direction() == DirectorySyncDirection::LeftToRight {
        TransferSpec::upload(&review.local, &review.remote)
    } else {
        TransferSpec::download(&review.remote, &review.local)
    };
    let ownership = sftp.reserve_directory_sync(&spec, &authorized).await?;
    let fresh = plan(
        sftp,
        review.local.clone(),
        review.remote.clone(),
        review.plan.direction(),
        review.plan.delete_policy(),
        stop,
    )
    .await?;
    if fresh.local != review.local
        || fresh.remote != review.remote
        || fresh.report != review.report
        || fresh.plan != review.plan
    {
        return Err(problem(
            "directory snapshots changed after review; no synchronization was started",
        ));
    }
    let confirmed = review
        .plan
        .clone()
        .confirm(review.plan.review_token())
        .map_err(|e| problem(e.to_string()))?;
    let mut completed = 0;
    for (index, operation) in confirmed.plan().operations().iter().enumerate() {
        check_stop(stop)?;
        journal::mark(journal, index, journal::StepState::Verifying);
        if let DirectorySyncOperation::Delete { path, .. } = operation {
            journal::mark(journal, index, journal::StepState::Unknown);
            let result = match review.plan.direction() {
                DirectorySyncDirection::LeftToRight => {
                    ownership.remove_remote_reviewed(&confirmed, path).await
                }
                DirectorySyncDirection::RightToLeft => {
                    ownership.remove_local_reviewed(&confirmed, path).await
                }
            };
            if let Err(error) = result {
                if !matches!(error, SessionError::MutationUncertain) {
                    journal::mark(journal, index, journal::StepState::Rejected);
                }
                return Err(error.into());
            }
            journal::mark(journal, index, journal::StepState::Completed);
            completed += 1;
            super::send_worker_progress(
                progress,
                Message::new(
                    format!("镜像删除已确认：{path}"),
                    format!("Mirror deletion confirmed: {path}"),
                ),
            );
            continue;
        }
        let DirectorySyncOperation::Copy {
            path,
            source_kind,
            expected_source_hash,
            expected_source_size,
            expected_destination_kind,
            expected_destination_size,
            expected_destination_hash,
        } = operation
        else {
            return Err(problem(
                "directory merge does not delete destination-only entries",
            ));
        };
        let local = local_child(&review.local, path)?;
        let remote = remote_child(&review.remote, path);
        let left = current_local(&local, stop)?;
        let right = current_remote(sftp, &remote, stop).await?;
        let (source, target) = if review.plan.direction() == DirectorySyncDirection::LeftToRight {
            (left.as_ref(), right.as_ref())
        } else {
            (right.as_ref(), left.as_ref())
        };
        expected(
            source,
            Some(*source_kind),
            *expected_source_size,
            *expected_source_hash,
        )?;
        expected(
            target,
            *expected_destination_kind,
            *expected_destination_size,
            *expected_destination_hash,
        )?;
        match (review.plan.direction(), source_kind) {
            (DirectorySyncDirection::LeftToRight, DirectoryEntryKind::Directory) => {
                if target.is_none() {
                    journal::mark(journal, index, journal::StepState::Unknown);
                    mutation_result(ownership.mkdir_remote(&remote).await, journal, index)?;
                    ownership
                        .verify_mutation(async {
                            let entry = sftp.inspect_entry(&remote).await?;
                            if !entry.is_some_and(|e| {
                                e.permissions.map(|m| m & 0o170000) == Some(0o040000)
                            }) {
                                return Err(SessionError::UnverifiedMutation(
                                    "created directory type mismatch",
                                ));
                            }
                            Ok(())
                        })
                        .await?;
                }
            }
            (DirectorySyncDirection::RightToLeft, DirectoryEntryKind::Directory) => {
                if target.is_none() {
                    journal::mark(journal, index, journal::StepState::Unknown);
                    mutation_result(
                        ownership.local_operation(|| fs::create_dir(&local)),
                        journal,
                        index,
                    )?;
                    ownership
                        .verify_mutation(async {
                            let entry = current_local(&local, stop).map_err(|_| {
                                SessionError::UnverifiedMutation(
                                    "created local directory readback failed",
                                )
                            })?;
                            if !entry.is_some_and(|e| e.kind == DirectoryEntryKind::Directory) {
                                return Err(SessionError::UnverifiedMutation(
                                    "created local directory type mismatch",
                                ));
                            }
                            Ok(())
                        })
                        .await?;
                }
            }
            (DirectorySyncDirection::LeftToRight, DirectoryEntryKind::File) => {
                let bytes = local_bytes(&local, stop)?;
                verify_bytes(&bytes, *expected_source_size, *expected_source_hash)?;
                expected(
                    current_remote(sftp, &remote, stop).await?.as_ref(),
                    *expected_destination_kind,
                    *expected_destination_size,
                    *expected_destination_hash,
                )?;
                check_stop(stop)?;
                journal::mark(journal, index, journal::StepState::Unknown);
                mutation_result(
                    ownership.write_remote_atomic(&remote, &bytes).await,
                    journal,
                    index,
                )?;
                ownership
                    .verify_mutation(async {
                        let bytes = sftp.read_regular(&remote, MAX_DIRECTORY_HASH_BYTES).await?;
                        verify_bytes(&bytes, *expected_source_size, *expected_source_hash).map_err(
                            |_| {
                                SessionError::UnverifiedMutation("directory file readback mismatch")
                            },
                        )
                    })
                    .await?;
            }
            (DirectorySyncDirection::RightToLeft, DirectoryEntryKind::File) => {
                let bytes = sftp.read_regular(&remote, MAX_DIRECTORY_HASH_BYTES).await?;
                verify_bytes(&bytes, *expected_source_size, *expected_source_hash)?;
                let parent = local
                    .parent()
                    .ok_or_else(|| problem("local sync target has no parent"))?;
                journal::mark(journal, index, journal::StepState::Unknown);
                let temporary = mutation_result(
                    ownership.local_operation(|| {
                        let mut file = tempfile::NamedTempFile::new_in(parent)?;
                        file.write_all(&bytes)?;
                        file.flush()?;
                        Ok(file)
                    }),
                    journal,
                    index,
                )?;
                expected(
                    current_local(&local, stop)?.as_ref(),
                    *expected_destination_kind,
                    *expected_destination_size,
                    *expected_destination_hash,
                )?;
                check_stop(stop)?;
                ownership.local_operation(|| {
                    if target.is_some() {
                        temporary
                            .as_file()
                            .set_permissions(fs::metadata(&local)?.permissions())?;
                        temporary.persist(&local).map_err(|e| e.error)?;
                    } else {
                        temporary.persist_noclobber(&local).map_err(|e| e.error)?;
                    }
                    Ok(())
                })?;
                ownership
                    .verify_mutation(async {
                        let bytes = local_bytes(&local, stop).map_err(|_| {
                            SessionError::UnverifiedMutation("local directory file readback failed")
                        })?;
                        verify_bytes(&bytes, *expected_source_size, *expected_source_hash).map_err(
                            |_| {
                                SessionError::UnverifiedMutation(
                                    "local directory file readback mismatch",
                                )
                            },
                        )
                    })
                    .await?;
            }
            _ => return Err(problem("unsupported synchronization object type")),
        }
        journal::mark(journal, index, journal::StepState::Completed);
        completed += 1;
        super::send_worker_progress(
            progress,
            Message::new(
                format!(
                    "目录同步 {completed}/{}：{path}",
                    review.plan.operation_count()
                ),
                format!(
                    "Directory sync {completed}/{}: {path}",
                    review.plan.operation_count()
                ),
            ),
        );
    }
    Ok(Outcome::Done(Message::new(
        format!("目录同步完成：{completed} 项；请查看逐项结果并刷新列表"),
        format!(
            "Directory synchronization complete: {completed} operations; inspect item results and refresh the listing"
        ),
    )))
}

fn mutation_result<T>(
    result: Result<T, SessionError>,
    journal: &journal::SharedJournal,
    index: usize,
) -> Result<T, FileFailure> {
    result.map_err(|error| {
        if !matches!(&error, SessionError::MutationUncertain) {
            journal::mark(journal, index, journal::StepState::Rejected);
        }
        error.into()
    })
}

fn verify_bytes(
    bytes: &[u8],
    size: Option<u64>,
    hash: Option<keelshell_core::DirectoryContentHash>,
) -> Result<(), FileFailure> {
    if size != Some(bytes.len() as u64)
        || hash != Some(hash_directory_content(bytes).map_err(|e| problem(e.to_string()))?)
    {
        return Err(problem(
            "synchronization file content changed or readback failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AtomicBool, FileFailure, MAX_DIRECTORY_HASH_BYTES, MAX_HASH_TOTAL, Ordering, Path, budget,
        fs, local_bytes, local_child,
    };

    #[test]
    fn portable_paths_refuse_device_drive_traversal_and_stream_syntax() {
        let root = Path::new("/fixture");
        for relative in [
            "../escape",
            "C:escape",
            "a\\b",
            "a//b",
            "CON.txt",
            "NUL .txt",
            "CONIN$",
            "CONOUT$.log",
            "CLOCK$",
            "LPT1",
            "COM¹",
            "COM².txt",
            "LPT³",
            "a.",
            "a/b:stream",
            "/absolute",
        ] {
            assert!(local_child(root, relative).is_err(), "{relative}");
        }
        assert_eq!(
            local_child(root, "中文/a.txt").unwrap_or_else(|error| panic!("fixture: {error}")),
            root.join("中文").join("a.txt")
        );
    }

    #[test]
    fn content_budget_counts_both_sides_and_rejects_overflow() {
        let mut total = MAX_HASH_TOTAL - 1;
        assert!(budget(&mut total, 1).is_ok());
        assert!(budget(&mut total, 1).is_err());
        let mut overflow = u64::MAX;
        assert!(budget(&mut overflow, 1).is_err());
        let mut empty = 0;
        assert!(budget(&mut empty, MAX_DIRECTORY_HASH_BYTES as u64 + 1).is_err());
    }

    #[test]
    fn local_content_reads_are_bounded_cancelled_and_do_not_mutate_files() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("fixture: {error}"));
        let file = root
            .path()
            .canonicalize()
            .unwrap_or_else(|error| panic!("fixture: {error}"))
            .join("file");
        fs::write(&file, b"review bytes").unwrap_or_else(|error| panic!("fixture: {error}"));
        let stop = AtomicBool::new(false);
        assert_eq!(
            local_bytes(&file, &stop).unwrap_or_else(|error| panic!("fixture: {error}")),
            b"review bytes"
        );
        stop.store(true, Ordering::Release);
        assert!(matches!(
            local_bytes(&file, &stop),
            Err(FileFailure::Cancelled)
        ));
        assert_eq!(
            fs::read(&file).unwrap_or_else(|error| panic!("fixture: {error}")),
            b"review bytes"
        );
        stop.store(false, Ordering::Release);
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap_or_else(|error| panic!("fixture: {error}"))
            .set_len(MAX_DIRECTORY_HASH_BYTES as u64 + 1)
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        assert!(local_bytes(&file, &stop).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn local_content_refuses_static_leaf_and_parent_links() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("fixture: {error}"));
        let base = root
            .path()
            .canonicalize()
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        fs::create_dir(base.join("real")).unwrap_or_else(|error| panic!("fixture: {error}"));
        fs::write(base.join("real/file"), b"private")
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        std::os::unix::fs::symlink(base.join("real"), base.join("link"))
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        std::os::unix::fs::symlink(base.join("real/file"), base.join("leaf"))
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        for file in [base.join("link/file"), base.join("leaf")] {
            assert!(local_bytes(&file, &AtomicBool::new(false)).is_err());
        }
    }
}
