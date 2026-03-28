use crate::buffer::{Boundary, BufferId};

/// Operators that wait for a motion or text object to define the target region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    /// Delete text and enter insert mode after a successful operation.
    Change,
}

/// Boundary-based delete targets that mirror motion families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryMotion {
    /// Move to the last non-whitespace character of the current line or target line.
    LineEnd,
    /// Move to the start of the current line or target line.
    LineStart,
    /// Move to the first non-whitespace character of the current line or target line.
    LineContentStart,
    /// Move to the next word start.
    WordForward,
    /// Move to the end of the current or next word.
    WordEnd,
    /// Move to the previous word start.
    WordBackward,
    /// Move to the next BigWord start.
    BigWordForward,
    /// Move to the end of the current or next BigWord.
    BigWordEnd,
    /// Move to the previous BigWord start.
    BigWordBackward,
}

/// Text objects that define a selection region for use with operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    InnerWord,
    AroundWord,
    /// Text between matching delimiters, excluding the delimiters themselves.
    InnerBracket(BracketKind),
    /// Text between matching delimiters, including the delimiters.
    AroundBracket(BracketKind),
    /// Text between matching quotes, excluding the quote delimiters.
    InnerQuote(QuoteKind),
    /// Text between matching quotes, including the quote delimiters.
    AroundQuote(QuoteKind),
}

/// Supported delimiter families for bracket text objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BracketKind {
    /// Parenthesis pairs `(` and `)`.
    Paren,
    /// Square bracket pairs `[` and `]`.
    Square,
    /// Curly brace pairs `{` and `}`.
    Curly,
    /// Angle bracket pairs `<` and `>`.
    Angle,
}

impl BracketKind {
    /// Returns the opening delimiter for this bracket family.
    pub fn opening_delimiter(self) -> char {
        match self {
            BracketKind::Paren => '(',
            BracketKind::Square => '[',
            BracketKind::Curly => '{',
            BracketKind::Angle => '<',
        }
    }

    /// Returns the closing delimiter for this bracket family.
    pub fn closing_delimiter(self) -> char {
        match self {
            BracketKind::Paren => ')',
            BracketKind::Square => ']',
            BracketKind::Curly => '}',
            BracketKind::Angle => '>',
        }
    }

    /// Returns true when the provided character is the opening delimiter.
    pub fn matches_opening(self, ch: char) -> bool {
        ch == self.opening_delimiter()
    }

    /// Returns true when the provided character is the closing delimiter.
    pub fn matches_closing(self, ch: char) -> bool {
        ch == self.closing_delimiter()
    }
}

/// Supported delimiter families for quote text objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuoteKind {
    /// Single quote delimiters (`'`).
    Single,
    /// Double quote delimiters (`"`).
    Double,
    /// Backtick delimiters (`` ` ``).
    Backtick,
}

impl QuoteKind {
    /// Returns the delimiter character for this quote family.
    pub fn delimiter(self) -> char {
        match self {
            QuoteKind::Single => '\'',
            QuoteKind::Double => '"',
            QuoteKind::Backtick => '`',
        }
    }
}

/// Operator targets used after an operator key is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorTarget {
    TextObject(TextObject),
    BoundaryMotion(BoundaryMotion),
    LinewiseMotion(LinewiseMotion),
}

/// Linewise operator targets for whole-line deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinewiseMotion {
    /// Move to the first line of the file.
    FirstLine,
    /// Move to the last line of the file.
    LastLine,
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
    PreviousTab,
    NextTab,
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
    SaveBuffer(Option<BufferId>),
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
                | Action::PreviousTab
                | Action::NextTab
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
                | Action::PreviousTab
                | Action::NextTab
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
            Action::Operation(Operator::Change, _) => true,
            Action::Operation(Operator::Delete, _) => false,
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
            Action::Operation(Operator::Delete, _) | Action::Operation(Operator::Change, _) => true,
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
