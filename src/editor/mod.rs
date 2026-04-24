//! Editor module for vim-style modal editing.
//!
//! This module provides the `Mode` trait and implementations for Normal and Insert modes,
//! along with the `Action` enum that represents actions triggered by keypresses.

mod action;
mod insert;
mod keymap;
mod mode;
mod normal;
pub mod pairs;
mod resizing;
mod surround;
mod visual;
mod visual_common;
mod visual_line;

pub use action::{
    Action, ActionKind, BoundaryMotion, BracketKind, DelimiterFamily, HandleKeyResult,
    LinewiseMotion, Operator, OperatorTarget, QuoteKind, RepeatReplay, TextObject,
};
pub use insert::InsertMode;
pub use keymap::{CountParser, KeyStringParseError, Keymap, TrieKeymap, validate_key_string};
pub use mode::{Mode, ModeKind};
pub use normal::NormalMode;
pub use resizing::ResizingMode;
pub use visual::VisualMode;
pub use visual_line::VisualLineMode;

#[cfg(test)]
mod tests;
