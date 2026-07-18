//! Internal layout geometry and focus helpers.

use crate::ui::geometry::{Position, Size};

use super::PaneId;

#[derive(Debug, Clone, Copy)]
pub struct PaneRegion {
    /// Pane identifier.
    pub id: PaneId,
    /// Top-left origin of the pane.
    pub origin: Position,
    /// Content size of the pane.
    pub size: Size,
}

impl PaneRegion {
    pub(super) fn left(self) -> u16 {
        self.origin.col
    }

    pub(super) fn right(self) -> u16 {
        self.origin.col.saturating_add(self.size.cols)
    }

    pub(super) fn top(self) -> u16 {
        self.origin.row
    }

    pub(super) fn bottom(self) -> u16 {
        self.origin.row.saturating_add(self.size.rows)
    }

    pub(super) fn vertical_overlap(self, other: Self) -> u16 {
        self.bottom()
            .min(other.bottom())
            .saturating_sub(self.top().max(other.top()))
    }

    pub(super) fn horizontal_overlap(self, other: Self) -> u16 {
        self.right()
            .min(other.right())
            .saturating_sub(self.left().max(other.left()))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FocusDirection {
    Left,
    Down,
    Up,
    Right,
}
