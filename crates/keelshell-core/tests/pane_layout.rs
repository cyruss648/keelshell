use keelshell_core::{
    MAX_LAYOUT_DEPTH, MAX_LAYOUT_PANES, MAX_SPLIT_RATIO_BPS, MIN_SPLIT_RATIO_BPS, PaneLayout,
    PaneNode, PaneNodeId, SplitAxis, ValidationError,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn id(value: u128) -> PaneNodeId {
    PaneNodeId(Uuid::from_u128(value))
}

fn branch(value: u128, first: u128, second: u128, axis: SplitAxis, ratio_bps: u16) -> PaneNode {
    PaneNode::Split {
        id: id(value),
        first: id(first),
        second: id(second),
        axis,
        ratio_bps,
    }
}

fn nested_layout() -> PaneLayout {
    // Root: 1 | (2 above (3 | 4)). Storage deliberately differs from screen order.
    PaneLayout {
        root: id(10),
        active: id(2),
        nodes: vec![
            PaneNode::Leaf { id: id(4) },
            branch(30, 3, 4, SplitAxis::Horizontal, 7000),
            PaneNode::Leaf { id: id(2) },
            branch(10, 1, 20, SplitAxis::Horizontal, 6000),
            PaneNode::Leaf { id: id(1) },
            branch(20, 2, 30, SplitAxis::Vertical, 3500),
            PaneNode::Leaf { id: id(3) },
        ],
    }
}

fn assert_rejected_without_change(
    layout: &PaneLayout,
    action: impl FnOnce(&mut PaneLayout) -> Result<(), ValidationError>,
) -> TestResult {
    let before = serde_json::to_vec(layout)?;
    let mut candidate = layout.clone();
    assert!(action(&mut candidate).is_err());
    assert_eq!(&candidate, layout);
    assert_eq!(serde_json::to_vec(&candidate)?, before);
    Ok(())
}

#[test]
fn split_single_leaf_keeps_identity_and_sets_new_leaf_active() -> TestResult {
    for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
        let mut layout = PaneLayout::single(id(1))?;
        layout.split(id(1), id(2), id(10), axis, 6500)?;
        assert_eq!(layout.root, id(10));
        assert_eq!(layout.active, id(2));
        assert_eq!(layout.leaf_ids()?, vec![id(1), id(2)]);
        assert!(layout.nodes.contains(&PaneNode::Leaf { id: id(1) }));
        assert!(layout.nodes.contains(&branch(10, 1, 2, axis, 6500)));
        layout.close(id(2))?;
        assert_eq!(layout, PaneLayout::single(id(1))?);
    }
    Ok(())
}

#[test]
fn deep_split_replaces_either_child_reference_without_changing_other_branches() -> TestResult {
    for (target, expected_order, first, second) in [
        (3, vec![id(1), id(2), id(3), id(5), id(4)], 40, 4),
        (4, vec![id(1), id(2), id(3), id(4), id(5)], 3, 40),
    ] {
        let mut layout = nested_layout();
        let original_nodes = layout.nodes.clone();
        layout.split(id(target), id(5), id(40), SplitAxis::Vertical, 2500)?;
        assert_eq!(layout.root, id(10));
        assert_eq!(layout.active, id(5));
        assert_eq!(layout.leaf_ids()?, expected_order);
        let mut expected_nodes = original_nodes;
        expected_nodes[1] = branch(30, first, second, SplitAxis::Horizontal, 7000);
        expected_nodes.push(branch(40, target, 5, SplitAxis::Vertical, 2500));
        expected_nodes.push(PaneNode::Leaf { id: id(5) });
        assert_eq!(layout.nodes, expected_nodes);
    }
    Ok(())
}

#[test]
fn closing_active_leaf_promotes_sibling_subtree_and_selects_its_first_leaf() -> TestResult {
    let mut layout = nested_layout();
    layout.close(id(2))?;
    assert_eq!(layout.active, id(3));
    assert_eq!(layout.root, id(10));
    assert_eq!(layout.leaf_ids()?, vec![id(1), id(3), id(4)]);
    assert_eq!(
        layout.nodes,
        vec![
            PaneNode::Leaf { id: id(4) },
            branch(30, 3, 4, SplitAxis::Horizontal, 7000),
            branch(10, 1, 30, SplitAxis::Horizontal, 6000),
            PaneNode::Leaf { id: id(1) },
            PaneNode::Leaf { id: id(3) },
        ]
    );
    Ok(())
}

#[test]
fn closing_root_child_promotes_whole_subtree_including_nested_ratios() -> TestResult {
    let mut layout = nested_layout();
    layout.set_active(id(1))?;
    let expected_nodes = layout
        .nodes
        .iter()
        .filter(|node| node.id() != id(1) && node.id() != id(10))
        .cloned()
        .collect::<Vec<_>>();
    layout.close(id(1))?;
    assert_eq!(
        layout,
        PaneLayout {
            root: id(20),
            active: id(2),
            nodes: expected_nodes,
        }
    );
    assert_eq!(layout.leaf_ids()?, vec![id(2), id(3), id(4)]);
    Ok(())
}

#[test]
fn closing_second_child_selects_first_leaf_of_first_sibling_subtree() -> TestResult {
    let mut layout = nested_layout();
    for node in &mut layout.nodes {
        if node.id() == id(10) {
            *node = branch(10, 20, 1, SplitAxis::Horizontal, 6000);
        }
        if node.id() == id(20) {
            *node = branch(20, 30, 2, SplitAxis::Vertical, 3500);
        }
    }
    layout.set_active(id(1))?;
    layout.close(id(1))?;
    assert_eq!(layout.root, id(20));
    assert_eq!(layout.active, id(3));
    assert_eq!(layout.leaf_ids()?, vec![id(3), id(4), id(2)]);
    assert!(
        layout
            .nodes
            .contains(&branch(30, 3, 4, SplitAxis::Horizontal, 7000))
    );
    Ok(())
}

#[test]
fn closing_inactive_leaf_preserves_selection_inside_or_outside_sibling_subtree() -> TestResult {
    for active in [1, 3, 4] {
        let mut layout = nested_layout();
        layout.set_active(id(active))?;
        layout.close(id(2))?;
        assert_eq!(layout.active, id(active));
        assert_eq!(layout.leaf_ids()?, vec![id(1), id(3), id(4)]);
    }
    Ok(())
}

#[test]
fn closing_deep_first_or_second_leaf_rewrites_the_correct_parent_edge() -> TestResult {
    for (closed, sibling) in [(3, 4), (4, 3)] {
        let mut layout = nested_layout();
        layout.set_active(id(closed))?;
        layout.close(id(closed))?;
        assert_eq!(layout.active, id(sibling));
        assert_eq!(layout.leaf_ids()?, vec![id(1), id(2), id(sibling)]);
        assert!(
            layout
                .nodes
                .contains(&branch(20, 2, sibling, SplitAxis::Vertical, 3500))
        );
        assert!(
            layout
                .nodes
                .contains(&branch(10, 1, 20, SplitAxis::Horizontal, 6000))
        );
    }
    Ok(())
}

#[test]
fn repeated_close_reaches_one_leaf_and_then_refuses_without_mutating() -> TestResult {
    let mut layout = nested_layout();
    for pane in [2, 4, 1] {
        layout.close(id(pane))?;
        layout.validate()?;
    }
    assert_eq!(layout, PaneLayout::single(id(3))?);
    assert_rejected_without_change(&layout, |value| value.close(id(3)))?;
    Ok(())
}

#[test]
fn setters_preserve_structure_and_accept_inclusive_ratio_bounds() -> TestResult {
    let mut layout = nested_layout();
    let original = layout.clone();
    layout.set_active(id(4))?;
    assert_eq!(layout.nodes, original.nodes);
    assert_eq!(layout.root, original.root);
    for ratio in [MIN_SPLIT_RATIO_BPS, MAX_SPLIT_RATIO_BPS] {
        layout.set_ratio(id(20), ratio)?;
        let mut expected = original.nodes.clone();
        expected[5] = branch(20, 2, 30, SplitAxis::Vertical, ratio);
        assert_eq!(layout.nodes, expected);
        assert_eq!(layout.active, id(4));
        layout.validate()?;
    }
    Ok(())
}

#[test]
fn invalid_targets_and_ratios_leave_complete_layout_unchanged() -> TestResult {
    let layout = nested_layout();
    for target in [id(0), id(999), id(10), id(20)] {
        assert_rejected_without_change(&layout, |value| value.close(target))?;
        assert_rejected_without_change(&layout, |value| value.set_active(target))?;
        assert_rejected_without_change(&layout, |value| {
            value.split(target, id(5), id(40), SplitAxis::Vertical, 5000)
        })?;
    }
    for target in [id(0), id(999), id(1), id(2)] {
        assert_rejected_without_change(&layout, |value| value.set_ratio(target, 5000))?;
    }
    for ratio in [
        0,
        MIN_SPLIT_RATIO_BPS - 1,
        MAX_SPLIT_RATIO_BPS + 1,
        u16::MAX,
    ] {
        assert_rejected_without_change(&layout, |value| value.set_ratio(id(20), ratio))?;
        assert_rejected_without_change(&layout, |value| {
            value.split(id(3), id(5), id(40), SplitAxis::Vertical, ratio)
        })?;
    }
    Ok(())
}

#[test]
fn new_split_ids_must_be_non_nil_distinct_and_globally_unused() -> TestResult {
    let layout = nested_layout();
    for (new_pane, split_node) in [
        (0, 40),
        (5, 0),
        (5, 5),
        (1, 40),
        (5, 1),
        (10, 40),
        (5, 10),
        (3, 40),
        (5, 3),
    ] {
        assert_rejected_without_change(&layout, |value| {
            value.split(
                id(3),
                id(new_pane),
                id(split_node),
                SplitAxis::Horizontal,
                5000,
            )
        })?;
    }
    Ok(())
}

#[test]
fn depth_limit_rejection_preserves_selection_and_all_original_nodes() -> TestResult {
    let mut layout = PaneLayout::single(id(1))?;
    for depth in 1..=MAX_LAYOUT_DEPTH {
        layout.split(
            id(1),
            id(100 + depth as u128),
            id(200 + depth as u128),
            SplitAxis::Vertical,
            MIN_SPLIT_RATIO_BPS,
        )?;
    }
    layout.set_active(id(101))?;
    assert_rejected_without_change(&layout, |value| {
        value.split(id(1), id(2), id(3), SplitAxis::Horizontal, 5000)
    })?;
    // The depth limit applies to the changed branch, not to the whole tree's
    // ability to accept a split on a shallower branch.
    layout.split(id(101), id(2), id(3), SplitAxis::Horizontal, 5000)?;
    assert_eq!(layout.active, id(2));
    assert_eq!(layout.leaf_ids()?.len(), MAX_LAYOUT_DEPTH + 2);
    Ok(())
}

#[test]
fn thirty_two_panes_and_sixty_three_nodes_are_valid_but_one_more_is_atomic_error() -> TestResult {
    let mut layout = PaneLayout::single(id(1))?;
    let mut next = 2;
    // Split all current leaves per round to reach 32 leaves at depth five.
    for _ in 0..5 {
        for pane in layout.leaf_ids()? {
            layout.split(
                pane,
                id(next),
                id(next + 1),
                SplitAxis::Horizontal,
                MAX_SPLIT_RATIO_BPS,
            )?;
            next += 2;
        }
    }
    assert_eq!(layout.leaf_ids()?.len(), MAX_LAYOUT_PANES);
    assert_eq!(layout.nodes.len(), 63);
    layout.validate()?;
    assert_rejected_without_change(&layout, |value| {
        value.split(id(1), id(next), id(next + 1), SplitAxis::Vertical, 5000)
    })?;
    layout.close(id(1))?;
    let survivor = layout.active;
    layout.split(survivor, id(next), id(next + 1), SplitAxis::Vertical, 5000)?;
    assert_eq!(layout.leaf_ids()?.len(), MAX_LAYOUT_PANES);
    Ok(())
}

#[test]
fn repeated_edits_round_trip_with_stable_screen_order_and_node_identities() -> TestResult {
    let mut layout = nested_layout();
    for step in 0..64 {
        let new_pane = id(100 + step * 2);
        let split_node = id(101 + step * 2);
        let before = layout.clone();
        layout.split(id(3), new_pane, split_node, SplitAxis::Vertical, 4000)?;
        layout = serde_json::from_slice(&serde_json::to_vec(&layout)?)?;
        assert_eq!(
            layout.leaf_ids()?,
            vec![id(1), id(2), id(3), new_pane, id(4)]
        );
        layout.close(new_pane)?;
        layout.set_active(before.active)?;
        assert_eq!(layout, before);
    }
    Ok(())
}

#[test]
fn json_contract_uses_uuid_strings_struct_variants_and_snake_case_axes() -> TestResult {
    let layout = nested_layout();
    let document = serde_json::to_value(&layout)?;
    assert_eq!(serde_json::to_value(id(1))?, json!(id(1).0.to_string()));
    assert_eq!(
        serde_json::to_value(PaneNode::Leaf { id: id(1) })?,
        json!({"type": "leaf", "id": id(1)})
    );
    assert_eq!(
        serde_json::to_value(branch(10, 1, 2, SplitAxis::Horizontal, 4000))?,
        json!({
            "type": "split", "id": id(10), "axis": "horizontal",
            "ratio_bps": 4000, "first": id(1), "second": id(2)
        })
    );
    assert_eq!(
        serde_json::to_value(SplitAxis::Vertical)?,
        json!("vertical")
    );
    let decoded: PaneLayout = serde_json::from_value(document)?;
    decoded.validate()?;
    assert_eq!(decoded, layout);
    assert_eq!(decoded.leaf_ids()?, vec![id(1), id(2), id(3), id(4)]);
    Ok(())
}

#[test]
fn unknown_fields_are_rejected_at_layout_leaf_and_split_boundaries() -> TestResult {
    let layout = nested_layout();
    let mut document = serde_json::to_value(&layout)?;
    document["unexpected"] = json!(true);
    assert!(serde_json::from_value::<PaneLayout>(document).is_err());

    for (node, field) in [(0, "unexpected"), (0, "axis"), (1, "unexpected")] {
        let mut document = serde_json::to_value(&layout)?;
        document["nodes"][node][field] = json!("unrecognized");
        assert!(serde_json::from_value::<PaneLayout>(document).is_err());
    }
    for document in [
        json!({"type": "leaf", "id": id(1), "extra": 1}),
        json!({"type": "leaf", "id": id(1), "first": id(2)}),
    ] {
        assert!(serde_json::from_value::<PaneNode>(document).is_err());
    }
    Ok(())
}

#[test]
fn malformed_tags_ids_axes_and_ratios_are_not_accepted_as_wire_values() -> TestResult {
    let layout = nested_layout();
    for (node, field, value) in [
        (0, "type", json!("Leaf")),
        (0, "type", json!("unknown")),
        (0, "id", json!("not-a-uuid")),
        (0, "id", json!({"uuid": id(1)})),
        (1, "axis", json!("Horizontal")),
        (1, "axis", json!("diagonal")),
        (1, "axis", json!({"horizontal": {}})),
        (1, "ratio_bps", json!(-1)),
        (1, "ratio_bps", json!(65_536)),
        (1, "ratio_bps", json!(0.5)),
        (1, "ratio_bps", json!("5000")),
        (1, "first", Value::Null),
    ] {
        let mut document = serde_json::to_value(&layout)?;
        document["nodes"][node][field] = value;
        assert!(
            serde_json::from_value::<PaneLayout>(document).is_err(),
            "node {node}, field {field}"
        );
    }
    Ok(())
}

#[test]
fn required_layout_and_variant_fields_cannot_be_missing_or_null() -> TestResult {
    let source = serde_json::to_value(nested_layout())?;
    for field in ["root", "active", "nodes"] {
        let mut document = source.clone();
        document
            .as_object_mut()
            .ok_or("layout object")?
            .remove(field);
        assert!(serde_json::from_value::<PaneLayout>(document).is_err());
        let mut document = source.clone();
        document[field] = Value::Null;
        assert!(serde_json::from_value::<PaneLayout>(document).is_err());
    }
    for (node, fields) in [
        (0, &["type", "id"][..]),
        (
            1,
            &["type", "id", "axis", "ratio_bps", "first", "second"][..],
        ),
    ] {
        for field in fields {
            let mut document = source.clone();
            document["nodes"][node]
                .as_object_mut()
                .ok_or("node object")?
                .remove(*field);
            assert!(serde_json::from_value::<PaneLayout>(document).is_err());
        }
    }
    Ok(())
}

#[test]
fn duplicate_json_fields_are_rejected_including_leaf_identity() -> TestResult {
    let uuid = id(1).0.to_string();
    for wire in [
        format!(r#"{{"type":"leaf","id":"{uuid}","id":"{uuid}"}}"#),
        format!(r#"{{"type":"leaf","type":"leaf","id":"{uuid}"}}"#),
        format!(
            r#"{{"type":"split","id":"{uuid}","axis":"horizontal","ratio_bps":5000,"ratio_bps":6000,"first":"{uuid}","second":"{uuid}"}}"#
        ),
    ] {
        assert!(serde_json::from_str::<PaneNode>(&wire).is_err());
    }
    let wire = format!(
        r#"{{"root":"{uuid}","root":"{uuid}","active":"{uuid}","nodes":[{{"type":"leaf","id":"{uuid}"}}]}}"#
    );
    assert!(serde_json::from_str::<PaneLayout>(&wire).is_err());
    Ok(())
}

#[test]
fn domain_invalid_json_requires_validation_and_cannot_be_repaired_by_mutators() -> TestResult {
    let source = serde_json::to_value(nested_layout())?;
    for (node, field, value) in [
        (0, "id", json!(Uuid::nil())),
        (1, "ratio_bps", json!(999)),
        (1, "first", json!(id(999))),
        (1, "second", json!(id(3))),
        (1, "first", json!(id(10))),
    ] {
        let mut document = source.clone();
        document["nodes"][node][field] = value;
        let layout: PaneLayout = serde_json::from_value(document)?;
        assert!(layout.validate().is_err());
        assert!(layout.leaf_ids().is_err());
        assert_rejected_without_change(&layout, |value| value.set_active(id(1)))?;
        assert_rejected_without_change(&layout, |value| value.close(id(4)))?;
        assert_rejected_without_change(&layout, |value| value.set_ratio(id(30), 5000))?;
        assert_rejected_without_change(&layout, |value| {
            value.split(id(1), id(5), id(40), SplitAxis::Horizontal, 5000)
        })?;
    }
    Ok(())
}
