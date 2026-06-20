//! LSP request methods on `LspServerSession`.
//!
//! Each method sends a JSON-RPC request and returns the raw LSP response type.
//! Core is responsible for converting LSP types to editor-facing types
//! (`CompletionCandidate`, `DocumentSymbolItem`, `ReferenceItem`, etc.).
//!
//! Methods that previously called core globals now take the needed data as
//! parameters (e.g., `code_actions` takes diagnostics, inlay hint methods take
//! config booleans).

use std::sync::mpsc;

use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionTriggerKind,
    CompletionItem, CompletionParams, CompletionResponse, InlayHint, InlayHintKind,
    InlayHintParams, Location, PrepareRenameResponse, ReferenceContext, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkspaceEdit, WorkspaceSymbolResponse,
};
use serde_json::json;
use urvim_json_rpc::{Message, Request, RequestId};
use urvim_text::{Cursor, PieceTable};

use urvim_id::BufferId;

use super::session::{
    BufferAttachment, LspServerSession, cursor_to_lsp_position, line_range_to_lsp_range,
    position_to_lsp_json, range_text,
};

/// Snapshot needed to request inlay hints for a buffer.
///
/// This type is defined in `urvim_lsp` (not core) because it only uses types
/// available to this crate. Core's background job system imports it from here.
#[derive(Debug, Clone)]
pub struct LspInlayHintSnapshot {
    pub buffer_id: BufferId,
    pub uri: String,
    pub lines: PieceTable,
    pub position_encoding: lsp_types::PositionEncodingKind,
}

impl LspServerSession {
    pub fn hover(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
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

    pub fn completion(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
    ) -> Result<Option<CompletionResponse>, String> {
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
        Ok(response)
    }

    pub fn resolve_completion(
        &mut self,
        item: &serde_json::Value,
    ) -> Result<Option<CompletionItem>, String> {
        let result = self
            .request_raw("completionItem/resolve", Some(item.clone()))
            .map_err(|error| error.to_string())?;

        let Some(value) = result else {
            return Ok(None);
        };

        let item =
            serde_json::from_value::<CompletionItem>(value).map_err(|error| error.to_string())?;
        Ok(Some(item))
    }

    /// Returns the first definition target as `(uri_string, position)`.
    ///
    /// Core is responsible for opening the file and converting the LSP position
    /// to a buffer cursor.
    pub fn definition(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
    ) -> Result<Option<(String, lsp_types::Position)>, String> {
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

        Ok(first_definition_target(response))
    }

    pub fn references(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
    ) -> Result<Option<Vec<Location>>, String> {
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
        Ok(response)
    }

    pub fn document_symbols(
        &mut self,
        attachment: &BufferAttachment,
    ) -> Result<Option<lsp_types::DocumentSymbolResponse>, String> {
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
        Ok(response)
    }

    pub fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> Result<Option<WorkspaceSymbolResponse>, String> {
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
        Ok(response)
    }

    /// Returns the raw `WorkspaceEdit` from a rename request.
    ///
    /// Core converts the edit to `LspRuntimeEffect` values and applies them.
    pub fn rename(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
        new_name: &str,
    ) -> Result<Option<WorkspaceEdit>, String> {
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

        let edit = serde_json::from_value::<Option<WorkspaceEdit>>(value)
            .map_err(|error| error.to_string())?;
        Ok(edit)
    }

    pub fn rename_placeholder(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
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

    /// Requests code actions, taking diagnostics as a parameter.
    ///
    /// Core reads diagnostics from the diagnostics store and passes them in.
    pub fn code_actions(
        &mut self,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        cursor: Cursor,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Result<Option<Vec<CodeActionOrCommand>>, String> {
        if !self.supports_code_actions() {
            return Err("attached server does not support code actions".to_string());
        }

        let position =
            cursor_to_lsp_position(lines, cursor, self.negotiated.position_encoding.clone());
        let range_start = position.clone();
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
        Ok(actions)
    }

    pub fn execute_command(
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

    /// Requests inlay hints for a range, filtering by configured kinds.
    ///
    /// `inlay_hints_enabled`, `inlay_hint_type_enabled`, and
    /// `inlay_hint_parameter_enabled` are read from config by the caller (core).
    pub fn request_inlay_hints_for_range(
        &mut self,
        uri: &str,
        lines: &PieceTable,
        start_line: usize,
        end_line: usize,
        encoding: lsp_types::PositionEncodingKind,
        inlay_hints_enabled: bool,
        inlay_hint_type_enabled: bool,
        inlay_hint_parameter_enabled: bool,
    ) -> Result<Option<Vec<InlayHint>>, String> {
        if !inlay_hints_enabled {
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
                .filter(|hint| {
                    inlay_hint_kind_enabled(
                        hint.kind.as_ref(),
                        inlay_hint_type_enabled,
                        inlay_hint_parameter_enabled,
                    )
                })
                .collect()
        }))
    }

    /// Returns a snapshot for chunked inlay-hint requests.
    ///
    /// `inlay_hints_enabled` is read from config by the caller (core).
    pub fn inlay_hint_snapshot(
        &self,
        buffer_id: BufferId,
        attachment: &BufferAttachment,
        lines: &PieceTable,
        inlay_hints_enabled: bool,
    ) -> Result<Option<LspInlayHintSnapshot>, String> {
        if !inlay_hints_enabled {
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

    /// Sends a viewport inlay-hint request and returns a response receiver.
    ///
    /// The caller briefly holds the runtime mutex to send the request and
    /// register a response channel, then releases it. The receiver can be
    /// waited on independently so the background worker does not block the UI.
    pub fn send_inlay_hint_request(
        &self,
        snapshot: &LspInlayHintSnapshot,
        start_line: usize,
        end_line: usize,
    ) -> Result<mpsc::Receiver<Message>, String> {
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

        let id = RequestId::Number(
            self.next_request_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );
        let value = serde_json::to_value(params).map_err(|e| e.to_string())?;
        let request = Message::Request(Request::new(
            id.clone(),
            "textDocument/inlayHint",
            Some(value),
        ));
        let (tx, rx) = mpsc::channel();
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(id.clone(), tx);
        }
        self.write_message(&request).map_err(|e| e.to_string())?;
        Ok(rx)
    }
}

fn inlay_hint_kind_enabled(
    kind: Option<&InlayHintKind>,
    type_enabled: bool,
    parameter_enabled: bool,
) -> bool {
    let Some(kind) = kind else {
        return true;
    };

    match kind {
        k if k == &InlayHintKind::TYPE => type_enabled,
        k if k == &InlayHintKind::PARAMETER => parameter_enabled,
        _ => false,
    }
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

/// Extracts the display text from an LSP inlay hint label.
///
/// An `InlayHintLabel` is either a plain string or a sequence of label parts.
/// Returns `None` when the resulting text is empty.
pub fn inlay_hint_label_to_text(label: &lsp_types::InlayHintLabel) -> Option<String> {
    let text = match label {
        lsp_types::InlayHintLabel::String(text) => text.clone(),
        lsp_types::InlayHintLabel::LabelParts(parts) => parts
            .iter()
            .map(|part| part.value.as_str())
            .collect::<String>(),
    };

    (!text.is_empty()).then_some(text)
}
