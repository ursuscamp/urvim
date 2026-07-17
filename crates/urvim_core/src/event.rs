//! Editor event queue.
//!
//! Event-producing code enqueues semantic [`EditorEvent`] values; the app layer
//! drains the queue in FIFO order and translates events to plugin hook
//! notifications. Production code never dispatches plugin events directly.
//!
//! This module owns the [`EditorEvent`] enum and the [`BufferEventSnapshot`]
//! helper used to preserve buffer metadata as it existed when an event was
//! enqueued.

use std::path::PathBuf;

use crate::buffer::{Buffer, BufferId};

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

/// Semantic editor event produced by `urvim_core` and drained by the app loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    /// Editor startup finished far enough for dynamic hooks to receive events.
    EditorStarted,
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
    /// A non-plugin command was executed successfully.
    CommandExecuted {
        /// `Debug` representation of the executed command.
        command: String,
    },
    /// Diagnostics changed for a buffer.
    DiagnosticsChanged {
        /// Identifier of the buffer whose diagnostics changed.
        buffer_id: BufferId,
    },
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
