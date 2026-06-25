//! LSP runtime wrapper — thin core adapter over `urvim_lsp::runtime::LspRuntime`.
//!
//! Core owns the buffer pool, diagnostics store, and UI notification system.
//! This wrapper builds `LspDocumentSnapshot` values from buffer state, passes
//! them to `urvim_lsp` protocol methods, converts raw LSP responses to
//! editor-facing types, and applies `LspRuntimeEffect` values to editor state.
//!
//! ## Submodules
//!
//! - [`completion`]: `CompletionResponse` → `Vec<CompletionCandidate>`.
//! - [`symbols`]: symbol/reference conversion from raw LSP types to core types.
//! - [`effects`]: `LspRuntimeEffect` application to editor state.

pub(crate) mod completion;
pub(crate) mod effects;
pub(crate) mod symbols;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use lsp_types::{CodeActionOrCommand, PositionEncodingKind, WorkspaceSymbolResponse};
use serde_json::json;
use urvim_json_rpc::{ErrorResponse, Message, Response, SuccessResponse};

use crate::config::Config;
use crate::globals;
use crate::lsp::documents::snapshot_for_buffer;
use urvim_id::BufferId;
use urvim_lsp::document::LspDocumentSnapshot;
use urvim_lsp::position::{text_encoding_from_lsp, text_position_from_lsp};
use urvim_lsp::runtime::LspInlayHintSnapshot;
use urvim_lsp::runtime::workspace_edit::workspace_edit_to_effects;
use urvim_text::{Cursor, PieceTable, TextSnapshot};

pub use urvim_lsp::runtime::LspServerStatus;

use self::completion::{completion_item_additional_text_edits, completion_response_to_candidates};
use self::symbols::{
    build_document_symbol_nodes, flatten_document_symbol_response, locations_to_reference_items,
    workspace_symbol_information_to_item, workspace_symbol_to_item,
};

/// A document symbol resolved to a buffer location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbolItem {
    pub path: PathBuf,
    pub cursor: Cursor,
    pub range: Option<lsp_types::Range>,
    pub kind: lsp_types::SymbolKind,
    pub name: String,
    pub detail: Option<String>,
    pub depth: usize,
    pub search_text: String,
}

#[derive(Debug, Clone)]
pub struct DocumentSymbolTree {
    pub item: DocumentSymbolItem,
    pub children: Vec<DocumentSymbolTree>,
}

/// A single LSP reference location shown by the references picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceItem {
    pub path: PathBuf,
    pub cursor: Cursor,
    pub line_text: String,
}

/// A code action ready to be shown in the picker and later applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionApplication {
    pub title: String,
    pub kind: Option<String>,
    pub edit: Option<lsp_types::WorkspaceEdit>,
    pub command: Option<String>,
    pub command_arguments_json: Option<String>,
}

/// A non-blocking LSP request owned by the editor loop until the server responds.
#[derive(Debug)]
pub struct PendingLspRequest {
    kind: PendingLspRequestKind,
    cursor: Cursor,
    started_at: Instant,
    timeout: Duration,
    receiver: mpsc::Receiver<Message>,
    snapshot_text: PieceTable,
    position_encoding: PositionEncodingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingLspRequestKind {
    Hover,
    Definition,
    Completion,
}

/// Result of polling a pending LSP request without blocking.
#[derive(Debug)]
pub enum PendingLspPoll {
    Pending(PendingLspRequest),
    Ready(Result<serde_json::Value, String>),
}

impl PendingLspRequest {
    fn new(
        kind: PendingLspRequestKind,
        cursor: Cursor,
        receiver: mpsc::Receiver<Message>,
        snapshot_text: PieceTable,
        position_encoding: PositionEncodingKind,
    ) -> Self {
        Self {
            kind,
            cursor,
            started_at: Instant::now(),
            timeout: Duration::from_secs(10),
            receiver,
            snapshot_text,
            position_encoding,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(
        receiver: mpsc::Receiver<Message>,
        kind: &str,
        lines: &str,
        cursor: Cursor,
    ) -> Self {
        let kind = match kind {
            "hover" => PendingLspRequestKind::Hover,
            "definition" => PendingLspRequestKind::Definition,
            "completion" => PendingLspRequestKind::Completion,
            other => panic!("unknown pending LSP request kind {other}"),
        };
        Self {
            kind,
            cursor,
            started_at: Instant::now(),
            timeout: Duration::from_secs(10),
            receiver,
            snapshot_text: PieceTable::from_text(lines),
            position_encoding: PositionEncodingKind::UTF16,
        }
    }

    /// Polls this request once and returns immediately.
    pub fn poll(self) -> PendingLspPoll {
        if self.started_at.elapsed() >= self.timeout {
            return PendingLspPoll::Ready(Err("timed out waiting for LSP response".to_string()));
        }

        match self.receiver.try_recv() {
            Ok(message) => PendingLspPoll::Ready(self.resolve_message(message)),
            Err(mpsc::TryRecvError::Empty) => PendingLspPoll::Pending(self),
            Err(mpsc::TryRecvError::Disconnected) => PendingLspPoll::Ready(Err(
                "LSP response channel disconnected before a response was received".to_string(),
            )),
        }
    }

    fn resolve_message(self, message: Message) -> Result<serde_json::Value, String> {
        match message {
            Message::Response(Response::Success(SuccessResponse { result, .. })) => {
                self.resolve_success(result)
            }
            Message::Response(Response::Error(ErrorResponse { error, .. })) => Err(error.message),
            _ => Err("LSP request resolved with an unexpected message".to_string()),
        }
    }

    fn resolve_success(self, result: serde_json::Value) -> Result<serde_json::Value, String> {
        match self.kind {
            PendingLspRequestKind::Hover => resolve_hover_value(result),
            PendingLspRequestKind::Definition => resolve_definition_value(result),
            PendingLspRequestKind::Completion => resolve_completion_value(
                result,
                &self.snapshot_text,
                self.cursor,
                self.position_encoding,
            ),
        }
    }
}

/// LSP runtime state and session management.
///
/// This is a thin wrapper around `urvim_lsp::runtime::LspRuntime` that handles
/// editor-side concerns: building snapshots from the buffer pool, converting
/// LSP responses to editor types, and applying `LspRuntimeEffect` values.
#[derive(Debug)]
pub struct LspRuntime {
    runtime: urvim_lsp::runtime::LspRuntime,
}

impl LspRuntime {
    /// Creates a new runtime from the resolved editor config.
    pub fn new(config: &Config) -> Self {
        Self {
            runtime: urvim_lsp::runtime::LspRuntime::new(&config.lsp),
        }
    }

    /// Returns true when any LSP session is attached to the given buffer.
    pub fn buffer_has_lsp(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_has_lsp(buffer_id)
    }

    /// Returns the current compact status for each LSP server.
    pub fn server_statuses(&self) -> Vec<LspServerStatus> {
        self.runtime.server_statuses()
    }

    /// Synchronizes server sessions with the current editor buffers.
    pub fn sync(&mut self) {
        let documents = self.build_all_snapshots();
        self.runtime.sync_documents(&documents);
        self.drain_effects();
    }

    /// Shuts down all running LSP sessions.
    pub fn shutdown(&mut self) {
        self.runtime.shutdown();
    }

    /// Notifies attached sessions that a buffer has been saved.
    pub fn did_save_buffer(&mut self, buffer_id: BufferId) {
        self.runtime.did_save_buffer(buffer_id);
    }

    // -----------------------------------------------------------------------
    // hover
    // -----------------------------------------------------------------------

    pub fn hover_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<Option<String>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.runtime.hover_document(&snapshot, cursor)
    }

    pub fn request_hover_buffer_async(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<PendingLspRequest, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
        let receiver = self.runtime.hover_document_async(&snapshot, cursor)?;
        Ok(PendingLspRequest::new(
            PendingLspRequestKind::Hover,
            cursor,
            receiver,
            snapshot.text,
            encoding,
        ))
    }

    pub fn hover_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Result<Option<String>, String> {
        self.runtime.hover_document(snapshot, cursor)
    }

    // -----------------------------------------------------------------------
    // completion
    // -----------------------------------------------------------------------

    pub fn completion_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionCandidate>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let response = self.runtime.completion_document(&snapshot, cursor)?;
        Ok(response.map(|r| {
            completion_response_to_candidates(
                r,
                &snapshot.text,
                cursor,
                self.runtime.position_encoding_for_buffer(buffer_id),
            )
        }))
    }

    pub fn request_completion_buffer_async(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<PendingLspRequest, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
        let receiver = self.runtime.completion_document_async(&snapshot, cursor)?;
        Ok(PendingLspRequest::new(
            PendingLspRequestKind::Completion,
            cursor,
            receiver,
            snapshot.text,
            encoding,
        ))
    }

    pub fn completion_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionCandidate>>, String> {
        let response = self.runtime.completion_document(snapshot, cursor)?;
        Ok(response.map(|r| {
            completion_response_to_candidates(
                r,
                &snapshot.text,
                cursor,
                self.runtime.position_encoding_for_buffer(snapshot.id),
            )
        }))
    }

    pub fn resolve_completion_additional_text_edits(
        &mut self,
        buffer_id: BufferId,
        item: &serde_json::Value,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionTextEdit>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let resolved = self.runtime.resolve_completion_document(&snapshot, item)?;
        Ok(resolved.map(|item| {
            completion_item_additional_text_edits(
                &item,
                &snapshot.text,
                self.runtime.position_encoding_for_buffer(buffer_id),
            )
        }))
    }

    pub fn resolve_completion_additional_text_edits_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        item: &serde_json::Value,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionTextEdit>>, String> {
        let resolved = self.runtime.resolve_completion_document(snapshot, item)?;
        Ok(resolved.map(|item| {
            completion_item_additional_text_edits(
                &item,
                &snapshot.text,
                self.runtime.position_encoding_for_buffer(snapshot.id),
            )
        }))
    }

    // -----------------------------------------------------------------------
    // capability checks
    // -----------------------------------------------------------------------

    pub fn buffer_supports_hover(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_hover(buffer_id)
    }

    pub fn buffer_supports_definition(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_definition(buffer_id)
    }

    pub fn buffer_supports_references(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_references(buffer_id)
    }

    pub fn buffer_supports_document_symbols(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_document_symbols(buffer_id)
    }

    pub fn buffer_supports_rename(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_rename(buffer_id)
    }

    pub fn buffer_supports_inlay_hints(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        let inlay_hints_enabled = globals::with_config(|c| c.inlay_hints_enabled()).unwrap_or(true);
        self.runtime
            .buffer_supports_inlay_hints(buffer_id, inlay_hints_enabled)
    }

    pub fn buffer_has_active_progress(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_has_active_progress(buffer_id)
    }

    pub fn buffer_supports_code_actions(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.runtime.buffer_supports_code_actions(buffer_id)
    }

    // -----------------------------------------------------------------------
    // definition
    // -----------------------------------------------------------------------

    pub fn definition_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<Option<(BufferId, Cursor)>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.definition_document(&snapshot, cursor)
    }

    pub fn request_definition_buffer_async(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<PendingLspRequest, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
        let receiver = self.runtime.definition_document_async(&snapshot, cursor)?;
        Ok(PendingLspRequest::new(
            PendingLspRequestKind::Definition,
            cursor,
            receiver,
            snapshot.text,
            encoding,
        ))
    }

    pub fn definition_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Result<Option<(BufferId, Cursor)>, String> {
        let result = self.runtime.definition_document(snapshot, cursor)?;
        let Some((uri, position)) = result else {
            return Ok(None);
        };

        let path = uri_to_file_path(&uri)?;
        let buffer_id = globals::open_buffer(&path).map_err(|e| e.to_string())?;
        let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
        let lines = globals::with_buffer(buffer_id, |b| b.text_snapshot())
            .ok_or_else(|| "failed to read definition target buffer".to_string())?;
        let cursor = position_to_cursor(&lines, position, encoding)
            .ok_or_else(|| "failed to convert definition location".to_string())?;
        Ok(Some((buffer_id, cursor)))
    }

    // -----------------------------------------------------------------------
    // references
    // -----------------------------------------------------------------------

    pub fn references_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<Option<Vec<ReferenceItem>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.references_document(&snapshot, cursor)
    }

    pub fn references_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Result<Option<Vec<ReferenceItem>>, String> {
        let locations = self.runtime.references_document(snapshot, cursor)?;
        let Some(locations) = locations else {
            return Ok(None);
        };
        let items = locations_to_reference_items(locations);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    // -----------------------------------------------------------------------
    // document symbols
    // -----------------------------------------------------------------------

    pub fn document_symbols_buffer(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.document_symbols_document(&snapshot)
    }

    pub fn document_symbols_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        let response = self.runtime.document_symbols_document(snapshot)?;
        let Some(response) = response else {
            return Ok(None);
        };
        let encoding = self.runtime.position_encoding_for_buffer(snapshot.id);
        let path = uri_to_file_path(&snapshot.uri)?;
        let items = flatten_document_symbol_response(response, path, &snapshot.text, encoding);
        Ok(Some(items))
    }

    pub fn document_symbols_tree_buffer(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<Vec<DocumentSymbolTree>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.document_symbols_tree_document(&snapshot)
    }

    pub fn document_symbols_tree_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
    ) -> Result<Option<Vec<DocumentSymbolTree>>, String> {
        let response = self.runtime.document_symbols_document(snapshot)?;
        let Some(response) = response else {
            return Ok(None);
        };
        let encoding = self.runtime.position_encoding_for_buffer(snapshot.id);
        let path = uri_to_file_path(&snapshot.uri)?;
        let nodes = build_document_symbol_nodes(response, path, &snapshot.text, encoding);
        Ok(Some(nodes))
    }

    // -----------------------------------------------------------------------
    // workspace symbols
    // -----------------------------------------------------------------------

    pub fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        self.sync();
        let response = self.runtime.workspace_symbols(query)?;
        let Some(response) = response else {
            return Ok(None);
        };
        let items: Vec<DocumentSymbolItem> = match response {
            WorkspaceSymbolResponse::Flat(symbols) => symbols
                .into_iter()
                .filter_map(workspace_symbol_information_to_item)
                .collect(),
            WorkspaceSymbolResponse::Nested(symbols) => symbols
                .into_iter()
                .filter_map(workspace_symbol_to_item)
                .collect(),
        };
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    // -----------------------------------------------------------------------
    // rename
    // -----------------------------------------------------------------------

    pub fn rename_placeholder(&mut self, buffer_id: BufferId, cursor: Cursor) -> Option<String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id).ok()?;
        self.runtime.rename_placeholder_document(&snapshot, cursor)
    }

    pub fn rename_placeholder_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Option<String> {
        self.runtime.rename_placeholder_document(snapshot, cursor)
    }

    pub fn rename_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
        new_name: &str,
    ) -> Result<(), String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.rename_document(&snapshot, cursor, new_name)
    }

    pub fn rename_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
        new_name: &str,
    ) -> Result<(), String> {
        let edit = self.runtime.rename_document(snapshot, cursor, new_name)?;
        let Some(edit) = edit else {
            return Err("rename returned no workspace edit".to_string());
        };
        let effects = workspace_edit_to_effects(&edit)?;
        let has_file_ops = effects.iter().any(|e| {
            matches!(
                e,
                urvim_lsp::document::LspRuntimeEffect::WorkspaceFileOperation { .. }
            )
        });
        self.apply_effects(effects);
        if has_file_ops {
            self.sync();
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // inlay hints
    // -----------------------------------------------------------------------

    pub fn request_inlay_hints_for_range(
        &mut self,
        buffer_id: BufferId,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: PositionEncodingKind,
    ) -> Result<Option<Vec<lsp_types::InlayHint>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        let inlay_hints_enabled = globals::with_config(|c| c.inlay_hints_enabled()).unwrap_or(true);
        let (type_enabled, parameter_enabled) = inlay_hint_kind_config();
        self.runtime.request_inlay_hints_for_range_document(
            &snapshot,
            uri,
            lines,
            start_line,
            end_line,
            encoding,
            inlay_hints_enabled,
            type_enabled,
            parameter_enabled,
        )
    }

    pub fn request_inlay_hints_for_range_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: PositionEncodingKind,
    ) -> Result<Option<Vec<lsp_types::InlayHint>>, String> {
        let inlay_hints_enabled = globals::with_config(|c| c.inlay_hints_enabled()).unwrap_or(true);
        let (type_enabled, parameter_enabled) = inlay_hint_kind_config();
        self.runtime.request_inlay_hints_for_range_document(
            snapshot,
            uri,
            lines,
            start_line,
            end_line,
            encoding,
            inlay_hints_enabled,
            type_enabled,
            parameter_enabled,
        )
    }

    pub fn inlay_hint_snapshot(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.inlay_hint_snapshot_document(&snapshot)
    }

    pub fn inlay_hint_snapshot_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        let inlay_hints_enabled = globals::with_config(|c| c.inlay_hints_enabled()).unwrap_or(true);
        self.runtime
            .inlay_hint_snapshot_document(snapshot, inlay_hints_enabled)
    }

    pub fn send_inlay_hint_request_get_receiver(
        &mut self,
        buffer_id: BufferId,
        snapshot: &LspInlayHintSnapshot,
        start_line: usize,
        end_line: usize,
    ) -> Result<std::sync::mpsc::Receiver<urvim_json_rpc::Message>, String> {
        self.sync();
        self.runtime
            .send_inlay_hint_request(buffer_id, snapshot, start_line, end_line)
    }

    // -----------------------------------------------------------------------
    // code actions
    // -----------------------------------------------------------------------

    pub fn code_actions_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: Cursor,
    ) -> Result<Option<Vec<CodeActionApplication>>, String> {
        self.sync();
        let snapshot = build_document_snapshot(buffer_id)?;
        self.code_actions_document(&snapshot, cursor)
    }

    pub fn code_actions_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: Cursor,
    ) -> Result<Option<Vec<CodeActionApplication>>, String> {
        let diagnostics = globals::with_diagnostics_store(|store| {
            store.diagnostics_at_buffer_cursor(snapshot.id, cursor)
        })
        .unwrap_or_default();
        let actions = self
            .runtime
            .code_actions_document(snapshot, cursor, diagnostics)?;
        let Some(actions) = actions else {
            return Ok(None);
        };
        let applications = actions
            .into_iter()
            .filter_map(|action| match action {
                CodeActionOrCommand::CodeAction(action)
                    if action.edit.is_some() || action.command.is_some() =>
                {
                    let (command, command_arguments_json) = action
                        .command
                        .as_ref()
                        .map(|command| {
                            (
                                Some(command.command.clone()),
                                Some(serde_json::to_string(&command.arguments).unwrap_or_else(
                                    |error| {
                                        format!(
                                            "[\"command arguments serialization failed: {error}\"]"
                                        )
                                    },
                                )),
                            )
                        })
                        .unwrap_or((None, None));
                    Some(CodeActionApplication {
                        title: action.title,
                        kind: action.kind.map(|kind| kind.as_str().to_string()),
                        edit: action.edit,
                        command,
                        command_arguments_json,
                    })
                }
                CodeActionOrCommand::Command(command) => Some(CodeActionApplication {
                    title: command.title,
                    kind: None,
                    edit: None,
                    command: Some(command.command),
                    command_arguments_json: Some(
                        serde_json::to_string(&command.arguments).unwrap_or_else(|error| {
                            format!("[\"command arguments serialization failed: {error}\"]")
                        }),
                    ),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        if applications.is_empty() {
            Ok(None)
        } else {
            Ok(Some(applications))
        }
    }

    pub fn apply_code_action(
        &mut self,
        buffer_id: BufferId,
        action: CodeActionApplication,
    ) -> Result<(), String> {
        self.sync();

        let mut has_file_ops = false;
        if let Some(edit) = action.edit.as_ref() {
            let effects = workspace_edit_to_effects(edit)?;
            has_file_ops = effects.iter().any(|e| {
                matches!(
                    e,
                    urvim_lsp::document::LspRuntimeEffect::WorkspaceFileOperation { .. }
                )
            });
            self.apply_effects(effects);
        }

        if has_file_ops {
            self.sync();
        }

        if let Some(command) = action.command.as_ref() {
            let arguments: Option<Vec<serde_json::Value>> = action
                .command_arguments_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            self.runtime.execute_command_document(
                buffer_id,
                command.as_str(),
                arguments.clone(),
            )?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // snapshot building
    // -----------------------------------------------------------------------

    fn build_all_snapshots(&self) -> Vec<LspDocumentSnapshot> {
        let buffer_ids = globals::with_buffer_pool(|pool| pool.buffer_ids());
        buffer_ids
            .into_iter()
            .filter_map(|id| {
                globals::with_buffer(id, |buffer| snapshot_for_buffer(buffer, id, 0)).flatten()
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Free helpers used across submodules
// ---------------------------------------------------------------------------

fn build_document_snapshot(buffer_id: BufferId) -> Result<LspDocumentSnapshot, String> {
    globals::with_buffer(buffer_id, |buffer| {
        snapshot_for_buffer(buffer, buffer_id, 0)
    })
    .ok_or_else(|| format!("no buffer for id {buffer_id:?}"))?
    .ok_or_else(|| "buffer has no file path".to_string())
}

fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    let url = url::Url::parse(uri).map_err(|e| e.to_string())?;
    url.to_file_path()
        .map_err(|()| "LSP URI is not a file path".to_string())
}

fn inlay_hint_kind_config() -> (bool, bool) {
    globals::with_config(|config| {
        (
            config.inlay_hint_kind_enabled(&crate::config::InlayHintCapability::Type),
            config.inlay_hint_kind_enabled(&crate::config::InlayHintCapability::Parameter),
        )
    })
    .unwrap_or((true, true))
}

fn position_to_cursor(
    lines: &PieceTable,
    position: lsp_types::Position,
    encoding: PositionEncodingKind,
) -> Option<Cursor> {
    lines.cursor_for_position(
        text_position_from_lsp(position),
        text_encoding_from_lsp(encoding),
    )
}

fn resolve_hover_value(result: serde_json::Value) -> Result<serde_json::Value, String> {
    let hover = serde_json::from_value::<Option<lsp_types::Hover>>(result)
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "contents": hover.map(|hover| format_lsp_hover_contents(&hover.contents)),
    }))
}

fn resolve_definition_value(result: serde_json::Value) -> Result<serde_json::Value, String> {
    let response = serde_json::from_value::<Option<lsp_types::GotoDefinitionResponse>>(result)
        .map_err(|error| error.to_string())?;
    let Some(response) = response else {
        return Ok(json!({ "target": null }));
    };
    let Some((uri, position)) = first_definition_target(response) else {
        return Ok(json!({ "target": null }));
    };

    let path = uri_to_file_path(&uri)?;
    let buffer_id = globals::open_buffer(&path).map_err(|error| error.to_string())?;
    let encoding = globals::try_with_lsp_runtime_mut(|runtime| {
        runtime.runtime.position_encoding_for_buffer(buffer_id)
    })
    .unwrap_or(PositionEncodingKind::UTF16);
    let lines = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())
        .ok_or_else(|| "failed to read definition target buffer".to_string())?;
    let cursor = position_to_cursor(&lines, position, encoding)
        .ok_or_else(|| "failed to convert definition location".to_string())?;

    Ok(json!({
        "target": {
            "path": path,
            "buffer_id": buffer_id.get(),
            "line": cursor.line,
            "col": cursor.col,
        }
    }))
}

fn resolve_completion_value(
    result: serde_json::Value,
    lines: &PieceTable,
    cursor: Cursor,
    encoding: PositionEncodingKind,
) -> Result<serde_json::Value, String> {
    let response = serde_json::from_value::<Option<lsp_types::CompletionResponse>>(result)
        .map_err(|error| error.to_string())?;
    let items = response
        .map(|response| completion_response_to_candidates(response, lines, cursor, encoding))
        .unwrap_or_default()
        .into_iter()
        .map(completion_candidate_json)
        .collect::<Vec<_>>();
    Ok(json!({ "items": items }))
}

fn completion_candidate_json(
    candidate: crate::ui::completion::CompletionCandidate,
) -> serde_json::Value {
    json!({
        "label": candidate.label,
        "replacement": candidate.replacement,
        "range": text_range_json(candidate.range),
        "symbol": candidate.symbol,
        "kind": candidate.kind.and_then(|kind| serde_json::to_value(kind).ok()),
        "detail": candidate.detail,
        "labelDetail": candidate.label_detail,
        "labelDescription": candidate.label_description,
        "insertFormat": candidate.insert_format.map(|format| match format {
            crate::ui::completion::CompletionInsertFormat::PlainText => "plainText",
            crate::ui::completion::CompletionInsertFormat::Snippet => "snippet",
        }),
        "additionalTextEdits": candidate.additional_text_edits.into_iter().map(|edit| json!({
            "range": text_range_json(edit.range),
            "text": edit.text,
        })).collect::<Vec<_>>(),
        "lspCompletionItem": candidate.lsp_completion_item,
        "deprecated": candidate.deprecated,
        "preselect": candidate.preselect,
    })
}

fn text_range_json(range: crate::buffer::TextObjectRange) -> serde_json::Value {
    json!({
        "start": { "line": range.start.line, "col": range.start.col },
        "end": { "line": range.end.line, "col": range.end.col },
    })
}

fn format_lsp_hover_contents(contents: &lsp_types::HoverContents) -> String {
    match contents {
        lsp_types::HoverContents::Scalar(marked) => marked_string_text(marked),
        lsp_types::HoverContents::Array(items) => items
            .iter()
            .map(marked_string_text)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
    }
}

fn marked_string_text(marked: &lsp_types::MarkedString) -> String {
    match marked {
        lsp_types::MarkedString::String(text) => text.clone(),
        lsp_types::MarkedString::LanguageString(value) => value.value.clone(),
    }
}

fn first_definition_target(
    response: lsp_types::GotoDefinitionResponse,
) -> Option<(String, lsp_types::Position)> {
    match response {
        lsp_types::GotoDefinitionResponse::Scalar(location) => {
            Some((location.uri.to_string(), location.range.start))
        }
        lsp_types::GotoDefinitionResponse::Array(locations) => locations
            .into_iter()
            .next()
            .map(|location| (location.uri.to_string(), location.range.start)),
        lsp_types::GotoDefinitionResponse::Link(links) => links.into_iter().next().map(|link| {
            (
                link.target_uri.to_string(),
                link.target_selection_range.start,
            )
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, LspConfig, LspServerConfig};
    use std::collections::BTreeMap;
    use urvim_json_rpc::{Message, RequestId, Response, SuccessResponse};
    use urvim_text::TextRef;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "urvim-lsp-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn empty_runtime() -> LspRuntime {
        LspRuntime::new(&Config::default())
    }

    #[test]
    fn runtime_collects_enabled_servers_from_config() {
        let mut config = Config::default();
        config.lsp = LspConfig {
            servers: BTreeMap::from([(
                "rust_analyzer".to_string(),
                LspServerConfig {
                    enabled: true,
                    command: "rust-analyzer".to_string(),
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    filetypes: vec!["rust".to_string()],
                    root_markers: vec!["Cargo.toml".to_string()],
                    settings: serde_json::Value::Object(Default::default()),
                },
            )]),
        };

        let runtime = LspRuntime::new(&config);
        assert!(runtime.runtime.servers.contains_key("rust_analyzer"));
    }

    #[test]
    fn runtime_exposes_per_server_progress_status() {
        let mut config = Config::default();
        config.lsp = LspConfig {
            servers: BTreeMap::from([(
                "rust_analyzer".to_string(),
                LspServerConfig {
                    enabled: true,
                    command: "rust-analyzer".to_string(),
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    filetypes: vec!["rust".to_string()],
                    root_markers: vec!["Cargo.toml".to_string()],
                    settings: serde_json::Value::Object(Default::default()),
                },
            )]),
        };

        let mut runtime = LspRuntime::new(&config);
        let server = runtime
            .runtime
            .servers
            .get_mut("rust_analyzer")
            .expect("server runtime");
        {
            let mut progress = server.progress.lock().expect("progress lock");
            progress.set_begin(
                "token-1".to_string(),
                lsp_types::WorkDoneProgressBegin {
                    title: "indexing".to_string(),
                    cancellable: None,
                    message: Some("workspace".to_string()),
                    percentage: Some(12),
                },
            );
        }

        assert_eq!(
            runtime.server_statuses(),
            vec![LspServerStatus {
                server_name: "rust_analyzer".to_string(),
                message: "indexing workspace 12%".to_string(),
            }]
        );
    }

    #[test]
    fn build_document_snapshot_returns_snapshot_for_file_buffer() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let temp = temp_dir("snapshot");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("snapshot.rs");
        std::fs::write(&path, "fn main() {}\n").expect("write");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let snapshot = build_document_snapshot(buffer_id).expect("snapshot");

        assert_eq!(snapshot.id, buffer_id);
        assert!(snapshot.uri.starts_with("file://"));
        assert!(snapshot.uri.contains("snapshot.rs"));
        assert_eq!(snapshot.path, path);
        assert_eq!(snapshot.language_id, "rust");
        assert_eq!(snapshot.version, 0);
        assert_eq!(snapshot.text.text().to_text(), "fn main() {}");
    }

    #[test]
    fn build_document_snapshot_errors_for_unknown_buffer() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let unknown_id = BufferId::new(9999);
        let error = build_document_snapshot(unknown_id)
            .err()
            .expect("unknown buffer should error");
        assert!(error.contains("no buffer"));
    }

    #[test]
    fn hover_buffer_delegates_and_errors_without_session() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let temp = temp_dir("buffer-delegate");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("delegate.rs");
        std::fs::write(&path, "fn main() {}\n").expect("write");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");

        let mut runtime = empty_runtime();
        let error = runtime
            .hover_buffer(buffer_id, Cursor::new(0, 0))
            .err()
            .expect("no session should error");
        assert!(error.contains("no attached LSP server"));
    }

    #[test]
    fn pending_hover_request_converts_lsp_response() {
        let (tx, rx) = mpsc::channel();
        let pending =
            PendingLspRequest::new_for_test(rx, "hover", "fn main() {}", Cursor::new(0, 0));
        tx.send(Message::Response(Response::Success(SuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(1),
            result: serde_json::to_value(Some(lsp_types::Hover {
                contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                    kind: lsp_types::MarkupKind::Markdown,
                    value: "hover docs".to_string(),
                }),
                range: None,
            }))
            .expect("hover json"),
        })))
        .expect("send response");

        let PendingLspPoll::Ready(Ok(result)) = pending.poll() else {
            panic!("pending hover should be ready");
        };

        assert_eq!(result["contents"], "hover docs");
    }

    #[test]
    fn pending_completion_request_converts_lsp_response() {
        let (tx, rx) = mpsc::channel();
        let pending = PendingLspRequest::new_for_test(rx, "completion", "cl", Cursor::new(0, 2));
        tx.send(Message::Response(Response::Success(SuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(1),
            result: serde_json::to_value(Some(lsp_types::CompletionResponse::Array(vec![
                lsp_types::CompletionItem {
                    label: "clone".to_string(),
                    insert_text: Some("clone".to_string()),
                    detail: Some("fn clone".to_string()),
                    ..Default::default()
                },
            ])))
            .expect("completion json"),
        })))
        .expect("send response");

        let PendingLspPoll::Ready(Ok(result)) = pending.poll() else {
            panic!("pending completion should be ready");
        };

        assert_eq!(result["items"][0]["label"], "clone");
        assert_eq!(result["items"][0]["replacement"], "clone");
    }

    #[test]
    fn rename_placeholder_document_returns_none_when_no_session_attached() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let temp = temp_dir("no-rename");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("no_rename.rs");
        std::fs::write(&path, "fn main() {}\n").expect("write");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let snapshot = build_document_snapshot(buffer_id).expect("snapshot");

        let mut runtime = empty_runtime();
        assert!(
            runtime
                .rename_placeholder_document(&snapshot, Cursor::new(0, 0))
                .is_none()
        );
    }
}
