use crate::buffer::{
    BufferId, PieceTable, TextEncoding, TextPosition, TextRange, TextRef, TextSnapshot,
};
use crate::config::{Config, InlayHintCapability, LspServerConfig};
use crate::globals;
use crate::json_rpc::{
    ErrorResponse, Message, Notification, Request, RequestId, Response, SuccessResponse,
    decode_message, encode_message,
};
use crate::lsp::inlay_hint_job::LspInlayHintSnapshot;
use crate::lsp::position::position_to_byte_offset;
use crate::ui::completion::{CompletionCandidate, CompletionInsertFormat};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    CodeActionTriggerKind, CompletionItem, CompletionParams, CompletionResponse, CreateFile,
    DeleteFile, Diagnostic, DocumentChangeOperation, InitializeResult, InlayHint, InlayHintKind,
    InlayHintParams, Location, OneOf, PositionEncodingKind, PrepareRenameResponse, ProgressParams,
    ProgressParamsValue, ReferenceContext, RenameFile, ResourceOp, ServerCapabilities,
    TextDocumentIdentifier, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressReport,
    WorkspaceLocation, WorkspaceSymbol, WorkspaceSymbolResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

impl From<lsp_types::Position> for TextPosition {
    fn from(position: lsp_types::Position) -> Self {
        Self {
            line: position.line as usize,
            character: position.character as usize,
        }
    }
}

impl From<TextPosition> for lsp_types::Position {
    fn from(position: TextPosition) -> Self {
        Self::new(position.line as u32, position.character as u32)
    }
}

impl From<lsp_types::Range> for TextRange {
    fn from(range: lsp_types::Range) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

impl From<TextRange> for lsp_types::Range {
    fn from(range: TextRange) -> Self {
        Self::new(range.start.into(), range.end.into())
    }
}

fn text_encoding_from_lsp(encoding: PositionEncodingKind) -> TextEncoding {
    if encoding == PositionEncodingKind::UTF8 {
        TextEncoding::Utf8
    } else {
        TextEncoding::Utf16
    }
}

#[derive(Debug, Clone)]
struct BufferAttachment {
    uri: String,
    version: i32,
    generation: u64,
    language_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LspSessionState {
    Starting,
    Running,
    ShuttingDown,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
struct NegotiatedCapabilities {
    server_capabilities: Option<ServerCapabilities>,
    position_encoding: PositionEncodingKind,
    text_document_sync: Option<TextDocumentSyncKind>,
}

#[derive(Debug, Clone)]
enum WorkspaceResourceOperationEffect {
    Create {
        path: PathBuf,
    },
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    Delete {
        path: PathBuf,
        buffer_id: Option<BufferId>,
    },
}

impl Default for NegotiatedCapabilities {
    fn default() -> Self {
        Self {
            server_capabilities: None,
            position_encoding: PositionEncodingKind::UTF16,
            text_document_sync: None,
        }
    }
}

#[derive(Debug)]
struct LspServerSession {
    state: LspSessionState,
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<RequestId, mpsc::Sender<Message>>>>,
    next_request_id: AtomicU64,
    attachments: Arc<Mutex<HashMap<BufferId, BufferAttachment>>>,
    uri_to_buffer: Arc<Mutex<HashMap<String, BufferId>>>,
    progress: Arc<Mutex<ServerProgressState>>,
    root: PathBuf,
    server_name: String,
    negotiated: NegotiatedCapabilities,
    position_encoding: Arc<Mutex<PositionEncodingKind>>,
    initialization_options: Value,
}

#[derive(Debug)]
struct ServerRuntime {
    config: LspServerConfig,
    sessions: BTreeMap<PathBuf, LspServerSession>,
    failed_sessions: BTreeMap<PathBuf, String>,
    progress: Arc<Mutex<ServerProgressState>>,
}

#[derive(Debug, Default)]
struct ServerProgressState {
    next_sequence: u64,
    entries: HashMap<String, ServerProgressEntry>,
}

#[derive(Debug, Clone)]
struct ServerProgressEntry {
    sequence: u64,
    title: String,
    message: Option<String>,
    percentage: Option<u32>,
}

/// Compact per-server LSP status shown in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerStatus {
    /// Server name, such as `rust-analyzer`.
    pub server_name: String,
    /// Short human-readable status string.
    pub message: String,
}

/// A document symbol resolved to a buffer location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbolItem {
    /// The file that owns the symbol.
    pub path: PathBuf,
    /// The resolved buffer cursor for document-symbol locations.
    ///
    /// Workspace-symbol items keep a placeholder cursor and store the raw LSP
    /// range separately so the picker can resolve it lazily.
    pub cursor: crate::buffer::Cursor,
    /// Optional raw LSP range for lazy cursor resolution.
    pub range: Option<lsp_types::Range>,
    /// The symbol kind reported by the language server.
    pub kind: lsp_types::SymbolKind,
    /// The symbol name.
    pub name: String,
    /// Optional symbol detail or signature.
    pub detail: Option<String>,
    /// Flattened nesting depth used for indentation.
    pub depth: usize,
    /// Lowercased searchable text for query filtering.
    pub search_text: String,
}

/// A code action ready to be shown in the picker and later applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionApplication {
    /// Human-readable action title.
    pub title: String,
    /// Optional action kind string.
    pub kind: Option<String>,
    /// Optional edit to apply before any command.
    pub edit: Option<lsp_types::WorkspaceEdit>,
    /// Optional command name to execute after the edit is applied.
    pub command: Option<String>,
    /// Optional JSON-encoded command arguments.
    pub command_arguments_json: Option<String>,
}

/// LSP runtime state and session management.
#[derive(Debug)]
pub struct LspRuntime {
    servers: BTreeMap<String, ServerRuntime>,
}

#[derive(Debug, Deserialize)]
struct PublishDiagnosticsNotification {
    uri: String,
    diagnostics: Vec<Diagnostic>,
}

impl LspRuntime {
    /// Creates a new runtime from the resolved editor config.
    pub fn new(config: &Config) -> Self {
        let mut servers = BTreeMap::new();
        for (name, server) in &config.lsp.servers {
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

        Self { servers }
    }

    /// Returns true when any LSP session is attached to the given buffer.
    pub fn buffer_has_lsp(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |_, _, _| Ok(()))
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

    /// Synchronizes server sessions with the current editor buffers.
    pub fn sync(&mut self) {
        let buffer_ids = globals::with_buffer_pool(|pool| pool.buffer_ids());

        for (server_name, server) in &mut self.servers {
            if !server.config.enabled {
                continue;
            }

            let mut live_targets = BTreeSet::new();

            for buffer_id in &buffer_ids {
                let Some((path, syntax_name, generation)) =
                    globals::with_buffer(*buffer_id, |buffer| {
                        (
                            buffer.path().cloned(),
                            buffer.syntax_name().to_string(),
                            buffer.syntax_generation(),
                        )
                    })
                else {
                    continue;
                };

                if !server.matches_filetype(&syntax_name) {
                    continue;
                }

                let Some(path) = path else {
                    continue;
                };

                let Some(root) =
                    resolve_workspace_root(path.as_path(), &server.config.root_markers)
                else {
                    continue;
                };

                live_targets.insert((*buffer_id, root.clone()));

                if !server.sessions.contains_key(&root) {
                    match LspServerSession::spawn(
                        server_name,
                        &server.config,
                        &root,
                        server.progress.clone(),
                    ) {
                        Ok(session) => {
                            server.failed_sessions.remove(&root);
                            server.sessions.insert(root.clone(), session);
                        }
                        Err(error) => {
                            tracing::warn!(
                                server = server_name,
                                root = ?root,
                                error = %error,
                                "failed to start lsp server"
                            );
                            server
                                .failed_sessions
                                .insert(root.clone(), error.to_string());
                            continue;
                        }
                    }
                }

                if let Some(session) = server.sessions.get_mut(&root) {
                    session.sync_buffer(*buffer_id, path.as_path(), generation, &syntax_name);
                }
            }

            server.cleanup_detached_buffers(&live_targets);
        }
    }

    /// Shuts down all running LSP sessions.
    pub fn shutdown(&mut self) {
        for server in self.servers.values_mut() {
            for session in server.sessions.values_mut() {
                let _ = session.shutdown();
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

    fn apply_workspace_file_operations(&mut self, effects: &[WorkspaceResourceOperationEffect]) {
        for server in self.servers.values_mut() {
            server.apply_workspace_file_operations(effects);
        }
    }

    /// Requests hover information for the attached server owning `buffer_id`.
    pub fn hover_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<String>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.hover(attachment, lines, cursor)
        })
    }

    /// Requests completion candidates for the attached server owning `buffer_id`.
    pub fn completion_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<CompletionCandidate>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.completion(attachment, lines, cursor)
        })
    }

    /// Resolves any deferred additional edits for a completion item.
    pub fn resolve_completion_additional_text_edits(
        &mut self,
        buffer_id: BufferId,
        item: &serde_json::Value,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionTextEdit>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _attachment, lines| {
            session.resolve_completion_additional_text_edits(lines, item)
        })
    }

    /// Returns whether the attached server for `buffer_id` supports hover.
    pub fn buffer_supports_hover(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| Ok(session.supports_hover()))
            .unwrap_or(false)
    }

    /// Requests a go-to-definition target for the attached server owning `buffer_id`.
    pub fn definition_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<(BufferId, crate::buffer::Cursor)>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.definition(attachment, lines, cursor)
        })
    }

    /// Returns whether the attached server for `buffer_id` supports go to definition.
    pub fn buffer_supports_definition(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| Ok(session.supports_definition()))
            .unwrap_or(false)
    }

    /// Requests references for the symbol at `cursor` in `buffer_id`.
    pub fn references_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<ReferenceItem>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.references(attachment, lines, cursor)
        })
    }

    /// Returns whether the attached server for `buffer_id` supports references.
    pub fn buffer_supports_references(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| Ok(session.supports_references()))
            .unwrap_or(false)
    }

    /// Requests document symbols for the attached server owning `buffer_id`.
    pub fn document_symbols_buffer(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.document_symbols(attachment, lines)
        })
    }

    /// Requests document symbols for the attached server owning `buffer_id` as a tree.
    pub fn document_symbols_tree_buffer(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<Vec<DocumentSymbolTree>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.document_symbols_tree(attachment, lines)
        })
    }

    /// Requests workspace symbols matching `query` from all attached servers.
    pub fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        self.sync();

        let mut items = Vec::new();
        for server in self.servers.values_mut() {
            for session in server.sessions.values_mut() {
                let Some(mut session_items) = session.workspace_symbols(query).ok().flatten()
                else {
                    continue;
                };
                items.append(&mut session_items);
            }
        }

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    /// Returns whether the attached server for `buffer_id` supports document symbols.
    pub fn buffer_supports_document_symbols(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| {
            Ok(session.supports_document_symbols())
        })
        .unwrap_or(false)
    }

    /// Returns the server-provided placeholder for an LSP rename, if available.
    pub fn rename_placeholder(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Option<String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            Ok(session.rename_placeholder(attachment, lines, cursor))
        })
        .ok()
        .flatten()
    }

    /// Requests a rename on the attached server owning `buffer_id` and applies the edit.
    pub fn rename_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
        new_name: &str,
    ) -> Result<(), String> {
        self.sync();
        let result = self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.rename(attachment, lines, cursor, new_name)
        })?;
        if result.0 {
            self.apply_workspace_file_operations(&result.1);
            self.sync();
        }
        Ok(())
    }

    /// Returns whether the attached server for `buffer_id` supports rename.
    pub fn buffer_supports_rename(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| Ok(session.supports_rename()))
            .unwrap_or(false)
    }

    /// Returns whether the attached server for `buffer_id` supports inlay hints.
    pub fn buffer_supports_inlay_hints(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(
            buffer_id,
            |session, _, _| Ok(session.supports_inlay_hints()),
        )
        .unwrap_or(false)
    }

    /// Returns whether the attached server is reporting active progress for `buffer_id`.
    pub fn buffer_has_active_progress(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| Ok(session.has_active_progress()))
            .unwrap_or(false)
    }

    /// Requests inlay hints for a buffer range.
    pub fn request_inlay_hints_for_range(
        &mut self,
        buffer_id: BufferId,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: PositionEncodingKind,
    ) -> Result<Option<Vec<InlayHint>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, _live_lines| {
            session.request_inlay_hints_for_range(
                attachment, uri, lines, start_line, end_line, encoding,
            )
        })
    }

    /// Returns a snapshot for chunked inlay-hint requests.
    pub fn inlay_hint_snapshot(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.inlay_hint_snapshot(buffer_id, attachment, lines)
        })
    }

    /// Sends a viewport inlay-hint request and returns a response receiver.
    ///
    /// The caller briefly holds the global runtime mutex to send the request
    /// and register a response channel, then releases it.  The receiver can
    /// be waited on independently so the background worker does not block the
    /// UI hot path.
    pub fn send_inlay_hint_request_get_receiver(
        &mut self,
        buffer_id: BufferId,
        snapshot: &LspInlayHintSnapshot,
        start_line: usize,
        end_line: usize,
    ) -> Result<mpsc::Receiver<Message>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _attachment, _live_lines| {
            let Some(range) = line_range_to_lsp_range(
                &snapshot.lines,
                start_line,
                end_line,
                snapshot.position_encoding.clone(),
            ) else {
                return Err("invalid inlay hint range".to_string());
            };

            let params = InlayHintParams {
                work_done_progress_params: Default::default(),
                text_document: TextDocumentIdentifier {
                    uri: snapshot
                        .uri
                        .parse::<lsp_types::Uri>()
                        .map_err(|e| format!("invalid uri: {e}"))?,
                },
                range,
            };

            let id = RequestId::Number(session.next_request_id.fetch_add(1, Ordering::SeqCst));
            let value = serde_json::to_value(params).map_err(|e| e.to_string())?;
            let request = Message::Request(Request::new(
                id.clone(),
                "textDocument/inlayHint",
                Some(value),
            ));
            let (tx, rx) = mpsc::channel();
            if let Ok(mut pending) = session.pending.lock() {
                pending.insert(id.clone(), tx);
            }
            session.write_message(&request).map_err(|e| e.to_string())?;
            Ok(rx)
        })
    }

    /// Returns whether the attached server for `buffer_id` supports code actions.
    pub fn buffer_supports_code_actions(&mut self, buffer_id: BufferId) -> bool {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, _, _| {
            Ok(session.supports_code_actions())
        })
        .unwrap_or(false)
    }

    /// Requests code actions for the attached server owning `buffer_id`.
    pub fn code_actions_buffer(
        &mut self,
        buffer_id: BufferId,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<CodeActionApplication>>, String> {
        self.sync();
        self.with_session_for_buffer(buffer_id, |session, attachment, lines| {
            session.code_actions(buffer_id, attachment, lines, cursor)
        })
    }

    /// Applies a selected code action on the attached server owning `buffer_id`.
    pub fn apply_code_action(
        &mut self,
        buffer_id: BufferId,
        action: CodeActionApplication,
    ) -> Result<(), String> {
        self.sync();
        let result = self.with_session_for_buffer(buffer_id, |session, _, _| {
            session.apply_code_action_edit(&action)
        })?;

        if result.0 {
            self.apply_workspace_file_operations(&result.1);
            self.sync();
        }

        if let Some(command) = action.command.as_ref() {
            let arguments: Option<Vec<serde_json::Value>> = action
                .command_arguments_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            self.with_session_for_buffer(buffer_id, |session, _, _| {
                session.execute_command(command.as_str(), arguments.clone())
            })?;
        }

        Ok(())
    }

    fn with_session_for_buffer<R>(
        &mut self,
        buffer_id: BufferId,
        f: impl FnOnce(&mut LspServerSession, &BufferAttachment, &PieceTable) -> Result<R, String>,
    ) -> Result<R, String> {
        for server in self.servers.values_mut() {
            if let Some(session) = server.session_for_buffer_mut(buffer_id) {
                let Some(attachment) = session.buffer_attachment(buffer_id) else {
                    continue;
                };
                let Some(lines) = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())
                else {
                    continue;
                };
                return f(session, &attachment, &lines);
            }
        }

        Err("no attached LSP server for active buffer".to_string())
    }
}

impl ServerRuntime {
    fn matches_filetype(&self, syntax_name: &str) -> bool {
        self.config
            .filetypes
            .iter()
            .any(|filetype| filetype == syntax_name)
    }

    fn session_for_buffer_mut(&mut self, buffer_id: BufferId) -> Option<&mut LspServerSession> {
        self.sessions
            .values_mut()
            .find(|session| session.contains_buffer(buffer_id))
    }

    fn cleanup_detached_buffers(&mut self, live_targets: &BTreeSet<(BufferId, PathBuf)>) {
        let live_buffers = live_targets
            .iter()
            .map(|(buffer_id, _)| *buffer_id)
            .collect::<BTreeSet<_>>();

        for session in self.sessions.values_mut() {
            session.cleanup_detached_buffers(&live_buffers);
        }
    }

    fn apply_workspace_file_operations(&mut self, effects: &[WorkspaceResourceOperationEffect]) {
        for session in self.sessions.values_mut() {
            for effect in effects {
                session.apply_workspace_file_operation(effect);
            }
        }
    }
}

impl LspServerSession {
    fn spawn(
        server_name: &str,
        config: &LspServerConfig,
        root: &Path,
        progress: Arc<Mutex<ServerProgressState>>,
    ) -> io::Result<Self> {
        let mut command = Command::new(&config.command);
        command.current_dir(root);
        command.args(&config.args);
        command.envs(&config.env);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(open_lsp_log_stderr());

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdout"))?;

        let stdin = Arc::new(Mutex::new(stdin));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let uri_to_buffer = Arc::new(Mutex::new(HashMap::new()));
        let attachments = Arc::new(Mutex::new(HashMap::new()));
        let position_encoding = Arc::new(Mutex::new(PositionEncodingKind::UTF16));

        let mut session = Self {
            state: LspSessionState::Starting,
            child,
            stdin: stdin.clone(),
            pending: pending.clone(),
            next_request_id: AtomicU64::new(1),
            attachments,
            uri_to_buffer: uri_to_buffer.clone(),
            progress,
            root: root.to_path_buf(),
            server_name: server_name.to_string(),
            negotiated: NegotiatedCapabilities::default(),
            position_encoding,
            initialization_options: serde_json::to_value(&config.settings).unwrap_or(Value::Null),
        };

        session.spawn_reader(stdout);
        let initialize_result = match session.initialize() {
            Ok(result) => result,
            Err(error) => {
                session.state = LspSessionState::Failed;
                return Err(error);
            }
        };
        session.record_initialize_result(&initialize_result);
        session.state = LspSessionState::Running;
        Ok(session)
    }

    fn spawn_reader(&self, stdout: ChildStdout) {
        let pending = self.pending.clone();
        let uri_to_buffer = self.uri_to_buffer.clone();
        let progress = self.progress.clone();
        let stdin = self.stdin.clone();
        let attachments = self.attachments.clone();
        let position_encoding = self.position_encoding.clone();
        let server_name = self.server_name.clone();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let Some(bytes) = read_framed_message(&mut reader).ok().flatten() else {
                    break;
                };

                let Ok(message) = decode_message(&bytes) else {
                    continue;
                };

                match message {
                    Message::Request(Request { id, method, .. })
                        if method == "window/workDoneProgress/create" =>
                    {
                        let response = Message::Response(Response::Success(SuccessResponse {
                            id,
                            result: Value::Null,
                            jsonrpc: "2.0".to_string(),
                        }));
                        if let Ok(bytes) = encode_message(&response)
                            && let Ok(mut stdin) = stdin.lock()
                        {
                            let _ = stdin.write_all(&bytes);
                            let _ = stdin.flush();
                        }
                    }
                    Message::Response(Response::Success(SuccessResponse {
                        id, result, ..
                    })) => {
                        if let Some(sender) = pending
                            .lock()
                            .ok()
                            .and_then(|mut pending| pending.remove(&id))
                        {
                            let _ = sender.send(Message::Response(Response::Success(
                                SuccessResponse {
                                    id,
                                    result,
                                    jsonrpc: "2.0".to_string(),
                                },
                            )));
                        }
                    }
                    Message::Response(Response::Error(ErrorResponse { id, error, .. })) => {
                        if let Some(sender) = pending
                            .lock()
                            .ok()
                            .and_then(|mut pending| pending.remove(&id))
                        {
                            let _ =
                                sender.send(Message::Response(Response::Error(ErrorResponse {
                                    id,
                                    error,
                                    jsonrpc: "2.0".to_string(),
                                })));
                        }
                    }
                    Message::Notification(Notification { method, params, .. })
                        if method == "textDocument/publishDiagnostics" =>
                    {
                        if let Some(params) = params
                            && let Ok(params) =
                                serde_json::from_value::<PublishDiagnosticsNotification>(params)
                            && let Some(buffer_id) = uri_to_buffer
                                .lock()
                                .ok()
                                .and_then(|map| map.get(params.uri.as_str()).copied())
                        {
                            let attachment = attachments
                                .lock()
                                .ok()
                                .and_then(|map| map.get(&buffer_id).cloned());
                            let lines =
                                globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot());
                            let encoding = position_encoding
                                .lock()
                                .ok()
                                .map(|encoding| encoding.clone())
                                .unwrap_or(PositionEncodingKind::UTF16);
                            if let (Some(_attachment), Some(lines)) = (attachment, lines) {
                                let converted = params
                                    .diagnostics
                                    .into_iter()
                                    .filter_map(|diagnostic| {
                                        convert_diagnostic(&lines, diagnostic, encoding.clone())
                                    })
                                    .collect();
                                globals::with_diagnostics_store(|store| {
                                    store.set(buffer_id, server_name.as_str(), converted)
                                });
                            }
                            globals::request_inlay_hint_retry();
                            globals::request_notification_redraw();
                        }
                    }
                    Message::Notification(Notification { method, params, .. })
                        if method == "$/progress" =>
                    {
                        if let Some(params) = params
                            && let Ok(params) = serde_json::from_value::<ProgressParams>(params)
                        {
                            if handle_progress_notification(&progress, params) {
                                globals::request_inlay_hint_retry();
                            }
                            globals::request_notification_redraw();
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn buffer_attachment(&self, buffer_id: BufferId) -> Option<BufferAttachment> {
        self.attachments
            .lock()
            .ok()
            .and_then(|attachments| attachments.get(&buffer_id).cloned())
    }

    fn contains_buffer(&self, buffer_id: BufferId) -> bool {
        self.attachments
            .lock()
            .ok()
            .is_some_and(|attachments| attachments.contains_key(&buffer_id))
    }

    fn remove_buffer(&self, buffer_id: BufferId) -> Option<BufferAttachment> {
        let attachment = self
            .attachments
            .lock()
            .ok()
            .and_then(|mut attachments| attachments.remove(&buffer_id));

        if let Some(attachment) = attachment.as_ref()
            && let Ok(mut map) = self.uri_to_buffer.lock()
        {
            map.remove(&attachment.uri);
        }

        attachment
    }

    fn initialize(&self) -> io::Result<InitializeResult> {
        let root_uri = file_uri_string(&self.root)?;
        let params = initialize_params(
            root_uri.as_str(),
            self.root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("workspace"),
            self.initialization_options.clone(),
        );

        let result = self.request_raw("initialize", Some(params))?;
        let Some(result) = result else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "initialize returned no result",
            ));
        };

        let initialize_result = serde_json::from_value::<InitializeResult>(result)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        self.notify("initialized", Some(json!({})))?;
        Ok(initialize_result)
    }

    fn sync_buffer(
        &mut self,
        buffer_id: BufferId,
        path: &Path,
        generation: u64,
        syntax_name: &str,
    ) {
        if !matches!(self.state, LspSessionState::Running) {
            return;
        }

        if matches!(
            self.negotiated.text_document_sync,
            Some(TextDocumentSyncKind::NONE)
        ) {
            return;
        }

        let Some(uri) = file_uri_string(path).ok() else {
            return;
        };

        let attachment_exists = self
            .attachments
            .lock()
            .ok()
            .is_some_and(|attachments| attachments.contains_key(&buffer_id));

        if !attachment_exists {
            let Some(lines) = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())
            else {
                return;
            };
            let text = buffer_text_from_lines(&lines);
            let params = json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": syntax_name,
                    "version": 1,
                    "text": text,
                }
            });
            self.notify("textDocument/didOpen", Some(params)).ok();
            if let Ok(mut attachments) = self.attachments.lock() {
                attachments.insert(
                    buffer_id,
                    BufferAttachment {
                        uri: uri.clone(),
                        version: 1,
                        generation,
                        language_id: syntax_name.to_string(),
                    },
                );
            }
            if let Ok(mut map) = self.uri_to_buffer.lock() {
                map.insert(uri, buffer_id);
            }
            return;
        }

        if let Ok(mut attachments) = self.attachments.lock()
            && let Some(attachment) = attachments.get_mut(&buffer_id)
            && attachment.generation != generation
        {
            let Some(lines) = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())
            else {
                return;
            };
            let text = buffer_text_from_lines(&lines);
            attachment.version = attachment.version.saturating_add(1);
            attachment.generation = generation;
            let params = json!({
                "textDocument": {
                    "uri": attachment.uri,
                    "version": attachment.version,
                },
                "contentChanges": [{"text": text}],
            });
            self.notify("textDocument/didChange", Some(params)).ok();
        }
    }

    pub fn did_save_buffer(&self, buffer_id: BufferId) {
        if !matches!(self.state, LspSessionState::Running) {
            return;
        }

        let Some(attachment) = self.buffer_attachment(buffer_id) else {
            return;
        };

        let params = json!({
            "textDocument": {
                "uri": attachment.uri,
            }
        });
        self.notify("textDocument/didSave", Some(params)).ok();
    }

    fn supports_hover(&self) -> bool {
        matches!(
            self.negotiated
                .server_capabilities
                .as_ref()
                .and_then(|capabilities| { capabilities.hover_provider.as_ref() }),
            Some(lsp_types::HoverProviderCapability::Simple(true))
                | Some(lsp_types::HoverProviderCapability::Options(_))
        )
    }

    fn supports_completion(&self) -> bool {
        self.negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.completion_provider.as_ref())
            .is_some()
    }

    fn supports_definition(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.definition_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn supports_document_symbols(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.document_symbol_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn supports_references(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.references_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn supports_workspace_symbols(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.workspace_symbol_provider.as_ref())
        {
            Some(OneOf::Left(enabled)) => *enabled,
            Some(OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn supports_rename(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.rename_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn supports_inlay_hints(&self) -> bool {
        self.server_supports_inlay_hints() && self.config_inlay_hints_enabled()
    }

    fn has_active_progress(&self) -> bool {
        self.progress
            .lock()
            .is_ok_and(|progress| progress.has_active_progress())
    }

    fn server_supports_inlay_hints(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.inlay_hint_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    fn config_inlay_hints_enabled(&self) -> bool {
        globals::with_config(|config| config.inlay_hints_enabled()).unwrap_or(true)
    }

    fn supports_code_actions(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.code_action_provider.as_ref())
        {
            Some(CodeActionProviderCapability::Simple(enabled)) => *enabled,
            Some(CodeActionProviderCapability::Options(_)) => true,
            None => false,
        }
    }

    fn supports_prepare_rename(&self) -> bool {
        let Some(capabilities) = self.negotiated.server_capabilities.as_ref() else {
            return false;
        };

        match capabilities.rename_provider.as_ref() {
            Some(lsp_types::OneOf::Right(options)) => options.prepare_provider.unwrap_or(false),
            None => false,
            Some(lsp_types::OneOf::Left(_)) => false,
        }
    }

    fn hover(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<String>, String> {
        if !self.supports_hover() {
            return Err("attached server does not support hover".to_string());
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
            "position": position_to_lsp_json(lines, cursor, self.negotiated.position_encoding.clone())
        });
        let result = self
            .request_raw("textDocument/hover", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let hover = serde_json::from_value::<Option<lsp_types::Hover>>(value)
            .map_err(|error| error.to_string())?;
        Ok(hover.map(|hover| format_hover(&hover.contents)))
    }

    fn completion(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<CompletionCandidate>>, String> {
        if !self.supports_completion() {
            return Ok(None);
        }

        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: attachment
                        .uri
                        .parse::<lsp_types::Uri>()
                        .map_err(|error| error.to_string())?,
                },
                position: cursor_to_lsp_position(
                    lines,
                    cursor,
                    self.negotiated.position_encoding.clone(),
                ),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        let result = self
            .request_raw(
                "textDocument/completion",
                Some(serde_json::to_value(params).map_err(|error| error.to_string())?),
            )
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<CompletionResponse>>(value)
            .map_err(|error| error.to_string())?;
        let Some(response) = response else {
            return Ok(None);
        };

        let items = completion_response_to_candidates(
            response,
            lines,
            cursor,
            self.negotiated.position_encoding.clone(),
        );
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    fn resolve_completion_additional_text_edits(
        &mut self,
        lines: &PieceTable,
        item: &serde_json::Value,
    ) -> Result<Option<Vec<crate::ui::completion::CompletionTextEdit>>, String> {
        let result = self
            .request_raw("completionItem/resolve", Some(item.clone()))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let item =
            serde_json::from_value::<CompletionItem>(value).map_err(|error| error.to_string())?;
        Ok(Some(completion_item_additional_text_edits(
            &item,
            lines,
            self.negotiated.position_encoding.clone(),
        )))
    }

    fn definition(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<(BufferId, crate::buffer::Cursor)>, String> {
        if !self.supports_definition() {
            return Err("attached server does not support go to definition".to_string());
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
            "position": position_to_lsp_json(lines, cursor, self.negotiated.position_encoding.clone())
        });
        let result = self
            .request_raw("textDocument/definition", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<lsp_types::GotoDefinitionResponse>>(value)
            .map_err(|error| error.to_string())?;
        let Some(response) = response else {
            return Ok(None);
        };

        let Some((uri, position)) = first_definition_target(response) else {
            return Ok(None);
        };

        let path = uri_to_file_path(&uri)?;
        let buffer_id = crate::globals::open_buffer(&path).map_err(|error| error.to_string())?;
        let lines = crate::globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())
            .ok_or_else(|| "failed to read definition target buffer".to_string())?;
        let cursor =
            position_to_cursor(&lines, position, self.negotiated.position_encoding.clone())
                .ok_or_else(|| "failed to convert definition location".to_string())?;

        Ok(Some((buffer_id, cursor)))
    }

    fn references(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<ReferenceItem>>, String> {
        if !self.supports_references() {
            return Err("attached server does not support references".to_string());
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
            "position": position_to_lsp_json(lines, cursor, self.negotiated.position_encoding.clone()),
            "context": ReferenceContext { include_declaration: true },
        });
        let result = self
            .request_raw("textDocument/references", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<Vec<Location>>>(value)
            .map_err(|error| error.to_string())?;
        let Some(locations) = response else {
            return Ok(None);
        };

        let items =
            locations_to_reference_items(locations, self.negotiated.position_encoding.clone());
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    fn document_symbols(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        if !self.supports_document_symbols() {
            return Err("attached server does not support document symbols".to_string());
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
        });
        let result = self
            .request_raw("textDocument/documentSymbol", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<lsp_types::DocumentSymbolResponse>>(value)
            .map_err(|error| error.to_string())?;
        let Some(response) = response else {
            return Ok(None);
        };

        let path = uri_to_file_path(&attachment.uri)?;
        let items = flatten_document_symbol_response(
            response,
            path,
            lines,
            self.negotiated.position_encoding.clone(),
        );
        Ok(Some(items))
    }

    fn document_symbols_tree(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
    ) -> Result<Option<Vec<DocumentSymbolTree>>, String> {
        if !self.supports_document_symbols() {
            return Err("attached server does not support document symbols".to_string());
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
        });
        let result = self
            .request_raw("textDocument/documentSymbol", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<lsp_types::DocumentSymbolResponse>>(value)
            .map_err(|error| error.to_string())?;
        let Some(response) = response else {
            return Ok(None);
        };

        let path = uri_to_file_path(&attachment.uri)?;
        let nodes = build_document_symbol_nodes(
            response,
            path,
            lines,
            self.negotiated.position_encoding.clone(),
        );
        Ok(Some(nodes))
    }

    fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> Result<Option<Vec<DocumentSymbolItem>>, String> {
        if !self.supports_workspace_symbols() {
            return Err("attached server does not support workspace symbols".to_string());
        }

        let params = json!({
            "query": query,
        });
        let result = self
            .request_raw("workspace/symbol", Some(params))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let response = serde_json::from_value::<Option<WorkspaceSymbolResponse>>(value)
            .map_err(|error| error.to_string())?;
        let Some(response) = response else {
            return Ok(None);
        };

        let items: Vec<DocumentSymbolItem> = match response {
            WorkspaceSymbolResponse::Flat(symbols) => symbols
                .into_iter()
                .filter_map(|symbol| workspace_symbol_information_to_item(symbol))
                .collect(),
            WorkspaceSymbolResponse::Nested(symbols) => symbols
                .into_iter()
                .filter_map(|symbol| workspace_symbol_to_item(symbol))
                .collect(),
        };

        Ok(Some(items))
    }

    fn rename(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
        new_name: &str,
    ) -> Result<(bool, Vec<WorkspaceResourceOperationEffect>), String> {
        if !self.supports_rename() {
            return Err("attached server does not support rename".to_string());
        }

        let position_json =
            position_to_lsp_json(lines, cursor, self.negotiated.position_encoding.clone());

        if self.supports_prepare_rename() {
            let prepare_position_json = position_json.clone();
            let prepare_params = json!({
                "textDocument": { "uri": attachment.uri },
                "position": prepare_position_json,
            });
            let prepared = self
                .request_raw("textDocument/prepareRename", Some(prepare_params))
                .map_err(|error| error.to_string())?;
            let Some(value) = prepared else {
                return Err("rename is not valid at the current cursor position".to_string());
            };

            let prepared =
                serde_json::from_value::<Option<lsp_types::PrepareRenameResponse>>(value)
                    .map_err(|error| error.to_string())?;
            if prepared.is_none() {
                return Err("rename is not valid at the current cursor position".to_string());
            }
        }

        let params = json!({
            "textDocument": { "uri": attachment.uri },
            "position": position_json,
            "newName": new_name,
        });
        let result = self
            .request_raw("textDocument/rename", Some(params))
            .map_err(|error| error.to_string())?;
        let Some(value) = result else {
            return Err("rename returned no workspace edit".to_string());
        };

        let edit = serde_json::from_value::<Option<lsp_types::WorkspaceEdit>>(value)
            .map_err(|error| error.to_string())?;
        let Some(edit) = edit else {
            return Err("rename returned no workspace edit".to_string());
        };

        let effects = apply_workspace_edit(&edit, self.negotiated.position_encoding.clone())?;
        Ok((true, effects))
    }

    fn rename_placeholder(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Option<String> {
        if !self.supports_rename() {
            return None;
        }

        let position_json =
            position_to_lsp_json(lines, cursor, self.negotiated.position_encoding.clone());

        if !self.supports_prepare_rename() {
            return None;
        }

        let prepare_params = json!({
            "textDocument": { "uri": attachment.uri },
            "position": position_json,
        });
        let prepared = self
            .request_raw("textDocument/prepareRename", Some(prepare_params))
            .ok()??;

        let prepared = serde_json::from_value::<Option<PrepareRenameResponse>>(prepared).ok()??;
        match prepared {
            PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } => {
                if placeholder.trim().is_empty() {
                    range_text(lines, &range, self.negotiated.position_encoding.clone())
                } else {
                    Some(placeholder)
                }
            }
            PrepareRenameResponse::Range(range) => {
                range_text(lines, &range, self.negotiated.position_encoding.clone())
            }
            PrepareRenameResponse::DefaultBehavior { .. } => None,
        }
    }

    fn code_actions(
        &mut self,
        buffer_id: BufferId,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: crate::buffer::Cursor,
    ) -> Result<Option<Vec<CodeActionApplication>>, String> {
        if !self.supports_code_actions() {
            return Err("attached server does not support code actions".to_string());
        }

        let position =
            cursor_to_lsp_position(lines, cursor, self.negotiated.position_encoding.clone());
        let range_start = position.clone();
        let diagnostics = globals::with_diagnostics_store(|store| {
            store.diagnostics_at_buffer_cursor(buffer_id, cursor)
        })
        .ok_or_else(|| "LSP runtime is not available".to_string())?;
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: attachment
                    .uri
                    .parse::<lsp_types::Uri>()
                    .map_err(|error| error.to_string())?,
            },
            range: lsp_types::Range::new(range_start, position),
            context: CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: Some(CodeActionTriggerKind::INVOKED),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .request_raw(
                "textDocument/codeAction",
                Some(serde_json::to_value(params).map_err(|error| error.to_string())?),
            )
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let actions = serde_json::from_value::<Option<Vec<CodeActionOrCommand>>>(value)
            .map_err(|error| error.to_string())?;
        let Some(actions) = actions else {
            return Ok(None);
        };

        let actions = actions
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

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    fn request_inlay_hints_for_range(
        &mut self,
        _attachment: &BufferAttachment,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: PositionEncodingKind,
    ) -> Result<Option<Vec<InlayHint>>, String> {
        if !self.config_inlay_hints_enabled() {
            return Ok(None);
        }

        if !self.server_supports_inlay_hints() {
            return Err("attached server does not support inlay hints".to_string());
        }

        let Some(range) = line_range_to_lsp_range(lines, start_line, end_line, encoding.clone())
        else {
            return Ok(None);
        };

        let params = InlayHintParams {
            work_done_progress_params: Default::default(),
            text_document: TextDocumentIdentifier {
                uri: uri
                    .parse::<lsp_types::Uri>()
                    .map_err(|error| error.to_string())?,
            },
            range,
        };
        let result = self
            .request_raw(
                "textDocument/inlayHint",
                Some(serde_json::to_value(params).map_err(|error| error.to_string())?),
            )
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let hints = serde_json::from_value::<Option<Vec<InlayHint>>>(value)
            .map_err(|error| error.to_string())?;
        Ok(hints.map(|hints| {
            hints
                .into_iter()
                .filter(|hint| self.inlay_hint_enabled(hint.kind.as_ref()))
                .collect()
        }))
    }

    fn inlay_hint_snapshot(
        &self,
        buffer_id: BufferId,
        attachment: &BufferAttachment,
        lines: &PieceTable,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        if !self.config_inlay_hints_enabled() {
            return Ok(None);
        }

        if !self.server_supports_inlay_hints() {
            return Err("attached server does not support inlay hints".to_string());
        }

        Ok(Some(LspInlayHintSnapshot {
            buffer_id,
            uri: attachment.uri.clone(),
            lines: lines.clone(),
            position_encoding: self.negotiated.position_encoding.clone(),
        }))
    }

    fn inlay_hint_enabled(&self, kind: Option<&InlayHintKind>) -> bool {
        let Some(kind) = kind else {
            return true;
        };

        globals::with_config(|config| match kind {
            k if k == &InlayHintKind::TYPE => {
                config.inlay_hint_kind_enabled(&InlayHintCapability::Type)
            }
            k if k == &InlayHintKind::PARAMETER => {
                config.inlay_hint_kind_enabled(&InlayHintCapability::Parameter)
            }
            _ => false,
        })
        .unwrap_or(true)
    }

    fn apply_code_action_edit(
        &mut self,
        action: &CodeActionApplication,
    ) -> Result<(bool, Vec<WorkspaceResourceOperationEffect>), String> {
        let mut effects = Vec::new();

        if let Some(edit) = action.edit.as_ref() {
            effects = apply_workspace_edit(edit, self.negotiated.position_encoding.clone())?;
        }

        Ok((!effects.is_empty(), effects))
    }

    fn execute_command(
        &mut self,
        command: &str,
        arguments: Option<Vec<serde_json::Value>>,
    ) -> Result<(), String> {
        let params = json!({
            "command": command,
            "arguments": arguments,
        });
        self.request_raw("workspace/executeCommand", Some(params))
            .map_err(|error| error.to_string())?;

        Ok(())
    }

    fn cleanup_detached_buffers(&mut self, live_buffers: &BTreeSet<BufferId>) {
        let to_remove = self
            .attachments
            .lock()
            .ok()
            .map(|attachments| {
                attachments
                    .keys()
                    .copied()
                    .filter(|buffer_id| !live_buffers.contains(buffer_id))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for buffer_id in to_remove {
            if let Some(attachment) = self.remove_buffer(buffer_id) {
                let params = json!({"textDocument": {"uri": attachment.uri}});
                self.notify("textDocument/didClose", Some(params)).ok();
                if let Ok(mut map) = self.uri_to_buffer.lock() {
                    map.remove(&attachment.uri);
                }
                globals::with_diagnostics_store(|store| store.clear(buffer_id, &self.server_name));
            }
        }
    }

    fn apply_workspace_file_operation(&mut self, effect: &WorkspaceResourceOperationEffect) {
        match effect {
            WorkspaceResourceOperationEffect::Create { path } => {
                let _ = path;
            }
            WorkspaceResourceOperationEffect::Rename { old_path, new_path } => {
                self.rename_buffer_attachment(old_path, new_path);
            }
            WorkspaceResourceOperationEffect::Delete { path, buffer_id } => {
                let _ = path;
                if let Some(buffer_id) = buffer_id {
                    self.close_buffer_attachment(*buffer_id);
                }
            }
        }
    }

    fn close_buffer_attachment(&mut self, buffer_id: BufferId) {
        let Some(attachment) = self.remove_buffer(buffer_id) else {
            return;
        };

        let params = json!({"textDocument": {"uri": attachment.uri}});
        self.notify("textDocument/didClose", Some(params)).ok();
        globals::with_diagnostics_store(|store| store.clear(buffer_id, &self.server_name));
    }

    fn rename_buffer_attachment(&mut self, old_path: &Path, new_path: &Path) {
        let Some(old_uri) = file_uri_string(old_path).ok() else {
            return;
        };
        let Some(new_uri) = file_uri_string(new_path).ok() else {
            return;
        };

        let Some(buffer_id) = self
            .uri_to_buffer
            .lock()
            .ok()
            .and_then(|map| map.get(old_uri.as_str()).copied())
        else {
            return;
        };

        let Some(mut attachment) = self.buffer_attachment(buffer_id) else {
            return;
        };

        attachment.uri = new_uri.clone();
        attachment.version = 1;

        if let Ok(mut attachments) = self.attachments.lock() {
            attachments.insert(buffer_id, attachment.clone());
        }

        if let Ok(mut map) = self.uri_to_buffer.lock() {
            map.remove(old_uri.as_str());
            map.insert(new_uri.clone(), buffer_id);
        }

        let close_params = json!({"textDocument": {"uri": old_uri}});
        self.notify("textDocument/didClose", Some(close_params))
            .ok();

        let language_id = attachment.language_id.clone();
        let Some(lines) = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot()) else {
            return;
        };
        let text = buffer_text_from_lines(&lines);

        let open_params = json!({
            "textDocument": {
                "uri": new_uri,
                "languageId": language_id,
                "version": attachment.version,
                "text": text,
            }
        });
        self.notify("textDocument/didOpen", Some(open_params)).ok();
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.state = LspSessionState::ShuttingDown;
        let response = self.request_raw("shutdown", None)?;
        if response.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "shutdown returned no result",
            ));
        }
        self.notify("exit", None)?;
        self.child.kill().ok();
        if let Ok(mut progress) = self.progress.lock() {
            progress.clear();
        }
        Ok(())
    }

    fn request_raw(&self, method: &str, params: Option<Value>) -> io::Result<Option<Value>> {
        let id = RequestId::Number(self.next_request_id.fetch_add(1, Ordering::SeqCst));
        let request = Message::Request(Request::new(id.clone(), method, params));
        let (tx, rx) = mpsc::channel();
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(id.clone(), tx);
        }
        self.write_message(&request)?;
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Message::Response(Response::Success(SuccessResponse { result, .. }))) => {
                Ok(Some(result))
            }
            Ok(Message::Response(Response::Error(ErrorResponse { error, .. }))) => {
                Err(io::Error::new(io::ErrorKind::Other, error.message))
            }
            Ok(_) => Ok(None),
            Err(_) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for LSP response",
                ))
            }
        }
    }

    fn notify(&self, method: &str, params: Option<Value>) -> io::Result<()> {
        let message = Message::Notification(Notification::new(method, params));
        self.write_message(&message)
    }

    fn write_message(&self, message: &Message) -> io::Result<()> {
        let bytes = encode_message(message)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "stdin lock poisoned"))?;
        stdin.write_all(&bytes)?;
        stdin.flush()?;
        Ok(())
    }

    fn record_initialize_result(&mut self, result: &InitializeResult) {
        self.negotiated.server_capabilities = Some(result.capabilities.clone());
        self.negotiated.position_encoding = result
            .capabilities
            .position_encoding
            .clone()
            .unwrap_or(PositionEncodingKind::UTF16);
        if let Ok(mut encoding) = self.position_encoding.lock() {
            *encoding = self.negotiated.position_encoding.clone();
        }
        self.negotiated.text_document_sync = resolve_text_document_sync(&result.capabilities);
    }
}

impl ServerProgressState {
    fn set_begin(&mut self, token: String, begin: WorkDoneProgressBegin) {
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.entries.insert(
            token,
            ServerProgressEntry {
                sequence: self.next_sequence,
                title: begin.title,
                message: begin.message,
                percentage: begin.percentage,
            },
        );
    }

    fn set_report(&mut self, token: String, report: WorkDoneProgressReport) {
        self.next_sequence = self.next_sequence.saturating_add(1);
        let entry = self.entries.entry(token).or_insert(ServerProgressEntry {
            sequence: 0,
            title: String::new(),
            message: None,
            percentage: None,
        });
        entry.sequence = self.next_sequence;
        if let Some(message) = report.message {
            entry.message = Some(message);
        }
        if let Some(percentage) = report.percentage {
            entry.percentage = Some(percentage);
        }
    }

    fn clear_token(&mut self, token: &str) {
        self.entries.remove(token);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn current_message(&self) -> Option<String> {
        self.entries
            .values()
            .max_by_key(|entry| entry.sequence)
            .map(|entry| {
                format_progress_message(&entry.title, entry.message.as_deref(), entry.percentage)
            })
    }

    fn has_active_progress(&self) -> bool {
        !self.entries.is_empty()
    }
}

fn handle_progress_notification(
    progress: &Arc<Mutex<ServerProgressState>>,
    params: ProgressParams,
) -> bool {
    let Some(token) = progress_token_key(&params.token) else {
        return false;
    };

    let Ok(mut progress) = progress.lock() else {
        return false;
    };

    match params.value {
        ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(begin)) => {
            progress.set_begin(token, begin);
            false
        }
        ProgressParamsValue::WorkDone(WorkDoneProgress::Report(report)) => {
            progress.set_report(token, report);
            false
        }
        ProgressParamsValue::WorkDone(WorkDoneProgress::End(end)) => {
            if let Some(message) = end.message {
                progress.set_report(
                    token.clone(),
                    WorkDoneProgressReport {
                        cancellable: None,
                        message: Some(message),
                        percentage: Some(100),
                    },
                );
            }
            progress.clear_token(&token);
            true
        }
    }
}

fn progress_token_key(token: &lsp_types::ProgressToken) -> Option<String> {
    match token {
        lsp_types::ProgressToken::String(value) => Some(format!("s:{value}")),
        lsp_types::ProgressToken::Number(value) => Some(format!("n:{value}")),
    }
}

fn format_progress_message(title: &str, message: Option<&str>, percentage: Option<u32>) -> String {
    let mut parts = Vec::new();
    let title = title.trim();
    if !title.is_empty() {
        parts.push(title.to_string());
    }

    if let Some(message) = message.map(str::trim).filter(|message| !message.is_empty()) {
        parts.push(message.to_string());
    }

    let mut text = parts.join(" ");
    if let Some(percentage) = percentage {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!("{}%", percentage.min(100)));
    }

    text
}

fn resolve_text_document_sync(capabilities: &ServerCapabilities) -> Option<TextDocumentSyncKind> {
    match capabilities.text_document_sync.as_ref()? {
        TextDocumentSyncCapability::Kind(kind) => Some(*kind),
        TextDocumentSyncCapability::Options(options) => options.change,
    }
}

fn resolve_workspace_root(path: &Path, root_markers: &[String]) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        if root_markers
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            return Some(current.to_path_buf());
        }

        let Some(parent) = current.parent() else {
            return Some(path.parent().unwrap_or(current).to_path_buf());
        };

        current = parent;
    }
}

fn file_uri_string(path: &Path) -> io::Result<String> {
    let url = url::Url::from_file_path(path)
        .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "invalid file path"))?;
    Ok(url.to_string())
}

fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    let url = url::Url::parse(uri).map_err(|error| error.to_string())?;
    url.to_file_path()
        .map_err(|()| "LSP URI is not a file path".to_string())
}

fn buffer_text_from_lines(lines: &PieceTable) -> String {
    lines.text().to_text()
}

fn convert_diagnostic(
    lines: &PieceTable,
    diagnostic: Diagnostic,
    encoding: PositionEncodingKind,
) -> Option<Diagnostic> {
    let start = position_to_cursor(lines, diagnostic.range.start, encoding.clone())?;
    let end = position_to_cursor(lines, diagnostic.range.end, encoding)?;
    let mut diagnostic = diagnostic;
    diagnostic.range = lsp_types::Range::new(
        lsp_types::Position::new(start.line as u32, start.col as u32),
        lsp_types::Position::new(end.line as u32, end.col as u32),
    );
    Some(diagnostic)
}

fn position_to_lsp_json(
    lines: &PieceTable,
    cursor: crate::buffer::Cursor,
    encoding: PositionEncodingKind,
) -> Value {
    let position = cursor_to_lsp_position(lines, cursor, encoding);
    json!({"line": position.line, "character": position.character})
}

fn line_range_to_lsp_range(
    lines: &PieceTable,
    start_line: usize,
    end_line: usize,
    encoding: PositionEncodingKind,
) -> Option<lsp_types::Range> {
    lines
        .line_range_for_lines(start_line, end_line, text_encoding_from_lsp(encoding))
        .map(Into::into)
}

fn cursor_to_lsp_position(
    lines: &PieceTable,
    cursor: crate::buffer::Cursor,
    encoding: PositionEncodingKind,
) -> lsp_types::Position {
    lines
        .position_for_cursor(cursor, text_encoding_from_lsp(encoding))
        .map(Into::into)
        .unwrap_or_else(|| lsp_types::Position::new(cursor.line as u32, 0))
}

fn position_to_cursor(
    lines: &PieceTable,
    position: lsp_types::Position,
    encoding: PositionEncodingKind,
) -> Option<crate::buffer::Cursor> {
    lines.cursor_for_position(position.into(), text_encoding_from_lsp(encoding))
}

fn range_text(
    lines: &PieceTable,
    range: &lsp_types::Range,
    encoding: PositionEncodingKind,
) -> Option<String> {
    let range = lines.cursors_for_range((*range).into(), text_encoding_from_lsp(encoding))?;
    lines
        .range(range.start, range.end)
        .map(|text| text.to_text())
}

fn completion_response_to_candidates(
    response: CompletionResponse,
    lines: &PieceTable,
    cursor: crate::buffer::Cursor,
    encoding: PositionEncodingKind,
) -> Vec<CompletionCandidate> {
    let items = match response {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };

    let query = current_word_prefix_text(lines, cursor);
    let mut items = items;
    rank_completion_items(&mut items, query.as_str());

    items
        .into_iter()
        .filter_map(|item| completion_item_to_candidate(item, lines, cursor, encoding.clone()))
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut deduped, item| {
            if let Some(existing) = deduped
                .iter_mut()
                .find(|existing| completion_candidate_same_identity(existing, &item))
            {
                if completion_candidate_score(&item) > completion_candidate_score(existing) {
                    *existing = item;
                }
            } else {
                deduped.push(item);
            }
            deduped
        })
}

fn rank_completion_items(items: &mut Vec<CompletionItem>, query: &str) {
    if query.trim().is_empty() {
        return;
    }

    let query = query.to_lowercase();
    items.retain(|item| {
        item.filter_text
            .as_deref()
            .unwrap_or(item.label.as_str())
            .to_lowercase()
            .starts_with(query.as_str())
    });
    items.sort_by(|left, right| {
        let left_sort = left
            .sort_text
            .as_deref()
            .unwrap_or(left.label.as_str())
            .to_lowercase();
        let right_sort = right
            .sort_text
            .as_deref()
            .unwrap_or(right.label.as_str())
            .to_lowercase();
        match left_sort.cmp(&right_sort) {
            std::cmp::Ordering::Equal => left.label.to_lowercase().cmp(&right.label.to_lowercase()),
            ordering => ordering,
        }
    });
}

fn completion_item_to_candidate(
    item: CompletionItem,
    lines: &PieceTable,
    cursor: crate::buffer::Cursor,
    encoding: PositionEncodingKind,
) -> Option<CompletionCandidate> {
    let deprecated = completion_item_is_deprecated(&item);
    let additional_text_edits =
        completion_item_additional_text_edits(&item, lines, encoding.clone());
    let completion_item_json = serde_json::to_value(&item).ok();
    let label = item.label;
    let label_details = item.label_details;
    let (range, replacement) = match item.text_edit {
        Some(lsp_types::CompletionTextEdit::Edit(edit)) => (
            lsp_range_to_cursor_range(lines, &edit.range, encoding.clone())?,
            edit.new_text,
        ),
        Some(lsp_types::CompletionTextEdit::InsertAndReplace(edit)) => (
            lsp_range_to_cursor_range(lines, &edit.replace, encoding.clone())?,
            edit.new_text,
        ),
        None => {
            let replacement = item.insert_text.unwrap_or_else(|| label.clone());
            (current_word_range(lines, cursor), replacement)
        }
    };

    let mut candidate = CompletionCandidate::new(label, replacement, range, None);
    candidate.kind = item.kind;
    candidate.insert_format = item.insert_text_format.map(|format| match format {
        lsp_types::InsertTextFormat::PLAIN_TEXT => CompletionInsertFormat::PlainText,
        lsp_types::InsertTextFormat::SNIPPET => CompletionInsertFormat::Snippet,
        _ => CompletionInsertFormat::PlainText,
    });
    candidate.detail = item.detail;
    candidate.additional_text_edits = additional_text_edits;
    candidate.lsp_completion_item = completion_item_json;
    candidate.label_detail = label_details
        .as_ref()
        .and_then(|details| details.detail.clone());
    candidate.label_description = label_details
        .as_ref()
        .and_then(|details| details.description.clone());
    candidate.deprecated = deprecated;
    candidate.preselect = item.preselect.unwrap_or(false);

    Some(candidate)
}

fn lsp_range_to_cursor_range(
    lines: &PieceTable,
    range: &lsp_types::Range,
    encoding: PositionEncodingKind,
) -> Option<crate::buffer::TextObjectRange> {
    Some(crate::buffer::TextObjectRange {
        start: position_to_cursor(lines, range.start, encoding.clone())?,
        end: position_to_cursor(lines, range.end, encoding)?,
    })
}

fn completion_candidate_same_identity(
    left: &CompletionCandidate,
    right: &CompletionCandidate,
) -> bool {
    left.label == right.label
        && left.replacement == right.replacement
        && left.range == right.range
        && left.kind == right.kind
        && left.symbol == right.symbol
        && left.insert_format == right.insert_format
}

fn completion_candidate_score(candidate: &CompletionCandidate) -> usize {
    candidate.additional_text_edits.len() + usize::from(candidate.lsp_completion_item.is_some())
}

fn completion_item_additional_text_edits(
    item: &CompletionItem,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<crate::ui::completion::CompletionTextEdit> {
    item.additional_text_edits
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|edit| {
            lsp_range_to_cursor_range(lines, &edit.range, encoding.clone()).map(|range| {
                crate::ui::completion::CompletionTextEdit {
                    range,
                    text: edit.new_text.clone(),
                }
            })
        })
        .collect()
}

fn current_word_prefix_text(lines: &PieceTable, cursor: crate::buffer::Cursor) -> String {
    let Some(line) = lines.line(cursor.line) else {
        return String::new();
    };
    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        start = prev_start;
    }

    line.range_text(start, cursor_col).unwrap_or_default()
}

fn completion_item_is_deprecated(item: &CompletionItem) -> bool {
    if item.deprecated.unwrap_or(false) {
        return true;
    }

    item.tags
        .as_ref()
        .is_some_and(|tags| tags.contains(&lsp_types::CompletionItemTag::DEPRECATED))
}

fn current_word_range(
    lines: &PieceTable,
    cursor: crate::buffer::Cursor,
) -> crate::buffer::TextObjectRange {
    let Some(line) = lines.line(cursor.line) else {
        return crate::buffer::TextObjectRange {
            start: cursor,
            end: cursor,
        };
    };
    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        start = prev_start;
    }

    crate::buffer::TextObjectRange {
        start: crate::buffer::Cursor::new(cursor.line, start),
        end: crate::buffer::Cursor::new(cursor.line, cursor_col),
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn format_hover(contents: &lsp_types::HoverContents) -> String {
    match contents {
        lsp_types::HoverContents::Scalar(marked) => format_marked_string(marked),
        lsp_types::HoverContents::Array(items) => items
            .iter()
            .map(format_marked_string)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
    }
}

fn format_marked_string(marked: &lsp_types::MarkedString) -> String {
    match marked {
        lsp_types::MarkedString::String(text) => text.clone(),
        lsp_types::MarkedString::LanguageString(language) => {
            format!("```{}\n{}\n```", language.language, language.value)
        }
    }
}

fn initialize_params(root_uri: &str, workspace_name: &str, initialization_options: Value) -> Value {
    json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "initializationOptions": initialization_options,
        "capabilities": {
            "workspace": {
                "applyEdit": true
            },
            "textDocument": {
                "completion": {
                    "completionItem": {
                        "snippetSupport": true,
                        "insertReplaceSupport": true,
                        "resolveSupport": {
                            "properties": ["additionalTextEdits"]
                        }
                    }
                },
                "hover": {
                    "contentFormat": ["markdown", "plaintext"]
                },
                "documentSymbol": {
                    "hierarchicalDocumentSymbolSupport": true
                },
                "codeAction": {
                    "codeActionLiteralSupport": {
                        "codeActionKind": {
                            "valueSet": [
                                "",
                                "quickfix",
                                "refactor",
                                "refactor.extract",
                                "refactor.inline",
                                "refactor.rewrite",
                                "source",
                                "source.organizeImports",
                                "source.fixAll"
                            ]
                        }
                    },
                    "isPreferredSupport": true,
                    "disabledSupport": true,
                    "dataSupport": true
                },
                "inlayHint": {
                    "dynamicRegistration": false
                }
            },
            "window": {
                "workDoneProgress": true
            },
            "general": {
                "positionEncodings": ["utf-8", "utf-16"]
            }
        },
        "workspaceFolders": [{
            "uri": root_uri,
            "name": workspace_name
        }]
    })
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

fn flatten_document_symbol_response(
    response: lsp_types::DocumentSymbolResponse,
    path: PathBuf,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<DocumentSymbolItem> {
    let nodes = build_document_symbol_nodes(response, path, lines, encoding);
    flatten_document_symbol_nodes(nodes)
}

#[derive(Debug, Clone)]
pub struct DocumentSymbolTree {
    pub item: DocumentSymbolItem,
    pub children: Vec<DocumentSymbolTree>,
}

/// A single LSP reference location shown by the references picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceItem {
    /// File containing the reference.
    pub path: PathBuf,
    /// Cursor position resolved to urvim byte-column coordinates.
    pub cursor: crate::buffer::Cursor,
    /// Source line text for display.
    pub line_text: String,
}

fn build_document_symbol_nodes(
    response: lsp_types::DocumentSymbolResponse,
    path: PathBuf,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<DocumentSymbolTree> {
    match response {
        lsp_types::DocumentSymbolResponse::Flat(symbols) => symbols
            .into_iter()
            .filter_map(|symbol| {
                let cursor =
                    position_to_cursor(lines, symbol.location.range.start, encoding.clone())?;
                Some(DocumentSymbolTree {
                    item: DocumentSymbolItem {
                        path: path.clone(),
                        cursor,
                        range: None,
                        kind: symbol.kind,
                        name: symbol.name.clone(),
                        detail: None,
                        depth: 0,
                        search_text: document_symbol_search_text(
                            symbol.name.as_str(),
                            None,
                            symbol.kind,
                        ),
                    },
                    children: Vec::new(),
                })
            })
            .collect(),
        lsp_types::DocumentSymbolResponse::Nested(symbols) => {
            build_nested_document_symbol_nodes(symbols, path.as_path(), lines, encoding, &[])
        }
    }
}

fn build_nested_document_symbol_nodes(
    symbols: Vec<lsp_types::DocumentSymbol>,
    path: &Path,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
    ancestors: &[String],
) -> Vec<DocumentSymbolTree> {
    let mut nodes = Vec::new();

    for symbol in symbols {
        let mut next_ancestors = ancestors.to_vec();
        next_ancestors.push(symbol.name.clone());

        if let Some(cursor) =
            position_to_cursor(lines, symbol.selection_range.start, encoding.clone())
        {
            let children = symbol.children.map_or_else(Vec::new, |children| {
                build_nested_document_symbol_nodes(
                    children,
                    path,
                    lines,
                    encoding.clone(),
                    &next_ancestors,
                )
            });

            nodes.push(DocumentSymbolTree {
                item: DocumentSymbolItem {
                    path: path.to_path_buf(),
                    cursor,
                    range: None,
                    kind: symbol.kind,
                    name: symbol.name.clone(),
                    detail: symbol.detail.clone(),
                    depth: ancestors.len(),
                    search_text: document_symbol_search_text(
                        symbol.name.as_str(),
                        symbol.detail.as_deref(),
                        symbol.kind,
                    ),
                },
                children,
            });
        }
    }

    nodes
}

#[cfg(test)]
fn filter_document_symbol_nodes(
    nodes: Vec<DocumentSymbolTree>,
    query: &str,
    fuzzy: bool,
) -> Vec<DocumentSymbolTree> {
    nodes
        .into_iter()
        .filter_map(|node| filter_document_symbol_node(node, query, fuzzy))
        .collect()
}

#[cfg(test)]
fn filter_document_symbol_node(
    node: DocumentSymbolTree,
    query: &str,
    fuzzy: bool,
) -> Option<DocumentSymbolTree> {
    if query.is_empty() {
        return Some(node);
    }

    let children = filter_document_symbol_nodes(node.children, query, fuzzy);
    if document_symbol_matches(node.item.search_text.as_str(), query, fuzzy) || !children.is_empty()
    {
        Some(DocumentSymbolTree {
            item: node.item,
            children,
        })
    } else {
        None
    }
}

fn flatten_document_symbol_nodes(nodes: Vec<DocumentSymbolTree>) -> Vec<DocumentSymbolItem> {
    let mut items = Vec::new();
    flatten_document_symbol_nodes_into(nodes, &mut items);
    items
}

fn flatten_document_symbol_nodes_into(
    nodes: Vec<DocumentSymbolTree>,
    items: &mut Vec<DocumentSymbolItem>,
) {
    for node in nodes {
        items.push(node.item);
        flatten_document_symbol_nodes_into(node.children, items);
    }
}

fn document_symbol_search_text(
    name: &str,
    detail: Option<&str>,
    kind: lsp_types::SymbolKind,
) -> String {
    let mut text = String::new();
    text.push_str(name);
    text.push(' ');
    text.push_str(symbol_kind_label(kind));
    if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
        text.push(' ');
        text.push_str(detail);
    }
    text.to_lowercase()
}

fn workspace_symbol_information_to_item(
    symbol: lsp_types::SymbolInformation,
) -> Option<DocumentSymbolItem> {
    let path = uri_to_file_path(symbol.location.uri.as_str()).ok()?;
    let name = symbol.name;
    let container_name = symbol.container_name;
    let kind = symbol.kind;

    Some(DocumentSymbolItem {
        path,
        cursor: crate::buffer::Cursor::new(0, 0),
        range: Some(symbol.location.range),
        kind,
        name: name.clone(),
        detail: container_name.clone(),
        depth: 0,
        search_text: workspace_symbol_search_text(name.as_str(), container_name.as_deref(), kind),
    })
}

fn workspace_symbol_to_item(symbol: WorkspaceSymbol) -> Option<DocumentSymbolItem> {
    let (uri, range) = match symbol.location {
        OneOf::Left(Location { uri, range }) => (uri, Some(range)),
        OneOf::Right(WorkspaceLocation { uri }) => (uri, None),
    };
    let path = uri_to_file_path(uri.as_str()).ok()?;
    let name = symbol.name;
    let container_name = symbol.container_name;
    let kind = symbol.kind;

    Some(DocumentSymbolItem {
        path,
        cursor: crate::buffer::Cursor::new(0, 0),
        range,
        kind,
        name: name.clone(),
        detail: container_name.clone(),
        depth: 0,
        search_text: workspace_symbol_search_text(name.as_str(), container_name.as_deref(), kind),
    })
}

fn workspace_symbol_search_text(
    name: &str,
    container_name: Option<&str>,
    kind: lsp_types::SymbolKind,
) -> String {
    let mut text = String::new();
    text.push_str(name);
    text.push(' ');
    if let Some(container_name) = container_name.filter(|value| !value.trim().is_empty()) {
        text.push_str(container_name);
        text.push(' ');
    }
    text.push_str(symbol_kind_label(kind));
    text.to_lowercase()
}

fn locations_to_reference_items(
    locations: Vec<Location>,
    encoding: PositionEncodingKind,
) -> Vec<ReferenceItem> {
    locations
        .into_iter()
        .filter_map(|location| location_to_reference_item(location, encoding.clone()))
        .collect()
}

fn location_to_reference_item(
    location: Location,
    encoding: PositionEncodingKind,
) -> Option<ReferenceItem> {
    let path = uri_to_file_path(location.uri.as_str()).ok()?;
    let buffer_id = crate::globals::open_buffer(&path).ok()?;
    let lines = crate::globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())?;
    let cursor = position_to_cursor(&lines, location.range.start, encoding)?;
    let line_text = lines
        .get(cursor.line)
        .map(|line| line.to_text().trim().to_string())
        .unwrap_or_default();

    Some(ReferenceItem {
        path,
        cursor,
        line_text,
    })
}

#[cfg(test)]
fn document_symbol_matches(search_text: &str, query: &str, fuzzy: bool) -> bool {
    if fuzzy {
        fuzzy_matches(query, search_text)
    } else {
        exact_matches(query, search_text)
    }
}

#[cfg(test)]
fn exact_matches(query: &str, candidate: &str) -> bool {
    candidate
        .to_lowercase()
        .contains(query.to_lowercase().as_str())
}

#[cfg(test)]
fn fuzzy_matches(query: &str, candidate: &str) -> bool {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return true;
    };

    for hay in candidate.chars().flat_map(char::to_lowercase) {
        if hay == needle {
            match query_chars.next() {
                Some(next) => needle = next,
                None => return true,
            }
        }
    }

    false
}

fn symbol_kind_label(kind: lsp_types::SymbolKind) -> &'static str {
    match kind {
        lsp_types::SymbolKind::FILE => "file",
        lsp_types::SymbolKind::MODULE => "module",
        lsp_types::SymbolKind::NAMESPACE => "namespace",
        lsp_types::SymbolKind::PACKAGE => "package",
        lsp_types::SymbolKind::CLASS => "class",
        lsp_types::SymbolKind::METHOD => "method",
        lsp_types::SymbolKind::PROPERTY => "property",
        lsp_types::SymbolKind::FIELD => "field",
        lsp_types::SymbolKind::CONSTRUCTOR => "constructor",
        lsp_types::SymbolKind::ENUM => "enum",
        lsp_types::SymbolKind::INTERFACE => "interface",
        lsp_types::SymbolKind::FUNCTION => "function",
        lsp_types::SymbolKind::VARIABLE => "variable",
        lsp_types::SymbolKind::CONSTANT => "constant",
        lsp_types::SymbolKind::STRING => "string",
        lsp_types::SymbolKind::NUMBER => "number",
        lsp_types::SymbolKind::BOOLEAN => "boolean",
        lsp_types::SymbolKind::ARRAY => "array",
        lsp_types::SymbolKind::OBJECT => "object",
        lsp_types::SymbolKind::KEY => "key",
        lsp_types::SymbolKind::NULL => "null",
        lsp_types::SymbolKind::ENUM_MEMBER => "enum-member",
        lsp_types::SymbolKind::STRUCT => "struct",
        lsp_types::SymbolKind::EVENT => "event",
        lsp_types::SymbolKind::OPERATOR => "operator",
        lsp_types::SymbolKind::TYPE_PARAMETER => "type-parameter",
        _ => "symbol",
    }
}

fn apply_workspace_edit(
    edit: &lsp_types::WorkspaceEdit,
    encoding: PositionEncodingKind,
) -> Result<Vec<WorkspaceResourceOperationEffect>, String> {
    let mut effects = Vec::new();

    if let Some(changes) = edit.changes.as_ref() {
        for (uri, edits) in changes {
            apply_text_edits(uri, edits, encoding.clone())?;
        }
    }

    if let Some(changes) = edit.document_changes.as_ref() {
        match changes {
            lsp_types::DocumentChanges::Edits(edits) => {
                let mut grouped = BTreeMap::<String, Vec<lsp_types::TextEdit>>::new();
                for text_document_edit in edits {
                    let uri = text_document_edit.text_document.uri.to_string();
                    let edits = text_document_edit
                        .edits
                        .iter()
                        .map(|edit| match edit {
                            lsp_types::OneOf::Left(text_edit) => Ok(text_edit.clone()),
                            lsp_types::OneOf::Right(annotated) => Ok(annotated.text_edit.clone()),
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    grouped.entry(uri).or_default().extend(edits);
                }

                for (uri, edits) in grouped {
                    let uri = uri
                        .parse::<lsp_types::Uri>()
                        .map_err(|error| error.to_string())?;
                    apply_text_edits(&uri, &edits, encoding.clone())?;
                }
            }
            lsp_types::DocumentChanges::Operations(operations) => {
                for operation in operations {
                    match operation {
                        DocumentChangeOperation::Edit(text_document_edit) => {
                            let uri = text_document_edit.text_document.uri.clone();
                            let edits = text_document_edit
                                .edits
                                .iter()
                                .map(|edit| match edit {
                                    lsp_types::OneOf::Left(text_edit) => Ok(text_edit.clone()),
                                    lsp_types::OneOf::Right(annotated) => {
                                        Ok(annotated.text_edit.clone())
                                    }
                                })
                                .collect::<Result<Vec<_>, String>>()?;
                            apply_text_edits(&uri, &edits, encoding.clone())?;
                        }
                        DocumentChangeOperation::Op(resource_op) => match resource_op {
                            ResourceOp::Create(create) => {
                                let effect = apply_create_file(create)?;
                                effects.push(effect);
                            }
                            ResourceOp::Rename(rename) => {
                                let effect = apply_rename_file(rename)?;
                                effects.push(effect);
                            }
                            ResourceOp::Delete(delete) => {
                                let effect = apply_delete_file(delete)?;
                                effects.push(effect);
                            }
                        },
                    }
                }
            }
        }
    }

    Ok(effects)
}

fn apply_create_file(create: &CreateFile) -> Result<WorkspaceResourceOperationEffect, String> {
    let path = uri_to_file_path(&create.uri.to_string())?;
    let options = create.options.as_ref();
    let exists = path.exists();

    if exists {
        let overwrite = options
            .and_then(|options| options.overwrite)
            .unwrap_or(false);
        let ignore_if_exists = options
            .and_then(|options| options.ignore_if_exists)
            .unwrap_or(false);
        if ignore_if_exists {
            return Ok(WorkspaceResourceOperationEffect::Create { path });
        }
        if !overwrite {
            return Err(format!("file already exists: {}", path.display()));
        }
        let buffer_id = crate::globals::with_buffer_pool(|pool| {
            crate::AbsolutePath::from_path(&path).and_then(|abs| pool.buffer_id_for_path(&abs))
        });
        remove_path(&path)?;
        if let Some(buffer_id) = buffer_id {
            crate::globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
            crate::globals::enqueue_workspace_file_operation_notification(
                crate::globals::WorkspaceFileOperationNotification::Delete {
                    path: path.clone(),
                    buffer_id: Some(buffer_id),
                },
            );
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| error.to_string())?;

    crate::globals::enqueue_workspace_file_operation_notification(
        crate::globals::WorkspaceFileOperationNotification::Create { path: path.clone() },
    );
    crate::session::mark_dirty();
    crate::globals::request_notification_redraw();
    Ok(WorkspaceResourceOperationEffect::Create { path })
}

fn apply_rename_file(rename: &RenameFile) -> Result<WorkspaceResourceOperationEffect, String> {
    let old_path = uri_to_file_path(&rename.old_uri.to_string())?;
    let new_path = uri_to_file_path(&rename.new_uri.to_string())?;
    let options = rename.options.as_ref();

    if !old_path.exists() {
        return Err(format!("file does not exist: {}", old_path.display()));
    }

    let target_exists = new_path.exists();
    if target_exists {
        let overwrite = options
            .and_then(|options| options.overwrite)
            .unwrap_or(false);
        let ignore_if_exists = options
            .and_then(|options| options.ignore_if_exists)
            .unwrap_or(false);
        if ignore_if_exists {
            return Ok(WorkspaceResourceOperationEffect::Rename { old_path, new_path });
        }
        if !overwrite {
            return Err(format!("file already exists: {}", new_path.display()));
        }
        remove_path(&new_path)?;
    }

    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let replaced_buffer_id = crate::globals::with_buffer_pool(|pool| {
        crate::AbsolutePath::from_path(&new_path).and_then(|abs| pool.buffer_id_for_path(&abs))
    });
    fs::rename(&old_path, &new_path).map_err(|error| error.to_string())?;

    if let Some(buffer_id) = replaced_buffer_id {
        crate::globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
        crate::globals::enqueue_workspace_file_operation_notification(
            crate::globals::WorkspaceFileOperationNotification::Delete {
                path: new_path.clone(),
                buffer_id: Some(buffer_id),
            },
        );
    }

    if let Some(source_buffer_id) = crate::globals::with_buffer_pool(|pool| {
        crate::AbsolutePath::from_path(&old_path).and_then(|abs| pool.buffer_id_for_path(&abs))
    }) {
        crate::globals::with_buffer_pool(|pool| {
            pool.rename_buffer_path(source_buffer_id, &new_path)
        })
        .map_err(|error| error.to_string())?;
    }

    crate::globals::enqueue_workspace_file_operation_notification(
        crate::globals::WorkspaceFileOperationNotification::Rename {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
        },
    );
    crate::session::mark_dirty();
    crate::globals::request_notification_redraw();

    Ok(WorkspaceResourceOperationEffect::Rename { old_path, new_path })
}

fn apply_delete_file(delete: &DeleteFile) -> Result<WorkspaceResourceOperationEffect, String> {
    let path = uri_to_file_path(&delete.uri.to_string())?;
    let options = delete.options.as_ref();
    let exists = path.exists();

    if !exists {
        if options
            .and_then(|options| options.ignore_if_not_exists)
            .unwrap_or(false)
        {
            return Ok(WorkspaceResourceOperationEffect::Delete {
                path,
                buffer_id: None,
            });
        }
        return Err(format!("file does not exist: {}", path.display()));
    }

    let buffer_id = crate::globals::with_buffer_pool(|pool| {
        crate::AbsolutePath::from_path(&path).and_then(|abs| pool.buffer_id_for_path(&abs))
    });
    remove_path(&path).map_err(|error| error.to_string())?;

    if let Some(buffer_id) = buffer_id {
        crate::globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
        crate::globals::enqueue_workspace_file_operation_notification(
            crate::globals::WorkspaceFileOperationNotification::Delete {
                path: path.clone(),
                buffer_id: Some(buffer_id),
            },
        );
    }

    crate::session::mark_dirty();
    crate::globals::request_notification_redraw();
    Ok(WorkspaceResourceOperationEffect::Delete { path, buffer_id })
}

fn remove_path(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else {
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

fn apply_text_edits(
    uri: &lsp_types::Uri,
    edits: &[lsp_types::TextEdit],
    encoding: PositionEncodingKind,
) -> Result<(), String> {
    let path = uri_to_file_path(&uri.to_string())?;
    let buffer_id = crate::globals::open_buffer(&path).map_err(|error| error.to_string())?;

    let mut sorted_edits = edits.to_vec();
    sorted_edits.sort_by(|left, right| {
        right
            .range
            .start
            .line
            .cmp(&left.range.start.line)
            .then_with(|| right.range.start.character.cmp(&left.range.start.character))
    });

    let current_text = crate::globals::with_buffer(buffer_id, |buffer| buffer.as_str())
        .ok_or_else(|| "failed to read buffer for workspace edit".to_string())?;
    let updated_text = apply_text_edits_to_string(&current_text, &sorted_edits, encoding.clone())?;

    let applied = crate::globals::with_buffer_mut(buffer_id, |buffer| {
        buffer.replace_text(updated_text.as_str());
        buffer.push_snapshot(buffer.current_cursor());
        true
    })
    .unwrap_or(false);

    if !applied {
        return Err("failed to apply workspace edit".to_string());
    }

    crate::globals::with_buffer_pool(|pool| pool.request_buffer_cache_refresh(buffer_id));

    crate::session::mark_dirty();
    crate::globals::request_notification_redraw();
    Ok(())
}

fn apply_text_edits_to_string(
    text: &str,
    edits: &[lsp_types::TextEdit],
    encoding: PositionEncodingKind,
) -> Result<String, String> {
    let mut current = text.to_string();

    for edit in edits {
        let Some(start) = position_to_byte_offset(&current, edit.range.start, encoding.clone())
        else {
            return Err("failed to convert edit start position".to_string());
        };
        let Some(end) = position_to_byte_offset(&current, edit.range.end, encoding.clone()) else {
            return Err("failed to convert edit end position".to_string());
        };
        if start > end || end > current.len() {
            return Err("workspace edit range is invalid".to_string());
        }
        current.replace_range(start..end, edit.new_text.as_str());
    }

    Ok(current)
}

fn open_lsp_log_stderr() -> Stdio {
    match OpenOptions::new().create(true).append(true).open("lsp.log") {
        Ok(file) => Stdio::from(file),
        Err(error) => {
            tracing::warn!(
                ?error,
                "failed to open debug.log for LSP stderr; discarding stderr"
            );
            Stdio::null()
        }
    }
}

fn read_framed_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut headers = Vec::new();
    let mut content_length = None;

    loop {
        let mut line = Vec::new();
        let bytes = reader.read_until(b'\n', &mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }

        if let Ok(text) = std::str::from_utf8(&line)
            && let Some((name, value)) = text.trim_end().split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            );
        }

        headers.extend_from_slice(&line);
    }

    let content_length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing content length"))?;
    let mut payload = vec![0u8; content_length];
    reader.read_exact(&mut payload)?;

    let mut framed = headers;
    framed.extend_from_slice(b"\r\n");
    framed.extend_from_slice(&payload);
    Ok(Some(framed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, LspConfig, LspServerConfig};
    use lsp_types::{CompletionItem, CompletionItemTag, WorkDoneProgressEnd};
    use std::path::PathBuf;

    fn line_snapshot(text: &str) -> PieceTable {
        PieceTable::from_text(text)
    }

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

    #[allow(deprecated)]
    fn document_symbol(
        name: &str,
        kind: lsp_types::SymbolKind,
        range: lsp_types::Range,
        selection_range: lsp_types::Range,
        children: Option<Vec<lsp_types::DocumentSymbol>>,
    ) -> lsp_types::DocumentSymbol {
        lsp_types::DocumentSymbol {
            name: name.to_string(),
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children,
        }
    }

    #[allow(deprecated)]
    fn symbol_information(
        name: &str,
        kind: lsp_types::SymbolKind,
        uri: &str,
        range: lsp_types::Range,
    ) -> lsp_types::SymbolInformation {
        lsp_types::SymbolInformation {
            name: name.to_string(),
            kind,
            tags: None,
            deprecated: None,
            location: lsp_types::Location {
                uri: uri.parse().expect("uri"),
                range,
            },
            container_name: None,
        }
    }

    #[test]
    fn completion_item_deprecated_uses_flag_or_tags() {
        let flagged = CompletionItem {
            label: "flagged".to_string(),
            deprecated: Some(true),
            ..CompletionItem::default()
        };
        assert!(completion_item_is_deprecated(&flagged));

        let tagged = CompletionItem {
            label: "tagged".to_string(),
            tags: Some(vec![CompletionItemTag::DEPRECATED]),
            ..CompletionItem::default()
        };
        assert!(completion_item_is_deprecated(&tagged));

        let plain = CompletionItem {
            label: "plain".to_string(),
            ..CompletionItem::default()
        };
        assert!(!completion_item_is_deprecated(&plain));
    }

    #[test]
    fn resolve_workspace_root_prefers_nearest_marker() {
        let root = std::env::temp_dir().join(format!(
            "urvim-lsp-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let nested = root.join("src");
        std::fs::create_dir_all(&nested).expect("dirs");
        let marker = root.join("Cargo.toml");
        std::fs::write(&marker, "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n")
            .expect("marker");

        let resolved = resolve_workspace_root(
            nested.join("main.rs").as_path(),
            &["Cargo.toml".to_string()],
        )
        .expect("root");

        assert_eq!(resolved, root);
    }

    #[test]
    fn server_runtime_matches_filetypes() {
        let runtime = ServerRuntime {
            config: LspServerConfig {
                enabled: true,
                command: "rust-analyzer".to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                filetypes: vec!["rust".to_string()],
                root_markers: vec!["Cargo.toml".to_string()],
                settings: toml::Value::Table(Default::default()),
            },
            sessions: BTreeMap::new(),
            failed_sessions: BTreeMap::new(),
            progress: Arc::new(Mutex::new(ServerProgressState::default())),
        };

        assert!(runtime.matches_filetype("rust"));
        assert!(!runtime.matches_filetype("python"));
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
                    settings: toml::Value::Table(Default::default()),
                },
            )]),
        };

        let runtime = LspRuntime::new(&config);
        assert!(runtime.servers.contains_key("rust_analyzer"));
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
                    settings: toml::Value::Table(Default::default()),
                },
            )]),
        };

        let mut runtime = LspRuntime::new(&config);
        let server = runtime
            .servers
            .get_mut("rust_analyzer")
            .expect("server runtime");
        {
            let mut progress = server.progress.lock().expect("progress lock");
            progress.set_begin(
                "token-1".to_string(),
                WorkDoneProgressBegin {
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
    fn progress_notification_updates_server_status() {
        let progress = Arc::new(Mutex::new(ServerProgressState::default()));

        handle_progress_notification(
            &progress,
            ProgressParams {
                token: lsp_types::ProgressToken::String("token-1".to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: "Indexing".to_string(),
                        cancellable: None,
                        message: Some("workspace".to_string()),
                        percentage: Some(12),
                    },
                )),
            },
        );

        assert_eq!(
            progress.lock().expect("progress lock").current_message(),
            Some("Indexing workspace 12%".to_string())
        );

        handle_progress_notification(
            &progress,
            ProgressParams {
                token: lsp_types::ProgressToken::String("token-1".to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: None,
                        message: Some("crate graph".to_string()),
                        percentage: Some(42),
                    },
                )),
            },
        );

        assert_eq!(
            progress.lock().expect("progress lock").current_message(),
            Some("Indexing crate graph 42%".to_string())
        );

        handle_progress_notification(
            &progress,
            ProgressParams {
                token: lsp_types::ProgressToken::String("token-1".to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: None,
                })),
            },
        );

        assert_eq!(
            progress.lock().expect("progress lock").current_message(),
            None
        );
    }

    #[test]
    fn resolve_text_document_sync_prefers_kind_over_options() {
        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            ..Default::default()
        };

        assert_eq!(
            resolve_text_document_sync(&capabilities),
            Some(TextDocumentSyncKind::FULL)
        );
    }

    #[test]
    fn nested_document_symbols_keep_matching_parents() {
        let response = lsp_types::DocumentSymbolResponse::Nested(vec![document_symbol(
            "outer",
            lsp_types::SymbolKind::FUNCTION,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 30),
            ),
            lsp_types::Range::new(
                lsp_types::Position::new(0, 3),
                lsp_types::Position::new(0, 8),
            ),
            Some(vec![document_symbol(
                "inner",
                lsp_types::SymbolKind::FUNCTION,
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 10),
                    lsp_types::Position::new(0, 28),
                ),
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 13),
                    lsp_types::Position::new(0, 18),
                ),
                None,
            )]),
        )]);
        let lines = PieceTable::from_text("fn outer() { fn inner() {} }");
        let nodes = build_document_symbol_nodes(
            response,
            PathBuf::from("/tmp/example.rs"),
            &lines,
            PositionEncodingKind::UTF16,
        );
        let filtered = filter_document_symbol_nodes(nodes, "inner", false);
        let flattened = flatten_document_symbol_nodes(filtered);

        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].name, "outer");
        assert_eq!(flattened[0].depth, 0);
        assert_eq!(flattened[1].name, "inner");
        assert_eq!(flattened[1].depth, 1);
    }

    #[test]
    fn direct_matches_do_not_keep_descendants() {
        let response = lsp_types::DocumentSymbolResponse::Nested(vec![document_symbol(
            "ByteBuffer",
            lsp_types::SymbolKind::STRUCT,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 30),
            ),
            lsp_types::Range::new(
                lsp_types::Position::new(0, 6),
                lsp_types::Position::new(0, 16),
            ),
            Some(vec![document_symbol(
                "push",
                lsp_types::SymbolKind::METHOD,
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 10),
                    lsp_types::Position::new(0, 28),
                ),
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 13),
                    lsp_types::Position::new(0, 17),
                ),
                None,
            )]),
        )]);
        let lines = PieceTable::from_text("struct ByteBuffer { fn push(&self) {} }");
        let nodes = build_document_symbol_nodes(
            response,
            PathBuf::from("/tmp/example.rs"),
            &lines,
            PositionEncodingKind::UTF16,
        );
        let filtered = filter_document_symbol_nodes(nodes, "ByteBuffer", false);
        let flattened = flatten_document_symbol_nodes(filtered);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "ByteBuffer");
        assert_eq!(flattened[0].depth, 0);
    }

    #[test]
    fn direct_parent_match_keeps_matching_children_only() {
        let response = lsp_types::DocumentSymbolResponse::Nested(vec![document_symbol(
            "ByteBuffer",
            lsp_types::SymbolKind::STRUCT,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 30),
            ),
            lsp_types::Range::new(
                lsp_types::Position::new(0, 6),
                lsp_types::Position::new(0, 16),
            ),
            Some(vec![
                document_symbol(
                    "buffer",
                    lsp_types::SymbolKind::FIELD,
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 10),
                        lsp_types::Position::new(0, 20),
                    ),
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 10),
                        lsp_types::Position::new(0, 16),
                    ),
                    None,
                ),
                document_symbol(
                    "len",
                    lsp_types::SymbolKind::FIELD,
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 21),
                        lsp_types::Position::new(0, 28),
                    ),
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 21),
                        lsp_types::Position::new(0, 24),
                    ),
                    None,
                ),
            ]),
        )]);
        let lines = PieceTable::from_text("struct ByteBuffer { buffer: usize, len: usize }");
        let nodes = build_document_symbol_nodes(
            response,
            PathBuf::from("/tmp/example.rs"),
            &lines,
            PositionEncodingKind::UTF16,
        );
        let filtered = filter_document_symbol_nodes(nodes, "buffer", false);
        let flattened = flatten_document_symbol_nodes(filtered);

        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].name, "ByteBuffer");
        assert_eq!(flattened[1].name, "buffer");
    }

    #[test]
    fn fuzzy_matching_keeps_ancestor_chain() {
        let response = lsp_types::DocumentSymbolResponse::Nested(vec![document_symbol(
            "Container",
            lsp_types::SymbolKind::STRUCT,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 30),
            ),
            lsp_types::Range::new(
                lsp_types::Position::new(0, 8),
                lsp_types::Position::new(0, 17),
            ),
            Some(vec![document_symbol(
                "Buffer",
                lsp_types::SymbolKind::STRUCT,
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 10),
                    lsp_types::Position::new(0, 28),
                ),
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 13),
                    lsp_types::Position::new(0, 19),
                ),
                None,
            )]),
        )]);
        let lines = PieceTable::from_text("struct Container { struct Buffer {} }");
        let nodes = build_document_symbol_nodes(
            response,
            PathBuf::from("/tmp/example.rs"),
            &lines,
            PositionEncodingKind::UTF16,
        );
        let filtered = filter_document_symbol_nodes(nodes, "buf", true);
        let flattened = flatten_document_symbol_nodes(filtered);

        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].name, "Container");
        assert_eq!(flattened[1].name, "Buffer");
    }

    #[test]
    fn flat_document_symbols_filter_without_hierarchy() {
        let response = lsp_types::DocumentSymbolResponse::Flat(vec![
            symbol_information(
                "Alpha",
                lsp_types::SymbolKind::FUNCTION,
                "file:///tmp/example.rs",
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 5),
                ),
            ),
            symbol_information(
                "BetaBuffer",
                lsp_types::SymbolKind::FUNCTION,
                "file:///tmp/example.rs",
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 6),
                    lsp_types::Position::new(0, 16),
                ),
            ),
        ]);
        let lines = PieceTable::from_text("Alpha BetaBuffer");
        let nodes = build_document_symbol_nodes(
            response,
            PathBuf::from("/tmp/example.rs"),
            &lines,
            PositionEncodingKind::UTF16,
        );
        let filtered = filter_document_symbol_nodes(nodes, "beta", false);
        let flattened = flatten_document_symbol_nodes(filtered);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "BetaBuffer");
        assert_eq!(flattened[0].depth, 0);
    }

    #[test]
    fn session_spawn_returns_error_for_missing_command() {
        let root = std::env::temp_dir().join(format!(
            "urvim-lsp-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("root");

        let config = LspServerConfig {
            enabled: true,
            command: "definitely-not-a-real-command".to_string(),
            args: Vec::new(),
            env: BTreeMap::new(),
            filetypes: vec!["rust".to_string()],
            root_markers: vec!["Cargo.toml".to_string()],
            settings: toml::Value::Table(Default::default()),
        };

        let error = LspServerSession::spawn(
            "missing",
            &config,
            &root,
            Arc::new(Mutex::new(ServerProgressState::default())),
        )
        .expect_err("missing command should fail to spawn");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn hover_formatter_uses_plain_text_and_code_blocks() {
        let hover = lsp_types::HoverContents::Array(vec![
            lsp_types::MarkedString::String("plain".to_string()),
            lsp_types::MarkedString::LanguageString(lsp_types::LanguageString {
                language: "rust".to_string(),
                value: "fn demo()".to_string(),
            }),
        ]);

        let text = format_hover(&hover);
        assert!(text.contains("plain"));
        assert!(text.contains("```rust"));
    }

    #[test]
    fn initialize_params_advertise_hierarchical_document_symbols() {
        let params = initialize_params("file:///tmp/example.rs", "workspace", Value::Null);

        assert_eq!(
            params["capabilities"]["textDocument"]["documentSymbol"]["hierarchicalDocumentSymbolSupport"],
            true
        );
    }

    #[test]
    fn initialize_params_include_initialization_options() {
        let params = initialize_params(
            "file:///tmp/example.rs",
            "workspace",
            json!({"cargo": {"allTargets": false}}),
        );

        assert_eq!(
            params["initializationOptions"]["cargo"]["allTargets"],
            false
        );
    }

    #[test]
    fn initialize_params_advertise_code_actions() {
        let params = initialize_params("file:///tmp/example.rs", "workspace", Value::Null);

        assert_eq!(params["capabilities"]["workspace"]["applyEdit"], true);
        assert_eq!(
            params["capabilities"]["textDocument"]["codeAction"]["dataSupport"],
            true
        );
        assert_eq!(
            params["capabilities"]["textDocument"]["inlayHint"]["dynamicRegistration"],
            false
        );
        assert_eq!(
            params["capabilities"]["textDocument"]["completion"]["completionItem"]["resolveSupport"]
                ["properties"],
            json!(["additionalTextEdits"])
        );
    }

    #[test]
    fn flatten_document_symbol_response_preserves_nested_depth() {
        let response = lsp_types::DocumentSymbolResponse::Nested(vec![document_symbol(
            "outer",
            lsp_types::SymbolKind::FUNCTION,
            lsp_types::Range::default(),
            lsp_types::Range::default(),
            Some(vec![document_symbol(
                "inner",
                lsp_types::SymbolKind::FUNCTION,
                lsp_types::Range::default(),
                lsp_types::Range::default(),
                None,
            )]),
        )]);
        let items = flatten_document_symbol_response(
            response,
            std::path::PathBuf::from("/tmp/example.rs"),
            &PieceTable::from_text("fn outer() { fn inner() {} }"),
            PositionEncodingKind::UTF16,
        );

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[1].depth, 1);
    }

    #[test]
    fn definition_target_prefers_first_result() {
        let response = lsp_types::GotoDefinitionResponse::Array(vec![
            lsp_types::Location {
                uri: "file:///tmp/one.rs".parse().expect("uri"),
                range: lsp_types::Range::default(),
            },
            lsp_types::Location {
                uri: "file:///tmp/two.rs".parse().expect("uri"),
                range: lsp_types::Range::default(),
            },
        ]);

        let (uri, _) = first_definition_target(response).expect("target");
        assert_eq!(uri, "file:///tmp/one.rs");
    }

    #[test]
    fn completion_response_uses_text_edits_and_insert_text() {
        let mut edit_item =
            lsp_types::CompletionItem::new_simple("edit".to_string(), "".to_string());
        edit_item.text_edit = Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(0, 5),
            },
            new_text: "hi".to_string(),
        }));

        let mut insert_item =
            lsp_types::CompletionItem::new_simple("insert".to_string(), "".to_string());
        insert_item.insert_text = Some("earth".to_string());

        let response = lsp_types::CompletionResponse::Array(vec![edit_item, insert_item]);
        let items = completion_response_to_candidates(
            response,
            &line_snapshot("hello world"),
            crate::buffer::Cursor::new(0, 0),
            PositionEncodingKind::UTF8,
        );

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].replacement, "hi");
        assert_eq!(items[1].replacement, "earth");
        assert!(items[0].lsp_completion_item.is_some());
        assert!(items[1].lsp_completion_item.is_some());
    }

    #[test]
    fn completion_response_prefers_items_with_additional_edits_when_labels_match() {
        let plain = lsp_types::CompletionItem::new_simple("width".to_string(), "width".to_string());
        let mut imported =
            lsp_types::CompletionItem::new_simple("width".to_string(), "width".to_string());
        imported.additional_text_edits = Some(vec![lsp_types::TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(0, 0),
            },
            new_text: "use foo::Width;\n".to_string(),
        }]);

        let items = completion_response_to_candidates(
            CompletionResponse::Array(vec![plain, imported]),
            &line_snapshot(""),
            crate::buffer::Cursor::new(0, 0),
            PositionEncodingKind::UTF8,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "width");
        assert_eq!(items[0].additional_text_edits.len(), 1);
    }

    #[test]
    fn utf16_position_conversion_round_trips_ascii() {
        let lines = line_snapshot("hello\nworld");
        let cursor = crate::buffer::Cursor::new(1, 3);
        let position = cursor_to_lsp_position(&lines, cursor, PositionEncodingKind::UTF16);
        let round_tripped =
            position_to_cursor(&lines, position, PositionEncodingKind::UTF16).expect("cursor");

        assert_eq!(round_tripped, cursor);
    }

    #[test]
    fn unicode_position_conversion_round_trips_utf8_and_utf16() {
        let lines = line_snapshot("a𝄞b\nçd");

        let utf8_cursor = crate::buffer::Cursor::new(0, 5);
        let utf8_position = cursor_to_lsp_position(&lines, utf8_cursor, PositionEncodingKind::UTF8);
        assert_eq!(utf8_position.line, 0);
        assert_eq!(utf8_position.character, 5);
        assert_eq!(
            position_to_cursor(&lines, utf8_position, PositionEncodingKind::UTF8),
            Some(utf8_cursor)
        );

        let utf16_cursor = crate::buffer::Cursor::new(0, 5);
        let utf16_position =
            cursor_to_lsp_position(&lines, utf16_cursor, PositionEncodingKind::UTF16);
        assert_eq!(utf16_position.line, 0);
        assert_eq!(utf16_position.character, 3);
        assert_eq!(
            position_to_cursor(&lines, utf16_position, PositionEncodingKind::UTF16),
            Some(utf16_cursor)
        );
    }

    #[test]
    fn line_range_to_lsp_range_uses_next_line_as_exclusive_end() {
        let lines = line_snapshot("zero\none\ntwo");

        let range =
            line_range_to_lsp_range(&lines, 0, 1, PositionEncodingKind::UTF8).expect("range");

        assert_eq!(range.start, lsp_types::Position::new(0, 0));
        assert_eq!(range.end, lsp_types::Position::new(1, 0));
    }

    #[test]
    fn line_range_to_lsp_range_ends_at_file_end_for_final_chunk() {
        let lines = line_snapshot("zero\none");

        let range =
            line_range_to_lsp_range(&lines, 1, 2, PositionEncodingKind::UTF8).expect("range");

        assert_eq!(range.start, lsp_types::Position::new(1, 0));
        assert_eq!(range.end, lsp_types::Position::new(1, 3));
    }

    #[test]
    fn range_text_extracts_the_rename_seed() {
        let lines = line_snapshot("let renamed = original;");
        let range = lsp_types::Range {
            start: lsp_types::Position::new(0, 4),
            end: lsp_types::Position::new(0, 11),
        };

        assert_eq!(
            range_text(&lines, &range, PositionEncodingKind::UTF8),
            Some("renamed".to_string())
        );
    }

    #[test]
    fn range_text_extracts_multiline_rename_seed() {
        let lines = line_snapshot("alpha\nbeta\ngamma");
        let range = lsp_types::Range {
            start: lsp_types::Position::new(0, 2),
            end: lsp_types::Position::new(2, 3),
        };

        assert_eq!(
            range_text(&lines, &range, PositionEncodingKind::UTF8),
            Some("pha\nbeta\ngam".to_string())
        );
    }

    #[test]
    fn workspace_edit_applies_text_changes() {
        let temp_dir = temp_dir("edit");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("sample.rs");
        std::fs::write(&path, "hello world").expect("write");
        let uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(HashMap::from([(
                uri,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position::new(0, 6),
                        end: lsp_types::Position::new(0, 11),
                    },
                    new_text: "urvim".to_string(),
                }],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");
        assert!(effects.is_empty());

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let text = crate::globals::with_buffer(buffer_id, |buffer| buffer.as_str())
            .expect("buffer should exist");

        assert_eq!(text, "hello urvim");
    }

    #[test]
    fn workspace_edit_applies_multiple_edits_in_one_file() {
        let temp_dir = temp_dir("edit-many");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("sample.rs");
        std::fs::write(&path, "abcdef").expect("write");
        let uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(HashMap::from([(
                uri,
                vec![
                    lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(0, 1),
                            end: lsp_types::Position::new(0, 2),
                        },
                        new_text: "X".to_string(),
                    },
                    lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(0, 4),
                            end: lsp_types::Position::new(0, 5),
                        },
                        new_text: "Y".to_string(),
                    },
                ],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");
        assert!(effects.is_empty());

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let text = crate::globals::with_buffer(buffer_id, |buffer| buffer.as_str())
            .expect("buffer should exist");

        assert_eq!(text, "aXcdYf");
    }

    #[test]
    fn workspace_edit_pushes_undo_checkpoint_before_apply() {
        let temp_dir = temp_dir("edit-undo");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("sample.rs");
        std::fs::write(&path, "hello world").expect("write");
        let uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        crate::globals::with_buffer_mut(buffer_id, |buffer| {
            buffer.push_snapshot(crate::buffer::Cursor::new(0, 6));
        })
        .expect("buffer should exist");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(HashMap::from([(
                uri,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position::new(0, 6),
                        end: lsp_types::Position::new(0, 11),
                    },
                    new_text: "urvim".to_string(),
                }],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");
        assert!(effects.is_empty());

        let cursor = crate::globals::with_buffer_mut(buffer_id, |buffer| buffer.undo())
            .expect("buffer should exist")
            .expect("undo should be available");
        let text = crate::globals::with_buffer(buffer_id, |buffer| buffer.as_str())
            .expect("buffer should exist");

        assert_eq!(cursor, crate::buffer::Cursor::new(0, 6));
        assert_eq!(text, "hello world");
    }

    #[test]
    fn workspace_edit_applies_utf16_multibyte_ranges() {
        let temp_dir = temp_dir("edit-utf16");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("sample.rs");
        std::fs::write(&path, "a𝄞b").expect("write");
        let uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(HashMap::from([(
                uri,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position::new(0, 1),
                        end: lsp_types::Position::new(0, 3),
                    },
                    new_text: "X".to_string(),
                }],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");
        assert!(effects.is_empty());

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let text = crate::globals::with_buffer(buffer_id, |buffer| buffer.as_str())
            .expect("buffer should exist");

        assert_eq!(text, "aXb");
    }

    #[test]
    fn workspace_edit_applies_resource_create_and_notifies_ui() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp_dir = temp_dir("create");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("created.rs");
        let uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Create(
                    lsp_types::CreateFile {
                        uri,
                        options: None,
                        annotation_id: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");

        assert_eq!(effects.len(), 1);
        assert!(path.exists());
        assert!(matches!(
            effects[0],
            WorkspaceResourceOperationEffect::Create { .. }
        ));
        assert!(crate::globals::take_workspace_file_operation_notification().is_some());
    }

    #[test]
    fn workspace_edit_renames_loaded_buffer_and_updates_pool() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp_dir = temp_dir("rename");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let old_path = temp_dir.join("old.rs");
        let new_path = temp_dir.join("new.rs");
        std::fs::write(&old_path, "hello world").expect("write");

        let buffer_id = crate::globals::open_buffer(&old_path).expect("buffer should open");
        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Rename(
                    lsp_types::RenameFile {
                        old_uri: url::Url::from_file_path(&old_path)
                            .expect("old uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        new_uri: url::Url::from_file_path(&new_path)
                            .expect("new uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        options: None,
                        annotation_id: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");

        assert_eq!(effects.len(), 1);
        assert!(new_path.exists());
        assert!(!old_path.exists());
        assert_eq!(
            crate::globals::with_buffer_pool(|pool| {
                pool.buffer_id_for_path(&crate::AbsolutePath::from_path(&new_path).expect("abs"))
            }),
            Some(buffer_id)
        );
        assert!(
            crate::globals::with_buffer_pool(|pool| {
                pool.buffer_id_for_path(&crate::AbsolutePath::from_path(&old_path).expect("abs"))
            })
            .is_none()
        );
    }

    #[test]
    fn workspace_edit_deletes_loaded_buffer_and_removes_it_from_pool() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp_dir = temp_dir("delete");
        std::fs::create_dir_all(&temp_dir).expect("root");
        let path = temp_dir.join("delete.rs");
        std::fs::write(&path, "hello world").expect("write");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Delete(
                    lsp_types::DeleteFile {
                        uri: url::Url::from_file_path(&path)
                            .expect("uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        options: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = apply_workspace_edit(&edit, PositionEncodingKind::UTF16).expect("apply edit");

        assert_eq!(effects.len(), 1);
        assert!(!path.exists());
        assert!(crate::globals::with_buffer(buffer_id, |_| ()).is_none());
        assert!(matches!(
            effects[0],
            WorkspaceResourceOperationEffect::Delete {
                buffer_id: Some(_),
                ..
            }
        ));
        assert!(matches!(
            crate::globals::take_workspace_file_operation_notification(),
            Some(crate::globals::WorkspaceFileOperationNotification::Delete { .. })
        ));
    }
}
