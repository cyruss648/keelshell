//! Handle-relative Windows archive observations and private tree publication.
//!
//! Capabilities retain every reviewed ancestor and source object. Equality only
//! compares observations; cloning a review keeps those capabilities alive.
//! Failed or abandoned staging remains isolated and is never removed here.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use keelshell_core::{
    ArchiveEntryKind, ArchiveMember, MAX_ARCHIVE_DEPTH as MAX_DEPTH,
    MAX_ARCHIVE_ENTRIES as MAX_ENTRIES, MAX_ARCHIVE_EXPANDED_BYTES as MAX_EXPANDED,
    MAX_ARCHIVE_MANIFEST_BYTES as MAX_MANIFEST, MAX_ARCHIVE_PATH_BYTES as MAX_PATH,
};
use keelshell_platform::{
    Directory, FileInfo, Identity, PrivateEntry, WindowsPath, metadata, validate_windows_component,
    verify_private,
};
use sha2::{Digest, Sha256};

use super::mutation_worker::{MutationWorker, observe_mutation, submit_mutation};
use super::{ArchiveTransferPlan, Authority, UnverifiedStage, changed, invalid_archive};
use crate::sftp::{FileMutationScope, RawSftpSession, SftpSession};
use crate::{Result, SessionError};

type Ancestors = BTreeMap<PathBuf, Ancestor>;

#[derive(Clone, Debug)]
struct Ancestor {
    stamp: FileInfo,
    held: Directory,
}

impl PartialEq for Ancestor {
    fn eq(&self, other: &Self) -> bool {
        self.stamp == other.stamp
    }
}

impl Eq for Ancestor {}

#[derive(Clone, Debug)]
enum HeldEntry {
    Directory(Directory),
    File(Arc<File>),
}

impl HeldEntry {
    fn as_file(&self) -> &File {
        match self {
            Self::Directory(directory) => directory.as_file(),
            Self::File(file) => file,
        }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    path: PathBuf,
    member: String,
    stamp: FileInfo,
    digest: Option<[u8; 32]>,
    children: Vec<String>,
    held: HeldEntry,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.member == other.member
            && self.stamp == other.stamp
            && self.digest == other.digest
            && self.children == other.children
    }
}

impl Eq for Entry {}

/// A complete local review whose clones retain the original object identities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Manifest {
    ancestors: Ancestors,
    entries: Vec<Entry>,
    destination: Option<PathBuf>,
}

fn stopped(stop: &AtomicBool) -> Result<()> {
    if stop.load(Ordering::Acquire) {
        Err(SessionError::Closed)
    } else {
        Ok(())
    }
}

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

async fn blocking<T: Send + 'static>(
    authority: &Authority<'_>,
    operation: impl FnOnce(&AtomicBool) -> Result<T> + Send + 'static,
) -> Result<T> {
    authority.check()?;
    let stop = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(stop.clone());
    let task = tokio::task::spawn_blocking(move || {
        stopped(&stop)?;
        let result = operation(&stop);
        stopped(&stop)?;
        result
    });
    authority
        .wait(async { task.await.map_err(|_| SessionError::Worker)? })
        .await
}

async fn mutation_blocking<T: Send + 'static>(
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
    operation: impl FnOnce(&MutationWorker) -> Result<T> + Send + 'static,
) -> Result<T> {
    authority.wait(scope.revalidate()).await?;
    authority.check()?;
    let stop = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(stop.clone());
    let worker = MutationWorker {
        ticket: scope.ticket()?,
        stop,
    };
    let mut task = submit_mutation(scope.pending.clone(), worker, operation);
    authority
        .wait(observe_mutation(&scope.pending, &mut task))
        .await
}

fn alias(name: &str) -> String {
    keelshell_platform::filename_alias(name)
}

fn checked_path(path: &Path) -> Result<PathBuf> {
    let text = path
        .to_str()
        .ok_or(SessionError::Invalid("archive local path is not Unicode"))?;
    if text.len() > MAX_PATH {
        return Err(SessionError::OutputLimit(MAX_PATH));
    }
    let parsed = WindowsPath::parse(text)?;
    let mut normalized = PathBuf::from(format!("{}:\\", char::from(parsed.drive())));
    for component in parsed.components() {
        normalized.push(component);
    }
    Ok(normalized)
}

fn basename(path: &Path) -> Result<&str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(SessionError::Invalid(
            "archive local path has no named component",
        ))?;
    validate_windows_component(name)?;
    Ok(name)
}

fn ancestor_stamp(mut stamp: FileInfo) -> Result<FileInfo> {
    if !stamp.directory {
        return Err(SessionError::Invalid("archive ancestor is not a directory"));
    }
    // Sibling creation, including our own staging, changes these timestamps.
    // Identity, creation time, links, attributes and complete security remain.
    stamp.modified = 0;
    stamp.changed = 0;
    Ok(stamp)
}

fn retain_ancestor(ancestors: &mut Ancestors, path: PathBuf, held: Directory) -> Result<()> {
    let stamp = ancestor_stamp(metadata(held.as_file())?)?;
    if let Some(expected) = ancestors.get(&path) {
        if expected.stamp != stamp {
            return Err(changed());
        }
    } else {
        ancestors.insert(path, Ancestor { stamp, held });
    }
    Ok(())
}

fn observe_parents(path: &Path, ancestors: &mut Ancestors, stop: &AtomicBool) -> Result<Directory> {
    stopped(stop)?;
    let parent = path
        .parent()
        .ok_or(SessionError::Invalid("archive local path has no parent"))?;
    let mut paths: Vec<_> = parent.ancestors().map(Path::to_path_buf).collect();
    paths.reverse();
    // Keep the entire chain. Keeping only its final handle would allow an
    // earlier ancestor's identity to be reused while a review is still live.
    let chain = Directory::open_chain(parent)?;
    stopped(stop)?;
    if paths.len() != chain.len() {
        return Err(changed());
    }
    for (index, (path, held)) in paths.iter().zip(&chain).enumerate() {
        stopped(stop)?;
        if index != 0 {
            chain[index - 1].require_unique_name(basename(path)?)?;
        }
        retain_ancestor(ancestors, path.clone(), held.clone())?;
    }
    chain.last().cloned().ok_or_else(changed)
}

fn verify_ancestors(
    ancestors: &Ancestors,
    stop: &AtomicBool,
) -> Result<BTreeMap<PathBuf, Directory>> {
    let mut paths: Vec<_> = ancestors.keys().collect();
    paths.sort_by_key(|path| path.components().count());
    let mut current = BTreeMap::<PathBuf, Directory>::new();
    for path in paths {
        stopped(stop)?;
        let expected = ancestors.get(path).ok_or_else(changed)?;
        if ancestor_stamp(metadata(expected.held.as_file())?)? != expected.stamp {
            return Err(changed());
        }
        let held = if let Some(parent) = path.parent() {
            let parent = current.get(parent).ok_or_else(changed)?;
            let name = basename(path)?;
            parent.require_unique_name(name)?;
            parent.open_directory(name)?
        } else {
            let mut root = Directory::open_chain(path)?;
            if root.len() != 1 {
                return Err(changed());
            }
            root.pop().ok_or_else(changed)?
        };
        if ancestor_stamp(metadata(held.as_file())?)? != expected.stamp {
            return Err(changed());
        }
        current.insert(path.clone(), held);
    }
    stopped(stop)?;
    Ok(current)
}

fn parent_in<'a>(current: &'a BTreeMap<PathBuf, Directory>, path: &Path) -> Result<&'a Directory> {
    current
        .get(path.parent().ok_or_else(changed)?)
        .ok_or_else(changed)
}

fn names(directory: &Directory, stop: &AtomicBool) -> Result<Vec<String>> {
    stopped(stop)?;
    let mut names = directory.entries()?;
    if names.len() > MAX_ENTRIES {
        return Err(SessionError::EntryLimit(MAX_ENTRIES));
    }
    let mut aliases = BTreeSet::new();
    for name in &names {
        stopped(stop)?;
        validate_windows_component(name)?;
        if !aliases.insert(alias(name)) {
            return Err(SessionError::Invalid(
                "archive directory contains Unicode case aliases",
            ));
        }
    }
    names.sort();
    stopped(stop)?;
    Ok(names)
}

fn read_file(
    file: &File,
    stamp: &FileInfo,
    bound: u64,
    stop: &AtomicBool,
) -> Result<(Vec<u8>, [u8; 32])> {
    stopped(stop)?;
    if stamp.directory || stamp.bytes > bound {
        return Err(SessionError::OutputLimit(bound as usize));
    }
    if metadata(file)? != *stamp {
        return Err(changed());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(stamp.bytes as usize)
        .map_err(|_| SessionError::OutputLimit(bound as usize))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut reader = file;
    reader.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    loop {
        stopped(stop)?;
        let remaining = stamp.bytes.saturating_sub(bytes.len() as u64) + 1;
        let length = buffer.len().min(remaining as usize);
        let count = reader.read(&mut buffer[..length])?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > stamp.bytes {
            return Err(changed());
        }
        bytes.extend_from_slice(&buffer[..count]);
        digest.update(&buffer[..count]);
    }
    if bytes.len() as u64 != stamp.bytes || metadata(file)? != *stamp {
        return Err(changed());
    }
    stopped(stop)?;
    Ok((bytes, digest.finalize().into()))
}

fn open_entry(parent: &Directory, name: &str) -> Result<HeldEntry> {
    validate_windows_component(name)?;
    parent.require_unique_name(name)?;
    match parent.open_directory(name) {
        Ok(directory) => Ok(HeldEntry::Directory(directory)),
        Err(_) => Ok(HeldEntry::File(Arc::new(parent.open_file(name)?))),
    }
}

fn destination_blocking(path: PathBuf, stop: &AtomicBool) -> Result<Manifest> {
    let path = checked_path(&path)?;
    let name = basename(&path)?;
    let mut manifest = Manifest::default();
    observe_parents(&path, &mut manifest.ancestors, stop)?;
    let current = verify_ancestors(&manifest.ancestors, stop)?;
    parent_in(&current, &path)?.require_absent(name)?;
    stopped(stop)?;
    manifest.destination = Some(path);
    Ok(manifest)
}

/// Observe an absent destination and retain all of its reviewed ancestors.
pub(super) async fn destination(path: &Path, authority: &Authority<'_>) -> Result<Manifest> {
    let path = path.to_owned();
    blocking(authority, move |stop| destination_blocking(path, stop)).await
}

fn selected_sources(sources: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if sources.is_empty() || sources.len() > 32 {
        return Err(SessionError::Invalid(
            "archive requires one to 32 selected sources",
        ));
    }
    let mut selected: Vec<Vec<String>> = Vec::new();
    let mut paths = Vec::with_capacity(sources.len());
    for path in sources {
        let path = checked_path(path)?;
        basename(&path)?;
        let components: Vec<_> = path
            .components()
            .map(|part| alias(&part.as_os_str().to_string_lossy()))
            .collect();
        if selected
            .iter()
            .any(|other| components.starts_with(other) || other.starts_with(&components))
        {
            return Err(SessionError::Invalid(
                "archive selected source aliases overlap",
            ));
        }
        selected.push(components);
        paths.push(path);
    }
    Ok(paths)
}

fn verify_sources(manifest: &Manifest, members: &[ArchiveMember], stop: &AtomicBool) -> Result<()> {
    if manifest.entries.len() != members.len() {
        return Err(changed());
    }
    let current = verify_ancestors(&manifest.ancestors, stop)?;
    for (entry, member) in manifest.entries.iter().zip(members) {
        stopped(stop)?;
        if entry.member != member.path() || metadata(entry.held.as_file())? != entry.stamp {
            return Err(changed());
        }
        let parent = parent_in(&current, &entry.path)?;
        let name = basename(&entry.path)?;
        parent.require_unique_name(name)?;
        if entry.stamp.directory {
            let directory = parent.open_directory(name)?;
            if member.kind() != ArchiveEntryKind::Directory
                || metadata(directory.as_file())? != entry.stamp
                || names(&directory, stop)? != entry.children
                || metadata(directory.as_file())? != entry.stamp
            {
                return Err(changed());
            }
        } else {
            let file = parent.open_file(name)?;
            let (bytes, digest) = read_file(&file, &entry.stamp, entry.stamp.bytes, stop)?;
            if member.kind() != ArchiveEntryKind::File
                || bytes != member.bytes()
                || Some(digest) != entry.digest
            {
                return Err(changed());
            }
        }
    }
    let current = verify_ancestors(&manifest.ancestors, stop)?;
    // Re-enumerate every directory after all file reads. Timestamps alone are
    // insufficient evidence that no child was inserted, removed or substituted.
    for entry in &manifest.entries {
        stopped(stop)?;
        let parent = parent_in(&current, &entry.path)?;
        let held = open_entry(parent, basename(&entry.path)?)?;
        if metadata(held.as_file())? != entry.stamp
            || metadata(entry.held.as_file())? != entry.stamp
        {
            return Err(changed());
        }
        if let HeldEntry::Directory(directory) = held
            && names(&directory, stop)? != entry.children
        {
            return Err(changed());
        }
    }
    stopped(stop)
}

fn scan_blocking(
    sources: Vec<PathBuf>,
    stop: &AtomicBool,
) -> Result<(Manifest, Vec<ArchiveMember>)> {
    let sources = selected_sources(&sources)?;
    let mut manifest = Manifest::default();
    let mut pending = Vec::new();
    for path in sources {
        stopped(stop)?;
        let parent = observe_parents(&path, &mut manifest.ancestors, stop)?;
        let member = basename(&path)?.to_owned();
        pending.push((path, member, parent));
    }
    let mut members = Vec::new();
    let mut aliases = BTreeSet::new();
    let mut identities = BTreeSet::<Identity>::new();
    let mut expanded = 0u64;
    let mut manifest_bytes = 0usize;
    while let Some((path, member, parent)) = pending.pop() {
        stopped(stop)?;
        if members.len() >= MAX_ENTRIES
            || member.len() > MAX_PATH
            || member.split('/').count() > MAX_DEPTH
            || !aliases.insert(alias(&member))
        {
            return Err(SessionError::Invalid(
                "archive source exceeds bounds or contains filename aliases",
            ));
        }
        let held = open_entry(&parent, basename(&path)?)?;
        let stamp = metadata(held.as_file())?;
        if !identities.insert(stamp.identity) {
            return Err(SessionError::Invalid(
                "archive source filesystem aliases overlap",
            ));
        }
        manifest_bytes = manifest_bytes
            .checked_add(member.len() + usize::from(stamp.directory))
            .filter(|count| *count <= MAX_MANIFEST)
            .ok_or(SessionError::OutputLimit(MAX_MANIFEST))?;
        let (archive_member, digest, children) = match &held {
            HeldEntry::Directory(directory) => {
                let entry =
                    ArchiveMember::new_directory(member.clone()).map_err(|_| invalid_archive())?;
                let children = names(directory, stop)?;
                if members.len() + pending.len() + children.len() + 1 > MAX_ENTRIES {
                    return Err(SessionError::EntryLimit(MAX_ENTRIES));
                }
                retain_ancestor(&mut manifest.ancestors, path.clone(), directory.clone())?;
                for name in &children {
                    let relative = format!("{member}/{name}");
                    ArchiveMember::new_directory(relative.clone())
                        .map_err(|_| invalid_archive())?;
                    let child = checked_path(&path.join(name))?;
                    pending.push((child, relative, directory.clone()));
                }
                (entry, None, children)
            }
            HeldEntry::File(file) => {
                let (bytes, digest) =
                    read_file(file, &stamp, MAX_EXPANDED.saturating_sub(expanded), stop)?;
                expanded += bytes.len() as u64;
                let entry = ArchiveMember::new_file(member.clone(), bytes)
                    .map_err(|_| invalid_archive())?;
                (entry, Some(digest), Vec::new())
            }
        };
        if metadata(held.as_file())? != stamp {
            return Err(changed());
        }
        manifest.entries.push(Entry {
            path,
            member,
            stamp,
            digest,
            children,
            held,
        });
        members.push(archive_member);
    }
    manifest.entries.sort_by(|a, b| a.member.cmp(&b.member));
    members.sort_by(|a, b| a.path().cmp(b.path()));
    verify_sources(&manifest, &members, stop)?;
    Ok((manifest, members))
}

/// Read every selected source and recheck its full contents and namespace.
pub(super) async fn scan(
    sources: &[PathBuf],
    authority: &Authority<'_>,
) -> Result<(Manifest, Vec<ArchiveMember>)> {
    let sources = sources.to_vec();
    blocking(authority, move |stop| scan_blocking(sources, stop)).await
}

/// Windows extraction relies on the safe platform crate's fail-closed primitives.
pub(super) fn extraction_supported() -> Result<()> {
    Ok(())
}

/// Choose an uncreated sibling name; no filesystem action occurs here.
pub(super) fn staging_path(destination: &Path) -> Result<PathBuf> {
    let destination = checked_path(destination)?;
    basename(&destination)?;
    let parent = destination
        .parent()
        .ok_or(SessionError::Invalid("archive target has no parent"))?;
    checked_path(&parent.join(format!(".keelshell-archive-{}", uuid::Uuid::new_v4())))
}

#[derive(Clone, Copy)]
struct Expected<'a> {
    kind: ArchiveEntryKind,
    bytes: &'a [u8],
}

fn member_target(root: &Path, member: &str) -> Result<PathBuf> {
    if member.is_empty() || member.len() > MAX_PATH {
        return Err(invalid_archive());
    }
    let mut target = root.to_owned();
    let mut depth = 0;
    // Archive names use slash separators. Never hand that representation to
    // WindowsPath::parse or Path::join as one native filename.
    for component in member.split('/') {
        depth += 1;
        if depth > MAX_DEPTH {
            return Err(invalid_archive());
        }
        validate_windows_component(component)?;
        target.push(component);
    }
    checked_path(&target)
}

fn expected_tree<'a>(
    root: &Path,
    members: &'a [ArchiveMember],
) -> Result<BTreeMap<String, Expected<'a>>> {
    let root = checked_path(root)?;
    basename(&root)?;
    if members.len() > MAX_ENTRIES {
        return Err(SessionError::EntryLimit(MAX_ENTRIES));
    }
    let mut expected = BTreeMap::<String, Expected<'a>>::new();
    let mut aliases = BTreeMap::new();
    let mut explicit = BTreeSet::new();
    let mut manifest_bytes = 0usize;
    let mut expanded = 0u64;
    for member in members {
        member_target(&root, member.path())?;
        if !explicit.insert(member.path()) {
            return Err(invalid_archive());
        }
        manifest_bytes = manifest_bytes
            .checked_add(
                member.path().len() + usize::from(member.kind() == ArchiveEntryKind::Directory),
            )
            .filter(|count| *count <= MAX_MANIFEST)
            .ok_or(SessionError::OutputLimit(MAX_MANIFEST))?;
        expanded = expanded
            .checked_add(member.bytes().len() as u64)
            .filter(|count| *count <= MAX_EXPANDED)
            .ok_or(SessionError::OutputLimit(MAX_EXPANDED as usize))?;
        for (end, _) in member
            .path()
            .match_indices('/')
            .chain(std::iter::once((member.path().len(), "/")))
        {
            let prefix = &member.path()[..end];
            let terminal = end == member.path().len();
            let kind = if terminal {
                member.kind()
            } else {
                ArchiveEntryKind::Directory
            };
            let key = alias(prefix);
            if let Some(previous) = aliases.insert(key, prefix.to_owned())
                && previous != prefix
            {
                return Err(invalid_archive());
            }
            if expected.get(prefix).is_some_and(|node| node.kind != kind) {
                return Err(invalid_archive());
            }
            expected.insert(
                prefix.to_owned(),
                Expected {
                    kind,
                    bytes: if terminal { member.bytes() } else { &[] },
                },
            );
            if expected.len() > MAX_ENTRIES {
                return Err(SessionError::EntryLimit(MAX_ENTRIES));
            }
        }
    }
    Ok(expected)
}

/// Validate explicit and implied destinations using literal Windows components.
pub(super) fn validate_destinations(root: &Path, members: &[ArchiveMember]) -> Result<()> {
    expected_tree(root, members).map(|_| ())
}

struct TreeEntry {
    stamp: FileInfo,
    digest: Option<[u8; 32]>,
    children: Vec<String>,
    held: HeldEntry,
}

struct Tree {
    // The empty key is the root; all other keys retain archive slash spelling.
    entries: BTreeMap<String, TreeEntry>,
}

struct PublishedObservation {
    stamp: FileInfo,
    digest: Option<[u8; 32]>,
    children: Vec<String>,
}

fn release_tree_for_publication(tree: Tree) -> BTreeMap<String, PublishedObservation> {
    // Windows refuses a directory rename while a descendant has an open link,
    // including read-only handles that share DELETE. All child writes are done.
    // Consume only staging observations; the caller still retains its exact
    // PrivateEntry root and every reviewed ancestor across the root-only rename.
    tree.entries
        .into_iter()
        .map(|(relative, entry)| {
            let TreeEntry {
                stamp,
                digest,
                children,
                held,
            } = entry;
            drop(held);
            (
                relative,
                PublishedObservation {
                    stamp,
                    digest,
                    children,
                },
            )
        })
        .collect()
}

fn split_member(member: &str) -> (&str, &str) {
    member.rsplit_once('/').unwrap_or(("", member))
}

fn tree_directory<'a>(tree: &'a Tree, relative: &str) -> Result<&'a Directory> {
    match &tree.entries.get(relative).ok_or_else(changed)?.held {
        HeldEntry::Directory(directory) => Ok(directory),
        HeldEntry::File(_) => Err(changed()),
    }
}

fn verify_tree_links(tree: &Tree, stop: &AtomicBool) -> Result<()> {
    for (relative, entry) in &tree.entries {
        stopped(stop)?;
        verify_private(entry.held.as_file(), entry.stamp.directory)?;
        if metadata(entry.held.as_file())? != entry.stamp {
            return Err(changed());
        }
        if let HeldEntry::Directory(directory) = &entry.held
            && names(directory, stop)? != entry.children
        {
            return Err(changed());
        }
        if !relative.is_empty() {
            let (parent, name) = split_member(relative);
            let parent = tree_directory(tree, parent)?;
            parent.require_unique_name(name)?;
            let named = if entry.stamp.directory {
                HeldEntry::Directory(parent.open_directory(name)?)
            } else {
                HeldEntry::File(Arc::new(parent.open_file(name)?))
            };
            verify_private(named.as_file(), entry.stamp.directory)?;
            if metadata(named.as_file())? != entry.stamp {
                return Err(changed());
            }
        }
    }
    stopped(stop)
}

fn capture_tree(
    root: Directory,
    expected: &BTreeMap<String, Expected<'_>>,
    stop: &AtomicBool,
) -> Result<Tree> {
    let mut tree = Tree {
        entries: BTreeMap::new(),
    };
    let mut pending = vec![(String::new(), HeldEntry::Directory(root))];
    let mut identities = BTreeSet::new();
    while let Some((relative, held)) = pending.pop() {
        stopped(stop)?;
        let stamp = metadata(held.as_file())?;
        verify_private(held.as_file(), stamp.directory)?;
        if !identities.insert(stamp.identity)
            || tree.entries.contains_key(&relative)
            || tree.entries.len() > MAX_ENTRIES
        {
            return Err(changed());
        }
        let node = if relative.is_empty() {
            Expected {
                kind: ArchiveEntryKind::Directory,
                bytes: &[],
            }
        } else {
            *expected.get(&relative).ok_or_else(changed)?
        };
        if stamp.directory != (node.kind == ArchiveEntryKind::Directory) {
            return Err(changed());
        }
        let (digest, children) = match &held {
            HeldEntry::Directory(directory) => {
                let children = names(directory, stop)?;
                for name in &children {
                    stopped(stop)?;
                    let child = if relative.is_empty() {
                        name.clone()
                    } else {
                        format!("{relative}/{name}")
                    };
                    let node = expected.get(&child).ok_or_else(changed)?;
                    let held = if node.kind == ArchiveEntryKind::Directory {
                        HeldEntry::Directory(directory.open_directory(name)?)
                    } else {
                        HeldEntry::File(Arc::new(directory.open_file(name)?))
                    };
                    pending.push((child, held));
                }
                (None, children)
            }
            HeldEntry::File(file) => {
                let (bytes, digest) = read_file(file, &stamp, node.bytes.len() as u64, stop)?;
                if bytes != node.bytes {
                    return Err(SessionError::UnverifiedMutation(
                        "archive extraction content changed",
                    ));
                }
                (Some(digest), Vec::new())
            }
        };
        if metadata(held.as_file())? != stamp {
            return Err(changed());
        }
        tree.entries.insert(
            relative,
            TreeEntry {
                stamp,
                digest,
                children,
                held,
            },
        );
    }
    if tree.entries.len() != expected.len() + 1 {
        return Err(SessionError::UnverifiedMutation(
            "archive extraction member inventory changed",
        ));
    }
    verify_tree_links(&tree, stop)?;
    Ok(tree)
}

fn compare_tree(expected: &Tree, actual: &Tree) -> Result<()> {
    if expected.entries.len() != actual.entries.len() {
        return Err(changed());
    }
    for (relative, before) in &expected.entries {
        let after = actual.entries.get(relative).ok_or_else(changed)?;
        if before.stamp != after.stamp
            || before.digest != after.digest
            || before.children != after.children
        {
            return Err(changed());
        }
        // The original read handles prevent identity reuse between the two
        // observations; namespace readback must still identify those objects.
        if metadata(before.held.as_file())? != after.stamp {
            return Err(changed());
        }
    }
    Ok(())
}

fn compare_published_observations(
    expected: &BTreeMap<String, PublishedObservation>,
    actual: &Tree,
) -> Result<()> {
    if expected.len() != actual.entries.len() {
        return Err(changed());
    }
    for (relative, before) in expected {
        let after = actual.entries.get(relative).ok_or_else(changed)?;
        let same_stamp = if relative.is_empty() {
            ancestor_stamp(before.stamp.clone())? == ancestor_stamp(after.stamp.clone())?
        } else {
            before.stamp == after.stamp
        };
        if !same_stamp || before.digest != after.digest || before.children != after.children {
            return Err(changed());
        }
    }
    Ok(())
}

struct Staged {
    entry: PrivateEntry,
    tree: Tree,
    unverified: UnverifiedStage,
}

fn named_stage(parent: &Directory, name: &str, entry: &PrivateEntry) -> Result<Directory> {
    parent.require_unique_name(name)?;
    verify_private(entry.as_file(), true)?;
    let named = parent.open_directory(name)?;
    verify_private(named.as_file(), true)?;
    if metadata(named.as_file())? != metadata(entry.as_file())? {
        return Err(changed());
    }
    Ok(named)
}

fn build_stage(
    reviewed: &Manifest,
    members: &[ArchiveMember],
    destination: &Path,
    staging: &Path,
    worker: &MutationWorker,
) -> Result<Staged> {
    worker.check()?;
    let expected = expected_tree(staging, members)?;
    if reviewed.destination.as_deref() != Some(destination)
        || destination.parent() != staging.parent()
        || alias(basename(destination)?) == alias(basename(staging)?)
    {
        return Err(changed());
    }
    let current = verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    let parent = parent_in(&current, destination)?;
    parent.require_absent(basename(destination)?)?;
    parent.require_absent(basename(staging)?)?;
    worker.check()?;
    // Arm before CREATE, whose internal post-create security check can fail
    // after a named object was created. Such evidence is never auto-deleted.
    let unverified = UnverifiedStage {
        ticket: Some(worker.ticket.clone()),
    };
    let entry = parent.create_directory(basename(staging)?)?;
    let root = entry.as_directory()?;
    let mut directories = BTreeMap::from([(String::new(), root)]);
    let mut created = BTreeMap::<String, (FileInfo, HeldEntry)>::new();
    created.insert(
        String::new(),
        (
            metadata(entry.as_file())?,
            HeldEntry::Directory(entry.as_directory()?),
        ),
    );
    let mut ordered: Vec<_> = expected
        .iter()
        .filter(|(_, node)| node.kind == ArchiveEntryKind::Directory)
        .map(|(path, _)| path)
        .collect();
    ordered.sort_by_key(|path| path.split('/').count());
    for relative in ordered {
        worker.check()?;
        verify_ancestors(&reviewed.ancestors, &worker.stop)?;
        verify_private(entry.as_file(), true)?;
        let (parent, name) = split_member(relative);
        let parent = directories.get(parent).ok_or_else(changed)?;
        let child = parent.create_directory(name)?;
        let held = child.as_directory()?;
        created.insert(
            relative.clone(),
            (
                metadata(child.as_file())?,
                HeldEntry::Directory(held.clone()),
            ),
        );
        directories.insert(relative.clone(), held);
        // Duplicate directory capabilities retain the same object identity
        // while the tree is populated.
        drop(child);
    }
    for (relative, node) in &expected {
        if node.kind != ArchiveEntryKind::File {
            continue;
        }
        worker.check()?;
        verify_ancestors(&reviewed.ancestors, &worker.stop)?;
        verify_private(entry.as_file(), true)?;
        let (parent, name) = split_member(relative);
        let parent = directories.get(parent).ok_or_else(changed)?;
        let mut child = parent.create_file(name)?;
        for chunk in node.bytes.chunks(64 * 1024) {
            worker.check()?;
            verify_private(entry.as_file(), true)?;
            verify_private(child.as_file(), false)?;
            child.write_all(chunk)?;
        }
        worker.check()?;
        child.sync_all()?;
        verify_private(child.as_file(), false)?;
        // Observation handles share ALL, so retaining them cannot block the
        // eventual directory rename after this creation handle is closed.
        let held = parent.open_file(name)?;
        let written = metadata(child.as_file())?;
        if metadata(&held)? != written {
            return Err(changed());
        }
        drop(child);
        // Windows may defer last-write timestamps until its writing handles
        // close. The read capability pins identity while we take that baseline.
        let stamp = metadata(&held)?;
        verify_private(&held, false)?;
        if stamp.identity != written.identity
            || read_file(&held, &stamp, node.bytes.len() as u64, &worker.stop)?.0 != node.bytes
        {
            return Err(changed());
        }
        created.insert(relative.clone(), (stamp, HeldEntry::File(Arc::new(held))));
    }
    worker.check()?;
    // Replace duplicate directory creation capabilities with independent READ
    // observations before the complete snapshot. Directory creation already
    // uses READ + SHARE_ALL; publication's short DELETE lease stays in platform.
    for (relative, (stamp, held)) in &mut created {
        worker.check()?;
        if stamp.directory {
            let read = if relative.is_empty() {
                named_stage(parent, basename(staging)?, &entry)?
            } else {
                let (parent, name) = split_member(relative);
                directories
                    .get(parent)
                    .ok_or_else(changed)?
                    .open_directory(name)?
            };
            if ancestor_stamp(metadata(read.as_file())?)? != ancestor_stamp(stamp.clone())? {
                return Err(changed());
            }
            *held = HeldEntry::Directory(read);
        }
    }
    // Only the root PrivateEntry retains its original directory capability.
    drop(directories);
    let tree = capture_tree(
        named_stage(parent, basename(staging)?, &entry)?,
        &expected,
        &worker.stop,
    )?;
    if created.len() != tree.entries.len() {
        return Err(changed());
    }
    for (relative, (stamp, held)) in &created {
        let actual = &tree.entries.get(relative).ok_or_else(changed)?.stamp;
        if metadata(held.as_file())? != *actual
            || if stamp.directory {
                ancestor_stamp(stamp.clone())? != ancestor_stamp(actual.clone())?
            } else {
                stamp != actual
            }
        {
            return Err(changed());
        }
    }
    drop(created);
    verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    worker.check()?;
    Ok(Staged {
        entry,
        tree,
        unverified,
    })
}

fn publish_stage(
    stage: Staged,
    reviewed: &Manifest,
    members: &[ArchiveMember],
    destination: &Path,
    staging: &Path,
    worker: &MutationWorker,
) -> Result<UnverifiedStage> {
    worker.check()?;
    let expected = expected_tree(staging, members)?;
    let current = verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    let parent = parent_in(&current, destination)?;
    parent.require_absent(basename(destination)?)?;
    let actual = capture_tree(
        named_stage(parent, basename(staging)?, &stage.entry)?,
        &expected,
        &worker.stop,
    )?;
    compare_tree(&stage.tree, &actual)?;
    drop(actual);
    let current = verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    let parent = parent_in(&current, destination)?;
    named_stage(parent, basename(staging)?, &stage.entry)?;
    parent.require_absent(basename(destination)?)?;
    worker.check()?;
    let baseline = release_tree_for_publication(stage.tree);
    worker.check()?;
    parent.rename_no_replace(&stage.entry, basename(destination)?)?;
    worker.check()?;
    let current = verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    let parent = parent_in(&current, destination)?;
    parent.require_absent(basename(staging)?)?;
    let published = named_stage(parent, basename(destination)?, &stage.entry)?;
    let actual = capture_tree(published, &expected, &worker.stop)?;
    // Descendants cannot remain open across the Windows root rename. Reopen
    // through the retained root and compare every original observation, digest
    // and complete byte sequence before considering publication verified.
    compare_published_observations(&baseline, &actual)?;
    let current = verify_ancestors(&reviewed.ancestors, &worker.stop)?;
    let parent = parent_in(&current, destination)?;
    parent.require_unique_name(basename(destination)?)?;
    parent.require_absent(basename(staging)?)?;
    named_stage(parent, basename(destination)?, &stage.entry)?;
    verify_tree_links(&actual, &worker.stop)?;
    worker.check()?;
    // The async owner disarms only after it receives this fully verified result.
    // If its await was abandoned, dropping the result still quarantines it.
    Ok(stage.unverified)
}

/// Stage a complete private tree, revalidate the remote review, then publish it.
pub(super) async fn extract(
    sftp: &SftpSession,
    plan: &ArchiveTransferPlan,
    raw: &RawSftpSession,
    destination: &Path,
    staging: &Path,
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
) -> Result<()> {
    let destination = checked_path(destination)?;
    let staging = checked_path(staging)?;
    validate_destinations(&destination, plan.members())?;
    validate_destinations(&staging, plan.members())?;
    let reviewed = plan.reviewed.clone();
    let build_destination = destination.clone();
    let build_staging = staging.clone();
    let stage = mutation_blocking(scope, authority, move |worker| {
        build_stage(
            &reviewed.local,
            reviewed.bundle.members(),
            &build_destination,
            &build_staging,
            worker,
        )
    })
    .await?;
    sftp.revalidate_archive(plan, raw, authority).await?;
    let reviewed = plan.reviewed.clone();
    let mut unverified = mutation_blocking(scope, authority, move |worker| {
        publish_stage(
            stage,
            &reviewed.local,
            reviewed.bundle.members(),
            &destination,
            &staging,
            worker,
        )
    })
    .await?;
    authority.check()?;
    unverified.ticket = None;
    Ok(())
}

#[cfg(all(test, windows))]
#[path = "local_windows/tests.rs"]
mod tests;
