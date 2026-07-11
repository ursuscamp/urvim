//! Layout tree node types and split metadata.

use crate::ui::plugin_pane::{PluginPane, PluginPaneOptions};
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

/// Leaf node that owns one pane-hosted editor or plugin UI.
#[derive(Debug)]
pub struct PaneNode {
    pub(super) id: PaneId,
    pub(super) content: PaneContent,
}

impl PaneNode {
    pub(super) fn new_editor(id: PaneId, window_group: WindowGroup) -> Self {
        Self {
            id,
            content: PaneContent::Editor(window_group),
        }
    }

    pub(super) fn new_plugin(id: PaneId, owner: String, options: PluginPaneOptions) -> Self {
        Self {
            id,
            content: PaneContent::Plugin(PluginPane::new(owner, options)),
        }
    }

    pub(super) fn is_plugin(&self) -> bool {
        matches!(self.content, PaneContent::Plugin(_))
    }

    pub(super) fn editor_window_group(&self) -> Option<&WindowGroup> {
        match &self.content {
            PaneContent::Editor(window_group) => Some(window_group),
            PaneContent::Plugin(_) => None,
        }
    }

    pub(super) fn editor_window_group_mut(&mut self) -> Option<&mut WindowGroup> {
        match &mut self.content {
            PaneContent::Editor(window_group) => Some(window_group),
            PaneContent::Plugin(_) => None,
        }
    }

    pub(super) fn plugin_pane(&self) -> Option<&PluginPane> {
        match &self.content {
            PaneContent::Editor(_) => None,
            PaneContent::Plugin(pane) => Some(pane),
        }
    }

    pub(super) fn plugin_pane_mut(&mut self) -> Option<&mut PluginPane> {
        match &mut self.content {
            PaneContent::Editor(_) => None,
            PaneContent::Plugin(pane) => Some(pane),
        }
    }
}

/// Content hosted by a layout pane.
#[derive(Debug)]
pub enum PaneContent {
    /// A traditional editor window group.
    Editor(WindowGroup),
    /// A retained plugin-owned UI window.
    Plugin(PluginPane),
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
    /// A visible pane that owns editor or plugin content.
    Pane(PaneNode),
    /// A binary split node that owns two child layout nodes.
    Split(SplitNode),
}
