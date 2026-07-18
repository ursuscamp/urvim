//! Editor event queue.
//!
//! Event-producing code enqueues semantic [`EditorEvent`] values; the app layer
//! drains the queue in FIFO order and translates events to plugin hook
//! notifications. Production code never dispatches plugin events directly.
//!
//! This module owns the [`EditorEvent`] enum and the [`BufferEventSnapshot`]
//! helper used to preserve buffer metadata as it existed when an event was
//! enqueued.

use std::io;
use std::path::PathBuf;

use crate::buffer::{Buffer, BufferId};
use crate::layout::PaneId;
use crate::window::TabId;

mod transaction;

pub use transaction::{
    EventSource, EventSourceKind, EventSourceScope, EventTransaction, PaneEventSnapshot,
    buffer_changed_range, capture_pane_state, current_event_source, flush_buffer_changes_before,
    record_buffer_change,
};

/// Stable origin of a committed theme change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChangeSource {
    /// Selected from the colorscheme picker.
    Picker,
    /// Selected through a plugin API.
    Plugin,
    /// Selected because the active plugin theme was removed.
    Fallback,
}

impl ThemeChangeSource {
    /// Returns the stable plugin payload value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Picker => "picker",
            Self::Plugin => "plugin",
            Self::Fallback => "fallback",
        }
    }
}

/// Stable reason the editor entered final shutdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownReason {
    /// An accepted quit command ended the editor.
    Quit,
    /// Closing the final pane ended the editor.
    LastWindowClosed,
    /// An I/O error ended the editor loop.
    Error,
}

impl ShutdownReason {
    /// Returns the stable plugin payload value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::LastWindowClosed => "last_window_closed",
            Self::Error => "error",
        }
    }
}

/// Zero-based text position whose column is a UTF-8 byte offset in the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventPosition {
    /// Zero-based row number.
    pub row: usize,
    /// Zero-based UTF-8 byte column.
    pub col: usize,
}

/// Minimal replacement range between a buffer's pre- and post-transaction text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangedRange {
    /// Start of the replacement in both versions.
    pub start: EventPosition,
    /// End of the replaced text in the previous version.
    pub old_end: EventPosition,
    /// End of the inserted text in the final version.
    pub new_end: EventPosition,
}

/// Selection state visible to event consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSelection {
    /// Selection anchor.
    pub anchor: EventPosition,
    /// Selection cursor.
    pub cursor: EventPosition,
    /// Whether the selection is linewise rather than characterwise.
    pub linewise: bool,
}

/// Snapshot of buffer metadata captured when a buffer event is enqueued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferEventSnapshot {
    /// Identifier of the buffer.
    pub buffer_id: BufferId,
    /// Resolved absolute path of the buffer, if one was set.
    pub path: Option<PathBuf>,
    /// File name component of the resolved path, if one was set.
    pub file_name: Option<String>,
    /// Resolved canonical syntax name for the buffer.
    pub filetype: String,
    /// Whether the buffer was modified at the time of the snapshot.
    pub modified: bool,
}

impl BufferEventSnapshot {
    /// Builds a snapshot from a live `Buffer` reference.
    pub fn from_buffer(buffer_id: BufferId, buffer: &Buffer) -> Self {
        Self {
            buffer_id,
            path: buffer.path().map(|path| path.as_path().to_path_buf()),
            file_name: buffer
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            filetype: buffer.syntax_name().to_string(),
            modified: buffer.is_modified(),
        }
    }
}

/// Snapshot of a failed buffer filesystem operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferErrorSnapshot {
    /// Buffer involved in the operation, if one was identified.
    pub buffer_id: Option<BufferId>,
    /// Attempted absolute path, when path resolution succeeded.
    pub path: Option<PathBuf>,
    /// Stable, lowercase error category.
    pub error_kind: String,
    /// Human-readable operating-system or validation error.
    pub message: String,
}

impl BufferErrorSnapshot {
    /// Builds an error snapshot from an I/O error.
    pub fn from_io_error(
        buffer_id: Option<BufferId>,
        path: Option<PathBuf>,
        error: &io::Error,
    ) -> Self {
        Self {
            buffer_id,
            path,
            error_kind: io_error_kind_name(error.kind()).to_string(),
            message: error.to_string(),
        }
    }
}

/// Snapshot of a successful buffer path change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferPathChangeSnapshot {
    /// Buffer metadata after the path changed.
    pub buffer: BufferEventSnapshot,
    /// Previous absolute path, or `None` for a previously unnamed buffer.
    pub previous_path: Option<PathBuf>,
}

/// Aggregate diagnostics state captured immediately after one source mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticsEventSnapshot {
    /// Buffer whose diagnostics changed.
    pub buffer_id: BufferId,
    /// Plugin namespace or LSP server that was mutated.
    pub source: String,
    /// Whether the mutation cleared this source rather than replacing it.
    pub cleared: bool,
    /// Diagnostics remaining for the mutated source.
    pub source_count: usize,
    /// Diagnostics remaining across all sources for the buffer.
    pub total_count: usize,
    /// Remaining error diagnostics across all sources.
    pub errors: usize,
    /// Remaining warning diagnostics across all sources.
    pub warnings: usize,
    /// Remaining information diagnostics across all sources.
    pub information: usize,
    /// Remaining hint diagnostics across all sources.
    pub hints: usize,
}

/// Returns the stable lowercase name for an I/O error category.
pub fn io_error_kind_name(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::ConnectionRefused => "connection_refused",
        io::ErrorKind::ConnectionReset => "connection_reset",
        io::ErrorKind::HostUnreachable => "host_unreachable",
        io::ErrorKind::NetworkUnreachable => "network_unreachable",
        io::ErrorKind::ConnectionAborted => "connection_aborted",
        io::ErrorKind::NotConnected => "not_connected",
        io::ErrorKind::AddrInUse => "address_in_use",
        io::ErrorKind::AddrNotAvailable => "address_not_available",
        io::ErrorKind::NetworkDown => "network_down",
        io::ErrorKind::BrokenPipe => "broken_pipe",
        io::ErrorKind::AlreadyExists => "already_exists",
        io::ErrorKind::WouldBlock => "would_block",
        io::ErrorKind::NotADirectory => "not_a_directory",
        io::ErrorKind::IsADirectory => "is_a_directory",
        io::ErrorKind::DirectoryNotEmpty => "directory_not_empty",
        io::ErrorKind::ReadOnlyFilesystem => "read_only_filesystem",
        io::ErrorKind::StaleNetworkFileHandle => "stale_network_file_handle",
        io::ErrorKind::InvalidInput => "invalid_input",
        io::ErrorKind::InvalidData => "invalid_data",
        io::ErrorKind::TimedOut => "timed_out",
        io::ErrorKind::WriteZero => "write_zero",
        io::ErrorKind::StorageFull => "storage_full",
        io::ErrorKind::NotSeekable => "not_seekable",
        io::ErrorKind::FileTooLarge => "file_too_large",
        io::ErrorKind::ResourceBusy => "resource_busy",
        io::ErrorKind::ExecutableFileBusy => "executable_file_busy",
        io::ErrorKind::Deadlock => "deadlock",
        io::ErrorKind::CrossesDevices => "crosses_devices",
        io::ErrorKind::TooManyLinks => "too_many_links",
        io::ErrorKind::InvalidFilename => "invalid_filename",
        io::ErrorKind::ArgumentListTooLong => "argument_list_too_long",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::Unsupported => "unsupported",
        io::ErrorKind::UnexpectedEof => "unexpected_eof",
        io::ErrorKind::OutOfMemory => "out_of_memory",
        _ => "other",
    }
}

/// Semantic editor event produced by `urvim_core` and drained by the app loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    /// Editor startup finished far enough for dynamic hooks to receive events.
    EditorStarted,
    /// The editor has committed to shutting down.
    EditorWillShutdown {
        /// Why final shutdown began.
        reason: ShutdownReason,
        /// Modified buffers still loaded when shutdown began.
        modified_buffer_ids: Vec<BufferId>,
        /// Whether session persistence is enabled for this run.
        session_enabled: bool,
    },
    /// The committed colorscheme changed.
    ThemeChanged {
        /// Previously committed theme name.
        previous_theme: String,
        /// Newly committed theme name.
        theme: String,
        /// Origin of the committed change.
        source: ThemeChangeSource,
    },
    /// A buffer was loaded into the buffer pool.
    BufferLoaded {
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer was added to a UI pane.
    BufferOpened {
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer was saved successfully.
    BufferSaved {
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer save failed.
    BufferSaveFailed {
        /// Filesystem failure details captured at enqueue time.
        error: BufferErrorSnapshot,
    },
    /// Opening a buffer failed.
    BufferOpenFailed {
        /// Filesystem failure details captured at enqueue time.
        error: BufferErrorSnapshot,
    },
    /// A buffer's resolved path changed.
    BufferPathChanged {
        /// Path transition captured after the successful operation.
        snapshot: BufferPathChangeSnapshot,
    },
    /// A clean buffer was reloaded after its file changed externally.
    BufferReloaded {
        /// Buffer metadata after reloading.
        snapshot: BufferEventSnapshot,
    },
    /// A modified buffer's backing file changed externally.
    ExternalFileConflict {
        /// Buffer metadata when the distinct disk state was observed.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer's text changed during one completed event transaction.
    BufferChanged {
        /// Changed buffer.
        buffer_id: BufferId,
        /// Minimal UTF-8-safe replacement range.
        changed_range: ChangedRange,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// A buffer's text changed during one granular Insert or Replace transaction.
    InsertBufferChanged {
        /// Changed buffer.
        buffer_id: BufferId,
        /// Minimal UTF-8-safe replacement range.
        changed_range: ChangedRange,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// A buffer changed over one completed Insert or Replace session.
    InsertSessionChanged {
        /// Window that owned the session.
        window_id: PaneId,
        /// Tab that owned the session.
        tab_id: TabId,
        /// Changed buffer.
        buffer_id: BufferId,
        /// Mode in which the session began.
        mode: String,
        /// Minimal UTF-8-safe replacement range across the session.
        changed_range: ChangedRange,
    },
    /// A buffer's modified flag changed during one completed event transaction.
    BufferModifiedChanged {
        /// Changed buffer.
        buffer_id: BufferId,
        /// Modified flag before the transaction.
        previous_modified: bool,
        /// Modified flag after the transaction.
        modified: bool,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// A buffer tab/view was closed from the UI.
    BufferClosed {
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer was removed from the buffer pool.
    BufferUnloaded {
        /// Snapshot of buffer metadata captured before removal.
        snapshot: BufferEventSnapshot,
    },
    /// A buffer filetype changed.
    BufferFiletypeChanged {
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// An editor window was created.
    WindowCreated {
        /// Stable window identifier.
        window_id: PaneId,
        /// Buffer shown by the window's active tab.
        buffer_id: BufferId,
        /// Active tab's stable runtime identifier.
        tab_id: TabId,
    },
    /// An editor window was closed.
    WindowClosed {
        /// Stable window identifier.
        window_id: PaneId,
        /// Buffer shown by the window's final active tab.
        buffer_id: BufferId,
        /// Final active tab's stable runtime identifier.
        tab_id: TabId,
    },
    /// An editor window received focus.
    WindowFocused {
        /// Previously focused window, or `None` when there was none.
        previous_window_id: Option<PaneId>,
        /// Stable window identifier.
        window_id: PaneId,
        /// Buffer shown by the focused window's active tab.
        buffer_id: BufferId,
        /// Focused window's active tab identifier.
        tab_id: TabId,
    },
    /// A tab was opened in an editor window.
    TabOpened {
        /// Stable window identifier.
        window_id: PaneId,
        /// Stable runtime tab identifier.
        tab_id: TabId,
        /// Buffer metadata at enqueue time.
        snapshot: BufferEventSnapshot,
    },
    /// A tab was closed in an editor window.
    TabClosed {
        /// Stable window identifier.
        window_id: PaneId,
        /// Stable runtime tab identifier.
        tab_id: TabId,
        /// Buffer metadata captured before the tab was removed.
        snapshot: BufferEventSnapshot,
    },
    /// A different tab became active in an editor window.
    TabActivated {
        /// Previously active tab, or `None` for a newly created window.
        previous_tab_id: Option<TabId>,
        /// Stable window identifier.
        window_id: PaneId,
        /// Stable runtime tab identifier.
        tab_id: TabId,
        /// Buffer shown by the tab.
        buffer_id: BufferId,
    },
    /// The active editor buffer changed.
    ActiveBufferChanged {
        /// Previously active buffer, or `None` when there was none.
        previous_buffer_id: Option<BufferId>,
        /// Newly active buffer.
        buffer_id: BufferId,
        /// Window containing the newly active buffer.
        window_id: PaneId,
        /// Active tab showing the newly active buffer.
        tab_id: TabId,
    },
    /// An editor window's mode changed during one completed event transaction.
    ModeChanged {
        /// Changed window.
        window_id: PaneId,
        /// Buffer shown by the window.
        buffer_id: BufferId,
        /// Previous stable lowercase mode name.
        previous_mode: String,
        /// Final stable lowercase mode name.
        mode: String,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// An editor window's cursor moved during one completed event transaction.
    CursorMoved {
        /// Changed window.
        window_id: PaneId,
        /// Buffer shown by the window.
        buffer_id: BufferId,
        /// Previous cursor position.
        previous_position: EventPosition,
        /// Final cursor position.
        position: EventPosition,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// An editor window's visual selection changed during one completed transaction.
    SelectionChanged {
        /// Changed window.
        window_id: PaneId,
        /// Buffer shown by the window.
        buffer_id: BufferId,
        /// Previous selection, or `None`.
        previous_selection: Option<EventSelection>,
        /// Final selection, or `None`.
        selection: Option<EventSelection>,
        /// Direct origin of the transaction.
        source: EventSource,
    },
    /// A user-facing command completed or was accepted for asynchronous work.
    CommandExecuted {
        /// Stable command name.
        command: String,
        /// Whether the command completed successfully or was accepted.
        success: bool,
        /// Failure detail, or `None` on success.
        error: Option<String>,
    },
    /// An initialized LSP server session started.
    LspServerStarted {
        /// Configured server name.
        server_name: String,
        /// Workspace root identifying the session with the server name.
        workspace_root: PathBuf,
    },
    /// An LSP server session failed to start or initialize.
    LspServerStartFailed {
        /// Configured server name.
        server_name: String,
        /// Workspace root identifying the failed session state.
        workspace_root: PathBuf,
        /// Human-readable process or initialization failure.
        error: String,
    },
    /// An LSP server session stopped.
    LspServerStopped {
        /// Configured server name.
        server_name: String,
        /// Workspace root identifying the session with the server name.
        workspace_root: PathBuf,
        /// Stable stop reason.
        reason: String,
    },
    /// An LSP server session attached to a buffer.
    LspBufferAttached {
        /// Configured server name.
        server_name: String,
        /// Workspace root identifying the session with the server name.
        workspace_root: PathBuf,
        /// Attached buffer identity.
        buffer_id: BufferId,
        /// URI opened in the server.
        uri: String,
        /// Language identifier opened in the server.
        language_id: String,
    },
    /// An LSP server session detached from a buffer.
    LspBufferDetached {
        /// Configured server name.
        server_name: String,
        /// Workspace root identifying the session with the server name.
        workspace_root: PathBuf,
        /// Detached buffer identity.
        buffer_id: BufferId,
        /// URI previously opened in the server.
        uri: String,
        /// Language identifier previously opened in the server.
        language_id: String,
        /// Stable detach reason.
        reason: String,
    },
    /// Diagnostics changed for a buffer.
    DiagnosticsChanged {
        /// Post-mutation aggregate state.
        snapshot: DiagnosticsEventSnapshot,
    },
}

impl EditorEvent {
    /// Returns whether this event is a coalesced high-frequency transaction event.
    pub fn is_high_frequency(&self) -> bool {
        matches!(
            self,
            Self::BufferChanged { .. }
                | Self::InsertBufferChanged { .. }
                | Self::BufferModifiedChanged { .. }
                | Self::ModeChanged { .. }
                | Self::CursorMoved { .. }
                | Self::SelectionChanged { .. }
        )
    }

    /// Returns the directly originating plugin for suppressible high-frequency events.
    pub fn direct_plugin_source(&self) -> Option<&str> {
        let source = match self {
            Self::BufferChanged { source, .. }
            | Self::InsertBufferChanged { source, .. }
            | Self::BufferModifiedChanged { source, .. }
            | Self::ModeChanged { source, .. }
            | Self::CursorMoved { source, .. }
            | Self::SelectionChanged { source, .. } => source,
            _ => return None,
        };
        (source.kind == EventSourceKind::Plugin)
            .then(|| source.name.as_deref())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::AbsolutePath;
    use std::path::PathBuf;

    #[test]
    fn buffer_event_snapshot_captures_live_buffer_metadata() {
        let path = AbsolutePath::from_path(std::path::Path::new("/tmp/snapshot.rs")).unwrap();
        let mut buffer = Buffer::with_path(path.clone());
        buffer.insert_text(crate::buffer::Cursor::new(0, 0), "hi");
        let buffer_id = BufferId::new(7);

        let snapshot = BufferEventSnapshot::from_buffer(buffer_id, &buffer);

        assert_eq!(snapshot.buffer_id, buffer_id);
        assert_eq!(snapshot.path, Some(PathBuf::from("/tmp/snapshot.rs")));
        assert_eq!(snapshot.file_name.as_deref(), Some("snapshot.rs"));
        assert!(snapshot.modified);
    }
}
