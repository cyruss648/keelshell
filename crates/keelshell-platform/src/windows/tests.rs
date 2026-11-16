#![cfg(test)]
#![cfg(windows)]

use super::*;

fn fixture() -> io::Result<(tempfile::TempDir, std::path::PathBuf, Directory)> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().canonicalize()?;
    let directory = Directory::open_chain(&path)?
        .pop()
        .ok_or_else(|| io::Error::other("missing test directory capability"))?;
    Ok((temporary, path, directory))
}

#[test]
fn drive_chain_keeps_every_literal_ancestor_and_reopens_the_same_object() -> io::Result<()> {
    let (_temporary, path, directory) = fixture()?;
    let chain = Directory::open_chain(&path)?;
    assert_eq!(
        chain.len(),
        WindowsPath::parse(path.to_str().ok_or_else(|| io::Error::other("test path"))?)?
            .components()
            .count()
            + 1
    );
    assert_eq!(
        chain
            .last()
            .ok_or_else(|| io::Error::other("test chain"))?
            .stamp()?,
        directory.stamp()?
    );
    Ok(())
}

#[test]
fn private_file_has_its_verified_dacl_before_the_first_content_write() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let mut entry = parent.create_file("收到的文件.bin")?;
    verify_private(entry.as_file(), false)?;
    assert_eq!(metadata(entry.as_file())?.bytes, 0);
    entry.write_all(&[0, 255, 13, 10, 128])?;
    entry.sync_all()?;
    entry.seek(SeekFrom::Start(0))?;
    let mut actual = Vec::new();
    entry.read_to_end(&mut actual)?;
    assert_eq!(actual, [0, 255, 13, 10, 128]);
    verify_private(entry.as_file(), false)?;
    Ok(())
}

#[test]
fn private_directory_and_child_creation_do_not_rely_on_default_permissions() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let entry = parent.create_directory("staging")?;
    verify_private(entry.as_file(), true)?;
    let directory = entry.as_directory()?;
    let mut child = directory.create_file("child")?;
    verify_private(child.as_file(), false)?;
    child.write_all(b"private before write")?;
    assert!(entry.sync_all().is_err());
    Ok(())
}

#[test]
fn exclusive_create_preserves_an_existing_file_directory_and_case_alias() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::write(path.join("existing"), b"external")?;
    std::fs::create_dir(path.join("directory"))?;
    for name in ["existing", "EXISTING", "directory"] {
        assert!(parent.create_file(name).is_err());
        assert!(parent.create_directory(name).is_err());
    }
    assert_eq!(std::fs::read(path.join("existing"))?, b"external");
    assert!(parent.open_directory("directory")?.entries()?.is_empty());
    Ok(())
}

#[test]
fn held_file_publication_retains_identity_and_leaves_no_old_name() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let mut entry = parent.create_file("stage")?;
    entry.write_all(b"reviewed complete content\0\xff")?;
    entry.sync_all()?;
    let before = metadata(entry.as_file())?;
    parent.rename_no_replace(&entry, "published")?;
    let mut published = parent.open_file("published")?;
    assert_eq!(metadata(&published)?.identity, before.identity);
    verify_private(&published, false)?;
    let mut bytes = Vec::new();
    published.read_to_end(&mut bytes)?;
    assert_eq!(bytes, b"reviewed complete content\0\xff");
    parent.require_absent("stage")?;
    parent.require_unique_name("published")?;
    Ok(())
}

#[test]
fn retained_object_rename_never_replaces_a_file_or_empty_directory() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::write(path.join("existing-file"), b"external owner")?;
    std::fs::create_dir(path.join("existing-directory"))?;
    let mut stage = parent.create_file("stage")?;
    stage.write_all(b"reviewed")?;
    for destination in ["existing-file", "existing-directory"] {
        // Exercise the actual no-replace syscall as well as the public precheck.
        assert!(native::rename_no_replace(stage.as_file(), parent.as_file(), destination).is_err());
        assert!(parent.rename_no_replace(&stage, destination).is_err());
        assert_eq!(std::fs::read(path.join("stage"))?, b"reviewed");
    }
    assert_eq!(
        std::fs::read(path.join("existing-file"))?,
        b"external owner"
    );
    assert!(
        std::fs::read_dir(path.join("existing-directory"))?
            .next()
            .is_none()
    );
    Ok(())
}

#[test]
fn complete_private_tree_is_published_through_its_retained_root() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let stage = parent.create_directory("stage")?;
    let tree = stage.as_directory()?;
    let nested = tree.create_directory("子目录")?;
    let nested_directory = nested.as_directory()?;
    let mut file = nested_directory.create_file("payload")?;
    file.write_all(b"binary\0\xff")?;
    file.sync_all()?;
    drop(file);
    drop(nested_directory);
    drop(nested);
    drop(tree);
    let identity = metadata(stage.as_file())?.identity;
    parent.rename_no_replace(&stage, "published")?;
    let published = parent.open_directory("published")?;
    assert_eq!(metadata(published.as_file())?.identity, identity);
    verify_private(published.as_file(), true)?;
    let nested = published.open_directory("子目录")?;
    verify_private(nested.as_file(), true)?;
    let mut file = nested.open_file("payload")?;
    verify_private(&file, false)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    assert_eq!(bytes, b"binary\0\xff");
    parent.require_absent("stage")?;
    Ok(())
}

#[test]
fn directory_no_replace_syscall_preserves_other_empty_directories() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::create_dir(path.join("existing"))?;
    let stage = parent.create_directory("stage")?;
    let source_lease = native::directory_rename_handle(stage.as_file())?;
    assert!(native::rename_no_replace(&source_lease, parent.as_file(), "existing").is_err());
    assert_eq!(
        parent.open_directory("stage")?.entries()?,
        Vec::<String>::new()
    );
    assert!(std::fs::read_dir(path.join("existing"))?.next().is_none());
    native::rename_no_replace(&source_lease, parent.as_file(), "published")?;
    assert_eq!(
        metadata(parent.open_directory("published")?.as_file())?.identity,
        metadata(stage.as_file())?.identity
    );
    Ok(())
}

#[test]
fn publication_into_a_retained_private_target_directory_supports_files_and_directories()
-> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let private_target = parent.create_directory("private-target")?;
    let target = private_target.as_directory()?;
    let mut file = parent.create_file("file-stage")?;
    file.write_all(b"private target content")?;
    file.sync_all()?;
    let file_identity = metadata(file.as_file())?.identity;
    target.rename_no_replace(&file, "file")?;
    assert_eq!(
        metadata(&target.open_file("file")?)?.identity,
        file_identity
    );
    let directory = parent.create_directory("directory-stage")?;
    let directory_identity = metadata(directory.as_file())?.identity;
    target.rename_no_replace(&directory, "directory")?;
    assert_eq!(
        metadata(target.open_directory("directory")?.as_file())?.identity,
        directory_identity
    );
    verify_private(private_target.as_file(), true)?;
    parent.require_absent("file-stage")?;
    parent.require_absent("directory-stage")?;
    Ok(())
}

fn rename_boundary_names(directory: bool) -> io::Result<()> {
    for name in ["x".to_owned(), format!("{}x", "🦀".repeat(127))] {
        let (_temporary, _path, parent) = fixture()?;
        let mut stage = if directory {
            parent.create_directory("stage")?
        } else {
            parent.create_file("stage")?
        };
        if !directory {
            stage.write_all(b"short and 255-unit destination")?;
            stage.sync_all()?;
        }
        let identity = metadata(stage.as_file())?.identity;
        parent.rename_no_replace(&stage, &name)?;
        let actual = if directory {
            metadata(parent.open_directory(&name)?.as_file())?
        } else {
            metadata(&parent.open_file(&name)?)?
        };
        assert_eq!(actual.identity, identity);
        parent.require_unique_name(&name)?;
        parent.require_absent("stage")?;
    }
    Ok(())
}

#[test]
fn file_rename_buffers_cover_one_and_255_utf16_unit_names() -> io::Result<()> {
    rename_boundary_names(false)
}

#[test]
fn directory_rename_buffers_cover_one_and_255_utf16_unit_names() -> io::Result<()> {
    rename_boundary_names(true)
}

#[test]
fn private_file_sharing_allows_readback_and_rejects_other_writers_and_delete_openers()
-> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::DELETE;

    let (_temporary, path, parent) = fixture()?;
    let mut entry = parent.create_file("stage")?;
    entry.write_all(b"sole writer")?;
    entry.sync_all()?;
    assert_eq!(std::fs::read(path.join("stage"))?, b"sole writer");
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(path.join("stage"))
            .is_err()
    );
    assert!(
        std::fs::OpenOptions::new()
            .access_mode(DELETE)
            .open(path.join("stage"))
            .is_err()
    );
    assert!(std::fs::remove_file(path.join("stage")).is_err());
    verify_private(entry.as_file(), false)?;
    Ok(())
}

#[test]
fn simultaneous_enumerations_have_independent_native_scan_positions() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    for name in ["first", "second", "文件", "fourth"] {
        std::fs::write(path.join(name), name.as_bytes())?;
    }
    let one = parent.clone();
    let two = parent.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let first_sender = sender.clone();
    let first = std::thread::spawn(move || {
        let _ = first_sender.send(one.entries());
    });
    let second = std::thread::spawn(move || {
        let _ = sender.send(two.entries());
    });
    let mut results = Vec::new();
    for _ in 0..2 {
        let mut names = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "directory enumeration"))??;
        names.sort();
        results.push(names);
    }
    first
        .join()
        .map_err(|_| io::Error::other("first enumeration worker"))?;
    second
        .join()
        .map_err(|_| io::Error::other("second enumeration worker"))?;
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0].len(), 4);
    Ok(())
}

// Return one non-dot entry without ever restarting the held native scan. The
// fixed fixture permits at most the two dot entries before a name or EOF.
fn next_cursor_name(cursor: &File) -> io::Result<Option<String>> {
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;
    use std::ptr;
    use windows_sys::Wdk::Storage::FileSystem::{FileNamesInformation, NtQueryDirectoryFile};
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    let mut storage = [0u64; 128];
    for _ in 0..3 {
        let mut status_block = IO_STATUS_BLOCK::default();
        // SAFETY: callers supply native synchronous directory handles. The
        // aligned 1 KiB buffer fits a complete maximum-length name record, and
        // the initialized buffer and IOSB stay live until synchronous completion.
        let status = unsafe {
            NtQueryDirectoryFile(
                cursor.as_raw_handle(),
                ptr::null_mut(),
                None,
                ptr::null(),
                &mut status_block,
                storage.as_mut_ptr().cast(),
                size_of::<[u64; 128]>() as u32,
                FileNamesInformation,
                true,
                ptr::null(),
                false,
            )
        };
        if status == 0x8000_0006_u32 as i32 {
            return Ok(None);
        }
        if status != 0 {
            return Err(io::Error::other(format!(
                "single-entry cursor query NTSTATUS=0x{:08x}",
                status as u32
            )));
        }
        let length = status_block.Information;
        if length == 0 || length > size_of::<[u64; 128]>() {
            return Err(io::Error::other("single-entry cursor response length"));
        }
        // SAFETY: length was checked against this initialized storage.
        let bytes = unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), length) };
        let mut names = Vec::new();
        crate::directory_records::decode(bytes, &mut names)?;
        if let Some(name) = names.pop() {
            assert!(
                names.is_empty(),
                "ReturnSingleEntry returned multiple names"
            );
            return Ok(Some(name));
        }
    }
    Err(io::Error::other("too many dot entries in cursor fixture"))
}

fn finish_cursor(cursor: &File, mut names: Vec<String>) -> io::Result<Vec<String>> {
    for _ in 0..5 {
        match next_cursor_name(cursor)? {
            Some(name) => names.push(name),
            None => {
                names.sort();
                return Ok(names);
            }
        }
    }
    Err(io::Error::other("cursor fixture did not reach EOF"))
}

#[test]
fn reopened_directory_cursors_do_not_advance_or_restart_other_live_scans() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    let expected = ["alpha", "bravo", "文件", "🦀"];
    for name in expected {
        std::fs::write(path.join(name), [])?;
    }
    let parent_first = next_cursor_name(parent.as_file())?
        .ok_or_else(|| io::Error::other("missing parent scan entry"))?;
    let first = native::directory_cursor(parent.as_file())?;
    let first_name =
        next_cursor_name(&first)?.ok_or_else(|| io::Error::other("missing first cursor entry"))?;
    let second = native::directory_cursor(parent.as_file())?;
    let mut expected: Vec<String> = expected.into_iter().map(str::to_owned).collect();
    expected.sort();
    // All three scans are live. Exhausting the second must leave both prior
    // positions unchanged. A DuplicateHandle-based opener fails these sets.
    assert_eq!(finish_cursor(&second, Vec::new())?, expected);
    assert_eq!(finish_cursor(&first, vec![first_name])?, expected);
    assert_eq!(
        finish_cursor(parent.as_file(), vec![parent_first])?,
        expected
    );
    Ok(())
}

#[test]
fn directory_cursor_keeps_the_held_identity_when_its_original_path_is_rebound() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::create_dir(path.join("held"))?;
    std::fs::write(path.join("held/original-only"), [])?;
    let held = parent.open_directory("held")?;
    let original = held.stamp()?;
    std::fs::rename(path.join("held"), path.join("moved"))?;
    std::fs::create_dir(path.join("held"))?;
    std::fs::write(path.join("held/replacement-only"), [])?;
    let cursor = native::directory_cursor(held.as_file())?;
    assert_eq!(metadata(&cursor)?.directory_stamp()?, original);
    assert_eq!(held.entries()?, ["original-only"]);
    assert_eq!(held.stamp()?, original);
    assert_eq!(
        parent.open_directory("held")?.entries()?,
        ["replacement-only"]
    );
    Ok(())
}

#[test]
fn directory_cursor_preserves_private_dacl_and_rejects_regular_files() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    let stage = parent.create_directory("private")?;
    let before = metadata(stage.as_file())?.directory_stamp()?;
    let cursor = native::directory_cursor(stage.as_file())?;
    verify_private(&cursor, true)?;
    assert_eq!(metadata(&cursor)?.directory_stamp()?, before);
    verify_private(stage.as_file(), true)?;
    std::fs::write(path.join("regular"), [])?;
    let regular = parent.open_file("regular")?;
    assert!(native::directory_cursor(&regular).is_err());
    Ok(())
}

#[test]
fn directory_enumeration_continues_after_a_full_native_query_buffer() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    let mut expected = Vec::new();
    for index in 0..1100 {
        let name = format!("entry-{index:04}-{}", "x".repeat(48));
        std::fs::write(path.join(&name), [])?;
        expected.push(name);
    }
    let mut actual = parent.entries()?;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn opening_a_hard_linked_file_fails_instead_of_using_size_as_identity() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::write(path.join("first"), b"same bytes")?;
    std::fs::hard_link(path.join("first"), path.join("second"))?;
    assert!(parent.open_file("first").is_err());
    assert!(parent.open_file("second").is_err());
    assert!(metadata(&File::open(path.join("first"))?).is_err());
    Ok(())
}

#[test]
fn directory_stamp_survives_sibling_writes_but_distinguishes_replacement() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    std::fs::create_dir(path.join("first"))?;
    std::fs::create_dir(path.join("second"))?;
    let first = parent.open_directory("first")?;
    let before = first.stamp()?;
    std::fs::write(path.join("first/child"), b"changed namespace")?;
    assert_eq!(before, first.stamp()?);
    assert_ne!(before, parent.open_directory("second")?.stamp()?);
    Ok(())
}

#[test]
fn existing_private_entry_can_be_reopened_only_after_the_creation_writer_closes() -> io::Result<()>
{
    let (_temporary, _path, parent) = fixture()?;
    let entry = parent.create_file("stage")?;
    let identity = metadata(entry.as_file())?.identity;
    // A second write/delete opener would violate the original share-read lease.
    assert!(parent.open_private_entry("stage", false).is_err());
    drop(entry);
    let reopened = parent.open_private_entry("stage", false)?;
    assert_eq!(metadata(reopened.as_file())?.identity, identity);
    parent.rename_no_replace(&reopened, "published")?;
    Ok(())
}

#[test]
fn component_relative_operations_reject_every_escape_before_native_io() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    for name in ["..", ".", r"a\b", "a/b", "a:stream", "CONOUT$", "NUL .txt"] {
        assert!(parent.open_file(name).is_err());
        assert!(parent.open_directory(name).is_err());
        assert!(parent.create_file(name).is_err());
        assert!(parent.create_directory(name).is_err());
        assert!(parent.require_absent(name).is_err());
    }
    assert!(parent.entries()?.is_empty());
    Ok(())
}

#[test]
fn security_observations_are_opaque_in_public_debug_output() -> io::Result<()> {
    let (_temporary, _path, parent) = fixture()?;
    let entry = parent.create_file("private")?;
    let information = metadata(entry.as_file())?;
    let debug = format!("{information:?}");
    assert!(debug.starts_with("FileInfo {"));
    assert!(!debug.contains("security"));
    let debug = format!("{:?}", parent.stamp()?);
    assert!(debug.starts_with("DirectoryStamp {"));
    assert!(!debug.contains("security"));
    Ok(())
}

#[test]
fn directory_publication_retains_the_source_rebound_before_its_rename_lease() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    let stage = parent.create_directory("stage")?;
    let mut original_file = stage.as_directory()?.create_file("original-only")?;
    original_file.write_all(b"original\0\xff")?;
    original_file.sync_all()?;
    drop(original_file);
    let original = metadata(stage.as_file())?.directory_stamp()?;

    // Rebind the source name before the public operation obtains its lease.
    std::fs::rename(path.join("stage"), path.join("moved"))?;
    let replacement = parent.create_directory("stage")?;
    let mut replacement_file = replacement
        .as_directory()?
        .create_file("replacement-only")?;
    replacement_file.write_all(b"replacement")?;
    replacement_file.sync_all()?;
    drop(replacement_file);
    let replacement_before = metadata(replacement.as_file())?.directory_stamp()?;
    assert_ne!(replacement_before, original);

    parent.rename_no_replace(&stage, "published")?;
    let published = parent.open_directory("published")?;
    assert_eq!(published.stamp()?, original);
    assert_eq!(metadata(stage.as_file())?.directory_stamp()?, original);
    verify_private(published.as_file(), true)?;
    assert_eq!(published.entries()?, ["original-only"]);
    assert_eq!(
        std::fs::read(path.join("published/original-only"))?,
        b"original\0\xff"
    );
    let untouched = parent.open_directory("stage")?;
    assert_eq!(untouched.stamp()?, replacement_before);
    assert_eq!(untouched.entries()?, ["replacement-only"]);
    assert_eq!(
        std::fs::read(path.join("stage/replacement-only"))?,
        b"replacement"
    );
    parent.require_absent("moved")?;
    parent.require_unique_name("published")?;
    Ok(())
}

#[test]
fn directory_rename_lease_retains_the_source_rebound_after_acquisition() -> io::Result<()> {
    let (_temporary, path, parent) = fixture()?;
    let stage = parent.create_directory("stage")?;
    let mut original_file = stage.as_directory()?.create_file("original-only")?;
    original_file.write_all(b"retained lease\0\xff")?;
    original_file.sync_all()?;
    drop(original_file);
    let original = metadata(stage.as_file())?.directory_stamp()?;
    let source_lease = native::directory_rename_handle(stage.as_file())?;
    verify_private(&source_lease, true)?;
    assert_eq!(metadata(&source_lease)?.directory_stamp()?, original);

    // Use the already granted DELETE lease to move the held object. A second
    // delete opener would instead test sharing, not publication after rebinding.
    native::rename_no_replace(&source_lease, parent.as_file(), "moved")?;
    let replacement = parent.create_directory("stage")?;
    let mut replacement_file = replacement
        .as_directory()?
        .create_file("replacement-only")?;
    replacement_file.write_all(b"replacement after lease")?;
    replacement_file.sync_all()?;
    drop(replacement_file);
    let replacement_before = metadata(replacement.as_file())?.directory_stamp()?;
    assert_ne!(replacement_before, original);

    native::rename_no_replace(&source_lease, parent.as_file(), "published")?;
    let published = parent.open_directory("published")?;
    assert_eq!(published.stamp()?, original);
    assert_eq!(metadata(&source_lease)?.directory_stamp()?, original);
    assert_eq!(metadata(stage.as_file())?.directory_stamp()?, original);
    verify_private(&source_lease, true)?;
    verify_private(published.as_file(), true)?;
    assert_eq!(published.entries()?, ["original-only"]);
    assert_eq!(
        std::fs::read(path.join("published/original-only"))?,
        b"retained lease\0\xff"
    );
    let untouched = parent.open_directory("stage")?;
    assert_eq!(untouched.stamp()?, replacement_before);
    assert_eq!(untouched.entries()?, ["replacement-only"]);
    assert_eq!(
        std::fs::read(path.join("stage/replacement-only"))?,
        b"replacement after lease"
    );
    parent.require_absent("moved")?;
    parent.require_unique_name("published")?;
    Ok(())
}

#[test]
fn directory_rename_lease_preserves_private_kind_access_and_share_contracts() -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES,
    };

    let (_temporary, path, parent) = fixture()?;
    let stage = parent.create_directory("stage")?;
    let tree = stage.as_directory()?;
    let reopened = parent.open_private_entry("stage", true)?;
    let observed = parent.open_directory("stage")?;
    let before = tree.stamp()?;
    verify_private(stage.as_file(), true)?;

    // Confirm that a timestamp write is supported on this directory when its
    // handle actually requests the right; the retained handles must lack it.
    let probe_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(946_684_800);
    let attribute_writer = std::fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path.join("stage"))?;
    attribute_writer.set_modified(probe_time)?;
    drop(attribute_writer);
    let modified = metadata(stage.as_file())?.modified;

    let mut regular = parent.create_file("regular")?;
    regular.write_all(b"a private file is still not a directory")?;
    regular.sync_all()?;
    let regular_before = metadata(regular.as_file())?;
    let error = native::directory_rename_handle(regular.as_file())
        .err()
        .ok_or_else(|| io::Error::other("directory rename leases must reject regular files"))?;
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(metadata(regular.as_file())?, regular_before);

    let require_read_only_handles = || -> io::Result<()> {
        for retained in [
            stage.as_file(),
            tree.as_file(),
            reopened.as_file(),
            observed.as_file(),
        ] {
            // Exercise the kernel's access check rather than comparing masks.
            let error = native::rename_no_replace(retained, parent.as_file(), "unexpected")
                .err()
                .ok_or_else(|| {
                    io::Error::other("long-lived directory handles must not acquire DELETE access")
                })?;
            assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED as i32));
            let error = retained.set_modified(probe_time).err().ok_or_else(|| {
                io::Error::other(
                    "long-lived directory handles must not acquire write-attribute access",
                )
            })?;
            assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED as i32));
        }
        parent.require_absent("unexpected")?;
        assert_eq!(tree.stamp()?, before);
        assert_eq!(metadata(stage.as_file())?.modified, modified);
        Ok(())
    };
    let read_without_delete_sharing = || {
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path.join("stage"))
    };
    let open_delete = || {
        std::fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path.join("stage"))
    };

    require_read_only_handles()?;
    // Both probes must work before the short lease and again after it closes.
    drop(read_without_delete_sharing()?);
    drop(open_delete()?);
    let source_lease = native::directory_rename_handle(stage.as_file())?;
    assert_eq!(metadata(&source_lease)?.directory_stamp()?, before);
    verify_private(&source_lease, true)?;
    require_read_only_handles()?;
    let error = read_without_delete_sharing()
        .err()
        .ok_or_else(|| io::Error::other("the active DELETE lease must require delete sharing"))?;
    assert_eq!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION as i32));
    let error = open_delete()
        .err()
        .ok_or_else(|| io::Error::other("the source lease must deny another DELETE opener"))?;
    assert_eq!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION as i32));
    drop(source_lease);

    require_read_only_handles()?;
    drop(read_without_delete_sharing()?);
    drop(open_delete()?);
    verify_private(stage.as_file(), true)?;
    assert_eq!(tree.stamp()?, before);
    assert!(tree.entries()?.is_empty());
    Ok(())
}
