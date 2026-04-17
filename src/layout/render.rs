//! Layout rendering and focus navigation helpers.

use super::Layout;
use super::geometry::FocusDirection;
use super::node::{LayoutNode, SplitAxis, SplitSize};
use crate::screen::Screen;
use crate::status_bar::StatusBarContext;
use crate::window::{Position, Size};

impl Layout {
    pub(super) fn move_focus(&mut self, direction: FocusDirection) -> bool {
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
            let target = self
                .resolve_directional_focus_target(current.id, candidate.id)
                .unwrap_or(candidate.id);
            return self.focus_pane(target);
        }

        false
    }

    pub(super) fn render_layout(&mut self, screen: &mut Screen, origin: Position, size: Size) {
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

    pub(super) fn render_node(
        node: &mut LayoutNode,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        match node {
            LayoutNode::Pane(pane) => pane.window_group.render(screen, origin, size),
            LayoutNode::Split(split) => {
                let (first_origin, first_size, second_origin, second_size) =
                    Self::split_regions(origin, size, split.axis, split.split_size);
                Self::render_node(&mut split.first, screen, first_origin, first_size);
                Self::render_node(&mut split.second, screen, second_origin, second_size);
            }
        }
    }

    pub(super) fn split_regions(
        origin: Position,
        size: Size,
        axis: SplitAxis,
        split_size: SplitSize,
    ) -> (Position, Size, Position, Size) {
        match axis {
            SplitAxis::Horizontal => {
                let first_rows = Self::weighted_extent(
                    size.rows,
                    split_size.first_weight(),
                    split_size.second_weight(),
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
                    split_size.first_weight(),
                    split_size.second_weight(),
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
}
