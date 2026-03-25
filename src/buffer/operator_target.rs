use super::*;
use crate::editor::{BoundaryMotion, OperatorTarget, TextObject};

impl Buffer {
    /// Resolves the range affected by an operator target at the given cursor.
    pub fn get_operator_target_range(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
    ) -> Option<TextObjectRange> {
        self.get_operator_target_range_with_count(cursor, target, 1)
    }

    /// Resolves the range affected by an operator target with count expansion.
    pub fn get_operator_target_range_with_count(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
        count: usize,
    ) -> Option<TextObjectRange> {
        if count == 0 {
            return None;
        }

        match target {
            OperatorTarget::TextObject(text_object) => match text_object {
                TextObject::InnerWord => self.get_inner_word_range_with_count(cursor, count),
                TextObject::AroundWord => self.get_around_word_range_with_count(cursor, count),
            },
            OperatorTarget::BoundaryMotion(motion) => {
                self.get_boundary_motion_range_with_count(cursor, motion, count)
            }
        }
    }

    fn get_boundary_motion_range_with_count(
        &self,
        cursor: Cursor,
        motion: BoundaryMotion,
        count: usize,
    ) -> Option<TextObjectRange> {
        match motion {
            BoundaryMotion::WordForward => {
                self.get_forward_boundary_range_with_count(cursor, Boundary::Word, count)
            }
            BoundaryMotion::WordEnd => {
                self.get_forward_end_boundary_range_with_count(cursor, Boundary::WordEnd, count)
            }
            BoundaryMotion::WordBackward => {
                self.get_backward_boundary_range_with_count(cursor, Boundary::Word, count)
            }
            BoundaryMotion::BigWordForward => {
                self.get_forward_boundary_range_with_count(cursor, Boundary::BigWord, count)
            }
            BoundaryMotion::BigWordEnd => {
                self.get_forward_end_boundary_range_with_count(cursor, Boundary::BigWordEnd, count)
            }
            BoundaryMotion::BigWordBackward => {
                self.get_backward_boundary_range_with_count(cursor, Boundary::BigWord, count)
            }
        }
    }

    fn get_forward_boundary_range_with_count(
        &self,
        cursor: Cursor,
        boundary: Boundary,
        count: usize,
    ) -> Option<TextObjectRange> {
        let mut target = cursor;
        for _ in 0..count {
            target = self.next_boundary(target, boundary)?;
        }
        Some(TextObjectRange {
            start: cursor,
            end: target,
        })
    }

    fn get_forward_end_boundary_range_with_count(
        &self,
        cursor: Cursor,
        boundary: Boundary,
        count: usize,
    ) -> Option<TextObjectRange> {
        let mut target = cursor;
        for _ in 0..count {
            target = self.next_boundary(target, boundary)?;
        }
        let end = self.next_cursor(target)?;
        Some(TextObjectRange { start: cursor, end })
    }

    fn get_backward_boundary_range_with_count(
        &self,
        cursor: Cursor,
        boundary: Boundary,
        count: usize,
    ) -> Option<TextObjectRange> {
        let mut target = cursor;
        for _ in 0..count {
            target = self.prev_boundary(target, boundary)?;
        }
        Some(TextObjectRange {
            start: target,
            end: cursor,
        })
    }
}
