use keelshell_core::{
    DirectoryCompareError, DirectoryCompareSide, DirectoryEntryKind, DirectoryEntrySnapshot,
    DirectoryEntryStatus, MAX_DIRECTORY_COMPARE_PATH_BYTES, compare_directories,
};

fn file(path: &str, size: u64, modified: u64) -> DirectoryEntrySnapshot {
    DirectoryEntrySnapshot::new(path, DirectoryEntryKind::File, Some(size), Some(modified))
}

#[test]
fn public_comparison_reports_stable_rows_for_a_remote_review() {
    let left = [file("config/app.toml", 10, 4), file("same.txt", 2, 7)];
    let right = [
        file("config/app.toml", 10, 5),
        file("new.txt", 4, 7),
        file("same.txt", 2, 7),
    ];
    let report = compare_directories(&left, &right)
        .unwrap_or_else(|error| panic!("bounded comparison: {error}"));
    assert_eq!(report.rows()[0].path, "config/app.toml");
    assert_eq!(
        report.rows()[0].status,
        DirectoryEntryStatus::Changed,
        "a timestamp mismatch must remain reviewable"
    );
    assert_eq!(report.rows()[1].status, DirectoryEntryStatus::RightOnly);
    assert_eq!(report.same_count(), 1);
    assert_eq!(report.review_count(), 2);
}

#[test]
fn public_validation_identifies_side_and_rejects_unsafe_paths() {
    let invalid = DirectoryEntrySnapshot::new(
        "folder/../secret",
        DirectoryEntryKind::File,
        Some(1),
        Some(1),
    );
    assert!(matches!(
        compare_directories(&[], &[invalid]),
        Err(DirectoryCompareError::InvalidPath {
            side: DirectoryCompareSide::Right,
            ..
        })
    ));

    let long_path = "x".repeat(MAX_DIRECTORY_COMPARE_PATH_BYTES + 1);
    let entry = DirectoryEntrySnapshot::new(long_path, DirectoryEntryKind::File, None, None);
    assert!(matches!(
        compare_directories(&[entry], &[]),
        Err(DirectoryCompareError::PathTooLong {
            side: DirectoryCompareSide::Left,
            ..
        })
    ));
}
