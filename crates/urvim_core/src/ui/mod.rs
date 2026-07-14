//! Internal UI event and intent types.
//!
//! These types provide a unified dispatch envelope that carries either editing
//! actions or UI orchestration commands.

pub mod completion;
pub mod confirmation_box;
pub mod diagnostic_hover;
pub mod floating_window;
pub mod hover;
pub mod inputs;
pub mod line_format;
pub mod lsp_rename;
pub mod picker;
pub mod plugin_pane;
pub mod plugin_window;
pub mod text_width;

use crate::buffer::BufferId;
use crate::buffer::Cursor;
use crate::editor::EditorAction;
use crate::notification::NotificationLevel;
use crate::window::{Position, Size};
use std::path::PathBuf;
use urvim_terminal::{Event, Key};

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

/// Unified dispatch envelope for editor operations and UI commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// An operation interpreted against an editor window and its modal state.
    Editor(EditorAction),
    /// UI/app orchestration command.
    Command(Command),
}

impl Intent {
    /// Creates an intent from any action or command payload.
    pub fn new<T: Into<Intent>>(payload: T) -> Self {
        payload.into()
    }

    /// Returns the contained editor action, if this is an editor intent.
    pub fn as_editor_action(&self) -> Option<&EditorAction> {
        match self {
            Intent::Editor(action) => Some(action),
            Intent::Command(_) => None,
        }
    }

    /// Returns the contained command, if this is a command intent.
    pub fn as_command(&self) -> Option<&Command> {
        match self {
            Intent::Editor(_) => None,
            Intent::Command(command) => Some(command),
        }
    }
}

impl From<EditorAction> for Intent {
    fn from(action: EditorAction) -> Self {
        Intent::Editor(action)
    }
}

impl From<Command> for Intent {
    fn from(command: Command) -> Self {
        Intent::Command(command)
    }
}

/// Controls where a global key mapping is inherited outside editor input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapInheritance {
    /// Inherited by persistent editor and plugin focus targets.
    Focus,
    /// Inherited by application input contexts that enable global mappings.
    Application,
    /// Available only while an editor handles input.
    Editor,
    /// Never inherited; only explicit local or programmatic invocation is allowed.
    Explicit,
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
    /// Open a new unnamed buffer in a new tab.
    OpenUnnamedBuffer,
    /// Open the insert-mode completion popup.
    OpenCompletion,
    /// Open the visible-buffer picker overlay.
    OpenBufferPicker,
    /// Open the file picker overlay.
    OpenFilePicker,
    /// Open the git picker overlay.
    OpenGitPicker,
    /// Open the colorscheme picker overlay.
    OpenColorschemePicker,
    /// Open the filetype picker overlay.
    OpenFiletypePicker,
    /// Open the active-buffer document symbol picker overlay.
    OpenDocumentSymbolsPicker,
    /// Open the workspace symbol picker overlay.
    OpenWorkspaceSymbolsPicker,
    /// Open the live grep picker overlay.
    OpenGrepPicker,
    /// Write all modified buffers in the pool.
    WriteAll,
    /// Save the current buffer or a specific buffer when provided.
    SaveBuffer(Option<BufferId>),
    /// Save the active buffer or a specific buffer to a new path.
    SaveBufferAs {
        /// Buffer to save, defaulting to the active buffer.
        buffer_id: Option<BufferId>,
        /// New path for the buffer.
        path: PathBuf,
    },
    /// Close a buffer from the current pane.
    CloseBuffer(Option<BufferId>),
    /// Unload a buffer from every pane and the buffer pool.
    UnloadBuffer {
        /// Buffer to unload, defaulting to the active buffer.
        buffer_id: Option<BufferId>,
        /// Whether modified buffers should be unloaded.
        force: bool,
    },
    /// Overwrite a file that changed on disk.
    OverwriteBuffer(Option<crate::buffer::BufferId>),
    /// Send a request to a process-backed plugin command.
    PluginRequest {
        /// Plugin namespace.
        plugin: String,
        /// Plugin command or script name.
        command: String,
        /// Raw command arguments.
        args: Vec<String>,
    },
    /// Show loaded plugin runtime statuses.
    PluginStatus,
    /// Run an LSP hover query on the active buffer.
    LspHover,
    /// Run an LSP go-to-definition query on the active buffer.
    LspDefinition,
    /// Open a picker for references to the symbol under the cursor.
    LspReferences,
    /// Jump to the previous diagnostic in the active buffer.
    LspPreviousDiagnostic,
    /// Jump to the next diagnostic in the active buffer.
    LspNextDiagnostic,
    /// Jump to the previous error diagnostic in the active buffer.
    LspPreviousErrorDiagnostic,
    /// Jump to the next error diagnostic in the active buffer.
    LspNextErrorDiagnostic,
    /// Open the rename prompt for an LSP rename.
    LspRenamePrompt,
    /// Run an LSP rename with the provided replacement name.
    LspRename(String),
    /// Apply a selected completion replacement.
    ApplyCompletion(ApplyCompletion),
    /// Open a picker for available LSP code actions.
    LspCodeActions,
    /// Apply a selected LSP code action.
    LspApplyCodeAction {
        /// Buffer that owns the action.
        buffer_id: crate::buffer::BufferId,
        /// Code-action payload to apply.
        action: crate::lsp::runtime::CodeActionApplication,
    },
    /// Open a file selected from a picker.
    OpenFile(PathBuf),
    /// Open a file and place the cursor at the provided position.
    OpenFileAtCursor(PathBuf, Cursor),
    /// Focus the first pane showing the selected buffer.
    FocusBuffer(BufferId),
    /// Set the filetype for a buffer, defaulting to the active buffer.
    SetBufferFiletype(Option<BufferId>, String),
    /// Toggle staging for a git picker selection.
    GitPickerToggleStage(crate::ui::picker::git::GitPickerAction),
    /// Request discarding a git picker selection.
    GitPickerDiscard(crate::ui::picker::git::GitPickerAction),
    /// Confirm discarding a git picker selection.
    GitPickerDiscardConfirmed(crate::ui::picker::git::GitPickerAction),
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
    /// Switch backward through editor tabs by the provided count.
    PreviousTab(usize),
    /// Switch forward through editor tabs by the provided count.
    NextTab(usize),
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
    /// Focus the next persistent editor or plugin window.
    FocusNextWindow,
    /// Focus the previous persistent editor or plugin window.
    FocusPreviousWindow,
    /// Close the focused pane.
    ClosePane,
    /// Attempt to exit the editor, allowing the app to confirm first if needed.
    TryQuit,
    /// Exit the editor.
    Quit,
}

impl Command {
    /// Returns where a global mapping for this command may be inherited.
    pub fn keymap_inheritance(&self) -> KeymapInheritance {
        match self {
            Self::ResizePaneLeft(_)
            | Self::ResizePaneRight(_)
            | Self::ResizePaneUp(_)
            | Self::ResizePaneDown(_)
            | Self::EqualizeSplits
            | Self::FocusPaneLeft
            | Self::FocusPaneDown
            | Self::FocusPaneUp
            | Self::FocusPaneRight
            | Self::FocusNextWindow
            | Self::FocusPreviousWindow
            | Self::ClosePane => KeymapInheritance::Focus,
            Self::OpenCommandLine
            | Self::OpenBufferPicker
            | Self::OpenFilePicker
            | Self::OpenGitPicker
            | Self::OpenColorschemePicker
            | Self::OpenWorkspaceSymbolsPicker
            | Self::OpenGrepPicker
            | Self::WriteAll
            | Self::PluginRequest { .. }
            | Self::PluginStatus
            | Self::TryQuit
            | Self::Quit => KeymapInheritance::Application,
            Self::OpenUnnamedBuffer
            | Self::OpenCompletion
            | Self::OpenFiletypePicker
            | Self::OpenDocumentSymbolsPicker
            | Self::SaveBuffer(_)
            | Self::SaveBufferAs { .. }
            | Self::CloseBuffer(_)
            | Self::UnloadBuffer { .. }
            | Self::LspHover
            | Self::LspDefinition
            | Self::LspReferences
            | Self::LspPreviousDiagnostic
            | Self::LspNextDiagnostic
            | Self::LspPreviousErrorDiagnostic
            | Self::LspNextErrorDiagnostic
            | Self::LspRenamePrompt
            | Self::LspCodeActions
            | Self::PreviousTab(_)
            | Self::NextTab(_)
            | Self::ToggleWrap
            | Self::SplitVertical
            | Self::SplitHorizontal => KeymapInheritance::Editor,
            Self::EnqueueNotification { .. }
            | Self::OverwriteBuffer(_)
            | Self::LspRename(_)
            | Self::ApplyCompletion(_)
            | Self::LspApplyCodeAction { .. }
            | Self::OpenFile(_)
            | Self::OpenFileAtCursor(_, _)
            | Self::FocusBuffer(_)
            | Self::SetBufferFiletype(_, _)
            | Self::GitPickerToggleStage(_)
            | Self::GitPickerDiscard(_)
            | Self::GitPickerDiscardConfirmed(_) => KeymapInheritance::Explicit,
        }
    }
}

impl Intent {
    /// Returns where a global mapping for this intent may be inherited.
    pub fn keymap_inheritance(&self) -> KeymapInheritance {
        match self {
            Self::Editor(_) => KeymapInheritance::Editor,
            Self::Command(command) => command.keymap_inheritance(),
        }
    }
}

/// Payload for applying a selected completion replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyCompletion {
    /// Buffer range to replace.
    pub range: crate::buffer::TextObjectRange,
    /// Replacement text.
    pub text: String,
    /// Extra edits to apply when the completion is accepted.
    pub additional_text_edits: Vec<crate::ui::completion::CompletionTextEdit>,
    /// Opaque serialized LSP completion item for resolve requests.
    pub lsp_completion_item: Option<serde_json::Value>,
    /// Replacement text insertion format.
    pub format: crate::ui::completion::CompletionInsertFormat,
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

    #[test]
    fn keymap_inheritance_distinguishes_editor_focus_and_application_intents() {
        assert_eq!(
            Command::FocusPaneLeft.keymap_inheritance(),
            KeymapInheritance::Focus
        );
        assert_eq!(
            Command::OpenFilePicker.keymap_inheritance(),
            KeymapInheritance::Application
        );
        assert_eq!(
            Command::LspDefinition.keymap_inheritance(),
            KeymapInheritance::Editor
        );
        assert_eq!(
            Command::EnqueueNotification {
                level: NotificationLevel::Info,
                message: String::new(),
            }
            .keymap_inheritance(),
            KeymapInheritance::Explicit
        );
    }

    #[test]
    fn editor_intents_are_editor_only() {
        let intent = Intent::Editor(crate::editor::EditorAction::new(
            crate::editor::EditorOperation::MoveDown,
        ));
        assert_eq!(intent.keymap_inheritance(), KeymapInheritance::Editor);
    }
}
