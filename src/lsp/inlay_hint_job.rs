//! Background job for chunked LSP inlay hints.

use crate::background::{
    JobContext, JobError, JobEvent, JobPayload, LspInlayHint, LspInlayHintsChunk,
};
use crate::buffer::{BufferId, Cursor};
use crate::config::InlayHintCapability;
use crate::globals;
use crate::json_rpc::{ErrorResponse, Message, Response, SuccessResponse};
use crate::lsp::position::position_character_to_byte_index;
use imbl::Vector;
use lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintLabelPart, PositionEncodingKind,
};
use smol_str::SmolStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_LSP_INLAY_HINT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Snapshot needed to request inlay hints for a buffer.
#[derive(Debug, Clone)]
pub struct LspInlayHintSnapshot {
    /// Buffer being hinted.
    pub buffer_id: BufferId,
    /// Document URI.
    pub uri: String,
    /// Buffer lines at the time the job started.
    pub lines: Vector<std::sync::Arc<str>>,
    /// Position encoding negotiated for the server.
    pub position_encoding: PositionEncodingKind,
}

/// Background inlay-hint request payload.
#[derive(Debug, Clone)]
pub struct LspInlayHintJob {
    snapshot: LspInlayHintSnapshot,
    start_line: usize,
    syntax_generation: u64,
}

impl LspInlayHintJob {
    const VIEWPORT_LINES: usize = 120;

    /// Creates a new chunked inlay-hint job.
    pub fn new(snapshot: LspInlayHintSnapshot, start_line: usize, syntax_generation: u64) -> Self {
        Self {
            snapshot,
            start_line,
            syntax_generation,
        }
    }

    /// Returns the next inlay-hint generation token.
    pub fn next_generation() -> u64 {
        NEXT_LSP_INLAY_HINT_GENERATION.fetch_add(1, Ordering::SeqCst)
    }

    /// Runs the inlay-hint request against the active LSP runtime.
    ///
    /// Only holds the global runtime mutex for the brief send+register step, then
    /// releases it before waiting for the LSP response so the UI thread is never
    /// blocked during a slow inlay-hint round trip.
    pub fn run(self, context: &JobContext, event_tx: &Sender<JobEvent>) {
        let _ = event_tx.send(JobEvent::Started {
            kind: context.kind().clone(),
            token: context.token(),
        });

        let line = self.start_line.min(self.snapshot.lines.len());
        if context.is_stopping() || !context.is_current() || context.is_aborted() {
            return;
        }

        let end_line = (line + Self::VIEWPORT_LINES).min(self.snapshot.lines.len());

        // Brief lock to send the request and register a response channel.
        let rx = match globals::with_lsp_runtime_mut(|runtime| {
            runtime.send_inlay_hint_request_get_receiver(
                self.snapshot.buffer_id,
                &self.snapshot,
                line,
                end_line,
            )
        }) {
            Some(Ok(rx)) => rx,
            Some(Err(error)) => {
                let _ = event_tx.send(JobEvent::Failed {
                    kind: context.kind().clone(),
                    token: context.token(),
                    error: JobError::Message(error),
                });
                return;
            }
            None => {
                let _ = event_tx.send(JobEvent::Failed {
                    kind: context.kind().clone(),
                    token: context.token(),
                    error: JobError::Message("LSP runtime is not available".to_string()),
                });
                return;
            }
        };
        // Global mutex is released — the receiver lives independently.

        let encoding = self.snapshot.position_encoding.clone();
        let hints = match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Message::Response(Response::Success(SuccessResponse { result, .. }))) => {
                serde_json::from_value::<Option<Vec<InlayHint>>>(result)
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            }
            Ok(Message::Response(Response::Error(ErrorResponse { error, .. }))) => {
                let _ = event_tx.send(JobEvent::Failed {
                    kind: context.kind().clone(),
                    token: context.token(),
                    error: JobError::Message(error.message),
                });
                return;
            }
            Ok(_) => Vec::new(),
            Err(_) => {
                let _ = event_tx.send(JobEvent::Failed {
                    kind: context.kind().clone(),
                    token: context.token(),
                    error: JobError::Message(
                        "timed out waiting for inlay hint response".to_string(),
                    ),
                });
                return;
            }
        };

        // Filter hints based on config-enabled kinds.
        let hints: Vec<InlayHint> = hints
            .into_iter()
            .filter(|hint| inlay_hint_kind_enabled(hint.kind.as_ref()))
            .collect();

        let payload = JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id: self.snapshot.buffer_id,
            syntax_generation: self.syntax_generation,
            start_line: line,
            end_line,
            hints: hints
                .into_iter()
                .filter_map(|hint| {
                    inlay_hint_to_payload(&hint, &self.snapshot.lines, encoding.clone())
                })
                .collect(),
        });

        if event_tx
            .send(JobEvent::Chunk {
                kind: context.kind().clone(),
                token: context.token(),
                payload,
            })
            .is_err()
        {
            return;
        }

        let _ = event_tx.send(JobEvent::Completed {
            kind: context.kind().clone(),
            token: context.token(),
            payload: None,
        });
    }
}

fn inlay_hint_to_payload(
    hint: &InlayHint,
    lines: &Vector<std::sync::Arc<str>>,
    encoding: PositionEncodingKind,
) -> Option<LspInlayHint> {
    let position = position_to_cursor(lines, hint.position, encoding)?;
    let label = inlay_hint_label_to_text(&hint.label)?;
    Some(LspInlayHint { position, label })
}

fn inlay_hint_label_to_text(label: &InlayHintLabel) -> Option<SmolStr> {
    let text = match label {
        InlayHintLabel::String(text) => text.clone(),
        InlayHintLabel::LabelParts(parts) => parts
            .iter()
            .map(|part: &InlayHintLabelPart| part.value.as_str())
            .collect::<String>(),
    };

    (!text.is_empty()).then_some(SmolStr::new(text))
}

fn position_to_cursor(
    lines: &Vector<std::sync::Arc<str>>,
    position: lsp_types::Position,
    encoding: PositionEncodingKind,
) -> Option<Cursor> {
    let line = lines.get(position.line as usize)?;
    let col = position_character_to_byte_index(line.as_ref(), position.character, encoding)?;
    Some(Cursor::new(position.line as usize, col))
}

/// Returns whether the given inlay-hint kind is enabled in the current config.
fn inlay_hint_kind_enabled(kind: Option<&InlayHintKind>) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{InlayHintLabel, Position};
    use std::sync::Arc;

    fn line_vector(text: &str) -> Vector<Arc<str>> {
        Vector::from_iter([Arc::from(text)])
    }

    #[test]
    fn inlay_hint_to_payload_preserves_label_text() {
        let lines = line_vector("let value = foo();");
        let hint = InlayHint {
            position: Position::new(0, 4),
            label: InlayHintLabel::String("name:".into()),
            kind: None,
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        };

        let payload =
            inlay_hint_to_payload(&hint, &lines, PositionEncodingKind::UTF8).expect("payload");

        assert_eq!(payload.label, SmolStr::new("name:"));
    }
}
