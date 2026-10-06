//! Reviewed continuation of a partial tree; validate every file before writing.

use super::*;

/// A read-only continuation plan for one existing partial directory tree.
///
/// The destination root must exist. Existing entries must be a subset of the
/// source tree with matching types, and existing file bytes must equal the
/// complete source prefix. Missing files and directories are created only after
/// the whole reviewed source and destination have been validated again.
///
/// The plan belongs to one authenticated SSH connection, not one SFTP subsystem.
/// It is not serialized: reconnecting or restarting requires another scan and
/// explicit review. Cancellation and failure preserve partial output.
#[derive(Clone)]
pub struct DirectoryResumePlan {
    connection: SshSession,
    spec: TransferSpec,
    source: Vec<Entry>,
    target: Vec<Entry>,
    files: Vec<FileResumePlan>,
    bytes: u64,
    existing_bytes: u64,
}

impl std::fmt::Debug for DirectoryResumePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectoryResumePlan")
            .field("spec", &self.spec)
            .field("entries", &self.source.len())
            .field("bytes", &self.bytes)
            .field("existing_bytes", &self.existing_bytes)
            .finish_non_exhaustive()
    }
}

impl DirectoryResumePlan {
    /// Exact local source or destination reviewed for continuation.
    pub fn local_path(&self) -> &Path {
        &self.spec.local
    }

    /// Exact remote source or destination reviewed for continuation.
    pub fn remote_path(&self) -> &str {
        &self.spec.remote
    }

    /// Whether this plan uploads or downloads the tree.
    pub fn direction(&self) -> TransferDirection {
        self.spec.direction
    }

    /// Number of regular source files, including complete and empty files.
    pub fn files(&self) -> usize {
        self.files.len()
    }

    /// Number of source directories, including the root and empty directories.
    pub fn directories(&self) -> usize {
        self.source.iter().filter(|entry| entry.directory).count()
    }

    /// Total source bytes, including already verified destination prefixes.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Bytes present in destination files and verified against the source.
    pub fn existing_bytes(&self) -> u64 {
        self.existing_bytes
    }
}

impl SftpSession {
    /// Scan an existing partial directory and verify every existing file prefix.
    ///
    /// This method performs no writes. Both roots must be real directories;
    /// symlinks, extra destination entries, type conflicts and changed prefixes
    /// are rejected. The usual directory depth, size, count and name limits
    /// apply. Review the result before passing it to `enqueue_directory_resume`.
    pub async fn plan_directory_resume(&self, spec: TransferSpec) -> Result<DirectoryResumePlan> {
        deadline(
            SCAN_TIMEOUT,
            "SFTP directory resume scan",
            self.scan_directory_resume(spec),
        )
        .await
    }

    async fn scan_directory_resume(&self, spec: TransferSpec) -> Result<DirectoryResumePlan> {
        let (source, target, bytes, existing_bytes) = self.resume_tree_entries(&spec).await?;
        let mut files = Vec::new();
        for entry in source.iter().filter(|entry| !entry.directory) {
            let file_spec = TransferSpec {
                direction: spec.direction,
                local: local_child(&spec.local, &entry.relative),
                remote: remote_child(&spec.remote, &entry.relative),
            };
            let file = self.plan_file_resume_allow_missing(file_spec).await?;
            let expected_target = target
                .binary_search_by(|item| item.relative.cmp(&entry.relative))
                .ok()
                .map(|position| target[position].size)
                .unwrap_or(0);
            if file.bytes() != entry.size || file.existing_bytes() != expected_target {
                return Err(SessionError::Invalid(
                    "directory changed during resume scan",
                ));
            }
            files.push(file);
        }
        // A file scan can outlive a directory scan. Recheck the complete entry
        // sets before publishing a plan whose summary the user will approve.
        let (current_source, current_target, _, _) = self.resume_tree_entries(&spec).await?;
        if current_source != source || current_target != target {
            return Err(SessionError::Invalid(
                "directory changed during resume scan",
            ));
        }
        Ok(DirectoryResumePlan {
            connection: self._connection.clone(),
            spec,
            source,
            target,
            files,
            bytes,
            existing_bytes,
        })
    }

    async fn resume_tree_entries(
        &self,
        spec: &TransferSpec,
    ) -> Result<(Vec<Entry>, Vec<Entry>, u64, u64)> {
        validate_remote_root(&spec.remote)?;
        validate_local_root(&spec.local)?;
        local_directory_chain(&spec.local).await?;
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        remote_directory_chain(&raw.0, &spec.remote).await?;
        let (local, local_bytes) = scan_local(&spec.local).await?;
        let (remote, remote_bytes) = scan_remote(&raw.0, &spec.remote).await?;
        let (source, target, bytes, existing_bytes) = match spec.direction {
            TransferDirection::Upload => (local, remote, local_bytes, remote_bytes),
            TransferDirection::Download => (remote, local, remote_bytes, local_bytes),
        };
        for entry in &source {
            validate_lengths(
                &local_child(&spec.local, &entry.relative),
                &remote_child(&spec.remote, &entry.relative),
            )?;
        }
        for entry in &target {
            let position = source
                .binary_search_by(|item| item.relative.cmp(&entry.relative))
                .map_err(|_| {
                    SessionError::Invalid("resume destination contains an unplanned entry")
                })?;
            let expected = &source[position];
            if entry.directory != expected.directory || entry.size > expected.size {
                return Err(SessionError::Invalid(
                    "resume destination type or length does not match the source",
                ));
            }
        }
        Ok((source, target, bytes, existing_bytes))
    }

    async fn verify_resume_tree_preflight(
        &self,
        plan: &DirectoryResumePlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<u64> {
        // Do not interleave validation and creation. In particular, an early
        // missing file must remain absent when a later prefix no longer matches.
        loop {
            context.checkpoint().await?;
            let generation = context.resume_generation();
            let (source, target, _, _) = context
                .validation(
                    SCAN_TIMEOUT,
                    "directory resume tree revalidation",
                    self.resume_tree_entries(&plan.spec),
                )
                .await?;
            if source != plan.source || target != plan.target {
                return Err(SessionError::Invalid(
                    "directory changed after review; scan and review again",
                )
                .into());
            }
            for file in &plan.files {
                context.checkpoint().await?;
                context
                    .validation(
                        SCAN_TIMEOUT,
                        "directory resume file revalidation",
                        self.validate_file_resume(file),
                    )
                    .await?;
                context.checkpoint().await?;
            }
            if context.resume_generation() == generation {
                return Ok(generation);
            }
            // A long pause during preflight invalidates earlier verification.
            // Complete another uninterrupted pass before admitting any writes.
        }
    }

    pub(in crate::sftp) async fn queued_directory_resume(
        &self,
        plan: &DirectoryResumePlan,
        context: &TransferContext,
    ) -> TransferExecutionResult<()> {
        if !Arc::ptr_eq(&plan.connection.handle, &self._connection.handle) {
            return Err(SessionError::Invalid(
                "directory resume plan belongs to a different SSH connection",
            )
            .into());
        }
        let raw = DirectoryChannel(self._connection.sftp_raw().await?);
        let mut verified_generation = self.verify_resume_tree_preflight(plan, context).await?;
        let mut writes_started = false;
        for entry in plan.source.iter().filter(|entry| entry.directory) {
            context.checkpoint().await?;
            if !writes_started && context.resume_generation() != verified_generation {
                verified_generation = self.verify_resume_tree_preflight(plan, context).await?;
            }
            let local = local_child(&plan.spec.local, &entry.relative);
            let remote = remote_child(&plan.spec.remote, &entry.relative);
            let existed = plan
                .target
                .binary_search_by(|item| item.relative.cmp(&entry.relative))
                .is_ok();
            match plan.spec.direction {
                TransferDirection::Upload => {
                    verify_local_entry(&local, entry).await?;
                    if existed {
                        remote_directory_chain(&raw.0, &remote).await?;
                    } else {
                        remote_directory_chain(&raw.0, remote_parent(&remote)?).await?;
                        remote_absent(&raw.0, &remote).await?;
                        let mut attrs = FileAttributes::empty();
                        attrs.permissions = Some(0o700);
                        writes_started = true;
                        context.remote_mutation(raw.0.mkdir(remote, attrs)).await?;
                    }
                }
                TransferDirection::Download => {
                    verify_remote_entry(&raw.0, &remote, entry).await?;
                    if existed {
                        local_directory_chain(&local).await?;
                    } else {
                        local_directory_chain(
                            local
                                .parent()
                                .ok_or(SessionError::Invalid("local target has no parent"))?,
                        )
                        .await?;
                        writes_started = true;
                        context.local_mutation(tokio::fs::create_dir(local)).await?;
                    }
                }
            }
        }
        for file in &plan.files {
            context.checkpoint().await?;
            if !writes_started && context.resume_generation() != verified_generation {
                verified_generation = self.verify_resume_tree_preflight(plan, context).await?;
            }
            // The file executor rechecks this file immediately before opening
            // and counts its verified prefix plus acknowledged new bytes.
            writes_started = true;
            self.execute_file_resume(file, context).await?;
        }
        loop {
            context.checkpoint().await?;
            let generation = context.resume_generation();
            let (source, target, _, _) = context
                .validation(
                    SCAN_TIMEOUT,
                    "completed resume tree validation",
                    self.resume_tree_entries(&plan.spec),
                )
                .await?;
            if source != plan.source
                || target.len() != plan.source.len()
                || target.iter().zip(&plan.source).any(|(target, source)| {
                    target.relative != source.relative
                        || target.directory != source.directory
                        || target.size != source.size
                })
            {
                return Err(
                    SessionError::Invalid("directory changed during resumed transfer").into(),
                );
            }
            // An early completed file can change while a later file is paused.
            // A pause during this final verification invalidates the pass too.
            for file in &plan.files {
                context.checkpoint().await?;
                context
                    .validation(
                        SCAN_TIMEOUT,
                        "completed directory resume file validation",
                        self.validate_completed_file_resume(file),
                    )
                    .await?;
                context.checkpoint().await?;
            }
            if context.resume_generation() == generation {
                break;
            }
        }
        Ok(())
    }
}
