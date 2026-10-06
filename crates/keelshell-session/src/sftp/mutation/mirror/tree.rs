//! Remaining reviewed subtree observations; never implicit removal expansion.
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Default)]
pub(in crate::sftp::mutation) struct Progress {
    fingerprint: Option<String>,
    completed: BTreeSet<String>,
    invalidated: bool,
}
impl FileMutationScope<'_> {
    fn progress(&self) -> Result<std::sync::MutexGuard<'_, Progress>> {
        self.mirror_progress
            .lock()
            .map_err(|_| SessionError::Invalid("mirror progress is unavailable"))
    }
    pub(super) fn bind_mirror(&self, confirmed: &ConfirmedDirectorySync, path: &str) -> Result<()> {
        let mut progress = self.progress()?;
        let fingerprint = confirmed.plan().review_fingerprint();
        if progress.invalidated
            || progress
                .fingerprint
                .as_ref()
                .is_some_and(|bound| bound != &fingerprint)
            || progress.completed.contains(path)
        {
            return Err(SessionError::Invalid(
                "mirror owner cannot change or replay its reviewed plan",
            ));
        }
        progress.fingerprint = Some(fingerprint);
        Ok(())
    }
    pub(super) fn retain_refusal<T>(&self, result: Result<T>) -> Result<T> {
        if result.is_err() && !matches!(&result, Err(SessionError::Closed)) {
            // Any failed reviewed prerequisite invalidates this approval, even
            // if I/O/protocol errors hide the particular observed difference.
            // This only withdraws the plan: pending-write quarantine remains
            // governed by complete(), and Closed keeps the session lifecycle.
            self.progress()?.invalidated = true;
        }
        result
    }
    pub(super) fn record_removal(&self, path: &str) -> Result<()> {
        self.progress()?.completed.insert(path.to_owned());
        Ok(())
    }
    /// Consume an observation only while the same session and reservation are
    /// still live. Observations confer no new route or mutation authority.
    pub(super) fn observe<T>(
        &self,
        future: impl std::future::Future<Output = Result<T>>,
    ) -> impl std::future::Future<Output = Result<T>> {
        // The checked SFTP futures carry large debug state. Heap-owning that
        // state at the boundary keeps nested worker polls off the standard stack.
        let observation = Box::pin(future);
        async move {
            self.authority()?;
            if !self.ticket()?.is_active() {
                return Err(SessionError::MutationQuarantined);
            }
            let result = observation.await;
            self.authority()?;
            if !self.ticket()?.is_active() {
                return Err(SessionError::MutationQuarantined);
            }
            result
        }
    }
    pub(super) fn subtree_root<'a>(
        &self,
        confirmed: &'a ConfirmedDirectorySync,
        path: &'a str,
    ) -> &'a str {
        confirmed
            .plan()
            .operations()
            .iter()
            .filter_map(|op| match op {
                DirectorySyncOperation::Delete {
                    path: ancestor,
                    expected_destination_kind: DirectoryEntryKind::Directory,
                    ..
                } if ancestor == path
                    || path
                        .strip_prefix(ancestor.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/')) =>
                {
                    Some(ancestor.as_str())
                }
                _ => None,
            })
            .min_by_key(|ancestor| ancestor.len())
            .unwrap_or(path)
    }
    fn remaining(
        &self,
        confirmed: &ConfirmedDirectorySync,
        root: &str,
    ) -> Result<BTreeMap<String, DeleteObservation>> {
        let progress = self.progress()?;
        Ok(confirmed
            .plan()
            .operations()
            .iter()
            .filter_map(|op| match op {
                DirectorySyncOperation::Delete {
                    path,
                    expected_destination_kind,
                    expected_destination_size,
                    expected_destination_hash,
                } if (path == root
                    || path
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('/')))
                    && !progress.completed.contains(path) =>
                {
                    Some((
                        path.clone(),
                        DeleteObservation {
                            kind: *expected_destination_kind,
                            size: *expected_destination_size,
                            hash: *expected_destination_hash,
                        },
                    ))
                }
                _ => None,
            })
            .collect())
    }
    pub(super) async fn source_absent(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
        direction: DirectorySyncDirection,
    ) -> Result<()> {
        let root = self.subtree_root(confirmed, relative);
        let (local, remote, _) = self.mirror_paths(confirmed, root, direction)?;
        match direction {
            DirectorySyncDirection::LeftToRight => {
                self.observe(directory::local_directory_chain(
                    local
                        .parent()
                        .ok_or(SessionError::Invalid("mirror source has no parent"))?,
                ))
                .await?;
                self.observe(async {
                    match tokio::fs::symlink_metadata(&local).await {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        Ok(_) => Err(SessionError::Invalid(
                            "mirror source subtree is no longer absent",
                        )),
                        Err(error) => Err(error.into()),
                    }
                })
                .await
            }
            DirectorySyncDirection::RightToLeft => {
                if self
                    .observe(self.sftp.inspect_entry(&remote))
                    .await?
                    .is_some()
                {
                    return Err(SessionError::Invalid(
                        "mirror source subtree is no longer absent",
                    ));
                }
                Ok(())
            }
        }
    }
    pub(super) async fn verify_subtree(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
        direction: DirectorySyncDirection,
    ) -> Result<()> {
        let result = Box::pin(self.verify_subtree_checked(confirmed, relative, direction)).await;
        self.retain_refusal(result)
    }
    async fn verify_subtree_checked(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
        direction: DirectorySyncDirection,
    ) -> Result<()> {
        let root = self.subtree_root(confirmed, relative);
        let expected = self.remaining(confirmed, root)?;
        // A single leaf still uses the exact regular-file/empty-directory check.
        // A directory subtree additionally requires its entire remaining namespace.
        if !expected
            .get(root)
            .is_some_and(|entry| entry.kind == DirectoryEntryKind::Directory)
        {
            return Ok(());
        }
        let first = self.subtree_namespace(confirmed, root, direction).await?;
        if first != expected.keys().cloned().collect() {
            return Err(SessionError::Invalid(
                "reviewed mirror subtree namespace changed",
            ));
        }
        let mut content_bytes = 0u64;
        for (path, observation) in &expected {
            self.authority()?;
            if observation.kind == DirectoryEntryKind::File {
                content_bytes = content_bytes
                    .checked_add(observation.size.unwrap_or(u64::MAX))
                    .filter(|bytes| *bytes <= keelshell_core::MAX_DIRECTORY_MIRROR_CONTENT_BYTES)
                    .ok_or(SessionError::OutputLimit(
                        keelshell_core::MAX_DIRECTORY_MIRROR_CONTENT_BYTES as usize,
                    ))?;
            }
            let (local, remote, _) = self.mirror_paths(confirmed, path, direction)?;
            match direction {
                DirectorySyncDirection::LeftToRight => {
                    let actual = self
                        .observe(self.sftp.inspect_entry(&remote))
                        .await?
                        .ok_or(SessionError::Invalid(
                            "reviewed mirror subtree entry disappeared",
                        ))?;
                    if actual.permissions.map(|mode| mode & 0o170000)
                        != Some(match observation.kind {
                            DirectoryEntryKind::File => 0o100000,
                            DirectoryEntryKind::Directory => 0o040000,
                            _ => {
                                return Err(SessionError::Invalid(
                                    "unsupported mirror subtree type",
                                ));
                            }
                        })
                    {
                        return Err(SessionError::Invalid(
                            "reviewed mirror subtree type changed",
                        ));
                    }
                    if observation.kind == DirectoryEntryKind::File {
                        let snapshot = self
                            .observe(
                                self.sftp
                                    .read_regular_snapshot(&remote, MAX_DIRECTORY_HASH_BYTES),
                            )
                            .await?;
                        verify_content(&snapshot.content, observation)?;
                    }
                }
                DirectorySyncDirection::RightToLeft => {
                    self.observe(verify_local_kind_content(&local, observation))
                        .await?;
                }
            }
        }
        // A second namespace sweep catches additions, removals and reappearing
        // completed children during the content reads. The final syscall is still
        // not atomic with external writers; RMDIR never falls back to recursion.
        if self.subtree_namespace(confirmed, root, direction).await? != first {
            return Err(SessionError::Invalid(
                "mirror subtree changed during content revalidation",
            ));
        }
        Ok(())
    }
    async fn subtree_namespace(
        &self,
        confirmed: &ConfirmedDirectorySync,
        root: &str,
        direction: DirectorySyncDirection,
    ) -> Result<BTreeSet<String>> {
        let mut pending = VecDeque::from([root.to_owned()]);
        let mut names = BTreeSet::new();
        while let Some(relative) = pending.pop_front() {
            self.authority()?;
            let (local, remote, expected) = self.mirror_paths(confirmed, &relative, direction)?;
            if !names.insert(relative.clone())
                || names.len() > keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES
            {
                return Err(SessionError::EntryLimit(
                    keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES,
                ));
            }
            let depth = relative
                .split('/')
                .count()
                .saturating_sub(usize::from(expected.kind != DirectoryEntryKind::Directory));
            if depth > keelshell_core::MAX_DIRECTORY_MIRROR_DEPTH {
                return Err(SessionError::Invalid("mirror subtree exceeds depth budget"));
            }
            if expected.kind != DirectoryEntryKind::Directory {
                continue;
            }
            match direction {
                DirectorySyncDirection::LeftToRight => {
                    let before = self
                        .observe(self.sftp.inspect_entry(&remote))
                        .await?
                        .ok_or(SessionError::Invalid(
                            "reviewed mirror directory disappeared",
                        ))?;
                    if before.permissions.map(|mode| mode & 0o170000) != Some(0o040000) {
                        return Err(SessionError::Invalid(
                            "mirror directory is not a real directory",
                        ));
                    }
                    let children = self
                        .observe(
                            self.sftp.list_limited(
                                &remote,
                                keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES
                                    .saturating_sub(names.len()),
                            ),
                        )
                        .await?;
                    let after = self.observe(self.sftp.inspect_entry(&remote)).await?;
                    if !after.is_some_and(|entry| {
                        entry.permissions == before.permissions
                            && entry.modified == before.modified
                            && entry.size == before.size
                    }) {
                        return Err(SessionError::Invalid(
                            "mirror directory changed during listing",
                        ));
                    }
                    let mut folded = BTreeSet::new();
                    for child in children {
                        let name = child
                            .path
                            .strip_prefix(&remote)
                            .and_then(|suffix| suffix.strip_prefix('/'))
                            .ok_or(SessionError::Invalid(
                                "mirror child escaped its selected parent",
                            ))?;
                        if name.contains('/') || !folded.insert(name.to_lowercase()) {
                            return Err(SessionError::Invalid("ambiguous mirror child name"));
                        }
                        let child_relative = format!("{relative}/{name}");
                        // Membership comes only from the immutable reviewed plan,
                        // never from expanding the currently observed namespace.
                        self.mirror_paths(confirmed, &child_relative, direction)?;
                        pending.push_back(child_relative);
                    }
                }
                DirectorySyncDirection::RightToLeft => {
                    self.observe(directory::local_directory_chain(&local))
                        .await?;
                    let mut listing = self
                        .observe(async { Ok(tokio::fs::read_dir(&local).await?) })
                        .await?;
                    let mut folded = BTreeSet::new();
                    while let Some(child) = self
                        .observe(async { Ok(listing.next_entry().await?) })
                        .await?
                    {
                        let name = child.file_name();
                        let name = name
                            .to_str()
                            .ok_or(SessionError::Invalid("mirror child name is not UTF-8"))?;
                        if name.contains('/') || !folded.insert(name.to_lowercase()) {
                            return Err(SessionError::Invalid("ambiguous mirror child name"));
                        }
                        let child_relative = format!("{relative}/{name}");
                        self.mirror_paths(confirmed, &child_relative, direction)?;
                        pending.push_back(child_relative);
                        if pending.len().saturating_add(names.len())
                            > keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES
                        {
                            return Err(SessionError::EntryLimit(
                                keelshell_core::MAX_DIRECTORY_COMPARE_ENTRIES,
                            ));
                        }
                    }
                    self.observe(directory::local_directory_chain(&local))
                        .await?;
                }
            }
        }
        Ok(names)
    }
}

async fn verify_local_kind_content(path: &Path, expected: &DeleteObservation) -> Result<()> {
    if expected.kind == DirectoryEntryKind::File {
        return verify_local(path, expected).await;
    }
    directory::local_directory_chain(path).await
}
