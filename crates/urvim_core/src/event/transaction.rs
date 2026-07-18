//! Coalescing event transactions and direct event origins.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::buffer::{BufferId, Cursor, PieceTable, TextRef, TextSnapshot};
use crate::editor::ModeKind;
use crate::editor_tab::{VisualSelection, VisualSelectionKind};
use crate::globals;
use crate::layout::PaneId;

use super::{ChangedRange, EditorEvent, EventPosition, EventSelection};

/// Stable source category attached to high-frequency editor events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSourceKind {
    /// A normal user action.
    User,
    /// Raw terminal or register paste.
    Paste,
    /// Undo history traversal.
    Undo,
    /// Redo history traversal.
    Redo,
    /// A plugin callback.
    Plugin,
    /// An LSP effect batch.
    Lsp,
    /// External file reload.
    Reload,
    /// Editor maintenance or unclassified internal work.
    Internal,
}

/// Direct origin attached to high-frequency editor events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventSource {
    /// Stable source category.
    pub kind: EventSourceKind,
    /// Plugin or LSP server name when applicable.
    pub name: Option<String>,
}

impl EventSource {
    /// Creates a user source.
    pub fn user() -> Self {
        Self::new(EventSourceKind::User, None)
    }
    /// Creates a paste source.
    pub fn paste() -> Self {
        Self::new(EventSourceKind::Paste, None)
    }
    /// Creates an undo source.
    pub fn undo() -> Self {
        Self::new(EventSourceKind::Undo, None)
    }
    /// Creates a redo source.
    pub fn redo() -> Self {
        Self::new(EventSourceKind::Redo, None)
    }
    /// Creates a named plugin source.
    pub fn plugin(name: impl Into<String>) -> Self {
        Self::new(EventSourceKind::Plugin, Some(name.into()))
    }
    /// Creates an optionally named LSP source.
    pub fn lsp(name: Option<impl Into<String>>) -> Self {
        Self::new(EventSourceKind::Lsp, name.map(Into::into))
    }
    /// Creates an external reload source.
    pub fn reload() -> Self {
        Self::new(EventSourceKind::Reload, None)
    }
    /// Creates an internal source.
    pub fn internal() -> Self {
        Self::new(EventSourceKind::Internal, None)
    }

    fn new(kind: EventSourceKind, name: Option<String>) -> Self {
        Self { kind, name }
    }
}

impl Default for EventSource {
    fn default() -> Self {
        Self::internal()
    }
}

/// Complete high-frequency state for one visible editor pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneEventSnapshot {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Active buffer identifier.
    pub buffer_id: BufferId,
    /// Current mode.
    pub mode: ModeKind,
    /// Current cursor.
    pub cursor: Cursor,
    /// Current visual selection.
    pub selection: Option<VisualSelection>,
}

#[derive(Debug)]
struct BufferChange {
    before: PieceTable,
    after: PieceTable,
    before_modified: bool,
    after_modified: bool,
}

#[derive(Debug)]
struct TransactionState {
    source: EventSource,
    buffer_change_kind: BufferChangeEventKind,
    buffers: BTreeMap<BufferId, BufferChange>,
    panes_before: Option<BTreeMap<PaneId, PaneEventSnapshot>>,
    panes_after: Option<BTreeMap<PaneId, PaneEventSnapshot>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BufferChangeEventKind {
    #[default]
    Regular,
    Insert,
}

thread_local! {
    static SOURCES: RefCell<Vec<EventSource>> = const { RefCell::new(Vec::new()) };
    static TRANSACTION: RefCell<Option<TransactionState>> = const { RefCell::new(None) };
}

/// RAII scope that classifies transactions begun while it is active.
pub struct EventSourceScope;

impl EventSourceScope {
    /// Pushes `source` until the returned scope is dropped.
    pub fn new(source: EventSource) -> Self {
        SOURCES.with(|sources| sources.borrow_mut().push(source));
        Self
    }
}

impl Drop for EventSourceScope {
    fn drop(&mut self) {
        SOURCES.with(|sources| {
            sources.borrow_mut().pop();
        });
    }
}

/// Returns the active direct source, falling back to `internal`.
pub fn current_event_source() -> EventSource {
    SOURCES
        .with(|sources| sources.borrow().last().cloned())
        .unwrap_or_default()
}

/// RAII coalescing boundary. Nested boundaries join the outer transaction.
pub struct EventTransaction {
    owner: bool,
    _source: Option<EventSourceScope>,
}

impl EventTransaction {
    /// Begins a transaction with an explicit direct source.
    pub fn new(source: EventSource) -> Self {
        Self::new_with_buffer_change_kind(source, BufferChangeEventKind::Regular)
    }

    /// Begins a transaction whose text changes belong to an Insert or Replace session.
    pub fn new_insert(source: EventSource) -> Self {
        Self::new_with_buffer_change_kind(source, BufferChangeEventKind::Insert)
    }

    fn new_with_buffer_change_kind(
        source: EventSource,
        buffer_change_kind: BufferChangeEventKind,
    ) -> Self {
        let source_scope = EventSourceScope::new(source.clone());
        let owner = TRANSACTION.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                false
            } else {
                *slot = Some(TransactionState {
                    source,
                    buffer_change_kind,
                    buffers: BTreeMap::new(),
                    panes_before: None,
                    panes_after: None,
                });
                true
            }
        });
        Self {
            owner,
            _source: Some(source_scope),
        }
    }

    /// Ensures mutable buffer access occurs in a transaction.
    pub fn ensure() -> Self {
        Self::new(current_event_source())
    }
}

impl Drop for EventTransaction {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        let state = TRANSACTION.with(|slot| slot.borrow_mut().take());
        if let Some(state) = state {
            let TransactionState {
                source,
                buffer_change_kind,
                buffers,
                panes_before,
                panes_after,
            } = state;
            emit_buffer_changes(source.clone(), buffer_change_kind, buffers);
            emit_pane_changes(source, panes_before, panes_after);
        }
    }
}

/// Records first-before/final-after buffer state for the active transaction.
pub fn record_buffer_change(
    buffer_id: BufferId,
    before: PieceTable,
    after: PieceTable,
    before_modified: bool,
    after_modified: bool,
) {
    TRANSACTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot
            .as_mut()
            .expect("buffer mutation must have a transaction");
        state
            .buffers
            .entry(buffer_id)
            .and_modify(|change| {
                change.after = after.clone();
                change.after_modified = after_modified;
            })
            .or_insert(BufferChange {
                before,
                after,
                before_modified,
                after_modified,
            });
    });
}

/// Flushes pending buffer dimensions before a lifecycle or domain event.
///
/// High-frequency events are ignored so the flush can enqueue its own output
/// through the normal queue without recursion. The active transaction remains
/// open and later mutations begin a new coalescing segment.
pub fn flush_buffer_changes_before(event: &EditorEvent) {
    if event.is_high_frequency() {
        return;
    }

    let pending = TRANSACTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut()?;
        (!state.buffers.is_empty()).then(|| {
            (
                state.source.clone(),
                state.buffer_change_kind,
                std::mem::take(&mut state.buffers),
            )
        })
    });
    if let Some((source, buffer_change_kind, buffers)) = pending {
        emit_buffer_changes(source, buffer_change_kind, buffers);
    }
}

/// Captures pane state. The first capture is the pre-state and later captures update the final state.
pub fn capture_pane_state(panes: Vec<PaneEventSnapshot>) {
    TRANSACTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return;
        };
        let panes: BTreeMap<_, _> = panes.into_iter().map(|pane| (pane.pane_id, pane)).collect();
        if state.panes_before.is_none() {
            state.panes_before = Some(panes.clone());
        }
        state.panes_after = Some(panes);
    });
}

fn emit_buffer_changes(
    source: EventSource,
    buffer_change_kind: BufferChangeEventKind,
    buffers: BTreeMap<BufferId, BufferChange>,
) {
    for (buffer_id, change) in buffers {
        if let Some(changed_range) = buffer_changed_range(&change.before, &change.after) {
            let event = match buffer_change_kind {
                BufferChangeEventKind::Regular => EditorEvent::BufferChanged {
                    buffer_id,
                    changed_range,
                    source: source.clone(),
                },
                BufferChangeEventKind::Insert => EditorEvent::InsertBufferChanged {
                    buffer_id,
                    changed_range,
                    source: source.clone(),
                },
            };
            globals::enqueue_editor_event(event);
        }
        if change.before_modified != change.after_modified {
            globals::enqueue_editor_event(EditorEvent::BufferModifiedChanged {
                buffer_id,
                previous_modified: change.before_modified,
                modified: change.after_modified,
                source: source.clone(),
            });
        }
    }
}

/// Returns the minimal changed range between two snapshots, or `None` when their text matches.
pub fn buffer_changed_range(before: &PieceTable, after: &PieceTable) -> Option<ChangedRange> {
    if text_snapshots_equal(before, after) {
        return None;
    }

    let before = before.text().to_text();
    let after = after.text().to_text();
    Some(minimal_changed_range(&before, &after))
}

fn emit_pane_changes(
    source: EventSource,
    before: Option<BTreeMap<PaneId, PaneEventSnapshot>>,
    after: Option<BTreeMap<PaneId, PaneEventSnapshot>>,
) {
    let (Some(before), Some(after)) = (before, after) else {
        return;
    };
    for (pane_id, final_state) in after {
        let Some(previous) = before.get(&pane_id) else {
            continue;
        };
        if previous.buffer_id != final_state.buffer_id {
            continue;
        }
        if previous.mode != final_state.mode {
            globals::enqueue_editor_event(EditorEvent::ModeChanged {
                pane_id,
                buffer_id: final_state.buffer_id,
                previous_mode: mode_name(previous.mode).to_string(),
                mode: mode_name(final_state.mode).to_string(),
                source: source.clone(),
            });
        }
        if previous.cursor != final_state.cursor {
            globals::enqueue_editor_event(EditorEvent::CursorMoved {
                pane_id,
                buffer_id: final_state.buffer_id,
                previous_position: cursor_position(previous.cursor),
                position: cursor_position(final_state.cursor),
                source: source.clone(),
            });
        }
        let old_selection = event_selection(previous);
        let new_selection = event_selection(&final_state);
        if old_selection != new_selection {
            globals::enqueue_editor_event(EditorEvent::SelectionChanged {
                pane_id,
                buffer_id: final_state.buffer_id,
                previous_selection: old_selection,
                selection: new_selection,
                source: source.clone(),
            });
        }
    }
}

fn text_snapshots_equal(before: &PieceTable, after: &PieceTable) -> bool {
    if before == after {
        return true;
    }
    if before.len() != after.len() {
        return false;
    }

    let before_text = before.text();
    let after_text = after.text();
    before_text
        .chunks()
        .flat_map(str::bytes)
        .eq(after_text.chunks().flat_map(str::bytes))
}

fn minimal_changed_range(before: &str, after: &str) -> ChangedRange {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let mut prefix = prefix;
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = before.len().min(after.len()).saturating_sub(prefix);
    let suffix = before.as_bytes()[before.len() - max_suffix..]
        .iter()
        .rev()
        .zip(after.as_bytes()[after.len() - max_suffix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut old_end = before.len() - suffix;
    let mut new_end = after.len() - suffix;
    while !before.is_char_boundary(old_end) || !after.is_char_boundary(new_end) {
        old_end += 1;
        new_end += 1;
    }
    ChangedRange {
        start: offset_position(before, prefix),
        old_end: offset_position(before, old_end),
        new_end: offset_position(after, new_end),
    }
}

fn offset_position(text: &str, offset: usize) -> EventPosition {
    let prefix = &text[..offset];
    let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let col = prefix
        .rfind('\n')
        .map_or(prefix.len(), |index| prefix.len() - index - 1);
    EventPosition { row, col }
}

fn cursor_position(cursor: Cursor) -> EventPosition {
    EventPosition {
        row: cursor.line,
        col: cursor.col,
    }
}

fn event_selection(state: &PaneEventSnapshot) -> Option<EventSelection> {
    state.selection.map(|selection| EventSelection {
        anchor: cursor_position(selection.anchor),
        cursor: cursor_position(state.cursor),
        linewise: selection.kind == VisualSelectionKind::Line,
    })
}

fn mode_name(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Normal => "normal",
        ModeKind::Insert => "insert",
        ModeKind::Replace => "replace",
        ModeKind::Visual => "visual",
        ModeKind::VisualLine => "visual_line",
        ModeKind::Resizing => "resizing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn take_events() -> Vec<EditorEvent> {
        globals::take_editor_event_batch()
    }

    fn text(text: &str) -> PieceTable {
        PieceTable::from_text(text)
    }

    #[test]
    fn minimal_range_uses_utf8_byte_columns() {
        assert_eq!(
            minimal_changed_range("aéz", "a日z"),
            ChangedRange {
                start: EventPosition { row: 0, col: 1 },
                old_end: EventPosition { row: 0, col: 3 },
                new_end: EventPosition { row: 0, col: 4 },
            }
        );
    }

    #[test]
    fn minimal_range_tracks_multiline_positions() {
        assert_eq!(
            minimal_changed_range("one\ntwo", "one\nthree"),
            ChangedRange {
                start: EventPosition { row: 1, col: 1 },
                old_end: EventPosition { row: 1, col: 3 },
                new_end: EventPosition { row: 1, col: 5 },
            }
        );
    }

    #[test]
    fn structurally_distinct_snapshots_compare_by_content_without_materializing() {
        let before = text("same");
        let after = before.update(0, Arc::<str>::from("same"));

        assert_ne!(before, after);
        assert!(text_snapshots_equal(&before, &after));
    }

    #[test]
    fn transaction_coalesces_first_text_and_final_modified_state() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new(EventSource::user());
        record_buffer_change(BufferId::new(7), text("abc"), text("axc"), false, true);
        record_buffer_change(BufferId::new(7), text("axc"), text("axy"), true, true);
        drop(transaction);

        let events = take_events();
        assert!(matches!(
            &events[0],
            EditorEvent::BufferChanged {
                buffer_id,
                changed_range: ChangedRange {
                    start: EventPosition { row: 0, col: 1 },
                    old_end: EventPosition { row: 0, col: 3 },
                    new_end: EventPosition { row: 0, col: 3 },
                },
                source,
            } if *buffer_id == BufferId::new(7) && *source == EventSource::user()
        ));
        assert!(matches!(
            &events[1],
            EditorEvent::BufferModifiedChanged {
                previous_modified: false,
                modified: true,
                ..
            }
        ));
    }

    #[test]
    fn insert_transaction_emits_granular_insert_change() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new_insert(EventSource::user());
        record_buffer_change(BufferId::new(9), text("a"), text("ab"), true, true);
        drop(transaction);

        assert!(matches!(
            take_events().as_slice(),
            [EditorEvent::InsertBufferChanged {
                buffer_id,
                source,
                ..
            }] if *buffer_id == BufferId::new(9) && *source == EventSource::user()
        ));
    }

    #[test]
    fn outer_transaction_keeps_its_buffer_change_classification() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new(EventSource::plugin("demo"));
        {
            let _nested = EventTransaction::new_insert(EventSource::user());
            record_buffer_change(BufferId::new(5), text("a"), text("ab"), true, true);
        }
        drop(transaction);

        assert!(matches!(
            take_events().as_slice(),
            [EditorEvent::BufferChanged { source, .. }]
                if *source == EventSource::plugin("demo")
        ));
    }

    #[test]
    fn transaction_suppresses_net_no_op_independently() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new(EventSource::internal());
        record_buffer_change(BufferId::new(3), text("same"), text("changed"), false, true);
        record_buffer_change(BufferId::new(3), text("changed"), text("same"), true, false);
        drop(transaction);
        assert!(take_events().is_empty());
    }

    #[test]
    fn nested_source_does_not_replace_direct_transaction_origin() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new(EventSource::plugin("owner"));
        {
            let _nested = EventTransaction::new(EventSource::plugin("observer"));
            record_buffer_change(BufferId::new(1), text(""), text("x"), false, true);
        }
        drop(transaction);
        assert!(matches!(
            take_events().as_slice(),
            [EditorEvent::BufferChanged { source, .. }, EditorEvent::BufferModifiedChanged { source: modified_source, .. }]
                if *source == EventSource::plugin("owner") && *modified_source == EventSource::plugin("owner")
        ));
    }

    #[test]
    fn pane_dimensions_coalesce_and_emit_separate_events() {
        globals::clear_editor_events_for_tests();
        let transaction = EventTransaction::new(EventSource::paste());
        capture_pane_state(vec![PaneEventSnapshot {
            pane_id: PaneId(2),
            buffer_id: BufferId::new(4),
            mode: ModeKind::Visual,
            cursor: Cursor::new(0, 1),
            selection: Some(VisualSelection {
                anchor: Cursor::new(0, 0),
                kind: VisualSelectionKind::Character,
            }),
        }]);
        capture_pane_state(vec![PaneEventSnapshot {
            pane_id: PaneId(2),
            buffer_id: BufferId::new(4),
            mode: ModeKind::Normal,
            cursor: Cursor::new(1, 2),
            selection: None,
        }]);
        drop(transaction);

        let events = take_events();
        assert!(matches!(events[0], EditorEvent::ModeChanged { .. }));
        assert!(matches!(events[1], EditorEvent::CursorMoved { .. }));
        assert!(matches!(events[2], EditorEvent::SelectionChanged { .. }));
    }
}
