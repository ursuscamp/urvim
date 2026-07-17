//! LSP runtime: session management, request dispatch, and protocol plumbing.
//!
//! This module is the process-level LSP runtime that lives in `urvim_lsp`.
//! It owns server processes, handles JSON-RPC communication, and exposes
//! document-shaped request methods. Editor-side work (buffer mutation,
//! diagnostics storage, UI notifications) is communicated to core via
//! `LspRuntimeEffect` values drained from an effect channel.
//!
//! Module layout:
//!
//! - [`session`]: `LspServerSession`, `ServerRuntime`, process spawn, reader
//!   thread, JSON-RPC plumbing, position/URI helpers.
//! - [`capabilities`]: server capability checks.
//! - [`requests`]: per-session request handlers returning raw LSP types.
//! - [`workspace_edit`]: `WorkspaceEdit` → `LspRuntimeEffect` conversion.

pub mod capabilities;
pub mod requests;
pub mod session;
pub mod workspace_edit;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use lsp_types::PositionEncodingKind;
use urvim_id::BufferId;
use urvim_text::PieceTable;

use crate::config::LspConfig;
use crate::document::{LspDocumentSnapshot, LspRuntimeEffect};

pub use requests::LspInlayHintSnapshot;
pub use session::{LspServerSession, LspServerStatus, ServerRuntime};

use session::{
    BufferAttachment, ServerProgressState, buffer_text_from_lines, resolve_workspace_root,
};

/// LSP runtime state and session management.
///
/// Owned by core's `LspRuntime` wrapper. Core builds `LspDocumentSnapshot`
/// values from the buffer pool, passes them to `sync_documents` and the
/// `*_document` request methods, and drains `LspRuntimeEffect` values from
/// the effect channel to apply editor-side changes.
#[derive(Debug)]
pub struct LspRuntime {
    pub servers: BTreeMap<String, ServerRuntime>,
    effect_sender: mpsc::Sender<LspRuntimeEffect>,
    effect_receiver: mpsc::Receiver<LspRuntimeEffect>,
    shutting_down: bool,
}

impl LspRuntime {
    /// Creates a new runtime from the resolved LSP config.
    pub fn new(config: &LspConfig) -> Self {
        let (effect_sender, effect_receiver) = mpsc::channel();
        let mut servers = BTreeMap::new();
        for (name, server) in &config.servers {
            servers.insert(
                name.clone(),
                ServerRuntime {
                    config: server.clone(),
                    sessions: BTreeMap::new(),
                    failed_sessions: BTreeMap::new(),
                    progress: Arc::new(Mutex::new(ServerProgressState::default())),
                },
            );
        }

        Self {
            servers,
            effect_sender,
            effect_receiver,
            shutting_down: false,
        }
    }

    /// Returns true when any LSP session is attached to the given buffer.
    pub fn buffer_has_lsp(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |_| Ok(()))
            .is_ok()
    }

    /// Returns the current compact status for each LSP server.
    pub fn server_statuses(&self) -> Vec<LspServerStatus> {
        self.servers
            .iter()
            .filter_map(|(server_name, server)| {
                server.progress.lock().ok().and_then(|progress| {
                    progress.current_message().map(|message| LspServerStatus {
                        server_name: server_name.clone(),
                        message,
                    })
                })
            })
            .collect()
    }

    /// Synchronizes server sessions with the given document snapshots.
    ///
    /// Core builds snapshots from the buffer pool and passes them here. This
    /// method spawns sessions for new workspace roots, sends didOpen/didChange
    /// for changed documents, and cleans up detached buffers (emitting
    /// `ClearDiagnostics` effects for core to apply).
    pub fn sync_documents(&mut self, documents: &[LspDocumentSnapshot]) {
        if self.shutting_down {
            return;
        }

        for (server_name, server) in &mut self.servers {
            if !server.config.enabled {
                continue;
            }

            let live_targets = documents
                .iter()
                .filter(|doc| server.matches_filetype(&doc.language_id))
                .filter_map(|doc| {
                    resolve_workspace_root(&doc.path, &server.config.root_markers)
                        .map(|root| (doc, root))
                })
                .collect::<Vec<_>>();
            let live_roots = live_targets
                .iter()
                .map(|(_, root)| root.clone())
                .collect::<BTreeSet<_>>();
            server
                .failed_sessions
                .retain(|root, _| live_roots.contains(root));

            for (doc, root) in &live_targets {
                if server.failed_sessions.contains_key(root) {
                    continue;
                }

                if !server.sessions.contains_key(root) {
                    match LspServerSession::spawn(
                        server_name,
                        &server.config,
                        root,
                        server.progress.clone(),
                        self.effect_sender.clone(),
                    ) {
                        Ok(session) => {
                            server.sessions.insert(root.clone(), session);
                            self.effect_sender
                                .send(LspRuntimeEffect::ServerStarted {
                                    server_name: server_name.clone(),
                                    workspace_root: root.clone(),
                                })
                                .ok();
                        }
                        Err(error) => {
                            tracing::warn!(
                                server = server_name,
                                root = ?root,
                                error = %error,
                                "failed to start lsp server"
                            );
                            let error = error.to_string();
                            server.failed_sessions.insert(root.clone(), error.clone());
                            self.effect_sender
                                .send(LspRuntimeEffect::ServerStartFailed {
                                    server_name: server_name.clone(),
                                    workspace_root: root.clone(),
                                    error,
                                })
                                .ok();
                            continue;
                        }
                    }
                }

                if let Some(session) = server.sessions.get_mut(root) {
                    let text = buffer_text_from_lines(&doc.text);
                    session
                        .sync_document(doc.id, &doc.path, doc.generation, &doc.language_id, &text)
                        .ok();
                }
            }

            let live_targets = live_targets
                .into_iter()
                .map(|(doc, root)| (doc.id, root))
                .collect::<BTreeSet<_>>();
            server.cleanup_detached_buffers(&live_targets);
        }
    }

    /// Shuts down all running LSP sessions.
    pub fn shutdown(&mut self) {
        self.shutting_down = true;
        for (server_name, server) in &mut self.servers {
            for session in server.sessions.values_mut() {
                for buffer_id in session.attached_buffer_ids() {
                    session.close_buffer(buffer_id, "shutdown");
                }
                session.shutdown().ok();
                self.effect_sender
                    .send(LspRuntimeEffect::ServerStopped {
                        server_name: server_name.clone(),
                        workspace_root: session.root.clone(),
                        reason: "shutdown".to_string(),
                    })
                    .ok();
            }
            server.sessions.clear();
            server.failed_sessions.clear();
        }
    }

    /// Notifies attached sessions that a buffer has been saved.
    pub fn did_save_buffer(&mut self, buffer_id: BufferId) {
        for server in self.servers.values_mut() {
            for session in server.sessions.values() {
                session.did_save_buffer(buffer_id);
            }
        }
    }

    /// Drains all pending `LspRuntimeEffect` values from the effect channel.
    ///
    /// Core calls this after `sync_documents` and after any request that might
    /// produce effects (rename, code actions).
    pub fn drain_effects(&self) -> Vec<LspRuntimeEffect> {
        let mut effects = Vec::new();
        while let Ok(effect) = self.effect_receiver.try_recv() {
            effects.push(effect);
        }
        effects
    }

    /// Returns the negotiated position encoding for a buffer, defaulting to
    /// UTF-16 if no session is attached.
    ///
    /// Core uses this when converting raw diagnostic positions to buffer
    /// cursors.
    pub fn position_encoding_for_buffer(&self, buffer_id: BufferId) -> PositionEncodingKind {
        for server in self.servers.values() {
            if let Some(session) = server
                .sessions
                .values()
                .find(|s| s.contains_buffer(buffer_id))
            {
                return session.negotiated.position_encoding.clone();
            }
        }
        PositionEncodingKind::UTF16
    }

    /// Returns the configured name of the first server attached to a buffer.
    pub fn server_name_for_buffer(&self, buffer_id: BufferId) -> Option<String> {
        self.servers.iter().find_map(|(server_name, server)| {
            server
                .sessions
                .values()
                .any(|session| session.contains_buffer(buffer_id))
                .then(|| server_name.clone())
        })
    }

    /// Notifies all sessions that a file was renamed, updating attachments.
    ///
    /// Core calls this after applying a `WorkspaceFileOperation::Rename` effect.
    /// `text` is the current buffer text (core reads it from the buffer pool).
    pub fn handle_file_renamed(&mut self, old_path: &Path, new_path: &Path, text: &str) {
        for server in self.servers.values_mut() {
            for session in server.sessions.values_mut() {
                session.rename_buffer_attachment(old_path, new_path, text);
            }
        }
    }

    /// Detaches a buffer from all sessions that hold it.
    ///
    /// Core calls this after applying a `WorkspaceFileOperation::Delete` effect.
    pub fn handle_file_deleted(&mut self, buffer_id: BufferId) {
        for server in self.servers.values_mut() {
            for session in server.sessions.values_mut() {
                if session.contains_buffer(buffer_id) {
                    session.close_buffer(buffer_id, "file_deleted");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // document-shaped request methods
    // -----------------------------------------------------------------------

    pub fn hover_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<Option<String>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.hover(attachment, lines, cursor)
        })
    }

    pub fn hover_document_async(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<mpsc::Receiver<urvim_json_rpc::Message>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.hover_async(attachment, lines, cursor)
        })
    }

    pub fn completion_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<Option<lsp_types::CompletionResponse>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.completion(attachment, lines, cursor)
        })
    }

    pub fn completion_document_async(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<mpsc::Receiver<urvim_json_rpc::Message>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.completion_async(attachment, lines, cursor)
        })
    }

    pub fn resolve_completion_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        item: &serde_json::Value,
    ) -> Result<Option<lsp_types::CompletionItem>, String> {
        self.with_session_for_document(snapshot, |session, _attachment, _lines| {
            session.resolve_completion(item)
        })
    }

    pub fn definition_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<Option<(String, lsp_types::Position)>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.definition(attachment, lines, cursor)
        })
    }

    pub fn definition_document_async(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<mpsc::Receiver<urvim_json_rpc::Message>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.definition_async(attachment, lines, cursor)
        })
    }

    pub fn references_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Result<Option<Vec<lsp_types::Location>>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.references(attachment, lines, cursor)
        })
    }

    pub fn document_symbols_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
    ) -> Result<Option<lsp_types::DocumentSymbolResponse>, String> {
        self.with_session_for_document(snapshot, |session, attachment, _lines| {
            session.document_symbols(attachment)
        })
    }

    pub fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> Result<Option<lsp_types::WorkspaceSymbolResponse>, String> {
        for server in self.servers.values_mut() {
            for session in server.sessions.values_mut() {
                if let Ok(Some(response)) = session.workspace_symbols(query) {
                    return Ok(Some(response));
                }
            }
        }
        Ok(None)
    }

    pub fn rename_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
        new_name: &str,
    ) -> Result<Option<lsp_types::WorkspaceEdit>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.rename(attachment, lines, cursor, new_name)
        })
    }

    pub fn rename_placeholder_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
    ) -> Option<String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            Ok(session.rename_placeholder(attachment, lines, cursor))
        })
        .ok()
        .flatten()
    }

    pub fn code_actions_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        cursor: urvim_text::Cursor,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Result<Option<Vec<lsp_types::CodeActionOrCommand>>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.code_actions(attachment, lines, cursor, diagnostics)
        })
    }

    pub fn execute_command_document(
        &mut self,
        buffer_id: BufferId,
        command: &str,
        arguments: Option<Vec<serde_json::Value>>,
    ) -> Result<(), String> {
        self.with_session_for_buffer_id(buffer_id, |session| {
            session.execute_command(command, arguments)
        })
    }

    pub fn request_inlay_hints_for_range_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: PositionEncodingKind,
        inlay_hints_enabled: bool,
        inlay_hint_type_enabled: bool,
        inlay_hint_parameter_enabled: bool,
    ) -> Result<Option<Vec<lsp_types::InlayHint>>, String> {
        self.with_session_for_document(snapshot, |session, _attachment, _live_lines| {
            session.request_inlay_hints_for_range(
                uri,
                lines,
                start_line,
                end_line,
                encoding,
                inlay_hints_enabled,
                inlay_hint_type_enabled,
                inlay_hint_parameter_enabled,
            )
        })
    }

    pub fn inlay_hint_snapshot_document(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        inlay_hints_enabled: bool,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        self.with_session_for_document(snapshot, |session, attachment, lines| {
            session.inlay_hint_snapshot(snapshot.id, attachment, lines, inlay_hints_enabled)
        })
    }

    pub fn send_inlay_hint_request(
        &mut self,
        buffer_id: BufferId,
        snapshot: &LspInlayHintSnapshot,
        start_line: usize,
        end_line: usize,
    ) -> Result<mpsc::Receiver<urvim_json_rpc::Message>, String> {
        self.with_session_for_buffer_id(buffer_id, |session| {
            session.send_inlay_hint_request(snapshot, start_line, end_line)
        })
    }

    // -----------------------------------------------------------------------
    // capability checks
    // -----------------------------------------------------------------------

    pub fn buffer_supports_hover(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.supports_hover()))
            .unwrap_or(false)
    }

    pub fn buffer_supports_definition(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.supports_definition()))
            .unwrap_or(false)
    }

    pub fn buffer_supports_references(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.supports_references()))
            .unwrap_or(false)
    }

    pub fn buffer_supports_document_symbols(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(
            buffer_id,
            |session| Ok(session.supports_document_symbols()),
        )
        .unwrap_or(false)
    }

    pub fn buffer_supports_rename(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.supports_rename()))
            .unwrap_or(false)
    }

    pub fn buffer_supports_inlay_hints(
        &mut self,
        buffer_id: BufferId,
        inlay_hints_enabled: bool,
    ) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| {
            Ok(session.supports_inlay_hints(inlay_hints_enabled))
        })
        .unwrap_or(false)
    }

    pub fn buffer_has_active_progress(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.has_active_progress()))
            .unwrap_or(false)
    }

    pub fn buffer_supports_code_actions(&mut self, buffer_id: BufferId) -> bool {
        self.with_session_for_buffer_id(buffer_id, |session| Ok(session.supports_code_actions()))
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // internal helpers
    // -----------------------------------------------------------------------

    fn with_session_for_document<R>(
        &mut self,
        snapshot: &LspDocumentSnapshot,
        f: impl FnOnce(&mut LspServerSession, &BufferAttachment, &PieceTable) -> Result<R, String>,
    ) -> Result<R, String> {
        for server in self.servers.values_mut() {
            if let Some(session) = server.session_for_buffer_mut(snapshot.id) {
                let Some(attachment) = session.buffer_attachment(snapshot.id) else {
                    continue;
                };
                return f(session, &attachment, &snapshot.text);
            }
        }
        Err("no attached LSP server for active buffer".to_string())
    }

    fn with_session_for_buffer_id<R>(
        &mut self,
        buffer_id: BufferId,
        f: impl FnOnce(&mut LspServerSession) -> Result<R, String>,
    ) -> Result<R, String> {
        for server in self.servers.values_mut() {
            if let Some(session) = server.session_for_buffer_mut(buffer_id) {
                return f(session);
            }
        }
        Err("no attached LSP server for active buffer".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LspServerConfig;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn temp_document() -> (PathBuf, LspDocumentSnapshot) {
        let root = std::env::temp_dir().join(format!(
            "urvim-lsp-lifecycle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create test root");
        let path = root.join("sample.rs");
        std::fs::write(&path, "fn main() {}\n").expect("write document");
        let uri = url::Url::from_file_path(&path)
            .expect("file URI")
            .to_string();
        let document = LspDocumentSnapshot {
            id: BufferId::new(7),
            uri,
            path,
            language_id: "rust".to_string(),
            version: 1,
            generation: 1,
            text: PieceTable::from_text("fn main() {}\n"),
        };
        (root, document)
    }

    #[test]
    fn failed_start_emits_once_per_live_failed_state() {
        let (root, document) = temp_document();
        let config = LspConfig {
            servers: BTreeMap::from([(
                "missing".to_string(),
                LspServerConfig {
                    enabled: true,
                    command: root.join("does-not-exist").to_string_lossy().into_owned(),
                    filetypes: vec!["rust".to_string()],
                    ..LspServerConfig::default()
                },
            )]),
        };
        let mut runtime = LspRuntime::new(&config);

        runtime.sync_documents(std::slice::from_ref(&document));
        assert!(matches!(
            runtime.drain_effects().as_slice(),
            [LspRuntimeEffect::ServerStartFailed {
                server_name,
                workspace_root,
                error,
            }] if server_name == "missing"
                && workspace_root == &root
                && !error.is_empty()
        ));

        runtime.sync_documents(std::slice::from_ref(&document));
        assert!(runtime.drain_effects().is_empty());

        runtime.sync_documents(&[]);
        runtime.sync_documents(std::slice::from_ref(&document));
        assert!(matches!(
            runtime.drain_effects().as_slice(),
            [LspRuntimeEffect::ServerStartFailed { .. }]
        ));

        runtime.shutdown();
        runtime.sync_documents(&[document]);
        assert!(runtime.drain_effects().is_empty());
        std::fs::remove_dir_all(root).ok();
    }
}
