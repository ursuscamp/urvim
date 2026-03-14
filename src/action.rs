//! Action result module.
//!
//! This module provides the ActionResult enum which indicates whether
//! a widget handled an action or not.

/// Result of a widget processing an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionResult {
    /// The widget handled the action
    Handled,
    /// The widget did not handle the action
    NotHandled,
}
