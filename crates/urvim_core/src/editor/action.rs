use crate::buffer::{Boundary, Cursor};
use crate::editor::ModeKind;
use crate::globals;
use crate::register::RegisterName;
use crate::ui::{Command, Intent};

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

/// Supported delimiter families for surround manipulation commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelimiterFamily {
    /// Parenthesis pairs `(` and `)`.
    Paren,
    /// Square bracket pairs `[` and `]`.
    Square,
    /// Curly brace pairs `{` and `}`.
    Curly,
    /// Angle bracket pairs `<` and `>`.
    Angle,
    /// Double quote delimiters (`"`).
    DoubleQuote,
    /// Single quote delimiters (`'`).
    SingleQuote,
    /// Backtick delimiters (`` ` ``).
    Backtick,
}

impl DelimiterFamily {
    /// Resolves a canonical key token to a surround delimiter family.
    ///
    /// Bracket families accept both opening and closing selector keys.
    pub fn from_selector_key(key: &str) -> Option<Self> {
        match key {
            "(" | ")" => Some(Self::Paren),
            "[" | "]" => Some(Self::Square),
            "{" | "}" => Some(Self::Curly),
            "<LessThan>" | "<GreaterThan>" => Some(Self::Angle),
            "\"" => Some(Self::DoubleQuote),
            "'" => Some(Self::SingleQuote),
            "`" => Some(Self::Backtick),
            _ => None,
        }
    }

    /// Returns the opening delimiter character for this family.
    pub fn opening_delimiter(self) -> char {
        match self {
            Self::Paren => '(',
            Self::Square => '[',
            Self::Curly => '{',
            Self::Angle => '<',
            Self::DoubleQuote => '"',
            Self::SingleQuote => '\'',
            Self::Backtick => '`',
        }
    }

    /// Returns the closing delimiter character for this family.
    pub fn closing_delimiter(self) -> char {
        match self {
            Self::Paren => ')',
            Self::Square => ']',
            Self::Curly => '}',
            Self::Angle => '>',
            Self::DoubleQuote => '"',
            Self::SingleQuote => '\'',
            Self::Backtick => '`',
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

/// Editor-specific operation with modal metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorAction {
    pub kind: Option<EditorOperation>,
    pub from_mode: Option<ModeKind>,
    pub to_mode: Option<ModeKind>,
    pub register: Option<RegisterName>,
}

/// Operation interpreted against an editor window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorOperation {
    /// Move the cursor one character to the left.
    MoveLeft,
    /// Move the cursor one character down.
    MoveDown,
    /// Move the cursor one character up.
    MoveUp,
    /// Move the cursor one character to the right.
    MoveRight,
    /// Insert a single character at the cursor.
    InsertChar(char),
    /// Insert a string at the cursor.
    InsertText(String),
    /// Insert raw paste text at the cursor without helper transforms.
    InsertRawPaste(String),
    /// Replace the active visual selection with raw paste text.
    ReplaceSelectionRawPaste(String),
    /// Insert a newline, letting the window decide whether to auto-indent it.
    InsertNewline,
    /// Exit the editor.
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
    /// Toggle the fold containing the cursor.
    ToggleFold,
    /// Open the fold containing the cursor.
    OpenFold,
    /// Close the fold containing the cursor.
    CloseFold,
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
    /// Move to the matching bracket for the one under the cursor.
    MoveToMatchingBracket,
    /// Move to the previous paragraph.
    MoveToPreviousParagraph,
    /// Move to the next paragraph.
    MoveToNextParagraph,
    /// Move to the previous diff hunk.
    MoveToPreviousDiffHunk,
    /// Move to the next diff hunk.
    MoveToNextDiffHunk,
    /// Move to the previous diff hunk end.
    MoveToPreviousDiffHunkEnd,
    /// Move to the next diff hunk end.
    MoveToNextDiffHunkEnd,
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
    /// Open the command-line overlay in normal mode.
    /// Wrap another action in a repeat count.
    Count(usize, Box<EditorAction>),
    /// Apply an operator to the given target region.
    Operation(Operator, OperatorTarget),
    /// Replace a surrounding delimiter pair around the cursor.
    SurroundReplace {
        target: DelimiterFamily,
        replacement: DelimiterFamily,
    },
    /// Delete a surrounding delimiter pair around the cursor.
    SurroundDelete { target: DelimiterFamily },
    /// Add a surrounding delimiter pair around a text object.
    SurroundAdd {
        target: TextObject,
        delimiter: DelimiterFamily,
    },
    /// Add a surrounding delimiter pair around the active visual selection.
    SurroundAddSelection { delimiter: DelimiterFamily },
    /// Replace a single character under the cursor with the given character.
    ReplaceChar(char),
    /// Restore the previous character replaced in the current replace-mode session.
    ReplaceBackspaceLast,
    /// Undo the last character inserted while in replace mode.
    ReplaceBackspace {
        cursor: Cursor,
        replaced: Option<char>,
        inserted: char,
    },
}

impl EditorAction {
    /// Creates a plain action envelope carrying the given intent payload.
    pub fn new(kind: EditorOperation) -> Self {
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
        Self::new(EditorOperation::InsertChar(ch))
    }

    /// Creates an insert-text action.
    pub fn insert_text(text: String) -> Self {
        Self::new(EditorOperation::InsertText(text))
    }

    /// Creates a raw-paste insertion action.
    pub fn insert_raw_paste(text: String) -> Self {
        Self::new(EditorOperation::InsertRawPaste(text))
    }

    /// Creates a raw-paste visual replacement action.
    pub fn replace_selection_raw_paste(text: String) -> Self {
        Self::new(EditorOperation::ReplaceSelectionRawPaste(text))
    }

    /// Creates an insert-newline action.
    pub fn insert_newline() -> Self {
        Self::new(EditorOperation::InsertNewline)
    }

    /// Creates a motion that moves forward to the given boundary.
    pub fn forward_to(boundary: Boundary) -> Self {
        Self::new(EditorOperation::ForwardTo(boundary))
    }

    /// Creates a motion that moves backward to the given boundary.
    pub fn back_to(boundary: Boundary) -> Self {
        Self::new(EditorOperation::BackTo(boundary))
    }

    /// Creates a forward-finding motion.
    pub fn find_forward(target: char) -> Self {
        Self::new(EditorOperation::FindForward(target))
    }

    /// Creates a backward-finding motion.
    pub fn find_backward(target: char) -> Self {
        Self::new(EditorOperation::FindBackward(target))
    }

    /// Creates a forward till motion.
    pub fn till_forward(target: char) -> Self {
        Self::new(EditorOperation::TillForward(target))
    }

    /// Creates a backward till motion.
    pub fn till_backward(target: char) -> Self {
        Self::new(EditorOperation::TillBackward(target))
    }

    /// Creates a paste-after action.
    pub fn paste_after() -> Self {
        Self::new(EditorOperation::PasteAfter)
    }

    /// Creates a paste-before action.
    pub fn paste_before() -> Self {
        Self::new(EditorOperation::PasteBefore)
    }

    /// Creates a line-comment toggle action.
    pub fn toggle_line_comment() -> Self {
        Self::new(EditorOperation::ToggleLineComment)
    }

    /// Creates a jumplist backward navigation action.
    pub fn jump_backward() -> Self {
        Self::new(EditorOperation::JumpBackward)
    }

    /// Creates a jumplist forward navigation action.
    pub fn jump_forward() -> Self {
        Self::new(EditorOperation::JumpForward)
    }

    /// Wraps an action in a repeat count.
    pub fn count(count: usize, inner: Box<EditorAction>) -> Self {
        let register = inner.register;
        let from_mode = inner.from_mode;
        let to_mode = inner.to_mode;
        Self {
            kind: Some(EditorOperation::Count(count, inner)),
            from_mode,
            to_mode,
            register,
        }
    }

    /// Creates an operator action.
    pub fn operation(operator: Operator, target: OperatorTarget) -> Self {
        Self::new(EditorOperation::Operation(operator, target))
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
    fn kind_ref(&self) -> Option<&EditorOperation> {
        self.kind.as_ref()
    }

    /// Returns true when the action should clear the remembered column.
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(EditorOperation::MoveLeft)
                | Some(EditorOperation::MoveRight)
                | Some(EditorOperation::ForwardTo(_))
                | Some(EditorOperation::BackTo(_))
                | Some(EditorOperation::MoveToLineEnd)
                | Some(EditorOperation::MoveToLineStart)
                | Some(EditorOperation::MoveToLineContentStart)
                | Some(EditorOperation::InsertChar(_))
                | Some(EditorOperation::InsertText(_))
                | Some(EditorOperation::InsertRawPaste(_))
                | Some(EditorOperation::ReplaceSelectionRawPaste(_))
                | Some(EditorOperation::InsertNewline)
                | Some(EditorOperation::DeleteBackward)
                | Some(EditorOperation::DeleteForward)
                | Some(EditorOperation::DeleteSelection)
                | Some(EditorOperation::DeleteLine)
                | Some(EditorOperation::YankLine)
                | Some(EditorOperation::YankSelection)
                | Some(EditorOperation::ChangeLine)
                | Some(EditorOperation::ChangeSelection)
                | Some(EditorOperation::VisualTextObject(_))
                | Some(EditorOperation::ChangeToLineEnd)
                | Some(EditorOperation::JoinWithSpace)
                | Some(EditorOperation::JoinWithoutSpace)
                | Some(EditorOperation::IndentDecrease)
                | Some(EditorOperation::IndentIncrease)
                | Some(EditorOperation::AppendAfterCursor)
                | Some(EditorOperation::AppendToLineEnd)
                | Some(EditorOperation::InsertAtLineStart)
                | Some(EditorOperation::OpenLineBelow)
                | Some(EditorOperation::OpenLineAbove)
                | Some(EditorOperation::ToggleLineComment)
                | Some(EditorOperation::FindForward(_))
                | Some(EditorOperation::FindBackward(_))
                | Some(EditorOperation::TillForward(_))
                | Some(EditorOperation::TillBackward(_))
                | Some(EditorOperation::RepeatLastFind)
                | Some(EditorOperation::RepeatLastFindReverse)
                | Some(EditorOperation::ToggleFold)
                | Some(EditorOperation::OpenFold)
                | Some(EditorOperation::CloseFold)
                | Some(EditorOperation::SurroundReplace { .. })
                | Some(EditorOperation::SurroundDelete { .. })
                | Some(EditorOperation::SurroundAdd { .. })
                | Some(EditorOperation::SurroundAddSelection { .. })
                | Some(EditorOperation::ReplaceChar(_))
                | Some(EditorOperation::ReplaceBackspace { .. })
        )
    }

    /// Returns true when the action should reuse the remembered column.
    pub fn uses_remembered_column(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(EditorOperation::MoveUp)
                | Some(EditorOperation::MoveDown)
                | Some(EditorOperation::MoveToFirstLine)
                | Some(EditorOperation::MoveToLastLine)
                | Some(EditorOperation::MovePageUp)
                | Some(EditorOperation::MovePageDown)
                | Some(EditorOperation::MoveHalfPageUp)
                | Some(EditorOperation::MoveHalfPageDown)
                | Some(EditorOperation::MoveToScreenTop)
                | Some(EditorOperation::MoveToScreenMiddle)
                | Some(EditorOperation::MoveToScreenBottom)
                | Some(EditorOperation::MoveToPreviousParagraph)
                | Some(EditorOperation::MoveToNextParagraph)
                | Some(EditorOperation::MoveToPreviousDiffHunk)
                | Some(EditorOperation::MoveToNextDiffHunk)
                | Some(EditorOperation::MoveToPreviousDiffHunkEnd)
                | Some(EditorOperation::MoveToNextDiffHunkEnd)
        )
    }

    /// Returns true when the action can be prefixed with a count.
    pub fn is_countable(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(EditorOperation::MoveLeft)
                | Some(EditorOperation::MoveRight)
                | Some(EditorOperation::MoveUp)
                | Some(EditorOperation::MoveDown)
                | Some(EditorOperation::ForwardTo(_))
                | Some(EditorOperation::BackTo(_))
                | Some(EditorOperation::MoveToFirstLine)
                | Some(EditorOperation::MoveToLastLine)
                | Some(EditorOperation::MoveToScreenTop)
                | Some(EditorOperation::MoveToScreenBottom)
                | Some(EditorOperation::JoinWithSpace)
                | Some(EditorOperation::JoinWithoutSpace)
                | Some(EditorOperation::IndentDecrease)
                | Some(EditorOperation::IndentIncrease)
                | Some(EditorOperation::DeleteLine)
                | Some(EditorOperation::ChangeLine)
                | Some(EditorOperation::VisualTextObject(_))
                | Some(EditorOperation::ChangeToLineEnd)
                | Some(EditorOperation::YankSelection)
                | Some(EditorOperation::PasteAfter)
                | Some(EditorOperation::PasteBefore)
                | Some(EditorOperation::OpenLineBelow)
                | Some(EditorOperation::OpenLineAbove)
                | Some(EditorOperation::ToggleLineComment)
                | Some(EditorOperation::FindForward(_))
                | Some(EditorOperation::FindBackward(_))
                | Some(EditorOperation::TillForward(_))
                | Some(EditorOperation::TillBackward(_))
                | Some(EditorOperation::RepeatLastFind)
                | Some(EditorOperation::RepeatLastChange)
                | Some(EditorOperation::Operation(_, _))
                | Some(EditorOperation::RepeatLastFindReverse)
                | Some(EditorOperation::MoveToPreviousParagraph)
                | Some(EditorOperation::MoveToNextParagraph)
                | Some(EditorOperation::ReplaceChar(_))
        )
    }

    /// Returns true when the action is line-oriented.
    pub fn is_line_action(&self) -> bool {
        matches!(
            self.kind_ref(),
            Some(EditorOperation::MoveToLineEnd)
                | Some(EditorOperation::MoveToLineStart)
                | Some(EditorOperation::MoveToLineContentStart)
                | Some(EditorOperation::MoveToFirstLine)
                | Some(EditorOperation::MoveToLastLine)
                | Some(EditorOperation::YankLine)
                | Some(EditorOperation::AppendToLineEnd)
                | Some(EditorOperation::InsertAtLineStart)
                | Some(EditorOperation::ToggleLineComment)
                | Some(EditorOperation::IndentDecrease)
                | Some(EditorOperation::IndentIncrease)
        )
    }

    /// Wraps the action in a count when the action supports it.
    pub fn with_count(self, count: usize) -> Option<EditorAction> {
        if (self.is_countable() || self.is_line_action()) && count > 0 && count < 10000 {
            Some(EditorAction::count(count, Box::new(self)))
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
            Some(EditorOperation::DeleteBackward)
            | Some(EditorOperation::DeleteForward)
            | Some(EditorOperation::DeleteSelection)
            | Some(EditorOperation::DeleteLine)
            | Some(EditorOperation::PasteAfter)
            | Some(EditorOperation::PasteBefore)
            | Some(EditorOperation::InsertRawPaste(_))
            | Some(EditorOperation::ReplaceSelectionRawPaste(_))
            | Some(EditorOperation::JoinWithSpace)
            | Some(EditorOperation::JoinWithoutSpace)
            | Some(EditorOperation::IndentDecrease)
            | Some(EditorOperation::IndentIncrease)
            | Some(EditorOperation::ToggleLineComment) => true,
            Some(EditorOperation::InsertChar(_)) => false,
            Some(EditorOperation::Undo) | Some(EditorOperation::Redo) => false,
            Some(EditorOperation::Count(_, inner)) => inner.is_snapshottable(),
            Some(EditorOperation::Operation(Operator::Delete, _)) => true,
            Some(EditorOperation::Operation(Operator::Change, _)) => false,
            Some(EditorOperation::Operation(Operator::Yank, _)) => false,
            Some(EditorOperation::ReplaceChar(_)) => self.from_mode != Some(ModeKind::Replace),
            Some(
                EditorOperation::ReplaceBackspaceLast | EditorOperation::ReplaceBackspace { .. },
            ) => false,
            Some(EditorOperation::SurroundReplace { .. })
            | Some(EditorOperation::SurroundDelete { .. })
            | Some(EditorOperation::SurroundAdd { .. })
            | Some(EditorOperation::SurroundAddSelection { .. }) => true,
            Some(EditorOperation::Operation(Operator::Lowercase, _))
            | Some(EditorOperation::Operation(Operator::Uppercase, _))
            | Some(EditorOperation::Operation(Operator::ToggleCase, _)) => true,
            _ => false,
        }
    }

    pub fn updates_snapshot_cursor(&self) -> bool {
        match self.kind_ref() {
            Some(EditorOperation::MoveLeft)
            | Some(EditorOperation::MoveDown)
            | Some(EditorOperation::MoveUp)
            | Some(EditorOperation::MoveRight)
            | Some(EditorOperation::ForwardTo(_))
            | Some(EditorOperation::BackTo(_))
            | Some(EditorOperation::MoveToLineEnd)
            | Some(EditorOperation::MoveToLineStart)
            | Some(EditorOperation::MoveToLineContentStart)
            | Some(EditorOperation::MoveToFirstLine)
            | Some(EditorOperation::MoveToLastLine)
            | Some(EditorOperation::MovePageUp)
            | Some(EditorOperation::MovePageDown)
            | Some(EditorOperation::MoveHalfPageUp)
            | Some(EditorOperation::MoveHalfPageDown)
            | Some(EditorOperation::MoveToScreenTop)
            | Some(EditorOperation::MoveToScreenMiddle)
            | Some(EditorOperation::MoveToScreenBottom)
            | Some(EditorOperation::PasteAfter)
            | Some(EditorOperation::PasteBefore)
            | Some(EditorOperation::MoveToMatchingBracket)
            | Some(EditorOperation::MoveToPreviousParagraph)
            | Some(EditorOperation::MoveToNextParagraph)
            | Some(EditorOperation::VisualTextObject(_))
            | Some(EditorOperation::FindForward(_))
            | Some(EditorOperation::FindBackward(_))
            | Some(EditorOperation::TillForward(_))
            | Some(EditorOperation::TillBackward(_))
            | Some(EditorOperation::RepeatLastFind)
            | Some(EditorOperation::RepeatLastFindReverse) => true,
            Some(EditorOperation::Count(_, inner)) => inner.updates_snapshot_cursor(),
            Some(EditorOperation::Operation(
                Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase,
                _,
            )) => false,
            Some(EditorOperation::Operation(_, _)) => false,
            _ => false,
        }
    }

    /// Returns true when this action should become the new dot-repeat source after it succeeds.
    pub fn is_dot_repeat_source(&self) -> bool {
        match self.kind_ref() {
            Some(EditorOperation::DeleteBackward)
            | Some(EditorOperation::DeleteForward)
            | Some(EditorOperation::DeleteSelection)
            | Some(EditorOperation::JoinWithSpace)
            | Some(EditorOperation::JoinWithoutSpace)
            | Some(EditorOperation::DeleteLine)
            | Some(EditorOperation::ChangeLine)
            | Some(EditorOperation::ChangeSelection)
            | Some(EditorOperation::VisualTextObject(_))
            | Some(EditorOperation::ChangeToLineEnd)
            | Some(EditorOperation::InsertRawPaste(_))
            | Some(EditorOperation::ReplaceSelectionRawPaste(_))
            | Some(EditorOperation::PasteAfter)
            | Some(EditorOperation::PasteBefore)
            | Some(EditorOperation::IndentDecrease)
            | Some(EditorOperation::IndentIncrease)
            | Some(EditorOperation::AppendAfterCursor)
            | Some(EditorOperation::AppendToLineEnd)
            | Some(EditorOperation::InsertAtLineStart)
            | Some(EditorOperation::OpenLineBelow)
            | Some(EditorOperation::OpenLineAbove)
            | Some(EditorOperation::ToggleLineComment)
            | Some(EditorOperation::Operation(Operator::Delete, _))
            | Some(EditorOperation::Operation(Operator::Change, _))
            | Some(EditorOperation::Operation(Operator::Lowercase, _))
            | Some(EditorOperation::Operation(Operator::Uppercase, _))
            | Some(EditorOperation::Operation(Operator::ToggleCase, _))
            | Some(EditorOperation::ReplaceChar(_))
            | Some(EditorOperation::ReplaceBackspaceLast)
            | Some(EditorOperation::ReplaceBackspace { .. })
            | Some(EditorOperation::SurroundReplace { .. })
            | Some(EditorOperation::SurroundDelete { .. })
            | Some(EditorOperation::SurroundAdd { .. })
            | Some(EditorOperation::SurroundAddSelection { .. }) => true,
            Some(EditorOperation::Count(_, inner)) => inner.is_dot_repeat_source(),
            _ => false,
        }
    }

    /// Returns true when this action is the dot-repeat command itself.
    pub fn is_repeat_command(&self) -> bool {
        matches!(self.kind_ref(), Some(EditorOperation::RepeatLastChange))
            || matches!(self.kind_ref(), Some(EditorOperation::Count(_, inner)) if matches!(inner.kind_ref(), Some(EditorOperation::RepeatLastChange)))
    }

    /// Returns the repeat source and count recorded by this action, if it is repeatable.
    pub fn dot_repeat_source(&self) -> Option<(EditorAction, usize)> {
        match self.kind_ref() {
            Some(EditorOperation::Count(count, inner)) => {
                let (action, source_count) = inner.dot_repeat_source()?;
                Some((action, count.saturating_mul(source_count)))
            }
            Some(kind) if self.is_dot_repeat_source() => Some((
                EditorAction {
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
            Some(EditorOperation::RepeatLastChange) => {
                globals::get_last_repeat().map(|state| RepeatReplay {
                    action: state.action,
                    structural_count: state.count,
                    repeat_count: 1,
                    insert_text: state.insert_text,
                })
            }
            Some(EditorOperation::Count(count, inner))
                if matches!(inner.kind_ref(), Some(EditorOperation::RepeatLastChange)) =>
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
    Complete(Intent),
    WaitForMore,
    InvalidSequence,
}

impl HandleKeyResult {
    /// Creates a completed key handling result from any action or command payload.
    pub fn complete<T: Into<Intent>>(payload: T) -> Self {
        HandleKeyResult::Complete(payload.into())
    }
}

impl From<EditorAction> for HandleKeyResult {
    fn from(action: EditorAction) -> Self {
        HandleKeyResult::Complete(Intent::from(action))
    }
}

impl From<Command> for HandleKeyResult {
    fn from(command: Command) -> Self {
        HandleKeyResult::Complete(Intent::from(command))
    }
}

impl From<Intent> for HandleKeyResult {
    fn from(intent: Intent) -> Self {
        HandleKeyResult::Complete(intent)
    }
}

/// A resolved dot-repeat replay.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatReplay {
    /// The repeatable normal-mode action to replay.
    pub action: EditorAction,
    /// The count to apply to the stored structural action.
    pub structural_count: usize,
    /// The number of times to replay the completed edit for the dot command.
    pub repeat_count: usize,
    /// The committed insert text captured from the original edit, if any.
    pub insert_text: Option<String>,
}
