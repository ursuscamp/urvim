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
mod visual;

pub use action::{
    Action, ActionKind, BoundaryMotion, BracketKind, HandleKeyResult, LinewiseMotion, Operator,
    OperatorTarget, QuoteKind, RepeatReplay, TextObject,
};
pub use insert::InsertMode;
pub use keymap::{CountParser, KeyStringParseError, Keymap, TrieKeymap, validate_key_string};
pub use mode::{Mode, ModeKind};
pub use normal::NormalMode;
pub use visual::VisualMode;

#[cfg(test)]
mod tests;
