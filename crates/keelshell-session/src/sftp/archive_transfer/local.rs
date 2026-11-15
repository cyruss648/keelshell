//! Local observations and private extraction. Files never inherit archive modes.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::time::SystemTime;

use keelshell_core::{
    ArchiveEntryKind, MAX_ARCHIVE_DEPTH as MAX_DEPTH, MAX_ARCHIVE_ENTRIES as MAX_ENTRIES,
    MAX_ARCHIVE_EXPANDED_BYTES as MAX_EXPANDED, MAX_ARCHIVE_MANIFEST_BYTES as MAX_MANIFEST,
    MAX_ARCHIVE_PATH_BYTES as MAX_PATH,
};
use sha2::{Digest, Sha256};

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    directory: bool,
    bytes: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    owner: (u32, u32),
    #[cfg(windows)]
    attributes: u32,
}

impl Stamp {
    fn inspect(metadata: &Metadata) -> Result<Self> {
        if metadata.file_type().is_symlink() || !(metadata.is_dir() || metadata.is_file()) {
            return Err(SessionError::Invalid(
                "archive rejects local links and special files",
            ));
        }
        #[cfg(unix)]
        let (identity, mode, owner) = {
            use std::os::unix::fs::MetadataExt;
            if metadata.is_file() && metadata.nlink() != 1 {
                return Err(SessionError::Invalid(
                    "archive rejects hard-linked source aliases",
                ));
            }
            (
                (metadata.dev(), metadata.ino()),
                metadata.mode(),
                (metadata.uid(), metadata.gid()),
            )
        };
        #[cfg(windows)]
        let attributes = {
            use std::os::windows::fs::MetadataExt;
            let attributes = metadata.file_attributes();
            if attributes & 0x400 != 0 {
                return Err(SessionError::Invalid(
                    "archive rejects local reparse points",
                ));
            }
            attributes
        };
        Ok(Self {
            directory: metadata.is_dir(),
            bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            identity,
            #[cfg(unix)]
            mode,
            #[cfg(unix)]
            owner,
            #[cfg(windows)]
            attributes,
        })
    }

    fn ancestor(metadata: &Metadata) -> Result<Self> {
        let mut stamp = Self::inspect(metadata)?;
        if !stamp.directory {
            return Err(SessionError::Invalid("archive ancestor is not a directory"));
        }
        // Sibling creation, including our own staging, changes directory mtime.
        // Ancestor identity, ownership and mode remain review invariants.
        stamp.modified = None;
        Ok(stamp)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    path: PathBuf,
    member: String,
    stamp: Stamp,
    digest: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Manifest {
    ancestors: BTreeMap<PathBuf, Stamp>,
    entries: Vec<Entry>,
}

fn validate_path(path: &Path) -> Result<()> {
    directory::validate_local_root(path)?;
    let text = path
        .to_str()
        .ok_or(SessionError::Invalid("archive path is not UTF-8"))?;
    let normalized: PathBuf = path.components().collect();
    if text.len() > MAX_PATH || normalized.as_os_str() != path.as_os_str() {
        return Err(SessionError::Invalid(
            "archive path is not normalized or exceeds its bound",
        ));
    }
    Ok(())
}

async fn parents(path: &Path, authority: &Authority<'_>) -> Result<BTreeMap<PathBuf, Stamp>> {
    let parent = path
        .parent()
        .ok_or(SessionError::Invalid("archive path has no parent"))?;
    let mut result = BTreeMap::new();
    for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if ancestor.has_root() {
            let metadata = authority.io(tokio::fs::symlink_metadata(ancestor)).await?;
            result.insert(ancestor.to_owned(), Stamp::ancestor(&metadata)?);
        }
    }
    Ok(result)
}

async fn verify_parents(manifest: &Manifest, authority: &Authority<'_>) -> Result<()> {
    for (path, expected) in &manifest.ancestors {
        if Stamp::ancestor(&authority.io(tokio::fs::symlink_metadata(path)).await?)? != *expected {
            return Err(changed());
        }
    }
    authority.check()
}

async fn absent(path: &Path, authority: &Authority<'_>) -> Result<()> {
    match authority.io(tokio::fs::symlink_metadata(path)).await {
        Err(SessionError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(SessionError::Invalid("archive destination already exists")),
    }
}

pub(super) async fn destination(path: &Path, authority: &Authority<'_>) -> Result<Manifest> {
    validate_path(path)?;
    let ancestors = parents(path, authority).await?;
    absent(path, authority).await?;
    Ok(Manifest {
        ancestors,
        entries: Vec::new(),
    })
}

async fn open_read(path: &Path, authority: &Authority<'_>) -> Result<tokio::fs::File> {
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    options
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32);
    #[cfg(windows)]
    options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    authority.io(options.open(path)).await
}

async fn read_file(
    path: &Path,
    stamp: &Stamp,
    bound: u64,
    authority: &Authority<'_>,
) -> Result<(Vec<u8>, [u8; 32])> {
    if stamp.directory || stamp.bytes > bound {
        return Err(SessionError::OutputLimit(bound as usize));
    }
    let mut file = open_read(path, authority).await?;
    if Stamp::inspect(&authority.io(file.metadata()).await?)? != *stamp {
        return Err(changed());
    }
    let mut bytes = Vec::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut digest = Sha256::new();
    loop {
        let remaining = bound.saturating_sub(bytes.len() as u64).saturating_add(1);
        let length = buffer.len().min(remaining as usize);
        let count = authority.io(file.read(&mut buffer[..length])).await?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() as u64 > bound {
            return Err(SessionError::OutputLimit(bound as usize));
        }
        digest.update(&buffer[..count]);
    }
    if bytes.len() as u64 != stamp.bytes
        || Stamp::inspect(&authority.io(file.metadata()).await?)? != *stamp
        || Stamp::inspect(&authority.io(tokio::fs::symlink_metadata(path)).await?)? != *stamp
    {
        return Err(changed());
    }
    Ok((bytes, digest.finalize().into()))
}

pub(super) async fn scan(
    sources: &[PathBuf],
    authority: &Authority<'_>,
) -> Result<(Manifest, Vec<ArchiveMember>)> {
    if sources.is_empty() || sources.len() > 32 {
        return Err(SessionError::Invalid(
            "archive requires one to 32 selected sources",
        ));
    }
    let mut manifest = Manifest::default();
    let mut pending = Vec::new();
    let mut selected = BTreeSet::new();
    for source in sources {
        validate_path(source)?;
        if sources
            .iter()
            .any(|other| source != other && source.starts_with(other))
            || !selected.insert(source.to_string_lossy().to_lowercase())
        {
            return Err(SessionError::Invalid(
                "archive selected source aliases overlap",
            ));
        }
        manifest.ancestors.extend(parents(source, authority).await?);
        let name =
            source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(SessionError::Invalid(
                    "archive source must have a UTF-8 basename",
                ))?;
        pending.push((source.clone(), name.to_owned()));
    }
    let mut members = Vec::new();
    let mut names = BTreeSet::new();
    #[cfg(unix)]
    let mut identities = BTreeSet::new();
    let mut expanded = 0u64;
    let mut manifest_bytes = 0usize;
    while let Some((path, member)) = pending.pop() {
        authority.check()?;
        validate_path(&path)?;
        if member.split('/').count() > MAX_DEPTH || member.len() > MAX_PATH {
            return Err(SessionError::Invalid(
                "archive member path exceeds its bound",
            ));
        }
        manifest_bytes = manifest_bytes
            .checked_add(member.len())
            .filter(|count| *count <= MAX_MANIFEST)
            .ok_or(SessionError::OutputLimit(MAX_MANIFEST))?;
        if members.len() >= MAX_ENTRIES || !names.insert(member.to_lowercase()) {
            return Err(SessionError::Invalid(
                "archive has too many members or filename aliases",
            ));
        }
        let chain = parents(&path, authority).await?;
        for (ancestor, stamp) in chain {
            if manifest
                .ancestors
                .get(&ancestor)
                .is_some_and(|expected| *expected != stamp)
            {
                return Err(changed());
            }
            manifest.ancestors.insert(ancestor, stamp);
        }
        let stamp = Stamp::inspect(&authority.io(tokio::fs::symlink_metadata(&path)).await?)?;
        #[cfg(unix)]
        if !identities.insert(stamp.identity) {
            return Err(SessionError::Invalid(
                "archive source filesystem aliases overlap",
            ));
        }
        let (archive_member, digest) = if stamp.directory {
            let entry =
                ArchiveMember::new_directory(member.clone()).map_err(|_| invalid_archive())?;
            let mut directory = authority.io(tokio::fs::read_dir(&path)).await?;
            while let Some(child) = authority.io(directory.next_entry()).await? {
                let name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| SessionError::Invalid("archive filename is not UTF-8"))?;
                if pending.len() + members.len() + 1 >= MAX_ENTRIES {
                    return Err(SessionError::EntryLimit(MAX_ENTRIES));
                }
                let relative = format!("{member}/{name}");
                // Validate names before opening them, including colon/ADS and
                // portable device-name and traversal restrictions in the codec.
                ArchiveMember::new_directory(relative.clone()).map_err(|_| invalid_archive())?;
                pending.push((child.path(), relative));
            }
            (entry, None)
        } else {
            let (bytes, digest) = read_file(
                &path,
                &stamp,
                MAX_EXPANDED.saturating_sub(expanded),
                authority,
            )
            .await?;
            expanded += bytes.len() as u64;
            (
                ArchiveMember::new_file(member.clone(), bytes).map_err(|_| invalid_archive())?,
                Some(digest),
            )
        };
        manifest.entries.push(Entry {
            path,
            member,
            stamp,
            digest,
        });
        members.push(archive_member);
    }
    verify_parents(&manifest, authority).await?;
    for entry in &manifest.entries {
        if Stamp::inspect(
            &authority
                .io(tokio::fs::symlink_metadata(&entry.path))
                .await?,
        )? != entry.stamp
        {
            return Err(changed());
        }
    }
    manifest.entries.sort_by(|a, b| a.member.cmp(&b.member));
    members.sort_by(|a, b| a.path().cmp(b.path()));
    Ok((manifest, members))
}

pub(super) fn extraction_supported() -> Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(SessionError::Unsupported(
            "archive extraction requires owner-only staging and atomic no-replace directory publication",
        ))
    }
}

pub(super) fn staging_path(destination: &Path) -> Result<PathBuf> {
    extraction_supported()?;
    let parent = destination
        .parent()
        .ok_or(SessionError::Invalid("archive target has no parent"))?;
    let staging = parent.join(format!(".keelshell-archive-{}", uuid::Uuid::new_v4()));
    validate_path(&staging)?;
    Ok(staging)
}

pub(super) fn validate_destinations(root: &Path, members: &[ArchiveMember]) -> Result<()> {
    let mut paths = BTreeSet::new();
    for member in members {
        let target = root.join(member.path());
        validate_path(&target)?;
        for ancestor in Path::new(member.path()).ancestors() {
            if !ancestor.as_os_str().is_empty() {
                paths.insert(ancestor.to_owned());
                if paths.len() > MAX_ENTRIES {
                    return Err(SessionError::EntryLimit(MAX_ENTRIES));
                }
            }
        }
    }
    Ok(())
}

async fn private_directory(
    path: &Path,
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
) -> Result<Stamp> {
    let builder = tokio::fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    authority
        .local_mutation(scope, builder.create(path))
        .await?;
    let stamp = Stamp::inspect(&authority.io(tokio::fs::symlink_metadata(path)).await?)?;
    #[cfg(unix)]
    if !stamp.directory || stamp.mode & 0o7777 != 0o700 {
        return Err(SessionError::UnverifiedMutation(
            "archive staging is not owner-only",
        ));
    }
    Ok(stamp)
}

/// Cancellation is checked again inside the queued blocking syscall. A dropped
/// await never clears pending or releases the worker's reservation prematurely.
#[cfg(any(target_os = "macos", target_os = "linux"))]
struct PublishWait(Arc<AtomicBool>);
#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Drop for PublishWait {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

async fn publish_directory(
    staging: &Path,
    destination: &Path,
    expected_parent: &Stamp,
    expected_stage: &Stamp,
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
) -> Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use rustix::fs::{Mode, OFlags, RenameFlags, open, renameat_with};
        let parent = destination
            .parent()
            .ok_or(SessionError::Invalid("archive target has no parent"))?
            .to_owned();
        let old_name = staging.file_name().ok_or(SessionError::Worker)?.to_owned();
        let new_name = destination
            .file_name()
            .ok_or(SessionError::Worker)?
            .to_owned();
        let expected_parent = expected_parent.clone();
        let expected_stage = expected_stage.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let _wait = PublishWait(cancelled.clone());
        let ticket = scope.ticket()?;
        let pending = scope.pending.clone();
        authority.wait(scope.revalidate()).await?;
        authority.check()?;
        pending.store(true, Ordering::Release);
        authority
            .wait(async {
                let result = tokio::task::spawn_blocking(move || {
                    let _ticket = ticket;
                    let parent: std::fs::File = open(
                        &parent,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )
                    .map_err(std::io::Error::from)?
                    .into();
                    if Stamp::ancestor(&parent.metadata()?).map_err(std::io::Error::other)?
                        != expected_parent
                    {
                        return Err(std::io::Error::other("archive publication parent changed"));
                    }
                    let named: std::fs::File = rustix::fs::openat(
                        &parent,
                        &old_name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )
                    .map_err(std::io::Error::from)?
                    .into();
                    if Stamp::ancestor(&named.metadata()?).map_err(std::io::Error::other)?
                        != expected_stage
                    {
                        return Err(std::io::Error::other("archive publication staging changed"));
                    }
                    if cancelled.load(Ordering::Acquire) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted,
                            "archive publication cancelled",
                        ));
                    }
                    renameat_with(&parent, old_name, &parent, new_name, RenameFlags::NOREPLACE)
                        .map_err(std::io::Error::from)
                })
                .await
                .map_err(|_| SessionError::Worker)?;
                pending.store(false, Ordering::Release);
                result.map_err(Into::into)
            })
            .await
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (
            staging,
            destination,
            expected_parent,
            expected_stage,
            scope,
            authority,
        );
        extraction_supported()
    }
}

async fn verify_tree(
    root: &Path,
    members: &[ArchiveMember],
    authority: &Authority<'_>,
) -> Result<()> {
    let mut expected = BTreeMap::new();
    for member in members {
        let path = Path::new(member.path());
        for ancestor in path.ancestors().skip(1) {
            if !ancestor.as_os_str().is_empty() {
                expected
                    .entry(ancestor.to_string_lossy().into_owned())
                    .or_insert((ArchiveEntryKind::Directory, &[][..]));
            }
        }
        expected.insert(member.path().to_owned(), (member.kind(), member.bytes()));
    }
    let mut pending = vec![String::new()];
    let mut seen = BTreeSet::new();
    while let Some(relative) = pending.pop() {
        let path = root.join(&relative);
        parents(&path, authority).await?;
        let stamp = Stamp::inspect(&authority.io(tokio::fs::symlink_metadata(&path)).await?)?;
        #[cfg(unix)]
        if stamp.mode & 0o7777 != if stamp.directory { 0o700 } else { 0o600 } {
            return Err(SessionError::UnverifiedMutation(
                "archive extraction permissions changed",
            ));
        }
        if !relative.is_empty() {
            if !seen.insert(relative.clone()) {
                return Err(changed());
            }
            let (kind, bytes) = expected.get(&relative).ok_or_else(changed)?;
            if (*kind == ArchiveEntryKind::Directory) != stamp.directory {
                return Err(changed());
            }
            if !stamp.directory
                && read_file(&path, &stamp, bytes.len() as u64, authority)
                    .await?
                    .0
                    != *bytes
            {
                return Err(SessionError::UnverifiedMutation(
                    "archive extraction content changed",
                ));
            }
        } else if !stamp.directory {
            return Err(changed());
        }
        if stamp.directory {
            let mut directory = authority.io(tokio::fs::read_dir(&path)).await?;
            while let Some(child) = authority.io(directory.next_entry()).await? {
                let name = child.file_name().into_string().map_err(|_| changed())?;
                let child = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                if !expected.contains_key(&child) || pending.len() + seen.len() >= MAX_ENTRIES {
                    return Err(changed());
                }
                pending.push(child);
            }
        }
    }
    if seen.len() != expected.len() {
        return Err(SessionError::UnverifiedMutation(
            "archive extraction member inventory changed",
        ));
    }
    Ok(())
}

pub(super) async fn extract(
    sftp: &SftpSession,
    plan: &ArchiveTransferPlan,
    raw: &RawSftpSession,
    destination: &Path,
    staging: &Path,
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
) -> Result<()> {
    extraction_supported()?;
    validate_destinations(staging, plan.members())?;
    verify_parents(&plan.reviewed.local, authority).await?;
    absent(staging, authority).await?;
    let mut unverified = UnverifiedStage {
        ticket: Some(scope.ticket()?),
    };
    let mut stamp = private_directory(staging, scope, authority).await?;
    stamp.modified = None;
    let mut staged = Manifest::default();
    staged.ancestors.insert(staging.to_owned(), stamp);
    let mut directories = BTreeSet::new();
    for member in plan.members() {
        let path = Path::new(member.path());
        let start = usize::from(member.kind() == ArchiveEntryKind::File);
        for ancestor in path.ancestors().skip(start) {
            if !ancestor.as_os_str().is_empty() {
                directories.insert(ancestor.to_owned());
            }
        }
    }
    let mut directories: Vec<_> = directories.into_iter().collect();
    directories.sort_by_key(|path| path.components().count());
    for relative in directories {
        verify_parents(&plan.reviewed.local, authority).await?;
        verify_parents(&staged, authority).await?;
        let path = staging.join(relative);
        parents(&path, authority).await?;
        let mut stamp = private_directory(&path, scope, authority).await?;
        stamp.modified = None;
        staged.ancestors.insert(path, stamp);
    }
    for member in plan
        .members()
        .iter()
        .filter(|member| member.kind() == ArchiveEntryKind::File)
    {
        verify_parents(&plan.reviewed.local, authority).await?;
        verify_parents(&staged, authority).await?;
        let target = staging.join(member.path());
        parents(&target, authority).await?;
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        options
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        let mut file = authority
            .local_mutation(scope, options.open(&target))
            .await?;
        for chunk in member.bytes().chunks(64 * 1024) {
            verify_parents(&staged, authority).await?;
            authority
                .local_mutation(scope, file.write_all(chunk))
                .await?;
        }
        authority.local_mutation(scope, file.sync_all()).await?;
        drop(file);
    }
    verify_tree(staging, plan.members(), authority).await?;
    sftp.revalidate_archive(plan, raw, authority).await?;
    verify_parents(&plan.reviewed.local, authority).await?;
    verify_parents(&staged, authority).await?;
    verify_tree(staging, plan.members(), authority).await?;
    absent(destination, authority).await?;
    let expected_parent = plan
        .reviewed
        .local
        .ancestors
        .get(destination.parent().ok_or(SessionError::Worker)?)
        .ok_or(SessionError::Worker)?;
    let expected_stage = staged.ancestors.get(staging).ok_or(SessionError::Worker)?;
    publish_directory(
        staging,
        destination,
        expected_parent,
        expected_stage,
        scope,
        authority,
    )
    .await?;
    verify_parents(&plan.reviewed.local, authority).await?;
    if Stamp::ancestor(
        &authority
            .io(tokio::fs::symlink_metadata(destination))
            .await?,
    )? != *expected_stage
    {
        return Err(SessionError::UnverifiedMutation(
            "archive published directory identity changed",
        ));
    }
    verify_tree(destination, plan.members(), authority).await?;
    absent(staging, authority).await?;
    authority.check()?;
    unverified.ticket = None;
    Ok(())
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    #[test]
    fn local_archive_publication_primitive_never_replaces_files_or_empty_directories()
    -> std::io::Result<()> {
        use rustix::fs::{Mode, OFlags, RenameFlags, open, renameat_with};
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().canonicalize()?;
        std::fs::create_dir(root.join("staging"))?;
        std::fs::write(root.join("staging/reviewed"), b"reviewed")?;
        std::fs::create_dir(root.join("existing-directory"))?;
        std::fs::write(root.join("existing-file"), b"external owner")?;
        let parent = open(&root, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())?;
        for destination in ["existing-directory", "existing-file"] {
            assert!(
                renameat_with(
                    &parent,
                    "staging",
                    &parent,
                    destination,
                    RenameFlags::NOREPLACE
                )
                .is_err()
            );
            assert_eq!(std::fs::read(root.join("staging/reviewed"))?, b"reviewed");
        }
        assert_eq!(
            std::fs::read(root.join("existing-file"))?,
            b"external owner"
        );
        assert!(
            std::fs::read_dir(root.join("existing-directory"))?
                .next()
                .is_none()
        );
        renameat_with(
            &parent,
            "staging",
            &parent,
            "published",
            RenameFlags::NOREPLACE,
        )?;
        assert_eq!(std::fs::read(root.join("published/reviewed"))?, b"reviewed");
        Ok(())
    }
}
