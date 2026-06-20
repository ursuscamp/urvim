//! Core adapter for diff refresh jobs.

use super::BufferId;
use crate::path::AbsolutePath;

pub use urvim_diff::*;

/// Result produced by a background diff refresh job.
#[derive(Debug, Clone)]
pub struct DiffRefreshResult {
    /// Buffer receiving the refreshed diff data.
    pub buffer_id: BufferId,
    /// Generation used when the refresh was requested.
    pub generation: u64,
    /// Whether the file is tracked.
    pub tracked: bool,
    /// Normalized diff hunks.
    pub hunks: Vec<DiffHunk>,
}

/// Background job that refreshes a buffer's diff cache.
#[derive(Debug)]
pub struct DiffRefreshJob {
    buffer_id: BufferId,
    generation: u64,
    path: AbsolutePath,
    lines: Vec<String>,
}

impl DiffRefreshJob {
    /// Creates a new diff refresh job.
    pub fn new(
        buffer_id: BufferId,
        generation: u64,
        path: AbsolutePath,
        lines: Vec<String>,
    ) -> Self {
        Self {
            buffer_id,
            generation,
            path,
            lines,
        }
    }

    /// Runs the diff refresh job on a worker thread.
    pub fn run(
        self,
        context: &crate::background::JobContext,
        event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    ) {
        let provider = GitDiffProvider;
        let input = DiffInput {
            path: Some(self.path.as_path()),
            lines: &self.lines,
        };

        let snapshot = provider
            .collect(&input)
            .unwrap_or_else(|_| DiffSnapshot::untracked());
        event_tx
            .send(crate::background::JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(crate::background::JobPayload::DiffRefresh(
                    DiffRefreshResult {
                        buffer_id: self.buffer_id,
                        generation: self.generation,
                        tracked: snapshot.tracked,
                        hunks: snapshot.hunks,
                    },
                )),
            })
            .ok();
    }
}
