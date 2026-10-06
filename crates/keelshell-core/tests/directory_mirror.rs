use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDeletePolicy as Policy, DirectorySyncDirection as Direction,
    DirectorySyncOperation as Operation, DirectorySyncPlanError as Error, compare_directories,
    directory_mirror_conflicts, hash_directory_content, plan_directory_mirror, plan_directory_sync,
};

fn file(path: &str, bytes: &[u8]) -> Entry {
    Entry::new(path, Kind::File, Some(bytes.len() as u64), Some(0)).with_content_hash(
        hash_directory_content(bytes).unwrap_or_else(|error| panic!("fixture hash: {error}")),
    )
}
fn dir(path: &str) -> Entry {
    Entry::new(path, Kind::Directory, None, Some(0))
}

#[test]
fn exact_direction_and_restricted_policy_are_part_of_the_confirmation() {
    let report = compare_directories(
        &[file("source", b"new")],
        &[file("target", b"old"), dir("empty")],
    )
    .unwrap_or_else(|error| panic!("fixture report: {error}"));
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let mirror = plan_directory_mirror(&report, direction)
            .unwrap_or_else(|error| panic!("bounded plan: {error}"));
        assert!(mirror.is_bounded_mirror());
        let generic = plan_directory_sync(&report, direction, Policy::IncludeDeletes)
            .unwrap_or_else(|error| panic!("intent plan: {error}"));
        assert!(!generic.is_bounded_mirror());
        assert_ne!(mirror.review_fingerprint(), generic.review_fingerprint());
        assert_eq!(mirror.review_fingerprint().len(), 64);
        assert!(mirror.clone().confirm(generic.review_token()).is_err());
        assert!(mirror.clone().confirm(mirror.review_token()).is_ok());
        let deletes: Vec<_> = mirror
            .operations()
            .iter()
            .filter_map(|op| {
                if let Operation::Delete { path, .. } = op {
                    Some(path.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            deletes,
            if direction == Direction::LeftToRight {
                vec!["empty", "target"]
            } else {
                vec!["source"]
            }
        );
    }
}
#[test]
fn no_incomplete_content_or_kind_conflict_can_reach_a_mirror_executor() {
    let missing = Entry::new("file", Kind::File, Some(0), Some(0));
    let report = compare_directories(&[], &[missing])
        .unwrap_or_else(|error| panic!("metadata report: {error}"));
    assert!(matches!(
        plan_directory_mirror(&report, Direction::LeftToRight),
        Err(Error::IncompleteMirrorContent { .. })
    ));
    let report = compare_directories(&[file("same", b"x")], &[dir("same")])
        .unwrap_or_else(|error| panic!("type conflict: {error}"));
    assert!(matches!(
        plan_directory_mirror(&report, Direction::LeftToRight),
        Err(Error::UnsafeMirrorRow { .. })
    ));
}
#[test]
fn complete_nonempty_subtrees_are_reviewed_children_before_parents_in_both_directions() {
    let nested = [
        dir("tree"),
        dir("tree/deep"),
        file("tree/deep/child", b"keep"),
        file("tree/leaf", b"leaf"),
    ];
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let report = if direction == Direction::LeftToRight {
            compare_directories(&[], &nested)
        } else {
            compare_directories(&nested, &[])
        }
        .unwrap_or_else(|error| panic!("complete report: {error}"));
        let plan = plan_directory_mirror(&report, direction)
            .unwrap_or_else(|error| panic!("recursive plan: {error}"));
        let paths: Vec<_> = plan
            .operations()
            .iter()
            .filter_map(|op| match op {
                Operation::Delete { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(paths, ["tree/deep/child", "tree/deep", "tree/leaf", "tree"]);
    }
}
#[test]
fn mirror_budget_refuses_deep_trees_and_combined_large_content_without_approval() {
    let too_deep: Vec<_> = (1..=33)
        .map(|depth| dir(&vec!["a"; depth].join("/")))
        .collect();
    let report =
        compare_directories(&[], &too_deep).unwrap_or_else(|error| panic!("deep report: {error}"));
    assert!(matches!(
        plan_directory_mirror(&report, Direction::LeftToRight),
        Err(Error::MirrorBudget { .. })
    ));
    let files: Vec<_> = (0..5)
        .map(|index| {
            Entry::new(
                format!("large-{index}"),
                Kind::File,
                Some(64 * 1024 * 1024),
                Some(0),
            )
            .with_content_hash([index; 32])
        })
        .collect();
    let report =
        compare_directories(&[], &files).unwrap_or_else(|error| panic!("large report: {error}"));
    assert!(matches!(
        plan_directory_mirror(&report, Direction::LeftToRight),
        Err(Error::MirrorBudget { .. })
    ));
}
#[test]
fn links_names_case_collisions_and_missing_parent_evidence_fail_closed() {
    for kind in [Kind::Symlink, Kind::Other] {
        let report = compare_directories(&[], &[Entry::new("object", kind, None, Some(0))])
            .unwrap_or_else(|error| panic!("report: {error}"));
        assert!(plan_directory_mirror(&report, Direction::LeftToRight).is_err());
    }
    for name in [
        "CON.txt",
        "NUL .txt",
        "CONIN$ .log",
        "COM¹",
        "LPT9",
        "CONIN$",
        "CLOCK$",
        "a:stream",
        "a.",
        "a ",
    ] {
        let report = compare_directories(&[], &[file(name, b"x")])
            .unwrap_or_else(|error| panic!("report: {error}"));
        assert!(
            plan_directory_mirror(&report, Direction::LeftToRight).is_err(),
            "{name}"
        );
    }
    let report = compare_directories(&[file("A", b"x")], &[file("a", b"y")])
        .unwrap_or_else(|error| panic!("case collision: {error}"));
    assert!(plan_directory_mirror(&report, Direction::LeftToRight).is_err());
    let report = compare_directories(&[], &[file("missing/child", b"x")])
        .unwrap_or_else(|error| panic!("incomplete hierarchy: {error}"));
    assert!(plan_directory_mirror(&report, Direction::LeftToRight).is_err());
    for name in ["../escape", "/absolute", "a//b", "a\\b", "bad\nname"] {
        assert!(
            compare_directories(&[], &[file(name, b"x")]).is_err(),
            "{name}"
        );
    }
}

#[test]
fn the_read_only_conflict_list_does_not_truncate_after_one_hundred_rows() {
    let entries: Vec<_> = (0..150)
        .map(|index| Entry::new(format!("link-{index:03}"), Kind::Symlink, None, Some(0)))
        .collect();
    let report = compare_directories(&[], &entries)
        .unwrap_or_else(|error| panic!("bounded report: {error}"));
    let conflicts = directory_mirror_conflicts(&report, Direction::LeftToRight);
    assert_eq!(conflicts.len(), 150);
    assert_eq!(
        conflicts
            .first()
            .unwrap_or_else(|| panic!("first conflict"))
            .path,
        "link-000"
    );
    assert_eq!(
        conflicts
            .last()
            .unwrap_or_else(|| panic!("last conflict"))
            .path,
        "link-149"
    );
    assert!(plan_directory_mirror(&report, Direction::LeftToRight).is_err());
}
