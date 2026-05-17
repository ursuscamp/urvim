mod code_actions;
mod colorscheme;
mod doc_symbols;
mod file;
mod grep;
mod references;

use super::Layout;
use crate::background::JobEvent;
use crate::background::JobPayload;
use crate::ui::picker::PickerSearchEvent;

impl Layout {
    /// Dispatches a picker-related job event.
    pub fn dispatch_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Started { .. } => {}
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::PreviewSyntax(result),
            } if kind == crate::background::JobKind::PickerPreviewSyntax => {
                self.dispatch_preview_syntax_chunk(token.generation(), result);
            }
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::FileSearchSnapshot(results),
            } if kind == crate::background::JobKind::FilePickerSearch => {
                if let Some(picker) = self.dialogs.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerResults {
                        generation: token.generation(),
                        results,
                    });
                }
            }
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::DocSymbolsSearch(chunk),
            } if matches!(
                kind,
                crate::background::JobKind::DocSymbolsPickerSearch
                    | crate::background::JobKind::WorkspaceSymbolsPickerSearch
            ) =>
            {
                let picker = match kind {
                    crate::background::JobKind::DocSymbolsPickerSearch => {
                        self.dialogs.doc_symbols_picker.as_mut()
                    }
                    crate::background::JobKind::WorkspaceSymbolsPickerSearch => {
                        self.dialogs.workspace_symbols_picker.as_mut()
                    }
                    _ => None,
                };

                if let Some(picker) = picker {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
                    });
                }
            }
            JobEvent::Completed {
                kind,
                token,
                payload: Some(JobPayload::PreviewSyntax(result)),
            } if kind == crate::background::JobKind::PickerPreviewSyntax => {
                self.dispatch_preview_syntax_complete(token.generation(), result);
            }
            JobEvent::Completed {
                kind,
                token,
                payload: Some(JobPayload::DocSymbolsSearch(chunk)),
            } if matches!(
                kind,
                crate::background::JobKind::DocSymbolsPickerSearch
                    | crate::background::JobKind::WorkspaceSymbolsPickerSearch
            ) =>
            {
                let picker = match kind {
                    crate::background::JobKind::DocSymbolsPickerSearch => {
                        self.dialogs.doc_symbols_picker.as_mut()
                    }
                    crate::background::JobKind::WorkspaceSymbolsPickerSearch => {
                        self.dialogs.workspace_symbols_picker.as_mut()
                    }
                    _ => None,
                };

                if let Some(picker) = picker {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
                    });
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if matches!(
                    kind,
                    crate::background::JobKind::DocSymbolsPickerSearch
                        | crate::background::JobKind::WorkspaceSymbolsPickerSearch
                ) =>
            {
                let picker = match kind {
                    crate::background::JobKind::DocSymbolsPickerSearch => {
                        self.dialogs.doc_symbols_picker.as_mut()
                    }
                    crate::background::JobKind::WorkspaceSymbolsPickerSearch => {
                        self.dialogs.workspace_symbols_picker.as_mut()
                    }
                    _ => None,
                };

                if let Some(picker) = picker {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind == crate::background::JobKind::PickerPreviewSyntax =>
            {
                self.dispatch_preview_syntax_failed(token.generation());
            }
            JobEvent::Completed { kind, token, .. }
                if kind == crate::background::JobKind::FilePickerSearch =>
            {
                if let Some(picker) = self.dialogs.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind == crate::background::JobKind::FilePickerSearch =>
            {
                if let Some(picker) = self.dialogs.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            other => self.dispatch_grep_picker_job_event(other),
        }
    }

    fn dispatch_preview_syntax_chunk(
        &mut self,
        generation: u64,
        result: crate::ui::picker::preview::PreviewSyntaxRefreshResult,
    ) {
        if let Some(picker) = self.dialogs.file_picker.as_mut() {
            picker.handle_preview_syntax_refresh_chunk(generation, result);
        } else if let Some(picker) = self.dialogs.grep_picker.as_mut() {
            picker.handle_preview_syntax_refresh_chunk(generation, result);
        } else if let Some(picker) = self.dialogs.doc_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh_chunk(generation, result);
        } else if let Some(picker) = self.dialogs.workspace_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh_chunk(generation, result);
        } else if let Some(picker) = self.dialogs.references_picker.as_mut() {
            picker.handle_preview_syntax_refresh_chunk(generation, result);
        }
    }

    fn dispatch_preview_syntax_complete(
        &mut self,
        generation: u64,
        result: crate::ui::picker::preview::PreviewSyntaxRefreshResult,
    ) {
        if let Some(picker) = self.dialogs.file_picker.as_mut() {
            picker.handle_preview_syntax_refresh(generation, result);
        } else if let Some(picker) = self.dialogs.grep_picker.as_mut() {
            picker.handle_preview_syntax_refresh(generation, result);
        } else if let Some(picker) = self.dialogs.doc_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh(generation, result);
        } else if let Some(picker) = self.dialogs.workspace_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh(generation, result);
        } else if let Some(picker) = self.dialogs.references_picker.as_mut() {
            picker.handle_preview_syntax_refresh(generation, result);
        }
    }

    fn dispatch_preview_syntax_failed(&mut self, generation: u64) {
        if let Some(picker) = self.dialogs.file_picker.as_mut() {
            picker.handle_preview_syntax_refresh_failed(generation);
        } else if let Some(picker) = self.dialogs.grep_picker.as_mut() {
            picker.handle_preview_syntax_refresh_failed(generation);
        } else if let Some(picker) = self.dialogs.doc_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh_failed(generation);
        } else if let Some(picker) = self.dialogs.workspace_symbols_picker.as_mut() {
            picker.handle_preview_syntax_refresh_failed(generation);
        } else if let Some(picker) = self.dialogs.references_picker.as_mut() {
            picker.handle_preview_syntax_refresh_failed(generation);
        }
    }
}
