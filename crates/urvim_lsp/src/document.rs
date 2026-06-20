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
