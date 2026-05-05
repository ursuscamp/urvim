//! Layout tree node types and split metadata.

use crate::window_group::WindowGroup;

/// Stable identifier for a pane in the layout split tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub usize);

/// Orientation of a binary split node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitAxis {
    /// Child panes are stacked vertically and divide rows.
    Horizontal,
    /// Child panes sit side-by-side and divide columns.
    Vertical,
}

/// Relative size weights for the two children of a split node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitSize {
    first_weight: u16,
    second_weight: u16,
}

impl SplitSize {
    /// Creates a split-size ratio from integer child weights.
    pub fn new(first_weight: u16, second_weight: u16) -> Self {
        Self {
            first_weight: first_weight.max(1),
            second_weight: second_weight.max(1),
        }
    }

    /// Returns an even `1:1` split ratio.
    pub fn even() -> Self {
        Self::new(1, 1)
    }

    /// Returns the first child's stored weight.
    pub fn first_weight(&self) -> u16 {
        self.first_weight
    }

    /// Returns the second child's stored weight.
    pub fn second_weight(&self) -> u16 {
        self.second_weight
    }
}

/// Leaf node that owns one pane-hosted window group.
#[derive(Debug)]
pub struct PaneNode {
    pub(super) id: PaneId,
    pub(super) window_group: WindowGroup,
}

impl PaneNode {
    pub(super) fn new(id: PaneId, window_group: WindowGroup) -> Self {
        Self { id, window_group }
    }
}

/// Internal binary split node that divides space between two children.
#[derive(Debug)]
pub struct SplitNode {
    pub(super) axis: SplitAxis,
    pub(super) first: Box<LayoutNode>,
    pub(super) second: Box<LayoutNode>,
    pub(super) split_size: SplitSize,
    pub(super) last_focused_pane: PaneId,
}

impl SplitNode {
    pub(super) fn new(
        axis: SplitAxis,
        first: LayoutNode,
        second: LayoutNode,
        last_focused_pane: PaneId,
    ) -> Self {
        Self {
            axis,
            first: Box::new(first),
            second: Box::new(second),
            split_size: SplitSize::even(),
            last_focused_pane,
        }
    }
}

/// Recursive layout tree node.
#[derive(Debug)]
pub enum LayoutNode {
    /// A visible editor pane that owns a window group.
    Pane(PaneNode),
    /// A binary split node that owns two child layout nodes.
    Split(SplitNode),
}
