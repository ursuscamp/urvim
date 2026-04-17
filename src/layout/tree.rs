//! Layout tree mutation and traversal helpers.

use super::geometry::PaneRegion;
use super::node::{LayoutNode, PaneNode, SplitAxis, SplitNode};
use super::{Layout, PaneId};
use crate::window::Window;
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildSide {
    First,
    Second,
}

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
        let next_focused = if let Some(root) = self.root.as_ref() {
            if Self::find_pane(root, focused_pane).is_some() {
                focused_pane
            } else {
                Self::first_pane_in_node(root).unwrap_or(focused_pane)
            }
        } else {
            return;
        };

        self.focus_pane(next_focused);
    }

    pub(super) fn split_focused_pane(&mut self, axis: SplitAxis) -> bool {
        let Some(root) = self.root.take() else {
            return false;
        };

        let new_pane_id = self.allocate_pane_id();
        let (root, changed) = Self::split_node(root, self.focused_pane, axis, new_pane_id);
        self.root = Some(root);
        if changed {
            self.focus_pane(new_pane_id);
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
                        new_pane_id,
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
                    last_focused_pane,
                } = split;

                let (first, changed) = Self::split_node(*first, target, axis, new_pane_id);
                if changed {
                    return (
                        Self::rebuild_split(
                            split_axis,
                            first,
                            *second,
                            split_size,
                            last_focused_pane,
                        ),
                        true,
                    );
                }

                let (second, changed) = Self::split_node(*second, target, axis, new_pane_id);
                (
                    Self::rebuild_split(split_axis, first, second, split_size, last_focused_pane),
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
            let next_focused = self.first_pane_id().unwrap_or(self.focused_pane);
            self.focus_pane(next_focused);
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
                    last_focused_pane,
                } = split;

                let first_node = *first;
                let second_node = *second;
                let (first, removed_first) = Self::remove_pane(first_node, target);
                if removed_first {
                    return match first {
                        Some(first) => (
                            Some(Self::rebuild_split(
                                axis,
                                first,
                                second_node,
                                split_size,
                                last_focused_pane,
                            )),
                            true,
                        ),
                        None => (Some(second_node), true),
                    };
                }

                let (second, removed_second) = Self::remove_pane(second_node, target);
                if removed_second {
                    return match second {
                        Some(second) => {
                            let first = first.expect("first child should exist");
                            (
                                Some(Self::rebuild_split(
                                    axis,
                                    first,
                                    second,
                                    split_size,
                                    last_focused_pane,
                                )),
                                true,
                            )
                        }
                        None => (Some(first.expect("first child should exist")), true),
                    };
                }

                let first = first.expect("first child should exist");
                let second = second.expect("second child should exist");
                (
                    Some(Self::rebuild_split(
                        axis,
                        first,
                        second,
                        split_size,
                        last_focused_pane,
                    )),
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
                let SplitNode {
                    axis,
                    first,
                    second,
                    split_size,
                    last_focused_pane,
                } = split;

                let first = Self::prune_empty_nodes(*first);
                let second = Self::prune_empty_nodes(*second);
                match (first, second) {
                    (Some(first), Some(second)) => Some(Self::rebuild_split(
                        axis,
                        first,
                        second,
                        split_size,
                        last_focused_pane,
                    )),
                    (Some(first), None) => Some(first),
                    (None, Some(second)) => Some(second),
                    (None, None) => None,
                }
            }
        }
    }

    fn rebuild_split(
        axis: SplitAxis,
        first: LayoutNode,
        second: LayoutNode,
        split_size: super::node::SplitSize,
        remembered: PaneId,
    ) -> LayoutNode {
        let last_focused_pane = Self::normalize_last_focused_pane(&first, &second, remembered);
        LayoutNode::Split(SplitNode {
            axis,
            first: Box::new(first),
            second: Box::new(second),
            split_size,
            last_focused_pane,
        })
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

    fn normalize_last_focused_pane(
        first: &LayoutNode,
        second: &LayoutNode,
        remembered: PaneId,
    ) -> PaneId {
        if Self::find_pane(first, remembered).is_some()
            || Self::find_pane(second, remembered).is_some()
        {
            remembered
        } else {
            Self::first_pane_in_node(first)
                .or_else(|| Self::first_pane_in_node(second))
                .unwrap_or(remembered)
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

    pub(super) fn focus_pane(&mut self, pane_id: PaneId) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        if Self::record_focus_in_node(root, pane_id) {
            self.focused_pane = pane_id;
            true
        } else {
            false
        }
    }

    fn record_focus_in_node(node: &mut LayoutNode, target: PaneId) -> bool {
        match node {
            LayoutNode::Pane(pane) => pane.id == target,
            LayoutNode::Split(split) => {
                if Self::record_focus_in_node(&mut split.first, target) {
                    split.last_focused_pane = target;
                    return true;
                }

                if Self::record_focus_in_node(&mut split.second, target) {
                    split.last_focused_pane = target;
                    return true;
                }

                false
            }
        }
    }

    pub(super) fn pane_path(node: &LayoutNode, target: PaneId, path: &mut Vec<ChildSide>) -> bool {
        match node {
            LayoutNode::Pane(pane) => pane.id == target,
            LayoutNode::Split(split) => {
                path.push(ChildSide::First);
                if Self::pane_path(&split.first, target, path) {
                    return true;
                }
                path.pop();

                path.push(ChildSide::Second);
                if Self::pane_path(&split.second, target, path) {
                    return true;
                }
                path.pop();
                false
            }
        }
    }

    pub(super) fn node_at_path<'a>(
        node: &'a LayoutNode,
        path: &[ChildSide],
    ) -> Option<&'a LayoutNode> {
        if path.is_empty() {
            return Some(node);
        }

        match (node, path[0]) {
            (LayoutNode::Pane(_), _) => None,
            (LayoutNode::Split(split), ChildSide::First) => {
                Self::node_at_path(&split.first, &path[1..])
            }
            (LayoutNode::Split(split), ChildSide::Second) => {
                Self::node_at_path(&split.second, &path[1..])
            }
        }
    }

    fn resolve_preferred_pane(node: &LayoutNode) -> Option<PaneId> {
        match node {
            LayoutNode::Pane(pane) => Some(pane.id),
            LayoutNode::Split(split) => {
                if Self::find_pane(node, split.last_focused_pane).is_some() {
                    Some(split.last_focused_pane)
                } else {
                    Self::first_pane_in_node(node)
                }
            }
        }
    }

    pub(super) fn resolve_directional_focus_target(
        &self,
        current: PaneId,
        candidate: PaneId,
    ) -> Option<PaneId> {
        let root = self.root.as_ref()?;
        let mut current_path = Vec::new();
        let mut candidate_path = Vec::new();
        if !Self::pane_path(root, current, &mut current_path) {
            return None;
        }
        if !Self::pane_path(root, candidate, &mut candidate_path) {
            return None;
        }

        let divergence = current_path
            .iter()
            .zip(candidate_path.iter())
            .position(|(a, b)| a != b)?;

        let subtree_path = &candidate_path[..=divergence];
        let subtree = Self::node_at_path(root, subtree_path)?;
        Self::resolve_preferred_pane(subtree)
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
