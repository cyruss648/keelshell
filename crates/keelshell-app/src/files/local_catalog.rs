//! Bounded metadata-only browsing of an explicitly selected local directory.
//!
//! A listing is a display source, not an object or permission anchor. Transfers
//! must independently validate and authorize their source or destination.

use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

const MAX_ENTRIES: usize = 4096;
const MAX_NAME_BYTES: usize = 1024 * 1024;
const MAX_ELAPSED: Duration = Duration::from_secs(5);

#[cfg(windows)]
const WINDOWS_HIDDEN_ATTRIBUTE: u32 = 0x0000_0002;
#[cfg(windows)]
const WINDOWS_REPARSE_ATTRIBUTE: u32 = 0x0000_0400;

/// The type observed without following the entry's final symbolic link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LocalEntryKind {
    /// An ordinary directory; its children have not been enumerated.
    Directory,
    /// An ordinary file; its contents have not been opened.
    File,
    /// A symbolic link, including a link whose target does not exist.
    Symlink,
    /// Another object, including unrecognized Windows reparse points.
    Other,
}

/// Native names and metadata for one immediate directory entry.
///
/// Paths retain the caller's native encoding. Neither the path nor the observed
/// type authorizes later navigation, transfer, or mutation; each operation must
/// perform its own validation because filesystem objects can change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalEntry {
    /// Immediate native filename, without a lossy display conversion.
    pub(super) name: OsString,
    /// The explicitly supplied directory joined with the immediate filename.
    pub(super) path: PathBuf,
    /// Object type observed using metadata that does not follow final links.
    pub(super) kind: LocalEntryKind,
    /// Byte length for ordinary files only.
    pub(super) size: Option<u64>,
    /// Whole seconds since the Unix epoch, if supported and nonnegative.
    pub(super) modified: Option<u64>,
    /// Unix dot-name or Windows hidden-attribute status.
    pub(super) hidden: bool,
}

/// A complete, unsorted, single-level listing within the browsing budgets.
///
/// The directory preserves the supplied native path. The before/after metadata
/// check detects some changes; it is not a cross-platform exact object identity
/// or permission anchor, and cannot eliminate changes between filesystem calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalListing {
    /// The absolute directory explicitly supplied by the caller.
    pub(super) directory: PathBuf,
    /// Immediate entries only; there is no partial success on a budget failure.
    pub(super) entries: Vec<LocalEntry>,
}

/// A browsing failure that retains no customer path, filename, or I/O message.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum LocalBrowseError {
    /// Cancellation was observed before starting or after a filesystem call.
    #[error("local directory browsing was cancelled")]
    Cancelled,
    /// The caller supplied a relative path.
    #[error("local directory browsing requires an absolute path")]
    RelativePath,
    /// Parent components are unsupported because resolving them can cross links.
    #[error("local directory browsing does not accept parent path components")]
    UnsupportedPath,
    /// The selected root is a final symbolic link or Windows reparse point.
    #[error("the local browsing root is a symbolic link or reparse point")]
    RootLink,
    /// The selected root is not an ordinary directory.
    #[error("the local browsing root is not a directory")]
    NotDirectory,
    /// The directory's observed metadata changed while it was enumerated.
    #[error("the local directory changed during browsing")]
    Changed,
    /// More than 4096 immediate entries were observed.
    #[error("the local directory exceeds the entry browsing limit")]
    EntryLimit,
    /// Native encoded filenames exceeded a combined 1 MiB.
    #[error("the local directory exceeds the filename browsing limit")]
    NameLimit,
    /// Five seconds elapsed at a cooperative checkpoint.
    #[error("the local directory browsing time budget was exhausted")]
    TimedOut,
    /// Metadata for the directory could not be observed.
    #[error("local directory metadata is unavailable ({0:?})")]
    RootMetadata(io::ErrorKind),
    /// The directory iterator could not be opened.
    #[error("the local directory cannot be listed ({0:?})")]
    DirectoryRead(io::ErrorKind),
    /// An immediate directory entry could not be obtained.
    #[error("a local directory entry is unavailable ({0:?})")]
    EntryRead(io::ErrorKind),
    /// Metadata for an immediate entry could not be observed.
    #[error("local entry metadata is unavailable ({0:?})")]
    EntryMetadata(io::ErrorKind),
}

/// Observe one explicitly supplied absolute directory without opening contents.
///
/// Call this blocking function on a worker, away from the UI thread. It performs
/// one-level enumeration and metadata reads only, never discovers a home or
/// default directory, and never reads environment, authentication, or contents.
/// Entry links are displayed without following their targets. The final root
/// link or Windows reparse point is rejected, including spellings ending in a
/// separator or `/.`; ordinary prefix links remain possible.
///
/// At most 4096 entries and 1 MiB of native encoded filenames are returned.
/// Cancellation and a five-second elapsed budget are checked before and after
/// filesystem calls and for each entry. These checks are cooperative: they do
/// not interrupt an individual blocked filesystem syscall. Cancellation already
/// set on entry takes precedence over path validation and all filesystem I/O.
///
/// A before/after metadata comparison rejects observed root changes. This does
/// not provide a cross-platform exact inode or permission anchor. The returned
/// paths are a display source and confer no transfer or mutation authority.
///
/// # Errors
///
/// Returns a typed error for cancellation, unsupported or non-directory roots,
/// observed changes, exhausted budgets, or metadata/enumeration failures. No
/// partial listing is returned and error values contain no supplied path data.
pub(super) fn list_directory(
    path: &Path,
    stop: &AtomicBool,
) -> Result<LocalListing, LocalBrowseError> {
    let started = Instant::now();
    checkpoint(stop, started)?;
    let metadata_path = metadata_root_path(path)?;

    checkpoint(stop, started)?;
    let before_result = fs::symlink_metadata(&metadata_path);
    checkpoint(stop, started)?;
    let before = before_result.map_err(|error| LocalBrowseError::RootMetadata(error.kind()))?;
    validate_root(&before)?;
    let stamp = DirectoryStamp::from_metadata(&before);

    checkpoint(stop, started)?;
    let iterator_result = fs::read_dir(&metadata_path);
    checkpoint(stop, started)?;
    let mut iterator =
        iterator_result.map_err(|error| LocalBrowseError::DirectoryRead(error.kind()))?;
    let mut entries = Vec::new();
    let mut name_bytes = 0;

    loop {
        checkpoint(stop, started)?;
        let next = iterator.next();
        checkpoint(stop, started)?;
        let Some(entry_result) = next else {
            break;
        };
        let entry = entry_result.map_err(|error| LocalBrowseError::EntryRead(error.kind()))?;
        if entries.len() >= MAX_ENTRIES {
            return Err(LocalBrowseError::EntryLimit);
        }
        let name = entry.file_name();
        name_bytes = add_name_bytes(name_bytes, &name)?;
        let entry_path = path.join(&name);

        checkpoint(stop, started)?;
        let metadata_result = fs::symlink_metadata(&entry_path);
        checkpoint(stop, started)?;
        let metadata =
            metadata_result.map_err(|error| LocalBrowseError::EntryMetadata(error.kind()))?;
        let kind = entry_kind(&metadata);
        entries.push(LocalEntry {
            hidden: is_hidden(&name, &metadata),
            name,
            path: entry_path,
            kind,
            size: (kind == LocalEntryKind::File).then_some(metadata.len()),
            modified: modified_seconds(&metadata),
        });
    }

    checkpoint(stop, started)?;
    let after_result = fs::symlink_metadata(&metadata_path);
    checkpoint(stop, started)?;
    let after = after_result.map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            LocalBrowseError::Changed
        } else {
            LocalBrowseError::RootMetadata(error.kind())
        }
    })?;
    ensure_unchanged(&stamp, &after)?;
    checkpoint(stop, started)?;

    Ok(LocalListing {
        directory: path.to_path_buf(),
        entries,
    })
}

fn checkpoint(stop: &AtomicBool, started: Instant) -> Result<(), LocalBrowseError> {
    if stop.load(Ordering::Acquire) {
        return Err(LocalBrowseError::Cancelled);
    }
    if started.elapsed() >= MAX_ELAPSED {
        return Err(LocalBrowseError::TimedOut);
    }
    Ok(())
}

fn metadata_root_path(path: &Path) -> Result<PathBuf, LocalBrowseError> {
    if !path.is_absolute() {
        return Err(LocalBrowseError::RelativePath);
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(LocalBrowseError::UnsupportedPath);
    }
    // Removing trailing separators and `.` prevents a final link from becoming
    // a followed directory solely because of its supplied spelling. No `..`
    // is collapsed, since it could resolve differently through a prefix link.
    Ok(path.components().collect())
}

fn validate_root(metadata: &Metadata) -> Result<(), LocalBrowseError> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(LocalBrowseError::RootLink);
    }
    if !metadata.is_dir() {
        return Err(LocalBrowseError::NotDirectory);
    }
    Ok(())
}

fn add_name_bytes(current: usize, name: &OsStr) -> Result<usize, LocalBrowseError> {
    let total = current
        .checked_add(name.as_encoded_bytes().len())
        .ok_or(LocalBrowseError::NameLimit)?;
    if total > MAX_NAME_BYTES {
        return Err(LocalBrowseError::NameLimit);
    }
    Ok(total)
}

fn entry_kind(metadata: &Metadata) -> LocalEntryKind {
    if metadata.file_type().is_symlink() {
        LocalEntryKind::Symlink
    } else if is_reparse_point(metadata) {
        // Unknown reparse objects must not appear as navigable directories or
        // ordinary transferable files, even if their other type bits say so.
        LocalEntryKind::Other
    } else if metadata.is_dir() {
        LocalEntryKind::Directory
    } else if metadata.is_file() {
        LocalEntryKind::File
    } else {
        LocalEntryKind::Other
    }
}

fn modified_seconds(metadata: &Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(windows)]
fn is_hidden(_name: &OsStr, metadata: &Metadata) -> bool {
    windows_hidden_attributes(metadata.file_attributes())
}

#[cfg(windows)]
fn windows_hidden_attributes(attributes: u32) -> bool {
    attributes & WINDOWS_HIDDEN_ATTRIBUTE != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &OsStr, _metadata: &Metadata) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_attributes() & WINDOWS_REPARSE_ATTRIBUTE != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}

// Access time is excluded because enumeration itself may advance it.
#[derive(Debug, Eq, PartialEq)]
struct DirectoryStamp {
    len: u64,
    readonly: bool,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    uid: u32,
    #[cfg(unix)]
    gid: u32,
    #[cfg(unix)]
    changed: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    #[cfg(windows)]
    attributes: u32,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
}

impl DirectoryStamp {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            len: metadata.len(),
            readonly: metadata.permissions().readonly(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            mode: metadata.mode(),
            #[cfg(unix)]
            uid: metadata.uid(),
            #[cfg(unix)]
            gid: metadata.gid(),
            #[cfg(unix)]
            changed: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
            #[cfg(windows)]
            attributes: metadata.file_attributes(),
            #[cfg(windows)]
            creation_time: metadata.creation_time(),
            #[cfg(windows)]
            last_write_time: metadata.last_write_time(),
        }
    }
}

fn ensure_unchanged(before: &DirectoryStamp, after: &Metadata) -> Result<(), LocalBrowseError> {
    if validate_root(after).is_err() || before != &DirectoryStamp::from_metadata(after) {
        return Err(LocalBrowseError::Changed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn named_entry<'a>(listing: &'a LocalListing, name: &OsStr) -> io::Result<&'a LocalEntry> {
        listing
            .entries
            .iter()
            .find(|entry| entry.name.as_os_str() == name)
            .ok_or_else(|| io::Error::other("owned fixture entry is missing"))
    }

    #[test]
    fn empty_directory_returns_a_complete_empty_listing() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        assert_eq!(
            listing,
            LocalListing {
                directory: fixture.path().to_path_buf(),
                entries: Vec::new(),
            }
        );
        Ok(())
    }

    #[test]
    fn mixed_entries_are_listed_without_descending_into_a_child_directory() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let child = fixture.path().join("child");
        fs::create_dir(&child)?;
        fs::write(child.join("nested-only"), b"nested fixture")?;
        fs::write(fixture.path().join("file"), b"owned fixture")?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        assert_eq!(listing.entries.len(), 2);
        let child_entry = named_entry(&listing, OsStr::new("child"))?;
        assert_eq!(
            (child_entry.kind, child_entry.size),
            (LocalEntryKind::Directory, None)
        );
        assert_eq!(
            named_entry(&listing, OsStr::new("file"))?.kind,
            LocalEntryKind::File
        );
        assert!(
            listing
                .entries
                .iter()
                .all(|entry| entry.path.parent() == Some(fixture.path()))
        );
        Ok(())
    }

    #[test]
    fn regular_file_size_and_modification_time_match_its_metadata() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("ordinary");
        fs::write(&path, b"owned metadata fixture")?;
        let metadata = fs::symlink_metadata(&path)?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        let entry = named_entry(&listing, OsStr::new("ordinary"))?;
        assert_eq!(
            (entry.kind, entry.size, entry.modified),
            (
                LocalEntryKind::File,
                Some(metadata.len()),
                metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs()),
            )
        );
        Ok(())
    }

    #[test]
    fn cancellation_takes_precedence_over_missing_path_io() -> TestResult {
        let fixture = tempfile::tempdir()?;
        assert_eq!(
            list_directory(&fixture.path().join("absent"), &AtomicBool::new(true)),
            Err(LocalBrowseError::Cancelled)
        );
        Ok(())
    }

    #[test]
    fn cancellation_takes_precedence_over_relative_path_validation() {
        assert_eq!(
            list_directory(Path::new("relative"), &AtomicBool::new(true)),
            Err(LocalBrowseError::Cancelled)
        );
    }

    #[test]
    fn relative_paths_are_rejected_before_listing() {
        assert_eq!(
            list_directory(Path::new("relative"), &AtomicBool::new(false)),
            Err(LocalBrowseError::RelativePath)
        );
    }

    #[test]
    fn parent_components_are_rejected_without_collapsing_missing_prefixes() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("absent").join("..");
        assert_eq!(
            list_directory(&path, &AtomicBool::new(false)),
            Err(LocalBrowseError::UnsupportedPath)
        );
        Ok(())
    }

    #[test]
    fn regular_file_roots_are_rejected() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("ordinary");
        fs::write(&path, b"owned fixture")?;
        assert_eq!(
            list_directory(&path, &AtomicBool::new(false)),
            Err(LocalBrowseError::NotDirectory)
        );
        Ok(())
    }

    #[test]
    fn directory_and_entry_paths_preserve_the_explicit_native_spelling() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let directory = fixture.path().join(".");
        fs::write(fixture.path().join("ordinary"), b"owned fixture")?;
        let listing = list_directory(&directory, &AtomicBool::new(false))?;
        assert_eq!(listing.directory.as_os_str(), directory.as_os_str());
        assert_eq!(
            named_entry(&listing, OsStr::new("ordinary"))?
                .path
                .as_os_str(),
            directory.join("ordinary").as_os_str()
        );
        Ok(())
    }

    #[test]
    fn a_4097th_entry_rejects_the_whole_listing() -> TestResult {
        let fixture = tempfile::tempdir()?;
        for index in 0..=MAX_ENTRIES {
            fs::File::create(fixture.path().join(format!("entry-{index:04}")))?;
        }
        assert_eq!(
            list_directory(fixture.path(), &AtomicBool::new(false)),
            Err(LocalBrowseError::EntryLimit)
        );
        Ok(())
    }

    #[test]
    fn encoded_filename_budget_accepts_the_boundary_and_rejects_excess() {
        let name = OsStr::new("abc");
        assert_eq!(add_name_bytes(MAX_NAME_BYTES - 3, name), Ok(MAX_NAME_BYTES));
        assert_eq!(
            add_name_bytes(MAX_NAME_BYTES - 2, name),
            Err(LocalBrowseError::NameLimit)
        );
        assert_eq!(
            add_name_bytes(usize::MAX, name),
            Err(LocalBrowseError::NameLimit)
        );
    }

    #[test]
    fn elapsed_budget_is_checked_without_a_sleep() -> TestResult {
        let started = Instant::now()
            .checked_sub(MAX_ELAPSED)
            .ok_or_else(|| io::Error::other("test clock cannot represent elapsed budget"))?;
        assert_eq!(
            checkpoint(&AtomicBool::new(false), started),
            Err(LocalBrowseError::TimedOut)
        );
        Ok(())
    }

    #[test]
    fn cancellation_takes_precedence_over_an_exhausted_elapsed_budget() -> TestResult {
        let started = Instant::now()
            .checked_sub(MAX_ELAPSED)
            .ok_or_else(|| io::Error::other("test clock cannot represent elapsed budget"))?;
        assert_eq!(
            checkpoint(&AtomicBool::new(true), started),
            Err(LocalBrowseError::Cancelled)
        );
        Ok(())
    }

    #[test]
    fn root_replaced_by_a_regular_file_is_reported_as_changed() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let root = fixture.path().join("selected");
        fs::create_dir(&root)?;
        let stamp = DirectoryStamp::from_metadata(&fs::symlink_metadata(&root)?);
        fs::remove_dir(&root)?;
        fs::write(&root, b"replacement fixture")?;
        assert_eq!(
            ensure_unchanged(&stamp, &fs::symlink_metadata(&root)?),
            Err(LocalBrowseError::Changed)
        );
        Ok(())
    }

    #[test]
    fn io_errors_do_not_retain_owned_fixture_path_text() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let name = "owned-private-fixture-name";
        let result = list_directory(&fixture.path().join(name), &AtomicBool::new(false));
        let error = result
            .err()
            .ok_or_else(|| io::Error::other("missing owned fixture unexpectedly existed"))?;
        assert_eq!(
            error,
            LocalBrowseError::RootMetadata(io::ErrorKind::NotFound)
        );
        assert!(!format!("{error:?} {error}").contains(name));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_non_utf8_names_and_paths_are_retained_exactly() -> TestResult {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let fixture = tempfile::tempdir()?;
        let name = OsString::from_vec(b"native-\xff-name".to_vec());
        let path = fixture.path().join(&name);
        assert_eq!(name.as_bytes(), b"native-\xff-name");
        let metadata_path = metadata_root_path(&path)?;
        assert_eq!(
            metadata_path.as_os_str().as_bytes(),
            path.as_os_str().as_bytes()
        );
        match fs::File::create(&path) {
            Ok(_) => {}
            #[cfg(target_os = "macos")]
            Err(error) if error.raw_os_error() == Some(92) => {
                // The owned filesystem rejected these bytes before a file
                // existed. Check a typed scan failure without inventing a
                // listing; accepting filesystems retain the full checks below.
                let metadata_error = fs::symlink_metadata(&path)
                    .err()
                    .ok_or_else(|| io::Error::other("the rejected name must remain absent"))?;
                assert_eq!(error.raw_os_error(), Some(92));
                assert_eq!(metadata_error.kind(), io::ErrorKind::NotFound);
                assert_eq!(metadata_error.raw_os_error(), Some(2));
                let scan_error = list_directory(&path, &AtomicBool::new(false))
                    .err()
                    .ok_or_else(|| {
                        io::Error::other("scanner must reject the unrepresentable filesystem name")
                    })?;
                assert_eq!(
                    scan_error,
                    LocalBrowseError::RootMetadata(metadata_error.kind())
                );
                assert_eq!(
                    metadata_path.as_os_str().as_bytes(),
                    path.as_os_str().as_bytes()
                );
                assert_eq!(
                    path.file_name().map(OsStrExt::as_bytes),
                    Some(name.as_bytes())
                );
                eprintln!(
                    "owned macOS filename fixture: EILSEQ(92), native bytes retained, typed RootMetadata rejection"
                );
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        let entry = named_entry(&listing, &name)?;
        assert_eq!(entry.name.as_bytes(), name.as_bytes());
        assert_eq!(
            entry.path.as_os_str().as_bytes(),
            path.as_os_str().as_bytes()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_dot_names_are_marked_hidden_without_filtering_entries() -> TestResult {
        let fixture = tempfile::tempdir()?;
        fs::File::create(fixture.path().join(".hidden"))?;
        fs::File::create(fixture.path().join("visible"))?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        assert_eq!(listing.entries.len(), 2);
        assert!(named_entry(&listing, OsStr::new(".hidden"))?.hidden);
        assert!(!named_entry(&listing, OsStr::new("visible"))?.hidden);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_replacement_directory_is_reported_as_changed() -> TestResult {
        let fixture = tempfile::tempdir()?;
        let root = fixture.path().join("selected");
        fs::create_dir(&root)?;
        let stamp = DirectoryStamp::from_metadata(&fs::symlink_metadata(&root)?);
        fs::rename(&root, fixture.path().join("retired"))?;
        fs::create_dir(&root)?;
        assert_eq!(
            ensure_unchanged(&stamp, &fs::symlink_metadata(&root)?),
            Err(LocalBrowseError::Changed)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_directory_permission_changes_are_reported_as_changed() -> TestResult {
        use std::os::unix::fs::PermissionsExt;

        let fixture = tempfile::tempdir()?;
        let root = fixture.path().join("selected");
        fs::create_dir(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let stamp = DirectoryStamp::from_metadata(&fs::symlink_metadata(&root)?);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500))?;
        let after = fs::symlink_metadata(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        assert_eq!(
            ensure_unchanged(&stamp, &after),
            Err(LocalBrowseError::Changed)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_links_include_dangling_targets_without_following_them() -> TestResult {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir()?;
        fs::create_dir(fixture.path().join("directory"))?;
        fs::write(
            fixture.path().join("directory").join("nested"),
            b"owned fixture",
        )?;
        fs::write(fixture.path().join("file"), b"owned fixture")?;
        symlink("directory", fixture.path().join("directory-link"))?;
        symlink("file", fixture.path().join("file-link"))?;
        symlink("absent", fixture.path().join("dangling-link"))?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        assert_eq!(listing.entries.len(), 5);
        for name in ["directory-link", "file-link", "dangling-link"] {
            let entry = named_entry(&listing, OsStr::new(name))?;
            assert_eq!((entry.kind, entry.size), (LocalEntryKind::Symlink, None));
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_entry_links_include_dangling_targets_without_following_them() -> TestResult {
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let fixture = tempfile::tempdir()?;
        let directory = fixture.path().join("directory");
        let file = fixture.path().join("file");
        fs::create_dir(&directory)?;
        fs::write(directory.join("nested"), b"owned fixture")?;
        fs::write(&file, b"owned fixture")?;
        symlink_dir(&directory, fixture.path().join("directory-link"))?;
        symlink_file(&file, fixture.path().join("file-link"))?;
        symlink_file(
            fixture.path().join("absent"),
            fixture.path().join("dangling-link"),
        )?;
        let listing = list_directory(fixture.path(), &AtomicBool::new(false))?;
        assert_eq!(listing.entries.len(), 5);
        for name in ["directory-link", "file-link", "dangling-link"] {
            let entry = named_entry(&listing, OsStr::new(name))?;
            assert_eq!((entry.kind, entry.size), (LocalEntryKind::Symlink, None));
        }
        Ok(())
    }

    #[cfg(any(unix, windows))]
    fn root_link_fixture() -> io::Result<(tempfile::TempDir, PathBuf)> {
        let fixture = tempfile::tempdir()?;
        let target = fixture.path().join("target");
        let link = fixture.path().join("root-link");
        fs::create_dir(&target)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target, &link)?;
        Ok((fixture, link))
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn final_directory_link_roots_are_rejected() -> TestResult {
        let (_fixture, link) = root_link_fixture()?;
        assert_eq!(
            list_directory(&link, &AtomicBool::new(false)),
            Err(LocalBrowseError::RootLink)
        );
        Ok(())
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn final_directory_link_roots_with_a_trailing_separator_are_rejected() -> TestResult {
        let (_fixture, link) = root_link_fixture()?;
        let mut spelling = link.into_os_string();
        spelling.push(std::path::MAIN_SEPARATOR_STR);
        assert_eq!(
            list_directory(&PathBuf::from(spelling), &AtomicBool::new(false)),
            Err(LocalBrowseError::RootLink)
        );
        Ok(())
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn final_directory_link_roots_with_a_trailing_dot_are_rejected() -> TestResult {
        let (_fixture, link) = root_link_fixture()?;
        assert_eq!(
            list_directory(&link.join("."), &AtomicBool::new(false)),
            Err(LocalBrowseError::RootLink)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ordinary_prefix_links_remain_supported() -> TestResult {
        let (fixture, link) = root_link_fixture()?;
        fs::create_dir(fixture.path().join("target").join("selected"))?;
        fs::File::create(fixture.path().join("target").join("selected").join("file"))?;
        let directory = link.join("selected");
        let listing = list_directory(&directory, &AtomicBool::new(false))?;
        assert_eq!(listing.directory, directory);
        assert_eq!(listing.entries.len(), 1);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_hidden_attribute_is_respected_with_other_metadata_flags() {
        assert!(windows_hidden_attributes(0x0000_0022));
        assert!(!windows_hidden_attributes(0x0000_0020));
        assert!(!windows_hidden_attributes(0x0000_0080));
    }
}
