//! Editor module for vim-style modal editing.
//!
//! This module provides the `Mode` trait and implementations for Normal and Insert modes,
//! along with the `EditorAction` enum that represents actions triggered by keypresses.

mod action;
mod insert;
mod keymap;
mod mode;
mod normal;
pub mod pairs;
mod replace;
mod resizing;
mod surround;
mod text_object;
mod visual;
mod visual_common;
mod visual_line;

pub use action::{
    BoundaryMotion, BracketKind, DelimiterFamily, EditorAction, EditorOperation, HandleKeyResult,
    LinewiseMotion, Operator, OperatorTarget, QuoteKind, RepeatReplay, TextObject,
};
pub use insert::InsertMode;
pub use keymap::{
    CountParser, InheritedKeymap, KeyStringParseError, Keymap, TrieKeymap, validate_key_string,
};
pub use mode::{Mode, ModeKind};
pub use normal::NormalMode;
pub use replace::ReplaceMode;
pub use resizing::ResizingMode;
pub use visual::VisualMode;
pub use visual_line::VisualLineMode;

#[cfg(test)]
mod tests;
