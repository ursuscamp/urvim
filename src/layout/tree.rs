//! Layout tree mutation and traversal helpers.

use super::geometry::PaneRegion;
use super::node::{LayoutNode, PaneNode, SplitAxis, SplitNode};
use super::{Layout, PaneId};
use crate::window::Window;
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;

impl Layout {
    pub(super) fn allocate_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    pub(super) fn prune_empty_panes(&mut self) {
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

    pub(super) fn split_focused_pane(&mut self, axis: SplitAxis) -> bool {
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
            LayoutNode::Pane(pane) if pane.id == target => {
                let buffer_view = pane.window_group.active_buffer_view();
                let buffer_id = buffer_view.buffer_id();
                let cursor = buffer_view.cursor();
                let scroll_offset = buffer_view.scroll_offset();

                let mut window_group = WindowGroup::new(vec![Window::from_buffer_id(buffer_id)]);
                {
                    let view = window_group.active_window_mut().buffer_view_mut();
                    view.set_scroll_offset(scroll_offset);
                    view.set_cursor(cursor);
                }

                (
                    LayoutNode::Split(SplitNode::new(
                        axis,
                        LayoutNode::Pane(pane),
                        LayoutNode::Pane(PaneNode::new(new_pane_id, window_group)),
                    )),
                    true,
                )
            }
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

    pub(super) fn close_focused_pane(&mut self) -> bool {
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
                if pane.window_group.is_empty() {
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

    pub(super) fn first_pane_id(&self) -> Option<PaneId> {
        self.root.as_ref().and_then(Self::first_pane_in_node)
    }

    fn first_pane_in_node(node: &LayoutNode) -> Option<PaneId> {
        match node {
            LayoutNode::Pane(pane) => Some(pane.id),
            LayoutNode::Split(split) => Self::first_pane_in_node(&split.first)
                .or_else(|| Self::first_pane_in_node(&split.second)),
        }
    }

    pub(super) fn find_pane(node: &LayoutNode, id: PaneId) -> Option<&PaneNode> {
        match node {
            LayoutNode::Pane(pane) if pane.id == id => Some(pane),
            LayoutNode::Pane(_) => None,
            LayoutNode::Split(split) => {
                Self::find_pane(&split.first, id).or_else(|| Self::find_pane(&split.second, id))
            }
        }
    }

    pub(super) fn find_pane_mut(node: &mut LayoutNode, id: PaneId) -> Option<&mut PaneNode> {
        match node {
            LayoutNode::Pane(pane) if pane.id == id => Some(pane),
            LayoutNode::Pane(_) => None,
            LayoutNode::Split(split) => Self::find_pane_mut(&mut split.first, id)
                .or_else(|| Self::find_pane_mut(&mut split.second, id)),
        }
    }

    pub(super) fn pane_regions(&self) -> Vec<PaneRegion> {
        let mut regions = Vec::new();
        let Some(root) = self.root.as_ref() else {
            return regions;
        };

        let content_rows = self.size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, self.size.cols);
        Self::collect_pane_regions(root, self.origin, content_size, &mut regions);
        regions
    }

    pub(super) fn pane_region(&self, id: PaneId) -> Option<PaneRegion> {
        self.pane_regions()
            .into_iter()
            .find(|region| region.id == id)
    }

    pub(super) fn collect_pane_regions(
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
}
