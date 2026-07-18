//! Insert and Replace session change tracking for plugin events.

use crate::buffer::{BufferId, PieceTable};
use crate::editor::ModeKind;
use crate::event::{EditorEvent, buffer_changed_range};
use crate::globals;
use crate::layout::{Layout, PaneId};
use crate::window::TabId;

#[derive(Debug)]
pub(super) struct InsertSession {
    window_id: PaneId,
    tab_id: TabId,
    buffer_id: BufferId,
    mode: ModeKind,
    before: PieceTable,
    after: PieceTable,
}

impl Layout {
    /// Starts an Insert or Replace event session for the focused editor tab.
    pub fn begin_insert_session(&mut self, mode: ModeKind) {
        debug_assert!(matches!(mode, ModeKind::Insert | ModeKind::Replace));
        let Some((window_id, tab_id, buffer_id)) = self.focused_editor_identity() else {
            return;
        };

        if self.insert_session.as_ref().is_some_and(|session| {
            session.window_id == window_id
                && session.tab_id == tab_id
                && session.buffer_id == buffer_id
        }) {
            return;
        }

        self.finish_insert_session();
        let Some(before) = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot()) else {
            return;
        };
        self.insert_session = Some(InsertSession {
            window_id,
            tab_id,
            buffer_id,
            mode,
            after: before.clone(),
            before,
        });
    }

    /// Returns the current text of the buffer owned by the active insert session.
    pub fn insert_session_text_snapshot(&self) -> Option<PieceTable> {
        let session = self.insert_session.as_ref()?;
        globals::with_buffer(session.buffer_id, |buffer| buffer.text_snapshot())
    }

    /// Records text when the latest insert-owned transaction changed the session buffer.
    pub fn record_insert_session_change(&mut self, transaction_before: &PieceTable) {
        let Some(session) = self.insert_session.as_mut() else {
            return;
        };
        let Some(after) = globals::with_buffer(session.buffer_id, |buffer| buffer.text_snapshot())
        else {
            return;
        };
        if buffer_changed_range(transaction_before, &after).is_some() {
            session.after = after;
        }
    }

    /// Finishes the active insert event session and emits its aggregate change.
    pub fn finish_insert_session(&mut self) {
        let Some(session) = self.insert_session.take() else {
            return;
        };
        let Some(changed_range) = buffer_changed_range(&session.before, &session.after) else {
            return;
        };

        globals::enqueue_editor_event(EditorEvent::InsertSessionChanged {
            window_id: session.window_id,
            tab_id: session.tab_id,
            buffer_id: session.buffer_id,
            mode: insert_mode_name(session.mode).to_string(),
            changed_range,
        });
    }

    /// Finishes the active insert session after its editor tab loses focus.
    pub fn reconcile_insert_session_focus(&mut self) {
        let Some(session) = self.insert_session.as_ref() else {
            return;
        };
        let identity = (session.window_id, session.tab_id, session.buffer_id);
        if self.focused_editor_identity() != Some(identity) {
            self.finish_insert_session();
        }
    }

    fn focused_editor_identity(&self) -> Option<(PaneId, TabId, BufferId)> {
        if self.plugin_windows.focused().is_some() || self.focused_plugin_pane().is_some() {
            return None;
        }
        let window_id = self.active_window_id()?;
        let group = self.active_window_group();
        let window = group.active_window();
        Some((window_id, window.tab_id(), window.buffer_view().buffer_id()))
    }
}

fn insert_mode_name(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Insert => "insert",
        ModeKind::Replace => "replace",
        _ => unreachable!("insert session must start in Insert or Replace mode"),
    }
}
