//! Widget module.
//!
//! This module provides the Widget trait for widgets that can process actions.

use crate::action::ActionResult;
use crate::editor::Action;

/// Trait for widgets that can process actions.
///
/// Widgets are UI components (window, status bar, etc.) that can
/// handle user actions. The main event loop passes actions to widgets
/// first, and if no widget handles them, processes them at the app level.
pub trait Widget {
    /// Process an action and return whether it was handled.
    ///
    /// # Arguments
    /// * `action` - The action to process
    ///
    /// # Returns
    /// * `ActionResult::Handled` - Widget handled the action
    /// * `ActionResult::NotHandled` - Widget did not handle the action
    fn process_action(&mut self, action: &Action) -> ActionResult;
}
