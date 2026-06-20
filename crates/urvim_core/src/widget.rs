//! Widget module.
//!
//! This module provides the `Widget` trait for UI components that participate
//! in action handling, event routing, layout, and rendering.

use crate::screen::Screen;
use crate::ui::{FocusPolicy, UiConstraints, UiContext, UiEvent, UiEventResult, UiRect};
use crate::window::Size;

/// Trait for UI widgets.
///
/// Widgets are UI components (window, status bar overlays, pane containers,
/// and future floating components) that can participate in optional UI event
/// routing and rendering lifecycle hooks.
pub trait Widget {
    /// Handles an internal UI event.
    ///
    /// Default behavior does not handle the event.
    fn handle_ui_event(&mut self, _event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        UiEventResult::NotHandled
    }

    /// Computes this widget's desired layout size under constraints.
    ///
    /// Default behavior uses all available space.
    fn layout(&mut self, constraints: UiConstraints) -> Size {
        constraints.available
    }

    /// Renders this widget into the provided rectangle.
    ///
    /// Default behavior is a no-op.
    fn render_widget(&mut self, _screen: &mut Screen, _rect: UiRect, _ctx: &UiContext) {}

    /// Returns the widget's focus policy.
    ///
    /// Default behavior is passive.
    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}
