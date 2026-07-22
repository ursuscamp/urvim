//! Internal UI event and intent types.
//!
//! These types provide a unified dispatch envelope that carries either editing
//! actions or UI orchestration commands.

pub mod completion;
pub mod confirmation_box;
pub mod diagnostic_hover;
pub mod geometry;
pub mod hover;
pub mod input_box;
pub mod inputs;
pub mod key_guide;
pub mod line_format;
pub mod lsp_rename;
pub mod overlay;
pub mod picker;
pub mod plugin_pane;
pub mod text_width;

use crate::buffer::BufferId;
use crate::buffer::Cursor;
use crate::editor::EditorAction;
use crate::notification::NotificationLevel;
pub use geometry::{Position, Size};
use std::borrow::Cow;
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
    /// An operation interpreted against an editor tab and its modal state.
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
    /// Resolve a plugin picker with the selected opaque item.
    PluginPickerSelect {
        /// Plugin that owns the picker.
        plugin: String,
        /// Picker instance identity.
        picker_id: crate::ui::picker::plugin::PluginPickerId,
        /// Selected item identity.
        item_id: crate::ui::picker::plugin::PluginPickerItemId,
    },
    /// Resolve a plugin confirmation with the selected response.
    PluginConfirmationSelect {
        /// Plugin that owns the confirmation.
        plugin: String,
        /// Confirmation instance identity.
        confirmation_id: crate::ui::confirmation_box::PluginConfirmationId,
        /// Selected response.
        selection: crate::ui::confirmation_box::PluginConfirmationSelection,
    },
    /// Submit text entered into a plugin-owned input box.
    PluginInputSubmit {
        /// Plugin that owns the input box.
        plugin: String,
        /// Input box instance identity.
        input_id: crate::ui::input_box::PluginInputId,
        /// Exact submitted text.
        text: String,
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
    /// Toggle visual wrapping for the focused editor pane.
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
    /// Focus the next persistent pane or overlay target.
    FocusNextTarget,
    /// Focus the previous persistent pane or overlay target.
    FocusPreviousTarget,
    /// Close the focused pane.
    ClosePane,
    /// Attempt to exit the editor, allowing the app to confirm first if needed.
    TryQuit,
    /// Exit the editor.
    Quit,
}

impl Command {
    /// Returns the stable semantic identifier exposed by command lifecycle events.
    ///
    /// Static commands use dotted, kebab-case names. Plugin commands preserve
    /// their plugin and command identifiers in `plugin.<plugin>.<command>`.
    pub fn event_name(&self) -> Cow<'_, str> {
        match self {
            Self::EnqueueNotification { .. } => Cow::Borrowed("notification.enqueue"),
            Self::OpenCommandLine => Cow::Borrowed("command-line.open"),
            Self::OpenUnnamedBuffer => Cow::Borrowed("buffer.new"),
            Self::OpenCompletion => Cow::Borrowed("completion.open"),
            Self::OpenBufferPicker => Cow::Borrowed("picker.buffer"),
            Self::OpenFilePicker => Cow::Borrowed("picker.file"),
            Self::OpenGitPicker => Cow::Borrowed("picker.git"),
            Self::OpenColorschemePicker => Cow::Borrowed("picker.colorscheme"),
            Self::OpenFiletypePicker => Cow::Borrowed("picker.filetype"),
            Self::OpenDocumentSymbolsPicker => Cow::Borrowed("picker.document-symbols"),
            Self::OpenWorkspaceSymbolsPicker => Cow::Borrowed("picker.workspace-symbols"),
            Self::OpenGrepPicker => Cow::Borrowed("picker.grep"),
            Self::WriteAll => Cow::Borrowed("buffer.save-all"),
            Self::SaveBuffer(_) => Cow::Borrowed("buffer.save"),
            Self::SaveBufferAs { .. } => Cow::Borrowed("buffer.save-as"),
            Self::CloseBuffer(_) => Cow::Borrowed("buffer.close"),
            Self::UnloadBuffer { .. } => Cow::Borrowed("buffer.unload"),
            Self::OverwriteBuffer(_) => Cow::Borrowed("buffer.overwrite"),
            Self::PluginRequest {
                plugin, command, ..
            } => Cow::Owned(format!("plugin.{plugin}.{command}")),
            Self::PluginPickerSelect { .. } => Cow::Borrowed("plugin.picker-select"),
            Self::PluginConfirmationSelect { .. } => Cow::Borrowed("plugin.confirmation-select"),
            Self::PluginInputSubmit { .. } => Cow::Borrowed("plugin.input-submit"),
            Self::PluginStatus => Cow::Borrowed("plugin.status"),
            Self::LspHover => Cow::Borrowed("lsp.hover"),
            Self::LspDefinition => Cow::Borrowed("lsp.definition"),
            Self::LspReferences => Cow::Borrowed("lsp.references"),
            Self::LspPreviousDiagnostic => Cow::Borrowed("lsp.previous-diagnostic"),
            Self::LspNextDiagnostic => Cow::Borrowed("lsp.next-diagnostic"),
            Self::LspPreviousErrorDiagnostic => Cow::Borrowed("lsp.previous-error"),
            Self::LspNextErrorDiagnostic => Cow::Borrowed("lsp.next-error"),
            Self::LspRenamePrompt => Cow::Borrowed("lsp.rename-prompt"),
            Self::LspRename(_) => Cow::Borrowed("lsp.rename"),
            Self::ApplyCompletion(_) => Cow::Borrowed("completion.apply"),
            Self::LspCodeActions => Cow::Borrowed("lsp.code-actions"),
            Self::LspApplyCodeAction { .. } => Cow::Borrowed("lsp.apply-code-action"),
            Self::OpenFile(_) => Cow::Borrowed("buffer.open"),
            Self::OpenFileAtCursor(_, _) => Cow::Borrowed("buffer.open-at-cursor"),
            Self::FocusBuffer(_) => Cow::Borrowed("buffer.focus"),
            Self::SetBufferFiletype(_, _) => Cow::Borrowed("buffer.set-filetype"),
            Self::GitPickerToggleStage(_) => Cow::Borrowed("git.toggle-stage"),
            Self::GitPickerDiscard(_) => Cow::Borrowed("git.discard"),
            Self::GitPickerDiscardConfirmed(_) => Cow::Borrowed("git.discard-confirmed"),
            Self::ResizePaneLeft(_) => Cow::Borrowed("pane.resize-left"),
            Self::ResizePaneRight(_) => Cow::Borrowed("pane.resize-right"),
            Self::ResizePaneUp(_) => Cow::Borrowed("pane.resize-up"),
            Self::ResizePaneDown(_) => Cow::Borrowed("pane.resize-down"),
            Self::EqualizeSplits => Cow::Borrowed("pane.equalize"),
            Self::PreviousTab(_) => Cow::Borrowed("tab.previous"),
            Self::NextTab(_) => Cow::Borrowed("tab.next"),
            Self::ToggleWrap => Cow::Borrowed("pane.wrap-toggle"),
            Self::SplitVertical => Cow::Borrowed("pane.split-vertical"),
            Self::SplitHorizontal => Cow::Borrowed("pane.split-horizontal"),
            Self::FocusPaneLeft => Cow::Borrowed("pane.focus-left"),
            Self::FocusPaneDown => Cow::Borrowed("pane.focus-down"),
            Self::FocusPaneUp => Cow::Borrowed("pane.focus-up"),
            Self::FocusPaneRight => Cow::Borrowed("pane.focus-right"),
            Self::FocusNextTarget => Cow::Borrowed("focus.next"),
            Self::FocusPreviousTarget => Cow::Borrowed("focus.previous"),
            Self::ClosePane => Cow::Borrowed("pane.close"),
            Self::TryQuit => Cow::Borrowed("editor.try-quit"),
            Self::Quit => Cow::Borrowed("editor.quit"),
        }
    }

    /// Returns whether this command represents an internal UI response.
    pub fn is_internal_response(&self) -> bool {
        matches!(
            self,
            Self::EnqueueNotification { .. }
                | Self::ApplyCompletion(_)
                | Self::OverwriteBuffer(_)
                | Self::GitPickerDiscardConfirmed(_)
                | Self::PluginPickerSelect { .. }
                | Self::PluginConfirmationSelect { .. }
                | Self::PluginInputSubmit { .. }
        )
    }

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
            | Self::FocusNextTarget
            | Self::FocusPreviousTarget
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
            | Self::PluginPickerSelect { .. }
            | Self::PluginConfirmationSelect { .. }
            | Self::PluginInputSubmit { .. }
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
    fn command_event_names_are_stable_semantic_identifiers() {
        assert_eq!(Command::SaveBuffer(None).event_name(), "buffer.save");
        assert_eq!(
            Command::SaveBufferAs {
                buffer_id: None,
                path: PathBuf::from("file.txt"),
            }
            .event_name(),
            "buffer.save-as"
        );
        assert_eq!(Command::CloseBuffer(None).event_name(), "buffer.close");
        assert_eq!(Command::NextTab(1).event_name(), "tab.next");
        assert_eq!(Command::LspHover.event_name(), "lsp.hover");
        assert_eq!(
            Command::PluginRequest {
                plugin: "acme-tools".to_string(),
                command: "sync.now".to_string(),
                args: Vec::new(),
            }
            .event_name(),
            "plugin.acme-tools.sync.now"
        );
    }

    #[test]
    fn internal_commands_are_excluded_from_command_events() {
        assert!(
            Command::EnqueueNotification {
                level: NotificationLevel::Info,
                message: String::new(),
            }
            .is_internal_response()
        );
        assert!(
            Command::ApplyCompletion(ApplyCompletion {
                range: crate::buffer::TextObjectRange {
                    start: Cursor::new(0, 0),
                    end: Cursor::new(0, 0),
                },
                text: String::new(),
                additional_text_edits: Vec::new(),
                lsp_completion_item: None,
                format: crate::ui::completion::CompletionInsertFormat::PlainText,
            })
            .is_internal_response()
        );
        assert!(Command::OverwriteBuffer(None).is_internal_response());
    }

    #[test]
    fn editor_intents_are_editor_only() {
        let intent = Intent::Editor(crate::editor::EditorAction::new(
            crate::editor::EditorOperation::MoveDown,
        ));
        assert_eq!(intent.keymap_inheritance(), KeymapInheritance::Editor);
    }
}
