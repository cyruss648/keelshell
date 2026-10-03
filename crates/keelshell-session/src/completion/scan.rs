use super::{
    CompletionCandidate, CompletionError, CompletionKind, CompletionQuery, CompletionResult,
    MAX_PATH, Result, probe_directories, safe_name, safe_path, session_error,
};
use crate::{SshSession, ssh::OwnedRawSftpSession};
use russh_sftp::{
    client::error::Error as SftpError,
    protocol::{FileAttributes, StatusCode},
};
use std::collections::HashSet;
use tokio::time::Instant;

const MAX_ENTRIES: usize = 8192;
const MAX_NAME_BYTES: usize = 2 * 1024 * 1024;
const MAX_STATS: usize = 64;
const MAX_CANDIDATES: usize = 64;

pub(super) async fn complete(
    session: &SshSession,
    query: CompletionQuery,
    until: Instant,
) -> Result<CompletionResult> {
    let (directories, prefix, commands, directories_only, skipped, limited) = match query {
        CompletionQuery::Commands { prefix } => {
            let output = session
                .completion_exec_until(until)
                .await
                .map_err(session_error)?;
            let (directories, skipped, limited) = probe_directories(output)?;
            (directories, prefix, true, false, skipped, limited)
        }
        CompletionQuery::Paths {
            directory,
            prefix,
            directories_only,
        } => (vec![directory], prefix, false, directories_only, 0, false),
    };
    let raw = session
        .completion_sftp_until(until)
        .await
        .map_err(session_error)?;
    let mut scan = Scan {
        raw: &raw,
        entries: 0,
        name_bytes: 0,
        stats: 0,
        candidates: Vec::new(),
        result: CompletionResult {
            candidates: Vec::new(),
            resolved_directory: None,
            limited,
            skipped_unsafe_entries: 0,
            skipped_path_directories: skipped,
        },
    };
    let mut canonical_seen = HashSet::new();
    for (rank, directory) in directories.iter().enumerate() {
        let parent = match canonical_directory(&raw, directory).await {
            Ok(parent) => parent,
            Err(error)
                if commands
                    && matches!(
                        error,
                        CompletionError::PermissionDenied | CompletionError::Unavailable
                    ) =>
            {
                scan.result.skipped_path_directories += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        if !canonical_seen.insert(parent.clone()) {
            scan.result.skipped_path_directories += 1;
            continue;
        }
        if !commands {
            scan.result.resolved_directory = Some(parent.clone());
        }
        match scan
            .directory(&parent, &prefix, rank, commands, directories_only)
            .await
        {
            Ok(()) => {}
            Err(error)
                if commands
                    && matches!(
                        error,
                        CompletionError::PermissionDenied | CompletionError::Unavailable
                    ) =>
            {
                scan.result.skipped_path_directories += 1
            }
            Err(error) => return Err(error),
        }
        if scan.entries >= MAX_ENTRIES || scan.name_bytes >= MAX_NAME_BYTES {
            scan.result.limited |= rank + 1 < directories.len();
            break;
        }
    }
    scan.result.candidates = scan
        .candidates
        .into_iter()
        .map(|(_, candidate)| candidate)
        .collect();
    Ok(scan.result)
}

pub(super) async fn canonical_directory(raw: &OwnedRawSftpSession, path: &str) -> Result<String> {
    let mut response = raw.realpath(path).await.map_err(sftp_error)?;
    if response.files.len() != 1 {
        return Err(CompletionError::InvalidResponse);
    }
    let file = response
        .files
        .pop()
        .ok_or(CompletionError::InvalidResponse)?;
    if file.filename.contains("//")
        || !safe_path(&file.filename)
        || file
            .filename
            .split('/')
            .any(|part| matches!(part, "." | ".."))
    {
        return Err(CompletionError::InvalidResponse);
    }
    let parent = file.filename.trim_end_matches('/');
    Ok(if parent.is_empty() {
        "/".to_owned()
    } else {
        parent.to_owned()
    })
}

struct Scan<'a> {
    raw: &'a OwnedRawSftpSession,
    entries: usize,
    name_bytes: usize,
    stats: usize,
    candidates: Vec<(usize, CompletionCandidate)>,
    result: CompletionResult,
}
impl Scan<'_> {
    async fn directory(
        &mut self,
        parent: &str,
        prefix: &str,
        rank: usize,
        commands: bool,
        directories_only: bool,
    ) -> Result<()> {
        let handle = self.raw.opendir(parent).await.map_err(sftp_error)?.handle;
        // Opaque handles can contain controls, but must survive the library's
        // UTF-8 conversion exactly. Channel teardown releases a rejected handle.
        if handle.len() > 256 || handle.contains('\u{fffd}') {
            return Err(CompletionError::InvalidResponse);
        }
        let outcome = self
            .read_directory(&handle, parent, prefix, rank, commands, directories_only)
            .await;
        self.raw.close(handle).await.map_err(sftp_error)?;
        outcome
    }
    async fn read_directory(
        &mut self,
        handle: &str,
        parent: &str,
        prefix: &str,
        rank: usize,
        commands: bool,
        directories_only: bool,
    ) -> Result<()> {
        loop {
            let page = match self.raw.readdir(handle).await {
                Ok(page) => page,
                Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => break,
                Err(error) => return Err(sftp_error(error)),
            };
            if page.files.is_empty() {
                return Err(CompletionError::InvalidResponse);
            }
            for file in page.files {
                let bytes = file.filename.len().saturating_add(file.longname.len());
                if self.entries >= MAX_ENTRIES
                    || bytes > MAX_NAME_BYTES.saturating_sub(self.name_bytes)
                {
                    self.result.limited = true;
                    return Ok(());
                }
                self.entries += 1;
                self.name_bytes += bytes;
                if file.filename == "." || file.filename == ".." {
                    continue;
                }
                if !safe_name(&file.filename) {
                    self.result.skipped_unsafe_entries += 1;
                    continue;
                }
                if !file.filename.starts_with(prefix) {
                    continue;
                }
                let path = format!("{}/{}", parent.trim_end_matches('/'), file.filename);
                if path.len() > MAX_PATH {
                    self.result.skipped_unsafe_entries += 1;
                    continue;
                }
                let mut attrs = file.attrs;
                if attrs.permissions.is_none_or(|mode| mode & 0o170000 == 0) {
                    let Some(observed) = self.attributes(&path, false).await? else {
                        continue;
                    };
                    attrs = observed;
                }
                let is_symlink = attrs.file_type().is_symlink();
                if is_symlink {
                    let Some(target) = self.attributes(&path, true).await? else {
                        if !commands && !directories_only {
                            self.insert(
                                rank,
                                CompletionCandidate {
                                    name: file.filename,
                                    path,
                                    kind: CompletionKind::Symlink,
                                    is_symlink,
                                },
                                commands,
                            );
                        }
                        continue;
                    };
                    attrs = target;
                }
                let Some(kind) = classify(&attrs, commands, directories_only) else {
                    continue;
                };
                self.insert(
                    rank,
                    CompletionCandidate {
                        name: file.filename,
                        path,
                        kind,
                        is_symlink,
                    },
                    commands,
                );
            }
        }
        Ok(())
    }
    async fn attributes(&mut self, path: &str, follow: bool) -> Result<Option<FileAttributes>> {
        if self.stats == MAX_STATS {
            self.result.limited = true;
            return Ok(None);
        }
        self.stats += 1;
        // NAME attributes are optional. LSTAT first preserves link identity;
        // STAT alone could mislabel an untyped symlink as an ordinary file.
        let result = if follow {
            self.raw.stat(path).await
        } else {
            self.raw.lstat(path).await
        };
        match result {
            Ok(response) => Ok(Some(response.attrs)),
            Err(SftpError::Status(status))
                if matches!(
                    status.status_code,
                    StatusCode::NoSuchFile | StatusCode::PermissionDenied | StatusCode::Failure
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(sftp_error(error)),
        }
    }

    fn insert(&mut self, rank: usize, candidate: CompletionCandidate, commands: bool) {
        // Earlier PATH directories win; duplicates never shift provenance. The
        // vector stays bounded while retaining the best deterministic 64 rows.
        if self
            .candidates
            .iter()
            .any(|(_, existing)| existing.name == candidate.name)
        {
            return;
        }
        let key = |rank: usize, kind: CompletionKind| {
            if commands {
                rank
            } else {
                usize::from(kind != CompletionKind::Directory)
            }
        };
        let at = self.candidates.partition_point(|(prior_rank, prior)| {
            (key(*prior_rank, prior.kind), prior.name.as_str())
                <= (key(rank, candidate.kind), candidate.name.as_str())
        });
        if at < MAX_CANDIDATES {
            self.candidates.insert(at, (rank, candidate));
        } else {
            self.result.limited = true;
        }
        if self.candidates.len() > MAX_CANDIDATES {
            self.candidates.pop();
            self.result.limited = true;
        }
    }
}
fn classify(
    attrs: &FileAttributes,
    commands: bool,
    directories_only: bool,
) -> Option<CompletionKind> {
    let kind = attrs.file_type();
    if commands {
        return (kind.is_file() && attrs.permissions.is_some_and(|mode| mode & 0o111 != 0))
            .then_some(CompletionKind::Executable);
    }
    if kind.is_dir() {
        Some(CompletionKind::Directory)
    } else if kind.is_file() && !directories_only {
        Some(CompletionKind::File)
    } else {
        None
    }
}
fn sftp_error(error: SftpError) -> CompletionError {
    match error {
        SftpError::Status(status) => match status.status_code {
            StatusCode::PermissionDenied => CompletionError::PermissionDenied,
            StatusCode::NoSuchFile | StatusCode::Failure => CompletionError::Unavailable,
            StatusCode::OpUnsupported => CompletionError::UnsupportedEnvironment,
            _ => CompletionError::InvalidResponse,
        },
        SftpError::Timeout => CompletionError::Timeout,
        SftpError::IO(_) => CompletionError::Transport,
        _ => CompletionError::InvalidResponse,
    }
}
