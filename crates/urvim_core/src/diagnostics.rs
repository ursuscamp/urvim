//! Editor-facing diagnostics storage and navigation helpers.

use crate::buffer::{BufferId, Cursor};
use crate::event::DiagnosticsEventSnapshot;
use crate::lsp::diagnostics::{DiagnosticCounts, diagnostic_severity};
use lsp_types::{Diagnostic, DiagnosticSeverity, Position};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DiagnosticKey {
    buffer_id: BufferId,
    server_name: String,
}

/// In-memory diagnostics store keyed by buffer and source server.
#[derive(Debug, Default)]
pub struct DiagnosticsStore {
    diagnostics: Mutex<BTreeMap<DiagnosticKey, Vec<Diagnostic>>>,
}

impl DiagnosticsStore {
    /// Creates an empty diagnostics store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces diagnostics for a buffer and source, returning changed aggregate state.
    pub fn set(
        &self,
        buffer_id: BufferId,
        server_name: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Option<DiagnosticsEventSnapshot> {
        let source = server_name.into();
        let key = DiagnosticKey {
            buffer_id,
            server_name: source.clone(),
        };
        let mut store = self.diagnostics.lock().ok()?;
        if store
            .get(&key)
            .is_some_and(|current| current == &diagnostics)
        {
            return None;
        }
        store.insert(key, diagnostics);
        Some(diagnostics_snapshot(&store, buffer_id, source, false))
    }

    /// Clears diagnostics for a buffer and source server.
    pub fn clear(
        &self,
        buffer_id: BufferId,
        server_name: &str,
    ) -> Option<DiagnosticsEventSnapshot> {
        let mut store = self.diagnostics.lock().ok()?;
        if store
            .remove(&DiagnosticKey {
                buffer_id,
                server_name: server_name.to_string(),
            })
            .is_none()
        {
            return None;
        }
        Some(diagnostics_snapshot(
            &store,
            buffer_id,
            server_name.to_string(),
            true,
        ))
    }

    /// Clears all diagnostics for a buffer.
    pub fn clear_buffer(&self, buffer_id: BufferId) {
        if let Ok(mut store) = self.diagnostics.lock() {
            store.retain(|key, _| key.buffer_id != buffer_id);
        }
    }

    /// Clears all diagnostics for every buffer and source server.
    pub fn clear_all(&self) {
        if let Ok(mut store) = self.diagnostics.lock() {
            store.clear();
        }
    }

    /// Returns diagnostics for a buffer across all source servers.
    pub fn diagnostics_for_buffer(&self, buffer_id: BufferId) -> Vec<Diagnostic> {
        let mut diagnostics = self.collect_diagnostics(buffer_id);
        diagnostics.sort_by(|left, right| position_cmp(left.range.start, right.range.start));
        diagnostics
    }

    /// Returns diagnostics for a buffer from a single source server.
    pub fn diagnostics_for_buffer_source(
        &self,
        buffer_id: BufferId,
        server_name: &str,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = self
            .diagnostics
            .lock()
            .ok()
            .and_then(|store| {
                store
                    .get(&DiagnosticKey {
                        buffer_id,
                        server_name: server_name.to_string(),
                    })
                    .cloned()
            })
            .unwrap_or_default();
        diagnostics.sort_by(|left, right| position_cmp(left.range.start, right.range.start));
        diagnostics
    }

    /// Returns compact diagnostic counts for a buffer across all sources.
    pub fn diagnostic_counts_for_buffer(&self, buffer_id: BufferId) -> DiagnosticCounts {
        let mut counts = DiagnosticCounts::default();
        for diagnostic in self.collect_diagnostics(buffer_id) {
            match diagnostic_severity(&diagnostic) {
                DiagnosticSeverity::ERROR => counts.error += 1,
                DiagnosticSeverity::WARNING => counts.warning += 1,
                DiagnosticSeverity::INFORMATION => counts.info += 1,
                DiagnosticSeverity::HINT => counts.hint += 1,
                _ => counts.info += 1,
            }
        }
        counts
    }

    /// Returns diagnostics attached to the cursor position in a buffer.
    pub fn diagnostics_at_buffer_cursor(
        &self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Vec<Diagnostic> {
        let position = position_from_cursor(cursor);
        self.collect_diagnostics(buffer_id)
            .into_iter()
            .filter(|diagnostic| diagnostic_contains_position(diagnostic, position))
            .collect()
    }

    /// Returns the next diagnostic cursor after the provided cursor.
    pub fn next_diagnostic_cursor(
        &self,
        buffer_id: BufferId,
        cursor: Cursor,
        severity: Option<DiagnosticSeverity>,
    ) -> Option<Cursor> {
        let position = position_from_cursor(cursor);
        let mut diagnostics = self.collect_diagnostics(buffer_id);
        diagnostics.sort_by(|left, right| position_cmp(left.range.start, right.range.start));

        diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic_matches_severity(diagnostic, severity))
            .find(|diagnostic| position_cmp(diagnostic.range.start, position).is_gt())
            .map(|diagnostic| cursor_from_position(diagnostic.range.start))
    }

    /// Returns the previous diagnostic cursor before the provided cursor.
    pub fn previous_diagnostic_cursor(
        &self,
        buffer_id: BufferId,
        cursor: Cursor,
        severity: Option<DiagnosticSeverity>,
    ) -> Option<Cursor> {
        let position = position_from_cursor(cursor);
        let mut diagnostics = self.collect_diagnostics(buffer_id);
        diagnostics.sort_by(|left, right| position_cmp(left.range.start, right.range.start));

        diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic_matches_severity(diagnostic, severity))
            .rev()
            .find(|diagnostic| position_cmp(diagnostic.range.start, position).is_lt())
            .map(|diagnostic| cursor_from_position(diagnostic.range.start))
    }

    fn collect_diagnostics(&self, buffer_id: BufferId) -> Vec<Diagnostic> {
        self.diagnostics
            .lock()
            .ok()
            .map(|store| {
                store
                    .iter()
                    .filter(|(key, _)| key.buffer_id == buffer_id)
                    .flat_map(|(_, diagnostics)| diagnostics.iter().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn diagnostics_snapshot(
    store: &BTreeMap<DiagnosticKey, Vec<Diagnostic>>,
    buffer_id: BufferId,
    source: String,
    cleared: bool,
) -> DiagnosticsEventSnapshot {
    let source_count = store
        .get(&DiagnosticKey {
            buffer_id,
            server_name: source.clone(),
        })
        .map_or(0, Vec::len);
    let mut snapshot = DiagnosticsEventSnapshot {
        buffer_id,
        source,
        cleared,
        source_count,
        total_count: 0,
        errors: 0,
        warnings: 0,
        information: 0,
        hints: 0,
    };
    for diagnostic in store
        .iter()
        .filter(|(key, _)| key.buffer_id == buffer_id)
        .flat_map(|(_, diagnostics)| diagnostics)
    {
        snapshot.total_count += 1;
        match diagnostic_severity(diagnostic) {
            DiagnosticSeverity::ERROR => snapshot.errors += 1,
            DiagnosticSeverity::WARNING => snapshot.warnings += 1,
            DiagnosticSeverity::HINT => snapshot.hints += 1,
            _ => snapshot.information += 1,
        }
    }
    snapshot
}

fn position_from_cursor(cursor: Cursor) -> Position {
    Position::new(cursor.line as u32, cursor.col as u32)
}

fn cursor_from_position(position: Position) -> Cursor {
    Cursor::new(position.line as usize, position.character as usize)
}

fn diagnostic_matches_severity(
    diagnostic: &Diagnostic,
    severity: Option<DiagnosticSeverity>,
) -> bool {
    match severity {
        Some(target) => diagnostic_severity(diagnostic) == target,
        None => true,
    }
}

fn position_cmp(left: Position, right: Position) -> std::cmp::Ordering {
    left.line
        .cmp(&right.line)
        .then_with(|| left.character.cmp(&right.character))
}

fn diagnostic_contains_position(diagnostic: &Diagnostic, position: Position) -> bool {
    matches!(
        position_cmp(position, diagnostic.range.start),
        std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
    ) && matches!(
        position_cmp(position, diagnostic.range.end),
        std::cmp::Ordering::Less | std::cmp::Ordering::Equal
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, Range};

    fn diagnostic(line: u32, start: u32, end: u32, severity: DiagnosticSeverity) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, start), Position::new(line, end)),
            severity: Some(severity),
            code: None,
            code_description: None,
            source: Some("lsp".to_string()),
            message: format!("{severity:?}"),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn store_replaces_and_clears_source_diagnostics() {
        let store = DiagnosticsStore::new();
        let buffer_id = BufferId::new(7);

        store.set(
            buffer_id,
            "server-a",
            vec![diagnostic(0, 0, 1, DiagnosticSeverity::WARNING)],
        );
        assert_eq!(store.diagnostics_for_buffer(buffer_id).len(), 1);

        store.clear(buffer_id, "server-a");
        assert!(store.diagnostics_for_buffer(buffer_id).is_empty());
    }

    #[test]
    fn store_aggregates_across_sources() {
        let store = DiagnosticsStore::new();
        let buffer_id = BufferId::new(7);

        store.set(
            buffer_id,
            "server-a",
            vec![diagnostic(1, 0, 2, DiagnosticSeverity::WARNING)],
        );
        store.set(
            buffer_id,
            "server-b",
            vec![diagnostic(0, 0, 1, DiagnosticSeverity::ERROR)],
        );

        assert_eq!(
            store.diagnostic_counts_for_buffer(buffer_id),
            DiagnosticCounts {
                error: 1,
                warning: 1,
                info: 0,
                hint: 0,
            }
        );
        assert_eq!(store.diagnostics_for_buffer(buffer_id).len(), 2);
    }

    #[test]
    fn mutation_snapshot_is_post_change_and_suppresses_no_ops() {
        let store = DiagnosticsStore::new();
        let buffer_id = BufferId::new(9);
        let errors = vec![diagnostic(0, 0, 1, DiagnosticSeverity::ERROR)];

        let first = store
            .set(buffer_id, "server-a", errors.clone())
            .expect("first replacement changes the store");
        assert_eq!(first.source, "server-a");
        assert_eq!(first.source_count, 1);
        assert_eq!(first.total_count, 1);
        assert_eq!(first.errors, 1);
        assert!(!first.cleared);
        assert!(store.set(buffer_id, "server-a", errors).is_none());

        store.set(
            buffer_id,
            "plugin-ns",
            vec![diagnostic(1, 0, 1, DiagnosticSeverity::WARNING)],
        );
        let cleared = store
            .clear(buffer_id, "server-a")
            .expect("existing source is removed");
        assert!(cleared.cleared);
        assert_eq!(cleared.source_count, 0);
        assert_eq!(cleared.total_count, 1);
        assert_eq!(cleared.warnings, 1);
        assert!(store.clear(buffer_id, "server-a").is_none());
    }

    #[test]
    fn store_navigates_by_cursor_position() {
        let store = DiagnosticsStore::new();
        let buffer_id = BufferId::new(7);

        store.set(
            buffer_id,
            "server-a",
            vec![
                diagnostic(0, 0, 1, DiagnosticSeverity::WARNING),
                diagnostic(2, 0, 1, DiagnosticSeverity::ERROR),
            ],
        );

        assert_eq!(
            store.next_diagnostic_cursor(buffer_id, Cursor::new(0, 0), None),
            Some(Cursor::new(2, 0))
        );
        assert_eq!(
            store.previous_diagnostic_cursor(buffer_id, Cursor::new(3, 0), None),
            Some(Cursor::new(2, 0))
        );
        assert_eq!(
            store
                .diagnostics_at_buffer_cursor(buffer_id, Cursor::new(0, 0))
                .len(),
            1
        );
    }
}
