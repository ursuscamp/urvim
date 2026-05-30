//! Background job for LSP rename requests.

use crate::background::{JobContext, JobEvent, JobPayload};
use crate::buffer::BufferId;
use crate::buffer::Cursor;
use crate::globals;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_LSP_RENAME_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Background rename request payload.
#[derive(Debug, Clone)]
pub struct LspRenameJob {
    buffer_id: BufferId,
    cursor: Cursor,
    new_name: String,
}

impl LspRenameJob {
    /// Creates a new rename job payload.
    pub fn new(buffer_id: BufferId, cursor: Cursor, new_name: String) -> Self {
        Self {
            buffer_id,
            cursor,
            new_name,
        }
    }

    /// Returns the next rename generation token.
    pub fn next_generation() -> u64 {
        NEXT_LSP_RENAME_GENERATION.fetch_add(1, Ordering::SeqCst)
    }

    /// Runs the rename against the global LSP runtime.
    pub fn run(self, context: &JobContext, event_tx: &Sender<JobEvent>) {
        event_tx
            .send(JobEvent::Started {
                kind: context.kind().clone(),
                token: context.token(),
            })
            .ok();

        let result = globals::with_lsp_runtime_mut(|runtime| {
            runtime.rename_buffer(self.buffer_id, self.cursor, self.new_name.as_str())
        })
        .ok_or_else(|| "LSP runtime is not available".to_string())
        .and_then(|result| result);

        event_tx
            .send(JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(JobPayload::LspRename(result)),
            })
            .ok();
    }
}
