//! Windows ABI calls stay below the safe filesystem boundary.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::{WindowsPath, validate_windows_component};

mod native;
mod security;

/// Stable, handle-observed filesystem object identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identity {
    /// Serial number reported for the actual opened volume.
    pub volume: u64,
    /// Complete 128-bit file identifier reported by the opened handle.
    pub file: [u8; 16],
}

/// Directory review invariants excluding timestamps changed by sibling writes.
#[derive(Clone, PartialEq, Eq)]
pub struct DirectoryStamp {
    identity: Identity,
    attributes: u32,
    security: Vec<u8>,
}

/// Metadata and access-control observation from the retained native handle.
#[derive(Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// Actual volume and file identity.
    pub identity: Identity,
    /// Logical file length, zero for directories.
    pub bytes: u64,
    /// Actual native object kind.
    pub directory: bool,
    /// Number of hard links reported by the native handle.
    pub links: u32,
    /// File attributes, with reparse points already rejected.
    pub attributes: u32,
    /// Native creation time, in Windows file-time units.
    pub created: i64,
    /// Native last-write time, excluding last-access changes caused by reading.
    pub modified: i64,
    /// Native change time, including metadata mutations.
    pub changed: i64,
    security: Vec<u8>,
}

impl fmt::Debug for DirectoryStamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectoryStamp")
            .field("identity", &self.identity)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for FileInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileInfo")
            .field("identity", &self.identity)
            .field("bytes", &self.bytes)
            .field("directory", &self.directory)
            .field("links", &self.links)
            .field("attributes", &self.attributes)
            .field("created", &self.created)
            .field("modified", &self.modified)
            .field("changed", &self.changed)
            .finish_non_exhaustive()
    }
}

impl FileInfo {
    /// Capture identity, attributes and access control for a directory review.
    pub fn directory_stamp(&self) -> io::Result<DirectoryStamp> {
        if !self.directory {
            return Err(invalid("expected a directory handle"));
        }
        Ok(DirectoryStamp {
            identity: self.identity,
            attributes: self.attributes,
            security: self.security.clone(),
        })
    }
}

/// A directory capability resolved without following a reparse point.
///
/// Methods accept individual checked names and use the retained handle as their
/// root. Namespace revalidation and mutation reservations remain the caller's
/// responsibility. Dropping a capability only closes its handle.
#[derive(Clone, Debug)]
pub struct Directory {
    file: Arc<File>,
}

impl Directory {
    /// Open the drive root and every descendant directory separately.
    ///
    /// The returned chain includes the root and retains all observed objects,
    /// allowing a review to compare ancestor identities without inode reuse.
    pub fn open_chain(path: &Path) -> io::Result<Vec<Self>> {
        let parsed = WindowsPath::parse(
            path.to_str()
                .ok_or_else(|| invalid("local path is not Unicode"))?,
        )?;
        let root = Self::from_file(native::open_root(parsed.drive())?)?;
        let mut chain = vec![root];
        for name in parsed.components() {
            let parent = chain
                .last()
                .ok_or_else(|| invalid("missing local drive capability"))?;
            let child = parent.open_directory(name)?;
            chain.push(child);
        }
        Ok(chain)
    }

    // Only the native synchronous opener and PrivateEntry can create directory
    // capabilities. Accepting an arbitrary File would lose both local-volume
    // provenance and the synchronous NT buffer-lifetime invariant.
    fn from_file(file: File) -> io::Result<Self> {
        metadata(&file)?.directory_stamp()?;
        Ok(Self {
            file: Arc::new(file),
        })
    }

    /// Borrow the held descriptor for observations, without transferring it.
    pub fn as_file(&self) -> &File {
        &self.file
    }

    /// Inspect the actual held directory.
    pub fn stamp(&self) -> io::Result<DirectoryStamp> {
        metadata(&self.file)?.directory_stamp()
    }

    /// Open one existing child directory without following reparse points.
    pub fn open_directory(&self, name: &str) -> io::Result<Self> {
        validate_windows_component(name)?;
        Self::from_file(native::open_child(&self.file, name, true)?)
    }

    /// Open one existing single-link regular file for observation.
    pub fn open_file(&self, name: &str) -> io::Result<File> {
        validate_windows_component(name)?;
        let file = native::open_child(&self.file, name, false)?;
        if metadata(&file)?.directory {
            return Err(invalid("expected a regular file"));
        }
        Ok(file)
    }

    /// Create a new private regular file, atomically refusing an existing name.
    ///
    /// The owner-only protected DACL is supplied to the create syscall. A failed
    /// post-create verification leaves the named object for the caller's
    /// quarantine; this function never deletes an uncertain result.
    pub fn create_file(&self, name: &str) -> io::Result<PrivateEntry> {
        self.create(name, false)
    }

    /// Create a new private directory with the DACL set by the create syscall.
    ///
    /// The retained directory capability has read access and shares all access.
    /// It can serve as a rename target without retaining conflicting DELETE
    /// access. Publication obtains its source lease from the actual held object.
    pub fn create_directory(&self, name: &str) -> io::Result<PrivateEntry> {
        self.create(name, true)
    }

    fn create(&self, name: &str, directory: bool) -> io::Result<PrivateEntry> {
        validate_windows_component(name)?;
        self.require_absent(name)?;
        let security = security::PrivateSecurity::new(directory)?;
        let file = native::create_private(&self.file, name, directory, &security)?;
        verify_private(&file, directory)?;
        if !directory && metadata(&file)?.bytes != 0 {
            return Err(invalid("new private file was not empty"));
        }
        Ok(PrivateEntry { file, directory })
    }

    /// Reopen a private staging object for publication through its exact handle.
    ///
    /// This does not establish that the caller created the object. Its expected
    /// identity and mutation reservation must be checked by the caller.
    pub fn open_private_entry(&self, name: &str, directory: bool) -> io::Result<PrivateEntry> {
        validate_windows_component(name)?;
        let file = native::open_private(&self.file, name, directory)?;
        verify_private(&file, directory)?;
        Ok(PrivateEntry { file, directory })
    }

    /// Enumerate bounded names through an independent native directory cursor.
    pub fn entries(&self) -> io::Result<Vec<String>> {
        native::entries(&self.file)
    }

    /// Refuse both an existing name and portable Unicode case aliases.
    pub fn require_absent(&self, name: &str) -> io::Result<()> {
        validate_windows_component(name)?;
        let requested = alias(name);
        if self
            .entries()?
            .iter()
            .any(|entry| alias(entry) == requested)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "local destination or case alias already exists",
            ));
        }
        Ok(())
    }

    /// Verify the exact published spelling is the sole portable case alias.
    pub fn require_unique_name(&self, name: &str) -> io::Result<()> {
        validate_windows_component(name)?;
        let requested = alias(name);
        let names = self.entries()?;
        let mut aliases = names.iter().filter(|entry| alias(entry) == requested);
        if aliases.next().is_none_or(|entry| entry != name) || aliases.next().is_some() {
            return Err(invalid("published local name changed or has a case alias"));
        }
        Ok(())
    }

    /// Rename the retained staging object without replacing an existing target.
    ///
    /// The syscall receives one checked child name and this directory handle.
    /// Success is only a namespace result; callers must verify the published
    /// identity, access control, complete bytes and their original ancestor chain.
    pub fn rename_no_replace(&self, source: &PrivateEntry, name: &str) -> io::Result<()> {
        validate_windows_component(name)?;
        self.require_absent(name)?;
        verify_private(&source.file, source.directory)?;
        let directory_lease = if source.directory {
            Some(native::directory_rename_handle(&source.file)?)
        } else {
            None
        };
        native::rename_no_replace(
            directory_lease.as_ref().unwrap_or(&source.file),
            &self.file,
            name,
        )
    }
}

/// A private synchronous object retained across creation and publication.
///
/// Its descriptor cannot be replaced through the safe API. This keeps native
/// rename buffers synchronous and ensures that rename addresses the held object.
/// Files retain their sole writer/DELETE creation handle. Directories retain a
/// read capability; publication opens a short-lived source lease by that handle.
/// Dropping it closes the handle and leaves any named file or directory intact.
#[derive(Debug)]
pub struct PrivateEntry {
    file: File,
    directory: bool,
}

impl PrivateEntry {
    /// Borrow the exact retained object for identity or DACL verification.
    pub fn as_file(&self) -> &File {
        &self.file
    }

    /// Borrow a directory capability for creating or checking private children.
    pub fn as_directory(&self) -> io::Result<Directory> {
        if !self.directory {
            return Err(invalid("private object is not a directory"));
        }
        Directory::from_file(self.file.try_clone()?)
    }

    /// Flush the retained regular file before publication.
    ///
    /// Directory crash durability is not inferred from a namespace success.
    pub fn sync_all(&self) -> io::Result<()> {
        if self.directory {
            return Err(invalid("directory flush is not a file durability proof"));
        }
        self.file.sync_all()
    }
}

impl Read for PrivateEntry {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Write for PrivateEntry {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for PrivateEntry {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

/// Inspect identity, kind, link count, timestamps and security through a handle.
///
/// Reparse points, non-disk objects, deletion-pending objects and hard-linked
/// regular files are rejected rather than approximated with size/timestamps.
pub fn metadata(file: &File) -> io::Result<FileInfo> {
    native::metadata(file)
}

/// Verify protected owner-only access on the exact opened filesystem object.
pub fn verify_private(file: &File, directory: bool) -> io::Result<()> {
    if metadata(file)?.directory != directory {
        return Err(invalid("private local object changed kind"));
    }
    security::verify_private(file, directory)
}

fn alias(name: &str) -> String {
    name.chars()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests;
