//! Internal job framework for deferred editor work.
//!
//! The job framework runs deferred work on a single serial background thread
//! and returns job events to the main thread through a completion queue. It is
//! intentionally small so future deferred tasks can reuse the same scheduling,
//! cancellation, and shutdown behavior.

mod context;
mod error;
mod event;
mod handle;
mod manager;
mod queue;
mod shared;
mod token;
mod worker;

pub use context::JobContext;
pub use error::{JobError, JobSubmitError};
pub use event::{JobEvent, JobPayload, JobSubmissionMode};
pub use handle::JobHandle;
pub use manager::JobManager;
pub use token::{JobKind, JobToken};

use std::sync::mpsc::Sender;

use crate::buffer::BufferCacheRefreshJob;
use crate::buffer::BufferCacheRefreshResult;
use crate::lsp::rename_job::LspRenameJob;
use crate::ui::doc_symbols_picker::DocSymbolsPickerItem;
use crate::ui::doc_symbols_picker::DocSymbolsPickerSearchJob;
use crate::ui::file_picker::{FilePickerItem, PickerSearchJob};
use crate::ui::grep_picker::{GrepPickerItem, GrepPickerSearchJob};
use crate::ui::picker_preview::PreviewSyntaxRefreshJob;

/// Concrete background jobs known to the editor.
#[derive(Debug)]
pub enum BackgroundJob {
    /// Refreshes syntax and indent caches for a buffer.
    BufferCacheRefresh(BufferCacheRefreshJob),
    /// Streams file picker matches.
    FilePickerSearch(PickerSearchJob),
    /// Streams live grep matches.
    GrepPickerSearch(GrepPickerSearchJob),
    /// Streams document symbol picker matches.
    DocSymbolsPickerSearch(DocSymbolsPickerSearchJob),
    /// Refreshes preview syntax for picker panes.
    PickerPreviewSyntax(PreviewSyntaxRefreshJob),
    /// Runs an LSP rename on a background thread.
    LspRename(LspRenameJob),
    /// Test-only gate job used to block the worker thread.
    #[cfg(test)]
    Gate {
        gate: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    },
}

impl BackgroundJob {
    /// Runs this job and emits lifecycle events.
    pub fn run(self, context: &JobContext, event_tx: &Sender<JobEvent>) {
        match self {
            Self::BufferCacheRefresh(job) => job.run(context, event_tx),
            Self::FilePickerSearch(job) => job.run(context, event_tx),
            Self::GrepPickerSearch(job) => job.run(context, event_tx),
            Self::DocSymbolsPickerSearch(job) => job.run(context, event_tx),
            Self::PickerPreviewSyntax(job) => job.run(context, event_tx),
            Self::LspRename(job) => job.run(context, event_tx),
            #[cfg(test)]
            Self::Gate { gate } => {
                let (lock, cvar) = &*gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }

                let _ = event_tx.send(JobEvent::Completed {
                    kind: context.kind().clone(),
                    token: context.token(),
                    payload: None,
                });
            }
        }
    }
}

impl From<BufferCacheRefreshJob> for BackgroundJob {
    /// Wraps a buffer cache refresh job in the background job enum.
    fn from(value: BufferCacheRefreshJob) -> Self {
        Self::BufferCacheRefresh(value)
    }
}

impl From<PickerSearchJob> for BackgroundJob {
    /// Wraps a file picker search job in the background job enum.
    fn from(value: PickerSearchJob) -> Self {
        Self::FilePickerSearch(value)
    }
}

impl From<GrepPickerSearchJob> for BackgroundJob {
    /// Wraps a live grep search job in the background job enum.
    fn from(value: GrepPickerSearchJob) -> Self {
        Self::GrepPickerSearch(value)
    }
}

impl From<DocSymbolsPickerSearchJob> for BackgroundJob {
    /// Wraps a document symbol picker search job in the background job enum.
    fn from(value: DocSymbolsPickerSearchJob) -> Self {
        Self::DocSymbolsPickerSearch(value)
    }
}

impl From<PreviewSyntaxRefreshJob> for BackgroundJob {
    /// Wraps a picker preview syntax refresh job in the background job enum.
    fn from(value: PreviewSyntaxRefreshJob) -> Self {
        Self::PickerPreviewSyntax(value)
    }
}

impl From<LspRenameJob> for BackgroundJob {
    /// Wraps an LSP rename job in the background job enum.
    fn from(value: LspRenameJob) -> Self {
        Self::LspRename(value)
    }
}

impl From<BufferCacheRefreshResult> for JobPayload {
    /// Wraps a completed buffer cache refresh result.
    fn from(value: BufferCacheRefreshResult) -> Self {
        Self::BufferCacheRefresh(value)
    }
}

impl From<Vec<FilePickerItem>> for JobPayload {
    /// Wraps a streamed file picker chunk.
    fn from(value: Vec<FilePickerItem>) -> Self {
        Self::FileSearchChunk(value)
    }
}

impl From<Vec<GrepPickerItem>> for JobPayload {
    /// Wraps a streamed grep picker chunk.
    fn from(value: Vec<GrepPickerItem>) -> Self {
        Self::GrepSearchChunk(value)
    }
}

impl From<Vec<DocSymbolsPickerItem>> for JobPayload {
    /// Wraps a document symbol picker result set.
    fn from(value: Vec<DocSymbolsPickerItem>) -> Self {
        Self::DocSymbolsSearch(value)
    }
}
