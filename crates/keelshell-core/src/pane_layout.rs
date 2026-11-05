//! Bounded pane topology, independent of rendering, sessions and persistence.
//!
//! Nodes are stored flat; their vector order has no geometric meaning. A valid
//! layout is one rooted binary tree with unique, non-nil identities and an active
//! leaf. Traversal is iterative, including validation of untrusted graphs.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ValidationError;

/// Maximum number of leaf panes in one layout.
pub const MAX_LAYOUT_PANES: usize = 32;
/// Maximum node depth, inclusive; the root has depth zero.
pub const MAX_LAYOUT_DEPTH: usize = 8;
/// Minimum share of a split assigned to its first child, in basis points.
pub const MIN_SPLIT_RATIO_BPS: u16 = 1000;
/// Maximum share of a split assigned to its first child, in basis points.
pub const MAX_SPLIT_RATIO_BPS: u16 = 9000;

const MAX_LAYOUT_NODES: usize = MAX_LAYOUT_PANES * 2 - 1;

/// Stable identity shared by leaf panes and split nodes.
///
/// Serializes transparently as a UUID. Construction and deserialization permit
/// nil UUIDs, but [`PaneLayout::validate`] rejects them in every identity position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneNodeId(
    /// UUID supplied by the caller; it must be non-nil and unique within a layout.
    pub Uuid,
);

/// Direction in which a split places its ordered children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    /// Place `first` on the left and `second` on the right.
    Horizontal,
    /// Place `first` above `second`.
    Vertical,
}

// A derived unit enum also accepts externally tagged objects. The axis wire
// contract is a string, so reject those objects and any fields they might carry.
impl<'de> Deserialize<'de> for SplitAxis {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AxisVisitor;

        impl serde::de::Visitor<'_> for AxisVisitor {
            type Value = SplitAxis;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("the string \"horizontal\" or \"vertical\"")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "horizontal" => Ok(SplitAxis::Horizontal),
                    "vertical" => Ok(SplitAxis::Vertical),
                    _ => Err(E::custom("axis must be \"horizontal\" or \"vertical\"")),
                }
            }
        }

        deserializer.deserialize_str(AxisVisitor)
    }
}

/// A leaf pane or a binary split referencing other flat nodes.
///
/// Both wire variants are structs so unknown fields are rejected even on leaves.
/// Child identities must be distinct; each non-root node has exactly one parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaneNode {
    /// A pane with no children or associated runtime data.
    Leaf {
        /// Unique, non-nil identity of this pane.
        id: PaneNodeId,
    },
    /// Two ordered children partitioning their parent's area.
    Split {
        /// Unique, non-nil identity of this split.
        id: PaneNodeId,
        /// Left/right or top/bottom placement of the children.
        axis: SplitAxis,
        /// First child's share out of 10,000, within the inclusive ratio bounds.
        ratio_bps: u16,
        /// Left child for a horizontal split; top child for a vertical split.
        first: PaneNodeId,
        /// Right child for a horizontal split; bottom child for a vertical split.
        second: PaneNodeId,
    },
}

impl PaneNode {
    /// Return this node's identity without following any references.
    pub const fn id(&self) -> PaneNodeId {
        match self {
            Self::Leaf { id } | Self::Split { id, .. } => *id,
        }
    }
}

/// A flat, ordered binary pane tree with one active leaf.
///
/// Valid layouts have 1–32 leaves, at most 63 total nodes and depth at most eight
/// (root depth zero). Every node is reachable exactly once from `root`; no child
/// is shared, missing or cyclic. Split ratios allocate 10–90% to `first`.
///
/// Serde checks the wire shape and rejects unknown fields. Call [`Self::validate`]
/// after deserialization or direct field edits to check the graph invariants.
/// Every mutation method also validates its input and leaves the entire layout,
/// including vector order, unchanged on error.
///
/// # Examples
///
/// ```
/// use keelshell_core::{PaneLayout, PaneNodeId, SplitAxis};
/// use uuid::Uuid;
///
/// let left = PaneNodeId(Uuid::from_u128(1));
/// let right = PaneNodeId(Uuid::from_u128(2));
/// let branch = PaneNodeId(Uuid::from_u128(3));
/// let mut layout = PaneLayout::single(left)?;
/// layout.split(left, right, branch, SplitAxis::Horizontal, 6000)?;
/// assert_eq!(layout.leaf_ids()?, vec![left, right]);
/// assert_eq!(layout.active, right);
/// layout.close(right)?;
/// assert_eq!(layout, PaneLayout::single(left)?);
/// # Ok::<(), keelshell_core::ValidationError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneLayout {
    /// Identity of the only node without a parent.
    pub root: PaneNodeId,
    /// Identity of the selected leaf; split nodes cannot be active.
    pub active: PaneNodeId,
    /// All nodes in arbitrary storage order, with unique, non-nil identities.
    pub nodes: Vec<PaneNode>,
}

impl PaneLayout {
    /// Create one active leaf using a caller-supplied identity.
    ///
    /// # Errors
    ///
    /// Rejects a nil identity.
    pub fn single(id: PaneNodeId) -> Result<Self, ValidationError> {
        validate_id(id, "pane_layout.nodes")?;
        Ok(Self {
            root: id,
            active: id,
            nodes: vec![PaneNode::Leaf { id }],
        })
    }

    /// Check all identity, tree, selection, ratio, size and depth invariants.
    ///
    /// Traversal uses an explicit stack and checks the node count before building
    /// indexes, so even malformed graphs cannot cause recursive traversal.
    ///
    /// # Errors
    ///
    /// Rejects empty or oversized layouts, nil or duplicate identities, missing
    /// references, repeated children, cycles, shared children, multiple parents,
    /// orphans, invalid ratios, excessive depth and an active node that is not a
    /// reachable leaf.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validated_leaf_ids().map(|_| ())
    }

    /// Return leaf identities in screen order, visiting `first` before `second`.
    ///
    /// This is the leaf subsequence of preorder traversal, independent of node
    /// storage order: left before right, and top before bottom.
    ///
    /// # Errors
    ///
    /// Rejects any invalid layout, using the same checks as [`Self::validate`].
    pub fn leaf_ids(&self) -> Result<Vec<PaneNodeId>, ValidationError> {
        self.validated_leaf_ids()
    }

    /// Split a leaf, preserving it as `first` and adding `new_pane` as `second`.
    ///
    /// Replaces the leaf's parent reference (or the root) with `split_node`.
    /// The original leaf retains its identity and the new leaf becomes active.
    /// Both new identities must be non-nil, distinct and absent from the layout.
    ///
    /// # Errors
    ///
    /// Rejects an invalid input layout, a non-leaf target, invalid or colliding
    /// identities, an out-of-range ratio, or a result exceeding size/depth bounds.
    /// On error the entire original layout is preserved.
    pub fn split(
        &mut self,
        pane: PaneNodeId,
        new_pane: PaneNodeId,
        split_node: PaneNodeId,
        axis: SplitAxis,
        ratio_bps: u16,
    ) -> Result<(), ValidationError> {
        self.validate()?;
        self.require_leaf(pane)?;
        validate_id(new_pane, "pane_layout.nodes")?;
        validate_id(split_node, "pane_layout.nodes")?;
        if new_pane == split_node
            || self
                .nodes
                .iter()
                .any(|node| node.id() == new_pane || node.id() == split_node)
        {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "new identities must be distinct and unused",
            ));
        }
        validate_ratio(ratio_bps)?;

        // The validated input bounds this copy. Publish only after checking the
        // complete candidate, including depth changes below the replaced leaf.
        let mut candidate = self.clone();
        candidate.replace_reference(pane, split_node)?;
        candidate.nodes.push(PaneNode::Split {
            id: split_node,
            axis,
            ratio_bps,
            first: pane,
            second: new_pane,
        });
        candidate.nodes.push(PaneNode::Leaf { id: new_pane });
        candidate.active = new_pane;
        self.replace_validated(candidate)
    }

    /// Close a leaf and remove its direct parent, promoting the sibling subtree.
    ///
    /// All surviving nodes retain their identities, axes, ratios and child order.
    /// If the closed pane was active, select the first leaf of its sibling
    /// subtree in screen order; otherwise preserve the current active pane.
    ///
    /// # Errors
    ///
    /// Rejects an invalid input layout, a missing/non-leaf target or the final
    /// remaining leaf. On error the entire original layout is preserved.
    pub fn close(&mut self, pane: PaneNodeId) -> Result<(), ValidationError> {
        self.validate()?;
        self.require_leaf(pane)?;
        if self.root == pane {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "the last pane cannot be closed",
            ));
        }
        let (parent, sibling) = self
            .nodes
            .iter()
            .find_map(|node| match node {
                PaneNode::Split {
                    id, first, second, ..
                } if *first == pane => Some((*id, *second)),
                PaneNode::Split {
                    id, first, second, ..
                } if *second == pane => Some((*id, *first)),
                _ => None,
            })
            .ok_or_else(|| ValidationError::new("pane_layout.nodes", "pane must have a parent"))?;

        let mut candidate = self.clone();
        candidate.replace_reference(parent, sibling)?;
        candidate
            .nodes
            .retain(|node| node.id() != pane && node.id() != parent);
        if self.active == pane {
            candidate.active = self.first_leaf(sibling)?;
        }
        self.replace_validated(candidate)
    }

    /// Set the first child's share of an existing split, in basis points.
    ///
    /// # Errors
    ///
    /// Rejects an invalid input layout, a missing/non-split target or a ratio
    /// outside the inclusive bounds. On error the entire layout is preserved.
    pub fn set_ratio(
        &mut self,
        split_node: PaneNodeId,
        ratio_bps: u16,
    ) -> Result<(), ValidationError> {
        self.validate()?;
        validate_ratio(ratio_bps)?;
        let Some(PaneNode::Split {
            ratio_bps: current, ..
        }) = self.nodes.iter_mut().find(|node| node.id() == split_node)
        else {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "ratio target must be an existing split",
            ));
        };
        *current = ratio_bps;
        Ok(())
    }

    /// Select an existing leaf without altering the topology.
    ///
    /// # Errors
    ///
    /// Rejects an invalid input layout or a missing/non-leaf target. On error
    /// the entire original layout is preserved.
    pub fn set_active(&mut self, pane: PaneNodeId) -> Result<(), ValidationError> {
        self.validate()?;
        self.require_leaf(pane)?;
        self.active = pane;
        Ok(())
    }

    fn validated_leaf_ids(&self) -> Result<Vec<PaneNodeId>, ValidationError> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_LAYOUT_NODES {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "must contain between 1 and 63 nodes",
            ));
        }
        validate_id(self.root, "pane_layout.root")?;
        validate_id(self.active, "pane_layout.active")?;
        let mut by_id = HashMap::with_capacity(self.nodes.len());
        let mut leaf_count = 0;
        for node in &self.nodes {
            validate_id(node.id(), "pane_layout.nodes")?;
            if by_id.insert(node.id(), node).is_some() {
                return Err(ValidationError::new(
                    "pane_layout.nodes",
                    "node identities must be unique",
                ));
            }
            match node {
                PaneNode::Leaf { .. } => leaf_count += 1,
                PaneNode::Split {
                    ratio_bps,
                    first,
                    second,
                    ..
                } => {
                    validate_ratio(*ratio_bps)?;
                    validate_id(*first, "pane_layout.nodes")?;
                    validate_id(*second, "pane_layout.nodes")?;
                    if first == second {
                        return Err(ValidationError::new(
                            "pane_layout.nodes",
                            "split children must be distinct",
                        ));
                    }
                }
            }
        }
        if leaf_count > MAX_LAYOUT_PANES {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "must contain at most 32 leaf panes",
            ));
        }
        if !by_id.contains_key(&self.root) {
            return Err(ValidationError::new(
                "pane_layout.root",
                "must identify an existing node",
            ));
        }
        if !matches!(by_id.get(&self.active), Some(PaneNode::Leaf { .. })) {
            return Err(ValidationError::new(
                "pane_layout.active",
                "must identify an existing leaf",
            ));
        }
        let mut visited = HashSet::with_capacity(self.nodes.len());
        let mut leaves = Vec::with_capacity(leaf_count);
        let mut pending = vec![(self.root, 0)];
        while let Some((id, depth)) = pending.pop() {
            if !visited.insert(id) {
                return Err(ValidationError::new(
                    "pane_layout.nodes",
                    "must not contain cycles or shared children",
                ));
            }
            if depth > MAX_LAYOUT_DEPTH {
                return Err(ValidationError::new(
                    "pane_layout.depth",
                    "node depth must not exceed 8 with root at depth 0",
                ));
            }
            match by_id.get(&id) {
                Some(PaneNode::Leaf { .. }) => leaves.push(id),
                Some(PaneNode::Split { first, second, .. }) => {
                    pending.push((*second, depth + 1));
                    pending.push((*first, depth + 1));
                }
                None => {
                    return Err(ValidationError::new(
                        "pane_layout.nodes",
                        "every child must identify an existing node",
                    ));
                }
            }
        }
        if visited.len() != self.nodes.len() {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "every node must be reachable from the root",
            ));
        }
        Ok(leaves)
    }

    fn require_leaf(&self, id: PaneNodeId) -> Result<(), ValidationError> {
        if !matches!(
            self.nodes.iter().find(|node| node.id() == id),
            Some(PaneNode::Leaf { .. })
        ) {
            return Err(ValidationError::new(
                "pane_layout.nodes",
                "pane must identify an existing leaf",
            ));
        }
        Ok(())
    }

    fn first_leaf(&self, mut id: PaneNodeId) -> Result<PaneNodeId, ValidationError> {
        for _ in 0..=MAX_LAYOUT_DEPTH {
            match self.nodes.iter().find(|node| node.id() == id) {
                Some(PaneNode::Leaf { .. }) => return Ok(id),
                Some(PaneNode::Split { first, .. }) => id = *first,
                None => break,
            }
        }
        Err(ValidationError::new(
            "pane_layout.nodes",
            "subtree must contain a reachable leaf within the depth bound",
        ))
    }

    fn replace_reference(
        &mut self,
        old: PaneNodeId,
        replacement: PaneNodeId,
    ) -> Result<(), ValidationError> {
        if self.root == old {
            self.root = replacement;
            return Ok(());
        }
        for node in &mut self.nodes {
            if let PaneNode::Split { first, second, .. } = node {
                if *first == old {
                    *first = replacement;
                    return Ok(());
                }
                if *second == old {
                    *second = replacement;
                    return Ok(());
                }
            }
        }
        Err(ValidationError::new(
            "pane_layout.nodes",
            "non-root node must have a parent",
        ))
    }

    fn replace_validated(&mut self, candidate: Self) -> Result<(), ValidationError> {
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

fn validate_id(id: PaneNodeId, field: &'static str) -> Result<(), ValidationError> {
    if id.0.is_nil() {
        return Err(ValidationError::new(field, "identity must not be nil"));
    }
    Ok(())
}

fn validate_ratio(ratio_bps: u16) -> Result<(), ValidationError> {
    if !(MIN_SPLIT_RATIO_BPS..=MAX_SPLIT_RATIO_BPS).contains(&ratio_bps) {
        return Err(ValidationError::new(
            "pane_layout.ratio_bps",
            "must be between 1000 and 9000 basis points",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
