//! LSP server session lifecycle, JSON-RPC plumbing, and shared helpers.
//!
//! `LspServerSession` owns the server process, stdin/stdout pipes, and the
//! reader thread. The reader thread emits `LspRuntimeEffect` values through a
//! channel instead of touching editor globals directly — core drains the channel
//! and applies the effects.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use lsp_types::{
    Diagnostic, InitializeResult, PositionEncodingKind, ProgressParams, ProgressParamsValue,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, WorkDoneProgress,
    WorkDoneProgressBegin, WorkDoneProgressReport,
};
use serde::Deserialize;
use serde_json::{Value, json};
use urvim_json_rpc::{
    ErrorResponse, Message, Notification, Request, RequestId, Response, SuccessResponse,
    decode_message, encode_message,
};
use urvim_text::{Cursor, PieceTable, TextRef, TextSnapshot};

use crate::config::LspServerConfig;
use urvim_id::BufferId;

use crate::document::LspRuntimeEffect;
use crate::position::{
    lsp_position_from_text, lsp_range_from_text, text_encoding_from_lsp, text_position_from_lsp,
    text_range_from_lsp,
};

/// Per-buffer attachment state tracked by the session.
#[derive(Debug, Clone)]
pub struct BufferAttachment {
    pub uri: String,
    pub version: i32,
    pub generation: u64,
    pub language_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspSessionState {
    Starting,
    Running,
    ShuttingDown,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NegotiatedCapabilities {
    pub server_capabilities: Option<ServerCapabilities>,
    pub position_encoding: PositionEncodingKind,
    pub text_document_sync: Option<TextDocumentSyncKind>,
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

/// One running LSP server process and its protocol state.
#[derive(Debug)]
pub struct LspServerSession {
    pub state: LspSessionState,
    pub child: Child,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub pending: Arc<Mutex<HashMap<RequestId, mpsc::Sender<Message>>>>,
    pub next_request_id: AtomicU64,
    pub attachments: Arc<Mutex<HashMap<BufferId, BufferAttachment>>>,
    pub uri_to_buffer: Arc<Mutex<HashMap<String, BufferId>>>,
    pub progress: Arc<Mutex<ServerProgressState>>,
    pub root: PathBuf,
    pub server_name: String,
    pub negotiated: NegotiatedCapabilities,
    pub position_encoding: Arc<Mutex<PositionEncodingKind>>,
    pub initialization_options: Value,
    pub effect_sender: mpsc::Sender<LspRuntimeEffect>,
}

/// Per-server-config runtime: manages sessions for one LSP server config.
#[derive(Debug)]
pub struct ServerRuntime {
    pub config: LspServerConfig,
    pub sessions: BTreeMap<PathBuf, LspServerSession>,
    pub failed_sessions: BTreeMap<PathBuf, String>,
    pub progress: Arc<Mutex<ServerProgressState>>,
}

#[derive(Debug, Default)]
pub struct ServerProgressState {
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
    pub server_name: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct PublishDiagnosticsNotification {
    uri: String,
    diagnostics: Vec<Diagnostic>,
}

impl ServerRuntime {
    pub fn matches_filetype(&self, syntax_name: &str) -> bool {
        self.config
            .filetypes
            .iter()
            .any(|filetype| filetype == syntax_name)
    }

    pub fn session_for_buffer_mut(&mut self, buffer_id: BufferId) -> Option<&mut LspServerSession> {
        self.sessions
            .values_mut()
            .find(|session| session.contains_buffer(buffer_id))
    }

    pub fn cleanup_detached_buffers(&mut self, live_targets: &BTreeSet<(BufferId, PathBuf)>) {
        let live_buffers = live_targets
            .iter()
            .map(|(buffer_id, _)| *buffer_id)
            .collect::<BTreeSet<_>>();

        for session in self.sessions.values_mut() {
            session.cleanup_detached_buffers(&live_buffers);
        }
    }
}

impl LspServerSession {
    pub fn spawn(
        server_name: &str,
        config: &LspServerConfig,
        root: &Path,
        progress: Arc<Mutex<ServerProgressState>>,
        effect_sender: mpsc::Sender<LspRuntimeEffect>,
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
            effect_sender,
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
        let server_name = self.server_name.clone();
        let effect_sender = self.effect_sender.clone();

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
                            stdin.write_all(&bytes).ok();
                            stdin.flush().ok();
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
                            sender
                                .send(Message::Response(Response::Success(SuccessResponse {
                                    id,
                                    result,
                                    jsonrpc: "2.0".to_string(),
                                })))
                                .ok();
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
                            effect_sender
                                .send(LspRuntimeEffect::Diagnostics {
                                    buffer_id,
                                    server_name: server_name.clone(),
                                    diagnostics: params.diagnostics,
                                })
                                .ok();
                            effect_sender
                                .send(LspRuntimeEffect::RequestInlayHintRetry)
                                .ok();
                            effect_sender.send(LspRuntimeEffect::RequestRedraw).ok();
                        }
                    }
                    Message::Notification(Notification { method, params, .. })
                        if method == "$/progress" =>
                    {
                        if let Some(params) = params
                            && let Ok(params) = serde_json::from_value::<ProgressParams>(params)
                        {
                            if handle_progress_notification(&progress, params) {
                                effect_sender
                                    .send(LspRuntimeEffect::RequestInlayHintRetry)
                                    .ok();
                            }
                            effect_sender.send(LspRuntimeEffect::RequestRedraw).ok();
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    pub fn buffer_attachment(&self, buffer_id: BufferId) -> Option<BufferAttachment> {
        self.attachments
            .lock()
            .ok()
            .and_then(|attachments| attachments.get(&buffer_id).cloned())
    }

    pub fn contains_buffer(&self, buffer_id: BufferId) -> bool {
        self.attachments
            .lock()
            .ok()
            .is_some_and(|attachments| attachments.contains_key(&buffer_id))
    }

    pub fn attached_buffer_ids(&self) -> Vec<BufferId> {
        self.attachments
            .lock()
            .ok()
            .map(|attachments| attachments.keys().copied().collect())
            .unwrap_or_default()
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

    /// Syncs a document with the server, sending didOpen or didChange as needed.
    ///
    /// `text` is the current full document text, passed in by core (which reads
    /// it from the buffer pool). The session does NOT access any editor state
    /// directly.
    pub fn sync_document(
        &mut self,
        buffer_id: BufferId,
        path: &Path,
        generation: u64,
        syntax_name: &str,
        text: &str,
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

    pub fn request_raw(&self, method: &str, params: Option<Value>) -> io::Result<Option<Value>> {
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

    pub fn write_message(&self, message: &Message) -> io::Result<()> {
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

    pub fn shutdown(&mut self) -> io::Result<()> {
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
            self.close_buffer(buffer_id);
        }
    }

    /// Detaches a buffer: removes the attachment, sends didClose, and emits a
    /// `ClearDiagnostics` effect for core to apply.
    pub fn close_buffer(&mut self, buffer_id: BufferId) {
        if let Some(attachment) = self.remove_buffer(buffer_id) {
            let params = json!({"textDocument": {"uri": attachment.uri}});
            self.notify("textDocument/didClose", Some(params)).ok();
            self.effect_sender
                .send(LspRuntimeEffect::ClearDiagnostics {
                    buffer_id,
                    server_name: self.server_name.clone(),
                })
                .ok();
        }
    }

    /// Updates the session attachment when a file is renamed on disk.
    ///
    /// `text` is the current buffer text, passed in by core. The session sends
    /// didClose for the old URI and didOpen for the new URI.
    pub fn rename_buffer_attachment(&mut self, old_path: &Path, new_path: &Path, text: &str) {
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
}

impl ServerProgressState {
    pub fn set_begin(&mut self, token: String, begin: WorkDoneProgressBegin) {
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

    pub fn set_report(&mut self, token: String, report: WorkDoneProgressReport) {
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

    pub fn clear_token(&mut self, token: &str) {
        self.entries.remove(token);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn current_message(&self) -> Option<String> {
        self.entries
            .values()
            .max_by_key(|entry| entry.sequence)
            .map(|entry| {
                format_progress_message(&entry.title, entry.message.as_deref(), entry.percentage)
            })
    }

    pub fn has_active_progress(&self) -> bool {
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

pub fn resolve_workspace_root(path: &Path, root_markers: &[String]) -> Option<PathBuf> {
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

pub fn file_uri_string(path: &Path) -> io::Result<String> {
    let url = url::Url::from_file_path(path)
        .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "invalid file path"))?;
    Ok(url.to_string())
}

pub fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    let url = url::Url::parse(uri).map_err(|error| error.to_string())?;
    url.to_file_path()
        .map_err(|()| "LSP URI is not a file path".to_string())
}

pub fn buffer_text_from_lines(lines: &PieceTable) -> String {
    lines.text().to_text()
}

pub fn position_to_lsp_json(
    lines: &PieceTable,
    cursor: Cursor,
    encoding: PositionEncodingKind,
) -> Value {
    let position = cursor_to_lsp_position(lines, cursor, encoding);
    json!({"line": position.line, "character": position.character})
}

pub fn line_range_to_lsp_range(
    lines: &PieceTable,
    start_line: usize,
    end_line: usize,
    encoding: PositionEncodingKind,
) -> Option<lsp_types::Range> {
    lines
        .line_range_for_lines(start_line, end_line, text_encoding_from_lsp(encoding))
        .map(lsp_range_from_text)
}

pub fn cursor_to_lsp_position(
    lines: &PieceTable,
    cursor: Cursor,
    encoding: PositionEncodingKind,
) -> lsp_types::Position {
    lines
        .position_for_cursor(cursor, text_encoding_from_lsp(encoding))
        .map(lsp_position_from_text)
        .unwrap_or_else(|| lsp_types::Position::new(cursor.line as u32, 0))
}

pub fn position_to_cursor(
    lines: &PieceTable,
    position: lsp_types::Position,
    encoding: PositionEncodingKind,
) -> Option<Cursor> {
    lines.cursor_for_position(
        text_position_from_lsp(position),
        text_encoding_from_lsp(encoding),
    )
}

pub fn range_text(
    lines: &PieceTable,
    range: &lsp_types::Range,
    encoding: PositionEncodingKind,
) -> Option<String> {
    let range = lines.cursors_for_range(
        text_range_from_lsp(*range),
        text_encoding_from_lsp(encoding),
    )?;
    lines
        .range(range.start, range.end)
        .map(|text| text.to_text())
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
