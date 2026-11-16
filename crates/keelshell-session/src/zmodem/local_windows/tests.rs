#![cfg(test)]
#![cfg(windows)]

use super::*;

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn checked<T>(result: Result<T>) -> TestResult<T> {
    result.map_err(|reason| format!("{reason:?}").into())
}

fn root() -> TestResult<(tempfile::TempDir, PathBuf)> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    Ok((temporary, root))
}

fn literal_child(root: &Path, suffix: &str) -> TestResult<PathBuf> {
    // Path::join normalizes dot components after a verbatim Windows prefix.
    // Preserve their spelling so these tests actually reach our path validator.
    let root = root.to_str().ok_or("temporary path is not Unicode")?;
    Ok(PathBuf::from(format!(r"{root}\{suffix}")))
}

fn staged(
    destination: &Destination,
    bytes: &[u8],
    stop: &AtomicBool,
    mutated: &AtomicBool,
) -> TestResult<(Directory, PrivateEntry, FileInfo)> {
    let parent = checked(destination.revalidate(stop))?;
    let mut staging = checked(destination.create_staging(&parent, stop, mutated))?;
    let stamp = checked(write_staging(&mut staging, bytes, stop))?;
    Ok((parent, staging, stamp))
}

#[test]
fn zmodem_upload_local_observation_keeps_complete_binary_and_empty_content() -> TestResult {
    let (_temporary, root) = root()?;
    let binary = root.join("文件.bin");
    let empty = root.join("empty.bin");
    std::fs::write(&binary, [0, 255, 128, 24, 10])?;
    std::fs::write(&empty, b"")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(
        vec![binary.clone(), empty.clone()],
        &stop,
    ))?;
    let review = plan.review();
    assert_eq!(review.len(), 2);
    assert_eq!(review[0].path, binary);
    assert_eq!(review[0].name, "文件.bin");
    assert_eq!(review[0].size, 5);
    assert_eq!(
        review[0].sha256,
        Some("e6530919ba2a0c9e7d323a3aa5196a4f95bf830dc89c575245c182aaf2a7f319".into())
    );
    assert_eq!(review[1].path, empty);
    assert_eq!(review[1].size, 0);
    assert_eq!(
        review[1].sha256,
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into())
    );
    let files = checked(plan.confirm(&stop))?;
    assert_eq!(files[0].bytes(), [0, 255, 128, 24, 10]);
    assert_eq!(files[1].bytes(), b"");
    assert_eq!(std::fs::read_dir(root)?.count(), 2);
    Ok(())
}

#[test]
fn zmodem_upload_local_observation_rejects_empty_and_excessive_batches_before_io() -> TestResult {
    let stop = AtomicBool::new(false);
    assert!(matches!(
        UploadPlan::prepare(Vec::new(), &stop),
        Err(ZmodemFailure::Limit)
    ));
    assert!(matches!(
        UploadPlan::prepare(vec![PathBuf::from("not-opened"); MAX_FILES + 1], &stop),
        Err(ZmodemFailure::Limit)
    ));
    Ok(())
}

#[test]
fn zmodem_upload_local_observation_rejects_case_aliases() -> TestResult {
    let (_temporary, root) = root()?;
    let first = root.join("Report.txt");
    let nested = root.join("nested");
    std::fs::create_dir(&nested)?;
    let second = nested.join("report.TXT");
    std::fs::write(&first, b"first")?;
    std::fs::write(&second, b"second")?;
    assert!(matches!(
        UploadPlan::prepare(vec![first, second], &AtomicBool::new(false)),
        Err(ZmodemFailure::InvalidPath)
    ));
    Ok(())
}

#[test]
fn zmodem_windows_upload_rejects_expanding_unicode_case_aliases() -> TestResult {
    let (_temporary, root) = root()?;
    let first = root.join("straße.bin");
    let nested = root.join("nested");
    std::fs::create_dir(&nested)?;
    let second = nested.join("STRASSE.BIN");
    std::fs::write(&first, b"first")?;
    std::fs::write(&second, b"second")?;
    assert!(matches!(
        UploadPlan::prepare(vec![first, second], &AtomicBool::new(false)),
        Err(ZmodemFailure::InvalidPath)
    ));
    Ok(())
}

#[test]
fn zmodem_windows_upload_rejects_hardlink_aliases() -> TestResult {
    let (_temporary, root) = root()?;
    let original = root.join("original.bin");
    let hardlink = root.join("hardlink.bin");
    std::fs::write(&original, b"unchanged")?;
    std::fs::hard_link(&original, &hardlink)?;
    let stop = AtomicBool::new(false);
    for path in [&original, &hardlink] {
        assert!(matches!(
            UploadPlan::prepare(vec![path.clone()], &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
    }
    assert_eq!(std::fs::read(original)?, b"unchanged");
    assert_eq!(std::fs::read(hardlink)?, b"unchanged");
    Ok(())
}

#[test]
fn zmodem_upload_local_observation_rejects_size_excess_before_reading_sparse_content() -> TestResult
{
    let (_temporary, root) = root()?;
    let path = root.join("oversized.bin");
    std::fs::File::create(&path)?.set_len(MAX_FILE_BYTES + 1)?;
    assert!(matches!(
        UploadPlan::prepare(vec![path], &AtomicBool::new(false)),
        Err(ZmodemFailure::Limit)
    ));
    Ok(())
}

#[test]
fn zmodem_windows_upload_enforces_the_remaining_batch_byte_budget_before_reading() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("bounded.bin");
    std::fs::write(&path, b"four")?;
    assert!(matches!(
        read_source(&path, 3, &AtomicBool::new(false)),
        Err(ZmodemFailure::Limit)
    ));
    assert_eq!(std::fs::read(path)?, b"four");
    Ok(())
}

#[test]
fn zmodem_windows_upload_rejects_total_byte_excess_before_reading_the_last_file() -> TestResult {
    let (_temporary, root) = root()?;
    let first = root.join("first.bin");
    let second = root.join("second.bin");
    let last = root.join("last.bin");
    std::fs::File::create(&first)?.set_len(MAX_FILE_BYTES)?;
    std::fs::File::create(&second)?.set_len(MAX_TOTAL_BYTES - MAX_FILE_BYTES)?;
    std::fs::write(&last, b"x")?;
    assert!(matches!(
        UploadPlan::prepare(vec![first, second, last.clone()], &AtomicBool::new(false)),
        Err(ZmodemFailure::Limit)
    ));
    assert_eq!(std::fs::read(last)?, b"x");
    Ok(())
}

#[test]
fn zmodem_windows_upload_rejects_directories_without_reading_them_as_files() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("directory");
    std::fs::create_dir(&directory)?;
    assert!(matches!(
        UploadPlan::prepare(vec![directory], &AtomicBool::new(false)),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert_eq!(std::fs::read_dir(root)?.count(), 1);
    Ok(())
}

#[test]
fn zmodem_upload_local_confirmation_rejects_an_identical_replacement_inode() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.txt");
    std::fs::write(&path, b"same bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::rename(&path, root.join("retained.txt"))?;
    std::fs::write(&path, b"same bytes")?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    assert_eq!(std::fs::read(path)?, b"same bytes");
    Ok(())
}

#[test]
fn zmodem_upload_local_confirmation_rejects_missing_and_changed_sources() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.txt");
    std::fs::write(&path, b"first")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::write(&path, b"other")?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::remove_file(&path)?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    Ok(())
}

#[test]
fn zmodem_windows_confirmation_rejects_metadata_only_changes() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.bin");
    std::fs::write(&path, b"same bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    let original = std::fs::metadata(&path)?.permissions();
    let mut changed = original.clone();
    changed.set_readonly(true);
    std::fs::set_permissions(&path, changed)?;
    let result = plan.confirm(&stop);
    std::fs::set_permissions(&path, original)?;
    assert!(matches!(result, Err(ZmodemFailure::SourceChanged)));
    assert_eq!(std::fs::read(path)?, b"same bytes");
    Ok(())
}

#[test]
fn zmodem_windows_retained_source_blocks_ancestor_rename_and_preserves_review() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("reviewed");
    std::fs::create_dir(&directory)?;
    let path = directory.join("source.txt");
    std::fs::write(&path, b"same bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    let retained = root.join("retained");
    // The retained source is an open descendant, so Windows blocks this attack.
    // This proves prevention, not SourceChanged after an ancestor replacement.
    let error = std::fs::rename(&directory, &retained)
        .err()
        .ok_or("ancestor rename unexpectedly succeeded with an open source")?;
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.raw_os_error(), Some(5));
    assert!(!retained.try_exists()?);
    let current = checked(parents(&path))?;
    assert_eq!(current.stamps, plan.sources[0].parent.stamps);
    let named = checked(current.directory())?.open_file("source.txt")?;
    assert_eq!(metadata(&named)?.identity, plan.sources[0].stamp.identity);
    assert_eq!(checked(plan.confirm(&stop))?[0].bytes(), b"same bytes");
    assert_eq!(std::fs::read(path)?, b"same bytes");
    Ok(())
}

#[test]
fn zmodem_windows_confirmation_rejects_a_new_hardlink_after_review() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.bin");
    let alias = root.join("alias.bin");
    std::fs::write(&path, b"reviewed")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::hard_link(&path, &alias)?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    assert_eq!(std::fs::read(path)?, b"reviewed");
    assert_eq!(std::fs::read(alias)?, b"reviewed");
    Ok(())
}

#[test]
fn zmodem_windows_confirmation_rejects_reused_path_after_hardlink_rebinding() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.bin");
    let retained = root.join("retained.bin");
    std::fs::write(&path, b"same bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::hard_link(&path, &retained)?;
    std::fs::remove_file(&path)?;
    std::fs::write(&path, b"same bytes")?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    assert_eq!(std::fs::read(path)?, b"same bytes");
    assert_eq!(std::fs::read(retained)?, b"same bytes");
    Ok(())
}

#[test]
fn zmodem_windows_confirmation_releases_no_batch_when_the_last_source_changed() -> TestResult {
    let (_temporary, root) = root()?;
    let first = root.join("first.bin");
    let last = root.join("last.bin");
    std::fs::write(&first, b"first")?;
    std::fs::write(&last, b"last")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![first, last.clone()], &stop))?;
    std::fs::write(&last, b"late")?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    Ok(())
}

#[test]
fn zmodem_windows_upload_cancellation_releases_no_review_or_sender_bytes() -> TestResult {
    let (_temporary, root) = root()?;
    let path = root.join("source.bin");
    std::fs::write(&path, b"reviewed")?;
    let stop = AtomicBool::new(true);
    assert!(matches!(
        UploadPlan::prepare(vec![path.clone()], &stop),
        Err(ZmodemFailure::Stale)
    ));
    stop.store(false, Ordering::Release);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    stop.store(true, Ordering::Release);
    assert!(matches!(plan.confirm(&stop), Err(ZmodemFailure::Stale)));
    assert_eq!(std::fs::read(path)?, b"reviewed");
    Ok(())
}

#[test]
fn zmodem_local_paths_reject_relative_traversal_and_reserved_source_names() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    for path in [
        PathBuf::from("valid.bin"),
        PathBuf::from(r"C:valid.bin"),
        literal_child(&root, r"nested\..\valid.bin")?,
        literal_child(&root, r"nested\.\valid.bin")?,
        literal_child(&root, r"CON.txt")?,
        literal_child(&root, r"COM¹.txt")?,
        literal_child(&root, r"NUL.txt\valid.bin")?,
        literal_child(&root, r"valid.bin:stream")?,
        literal_child(&root, r"trailing.\valid.bin")?,
        literal_child(&root, r"trailing \valid.bin")?,
        literal_child(&root, r"double\\valid.bin")?,
    ] {
        assert!(matches!(
            UploadPlan::prepare(vec![path.clone()], &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
        assert!(matches!(
            Destination::prepare(path, &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
    }
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}

#[test]
fn zmodem_review_paths_reject_invisible_or_line_breaking_ancestor_names() -> TestResult {
    let (_temporary, root) = root()?;
    for name in [
        "hidden\nancestor",
        "hidden\rancestor",
        "hidden\u{202e}ancestor",
        "hidden\u{2066}ancestor",
    ] {
        let candidate = literal_child(&root, &format!(r"{name}\safe.bin"))?;
        assert!(matches!(path(&candidate), Err(ZmodemFailure::InvalidPath)));
    }
    Ok(())
}

#[test]
fn zmodem_windows_paths_reject_unc_and_device_namespaces_before_io() -> TestResult {
    let stop = AtomicBool::new(false);
    for spelling in [
        r"\\server\share\safe.bin",
        r"\\?\UNC\server\share\safe.bin",
        r"\\.\PhysicalDrive0",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\safe.bin",
    ] {
        assert!(matches!(
            UploadPlan::prepare(vec![PathBuf::from(spelling)], &stop),
            Err(ZmodemFailure::Unsupported | ZmodemFailure::InvalidPath)
        ));
        assert!(matches!(
            Destination::prepare(PathBuf::from(spelling), &stop),
            Err(ZmodemFailure::Unsupported | ZmodemFailure::InvalidPath)
        ));
    }
    Ok(())
}

#[test]
fn zmodem_windows_paths_reject_non_unicode_names_before_io() -> TestResult {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let (_temporary, root) = root()?;
    let mut spelling: Vec<_> = root.as_os_str().encode_wide().collect();
    spelling.extend([b'\\' as u16, 0xd800, b'.' as u16, b'x' as u16]);
    let path = PathBuf::from(OsString::from_wide(&spelling));
    let stop = AtomicBool::new(false);
    assert!(matches!(
        UploadPlan::prepare(vec![path.clone()], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert!(matches!(
        Destination::prepare(path, &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}

#[test]
fn zmodem_windows_reviews_retain_every_original_ancestor() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("outer").join("inner");
    std::fs::create_dir_all(&directory)?;
    let source = directory.join("source.bin");
    std::fs::write(&source, b"reviewed")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![source], &stop))?;
    let destination = checked(Destination::prepare(directory.join("new.bin"), &stop))?;
    let text = directory.to_str().ok_or("temporary path is not Unicode")?;
    let expected = WindowsPath::parse(text)?.components().len() + 1;
    assert_eq!(plan.sources[0].parent.directories.len(), expected);
    assert_eq!(plan.sources[0].parent.stamps.len(), expected);
    assert_eq!(destination.parent.directories.len(), expected);
    assert_eq!(destination.parent.stamps.len(), expected);
    let clone = destination.clone();
    drop(destination);
    checked(clone.revalidate(&stop))?;
    assert_eq!(checked(plan.confirm(&stop))?[0].bytes(), b"reviewed");
    Ok(())
}

#[test]
fn zmodem_windows_retained_reviews_block_grandparent_rename_and_preserve_namespace() -> TestResult {
    let (_temporary, root) = root()?;
    let outer = root.join("outer");
    let leaf = outer.join("leaf");
    std::fs::create_dir_all(&leaf)?;
    let path = leaf.join("source.bin");
    std::fs::write(&path, b"unchanged identity and bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    let destination = checked(Destination::prepare(leaf.join("new.bin"), &stop))?;
    let retained = root.join("retained");
    // Both reviews retain the leaf; the upload also retains its source file.
    // Windows blocks the outer rename before any replacement can occur.
    let error = std::fs::rename(&outer, &retained)
        .err()
        .ok_or("grandparent rename unexpectedly succeeded with open descendants")?;
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.raw_os_error(), Some(5));
    assert!(!retained.try_exists()?);
    let current = checked(parents(&path))?;
    let named = checked(current.directory())?.open_file("source.bin")?;
    assert_eq!(metadata(&named)?.identity, plan.sources[0].stamp.identity);
    assert_eq!(current.stamps.last(), plan.sources[0].parent.stamps.last());
    assert_eq!(current.stamps, plan.sources[0].parent.stamps);
    assert_eq!(current.stamps, destination.parent.stamps);
    // The original namespace remains valid. Do not claim post-replacement refusal.
    checked(destination.revalidate(&stop))?;
    assert!(!destination.path.try_exists()?);
    assert!(!destination.staging.try_exists()?);
    assert_eq!(
        checked(plan.confirm(&stop))?[0].bytes(),
        b"unchanged identity and bytes"
    );
    assert_eq!(std::fs::read(path)?, b"unchanged identity and bytes");
    assert_eq!(std::fs::read_dir(leaf)?.count(), 1);
    Ok(())
}

#[test]
fn zmodem_windows_destination_rejects_existing_files_directories_and_unicode_aliases() -> TestResult
{
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    std::fs::write(root.join("Report.txt"), b"existing owner")?;
    std::fs::write(root.join("straße.bin"), b"Unicode owner")?;
    std::fs::create_dir(root.join("existing-dir"))?;
    for name in ["Report.txt", "report.TXT", "STRASSE.BIN", "existing-dir"] {
        assert!(matches!(
            Destination::prepare(root.join(name), &stop),
            Err(ZmodemFailure::DestinationExists)
        ));
    }
    assert_eq!(std::fs::read(root.join("Report.txt"))?, b"existing owner");
    assert_eq!(std::fs::read(root.join("straße.bin"))?, b"Unicode owner");
    Ok(())
}

#[test]
fn zmodem_local_destination_preparation_is_read_only_and_cancellation_creates_nothing() -> TestResult
{
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("safe.txt"), &stop))?;
    assert_eq!(std::fs::read_dir(&root)?.count(), 0);
    stop.store(true, Ordering::Release);
    assert!(matches!(
        destination.revalidate(&stop),
        Err(ZmodemFailure::Stale)
    ));
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"cancelled".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::Stale)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}

#[test]
fn zmodem_local_publication_preserves_a_target_created_after_preparation() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("raced.txt");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let staging = destination.staging.clone();
    std::fs::write(&target, b"external owner")?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"reviewed".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert!(!staging.try_exists()?);
    assert_eq!(std::fs::read(target)?, b"external owner");
    Ok(())
}

#[test]
fn zmodem_windows_publication_preserves_a_unicode_alias_created_after_review() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("STRASSE.bin");
    let destination = checked(Destination::prepare(target, &stop))?;
    let staging = destination.staging.clone();
    let alias = root.join("straße.BIN");
    std::fs::write(&alias, b"external owner")?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"reviewed".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert!(!staging.try_exists()?);
    assert_eq!(std::fs::read(alias)?, b"external owner");
    Ok(())
}

#[test]
fn zmodem_windows_publication_preserves_a_precreated_staging_name_before_mutation() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("new.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let staging = destination.staging.clone();
    std::fs::write(&staging, b"external staging owner")?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"reviewed".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert!(!target.try_exists()?);
    assert_eq!(std::fs::read(staging)?, b"external staging owner");
    Ok(())
}

#[test]
fn zmodem_windows_exclusive_create_marks_mutated_before_a_late_collision() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("new.bin"), &stop))?;
    let parent = checked(destination.revalidate(&stop))?;
    std::fs::write(&destination.staging, b"late external owner")?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.create_staging(&parent, &stop, &mutated),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read(&destination.staging)?, b"late external owner");
    assert!(!destination.path.try_exists()?);
    Ok(())
}

#[test]
fn zmodem_local_publication_rejects_a_replaced_parent_without_writing_either_tree() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("reviewed");
    std::fs::create_dir(&directory)?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(directory.join("new.txt"), &stop))?;
    let retained = root.join("retained");
    std::fs::rename(&directory, &retained)?;
    std::fs::create_dir(&directory)?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"reviewed".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::SourceChanged)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read_dir(directory)?.count(), 0);
    assert_eq!(std::fs::read_dir(retained)?.count(), 0);
    Ok(())
}

#[test]
fn zmodem_windows_create_sets_private_dacl_before_any_content_is_written() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("new.bin"), &stop))?;
    let parent = checked(destination.revalidate(&stop))?;
    let mutated = AtomicBool::new(false);
    let staging = checked(destination.create_staging(&parent, &stop, &mutated))?;
    verify_private(staging.as_file(), false)?;
    let info = metadata(staging.as_file())?;
    let named = parent.open_file(checked(basename(&destination.staging))?)?;
    verify_private(&named, false)?;
    assert_eq!(metadata(&named)?, info);
    assert_eq!(info.bytes, 0);
    assert_eq!(info.links, 1);
    assert!(mutated.load(Ordering::Acquire));
    assert!(!destination.path.try_exists()?);
    Ok(())
}

#[test]
fn zmodem_windows_cancellation_after_create_retains_empty_private_staging_and_mutation()
-> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("new.bin"), &stop))?;
    let parent = checked(destination.revalidate(&stop))?;
    let mutated = AtomicBool::new(false);
    let mut staging = checked(destination.create_staging(&parent, &stop, &mutated))?;
    stop.store(true, Ordering::Release);
    assert!(matches!(
        write_staging(&mut staging, b"never written", &stop),
        Err(ZmodemFailure::Stale)
    ));
    verify_private(staging.as_file(), false)?;
    drop(staging);
    assert_eq!(std::fs::read(&destination.staging)?, b"");
    assert!(!destination.path.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_cancellation_after_readback_retains_complete_private_staging() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("new.bin"), &stop))?;
    let stage_path = destination.staging.clone();
    let target = destination.path.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"complete reviewed data";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    stop.store(true, Ordering::Release);
    assert!(matches!(
        destination.publish_staging(&parent, &staging, &stamp, bytes, &stop),
        Err(ZmodemFailure::Stale)
    ));
    verify_private(staging.as_file(), false)?;
    drop(staging);
    assert_eq!(std::fs::read(stage_path)?, bytes);
    assert!(!target.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_target_race_after_staging_preserves_both_names_for_quarantine() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("raced.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage_path = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"reviewed data";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    std::fs::write(&target, b"external owner")?;
    assert!(matches!(
        destination.publish_staging(&parent, &staging, &stamp, bytes, &stop),
        Err(ZmodemFailure::DestinationExists)
    ));
    drop(staging);
    assert_eq!(std::fs::read(stage_path)?, bytes);
    assert_eq!(std::fs::read(target)?, b"external owner");
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_cancellation_after_rename_retains_the_published_target_for_quarantine()
-> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("new.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage_path = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"complete published data";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    parent.rename_no_replace(&staging, "new.bin")?;
    stop.store(true, Ordering::Release);
    assert!(matches!(
        destination.verify_publication(&staging, &stamp, bytes, &stop),
        Err(ZmodemFailure::Stale)
    ));
    verify_private(staging.as_file(), false)?;
    drop(staging);
    assert_eq!(std::fs::read(target)?, bytes);
    assert!(!stage_path.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_final_readback_rejects_a_late_unicode_alias_and_retains_mutation() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("STRASSE.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let mutated = AtomicBool::new(false);
    let bytes = b"reviewed data";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    parent.rename_no_replace(&staging, "STRASSE.bin")?;
    let alias = root.join("straße.BIN");
    std::fs::write(&alias, b"external owner")?;
    assert!(matches!(
        destination.verify_publication(&staging, &stamp, bytes, &stop),
        Err(ZmodemFailure::UnverifiedPublication)
    ));
    drop(staging);
    assert_eq!(std::fs::read(target)?, bytes);
    assert_eq!(std::fs::read(alias)?, b"external owner");
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_final_readback_rejects_reappeared_staging_and_retains_both_paths() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("new.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage_path = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"reviewed data";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    parent.rename_no_replace(&staging, "new.bin")?;
    std::fs::write(&stage_path, b"external staging owner")?;
    assert!(matches!(
        destination.verify_publication(&staging, &stamp, bytes, &stop),
        Err(ZmodemFailure::UnverifiedPublication)
    ));
    drop(staging);
    assert_eq!(std::fs::read(target)?, bytes);
    assert_eq!(std::fs::read(stage_path)?, b"external staging owner");
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_retained_creation_handle_blocks_stage_replacement_and_other_writers() -> TestResult
{
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("safe.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage_path = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"retained object";
    let (parent, staging, stamp) = staged(&destination, bytes, &stop, &mutated)?;
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&stage_path)
            .is_err()
    );
    assert!(std::fs::rename(&stage_path, root.join("stolen.part")).is_err());
    assert!(std::fs::remove_file(&stage_path).is_err());
    let saved = checked(destination.publish_staging(&parent, &staging, &stamp, bytes, &stop))?;
    assert_eq!(metadata(staging.as_file())?.identity, stamp.identity);
    drop(staging);
    assert_eq!(saved.path, target);
    assert_eq!(std::fs::read(target)?, bytes);
    assert!(!stage_path.try_exists()?);
    assert!(!root.join("stolen.part").try_exists()?);
    Ok(())
}

#[test]
fn zmodem_windows_publication_rejects_an_identical_staging_replacement_identity() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("new.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage_path = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = b"same bytes";
    let (parent, staging, original) = staged(&destination, bytes, &stop, &mutated)?;
    drop(staging);
    let retained = root.join("retained.part");
    std::fs::rename(&stage_path, &retained)?;
    let mut replacement = parent.create_file(checked(basename(&stage_path))?)?;
    checked(write_staging(&mut replacement, bytes, &stop))?;
    assert!(matches!(
        destination.publish_staging(&parent, &replacement, &original, bytes, &stop),
        Err(ZmodemFailure::UnverifiedPublication)
    ));
    drop(replacement);
    assert_eq!(std::fs::read(stage_path)?, bytes);
    assert_eq!(std::fs::read(retained)?, bytes);
    assert!(!target.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn zmodem_windows_no_replace_primitive_preserves_existing_files_and_directories() -> TestResult {
    let (_temporary, root) = root()?;
    let parent = checked(parents(&root.join("stage")))?;
    let directory = checked(parent.directory())?;
    let mut staging = directory.create_file("stage")?;
    staging.write_all(b"reviewed")?;
    staging.sync_all()?;
    std::fs::write(root.join("file"), b"existing")?;
    std::fs::create_dir(root.join("directory"))?;
    for target in ["file", "directory"] {
        assert_eq!(
            directory
                .rename_no_replace(&staging, target)
                .err()
                .ok_or("no-replace rename unexpectedly succeeded")?
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(root.join("stage"))?, b"reviewed");
    }
    assert_eq!(std::fs::read(root.join("file"))?, b"existing");
    Ok(())
}

#[test]
fn zmodem_windows_content_readback_rejects_same_length_corruption_short_and_long_files()
-> TestResult {
    let (_temporary, root) = root()?;
    let target = root.join("content.bin");
    let bytes = b"complete binary\0\xff";
    std::fs::write(&target, bytes)?;
    let parent = checked(parents(&target))?;
    let mut file = checked(parent.directory())?.open_file("content.bin")?;
    let stop = AtomicBool::new(false);
    checked(verify_content(&mut file, bytes, &stop))?;
    for wrong in [
        b"Complete binary\0\xff".as_slice(),
        &bytes[..bytes.len() - 1],
        b"complete binary\0\xffextra".as_slice(),
    ] {
        assert!(matches!(
            verify_content(&mut file, wrong, &stop),
            Err(ZmodemFailure::UnverifiedPublication)
        ));
    }
    Ok(())
}

#[test]
fn zmodem_windows_publication_verifies_binary_content_identity_private_dacl_and_new_target()
-> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("文件.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let staging = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes: Vec<_> = (0..(3 * 64 * 1024 + 17))
        .map(|index| (index % 256) as u8)
        .collect();
    let saved = checked(destination.publish(bytes.clone(), &stop, &mutated))?;
    assert_eq!(saved.path, target);
    assert_eq!(saved.bytes, bytes.len() as u64);
    assert_eq!(
        saved.sha256,
        Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let parent = checked(parents(&target))?;
    let directory = checked(parent.directory())?;
    let file = directory.open_file("文件.bin")?;
    verify_private(&file, false)?;
    assert_eq!(metadata(&file)?.links, 1);
    assert_eq!(metadata(&file)?.bytes, bytes.len() as u64);
    directory.require_unique_name("文件.bin")?;
    directory.require_absent(checked(basename(&staging))?)?;
    assert_eq!(std::fs::read(target)?, bytes);
    assert!(!staging.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read_dir(root)?.count(), 1);
    Ok(())
}

#[test]
fn zmodem_windows_publication_verifies_an_empty_file_without_leaving_staging() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let root_text = root.to_str().ok_or("temporary path is not Unicode")?;
    let ordinary = PathBuf::from(root_text.strip_prefix(r"\\?\").unwrap_or(root_text));
    let target = ordinary.join("empty.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let staging = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let saved = checked(destination.publish(Vec::new(), &stop, &mutated))?;
    assert_eq!(saved.path, target);
    assert_eq!(saved.bytes, 0);
    assert_eq!(
        saved.sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let parent = checked(parents(&target))?;
    let file = checked(parent.directory())?.open_file("empty.bin")?;
    verify_private(&file, false)?;
    assert_eq!(std::fs::read(target)?, b"");
    assert!(!staging.try_exists()?);
    assert!(mutated.load(Ordering::Acquire));
    Ok(())
}

#[test]
#[ignore = "requires Windows symbolic-link privilege or Developer Mode"]
fn zmodem_windows_upload_rejects_real_file_and_ancestor_reparse_points() -> TestResult {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    let (_temporary, root) = root()?;
    let actual = root.join("actual");
    let alias = root.join("alias");
    let file_link = root.join("file-link.bin");
    std::fs::create_dir(&actual)?;
    let source = actual.join("safe.bin");
    std::fs::write(&source, b"ordinary")?;
    symlink_file(&source, &file_link)?;
    symlink_dir(&actual, &alias)?;
    let stop = AtomicBool::new(false);
    for path in [file_link, alias.join("safe.bin")] {
        assert!(matches!(
            UploadPlan::prepare(vec![path], &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
    }
    assert!(matches!(
        Destination::prepare(alias.join("new.bin"), &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert_eq!(std::fs::read(source)?, b"ordinary");
    Ok(())
}

#[test]
#[ignore = "requires Windows symbolic-link privilege or Developer Mode"]
fn zmodem_windows_destination_preserves_a_dangling_reparse_point() -> TestResult {
    use std::os::windows::fs::symlink_file;

    let (_temporary, root) = root()?;
    let target = root.join("dangling.bin");
    symlink_file(root.join("absent.bin"), &target)?;
    assert!(matches!(
        Destination::prepare(target.clone(), &AtomicBool::new(false)),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(std::fs::symlink_metadata(target)?.is_symlink());
    Ok(())
}
