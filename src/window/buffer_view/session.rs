use super::BufferView;
use crate::buffer::{BufferId, Cursor};
use crate::window::Position;

impl BufferView {
    /// Builds a window-local buffer view from saved state.
    pub fn from_session_state(
        buffer_id: BufferId,
        cursor: Cursor,
        scroll_offset: Position,
    ) -> Self {
        let mut view = Self::from_buffer_id(buffer_id);
        view.set_cursor_synced(cursor);
        view.set_scroll_offset(scroll_offset);
        view
    }
}
