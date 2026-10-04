use keelshell_core::{
    DirectoryEntryKind, DirectoryEntrySnapshot, DirectoryEntryStatus, DirectorySyncDeletePolicy,
    DirectorySyncDirection, DirectorySyncOperation, DirectorySyncPlanError, compare_directories,
    hash_directory_content, plan_directory_sync,
};

#[test]
fn public_sync_plan_keeps_deletes_explicit_and_carries_source_revalidation() {
    let content = hash_directory_content(b"local").unwrap_or_else(|error| panic!("hash: {error}"));
    let left =
        [
            DirectoryEntrySnapshot::new(
                "deploy/app.bin",
                DirectoryEntryKind::File,
                Some(5),
                Some(1),
            )
            .with_content_hash(content),
        ];
    let right = [
        DirectoryEntrySnapshot::new("deploy/app.bin", DirectoryEntryKind::File, Some(4), Some(1)),
        DirectoryEntrySnapshot::new("old.cfg", DirectoryEntryKind::File, Some(2), Some(1)),
    ];
    let report =
        compare_directories(&left, &right).unwrap_or_else(|error| panic!("comparison: {error}"));
    assert_eq!(report.rows()[0].status, DirectoryEntryStatus::Changed);
    let plan = plan_directory_sync(
        &report,
        DirectorySyncDirection::LeftToRight,
        DirectorySyncDeletePolicy::IncludeDeletes,
    )
    .unwrap_or_else(|error| panic!("plan: {error}"));
    assert!(plan.operations().iter().any(|operation| matches!(
        operation,
        DirectorySyncOperation::Copy {
            path,
            source_kind: DirectoryEntryKind::File,
            expected_source_hash: Some(hash),
            expected_source_size: Some(5)
        } if path == "deploy/app.bin" && hash == &content
    )));
    assert!(plan.operations().iter().any(|operation| matches!(
        operation,
        DirectorySyncOperation::Delete { path } if path == "old.cfg"
    )));
    let confirmed = plan
        .clone()
        .confirm(plan.review_token())
        .unwrap_or_else(|error| panic!("confirm: {error}"));
    assert_eq!(confirmed.plan().operation_count(), 2);
}

#[test]
fn public_sync_plan_refuses_uncertain_metadata_before_any_action() {
    let left = [DirectoryEntrySnapshot::new(
        "unknown.bin",
        DirectoryEntryKind::File,
        Some(3),
        None,
    )];
    let right = [DirectoryEntrySnapshot::new(
        "unknown.bin",
        DirectoryEntryKind::File,
        Some(3),
        None,
    )];
    let report =
        compare_directories(&left, &right).unwrap_or_else(|error| panic!("comparison: {error}"));
    assert!(matches!(
        plan_directory_sync(
            &report,
            DirectorySyncDirection::LeftToRight,
            DirectorySyncDeletePolicy::PreserveDestination
        ),
        Err(DirectorySyncPlanError::UncertainRow { path }) if path == "unknown.bin"
    ));
}
