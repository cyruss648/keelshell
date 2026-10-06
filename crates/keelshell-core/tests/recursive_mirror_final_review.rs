use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDirection as Direction, DirectorySyncPlanError, MAX_DIRECTORY_HASH_BYTES,
    compare_directories, plan_directory_mirror,
};

#[test]
fn combined_byte_budget_counts_identical_entries_on_both_sides_at_the_exact_boundary() {
    // Fixed observations exercise planning policy without allocating file bodies
    // or claiming that a transport actually read these synthetic large files.
    let entries: Vec<_> = (0..2)
        .map(|index| {
            Entry::new(
                format!("large-{index}"),
                Kind::File,
                Some(MAX_DIRECTORY_HASH_BYTES as u64),
                Some(0),
            )
            .with_content_hash([index; 32])
        })
        .collect();
    let report = compare_directories(&entries, &entries)
        .unwrap_or_else(|error| panic!("fixed report: {error}"));
    let plan = plan_directory_mirror(&report, Direction::LeftToRight)
        .unwrap_or_else(|error| panic!("exact 256 MiB policy boundary: {error}"));
    assert_eq!(plan.operation_count(), 0);
    let mut extra = entries.clone();
    extra.push(
        Entry::new("one-byte-extra", Kind::File, Some(1), Some(0)).with_content_hash([7; 32]),
    );
    let report = compare_directories(&entries, &extra)
        .unwrap_or_else(|error| panic!("fixed overflow report: {error}"));
    assert!(matches!(
        plan_directory_mirror(&report, Direction::LeftToRight),
        Err(DirectorySyncPlanError::MirrorBudget { .. })
    ));
}
