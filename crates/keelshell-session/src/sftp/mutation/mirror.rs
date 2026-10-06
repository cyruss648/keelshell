//! Exact reviewed mirror removals inside the existing tree owner.
use super::*;
use keelshell_core::{
    ConfirmedDirectorySync, DirectoryEntryKind, DirectorySyncDirection, DirectorySyncOperation,
    MAX_DIRECTORY_HASH_BYTES, hash_directory_content,
};

pub(super) mod tree;

struct DeleteObservation {
    kind: DirectoryEntryKind,
    size: Option<u64>,
    hash: Option<keelshell_core::DirectoryContentHash>,
}
impl FileMutationScope<'_> {
    fn mirror_paths(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
        direction: DirectorySyncDirection,
    ) -> Result<(PathBuf, String, DeleteObservation)> {
        let plan = confirmed.plan();
        if !plan.is_bounded_mirror() || plan.direction() != direction || self.targets.len() != 2 {
            return Err(SessionError::Invalid(
                "removal requires the exact bounded mirror direction and owner",
            ));
        }
        let Some(DirectorySyncOperation::Delete {
            expected_destination_kind,
            expected_destination_size,
            expected_destination_hash,
            ..
        }) = plan.operations().iter().find(
            |op| matches!(op, DirectorySyncOperation::Delete { path, .. } if path == relative),
        )
        else {
            return Err(SessionError::Invalid(
                "path is not an explicitly reviewed mirror deletion",
            ));
        };
        let (local_root, remote_root) = match (&self.targets[0], &self.targets[1]) {
            (
                RequestedTarget::Local(local, local_write, true),
                RequestedTarget::Remote(remote, remote_write, _),
            ) if *local_write == (direction == DirectorySyncDirection::RightToLeft)
                && *remote_write == (direction == DirectorySyncDirection::LeftToRight) =>
            {
                (local, remote)
            }
            _ => {
                return Err(SessionError::Invalid(
                    "mirror roots do not match reserved source and destination",
                ));
            }
        };
        let mut local = local_root.clone();
        for component in relative.split('/') {
            local.push(component);
        }
        directory::validate_local_root(&local)?;
        let remote = format!("{}/{relative}", remote_root.trim_end_matches('/'));
        inspection::inspection_path(&remote)?;
        Ok((
            local,
            remote,
            DeleteObservation {
                kind: *expected_destination_kind,
                size: *expected_destination_size,
                hash: *expected_destination_hash,
            },
        ))
    }

    /// Delete one explicitly confirmed destination-only regular file or a remote
    /// directory after its individually confirmed children. The remaining reviewed
    /// subtree is checked before each deletion; no implicit expansion is permitted.
    /// Rechecks source absence, target type/content, parents and owner immediately
    /// before the one protocol request; only acknowledged deletion plus checked
    /// absence is success. Lost replies/readback retain process-wide quarantine.
    /// SFTP v3 supplies observations, not remote no-follow or compare-and-swap.
    /// After child admission, any refusal except Closed withdraws the mirror review;
    /// a fresh comparison/confirmation needs a new owner. Plan withdrawal alone
    /// does not turn a failed read into an unknown write.
    pub async fn remove_remote_reviewed(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
    ) -> Result<()> {
        let _child = self.begin_child()?;
        let result = deadline(
            self.sftp.timeout,
            "reviewed mirror removal",
            Box::pin(async {
                let (_, remote, expected) =
                    self.mirror_paths(confirmed, relative, DirectorySyncDirection::LeftToRight)?;
                self.bind_mirror(confirmed, relative)?;
                let raw = DirectoryChannel(self.observe(self.sftp._connection.sftp_raw()).await?);
                self.remote_child(&remote).await?;
                self.revalidate().await?;
                self.source_absent(confirmed, relative, DirectorySyncDirection::LeftToRight)
                    .await?;
                self.verify_subtree(confirmed, relative, DirectorySyncDirection::LeftToRight)
                    .await?;
                self.source_absent(confirmed, relative, DirectorySyncDirection::LeftToRight)
                    .await?;
                let entry = self
                    .observe(self.sftp.inspect_entry(&remote))
                    .await?
                    .ok_or(SessionError::Invalid("reviewed mirror target disappeared"))?;
                match expected.kind {
                    DirectoryEntryKind::File => {
                        if entry.permissions.map(|m| m & 0o170000) != Some(0o100000) {
                            return Err(SessionError::Invalid(
                                "mirror target is no longer a regular file",
                            ));
                        }
                        let snapshot = self
                            .observe(
                                self.sftp
                                    .read_regular_snapshot(&remote, MAX_DIRECTORY_HASH_BYTES),
                            )
                            .await?;
                        verify_content(&snapshot.content, &expected)?;
                        let again = self
                            .observe(self.sftp.inspect_entry(&remote))
                            .await?
                            .ok_or(SessionError::Invalid("reviewed mirror target disappeared"))?;
                        if again.size != snapshot.entry.size
                            || again.permissions != snapshot.entry.permissions
                            || again.modified != snapshot.entry.modified
                        {
                            return Err(SessionError::Invalid(
                                "mirror target changed during revalidation",
                            ));
                        }
                    }
                    DirectoryEntryKind::Directory => {
                        if entry.permissions.map(|m| m & 0o170000) != Some(0o040000)
                            || !self.observe(self.sftp.list(&remote)).await?.is_empty()
                        {
                            return Err(SessionError::Invalid(
                                "mirror directory must still be a real empty directory",
                            ));
                        }
                    }
                    _ => return Err(SessionError::Invalid("unsupported mirror deletion kind")),
                }
                // Root/route admission precedes the final observations. Do not
                // insert another canonicalization wait between checked subtree
                // content and dispatch; it could otherwise consume stale observations.
                self.verify_subtree(confirmed, relative, DirectorySyncDirection::LeftToRight)
                    .await?;
                match expected.kind {
                    DirectoryEntryKind::File => {
                        self.dispatch_removal(raw.0.remove(&remote)).await?
                    }
                    DirectoryEntryKind::Directory => {
                        self.dispatch_removal(raw.0.rmdir(&remote)).await?
                    }
                    _ => return Err(SessionError::Invalid("unsupported mirror deletion kind")),
                };
                // Even an acknowledged mutation must not release its owner while
                // the required absence observation is lost or ambiguous.
                self.pending.store(true, Ordering::Release);
                if self.sftp.inspect_entry(&remote).await?.is_some() {
                    return Err(SessionError::MutationUncertain);
                }
                self.pending.store(false, Ordering::Release);
                Ok(())
            }),
        )
        .await;
        let result = self.retain_refusal(result);
        let result = self.complete(result).await;
        if result.is_ok() {
            self.record_removal(relative)?;
        }
        result
    }

    async fn dispatch_removal<T>(
        &self,
        operation: impl std::future::Future<
            Output = std::result::Result<T, russh_sftp::client::error::Error>,
        >,
    ) -> Result<T> {
        self.authority()?;
        if !self.ticket()?.is_active() {
            return Err(SessionError::MutationQuarantined);
        }
        if self.pending.load(Ordering::Acquire) {
            return Err(SessionError::MutationUncertain);
        }
        let dispatch = async {
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
            result=dispatch=>result,
        };
        self.result(result)
    }

    /// Delete one explicitly confirmed local mirror destination. Uses the same
    /// existing shared owner and unknown-result quarantine as remote mutations.
    /// Async filesystem deletion can outlive a dropped future: pending remains
    /// true until the actual syscall result and checked absence are observed.
    /// Path checks do not claim a filesystem-wide compare-and-swap lock.
    /// After child admission, any refusal except Closed withdraws the mirror review;
    /// pending-write quarantine and dropped-read lifecycle remain unchanged.
    pub async fn remove_local_reviewed(
        &self,
        confirmed: &ConfirmedDirectorySync,
        relative: &str,
    ) -> Result<()> {
        let _child = self.begin_child()?;
        let result = deadline(
            self.sftp.timeout,
            "reviewed local mirror removal",
            Box::pin(async {
                let (local, _, expected) =
                    self.mirror_paths(confirmed, relative, DirectorySyncDirection::RightToLeft)?;
                self.bind_mirror(confirmed, relative)?;
                self.source_absent(confirmed, relative, DirectorySyncDirection::RightToLeft)
                    .await?;
                self.revalidate().await?;
                self.source_absent(confirmed, relative, DirectorySyncDirection::RightToLeft)
                    .await?;
                self.verify_subtree(confirmed, relative, DirectorySyncDirection::RightToLeft)
                    .await?;
                self.observe(verify_local(&local, &expected)).await?;
                // Source observation awaits the peer; revocation during that wait
                // must still stop the local syscall before dispatch.
                self.authority()?;
                self.pending.store(true, Ordering::Release);
                let removed = match expected.kind {
                    DirectoryEntryKind::File => tokio::fs::remove_file(&local).await,
                    DirectoryEntryKind::Directory => tokio::fs::remove_dir(&local).await,
                    _ => return Err(SessionError::Invalid("unsupported mirror deletion kind")),
                };
                if let Err(error) = removed {
                    self.pending.store(false, Ordering::Release);
                    return Err(error.into());
                }
                directory::local_directory_chain(
                    local
                        .parent()
                        .ok_or(SessionError::Invalid("mirror target has no parent"))?,
                )
                .await?;
                match tokio::fs::symlink_metadata(&local).await {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return Err(SessionError::MutationUncertain),
                }
                self.pending.store(false, Ordering::Release);
                Ok(())
            }),
        )
        .await;
        let result = self.retain_refusal(result);
        let result = self.complete(result).await;
        if result.is_ok() {
            self.record_removal(relative)?;
        }
        result
    }
}

fn verify_content(bytes: &[u8], expected: &DeleteObservation) -> Result<()> {
    if expected.size != Some(bytes.len() as u64)
        || expected.hash
            != Some(
                hash_directory_content(bytes)
                    .map_err(|_| SessionError::OutputLimit(MAX_DIRECTORY_HASH_BYTES))?,
            )
    {
        return Err(SessionError::Invalid("reviewed mirror content changed"));
    }
    Ok(())
}

async fn verify_local(path: &Path, expected: &DeleteObservation) -> Result<()> {
    directory::local_directory_chain(
        path.parent()
            .ok_or(SessionError::Invalid("mirror target has no parent"))?,
    )
    .await?;
    let before = tokio::fs::symlink_metadata(path).await?;
    if before.file_type().is_symlink() {
        return Err(SessionError::Invalid("mirror target is a symbolic link"));
    }
    match expected.kind {
        DirectoryEntryKind::File
            if before.is_file() && before.len() <= MAX_DIRECTORY_HASH_BYTES as u64 =>
        {
            let file = tokio::fs::File::open(path).await?;
            let opened = file.metadata().await?;
            if !opened.is_file()
                || before.len() != opened.len()
                || before.modified().ok() != opened.modified().ok()
            {
                return Err(SessionError::Invalid("mirror file changed while opening"));
            }
            let mut bytes = Vec::new();
            file.take(MAX_DIRECTORY_HASH_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .await?;
            verify_content(&bytes, expected)?;
            let after = tokio::fs::symlink_metadata(path).await?;
            if !after.is_file()
                || after.file_type().is_symlink()
                || before.len() != after.len()
                || before.modified().ok() != after.modified().ok()
            {
                return Err(SessionError::Invalid("mirror file changed while reading"));
            }
        }
        DirectoryEntryKind::Directory if before.is_dir() => {
            if tokio::fs::read_dir(path)
                .await?
                .next_entry()
                .await?
                .is_some()
            {
                return Err(SessionError::Invalid("mirror directory is no longer empty"));
            }
        }
        _ => return Err(SessionError::Invalid("mirror destination type changed")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn size<F>(
        _: impl FnOnce(&'static FileMutationScope<'static>, &'static ConfirmedDirectorySync) -> F,
    ) -> usize {
        std::mem::size_of::<F>()
    }
    #[test]
    fn recursive_mirror_futures_fit_the_standard_transport_worker_stack() {
        let remote = size(|scope, confirmed| scope.remove_remote_reviewed(confirmed, "tree/file"));
        let local = size(|scope, confirmed| scope.remove_local_reviewed(confirmed, "tree/file"));
        let subtree = size(|scope, confirmed| {
            scope.verify_subtree(confirmed, "tree/file", DirectorySyncDirection::LeftToRight)
        });
        eprintln!(
            "recursive mirror future bytes: remote={remote}, local={local}, subtree={subtree}"
        );
        for (name, bytes) in [("remote", remote), ("local", local), ("subtree", subtree)] {
            assert!(
                bytes < 8 * 1024,
                "{name} mirror future consumes {bytes} inline bytes"
            );
        }
    }
}
