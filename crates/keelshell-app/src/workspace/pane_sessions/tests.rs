use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn replacing_inactive_session_preserves_owner_geometry_and_other_selection() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let geometry = panes.layout().clone();
    let owner = owner(&panes, 1)?;
    let selected = panes.active_session();
    panes.replace(1, 100)?;
    assert_eq!(panes.layout(), &geometry);
    assert_eq!(panes.pane_for(100), Some(owner));
    assert_eq!(panes.tabs_in(owner), &[100, 5, 6]);
    assert_eq!(panes.selected_in(owner), Some(5));
    assert_eq!(panes.active_session(), selected);
    assert_eq!(panes.pane_for(1), None);
    Ok(())
}

#[test]
fn replacing_active_session_preserves_leaf_selection_at_full_capacity() -> TestResult {
    let mut panes = full_workspace()?;
    let old = panes.active_session().ok_or("expected active session")?;
    let geometry = panes.layout().clone();
    panes.replace(old, 1000)?;
    assert_eq!(panes.layout(), &geometry);
    assert_eq!(panes.active_session(), Some(1000));
    assert_eq!(panes.session_count(), MAX_LAYOUT_TABS);
    Ok(())
}

#[test]
fn replacing_unknown_or_duplicate_key_is_atomic() -> TestResult {
    let panes = nested_with_tabs()?;
    assert_eq!(
        rejected(&panes, |panes| panes.replace(99, 100))?,
        PaneSessionError::UnknownSession
    );
    assert_eq!(
        rejected(&panes, |panes| panes.replace(1, 2))?,
        PaneSessionError::DuplicateSession
    );
    Ok(())
}

fn populated(keys: &[u32], active: Option<u32>) -> Result<SessionPanes<u32>, PaneSessionError> {
    let mut panes = SessionPanes::new();
    panes.reconcile(keys, active)?;
    Ok(panes)
}

fn owner(panes: &SessionPanes<u32>, key: u32) -> Result<PaneNodeId, &'static str> {
    panes.pane_for(key).ok_or("expected a registered key")
}

fn parent_of(panes: &SessionPanes<u32>, child: PaneNodeId) -> Result<PaneNodeId, &'static str> {
    panes
        .layout()
        .nodes
        .iter()
        .find_map(|node| match node {
            PaneNode::Split {
                id, first, second, ..
            } if *first == child || *second == child => Some(*id),
            _ => None,
        })
        .ok_or("expected a parent split")
}

fn nested() -> Result<SessionPanes<u32>, PaneSessionError> {
    // 1 | (2 above (3 | 4)), with one session in every leaf.
    let mut panes = populated(&[1], Some(1))?;
    panes.split(1, 2, SplitAxis::Horizontal)?;
    panes.split(2, 3, SplitAxis::Vertical)?;
    panes.split(3, 4, SplitAxis::Horizontal)?;
    Ok(panes)
}

fn nested_with_tabs() -> Result<SessionPanes<u32>, PaneSessionError> {
    let mut panes = nested()?;
    assert!(panes.select(1));
    panes.reconcile(&[1, 2, 3, 4, 5, 6], Some(5))?;
    assert!(panes.select(2));
    panes.reconcile(&[1, 2, 3, 4, 5, 6, 7, 8], Some(7))?;
    Ok(panes)
}

fn full_workspace() -> Result<SessionPanes<u32>, PaneSessionError> {
    let mut keys: Vec<_> = (0..32).collect();
    let mut panes = populated(&keys, Some(0))?;
    for first in [32, 64, 96] {
        panes.split(0, first, SplitAxis::Horizontal)?;
        keys.extend(first..first + 32);
        assert!(panes.select(first));
        panes.reconcile(&keys, Some(first))?;
    }
    Ok(panes)
}

fn rejected<T>(
    original: &SessionPanes<u32>,
    action: impl FnOnce(&mut SessionPanes<u32>) -> Result<T, PaneSessionError>,
) -> Result<PaneSessionError, Box<dyn std::error::Error>> {
    let mut candidate = original.clone();
    let error = action(&mut candidate)
        .err()
        .ok_or("expected the operation to fail")?;
    assert_eq!(&candidate, original);
    Ok(error)
}

fn assert_keys(panes: &SessionPanes<u32>, expected: &[u32]) -> TestResult {
    let leaves = panes.layout().leaf_ids()?;
    let mut actual = Vec::new();
    for leaf in leaves {
        let tabs = panes.tabs_in(leaf);
        assert!(tabs.len() <= MAX_LAYOUT_TABS_PER_PANE);
        match panes.selected_in(leaf) {
            Some(key) => assert!(tabs.contains(&key)),
            None => assert!(tabs.is_empty()),
        }
        for key in tabs {
            assert_eq!(panes.pane_for(*key), Some(leaf));
        }
        actual.extend_from_slice(tabs);
    }
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
    assert!(actual.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(actual.len() <= MAX_LAYOUT_TABS);
    Ok(())
}

#[test]
fn new_and_empty_reconcile_keep_one_valid_uuid_leaf() -> TestResult {
    let mut panes = SessionPanes::<u32>::new();
    let leaf = panes.active_pane();
    assert!(!leaf.0.is_nil());
    assert_eq!(panes.layout().root, leaf);
    assert_eq!(panes.layout().leaf_ids()?, vec![leaf]);
    assert_eq!(panes.active_session(), None);
    assert_eq!(panes.selected_in(leaf), None);
    assert!(panes.tabs_in(leaf).is_empty());
    assert!(panes.displayed_sessions().is_empty());
    assert!(panes.can_add());
    assert!(!panes.can_split(1));
    let before = panes.clone();
    panes.reconcile(&[], None)?;
    assert_eq!(panes, before);
    panes.reconcile(&[9, 2, 7], None)?;
    assert_eq!(panes.active_pane(), leaf);
    assert_eq!(panes.active_session(), Some(9));
    assert_eq!(panes.tabs_in(leaf), &[9, 2, 7]);
    Ok(())
}

#[test]
fn authoritative_reordering_preserves_owners_local_order_and_selections() -> TestResult {
    let mut panes = populated(&[10, 20, 30, 40], Some(30))?;
    let left = owner(&panes, 10)?;
    let right = panes.split(10, 40, SplitAxis::Horizontal)?;
    assert!(panes.select(30));
    assert!(panes.select(40));
    let before = panes.clone();
    panes.reconcile(&[40, 30, 10, 20], None)?;
    assert_eq!(panes, before);
    panes.reconcile(&[20, 40, 10, 30], Some(20))?;
    assert_eq!(panes.tabs_in(left), &[10, 20, 30]);
    assert_eq!(panes.tabs_in(right), &[40]);
    assert_eq!(panes.selected_in(left), Some(20));
    assert_eq!(panes.selected_in(right), Some(40));
    assert_eq!(panes.active_pane(), left);
    assert_eq!(panes.layout().nodes, before.layout().nodes);
    assert_keys(&panes, &[10, 20, 30, 40])
}

#[test]
fn new_keys_append_in_input_order_before_the_explicit_active_is_applied() -> TestResult {
    let mut panes = populated(&[1], Some(1))?;
    let left = panes.active_pane();
    let right = panes.split(1, 2, SplitAxis::Horizontal)?;
    panes.reconcile(&[4, 2, 3, 1, 5], Some(2))?;
    assert_eq!(panes.tabs_in(left), &[1, 4, 3, 5]);
    assert_eq!(panes.tabs_in(right), &[2]);
    assert_eq!(panes.selected_in(left), Some(1));
    assert_eq!(panes.active_session(), Some(2));
    assert_eq!(panes.active_pane(), right);
    assert_keys(&panes, &[1, 2, 3, 4, 5])
}

#[test]
fn selecting_keys_keeps_each_other_leafs_selected_tab() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let left = owner(&panes, 1)?;
    let right = owner(&panes, 2)?;
    assert_eq!(panes.tabs_in(left), &[1, 5, 6]);
    assert_eq!(panes.tabs_in(right), &[2, 7, 8]);
    assert_eq!(panes.displayed_sessions(), vec![5, 7, 3, 4]);
    assert!(panes.select(6));
    assert_eq!(panes.active_pane(), left);
    assert_eq!(panes.selected_in(right), Some(7));
    assert!(panes.select(8));
    assert_eq!(panes.selected_in(left), Some(6));
    assert_eq!(panes.displayed_sessions(), vec![6, 8, 3, 4]);
    assert!(panes.select(8));
    assert_eq!(panes.active_session(), Some(8));
    Ok(())
}

#[test]
fn nested_horizontal_and_vertical_splits_preserve_leaf_identities_and_geometry() -> TestResult {
    let mut panes = populated(&[1], Some(1))?;
    let first = panes.active_pane();
    let second = panes.split(1, 2, SplitAxis::Horizontal)?;
    let root = panes.layout().root;
    let third = panes.split(2, 3, SplitAxis::Vertical)?;
    let vertical = parent_of(&panes, second)?;
    let fourth = panes.split(3, 4, SplitAxis::Horizontal)?;
    let horizontal = parent_of(&panes, third)?;
    assert_eq!(panes.layout().root, root);
    assert_eq!(
        panes.layout().leaf_ids()?,
        vec![first, second, third, fourth]
    );
    for (id, axis, first, second) in [
        (root, SplitAxis::Horizontal, first, vertical),
        (vertical, SplitAxis::Vertical, second, horizontal),
        (horizontal, SplitAxis::Horizontal, third, fourth),
    ] {
        assert!(panes.layout().nodes.contains(&PaneNode::Split {
            id,
            axis,
            ratio_bps: 5000,
            first,
            second,
        }));
    }
    assert_eq!(panes.active_pane(), third);
    assert_eq!(panes.active_session(), Some(3));
    assert_eq!(panes.displayed_sessions(), vec![1, 2, 3, 4]);
    assert_keys(&panes, &[1, 2, 3, 4])
}

#[test]
fn displayed_order_ignores_flat_node_storage_and_authoritative_session_order() -> TestResult {
    let mut panes = nested_with_tabs()?;
    panes.layout.nodes.reverse();
    panes.reconcile(&[8, 7, 6, 5, 4, 3, 2, 1], None)?;
    assert_eq!(panes.displayed_sessions(), vec![5, 7, 3, 4]);
    assert_keys(&panes, &[1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn splitting_a_registered_peer_moves_only_that_key_and_activates_primary() -> TestResult {
    let mut panes = populated(&[1, 2, 3, 4], Some(3))?;
    let original = panes.active_pane();
    let peer = panes.split(2, 3, SplitAxis::Vertical)?;
    assert_eq!(panes.tabs_in(original), &[1, 2, 4]);
    assert_eq!(panes.tabs_in(peer), &[3]);
    assert_eq!(panes.selected_in(original), Some(2));
    assert_eq!(panes.selected_in(peer), Some(3));
    assert_eq!(panes.active_pane(), original);
    assert_eq!(panes.active_session(), Some(2));
    assert_keys(&panes, &[1, 2, 3, 4])
}

#[test]
fn a_source_with_thirty_two_tabs_can_split_a_new_peer_without_overflow() -> TestResult {
    let keys: Vec<_> = (0..32).collect();
    let mut panes = populated(&keys, Some(0))?;
    let original = panes.active_pane();
    assert_eq!(panes.tabs_in(original).len(), MAX_LAYOUT_TABS_PER_PANE);
    assert!(!panes.can_add());
    assert!(panes.can_split(0));
    let peer = panes.split(0, 32, SplitAxis::Vertical)?;
    assert_eq!(panes.tabs_in(original), keys);
    assert_eq!(panes.tabs_in(peer), &[32]);
    assert_eq!(panes.selected_in(peer), Some(32));
    assert_eq!(panes.active_session(), Some(0));
    assert_eq!(panes.active_pane(), original);
    assert!(!panes.can_add());
    assert!(panes.select(32));
    assert!(panes.can_add());
    assert_keys(&panes, &(0..33).collect::<Vec<_>>())
}

#[test]
fn total_limit_rejects_a_new_split_peer_but_allows_moving_a_registered_peer() -> TestResult {
    let mut panes = full_workspace()?;
    assert_keys(&panes, &(0..128).collect::<Vec<_>>())?;
    assert!(panes.can_split(0));
    assert_eq!(
        rejected(&panes, |panes| panes.split(0, 128, SplitAxis::Horizontal))?,
        PaneSessionError::SessionLimit
    );
    let source = owner(&panes, 0)?;
    let peer = panes.split(0, 1, SplitAxis::Vertical)?;
    assert_eq!(panes.tabs_in(source).len(), 31);
    assert_eq!(panes.tabs_in(peer), &[1]);
    assert_eq!(panes.active_session(), Some(0));
    assert!(!panes.can_add());
    assert!(panes.select(1));
    assert!(!panes.can_add());
    assert!(panes.can_split(1));
    assert_eq!(
        rejected(&panes, |panes| panes.split(1, 128, SplitAxis::Vertical))?,
        PaneSessionError::SessionLimit
    );
    assert_keys(&panes, &(0..128).collect::<Vec<_>>())
}

#[test]
fn invalid_split_keys_are_atomic_errors() -> TestResult {
    let panes = nested()?;
    for (primary, peer, expected) in [
        (1, 1, PaneSessionError::SameSession),
        (99, 5, PaneSessionError::UnknownSession),
        (99, 1, PaneSessionError::UnknownSession),
        (1, 2, PaneSessionError::DifferentPanes),
    ] {
        assert_eq!(
            rejected(&panes, |panes| panes.split(
                primary,
                peer,
                SplitAxis::Horizontal
            ))?,
            expected
        );
    }
    assert!(!panes.can_split(99));
    Ok(())
}

#[test]
fn split_depth_preflight_is_local_to_primary_and_depth_failure_is_atomic() -> TestResult {
    let mut panes = populated(&[0], Some(0))?;
    for peer in 1..=8 {
        panes.split(0, peer, SplitAxis::Vertical)?;
    }
    assert_eq!(panes.layout().leaf_ids()?.len(), MAX_LAYOUT_DEPTH + 1);
    assert!(!panes.can_split(0));
    assert!(panes.can_split(1));
    assert!(matches!(
        rejected(&panes, |panes| panes.split(0, 9, SplitAxis::Horizontal))?,
        PaneSessionError::Layout(error) if error.field == "pane_layout.depth"
    ));
    panes.reconcile(&(0..10).collect::<Vec<_>>(), Some(0))?;
    assert!(matches!(
        rejected(&panes, |panes| panes.split(0, 9, SplitAxis::Horizontal))?,
        PaneSessionError::Layout(error) if error.field == "pane_layout.depth"
    ));
    panes.split(1, 10, SplitAxis::Horizontal)?;
    assert_eq!(panes.active_session(), Some(1));
    assert_keys(&panes, &(0..11).collect::<Vec<_>>())
}

#[test]
fn thirty_two_leaves_refuse_another_split_then_allow_one_after_removal() -> TestResult {
    let mut panes = populated(&[0], Some(0))?;
    let mut next = 1;
    for _ in 0..5 {
        for primary in panes.displayed_sessions() {
            panes.split(primary, next, SplitAxis::Horizontal)?;
            next += 1;
        }
    }
    assert_eq!(panes.layout().leaf_ids()?.len(), MAX_LAYOUT_PANES);
    assert!(!panes.can_split(0));
    assert!(panes.can_add());
    assert!(matches!(
        rejected(&panes, |panes| panes.split(0, 32, SplitAxis::Vertical))?,
        PaneSessionError::Layout(_)
    ));
    panes.remove(31)?;
    assert!(panes.can_split(0));
    panes.split(0, 32, SplitAxis::Vertical)?;
    assert_eq!(panes.layout().leaf_ids()?.len(), MAX_LAYOUT_PANES);
    let mut keys: Vec<_> = (0..31).collect();
    keys.push(32);
    assert_keys(&panes, &keys)
}

#[test]
fn ratio_setter_accepts_core_bounds_and_preserves_all_session_selections() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let root = panes.layout().root;
    let active = panes.active_session();
    let displayed = panes.displayed_sessions();
    for ratio in [1000, 9000, 5000] {
        panes.set_ratio(root, ratio)?;
        assert!(panes.layout().nodes.iter().any(|node| matches!(
            node,
            PaneNode::Split { id, ratio_bps, .. } if *id == root && *ratio_bps == ratio
        )));
        assert_eq!(panes.active_session(), active);
        assert_eq!(panes.displayed_sessions(), displayed);
    }
    for ratio in [0, 999, 9001, u16::MAX] {
        assert!(matches!(
            rejected(&panes, |panes| panes.set_ratio(root, ratio))?,
            PaneSessionError::Layout(_)
        ));
    }
    for target in [owner(&panes, 1)?, PaneNodeId(Uuid::nil())] {
        assert!(matches!(
            rejected(&panes, |panes| panes.set_ratio(target, 5000))?,
            PaneSessionError::Layout(_)
        ));
    }
    assert_keys(&panes, &[1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn removing_selected_tabs_prefers_the_successor_then_predecessor_then_none() -> TestResult {
    let mut panes = populated(&[10, 20, 30, 40], Some(20))?;
    let leaf = panes.active_pane();
    assert_eq!(panes.remove(20)?, Some(30));
    assert_eq!(panes.tabs_in(leaf), &[10, 30, 40]);
    assert_eq!(panes.remove(30)?, Some(40));
    assert_eq!(panes.remove(40)?, Some(10));
    assert_eq!(panes.remove(10)?, None);
    assert_eq!(panes.active_pane(), leaf);
    assert_eq!(panes.layout().leaf_ids()?, vec![leaf]);
    assert!(panes.displayed_sessions().is_empty());
    assert!(panes.can_add());
    assert_keys(&panes, &[])
}

#[test]
fn removing_an_unselected_tab_does_not_change_the_selected_key() -> TestResult {
    let mut panes = populated(&[10, 20, 30, 40], Some(30))?;
    let leaf = panes.active_pane();
    assert_eq!(panes.remove(10)?, Some(30));
    assert_eq!(panes.remove(40)?, Some(30));
    assert_eq!(panes.tabs_in(leaf), &[20, 30]);
    assert_eq!(panes.selected_in(leaf), Some(30));
    Ok(())
}

#[test]
fn closing_inactive_selections_and_then_their_leaf_preserves_the_active_key() -> TestResult {
    let mut panes = populated(&[1, 5, 6], Some(5))?;
    let left = panes.active_pane();
    let right = panes.split(1, 2, SplitAxis::Horizontal)?;
    assert!(panes.select(5));
    assert!(panes.select(2));
    assert_eq!(panes.remove(5)?, Some(2));
    assert_eq!(panes.selected_in(left), Some(6));
    assert_eq!(panes.remove(1)?, Some(2));
    assert_eq!(panes.selected_in(left), Some(6));
    assert_eq!(panes.remove(6)?, Some(2));
    assert_eq!(panes.layout().root, right);
    assert_eq!(panes.active_pane(), right);
    assert!(panes.tabs_in(left).is_empty());
    assert_eq!(panes.selected_in(left), None);
    assert_keys(&panes, &[2])
}

#[test]
fn closing_the_active_root_child_promotes_the_entire_sibling_subtree() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let closed = owner(&panes, 1)?;
    let old_root = panes.layout().root;
    let survivor = owner(&panes, 2)?;
    let promoted = parent_of(&panes, survivor)?;
    panes.remove(5)?;
    panes.remove(6)?;
    assert!(panes.select(1));
    panes.set_ratio(promoted, 3500)?;
    let surviving_nodes: Vec<_> = panes
        .layout()
        .nodes
        .iter()
        .filter(|node| node.id() != closed && node.id() != old_root)
        .cloned()
        .collect();
    assert_eq!(panes.remove(1)?, Some(7));
    assert_eq!(panes.layout().root, promoted);
    assert_eq!(panes.layout().nodes, surviving_nodes);
    assert_eq!(panes.active_pane(), survivor);
    assert_eq!(panes.displayed_sessions(), vec![7, 3, 4]);
    assert_keys(&panes, &[2, 3, 4, 7, 8])
}

#[test]
fn closing_a_deep_active_leaf_selects_its_sibling_and_preserves_other_branches() -> TestResult {
    let mut panes = nested()?;
    let root = panes.layout().root;
    let second = owner(&panes, 2)?;
    let fourth = owner(&panes, 4)?;
    assert!(panes.select(3));
    assert_eq!(panes.remove(3)?, Some(4));
    assert_eq!(panes.active_pane(), fourth);
    assert_eq!(panes.layout().root, root);
    assert_eq!(panes.pane_for(2), Some(second));
    assert_eq!(panes.displayed_sessions(), vec![1, 2, 4]);
    assert_keys(&panes, &[1, 2, 4])
}

#[test]
fn collapsing_an_active_leaf_appends_all_tabs_and_keeps_its_active_key() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let closed = owner(&panes, 1)?;
    let old_root = panes.layout().root;
    let destination = owner(&panes, 2)?;
    let promoted = parent_of(&panes, destination)?;
    let nested_split = parent_of(&panes, owner(&panes, 3)?)?;
    panes.set_ratio(promoted, 3500)?;
    panes.set_ratio(nested_split, 7000)?;
    assert!(panes.select(5));
    let surviving_nodes: Vec<_> = panes
        .layout()
        .nodes
        .iter()
        .filter(|node| node.id() != closed && node.id() != old_root)
        .cloned()
        .collect();
    panes.collapse(closed)?;
    assert_eq!(panes.layout().root, promoted);
    assert_eq!(panes.layout().nodes, surviving_nodes);
    assert_eq!(panes.tabs_in(destination), &[2, 7, 8, 1, 5, 6]);
    assert_eq!(panes.active_session(), Some(5));
    assert_eq!(panes.active_pane(), destination);
    assert_eq!(panes.selected_in(destination), Some(5));
    assert_eq!(panes.displayed_sessions(), vec![5, 3, 4]);
    assert_keys(&panes, &[1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn collapsing_an_inactive_leaf_targets_its_sibling_not_the_active_leaf() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let closed = owner(&panes, 1)?;
    let destination = owner(&panes, 2)?;
    let active = owner(&panes, 4)?;
    assert!(panes.select(4));
    panes.collapse(closed)?;
    assert_eq!(panes.tabs_in(destination), &[2, 7, 8, 1, 5, 6]);
    assert_eq!(panes.selected_in(destination), Some(7));
    assert_eq!(panes.tabs_in(active), &[4]);
    assert_eq!(panes.active_pane(), active);
    assert_eq!(panes.active_session(), Some(4));
    assert_eq!(panes.displayed_sessions(), vec![7, 3, 4]);
    assert_keys(&panes, &[1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn collapsing_a_second_child_uses_the_first_leaf_of_the_first_sibling_subtree() -> TestResult {
    let mut panes = populated(&[1], Some(1))?;
    let first = panes.active_pane();
    let closed = panes.split(1, 2, SplitAxis::Horizontal)?;
    panes.split(1, 3, SplitAxis::Vertical)?;
    let promoted = parent_of(&panes, first)?;
    panes.split(1, 4, SplitAxis::Horizontal)?;
    assert!(panes.select(2));
    panes.collapse(closed)?;
    assert_eq!(panes.layout().root, promoted);
    assert_eq!(panes.tabs_in(first), &[1, 2]);
    assert_eq!(panes.active_session(), Some(2));
    assert_eq!(panes.active_pane(), first);
    assert_eq!(panes.displayed_sessions(), vec![2, 4, 3]);
    assert_keys(&panes, &[1, 2, 3, 4])
}

#[test]
fn collapse_allows_exactly_thirty_two_destination_tabs() -> TestResult {
    let mut panes = populated(&(0..32).collect::<Vec<_>>(), Some(0))?;
    let source = panes.active_pane();
    let destination = panes.split(0, 31, SplitAxis::Horizontal)?;
    panes.collapse(source)?;
    let mut expected = vec![31];
    expected.extend(0..31);
    assert_eq!(panes.tabs_in(destination), expected);
    assert_eq!(panes.layout().root, destination);
    assert_eq!(panes.active_session(), Some(0));
    assert!(!panes.can_add());
    assert_keys(&panes, &(0..32).collect::<Vec<_>>())
}

#[test]
fn collapse_overflow_preserves_geometry_tabs_and_selections_for_any_active_leaf() -> TestResult {
    let mut panes = full_workspace()?;
    let source = owner(&panes, 32)?;
    for active in [0, 32, 64] {
        assert!(panes.select(active));
        assert_eq!(
            rejected(&panes, |panes| panes.collapse(source))?,
            PaneSessionError::PaneTabLimit
        );
    }
    Ok(())
}

#[test]
fn collapse_rejects_the_last_leaf_and_unknown_or_split_targets() -> TestResult {
    for keys in [&[][..], &[1][..]] {
        let panes = populated(keys, keys.first().copied())?;
        let leaf = panes.active_pane();
        assert!(matches!(
            rejected(&panes, |panes| panes.collapse(leaf))?,
            PaneSessionError::Layout(_)
        ));
    }
    let panes = nested()?;
    for target in [PaneNodeId(Uuid::nil()), panes.layout().root] {
        assert_eq!(
            rejected(&panes, |panes| panes.collapse(target))?,
            PaneSessionError::UnknownPane
        );
    }
    Ok(())
}

#[test]
fn reconciliation_rejects_duplicates_and_unknown_active_keys_without_mutating() -> TestResult {
    let panes = nested_with_tabs()?;
    for keys in [&[1, 2, 1][..], &[9, 9][..]] {
        assert_eq!(
            rejected(&panes, |panes| panes.reconcile(keys, None))?,
            PaneSessionError::DuplicateSession
        );
    }
    for (keys, active) in [(&[1, 2][..], 3), (&[][..], 1), (&[9][..], 8)] {
        assert_eq!(
            rejected(&panes, |panes| panes.reconcile(keys, Some(active)))?,
            PaneSessionError::UnknownActive
        );
    }
    Ok(())
}

#[test]
fn reconciliation_limits_are_atomic_even_after_candidate_removal_and_collapse() -> TestResult {
    let mut panes = populated(&(0..32).collect::<Vec<_>>(), Some(0))?;
    panes.split(0, 32, SplitAxis::Horizontal)?;
    let mut replacement: Vec<_> = (0..32).collect();
    replacement.push(33);
    // The candidate closes peer 32 before discovering source 0 cannot hold 33.
    assert_eq!(
        rejected(&panes, |panes| panes.reconcile(&replacement, Some(33)))?,
        PaneSessionError::PaneTabLimit
    );
    assert_eq!(
        rejected(&panes, |panes| {
            panes.reconcile(&(100..133).collect::<Vec<_>>(), None)
        })?,
        PaneSessionError::PaneTabLimit
    );
    assert_eq!(
        rejected(&panes, |panes| {
            panes.reconcile(&(0..129).collect::<Vec<_>>(), Some(0))
        })?,
        PaneSessionError::SessionLimit
    );
    Ok(())
}

#[test]
fn reconciliation_of_multiple_removals_uses_the_next_surviving_tab() -> TestResult {
    let mut panes = populated(&[1, 2, 3, 4, 5, 6], Some(3))?;
    let leaf = panes.active_pane();
    panes.reconcile(&[6, 2, 4], None)?;
    assert_eq!(panes.tabs_in(leaf), &[2, 4, 6]);
    assert_eq!(panes.active_session(), Some(4));
    panes.reconcile(&[2], None)?;
    assert_eq!(panes.active_session(), Some(2));
    assert_keys(&panes, &[2])
}

#[test]
fn reconciliation_collapses_empty_leaves_before_adding_to_the_fallback_active_leaf() -> TestResult {
    let mut panes = nested()?;
    let second = owner(&panes, 2)?;
    let fourth = owner(&panes, 4)?;
    assert!(panes.select(1));
    panes.reconcile(&[2, 4, 5], Some(4))?;
    assert_eq!(panes.layout().leaf_ids()?, vec![second, fourth]);
    assert_eq!(panes.tabs_in(second), &[2, 5]);
    assert_eq!(panes.selected_in(second), Some(2));
    assert_eq!(panes.tabs_in(fourth), &[4]);
    assert_eq!(panes.active_pane(), fourth);
    assert_eq!(panes.active_session(), Some(4));
    assert_keys(&panes, &[2, 4, 5])
}

#[test]
fn reconciling_all_sessions_away_keeps_a_surviving_leaf_for_later_additions() -> TestResult {
    let mut panes = nested_with_tabs()?;
    let previous_leaves = panes.layout().leaf_ids()?;
    panes.reconcile(&[], None)?;
    let survivor = panes.active_pane();
    assert!(previous_leaves.contains(&survivor));
    assert_eq!(panes.layout().leaf_ids()?, vec![survivor]);
    assert_eq!(panes.layout().root, survivor);
    assert_eq!(panes.active_session(), None);
    assert!(panes.displayed_sessions().is_empty());
    assert!(panes.can_add());
    assert_keys(&panes, &[])?;
    panes.reconcile(&[50, 60], Some(60))?;
    assert_eq!(panes.active_pane(), survivor);
    assert_eq!(panes.tabs_in(survivor), &[50, 60]);
    assert_eq!(panes.active_session(), Some(60));
    assert_keys(&panes, &[50, 60])
}

#[test]
fn unknown_keys_and_nonleaf_lookups_do_not_change_runtime_state() -> TestResult {
    let mut panes = nested()?;
    let before = panes.clone();
    for target in [panes.layout().root, PaneNodeId(Uuid::nil())] {
        assert!(panes.tabs_in(target).is_empty());
        assert_eq!(panes.selected_in(target), None);
    }
    assert_eq!(panes.pane_for(99), None);
    assert!(!panes.select(99));
    assert_eq!(panes, before);
    assert_eq!(
        rejected(&panes, |panes| panes.remove(99))?,
        PaneSessionError::UnknownSession
    );
    Ok(())
}

#[test]
fn runtime_keys_need_neither_debug_nor_default() -> TestResult {
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    struct Key(u32);

    let mut panes = SessionPanes::new();
    panes.reconcile(&[Key(1), Key(2)], Some(Key(2)))?;
    let peer = panes.split(Key(1), Key(3), SplitAxis::Horizontal)?;
    assert!(panes.tabs_in(peer) == [Key(3)]);
    assert!(panes.active_session() == Some(Key(1)));
    assert!(panes == panes.clone());
    assert!(panes.select(Key(3)));
    panes.collapse(peer)?;
    assert!(panes.active_session() == Some(Key(3)));
    Ok(())
}
