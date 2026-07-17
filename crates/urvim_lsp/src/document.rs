//! Document DTOs for the editor-LSP boundary.
//!
//! Document identity uses `BufferId` from the shared identity crate so that
//! both `urvim_core` and `urvim_lsp` refer to the same buffer identity without
//! one depending on the other.

use std::path::PathBuf;

pub use urvim_id::BufferId;

/// A snapshot of the document state at a point in time, suitable for
/// passing across the editor–LSP boundary.
#[derive(Debug, Clone)]
pub struct LspDocumentSnapshot {
    /// The buffer identity.
    pub id: BufferId,
    /// The document URI as a string.
    pub uri: String,
    /// The local file path, if applicable.
    pub path: PathBuf,
    /// The language identifier used by the LSP server.
    pub language_id: String,
    /// The protocol version number.
    pub version: i32,
    /// A monotonically increasing generation counter for cache invalidation.
    pub generation: u64,
    /// The current text content.
    pub text: urvim_text::PieceTable,
}

/// An LSP text edit expressed in editor-neutral terms.
///
/// Core converts this into buffer edit calls.
#[derive(Debug, Clone)]
pub struct LspTextEdit {
    /// The range in the document to replace.
    pub range: urvim_text::TextRange,
    /// The replacement text.
    pub text: String,
}

/// Effects emitted by the LSP runtime when editor-side work is required.
///
/// Core receives these effects and applies them to editor state. The `urvim_lsp`
/// runtime never touches `urvim_core` globals directly — instead it enqueues
/// effects via a channel and core drains them through `apply_lsp_effect`.
#[derive(Debug, Clone)]
pub enum LspRuntimeEffect {
    /// An initialized server session started.
    ServerStarted {
        /// Configured server name.
        server_name: String,
        /// Workspace root that identifies the session with the server name.
        workspace_root: PathBuf,
    },
    /// A server session failed to start or initialize.
    ServerStartFailed {
        /// Configured server name.
        server_name: String,
        /// Workspace root that identifies the failed session state.
        workspace_root: PathBuf,
        /// Human-readable process or initialization failure.
        error: String,
    },
    /// A server session stopped.
    ServerStopped {
        /// Configured server name.
        server_name: String,
        /// Workspace root that identifies the session with the server name.
        workspace_root: PathBuf,
        /// Stable reason the runtime stopped the session.
        reason: String,
    },
    /// A buffer was successfully opened in a server session.
    BufferAttached {
        /// Configured server name.
        server_name: String,
        /// Workspace root that identifies the session with the server name.
        workspace_root: PathBuf,
        /// Attached buffer identity.
        buffer_id: BufferId,
        /// URI sent to the server.
        uri: String,
        /// Language identifier sent to the server.
        language_id: String,
    },
    /// An existing buffer attachment was removed from a server session.
    BufferDetached {
        /// Configured server name.
        server_name: String,
        /// Workspace root that identifies the session with the server name.
        workspace_root: PathBuf,
        /// Detached buffer identity.
        buffer_id: BufferId,
        /// URI previously sent to the server.
        uri: String,
        /// Language identifier previously sent to the server.
        language_id: String,
        /// Stable reason the attachment was removed.
        reason: String,
    },
    /// Diagnostics received for a buffer.
    ///
    /// The diagnostics carry raw LSP positions; core converts them to buffer
    /// cursor coordinates using the current buffer text.
    Diagnostics {
        /// The buffer that owns the diagnostics.
        buffer_id: BufferId,
        /// The server that produced the diagnostics.
        server_name: String,
        /// The raw LSP diagnostic values.
        diagnostics: Vec<lsp_types::Diagnostic>,
    },
    /// Clear diagnostics for a buffer that has been detached from a server.
    ClearDiagnostics {
        /// The buffer whose diagnostics should be cleared.
        buffer_id: BufferId,
        /// The server name whose diagnostics should be cleared.
        server_name: String,
    },
    /// The LSP server requested opening a file on disk.
    OpenDocument {
        /// The file path to open.
        path: PathBuf,
    },
    /// Apply text edits to a file.
    ///
    /// Core resolves `path` to a buffer (opening it if needed), converts the
    /// `TextRange` positions to buffer cursors using the negotiated position
    /// encoding, and applies the edits.
    ApplyTextEdits {
        /// Server that produced the edit when known.
        server_name: Option<String>,
        /// The file path to edit.
        path: PathBuf,
        /// The edits to apply.
        edits: Vec<LspTextEdit>,
    },
    /// A workspace-level file operation.
    WorkspaceFileOperation {
        /// The operation to perform.
        operation: LspWorkspaceFileOperation,
    },
    /// Request the editor redraw the screen.
    RequestRedraw,
    /// Request the editor to refresh inlay hints.
    RequestInlayHintRetry,
}

/// A workspace-level file operation originating from an LSP server.
#[derive(Debug, Clone)]
pub enum LspWorkspaceFileOperation {
    /// Create a file.
    Create {
        /// The file path to create.
        path: PathBuf,
        /// Whether to overwrite an existing file.
        overwrite: bool,
        /// Whether to silently ignore if the file already exists.
        ignore_if_exists: bool,
    },
    /// Rename a file.
    Rename {
        /// The old file path.
        old_path: PathBuf,
        /// The new file path.
        new_path: PathBuf,
        /// Whether to overwrite an existing target file.
        overwrite: bool,
        /// Whether to silently ignore if the target already exists.
        ignore_if_exists: bool,
    },
    /// Delete a file.
    Delete {
        /// The file path to delete.
        path: PathBuf,
        /// Whether to silently ignore if the file does not exist.
        ignore_if_not_exists: bool,
    },
}
