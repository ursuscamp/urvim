use super::Layout;
use crate::background::{JobEvent, JobKind, JobPayload, JobToken};
use crate::buffer::{Buffer, Cursor, Gravity};
use crate::lsp::inlay_hint_job::LspInlayHintJob;
use smol_str::SmolStr;

impl Layout {
    /// Marks the active viewport for an inlay-hint request on the next render pass.
    pub fn request_inlay_hints_for_active_viewport(&mut self) {
        self.inlay_hints = super::InlayHintState::Pending;
    }

    /// Returns whether an inlay-hint request is currently pending.
    pub(super) fn inlay_hint_request_pending(&self) -> bool {
        self.inlay_hints.is_pending()
    }

    pub(super) fn request_active_buffer_inlay_hints(&mut self) {
        let super::InlayHintState::Pending = &self.inlay_hints else {
            return;
        };

        let buffer_view = self.active_buffer_view();
        let Some(buffer_id) = buffer_view.buffer_id_opt() else {
            return;
        };

        let start_line = buffer_view.scroll_offset().row as usize;
        let Some(syntax_generation) = buffer_view.with_buffer(|buffer| buffer.syntax_generation())
        else {
            return;
        };

        let params = super::InlayHintRequestParams {
            buffer_id,
            start_line,
            syntax_generation,
        };

        if crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_has_active_progress(buffer_id)
        })
        .unwrap_or(false)
        {
            return;
        }

        // Dedup: same request already in-flight.
        if matches!(
            &self.inlay_hints,
            super::InlayHintState::InFlight(super::InFlightInlayHintRequest { params: p, .. })
                if *p == params
        ) {
            self.inlay_hints = super::InlayHintState::Idle;
            return;
        }

        let Some(snapshot) = crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.inlay_hint_snapshot(buffer_id)
        })
        .and_then(|result| result.ok().flatten()) else {
            return;
        };

        let generation = LspInlayHintJob::next_generation();
        let job = LspInlayHintJob::new(snapshot, start_line, syntax_generation);
        if self
            .jobs
            .submit_latest_only(
                JobKind::LspInlayHints(buffer_id),
                JobToken::new(generation),
                job,
            )
            .is_ok()
        {
            self.inlay_hints = super::InlayHintState::InFlight(super::InFlightInlayHintRequest {
                params,
                received_hints: false,
            });
        }
    }

    fn apply_lsp_inlay_hint_chunk(&mut self, chunk: crate::background::LspInlayHintsChunk) {
        let hint_count = chunk.hints.len();
        if hint_count == 0 {
            return;
        }

        // Mark that this flight has received hints.
        if let super::InlayHintState::InFlight(ref mut flight) = self.inlay_hints {
            flight.received_hints = true;
        }

        let _ = crate::globals::with_buffer_mut(chunk.buffer_id, |buffer| {
            if buffer.syntax_generation() != chunk.syntax_generation {
                return;
            }
            buffer.clear_inlay_hints_for_lines(chunk.start_line, chunk.end_line);
            for hint in chunk.hints {
                let label = Self::pad_inlay_hint_label(buffer, hint.position, hint.label);
                buffer.insert_inlay_hint(hint.position, Gravity::Right, label);
            }
            buffer.update_inlay_hints();
        });
    }

    fn pad_inlay_hint_label(buffer: &Buffer, position: Cursor, label: SmolStr) -> SmolStr {
        let at_line_end = buffer
            .line_at(position.line)
            .is_some_and(|line| position.col == line.len());

        if label.ends_with(':') {
            let mut text = label.to_string();
            text.push(' ');
            return SmolStr::new(text);
        }

        if at_line_end && !label.starts_with(':') {
            let mut text = String::with_capacity(label.len() + 1);
            text.push(' ');
            text.push_str(label.as_str());
            return SmolStr::new(text);
        }

        label
    }

    /// Routes LSP-related background job events.
    pub fn dispatch_lsp_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Started { .. } => {}
            JobEvent::Chunk {
                kind,
                payload: JobPayload::LspInlayHintsChunk(chunk),
                ..
            } if matches!(kind, JobKind::LspInlayHints(_)) => {
                self.apply_lsp_inlay_hint_chunk(chunk);
            }
            JobEvent::Completed {
                kind,
                payload: Some(JobPayload::LspRename(result)),
                ..
            } if matches!(kind, JobKind::LspRename(_)) => {
                if let Err(error) = result {
                    crate::notify_error!("LSP rename failed: {}", error);
                } else {
                    crate::globals::request_notification_redraw();
                }
            }
            JobEvent::Completed { kind, .. } if matches!(kind, JobKind::LspRename(_)) => {}
            JobEvent::Completed { kind, .. } if matches!(kind, JobKind::LspInlayHints(_)) => {
                if matches!(
                    self.inlay_hints,
                    super::InlayHintState::InFlight(super::InFlightInlayHintRequest {
                        received_hints: false,
                        ..
                    })
                ) {
                    self.inlay_hints = super::InlayHintState::Idle;
                }
            }
            JobEvent::Failed { kind, .. } if matches!(kind, JobKind::LspRename(_)) => {
                crate::notify_error!("LSP rename failed: job worker reported failure");
            }
            JobEvent::Failed { kind, error, .. } if matches!(kind, JobKind::LspInlayHints(_)) => {
                tracing::warn!(?error, "LSP inlay hints failed");
                if matches!(self.inlay_hints, super::InlayHintState::InFlight(_)) {
                    self.inlay_hints = super::InlayHintState::Idle;
                }
            }
            _ => {}
        }
    }
}
