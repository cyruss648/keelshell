//! Bounded Windows observations and private, handle-relative publication.
//!
//! Every reviewed ancestor and source remains open until its review is consumed.
//! Failed or cancelled mutations retain their names for the controller's shared
//! quarantine. A verified publication does not establish directory crash durability.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use keelshell_core::zmodem::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, validate_basename};
use keelshell_platform::{
    Directory, DirectoryStamp, FileInfo, PrivateEntry, WindowsPath, metadata,
    validate_windows_component, verify_private,
};
use sha2::{Digest, Sha256};

use super::{ZmodemFailure, ZmodemFileReview};

type Result<T> = std::result::Result<T, ZmodemFailure>;

/// Complete observed content stays private to the controller until confirmation.
pub(super) struct UploadPlan {
    sources: Vec<Source>,
}

struct Source {
    view: ZmodemFileReview,
    bytes: Vec<u8>,
    stamp: FileInfo,
    parent: Parents,
    // Retaining the actual object prevents file-ID reuse from reviving a review.
    descriptor: File,
}

/// The reviewed namespace and unpredictable, as-yet-uncreated staging name.
#[derive(Clone)]
pub(super) struct Destination {
    pub(super) path: PathBuf,
    pub(super) staging: PathBuf,
    parent: Parents,
}

#[derive(Clone)]
struct Parents {
    path: PathBuf,
    stamps: Vec<DirectoryStamp>,
    // Keep the entire chain, including the drive root, against identity reuse.
    directories: Vec<Directory>,
}

/// Successful full readback of the exact new target, with its private DACL.
pub(super) struct Saved {
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

fn cancelled(stop: &AtomicBool) -> Result<()> {
    if stop.load(Ordering::Acquire) {
        Err(ZmodemFailure::Stale)
    } else {
        Ok(())
    }
}

fn invalid_path(error: std::io::Error) -> ZmodemFailure {
    if error.kind() == std::io::ErrorKind::Unsupported {
        ZmodemFailure::Unsupported
    } else {
        ZmodemFailure::InvalidPath
    }
}

fn local_io(_: std::io::Error) -> ZmodemFailure {
    ZmodemFailure::LocalIo
}

fn unverified(_: std::io::Error) -> ZmodemFailure {
    ZmodemFailure::UnverifiedPublication
}

fn publication_error(error: std::io::Error) -> ZmodemFailure {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        ZmodemFailure::DestinationExists
    } else {
        ZmodemFailure::UnverifiedPublication
    }
}

fn alias(name: &str) -> String {
    keelshell_platform::filename_alias(name)
}

fn path(path: &Path) -> Result<&str> {
    let text = path.to_str().ok_or(ZmodemFailure::InvalidPath)?;
    let parsed = WindowsPath::parse(text).map_err(invalid_path)?;
    if parsed.components().len() == 0 || path.file_name().is_none() {
        return Err(ZmodemFailure::InvalidPath);
    }
    Ok(text)
}

fn basename(path: &Path) -> Result<&str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ZmodemFailure::InvalidPath)?;
    validate_windows_component(name).map_err(invalid_path)?;
    validate_basename(name).map_err(|_| ZmodemFailure::InvalidPath)?;
    Ok(name)
}

impl Parents {
    fn directory(&self) -> Result<&Directory> {
        self.directories.last().ok_or(ZmodemFailure::InvalidPath)
    }
}

fn parents(path: &Path) -> Result<Parents> {
    let parent = path.parent().ok_or(ZmodemFailure::InvalidPath)?;
    // open_chain checks each literal component and never follows a reparse point.
    let directories = Directory::open_chain(parent).map_err(invalid_path)?;
    let stamps = directories
        .iter()
        .map(Directory::stamp)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(invalid_path)?;
    Ok(Parents {
        path: parent.to_owned(),
        stamps,
        directories,
    })
}

fn revalidate_parents(path: &Path, expected: &Parents) -> Result<Directory> {
    let current = parents(path).map_err(|_| ZmodemFailure::SourceChanged)?;
    if current.path != expected.path || current.stamps != expected.stamps {
        return Err(ZmodemFailure::SourceChanged);
    }
    // Compare the retained objects as well as the newly resolved namespace.
    for (directory, stamp) in expected.directories.iter().zip(&expected.stamps) {
        if directory
            .stamp()
            .map_err(|_| ZmodemFailure::SourceChanged)?
            != *stamp
        {
            return Err(ZmodemFailure::SourceChanged);
        }
    }
    Ok(current.directory()?.clone())
}

fn read_source(path: &Path, bound: u64, stop: &AtomicBool) -> Result<Source> {
    cancelled(stop)?;
    self::path(path)?;
    let name = basename(path)?.to_owned();
    let parent = parents(path)?;
    let mut file = parent.directory()?.open_file(&name).map_err(invalid_path)?;
    // metadata rejects reparse points, pending deletion and non-single-link files.
    let stamp = metadata(&file).map_err(invalid_path)?;
    if stamp.bytes > MAX_FILE_BYTES || stamp.bytes > bound {
        return Err(ZmodemFailure::Limit);
    }
    revalidate_parents(path, &parent)?;
    let capacity = usize::try_from(stamp.bytes).map_err(|_| ZmodemFailure::Limit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ZmodemFailure::Limit)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancelled(stop)?;
        let count = file.read(&mut buffer).map_err(local_io)?;
        if count == 0 {
            break;
        }
        if bytes
            .len()
            .checked_add(count)
            .is_none_or(|end| end > capacity)
        {
            return Err(ZmodemFailure::SourceChanged);
        }
        digest.update(&buffer[..count]);
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != stamp.bytes
        || metadata(&file).map_err(|_| ZmodemFailure::SourceChanged)? != stamp
    {
        return Err(ZmodemFailure::SourceChanged);
    }
    let directory = revalidate_parents(path, &parent)?;
    let named = directory
        .open_file(&name)
        .map_err(|_| ZmodemFailure::SourceChanged)?;
    if metadata(&named).map_err(|_| ZmodemFailure::SourceChanged)? != stamp {
        return Err(ZmodemFailure::SourceChanged);
    }
    revalidate_parents(path, &parent)?;
    cancelled(stop)?;
    Ok(Source {
        view: ZmodemFileReview {
            path: path.to_owned(),
            name,
            size: stamp.bytes,
            sha256: Some(
                digest
                    .finalize()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            ),
        },
        bytes,
        stamp,
        parent,
        descriptor: file,
    })
}

impl UploadPlan {
    /// Observe all bounded files without granting sender or filesystem authority.
    pub(super) fn prepare(paths: Vec<PathBuf>, stop: &AtomicBool) -> Result<Self> {
        if paths.is_empty() || paths.len() > MAX_FILES {
            return Err(ZmodemFailure::Limit);
        }
        let mut sources = Vec::with_capacity(paths.len());
        let mut names = BTreeSet::new();
        let mut identities = BTreeSet::new();
        let mut total = 0u64;
        for path in paths {
            let source = read_source(&path, MAX_TOTAL_BYTES.saturating_sub(total), stop)?;
            if !names.insert(alias(&source.view.name)) || !identities.insert(source.stamp.identity)
            {
                return Err(ZmodemFailure::InvalidPath);
            }
            total = total
                .checked_add(source.stamp.bytes)
                .filter(|bytes| *bytes <= MAX_TOTAL_BYTES)
                .ok_or(ZmodemFailure::Limit)?;
            sources.push(source);
        }
        cancelled(stop)?;
        Ok(Self { sources })
    }

    /// Return only the exact names, paths, sizes and hashes for explicit review.
    pub(super) fn review(&self) -> Vec<ZmodemFileReview> {
        self.sources
            .iter()
            .map(|source| source.view.clone())
            .collect()
    }

    /// Re-read every source and release bytes only after the whole review matches.
    pub(super) fn confirm(
        self,
        stop: &AtomicBool,
    ) -> Result<Vec<keelshell_core::zmodem::SendFile>> {
        for source in &self.sources {
            cancelled(stop)?;
            revalidate_parents(&source.view.path, &source.parent)?;
            let current =
                read_source(&source.view.path, source.stamp.bytes, stop).map_err(|reason| {
                    match reason {
                        ZmodemFailure::Stale => reason,
                        _ => ZmodemFailure::SourceChanged,
                    }
                })?;
            if current.stamp != source.stamp
                || current.parent.path != source.parent.path
                || current.parent.stamps != source.parent.stamps
                || current.view != source.view
                || current.bytes != source.bytes
                || metadata(&source.descriptor).map_err(|_| ZmodemFailure::SourceChanged)?
                    != source.stamp
            {
                return Err(ZmodemFailure::SourceChanged);
            }
            revalidate_parents(&source.view.path, &source.parent)?;
        }
        cancelled(stop)?;
        self.sources
            .into_iter()
            .map(|source| {
                keelshell_core::zmodem::SendFile::new(source.view.name, source.bytes)
                    .map_err(|_| ZmodemFailure::InvalidPath)
            })
            .collect()
    }
}

fn absent(parent: &Directory, path: &Path) -> Result<()> {
    parent
        .require_absent(basename(path)?)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => ZmodemFailure::DestinationExists,
            std::io::ErrorKind::Unsupported => ZmodemFailure::Unsupported,
            _ => ZmodemFailure::LocalIo,
        })
}

impl Destination {
    /// Review an absent destination and staging name without changing the disk.
    pub(super) fn prepare(path: PathBuf, stop: &AtomicBool) -> Result<Self> {
        cancelled(stop)?;
        self::path(&path)?;
        basename(&path)?;
        let parent = parents(&path)?;
        let directory = revalidate_parents(&path, &parent)?;
        absent(&directory, &path)?;
        let staging = parent
            .path
            .join(format!(".keelshell-zmodem-{}.part", uuid::Uuid::new_v4()));
        self::path(&staging)?;
        absent(&directory, &staging)?;
        cancelled(stop)?;
        Ok(Self {
            path,
            staging,
            parent,
        })
    }

    /// Check the original ancestor chain and both uncreated names again.
    pub(super) fn revalidate(&self, stop: &AtomicBool) -> Result<Directory> {
        cancelled(stop)?;
        let parent = revalidate_parents(&self.path, &self.parent)?;
        absent(&parent, &self.path)?;
        absent(&parent, &self.staging)?;
        cancelled(stop)?;
        Ok(parent)
    }

    fn create_staging(
        &self,
        parent: &Directory,
        stop: &AtomicBool,
        mutated: &AtomicBool,
    ) -> Result<PrivateEntry> {
        let name = basename(&self.staging)?;
        cancelled(stop)?;
        // Even a failed create can leave a private entry after a failed native
        // postcondition. Never clear the shared reservation after this point.
        mutated.store(true, Ordering::Release);
        parent.create_file(name).map_err(publication_error)
    }

    /// Verify complete private staging and publish its retained creation handle.
    ///
    /// Any error after the creation attempt leaves `mutated` set and retains
    /// both reviewed paths for quarantine; this backend never deletes a name.
    pub(super) fn publish(
        self,
        bytes: Vec<u8>,
        stop: &AtomicBool,
        mutated: &AtomicBool,
    ) -> Result<Saved> {
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(ZmodemFailure::Limit);
        }
        let parent = self.revalidate(stop)?;
        let mut staging = self.create_staging(&parent, stop, mutated)?;
        let stamp = write_staging(&mut staging, &bytes, stop)?;
        self.publish_staging(&parent, &staging, &stamp, &bytes, stop)
    }

    fn publish_staging(
        self,
        parent: &Directory,
        staging: &PrivateEntry,
        stamp: &FileInfo,
        bytes: &[u8],
        stop: &AtomicBool,
    ) -> Result<Saved> {
        cancelled(stop)?;
        let stage_name = basename(&self.staging)?;
        let destination_name = basename(&self.path)?;
        verify_private(staging.as_file(), false).map_err(unverified)?;
        if metadata(staging.as_file()).map_err(unverified)? != *stamp
            || stamp.bytes != bytes.len() as u64
        {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        let current_parent = revalidate_parents(&self.path, &self.parent)?;
        absent(&current_parent, &self.path)?;
        let named = parent.open_file(stage_name).map_err(unverified)?;
        if metadata(&named).map_err(unverified)? != *stamp {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        verify_private(&named, false).map_err(unverified)?;
        parent.require_unique_name(stage_name).map_err(unverified)?;
        // An observation handle shares all access. Keep the original private
        // creation handle (sharing only reads) as the sole writer and renamer.
        drop(named);
        cancelled(stop)?;
        current_parent
            .rename_no_replace(staging, destination_name)
            .map_err(publication_error)?;
        // File data was flushed before rename. No directory flush or crash
        // durability is inferred from a successful Windows namespace operation.
        self.verify_publication(staging, stamp, bytes, stop)
    }

    fn verify_publication(
        self,
        staging: &PrivateEntry,
        stamp: &FileInfo,
        bytes: &[u8],
        stop: &AtomicBool,
    ) -> Result<Saved> {
        cancelled(stop)?;
        let stage_name = basename(&self.staging)?;
        let destination_name = basename(&self.path)?;
        let published_parent = revalidate_parents(&self.path, &self.parent)?;
        let mut published = published_parent
            .open_file(destination_name)
            .map_err(unverified)?;
        let actual = metadata(&published).map_err(unverified)?;
        // Rename may change timestamps. The retained creation object, its
        // identity and private DACL must agree with the exact reopened target.
        if actual.identity != stamp.identity
            || actual.bytes != bytes.len() as u64
            || metadata(staging.as_file()).map_err(unverified)? != actual
        {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        verify_private(&published, false).map_err(unverified)?;
        verify_content(&mut published, bytes, stop)?;
        if metadata(&published).map_err(unverified)? != actual {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        verify_private(&published, false).map_err(unverified)?;
        let sha256 = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let final_parent = revalidate_parents(&self.path, &self.parent)?;
        final_parent
            .require_unique_name(destination_name)
            .map_err(unverified)?;
        final_parent
            .require_absent(stage_name)
            .map_err(unverified)?;
        let named = final_parent
            .open_file(destination_name)
            .map_err(unverified)?;
        if metadata(&named).map_err(unverified)? != actual
            || metadata(staging.as_file()).map_err(unverified)? != actual
        {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        verify_private(&named, false).map_err(unverified)?;
        revalidate_parents(&self.path, &self.parent)?;
        cancelled(stop)?;
        Ok(Saved {
            path: self.path,
            bytes: bytes.len() as u64,
            sha256,
        })
    }
}

fn write_staging(staging: &mut PrivateEntry, bytes: &[u8], stop: &AtomicBool) -> Result<FileInfo> {
    cancelled(stop)?;
    verify_private(staging.as_file(), false).map_err(unverified)?;
    let before = metadata(staging.as_file()).map_err(unverified)?;
    if before.bytes != 0 {
        return Err(ZmodemFailure::UnverifiedPublication);
    }
    for chunk in bytes.chunks(64 * 1024) {
        cancelled(stop)?;
        staging.write_all(chunk).map_err(unverified)?;
    }
    staging.flush().map_err(unverified)?;
    staging.sync_all().map_err(unverified)?;
    verify_private(staging.as_file(), false).map_err(unverified)?;
    let stamp = metadata(staging.as_file()).map_err(unverified)?;
    if stamp.identity != before.identity || stamp.bytes != bytes.len() as u64 {
        return Err(ZmodemFailure::UnverifiedPublication);
    }
    verify_content(staging, bytes, stop)?;
    if metadata(staging.as_file()).map_err(unverified)? != stamp {
        return Err(ZmodemFailure::UnverifiedPublication);
    }
    verify_private(staging.as_file(), false).map_err(unverified)?;
    cancelled(stop)?;
    Ok(stamp)
}

fn verify_content(file: &mut (impl Read + Seek), bytes: &[u8], stop: &AtomicBool) -> Result<()> {
    file.seek(SeekFrom::Start(0)).map_err(unverified)?;
    let mut offset = 0usize;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancelled(stop)?;
        let count = file.read(&mut buffer).map_err(unverified)?;
        if count == 0 {
            break;
        }
        let end = offset
            .checked_add(count)
            .filter(|end| *end <= bytes.len())
            .ok_or(ZmodemFailure::UnverifiedPublication)?;
        if bytes[offset..end] != buffer[..count] {
            return Err(ZmodemFailure::UnverifiedPublication);
        }
        offset = end;
    }
    if offset != bytes.len() {
        return Err(ZmodemFailure::UnverifiedPublication);
    }
    cancelled(stop)
}

#[cfg(test)]
#[path = "local_windows/tests.rs"]
mod tests;
