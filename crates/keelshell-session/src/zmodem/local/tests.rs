#![cfg(test)]

use super::*;

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn checked<T>(result: Result<T>) -> TestResult<T> {
    result.map_err(|reason| format!("{reason:?}").into())
}

fn root() -> TestResult<(tempfile::TempDir, PathBuf)> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    Ok((temporary, root))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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
    assert_eq!(review[0].path, binary);
    assert_eq!(review[0].name, "文件.bin");
    assert_eq!(review[0].size, 5);
    assert_eq!(
        review[0].sha256,
        Some("e6530919ba2a0c9e7d323a3aa5196a4f95bf830dc89c575245c182aaf2a7f319".into())
    );
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
fn zmodem_upload_local_observation_rejects_empty_and_excessive_batches_before_io() {
    let stop = AtomicBool::new(false);
    assert!(matches!(
        UploadPlan::prepare(Vec::new(), &stop),
        Err(ZmodemFailure::Limit)
    ));
    assert!(matches!(
        UploadPlan::prepare(vec![PathBuf::from("not-opened"); MAX_FILES + 1], &stop),
        Err(ZmodemFailure::Limit)
    ));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_upload_local_observation_rejects_links_and_hardlink_aliases() -> TestResult {
    use std::os::unix::fs::symlink;
    let (_temporary, root) = root()?;
    let original = root.join("original.bin");
    let symlink_path = root.join("symlink.bin");
    let hardlink = root.join("hardlink.bin");
    std::fs::write(&original, b"unchanged")?;
    symlink(&original, &symlink_path)?;
    let stop = AtomicBool::new(false);
    assert!(matches!(
        UploadPlan::prepare(vec![symlink_path], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    std::fs::hard_link(&original, &hardlink)?;
    assert!(matches!(
        UploadPlan::prepare(vec![original.clone()], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert!(matches!(
        UploadPlan::prepare(vec![hardlink], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert_eq!(std::fs::read(original)?, b"unchanged");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_upload_local_observation_rejects_symlinked_ancestors_and_special_files() -> TestResult {
    use std::os::unix::fs::symlink;
    let (_temporary, root) = root()?;
    let actual = root.join("actual");
    let alias = root.join("alias");
    std::fs::create_dir(&actual)?;
    std::fs::write(actual.join("safe.bin"), b"ordinary")?;
    symlink(&actual, &alias)?;
    let stop = AtomicBool::new(false);
    assert!(matches!(
        UploadPlan::prepare(vec![alias.join("safe.bin")], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    let fifo = root.join("not-a-file");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()?
            .success()
    );
    assert!(matches!(
        UploadPlan::prepare(vec![fifo], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    assert!(matches!(
        UploadPlan::prepare(vec![actual], &stop),
        Err(ZmodemFailure::InvalidPath)
    ));
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_upload_local_confirmation_rejects_replaced_ancestor_even_with_same_bytes() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("reviewed");
    std::fs::create_dir(&directory)?;
    let path = directory.join("source.txt");
    std::fs::write(&path, b"same bytes")?;
    let stop = AtomicBool::new(false);
    let plan = checked(UploadPlan::prepare(vec![path.clone()], &stop))?;
    std::fs::rename(&directory, root.join("retained"))?;
    std::fs::create_dir(&directory)?;
    std::fs::write(&path, b"same bytes")?;
    assert!(matches!(
        plan.confirm(&stop),
        Err(ZmodemFailure::SourceChanged)
    ));
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_paths_reject_relative_traversal_and_reserved_source_names() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    std::fs::create_dir(root.join("nested"))?;
    std::fs::write(root.join("valid.bin"), b"valid")?;
    std::fs::write(root.join("CON.txt"), b"device alias")?;
    for path in [
        PathBuf::from("valid.bin"),
        root.join("nested/../valid.bin"),
        root.join("CON.txt"),
    ] {
        assert!(matches!(
            UploadPlan::prepare(vec![path], &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
    }
    for path in [
        PathBuf::from("new.bin"),
        root.join("nested/../new.bin"),
        root.join("NUL.txt"),
    ] {
        assert!(matches!(
            Destination::prepare(path, &stop),
            Err(ZmodemFailure::InvalidPath)
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_destination_rejects_existing_case_aliases_and_symlinks() -> TestResult {
    use std::os::unix::fs::symlink;
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    std::fs::write(root.join("Report.txt"), b"existing owner")?;
    assert!(matches!(
        Destination::prepare(root.join("report.TXT"), &stop),
        Err(ZmodemFailure::DestinationExists)
    ));
    symlink(root.join("absent"), root.join("dangling.txt"))?;
    assert!(matches!(
        Destination::prepare(root.join("dangling.txt"), &stop),
        Err(ZmodemFailure::DestinationExists)
    ));
    std::fs::create_dir(root.join("existing-dir"))?;
    assert!(matches!(
        Destination::prepare(root.join("existing-dir"), &stop),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert_eq!(std::fs::read(root.join("Report.txt"))?, b"existing owner");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_destination_preparation_is_read_only_and_cancellation_creates_nothing() -> TestResult
{
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(root.join("safe.txt"), &stop))?;
    assert_eq!(std::fs::read_dir(&root)?.count(), 0);
    stop.store(true, Ordering::Release);
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"cancelled".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::Stale)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_publication_preserves_a_target_created_after_preparation() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("raced.txt");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let stage = destination.staging.clone();
    std::fs::write(&target, b"external owner")?;
    let mutated = AtomicBool::new(false);
    assert!(matches!(
        destination.publish(b"reviewed".to_vec(), &stop, &mutated),
        Err(ZmodemFailure::DestinationExists)
    ));
    assert!(!mutated.load(Ordering::Acquire));
    assert!(!stage.exists());
    assert_eq!(std::fs::read(target)?, b"external owner");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_publication_rejects_a_replaced_parent_without_writing_either_tree() -> TestResult {
    let (_temporary, root) = root()?;
    let directory = root.join("reviewed");
    std::fs::create_dir(&directory)?;
    let target = directory.join("new.txt");
    let stop = AtomicBool::new(false);
    let destination = checked(Destination::prepare(target, &stop))?;
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_publication_verifies_binary_content_identity_private_acl_and_new_target()
-> TestResult {
    use std::os::unix::fs::MetadataExt;
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("safe.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let staging = destination.staging.clone();
    let mutated = AtomicBool::new(false);
    let bytes = [0, 255, 128, 13, 10, 24];
    let saved = checked(destination.publish(bytes.to_vec(), &stop, &mutated))?;
    assert_eq!(saved.path, target);
    assert_eq!(saved.bytes, bytes.len() as u64);
    assert_eq!(
        saved.sha256,
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(std::fs::read(&target)?, bytes);
    assert_eq!(std::fs::metadata(&target)?.mode() & 0o7777, 0o600);
    assert_eq!(std::fs::metadata(&target)?.nlink(), 1);
    let file = File::open(&target)?;
    checked(private_acl(&file, false))?;
    assert!(!staging.exists());
    assert!(mutated.load(Ordering::Acquire));
    assert_eq!(std::fs::read_dir(root)?.count(), 1);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn zmodem_local_no_replace_primitive_preserves_existing_files_directories_and_links() -> TestResult
{
    use rustix::fs::{Mode, OFlags, RenameFlags, open, renameat_with};
    use std::os::unix::fs::symlink;
    let (_temporary, root) = root()?;
    std::fs::write(root.join("stage"), b"reviewed")?;
    std::fs::write(root.join("file"), b"existing")?;
    std::fs::create_dir(root.join("directory"))?;
    symlink(root.join("absent"), root.join("link"))?;
    let descriptor = open(
        &root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )?;
    for target in ["file", "directory", "link"] {
        assert!(
            renameat_with(
                &descriptor,
                "stage",
                &descriptor,
                target,
                RenameFlags::NOREPLACE
            )
            .is_err()
        );
        assert_eq!(std::fs::read(root.join("stage"))?, b"reviewed");
    }
    assert_eq!(std::fs::read(root.join("file"))?, b"existing");
    assert!(std::fs::symlink_metadata(root.join("link"))?.is_symlink());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn zmodem_local_receive_clears_real_inherited_darwin_acl_before_writing_content() -> TestResult {
    let (_temporary, root) = root()?;
    let status = std::process::Command::new("/bin/chmod")
        .arg("+a")
        .arg("everyone allow read,write,file_inherit,directory_inherit")
        .arg(&root)
        .status()?;
    assert!(status.success());
    let stop = AtomicBool::new(false);
    let target = root.join("private.bin");
    let destination = checked(Destination::prepare(target.clone(), &stop))?;
    let mutated = AtomicBool::new(false);
    let saved = checked(destination.publish(b"private received data".to_vec(), &stop, &mutated))?;
    assert_eq!(saved.path, target);
    checked(private_acl(&File::open(&target)?, false))?;
    assert_eq!(std::fs::read(target)?, b"private received data");
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn zmodem_darwin_acl_probe_checks_the_retained_vnode_and_rejects_late_entries() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, root) = root()?;
    let target = root.join("private.bin");
    let held = File::create(&target)?;
    held.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    let add = |path: &Path| -> TestResult {
        assert!(
            std::process::Command::new("/bin/chmod")
                .args(["+a", "everyone allow read,write"])
                .arg(path)
                .status()?
                .success()
        );
        Ok(())
    };
    add(&target)?;
    assert!(
        private_acl(&held, false).is_err(),
        "an ACL cannot hide behind /dev/fd metadata"
    );
    checked(private_acl(&held, true))?;
    checked(private_acl(&held, false))?;
    add(&target)?;
    assert!(
        private_acl(&held, false).is_err(),
        "late ACL injection must be observed"
    );
    checked(private_acl(&held, true))?;
    std::fs::rename(&target, root.join("retained.bin"))?;
    let replacement = File::create(&target)?;
    replacement.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    add(&target)?;
    checked(private_acl(&held, false))?;
    assert!(
        private_acl(&replacement, false).is_err(),
        "a replaced pathname cannot redirect the retained descriptor"
    );
    Ok(())
}

#[test]
fn zmodem_review_paths_reject_invisible_or_line_breaking_ancestor_names() {
    for name in [
        "hidden\nancestor",
        "hidden\rancestor",
        "hidden\u{202e}ancestor",
        "hidden\u{2066}ancestor",
    ] {
        let candidate = std::env::temp_dir().join(name).join("safe.bin");
        assert!(matches!(path(&candidate), Err(ZmodemFailure::InvalidPath)));
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[test]
fn zmodem_local_unverified_backend_reports_unsupported_before_staging() -> TestResult {
    let (_temporary, root) = root()?;
    let stop = AtomicBool::new(false);
    let target = root.join("new.txt");
    assert!(matches!(
        Destination::prepare(target.clone(), &stop),
        Err(ZmodemFailure::Unsupported)
    ));
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(root)?.count(), 0);
    Ok(())
}
