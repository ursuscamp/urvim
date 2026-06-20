//! Internal layout geometry and focus helpers.

use crate::window::{Position, Size};

use super::PaneId;

#[derive(Debug, Clone, Copy)]
pub(super) struct PaneRegion {
    pub(super) id: PaneId,
    pub(super) origin: Position,
    pub(super) size: Size,
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
