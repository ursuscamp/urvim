use crate::buffer::{Boundary, BufferId};
use crate::editor::ModeKind;
use crate::globals;
use crate::register::RegisterName;

/// Operators that wait for a motion or text object to define the target region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    /// Delete text and enter insert mode after a successful operation.
    Change,
    /// Copy text into a register without mutating the buffer.
    Yank,
    /// Lowercase the targeted text.
    Lowercase,
    /// Uppercase the targeted text.
    Uppercase,
    /// Toggle the case of the targeted text.
    ToggleCase,
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
    /// Text between BigWord boundaries, excluding trailing whitespace.
    InnerBigWord,
    /// Text between BigWord boundaries, including trailing whitespace.
    AroundBigWord,
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
    /// Character-scan target resolved from `f`, `F`, `t`, or `T`.
    CharacterScan(globals::FindState),
    /// The active visual selection resolved from the current visual mode.
    Selection,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: Option<ActionKind>,
    pub from_mode: Option<ModeKind>,
    pub to_mode: Option<ModeKind>,
    pub register: Option<RegisterName>,
}

/// Intent payload for an action envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    /// Move the cursor one character to the left.
    MoveLeft,
    /// Move the cursor one character down.
    MoveDown,
    /// Move the cursor one character up.
    MoveUp,
    /// Move the cursor one character to the right.
    MoveRight,
    /// Shrink the focused pane horizontally.
    ResizePaneLeft,
    /// Grow the focused pane horizontally.
    ResizePaneRight,
    /// Shrink the focused pane vertically.
    ResizePaneUp,
    /// Grow the focused pane vertically.
    ResizePaneDown,
    /// Equalize all split ratios in the layout.
    EqualizeSplits,
    /// Split the focused pane vertically.
    SplitVertical,
    /// Split the focused pane horizontally.
    SplitHorizontal,
    /// Focus the pane to the left.
    FocusPaneLeft,
    /// Focus the pane below.
    FocusPaneDown,
    /// Focus the pane above.
    FocusPaneUp,
    /// Focus the pane to the right.
    FocusPaneRight,
    /// Close the focused pane.
    ClosePane,
    /// Insert a single character at the cursor.
    InsertChar(char),
    /// Insert a string at the cursor.
    InsertText(String),
    /// Insert a newline, letting the window decide whether to auto-indent it.
    InsertNewline,
    /// Exit the editor.
    Quit,
    /// Move forward to the next boundary of the requested kind.
    ForwardTo(Boundary),
    /// Move backward to the previous boundary of the requested kind.
    BackTo(Boundary),
    /// Move to the end of the current line.
    MoveToLineEnd,
    /// Move to the start of the current line.
    MoveToLineStart,
    /// Move to the first non-whitespace character of the current line.
    MoveToLineContentStart,
    /// Move to the first line of the buffer.
    MoveToFirstLine,
    /// Move to the last line of the buffer.
    MoveToLastLine,
    /// Move up by one viewport height.
    MovePageUp,
    /// Move down by one viewport height.
    MovePageDown,
    /// Move up by half of the viewport height.
    MoveHalfPageUp,
    /// Move down by half of the viewport height.
    MoveHalfPageDown,
    /// Move backward through the current window's jumplist.
    JumpBackward,
    /// Move forward through the current window's jumplist.
    JumpForward,
    /// Move to the top of the screen.
    MoveToScreenTop,
    /// Move to the middle of the screen.
    MoveToScreenMiddle,
    /// Move to the bottom of the screen.
    MoveToScreenBottom,
    /// Scroll the viewport so the cursor line appears at the top.
    ViewportCursorTop,
    /// Scroll the viewport so the cursor line appears at the center.
    ViewportCursorCenter,
    /// Scroll the viewport so the cursor line appears at the bottom.
    ViewportCursorBottom,
    /// Delete the character before the cursor.
    DeleteBackward,
    /// Delete the character under the cursor.
    DeleteForward,
    /// Delete the active visual selection.
    DeleteSelection,
    /// Join the current line with the next line using a space.
    JoinWithSpace,
    /// Join the current line with the next line without inserting a space.
    JoinWithoutSpace,
    /// Delete the current line.
    DeleteLine,
    /// Yank the current line.
    YankLine,
    /// Copy the active visual selection without mutating the buffer.
    YankSelection,
    /// Replace the current line and enter insert mode.
    ChangeLine,
    /// Replace the active visual selection and enter insert mode.
    ChangeSelection,
    /// Select a text object while in visual mode.
    VisualTextObject(TextObject),
    /// Change from the cursor to the end of the line.
    ChangeToLineEnd,
    /// Paste after the cursor.
    PasteAfter,
    /// Paste before the cursor.
    PasteBefore,
    /// Move the cursor after the current character for insert mode.
    AppendAfterCursor,
    /// Move to the end of the current line for insert mode.
    AppendToLineEnd,
    /// Move to the start of the current line for insert mode.
    InsertAtLineStart,
    /// Open a new line below the cursor and enter insert mode.
    OpenLineBelow,
    /// Open a new line above the cursor and enter insert mode.
    OpenLineAbove,
    /// Shift the current line or line range left by one indentation step.
    IndentDecrease,
    /// Shift the current line or line range right by one indentation step.
    IndentIncrease,
    /// Toggle the current line's comment prefix.
    ToggleLineComment,
    /// Switch to the previous tab.
    PreviousTab,
    /// Switch to the next tab.
    NextTab,
    /// Move to the matching bracket for the one under the cursor.
    MoveToMatchingBracket,
    /// Move to the previous paragraph.
    MoveToPreviousParagraph,
    /// Move to the next paragraph.
    MoveToNextParagraph,
    /// Move to the next occurrence of the given character.
    FindForward(char),
    /// Move to the previous occurrence of the given character.
    FindBackward(char),
    /// Move just before the next occurrence of the given character.
    TillForward(char),
    /// Move just after the previous occurrence of the given character.
    TillBackward(char),
    /// Repeat the last successful character search in the forward direction.
    RepeatLastFind,
    /// Repeat the last successful character search in the reverse direction.
    RepeatLastFindReverse,
    /// Repeat the last successful repeatable edit.
    RepeatLastChange,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Save the current buffer or a specific buffer when provided.
    SaveBuffer(Option<BufferId>),
    /// Wrap another action in a repeat count.
    Count(usize, Box<Action>),
    /// Apply an operator to the given target region.
    Operation(Operator, OperatorTarget),
}

impl Action {
    /// Creates a plain action envelope carrying the given intent payload.
    pub fn new(kind: ActionKind) -> Self {
        Self {
            kind: Some(kind),
            from_mode: None,
            to_mode: None,
            register: None,
        }
    }

    /// Creates an action envelope that only carries mode metadata.
    pub fn none() -> Self {
        Self {
            kind: None,
            from_mode: None,
            to_mode: None,
            register: None,
        }
    }

    /// Creates an action that transitions to the provided mode after it succeeds.
    pub fn mode_transition(to_mode: ModeKind) -> Self {
        Self {
            kind: None,
            from_mode: None,
            to_mode: Some(to_mode),
            register: None,
        }
    }

    /// Creates an insert-char action.
    pub fn insert_char(ch: char) -> Self {
        Self::new(ActionKind::InsertChar(ch))
    }

    /// Creates an insert-text action.
    pub fn insert_text(text: String) -> Self {
        Self::new(ActionKind::InsertText(text))
    }

    /// Creates an insert-newline action.
    pub fn insert_newline() -> Self {
        Self::new(ActionKind::InsertNewline)
    }

    /// Creates a motion that moves forward to the given boundary.
    pub fn forward_to(boundary: Boundary) -> Self {
        Self::new(ActionKind::ForwardTo(boundary))
    }

    /// Creates a motion that moves backward to the given boundary.
    pub fn back_to(boundary: Boundary) -> Self {
        Self::new(ActionKind::BackTo(boundary))
    }

    /// Creates a forward-finding motion.
    pub fn find_forward(target: char) -> Self {
        Self::new(ActionKind::FindForward(target))
    }

    /// Creates a backward-finding motion.
    pub fn find_backward(target: char) -> Self {
        Self::new(ActionKind::FindBackward(target))
    }

    /// Creates a forward till motion.
    pub fn till_forward(target: char) -> Self {
        Self::new(ActionKind::TillForward(target))
    }

    /// Creates a backward till motion.
    pub fn till_backward(target: char) -> Self {
        Self::new(ActionKind::TillBackward(target))
    }

    /// Creates a save-buffer action.
    pub fn save_buffer(target: Option<BufferId>) -> Self {
        Self::new(ActionKind::SaveBuffer(target))
    }

    /// Creates a paste-after action.
    pub fn paste_after() -> Self {
        Self::new(ActionKind::PasteAfter)
    }

    /// Creates a paste-before action.
    pub fn paste_before() -> Self {
        Self::new(ActionKind::PasteBefore)
    }

    /// Creates a line-comment toggle action.
    pub fn toggle_line_comment() -> Self {
        Self::new(ActionKind::ToggleLineComment)
    }

    /// Creates a jumplist backward navigation action.
    pub fn jump_backward() -> Self {
        Self::new(ActionKind::JumpBackward)
    }

    /// Creates a jumplist forward navigation action.
    pub fn jump_forward() -> Self {
        Self::new(ActionKind::JumpForward)
    }

    /// Wraps an action in a repeat count.
    pub fn count(count: usize, inner: Box<Action>) -> Self {
        let register = inner.register;
        let from_mode = inner.from_mode;
        let to_mode = inner.to_mode;
        Self {
            kind: Some(ActionKind::Count(count, inner)),
            from_mode,
            to_mode,
            register,
        }
    }

    /// Creates an operator action.
    pub fn operation(operator: Operator, target: OperatorTarget) -> Self {
        Self::new(ActionKind::Operation(operator, target))
    }

    /// Targets a register for this action.
    pub fn with_register(self, register: RegisterName) -> Self {
        Self {
            kind: self.kind,
            from_mode: self.from_mode,
            to_mode: self.to_mode,
            register: Some(register),
        }
    }

    /// Overrides both the source and destination mode metadata.
    pub fn with_mode(self, from_mode: Option<ModeKind>, to_mode: Option<ModeKind>) -> Self {
        Self {
            kind: self.kind,
            from_mode,
            to_mode,
            register: self.register,
        }
    }

    /// Records the mode in which this action was created.
    pub fn with_from_mode(self, from_mode: ModeKind) -> Self {
        Self {
            kind: self.kind,
            from_mode: Some(from_mode),
            to_mode: self.to_mode,
            register: self.register,
        }
    }

    /// Records the mode this action should transition to after it succeeds.
    pub fn with_to_mode(self, to_mode: ModeKind) -> Self {
        Self {
            kind: self.kind,
            from_mode: self.from_mode,
            to_mode: Some(to_mode),
            register: self.register,
        }
    }

    /// Returns the action kind if one is present.
    fn kind_ref(&self) -> Option<&ActionKind> {
        self.kind.as_ref()
    }

    /// Returns true when the action should clear the remembered column.
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(ActionKind::MoveLeft)
                | Some(ActionKind::MoveRight)
                | Some(ActionKind::FocusPaneLeft)
                | Some(ActionKind::FocusPaneRight)
                | Some(ActionKind::ForwardTo(_))
                | Some(ActionKind::BackTo(_))
                | Some(ActionKind::MoveToLineEnd)
                | Some(ActionKind::MoveToLineStart)
                | Some(ActionKind::MoveToLineContentStart)
                | Some(ActionKind::InsertChar(_))
                | Some(ActionKind::InsertText(_))
                | Some(ActionKind::InsertNewline)
                | Some(ActionKind::DeleteBackward)
                | Some(ActionKind::DeleteForward)
                | Some(ActionKind::DeleteSelection)
                | Some(ActionKind::DeleteLine)
                | Some(ActionKind::YankLine)
                | Some(ActionKind::YankSelection)
                | Some(ActionKind::ChangeLine)
                | Some(ActionKind::ChangeSelection)
                | Some(ActionKind::VisualTextObject(_))
                | Some(ActionKind::ChangeToLineEnd)
                | Some(ActionKind::JoinWithSpace)
                | Some(ActionKind::JoinWithoutSpace)
                | Some(ActionKind::IndentDecrease)
                | Some(ActionKind::IndentIncrease)
                | Some(ActionKind::AppendAfterCursor)
                | Some(ActionKind::AppendToLineEnd)
                | Some(ActionKind::InsertAtLineStart)
                | Some(ActionKind::OpenLineBelow)
                | Some(ActionKind::OpenLineAbove)
                | Some(ActionKind::ToggleLineComment)
                | Some(ActionKind::FindForward(_))
                | Some(ActionKind::FindBackward(_))
                | Some(ActionKind::TillForward(_))
                | Some(ActionKind::TillBackward(_))
                | Some(ActionKind::RepeatLastFind)
                | Some(ActionKind::RepeatLastFindReverse)
        )
    }

    /// Returns true when the action should reuse the remembered column.
    pub fn uses_remembered_column(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(ActionKind::MoveUp)
                | Some(ActionKind::MoveDown)
                | Some(ActionKind::FocusPaneUp)
                | Some(ActionKind::FocusPaneDown)
                | Some(ActionKind::MoveToFirstLine)
                | Some(ActionKind::MoveToLastLine)
                | Some(ActionKind::MovePageUp)
                | Some(ActionKind::MovePageDown)
                | Some(ActionKind::MoveHalfPageUp)
                | Some(ActionKind::MoveHalfPageDown)
                | Some(ActionKind::MoveToScreenTop)
                | Some(ActionKind::MoveToScreenMiddle)
                | Some(ActionKind::MoveToScreenBottom)
                | Some(ActionKind::MoveToPreviousParagraph)
                | Some(ActionKind::MoveToNextParagraph)
        )
    }

    /// Returns true when the action can be prefixed with a count.
    pub fn is_countable(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(ActionKind::MoveLeft)
                | Some(ActionKind::MoveRight)
                | Some(ActionKind::MoveUp)
                | Some(ActionKind::MoveDown)
                | Some(ActionKind::FocusPaneLeft)
                | Some(ActionKind::FocusPaneRight)
                | Some(ActionKind::FocusPaneUp)
                | Some(ActionKind::FocusPaneDown)
                | Some(ActionKind::ForwardTo(_))
                | Some(ActionKind::BackTo(_))
                | Some(ActionKind::MoveToFirstLine)
                | Some(ActionKind::MoveToLastLine)
                | Some(ActionKind::MoveToScreenTop)
                | Some(ActionKind::MoveToScreenBottom)
                | Some(ActionKind::JoinWithSpace)
                | Some(ActionKind::JoinWithoutSpace)
                | Some(ActionKind::IndentDecrease)
                | Some(ActionKind::IndentIncrease)
                | Some(ActionKind::DeleteLine)
                | Some(ActionKind::ChangeLine)
                | Some(ActionKind::VisualTextObject(_))
                | Some(ActionKind::ChangeToLineEnd)
                | Some(ActionKind::YankSelection)
                | Some(ActionKind::PasteAfter)
                | Some(ActionKind::PasteBefore)
                | Some(ActionKind::OpenLineBelow)
                | Some(ActionKind::OpenLineAbove)
                | Some(ActionKind::ToggleLineComment)
                | Some(ActionKind::PreviousTab)
                | Some(ActionKind::NextTab)
                | Some(ActionKind::FindForward(_))
                | Some(ActionKind::FindBackward(_))
                | Some(ActionKind::TillForward(_))
                | Some(ActionKind::TillBackward(_))
                | Some(ActionKind::RepeatLastFind)
                | Some(ActionKind::RepeatLastChange)
                | Some(ActionKind::Operation(_, _))
                | Some(ActionKind::RepeatLastFindReverse)
                | Some(ActionKind::MoveToPreviousParagraph)
                | Some(ActionKind::MoveToNextParagraph)
        )
    }

    /// Returns true when the action is line-oriented.
    pub fn is_line_action(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(ActionKind::MoveToLineEnd)
                | Some(ActionKind::MoveToLineStart)
                | Some(ActionKind::MoveToLineContentStart)
                | Some(ActionKind::MoveToFirstLine)
                | Some(ActionKind::MoveToLastLine)
                | Some(ActionKind::YankLine)
                | Some(ActionKind::AppendToLineEnd)
                | Some(ActionKind::InsertAtLineStart)
                | Some(ActionKind::ToggleLineComment)
                | Some(ActionKind::IndentDecrease)
                | Some(ActionKind::IndentIncrease)
                | Some(ActionKind::PreviousTab)
                | Some(ActionKind::NextTab)
                | Some(ActionKind::SplitVertical)
                | Some(ActionKind::SplitHorizontal)
                | Some(ActionKind::ClosePane)
        )
    }

    /// Wraps the action in a count when the action supports it.
    pub fn with_count(self, count: usize) -> Option<Action> {
        if (self.is_countable() || self.is_line_action()) && count > 0 && count < 10000 {
            Some(Action::count(count, Box::new(self)))
        } else {
            None
        }
    }

    /// Returns true when this action transitions to insert mode after it succeeds.
    pub fn switches_to_insert_mode(&self) -> bool {
        self.to_mode == Some(ModeKind::Insert)
    }

    pub fn is_snapshottable(&self) -> bool {
        match self.kind_ref() {
            None => false,
            Some(ActionKind::DeleteBackward)
            | Some(ActionKind::DeleteForward)
            | Some(ActionKind::DeleteSelection)
            | Some(ActionKind::DeleteLine)
            | Some(ActionKind::PasteAfter)
            | Some(ActionKind::PasteBefore)
            | Some(ActionKind::JoinWithSpace)
            | Some(ActionKind::JoinWithoutSpace)
            | Some(ActionKind::IndentDecrease)
            | Some(ActionKind::IndentIncrease)
            | Some(ActionKind::ToggleLineComment) => true,
            Some(ActionKind::InsertChar(_)) => false,
            Some(ActionKind::Undo) | Some(ActionKind::Redo) => false,
            Some(ActionKind::Count(_, inner)) => inner.is_snapshottable(),
            Some(ActionKind::Operation(Operator::Delete, _)) => true,
            Some(ActionKind::Operation(Operator::Change, _)) => false,
            Some(ActionKind::Operation(Operator::Yank, _)) => false,
            Some(ActionKind::Operation(Operator::Lowercase, _))
            | Some(ActionKind::Operation(Operator::Uppercase, _))
            | Some(ActionKind::Operation(Operator::ToggleCase, _)) => true,
            _ => false,
        }
    }

    pub fn updates_snapshot_cursor(&self) -> bool {
        match self.kind_ref() {
            Some(ActionKind::MoveLeft)
            | Some(ActionKind::MoveDown)
            | Some(ActionKind::MoveUp)
            | Some(ActionKind::MoveRight)
            | Some(ActionKind::FocusPaneLeft)
            | Some(ActionKind::FocusPaneDown)
            | Some(ActionKind::FocusPaneUp)
            | Some(ActionKind::FocusPaneRight)
            | Some(ActionKind::ForwardTo(_))
            | Some(ActionKind::BackTo(_))
            | Some(ActionKind::MoveToLineEnd)
            | Some(ActionKind::MoveToLineStart)
            | Some(ActionKind::MoveToLineContentStart)
            | Some(ActionKind::MoveToFirstLine)
            | Some(ActionKind::MoveToLastLine)
            | Some(ActionKind::MovePageUp)
            | Some(ActionKind::MovePageDown)
            | Some(ActionKind::MoveHalfPageUp)
            | Some(ActionKind::MoveHalfPageDown)
            | Some(ActionKind::MoveToScreenTop)
            | Some(ActionKind::MoveToScreenMiddle)
            | Some(ActionKind::MoveToScreenBottom)
            | Some(ActionKind::PasteAfter)
            | Some(ActionKind::PasteBefore)
            | Some(ActionKind::MoveToMatchingBracket)
            | Some(ActionKind::MoveToPreviousParagraph)
            | Some(ActionKind::MoveToNextParagraph)
            | Some(ActionKind::VisualTextObject(_))
            | Some(ActionKind::FindForward(_))
            | Some(ActionKind::FindBackward(_))
            | Some(ActionKind::TillForward(_))
            | Some(ActionKind::TillBackward(_))
            | Some(ActionKind::RepeatLastFind)
            | Some(ActionKind::RepeatLastFindReverse) => true,
            Some(ActionKind::Count(_, inner)) => inner.updates_snapshot_cursor(),
            Some(ActionKind::Operation(
                Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase,
                _,
            )) => false,
            Some(ActionKind::Operation(_, _)) => false,
            _ => false,
        }
    }

    /// Returns true when this action should become the new dot-repeat source after it succeeds.
    pub fn is_dot_repeat_source(&self) -> bool {
        match self.kind_ref() {
            Some(ActionKind::DeleteBackward)
            | Some(ActionKind::DeleteForward)
            | Some(ActionKind::DeleteSelection)
            | Some(ActionKind::JoinWithSpace)
            | Some(ActionKind::JoinWithoutSpace)
            | Some(ActionKind::DeleteLine)
            | Some(ActionKind::ChangeLine)
            | Some(ActionKind::ChangeSelection)
            | Some(ActionKind::VisualTextObject(_))
            | Some(ActionKind::ChangeToLineEnd)
            | Some(ActionKind::PasteAfter)
            | Some(ActionKind::PasteBefore)
            | Some(ActionKind::IndentDecrease)
            | Some(ActionKind::IndentIncrease)
            | Some(ActionKind::AppendAfterCursor)
            | Some(ActionKind::AppendToLineEnd)
            | Some(ActionKind::InsertAtLineStart)
            | Some(ActionKind::OpenLineBelow)
            | Some(ActionKind::OpenLineAbove)
            | Some(ActionKind::ToggleLineComment)
            | Some(ActionKind::Operation(Operator::Delete, _))
            | Some(ActionKind::Operation(Operator::Change, _))
            | Some(ActionKind::Operation(Operator::Lowercase, _))
            | Some(ActionKind::Operation(Operator::Uppercase, _))
            | Some(ActionKind::Operation(Operator::ToggleCase, _)) => true,
            Some(ActionKind::Count(_, inner)) => inner.is_dot_repeat_source(),
            _ => false,
        }
    }

    /// Returns true when this action is the dot-repeat command itself.
    pub fn is_repeat_command(&self) -> bool {
        matches!(self.kind_ref(), Some(ActionKind::RepeatLastChange))
            || matches!(self.kind_ref(), Some(ActionKind::Count(_, inner)) if matches!(inner.kind_ref(), Some(ActionKind::RepeatLastChange)))
    }

    /// Returns the repeat source and count recorded by this action, if it is repeatable.
    pub fn dot_repeat_source(&self) -> Option<(Action, usize)> {
        match self.kind_ref() {
            Some(ActionKind::Count(count, inner)) => {
                let (action, source_count) = inner.dot_repeat_source()?;
                Some((action, count.saturating_mul(source_count)))
            }
            Some(kind) if self.is_dot_repeat_source() => Some((
                Action {
                    kind: Some(kind.clone()),
                    from_mode: self.from_mode,
                    to_mode: self.to_mode,
                    register: self.register,
                },
                1,
            )),
            _ => None,
        }
    }

    /// Resolves this action into the buffer edit that should be replayed for dot repeat.
    pub fn resolve_dot_repeat(&self) -> Option<RepeatReplay> {
        match self.kind_ref() {
            Some(ActionKind::RepeatLastChange) => {
                globals::get_last_repeat().map(|state| RepeatReplay {
                    action: state.action,
                    structural_count: state.count,
                    repeat_count: 1,
                    insert_text: state.insert_text,
                })
            }
            Some(ActionKind::Count(count, inner))
                if matches!(inner.kind_ref(), Some(ActionKind::RepeatLastChange)) =>
            {
                globals::get_last_repeat().map(|state| RepeatReplay {
                    action: state.action,
                    structural_count: state.count,
                    repeat_count: *count,
                    insert_text: state.insert_text,
                })
            }
            _ => None,
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

/// A resolved dot-repeat replay.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatReplay {
    /// The repeatable normal-mode action to replay.
    pub action: Action,
    /// The count to apply to the stored structural action.
    pub structural_count: usize,
    /// The number of times to replay the completed edit for the dot command.
    pub repeat_count: usize,
    /// The committed insert text captured from the original edit, if any.
    pub insert_text: Option<String>,
}
