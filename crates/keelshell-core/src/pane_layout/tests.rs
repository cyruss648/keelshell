use super::*;

fn id(value: u128) -> PaneNodeId {
    PaneNodeId(Uuid::from_u128(value))
}

fn leaf(value: u128) -> PaneNode {
    PaneNode::Leaf { id: id(value) }
}

fn branch(value: u128, first: u128, second: u128) -> PaneNode {
    PaneNode::Split {
        id: id(value),
        axis: SplitAxis::Horizontal,
        ratio_bps: 5000,
        first: id(first),
        second: id(second),
    }
}

fn three_panes() -> PaneLayout {
    PaneLayout {
        root: id(10),
        active: id(2),
        nodes: vec![
            leaf(3),
            branch(20, 2, 3),
            leaf(1),
            branch(10, 1, 20),
            leaf(2),
        ],
    }
}

fn assert_rejected_unchanged(layout: &PaneLayout) {
    assert!(layout.validate().is_err(), "{layout:?}");
    assert!(layout.leaf_ids().is_err(), "{layout:?}");
    let mut candidate = layout.clone();
    assert!(
        candidate
            .split(id(1), id(4), id(30), SplitAxis::Vertical, 5000)
            .is_err()
    );
    assert_eq!(&candidate, layout);
    assert!(candidate.close(id(1)).is_err());
    assert_eq!(&candidate, layout);
    assert!(candidate.set_active(id(1)).is_err());
    assert_eq!(&candidate, layout);
    assert!(candidate.set_ratio(id(10), 6000).is_err());
    assert_eq!(&candidate, layout);
}

#[test]
fn empty_layout_is_rejected_before_any_mutation() {
    assert_rejected_unchanged(&PaneLayout {
        root: id(1),
        active: id(1),
        nodes: vec![],
    });
}

#[test]
fn node_count_is_bounded_before_traversal() {
    assert_rejected_unchanged(&PaneLayout {
        root: id(1),
        active: id(1),
        nodes: (1..=64).map(leaf).collect(),
    });
}

#[test]
fn leaf_count_is_bounded_even_when_total_node_count_is_small() {
    let layout = PaneLayout {
        root: id(1),
        active: id(1),
        nodes: (1..=33).map(leaf).collect(),
    };
    assert_eq!(
        layout.validate(),
        Err(ValidationError::new(
            "pane_layout.nodes",
            "must contain at most 32 leaf panes",
        ))
    );
    assert_rejected_unchanged(&layout);
}

#[test]
fn nil_ids_are_rejected_in_every_identity_position() {
    let mut cases = Vec::new();
    let mut layout = three_panes();
    layout.root = id(0);
    cases.push(layout);
    let mut layout = three_panes();
    layout.active = id(0);
    cases.push(layout);
    let mut layout = three_panes();
    layout.nodes[0] = leaf(0);
    cases.push(layout);
    let mut layout = three_panes();
    layout.nodes[1] = branch(0, 2, 3);
    cases.push(layout);
    let mut layout = three_panes();
    layout.nodes[1] = branch(20, 0, 3);
    cases.push(layout);
    let mut layout = three_panes();
    layout.nodes[1] = branch(20, 2, 0);
    cases.push(layout);
    for layout in cases {
        assert_rejected_unchanged(&layout);
    }
    assert!(PaneLayout::single(id(0)).is_err());
}

#[test]
fn duplicate_ids_are_rejected_across_leaf_and_split_kinds() {
    for duplicate in [leaf(1), branch(1, 2, 3), leaf(20), branch(20, 1, 3)] {
        let mut layout = three_panes();
        layout.nodes.push(duplicate);
        assert_rejected_unchanged(&layout);
    }
}

#[test]
fn missing_root_and_either_missing_child_are_rejected() {
    let mut layout = three_panes();
    layout.root = id(999);
    assert_rejected_unchanged(&layout);
    for replacement in [branch(20, 999, 3), branch(20, 2, 999)] {
        let mut layout = three_panes();
        layout.nodes[1] = replacement;
        assert_rejected_unchanged(&layout);
    }
}

#[test]
fn missing_or_split_active_node_is_rejected() {
    for active in [id(999), id(10), id(20)] {
        let mut layout = three_panes();
        layout.active = active;
        assert_rejected_unchanged(&layout);
    }
}

#[test]
fn identical_split_children_are_rejected() {
    let mut layout = three_panes();
    layout.nodes[1] = branch(20, 2, 2);
    assert_rejected_unchanged(&layout);
}

#[test]
fn self_and_ancestor_cycles_are_rejected_iteratively() {
    for replacement in [
        branch(10, 10, 20),
        branch(10, 1, 10),
        branch(20, 10, 3),
        branch(20, 2, 10),
    ] {
        let mut layout = three_panes();
        let target = if replacement.id() == id(10) { 3 } else { 1 };
        layout.nodes[target] = replacement;
        assert_rejected_unchanged(&layout);
    }
}

#[test]
fn shared_leaf_and_shared_subtree_are_rejected() {
    let mut layout = three_panes();
    layout.nodes[1] = branch(20, 1, 3);
    assert_rejected_unchanged(&layout);

    let layout = PaneLayout {
        root: id(10),
        active: id(1),
        nodes: vec![
            branch(10, 20, 30),
            branch(20, 1, 40),
            branch(30, 2, 40),
            branch(40, 3, 4),
            leaf(1),
            leaf(2),
            leaf(3),
            leaf(4),
        ],
    };
    assert_rejected_unchanged(&layout);
}

#[test]
fn unreachable_nodes_and_disconnected_cycles_are_rejected() {
    let mut layout = three_panes();
    layout.nodes.push(leaf(4));
    assert_rejected_unchanged(&layout);
    layout.active = id(4);
    assert_rejected_unchanged(&layout);

    let mut layout = three_panes();
    layout
        .nodes
        .extend([branch(30, 40, 4), branch(40, 30, 5), leaf(4), leaf(5)]);
    assert_rejected_unchanged(&layout);
}

#[test]
fn unreachable_second_parent_is_rejected() {
    let mut layout = three_panes();
    layout.nodes.extend([branch(30, 1, 4), leaf(4)]);
    assert_rejected_unchanged(&layout);
}

#[test]
fn invalid_ratios_are_rejected_even_in_an_unreachable_split() {
    for ratio in [
        0,
        MIN_SPLIT_RATIO_BPS - 1,
        MAX_SPLIT_RATIO_BPS + 1,
        u16::MAX,
    ] {
        let mut invalid_branch = branch(20, 2, 3);
        if let PaneNode::Split { ratio_bps, .. } = &mut invalid_branch {
            *ratio_bps = ratio;
        }
        let mut layout = three_panes();
        layout.nodes[1] = invalid_branch.clone();
        assert_rejected_unchanged(&layout);
        let mut layout = PaneLayout {
            root: id(1),
            active: id(1),
            nodes: vec![leaf(1), leaf(2), leaf(3)],
        };
        layout.nodes.push(invalid_branch);
        assert_rejected_unchanged(&layout);
    }
}

#[test]
fn root_depth_zero_and_depth_eight_are_valid_but_nine_is_rejected() -> Result<(), ValidationError> {
    let mut layout = PaneLayout::single(id(1))?;
    for depth in 1..=MAX_LAYOUT_DEPTH {
        layout.split(
            id(1),
            id(100 + depth as u128),
            id(200 + depth as u128),
            SplitAxis::Horizontal,
            5000,
        )?;
    }
    layout.validate()?;
    assert_eq!(layout.leaf_ids()?.len(), MAX_LAYOUT_DEPTH + 1);

    // Extend the deepest first path directly to exercise validation separately
    // from the split operation's transaction.
    for node in &mut layout.nodes {
        if let PaneNode::Split { first, .. } = node
            && *first == id(1)
        {
            *first = id(300);
        }
    }
    layout.nodes.extend([branch(300, 1, 400), leaf(400)]);
    assert_rejected_unchanged(&layout);
    Ok(())
}
