use crate::buffer::Boundary;

/// Operators that wait for a motion or text object to define the target region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
}

/// Boundary-based delete targets that mirror motion families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryMotion {
    WordForward,
    WordEnd,
    WordBackward,
    BigWordForward,
    BigWordEnd,
    BigWordBackward,
}

/// Text objects that define a selection region for use with operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    InnerWord,
    AroundWord,
}

/// Operator targets used after an operator key is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorTarget {
    TextObject(TextObject),
    BoundaryMotion(BoundaryMotion),
}

/// Actions that the main event loop processes.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    MoveLeft,
    MoveDown,
    MoveUp,
    MoveRight,
    InsertChar(char),
    SwitchToNormal,
    SwitchToInsert,
    Quit,
    None,
    ForwardTo(Boundary),
    BackTo(Boundary),
    MoveToLineEnd,
    MoveToLineStart,
    MoveToLineContentStart,
    MoveToFirstLine,
    MoveToLastLine,
    MoveToScreenTop,
    MoveToScreenMiddle,
    MoveToScreenBottom,
    DeleteBackward,
    DeleteForward,
    JoinWithSpace,
    JoinWithoutSpace,
    DeleteLine,
    ChangeLine,
    ChangeToLineEnd,
    AppendAfterCursor,
    AppendToLineEnd,
    InsertAtLineStart,
    OpenLineBelow,
    OpenLineAbove,
    MoveToMatchingBracket,
    MoveToPreviousParagraph,
    MoveToNextParagraph,
    FindForward(char),
    FindBackward(char),
    TillForward(char),
    TillBackward(char),
    RepeatLastFind,
    RepeatLastFindReverse,
    Undo,
    Redo,
    Count(usize, Box<Action>),
    Operation(Operator, OperatorTarget),
}

impl Action {
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft
                | Action::MoveRight
                | Action::ForwardTo(_)
                | Action::BackTo(_)
                | Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
                | Action::InsertChar(_)
                | Action::DeleteBackward
                | Action::DeleteForward
                | Action::DeleteLine
                | Action::ChangeLine
                | Action::ChangeToLineEnd
                | Action::JoinWithSpace
                | Action::JoinWithoutSpace
                | Action::AppendAfterCursor
                | Action::AppendToLineEnd
                | Action::InsertAtLineStart
                | Action::OpenLineBelow
                | Action::OpenLineAbove
                | Action::FindForward(_)
                | Action::FindBackward(_)
                | Action::TillForward(_)
                | Action::TillBackward(_)
                | Action::RepeatLastFind
                | Action::RepeatLastFindReverse
        )
    }

    pub fn uses_remembered_column(&self) -> bool {
        matches!(
            self,
            Action::MoveUp
                | Action::MoveDown
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::MoveToScreenTop
                | Action::MoveToScreenMiddle
                | Action::MoveToScreenBottom
                | Action::MoveToPreviousParagraph
                | Action::MoveToNextParagraph
        )
    }

    pub fn is_countable(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft
                | Action::MoveRight
                | Action::MoveUp
                | Action::MoveDown
                | Action::ForwardTo(_)
                | Action::BackTo(_)
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::MoveToScreenTop
                | Action::MoveToScreenBottom
                | Action::JoinWithSpace
                | Action::JoinWithoutSpace
                | Action::DeleteLine
                | Action::ChangeLine
                | Action::ChangeToLineEnd
                | Action::OpenLineBelow
                | Action::OpenLineAbove
                | Action::FindForward(_)
                | Action::FindBackward(_)
                | Action::TillForward(_)
                | Action::TillBackward(_)
                | Action::RepeatLastFind
                | Action::Operation(_, _)
                | Action::RepeatLastFindReverse
                | Action::MoveToPreviousParagraph
                | Action::MoveToNextParagraph
        )
    }

    pub fn is_line_action(&self) -> bool {
        matches!(
            self,
            Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::AppendToLineEnd
                | Action::InsertAtLineStart
        )
    }

    pub fn with_count(self, count: usize) -> Option<Action> {
        if (self.is_countable() || self.is_line_action()) && count > 0 && count < 10000 {
            Some(Action::Count(count, Box::new(self)))
        } else {
            None
        }
    }

    pub fn switches_to_insert_mode(&self) -> bool {
        match self {
            Action::SwitchToInsert
            | Action::AppendAfterCursor
            | Action::AppendToLineEnd
            | Action::InsertAtLineStart
            | Action::ChangeLine
            | Action::ChangeToLineEnd
            | Action::OpenLineBelow
            | Action::OpenLineAbove => true,
            Action::Count(_, inner) => inner.switches_to_insert_mode(),
            Action::Operation(_, _) => false,
            _ => false,
        }
    }

    pub fn is_snapshottable(&self) -> bool {
        match self {
            Action::SwitchToNormal => true,
            Action::DeleteBackward
            | Action::DeleteForward
            | Action::DeleteLine
            | Action::ChangeLine
            | Action::ChangeToLineEnd
            | Action::JoinWithSpace
            | Action::JoinWithoutSpace
            | Action::AppendAfterCursor
            | Action::AppendToLineEnd
            | Action::InsertAtLineStart
            | Action::OpenLineBelow
            | Action::OpenLineAbove => true,
            Action::InsertChar(_) => false,
            Action::Undo | Action::Redo => false,
            Action::Count(_, inner) => inner.is_snapshottable(),
            Action::Operation(Operator::Delete, _) => true,
            _ => false,
        }
    }

    pub fn updates_snapshot_cursor(&self) -> bool {
        match self {
            Action::MoveLeft
            | Action::MoveDown
            | Action::MoveUp
            | Action::MoveRight
            | Action::ForwardTo(_)
            | Action::BackTo(_)
            | Action::MoveToLineEnd
            | Action::MoveToLineStart
            | Action::MoveToLineContentStart
            | Action::MoveToFirstLine
            | Action::MoveToLastLine
            | Action::MoveToScreenTop
            | Action::MoveToScreenMiddle
            | Action::MoveToScreenBottom
            | Action::MoveToMatchingBracket
            | Action::MoveToPreviousParagraph
            | Action::MoveToNextParagraph
            | Action::FindForward(_)
            | Action::FindBackward(_)
            | Action::TillForward(_)
            | Action::TillBackward(_)
            | Action::RepeatLastFind
            | Action::RepeatLastFindReverse => true,
            Action::Count(_, inner) => inner.updates_snapshot_cursor(),
            Action::Operation(_, _) => false,
            _ => false,
        }
    }
}

/// Result of processing a key in a mode.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKeyResult {
    Complete(Action),
    WaitForMore,
    InvalidSequence,
}
