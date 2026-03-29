use super::*;
use crate::editor::{BoundaryMotion, LinewiseMotion, OperatorTarget, TextObject};

/// A whole-line delete range resolved from a linewise motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinewiseDeleteRange {
    /// The first line to delete.
    pub start_line: usize,
    /// The number of lines to delete.
    pub count: usize,
}

impl LinewiseDeleteRange {
    /// Creates a new linewise delete range.
    pub fn new(start_line: usize, count: usize) -> Self {
        Self { start_line, count }
    }
}

impl Buffer {
    /// Resolves the range affected by an operator target at the given cursor.
    pub fn get_operator_target_range(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
    ) -> Option<TextObjectRange> {
        match target {
            OperatorTarget::TextObject(text_object) => match text_object {
                TextObject::InnerWord => self.get_inner_word_range_with_count(cursor, 1),
                TextObject::AroundWord => self.get_around_word_range_with_count(cursor, 1),
                TextObject::InnerBigWord => self.get_inner_big_word_range_with_count(cursor, 1),
                TextObject::AroundBigWord => self.get_around_big_word_range_with_count(cursor, 1),
                TextObject::InnerBracket(kind) => {
                    self.get_inner_bracket_range_with_count(cursor, kind, 1)
                }
                TextObject::AroundBracket(kind) => {
                    self.get_around_bracket_range_with_count(cursor, kind, 1)
                }
                TextObject::InnerQuote(kind) => {
                    self.get_inner_quote_range_with_count(cursor, kind, 1)
                }
                TextObject::AroundQuote(kind) => {
                    self.get_around_quote_range_with_count(cursor, kind, 1)
                }
            },
            OperatorTarget::BoundaryMotion(motion) => {
                self.get_boundary_motion_range(cursor, motion)
            }
            OperatorTarget::LinewiseMotion(_) => None,
        }
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
                TextObject::InnerBigWord => self.get_inner_big_word_range_with_count(cursor, count),
                TextObject::AroundBigWord => self.get_around_big_word_range_with_count(cursor, count),
                TextObject::InnerBracket(kind) => {
                    self.get_inner_bracket_range_with_count(cursor, kind, count)
                }
                TextObject::AroundBracket(kind) => {
                    self.get_around_bracket_range_with_count(cursor, kind, count)
                }
                TextObject::InnerQuote(kind) => {
                    self.get_inner_quote_range_with_count(cursor, kind, count)
                }
                TextObject::AroundQuote(kind) => {
                    self.get_around_quote_range_with_count(cursor, kind, count)
                }
            },
            OperatorTarget::BoundaryMotion(motion) => {
                self.get_boundary_motion_range_with_count(cursor, motion, count)
            }
            OperatorTarget::LinewiseMotion(_) => None,
        }
    }

    /// Resolves the range affected by a linewise operator target at the given cursor.
    pub fn get_linewise_operator_target_range(
        &self,
        cursor: Cursor,
        target: LinewiseMotion,
    ) -> Option<LinewiseDeleteRange> {
        match target {
            LinewiseMotion::FirstLine => {
                self.get_linewise_operator_target_range_with_count(cursor, target, 1)
            }
            LinewiseMotion::LastLine => self.get_linewise_operator_target_range_with_count(
                cursor,
                target,
                self.line_count(),
            ),
        }
    }

    /// Resolves the range affected by a linewise operator target using a line count.
    pub fn get_linewise_operator_target_range_with_count(
        &self,
        cursor: Cursor,
        target: LinewiseMotion,
        count: usize,
    ) -> Option<LinewiseDeleteRange> {
        if count == 0 {
            return None;
        }

        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        let target_line = match target {
            LinewiseMotion::FirstLine => count.saturating_sub(1).min(total_lines.saturating_sub(1)),
            LinewiseMotion::LastLine => count.saturating_sub(1).min(total_lines.saturating_sub(1)),
        };

        let start_line = cursor.line.min(target_line);
        let end_line = cursor.line.max(target_line);
        Some(LinewiseDeleteRange::new(
            start_line,
            end_line - start_line + 1,
        ))
    }

    fn get_boundary_motion_range(
        &self,
        cursor: Cursor,
        motion: BoundaryMotion,
    ) -> Option<TextObjectRange> {
        match motion {
            BoundaryMotion::LineEnd => self.get_line_anchor_range(cursor, motion, 1),
            BoundaryMotion::LineStart => self.get_line_anchor_range(cursor, motion, 1),
            BoundaryMotion::LineContentStart => self.get_line_anchor_range(cursor, motion, 1),
            BoundaryMotion::WordForward => {
                self.get_forward_boundary_range_with_count(cursor, Boundary::Word, 1)
            }
            BoundaryMotion::WordEnd => {
                self.get_forward_end_boundary_range_with_count(cursor, Boundary::WordEnd, 1)
            }
            BoundaryMotion::WordBackward => {
                self.get_backward_boundary_range_with_count(cursor, Boundary::Word, 1)
            }
            BoundaryMotion::BigWordForward => {
                self.get_forward_boundary_range_with_count(cursor, Boundary::BigWord, 1)
            }
            BoundaryMotion::BigWordEnd => {
                self.get_forward_end_boundary_range_with_count(cursor, Boundary::BigWordEnd, 1)
            }
            BoundaryMotion::BigWordBackward => {
                self.get_backward_boundary_range_with_count(cursor, Boundary::BigWord, 1)
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
            BoundaryMotion::LineEnd => self.get_line_anchor_range(cursor, motion, count),
            BoundaryMotion::LineStart => self.get_line_anchor_range(cursor, motion, count),
            BoundaryMotion::LineContentStart => self.get_line_anchor_range(cursor, motion, count),
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

    fn get_line_anchor_range(
        &self,
        cursor: Cursor,
        motion: BoundaryMotion,
        count: usize,
    ) -> Option<TextObjectRange> {
        if count == 0 {
            return None;
        }
        let target_line = if count == 1 {
            cursor.line
        } else {
            count
                .saturating_sub(1)
                .min(self.line_count().saturating_sub(1))
        };
        let target_col = match motion {
            BoundaryMotion::LineEnd => self.line_len(target_line),
            BoundaryMotion::LineStart => 0,
            BoundaryMotion::LineContentStart => {
                self.first_non_whitespace_col(target_line).unwrap_or(0)
            }
            _ => return None,
        };
        let target = Cursor::new(target_line, target_col);
        Some(self.ordered_text_range(cursor, target))
    }

    fn ordered_text_range(&self, a: Cursor, b: Cursor) -> TextObjectRange {
        if (a.line, a.col) <= (b.line, b.col) {
            TextObjectRange { start: a, end: b }
        } else {
            TextObjectRange { start: b, end: a }
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
