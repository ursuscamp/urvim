use std::fmt;

use crate::buffer::BufferCacheRefreshResult;
use crate::ui::doc_symbols_picker::DocSymbolsPickerItem;
use crate::ui::file_picker::FilePickerItem;
use crate::ui::grep_picker::GrepPickerItem;
use crate::ui::picker_preview::PreviewSyntaxRefreshResult;

use super::error::JobError;
use super::token::{JobKind, JobToken};

/// Controls how queued jobs behave when newer work supersedes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobSubmissionMode {
    /// Keep every queued job until it runs or is superseded.
    Standard,
    /// Keep only the newest queued job for the same kind.
    LatestOnly,
}

/// Payloads produced by background jobs.
#[derive(Debug, Clone)]
pub enum JobPayload {
    /// Buffer cache refresh result.
    BufferCacheRefresh(BufferCacheRefreshResult),
    /// File picker search chunk.
    FileSearchChunk(Vec<FilePickerItem>),
    /// Live grep picker search chunk.
    GrepSearchChunk(Vec<GrepPickerItem>),
    /// Document symbol picker search chunk.
    DocSymbolsSearchChunk(Vec<DocSymbolsPickerItem>),
    /// Picker preview syntax refresh result.
    PreviewSyntax(PreviewSyntaxRefreshResult),
    /// LSP rename outcome.
    LspRename(Result<(), String>),
}

/// A job lifecycle event.
#[derive(Clone)]
pub enum JobEvent {
    /// The job started running.
    Started {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
    },
    /// The job produced a chunk of output.
    Chunk {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The chunk payload produced by the job.
        payload: JobPayload,
    },
    /// The job completed successfully.
    Completed {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The output produced by the job, if any.
        payload: Option<JobPayload>,
    },
    /// The job panicked while running.
    Failed {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The captured failure reason.
        error: JobError,
    },
}

impl JobEvent {
    /// Returns the job kind associated with this event.
    pub fn kind(&self) -> &JobKind {
        match self {
            Self::Started { kind, .. }
            | Self::Chunk { kind, .. }
            | Self::Completed { kind, .. }
            | Self::Failed { kind, .. } => kind,
        }
    }

    /// Returns the token associated with this event.
    pub fn token(&self) -> JobToken {
        match self {
            Self::Started { token, .. }
            | Self::Chunk { token, .. }
            | Self::Completed { token, .. }
            | Self::Failed { token, .. } => *token,
        }
    }

    /// Returns true when this event is a successful lifecycle event.
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Returns true when this event marks the start of a job.
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Started { .. })
    }

    /// Returns true when this event carries streamed output.
    pub fn is_chunk(&self) -> bool {
        matches!(self, Self::Chunk { .. })
    }

    /// Returns true when this event is a terminal completion.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }

    /// Returns the chunk payload if this event is a chunk.
    pub fn into_chunk_payload(self) -> Result<(JobKind, JobToken, JobPayload), Self> {
        match self {
            Self::Chunk {
                kind,
                token,
                payload,
            } => Ok((kind, token, payload)),
            other => Err(other),
        }
    }

    /// Returns the completion payload if this event is a terminal completion.
    pub fn into_completed_payload(self) -> Result<(JobKind, JobToken, Option<JobPayload>), Self> {
        match self {
            Self::Completed {
                kind,
                token,
                payload,
            } => Ok((kind, token, payload)),
            other => Err(other),
        }
    }
}

impl fmt::Debug for JobEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started { kind, token } => f
                .debug_struct("JobEvent::Started")
                .field("kind", kind)
                .field("token", token)
                .finish(),
            Self::Chunk { kind, token, .. } => f
                .debug_struct("JobEvent::Chunk")
                .field("kind", kind)
                .field("token", token)
                .field("payload", &"<opaque>")
                .finish(),
            Self::Completed { kind, token, .. } => f
                .debug_struct("JobEvent::Completed")
                .field("kind", kind)
                .field("token", token)
                .field("payload", &"<opaque>")
                .finish(),
            Self::Failed { kind, token, error } => f
                .debug_struct("JobEvent::Failed")
                .field("kind", kind)
                .field("token", token)
                .field("error", error)
                .finish(),
        }
    }
}
