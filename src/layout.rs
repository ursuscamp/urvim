//! Layout module.
//!
//! This module provides the `Layout` root container, which owns a binary split
//! tree of pane-hosted tab groups, routes split-management actions, and renders
//! a footer status bar below the active editor region.

use crate::action::ActionResult;
use crate::editor::{Action, ActionKind, ModeKind};
use crate::screen::Screen;
use crate::status_bar::{StatusBar, StatusBarContext};
use crate::tab_group::TabGroup;
use crate::terminal::CursorStyle;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;

/// Stable identifier for a pane in the layout split tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub usize);

/// Orientation of a binary split node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Leaf node that owns one pane-hosted tab group.
#[derive(Debug)]
pub struct PaneNode {
    id: PaneId,
    tab_group: TabGroup,
}

impl PaneNode {
    fn new(id: PaneId, tab_group: TabGroup) -> Self {
        Self { id, tab_group }
    }
}

/// Internal binary split node that divides space between two children.
#[derive(Debug)]
pub struct SplitNode {
    axis: SplitAxis,
    first: Box<LayoutNode>,
    second: Box<LayoutNode>,
    split_size: SplitSize,
}

impl SplitNode {
    fn new(axis: SplitAxis, first: LayoutNode, second: LayoutNode) -> Self {
        Self {
            axis,
            first: Box::new(first),
            second: Box::new(second),
            split_size: SplitSize::even(),
        }
    }
}

/// Recursive layout tree node.
#[derive(Debug)]
pub enum LayoutNode {
    /// A visible editor pane that owns a tab group.
    Pane(PaneNode),
    /// A binary split node that owns two child layout nodes.
    Split(SplitNode),
}

#[derive(Debug, Clone, Copy)]
struct PaneRegion {
    id: PaneId,
    origin: Position,
    size: Size,
}

impl PaneRegion {
    fn left(self) -> u16 {
        self.origin.col
    }

    fn right(self) -> u16 {
        self.origin.col.saturating_add(self.size.cols)
    }

    fn top(self) -> u16 {
        self.origin.row
    }

    fn bottom(self) -> u16 {
        self.origin.row.saturating_add(self.size.rows)
    }

    fn vertical_overlap(self, other: Self) -> u16 {
        self.bottom()
            .min(other.bottom())
            .saturating_sub(self.top().max(other.top()))
    }

    fn horizontal_overlap(self, other: Self) -> u16 {
        self.right()
            .min(other.right())
            .saturating_sub(self.left().max(other.left()))
    }
}

#[derive(Debug, Clone, Copy)]
enum FocusDirection {
    Left,
    Down,
    Up,
    Right,
}

/// Root layout container for urvim.
///
/// The layout owns a binary split tree of panes, tracks the focused pane,
/// routes split-management actions, and renders a footer status bar beneath
/// the editor content area.
#[derive(Debug)]
pub struct Layout {
    root: Option<LayoutNode>,
    focused_pane: PaneId,
    next_pane_id: usize,
    status_bar: StatusBar,
    origin: Position,
    size: Size,
}

impl Layout {
    /// Creates a layout from an existing tab group.
    pub fn new(tab_group: TabGroup) -> Self {
        let focused_pane = PaneId(0);
        Self {
            root: Some(LayoutNode::Pane(PaneNode::new(focused_pane, tab_group))),
            focused_pane,
            next_pane_id: 1,
            status_bar: StatusBar::new(),
            origin: Position::default(),
            size: Size::default(),
        }
    }

    /// Creates a layout from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        Self::new(TabGroup::from_paths(paths))
    }

    /// Returns true when the layout has no panes left to render.
    pub fn should_exit(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the active tab group for the focused pane.
    pub fn active_tab_group(&self) -> &TabGroup {
        let root = self
            .root
            .as_ref()
            .expect("layout should contain a focused pane");
        Self::find_pane(root, self.focused_pane)
            .map(|pane| &pane.tab_group)
            .expect("focused pane should exist")
    }

    /// Returns the active tab group mutably for the focused pane.
    pub fn active_tab_group_mut(&mut self) -> &mut TabGroup {
        let focused_pane = self.focused_pane;
        let root = self
            .root
            .as_mut()
            .expect("layout should contain a focused pane");
        Self::find_pane_mut(root, focused_pane)
            .map(|pane| &mut pane.tab_group)
            .expect("focused pane should exist")
    }

    /// Returns the active tab group.
    pub fn tab_group(&self) -> &TabGroup {
        self.active_tab_group()
    }

    /// Returns the active tab group mutably.
    pub fn tab_group_mut(&mut self) -> &mut TabGroup {
        self.active_tab_group_mut()
    }

    /// Returns the current layout mode label.
    pub fn mode_label(&self) -> &'static str {
        self.active_tab_group().active_window_mode_label()
    }

    /// Returns the current mode kind of the focused pane's active window.
    pub fn active_window_mode_kind(&self) -> ModeKind {
        self.active_tab_group().active_window_mode_kind()
    }

    /// Returns the cursor style of the focused pane's active window.
    pub fn active_window_cursor_style(&self) -> CursorStyle {
        self.active_tab_group().active_window_cursor_style()
    }

    /// Returns the last rendered layout origin.
    pub fn origin(&self) -> Position {
        self.origin
    }

    /// Returns the last rendered layout size.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Returns the active buffer view from the focused pane.
    pub fn active_buffer_view(&self) -> &BufferView {
        self.active_tab_group().active_buffer_view()
    }

    /// Returns the active buffer view mutably from the focused pane.
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView {
        self.active_tab_group_mut().active_buffer_view_mut()
    }

    /// Returns and clears any repeat-text suffix produced by the active child window.
    pub fn take_pending_repeat_suffix(&mut self) -> Option<String> {
        if self.should_exit() {
            return None;
        }

        self.active_tab_group_mut().take_pending_repeat_suffix()
    }

    /// Returns the visual cursor for the focused pane, if any.
    pub fn visual_cursor(&self) -> Option<Position> {
        let pane_region = self.pane_region(self.focused_pane)?;
        let mut pos = self.active_tab_group().visual_cursor()?;
        pos.row = pos.row.saturating_add(pane_region.origin.row);
        pos.col = pos.col.saturating_add(pane_region.origin.col);
        Some(pos)
    }

    /// Renders the layout tree and footer status bar.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.prune_empty_panes();
        self.origin = origin;
        self.size = size;

        if size.rows == 0 {
            return;
        }

        let content_rows = size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, size.cols);

        if let Some(root) = self.root.as_mut() {
            Self::render_node(root, screen, origin, content_size);
        }

        if self.should_exit() {
            return;
        }

        let buffer_view = self.active_buffer_view();
        let buffer_name = buffer_view
            .file_name()
            .unwrap_or_else(|| "Untitled".to_string());
        let syntax_name = buffer_view.syntax_name();
        let syntax_label = buffer_view.syntax_label();
        let cursor = buffer_view.cursor();
        let context = StatusBarContext {
            mode_label: self.mode_label(),
            modified: buffer_view.is_modified(),
            syntax_name: syntax_name.as_str(),
            syntax_label: syntax_label.as_str(),
            buffer_name: buffer_name.as_str(),
            cursor_line: cursor.line,
            cursor_byte_col: cursor.col,
            line_count: buffer_view.line_count(),
        };

        let footer_origin = Position::new(origin.row.saturating_add(content_rows), origin.col);
        self.status_bar
            .render(screen, footer_origin, Size::new(1, size.cols), &context);
    }

    fn allocate_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    fn prune_empty_panes(&mut self) {
        let Some(root) = self.root.take() else {
            return;
        };

        let focused_pane = self.focused_pane;
        self.root = Self::prune_empty_nodes(root);
        if let Some(root) = self.root.as_ref() {
            if Self::find_pane(root, focused_pane).is_some() {
                self.focused_pane = focused_pane;
            } else {
                self.focused_pane = self.first_pane_id().unwrap_or(focused_pane);
            }
        }
    }

    fn split_focused_pane(&mut self, axis: SplitAxis) -> bool {
        let Some(root) = self.root.take() else {
            return false;
        };

        let new_pane_id = self.allocate_pane_id();
        let (root, changed) = Self::split_node(root, self.focused_pane, axis, new_pane_id);
        self.root = Some(root);
        if changed {
            self.focused_pane = new_pane_id;
        }
        changed
    }

    fn split_node(
        node: LayoutNode,
        target: PaneId,
        axis: SplitAxis,
        new_pane_id: PaneId,
    ) -> (LayoutNode, bool) {
        match node {
            LayoutNode::Pane(pane) if pane.id == target => (
                LayoutNode::Split(SplitNode::new(
                    axis,
                    LayoutNode::Pane(pane),
                    LayoutNode::Pane(PaneNode::new(new_pane_id, TabGroup::new(Vec::new()))),
                )),
                true,
            ),
            LayoutNode::Pane(pane) => (LayoutNode::Pane(pane), false),
            LayoutNode::Split(split) => {
                let SplitNode {
                    axis: split_axis,
                    first,
                    second,
                    split_size,
                } = split;
                let (first, changed) = Self::split_node(*first, target, axis, new_pane_id);
                if changed {
                    return (
                        LayoutNode::Split(SplitNode {
                            axis: split_axis,
                            first: Box::new(first),
                            second,
                            split_size,
                        }),
                        true,
                    );
                }

                let (second, changed) = Self::split_node(*second, target, axis, new_pane_id);
                (
                    LayoutNode::Split(SplitNode {
                        axis: split_axis,
                        first: Box::new(first),
                        second: Box::new(second),
                        split_size,
                    }),
                    changed,
                )
            }
        }
    }

    fn close_focused_pane(&mut self) -> bool {
        let Some(root) = self.root.take() else {
            return false;
        };

        let (root, removed) = Self::remove_pane(root, self.focused_pane);
        self.root = root;
        if removed {
            self.focused_pane = self.first_pane_id().unwrap_or(self.focused_pane);
        }
        removed
    }

    fn remove_pane(node: LayoutNode, target: PaneId) -> (Option<LayoutNode>, bool) {
        match node {
            LayoutNode::Pane(pane) if pane.id == target => (None, true),
            LayoutNode::Pane(pane) => (Some(LayoutNode::Pane(pane)), false),
            LayoutNode::Split(split) => {
                let SplitNode {
                    axis,
                    first,
                    second,
                    split_size,
                } = split;
                let first_node = *first;
                let second_node = *second;
                let (first, removed_first) = Self::remove_pane(first_node, target);
                if removed_first {
                    return match first {
                        Some(first) => (
                            Some(LayoutNode::Split(SplitNode {
                                axis,
                                first: Box::new(first),
                                second: Box::new(second_node),
                                split_size,
                            })),
                            true,
                        ),
                        None => (Some(second_node), true),
                    };
                }

                let (second, removed_second) = Self::remove_pane(second_node, target);
                if removed_second {
                    return match second {
                        Some(second) => (
                            Some(LayoutNode::Split(SplitNode {
                                axis,
                                first: Box::new(first.expect("first child should exist")),
                                second: Box::new(second),
                                split_size,
                            })),
                            true,
                        ),
                        None => (Some(first.expect("first child should exist")), true),
                    };
                }

                (
                    Some(LayoutNode::Split(SplitNode {
                        axis,
                        first: Box::new(first.expect("first child should exist")),
                        second: Box::new(second.expect("second child should exist")),
                        split_size,
                    })),
                    false,
                )
            }
        }
    }

    fn prune_empty_nodes(node: LayoutNode) -> Option<LayoutNode> {
        match node {
            LayoutNode::Pane(pane) => {
                if pane.tab_group.is_empty() {
                    None
                } else {
                    Some(LayoutNode::Pane(pane))
                }
            }
            LayoutNode::Split(split) => {
                let first = Self::prune_empty_nodes(*split.first);
                let second = Self::prune_empty_nodes(*split.second);
                match (first, second) {
                    (Some(first), Some(second)) => Some(LayoutNode::Split(SplitNode {
                        axis: split.axis,
                        first: Box::new(first),
                        second: Box::new(second),
                        split_size: split.split_size,
                    })),
                    (Some(first), None) => Some(first),
                    (None, Some(second)) => Some(second),
                    (None, None) => None,
                }
            }
        }
    }

    fn move_focus(&mut self, direction: FocusDirection) -> bool {
        let regions = self.pane_regions();
        let Some(current) = regions
            .iter()
            .copied()
            .find(|region| region.id == self.focused_pane)
        else {
            return false;
        };

        let candidate = match direction {
            FocusDirection::Left => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.right() <= current.left())
                .filter(|region| region.vertical_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        current.left().saturating_sub(region.right()),
                        current.top().abs_diff(region.top()),
                    )
                }),
            FocusDirection::Down => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.top() >= current.bottom())
                .filter(|region| region.horizontal_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        region.top().saturating_sub(current.bottom()),
                        current.left().abs_diff(region.left()),
                    )
                }),
            FocusDirection::Up => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.bottom() <= current.top())
                .filter(|region| region.horizontal_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        current.top().saturating_sub(region.bottom()),
                        current.left().abs_diff(region.left()),
                    )
                }),
            FocusDirection::Right => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.left() >= current.right())
                .filter(|region| region.vertical_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        region.left().saturating_sub(current.right()),
                        current.top().abs_diff(region.top()),
                    )
                }),
        };

        if let Some(candidate) = candidate {
            self.focused_pane = candidate.id;
            return true;
        }

        false
    }

    fn pane_regions(&self) -> Vec<PaneRegion> {
        let mut regions = Vec::new();
        let Some(root) = self.root.as_ref() else {
            return regions;
        };

        let content_rows = self.size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, self.size.cols);
        Self::collect_pane_regions(root, self.origin, content_size, &mut regions);
        regions
    }

    fn pane_region(&self, id: PaneId) -> Option<PaneRegion> {
        self.pane_regions()
            .into_iter()
            .find(|region| region.id == id)
    }

    fn first_pane_id(&self) -> Option<PaneId> {
        self.root.as_ref().and_then(Self::first_pane_in_node)
    }

    fn first_pane_in_node(node: &LayoutNode) -> Option<PaneId> {
        match node {
            LayoutNode::Pane(pane) => Some(pane.id),
            LayoutNode::Split(split) => Self::first_pane_in_node(&split.first)
                .or_else(|| Self::first_pane_in_node(&split.second)),
        }
    }

    fn collect_pane_regions(
        node: &LayoutNode,
        origin: Position,
        size: Size,
        regions: &mut Vec<PaneRegion>,
    ) {
        match node {
            LayoutNode::Pane(pane) => regions.push(PaneRegion {
                id: pane.id,
                origin,
                size,
            }),
            LayoutNode::Split(split) => {
                let (first_origin, first_size, second_origin, second_size) =
                    Self::split_regions(origin, size, split.axis, split.split_size);
                Self::collect_pane_regions(&split.first, first_origin, first_size, regions);
                Self::collect_pane_regions(&split.second, second_origin, second_size, regions);
            }
        }
    }

    fn render_node(node: &mut LayoutNode, screen: &mut Screen, origin: Position, size: Size) {
        match node {
            LayoutNode::Pane(pane) => pane.tab_group.render(screen, origin, size),
            LayoutNode::Split(split) => {
                let (first_origin, first_size, second_origin, second_size) =
                    Self::split_regions(origin, size, split.axis, split.split_size);
                Self::render_node(&mut split.first, screen, first_origin, first_size);
                Self::render_node(&mut split.second, screen, second_origin, second_size);
            }
        }
    }

    fn split_regions(
        origin: Position,
        size: Size,
        axis: SplitAxis,
        split_size: SplitSize,
    ) -> (Position, Size, Position, Size) {
        match axis {
            SplitAxis::Horizontal => {
                let first_rows = Self::weighted_extent(
                    size.rows,
                    split_size.first_weight,
                    split_size.second_weight,
                );
                let second_rows = size.rows.saturating_sub(first_rows);
                (
                    origin,
                    Size::new(first_rows, size.cols),
                    Position::new(origin.row.saturating_add(first_rows), origin.col),
                    Size::new(second_rows, size.cols),
                )
            }
            SplitAxis::Vertical => {
                let first_cols = Self::weighted_extent(
                    size.cols,
                    split_size.first_weight,
                    split_size.second_weight,
                );
                let second_cols = size.cols.saturating_sub(first_cols);
                (
                    origin,
                    Size::new(size.rows, first_cols),
                    Position::new(origin.row, origin.col.saturating_add(first_cols)),
                    Size::new(size.rows, second_cols),
                )
            }
        }
    }

    fn weighted_extent(total: u16, first_weight: u16, second_weight: u16) -> u16 {
        let total = u32::from(total);
        let denominator = u32::from(first_weight.max(1)) + u32::from(second_weight.max(1));
        ((total * u32::from(first_weight.max(1))) / denominator) as u16
    }

    fn find_pane(node: &LayoutNode, id: PaneId) -> Option<&PaneNode> {
        match node {
            LayoutNode::Pane(pane) if pane.id == id => Some(pane),
            LayoutNode::Pane(_) => None,
            LayoutNode::Split(split) => {
                Self::find_pane(&split.first, id).or_else(|| Self::find_pane(&split.second, id))
            }
        }
    }

    fn find_pane_mut(node: &mut LayoutNode, id: PaneId) -> Option<&mut PaneNode> {
        match node {
            LayoutNode::Pane(pane) if pane.id == id => Some(pane),
            LayoutNode::Pane(_) => None,
            LayoutNode::Split(split) => Self::find_pane_mut(&mut split.first, id)
                .or_else(|| Self::find_pane_mut(&mut split.second, id)),
        }
    }
}

impl Widget for Layout {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        self.prune_empty_panes();
        let handled = match action.kind.as_ref() {
            Some(ActionKind::SplitVertical) => self.split_focused_pane(SplitAxis::Vertical),
            Some(ActionKind::SplitHorizontal) => self.split_focused_pane(SplitAxis::Horizontal),
            Some(ActionKind::FocusPaneLeft) => self.move_focus(FocusDirection::Left),
            Some(ActionKind::FocusPaneDown) => self.move_focus(FocusDirection::Down),
            Some(ActionKind::FocusPaneUp) => self.move_focus(FocusDirection::Up),
            Some(ActionKind::FocusPaneRight) => self.move_focus(FocusDirection::Right),
            Some(ActionKind::ClosePane) => self.close_focused_pane(),
            _ => {
                if self.should_exit() {
                    false
                } else {
                    let handled =
                        self.active_tab_group_mut().process_action(action) == ActionResult::Handled;
                    if handled && self.active_tab_group().is_empty() {
                        self.close_focused_pane();
                    }
                    handled
                }
            }
        };

        if handled {
            ActionResult::Handled
        } else {
            ActionResult::NotHandled
        }
    }
}

#[cfg(test)]
mod tests;
