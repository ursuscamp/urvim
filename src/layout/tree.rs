//! Layout tree mutation and traversal helpers.

use super::geometry::PaneRegion;
use super::node::{LayoutNode, PaneNode, SplitAxis, SplitNode};
use super::{Layout, PaneId};
use crate::window::Window;
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildSide {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeOutcome {
    NotFound,
    Found,
    Handled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
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
                let wrapped_row_offset = buffer_view.wrapped_row_offset();
                let wrap_enabled = pane.window_group.active_window().wrap_enabled();

                let mut window_group = WindowGroup::new(vec![Window::from_buffer_id(buffer_id)]);
                {
                    let window = window_group.active_window_mut();
                    let view = window.buffer_view_mut();
                    view.set_scroll_offset(scroll_offset);
                    view.set_wrapped_row_offset(wrapped_row_offset);
                    view.set_cursor(cursor);
                    window.set_wrap_enabled(wrap_enabled);
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

    pub(super) fn resize_focused_pane(
        &mut self,
        axis: SplitAxis,
        direction: ResizeDirection,
    ) -> bool {
        let origin = self.origin;
        let content_size = Size::new(self.size.rows.saturating_sub(1), self.size.cols);
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        let mut path = Vec::new();
        if !Self::pane_path(root, self.focused_pane, &mut path) {
            return false;
        }

        matches!(
            Self::resize_node(root, origin, content_size, &path, axis, direction),
            ResizeOutcome::Handled | ResizeOutcome::Found
        )
    }

    pub(super) fn equalize_splits(&mut self) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        Self::equalize_node(root);
        true
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

    fn resize_node(
        node: &mut LayoutNode,
        origin: Position,
        size: Size,
        path: &[ChildSide],
        axis: SplitAxis,
        direction: ResizeDirection,
    ) -> ResizeOutcome {
        match node {
            LayoutNode::Pane(_) => {
                if path.is_empty() {
                    ResizeOutcome::Found
                } else {
                    ResizeOutcome::NotFound
                }
            }
            LayoutNode::Split(split) => {
                let Some((next_side, remaining)) = path.split_first() else {
                    return ResizeOutcome::NotFound;
                };

                let (first_origin, first_size, second_origin, second_size) =
                    Self::split_regions(origin, size, split.axis, split.split_size);

                let (child, child_origin, child_size) = match next_side {
                    ChildSide::First => (&mut split.first, first_origin, first_size),
                    ChildSide::Second => (&mut split.second, second_origin, second_size),
                };

                match Self::resize_node(child, child_origin, child_size, remaining, axis, direction)
                {
                    ResizeOutcome::Handled => ResizeOutcome::Handled,
                    ResizeOutcome::NotFound => ResizeOutcome::NotFound,
                    ResizeOutcome::Found => {
                        if split.axis == axis {
                            Self::resize_split(
                                split,
                                split.axis,
                                first_size,
                                second_size,
                                *next_side,
                                direction,
                            );
                            ResizeOutcome::Handled
                        } else {
                            ResizeOutcome::Found
                        }
                    }
                }
            }
        }
    }

    fn resize_split(
        split: &mut SplitNode,
        axis: SplitAxis,
        first_size: Size,
        second_size: Size,
        target_side: ChildSide,
        direction: ResizeDirection,
    ) {
        let total = match axis {
            SplitAxis::Horizontal => first_size.rows.saturating_add(second_size.rows),
            SplitAxis::Vertical => first_size.cols.saturating_add(second_size.cols),
        };

        if total <= 1 {
            return;
        }

        let current_target = match (axis, target_side) {
            (SplitAxis::Horizontal, ChildSide::First) => first_size.rows,
            (SplitAxis::Horizontal, ChildSide::Second) => second_size.rows,
            (SplitAxis::Vertical, ChildSide::First) => first_size.cols,
            (SplitAxis::Vertical, ChildSide::Second) => second_size.cols,
        };

        let delta = match (axis, direction, target_side) {
            (SplitAxis::Vertical, ResizeDirection::Left, ChildSide::First) => -1,
            (SplitAxis::Vertical, ResizeDirection::Left, ChildSide::Second) => 1,
            (SplitAxis::Vertical, ResizeDirection::Right, ChildSide::First) => 1,
            (SplitAxis::Vertical, ResizeDirection::Right, ChildSide::Second) => -1,
            (SplitAxis::Horizontal, ResizeDirection::Up, ChildSide::First) => -1,
            (SplitAxis::Horizontal, ResizeDirection::Up, ChildSide::Second) => 1,
            (SplitAxis::Horizontal, ResizeDirection::Down, ChildSide::First) => 1,
            (SplitAxis::Horizontal, ResizeDirection::Down, ChildSide::Second) => -1,
            _ => 0,
        };

        if delta == 0 {
            return;
        }

        let min_target = 1i16;
        let max_target = total.saturating_sub(1) as i16;
        let desired_target = (current_target as i16 + delta).clamp(min_target, max_target) as u16;

        let (first_weight, second_weight) = match (axis, target_side) {
            (SplitAxis::Horizontal, ChildSide::First) => (desired_target, total - desired_target),
            (SplitAxis::Horizontal, ChildSide::Second) => (total - desired_target, desired_target),
            (SplitAxis::Vertical, ChildSide::First) => (desired_target, total - desired_target),
            (SplitAxis::Vertical, ChildSide::Second) => (total - desired_target, desired_target),
        };

        split.split_size = super::node::SplitSize::new(first_weight, second_weight);
    }

    fn equalize_node(node: &mut LayoutNode) {
        match node {
            LayoutNode::Pane(_) => {}
            LayoutNode::Split(split) => {
                split.split_size = super::node::SplitSize::even();
                Self::equalize_node(&mut split.first);
                Self::equalize_node(&mut split.second);
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

    pub(super) fn prune_expired_yank_flashes_at(&mut self, now: Instant) -> bool {
        Self::prune_expired_yank_flashes_in_node(self.root.as_mut(), now)
    }

    fn prune_expired_yank_flashes_in_node(node: Option<&mut LayoutNode>, now: Instant) -> bool {
        let Some(node) = node else {
            return false;
        };

        match node {
            LayoutNode::Pane(pane) => pane.window_group.prune_expired_yank_flash(now),
            LayoutNode::Split(split) => {
                let first =
                    Self::prune_expired_yank_flashes_in_node(Some(split.first.as_mut()), now);
                let second =
                    Self::prune_expired_yank_flashes_in_node(Some(split.second.as_mut()), now);
                first || second
            }
        }
    }
}
