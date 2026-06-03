//! Background completion job execution.

use super::CompletionSourceKind;
use super::sources::buffer_words::buffer_words_completion_candidates;
use super::sources::current_word_prefix;
use super::sources::paths::path_completion_candidates;
use crate::background::{JobContext, JobEvent, JobPayload};
use crate::buffer::{BufferId, Cursor};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// A background completion request.
#[derive(Debug)]
pub struct CompletionJob {
    buffer_id: BufferId,
    cursor: Cursor,
    source: CompletionSourceKind,
}

impl CompletionJob {
    /// Creates a new completion job for a buffer cursor.
    pub fn new(buffer_id: BufferId, cursor: Cursor, source: CompletionSourceKind) -> Self {
        Self {
            buffer_id,
            cursor,
            source,
        }
    }

    /// Runs the completion job on a worker thread.
    pub fn run(self, context: &JobContext, event_tx: &Sender<JobEvent>) {
        let Self {
            buffer_id,
            cursor,
            source,
        } = self;
        let results = match source {
            CompletionSourceKind::Lsp => crate::globals::with_lsp_runtime_mut(|runtime| {
                runtime.completion_buffer(buffer_id, cursor)
            })
            .and_then(Result::ok)
            .flatten()
            .unwrap_or_default(),
            CompletionSourceKind::Paths | CompletionSourceKind::BufferWords => {
                crate::globals::with_buffer(buffer_id, |buffer| {
                    let cwd = std::env::current_dir().ok();
                    let home = home_directory_path();
                    match source {
                        CompletionSourceKind::Paths => path_completion_candidates(
                            buffer,
                            cursor,
                            cwd.as_deref(),
                            home.as_deref(),
                        ),
                        CompletionSourceKind::BufferWords => {
                            let query = current_word_prefix(buffer, cursor).1;
                            buffer_words_completion_candidates(buffer, cursor, query.as_str())
                        }
                        CompletionSourceKind::Lsp => Vec::new(),
                    }
                })
                .unwrap_or_default()
            }
        };

        if context.is_stopping() || !context.is_current() || context.is_aborted() {
            return;
        }

        event_tx
            .send(JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(JobPayload::CompletionResults {
                    source,
                    items: results,
                }),
            })
            .ok();
    }
}

fn home_directory_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
