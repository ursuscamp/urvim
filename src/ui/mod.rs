//! Internal UI event and intent types.
//!
//! These types provide a unified dispatch envelope that carries either editing
//! actions or UI orchestration commands.

pub mod confirmation_box;
pub mod floating_window;

use crate::editor::Action;
use crate::notification::NotificationLevel;
use crate::terminal::{Event, Key};
use crate::window::{Position, Size};

/// Internal UI event routed between widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// Key event from the terminal input layer.
    Key(Key),
    /// Bracketed paste text.
    Paste(String),
    /// Terminal resize event.
    Resize(u16, u16),
    /// Periodic wake-up event.
    Tick,
}

/// Result of widget-level UI event handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEventResult {
    /// Event handled and optionally emitted follow-up intents.
    Handled(Vec<Intent>),
    /// Event was not handled by this widget.
    NotHandled,
}

impl UiEventResult {
    /// Returns true when this result indicates handled status.
    pub fn handled(&self) -> bool {
        matches!(self, UiEventResult::Handled(_))
    }

    /// Consumes this result and returns emitted intents.
    pub fn into_intents(self) -> Vec<Intent> {
        match self {
            UiEventResult::Handled(intents) => intents,
            UiEventResult::NotHandled => Vec::new(),
        }
    }
}

/// Unified dispatch envelope for editor actions and UI commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Editing-semantic action.
    Action(Action),
    /// UI/app orchestration command.
    Command(Command),
}

impl Intent {
    /// Creates an intent from any action or command payload.
    pub fn new<T: Into<Intent>>(payload: T) -> Self {
        payload.into()
    }

    /// Returns the contained action, if this is an action intent.
    pub fn as_action(&self) -> Option<&Action> {
        match self {
            Intent::Action(action) => Some(action),
            Intent::Command(_) => None,
        }
    }

    /// Returns the contained command, if this is a command intent.
    pub fn as_command(&self) -> Option<&Command> {
        match self {
            Intent::Action(_) => None,
            Intent::Command(command) => Some(command),
        }
    }
}

impl From<Action> for Intent {
    fn from(action: Action) -> Self {
        Intent::Action(action)
    }
}

impl From<Command> for Intent {
    fn from(command: Command) -> Self {
        Intent::Command(command)
    }
}

/// UI/app orchestration command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Enqueue a user-facing notification.
    EnqueueNotification {
        /// Notification level.
        level: NotificationLevel,
        /// Notification message text.
        message: String,
    },
    /// Open the command-line overlay.
    OpenCommandLine,
    /// Shrink the focused pane horizontally by the provided count.
    ResizePaneLeft(usize),
    /// Grow the focused pane horizontally by the provided count.
    ResizePaneRight(usize),
    /// Shrink the focused pane vertically by the provided count.
    ResizePaneUp(usize),
    /// Grow the focused pane vertically by the provided count.
    ResizePaneDown(usize),
    /// Equalize all split ratios in the layout.
    EqualizeSplits,
    /// Toggle visual wrapping for the focused window.
    ToggleWrap,
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
    /// Attempt to exit the editor, allowing the app to confirm first if needed.
    TryQuit,
    /// Exit the editor.
    Quit,
}

/// Widget focus policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    /// Widget does not accept focus.
    Passive,
    /// Widget may receive focus and event routing priority.
    Focusable,
}

/// Widget layout constraints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiConstraints {
    /// Layout origin.
    pub origin: Position,
    /// Available space.
    pub available: Size,
}

impl UiConstraints {
    /// Creates constraints from origin and available size.
    pub fn new(origin: Position, available: Size) -> Self {
        Self { origin, available }
    }
}

/// Rectangle for widget rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRect {
    /// Rectangle origin.
    pub origin: Position,
    /// Rectangle size.
    pub size: Size,
}

impl UiRect {
    /// Creates a new widget rectangle.
    pub fn new(origin: Position, size: Size) -> Self {
        Self { origin, size }
    }
}

impl From<Event> for UiEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::Key(key) => UiEvent::Key(key),
            Event::Paste(text) => UiEvent::Paste(text),
            Event::Resize(rows, cols) => UiEvent::Resize(rows, cols),
            Event::Tick => UiEvent::Tick,
        }
    }
}

/// Shared UI context passed to widget event/render hooks.
#[derive(Debug, Default)]
pub struct UiContext;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_event_result_reports_handled_state() {
        assert!(UiEventResult::Handled(Vec::new()).handled());
        assert!(!UiEventResult::NotHandled.handled());
    }

    #[test]
    fn ui_event_result_extracts_intents() {
        let intents = UiEventResult::Handled(vec![Intent::Command(Command::Quit)]).into_intents();
        assert_eq!(intents.len(), 1);

        let intents = UiEventResult::NotHandled.into_intents();
        assert!(intents.is_empty());
    }

    #[test]
    fn ui_rect_constructor_sets_fields() {
        let rect = UiRect::new(Position::new(1, 2), Size::new(3, 4));
        assert_eq!(rect.origin, Position::new(1, 2));
        assert_eq!(rect.size, Size::new(3, 4));
    }
}
