use super::EditorTab;
use crate::buffer::{BufferId, Cursor};
use crate::session::{SessionCursor, SessionEditorTab, SessionPosition};
use crate::ui::geometry::Position;

impl EditorTab {
    /// Converts a live tab into serializable session state.
    pub fn to_session(&self) -> SessionEditorTab {
        let path = self
            .buffer_view()
            .with_buffer(|buffer| {
                buffer
                    .path()
                    .map(|path| path.as_path().display().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        SessionEditorTab {
            path,
            cursor: SessionCursor {
                row: self.buffer_view().cursor().line,
                col: self.buffer_view().cursor().col,
            },
            scroll_offset: SessionPosition {
                row: self.buffer_view().scroll_offset().row,
                col: self.buffer_view().scroll_offset().col,
            },
            wrapped_row_offset: self.buffer_view().wrapped_row_offset(),
            wrap_enabled: self.wrap_enabled(),
        }
    }

    /// Restores a live tab from serialized session state.
    pub fn from_session(session: SessionEditorTab, buffer_id: BufferId) -> Self {
        let mut tab = Self::from_buffer_id(buffer_id);
        tab.set_wrap_enabled(session.wrap_enabled);
        {
            let view = tab.buffer_view_mut();
            view.set_cursor_synced(Cursor::new(session.cursor.row, session.cursor.col));
            let clamped_scroll = view
                .with_buffer(|buffer| {
                    if buffer.line_count() == 0 {
                        return Position::new(0, 0);
                    }

                    let row = session
                        .scroll_offset
                        .row
                        .min(buffer.line_count().saturating_sub(1) as u16);
                    let col = session
                        .scroll_offset
                        .col
                        .min(buffer.visual_line_width(row as usize) as u16);
                    Position::new(row, col)
                })
                .unwrap_or_else(|| Position::new(0, 0));
            view.set_scroll_offset(clamped_scroll);
            view.set_wrapped_row_offset(session.wrapped_row_offset);
        }
        tab
    }
}
