//! Internal job framework for deferred editor work.
//!
//! The job framework runs deferred work on a pool of background worker threads
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
pub use event::{JobEvent, JobPayload, JobSubmissionMode, LspInlayHint, LspInlayHintsChunk};
pub use handle::JobHandle;
pub use manager::JobManager;
pub use token::{JobKind, JobToken};

use std::sync::mpsc::Sender;

use crate::buffer::{
    IndentScopeRefreshJob, IndentScopeRefreshResult, SyntaxRefreshJob, SyntaxRefreshResult,
};
use crate::lsp::inlay_hint_job::LspInlayHintJob;
use crate::lsp::rename_job::LspRenameJob;
use crate::ui::picker::doc_symbols::{DocSymbolsPickerItem, DocSymbolsPickerSearchJob};
use crate::ui::picker::file::{FilePickerItem, PickerSearchJob};
use crate::ui::picker::grep::{GrepPickerItem, GrepPickerSearchJob};
use crate::ui::picker::preview::PreviewSyntaxRefreshJob;

/// Concrete background jobs known to the editor.
#[derive(Debug)]
pub enum BackgroundJob {
    /// Refreshes the syntax cache for a buffer.
    SyntaxRefresh(SyntaxRefreshJob),
    /// Refreshes the indent scope cache for a buffer.
    IndentScopeRefresh(IndentScopeRefreshJob),
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
    /// Streams LSP inlay hints on a background thread.
    LspInlayHints(LspInlayHintJob),
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
            Self::SyntaxRefresh(job) => job.run(context, event_tx),
            Self::IndentScopeRefresh(job) => job.run(context, event_tx),
            Self::FilePickerSearch(job) => job.run(context, event_tx),
            Self::GrepPickerSearch(job) => job.run(context, event_tx),
            Self::DocSymbolsPickerSearch(job) => job.run(context, event_tx),
            Self::PickerPreviewSyntax(job) => job.run(context, event_tx),
            Self::LspRename(job) => job.run(context, event_tx),
            Self::LspInlayHints(job) => job.run(context, event_tx),
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

impl From<SyntaxRefreshJob> for BackgroundJob {
    fn from(value: SyntaxRefreshJob) -> Self {
        Self::SyntaxRefresh(value)
    }
}

impl From<IndentScopeRefreshJob> for BackgroundJob {
    fn from(value: IndentScopeRefreshJob) -> Self {
        Self::IndentScopeRefresh(value)
    }
}

impl From<PickerSearchJob> for BackgroundJob {
    fn from(value: PickerSearchJob) -> Self {
        Self::FilePickerSearch(value)
    }
}

impl From<GrepPickerSearchJob> for BackgroundJob {
    fn from(value: GrepPickerSearchJob) -> Self {
        Self::GrepPickerSearch(value)
    }
}

impl From<DocSymbolsPickerSearchJob> for BackgroundJob {
    fn from(value: DocSymbolsPickerSearchJob) -> Self {
        Self::DocSymbolsPickerSearch(value)
    }
}

impl From<PreviewSyntaxRefreshJob> for BackgroundJob {
    fn from(value: PreviewSyntaxRefreshJob) -> Self {
        Self::PickerPreviewSyntax(value)
    }
}

impl From<LspRenameJob> for BackgroundJob {
    fn from(value: LspRenameJob) -> Self {
        Self::LspRename(value)
    }
}

impl From<LspInlayHintJob> for BackgroundJob {
    fn from(value: LspInlayHintJob) -> Self {
        Self::LspInlayHints(value)
    }
}

impl From<SyntaxRefreshResult> for JobPayload {
    fn from(value: SyntaxRefreshResult) -> Self {
        Self::SyntaxRefresh(value)
    }
}

impl From<IndentScopeRefreshResult> for JobPayload {
    fn from(value: IndentScopeRefreshResult) -> Self {
        Self::IndentScopeRefresh(value)
    }
}

impl From<Vec<FilePickerItem>> for JobPayload {
    fn from(value: Vec<FilePickerItem>) -> Self {
        Self::FileSearchChunk(value)
    }
}

impl From<Vec<GrepPickerItem>> for JobPayload {
    fn from(value: Vec<GrepPickerItem>) -> Self {
        Self::GrepSearchChunk(value)
    }
}

impl From<Vec<DocSymbolsPickerItem>> for JobPayload {
    fn from(value: Vec<DocSymbolsPickerItem>) -> Self {
        Self::DocSymbolsSearch(value)
    }
}
